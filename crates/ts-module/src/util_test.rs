use super::*;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_tspath as tspath;

#[test]
fn applicable_versioned_types_key_matches_typescript_version() {
    assert!(is_applicable_versioned_types_key("types@*"));
    assert!(is_applicable_versioned_types_key("types@>=0.0.0"));
    assert!(is_applicable_versioned_types_key("types@>=7.0.0-0"));

    assert!(!is_applicable_versioned_types_key("types@<1.0.0"));
    assert!(!is_applicable_versioned_types_key("types@not-a-range"));
    assert!(!is_applicable_versioned_types_key("types"));
    assert!(!is_applicable_versioned_types_key("versions@>=0.0.0"));
}

#[test]
fn resolution_diagnostic_matches_extension_and_options() {
    let options = core::CompilerOptions {
        no_implicit_any: core::TSTrue,
        strict: core::TSFalse,
        module_resolution: core::ModuleResolutionKind::Node16,
        ..core::CompilerOptions::default()
    };

    assert_resolution_diagnostic(&options, tspath::EXTENSION_TS, false, None);
    assert_resolution_diagnostic(
        &options,
        tspath::EXTENSION_TSX,
        false,
        Some(diagnostics::Module_0_was_resolved_to_1_but_jsx_is_not_set.key()),
    );
    assert_resolution_diagnostic(
        &options,
        tspath::EXTENSION_JSX,
        false,
        Some(diagnostics::Module_0_was_resolved_to_1_but_jsx_is_not_set.key()),
    );

    let jsx_options = core::CompilerOptions {
        jsx: core::JsxEmit::Preserve,
        no_implicit_any: core::TSTrue,
        strict: core::TSFalse,
        ..core::CompilerOptions::default()
    };
    assert_resolution_diagnostic(
        &jsx_options,
        tspath::EXTENSION_JSX,
        false,
        Some(diagnostics::Could_not_find_a_declaration_file_for_module_0_1_implicitly_has_an_any_type.key()),
    );

    let allow_js_options = core::CompilerOptions {
        allow_js: core::TSTrue,
        no_implicit_any: core::TSTrue,
        strict: core::TSFalse,
        ..core::CompilerOptions::default()
    };
    assert_resolution_diagnostic(&allow_js_options, tspath::EXTENSION_JS, false, None);

    assert_resolution_diagnostic(
        &options,
        tspath::EXTENSION_JSON,
        false,
        Some(diagnostics::Module_0_was_resolved_to_1_but_resolveJsonModule_is_not_used.key()),
    );

    let resolve_json_options = core::CompilerOptions {
        resolve_json_module: core::TSTrue,
        ..core::CompilerOptions::default()
    };
    assert_resolution_diagnostic(&resolve_json_options, tspath::EXTENSION_JSON, false, None);

    assert_resolution_diagnostic(
        &options,
        ".css",
        false,
        Some(diagnostics::Module_0_was_resolved_to_1_but_allowArbitraryExtensions_is_not_set.key()),
    );
    assert_resolution_diagnostic(&options, ".css", true, None);

    let arbitrary_options = core::CompilerOptions {
        allow_arbitrary_extensions: core::TSTrue,
        ..core::CompilerOptions::default()
    };
    assert_resolution_diagnostic(&arbitrary_options, ".css", false, None);
}

fn assert_resolution_diagnostic(
    options: &core::CompilerOptions,
    extension: &str,
    file_is_declaration: bool,
    expected_key: Option<&String>,
) {
    let resolved_module = ResolvedModule {
        extension: extension.to_string(),
        ..ResolvedModule::default()
    };
    let actual = get_resolution_diagnostic(options, &resolved_module, file_is_declaration);
    assert_eq!(
        actual.map(|message| message.key()),
        expected_key,
        "unexpected diagnostic for extension {extension:?}"
    );
}
