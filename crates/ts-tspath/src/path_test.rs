use std::cmp::Ordering;

use crate::path::*;

#[test]
fn normalize_slashes_should_match_go_cases() {
    assert_eq!(normalize_slashes("a"), "a");
    assert_eq!(normalize_slashes("a/b"), "a/b");
    assert_eq!(normalize_slashes("a\\b"), "a/b");
    assert_eq!(normalize_slashes("\\\\server\\path"), "//server/path");
}

#[test]
fn root_length_should_match_disk_unc_and_url_cases() {
    let cases = [
        ("a", 0),
        ("/", 1),
        ("/path", 1),
        ("c:", 2),
        ("c:d", 0),
        ("c:/", 3),
        ("c:\\", 3),
        ("//server", 8),
        ("//server/share", 9),
        ("\\\\server", 8),
        ("\\\\server\\share", 9),
        ("file:///", 8),
        ("file:///path", 8),
        ("file:///c:", 10),
        ("file:///c:d", 8),
        ("file:///c:/path", 11),
        ("file:///c%3a", 12),
        ("file:///c%3ad", 8),
        ("file:///c%3a/path", 13),
        ("file:///c%3A", 12),
        ("file:///c%3Ad", 8),
        ("file:///c%3A/path", 13),
        ("file://localhost", 16),
        ("file://localhost/", 17),
        ("file://localhost/path", 17),
        ("file://localhost/c:", 19),
        ("file://localhost/c:d", 17),
        ("file://localhost/c:/path", 20),
        ("file://server", 13),
        ("file://server/", 14),
        ("file://server/path", 14),
        ("http://server", 13),
        ("http://server/path", 14),
    ];

    for (path, expected) in cases {
        assert_eq!(get_root_length(path), expected, "path={path:?}");
    }
}

#[test]
fn rooted_and_url_classification_should_match_go_cases() {
    for path in [
        "file:///path",
        "file:///c:",
        "file://server/path",
        "http://server",
    ] {
        assert!(is_url(path), "path={path:?}");
        assert!(!is_rooted_disk_path(path), "path={path:?}");
    }

    for path in ["/", "c:", "c:/", "c:\\", "//server", "\\\\server\\share"] {
        assert!(!is_url(path), "path={path:?}");
        assert!(is_rooted_disk_path(path), "path={path:?}");
    }

    for path in ["a", "c:d", "path/to/file.ext", "./path/to/file.ext"] {
        assert!(!is_url(path), "path={path:?}");
        assert!(!is_rooted_disk_path(path), "path={path:?}");
    }
}

#[test]
fn get_directory_path_should_match_go_cases() {
    let cases = [
        ("", ""),
        ("a", ""),
        ("a/b", "a"),
        ("/", "/"),
        ("/a", "/"),
        ("/a/", "/"),
        ("/a/b", "/a"),
        ("c:", "c:"),
        ("c:d", ""),
        ("c:/path", "c:/"),
        ("//server/share", "//server/"),
        ("\\\\server\\share", "//server/"),
        ("file:///path", "file:///"),
        ("file:///c:", "file:///c:"),
        ("file:///c:d", "file:///"),
        ("file:///c:/path", "file:///c:/"),
        ("file://server/path", "file://server/"),
        ("http://server/path", "http://server/"),
    ];

    for (path, expected) in cases {
        assert_eq!(get_directory_path(path), expected, "path={path:?}");
    }
}

#[test]
fn components_and_reduction_should_match_go_cases() {
    assert_eq!(get_path_components("", ""), vec![""]);
    assert_eq!(get_path_components("a", ""), vec!["", "a"]);
    assert_eq!(get_path_components("./a", ""), vec!["", ".", "a"]);
    assert_eq!(get_path_components("/", ""), vec!["/"]);
    assert_eq!(get_path_components("/a", ""), vec!["/", "a"]);
    assert_eq!(get_path_components("c:/path", ""), vec!["c:/", "path"]);
    assert_eq!(
        get_path_components("//server/share", ""),
        vec!["//server/", "share"]
    );
    assert_eq!(
        get_path_components("file:///c:/path", ""),
        vec!["file:///c:/", "path"]
    );

    assert_eq!(reduce_path_components(&strings(&[""])), strings(&[""]));
    assert_eq!(reduce_path_components(&strings(&["", "."])), strings(&[""]));
    assert_eq!(
        reduce_path_components(&strings(&["", ".", "a"])),
        strings(&["", "a"])
    );
    assert_eq!(
        reduce_path_components(&strings(&["", "a", ".."])),
        strings(&[""])
    );
    assert_eq!(
        reduce_path_components(&strings(&["/", "a", ".."])),
        strings(&["/"])
    );
}

#[test]
fn combine_and_resolve_paths_should_match_go_cases() {
    assert_eq!(
        combine_paths("path", &["to", "file.ext"]),
        "path/to/file.ext"
    );
    assert_eq!(
        combine_paths("path", &["dir", "..", "to", "file.ext"]),
        "path/dir/../to/file.ext"
    );
    assert_eq!(combine_paths("/path", &["/to", "file.ext"]), "/to/file.ext");
    assert_eq!(
        combine_paths("c:/path", &["c:/to", "file.ext"]),
        "c:/to/file.ext"
    );
    assert_eq!(
        combine_paths("file:///path", &["file:///to", "file.ext"]),
        "file:///to/file.ext"
    );
    assert_eq!(combine_paths("/a/..", &["b/"]), "/a/../b/");

    assert_eq!(resolve_path("", &[]), "");
    assert_eq!(resolve_path(".", &[]), "");
    assert_eq!(resolve_path("..", &[]), "..");
    assert_eq!(resolve_path("/", &[]), "/");
    assert_eq!(resolve_path("/a/./b/", &[]), "/a/b/");
    assert_eq!(resolve_path("/a/../b", &[]), "/b");
    assert_eq!(resolve_path("/a/..", &["b"]), "/b");
    assert_eq!(resolve_path("a", &["b", "../c"]), "a/c");
}

