use std::collections::HashMap;

use ts_ast as ast;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as core;
use ts_lsproto as lsproto;
use ts_printer as printer;
use ts_scanner as scanner;
use ts_stringutil as stringutil;

use crate::LanguageService;
use crate::change;
use crate::lsutil;

// OrganizeImports organizes imports by:
//  1. Removing unused imports
//  2. Coalescing imports from the same module
//  3. Sorting imports
impl LanguageService<'_> {
    pub(crate) fn organize_imports(
        &self,
        ctx: &core::Context,
        source_file: &ast::SourceFile,
        program: &compiler::Program,
        kind: lsproto::CodeActionKind,
    ) -> Result<HashMap<String, Vec<lsproto::TextEdit>>, core::Error> {
        let mut change_tracker = change::new_tracker(
            ctx.clone(),
            program.options(),
            self.format_options(),
            &self.converters,
        );
        let should_sort = kind == lsproto::CodeActionKind::SourceSortImports
            || kind == lsproto::CodeActionKind::SourceOrganizeImports;
        let should_combine = should_sort;
        let should_remove = kind == lsproto::CodeActionKind::SourceRemoveUnusedImports
            || kind == lsproto::CodeActionKind::SourceOrganizeImports;

        let statement_nodes = source_file.statements_view().iter().collect::<Vec<_>>();
        let top_level_statements = statement_nodes;
        let top_level_import_decls =
            lsutil::filter_import_declarations(source_file.store(), &top_level_statements);
        let top_level_import_group_decls =
            group_by_newline_contiguous(source_file, &top_level_import_decls);

        let preferences = self.user_preferences();
        let (mut comparers_to_test, type_orders_to_test) =
            lsutil::get_detection_lists(preferences.clone());
        let default_comparer = comparers_to_test.remove(0);

        let mut module_specifier_comparer: Option<Box<dyn Fn(&str, &str) -> i32>> = None;
        let mut named_import_comparer: Option<Box<dyn Fn(&str, &str) -> i32>> = None;
        if !preferences.organize_imports_ignore_case.is_unknown() {
            module_specifier_comparer = Some(default_comparer);
            let named_default_comparer =
                lsutil::get_detection_lists(preferences.clone()).0.remove(0);
            named_import_comparer = Some(named_default_comparer);
        }
        let mut type_order = preferences.organize_imports_type_order;

        if preferences.organize_imports_ignore_case.is_unknown() {
            let (result, _) = lsutil::detect_module_specifier_case_by_sort(
                source_file.store(),
                top_level_import_group_decls.clone(),
                lsutil::get_comparers(preferences.clone()),
            );
            module_specifier_comparer = Some(result);
        }

        if type_order == lsutil::OrganizeImportsTypeOrder::Auto
            || preferences.organize_imports_ignore_case.is_unknown()
        {
            let comparer_funcs = lsutil::get_comparers(preferences.clone());
            let (named_import_comparer2, type_order2, found) =
                lsutil::detect_named_import_organization_by_sort(
                    source_file.store(),
                    &top_level_import_decls,
                    &comparer_funcs,
                    &type_orders_to_test,
                );
            if found {
                if (named_import_comparer.is_none()
                    || preferences.organize_imports_ignore_case.is_unknown())
                    && named_import_comparer2.is_some()
                {
                    let cmp = named_import_comparer2.unwrap();
                    named_import_comparer = Some(Box::new(cmp));
                }
                if type_order == lsutil::OrganizeImportsTypeOrder::Auto {
                    type_order = type_order2;
                }
            }
        }

        let comparer = OrganizeImportsComparerSettings {
            module_specifier_comparer,
            named_import_comparer,
            type_order,
        };

        for import_group_decl in &top_level_import_group_decls {
            organize_imports_worker(
                import_group_decl,
                &comparer,
                should_sort,
                should_combine,
                should_remove,
                source_file,
                program,
                &mut change_tracker,
                ctx,
            )?;
        }

        if kind != lsproto::CodeActionKind::SourceRemoveUnusedImports {
            let top_level_export_group_decls = get_top_level_export_groups(source_file);
            for export_group_decl in &top_level_export_group_decls {
                organize_exports_worker(
                    export_group_decl,
                    &comparer,
                    source_file,
                    &mut change_tracker,
                );
            }
        }

        let store = source_file.store();
        for stmt in source_file.statements_view().iter() {
            if !ast::is_ambient_module(store, stmt) {
                continue;
            }
            let Some(body) = store.body(stmt) else {
                continue;
            };
            let statements: Vec<_> = store
                .statements(body)
                .map(|statements| statements.iter().collect())
                .unwrap_or_default();
            let ambient_module_import_decls = statements
                .iter()
                .filter(|stmt| store.kind(**stmt) == ast::Kind::ImportDeclaration)
                .map(|stmt| *stmt)
                .collect::<Vec<_>>();
            let ambient_module_import_group_decls =
                group_by_newline_contiguous(source_file, &ambient_module_import_decls);

            for import_group_decl in &ambient_module_import_group_decls {
                organize_imports_worker(
                    import_group_decl,
                    &comparer,
                    should_sort,
                    should_combine,
                    should_remove,
                    source_file,
                    program,
                    &mut change_tracker,
                    ctx,
                )?;
            }

            if kind != lsproto::CodeActionKind::SourceRemoveUnusedImports {
                let ambient_module_export_decls = statements
                    .iter()
                    .filter(|stmt| store.kind(**stmt) == ast::Kind::ExportDeclaration)
                    .map(|stmt| *stmt)
                    .collect::<Vec<_>>();
                organize_exports_worker(
                    &ambient_module_export_decls,
                    &comparer,
                    source_file,
                    &mut change_tracker,
                );
            }
        }

        Ok(change_tracker.get_changes())
    }
}

pub(crate) struct OrganizeImportsComparerSettings {
    pub module_specifier_comparer: Option<Box<dyn Fn(&str, &str) -> i32>>,
    pub named_import_comparer: Option<Box<dyn Fn(&str, &str) -> i32>>,
    pub type_order: lsutil::OrganizeImportsTypeOrder,
}

