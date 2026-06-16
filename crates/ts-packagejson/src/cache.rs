use std::fmt;
use std::sync::{LazyLock, Mutex, OnceLock};

use crate::{Fields, JsonValueType};
use ts_collections::{OrderedMap, SyncMap};
use ts_diagnostics as diagnostics;
use ts_tspath as tspath;

static TYPESCRIPT_VERSION: LazyLock<ts_semver::Version> =
    LazyLock::new(|| ts_semver::must_parse(ts_core::version()));

#[derive(Clone)]
pub struct DiagnosticAndArgs {
    pub message: &'static diagnostics::Message,
    pub args: Vec<String>,
}

pub struct PackageJson {
    pub fields: Fields,
    pub parseable: bool,
    version_paths: OnceLock<VersionPaths>,
    version_traces: Mutex<Vec<DiagnosticAndArgs>>,
}

impl Clone for PackageJson {
    fn clone(&self) -> Self {
        let version_paths = OnceLock::new();
        if let Some(paths) = self.version_paths.get() {
            let _ = version_paths.set(paths.clone());
        }
        Self {
            fields: self.fields.clone(),
            parseable: self.parseable,
            version_paths,
            version_traces: Mutex::new(
                self.version_traces
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .clone(),
            ),
        }
    }
}

impl Default for PackageJson {
    fn default() -> Self {
        Self {
            fields: Fields::default(),
            parseable: false,
            version_paths: OnceLock::new(),
            version_traces: Mutex::new(Vec::new()),
        }
    }
}

impl PackageJson {
    pub fn new(fields: Fields, parseable: bool) -> Self {
        Self {
            fields,
            parseable,
            ..Default::default()
        }
    }

    pub fn get_version_paths(
        &self,
        mut trace: Option<impl FnMut(&'static diagnostics::Message, &[String])>,
    ) -> VersionPaths {
        let version_paths = self
            .version_paths
            .get_or_init(|| self.compute_version_paths())
            .clone();
        if let Some(trace) = trace.as_mut() {
            for diagnostic in self
                .version_traces
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .iter()
            {
                trace(diagnostic.message, &diagnostic.args);
            }
        }
        version_paths
    }

    fn push_trace(&self, message: &'static diagnostics::Message, args: Vec<String>) {
        self.version_traces
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push(DiagnosticAndArgs { message, args });
    }

    fn compute_version_paths(&self) -> VersionPaths {
        let types_versions = &self.fields.path_fields.types_versions;
        if types_versions.type_ == JsonValueType::NotPresent {
            self.push_trace(
                &diagnostics::X_package_json_does_not_have_a_0_field,
                vec!["typesVersions".to_owned()],
            );
            return VersionPaths::default();
        }
        if types_versions.type_ != JsonValueType::Object {
            self.push_trace(
                &diagnostics::Expected_type_of_0_field_in_package_json_to_be_1_got_2,
                vec![
                    "typesVersions".to_owned(),
                    "object".to_owned(),
                    types_versions.type_.to_string(),
                ],
            );
            return VersionPaths::default();
        }

        self.push_trace(
            &diagnostics::X_package_json_has_a_typesVersions_field_with_version_specific_path_mappings,
            vec!["typesVersions".to_owned()],
        );

        for (key, value) in types_versions.as_object() {
            let (key_range, ok) = ts_semver::try_parse_version_range(key);
            if !ok {
                self.push_trace(
                    &diagnostics::X_package_json_has_a_typesVersions_entry_0_that_is_not_a_valid_semver_range,
                    vec![key.clone()],
                );
                continue;
            }
            if !key_range.test(&TYPESCRIPT_VERSION) {
                continue;
            }
            if !value.is_object() {
                self.push_trace(
                    &diagnostics::Expected_type_of_0_field_in_package_json_to_be_1_got_2,
                    vec![
                        format!("typesVersions['{key}']"),
                        "object".to_owned(),
                        json_type_name(value).to_owned(),
                    ],
                );
                return VersionPaths::default();
            }
            return VersionPaths {
                version: key.clone(),
                paths_json: Some(ordered_map_from_json_object(value.as_object().unwrap())),
                paths: OnceLock::new(),
            };
        }

        self.push_trace(
            &diagnostics::X_package_json_does_not_have_a_typesVersions_entry_that_matches_version_0,
            vec![ts_core::version_major_minor()],
        );
        VersionPaths::default()
    }
}

#[derive(Clone, Default)]
pub struct VersionPaths {
    pub version: String,
    paths_json: Option<OrderedMap<String, serde_json::Value>>,
    paths: OnceLock<OrderedMap<String, Vec<String>>>,
}

impl VersionPaths {
    pub fn exists(&self) -> bool {
        !self.version.is_empty() && self.paths_json.is_some()
    }

