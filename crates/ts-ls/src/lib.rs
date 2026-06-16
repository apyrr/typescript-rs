#![forbid(unsafe_code)]

mod lsconv;
#[expect(
    dead_code,
    reason = "ported LS utility surface is ahead of current callers"
)]
mod lsutil;

#[expect(
    dead_code,
    reason = "ported LS API helpers are ahead of current callers"
)]
mod api;
#[expect(
    dead_code,
    reason = "ported auto-import service is ahead of current callers"
)]
mod autoimport;
mod autoinsert;
#[expect(
    dead_code,
    reason = "ported call hierarchy service is ahead of current callers"
)]
mod callhierarchy;
mod change;
mod codeactions;
mod codeactions_fixclassincorrectlyimplementsinterface;
#[expect(
    dead_code,
    reason = "ported code action helper is ahead of current callers"
)]
mod codeactions_fixmissingtypeannotation;
#[expect(
    dead_code,
    reason = "ported import fix helper is ahead of current callers"
)]
mod codeactions_importfixes;
#[expect(
    dead_code,
    reason = "ported missing-member fixer is ahead of current callers"
)]
mod codeactions_missingmemberfixer;
mod codelens;
#[expect(
    dead_code,
    reason = "ported completion service is ahead of current callers"
)]
mod completions;
#[expect(dead_code, reason = "ported LS constants are ahead of current callers")]
mod constants;
mod crossproject;
mod definition;
mod diagnostics;
mod documenthighlights;
mod file_rename;
#[expect(
    dead_code,
    reason = "ported find-all-references service is ahead of current callers"
)]
mod findallreferences;
#[expect(
    unused_assignments,
    reason = "ported folding traversal shape follows upstream control flow"
)]
mod folding;
mod format;
#[cfg(test)]
mod format_test;
mod host;
#[expect(dead_code, reason = "ported hover service is ahead of current callers")]
mod hover;
#[expect(
    dead_code,
    non_snake_case,
    reason = "module name mirrors upstream TypeScript-Go importTracker"
)]
mod importTracker;
#[expect(
    dead_code,
    reason = "ported inlay-hints service is ahead of current callers"
)]
mod inlay_hints;
mod languageservice;
mod linkedediting;
mod organizeimports;
mod rename;
mod selectionranges;
#[expect(
    dead_code,
    private_interfaces,
    reason = "ported semantic token service is ahead of current callers"
)]
mod semantictokens;
#[expect(
    dead_code,
    reason = "ported signature-help service is ahead of current callers"
)]
mod signaturehelp;
mod source_map;
mod sourcedefinition;
#[expect(
    dead_code,
    reason = "ported string-completion service is ahead of current callers"
)]
mod string_completions;
mod symbols;
#[expect(dead_code, reason = "ported LS helpers are ahead of current callers")]
mod utilities;

pub use crossproject::{CrossProjectOrchestrator, Project};
pub use host::Host;
pub use languageservice::{LanguageService, new_language_service};
pub use lsconv::{
    Converters, LspLineMap, LspLineStarts, Script, compute_lsp_line_starts, diagnostic_to_lsp_pull,
    diagnostic_to_lsp_push, file_name_to_document_uri, language_kind_to_script_kind,
    new_converters,
};
pub use lsutil::{
    CodeLensUserPreferences, EditorSettings, FormatCodeSettings, FormatSettingValue,
    IncludeInlayParameterNameHints, IndentStyle, InlayHintsPreferences,
    JsxAttributeCompletionStyle, OrganizeImportsCaseFirst, OrganizeImportsCollation,
    OrganizeImportsTypeOrder, QuotePreference, SemicolonPreference, UserPreferences,
    from_ls_format_options, get_default_format_code_settings, new_default_user_preferences,
    parse_indent_style, parse_semicolon_preference, parse_user_preferences,
};
pub use rename::{client_supports_document_changes, client_supports_will_rename_files};
pub use symbols::provide_workspace_symbols;

pub use api::{ApiError, LanguageServiceSymbolHandle, LanguageServiceTypeHandle};
pub use autoimport::{
    BucketStats as AutoImportBucketStats, CacheStats as AutoImportCacheStats,
    Registry as AutoImportRegistry, RegistryChange as AutoImportRegistryChange, RegistryCloneHost,
    new_registry as new_auto_import_registry,
};
pub use completions::ERR_NEEDS_AUTO_IMPORTS;
