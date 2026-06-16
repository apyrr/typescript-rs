use std::collections::HashMap;
use std::sync::Arc;

#[cfg(feature = "nightly_bench")]
extern crate test;

use ts_core as core;
use ts_lsproto as lsproto;
use ts_tspath as tspath;

use crate::{self as project, ProgramUpdateKind};

const LANGUAGE_KIND_TYPESCRIPT: &str = "typescript";

fn setup(files: HashMap<String, String>) -> project::Session {
    let fs = ts_bundled::wrap_fs(ts_vfs::vfstest::from_map(files, false));
    project::new_session(project::SessionInit {
        background_ctx: core::Context::default(),
        options: project::SessionOptions {
            current_directory: "/".to_string(),
            default_library_path: ts_bundled::lib_path(),
            typings_location: "/home/src/Library/Caches/typescript".to_string(),
            position_encoding: lsproto::PositionEncodingKind::UTF8,
            watch_enabled: false,
            logging_enabled: false,
            telemetry_enabled: false,
            push_diagnostics_enabled: true,
            debounce_delay: Default::default(),
            locale: Default::default(),
        },
        fs: Arc::new(fs),
        client: None,
        logger: Arc::new(project::logging::new_test_logger()),
        npm_executor: Default::default(),
        parse_cache: None,
    })
}

fn partial_change(
    text: &str,
    start_line: u32,
    start_character: u32,
    end_line: u32,
    end_character: u32,
) -> project::TextDocumentContentChangePartialOrWholeDocument {
    project::TextDocumentContentChangePartialOrWholeDocument {
        partial: Some(lsproto::TextDocumentContentChangePartial {
            text: text.to_string(),
            range: lsproto::Range {
                start: lsproto::Position {
                    line: start_line,
                    character: start_character,
                },
                end: lsproto::Position {
                    line: end_line,
                    character: end_character,
                },
            },
            range_length: None,
        }),
        whole_document: None,
    }
}

