use ts_bundled as bundled;
use ts_compiler as compiler;
use ts_core as core;
use ts_repo as repo;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;
use ts_vfs_os as osvfs;
use ts_vfs_test as vfstest;

struct TestFile {
    file_name: String,
    contents: String,
}

struct ProgramTest {
    test_name: String,
    files: Vec<TestFile>,
    expected_files: Vec<String>,
    target: core::ScriptTarget,
}

struct CompilerHostParseConfigHost<'a>(&'a dyn compiler::CompilerHost);

impl tsoptions::ParseConfigHost for CompilerHostParseConfigHost<'_> {
    fn fs(&self) -> &dyn compiler::vfs::Fs {
        self.0.fs()
    }

    fn get_current_directory(&self) -> String {
        self.0.get_current_directory()
    }
}

fn parsed_command_line(
    file_names: Vec<String>,
    compiler_options: core::CompilerOptions,
) -> Box<tsoptions::ParsedCommandLine> {
    let mut config = tsoptions::ParsedCommandLine {
        file_names,
        ..Default::default()
    };
    config.set_compiler_options(compiler_options);
    Box::new(config)
}

fn program_options(
    config: Box<tsoptions::ParsedCommandLine>,
    host: Box<dyn compiler::CompilerHost>,
) -> compiler::ProgramOptions {
    compiler::ProgramOptions {
        config,
        host,
        use_source_of_project_reference: false,
        single_threaded: Default::default(),
        create_checker_pool: None,
        typings_location: String::new(),
        project_name: String::new(),
        type_script_version: String::new(),
        tracing: None,
    }
}

