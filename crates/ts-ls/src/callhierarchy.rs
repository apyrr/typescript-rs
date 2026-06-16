use std::collections::HashMap;

use ts_ast as ast;
use ts_astnav as astnav;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as core;
use ts_lsproto::{self as lsproto, DocumentUriExt};
use ts_printer as printer;
use ts_scanner as scanner;

use crate::crossproject::{combine_incoming_calls, handle_cross_project};
use crate::findallreferences::{
    EntryKind, ReferenceEntry, SymbolAndEntriesData, SymbolEntryTransformOptions,
};
use crate::lsconv;
use crate::symbols::get_symbol_kind_from_node;
use crate::{CrossProjectOrchestrator, LanguageService};

type CallHierarchyDeclaration = ast::Node;

enum CallHierarchyDeclarationResult {
    Node(CallHierarchyDeclaration),
    Nodes(Vec<CallHierarchyDeclaration>),
}

fn combine_incoming_calls_boxed(
    results: Box<dyn Iterator<Item = lsproto::CallHierarchyIncomingCallsResponse>>,
) -> lsproto::CallHierarchyIncomingCallsResponse {
    combine_incoming_calls(results)
}

fn source_file_of_node<'a>(
    program: &'a compiler::Program,
    node: &ast::Node,
) -> &'a ast::SourceFile {
    for file in program.get_parsed_source_files_refs() {
        if file.store().store_id() == node.store_id()
            && let Some(source_file) = ast::get_source_file_of_node(file.store(), Some(*node))
            && source_file == file.as_node()
        {
            return file;
        }
    }
    panic!("node must belong to a program source file store");
}

fn store_for_node<'a>(program: &'a compiler::Program, node: &ast::Node) -> &'a ast::AstStore {
    source_file_of_node(program, node).store()
}

// Indictates whether a node is named function or class expression.
fn is_named_expression(store: &ast::AstStore, node: Option<&ast::Node>) -> bool {
    let Some(node) = node else {
        return false;
    };
    if !ast::is_function_expression(store, *node) && !ast::is_class_expression(store, *node) {
        return false;
    }
    let name = store.name(*node);
    name.is_some_and(|name| ast::is_identifier(store, name))
}

fn is_variable_like(store: &ast::AstStore, node: Option<&ast::Node>) -> bool {
    let Some(node) = node else {
        return false;
    };
    ast::is_property_declaration(store, *node) || ast::is_variable_declaration(store, *node)
}

// Indicates whether a node is a function, arrow, or class expression assigned to a constant variable or class property.
fn is_assigned_expression(store: &ast::AstStore, node: Option<&ast::Node>) -> bool {
    let Some(node) = node else {
        return false;
    };
    if !(ast::is_function_expression(store, *node)
        || ast::is_arrow_function(store, *node)
        || ast::is_class_expression(store, *node))
    {
        return false;
    }
    if store.name(*node).is_some() {
        return false;
    }
    let parent = store.parent(*node);
    let Some(parent) = parent.as_ref() else {
        return false;
    };
    if !is_variable_like(store, Some(parent)) {
        return false;
    }

    if store
        .initializer(*parent)
        .as_ref()
        .is_none_or(|initializer| initializer != node)
    {
        return false;
    }

    let name = store.name(*parent);
    if !name.is_some_and(|name| ast::is_identifier(store, name)) {
        return false;
    }

    ast::get_combined_node_flags(store, *parent).intersects(ast::NodeFlags::Const)
        || ast::is_property_declaration(store, *parent)
}

// Indicates whether a node could possibly be a call hierarchy declaration.
//
// See `resolveCallHierarchyDeclaration` for the specific rules.
fn is_possible_call_hierarchy_declaration(store: &ast::AstStore, node: Option<&ast::Node>) -> bool {
    let Some(node) = node else {
        return false;
    };
    ast::is_source_file(store, *node)
        || ast::is_module_declaration(store, *node)
        || ast::is_function_declaration(store, *node)
        || ast::is_function_expression(store, *node)
        || ast::is_class_declaration(store, *node)
        || ast::is_class_expression(store, *node)
        || ast::is_class_static_block_declaration(store, *node)
        || ast::is_method_declaration(store, *node)
        || ast::is_method_signature_declaration(store, *node)
        || ast::is_get_accessor_declaration(store, *node)
        || ast::is_set_accessor_declaration(store, *node)
}

// Indicates whether a node is a valid a call hierarchy declaration.
//
// See `resolveCallHierarchyDeclaration` for the specific rules.
fn is_valid_call_hierarchy_declaration(store: &ast::AstStore, node: Option<&ast::Node>) -> bool {
    let Some(node) = node else {
        return false;
    };

    if ast::is_source_file(store, *node) {
        return true;
    }

    if ast::is_module_declaration(store, *node) {
        return store
            .name(*node)
            .is_some_and(|name| ast::is_identifier(store, name));
    }

    ast::is_function_declaration(store, *node)
        || ast::is_class_declaration(store, *node)
        || ast::is_class_static_block_declaration(store, *node)
        || ast::is_method_declaration(store, *node)
        || ast::is_method_signature_declaration(store, *node)
        || ast::is_get_accessor_declaration(store, *node)
        || ast::is_set_accessor_declaration(store, *node)
        || is_named_expression(store, Some(node))
        || is_assigned_expression(store, Some(node))
}

// Gets the node that can be used as a reference to a call hierarchy declaration.
fn get_call_hierarchy_declaration_reference_node(
    store: &ast::AstStore,
    node: Option<&ast::Node>,
) -> Option<ast::Node> {
    let node = node?;

    if ast::is_source_file(store, *node) {
        return Some(*node);
    }

    if let Some(name) = store.name(*node) {
        return Some(name);
    }

    if is_assigned_expression(store, Some(node)) {
        return store.parent(*node).and_then(|parent| store.name(parent));
    }

    if let Some(modifiers) = store.modifiers(*node) {
        for modifier in modifiers.nodes() {
            if store.kind(modifier) == ast::Kind::DefaultKeyword {
                return Some(modifier);
            }
        }
    }

    None
}

