use serde::{Deserialize, Serialize};
use ts_tspath as tspath;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectReference {
    #[serde(rename = "Path")]
    pub path: String,
    #[serde(rename = "OriginalPath")]
    pub original_path: String,
    #[serde(rename = "Circular")]
    pub circular: bool,
}

pub fn resolve_project_reference_path(ref_: &ProjectReference) -> String {
    resolve_config_file_name_of_project_reference(&ref_.path)
}

pub fn resolve_config_file_name_of_project_reference(path: &str) -> String {
    if tspath::file_extension_is(path, tspath::EXTENSION_JSON) {
        return path.to_string();
    }
    tspath::combine_paths(path, &["tsconfig.json"])
}
