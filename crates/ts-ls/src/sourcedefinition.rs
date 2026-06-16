use std::collections::HashMap;

use ts_ast as ast;
use ts_astnav as astnav;
use ts_checker as checker;
use ts_collections as collections;
use ts_compiler as compiler;
use ts_core as core;
use ts_lsproto as lsproto;
use ts_module as module;
use ts_modulespecifiers as modulespecifiers;
use ts_packagejson as packagejson;
use ts_parser as parser;
use ts_tspath as tspath;
use ts_vfs as vfs;

use crate::LanguageService;
use crate::definition::{get_declarations_from_location, try_get_signature_declaration};

impl LanguageService<'_> {
    pub fn provide_source_definition(
        &self,
        ctx: &core::Context,
        document_uri: lsproto::DocumentUri,
        position: lsproto::Position,
    ) -> Result<lsproto::DefinitionResponse, core::Error> {
        let caps = lsproto::get_client_capabilities(ctx);
        let client_supports_link = caps.text_document.definition.link_support;

        let (program, file) = self.get_program_and_file(document_uri.clone());
        let pos = self
            .converters
            .line_and_character_to_position(file, position) as i32;
        let mut resolver = self.new_source_def_resolver(program, &file.file_name());
        let Some(node) = astnav::get_touching_property_name(file, pos) else {
            return Ok(lsproto::LocationOrLocationsOrDefinitionLinksOrNull::default());
        };

        if file.store().kind(node) == ast::Kind::SourceFile {
            if let Some((declarations, r#ref)) =
                resolver.resolve_triple_slash_reference(file, pos, program)
            {
                if !declarations.is_empty() {
                    let origin_selection_range =
                        self.create_lsp_range_from_bounds(r#ref.pos(), r#ref.end(), file);
                    let extra_source_files = Vec::new();
                    return Ok(self.create_definition_locations_from_nodes(
                        origin_selection_range,
                        client_supports_link,
                        declarations,
                        None,
                        &extra_source_files,
                    ));
                }
            }
            return Ok(lsproto::LocationOrLocationsOrDefinitionLinksOrNull::default());
        }

        let origin_selection_range = self.create_lsp_range_from_node(node, file);

        let containing_module_specifier = find_containing_module_specifier(file.store(), node);
        if Some(node) == containing_module_specifier {
            let specifier_mode = program.get_mode_for_usage_location(file, &node);
            if let Some(implementation_file) =
                resolver.resolve_implementation(file.store().text(node).as_str(), specifier_mode)
            {
                if let Some(source_file) = resolver.get_or_parse_source_file(&implementation_file) {
                    let declarations = get_source_definition_entry_declarations(source_file);
                    let extra_source_files = resolver.extra_source_files();
                    return Ok(self.create_definition_locations_from_nodes(
                        origin_selection_range,
                        client_supports_link,
                        declarations,
                        None,
                        &extra_source_files,
                    ));
                }
            }
            return self.provide_definition_worker(ctx, document_uri.clone(), position);
        }

        let mut resolved_impl_file = String::new();
        if let Some(containing_module_specifier) = containing_module_specifier {
            let specifier_mode =
                program.get_mode_for_usage_location(file, &containing_module_specifier);
            if let Some(found) = resolver.resolve_implementation(
                file.store().text(containing_module_specifier).as_str(),
                specifier_mode,
            ) {
                resolved_impl_file = found;
            }
        }

        if !resolved_impl_file.is_empty() {
            let names =
                get_candidate_source_declaration_names(file.store(), Some(node), None, None);
            let module_results =
                resolver.search_implementation_file(node, &resolved_impl_file, &names);
            if !module_results.is_empty() {
                if (!ast::is_part_of_type_node(file.store(), &node)
                    && !ast::is_part_of_type_only_import_or_export_declaration(file.store(), &node))
                    || resolver.has_concrete_source_declarations(&module_results)
                {
                    let extra_source_files = resolver.extra_source_files();
                    return Ok(self.create_definition_locations_from_nodes(
                        origin_selection_range,
                        client_supports_link,
                        unique_declaration_nodes(module_results),
                        None,
                        &extra_source_files,
                    ));
                }
            }
        }

        let (checker_declarations, module_specifier) =
            get_source_def_checker_info(ctx, program, file, &node)?;

        let declarations = resolver.resolve_from_checker_info(
            node,
            &resolved_impl_file,
            &checker_declarations,
            &module_specifier,
        );
        if declarations.is_empty() {
            if containing_module_specifier.is_some()
                && !resolved_impl_file.is_empty()
                && !has_concrete_source_declarations(file.store(), &checker_declarations)
            {
                if let Some(source_file) = resolver.get_or_parse_source_file(&resolved_impl_file) {
                    let declarations = get_source_definition_entry_declarations(source_file);
                    let extra_source_files = resolver.extra_source_files();
                    return Ok(self.create_definition_locations_from_nodes(
                        origin_selection_range,
                        client_supports_link,
                        declarations,
                        None,
                        &extra_source_files,
                    ));
                }
            }
            return self.provide_definition_worker(ctx, document_uri.clone(), position);
        }

        let extra_source_files = resolver.extra_source_files();
        Ok(self.create_definition_locations_from_nodes(
            origin_selection_range,
            client_supports_link,
            declarations,
            None,
            &extra_source_files,
        ))
    }

    pub(crate) fn new_source_def_resolver<'a>(
        &'a self,
        program: &'a compiler::Program,
        resolve_from: &str,
    ) -> SourceDefResolver<'a> {
        let options = program.options();
        let mut no_dts_options = options.clone();
        no_dts_options.no_dts_resolution = core::Tristate::True;
        SourceDefResolver {
            ls: self,
            program,
            fs: program.host().fs(),
            options,
            resolve_from: resolve_from.to_string(),
            parsed_files: HashMap::new(),
            parsed_files_by_store: HashMap::new(),
            resolver: module::Resolver::new(
                SourceDefResolutionHost::new(program.host()),
                no_dts_options,
                program.get_global_typings_cache_location().to_string(),
                String::new(),
            ),
        }
    }
}