// Gets the symbol for a call hierarchy declaration.
fn get_symbol_of_call_hierarchy_declaration(
    store: &ast::AstStore,
    c: &mut checker::Checker<'_, '_>,
    node: &ast::Node,
) -> Option<ast::SymbolIdentity> {
    if ast::is_class_static_block_declaration(store, *node) {
        return None;
    }
    let location = get_call_hierarchy_declaration_reference_node(store, Some(node))?;
    c.get_symbol_at_location_public(location)
}

// Gets the text and range for the name of a call hierarchy declaration.
fn get_call_hierarchy_item_name<'program>(
    program: &'program compiler::Program,
    c: &mut checker::Checker<'program, '_>,
    node: &ast::Node,
) -> Result<(String, i32, i32), core::Error> {
    let source_file = source_file_of_node(program, node);
    let store = source_file.store();
    if ast::is_source_file(store, *node) {
        return Ok((source_file.file_name(), 0, 0));
    }

    if (ast::is_function_declaration(store, *node) || ast::is_class_declaration(store, *node))
        && store.name(*node).is_none()
    {
        if let Some(modifiers) = store.modifiers(*node) {
            for modifier in modifiers.nodes() {
                if store.kind(modifier) == ast::Kind::DefaultKeyword {
                    let start = scanner::skip_trivia(
                        source_file.text(),
                        store.loc(modifier).pos() as usize,
                    ) as i32;
                    return Ok(("default".to_string(), start, store.loc(modifier).end()));
                }
            }
        }
    }

    if ast::is_class_static_block_declaration(store, *node) {
        let pos = scanner::skip_trivia(
            source_file.text(),
            move_range_past_modifiers(store, node).pos() as usize,
        ) as i32;
        let end = pos + 6; // "static".length
        let symbol = store
            .parent(*node)
            .and_then(|parent| c.get_symbol_at_location_public(parent));
        let prefix = symbol
            .and_then(|symbol| c.symbol_identity_to_string_public(symbol))
            .map(|symbol| format!("{symbol} "))
            .unwrap_or_default();
        return Ok((format!("{prefix}static {{}}"), pos, end));
    }

    let decl_name = if is_assigned_expression(store, Some(node)) {
        store.parent(*node).and_then(|parent| store.name(parent))
    } else {
        ast::get_name_of_declaration(store, Some(*node))
    };

    if decl_name
        .as_ref()
        .is_none_or(|decl_name| !ast::node_is_present(store, Some(*decl_name)))
    {
        if ast::is_function_declaration(store, *node) || ast::is_function_expression(store, *node) {
            let kw_pos = scanner::skip_trivia(
                source_file.text(),
                move_range_past_modifiers(store, node).pos() as usize,
            ) as i32;
            return Ok(("(anonymous)".to_string(), kw_pos, kw_pos + 8)); // "function".length
        }
        if ast::is_class_declaration(store, *node) || ast::is_class_expression(store, *node) {
            let kw_pos = scanner::skip_trivia(
                source_file.text(),
                move_range_past_modifiers(store, node).pos() as usize,
            ) as i32;
            return Ok(("(anonymous)".to_string(), kw_pos, kw_pos + 5)); // "class".length
        }
    }

    let decl_name = decl_name.unwrap();
    let text = get_text_of_call_hierarchy_name(program, c, node, &decl_name, node)?;

    let name_pos =
        scanner::skip_trivia(source_file.text(), store.loc(decl_name).pos() as usize) as i32;

    Ok((text, name_pos, store.loc(decl_name).end()))
}

fn get_text_of_call_hierarchy_name<'program>(
    program: &'program compiler::Program,
    c: &mut checker::Checker<'program, '_>,
    source_node: &ast::Node,
    name: &ast::Node,
    print_node: &ast::Node,
) -> Result<String, core::Error> {
    let source_file = source_file_of_node(program, source_node);
    let store = source_file.store();
    if ast::is_identifier(store, *name) || ast::is_string_or_numeric_literal_like(store, *name) {
        return Ok(store.text(*name));
    }
    if ast::is_computed_property_name(store, *name) {
        let expr = store.expression(*name);
        if expr.is_some_and(|expr| ast::is_string_or_numeric_literal_like(store, expr)) {
            return Ok(store.text(expr.unwrap()));
        }
    }

    let symbol = c.get_symbol_at_location_public(*name);
    if let Some(symbol) = symbol {
        let text = c
            .symbol_identity_to_string_public(symbol)
            .unwrap_or_default();
        if !text.is_empty() {
            return Ok(text);
        }
    }

    let writer = printer::share_text_writer(printer::get_single_line_string_writer());
    let mut printer = printer::new_printer(
        printer::PrinterOptions {
            remove_comments: true,
            ..printer::PrinterOptions::default()
        },
        printer::PrintHandlers::default(),
        None,
    );
    printer.write_node(Some(print_node), Some(source_file), writer.clone(), None);
    let text = writer.borrow().string();
    Ok(text)
}

