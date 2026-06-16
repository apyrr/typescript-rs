use super::*;
use serde_json::Value;
use ts_core as core;
use ts_symlinks as symlinks;
use ts_tspath as tspath;

// Mock host for testing
#[derive(Clone, Debug)]
struct MockModuleSpecifierGenerationHost {
    current_dir: String,
    use_case_sensitive_file_names: bool,
    symlink_cache: Option<symlinks::KnownSymlinks>,
}

impl MockModuleSpecifierGenerationHost {
    fn new(current_dir: &str, use_case_sensitive_file_names: bool) -> Self {
        Self {
            current_dir: current_dir.to_string(),
            use_case_sensitive_file_names,
            symlink_cache: None,
        }
    }
}

impl ModuleSpecifierGenerationHost for MockModuleSpecifierGenerationHost {
    fn symlink_cache(&self) -> Option<symlinks::KnownSymlinks> {
        self.symlink_cache.clone()
    }

    fn common_source_directory(&self) -> String {
        self.current_dir.clone()
    }

    fn global_typings_cache_location(&self) -> String {
        String::new()
    }

    fn use_case_sensitive_file_names(&self) -> bool {
        self.use_case_sensitive_file_names
    }

    fn current_directory(&self) -> String {
        self.current_dir.clone()
    }

    fn project_reference_from_source(
        &self,
        _path: tspath::Path,
    ) -> Option<ts_tsoptions::SourceOutputAndProjectReference> {
        None
    }

    fn redirect_targets(&self, _path: tspath::Path) -> Vec<String> {
        Vec::new()
    }

    fn source_of_project_reference_if_output_included(
        &self,
        file: &dyn ts_ast::HasFileName,
    ) -> String {
        file.file_name()
    }

    fn file_exists(&self, _path: &str) -> bool {
        true // Mock implementation
    }

    fn nearest_ancestor_directory_with_package_json(&self, _dirname: &str) -> String {
        String::new()
    }

    fn package_json_info(&self, _pkg_json_path: &str) -> Option<ts_packagejson::InfoCacheEntry> {
        None
    }

    fn default_resolution_mode_for_file(
        &self,
        _file: &dyn ts_ast::HasFileName,
    ) -> core::ResolutionMode {
        core::RESOLUTION_MODE_NONE
    }

    fn resolved_module_from_module_specifier(
        &self,
        _file: &dyn ts_ast::HasFileName,
        _module_specifier: &ts_ast::StringLiteralLike,
    ) -> Option<ts_module::ResolvedModule> {
        None
    }

    fn mode_for_usage_location(
        &self,
        file: &dyn ts_ast::HasFileName,
        _module_specifier: &ts_ast::StringLiteralLike,
    ) -> core::ResolutionMode {
        self.default_resolution_mode_for_file(file)
    }
}

#[test]
fn test_get_each_file_name_of_module() {
    struct TestCase {
        name: &'static str,
        importing_file: &'static str,
        imported_file: &'static str,
        prefer_symlinks: bool,
        expected_count: usize,
        expected_paths: Option<&'static [&'static str]>,
    }

    let tests = [
        TestCase {
            name: "basic file path",
            importing_file: "/project/src/main.ts",
            imported_file: "/project/lib/utils.ts",
            prefer_symlinks: false,
            expected_count: 1,
            expected_paths: Some(&["/project/lib/utils.ts"]),
        },
        TestCase {
            name: "symlink preference false",
            importing_file: "/project/src/main.ts",
            imported_file: "/project/lib/utils.ts",
            prefer_symlinks: false,
            expected_count: 1,
            expected_paths: None,
        },
        TestCase {
            name: "symlink preference true",
            importing_file: "/project/src/main.ts",
            imported_file: "/project/lib/utils.ts",
            prefer_symlinks: true,
            expected_count: 1,
            expected_paths: None,
        },
        TestCase {
            name: "ignored path with no alternatives",
            importing_file: "/project/src/main.ts",
            imported_file: "/project/node_modules/.pnpm/file.ts",
            prefer_symlinks: false,
            expected_count: 1, // Should return 1 because there's no better option (all paths are ignored)
            expected_paths: None,
        },
    ];

    for test in tests {
        let host = MockModuleSpecifierGenerationHost {
            current_dir: "/project".to_string(),
            use_case_sensitive_file_names: true,
            symlink_cache: Some(symlinks::new_known_symlink("/project", true)),
        };

        let result = get_each_file_name_of_module(
            test.importing_file,
            test.imported_file,
            &host,
            test.prefer_symlinks,
        );

        assert_eq!(
            result.len(),
            test.expected_count,
            "{}: Expected {} paths, got {}",
            test.name,
            test.expected_count,
            result.len()
        );

        if let Some(expected_paths) = test.expected_paths {
            for (i, expected_path) in expected_paths.iter().enumerate() {
                assert!(
                    i < result.len(),
                    "{}: Expected path {}: {}, but result has only {} paths",
                    test.name,
                    i,
                    expected_path,
                    result.len()
                );
                assert_eq!(
                    result[i].file_name, *expected_path,
                    "{}: Expected path {} to be {}, got {}",
                    test.name, i, expected_path, result[i].file_name
                );
            }
        }

        for (i, path) in result.iter().enumerate() {
            assert!(
                !path.file_name.is_empty(),
                "{}: Path {} has empty FileName",
                test.name,
                i
            );
        }
    }
}

