use std::sync::Arc;

use ts_collections::{self as collections, OrderedMap};
use ts_core as core;
use ts_ls::Project as CrossProject;
use ts_tspath as tspath;

use crate::{ConfigFileRegistry, INFERRED_PROJECT_NAME, Project};

pub struct ProjectCollection {
    pub(crate) to_path: Arc<dyn Fn(String) -> tspath::Path + Send + Sync>,
    pub(crate) config_file_registry: Option<ConfigFileRegistry>,
    // fileDefaultProjects is a map of file paths to the config file path (the key
    // into `configuredProjects`) of the default project for that file. If the file
    // belongs to the inferred project, the value is `inferredProjectName`. This map
    // contains quick lookups for only the associations discovered during the latest
    // snapshot update.
    pub(crate) file_default_projects: std::collections::HashMap<tspath::Path, tspath::Path>,
    // configuredProjects is the set of loaded projects associated with a tsconfig
    // file, keyed by the config file path.
    pub(crate) configured_projects: std::collections::HashMap<tspath::Path, Project>,
    // inferredProject is a fallback project that is used when no configured
    // project can be found for an open file.
    pub(crate) inferred_project: Option<Project>,
    // apiOpenedProjects is the set of projects that should be kept open for
    // API clients.
    pub(crate) api_opened_projects: std::collections::HashMap<tspath::Path, ()>,
}

impl Default for ProjectCollection {
    fn default() -> Self {
        Self {
            to_path: Arc::new(|file_name| file_name),
            config_file_registry: None,
            file_default_projects: std::collections::HashMap::new(),
            configured_projects: std::collections::HashMap::new(),
            inferred_project: None,
            api_opened_projects: std::collections::HashMap::new(),
        }
    }
}

impl ProjectCollection {
    pub fn config_file_registry(&self) -> Option<&ConfigFileRegistry> {
        self.config_file_registry.as_ref()
    }

    pub fn configured_project(&self, path: tspath::Path) -> Option<&Project> {
        self.configured_projects.get(&path)
    }

    pub fn get_project_by_path(&self, project_path: tspath::Path) -> Option<&Project> {
        if let Some(project) = self.configured_projects.get(&project_path) {
            return Some(project);
        }

        if project_path == INFERRED_PROJECT_NAME {
            return self.inferred_project.as_ref();
        }

        None
    }

    // ConfiguredProjects returns all configured projects in a stable order.
    pub fn configured_projects(&self) -> Vec<&Project> {
        let mut projects = Vec::with_capacity(self.configured_projects.len());
        self.fill_configured_projects(&mut projects);
        projects
    }

