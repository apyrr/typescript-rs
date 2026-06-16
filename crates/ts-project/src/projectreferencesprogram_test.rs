use std::collections::HashMap;
use std::time::SystemTime;

use crate as project;
use crate::projecttestutil;
use ts_core as core;
use ts_ls as lsconv;
use ts_lsproto as lsproto;
use ts_tspath as tspath;
use ts_vfs::vfstest;
use ts_vfs::vfstest::IntoMapFile;

const LANGUAGE_KIND_TYPESCRIPT: &str = "typescript";

type FileMap = HashMap<String, vfstest::MapFile>;

#[test]
fn test_project_references_program() {
    if !ts_bundled::EMBEDDED {
        return;
    }

    // program for referenced project
    {
        let files = files_for_referenced_project_program(false);
        let (mut session, _) = projecttestutil::setup(files.clone());
        let mut snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 0);

        let uri =
            lsproto::DocumentUri::from("file:///user/username/projects/myproject/main/main.ts");
        session.did_open_file(
            core::Context::default(),
            uri,
            1,
            file_text(&files, "/user/username/projects/myproject/main/main.ts"),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );

        snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        let projects = snapshot.project_collection.projects();
        let p = &projects[0];
        assert_eq!(p.kind, project::Kind::Configured);

        let file = p
            .program
            .as_ref()
            .unwrap()
            .get_source_file_by_path(tspath::Path::from(
                "/user/username/projects/myproject/dependency/fns.ts",
            ));
        assert!(file.is_some());
        let dts_file = p
            .program
            .as_ref()
            .unwrap()
            .get_source_file_by_path(tspath::Path::from(
                "/user/username/projects/myproject/decls/fns.d.ts",
            ));
        assert!(dts_file.is_none());
    }

    // program with disableSourceOfProjectReferenceRedirect
    {
        let mut files = files_for_referenced_project_program(true);
        files.insert(
            "/user/username/projects/myproject/decls/fns.d.ts".to_string(),
            map_file(
                r#"
            export declare function fn1(): void;
            export declare function fn2(): void;
            export declare function fn3(): void;
            export declare function fn4(): void;
            export declare function fn5(): void;
        "#,
            ),
        );
        let (mut session, _) = projecttestutil::setup(files.clone());
        let mut snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 0);

        let uri =
            lsproto::DocumentUri::from("file:///user/username/projects/myproject/main/main.ts");
        session.did_open_file(
            core::Context::default(),
            uri,
            1,
            file_text(&files, "/user/username/projects/myproject/main/main.ts"),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );

        snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        let projects = snapshot.project_collection.projects();
        let p = &projects[0];
        assert_eq!(p.kind, project::Kind::Configured);

        let file = p
            .program
            .as_ref()
            .unwrap()
            .get_source_file_by_path(tspath::Path::from(
                "/user/username/projects/myproject/dependency/fns.ts",
            ));
        assert!(file.is_none());
        let dts_file = p
            .program
            .as_ref()
            .unwrap()
            .get_source_file_by_path(tspath::Path::from(
                "/user/username/projects/myproject/decls/fns.d.ts",
            ));
        assert!(dts_file.is_some());
    }

    // references through symlink with index and typings
    {
        let (files, a_test, b_foo, b_bar) = files_for_symlink_references(false, "");
        check_symlink_references(files, &a_test, &b_foo, &b_bar);
    }

    // references through symlink with index and typings with preserveSymlinks
    {
        let (files, a_test, b_foo, b_bar) = files_for_symlink_references(true, "");
        check_symlink_references(files, &a_test, &b_foo, &b_bar);
    }

    // references through symlink with index and typings scoped package
    {
        let (files, a_test, b_foo, b_bar) = files_for_symlink_references(false, "@issue/");
        check_symlink_references(files, &a_test, &b_foo, &b_bar);
    }

    // references through symlink with index and typings with scoped package preserveSymlinks
    {
        let (files, a_test, b_foo, b_bar) = files_for_symlink_references(true, "@issue/");
        check_symlink_references(files, &a_test, &b_foo, &b_bar);
    }

    // references through symlink referencing from subFolder
    {
        let (files, a_test, b_foo, b_bar) = files_for_symlink_references_in_subfolder(false, "");
        check_symlink_references(files, &a_test, &b_foo, &b_bar);
    }

    // references through symlink referencing from subFolder with preserveSymlinks
    {
        let (files, a_test, b_foo, b_bar) = files_for_symlink_references_in_subfolder(true, "");
        check_symlink_references(files, &a_test, &b_foo, &b_bar);
    }

    // references through symlink referencing from subFolder scoped package
    {
        let (files, a_test, b_foo, b_bar) =
            files_for_symlink_references_in_subfolder(false, "@issue/");
        check_symlink_references(files, &a_test, &b_foo, &b_bar);
    }

    // references through symlink referencing from subFolder with scoped package preserveSymlinks
    {
        let (files, a_test, b_foo, b_bar) =
            files_for_symlink_references_in_subfolder(true, "@issue/");
        check_symlink_references(files, &a_test, &b_foo, &b_bar);
    }

    // when new file is added to referenced project
    {
        let mut files = files_for_referenced_project_program(false);
        let (mut session, utils) = projecttestutil::setup(files.clone());
        let uri =
            lsproto::DocumentUri::from("file:///user/username/projects/myproject/main/main.ts");
        session.did_open_file(
            core::Context::default(),
            uri.clone(),
            1,
            file_text(&files, "/user/username/projects/myproject/main/main.ts"),
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
        let mut snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        let program_before = snapshot.project_collection.projects()[0]
            .program
            .as_ref()
            .unwrap()
            .clone();

        files.insert(
            "/user/username/projects/myproject/dependency/fns2.ts".to_string(),
            map_file("export const x = 2;"),
        );
        utils
            .fs()
            .write_file(
                "/user/username/projects/myproject/dependency/fns2.ts",
                "export const x = 2;",
            )
            .unwrap();
        session.did_change_watched_files(
            core::Context::default(),
            vec![lsproto::FileEvent {
                typ: lsproto::FileChangeType::Created,
                uri: "file:///user/username/projects/myproject/dependency/fns2.ts".to_string(),
            }],
        );

        session
            .get_language_service(core::Context::default(), uri)
            .expect("GetLanguageService should succeed");
        snapshot = session.snapshot();
        assert_eq!(snapshot.project_collection.projects().len(), 1);
        assert!(!std::ptr::eq(
            snapshot.project_collection.projects()[0]
                .program
                .as_ref()
                .unwrap(),
            &program_before,
        ));
    }
}

