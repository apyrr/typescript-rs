use ts_ast as ast;
use ts_checker as checker;
use ts_collections::{FastHashMap as HashMap, FastHashMapExt};
use ts_compiler as compiler;
use ts_core as core;
use ts_debug as debug;
use ts_locale as locale;
use ts_lsproto as lsproto;
use ts_nodebuilder as nodebuilder;
use ts_printer as printer;

use crate::autoimport::{
    Fix, NewImportBinding, View, add_namespace_qualifier, add_to_existing_import,
    get_add_to_existing_import_fix, make_new_import_text_from_bindings, symbol_identity_to_export,
};
use crate::change;
use crate::lsconv;
use crate::lsutil;

pub struct AutoImportableReferenceTypeNode {
    pub type_node: ast::Node,
    pub(crate) symbols: Vec<ast::SymbolIdentity>,
    pub converted: bool,
}

// addToExistingState tracks modifications to an existing import clause or binding pattern
#[derive(Clone, Debug, Default)]
pub struct AddToExistingState {
    pub import_clause_or_binding_pattern: Option<ast::Node>,
    pub default_import: Option<NewImportBinding>,
    pub named_imports: HashMap<String, NewImportBinding>,
}

// importsCollection tracks new imports to be created for a given module specifier
#[derive(Clone, Debug, Default)]
pub struct ImportsCollection {
    pub default_import: Option<NewImportBinding>,
    pub named_imports: HashMap<String, NewImportBinding>,
    pub namespace_like_import: Option<NewImportBinding>,
    pub use_require: bool,
}

pub fn new_imports_key(module_specifier: &str, top_level_type_only: bool) -> String {
    if top_level_type_only {
        return format!("1|{module_specifier}");
    }
    format!("0|{module_specifier}")
}

pub struct ImportAdder<'a> {
    // Context
    pub ctx: core::Context,
    pub checker_present: bool,
    pub view: Option<View<'a>>,
    pub format_options: Option<lsutil::FormatCodeSettings>,
    pub converters: &'a lsconv::Converters,
    pub preferences: lsutil::UserPreferences,

    // State
    pub add_to_namespace: Vec<Fix>, // Namespace fixes don't conflict, so just build a list
    pub add_to_existing: HashMap<usize, AddToExistingState>, // importClauseOrBindingPattern -> default or named bindings
    pub new_imports: HashMap<String, ImportsCollection>, // module specifier + type only -> imports
    pub edits_cache: Vec<lsproto::TextEdit>,
}

impl<'a> ImportAdder<'a> {
    pub fn new(
        ctx: &core::Context,
        _program: &compiler::Program,
        _file: &ast::SourceFile,
        view: View<'a>,
        format_options: lsutil::FormatCodeSettings,
        converters: &'a lsconv::Converters,
        preferences: lsutil::UserPreferences,
    ) -> Self {
        Self {
            ctx: ctx.clone(),
            checker_present: true,
            view: Some(view),
            format_options: Some(format_options),
            converters,
            preferences,
            add_to_namespace: Vec::new(),
            add_to_existing: HashMap::new(),
            new_imports: HashMap::new(),
            edits_cache: Vec::new(),
        }
    }

    pub fn has_fixes(&self) -> bool {
        !self.add_to_namespace.is_empty()
            || !self.add_to_existing.is_empty()
            || !self.new_imports.is_empty()
    }

    pub(crate) fn add_import_from_exported_symbol(
        &mut self,
        checker: &mut checker::Checker<'a, '_>,
        exported_symbol: ast::SymbolIdentity,
        is_valid_type_only_use_site: bool,
    ) -> Result<(), core::Error> {
        let Some(skipped) = checker.skip_alias_public(exported_symbol) else {
            return Ok(());
        };
        let Some(symbol) = checker.get_merged_symbol_public(skipped) else {
            return Ok(());
        };
        let export_infos = self.get_all_exports_for_symbol(checker, symbol);
        if export_infos.is_empty() {
            // If no exportInfo is found, this means export could not be resolved when we have filtered for autoImportFileExcludePatterns,
            //     so we should not generate an import.
            // debug.Assert(len(adder.ls.UserPreferences().AutoImportFileExcludePatterns) > 0)
            return Ok(());
        }
        let Some(view) = self.view.as_ref() else {
            return Ok(());
        };
        let Some(file) = view.importing_file.as_ref() else {
            return Ok(());
        };
        if let Some(fix) = self.get_import_fix_for_symbol(
            checker,
            view,
            file,
            &export_infos,
            is_valid_type_only_use_site,
        )? {
            self.add_import_fix(&fix);
        }
        Ok(())
    }