pub(crate) fn organize_imports_worker<'a>(
    old_import_decls: &[ast::Statement],
    comparer: &OrganizeImportsComparerSettings,
    should_sort: bool,
    should_combine: bool,
    should_remove: bool,
    source_file: &'a ast::SourceFile,
    program: &'a compiler::Program,
    change_tracker: &mut change::Tracker<'a>,
    ctx: &core::Context,
) -> Result<(), core::Error> {
    if old_import_decls.is_empty() {
        return Ok(());
    }

    // Header comment preservation is handled via LeadingTriviaOptionExclude in the change tracker below
    let mut processed_imports = old_import_decls.to_vec();
    if should_remove {
        processed_imports = program.with_type_checker_for_file_using(
            compiler::CheckerAccess::context(ctx),
            source_file,
            |type_checker| {
                Ok(remove_unused_imports(
                    &processed_imports,
                    source_file,
                    type_checker,
                    program,
                    change_tracker,
                ))
            },
        )?;
    }

    let mut new_import_decls: Vec<ast::Statement> = Vec::new();
    if should_combine {
        let mut grouped = group_by_module_specifier(source_file.store(), &processed_imports);
        if should_sort {
            grouped.sort_by(|a, b| {
                if a.is_empty() || b.is_empty() {
                    return std::cmp::Ordering::Equal;
                }
                let cmp = lsutil::compare_module_specifiers(
                    source_file.store(),
                    lsutil::get_module_specifier_expression(source_file.store(), &a[0]).as_ref(),
                    lsutil::get_module_specifier_expression(source_file.store(), &b[0]).as_ref(),
                    |m1, m2| {
                        comparer
                            .module_specifier_comparer
                            .as_ref()
                            .map(|f| f(m1, m2))
                            .unwrap_or_else(|| stringutil::compare_strings_case_sensitive(m1, m2))
                    },
                );
                cmp.cmp(&0)
            });
        }

        let specifier_comparer_preferences = lsutil::UserPreferences {
            organize_imports_type_order: comparer.type_order,
            ..lsutil::UserPreferences::default()
        };
        for import_group in grouped {
            let mut coalesced = coalesce_imports_worker(
                &import_group,
                comparer.module_specifier_comparer.as_deref(),
                comparer.named_import_comparer.as_deref(),
                &specifier_comparer_preferences,
                source_file,
                change_tracker,
            );
            if should_sort {
                coalesced.sort_by(|a, b| {
                    let cmp = lsutil::compare_imports_or_require_statements(
                        source_file.store(),
                        a,
                        b,
                        |m1, m2| {
                            comparer
                                .module_specifier_comparer
                                .as_ref()
                                .map(|f| f(m1, m2))
                                .unwrap_or_else(|| {
                                    stringutil::compare_strings_case_sensitive(m1, m2)
                                })
                        },
                    );
                    cmp.cmp(&0)
                });
            }
            new_import_decls.extend(coalesced);
        }
    } else {
        new_import_decls = processed_imports;
    }

    if should_sort && !should_combine {
        new_import_decls.sort_by(|a, b| {
            let cmp = lsutil::compare_imports_or_require_statements(
                source_file.store(),
                a,
                b,
                |m1, m2| {
                    comparer
                        .module_specifier_comparer
                        .as_ref()
                        .map(|f| f(m1, m2))
                        .unwrap_or_else(|| stringutil::compare_strings_case_sensitive(m1, m2))
                },
            );
            cmp.cmp(&0)
        });
    }

    if new_import_decls.is_empty() {
        change_tracker.delete_node_range(
            source_file,
            old_import_decls[0],
            old_import_decls[old_import_decls.len() - 1],
            change::LEADING_TRIVIA_OPTION_EXCLUDE,
            change::TRAILING_TRIVIA_OPTION_INCLUDE,
        );
    } else {
        for imp in &new_import_decls {
            change_tracker
                .emit_context
                .set_emit_flags(imp, printer::EF_NO_LEADING_COMMENTS);
        }

        let options = change::NodeOptions {
            leading_trivia_option: change::LEADING_TRIVIA_OPTION_EXCLUDE,
            trailing_trivia_option: change::TRAILING_TRIVIA_OPTION_INCLUDE,
            suffix: "\n".to_string(),
            ..Default::default()
        };

        let new_nodes = new_import_decls.iter().copied().collect();
        change_tracker.replace_node_with_nodes(
            source_file,
            old_import_decls[0],
            new_nodes,
            Some(options),
        );

        if old_import_decls.len() > 1 {
            for old_import_decl in old_import_decls.iter().skip(1) {
                change_tracker.delete(source_file, *old_import_decl);
            }
        }
    }
    Ok(())
}

pub(crate) fn group_by_module_specifier(
    store: &ast::AstStore,
    imports: &[ast::Statement],
) -> Vec<Vec<ast::Statement>> {
    let mut groups: HashMap<String, Vec<ast::Statement>> = HashMap::new();
    let mut order = Vec::new();

    for imp in imports {
        let specifier = lsutil::get_external_module_name(
            store,
            lsutil::get_module_specifier_expression(store, imp).as_ref(),
        );
        if !groups.contains_key(&specifier) {
            order.push(specifier.clone());
        }
        groups.entry(specifier).or_default().push(*imp);
    }

    order
        .into_iter()
        .map(|key| groups.remove(&key).unwrap_or_default())
        .collect()
}

fn explicit_node_store<'a>(
    source_store: &'a ast::AstStore,
    factory_store: Option<&'a ast::AstStore>,
    node: ast::Node,
) -> &'a ast::AstStore {
    if let Some(factory_store) = factory_store
        && node.store_id() == factory_store.store_id()
    {
        return factory_store;
    }
    assert_eq!(
        node.store_id(),
        source_store.store_id(),
        "organize imports node belongs to an unexpected AST store"
    );
    source_store
}

