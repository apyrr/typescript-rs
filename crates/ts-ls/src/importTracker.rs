use std::{cell::RefCell, collections::HashSet};

use ts_ast as ast;
use ts_checker as checker;
use ts_collections as collections;
use ts_compiler as compiler;
use ts_core as core;
use ts_debug as debug;

use crate::utilities::is_object_binding_element_without_property_name;

fn store_for_node<'a>(source_files: &[&'a ast::SourceFile], node: ast::Node) -> &'a ast::AstStore {
    source_files
        .iter()
        .copied()
        .find(|file| file.store().store_id() == node.store_id())
        .map(|file| file.store())
        .expect("node must belong to one of the tracked source file stores")
}

fn source_file_for_node<'a>(
    source_files: &[&'a ast::SourceFile],
    node: ast::Node,
) -> &'a ast::SourceFile {
    source_files
        .iter()
        .copied()
        .find(|file| file.store().store_id() == node.store_id())
        .expect("node must belong to one of the tracked source file stores")
}

fn node_seen_tracker() -> impl FnMut(ast::Node) -> bool {
    let mut seen = HashSet::new();
    move |node| seen.insert(node)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ImpExpKind {
    #[default]
    Unknown = 0,
    Import = 1,
    Export = 2,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ImportExportSymbol {
    pub kind: ImpExpKind,
    pub(crate) symbol: Option<ast::SymbolIdentity>,
    pub export_info: Option<ExportInfo>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ExportKind {
    #[default]
    Named = 0,
    Default = 1,
    ExportEquals = 2,
    Umd = 3,
    Module = 4,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ExportInfo {
    pub(crate) exporting_module_symbol: Option<ast::SymbolIdentity>,
    pub export_kind: ExportKind,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct LocationAndSymbol {
    pub import_location: Option<ast::Node>,
    pub(crate) import_symbol: Option<ast::SymbolIdentity>,
}

#[derive(Clone, Default)]
pub(crate) struct ImportsResult<'a> {
    pub import_searches: Vec<LocationAndSymbol>,
    pub single_references: Vec<ast::Node>,
    pub indirect_users: Vec<&'a ast::SourceFile>,
}

pub(crate) type ImportTracker<'a> =
    Box<dyn FnMut(ast::SymbolIdentity, &ExportInfo, bool) -> ImportsResult<'a> + 'a>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ModuleReferenceKind {
    #[default]
    Import = 0,
    Reference = 1,
    Implicit = 2,
}

// ModuleReference represents a reference to a module, either via import, <reference>, or implicit reference
#[derive(Clone, Default)]
pub(crate) struct ModuleReference<'a> {
    pub kind: ModuleReferenceKind,
    pub literal: Option<ast::Node>, // for import and implicit kinds (StringLiteralLike)
    pub referencing_file: Option<&'a ast::SourceFile>,
    pub r#ref: Option<&'a ast::FileReference>, // for reference kind
}

// Creates the imports map and returns an ImportTracker that uses it. Call this lazily to avoid calling `getDirectImportsMap` unnecessarily.
pub(crate) fn create_import_tracker<'a>(
    ctx: &core::Context,
    program: &compiler::Program,
    source_files: &'a [&'a ast::SourceFile],
    source_files_set: &collections::Set<String>,
    checker: &'a mut checker::Checker<'a, '_>,
) -> ImportTracker<'a> {
    let all_direct_imports = get_direct_imports_map(ctx, program, source_files, checker);
    let source_files_owned = source_files.to_vec();
    let source_files_set = source_files_set.clone();
    Box::new(
        move |_export_symbol: ast::SymbolIdentity,
              export_info: &ExportInfo,
              is_for_rename: bool| {
            let (direct_imports, indirect_users) = get_importers_for_export(
                &source_files_owned,
                &source_files_set,
                &all_direct_imports,
                export_info,
                checker,
            );
            let (import_searches, single_references) = get_searches_from_direct_imports(
                &source_files_owned,
                &direct_imports,
                _export_symbol,
                export_info.export_kind,
                checker,
                is_for_rename,
            );
            ImportsResult {
                import_searches,
                single_references,
                indirect_users,
            }
        },
    )
}

// Returns a map from a module symbol to all import statements that directly reference the module
pub(crate) fn get_direct_imports_map(
    ctx: &core::Context,
    program: &compiler::Program,
    source_files: &[&ast::SourceFile],
    checker: &mut checker::Checker,
) -> std::collections::HashMap<ast::SymbolIdentity, Vec<ast::Node>> {
    let mut result = std::collections::HashMap::new();
    for source_file in source_files {
        if ctx.err().is_some() {
            return result;
        }
        for_each_import(
            program,
            source_file,
            |_store, import_decl, module_specifier| {
                if let Some(module_symbol) = checker.get_symbol_at_location_public(module_specifier)
                {
                    result
                        .entry(module_symbol)
                        .or_insert_with(Vec::new)
                        .push(import_decl);
                }
            },
        );
    }
    result
}