fn get_call_hierarchy_item_container_name<'program>(
    program: &'program compiler::Program,
    c: &mut checker::Checker<'program, '_>,
    node: &ast::Node,
) -> Result<String, core::Error> {
    let store = store_for_node(program, node);
    if is_assigned_expression(store, Some(node)) {
        let parent = store.parent(*node).unwrap();
        if ast::is_property_declaration(store, parent)
            && store
                .parent(parent)
                .is_some_and(|parent| ast::is_class_like(store, parent))
        {
            let class_parent = store.parent(parent).unwrap();
            if ast::is_class_expression(store, class_parent) {
                if let Some(assigned_name) = ast::get_assigned_name(store, &class_parent) {
                    return get_text_of_call_hierarchy_name(
                        program,
                        c,
                        node,
                        &assigned_name,
                        &assigned_name,
                    );
                }
            } else if let Some(name) = store.name(class_parent) {
                return get_text_of_call_hierarchy_name(program, c, node, &name, &name);
            }
        }
        if store
            .parent(parent)
            .and_then(|p| store.parent(p))
            .and_then(|p| store.parent(p))
            .is_some_and(|node| ast::is_module_block(store, node))
        {
            let mod_parent = store
                .parent(
                    store
                        .parent(store.parent(store.parent(parent).unwrap()).unwrap())
                        .unwrap(),
                )
                .unwrap();
            if ast::is_module_declaration(store, mod_parent) {
                if let Some(name) = store.name(mod_parent) {
                    if ast::is_identifier(store, name) {
                        return Ok(store.text(name));
                    }
                }
            }
        }
        return Ok(String::new());
    }

    match store.kind(*node) {
        ast::Kind::GetAccessor | ast::Kind::SetAccessor | ast::Kind::MethodDeclaration => {
            if store
                .parent(*node)
                .is_some_and(|node| ast::is_object_literal_expression(store, node))
            {
                if let Some(parent) = store.parent(*node)
                    && let Some(assigned_name) = ast::get_assigned_name(store, &parent)
                {
                    return get_text_of_call_hierarchy_name(
                        program,
                        c,
                        node,
                        &assigned_name,
                        &assigned_name,
                    );
                }
            }
            let parent = store.parent(*node);
            if let Some(name) = ast::get_name_of_declaration(store, parent) {
                return get_text_of_call_hierarchy_name(program, c, node, &name, &name);
            }
        }
        ast::Kind::FunctionDeclaration
        | ast::Kind::ClassDeclaration
        | ast::Kind::ModuleDeclaration => {
            if store
                .parent(*node)
                .is_some_and(|node| ast::is_module_block(store, node))
                && store
                    .parent(*node)
                    .and_then(|parent| store.parent(parent))
                    .is_some_and(|node| ast::is_module_declaration(store, node))
            {
                if let Some(name) = store
                    .parent(*node)
                    .and_then(|parent| store.parent(parent))
                    .and_then(|parent| store.name(parent))
                {
                    if ast::is_identifier(store, name) {
                        return Ok(store.text(name));
                    }
                }
            }
        }
        _ => {}
    }

    Ok(String::new())
}

fn move_range_past_modifiers(store: &ast::AstStore, node: &ast::Node) -> core::TextRange {
    if let Some(modifiers) = store.modifiers(*node) {
        if let Some(last_modifier) = modifiers.nodes().last() {
            return core::new_text_range(store.loc(last_modifier).end(), store.loc(*node).end());
        }
    }
    store.loc(*node)
}

// Finds the implementation of a function-like declaration, if one exists.
fn find_implementation(
    program: &compiler::Program,
    store: &ast::AstStore,
    c: &mut checker::Checker<'_, '_>,
    node: Option<&ast::Node>,
) -> Option<ast::Node> {
    let node = node?;

    if !ast::is_function_like_declaration(store, Some(*node)) {
        return Some(*node);
    }

    if store.body(*node).is_some() {
        return Some(*node);
    }

    if ast::is_constructor_declaration(store, *node) {
        return store
            .parent(*node)
            .and_then(|parent| ast::get_first_constructor_with_body(store, &parent));
    }

    if ast::is_function_declaration(store, *node) || ast::is_method_declaration(store, *node) {
        let symbol = get_symbol_of_call_hierarchy_declaration(store, c, node);
        if let Some(symbol) = symbol {
            let value_declaration = c.symbol_value_declaration_public(symbol);
            if let Some(value_declaration) = value_declaration {
                let value_store = store_for_node(program, &value_declaration);
                if ast::is_function_like_declaration(value_store, Some(value_declaration))
                    && value_store.body(value_declaration).is_some()
                {
                    return Some(value_declaration);
                }
            }
        }
        return None;
    }

    Some(*node)
}

fn find_all_initial_declarations(
    program: &compiler::Program,
    store: &ast::AstStore,
    c: &mut checker::Checker<'_, '_>,
    node: &ast::Node,
) -> Vec<ast::Node> {
    if ast::is_class_static_block_declaration(store, *node) {
        return Vec::new();
    }

    let Some(symbol) = get_symbol_of_call_hierarchy_declaration(store, c, node) else {
        return Vec::new();
    };
    let mut declarations = {
        let declarations = c.collect_symbol_declarations_public(symbol);
        if declarations.is_empty() {
            return Vec::new();
        }
        declarations
    };
    declarations.sort_by(|a, b| {
        source_file_of_node(program, a)
            .file_name()
            .cmp(&source_file_of_node(program, b).file_name())
            .then_with(|| {
                let a_store = store_for_node(program, a);
                let b_store = store_for_node(program, b);
                a_store.loc(*a).pos().cmp(&b_store.loc(*b).pos())
            })
    });

    let mut result = Vec::new();
    let mut last_decl: Option<ast::Node> = None;
    for decl in declarations {
        let decl_store = store_for_node(program, &decl);
        if is_valid_call_hierarchy_declaration(decl_store, Some(&decl)) {
            if last_decl.as_ref().is_none_or(|last_decl| {
                let last_store = store_for_node(program, last_decl);
                last_store.parent(*last_decl) != decl_store.parent(decl)
                    || last_store.loc(*last_decl).end() != decl_store.loc(decl).pos()
            }) {
                result.push(decl);
            }
            last_decl = Some(decl);
        }
    }

    result
}

// Find the implementation or the first declaration for a call hierarchy declaration.
fn find_implementation_or_all_initial_declarations(
    program: &compiler::Program,
    store: &ast::AstStore,
    c: &mut checker::Checker<'_, '_>,
    node: &ast::Node,
) -> CallHierarchyDeclarationResult {
    if ast::is_class_static_block_declaration(store, *node) {
        return CallHierarchyDeclarationResult::Node(*node);
    }

    if ast::is_function_like_declaration(store, Some(*node)) {
        if let Some(implementation) = find_implementation(program, store, c, Some(node)) {
            return CallHierarchyDeclarationResult::Node(implementation);
        }
        let declarations = find_all_initial_declarations(program, store, c, node);
        if !declarations.is_empty() {
            return CallHierarchyDeclarationResult::Nodes(declarations);
        }
        return CallHierarchyDeclarationResult::Node(*node);
    }

    let declarations = find_all_initial_declarations(program, store, c, node);
    if !declarations.is_empty() {
        return CallHierarchyDeclarationResult::Nodes(declarations);
    }
    CallHierarchyDeclarationResult::Node(*node)
}