fn compare_import_or_export_specifiers_with_stores(
    source_store: &ast::AstStore,
    factory_store: Option<&ast::AstStore>,
    s1: &ast::Node,
    s2: &ast::Node,
    comparer: impl Fn(&str, &str) -> i32,
    preferences: lsutil::UserPreferences,
) -> i32 {
    let type_order = preferences.organize_imports_type_order;
    let s1_store = explicit_node_store(source_store, factory_store, *s1);
    let s2_store = explicit_node_store(source_store, factory_store, *s2);
    let s1_name = s1_store.name(*s1).unwrap();
    let s2_name = s2_store.name(*s2).unwrap();
    let s1_name_store = explicit_node_store(source_store, factory_store, s1_name);
    let s2_name_store = explicit_node_store(source_store, factory_store, s2_name);
    let s1_name_text = s1_name_store.text(s1_name);
    let s2_name_text = s2_name_store.text(s2_name);

    match type_order {
        lsutil::OrganizeImportsTypeOrder::First => {
            let cmp = core::compare_booleans(
                s2_store.is_type_only(*s2).unwrap_or(false),
                s1_store.is_type_only(*s1).unwrap_or(false),
            );
            if cmp != 0 {
                return cmp;
            }
            comparer(&s1_name_text, &s2_name_text)
        }
        lsutil::OrganizeImportsTypeOrder::Inline => comparer(&s1_name_text, &s2_name_text),
        lsutil::OrganizeImportsTypeOrder::Last | lsutil::OrganizeImportsTypeOrder::Auto => {
            let cmp = core::compare_booleans(
                s1_store.is_type_only(*s1).unwrap_or(false),
                s2_store.is_type_only(*s2).unwrap_or(false),
            );
            if cmp != 0 {
                return cmp;
            }
            comparer(&s1_name_text, &s2_name_text)
        }
    }
}

fn output_node(
    import_state: &mut ast::AstImportState,
    source_store: &ast::AstStore,
    factory: &mut ast::NodeFactory,
    node: ast::Node,
) -> ast::Node {
    if node.store_id() == factory.store().store_id() {
        return node;
    }
    assert_eq!(
        node.store_id(),
        source_store.store_id(),
        "organize imports source node belongs to an unexpected AST store"
    );
    import_state.preserve_node(source_store, factory, node)
}

fn optional_output_node(
    import_state: &mut ast::AstImportState,
    source_store: &ast::AstStore,
    factory: &mut ast::NodeFactory,
    node: Option<ast::Node>,
) -> Option<ast::Node> {
    node.map(|node| output_node(import_state, source_store, factory, node))
}

fn optional_output_modifiers(
    import_state: &mut ast::AstImportState,
    source_store: &ast::AstStore,
    factory: &mut ast::NodeFactory,
    node: ast::Node,
) -> Option<ast::ModifierList> {
    import_state
        .preserve_optional_source_modifier_list(factory, source_store.source_modifiers(node))
}

fn new_output_node_list(
    import_state: &mut ast::AstImportState,
    source_store: &ast::AstStore,
    factory: &mut ast::NodeFactory,
    loc: core::TextRange,
    range: core::TextRange,
    nodes: impl IntoIterator<Item = ast::Node>,
) -> ast::NodeList {
    let output_nodes = nodes
        .into_iter()
        .map(|node| output_node(import_state, source_store, factory, node))
        .collect::<Vec<_>>();
    factory.new_node_list(loc, range, output_nodes)
}

pub(crate) fn remove_unused_imports<'a>(
    old_imports: &[ast::Statement],
    source_file: &'a ast::SourceFile,
    type_checker: &mut checker::Checker<'a, '_>,
    program: &compiler::Program,
    change_tracker: &mut change::Tracker<'a>,
) -> Vec<ast::Statement> {
    let compiler_options = program.options();
    let store = source_file.store();
    let jsx_elements_present = (store.subtree_facts(source_file.as_node())
        & ast::SUBTREE_CONTAINS_JSX)
        != ast::SubtreeFacts::NONE;
    let jsx_mode_needs_explicit_import = compiler_options.jsx == core::JsxEmit::React
        || compiler_options.jsx == core::JsxEmit::ReactNative;

    let mut factory = ast::new_node_factory(ast::NodeFactoryHooks::default());
    let mut import_state = ast::AstImportState::new();
    let mut used_imports = Vec::with_capacity(old_imports.len());

    for import_decl in old_imports {
        let import_decl_node = *import_decl;
        let Some(import_clause) = store.import_clause(import_decl_node) else {
            used_imports.push(*import_decl);
            continue;
        };

        let mut name = store.name(import_clause);
        let mut named_bindings = store.named_bindings(import_clause);

        if let Some(import_name) = name {
            if !type_checker.is_declaration_used(
                source_file,
                import_name,
                jsx_elements_present,
                jsx_mode_needs_explicit_import,
            ) {
                name = None;
            }
        }

        if let Some(bindings) = named_bindings {
            match store.kind(bindings) {
                ast::Kind::NamespaceImport => {
                    let name_node = store.name(bindings).unwrap();
                    if !type_checker.is_declaration_used(
                        source_file,
                        name_node,
                        jsx_elements_present,
                        jsx_mode_needs_explicit_import,
                    ) {
                        named_bindings = None;
                    }
                }
                ast::Kind::NamedImports => {
                    let original_bindings = bindings;
                    let original_elements = store.elements(bindings).unwrap();
                    let elements = original_elements.iter().collect::<Vec<_>>();
                    let new_elements = filter_used_import_specifiers(
                        store,
                        &elements,
                        type_checker,
                        source_file,
                        jsx_elements_present,
                        jsx_mode_needs_explicit_import,
                    );
                    if new_elements.is_empty() {
                        named_bindings = None;
                    } else if new_elements.len() < original_elements.len() {
                        let new_list = new_output_node_list(
                            &mut import_state,
                            store,
                            &mut factory,
                            original_elements.loc(),
                            original_elements.range(),
                            new_elements,
                        );
                        let updated_named_imports =
                            factory.update_named_imports_from_store(store, bindings, new_list);
                        named_bindings = Some(updated_named_imports);
                    }
                    if let Some(bindings) = &named_bindings {
                        if !ast::node_is_synthesized(store, original_bindings)
                            && !printer::range_is_on_single_line(
                                store.loc(original_bindings),
                                source_file,
                            )
                        {
                            change_tracker
                                .emit_context
                                .set_emit_flags(bindings, printer::EF_MULTI_LINE);
                        }
                    }
                }
                _ => {}
            }
        }

        if name.is_some() || named_bindings.is_some() {
            let phase_modifier = store.phase_modifier(import_clause);
            let clause_name = optional_output_node(&mut import_state, store, &mut factory, name);
            let clause_named_bindings =
                optional_output_node(&mut import_state, store, &mut factory, named_bindings);
            let new_clause = factory.update_import_clause_from_store(
                store,
                import_clause,
                phase_modifier,
                clause_name,
                clause_named_bindings,
            );
            let modifiers =
                optional_output_modifiers(&mut import_state, store, &mut factory, import_decl_node);
            let module_specifier = optional_output_node(
                &mut import_state,
                store,
                &mut factory,
                store.module_specifier(import_decl_node),
            );
            let attributes = optional_output_node(
                &mut import_state,
                store,
                &mut factory,
                store.attributes(import_decl_node),
            );
            let new_import_decl = factory.update_import_declaration_from_store(
                store,
                import_decl_node,
                modifiers,
                Some(new_clause),
                module_specifier,
                attributes,
            );
            used_imports.push(new_import_decl);
        } else {
            let module_specifier = store.module_specifier(import_decl_node);
            if has_module_declaration_matching_specifier(
                store,
                source_file,
                module_specifier.as_ref(),
            ) {
                if source_file.is_declaration_file() {
                    let modifiers = optional_output_modifiers(
                        &mut import_state,
                        store,
                        &mut factory,
                        import_decl_node,
                    );
                    let module_specifier = optional_output_node(
                        &mut import_state,
                        store,
                        &mut factory,
                        store.module_specifier(import_decl_node),
                    );
                    let attributes = optional_output_node(
                        &mut import_state,
                        store,
                        &mut factory,
                        store.attributes(import_decl_node),
                    );
                    let new_import_decl = factory.update_import_declaration_from_store(
                        store,
                        import_decl_node,
                        modifiers,
                        None,
                        module_specifier,
                        attributes,
                    );
                    used_imports.push(new_import_decl);
                } else {
                    used_imports.push(*import_decl);
                }
            }
        }
    }

    used_imports
}