    pub fn edits(&self) -> Vec<lsproto::TextEdit> {
        let Some(view) = self.view.as_ref() else {
            return self.edits_cache.clone();
        };
        let Some(importing_file) = view.importing_file.as_ref() else {
            return self.edits_cache.clone();
        };
        let Some(program) = view.program else {
            return self.edits_cache.clone();
        };
        let Some(format_options) = self.format_options.clone() else {
            return self.edits_cache.clone();
        };
        let mut tracker = change::new_tracker(
            self.ctx.clone(),
            program.compiler_options(),
            format_options,
            self.converters,
        );
        let quote_preference = lsutil::get_quote_preference(importing_file, &self.preferences);

        for fix in &self.add_to_namespace {
            add_namespace_qualifier(fix, &mut tracker, importing_file, locale::und());
        }
        for entry in self.add_to_existing.values() {
            if let Some(import_clause_or_binding_pattern) =
                entry.import_clause_or_binding_pattern.as_ref()
            {
                add_to_existing_import(
                    &mut tracker,
                    importing_file,
                    import_clause_or_binding_pattern,
                    entry.default_import.as_ref(),
                    &sorted_named_imports(&entry.named_imports),
                    self.preferences.clone(),
                );
            }
        }

        let mut new_import_texts = Vec::new();
        let mut new_import_keys = self.new_imports.keys().collect::<Vec<_>>();
        new_import_keys.sort();
        for key in new_import_keys {
            let new_import = &self.new_imports[key];
            let module_specifier = &key[2..]; // From `${0 | 1}|${moduleSpecifier}` format
            new_import_texts.extend(make_new_import_text_from_bindings(
                module_specifier,
                quote_preference,
                new_import.use_require,
                new_import.default_import.as_ref(),
                &sorted_named_imports(&new_import.named_imports),
                new_import.namespace_like_import.as_ref(),
                program.compiler_options(),
                self.preferences.clone(),
            ));
        }
        if !new_import_texts.is_empty() {
            let new_line = tracker.new_line.clone();
            let text = new_import_texts.join(&new_line) + &new_line;
            let position = tracker
                .converters
                .position_to_line_and_character(importing_file, 0);
            tracker.insert_text(importing_file, position, &text);
        }
        tracker
            .get_changes()
            .remove(&importing_file.file_name())
            .unwrap_or_default()
    }

    // AddImportFix adds a fix to the import adder, accumulating it with other fixes
    // so that multiple imports from the same module are coalesced into a single import statement.
    pub fn add_import_fix(&mut self, fix: &Fix) {
        let symbol_name = fix
            .auto_import_fix
            .as_ref()
            .map(|f| f.name.clone())
            .unwrap_or_default();
        let Some(view) = self.view.as_ref() else {
            return;
        };
        let Some(program) = view.program else {
            return;
        };
        let compiler_options = program.compiler_options();

        match fix.kind() {
            Some(lsproto::AutoImportFixKind::UseNamespace) => {
                self.add_to_namespace.push(fix.clone());
            }
            Some(lsproto::AutoImportFixKind::AddToExisting) => {
                let Some(importing_file) = view.importing_file.as_ref() else {
                    return;
                };
                let Some(existing_fix) = get_add_to_existing_import_fix(importing_file, fix) else {
                    return;
                };
                let Some(import_clause_or_binding_pattern) =
                    existing_fix.import_clause_or_binding_pattern.as_ref()
                else {
                    return;
                };
                let key =
                    ast::get_node_id(importing_file.store(), *import_clause_or_binding_pattern)
                        as usize;
                let entry = self
                    .add_to_existing
                    .entry(key)
                    .or_insert_with(|| AddToExistingState {
                        import_clause_or_binding_pattern: Some(*import_clause_or_binding_pattern),
                        default_import: None,
                        named_imports: HashMap::new(),
                    });

                if fix.import_kind() == Some(lsproto::ImportKind::Named) {
                    let prev_type_only = entry
                        .named_imports
                        .get(&symbol_name)
                        .and_then(|import| import.add_as_type_only)
                        .unwrap_or(lsproto::AddAsTypeOnly::Allowed);
                    entry.named_imports.insert(
                        symbol_name.clone(),
                        NewImportBinding {
                            kind: Some(lsproto::ImportKind::Named),
                            name: symbol_name,
                            add_as_type_only: Some(reduce_add_as_type_only_values(
                                prev_type_only,
                                fix.auto_import_fix
                                    .as_ref()
                                    .map(|f| f.add_as_type_only)
                                    .unwrap_or(lsproto::AddAsTypeOnly::Allowed),
                            )),
                            property_name: existing_fix
                                .named_import
                                .as_ref()
                                .map(|import| import.property_name.clone())
                                .unwrap_or_default(),
                        },
                    );
                } else {
                    let prev_type_only = entry
                        .default_import
                        .as_ref()
                        .and_then(|import| import.add_as_type_only)
                        .unwrap_or(lsproto::AddAsTypeOnly::Allowed);
                    entry.default_import = Some(NewImportBinding {
                        kind: Some(lsproto::ImportKind::Default),
                        name: symbol_name,
                        add_as_type_only: Some(reduce_add_as_type_only_values(
                            prev_type_only,
                            fix.auto_import_fix
                                .as_ref()
                                .map(|f| f.add_as_type_only)
                                .unwrap_or(lsproto::AddAsTypeOnly::Allowed),
                        )),
                        property_name: String::new(),
                    });
                }
            }
            Some(lsproto::AutoImportFixKind::AddNew) => {
                let module_specifier = fix
                    .auto_import_fix
                    .as_ref()
                    .map(|f| f.module_specifier.clone())
                    .unwrap_or_default();
                let import_kind = fix.import_kind().unwrap_or(lsproto::ImportKind::Named);
                let use_require = fix
                    .auto_import_fix
                    .as_ref()
                    .map(|f| f.use_require)
                    .unwrap_or(false);
                let add_as_type_only = fix
                    .auto_import_fix
                    .as_ref()
                    .map(|f| f.add_as_type_only)
                    .unwrap_or(lsproto::AddAsTypeOnly::Allowed);
                let entry = self.get_new_import_entry(
                    &module_specifier,
                    import_kind,
                    use_require,
                    add_as_type_only,
                );

                match import_kind {
                    lsproto::ImportKind::Default => {
                        let prev = entry
                            .default_import
                            .as_ref()
                            .and_then(|i| i.add_as_type_only)
                            .unwrap_or(lsproto::AddAsTypeOnly::Allowed);
                        entry.default_import = Some(NewImportBinding {
                            kind: Some(lsproto::ImportKind::Default),
                            property_name: String::new(),
                            name: symbol_name,
                            add_as_type_only: Some(reduce_add_as_type_only_values(
                                prev,
                                add_as_type_only,
                            )),
                        });
                    }
                    lsproto::ImportKind::Named => {
                        let prev = entry
                            .named_imports
                            .get(&symbol_name)
                            .and_then(|i| i.add_as_type_only)
                            .unwrap_or(lsproto::AddAsTypeOnly::Allowed);
                        entry.named_imports.insert(
                            symbol_name.clone(),
                            NewImportBinding {
                                kind: Some(lsproto::ImportKind::Named),
                                property_name: String::new(),
                                name: symbol_name,
                                add_as_type_only: Some(reduce_add_as_type_only_values(
                                    prev,
                                    add_as_type_only,
                                )),
                            },
                        );
                    }
                    lsproto::ImportKind::CommonJS => {
                        if compiler_options.verbatim_module_syntax == core::TS_TRUE {
                            let prev = entry
                                .named_imports
                                .get(&symbol_name)
                                .and_then(|i| i.add_as_type_only)
                                .unwrap_or(lsproto::AddAsTypeOnly::Allowed);
                            entry.named_imports.insert(
                                symbol_name.clone(),
                                NewImportBinding {
                                    kind: Some(lsproto::ImportKind::CommonJS),
                                    property_name: String::new(),
                                    name: symbol_name,
                                    add_as_type_only: Some(reduce_add_as_type_only_values(
                                        prev,
                                        add_as_type_only,
                                    )),
                                },
                            );
                        } else {
                            entry.namespace_like_import = Some(NewImportBinding {
                                kind: Some(lsproto::ImportKind::CommonJS),
                                property_name: String::new(),
                                name: symbol_name,
                                add_as_type_only: Some(add_as_type_only),
                            });
                        }
                    }
                    lsproto::ImportKind::Namespace => {
                        entry.namespace_like_import = Some(NewImportBinding {
                            kind: Some(lsproto::ImportKind::Namespace),
                            property_name: String::new(),
                            name: symbol_name,
                            add_as_type_only: Some(add_as_type_only),
                        });
                    }
                    _ => {
                        debug::fail(&format!("Unexpected import kind: {:?}", import_kind));
                    }
                }
            }
            Some(lsproto::AutoImportFixKind::PromoteTypeOnly) => {}
            _ => {
                debug::fail(&format!("Unexpected fix kind: {:?}", fix.kind()));
            }
        }
    }

