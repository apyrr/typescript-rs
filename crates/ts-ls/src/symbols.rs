use std::collections::HashMap;

use ts_ast as ast;
use ts_astnav as astnav;
use ts_compiler as compiler;
use ts_core as core;
use ts_lsproto as lsproto;
use ts_printer as printer;
use ts_scanner as scanner;
use ts_stringutil as stringutil;
use ts_tspath as tspath;

use crate::LanguageService;
use crate::lsutil;

impl LanguageService<'_> {
    pub fn provide_document_symbols(
        &self,
        ctx: &core::Context,
        document_uri: lsproto::DocumentUri,
    ) -> Result<lsproto::DocumentSymbolResponse, core::Error> {
        let (_, file) = self.get_program_and_file(document_uri.clone());
        if lsproto::get_client_capabilities(ctx)
            .text_document
            .document_symbol
            .hierarchical_document_symbol_support
        {
            let symbols = self.get_document_symbols_for_children(ctx, file.as_node(), file);
            return Ok(lsproto::SymbolInformationsOrDocumentSymbolsOrNull {
                document_symbols: Some(symbols.into_iter().map(Some).collect()),
                ..Default::default()
            });
        }

        let symbol_infos = self.get_document_symbol_informations(ctx, file, document_uri);
        Ok(lsproto::SymbolInformationsOrDocumentSymbolsOrNull {
            symbol_informations: Some(
                symbol_infos
                    .into_iter()
                    .map(|symbol| Some(Box::new(symbol)))
                    .collect(),
            ),
            ..Default::default()
        })
    }

    pub fn get_document_symbol_informations(
        &self,
        ctx: &core::Context,
        file: &ast::SourceFile,
        document_uri: lsproto::DocumentUri,
    ) -> Vec<lsproto::SymbolInformation> {
        let doc_symbols = self.get_document_symbols_for_children(ctx, file.as_node(), file);
        let mut result = Vec::new();

        fn flatten(
            symbols: &[lsproto::DocumentSymbol],
            document_uri: lsproto::DocumentUri,
            container_name: Option<String>,
            result: &mut Vec<lsproto::SymbolInformation>,
        ) {
            for symbol in symbols {
                result.push(lsproto::SymbolInformation {
                    name: symbol.name.clone(),
                    kind: symbol.kind,
                    location: lsproto::Location {
                        uri: document_uri.clone(),
                        range: symbol.range,
                    },
                    container_name: container_name.clone(),
                    tags: symbol.tags.clone(),
                    deprecated: symbol.deprecated,
                    ..Default::default()
                });
                if let Some(children) = &symbol.children {
                    flatten(
                        children,
                        document_uri.clone(),
                        Some(symbol.name.clone()),
                        result,
                    );
                }
            }
        }

        flatten(&doc_symbols, document_uri, None, &mut result);
        result
    }

    pub fn get_document_symbols_for_children(
        &self,
        ctx: &core::Context,
        node: ast::Node,
        file: &ast::SourceFile,
    ) -> Vec<lsproto::DocumentSymbol> {
        get_document_symbols_for_children_worker(self, ctx, &node, file)
    }

    pub fn new_document_symbol(
        &self,
        file: &ast::SourceFile,
        node: ast::Node,
        name: Option<ast::Node>,
        children: Vec<lsproto::DocumentSymbol>,
    ) -> Option<lsproto::DocumentSymbol> {
        let store = file.store();
        let node_loc = store.loc(node);
        let node_start_pos = scanner::skip_trivia(file.text(), node_loc.pos() as usize) as i32;
        let generated_name = ast::get_name_of_declaration(store, Some(node));
        let name = name.or(generated_name);

        let (mut text, name_start_pos, name_end_pos) =
            if ast::is_module_declaration(store, node) && !ast::is_ambient_module(store, node) {
                let interior_module = get_interior_module(store, node);
                let name_node = name.unwrap();
                (
                    get_module_name(store, node),
                    scanner::skip_trivia(file.text(), store.loc(name_node).pos() as usize) as i32,
                    store.loc(store.name(interior_module).unwrap()).end(),
                )
            } else if ast::is_any_export_assignment(store, &node)
                && store.is_export_equals(node).unwrap_or(false)
            {
                if !ast::node_is_missing(store, name) {
                    let name = name.unwrap();
                    let name_loc = store.loc(name);
                    (
                        "export=".to_string(),
                        scanner::skip_trivia(file.text(), name_loc.pos() as usize) as i32,
                        name_loc.end(),
                    )
                } else {
                    ("export=".to_string(), node_start_pos, node_loc.end())
                }
            } else if let Some(name) = name {
                let name_loc = store.loc(name);
                (
                    get_text_of_name(store, file, name),
                    (scanner::skip_trivia(file.text(), name_loc.pos() as usize) as i32)
                        .max(node_start_pos),
                    name_loc.end().max(node_start_pos),
                )
            } else {
                let label = get_unnamed_node_label(store, file, node);
                (label, node_start_pos, node_start_pos)
            };

        if text.is_empty() {
            return None;
        }
        let truncated_text = stringutil::truncate_by_runes(&text, MAX_LENGTH as i32);
        if truncated_text.len() < text.len() {
            text = format!("{truncated_text}...");
        }

        Some(lsproto::DocumentSymbol {
            name: text,
            kind: get_symbol_kind_from_node(store, node),
            range: lsproto::Range {
                start: self
                    .converters
                    .position_to_line_and_character(file, node_start_pos),
                end: self
                    .converters
                    .position_to_line_and_character(file, node_loc.end()),
            },
            selection_range: lsproto::Range {
                start: self
                    .converters
                    .position_to_line_and_character(file, name_start_pos),
                end: self
                    .converters
                    .position_to_line_and_character(file, name_end_pos),
            },
            children: Some(children),
            ..Default::default()
        })
    }
}