// Calls `action` for each import, re-export, or require() in a file
pub(crate) fn for_each_import(
    program: &compiler::Program,
    source_file: &ast::SourceFile,
    mut action: impl FnMut(&ast::AstStore, ast::Node, ast::Node),
) {
    let store = source_file.store();
    let mut implicit_imports: Vec<ast::Node> = Vec::new();
    let (_, jsx_specifier) = program.get_jsx_runtime_import_specifier(source_file.path());
    if let Some(jsx_specifier) = jsx_specifier {
        implicit_imports.push(jsx_specifier);
    }
    let import_helpers_specifier = program.get_import_helpers_import_specifier(source_file.path());
    if let Some(import_helpers_specifier) = import_helpers_specifier {
        implicit_imports.push(import_helpers_specifier);
    }
    if source_file.external_module_indicator().is_some()
        || source_file.imports().len() + implicit_imports.len() != 0
    {
        for i in source_file.imports() {
            let import_from = ast::try_get_import_from_module_specifier(store, &i).unwrap();
            action(store, import_from, *i);
        }
        for i in &implicit_imports {
            let import_from = ast::try_get_import_from_module_specifier(store, i).unwrap();
            action(store, import_from, *i);
        }
    } else {
        let source_file_node = source_file.as_node();
        for_each_possible_import_or_export_statement(store, source_file_node, |node| {
            match store.kind(node) {
                ast::Kind::ExportDeclaration
                | ast::Kind::ImportDeclaration
                | ast::Kind::JSImportDeclaration => {
                    if let Some(specifier) = store.module_specifier(node) {
                        if ast::is_string_literal(store, specifier) {
                            action(store, node, specifier);
                        }
                    }
                }
                ast::Kind::ImportEqualsDeclaration => {
                    if is_external_module_import_equals(store, node) {
                        let module_reference = store.module_reference(node).unwrap();
                        let expression = store.expression(module_reference).unwrap();
                        action(store, node, expression);
                    }
                }
                _ => {}
            }
            false
        });
    }
}

pub(crate) fn for_each_possible_import_or_export_statement(
    store: &ast::AstStore,
    source_file_like: ast::Node,
    mut action: impl FnMut(ast::Node) -> bool,
) -> bool {
    for statement in get_statements_of_source_file_like(store, source_file_like) {
        if action(statement)
            || (is_ambient_module_declaration(store, statement)
                && for_each_possible_import_or_export_statement(store, statement, &mut action))
        {
            return true;
        }
    }
    false
}

pub(crate) fn get_source_file_like_for_import_declaration(
    store: &ast::AstStore,
    node: ast::Node,
) -> ast::Node {
    if ast::is_call_expression(store, node) {
        let source_file = ast::get_source_file_of_node(store, Some(node)).unwrap();
        return source_file;
    }
    let parent = store.parent(node);
    if parent
        .as_ref()
        .is_some_and(|parent| ast::is_source_file(store, *parent))
    {
        return parent.unwrap();
    }
    debug::assert(
        parent.as_ref().is_some_and(|parent| {
            ast::is_module_block(store, *parent)
                && store
                    .parent(*parent)
                    .as_ref()
                    .is_some_and(|parent| is_ambient_module_declaration(store, *parent))
        }),
        None,
    );
    store.parent(parent.unwrap()).unwrap()
}

pub(crate) fn is_ambient_module_declaration(store: &ast::AstStore, node: ast::Node) -> bool {
    ast::is_module_declaration(store, node)
        && store
            .name(node)
            .as_ref()
            .is_some_and(|name| ast::is_string_literal(store, *name))
}

pub(crate) fn get_statements_of_source_file_like(
    store: &ast::AstStore,
    node: ast::Node,
) -> Vec<ast::Node> {
    if ast::is_source_file(store, node) {
        return store.parser_access().source_file_statement_nodes(node);
    }
    if let Some(body) = store.body(node) {
        return store
            .statements(body)
            .map(|statements| statements.iter().collect())
            .unwrap_or_default();
    }
    Vec::new()
}