fn check_symlink_references(files: FileMap, a_test: &str, b_foo: &str, b_bar: &str) {
    let (mut session, _) = projecttestutil::setup(files.clone());
    let mut snapshot = session.snapshot();
    assert_eq!(snapshot.project_collection.projects().len(), 0);

    let uri = lsconv::file_name_to_document_uri(a_test);
    session.did_open_file(
        core::Context::default(),
        uri,
        1,
        file_text(&files, a_test),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );

    snapshot = session.snapshot();
    assert_eq!(snapshot.project_collection.projects().len(), 1);
    let projects = snapshot.project_collection.projects();
    let p = &projects[0];
    assert_eq!(p.kind, project::Kind::Configured);

    let foo_file = p.program.as_ref().unwrap().get_source_file(b_foo);
    assert!(foo_file.is_some());
    let bar_file = p.program.as_ref().unwrap().get_source_file(b_bar);
    assert!(bar_file.is_some());
}

fn files_for_referenced_project_program(
    disable_source_of_project_reference_redirect: bool,
) -> FileMap {
    HashMap::from([
        (
            "/user/username/projects/myproject/main/tsconfig.json".to_string(),
            map_file(format!(
                r#"{{
            "compilerOptions": {{
                "composite": true{}
            }},
            "references": [{{ "path": "../dependency" }}]
        }}"#,
                if disable_source_of_project_reference_redirect {
                    r#", "disableSourceOfProjectReferenceRedirect": true"#
                } else {
                    ""
                }
            )),
        ),
        (
            "/user/username/projects/myproject/main/main.ts".to_string(),
            map_file(
                r#"
            import {
                fn1,
                fn2,
                fn3,
                fn4,
                fn5
            } from '../decls/fns'
            fn1();
            fn2();
            fn3();
            fn4();
            fn5();
        "#,
            ),
        ),
        (
            "/user/username/projects/myproject/dependency/tsconfig.json".to_string(),
            map_file(
                r#"{
            "compilerOptions": {
                "composite": true,
                "declarationDir": "../decls"
            },
        }"#,
            ),
        ),
        (
            "/user/username/projects/myproject/dependency/fns.ts".to_string(),
            map_file(
                r#"
            export function fn1() { }
            export function fn2() { }
            export function fn3() { }
            export function fn4() { }
            export function fn5() { }
        "#,
            ),
        ),
    ])
}