#[test]
fn normalized_absolute_paths_should_match_go_cases() {
    let cases = [
        ("/", "", "/"),
        ("/.", "", "/"),
        ("/a/./b/", "", "/a/b"),
        ("/a/../b/", "", "/b"),
        ("\\a\\.\\b\\", "", "/a/b"),
        ("", "/home", "/home"),
        (".", "/home", "/home"),
        ("..", "/home", "/"),
        ("a", "b/c", "b/c/a"),
        (".a", "", ".a"),
        ("a.", "", "a."),
        ("a/..", "", ""),
        ("/a//", "", "/a"),
        ("a//b", "", "a/b"),
        ("a\\\\b", "", "a/b"),
        ("a\\/\\b", "", "a/b"),
    ];

    for (path, current_directory, expected) in cases {
        assert_eq!(
            get_normalized_absolute_path(path, current_directory),
            expected,
            "path={path:?}, current_directory={current_directory:?}"
        );
    }

    assert_eq!(
        get_normalized_absolute_path_without_root("/a/b/c.txt", "/a/b"),
        "a/b/c.txt"
    );
    assert_eq!(
        get_normalized_absolute_path_without_root("c:/work/hello.txt", "d:/workspaces"),
        "work/hello.txt"
    );
}

#[test]
fn relative_paths_and_to_path_should_match_go_cases() {
    let opts = ComparePathsOptions::default();
    assert_eq!(
        get_relative_path_to_directory_or_url("/", "/", false, &opts),
        ""
    );
    assert_eq!(
        get_relative_path_to_directory_or_url("/a", "/", false, &opts),
        ".."
    );
    assert_eq!(
        get_relative_path_to_directory_or_url("/a/b/c", "/b/c", false, &opts),
        "../../../b/c"
    );
    assert_eq!(
        get_relative_path_to_directory_or_url("file:///a/b", "file:///b", false, &opts),
        "../../b"
    );
    assert_eq!(
        get_relative_path_to_directory_or_url("file:///c:", "file:///d:", false, &opts),
        "file:///d:/"
    );

    assert_eq!(to_path("file.ext", "path/to", false), "path/to/file.ext");
    assert_eq!(to_path("file.ext", "/path/to", true), "/path/to/file.ext");
    assert_eq!(
        to_path("/path/to/../file.ext", "path/to", true),
        "/path/file.ext"
    );
}

#[test]
fn filename_casing_and_relative_detection_should_match_go_cases() {
    assert_eq!(
        to_file_name_lower_case("/user/UserName/projects/Project/file.ts"),
        "/user/username/projects/project/file.ts"
    );
    assert_eq!(
        to_file_name_lower_case("/user/UserName/projects/projectß/file.ts"),
        "/user/username/projects/projectß/file.ts"
    );
    assert_eq!(
        to_file_name_lower_case("/user/UserName/projects/İproject/file.ts"),
        "/user/username/projects/İproject/file.ts"
    );

    for path in [".", "..", "./", "../", "./foo/bar", "../foo/bar"] {
        assert!(path_is_relative(path), "path={path:?}");
    }
    for path in ["", "foo", "foo/bar", "/foo/bar", "c:/foo/bar"] {
        assert!(!path_is_relative(path), "path={path:?}");
    }
}

#[test]
fn common_parents_should_match_go_cases() {
    let opts = ComparePathsOptions::default();

    let paths: Vec<String> = Vec::new();
    let (got, ignored) = get_common_parents(&paths, 1, &opts);
    assert!(got.is_empty());
    assert!(ignored.is_empty());

    let paths = strings(&["/a/b/c/d"]);
    let (got, ignored) = get_common_parents(&paths, 1, &opts);
    assert_eq!(got, paths);
    assert!(ignored.is_empty());

    let paths = strings(&["/a/b/c/d", "/a/b/c/e", "/a/b/f/g"]);
    let (got, ignored) = get_common_parents(&paths, 1, &opts);
    assert_eq!(got, strings(&["/a/b"]));
    assert!(ignored.is_empty());

    let paths = strings(&["/a/b/c/d", "/a/b/c/e", "/a/b/f/g", "/x/y"]);
    let (got, ignored) = get_common_parents(&paths, 4, &opts);
    assert_eq!(got, strings(&["/a/b/c", "/a/b/f/g"]));
    assert!(ignored.contains("/x/y"));

    let paths = strings(&["c:/a/b/c/d", "d:/a/b/c/d"]);
    let (got, ignored) = get_common_parents(&paths, 1, &opts);
    assert_eq!(got, paths);
    assert!(ignored.is_empty());
}

#[test]
fn compare_paths_should_match_go_ordering_helpers() {
    assert_eq!(
        compare_paths_case_sensitive("/a/b", "/a/b", ""),
        Ordering::Equal
    );
    assert_ne!(
        compare_paths_case_sensitive("/A/b", "/a/b", ""),
        Ordering::Equal
    );
    assert_eq!(
        compare_paths_case_insensitive("/A/b", "/a/b", ""),
        Ordering::Equal
    );
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}
