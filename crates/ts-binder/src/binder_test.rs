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
