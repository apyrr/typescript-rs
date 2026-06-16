use std::collections::BTreeMap;

use crate::vfs::Fs;
use crate::vfstest::from_map;

use super::{UNLIMITED_DEPTH, Usage, compile_glob_pattern, match_files};

fn case_insensitive_host() -> impl Fs {
    from_map(
        BTreeMap::from([
            ("/dev/a.ts".to_owned(), String::new()),
            ("/dev/b.ts".to_owned(), String::new()),
            ("/dev/z/a.ts".to_owned(), String::new()),
            ("/dev/z/abz.ts".to_owned(), String::new()),
            ("/dev/z/aba.ts".to_owned(), String::new()),
            ("/dev/x/a.ts".to_owned(), String::new()),
            ("/dev/x/b.ts".to_owned(), String::new()),
            ("/dev/x/y/a.ts".to_owned(), String::new()),
            ("/dev/x/y/b.ts".to_owned(), String::new()),
        ]),
        false,
    )
}

fn dotted_folders_host() -> impl Fs {
    from_map(
        BTreeMap::from([
            ("/dev/x/d.ts".to_owned(), String::new()),
            ("/dev/x/y/.e.ts".to_owned(), String::new()),
            ("/dev/x/.y/a.ts".to_owned(), String::new()),
        ]),
        false,
    )
}

fn common_folders_host() -> impl Fs {
    from_map(
        BTreeMap::from([
            ("/dev/a.ts".to_owned(), String::new()),
            ("/dev/node_modules/a.ts".to_owned(), String::new()),
            ("/dev/bower_components/a.ts".to_owned(), String::new()),
            ("/dev/jspm_packages/a.ts".to_owned(), String::new()),
        ]),
        false,
    )
}

fn large_file_system_host() -> impl Fs {
    let mut files = BTreeMap::new();
    for dir in [
        "/project/src",
        "/project/src/components",
        "/project/src/utils",
        "/project/src/services",
        "/project/src/models",
        "/project/src/hooks",
        "/project/test",
        "/project/node_modules/react",
        "/project/node_modules/typescript",
        "/project/node_modules/@types/node",
    ] {
        for index in 0..20 {
            let suffix = char::from(b'a' + index);
            files.insert(format!("{dir}/file{suffix}.ts"), String::new());
            files.insert(format!("{dir}/file{suffix}.test.ts"), String::new());
        }
    }
    files.insert("/project/src/.hidden/secret.ts".to_owned(), String::new());
    files.insert("/project/.config/settings.ts".to_owned(), String::new());
    from_map(files, false)
}

#[test]
fn benchmark_read_directory_scenarios_are_represented() {
    let cases: Vec<(&str, Box<dyn Fs>, &str, Vec<&str>, Vec<&str>, Vec<&str>)> = vec![
        (
            "LiteralIncludes",
            Box::new(case_insensitive_host()),
            "/dev",
            vec![".ts", ".tsx", ".d.ts"],
            vec![],
            vec!["a.ts", "b.ts"],
        ),
        (
            "WildcardIncludes",
            Box::new(case_insensitive_host()),
            "/dev",
            vec![".ts", ".tsx", ".d.ts"],
            vec![],
            vec!["z/*.ts", "x/*.ts"],
        ),
        (
            "RecursiveWildcard",
            Box::new(case_insensitive_host()),
            "/dev",
            vec![".ts", ".tsx", ".d.ts"],
            vec![],
            vec!["**/a.ts"],
        ),
        (
            "RecursiveWithExcludes",
            Box::new(case_insensitive_host()),
            "/dev",
            vec![".ts", ".tsx", ".d.ts"],
            vec!["**/b.ts"],
            vec!["**/*.ts"],
        ),
        (
            "ComplexPattern",
            Box::new(case_insensitive_host()),
            "/dev",
            vec![".ts", ".tsx", ".d.ts"],
            vec!["*.ts", "z/??z.ts", "*/b.ts"],
            vec!["a.ts", "b.ts", "z/a.ts", "z/abz.ts", "z/aba.ts", "x/b.ts"],
        ),
        (
            "DottedFolders",
            Box::new(dotted_folders_host()),
            "/dev",
            vec![".ts", ".tsx", ".d.ts"],
            vec![],
            vec!["**/.*/*"],
        ),
        (
            "CommonPackageFolders",
            Box::new(common_folders_host()),
            "/dev",
            vec![".ts", ".tsx", ".d.ts"],
            vec![],
            vec!["**/a.ts"],
        ),
        (
            "LargeFileSystem",
            Box::new(large_file_system_host()),
            "/project",
            vec![".ts", ".tsx", ".d.ts"],
            vec!["**/node_modules/**", "**/*.test.ts"],
            vec!["src/**/*.ts"],
        ),
    ];

    for (_name, host, path, extensions, excludes, includes) in cases {
        let extensions = extensions
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let excludes = excludes.into_iter().map(str::to_owned).collect::<Vec<_>>();
        let includes = includes.into_iter().map(str::to_owned).collect::<Vec<_>>();
        let _ = match_files(super::MatchFilesOptions {
            path,
            extensions: &extensions,
            excludes: &excludes,
            includes: &includes,
            use_case_sensitive_file_names: host.use_case_sensitive_file_names(),
            current_directory: "/",
            depth: UNLIMITED_DEPTH,
            host: host.as_ref(),
        });
    }
}

#[test]
fn benchmark_pattern_compilation_scenarios_are_represented() {
    for (_name, spec) in [
        ("Literal", "src/file.ts"),
        ("SingleWildcard", "src/*.ts"),
        ("QuestionMark", "src/?.ts"),
        ("DoubleAsterisk", "**/file.ts"),
        ("Complex", "src/**/components/*.tsx"),
        ("DottedPattern", "**/.*/*"),
    ] {
        let _ = compile_glob_pattern(spec, "/project", Usage::Files, true);
    }
}

#[test]
fn benchmark_pattern_matching_scenarios_are_represented() {
    for (_name, spec, paths) in [
        (
            "LiteralMatch",
            "src/file.ts",
            vec![
                "/project/src/file.ts",
                "/project/src/other.ts",
                "/project/lib/file.ts",
            ],
        ),
        (
            "WildcardMatch",
            "src/*.ts",
            vec![
                "/project/src/file.ts",
                "/project/src/component.ts",
                "/project/src/deep/file.ts",
                "/project/lib/file.ts",
            ],
        ),
        (
            "RecursiveMatch",
            "**/file.ts",
            vec![
                "/project/file.ts",
                "/project/src/file.ts",
                "/project/src/deep/nested/file.ts",
                "/project/src/other.ts",
            ],
        ),
        (
            "ComplexMatch",
            "src/**/components/*.tsx",
            vec![
                "/project/src/components/Button.tsx",
                "/project/src/features/auth/components/Login.tsx",
                "/project/src/components/Button.ts",
                "/project/lib/components/Button.tsx",
            ],
        ),
    ] {
        if let Some(pattern) = compile_glob_pattern(spec, "/project", Usage::Files, true) {
            for path in paths {
                let _ = pattern.matches(path);
            }
        }
    }
}