// Resolves the call hierarchy declaration for a node.
fn resolve_call_hierarchy_declaration<'program>(
    program: &'program compiler::Program,
    c: &mut checker::Checker<'program, '_>,
    location: Option<&ast::Node>,
) -> Result<Option<CallHierarchyDeclarationResult>, core::Error> {
    // A call hierarchy item must refer to either a SourceFile, Module Declaration, Class Static Block, or something intrinsically callable that has a name:
    // - Class Declarations
    // - Class Expressions (with a name)
    // - Function Declarations
    // - Function Expressions (with a name or assigned to a const variable)
    // - Arrow Functions (assigned to a const variable)
    // - Constructors
    // - Class `static {}` initializer blocks
    // - Methods
    // - Accessors
    //
    // If a call is contained in a non-named callable Node (function expression, arrow function, etc.), then
    // its containing `CallHierarchyItem` is a containing function or SourceFile that matches the above list.

    let mut following_symbol = false;
    let mut location = location.cloned();

    while let Some(current) = location.as_ref() {
        let store = store_for_node(program, current);
        if is_valid_call_hierarchy_declaration(store, Some(current)) {
            let result =
                find_implementation_or_all_initial_declarations(program, store, c, current);
            return Ok(Some(result));
        }

        if is_possible_call_hierarchy_declaration(store, Some(current)) {
            let ancestor = ast::find_ancestor(store, Some(*current), |store, node| {
                is_valid_call_hierarchy_declaration(store, Some(&node))
            });
            if let Some(ancestor) = ancestor {
                let result =
                    find_implementation_or_all_initial_declarations(program, store, c, &ancestor);
                return Ok(Some(result));
            }
        }

        if ast::is_declaration_name(store, current) {
            let parent = store.parent(*current);
            if is_valid_call_hierarchy_declaration(store, parent.as_ref()) {
                let parent = parent.unwrap();
                let result =
                    find_implementation_or_all_initial_declarations(program, store, c, &parent);
                return Ok(Some(result));
            }
            if is_possible_call_hierarchy_declaration(store, parent.as_ref()) {
                let ancestor = ast::find_ancestor(store, parent, |store, node| {
                    is_valid_call_hierarchy_declaration(store, Some(&node))
                });
                if let Some(ancestor) = ancestor {
                    let result = find_implementation_or_all_initial_declarations(
                        program, store, c, &ancestor,
                    );
                    return Ok(Some(result));
                }
            }
            if is_variable_like(store, parent.as_ref()) {
                let initializer = parent.and_then(|parent| store.initializer(parent));
                if initializer
                    .as_ref()
                    .is_some_and(|initializer| is_assigned_expression(store, Some(initializer)))
                {
                    return Ok(initializer.map(CallHierarchyDeclarationResult::Node));
                }
            }
            return Ok(None);
        }

        if ast::is_constructor_declaration(store, *current) {
            let parent = store.parent(*current);
            if is_valid_call_hierarchy_declaration(store, parent.as_ref()) {
                return Ok(parent.map(CallHierarchyDeclarationResult::Node));
            }
            return Ok(None);
        }

        if store.kind(*current) == ast::Kind::StaticKeyword
            && store
                .parent(*current)
                .is_some_and(|node| ast::is_class_static_block_declaration(store, node))
        {
            location = store.parent(*current);
            continue;
        }

        // #39453
        if ast::is_variable_declaration(store, *current) {
            let initializer = store.initializer(*current);
            if initializer
                .as_ref()
                .is_some_and(|initializer| is_assigned_expression(store, Some(initializer)))
            {
                return Ok(initializer.map(CallHierarchyDeclarationResult::Node));
            }
        }

        if !following_symbol {
            let mut symbol = c.get_symbol_at_location_public(*current);
            if let Some(s) = symbol.clone() {
                if c.symbol_flags_public(s).unwrap_or(ast::SYMBOL_FLAGS_NONE)
                    & ast::SYMBOL_FLAGS_ALIAS
                    != 0
                {
                    symbol = c.skip_alias_public(s);
                }
                if let Some(value_declaration) =
                    symbol.and_then(|s| c.symbol_value_declaration_public(s))
                {
                    following_symbol = true;
                    location = Some(value_declaration);
                    continue;
                }
            }
        }
        return Ok(None);
    }
    Ok(None)
}

// Creates a `CallHierarchyItem` for a call hierarchy declaration.
impl LanguageService<'_> {
    fn create_call_hierarchy_item<'program>(
        &self,
        program: &'program compiler::Program,
        c: &mut checker::Checker<'program, '_>,
        node: &ast::Node,
    ) -> Result<lsproto::CallHierarchyItem, core::Error> {
        let source_file = source_file_of_node(program, node);
        let (name_text, name_pos, name_end) = get_call_hierarchy_item_name(program, c, node)?;
        let container_name = get_call_hierarchy_item_container_name(program, c, node)?;

        let kind = get_symbol_kind_from_node(source_file.store(), *node);

        let full_start = scanner::skip_trivia_ex(
            source_file.text(),
            source_file.store().loc(*node).pos() as usize,
            Some(&scanner::SkipTriviaOptions {
                stop_after_line_break: false,
                stop_at_comments: true,
            }),
        ) as i32;
        let script = self.get_script(&source_file.file_name()).unwrap();
        let span = self.converters.to_lsp_range(
            &script,
            core::new_text_range(full_start, source_file.store().loc(*node).end()),
        );
        let selection_span = self
            .converters
            .to_lsp_range(&script, core::new_text_range(name_pos, name_end));

        let mut item = lsproto::CallHierarchyItem {
            name: name_text,
            kind,
            uri: lsconv::file_name_to_document_uri(&source_file.file_name()),
            range: span,
            selection_range: selection_span,
            tags: None,
            detail: None,
            data: None,
        };

        if !container_name.is_empty() {
            item.detail = Some(container_name);
        }

        Ok(item)
    }
}

