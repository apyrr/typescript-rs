use std::collections::HashMap;

use crate::projecttestutil;
use ts_core as core;
use ts_lsproto as lsproto;
use ts_tspath as tspath;
use ts_vfs::Fs;

use crate::{self as project, Kind};

const LANGUAGE_KIND_TYPESCRIPT: &str = "typescript";

fn file_event(uri: &str, typ: lsproto::FileChangeType) -> lsproto::FileEvent {
    lsproto::FileEvent {
        uri: lsproto::DocumentUri::from(uri),
        typ,
    }
}

#[test]
fn test_project_collection_builder() {
    if !ts_bundled::EMBEDDED {
        return;
    }

    // when project found is solution referencing default project directly
    {
        let files = files_for_solution_config_file(&["./tsconfig-src.json"], "", &[]);
        let (mut session, _) = projecttestutil::setup(files.clone());
        let uri =
            lsproto::DocumentUri::from("file:///user/username/projects/myproject/src/main.ts");
        let content = files["/user/username/projects/myproject/src/main.ts"].clone();

        // Ensure configured project is found for open file
        session.did_open_file(
            core::Context::default(),
            uri.clone(),
            1,
            content,
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(
            snapshot
                .project_collection
                .configured_project(tspath::Path::from(
                    "/user/username/projects/myproject/tsconfig-src.json",
                ))
                .is_some()
        );

        // Ensure request can use existing snapshot
        let snapshot_ptr = snapshot as *const _;
        session
            .get_language_service(core::Context::default(), uri.clone())
            .expect("GetLanguageService should succeed");
        let request_snapshot = session.snapshot();
        assert_eq!(request_snapshot as *const _, snapshot_ptr);

        // Searched configs should be present while file is open
        assert!(
            request_snapshot
                .config_file_registry
                .as_ref()
                .unwrap()
                .get_config(tspath::Path::from(
                    "/user/username/projects/myproject/tsconfig.json",
                ))
                .is_some(),
            "solution config should be present"
        );
        assert!(
            request_snapshot
                .config_file_registry
                .as_ref()
                .unwrap()
                .get_config(tspath::Path::from(
                    "/user/username/projects/myproject/tsconfig-src.json",
                ))
                .is_some(),
            "direct reference should be present"
        );

        // Close the file and open one in an inferred project
        session.did_close_file(core::Context::default(), uri);
        let dummy_uri =
            lsproto::DocumentUri::from("file:///user/username/workspaces/dummy/dummy.ts");
        session.did_open_file(
            core::Context::default(),
            dummy_uri,
            1,
            "const x = 1;".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(snapshot.project_collection.inferred_project().is_some());

        // Config files should have been released
        assert!(
            snapshot
                .config_file_registry
                .as_ref()
                .unwrap()
                .get_config(tspath::Path::from(
                    "/user/username/projects/myproject/tsconfig.json",
                ))
                .is_none()
        );
        assert!(
            snapshot
                .config_file_registry
                .as_ref()
                .unwrap()
                .get_config(tspath::Path::from(
                    "/user/username/projects/myproject/tsconfig-src.json",
                ))
                .is_none()
        );
    }

    // when project found is solution referencing default project indirectly
    {
        let mut files = files_for_solution_config_file(
            &["./tsconfig-indirect1.json", "./tsconfig-indirect2.json"],
            "",
            &[],
        );
        apply_indirect_project_files(&mut files, 1, "");
        apply_indirect_project_files(&mut files, 2, "");
        let (mut session, _) = projecttestutil::setup(files.clone());
        let uri =
            lsproto::DocumentUri::from("file:///user/username/projects/myproject/src/main.ts");
        let content = files["/user/username/projects/myproject/src/main.ts"].clone();

        // Ensure configured project is found for open file
        session.did_open_file(
            core::Context::default(),
            uri.clone(),
            1,
            content,
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        let src_project = snapshot
            .project_collection
            .configured_project(tspath::Path::from(
                "/user/username/projects/myproject/tsconfig-src.json",
            ))
            .expect("source project should exist");

        // Verify the default project is the source project
        let default_project = snapshot.get_default_project(uri.clone());
        assert!(std::ptr::eq(default_project.unwrap(), src_project));

        // Searched configs should be present while file is open
        assert_config_present(
            snapshot,
            "/user/username/projects/myproject/tsconfig.json",
            "solution config should be present",
        );
        assert_config_present(
            snapshot,
            "/user/username/projects/myproject/tsconfig-indirect1.json",
            "direct reference should be present",
        );
        assert_config_present(
            snapshot,
            "/user/username/projects/myproject/tsconfig-src.json",
            "indirect reference should be present",
        );

        // Close the file and open one in an inferred project
        session.did_close_file(core::Context::default(), uri);
        let dummy_uri =
            lsproto::DocumentUri::from("file:///user/username/workspaces/dummy/dummy.ts");
        session.did_open_file(
            core::Context::default(),
            dummy_uri,
            1,
            "const x = 1;".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(snapshot.project_collection.inferred_project().is_some());

        // Config files should be released
        assert_config_absent(snapshot, "/user/username/projects/myproject/tsconfig.json");
        assert_config_absent(
            snapshot,
            "/user/username/projects/myproject/tsconfig-src.json",
        );
        assert_config_absent(
            snapshot,
            "/user/username/projects/myproject/tsconfig-indirect1.json",
        );
        assert_config_absent(
            snapshot,
            "/user/username/projects/myproject/tsconfig-indirect2.json",
        );
    }

    // when project found is solution with disableReferencedProjectLoad referencing default project directly
    {
        let files = files_for_solution_config_file(
            &["./tsconfig-src.json"],
            r#""disableReferencedProjectLoad": true"#,
            &[],
        );
        let (mut session, _) = projecttestutil::setup(files.clone());
        let uri =
            lsproto::DocumentUri::from("file:///user/username/projects/myproject/src/main.ts");
        let content = files["/user/username/projects/myproject/src/main.ts"].clone();

        // Ensure no configured project is created due to disableReferencedProjectLoad
        session.did_open_file(
            core::Context::default(),
            uri.clone(),
            1,
            content,
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(
            snapshot
                .project_collection
                .configured_project(tspath::Path::from(
                    "/user/username/projects/myproject/tsconfig-src.json",
                ))
                .is_none()
        );

        // Should use inferred project instead
        let default_project = snapshot.get_default_project(uri.clone());
        assert!(default_project.is_some());
        assert_eq!(default_project.unwrap().kind, Kind::Inferred);

        // Searched configs should be present while file is open
        assert_config_present(
            snapshot,
            "/user/username/projects/myproject/tsconfig.json",
            "solution config should be present",
        );
        assert_config_absent_with_message(
            snapshot,
            "/user/username/projects/myproject/tsconfig-src.json",
            "direct reference should not be present",
        );

        // Close the file and open another one in the inferred project
        session.did_close_file(core::Context::default(), uri);
        let dummy_uri =
            lsproto::DocumentUri::from("file:///user/username/workspaces/dummy/dummy.ts");
        session.did_open_file(
            core::Context::default(),
            dummy_uri,
            1,
            "const x = 1;".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(snapshot.project_collection.inferred_project().is_some());

        // Config files should be released
        assert_config_absent(snapshot, "/user/username/projects/myproject/tsconfig.json");
        assert_config_absent(
            snapshot,
            "/user/username/projects/myproject/tsconfig-src.json",
        );
    }

    // when project found is solution referencing default project indirectly through disableReferencedProjectLoad
    {
        let mut files = files_for_solution_config_file(&["./tsconfig-indirect1.json"], "", &[]);
        apply_indirect_project_files(&mut files, 1, r#""disableReferencedProjectLoad": true"#);
        let (mut session, _) = projecttestutil::setup(files.clone());
        let uri =
            lsproto::DocumentUri::from("file:///user/username/projects/myproject/src/main.ts");
        let content = files["/user/username/projects/myproject/src/main.ts"].clone();

        // Ensure no configured project is created due to disableReferencedProjectLoad in indirect project
        session.did_open_file(
            core::Context::default(),
            uri.clone(),
            1,
            content,
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(
            snapshot
                .project_collection
                .configured_project(tspath::Path::from(
                    "/user/username/projects/myproject/tsconfig-src.json",
                ))
                .is_none()
        );

        // Should use inferred project instead
        let default_project = snapshot.get_default_project(uri.clone());
        assert!(default_project.is_some());
        assert_eq!(default_project.unwrap().kind, Kind::Inferred);

        // Searched configs should be present while file is open
        assert_config_present(
            snapshot,
            "/user/username/projects/myproject/tsconfig.json",
            "solution config should be present",
        );
        assert_config_present(
            snapshot,
            "/user/username/projects/myproject/tsconfig-indirect1.json",
            "solution direct reference should be present",
        );
        assert_config_absent_with_message(
            snapshot,
            "/user/username/projects/myproject/tsconfig-src.json",
            "indirect reference should not be present",
        );

        // Close the file and open another one in the inferred project
        session.did_close_file(core::Context::default(), uri);
        let dummy_uri =
            lsproto::DocumentUri::from("file:///user/username/workspaces/dummy/dummy.ts");
        session.did_open_file(
            core::Context::default(),
            dummy_uri,
            1,
            "const x = 1;".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(snapshot.project_collection.inferred_project().is_some());

        // Config files should be released
        assert_config_absent(snapshot, "/user/username/projects/myproject/tsconfig.json");
        assert_config_absent(
            snapshot,
            "/user/username/projects/myproject/tsconfig-src.json",
        );
        assert_config_absent(
            snapshot,
            "/user/username/projects/myproject/tsconfig-indirect1.json",
        );
    }

    // when project found is solution referencing default project indirectly through disableReferencedProjectLoad in one but without it in another
    {
        let mut files = files_for_solution_config_file(
            &["./tsconfig-indirect1.json", "./tsconfig-indirect2.json"],
            "",
            &[],
        );
        apply_indirect_project_files(&mut files, 1, r#""disableReferencedProjectLoad": true"#);
        apply_indirect_project_files(&mut files, 2, "");
        let (mut session, _) = projecttestutil::setup(files.clone());
        let uri =
            lsproto::DocumentUri::from("file:///user/username/projects/myproject/src/main.ts");
        let content = files["/user/username/projects/myproject/src/main.ts"].clone();

        // Ensure configured project is found through the indirect project without disableReferencedProjectLoad
        session.did_open_file(
            core::Context::default(),
            uri.clone(),
            1,
            content,
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        let src_project = snapshot
            .project_collection
            .configured_project(tspath::Path::from(
                "/user/username/projects/myproject/tsconfig-src.json",
            ))
            .expect("source project should exist");

        // Verify the default project is the source project (found through indirect2, not indirect1)
        let default_project = snapshot.get_default_project(uri.clone());
        assert!(std::ptr::eq(default_project.unwrap(), src_project));

        // Searched configs should be present while file is open
        assert_config_present(
            snapshot,
            "/user/username/projects/myproject/tsconfig.json",
            "solution config should be present",
        );
        assert_config_present(
            snapshot,
            "/user/username/projects/myproject/tsconfig-indirect1.json",
            "direct reference 1 should be present",
        );
        assert_config_present(
            snapshot,
            "/user/username/projects/myproject/tsconfig-indirect2.json",
            "direct reference 2 should be present",
        );
        assert_config_present(
            snapshot,
            "/user/username/projects/myproject/tsconfig-src.json",
            "indirect reference should be present",
        );

        // Close the file and open another one in the inferred project
        session.did_close_file(core::Context::default(), uri);
        let dummy_uri =
            lsproto::DocumentUri::from("file:///user/username/workspaces/dummy/dummy.ts");
        session.did_open_file(
            core::Context::default(),
            dummy_uri,
            1,
            "const x = 1;".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(snapshot.project_collection.inferred_project().is_some());

        // Config files should be released
        assert_config_absent(snapshot, "/user/username/projects/myproject/tsconfig.json");
        assert_config_absent(
            snapshot,
            "/user/username/projects/myproject/tsconfig-src.json",
        );
        assert_config_absent(
            snapshot,
            "/user/username/projects/myproject/tsconfig-indirect1.json",
        );
        assert_config_absent(
            snapshot,
            "/user/username/projects/myproject/tsconfig-indirect2.json",
        );
    }

    // when project found is project with own files referencing the file from referenced project
    {
        let mut files =
            files_for_solution_config_file(&["./tsconfig-src.json"], "", &[r#""./own/main.ts""#]);
        files.insert(
            "/user/username/projects/myproject/own/main.ts".to_string(),
            r#"
			import { foo } from '../src/main';
			foo;
			export function bar() {}
		"#
            .to_string(),
        );
        let (mut session, _) = projecttestutil::setup(files.clone());
        let uri =
            lsproto::DocumentUri::from("file:///user/username/projects/myproject/src/main.ts");
        let content = files["/user/username/projects/myproject/src/main.ts"].clone();

        // Ensure configured project is found for open file - should load both projects
        session.did_open_file(
            core::Context::default(),
            uri.clone(),
            1,
            content,
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 2);
        let src_project = snapshot
            .project_collection
            .configured_project(tspath::Path::from(
                "/user/username/projects/myproject/tsconfig-src.json",
            ))
            .expect("source project should exist");
        let ancestor_project = snapshot
            .project_collection
            .configured_project(tspath::Path::from(
                "/user/username/projects/myproject/tsconfig.json",
            ));
        assert!(ancestor_project.is_some());

        // Verify the default project is the source project
        let default_project = snapshot.get_default_project(uri.clone());
        assert!(std::ptr::eq(default_project.unwrap(), src_project));

        // Searched configs should be present while file is open
        assert_config_present(
            snapshot,
            "/user/username/projects/myproject/tsconfig.json",
            "solution config should be present",
        );
        assert_config_present(
            snapshot,
            "/user/username/projects/myproject/tsconfig-src.json",
            "direct reference should be present",
        );

        // Close the file and open another one in the inferred project
        session.did_close_file(core::Context::default(), uri);
        let dummy_uri =
            lsproto::DocumentUri::from("file:///user/username/workspaces/dummy/dummy.ts");
        session.did_open_file(
            core::Context::default(),
            dummy_uri,
            1,
            "const x = 1;".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(snapshot.project_collection.inferred_project().is_some());

        // Config files should be released
        assert_config_absent(snapshot, "/user/username/projects/myproject/tsconfig.json");
        assert_config_absent(
            snapshot,
            "/user/username/projects/myproject/tsconfig-src.json",
        );
    }

    // when file is not part of first config tree found, looks into ancestor folder and its references to find default project
    {
        let files = HashMap::from([
            (
                "/home/src/projects/project/app/Component-demos.ts".to_string(),
                r#"
                import * as helpers from 'demos/helpers';
                export const demo = () => {
                    helpers;
                }
            "#
                .to_string(),
            ),
            (
                "/home/src/projects/project/app/Component.ts".to_string(),
                r#"export const Component = () => {}"#.to_string(),
            ),
            (
                "/home/src/projects/project/app/tsconfig.json".to_string(),
                r#"{
				"compilerOptions": {
					"composite": true,
					"outDir": "../app-dist/",
				},
				"include": ["**/*"],
				"exclude": ["**/*-demos.*"],
			}"#
                .to_string(),
            ),
            (
                "/home/src/projects/project/demos/helpers.ts".to_string(),
                "export const foo = 1;".to_string(),
            ),
            (
                "/home/src/projects/project/demos/tsconfig.json".to_string(),
                r#"{
				"compilerOptions": {
					"composite": true,
					"rootDir": "../",
					"outDir": "../demos-dist/",
					"paths": {
						"demos/*": ["./*"],
					},
				},
				"include": [
					"**/*",
					"../app/**/*-demos.*",
				],
			}"#
                .to_string(),
            ),
            (
                "/home/src/projects/project/tsconfig.json".to_string(),
                r#"{
				"compilerOptions": {
					"outDir": "./dist/",
				},
				"references": [
					{ "path": "./demos/tsconfig.json" },
					{ "path": "./app/tsconfig.json" },
				],
				"files": []
			}"#
                .to_string(),
            ),
        ]);
        let (mut session, _) = projecttestutil::setup(files.clone());
        let uri =
            lsproto::DocumentUri::from("file:///home/src/projects/project/app/Component-demos.ts");
        let content = files["/home/src/projects/project/app/Component-demos.ts"].clone();

        // Ensure configured project is found for open file
        session.did_open_file(
            core::Context::default(),
            uri.clone(),
            1,
            content,
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 2);
        let demo_project = snapshot
            .project_collection
            .configured_project(tspath::Path::from(
                "/home/src/projects/project/demos/tsconfig.json",
            ))
            .expect("demos project should exist");
        let solution_project = snapshot
            .project_collection
            .configured_project(tspath::Path::from(
                "/home/src/projects/project/tsconfig.json",
            ));
        assert!(solution_project.is_some());

        // Verify the default project is the demos project (not the app project that excludes demos files)
        let default_project = snapshot.get_default_project(uri.clone());
        assert!(std::ptr::eq(default_project.unwrap(), demo_project));

        // Searched configs should be present while file is open
        assert_config_present(
            snapshot,
            "/home/src/projects/project/app/tsconfig.json",
            "app config should be present",
        );
        assert_config_present(
            snapshot,
            "/home/src/projects/project/demos/tsconfig.json",
            "demos config should be present",
        );
        assert_config_present(
            snapshot,
            "/home/src/projects/project/tsconfig.json",
            "solution config should be present",
        );

        // Close the file and open another one in the inferred project
        session.did_close_file(core::Context::default(), uri);
        let dummy_uri =
            lsproto::DocumentUri::from("file:///user/username/workspaces/dummy/dummy.ts");
        session.did_open_file(
            core::Context::default(),
            dummy_uri,
            1,
            "const x = 1;".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(snapshot.project_collection.inferred_project().is_some());

        // Config files should be released
        assert_config_absent(snapshot, "/home/src/projects/project/app/tsconfig.json");
        assert_config_absent(snapshot, "/home/src/projects/project/demos/tsconfig.json");
        assert_config_absent(snapshot, "/home/src/projects/project/tsconfig.json");
    }

    // when dts file is next to ts file and included as root in referenced project
    {
        let files = HashMap::from([
            (
                "/home/src/projects/project/src/index.d.ts".to_string(),
                r#"
                 declare global {
                    interface Window {
                        electron: ElectronAPI
                        api: unknown
                    }
                }
            "#
                .to_string(),
            ),
            (
                "/home/src/projects/project/src/index.ts".to_string(),
                "const api = {}".to_string(),
            ),
            (
                "/home/src/projects/project/tsconfig.json".to_string(),
                r#"{
				"include": [
					"src/*.d.ts",
				],
				"references": [{ "path": "./tsconfig.node.json" }],
			}"#
                .to_string(),
            ),
            (
                "/home/src/projects/project/tsconfig.node.json".to_string(),
                r#"{
				"include": ["src/**/*"],
                "compilerOptions": {
                    "composite": true,
                },
			}"#
                .to_string(),
            ),
        ]);
        let (mut session, _) = projecttestutil::setup(files.clone());
        let uri = lsproto::DocumentUri::from("file:///home/src/projects/project/src/index.d.ts");
        let content = files["/home/src/projects/project/src/index.d.ts"].clone();

        // Ensure configured projects are found for open file
        session.did_open_file(
            core::Context::default(),
            uri.clone(),
            1,
            content,
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 2);
        let root_project = snapshot
            .project_collection
            .configured_project(tspath::Path::from(
                "/home/src/projects/project/tsconfig.json",
            ));
        assert!(root_project.is_some());

        // Verify the default project is inferred
        let default_project = snapshot.get_default_project(uri.clone());
        assert!(default_project.is_some());
        assert_eq!(default_project.unwrap().kind, Kind::Inferred);

        // Searched configs should be present while file is open
        assert_config_present(
            snapshot,
            "/home/src/projects/project/tsconfig.json",
            "root config should be present",
        );
        assert_config_present(
            snapshot,
            "/home/src/projects/project/tsconfig.node.json",
            "node config should be present",
        );

        // Close the file and open another one in the inferred project
        session.did_close_file(core::Context::default(), uri);
        let dummy_uri =
            lsproto::DocumentUri::from("file:///user/username/workspaces/dummy/dummy.ts");
        session.did_open_file(
            core::Context::default(),
            dummy_uri,
            1,
            "const x = 1;".to_string(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(snapshot.project_collection.inferred_project().is_some());

        // Config files should be released
        assert_config_absent(snapshot, "/home/src/projects/project/tsconfig.json");
        assert_config_absent(snapshot, "/home/src/projects/project/tsconfig.node.json");
    }

    // #1630
    {
        let files = HashMap::from([
            (
                "/project/lib/tsconfig.json".to_string(),
                r#"{
				"files": ["a.ts"]
			}"#
                .to_string(),
            ),
            (
                "/project/lib/a.ts".to_string(),
                "export const a = 1;".to_string(),
            ),
            (
                "/project/lib/b.ts".to_string(),
                "export const b = 1;".to_string(),
            ),
            (
                "/project/tsconfig.json".to_string(),
                r#"{
				"files": [],
				"references": [{ "path": "./lib" }],
				"compilerOptions": {
					"disableReferencedProjectLoad": true
				}
			}"#
                .to_string(),
            ),
            ("/project/index.ts".to_string(), String::new()),
        ]);

        let (mut session, _) = projecttestutil::setup(files.clone());

        // opening b.ts puts /project/lib/tsconfig.json in the config file registry and creates the project,
        // but the project is ultimately not a match
        session.did_open_file(
            core::Context::default(),
            "file:///project/lib/b.ts".to_string(),
            1,
            files["/project/lib/b.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        // opening an unrelated file triggers cleanup of /project/lib/tsconfig.json since no open file is part of that project,
        // but will keep the config file in the registry since lib/b.ts is still open
        session.did_open_file(
            core::Context::default(),
            "untitled:Untitled-1".to_string(),
            1,
            String::new(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        // Opening index.ts searches /project/tsconfig.json and then checks /project/lib/tsconfig.json without opening it.
        // No early return on config file existence means we try to find an already open project, which returns nil,
        // triggering a crash.
        session.did_open_file(
            core::Context::default(),
            "file:///project/index.ts".to_string(),
            1,
            files["/project/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
    }

    // inferred project root files are in stable order
    {
        let files = HashMap::from([
            (
                "/project/a.ts".to_string(),
                "export const a = 1;".to_string(),
            ),
            (
                "/project/b.ts".to_string(),
                "export const b = 1;".to_string(),
            ),
            (
                "/project/c.ts".to_string(),
                "export const c = 1;".to_string(),
            ),
        ]);

        let (mut session, _) = projecttestutil::setup(files.clone());

        // b, c, a
        session.did_open_file(
            core::Context::default(),
            "file:///project/b.ts".to_string(),
            1,
            files["/project/b.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.did_open_file(
            core::Context::default(),
            "file:///project/c.ts".to_string(),
            1,
            files["/project/c.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session.did_open_file(
            core::Context::default(),
            "file:///project/a.ts".to_string(),
            1,
            files["/project/a.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );

        let snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        let inferred_project = snapshot
            .project_collection
            .inferred_project()
            .expect("inferred project should exist");
        // It's more bookkeeping to maintain order of opening, since any file can move into or out of
        // the inferred project due to changes in other projects. Order shouldn't matter for correctness,
        // we just want it to be consistent, in case there are observable type ordering issues.
        assert_eq!(
            inferred_project
                .get_program()
                .unwrap()
                .command_line()
                .file_names,
            vec![
                "/project/a.ts".to_string(),
                "/project/b.ts".to_string(),
                "/project/c.ts".to_string(),
            ]
        );
    }

    // project lookup terminates
    {
        let files = HashMap::from([
            (
                "/tsconfig.json".to_string(),
                r#"{
				"files": [],
				"references": [
					{
						"path": "./packages/pkg1"
					},
					{
						"path": "./packages/pkg2"
					},
				]
			}"#
                .to_string(),
            ),
            (
                "/packages/pkg1/tsconfig.json".to_string(),
                r#"{
				"include": ["src/**/*.ts"],
				"compilerOptions": {
					"composite": true,
				},
				"references": [
					{
						"path": "../pkg2"
					},
				]
			}"#
                .to_string(),
            ),
            (
                "/packages/pkg2/tsconfig.json".to_string(),
                r#"{
				"include": ["src/**/*.ts"],
				"compilerOptions": {
					"composite": true,
				},
				"references": [
					{
						"path": "../pkg1"
					},
				]
			}"#
                .to_string(),
            ),
            ("/script.ts".to_string(), "export const a = 1;".to_string()),
        ]);
        let (mut session, _) = projecttestutil::setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///script.ts".to_string(),
            1,
            files["/script.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        // Test should terminate
    }

    // file moves to inferred project after import is deleted
    {
        // This test verifies that when a node_modules dependency file is open and its import
        // is deleted from the project root, requesting language service for the dependency
        // correctly moves it to an inferred project.
        let files = HashMap::from([
            (
                "/project/tsconfig.json".to_string(),
                r#"{"compilerOptions": {"strict": true}}"#.to_string(),
            ),
            (
                "/project/index.ts".to_string(),
                r#"import { helper } from "./node_modules/dep/index";"#.to_string(),
            ),
            (
                "/project/node_modules/dep/index.d.ts".to_string(),
                "export declare function helper(): void;".to_string(),
            ),
        ]);
        let (mut session, _) = projecttestutil::setup(files.clone());

        // Step 1: Open the project root file
        let root_uri = lsproto::DocumentUri::from("file:///project/index.ts");
        session.did_open_file(
            core::Context::default(),
            root_uri.clone(),
            1,
            files["/project/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session
            .get_language_service(core::Context::default(), root_uri.clone())
            .expect("GetLanguageService should succeed");

        // Step 2: Open the node_modules dependency file - should be in the configured project
        let dep_uri = lsproto::DocumentUri::from("file:///project/node_modules/dep/index.d.ts");
        session.did_open_file(
            core::Context::default(),
            dep_uri.clone(),
            1,
            files["/project/node_modules/dep/index.d.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );

        let snapshot = session.snapshot();
        let configured_project = snapshot
            .project_collection
            .configured_project(tspath::Path::from("/project/tsconfig.json"))
            .expect("configured project should exist");
        let default_project = snapshot.get_default_project(dep_uri.clone());
        assert!(
            std::ptr::eq(default_project.unwrap(), configured_project),
            "dependency should be in the configured project initially"
        );

        // Step 3: Delete the import from the root file
        session.did_change_file(
            core::Context::default(),
            root_uri,
            2,
            vec![crate::TextDocumentContentChangePartialOrWholeDocument {
                partial: None,
                whole_document: Some(lsproto::TextDocumentContentChangeWholeDocument {
                    text: "// import removed".to_string(),
                }),
            }],
        );

        // Step 4: Request language service for the dependency - it should now be in an inferred project
        let ls = session.get_language_service(core::Context::default(), dep_uri.clone());
        assert!(
            ls.is_ok(),
            "language service should be available for dependency"
        );

        let snapshot = session.snapshot();
        let default_project = snapshot.get_default_project(dep_uri);
        assert!(
            default_project.is_some(),
            "dependency should have a default project"
        );
        assert_eq!(
            default_project.unwrap().kind,
            Kind::Inferred,
            "dependency should be in an inferred project after import is deleted"
        );
    }

    // should update project on package.json change
    {
        // Set up a project with package.json "imports" that affect module resolution.
        // The package.json is not a program file, but it IS an affecting location.
        // When it changes, the project should be marked dirty and the program should be rebuilt.
        let package_json_files = HashMap::from([
            (
                "/home/projects/myproject/tsconfig.json".to_string(),
                r##"{
				"compilerOptions": {
					"module": "nodenext",
					"moduleResolution": "nodenext",
					"noLib": true,
					"noEmit": true
				}
			}"##
                .to_string(),
            ),
            (
                "/home/projects/myproject/package.json".to_string(),
                r##"{
				"name": "myproject",
				"type": "module",
				"imports": {
					"#utils": "./src/utils.ts"
				}
			}"##
                .to_string(),
            ),
            (
                "/home/projects/myproject/src/index.ts".to_string(),
                r##"import { add } from "#utils";"##.to_string(),
            ),
            (
                "/home/projects/myproject/src/utils.ts".to_string(),
                "export function add(a: number, b: number) { return a + b; }".to_string(),
            ),
        ]);

        let (mut session, utils) = projecttestutil::setup(package_json_files.clone());
        let index_uri = lsproto::DocumentUri::from("file:///home/projects/myproject/src/index.ts");
        session.did_open_file(
            core::Context::default(),
            index_uri.clone(),
            1,
            package_json_files["/home/projects/myproject/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );

        // Verify initial state: #utils resolves to utils.ts, so utils.ts is in the program
        let ls = session
            .get_language_service(core::Context::default(), index_uri.clone())
            .expect("GetLanguageService should succeed");
        let program = ls.get_program();
        assert_eq!(
            program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    program
                        .get_source_file("/home/projects/myproject/src/index.ts")
                        .as_ref(),
                )
                .len(),
            0,
            "should have no diagnostics with correct package.json"
        );

        // Now change the package.json to point #utils at a non-existent file
        utils
            .fs()
            .write_file(
                "/home/projects/myproject/package.json",
                r##"{
			"name": "myproject",
			"type": "module",
			"imports": {
				"#utils": "./src/nonexistent.ts"
			}
		}"##,
            )
            .expect("WriteFile should succeed");
        session.did_change_watched_files(
            core::Context::default(),
            vec![file_event(
                "file:///home/projects/myproject/package.json",
                lsproto::FileChangeType::CHANGED,
            )],
        );

        let ls = session
            .get_language_service(core::Context::default(), index_uri)
            .expect("GetLanguageService should succeed");
        let updated_program = ls.get_program();
        assert_eq!(
            updated_program
                .get_semantic_diagnostics(
                    projecttestutil::with_request_id(core::Context::default()),
                    updated_program
                        .get_source_file("/home/projects/myproject/src/index.ts")
                        .as_ref(),
                )
                .len(),
            1,
            "should have diagnostics after package.json change"
        );
    }
}

fn assert_config_present(snapshot: &project::Snapshot, path: &str, message: &str) {
    assert!(
        snapshot
            .config_file_registry
            .as_ref()
            .unwrap()
            .get_config(tspath::Path::from(path))
            .is_some(),
        "{}",
        message
    );
}

fn assert_config_absent(snapshot: &project::Snapshot, path: &str) {
    assert!(
        snapshot
            .config_file_registry
            .as_ref()
            .unwrap()
            .get_config(tspath::Path::from(path))
            .is_none()
    );
}

fn assert_config_absent_with_message(snapshot: &project::Snapshot, path: &str, message: &str) {
    assert!(
        snapshot
            .config_file_registry
            .as_ref()
            .unwrap()
            .get_config(tspath::Path::from(path))
            .is_none(),
        "{}",
        message
    );
}

fn files_for_solution_config_file(
    solution_refs: &[&str],
    compiler_options: &str,
    own_files: &[&str],
) -> HashMap<String, String> {
    let compiler_options_str = if !compiler_options.is_empty() {
        format!(
            r#""compilerOptions": {{
			{}
		}},"#,
            compiler_options
        )
    } else {
        String::new()
    };
    let own_files_str = if !own_files.is_empty() {
        own_files.join(",")
    } else {
        String::new()
    };
    HashMap::from([
        (
            "/user/username/projects/myproject/tsconfig.json".to_string(),
            format!(
                r#"{{
			{}
			"files": [{}],
			"references": [
				{}
			]
		}}"#,
                compiler_options_str,
                own_files_str,
                solution_refs
                    .iter()
                    .map(|reference| format!(r#"{{ "path": "{}" }}"#, reference))
                    .collect::<Vec<_>>()
                    .join(","),
            ),
        ),
        (
            "/user/username/projects/myproject/tsconfig-src.json".to_string(),
            r#"{
			"compilerOptions": {
				"composite": true,
				"outDir": "./target",
			},
			"include": ["./src/**/*"]
		}"#
            .to_string(),
        ),
        (
            "/user/username/projects/myproject/src/main.ts".to_string(),
            r#"
			import { foo } from './helpers/functions';
			export { foo };"#
                .to_string(),
        ),
        (
            "/user/username/projects/myproject/src/helpers/functions.ts".to_string(),
            "export const foo = 1;".to_string(),
        ),
    ])
}

fn apply_indirect_project_files(
    files: &mut HashMap<String, String>,
    project_index: i32,
    compiler_options: &str,
) {
    files.extend(files_for_indirect_project(project_index, compiler_options));
}

fn files_for_indirect_project(
    project_index: i32,
    compiler_options: &str,
) -> HashMap<String, String> {
    HashMap::from([
        (
            format!(
                "/user/username/projects/myproject/tsconfig-indirect{}.json",
                project_index
            ),
            format!(
                r#"{{
			"compilerOptions": {{
				"composite": true,
				"outDir": "./target/",
				{}
			}},
			"files": [
				"./indirect{}/main.ts"
			],
			"references": [
				{{
				"path": "./tsconfig-src.json"
				}}
			]
		}}"#,
                compiler_options, project_index
            ),
        ),
        (
            format!(
                "/user/username/projects/myproject/indirect{}/main.ts",
                project_index
            ),
            "export const indirect = 1;".to_string(),
        ),
    ])
}