pub(crate) fn filter_used_import_specifiers<'a>(
    store: &'a ast::AstStore,
    elements: &[ast::Node],
    type_checker: &mut checker::Checker<'a, '_>,
    source_file: &'a ast::SourceFile,
    jsx_elements_present: bool,
    jsx_mode_needs_explicit_import: bool,
) -> Vec<ast::Node> {
    let mut result = Vec::new();
    for elem in elements {
        let Some(name) = store.name(*elem) else {
            continue;
        };
        if type_checker.is_declaration_used(
            source_file,
            name,
            jsx_elements_present,
            jsx_mode_needs_explicit_import,
        ) {
            result.push(*elem);
        }
    }
    result
}

pub(crate) fn has_module_declaration_matching_specifier(
    store: &ast::AstStore,
    source_file: &ast::SourceFile,
    module_specifier: Option<&ast::Expression>,
) -> bool {
    let Some(module_specifier) = module_specifier else {
        return false;
    };
    if !ast::is_string_literal(store, *module_specifier) {
        return false;
    }
    let module_specifier_text = store.text(*module_specifier);

    for module_name in source_file.module_augmentations() {
        if ast::is_string_literal(store, *module_name)
            && store.text(*module_name) == module_specifier_text
        {
            return true;
        }
    }
    false
}

// getImportAttributesKey returns a key for grouping imports by their attributes.
pub(crate) fn get_import_attributes_key(
    store: &ast::AstStore,
    attributes: Option<&ast::Node>,
) -> String {
    let Some(attributes) = attributes else {
        return String::new();
    };

    let mut key = String::new();
    key.push_str(&store.token(*attributes).unwrap().as_str());
    key.push(' ');

    let mut attr_nodes: Vec<_> = store.source_attributes(*attributes).unwrap().nodes();
    attr_nodes.sort_by(|a, b| {
        let a_name = store.text(store.name(*a).unwrap());
        let b_name = store.text(store.name(*b).unwrap());
        stringutil::compare_strings_case_sensitive(&a_name, &b_name).cmp(&0)
    });

    for attr_node in &attr_nodes {
        key.push_str(&store.text(store.name(*attr_node).unwrap()));
        key.push(':');
        let value = store.value(*attr_node).unwrap();
        if ast::is_string_literal_like(store, value) {
            key.push('"');
            key.push_str(&store.text(value));
            key.push('"');
        } else {
            key.push_str(&store.text(value));
        }
        key.push(' ');
    }

    key
}

// groupByNewlineContiguous groups declarations by blank lines between them.
pub(crate) fn group_by_newline_contiguous(
    source_file: &ast::SourceFile,
    decls: &[ast::Statement],
) -> Vec<Vec<ast::Statement>> {
    let mut groups = Vec::new();
    let mut current_group = Vec::new();

    for decl in decls {
        if !current_group.is_empty() && is_new_group(source_file, decl) {
            groups.push(current_group);
            current_group = Vec::new();
        }
        current_group.push(*decl);
    }

    if !current_group.is_empty() {
        groups.push(current_group);
    }

    groups
}

pub(crate) fn is_new_group(source_file: &ast::SourceFile, decl: &ast::Statement) -> bool {
    let full_start = source_file.store().loc(*decl).pos();
    if full_start < 0 {
        return false;
    }

    let text = source_file.text();
    let text_len = text.len() as i32;
    if full_start >= text_len {
        return false;
    }

    let start_pos = scanner::skip_trivia(text, full_start as usize) as i32;
    if start_pos <= full_start {
        return false;
    }

    let trivia = &text[full_start as usize..start_pos as usize];
    let mut number_of_new_lines = 0;
    let bytes = trivia.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            continue;
        }
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' && bytes[i] != b'\r' {
                i += 1;
            }
            continue;
        }
        if bytes[i] == b'\n' {
            number_of_new_lines += 1;
            i += 1;
        } else if bytes[i] == b'\r' {
            number_of_new_lines += 1;
            i += 1;
            if i < bytes.len() && bytes[i] == b'\n' {
                i += 1;
            }
        } else {
            i += 1;
        }
        if number_of_new_lines >= 2 {
            return true;
        }
    }
    false
}

