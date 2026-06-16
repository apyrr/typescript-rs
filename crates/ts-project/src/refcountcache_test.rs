use std::{collections::HashMap, sync::Arc};

use crate::projecttestutil;
use ts_core as core;
use ts_lsproto as lsproto;
use ts_tspath as tspath;

use crate::{
    ProgramUpdateKind, ResourceRequest, Session, SessionInit, SessionOptions, SnapshotChange,
    UpdateReason, new_parse_cache_key, new_session,
};

const MAIN_TS: &str = "/user/username/projects/myproject/src/main.ts";
const MAIN_URI: &str = "file:///user/username/projects/myproject/src/main.ts";
const UTILS_TS: &str = "/user/username/projects/myproject/src/utils.ts";
const UTILS_URI: &str = "file:///user/username/projects/myproject/src/utils.ts";
const UNTITLED_URI: &str = "untitled:Untitled-1";
const LANGUAGE_KIND_TYPESCRIPT: &str = "typescript";

fn partial_change(
    text: &str,
    start_line: u32,
    start_character: u32,
    end_line: u32,
    end_character: u32,
) -> lsproto::TextDocumentContentChangePartialOrWholeDocument {
    lsproto::TextDocumentContentChangePartialOrWholeDocument {
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

fn whole_document_change(text: &str) -> lsproto::TextDocumentContentChangePartialOrWholeDocument {
    lsproto::TextDocumentContentChangePartialOrWholeDocument {
        partial: None,
        whole_document: Some(lsproto::TextDocumentContentChangeWholeDocument {
            text: text.to_string(),
        }),
    }
}

fn setup(files: HashMap<String, String>) -> Session {
    let fs: Arc<dyn ts_vfs::Fs + Send + Sync> =
        Arc::new(ts_bundled::wrap_fs(ts_vfs::vfstest::from_map(files, false)));
    new_session(SessionInit {
        background_ctx: core::Context::default(),
        options: SessionOptions {
            current_directory: "/".to_string(),
            default_library_path: ts_bundled::lib_path(),
            typings_location: projecttestutil::TEST_TYPINGS_LOCATION.to_string(),
            position_encoding: lsproto::PositionEncodingKind::UTF8,
            watch_enabled: false,
            logging_enabled: false,
            telemetry_enabled: false,
            push_diagnostics_enabled: true,
            debounce_delay: std::time::Duration::default(),
            locale: ts_locale::Locale::default(),
        },
        fs,
        client: None,
        logger: Arc::new(crate::logging::new_test_logger()),
        npm_executor: None,
        parse_cache: None,
    })
}

#[test]
fn test_ref_counting_caches() {
    if !ts_bundled::EMBEDDED {
        return;
    }

    // parseCache
    let files = HashMap::from([
        (MAIN_TS.to_string(), "const x = 1;".to_string()),
        (
            UTILS_TS.to_string(),
            "export function util() {}".to_string(),
        ),
    ]);

    // reuse unchanged file
    {
        let mut session = setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            MAIN_URI.to_string(),
            1,
            files[MAIN_TS].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.did_open_file(
            core::Context::default(),
            UTILS_URI.to_string(),
            1,
            files[UTILS_TS].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        let program = snapshot
            .project_collection
            .inferred_project()
            .unwrap()
            .program
            .as_ref()
            .unwrap();
        let main = program.get_source_file(MAIN_TS).unwrap();
        let utils = program.get_source_file(UTILS_TS).unwrap();
        let main_key = new_parse_cache_key(main.parse_options(), main.hash(), main.script_kind());
        let utils_key =
            new_parse_cache_key(utils.parse_options(), utils.hash(), utils.script_kind());
        let main_entry = session.parse_cache.entries.load(&main_key).unwrap();
        let utils_entry = session.parse_cache.entries.load(&utils_key).unwrap();
        assert_eq!(main_entry.ref_count, 1);
        assert_eq!(utils_entry.ref_count, 1);

        session.did_change_file(
            core::Context::default(),
            MAIN_URI.to_string(),
            2,
            vec![partial_change("const x = 2;", 0, 0, 0, 12)],
        );
        let ls = session
            .get_language_service(core::Context::default(), MAIN_URI.to_string())
            .expect("GetLanguageService should succeed");
        session.wait_for_background_tasks();
        let new_main = ls.get_program().get_source_file(MAIN_TS).unwrap();
        let new_main_entry = session
            .parse_cache
            .entries
            .load(&new_parse_cache_key(
                new_main.parse_options(),
                new_main.hash(),
                new_main.script_kind(),
            ))
            .unwrap();
        assert_ne!(new_main.hash(), main.hash());
        assert_ne!(new_main_entry.value.hash(), main_entry.value.hash());
        assert!(ls.get_program().get_source_file(UTILS_TS).unwrap() == utils);
        assert_eq!(main_entry.ref_count, 0);
        assert_eq!(new_main_entry.ref_count, 1);
        assert_eq!(utils_entry.ref_count, 1);
    }

    // release file on close
    {
        let mut session = setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            MAIN_URI.to_string(),
            1,
            files[MAIN_TS].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.did_open_file(
            core::Context::default(),
            UTILS_URI.to_string(),
            1,
            files[UTILS_TS].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        let program = snapshot
            .project_collection
            .inferred_project()
            .unwrap()
            .program
            .as_ref()
            .unwrap();
        let main = program.get_source_file(MAIN_TS).unwrap();
        let utils = program.get_source_file(UTILS_TS).unwrap();
        let main_key = new_parse_cache_key(main.parse_options(), main.hash(), main.script_kind());
        let utils_key =
            new_parse_cache_key(utils.parse_options(), utils.hash(), utils.script_kind());
        let main_entry = session.parse_cache.entries.load(&main_key).unwrap();
        let utils_entry = session.parse_cache.entries.load(&utils_key).unwrap();
        assert_eq!(main_entry.ref_count, 1);
        assert_eq!(utils_entry.ref_count, 1);

        session.did_close_file(core::Context::default(), MAIN_URI.to_string());
        session
            .get_language_service(core::Context::default(), UTILS_URI.to_string())
            .expect("GetLanguageService should succeed");
        session.wait_for_background_tasks();
        assert_eq!(utils_entry.ref_count, 1);
        assert_eq!(main_entry.ref_count, 0);
        assert!(session.parse_cache.entries.load(&main_key).is_none());
    }

    // unchanged program does not over-ref
    {
        let mut session = setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            MAIN_URI.to_string(),
            1,
            files[MAIN_TS].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.did_open_file(
            core::Context::default(),
            UTILS_URI.to_string(),
            1,
            files[UTILS_TS].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );

        let snapshot1 = session.snapshot();
        let program1 = snapshot1
            .project_collection
            .inferred_project()
            .unwrap()
            .program
            .as_ref()
            .unwrap();
        let main = program1.get_source_file(MAIN_TS).unwrap();
        let main_key = new_parse_cache_key(main.parse_options(), main.hash(), main.script_kind());
        let mut main_entry = session.parse_cache.entries.load(&main_key).unwrap();
        assert_eq!(main_entry.ref_count, 1, "initial refCount should be 1");

        session.did_change_file(
            core::Context::default(),
            UTILS_URI.to_string(),
            2,
            vec![partial_change("export function util2() {}", 0, 0, 0, 25)],
        );

        let ls = session
            .get_language_service(core::Context::default(), MAIN_URI.to_string())
            .expect("GetLanguageService should succeed");
        session.wait_for_background_tasks();
        let program2 = ls.get_program();
        let main2 = program2.get_source_file(MAIN_TS).unwrap();
        assert!(main == main2, "main.ts source file should be reused");

        main_entry = session.parse_cache.entries.load(&main_key).unwrap();
        assert_eq!(
            main_entry.ref_count, 1,
            "refCount should be 1 (only new snapshot)"
        );

        session.did_close_file(core::Context::default(), MAIN_URI.to_string());
        session.did_close_file(core::Context::default(), UTILS_URI.to_string());
        session.did_open_file(
            core::Context::default(),
            UNTITLED_URI.to_string(),
            1,
            String::new(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.wait_for_background_tasks();

        assert!(
            session.parse_cache.entries.load(&main_key).is_none(),
            "entry should be deleted after program is disposed"
        );
    }

    // fallback rebuild does not double-ref changed file
    {
        let test_files = HashMap::from([
            (MAIN_TS.to_string(), "const x = 1;".to_string()),
            (UTILS_TS.to_string(), "export const util = 1;".to_string()),
        ]);
        let mut session = setup(test_files.clone());
        session.did_open_file(
            core::Context::default(),
            MAIN_URI.to_string(),
            1,
            test_files[MAIN_TS].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );

        session
            .get_language_service(core::Context::default(), MAIN_URI.to_string())
            .expect("GetLanguageService should succeed");

        session.did_change_file(
            core::Context::default(),
            MAIN_URI.to_string(),
            2,
            vec![whole_document_change(
                "import { util } from \"./utils\";\nconst x = util;",
            )],
        );

        let ls_after = session
            .get_language_service(core::Context::default(), MAIN_URI.to_string())
            .expect("GetLanguageService should succeed");
        session.wait_for_background_tasks();

        let project = session.snapshot().project_collection.inferred_project();
        assert!(project.is_some());
        let project = project.unwrap();
        assert_eq!(project.program_update_kind, ProgramUpdateKind::NewFiles);

        let main = ls_after.get_program().get_source_file(MAIN_TS).unwrap();
        let main_key = new_parse_cache_key(main.parse_options(), main.hash(), main.script_kind());
        let main_entry = session.parse_cache.entries.load(&main_key).unwrap();
        assert_eq!(main_entry.ref_count, 1);

        session.did_close_file(core::Context::default(), MAIN_URI.to_string());
        session.did_open_file(
            core::Context::default(),
            UNTITLED_URI.to_string(),
            1,
            String::new(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.wait_for_background_tasks();

        assert!(session.parse_cache.entries.load(&main_key).is_none());
    }

    // case-only duplicate loads are released on dispose
    {
        let test_files = HashMap::from([
            (
                MAIN_TS.to_string(),
                "import { util as a } from \"./utils\";\nimport { util as b } from \"./UTILS\";\nconst x = a + b;"
                    .to_string(),
            ),
            (UTILS_TS.to_string(), "export const util = 1;".to_string()),
        ]);
        let mut session = setup(test_files.clone());
        session.did_open_file(
            core::Context::default(),
            MAIN_URI.to_string(),
            1,
            test_files[MAIN_TS].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );

        let ls = session
            .get_language_service(core::Context::default(), MAIN_URI.to_string())
            .expect("GetLanguageService should succeed");

        let mut project_entries = 0;
        session.parse_cache.entries.range(|key, _| {
            if key
                .source_file_parse_options
                .file_name
                .starts_with("/user/username/projects/myproject/src/")
            {
                project_entries += 1;
            }
            true
        });
        assert_eq!(project_entries, 3);

        let utils = ls.get_program().get_source_file(UTILS_TS);
        assert!(utils.is_some());

        session.did_close_file(core::Context::default(), MAIN_URI.to_string());
        session.did_open_file(
            core::Context::default(),
            UNTITLED_URI.to_string(),
            1,
            String::new(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.wait_for_background_tasks();

        project_entries = 0;
        session.parse_cache.entries.range(|key, _| {
            if key
                .source_file_parse_options
                .file_name
                .starts_with("/user/username/projects/myproject/src/")
            {
                project_entries += 1;
            }
            true
        });
        assert_eq!(project_entries, 0);
    }

    // extendedConfigCache
    let files = HashMap::from([
        (
            "/user/username/projects/myproject/tsconfig.json".to_string(),
            r#"{
                "extends": "./tsconfig.base.json"
            }"#
            .to_string(),
        ),
        (
            "/user/username/projects/myproject/tsconfig.base.json".to_string(),
            r#"{
                "compilerOptions": {}
            }"#
            .to_string(),
        ),
        (MAIN_TS.to_string(), "const x = 1;".to_string()),
    ]);

    // release extended configs with project close
    {
        let mut session = setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            MAIN_URI.to_string(),
            1,
            files[MAIN_TS].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        let config = snapshot
            .config_file_registry
            .as_ref()
            .unwrap()
            .get_config("/user/username/projects/myproject/tsconfig.json".to_string());
        let config = config.expect("config should exist");
        assert_eq!(
            config.extended_source_files()[0],
            "/user/username/projects/myproject/tsconfig.base.json"
        );
        let extended_config_entry = session
            .extended_config_cache
            .entries
            .load("/user/username/projects/myproject/tsconfig.base.json")
            .unwrap();
        assert_eq!(extended_config_entry.owners.len(), 1);

        session.did_close_file(core::Context::default(), MAIN_URI.to_string());
        session.did_open_file(
            core::Context::default(),
            UNTITLED_URI.to_string(),
            1,
            String::new(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.wait_for_background_tasks();
        assert!(
            session
                .extended_config_cache
                .entries
                .load("/user/username/projects/myproject/tsconfig.base.json")
                .is_none()
        );
    }

    // release cache entries for unretained clone
    {
        let mut session = setup(files);
        let uri = MAIN_URI.to_string();
        let base_snapshot = session.snapshot().clone_snapshot_value();
        let extended_config_path =
            tspath::Path::from("/user/username/projects/myproject/tsconfig.base.json");
        let clone = base_snapshot.clone_snapshot(
            core::Context::default(),
            SnapshotChange {
                reason: UpdateReason::RequestedLanguageServiceProjectNotLoaded,
                resource_request: ResourceRequest {
                    documents: vec![uri.clone()],
                    ..Default::default()
                },
                ..Default::default()
            },
            base_snapshot.fs.overlays.clone(),
            &mut session,
        );

        let project = clone.get_default_project(uri.clone());
        assert!(project.is_some());
        let project = project.unwrap();
        assert_eq!(project.program_last_update, clone.id());

        let main = project
            .program
            .as_ref()
            .unwrap()
            .get_source_file(MAIN_TS)
            .unwrap();
        let main_key = new_parse_cache_key(main.parse_options(), main.hash(), main.script_kind());
        let main_entry = session.parse_cache.entries.load(&main_key).unwrap();
        assert_eq!(main_entry.ref_count, 1);

        let extended_config_entry = session
            .extended_config_cache
            .entries
            .load(&extended_config_path)
            .unwrap();
        assert_eq!(extended_config_entry.owners.len(), 1);

        clone.deref(&mut session);

        assert!(session.parse_cache.entries.load(&main_key).is_none());
        assert!(
            session
                .extended_config_cache
                .entries
                .load(&extended_config_path)
                .is_none()
        );
    }
}
