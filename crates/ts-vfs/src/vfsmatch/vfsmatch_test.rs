use std::collections::BTreeMap;

use crate::vfs::Fs;
use crate::vfstest::from_map;

use super::{
    UNLIMITED_DEPTH, Usage, get_base_paths, is_implicit_glob, is_package_folder, match_files,
    new_spec_matcher, read_directory,
};

fn case_insensitive_host() -> impl Fs {
    from_map(
        BTreeMap::from([
            ("/dev/a.ts".to_owned(), String::new()),
            ("/dev/a.d.ts".to_owned(), String::new()),
            ("/dev/a.js".to_owned(), String::new()),
            ("/dev/b.ts".to_owned(), String::new()),
            ("/dev/b.js".to_owned(), String::new()),
            ("/dev/c.d.ts".to_owned(), String::new()),
            ("/dev/z/a.ts".to_owned(), String::new()),
            ("/dev/z/abz.ts".to_owned(), String::new()),
            ("/dev/z/aba.ts".to_owned(), String::new()),
            ("/dev/z/b.ts".to_owned(), String::new()),
            ("/dev/z/bbz.ts".to_owned(), String::new()),
            ("/dev/z/bba.ts".to_owned(), String::new()),
            ("/dev/x/a.ts".to_owned(), String::new()),
            ("/dev/x/aa.ts".to_owned(), String::new()),
            ("/dev/x/b.ts".to_owned(), String::new()),
            ("/dev/x/y/a.ts".to_owned(), String::new()),
            ("/dev/x/y/b.ts".to_owned(), String::new()),
            ("/dev/js/a.js".to_owned(), String::new()),
            ("/dev/js/b.js".to_owned(), String::new()),
            ("/dev/js/d.min.js".to_owned(), String::new()),
            ("/dev/js/ab.min.js".to_owned(), String::new()),
            ("/ext/ext.ts".to_owned(), String::new()),
            ("/ext/b/a..b.ts".to_owned(), String::new()),
        ]),
        false,
    )
}

fn case_sensitive_host() -> impl Fs {
    from_map(
        BTreeMap::from([
            ("/dev/a.ts".to_owned(), String::new()),
            ("/dev/A.ts".to_owned(), String::new()),
            ("/dev/B.ts".to_owned(), String::new()),
            ("/dev/b.ts".to_owned(), String::new()),
            ("/dev/x/a.ts".to_owned(), String::new()),
            ("/dev/x/y/a.ts".to_owned(), String::new()),
            ("/dev/q/a/c/b/d.ts".to_owned(), String::new()),
            ("/dev/js/d.MIN.js".to_owned(), String::new()),
        ]),
        true,
    )
}

fn common_folders_host() -> impl Fs {
    from_map(
        BTreeMap::from([
            ("/dev/a.ts".to_owned(), String::new()),
            ("/dev/b.ts".to_owned(), String::new()),
            ("/dev/x/a.ts".to_owned(), String::new()),
            ("/dev/node_modules/a.ts".to_owned(), String::new()),
            ("/dev/bower_components/a.ts".to_owned(), String::new()),
            ("/dev/jspm_packages/a.ts".to_owned(), String::new()),
        ]),
        false,
    )
}

fn dotted_folders_host() -> impl Fs {
    from_map(
        BTreeMap::from([
            ("/dev/x/d.ts".to_owned(), String::new()),
            ("/dev/x/y/d.ts".to_owned(), String::new()),
            ("/dev/x/y/.e.ts".to_owned(), String::new()),
            ("/dev/x/.y/a.ts".to_owned(), String::new()),
            ("/dev/.z/.b.ts".to_owned(), String::new()),
            ("/dev/.z/c.ts".to_owned(), String::new()),
            ("/dev/w/.u/e.ts".to_owned(), String::new()),
            ("/dev/g.min.js/.g/g.ts".to_owned(), String::new()),
        ]),
        false,
    )
}

fn mixed_extension_host() -> impl Fs {
    from_map(
        BTreeMap::from([
            ("/dev/a.ts".to_owned(), String::new()),
            ("/dev/a.d.ts".to_owned(), String::new()),
            ("/dev/a.js".to_owned(), String::new()),
            ("/dev/b.tsx".to_owned(), String::new()),
            ("/dev/b.d.ts".to_owned(), String::new()),
            ("/dev/b.jsx".to_owned(), String::new()),
            ("/dev/c.tsx".to_owned(), String::new()),
            ("/dev/c.js".to_owned(), String::new()),
            ("/dev/d.js".to_owned(), String::new()),
            ("/dev/e.jsx".to_owned(), String::new()),
            ("/dev/f.other".to_owned(), String::new()),
        ]),
        false,
    )
}

