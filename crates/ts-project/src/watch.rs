use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use lsp_types_full as lsproto;
use ts_collections as collections;
use ts_tspath as tspath;

pub const MIN_WATCH_LOCATION_DEPTH: usize = 2;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FileSystemWatcherKey {
    pub pattern: String,
    pub kind: lsproto::WatchKind,
}

#[derive(Clone, Debug)]
pub struct FileSystemWatcherValue {
    pub count: i32,
    pub id: WatcherId,
}

pub struct WatchRegistry {
    entries: Mutex<HashMap<FileSystemWatcherKey, FileSystemWatcherValue>>,
    pending: Mutex<HashSet<WatcherId>>,
}

pub fn new_watch_registry() -> WatchRegistry {
    WatchRegistry {
        entries: Mutex::new(HashMap::new()),
        pending: Mutex::new(HashSet::new()),
    }
}

impl WatchRegistry {
    pub fn acquire(&self, watcher: &lsproto::FileSystemWatcher, id: WatcherId) -> bool {
        let key = to_file_system_watcher_key(watcher);
        let mut entries = self.entries.lock().unwrap_or_else(|err| err.into_inner());
        let value = entries
            .entry(key)
            .or_insert(FileSystemWatcherValue { count: 0, id });
        value.count += 1;
        value.count == 1
    }

    pub fn release(&self, watcher: &lsproto::FileSystemWatcher) -> (WatcherId, bool) {
        let key = to_file_system_watcher_key(watcher);
        let mut entries = self.entries.lock().unwrap_or_else(|err| err.into_inner());
        let Some(value) = entries.get_mut(&key) else {
            return (WatcherId::default(), false);
        };
        if value.count <= 1 {
            let id = value.id.clone();
            entries.remove(&key);
            return (id, true);
        }
        value.count -= 1;
        (WatcherId::default(), false)
    }

    pub fn mark_pending(&self, id: WatcherId) {
        self.pending
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .insert(id);
    }

    pub fn clear_pending(&self, id: &WatcherId) {
        self.pending
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .remove(id);
    }

    pub fn is_pending(&self, id: &WatcherId) -> bool {
        self.pending
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .contains(id)
    }
}

#[derive(Clone, Debug, Default)]
pub struct PatternsAndIgnored {
    pub directories_outside_workspace: Vec<String>,
    pub patterns_inside_workspace: Vec<String>,
    pub ignored: HashSet<String>,
}

pub fn to_file_system_watcher_key(w: &lsproto::FileSystemWatcher) -> FileSystemWatcherKey {
    // PORT NOTE: mirrors Go's simple base/pattern string concatenation key.
    FileSystemWatcherKey {
        pattern: file_system_watcher_glob_string(w),
        kind: w.kind.unwrap_or(
            lsproto::WatchKind::Create | lsproto::WatchKind::Change | lsproto::WatchKind::Delete,
        ),
    }
}