// sourceDefResolver resolves source definitions by mapping .d.ts declarations
// to their implementation files (.js/.ts). It uses the NoDts module resolver
// and file parsing for resolution, but never acquires the type checker or
// the original program; all checker-dependent work is done before results
// are passed in.
pub(crate) struct SourceDefResolver<'a> {
    pub ls: &'a LanguageService<'a>,
    pub program: &'a compiler::Program,
    pub fs: &'a dyn vfs::Fs,
    pub options: &'a core::CompilerOptions,
    pub resolve_from: String,
    pub resolver: module::Resolver<SourceDefResolutionHost<'a>>,
    pub parsed_files: HashMap<String, Option<ast::SourceFile>>,
    pub parsed_files_by_store: HashMap<ast::StoreId, String>,
}

pub(crate) struct SourceDefResolutionHost<'a> {
    host: &'a dyn compiler::CompilerHost,
}

impl<'a> SourceDefResolutionHost<'a> {
    fn new(host: &'a dyn compiler::CompilerHost) -> Self {
        Self { host }
    }

    fn host(&self) -> &dyn compiler::CompilerHost {
        self.host
    }
}

impl module::ResolutionHost for SourceDefResolutionHost<'_> {
    fn get_current_directory(&self) -> String {
        self.host().get_current_directory()
    }

    fn fs(&self) -> &dyn vfs::Fs {
        self.host().fs()
    }
}

impl<'a> SourceDefResolver<'a> {
    pub(crate) fn extra_source_files(&self) -> Vec<&ast::SourceFile> {
        self.parsed_files
            .values()
            .filter_map(|source_file| source_file.as_ref())
            .collect()
    }

    fn side_parsed_source_file_for_node(&self, node: ast::Node) -> Option<&ast::SourceFile> {
        let file_name = self.parsed_files_by_store.get(&node.store_id())?;
        self.parsed_files.get(file_name)?.as_ref()
    }

    fn program_source_file_for_node(&self, node: ast::Node) -> Option<&ast::SourceFile> {
        self.program
            .get_parsed_source_files_refs()
            .into_iter()
            .find(|source_file| source_file.store().store_id() == node.store_id())
    }

    fn source_definition_file_for_node(&self, node: ast::Node) -> Option<&ast::SourceFile> {
        self.side_parsed_source_file_for_node(node)
            .or_else(|| self.program_source_file_for_node(node))
    }

    fn has_concrete_source_declarations(&self, declarations: &[ast::Node]) -> bool {
        declarations.iter().any(|declaration| {
            self.source_definition_file_for_node(*declaration)
                .is_some_and(|source_file| {
                    is_concrete_source_declaration(source_file.store(), *declaration)
                })
        })
    }