    pub fn get_new_import_entry(
        &mut self,
        module_specifier: &str,
        import_kind: lsproto::ImportKind,
        use_require: bool,
        add_as_type_only: lsproto::AddAsTypeOnly,
    ) -> &mut ImportsCollection {
        let type_only_key = new_imports_key(module_specifier, true);
        let non_type_only_key = new_imports_key(module_specifier, false);

        let has_type_only = self.new_imports.contains_key(&type_only_key);
        let has_non_type_only = self.new_imports.contains_key(&non_type_only_key);

        // A default import that requires type-only makes the whole import type-only.
        if import_kind == lsproto::ImportKind::Default
            && add_as_type_only == lsproto::AddAsTypeOnly::Required
        {
            return self
                .new_imports
                .entry(type_only_key)
                .or_insert_with(|| ImportsCollection {
                    use_require,
                    ..Default::default()
                });
        }

        if add_as_type_only == lsproto::AddAsTypeOnly::Allowed
            && (has_type_only || has_non_type_only)
        {
            if has_type_only {
                return self.new_imports.get_mut(&type_only_key).unwrap();
            }
            return self.new_imports.get_mut(&non_type_only_key).unwrap();
        }

        if has_non_type_only {
            return self.new_imports.get_mut(&non_type_only_key).unwrap();
        }

        self.new_imports
            .entry(non_type_only_key)
            .or_insert_with(|| ImportsCollection {
                use_require,
                ..Default::default()
            })
    }

    pub(crate) fn get_all_exports_for_symbol(
        &mut self,
        checker: &mut checker::Checker<'a, '_>,
        symbol: ast::SymbolIdentity,
    ) -> Vec<crate::autoimport::Export> {
        let Some(export) = symbol_identity_to_export(symbol, checker) else {
            return Vec::new();
        };
        let Some(view) = self.view.as_ref() else {
            return Vec::new();
        };
        view.search_by_export_id(export.export_id)
    }

    pub fn get_import_fix_for_symbol(
        &self,
        checker: &mut checker::Checker<'a, '_>,
        view: &View<'_>,
        _file: &ast::SourceFile,
        exports: &[crate::autoimport::Export],
        is_valid_type_only_use_site: bool,
    ) -> Result<Option<Fix>, core::Error> {
        let mut fixes = Vec::new();
        for export in exports {
            fixes.extend(view.get_fixes(
                checker,
                export,
                false, /*forJSX*/
                is_valid_type_only_use_site,
                None, /*usagePosition*/
            ));
        }
        fixes.sort_by(|a, b| view.compare_fixes_for_ranking(a, b).cmp(&0));
        Ok(fixes.into_iter().next())
    }
}

