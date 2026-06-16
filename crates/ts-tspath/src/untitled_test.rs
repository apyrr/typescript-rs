use crate::path::{
    get_encoded_root_length, get_normalized_absolute_path, is_rooted_disk_path, to_path,
};

#[test]
fn test_untitled_path_handling() {
    // Test that untitled paths are treated as rooted
    let untitled_path = "^/untitled/ts-nul-authority/Untitled-2";

    // GetEncodedRootLength should return 2 for "^/"
    let root_length = get_encoded_root_length(untitled_path);
    assert_eq!(
        root_length, 2,
        "GetEncodedRootLength should return 2 for untitled paths"
    );

    // IsRootedDiskPath should return true
    let is_rooted = is_rooted_disk_path(untitled_path);
    assert!(
        is_rooted,
        "IsRootedDiskPath should return true for untitled paths"
    );

    // ToPath should not resolve untitled paths against current directory
    let current_dir = "/home/user/project";
    let path = to_path(untitled_path, current_dir, true);
    // The path should be the original untitled path
    assert_eq!(
        path, "^/untitled/ts-nul-authority/Untitled-2",
        "ToPath should not resolve untitled paths against current directory"
    );

    // Test GetNormalizedAbsolutePath doesn't resolve untitled paths
    let normalized = get_normalized_absolute_path(untitled_path, current_dir);
    assert_eq!(
        normalized, "^/untitled/ts-nul-authority/Untitled-2",
        "GetNormalizedAbsolutePath should not resolve untitled paths"
    );
}

#[test]
fn test_untitled_path_edge_cases() {
    struct TestCase {
        path: &'static str,
        expected: isize,
        is_rooted: bool,
    }

    let test_cases = [
        TestCase {
            path: "^/",
            expected: 2,
            is_rooted: true,
        },
        TestCase {
            path: "^/untitled/ts-nul-authority/test",
            expected: 2,
            is_rooted: true,
        },
        TestCase {
            path: "^",
            expected: 0,
            is_rooted: false,
        },
        TestCase {
            path: "^x",
            expected: 0,
            is_rooted: false,
        },
        TestCase {
            path: "^^/",
            expected: 0,
            is_rooted: false,
        },
        TestCase {
            path: "x^/",
            expected: 0,
            is_rooted: false,
        },
        TestCase {
            path: "^/untitled/ts-nul-authority/path/with/deeper/structure",
            expected: 2,
            is_rooted: true,
        },
    ];

    for tc in test_cases {
        let root_length = get_encoded_root_length(tc.path);
        assert_eq!(
            root_length, tc.expected,
            "GetEncodedRootLength for path {}",
            tc.path
        );

        let is_rooted = is_rooted_disk_path(tc.path);
        assert_eq!(
            is_rooted, tc.is_rooted,
            "IsRootedDiskPath for path {}",
            tc.path
        );
    }
}