    // resolveFromCheckerInfo maps type-checker declarations to source
    // implementations. It uses only the NoDts module resolver and file parsing;
    // the type checker and original request file are not needed.
    pub(crate) fn resolve_from_checker_info(
        &mut self,
        node: ast::Node,
        resolved_impl_file: &str,
        checker_declarations: &[ast::Node],
        module_specifier: &str,
    ) -> Vec<ast::Node> {
        let mut resolved_impl_file = resolved_impl_file.to_string();
        if resolved_impl_file.is_empty() && !module_specifier.is_empty() {
            let resolve_from = self.resolve_from.clone();
            let implied = self.infer_implied_node_format(&resolve_from);
            if let Some(found) = self.resolve_implementation(module_specifier, implied) {
                resolved_impl_file = found;
            }
        }

        if checker_declarations.is_empty() && !resolved_impl_file.is_empty() {
            let Some(source_file) = self.source_definition_file_for_node(node) else {
                return Vec::new();
            };
            let names =
                get_candidate_source_declaration_names(source_file.store(), Some(node), None, None);
            let results = self.search_implementation_file(node, &resolved_impl_file, &names);
            if !results.is_empty() {
                return unique_declaration_nodes(results);
            }
        }

        let mut declarations = Vec::new();
        for declaration in checker_declarations {
            declarations.extend(self.map_declaration_to_source(
                node,
                *declaration,
                &resolved_impl_file,
            ));
        }
        let declarations = unique_declaration_nodes(declarations);
        if self.has_concrete_source_declarations(&declarations) {
            return declarations;
        }
        Vec::new()
    }

