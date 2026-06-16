// package parser_test

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ts_ast as ast;
use ts_core as core;
use ts_repo as repo;
use ts_testrunner as testrunner;
use ts_testutil::fixtures;
use ts_tspath as tspath;
use ts_vfs::{osvfs, vfs::Fs};

#[test]
fn parse_source_file_collects_type_only_empty_import_specifiers() {
    let opts = ast::SourceFileParseOptions {
        file_name: "/project/main.ts".to_string(),
        path: "/project/main.ts".to_string(),
        ..Default::default()
    };
    let source = r#"
import {} from "./value";
import type {} from "./type";
"#;

    let file = super::parse_source_file(opts, source.to_string(), core::ScriptKind::TS);
    let store = file.store();
    let imports = file
        .imports()
        .iter()
        .map(|module_specifier| store.text(*module_specifier))
        .collect::<Vec<_>>();

    assert_eq!(imports, vec!["./value", "./type"]);
}

#[test]
fn parse_declaration_file_collects_all_empty_import_specifiers() {
    let opts = ast::SourceFileParseOptions {
        file_name: "/project/types.d.ts".to_string(),
        path: "/project/types.d.ts".to_string(),
        ..Default::default()
    };
    let source = r#"
import {} from "./a.ts";
import {} from "./a.d.ts";
import type {} from "./a.d.ts";
"#;

    let file = super::parse_source_file(opts, source.to_string(), core::ScriptKind::TS);
    let store = file.store();
    let imports = file
        .imports()
        .iter()
        .map(|module_specifier| store.text(*module_specifier))
        .collect::<Vec<_>>();

    assert_eq!(imports, vec!["./a.ts", "./a.d.ts", "./a.d.ts"]);
}

#[test]
fn parse_source_file_preserves_single_quote_token_flags_for_literal_types() {
    let opts = ast::SourceFileParseOptions {
        file_name: "/project/main.ts".to_string(),
        path: "/project/main.ts".to_string(),
        ..Default::default()
    };
    let source = "type A = { kind: 'a' };";

    let file = super::parse_source_file(opts, source.to_string(), core::ScriptKind::TS);
    let store = file.store();
    let root = file.root();
    let statement = store
        .parser_access()
        .source_file_statement_nodes(root)
        .first()
        .copied()
        .expect("source file should have a statement");
    let type_literal = store
        .r#type(statement)
        .expect("type alias should have a type");
    let property = store
        .members(type_literal)
        .expect("type literal should have members")
        .first()
        .expect("type literal should have a member");
    let literal_type = store
        .r#type(property)
        .expect("property signature should have a type");
    let literal = store
        .literal(literal_type)
        .expect("literal type should have a literal");

    assert_eq!(store.kind(literal), ast::Kind::StringLiteral);
    assert!(
        store
            .token_flags(literal)
            .expect("string literal should have token flags")
            .contains(ast::TokenFlags::SINGLE_QUOTE)
    );
}

#[test]
fn parse_source_file_normalizes_numeric_class_property_names() {
    let opts = ast::SourceFileParseOptions {
        file_name: "/project/main.ts".to_string(),
        path: "/project/main.ts".to_string(),
        ..Default::default()
    };
    let source = "class C { 0.0 = 1; }";

    let file = super::parse_source_file(opts, source.to_string(), core::ScriptKind::TS);
    let store = file.store();
    let root = file.root();
    let class_decl = store
        .parser_access()
        .source_file_statement_nodes(root)
        .first()
        .copied()
        .expect("source file should have a statement");
    let member = store
        .members(class_decl)
        .expect("class should have members")
        .first()
        .expect("class should have a member");
    let name = store.name(member).expect("class member should have a name");

    assert_eq!(store.kind(name), ast::Kind::NumericLiteral);
    assert_eq!(store.text(name), "0");
}