pub(crate) fn coalesce_imports_worker<'a>(
    import_decls: &[ast::Statement],
    comparer: Option<&dyn Fn(&str, &str) -> i32>,
    named_import_comparer: Option<&dyn Fn(&str, &str) -> i32>,
    specifier_comparer_preferences: &lsutil::UserPreferences,
    source_file: &'a ast::SourceFile,
    change_tracker: &mut change::Tracker<'a>,
) -> Vec<ast::Statement> {
    if import_decls.is_empty() {
        return import_decls.to_vec();
    }

    let mut import_groups_by_attributes: HashMap<String, Vec<ast::Statement>> = HashMap::new();
    let mut attribute_keys = Vec::new();
    let source_store = source_file.store();

    for import_decl in import_decls {
        let key =
            get_import_attributes_key(source_store, source_store.attributes(*import_decl).as_ref());
        if !import_groups_by_attributes.contains_key(&key) {
            attribute_keys.push(key.clone());
        }
        import_groups_by_attributes
            .entry(key)
            .or_default()
            .push(*import_decl);
    }

    let mut coalesced_imports = Vec::new();
    let mut factory = ast::new_node_factory(ast::NodeFactoryHooks::default());
    let mut import_state = ast::AstImportState::new();

    for attribute_key in attribute_keys {
        let import_group_same_attrs = import_groups_by_attributes
            .remove(&attribute_key)
            .unwrap_or_default();
        let categorized = get_categorized_imports(source_store, &import_group_same_attrs);

        if let Some(import_without_clause) = categorized.import_without_clause {
            coalesced_imports.push(import_without_clause);
        }

        for (i, mut group) in [categorized.regular_imports, categorized.type_only_imports]
            .into_iter()
            .enumerate()
        {
            if group.is_empty() {
                continue;
            }

            let is_type_only = i == 1;

            if !is_type_only
                && group.default_imports.len() == 1
                && group.namespace_imports.len() == 1
                && group.named_imports.is_empty()
            {
                let default_import = group.default_imports[0];
                let namespace_import = group.namespace_imports[0];
                let default_import_node = default_import;
                let namespace_import_node = namespace_import;
                let default_clause_node = source_store.import_clause(default_import_node).unwrap();
                let namespace_clause_node =
                    source_store.import_clause(namespace_import_node).unwrap();
                let namespace_bindings = source_store.named_bindings(namespace_clause_node);
                let phase_modifier = source_store.phase_modifier(default_clause_node);
                let clause_name = optional_output_node(
                    &mut import_state,
                    source_store,
                    &mut factory,
                    source_store.name(default_clause_node),
                );
                let clause_named_bindings = optional_output_node(
                    &mut import_state,
                    source_store,
                    &mut factory,
                    namespace_bindings,
                );
                let new_clause = factory.update_import_clause_from_store(
                    source_store,
                    default_clause_node,
                    phase_modifier,
                    clause_name,
                    clause_named_bindings,
                );
                let modifiers = optional_output_modifiers(
                    &mut import_state,
                    source_store,
                    &mut factory,
                    default_import_node,
                );
                let module_specifier = optional_output_node(
                    &mut import_state,
                    source_store,
                    &mut factory,
                    source_store.module_specifier(default_import_node),
                );
                let attributes = optional_output_node(
                    &mut import_state,
                    source_store,
                    &mut factory,
                    source_store.attributes(default_import_node),
                );
                let new_import_decl = factory.update_import_declaration_from_store(
                    source_store,
                    default_import_node,
                    modifiers,
                    Some(new_clause),
                    module_specifier,
                    attributes,
                );
                coalesced_imports.push(new_import_decl);
                continue;
            }

            group.namespace_imports.sort_by(|a, b| {
                let n1_clause = source_store.import_clause(*a).unwrap();
                let n1_bindings = source_store.named_bindings(n1_clause).unwrap();
                let n1 = source_store.name(n1_bindings).unwrap();
                let n2_clause = source_store.import_clause(*b).unwrap();
                let n2_bindings = source_store.named_bindings(n2_clause).unwrap();
                let n2 = source_store.name(n2_bindings).unwrap();
                let n1_text = source_store.text(n1);
                let n2_text = source_store.text(n2);
                comparer
                    .map(|cmp| cmp(&n1_text, &n2_text))
                    .unwrap_or_else(|| {
                        stringutil::compare_strings_case_sensitive(&n1_text, &n2_text)
                    })
                    .cmp(&0)
            });

            for ns_import in &group.namespace_imports {
                let ns_import_node = *ns_import;
                let clause_node = source_store.import_clause(ns_import_node).unwrap();
                let phase_modifier = source_store.phase_modifier(clause_node);
                let clause_named_bindings = optional_output_node(
                    &mut import_state,
                    source_store,
                    &mut factory,
                    source_store.named_bindings(clause_node),
                );
                let new_clause = factory.update_import_clause_from_store(
                    source_store,
                    clause_node,
                    phase_modifier,
                    None,
                    clause_named_bindings,
                );
                let modifiers = optional_output_modifiers(
                    &mut import_state,
                    source_store,
                    &mut factory,
                    ns_import_node,
                );
                let module_specifier = optional_output_node(
                    &mut import_state,
                    source_store,
                    &mut factory,
                    source_store.module_specifier(ns_import_node),
                );
                let attributes = optional_output_node(
                    &mut import_state,
                    source_store,
                    &mut factory,
                    source_store.attributes(ns_import_node),
                );
                let new_import_decl = factory.update_import_declaration_from_store(
                    source_store,
                    ns_import_node,
                    modifiers,
                    Some(new_clause),
                    module_specifier,
                    attributes,
                );
                coalesced_imports.push(new_import_decl);
            }

            let first_default_import = group.default_imports.first().copied();
            let first_named_import = group.named_imports.first().copied();
            let Some(import_decl) = first_default_import.or(first_named_import) else {
                continue;
            };

            let mut new_default_import = None;
            let mut new_import_specifiers = Vec::new();

            if group.default_imports.len() == 1 {
                let default_import_node = group.default_imports[0];
                let default_clause_node = source_store.import_clause(default_import_node).unwrap();
                new_default_import = optional_output_node(
                    &mut import_state,
                    source_store,
                    &mut factory,
                    source_store.name(default_clause_node),
                );
            } else {
                for default_import in &group.default_imports {
                    let default_clause_node = source_store.import_clause(*default_import).unwrap();
                    let default_name = source_store.name(default_clause_node);
                    let property_name = factory.new_identifier("default".to_string());
                    let default_name = optional_output_node(
                        &mut import_state,
                        source_store,
                        &mut factory,
                        default_name,
                    );
                    let import_spec = factory.new_import_specifier(
                        false,
                        Some(property_name),
                        default_name.unwrap(),
                    );
                    new_import_specifiers.push(import_spec);
                }
            }

            new_import_specifiers.extend(get_new_import_specifiers(
                source_store,
                &group.named_imports,
                &mut factory,
                &mut import_state,
            ));
            new_import_specifiers.sort_by(|a, b| {
                compare_import_or_export_specifiers_with_stores(
                    source_store,
                    Some(factory.store()),
                    a,
                    b,
                    |a, b| {
                        named_import_comparer
                            .map(|cmp| cmp(a, b))
                            .unwrap_or_else(|| stringutil::compare_strings_case_sensitive(a, b))
                    },
                    specifier_comparer_preferences.clone(),
                )
                .cmp(&0)
            });

            let mut new_named_imports = None;
            if new_import_specifiers.is_empty() {
                if new_default_import.is_none() {
                    let empty = core::undefined_text_range();
                    let empty_list = factory.new_node_list(empty, empty, Vec::new());
                    new_named_imports = Some(factory.new_named_imports(empty_list));
                }
            } else if let Some(first_named_import) = first_named_import {
                let first_named_import_node = first_named_import;
                let first_named_clause =
                    source_store.import_clause(first_named_import_node).unwrap();
                let first_named_bindings_node =
                    source_store.named_bindings(first_named_clause).unwrap();
                let original_elements = source_store.elements(first_named_bindings_node).unwrap();
                let sorted_list = new_output_node_list(
                    &mut import_state,
                    source_store,
                    &mut factory,
                    original_elements.loc(),
                    original_elements.range(),
                    new_import_specifiers,
                );
                new_named_imports = Some(factory.update_named_imports_from_store(
                    source_store,
                    first_named_bindings_node,
                    sorted_list,
                ));
            } else {
                let empty = core::undefined_text_range();
                let sorted_list = factory.new_node_list(empty, empty, new_import_specifiers);
                new_named_imports = Some(factory.new_named_imports(sorted_list));
            }

            if let (Some(new_named_imports_node), Some(first_named_import)) =
                (new_named_imports.as_ref(), first_named_import)
            {
                let first_named_import_node = first_named_import;
                let first_named_clause =
                    source_store.import_clause(first_named_import_node).unwrap();
                let first_named_bindings = source_store.named_bindings(first_named_clause).unwrap();
                if !ast::node_is_synthesized(source_store, first_named_bindings)
                    && !printer::range_is_on_single_line(
                        source_store.loc(first_named_bindings),
                        source_file,
                    )
                {
                    change_tracker
                        .emit_context
                        .set_emit_flags(new_named_imports_node, printer::EF_MULTI_LINE);
                }
            }

            if is_type_only && new_default_import.is_some() && new_named_imports.is_some() {
                let import_decl_node = import_decl;
                let import_clause_node = source_store.import_clause(import_decl_node).unwrap();
                let default_clause = factory.new_import_clause(
                    source_store.phase_modifier(import_clause_node),
                    new_default_import.clone(),
                    None,
                );
                let default_modifiers = optional_output_modifiers(
                    &mut import_state,
                    source_store,
                    &mut factory,
                    import_decl_node,
                );
                let default_module_specifier = optional_output_node(
                    &mut import_state,
                    source_store,
                    &mut factory,
                    source_store.module_specifier(import_decl_node),
                );
                let default_attributes = optional_output_node(
                    &mut import_state,
                    source_store,
                    &mut factory,
                    source_store.attributes(import_decl_node),
                );
                let default_import_decl = factory.update_import_declaration_from_store(
                    source_store,
                    import_decl_node,
                    default_modifiers,
                    Some(default_clause),
                    default_module_specifier,
                    default_attributes,
                );
                coalesced_imports.push(default_import_decl);

                let named_decl_node = first_named_import.unwrap_or(import_decl);
                let named_import_decl_node = named_decl_node;
                let named_import_clause_node =
                    source_store.import_clause(named_import_decl_node).unwrap();
                let named_clause = factory.new_import_clause(
                    source_store.phase_modifier(named_import_clause_node),
                    None,
                    new_named_imports.clone(),
                );
                let named_modifiers = optional_output_modifiers(
                    &mut import_state,
                    source_store,
                    &mut factory,
                    named_import_decl_node,
                );
                let named_module_specifier = optional_output_node(
                    &mut import_state,
                    source_store,
                    &mut factory,
                    source_store.module_specifier(named_import_decl_node),
                );
                let named_attributes = optional_output_node(
                    &mut import_state,
                    source_store,
                    &mut factory,
                    source_store.attributes(named_import_decl_node),
                );
                let named_import_decl = factory.update_import_declaration_from_store(
                    source_store,
                    named_import_decl_node,
                    named_modifiers,
                    Some(named_clause),
                    named_module_specifier,
                    named_attributes,
                );
                coalesced_imports.push(named_import_decl);
            } else {
                let import_decl_node = import_decl;
                let clause_node_handle = source_store.import_clause(import_decl_node).unwrap();
                let new_clause = factory.update_import_clause_from_store(
                    source_store,
                    clause_node_handle,
                    source_store.phase_modifier(clause_node_handle),
                    new_default_import,
                    new_named_imports,
                );
                let modifiers = optional_output_modifiers(
                    &mut import_state,
                    source_store,
                    &mut factory,
                    import_decl_node,
                );
                let module_specifier = optional_output_node(
                    &mut import_state,
                    source_store,
                    &mut factory,
                    source_store.module_specifier(import_decl_node),
                );
                let attributes = optional_output_node(
                    &mut import_state,
                    source_store,
                    &mut factory,
                    source_store.attributes(import_decl_node),
                );
                let new_import_decl = factory.update_import_declaration_from_store(
                    source_store,
                    import_decl_node,
                    modifiers,
                    Some(new_clause),
                    module_specifier,
                    attributes,
                );
                coalesced_imports.push(new_import_decl);
            }
        }
    }

    coalesced_imports
}