struct CallSite {
    declaration: ast::Node,
    text_range: core::TextRange,
    source_file: ast::SourceFile,
}

fn convert_entry_to_call_site(
    program: &compiler::Program,
    entry: &ReferenceEntry,
) -> Option<CallSite> {
    if entry.kind != EntryKind::Node {
        return None;
    }

    let node = entry.node;
    let store = store_for_node(program, &node);
    if !ast::is_call_or_new_expression_target(store, node, true, true)
        && !ast::is_tagged_template_tag(store, node, true, true)
        && !ast::is_decorator_target(store, node, true, true)
        && !ast::is_jsx_opening_like_element_tag_name(store, node, true, true)
        && !ast::is_right_side_of_property_access(store, node)
        && !ast::is_argument_expression_of_element_access(store, node)
    {
        return None;
    }

    let source_file = source_file_of_node(program, &node);
    let ancestor = ast::find_ancestor(store, Some(node), |store, node| {
        is_valid_call_hierarchy_declaration(store, Some(&node))
    })
    .unwrap_or_else(|| source_file.as_node());

    let start = scanner::skip_trivia(source_file.text(), store.loc(node).pos() as usize) as i32;
    Some(CallSite {
        declaration: ancestor,
        text_range: core::new_text_range(start, store.loc(node).end()),
        source_file: source_file.share_readonly(),
    })
}

fn get_call_site_group_key(site: &CallSite) -> ast::NodeId {
    ast::get_node_id(site.source_file.store(), site.declaration)
}

impl LanguageService<'_> {
    fn convert_call_site_group_to_incoming_call<'program>(
        &self,
        program: &'program compiler::Program,
        c: &mut checker::Checker<'program, '_>,
        entries: &[CallSite],
    ) -> Result<lsproto::CallHierarchyIncomingCall, core::Error> {
        let mut from_ranges: Vec<_> = entries
            .iter()
            .map(|entry| {
                let script = self.get_script(&entry.source_file.file_name()).unwrap();
                self.converters.to_lsp_range(&script, entry.text_range)
            })
            .collect();

        from_ranges.sort_by(|a, b| lsproto::compare_ranges(*a, *b));

        Ok(lsproto::CallHierarchyIncomingCall {
            from: self.create_call_hierarchy_item(program, c, &entries[0].declaration)?,
            from_ranges,
        })
    }
}

struct IncomingEntry<'a> {
    ls: &'a LanguageService<'a>,
    node: ast::Node,
    source_file: ast::SourceFile,
    position: lsproto::Position,
}

impl Clone for IncomingEntry<'_> {
    fn clone(&self) -> Self {
        Self {
            ls: self.ls,
            node: self.node,
            source_file: self.source_file.share_readonly(),
            position: self.position,
        }
    }
}

impl<'a> IncomingEntry<'a> {
    fn get_source_file(&self) -> &ast::SourceFile {
        &self.source_file
    }
}

impl lsproto::HasTextDocumentUri for IncomingEntry<'_> {
    fn text_document_uri(&self) -> lsproto::DocumentUri {
        lsconv::file_name_to_document_uri(&self.source_file.file_name())
    }
}

impl lsproto::HasTextDocumentPosition for IncomingEntry<'_> {
    fn text_document_position(&self) -> lsproto::Position {
        self.position
    }
}

fn symbol_and_entries_to_incoming_calls_callback(
    ls: &LanguageService<'_>,
    ctx: &core::Context,
    params: IncomingEntry<'_>,
    data: SymbolAndEntriesData,
    options: SymbolEntryTransformOptions,
) -> Result<lsproto::CallHierarchyIncomingCallsResponse, core::Error> {
    ls.symbol_and_entries_to_incoming_calls(ctx, params, data, options)
}

// Gets the call sites that call into the provided call hierarchy declaration.
impl LanguageService<'_> {
    fn get_incoming_calls(
        &self,
        ctx: &core::Context,
        program: &compiler::Program,
        declaration: &ast::Node,
        orchestrator: &dyn CrossProjectOrchestrator,
    ) -> Result<lsproto::CallHierarchyIncomingCallsResponse, core::Error> {
        // Source files and modules have no incoming calls.
        let store = store_for_node(program, declaration);
        if ast::is_source_file(store, *declaration)
            || ast::is_module_declaration(store, *declaration)
            || ast::is_class_static_block_declaration(store, *declaration)
        {
            return Ok(lsproto::CallHierarchyIncomingCallsResponse::default());
        }

        let location = get_call_hierarchy_declaration_reference_node(store, Some(declaration));
        let Some(location) = location else {
            return Ok(lsproto::CallHierarchyIncomingCallsResponse::default());
        };

        let source_file = source_file_of_node(program, &location);
        let start = scanner::get_token_pos_of_node(&location, &source_file, false);
        let position = self.create_lsp_position(start as i32, &source_file);
        let incoming_entry = IncomingEntry {
            ls: self,
            node: location,
            source_file: source_file.share_readonly(),
            position,
        };

        let mut result = handle_cross_project(
            self,
            ctx,
            incoming_entry,
            Some(orchestrator),
            symbol_and_entries_to_incoming_calls_callback,
            combine_incoming_calls_boxed,
            false,
            false,
            SymbolEntryTransformOptions::default(),
        )?;
        if let Some(calls) = result.call_hierarchy_incoming_calls.as_mut() {
            calls.sort_by(|a, b| {
                let Some(a) = a else {
                    return std::cmp::Ordering::Greater;
                };
                let Some(b) = b else {
                    return std::cmp::Ordering::Less;
                };
                a.from
                    .uri
                    .to_string()
                    .cmp(&b.from.uri.to_string())
                    .then_with(|| {
                        if a.from_ranges.is_empty() || b.from_ranges.is_empty() {
                            std::cmp::Ordering::Equal
                        } else {
                            lsproto::compare_ranges(a.from_ranges[0], b.from_ranges[0])
                        }
                    })
            });
        }
        Ok(result)
    }

    fn symbol_and_entries_to_incoming_calls(
        &self,
        ctx: &core::Context,
        _params: IncomingEntry<'_>,
        data: SymbolAndEntriesData,
        _options: SymbolEntryTransformOptions,
    ) -> Result<lsproto::CallHierarchyIncomingCallsResponse, core::Error> {
        let program = self.get_program();
        let mut ref_entries = Vec::new();
        for symbol_and_entry in data.symbols_and_entries {
            ref_entries.extend(symbol_and_entry.references);
        }

        let mut call_sites = Vec::new();
        for entry in &ref_entries {
            if let Some(site) = convert_entry_to_call_site(program, entry) {
                call_sites.push(site);
            }
        }

        if call_sites.is_empty() {
            return Ok(lsproto::CallHierarchyIncomingCallsResponse::default());
        }

        let checker_file = source_file_of_node(program, &call_sites[0].declaration);
        let mut grouped: HashMap<ast::NodeId, Vec<CallSite>> = HashMap::new();
        for site in call_sites {
            grouped
                .entry(get_call_site_group_key(&site))
                .or_default()
                .push(site);
        }

        let result = program.with_type_checker_for_file_using(
            compiler::CheckerAccess::context(ctx),
            checker_file,
            |c| {
                let mut result = Vec::new();
                for sites in grouped.values() {
                    result.push(self.convert_call_site_group_to_incoming_call(program, c, sites)?);
                }
                Ok::<_, core::Error>(result)
            },
        )?;
        Ok(lsproto::CallHierarchyIncomingCallsResponse {
            call_hierarchy_incoming_calls: Some(result.into_iter().map(Some).collect()),
        })
    }
}

