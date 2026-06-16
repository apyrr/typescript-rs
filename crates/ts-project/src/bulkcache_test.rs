use std::collections::HashMap;

use crate::projecttestutil;
use ts_core as core;
use ts_lsproto as lsproto;
use ts_vfs::Fs;

use crate as project;

const LANGUAGE_KIND_TYPESCRIPT: &str = "typescript";

#[test]
fn test_bulk_cache_invalidation() {
    if !ts_bundled::EMBEDDED {
        return;
    }

    // Base file structure for testing
    let base_files = HashMap::from([
        (
            "/project/tsconfig.json".to_string(),
            r#"{
			"compilerOptions": {
				"strict": true,
				"target": "es2015",
				"types": ["node"]
			},
			"include": ["src/**/*"]
		}"#
            .to_string(),
        ),
        (
            "/project/src/index.ts".to_string(),
            r#"import { helper } from "./helper"; console.log(helper);"#.to_string(),
        ),
        (
            "/project/src/helper.ts".to_string(),
            r#"export const helper = "test";"#.to_string(),
        ),
        (
            "/project/src/utils/lib.ts".to_string(),
            r#"export function util() { return "util"; }"#.to_string(),
        ),
        (
            "/project/node_modules/@types/node/index.d.ts".to_string(),
            r#"import "./fs"; import "./console";"#.to_string(),
        ),
        (
            "/project/node_modules/@types/node/fs.d.ts".to_string(),
            "".to_string(),
        ),
        (
            "/project/node_modules/@types/node/console.d.ts".to_string(),
            "".to_string(),
        ),
    ]);

    // large number of node_modules changes invalidates only node_modules cache
    {
        fn test(
            base_files: &HashMap<String, String>,
            file_events: Vec<lsproto::FileEvent>,
            expect_node_modules_invalidation: bool,
        ) {
            let (mut session, utils) = projecttestutil::setup(base_files.clone());

            // Open a file to create the project
            session.did_open_file(
                core::Context::default(),
                "file:///project/src/index.ts".to_string(),
                1,
                base_files["/project/src/index.ts"].clone(),
                LANGUAGE_KIND_TYPESCRIPT.to_string(),
            );

            // Get initial snapshot and verify config
            let ls = session
                .get_language_service(
                    core::Context::default(),
                    "file:///project/src/index.ts".to_string(),
                )
                .expect("GetLanguageService should succeed");
            assert_eq!(
                ls.get_program().compiler_options().target,
                core::ScriptTarget::ES2015
            );

            let snapshot_before = session.snapshot();
            let config_before = snapshot_before
                .config_file_registry
                .as_ref()
                .map(|registry| registry as *const _);

            // Update tsconfig.json on disk to test that configs don't get reloaded
            utils
                .fs()
                .write_file(
                    "/project/tsconfig.json",
                    r#"{
			"compilerOptions": {
				"strict": true,
				"target": "esnext",
				"types": ["node"]
			},
			"include": ["src/**/*"]
		}"#,
                )
                .expect("WriteFile should succeed");
            // Update fs.d.ts in node_modules
            utils
                .fs()
                .write_file("/project/node_modules/@types/node/fs.d.ts", "new text")
                .expect("WriteFile should succeed");

            // Process the excessive node_modules changes
            session.did_change_watched_files(core::Context::default(), file_events);

            // Get language service again to trigger snapshot update
            let ls = session
                .get_language_service(
                    core::Context::default(),
                    "file:///project/src/index.ts".to_string(),
                )
                .expect("GetLanguageService should succeed");

            let snapshot_after = session.snapshot();
            let config_after = snapshot_after
                .config_file_registry
                .as_ref()
                .map(|registry| registry as *const _);

            // Config should NOT have been reloaded (target should remain ES2015, not esnext)
            assert_eq!(
                ls.get_program().compiler_options().target,
                core::ScriptTarget::ES2015,
                "Config should not have been reloaded for node_modules-only changes"
            );

            // Config registry should be the same instance (no configs reloaded)
            assert_eq!(
                config_before, config_after,
                "Config registry should not have changed for node_modules-only changes"
            );

            let fs_dts_text = snapshot_after
                .get_file("/project/node_modules/@types/node/fs.d.ts")
                .expect("fs.d.ts should exist")
                .content();
            if expect_node_modules_invalidation {
                assert_eq!(fs_dts_text, "new text");
            } else {
                assert_eq!(fs_dts_text, "");
            }
        }

        // with file existing in cache
        {
            let mut file_events = generate_file_events(
                1001,
                "file:///project/node_modules/generated/file%d.js",
                lsproto::FileChangeType::Created,
            );
            // Include two files in the program to trigger a full program creation.
            // Exclude fs.d.ts to show that its content still gets invalidated.
            file_events.push(lsproto::FileEvent {
                uri: "file:///project/node_modules/@types/node/index.d.ts".to_string(),
                typ: lsproto::FileChangeType::Changed,
            });
            file_events.push(lsproto::FileEvent {
                uri: "file:///project/node_modules/@types/node/console.d.ts".to_string(),
                typ: lsproto::FileChangeType::Changed,
            });

            test(&base_files, file_events, true);
        }

        // without file existing in cache
        {
            let file_events = generate_file_events(
                1001,
                "file:///project/node_modules/generated/file%d.js",
                lsproto::FileChangeType::Created,
            );
            test(&base_files, file_events, false);
        }
    }

    // large number of changes outside node_modules
    {
        fn test(
            base_files: &HashMap<String, String>,
            file_events: Vec<lsproto::FileEvent>,
            expect_config_reload: bool,
        ) {
            let (mut session, utils) = projecttestutil::setup(base_files.clone());

            // Open a file to create the project
            session.did_open_file(
                core::Context::default(),
                "file:///project/src/index.ts".to_string(),
                1,
                base_files["/project/src/index.ts"].clone(),
                LANGUAGE_KIND_TYPESCRIPT.to_string(),
            );

            // Get initial state
            let ls = session
                .get_language_service(
                    core::Context::default(),
                    "file:///project/src/index.ts".to_string(),
                )
                .expect("GetLanguageService should succeed");
            assert_eq!(
                ls.get_program().compiler_options().target,
                core::ScriptTarget::ES2015
            );

            // Update tsconfig.json on disk
            utils
                .fs()
                .write_file(
                    "/project/tsconfig.json",
                    r#"{
			"compilerOptions": {
				"strict": true,
				"target": "esnext",
				"types": ["node"]
			},
			"include": ["src/**/*"]
		}"#,
                )
                .expect("WriteFile should succeed");
            // Add root file
            utils
                .fs()
                .write_file("/project/src/rootFile.ts", r#"console.log("root file")"#)
                .expect("WriteFile should succeed");

            session.did_change_watched_files(core::Context::default(), file_events);
            let ls = session
                .get_language_service(
                    core::Context::default(),
                    "file:///project/src/index.ts".to_string(),
                )
                .expect("GetLanguageService should succeed");

            if expect_config_reload {
                assert_eq!(
                    ls.get_program().compiler_options().target,
                    core::ScriptTarget::ESNext,
                    "Config should have been reloaded for changes outside node_modules"
                );
                assert!(
                    ls.get_program()
                        .get_source_file("/project/src/rootFile.ts")
                        .is_some(),
                    "New root file should be present"
                );
            } else {
                assert_eq!(
                    ls.get_program().compiler_options().target,
                    core::ScriptTarget::ES2015,
                    "Config should not have been reloaded for changes outside node_modules"
                );
                assert!(
                    ls.get_program()
                        .get_source_file("/project/src/rootFile.ts")
                        .is_none(),
                    "New root file should not be present"
                );
            }
        }

        // with event matching include glob
        {
            let mut file_events = generate_file_events(
                1001,
                "file:///project/generated/file%d.ts",
                lsproto::FileChangeType::Created,
            );
            file_events.push(lsproto::FileEvent {
                uri: "file:///project/src/rootFile.ts".to_string(),
                typ: lsproto::FileChangeType::Created,
            });
            test(&base_files, file_events, true);
        }

        // without event matching include glob
        {
            let file_events = generate_file_events(
                1001,
                "file:///project/generated/file%d.ts",
                lsproto::FileChangeType::Created,
            );
            test(&base_files, file_events, false);
        }
    }

    // large number of changes outside node_modules causes project reevaluation
    {
        let (mut session, utils) = projecttestutil::setup(base_files.clone());

        // Open a file that will initially use the root tsconfig
        session.did_open_file(
            core::Context::default(),
            "file:///project/src/utils/lib.ts".to_string(),
            1,
            base_files["/project/src/utils/lib.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );

        // Initially, the file should use the root project (strict mode)
        let snapshot = session.snapshot();
        let initial_project = snapshot
            .get_default_project("file:///project/src/utils/lib.ts".to_string())
            .expect("default project should exist");
        assert_eq!(
            initial_project.name(),
            "/project/tsconfig.json",
            "Should initially use root tsconfig"
        );

        // Get language service to verify initial strict mode
        let ls = session
            .get_language_service(
                core::Context::default(),
                "file:///project/src/utils/lib.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");
        assert_eq!(
            ls.get_program().compiler_options().strict,
            core::TSTrue,
            "Should initially use strict mode from root config"
        );

        // Now create the nested tsconfig (this would normally be detected, but we'll simulate a missed event)
        utils
            .fs()
            .write_file(
                "/project/src/utils/tsconfig.json",
                r#"{
			"compilerOptions": {
				"strict": false,
				"target": "esnext"
			}
		}"#,
            )
            .expect("WriteFile should succeed");

        // Create excessive changes to trigger bulk invalidation
        let file_events = generate_file_events(
            1001,
            "file:///project/src/generated/file%d.ts",
            lsproto::FileChangeType::Created,
        );

        // Process the excessive changes - this should trigger project reevaluation
        session.did_change_watched_files(core::Context::default(), file_events);

        // Get language service - this should now find the nested config and switch projects
        let ls = session
            .get_language_service(
                core::Context::default(),
                "file:///project/src/utils/lib.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");

        let snapshot = session.snapshot();
        let new_project = snapshot
            .get_default_project("file:///project/src/utils/lib.ts".to_string())
            .expect("default project should exist");

        // The file should now use the nested tsconfig
        assert_eq!(
            new_project.name(),
            "/project/src/utils/tsconfig.json",
            "Should now use nested tsconfig after bulk invalidation"
        );
        assert_eq!(
            ls.get_program().compiler_options().strict,
            core::TSFalse,
            "Should now use non-strict mode from nested config"
        );
        assert_eq!(
            ls.get_program().compiler_options().target,
            core::ScriptTarget::ESNext,
            "Should use esnext target from nested config"
        );
    }

    // config file names cache
    {
        fn test(
            file_events: Vec<lsproto::FileEvent>,
            expect_config_discovery: bool,
            _test_name: &str,
        ) {
            let files = HashMap::from([(
                "/project/src/index.ts".to_string(),
                r#"console.log("test");"#.to_string(), // No tsconfig initially
            )]);
            let (mut session, utils) = projecttestutil::setup(files.clone());

            // Open file without tsconfig - should create inferred project
            session.did_open_file(
                core::Context::default(),
                "file:///project/src/index.ts".to_string(),
                1,
                files["/project/src/index.ts"].clone(),
                LANGUAGE_KIND_TYPESCRIPT.to_string(),
            );

            let snapshot = session.snapshot();
            assert!(
                snapshot.project_collection.inferred_project().is_some(),
                "Should have inferred project"
            );
            assert_eq!(
                snapshot
                    .get_default_project("file:///project/src/index.ts".to_string())
                    .expect("default project should exist")
                    .kind,
                project::Kind::Inferred
            );

            // Create a tsconfig that would affect this file (simulating a missed creation event)
            utils
                .fs()
                .write_file(
                    "/project/tsconfig.json",
                    r#"{
		"compilerOptions": {
			"strict": true
		},
		"include": ["src/**/*"]
	}"#,
                )
                .expect("WriteFile should succeed");

            // Process the changes
            session.did_change_watched_files(core::Context::default(), file_events);

            // Get language service to trigger config discovery
            session
                .get_language_service(
                    core::Context::default(),
                    "file:///project/src/index.ts".to_string(),
                )
                .expect("GetLanguageService should succeed");

            let snapshot = session.snapshot();
            let new_project = snapshot
                .get_default_project("file:///project/src/index.ts".to_string())
                .expect("default project should exist");

            // Check expected behavior
            if expect_config_discovery {
                // Should now use configured project instead of inferred
                assert_eq!(
                    new_project.kind,
                    project::Kind::Configured,
                    "Should now use configured project after cache invalidation"
                );
                assert_eq!(
                    new_project.name(),
                    "/project/tsconfig.json",
                    "Should use the newly discovered tsconfig"
                );
            } else {
                // Should still use inferred project (config file names cache not cleared)
                assert!(
                    std::ptr::eq(
                        new_project,
                        snapshot
                            .project_collection
                            .inferred_project()
                            .expect("inferred project should exist")
                    ),
                    "Should still use inferred project after node_modules-only changes"
                );
            }
        }

        // excessive changes only in node_modules does not affect config file names cache
        {
            let file_events = generate_file_events(
                1001,
                "file:///project/node_modules/generated/file%d.js",
                lsproto::FileChangeType::Created,
            );
            test(
                file_events,
                false,
                "node_modules changes should not clear config cache",
            );
        }

        // excessive changes outside node_modules clears config file names cache
        {
            let mut file_events = generate_file_events(
                1001,
                "file:///project/src/generated/file%d.ts",
                lsproto::FileChangeType::Created,
            );
            // Presence of any tsconfig.json file event triggers rediscovery for config for all open files
            file_events.push(lsproto::FileEvent {
                uri: "file:///project/src/generated/tsconfig.json".to_string(),
                typ: lsproto::FileChangeType::Created,
            });
            test(
                file_events,
                true,
                "non-node_modules changes should clear config cache",
            );
        }
    }

    // Simulate external build tool changing files in dist/ (not included by any project)
    // excessive changes in dist folder do not invalidate
    {
        let files = HashMap::from([(
            "/project/src/index.ts".to_string(),
            r#"console.log("test");"#.to_string(), // No tsconfig initially
        )]);
        let (mut session, utils) = projecttestutil::setup(files.clone());

        // Open file without tsconfig - should create inferred project
        session.did_open_file(
            core::Context::default(),
            "file:///project/src/index.ts".to_string(),
            1,
            files["/project/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );

        let snapshot = session.snapshot();
        assert_eq!(
            snapshot
                .get_default_project("file:///project/src/index.ts".to_string())
                .expect("default project should exist")
                .kind,
            project::Kind::Inferred
        );

        // Create a tsconfig that would affect this file (simulating a missed creation event)
        // This should NOT be discovered after dist-folder changes
        utils
            .fs()
            .write_file(
                "/project/tsconfig.json",
                r#"{
			"compilerOptions": {
				"strict": true
			},
			"include": ["src/**/*"]
		}"#,
            )
            .expect("WriteFile should succeed");

        // Create excessive changes in dist folder only
        let file_events = generate_file_events(
            1001,
            "file:///project/dist/generated/file%d.js",
            lsproto::FileChangeType::Created,
        );
        session.did_change_watched_files(core::Context::default(), file_events);

        // File should still use inferred project (config file names cache NOT cleared for dist changes)
        session
            .get_language_service(
                core::Context::default(),
                "file:///project/src/index.ts".to_string(),
            )
            .expect("GetLanguageService should succeed");

        let snapshot = session.snapshot();
        let new_project = snapshot
            .get_default_project("file:///project/src/index.ts".to_string())
            .expect("default project should exist");
        assert_eq!(
            new_project.kind,
            project::Kind::Inferred,
            "dist-folder changes should not cause config discovery"
        );
        // This assertion mirrors the Go source's dist-folder behavior check.
    }
}

// Helper function to generate excessive file change events
fn generate_file_events(
    count: usize,
    path_template: &str,
    change_type: lsproto::FileChangeType,
) -> Vec<lsproto::FileEvent> {
    let mut events = Vec::new();
    for i in 0..count {
        events.push(lsproto::FileEvent {
            uri: path_template.replace("%d", &i.to_string()),
            typ: change_type,
        });
    }
    events
}