fn files_for_symlink_references(
    preserve_symlinks: bool,
    scope: &str,
) -> (FileMap, String, String, String) {
    let a_test = "/user/username/projects/myproject/packages/A/src/index.ts".to_string();
    let b_foo = "/user/username/projects/myproject/packages/B/src/index.ts".to_string();
    let b_bar = "/user/username/projects/myproject/packages/B/src/bar.ts".to_string();
    let mut files = HashMap::from([
        (
            "/user/username/projects/myproject/packages/B/package.json".to_string(),
            map_file(
                r#"{
            "main": "lib/index.js",
            "types": "lib/index.d.ts"
        }"#,
            ),
        ),
        (
            a_test.clone(),
            map_file(format!(
                r#"
            import {{ foo }} from '{}b';
            import {{ bar }} from '{}b/lib/bar';
            foo();
            bar();
        "#,
                scope, scope
            )),
        ),
        (b_foo.clone(), map_file("export function foo() { }")),
        (b_bar.clone(), map_file("export function bar() { }")),
        (
            format!("/user/username/projects/myproject/node_modules/{}b", scope),
            vfstest::symlink("/user/username/projects/myproject/packages/B"),
        ),
    ]);
    add_config_for_package(&mut files, "A", preserve_symlinks, &["../B"]);
    add_config_for_package(&mut files, "B", preserve_symlinks, &[]);
    (files, a_test, b_foo, b_bar)
}

fn files_for_symlink_references_in_subfolder(
    preserve_symlinks: bool,
    scope: &str,
) -> (FileMap, String, String, String) {
    let a_test = "/user/username/projects/myproject/packages/A/src/test.ts".to_string();
    let b_foo = "/user/username/projects/myproject/packages/B/src/foo.ts".to_string();
    let b_bar = "/user/username/projects/myproject/packages/B/src/bar/foo.ts".to_string();
    let mut files = HashMap::from([
        (
            "/user/username/projects/myproject/packages/B/package.json".to_string(),
            map_file("{}"),
        ),
        (
            "/user/username/projects/myproject/packages/A/src/test.ts".to_string(),
            map_file(format!(
                r#"
            import {{ foo }} from '{}b/lib/foo';
            import {{ bar }} from '{}b/lib/bar/foo';
            foo();
            bar();
        "#,
                scope, scope
            )),
        ),
        (b_foo.clone(), map_file("export function foo() { }")),
        (b_bar.clone(), map_file("export function bar() { }")),
        (
            format!("/user/username/projects/myproject/node_modules/{}b", scope),
            vfstest::symlink("/user/username/projects/myproject/packages/B"),
        ),
    ]);
    add_config_for_package(&mut files, "A", preserve_symlinks, &["../B"]);
    add_config_for_package(&mut files, "B", preserve_symlinks, &[]);
    (files, a_test, b_foo, b_bar)
}

fn add_config_for_package(
    files: &mut FileMap,
    package_name: &str,
    preserve_symlinks: bool,
    references: &[&str],
) {
    let mut compiler_options = serde_json::json!({
        "outDir": "lib",
        "rootDir": "src",
        "composite": true,
    });
    if preserve_symlinks {
        compiler_options["preserveSymlinks"] = serde_json::Value::Bool(true);
    }
    let references_to_add: Vec<_> = references
        .iter()
        .map(|r#ref| serde_json::json!({ "path": r#ref }))
        .collect();
    files.insert(
        format!(
            "/user/username/projects/myproject/packages/{}/tsconfig.json",
            package_name
        ),
        map_file(core::must(core::stringify_json(
            &serde_json::json!({
                "compilerOptions": compiler_options,
                "include": ["src"],
                "references": references_to_add,
            }),
            "    ",
            "  ",
        ))),
    );
}

fn map_file(text: impl Into<String>) -> vfstest::MapFile {
    text.into().into_map_file(SystemTime::UNIX_EPOCH)
}

fn file_text(files: &FileMap, path: &str) -> String {
    String::from_utf8(files[path].data.to_vec()).expect("test files should be valid UTF-8")
}
