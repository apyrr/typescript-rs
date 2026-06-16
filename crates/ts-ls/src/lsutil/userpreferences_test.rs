use std::collections::HashMap;
use std::io::Cursor;

use ts_core as core;
use ts_json as json;
use ts_modulespecifiers as modulespecifiers;

use super::{
    CodeLensUserPreferences, IncludeInlayParameterNameHints, InlayHintsPreferences,
    JsxAttributeCompletionStyle, OrganizeImportsCaseFirst, OrganizeImportsCollation,
    OrganizeImportsTypeOrder, QuotePreference, UserPreferences, new_default_user_preferences,
    parse_user_preferences,
};

fn fill_non_zero_values(prefs: &mut UserPreferences) {
    *prefs = UserPreferences {
        quote_preference: QuotePreference::Single,
        lazy_configured_projects_from_external_project: core::Tristate::True,
        maximum_hover_length: 1,
        include_completions_for_module_exports: core::Tristate::True,
        include_completions_for_import_statements: core::Tristate::True,
        include_automatic_optional_chain_completions: core::Tristate::True,
        include_completions_with_class_member_snippets: core::Tristate::True,
        include_completions_with_object_literal_method_snippets: core::Tristate::True,
        jsx_attribute_completion_style: JsxAttributeCompletionStyle::Braces,
        import_module_specifier_preference:
            modulespecifiers::ImportModuleSpecifierPreference::Relative,
        import_module_specifier_ending: modulespecifiers::ImportModuleSpecifierEndingPreference::Js,
        auto_import_specifier_exclude_regexes: vec!["test".to_string()],
        auto_import_file_exclude_patterns: vec!["test".to_string()],
        auto_import_entrypoint_directory_search: core::Tristate::True,
        prefer_type_only_auto_imports: core::Tristate::True,
        organize_imports_ignore_case: core::Tristate::True,
        organize_imports_collation: OrganizeImportsCollation::Unicode,
        organize_imports_locale: "test".to_string(),
        organize_imports_numeric_collation: core::Tristate::True,
        organize_imports_accent_collation: core::Tristate::True,
        organize_imports_case_first: OrganizeImportsCaseFirst::Lower,
        organize_imports_type_order: OrganizeImportsTypeOrder::First,
        allow_text_changes_in_new_files: core::Tristate::True,
        use_aliases_for_rename: core::Tristate::True,
        allow_rename_of_import_path: core::Tristate::True,
        provide_refactor_not_applicable_reason: core::Tristate::True,
        inlay_hints: InlayHintsPreferences {
            include_inlay_parameter_name_hints: IncludeInlayParameterNameHints::All,
            include_inlay_parameter_name_hints_when_argument_matches_name: core::Tristate::True,
            include_inlay_function_parameter_type_hints: core::Tristate::True,
            include_inlay_variable_type_hints: core::Tristate::True,
            include_inlay_variable_type_hints_when_type_matches_name: core::Tristate::True,
            include_inlay_property_declaration_type_hints: core::Tristate::True,
            include_inlay_function_like_return_type_hints: core::Tristate::True,
            include_inlay_enum_member_value_hints: core::Tristate::True,
        },
        code_lens: CodeLensUserPreferences {
            references_code_lens_enabled: core::Tristate::True,
            implementations_code_lens_enabled: core::Tristate::True,
            references_code_lens_show_on_all_functions: core::Tristate::True,
            implementations_code_lens_show_on_interface_methods: core::Tristate::True,
            implementations_code_lens_show_on_all_class_methods: core::Tristate::True,
        },
        prefer_go_to_source_definition: true,
        exclude_library_symbols_in_nav_to: core::Tristate::True,
        disable_suggestions: core::Tristate::True,
        disable_line_text_in_references: core::Tristate::True,
        report_style_checks_as_warnings: core::Tristate::True,
        disable_automatic_type_acquisition: core::Tristate::True,
        automatic_type_acquisition_enabled: core::Tristate::True,
        custom_config_file_name: "test".to_string(),
        ..new_default_user_preferences()
    };
}

#[test]
fn test_user_preferences_roundtrip() {
    let mut original = UserPreferences::default();
    fill_non_zero_values(&mut original);
    let json_bytes = marshal_user_preferences(&original);

    {
        let mut parsed = UserPreferences::default();
        let mut dec = json::new_decoder(Cursor::new(json_bytes.as_slice()));
        parsed.unmarshal_json_from(&mut dec).unwrap();
        assert_eq!(parsed, original, "UnmarshalJSONFrom");
    }

    {
        let config = parse_json_bytes(&json_bytes);
        let parsed = UserPreferences::default().with_config(&config);
        assert_eq!(parsed, original, "withConfig");
    }
}