    pub fn get_paths(&self) -> Option<&OrderedMap<String, Vec<String>>> {
        if !self.exists() {
            return None;
        }
        Some(self.paths.get_or_init(|| {
            let paths_json = self.paths_json.as_ref().unwrap();
            let mut paths = OrderedMap::with_size_hint(paths_json.size());
            for (key, value) in paths_json.entries() {
                let Some(array) = value.as_array() else {
                    continue;
                };
                let mut slice = vec![String::new(); array.len()];
                for (i, path) in array.iter().enumerate() {
                    if let Some(path) = path.as_str() {
                        slice[i] = path.to_owned();
                    }
                }
                paths.set(key.clone(), slice);
            }
            paths
        }))
    }
}

#[derive(Clone, Default)]
pub struct InfoCacheEntry {
    pub package_directory: String,
    pub directory_exists: bool,
    pub contents: Option<PackageJson>,
}

impl fmt::Debug for InfoCacheEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InfoCacheEntry")
            .field("package_directory", &self.package_directory)
            .field("directory_exists", &self.directory_exists)
            .field("contents", &self.contents.as_ref().map(|_| "PackageJson"))
            .finish()
    }
}

impl InfoCacheEntry {
    pub fn exists(&self) -> bool {
        self.contents.is_some()
    }

    pub fn get_contents(&self) -> Option<&PackageJson> {
        self.contents.as_ref()
    }

    pub fn get_directory(&self) -> &str {
        &self.package_directory
    }
}

#[derive(Default)]
pub struct InfoCache {
    cache: SyncMap<tspath::Path, InfoCacheEntry>,
    current_directory: String,
    use_case_sensitive_file_names: bool,
}

impl InfoCache {
    pub fn new(current_directory: String, use_case_sensitive_file_names: bool) -> Self {
        Self {
            current_directory,
            use_case_sensitive_file_names,
            ..Default::default()
        }
    }

    pub fn get(&self, package_json_path: &str) -> Option<InfoCacheEntry>
    where
        InfoCacheEntry: Clone,
    {
        let key = tspath::to_path(
            package_json_path,
            &self.current_directory,
            self.use_case_sensitive_file_names,
        );
        let (value, ok) = self.cache.load(&key);
        if ok { value } else { None }
    }

    pub fn set(&self, package_json_path: &str, info: InfoCacheEntry) -> InfoCacheEntry
    where
        InfoCacheEntry: Clone,
    {
        let key = tspath::to_path(
            package_json_path,
            &self.current_directory,
            self.use_case_sensitive_file_names,
        );
        let (actual, _) = self.cache.load_or_store(key, Some(info));
        actual.unwrap_or_default()
    }
}

fn ordered_map_from_json_object(
    object: &serde_json::Map<String, serde_json::Value>,
) -> OrderedMap<String, serde_json::Value> {
    let mut result = OrderedMap::with_size_hint(object.len());
    for (key, value) in object {
        result.set(key.clone(), value.clone());
    }
    result
}

fn json_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}
