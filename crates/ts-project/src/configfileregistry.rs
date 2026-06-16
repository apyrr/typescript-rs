use std::collections::{HashMap, HashSet};
use std::fmt;

use lsp_types_full as lsproto;
use ts_core as core;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use crate::{PatternsAndIgnored, PendingReload, WatchedFiles, new_watched_files};

pub struct ConfigFileRegistry {
    // configs is a map of config file paths to their entries.
    pub configs: HashMap<tspath::Path, ConfigFileEntry>,
    // configFileNames is a map of open file paths to information
    // about their ancestor config file names. It is only used as
    // a cache during
    pub config_file_names: HashMap<tspath::Path, ConfigFileNames>,
    // customConfigFileName is the custom config file name preference that was
    // used when building this registry's configFileNames cache.
    pub custom_config_file_name: String,
}

impl Default for ConfigFileRegistry {
    fn default() -> Self {
        Self {
            configs: HashMap::new(),
            config_file_names: HashMap::new(),
            custom_config_file_name: String::new(),
        }
    }
}

impl Clone for ConfigFileRegistry {
    fn clone(&self) -> Self {
        self.clone_registry()
    }
}

impl fmt::Debug for ConfigFileRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigFileRegistry")
            .field("configs_len", &self.configs.len())
            .field("config_file_names_len", &self.config_file_names.len())
            .field("custom_config_file_name", &self.custom_config_file_name)
            .finish()
    }
}

pub struct ConfigFileEntry {
    pub file_name: String,
    pub pending_reload: PendingReload,
    pub command_line: Option<tsoptions::ParsedCommandLine>,
    // retainingProjects is the set of projects that have called acquireConfig
    // without releasing it. A config file entry may be acquired by a project
    // either because it is the config for that project or because it is the
    // config for a referenced project.
    pub retaining_projects: HashSet<tspath::Path>,
    // retainingOpenFiles is the set of open files that caused this config to
    // load during project collection building. This config file may or may not
    // end up being the config for the default project for these files, but
    // determining the default project loaded this config as a candidate, so
    // subsequent calls to `projectCollectionBuilder.findDefaultConfiguredProject`
    // will use this config as part of the search, so it must be retained.
    pub retaining_open_files: HashSet<tspath::Path>,
    // retainingConfigs is the set of config files that extend this one. This
    // provides a cheap reverse mapping for a project config's
    // `commandLine.ExtendedSourceFiles()` that can be used to notify the
    // extending projects when this config changes. An extended config file may
    // or may not also be used directly by a project, so it's possible that
    // when this is set, no other fields will be used.
    pub retaining_configs: HashSet<tspath::Path>,
    // rootFilesWatch is a watch for the root files of this config file.
    pub root_files_watch: Option<WatchedFiles<PatternsAndIgnored>>,
}

impl Default for ConfigFileEntry {
    fn default() -> Self {
        Self {
            file_name: String::new(),
            pending_reload: PendingReload::None,
            command_line: None,
            retaining_projects: HashSet::new(),
            retaining_open_files: HashSet::new(),
            retaining_configs: HashSet::new(),
            root_files_watch: None,
        }
    }
}

pub fn new_config_file_entry(
    has_relative_pattern_capability: bool,
    file_name: String,
) -> ConfigFileEntry {
    ConfigFileEntry {
        file_name: file_name.clone(),
        pending_reload: PendingReload::Full,
        command_line: None,
        retaining_projects: HashSet::new(),
        retaining_open_files: HashSet::new(),
        retaining_configs: HashSet::new(),
        root_files_watch: Some(new_watched_files(
            format!("root files for {file_name}"),
            lsproto::WatchKind::Create | lsproto::WatchKind::Change | lsproto::WatchKind::Delete,
            has_relative_pattern_capability,
            core::identity,
        )),
    }
}

pub fn new_extended_config_file_entry(
    file_name: String,
    extending_config_path: tspath::Path,
) -> ConfigFileEntry {
    let mut retaining_configs = HashSet::new();
    retaining_configs.insert(extending_config_path);
    ConfigFileEntry {
        file_name,
        pending_reload: PendingReload::Full,
        command_line: None,
        retaining_projects: HashSet::new(),
        retaining_open_files: HashSet::new(),
        retaining_configs,
        root_files_watch: None,
    }
}

impl Clone for ConfigFileEntry {
    fn clone(&self) -> Self {
        Self {
            file_name: self.file_name.clone(),
            pending_reload: self.pending_reload,
            command_line: self.command_line.clone(),
            // !!! eagerly cloning these maps makes everything more convenient,
            // but it could be avoided if needed.
            retaining_projects: self.retaining_projects.clone(),
            retaining_open_files: self.retaining_open_files.clone(),
            retaining_configs: self.retaining_configs.clone(),
            root_files_watch: self.root_files_watch.clone(),
        }
    }
}