pub fn new_import_adder<'a>(
    ctx: &core::Context,
    program: &compiler::Program,
    source_file: &ast::SourceFile,
    view: View<'a>,
    format_options: lsutil::FormatCodeSettings,
    converters: &'a lsconv::Converters,
    preferences: lsutil::UserPreferences,
) -> ImportAdder<'a> {
    ImportAdder::new(
        ctx,
        program,
        source_file,
        view,
        format_options,
        converters,
        preferences,
    )
}

// `NotAllowed` overrides `Required` because one addition of a new import might be required to be type-only
// because of `--importsNotUsedAsValues=error`, but if a second addition of the same import is `NotAllowed`
// to be type-only, the reason the first one was `Required` - the unused runtime dependency - is now moot.
// Alternatively, if one addition is `Required` because it has no value meaning under `--preserveValueImports`
// and `--isolatedModules`, it should be impossible for another addition to be `NotAllowed` since that would
// mean a type is being referenced in a value location.
pub fn reduce_add_as_type_only_values(
    prev_value: lsproto::AddAsTypeOnly,
    new_value: lsproto::AddAsTypeOnly,
) -> lsproto::AddAsTypeOnly {
    if new_value > prev_value {
        return new_value;
    }
    prev_value
}

pub fn sorted_named_imports(m: &HashMap<String, NewImportBinding>) -> Vec<NewImportBinding> {
    let mut keys: Vec<_> = m.keys().cloned().collect();
    keys.sort();
    let mut result = Vec::with_capacity(keys.len());
    for key in keys {
        result.push(m.get(&key).unwrap().clone());
    }
    result
}

pub fn type_to_auto_importable_type_node<'a>(
    checker: &mut checker::Checker<'a, '_>,
    import_adder: Option<&mut ImportAdder<'a>>,
    t: checker::TypeHandle,
    context_node: &'a ast::Node,
    source: &ast::AstStore,
    factory: &mut ast::NodeFactory,
) -> Result<Option<ast::Node>, core::Error> {
    let (mut emit_context, done) = printer::get_emit_context();
    let (type_node, id_to_symbol) = checker.type_to_type_node_for_ls_public(
        &mut emit_context,
        t,
        Some(*context_node),
        nodebuilder::FLAGS_NONE,
        nodebuilder::INTERNAL_FLAGS_NONE,
    );
    done(emit_context);
    let Some(type_node) = type_node else {
        return Ok(None);
    };
    Ok(Some(type_node_to_auto_importable_type_node(
        checker,
        source,
        factory,
        &type_node,
        import_adder,
        id_to_symbol,
    )?))
}

// TypeNodeToAutoImportableTypeNode converts import type references in a type node to
// simple type references and registers needed imports with the import adder.
pub fn type_node_to_auto_importable_type_node<'a>(
    checker: &mut checker::Checker<'a, '_>,
    source: &ast::AstStore,
    factory: &mut ast::NodeFactory,
    type_node: &ast::Node,
    import_adder: Option<&mut ImportAdder<'a>>,
    id_to_symbol: HashMap<ast::Node, ast::SymbolIdentity>,
) -> Result<ast::Node, core::Error> {
    let id_to_symbol = id_to_symbol
        .into_iter()
        .filter_map(|(identifier, symbol)| {
            let name = get_name_for_exported_symbol(source, checker, symbol, false);
            if name.is_empty() {
                return None;
            }
            Some((identifier, (symbol, name)))
        })
        .collect();
    let result =
        try_get_auto_importable_reference_from_type_node(source, factory, type_node, id_to_symbol);
    if let Some(mut import_adder) = import_adder {
        import_symbols(&mut import_adder, checker, &result.symbols)?;
    }
    Ok(result.type_node)
}

pub(crate) fn import_symbols<'a>(
    import_adder: &mut ImportAdder<'a>,
    checker: &mut checker::Checker<'a, '_>,
    symbols: &[ast::SymbolIdentity],
) -> Result<(), core::Error> {
    for symbol in symbols {
        import_adder.add_import_from_exported_symbol(checker, *symbol, true)?;
    }
    Ok(())
}

struct AutoImportableReferenceTraversal<'source, 'factory> {
    source: &'source ast::AstStore,
    factory: &'factory mut ast::NodeFactory,
    id_to_symbol: HashMap<ast::Node, (ast::SymbolIdentity, String)>,
    symbols: Vec<ast::SymbolIdentity>,
    converted: bool,
    traversal: ast::AstImportState,
}

impl<'source, 'factory> AutoImportableReferenceTraversal<'source, 'factory> {
    fn new(
        source: &'source ast::AstStore,
        factory: &'factory mut ast::NodeFactory,
        id_to_symbol: HashMap<ast::Node, (ast::SymbolIdentity, String)>,
    ) -> Self {
        Self {
            source,
            factory,
            id_to_symbol,
            symbols: Vec::new(),
            converted: false,
            traversal: ast::AstImportState::new(),
        }
    }