struct CallSiteCollector<'program, 'checker, 'state> {
    program: &'program compiler::Program,
    checker: &'checker mut checker::Checker<'program, 'state>,
    call_sites: Vec<CallSite>,
    err: Option<core::Error>,
}

impl<'program, 'checker, 'state> CallSiteCollector<'program, 'checker, 'state> {
    fn record_call_site(&mut self, node: ast::Node) {
        if self.err.is_some() {
            return;
        }
        let store = store_for_node(self.program, &node);
        let target = if ast::is_tagged_template_expression(store, node) {
            store.tag(node)
        } else if ast::is_jsx_opening_element(store, node)
            || ast::is_jsx_self_closing_element(store, node)
        {
            store.tag_name(node)
        } else if ast::is_property_access_expression(store, node)
            || ast::is_element_access_expression(store, node)
            || ast::is_class_static_block_declaration(store, node)
        {
            Some(node)
        } else if ast::is_call_expression(store, node)
            || ast::is_new_expression(store, node)
            || ast::is_decorator(store, node)
        {
            store.expression(node)
        } else {
            None
        };

        let Some(target) = target else {
            return;
        };

        let declaration =
            match resolve_call_hierarchy_declaration(self.program, self.checker, Some(&target)) {
                Ok(declaration) => declaration,
                Err(err) => {
                    self.err = Some(err);
                    return;
                }
            };
        let Some(declaration) = declaration else {
            return;
        };

        let source_file = source_file_of_node(self.program, &target);
        let target_store = source_file.store();
        let start =
            scanner::skip_trivia(source_file.text(), target_store.loc(target).pos() as usize)
                as i32;
        let text_range = core::new_text_range(start, target_store.loc(target).end());

        match declaration {
            CallHierarchyDeclarationResult::Node(declaration) => self.call_sites.push(CallSite {
                declaration,
                text_range,
                source_file: source_file.share_readonly(),
            }),
            CallHierarchyDeclarationResult::Nodes(declarations) => {
                for declaration in declarations {
                    self.call_sites.push(CallSite {
                        declaration,
                        text_range,
                        source_file: source_file.share_readonly(),
                    });
                }
            }
        }
    }

    fn collect(&mut self, node: Option<ast::Node>) {
        if self.err.is_some() {
            return;
        }
        let Some(node) = node else {
            return;
        };
        let store = store_for_node(self.program, &node);

        // do not descend into ambient nodes.
        if store.flags(node).intersects(ast::NodeFlags::Ambient) {
            return;
        }

        // do not descend into other call site declarations, other than class member names
        if is_valid_call_hierarchy_declaration(store, Some(&node)) {
            if ast::is_class_like(store, node) {
                if let Some(members) = store.members(node) {
                    for member in members {
                        if store
                            .name(member)
                            .as_ref()
                            .is_some_and(|name| ast::is_computed_property_name(store, *name))
                        {
                            self.collect(
                                store.name(member).and_then(|name| store.expression(name)),
                            );
                        }
                    }
                }
            }
            return;
        }

        match store.kind(node) {
            ast::Kind::Identifier
            | ast::Kind::ImportEqualsDeclaration
            | ast::Kind::ImportDeclaration
            | ast::Kind::ExportDeclaration
            | ast::Kind::InterfaceDeclaration
            | ast::Kind::TypeAliasDeclaration => {
                // do not descend into nodes that cannot contain callable nodes
                return;
            }
            ast::Kind::ClassStaticBlockDeclaration => {
                self.record_call_site(node);
                return;
            }
            ast::Kind::TypeAssertionExpression | ast::Kind::AsExpression => {
                // do not descend into the type side of an assertion
                self.collect(store.expression(node));
                return;
            }
            ast::Kind::VariableDeclaration | ast::Kind::Parameter => {
                // do not descend into the type of a variable or parameter declaration
                self.collect(store.name(node));
                self.collect(store.initializer(node));
                return;
            }
            ast::Kind::CallExpression | ast::Kind::NewExpression => {
                // do not descend into the type arguments of a call expression
                self.record_call_site(node);
                self.collect(store.expression(node));
                if let Some(arguments) = store.arguments(node) {
                    for arg in arguments {
                        self.collect(Some(arg));
                    }
                }
                return;
            }
            ast::Kind::TaggedTemplateExpression => {
                // do not descend into the type arguments of a tagged template expression
                self.record_call_site(node);
                self.collect(store.tag(node));
                self.collect(store.template(node));
                return;
            }
            ast::Kind::JsxOpeningElement | ast::Kind::JsxSelfClosingElement => {
                // do not descend into the type arguments of a JsxOpeningLikeElement
                self.record_call_site(node);
                self.collect(store.tag_name(node));
                self.collect(store.attributes(node));
                return;
            }
            ast::Kind::Decorator => {
                self.record_call_site(node);
                self.collect(store.expression(node));
                return;
            }
            ast::Kind::PropertyAccessExpression | ast::Kind::ElementAccessExpression => {
                self.record_call_site(node);
                let _ = store.for_each_present_child(node, |child| {
                    self.collect(Some(child));
                    std::ops::ControlFlow::Continue(())
                });
                return;
            }
            ast::Kind::SatisfiesExpression => {
                // do not descend into the type side of an assertion
                self.collect(store.expression(node));
                return;
            }
            _ => {}
        }

        if ast::is_part_of_type_node(store, &node) {
            // do not descend into types
            return;
        }

        let _ = store.for_each_present_child(node, |child| {
            self.collect(Some(child));
            std::ops::ControlFlow::Continue(())
        });
    }
}