#[test]
fn parse_source_file_reports_octal_literals_and_escape_sequences() {
    if repo::skip_if_no_type_script_submodule() {
        return;
    }

    let opts = ast::SourceFileParseOptions {
        file_name: "/project/octalLiteralAndEscapeSequence.ts".to_string(),
        path: "/project/octalLiteralAndEscapeSequence.ts".to_string(),
        ..Default::default()
    };
    let source = read_source_text(
        repo::type_script_submodule_path()
            .join("tests/cases/compiler/octalLiteralAndEscapeSequence.ts")
            .to_str()
            .expect("test path should be UTF-8"),
    );

    let file = super::parse_source_file(opts, source, core::ScriptKind::TS);
    let diagnostics = file.diagnostics();

    assert_eq!(diagnostics.len(), 109);
    assert_eq!(diagnostics[0].code(), 1121);
    assert_eq!(diagnostics[0].message_args(), &["0o0".to_string()]);
    assert_eq!(diagnostics[15].code(), 1487);
    assert_eq!(diagnostics[15].message_args(), &[r"\x05".to_string()]);
    assert_eq!(diagnostics[47].code(), 1488);
    assert_eq!(diagnostics[47].message_args(), &[r"\8".to_string()]);
    assert_eq!(diagnostics[108].code(), 1487);
    assert_eq!(diagnostics[108].message_args(), &[r"\x2d".to_string()]);
}

#[test]
fn parse_source_file_collects_external_module_augmentation() {
    let opts = ast::SourceFileParseOptions {
        file_name: "/project/main.ts".to_string(),
        path: "/project/main.ts".to_string(),
        ..Default::default()
    };
    let source = r#"
import x = require("./file1");
declare module "./file1" {
    interface A { a }
}
"#;

    let file = super::parse_source_file(opts, source.to_string(), core::ScriptKind::TS);
    let store = file.store();
    let augmentations = file
        .module_augmentations()
        .iter()
        .map(|module_specifier| store.text(*module_specifier))
        .collect::<Vec<_>>();
    let imports = file
        .imports()
        .iter()
        .map(|module_specifier| store.text(*module_specifier))
        .collect::<Vec<_>>();

    assert_eq!(
        file.external_module_indicator()
            .map(|node| store.kind(node)),
        Some(ast::Kind::ImportEqualsDeclaration)
    );
    assert_eq!(imports, vec!["./file1"]);
    assert_eq!(augmentations, vec!["./file1"]);
}

fn benchmark_parse_iterations(n: usize) -> Duration {
    let mut elapsed = Duration::ZERO;

    for f in fixtures::bench_fixtures() {
        f.skip_if_not_exist();

        let file_name = tspath::get_normalized_absolute_path(
            f.path().to_str().expect("fixture path should be UTF-8"),
            "/",
        );
        let path = tspath::to_path(
            &file_name,
            "/",
            osvfs::os::fs().use_case_sensitive_file_names(),
        );
        let source_text: Arc<str> = f.read_file().into();
        let script_kind = core::get_script_kind_from_file_name(&file_name);

        let opts = ast::SourceFileParseOptions {
            file_name,
            path,
            ..Default::default()
        };

        let start = Instant::now();
        for _ in 0..n {
            super::parse_source_file(opts.clone(), Arc::clone(&source_text), script_kind);
        }
        elapsed += start.elapsed();
    }

    elapsed
}