    fn finish(
        mut self,
        type_node: Option<ast::Node>,
        original: ast::Node,
    ) -> AutoImportableReferenceTypeNode {
        let type_node = type_node.unwrap_or_else(|| self.clone_node_to_factory(original));
        debug::assert(
            ast::is_type_node(self.store_for(type_node), type_node),
            Some("expected a type node".to_string()),
        );
        AutoImportableReferenceTypeNode {
            type_node,
            symbols: self.symbols,
            converted: self.converted,
        }
    }

    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        ast::AstImportState::store_for(self.source, self.factory, node)
    }

    fn preserved_node(&self, source: &ast::Node) -> Option<ast::Node> {
        self.traversal.preserved_node(self.factory, *source)
    }

    fn record_preserved_node(&mut self, source: ast::Node, imported: ast::Node) -> ast::Node {
        self.traversal
            .record_preserved_node(self.source.store_id(), self.factory, source, imported)
    }

    fn preserve_node(&mut self, node: ast::Node) -> ast::Node {
        self.traversal
            .preserve_node(self.source, self.factory, node)
    }

    fn clone_node_to_factory(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.factory.store().store_id() {
            return node;
        }
        self.factory.deep_clone_node_from_store(self.source, node)
    }

    fn preserve_source_node_list(&mut self, list: ast::SourceNodeList<'_>) -> ast::NodeList {
        self.traversal.preserve_source_node_list(self.factory, list)
    }

    fn preserve_source_modifier_list(
        &mut self,
        modifiers: ast::SourceModifierList<'_>,
    ) -> ast::ModifierList {
        self.traversal
            .preserve_source_modifier_list(self.factory, modifiers)
    }

    fn preserve_source_raw_node_slice(
        &mut self,
        nodes: ast::SourceRawNodeSlice<'_>,
    ) -> ast::RawNodeSlice {
        self.traversal
            .preserve_source_raw_node_slice(self.factory, nodes)
    }

    fn preserved_source_node_matches(
        &self,
        source: Option<ast::Node>,
        output: Option<ast::Node>,
    ) -> bool {
        self.traversal
            .preserved_source_node_matches(self.factory, source, output)
    }

    fn preserved_source_node_list_matches(
        &self,
        source: Option<ast::SourceNodeList<'_>>,
        output: Option<ast::NodeList>,
    ) -> bool {
        self.traversal
            .preserved_source_node_list_view_matches(self.factory, source, output)
    }

    fn preserved_source_modifier_list_matches(
        &self,
        source: Option<ast::SourceModifierList<'_>>,
        output: Option<ast::ModifierList>,
    ) -> bool {
        self.traversal
            .preserved_source_modifier_list_view_matches(self.factory, source, output)
    }

    fn preserved_source_raw_node_slice_matches(
        &self,
        source: Option<ast::SourceRawNodeSlice<'_>>,
        output: Option<ast::RawNodeSlice>,
    ) -> bool {
        self.traversal
            .preserved_source_raw_node_slice_view_matches(self.factory, source, output)
    }

    fn preserved_source_raw_string_slice_matches(
        &self,
        source: Option<ast::SourceRawStringSlice<'_>>,
        output: Option<ast::RawStringSlice>,
    ) -> bool {
        self.traversal
            .preserved_source_raw_string_slice_view_matches(self.factory, source, output)
    }

    fn flatten_visited_node(&mut self, visited: ast::Node, out: &mut Vec<ast::Node>) {
        self.traversal
            .flatten_visited_node(self.source, self.factory, visited, out);
    }

    fn append_visit_slice_result(
        &mut self,
        original: ast::Node,
        visited: Option<ast::Node>,
        out: &mut Vec<ast::Node>,
    ) {
        self.traversal
            .append_visit_slice_result(self.source, self.factory, original, visited, out);
    }

    fn visit_slice<I>(&mut self, nodes: I) -> Option<Vec<ast::Node>>
    where
        I: Clone + IntoIterator<Item = ast::Node>,
        I::IntoIter: ExactSizeIterator,
    {
        ast::visit_slice_with(self, nodes)
    }

    fn visit_each_child(&mut self, node: ast::Node) -> ast::Node {
        ast::AstGeneratedVisitEachChild::generated_visit_each_child(self, &node)
    }

    fn visit_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let node = node?;
        if let Some((qualifier, first_identifier, first_identifier_text)) =
            self.import_type_parts(node)
        {
            let Some((symbol, name)) = self.id_to_symbol.get(&first_identifier).cloned() else {
                let visited = self.visit_each_child(node);
                return Some(self.preserve_node(visited));
            };

            let qualifier = if name != first_identifier_text {
                let new_identifier = self.factory.new_identifier(name);
                replace_first_identifier_of_entity_name(
                    self.source,
                    self.factory,
                    &qualifier,
                    &new_identifier,
                )
            } else {
                self.clone_node_to_factory(qualifier)
            };

            self.symbols.push(symbol);
            self.converted = true;
            let type_arguments = self.visit_type_arguments_of(node);
            return Some(
                self.factory
                    .new_type_reference_node(qualifier, type_arguments),
            );
        }

        let visited = self.visit_each_child(node);
        Some(self.preserve_node(visited))
    }

    fn import_type_parts(&self, node: ast::Node) -> Option<(ast::Node, ast::Node, String)> {
        let store = self.store_for(node);
        if !is_literal_import_type_node(store, node) {
            return None;
        }
        let qualifier = store.qualifier(node)?;
        let first_identifier = get_first_identifier(store, qualifier);
        let first_identifier_text = store.text(first_identifier);
        Some((qualifier, first_identifier, first_identifier_text))
    }

    fn visit_type_arguments_of(&mut self, node: ast::Node) -> Option<ast::NodeList> {
        let type_arguments = self.source.source_type_arguments(node);
        self.visit_source_nodes(type_arguments)
    }

    fn visit_source_nodes(
        &mut self,
        nodes: Option<ast::SourceNodeList<'_>>,
    ) -> Option<ast::NodeList> {
        let nodes = nodes?;
        let source_list = nodes;
        if let Some(visited) = self.visit_slice(source_list) {
            Some(self.factory.new_node_list_with_trailing_comma(
                source_list.loc(),
                source_list.range(),
                visited,
                source_list.has_trailing_comma(),
            ))
        } else {
            Some(self.preserve_source_node_list(nodes))
        }
    }

    fn visit_source_nodes_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        let nodes = nodes?;
        if let Some(visited) = self.visit_slice(nodes.nodes()) {
            Some(self.factory.new_node_list_with_trailing_comma(
                nodes.loc(),
                nodes.range(),
                visited,
                nodes.has_trailing_comma(),
            ))
        } else {
            Some(
                self.traversal
                    .preserve_source_node_list_input(self.source, self.factory, &nodes),
            )
        }
    }

    fn visit_source_modifiers(
        &mut self,
        modifiers: Option<ast::SourceModifierList<'_>>,
    ) -> Option<ast::ModifierList> {
        let modifiers = modifiers?;
        let source_nodes = modifiers.nodes();
        if let Some(visited) = self.visit_slice(source_nodes) {
            Some(self.factory.new_modifier_list(
                source_nodes.loc(),
                source_nodes.range(),
                visited,
                ast::ModifierFlags::NONE,
            ))
        } else {
            Some(self.preserve_source_modifier_list(modifiers))
        }
    }

    fn visit_source_modifiers_input(
        &mut self,
        modifiers: Option<ast::SourceModifierListInput>,
    ) -> Option<ast::ModifierList> {
        let modifiers = modifiers?;
        if let Some(visited) = self.visit_slice(modifiers.nodes()) {
            Some(self.factory.new_modifier_list(
                modifiers.loc(),
                modifiers.range(),
                visited,
                modifiers.modifier_flags(),
            ))
        } else {
            Some(self.traversal.preserve_source_modifier_list_input(
                self.source,
                self.factory,
                &modifiers,
            ))
        }
    }

    fn append_raw_node_slice_result(
        &mut self,
        original: Option<ast::Node>,
        result: Option<ast::Node>,
        out: &mut Vec<Option<ast::Node>>,
    ) {
        self.traversal.append_raw_node_slice_result(
            self.source,
            self.factory,
            original,
            result,
            out,
        );
    }

    fn visit_source_raw_node_slice(
        &mut self,
        nodes: Option<ast::SourceRawNodeSlice<'_>>,
    ) -> Option<ast::RawNodeSlice> {
        let nodes = nodes?;
        if let Some(visited) = ast::visit_raw_node_slice_with(self, nodes) {
            return Some(self.factory.new_raw_node_slice(visited));
        }

        Some(self.preserve_source_raw_node_slice(nodes))
    }

    fn visit_source_raw_node_slice_input(
        &mut self,
        nodes: Option<ast::SourceRawNodeSliceInput>,
    ) -> Option<ast::RawNodeSlice> {
        let nodes = nodes?;
        let raw_nodes = nodes.iter().collect::<Vec<_>>();
        for (index, node) in raw_nodes.iter().copied().enumerate() {
            let visited = self.visit_node(node);
            if visited == node {
                continue;
            }

            let mut result = Vec::with_capacity(raw_nodes.len());
            result.extend(
                raw_nodes
                    .iter()
                    .copied()
                    .take(index)
                    .map(|node| node.map(|node| self.preserve_node(node))),
            );
            self.append_raw_node_slice_result(node, visited, &mut result);

            for node in raw_nodes.iter().copied().skip(index + 1) {
                let visited = self.visit_node(node);
                self.append_raw_node_slice_result(node, visited, &mut result);
            }

            return Some(self.factory.new_raw_node_slice(result));
        }

        Some(
            self.traversal
                .preserve_source_raw_node_slice_input(self.source, self.factory, &nodes),
        )
    }

    fn lift_to_block(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        self.traversal
            .lift_to_block(self.source, self.factory, node)
    }
}