    pub(crate) fn resolve_triple_slash_reference<'b>(
        &mut self,
        file: &'b ast::SourceFile,
        pos: i32,
        program: &'b compiler::Program,
    ) -> Option<(Vec<ast::Node>, &'b ast::FileReference)> {
        let r#ref = crate::utilities::get_reference_at_position(file, pos, program)?;
        let ref_file = r#ref.file?;

        if !ref_file.is_declaration_file() {
            let reference = r#ref.reference?;
            return Some((
                get_source_definition_entry_declarations(ref_file),
                reference,
            ));
        }

        let dts_file_name = ref_file.file_name();
        let preferred_mode = self.infer_implied_node_format(&dts_file_name);
        let implementation_file =
            self.find_implementation_file_from_dts_file_name(&dts_file_name, preferred_mode)?;
        let source_file = self.get_or_parse_source_file(&implementation_file)?;
        let reference = r#ref.reference?;
        Some((
            get_source_definition_entry_declarations(&source_file),
            reference,
        ))
    }

    // searchImplementationFile searches an implementation file
    // matching the given names. Returns nil when no declarations matched; callers
    // fall through to the checker path or to the standard definition provider.
    pub(crate) fn search_implementation_file(
        &mut self,
        original_node: ast::Node,
        implementation_file: &str,
        names: &[String],
    ) -> Vec<ast::Node> {
        if implementation_file.is_empty() {
            return Vec::new();
        }
        if self.get_or_parse_source_file(implementation_file).is_none() {
            return Vec::new();
        }
        let Some(original_source_file) = self
            .source_definition_file_for_node(original_node)
            .map(ast::SourceFile::share_readonly)
        else {
            return Vec::new();
        };
        let original_store = original_source_file.store();
        if is_default_import_name(&original_store, Some(original_node)) {
            let default_declarations = self.find_declarations_in_file(
                implementation_file,
                &["default".to_string()],
                &mut collections::Set::default(),
            );
            if !default_declarations.is_empty() {
                return filter_preferred_source_declarations(
                    &original_store,
                    original_node,
                    default_declarations,
                );
            }
            return self
                .get_or_parse_source_file(implementation_file)
                .map(get_source_definition_entry_declarations)
                .unwrap_or_default();
        }
        let declarations = self.find_declarations_in_file(
            implementation_file,
            names,
            &mut collections::Set::default(),
        );
        if !declarations.is_empty() {
            return filter_preferred_source_declarations(
                &original_store,
                original_node,
                declarations,
            );
        }
        Vec::new()
    }

    pub(crate) fn map_declaration_to_source(
        &mut self,
        original_node: ast::Node,
        declaration: ast::Node,
        resolved_impl_file: &str,
    ) -> Vec<ast::Node> {
        let Some(file) = self.source_definition_file_for_node(declaration) else {
            return Vec::new();
        };
        let start_pos = get_start_pos_from_declaration(file.store(), file, declaration);
        let file_name = file.file_name();

        if let Some(mapped) = self.ls.try_get_source_position(&file_name, start_pos) {
            if let Some(source_file) = self.get_or_parse_source_file(&mapped.file_name) {
                return vec![find_closest_declaration_node(&source_file, mapped.pos)];
            }
        }

        if !tspath::is_declaration_file_name(&file_name) {
            return vec![declaration];
        }

        let mut implementation_file = resolved_impl_file.to_string();
        if implementation_file.is_empty() {
            let Some(source_file) = self.source_definition_file_for_node(declaration) else {
                return Vec::new();
            };
            let dts_file_name = source_file.file_name().to_string();
            let preferred_mode = self.infer_implied_node_format(&dts_file_name);
            implementation_file = self
                .find_implementation_file_from_dts_file_name(&dts_file_name, preferred_mode)
                .unwrap_or_default();
        }

        let Some(original_source_file) = self.source_definition_file_for_node(original_node) else {
            return Vec::new();
        };
        let declaration_store = self
            .source_definition_file_for_node(declaration)
            .map(|source_file| source_file.store());
        let names = get_candidate_source_declaration_names(
            original_source_file.store(),
            Some(original_node),
            declaration_store,
            Some(declaration),
        );
        self.search_implementation_file(original_node, &implementation_file, &names)
    }

    pub(crate) fn find_implementation_file_from_dts_file_name(
        &mut self,
        dts_file_name: &str,
        preferred_mode: core::ResolutionMode,
    ) -> Option<String> {
        if let Some(js_ext) = try_get_js_extension_for_file(dts_file_name, self.options) {
            let candidate = tspath::change_extension(dts_file_name, &js_ext);
            if self.fs.file_exists(&candidate) {
                return Some(candidate);
            }
        }

        let parts = modulespecifiers::get_node_module_path_parts(dts_file_name)?;
        if dts_file_name.rfind("/node_modules/") != Some(parts.top_level_node_modules_index) {
            return None;
        }
        if parts.package_root_index < 0 {
            return None;
        }
        let package_root_index = parts.package_root_index as usize;

        let package_name_path_part =
            &dts_file_name[parts.top_level_package_name_index + 1..package_root_index];
        let package_name = module::get_package_name_from_types_package_name(
            &module::unmangle_scoped_package_name(package_name_path_part),
        );
        if package_name.is_empty() {
            return None;
        }

        let path_to_file_in_package = &dts_file_name[package_root_index + 1..];
        if !path_to_file_in_package.is_empty() {
            let specifier = format!(
                "{}/{}",
                package_name,
                tspath::remove_file_extension(path_to_file_in_package)
            );
            if let Some(implementation_file) =
                self.resolve_implementation(&specifier, preferred_mode)
            {
                return Some(implementation_file);
            }
        }
        self.resolve_implementation(&package_name, preferred_mode)
    }

    pub(crate) fn resolve_implementation(
        &mut self,
        module_name: &str,
        preferred_mode: core::ResolutionMode,
    ) -> Option<String> {
        self.resolve_implementation_from(module_name, &self.resolve_from.clone(), preferred_mode)
    }

    pub(crate) fn resolve_implementation_from(
        &mut self,
        module_name: &str,
        resolve_from_file: &str,
        preferred_mode: core::ResolutionMode,
    ) -> Option<String> {
        let mut modes = vec![preferred_mode];
        if preferred_mode != core::ModuleKind::ESNext {
            modes.push(core::ModuleKind::ESNext);
        }
        if preferred_mode != core::ModuleKind::CommonJS {
            modes.push(core::ModuleKind::CommonJS);
        }

        for mode in modes {
            let (resolved, _) =
                self.resolver
                    .resolve_module_name(module_name, resolve_from_file, mode, None);
            if resolved.is_resolved()
                && !tspath::is_declaration_file_name(&resolved.resolved_file_name)
            {
                return Some(resolved.resolved_file_name);
            }
        }
        None
    }

    pub(crate) fn get_or_parse_source_file(&mut self, file_name: &str) -> Option<&ast::SourceFile> {
        if let Some(source_file) = self.program.get_source_file_ref(file_name) {
            return Some(source_file);
        }
        if !self.parsed_files.contains_key(file_name) {
            let mut source_file = None;
            let (text, ok) = self.ls.read_file(file_name);
            if ok {
                let parsed = parser::parse_source_file(
                    ast::SourceFileParseOptions {
                        file_name: file_name.to_string(),
                        path: self.ls.to_path(file_name),
                        ..Default::default()
                    },
                    text,
                    core::get_script_kind_from_file_name(file_name),
                );
                self.parsed_files_by_store
                    .insert(parsed.store().store_id(), file_name.to_string());
                source_file = Some(parsed);
            }
            self.parsed_files.insert(file_name.to_string(), source_file);
        }
        self.parsed_files
            .get(file_name)
            .and_then(|source_file| source_file.as_ref())
    }

    // inferImpliedNodeFormat determines the module format for a source file that may not be
    // in the program, using the file extension and nearest package.json "type" field.
    pub(crate) fn infer_implied_node_format(&mut self, file_name: &str) -> core::ResolutionMode {
        let mut package_json_type = String::new();
        let scope = self
            .resolver
            .get_package_scope_for_path(&tspath::get_directory_path(file_name));
        if let Some(scope) = scope {
            let package_json_path = tspath::combine_paths(&scope, &["package.json"]);
            let (contents, ok) = self.fs.read_file(&package_json_path);
            if ok {
                if let Ok(fields) = packagejson::parse(contents.as_bytes()) {
                    let (value, ok) = fields.header_fields.type_.get_value();
                    if ok {
                        package_json_type = value;
                    }
                }
            }
        }
        ast::get_implied_node_format_for_file(file_name, &package_json_type)
    }

    pub(crate) fn find_declarations_in_file(
        &mut self,
        file_name: &str,
        names: &[String],
        seen: &mut collections::Set<String>,
    ) -> Vec<ast::Node> {
        if file_name.is_empty() || names.is_empty() {
            return Vec::new();
        }
        if !seen.add_if_absent(file_name.to_string()) {
            return Vec::new();
        }

        let (declarations, has_concrete_declarations, source_file_name, imports) = {
            let Some(source_file) = self.get_or_parse_source_file(file_name) else {
                return Vec::new();
            };
            let declarations = find_declaration_nodes_by_name(source_file, names);
            let has_concrete_declarations = !declarations.is_empty()
                && has_concrete_source_declarations(source_file.store(), &declarations);
            let source_file_name = source_file.file_name();
            let imports = source_file
                .imports()
                .iter()
                .map(|imp| source_file.store().text(*imp))
                .collect::<Vec<_>>();
            (
                declarations,
                has_concrete_declarations,
                source_file_name,
                imports,
            )
        };
        if has_concrete_declarations {
            return declarations;
        }

        let mut forwarded = Vec::new();
        for forwarded_file in self.get_forwarded_implementation_files(&source_file_name, &imports) {
            forwarded.extend(self.find_declarations_in_file(&forwarded_file, names, seen));
        }
        if !forwarded.is_empty() {
            if self.has_concrete_source_declarations(&forwarded) {
                return unique_declaration_nodes(forwarded);
            }
            let mut combined = declarations;
            combined.extend(forwarded);
            return unique_declaration_nodes(combined);
        }
        declarations
    }

    pub(crate) fn get_forwarded_implementation_files(
        &mut self,
        source_file_name: &str,
        imports: &[String],
    ) -> Vec<String> {
        let preferred_mode = self.infer_implied_node_format(source_file_name);
        let mut files = Vec::new();
        for module_name in imports {
            if let Some(implementation_file) = self.resolve_implementation_from(
                module_name.as_str(),
                source_file_name,
                preferred_mode,
            ) {
                files.push(implementation_file);
            }
        }
        core::deduplicate(&files)
    }
}

