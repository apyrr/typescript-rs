use std::collections::HashMap;

use ts_core as core;
use ts_ls as lsconv;
use ts_ls as lsutil;
use ts_project as project;
use ts_testutil::projecttestutil;
use ts_vfs::Fs;

use crate::lsproto;

const LANGUAGE_KIND_TYPESCRIPT: &str = "typescript";

fn init_mutable_lsp_client(
    files: HashMap<String, String>,
    prefs: lsutil::UserPreferences,
) -> (project::Session, projecttestutil::SessionUtils) {
    let (mut session, utils) = projecttestutil::setup(files);
    session.initialize_with_user_config(prefs);
    (session, utils)
}

#[test]
fn test_references_after_ancestor_project_config_deletion_1() {
    if !ts_bundled::EMBEDDED {
        return;
    }

    let files = HashMap::from([
        (
            "/root/tsconfig.json".to_string(),
            r#"{
			"files": [],
			"references": [{ "path": "./project" }]
		}"#
            .to_string(),
        ),
        (
            "/root/project/tsconfig.json".to_string(),
            r#"{
			"compilerOptions": { "composite": true },
			"include": ["src/**/*.ts"]
		}"#
            .to_string(),
        ),
        (
            "/root/project/src/main.ts".to_string(),
            "export function helloWorld() {}\nhelloWorld()\n".to_string(),
        ),
    ]);
    let (mut session, utils) = init_mutable_lsp_client(files, lsutil::UserPreferences::default());

    let ctx = core::Context::default();
    let main_uri = lsconv::file_name_to_document_uri("/root/project/src/main.ts");
    session.did_open_file(
        ctx.clone(),
        main_uri.clone(),
        1,
        "export function helloWorld() {}\nhelloWorld()\n".to_string(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );

    // Prime the child project so opening a file creates the ancestor configured-project placeholder.
    let language_service = session
        .get_language_service(ctx.clone(), main_uri.clone())
        .expect("expected response");
    let _ = language_service
        .provide_document_symbols(&ctx, main_uri.clone())
        .expect("documentSymbol should succeed");

    utils
        .fs()
        .remove("/root/tsconfig.json")
        .expect("Remove should succeed");
    session.did_change_watched_files(
        ctx.clone(),
        vec![lsproto::FileEvent {
            uri: lsconv::file_name_to_document_uri("/root/tsconfig.json"),
            typ: lsproto::FileChangeType::Deleted,
        }],
    );

    let language_service = session
        .get_language_service(ctx.clone(), main_uri.clone())
        .expect("expected response");
    let resp = language_service
        .provide_references(
            &ctx,
            &lsproto::ReferenceParams {
                text_document: lsproto::TextDocumentIdentifier {
                    uri: main_uri.clone(),
                },
                position: lsproto::Position {
                    line: 1,
                    character: 3,
                },
                work_done_token: None,
                partial_result_token: None,
                context: lsproto::ReferenceContext {
                    include_declaration: true,
                },
            },
            None,
        )
        .expect("references should succeed");

    let locations = resp
        .locations
        .expect("references response should contain locations");
    assert_eq!(locations.len(), 2);
    assert_eq!(
        locations,
        vec![
            lsproto::Location {
                uri: main_uri.clone(),
                range: lsproto::Range {
                    start: lsproto::Position {
                        line: 0,
                        character: 16,
                    },
                    end: lsproto::Position {
                        line: 0,
                        character: 26,
                    },
                },
            },
            lsproto::Location {
                uri: main_uri,
                range: lsproto::Range {
                    start: lsproto::Position {
                        line: 1,
                        character: 0,
                    },
                    end: lsproto::Position {
                        line: 1,
                        character: 10,
                    },
                },
            },
        ]
    );
}