impl ast::NodeSliceTraversal for AutoImportableReferenceTraversal<'_, '_> {
    fn visit_slice_node(&mut self, node: ast::Node) -> Option<ast::Node> {
        self.visit_node(Some(node))
    }

    fn import_slice_node(&mut self, node: ast::Node) -> ast::Node {
        self.preserve_node(node)
    }

    fn append_visited_slice_node(
        &mut self,
        original: ast::Node,
        visited: Option<ast::Node>,
        out: &mut Vec<ast::Node>,
    ) {
        self.append_visit_slice_result(original, visited, out);
    }
}

impl ast::RawNodeSliceTraversal for AutoImportableReferenceTraversal<'_, '_> {
    fn visit_raw_slice_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        match node {
            Some(node) => self.visit_node(Some(node)),
            None => self.visit_node(None),
        }
    }

    fn import_raw_slice_node(&mut self, node: ast::Node) -> ast::Node {
        self.preserve_node(node)
    }

    fn append_visited_raw_slice_node(
        &mut self,
        original: Option<ast::Node>,
        visited: Option<ast::Node>,
        out: &mut Vec<Option<ast::Node>>,
    ) {
        self.append_raw_node_slice_result(original, visited, out);
    }
}

impl<'source> ast::AstVisitEachChildRuntime<'source>
    for AutoImportableReferenceTraversal<'source, '_>
{
    fn source_store(&self) -> &'source ast::AstStore {
        self.source
    }

    fn factory(&self) -> &ast::NodeFactory {
        self.factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        self.factory
    }

    fn preserved_node(&self, source: ast::Node) -> Option<ast::Node> {
        let source = &source;
        AutoImportableReferenceTraversal::preserved_node(self, source)
    }

    fn preserve_node(&mut self, node: ast::Node) -> ast::Node {
        AutoImportableReferenceTraversal::preserve_node(self, node)
    }

    fn record_preserved_node(&mut self, source: ast::Node, imported: ast::Node) -> ast::Node {
        AutoImportableReferenceTraversal::record_preserved_node(self, source, imported)
    }

    fn preserved_source_node_matches(
        &self,
        source: Option<ast::Node>,
        output: Option<ast::Node>,
    ) -> bool {
        AutoImportableReferenceTraversal::preserved_source_node_matches(self, source, output)
    }

    fn preserved_source_node_list_input_matches(
        &self,
        source: Option<&ast::SourceNodeListInput>,
        output: Option<ast::NodeList>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        if source.store_id() == self.factory.store().store_id() {
            return output == Some(source.as_node_list());
        }
        self.traversal.preserved_source_node_list_input_matches(
            self.source,
            self.factory,
            Some(source),
            output,
        )
    }

    fn preserved_source_modifier_list_input_matches(
        &self,
        source: Option<&ast::SourceModifierListInput>,
        output: Option<ast::ModifierList>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        if source.store_id() == self.factory.store().store_id() {
            return output == Some(source.as_modifier_list());
        }
        self.traversal.preserved_source_modifier_list_input_matches(
            self.source,
            self.factory,
            Some(source),
            output,
        )
    }

    fn preserved_source_raw_node_slice_input_matches(
        &self,
        source: Option<&ast::SourceRawNodeSliceInput>,
        output: Option<ast::RawNodeSlice>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        if source.store_id() == self.factory.store().store_id() {
            return output == Some(source.as_raw_node_slice());
        }
        self.traversal
            .preserved_source_raw_node_slice_input_matches(
                self.source,
                self.factory,
                Some(source),
                output,
            )
    }

    fn preserved_source_raw_string_slice_input_matches(
        &self,
        source: Option<&ast::SourceRawStringSliceInput>,
        output: Option<ast::RawStringSlice>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        if source.store_id() == self.factory.store().store_id() {
            return output == Some(source.as_raw_string_slice());
        }
        self.traversal
            .preserved_source_raw_string_slice_input_matches(
                self.source,
                self.factory,
                Some(source),
                output,
            )
    }

    fn update_source_file_from_visited(
        &mut self,
        node: ast::Node,
        statements: Option<ast::NodeList>,
        end_of_file_token: Option<ast::Node>,
        source_unchanged: bool,
    ) -> ast::Node {
        let statements = statements.expect("source file statements cannot be removed");
        if node.store_id() == self.factory.store().store_id() {
            if source_unchanged {
                return node;
            }
            return self.factory.update_source_file_in_current_store(
                node,
                statements,
                end_of_file_token,
            );
        }
        if source_unchanged {
            let imported = self.preserve_node(node);
            return self.record_preserved_node(node, imported);
        }
        self.traversal.update_source_file_from_store(
            self.source,
            self.factory,
            node,
            statements,
            end_of_file_token,
        )
    }

    fn visit_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        AutoImportableReferenceTraversal::visit_node(self, node)
    }

    fn visit_token(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        AutoImportableReferenceTraversal::visit_node(self, node)
    }

    fn visit_nodes_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        AutoImportableReferenceTraversal::visit_source_nodes_input(self, nodes)
    }

    fn visit_modifiers_input(
        &mut self,
        modifiers: Option<ast::SourceModifierListInput>,
    ) -> Option<ast::ModifierList> {
        AutoImportableReferenceTraversal::visit_source_modifiers_input(self, modifiers)
    }

    fn visit_parameters_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        AutoImportableReferenceTraversal::visit_source_nodes_input(self, nodes)
    }

    fn visit_function_body(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        AutoImportableReferenceTraversal::visit_node(self, node)
    }

    fn visit_iteration_body(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let visited = AutoImportableReferenceTraversal::visit_node(self, node);
        self.lift_to_block(visited)
    }

    fn visit_top_level_statements_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        AutoImportableReferenceTraversal::visit_source_nodes_input(self, nodes)
    }

    fn visit_embedded_statement(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let visited = AutoImportableReferenceTraversal::visit_node(self, node);
        self.lift_to_block(visited)
    }

    fn visit_raw_node_slice_input(
        &mut self,
        nodes: Option<ast::SourceRawNodeSliceInput>,
    ) -> Option<ast::RawNodeSlice> {
        AutoImportableReferenceTraversal::visit_source_raw_node_slice_input(self, nodes)
    }
}