// getSourceDefCheckerInfo acquires the type checker for the given file and
// returns the definition declarations for node along with the module specifier
// of the import that brought the symbol into scope (empty if not applicable).
pub(crate) fn get_source_def_checker_info<'a>(
    ctx: &'a core::Context,
    program: &'a compiler::Program,
    file: &'a ast::SourceFile,
    node: &'a ast::Node,
) -> Result<(Vec<ast::Node>, String), core::Error> {
    program.with_type_checker_for_file_using(compiler::CheckerAccess::context(ctx), file, |c| {
        let store = file.store();
        let mut declarations = get_declarations_from_location(c, program, *node)
            .into_iter()
            .collect::<Vec<_>>();
        let is_property_name = store.parent(*node).as_ref().is_some_and(|parent| {
            ast::is_access_expression(store, *parent) && store.name(*parent) == Some(*node)
        });
        if declarations.is_empty() && is_property_name {
            if let Some(left) = store
                .parent(*node)
                .and_then(|parent| store.expression(parent))
            {
                let ty = c.get_type_at_location(left);
                if let Some(prop) = c.get_property_of_type_public(ty, &store.text(*node)) {
                    declarations = c.collect_symbol_declarations_public(prop);
                }
            }
        }
        if let Some(called_declaration) = try_get_signature_declaration(c, program, *node) {
            let non_function_declarations = declarations
                .into_iter()
                .filter(|node| {
                    let declaration_store = c
                        .try_source_file_for_node_public(*node)
                        .map(ast::SourceFile::store)
                        .unwrap_or(store);
                    !ast::is_function_like(declaration_store, Some(*node))
                })
                .collect::<Vec<_>>();
            declarations = non_function_declarations;
            declarations.push(called_declaration);
        }

        let mut module_specifier = String::new();
        let resolve_node_storage;
        let mut resolve_node = node;
        if is_property_name {
            let mut expr = store
                .parent(*node)
                .and_then(|parent| store.expression(parent));
            while expr
                .as_ref()
                .is_some_and(|expr| ast::is_access_expression(store, *expr))
            {
                expr = expr.and_then(|expr| store.expression(expr));
            }
            if let Some(expr) = expr {
                resolve_node_storage = expr;
                resolve_node = &resolve_node_storage;
            }
        }
        if let Some(sym) = c.get_symbol_at_location_public(*resolve_node) {
            let symbol_declarations = c.collect_symbol_declarations_public(sym);
            for d in &symbol_declarations {
                let Some(d_store) = c
                    .try_source_file_for_node_public(*d)
                    .map(ast::SourceFile::store)
                else {
                    continue;
                };
                if !ast::is_import_specifier(d_store, *d)
                    && !ast::is_import_clause(d_store, *d)
                    && !ast::is_namespace_import(d_store, *d)
                    && !ast::is_import_equals_declaration(d_store, *d)
                {
                    continue;
                }
                if let Some(spec) =
                    checker::try_get_module_specifier_from_declaration(d_store, Some(*d))
                {
                    if let Some(spec_store) = c
                        .try_source_file_for_node_public(spec)
                        .map(ast::SourceFile::store)
                    {
                        module_specifier = spec_store.text(spec);
                    }
                    break;
                }
            }
        }
        Ok((declarations, module_specifier))
    })
}

