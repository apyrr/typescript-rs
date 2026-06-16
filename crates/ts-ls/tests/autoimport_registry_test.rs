use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use ts_collections as collections;
use ts_core as core;
use ts_lsproto as lsproto;
use ts_project as project;
use ts_testutil::{autoimporttestutil, projecttestutil};
use ts_tspath as tspath;
use ts_vfs::vfstest;

use ts_ls as lsconv;
use ts_ls as lsutil;
use ts_ls::{AutoImportBucketStats as BucketStats, AutoImportCacheStats as CacheStats};

struct NoopClient;

impl project::Client for NoopClient {
    fn watch_files(
        &self,
        _ctx: &project::Context,
        _id: project::WatcherID,
        _watchers: Vec<project::FileSystemWatcher>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn unwatch_files(
        &self,
        _ctx: &project::Context,
        _id: project::WatcherID,
    ) -> Result<(), String> {
        Ok(())
    }

    fn refresh_diagnostics(&self, _ctx: &project::Context) -> Result<(), String> {
        Ok(())
    }

    fn publish_diagnostics(
        &self,
        _ctx: &project::Context,
        _params: project::PublishDiagnosticsParams,
    ) -> Result<(), String> {
        Ok(())
    }

    fn refresh_inlay_hints(&self, _ctx: &project::Context) -> Result<(), String> {
        Ok(())
    }

    fn refresh_code_lens(&self, _ctx: &project::Context) -> Result<(), String> {
        Ok(())
    }

    fn progress_start(&self, _message: &project::DiagnosticsMessage, _args: &[String]) {}

    fn progress_finish(&self, _message: &project::DiagnosticsMessage, _args: &[String]) {}

    fn send_telemetry(
        &self,
        _ctx: &project::Context,
        _telemetry: project::TelemetryEvent,
    ) -> Result<(), String> {
        Ok(())
    }

    fn is_active(&self) -> bool {
        true
    }
}

const LANGUAGE_KIND_JAVASCRIPT: &str = "javascript";
const LANGUAGE_KIND_TYPESCRIPT: &str = "typescript";
const LIFECYCLE_PROJECT_ROOT: &str = "/home/src/autoimport-lifecycle";
const MONOREPO_PROJECT_ROOT: &str = "/home/src/autoimport-monorepo";

#[test]
fn test_registry_lifecycle_builds_project_and_node_modules_buckets() {
    let fixture = autoimporttestutil::setup_lifecycle_session(LIFECYCLE_PROJECT_ROOT, 1);
    let mut session = setup_fixture(fixture.files.clone());
    let project = fixture.single_project();
    let main_file = project.file(0);
    let ctx = core::Context::default();
    session.did_open_file(
        ctx.clone(),
        main_file.file_handle.uri(),
        1,
        main_file.file_handle.content().to_string(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );

    let mut stats = auto_import_stats(&session);
    let mut project_bucket = single_bucket(&stats.project_buckets);
    let mut node_modules_bucket = single_bucket(&stats.node_modules_buckets);
    assert_eq!(project_bucket.state.dirty(), true);
    assert_eq!(project_bucket.file_count, 0);
    assert_eq!(node_modules_bucket.state.dirty(), true);
    assert_eq!(node_modules_bucket.file_count, 0);

    session
        .get_current_language_service_with_auto_imports(ctx, main_file.file_handle.uri())
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");

    stats = auto_import_stats(&session);
    project_bucket = single_bucket(&stats.project_buckets);
    node_modules_bucket = single_bucket(&stats.node_modules_buckets);
    assert_eq!(project_bucket.state.dirty(), false);
    assert!(project_bucket.export_count > 0);
    assert_eq!(node_modules_bucket.state.dirty(), false);
    assert!(node_modules_bucket.export_count > 0);
}

#[test]
fn test_registry_lifecycle_bucket_does_not_rebuild_on_same_file_change() {
    let fixture = autoimporttestutil::setup_lifecycle_session(LIFECYCLE_PROJECT_ROOT, 2);
    let mut session = setup_fixture(fixture.files.clone());
    let project = fixture.single_project();
    let main_file = project.file(0);
    let secondary_file = project.file(1);
    let ctx = core::Context::default();
    session.did_open_file(
        ctx.clone(),
        main_file.file_handle.uri(),
        1,
        main_file.file_handle.content().to_string(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session.did_open_file(
        ctx.clone(),
        secondary_file.file_handle.uri(),
        1,
        secondary_file.file_handle.content().to_string(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session
        .get_current_language_service_with_auto_imports(ctx.clone(), main_file.file_handle.uri())
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");

    let updated_content = format!("{}// change\n", main_file.file_handle.content());
    session.did_change_file(
        ctx.clone(),
        main_file.file_handle.uri(),
        2,
        vec![whole_document_change(&updated_content)],
    );

    session
        .get_language_service(ctx.clone(), main_file.file_handle.uri())
        .expect("GetLanguageService should succeed");

    let mut stats = auto_import_stats(&session);
    let mut project_bucket = single_bucket(&stats.project_buckets);
    let node_modules_bucket = single_bucket(&stats.node_modules_buckets);
    assert_eq!(project_bucket.state.dirty(), true);
    assert_eq!(
        project_bucket.state.dirty_file(),
        to_path(main_file.file_handle.file_name())
    );
    assert_eq!(node_modules_bucket.state.dirty(), false);
    assert_eq!(
        node_modules_bucket.state.dirty_file(),
        tspath::Path::default()
    );

    session
        .get_current_language_service_with_auto_imports(ctx.clone(), main_file.file_handle.uri())
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");
    stats = auto_import_stats(&session);
    project_bucket = single_bucket(&stats.project_buckets);
    assert_eq!(project_bucket.state.dirty(), true);
    assert_eq!(
        project_bucket.state.dirty_file(),
        to_path(main_file.file_handle.file_name())
    );

    session.did_change_file(
        ctx.clone(),
        secondary_file.file_handle.uri(),
        1,
        vec![whole_document_change("// new content")],
    );
    session
        .get_current_language_service_with_auto_imports(ctx, main_file.file_handle.uri())
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");
    stats = auto_import_stats(&session);
    project_bucket = single_bucket(&stats.project_buckets);
    assert_eq!(project_bucket.state.dirty(), false);
}

#[test]
fn test_registry_lifecycle_bucket_updates_on_same_file_change_when_new_files_added_to_the_program()
{
    let project_root = "/home/src/explicit-files-project";
    let files = HashMap::from([
        (
            format!("{project_root}/tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "module": "esnext",
                    "target": "esnext",
                    "strict": true
                },
                "files": ["index.ts"]
            }"#
            .to_string(),
        ),
        (format!("{project_root}/index.ts"), String::new()),
        (
            format!("{project_root}/utils.ts"),
            "export const foo = 1;\nexport const bar = 2;".to_string(),
        ),
    ]);
    let mut session = setup(files);
    let ctx = core::Context::default();
    let index_uri = format!("file://{project_root}/index.ts");

    session.did_open_file(
        ctx.clone(),
        index_uri.clone(),
        1,
        String::new(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session
        .get_current_language_service_with_auto_imports(ctx.clone(), index_uri.clone())
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");
    let mut stats = auto_import_stats(&session);
    let mut project_bucket = single_bucket(&stats.project_buckets);
    assert_eq!(project_bucket.file_count, 1);

    let new_content = r#"import { foo } from "./utils";"#;
    session.did_change_file(
        ctx.clone(),
        index_uri.clone(),
        2,
        vec![whole_document_change(new_content)],
    );

    session
        .get_current_language_service_with_auto_imports(ctx, index_uri)
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");
    stats = auto_import_stats(&session);
    project_bucket = single_bucket(&stats.project_buckets);
    assert_eq!(project_bucket.file_count, 2);
}

#[test]
fn test_registry_lifecycle_package_json_dependency_changes_invalidate_node_modules_buckets() {
    let fixture = autoimporttestutil::setup_lifecycle_session(LIFECYCLE_PROJECT_ROOT, 1);
    let mut session = setup_fixture(fixture.files.clone());
    let project = fixture.single_project();
    let main_file = project.file(0);
    let node_package = project.node_modules()[0].clone();
    let package_json = project.package_json_file();
    let ctx = core::Context::default();

    session.did_open_file(
        ctx.clone(),
        main_file.file_handle.uri(),
        1,
        main_file.file_handle.content().to_string(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session
        .get_current_language_service_with_auto_imports(ctx.clone(), main_file.file_handle.uri())
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");
    let mut stats = auto_import_stats(&session);
    let mut node_modules_bucket = single_bucket(&stats.node_modules_buckets);
    assert_eq!(node_modules_bucket.state.dirty(), false);

    let same_deps_content = format!(
        "{{\n  \"name\": \"local-project-stable\",\n  \"dependencies\": {{\n    \"{}\": \"*\"\n  }}\n}}\n",
        node_package.name
    );
    session
        .fs()
        .write_file(package_json.file_name(), &same_deps_content)
        .expect("package.json write should succeed");
    session.did_change_watched_files(
        ctx.clone(),
        vec![file_event(
            &package_json.uri(),
            lsproto::FileChangeType::Changed,
        )],
    );
    session
        .get_language_service(ctx.clone(), main_file.file_handle.uri())
        .expect("GetLanguageService should succeed");
    stats = auto_import_stats(&session);
    node_modules_bucket = single_bucket(&stats.node_modules_buckets);
    assert_eq!(node_modules_bucket.state.dirty(), false);

    let different_deps_content = format!(
        "{{\n  \"name\": \"local-project-stable\",\n  \"dependencies\": {{\n    \"{}\": \"*\",\n    \"newpkg\": \"*\"\n  }}\n}}\n",
        node_package.name
    );
    session
        .fs()
        .write_file(package_json.file_name(), &different_deps_content)
        .expect("package.json write should succeed");
    session.did_change_watched_files(
        ctx.clone(),
        vec![file_event(
            &package_json.uri(),
            lsproto::FileChangeType::Changed,
        )],
    );
    session
        .get_current_language_service_with_auto_imports(ctx, main_file.file_handle.uri())
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");
    stats = auto_import_stats(&session);
    assert!(
        single_bucket(&stats.node_modules_buckets)
            .dependency_names
            .as_ref()
            .unwrap()
            .has(&"newpkg".to_string())
    );
}

#[test]
fn test_registry_lifecycle_node_modules_buckets_get_deleted_when_no_open_files_can_reference_them()
{
    let fixture = autoimporttestutil::setup_monorepo_lifecycle_session(
        autoimporttestutil::MonorepoSetupConfig {
            root: MONOREPO_PROJECT_ROOT.to_string(),
            package_template: autoimporttestutil::MonorepoPackageTemplate {
                name: "monorepo".to_string(),
                node_module_names: vec!["pkg-root".to_string()],
                ..Default::default()
            },
            packages: vec![
                autoimporttestutil::MonorepoPackageConfig {
                    file_count: 1,
                    package_template: autoimporttestutil::MonorepoPackageTemplate {
                        name: "package-a".to_string(),
                        node_module_names: vec!["pkg-a".to_string()],
                        ..Default::default()
                    },
                },
                autoimporttestutil::MonorepoPackageConfig {
                    file_count: 1,
                    package_template: autoimporttestutil::MonorepoPackageTemplate {
                        name: "package-b".to_string(),
                        node_module_names: vec!["pkg-b".to_string()],
                        ..Default::default()
                    },
                },
            ],
            ..Default::default()
        },
    );
    let mut session = setup_fixture(fixture.files.clone());
    let monorepo = fixture.monorepo();
    let pkg_a = monorepo.package(0);
    let pkg_b = monorepo.package(1);
    let file_a = pkg_a.file(0);
    let file_b = pkg_b.file(0);
    let ctx = core::Context::default();

    session.did_open_file(
        ctx.clone(),
        file_a.file_handle.uri(),
        1,
        file_a.file_handle.content().to_string(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session
        .get_current_language_service_with_auto_imports(ctx.clone(), file_a.file_handle.uri())
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");

    session.did_open_file(
        ctx.clone(),
        file_b.file_handle.uri(),
        1,
        file_b.file_handle.content().to_string(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session
        .get_current_language_service_with_auto_imports(ctx.clone(), file_b.file_handle.uri())
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");
    let mut stats = auto_import_stats(&session);
    assert_eq!(stats.node_modules_buckets.len(), 3);
    assert_eq!(stats.project_buckets.len(), 2);

    session.did_close_file(ctx.clone(), file_a.file_handle.uri());
    session
        .get_current_language_service_with_auto_imports(ctx, file_b.file_handle.uri())
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");
    stats = auto_import_stats(&session);
    assert_eq!(stats.node_modules_buckets.len(), 2);
    assert_eq!(stats.project_buckets.len(), 1);
}

#[test]
fn test_registry_lifecycle_node_modules_bucket_dependency_selection_changes_with_open_files() {
    let monorepo_root = "/home/src/monorepo";
    let package_a_dir = tspath::combine_paths(monorepo_root, &["packages", "a"]);
    let monorepo_index = tspath::combine_paths(monorepo_root, &["index.js"]);
    let package_a_index = tspath::combine_paths(&package_a_dir, &["index.js"]);
    let fixture = autoimporttestutil::setup_monorepo_lifecycle_session(
        autoimporttestutil::MonorepoSetupConfig {
            root: monorepo_root.to_string(),
            package_template: autoimporttestutil::MonorepoPackageTemplate {
                name: "monorepo".to_string(),
                node_module_names: vec!["pkg1".to_string(), "pkg2".to_string(), "pkg3".to_string()],
                dependency_names: vec!["pkg1".to_string()],
            },
            packages: vec![autoimporttestutil::MonorepoPackageConfig {
                file_count: 0,
                package_template: autoimporttestutil::MonorepoPackageTemplate {
                    name: "a".to_string(),
                    dependency_names: vec!["pkg1".to_string(), "pkg2".to_string()],
                    ..Default::default()
                },
            }],
            extra_files: vec![
                autoimporttestutil::TextFileSpec {
                    path: monorepo_index.clone(),
                    content: "export const monorepoIndex = 1;\n".to_string(),
                },
                autoimporttestutil::TextFileSpec {
                    path: package_a_index.clone(),
                    content: "export const pkgA = 2;\n".to_string(),
                },
            ],
            ..Default::default()
        },
    );
    let mut session = setup_fixture(fixture.files.clone());
    let monorepo_handle = fixture.extra_file(&monorepo_index);
    let package_a_handle = fixture.extra_file(&package_a_index);
    let ctx = core::Context::default();

    session.did_open_file(
        ctx.clone(),
        monorepo_handle.uri(),
        1,
        monorepo_handle.content().to_string(),
        LANGUAGE_KIND_JAVASCRIPT.to_string(),
    );
    session
        .get_current_language_service_with_auto_imports(ctx.clone(), monorepo_handle.uri())
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");
    let mut stats = auto_import_stats(&session);
    assert!(
        single_bucket(&stats.node_modules_buckets)
            .dependency_names
            .as_ref()
            .unwrap()
            .equals(&set(&["pkg1"]))
    );

    session.did_open_file(
        ctx.clone(),
        package_a_handle.uri(),
        1,
        package_a_handle.content().to_string(),
        LANGUAGE_KIND_JAVASCRIPT.to_string(),
    );
    session
        .get_current_language_service_with_auto_imports(ctx.clone(), package_a_handle.uri())
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");
    stats = auto_import_stats(&session);
    assert!(
        single_bucket(&stats.node_modules_buckets)
            .dependency_names
            .as_ref()
            .unwrap()
            .equals(&set(&["pkg1", "pkg2"]))
    );

    session.did_close_file(ctx.clone(), package_a_handle.uri());
    session
        .get_current_language_service_with_auto_imports(ctx.clone(), monorepo_handle.uri())
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");
    stats = auto_import_stats(&session);
    assert!(
        single_bucket(&stats.node_modules_buckets)
            .dependency_names
            .as_ref()
            .unwrap()
            .equals(&set(&["pkg1"]))
    );

    session.did_close_file(ctx.clone(), monorepo_handle.uri());
    session.did_open_file(
        ctx.clone(),
        "untitled:Untitled-1".to_string(),
        0,
        String::new(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session
        .get_language_service(ctx, "untitled:Untitled-1".to_string())
        .expect("GetLanguageService should succeed");
    stats = auto_import_stats(&session);
    assert_eq!(stats.node_modules_buckets.len(), 0);
}

#[test]
fn test_registry_lifecycle_node_modules_bucket_includes_resolved_packages_from_all_projects() {
    let monorepo_root = "/home/src/cross-project-deps";
    let package_a_dir = tspath::combine_paths(monorepo_root, &["packages", "a"]);
    let package_b_dir = tspath::combine_paths(monorepo_root, &["packages", "b"]);
    let package_a_index = tspath::combine_paths(&package_a_dir, &["index.ts"]);
    let package_b_index = tspath::combine_paths(&package_b_dir, &["index.ts"]);
    let fixture = autoimporttestutil::setup_monorepo_lifecycle_session(autoimporttestutil::MonorepoSetupConfig {
        root: monorepo_root.to_string(),
        package_template: autoimporttestutil::MonorepoPackageTemplate { name: "monorepo".to_string(), node_module_names: vec!["pkg-listed".to_string(), "pkg-unlisted".to_string()], dependency_names: vec!["pkg-listed".to_string()] },
        packages: vec![
            autoimporttestutil::MonorepoPackageConfig { file_count: 0, package_template: autoimporttestutil::MonorepoPackageTemplate { name: "a".to_string(), dependency_names: vec!["pkg-listed".to_string()], ..Default::default() } },
            autoimporttestutil::MonorepoPackageConfig { file_count: 0, package_template: autoimporttestutil::MonorepoPackageTemplate { name: "b".to_string(), dependency_names: vec!["pkg-listed".to_string()], ..Default::default() } },
        ],
        extra_files: vec![
            autoimporttestutil::TextFileSpec { path: package_a_index.clone(), content: "import { pkg_unlisted_value } from \"pkg-unlisted\";\nexport const a = pkg_unlisted_value;\n".to_string() },
            autoimporttestutil::TextFileSpec { path: package_b_index.clone(), content: "export const b = 1;\n".to_string() },
        ],
        ..Default::default()
    });
    let mut session = setup_fixture(fixture.files.clone());
    let package_a_handle = fixture.extra_file(&package_a_index);
    let package_b_handle = fixture.extra_file(&package_b_index);
    let ctx = core::Context::default();

    session.did_open_file(
        ctx.clone(),
        package_a_handle.uri(),
        1,
        package_a_handle.content().to_string(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session
        .get_current_language_service_with_auto_imports(ctx.clone(), package_a_handle.uri())
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");
    session.did_open_file(
        ctx.clone(),
        package_b_handle.uri(),
        1,
        package_b_handle.content().to_string(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session
        .get_current_language_service_with_auto_imports(ctx, package_b_handle.uri())
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");

    let stats = auto_import_stats(&session);
    let node_modules_bucket = single_bucket(&stats.node_modules_buckets);
    let dependency_names = node_modules_bucket
        .dependency_names
        .as_ref()
        .expect("DependencyNames should not be nil");
    assert!(
        dependency_names.has(&"pkg-listed".to_string()),
        "pkg-listed should be in dependencies"
    );
    assert!(
        dependency_names.has(&"pkg-unlisted".to_string()),
        "pkg-unlisted should be in dependencies because project-a imports it"
    );
}

#[test]
fn test_registry_lifecycle_symlinked_monorepo_invalidates_on_source_file_change() {
    let monorepo_root = "/home/src/symlinked-monorepo-invalidation";
    let project_a_dir = tspath::combine_paths(monorepo_root, &["packages", "project-a"]);
    let project_b_dir = tspath::combine_paths(monorepo_root, &["packages", "project-b"]);
    let project_a_index = tspath::combine_paths(&project_a_dir, &["src", "index.ts"]);
    let project_b_src_index = tspath::combine_paths(&project_b_dir, &["src", "index.ts"]);
    let project_b_dist_index = tspath::combine_paths(&project_b_dir, &["dist", "index.d.ts"]);
    let other_pkg_dir = tspath::combine_paths(&project_a_dir, &["node_modules", "other-pkg"]);
    let files = HashMap::from([
        (tspath::combine_paths(&project_b_dir, &["tsconfig.json"]), "{\n\t\"compilerOptions\": {\n\t\t\"composite\": true,\n\t\t\"outDir\": \"./dist\",\n\t\t\"rootDir\": \"./src\",\n\t\t\"declaration\": true,\n\t\t\"module\": \"esnext\",\n\t\t\"strict\": true\n\t},\n\t\"include\": [\"src\"]\n}".to_string()),
        (tspath::combine_paths(&project_b_dir, &["package.json"]), "{\n\t\"name\": \"project-b\",\n\t\"version\": \"1.0.0\",\n\t\"main\": \"dist/index.js\",\n\t\"types\": \"dist/index.d.ts\"\n}".to_string()),
        (project_b_src_index.clone(), "export function projectBFunction(): string { return \"hello\"; }\nexport const projectBValue: number = 42;".to_string()),
        (project_b_dist_index.clone(), "export declare function projectBFunction(): string;\nexport declare const projectBValue: number;".to_string()),
        (tspath::combine_paths(&other_pkg_dir, &["package.json"]), "{\n\t\"name\": \"other-pkg\",\n\t\"version\": \"1.0.0\",\n\t\"main\": \"index.js\",\n\t\"types\": \"index.d.ts\"\n}".to_string()),
        (tspath::combine_paths(&other_pkg_dir, &["index.d.ts"]), "export declare function otherFunction(): void;\nexport declare const otherValue: string;".to_string()),
        (tspath::combine_paths(&project_a_dir, &["tsconfig.json"]), "{\n\t\"compilerOptions\": {\n\t\t\"module\": \"esnext\",\n\t\t\"strict\": true,\n\t\t\"outDir\": \"./dist\",\n\t\t\"rootDir\": \"./src\"\n\t},\n\t\"include\": [\"src\"],\n\t\"references\": [{ \"path\": \"../project-b\" }]\n}".to_string()),
        (tspath::combine_paths(&project_a_dir, &["package.json"]), "{\n\t\"name\": \"project-a\",\n\t\"dependencies\": { \"project-b\": \"*\", \"other-pkg\": \"*\" }\n}".to_string()),
        (project_a_index.clone(), "console.log(\"hello\");\n".to_string()),
    ]);
    let mut session = setup_with_symlinks(
        files,
        &[(
            tspath::combine_paths(&project_a_dir, &["node_modules", "project-b"]),
            project_b_dir.clone(),
        )],
    );
    let ctx = core::Context::default();
    let project_a_uri = lsconv::file_name_to_document_uri(&project_a_index);
    session.did_open_file(
        ctx.clone(),
        project_a_uri.clone(),
        1,
        read_file_or_panic(&session, &project_a_index),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session
        .get_current_language_service_with_auto_imports(ctx.clone(), project_a_uri.clone())
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");

    let mut stats = auto_import_stats(&session);
    let mut node_modules_bucket = single_bucket(&stats.node_modules_buckets);
    let initial_file_count = node_modules_bucket.file_count;
    assert_eq!(
        node_modules_bucket.state.dirty(),
        false,
        "bucket should be clean initially"
    );
    assert!(initial_file_count > 0, "bucket should have files initially");

    let project_b_uri = lsconv::file_name_to_document_uri(&project_b_src_index);
    session.did_open_file(
        ctx.clone(),
        project_b_uri.clone(),
        1,
        read_file_or_panic(&session, &project_b_src_index),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session.did_change_file(
        ctx.clone(),
        project_b_uri,
        2,
        vec![whole_document_change(
            "export const projectBValue: number = 42;",
        )],
    );

    session
        .get_language_service(ctx.clone(), project_a_uri.clone())
        .expect("GetLanguageService should succeed");
    stats = auto_import_stats(&session);
    node_modules_bucket = single_bucket(&stats.node_modules_buckets);
    assert_eq!(
        node_modules_bucket.state.dirty(),
        true,
        "bucket should be dirty after source file change"
    );
    let dirty_packages = node_modules_bucket
        .state
        .dirty_packages()
        .expect("dirty packages should be tracked");
    assert!(
        dirty_packages.has(&"project-b".to_string()),
        "project-b should be in dirty packages"
    );
    assert!(
        !dirty_packages.has(&"other-pkg".to_string()),
        "other-pkg should NOT be in dirty packages"
    );
    assert_eq!(dirty_packages.len(), 1, "only one package should be dirty");

    session
        .get_current_language_service_with_auto_imports(ctx, project_a_uri)
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");
    stats = auto_import_stats(&session);
    node_modules_bucket = single_bucket(&stats.node_modules_buckets);
    assert_eq!(
        node_modules_bucket.state.dirty(),
        false,
        "bucket should be clean after rebuild"
    );
}

#[test]
fn test_registry_lifecycle_pnpm_style_symlinks_only_grant_granular_updates_to_workspace_packages() {
    let monorepo_root = "/home/src/pnpm-monorepo";
    let project_a_dir = tspath::combine_paths(monorepo_root, &["packages", "project-a"]);
    let project_b_dir = tspath::combine_paths(monorepo_root, &["packages", "project-b"]);
    let project_a_index = tspath::combine_paths(&project_a_dir, &["src", "index.ts"]);
    let project_b_src_index = tspath::combine_paths(&project_b_dir, &["src", "index.ts"]);
    let project_b_dist_index = tspath::combine_paths(&project_b_dir, &["dist", "index.d.ts"]);
    let pnpm_store_dir = tspath::combine_paths(
        &project_a_dir,
        &["node_modules", ".pnpm-store", "other-pkg@1.0.0"],
    );
    let other_pkg_index = tspath::combine_paths(&pnpm_store_dir, &["index.d.ts"]);
    let files = HashMap::from([
        (tspath::combine_paths(&project_b_dir, &["tsconfig.json"]), "{\n\t\"compilerOptions\": {\n\t\t\"composite\": true,\n\t\t\"outDir\": \"./dist\",\n\t\t\"rootDir\": \"./src\",\n\t\t\"declaration\": true,\n\t\t\"module\": \"esnext\",\n\t\t\"strict\": true\n\t},\n\t\"include\": [\"src\"]\n}".to_string()),
        (tspath::combine_paths(&project_b_dir, &["package.json"]), "{\n\t\"name\": \"project-b\",\n\t\"version\": \"1.0.0\",\n\t\"main\": \"dist/index.js\",\n\t\"types\": \"dist/index.d.ts\"\n}".to_string()),
        (project_b_src_index.clone(), "export function projectBFunction(): string { return \"hello\"; }\nexport const projectBValue: number = 42;".to_string()),
        (project_b_dist_index, "export declare function projectBFunction(): string;\nexport declare const projectBValue: number;".to_string()),
        (tspath::combine_paths(&pnpm_store_dir, &["package.json"]), "{\n\t\"name\": \"other-pkg\",\n\t\"version\": \"1.0.0\",\n\t\"main\": \"index.js\",\n\t\"types\": \"index.d.ts\"\n}".to_string()),
        (other_pkg_index.clone(), "export declare function otherFunction(): void;\nexport declare const otherValue: string;".to_string()),
        (tspath::combine_paths(&project_a_dir, &["tsconfig.json"]), "{\n\t\"compilerOptions\": {\n\t\t\"module\": \"esnext\",\n\t\t\"strict\": true,\n\t\t\"outDir\": \"./dist\",\n\t\t\"rootDir\": \"./src\"\n\t},\n\t\"include\": [\"src\"],\n\t\"references\": [{ \"path\": \"../project-b\" }]\n}".to_string()),
        (tspath::combine_paths(&project_a_dir, &["package.json"]), "{\n\t\"name\": \"project-a\",\n\t\"dependencies\": { \"project-b\": \"*\", \"other-pkg\": \"*\" }\n}".to_string()),
        (project_a_index.clone(), "console.log(\"hello\");\n".to_string()),
    ]);
    let mut session = setup_with_symlinks(
        files,
        &[
            (
                tspath::combine_paths(&project_a_dir, &["node_modules", "project-b"]),
                project_b_dir.clone(),
            ),
            (
                tspath::combine_paths(&project_a_dir, &["node_modules", "other-pkg"]),
                pnpm_store_dir.clone(),
            ),
        ],
    );
    let ctx = core::Context::default();
    let project_a_uri = lsconv::file_name_to_document_uri(&project_a_index);
    let project_a_content = read_file_or_panic(&session, &project_a_index);
    session.did_open_file(
        ctx.clone(),
        project_a_uri.clone(),
        1,
        project_a_content,
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session
        .get_current_language_service_with_auto_imports(ctx.clone(), project_a_uri.clone())
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");

    let mut stats = auto_import_stats(&session);
    let mut node_modules_bucket = single_bucket(&stats.node_modules_buckets);
    assert_eq!(
        node_modules_bucket.state.dirty(),
        false,
        "bucket should be clean initially"
    );

    let project_b_uri = lsconv::file_name_to_document_uri(&project_b_src_index);
    let project_b_content = read_file_or_panic(&session, &project_b_src_index);
    session.did_open_file(
        ctx.clone(),
        project_b_uri.clone(),
        1,
        project_b_content,
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session.did_change_file(
        ctx.clone(),
        project_b_uri,
        2,
        vec![whole_document_change(
            "export const projectBValue: number = 42;",
        )],
    );

    session
        .get_language_service(ctx.clone(), project_a_uri.clone())
        .expect("GetLanguageService should succeed");
    stats = auto_import_stats(&session);
    node_modules_bucket = single_bucket(&stats.node_modules_buckets);
    assert_eq!(
        node_modules_bucket.state.dirty(),
        true,
        "bucket should be dirty after workspace package change"
    );
    let mut dirty_packages = node_modules_bucket
        .state
        .dirty_packages()
        .expect("dirty packages should be tracked for workspace package");
    assert!(
        dirty_packages.has(&"project-b".to_string()),
        "project-b should be in dirty packages"
    );
    assert_eq!(dirty_packages.len(), 1, "only project-b should be dirty");

    session
        .get_current_language_service_with_auto_imports(ctx.clone(), project_a_uri.clone())
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");
    stats = auto_import_stats(&session);
    node_modules_bucket = single_bucket(&stats.node_modules_buckets);
    assert_eq!(
        node_modules_bucket.state.dirty(),
        false,
        "bucket should be clean after rebuild"
    );

    let other_pkg_uri = lsconv::file_name_to_document_uri(&other_pkg_index);
    session.did_open_file(
        ctx.clone(),
        other_pkg_uri.clone(),
        1,
        read_file_or_panic(&session, &other_pkg_index),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session.did_change_file(
        ctx.clone(),
        other_pkg_uri,
        2,
        vec![whole_document_change(
            "export declare function otherFunction(): void;",
        )],
    );

    stats = auto_import_stats(&session);
    node_modules_bucket = single_bucket(&stats.node_modules_buckets);
    assert_eq!(
        node_modules_bucket.state.dirty(),
        true,
        "bucket should be dirty after registry package change"
    );
    if let Some(packages) = node_modules_bucket.state.dirty_packages() {
        dirty_packages = packages;
        assert!(
            !dirty_packages.has(&"other-pkg".to_string()),
            "other-pkg should NOT be in dirty packages (should trigger full rebuild)"
        );
    }
}

#[test]
fn test_registry_lifecycle_changed_file_exclude_patterns_triggers_bucket_rebuild() {
    let fixture = autoimporttestutil::setup_lifecycle_session(LIFECYCLE_PROJECT_ROOT, 1);
    let mut session = setup_fixture(fixture.files.clone());
    let project = fixture.single_project();
    let main_file = project.file(0);
    let ctx = core::Context::default();
    session.did_open_file(
        ctx.clone(),
        main_file.file_handle.uri(),
        1,
        main_file.file_handle.content().to_string(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session
        .get_current_language_service_with_auto_imports(ctx.clone(), main_file.file_handle.uri())
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");

    let stats = auto_import_stats(&session);
    let project_bucket = single_bucket(&stats.project_buckets);
    let node_modules_bucket = single_bucket(&stats.node_modules_buckets);
    assert_eq!(false, project_bucket.state.dirty());
    assert_eq!(false, node_modules_bucket.state.dirty());

    let default_project = session
        .current_default_project_info(main_file.file_handle.uri())
        .expect("default project should exist");
    let project_path = default_project.id.clone();
    let mut preferences = lsutil::new_default_user_preferences();
    preferences.include_completions_for_module_exports = core::TSTrue;
    preferences.include_completions_for_import_statements = core::TSTrue;
    let is_prepared = session
        .current_auto_import_registry_is_prepared_for_importing_file(
            main_file.file_handle.file_name(),
            project_path.clone(),
            preferences.clone(),
        )
        .expect("auto import registry");
    assert!(is_prepared);

    let mut new_preferences = lsutil::new_default_user_preferences();
    new_preferences.include_completions_for_module_exports = core::TSTrue;
    new_preferences.include_completions_for_import_statements = core::TSTrue;
    new_preferences.auto_import_file_exclude_patterns =
        vec!["**/node_modules/**/*.d.ts".to_string()];
    session.configure(new_preferences.clone());

    let is_prepared2 = session
        .current_auto_import_registry_is_prepared_for_importing_file(
            main_file.file_handle.file_name(),
            project_path.clone(),
            new_preferences.clone(),
        )
        .expect("auto import registry");
    assert!(!is_prepared2);

    session
        .get_current_language_service_with_auto_imports(ctx, main_file.file_handle.uri())
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");
    let is_prepared3 = session
        .current_auto_import_registry_is_prepared_for_importing_file(
            main_file.file_handle.file_name(),
            project_path,
            new_preferences,
        )
        .expect("auto import registry");
    assert!(
        is_prepared3,
        "IsPreparedForImportingFile should return true after bucket rebuild with new fileExcludePatterns"
    );
}

#[test]
fn test_registry_lifecycle_dedupes_packages_that_resolve_to_same_realpath_across_ancestor_node_modules_buckets()
 {
    let repo_root = "/home/src/autoimport-realpath-dedupe";
    let app_dir = tspath::combine_paths(repo_root, &["apps", "web"]);
    let shared_pkg_dir = tspath::combine_paths(repo_root, &["node_modules", "shared"]);
    let app_index = tspath::combine_paths(&app_dir, &["src", "index.ts"]);
    let files = HashMap::from([
        (tspath::combine_paths(repo_root, &["package.json"]), "{\n\t\"name\": \"repo-root\",\n\t\"private\": true,\n\t\"dependencies\": { \"shared\": \"*\" }\n}".to_string()),
        (tspath::combine_paths(repo_root, &["tsconfig.json"]), "{\n\t\"compilerOptions\": {\n\t\t\"module\": \"esnext\",\n\t\t\"target\": \"esnext\",\n\t\t\"strict\": true\n\t},\n\t\"include\": [\"apps/**/*\"]\n}".to_string()),
        (tspath::combine_paths(&app_dir, &["package.json"]), "{\n\t\"name\": \"web\",\n\t\"private\": true,\n\t\"dependencies\": { \"shared\": \"*\" }\n}".to_string()),
        (tspath::combine_paths(&app_dir, &["tsconfig.json"]), "{\n\t\"compilerOptions\": {\n\t\t\"module\": \"esnext\",\n\t\t\"target\": \"esnext\",\n\t\t\"strict\": true\n\t},\n\t\"include\": [\"src\"]\n}".to_string()),
        (app_index.clone(), "export const app = 1;\n".to_string()),
        (tspath::combine_paths(&shared_pkg_dir, &["package.json"]), "{\n\t\"name\": \"shared\",\n\t\"version\": \"1.0.0\",\n\t\"types\": \"index.d.ts\"\n}".to_string()),
        (tspath::combine_paths(&shared_pkg_dir, &["index.d.ts"]), "export declare const sharedValue: 1;\n".to_string()),
    ]);
    let mut session = setup_with_symlinks(
        files,
        &[(
            tspath::combine_paths(&app_dir, &["node_modules", "shared"]),
            shared_pkg_dir,
        )],
    );
    let ctx = core::Context::default();
    let app_uri = lsconv::file_name_to_document_uri(&app_index);
    session.did_open_file(
        ctx.clone(),
        app_uri.clone(),
        1,
        "export const app = 1;\n".to_string(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session
        .get_current_language_service_with_auto_imports(ctx, app_uri)
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");

    let stats = auto_import_stats(&session);
    assert_eq!(
        stats.node_modules_buckets.len(),
        2,
        "expected both app and repo node_modules buckets"
    );
    assert_eq!(
        stats.unique_package_count, 1,
        "expected one unique package after realpath dedup"
    );
}

#[test]
fn test_hidden_directories_in_node_modules_deep_import_through_subdirectory_package_json_in_hidden_store()
 {
    let project_root = "/home/src/fuse-project";
    let store_dir = format!("{project_root}/node_modules/.yarn-store");
    let pkg_store_dir = format!("{store_dir}/some-pkg-npm-1.0.0-abc123/package");
    let files = HashMap::from([
        (format!("{project_root}/tsconfig.json"), "{\n\t\"compilerOptions\": {\n\t\t\"module\": \"commonjs\",\n\t\t\"target\": \"es2020\",\n\t\t\"strict\": true\n\t}\n}".to_string()),
        (format!("{project_root}/package.json"), "{\n\t\"name\": \"test-project\",\n\t\"dependencies\": {\n\t\t\"some-pkg\": \"*\",\n\t\t\"real-package\": \"*\"\n\t}\n}".to_string()),
        (format!("{project_root}/index.ts"), "import { debug } from \"some-pkg/debug\";".to_string()),
        (format!("{project_root}/node_modules/real-package/package.json"), "{\"name\":\"real-package\",\"version\":\"1.0.0\",\"types\":\"index.d.ts\"}".to_string()),
        (format!("{project_root}/node_modules/real-package/index.d.ts"), "export declare const realExport: number;\n".to_string()),
        (format!("{pkg_store_dir}/package.json"), "{\"name\":\"some-pkg\",\"version\":\"1.0.0\",\"types\":\"index.d.ts\"}".to_string()),
        (format!("{pkg_store_dir}/index.d.ts"), "export declare const something: number;\n".to_string()),
        (format!("{pkg_store_dir}/debug/package.json"), "{\"main\":\"./debug.js\",\"types\":\"./debug.d.ts\"}".to_string()),
        (format!("{pkg_store_dir}/debug/debug.d.ts"), "export declare function debug(msg: string): void;\n".to_string()),
        (format!("{pkg_store_dir}/debug/debug.js"), "exports.debug = function(msg) { console.log(msg); };\n".to_string()),
        (format!("{store_dir}/other-pkg-npm-2.0.0-def456/package/package.json"), "{\"name\":\"other-pkg\",\"version\":\"1.0.0\",\"types\":\"index.d.ts\"}".to_string()),
        (format!("{store_dir}/other-pkg-npm-2.0.0-def456/package/index.d.ts"), "export declare const other: string;\n".to_string()),
    ]);
    let mut session = setup_with_symlinks(
        files,
        &[(
            format!("{project_root}/node_modules/some-pkg"),
            pkg_store_dir,
        )],
    );
    let ctx = core::Context::default();
    let index_uri = format!("file://{project_root}/index.ts");
    session.did_open_file(
        ctx.clone(),
        index_uri.clone(),
        1,
        "import { debug } from \"some-pkg/debug\";".to_string(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session
        .get_current_language_service_with_auto_imports(ctx, index_uri)
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");

    let stats = auto_import_stats(&session);
    let node_modules_bucket = single_bucket(&stats.node_modules_buckets);
    let dependency_names = node_modules_bucket
        .dependency_names
        .as_ref()
        .expect("DependencyNames should not be nil");
    for name in dependency_names.keys().unwrap().iter() {
        assert!(
            !name.starts_with('.'),
            "hidden directory {name:?} should not appear as a dependency name"
        );
    }
}

#[test]
fn test_auto_import_entrypoint_directory_search_default_limits_to_main_entrypoint() {
    let (project_root, files) = entrypoint_directory_search_files();
    let mut session = setup(files.clone());
    let ctx = core::Context::default();
    let index_uri = format!("file://{project_root}/index.ts");
    session.did_open_file(
        ctx.clone(),
        index_uri.clone(),
        1,
        files[&format!("{project_root}/index.ts")].clone(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session
        .get_current_language_service_with_auto_imports(ctx, index_uri)
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");
    let stats = auto_import_stats(&session);
    let node_modules_bucket = single_bucket(&stats.node_modules_buckets);
    assert_eq!(
        node_modules_bucket.file_count, 1,
        "expected only 1 file (main entrypoint) by default"
    );
}

#[test]
fn test_auto_import_entrypoint_directory_search_auto_import_entrypoint_directory_search_enables_all_files()
 {
    let (project_root, files) = entrypoint_directory_search_files();
    let mut session = setup(files.clone());
    let mut prefs = lsutil::new_default_user_preferences();
    prefs.auto_import_entrypoint_directory_search = core::TSTrue;
    session.configure(prefs);
    let ctx = core::Context::default();
    let index_uri = format!("file://{project_root}/index.ts");
    session.did_open_file(
        ctx.clone(),
        index_uri.clone(),
        1,
        files[&format!("{project_root}/index.ts")].clone(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session
        .get_current_language_service_with_auto_imports(ctx, index_uri)
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");
    let stats = auto_import_stats(&session);
    let node_modules_bucket = single_bucket(&stats.node_modules_buckets);
    assert!(
        node_modules_bucket.file_count >= 4,
        "expected at least 4 files from directory search, got {}",
        node_modules_bucket.file_count
    );
}

#[test]
fn test_auto_import_entrypoint_directory_search_changing_preference_triggers_rebuild() {
    let (project_root, files) = entrypoint_directory_search_files();
    let mut session = setup(files.clone());
    let ctx = core::Context::default();
    let index_uri = format!("file://{project_root}/index.ts");
    session.did_open_file(
        ctx.clone(),
        index_uri.clone(),
        1,
        files[&format!("{project_root}/index.ts")].clone(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session
        .get_current_language_service_with_auto_imports(ctx.clone(), index_uri.clone())
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");
    let mut stats = auto_import_stats(&session);
    let mut node_modules_bucket = single_bucket(&stats.node_modules_buckets);
    assert_eq!(
        node_modules_bucket.file_count, 1,
        "expected only 1 file initially"
    );

    let mut prefs = lsutil::new_default_user_preferences();
    prefs.auto_import_entrypoint_directory_search = core::TSTrue;
    session.configure(prefs.clone());

    let default_project = session
        .current_default_project_info(index_uri.clone())
        .expect("default project should exist");
    let project_path = default_project.id.clone();
    let is_prepared = session
        .current_auto_import_registry_is_prepared_for_importing_file(
            &format!("{project_root}/index.ts"),
            project_path,
            prefs,
        )
        .expect("auto import registry");
    assert!(
        !is_prepared,
        "registry should not be prepared after preference change"
    );

    session
        .get_current_language_service_with_auto_imports(ctx, index_uri)
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");
    stats = auto_import_stats(&session);
    node_modules_bucket = single_bucket(&stats.node_modules_buckets);
    assert!(
        node_modules_bucket.file_count >= 4,
        "expected at least 4 files after rebuild with directory search enabled, got {}",
        node_modules_bucket.file_count
    );
}

#[test]
fn test_auto_import_entrypoint_directory_search_deep_import_from_program_update_enables_recursive_search_for_that_package()
 {
    let (project_root, files) = entrypoint_directory_search_files();
    let mut session = setup(files.clone());
    let ctx = core::Context::default();
    let index_uri = format!("file://{project_root}/index.ts");
    session.did_open_file(
        ctx.clone(),
        index_uri.clone(),
        1,
        files[&format!("{project_root}/index.ts")].clone(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session
        .get_current_language_service_with_auto_imports(ctx.clone(), index_uri.clone())
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");
    let mut stats = auto_import_stats(&session);
    let mut node_modules_bucket = single_bucket(&stats.node_modules_buckets);
    assert_eq!(
        node_modules_bucket.file_count, 1,
        "expected only 1 file (main entrypoint) before deep import"
    );

    let new_content =
        "import { main } from \"my-pkg\";\nimport { deep } from \"my-pkg/nested/deep\";\n";
    session.did_change_file(
        ctx.clone(),
        index_uri.clone(),
        2,
        vec![whole_document_change(new_content)],
    );
    session
        .get_current_language_service_with_auto_imports(ctx, index_uri)
        .expect("GetCurrentLanguageServiceWithAutoImports should succeed");
    stats = auto_import_stats(&session);
    node_modules_bucket = single_bucket(&stats.node_modules_buckets);
    assert!(
        node_modules_bucket.file_count >= 4,
        "expected at least 4 files after deep import triggers recursive search, got {}",
        node_modules_bucket.file_count
    );
}

fn entrypoint_directory_search_files() -> (&'static str, HashMap<String, String>) {
    let project_root = "/home/src/entrypoint-search";
    let node_modules_dir = format!("{project_root}/node_modules");
    let pkg_dir = format!("{node_modules_dir}/my-pkg");
    (project_root, HashMap::from([
        (format!("{project_root}/tsconfig.json"), "{\n\t\"compilerOptions\": {\n\t\t\"module\": \"commonjs\",\n\t\t\"target\": \"es2020\"\n\t}\n}".to_string()),
        (format!("{project_root}/package.json"), "{\n\t\"name\": \"test-project\",\n\t\"dependencies\": { \"my-pkg\": \"*\" }\n}".to_string()),
        (format!("{project_root}/index.ts"), "import { main } from \"my-pkg\";".to_string()),
        (format!("{pkg_dir}/package.json"), "{\"name\":\"my-pkg\",\"version\":\"1.0.0\",\"types\":\"index.d.ts\"}".to_string()),
        (format!("{pkg_dir}/index.d.ts"), "export declare const main: number;\n".to_string()),
        (format!("{pkg_dir}/extra.d.ts"), "export declare const extra: string;\n".to_string()),
        (format!("{pkg_dir}/nested/deep.d.ts"), "export declare const deep: boolean;\n".to_string()),
        (format!("{pkg_dir}/nested/deeper.d.ts"), "export declare const deeper: boolean;\n".to_string()),
    ]))
}

fn auto_import_stats(session: &project::Session) -> CacheStats {
    session
        .current_auto_import_cache_stats()
        .expect("auto import registry")
}

fn read_file_or_panic(session: &project::Session, path: &str) -> String {
    let (text, ok) = session.fs().read_file(path);
    assert!(ok, "file should exist: {path}");
    text
}

fn single_bucket(buckets: &[BucketStats]) -> BucketStats {
    assert_eq!(buckets.len(), 1, "expected 1 bucket, got {}", buckets.len());
    buckets[0].clone()
}

fn setup(files: HashMap<String, String>) -> project::Session {
    setup_with_symlinks(files, &[])
}

fn setup_fixture(files: HashMap<String, Value>) -> project::Session {
    setup(
        files
            .into_iter()
            .map(|(path, value)| {
                let content = value
                    .as_str()
                    .unwrap_or_else(|| panic!("fixture file must be text: {path}"))
                    .to_string();
                (path, content)
            })
            .collect(),
    )
}

fn setup_with_symlinks(
    files: HashMap<String, String>,
    symlinks: &[(String, String)],
) -> project::Session {
    let fs = vfstest::from_map(files, false);
    for (link, target) in symlinks {
        fs.add_symlink(link, target);
    }
    project::new_session(project::SessionInit {
        background_ctx: core::Context::default(),
        options: project::SessionOptions {
            current_directory: "/".to_string(),
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
        fs: Arc::new(ts_bundled::wrap_fs(fs)),
        client: Some(Arc::new(NoopClient)),
        logger: Arc::new(project::new_test_logger()),
        npm_executor: None,
        parse_cache: None,
    })
}

fn whole_document_change(text: &str) -> project::TextDocumentContentChangePartialOrWholeDocument {
    project::TextDocumentContentChangePartialOrWholeDocument {
        partial: None,
        whole_document: Some(lsproto::TextDocumentContentChangeWholeDocument {
            text: text.to_string(),
        }),
    }
}

fn file_event(uri: &str, typ: lsproto::FileChangeType) -> lsproto::FileEvent {
    lsproto::FileEvent {
        uri: uri.to_string(),
        typ,
    }
}

fn to_path(file_name: &str) -> tspath::Path {
    tspath::to_path(file_name, "/", true)
}

fn set(items: &[&str]) -> collections::Set<String> {
    let mut result = collections::Set::default();
    for item in items {
        result.add((*item).to_string());
    }
    result
}
