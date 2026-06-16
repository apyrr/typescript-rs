use std::collections::HashMap;

use super::error_baseline::{diagnostic_from_ast_with_files, get_error_baseline};
use crate::baseline;
use crate::harnessutil::{
    CompilationOutput, CompilationResult, CompilerOptions, Diagnostic, HarnessOptions,
    ParsedCommandLine, ProgramHandle, TestFile, compile_files_ex,
};
use ts_collections::OrderedMap;
use ts_tspath as tspath;

pub struct JsEmitBaselineInput<'a> {
    pub baseline_path: &'a str,
    pub header: &'a str,
    pub options: &'a CompilerOptions,
    pub diagnostics: &'a [Diagnostic],
    pub program: Option<&'a ProgramHandle>,
    pub symlinks: &'a HashMap<String, String>,
    pub js: &'a OrderedMap<String, String>,
    pub dts: &'a OrderedMap<String, String>,
    pub inputs_and_outputs: &'a OrderedMap<String, CompilationOutput>,
    pub ts_config_files: &'a [TestFile],
    pub to_be_compiled: &'a [TestFile],
    pub other_files: &'a [TestFile],
    pub harness_settings: &'a HarnessOptions,
    pub opts: baseline::Options,
}

pub fn do_js_emit_baseline(input: JsEmitBaselineInput<'_>) -> Result<(), String> {
    let JsEmitBaselineInput {
        baseline_path,
        header,
        options,
        diagnostics,
        program,
        symlinks,
        js,
        dts,
        inputs_and_outputs,
        ts_config_files,
        to_be_compiled,
        other_files,
        harness_settings,
        opts,
    } = input;

    if !options.no_emit && !options.emit_declaration_only && js.is_empty() && diagnostics.is_empty()
    {
        return Err(
            "Expected at least one js file to be emitted or at least one error to be created."
                .to_string(),
        );
    }

    let mut ts_code = format!("//// [{header}] ////\r\n\r\n");
    let mut ts_sources = other_files.to_vec();
    ts_sources.extend_from_slice(to_be_compiled);
    for (index, file) in ts_sources.iter().enumerate() {
        ts_code.push_str(&format!(
            "//// [{}]\r\n{}",
            base_file_name(&file.unit_name),
            file.content
        ));
        if index + 1 < ts_sources.len() {
            ts_code.push_str("\r\n");
        }
    }

    let mut js_code = String::new();
    for (unit_name, content) in js.entries() {
        append_output(
            &mut js_code,
            &TestFile {
                unit_name: unit_name.clone(),
                content: content.clone(),
            },
            harness_settings,
        );
    }
    if !dts.is_empty() {
        js_code.push_str("\r\n\r\n");
        for (unit_name, content) in dts.entries() {
            js_code.push_str(&file_output(
                &TestFile {
                    unit_name: unit_name.clone(),
                    content: content.clone(),
                },
                harness_settings,
            ));
        }
    }

    let decl_context = prepare_declaration_compilation_context(
        to_be_compiled,
        other_files,
        diagnostics,
        program,
        js,
        dts,
        inputs_and_outputs,
        harness_settings,
        options,
        "",
    );
    let decl_file_compilation_result =
        compile_declaration_files(decl_context, symlinks.clone(), ts_config_files);

    if let Some(decl_file_compilation_result) = decl_file_compilation_result {
        if !decl_file_compilation_result
            .decl_result
            .diagnostics
            .is_empty()
        {
            js_code.push_str("\r\n\r\n//// [DtsFileErrors]\r\n");
            js_code.push_str("\r\n\r\n");
            let mut files = ts_config_files.to_vec();
            files.extend_from_slice(&decl_file_compilation_result.decl_input_files);
            files.extend_from_slice(&decl_file_compilation_result.decl_other_files);
            let diagnostics = decl_file_compilation_result
                .decl_result
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic_from_ast_with_files(diagnostic, &files))
                .collect::<Vec<_>>();
            js_code.push_str(&get_error_baseline(&files, &diagnostics, false));
        }
    }

    if !options.no_check && !options.no_emit {
        let mut no_check_options = options.clone();
        no_check_options.no_check = true;
        let no_check_harness_settings = harness_settings.clone();
        let no_check_result = compile_files_ex(
            to_be_compiled,
            other_files,
            &no_check_harness_settings,
            &no_check_options,
            &harness_settings.current_directory,
            symlinks.clone(),
            program.map(|program| ParsedCommandLine {
                config_file: program.0.command_line().config_file.clone(),
                config_file_path: program.0.command_line().config_file_path.clone(),
                ..ParsedCommandLine::default()
            }),
        );
        let no_check_dts = no_check_result
            .dts
            .entries()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<HashMap<_, _>>();
        let original_dts = dts
            .entries()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<HashMap<_, _>>();
        compare_result_file_sets(&mut js_code, &no_check_dts, &original_dts, harness_settings);

        let no_check_js = no_check_result
            .js
            .entries()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<HashMap<_, _>>();
        let original_js = js
            .entries()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<HashMap<_, _>>();
        compare_result_file_sets(&mut js_code, &no_check_js, &original_js, harness_settings);
    }

    let baseline_path = if baseline_path.ends_with(".ts") || baseline_path.ends_with(".tsx") {
        format!(
            "{}.js",
            baseline_path
                .rsplit_once('.')
                .map(|(base, _)| base)
                .unwrap_or(baseline_path)
        )
    } else {
        baseline_path.to_string()
    };
    let actual = if js_code.is_empty() {
        baseline::NO_CONTENT.to_string()
    } else {
        format!("{ts_code}\r\n\r\n{js_code}")
    };
    baseline::run(&baseline_path, &actual, opts)
}