pub(crate) fn find_containing_module_specifier(
    store: &ast::AstStore,
    node: ast::Node,
) -> Option<ast::Node> {
    let mut current = Some(node);
    while let Some(n) = current {
        if ast::is_any_import_or_re_export(store, n)
            || ast::is_require_call(store, n, true)
            || ast::is_import_call(store, n)
        {
            if let Some(module_specifier) = ast::get_external_module_name(store, n) {
                if ast::is_string_literal_like(store, module_specifier) {
                    return Some(module_specifier);
                }
            }
        }
        current = store.parent(n);
    }
    None
}

pub(crate) fn is_default_import_name(store: &ast::AstStore, node: Option<ast::Node>) -> bool {
    let Some(node) = node else {
        return false;
    };
    let Some(parent) = store.parent(node) else {
        return false;
    };
    if !ast::is_import_clause(store, parent)
        || store.name(parent) != Some(node)
        || store.parent(parent).is_none()
    {
        return false;
    }
    ast::is_default_import(store, &store.parent(parent).unwrap())
}

pub(crate) fn get_source_definition_entry_node(source_file: &ast::SourceFile) -> ast::Node {
    let statements = source_file.statements_view();
    if let Some(statement) = statements.first() {
        return statement;
    }
    source_file.as_node()
}

pub(crate) fn get_source_definition_entry_declarations(
    source_file: &ast::SourceFile,
) -> Vec<ast::Node> {
    vec![get_source_definition_entry_node(source_file)]
}

pub(crate) fn try_get_js_extension_for_file(
    dts_file_name: &str,
    options: &core::CompilerOptions,
) -> Option<String> {
    let ext = module::try_get_js_extension_for_file(
        dts_file_name,
        options.jsx == core::JsxEmit::Preserve,
    );
    if ext.is_empty() { None } else { Some(ext) }
}