pub(crate) fn get_importers_for_export<'a>(
    source_files: &[&'a ast::SourceFile],
    source_files_set: &collections::Set<String>,
    all_direct_imports: &std::collections::HashMap<ast::SymbolIdentity, Vec<ast::Node>>,
    export_info: &ExportInfo,
    checker: &mut checker::Checker,
) -> (Vec<ast::Node>, Vec<&'a ast::SourceFile>) {
    let direct_imports: RefCell<Vec<ast::Node>> = RefCell::new(Vec::new());
    let indirect_user_declarations: RefCell<Vec<ast::Node>> = RefCell::new(Vec::new());
    let mark_seen_direct_import =
        RefCell::new(Box::new(node_seen_tracker()) as Box<dyn FnMut(ast::Node) -> bool>);
    let mark_seen_indirect_user =
        RefCell::new(Box::new(node_seen_tracker()) as Box<dyn FnMut(ast::Node) -> bool>);
    let is_available_through_global = export_info
        .exporting_module_symbol
        .and_then(|symbol| checker.symbol_value_declaration_public(symbol))
        .as_ref()
        .is_some_and(|decl| {
            let store = store_for_node(source_files, *decl);
            is_source_file_with_global_exports(store, *decl, checker)
        });

    let get_direct_imports = |module_symbol: ast::SymbolIdentity| -> Vec<ast::Node> {
        all_direct_imports
            .get(&module_symbol)
            .cloned()
            .unwrap_or_default()
    };

    let is_exported = |mut node: Option<ast::Node>, stop_at_ambient_module: bool| -> bool {
        while let Some(current) = node {
            let store = store_for_node(source_files, current);
            if stop_at_ambient_module && is_ambient_module_declaration(store, current) {
                break;
            }
            if ast::has_syntactic_modifier(store, current, ast::MODIFIER_FLAGS_EXPORT) {
                return true;
            }
            node = store.parent(current);
        }
        false
    };

    let mut module_stack = Vec::new();
    if let Some(exporting_module_symbol) = export_info.exporting_module_symbol.clone() {
        module_stack.push(exporting_module_symbol);
    }
    while let Some(exporting_module_symbol) = module_stack.pop() {
        let these_direct_imports = get_direct_imports(exporting_module_symbol);
        for direct in these_direct_imports {
            if !(mark_seen_direct_import.borrow_mut())(direct) {
                continue;
            }
            let direct_store = store_for_node(source_files, direct);
            match direct_store.kind(direct) {
                ast::Kind::CallExpression => {
                    if ast::is_import_call(direct_store, direct) {
                        add_indirect_user_for_import_call(
                            source_files,
                            all_direct_imports,
                            checker,
                            &mark_seen_indirect_user,
                            &indirect_user_declarations,
                            is_available_through_global,
                            direct,
                            is_exported(Some(direct), true),
                        );
                    } else if !is_available_through_global {
                        let parent = direct_store.parent(direct).unwrap();
                        if export_info.export_kind == ExportKind::ExportEquals
                            && ast::is_variable_declaration(direct_store, parent)
                        {
                            let name = direct_store.name(parent).unwrap();
                            if ast::is_identifier(direct_store, name) {
                                direct_imports.borrow_mut().push(name);
                            }
                        }
                    }
                }
                ast::Kind::Identifier => {}
                ast::Kind::ImportEqualsDeclaration => {
                    if export_info.export_kind == ExportKind::ExportEquals {
                        direct_imports.borrow_mut().push(direct);
                    } else if !is_available_through_global {
                        let source_file_like =
                            get_source_file_like_for_import_declaration(direct_store, direct);
                        debug::assert(
                            ast::is_source_file(direct_store, source_file_like)
                                || ast::is_module_declaration(direct_store, source_file_like),
                            None,
                        );
                        let name = direct_store.name(direct).unwrap();
                        let is_re_export = ast::has_syntactic_modifier(
                            direct_store,
                            direct,
                            ast::MODIFIER_FLAGS_EXPORT,
                        );
                        let has_namespace_re_export = find_namespace_re_exports(
                            direct_store,
                            source_file_like,
                            name,
                            checker,
                        );
                        add_indirect_user(
                            source_files,
                            all_direct_imports,
                            checker,
                            &mark_seen_indirect_user,
                            &indirect_user_declarations,
                            is_available_through_global,
                            source_file_like,
                            is_re_export || has_namespace_re_export,
                        );
                    }
                }
                ast::Kind::ImportDeclaration | ast::Kind::JSImportDeclaration => {
                    direct_imports.borrow_mut().push(direct);
                    if let Some(import_clause) = direct_store.import_clause(direct) {
                        if let Some(named_bindings) = direct_store.named_bindings(import_clause) {
                            if ast::is_namespace_import(direct_store, named_bindings) {
                                if export_info.export_kind != ExportKind::ExportEquals
                                    && !is_available_through_global
                                {
                                    let source_file_like =
                                        get_source_file_like_for_import_declaration(
                                            direct_store,
                                            direct,
                                        );
                                    debug::assert(
                                        ast::is_source_file(direct_store, source_file_like)
                                            || ast::is_module_declaration(
                                                direct_store,
                                                source_file_like,
                                            ),
                                        None,
                                    );
                                    let name = direct_store.name(named_bindings).unwrap();
                                    let has_namespace_re_export = find_namespace_re_exports(
                                        direct_store,
                                        source_file_like,
                                        name,
                                        checker,
                                    );
                                    add_indirect_user(
                                        source_files,
                                        all_direct_imports,
                                        checker,
                                        &mark_seen_indirect_user,
                                        &indirect_user_declarations,
                                        is_available_through_global,
                                        source_file_like,
                                        has_namespace_re_export,
                                    );
                                }
                                continue;
                            }
                        }
                    }
                    if !is_available_through_global && ast::is_default_import(direct_store, &direct)
                    {
                        let source_file_like =
                            get_source_file_like_for_import_declaration(direct_store, direct);
                        add_indirect_user(
                            source_files,
                            all_direct_imports,
                            checker,
                            &mark_seen_indirect_user,
                            &indirect_user_declarations,
                            is_available_through_global,
                            source_file_like,
                            false,
                        );
                    }
                }
                ast::Kind::ExportDeclaration => {
                    let export_clause = direct_store.export_clause(direct);
                    if export_clause.is_none() {
                        if let Some(containing_module_symbol) =
                            get_containing_module_symbol(source_files, direct, checker)
                        {
                            module_stack.push(containing_module_symbol);
                        }
                    } else if ast::is_namespace_export(direct_store, export_clause.unwrap()) {
                        let source_file_like =
                            get_source_file_like_for_import_declaration(direct_store, direct);
                        add_indirect_user(
                            source_files,
                            all_direct_imports,
                            checker,
                            &mark_seen_indirect_user,
                            &indirect_user_declarations,
                            is_available_through_global,
                            source_file_like,
                            true,
                        );
                    } else {
                        direct_imports.borrow_mut().push(direct);
                    }
                }
                ast::Kind::ImportType => {
                    if !is_available_through_global
                        && direct_store.is_type_of(direct).unwrap_or(false)
                        && direct_store.qualifier(direct).is_none()
                        && is_exported(Some(direct), false)
                    {
                        let source_file =
                            ast::get_source_file_of_node(direct_store, Some(direct)).unwrap();
                        add_indirect_user(
                            source_files,
                            all_direct_imports,
                            checker,
                            &mark_seen_indirect_user,
                            &indirect_user_declarations,
                            is_available_through_global,
                            source_file,
                            true,
                        );
                    }
                    direct_imports.borrow_mut().push(direct);
                }
                _ => debug::fail("Unexpected import kind."),
            }
        }
    }

    let indirect_users = if is_available_through_global {
        source_files.to_vec()
    } else {
        if let Some(exporting_module_symbol) = export_info.exporting_module_symbol {
            for decl in checker.collect_symbol_declarations_public(exporting_module_symbol) {
                let store = store_for_node(source_files, decl);
                if ast::is_external_module_augmentation(store, &decl)
                    && source_files_set.has(
                        &ast::get_source_file_of_node(store, Some(decl))
                            .map(|source_file| {
                                store
                                    .as_source_file(source_file)
                                    .file_name_ref()
                                    .to_string()
                            })
                            .unwrap_or_default(),
                    )
                {
                    add_indirect_user(
                        source_files,
                        all_direct_imports,
                        checker,
                        &mark_seen_indirect_user,
                        &indirect_user_declarations,
                        is_available_through_global,
                        decl,
                        false,
                    );
                }
            }
        }
        indirect_user_declarations
            .borrow()
            .iter()
            .map(|node| source_file_for_node(source_files, *node))
            .collect()
    };
    (direct_imports.into_inner(), indirect_users)
}

