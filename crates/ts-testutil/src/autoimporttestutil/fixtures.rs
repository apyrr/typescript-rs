use std::collections::HashMap;

use serde_json::Value;
use ts_tspath as tspath;

#[derive(Clone, Default)]
pub struct FileHandle {
    file_name: String,
    content: String,
}

impl FileHandle {
    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn uri(&self) -> String {
        format!("file://{}", self.file_name)
    }
}

#[derive(Clone, Default)]
pub struct ProjectFileHandle {
    pub file_handle: FileHandle,
    pub export_identifier: String,
}

#[derive(Clone, Default)]
pub struct NodeModulesPackageHandle {
    pub name: String,
    pub directory: String,
    package_json: FileHandle,
    declaration: FileHandle,
}

impl NodeModulesPackageHandle {
    pub fn package_json_file(&self) -> FileHandle {
        self.package_json.clone()
    }

    pub fn declaration_file(&self) -> FileHandle {
        self.declaration.clone()
    }
}

#[derive(Clone, Default)]
pub struct ProjectHandle {
    root: String,
    files: Vec<ProjectFileHandle>,
    tsconfig: FileHandle,
    package_json: FileHandle,
    node_modules: Vec<NodeModulesPackageHandle>,
    dependencies: Vec<String>,
}

impl ProjectHandle {
    pub fn root(&self) -> &str {
        &self.root
    }

    pub fn files(&self) -> Vec<ProjectFileHandle> {
        self.files.clone()
    }

    pub fn file(&self, index: usize) -> ProjectFileHandle {
        self.files
            .get(index)
            .cloned()
            .unwrap_or_else(|| panic!("file index {index} out of range"))
    }

    pub fn tsconfig(&self) -> FileHandle {
        self.tsconfig.clone()
    }

    pub fn package_json_file(&self) -> FileHandle {
        self.package_json.clone()
    }

    pub fn node_modules(&self) -> Vec<NodeModulesPackageHandle> {
        self.node_modules.clone()
    }

    pub fn dependencies(&self) -> Vec<String> {
        self.dependencies.clone()
    }

    pub fn node_module_by_name(&self, name: &str) -> Option<NodeModulesPackageHandle> {
        self.node_modules
            .iter()
            .find(|pkg| pkg.name == name)
            .cloned()
    }
}

#[derive(Clone, Default)]
pub struct MonorepoHandle {
    root: String,
    root_node_modules: Vec<NodeModulesPackageHandle>,
    root_dependencies: Vec<String>,
    packages: Vec<ProjectHandle>,
    root_tsconfig: FileHandle,
    root_package_json: FileHandle,
}

impl MonorepoHandle {
    pub fn root(&self) -> &str {
        &self.root
    }

    pub fn root_node_modules(&self) -> Vec<NodeModulesPackageHandle> {
        self.root_node_modules.clone()
    }

    pub fn root_dependencies(&self) -> Vec<String> {
        self.root_dependencies.clone()
    }

    pub fn packages(&self) -> Vec<ProjectHandle> {
        self.packages.clone()
    }

    pub fn package(&self, index: usize) -> ProjectHandle {
        self.packages
            .get(index)
            .cloned()
            .unwrap_or_else(|| panic!("package index {index} out of range"))
    }

    pub fn root_tsconfig(&self) -> FileHandle {
        self.root_tsconfig.clone()
    }

    pub fn root_package_json_file(&self) -> FileHandle {
        self.root_package_json.clone()
    }
}

#[derive(Default)]
pub struct Fixture {
    pub files: HashMap<String, Value>,
    projects: Vec<ProjectHandle>,
}

impl Fixture {
    pub fn projects(&self) -> Vec<ProjectHandle> {
        self.projects.clone()
    }

    pub fn project(&self, index: usize) -> ProjectHandle {
        self.projects
            .get(index)
            .cloned()
            .unwrap_or_else(|| panic!("project index {index} out of range"))
    }

    pub fn single_project(&self) -> ProjectHandle {
        self.project(0)
    }
}