fn run_read_directory_case(
    host: &dyn Fs,
    path: &str,
    extensions: &[&str],
    excludes: &[&str],
    includes: &[&str],
    depth: i32,
) -> Vec<String> {
    match_files(super::MatchFilesOptions {
        path,
        extensions: &extensions
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>(),
        excludes: &excludes
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>(),
        includes: &includes
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>(),
        use_case_sensitive_file_names: host.use_case_sensitive_file_names(),
        current_directory: "/",
        depth,
        host,
    })
}

#[test]
fn test_is_implicit_glob() {
    assert!(is_implicit_glob("src"));
    assert!(!is_implicit_glob("*.ts"));
    assert!(!is_implicit_glob("a.ts"));
    assert!(!is_implicit_glob("a?"));
}

#[test]
fn test_get_base_paths() {
    assert_eq!(
        get_base_paths("/dev", &["x/*.ts".to_owned(), "z/a.ts".to_owned()], false),
        vec!["/dev"]
    );
}

#[test]
fn test_read_directory_defaults_include_common_package_folders() {
    let host = common_folders_host();
    let got = run_read_directory_case(
        &host,
        "/dev",
        &[".ts", ".tsx", ".d.ts"],
        &[],
        &[],
        UNLIMITED_DEPTH,
    );
    assert!(got.contains(&"/dev/a.ts".to_owned()));
    assert!(got.contains(&"/dev/b.ts".to_owned()));
    assert!(got.contains(&"/dev/x/a.ts".to_owned()));
    assert!(got.contains(&"/dev/node_modules/a.ts".to_owned()));
    assert!(got.contains(&"/dev/bower_components/a.ts".to_owned()));
    assert!(got.contains(&"/dev/jspm_packages/a.ts".to_owned()));
}

#[test]
fn test_read_directory_literal_includes_without_exclusions() {
    let host = case_insensitive_host();
    let got = run_read_directory_case(
        &host,
        "/dev",
        &[".ts", ".tsx", ".d.ts"],
        &[],
        &["a.ts", "b.ts"],
        UNLIMITED_DEPTH,
    );
    assert_eq!(got, vec!["/dev/a.ts", "/dev/b.ts"]);
}

#[test]
fn test_read_directory_literal_includes_with_non_ts_extensions_excluded() {
    let host = case_insensitive_host();
    let got = run_read_directory_case(
        &host,
        "/dev",
        &[".ts", ".tsx", ".d.ts"],
        &[],
        &["a.js", "b.js"],
        UNLIMITED_DEPTH,
    );
    assert!(got.is_empty());
}

#[test]
fn test_read_directory_literal_includes_with_wildcard_excludes() {
    let host = case_insensitive_host();
    let got = run_read_directory_case(
        &host,
        "/dev",
        &[".ts", ".tsx", ".d.ts"],
        &["*.ts", "z/??z.ts", "*/b.ts"],
        &["a.ts", "b.ts", "z/a.ts", "z/abz.ts", "z/aba.ts", "x/b.ts"],
        UNLIMITED_DEPTH,
    );
    assert_eq!(got, vec!["/dev/z/a.ts", "/dev/z/aba.ts"]);
}

#[test]
fn test_read_directory_literal_includes_with_recursive_excludes() {
    let host = case_insensitive_host();
    let got = run_read_directory_case(
        &host,
        "/dev",
        &[".ts", ".tsx", ".d.ts"],
        &["**/b.ts"],
        &["a.ts", "b.ts", "x/a.ts", "x/b.ts", "x/y/a.ts", "x/y/b.ts"],
        UNLIMITED_DEPTH,
    );
    assert_eq!(got, vec!["/dev/a.ts", "/dev/x/a.ts", "/dev/x/y/a.ts"]);
}

#[test]
fn test_read_directory_case_sensitive_exclude_is_respected() {
    let host = case_sensitive_host();
    let got = run_read_directory_case(
        &host,
        "/dev",
        &[".ts", ".tsx", ".d.ts"],
        &["**/b.ts"],
        &["B.ts"],
        UNLIMITED_DEPTH,
    );
    assert_eq!(got, vec!["/dev/B.ts"]);
}

#[test]
fn test_read_directory_wildcard_include_sorted_order() {
    let host = case_insensitive_host();
    let got = run_read_directory_case(
        &host,
        "/dev",
        &[".ts", ".tsx", ".d.ts"],
        &[],
        &["z/*.ts", "x/*.ts"],
        UNLIMITED_DEPTH,
    );
    assert_eq!(
        got,
        vec![
            "/dev/z/a.ts",
            "/dev/z/aba.ts",
            "/dev/z/abz.ts",
            "/dev/z/b.ts",
            "/dev/z/bba.ts",
            "/dev/z/bbz.ts",
            "/dev/x/a.ts",
            "/dev/x/aa.ts",
            "/dev/x/b.ts"
        ]
    );
}

#[test]
fn test_read_directory_wildcard_star_matches_only_ts_files() {
    let host = case_insensitive_host();
    let got = run_read_directory_case(
        &host,
        "/dev",
        &[".ts", ".tsx", ".d.ts"],
        &[],
        &["*"],
        UNLIMITED_DEPTH,
    );
    assert!(
        got.iter()
            .all(|file| file.ends_with(".ts") || file.ends_with(".tsx"))
    );
    assert!(!got.contains(&"/dev/a.js".to_owned()));
    assert!(!got.contains(&"/dev/b.js".to_owned()));
}