fn add_indirect_user_for_import_call<'a>(
    source_files: &[&'a ast::SourceFile],
    all_direct_imports: &std::collections::HashMap<ast::SymbolIdentity, Vec<ast::Node>>,
    checker: &mut checker::Checker<'_, '_>,
    mark_seen_indirect_user: &RefCell<Box<dyn FnMut(ast::Node) -> bool>>,
    indirect_user_declarations: &RefCell<Vec<ast::Node>>,
    is_available_through_global: bool,
    import_call: ast::Node,
    add_transitive_dependencies: bool,
) {
    let store = store_for_node(source_files, import_call);
    let parent = store.parent(import_call);
    let top = ast::find_ancestor(store, parent, |store, node| {
        is_ambient_module_declaration(store, node)
    });
    let top =
        top.unwrap_or_else(|| ast::get_source_file_of_node(store, Some(import_call)).unwrap());
    add_indirect_user(
        source_files,
        all_direct_imports,
        checker,
        mark_seen_indirect_user,
        indirect_user_declarations,
        is_available_through_global,
        top,
        add_transitive_dependencies,
    );
}

fn add_indirect_user<'a>(
    source_files: &[&'a ast::SourceFile],
    all_direct_imports: &std::collections::HashMap<ast::SymbolIdentity, Vec<ast::Node>>,
    checker: &mut checker::Checker<'_, '_>,
    mark_seen_indirect_user: &RefCell<Box<dyn FnMut(ast::Node) -> bool>>,
    indirect_user_declarations: &RefCell<Vec<ast::Node>>,
    is_available_through_global: bool,
    source_file_like: ast::Node,
    add_transitive_dependencies: bool,
) {
    let mut stack = vec![(source_file_like, add_transitive_dependencies)];
    while let Some((source_file_like, add_transitive_dependencies)) = stack.pop() {
        if is_available_through_global {
            continue;
        }
        if !(mark_seen_indirect_user.borrow_mut())(source_file_like) {
            continue;
        }
        indirect_user_declarations
            .borrow_mut()
            .push(source_file_like);
        if !add_transitive_dependencies {
            continue;
        }
        let Some(module_symbol) = checker.source_node_symbol_public(source_file_like) else {
            continue;
        };
        debug::assert(
            checker
                .symbol_flags_public(module_symbol)
                .is_some_and(|flags| flags & ast::SYMBOL_FLAGS_MODULE != 0),
            None,
        );
        for direct_import in all_direct_imports
            .get(&module_symbol)
            .cloned()
            .unwrap_or_default()
        {
            let direct_store = store_for_node(source_files, direct_import);
            if !ast::is_import_type_node(direct_store, direct_import) {
                let source_file_like =
                    get_source_file_like_for_import_declaration(direct_store, direct_import);
                stack.push((source_file_like, true));
            }
        }
    }
}

pub(crate) fn get_containing_module_symbol(
    source_files: &[&ast::SourceFile],
    importer: ast::Node,
    checker: &mut checker::Checker<'_, '_>,
) -> Option<ast::SymbolIdentity> {
    let store = store_for_node(source_files, importer);
    let source_file_like = get_source_file_like_for_import_declaration(store, importer);
    checker.source_node_symbol_public(source_file_like)
}

// Returns 'true' is the namespace 'name' is re-exported from this module, and 'false' if it is only used locally
pub(crate) fn find_namespace_re_exports<'a>(
    store: &ast::AstStore,
    source_file_like: ast::Node,
    name: ast::Node,
    checker: &mut checker::Checker<'a, '_>,
) -> bool {
    let namespace_import_symbol = checker.get_symbol_at_location_public(name);
    for_each_possible_import_or_export_statement(store, source_file_like, |statement| {
        if !ast::is_export_declaration(store, statement) {
            return false;
        }
        let export_clause = store.export_clause(statement);
        let module_specifier = store.module_specifier(statement);
        module_specifier.is_none()
            && export_clause.is_some()
            && ast::is_named_exports(store, export_clause.unwrap())
            && {
                store
                    .elements(export_clause.unwrap())
                    .is_some_and(|elements| {
                        elements.iter().any(|element| {
                            checker.get_export_specifier_local_target_symbol_public(element)
                                == namespace_import_symbol
                        })
                    })
            }
    })
}