pub(crate) fn get_candidate_source_declaration_names(
    original_store: &ast::AstStore,
    original_node: Option<ast::Node>,
    declaration_store: Option<&ast::AstStore>,
    declaration: Option<ast::Node>,
) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(declaration) = declaration {
        let declaration_store = declaration_store.unwrap_or(original_store);
        if let Some(name) = ast::get_name_of_declaration(declaration_store, Some(declaration)) {
            let text = ast::get_text_of_property_name(declaration_store, &name);
            if !text.is_empty() {
                names.push(text);
            }
        }
        if declaration_store.kind(declaration) == ast::Kind::ExportAssignment {
            names.push("default".to_string());
        }
        if (ast::is_function_declaration(declaration_store, declaration)
            || ast::is_class_declaration(declaration_store, declaration))
            && ast::get_combined_modifier_flags(declaration_store, declaration)
                .intersects(ast::MODIFIER_FLAGS_EXPORT_DEFAULT)
        {
            names.push("default".to_string());
        }
        if ast::is_import_specifier(declaration_store, declaration)
            || ast::is_export_specifier(declaration_store, declaration)
        {
            if let Some(prop_name) = declaration_store.property_name(declaration) {
                names.push(declaration_store.text(prop_name));
            }
        }
    }
    if let Some(original_node) = original_node {
        if ast::is_identifier(original_store, original_node)
            || ast::is_private_identifier(original_store, original_node)
        {
            names.push(original_store.text(original_node));
        }
        if is_default_import_name(original_store, Some(original_node)) {
            names.push("default".to_string());
        }
        if let Some(parent) = original_store.parent(original_node) {
            if ast::is_import_specifier(original_store, parent)
                || ast::is_export_specifier(original_store, parent)
            {
                if let Some(prop_name) = original_store.property_name(parent) {
                    names.push(original_store.text(prop_name));
                }
            }
        }
    }
    names
}

pub(crate) fn find_declaration_nodes_by_name(
    source_file: &ast::SourceFile,
    names: &[String],
) -> Vec<ast::Node> {
    let store = source_file.store();
    let filtered_names = names
        .iter()
        .filter(|name| !name.is_empty())
        .cloned()
        .collect::<Vec<_>>();
    let names = core::deduplicate(&filtered_names);
    if names.is_empty() {
        return Vec::new();
    }

    let mut wanted: collections::Set<String> = collections::Set::default();
    let mut want_default = false;
    for name in names {
        if name == "default" {
            want_default = true;
            continue;
        }
        wanted.add(name);
    }

    struct Candidate {
        node: ast::Node,
        depth: i32,
    }

    let mut candidates = Vec::new();
    let mut min_depth = i32::MAX;

    fn visit_declaration_node_by_name(
        store: &ast::AstStore,
        node: ast::Node,
        wanted: &collections::Set<String>,
        want_default: bool,
        candidates: &mut Vec<Candidate>,
        min_depth: &mut i32,
    ) {
        let mut matched = false;
        if let Some(name) = ast::get_name_of_declaration(store, Some(node)) {
            let text = ast::get_text_of_property_name(store, &name);
            if !text.is_empty() && wanted.has(&text) {
                matched = true;
            }
        }
        if want_default && store.kind(node) == ast::Kind::ExportAssignment {
            matched = true;
        }
        if want_default
            && (ast::is_function_declaration(store, node) || ast::is_class_declaration(store, node))
            && ast::get_combined_modifier_flags(store, node)
                .intersects(ast::MODIFIER_FLAGS_EXPORT_DEFAULT)
        {
            matched = true;
        }
        if matched {
            let depth = get_container_depth(store, node);
            candidates.push(Candidate { node, depth });
            if depth < *min_depth {
                *min_depth = depth;
            }
        }
        let _ = store.for_each_present_child(node, |child| {
            visit_declaration_node_by_name(
                store,
                child,
                wanted,
                want_default,
                candidates,
                min_depth,
            );
            std::ops::ControlFlow::Continue(())
        });
    }
    let source_file_node = source_file.as_node();
    let _ = store.for_each_present_child(source_file_node, |child| {
        visit_declaration_node_by_name(
            store,
            child,
            &wanted,
            want_default,
            &mut candidates,
            &mut min_depth,
        );
        std::ops::ControlFlow::Continue(())
    });

    unique_declaration_nodes(
        candidates
            .into_iter()
            .filter(|c| c.depth == min_depth)
            .map(|c| c.node)
            .collect(),
    )
}