fn get_document_symbols_for_children_worker(
    ls: &LanguageService<'_>,
    ctx: &core::Context,
    node: &ast::Node,
    file: &ast::SourceFile,
) -> Vec<lsproto::DocumentSymbol> {
    merge_expandos(collect_document_symbols_for_children(ls, ctx, node, file))
}

fn collect_document_symbols_for_children(
    ls: &LanguageService<'_>,
    ctx: &core::Context,
    node: &ast::Node,
    file: &ast::SourceFile,
) -> Vec<lsproto::DocumentSymbol> {
    let mut symbols = Vec::new();
    let mut expando_targets = std::collections::HashSet::new();
    let _ = file.store().for_each_present_child(*node, |child| {
        if visit_document_symbol_node(ls, ctx, &child, file, &mut symbols, &mut expando_targets) {
            std::ops::ControlFlow::Break(())
        } else {
            std::ops::ControlFlow::Continue(())
        }
    });
    symbols
}

fn add_symbol_for_node(
    ls: &LanguageService<'_>,
    symbols: &mut Vec<lsproto::DocumentSymbol>,
    file: &ast::SourceFile,
    node: &ast::Node,
    name: Option<&ast::Node>,
    children: Vec<lsproto::DocumentSymbol>,
) {
    if !file
        .store()
        .flags(*node)
        .intersects(ast::NodeFlags::Reparsed)
    {
        if let Some(symbol) = ls.new_document_symbol(file, *node, name.copied(), children) {
            symbols.push(symbol);
        }
    }
}

fn get_symbols_for_children(
    ls: &LanguageService<'_>,
    ctx: &core::Context,
    node: Option<&ast::Node>,
    file: &ast::SourceFile,
) -> Vec<lsproto::DocumentSymbol> {
    node.map(|node| collect_document_symbols_for_children(ls, ctx, node, file))
        .unwrap_or_default()
}

fn get_symbols_for_node(
    ls: &LanguageService<'_>,
    ctx: &core::Context,
    node: Option<&ast::Node>,
    file: &ast::SourceFile,
    expando_targets: &mut std::collections::HashSet<String>,
) -> Vec<lsproto::DocumentSymbol> {
    let mut result = Vec::new();
    if let Some(node) = node {
        visit_document_symbol_node(ls, ctx, node, file, &mut result, expando_targets);
    }
    result
}