#[test]
fn test_read_directory_wildcard_question_mark_single_character() {
    let host = case_insensitive_host();
    let got = run_read_directory_case(
        &host,
        "/dev",
        &[".ts", ".tsx", ".d.ts"],
        &[],
        &["x/?.ts"],
        UNLIMITED_DEPTH,
    );
    assert_eq!(got, vec!["/dev/x/a.ts", "/dev/x/b.ts"]);
}

#[test]
fn test_read_directory_wildcard_recursive_directory() {
    let host = case_insensitive_host();
    let got = run_read_directory_case(
        &host,
        "/dev",
        &[".ts", ".tsx", ".d.ts"],
        &[],
        &["**/a.ts"],
        UNLIMITED_DEPTH,
    );
    assert!(got.contains(&"/dev/a.ts".to_owned()));
    assert!(got.contains(&"/dev/z/a.ts".to_owned()));
    assert!(got.contains(&"/dev/x/a.ts".to_owned()));
    assert!(got.contains(&"/dev/x/y/a.ts".to_owned()));
}

#[test]
fn test_read_directory_depth_limit() {
    let host = case_sensitive_host();
    let got = run_read_directory_case(&host, "/dev", &[".ts"], &[], &["**/*.ts"], 2);
    assert!(got.contains(&"/dev/a.ts".to_owned()));
    assert!(got.contains(&"/dev/x/a.ts".to_owned()));
    assert!(!got.contains(&"/dev/x/y/a.ts".to_owned()));
    assert!(!got.contains(&"/dev/q/a/c/b/d.ts".to_owned()));
}

#[test]
fn test_read_directory_dot_folders_are_skipped_by_wildcards() {
    let host = dotted_folders_host();
    let got = run_read_directory_case(&host, "/dev", &[".ts"], &[], &["**/*.ts"], UNLIMITED_DEPTH);
    assert!(got.contains(&"/dev/x/d.ts".to_owned()));
    assert!(got.contains(&"/dev/x/y/d.ts".to_owned()));
    assert!(!got.contains(&"/dev/x/y/.e.ts".to_owned()));
    assert!(!got.contains(&"/dev/x/.y/a.ts".to_owned()));
}

#[test]
fn test_read_directory_mixed_extensions() {
    let host = mixed_extension_host();
    let got = run_read_directory_case(
        &host,
        "/dev",
        &[".ts", ".tsx", ".d.ts", ".js", ".jsx"],
        &[],
        &["*"],
        UNLIMITED_DEPTH,
    );
    assert!(got.contains(&"/dev/a.ts".to_owned()));
    assert!(got.contains(&"/dev/b.tsx".to_owned()));
    assert!(got.contains(&"/dev/d.js".to_owned()));
    assert!(got.contains(&"/dev/e.jsx".to_owned()));
    assert!(!got.contains(&"/dev/f.other".to_owned()));
}

#[test]
fn test_read_directory_min_js_handling() {
    let host = case_insensitive_host();
    let js = run_read_directory_case(&host, "/dev/js", &[".js"], &[], &["*.js"], UNLIMITED_DEPTH);
    assert!(js.contains(&"/dev/js/a.js".to_owned()));
    assert!(js.contains(&"/dev/js/b.js".to_owned()));
    assert!(!js.contains(&"/dev/js/d.min.js".to_owned()));
    let min_js = run_read_directory_case(
        &host,
        "/dev/js",
        &[".js"],
        &[],
        &["*.min.js"],
        UNLIMITED_DEPTH,
    );
    assert!(min_js.contains(&"/dev/js/d.min.js".to_owned()));
}

#[test]
fn test_spec_matcher() {
    let matcher = new_spec_matcher(&["**/*.ts".to_owned()], "/dev", Usage::Files, false).unwrap();
    assert!(matcher.match_string("/dev/a.ts"));
    assert!(matcher.match_string("/dev/x/y/a.ts"));
    assert!(!matcher.match_string("/dev/a.js"));
    assert_eq!(matcher.match_index("/dev/a.ts"), Some(0));
}

#[test]
fn test_read_directory_public_wrapper() {
    let host = case_insensitive_host();
    let got = read_directory(
        &host,
        "/",
        "/dev",
        &[".ts".to_owned()],
        &[],
        &["a.ts".to_owned()],
        UNLIMITED_DEPTH,
    );
    assert_eq!(got, vec!["/dev/a.ts"]);
}

#[test]
fn test_is_package_folder() {
    assert!(is_package_folder("node_modules"));
    assert!(is_package_folder("BOWER_COMPONENTS"));
    assert!(is_package_folder("jspm_packages"));
    assert!(!is_package_folder("src"));
}
