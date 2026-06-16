use ts_ast as ast;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_outputpaths as outputpaths;
use ts_printer as printer;
use ts_sourcemap as sourcemap;
use ts_stringutil as stringutil;
use ts_tracing as tracing;
use ts_transformers::{
    self as transformers, declarations, estransforms, inliners, jsxtransforms, moduletransforms,
    tstransforms,
};
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use crate::program::outputpaths_compiler_options;
use crate::{EmitHost, EmitResult, SourceMapEmitResult, WriteFileData};

pub type EmitOnly = u8;

pub const EMIT_ALL: EmitOnly = 0;
pub const EMIT_ONLY_NONE: EmitOnly = EMIT_ALL;
pub const EMIT_ONLY_JS: EmitOnly = 1;
pub const EMIT_ONLY_DTS: EmitOnly = 2;
pub const EMIT_ONLY_FORCED_DTS: EmitOnly = 3;

fn emit_host_options(host: &dyn printer::EmitHost) -> core::CompilerOptions {
    printer::EmitHost::options(host).unwrap_or_default()
}

pub struct Emitter<'a> {
    host: &'a mut dyn EmitHost,
    emit_only: EmitOnly,
    emitter_diagnostics: ast::DiagnosticsCollection,
    writer: printer::SharedEmitTextWriter,
    paths: &'a outputpaths::OutputPaths,
    source_file: &'a ast::SourceFile,
    emit_result: EmitResult,
    write_file: Option<crate::WriteFile>,
    tr: Option<&'a mut tracing::Tracing>,
}

impl<'a> Emitter<'a> {
    fn emit(&mut self) {
        let pop_trace = self.tr.as_mut().map(|tr| {
            tr.push(
                tracing::Phase::Emit,
                "emit",
                hashmap! { "path" => self.source_file.path().to_string() },
                true,
            )
        });
        self.emit_js_file(
            self.source_file,
            &self.paths.js_file_path(),
            &self.paths.source_map_file_path(),
        );
        self.emit_declaration_file(
            self.source_file,
            &self.paths.declaration_file_path(),
            &self.paths.declaration_map_path(),
        );
        self.emit_result.diagnostics = self.emitter_diagnostics.get_diagnostics();
        if let Some(pop_trace) = pop_trace {
            if let Some(tr) = self.tr.as_mut() {
                pop_trace(&mut **tr);
            }
        }
    }

    fn get_declaration_transformers(
        &mut self,
        emit_context: &mut printer::EmitContext,
        declaration_file_path: &str,
        declaration_map_path: &str,
    ) -> Vec<declarations::DeclarationTransformer<'_>> {
        let options = emit_host_options(self.host);
        let transform = declarations::new_declaration_transformer(
            self.host,
            Some(emit_context),
            &options,
            declaration_file_path,
            declaration_map_path,
        );
        vec![transform]
    }

    fn run_script_transformers(
        &mut self,
        emit_context: &mut printer::EmitContext,
        source_file: &ast::SourceFile,
    ) -> Option<Box<ast::SourceFile>> {
        let pop_trace = self.tr.as_mut().map(|tr| {
            tr.push(
                tracing::Phase::Emit,
                "transformNodes",
                hashmap! { "path" => source_file.path().to_string() },
                false,
            )
        });
        let transformers = get_script_transformers(emit_context, self.host, source_file);
        let transformed_source_file = if let Some(mut transformer) =
            transformers::chain::chain_constructed(transformers.len(), transformers, None)
        {
            Some(Box::new(
                transformer.transform_source_file_with_emit_context(source_file, emit_context),
            ))
        } else {
            None
        };
        if let Some(pop_trace) = pop_trace {
            if let Some(tr) = self.tr.as_mut() {
                pop_trace(&mut **tr);
            }
        }
        transformed_source_file
    }

    fn run_declaration_transformers(
        &mut self,
        emit_context: &mut printer::EmitContext,
        source_file: &ast::SourceFile,
        declaration_file_path: &str,
        declaration_map_path: &str,
    ) -> (Option<Box<ast::SourceFile>>, Vec<ast::Diagnostic>) {
        let pop_trace = self.tr.as_mut().map(|tr| {
            tr.push(
                tracing::Phase::Emit,
                "transformNodes",
                hashmap! { "path" => source_file.path().to_string() },
                false,
            )
        });
        let mut diags = Vec::new();
        let mut transformed_source_file: Option<Box<ast::SourceFile>> = None;
        for mut transformer in self
            .get_declaration_transformers(emit_context, declaration_file_path, declaration_map_path)
            .into_iter()
        {
            let input_source_file = transformed_source_file.as_deref().unwrap_or(source_file);
            transformed_source_file = Some(Box::new(
                transformer
                    .transform_source_file_with_emit_context(input_source_file, emit_context),
            ));
            diags.extend(transformer.get_diagnostics());
        }
        if let Some(pop_trace) = pop_trace {
            if let Some(tr) = self.tr.as_mut() {
                pop_trace(&mut **tr);
            }
        }
        (transformed_source_file, diags)
    }
}