pub fn file_output(file: &TestFile, settings: &HarnessOptions) -> String {
    let file_name = if settings.full_emit_paths {
        remove_test_path_prefixes(&file.unit_name)
    } else {
        base_file_name(&file.unit_name)
    };
    format!("//// [{file_name}]\r\n{}", file.content)
}

#[derive(Clone, Debug)]
pub struct DeclarationCompilationContext {
    pub decl_input_files: Vec<TestFile>,
    pub decl_other_files: Vec<TestFile>,
    pub harness_settings: HarnessOptions,
    pub options: CompilerOptions,
    pub current_directory: String,
    pub config_file: Option<ts_tsoptions::TsConfigSourceFile>,
    pub config_file_path: String,
}

pub fn prepare_declaration_compilation_context(
    _input_files: &[TestFile],
    _other_files: &[TestFile],
    diagnostics: &[Diagnostic],
    program: Option<&ProgramHandle>,
    js: &OrderedMap<String, String>,
    dts: &OrderedMap<String, String>,
    inputs_and_outputs: &OrderedMap<String, CompilationOutput>,
    harness_settings: &HarnessOptions,
    options: &CompilerOptions,
    current_directory: &str,
) -> Option<DeclarationCompilationContext> {
    if options.declaration && diagnostics.is_empty() {
        if options.emit_declaration_only {
            if !js.is_empty() || (dts.is_empty() && !options.no_emit) {
                panic!("Only declaration files should be generated when emitDeclarationOnly:true");
            }
        } else if dts.size() != get_number_of_js_files(js, false) {
            panic!(
                "There were no errors and declFiles generated did not match number of js files generated"
            );
        }
    }

    let mut decl_input_files = Vec::new();
    let mut decl_other_files = Vec::new();

    if !options.declaration || !diagnostics.is_empty() || dts.is_empty() {
        return None;
    }

    for file in _input_files {
        add_dts_file(
            file,
            &mut decl_input_files,
            &decl_other_files,
            program,
            dts,
            inputs_and_outputs,
            harness_settings,
            options,
        );
    }
    for file in _other_files {
        add_dts_file(
            file,
            &mut decl_other_files,
            &decl_input_files,
            program,
            dts,
            inputs_and_outputs,
            harness_settings,
            options,
        );
    }

    let (config_file, config_file_path) = program
        .map(|program| {
            (
                program.0.command_line().config_file.clone(),
                program.0.command_line().config_file_path.clone(),
            )
        })
        .unwrap_or_default();

    Some(DeclarationCompilationContext {
        decl_input_files,
        decl_other_files,
        harness_settings: harness_settings.clone(),
        options: options.clone(),
        current_directory: if current_directory.is_empty() {
            harness_settings.current_directory.clone()
        } else {
            current_directory.to_string()
        },
        config_file,
        config_file_path,
    })
}

fn add_dts_file(
    file: &TestFile,
    dts_files: &mut Vec<TestFile>,
    other_dts_files: &[TestFile],
    program: Option<&ProgramHandle>,
    dts: &OrderedMap<String, String>,
    inputs_and_outputs: &OrderedMap<String, CompilationOutput>,
    harness_settings: &HarnessOptions,
    options: &CompilerOptions,
) {
    if tspath::is_declaration_file_name(&file.unit_name)
        || tspath::has_json_file_extension(&file.unit_name)
    {
        dts_files.push(file.clone());
    } else if tspath::has_ts_file_extension(&file.unit_name)
        || (tspath::has_js_file_extension(&file.unit_name) && options.allow_js.unwrap_or(false))
    {
        let Some(decl_file) = find_result_code_file(
            &file.unit_name,
            program,
            dts,
            inputs_and_outputs,
            harness_settings,
            options,
        ) else {
            return;
        };
        if find_unit(&decl_file.unit_name, dts_files).is_none()
            && find_unit(&decl_file.unit_name, other_dts_files).is_none()
        {
            dts_files.push(TestFile {
                unit_name: decl_file.unit_name,
                content: decl_file.content.trim_start_matches('\u{feff}').to_string(),
            });
        }
    }
}

