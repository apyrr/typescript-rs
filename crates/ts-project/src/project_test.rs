use std::collections::HashMap;
use std::time::Duration;

use crate::projecttestutil;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_lsproto as lsproto;
use ts_tspath as tspath;
use ts_vfs::Fs;

use crate::{self as project, ProgramUpdateKind};

const LANGUAGE_KIND_JAVASCRIPT: &str = "javascript";
const LANGUAGE_KIND_TYPESCRIPT: &str = "typescript";

fn partial_change(
    text: &str,
    start_line: i32,
    start_character: i32,
    end_line: i32,
    end_character: i32,
) -> project::TextDocumentContentChangePartialOrWholeDocument {
    project::TextDocumentContentChangePartialOrWholeDocument {
        partial: Some(lsproto::TextDocumentContentChangePartial {
            text: text.to_string(),
            range: lsproto::Range {
                start: lsproto::Position {
                    line: start_line as u32,
                    character: start_character as u32,
                },
                end: lsproto::Position {
                    line: end_line as u32,
                    character: end_character as u32,
                },
            },
            range_length: None,
        }),
        whole_document: None,
    }
}

#[test]
fn test_project_program_update_kind() {
    if !ts_bundled::EMBEDDED {
        return;
    }

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
        let snapshot = session.snapshot();
        let configured = snapshot
            .project_collection
            .configured_project(tspath::Path::from("/src/tsconfig.json"))
            .expect("configured project should exist");
        assert_eq!(configured.program_update_kind, ProgramUpdateKind::NewFiles);
    }

    {
        let files = HashMap::from([
            ("/src/tsconfig.json".to_string(), "{}".to_string()),
            (
                "/src/index.ts".to_string(),
                "console.log('Hello');".to_string(),
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
        session.did_change_file(
            core::Context::default(),
            "file:///src/index.ts".to_string(),
            2,
            vec![partial_change("\n", 0, 20, 0, 20)],
        );
        session
            .get_language_service(core::Context::default(), "file:///src/index.ts".to_string())
            .expect("GetLanguageService should succeed");
        let snapshot = session.snapshot();
        let configured = snapshot
            .project_collection
            .configured_project(tspath::Path::from("/src/tsconfig.json"))
            .expect("configured project should exist");
        assert_eq!(configured.program_update_kind, ProgramUpdateKind::Cloned);
    }

    {
        let files = HashMap::from([
            (
                "/src/tsconfig.json".to_string(),
                r#"{"compilerOptions": {"strict": true}}"#.to_string(),
            ),
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
        utils
            .fs()
            .write_file(
                "/src/tsconfig.json",
                r#"{"compilerOptions": {"strict": false}}"#,
            )
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![lsproto::FileEvent {
                uri: lsproto::DocumentUri::from("file:///src/tsconfig.json"),
                typ: lsproto::FileChangeType::CHANGED,
            }],
        );
        session
            .get_language_service(core::Context::default(), "file:///src/index.ts".to_string())
            .expect("GetLanguageService should succeed");
        let snapshot = session.snapshot();
        let configured = snapshot
            .project_collection
            .configured_project(tspath::Path::from("/src/tsconfig.json"))
            .expect("configured project should exist");
        assert_eq!(
            configured.program_update_kind,
            ProgramUpdateKind::SameFileNames
        );
    }

    {
        let files = HashMap::from([
            ("/src/tsconfig.json".to_string(), "{}".to_string()),
            ("/src/index.ts".to_string(), "export {}".to_string()),
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
        let content = "export const y = 2;";
        utils
            .fs()
            .write_file("/src/newfile.ts", content)
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![lsproto::FileEvent {
                uri: lsproto::DocumentUri::from("file:///src/newfile.ts"),
                typ: lsproto::FileChangeType::CREATED,
            }],
        );
        session.did_open_file(
            core::Context::default(),
            "file:///src/newfile.ts".to_string(),
            1,
            content.to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session
            .get_language_service(
                core::Context::default(),
                "file:///src/newfile.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let snapshot = session.snapshot();
        let configured = snapshot
            .project_collection
            .configured_project(tspath::Path::from("/src/tsconfig.json"))
            .expect("configured project should exist");
        assert_eq!(configured.program_update_kind, ProgramUpdateKind::NewFiles);
    }

    {
        let files = HashMap::from([
            ("/src/tsconfig.json".to_string(), "{}".to_string()),
            (
                "/src/index.ts".to_string(),
                "export const x = 1;".to_string(),
            ),
            (
                "/src/other.ts".to_string(),
                "export const z = 3;".to_string(),
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
        session.did_change_file(
            core::Context::default(),
            "file:///src/index.ts".to_string(),
            2,
            vec![partial_change(
                "\nimport \"./does-not-exist\";\n",
                0,
                0,
                0,
                0,
            )],
        );
        session
            .get_language_service(core::Context::default(), "file:///src/index.ts".to_string())
            .expect("GetLanguageService should succeed");
        let snapshot = session.snapshot();
        let configured = snapshot
            .project_collection
            .configured_project(tspath::Path::from("/src/tsconfig.json"))
            .expect("configured project should exist");
        assert_eq!(
            configured.program_update_kind,
            ProgramUpdateKind::SameFileNames
        );
    }
}

#[test]
fn test_project() {
    if !ts_bundled::EMBEDDED {
        return;
    }

    let files = HashMap::from([
        (
            "/user/username/projects/project1/app.js".to_string(),
            String::new(),
        ),
        (
            "/user/username/projects/project1/package.json".to_string(),
            r#"{"name":"p1","dependencies":{"jquery":"^3.1.0"}}"#.to_string(),
        ),
        (
            "/user/username/projects/project2/app.js".to_string(),
            String::new(),
        ),
    ]);
    let (mut session, utils) = projecttestutil::setup_with_typings_installer(
        files.clone(),
        projecttestutil::TypingsInstallerOptions {
            package_to_file: HashMap::from([(
                "jquery".to_string(),
                "declare const $: { x: number }".to_string(),
            )]),
            ..Default::default()
        },
    );

    let uri1 = "file:///user/username/projects/project1/app.js";
    session.did_open_file(
        core::Context::default(),
        uri1.to_string(),
        1,
        files["/user/username/projects/project1/app.js"].clone(),
        LANGUAGE_KIND_JAVASCRIPT.to_string(),
    );
    session.wait_for_background_tasks();
    assert!(
        !utils.npm_executor().npm_install_calls().is_empty(),
        "expected at least one npm install call from ATA"
    );
    session
        .get_language_service(core::Context::default(), uri1.to_string())
        .expect("GetLanguageService should succeed");

    let uri2 = "file:///user/username/projects/project2/app.js";
    session.did_open_file(
        core::Context::default(),
        uri2.to_string(),
        1,
        String::new(),
        LANGUAGE_KIND_JAVASCRIPT.to_string(),
    );
    session
        .get_language_service(core::Context::default(), uri2.to_string())
        .expect("GetLanguageService should succeed");
}

#[test]
fn test_push_diagnostics() {
    if !ts_bundled::EMBEDDED {
        return;
    }

    {
        let files = HashMap::from([
            (
                "/src/tsconfig.json".to_string(),
                r#"{"compilerOptions": {"baseUrl": "."}}"#.to_string(),
            ),
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
        session.wait_for_background_tasks();

        let calls = utils.client().publish_diagnostics_calls();
        let tsconfig_call = calls
            .iter()
            .find(|call| call.params.uri == "file:///src/tsconfig.json")
            .expect("expected PublishDiagnostics call for tsconfig.json");
        assert!(
            !tsconfig_call.params.diagnostics.is_empty(),
            "expected at least one diagnostic"
        );
    }

    {
        let files = HashMap::from([
            (
                "/src/tsconfig.json".to_string(),
                r#"{"compilerOptions": {"baseUrl": "."}}"#.to_string(),
            ),
            (
                "/src/index.ts".to_string(),
                "export const x = 1;".to_string(),
            ),
            (
                "/src2/tsconfig.json".to_string(),
                r#"{"compilerOptions": {}}"#.to_string(),
            ),
            (
                "/src2/index.ts".to_string(),
                "export const y = 2;".to_string(),
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
        session.wait_for_background_tasks();

        session.did_close_file(core::Context::default(), "file:///src/index.ts".to_string());
        session.did_open_file(
            core::Context::default(),
            "file:///src2/index.ts".to_string(),
            1,
            files["/src2/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session
            .get_language_service(
                core::Context::default(),
                "file:///src2/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        session.wait_for_background_tasks();

        let calls = utils.client().publish_diagnostics_calls();
        let first_project_calls = calls
            .iter()
            .filter(|call| call.params.uri == "file:///src/tsconfig.json")
            .collect::<Vec<_>>();
        assert!(
            first_project_calls.len() >= 2,
            "expected at least 2 PublishDiagnostics calls for first project"
        );
        let last_call = first_project_calls[first_project_calls.len() - 1];
        assert_eq!(
            last_call.params.diagnostics.len(),
            0,
            "expected empty diagnostics after project cleanup"
        );
    }

    {
        let files = HashMap::from([
            (
                "/src/tsconfig.json".to_string(),
                r#"{"compilerOptions": {"baseUrl": "."}}"#.to_string(),
            ),
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
        session.wait_for_background_tasks();

        let initial_call_count = utils.client().publish_diagnostics_calls().len();
        utils
            .fs()
            .write_file("/src/tsconfig.json", r#"{"compilerOptions": {}}"#)
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![lsproto::FileEvent {
                uri: lsproto::DocumentUri::from("file:///src/tsconfig.json"),
                typ: lsproto::FileChangeType::CHANGED,
            }],
        );
        session
            .get_language_service(core::Context::default(), "file:///src/index.ts".to_string())
            .expect("GetLanguageService should succeed");
        session.wait_for_background_tasks();

        let calls = utils.client().publish_diagnostics_calls();
        assert!(
            calls.len() > initial_call_count,
            "expected additional PublishDiagnostics call after change"
        );
        let last_tsconfig_call = calls
            .iter()
            .rev()
            .find(|call| call.params.uri == "file:///src/tsconfig.json")
            .expect("expected PublishDiagnostics call for tsconfig.json");
        assert_eq!(
            last_tsconfig_call.params.diagnostics.len(),
            0,
            "expected no diagnostics after removing baseUrl option"
        );
    }

    {
        let files = HashMap::from([(
            "/src/index.ts".to_string(),
            "let x: number = 'not a number';".to_string(),
        )]);
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
        session.wait_for_background_tasks();

        assert_eq!(
            utils.client().publish_diagnostics_calls().len(),
            0,
            "expected no PublishDiagnostics calls for inferred projects"
        );
    }

    {
        let files = HashMap::from([
            (
                "/src/tsconfig.json".to_string(),
                r#"{
                "compilerOptions": {
                    "target": "es2020"
                }
            }"#
                .to_string(),
            ),
            (
                "/src/index.ts".to_string(),
                r#"export function f() {
                using x = { [Symbol.dispose]() {} };
            }"#
                .to_string(),
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
        let ls = session
            .get_language_service(
                projecttestutil::with_request_id(core::Context::default()),
                "file:///src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let ctx = projecttestutil::with_request_id(core::Context::default());
        ls.provide_diagnostics(&ctx, lsproto::DocumentUri::from("file:///src/index.ts"))
            .expect("ProvideDiagnostics should succeed");
        session.enqueue_publish_global_diagnostics();
        session.wait_for_background_tasks();

        let calls = utils.client().publish_diagnostics_calls();
        let last_tsconfig_call = calls
            .iter()
            .rev()
            .find(|call| call.params.uri == "file:///src/tsconfig.json")
            .expect("expected PublishDiagnostics call for tsconfig.json");
        assert!(
            last_tsconfig_call
                .params
                .diagnostics
                .iter()
                .any(|diag| diag.message.contains("Cannot find global")),
            "expected a 'Cannot find global' diagnostic on tsconfig.json, got: {:?}",
            last_tsconfig_call.params.diagnostics
        );
    }
}

#[test]
fn test_display_name() {
    if !ts_bundled::EMBEDDED {
        return;
    }

    {
        let files = HashMap::from([
            ("/home/projects/tsconfig.json".to_string(), "{}".to_string()),
            (
                "/home/projects/index.ts".to_string(),
                "export const x = 1;".to_string(),
            ),
        ]);
        let (mut session, _) = projecttestutil::setup(files);
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/index.ts".to_string(),
            1,
            "export const x = 1;".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");

        let snapshot = session.snapshot();
        let configured = snapshot
            .project_collection
            .configured_project(tspath::Path::from("/home/projects/tsconfig.json"))
            .expect("configured project should exist");
        assert_eq!(configured.display_name("/home/projects"), "tsconfig.json");
    }

    {
        let files = HashMap::from([
            (
                "/home/projects/sub/tsconfig.json".to_string(),
                "{}".to_string(),
            ),
            (
                "/home/projects/sub/index.ts".to_string(),
                "export const x = 1;".to_string(),
            ),
        ]);
        let (mut session, _) = projecttestutil::setup(files);
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/sub/index.ts".to_string(),
            1,
            "export const x = 1;".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/sub/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");

        let snapshot = session.snapshot();
        let configured = snapshot
            .project_collection
            .configured_project(tspath::Path::from("/home/projects/sub/tsconfig.json"))
            .expect("configured project should exist");
        assert_eq!(
            configured.display_name("/home/projects"),
            "sub/tsconfig.json"
        );
    }

    {
        let files = HashMap::from([(
            "/home/projects/index.ts".to_string(),
            "export const x = 1;".to_string(),
        )]);
        let (mut session, _) = projecttestutil::setup_with_options(
            files,
            project::SessionOptions {
                current_directory: "/home/projects".to_string(),
                default_library_path: ts_bundled::lib_path(),
                typings_location: projecttestutil::TEST_TYPINGS_LOCATION.to_string(),
                position_encoding: lsproto::PositionEncodingKind::UTF8,
                watch_enabled: true,
                logging_enabled: true,
                telemetry_enabled: false,
                push_diagnostics_enabled: true,
                debounce_delay: Duration::default(),
                locale: ts_locale::Locale::default(),
            },
        );
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/index.ts".to_string(),
            1,
            "export const x = 1;".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");

        let snapshot = session.snapshot();
        let inferred = snapshot
            .project_collection
            .inferred_project()
            .expect("inferred project should exist");
        assert_eq!(inferred.display_name("/home"), "projects");
    }
}

#[test]
fn test_progress_notifications() {
    if !ts_bundled::EMBEDDED {
        return;
    }

    {
        let files = HashMap::from([
            ("/home/projects/tsconfig.json".to_string(), "{}".to_string()),
            (
                "/home/projects/index.ts".to_string(),
                "export const x = 1;".to_string(),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup(files);
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/index.ts".to_string(),
            1,
            "export const x = 1;".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");

        let start_calls = utils.client().progress_start_calls();
        let finish_calls = utils.client().progress_finish_calls();
        assert!(
            !start_calls.is_empty(),
            "expected at least one ProgressStart call"
        );
        assert!(
            !finish_calls.is_empty(),
            "expected at least one ProgressFinish call"
        );
        assert!(
            start_calls
                .iter()
                .any(|call| call.message.code() == diagnostics::Project_0.code()),
            "expected ProgressStart with Project_0 message"
        );
        assert!(
            finish_calls
                .iter()
                .any(|call| call.message.code() == diagnostics::Project_0.code()),
            "expected ProgressFinish with Project_0 message"
        );
    }

    {
        let files = HashMap::from([(
            "/home/projects/index.ts".to_string(),
            "export const x = 1;".to_string(),
        )]);
        let (mut session, utils) = projecttestutil::setup(files);
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/index.ts".to_string(),
            1,
            "export const x = 1;".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");

        let start_calls = utils.client().progress_start_calls();
        let finish_calls = utils.client().progress_finish_calls();
        assert!(
            !start_calls.is_empty(),
            "expected at least one ProgressStart call"
        );
        assert!(
            !finish_calls.is_empty(),
            "expected at least one ProgressFinish call"
        );
        assert!(
            start_calls
                .iter()
                .any(|call| call.message.code() == diagnostics::Project_0.code()),
            "expected ProgressStart with Project_0 message"
        );
    }

    {
        let files = HashMap::from([
            ("/home/projects/tsconfig.json".to_string(), "{}".to_string()),
            (
                "/home/projects/a.ts".to_string(),
                "export const a = 1;".to_string(),
            ),
            (
                "/home/projects/b.ts".to_string(),
                "export const b = 2;".to_string(),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup(files);
        session.did_open_file(
            core::Context::default(),
            "file:///home/projects/a.ts".to_string(),
            1,
            "export const a = 1;".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session
            .get_language_service(
                core::Context::default(),
                "file:///home/projects/a.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");

        let start_calls = utils.client().progress_start_calls();
        let finish_calls = utils.client().progress_finish_calls();
        let starts = start_calls
            .iter()
            .filter(|call| call.message.code() == diagnostics::Project_0.code())
            .count();
        let finishes = finish_calls
            .iter()
            .filter(|call| call.message.code() == diagnostics::Project_0.code())
            .count();
        assert_eq!(
            starts, finishes,
            "ProgressStart and ProgressFinish calls for Project_0 should be balanced"
        );
    }
}