impl<'source> ast::AstGeneratedVisitEachChild<'source>
    for AutoImportableReferenceTraversal<'source, '_>
{
}

// Given a type node containing 'import("./a").SomeType<import("./b").OtherType<...>>',
// returns an equivalent type reference node with any nested ImportTypeNodes also replaced
// with type references, and a list of symbols that must be imported to use the type reference.
// TryGetAutoImportableReferenceFromTypeNode converts import type references in a type node
// to simple type references and returns the transformed type node and the symbols that need
// to be imported.
pub fn try_get_auto_importable_reference_from_type_node(
    source: &ast::AstStore,
    factory: &mut ast::NodeFactory,
    import_type_node: &ast::Node,
    id_to_symbol: HashMap<ast::Node, (ast::SymbolIdentity, String)>,
) -> AutoImportableReferenceTypeNode {
    let mut traversal = AutoImportableReferenceTraversal::new(source, factory, id_to_symbol);
    let type_node = traversal.visit_node(Some(*import_type_node));
    traversal.finish(type_node, *import_type_node)
}

pub fn try_get_auto_importable_reference_from_type_node_from_identifiers(
    source: &ast::AstStore,
    factory: &mut ast::NodeFactory,
    import_type_node: &ast::Node,
    id_to_symbol: HashMap<ast::IdentifierNode, ast::SymbolIdentity>,
) -> AutoImportableReferenceTypeNode {
    let id_to_symbol = id_to_symbol
        .into_iter()
        .map(|(identifier, symbol)| (identifier, (symbol, source.text(identifier))))
        .collect();
    try_get_auto_importable_reference_from_type_node(
        source,
        factory,
        import_type_node,
        id_to_symbol,
    )
}