#[derive(Default)]
pub(crate) struct CategorizedImports {
    pub import_without_clause: Option<ast::Statement>,
    pub type_only_imports: ImportGroup,
    pub regular_imports: ImportGroup,
}

#[derive(Default)]
pub(crate) struct ImportGroup {
    pub default_imports: Vec<ast::Statement>,
    pub namespace_imports: Vec<ast::Statement>,
    pub named_imports: Vec<ast::Statement>,
}

impl ImportGroup {
    pub(crate) fn is_empty(&self) -> bool {
        self.default_imports.is_empty()
            && self.namespace_imports.is_empty()
            && self.named_imports.is_empty()
    }
}

pub(crate) fn get_categorized_imports(
    store: &ast::AstStore,
    import_decls: &[ast::Statement],
) -> CategorizedImports {
    let mut import_without_clause = None;
    let mut type_only_imports = ImportGroup::default();
    let mut regular_imports = ImportGroup::default();

    for import_decl in import_decls {
        let import_decl_node = *import_decl;
        let Some(import_clause) = store.import_clause(import_decl_node) else {
            if import_without_clause.is_none() {
                import_without_clause = Some(*import_decl);
            }
            continue;
        };

        let group = if store.is_type_only(import_clause).unwrap_or(false) {
            &mut type_only_imports
        } else {
            &mut regular_imports
        };

        if store.name(import_clause).is_some() {
            group.default_imports.push(*import_decl);
        }

        if let Some(named_bindings) = store.named_bindings(import_clause) {
            match store.kind(named_bindings) {
                ast::Kind::NamespaceImport => group.namespace_imports.push(*import_decl),
                ast::Kind::NamedImports => group.named_imports.push(*import_decl),
                _ => {}
            }
        }
    }

    CategorizedImports {
        import_without_clause,
        type_only_imports,
        regular_imports,
    }
}

