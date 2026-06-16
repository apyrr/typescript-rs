use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write;
use std::time::SystemTime;

use serde::Serialize;
use ts_collections::OrderedMap;
use ts_project::{ConfigFileRegistry, Session, SnapshotHandle};
use ts_testutil::fsbaselineutil::{FsDiffer, MapFs, MapFsEntry};
use ts_tspath as tspath;

use crate::{FourslashTest, TestFs, TestingT, is_lib_file};

pub struct StateBaseline {
    pub baseline: String,
    pub fs_differ: FsDiffer<TestFs>,
    pub is_initialized: bool,

    pub serialized_projects: BTreeMap<String, ProjectInfo>,
    pub serialized_open_files: BTreeMap<String, OpenFileInfo>,
    pub serialized_config_file_registry: Option<ConfigFileRegistry>,
}

pub fn new_state_baseline(fs_from_map: TestFs) -> StateBaseline {
    let mut state_baseline = StateBaseline {
        fs_differ: FsDiffer {
            fs: fs_from_map,
            default_libs: None,
            written_files: HashSet::new(),
            serialized_diff: None,
        },
        baseline: String::new(),
        is_initialized: false,
        serialized_projects: BTreeMap::new(),
        serialized_open_files: BTreeMap::new(),
        serialized_config_file_registry: None,
    };
    writeln!(
        state_baseline.baseline,
        "UseCaseSensitiveFileNames: {}",
        state_baseline.fs_differ.fs.use_case_sensitive_file_names()
    )
    .unwrap();
    baseline_fs_with_diff(&mut state_baseline.fs_differ, &mut state_baseline.baseline);
    state_baseline
}

impl MapFs for TestFs {
    fn entries(&self) -> BTreeMap<String, MapFsEntry> {
        let mut entries = BTreeMap::new();
        for (path, content) in &self.files {
            entries.insert(
                path.clone(),
                MapFsEntry {
                    data: content.as_bytes().to_vec(),
                    mtime: None::<SystemTime>,
                    is_regular: true,
                    symlink_target: None,
                },
            );
        }
        for (path, target) in &self.symlinks {
            entries.insert(
                path.clone(),
                MapFsEntry {
                    data: Vec::new(),
                    mtime: None::<SystemTime>,
                    is_regular: false,
                    symlink_target: Some(target.clone()),
                },
            );
        }
        entries
    }

    fn get_file_info(&self, path: &str) -> Option<MapFsEntry> {
        self.files
            .get(path)
            .map(|content| MapFsEntry {
                data: content.as_bytes().to_vec(),
                mtime: None::<SystemTime>,
                is_regular: true,
                symlink_target: None,
            })
            .or_else(|| {
                self.symlinks.get(path).map(|target| MapFsEntry {
                    data: Vec::new(),
                    mtime: None::<SystemTime>,
                    is_regular: false,
                    symlink_target: Some(target.clone()),
                })
            })
    }
}

fn baseline_fs_with_diff(fs_differ: &mut FsDiffer<TestFs>, w: &mut String) {
    let mut bytes = Vec::new();
    fs_differ.baseline_fs_with_diff(&mut bytes).unwrap();
    w.push_str(&String::from_utf8(bytes).unwrap());
}

#[derive(Serialize)]
pub struct RequestOrMessage<'a, Params> {
    pub method: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<&'a Params>,
}

impl FourslashTest {
    pub fn baseline_request_or_notification<Params>(
        &mut self,
        t: &mut TestingT,
        method: &str,
        params: &Params,
    ) where
        Params: Serialize,
    {
        self.baseline_request_or_notification_for_state(t, method, params);
    }

    pub fn baseline_request_or_notification_for_state<Params>(
        &mut self,
        _t: &mut TestingT,
        method: &str,
        params: &Params,
    ) where
        Params: Serialize,
    {
        if self.state_baseline.is_none() {
            return;
        }

        let state_baseline = self.state_baseline.as_mut().unwrap();
        state_baseline.baseline.push('\n');
        let json = serde_json::to_string_pretty(&RequestOrMessage {
            method,
            params: Some(params),
        })
        .unwrap();
        state_baseline.baseline.push_str(&json);
        state_baseline.baseline.push('\n');
        state_baseline.is_initialized = true;
    }

    pub fn baseline_projects_after_notification(&mut self, t: &mut TestingT, file_name: &str) {
        if self.state_baseline.is_none() {
            return;
        }
        let _ = file_name;
        self.baseline_state_for_statebaseline(t);
    }

