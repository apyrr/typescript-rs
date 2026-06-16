// package checker_test

use ts_ast as ast;
use ts_bundled as bundled;
use ts_compiler as compiler;
use ts_core as core;
use ts_repo as repo;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;
use ts_vfs::osvfs;
use ts_vfs::vfstest;

#[test]
fn test_get_symbol_at_location() {
    let content = r#"interface Foo {
  bar: string;
}
declare const foo: Foo;
foo.bar;"#;
    let mut fs = vfstest::from_map(
        [
            ("/foo.ts", content),
            (
                "/tsconfig.json",
                r#"
				{
					"compilerOptions": {},
					"files": ["foo.ts"]
				}
			"#,
            ),
        ],
        false, /*useCaseSensitiveFileNames*/
    );
    fs = bundled::wrap_fs(fs);

    let cd = "/";
    let host = compiler::new_compiler_host(cd, fs, bundled::lib_path(), None, None);

    let (parsed, errors) = tsoptions::get_parsed_command_line_of_config_file(
        "/tsconfig.json",
        Some(&core::CompilerOptions::default()),
        None,
        host,
        None,
    );
    assert_eq!(errors.len(), 0, "Expected no errors in parsed command line");

    let p = compiler::new_program(compiler::ProgramOptions {
        config: parsed,
        host,
        use_source_of_project_reference: false,
        single_threaded: core::TS_UNKNOWN,
        create_checker_pool: None,
        typings_location: String::new(),
        project_name: String::new(),
        type_script_version: String::new(),
        tracing: None,
    });
    p.bind_source_files();
    let file = p.get_source_file("/foo.ts");
    let interface_id = file.statements.nodes[0].name();
    let var_id = file.statements.nodes[1]
        .as_variable_statement()
        .declaration_list
        .as_variable_declaration_list()
        .declarations
        .nodes[0]
        .name();
    let prop_access = file.statements.nodes[2].expression();
    let nodes: Vec<ast::Node> = vec![interface_id, var_id, prop_access];
    let ctx = test_context();
    p.with_type_checker_for_file_using(compiler::CheckerAccess::context(&ctx), &file, |c| {
        for node in nodes {
            let symbol = c.get_symbol_at_location_public(node);
            if symbol.is_none() {
                panic!("Expected symbol to be non-nil");
            }
        }
    });
}

fn benchmark_new_checker_iterations(n: usize) {
    if repo::skip_if_no_type_script_submodule() {
        return;
    }
    let mut fs = osvfs::fs();
    fs = bundled::wrap_fs(fs);

    let root_path = tspath::combine_paths(
        &tspath::normalize_slashes(&repo::type_script_submodule_path()),
        ["src", "compiler"],
    );

    let host = compiler::new_compiler_host(&root_path, fs, bundled::lib_path(), None, None);
    let (parsed, errors) = tsoptions::get_parsed_command_line_of_config_file(
        &tspath::combine_paths(&root_path, ["tsconfig.json"]),
        Some(&core::CompilerOptions::default()),
        None,
        host,
        None,
    );
    assert_eq!(errors.len(), 0, "Expected no errors in parsed command line");
    let p = compiler::new_program(compiler::ProgramOptions {
        config: parsed,
        host,
        use_source_of_project_reference: false,
        single_threaded: core::TS_UNKNOWN,
        create_checker_pool: None,
        typings_location: String::new(),
        project_name: String::new(),
        type_script_version: String::new(),
        tracing: None,
    });

    for _ in 0..n {
        let source_file = p
            .get_parsed_source_files_refs()
            .into_iter()
            .next()
            .expect("benchmark program should have at least one source file");
        let ctx = test_context();
        p.with_type_checker_for_file_using(compiler::CheckerAccess::context(&ctx), source_file, |_| {});
    }
}

fn test_context() -> core::Context {
    core::Context::default()
}