#[test]
fn test_user_preferences_serialize() {
    {
        let name = "config path field serializes to nested path";
        let prefs = UserPreferences {
            quote_preference: QuotePreference::Single,
            ..Default::default()
        };
        let json_bytes = marshal_user_preferences(&prefs);
        let actual = parse_json_bytes(&json_bytes);

        let preferences = actual["preferences"].as_object().unwrap();
        assert_eq!(preferences["quoteStyle"], "single", "{name}");
    }

    {
        let name = "raw-only field serializes to unstable section";
        let prefs = UserPreferences {
            disable_suggestions: core::Tristate::True,
            ..Default::default()
        };
        let json_bytes = marshal_user_preferences(&prefs);
        let actual = parse_json_bytes(&json_bytes);

        let unstable = actual["unstable"].as_object().unwrap();
        assert_eq!(unstable["disableSuggestions"], true, "{name}");
    }

    {
        let name = "inlay hint inversion on serialize";
        let prefs = UserPreferences {
            inlay_hints: InlayHintsPreferences {
                include_inlay_parameter_name_hints: IncludeInlayParameterNameHints::All,
                include_inlay_parameter_name_hints_when_argument_matches_name: core::Tristate::True,
                ..Default::default()
            },
            ..Default::default()
        };
        let json_bytes = marshal_user_preferences(&prefs);
        let actual = parse_json_bytes(&json_bytes);

        let inlay_hints = actual["inlayHints"].as_object().unwrap();
        let parameter_names = inlay_hints["parameterNames"].as_object().unwrap();
        assert_eq!(parameter_names["enabled"], "all", "{name}");
        assert_eq!(
            parameter_names["suppressWhenArgumentMatchesName"], false,
            "{name}"
        );
    }

    {
        let name = "mixed config and unstable fields";
        let prefs = UserPreferences {
            quote_preference: QuotePreference::Single,
            disable_suggestions: core::Tristate::True,
            ..Default::default()
        };
        let json_bytes = marshal_user_preferences(&prefs);
        let actual = parse_json_bytes(&json_bytes);

        let preferences = actual["preferences"].as_object().unwrap();
        assert_eq!(preferences["quoteStyle"], "single", "{name}");

        let unstable = actual["unstable"].as_object().unwrap();
        assert_eq!(unstable["disableSuggestions"], true, "{name}");
    }
}