pub(crate) fn get_searches_from_direct_imports(
    source_files: &[&ast::SourceFile],
    direct_imports: &[ast::Node],
    export_symbol: ast::SymbolIdentity,
    export_kind: ExportKind,
    checker: &mut checker::Checker,
    is_for_rename: bool,
) -> (Vec<LocationAndSymbol>, Vec<ast::Node>) {
    let import_searches: RefCell<Vec<LocationAndSymbol>> = RefCell::new(Vec::new());
    let single_references: RefCell<Vec<ast::Node>> = RefCell::new(Vec::new());
    let Some(export_name) = checker.symbol_name_public(export_symbol) else {
        return (Vec::new(), Vec::new());
    };

    let mut handle_import = |decl: ast::Node| {
        let store = store_for_node(source_files, decl);
        if ast::is_import_equals_declaration(store, decl) {
            if is_external_module_import_equals(store, decl) {
                let name = store.name(decl).unwrap();
                handle_namespace_import_like(
                    store,
                    &import_searches,
                    name,
                    &export_name,
                    export_kind,
                    checker,
                    is_for_rename,
                );
            }
            return;
        }
        if ast::is_identifier(store, decl) {
            handle_namespace_import_like(
                store,
                &import_searches,
                decl,
                &export_name,
                export_kind,
                checker,
                is_for_rename,
            );
            return;
        }
        if ast::is_import_type_node(store, decl) {
            if let Some(qualifier) = store.qualifier(decl) {
                if let Some(first_identifier) = ast::get_first_identifier(store, &qualifier)
                    && store.text(first_identifier) == export_name
                {
                    single_references.borrow_mut().push(first_identifier);
                }
            } else if export_kind == ExportKind::ExportEquals {
                let argument = store.argument(decl).unwrap();
                let literal = store.literal(argument).unwrap();
                single_references.borrow_mut().push(literal);
            }
            return;
        }
        if store
            .module_specifier(decl)
            .is_none_or(|specifier| !ast::is_string_literal(store, specifier))
        {
            return;
        }
        if ast::is_export_declaration(store, decl) {
            if let Some(export_clause) = store.export_clause(decl) {
                if ast::is_named_exports(store, export_clause) {
                    search_for_named_import(
                        store,
                        &import_searches,
                        &single_references,
                        export_clause,
                        &export_name,
                        export_kind,
                        checker,
                        is_for_rename,
                    );
                }
            }
            return;
        }
        if let Some(import_clause) = store.import_clause(decl) {
            if let Some(named_bindings) = store.named_bindings(import_clause) {
                match store.kind(named_bindings) {
                    ast::Kind::NamespaceImport => handle_namespace_import_like(
                        store,
                        &import_searches,
                        store.name(named_bindings).unwrap(),
                        &export_name,
                        export_kind,
                        checker,
                        is_for_rename,
                    ),
                    ast::Kind::NamedImports => {
                        if export_kind == ExportKind::Named || export_kind == ExportKind::Default {
                            search_for_named_import(
                                store,
                                &import_searches,
                                &single_references,
                                named_bindings,
                                &export_name,
                                export_kind,
                                checker,
                                is_for_rename,
                            );
                        }
                    }
                    _ => {}
                }
            }
            if let Some(name) = store.name(import_clause) {
                if (export_kind == ExportKind::Default || export_kind == ExportKind::ExportEquals)
                    && (!is_for_rename
                        || store.text(name)
                            == symbol_name_no_default(source_files, checker, export_symbol))
                {
                    let symbol = checker.get_symbol_at_location_public(name);
                    add_import_search(&import_searches, name, symbol);
                }
            }
        }
    };

    for decl in direct_imports {
        handle_import(*decl);
    }
    (import_searches.into_inner(), single_references.into_inner())
}

fn add_import_search(
    import_searches: &RefCell<Vec<LocationAndSymbol>>,
    location: ast::Node,
    symbol: Option<ast::SymbolIdentity>,
) {
    import_searches.borrow_mut().push(LocationAndSymbol {
        import_location: Some(location),
        import_symbol: symbol,
    });
}

fn is_import_name_match(export_name: &str, export_kind: ExportKind, name: &str) -> bool {
    name == export_name
        || (export_kind != ExportKind::Named && name == ast::INTERNAL_SYMBOL_NAME_DEFAULT)
}

fn handle_namespace_import_like(
    store: &ast::AstStore,
    import_searches: &RefCell<Vec<LocationAndSymbol>>,
    import_name: ast::Node,
    export_name: &str,
    export_kind: ExportKind,
    checker: &mut checker::Checker,
    is_for_rename: bool,
) {
    if export_kind == ExportKind::ExportEquals
        && (!is_for_rename
            || is_import_name_match(export_name, export_kind, &store.text(import_name)))
    {
        let symbol = checker.get_symbol_at_location_public(import_name);
        add_import_search(import_searches, import_name, symbol);
    }
}

fn search_for_named_import(
    store: &ast::AstStore,
    import_searches: &RefCell<Vec<LocationAndSymbol>>,
    single_references: &RefCell<Vec<ast::Node>>,
    named_bindings: ast::Node,
    export_name: &str,
    export_kind: ExportKind,
    checker: &mut checker::Checker,
    is_for_rename: bool,
) {
    let Some(elements) = store.elements(named_bindings) else {
        return;
    };
    for element in elements.iter() {
        let name = store.name(element).unwrap();
        let property_name = store.property_name(element);
        let match_name_node = property_name.as_ref().unwrap_or(&name);
        let match_name = store.text(*match_name_node);
        if !is_import_name_match(export_name, export_kind, &match_name) {
            continue;
        }
        if let Some(property_name) = property_name {
            single_references.borrow_mut().push(property_name);
            if !is_for_rename || store.text(name) == export_name {
                let symbol = checker.get_symbol_at_location_public(name);
                add_import_search(import_searches, name, symbol);
            }
        } else {
            let local_symbol = if ast::is_export_specifier(store, element)
                && store.property_name(element).is_some()
            {
                checker.get_export_specifier_local_target_symbol_public(element)
            } else {
                checker.get_symbol_at_location_public(name)
            };
            add_import_search(import_searches, name, local_symbol);
        }
    }
}

fn get_export_assignment_export(
    store: &ast::AstStore,
    ex: ast::Node,
    checker: &mut checker::Checker<'_, '_>,
) -> Option<ImportExportSymbol> {
    let export_assignment_symbol = checker.source_node_declaration_symbol_public(ex)?;
    let parent_symbol = checker.symbol_parent_public(export_assignment_symbol)?;
    let export_kind = if store.is_export_equals(ex).unwrap_or(false) {
        ExportKind::ExportEquals
    } else {
        ExportKind::Default
    };
    Some(ImportExportSymbol {
        kind: ImpExpKind::Export,
        symbol: None,
        export_info: Some(ExportInfo {
            exporting_module_symbol: Some(parent_symbol),
            export_kind,
        }),
    })
}