pub(crate) fn get_new_import_specifiers(
    store: &ast::AstStore,
    named_imports: &[ast::Statement],
    factory: &mut ast::NodeFactory,
    import_state: &mut ast::AstImportState,
) -> Vec<ast::Node> {
    let mut result = Vec::new();
    for named_import in named_imports {
        let Some(elements) = try_get_named_binding_elements(store, named_import) else {
            continue;
        };
        for elem in elements {
            if let (Some(property_name), Some(name)) = (store.property_name(elem), store.name(elem))
            {
                if store.text(property_name) == store.text(name) {
                    let name = output_node(import_state, store, factory, name);
                    let normalized = factory.update_import_specifier_from_store(
                        store,
                        elem,
                        store.is_type_only(elem).unwrap_or(false),
                        None,
                        Some(name),
                    );
                    result.push(normalized);
                    continue;
                }
            }

            result.push(output_node(import_state, store, factory, elem));
        }
    }
    result
}

pub(crate) fn try_get_named_binding_elements(
    store: &ast::AstStore,
    named_import: &ast::Statement,
) -> Option<Vec<ast::Node>> {
    if store.kind(*named_import) != ast::Kind::ImportDeclaration {
        return None;
    }
    let import_clause = store.import_clause(*named_import)?;
    let named_bindings = store.named_bindings(import_clause)?;
    if store.kind(named_bindings) == ast::Kind::NamedImports {
        return Some(store.elements(named_bindings)?.iter().collect());
    }
    None
}

pub(crate) fn get_top_level_export_groups(
    source_file: &ast::SourceFile,
) -> Vec<Vec<ast::Statement>> {
    let mut top_level_export_groups: Vec<Vec<ast::Statement>> = Vec::new();
    let store = source_file.store();
    let statements: Vec<_> = source_file.statements_view().iter().collect();
    let statements_len = statements.len();

    let mut i = 0;
    let mut group_index = 0;
    while i < statements_len {
        if store.kind(statements[i]) == ast::Kind::ExportDeclaration {
            if group_index >= top_level_export_groups.len() {
                top_level_export_groups.push(Vec::new());
            }
            if store.module_specifier(statements[i]).is_some() {
                top_level_export_groups[group_index].push(statements[i]);
                i += 1;
            } else {
                while i < statements_len
                    && store.kind(statements[i]) == ast::Kind::ExportDeclaration
                {
                    top_level_export_groups[group_index].push(statements[i]);
                    i += 1;
                }
                group_index += 1;
            }
        } else {
            i += 1;
            if group_index < top_level_export_groups.len()
                && !top_level_export_groups[group_index].is_empty()
            {
                group_index += 1;
            }
        }
    }

    let mut result = Vec::new();
    for export_group in top_level_export_groups {
        let sub_groups = group_by_newline_contiguous(source_file, &export_group);
        result.extend(sub_groups);
    }
    result
}

pub(crate) fn organize_exports_worker<'a>(
    old_export_decls: &[ast::Statement],
    comparer: &OrganizeImportsComparerSettings,
    source_file: &'a ast::SourceFile,
    change_tracker: &mut change::Tracker<'a>,
) {
    if old_export_decls.is_empty() {
        return;
    }

    let specifier_comparer_preferences = lsutil::UserPreferences {
        organize_imports_type_order: comparer.type_order,
        ..lsutil::UserPreferences::default()
    };
    let specifier_comparer_func = |s1: &ast::Node, s2: &ast::Node| {
        lsutil::compare_import_or_export_specifiers(
            source_file.store(),
            s1,
            s2,
            |a, b| {
                comparer
                    .named_import_comparer
                    .as_ref()
                    .map(|cmp| cmp(a, b))
                    .unwrap_or_else(|| stringutil::compare_strings_case_sensitive(a, b))
            },
            specifier_comparer_preferences.clone(),
        )
    };

    let new_export_decls = coalesce_exports_worker(
        old_export_decls,
        &specifier_comparer_func,
        comparer.module_specifier_comparer.as_deref(),
        source_file,
        change_tracker,
    );

    if !old_export_decls.is_empty() {
        if new_export_decls.is_empty() {
            change_tracker.delete_node_range(
                source_file,
                old_export_decls[0],
                old_export_decls[old_export_decls.len() - 1],
                change::LEADING_TRIVIA_OPTION_EXCLUDE,
                change::TRAILING_TRIVIA_OPTION_INCLUDE,
            );
        } else {
            for exp in &new_export_decls {
                change_tracker
                    .emit_context
                    .mark_emit_node(exp, printer::EF_NO_LEADING_COMMENTS);
            }

            let options = change::NodeOptions {
                leading_trivia_option: change::LEADING_TRIVIA_OPTION_EXCLUDE,
                trailing_trivia_option: change::TRAILING_TRIVIA_OPTION_INCLUDE,
                suffix: "\n".to_string(),
                ..Default::default()
            };

            let new_nodes = new_export_decls.iter().copied().collect();
            change_tracker.replace_node_with_nodes(
                source_file,
                old_export_decls[0],
                new_nodes,
                Some(options),
            );

            if old_export_decls.len() > 1 {
                for old_export_decl in old_export_decls.iter().skip(1) {
                    change_tracker.delete(source_file, *old_export_decl);
                }
            }
        }
    }
}

