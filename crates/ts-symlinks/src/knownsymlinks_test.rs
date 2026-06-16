use super::*;
use ts_tspath as tspath;

#[test]
fn test_new_known_symlink() {
    let cache = new_known_symlink("/test/dir", true);
    assert_eq!(cache.cwd, "/test/dir");
    assert!(cache.use_case_sensitive_file_names);
}

#[test]
fn test_set_directory() {
    let mut cache = new_known_symlink("/test/dir", true);
    let symlink_path = tspath::ensure_trailing_directory_separator(&tspath::to_path(
        "/test/symlink",
        "/test/dir",
        true,
    ));
    let real_directory = KnownDirectoryLink {
        real: "/real/path/".to_string(),
        real_path: tspath::ensure_trailing_directory_separator(&tspath::to_path(
            "/real/path",
            "/test/dir",
            true,
        )),
    };

    cache.set_directory(
        "/test/symlink".to_string(),
        symlink_path.clone(),
        Some(real_directory.clone()),
    );

    let stored = cache
        .directories()
        .get(&symlink_path)
        .and_then(Option::as_ref)
        .expect("Expected directory to be stored");
    assert_eq!(stored.real, real_directory.real);
    assert_eq!(stored.real_path, real_directory.real_path);
    assert!(
        cache
            .directories_by_realpath()
            .get(&real_directory.real_path)
            .is_some_and(|set| set.contains("/test/symlink"))
    );
}

#[test]
fn test_set_file() {
    let mut cache = new_known_symlink("/test/dir", true);
    let symlink = "/test/symlink/file.ts";
    let symlink_path = tspath::to_path(symlink, "/test/dir", true);
    let realpath = "/real/path/file.ts";

    cache.set_file(
        symlink.to_string(),
        symlink_path.clone(),
        realpath.to_string(),
    );

    assert_eq!(
        cache.files().get(&symlink_path),
        Some(&realpath.to_string())
    );
}

#[test]
fn test_process_resolution() {
    let mut cache = new_known_symlink("/test/dir", true);

    cache.process_resolution(String::new(), String::new());
    cache.process_resolution("original".to_string(), String::new());
    cache.process_resolution(String::new(), "resolved".to_string());

    let original_path = "/test/original/file.ts";
    let resolved_path = "/test/resolved/file.ts";
    cache.process_resolution(original_path.to_string(), resolved_path.to_string());

    let symlink_path = tspath::to_path(original_path, "/test/dir", true);
    assert_eq!(
        cache.files().get(&symlink_path),
        Some(&resolved_path.to_string())
    );
}

#[test]
fn test_guess_directory_symlink() {
    let cache = new_known_symlink("/test/dir", true);
    let tests = [
        (
            "identical paths",
            "/test/path/file.ts",
            "/test/path/file.ts",
            ("/", "/"),
        ),
        (
            "different files same directory",
            "/test/path/file1.ts",
            "/test/path/file2.ts",
            ("", ""),
        ),
        (
            "different directories",
            "/test/path1/file.ts",
            "/test/path2/file.ts",
            ("/test/path1", "/test/path2"),
        ),
        (
            "node_modules paths",
            "/test/node_modules/pkg/file.ts",
            "/test/node_modules/pkg/file.ts",
            ("/test/node_modules/pkg", "/test/node_modules/pkg"),
        ),
        (
            "scoped package paths",
            "/test/node_modules/@scope/pkg/file.ts",
            "/test/node_modules/@scope/pkg/file.ts",
            (
                "/test/node_modules/@scope/pkg",
                "/test/node_modules/@scope/pkg",
            ),
        ),
    ];

    for (_name, a, b, expected) in tests {
        let actual = cache.guess_directory_symlink(a, b, "/test/dir");
        assert_eq!(actual, (expected.0.to_string(), expected.1.to_string()));
    }
}

#[test]
fn test_is_node_modules_or_scoped_package_directory() {
    let cache = new_known_symlink("/test/dir", true);
    let tests = [
        ("node_modules", true),
        ("@scope", true),
        ("src", false),
        ("", false),
        ("NODE_MODULES", false),
        ("@SCOPE", true),
    ];

    for (dir, expected) in tests {
        assert_eq!(
            cache.is_node_modules_or_scoped_package_directory(dir),
            expected
        );
    }
}

#[test]
fn test_set_symlinks_from_resolutions() {
    let mut cache = new_known_symlink("/test/dir", true);
    let resolved_modules = vec![
        ts_module::ResolvedModule {
            original_path: "/test/original/file1.ts".to_string(),
            resolved_file_name: "/test/resolved/file1.ts".to_string(),
            ..ts_module::ResolvedModule::default()
        },
        ts_module::ResolvedModule {
            original_path: "/test/original/file2.ts".to_string(),
            resolved_file_name: "/test/resolved/file2.ts".to_string(),
            ..ts_module::ResolvedModule::default()
        },
    ];

    cache.set_symlinks_from_resolutions(
        |callback, _file| {
            for resolution in &resolved_modules {
                callback(
                    resolution,
                    "",
                    ts_core::ResolutionMode::None,
                    tspath::to_path("/test/source.ts", "/test/dir", true),
                );
            }
        },
        |_callback, _file| {},
    );

    for resolution in &resolved_modules {
        let symlink_path = tspath::to_path(&resolution.original_path, "/test/dir", true);
        assert_eq!(
            cache.files().get(&symlink_path),
            Some(&resolution.resolved_file_name)
        );
    }
}

#[test]
fn test_known_symlinks_thread_safety() {
    let mut cache = new_known_symlink("/test/dir", true);

    for i in 0..10 {
        let suffix = i.to_string();
        let symlink_path = tspath::ensure_trailing_directory_separator(&tspath::to_path(
            &format!("/test/symlink{suffix}"),
            "/test/dir",
            true,
        ));
        let real_directory = KnownDirectoryLink {
            real: format!("/real/path{suffix}/"),
            real_path: tspath::ensure_trailing_directory_separator(&tspath::to_path(
                &format!("/real/path{suffix}"),
                "/test/dir",
                true,
            )),
        };

        cache.set_directory(
            format!("/test/symlink{suffix}"),
            symlink_path,
            Some(real_directory),
        );
    }

    assert_eq!(cache.directories().len(), 10);
}