    pub fn baseline_state(&mut self, t: &mut TestingT) {
        self.baseline_state_for_statebaseline(t);
    }

    pub fn baseline_state_for_statebaseline(&mut self, t: &mut TestingT) {
        if self.state_baseline.is_none() {
            return;
        }

        let serialized = self.serialized_state(t);
        if !serialized.is_empty() {
            let state_baseline = self.state_baseline.as_mut().unwrap();
            state_baseline.baseline.push('\n');
            state_baseline.baseline.push_str(&serialized);
        }
    }

    pub fn serialized_state(&mut self, t: &mut TestingT) -> String {
        let mut builder = String::new();
        if let Some(state_baseline) = &mut self.state_baseline {
            baseline_fs_with_diff(&mut state_baseline.fs_differ, &mut builder);
            if builder.trim().is_empty() {
                builder.clear();
            }
        }

        self.print_state_diff(t, &mut builder);
        builder
    }
}

#[derive(Clone, Default, Eq, PartialEq)]
pub struct ProjectInfo {
    pub program_id: Option<usize>,
    pub source_files: BTreeMap<tspath::Path, SourceFileInfo>,
}

#[derive(Clone, Default, Eq, PartialEq)]
pub struct SourceFileInfo {
    pub file_name: String,
    pub text: String,
}

#[derive(Clone, Default, Eq, PartialEq)]
pub struct OpenFileInfo {
    pub default_project_name: String,
    pub all_projects: Vec<String>,
}

#[derive(Clone, Default)]
pub struct DiffTableOptions {
    pub indent: String,
    pub sort_keys: bool,
}

pub struct DiffTable {
    pub diff: OrderedMap<String, String>,
    pub options: DiffTableOptions,
}

impl DiffTable {
    pub fn add(&mut self, key: String, value: String) {
        self.diff.set(key, value);
    }

    pub fn print(&self, w: &mut String, header: &str) {
        let count = self.diff.size();
        if count == 0 {
            return;
        }
        if !header.is_empty() {
            writeln!(w, "{}{}", self.options.indent, header).unwrap();
        }
        let mut diff_keys = self.diff.keys().cloned().collect::<Vec<_>>();
        let key_width = diff_keys
            .iter()
            .map(|key| key.len())
            .max()
            .unwrap_or_default();
        let indent = format!("{}  ", self.options.indent);
        if self.options.sort_keys {
            diff_keys.sort();
        }

        for key in diff_keys {
            let value = self.diff.get_or_zero(&key);
            writeln!(
                w,
                "{}{:<width$} {}",
                indent,
                key,
                value,
                width = key_width + 1
            )
            .unwrap();
        }
    }
}

pub struct DiffTableWriter {
    pub has_change: bool,
    pub header: String,
    pub diffs: BTreeMap<String, Box<dyn Fn(&mut String)>>,
}

pub fn new_diff_table_writer(header: &str) -> DiffTableWriter {
    DiffTableWriter {
        header: header.to_string(),
        diffs: BTreeMap::new(),
        has_change: false,
    }
}

impl DiffTableWriter {
    pub fn set_has_change(&mut self) {
        self.has_change = true;
    }

    pub fn add<F>(&mut self, key: String, f: F)
    where
        F: Fn(&mut String) + 'static,
    {
        self.diffs.insert(key, Box::new(f));
    }

    pub fn print(&self, w: &mut String) {
        if self.has_change {
            writeln!(w, "{}::", self.header).unwrap();
            for f in self.diffs.values() {
                f(w);
            }
        }
    }
}

pub fn are_iter_seq_equal(a: &[String], b: &[String]) -> bool {
    let mut a_slice = a.to_vec();
    let mut b_slice = b.to_vec();
    a_slice.sort();
    b_slice.sort();
    a_slice == b_slice
}

pub fn print_slices_with_diff_table(
    w: &mut String,
    header: &str,
    new_slice: &[String],
    get_old_slice: impl Fn() -> Vec<String>,
    options: DiffTableOptions,
    top_change: &str,
    is_default: impl Fn(&str) -> bool,
) {
    let old_slice = if top_change == "*modified*" {
        get_old_slice()
    } else {
        Vec::new()
    };
    let mut table = DiffTable {
        options,
        diff: OrderedMap::new(),
    };
    for entry in new_slice {
        let mut entry_change = String::new();
        if is_default(entry) {
            entry_change = "(default) ".to_string();
        }
        if top_change == "*modified*" && !old_slice.contains(entry) {
            entry_change = "*new*".to_string();
        }
        table.add(entry.clone(), entry_change);
    }
    if top_change == "*modified*" {
        for entry in old_slice {
            if !new_slice.contains(&entry) {
                table.add(entry, "*deleted*".to_string());
            }
        }
    }
    table.print(w, header);
}

