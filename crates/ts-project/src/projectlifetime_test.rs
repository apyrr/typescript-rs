use std::collections::HashMap;

use crate::projecttestutil;
use ts_core as core;
use ts_lsproto as lsproto;
use ts_tspath as tspath;
use ts_vfs::Fs;

const LANGUAGE_KIND_TYPESCRIPT: &str = "typescript";

#[test]
fn test_project_lifetime() {
    if !ts_bundled::EMBEDDED {
        return;
    }

    // configured project
    {
        let files = HashMap::from([
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
            (
                "/home/projects/TS/p2/tsconfig.json".to_string(),
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
                "/home/projects/TS/p2/src/index.ts".to_string(),
                r#"import { x } from "./x";"#.to_string(),
            ),
            (
                "/home/projects/TS/p2/src/x.ts".to_string(),
                "export const x = 1;".to_string(),
            ),
            (
                "/home/projects/TS/p2/config.ts".to_string(),
                "let x = 1, y = 2;".to_string(),
            ),
            (
                "/home/projects/TS/p3/tsconfig.json".to_string(),
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
                "/home/projects/TS/p3/src/index.ts".to_string(),
                r#"import { x } from "./x";"#.to_string(),
            ),
            (
                "/home/projects/TS/p3/src/x.ts".to_string(),
                "export const x = 1;".to_string(),
            ),
            (
                "/home/projects/TS/p3/config.ts".to_string(),
                "let x = 1, y = 2;".to_string(),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup(files.clone());
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 0);

        let uri1 = "file:///home/projects/TS/p1/src/index.ts".to_string();
        let uri2 = "file:///home/projects/TS/p2/src/index.ts".to_string();
        session.did_open_file(
            core::Context::default(),
            uri1.clone(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.did_open_file(
            core::Context::default(),
            uri2.clone(),
            1,
            files["/home/projects/TS/p2/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.wait_for_background_tasks();
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 2);
        assert!(
            snapshot
                .project_collection
                .configured_project(tspath::Path::from("/home/projects/ts/p1/tsconfig.json"))
                .is_some()
        );
        assert!(
            snapshot
                .project_collection
                .configured_project(tspath::Path::from("/home/projects/ts/p2/tsconfig.json"))
                .is_some()
        );
        assert_eq!(utils.client().watch_files_calls().len(), 1);
        assert!(
            snapshot
                .config_file_registry
                .as_ref()
                .unwrap()
                .get_config(tspath::Path::from("/home/projects/ts/p1/tsconfig.json"))
                .is_some()
        );
        assert!(
            snapshot
                .config_file_registry
                .as_ref()
                .unwrap()
                .get_config(tspath::Path::from("/home/projects/ts/p2/tsconfig.json"))
                .is_some()
        );

        session.did_close_file(core::Context::default(), uri1.clone());
        let uri3 = "file:///home/projects/TS/p3/src/index.ts".to_string();
        session.did_open_file(
            core::Context::default(),
            uri3.clone(),
            1,
            files["/home/projects/TS/p3/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.wait_for_background_tasks();
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 2);
        assert!(
            snapshot
                .project_collection
                .configured_project(tspath::Path::from("/home/projects/ts/p1/tsconfig.json"))
                .is_none()
        );
        assert!(
            snapshot
                .project_collection
                .configured_project(tspath::Path::from("/home/projects/ts/p2/tsconfig.json"))
                .is_some()
        );
        assert!(
            snapshot
                .project_collection
                .configured_project(tspath::Path::from("/home/projects/ts/p3/tsconfig.json"))
                .is_some()
        );
        assert!(
            snapshot
                .config_file_registry
                .as_ref()
                .unwrap()
                .get_config(tspath::Path::from("/home/projects/ts/p1/tsconfig.json"))
                .is_none()
        );
        assert!(
            snapshot
                .config_file_registry
                .as_ref()
                .unwrap()
                .get_config(tspath::Path::from("/home/projects/ts/p2/tsconfig.json"))
                .is_some()
        );
        assert!(
            snapshot
                .config_file_registry
                .as_ref()
                .unwrap()
                .get_config(tspath::Path::from("/home/projects/ts/p3/tsconfig.json"))
                .is_some()
        );
        assert_eq!(utils.client().watch_files_calls().len(), 1);
        assert_eq!(utils.client().unwatch_files_calls().len(), 0);

        session.did_close_file(core::Context::default(), uri2.clone());
        session.did_close_file(core::Context::default(), uri3);
        session.did_open_file(
            core::Context::default(),
            uri1,
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.wait_for_background_tasks();
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(
            snapshot
                .project_collection
                .configured_project(tspath::Path::from("/home/projects/ts/p1/tsconfig.json"))
                .is_some()
        );
        assert!(
            snapshot
                .config_file_registry
                .as_ref()
                .unwrap()
                .get_config(tspath::Path::from("/home/projects/ts/p1/tsconfig.json"))
                .is_some()
        );
        assert!(
            snapshot
                .config_file_registry
                .as_ref()
                .unwrap()
                .get_config(tspath::Path::from("/home/projects/ts/p2/tsconfig.json"))
                .is_none()
        );
        assert!(
            snapshot
                .config_file_registry
                .as_ref()
                .unwrap()
                .get_config(tspath::Path::from("/home/projects/ts/p3/tsconfig.json"))
                .is_none()
        );
        assert_eq!(utils.client().watch_files_calls().len(), 1);
        assert_eq!(utils.client().unwatch_files_calls().len(), 0);
    }

    // unrooted inferred projects
    {
        let files = HashMap::from([
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
            (
                "/home/projects/TS/p2/src/index.ts".to_string(),
                r#"import { x } from "./x";"#.to_string(),
            ),
            (
                "/home/projects/TS/p2/src/x.ts".to_string(),
                "export const x = 1;".to_string(),
            ),
            (
                "/home/projects/TS/p2/config.ts".to_string(),
                "let x = 1, y = 2;".to_string(),
            ),
            (
                "/home/projects/TS/p3/src/index.ts".to_string(),
                r#"import { x } from "./x";"#.to_string(),
            ),
            (
                "/home/projects/TS/p3/src/x.ts".to_string(),
                "export const x = 1;".to_string(),
            ),
            (
                "/home/projects/TS/p3/config.ts".to_string(),
                "let x = 1, y = 2;".to_string(),
            ),
        ]);
        let (mut session, _) = projecttestutil::setup(files.clone());
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 0);

        let uri1 = "file:///home/projects/TS/p1/src/index.ts".to_string();
        let uri2 = "file:///home/projects/TS/p2/src/index.ts".to_string();
        session.did_open_file(
            core::Context::default(),
            uri1.clone(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.did_open_file(
            core::Context::default(),
            uri2.clone(),
            1,
            files["/home/projects/TS/p2/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(snapshot.project_collection.inferred_project().is_some());

        session.did_close_file(core::Context::default(), uri1.clone());
        let uri3 = "file:///home/projects/TS/p3/src/index.ts".to_string();
        session.did_open_file(
            core::Context::default(),
            uri3.clone(),
            1,
            files["/home/projects/TS/p3/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(snapshot.project_collection.inferred_project().is_some());

        session.did_close_file(core::Context::default(), uri2);
        session.did_close_file(core::Context::default(), uri3);
        session.did_open_file(
            core::Context::default(),
            uri1,
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(snapshot.project_collection.inferred_project().is_some());
    }

    // file moves from inferred to configured project
    {
        let files = HashMap::from([
            (
                "/home/projects/ts/foo.ts".to_string(),
                "export const foo = 1;".to_string(),
            ),
            (
                "/home/projects/ts/p1/tsconfig.json".to_string(),
                r#"{
                "compilerOptions": {
                    "noLib": true,
                    "module": "nodenext",
                    "strict": true
                },
                "include": ["main.ts"]
            }"#
                .to_string(),
            ),
            (
                "/home/projects/ts/p1/main.ts".to_string(),
                r#"import { foo } from "../foo"; console.log(foo);"#.to_string(),
            ),
        ]);
        let (mut session, _) = projecttestutil::setup(files.clone());

        let foo_uri = "file:///home/projects/ts/foo.ts".to_string();
        session.did_open_file(
            core::Context::default(),
            foo_uri.clone(),
            1,
            files["/home/projects/ts/foo.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(snapshot.project_collection.inferred_project().is_some());
        assert!(
            snapshot
                .project_collection
                .configured_project(tspath::Path::from("/home/projects/ts/p1/tsconfig.json"))
                .is_none()
        );

        let main_uri = "file:///home/projects/ts/p1/main.ts".to_string();
        session.did_open_file(
            core::Context::default(),
            main_uri.clone(),
            1,
            files["/home/projects/ts/p1/main.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(snapshot.project_collection.inferred_project().is_none());
        assert!(
            snapshot
                .project_collection
                .configured_project(tspath::Path::from("/home/projects/ts/p1/tsconfig.json"))
                .is_some()
        );
        assert!(
            snapshot
                .config_file_registry
                .as_ref()
                .unwrap()
                .get_config(tspath::Path::from("/home/projects/ts/p1/tsconfig.json"))
                .is_some()
        );

        session.did_close_file(core::Context::default(), main_uri);
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(
            snapshot
                .project_collection
                .configured_project(tspath::Path::from("/home/projects/ts/p1/tsconfig.json"))
                .is_some()
        );

        session.did_close_file(core::Context::default(), foo_uri);
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(
            snapshot
                .config_file_registry
                .as_ref()
                .unwrap()
                .get_config(tspath::Path::from("/home/projects/ts/p1/tsconfig.json"))
                .is_some()
        );
    }

    // file move from inferred to configured via didOpen/didClose sequence
    {
        let files = HashMap::from([
            (
                "/home/projects/TS/p1/tsconfig.json".to_string(),
                r#"{
                "compilerOptions": {
                    "noLib": true
                },
                "include": ["src"]
            }"#
                .to_string(),
            ),
            (
                "/home/projects/TS/p1/index.ts".to_string(),
                "export const x = 1;".to_string(),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup(files.clone());

        let index_uri = "file:///home/projects/TS/p1/index.ts".to_string();
        session.did_open_file(
            core::Context::default(),
            index_uri.clone(),
            1,
            files["/home/projects/TS/p1/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(snapshot.project_collection.inferred_project().is_some());
        assert!(
            snapshot
                .project_collection
                .configured_project(tspath::Path::from("/home/projects/ts/p1/tsconfig.json"))
                .is_none()
        );

        utils
            .fs()
            .write_file(
                "/home/projects/TS/p1/src/index.ts",
                &files["/home/projects/TS/p1/index.ts"],
            )
            .expect("WriteFile should succeed");
        utils
            .fs()
            .remove("/home/projects/TS/p1/index.ts")
            .expect("Remove should succeed");

        let src_index_uri = "file:///home/projects/TS/p1/src/index.ts".to_string();
        session.did_open_file(
            core::Context::default(),
            src_index_uri.clone(),
            1,
            files["/home/projects/TS/p1/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.did_close_file(core::Context::default(), index_uri.clone());
        session.did_change_watched_files(
            core::Context::default(),
            vec![
                lsproto::FileEvent {
                    uri: lsproto::DocumentUri::from(src_index_uri.clone()),
                    typ: lsproto::FileChangeType::CREATED,
                },
                lsproto::FileEvent {
                    uri: lsproto::DocumentUri::from(index_uri),
                    typ: lsproto::FileChangeType::DELETED,
                },
            ],
        );

        session
            .get_language_service(core::Context::default(), src_index_uri)
            .expect("GetLanguageService should succeed");
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(snapshot.project_collection.inferred_project().is_none());
        assert!(
            snapshot
                .project_collection
                .configured_project(tspath::Path::from("/home/projects/ts/p1/tsconfig.json"))
                .is_some()
        );
    }

    // tsconfig move from subdirectory to parent via didChangeWatchedFiles
    {
        let files = HashMap::from([
            (
                "/home/projects/TS/p1/src/tsconfig.json".to_string(),
                r#"{
                "compilerOptions": {
                    "noLib": true
                },
                "include": ["src"]
            }"#
                .to_string(),
            ),
            (
                "/home/projects/TS/p1/src/index.ts".to_string(),
                "export const x = 1;".to_string(),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup(files.clone());

        let index_uri = "file:///home/projects/TS/p1/src/index.ts".to_string();
        session.did_open_file(
            core::Context::default(),
            index_uri.clone(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(snapshot.project_collection.inferred_project().is_some());
        assert!(
            snapshot
                .project_collection
                .configured_project(tspath::Path::from("/home/projects/ts/p1/src/tsconfig.json"))
                .is_none()
        );

        let tsconfig_content = files["/home/projects/TS/p1/src/tsconfig.json"].clone();
        utils
            .fs()
            .write_file("/home/projects/TS/p1/tsconfig.json", &tsconfig_content)
            .expect("WriteFile should succeed");
        utils
            .fs()
            .remove("/home/projects/TS/p1/src/tsconfig.json")
            .expect("Remove should succeed");

        let new_tsconfig_uri = "file:///home/projects/TS/p1/tsconfig.json".to_string();
        let old_tsconfig_uri = "file:///home/projects/TS/p1/src/tsconfig.json".to_string();
        session.did_change_watched_files(
            core::Context::default(),
            vec![
                lsproto::FileEvent {
                    uri: lsproto::DocumentUri::from(new_tsconfig_uri),
                    typ: lsproto::FileChangeType::CREATED,
                },
                lsproto::FileEvent {
                    uri: lsproto::DocumentUri::from(old_tsconfig_uri),
                    typ: lsproto::FileChangeType::DELETED,
                },
            ],
        );

        session
            .get_language_service(core::Context::default(), index_uri)
            .expect("GetLanguageService should succeed");
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(snapshot.project_collection.inferred_project().is_none());
        assert!(
            snapshot
                .project_collection
                .configured_project(tspath::Path::from("/home/projects/ts/p1/tsconfig.json"))
                .is_some()
        );
    }

    // deleted open file remains in project until closed
    {
        let files = HashMap::from([
            (
                "/home/projects/TS/p1/tsconfig.json".to_string(),
                r#"{
                "compilerOptions": {
                    "noLib": true
                },
                "include": ["src"]
            }"#
                .to_string(),
            ),
            (
                "/home/projects/TS/p1/src/index.ts".to_string(),
                String::new(),
            ),
            (
                "/home/projects/TS/p1/src/x.ts".to_string(),
                "export const x = 1;".to_string(),
            ),
        ]);
        let (mut session, utils) = projecttestutil::setup(files.clone());

        let index_uri = "file:///home/projects/TS/p1/src/index.ts".to_string();
        let x_uri = "file:///home/projects/TS/p1/src/x.ts".to_string();
        session.did_open_file(
            core::Context::default(),
            index_uri.clone(),
            1,
            files["/home/projects/TS/p1/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.did_open_file(
            core::Context::default(),
            x_uri.clone(),
            1,
            files["/home/projects/TS/p1/src/x.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );

        let language_service = session
            .get_language_service(core::Context::default(), index_uri.clone())
            .expect("GetLanguageService should succeed");
        let program = language_service.get_program();
        assert!(
            program
                .get_source_file("/home/projects/TS/p1/src/index.ts")
                .is_some(),
            "index.ts should be in project"
        );
        assert!(
            program
                .get_source_file("/home/projects/TS/p1/src/x.ts")
                .is_some(),
            "x.ts should be in project"
        );

        utils
            .fs()
            .remove("/home/projects/TS/p1/src/x.ts")
            .expect("Remove should succeed");
        utils
            .fs()
            .write_file("/home/projects/TS/p1/src/y.ts", "export const y = 2;")
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![
                lsproto::FileEvent {
                    uri: lsproto::DocumentUri::from(x_uri.clone()),
                    typ: lsproto::FileChangeType::DELETED,
                },
                lsproto::FileEvent {
                    uri: lsproto::DocumentUri::from("file:///home/projects/TS/p1/src/y.ts"),
                    typ: lsproto::FileChangeType::CREATED,
                },
            ],
        );

        let language_service = session
            .get_language_service(core::Context::default(), x_uri.clone())
            .expect("GetLanguageService should succeed");
        let program = language_service.get_program();
        assert!(
            program
                .get_source_file("/home/projects/TS/p1/src/index.ts")
                .is_some(),
            "index.ts should still be in project"
        );
        assert!(
            program
                .get_source_file("/home/projects/TS/p1/src/x.ts")
                .is_some(),
            "x.ts should still be in project (open overlay)"
        );
        assert!(
            program
                .get_source_file("/home/projects/TS/p1/src/y.ts")
                .is_some(),
            "y.ts should be in project (new file)"
        );

        session.did_close_file(core::Context::default(), x_uri);
        let language_service = session
            .get_language_service(core::Context::default(), index_uri)
            .expect("GetLanguageService should succeed");
        let program = language_service.get_program();
        assert!(
            program
                .get_source_file("/home/projects/TS/p1/src/index.ts")
                .is_some(),
            "index.ts should still be in project"
        );
        assert!(
            program
                .get_source_file("/home/projects/TS/p1/src/x.ts")
                .is_none(),
            "x.ts should no longer be in project (closed and deleted)"
        );
        assert!(
            program
                .get_source_file("/home/projects/TS/p1/src/y.ts")
                .is_some(),
            "y.ts should still be in project"
        );
    }
}