fn esnext_libs() -> Vec<String> {
    vec![
        "lib.es5.d.ts",
        "lib.es2015.d.ts",
        "lib.es2016.d.ts",
        "lib.es2017.d.ts",
        "lib.es2018.d.ts",
        "lib.es2019.d.ts",
        "lib.es2020.d.ts",
        "lib.es2021.d.ts",
        "lib.es2022.d.ts",
        "lib.es2023.d.ts",
        "lib.es2024.d.ts",
        "lib.es2025.d.ts",
        "lib.esnext.d.ts",
        "lib.dom.d.ts",
        "lib.dom.iterable.d.ts",
        "lib.dom.asynciterable.d.ts",
        "lib.webworker.importscripts.d.ts",
        "lib.scripthost.d.ts",
        "lib.es2015.core.d.ts",
        "lib.es2015.collection.d.ts",
        "lib.es2015.generator.d.ts",
        "lib.es2015.iterable.d.ts",
        "lib.es2015.promise.d.ts",
        "lib.es2015.proxy.d.ts",
        "lib.es2015.reflect.d.ts",
        "lib.es2015.symbol.d.ts",
        "lib.es2015.symbol.wellknown.d.ts",
        "lib.es2016.array.include.d.ts",
        "lib.es2016.intl.d.ts",
        "lib.es2017.arraybuffer.d.ts",
        "lib.es2017.date.d.ts",
        "lib.es2017.object.d.ts",
        "lib.es2017.sharedmemory.d.ts",
        "lib.es2017.string.d.ts",
        "lib.es2017.intl.d.ts",
        "lib.es2017.typedarrays.d.ts",
        "lib.es2018.asyncgenerator.d.ts",
        "lib.es2018.asynciterable.d.ts",
        "lib.es2018.intl.d.ts",
        "lib.es2018.promise.d.ts",
        "lib.es2018.regexp.d.ts",
        "lib.es2019.array.d.ts",
        "lib.es2019.object.d.ts",
        "lib.es2019.string.d.ts",
        "lib.es2019.symbol.d.ts",
        "lib.es2019.intl.d.ts",
        "lib.es2020.bigint.d.ts",
        "lib.es2020.date.d.ts",
        "lib.es2020.promise.d.ts",
        "lib.es2020.sharedmemory.d.ts",
        "lib.es2020.string.d.ts",
        "lib.es2020.symbol.wellknown.d.ts",
        "lib.es2020.intl.d.ts",
        "lib.es2020.number.d.ts",
        "lib.es2021.promise.d.ts",
        "lib.es2021.string.d.ts",
        "lib.es2021.weakref.d.ts",
        "lib.es2021.intl.d.ts",
        "lib.es2022.array.d.ts",
        "lib.es2022.error.d.ts",
        "lib.es2022.intl.d.ts",
        "lib.es2022.object.d.ts",
        "lib.es2022.string.d.ts",
        "lib.es2022.regexp.d.ts",
        "lib.es2023.array.d.ts",
        "lib.es2023.collection.d.ts",
        "lib.es2023.intl.d.ts",
        "lib.es2024.arraybuffer.d.ts",
        "lib.es2024.collection.d.ts",
        "lib.es2024.object.d.ts",
        "lib.es2024.promise.d.ts",
        "lib.es2024.regexp.d.ts",
        "lib.es2024.sharedmemory.d.ts",
        "lib.es2024.string.d.ts",
        "lib.es2025.collection.d.ts",
        "lib.es2025.float16.d.ts",
        "lib.es2025.intl.d.ts",
        "lib.es2025.iterator.d.ts",
        "lib.es2025.promise.d.ts",
        "lib.es2025.regexp.d.ts",
        "lib.esnext.array.d.ts",
        "lib.esnext.collection.d.ts",
        "lib.esnext.date.d.ts",
        "lib.esnext.decorators.d.ts",
        "lib.esnext.disposable.d.ts",
        "lib.esnext.error.d.ts",
        "lib.esnext.intl.d.ts",
        "lib.esnext.sharedmemory.d.ts",
        "lib.esnext.temporal.d.ts",
        "lib.esnext.typedarrays.d.ts",
        "lib.decorators.d.ts",
        "lib.decorators.legacy.d.ts",
        "lib.esnext.full.d.ts",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn program_test_cases() -> Vec<ProgramTest> {
    let mut cases = Vec::new();
    cases.push(ProgramTest {
        test_name: "BasicFileOrdering".to_string(),
        files: vec![
            TestFile { file_name: "c:/dev/src/index.ts".to_string(), contents: "/// <reference path='c:/dev/src2/a/5.ts' />\n/// <reference path='c:/dev/src2/a/10.ts' />".to_string() },
            TestFile { file_name: "c:/dev/src2/a/5.ts".to_string(), contents: "/// <reference path='4.ts' />".to_string() },
            TestFile { file_name: "c:/dev/src2/a/4.ts".to_string(), contents: "/// <reference path='b/3.ts' />".to_string() },
            TestFile { file_name: "c:/dev/src2/a/b/3.ts".to_string(), contents: "/// <reference path='2.ts' />".to_string() },
            TestFile { file_name: "c:/dev/src2/a/b/2.ts".to_string(), contents: "/// <reference path='c/1.ts' />".to_string() },
            TestFile { file_name: "c:/dev/src2/a/b/c/1.ts".to_string(), contents: "console.log('hello');".to_string() },
            TestFile { file_name: "c:/dev/src2/a/10.ts".to_string(), contents: "/// <reference path='b/c/d/9.ts' />".to_string() },
            TestFile { file_name: "c:/dev/src2/a/b/c/d/9.ts".to_string(), contents: "/// <reference path='e/8.ts' />".to_string() },
            TestFile { file_name: "c:/dev/src2/a/b/c/d/e/8.ts".to_string(), contents: "/// <reference path='7.ts' />".to_string() },
            TestFile { file_name: "c:/dev/src2/a/b/c/d/e/7.ts".to_string(), contents: "/// <reference path='f/6.ts' />".to_string() },
            TestFile { file_name: "c:/dev/src2/a/b/c/d/e/f/6.ts".to_string(), contents: "console.log('world!');".to_string() },
        ],
        expected_files: {
            let mut files = esnext_libs();
            files.extend([
                "c:/dev/src2/a/b/c/1.ts",
                "c:/dev/src2/a/b/2.ts",
                "c:/dev/src2/a/b/3.ts",
                "c:/dev/src2/a/4.ts",
                "c:/dev/src2/a/5.ts",
                "c:/dev/src2/a/b/c/d/e/f/6.ts",
                "c:/dev/src2/a/b/c/d/e/7.ts",
                "c:/dev/src2/a/b/c/d/e/8.ts",
                "c:/dev/src2/a/b/c/d/9.ts",
                "c:/dev/src2/a/10.ts",
                "c:/dev/src/index.ts",
            ].into_iter().map(str::to_string));
            files
        },
        target: core::ScriptTarget::EsNext,
    });
    cases.push(ProgramTest {
        test_name: "FileOrderingImports".to_string(),
        files: vec![
            TestFile { file_name: "c:/dev/src/index.ts".to_string(), contents: "import * as five from '../src2/a/5.ts';\nimport * as ten from '../src2/a/10.ts';".to_string() },
            TestFile { file_name: "c:/dev/src2/a/5.ts".to_string(), contents: "import * as four from './4.ts';".to_string() },
            TestFile { file_name: "c:/dev/src2/a/4.ts".to_string(), contents: "import * as three from './b/3.ts';".to_string() },
            TestFile { file_name: "c:/dev/src2/a/b/3.ts".to_string(), contents: "import * as two from './2.ts';".to_string() },
            TestFile { file_name: "c:/dev/src2/a/b/2.ts".to_string(), contents: "import * as one from './c/1.ts';".to_string() },
            TestFile { file_name: "c:/dev/src2/a/b/c/1.ts".to_string(), contents: "console.log('hello');".to_string() },
            TestFile { file_name: "c:/dev/src2/a/10.ts".to_string(), contents: "import * as nine from './b/c/d/9.ts';".to_string() },
            TestFile { file_name: "c:/dev/src2/a/b/c/d/9.ts".to_string(), contents: "import * as eight from './e/8.ts';".to_string() },
            TestFile { file_name: "c:/dev/src2/a/b/c/d/e/8.ts".to_string(), contents: "import * as seven from './7.ts';".to_string() },
            TestFile { file_name: "c:/dev/src2/a/b/c/d/e/7.ts".to_string(), contents: "import * as six from './f/6.ts';".to_string() },
            TestFile { file_name: "c:/dev/src2/a/b/c/d/e/f/6.ts".to_string(), contents: "console.log('world!');".to_string() },
        ],
        expected_files: {
            let mut files = esnext_libs();
            files.extend([
                "c:/dev/src2/a/b/c/1.ts",
                "c:/dev/src2/a/b/2.ts",
                "c:/dev/src2/a/b/3.ts",
                "c:/dev/src2/a/4.ts",
                "c:/dev/src2/a/5.ts",
                "c:/dev/src2/a/b/c/d/e/f/6.ts",
                "c:/dev/src2/a/b/c/d/e/7.ts",
                "c:/dev/src2/a/b/c/d/e/8.ts",
                "c:/dev/src2/a/b/c/d/9.ts",
                "c:/dev/src2/a/10.ts",
                "c:/dev/src/index.ts",
            ].into_iter().map(str::to_string));
            files
        },
        target: core::ScriptTarget::EsNext,
    });
    cases.push(ProgramTest {
        test_name: "FileOrderingCycles".to_string(),
        files: vec![
            TestFile { file_name: "c:/dev/src/index.ts".to_string(), contents: "import * as five from '../src2/a/5.ts';\nimport * as ten from '../src2/a/10.ts';".to_string() },
            TestFile { file_name: "c:/dev/src2/a/5.ts".to_string(), contents: "import * as four from './4.ts';".to_string() },
            TestFile { file_name: "c:/dev/src2/a/4.ts".to_string(), contents: "import * as three from './b/3.ts';".to_string() },
            TestFile { file_name: "c:/dev/src2/a/b/3.ts".to_string(), contents: "import * as two from './2.ts';\nimport * as cycle from 'c:/dev/src/index.ts'; ".to_string() },
            TestFile { file_name: "c:/dev/src2/a/b/2.ts".to_string(), contents: "import * as one from './c/1.ts';".to_string() },
            TestFile { file_name: "c:/dev/src2/a/b/c/1.ts".to_string(), contents: "console.log('hello');".to_string() },
            TestFile { file_name: "c:/dev/src2/a/10.ts".to_string(), contents: "import * as nine from './b/c/d/9.ts';".to_string() },
            TestFile { file_name: "c:/dev/src2/a/b/c/d/9.ts".to_string(), contents: "import * as eight from './e/8.ts';\nimport * as cycle from 'c:/dev/src/index.ts';".to_string() },
            TestFile { file_name: "c:/dev/src2/a/b/c/d/e/8.ts".to_string(), contents: "import * as seven from './7.ts';".to_string() },
            TestFile { file_name: "c:/dev/src2/a/b/c/d/e/7.ts".to_string(), contents: "import * as six from './f/6.ts';".to_string() },
            TestFile { file_name: "c:/dev/src2/a/b/c/d/e/f/6.ts".to_string(), contents: "console.log('world!');".to_string() },
        ],
        expected_files: {
            let mut files = esnext_libs();
            files.extend([
                "c:/dev/src2/a/b/c/1.ts",
                "c:/dev/src2/a/b/2.ts",
                "c:/dev/src2/a/b/3.ts",
                "c:/dev/src2/a/4.ts",
                "c:/dev/src2/a/5.ts",
                "c:/dev/src2/a/b/c/d/e/f/6.ts",
                "c:/dev/src2/a/b/c/d/e/7.ts",
                "c:/dev/src2/a/b/c/d/e/8.ts",
                "c:/dev/src2/a/b/c/d/9.ts",
                "c:/dev/src2/a/10.ts",
                "c:/dev/src/index.ts",
            ].into_iter().map(str::to_string));
            files
        },
        target: core::ScriptTarget::EsNext,
    });
    cases
}

#[test]
fn test_program() {
    if !bundled::EMBEDDED {
        // Without embedding, we'd need to read all of the lib files out from disk into the MapFS.
        // Just skip this for now.
        return;
    }

    for test_case in program_test_cases() {
        let lib_prefix = bundled::lib_path() + "/";
        let mut fs = vfstest::from_map(
            std::iter::empty::<(&str, &str)>(),
            false, /*useCaseSensitiveFileNames*/
        );
        fs = bundled::wrap_fs(fs);

        for test_file in &test_case.files {
            let _ = fs.write_file(&test_file.file_name, &test_file.contents);
        }

        let opts = core::CompilerOptions {
            target: test_case.target,
            ..Default::default()
        };

        let host = compiler::new_compiler_host(
            "c:/dev/src".to_string(),
            fs,
            bundled::lib_path(),
            None,
            None,
        );

        let program = compiler::new_program(program_options(
            parsed_command_line(vec!["c:/dev/src/index.ts".to_string()], opts),
            host,
        ));

        let mut actual_files = Vec::new();
        for file in program.get_source_files() {
            actual_files.push(
                file.file_name()
                    .strip_prefix(&lib_prefix)
                    .unwrap_or(file.file_name())
                    .to_string(),
            );
        }

        assert_eq!(
            test_case.expected_files, actual_files,
            "{}",
            test_case.test_name
        );
    }
}

fn benchmark_new_program_iterations(iterations: usize) {
    if !bundled::EMBEDDED {
        // Without embedding, we'd need to read all of the lib files out from disk into the MapFS.
        // Just skip this for now.
        return;
    }

    for test_case in program_test_cases() {
        let mut fs = vfstest::from_map(
            std::iter::empty::<(&str, &str)>(),
            false, /*useCaseSensitiveFileNames*/
        );
        fs = bundled::wrap_fs(fs);

        for test_file in &test_case.files {
            let _ = fs.write_file(&test_file.file_name, &test_file.contents);
        }

        let opts = core::CompilerOptions {
            target: test_case.target,
            ..Default::default()
        };
        let host = compiler::new_compiler_host(
            "c:/dev/src".to_string(),
            fs,
            bundled::lib_path(),
            None,
            None,
        );
        let program_opts = program_options(
            parsed_command_line(vec!["c:/dev/src/index.ts".to_string()], opts),
            host,
        );

        for _ in 0..iterations {
            compiler::new_program(program_opts.clone());
        }
    }

    if repo::skip_if_no_type_script_submodule() {
        return;
    }

    let root_path = tspath::normalize_slashes(
        std::path::Path::new(&repo::type_script_submodule_path())
            .join("src")
            .join("compiler")
            .to_string_lossy()
            .as_ref(),
    );

    let mut fs = osvfs::fs();
    fs = bundled::wrap_fs(fs);

    let host = compiler::new_compiler_host(root_path.clone(), fs, bundled::lib_path(), None, None);

    let parse_host = CompilerHostParseConfigHost(host.as_ref());
    let (parsed, errors) = tsoptions::get_parsed_command_line_of_config_file(
        &tspath::combine_paths(&root_path, &["tsconfig.json"]),
        None,
        None,
        &parse_host,
        None,
    );
    assert_eq!(errors.len(), 0, "Expected no errors in parsed command line");

    let opts = program_options(
        Box::new(parsed.expect("Expected parsed command line")),
        host,
    );

    for _ in 0..iterations {
        compiler::new_program(opts.clone());
    }
}