impl ConfigFileRegistry {
    pub fn get_config(&self, path: tspath::Path) -> Option<tsoptions::ParsedCommandLine> {
        self.configs
            .get(&path)
            .and_then(|entry| entry.command_line.clone())
    }

    pub fn get_config_file_name(&self, path: tspath::Path) -> String {
        self.config_file_names
            .get(&path)
            .map(|entry| entry.nearest_config_file_name.clone())
            .unwrap_or_default()
    }

    pub fn get_ancestor_config_file_name(
        &self,
        path: tspath::Path,
        higher_than_config: &str,
    ) -> String {
        self.config_file_names
            .get(&path)
            .and_then(|entry| entry.ancestors.get(higher_than_config).cloned())
            .unwrap_or_default()
    }

    // clone creates a shallow copy of the configFileRegistry.
    pub fn clone_registry(&self) -> ConfigFileRegistry {
        ConfigFileRegistry {
            configs: self.configs.clone(),
            config_file_names: self.config_file_names.clone(),
            custom_config_file_name: self.custom_config_file_name.clone(),
        }
    }

    // For testing
    pub fn for_each_test_config_entry(&self, mut cb: impl FnMut(tspath::Path, &TestConfigEntry)) {
        for (path, entry) in &self.configs {
            let test_entry = TestConfigEntry {
                file_name: entry.file_name.clone(),
                retaining_projects: entry.retaining_projects.iter().cloned().collect(),
                retaining_open_files: entry.retaining_open_files.iter().cloned().collect(),
                retaining_configs: entry.retaining_configs.iter().cloned().collect(),
            };
            cb(path.clone(), &test_entry);
        }
    }

    // For testing
    pub fn get_test_config_entry(&self, path: tspath::Path) -> Option<TestConfigEntry> {
        self.configs.get(&path).map(|entry| TestConfigEntry {
            file_name: entry.file_name.clone(),
            retaining_projects: entry.retaining_projects.iter().cloned().collect(),
            retaining_open_files: entry.retaining_open_files.iter().cloned().collect(),
            retaining_configs: entry.retaining_configs.iter().cloned().collect(),
        })
    }

    // For testing
    pub fn for_each_test_config_file_names_entry(
        &self,
        mut cb: impl FnMut(tspath::Path, &TestConfigFileNamesEntry),
    ) {
        for (path, entry) in &self.config_file_names {
            let test_entry = TestConfigFileNamesEntry {
                nearest_config_file_name: entry.nearest_config_file_name.clone(),
                ancestors: entry.ancestors.clone(),
            };
            cb(path.clone(), &test_entry);
        }
    }

    // For testing
    pub fn get_test_config_file_names_entry(
        &self,
        path: tspath::Path,
    ) -> Option<TestConfigFileNamesEntry> {
        self.config_file_names
            .get(&path)
            .map(|entry| TestConfigFileNamesEntry {
                nearest_config_file_name: entry.nearest_config_file_name.clone(),
                ancestors: entry.ancestors.clone(),
            })
    }
}

pub struct TestConfigEntry {
    pub file_name: String,
    pub retaining_projects: Vec<tspath::Path>,
    pub retaining_open_files: Vec<tspath::Path>,
    pub retaining_configs: Vec<tspath::Path>,
}

pub struct TestConfigFileNamesEntry {
    pub nearest_config_file_name: String,
    pub ancestors: HashMap<String, String>,
}

pub struct ConfigFileNames {
    // nearestConfigFileName is the file name of the nearest ancestor config file.
    pub nearest_config_file_name: String,
    // ancestors is a map from one ancestor config file path to the next.
    // For example, if `/a`, `/a/b`, and `/a/b/c` all contain config files,
    // the fully loaded map will look like:
    //		{
    //			"/a/b/c/tsconfig.json": "/a/b/tsconfig.json",
    //			"/a/b/tsconfig.json": "/a/tsconfig.json"
    //		}
    pub ancestors: HashMap<String, String>,
}

impl Default for ConfigFileNames {
    fn default() -> Self {
        Self {
            nearest_config_file_name: String::new(),
            ancestors: HashMap::new(),
        }
    }
}

impl Clone for ConfigFileNames {
    fn clone(&self) -> Self {
        Self {
            nearest_config_file_name: self.nearest_config_file_name.clone(),
            ancestors: self.ancestors.clone(),
        }
    }
}

impl crate::dirty::Cloneable<ConfigFileNames> for ConfigFileNames {
    fn clone_value(&self) -> ConfigFileNames {
        self.clone()
    }
}