#[test]
fn compiler_host_gets_frozen_with_snapshots_fs_only_once() {
    if !ts_bundled::EMBEDDED {
        return;
    }

    let files = HashMap::from([
        (
            "/home/projects/TS/p1/tsconfig.json".to_string(),
            "{}".to_string(),
        ),
        (
            "/home/projects/TS/p1/index.ts".to_string(),
            "console.log('Hello, world!');".to_string(),
        ),
    ]);
    let mut session = setup(files.clone());
    session.did_open_file(
        core::Context::default(),
        "file:///home/projects/TS/p1/index.ts".to_string(),
        1,
        files["/home/projects/TS/p1/index.ts"].clone(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session.did_open_file(
        core::Context::default(),
        "untitled:Untitled-1".to_string(),
        1,
        String::new(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    let inferred_before = {
        let snapshot_before = session.snapshot();
        snapshot_before
            .project_collection
            .inferred_project()
            .expect("inferred project should exist before clone") as *const _
    };

    session.did_change_file(
        core::Context::default(),
        "file:///home/projects/TS/p1/index.ts".to_string(),
        2,
        vec![partial_change("\n", 0, 24, 0, 24)],
    );
    session
        .get_language_service(
            core::Context::default(),
            "file:///home/projects/TS/p1/index.ts".to_string(),
        )
        .expect("GetLanguageService should succeed");
    let snapshot_after = session.snapshot();

    assert_eq!(
        snapshot_after
            .project_collection
            .configured_project(tspath::Path::from("/home/projects/ts/p1/tsconfig.json"))
            .expect("configured project should exist")
            .program_update_kind,
        ProgramUpdateKind::Cloned,
    );
    let inferred_after = snapshot_after
        .project_collection
        .inferred_project()
        .expect("inferred project should exist after clone");
    assert!(std::ptr::eq(inferred_before, inferred_after as *const _));
    assert_eq!(
        inferred_after.program_update_kind,
        ProgramUpdateKind::NewFiles,
    );
    assert!(inferred_after.host.is_some());
}

#[test]
fn cached_disk_files_are_cleaned_up() {
    if !ts_bundled::EMBEDDED {
        return;
    }

    let files = HashMap::from([
        (
            "/home/projects/TS/p1/tsconfig.json".to_string(),
            "{}".to_string(),
        ),
        (
            "/home/projects/TS/p1/index.ts".to_string(),
            "import { a } from './a'; console.log(a);".to_string(),
        ),
        (
            "/home/projects/TS/p1/a.ts".to_string(),
            "export const a = 1;".to_string(),
        ),
        (
            "/home/projects/TS/p2/tsconfig.json".to_string(),
            "{}".to_string(),
        ),
        (
            "/home/projects/TS/p2/index.ts".to_string(),
            "import { b } from './b'; console.log(b);".to_string(),
        ),
        (
            "/home/projects/TS/p2/b.ts".to_string(),
            "export const b = 2;".to_string(),
        ),
    ]);
    let mut session = setup(files.clone());
    session.did_open_file(
        core::Context::default(),
        "file:///home/projects/TS/p1/index.ts".to_string(),
        1,
        files["/home/projects/TS/p1/index.ts"].clone(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session.did_open_file(
        core::Context::default(),
        "file:///home/projects/TS/p2/index.ts".to_string(),
        1,
        files["/home/projects/TS/p2/index.ts"].clone(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    let snapshot_before = session.snapshot();

    assert!(
        snapshot_before
            .fs
            .disk_files
            .contains_key("/home/projects/ts/p1/a.ts")
    );
    assert!(
        snapshot_before
            .fs
            .disk_files
            .contains_key("/home/projects/ts/p2/b.ts")
    );

    session.did_close_file(
        core::Context::default(),
        "file:///home/projects/TS/p1/index.ts".to_string(),
    );
    session.did_open_file(
        core::Context::default(),
        "untitled:Untitled-1".to_string(),
        1,
        String::new(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    let snapshot_after = session.snapshot();

    assert!(
        !snapshot_after
            .fs
            .disk_files
            .contains_key("/home/projects/ts/p1/a.ts")
    );
    assert!(
        snapshot_after
            .fs
            .disk_files
            .contains_key("/home/projects/ts/p2/b.ts")
    );
}

#[test]
fn get_file_returns_none_for_non_existent_files() {
    if !ts_bundled::EMBEDDED {
        return;
    }

    let files = HashMap::from([
        (
            "/home/projects/TS/p1/tsconfig.json".to_string(),
            "{}".to_string(),
        ),
        (
            "/home/projects/TS/p1/index.ts".to_string(),
            "console.log('Hello, world!');".to_string(),
        ),
    ]);
    let mut session = setup(files.clone());
    session.did_open_file(
        core::Context::default(),
        "file:///home/projects/TS/p1/index.ts".to_string(),
        1,
        files["/home/projects/TS/p1/index.ts"].clone(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    let snapshot = session.snapshot();

    let handle = snapshot.get_file("/home/projects/TS/p1/nonexistent.ts");
    assert!(
        handle.is_none(),
        "GetFile should return nil for non-existent file"
    );

    let contents = snapshot.read_file("/home/projects/TS/p1/nonexistent.ts");
    assert!(
        contents.is_none(),
        "ReadFile should return false for non-existent file"
    );
}

#[test]
fn program_change_loads_node_modules_dependency_and_auto_imports_includes_it() {
    if !ts_bundled::EMBEDDED {
        return;
    }

    let files = HashMap::from([
        (
            "/home/projects/otherproject/tsconfig.json".to_string(),
            r#"{
                "compilerOptions": {
                    "module": "commonjs"
                }
            }"#
            .to_string(),
        ),
        (
            "/home/projects/otherproject/index.ts".to_string(),
            String::new(),
        ),
        (
            "/home/projects/node_modules/foo/package.json".to_string(),
            r#"{
                "types": "index.d.ts",
                "typesVersions": {
                    "*": {
                        "bar/*": ["dist/*"],
                        "exact-match": ["dist/index.d.ts"],
                        "foo/*": ["dist/*"],
                        "*": ["dist/*"]
                    }
                }
            }"#
            .to_string(),
        ),
        (
            "/home/projects/node_modules/foo/nope.d.ts".to_string(),
            "export const nope = 0;".to_string(),
        ),
        (
            "/home/projects/node_modules/foo/dist/index.d.ts".to_string(),
            "export const index = 0;".to_string(),
        ),
        (
            "/home/projects/node_modules/foo/dist/blah.d.ts".to_string(),
            "export const blah = 0;".to_string(),
        ),
        (
            "/home/projects/node_modules/foo/dist/foo/onlyInFooFolder.d.ts".to_string(),
            "export const foo = 0;".to_string(),
        ),
        (
            "/home/projects/node_modules/foo/dist/subfolder/one.d.ts".to_string(),
            "export const one = 0;".to_string(),
        ),
    ]);
    let mut session = setup(files.clone());
    let ctx = core::Context::default();
    let other_index_uri = "file:///home/projects/otherproject/index.ts".to_string();

    session.did_open_file(
        ctx.clone(),
        other_index_uri.clone(),
        1,
        files["/home/projects/otherproject/index.ts"].clone(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );

    session.did_change_file(
        ctx.clone(),
        other_index_uri.clone(),
        2,
        vec![partial_change(
            r#"import {} from "foo/foo/subfolder/one";"#,
            0,
            0,
            0,
            0,
        )],
    );

    session
        .get_current_language_service_with_auto_imports(ctx, other_index_uri)
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");
    session.close();
}

#[cfg(feature = "nightly_bench")]
#[bench]
fn benchmark_snapshot_clone_ref_cost(b: &mut test::Bencher) {
    if !ts_bundled::EMBEDDED {
        return;
    }

    for large_project_size in [100, 1000, 10_000] {
        let mut files = HashMap::from([
            (
                "/small/tsconfig.json".to_string(),
                r#"{"compilerOptions": {"strict": true}}"#.to_string(),
            ),
            (
                "/large/tsconfig.json".to_string(),
                r#"{"compilerOptions": {"strict": true}}"#.to_string(),
            ),
        ]);

        for i in 0..100 {
            files.insert(
                format!("/small/file{i}.ts"),
                format!("export const small{i} = {i};"),
            );
        }

        for i in 0..large_project_size {
            files.insert(
                format!("/large/file{i}.ts"),
                format!("export const large{i} = {i};"),
            );
        }

        let fs = ts_bundled::wrap_fs(ts_vfs::vfstest::from_map(files.clone(), false));
        let mut session = project::new_session(project::SessionInit {
            background_ctx: core::Context::default(),
            options: project::SessionOptions {
                current_directory: "/".to_string(),
                default_library_path: ts_bundled::lib_path(),
                typings_location: "/home/src/Library/Caches/typescript".to_string(),
                position_encoding: lsproto::PositionEncodingKind::UTF8,
                watch_enabled: false,
                logging_enabled: false,
                telemetry_enabled: false,
                push_diagnostics_enabled: true,
                debounce_delay: Default::default(),
                locale: Default::default(),
            },
            fs: Arc::new(fs),
            client: None,
            logger: Arc::new(project::logging::new_test_logger()),
            npm_executor: Default::default(),
            parse_cache: None,
        });

        session.did_open_file(
            core::Context::default(),
            "file:///small/file0.ts".to_string(),
            1,
            files["/small/file0.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session
            .get_language_service(
                core::Context::default(),
                "file:///small/file0.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");

        session.did_open_file(
            core::Context::default(),
            "file:///large/file0.ts".to_string(),
            1,
            files["/large/file0.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session
            .get_language_service(
                core::Context::default(),
                "file:///large/file0.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");

        session.wait_for_background_tasks();

        let mut strict = true;
        b.iter(|| {
            strict = !strict;
            let tsconfig_content = if strict {
                r#"{"compilerOptions": {"strict": true}}"#
            } else {
                r#"{"compilerOptions": {"strict": false}}"#
            };
            session
                .fs()
                .write_file("/small/tsconfig.json", tsconfig_content)
                .expect("WriteFile should succeed");
            session.pending_file_changes.push(FileChange {
                kind: FileChangeKind::WatchChange,
                uri: "file:///small/tsconfig.json".to_string(),
                version: 0,
                content: String::new(),
                language_kind: String::new(),
                changes: Vec::new(),
            });
            session
                .get_language_service(
                    core::Context::default(),
                    "file:///small/file0.ts".to_string(),
                )
                .expect("GetLanguageService should succeed");
            session.wait_for_background_tasks();
        });
    }
}