#[test]
fn test_get_each_file_name_of_module_with_symlinks() {
    let mut symlink_cache = symlinks::new_known_symlink("/project", true);
    let mut host = MockModuleSpecifierGenerationHost {
        current_dir: "/project".to_string(),
        use_case_sensitive_file_names: true,
        symlink_cache: Some(symlink_cache.clone()),
    };

    let symlink_path = tspath::ensure_trailing_directory_separator(&tspath::to_path(
        "/project/symlink",
        "/project",
        true,
    ));
    let real_directory = symlinks::KnownDirectoryLink {
        real: "/real/path/".to_string(),
        real_path: tspath::ensure_trailing_directory_separator(&tspath::to_path(
            "/real/path",
            "/project",
            true,
        )),
    };
    symlink_cache.set_directory(
        "/project/symlink".to_string(),
        symlink_path,
        Some(real_directory),
    );
    host.symlink_cache = Some(symlink_cache);

    let result =
        get_each_file_name_of_module("/project/src/main.ts", "/real/path/file.ts", &host, true);

    // Should find the symlink path
    let found = result
        .iter()
        .any(|path| path.file_name == "/project/symlink/file.ts");

    assert!(
        found,
        "Expected to find symlink path /project/symlink/file.ts"
    );
}

#[test]
fn test_contains_node_modules() {
    struct TestCase {
        name: &'static str,
        path: &'static str,
        expected: bool,
    }

    let tests = [
        TestCase {
            name: "contains node_modules",
            path: "/project/node_modules/lodash/index.js",
            expected: true,
        },
        TestCase {
            name: "does not contain node_modules",
            path: "/project/src/utils.ts",
            expected: false,
        },
        TestCase {
            name: "node_modules in middle",
            path: "/project/packages/node_modules/pkg/file.js",
            expected: true,
        },
        TestCase {
            name: "empty path",
            path: "",
            expected: false,
        },
    ];

    for test in tests {
        let result = contains_node_modules(test.path);
        assert_eq!(
            result, test.expected,
            "{}: ContainsNodeModules({:?}) = {}, expected {}",
            test.name, test.path, result, test.expected
        );
    }
}

#[test]
fn test_contains_ignored_path() {
    struct TestCase {
        name: &'static str,
        path: &'static str,
        expected: bool,
    }

    let tests = [
        TestCase {
            name: "ignored path",
            path: "/project/node_modules/.pnpm/file.ts",
            expected: true,
        },
        TestCase {
            name: "not ignored path",
            path: "/project/src/file.ts",
            expected: false,
        },
    ];

    for test in tests {
        let result = contains_ignored_path(test.path);
        assert_eq!(
            result, test.expected,
            "{}: containsIgnoredPath({:?}) = {}, expected {}",
            test.name, test.path, result, test.expected
        );
    }
}

#[test]
fn test_try_get_real_file_name_for_non_js_declaration_file_name() {
    struct TestCase {
        name: &'static str,
        file_name: &'static str,
        expected: &'static str,
    }

    let tests = [
        TestCase {
            name: "json declaration file",
            file_name: "/project/foo.d.json.ts",
            expected: "/project/foo.json",
        },
        TestCase {
            name: "multi-dot source extension declaration file",
            file_name: "/project/foo.module.d.css.ts",
            expected: "/project/foo.module.css",
        },
        TestCase {
            name: "plain dts file ignored",
            file_name: "/project/foo.d.ts",
            expected: "",
        },
    ];

    for test in tests {
        let got = try_get_real_file_name_for_non_js_declaration_file_name(test.file_name);
        assert_eq!(
            got, test.expected,
            "{}: TryGetRealFileNameForNonJSDeclarationFileName({:?}) = {:?}, expected {:?}",
            test.name, test.file_name, got, test.expected
        );
    }
}

#[test]
fn test_try_get_module_name_from_exports_or_imports() {
    struct TestCase {
        name: &'static str,
        target_file_path: &'static str,
        expected: &'static str,
    }

    let tests = [
        TestCase {
            name: "match",
            target_file_path: "/pkg/src/things/thing1/index.ts",
            expected: "./src/things/thing1",
        },
        TestCase {
            name: "mismatch with matching leading and trailing strings",
            target_file_path: "/pkg/src/things/index.ts",
            expected: "",
        },
    ];

    for test in tests {
        let result = try_get_module_name_from_exports_or_imports(
            &core::CompilerOptions::default(),
            &MockModuleSpecifierGenerationHost::new("", false),
            ExportsOrImportsModuleNameInput {
                target_file_path: test.target_file_path,
                package_directory: "/pkg",
                package_name: "./src/things/*",
                exports: &Value::String("./src/things/*/index.js".to_string()),
                conditions: &[],
                mode: MatchingMode::Pattern,
                is_imports: false,
                prefer_ts_extension: false,
            },
        );
        assert_eq!(
            result, test.expected,
            "{}: tryGetModuleNameFromExportsOrImports(targetFilePath = {:?}) = {:?}, expected {:?}",
            test.name, test.target_file_path, result, test.expected
        );
    }
}