pub fn slice_from_iter_seq_path(seq: &[String]) -> Vec<String> {
    let mut result = seq.to_vec();
    result.sort();
    result
}

pub fn print_path_iter_seq_with_diff_table(
    w: &mut String,
    header: &str,
    new_iter_seq: &[String],
    get_old_iter_seq: impl Fn() -> Vec<String>,
    options: DiffTableOptions,
    top_change: &str,
) {
    print_slices_with_diff_table(
        w,
        header,
        &slice_from_iter_seq_path(new_iter_seq),
        || slice_from_iter_seq_path(&get_old_iter_seq()),
        options,
        top_change,
        |_| false,
    );
}

impl FourslashTest {
    pub fn print_state_diff(&mut self, t: &mut TestingT, w: &mut String) {
        if self
            .state_baseline
            .as_ref()
            .is_none_or(|state_baseline| !state_baseline.is_initialized)
        {
            return;
        }

        let _ = (t, w);
    }

    pub fn print_projects_diff(
        &mut self,
        _t: &mut TestingT,
        session: &Session,
        snapshot: &SnapshotHandle,
        w: &mut String,
    ) {
        let mut current_projects = BTreeMap::new();
        let options = DiffTableOptions {
            indent: "  ".to_string(),
            sort_keys: false,
        };
        let mut projects_diff_table = new_diff_table_writer("Projects");

        for project in session.snapshot_projects(snapshot) {
            let program = project.get_program();
            let project_name = project.name();
            let project_info = project_info_from_program(program.as_deref());
            let old_project_info = self
                .state_baseline
                .as_ref()
                .unwrap()
                .serialized_projects
                .get(&project_name)
                .cloned();
            current_projects.insert(project_name.clone(), project_info.clone());
            let project_change = if let Some(old_project_info) = &old_project_info {
                if old_project_info.program_id != project_info.program_id {
                    projects_diff_table.set_has_change();
                    "*modified*"
                } else {
                    ""
                }
            } else {
                projects_diff_table.set_has_change();
                "*new*"
            };
            let options = options.clone();
            projects_diff_table.add(project_name.clone(), move |w| {
                writeln!(w, "  [{}] {}", project_name, project_change).unwrap();
                let mut sub_diff = DiffTable {
                    options: options.clone(),
                    diff: OrderedMap::new(),
                };
                for (path, file) in &project_info.source_files {
                    let mut file_diff = "";
                    if project_change == "*modified*" {
                        if let Some(old_project_info) = &old_project_info {
                            file_diff = match old_project_info.source_files.get(path) {
                                None => "*new*",
                                Some(old_file) if old_file.text != file.text => "*modified*",
                                Some(_) => "",
                            };
                        } else if !is_lib_file(&file.file_name) {
                            file_diff = "*new*";
                        }
                    }
                    if !file_diff.is_empty() || !is_lib_file(&file.file_name) {
                        sub_diff.add(file.file_name.clone(), file_diff.to_string());
                    }
                }
                if project_change == "*modified*" {
                    if let Some(old_project_info) = &old_project_info {
                        for (path, file) in &old_project_info.source_files {
                            if !project_info.source_files.contains_key(path) {
                                sub_diff.add(file.file_name.clone(), "*deleted*".to_string());
                            }
                        }
                    }
                }
                sub_diff.print(w, "");
            });
        }

        if let Some(state_baseline) = &mut self.state_baseline {
            for (project_name, info) in state_baseline.serialized_projects.clone() {
                if !current_projects.contains_key(&project_name) {
                    projects_diff_table.set_has_change();
                    let options = options.clone();
                    projects_diff_table.add(project_name.clone(), move |w| {
                        writeln!(w, "  [{}] *deleted*", project_name).unwrap();
                        let mut sub_diff = DiffTable {
                            options: options.clone(),
                            diff: OrderedMap::new(),
                        };
                        for file in info.source_files.values() {
                            if !is_lib_file(&file.file_name) {
                                sub_diff.add(file.file_name.clone(), String::new());
                            }
                        }
                        sub_diff.print(w, "");
                    });
                }
            }
            state_baseline.serialized_projects = current_projects;
        }
        projects_diff_table.print(w);
    }