    pub fn fill_configured_projects<'b>(&'b self, projects: &mut Vec<&'b Project>) {
        for project in self.configured_projects.values() {
            projects.push(project);
        }
        projects.sort_by(|a, b| a.name().cmp(&b.name()));
    }

    // ProjectsByPath returns an ordered map of configured projects keyed by their config file path,
    // plus the inferred project, if it exists, with the key `inferredProjectName`.
    pub fn projects_by_path(&self) -> OrderedMap<tspath::Path, &Project> {
        let mut projects = collections::new_ordered_map_with_size_hint(
            self.configured_projects.len() + core::if_else(self.inferred_project.is_some(), 1, 0),
        );
        for project in self.configured_projects() {
            projects.set(project.config_file_path.clone(), project);
        }
        if let Some(inferred_project) = &self.inferred_project {
            projects.set(INFERRED_PROJECT_NAME.to_string(), inferred_project);
        }
        projects
    }

    // Projects returns all projects, including the inferred project if it exists, in a stable order.
    pub fn projects(&self) -> Vec<&Project> {
        if self.inferred_project.is_none() {
            return self.configured_projects();
        }
        let mut projects = Vec::with_capacity(self.configured_projects.len() + 1);
        self.fill_configured_projects(&mut projects);
        projects.push(self.inferred_project.as_ref().unwrap());
        projects
    }

    pub fn inferred_project(&self) -> Option<&Project> {
        self.inferred_project.as_ref()
    }

    pub fn get_projects_containing_file(&self, path: tspath::Path) -> Vec<&dyn CrossProject> {
        let mut projects: Vec<&dyn CrossProject> = Vec::new();
        for project in self.configured_projects() {
            if project.contains_file(path.clone()) {
                projects.push(project);
            }
        }
        if self
            .inferred_project
            .as_ref()
            .is_some_and(|project| project.contains_file(path))
        {
            projects.push(self.inferred_project.as_ref().unwrap());
        }
        projects
    }

    // !!! result could be cached
    pub fn get_default_project(&self, path: tspath::Path) -> Option<&Project> {
        if let Some(result) = self.file_default_projects.get(&path) {
            if result == INFERRED_PROJECT_NAME {
                return self.inferred_project.as_ref();
            }
            return self.configured_projects.get(result);
        }

        let mut containing_projects = Vec::new();
        let mut first_configured_project = None;
        let mut first_non_source_of_project_reference_redirect = None;
        let mut multiple_direct_inclusions = false;
        for project in self.configured_projects() {
            if project.contains_file(path.clone()) {
                containing_projects.push(project);
                if !multiple_direct_inclusions
                    && !project.is_source_from_project_reference(path.clone())
                {
                    if first_non_source_of_project_reference_redirect.is_none() {
                        first_non_source_of_project_reference_redirect = Some(project);
                    } else {
                        multiple_direct_inclusions = true;
                    }
                }
                if first_configured_project.is_none() {
                    first_configured_project = Some(project);
                }
            }
        }
        if containing_projects.len() == 1 {
            return Some(containing_projects[0]);
        }
        if containing_projects.is_empty() {
            if self
                .inferred_project
                .as_ref()
                .is_some_and(|project| project.contains_file(path))
            {
                return self.inferred_project.as_ref();
            }
            return None;
        }
        if !multiple_direct_inclusions {
            if first_non_source_of_project_reference_redirect.is_some() {
                // Multiple projects include the file, but only one is a direct inclusion.
                return first_non_source_of_project_reference_redirect;
            }
            // Multiple projects include the file, and none are direct inclusions.
            return first_configured_project;
        }
        // Multiple projects include the file directly.
        if let Some(default_project) = self.find_default_configured_project(path.clone()) {
            return Some(default_project);
        }
        first_configured_project
    }

    pub fn find_default_configured_project(&self, path: tspath::Path) -> Option<&Project> {
        if let Some(config_file_registry) = &self.config_file_registry {
            let config_file_name = config_file_registry.get_config_file_name(path.clone());
            if !config_file_name.is_empty() {
                return self.find_default_configured_project_worker(
                    path,
                    config_file_name,
                    None,
                    None,
                );
            }
        }
        None
    }

    pub fn find_default_configured_project_worker<'b>(
        &'b self,
        path: tspath::Path,
        config_file_name: String,
        visited: Option<collections::SyncSet<tspath::Path>>,
        fallback: Option<&'b Project>,
    ) -> Option<&'b Project> {
        let config_file_path = (self.to_path)(config_file_name.clone());
        let project = self.configured_projects.get(&config_file_path)?;
        let visited = visited.unwrap_or_default();

        // Look in the config's project and its references recursively.
        let search = core::breadth_first_search_parallel_ex(
            project.config_file_path.clone(),
            |project_path| {
                let Some(project) = self.configured_projects.get(&project_path) else {
                    return Vec::new();
                };
                let Some(command_line) = &project.command_line else {
                    return Vec::new();
                };
                let mut command_line = command_line.clone();
                // A referenced project may not be loaded if `disableReferencedProjectLoad` is true.
                core::map_non_nil(
                    &command_line.resolved_project_reference_paths(),
                    |config_file_name| {
                        self.configured_projects
                            .get(&(self.to_path)(config_file_name.clone()))
                            .map(|project| project.config_file_path.clone())
                            .unwrap_or_default()
                    },
                )
            },
            |project_path| {
                let project = self.configured_projects.get(&project_path).unwrap();
                if project.contains_file(path.clone()) {
                    return (
                        true,
                        !project.is_source_from_project_reference(path.clone()),
                    );
                }
                (false, false)
            },
            core::BreadthFirstSearchOptions {
                visited: Some(visited),
                preprocess_level: None,
            },
            core::identity,
        );

        if search.stopped {
            // If we found a project that directly contains the file, return it.
            return self.configured_projects.get(&search.path[0]);
        }
        let mut fallback = fallback;
        if !search.path.is_empty() && fallback.is_none() {
            // If we found a project that contains the file, but it is a source from
            // a project reference, record it as a fallback.
            fallback = self.configured_projects.get(&search.path[0]);
        }

        // Look for tsconfig.json files higher up the directory tree and do the same. This handles
        // the common case where a higher-level "solution" tsconfig.json contains all projects in a
        // workspace.
        let config_file_registry = self.config_file_registry.as_ref().unwrap();
        if config_file_registry
            .get_config(path.clone())
            .is_some_and(|config| {
                config
                    .compiler_options()
                    .disable_solution_searching
                    .is_true()
            })
        {
            return fallback;
        }
        let ancestor_config_name =
            config_file_registry.get_ancestor_config_file_name(path.clone(), &config_file_name);
        if !ancestor_config_name.is_empty() {
            return self.find_default_configured_project_worker(
                path,
                ancestor_config_name,
                None,
                fallback,
            );
        }
        fallback
    }

    // clone creates a shallow copy of the project collection.
    pub fn clone_collection(&self) -> ProjectCollection {
        ProjectCollection {
            to_path: self.to_path.clone(),
            config_file_registry: self.config_file_registry.clone(),
            configured_projects: self.configured_projects.clone(),
            inferred_project: self.inferred_project.clone(),
            file_default_projects: self.file_default_projects.clone(),
            api_opened_projects: self.api_opened_projects.clone(),
        }
    }
}

