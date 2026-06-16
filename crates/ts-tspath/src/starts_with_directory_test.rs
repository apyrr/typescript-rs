use crate::path::starts_with_directory;

#[test]
fn test_starts_with_directory() {
    struct TestCase {
        name: &'static str,
        file_name: &'static str,
        directory_name: &'static str,
        use_case_sensitive_file_names: bool,
        expected: bool,
    }

    let tests = [
        TestCase {
            name: "exact match case sensitive",
            file_name: "/project/src/file.ts",
            directory_name: "/project/src",
            use_case_sensitive_file_names: true,
            expected: true,
        },
        TestCase {
            name: "exact match case insensitive",
            file_name: "/project/src/file.ts",
            directory_name: "/PROJECT/SRC",
            use_case_sensitive_file_names: false,
            expected: true,
        },
        TestCase {
            name: "case sensitive mismatch",
            file_name: "/project/src/file.ts",
            directory_name: "/PROJECT/SRC",
            use_case_sensitive_file_names: true,
            expected: false,
        },
        TestCase {
            name: "file not in directory",
            file_name: "/project/lib/file.ts",
            directory_name: "/project/src",
            use_case_sensitive_file_names: true,
            expected: false,
        },
        TestCase {
            name: "file in subdirectory",
            file_name: "/project/src/components/Button.tsx",
            directory_name: "/project/src",
            use_case_sensitive_file_names: true,
            expected: true,
        },
        TestCase {
            name: "file in parent directory",
            file_name: "/project/file.ts",
            directory_name: "/project/src",
            use_case_sensitive_file_names: true,
            expected: false,
        },
        TestCase {
            name: "windows style separators",
            file_name: "C:\\project\\src\\file.ts",
            directory_name: "C:\\project\\src",
            use_case_sensitive_file_names: true,
            expected: true,
        },
        TestCase {
            name: "mixed separators",
            file_name: "/project/src/file.ts",
            directory_name: "\\project\\src",
            use_case_sensitive_file_names: true,
            expected: false,
        },
        TestCase {
            name: "empty directory name",
            file_name: "/project/src/file.ts",
            directory_name: "",
            use_case_sensitive_file_names: true,
            expected: false,
        },
        TestCase {
            name: "empty file name",
            file_name: "",
            directory_name: "/project/src",
            use_case_sensitive_file_names: true,
            expected: false,
        },
        TestCase {
            name: "identical paths",
            file_name: "/project/src",
            directory_name: "/project/src",
            use_case_sensitive_file_names: true,
            expected: false,
        },
        TestCase {
            name: "directory with trailing separator",
            file_name: "/project/src/file.ts",
            directory_name: "/project/src/",
            use_case_sensitive_file_names: true,
            expected: true,
        },
        TestCase {
            name: "unicode characters",
            file_name: "/project/测试/file.ts",
            directory_name: "/project/测试",
            use_case_sensitive_file_names: true,
            expected: true,
        },
        TestCase {
            name: "unicode case insensitive",
            file_name: "/project/测试/file.ts",
            directory_name: "/PROJECT/测试",
            use_case_sensitive_file_names: false,
            expected: true,
        },
    ];

    for tt in tests {
        let result = starts_with_directory(
            tt.file_name,
            tt.directory_name,
            tt.use_case_sensitive_file_names,
        );
        assert_eq!(
            result, tt.expected,
            "StartsWithDirectory({:?}, {:?}, {}) for {}",
            tt.file_name, tt.directory_name, tt.use_case_sensitive_file_names, tt.name
        );
    }
}

#[test]
fn test_starts_with_directory_edge_cases() {
    struct TestCase {
        name: &'static str,
        file_name: &'static str,
        directory_name: &'static str,
        use_case_sensitive_file_names: bool,
        expected: bool,
    }

    let tests = [
        TestCase {
            name: "file name shorter than directory",
            file_name: "/proj",
            directory_name: "/project",
            use_case_sensitive_file_names: true,
            expected: false,
        },
        TestCase {
            name: "file name starts with directory but no separator",
            file_name: "/projectsrc/file.ts",
            directory_name: "/project",
            use_case_sensitive_file_names: true,
            expected: false,
        },
        TestCase {
            name: "relative paths",
            file_name: "src/file.ts",
            directory_name: "src",
            use_case_sensitive_file_names: true,
            expected: true,
        },
        TestCase {
            name: "absolute vs relative",
            file_name: "/project/src/file.ts",
            directory_name: "project/src",
            use_case_sensitive_file_names: true,
            expected: false,
        },
    ];

    for tt in tests {
        let result = starts_with_directory(
            tt.file_name,
            tt.directory_name,
            tt.use_case_sensitive_file_names,
        );
        assert_eq!(
            result, tt.expected,
            "StartsWithDirectory({:?}, {:?}, {}) for {}",
            tt.file_name, tt.directory_name, tt.use_case_sensitive_file_names, tt.name
        );
    }
}