fn clone_node_to_factory(
    source: &ast::AstStore,
    factory: &mut ast::NodeFactory,
    node: ast::Node,
) -> ast::Node {
    if node.store_id() == factory.store().store_id() {
        return node;
    }
    factory.deep_clone_node_from_store(source, node)
}

fn is_literal_import_type_node(store: &ast::AstStore, node: ast::Node) -> bool {
    if store.kind(node) != ast::Kind::ImportType {
        return false;
    }
    let Some(argument) = store.argument(node) else {
        return false;
    };
    if store.kind(argument) != ast::Kind::LiteralType {
        return false;
    }
    store
        .literal(argument)
        .is_some_and(|literal| store.kind(literal) == ast::Kind::StringLiteral)
}

fn get_first_identifier(store: &ast::AstStore, node: ast::Node) -> ast::Node {
    match store.kind(node) {
        ast::Kind::Identifier => node,
        ast::Kind::QualifiedName => get_first_identifier(
            store,
            store
                .left(node)
                .expect("qualified name should have a left side"),
        ),
        _ => panic!("expected entity name"),
    }
}

// If a type checker and multiple files are available, consider using `forEachNameOfDefaultExport`
// instead, which searches for names of re-exported defaults/namespaces in target files.
pub fn get_name_for_exported_symbol(
    store: &ast::AstStore,
    checker: &mut checker::Checker<'_, '_>,
    symbol: ast::SymbolIdentity,
    prefer_capitalized: bool,
) -> String {
    let Some(symbol_name) = checker.symbol_name_public(symbol) else {
        return String::new();
    };
    if symbol_name == ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS
        || symbol_name == ast::INTERNAL_SYMBOL_NAME_DEFAULT
    {
        for declaration in checker.collect_symbol_declarations_public(symbol) {
            if ast::is_export_assignment(store, declaration) {
                if let Some(expression) = store.expression(declaration) {
                    let inner = ast::skip_outer_expressions(store, expression, ast::OEK_ALL);
                    if store.kind(inner) == ast::Kind::Identifier {
                        return store.text(inner);
                    }
                }
                continue;
            }
            if ast::is_export_specifier(store, declaration)
                && checker
                    .source_node_symbol_public(declaration)
                    .and_then(|symbol| checker.symbol_flags_public(symbol))
                    .is_some_and(|flags| flags == ast::SYMBOL_FLAGS_ALIAS)
                && store.property_name(declaration).is_some()
            {
                let property_name = store.property_name(declaration).unwrap();
                if store.kind(property_name) == ast::Kind::Identifier {
                    return store.text(property_name);
                }
                continue;
            }
            if let Some(name) = ast::get_name_of_declaration(store, Some(declaration))
                && store.kind(name) == ast::Kind::Identifier
            {
                return store.text(name);
            }
        }
        if let Some(parent) = checker.symbol_parent_public(symbol)
            && let Some(parent_name) = checker.symbol_name_public(parent)
        {
            return if prefer_capitalized {
                capitalize_identifier(&parent_name)
            } else {
                parent_name
            };
        }
    }
    symbol_name
}

fn capitalize_identifier(text: &str) -> String {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    first.to_uppercase().chain(chars).collect()
}

pub fn replace_first_identifier_of_entity_name(
    source: &ast::AstStore,
    factory: &mut ast::NodeFactory,
    name: &ast::Node,
    new_identifier: &ast::IdentifierNode,
) -> ast::Node {
    if source.kind(*name) == ast::Kind::Identifier {
        return *new_identifier;
    }
    let (left, right) = {
        (
            source
                .left(*name)
                .expect("qualified name should have a left side"),
            source
                .right(*name)
                .expect("qualified name should have a right side"),
        )
    };
    let left = replace_first_identifier_of_entity_name(source, factory, &left, new_identifier);
    let right = clone_node_to_factory(source, factory, right);
    factory.new_qualified_name(left, right)
}