    pub fn print_open_files_diff(
        &mut self,
        _t: &mut TestingT,
        session: &Session,
        snapshot: &SnapshotHandle,
        w: &mut String,
    ) {
        let mut current_open_files = BTreeMap::new();
        let mut files_diff_table = new_diff_table_writer("Open Files");
        let options = DiffTableOptions {
            indent: "  ".to_string(),
            sort_keys: true,
        };
        for file_name in &self.open_files {
            let path = tspath::to_path(file_name, "/", self.vfs.use_case_sensitive_file_names());
            let default_project = session.get_snapshot_default_project(snapshot, path.clone());
            let new_file_info = OpenFileInfo {
                default_project_name: default_project
                    .map(|project| project.name())
                    .unwrap_or_default(),
                all_projects: session
                    .snapshot_projects(snapshot)
                    .into_iter()
                    .filter(|project| {
                        project.get_program().is_some_and(|program| {
                            project_info_from_program(Some(program))
                                .source_files
                                .contains_key(&path)
                        })
                    })
                    .map(|project| project.name())
                    .collect(),
            };
            let mut new_file_info = new_file_info;
            new_file_info.all_projects.sort();
            current_open_files.insert(file_name.clone(), new_file_info.clone());
            let old_file_info = self
                .state_baseline
                .as_ref()
                .unwrap()
                .serialized_open_files
                .get(file_name)
                .cloned();
            let open_file_change = if let Some(old_file_info) = &old_file_info {
                if old_file_info.default_project_name != new_file_info.default_project_name
                    || old_file_info.all_projects != new_file_info.all_projects
                {
                    files_diff_table.set_has_change();
                    "*modified*"
                } else {
                    ""
                }
            } else {
                files_diff_table.set_has_change();
                "*new*"
            };
            let file_name_for_row = file_name.clone();
            let all_projects = new_file_info.all_projects.clone();
            let default_project_name = new_file_info.default_project_name.clone();
            let options = options.clone();
            let old_file_info_for_row = old_file_info.clone();
            files_diff_table.add(file_name.clone(), move |w| {
                writeln!(w, "  [{}] {}", file_name_for_row, open_file_change).unwrap();
                print_slices_with_diff_table(
                    w,
                    "",
                    &all_projects,
                    || {
                        old_file_info_for_row
                            .clone()
                            .map(|info| info.all_projects)
                            .unwrap_or_default()
                    },
                    options.clone(),
                    open_file_change,
                    |project_name| project_name == default_project_name,
                );
            });
        }
        if let Some(state_baseline) = &mut self.state_baseline {
            for file_name in state_baseline
                .serialized_open_files
                .keys()
                .cloned()
                .collect::<Vec<_>>()
            {
                if !current_open_files.contains_key(&file_name) {
                    files_diff_table.set_has_change();
                    files_diff_table.add(file_name.clone(), move |w| {
                        writeln!(w, "  [{}] *closed*", file_name).unwrap();
                    });
                }
            }
            state_baseline.serialized_open_files = current_open_files;
        }
        files_diff_table.print(w);
    }

