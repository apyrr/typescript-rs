use std::collections::BTreeMap;
use std::sync::Arc;

use crate::autoimport::{get_package_realpath_funcs, word_indices};
use ts_vfs::vfstest;

#[test]
fn test_word_indices() {
    let tests = [
        // Basic camelCase
        ("camelCase", vec!["camelCase", "Case"]),
        // snake_case
        ("snake_case", vec!["snake_case", "case"]),
        // ParseURL - uppercase sequence followed by lowercase
        ("ParseURL", vec!["ParseURL", "URL"]),
        // XMLHttpRequest - multiple uppercase sequences
        (
            "XMLHttpRequest",
            vec!["XMLHttpRequest", "HttpRequest", "Request"],
        ),
        // Single word lowercase
        ("hello", vec!["hello"]),
        // Single word uppercase
        ("HELLO", vec!["HELLO"]),
        // Mixed with numbers
        (
            "parseHTML5Parser",
            vec!["parseHTML5Parser", "HTML5Parser", "Parser"],
        ),
        // Underscore variations
        ("__proto__", vec!["__proto__", "proto__"]),
        ("_private_member", vec!["_private_member", "member"]),
        // Single character
        ("a", vec!["a"]),
        ("A", vec!["A"]),
        // Consecutive underscores
        (
            "test__double__underscore",
            vec![
                "test__double__underscore",
                "double__underscore",
                "underscore",
            ],
        ),
    ];

    for (input, expected_words) in tests {
        let indices = word_indices(input);
        let actual_words = indices
            .into_iter()
            .map(|idx| input[idx..].to_string())
            .collect::<Vec<_>>();
        let expected_words = expected_words
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        assert_eq!(
            actual_words, expected_words,
            "word_indices({input:?}) produced words {actual_words:?}, want {expected_words:?}"
        );
    }
}

#[test]
fn test_get_package_realpath_funcs_follows_node_modules_symlinks() {
    // Simulate a layout where the package directory is itself a symlink (e.g. Bazel's
    // convenience symlinks or pnpm's virtual store):
    //   /symlink-bin/pkg/              -> symlink to /real/bin/pkg/
    //   /real/bin/pkg/node_modules/dep -> symlink to /real/dep/
    //
    // When toRealpath is used as the module resolver's Realpath, it must follow
    // the node_modules symlink so that /real/bin/pkg/node_modules/dep/index.d.ts
    // resolves to /real/dep/index.d.ts; otherwise the same dep file gets different
    // cache keys depending on which path it was reached through.
    let mut files = BTreeMap::new();
    files.insert(
        "/real/bin/pkg/index.d.ts".to_string(),
        "export declare const a: number;".to_string(),
    );
    files.insert(
        "/real/dep/index.d.ts".to_string(),
        "export declare const b: number;".to_string(),
    );
    files.insert(
        "/real/dep/src/utils/helper.d.ts".to_string(),
        "export declare const c: number;".to_string(),
    );
    let fs = vfstest::from_map(files, true);
    fs.add_symlink("/symlink-bin/pkg", "/real/bin/pkg");
    fs.add_symlink("/real/bin/pkg/node_modules/dep", "/real/dep");

    let (to_realpath, _) = get_package_realpath_funcs(Arc::new(fs.clone()), "/symlink-bin/pkg");

    // Files inside the package should be converted via string replacement (fast path).
    assert_eq!(
        to_realpath("/symlink-bin/pkg/index.d.ts"),
        "/real/bin/pkg/index.d.ts",
        "package files should be converted via prefix replacement"
    );

    // Files outside the package (e.g. node_modules symlinks) should be resolved via
    // fs.Realpath so the cache key is the canonical realpath, not the symlink path.
    assert_eq!(
        to_realpath("/real/bin/pkg/node_modules/dep/index.d.ts"),
        "/real/dep/index.d.ts",
        "node_modules symlinks must be followed so the same file gets a consistent cache key"
    );

    // Files in subdirectories of an already-resolved external package should
    // use the cached prefix mapping without additional realpath calls.
    assert_eq!(
        to_realpath("/real/bin/pkg/node_modules/dep/src/utils/helper.d.ts"),
        "/real/dep/src/utils/helper.d.ts",
        "subdirectories of a resolved external package should use cached prefix mapping"
    );
}