pub(crate) fn get_import_or_export_symbol<'a>(
    store: &'a ast::AstStore,
    source_files: &[&'a ast::SourceFile],
    node: ast::Node,
    symbol: ast::SymbolIdentity,
    checker: &mut checker::Checker<'a, '_>,
    coming_from_export: bool,
) -> Option<ImportExportSymbol> {
    let symbol_flags = checker
        .symbol_flags_public(symbol)
        .unwrap_or(ast::SYMBOL_FLAGS_NONE);
    let symbol_declarations = checker.collect_symbol_declarations_public(symbol);
    let symbol_name = checker.symbol_name_public(symbol).unwrap_or_default();

    let get_export_kind_for_declaration = |node: ast::Node| -> ExportKind {
        if ast::has_syntactic_modifier(store, node, ast::MODIFIER_FLAGS_DEFAULT) {
            ExportKind::Default
        } else {
            ExportKind::Named
        }
    };

    let export = {
        let parent = store.parent(node)?;
        let grandparent = store.parent(parent);
        if let Some(export_symbol) = checker.symbol_export_symbol_public(symbol) {
            if ast::is_property_access_expression(store, parent) {
                if grandparent
                    .as_ref()
                    .is_some_and(|grandparent| ast::is_binary_expression(store, *grandparent))
                    && symbol_declarations.contains(&parent)
                {
                    return get_special_property_export(
                        store,
                        &grandparent.unwrap(),
                        false,
                        symbol,
                        checker,
                    );
                }
                return None;
            }
            return make_import_export_symbol(
                Some(export_symbol),
                export_symbol,
                get_export_kind_for_declaration(parent),
                checker,
            );
        }

        let export_node = get_export_node(store, parent, node);
        match export_node {
            Some(export_node)
                if ast::has_syntactic_modifier(store, export_node, ast::MODIFIER_FLAGS_EXPORT)
                    || ast::is_implicitly_exported_js_type_alias(store, export_node) =>
            {
                if ast::is_import_equals_declaration(store, export_node)
                    && store
                        .module_reference(export_node)
                        .as_ref()
                        .is_some_and(|module_reference| *module_reference == node)
                {
                    if coming_from_export {
                        return None;
                    }
                    let lhs_name = store.name(export_node).unwrap();
                    let lhs_symbol = checker.get_symbol_at_location_public(lhs_name)?;
                    return Some(ImportExportSymbol {
                        kind: ImpExpKind::Import,
                        symbol: Some(lhs_symbol),
                        export_info: None,
                    });
                }
                make_import_export_symbol(
                    None,
                    symbol,
                    get_export_kind_for_declaration(export_node),
                    checker,
                )
            }
            _ if ast::is_namespace_export(store, parent) => {
                make_import_export_symbol(None, symbol, ExportKind::Named, checker)
            }
            _ if ast::is_export_assignment(store, parent) => {
                get_export_assignment_export(store, parent, checker)
            }
            _ if grandparent
                .as_ref()
                .is_some_and(|grandparent| ast::is_export_assignment(store, *grandparent)) =>
            {
                get_export_assignment_export(store, grandparent.unwrap(), checker)
            }
            _ if ast::is_binary_expression(store, parent) => {
                get_special_property_export(store, &parent, true, symbol, checker)
            }
            _ if grandparent
                .as_ref()
                .is_some_and(|grandparent| ast::is_binary_expression(store, *grandparent)) =>
            {
                get_special_property_export(store, &grandparent.unwrap(), true, symbol, checker)
            }
            _ => None,
        }
    };

    if export.is_some() || coming_from_export {
        return export;
    }

    let import = {
        if !is_node_import(store, node) {
            return None;
        }
        let mut imported_symbol = if symbol_flags & ast::SYMBOL_FLAGS_ALIAS != 0 {
            symbol_declarations
                .iter()
                .find_map(|declaration| checker.get_symbol_at_location_public(*declaration))
                .and_then(|symbol| checker.get_immediate_aliased_symbol_public(symbol))
        } else {
            get_property_symbol_of_object_binding_pattern_without_property_name(
                source_files,
                symbol,
                checker,
            )
        }?;
        imported_symbol = skip_export_specifier_symbol(source_files, imported_symbol, checker)?;
        if checker
            .symbol_name_public(imported_symbol)
            .is_some_and(|name| name == "export=")
        {
            imported_symbol =
                get_export_equals_local_symbol(source_files, imported_symbol, checker)?;
        }
        let imported_name = symbol_name_no_default(source_files, checker, imported_symbol);
        if imported_name.is_empty()
            || imported_name == ast::INTERNAL_SYMBOL_NAME_DEFAULT
            || imported_name == symbol_name
        {
            return Some(ImportExportSymbol {
                kind: ImpExpKind::Import,
                symbol: Some(imported_symbol.clone()),
                export_info: None,
            });
        }
        None
    };
    import
}

fn make_import_export_symbol(
    symbol_identity: Option<ast::SymbolIdentity>,
    export_symbol: ast::SymbolIdentity,
    kind: ExportKind,
    checker: &mut checker::Checker,
) -> Option<ImportExportSymbol> {
    get_export_info_from_identity(export_symbol, kind, checker).map(|export_info| {
        ImportExportSymbol {
            kind: ImpExpKind::Export,
            symbol: symbol_identity,
            export_info: Some(export_info),
        }
    })
}

fn get_special_property_export(
    store: &ast::AstStore,
    node: &ast::Node,
    use_lhs_symbol: bool,
    symbol: ast::SymbolIdentity,
    checker: &mut checker::Checker,
) -> Option<ImportExportSymbol> {
    let kind = match ast::get_assignment_declaration_kind(store, *node) {
        ast::JSDeclarationKind::ExportsProperty => ExportKind::Named,
        ast::JSDeclarationKind::ModuleExports => ExportKind::ExportEquals,
        _ => return None,
    };
    if use_lhs_symbol {
        let left = store.left(*node)?;
        debug::assert(ast::is_access_expression(store, left), None);
        let name = store
            .name(left)
            .or_else(|| store.argument_expression(left))?;
        let symbol_identity = checker.get_symbol_at_location_public(name)?;
        return make_import_export_symbol_from_identity(symbol_identity, kind, checker);
    }
    make_import_export_symbol(None, symbol, kind, checker)
}

