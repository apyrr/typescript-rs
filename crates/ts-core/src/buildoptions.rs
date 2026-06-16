use serde::{Deserialize, Serialize};

use crate::Tristate;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildOptions {
    #[serde(default, skip_serializing_if = "Tristate::is_unknown")]
    pub dry: Tristate,
    #[serde(default, skip_serializing_if = "Tristate::is_unknown")]
    pub force: Tristate,
    #[serde(default, skip_serializing_if = "Tristate::is_unknown")]
    pub verbose: Tristate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builders: Option<i32>,
    #[serde(
        default,
        rename = "stopBuildOnErrors",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub stop_build_on_errors: Tristate,

    // CompilerOptions are not parsed here and will be available on ParsedBuildCommandLine

    // Internal fields
    #[serde(default, skip_serializing_if = "Tristate::is_unknown")]
    pub clean: Tristate,
}