fn find_unit<'a>(file_name: &str, units: &'a [TestFile]) -> Option<&'a TestFile> {
    units.iter().find(|unit| unit.unit_name == file_name)
}

fn find_result_code_file(
    file_name: &str,
    program: Option<&ProgramHandle>,
    dts: &OrderedMap<String, String>,
    inputs_and_outputs: &OrderedMap<String, CompilationOutput>,
    harness_settings: &HarnessOptions,
    options: &CompilerOptions,
) -> Option<TestFile> {
    if let Some(outputs) = inputs_and_outputs.get(&file_name.to_string()) {
        return outputs.dts.clone();
    }

    let program = program?;
    let source_file = program.0.get_source_file_ref(file_name).unwrap_or_else(|| {
        panic!("Program has no source file with name '{file_name}'");
    });
    let mut source_file_name = if !options.out_dir.is_empty() {
        let mut source_file_path = tspath::get_normalized_absolute_path(
            &source_file.file_name(),
            &harness_settings.current_directory,
        );
        source_file_path = source_file_path.replacen(&program.0.common_source_directory(), "", 1);
        tspath::combine_paths(&options.out_dir, &[&source_file_path])
    } else {
        source_file.file_name()
    };

    source_file_name = tspath::remove_file_extension(&source_file_name);
    source_file_name.push_str(&tspath::get_declaration_emit_extension_for_path(
        &source_file_name,
    ));
    dts.get(&source_file_name).map(|content| TestFile {
        unit_name: source_file_name,
        content: content.clone(),
    })
}

pub fn get_number_of_js_files(js: &OrderedMap<String, String>, include_json: bool) -> usize {
    if include_json {
        js.size()
    } else {
        js.entries()
            .filter(|(unit_name, _)| !tspath::file_extension_is(unit_name, tspath::EXTENSION_JSON))
            .count()
    }
}

#[derive(Clone)]
pub struct DeclarationCompilationResult {
    pub decl_input_files: Vec<TestFile>,
    pub decl_other_files: Vec<TestFile>,
    pub decl_result: CompilationResult,
}

pub fn compile_declaration_files(
    context: Option<DeclarationCompilationContext>,
    symlinks: HashMap<String, String>,
    _ts_config_files: &[TestFile],
) -> Option<DeclarationCompilationResult> {
    let context = context?;
    let tsconfig = context.config_file.as_ref().map(|_| ParsedCommandLine {
        config_file: context.config_file.clone(),
        config_file_path: context.config_file_path.clone(),
        ..ParsedCommandLine::default()
    });
    let decl_result = compile_files_ex(
        &context.decl_input_files,
        &context.decl_other_files,
        &context.harness_settings,
        &context.options,
        &context.current_directory,
        symlinks.clone(),
        tsconfig,
    );
    Some(DeclarationCompilationResult {
        decl_input_files: context.decl_input_files,
        decl_other_files: context.decl_other_files,
        decl_result,
    })
}

fn append_output(out: &mut String, file: &TestFile, settings: &HarnessOptions) {
    if !out.is_empty() && !out.ends_with('\n') {
        out.push_str("\r\n");
    }
    out.push_str(&file_output(file, settings));
}

pub fn compare_result_file_sets(
    out: &mut String,
    no_check_files: &HashMap<String, String>,
    original_files: &HashMap<String, String>,
    settings: &HarnessOptions,
) {
    let mut keys = no_check_files.keys().collect::<Vec<_>>();
    keys.sort();

    for key in keys {
        let no_check_content = &no_check_files[key];
        match original_files.get(key) {
            None => {
                out.push_str("\r\n\r\n!!!! File ");
                out.push_str(&remove_test_path_prefixes(key));
                out.push_str(" missing from original emit, but present in noCheck emit\r\n");
                out.push_str(&file_output(
                    &TestFile {
                        unit_name: key.clone(),
                        content: no_check_content.clone(),
                    },
                    settings,
                ));
            }
            Some(original_content) if original_content != no_check_content => {
                out.push_str("\r\n\r\n!!!! File ");
                out.push_str(&remove_test_path_prefixes(key));
                out.push_str(" differs from original emit in noCheck emit\r\n");
                let file_name = if settings.full_emit_paths {
                    remove_test_path_prefixes(key)
                } else {
                    base_file_name(key)
                };
                out.push_str(&format!("//// [{file_name}]\r\n"));
                out.push_str(&baseline::diff_text(
                    "Expected\tThe full check baseline",
                    "Actual\twith noCheck set",
                    original_content,
                    no_check_content,
                ));
            }
            _ => {}
        }
    }
}

fn base_file_name(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

fn remove_test_path_prefixes(path: &str) -> String {
    path.replace("/.src/", "").replace("/.lib/", "")
}