pub(crate) fn emit_source_file(
    host: &mut dyn EmitHost,
    source_file: &ast::SourceFile,
    emit_only: EmitOnly,
    write_file: Option<crate::WriteFile>,
    tr: Option<&mut tracing::Tracing>,
    new_line: &str,
) -> EmitResult {
    let output_source_file = source_file.share_readonly();
    let options = emit_host_options(host);
    let output_options = outputpaths_compiler_options(&options);
    let paths = outputpaths::get_output_paths_for(
        &output_source_file,
        &output_options,
        host,
        emit_only == crate::EMIT_ONLY_FORCED_DTS,
    );
    let mut emitter = Emitter {
        host,
        emit_only,
        emitter_diagnostics: ast::DiagnosticsCollection::default(),
        writer: printer::new_shared_text_writer(new_line.to_string(), 0),
        paths: &paths,
        source_file,
        emit_result: EmitResult {
            emit_skipped: false,
            diagnostics: Vec::new(),
            emitted_files: Vec::new(),
            source_maps: Vec::new(),
        },
        write_file,
        tr,
    };
    emitter.emit();
    emitter.emit_result
}

fn should_emit_source_maps(
    map_options: &core::CompilerOptions,
    source_file: &ast::SourceFile,
) -> bool {
    (map_options.source_map.is_true() || map_options.inline_source_map.is_true())
        && !tspath::file_extension_is(&source_file.file_name(), tspath::EXTENSION_JSON)
}

fn should_emit_declaration_source_maps(
    map_options: &core::CompilerOptions,
    source_file: &ast::SourceFile,
) -> bool {
    map_options.declaration_map.is_true()
        && !tspath::file_extension_is(&source_file.file_name(), tspath::EXTENSION_JSON)
}

fn get_source_root(map_options: &core::CompilerOptions) -> String {
    // Normalize source root and make sure it has trailing "/" so that it can be used to combine paths with the
    // relative paths of the sources list in the sourcemap
    let mut source_root = tspath::normalize_slashes(&map_options.source_root);
    if !source_root.is_empty() {
        source_root = tspath::ensure_trailing_directory_separator(&source_root);
    }
    source_root
}

