// package binder

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use ts_ast as ast;
use ts_ast::SymbolFlagsExt;
use ts_core as core;
use ts_parser as parser;
use ts_testutil::fixtures;
use ts_tspath as tspath;

use super::bind_parsed_source_file;

fn finish_ambient_source_file(
    mut factory: ast::NodeFactory,
    path: &str,
    statements: impl ast::IntoNodeList,
) -> ast::ParsedSourceFile {
    let end_of_file = factory.new_token(ast::Kind::EndOfFile);
    let root = factory.new_source_file(
        ast::SourceFileParseOptions {
            file_name: path.to_owned(),
            path: tspath::to_path(path, "/", true),
            ..Default::default()
        },
        "",
        statements,
        Some(end_of_file),
    );
    factory.finish_parsed_source_file_as_parsed(
        root,
        ast::ParsedSourceFileMetadata {
            script_kind: core::ScriptKind::TS,
            source_flags: ast::NodeFlags::Ambient,
            ..Default::default()
        },
    )
}

fn file_has_export_context(
    source_file: &ast::ParsedSourceFile,
    state: &crate::ProgramBindingState,
) -> bool {
    node_has_export_context(source_file.root(), source_file, state)
}

fn node_has_export_context(
    node: ast::Node,
    source_file: &ast::ParsedSourceFile,
    state: &crate::ProgramBindingState,
) -> bool {
    state
        .flags_for_node(node, source_file.store().flags(node))
        .intersects(ast::NodeFlags::ExportContext)
}

#[test]
fn bind_source_file_adds_import_equals_aliases_to_script_locals() {
    let file_name = "/aliasBug.ts".to_owned();
    let path = tspath::to_path(&file_name, "/", true);
    let source_file = parser::parse_source_file_as_parsed(
        ast::SourceFileParseOptions {
            file_name,
            path,
            ..Default::default()
        },
        "namespace foo {}\nimport provide = foo;\n".to_owned(),
        core::ScriptKind::TS,
    );

    let state = bind_parsed_source_file(&source_file);
    let provide = state
        .lookup_local(source_file.root(), "provide")
        .expect("script import-equals alias should be in source file locals");
    assert!(
        state
            .symbol_flags(provide)
            .intersects(ast::SYMBOL_FLAGS_ALIAS)
    );
}

#[test]
fn bind_source_file_export_context_should_use_top_level_export_declarations() {
    let mut no_export_factory = ast::NodeFactory::default();
    let no_export_statements = no_export_factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        std::iter::empty::<ast::Node>(),
    );
    let no_export_file = finish_ambient_source_file(
        no_export_factory,
        "/ambient-no-export.d.ts",
        no_export_statements,
    );
    let no_export_state = bind_parsed_source_file(&no_export_file);
    assert!(file_has_export_context(&no_export_file, &no_export_state));

    let mut export_factory = ast::NodeFactory::default();
    let export_declaration = export_factory.new_export_declaration(
        None::<ast::ModifierList>,
        false,
        None::<ast::Node>,
        None::<ast::Node>,
        None::<ast::Node>,
    );
    let export_statements = export_factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        [export_declaration],
    );
    let export_file =
        finish_ambient_source_file(export_factory, "/ambient-export.d.ts", export_statements);
    let export_state = bind_parsed_source_file(&export_file);
    assert!(!file_has_export_context(&export_file, &export_state));

    let mut namespace_factory = ast::NodeFactory::default();
    let namespace_name = namespace_factory.new_identifier("globalName");
    let namespace_export = namespace_factory
        .new_namespace_export_declaration(None::<ast::ModifierList>, Some(namespace_name));
    let namespace_statements = namespace_factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        [namespace_export],
    );
    let namespace_file = finish_ambient_source_file(
        namespace_factory,
        "/ambient-namespace-export.d.ts",
        namespace_statements,
    );
    let namespace_state = bind_parsed_source_file(&namespace_file);
    assert!(file_has_export_context(&namespace_file, &namespace_state));
}

#[test]
fn bind_ambient_module_export_context_should_use_module_body_export_declarations() {
    let no_export_file = parse_ambient_module_source(
        "/ambient-module-no-export.d.ts",
        "declare module 'ambient' {}",
    );
    let no_export_module = no_export_file.statements_view().first().unwrap();
    let no_export_state = bind_parsed_source_file(&no_export_file);
    assert!(node_has_export_context(
        no_export_module,
        &no_export_file,
        &no_export_state
    ));

    let export_file = parse_ambient_module_source(
        "/ambient-module-export.d.ts",
        "declare module 'ambient' { export {}; }",
    );
    let export_module = export_file.statements_view().first().unwrap();
    let export_state = bind_parsed_source_file(&export_file);
    assert!(!node_has_export_context(
        export_module,
        &export_file,
        &export_state
    ));

    let namespace_file = parse_ambient_module_source(
        "/ambient-module-namespace-export.d.ts",
        "declare module 'ambient' { export as namespace globalName; }",
    );
    let namespace_module = namespace_file.statements_view().first().unwrap();
    let namespace_state = bind_parsed_source_file(&namespace_file);
    assert!(node_has_export_context(
        namespace_module,
        &namespace_file,
        &namespace_state
    ));
}

