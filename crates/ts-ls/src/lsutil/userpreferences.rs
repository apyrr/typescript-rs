use std::collections::HashMap;
use std::io::{Read, Write};

use ts_core as core;
use ts_json as json;
use ts_modulespecifiers as modulespecifiers;
use ts_vfs::vfsmatch;

use super::{
    FormatCodeSettings, FormatSettingValue, IndentStyle, SemicolonPreference,
    get_default_format_code_settings, parse_indent_style, parse_semicolon_preference,
};

pub fn new_default_user_preferences() -> UserPreferences {
    UserPreferences {
        format_code_settings: get_default_format_code_settings(),
        include_completions_for_module_exports: core::Tristate::True,
        include_completions_for_import_statements: core::Tristate::True,
        allow_rename_of_import_path: core::Tristate::True,
        provide_refactor_not_applicable_reason: core::Tristate::True,
        disable_line_text_in_references: core::Tristate::True,
        report_style_checks_as_warnings: core::Tristate::True,
        exclude_library_symbols_in_nav_to: core::Tristate::True,
        import_module_specifier_ending:
            modulespecifiers::ImportModuleSpecifierEndingPreference::None,
        ..Default::default()
    }
}

// UserPreferences represents TypeScript language service preferences.
//
// Fields are populated using two tags:
//   - `raw:"name"` or `raw:"name,invert"` - TypeScript/raw name for unstable section lookup
//   - `config:"path.to.setting"` or `config:"path.to.setting,invert"` - VS Code nested config path
//
// At least one tag must be present on each preference field.
// The `,invert` modifier inverts boolean values (e.g., VS Code's "suppress" -> our "include").
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPreferences {
    pub format_code_settings: FormatCodeSettings,

    pub quote_preference: QuotePreference,
    pub lazy_configured_projects_from_external_project: core::Tristate,

    // A positive integer indicating the maximum length of a hover text before it is truncated.
    //
    // Default: `500`
    pub maximum_hover_length: i32,

    // ------- Completions -------
    // If enabled, TypeScript will search through all external modules' exports and add them to the completions list.
    // This affects lone identifier completions but not completions on the right hand side of `obj.`.
    pub include_completions_for_module_exports: core::Tristate,
    // Enables auto-import-style completions on partially-typed import statements. E.g., allows
    // `import write|` to be completed to `import { writeFile } from "fs"`.
    pub include_completions_for_import_statements: core::Tristate,
    // Unless this option is `false`,  member completion lists triggered with `.` will include entries
    // on potentially-null and potentially-undefined values, with insertion text to replace
    // preceding `.` tokens with `?.`.
    pub include_automatic_optional_chain_completions: core::Tristate,
    // If enabled, completions for class members (e.g. methods and properties) will include
    // a whole declaration for the member.
    // E.g., `class A { f| }` could be completed to `class A { foo(): number {} }`, instead of
    // `class A { foo }`.
    pub include_completions_with_class_member_snippets: core::Tristate,
    // If enabled, object literal methods will have a method declaration completion entry in addition
    // to the regular completion entry containing just the method name.
    // E.g., `const objectLiteral: T = { f| }` could be completed to `const objectLiteral: T = { foo(): void {} }`,
    // in addition to `const objectLiteral: T = { foo }`.
    pub include_completions_with_object_literal_method_snippets: core::Tristate,
    pub jsx_attribute_completion_style: JsxAttributeCompletionStyle,

    // ------- AutoImports --------
    pub import_module_specifier_preference: modulespecifiers::ImportModuleSpecifierPreference,
    // Determines whether we import `foo/index.ts` as "foo", "foo/index", or "foo/index.js"
    pub import_module_specifier_ending: modulespecifiers::ImportModuleSpecifierEndingPreference,
    pub auto_import_specifier_exclude_regexes: Vec<String>,
    pub auto_import_file_exclude_patterns: Vec<String>,
    pub auto_import_entrypoint_directory_search: core::Tristate,
    pub prefer_type_only_auto_imports: core::Tristate,

    // ------- OrganizeImports -------
    // Indicates whether imports should be organized in a case-insensitive manner.
    //
    // Default: TSUnknown ("auto" in strada), will perform detection
    pub organize_imports_ignore_case: core::Tristate,
    // Indicates whether imports should be organized via an "ordinal" (binary) comparison using the numeric value of their
    // code points, or via "unicode" collation (via the Unicode Collation Algorithm (https://unicode.org/reports/tr10/#Scope))
    //
    // using rules associated with the locale specified in organizeImportsCollationLocale.
    //
    // Default: Ordinal
    pub organize_imports_collation: OrganizeImportsCollation,
    // Indicates the locale to use for "unicode" collation. If not specified, the locale `"en"` is used as an invariant
    // for the sake of consistent sorting. Use `"auto"` to use the detected UI locale.
    //
    // This preference is ignored if organizeImportsCollation is not `unicode`.
    //
    // Default: `"en"`
    pub organize_imports_locale: String,
    // Indicates whether numeric collation should be used for digit sequences in strings. When `true`, will collate
    // strings such that `a1z < a2z < a100z`. When `false`, will collate strings such that `a1z < a100z < a2z`.
    //
    // This preference is ignored if organizeImportsCollation is not `unicode`.
    //
    // Default: `false`
    pub organize_imports_numeric_collation: core::Tristate,
    // Indicates whether accents and other diacritic marks are considered unequal for the purpose of collation. When
    // `true`, characters with accents and other diacritics will be collated in the order defined by the locale specified
    // in organizeImportsCollationLocale.
    //
    // This preference is ignored if organizeImportsCollation is not `unicode`.
    //
    // Default: `true`
    pub organize_imports_accent_collation: core::Tristate,
    // Indicates whether upper case or lower case should sort first. When `false`, the default order for the locale
    // specified in organizeImportsCollationLocale is used.
    //
    // This permission is ignored if:
    //	- organizeImportsCollation is not `unicode`
    //	- organizeImportsIgnoreCase is `true`
    //	- organizeImportsIgnoreCase is `auto` and the auto-detected case sensitivity is case-insensitive.
    //
    // Default: `false`
    pub organize_imports_case_first: OrganizeImportsCaseFirst,
    // Indicates where named type-only imports should sort. "inline" sorts named imports without regard to if the import is type-only.
    //
    // Default: `auto`, which defaults to `last`
    pub organize_imports_type_order: OrganizeImportsTypeOrder,

    // ------- MoveToFile -------
    pub allow_text_changes_in_new_files: core::Tristate,

    // ------- Rename -------
    pub use_aliases_for_rename: core::Tristate,
    pub allow_rename_of_import_path: core::Tristate,

    // ------- CodeFixes/Refactors -------
    pub provide_refactor_not_applicable_reason: core::Tristate,

    // ------- InlayHints -------
    pub inlay_hints: InlayHintsPreferences,

    // ------- CodeLens -------
    pub code_lens: CodeLensUserPreferences,

    // ------- Definition -------
    pub prefer_go_to_source_definition: bool,

    // ------- Symbols -------
    pub exclude_library_symbols_in_nav_to: core::Tristate,

    // ------- Misc -------
    pub disable_suggestions: core::Tristate,
    pub disable_line_text_in_references: core::Tristate,
    pub report_style_checks_as_warnings: core::Tristate,

    // ------- ATA -------
    // DisableAutomaticTypeAcquisition is the deprecated setting from typescript.disableAutomaticTypeAcquisition.
    pub disable_automatic_type_acquisition: core::Tristate,
    // AutomaticTypeAcquisitionEnabled is the unified setting from tsserver.automaticTypeAcquisition.enabled under the js/ts section.
    // When set, it takes precedence over DisableAutomaticTypeAcquisition.
    pub automatic_type_acquisition_enabled: core::Tristate,
    // Go source notes tsserver.web.typeAcquisition.enabled belongs here when web support exists.

    // ------- Project Configuration -------
    // CustomConfigFileName specifies a custom config file name to use before defaulting to tsconfig.json/jsconfig.json.
    pub custom_config_file_name: String,
}