fn collect_call_sites<'program>(
    program: &'program compiler::Program,
    c: &mut checker::Checker<'program, '_>,
    node: &ast::Node,
) -> Result<Vec<CallSite>, core::Error> {
    let store = store_for_node(program, node);
    let mut collector = CallSiteCollector {
        program,
        checker: c,
        call_sites: Vec::new(),
        err: None,
    };

    match store.kind(*node) {
        ast::Kind::SourceFile => {
            if let Some(statements) = store.statements(*node) {
                for stmt in statements {
                    collector.collect(Some(stmt));
                }
            }
        }
        ast::Kind::ModuleDeclaration => {
            let body = store.body(*node);
            if !ast::has_syntactic_modifier(store, *node, ast::ModifierFlags::Ambient)
                && body
                    .as_ref()
                    .is_some_and(|body| ast::is_module_block(store, *body))
            {
                if let Some(statements) = store.statements(body.unwrap()) {
                    for stmt in statements {
                        collector.collect(Some(stmt));
                    }
                }
            }
        }
        ast::Kind::FunctionDeclaration
        | ast::Kind::FunctionExpression
        | ast::Kind::ArrowFunction
        | ast::Kind::MethodDeclaration
        | ast::Kind::GetAccessor
        | ast::Kind::SetAccessor => {
            if let Some(implementation) =
                find_implementation(program, store, collector.checker, Some(node))
            {
                if let Some(parameters) = store.parameters(implementation) {
                    for param in parameters {
                        collector.collect(Some(param));
                    }
                }
                collector.collect(store.body(implementation));
            }
        }
        ast::Kind::ClassDeclaration | ast::Kind::ClassExpression => {
            if let Some(modifiers) = store.modifiers(*node) {
                for modifier in modifiers.nodes() {
                    collector.collect(Some(modifier));
                }
            }

            let heritage = ast::get_class_extends_heritage_element(store, node);
            if let Some(heritage) = heritage {
                collector.collect(store.expression(heritage));
            }

            if let Some(members) = store.members(*node) {
                for member in members {
                    if ast::can_have_modifiers(store, member) {
                        if let Some(modifiers) = store.modifiers(member) {
                            for modifier in modifiers.nodes() {
                                collector.collect(Some(modifier));
                            }
                        }
                    }

                    if ast::is_property_declaration(store, member) {
                        collector.collect(store.initializer(member));
                    } else if ast::is_constructor_declaration(store, member) {
                        if let Some(body) = store.body(member) {
                            if let Some(parameters) = store.parameters(member) {
                                for param in parameters {
                                    collector.collect(Some(param));
                                }
                            }
                            collector.collect(Some(body));
                        }
                    } else if ast::is_class_static_block_declaration(store, member) {
                        collector.collect(Some(member));
                    }
                }
            }
        }
        ast::Kind::ClassStaticBlockDeclaration => {
            collector.collect(store.body(*node));
        }
        _ => unreachable!("unexpected call hierarchy declaration kind"),
    }

    match collector.err {
        Some(err) => Err(err),
        None => Ok(collector.call_sites),
    }
}

