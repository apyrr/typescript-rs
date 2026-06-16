use serde::{Deserialize, Deserializer, Serialize};

use crate::{CompilerOptions, ProjectReference, TypeAcquisition};

pub use crate::watchoptions::WatchOptions;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ParsedOptions {
    #[serde(rename = "compilerOptions")]
    pub compiler_options: Option<CompilerOptions>,
    #[serde(rename = "watchOptions")]
    pub watch_options: Option<WatchOptions>,
    #[serde(rename = "typeAcquisition")]
    pub type_acquisition: Option<TypeAcquisition>,

    #[serde(rename = "fileNames")]
    #[serde(deserialize_with = "deserialize_file_names")]
    pub file_names: Option<Vec<String>>,
    #[serde(rename = "projectReferences")]
    pub project_references: Option<Vec<Option<ProjectReference>>>,
}

fn deserialize_file_names<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let file_names = Option::<Vec<Option<String>>>::deserialize(deserializer)?;
    Ok(file_names.map(|file_names| {
        file_names
            .into_iter()
            .map(|file_name| file_name.unwrap_or_default())
            .collect()
    }))
}
