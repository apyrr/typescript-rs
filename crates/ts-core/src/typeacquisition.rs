use serde::{Deserialize, Serialize};

use crate::Tristate;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeAcquisition {
    #[serde(default, skip_serializing_if = "Tristate::is_unknown")]
    pub enable: Tristate,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
    #[serde(
        rename = "disableFilenameBasedTypeAcquisition",
        default,
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub disable_filename_based_type_acquisition: Tristate,
}

impl TypeAcquisition {
    pub fn equals(&self, other: &TypeAcquisition) -> bool {
        self == other
    }
}

pub fn type_acquisition_equals(
    ta: Option<&TypeAcquisition>,
    other: Option<&TypeAcquisition>,
) -> bool {
    match (ta, other) {
        (Some(ta), Some(other)) => std::ptr::eq(ta, other) || ta.equals(other),
        (None, None) => true,
        _ => false,
    }
}