// IsATADisabled returns whether Automatic Type Acquisition is disabled based on user preferences.
// It checks the unified setting (tsserver.automaticTypeAcquisition.enabled) first,
// then falls back to the deprecated setting (disableAutomaticTypeAcquisition).
impl UserPreferences {
    pub fn is_ata_disabled(&self) -> bool {
        if !self.automatic_type_acquisition_enabled.is_unknown() {
            return !self.automatic_type_acquisition_enabled.is_true();
        }
        self.disable_automatic_type_acquisition.is_true()
    }
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InlayHintsPreferences {
    pub include_inlay_parameter_name_hints: IncludeInlayParameterNameHints,
    pub include_inlay_parameter_name_hints_when_argument_matches_name: core::Tristate,
    pub include_inlay_function_parameter_type_hints: core::Tristate,
    pub include_inlay_variable_type_hints: core::Tristate,
    pub include_inlay_variable_type_hints_when_type_matches_name: core::Tristate,
    pub include_inlay_property_declaration_type_hints: core::Tristate,
    pub include_inlay_function_like_return_type_hints: core::Tristate,
    pub include_inlay_enum_member_value_hints: core::Tristate,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeLensUserPreferences {
    pub references_code_lens_enabled: core::Tristate,
    pub implementations_code_lens_enabled: core::Tristate,
    pub references_code_lens_show_on_all_functions: core::Tristate,
    pub implementations_code_lens_show_on_interface_methods: core::Tristate,
    pub implementations_code_lens_show_on_all_class_methods: core::Tristate,
}

// --- Enum Types ---

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub enum QuotePreference {
    #[serde(rename = "")]
    #[default]
    Unknown,
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "double")]
    Double,
    #[serde(rename = "single")]
    Single,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub enum JsxAttributeCompletionStyle {
    #[serde(rename = "")]
    Unknown,
    #[serde(rename = "auto")]
    #[default]
    Auto,
    #[serde(rename = "braces")]
    Braces,
    #[serde(rename = "none")]
    None,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub enum IncludeInlayParameterNameHints {
    #[serde(rename = "")]
    #[default]
    None,
    #[serde(rename = "all")]
    All,
    #[serde(rename = "literals")]
    Literals,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub enum OrganizeImportsCollation {
    #[serde(rename = "ordinal")]
    #[default]
    Ordinal,
    #[serde(rename = "unicode")]
    Unicode,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub enum OrganizeImportsCaseFirst {
    #[serde(rename = "")]
    #[default]
    False,
    #[serde(rename = "lower")]
    Lower,
    #[serde(rename = "upper")]
    Upper,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, serde::Serialize)]
pub enum OrganizeImportsTypeOrder {
    #[serde(rename = "auto")]
    #[default]
    Auto,
    #[serde(rename = "last")]
    Last,
    #[serde(rename = "inline")]
    Inline,
    #[serde(rename = "first")]
    First,
}

// --- Reflection-based parsing infrastructure ---

fn get_nested_value<'a>(
    config: &'a HashMap<String, serde_json::Value>,
    path: &str,
) -> Option<&'a serde_json::Value> {
    let mut current = config.get(path.split('.').next()?)?;
    for part in path.split('.').skip(1) {
        current = current.as_object()?.get(part)?;
    }
    Some(current)
}

fn set_nested_value(
    config: &mut serde_json::Map<String, serde_json::Value>,
    path: &str,
    value: serde_json::Value,
) {
    if value.is_null() {
        return;
    }
    let mut parts = path.split('.').peekable();
    let mut current = config;
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            current.insert(part.to_string(), value);
            return;
        }
        if !current.contains_key(part) || !current.get(part).unwrap().is_object() {
            current.insert(
                part.to_string(),
                serde_json::Value::Object(serde_json::Map::new()),
            );
        }
        current = current
            .get_mut(part)
            .and_then(serde_json::Value::as_object_mut)
            .expect("object inserted above");
    }
}

impl UserPreferences {
    pub fn with_config(mut self, config: &HashMap<String, serde_json::Value>) -> UserPreferences {
        if let Some(unstable) = config
            .get("unstable")
            .and_then(serde_json::Value::as_object)
        {
            self.apply_unstable(unstable);
        }

        self.apply_config_path_overrides(config);

        // Validate CustomConfigFileName for path traversal
        if !self.custom_config_file_name.is_empty() {
            let name = self.custom_config_file_name.trim().to_string();
            if name.contains(['/', '\\']) || name == ".." || name == "." {
                self.custom_config_file_name.clear();
            } else {
                self.custom_config_file_name = name;
            }
        }

        self
    }

    fn apply_unstable(&mut self, unstable: &serde_json::Map<String, serde_json::Value>) {
        if let Some(v) = unstable.get("quotePreference") {
            self.quote_preference = parse_quote_preference(v);
        }
        if let Some(v) = unstable.get("lazyConfiguredProjectsFromExternalProject") {
            self.lazy_configured_projects_from_external_project = parse_tristate(v);
        }
        if let Some(v) = unstable.get("maximumHoverLength") {
            self.maximum_hover_length = parse_i32(v);
        }
        if let Some(v) = unstable.get("includeCompletionsForModuleExports") {
            self.include_completions_for_module_exports = parse_tristate(v);
        }
        if let Some(v) = unstable.get("includeCompletionsForImportStatements") {
            self.include_completions_for_import_statements = parse_tristate(v);
        }
        if let Some(v) = unstable.get("includeAutomaticOptionalChainCompletions") {
            self.include_automatic_optional_chain_completions = parse_tristate(v);
        }
        if let Some(v) = unstable.get("includeCompletionsWithClassMemberSnippets") {
            self.include_completions_with_class_member_snippets = parse_tristate(v);
        }
        if let Some(v) = unstable.get("includeCompletionsWithObjectLiteralMethodSnippets") {
            self.include_completions_with_object_literal_method_snippets = parse_tristate(v);
        }
        if let Some(v) = unstable.get("jsxAttributeCompletionStyle") {
            self.jsx_attribute_completion_style = parse_jsx_attribute_completion_style(v);
        }
        if let Some(v) = unstable.get("importModuleSpecifierPreference") {
            self.import_module_specifier_preference = parse_import_module_specifier_preference(v);
        }
        if let Some(v) = unstable.get("importModuleSpecifierEnding") {
            self.import_module_specifier_ending = parse_import_module_specifier_ending(v);
        }
        if let Some(v) = unstable.get("autoImportSpecifierExcludeRegexes") {
            self.auto_import_specifier_exclude_regexes = parse_string_vec(v);
        }
        if let Some(v) = unstable.get("autoImportFileExcludePatterns") {
            self.auto_import_file_exclude_patterns = parse_string_vec(v);
        }
        if let Some(v) = unstable.get("autoImportEntrypointDirectorySearch") {
            self.auto_import_entrypoint_directory_search = parse_tristate(v);
        }
        if let Some(v) = unstable.get("preferTypeOnlyAutoImports") {
            self.prefer_type_only_auto_imports = parse_tristate(v);
        }
        if let Some(v) = unstable.get("organizeImportsIgnoreCase") {
            self.organize_imports_ignore_case = parse_tristate(v);
        }
        if let Some(v) = unstable.get("organizeImportsCollation") {
            self.organize_imports_collation = parse_organize_imports_collation(v);
        }
        if let Some(v) = unstable.get("organizeImportsLocale") {
            self.organize_imports_locale = parse_string(v);
        }
        if let Some(v) = unstable.get("organizeImportsNumericCollation") {
            self.organize_imports_numeric_collation = parse_tristate(v);
        }
        if let Some(v) = unstable.get("organizeImportsAccentCollation") {
            self.organize_imports_accent_collation = parse_tristate(v);
        }
        if let Some(v) = unstable.get("organizeImportsCaseFirst") {
            self.organize_imports_case_first = parse_organize_imports_case_first(v);
        }
        if let Some(v) = unstable.get("organizeImportsTypeOrder") {
            self.organize_imports_type_order = parse_organize_imports_type_order(v);
        }
        if let Some(v) = unstable.get("allowTextChangesInNewFiles") {
            self.allow_text_changes_in_new_files = parse_tristate(v);
        }
        if let Some(v) = unstable.get("providePrefixAndSuffixTextForRename") {
            self.use_aliases_for_rename = parse_tristate(v);
        }
        if let Some(v) = unstable.get("allowRenameOfImportPath") {
            self.allow_rename_of_import_path = parse_tristate(v);
        }
        if let Some(v) = unstable.get("provideRefactorNotApplicableReason") {
            self.provide_refactor_not_applicable_reason = parse_tristate(v);
        }
        if let Some(v) = unstable.get("preferGoToSourceDefinition") {
            self.prefer_go_to_source_definition = parse_bool(v);
        }
        if let Some(v) = unstable.get("excludeLibrarySymbolsInNavTo") {
            self.exclude_library_symbols_in_nav_to = parse_tristate(v);
        }
        if let Some(v) = unstable.get("disableSuggestions") {
            self.disable_suggestions = parse_tristate(v);
        }
        if let Some(v) = unstable.get("disableLineTextInReferences") {
            self.disable_line_text_in_references = parse_tristate(v);
        }
        if let Some(v) = unstable.get("reportStyleChecksAsWarnings") {
            self.report_style_checks_as_warnings = parse_tristate(v);
        }
        if let Some(v) = unstable.get("disableAutomaticTypeAcquisition") {
            self.disable_automatic_type_acquisition = parse_tristate(v);
        }
        if let Some(v) = unstable.get("automaticTypeAcquisitionEnabled") {
            self.automatic_type_acquisition_enabled = parse_tristate(v);
        }
        if let Some(v) = unstable.get("customConfigFileName") {
            self.custom_config_file_name = parse_string(v);
        }
        self.apply_inlay_hints_unstable(unstable);
        self.apply_code_lens_unstable(unstable);
        self.apply_format_unstable(unstable);
    }

