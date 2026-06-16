#![forbid(unsafe_code)]
use std::cmp::Ordering;

use ts_ast as ast;
use ts_core as core;
use ts_tspath as tspath;

pub type CompilerOptions = core::CompilerOptions;
pub type SourceFile = ast::SourceFile;

pub trait OutputPathsHost {
    fn common_source_directory(&self) -> String;
    fn get_current_directory(&self) -> String;
    fn use_case_sensitive_file_names(&self) -> bool;
}

#[derive(Default)]
pub struct OutputPaths {
    js_file_path: String,
    source_map_file_path: String,
    declaration_file_path: String,
    declaration_map_path: String,
}

impl OutputPaths {
    // DeclarationFilePath implements declarations.OutputPaths.
    pub fn declaration_file_path(&self) -> &str {
        &self.declaration_file_path
    }

    // JsFilePath implements declarations.OutputPaths.
    pub fn js_file_path(&self) -> &str {
        &self.js_file_path
    }

    pub fn source_map_file_path(&self) -> &str {
        &self.source_map_file_path
    }

    pub fn declaration_map_path(&self) -> &str {
        &self.declaration_map_path
    }
}

pub fn get_output_paths_for(
    source_file: &SourceFile,
    options: &CompilerOptions,
    host: &dyn OutputPathsHost,
    force_dts_emit: bool,
) -> OutputPaths {
    let file_name = source_file.file_name();
    let own_output_file_path = get_own_emit_output_file_path(
        &file_name,
        options,
        host,
        &get_output_extension(&file_name, options.jsx),
    );
    let is_json_file = ast::is_json_source_file(source_file);
    // If json file emits to the same location skip writing it, if emitDeclarationOnly skip writing it
    let is_json_emitted_to_same_location = is_json_file
        && tspath::compare_paths(
            &file_name,
            &own_output_file_path,
            &tspath::ComparePathsOptions {
                current_directory: host.get_current_directory(),
                use_case_sensitive_file_names: host.use_case_sensitive_file_names(),
            },
        ) == Ordering::Equal;
    let mut paths = OutputPaths::default();
    if !options.emit_declaration_only.is_true() && !is_json_emitted_to_same_location {
        paths.js_file_path = own_output_file_path;
        if !ast::is_json_source_file(source_file) {
            paths.source_map_file_path = get_source_map_file_path(&paths.js_file_path, options);
        }
    }
    if force_dts_emit || options.get_emit_declarations() && !is_json_file {
        paths.declaration_file_path =
            get_declaration_emit_output_file_path(&file_name, options, host);
        if options.get_are_declaration_maps_enabled() {
            paths.declaration_map_path = paths.declaration_file_path.clone() + ".map";
        }
    }
    paths
}

pub fn for_each_emitted_file(
    host: &dyn OutputPathsHost,
    options: &CompilerOptions,
    mut action: impl FnMut(OutputPaths, &SourceFile) -> bool,
    source_files: &[SourceFile],
    force_dts_emit: bool,
) -> bool {
    for source_file in source_files {
        if action(
            get_output_paths_for(source_file, options, host, force_dts_emit),
            source_file,
        ) {
            return true;
        }
    }
    false
}

pub fn get_output_js_file_name(
    input_file_name: &str,
    options: &CompilerOptions,
    host: &dyn OutputPathsHost,
) -> String {
    if options.emit_declaration_only.is_true() {
        return String::new();
    }
    let output_file_name = get_output_js_file_name_worker(input_file_name, options, host);
    if !tspath::file_extension_is(&output_file_name, tspath::EXTENSION_JSON)
        || tspath::compare_paths(
            input_file_name,
            &output_file_name,
            &tspath::ComparePathsOptions {
                current_directory: host.get_current_directory(),
                use_case_sensitive_file_names: host.use_case_sensitive_file_names(),
            },
        ) != Ordering::Equal
    {
        return output_file_name;
    }
    String::new()
}

pub fn get_output_js_file_name_worker(
    input_file_name: &str,
    options: &CompilerOptions,
    host: &dyn OutputPathsHost,
) -> String {
    tspath::change_extension(
        &get_output_path_without_changing_extension(input_file_name, &options.out_dir, host),
        &get_output_extension(input_file_name, options.jsx),
    )
}

pub fn get_output_declaration_file_name_worker(
    input_file_name: &str,
    options: &CompilerOptions,
    host: &dyn OutputPathsHost,
) -> String {
    let dir = if options.declaration_dir.is_empty() {
        &options.out_dir
    } else {
        &options.declaration_dir
    };
    tspath::change_extension(
        &get_output_path_without_changing_extension(input_file_name, dir, host),
        &tspath::get_declaration_emit_extension_for_path(input_file_name),
    )
}

