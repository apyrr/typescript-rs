use std::collections::HashMap;

use crate::projecttestutil;
use ts_core as core;
use ts_lsproto as lsproto;
use ts_vfs::Fs;

const LANGUAGE_KIND_TYPESCRIPT: &str = "typescript";

fn file_event(uri: &str, typ: lsproto::FileChangeType) -> lsproto::FileEvent {
    lsproto::FileEvent {
        uri: lsproto::DocumentUri::from(uri),
        typ,
    }
}

#[test]
fn test_config_file_changes() {
    if !ts_bundled::EMBEDDED {
        return;
    }

    let files = HashMap::from([
        ("/tsconfig.more-base.json".to_string(), "{}".to_string()),
        (
            "/tsconfig.base.json".to_string(),
            r#"{"extends": "../tsconfig.more-base.json", "compilerOptions": {"strict": true}}"#.to_string(),
        ),
        (
            "/src/tsconfig.json".to_string(),
            r#"{"extends": "../tsconfig.base.json", "compilerOptions": {"target": "es6"}, "references": [{"path": "../utils"}]}"#.to_string(),
        ),
        (
            "/src/index.ts".to_string(),
            r#"console.log("Hello, world!");"#.to_string(),
        ),
        (
            "/src/subfolder/foo.ts".to_string(),
            r#"export const foo = "bar";"#.to_string(),
        ),
        (
            "/utils/tsconfig.json".to_string(),
            r#"{"compilerOptions": {"composite": true}}"#.to_string(),
        ),
        (
            "/utils/index.ts".to_string(),
            r#"console.log("Hello, test!");"#.to_string(),
        ),
    ]);

    // should update program options on config file change
    {
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///src/index.ts".to_string(),
            1,
            files["/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );

        utils
            .fs()
            .write_file(
                "/src/tsconfig.json",
                r#"{"extends": "../tsconfig.base.json", "compilerOptions": {"target": "esnext"}, "references": [{"path": "../utils"}]}"#,
            )
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///src/tsconfig.json",
                lsproto::FileChangeType::CHANGED,
            )],
        );

        let ls = session
            .get_language_service(core::Context::default(), "file:///src/index.ts".to_string())
            .expect("GetLanguageService should succeed");
        assert_eq!(
            ls.get_program().compiler_options().target,
            core::ScriptTarget::ESNext
        );
    }

    // should update project on extended config file change
    {
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///src/index.ts".to_string(),
            1,
            files["/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );

        utils
            .fs()
            .write_file(
                "/tsconfig.base.json",
                r#"{"compilerOptions": {"strict": false}}"#,
            )
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///tsconfig.base.json",
                lsproto::FileChangeType::CHANGED,
            )],
        );

        let ls = session
            .get_language_service(core::Context::default(), "file:///src/index.ts".to_string())
            .expect("GetLanguageService should succeed");
        assert_eq!(ls.get_program().compiler_options().strict, core::TS_FALSE);
    }

    // should update project on doubly extended config file change
    {
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///src/index.ts".to_string(),
            1,
            files["/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );

        utils
            .fs()
            .write_file(
                "/tsconfig.more-base.json",
                r#"{"compilerOptions": {"verbatimModuleSyntax": true}}"#,
            )
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///tsconfig.more-base.json",
                lsproto::FileChangeType::CHANGED,
            )],
        );

        let ls = session
            .get_language_service(core::Context::default(), "file:///src/index.ts".to_string())
            .expect("GetLanguageService should succeed");
        assert_eq!(
            ls.get_program().compiler_options().verbatim_module_syntax,
            core::TS_TRUE
        );
    }

    // should update project on referenced config file change
    {
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///src/index.ts".to_string(),
            1,
            files["/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot_before = session.snapshot().id();

        utils
            .fs()
            .write_file(
                "/utils/tsconfig.json",
                r#"{"compilerOptions": {"composite": true, "target": "esnext"}}"#,
            )
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///utils/tsconfig.json",
                lsproto::FileChangeType::CHANGED,
            )],
        );

        session
            .get_language_service(core::Context::default(), "file:///src/index.ts".to_string())
            .expect("GetLanguageService should succeed");
        let snapshot_after = session.snapshot().id();
        assert_ne!(
            snapshot_after, snapshot_before,
            "Snapshot should be updated after config file change"
        );
    }

    // should close project on config file deletion
    {
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///src/index.ts".to_string(),
            1,
            files["/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );

        utils
            .fs()
            .remove("/src/tsconfig.json")
            .expect("Remove should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///src/tsconfig.json",
                lsproto::FileChangeType::DELETED,
            )],
        );

        session
            .get_language_service(core::Context::default(), "file:///src/index.ts".to_string())
            .expect("GetLanguageService should succeed");
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(snapshot.project_collection.inferred_project().is_some());
    }

    // config file creation then deletion
    {
        let (mut session, utils) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///src/subfolder/foo.ts".to_string(),
            1,
            files["/src/subfolder/foo.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );

        utils
            .fs()
            .write_file("/src/subfolder/tsconfig.json", "{}")
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///src/subfolder/tsconfig.json",
                lsproto::FileChangeType::CREATED,
            )],
        );

        session
            .get_language_service(
                core::Context::default(),
                "file:///src/subfolder/foo.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 2);
        assert_eq!(
            snapshot
                .get_default_project(lsproto::DocumentUri::from("file:///src/subfolder/foo.ts"))
                .unwrap()
                .name(),
            "/src/subfolder/tsconfig.json"
        );

        utils
            .fs()
            .remove("/src/subfolder/tsconfig.json")
            .expect("Remove should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///src/subfolder/tsconfig.json",
                lsproto::FileChangeType::DELETED,
            )],
        );

        session
            .get_language_service(
                core::Context::default(),
                "file:///src/subfolder/foo.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        let snapshot = session.snapshot();
        assert_eq!(
            snapshot
                .get_default_project(lsproto::DocumentUri::from("file:///src/subfolder/foo.ts"))
                .unwrap()
                .name(),
            "/src/tsconfig.json"
        );
        assert_eq!(snapshot.project_collection.projects().len(), 2);

        session.did_open_file(
            core::Context::default(),
            "file:///src/index.ts".to_string(),
            1,
            files["/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
    }

    // should update project when missing extended config is created
    {
        let mut missing_base_files = HashMap::new();
        for (key, value) in &files {
            if key == "/tsconfig.base.json" {
                continue;
            }
            missing_base_files.insert(key.clone(), value.clone());
        }

        let (mut session, utils) = projecttestutil::setup(missing_base_files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///src/index.ts".to_string(),
            1,
            missing_base_files["/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );

        utils
            .fs()
            .write_file(
                "/tsconfig.base.json",
                r#"{"compilerOptions": {"strict": true}}"#,
            )
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///tsconfig.base.json",
                lsproto::FileChangeType::CREATED,
            )],
        );

        let ls = session
            .get_language_service(core::Context::default(), "file:///src/index.ts".to_string())
            .expect("GetLanguageService should succeed");
        assert_eq!(ls.get_program().compiler_options().strict, core::TS_TRUE);
    }
}