pub(crate) fn coalesce_exports_worker<'a>(
    export_group: &[ast::Statement],
    specifier_comparer: &impl Fn(&ast::Node, &ast::Node) -> i32,
    module_specifier_comparer: Option<&dyn Fn(&str, &str) -> i32>,
    source_file: &'a ast::SourceFile,
    change_tracker: &mut change::Tracker<'a>,
) -> Vec<ast::Statement> {
    if export_group.is_empty() {
        return export_group.to_vec();
    }

    let mut exports_by_module_specifier: HashMap<String, Vec<ast::Statement>> = HashMap::new();
    let mut module_specifier_order = Vec::new();
    let source_store = source_file.store();

    for export_decl in export_group {
        let module_specifier = source_store
            .module_specifier(*export_decl)
            .map(|ms| source_store.text(ms))
            .unwrap_or_default();
        if !exports_by_module_specifier.contains_key(&module_specifier) {
            module_specifier_order.push(module_specifier.clone());
        }
        exports_by_module_specifier
            .entry(module_specifier)
            .or_default()
            .push(*export_decl);
    }

    module_specifier_order.sort_by(|a, b| {
        if a.is_empty() && !b.is_empty() {
            return std::cmp::Ordering::Greater;
        }
        if !a.is_empty() && b.is_empty() {
            return std::cmp::Ordering::Less;
        }
        module_specifier_comparer
            .map(|cmp| cmp(a, b))
            .unwrap_or_else(|| stringutil::compare_strings_case_sensitive(a, b))
            .cmp(&0)
    });

    let mut coalesced_exports = Vec::new();
    let mut factory = ast::new_node_factory(ast::NodeFactoryHooks::default());
    let mut import_state = ast::AstImportState::new();
    for module_specifier in module_specifier_order {
        let group = exports_by_module_specifier
            .remove(&module_specifier)
            .unwrap_or_default();
        let categorized = get_categorized_exports(source_store, &group);

        if let Some(export_without_clause) = categorized.export_without_clause {
            coalesced_exports.push(export_without_clause);
        }

        for sub_group in [categorized.named_exports, categorized.type_only_exports] {
            if sub_group.is_empty() {
                continue;
            }

            let mut new_export_specifiers = Vec::new();
            for export_decl in &sub_group {
                let export_clause = source_store.export_clause(*export_decl);
                if let Some(export_clause) = export_clause {
                    if source_store.kind(export_clause) == ast::Kind::NamedExports {
                        if let Some(elements) = source_store.elements(export_clause) {
                            new_export_specifiers.extend(elements.iter());
                        }
                    }
                }
            }

            new_export_specifiers.sort_by(|a, b| specifier_comparer(a, b).cmp(&0));

            let export_decl_node = sub_group[0];
            let mut updated_export_clause = None;
            if let Some(export_clause) = source_store.export_clause(export_decl_node) {
                if source_store.kind(export_clause) == ast::Kind::NamedExports {
                    let elements = source_store.elements(export_clause);
                    let (loc, range) = elements
                        .map(|elements| (elements.loc(), elements.range()))
                        .unwrap_or_else(|| {
                            let empty = core::undefined_text_range();
                            (empty, empty)
                        });
                    let sorted_list = new_output_node_list(
                        &mut import_state,
                        source_store,
                        &mut factory,
                        loc,
                        range,
                        new_export_specifiers,
                    );
                    let updated = factory.update_named_exports_from_store(
                        source_store,
                        export_clause,
                        sorted_list,
                    );
                    if !ast::node_is_synthesized(source_store, export_clause)
                        && !printer::range_is_on_single_line(
                            source_store.loc(export_clause),
                            source_file,
                        )
                    {
                        change_tracker
                            .emit_context
                            .set_emit_flags(&updated, printer::EF_MULTI_LINE);
                    }
                    updated_export_clause = Some(updated);
                } else {
                    updated_export_clause = optional_output_node(
                        &mut import_state,
                        source_store,
                        &mut factory,
                        Some(export_clause),
                    );
                }
            }

            let modifiers = optional_output_modifiers(
                &mut import_state,
                source_store,
                &mut factory,
                export_decl_node,
            );
            let module_specifier = optional_output_node(
                &mut import_state,
                source_store,
                &mut factory,
                source_store.module_specifier(export_decl_node),
            );
            let attributes = optional_output_node(
                &mut import_state,
                source_store,
                &mut factory,
                source_store.attributes(export_decl_node),
            );
            let new_export_decl = factory.update_export_declaration_from_store(
                source_store,
                export_decl_node,
                modifiers,
                source_store.is_type_only(export_decl_node).unwrap_or(false),
                updated_export_clause,
                module_specifier,
                attributes,
            );
            coalesced_exports.push(new_export_decl);
        }
    }

    coalesced_exports
}

#[derive(Default)]
pub(crate) struct CategorizedExports {
    pub export_without_clause: Option<ast::Statement>,
    pub named_exports: Vec<ast::Statement>,
    pub type_only_exports: Vec<ast::Statement>,
}

pub(crate) fn get_categorized_exports(
    store: &ast::AstStore,
    export_group: &[ast::Statement],
) -> CategorizedExports {
    let mut export_without_clause = None;
    let mut named_exports = Vec::new();
    let mut type_only_exports = Vec::new();

    for export_decl in export_group {
        if store.export_clause(*export_decl).is_none() {
            if export_without_clause.is_none() {
                export_without_clause = Some(*export_decl);
            }
        } else if store.is_type_only(*export_decl).unwrap_or(false) {
            type_only_exports.push(*export_decl);
        } else {
            named_exports.push(*export_decl);
        }
    }

    CategorizedExports {
        export_without_clause,
        named_exports,
        type_only_exports,
    }
}