#[derive(Default)]
pub struct MonorepoFixture {
    pub files: HashMap<String, Value>,
    monorepo: MonorepoHandle,
    extra: Vec<FileHandle>,
}

impl MonorepoFixture {
    pub fn monorepo(&self) -> MonorepoHandle {
        self.monorepo.clone()
    }

    pub fn extra_files(&self) -> Vec<FileHandle> {
        self.extra.clone()
    }

    pub fn extra_file(&self, path: &str) -> FileHandle {
        let normalized = normalize_absolute_path(path);
        self.extra
            .iter()
            .find(|handle| handle.file_name == normalized)
            .cloned()
            .unwrap_or_else(|| panic!("extra file not found: {path}"))
    }
}

#[derive(Clone, Default)]
pub struct MonorepoPackageTemplate {
    pub name: String,
    pub node_module_names: Vec<String>,
    pub dependency_names: Vec<String>,
}

#[derive(Clone, Default)]
pub struct MonorepoSetupConfig {
    pub root: String,
    pub package_template: MonorepoPackageTemplate,
    pub packages: Vec<MonorepoPackageConfig>,
    pub extra_files: Vec<TextFileSpec>,
    pub symlinks: Vec<SymlinkSpec>,
}

#[derive(Clone, Default)]
pub struct MonorepoPackageConfig {
    pub file_count: usize,
    pub package_template: MonorepoPackageTemplate,
}

#[derive(Clone, Default)]
pub struct TextFileSpec {
    pub path: String,
    pub content: String,
}

#[derive(Clone, Default)]
pub struct SymlinkSpec {
    pub link: String,
    pub target: String,
}

pub fn setup_lifecycle_session(project_root: &str, file_count: usize) -> Fixture {
    let mut builder = FileMapBuilder::new(None);
    builder.add_local_project(project_root, file_count);
    let node_modules_dir = combine_paths(project_root, "node_modules");
    let deps = builder.add_node_modules_packages(&node_modules_dir, 1);
    builder.add_package_json_with_dependencies(project_root, &deps);
    Fixture {
        files: builder.files(),
        projects: builder.project_handles(),
    }
}

pub fn setup_monorepo_lifecycle_session(config: MonorepoSetupConfig) -> MonorepoFixture {
    let mut builder = FileMapBuilder::new(None);
    let monorepo_root = normalize_absolute_path(&config.root);
    let monorepo_name = if config.package_template.name.is_empty() {
        "monorepo".to_owned()
    } else {
        config.package_template.name.clone()
    };

    let root_tsconfig_path = combine_paths(&monorepo_root, "tsconfig.json");
    let root_tsconfig_content = default_tsconfig(true);
    builder.add_text_file(&root_tsconfig_path, &root_tsconfig_content);
    let root_tsconfig = FileHandle {
        file_name: root_tsconfig_path,
        content: root_tsconfig_content,
    };

    let root_node_modules_dir = combine_paths(&monorepo_root, "node_modules");
    let root_node_modules = builder.add_node_modules_packages_with_names(
        &root_node_modules_dir,
        &config.package_template.node_module_names,
    );
    let root_dependencies = select_packages_by_name(
        &root_node_modules,
        &config.package_template.dependency_names,
    );
    let root_package_json =
        builder.add_root_package_json(&monorepo_root, &monorepo_name, &root_dependencies);

    let packages_dir = combine_paths(&monorepo_root, "packages");
    for package in &config.packages {
        let pkg_dir = combine_paths(&packages_dir, &package.package_template.name);
        builder.add_local_project(&pkg_dir, package.file_count);
        let mut pkg_node_modules = Vec::new();
        if !package.package_template.node_module_names.is_empty() {
            let pkg_node_modules_dir = combine_paths(&pkg_dir, "node_modules");
            pkg_node_modules = builder.add_node_modules_packages_with_names(
                &pkg_node_modules_dir,
                &package.package_template.node_module_names,
            );
        }
        let mut available = root_node_modules.clone();
        available.extend(pkg_node_modules);
        let selected =
            select_packages_by_name(&available, &package.package_template.dependency_names);
        if !selected.is_empty() {
            builder.add_package_json_with_dependencies_named(
                &pkg_dir,
                &package.package_template.name,
                &selected,
            );
        }
    }

    let mut extra = Vec::with_capacity(config.extra_files.len());
    for file in &config.extra_files {
        builder.add_text_file(&file.path, &file.content);
        extra.push(FileHandle {
            file_name: normalize_absolute_path(&file.path),
            content: file.content.clone(),
        });
    }
    for symlink in &config.symlinks {
        builder.add_symlink(&symlink.link, &symlink.target);
    }

    let package_handles = config
        .packages
        .iter()
        .filter_map(|pkg| {
            let pkg_dir = combine_paths(&packages_dir, &pkg.package_template.name);
            builder
                .projects
                .get(&pkg_dir)
                .map(ProjectRecord::to_handles)
        })
        .collect::<Vec<_>>();
    let root_node_modules_handles = builder
        .projects
        .get(&monorepo_root)
        .map(|record| record.node_modules.clone())
        .unwrap_or_default();

    MonorepoFixture {
        files: builder.files(),
        monorepo: MonorepoHandle {
            root: monorepo_root,
            root_node_modules: root_node_modules_handles,
            root_dependencies: package_names(&root_dependencies),
            packages: package_handles,
            root_tsconfig,
            root_package_json,
        },
        extra,
    }
}