// findDefaultConfiguredProjectFromProgramInclusion finds the default configured project for a file
// based on the file's inclusion in existing projects. The projects should be sorted, as ties will
// be broken by slice order. `getProject` should return a project with an up-to-date program.
// Along with the resulting project path, a boolean is returned indicating whether there were multiple
// direct inclusions of the file in different projects, indicating that the caller may want to perform
// additional logic to determine the best project.
pub fn find_default_configured_project_from_program_inclusion(
    _file_name: &str,
    path: tspath::Path,
    project_paths: Vec<tspath::Path>,
    get_project: impl Fn(tspath::Path) -> Option<Project>,
) -> (tspath::Path, bool) {
    let mut containing_projects = Vec::new();
    let mut first_configured_project = tspath::Path::default();
    let mut first_non_source_of_project_reference_redirect = tspath::Path::default();
    let mut multiple_direct_inclusions = false;

    for project_path in project_paths {
        let Some(project) = get_project(project_path.clone()) else {
            continue;
        };
        if project.contains_file(path.clone()) {
            containing_projects.push(project_path.clone());
            if !multiple_direct_inclusions
                && !project.is_source_from_project_reference(path.clone())
            {
                if first_non_source_of_project_reference_redirect.is_empty() {
                    first_non_source_of_project_reference_redirect = project_path.clone();
                } else {
                    multiple_direct_inclusions = true;
                }
            }
            if first_configured_project.is_empty() {
                first_configured_project = project_path;
            }
        }
    }

    if containing_projects.len() == 1 {
        return (containing_projects[0].clone(), false);
    }
    if !multiple_direct_inclusions {
        if !first_non_source_of_project_reference_redirect.is_empty() {
            // Multiple projects include the file, but only one is a direct inclusion.
            return (first_non_source_of_project_reference_redirect, false);
        }
        // Multiple projects include the file, and none are direct inclusions.
        return (first_configured_project, false);
    }
    // Multiple projects include the file directly.
    (first_configured_project, true)
}