impl<'a> Emitter<'a> {
    fn get_source_map_directory(
        &self,
        map_options: &core::CompilerOptions,
        file_path: &str,
        source_file: Option<&ast::SourceFile>,
    ) -> String {
        if !map_options.source_root.is_empty() {
            return printer::EmitHost::common_source_directory(self.host);
        }
        if !map_options.map_root.is_empty() {
            let mut source_map_dir = tspath::normalize_slashes(&map_options.map_root);
            if let Some(source_file) = source_file {
                // For modules or multiple emit files the mapRoot will have directory structure like the sources
                // So if src\a.ts and src\lib\b.ts are compiled together user would be moving the maps into mapRoot\a.js.map and mapRoot\lib\b.js.map
                source_map_dir =
                    tspath::get_directory_path(&outputpaths::get_source_file_path_in_new_dir(
                        &source_file.file_name(),
                        &source_map_dir,
                        &printer::EmitHost::get_current_directory(self.host),
                        &printer::EmitHost::common_source_directory(self.host),
                        printer::EmitHost::use_case_sensitive_file_names(self.host),
                    ));
            }
            if tspath::get_root_length(&source_map_dir) == 0 {
                // The relative paths are relative to the common directory
                source_map_dir = tspath::combine_paths(
                    &printer::EmitHost::common_source_directory(self.host),
                    &[&source_map_dir],
                );
            }
            return source_map_dir;
        }
        tspath::get_directory_path(&tspath::normalize_path(file_path))
    }

    fn get_source_mapping_url(
        &self,
        map_options: &core::CompilerOptions,
        source_map_generator: &mut sourcemap::Generator,
        file_path: &str,
        source_map_file_path: &str,
        source_file: Option<&ast::SourceFile>,
    ) -> String {
        if map_options.inline_source_map.is_true() {
            // Encode the sourceMap into the sourceMap url
            return source_map_generator.base64_data_url();
        }

        let source_map_file =
            tspath::get_base_file_name(&tspath::normalize_slashes(source_map_file_path));
        if !map_options.map_root.is_empty() {
            let mut source_map_dir = tspath::normalize_slashes(&map_options.map_root);
            if let Some(source_file) = source_file {
                // For modules or multiple emit files the mapRoot will have directory structure like the sources
                // So if src\a.ts and src\lib\b.ts are compiled together user would be moving the maps into mapRoot\a.js.map and mapRoot\lib\b.js.map
                source_map_dir =
                    tspath::get_directory_path(&outputpaths::get_source_file_path_in_new_dir(
                        &source_file.file_name(),
                        &source_map_dir,
                        &printer::EmitHost::get_current_directory(self.host),
                        &printer::EmitHost::common_source_directory(self.host),
                        printer::EmitHost::use_case_sensitive_file_names(self.host),
                    ));
            }
            if tspath::get_root_length(&source_map_dir) == 0 {
                // The relative paths are relative to the common directory
                source_map_dir = tspath::combine_paths(
                    &printer::EmitHost::common_source_directory(self.host),
                    &[&source_map_dir],
                );
                return stringutil::encode_uri(&tspath::get_relative_path_to_directory_or_url(
                    &tspath::get_directory_path(&tspath::normalize_path(file_path)), // get the relative sourceMapDir path based on jsFilePath
                    &tspath::combine_paths(&source_map_dir, &[&source_map_file]), // this is where user expects to see sourceMap
                    true, /*isAbsolutePathAnUrl*/
                    &tspath::ComparePathsOptions {
                        use_case_sensitive_file_names:
                            printer::EmitHost::use_case_sensitive_file_names(self.host),
                        current_directory: printer::EmitHost::get_current_directory(self.host),
                    },
                ));
            } else {
                return stringutil::encode_uri(&tspath::combine_paths(
                    &source_map_dir,
                    &[&source_map_file],
                ));
            }
        }
        stringutil::encode_uri(&source_map_file)
    }
}

pub trait SourceFileMayBeEmittedHost {
    fn options(&self) -> &core::CompilerOptions;
    fn get_project_reference_from_source(
        &self,
        path: tspath::Path,
    ) -> Option<tsoptions::SourceOutputAndProjectReference>;
    fn is_source_file_from_external_library(&self, file: &ast::SourceFile) -> bool;
    fn get_current_directory(&self) -> String;
    fn use_case_sensitive_file_names(&self) -> bool;
    fn source_files(&self) -> Vec<&ast::SourceFile>;
}