fn visit_children(
    ls: &LanguageService<'_>,
    ctx: &core::Context,
    node: &ast::Node,
    file: &ast::SourceFile,
    symbols: &mut Vec<lsproto::DocumentSymbol>,
    expando_targets: &mut std::collections::HashSet<String>,
) -> bool {
    let mut should_stop = false;
    let _ = file.store().for_each_present_child(*node, |child| {
        should_stop = visit_document_symbol_node(ls, ctx, &child, file, symbols, expando_targets);
        if should_stop {
            std::ops::ControlFlow::Break(())
        } else {
            std::ops::ControlFlow::Continue(())
        }
    });
    should_stop
}

fn visit_document_symbol_node(
    ls: &LanguageService<'_>,
    ctx: &core::Context,
    node: &ast::Node,
    file: &ast::SourceFile,
    symbols: &mut Vec<lsproto::DocumentSymbol>,
    expando_targets: &mut std::collections::HashSet<String>,
) -> bool {
    if ctx.err().is_some() {
        return true;
    }
    let store = file.store();

    match store.kind(*node) {
        ast::Kind::ClassDeclaration
        | ast::Kind::ClassExpression
        | ast::Kind::InterfaceDeclaration
        | ast::Kind::EnumDeclaration => {
            if ast::is_class_like(store, *node)
                && !ast::get_declaration_name(store, node).is_empty()
            {
                expando_targets.insert(ast::get_declaration_name(store, node));
            }
            add_symbol_for_node(
                ls,
                symbols,
                file,
                node,
                None,
                get_symbols_for_children(ls, ctx, Some(node), file),
            );
        }
        ast::Kind::ModuleDeclaration => {
            add_symbol_for_node(ls, symbols, file, node, None, {
                let interior_module = get_interior_module(store, *node);
                get_symbols_for_children(ls, ctx, Some(&interior_module), file)
            });
        }
        ast::Kind::Constructor => {
            let body = store.body(*node);
            add_symbol_for_node(
                ls,
                symbols,
                file,
                node,
                None,
                get_symbols_for_children(ls, ctx, body.as_ref(), file),
            );
            if let Some(parameters) = store.parameters(*node) {
                for param in parameters {
                    if ast::is_parameter_property_declaration(store, param, *node) {
                        add_symbol_for_node(ls, symbols, file, &param, None, Vec::new());
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
            let decl_name = ast::get_declaration_name(store, node);
            if !decl_name.is_empty() {
                expando_targets.insert(decl_name);
            }
            let body = store.body(*node);
            add_symbol_for_node(
                ls,
                symbols,
                file,
                node,
                None,
                get_symbols_for_children(ls, ctx, body.as_ref(), file),
            );
        }
        ast::Kind::VariableDeclaration
        | ast::Kind::BindingElement
        | ast::Kind::PropertyAssignment
        | ast::Kind::PropertyDeclaration => {
            if let Some(node_name) = store.name(*node) {
                if ast::is_binding_pattern(store, node_name) {
                    visit_document_symbol_node(ls, ctx, &node_name, file, symbols, expando_targets);
                } else {
                    let initializer = store.initializer(*node);
                    add_symbol_for_node(
                        ls,
                        symbols,
                        file,
                        node,
                        None,
                        get_symbols_for_children(ls, ctx, initializer.as_ref(), file),
                    );
                }
            }
        }
        ast::Kind::SpreadAssignment => {
            add_symbol_for_node(
                ls,
                symbols,
                file,
                node,
                store.expression(*node).as_ref(),
                Vec::new(),
            );
        }
        ast::Kind::MethodSignature
        | ast::Kind::PropertySignature
        | ast::Kind::CallSignature
        | ast::Kind::ConstructSignature
        | ast::Kind::IndexSignature
        | ast::Kind::EnumMember
        | ast::Kind::ShorthandPropertyAssignment
        | ast::Kind::TypeAliasDeclaration
        | ast::Kind::ImportEqualsDeclaration
        | ast::Kind::ExportSpecifier => {
            add_symbol_for_node(ls, symbols, file, node, None, Vec::new());
        }
        ast::Kind::ImportClause => {
            if let Some(name) = store.name(*node) {
                add_symbol_for_node(ls, symbols, file, &name, Some(&name), Vec::new());
            }
            if let Some(named_bindings) = store.named_bindings(*node) {
                if store.kind(named_bindings) == ast::Kind::NamespaceImport {
                    add_symbol_for_node(ls, symbols, file, &named_bindings, None, Vec::new());
                } else {
                    if let Some(elements) = store.elements(named_bindings) {
                        for element in elements {
                            add_symbol_for_node(ls, symbols, file, &element, None, Vec::new());
                        }
                    }
                }
            }
        }
        ast::Kind::BinaryExpression | ast::Kind::CallExpression => {
            match ast::get_assignment_declaration_kind(store, *node) {
                ast::JSDeclarationKind::None
                | ast::JSDeclarationKind::ThisProperty
                | ast::JSDeclarationKind::ModuleExports
                | ast::JSDeclarationKind::ExportsProperty
                | ast::JSDeclarationKind::Prototype
                | ast::JSDeclarationKind::PrototypeProperty
                | ast::JSDeclarationKind::ObjectDefinePropertyExports => {
                    visit_children(ls, ctx, node, file, symbols, expando_targets);
                }
                ast::JSDeclarationKind::Property
                | ast::JSDeclarationKind::ObjectDefinePropertyValue => {
                    let (target, mut target_function, definition, property_name) =
                        if ast::is_binary_expression(store, *node) {
                            let target = store.left(*node).unwrap();
                            let target_function = store.expression(target).unwrap();
                            let definition = store.right(*node).unwrap();
                            let property_name = if ast::is_property_access_expression(store, target)
                            {
                                store.name(target).unwrap()
                            } else {
                                store.argument_expression(target).unwrap()
                            };
                            (target, target_function, definition, property_name)
                        } else {
                            let args = store.arguments(*node).unwrap();
                            let args = args.into_iter().collect::<Vec<_>>();
                            let target_function = args[0];
                            let target = args[1];
                            let property_name = target;
                            let definition = args[2];
                            (target, target_function, definition, property_name)
                        };

                    if is_prototype_expando(store, target_function) {
                        target_function = store.expression(target_function).unwrap();
                        if ast::is_identifier(store, target_function) {
                            expando_targets.insert(store.text(target_function));
                        }
                    }
                    if ast::is_identifier(store, target_function)
                        && expando_targets.contains(&store.text(target_function))
                    {
                        let mut nested_symbols = Vec::new();
                        let mut nested_expando_targets = std::collections::HashSet::new();
                        add_symbol_for_node(
                            ls,
                            &mut nested_symbols,
                            file,
                            &target,
                            Some(&property_name),
                            get_symbols_for_node(
                                ls,
                                ctx,
                                Some(&definition),
                                file,
                                &mut nested_expando_targets,
                            ),
                        );
                        add_symbol_for_node(
                            ls,
                            symbols,
                            file,
                            node,
                            Some(&target_function),
                            nested_symbols,
                        );
                    } else {
                        visit_children(ls, ctx, node, file, symbols, expando_targets);
                    }
                }
            }
        }
        ast::Kind::ExportAssignment => {
            if store.is_export_equals(*node).unwrap_or(false) {
                let expression = store.expression(*node);
                add_symbol_for_node(
                    ls,
                    symbols,
                    file,
                    node,
                    None,
                    get_symbols_for_node(ls, ctx, expression.as_ref(), file, expando_targets),
                );
            } else {
                visit_children(ls, ctx, node, file, symbols, expando_targets);
            }
        }
        _ => {
            visit_children(ls, ctx, node, file, symbols, expando_targets);
        }
    }
    false
}

pub(crate) fn is_prototype_expando(store: &ast::AstStore, target: ast::Node) -> bool {
    if ast::is_access_expression(store, target) {
        let access_name = ast::get_element_or_property_access_name(store, target);
        return access_name.is_some_and(|access_name| store.text_eq(access_name, "prototype"));
    }
    false
}

pub const MAX_LENGTH: usize = 150;

pub fn merge_expandos(mut symbols: Vec<lsproto::DocumentSymbol>) -> Vec<lsproto::DocumentSymbol> {
    let mut merged = Vec::new();
    let mut name_to_expando_target_indices: HashMap<String, Vec<usize>> = HashMap::new();
    let mut name_to_namespace_index: HashMap<String, usize> = HashMap::new();

    for (i, symbol) in symbols.iter().enumerate() {
        if is_anonymous_name(&symbol.name) {
            continue;
        }
        if matches!(
            symbol.kind,
            lsproto::SymbolKind::CLASS
                | lsproto::SymbolKind::FUNCTION
                | lsproto::SymbolKind::VARIABLE
        ) {
            name_to_expando_target_indices
                .entry(symbol.name.clone())
                .or_default()
                .push(i);
        }
        if symbol.kind == lsproto::SymbolKind::NAMESPACE {
            name_to_namespace_index
                .entry(symbol.name.clone())
                .or_insert(i);
        }
    }

    for i in 0..symbols.len() {
        if let Some(children) = symbols[i].children.take() {
            symbols[i].children = Some(merge_expandos(children));
        }
        if is_anonymous_name(&symbols[i].name) {
            continue;
        }

        if symbols[i].kind == lsproto::SymbolKind::PROPERTY {
            if let Some(indices) = name_to_expando_target_indices.get(&symbols[i].name) {
                let source = symbols[i].clone();
                for target_index in indices.iter().rev() {
                    if *target_index != i {
                        merge_children(&mut symbols[*target_index], &source);
                    }
                }
            }
        }

        if symbols[i].kind == lsproto::SymbolKind::NAMESPACE {
            if let Some(target_index) = name_to_namespace_index.get(&symbols[i].name) {
                if *target_index != i {
                    let source = symbols[i].clone();
                    merge_children(&mut symbols[*target_index], &source);
                }
            }
        }
    }

    for (i, symbol) in symbols.into_iter().enumerate() {
        let merged_property = name_to_expando_target_indices
            .get(&symbol.name)
            .is_some_and(|targets| {
                symbol.kind == lsproto::SymbolKind::PROPERTY && !targets.is_empty()
            });
        let merged_namespace =
            name_to_namespace_index
                .get(&symbol.name)
                .is_some_and(|target_index| {
                    symbol.kind == lsproto::SymbolKind::NAMESPACE && *target_index != i
                });
        if !merged_property && !merged_namespace {
            merged.push(symbol);
        }
    }
    merged
}

pub fn merge_children(target: &mut lsproto::DocumentSymbol, source: &lsproto::DocumentSymbol) {
    if let Some(source_children) = &source.children {
        if target.children.is_none() {
            target.children = Some(source_children.clone());
        } else {
            let mut merged = target.children.take().unwrap();
            merged.extend(source_children.clone());
            let mut merged = merge_expandos(merged);
            merged.sort_by(|a, b| lsproto::compare_ranges(a.range, b.range));
            target.children = Some(merged);
        }
    }
}

pub fn is_anonymous_name(name: &str) -> bool {
    name == "<function>"
        || name == "<class>"
        || name == "export="
        || name == "default"
        || name == "constructor"
        || name == "()"
        || name == "new()"
        || name == "[]"
        || name.ends_with(") callback")
}

pub fn get_text_of_name(
    store: &ast::AstStore,
    source_file: &ast::SourceFile,
    node: ast::Node,
) -> String {
    match store.kind(node) {
        ast::Kind::Identifier | ast::Kind::PrivateIdentifier | ast::Kind::NumericLiteral => {
            store.text(node)
        }
        ast::Kind::StringLiteral => {
            format!(
                "\"{}\"",
                printer::escape_string(store.text(node), printer::QuoteChar::DoubleQuote)
            )
        }
        ast::Kind::NoSubstitutionTemplateLiteral => {
            format!(
                "`{}`",
                printer::escape_string(store.text(node), printer::QuoteChar::Backtick)
            )
        }
        ast::Kind::ComputedPropertyName => {
            let expression = store.expression(node).unwrap();
            if ast::is_string_or_numeric_literal_like(store, expression) {
                get_text_of_name(store, source_file, expression)
            } else {
                scanner::get_text_of_node(source_file, &node)
            }
        }
        _ => scanner::get_text_of_node(source_file, &node),
    }
}

pub fn get_unnamed_node_label(
    store: &ast::AstStore,
    source_file: &ast::SourceFile,
    node: ast::Node,
) -> String {
    let parent = store.parent(node);
    if let Some(parent) = ast::walk_up_parenthesized_expressions(store, parent) {
        if ast::is_export_assignment(store, parent) {
            if store.is_export_equals(parent).unwrap_or(false) {
                return "export=".to_string();
            }
            return "default".to_string();
        }
    }
    match store.kind(node) {
        ast::Kind::FunctionDeclaration
        | ast::Kind::FunctionExpression
        | ast::Kind::ArrowFunction => {
            if ast::get_combined_modifier_flags(store, node).intersects(ast::ModifierFlags::Default)
            {
                "default".to_string()
            } else if parent
                .as_ref()
                .is_some_and(|parent| ast::is_call_expression(store, *parent))
            {
                let parent = parent.as_ref().unwrap();
                let expression = store.expression(*parent).unwrap();
                let mut name = get_call_expression_name(store, expression);
                if !name.is_empty() {
                    name = clean_callback_text(&name);
                    if name.len() > MAX_LENGTH {
                        return format!("{name} callback");
                    }
                    let args = clean_callback_text(&get_call_expression_literal_args(
                        store,
                        source_file,
                        *parent,
                    ));
                    format!("{name}({args}) callback")
                } else {
                    "<function>".to_string()
                }
            } else {
                "<function>".to_string()
            }
        }
        ast::Kind::ClassDeclaration | ast::Kind::ClassExpression => {
            if ast::get_combined_modifier_flags(store, node).intersects(ast::ModifierFlags::Default)
            {
                "default".to_string()
            } else {
                "<class>".to_string()
            }
        }
        ast::Kind::Constructor => "constructor".to_string(),
        ast::Kind::CallSignature => "()".to_string(),
        ast::Kind::ConstructSignature => "new()".to_string(),
        ast::Kind::IndexSignature => "[]".to_string(),
        _ => String::new(),
    }
}

pub(crate) fn get_call_expression_name(store: &ast::AstStore, node: ast::Node) -> String {
    match store.kind(node) {
        ast::Kind::Identifier | ast::Kind::PrivateIdentifier => store.text(node),
        ast::Kind::PropertyAccessExpression => {
            let expression = store.expression(node).unwrap();
            let name = store.name(node).unwrap();
            let left = get_call_expression_name(store, expression);
            let right = get_call_expression_name(store, name);
            if !left.is_empty() {
                format!("{left}.{right}")
            } else {
                right
            }
        }
        _ => String::new(),
    }
}

pub fn get_call_expression_literal_args(
    store: &ast::AstStore,
    source_file: &ast::SourceFile,
    call_expr: ast::Node,
) -> String {
    let mut parts = Vec::new();
    if let Some(arguments) = store.arguments(call_expr) {
        for arg in arguments {
            if ast::is_string_literal_like(store, arg) || ast::is_template_expression(store, arg) {
                parts.push(scanner::get_text_of_node(source_file, &arg));
            }
        }
    }
    parts.join(", ")
}

pub fn clean_callback_text(text: &str) -> String {
    let mut text = if stringutil::truncate_by_runes(text, MAX_LENGTH as i32).len() < text.len() {
        format!(
            "{}...",
            stringutil::truncate_by_runes(text, MAX_LENGTH as i32)
        )
    } else {
        text.to_string()
    };
    text.retain(|r| !stringutil::is_line_break(r));
    text
}

pub(crate) fn get_interior_module(store: &ast::AstStore, mut node: ast::Node) -> ast::Node {
    while store
        .body(node)
        .as_ref()
        .is_some_and(|body| ast::is_module_declaration(store, *body))
    {
        node = store.body(node).unwrap();
    }
    node
}

pub(crate) fn get_module_name(store: &ast::AstStore, mut node: ast::Node) -> String {
    let mut result = store.text(store.name(node).unwrap());
    while store
        .body(node)
        .as_ref()
        .is_some_and(|body| ast::is_module_declaration(store, *body))
    {
        node = store.body(node).unwrap();
        result.push('.');
        result.push_str(&store.text(store.name(node).unwrap()));
    }
    result
}

pub struct DeclarationInfo {
    pub name: String,
    pub declaration: ast::Node,
    pub source_file: ast::SourceFile,
    pub match_score: i32,
}

impl Clone for DeclarationInfo {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            declaration: self.declaration,
            source_file: self.source_file.share_readonly(),
            match_score: self.match_score,
        }
    }
}

pub fn provide_workspace_symbols(
    ctx: &core::Context,
    programs: &[&compiler::Program],
    converters: &crate::Converters,
    preferences: lsutil::UserPreferences,
    query: &str,
) -> Result<lsproto::WorkspaceSymbolResponse, core::Error> {
    let exclude_library_symbols = preferences.exclude_library_symbols_in_nav_to.is_true();
    let mut source_files: HashMap<tspath::Path, ast::SourceFile> = HashMap::new();

    for program in programs {
        for source_file in program.source_files() {
            if (program.has_ts_file() || !source_file.is_declaration_file())
                && !should_exclude_file(&source_file, program, exclude_library_symbols)
            {
                source_files.insert(source_file.path(), source_file);
            }
        }
    }

    let mut infos = Vec::new();
    for source_file in source_files.values_mut() {
        if ctx.err().is_some() {
            return Ok(lsproto::SymbolInformationsOrWorkspaceSymbolsOrNull::default());
        }
        let _store = source_file.store();
        for (name, declarations) in get_declaration_map(source_file) {
            let score = get_match_score(&name, query);
            if score >= 0 {
                for declaration in declarations {
                    infos.push(DeclarationInfo {
                        name: name.clone(),
                        declaration,
                        source_file: source_file.share_readonly(),
                        match_score: score,
                    });
                }
            }
        }
    }

    infos.sort_by(compare_declaration_infos);
    let count = infos.len().min(256);
    let mut symbols = Vec::with_capacity(count);
    for info in infos.into_iter().take(count) {
        let node = info.declaration;
        let source_file = &info.source_file;
        let pos = astnav::get_start_of_node(node, source_file);
        let container = crate::utilities::get_container_node(source_file.store(), node);
        let container_name =
            container.map(|container| ast::get_declaration_name(source_file.store(), &container));
        symbols.push(Box::new(lsproto::SymbolInformation {
            name: info.name.to_string(),
            kind: get_symbol_kind_from_node(source_file.store(), node),
            location: converters.to_lsp_location(
                source_file,
                core::new_text_range(pos, source_file.store().loc(node).end()),
            ),
            container_name,
            ..Default::default()
        }));
    }

    Ok(lsproto::SymbolInformationsOrWorkspaceSymbolsOrNull {
        symbol_informations: Some(symbols.into_iter().map(Some).collect()),
        ..Default::default()
    })
}

fn get_declaration_map(source_file: &ast::SourceFile) -> HashMap<String, Vec<ast::Node>> {
    fn visit(store: &ast::AstStore, node: ast::Node, result: &mut HashMap<String, Vec<ast::Node>>) {
        let name = ast::get_declaration_name(store, &node);
        if !name.is_empty() {
            result.entry(name).or_default().push(node);
        }
        let _ = store.for_each_present_child(node, |child| {
            visit(store, child, result);
            std::ops::ControlFlow::Continue(())
        });
    }

    let mut result = HashMap::new();
    visit(source_file.store(), source_file.as_node(), &mut result);
    result
}

pub fn should_exclude_file(
    file: &ast::SourceFile,
    program: &compiler::Program,
    exclude_library_symbols: bool,
) -> bool {
    exclude_library_symbols
        && (is_inside_node_modules(&file.file_name()) || program.is_lib_file(file))
}

pub fn is_inside_node_modules(file_name: &str) -> bool {
    file_name.contains("/node_modules/")
}

pub fn get_match_score(mut s: &str, pattern: &str) -> i32 {
    let mut score = 0;
    for p in pattern.chars() {
        let exact = p.is_uppercase();
        loop {
            let Some(c) = s.chars().next() else {
                return -1;
            };
            s = &s[c.len_utf8()..];
            if (exact && c == p) || (!exact && to_lower_rune(c) == to_lower_rune(p)) {
                break;
            }
            score += 1;
        }
    }
    score
}

fn to_lower_rune(ch: char) -> char {
    ch.to_lowercase().next().unwrap_or(ch)
}

pub fn compare_declaration_infos(a: &DeclarationInfo, b: &DeclarationInfo) -> std::cmp::Ordering {
    a.match_score
        .cmp(&b.match_score)
        .then_with(|| stringutil::compare_strings_case_insensitive(&a.name, &b.name).cmp(&0))
        .then_with(|| a.name.cmp(&b.name))
        .then_with(|| {
            if a.source_file.path() != b.source_file.path() {
                return a
                    .source_file
                    .path()
                    .to_string()
                    .cmp(&b.source_file.path().to_string());
            }
            a.source_file
                .store()
                .loc(a.declaration)
                .pos()
                .cmp(&b.source_file.store().loc(b.declaration).pos())
        })
}

pub(crate) fn get_symbol_kind_from_node(
    store: &ast::AstStore,
    node: ast::Node,
) -> lsproto::SymbolKind {
    match store.kind(node) {
        ast::Kind::SourceFile => {
            if ast::is_external_module(&store.source_file_view(node)) {
                lsproto::SymbolKind::MODULE
            } else {
                lsproto::SymbolKind::FILE
            }
        }
        ast::Kind::ModuleDeclaration => lsproto::SymbolKind::NAMESPACE,
        ast::Kind::ClassDeclaration | ast::Kind::ClassExpression => lsproto::SymbolKind::CLASS,
        ast::Kind::InterfaceDeclaration => lsproto::SymbolKind::INTERFACE,
        ast::Kind::TypeAliasDeclaration => lsproto::SymbolKind::CLASS,
        ast::Kind::EnumDeclaration => lsproto::SymbolKind::ENUM,
        ast::Kind::VariableDeclaration => lsproto::SymbolKind::VARIABLE,
        ast::Kind::ArrowFunction
        | ast::Kind::FunctionDeclaration
        | ast::Kind::FunctionExpression => lsproto::SymbolKind::FUNCTION,
        ast::Kind::GetAccessor | ast::Kind::SetAccessor => lsproto::SymbolKind::PROPERTY,
        ast::Kind::MethodDeclaration | ast::Kind::MethodSignature => lsproto::SymbolKind::METHOD,
        ast::Kind::PropertyDeclaration
        | ast::Kind::PropertySignature
        | ast::Kind::PropertyAssignment
        | ast::Kind::ShorthandPropertyAssignment
        | ast::Kind::SpreadAssignment
        | ast::Kind::IndexSignature => lsproto::SymbolKind::PROPERTY,
        ast::Kind::CallSignature => lsproto::SymbolKind::METHOD,
        ast::Kind::ConstructSignature => lsproto::SymbolKind::CONSTRUCTOR,
        ast::Kind::Constructor | ast::Kind::ClassStaticBlockDeclaration => {
            lsproto::SymbolKind::CONSTRUCTOR
        }
        ast::Kind::TypeParameter => lsproto::SymbolKind::TYPE_PARAMETER,
        ast::Kind::EnumMember => lsproto::SymbolKind::ENUM_MEMBER,
        ast::Kind::Parameter => {
            if ast::has_syntactic_modifier(
                store,
                node,
                ast::ModifierFlags::ParameterPropertyModifier,
            ) {
                lsproto::SymbolKind::PROPERTY
            } else {
                lsproto::SymbolKind::VARIABLE
            }
        }
        ast::Kind::BinaryExpression | ast::Kind::CallExpression => {
            match ast::get_assignment_declaration_kind(store, node) {
                ast::JSDeclarationKind::ThisProperty
                | ast::JSDeclarationKind::Property
                | ast::JSDeclarationKind::ObjectDefinePropertyValue => {
                    lsproto::SymbolKind::PROPERTY
                }
                _ => lsproto::SymbolKind::VARIABLE,
            }
        }
        ast::Kind::StringLiteral
        | ast::Kind::NoSubstitutionTemplateLiteral
        | ast::Kind::NumericLiteral => lsproto::SymbolKind::PROPERTY,
        _ => lsproto::SymbolKind::VARIABLE,
    }
}
