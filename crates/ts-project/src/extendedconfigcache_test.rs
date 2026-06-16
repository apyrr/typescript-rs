use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::projecttestutil;
use ts_core as core;
use ts_lsproto as lsproto;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;
use ts_vfs as vfs;

use crate::client::{
    Context as ClientContext, DiagnosticsMessage, FileSystemWatcher, PublishDiagnosticsParams,
    TelemetryEvent, WatcherID,
};
use crate::{Client, Session, SessionInit, SessionOptions, new_session};

struct NoopClient;

impl Client for NoopClient {
    fn watch_files(
        &self,
        _ctx: &ClientContext,
        _id: WatcherID,
        _watchers: Vec<FileSystemWatcher>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn unwatch_files(&self, _ctx: &ClientContext, _id: WatcherID) -> Result<(), String> {
        Ok(())
    }

    fn refresh_diagnostics(&self, _ctx: &ClientContext) -> Result<(), String> {
        Ok(())
    }

    fn publish_diagnostics(
        &self,
        _ctx: &ClientContext,
        _params: PublishDiagnosticsParams,
    ) -> Result<(), String> {
        Ok(())
    }

    fn refresh_inlay_hints(&self, _ctx: &ClientContext) -> Result<(), String> {
        Ok(())
    }

    fn refresh_code_lens(&self, _ctx: &ClientContext) -> Result<(), String> {
        Ok(())
    }

    fn progress_start(&self, _message: &DiagnosticsMessage, _args: &[String]) {}

    fn progress_finish(&self, _message: &DiagnosticsMessage, _args: &[String]) {}

    fn send_telemetry(
        &self,
        _ctx: &ClientContext,
        _telemetry: TelemetryEvent,
    ) -> Result<(), String> {
        Ok(())
    }

    fn is_active(&self) -> bool {
        true
    }
}

// TestExtendedConfigCacheOwnership tests the invariant that each ExtendedSourceFile
// of a config in the ConfigFileRegistry is owned exactly once per snapshot that
// references it, and released exactly once when that snapshot is removed.
#[test]
fn test_extended_config_cache_ownership() {
    if !ts_bundled::EMBEDDED {
        return;
    }

    let setup = |files: HashMap<String, String>| -> Session {
        let fs_from_map = vfs::vfstest::from_map(files, false);
        let fs: Arc<dyn vfs::Fs + Send + Sync> = Arc::new(ts_bundled::wrap_fs(fs_from_map));
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
                debounce_delay: Duration::default(),
                locale: ts_locale::Locale::default(),
            },
            fs,
            client: Some(Arc::new(NoopClient)),
            logger: Arc::new(crate::logging::new_test_logger()),
            npm_executor: None,
            parse_cache: None,
        })
    };

    let mut untitled_seq = 0;
    let mut open_untitled = |session: &mut Session| {
        untitled_seq += 1;
        let uri = format!("untitled:Untitled-{untitled_seq}");
        session.did_open_file(
            core::Context::default(),
            uri,
            1,
            String::new(),
            "typescript".to_string(),
        );
    };

    // flushCloseProject is the canonical way to ensure project close work is applied.
    // Close the file, then open an unrelated file.
    let mut flush_close_project = |session: &mut Session, file_uri: String| {
        session.did_close_file(core::Context::default(), file_uri);
        open_untitled(session);
    };

    let owner_count = |session: &Session, path: tspath::Path| -> usize {
        let Some(entry) = session.extended_config_cache.entries.load(&path) else {
            return 0;
        };
        entry.owners.len()
    };

    let assert_no_entry = |session: &Session, file_name: &str| {
        let path = session.to_path(file_name);
        assert!(session.extended_config_cache.entries.load(&path).is_none());
    };

    let expected_extended_owner_counts =
        |session: &Session, snapshot: &crate::Snapshot| -> HashMap<tspath::Path, usize> {
            let mut result = HashMap::new();
            for cfg in snapshot
                .config_file_registry
                .as_ref()
                .unwrap()
                .configs
                .values()
            {
                let Some(command_line) = &cfg.command_line else {
                    continue;
                };
                for file in command_line.extended_source_files() {
                    *result.entry(session.to_path(file)).or_insert(0) += 1;
                }
            }
            result
        };

    let assert_extended_owner_counts_match_registry =
        |session: &Session, snapshot: &crate::Snapshot| {
            let expected = expected_extended_owner_counts(session, snapshot);
            for (path, want) in expected {
                let got = owner_count(session, path.clone());
                assert_eq!(got, want, "extended config {path} owner count mismatch");
            }
        };

    // multi-extends shared ancestor counted once
    {
        // One config extends *two* configs; both extend a shared root.
        // Expected behavior: ExtendedSourceFiles() is deduped, so the shared root should only
        // be ref'd once for this config.
        let files = HashMap::from([
            (
                "/project/tsconfig.json".to_string(),
                r#"{
                "extends": ["./tsconfig.base1.json", "./tsconfig.base2.json"]
            }"#
                .to_string(),
            ),
            (
                "/project/tsconfig.base1.json".to_string(),
                r#"{
                "extends": "./tsconfig.root.json",
                "compilerOptions": {"strict": true}
            }"#
                .to_string(),
            ),
            (
                "/project/tsconfig.base2.json".to_string(),
                r#"{
                "extends": "./tsconfig.root.json",
                "compilerOptions": {"noImplicitAny": true}
            }"#
                .to_string(),
            ),
            (
                "/project/tsconfig.root.json".to_string(),
                r#"{
                "compilerOptions": {"target": "ES2020"}
            }"#
                .to_string(),
            ),
            (
                "/project/src/main.ts".to_string(),
                "export const x = 1;".to_string(),
            ),
        ]);

        let mut session = setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///project/src/main.ts".to_string(),
            1,
            files["/project/src/main.ts"].clone(),
            "typescript".to_string(),
        );
        let snapshot = session.snapshot();

        let config = snapshot
            .config_file_registry
            .as_ref()
            .unwrap()
            .get_config("/project/tsconfig.json".to_string());
        assert!(config.is_some());
        let config = config.unwrap();
        // Shared root should only appear once in the flattened list.
        let mut root_count = 0;
        for f in config.extended_source_files() {
            if f == "/project/tsconfig.root.json" {
                root_count += 1;
            }
        }
        assert_eq!(root_count, 1);

        // And the cache owner counts should match the registry's deduped list.
        assert_extended_owner_counts_match_registry(&session, snapshot);

        flush_close_project(&mut session, "file:///project/src/main.ts".to_string());
        assert_no_entry(&session, "/project/tsconfig.base1.json");
        assert_no_entry(&session, "/project/tsconfig.base2.json");
        assert_no_entry(&session, "/project/tsconfig.root.json");
    }

    // ExtendedSourceFiles can contain same path twice (case-insensitive)
    {
        // This test is descriptive, not prescriptive. This seems bad and unintentional,
        // but is here to show that while the problem exists in the underlying config parsing
        // API, it doesn't disrupt cache ownership.
        let files = HashMap::from([
            (
                "/project/tsconfig.json".to_string(),
                r#"{
                "extends": ["./Shared.json", "./shared.json"]
            }"#
                .to_string(),
            ),
            (
                "/project/shared.json".to_string(),
                r#"{
                "compilerOptions": {"strict": true}
            }"#
                .to_string(),
            ),
        ]);

        // This test intentionally bypasses the project system's ExtendedConfigCache so we can
        // observe how ExtendedSourceFiles behaves when the same underlying file is referenced
        // with different casing on a case-insensitive FS.
        let fs_from_map = vfs::vfstest::from_map(files, false);
        let fs: Arc<dyn vfs::Fs + Send + Sync> = Arc::new(ts_bundled::wrap_fs(fs_from_map));

        // Minimal ParseConfigHost implementation.
        let h = TestParseConfigHost {
            fs,
            cwd: "/".to_string(),
        };
        let (cmd, diags) = tsoptions::get_parsed_command_line_of_config_file(
            "/project/tsconfig.json",
            None,
            None,
            &h,
            None,
        );
        assert_eq!(diags.len(), 0);
        assert!(cmd.is_some());
        let cmd = cmd.unwrap();

        let extended = cmd.extended_source_files();
        assert_eq!(extended.len(), 2);
        assert_eq!(extended[0], "/project/Shared.json");
        assert_eq!(extended[1], "/project/shared.json");
    }

    // project system dedupes case-only extends via cache
    {
        let files = HashMap::from([
            (
                "/project/tsconfig.json".to_string(),
                r#"{
                "extends": ["./Shared.json", "./shared.json"]
            }"#
                .to_string(),
            ),
            (
                "/project/shared.json".to_string(),
                r#"{
                "compilerOptions": {"strict": true}
            }"#
                .to_string(),
            ),
            (
                "/project/src/main.ts".to_string(),
                "export const x = 1;".to_string(),
            ),
        ]);

        let mut session = setup(files.clone());
        session.did_open_file(
            core::Context::default(),
            "file:///project/src/main.ts".to_string(),
            1,
            files["/project/src/main.ts"].clone(),
            "typescript".to_string(),
        );
        let snapshot = session.snapshot();

        let config = snapshot
            .config_file_registry
            .as_ref()
            .unwrap()
            .get_config("/project/tsconfig.json".to_string());
        assert!(config.is_some());
        let config = config.unwrap();
        let extended = config.extended_source_files();
        assert_eq!(extended.len(), 1);
        assert_eq!(
            session.to_path(&extended[0]),
            session.to_path("/project/shared.json")
        );
    }

    // transitive extended config ownership with new project
    {
        // Scenario: transitive extends chain where a new project reuses a cached
        // extended config without reparsing it, which should still acquire the transitive deps.
        //
        // projectA/tsconfig.json extends shared/tsconfig.base.json extends shared/tsconfig.common.json
        // projectB/tsconfig.json extends shared/tsconfig.base.json extends shared/tsconfig.common.json
        //
        // When projectB is opened AFTER projectA, tsconfig.base.json is retrieved from cache
        // (not reparsed), so tsconfig.common.json still needs projectB's snapshot ownership.
        let files = HashMap::from([
            (
                "/user/username/projects/shared/tsconfig.common.json".to_string(),
                r#"{
                    "compilerOptions": { "strict": true }
                }"#
                .to_string(),
            ),
            (
                "/user/username/projects/shared/tsconfig.base.json".to_string(),
                r#"{
                    "extends": "./tsconfig.common.json",
                    "compilerOptions": { "target": "ES2020" }
                }"#
                .to_string(),
            ),
            (
                "/user/username/projects/projectA/tsconfig.json".to_string(),
                r#"{
                    "extends": "../shared/tsconfig.base.json"
                }"#
                .to_string(),
            ),
            (
                "/user/username/projects/projectA/src/main.ts".to_string(),
                "const a = 1;".to_string(),
            ),
            (
                "/user/username/projects/projectB/tsconfig.json".to_string(),
                r#"{
                    "extends": "../shared/tsconfig.base.json"
                }"#
                .to_string(),
            ),
            (
                "/user/username/projects/projectB/src/main.ts".to_string(),
                "const b = 2;".to_string(),
            ),
            (
                "/user/username/projects/other/src/main.ts".to_string(),
                "const other = 3;".to_string(),
            ),
        ]);

        let mut session = setup(files.clone());

        // Step 1: Open file in projectA - this parses the full extends chain
        session.did_open_file(
            core::Context::default(),
            "file:///user/username/projects/projectA/src/main.ts".to_string(),
            1,
            files["/user/username/projects/projectA/src/main.ts"].clone(),
            "typescript".to_string(),
        );

        // Verify extended configs are in cache with correct owner counts
        let base_entry = session
            .extended_config_cache
            .entries
            .load("/user/username/projects/shared/tsconfig.base.json");
        let common_entry = session
            .extended_config_cache
            .entries
            .load("/user/username/projects/shared/tsconfig.common.json");
        assert!(
            base_entry.is_some(),
            "tsconfig.base.json should be in cache"
        );
        assert!(
            common_entry.is_some(),
            "tsconfig.common.json should be in cache"
        );
        assert_eq!(base_entry.unwrap().owners.len(), 1);
        assert_eq!(common_entry.unwrap().owners.len(), 1);

        // Step 2: Open file in projectB - this should acquire tsconfig.base.json from cache
        // (not reparse it), and should also acquire tsconfig.common.json.
        session.did_open_file(
            core::Context::default(),
            "file:///user/username/projects/projectB/src/main.ts".to_string(),
            1,
            files["/user/username/projects/projectB/src/main.ts"].clone(),
            "typescript".to_string(),
        );

        // Step 3: Close projectA file and open an unrelated file to force projectA cleanup
        session.did_close_file(
            core::Context::default(),
            "file:///user/username/projects/projectA/src/main.ts".to_string(),
        );
        // Opening another file triggers cleanup of closed projects
        session.did_open_file(
            core::Context::default(),
            "file:///user/username/projects/other/src/main.ts".to_string(),
            1,
            files["/user/username/projects/other/src/main.ts"].clone(),
            "typescript".to_string(),
        );

        // Close the other file too so only projectB remains
        session.did_close_file(
            core::Context::default(),
            "file:///user/username/projects/other/src/main.ts".to_string(),
        );

        // Step 4: Trigger another snapshot clone for projectB
        session.did_change_file(
            core::Context::default(),
            "file:///user/username/projects/projectB/src/main.ts".to_string(),
            2,
            vec![
                serde_json::from_value(serde_json::json!({
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 12 }
                    },
                    "text": "const b = 3;"
                }))
                .unwrap(),
            ],
        );
        // This call triggered the panic
        assert!(
            session
                .get_language_service(
                    core::Context::default(),
                    "file:///user/username/projects/projectB/src/main.ts".to_string()
                )
                .is_ok()
        );
    }
}

struct TestParseConfigHost {
    fs: Arc<dyn vfs::Fs + Send + Sync>,
    cwd: String,
}

impl tsoptions::ParseConfigHost for TestParseConfigHost {
    fn fs(&self) -> &dyn vfs::Fs {
        self.fs.as_ref()
    }

    fn get_current_directory(&self) -> String {
        self.cwd.clone()
    }
}