    pub fn print_config_file_registry_diff(
        &mut self,
        _t: &mut TestingT,
        session: &Session,
        snapshot: &SnapshotHandle,
        w: &mut String,
    ) {
        let Some(config_file_registry) = session.snapshot_config_file_registry(snapshot) else {
            return;
        };
        let Some(state_baseline) = &mut self.state_baseline else {
            return;
        };

        if config_registry_ptr_eq(
            state_baseline.serialized_config_file_registry.as_ref(),
            Some(config_file_registry),
        ) {
            return;
        }

        let mut config_diffs_table = new_diff_table_writer("Config");
        let mut config_file_names_diffs_table = new_diff_table_writer("Config File Names");
        let options = DiffTableOptions {
            indent: "    ".to_string(),
            sort_keys: true,
        };
        let old_registry = state_baseline.serialized_config_file_registry.as_ref();

        config_file_registry.for_each_test_config_entry(|path, entry| {
            let old_entry =
                old_registry.and_then(|registry| registry.get_test_config_entry(path.clone()));
            let config_change = if let Some(old_entry) = &old_entry {
                if !are_iter_seq_equal(&old_entry.retaining_projects, &entry.retaining_projects)
                    || !are_iter_seq_equal(
                        &old_entry.retaining_open_files,
                        &entry.retaining_open_files,
                    )
                    || !are_iter_seq_equal(&old_entry.retaining_configs, &entry.retaining_configs)
                {
                    config_diffs_table.set_has_change();
                    "*modified*"
                } else {
                    ""
                }
            } else {
                config_diffs_table.set_has_change();
                "*new*"
            };
            let entry = TestConfigEntryInfo::from_entry(entry);
            let old_entry = old_entry.map(|entry| TestConfigEntryInfo::from_entry(&entry));
            let options = options.clone();
            config_diffs_table.add(path.clone(), move |w| {
                writeln!(w, "  [{}] {}", entry.file_name, config_change).unwrap();
                let mut retaining_projects_modified = "";
                let mut retaining_open_files_modified = "";
                let mut retaining_configs_modified = "";
                if config_change == "*modified*" {
                    if old_entry.as_ref().is_some_and(|old_entry| {
                        !are_iter_seq_equal(
                            &entry.retaining_projects,
                            &old_entry.retaining_projects,
                        )
                    }) {
                        retaining_projects_modified = " *modified*";
                    }
                    if old_entry.as_ref().is_some_and(|old_entry| {
                        !are_iter_seq_equal(
                            &entry.retaining_open_files,
                            &old_entry.retaining_open_files,
                        )
                    }) {
                        retaining_open_files_modified = " *modified*";
                    }
                    if old_entry.as_ref().is_some_and(|old_entry| {
                        !are_iter_seq_equal(&entry.retaining_configs, &old_entry.retaining_configs)
                    }) {
                        retaining_configs_modified = " *modified*";
                    }
                }
                print_path_iter_seq_with_diff_table(
                    w,
                    &format!("RetainingProjects:{retaining_projects_modified}"),
                    &entry.retaining_projects,
                    || {
                        old_entry
                            .clone()
                            .map(|entry| entry.retaining_projects)
                            .unwrap_or_default()
                    },
                    options.clone(),
                    config_change,
                );
                print_path_iter_seq_with_diff_table(
                    w,
                    &format!("RetainingOpenFiles:{retaining_open_files_modified}"),
                    &entry.retaining_open_files,
                    || {
                        old_entry
                            .clone()
                            .map(|entry| entry.retaining_open_files)
                            .unwrap_or_default()
                    },
                    options.clone(),
                    config_change,
                );
                print_path_iter_seq_with_diff_table(
                    w,
                    &format!("RetainingConfigs:{retaining_configs_modified}"),
                    &entry.retaining_configs,
                    || {
                        old_entry
                            .clone()
                            .map(|entry| entry.retaining_configs)
                            .unwrap_or_default()
                    },
                    options.clone(),
                    config_change,
                );
            });
        });

        config_file_registry.for_each_test_config_file_names_entry(|path, entry| {
            let old_entry = old_registry
                .and_then(|registry| registry.get_test_config_file_names_entry(path.clone()));
            let config_file_names_change = if let Some(old_entry) = &old_entry {
                if old_entry.nearest_config_file_name != entry.nearest_config_file_name
                    || old_entry.ancestors != entry.ancestors
                {
                    config_file_names_diffs_table.set_has_change();
                    "*modified*"
                } else {
                    ""
                }
            } else {
                config_file_names_diffs_table.set_has_change();
                "*new*"
            };
            let entry = TestConfigFileNamesEntryInfo::from_entry(entry);
            let old_entry = old_entry.map(|entry| TestConfigFileNamesEntryInfo::from_entry(&entry));
            let options = options.clone();
            config_file_names_diffs_table.add(path.clone(), move |w| {
                writeln!(w, "  [{}] {}", path, config_file_names_change).unwrap();
                let mut nearest_config_file_name_modified = "";
                let mut ancestor_diff_modified = "";
                if config_file_names_change == "*modified*" {
                    if old_entry.as_ref().is_some_and(|old_entry| {
                        old_entry.nearest_config_file_name != entry.nearest_config_file_name
                    }) {
                        nearest_config_file_name_modified = " *modified*";
                    }
                    if old_entry
                        .as_ref()
                        .is_some_and(|old_entry| old_entry.ancestors != entry.ancestors)
                    {
                        ancestor_diff_modified = " *modified*";
                    }
                }
                writeln!(
                    w,
                    "    NearestConfigFileName: {}{}",
                    entry.nearest_config_file_name, nearest_config_file_name_modified
                )
                .unwrap();
                let mut ancestor_diff = DiffTable {
                    options: options.clone(),
                    diff: OrderedMap::new(),
                };
                for (config, ancestor_of_config) in &entry.ancestors {
                    let mut ancestor_change = "";
                    if config_file_names_change == "*modified*" {
                        if let Some(old_config_file_name) = old_entry
                            .as_ref()
                            .and_then(|old_entry| old_entry.ancestors.get(config))
                        {
                            if old_config_file_name != ancestor_of_config {
                                ancestor_change = "*modified*";
                            }
                        } else {
                            ancestor_change = "*new*";
                        }
                    }
                    ancestor_diff.add(
                        config.clone(),
                        format!("{} {}", ancestor_of_config, ancestor_change),
                    );
                }
                if config_file_names_change == "*modified*" {
                    if let Some(old_entry) = &old_entry {
                        for (ancestor_path, old_config_file_name) in &old_entry.ancestors {
                            if !entry.ancestors.contains_key(ancestor_path) {
                                ancestor_diff.add(
                                    ancestor_path.clone(),
                                    format!("{} *deleted*", old_config_file_name),
                                );
                            }
                        }
                    }
                }
                ancestor_diff.print(w, &format!("Ancestors:{ancestor_diff_modified}"));
            });
        });

        if let Some(old_registry) = old_registry {
            old_registry.for_each_test_config_entry(|path, entry| {
                if config_file_registry
                    .get_test_config_entry(path.clone())
                    .is_none()
                {
                    config_diffs_table.set_has_change();
                    let file_name = entry.file_name.clone();
                    config_diffs_table.add(path, move |w| {
                        writeln!(w, "  [{}] *deleted*", file_name).unwrap();
                    });
                }
            });
            old_registry.for_each_test_config_file_names_entry(|path, _entry| {
                if config_file_registry
                    .get_test_config_file_names_entry(path.clone())
                    .is_none()
                {
                    config_file_names_diffs_table.set_has_change();
                    config_file_names_diffs_table.add(path.clone(), move |w| {
                        writeln!(w, "  [{}] *deleted*", path).unwrap();
                    });
                }
            });
        }

        state_baseline.serialized_config_file_registry =
            Some(config_file_registry.clone_registry());
        config_diffs_table.print(w);
        config_file_names_diffs_table.print(w);
    }
}

