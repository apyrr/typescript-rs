use std::collections::HashMap;
use std::time::Duration;

use crate::projecttestutil;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto::{self as lsproto, DocumentUriExt};
use ts_tspath as tspath;
use ts_vfs::vfstest;
use ts_vfs::vfstest::IntoMapFile;

use crate as project;

const LANGUAGE_KIND_JAVASCRIPT: &str = "javascript";
const LANGUAGE_KIND_TYPESCRIPT: &str = "typescript";

fn partial_change(
    text: &str,
    start_line: u32,
    start_character: u32,
    end_line: u32,
    end_character: u32,
) -> project::TextDocumentContentChangePartialOrWholeDocument {
    project::TextDocumentContentChangePartialOrWholeDocument {
        partial: Some(lsproto::TextDocumentContentChangePartial {
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
            text: text.to_string(),
        }),
        whole_document: None,
    }
}

fn same_program(left: &ts_compiler::Program, right: &ts_compiler::Program) -> bool {
    std::ptr::eq(left, right)
}

fn same_source_file(left: Option<ts_ast::SourceFile>, right: Option<ts_ast::SourceFile>) -> bool {
    left == right
}

fn file_event(uri: &str, typ: lsproto::FileChangeType) -> lsproto::FileEvent {
    lsproto::FileEvent {
        uri: lsproto::DocumentUri::from(uri),
        typ,
    }
}

fn setup_options(current_directory: &str) -> projecttestutil::SessionOptions {
    projecttestutil::SessionOptions {
        current_directory: current_directory.to_string(),
        default_library_path: ts_bundled::lib_path(),
        typings_location: projecttestutil::TEST_TYPINGS_LOCATION.to_string(),
        position_encoding: lsproto::PositionEncodingKind::UTF8,
        watch_enabled: true,
        logging_enabled: true,
        telemetry_enabled: false,
        push_diagnostics_enabled: true,
        debounce_delay: Duration::default(),
        locale: ts_locale::Locale::default(),
    }
}