    fn apply_config_path_overrides(&mut self, config: &HashMap<String, serde_json::Value>) {
        if let Some(v) = get_nested_value(config, "preferences.quoteStyle") {
            self.quote_preference = parse_quote_preference(v);
        }
        if let Some(v) = get_nested_value(config, "suggest.autoImports") {
            self.include_completions_for_module_exports = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "suggest.includeCompletionsForImportStatements") {
            self.include_completions_for_import_statements = parse_tristate(v);
        }
        if let Some(v) =
            get_nested_value(config, "suggest.includeAutomaticOptionalChainCompletions")
        {
            self.include_automatic_optional_chain_completions = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "suggest.classMemberSnippets.enabled") {
            self.include_completions_with_class_member_snippets = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "suggest.objectLiteralMethodSnippets.enabled") {
            self.include_completions_with_object_literal_method_snippets = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "preferences.jsxAttributeCompletionStyle") {
            self.jsx_attribute_completion_style = parse_jsx_attribute_completion_style(v);
        }
        if let Some(v) = get_nested_value(config, "preferences.importModuleSpecifier") {
            self.import_module_specifier_preference = parse_import_module_specifier_preference(v);
        }
        if let Some(v) = get_nested_value(config, "preferences.importModuleSpecifierEnding") {
            self.import_module_specifier_ending = parse_import_module_specifier_ending(v);
        }
        if let Some(v) = get_nested_value(config, "preferences.autoImportSpecifierExcludeRegexes") {
            self.auto_import_specifier_exclude_regexes = parse_string_vec(v);
        }
        if let Some(v) = get_nested_value(config, "preferences.autoImportFileExcludePatterns") {
            self.auto_import_file_exclude_patterns = parse_string_vec(v);
        }
        if let Some(v) = get_nested_value(config, "preferences.autoImportEntrypointDirectorySearch")
        {
            self.auto_import_entrypoint_directory_search = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "preferences.preferTypeOnlyAutoImports") {
            self.prefer_type_only_auto_imports = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "preferences.organizeImports.caseSensitivity") {
            self.organize_imports_ignore_case = parse_case_sensitivity(v);
        }
        if let Some(v) = get_nested_value(config, "preferences.organizeImports.unicodeCollation") {
            self.organize_imports_collation = parse_organize_imports_collation(v);
        }
        if let Some(v) = get_nested_value(config, "preferences.organizeImports.locale") {
            self.organize_imports_locale = parse_string(v);
        }
        if let Some(v) = get_nested_value(config, "preferences.organizeImports.numericCollation") {
            self.organize_imports_numeric_collation = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "preferences.organizeImports.accentCollation") {
            self.organize_imports_accent_collation = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "preferences.organizeImports.caseFirst") {
            self.organize_imports_case_first = parse_organize_imports_case_first(v);
        }
        if let Some(v) = get_nested_value(config, "preferences.organizeImports.typeOrder") {
            self.organize_imports_type_order = parse_organize_imports_type_order(v);
        }
        if let Some(v) = get_nested_value(config, "preferences.useAliasesForRenames") {
            self.use_aliases_for_rename = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "workspaceSymbols.excludeLibrarySymbols") {
            self.exclude_library_symbols_in_nav_to = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "disableAutomaticTypeAcquisition") {
            self.disable_automatic_type_acquisition = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "tsserver.automaticTypeAcquisition.enabled") {
            self.automatic_type_acquisition_enabled = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "native-preview.customConfigFileName") {
            self.custom_config_file_name = parse_string(v);
        }
        self.apply_inlay_hints_config(config);
        self.apply_code_lens_config(config);
        self.apply_format_config(config);
    }

    fn apply_inlay_hints_unstable(
        &mut self,
        unstable: &serde_json::Map<String, serde_json::Value>,
    ) {
        if let Some(v) = unstable.get("includeInlayParameterNameHints") {
            self.inlay_hints.include_inlay_parameter_name_hints =
                parse_include_inlay_parameter_name_hints(v);
        }
        if let Some(v) = unstable.get("includeInlayParameterNameHintsWhenArgumentMatchesName") {
            self.inlay_hints
                .include_inlay_parameter_name_hints_when_argument_matches_name = parse_tristate(v);
        }
        if let Some(v) = unstable.get("includeInlayFunctionParameterTypeHints") {
            self.inlay_hints.include_inlay_function_parameter_type_hints = parse_tristate(v);
        }
        if let Some(v) = unstable.get("includeInlayVariableTypeHints") {
            self.inlay_hints.include_inlay_variable_type_hints = parse_tristate(v);
        }
        if let Some(v) = unstable.get("includeInlayVariableTypeHintsWhenTypeMatchesName") {
            self.inlay_hints
                .include_inlay_variable_type_hints_when_type_matches_name = parse_tristate(v);
        }
        if let Some(v) = unstable.get("includeInlayPropertyDeclarationTypeHints") {
            self.inlay_hints
                .include_inlay_property_declaration_type_hints = parse_tristate(v);
        }
        if let Some(v) = unstable.get("includeInlayFunctionLikeReturnTypeHints") {
            self.inlay_hints
                .include_inlay_function_like_return_type_hints = parse_tristate(v);
        }
        if let Some(v) = unstable.get("includeInlayEnumMemberValueHints") {
            self.inlay_hints.include_inlay_enum_member_value_hints = parse_tristate(v);
        }
    }