struct FileMapBuilder {
    files: HashMap<String, Value>,
    next_package_id: usize,
    next_project_id: usize,
    projects: HashMap<String, ProjectRecord>,
}

#[derive(Clone, Default)]
struct ProjectRecord {
    root: String,
    source_files: Vec<ProjectFile>,
    tsconfig: FileHandle,
    package_json: Option<FileHandle>,
    node_modules: Vec<NodeModulesPackageHandle>,
    dependencies: Vec<String>,
}

#[derive(Clone)]
struct ProjectFile {
    file_name: String,
    export_identifier: String,
    content: String,
}

impl FileMapBuilder {
    fn new(initial: Option<HashMap<String, Value>>) -> Self {
        let mut builder = Self {
            files: HashMap::new(),
            next_package_id: 0,
            next_project_id: 0,
            projects: HashMap::new(),
        };
        if let Some(initial) = initial {
            for (path, content) in initial {
                builder
                    .files
                    .insert(normalize_absolute_path(&path), content);
            }
        }
        builder
    }

    fn ensure_project_record(&mut self, root: &str) -> &mut ProjectRecord {
        self.projects
            .entry(root.to_owned())
            .or_insert_with(|| ProjectRecord {
                root: root.to_owned(),
                ..Default::default()
            })
    }

    fn project_handles(&self) -> Vec<ProjectHandle> {
        let mut keys = self.projects.keys().cloned().collect::<Vec<_>>();
        keys.sort();
        keys.into_iter()
            .filter_map(|key| self.projects.get(&key).map(ProjectRecord::to_handles))
            .collect()
    }

    fn files(&self) -> HashMap<String, Value> {
        self.files.clone()
    }

    fn add_text_file(&mut self, path: &str, contents: &str) {
        self.files.insert(
            normalize_absolute_path(path),
            Value::String(contents.to_owned()),
        );
    }

    fn add_symlink(&mut self, link_path: &str, target_path: &str) {
        self.files.insert(
            normalize_absolute_path(link_path),
            Value::String(format!("symlink:{}", normalize_absolute_path(target_path))),
        );
    }

    fn add_node_modules_packages(
        &mut self,
        node_modules_dir: &str,
        count: usize,
    ) -> Vec<NodeModulesPackageHandle> {
        (0..count)
            .map(|_| self.add_named_node_modules_package(node_modules_dir, ""))
            .collect()
    }