#[test]
fn test_session() {
    if !ts_bundled::EMBEDDED {
        return;
    }

    let default_files = HashMap::from([
        (
            "/home/projects/TS/p1/tsconfig.json".to_string(),
            r#"{
            "compilerOptions": {
                "noLib": true,
                "module": "nodenext",
                "strict": true
            },
            "include": ["src"]
        }"#
            .to_string(),
        ),
        (
            "/home/projects/TS/p1/src/index.ts".to_string(),
            r#"import { x } from "./x";"#.to_string(),
        ),
        (
            "/home/projects/TS/p1/src/x.ts".to_string(),
            "export const x = 1;".to_string(),
        ),
        (
            "/home/projects/TS/p1/config.ts".to_string(),
            "let x = 1, y = 2;".to_string(),
        ),
    ]);

    // DidOpenFile/create configured project
    {
        let (mut session, _) = projecttestutil::setup(default_files.clone());
        let mut snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 0);

        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            1,
            default_files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );

        snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(
            snapshot
                .project_collection
                .configured_project(tspath::Path::from("/home/projects/ts/p1/tsconfig.json"))
                .is_some()
        );

        let ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let program = ls.get_program();
        let x = program
            .get_source_file("/home/projects/TS/p1/src/x.ts")
            .expect("x.ts should be present");
        assert_eq!(x.text(), "export const x = 1;");
    }

    // DidOpenFile/create inferred project
    {
        let (mut session, _) = projecttestutil::setup(default_files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/config.ts".to_string(),
            1,
            default_files["/home/projects/TS/p1/config.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 2);
        assert!(
            snapshot
                .project_collection
                .configured_project(tspath::Path::from("/home/projects/ts/p1/tsconfig.json"))
                .is_some()
        );
        assert!(snapshot.project_collection.inferred_project().is_some());
    }

    // DidOpenFile/inferred project for in-memory files
    {
        let (mut session, _) = projecttestutil::setup(default_files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/config.ts".to_string(),
            1,
            default_files["/home/projects/TS/p1/config.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.did_open_file(
            core::Context::default(),
            "untitled:Untitled-1".to_string(),
            1,
            "x".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.did_open_file(
            core::Context::default(),
            "untitled:Untitled-2".to_string(),
            1,
            "y".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(snapshot.project_collection.inferred_project().is_some());
    }

    // DidOpenFile/inferred project JS file
    {
        let js_files = HashMap::from([(
            "/home/projects/TS/p1/index.js".to_string(),
            r#"import { x } from "./x";"#.to_string(),
        )]);
        let (mut session, _) = projecttestutil::setup(js_files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/index.js".to_string(),
            1,
            js_files["/home/projects/TS/p1/index.js"].clone(),
            LANGUAGE_KIND_JAVASCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        let ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/index.js".to_string(),
            )
            .expect("GetLanguageService should succeed");
        assert!(
            ls.get_program()
                .get_source_file("/home/projects/TS/p1/index.js")
                .is_some()
        );
    }

    // watchChange and didOpen in same batch rebuilds program
    {
        let files = HashMap::from([
            (
                "/home/projects/TS/p1/tsconfig.json".to_string(),
                r#"{
                "compilerOptions": {
                    "noLib": true,
                    "strict": true
                }
            }"#
                .to_string(),
            ),
            (
                "/home/projects/TS/p1/src/a.ts".to_string(),
                "export const a = 1;\n".to_string(),
            ),
            (
                "/home/projects/TS/p1/src/b.ts".to_string(),
                "export const b = 1;\n".to_string(),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup(files.clone());
        let old_content = files["/home/projects/TS/p1/src/a.ts"].clone();
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/b.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/b.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let mut ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/b.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        assert_eq!(
            ls.get_program()
                .get_source_file("/home/projects/TS/p1/src/a.ts")
                .as_ref()
                .expect("source file should exist")
                .text(),
            old_content
        );
        let new_content = "export const a = 2;\nexport const extra = true;\n";
        utils
            .fs()
            .write_file("/home/projects/TS/p1/src/a.ts", new_content)
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///home/projects/TS/p1/src/a.ts",
                lsproto::FileChangeType::CHANGED,
            )],
        );
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/a.ts".to_string(),
            1,
            new_content.to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/a.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        assert_eq!(
            ls.get_program()
                .get_source_file("/home/projects/TS/p1/src/a.ts")
                .as_ref()
                .expect("source file should exist")
                .text(),
            new_content
        );
    }

    // DidChangeFile/update file and program
    {
        let (mut session, _) = projecttestutil::setup(default_files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/x.ts".to_string(),
            1,
            default_files["/home/projects/TS/p1/src/x.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let ls_before = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/x.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let program_before = ls_before.get_program();
        session.did_change_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/x.ts".to_string(),
            2,
            vec![partial_change("2", 0, 17, 0, 18)],
        );
        let ls_after = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/x.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let program_after = ls_after.get_program();
        assert!(!same_program(program_after, program_before));
        assert_eq!(
            program_after
                .get_source_file("/home/projects/TS/p1/src/x.ts")
                .as_ref()
                .expect("source file should exist")
                .text(),
            "export const x = 2;"
        );
    }

    // DidChangeFile/update untitled file
    {
        let (mut session, _) = projecttestutil::setup(default_files.clone());
        session.did_open_file(
            core::Context::default(),
            "untitled:Untitled-1".to_string(),
            1,
            "let x = 1;".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let ls_before = session
            .get_language_service(core::Context::default(), "untitled:Untitled-1".to_string())
            .expect("GetLanguageService should succeed");
        let program_before = ls_before.get_program();
        let untitled_file_name = lsproto::DocumentUri::from("untitled:Untitled-1").file_name();
        assert_eq!(
            program_before
                .get_source_file(&untitled_file_name)
                .as_ref()
                .expect("source file should exist")
                .text(),
            "let x = 1;"
        );
        session.did_change_file(
            core::Context::default(),
            "untitled:Untitled-1".to_string(),
            2,
            vec![partial_change("2", 0, 8, 0, 9)],
        );
        let ls_after = session
            .get_language_service(core::Context::default(), "untitled:Untitled-1".to_string())
            .expect("GetLanguageService should succeed");
        let program_after = ls_after.get_program();
        assert!(!same_program(program_after, program_before));
        assert_eq!(
            program_after
                .get_source_file(&untitled_file_name)
                .as_ref()
                .expect("source file should exist")
                .text(),
            "let x = 2;"
        );
    }

    // DidChangeFile/unchanged source files are reused
    {
        let (mut session, _) = projecttestutil::setup(default_files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/x.ts".to_string(),
            1,
            default_files["/home/projects/TS/p1/src/x.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let ls_before = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/x.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let program_before = ls_before.get_program();
        let index_file_before = program_before.get_source_file("/home/projects/TS/p1/src/index.ts");
        session.did_change_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/x.ts".to_string(),
            2,
            vec![partial_change(";", 0, 0, 0, 0)],
        );
        let ls_after = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/x.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        assert!(same_source_file(
            ls_after
                .get_program()
                .get_source_file("/home/projects/TS/p1/src/index.ts"),
            index_file_before
        ));
    }

    // DidChangeFile/change can pull in new files
    {
        let mut files = default_files.clone();
        files.insert(
            "/home/projects/TS/p1/y.ts".to_string(),
            "export const y = 2;".to_string(),
        );
        let (mut session, _) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let ls_before = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        assert!(
            ls_before
                .get_program()
                .get_source_file("/home/projects/TS/p1/y.ts")
                .is_none()
        );
        session.did_change_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            2,
            vec![partial_change(r#"import { y } from "../y";\n"#, 0, 0, 0, 0)],
        );
        let ls_after = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        assert!(
            ls_after
                .get_program()
                .get_source_file("/home/projects/TS/p1/y.ts")
                .is_some()
        );
    }

    // DidChangeFile/single-file change followed by config change reloads program
    {
        let mut files = default_files.clone();
        files.insert(
            "/home/projects/TS/p1/tsconfig.json".to_string(),
            r#"{
                "compilerOptions": {
                    "noLib": true,
                    "module": "nodenext",
                    "strict": true
                },
                "include": ["src/index.ts"]
            }"#
            .to_string(),
        );
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let ls_before = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        assert_eq!(ls_before.get_program().get_source_files().len(), 2);
        session.did_change_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            2,
            vec![partial_change("\n", 0, 0, 0, 0)],
        );
        utils
            .fs()
            .write_file(
                "/home/projects/TS/p1/tsconfig.json",
                r#"{
                "compilerOptions": {
                    "noLib": true,
                    "module": "nodenext",
                    "strict": true
                },
                "include": ["./**/*"]
            }"#,
            )
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///home/projects/TS/p1/tsconfig.json",
                lsproto::FileChangeType::CHANGED,
            )],
        );
        let ls_after = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        assert_eq!(ls_after.get_program().get_source_files().len(), 3);
    }

    // DidCloseFile/Configured projects/delete a file, close it, recreate it
    {
        let files = default_files.clone();
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/x.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/x.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        utils
            .fs()
            .remove("/home/projects/TS/p1/src/x.ts")
            .expect("Remove should succeed");
        session.did_close_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/x.ts".to_string(),
        );
        let mut ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        assert!(
            ls.get_program()
                .get_source_file("/home/projects/TS/p1/src/x.ts")
                .is_none()
        );
        utils
            .fs()
            .write_file("/home/projects/TS/p1/src/x.ts", "")
            .expect("WriteFile should succeed");
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/x.ts".to_string(),
            1,
            "".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/x.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        assert_eq!(
            ls.get_program()
                .get_source_file("/home/projects/TS/p1/src/x.ts")
                .as_ref()
                .expect("source file should exist")
                .text(),
            ""
        );
    }

    // DidCloseFile/Inferred projects/delete a file, close it, recreate it
    {
        let mut files = default_files.clone();
        files.remove("/home/projects/TS/p1/tsconfig.json");
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/x.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/x.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        utils
            .fs()
            .remove("/home/projects/TS/p1/src/x.ts")
            .expect("Remove should succeed");
        session.did_close_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/x.ts".to_string(),
        );
        let mut ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        assert!(
            ls.get_program()
                .get_source_file("/home/projects/TS/p1/src/x.ts")
                .is_none()
        );
        utils
            .fs()
            .write_file("/home/projects/TS/p1/src/x.ts", "")
            .expect("WriteFile should succeed");
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/x.ts".to_string(),
            1,
            "".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/x.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        assert_eq!(
            ls.get_program()
                .get_source_file("/home/projects/TS/p1/src/x.ts")
                .as_ref()
                .expect("source file should exist")
                .text(),
            ""
        );
    }

    // DidCloseFile/Inferred projects/close untitled file
    {
        let (mut session, _) = projecttestutil::setup(default_files.clone());
        session.did_open_file(
            core::Context::default(),
            "untitled:Untitled-1".to_string(),
            1,
            "let x = 1;".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.did_close_file(core::Context::default(), "untitled:Untitled-1".to_string());
        session.did_open_file(
            core::Context::default(),
            "untitled:Untitled-2".to_string(),
            1,
            "".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
    }

    // DidSaveFile/save event first
    {
        let (mut session, _) = projecttestutil::setup(default_files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            1,
            default_files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let mut snapshot = session.snapshot();
        assert_eq!(snapshot.id(), 1);
        session.did_save_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
        );
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///home/projects/TS/p1/src/index.ts",
                lsproto::FileChangeType::CHANGED,
            )],
        );
        session.wait_for_background_tasks();
        snapshot = session.snapshot();
        assert_eq!(snapshot.id(), 1);
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/x.ts".to_string(),
            1,
            default_files["/home/projects/TS/p1/src/x.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        snapshot = session.snapshot();
        assert!(
            snapshot
                .get_file("/home/projects/TS/p1/src/index.ts")
                .unwrap()
                .matches_disk_text()
        );
    }

    // DidSaveFile/watch event first
    {
        let (mut session, _) = projecttestutil::setup(default_files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            1,
            default_files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let mut snapshot = session.snapshot();
        assert_eq!(snapshot.id(), 1);
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///home/projects/TS/p1/src/index.ts",
                lsproto::FileChangeType::CHANGED,
            )],
        );
        session.did_save_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
        );
        session.wait_for_background_tasks();
        snapshot = session.snapshot();
        assert_eq!(snapshot.id(), 1);
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/x.ts".to_string(),
            1,
            default_files["/home/projects/TS/p1/src/x.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        snapshot = session.snapshot();
        assert!(
            snapshot
                .get_file("/home/projects/TS/p1/src/index.ts")
                .unwrap()
                .matches_disk_text()
        );
    }

    // Source file sharing/projects with similar options share source files
    {
        let mut files = default_files.clone();
        files.insert(
            "/home/projects/TS/p2/tsconfig.json".to_string(),
            r#"{
                "compilerOptions": {
                    "noLib": true,
                    "module": "nodenext",
                    "strict": true,
                    "noCheck": true
                }
            }"#
            .to_string(),
        );
        files.insert(
            "/home/projects/TS/p2/src/index.ts".to_string(),
            r#"import { x } from "../../p1/src/x";"#.to_string(),
        );
        let (mut session, _) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p2/src/index.ts".to_string(),
            1,
            files["/home/projects/TS/p2/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        assert_eq!(session.snapshot().project_collection.projects().len(), 2);
        let x1 = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed")
            .get_program()
            .get_source_file("/home/projects/TS/p1/src/x.ts");
        let x2 = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p2/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed")
            .get_program()
            .get_source_file("/home/projects/TS/p1/src/x.ts");
        assert!(same_source_file(x1, x2));
    }

    // Source file sharing/projects with different options do not share source files
    {
        let mut files = default_files.clone();
        files.insert(
            "/home/projects/TS/p2/tsconfig.json".to_string(),
            r#"{
                "compilerOptions": {
                    "noLib": true,
                    "module": "nodenext",
                    "strict": true,
                    "moduleDetection": "auto"
                },
                "include": ["src"]
            }"#
            .to_string(),
        );
        files.insert(
            "/home/projects/TS/p2/src/index.ts".to_string(),
            r#"import { x } from "../../p1/src/x";"#.to_string(),
        );
        let (mut session, _) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p2/src/index.ts".to_string(),
            1,
            files["/home/projects/TS/p2/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        assert_eq!(session.snapshot().project_collection.projects().len(), 2);
        let x1 = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed")
            .get_program()
            .get_source_file("/home/projects/TS/p1/src/x.ts");
        let x2 = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p2/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed")
            .get_program()
            .get_source_file("/home/projects/TS/p1/src/x.ts");
        assert!(x1.is_some() && x2.is_some());
        assert!(!same_source_file(x1, x2));
    }

    // DidChangeWatchedFiles/change open file
    {
        let files = default_files.clone();
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/x.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/x.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let program_before = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed")
            .get_program() as *const _;
        utils
            .fs()
            .write_file("/home/projects/TS/p1/src/x.ts", "export const x = 2;")
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///home/projects/TS/p1/src/x.ts",
                lsproto::FileChangeType::CHANGED,
            )],
        );
        let ls_after = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        assert!(std::ptr::eq(
            program_before,
            ls_after.get_program() as *const _
        ));
    }

    // DidChangeWatchedFiles/change closed program file
    {
        let files = default_files.clone();
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let program_before = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed")
            .get_program() as *const _;
        utils
            .fs()
            .write_file("/home/projects/TS/p1/src/x.ts", "export const x = 2;")
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///home/projects/TS/p1/src/x.ts",
                lsproto::FileChangeType::CHANGED,
            )],
        );
        let ls_after = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        assert!(!std::ptr::eq(
            ls_after.get_program() as *const _,
            program_before
        ));
    }

    // DidChangeWatchedFiles/change program file not in tsconfig root files
    for workspace_dir in ["/", "/home/projects/TS/p1", "/somewhere/else/entirely"] {
        let files = HashMap::from([
            (
                "/home/projects/TS/p1/tsconfig.json".to_string(),
                r#"{
                    "compilerOptions": {
                        "noLib": true,
                        "module": "nodenext",
                        "strict": true
                    },
                    "files": ["src/index.ts"]
                }"#
                .to_string(),
            ),
            (
                "/home/projects/TS/p1/src/index.ts".to_string(),
                r#"import { x } from "../../x";"#.to_string(),
            ),
            (
                "/home/projects/TS/x.ts".to_string(),
                "export const x = 1;".to_string(),
            ),
        ]);
        let (mut session, utils) =
            projecttestutil::setup_with_options(files.clone(), setup_options(workspace_dir));
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let program_before = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed")
            .get_program() as *const _;
        session.wait_for_background_tasks();
        assert!(utils.watches_file("/home/projects/ts/x.ts"));
        utils
            .fs()
            .write_file("/home/projects/TS/x.ts", "export const x = 2;")
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///home/projects/TS/x.ts",
                lsproto::FileChangeType::CHANGED,
            )],
        );
        let ls_after = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        assert!(!std::ptr::eq(
            ls_after.get_program() as *const _,
            program_before
        ));
    }

    // DidChangeWatchedFiles/change config file
    {
        let files = HashMap::from([
            (
                "/home/projects/TS/p1/tsconfig.json".to_string(),
                r#"{"compilerOptions":{"noLib":true,"strict":false}}"#.to_string(),
            ),
            (
                "/home/projects/TS/p1/src/x.ts".to_string(),
                "export declare const x: number | undefined;".to_string(),
            ),
            (
                "/home/projects/TS/p1/src/index.ts".to_string(),
                "\n\t\t\t\t\timport { x } from \"./x\";\n\t\t\t\t\tlet y: number = x;".to_string(),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let mut ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let mut program = ls.get_program();
        assert_eq!(
            program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/TS/p1/src/index.ts")
                        .as_ref()
                )
                .len(),
            0
        );
        utils
            .fs()
            .write_file(
                "/home/projects/TS/p1/tsconfig.json",
                r#"{"compilerOptions":{"noLib":false,"strict":true}}"#,
            )
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///home/projects/TS/p1/tsconfig.json",
                lsproto::FileChangeType::CHANGED,
            )],
        );
        ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        program = ls.get_program();
        assert_eq!(
            program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/TS/p1/src/index.ts")
                        .as_ref()
                )
                .len(),
            1
        );
    }

    // DidChangeWatchedFiles/delete explicitly included file
    {
        let files = HashMap::from([
            (
                "/home/projects/TS/p1/tsconfig.json".to_string(),
                r#"{"compilerOptions":{"noLib":true},"files":["src/index.ts","src/x.ts"]}"#
                    .to_string(),
            ),
            (
                "/home/projects/TS/p1/src/x.ts".to_string(),
                "export declare const x: number | undefined;".to_string(),
            ),
            (
                "/home/projects/TS/p1/src/index.ts".to_string(),
                r#"import { x } from "./x";"#.to_string(),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let mut ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let mut program = ls.get_program();
        assert!(
            program
                .command_line()
                .file_names
                .contains(&"/home/projects/TS/p1/src/x.ts".to_string())
        );
        assert_eq!(
            program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/TS/p1/src/index.ts")
                        .as_ref()
                )
                .len(),
            0
        );
        utils
            .fs()
            .remove("/home/projects/TS/p1/src/x.ts")
            .expect("Remove should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///home/projects/TS/p1/src/x.ts",
                lsproto::FileChangeType::DELETED,
            )],
        );
        ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        program = ls.get_program();
        assert!(
            program
                .command_line()
                .file_names
                .contains(&"/home/projects/TS/p1/src/x.ts".to_string())
        );
        assert_eq!(
            program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/TS/p1/src/index.ts")
                        .as_ref()
                )
                .len(),
            1
        );
        assert!(
            program
                .get_source_file("/home/projects/TS/p1/src/x.ts")
                .is_none()
        );
        session.did_open_file(
            core::Context::default(),
            "untitled:Untitled-1".to_string(),
            1,
            "".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        assert!(
            session
                .snapshot()
                .get_file("/home/projects/TS/p1/src/x.ts")
                .is_none()
        );
    }

    // DidChangeWatchedFiles/delete wildcard included file
    {
        let files = HashMap::from([
            (
                "/home/projects/TS/p1/tsconfig.json".to_string(),
                r#"{"compilerOptions":{"noLib":true},"include":["src"]}"#.to_string(),
            ),
            (
                "/home/projects/TS/p1/src/index.ts".to_string(),
                "let x = 2;".to_string(),
            ),
            (
                "/home/projects/TS/p1/src/x.ts".to_string(),
                "let y = x;".to_string(),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/x.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/x.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let mut ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/x.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let mut program = ls.get_program();
        assert!(
            program
                .command_line()
                .file_names
                .contains(&"/home/projects/TS/p1/src/index.ts".to_string())
        );
        assert_eq!(
            program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/TS/p1/src/x.ts")
                        .as_ref()
                )
                .len(),
            0
        );
        utils
            .fs()
            .remove("/home/projects/TS/p1/src/index.ts")
            .expect("Remove should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///home/projects/TS/p1/src/index.ts",
                lsproto::FileChangeType::DELETED,
            )],
        );
        ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/x.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        program = ls.get_program();
        assert!(
            !program
                .command_line()
                .file_names
                .contains(&"/home/projects/TS/p1/src/index.ts".to_string())
        );
        assert_eq!(
            program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/TS/p1/src/x.ts")
                        .as_ref()
                )
                .len(),
            1
        );
        session.did_open_file(
            core::Context::default(),
            "untitled:Untitled-1".to_string(),
            1,
            "".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        assert!(
            session
                .snapshot()
                .get_file("/home/projects/TS/p1/src/index.ts")
                .is_none()
        );
    }

    // DidChangeWatchedFiles/delete directory with wildcard included files
    {
        let files = HashMap::from([
            (
                "/home/projects/TS/p1/tsconfig.json".to_string(),
                r#"{"compilerOptions":{"noLib":true},"include":["src"]}"#.to_string(),
            ),
            (
                "/home/projects/TS/p1/src/index.ts".to_string(),
                r#"import { x } from "./sub/x";"#.to_string(),
            ),
            (
                "/home/projects/TS/p1/src/sub/x.ts".to_string(),
                "export const x = 1;".to_string(),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let mut ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let mut program = ls.get_program();
        assert!(
            program
                .command_line()
                .file_names
                .contains(&"/home/projects/TS/p1/src/sub/x.ts".to_string())
        );
        assert_eq!(
            program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/TS/p1/src/index.ts")
                        .as_ref()
                )
                .len(),
            0
        );
        utils
            .fs()
            .remove("/home/projects/TS/p1/src/sub")
            .expect("Remove should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///home/projects/TS/p1/src/sub",
                lsproto::FileChangeType::DELETED,
            )],
        );
        ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        program = ls.get_program();
        assert!(
            !program
                .command_line()
                .file_names
                .contains(&"/home/projects/TS/p1/src/sub/x.ts".to_string())
        );
        assert_eq!(
            program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/TS/p1/src/index.ts")
                        .as_ref()
                )
                .len(),
            1
        );
    }

    // DidChangeWatchedFiles/delete directory with program-only files
    {
        let files = HashMap::from([
            (
                "/home/projects/TS/p1/tsconfig.json".to_string(),
                r#"{"compilerOptions":{"noLib":true},"files":["src/index.ts"]}"#.to_string(),
            ),
            (
                "/home/projects/TS/p1/src/index.ts".to_string(),
                r#"import { x } from "./sub/x";"#.to_string(),
            ),
            (
                "/home/projects/TS/p1/src/sub/x.ts".to_string(),
                "export const x = 1;".to_string(),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let mut ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let mut program = ls.get_program();
        assert!(
            program
                .command_line()
                .file_names
                .contains(&"/home/projects/TS/p1/src/index.ts".to_string())
        );
        assert!(
            program
                .get_source_file("/home/projects/TS/p1/src/sub/x.ts")
                .is_some()
        );
        assert_eq!(
            program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/TS/p1/src/index.ts")
                        .as_ref()
                )
                .len(),
            0
        );
        utils
            .fs()
            .remove("/home/projects/TS/p1/src/sub")
            .expect("Remove should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///home/projects/TS/p1/src/sub",
                lsproto::FileChangeType::DELETED,
            )],
        );
        ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        program = ls.get_program();
        assert!(
            program
                .get_source_file("/home/projects/TS/p1/src/sub/x.ts")
                .is_none()
        );
        assert_eq!(
            program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/TS/p1/src/index.ts")
                        .as_ref()
                )
                .len(),
            1
        );
    }

    // DidChangeWatchedFiles/delete sibling folder schedules diagnostics refresh
    {
        let files = HashMap::from([
            (
                "/home/projects/TS/p1/tsconfig.json".to_string(),
                r#"{"compilerOptions":{"noLib":true},"files":["index.ts"]}"#.to_string(),
            ),
            (
                "/home/projects/TS/p1/index.ts".to_string(),
                "import { content } from \"./f/content\";\n\nexport const value = content;"
                    .to_string(),
            ),
            (
                "/home/projects/TS/p1/f/content.ts".to_string(),
                "export const content = 1;".to_string(),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup(files.clone());
        let content_uri = lsproto::DocumentUri::from("file:///home/projects/TS/p1/f/content.ts");
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/index.ts".to_string(),
            1,
            files["/home/projects/TS/p1/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.did_open_file(
            core::Context::default(),
            content_uri.clone(),
            1,
            files["/home/projects/TS/p1/f/content.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        session.wait_for_background_tasks();
        let baseline_refresh_count = utils.client().refresh_diagnostics_calls().len();
        utils
            .fs()
            .remove("/home/projects/TS/p1/f")
            .expect("Remove should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///home/projects/TS/p1/f",
                lsproto::FileChangeType::DELETED,
            )],
        );
        session.did_close_file(core::Context::default(), content_uri);
        session.wait_for_background_tasks();
        assert!(utils.client().refresh_diagnostics_calls().len() > baseline_refresh_count);
    }

    // DidChangeWatchedFiles/delete sibling folder schedules diagnostics refresh after opening third file
    {
        let files = HashMap::from([
            (
                "/home/projects/TS/p1/tsconfig.json".to_string(),
                r#"{"compilerOptions":{"noLib":true},"files":["index.ts","third.ts"]}"#.to_string(),
            ),
            (
                "/home/projects/TS/p1/index.ts".to_string(),
                "import { content } from \"./f/content\";\n\nexport const value = content;"
                    .to_string(),
            ),
            (
                "/home/projects/TS/p1/f/content.ts".to_string(),
                "export const content = 1;".to_string(),
            ),
            (
                "/home/projects/TS/p1/third.ts".to_string(),
                "export const third = 3;".to_string(),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup(files.clone());
        let content_uri = lsproto::DocumentUri::from("file:///home/projects/TS/p1/f/content.ts");
        let third_uri = lsproto::DocumentUri::from("file:///home/projects/TS/p1/third.ts");
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/index.ts".to_string(),
            1,
            files["/home/projects/TS/p1/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.did_open_file(
            core::Context::default(),
            content_uri,
            1,
            files["/home/projects/TS/p1/f/content.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        session.wait_for_background_tasks();
        let baseline_refresh_count = utils.client().refresh_diagnostics_calls().len();
        utils
            .fs()
            .remove("/home/projects/TS/p1/f")
            .expect("Remove should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///home/projects/TS/p1/f",
                lsproto::FileChangeType::DELETED,
            )],
        );
        session.did_open_file(
            core::Context::default(),
            third_uri,
            1,
            files["/home/projects/TS/p1/third.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.wait_for_background_tasks();
        assert!(utils.client().refresh_diagnostics_calls().len() > baseline_refresh_count);
    }

    // DidChangeWatchedFiles/create explicitly included file
    {
        let files = HashMap::from([
            (
                "/home/projects/TS/p1/tsconfig.json".to_string(),
                r#"{"compilerOptions":{"noLib":true},"files":["src/index.ts","src/y.ts"]}"#
                    .to_string(),
            ),
            (
                "/home/projects/TS/p1/src/index.ts".to_string(),
                r#"import { y } from "./y";"#.to_string(),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let mut ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let mut program = ls.get_program();
        assert_eq!(
            program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/TS/p1/src/index.ts")
                        .as_ref()
                )
                .len(),
            1
        );
        utils
            .fs()
            .write_file("/home/projects/TS/p1/src/y.ts", "export const y = 1;")
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///home/projects/TS/p1/src/y.ts",
                lsproto::FileChangeType::CREATED,
            )],
        );
        ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        program = ls.get_program();
        assert_eq!(
            program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/TS/p1/src/index.ts")
                        .as_ref()
                )
                .len(),
            0
        );
        assert!(
            program
                .get_source_file("/home/projects/TS/p1/src/y.ts")
                .is_some()
        );
    }

    // DidChangeWatchedFiles/create failed lookup location
    {
        let files = HashMap::from([
            (
                "/home/projects/TS/p1/tsconfig.json".to_string(),
                r#"{"compilerOptions":{"noLib":true},"files":["src/index.ts"]}"#.to_string(),
            ),
            (
                "/home/projects/TS/p1/src/index.ts".to_string(),
                r#"import { z } from "./z";"#.to_string(),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let mut ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let mut program = ls.get_program();
        assert_eq!(
            program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/TS/p1/src/index.ts")
                        .as_ref()
                )
                .len(),
            1
        );
        utils
            .fs()
            .write_file("/home/projects/TS/p1/src/z.ts", "export const z = 1;")
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///home/projects/TS/p1/src/z.ts",
                lsproto::FileChangeType::CREATED,
            )],
        );
        ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        program = ls.get_program();
        assert_eq!(
            program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/TS/p1/src/index.ts")
                        .as_ref()
                )
                .len(),
            0
        );
        assert!(
            program
                .get_source_file("/home/projects/TS/p1/src/z.ts")
                .is_some()
        );
    }

    // DidChangeWatchedFiles/create wildcard included file
    {
        let files = HashMap::from([
            (
                "/home/projects/TS/p1/tsconfig.json".to_string(),
                r#"{"compilerOptions":{"noLib":true},"include":["src"]}"#.to_string(),
            ),
            (
                "/home/projects/TS/p1/src/index.ts".to_string(),
                "a;".to_string(),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let mut ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let mut program = ls.get_program();
        assert_eq!(
            program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/TS/p1/src/index.ts")
                        .as_ref()
                )
                .len(),
            1
        );
        utils
            .fs()
            .write_file("/home/projects/TS/p1/src/a.ts", "const a = 1;")
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///home/projects/TS/p1/src/a.ts",
                lsproto::FileChangeType::CREATED,
            )],
        );
        ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        program = ls.get_program();
        assert_eq!(
            program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/TS/p1/src/index.ts")
                        .as_ref()
                )
                .len(),
            0
        );
        assert!(
            program
                .get_source_file("/home/projects/TS/p1/src/a.ts")
                .is_some()
        );
    }

    // DidChangeWatchedFiles/irrelevant extension changes are filtered out
    {
        let files = HashMap::from([
            (
                "/home/projects/TS/p1/tsconfig.json".to_string(),
                r#"{"compilerOptions":{"noLib":true},"include":["src"]}"#.to_string(),
            ),
            (
                "/home/projects/TS/p1/src/index.ts".to_string(),
                "export const x = 1;".to_string(),
            ),
            (
                "/home/projects/TS/p1/src/data.txt".to_string(),
                "some text".to_string(),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let program = ls.get_program();
        assert_eq!(
            program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/TS/p1/src/index.ts")
                        .as_ref()
                )
                .len(),
            0
        );
        let old_program = program as *const _;
        utils
            .fs()
            .write_file("/home/projects/TS/p1/src/data.txt", "updated text")
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![
                file_event(
                    "file:///home/projects/TS/p1/src/data.txt",
                    lsproto::FileChangeType::CHANGED,
                ),
                file_event(
                    "file:///home/projects/TS/p1/src/styles.css",
                    lsproto::FileChangeType::CREATED,
                ),
                file_event(
                    "file:///home/projects/TS/p1/src/image.png",
                    lsproto::FileChangeType::CREATED,
                ),
            ],
        );
        let ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let program = ls.get_program();
        assert!(
            std::ptr::eq(program as *const _, old_program),
            "program should not be rebuilt for irrelevant extension changes"
        );
    }

    // DidChangeWatchedFiles/pnpm install links local package
    {
        let files = HashMap::from([
            (
                "/home/projects/pnpm/pnpm-workspace.yaml".to_string(),
                "packages:\n  - 'packages/*'".to_string(),
            ),
            (
                "/home/projects/pnpm/packages/alpha/package.json".to_string(),
                r#"{ "name": "@repo/alpha", "main": "index.ts" }"#.to_string(),
            ),
            (
                "/home/projects/pnpm/packages/alpha/tsconfig.json".to_string(),
                r#"{"compilerOptions":{"noLib":true,"composite":true}}"#.to_string(),
            ),
            (
                "/home/projects/pnpm/packages/alpha/index.ts".to_string(),
                "export const alpha = 1;".to_string(),
            ),
            (
                "/home/projects/pnpm/packages/beta/package.json".to_string(),
                r#"{ "name": "@repo/beta" }"#.to_string(),
            ),
            (
                "/home/projects/pnpm/packages/beta/tsconfig.json".to_string(),
                r#"{"compilerOptions":{"noLib":true}}"#.to_string(),
            ),
            (
                "/home/projects/pnpm/packages/beta/index.ts".to_string(),
                r#"import { alpha } from "@repo/alpha";"#.to_string(),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/pnpm/packages/beta/index.ts".to_string(),
            1,
            files["/home/projects/pnpm/packages/beta/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let mut ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/pnpm/packages/beta/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let mut program = ls.get_program();
        assert_eq!(
            program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/pnpm/packages/beta/index.ts")
                        .as_ref()
                )
                .len(),
            1
        );
        let map_fs = utils
            .fs_from_file_map()
            .expect("map fs should be available");
        map_fs.mkdir_all("home/projects/pnpm/packages/beta/node_modules/@repo");
        map_fs.add_symlink(
            "home/projects/pnpm/packages/beta/node_modules/@repo/alpha",
            "home/projects/pnpm/packages/alpha",
        );
        session.did_change_watched_files(
            core::Context::default(),
            vec![
                file_event(
                    "file:///home/projects/pnpm/packages/beta/node_modules",
                    lsproto::FileChangeType::CREATED,
                ),
                file_event(
                    "file:///home/projects/pnpm/packages/beta/node_modules/%40repo",
                    lsproto::FileChangeType::CREATED,
                ),
                file_event(
                    "file:///home/projects/pnpm/packages/beta/node_modules/%40repo/alpha",
                    lsproto::FileChangeType::CREATED,
                ),
                file_event(
                    "file:///home/projects/pnpm/pnpm-lock.yaml",
                    lsproto::FileChangeType::CREATED,
                ),
                file_event(
                    "file:///home/projects/pnpm/packages/beta/node_modules/.bin/tsc",
                    lsproto::FileChangeType::CHANGED,
                ),
                file_event(
                    "file:///home/projects/pnpm/packages/beta/node_modules/.bin/tsserver",
                    lsproto::FileChangeType::CHANGED,
                ),
            ],
        );
        ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/pnpm/packages/beta/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        program = ls.get_program();
        let diags = program.get_semantic_diagnostics(
            projecttestutil::with_request_id(core::Context::default()),
            program
                .get_source_file("/home/projects/pnpm/packages/beta/index.ts")
                .as_ref(),
        );
        assert_eq!(diags.len(), 0);
    }

    // DidChangeWatchedFiles/symlinked node_modules package.json change invalidates resolution
    {
        let files = HashMap::from([
            (
                "/home/projects/myproject/tsconfig.json".to_string(),
                map_file(
                    r#"{"compilerOptions":{"noLib":true,"module":"nodenext","moduleResolution":"nodenext"},"files":["src/index.ts"]}"#,
                ),
            ),
            (
                "/home/projects/myproject/src/index.ts".to_string(),
                map_file(r#"import { foo } from "mylib";"#),
            ),
            (
                "/home/projects/mylib/package.json".to_string(),
                map_file(r#"{"name":"mylib","main":"dist/index.js"}"#),
            ),
            (
                "/home/projects/mylib/dist/index.js".to_string(),
                map_file("exports.foo = function() { return 1; };"),
            ),
            (
                "/home/projects/mylib/dist/index.d.ts".to_string(),
                map_file("export declare function foo(): number;"),
            ),
            (
                "/home/projects/myproject/node_modules/mylib".to_string(),
                vfstest::symlink("/home/projects/mylib"),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup_map_files_with_options(
            files.clone(),
            setup_options("/home/projects/myproject"),
        );
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/myproject/src/index.ts".to_string(),
            1,
            file_text(&files, "/home/projects/myproject/src/index.ts"),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let mut ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/myproject/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let mut program = ls.get_program();
        session.wait_for_background_tasks();
        assert_eq!(
            program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/myproject/src/index.ts")
                        .as_ref()
                )
                .len(),
            0,
            "import should resolve initially"
        );
        assert!(
            utils.watches_file("/home/projects/mylib/package.json"),
            "realpath of package.json should be watched"
        );
        assert!(
            utils.watches_file("/home/projects/mylib/dist/index.d.ts"),
            "realpath of dist/index.d.ts should be watched"
        );
        utils
            .fs()
            .write_file("/home/projects/mylib/package.json", r#"{"name":"mylib"}"#)
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///home/projects/mylib/package.json",
                lsproto::FileChangeType::CHANGED,
            )],
        );
        ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/myproject/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        program = ls.get_program();
        assert!(
            !program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/myproject/src/index.ts")
                        .as_ref()
                )
                .is_empty(),
            "import should fail after removing main from package.json"
        );
    }

    // DidChangeWatchedFiles/create file in non-existent directory
    {
        let files = HashMap::from([
            (
                "/home/projects/TS/p1/tsconfig.json".to_string(),
                r#"{"compilerOptions":{"noLib":true},"files":["src/index.ts"]}"#.to_string(),
            ),
            (
                "/home/projects/TS/p1/src/index.ts".to_string(),
                r#"import { helper } from "./lib/helper";"#.to_string(),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let mut ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let mut program = ls.get_program();
        assert_eq!(
            program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/TS/p1/src/index.ts")
                        .as_ref()
                )
                .len(),
            1
        );
        utils
            .fs()
            .write_file(
                "/home/projects/TS/p1/src/lib/helper.ts",
                "export const helper = 1;",
            )
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///home/projects/TS/p1/src/lib/helper.ts",
                lsproto::FileChangeType::CREATED,
            )],
        );
        ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        program = ls.get_program();
        assert_eq!(
            program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/TS/p1/src/index.ts")
                        .as_ref()
                )
                .len(),
            0
        );
        assert!(
            program
                .get_source_file("/home/projects/TS/p1/src/lib/helper.ts")
                .is_some()
        );
    }

    // DidChangeWatchedFiles/create symlink directory matching include pattern
    {
        let files = HashMap::from([
            (
                "/home/projects/TS/p1/tsconfig.json".to_string(),
                r#"{"compilerOptions":{"noLib":true},"include":["src"]}"#.to_string(),
            ),
            (
                "/home/projects/TS/p1/src/index.ts".to_string(),
                "export const x = 1;".to_string(),
            ),
            (
                "/home/projects/TS/shared/utils.ts".to_string(),
                r#"export const util = "hello";"#.to_string(),
            ),
            (
                "/home/projects/TS/shared/helpers.ts".to_string(),
                "export const helper = 42;".to_string(),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/src/index.ts".to_string(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let mut ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let mut program = ls.get_program();
        assert!(
            program
                .command_line()
                .file_names
                .contains(&"/home/projects/TS/p1/src/index.ts".to_string())
        );
        assert!(
            !program
                .command_line()
                .file_names
                .contains(&"/home/projects/TS/p1/src/linked/utils.ts".to_string())
        );
        assert!(
            !program
                .command_line()
                .file_names
                .contains(&"/home/projects/TS/p1/src/linked/helpers.ts".to_string())
        );
        utils
            .fs_from_file_map()
            .expect("map fs should be available")
            .add_symlink("home/projects/TS/p1/src/linked", "home/projects/TS/shared");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///home/projects/TS/p1/src/linked",
                lsproto::FileChangeType::CREATED,
            )],
        );
        ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        program = ls.get_program();
        assert!(
            program
                .command_line()
                .file_names
                .contains(&"/home/projects/TS/p1/src/index.ts".to_string())
        );
        assert!(
            program
                .command_line()
                .file_names
                .contains(&"/home/projects/TS/p1/src/linked/utils.ts".to_string())
        );
        assert!(
            program
                .command_line()
                .file_names
                .contains(&"/home/projects/TS/p1/src/linked/helpers.ts".to_string())
        );
    }

    // refreshes code lenses and inlay hints when relevant user preferences change
    {
        let files = HashMap::from([
            ("/src/tsconfig.json".to_string(), "{}".to_string()),
            (
                "/src/index.ts".to_string(),
                "export const x = 1;".to_string(),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///src/index.ts".to_string(),
            1,
            files["/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session
            .get_language_service(core::Context::default(), "file:///src/index.ts".to_string())
            .expect("GetLanguageService should succeed");
        session.configure(lsutil::new_default_user_preferences());
        let mut new_prefs = session.config();
        new_prefs.code_lens.references_code_lens_enabled = core::Tristate::True;
        new_prefs
            .inlay_hints
            .include_inlay_function_like_return_type_hints = core::Tristate::True;
        session.configure(new_prefs);
        assert_eq!(
            utils.client().refresh_code_lens_calls().len(),
            1,
            "expected one RefreshCodeLens call after code lens preference change"
        );
        assert_eq!(
            utils.client().refresh_inlay_hints_calls().len(),
            1,
            "expected one RefreshInlayHints call after inlay hints preference change"
        );
    }

    // config parsing
    {
        let files = HashMap::from([
            ("/src/tsconfig.json".to_string(), "{}".to_string()),
            (
                "/src/index.ts".to_string(),
                "export const x = 1;".to_string(),
            ),
        ]);
        let (mut session, _) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///src/index.ts".to_string(),
            1,
            files["/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session
            .get_language_service(core::Context::default(), "file:///src/index.ts".to_string())
            .expect("GetLanguageService should succeed");
        let config_map1 = serde_json::json!({
            "preferences": { "useAliasesForRenames": true, "quoteStyle": "single" },
            "unstable": { "organizeImportsIgnoreCase": true },
        });
        session.configure(lsutil::parse_user_preferences(HashMap::from([(
            "js/ts".to_string(),
            config_map1,
        )])));
        let mut expected_prefs1 = lsutil::new_default_user_preferences();
        expected_prefs1.use_aliases_for_rename = core::Tristate::True;
        expected_prefs1.quote_preference = lsutil::QuotePreference::Single;
        expected_prefs1.organize_imports_ignore_case = core::Tristate::True;
        assert_eq!(session.config(), expected_prefs1);
        let config_map2 = serde_json::json!({
            "preferences": { "useAliasesForRenames": false, "quoteStyle": "double" },
            "unstable": { "organizeImportsIgnoreCase": false },
        });
        session.configure(lsutil::parse_user_preferences(HashMap::from([(
            "js/ts".to_string(),
            config_map2,
        )])));
        let mut expected_prefs2 = lsutil::new_default_user_preferences();
        expected_prefs2.use_aliases_for_rename = core::Tristate::False;
        expected_prefs2.quote_preference = lsutil::QuotePreference::Double;
        expected_prefs2.organize_imports_ignore_case = core::Tristate::False;
        assert_eq!(session.config(), expected_prefs2);
    }

    // language service for closed files/closed file in configured project not yet opened
    {
        let files = HashMap::from([
            (
                "/home/projects/TS/p1/tsconfig.json".to_string(),
                r#"{"compilerOptions":{"noLib":true,"strict":true},"include":["src"]}"#.to_string(),
            ),
            (
                "/home/projects/TS/p1/src/index.ts".to_string(),
                "export const x: number = 1;".to_string(),
            ),
        ]);
        let (mut session, _) = projecttestutil::setup(files);
        let ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/p1/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let program = ls.get_program();
        let source_file = program
            .get_source_file("/home/projects/TS/p1/src/index.ts")
            .expect("source file should exist");
        assert_eq!(source_file.text(), "export const x: number = 1;");
    }

    // language service for closed files/closed file with no configured project creates inferred project
    {
        let files = HashMap::from([(
            "/home/projects/TS/loose/index.ts".to_string(),
            r#"const greeting: string = "hello";"#.to_string(),
        )]);
        let (mut session, _) = projecttestutil::setup(files);
        let ls = session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/TS/loose/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let program = ls.get_program();
        let source_file = program
            .get_source_file("/home/projects/TS/loose/index.ts")
            .expect("source file should exist");
        assert_eq!(source_file.text(), r#"const greeting: string = "hello";"#);
    }

    // jsconfig.json used for JS files when tsconfig.json exists in same directory
    {
        let files = HashMap::from([
            (
                "/home/projects/TS/p1/tsconfig.json".to_string(),
                r#"{"compilerOptions":{"noLib":true,"strict":true}}"#.to_string(),
            ),
            (
                "/home/projects/TS/p1/jsconfig.json".to_string(),
                r#"{"compilerOptions":{"noLib":true,"checkJs":true}}"#.to_string(),
            ),
            (
                "/home/projects/TS/p1/index.ts".to_string(),
                "export const x: number = 1;".to_string(),
            ),
            (
                "/home/projects/TS/p1/app.js".to_string(),
                r#"/** @type {number} */ var y = "not a number";"#.to_string(),
            ),
        ]);
        let (mut session, _) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/app.js".to_string(),
            1,
            files["/home/projects/TS/p1/app.js"].clone(),
            LANGUAGE_KIND_JAVASCRIPT.to_string(),
        );
        let mut snapshot = session.snapshot();
        let js_uri = lsproto::DocumentUri::from("file:///home/projects/TS/p1/app.js");
        let default_project = snapshot
            .get_default_project(js_uri)
            .expect("JS file should have a default project");
        assert_eq!(
            default_project.name(),
            "/home/projects/TS/p1/jsconfig.json",
            "JS file should belong to jsconfig.json project, not tsconfig.json"
        );
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/TS/p1/index.ts".to_string(),
            1,
            files["/home/projects/TS/p1/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        snapshot = session.snapshot();
        let ts_uri = lsproto::DocumentUri::from("file:///home/projects/TS/p1/index.ts");
        let default_ts_project = snapshot
            .get_default_project(ts_uri)
            .expect("TS file should have a default project");
        assert_eq!(
            default_ts_project.name(),
            "/home/projects/TS/p1/tsconfig.json",
            "TS file should belong to tsconfig.json project"
        );
    }
}

fn file_text(files: &HashMap<String, vfstest::MapFile>, file_name: &str) -> String {
    String::from_utf8(
        files
            .get(file_name)
            .expect("file should exist")
            .data
            .to_vec(),
    )
    .expect("test file content should be UTF-8")
}

fn map_file(text: &str) -> vfstest::MapFile {
    text.to_string()
        .into_map_file(std::time::SystemTime::UNIX_EPOCH)
}