pub(crate) fn source_file_may_be_emitted(
    source_file: &ast::SourceFile,
    host: &dyn SourceFileMayBeEmittedHost,
    force_dts_emit: bool,
) -> bool {
    // TODO: move this to outputpaths?

    let options = host.options();
    // Js files are emitted only if option is enabled
    if options.no_emit_for_js_files.is_true() && ast::is_source_file_js(source_file) {
        return false;
    }

    // Declaration files are not emitted
    if source_file.is_declaration_file() {
        return false;
    }

    // Source file from node_modules are not emitted
    if host.is_source_file_from_external_library(source_file) {
        return false;
    }

    // forcing dts emit => file needs to be emitted
    if force_dts_emit {
        return true;
    }

    // Check other conditions for file emit
    // Source files from referenced projects are not emitted
    if host
        .get_project_reference_from_source(source_file.path())
        .is_some()
    {
        return false;
    }

    // Any non json file should be emitted
    if !ast::is_json_source_file(source_file) {
        return true;
    }

    // Json file is not emitted if outDir is not specified
    if options.out_dir.is_empty() {
        return false;
    }

    // Otherwise, if rootDir is specified or a config file exists, we know the common source directory and can check if the file would be emitted in the same location
    if !options.root_dir.is_empty() || !options.config_file_path.is_empty() {
        let common_dir = tspath::get_normalized_absolute_path(
            &outputpaths::get_common_source_directory(
                &crate::program::outputpaths_compiler_options(options),
                || Vec::<String>::new(),
                &host.get_current_directory(),
                host.use_case_sensitive_file_names(),
                None::<fn(Vec<String>, &str) -> bool>,
            ),
            &host.get_current_directory(),
        );
        let output_path = outputpaths::get_source_file_path_in_new_dir_worker(
            &source_file.file_name(),
            &options.out_dir,
            &host.get_current_directory(),
            &common_dir,
            host.use_case_sensitive_file_names(),
        );
        if tspath::compare_paths(
            &source_file.file_name(),
            &output_path,
            &tspath::ComparePathsOptions {
                use_case_sensitive_file_names: host.use_case_sensitive_file_names(),
                current_directory: host.get_current_directory(),
            },
        ) == std::cmp::Ordering::Equal
        {
            return false;
        }
    }

    true
}

pub(crate) fn get_source_files_to_emit<'a>(
    host: &'a dyn SourceFileMayBeEmittedHost,
    target_source_file: Option<&'a ast::SourceFile>,
    force_dts_emit: bool,
) -> Vec<&'a ast::SourceFile> {
    let source_files = if let Some(target_source_file) = target_source_file {
        vec![target_source_file]
    } else {
        host.source_files()
    };
    source_files
        .into_iter()
        .filter(|source_file| source_file_may_be_emitted(source_file, host, force_dts_emit))
        .collect()
}

fn is_source_file_not_json(file: &ast::SourceFile) -> bool {
    !ast::is_json_source_file(file)
}

pub(crate) fn get_declaration_diagnostics(
    host: &mut dyn EmitHost,
    file: &ast::SourceFile,
) -> Vec<ast::Diagnostic> {
    // TODO: use p.getSourceFilesToEmit cache
    let source_file_host: &dyn SourceFileMayBeEmittedHost = host;
    let full_files: Vec<_> = get_source_files_to_emit(source_file_host, Some(file), false)
        .into_iter()
        .filter(|f| is_source_file_not_json(f))
        .collect();
    if !full_files.iter().any(|f| std::ptr::eq(*f, file)) {
        return Vec::new();
    }
    let options = emit_host_options(host);
    let mut transform = declarations::new_declaration_transformer(host, None, &options, "", "");
    transform.transform_source_file(file);
    transform.get_diagnostics()
}