// getContainerDepth counts the number of container nodes above a declaration,
// matching the behavior of getDepth in getTopMostDeclarationNamesInFile.
pub(crate) fn get_container_depth(store: &ast::AstStore, node: ast::Node) -> i32 {
    let mut depth = 0;
    let mut current = Some(node);
    while let Some(node) = current {
        current = crate::utilities::get_container_node(store, node);
        depth += 1;
    }
    depth
}

pub(crate) fn filter_preferred_source_declarations(
    original_store: &ast::AstStore,
    original_node: ast::Node,
    declarations: Vec<ast::Node>,
) -> Vec<ast::Node> {
    if declarations.len() <= 1 {
        return declarations;
    }
    if let preferred @ true =
        !get_property_like_source_declarations(original_store, original_node, &declarations)
            .is_empty()
    {
        let _ = preferred;
        return get_property_like_source_declarations(original_store, original_node, &declarations);
    }
    let preferred = declarations
        .iter()
        .copied()
        .filter(|node| is_concrete_source_declaration(original_store, *node))
        .collect::<Vec<_>>();
    if !preferred.is_empty() {
        return preferred;
    }
    declarations
}

pub(crate) fn get_property_like_source_declarations(
    original_store: &ast::AstStore,
    original_node: ast::Node,
    declarations: &[ast::Node],
) -> Vec<ast::Node> {
    if original_store.parent(original_node).is_none_or(|parent| {
        !ast::is_access_expression(original_store, parent)
            || original_store.name(parent) != Some(original_node)
    }) {
        return Vec::new();
    }
    declarations
        .iter()
        .copied()
        .filter(|node| {
            matches!(
                original_store.kind(*node),
                ast::Kind::PropertyAssignment
                    | ast::Kind::ShorthandPropertyAssignment
                    | ast::Kind::PropertyDeclaration
                    | ast::Kind::PropertySignature
                    | ast::Kind::MethodDeclaration
                    | ast::Kind::MethodSignature
                    | ast::Kind::GetAccessor
                    | ast::Kind::SetAccessor
                    | ast::Kind::EnumMember
            )
        })
        .collect()
}

pub(crate) fn has_concrete_source_declarations(
    store: &ast::AstStore,
    declarations: &[impl std::borrow::Borrow<ast::Node>],
) -> bool {
    declarations
        .iter()
        .any(|node| is_concrete_source_declaration(store, *std::borrow::Borrow::borrow(node)))
}

pub(crate) fn is_concrete_source_declaration(store: &ast::AstStore, node: ast::Node) -> bool {
    let kind = store.kind(node);
    if kind == ast::Kind::ExportAssignment {
        return false;
    }
    matches!(
        kind,
        ast::Kind::ClassDeclaration
            | ast::Kind::ClassExpression
            | ast::Kind::FunctionDeclaration
            | ast::Kind::FunctionExpression
            | ast::Kind::VariableDeclaration
            | ast::Kind::PropertyAssignment
            | ast::Kind::ShorthandPropertyAssignment
            | ast::Kind::PropertyDeclaration
            | ast::Kind::PropertySignature
            | ast::Kind::MethodDeclaration
            | ast::Kind::MethodSignature
            | ast::Kind::GetAccessor
            | ast::Kind::SetAccessor
            | ast::Kind::EnumMember
            | ast::Kind::InterfaceDeclaration
            | ast::Kind::TypeAliasDeclaration
    )
}

pub(crate) fn unique_declaration_nodes(nodes: Vec<ast::Node>) -> Vec<ast::Node> {
    let mut seen: collections::Set<ast::Node> = collections::Set::default();
    let mut result = Vec::new();
    for node in nodes {
        if !seen.add_if_absent(node) {
            continue;
        }
        result.push(node);
    }
    result
}

pub(crate) fn find_closest_declaration_node(source_file: &ast::SourceFile, pos: i32) -> ast::Node {
    let mut current = astnav::get_touching_property_name(source_file, pos);
    let store = source_file.store();
    while let Some(node) = current {
        if ast::is_declaration(store, node) || store.kind(node) == ast::Kind::ExportAssignment {
            return node;
        }
        current = node
            .store_id()
            .eq(&source_file.store().store_id())
            .then(|| store.parent(node))
            .flatten();
    }
    get_source_definition_entry_node(source_file)
}

pub(crate) fn get_start_pos_from_declaration(
    store: &ast::AstStore,
    file: &ast::SourceFile,
    declaration: ast::Node,
) -> core::TextPos {
    let name = ast::get_name_of_declaration(store, Some(declaration)).unwrap_or(declaration);
    astnav::get_start_of_node(name, file)
}
