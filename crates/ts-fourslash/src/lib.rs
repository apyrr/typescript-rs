#![forbid(unsafe_code)]

pub mod baselineutil;
pub mod fourslash;
pub mod semantictokens;
mod skip_if_failing;
pub mod statebaseline;
pub mod test_parser;

#[expect(
    ambiguous_glob_reexports,
    reason = "fourslash harness preserves upstream-style glob re-exports"
)]
pub use baselineutil::*;
pub use fourslash::*;
pub use semantictokens::*;
pub use skip_if_failing::*;
pub use statebaseline::*;
pub use test_parser::*;

#[cfg(all(test, feature = "generated-fourslash"))]
pub mod generated_prelude {
    pub use crate::tests::util::*;
    pub use crate::{
        ApplyCodeActionFromCompletionOptions, AutoImportFix, CompletionsExpectedCodeAction,
        CompletionsExpectedItem, CompletionsExpectedItemDefaults, CompletionsExpectedItems,
        CompletionsExpectedList, ExpectedCompletionEditRange, FourslashDiagnostic,
        InlayHintsPreferences, MarkerInput, MarkerOrRangeOrName, SemanticToken,
        SignatureHelpContext, TestingT, UserPreferences, VerifyCodeFixAllOptions,
        VerifyCodeFixOptions, VerifySignatureHelpOptions, new_fourslash, range_marker_data,
        skip_if_failing, symbol_information, workspace_symbol_case,
        workspace_symbol_case_from_range_with_pattern, workspace_symbol_case_with_preferences,
    };
}

pub mod tests {
    #[path = "util/util.rs"]
    pub mod util;

    #[cfg(all(test, feature = "generated-fourslash"))]
    pub mod generated;
}