pub fn get_output_extension(file_name: &str, jsx: core::JsxEmit) -> String {
    if tspath::file_extension_is(file_name, tspath::EXTENSION_JSON) {
        return tspath::EXTENSION_JSON.to_string();
    }
    if jsx == core::JsxEmit::Preserve
        && tspath::file_extension_is_one_of(
            file_name,
            &[tspath::EXTENSION_JSX, tspath::EXTENSION_TSX],
        )
    {
        return tspath::EXTENSION_JSX.to_string();
    }
    if tspath::file_extension_is_one_of(file_name, &[tspath::EXTENSION_MTS, tspath::EXTENSION_MJS])
    {
        return tspath::EXTENSION_MJS.to_string();
    }
    if tspath::file_extension_is_one_of(file_name, &[tspath::EXTENSION_CTS, tspath::EXTENSION_CJS])
    {
        return tspath::EXTENSION_CJS.to_string();
    }
    tspath::EXTENSION_JS.to_string()
}

pub fn get_declaration_emit_output_file_path(
    file: &str,
    options: &CompilerOptions,
    host: &dyn OutputPathsHost,
) -> String {
    let output_dir = if !options.declaration_dir.is_empty() {
        Some(options.declaration_dir.as_str())
    } else if !options.out_dir.is_empty() {
        Some(options.out_dir.as_str())
    } else {
        None
    };

    let path = if let Some(output_dir) = output_dir {
        get_source_file_path_in_new_dir_worker(
            file,
            output_dir,
            &host.get_current_directory(),
            &host.common_source_directory(),
            host.use_case_sensitive_file_names(),
        )
    } else {
        file.to_string()
    };
    let declaration_extension = tspath::get_declaration_emit_extension_for_path(&path);
    tspath::remove_file_extension(&path) + &declaration_extension
}

pub fn get_source_file_path_in_new_dir(
    file_name: &str,
    new_dir_path: &str,
    current_directory: &str,
    common_source_directory: &str,
    use_case_sensitive_file_names: bool,
) -> String {
    let mut source_file_path = tspath::get_normalized_absolute_path(file_name, current_directory);
    let common_source_directory =
        tspath::ensure_trailing_directory_separator(common_source_directory);
    let is_source_file_in_common_source_directory = tspath::contains_path(
        &common_source_directory,
        &source_file_path,
        &tspath::ComparePathsOptions {
            use_case_sensitive_file_names,
            current_directory: current_directory.to_string(),
        },
    );
    if is_source_file_in_common_source_directory {
        source_file_path = source_file_path[common_source_directory.len()..].to_string();
    }
    tspath::combine_paths(new_dir_path, &[&source_file_path])
}

fn get_output_path_without_changing_extension(
    input_file_name: &str,
    output_directory: &str,
    host: &dyn OutputPathsHost,
) -> String {
    if !output_directory.is_empty() {
        let relative_path = tspath::get_relative_path_from_directory(
            &host.common_source_directory(),
            input_file_name,
            &tspath::ComparePathsOptions {
                use_case_sensitive_file_names: host.use_case_sensitive_file_names(),
                current_directory: host.get_current_directory(),
            },
        );
        return tspath::resolve_path(output_directory, &[&relative_path]);
    }
    input_file_name.to_string()
}

pub fn get_source_file_path_in_new_dir_worker(
    file_name: &str,
    new_dir_path: &str,
    current_directory: &str,
    common_source_directory: &str,
    use_case_sensitive_file_names: bool,
) -> String {
    let mut source_file_path = tspath::get_normalized_absolute_path(file_name, current_directory);
    let common_dir =
        tspath::get_canonical_file_name(common_source_directory, use_case_sensitive_file_names);
    let canon_file =
        tspath::get_canonical_file_name(&source_file_path, use_case_sensitive_file_names);
    let is_source_file_in_common_source_directory = canon_file.starts_with(&common_dir);
    if is_source_file_in_common_source_directory {
        source_file_path = source_file_path[common_source_directory.len()..].to_string();
    }
    tspath::combine_paths(new_dir_path, &[&source_file_path])
}

fn get_own_emit_output_file_path(
    file_name: &str,
    options: &CompilerOptions,
    host: &dyn OutputPathsHost,
    extension: &str,
) -> String {
    let emit_output_file_path_without_extension = if !options.out_dir.is_empty() {
        let current_directory = host.get_current_directory();
        tspath::remove_file_extension(&get_source_file_path_in_new_dir(
            file_name,
            &options.out_dir,
            &current_directory,
            &host.common_source_directory(),
            host.use_case_sensitive_file_names(),
        ))
    } else {
        tspath::remove_file_extension(file_name)
    };
    emit_output_file_path_without_extension + extension
}