impl LanguageService<'_> {
    fn convert_call_site_group_to_outgoing_call<'program>(
        &self,
        program: &'program compiler::Program,
        c: &mut checker::Checker<'program, '_>,
        entries: &[CallSite],
    ) -> Result<lsproto::CallHierarchyOutgoingCall, core::Error> {
        let mut from_ranges: Vec<_> = entries
            .iter()
            .map(|entry| {
                let script = self.get_script(&entry.source_file.file_name()).unwrap();
                self.converters.to_lsp_range(&script, entry.text_range)
            })
            .collect();

        from_ranges.sort_by(|a, b| lsproto::compare_ranges(*a, *b));

        Ok(lsproto::CallHierarchyOutgoingCall {
            to: self.create_call_hierarchy_item(program, c, &entries[0].declaration)?,
            from_ranges,
        })
    }

    // Gets the call sites that call out of the provided call hierarchy declaration.
    fn get_outgoing_calls<'program>(
        &self,
        program: &'program compiler::Program,
        c: &mut checker::Checker<'program, '_>,
        declaration: &ast::Node,
    ) -> Result<Vec<lsproto::CallHierarchyOutgoingCall>, core::Error> {
        let store = store_for_node(program, declaration);
        if store
            .flags(*declaration)
            .intersects(ast::NodeFlags::Ambient)
            || ast::is_method_signature_declaration(store, *declaration)
        {
            return Ok(Vec::new());
        }

        let call_sites = collect_call_sites(program, c, declaration)?;

        if call_sites.is_empty() {
            return Ok(Vec::new());
        }

        let mut grouped: HashMap<ast::NodeId, Vec<CallSite>> = HashMap::new();
        for site in call_sites {
            grouped
                .entry(get_call_site_group_key(&site))
                .or_default()
                .push(site);
        }

        let mut result = Vec::new();
        for sites in grouped.values() {
            result.push(self.convert_call_site_group_to_outgoing_call(program, c, sites)?);
        }

        result.sort_by(|a, b| {
            a.to.uri
                .to_string()
                .cmp(&b.to.uri.to_string())
                .then_with(|| {
                    if a.from_ranges.is_empty() || b.from_ranges.is_empty() {
                        std::cmp::Ordering::Equal
                    } else {
                        lsproto::compare_ranges(a.from_ranges[0], b.from_ranges[0])
                    }
                })
        });

        Ok(result)
    }

    pub fn provide_prepare_call_hierarchy(
        &self,
        ctx: &core::Context,
        document_uri: lsproto::DocumentUri,
        position: lsproto::Position,
    ) -> Result<lsproto::CallHierarchyPrepareResponse, core::Error> {
        let (program, file) = self.get_program_and_file(document_uri);
        let node = astnav::get_touching_property_name(
            file,
            self.converters
                .line_and_character_to_position(file, position) as i32,
        );

        if node.is_some_and(|node| file.store().kind(node) == ast::Kind::SourceFile) {
            return Ok(lsproto::CallHierarchyPrepareResponse::default());
        }

        let items = program.with_type_checker_for_file_using(
            compiler::CheckerAccess::context(ctx),
            file,
            |c| {
                let declaration = resolve_call_hierarchy_declaration(program, c, node.as_ref())?;
                let Some(declaration) = declaration else {
                    return Ok(Vec::new());
                };

                match declaration {
                    CallHierarchyDeclarationResult::Node(declaration) => {
                        Ok(vec![self.create_call_hierarchy_item(
                            program,
                            c,
                            &declaration,
                        )?])
                    }
                    CallHierarchyDeclarationResult::Nodes(declarations) => declarations
                        .into_iter()
                        .map(|declaration| {
                            self.create_call_hierarchy_item(program, c, &declaration)
                        })
                        .collect::<Result<Vec<_>, _>>(),
                }
            },
        )?;

        if items.is_empty() {
            return Ok(lsproto::CallHierarchyPrepareResponse::default());
        }
        Ok(lsproto::CallHierarchyPrepareResponse {
            call_hierarchy_items: Some(items.into_iter().map(Some).collect()),
        })
    }

    pub fn provide_call_hierarchy_incoming_calls(
        &self,
        ctx: &core::Context,
        item: &lsproto::CallHierarchyItem,
        orchestrator: &dyn CrossProjectOrchestrator,
    ) -> Result<lsproto::CallHierarchyIncomingCallsResponse, core::Error> {
        let program = self.get_program();
        let file_name = item.uri.file_name();
        let file = program.get_source_file_ref(&file_name);
        let Some(file) = file else {
            return Ok(lsproto::CallHierarchyIncomingCallsResponse::default());
        };

        let pos = self
            .converters
            .line_and_character_to_position(file, item.selection_range.start);
        let node = if pos == 0 {
            Some(file.as_node())
        } else {
            astnav::get_touching_property_name(file, pos as i32)
        };

        let Some(node) = node else {
            return Ok(lsproto::CallHierarchyIncomingCallsResponse::default());
        };

        let declaration = program.with_type_checker_for_file_using(
            compiler::CheckerAccess::context(ctx),
            file,
            |c| resolve_call_hierarchy_declaration(program, c, Some(&node)),
        )?;
        let Some(declaration) = declaration else {
            return Ok(lsproto::CallHierarchyIncomingCallsResponse::default());
        };

        let decl = match declaration {
            CallHierarchyDeclarationResult::Node(declaration) => Some(declaration),
            CallHierarchyDeclarationResult::Nodes(declarations) => declarations.first().cloned(),
        };

        let Some(decl) = decl else {
            return Ok(lsproto::CallHierarchyIncomingCallsResponse::default());
        };

        self.get_incoming_calls(ctx, program, &decl, orchestrator)
    }

    pub fn provide_call_hierarchy_outgoing_calls(
        &self,
        ctx: &core::Context,
        item: &lsproto::CallHierarchyItem,
    ) -> Result<lsproto::CallHierarchyOutgoingCallsResponse, core::Error> {
        let program = self.get_program();
        let file_name = item.uri.file_name();
        let file = program.get_source_file_ref(&file_name);
        let Some(file) = file else {
            return Ok(lsproto::CallHierarchyOutgoingCallsResponse::default());
        };

        let pos = self
            .converters
            .line_and_character_to_position(file, item.selection_range.start);
        let node = if pos == 0 {
            Some(file.as_node())
        } else {
            astnav::get_touching_property_name(file, pos as i32)
        };

        let Some(node) = node else {
            return Ok(lsproto::CallHierarchyOutgoingCallsResponse::default());
        };

        let calls = program.with_type_checker_for_file_using(
            compiler::CheckerAccess::context(ctx),
            file,
            |c| {
                let declaration = resolve_call_hierarchy_declaration(program, c, Some(&node))?;
                let Some(declaration) = declaration else {
                    return Ok(Vec::new());
                };

                let decl = match declaration {
                    CallHierarchyDeclarationResult::Node(declaration) => Some(declaration),
                    CallHierarchyDeclarationResult::Nodes(declarations) => {
                        declarations.first().cloned()
                    }
                };

                let Some(decl) = decl else {
                    return Ok(Vec::new());
                };

                self.get_outgoing_calls(program, c, &decl)
            },
        )?;
        if calls.is_empty() {
            return Ok(lsproto::CallHierarchyOutgoingCallsResponse::default());
        }
        Ok(lsproto::CallHierarchyOutgoingCallsResponse {
            call_hierarchy_outgoing_calls: Some(calls.into_iter().map(Some).collect()),
        })
    }
}