fn get_module_transformer(
    opts: &transformers::TransformOptions,
    source_file: &ast::SourceFile,
) -> transformers::Transformer {
    match opts.compiler_options.get_emit_module_kind() {
        core::ModuleKind::Preserve => {
            // `ESModuleTransformer` contains logic for preserving CJS input syntax in `--module preserve`
            let format = (opts.get_emit_module_format_of_file)(source_file);
            moduletransforms::new_es_module_transformer(opts, format)
        }

        core::ModuleKind::ESNext
        | core::ModuleKind::ES2022
        | core::ModuleKind::ES2020
        | core::ModuleKind::ES2015
        | core::ModuleKind::Node20
        | core::ModuleKind::Node18
        | core::ModuleKind::Node16
        | core::ModuleKind::NodeNext
        | core::ModuleKind::CommonJS => {
            let format = (opts.get_emit_module_format_of_file)(source_file);
            moduletransforms::new_implied_module_transformer(opts, format)
        }

        _ => moduletransforms::new_common_js_module_transformer(opts),
    }
}

fn get_script_transformers(
    emit_context: &mut printer::EmitContext,
    host: &mut dyn printer::EmitHost,
    source_file: &ast::SourceFile,
) -> Vec<transformers::Transformer> {
    let mut tx = Vec::new();
    let options = emit_host_options(host);

    // JS files don't use reference calculations as they don't do import elision, no need to calculate it
    let import_elision_enabled = !options.verbatim_module_syntax.is_true()
        && !ast::is_in_js_file(source_file.store(), source_file.as_node());
    let jsx_transform_enabled = options.get_jsx_transform_enabled()
        && (source_file.language_variant() == core::LanguageVariant::JSX
            || source_file.script_kind() == core::ScriptKind::JSX
            || source_file.script_kind() == core::ScriptKind::TSX);

    if import_elision_enabled
        || jsx_transform_enabled
        || !options.get_isolated_modules()
        || options.emit_decorator_metadata.is_true()
    {
        printer::with_emit_resolver(host, |resolver| {
            resolver.mark_linked_references_recursively(source_file);
        });
    }

    let import_elision_facts = if import_elision_enabled {
        Some(printer::with_emit_resolver(host, |resolver| {
            tstransforms::importelision::collect_import_elision_resolver_facts(
                source_file,
                resolver,
            )
        }))
    } else {
        None
    };
    let metadata_facts = if options.emit_decorator_metadata.is_true() {
        Some(printer::with_emit_resolver(host, |resolver| {
            tstransforms::metadata::collect_metadata_resolver_facts(source_file, resolver, &options)
        }))
    } else {
        None
    };
    let runtime_syntax_facts = printer::with_emit_resolver(host, |resolver| {
        tstransforms::runtimesyntax::collect_runtime_syntax_resolver_facts(
            source_file,
            emit_context,
            resolver,
        )
    });
    let legacy_decorators_facts = if options.experimental_decorators.is_true() {
        Some(printer::with_emit_resolver(host, |resolver| {
            tstransforms::legacydecorators::collect_legacy_decorators_resolver_facts(
                source_file,
                resolver,
            )
        }))
    } else {
        None
    };
    let const_enum_inlining_facts = if !options.get_isolated_modules() {
        Some(printer::with_emit_resolver(host, |resolver| {
            inliners::constenum::collect_const_enum_inlining_facts(source_file, resolver)
        }))
    } else {
        None
    };
    let jsx_facts = if jsx_transform_enabled {
        Some(printer::with_emit_resolver(host, |resolver| {
            jsxtransforms::collect_jsx_resolver_facts(source_file, resolver, &options)
        }))
    } else {
        None
    };

    let emit_binding_facts = printer::EmitHost::emit_binding_facts(host, source_file);
    let common_js_module_indicator = host.source_file_common_js_module_indicator(source_file);
    let external_module_indicator = host.source_file_external_module_indicator(source_file);
    let source_file_root = Some(emit_binding_facts.root());
    let mut module_transform_facts = printer::with_emit_resolver(host, |resolver| {
        moduletransforms::collect_module_transform_resolver_facts(source_file, resolver, &options)
    });
    module_transform_facts.common_js_module_indicator = common_js_module_indicator;
    module_transform_facts.external_module_indicator = external_module_indicator;
    module_transform_facts.source_file_root = source_file_root;
    let get_emit_module_format_of_file =
        |file: &dyn ast::HasFileName| host.get_emit_module_format_of_file(file);

    let opts = transformers::TransformOptions {
        context: emit_context,
        compiler_options: &options,
        get_emit_module_format_of_file: &get_emit_module_format_of_file,
        module_transform_facts,
        import_elision_facts,
        metadata_facts,
        runtime_syntax_facts: Some(runtime_syntax_facts),
        legacy_decorators_facts,
        const_enum_inlining_facts,
        jsx_facts,
    };

    // transform TypeScript syntax
    {
        // use type nodes to add metadata decorators
        if options.emit_decorator_metadata.is_true() {
            tx.push(tstransforms::new_metadata_transformer(&opts));
        }

        // erase types
        tx.push(tstransforms::new_type_eraser_transformer(&opts));

        // elide imports
        if import_elision_enabled {
            tx.push(tstransforms::new_import_elision_transformer(&opts));
        }

        // transform `enum`, `namespace`, and parameter properties
        tx.push(tstransforms::new_runtime_syntax_transformer(&opts));

        if options.experimental_decorators.is_true() {
            tx.push(tstransforms::new_legacy_decorators_transformer(&opts));
        }
    }

    if jsx_transform_enabled {
        tx.push(jsxtransforms::new_jsx_transformer(&opts));
    }

    if let Some(downleveler) = estransforms::get_es_transformer(&opts) {
        tx.push(downleveler);
    }

    tx.push(estransforms::new_use_strict_transformer(
        &opts,
        host.get_emit_module_format_of_file(source_file),
    ));

    // transform module syntax
    tx.push(get_module_transformer(&opts, source_file));

    // inlining (formerly done via substitutions)
    if !options.get_isolated_modules() {
        tx.push(inliners::new_const_enum_inlining_transformer(&opts));
    }
    tx
}