pub fn get_source_map_file_path(js_file_path: &str, options: &CompilerOptions) -> String {
    if options.source_map.is_true() && !options.inline_source_map.is_true() {
        return js_file_path.to_string() + ".map";
    }
    String::new()
}

pub fn get_build_info_file_name(
    options: &CompilerOptions,
    opts: tspath::ComparePathsOptions,
) -> String {
    if !options.is_incremental() && !options.build.is_true() {
        return String::new();
    }
    if !options.ts_build_info_file.is_empty() {
        return options.ts_build_info_file.clone();
    }
    if options.config_file_path.is_empty() {
        return String::new();
    }
    let config_file_extension_less = tspath::remove_file_extension(&options.config_file_path);
    let build_info_extension_less = if !options.out_dir.is_empty() {
        if !options.root_dir.is_empty() {
            let relative_path = tspath::get_relative_path_from_directory(
                &options.root_dir,
                &config_file_extension_less,
                &opts,
            );
            tspath::resolve_path(&options.out_dir, &[&relative_path])
        } else {
            tspath::combine_paths(
                &options.out_dir,
                &[&tspath::get_base_file_name(&config_file_extension_less)],
            )
        }
    } else {
        config_file_extension_less
    };
    build_info_extension_less + tspath::EXTENSION_TS_BUILD_INFO
}

fn compute_common_source_directory_of_filenames(
    file_names: &[String],
    current_directory: &str,
    use_case_sensitive_file_names: bool,
) -> String {
    let mut common_path_components: Option<Vec<String>> = None;
    for source_file in file_names {
        // Each file contributes into common source file path
        let mut source_path_components =
            tspath::get_normalized_path_components(source_file, current_directory);

        // The base file name is not part of the common directory path
        source_path_components.truncate(source_path_components.len().saturating_sub(1));

        let Some(common) = common_path_components.as_mut() else {
            // first file
            common_path_components = Some(source_path_components);
            continue;
        };

        let n = common.len().min(source_path_components.len());
        for i in 0..n {
            if tspath::get_canonical_file_name(&common[i], use_case_sensitive_file_names)
                != tspath::get_canonical_file_name(
                    &source_path_components[i],
                    use_case_sensitive_file_names,
                )
            {
                if i == 0 {
                    // Failed to find any common path component
                    return String::new();
                }

                // New common path found that is 0 -> i-1
                common.truncate(i);
                break;
            }
        }

        // If the sourcePathComponents was shorter than the commonPathComponents, truncate to the sourcePathComponents
        if source_path_components.len() < common.len() {
            common.truncate(source_path_components.len());
        }
    }

    let Some(common_path_components) = common_path_components else {
        return current_directory.to_owned();
    };
    if common_path_components.is_empty() {
        // Can happen when all input files are .d.ts files
        return current_directory.to_owned();
    }

    tspath::get_path_from_path_components(&common_path_components)
}

pub fn get_computed_common_source_directory(
    emitted_files: &[String],
    current_directory: &str,
    use_case_sensitive_file_names: bool,
) -> String {
    let mut common_source_directory = compute_common_source_directory_of_filenames(
        emitted_files,
        current_directory,
        use_case_sensitive_file_names,
    );
    if !common_source_directory.is_empty() {
        common_source_directory =
            tspath::ensure_trailing_directory_separator(&common_source_directory);
    }
    common_source_directory
}

pub fn get_common_source_directory(
    options: &CompilerOptions,
    files: impl Fn() -> Vec<String>,
    current_directory: &str,
    use_case_sensitive_file_names: bool,
    check_source_files_belong_to_path: Option<impl Fn(Vec<String>, &str) -> bool>,
) -> String {
    let mut common_source_directory;
    if !options.root_dir.is_empty() {
        // If a rootDir is specified use it as the commonSourceDirectory
        common_source_directory = options.root_dir.clone();
        if let Some(check_source_files_belong_to_path) = check_source_files_belong_to_path {
            check_source_files_belong_to_path(files(), &options.root_dir);
        }
    } else if !options.config_file_path.is_empty() {
        // If the rootDir is not specified, then the common source directory is the directory of the config file.
        common_source_directory = tspath::get_directory_path(&options.config_file_path);
        if let Some(check_source_files_belong_to_path) = check_source_files_belong_to_path {
            check_source_files_belong_to_path(files(), &common_source_directory);
        }
    } else {
        common_source_directory = compute_common_source_directory_of_filenames(
            &files(),
            current_directory,
            use_case_sensitive_file_names,
        );
    }

    if !common_source_directory.is_empty() {
        // Make sure directory path ends with directory separator so this string can directly
        // used to replace with "" to get the relative path of the source file and the relative path doesn't
        // start with / making it rooted path
        common_source_directory =
            tspath::ensure_trailing_directory_separator(&common_source_directory);
    }

    common_source_directory
}