pub(crate) fn get_export_info(
    export_symbol: ast::SymbolIdentity,
    export_kind: ExportKind,
    checker: &mut checker::Checker,
) -> Option<ExportInfo> {
    get_export_info_from_identity(export_symbol, export_kind, checker)
}

fn make_import_export_symbol_from_identity(
    symbol: ast::SymbolIdentity,
    kind: ExportKind,
    checker: &mut checker::Checker,
) -> Option<ImportExportSymbol> {
    get_export_info_from_identity(symbol, kind, checker).map(|export_info| ImportExportSymbol {
        kind: ImpExpKind::Export,
        symbol: Some(symbol),
        export_info: Some(export_info),
    })
}

fn get_export_info_from_identity(
    export_symbol: ast::SymbolIdentity,
    export_kind: ExportKind,
    checker: &mut checker::Checker,
) -> Option<ExportInfo> {
    let parent = checker
        .symbol_parent_public(export_symbol)
        .and_then(|parent| checker.get_merged_symbol_public(parent))?;
    if checker.is_external_module_symbol_public(parent) {
        return Some(ExportInfo {
            exporting_module_symbol: Some(parent),
            export_kind,
        });
    }
    None
}

// If a reference is a class expression, the exported node would be its parent.
// If a reference is a variable declaration, the exported node would be the variable statement.
pub(crate) fn get_export_node(
    store: &ast::AstStore,
    parent: ast::Node,
    node: ast::Node,
) -> Option<ast::Node> {
    let declaration = if ast::is_variable_declaration(store, parent) {
        Some(parent)
    } else if ast::is_binding_element(store, parent) {
        ast::walk_up_binding_elements_and_patterns(store, &parent)
    } else {
        None
    };
    if let Some(declaration) = declaration {
        if store
            .name(parent)
            .as_ref()
            .is_some_and(|name| *name == node)
            && !ast::is_catch_clause(store, store.parent(declaration).unwrap())
            && ast::is_variable_statement(
                store,
                store.parent(store.parent(declaration).unwrap()).unwrap(),
            )
        {
            return Some(store.parent(store.parent(declaration).unwrap()).unwrap());
        }
        return None;
    }
    Some(parent)
}

pub(crate) fn is_node_import(store: &ast::AstStore, node: ast::Node) -> bool {
    let Some(parent) = store.parent(node) else {
        return false;
    };
    match store.kind(parent) {
        ast::Kind::ImportEqualsDeclaration => {
            store
                .name(parent)
                .as_ref()
                .is_some_and(|name| *name == node)
                && is_external_module_import_equals(store, parent)
        }
        ast::Kind::ImportSpecifier => store.property_name(parent).is_none(),
        ast::Kind::ImportClause | ast::Kind::NamespaceImport => {
            debug::assert(
                store
                    .name(parent)
                    .as_ref()
                    .is_some_and(|name| *name == node),
                None,
            );
            true
        }
        ast::Kind::BindingElement => {
            ast::is_in_js_file(store, node)
                && store
                    .parent(parent)
                    .and_then(|parent| store.parent(parent))
                    .as_ref()
                    .is_some_and(|declaration| {
                        ast::is_variable_declaration_initialized_to_bare_or_accessed_require(
                            store,
                            declaration,
                        )
                    })
        }
        _ => false,
    }
}

pub(crate) fn is_external_module_import_equals(store: &ast::AstStore, node: ast::Node) -> bool {
    let module_reference = store.module_reference(node);
    module_reference
        .is_some_and(|module_reference| ast::is_external_module_reference(store, module_reference))
        && store
            .module_reference(node)
            .and_then(|module_reference| store.expression(module_reference))
            .is_some_and(|expr| store.kind(expr) == ast::Kind::StringLiteral)
}

// If at an export specifier, go to the symbol it refers to. */
pub(crate) fn skip_export_specifier_symbol<'a>(
    source_files: &[&'a ast::SourceFile],
    symbol: ast::SymbolIdentity,
    checker: &mut checker::Checker<'a, '_>,
) -> Option<ast::SymbolIdentity> {
    let declarations = checker.collect_symbol_declarations_public(symbol);
    for declaration in declarations {
        let store = store_for_node(source_files, declaration);
        match store.kind(declaration) {
            ast::Kind::ExportSpecifier
                if store.property_name(declaration).is_none()
                    && store
                        .parent(declaration)
                        .and_then(|parent| store.parent(parent))
                        .and_then(|parent| store.module_specifier(parent))
                        .is_none() =>
            {
                return Some(
                    checker
                        .get_export_specifier_local_target_symbol_public(declaration)
                        .unwrap_or(symbol),
                );
            }
            ast::Kind::PropertyAccessExpression
                if store
                    .expression(declaration)
                    .as_ref()
                    .is_some_and(|expression| {
                        ast::is_module_exports_access_expression(store, *expression)
                    })
                    && !ast::is_private_identifier(store, store.name(declaration).unwrap()) =>
            {
                return checker.get_symbol_at_location_public(declaration);
            }
            ast::Kind::ShorthandPropertyAssignment
                if store
                    .parent(declaration)
                    .and_then(|parent| store.parent(parent))
                    .as_ref()
                    .is_some_and(|parent| ast::is_binary_expression(store, *parent))
                    && ast::get_assignment_declaration_kind(
                        store,
                        store
                            .parent(declaration)
                            .and_then(|parent| store.parent(parent))
                            .unwrap(),
                    ) == ast::JSDeclarationKind::ModuleExports =>
            {
                let name = store.name(declaration).unwrap();
                return checker.get_export_specifier_local_target_symbol_public(name);
            }
            _ => {}
        }
    }
    Some(symbol)
}