impl<'a> Emitter<'a> {
    fn emit_js_file(
        &mut self,
        source_file: &'a ast::SourceFile,
        js_file_path: &str,
        source_map_file_path: &str,
    ) {
        let options = emit_host_options(self.host);

        if self.emit_only != EMIT_ALL && self.emit_only != EMIT_ONLY_JS || js_file_path.is_empty() {
            return;
        }

        if options.no_emit == core::Tristate::True || self.host.is_emit_blocked(js_file_path) {
            self.emit_result.emit_skipped = true;
            return;
        }

        let pop_trace = self.tr.as_mut().map(|tr| {
            tr.push(
                tracing::Phase::Emit,
                "emitJsFileOrBundle",
                hashmap! { "jsFilePath" => js_file_path.to_string() },
                true,
            )
        });

        let (mut emit_context, put_emit_context) = printer::get_emit_context();
        emit_context.activate();
        let binding_facts = printer::EmitHost::emit_binding_facts(self.host, source_file);
        let transformed_source_file = self.run_script_transformers(&mut emit_context, source_file);
        let source_file = transformed_source_file.as_deref().unwrap_or(source_file);

        let printer_options = printer::PrinterOptions {
            remove_comments: options.remove_comments.is_true(),
            new_line: options.new_line,
            no_emit_helpers: options.no_emit_helpers.is_true(),
            source_map: options.source_map.is_true(),
            inline_source_map: options.inline_source_map.is_true(),
            inline_sources: options.inline_sources.is_true(),
            target: options.target,
            // !!!
            ..Default::default()
        };

        // create a printer to print the nodes
        let mut printer = printer::new_printer(
            printer_options,
            printer::PrintHandlers {
                // !!!
                ..Default::default()
            },
            Some(emit_context),
        );
        printer.set_binding_facts(Some(binding_facts));

        let printer = self.print_source_file(
            js_file_path,
            source_map_file_path,
            source_file,
            printer,
            should_emit_source_maps(&options, source_file),
        );
        put_emit_context(printer.into_emit_context());
        if let Some(pop_trace) = pop_trace {
            if let Some(tr) = self.tr.as_mut() {
                pop_trace(&mut **tr);
            }
        }
    }