    fn add_node_modules_packages_with_names(
        &mut self,
        node_modules_dir: &str,
        names: &[String],
    ) -> Vec<NodeModulesPackageHandle> {
        names
            .iter()
            .map(|name| self.add_named_node_modules_package(node_modules_dir, name))
            .collect()
    }

    fn add_named_node_modules_package(
        &mut self,
        node_modules_dir: &str,
        name: &str,
    ) -> NodeModulesPackageHandle {
        let normalized_dir = normalize_absolute_path(node_modules_dir);
        if base_file_name(&normalized_dir) != "node_modules" {
            panic!("nodeModulesDir must point to a node_modules directory: {node_modules_dir}");
        }
        self.next_package_id += 1;
        let resolved_name = if name.is_empty() {
            format!("pkg{}", self.next_package_id)
        } else {
            name.to_owned()
        };
        let export_name = format!("{}_value", sanitize_identifier(&resolved_name));
        let pkg_dir = combine_paths(&normalized_dir, &resolved_name);
        let package_json_path = combine_paths(&pkg_dir, "package.json");
        let package_json_content = format!(r#"{{"name":"{resolved_name}","types":"index.d.ts"}}"#);
        self.files.insert(
            package_json_path.clone(),
            Value::String(package_json_content.clone()),
        );
        let declaration_path = combine_paths(&pkg_dir, "index.d.ts");
        let declaration_content = format!("export declare const {export_name}: number;\n");
        self.files.insert(
            declaration_path.clone(),
            Value::String(declaration_content.clone()),
        );
        let package_handle = NodeModulesPackageHandle {
            name: resolved_name,
            directory: pkg_dir,
            package_json: FileHandle {
                file_name: package_json_path,
                content: package_json_content,
            },
            declaration: FileHandle {
                file_name: declaration_path,
                content: declaration_content,
            },
        };
        let project_root = directory_path(&normalized_dir);
        self.ensure_project_record(&project_root)
            .node_modules
            .push(package_handle.clone());
        package_handle
    }

    fn add_local_project(&mut self, project_dir: &str, file_count: usize) {
        let dir = normalize_absolute_path(project_dir);
        self.next_project_id += 1;
        let project_id = self.next_project_id;
        let tsconfig_path = combine_paths(&dir, "tsconfig.json");
        let tsconfig_content = default_tsconfig(false);
        self.files.insert(
            tsconfig_path.clone(),
            Value::String(tsconfig_content.clone()),
        );
        let mut source_files = Vec::with_capacity(file_count);
        for i in 1..=file_count {
            let path = combine_paths(&dir, &format!("file{i}.ts"));
            let export_name = format!("localExport{project_id}_{i}");
            let content = format!("export const {export_name} = {i};\n");
            self.files
                .insert(path.clone(), Value::String(content.clone()));
            source_files.push(ProjectFile {
                file_name: path,
                export_identifier: export_name,
                content,
            });
        }
        let record = self.ensure_project_record(&dir);
        record.tsconfig = FileHandle {
            file_name: tsconfig_path,
            content: tsconfig_content,
        };
        record.source_files.extend(source_files);
    }

    fn add_package_json_with_dependencies(
        &mut self,
        project_dir: &str,
        deps: &[NodeModulesPackageHandle],
    ) -> FileHandle {
        self.next_project_id += 1;
        self.add_package_json_with_dependencies_named(
            project_dir,
            &format!("local-project-{}", self.next_project_id),
            deps,
        )
    }

    fn add_package_json_with_dependencies_named(
        &mut self,
        project_dir: &str,
        package_name: &str,
        deps: &[NodeModulesPackageHandle],
    ) -> FileHandle {
        let dir = normalize_absolute_path(project_dir);
        let package_json_path = combine_paths(&dir, "package.json");
        let content = package_json_content(package_name, false, deps);
        self.files
            .insert(package_json_path.clone(), Value::String(content.clone()));
        let package_handle = FileHandle {
            file_name: package_json_path,
            content,
        };
        let record = self.ensure_project_record(&dir);
        record.package_json = Some(package_handle.clone());
        record.dependencies = package_names(deps);
        package_handle
    }

    fn add_root_package_json(
        &mut self,
        root_dir: &str,
        package_name: &str,
        deps: &[NodeModulesPackageHandle],
    ) -> FileHandle {
        let dir = normalize_absolute_path(root_dir);
        let package_json_path = combine_paths(&dir, "package.json");
        let name = if package_name.is_empty() {
            "monorepo-root"
        } else {
            package_name
        };
        let content = package_json_content(name, true, deps);
        self.files
            .insert(package_json_path.clone(), Value::String(content.clone()));
        FileHandle {
            file_name: package_json_path,
            content,
        }
    }
}

impl ProjectRecord {
    fn to_handles(&self) -> ProjectHandle {
        ProjectHandle {
            root: self.root.clone(),
            files: self
                .source_files
                .iter()
                .map(|file| ProjectFileHandle {
                    file_handle: FileHandle {
                        file_name: file.file_name.clone(),
                        content: file.content.clone(),
                    },
                    export_identifier: file.export_identifier.clone(),
                })
                .collect(),
            tsconfig: self.tsconfig.clone(),
            package_json: self.package_json.clone().unwrap_or_default(),
            node_modules: self.node_modules.clone(),
            dependencies: self.dependencies.clone(),
        }
    }
}

fn select_packages_by_name(
    available: &[NodeModulesPackageHandle],
    names: &[String],
) -> Vec<NodeModulesPackageHandle> {
    if names.is_empty() {
        return available.to_vec();
    }
    names
        .iter()
        .map(|name| {
            available
                .iter()
                .find(|candidate| candidate.name == *name)
                .cloned()
                .unwrap_or_else(|| panic!("dependency not found: {name}"))
        })
        .collect()
}

fn package_names(deps: &[NodeModulesPackageHandle]) -> Vec<String> {
    deps.iter().map(|dep| dep.name.clone()).collect()
}

fn sanitize_identifier(name: &str) -> String {
    let sanitized = name
        .chars()
        .filter_map(|ch| {
            if ch.is_ascii_alphanumeric() {
                Some(ch)
            } else if ch == '_' || ch == '-' {
                Some('_')
            } else {
                None
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "pkg".to_owned()
    } else {
        sanitized
    }
}

fn normalize_absolute_path(path: &str) -> String {
    let normalized = tspath::normalize_path(path);
    if !tspath::path_is_absolute(&normalized) {
        panic!("paths used in lifecycle tests must be absolute: {path}");
    }
    normalized
}

fn combine_paths(left: &str, right: &str) -> String {
    tspath::combine_paths(left, &[right])
}

fn directory_path(path: &str) -> String {
    tspath::get_directory_path(path)
}

fn base_file_name(path: &str) -> String {
    tspath::get_base_file_name(path)
}

fn default_tsconfig(monorepo_root: bool) -> String {
    let base_url = if monorepo_root {
        "    \"baseUrl\": \".\",\n"
    } else {
        ""
    };
    format!(
        "{{\n  \"compilerOptions\": {{\n    \"module\": \"esnext\",\n    \"target\": \"esnext\",\n    \"strict\": true,\n{base_url}    \"allowJs\": true,\n    \"checkJs\": true\n  }}\n}}\n"
    )
}

fn package_json_content(name: &str, private_: bool, deps: &[NodeModulesPackageHandle]) -> String {
    let mut content = format!("{{\n  \"name\": \"{name}\"");
    if private_ {
        content.push_str(",\n  \"private\": true");
    }
    if !deps.is_empty() {
        let dependency_lines = deps
            .iter()
            .map(|dep| format!("\"{}\": \"*\"", dep.name))
            .collect::<Vec<_>>()
            .join(",\n    ");
        content.push_str(",\n  \"dependencies\": {\n    ");
        content.push_str(&dependency_lines);
        content.push_str("\n  }\n");
    } else {
        content.push('\n');
    }
    content.push_str("}\n");
    content
}