#[test]
fn test_user_preferences_parse_unstable() {
    let tests = [
        (
            "unstable fields with correct casing",
            r#"{
                "unstable": {
                    "disableSuggestions": true,
                    "maximumHoverLength": 100,
                    "allowRenameOfImportPath": true
                }
            }"#,
            UserPreferences {
                disable_suggestions: core::Tristate::True,
                maximum_hover_length: 100,
                allow_rename_of_import_path: core::Tristate::True,
                ..Default::default()
            },
        ),
        (
            "nested preferences path",
            r#"{
                "preferences": {
                    "quoteStyle": "single",
                    "useAliasesForRenames": true
                }
            }"#,
            UserPreferences {
                quote_preference: QuotePreference::Single,
                use_aliases_for_rename: core::Tristate::True,
                ..Default::default()
            },
        ),
        (
            "suggest section",
            r#"{
                "suggest": {
                    "autoImports": false,
                    "includeCompletionsForImportStatements": true
                }
            }"#,
            UserPreferences {
                include_completions_for_module_exports: core::Tristate::False,
                include_completions_for_import_statements: core::Tristate::True,
                ..Default::default()
            },
        ),
        (
            "inlayHints with invert",
            r#"{
                "inlayHints": {
                    "parameterNames": {
                        "enabled": "all",
                        "suppressWhenArgumentMatchesName": true
                    }
                }
            }"#,
            UserPreferences {
                inlay_hints: InlayHintsPreferences {
                    include_inlay_parameter_name_hints: IncludeInlayParameterNameHints::All,
                    include_inlay_parameter_name_hints_when_argument_matches_name:
                        core::Tristate::False,
                    ..Default::default()
                },
                ..Default::default()
            },
        ),
        (
            "mixed config",
            r#"{
                "preferences": {
                    "importModuleSpecifier": "relative"
                },
                "workspaceSymbols": {
                    "excludeLibrarySymbols": true
                }
            }"#,
            UserPreferences {
                import_module_specifier_preference:
                    modulespecifiers::ImportModuleSpecifierPreference::Relative,
                exclude_library_symbols_in_nav_to: core::Tristate::True,
                ..Default::default()
            },
        ),
        (
            "stable config overrides unstable",
            r#"{
                "unstable": {
                    "quotePreference": "double"
                },
                "preferences": {
                    "quoteStyle": "single"
                }
            }"#,
            UserPreferences {
                quote_preference: QuotePreference::Single,
                ..Default::default()
            },
        ),
        (
            "unstable sets value when no stable config",
            r#"{
                "unstable": {
                    "includeAutomaticOptionalChainCompletions": false
                }
            }"#,
            UserPreferences {
                include_automatic_optional_chain_completions: core::Tristate::False,
                ..Default::default()
            },
        ),
        (
            "any field can be passed via unstable by its raw name",
            r#"{
                "unstable": {
                    "quotePreference": "double",
                    "includeCompletionsForModuleExports": true,
                    "excludeLibrarySymbolsInNavTo": true
                }
            }"#,
            UserPreferences {
                quote_preference: QuotePreference::Double,
                include_completions_for_module_exports: core::Tristate::True,
                exclude_library_symbols_in_nav_to: core::Tristate::True,
                ..Default::default()
            },
        ),
        (
            "TypeScript raw names work in unstable section",
            r#"{
                "unstable": {
                    "includeCompletionsForModuleExports": true,
                    "quotePreference": "single",
                    "providePrefixAndSuffixTextForRename": true,
                    "includeInlayParameterNameHints": "all",
                    "organizeImportsLocale": "en"
                }
            }"#,
            UserPreferences {
                include_completions_for_module_exports: core::Tristate::True,
                quote_preference: QuotePreference::Single,
                use_aliases_for_rename: core::Tristate::True,
                organize_imports_locale: "en".to_string(),
                inlay_hints: InlayHintsPreferences {
                    include_inlay_parameter_name_hints: IncludeInlayParameterNameHints::All,
                    ..Default::default()
                },
                ..Default::default()
            },
        ),
    ];

    for (name, input, expected) in tests {
        let parsed = parse_json_config(input);
        assert_eq!(
            UserPreferences::default().with_config(&parsed),
            expected,
            "{name}"
        );
    }
}

#[test]
fn test_user_preferences_parse_ata() {
    let name = "ParseUserPreferences with unified ATA setting in js/ts section";
    let prefs = parse_user_preferences(HashMap::from([(
        "js/ts".to_string(),
        serde_json::json!({
            "tsserver": { "automaticTypeAcquisition": { "enabled": false } }
        }),
    )]));
    assert!(prefs.is_ata_disabled(), "{name}");
    assert_eq!(
        prefs.automatic_type_acquisition_enabled,
        core::Tristate::False,
        "{name}"
    );

    let name = "ParseUserPreferences with deprecated disableAutomaticTypeAcquisition in typescript section";
    let prefs = parse_user_preferences(HashMap::from([(
        "typescript".to_string(),
        serde_json::json!({
            "disableAutomaticTypeAcquisition": true
        }),
    )]));
    assert!(prefs.is_ata_disabled(), "{name}");
    assert_eq!(
        prefs.disable_automatic_type_acquisition,
        core::Tristate::True,
        "{name}"
    );

    let name = "unified setting takes precedence over deprecated setting";
    let prefs = parse_user_preferences(HashMap::from([
        (
            "typescript".to_string(),
            serde_json::json!({
                "disableAutomaticTypeAcquisition": true
            }),
        ),
        (
            "js/ts".to_string(),
            serde_json::json!({
                "tsserver": { "automaticTypeAcquisition": { "enabled": true } }
            }),
        ),
    ]));
    assert!(!prefs.is_ata_disabled(), "{name}");
    assert_eq!(
        prefs.automatic_type_acquisition_enabled,
        core::Tristate::True,
        "{name}"
    );

    let name = "IsATADisabled returns false when neither setting is configured";
    let prefs = new_default_user_preferences();
    assert!(!prefs.is_ata_disabled(), "{name}");
}

fn parse_json_config(input: &str) -> HashMap<String, serde_json::Value> {
    serde_json::from_str(input).unwrap_or_default()
}

fn parse_json_bytes(input: &[u8]) -> HashMap<String, serde_json::Value> {
    serde_json::from_slice(input).unwrap()
}

fn marshal_user_preferences(prefs: &UserPreferences) -> Vec<u8> {
    let mut json_bytes = Vec::new();
    {
        let mut enc = json::Encoder::new(&mut json_bytes);
        prefs.marshal_json_to(&mut enc).unwrap();
    }
    json_bytes
}