    fn emit_declaration_file(
        &mut self,
        source_file: &'a ast::SourceFile,
        declaration_file_path: &str,
        declaration_map_path: &str,
    ) {
        let options = emit_host_options(self.host);

        if self.emit_only == EMIT_ONLY_JS || declaration_file_path.is_empty() {
            return;
        }

        if self.emit_only != EMIT_ONLY_FORCED_DTS
            && (options.no_emit == core::Tristate::True
                || self.host.is_emit_blocked(declaration_file_path))
        {
            self.emit_result.emit_skipped = true;
            return;
        }

        let pop_trace = self.tr.as_mut().map(|tr| {
            tr.push(
                tracing::Phase::Emit,
                "emitDeclarationFileOrBundle",
                hashmap! { "declarationFilePath" => declaration_file_path.to_string() },
                true,
            )
        });

        let (mut emit_context, put_emit_context) = printer::get_emit_context();
        emit_context.activate();
        let binding_facts = printer::EmitHost::emit_binding_facts(self.host, source_file);
        let (transformed_source_file, diags) = self.run_declaration_transformers(
            &mut emit_context,
            source_file,
            declaration_file_path,
            declaration_map_path,
        );
        let source_file = transformed_source_file.as_deref().unwrap_or(source_file);

        // !!! strada skipped emit if there were diagnostics

        for elem in diags {
            // Add declaration transform diagnostics to emit diagnostics
            self.emitter_diagnostics.add(elem);
        }

        let printer_options = printer::PrinterOptions {
            remove_comments: options.remove_comments.is_true(),
            new_line: options.new_line,
            no_emit_helpers: true,
            // Module: 			   options.Module, // NYI
            // ModuleResolution:   options.ModuleResolution, // NYI
            target: options.get_emit_script_target(),
            source_map: self.emit_only != EMIT_ONLY_FORCED_DTS && options.declaration_map.is_true(),
            inline_source_map: options.inline_source_map.is_true(),
            // InlineSources:       options.InlineSources.IsTrue(), // ignored, per strada
            // ExtendedDiagnostics: options.ExtendedDiagnostics.IsTrue(), // NYI
            only_print_jsdoc_style: true,
            omit_brace_source_map_positions: true,
            ..Default::default()
        };

        // create a printer to print the nodes
        let mut printer = printer::new_printer(
            printer_options,
            printer::PrintHandlers {
                // !!!
                ..Default::default()
            },
            Some(emit_context),
        );
        printer.set_binding_facts(Some(binding_facts));

        let printer = self.print_source_file(
            declaration_file_path,
            declaration_map_path,
            source_file,
            printer,
            self.emit_only != EMIT_ONLY_FORCED_DTS
                && should_emit_declaration_source_maps(&options, source_file),
        );
        put_emit_context(printer.into_emit_context());
        if let Some(pop_trace) = pop_trace {
            if let Some(tr) = self.tr.as_mut() {
                pop_trace(&mut **tr);
            }
        }
    }

