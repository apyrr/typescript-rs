use std::collections::HashMap;

use crate::projecttestutil;
use serde_json::json;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;

const LANGUAGE_KIND_TYPESCRIPT: &str = "typescript";

fn custom_config_preferences(custom_config_file_name: &str) -> lsutil::UserPreferences {
    lsutil::parse_user_preferences(HashMap::from([(
        "js/ts".to_string(),
        json!({
            "native-preview": {
                "customConfigFileName": custom_config_file_name,
            },
        }),
    )]))
}

#[test]
fn test_custom_config_file_name() {
    if !ts_bundled::EMBEDDED {
        return;
    }

    let files = HashMap::from([
        (
            "/src/tsconfig.json".to_string(),
            r#"{"compilerOptions": {"strict": false}}"#.to_string(),
        ),
        (
            "/src/tsconfig.all.json".to_string(),
            r#"{"compilerOptions": {"strict": true}}"#.to_string(),
        ),
        (
            "/src/index.ts".to_string(),
            "export const x = 1;".to_string(),
        ),
    ]);
    let uri = lsproto::DocumentUri::from("file:///src/index.ts");

    // picks up custom config and switches on preference change
    {
        let (mut session, _) = projecttestutil::setup(files.clone());

        session.did_open_file(
            core::Context::default(),
            uri.clone(),
            1,
            files["/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let ls = session
            .get_language_service(core::Context::default(), uri.clone())
            .expect("GetLanguageService should succeed");
        assert_eq!(ls.get_program().compiler_options().strict, core::TSFalse);

        let snapshot = session.snapshot();
        assert_eq!(
            snapshot.get_default_project(uri.clone()).unwrap().name(),
            "/src/tsconfig.json"
        );

        let mut prefs = lsutil::new_default_user_preferences();
        prefs.custom_config_file_name = "tsconfig.all.json".to_string();
        session.configure(prefs);

        let ls = session
            .get_language_service(core::Context::default(), uri.clone())
            .expect("GetLanguageService should succeed");
        assert_eq!(ls.get_program().compiler_options().strict, core::TSTrue);

        let snapshot = session.snapshot();
        assert_eq!(
            snapshot.get_default_project(uri.clone()).unwrap().name(),
            "/src/tsconfig.all.json"
        );
    }

    // uses tsconfig.json when customConfigFileName is empty
    {
        let (mut session, _) = projecttestutil::setup(files.clone());

        let prefs = lsutil::new_default_user_preferences();
        // default for CustomConfigFileName is "".
        assert_eq!(prefs.custom_config_file_name, "");
        session.configure(prefs);

        session.did_open_file(
            core::Context::default(),
            uri.clone(),
            1,
            files["/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session
            .get_language_service(core::Context::default(), uri.clone())
            .expect("GetLanguageService should succeed");

        let snapshot = session.snapshot();
        assert_eq!(
            snapshot.get_default_project(uri.clone()).unwrap().name(),
            "/src/tsconfig.json"
        );
    }

    // falls back to tsconfig.json when custom config missing
    {
        let (mut session, _) = projecttestutil::setup(files.clone());

        let mut prefs = lsutil::new_default_user_preferences();
        prefs.custom_config_file_name = "tsconfig.nonexistent.json".to_string();
        session.configure(prefs);

        session.did_open_file(
            core::Context::default(),
            uri.clone(),
            1,
            files["/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session
            .get_language_service(core::Context::default(), uri.clone())
            .expect("GetLanguageService should succeed");

        let snapshot = session.snapshot();
        assert_eq!(
            snapshot.get_default_project(uri.clone()).unwrap().name(),
            "/src/tsconfig.json"
        );
    }

    // reverts to tsconfig.json when custom config preference is cleared
    {
        let (mut session, _) = projecttestutil::setup(files.clone());

        // Step 1: Open file, verify it uses tsconfig.json (strict: false)
        session.did_open_file(
            core::Context::default(),
            uri.clone(),
            1,
            files["/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let ls = session
            .get_language_service(core::Context::default(), uri.clone())
            .expect("GetLanguageService should succeed");
        assert_eq!(ls.get_program().compiler_options().strict, core::TSFalse);

        let snapshot = session.snapshot();
        assert_eq!(
            snapshot.get_default_project(uri.clone()).unwrap().name(),
            "/src/tsconfig.json"
        );

        // Step 2: Switch to custom config (strict: true)
        let mut prefs = lsutil::new_default_user_preferences();
        prefs.custom_config_file_name = "tsconfig.all.json".to_string();
        session.configure(prefs);

        let ls = session
            .get_language_service(core::Context::default(), uri.clone())
            .expect("GetLanguageService should succeed");
        assert_eq!(ls.get_program().compiler_options().strict, core::TSTrue);

        let snapshot = session.snapshot();
        assert_eq!(
            snapshot.get_default_project(uri.clone()).unwrap().name(),
            "/src/tsconfig.all.json"
        );

        // Step 3: Clear custom config preference, should revert to tsconfig.json (strict: false)
        let mut prefs = lsutil::new_default_user_preferences();
        prefs.custom_config_file_name = "".to_string();
        session.configure(prefs);

        let ls = session
            .get_language_service(core::Context::default(), uri.clone())
            .expect("GetLanguageService should succeed");
        assert_eq!(ls.get_program().compiler_options().strict, core::TSFalse);

        let snapshot = session.snapshot();
        assert_eq!(
            snapshot.get_default_project(uri.clone()).unwrap().name(),
            "/src/tsconfig.json"
        );
    }

    // This test demonstrates the bug reported in #2020: after changing
    // customConfigFileName, the server does not schedule a diagnostics refresh,
    // so the VS Code client never knows to re-pull diagnostics and shows stale results.
    // schedules diagnostics refresh when custom config preference changes
    {
        let (mut session, utils) = projecttestutil::setup(files.clone());

        session.did_open_file(
            core::Context::default(),
            uri.clone(),
            1,
            files["/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session
            .get_language_service(core::Context::default(), uri.clone())
            .expect("GetLanguageService should succeed");
        session.wait_for_background_tasks();

        // Record baseline refresh call count
        let baseline_refresh_count = utils.client().refresh_diagnostics_calls().len();

        // Change the custom config preference
        let mut prefs = lsutil::new_default_user_preferences();
        prefs.custom_config_file_name = "tsconfig.all.json".to_string();
        session.configure(prefs);

        // GetLanguageService triggers the snapshot update with the new config
        session
            .get_language_service(core::Context::default(), uri.clone())
            .expect("GetLanguageService should succeed");
        session.wait_for_background_tasks();

        // The server should have scheduled a diagnostics refresh to tell the client
        // to re-pull diagnostics with the new project configuration.
        let refresh_count = utils.client().refresh_diagnostics_calls().len();
        assert!(
            refresh_count > baseline_refresh_count,
            "expected RefreshDiagnostics to be called after customConfigFileName change, got {refresh_count} calls (baseline {baseline_refresh_count})"
        );
    }

    // rejects path traversal in customConfigFileName
    {
        for invalid_name in [
            "/etc/passwd",
            "../tsconfig.json",
            "configs/tsconfig.all.json",
            "..\\tsconfig.json",
            "sub\\dir\\tsconfig.json",
            "..",
            ".",
        ] {
            let prefs = custom_config_preferences(invalid_name);
            assert_eq!(
                prefs.custom_config_file_name, "",
                "expected customConfigFileName to be cleared for invalid value {invalid_name:?}"
            );
        }
    }

    // accepts plain base file names in customConfigFileName
    {
        for valid_name in [
            "tsconfig.all.json",
            "tsconfig.editor.json",
            "jsconfig.custom.json",
        ] {
            let prefs = custom_config_preferences(valid_name);
            assert_eq!(
                prefs.custom_config_file_name, valid_name,
                "expected customConfigFileName to be {valid_name:?}"
            );
        }
    }

    // cleans up inferred project when custom config covers file
    {
        // Start without any tsconfig.json so file goes into inferred project, then
        // add a custom config that covers the file and verify it moves out of the
        // inferred project (not just getting a new default, but actually cleaned up).
        let files_no_config = HashMap::from([
            (
                "/src/tsconfig.all.json".to_string(),
                r#"{"compilerOptions": {"strict": true}, "include": ["./**/*"]}"#.to_string(),
            ),
            (
                "/src/index.ts".to_string(),
                "export const x = 1;".to_string(),
            ),
        ]);
        let uri_local = lsproto::DocumentUri::from("file:///src/index.ts");
        let (mut session, _) = projecttestutil::setup(files_no_config.clone());

        session.did_open_file(
            core::Context::default(),
            uri_local.clone(),
            1,
            files_no_config["/src/index.ts"].clone(),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        session
            .get_language_service(core::Context::default(), uri_local.clone())
            .expect("GetLanguageService should succeed");

        // Without any config, the file should be in the inferred project only.
        let snapshot = session.snapshot();
        assert_eq!(
            snapshot
                .get_default_project(uri_local.clone())
                .unwrap()
                .name(),
            "/dev/null/inferred"
        );
        let projects = snapshot.get_projects_containing_file(uri_local.clone());
        assert_eq!(
            projects.len(),
            1,
            "expected file to be in exactly 1 project before config change, got {}",
            projects.len()
        );

        // Now set custom config to pick up tsconfig.all.json
        let mut prefs = lsutil::new_default_user_preferences();
        prefs.custom_config_file_name = "tsconfig.all.json".to_string();
        session.configure(prefs);

        session
            .get_language_service(core::Context::default(), uri_local.clone())
            .expect("GetLanguageService should succeed");

        // File should now be in the configured project only, not duplicated in inferred.
        let snapshot = session.snapshot();
        assert_eq!(
            snapshot
                .get_default_project(uri_local.clone())
                .unwrap()
                .name(),
            "/src/tsconfig.all.json"
        );
        let projects = snapshot.get_projects_containing_file(uri_local.clone());
        assert_eq!(
            projects.len(),
            1,
            "expected file to be in exactly 1 project after config change, got {}",
            projects.len()
        );
    }
}