    fn apply_inlay_hints_config(&mut self, config: &HashMap<String, serde_json::Value>) {
        if let Some(v) = get_nested_value(config, "inlayHints.parameterNames.enabled") {
            self.inlay_hints.include_inlay_parameter_name_hints =
                parse_include_inlay_parameter_name_hints(v);
        }
        if let Some(v) = get_nested_value(
            config,
            "inlayHints.parameterNames.suppressWhenArgumentMatchesName",
        ) {
            self.inlay_hints
                .include_inlay_parameter_name_hints_when_argument_matches_name =
                invert_tristate(parse_tristate(v));
        }
        if let Some(v) = get_nested_value(config, "inlayHints.parameterTypes.enabled") {
            self.inlay_hints.include_inlay_function_parameter_type_hints = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "inlayHints.variableTypes.enabled") {
            self.inlay_hints.include_inlay_variable_type_hints = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(
            config,
            "inlayHints.variableTypes.suppressWhenTypeMatchesName",
        ) {
            self.inlay_hints
                .include_inlay_variable_type_hints_when_type_matches_name =
                invert_tristate(parse_tristate(v));
        }
        if let Some(v) = get_nested_value(config, "inlayHints.propertyDeclarationTypes.enabled") {
            self.inlay_hints
                .include_inlay_property_declaration_type_hints = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "inlayHints.functionLikeReturnTypes.enabled") {
            self.inlay_hints
                .include_inlay_function_like_return_type_hints = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "inlayHints.enumMemberValues.enabled") {
            self.inlay_hints.include_inlay_enum_member_value_hints = parse_tristate(v);
        }
    }

    fn apply_code_lens_unstable(&mut self, unstable: &serde_json::Map<String, serde_json::Value>) {
        if let Some(v) = unstable.get("referencesCodeLensEnabled") {
            self.code_lens.references_code_lens_enabled = parse_tristate(v);
        }
        if let Some(v) = unstable.get("implementationsCodeLensEnabled") {
            self.code_lens.implementations_code_lens_enabled = parse_tristate(v);
        }
        if let Some(v) = unstable.get("referencesCodeLensShowOnAllFunctions") {
            self.code_lens.references_code_lens_show_on_all_functions = parse_tristate(v);
        }
        if let Some(v) = unstable.get("implementationsCodeLensShowOnInterfaceMethods") {
            self.code_lens
                .implementations_code_lens_show_on_interface_methods = parse_tristate(v);
        }
        if let Some(v) = unstable.get("implementationsCodeLensShowOnAllClassMethods") {
            self.code_lens
                .implementations_code_lens_show_on_all_class_methods = parse_tristate(v);
        }
    }

    fn apply_code_lens_config(&mut self, config: &HashMap<String, serde_json::Value>) {
        if let Some(v) = get_nested_value(config, "referencesCodeLens.enabled") {
            self.code_lens.references_code_lens_enabled = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "implementationsCodeLens.enabled") {
            self.code_lens.implementations_code_lens_enabled = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "referencesCodeLens.showOnAllFunctions") {
            self.code_lens.references_code_lens_show_on_all_functions = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "implementationsCodeLens.showOnInterfaceMethods")
        {
            self.code_lens
                .implementations_code_lens_show_on_interface_methods = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "implementationsCodeLens.showOnAllClassMethods") {
            self.code_lens
                .implementations_code_lens_show_on_all_class_methods = parse_tristate(v);
        }
    }

    fn apply_format_unstable(&mut self, unstable: &serde_json::Map<String, serde_json::Value>) {
        if let Some(v) = unstable.get("baseIndentSize") {
            self.format_code_settings.editor_settings.base_indent_size = parse_i32(v);
        }
        if let Some(v) = unstable.get("indentSize") {
            self.format_code_settings.editor_settings.indent_size = parse_i32(v);
        }
        if let Some(v) = unstable.get("tabSize") {
            self.format_code_settings.editor_settings.tab_size = parse_i32(v);
        }
        if let Some(v) = unstable.get("newLineCharacter") {
            self.format_code_settings.editor_settings.new_line_character = parse_string(v);
        }
        if let Some(v) = unstable.get("convertTabsToSpaces") {
            self.format_code_settings
                .editor_settings
                .convert_tabs_to_spaces = parse_tristate(v);
        }
        if let Some(v) = unstable.get("indentStyle") {
            self.format_code_settings.editor_settings.indent_style = parse_indent_style_json(v);
        }
        if let Some(v) = unstable.get("trimTrailingWhitespace") {
            self.format_code_settings
                .editor_settings
                .trim_trailing_whitespace = parse_tristate(v);
        }
        if let Some(v) = unstable.get("insertSpaceAfterCommaDelimiter") {
            self.format_code_settings.insert_space_after_comma_delimiter = parse_tristate(v);
        }
        if let Some(v) = unstable.get("insertSpaceAfterSemicolonInForStatements") {
            self.format_code_settings
                .insert_space_after_semicolon_in_for_statements = parse_tristate(v);
        }
        if let Some(v) = unstable.get("insertSpaceBeforeAndAfterBinaryOperators") {
            self.format_code_settings
                .insert_space_before_and_after_binary_operators = parse_tristate(v);
        }
        if let Some(v) = unstable.get("insertSpaceAfterConstructor") {
            self.format_code_settings.insert_space_after_constructor = parse_tristate(v);
        }
        if let Some(v) = unstable.get("insertSpaceAfterKeywordsInControlFlowStatements") {
            self.format_code_settings
                .insert_space_after_keywords_in_control_flow_statements = parse_tristate(v);
        }
        if let Some(v) = unstable.get("insertSpaceAfterFunctionKeywordForAnonymousFunctions") {
            self.format_code_settings
                .insert_space_after_function_keyword_for_anonymous_functions = parse_tristate(v);
        }
        if let Some(v) = unstable.get("insertSpaceAfterOpeningAndBeforeClosingNonemptyParenthesis")
        {
            self.format_code_settings
                .insert_space_after_opening_and_before_closing_nonempty_parenthesis =
                parse_tristate(v);
        }
        if let Some(v) = unstable.get("insertSpaceAfterOpeningAndBeforeClosingNonemptyBrackets") {
            self.format_code_settings
                .insert_space_after_opening_and_before_closing_nonempty_brackets =
                parse_tristate(v);
        }
        if let Some(v) = unstable.get("insertSpaceAfterOpeningAndBeforeClosingNonemptyBraces") {
            self.format_code_settings
                .insert_space_after_opening_and_before_closing_nonempty_braces = parse_tristate(v);
        }
        if let Some(v) = unstable.get("insertSpaceAfterOpeningAndBeforeClosingEmptyBraces") {
            self.format_code_settings
                .insert_space_after_opening_and_before_closing_empty_braces = parse_tristate(v);
        }
        if let Some(v) = unstable.get("insertSpaceAfterOpeningAndBeforeClosingTemplateStringBraces")
        {
            self.format_code_settings
                .insert_space_after_opening_and_before_closing_template_string_braces =
                parse_tristate(v);
        }
        if let Some(v) = unstable.get("insertSpaceAfterOpeningAndBeforeClosingJsxExpressionBraces")
        {
            self.format_code_settings
                .insert_space_after_opening_and_before_closing_jsx_expression_braces =
                parse_tristate(v);
        }
        if let Some(v) = unstable.get("insertSpaceAfterTypeAssertion") {
            self.format_code_settings.insert_space_after_type_assertion = parse_tristate(v);
        }
        if let Some(v) = unstable.get("insertSpaceBeforeFunctionParenthesis") {
            self.format_code_settings
                .insert_space_before_function_parenthesis = parse_tristate(v);
        }
        if let Some(v) = unstable.get("placeOpenBraceOnNewLineForFunctions") {
            self.format_code_settings
                .place_open_brace_on_new_line_for_functions = parse_tristate(v);
        }
        if let Some(v) = unstable.get("placeOpenBraceOnNewLineForControlBlocks") {
            self.format_code_settings
                .place_open_brace_on_new_line_for_control_blocks = parse_tristate(v);
        }
        if let Some(v) = unstable.get("insertSpaceBeforeTypeAnnotation") {
            self.format_code_settings
                .insert_space_before_type_annotation = parse_tristate(v);
        }
        if let Some(v) = unstable.get("indentMultiLineObjectLiteralBeginningOnBlankLine") {
            self.format_code_settings
                .indent_multi_line_object_literal_beginning_on_blank_line = parse_tristate(v);
        }
        if let Some(v) = unstable.get("semicolons") {
            self.format_code_settings.semicolons = parse_semicolon_preference_json(v);
        }
        if let Some(v) = unstable.get("indentSwitchCase") {
            self.format_code_settings.indent_switch_case = parse_tristate(v);
        }
    }

    fn apply_format_config(&mut self, config: &HashMap<String, serde_json::Value>) {
        if let Some(v) = get_nested_value(config, "format.baseIndentSize") {
            self.format_code_settings.editor_settings.base_indent_size = parse_i32(v);
        }
        if let Some(v) = get_nested_value(config, "format.indentSize") {
            self.format_code_settings.editor_settings.indent_size = parse_i32(v);
        }
        if let Some(v) = get_nested_value(config, "format.tabSize") {
            self.format_code_settings.editor_settings.tab_size = parse_i32(v);
        }
        if let Some(v) = get_nested_value(config, "format.newLineCharacter") {
            self.format_code_settings.editor_settings.new_line_character = parse_string(v);
        }
        if let Some(v) = get_nested_value(config, "format.convertTabsToSpaces") {
            self.format_code_settings
                .editor_settings
                .convert_tabs_to_spaces = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "format.indentStyle") {
            self.format_code_settings.editor_settings.indent_style = parse_indent_style_json(v);
        }
        if let Some(v) = get_nested_value(config, "format.trimTrailingWhitespace") {
            self.format_code_settings
                .editor_settings
                .trim_trailing_whitespace = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "format.insertSpaceAfterCommaDelimiter") {
            self.format_code_settings.insert_space_after_comma_delimiter = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "format.insertSpaceAfterSemicolonInForStatements")
        {
            self.format_code_settings
                .insert_space_after_semicolon_in_for_statements = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "format.insertSpaceBeforeAndAfterBinaryOperators")
        {
            self.format_code_settings
                .insert_space_before_and_after_binary_operators = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "format.insertSpaceAfterConstructor") {
            self.format_code_settings.insert_space_after_constructor = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(
            config,
            "format.insertSpaceAfterKeywordsInControlFlowStatements",
        ) {
            self.format_code_settings
                .insert_space_after_keywords_in_control_flow_statements = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(
            config,
            "format.insertSpaceAfterFunctionKeywordForAnonymousFunctions",
        ) {
            self.format_code_settings
                .insert_space_after_function_keyword_for_anonymous_functions = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(
            config,
            "format.insertSpaceAfterOpeningAndBeforeClosingNonemptyParenthesis",
        ) {
            self.format_code_settings
                .insert_space_after_opening_and_before_closing_nonempty_parenthesis =
                parse_tristate(v);
        }
        if let Some(v) = get_nested_value(
            config,
            "format.insertSpaceAfterOpeningAndBeforeClosingNonemptyBrackets",
        ) {
            self.format_code_settings
                .insert_space_after_opening_and_before_closing_nonempty_brackets =
                parse_tristate(v);
        }
        if let Some(v) = get_nested_value(
            config,
            "format.insertSpaceAfterOpeningAndBeforeClosingNonemptyBraces",
        ) {
            self.format_code_settings
                .insert_space_after_opening_and_before_closing_nonempty_braces = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(
            config,
            "format.insertSpaceAfterOpeningAndBeforeClosingEmptyBraces",
        ) {
            self.format_code_settings
                .insert_space_after_opening_and_before_closing_empty_braces = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(
            config,
            "format.insertSpaceAfterOpeningAndBeforeClosingTemplateStringBraces",
        ) {
            self.format_code_settings
                .insert_space_after_opening_and_before_closing_template_string_braces =
                parse_tristate(v);
        }
        if let Some(v) = get_nested_value(
            config,
            "format.insertSpaceAfterOpeningAndBeforeClosingJsxExpressionBraces",
        ) {
            self.format_code_settings
                .insert_space_after_opening_and_before_closing_jsx_expression_braces =
                parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "format.insertSpaceAfterTypeAssertion") {
            self.format_code_settings.insert_space_after_type_assertion = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "format.insertSpaceBeforeFunctionParenthesis") {
            self.format_code_settings
                .insert_space_before_function_parenthesis = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "format.placeOpenBraceOnNewLineForFunctions") {
            self.format_code_settings
                .place_open_brace_on_new_line_for_functions = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "format.placeOpenBraceOnNewLineForControlBlocks")
        {
            self.format_code_settings
                .place_open_brace_on_new_line_for_control_blocks = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "format.insertSpaceBeforeTypeAnnotation") {
            self.format_code_settings
                .insert_space_before_type_annotation = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(
            config,
            "format.indentMultiLineObjectLiteralBeginningOnBlankLine",
        ) {
            self.format_code_settings
                .indent_multi_line_object_literal_beginning_on_blank_line = parse_tristate(v);
        }
        if let Some(v) = get_nested_value(config, "format.semicolons") {
            self.format_code_settings.semicolons = parse_semicolon_preference_json(v);
        }
        if let Some(v) = get_nested_value(config, "format.indentSwitchCase") {
            self.format_code_settings.indent_switch_case = parse_tristate(v);
        }
    }

    pub fn marshal_json_to<W: Write>(&self, enc: &mut json::Encoder<W>) -> serde_json::Result<()> {
        let mut config = serde_json::Map::new();

        self.write_format_json(&mut config);
        set_nested_value(
            &mut config,
            "preferences.quoteStyle",
            serialize_quote_preference(self.quote_preference),
        );
        set_nested_value(
            &mut config,
            "unstable.lazyConfiguredProjectsFromExternalProject",
            serialize_tristate(self.lazy_configured_projects_from_external_project),
        );
        set_nested_value(
            &mut config,
            "unstable.maximumHoverLength",
            serde_json::Value::Number(self.maximum_hover_length.into()),
        );
        set_nested_value(
            &mut config,
            "suggest.autoImports",
            serialize_tristate(self.include_completions_for_module_exports),
        );
        set_nested_value(
            &mut config,
            "suggest.includeCompletionsForImportStatements",
            serialize_tristate(self.include_completions_for_import_statements),
        );
        set_nested_value(
            &mut config,
            "suggest.includeAutomaticOptionalChainCompletions",
            serialize_tristate(self.include_automatic_optional_chain_completions),
        );
        set_nested_value(
            &mut config,
            "suggest.classMemberSnippets.enabled",
            serialize_tristate(self.include_completions_with_class_member_snippets),
        );
        set_nested_value(
            &mut config,
            "suggest.objectLiteralMethodSnippets.enabled",
            serialize_tristate(self.include_completions_with_object_literal_method_snippets),
        );
        set_nested_value(
            &mut config,
            "preferences.jsxAttributeCompletionStyle",
            serialize_jsx_attribute_completion_style(self.jsx_attribute_completion_style),
        );
        set_nested_value(
            &mut config,
            "preferences.importModuleSpecifier",
            serialize_import_module_specifier_preference(self.import_module_specifier_preference),
        );
        set_nested_value(
            &mut config,
            "preferences.importModuleSpecifierEnding",
            serialize_import_module_specifier_ending(self.import_module_specifier_ending),
        );
        if !self.auto_import_specifier_exclude_regexes.is_empty() {
            set_nested_value(
                &mut config,
                "preferences.autoImportSpecifierExcludeRegexes",
                serialize_string_vec(&self.auto_import_specifier_exclude_regexes),
            );
        }
        if !self.auto_import_file_exclude_patterns.is_empty() {
            set_nested_value(
                &mut config,
                "preferences.autoImportFileExcludePatterns",
                serialize_string_vec(&self.auto_import_file_exclude_patterns),
            );
        }
        set_nested_value(
            &mut config,
            "preferences.autoImportEntrypointDirectorySearch",
            serialize_tristate(self.auto_import_entrypoint_directory_search),
        );
        set_nested_value(
            &mut config,
            "preferences.preferTypeOnlyAutoImports",
            serialize_tristate(self.prefer_type_only_auto_imports),
        );
        set_nested_value(
            &mut config,
            "preferences.organizeImports.caseSensitivity",
            serialize_tristate(self.organize_imports_ignore_case),
        );
        set_nested_value(
            &mut config,
            "preferences.organizeImports.unicodeCollation",
            serialize_organize_imports_collation(self.organize_imports_collation),
        );
        set_nested_value(
            &mut config,
            "preferences.organizeImports.locale",
            serde_json::Value::String(self.organize_imports_locale.clone()),
        );
        set_nested_value(
            &mut config,
            "preferences.organizeImports.numericCollation",
            serialize_tristate(self.organize_imports_numeric_collation),
        );
        set_nested_value(
            &mut config,
            "preferences.organizeImports.accentCollation",
            serialize_tristate(self.organize_imports_accent_collation),
        );
        set_nested_value(
            &mut config,
            "preferences.organizeImports.caseFirst",
            serialize_organize_imports_case_first(self.organize_imports_case_first),
        );
        set_nested_value(
            &mut config,
            "preferences.organizeImports.typeOrder",
            serialize_organize_imports_type_order(self.organize_imports_type_order),
        );
        set_nested_value(
            &mut config,
            "unstable.allowTextChangesInNewFiles",
            serialize_tristate(self.allow_text_changes_in_new_files),
        );
        set_nested_value(
            &mut config,
            "preferences.useAliasesForRenames",
            serialize_tristate(self.use_aliases_for_rename),
        );
        set_nested_value(
            &mut config,
            "unstable.allowRenameOfImportPath",
            serialize_tristate(self.allow_rename_of_import_path),
        );
        set_nested_value(
            &mut config,
            "unstable.provideRefactorNotApplicableReason",
            serialize_tristate(self.provide_refactor_not_applicable_reason),
        );
        self.write_inlay_hints_json(&mut config);
        self.write_code_lens_json(&mut config);
        set_nested_value(
            &mut config,
            "unstable.preferGoToSourceDefinition",
            serde_json::Value::Bool(self.prefer_go_to_source_definition),
        );
        set_nested_value(
            &mut config,
            "workspaceSymbols.excludeLibrarySymbols",
            serialize_tristate(self.exclude_library_symbols_in_nav_to),
        );
        set_nested_value(
            &mut config,
            "unstable.disableSuggestions",
            serialize_tristate(self.disable_suggestions),
        );
        set_nested_value(
            &mut config,
            "unstable.disableLineTextInReferences",
            serialize_tristate(self.disable_line_text_in_references),
        );
        set_nested_value(
            &mut config,
            "unstable.reportStyleChecksAsWarnings",
            serialize_tristate(self.report_style_checks_as_warnings),
        );
        set_nested_value(
            &mut config,
            "disableAutomaticTypeAcquisition",
            serialize_tristate(self.disable_automatic_type_acquisition),
        );
        set_nested_value(
            &mut config,
            "tsserver.automaticTypeAcquisition.enabled",
            serialize_tristate(self.automatic_type_acquisition_enabled),
        );
        set_nested_value(
            &mut config,
            "native-preview.customConfigFileName",
            serde_json::Value::String(self.custom_config_file_name.clone()),
        );

        json::marshal_encode(
            enc,
            &serde_json::Value::Object(config),
            &[json::deterministic(true)],
        )
    }

    fn write_format_json(&self, config: &mut serde_json::Map<String, serde_json::Value>) {
        let settings = &self.format_code_settings;
        let editor = &settings.editor_settings;
        set_nested_value(
            config,
            "format.baseIndentSize",
            editor.base_indent_size.into(),
        );
        set_nested_value(config, "format.indentSize", editor.indent_size.into());
        set_nested_value(config, "format.tabSize", editor.tab_size.into());
        set_nested_value(
            config,
            "format.newLineCharacter",
            serde_json::Value::String(editor.new_line_character.clone()),
        );
        set_nested_value(
            config,
            "format.convertTabsToSpaces",
            serialize_tristate(editor.convert_tabs_to_spaces),
        );
        set_nested_value(
            config,
            "format.indentStyle",
            serialize_indent_style(editor.indent_style),
        );
        set_nested_value(
            config,
            "format.trimTrailingWhitespace",
            serialize_tristate(editor.trim_trailing_whitespace),
        );
        set_nested_value(
            config,
            "format.insertSpaceAfterCommaDelimiter",
            serialize_tristate(settings.insert_space_after_comma_delimiter),
        );
        set_nested_value(
            config,
            "format.insertSpaceAfterSemicolonInForStatements",
            serialize_tristate(settings.insert_space_after_semicolon_in_for_statements),
        );
        set_nested_value(
            config,
            "format.insertSpaceBeforeAndAfterBinaryOperators",
            serialize_tristate(settings.insert_space_before_and_after_binary_operators),
        );
        set_nested_value(
            config,
            "format.insertSpaceAfterConstructor",
            serialize_tristate(settings.insert_space_after_constructor),
        );
        set_nested_value(
            config,
            "format.insertSpaceAfterKeywordsInControlFlowStatements",
            serialize_tristate(settings.insert_space_after_keywords_in_control_flow_statements),
        );
        set_nested_value(
            config,
            "format.insertSpaceAfterFunctionKeywordForAnonymousFunctions",
            serialize_tristate(
                settings.insert_space_after_function_keyword_for_anonymous_functions,
            ),
        );
        set_nested_value(
            config,
            "format.insertSpaceAfterOpeningAndBeforeClosingNonemptyParenthesis",
            serialize_tristate(
                settings.insert_space_after_opening_and_before_closing_nonempty_parenthesis,
            ),
        );
        set_nested_value(
            config,
            "format.insertSpaceAfterOpeningAndBeforeClosingNonemptyBrackets",
            serialize_tristate(
                settings.insert_space_after_opening_and_before_closing_nonempty_brackets,
            ),
        );
        set_nested_value(
            config,
            "format.insertSpaceAfterOpeningAndBeforeClosingNonemptyBraces",
            serialize_tristate(
                settings.insert_space_after_opening_and_before_closing_nonempty_braces,
            ),
        );
        set_nested_value(
            config,
            "format.insertSpaceAfterOpeningAndBeforeClosingEmptyBraces",
            serialize_tristate(settings.insert_space_after_opening_and_before_closing_empty_braces),
        );
        set_nested_value(
            config,
            "format.insertSpaceAfterOpeningAndBeforeClosingTemplateStringBraces",
            serialize_tristate(
                settings.insert_space_after_opening_and_before_closing_template_string_braces,
            ),
        );
        set_nested_value(
            config,
            "format.insertSpaceAfterOpeningAndBeforeClosingJsxExpressionBraces",
            serialize_tristate(
                settings.insert_space_after_opening_and_before_closing_jsx_expression_braces,
            ),
        );
        set_nested_value(
            config,
            "format.insertSpaceAfterTypeAssertion",
            serialize_tristate(settings.insert_space_after_type_assertion),
        );
        set_nested_value(
            config,
            "format.insertSpaceBeforeFunctionParenthesis",
            serialize_tristate(settings.insert_space_before_function_parenthesis),
        );
        set_nested_value(
            config,
            "format.placeOpenBraceOnNewLineForFunctions",
            serialize_tristate(settings.place_open_brace_on_new_line_for_functions),
        );
        set_nested_value(
            config,
            "format.placeOpenBraceOnNewLineForControlBlocks",
            serialize_tristate(settings.place_open_brace_on_new_line_for_control_blocks),
        );
        set_nested_value(
            config,
            "format.insertSpaceBeforeTypeAnnotation",
            serialize_tristate(settings.insert_space_before_type_annotation),
        );
        set_nested_value(
            config,
            "format.indentMultiLineObjectLiteralBeginningOnBlankLine",
            serialize_tristate(settings.indent_multi_line_object_literal_beginning_on_blank_line),
        );
        set_nested_value(
            config,
            "format.semicolons",
            serialize_semicolon_preference(settings.semicolons),
        );
        set_nested_value(
            config,
            "format.indentSwitchCase",
            serialize_tristate(settings.indent_switch_case),
        );
    }

    fn write_inlay_hints_json(&self, config: &mut serde_json::Map<String, serde_json::Value>) {
        let inlay = &self.inlay_hints;
        set_nested_value(
            config,
            "inlayHints.parameterNames.enabled",
            serialize_include_inlay_parameter_name_hints(inlay.include_inlay_parameter_name_hints),
        );
        set_nested_value(
            config,
            "inlayHints.parameterNames.suppressWhenArgumentMatchesName",
            serialize_tristate(invert_tristate(
                inlay.include_inlay_parameter_name_hints_when_argument_matches_name,
            )),
        );
        set_nested_value(
            config,
            "inlayHints.parameterTypes.enabled",
            serialize_tristate(inlay.include_inlay_function_parameter_type_hints),
        );
        set_nested_value(
            config,
            "inlayHints.variableTypes.enabled",
            serialize_tristate(inlay.include_inlay_variable_type_hints),
        );
        set_nested_value(
            config,
            "inlayHints.variableTypes.suppressWhenTypeMatchesName",
            serialize_tristate(invert_tristate(
                inlay.include_inlay_variable_type_hints_when_type_matches_name,
            )),
        );
        set_nested_value(
            config,
            "inlayHints.propertyDeclarationTypes.enabled",
            serialize_tristate(inlay.include_inlay_property_declaration_type_hints),
        );
        set_nested_value(
            config,
            "inlayHints.functionLikeReturnTypes.enabled",
            serialize_tristate(inlay.include_inlay_function_like_return_type_hints),
        );
        set_nested_value(
            config,
            "inlayHints.enumMemberValues.enabled",
            serialize_tristate(inlay.include_inlay_enum_member_value_hints),
        );
    }

    fn write_code_lens_json(&self, config: &mut serde_json::Map<String, serde_json::Value>) {
        let code_lens = &self.code_lens;
        set_nested_value(
            config,
            "referencesCodeLens.enabled",
            serialize_tristate(code_lens.references_code_lens_enabled),
        );
        set_nested_value(
            config,
            "implementationsCodeLens.enabled",
            serialize_tristate(code_lens.implementations_code_lens_enabled),
        );
        set_nested_value(
            config,
            "referencesCodeLens.showOnAllFunctions",
            serialize_tristate(code_lens.references_code_lens_show_on_all_functions),
        );
        set_nested_value(
            config,
            "implementationsCodeLens.showOnInterfaceMethods",
            serialize_tristate(code_lens.implementations_code_lens_show_on_interface_methods),
        );
        set_nested_value(
            config,
            "implementationsCodeLens.showOnAllClassMethods",
            serialize_tristate(code_lens.implementations_code_lens_show_on_all_class_methods),
        );
    }

    pub fn unmarshal_json_from<R: Read>(
        &mut self,
        dec: &mut json::Decoder<R>,
    ) -> serde_json::Result<()> {
        let mut config = HashMap::<String, serde_json::Value>::new();
        json::unmarshal_decode(dec, &mut config, &[])?;
        *self = new_default_user_preferences().with_config(&config);
        Ok(())
    }

    // WithOverrides returns a copy of p with non-zero fields from overrides applied on top.
    // This is safe because all preference fields use types where zero = "not set":
    // Tristate (TSUnknown=0), int (0), string (""), slice (nil).
    pub fn with_overrides(mut self, overrides: UserPreferences) -> UserPreferences {
        merge_format_code_settings(
            &mut self.format_code_settings,
            overrides.format_code_settings,
        );
        if overrides.quote_preference != QuotePreference::Unknown {
            self.quote_preference = overrides.quote_preference;
        }
        merge_tristate(
            &mut self.lazy_configured_projects_from_external_project,
            overrides.lazy_configured_projects_from_external_project,
        );
        if overrides.maximum_hover_length != 0 {
            self.maximum_hover_length = overrides.maximum_hover_length;
        }
        merge_tristate(
            &mut self.include_completions_for_module_exports,
            overrides.include_completions_for_module_exports,
        );
        merge_tristate(
            &mut self.include_completions_for_import_statements,
            overrides.include_completions_for_import_statements,
        );
        merge_tristate(
            &mut self.include_automatic_optional_chain_completions,
            overrides.include_automatic_optional_chain_completions,
        );
        merge_tristate(
            &mut self.include_completions_with_class_member_snippets,
            overrides.include_completions_with_class_member_snippets,
        );
        merge_tristate(
            &mut self.include_completions_with_object_literal_method_snippets,
            overrides.include_completions_with_object_literal_method_snippets,
        );
        if overrides.jsx_attribute_completion_style != JsxAttributeCompletionStyle::Unknown {
            self.jsx_attribute_completion_style = overrides.jsx_attribute_completion_style;
        }
        if overrides.import_module_specifier_preference
            != modulespecifiers::ImportModuleSpecifierPreference::None
        {
            self.import_module_specifier_preference = overrides.import_module_specifier_preference;
        }
        if overrides.import_module_specifier_ending
            != modulespecifiers::ImportModuleSpecifierEndingPreference::None
        {
            self.import_module_specifier_ending = overrides.import_module_specifier_ending;
        }
        if !overrides.auto_import_specifier_exclude_regexes.is_empty() {
            self.auto_import_specifier_exclude_regexes =
                overrides.auto_import_specifier_exclude_regexes;
        }
        if !overrides.auto_import_file_exclude_patterns.is_empty() {
            self.auto_import_file_exclude_patterns = overrides.auto_import_file_exclude_patterns;
        }
        merge_tristate(
            &mut self.auto_import_entrypoint_directory_search,
            overrides.auto_import_entrypoint_directory_search,
        );
        merge_tristate(
            &mut self.prefer_type_only_auto_imports,
            overrides.prefer_type_only_auto_imports,
        );
        merge_tristate(
            &mut self.organize_imports_ignore_case,
            overrides.organize_imports_ignore_case,
        );
        if overrides.organize_imports_collation != OrganizeImportsCollation::Ordinal {
            self.organize_imports_collation = overrides.organize_imports_collation;
        }
        if !overrides.organize_imports_locale.is_empty() {
            self.organize_imports_locale = overrides.organize_imports_locale;
        }
        merge_tristate(
            &mut self.organize_imports_numeric_collation,
            overrides.organize_imports_numeric_collation,
        );
        merge_tristate(
            &mut self.organize_imports_accent_collation,
            overrides.organize_imports_accent_collation,
        );
        if overrides.organize_imports_case_first != OrganizeImportsCaseFirst::False {
            self.organize_imports_case_first = overrides.organize_imports_case_first;
        }
        if overrides.organize_imports_type_order != OrganizeImportsTypeOrder::Auto {
            self.organize_imports_type_order = overrides.organize_imports_type_order;
        }
        merge_tristate(
            &mut self.allow_text_changes_in_new_files,
            overrides.allow_text_changes_in_new_files,
        );
        merge_tristate(
            &mut self.use_aliases_for_rename,
            overrides.use_aliases_for_rename,
        );
        merge_tristate(
            &mut self.allow_rename_of_import_path,
            overrides.allow_rename_of_import_path,
        );
        merge_tristate(
            &mut self.provide_refactor_not_applicable_reason,
            overrides.provide_refactor_not_applicable_reason,
        );
        merge_inlay_hints(&mut self.inlay_hints, overrides.inlay_hints);
        merge_code_lens(&mut self.code_lens, overrides.code_lens);
        if overrides.prefer_go_to_source_definition {
            self.prefer_go_to_source_definition = overrides.prefer_go_to_source_definition;
        }
        merge_tristate(
            &mut self.exclude_library_symbols_in_nav_to,
            overrides.exclude_library_symbols_in_nav_to,
        );
        merge_tristate(&mut self.disable_suggestions, overrides.disable_suggestions);
        merge_tristate(
            &mut self.disable_line_text_in_references,
            overrides.disable_line_text_in_references,
        );
        merge_tristate(
            &mut self.report_style_checks_as_warnings,
            overrides.report_style_checks_as_warnings,
        );
        merge_tristate(
            &mut self.disable_automatic_type_acquisition,
            overrides.disable_automatic_type_acquisition,
        );
        merge_tristate(
            &mut self.automatic_type_acquisition_enabled,
            overrides.automatic_type_acquisition_enabled,
        );
        if !overrides.custom_config_file_name.is_empty() {
            self.custom_config_file_name = overrides.custom_config_file_name;
        }
        self
    }

    pub fn module_specifier_preferences(&self) -> modulespecifiers::UserPreferences {
        modulespecifiers::UserPreferences {
            import_module_specifier_preference: self.import_module_specifier_preference,
            import_module_specifier_ending: self.import_module_specifier_ending,
            auto_import_specifier_exclude_regexes: self
                .auto_import_specifier_exclude_regexes
                .clone(),
        }
    }

    pub fn parsed_auto_import_file_exclude_patterns(
        &self,
        use_case_sensitive_file_names: bool,
    ) -> Option<vfsmatch::SpecMatcher> {
        vfsmatch::new_spec_matcher(
            &self.auto_import_file_exclude_patterns,
            "",
            vfsmatch::Usage::Exclude,
            use_case_sensitive_file_names,
        )
    }

    pub fn is_module_specifier_excluded(&self, module_specifier: &str) -> bool {
        modulespecifiers::is_excluded_by_regex(
            module_specifier,
            &self.auto_import_specifier_exclude_regexes,
        )
    }
}

pub fn parse_user_preferences(items: HashMap<String, serde_json::Value>) -> UserPreferences {
    let mut prefs = new_default_user_preferences();
    // Apply editor settings first (tabSize, indentSize, etc.) as raw-name defaults,
    // then overlay language-specific settings with increasing precedence:
    // editor < javascript < typescript < js/ts
    if let Some(editor_settings) = items.get("editor").and_then(serde_json::Value::as_object) {
        let mut cfg = HashMap::new();
        cfg.insert(
            "unstable".to_string(),
            serde_json::Value::Object(editor_settings.clone()),
        );
        prefs = prefs.with_config(&cfg);
    }
    // Apply javascript, then typescript, then js/ts (highest precedence).
    for section in ["javascript", "typescript", "js/ts"] {
        if let Some(item) = items.get(section) {
            if let Some(settings) = item.as_object() {
                let cfg = settings
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<HashMap<_, _>>();
                prefs = prefs.with_config(&cfg);
            }
        }
    }
    prefs
}

fn merge_tristate(dst: &mut core::Tristate, src: core::Tristate) {
    if !src.is_unknown() {
        *dst = src;
    }
}

fn merge_format_code_settings(dst: &mut FormatCodeSettings, src: FormatCodeSettings) {
    if src.editor_settings.base_indent_size != 0 {
        dst.editor_settings.base_indent_size = src.editor_settings.base_indent_size;
    }
    if src.editor_settings.indent_size != 0 {
        dst.editor_settings.indent_size = src.editor_settings.indent_size;
    }
    if src.editor_settings.tab_size != 0 {
        dst.editor_settings.tab_size = src.editor_settings.tab_size;
    }
    if !src.editor_settings.new_line_character.is_empty() {
        dst.editor_settings.new_line_character = src.editor_settings.new_line_character;
    }
    merge_tristate(
        &mut dst.editor_settings.convert_tabs_to_spaces,
        src.editor_settings.convert_tabs_to_spaces,
    );
    if src.editor_settings.indent_style != IndentStyle::None {
        dst.editor_settings.indent_style = src.editor_settings.indent_style;
    }
    merge_tristate(
        &mut dst.editor_settings.trim_trailing_whitespace,
        src.editor_settings.trim_trailing_whitespace,
    );
    merge_tristate(
        &mut dst.insert_space_after_comma_delimiter,
        src.insert_space_after_comma_delimiter,
    );
    merge_tristate(
        &mut dst.insert_space_after_semicolon_in_for_statements,
        src.insert_space_after_semicolon_in_for_statements,
    );
    merge_tristate(
        &mut dst.insert_space_before_and_after_binary_operators,
        src.insert_space_before_and_after_binary_operators,
    );
    merge_tristate(
        &mut dst.insert_space_after_constructor,
        src.insert_space_after_constructor,
    );
    merge_tristate(
        &mut dst.insert_space_after_keywords_in_control_flow_statements,
        src.insert_space_after_keywords_in_control_flow_statements,
    );
    merge_tristate(
        &mut dst.insert_space_after_function_keyword_for_anonymous_functions,
        src.insert_space_after_function_keyword_for_anonymous_functions,
    );
    merge_tristate(
        &mut dst.insert_space_after_opening_and_before_closing_nonempty_parenthesis,
        src.insert_space_after_opening_and_before_closing_nonempty_parenthesis,
    );
    merge_tristate(
        &mut dst.insert_space_after_opening_and_before_closing_nonempty_brackets,
        src.insert_space_after_opening_and_before_closing_nonempty_brackets,
    );
    merge_tristate(
        &mut dst.insert_space_after_opening_and_before_closing_nonempty_braces,
        src.insert_space_after_opening_and_before_closing_nonempty_braces,
    );
    merge_tristate(
        &mut dst.insert_space_after_opening_and_before_closing_empty_braces,
        src.insert_space_after_opening_and_before_closing_empty_braces,
    );
    merge_tristate(
        &mut dst.insert_space_after_opening_and_before_closing_template_string_braces,
        src.insert_space_after_opening_and_before_closing_template_string_braces,
    );
    merge_tristate(
        &mut dst.insert_space_after_opening_and_before_closing_jsx_expression_braces,
        src.insert_space_after_opening_and_before_closing_jsx_expression_braces,
    );
    merge_tristate(
        &mut dst.insert_space_after_type_assertion,
        src.insert_space_after_type_assertion,
    );
    merge_tristate(
        &mut dst.insert_space_before_function_parenthesis,
        src.insert_space_before_function_parenthesis,
    );
    merge_tristate(
        &mut dst.place_open_brace_on_new_line_for_functions,
        src.place_open_brace_on_new_line_for_functions,
    );
    merge_tristate(
        &mut dst.place_open_brace_on_new_line_for_control_blocks,
        src.place_open_brace_on_new_line_for_control_blocks,
    );
    merge_tristate(
        &mut dst.insert_space_before_type_annotation,
        src.insert_space_before_type_annotation,
    );
    merge_tristate(
        &mut dst.indent_multi_line_object_literal_beginning_on_blank_line,
        src.indent_multi_line_object_literal_beginning_on_blank_line,
    );
    if src.semicolons != SemicolonPreference::Ignore {
        dst.semicolons = src.semicolons;
    }
    merge_tristate(&mut dst.indent_switch_case, src.indent_switch_case);
}

fn merge_inlay_hints(dst: &mut InlayHintsPreferences, src: InlayHintsPreferences) {
    if src.include_inlay_parameter_name_hints != IncludeInlayParameterNameHints::None {
        dst.include_inlay_parameter_name_hints = src.include_inlay_parameter_name_hints;
    }
    merge_tristate(
        &mut dst.include_inlay_parameter_name_hints_when_argument_matches_name,
        src.include_inlay_parameter_name_hints_when_argument_matches_name,
    );
    merge_tristate(
        &mut dst.include_inlay_function_parameter_type_hints,
        src.include_inlay_function_parameter_type_hints,
    );
    merge_tristate(
        &mut dst.include_inlay_variable_type_hints,
        src.include_inlay_variable_type_hints,
    );
    merge_tristate(
        &mut dst.include_inlay_variable_type_hints_when_type_matches_name,
        src.include_inlay_variable_type_hints_when_type_matches_name,
    );
    merge_tristate(
        &mut dst.include_inlay_property_declaration_type_hints,
        src.include_inlay_property_declaration_type_hints,
    );
    merge_tristate(
        &mut dst.include_inlay_function_like_return_type_hints,
        src.include_inlay_function_like_return_type_hints,
    );
    merge_tristate(
        &mut dst.include_inlay_enum_member_value_hints,
        src.include_inlay_enum_member_value_hints,
    );
}

fn merge_code_lens(dst: &mut CodeLensUserPreferences, src: CodeLensUserPreferences) {
    merge_tristate(
        &mut dst.references_code_lens_enabled,
        src.references_code_lens_enabled,
    );
    merge_tristate(
        &mut dst.implementations_code_lens_enabled,
        src.implementations_code_lens_enabled,
    );
    merge_tristate(
        &mut dst.references_code_lens_show_on_all_functions,
        src.references_code_lens_show_on_all_functions,
    );
    merge_tristate(
        &mut dst.implementations_code_lens_show_on_interface_methods,
        src.implementations_code_lens_show_on_interface_methods,
    );
    merge_tristate(
        &mut dst.implementations_code_lens_show_on_all_class_methods,
        src.implementations_code_lens_show_on_all_class_methods,
    );
}

fn parse_bool(v: &serde_json::Value) -> bool {
    v.as_bool().unwrap_or(false)
}

fn parse_i32(v: &serde_json::Value) -> i32 {
    if let Some(i) = v.as_i64() {
        return i as i32;
    }
    v.as_f64().unwrap_or_default() as i32
}

fn parse_string(v: &serde_json::Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn parse_string_vec(v: &serde_json::Value) -> Vec<String> {
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_tristate(v: &serde_json::Value) -> core::Tristate {
    match v.as_bool() {
        Some(true) => core::Tristate::True,
        Some(false) => core::Tristate::False,
        None => core::Tristate::Unknown,
    }
}

fn invert_tristate(v: core::Tristate) -> core::Tristate {
    match v {
        core::Tristate::True => core::Tristate::False,
        core::Tristate::False => core::Tristate::True,
        _ => core::Tristate::Unknown,
    }
}

fn parse_indent_style_json(v: &serde_json::Value) -> IndentStyle {
    if let Some(s) = v.as_str() {
        return parse_indent_style(FormatSettingValue::String(s));
    }
    if let Some(i) = v.as_i64() {
        return parse_indent_style(FormatSettingValue::Int(i as i32));
    }
    if let Some(f) = v.as_f64() {
        return parse_indent_style(FormatSettingValue::Float(f));
    }
    IndentStyle::Smart
}

fn parse_semicolon_preference_json(v: &serde_json::Value) -> SemicolonPreference {
    if let Some(s) = v.as_str() {
        return parse_semicolon_preference(FormatSettingValue::String(s));
    }
    SemicolonPreference::Ignore
}

fn parse_quote_preference(v: &serde_json::Value) -> QuotePreference {
    match v.as_str().unwrap_or_default().to_ascii_lowercase().as_str() {
        "auto" => QuotePreference::Auto,
        "double" => QuotePreference::Double,
        "single" => QuotePreference::Single,
        _ => QuotePreference::Unknown,
    }
}

fn parse_jsx_attribute_completion_style(v: &serde_json::Value) -> JsxAttributeCompletionStyle {
    match v.as_str().unwrap_or_default().to_ascii_lowercase().as_str() {
        "braces" => JsxAttributeCompletionStyle::Braces,
        "none" => JsxAttributeCompletionStyle::None,
        "" | "auto" => JsxAttributeCompletionStyle::Auto,
        _ => JsxAttributeCompletionStyle::Auto,
    }
}

fn parse_include_inlay_parameter_name_hints(
    v: &serde_json::Value,
) -> IncludeInlayParameterNameHints {
    match v.as_str().unwrap_or_default() {
        "all" => IncludeInlayParameterNameHints::All,
        "literals" => IncludeInlayParameterNameHints::Literals,
        _ => IncludeInlayParameterNameHints::None,
    }
}

fn parse_organize_imports_collation(v: &serde_json::Value) -> OrganizeImportsCollation {
    if v.as_str()
        .unwrap_or_default()
        .eq_ignore_ascii_case("unicode")
    {
        OrganizeImportsCollation::Unicode
    } else {
        OrganizeImportsCollation::Ordinal
    }
}

fn parse_organize_imports_case_first(v: &serde_json::Value) -> OrganizeImportsCaseFirst {
    match v.as_str().unwrap_or_default() {
        "lower" => OrganizeImportsCaseFirst::Lower,
        "upper" => OrganizeImportsCaseFirst::Upper,
        _ => OrganizeImportsCaseFirst::False,
    }
}

fn parse_organize_imports_type_order(v: &serde_json::Value) -> OrganizeImportsTypeOrder {
    match v.as_str().unwrap_or_default() {
        "last" => OrganizeImportsTypeOrder::Last,
        "inline" => OrganizeImportsTypeOrder::Inline,
        "first" => OrganizeImportsTypeOrder::First,
        _ => OrganizeImportsTypeOrder::Auto,
    }
}

fn parse_import_module_specifier_preference(
    v: &serde_json::Value,
) -> modulespecifiers::ImportModuleSpecifierPreference {
    match v.as_str().unwrap_or_default().to_ascii_lowercase().as_str() {
        "project-relative" => modulespecifiers::ImportModuleSpecifierPreference::ProjectRelative,
        "relative" => modulespecifiers::ImportModuleSpecifierPreference::Relative,
        "non-relative" => modulespecifiers::ImportModuleSpecifierPreference::NonRelative,
        _ => modulespecifiers::ImportModuleSpecifierPreference::Shortest,
    }
}

fn parse_import_module_specifier_ending(
    v: &serde_json::Value,
) -> modulespecifiers::ImportModuleSpecifierEndingPreference {
    match v.as_str().unwrap_or_default().to_ascii_lowercase().as_str() {
        "minimal" => modulespecifiers::ImportModuleSpecifierEndingPreference::Minimal,
        "index" => modulespecifiers::ImportModuleSpecifierEndingPreference::Index,
        "js" => modulespecifiers::ImportModuleSpecifierEndingPreference::Js,
        _ => modulespecifiers::ImportModuleSpecifierEndingPreference::Auto,
    }
}

fn parse_case_sensitivity(v: &serde_json::Value) -> core::Tristate {
    if let Some(s) = v.as_str() {
        match s.to_ascii_lowercase().as_str() {
            "caseinsensitive" => return core::Tristate::True,
            "casesensitive" => return core::Tristate::False,
            _ => {}
        }
    }
    parse_tristate(v)
}

fn serialize_tristate(v: core::Tristate) -> serde_json::Value {
    match v {
        core::Tristate::True => serde_json::Value::Bool(true),
        core::Tristate::False => serde_json::Value::Bool(false),
        _ => serde_json::Value::Null,
    }
}

fn serialize_quote_preference(v: QuotePreference) -> serde_json::Value {
    serde_json::Value::String(
        match v {
            QuotePreference::Auto => "auto",
            QuotePreference::Double => "double",
            QuotePreference::Single => "single",
            QuotePreference::Unknown => "",
        }
        .to_string(),
    )
}

fn serialize_indent_style(v: IndentStyle) -> serde_json::Value {
    serde_json::Value::Number(
        match v {
            IndentStyle::None => 0,
            IndentStyle::Block => 1,
            IndentStyle::Smart => 2,
        }
        .into(),
    )
}

fn serialize_semicolon_preference(v: SemicolonPreference) -> serde_json::Value {
    serde_json::Value::String(
        match v {
            SemicolonPreference::Ignore => "ignore",
            SemicolonPreference::Insert => "insert",
            SemicolonPreference::Remove => "remove",
        }
        .to_string(),
    )
}

fn serialize_jsx_attribute_completion_style(v: JsxAttributeCompletionStyle) -> serde_json::Value {
    serde_json::Value::String(
        match v {
            JsxAttributeCompletionStyle::Braces => "braces",
            JsxAttributeCompletionStyle::None => "none",
            JsxAttributeCompletionStyle::Auto => "auto",
            JsxAttributeCompletionStyle::Unknown => "",
        }
        .to_string(),
    )
}

fn serialize_include_inlay_parameter_name_hints(
    v: IncludeInlayParameterNameHints,
) -> serde_json::Value {
    serde_json::Value::String(
        match v {
            IncludeInlayParameterNameHints::All => "all",
            IncludeInlayParameterNameHints::Literals => "literals",
            IncludeInlayParameterNameHints::None => "",
        }
        .to_string(),
    )
}

fn serialize_organize_imports_collation(v: OrganizeImportsCollation) -> serde_json::Value {
    serde_json::Value::String(
        match v {
            OrganizeImportsCollation::Unicode => "unicode",
            OrganizeImportsCollation::Ordinal => "ordinal",
        }
        .to_string(),
    )
}

fn serialize_organize_imports_case_first(v: OrganizeImportsCaseFirst) -> serde_json::Value {
    serde_json::Value::String(
        match v {
            OrganizeImportsCaseFirst::Lower => "lower",
            OrganizeImportsCaseFirst::Upper => "upper",
            OrganizeImportsCaseFirst::False => "default",
        }
        .to_string(),
    )
}

fn serialize_organize_imports_type_order(v: OrganizeImportsTypeOrder) -> serde_json::Value {
    serde_json::Value::String(
        match v {
            OrganizeImportsTypeOrder::Last => "last",
            OrganizeImportsTypeOrder::Inline => "inline",
            OrganizeImportsTypeOrder::First => "first",
            OrganizeImportsTypeOrder::Auto => "auto",
        }
        .to_string(),
    )
}

fn serialize_import_module_specifier_preference(
    v: modulespecifiers::ImportModuleSpecifierPreference,
) -> serde_json::Value {
    serde_json::Value::String(
        match v {
            modulespecifiers::ImportModuleSpecifierPreference::ProjectRelative => {
                "project-relative"
            }
            modulespecifiers::ImportModuleSpecifierPreference::Relative => "relative",
            modulespecifiers::ImportModuleSpecifierPreference::NonRelative => "non-relative",
            modulespecifiers::ImportModuleSpecifierPreference::Shortest => "shortest",
            modulespecifiers::ImportModuleSpecifierPreference::None => "",
        }
        .to_string(),
    )
}

fn serialize_import_module_specifier_ending(
    v: modulespecifiers::ImportModuleSpecifierEndingPreference,
) -> serde_json::Value {
    serde_json::Value::String(
        match v {
            modulespecifiers::ImportModuleSpecifierEndingPreference::Minimal => "minimal",
            modulespecifiers::ImportModuleSpecifierEndingPreference::Index => "index",
            modulespecifiers::ImportModuleSpecifierEndingPreference::Js => "js",
            modulespecifiers::ImportModuleSpecifierEndingPreference::Auto => "auto",
            modulespecifiers::ImportModuleSpecifierEndingPreference::None => "",
        }
        .to_string(),
    )
}

fn serialize_string_vec(v: &[String]) -> serde_json::Value {
    serde_json::Value::Array(v.iter().cloned().map(serde_json::Value::String).collect())
}