    fn print_source_file(
        &mut self,
        js_file_path: &str,
        source_map_file_path: &str,
        source_file: &ast::SourceFile,
        mut printer_: printer::Printer,
        should_emit_source_maps: bool,
    ) -> printer::Printer {
        // !!! sourceMapGenerator
        let options = emit_host_options(self.host);
        let mut source_map_generator = None;
        if should_emit_source_maps {
            source_map_generator = Some(sourcemap::new_generator(
                tspath::get_base_file_name(&tspath::normalize_slashes(js_file_path)),
                get_source_root(&options),
                self.get_source_map_directory(&options, js_file_path, Some(source_file)),
                tspath::ComparePathsOptions {
                    use_case_sensitive_file_names: printer::EmitHost::use_case_sensitive_file_names(
                        self.host,
                    ),
                    current_directory: printer::EmitHost::get_current_directory(self.host),
                },
            ));
        }

        source_map_generator = printer_.write_node(
            Some(&source_file.as_node()),
            Some(source_file),
            self.writer.clone(),
            source_map_generator,
        );

        let mut source_map_url_pos = -1;
        if let Some(source_map_generator) = source_map_generator.as_mut() {
            if options.source_map.is_true()
                || options.inline_source_map.is_true()
                || options.get_are_declaration_maps_enabled()
            {
                self.emit_result.source_maps.push(SourceMapEmitResult {
                    input_source_file_names: source_map_generator.sources().to_vec(),
                    source_map: source_map_generator.raw_source_map(),
                    generated_file: js_file_path.to_string(),
                });
            }

            let source_mapping_url = self.get_source_mapping_url(
                &options,
                source_map_generator,
                js_file_path,
                source_map_file_path,
                Some(source_file),
            );

            if !source_mapping_url.is_empty() {
                if !self.writer.borrow().is_at_start_of_line() {
                    self.writer.borrow_mut().raw_write(
                        if options.new_line == core::NewLineKind::CRLF {
                            "\r\n"
                        } else {
                            "\n"
                        },
                    );
                }
                source_map_url_pos = self.writer.borrow().get_text_pos();
                self.writer
                    .borrow_mut()
                    .write_comment("//# sourceMappingURL=");
                self.writer.borrow_mut().write_comment(&source_mapping_url);
            }

            // Write the source map
            if !source_map_file_path.is_empty() {
                let source_map = source_map_generator.string();
                let err = self.write_text(source_map_file_path, &source_map, None);
                if let Err(err) = err {
                    let args = vec![js_file_path.to_string().into(), err.into()];
                    self.emitter_diagnostics.add(ast::new_compiler_diagnostic(
                        &diagnostics::Could_not_write_file_0_Colon_1,
                        &args,
                    ));
                } else {
                    self.emit_result
                        .emitted_files
                        .push(source_map_file_path.to_string());
                }
            }
        } else {
            self.writer.borrow_mut().write_line();
        }

        // Write the output file
        let mut text = self.writer.borrow().string();
        if options.emit_bom.is_true() {
            text = stringutil::add_utf8_byte_order_mark(&text);
        }
        let mut data = WriteFileData {
            source_map_url_pos,
            diagnostics: self.emitter_diagnostics.get_diagnostics(),
            ..Default::default()
        };
        let err = self.write_text(js_file_path, &text, Some(&mut data));
        let skipped_dts_write = data.skipped_dts_write;
        if let Err(err) = err {
            let args = vec![js_file_path.to_string().into(), err.into()];
            self.emitter_diagnostics.add(ast::new_compiler_diagnostic(
                &diagnostics::Could_not_write_file_0_Colon_1,
                &args,
            ));
        } else if !skipped_dts_write {
            self.emit_result
                .emitted_files
                .push(js_file_path.to_string());
        }

        // Reset state
        self.writer.borrow_mut().clear();
        printer_
    }

    fn write_text(
        &mut self,
        file_name: &str,
        text: &str,
        data: Option<&mut WriteFileData>,
    ) -> Result<(), String> {
        if let Some(write_file) = self.write_file.as_mut() {
            return (write_file.borrow_mut())(file_name, text, data);
        }
        self.host.write_file(file_name, text)
    }
}