fn parse_ambient_module_source(path: &str, text: &str) -> ast::ParsedSourceFile {
    parser::parse_source_file_as_parsed(
        ast::SourceFileParseOptions {
            file_name: path.to_owned(),
            path: tspath::to_path(path, "/", true),
            ..Default::default()
        },
        text.to_owned(),
        core::ScriptKind::TS,
    )
}

#[test]
fn bind_source_file_adds_top_level_js_type_aliases_to_script_locals() {
    let mut factory = ast::NodeFactory::default();
    let name = factory.new_identifier("NumberLike");
    let typedef = factory.new_js_type_alias_declaration(
        None::<ast::ModifierList>,
        Some(name),
        None::<ast::NodeList>,
        None::<ast::Node>,
    );
    let statements = factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        [typedef],
    );
    let end_of_file = factory.new_token(ast::Kind::EndOfFile);
    let root = factory.new_source_file(
        ast::SourceFileParseOptions {
            file_name: "/typedef.js".to_owned(),
            path: tspath::to_path("/typedef.js", "/", true),
            ..Default::default()
        },
        "",
        statements,
        Some(end_of_file),
    );
    let source_file = factory.finish_parsed_source_file_as_parsed(
        root,
        ast::ParsedSourceFileMetadata {
            script_kind: core::ScriptKind::JS,
            source_flags: ast::NodeFlags::JAVA_SCRIPT_FILE,
            ..Default::default()
        },
    );

    let state = bind_parsed_source_file(&source_file);
    let typedef = state
        .lookup_local(source_file.root(), "NumberLike")
        .expect("top-level JS typedef should be in source file locals");

    assert!(
        state
            .symbol_flags(typedef)
            .intersects(ast::SYMBOL_FLAGS_TYPE_ALIAS)
    );
}

#[test]
fn bind_parsed_source_file_returns_program_binding_state_without_mutating_ast() {
    let file_name = "/parsedAlias.ts".to_owned();
    let path = tspath::to_path(&file_name, "/", true);
    let parsed = parser::parse_source_file_as_parsed(
        ast::SourceFileParseOptions {
            file_name,
            path,
            ..Default::default()
        },
        "namespace foo {}\nimport provide = foo;\n".to_owned(),
        core::ScriptKind::TS,
    );

    let state = bind_parsed_source_file(&parsed);
    let provide = state
        .lookup_local(parsed.root(), "provide")
        .expect("script import-equals alias should be in source file locals");

    assert!(
        state
            .symbol_flags(provide)
            .intersects(ast::SYMBOL_FLAGS_ALIAS)
    );
}

#[test]
fn bind_parsed_source_file_reuses_bind_once_state_for_shared_source_file() {
    let file_name = "/bindOnce.ts".to_owned();
    let path = tspath::to_path(&file_name, "/", true);
    let parsed = parser::parse_source_file_as_parsed(
        ast::SourceFileParseOptions {
            file_name,
            path,
            ..Default::default()
        },
        "namespace foo {}\nimport provide = foo;\n".to_owned(),
        core::ScriptKind::TS,
    );
    let shared = parsed.share_readonly();

    let first = bind_parsed_source_file(&parsed);
    let second = bind_parsed_source_file(&shared);

    assert!(Arc::ptr_eq(&first, &second));
}

#[expect(
    dead_code,
    reason = "manual benchmark helper is not called by normal tests"
)]
fn benchmark_bind_iterations(n: usize) -> Duration {
    let mut elapsed = Duration::ZERO;

    for f in fixtures::bench_fixtures() {
        f.skip_if_not_exist();

        let file_name = tspath::get_normalized_absolute_path(&f.path().to_string_lossy(), "/");
        let path = tspath::to_path(&file_name, "/", true);
        let source_text = f.read_file();

        let parse_options = ast::SourceFileParseOptions {
            file_name: file_name.clone(),
            path,
            ..Default::default()
        };
        let script_kind = core::get_script_kind_from_file_name(&file_name);

        let mut source_files = Vec::with_capacity(n);
        for _ in 0..n {
            source_files.push(parser::parse_source_file_as_parsed(
                parse_options.clone(),
                source_text.clone(),
                script_kind,
            ));
        }

        // The above parses do a lot of work; keep them outside the measured bind section.
        let start = Instant::now();
        for source_file in &source_files {
            let _ = bind_parsed_source_file(source_file);
        }
        elapsed += start.elapsed();
    }

    elapsed
}