pub(crate) fn get_export_equals_local_symbol<'a>(
    source_files: &[&'a ast::SourceFile],
    imported_symbol: ast::SymbolIdentity,
    checker: &mut checker::Checker<'a, '_>,
) -> Option<ast::SymbolIdentity> {
    let flags = checker
        .symbol_flags_public(imported_symbol)
        .unwrap_or(ast::SYMBOL_FLAGS_NONE);
    if flags & ast::SYMBOL_FLAGS_ALIAS != 0 {
        return checker.get_immediate_aliased_symbol_public(imported_symbol);
    }
    let decl = checker.symbol_value_declaration_public(imported_symbol)?;
    let store = store_for_node(source_files, decl);
    match store.kind(decl) {
        ast::Kind::ExportAssignment => store
            .expression(decl)
            .and_then(|expr| checker.get_symbol_at_location_public(expr)),
        ast::Kind::BinaryExpression => store
            .right(decl)
            .and_then(|right| checker.get_symbol_at_location_public(right)),
        ast::Kind::SourceFile => checker.get_symbol_at_location_public(decl),
        _ => None,
    }
}

pub(crate) fn symbol_name_no_default(
    source_files: &[&ast::SourceFile],
    checker: &mut checker::Checker<'_, '_>,
    symbol: ast::SymbolIdentity,
) -> String {
    let Some(name) = checker.symbol_name_public(symbol) else {
        return String::new();
    };
    if name != ast::INTERNAL_SYMBOL_NAME_DEFAULT {
        return name;
    }
    for decl in checker.collect_symbol_declarations_public(symbol) {
        let store = store_for_node(source_files, decl);
        let name = ast::get_name_of_declaration(store, Some(decl));
        if name
            .as_ref()
            .is_some_and(|name| ast::is_identifier(store, *name))
        {
            return store.text(name.unwrap());
        }
    }
    String::new()
}

// findModuleReferences finds all references to a module symbol across the given source files.
// This includes import statements, <reference> directives, and implicit references (e.g., JSX runtime imports).
pub(crate) fn find_module_references<'a>(
    program: &compiler::Program,
    source_files: &[&'a ast::SourceFile],
    search_module_symbol: ast::SymbolIdentity,
    checker: &mut checker::Checker,
) -> Vec<ModuleReference<'a>> {
    let mut refs = Vec::new();

    for &referencing_file in source_files {
        let search_source_file = checker.symbol_value_declaration_public(search_module_symbol);
        if let Some(search_source_file_node) = search_source_file.filter(|node| {
            let store = store_for_node(source_files, *node);
            ast::is_source_file(store, *node)
        }) {
            let search_store = store_for_node(source_files, search_source_file_node);
            let search_source_file =
                ast::get_source_file_of_node(search_store, Some(search_source_file_node)).unwrap();
            let search_file_name = search_store
                .as_source_file(search_source_file)
                .file_name()
                .to_string();
            for r#ref in referencing_file.referenced_files() {
                if program
                    .get_source_file_from_reference_ref(referencing_file, r#ref)
                    .is_some_and(|source_file| source_file.file_name() == search_file_name)
                {
                    refs.push(ModuleReference {
                        kind: ModuleReferenceKind::Reference,
                        referencing_file: Some(referencing_file),
                        r#ref: Some(r#ref),
                        ..Default::default()
                    });
                }
            }

            for r#ref in referencing_file.type_reference_directives() {
                let referenced = program
                    .get_resolved_type_reference_directive_from_type_reference_directive(
                        r#ref,
                        referencing_file,
                    );
                if referenced
                    .as_ref()
                    .is_some_and(|referenced| referenced.resolved_file_name == search_file_name)
                {
                    refs.push(ModuleReference {
                        kind: ModuleReferenceKind::Reference,
                        referencing_file: Some(referencing_file),
                        r#ref: Some(r#ref),
                        ..Default::default()
                    });
                }
            }
        }

        for_each_import(
            program,
            referencing_file,
            |store, import_decl, module_specifier| {
                let module_symbol = checker.get_symbol_at_location_public(module_specifier);
                if module_symbol
                    .as_ref()
                    .is_some_and(|module_symbol| *module_symbol == search_module_symbol)
                {
                    if ast::node_is_synthesized(store, import_decl) {
                        refs.push(ModuleReference {
                            kind: ModuleReferenceKind::Implicit,
                            literal: Some(module_specifier),
                            referencing_file: Some(referencing_file),
                            ..Default::default()
                        });
                    } else {
                        refs.push(ModuleReference {
                            kind: ModuleReferenceKind::Import,
                            literal: Some(module_specifier),
                            ..Default::default()
                        });
                    }
                }
            },
        );
    }

    refs
}

pub(crate) fn is_source_file_with_global_exports(
    store: &ast::AstStore,
    value_declaration: ast::Node,
    checker: &checker::Checker<'_, '_>,
) -> bool {
    ast::is_source_file(store, value_declaration)
        && checker.source_node_has_global_exports_public(value_declaration)
}

pub(crate) fn get_property_symbol_from_binding_element<'a>(
    checker: &mut checker::Checker<'a, '_>,
    store: &'a ast::AstStore,
    binding_element: ast::Node,
) -> Option<ast::SymbolIdentity> {
    let type_of_pattern = checker.get_type_at_location(store.parent(binding_element).unwrap());
    checker.get_property_of_type_public(
        type_of_pattern,
        &store.text(store.name(binding_element).unwrap()),
    )
}

pub(crate) fn get_property_symbol_of_object_binding_pattern_without_property_name<'a>(
    source_files: &[&'a ast::SourceFile],
    symbol: ast::SymbolIdentity,
    checker: &mut checker::Checker<'a, '_>,
) -> Option<ast::SymbolIdentity> {
    let declarations = checker.collect_symbol_declarations_public(symbol);
    let binding_element = declarations.iter().find(|declaration| {
        let store = store_for_node(source_files, **declaration);
        store.kind(**declaration) == ast::Kind::BindingElement
    })?;
    let store = store_for_node(source_files, *binding_element);
    if is_object_binding_element_without_property_name(store, binding_element) {
        return get_property_symbol_from_binding_element(checker, store, *binding_element);
    }
    None
}