pub fn file_system_watcher_glob_string(w: &lsproto::FileSystemWatcher) -> String {
    match &w.glob_pattern {
        lsproto::GlobPattern::String(pattern) => pattern.clone(),
        lsproto::GlobPattern::Relative(relative_pattern) => {
            let base = match &relative_pattern.base_uri {
                lsproto::OneOf::Left(_) => {
                    panic!("workspace folder-based relative patterns are unsupported")
                }
                lsproto::OneOf::Right(uri) => uri.clone(),
            };
            format!("{}/{}", base.as_str(), relative_pattern.pattern)
        }
    }
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct WatcherId(pub String);

impl WatcherId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WatcherId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for WatcherId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for WatcherId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

static WATCHER_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub struct WatchedFiles<T> {
    name: String,
    watch_kind: lsproto::WatchKind,
    has_relative_pattern_capability: bool,
    compute_glob_patterns: Arc<dyn Fn(T) -> PatternsAndIgnored + Send + Sync>,
    input: Option<T>,
    computed_watchers: bool,
    workspace_watchers: Vec<lsproto::FileSystemWatcher>,
    outside_workspace_watchers: Vec<lsproto::FileSystemWatcher>,
    ignored: HashSet<String>,
    id: u64,
}

pub fn new_watched_files<T>(
    name: String,
    watch_kind: lsproto::WatchKind,
    has_relative_pattern_capability: bool,
    compute_glob_patterns: impl Fn(T) -> PatternsAndIgnored + Send + Sync + 'static,
) -> WatchedFiles<T> {
    WatchedFiles {
        id: WATCHER_ID.fetch_add(1, Ordering::SeqCst) + 1,
        name,
        watch_kind,
        has_relative_pattern_capability,
        compute_glob_patterns: Arc::new(compute_glob_patterns),
        input: None,
        computed_watchers: false,
        workspace_watchers: Vec::new(),
        outside_workspace_watchers: Vec::new(),
        ignored: HashSet::new(),
    }
}

pub struct Watchers {
    pub watcher_id: WatcherId,
    pub workspace_watchers: Vec<lsproto::FileSystemWatcher>,
    pub outside_workspace_watchers: Vec<lsproto::FileSystemWatcher>,
    pub ignored_paths: HashSet<String>,
}

impl<T: Clone + Default> WatchedFiles<T> {
    pub fn watchers(&mut self) -> Watchers {
        if !self.computed_watchers {
            let input = self.input.clone().unwrap_or_default();
            let result = (self.compute_glob_patterns)(input);

            let mut globs = result.patterns_inside_workspace;
            globs.sort();
            globs.dedup();

            let mut changed = false;
            let existing_workspace_globs = self
                .workspace_watchers
                .iter()
                .map(file_system_watcher_glob_string)
                .collect::<Vec<_>>();
            if existing_workspace_globs != globs {
                self.workspace_watchers = globs
                    .into_iter()
                    .map(|glob| lsproto::FileSystemWatcher {
                        glob_pattern: lsproto::GlobPattern::String(glob),
                        kind: Some(self.watch_kind),
                    })
                    .collect();
                changed = true;
            }

            let mut dirs_outside = result.directories_outside_workspace;
            dirs_outside.sort();
            dirs_outside.dedup();
            let expected_outside_globs = dirs_outside
                .iter()
                .map(|dir| {
                    recursive_directory_glob_pattern(dir, self.has_relative_pattern_capability)
                })
                .collect::<Vec<_>>();
            let existing_outside_globs = self
                .outside_workspace_watchers
                .iter()
                .map(file_system_watcher_glob_string)
                .collect::<Vec<_>>();
            if existing_outside_globs != expected_outside_globs {
                self.outside_workspace_watchers = dirs_outside
                    .iter()
                    .map(|dir| {
                        new_recursive_directory_watcher(
                            dir,
                            self.watch_kind,
                            self.has_relative_pattern_capability,
                        )
                    })
                    .collect();
                changed = true;
            }

            self.ignored = result.ignored;
            if changed {
                self.id = WATCHER_ID.fetch_add(1, Ordering::SeqCst) + 1;
            }
            self.computed_watchers = true;
        }

        Watchers {
            watcher_id: WatcherId(format!("{} watcher {}", self.name, self.id)),
            workspace_watchers: self.workspace_watchers.clone(),
            outside_workspace_watchers: self.outside_workspace_watchers.clone(),
            ignored_paths: self.ignored.clone(),
        }
    }

    pub fn id(&mut self) -> WatcherId {
        self.watchers().watcher_id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn watch_kind(&self) -> lsproto::WatchKind {
        self.watch_kind
    }

    pub fn clone_with_input(&self, input: T) -> WatchedFiles<T> {
        WatchedFiles {
            name: self.name.clone(),
            watch_kind: self.watch_kind,
            has_relative_pattern_capability: self.has_relative_pattern_capability,
            compute_glob_patterns: Arc::clone(&self.compute_glob_patterns),
            workspace_watchers: self.workspace_watchers.clone(),
            outside_workspace_watchers: self.outside_workspace_watchers.clone(),
            ignored: self.ignored.clone(),
            input: Some(input),
            computed_watchers: false,
            id: 0,
        }
    }

    pub fn clone_with(&self, input: T) -> WatchedFiles<T> {
        self.clone_with_input(input)
    }
}

pub fn create_resolution_lookup_glob_mapper(
    workspace_directory: String,
    lib_directory: String,
    current_directory: String,
    use_case_sensitive_file_names: bool,
) -> impl Fn(collections::SyncSet<tspath::Path>) -> PatternsAndIgnored {
    let workspace_directory_path = tspath::to_path(
        &workspace_directory,
        &current_directory,
        use_case_sensitive_file_names,
    );
    let current_directory_path = tspath::to_path(
        &current_directory,
        &current_directory,
        use_case_sensitive_file_names,
    );
    let lib_directory_path = tspath::to_path(
        &lib_directory,
        &current_directory,
        use_case_sensitive_file_names,
    );

    move |data| {
        let mut seen_dirs = HashSet::<tspath::Path>::new();
        let mut include_workspace = false;
        let mut include_root = false;
        let mut include_lib = false;
        let mut node_modules_directories = HashSet::<tspath::Path>::new();
        let mut external_directories = HashSet::<tspath::Path>::new();

        data.range(|path| {
            if tspath::is_dynamic_file_name(path) {
                return true;
            }

            let directory = tspath::get_directory_path(path);
            if !seen_dirs.insert(directory.clone()) {
                return true;
            }

            if tspath::path_contains_path(&workspace_directory_path, path) {
                include_workspace = true;
            } else if tspath::path_contains_path(&current_directory_path, path) {
                include_root = true;
            } else if tspath::path_contains_path(&lib_directory_path, path) {
                include_lib = true;
            } else if let Some(index) = path.find("/node_modules/") {
                node_modules_directories.insert(path[..index + "/node_modules".len()].to_owned());
            } else {
                external_directories.insert(directory);
            }
            true
        });

        let mut globs = Vec::new();
        if include_workspace {
            globs.push(get_recursive_glob_pattern(&workspace_directory_path));
        }
        if include_root {
            globs.push(get_recursive_glob_pattern(&current_directory_path));
        }
        if include_lib {
            globs.push(get_recursive_glob_pattern(&lib_directory_path));
        }

        let mut node_modules_globs = node_modules_directories
            .iter()
            .map(|dir| get_recursive_glob_pattern(dir))
            .collect::<Vec<_>>();
        node_modules_globs.sort();
        globs.extend(node_modules_globs);

        let mut outside_dirs = Vec::new();
        let mut ignored = HashSet::new();
        if !external_directories.is_empty() {
            let mut external_dir_strings = external_directories.into_iter().collect::<Vec<_>>();
            external_dir_strings.sort();
            let (parents, ignored_external_dirs) = get_common_parents_for_watching(
                &external_dir_strings,
                MIN_WATCH_LOCATION_DEPTH,
                &tspath::ComparePathsOptions {
                    current_directory: String::new(),
                    use_case_sensitive_file_names: true,
                },
            );
            outside_dirs = parents;
            outside_dirs.sort();
            ignored = ignored_external_dirs.into_iter().collect();
        }

        PatternsAndIgnored {
            directories_outside_workspace: outside_dirs,
            patterns_inside_workspace: globs,
            ignored,
        }
    }
}

pub fn get_typings_locations_globs(
    typings_files: &[String],
    typings_location: &str,
    workspace_directory: &str,
    current_directory: &str,
    use_case_sensitive_file_names: bool,
) -> PatternsAndIgnored {
    let mut include_typings_location = false;
    let mut include_workspace = false;
    let mut external_directories = BTreeMap::<tspath::Path, String>::new();
    let mut globs = BTreeMap::<tspath::Path, String>::new();
    let compare_paths_options = tspath::ComparePathsOptions {
        current_directory: current_directory.to_owned(),
        use_case_sensitive_file_names,
    };

    for file in typings_files {
        if tspath::contains_path(typings_location, file, &compare_paths_options) {
            include_typings_location = true;
        } else if !tspath::contains_path(workspace_directory, file, &compare_paths_options) {
            let directory = tspath::get_directory_path(file);
            external_directories.insert(
                tspath::to_path(&directory, current_directory, use_case_sensitive_file_names),
                directory,
            );
        } else {
            include_workspace = true;
        }
    }

    let external_directory_values = external_directories.into_values().collect::<Vec<_>>();
    let (mut external_directory_parents, ignored) = get_common_parents_for_watching(
        &external_directory_values,
        MIN_WATCH_LOCATION_DEPTH,
        &compare_paths_options,
    );
    external_directory_parents.sort();

    if include_workspace {
        globs.insert(
            tspath::to_path(
                workspace_directory,
                current_directory,
                use_case_sensitive_file_names,
            ),
            get_recursive_glob_pattern(workspace_directory),
        );
    }
    if include_typings_location {
        globs.insert(
            tspath::to_path(
                typings_location,
                current_directory,
                use_case_sensitive_file_names,
            ),
            get_recursive_glob_pattern(typings_location),
        );
    }

    PatternsAndIgnored {
        directories_outside_workspace: external_directory_parents,
        patterns_inside_workspace: globs.into_values().collect(),
        ignored: ignored.into_iter().collect(),
    }
}

pub fn get_path_components_for_watching(path: &str, current_directory: &str) -> Vec<String> {
    let components = tspath::get_path_components(path, current_directory);
    let root_length = perceived_os_root_length_for_watching(&components);
    if root_length <= 1 {
        return components;
    }

    let mut result = Vec::with_capacity(components.len() - root_length + 1);
    let paths = components[1..root_length]
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    result.push(tspath::combine_paths(&components[0], &paths));
    result.extend(components[root_length..].iter().cloned());
    result
}

pub fn perceived_os_root_length_for_watching(path_components: &[String]) -> usize {
    let length = path_components.len();
    if length <= 1 {
        return length;
    }
    if path_components[0].starts_with("//") {
        return 2;
    }
    if path_components[0].len() == 3
        && tspath::is_volume_character(path_components[0].as_bytes()[0])
        && path_components[0].as_bytes()[1] == b':'
        && path_components[0].as_bytes()[2] == b'/'
    {
        if path_components[1].eq_ignore_ascii_case("users") {
            return length.min(3);
        }
        return 1;
    }
    if path_components[1] == "home" {
        return length.min(3);
    }
    1
}

fn get_common_parents_for_watching(
    paths: &[String],
    min_components: usize,
    options: &tspath::ComparePathsOptions,
) -> (Vec<String>, BTreeSet<String>) {
    assert!(min_components >= 1, "minComponents must be at least 1");
    if paths.is_empty() {
        return (Vec::new(), BTreeSet::new());
    }
    if paths.len() == 1 {
        let components = tspath::reduce_path_components(&get_path_components_for_watching(
            &paths[0],
            &options.current_directory,
        ));
        if components.len() < min_components {
            return (Vec::new(), BTreeSet::from([paths[0].clone()]));
        }
        return (paths.to_vec(), BTreeSet::new());
    }

    let mut ignored = BTreeSet::new();
    let mut path_components = Vec::with_capacity(paths.len());
    for path in paths {
        let components = tspath::reduce_path_components(&get_path_components_for_watching(
            path,
            &options.current_directory,
        ));
        if components.len() < min_components {
            ignored.insert(path.clone());
        } else {
            path_components.push(components);
        }
    }

    let parents = get_common_parents_for_watching_worker(&path_components, min_components, options)
        .into_iter()
        .map(|components| tspath::get_path_from_path_components(&components))
        .collect();
    (parents, ignored)
}

fn get_common_parents_for_watching_worker(
    component_groups: &[Vec<String>],
    min_components: usize,
    options: &tspath::ComparePathsOptions,
) -> Vec<Vec<String>> {
    if component_groups.is_empty() {
        return Vec::new();
    }

    let max_depth = component_groups
        .iter()
        .map(Vec::len)
        .min()
        .unwrap_or_default();
    for last_common_index in 0..max_depth {
        let candidate = &component_groups[0][last_common_index];
        for components in component_groups.iter().skip(1) {
            if options.equal_strings(candidate, &components[last_common_index]) {
                continue;
            }
            if last_common_index < min_components {
                let mut grouped = BTreeMap::<tspath::Path, (Vec<String>, Vec<Vec<String>>)>::new();
                for group in component_groups {
                    let key = tspath::to_path(
                        &group[last_common_index],
                        &options.current_directory,
                        options.use_case_sensitive_file_names,
                    );
                    let entry = grouped
                        .entry(key)
                        .or_insert_with(|| (group[..=last_common_index].to_vec(), Vec::new()));
                    entry.1.push(group[last_common_index + 1..].to_vec());
                }

                let mut result = Vec::new();
                for (_, (head, tails)) in grouped {
                    let sub_results = get_common_parents_for_watching_worker(
                        &tails,
                        min_components - (last_common_index + 1),
                        options,
                    );
                    for mut sub_result in sub_results {
                        let mut parent = head.clone();
                        parent.append(&mut sub_result);
                        result.push(parent);
                    }
                }
                return result;
            }
            return vec![component_groups[0][..last_common_index].to_vec()];
        }
    }

    vec![component_groups[0][..max_depth].to_vec()]
}

pub fn get_recursive_glob_pattern(directory: &str) -> String {
    format!(
        "{}/**/*",
        tspath::remove_trailing_directory_separator(directory)
    )
}

pub fn recursive_directory_glob_pattern(directory: &str, use_relative_pattern: bool) -> String {
    if use_relative_pattern {
        return format!("{}/**/*", file_name_to_document_uri(directory).as_str());
    }
    get_recursive_glob_pattern(directory)
}

pub fn new_recursive_directory_watcher(
    directory: &str,
    kind: lsproto::WatchKind,
    use_relative_pattern: bool,
) -> lsproto::FileSystemWatcher {
    if use_relative_pattern {
        return lsproto::FileSystemWatcher {
            glob_pattern: lsproto::GlobPattern::Relative(lsproto::RelativePattern {
                base_uri: lsproto::OneOf::Right(file_name_to_document_uri(directory)),
                pattern: "**/*".to_owned(),
            }),
            kind: Some(kind),
        };
    }

    lsproto::FileSystemWatcher {
        glob_pattern: lsproto::GlobPattern::String(get_recursive_glob_pattern(directory)),
        kind: Some(kind),
    }
}

fn file_name_to_document_uri(file_name: &str) -> lsproto::Uri {
    let normalized = file_name.replace('\\', "/");
    let uri = if normalized.starts_with("file://") {
        normalized
    } else if normalized.starts_with('/') {
        format!("file://{normalized}")
    } else {
        format!("file:///{normalized}")
    };
    lsproto::Uri::from_str(&uri).expect("generated file URI should parse")
}