#[derive(Clone)]
struct TestConfigEntryInfo {
    file_name: String,
    retaining_projects: Vec<tspath::Path>,
    retaining_open_files: Vec<tspath::Path>,
    retaining_configs: Vec<tspath::Path>,
}

impl TestConfigEntryInfo {
    fn from_entry(entry: &ts_project::TestConfigEntry) -> Self {
        Self {
            file_name: entry.file_name.clone(),
            retaining_projects: entry.retaining_projects.clone(),
            retaining_open_files: entry.retaining_open_files.clone(),
            retaining_configs: entry.retaining_configs.clone(),
        }
    }
}

#[derive(Clone)]
struct TestConfigFileNamesEntryInfo {
    nearest_config_file_name: String,
    ancestors: HashMap<String, String>,
}

impl TestConfigFileNamesEntryInfo {
    fn from_entry(entry: &ts_project::TestConfigFileNamesEntry) -> Self {
        Self {
            nearest_config_file_name: entry.nearest_config_file_name.clone(),
            ancestors: entry.ancestors.clone(),
        }
    }
}

fn project_info_from_program(program: Option<&ts_compiler::Program>) -> ProjectInfo {
    let Some(program) = program else {
        return ProjectInfo::default();
    };
    let mut source_files = BTreeMap::new();
    for file in program.source_files_for_auto_imports() {
        source_files.insert(
            file.path(),
            SourceFileInfo {
                file_name: file.file_name(),
                text: file.text().to_string(),
            },
        );
    }
    ProjectInfo {
        program_id: Some(program as *const ts_compiler::Program as usize),
        source_files,
    }
}

fn config_registry_ptr_eq(a: Option<&ConfigFileRegistry>, b: Option<&ConfigFileRegistry>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => std::ptr::eq(a, b),
        (None, None) => true,
        _ => false,
    }
}