#[expect(
    dead_code,
    reason = "manual benchmark helper is not called by normal tests"
)]
fn benchmark_parse() {
    benchmark_parse_iterations(1);
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsableFile {
    path: String,
    name: String,
}

struct AllParsableFiles {
    root: PathBuf,
    pending: Vec<PathBuf>,
}

fn all_parsable_files(root: impl AsRef<Path>) -> AllParsableFiles {
    let root = root.as_ref().to_owned();
    AllParsableFiles {
        pending: vec![root.clone()],
        root,
    }
}

impl Iterator for AllParsableFiles {
    type Item = ParsableFile;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(path) = self.pending.pop() {
            let metadata = fs::symlink_metadata(&path).unwrap_or_else(|err| {
                panic!("failed to stat {}: {err}", path.display());
            });

            if metadata.is_dir() {
                let mut entries = fs::read_dir(&path)
                    .unwrap_or_else(|err| {
                        panic!("failed to read directory {}: {err}", path.display())
                    })
                    .map(|entry| {
                        entry
                            .unwrap_or_else(|err| panic!("failed to read directory entry: {err}"))
                            .path()
                    })
                    .collect::<Vec<_>>();
                entries.sort();
                self.pending.extend(entries.into_iter().rev());
                continue;
            }

            let path_string = path.to_str().expect("test path should be UTF-8");
            if tspath::try_get_extension_from_path(path_string).is_empty() {
                continue;
            }

            let name = if path == self.root {
                ".".to_string()
            } else {
                path.strip_prefix(&self.root)
                    .unwrap_or_else(|err| {
                        panic!(
                            "failed to make {} relative to {}: {err}",
                            path.display(),
                            self.root.display()
                        );
                    })
                    .components()
                    .map(|component| {
                        component
                            .as_os_str()
                            .to_str()
                            .expect("test path should be UTF-8")
                    })
                    .collect::<Vec<_>>()
                    .join("/")
            };

            return Some(ParsableFile {
                path: path_string.to_string(),
                name,
            });
        }

        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TestFile {
    content: String,
    name: String,
}

fn fuzz_parser_case(
    extensions: &HashSet<&'static str>,
    extension: &str,
    source_text: String,
    external_module_indicator_options_jsx: bool,
    external_module_indicator_options_force: bool,
) {
    if !extensions.contains(extension) {
        return;
    }

    let file_name = format!("/index{extension}");
    let path = tspath::Path::from(file_name.clone());

    let opts = ast::SourceFileParseOptions {
        file_name: file_name.clone(),
        path,
        external_module_indicator_options: ast::ExternalModuleIndicatorOptions {
            jsx: external_module_indicator_options_jsx,
            force: external_module_indicator_options_force,
        },
    };

    super::parse_source_file(
        opts,
        source_text,
        core::get_script_kind_from_file_name(&file_name),
    );
}

fn read_source_text(path: &str) -> String {
    let source_text = fs::read(path).unwrap_or_else(|err| panic!("failed to read {path}: {err}"));
    String::from_utf8_lossy(&source_text).into_owned()
}

#[test]
fn fuzz_parser() {
    if repo::skip_if_no_type_script_submodule() {
        return;
    }

    let tests = ["src", "scripts", "Herebyfile.mjs"];

    let mut extensions = HashSet::new();
    for es in tspath::all_supported_extensions_with_json() {
        for e in es {
            extensions.insert(e);
        }
    }

    let mut seeds = Vec::new();
    for test in tests {
        let root = repo::type_script_submodule_path().join(test);

        for file in all_parsable_files(root) {
            let source_text = read_source_text(&file.path);
            let extension = tspath::try_get_extension_from_path(&file.path);
            seeds.push((extension, source_text, false, false));
        }
    }

    let test_dirs = [
        repo::type_script_submodule_path().join("tests/cases/compiler"),
        repo::type_script_submodule_path().join("tests/cases/conformance"),
        repo::test_data_path().join("tests/cases/compiler"),
    ];

    for test_dir in test_dirs {
        if !test_dir.exists() {
            continue;
        }

        for file in all_parsable_files(test_dir) {
            let source_text = read_source_text(&file.path);

            let (test_units, _, _, _, parse_error) = testrunner::parse_test_files_and_symlinks(
                &source_text,
                &file.path,
                |filename, content, _file_options| {
                    Ok(TestFile {
                        content,
                        name: filename,
                    })
                },
            );
            assert!(parse_error.is_none(), "{:?}", parse_error);

            for unit in test_units {
                let extension = tspath::try_get_extension_from_path(&unit.name);
                if extension.is_empty() {
                    continue;
                }
                seeds.push((extension, unit.content, false, false));
            }
        }
    }

    for (
        extension,
        source_text,
        external_module_indicator_options_jsx,
        external_module_indicator_options_force,
    ) in seeds
    {
        fuzz_parser_case(
            &extensions,
            extension,
            source_text,
            external_module_indicator_options_jsx,
            external_module_indicator_options_force,
        );
    }
}

#[test]
fn parse_isolated_entity_name_accepts_dotted_identifier_names() {
    let parsed = super::parse_isolated_entity_name("React.createElement")
        .expect("dotted entity name should parse");
    let store = &parsed.store;
    let entity = parsed.node;

    assert_eq!(store.kind(entity), ast::Kind::QualifiedName);
    assert_eq!(store.loc(entity).pos(), 0);
    assert_eq!(store.loc(entity).end(), 19);
    assert_eq!(
        store.text(store.left(entity).expect("qualified left")),
        "React"
    );
    assert_eq!(
        store.text(store.right(entity).expect("qualified right")),
        "createElement"
    );
}

#[test]
fn parse_isolated_entity_name_rejects_extra_input() {
    assert!(super::parse_isolated_entity_name("React.createElement()").is_none());
    assert!(super::parse_isolated_entity_name("1.React").is_none());
    assert!(super::parse_isolated_entity_name("React.").is_none());
}

#[test]
fn get_comment_pragmas_reads_leading_directives() {
    let mut factory = ast::new_node_factory(ast::NodeFactoryHooks::default());
    let pragmas = crate::parser::get_comment_pragmas(
        &mut factory,
        "/// <reference path=\"lib.d.ts\" />\n// @ts-check\nconst x = 1;",
    );

    assert_eq!(pragmas.len(), 2);
    assert_eq!(pragmas[0].name, "reference");
    assert_eq!(pragmas[0].args["path"].value, "lib.d.ts");
    assert_eq!(pragmas[1].name, "ts-check");
}

#[test]
fn get_comment_pragmas_reads_leading_directives_after_shebang() {
    let mut factory = ast::new_node_factory(ast::NodeFactoryHooks::default());
    let pragmas = crate::parser::get_comment_pragmas(
        &mut factory,
        "#!/usr/bin/env node\n\n/// <reference path=\"lib.d.ts\" />\nconst x = 1;",
    );

    assert_eq!(pragmas.len(), 1);
    assert_eq!(pragmas[0].name, "reference");
    assert_eq!(pragmas[0].args["path"].value, "lib.d.ts");
}

#[test]
fn get_comment_pragmas_reads_jsx_multiline_directives() {
    let mut factory = ast::new_node_factory(ast::NodeFactoryHooks::default());
    let pragmas = crate::parser::get_comment_pragmas(
        &mut factory,
        "/* @jsx h\n * @jsxfrag Fragment */\nconst element = <div />;",
    );

    assert_eq!(pragmas.len(), 2);
    assert_eq!(pragmas[0].name, "jsx");
    assert_eq!(pragmas[0].args["factory"].value, "h");
    assert_eq!(pragmas[1].name, "jsxfrag");
    assert_eq!(pragmas[1].args["factory"].value, "Fragment");
}

#[test]
fn parse_source_file_sets_source_metadata() {
    let opts = ast::SourceFileParseOptions {
        file_name: "/index.d.ts".to_string(),
        path: "/index.d.ts".to_string(),
        ..Default::default()
    };
    let file = super::parse_source_file(
        opts,
        "/// <reference path=\"lib.d.ts\" />\nconst π = 1;".to_string(),
        core::ScriptKind::TS,
    );

    assert!(file.is_declaration_file());
    assert!(file.contains_non_ascii());
    assert_eq!(file.script_kind(), core::ScriptKind::TS);
    assert_eq!(file.language_variant(), core::LanguageVariant::Standard);
    assert_eq!(file.pragmas().len(), 1);
    assert_eq!(file.pragmas()[0].args["path"].value, "lib.d.ts");
    assert!(file.data().end_of_file_token().is_some());
}