#[test]
fn test_get_package_realpath_funcs_duplicate_cache_keys() {
    // Simulate two packages (app-a, app-b) that each have a node_modules symlink to
    // the same shared dependency. This is a typical pnpm/Bazel layout:
    //   /workspace/packages/app-a/              -> symlink to /store/app-a/
    //   /workspace/packages/app-b/              -> symlink to /store/app-b/
    //   /store/app-a/node_modules/shared-lib    -> symlink to /store/shared-lib/
    //   /store/app-b/node_modules/shared-lib    -> symlink to /store/shared-lib/
    let mut files = BTreeMap::new();
    files.insert(
        "/store/app-a/index.d.ts".to_string(),
        "export declare const a: number;".to_string(),
    );
    files.insert(
        "/store/app-b/index.d.ts".to_string(),
        "export declare const b: number;".to_string(),
    );
    files.insert(
        "/store/shared-lib/index.d.ts".to_string(),
        "export declare const shared: string;".to_string(),
    );
    let fs = vfstest::from_map(files, true);
    fs.add_symlink("/workspace/packages/app-a", "/store/app-a");
    fs.add_symlink("/workspace/packages/app-b", "/store/app-b");
    fs.add_symlink("/store/app-a/node_modules/shared-lib", "/store/shared-lib");
    fs.add_symlink("/store/app-b/node_modules/shared-lib", "/store/shared-lib");

    let (to_realpath_a, _) =
        get_package_realpath_funcs(Arc::new(fs.clone()), "/workspace/packages/app-a");
    let (to_realpath_b, _) =
        get_package_realpath_funcs(Arc::new(fs.clone()), "/workspace/packages/app-b");

    let shared_file_via_a = "/store/app-a/node_modules/shared-lib/index.d.ts";
    let shared_file_via_b = "/store/app-b/node_modules/shared-lib/index.d.ts";

    let resolved_a = to_realpath_a(shared_file_via_a);
    let resolved_b = to_realpath_b(shared_file_via_b);

    // Both should resolve to the same canonical realpath so the module resolver
    // uses a single cache key for the shared dependency, avoiding duplicate loads.
    let expected_realpath = "/store/shared-lib/index.d.ts";
    assert_eq!(
        resolved_a, expected_realpath,
        "app-a's toRealpath should follow the node_modules symlink to the realpath"
    );
    assert_eq!(
        resolved_b, expected_realpath,
        "app-b's toRealpath should follow the node_modules symlink to the realpath"
    );
}

#[test]
fn test_get_package_realpath_funcs_non_symlinked_package_with_symlinked_deps() {
    let mut files = BTreeMap::new();
    files.insert(
        "/real/my-pkg/index.d.ts".to_string(),
        "export declare const a: number;".to_string(),
    );
    files.insert(
        "/real/dep/index.d.ts".to_string(),
        "export declare const b: number;".to_string(),
    );
    let fs = vfstest::from_map(files, true);
    fs.add_symlink("/real/my-pkg/node_modules/dep", "/real/dep");

    let (to_realpath, _) = get_package_realpath_funcs(Arc::new(fs.clone()), "/real/my-pkg");

    // Files inside the (non-symlinked) package should be returned unchanged.
    assert_eq!(
        to_realpath("/real/my-pkg/index.d.ts"),
        "/real/my-pkg/index.d.ts"
    );

    // Files outside the package reached via symlinked node_modules should still be resolved.
    assert_eq!(
        to_realpath("/real/my-pkg/node_modules/dep/index.d.ts"),
        "/real/dep/index.d.ts",
        "symlinked deps must be resolved even when the package dir itself is not a symlink"
    );
}
