pub mod commonjsmodule;
pub mod esmodule;
pub mod externalmoduleinfo;
pub mod impliedmodule;
pub mod utilities;

use std::collections::{HashMap, HashSet};

use ts_ast as ast;
use ts_ast::{AstGeneratedVisitEachChild as _, AstVisitEachChildRuntime as _};
use ts_core as core;
use ts_printer as printer;
use ts_scanner as scanner;

use crate::{SourceFileTransformer, TransformOptions, Transformer};

#[derive(Clone, Copy)]
struct ImportReference {
    import_declaration: ast::Node,
    property_name: Option<ast::Node>,
}

#[derive(Clone, Default)]
pub struct ModuleTransformFacts {
    pub common_js_module_indicator: Option<ast::Node>,
    pub external_module_indicator: Option<ast::Node>,
    pub source_file_root: Option<ast::Node>,
    pub referenced_source_file_export_references: HashSet<core::TextRange>,
    exported_names: Vec<String>,
    value_export_specifier_names: HashSet<String>,
    referenced_direct_exported_variable_references: HashSet<core::TextRange>,
    referenced_export_bindings: HashMap<core::TextRange, Vec<String>>,
    referenced_import_references: HashMap<core::TextRange, ImportReference>,
}

impl ModuleTransformFacts {
    fn references_source_file_export(&self, store: &ast::AstStore, node: ast::Node) -> bool {
        self.referenced_source_file_export_references
            .contains(&store.loc(node))
    }

    fn references_direct_exported_variable(&self, store: &ast::AstStore, node: ast::Node) -> bool {
        self.referenced_direct_exported_variable_references
            .contains(&store.loc(node))
    }
}

pub fn collect_module_transform_resolver_facts(
    source_file: &ast::SourceFile,
    resolver: &mut dyn printer::EmitResolver,
    compiler_options: &core::CompilerOptions,
) -> ModuleTransformFacts {
    let mut facts = ModuleTransformFacts::default();
    let store = source_file.store();
    let exported_facts = collect_exported_bindings_for_resolver_facts(
        store,
        source_file.root(),
        resolver,
        compiler_options.should_preserve_const_enums(),
    );
    facts.exported_names = exported_facts.exported_names.clone();
    facts.value_export_specifier_names = exported_facts.value_export_specifier_names.clone();
    let mut stack = vec![source_file.root()];
    while let Some(node) = stack.pop() {
        if ast::is_identifier(store, node)
            && let Some(parent) = store.parent(node)
        {
            let is_reference = crate::utilities::is_identifier_reference(store, &node, parent)
                || store.kind(parent) == ast::Kind::ShorthandPropertyAssignment
                    && store.name(parent) == Some(node);
            let is_declaration_name_of_enum_or_namespace = matches!(
                store.kind(parent),
                ast::Kind::EnumDeclaration | ast::Kind::ModuleDeclaration
            ) && store.name(parent) == Some(node);
            let is_default_class_or_function_declaration_name = matches!(
                store.kind(parent),
                ast::Kind::ClassDeclaration | ast::Kind::FunctionDeclaration
            ) && store.name(parent)
                == Some(node)
                && ast::has_syntactic_modifier(store, parent, ast::ModifierFlags::DEFAULT);
            if is_reference || is_declaration_name_of_enum_or_namespace {
                if let Some(container) = resolver
                    .get_referenced_export_container(node, is_declaration_name_of_enum_or_namespace)
                    && store.kind(container) == ast::Kind::SourceFile
                {
                    facts
                        .referenced_source_file_export_references
                        .insert(store.loc(node));
                }
            }
            if is_reference
                && resolver
                    .get_referenced_value_declarations(node)
                    .iter()
                    .any(|declaration| {
                        let declaration_source = resolver
                            .source_file_store(*declaration)
                            .expect("resolver declaration should belong to a source file");
                        is_direct_exported_variable_declaration(declaration_source, *declaration)
                    })
            {
                facts
                    .referenced_direct_exported_variable_references
                    .insert(store.loc(node));
            }
            if let Some(declaration) = resolver.get_referenced_import_declaration(node)
                && let Some(reference) =
                    import_reference_from_referenced_import_declaration(resolver, declaration)
            {
                facts
                    .referenced_import_references
                    .insert(store.loc(node), reference);
            }
            if !is_default_class_or_function_declaration_name {
                let exported_names = get_exported_bindings_for_resolver_fact(
                    store,
                    node,
                    resolver,
                    &exported_facts.exported_bindings,
                );
                if !exported_names.is_empty() {
                    facts
                        .referenced_export_bindings
                        .insert(store.loc(node), exported_names);
                }
            }
        }
        let _ = store.for_each_present_child(node, |child| {
            stack.push(child);
            std::ops::ControlFlow::Continue(())
        });
    }
    facts
}

fn is_direct_exported_variable_declaration(source: &ast::AstStore, declaration: ast::Node) -> bool {
    if ast::is_import_equals_declaration(source, declaration) {
        return ast::has_syntactic_modifier(source, declaration, ast::ModifierFlags::EXPORT)
            && !ast::is_external_module_import_equals_declaration(source, declaration);
    }
    if !ast::is_variable_declaration(source, declaration) {
        return false;
    }
    let Some(declaration_list) = source.parent(declaration) else {
        return false;
    };
    if !ast::is_variable_declaration_list(source, declaration_list) {
        return false;
    }
    let Some(statement) = source.parent(declaration_list) else {
        return false;
    };
    source.kind(statement) == ast::Kind::VariableStatement
        && ast::has_syntactic_modifier(source, statement, ast::ModifierFlags::EXPORT)
}

#[derive(Default)]
struct ExternalModuleExportFacts {
    exported_bindings: HashMap<ast::Node, Vec<String>>,
    exported_names: Vec<String>,
    value_export_specifier_names: HashSet<String>,
}

fn collect_exported_bindings_for_resolver_facts(
    source: &ast::AstStore,
    file: ast::Node,
    resolver: &mut dyn printer::EmitResolver,
    preserve_const_enums: bool,
) -> ExternalModuleExportFacts {
    let mut exported_bindings = HashMap::new();
    let mut exported_names = Vec::new();
    let mut value_export_specifier_names = HashSet::new();
    let mut unique_exports = HashSet::new();
    let mut has_export_default = false;
    for node in source
        .parser_access()
        .source_file_statement_list(file)
        .iter()
    {
        match source.kind(node) {
            ast::Kind::ExportDeclaration => {
                if source.is_type_only(node).unwrap_or(false) {
                    continue;
                }
                let Some(export_clause) = source.export_clause(node) else {
                    continue;
                };
                if !ast::is_named_exports(source, export_clause) {
                    continue;
                }
                let Some(elements) = source.source_elements(export_clause) else {
                    continue;
                };
                for specifier in elements.iter() {
                    if source.is_type_only(specifier).unwrap_or(false) {
                        continue;
                    }
                    if !resolver.is_value_alias_declaration(specifier) {
                        continue;
                    }
                    let Some(exported_name) = source.name(specifier) else {
                        continue;
                    };
                    let specifier_name_text = source.text(exported_name);
                    if !unique_exports.insert(specifier_name_text.clone()) {
                        continue;
                    }
                    value_export_specifier_names.insert(specifier_name_text.clone());
                    let name = source
                        .property_name_or_name(specifier)
                        .unwrap_or(exported_name);
                    if source.kind(name) != ast::Kind::StringLiteral {
                        let import_declaration = resolver.get_referenced_import_declaration(name);
                        let declaration = import_declaration
                            .or_else(|| resolver.get_referenced_value_declaration(name));
                        if let Some(declaration) = declaration {
                            let declaration_source = resolver
                                .source_file_store(declaration)
                                .expect("resolver declaration should belong to a source file");
                            if declaration_source.kind(declaration)
                                == ast::Kind::FunctionDeclaration
                            {
                                unique_exports.remove(&specifier_name_text);
                                add_exported_function_declaration_for_resolver_facts(
                                    declaration_source,
                                    &mut exported_bindings,
                                    &mut exported_names,
                                    &mut unique_exports,
                                    &mut has_export_default,
                                    declaration,
                                    Some(specifier_name_text),
                                    ast::module_export_name_is_default(source, exported_name),
                                );
                                continue;
                            }
                            exported_bindings
                                .entry(declaration)
                                .or_insert_with(Vec::new)
                                .push(specifier_name_text.clone());
                        }
                    }
                    exported_names.push(specifier_name_text);
                }
            }
            ast::Kind::VariableStatement => {
                if !ast::has_syntactic_modifier(source, node, ast::ModifierFlags::EXPORT) {
                    continue;
                }
                if ast::has_ambient_modifier(source, node) {
                    continue;
                }
                let Some(declaration_list) = source.declaration_list(node) else {
                    continue;
                };
                let Some(declarations) = source.declarations(declaration_list) else {
                    continue;
                };
                for declaration in declarations.iter() {
                    collect_exported_variable_info_for_resolver_facts(
                        source,
                        &mut exported_bindings,
                        &mut exported_names,
                        &mut unique_exports,
                        preserve_const_enums,
                        declaration,
                    );
                }
            }
            ast::Kind::FunctionDeclaration => {
                if ast::has_syntactic_modifier(source, node, ast::ModifierFlags::EXPORT)
                    && source.body(node).is_some()
                {
                    add_exported_function_declaration_for_resolver_facts(
                        source,
                        &mut exported_bindings,
                        &mut exported_names,
                        &mut unique_exports,
                        &mut has_export_default,
                        node,
                        None,
                        ast::has_syntactic_modifier(source, node, ast::ModifierFlags::DEFAULT),
                    );
                }
            }
            ast::Kind::ClassDeclaration => {
                if !ast::has_syntactic_modifier(source, node, ast::ModifierFlags::EXPORT) {
                    continue;
                }
                if !is_runtime_value_declaration(source, node, preserve_const_enums) {
                    continue;
                }
                if ast::has_syntactic_modifier(source, node, ast::ModifierFlags::DEFAULT) {
                    if !has_export_default {
                        let name = source
                            .name(node)
                            .map(|name| source.text(name))
                            .unwrap_or_else(|| "default".to_owned());
                        exported_bindings
                            .entry(node)
                            .or_insert_with(Vec::new)
                            .push(name.clone());
                        has_export_default = true;
                    }
                } else if let Some(name) = source.name(node) {
                    let name_text = source.text(name);
                    if unique_exports.insert(name_text.clone()) {
                        exported_bindings
                            .entry(node)
                            .or_insert_with(Vec::new)
                            .push(name_text.clone());
                        exported_names.push(name_text);
                    }
                }
            }
            ast::Kind::EnumDeclaration | ast::Kind::ModuleDeclaration => {
                if !ast::has_syntactic_modifier(source, node, ast::ModifierFlags::EXPORT) {
                    continue;
                }
                if !is_runtime_value_declaration(source, node, preserve_const_enums) {
                    continue;
                }
                if let Some(name) = source.name(node)
                    && ast::is_identifier(source, name)
                {
                    let name_text = source.text(name);
                    if !has_top_level_default_class_or_function_declaration(
                        source, file, &name_text,
                    ) && unique_exports.insert(name_text.clone())
                    {
                        exported_bindings
                            .entry(node)
                            .or_insert_with(Vec::new)
                            .push(name_text.clone());
                        exported_names.push(name_text);
                    }
                }
            }
            _ => {}
        }
    }
    ExternalModuleExportFacts {
        exported_bindings,
        exported_names,
        value_export_specifier_names,
    }
}

fn add_exported_function_declaration_for_resolver_facts(
    source: &ast::AstStore,
    exported_bindings: &mut HashMap<ast::Node, Vec<String>>,
    _exported_names: &mut Vec<String>,
    unique_exports: &mut HashSet<String>,
    has_export_default: &mut bool,
    node: ast::Node,
    name: Option<String>,
    is_default: bool,
) {
    if is_default {
        if !*has_export_default {
            let name = name
                .or_else(|| source.name(node).map(|name| source.text(name)))
                .unwrap_or_else(|| "default".to_owned());
            exported_bindings
                .entry(node)
                .or_insert_with(Vec::new)
                .push(name);
            *has_export_default = true;
        }
    } else {
        let Some(name) = name.or_else(|| source.name(node).map(|name| source.text(name))) else {
            return;
        };
        if unique_exports.insert(name.clone()) {
            exported_bindings
                .entry(node)
                .or_insert_with(Vec::new)
                .push(name);
        }
    }
}

fn collect_exported_variable_info_for_resolver_facts(
    source: &ast::AstStore,
    exported_bindings: &mut HashMap<ast::Node, Vec<String>>,
    exported_names: &mut Vec<String>,
    unique_exports: &mut HashSet<String>,
    preserve_const_enums: bool,
    declaration: ast::Node,
) {
    if !is_runtime_value_declaration(source, declaration, preserve_const_enums) {
        return;
    }
    let Some(name) = source.name(declaration) else {
        return;
    };
    if ast::is_binding_pattern(source, name) {
        if let Some(elements) = source.source_elements(name) {
            for element in elements.iter() {
                if !ast::is_omitted_expression(source, element) {
                    collect_exported_variable_info_for_resolver_facts(
                        source,
                        exported_bindings,
                        exported_names,
                        unique_exports,
                        preserve_const_enums,
                        element,
                    );
                }
            }
        }
    } else {
        let text = source.text(name);
        if unique_exports.insert(text.clone()) {
            exported_bindings
                .entry(declaration)
                .or_insert_with(Vec::new)
                .push(text.clone());
            exported_names.push(text);
        }
    }
}

fn has_top_level_default_class_or_function_declaration(
    source: &ast::AstStore,
    file: ast::Node,
    name: &str,
) -> bool {
    source
        .parser_access()
        .source_file_statement_list(file)
        .iter()
        .any(|statement| {
            (ast::is_class_declaration(source, statement)
                || ast::is_function_declaration(source, statement))
                && ast::has_syntactic_modifier(source, statement, ast::ModifierFlags::DEFAULT)
                && source.name(statement).is_some_and(|node| {
                    ast::is_identifier(source, node) && source.text(node) == name
                })
        })
}

fn is_runtime_value_declaration(
    source: &ast::AstStore,
    declaration: ast::Node,
    preserve_const_enums: bool,
) -> bool {
    if ast::has_ambient_modifier(source, declaration) {
        return false;
    }
    match source.kind(declaration) {
        ast::Kind::ClassDeclaration
        | ast::Kind::FunctionDeclaration
        | ast::Kind::VariableDeclaration
        | ast::Kind::BindingElement
        | ast::Kind::ImportDeclaration
        | ast::Kind::ImportEqualsDeclaration => true,
        ast::Kind::EnumDeclaration => {
            preserve_const_enums || !ast::is_enum_const(source, declaration)
        }
        ast::Kind::ModuleDeclaration => {
            ast::is_instantiated_module(source, declaration, preserve_const_enums)
        }
        _ => false,
    }
}

fn get_exported_bindings_for_resolver_fact(
    source: &ast::AstStore,
    node: ast::Node,
    resolver: &mut dyn printer::EmitResolver,
    exported_bindings: &HashMap<ast::Node, Vec<String>>,
) -> Vec<String> {
    if let Some(declaration) = resolver.get_referenced_import_declaration(node) {
        return exported_bindings
            .get(&declaration)
            .cloned()
            .unwrap_or_default();
    }

    let mut bindings = Vec::new();
    let mut seen = HashSet::new();
    for declaration in resolver.get_referenced_value_declarations(node) {
        if let Some(exported_names) = exported_bindings.get(&declaration) {
            for binding in exported_names {
                if seen.insert(binding.clone()) {
                    bindings.push(binding.clone());
                }
            }
        }
    }
    if bindings.is_empty()
        && let Some(parent) = source.parent(node)
        && exported_bindings.contains_key(&parent)
    {
        return exported_bindings.get(&parent).cloned().unwrap_or_default();
    }
    bindings
}

fn import_reference_from_referenced_import_declaration(
    resolver: &mut dyn printer::EmitResolver,
    declaration: ast::Node,
) -> Option<ImportReference> {
    let source = resolver.source_file_store(declaration)?;
    match source.kind(declaration) {
        ast::Kind::ImportClause => {
            let import_declaration = source.parent(declaration)?;
            if source.kind(import_declaration) == ast::Kind::ImportDeclaration {
                Some(ImportReference {
                    import_declaration,
                    property_name: None,
                })
            } else {
                None
            }
        }
        ast::Kind::ImportSpecifier => {
            let mut ancestor = Some(declaration);
            while let Some(node) = ancestor {
                if source.kind(node) == ast::Kind::ImportDeclaration {
                    return Some(ImportReference {
                        import_declaration: node,
                        property_name: source.property_name_or_name(declaration),
                    });
                }
                ancestor = source.parent(node);
            }
            None
        }
        _ => None,
    }
}

pub fn new_es_module_transformer(
    opts: &TransformOptions,
    file_module_format: core::ModuleKind,
) -> Transformer {
    let mut tx = Transformer::default();
    tx.new_source_file_transformer(
        SourceFileTransformer::EsModule {
            compiler_options: opts.compiler_options.clone(),
            file_module_format,
        },
        Some(opts.context.fork()),
    );
    tx
}

pub fn new_implied_module_transformer(
    opts: &TransformOptions,
    file_module_format: core::ModuleKind,
) -> Transformer {
    let mut tx = Transformer::default();
    tx.new_source_file_transformer(
        SourceFileTransformer::ImpliedModule {
            compiler_options: opts.compiler_options.clone(),
            file_module_format,
            facts: opts.module_transform_facts.clone(),
        },
        Some(opts.context.fork()),
    );
    tx
}

pub fn new_common_js_module_transformer(opts: &TransformOptions) -> Transformer {
    let mut tx = Transformer::default();
    tx.new_source_file_transformer(
        SourceFileTransformer::CommonJsModule {
            compiler_options: opts.compiler_options.clone(),
            facts: opts.module_transform_facts.clone(),
        },
        Some(opts.context.fork()),
    );
    tx
}

pub(crate) fn visit_es_module_source_file_root_output(
    file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
    compiler_options: &core::CompilerOptions,
    file_module_format: core::ModuleKind,
) -> Option<ast::Node> {
    let factory_store_id = emit_context.factory.node_factory.store().store_id();
    if root.store_id() == factory_store_id {
        return visit_es_module_active_source_file_root_output(
            file,
            root,
            emit_context,
            compiler_options,
            file_module_format,
        );
    }
    assert_eq!(
        root.store_id(),
        file.store().store_id(),
        "ES module transform root must come from the input source or active emit factory"
    );
    let source = file.store();
    let source_file = source.as_source_file(root);
    let is_declaration_file = source_file.is_declaration_file();
    let is_external_module = source_file.external_module_indicator().is_some()
        || file.external_module_indicator().is_some();
    let end_of_file_token = source_file.end_of_file_token();
    let facts = esmodule::EsModuleFacts {
        is_declaration_file,
        is_external_module,
        isolated_modules: compiler_options.get_isolated_modules(),
        ..Default::default()
    };
    if esmodule::es_module_action_for_kind(ast::Kind::SourceFile, facts)
        == esmodule::EsModuleAction::SkipSourceFile
    {
        return None;
    }

    let source_statements = source.parser_access().source_file_statement_list(root);
    emit_context.add_requested_emit_helpers(&root);
    let source_statement_nodes = source_statements.iter().collect::<Vec<_>>();
    let mut import_require_statements = None;
    let mut visited_statements = Vec::new();
    let mut changed = false;
    for statement in &source_statement_nodes {
        if ast::is_import_equals_declaration(source, *statement) {
            let transformed = visit_es_module_import_equals_declaration(
                source,
                emit_context,
                statement,
                compiler_options,
                &mut import_require_statements,
            );
            visited_statements.extend(transformed);
            changed = true;
        } else if ast::is_export_assignment(source, *statement) {
            let transformed = visit_es_module_export_assignment(
                source,
                emit_context,
                statement,
                compiler_options,
            );
            visited_statements.extend(transformed);
            changed = true;
        } else if ast::is_export_declaration(source, *statement) {
            let transformed = visit_es_module_export_declaration(
                source,
                emit_context,
                statement,
                compiler_options,
            );
            changed |= transformed.len() != 1 || transformed[0] != *statement;
            visited_statements.extend(transformed);
        } else if es_module_statement_needs_rewrite(source, *statement, compiler_options) {
            let transformed = visit_es_module_rewrite_statement(
                source,
                emit_context,
                *statement,
                compiler_options,
            );
            visited_statements.push(transformed);
            changed = true;
        } else {
            let mut importer =
                ast::AstImporter::new(source, &mut emit_context.factory.node_factory);
            visited_statements.push(importer.preserve_node(*statement));
        }
    }
    emit_context.add_requested_emit_helpers(&root);

    let external_helpers_import_declaration = create_external_helpers_import_declaration_if_needed(
        file,
        root,
        emit_context,
        compiler_options,
        file_module_format,
    );
    let has_external_module_indicator = external_helpers_import_declaration.is_some() || {
        let output_store = emit_context.factory.store();
        visited_statements
            .iter()
            .chain(
                import_require_statements
                    .as_ref()
                    .into_iter()
                    .flat_map(|statements| statements.statements.iter()),
            )
            .any(|statement| ast::is_external_module_indicator(output_store, *statement))
    };
    let needs_empty_imports_marker = esmodule::needs_empty_imports_marker(
        is_external_module,
        compiler_options.get_emit_module_kind(),
        has_external_module_indicator,
    );
    if external_helpers_import_declaration.is_none()
        && import_require_statements.is_none()
        && !needs_empty_imports_marker
        && !changed
    {
        return None;
    }

    let mut statements = Vec::new();
    let mut rest = visited_statements.as_slice();
    if external_helpers_import_declaration.is_some() || import_require_statements.is_some() {
        let (prologue, after_prologue) = {
            let output_store = emit_context.factory.store();
            split_standard_prologue_in_store(output_store, &visited_statements)
        };
        let (custom, after_custom) = split_custom_prologue_in_store(emit_context, after_prologue);
        rest = after_custom;
        statements.extend(prologue.iter().chain(custom.iter()).copied());
        if let Some(external_helpers_import_declaration) = external_helpers_import_declaration {
            // The helpers import must be visited so that `import x = require("tslib")`
            // (TypeScript-only syntax) is transformed to `const x = require("tslib")`
            // for CJS output files via visitImportEqualsDeclaration.
            statements.push(external_helpers_import_declaration);
        }
        if let Some(import_require_statements) = import_require_statements {
            statements.extend(import_require_statements.statements);
        }
    }

    statements.extend(rest.iter().copied());
    if needs_empty_imports_marker {
        statements.push(create_empty_imports(&mut emit_context.factory.node_factory));
    }
    let statements = emit_context.factory.node_factory.new_node_list(
        source_statements.loc(),
        source_statements.range(),
        statements,
    );
    if root.store_id() == emit_context.factory.node_factory.store().store_id() {
        Some(
            emit_context
                .factory
                .node_factory
                .update_source_file_in_current_store(root, statements, end_of_file_token),
        )
    } else {
        let source_file = source.as_source_file(root);
        let mut importer = ast::AstImporter::new(source, &mut emit_context.factory.node_factory);
        let end_of_file_token = importer.preserve_optional_node(end_of_file_token);
        Some(importer.factory().update_source_file_from_store(
            source,
            root,
            source_file,
            Some(statements),
            end_of_file_token,
        ))
    }
}

#[derive(Clone, Copy)]
enum EsModuleStatementAction {
    Keep(ast::Node),
    ImportEquals(ast::Node),
    ExportAssignment(ast::Node),
    ExportDeclaration(ast::Node),
}

pub(crate) fn visit_es_module_active_source_file_root_output(
    file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
    compiler_options: &core::CompilerOptions,
    file_module_format: core::ModuleKind,
) -> Option<ast::Node> {
    let source = file.store();
    let (
        is_declaration_file,
        is_external_module,
        statements_loc,
        statements_range,
        end_of_file_token,
        actions,
    ) = {
        let active_source = emit_context.factory.node_factory.store();
        let source_file = active_source.source_file_view(root);
        let source_statements = active_source
            .source_statements(root)
            .expect("source file should have statements");
        let actions = source_statements
            .iter()
            .map(|statement| {
                let active_kind = active_source.kind(statement);
                let parsed = || {
                    emit_context
                        .parse_node(&statement)
                        .filter(|node| node.store_id() == source.store_id())
                };
                match active_kind {
                    ast::Kind::ImportEqualsDeclaration => parsed()
                        .map(EsModuleStatementAction::ImportEquals)
                        .unwrap_or(EsModuleStatementAction::Keep(statement)),
                    ast::Kind::ExportAssignment => parsed()
                        .filter(|node| source.is_export_equals(*node).unwrap_or(false))
                        .map(EsModuleStatementAction::ExportAssignment)
                        .unwrap_or(EsModuleStatementAction::Keep(statement)),
                    ast::Kind::ExportDeclaration => {
                        EsModuleStatementAction::ExportDeclaration(statement)
                    }
                    _ => EsModuleStatementAction::Keep(statement),
                }
            })
            .collect::<Vec<_>>();
        (
            source_file.is_declaration_file(),
            source_file.external_module_indicator().is_some()
                || file.external_module_indicator().is_some(),
            source_statements.loc(),
            source_statements.range(),
            source_file.end_of_file_token(),
            actions,
        )
    };
    let facts = esmodule::EsModuleFacts {
        is_declaration_file,
        is_external_module,
        isolated_modules: compiler_options.get_isolated_modules(),
        ..Default::default()
    };
    if esmodule::es_module_action_for_kind(ast::Kind::SourceFile, facts)
        == esmodule::EsModuleAction::SkipSourceFile
    {
        return None;
    }

    emit_context.add_requested_emit_helpers(&root);
    let mut import_require_statements = None;
    let mut visited_statements = Vec::new();
    let mut changed = false;
    for action in actions {
        match action {
            EsModuleStatementAction::ImportEquals(statement) => {
                let transformed = visit_es_module_import_equals_declaration(
                    source,
                    emit_context,
                    &statement,
                    compiler_options,
                    &mut import_require_statements,
                );
                visited_statements.extend(transformed);
                changed = true;
            }
            EsModuleStatementAction::ExportAssignment(statement) => {
                let transformed = visit_es_module_export_assignment(
                    source,
                    emit_context,
                    &statement,
                    compiler_options,
                );
                visited_statements.extend(transformed);
                changed = true;
            }
            EsModuleStatementAction::ExportDeclaration(statement) => {
                let transformed = visit_es_module_active_export_declaration(
                    emit_context,
                    &statement,
                    compiler_options,
                );
                changed |= transformed.len() != 1 || transformed[0] != statement;
                visited_statements.extend(transformed);
            }
            EsModuleStatementAction::Keep(statement) => {
                let needs_rewrite = {
                    let source = emit_context.factory.node_factory.store();
                    es_module_statement_needs_rewrite(source, statement, compiler_options)
                };
                if needs_rewrite {
                    let transformed = visit_es_module_rewrite_statement(
                        source,
                        emit_context,
                        statement,
                        compiler_options,
                    );
                    visited_statements.push(transformed);
                    changed = true;
                } else {
                    visited_statements.push(statement);
                }
            }
        }
    }
    emit_context.add_requested_emit_helpers(&root);

    let external_helpers_import_declaration = create_external_helpers_import_declaration_if_needed(
        file,
        root,
        emit_context,
        compiler_options,
        file_module_format,
    );
    let has_external_module_indicator = external_helpers_import_declaration.is_some() || {
        let output_store = emit_context.factory.store();
        visited_statements
            .iter()
            .chain(
                import_require_statements
                    .as_ref()
                    .into_iter()
                    .flat_map(|statements| statements.statements.iter()),
            )
            .any(|statement| ast::is_external_module_indicator(output_store, *statement))
    };
    let needs_empty_imports_marker = esmodule::needs_empty_imports_marker(
        is_external_module,
        compiler_options.get_emit_module_kind(),
        has_external_module_indicator,
    );
    if external_helpers_import_declaration.is_none()
        && import_require_statements.is_none()
        && !needs_empty_imports_marker
        && !changed
    {
        return None;
    }

    let mut statements = Vec::new();
    let mut rest = visited_statements.as_slice();
    if external_helpers_import_declaration.is_some() || import_require_statements.is_some() {
        let (prologue, after_prologue) = {
            let output_store = emit_context.factory.store();
            split_standard_prologue_in_store(output_store, &visited_statements)
        };
        let (custom, after_custom) = split_custom_prologue_in_store(emit_context, after_prologue);
        rest = after_custom;
        statements.extend(prologue.iter().chain(custom.iter()).copied());
        if let Some(external_helpers_import_declaration) = external_helpers_import_declaration {
            statements.push(external_helpers_import_declaration);
        }
        if let Some(import_require_statements) = import_require_statements {
            statements.extend(import_require_statements.statements);
        }
    }

    statements.extend(rest.iter().copied());
    if needs_empty_imports_marker {
        statements.push(create_empty_imports(&mut emit_context.factory.node_factory));
    }
    let statements = emit_context.factory.node_factory.new_node_list(
        statements_loc,
        statements_range,
        statements,
    );
    Some(
        emit_context
            .factory
            .node_factory
            .update_source_file_in_current_store(root, statements, end_of_file_token),
    )
}

fn split_standard_prologue_in_store<'a>(
    store: &ast::AstStore,
    source: &'a [ast::Node],
) -> (&'a [ast::Node], &'a [ast::Node]) {
    for (i, statement) in source.iter().enumerate() {
        if !ast::is_prologue_directive(store, *statement) {
            return (&source[..i], &source[i..]);
        }
    }
    (source, &[])
}

fn split_custom_prologue_in_store<'a>(
    emit_context: &mut printer::EmitContext,
    source: &'a [ast::Node],
) -> (&'a [ast::Node], &'a [ast::Node]) {
    for (i, statement) in source.iter().enumerate() {
        if emit_context.emit_flags(statement) & printer::EF_CUSTOM_PROLOGUE == 0 {
            return (&source[..i], &source[i..]);
        }
    }
    (&[], source)
}

struct ImportRequireStatements {
    statements: Vec<ast::Node>,
    require_helper_name: ast::Node,
}

fn visit_es_module_import_equals_declaration(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    node: &ast::Node,
    compiler_options: &core::CompilerOptions,
    import_require_statements: &mut Option<ImportRequireStatements>,
) -> Vec<ast::Node> {
    // Though an error in es2020 modules, in node-flavor es2020 modules, we can helpfully transform this to a synthetic `require` call
    // To give easy access to a synchronous `require` in node-flavor esm. We do the transform even in scenarios where we error, but `import.meta.url`
    // is available, just because the output is reasonable for a node-like runtime.
    if compiler_options.get_emit_module_kind() < core::ModuleKind::Node16 {
        return Vec::new();
    }

    if !ast::is_external_module_import_equals_declaration(source, *node) {
        panic!(
            "import= for internal module references should be handled in an earlier transformer."
        )
    }

    let name = {
        let name = source
            .name(*node)
            .expect("import equals declaration should have a name");
        emit_context
            .factory
            .node_factory
            .deep_clone_node_from_store_preserve_location(source, name)
    };
    let require_call = create_es_module_require_call(
        source,
        emit_context,
        node,
        compiler_options,
        import_require_statements,
    );
    let var_statement = {
        let factory = &mut emit_context.factory.node_factory;
        let declaration = factory.new_variable_declaration(name, None, None, require_call);
        let declarations = factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![declaration],
        );
        let declaration_list =
            factory.new_variable_declaration_list(declarations, ast::NodeFlags::CONST);
        factory.new_variable_statement(None, declaration_list)
    };
    emit_context.set_original(&var_statement, node);
    emit_context.assign_comment_and_source_map_ranges(&var_statement, node);

    let mut statements = vec![var_statement];
    statements.extend(append_exports_of_es_module_import_equals_declaration(
        source,
        emit_context,
        node,
    ));
    statements
}

fn append_exports_of_es_module_import_equals_declaration(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    node: &ast::Node,
) -> Vec<ast::Node> {
    if !ast::has_syntactic_modifier(source, *node, ast::ModifierFlags::EXPORT) {
        return Vec::new();
    }
    let name = source
        .name(*node)
        .expect("import equals declaration should have a name");
    let name = emit_context
        .factory
        .node_factory
        .deep_clone_node_from_store_preserve_location(source, name);
    let factory = &mut emit_context.factory.node_factory;
    let export_specifier = factory.new_export_specifier(false, None, name);
    let elements = factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        vec![export_specifier],
    );
    let named_exports = factory.new_named_exports(elements);
    vec![factory.new_export_declaration(None, false, named_exports, None, None)]
}

fn visit_es_module_export_assignment(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    node: &ast::Node,
    compiler_options: &core::CompilerOptions,
) -> Vec<ast::Node> {
    if !source.is_export_equals(*node).unwrap_or(false) {
        let mut importer = ast::AstImporter::new(source, &mut emit_context.factory.node_factory);
        return vec![importer.preserve_node(*node)];
    }
    if compiler_options.get_emit_module_kind() != core::ModuleKind::Preserve {
        // Elide `export=` as it is not legal with --module ES6
        return Vec::new();
    }
    let expression = source
        .expression(*node)
        .expect("export assignment should have an expression");
    let expression = {
        let mut importer = ast::AstImporter::new(source, &mut emit_context.factory.node_factory);
        importer.preserve_node(expression)
    };
    let factory = &mut emit_context.factory.node_factory;
    let module = factory.new_identifier("module");
    let exports = factory.new_identifier("exports");
    let module_exports =
        factory.new_property_access_expression(module, None, exports, ast::NodeFlags::NONE);
    let assignment = emit_context
        .factory
        .new_assignment_expression(module_exports, expression);
    let statement = emit_context.factory.new_expression_statement(assignment);
    emit_context.set_original(&statement, node);
    vec![statement]
}

fn visit_es_module_active_export_declaration(
    emit_context: &mut printer::EmitContext,
    node: &ast::Node,
    compiler_options: &core::CompilerOptions,
) -> Vec<ast::Node> {
    let (
        module_specifier,
        rewritten_module_specifier_text,
        export_clause,
        is_namespace_export,
        namespace_export_name,
        namespace_reexport_outputs_default_assignment,
        attributes,
    ) = {
        let source = emit_context.factory.node_factory.store();
        let module_specifier = source.module_specifier(*node);
        let rewritten_module_specifier_text = module_specifier.and_then(|module_specifier| {
            if !ast::is_string_literal(source, module_specifier) {
                return None;
            }
            let text = source.text(module_specifier);
            crate::moduletransforms::utilities::rewrite_module_specifier_text(
                &text,
                compiler_options,
            )
        });
        let export_clause = source.export_clause(*node);
        let is_namespace_export = export_clause
            .is_some_and(|export_clause| ast::is_namespace_export(source, export_clause));
        let namespace_export_name = if is_namespace_export {
            export_clause.and_then(|export_clause| source.name(export_clause))
        } else {
            None
        };
        let namespace_reexport_outputs_default_assignment = namespace_export_name
            .is_some_and(|name| ast::module_export_name_is_default(source, name));
        (
            module_specifier,
            rewritten_module_specifier_text,
            export_clause,
            is_namespace_export,
            namespace_export_name,
            namespace_reexport_outputs_default_assignment,
            source.attributes(*node),
        )
    };

    let Some(module_specifier) = module_specifier else {
        return vec![*node];
    };

    let updated_module_specifier = rewritten_module_specifier_text
        .map(|text| {
            emit_context
                .factory
                .node_factory
                .new_string_literal(text, ast::TokenFlags::NONE)
        })
        .unwrap_or(module_specifier);

    if compiler_options.module > core::ModuleKind::ES2015
        || export_clause.is_none()
        || !is_namespace_export
    {
        let updated = emit_context.factory.node_factory.update_export_declaration(
            *node,
            None,
            false,
            export_clause,
            Some(updated_module_specifier),
            attributes,
        );
        return vec![updated];
    }

    let old_identifier = namespace_export_name.expect("namespace export should have a name");
    let synth_name = emit_context.new_generated_name_for_node(old_identifier);
    let import_decl = {
        let factory = &mut emit_context.factory.node_factory;
        let namespace_import = factory.new_namespace_import(synth_name);
        let import_clause =
            factory.new_import_clause(None::<ast::Kind>, None::<ast::Node>, namespace_import);
        factory.new_import_declaration(
            None::<ast::ModifierList>,
            import_clause,
            updated_module_specifier,
            attributes,
        )
    };
    emit_context.set_original(
        &import_decl,
        &export_clause.expect("namespace export should have export clause"),
    );

    let export_decl = if namespace_reexport_outputs_default_assignment {
        emit_context.factory.node_factory.new_export_assignment(
            None::<ast::ModifierList>,
            false,
            None::<ast::Node>,
            synth_name,
        )
    } else {
        let factory = &mut emit_context.factory.node_factory;
        let export_specifier = factory.new_export_specifier(false, synth_name, old_identifier);
        let elements = factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![export_specifier],
        );
        let named_exports = factory.new_named_exports(elements);
        factory.new_export_declaration(
            None::<ast::ModifierList>,
            false,
            named_exports,
            None::<ast::Node>,
            None::<ast::Node>,
        )
    };
    emit_context.set_original(&export_decl, node);
    vec![import_decl, export_decl]
}

fn visit_es_module_export_declaration(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    node: &ast::Node,
    compiler_options: &core::CompilerOptions,
) -> Vec<ast::Node> {
    let Some(module_specifier) = source.module_specifier(*node) else {
        return vec![*node];
    };

    let updated_module_specifier =
        rewrite_es_module_specifier(source, emit_context, module_specifier, compiler_options);
    let export_clause = source.export_clause(*node);
    let is_namespace_export =
        export_clause.is_some_and(|export_clause| ast::is_namespace_export(source, export_clause));
    if compiler_options.module > core::ModuleKind::ES2015
        || export_clause.is_none()
        || !is_namespace_export
    {
        let (export_clause, attributes) = {
            let mut importer =
                ast::AstImporter::new(source, &mut emit_context.factory.node_factory);
            (
                export_clause.map(|export_clause| importer.preserve_node(export_clause)),
                source
                    .attributes(*node)
                    .map(|attributes| importer.preserve_node(attributes)),
            )
        };
        let updated = emit_context
            .factory
            .node_factory
            .update_export_declaration_from_store(
                source,
                *node,
                None::<ast::ModifierList>,
                false,
                export_clause,
                Some(updated_module_specifier),
                attributes,
            );
        return vec![updated];
    }

    let export_clause = export_clause.expect("namespace export should have export clause");
    let old_identifier = source
        .name(export_clause)
        .expect("namespace export should have a name");
    let synth_name = emit_context.new_generated_name_for_node(old_identifier);
    let attributes = {
        let mut importer = ast::AstImporter::new(source, &mut emit_context.factory.node_factory);
        source
            .attributes(*node)
            .map(|attributes| importer.preserve_node(attributes))
    };
    let import_decl = {
        let factory = &mut emit_context.factory.node_factory;
        let namespace_import = factory.new_namespace_import(synth_name);
        let import_clause =
            factory.new_import_clause(None::<ast::Kind>, None::<ast::Node>, namespace_import);
        factory.new_import_declaration(
            None::<ast::ModifierList>,
            import_clause,
            updated_module_specifier,
            attributes,
        )
    };
    emit_context.set_original(&import_decl, &export_clause);

    let export_decl = if esmodule::namespace_reexport_outputs_default_assignment(
        source
            .name(export_clause)
            .is_some_and(|name| ast::module_export_name_is_default(source, name)),
    ) {
        emit_context.factory.node_factory.new_export_assignment(
            None::<ast::ModifierList>,
            false,
            None::<ast::Node>,
            synth_name,
        )
    } else {
        let old_identifier = emit_context
            .factory
            .node_factory
            .deep_clone_node_from_store_preserve_location(source, old_identifier);
        let factory = &mut emit_context.factory.node_factory;
        let export_specifier = factory.new_export_specifier(false, synth_name, old_identifier);
        let elements = factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![export_specifier],
        );
        let named_exports = factory.new_named_exports(elements);
        factory.new_export_declaration(
            None::<ast::ModifierList>,
            false,
            named_exports,
            None::<ast::Node>,
            None::<ast::Node>,
        )
    };
    emit_context.set_original(&export_decl, node);
    vec![import_decl, export_decl]
}

fn rewrite_es_module_specifier(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    module_specifier: ast::Node,
    compiler_options: &core::CompilerOptions,
) -> ast::Node {
    if ast::is_string_literal(source, module_specifier) {
        let text = source.text(module_specifier);
        if let Some(text) = crate::moduletransforms::utilities::rewrite_module_specifier_text(
            &text,
            compiler_options,
        ) {
            return emit_context
                .factory
                .node_factory
                .new_string_literal(text, ast::TokenFlags::NONE);
        }
    }
    let mut importer = ast::AstImporter::new(source, &mut emit_context.factory.node_factory);
    importer.preserve_node(module_specifier)
}

fn es_module_statement_needs_rewrite(
    source: &ast::AstStore,
    node: ast::Node,
    compiler_options: &core::CompilerOptions,
) -> bool {
    if !compiler_options
        .rewrite_relative_import_extensions
        .is_true()
    {
        return false;
    }

    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
        if ast::is_import_declaration(source, node)
            && let Some(module_specifier) = source.module_specifier(node)
            && ast::is_string_literal(source, module_specifier)
            && crate::moduletransforms::utilities::rewrite_module_specifier_text(
                &source.text(module_specifier),
                compiler_options,
            )
            .is_some()
        {
            return true;
        }
        if source.kind(node) == ast::Kind::CallExpression
            && source
                .arguments(node)
                .is_some_and(|arguments| !arguments.is_empty())
            && (ast::is_import_call(source, node)
                || ast::is_in_js_file(source, node) && ast::is_require_call(source, node, false))
        {
            return true;
        }

        let _ = source.for_each_present_child(node, |child| {
            stack.push(child);
            std::ops::ControlFlow::Continue(())
        });
    }

    false
}

fn visit_es_module_rewrite_statement(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    statement: ast::Node,
    compiler_options: &core::CompilerOptions,
) -> ast::Node {
    let mut visitor = EsModuleRewriteVisitor {
        source,
        emit_context,
        import_state: ast::AstImportState::new(),
        compiler_options,
    };
    visitor
        .visit_node(Some(statement))
        .unwrap_or_else(|| visitor.preserve_node(statement))
}

struct EsModuleRewriteVisitor<'a, 'source> {
    source: &'source ast::AstStore,
    emit_context: &'a mut printer::EmitContext,
    import_state: ast::AstImportState,
    compiler_options: &'a core::CompilerOptions,
}

impl EsModuleRewriteVisitor<'_, '_> {
    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        ast::AstImportState::store_for(self.source, self.factory(), node)
    }

    fn preserve_source_node(&mut self, node: ast::Node) -> ast::Node {
        let mut import_state = std::mem::take(&mut self.import_state);
        let imported = import_state.preserve_node(
            self.source,
            &mut self.emit_context.factory.node_factory,
            node,
        );
        self.import_state = import_state;
        copy_originals_for_preserved_subtree_if_unset(self.emit_context, node, imported);
        imported
    }

    fn append_visited_node(
        &mut self,
        original: ast::Node,
        visited: Option<ast::Node>,
        out: &mut Vec<ast::Node>,
        changed: &mut bool,
    ) {
        match visited {
            Some(visited) if self.preserved_source_node_matches(Some(original), Some(visited)) => {
                out.push(self.preserve_source_node(original));
            }
            Some(visited) => {
                *changed = true;
                let store = self.store_for(visited);
                if store.kind(visited) == ast::Kind::SyntaxList {
                    let nodes = store
                        .syntax_list_children(visited)
                        .expect("SyntaxList should have children")
                        .iter()
                        .flatten()
                        .collect::<Vec<_>>();
                    for node in nodes {
                        out.push(self.preserve_node(node));
                    }
                } else {
                    out.push(self.preserve_node(visited));
                }
            }
            None => *changed = true,
        }
    }

    fn rewrite_module_specifier(&mut self, module_specifier: ast::Node) -> ast::Node {
        let source = self.store_for(module_specifier);
        if ast::is_string_literal(source, module_specifier) {
            let text = source.text(module_specifier);
            if let Some(text) = crate::moduletransforms::utilities::rewrite_module_specifier_text(
                &text,
                self.compiler_options,
            ) {
                return self
                    .factory_mut()
                    .new_string_literal(text, ast::TokenFlags::NONE);
            }
        }
        self.preserve_node(module_specifier)
    }

    fn rewrite_import_or_require_argument(&mut self, argument: ast::Node) -> ast::Node {
        let source = self.store_for(argument);
        if ast::is_string_literal_like(source, argument) {
            return self.rewrite_module_specifier(argument);
        }
        let argument = self.preserve_node(argument);
        self.emit_context
            .factory
            .new_rewrite_relative_import_extensions_helper(
                argument,
                self.compiler_options.jsx == core::JsxEmit::Preserve,
            )
    }

    fn visit_import_declaration(&mut self, node: ast::Node) -> ast::Node {
        let source = self.store_for(node);
        let import_clause = source.import_clause(node);
        let module_specifier = source
            .module_specifier(node)
            .expect("import declaration should have a module specifier");
        let attributes = source.attributes(node);
        let import_clause = self.visit_node(import_clause);
        let module_specifier = self.rewrite_module_specifier(module_specifier);
        let attributes = self.visit_node(attributes);
        if node.store_id() == self.source.store_id() {
            let source = self.source;
            self.factory_mut().update_import_declaration_from_store(
                source,
                node,
                None::<ast::ModifierList>,
                import_clause,
                module_specifier,
                attributes,
            )
        } else {
            self.factory_mut().update_import_declaration(
                node,
                None::<ast::ModifierList>,
                import_clause,
                module_specifier,
                attributes,
            )
        }
    }

    fn visit_import_or_require_call(&mut self, node: ast::Node) -> ast::Node {
        let source = self.store_for(node);
        let expression = source.expression(node);
        let question_dot_token = source.question_dot_token(node);
        let type_arguments = source
            .type_arguments(node)
            .map(ast::SourceNodeListInput::from_source);
        let arguments = source
            .arguments(node)
            .expect("call expression should have arguments");
        let arguments_loc = arguments.loc();
        let arguments_range = arguments.range();
        let arguments_has_trailing_comma = arguments.has_trailing_comma();
        let argument_nodes = arguments.iter().collect::<Vec<_>>();
        let flags = source.flags(node);

        let expression = self.visit_node(expression);
        let question_dot_token = question_dot_token.map(|token| self.preserve_node(token));
        let type_arguments = self.visit_nodes_input(type_arguments);
        let mut arguments = Vec::with_capacity(argument_nodes.len());
        let mut iter = argument_nodes.into_iter();
        if let Some(first_argument) = iter.next() {
            arguments.push(self.rewrite_import_or_require_argument(first_argument));
        }
        for argument in iter {
            if let Some(argument) = self.visit_node(Some(argument)) {
                arguments.push(argument);
            }
        }
        let arguments = self.factory_mut().new_node_list_with_trailing_comma(
            arguments_loc,
            arguments_range,
            arguments,
            arguments_has_trailing_comma,
        );
        if node.store_id() == self.source.store_id() {
            let source = self.source;
            self.factory_mut().update_call_expression_from_store(
                source,
                node,
                expression,
                question_dot_token,
                type_arguments,
                arguments,
                flags,
            )
        } else {
            self.factory_mut().update_call_expression(
                node,
                expression,
                question_dot_token,
                type_arguments,
                arguments,
                flags,
            )
        }
    }
}

impl<'source> ast::AstVisitEachChildRuntime<'source> for EsModuleRewriteVisitor<'_, 'source> {
    fn source_store(&self) -> &ast::AstStore {
        self.source
    }

    fn factory(&self) -> &ast::NodeFactory {
        &self.emit_context.factory.node_factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.emit_context.factory.node_factory
    }

    fn preserved_node(&self, source: ast::Node) -> Option<ast::Node> {
        self.import_state.preserved_node(self.factory(), source)
    }

    fn preserve_node(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
            node
        } else {
            self.preserve_source_node(node)
        }
    }

    fn record_preserved_node(&mut self, source: ast::Node, imported: ast::Node) -> ast::Node {
        let imported = self.preserve_node(imported);
        let mut import_state = std::mem::take(&mut self.import_state);
        let recorded = import_state.record_preserved_node(
            source.store_id(),
            &mut self.emit_context.factory.node_factory,
            source,
            imported,
        );
        self.import_state = import_state;
        recorded
    }

    fn preserved_source_node_matches(
        &self,
        source: Option<ast::Node>,
        output: Option<ast::Node>,
    ) -> bool {
        self.import_state
            .preserved_source_node_matches(self.factory(), source, output)
    }

    fn update_source_file_from_visited(
        &mut self,
        node: ast::Node,
        statements: Option<ast::NodeList>,
        end_of_file_token: Option<ast::Node>,
        source_unchanged: bool,
    ) -> ast::Node {
        if source_unchanged {
            let imported = self.preserve_source_node(node);
            return self.record_preserved_node(node, imported);
        }
        let mut import_state = std::mem::take(&mut self.import_state);
        let updated = import_state.update_source_file_from_store(
            self.source,
            &mut self.emit_context.factory.node_factory,
            node,
            statements,
            end_of_file_token,
        );
        self.import_state = import_state;
        updated
    }

    fn visit_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let node = node?;
        let source = self.store_for(node);
        if source.kind(node) == ast::Kind::ImportDeclaration
            && self
                .compiler_options
                .rewrite_relative_import_extensions
                .is_true()
        {
            return Some(self.visit_import_declaration(node));
        }
        if source.kind(node) == ast::Kind::CallExpression
            && self
                .compiler_options
                .rewrite_relative_import_extensions
                .is_true()
            && source
                .arguments(node)
                .is_some_and(|arguments| !arguments.is_empty())
            && (ast::is_import_call(source, node)
                || ast::is_in_js_file(source, node) && ast::is_require_call(source, node, false))
        {
            return Some(self.visit_import_or_require_call(node));
        }
        Some(self.generated_visit_each_child(&node))
    }

    fn visit_token(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        self.visit_node(node)
    }

    fn visit_nodes_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        let nodes = nodes?;
        let source_list = nodes.clone();
        let mut visited = Vec::with_capacity(source_list.len());
        let mut changed = false;
        for node in source_list.iter() {
            let result = self.visit_node(Some(node));
            self.append_visited_node(node, result, &mut visited, &mut changed);
        }
        if changed {
            Some(self.factory_mut().new_node_list_with_trailing_comma(
                source_list.loc(),
                source_list.range(),
                visited,
                source_list.has_trailing_comma(),
            ))
        } else {
            let mut import_state = std::mem::take(&mut self.import_state);
            let list = import_state.preserve_source_node_list_input(
                self.source,
                &mut self.emit_context.factory.node_factory,
                &nodes,
            );
            self.import_state = import_state;
            Some(list)
        }
    }

    fn visit_modifiers_input(
        &mut self,
        modifiers: Option<ast::SourceModifierListInput>,
    ) -> Option<ast::ModifierList> {
        let modifiers = modifiers?;
        let mut import_state = std::mem::take(&mut self.import_state);
        let list = import_state.preserve_source_modifier_list_input(
            self.source,
            &mut self.emit_context.factory.node_factory,
            &modifiers,
        );
        self.import_state = import_state;
        Some(list)
    }

    fn visit_parameters_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        self.visit_nodes_input(nodes)
    }

    fn visit_function_body(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        self.visit_node(node)
    }

    fn visit_iteration_body(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        self.visit_embedded_statement(node)
    }

    fn visit_top_level_statements_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        self.visit_nodes_input(nodes)
    }

    fn visit_embedded_statement(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let node = node?;
        let visited = self.visit_node(Some(node));
        let mut import_state = std::mem::take(&mut self.import_state);
        let lifted = import_state.lift_to_block(
            self.source,
            &mut self.emit_context.factory.node_factory,
            visited,
        );
        self.import_state = import_state;
        lifted
    }

    fn visit_raw_node_slice_input(
        &mut self,
        nodes: Option<ast::SourceRawNodeSliceInput>,
    ) -> Option<ast::RawNodeSlice> {
        let nodes = nodes?;
        let mut import_state = std::mem::take(&mut self.import_state);
        let list = import_state.preserve_source_raw_node_slice_input(
            self.source,
            &mut self.emit_context.factory.node_factory,
            &nodes,
        );
        self.import_state = import_state;
        Some(list)
    }
}

impl<'source> ast::AstGeneratedVisitEachChild<'source> for EsModuleRewriteVisitor<'_, 'source> {}

fn create_es_module_require_call(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    node: &ast::Node,
    compiler_options: &core::CompilerOptions,
    import_require_statements: &mut Option<ImportRequireStatements>,
) -> ast::Node {
    let mut args = Vec::new();
    if let Some(module_name) =
        ast::get_external_module_import_equals_declaration_expression(source, *node)
    {
        let text = source.text(module_name);
        let text = crate::moduletransforms::utilities::rewrite_module_specifier_text(
            &text,
            compiler_options,
        )
        .unwrap_or(text);
        args.push(
            emit_context
                .factory
                .node_factory
                .new_string_literal(text, ast::TokenFlags::NONE),
        );
    }

    if esmodule::create_require_uses_plain_require(compiler_options.get_emit_module_kind()) {
        let factory = &mut emit_context.factory.node_factory;
        let require = factory.new_identifier("require");
        let arguments = factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            args,
        );
        return factory.new_call_expression(require, None, None, arguments, ast::NodeFlags::NONE);
    }

    if import_require_statements.is_none() {
        let create_require_name = emit_context.factory.new_unique_name_ex(
            "_createRequire",
            printer::AutoGenerateOptions {
                flags: printer::GeneratedIdentifierFlags::OPTIMISTIC
                    | printer::GeneratedIdentifierFlags::FILE_LEVEL,
                ..Default::default()
            },
        );
        let require_helper_name = emit_context.factory.new_unique_name_ex(
            "__require",
            printer::AutoGenerateOptions {
                flags: printer::GeneratedIdentifierFlags::OPTIMISTIC
                    | printer::GeneratedIdentifierFlags::FILE_LEVEL,
                ..Default::default()
            },
        );

        let import_statement = {
            let factory = &mut emit_context.factory.node_factory;
            let property_name = factory.new_identifier("createRequire");
            let import_specifier =
                factory.new_import_specifier(false, property_name, create_require_name);
            let import_specifiers = factory.new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                vec![import_specifier],
            );
            let named_imports = factory.new_named_imports(import_specifiers);
            let import_clause =
                factory.new_import_clause(None::<ast::Kind>, None::<ast::Node>, named_imports);
            let module_specifier = factory.new_string_literal("module", ast::TokenFlags::NONE);
            factory.new_import_declaration(None, import_clause, module_specifier, None::<ast::Node>)
        };
        emit_context.mark_emit_node(&import_statement, printer::EF_CUSTOM_PROLOGUE);

        let require_statement = {
            let factory = &mut emit_context.factory.node_factory;
            let meta = factory.new_identifier("meta");
            let import_meta = factory.new_meta_property(ast::Kind::ImportKeyword, meta);
            let url = factory.new_identifier("url");
            let import_meta_url = factory.new_property_access_expression(
                import_meta,
                None,
                url,
                ast::NodeFlags::NONE,
            );
            let arguments = factory.new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                vec![import_meta_url],
            );
            let initializer = factory.new_call_expression(
                create_require_name,
                None,
                None,
                arguments,
                ast::NodeFlags::NONE,
            );
            let declaration =
                factory.new_variable_declaration(require_helper_name, None, None, initializer);
            let declarations = factory.new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                vec![declaration],
            );
            let declaration_list =
                factory.new_variable_declaration_list(declarations, ast::NodeFlags::CONST);
            factory.new_variable_statement(None, declaration_list)
        };
        emit_context.mark_emit_node(&require_statement, printer::EF_CUSTOM_PROLOGUE);

        *import_require_statements = Some(ImportRequireStatements {
            statements: vec![import_statement, require_statement],
            require_helper_name,
        });
    }

    let require_helper_name = import_require_statements
        .as_ref()
        .expect("import require statements should be initialized")
        .require_helper_name;
    let factory = &mut emit_context.factory.node_factory;
    let arguments = factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        args,
    );
    factory.new_call_expression(
        require_helper_name,
        None,
        None,
        arguments,
        ast::NodeFlags::NONE,
    )
}

fn is_effective_external_module_root(
    root: ast::Node,
    emit_context: &printer::EmitContext,
    compiler_options: &core::CompilerOptions,
) -> bool {
    emit_context.with_source_file_view(root, |source_file| {
        ast::is_effective_external_module(&source_file, compiler_options)
    })
}

fn is_file_level_unique_name_root(
    root: ast::Node,
    emit_context: &printer::EmitContext,
    name: &str,
) -> bool {
    emit_context.with_source_file_view(root, |source_file| {
        !source_file.identifiers().contains_key(name)
    })
}

fn create_external_helpers_import_declaration_if_needed(
    file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
    compiler_options: &core::CompilerOptions,
    file_module_format: core::ModuleKind,
) -> Option<ast::Node> {
    if !compiler_options.import_helpers.is_true()
        || !is_effective_external_module_root(root, emit_context, compiler_options)
    {
        return None;
    }

    let module_kind = compiler_options.get_emit_module_kind();
    if file_module_format == core::ModuleKind::CommonJS
        || file_module_format == core::ModuleKind::None && module_kind == core::ModuleKind::CommonJS
    {
        // When we emit to a non-ES module, generate a synthetic `import tslib = require("tslib")` to be further transformed.
        return create_common_js_external_helpers_import_declaration_if_needed(
            file,
            root,
            emit_context,
            compiler_options,
            file_module_format,
        );
    }

    let mut helper_names = emit_context
        .get_emit_helpers(&root)
        .into_iter()
        .filter_map(|helper| {
            let helper = printer::helper_from_key(helper);
            (!helper.scoped && !helper.import_name.is_empty()).then_some(helper.import_name)
        })
        .collect::<Vec<_>>();
    helper_names.sort_unstable();
    helper_names.dedup();
    if helper_names.is_empty() {
        return None;
    }

    let import_specifiers = helper_names
        .into_iter()
        .map(|name| {
            if is_file_level_unique_name_root(root, emit_context, name) {
                let name = emit_context.factory.node_factory.new_identifier(name);
                emit_context.factory.node_factory.new_import_specifier(
                    false,
                    None::<ast::Node>,
                    Some(name),
                )
            } else {
                let property_name = emit_context.factory.node_factory.new_identifier(name);
                let name = emit_context.factory.new_unscoped_helper_name(name);
                emit_context.factory.node_factory.new_import_specifier(
                    false,
                    Some(property_name),
                    Some(name),
                )
            }
        })
        .collect::<Vec<_>>();
    let elements = emit_context.factory.node_factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        import_specifiers,
    );
    let named_bindings = emit_context
        .factory
        .node_factory
        .new_named_imports(elements);
    let import_clause = emit_context.factory.node_factory.new_import_clause(
        None::<ast::Kind>,
        None::<ast::Node>,
        Some(named_bindings),
    );
    emit_context.set_external_helpers(file);
    let module_specifier = emit_context.factory.node_factory.new_string_literal(
        externalmoduleinfo::EXTERNAL_HELPERS_MODULE_NAME_TEXT,
        ast::TokenFlags::NONE,
    );
    let external_helpers_import_declaration =
        emit_context.factory.node_factory.new_import_declaration(
            None::<ast::ModifierList>,
            Some(import_clause),
            Some(module_specifier),
            None::<ast::Node>,
        );
    emit_context.mark_emit_node(
        &external_helpers_import_declaration,
        printer::EF_CUSTOM_PROLOGUE,
    );
    Some(external_helpers_import_declaration)
}

pub(crate) fn visit_common_js_module_source_file_root_output(
    file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
    compiler_options: &core::CompilerOptions,
    file_module_format: core::ModuleKind,
    facts: ModuleTransformFacts,
) -> Option<ast::Node> {
    let factory_store_id = emit_context.factory.node_factory.store().store_id();
    if root.store_id() == factory_store_id {
        return visit_common_js_module_active_source_file_root_output(
            file,
            root,
            emit_context,
            compiler_options,
            file_module_format,
            facts,
        );
    }
    assert_eq!(
        root.store_id(),
        file.store().store_id(),
        "CommonJS module transform root must come from the input source or active emit factory"
    );
    let source = file.store();
    let source_file = source.as_source_file(root);
    let subtree_contains_dynamic_import = source
        .subtree_facts(root)
        .contains(ast::SubtreeFacts::CONTAINS_DYNAMIC_IMPORT);
    let common_js_facts = commonjsmodule::CommonJsFacts {
        is_declaration_file: source_file.is_declaration_file(),
        is_effective_external_module: ast::is_effective_external_module(file, compiler_options),
        subtree_contains_dynamic_import,
        ..Default::default()
    };
    if commonjsmodule::common_js_action_for_kind(ast::Kind::SourceFile, common_js_facts)
        == commonjsmodule::CommonJsAction::SkipSourceFile
    {
        return None;
    }

    Some(transform_common_js_module_output(
        file,
        root,
        emit_context,
        compiler_options,
        file_module_format,
        subtree_contains_dynamic_import,
        facts,
    ))
}

#[derive(Clone, Copy)]
struct ActiveCommonJsStatement {
    active: ast::Node,
    parsed: Option<ast::Node>,
    is_prologue: bool,
    is_custom_prologue: bool,
}

pub(crate) fn visit_common_js_module_active_source_file_root_output(
    file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
    compiler_options: &core::CompilerOptions,
    file_module_format: core::ModuleKind,
    facts: ModuleTransformFacts,
) -> Option<ast::Node> {
    let source = file.store();
    let (
        is_declaration_file,
        is_effective_external_module,
        subtree_contains_dynamic_import,
        source_file_name,
        source_file_end_of_file_token,
        source_statements_loc,
        source_statements_range,
        active_statement_data,
    ) = {
        let active_source = emit_context.factory.node_factory.store();
        let source_file = active_source.source_file_view(root);
        let source_statements = active_source
            .source_statements(root)
            .expect("source file should have statements");
        let active_statement_data = source_statements
            .iter()
            .map(|active| (active, ast::is_prologue_directive(active_source, active)))
            .collect::<Vec<_>>();
        (
            source_file.is_declaration_file(),
            ast::is_effective_external_module(&source_file, compiler_options),
            active_source
                .subtree_facts(root)
                .contains(ast::SubtreeFacts::CONTAINS_DYNAMIC_IMPORT),
            source_file.file_name_ref().to_owned(),
            source_file.end_of_file_token(),
            source_statements.loc(),
            source_statements.range(),
            active_statement_data,
        )
    };
    let statements = active_statement_data
        .into_iter()
        .map(|(active, is_prologue)| {
            let parsed = emit_context
                .parse_node(&active)
                .filter(|node| node.store_id() == source.store_id());
            let is_custom_prologue =
                emit_context.emit_flags(&active) & printer::EF_CUSTOM_PROLOGUE != 0;
            ActiveCommonJsStatement {
                active,
                parsed,
                is_prologue,
                is_custom_prologue,
            }
        })
        .collect::<Vec<_>>();
    let common_js_facts = commonjsmodule::CommonJsFacts {
        is_declaration_file,
        is_effective_external_module,
        subtree_contains_dynamic_import,
        ..Default::default()
    };
    if commonjsmodule::common_js_action_for_kind(ast::Kind::SourceFile, common_js_facts)
        == commonjsmodule::CommonJsAction::SkipSourceFile
    {
        return None;
    }

    Some(transform_common_js_module_active_output(
        file,
        root,
        emit_context,
        compiler_options,
        file_module_format,
        subtree_contains_dynamic_import,
        facts,
        &source_file_name,
        source_file_end_of_file_token,
        source_statements_loc,
        source_statements_range,
        &statements,
    ))
}

fn transform_common_js_module_active_output(
    file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
    compiler_options: &core::CompilerOptions,
    file_module_format: core::ModuleKind,
    source_file_contains_dynamic_import: bool,
    facts: ModuleTransformFacts,
    source_file_name: &str,
    source_file_end_of_file_token: Option<ast::Node>,
    source_statements_loc: core::TextRange,
    source_statements_range: core::TextRange,
    source_statements: &[ActiveCommonJsStatement],
) -> ast::Node {
    let source = file.store();
    let read_root = file.root();
    emit_context.start_variable_environment();

    let mut statements = Vec::new();
    let mut rest_start = 0;
    let read_statement_nodes: Vec<_> = source_statements
        .iter()
        .filter_map(|statement| statement.parsed)
        .collect();
    let mut import_references = collect_import_references(source, &read_statement_nodes);
    {
        let active_source = emit_context.factory.node_factory.store();
        let active_import_statements = source_statements
            .iter()
            .filter(|statement| statement.parsed.is_none())
            .map(|statement| statement.active)
            .filter(|statement| ast::is_import_declaration(active_source, *statement))
            .collect::<Vec<_>>();
        add_import_references(
            active_source,
            &active_import_statements,
            &mut import_references,
        );
    }
    while rest_start < source_statements.len() && source_statements[rest_start].is_prologue {
        statements.push(source_statements[rest_start].active);
        rest_start += 1;
    }

    let mut exported_names =
        collect_active_exported_names(source, emit_context, source_statements, &facts);
    {
        let mut seen = exported_names.iter().cloned().collect::<HashSet<_>>();
        for name in facts.exported_names.iter().cloned() {
            push_exported_name(&mut exported_names, &mut seen, name);
        }
    }
    let direct_exported_names = collect_direct_exported_names(source, read_root);
    let mut local_export_specifiers = collect_local_export_specifiers(source, read_root, &facts);
    merge_local_export_specifiers(
        &mut local_export_specifiers,
        collect_active_local_export_specifiers(emit_context, source_statements, &facts),
    );
    let export_equals = find_active_export_equals(emit_context, source_statements);
    let mut handled_exported_names = HashSet::new();

    while rest_start < source_statements.len() && source_statements[rest_start].is_custom_prologue {
        if let Some(statement) = source_statements[rest_start].parsed {
            let mut visitor = TopLevelNestedCommonJsVisitor {
                source,
                emit_context,
                import_references: &import_references,
                compiler_options,
                file_name: source_file_name,
                module_format: file_module_format,
                source_file_contains_dynamic_import,
                local_export_specifiers: &local_export_specifiers,
                direct_exported_names: &direct_exported_names,
                module_transform_facts: &facts,
                handled_exported_names: &mut handled_exported_names,
            };
            statements.extend(visitor.visit_top_level_nested_statement(statement));
        } else {
            statements.push(source_statements[rest_start].active);
        }
        rest_start += 1;
    }

    if should_emit_underscore_underscore_es_module(
        root,
        emit_context,
        export_equals.is_some(),
        source,
        &facts,
    ) {
        let statement =
            create_underscore_underscore_es_module(&mut emit_context.factory.node_factory);
        emit_context.set_emit_flags(&statement, printer::EF_CUSTOM_PROLOGUE);
        statements.push(statement);
    }

    append_exported_names_preload(&mut statements, emit_context, &exported_names);
    if export_equals.is_none() {
        for (exported_name, local_name) in
            collect_active_exported_function_names(source, emit_context, source_statements)
        {
            let factory = &mut emit_context.factory.node_factory;
            let left = exports_property_access(factory, &exported_name);
            let equals = factory.new_token(ast::Kind::EqualsToken);
            let assignment = factory.new_binary_expression(None, left, None, equals, local_name);
            let statement = factory.new_expression_statement(assignment);
            emit_context.mark_emit_node(&statement, printer::EF_CUSTOM_PROLOGUE);
            statements.push(statement);
            handled_exported_names.insert(exported_name);
        }
    }

    for statement in &source_statements[rest_start..] {
        if active_statement_has_kind(
            emit_context,
            statement.active,
            ast::Kind::NotEmittedStatement,
        ) {
            statements.push(statement.active);
            continue;
        }
        if active_statement_has_kind(emit_context, statement.active, ast::Kind::SyntaxList) {
            if let Some(rewritten) = rewrite_active_common_js_statement_list(
                source,
                emit_context,
                &import_references,
                &direct_exported_names,
                &local_export_specifiers,
                &facts,
                compiler_options,
                source_file_name,
                file_module_format,
                source_file_contains_dynamic_import,
                statement.active,
            ) {
                statements.extend(rewritten);
                continue;
            }
        }
        let Some(read_statement) = statement.parsed else {
            let active_source = emit_context.store_for_node(statement.active);
            if ast::is_export_assignment(active_source, statement.active)
                && active_source
                    .is_export_equals(statement.active)
                    .unwrap_or(false)
            {
                continue;
            }
            if let Some(active_import) =
                active_import_declaration_to_require_input(emit_context, statement.active)
            {
                let generated_name_node = emit_context.most_original(&statement.active);
                statements.extend(transform_active_import_declaration_to_require(
                    emit_context,
                    statement.active,
                    generated_name_node,
                    active_import,
                ));
                continue;
            }
            if let Some(active_export) =
                active_export_declaration_to_common_js_input(emit_context, statement.active)
            {
                statements.extend(transform_active_export_declaration_to_common_js(
                    emit_context,
                    statement.active,
                    statement.active,
                    active_export,
                ));
                continue;
            }
            if let Some((expression, loc)) =
                active_export_assignment_input(emit_context, statement.active)
            {
                statements.push(transform_export_default_assignment_expression(
                    source,
                    emit_context,
                    expression,
                    loc,
                    &import_references,
                    compiler_options,
                    source_file_name,
                    file_module_format,
                    source_file_contains_dynamic_import,
                    &direct_exported_names,
                    &local_export_specifiers,
                    &facts,
                ));
                continue;
            }
            if let Some(statement) = transform_active_class_decorator_assignment_to_common_js(
                emit_context,
                statement.active,
                &direct_exported_names,
                &local_export_specifiers,
            ) {
                statements.push(statement);
                continue;
            }
            if let Some(rewritten) = rewrite_active_common_js_statement_list(
                source,
                emit_context,
                &import_references,
                &direct_exported_names,
                &local_export_specifiers,
                &facts,
                compiler_options,
                source_file_name,
                file_module_format,
                source_file_contains_dynamic_import,
                statement.active,
            ) {
                statements.extend(rewritten);
            } else {
                statements.push(statement.active);
            }
            continue;
        };
        if statement.active.store_id() == emit_context.factory.node_factory.store().store_id()
            && !active_statement_has_kind(
                emit_context,
                statement.active,
                source.kind(read_statement),
            )
        {
            if let Some(active_statements) = rewrite_active_common_js_statement_list(
                source,
                emit_context,
                &import_references,
                &direct_exported_names,
                &local_export_specifiers,
                &facts,
                compiler_options,
                source_file_name,
                file_module_format,
                source_file_contains_dynamic_import,
                statement.active,
            ) {
                statements.extend(active_statements);
                continue;
            }
        }
        if ast::is_variable_statement(source, read_statement)
            && ast::has_syntactic_modifier(source, read_statement, ast::ModifierFlags::EXPORT)
        {
            let transformed =
                if active_statement_has_kind(
                    emit_context,
                    statement.active,
                    ast::Kind::VariableStatement,
                ) && !variable_statement_has_binding_pattern(source, read_statement)
                {
                    transform_exported_variable_statement_active(
                        source,
                        emit_context,
                        statement.active,
                        read_statement,
                        &import_references,
                        compiler_options,
                        source_file_name,
                        file_module_format,
                        source_file_contains_dynamic_import,
                        &direct_exported_names,
                        &local_export_specifiers,
                        &facts,
                    )
                } else {
                    transform_exported_variable_statement(
                        source,
                        emit_context,
                        &read_statement,
                        Some(statement.active),
                        &import_references,
                        compiler_options,
                        source_file_name,
                        file_module_format,
                        source_file_contains_dynamic_import,
                        &direct_exported_names,
                        &local_export_specifiers,
                        &facts,
                    )
                };
            if let Some(first) = transformed.first() {
                emit_context.assign_comment_and_source_map_ranges(first, &read_statement);
            }
            statements.extend(transformed);
        } else if ast::is_variable_statement(source, read_statement) {
            if active_statement_has_kind(
                emit_context,
                statement.active,
                ast::Kind::VariableStatement,
            ) {
                statements.push(rewrite_common_js_statement(
                    source,
                    emit_context,
                    &import_references,
                    &direct_exported_names,
                    &local_export_specifiers,
                    &facts,
                    compiler_options,
                    source_file_name,
                    file_module_format,
                    source_file_contains_dynamic_import,
                    statement.active,
                ));
                let mut visitor = TopLevelNestedCommonJsVisitor {
                    source,
                    emit_context,
                    import_references: &import_references,
                    compiler_options,
                    file_name: source_file_name,
                    module_format: file_module_format,
                    source_file_contains_dynamic_import,
                    local_export_specifiers: &local_export_specifiers,
                    direct_exported_names: &direct_exported_names,
                    module_transform_facts: &facts,
                    handled_exported_names: &mut handled_exported_names,
                };
                visitor.append_exports_of_variable_statement(&mut statements, read_statement);
            } else {
                let mut visitor = TopLevelNestedCommonJsVisitor {
                    source,
                    emit_context,
                    import_references: &import_references,
                    compiler_options,
                    file_name: source_file_name,
                    module_format: file_module_format,
                    source_file_contains_dynamic_import,
                    local_export_specifiers: &local_export_specifiers,
                    direct_exported_names: &direct_exported_names,
                    module_transform_facts: &facts,
                    handled_exported_names: &mut handled_exported_names,
                };
                statements.extend(visitor.visit_top_level_nested_statement(read_statement));
            }
        } else if ast::is_class_declaration(source, read_statement)
            && ast::has_syntactic_modifier(source, read_statement, ast::ModifierFlags::EXPORT)
        {
            if !statement_needs_common_js_reference_rewrite(
                source,
                read_statement,
                emit_context,
                compiler_options,
                source_file_name,
                file_module_format,
                source_file_contains_dynamic_import,
                &facts,
            ) && active_statement_has_kind(
                emit_context,
                statement.active,
                ast::Kind::ClassDeclaration,
            ) {
                statements.push(strip_export_from_active_class_declaration(
                    file,
                    emit_context,
                    compiler_options,
                    statement.active,
                ));
            } else {
                statements.push(transform_exported_class_declaration_to_common_js(
                    file,
                    source,
                    emit_context,
                    &read_statement,
                    Some(statement.active),
                    &import_references,
                    compiler_options,
                    source_file_name,
                    file_module_format,
                    source_file_contains_dynamic_import,
                    &direct_exported_names,
                    &local_export_specifiers,
                    &facts,
                ));
            }
            if export_equals.is_none()
                && let Some(export_statement) =
                    create_export_statement_for_active_or_read_declaration(
                        source,
                        emit_context,
                        statement.active,
                        read_statement,
                    )
            {
                statements.push(export_statement);
            }
            if export_equals.is_none() {
                append_exports_of_active_or_read_declaration(
                    source,
                    emit_context,
                    statement.active,
                    read_statement,
                    &local_export_specifiers,
                    &mut statements,
                );
            }
        } else if ast::is_function_declaration(source, read_statement)
            && ast::has_syntactic_modifier(source, read_statement, ast::ModifierFlags::EXPORT)
        {
            if !statement_needs_common_js_reference_rewrite(
                source,
                read_statement,
                emit_context,
                compiler_options,
                source_file_name,
                file_module_format,
                source_file_contains_dynamic_import,
                &facts,
            ) && active_statement_has_kind(
                emit_context,
                statement.active,
                ast::Kind::FunctionDeclaration,
            ) {
                statements.push(strip_export_from_active_function_declaration(
                    emit_context,
                    statement.active,
                ));
            } else if active_statement_has_kind(
                emit_context,
                statement.active,
                ast::Kind::FunctionDeclaration,
            ) {
                let stripped =
                    strip_export_from_active_function_declaration(emit_context, statement.active);
                statements.push(rewrite_common_js_statement(
                    source,
                    emit_context,
                    &import_references,
                    &direct_exported_names,
                    &local_export_specifiers,
                    &facts,
                    compiler_options,
                    source_file_name,
                    file_module_format,
                    source_file_contains_dynamic_import,
                    stripped,
                ));
            } else {
                statements.push(transform_exported_function_declaration_to_common_js(
                    source,
                    emit_context,
                    &read_statement,
                    &import_references,
                    compiler_options,
                    source_file_name,
                    file_module_format,
                    source_file_contains_dynamic_import,
                    &direct_exported_names,
                    &local_export_specifiers,
                    &facts,
                ));
            }
        } else if ast::is_export_assignment(source, read_statement) {
            if !source.is_export_equals(read_statement).unwrap_or(false) {
                let active_export = active_export_assignment_input(emit_context, statement.active);
                if let Some((expression, _)) = active_export
                    && let Some(read_expression) = source.expression(read_statement)
                {
                    copy_originals_for_matching_subtree_if_unset(
                        emit_context,
                        read_expression,
                        expression,
                    );
                }
                let (expression, loc) = active_export.unwrap_or_else(|| {
                    (
                        source
                            .expression(read_statement)
                            .expect("export assignment should have expression"),
                        source.loc(read_statement),
                    )
                });
                statements.push(transform_export_default_assignment_expression(
                    source,
                    emit_context,
                    expression,
                    loc,
                    &import_references,
                    compiler_options,
                    source_file_name,
                    file_module_format,
                    source_file_contains_dynamic_import,
                    &direct_exported_names,
                    &local_export_specifiers,
                    &facts,
                ));
            }
        } else if ast::is_export_declaration(source, read_statement) {
            if active_statement_has_kind(
                emit_context,
                statement.active,
                ast::Kind::ExportDeclaration,
            ) && let Some(active_export) =
                active_export_declaration_to_common_js_input(emit_context, statement.active)
            {
                statements.extend(transform_active_export_declaration_to_common_js(
                    emit_context,
                    statement.active,
                    read_statement,
                    active_export,
                ));
            } else {
                statements.extend(transform_export_declaration_to_common_js(
                    source,
                    emit_context,
                    &read_statement,
                    compiler_options,
                ));
            }
        } else if ast::is_import_equals_declaration(source, read_statement) {
            if ast::is_external_module_import_equals_declaration(source, read_statement) {
                let mut importer =
                    ast::AstImporter::new(source, &mut emit_context.factory.node_factory);
                let transformed = transform_import_equals_declaration_to_require(
                    source,
                    &mut importer,
                    &read_statement,
                    compiler_options,
                );
                emit_context.set_original(&transformed, &read_statement);
                emit_context.assign_comment_and_source_map_ranges(&transformed, &read_statement);
                statements.push(transformed);
                if export_equals.is_none() {
                    append_exports_of_import_equals_declaration(
                        source,
                        emit_context,
                        read_statement,
                        &local_export_specifiers,
                        &mut statements,
                    );
                }
            } else {
                let active = if active_statement_has_kind(
                    emit_context,
                    statement.active,
                    ast::Kind::ImportEqualsDeclaration,
                ) {
                    panic!(
                        "import= for internal module references should be handled in an earlier transformer."
                    )
                } else {
                    statement.active
                };
                let active = rewrite_common_js_statement(
                    source,
                    emit_context,
                    &import_references,
                    &direct_exported_names,
                    &local_export_specifiers,
                    &facts,
                    compiler_options,
                    source_file_name,
                    file_module_format,
                    source_file_contains_dynamic_import,
                    active,
                );
                statements.push(active);
                if export_equals.is_none() {
                    append_exports_of_import_equals_declaration(
                        source,
                        emit_context,
                        read_statement,
                        &local_export_specifiers,
                        &mut statements,
                    );
                }
            }
        } else if ast::is_import_declaration(source, read_statement) {
            if let Some(active_import) =
                active_import_declaration_to_require_input(emit_context, statement.active)
            {
                statements.extend(transform_active_import_declaration_to_require(
                    emit_context,
                    statement.active,
                    read_statement,
                    active_import,
                ));
            } else {
                let transformed = transform_import_declaration_to_require(
                    source,
                    emit_context,
                    &read_statement,
                    compiler_options,
                );
                statements.extend(transformed);
            }
            append_exports_of_import_declaration(
                source,
                emit_context,
                read_statement,
                &exported_names,
                &local_export_specifiers,
                &mut statements,
            );
        } else {
            if active_statement_has_kind(
                emit_context,
                statement.active,
                source.kind(read_statement),
            ) {
                if source.kind(read_statement) == ast::Kind::ClassDeclaration {
                    let mut visitor = TopLevelNestedCommonJsVisitor {
                        source,
                        emit_context,
                        import_references: &import_references,
                        compiler_options,
                        file_name: source_file_name,
                        module_format: file_module_format,
                        source_file_contains_dynamic_import,
                        local_export_specifiers: &local_export_specifiers,
                        direct_exported_names: &direct_exported_names,
                        module_transform_facts: &facts,
                        handled_exported_names: &mut handled_exported_names,
                    };
                    statements.extend(visitor.visit_top_level_nested_statement(statement.active));
                    continue;
                }
                if matches!(
                    source.kind(read_statement),
                    ast::Kind::Block
                        | ast::Kind::ClassDeclaration
                        | ast::Kind::DoStatement
                        | ast::Kind::ForInStatement
                        | ast::Kind::ForOfStatement
                        | ast::Kind::ForStatement
                        | ast::Kind::IfStatement
                        | ast::Kind::LabeledStatement
                        | ast::Kind::SwitchStatement
                        | ast::Kind::TryStatement
                        | ast::Kind::WhileStatement
                        | ast::Kind::WithStatement
                ) {
                    let mut visitor = TopLevelNestedCommonJsVisitor {
                        source,
                        emit_context,
                        import_references: &import_references,
                        compiler_options,
                        file_name: source_file_name,
                        module_format: file_module_format,
                        source_file_contains_dynamic_import,
                        local_export_specifiers: &local_export_specifiers,
                        direct_exported_names: &direct_exported_names,
                        module_transform_facts: &facts,
                        handled_exported_names: &mut handled_exported_names,
                    };
                    statements.extend(visitor.visit_top_level_nested_statement(statement.active));
                    continue;
                }
                statements.push(rewrite_common_js_statement(
                    source,
                    emit_context,
                    &import_references,
                    &direct_exported_names,
                    &local_export_specifiers,
                    &facts,
                    compiler_options,
                    source_file_name,
                    file_module_format,
                    source_file_contains_dynamic_import,
                    statement.active,
                ));
                continue;
            }

            if let Some(active_statements) = rewrite_active_common_js_statement_list(
                source,
                emit_context,
                &import_references,
                &direct_exported_names,
                &local_export_specifiers,
                &facts,
                compiler_options,
                source_file_name,
                file_module_format,
                source_file_contains_dynamic_import,
                statement.active,
            ) {
                statements.extend(active_statements);
                continue;
            }

            if statement.active.store_id() == emit_context.factory.node_factory.store().store_id() {
                statements.push(rewrite_common_js_statement(
                    source,
                    emit_context,
                    &import_references,
                    &direct_exported_names,
                    &local_export_specifiers,
                    &facts,
                    compiler_options,
                    source_file_name,
                    file_module_format,
                    source_file_contains_dynamic_import,
                    statement.active,
                ));
                continue;
            }

            let mut visitor = TopLevelNestedCommonJsVisitor {
                source,
                emit_context,
                import_references: &import_references,
                compiler_options,
                file_name: source_file_name,
                module_format: file_module_format,
                source_file_contains_dynamic_import,
                local_export_specifiers: &local_export_specifiers,
                direct_exported_names: &direct_exported_names,
                module_transform_facts: &facts,
                handled_exported_names: &mut handled_exported_names,
            };
            let transformed = visitor.visit_top_level_nested_statement(read_statement);
            if transformed.len() == 1 && transformed[0] == read_statement {
                statements.push(statement.active);
            } else {
                statements.extend(transformed);
            }
        }
    }

    if let Some(export_equals) = export_equals {
        let expression = rewrite_common_js_expression(
            source,
            emit_context,
            &import_references,
            &direct_exported_names,
            &local_export_specifiers,
            &facts,
            compiler_options,
            source_file_name,
            file_module_format,
            source_file_contains_dynamic_import,
            export_equals.expression,
        );
        let statement =
            create_module_exports_assignment(&mut emit_context.factory.node_factory, expression);
        emit_context.assign_comment_and_source_map_ranges(&statement, &export_equals.node);
        emit_context.mark_emit_node(&statement, printer::EF_NO_COMMENTS);
        statements.push(statement);
    }

    emit_context.add_requested_emit_helpers(&root);
    let mut statements = emit_context.end_and_merge_variable_environment(source, &statements);
    if let Some(external_helpers_import_declaration) =
        create_common_js_external_helpers_import_declaration_if_needed(
            file,
            root,
            emit_context,
            compiler_options,
            file_module_format,
        )
    {
        let prologue_end = {
            let store = emit_context.factory.node_factory.store();
            statements
                .iter()
                .position(|statement| !ast::is_prologue_directive(store, *statement))
                .unwrap_or(statements.len())
        };
        let custom_end = statements[prologue_end..]
            .iter()
            .position(|statement| {
                emit_context.emit_flags(statement) & printer::EF_CUSTOM_PROLOGUE == 0
            })
            .map(|index| prologue_end + index)
            .unwrap_or(statements.len());
        let prologue = &statements[..prologue_end];
        let custom = &statements[prologue_end..custom_end];
        let rest = &statements[custom_end..];
        let mut updated = Vec::with_capacity(statements.len() + 1);
        updated.extend_from_slice(prologue);
        updated.extend_from_slice(custom);
        updated.push(external_helpers_import_declaration);
        updated.extend_from_slice(rest);
        statements = updated;
    }

    let statements = flatten_syntax_list_statements(emit_context, statements);
    let statement_list = emit_context.factory.node_factory.new_node_list(
        source_statements_loc,
        source_statements_range,
        statements,
    );
    emit_context
        .factory
        .node_factory
        .update_source_file_in_current_store(root, statement_list, source_file_end_of_file_token)
}

fn transform_common_js_module_output(
    file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
    compiler_options: &core::CompilerOptions,
    file_module_format: core::ModuleKind,
    source_file_contains_dynamic_import: bool,
    facts: ModuleTransformFacts,
) -> ast::Node {
    let source = file.store();
    emit_context.start_variable_environment();
    let source_file = source.as_source_file(root);
    let source_statements = source.parser_access().source_file_statement_list(root);
    let source_file_name = source_file.file_name_ref().to_owned();
    let source_file_end_of_file_token = source_file.end_of_file_token();
    let source_statements_loc = source_statements.loc();
    let source_statements_range = source_statements.range();

    let mut statements = Vec::new();
    let mut rest_start = 0;
    let source_statement_nodes: Vec<_> = source_statements.iter().collect();
    let import_references = collect_import_references(source, &source_statement_nodes);
    while rest_start < source_statement_nodes.len()
        && ast::is_prologue_directive(source, source_statement_nodes[rest_start])
    {
        let mut importer = ast::AstImporter::new(source, &mut emit_context.factory.node_factory);
        statements.push(importer.preserve_node(source_statement_nodes[rest_start]));
        rest_start += 1;
    }

    let exported_names = facts.exported_names.clone();
    let direct_exported_names = collect_direct_exported_names(source, root);
    let local_export_specifiers = collect_local_export_specifiers(source, root, &facts);
    let export_equals = find_export_equals(source, emit_context, root);
    let mut handled_exported_names = HashSet::new();

    while rest_start < source_statement_nodes.len()
        && emit_context.emit_flags(&source_statement_nodes[rest_start])
            & printer::EF_CUSTOM_PROLOGUE
            != 0
    {
        let mut visitor = TopLevelNestedCommonJsVisitor {
            source,
            emit_context,
            import_references: &import_references,
            compiler_options,
            file_name: &source_file_name,
            module_format: file_module_format,
            source_file_contains_dynamic_import,
            local_export_specifiers: &local_export_specifiers,
            direct_exported_names: &direct_exported_names,
            module_transform_facts: &facts,
            handled_exported_names: &mut handled_exported_names,
        };
        statements
            .extend(visitor.visit_top_level_nested_statement(source_statement_nodes[rest_start]));
        rest_start += 1;
    }

    if should_emit_underscore_underscore_es_module(
        root,
        emit_context,
        export_equals.is_some(),
        source,
        &facts,
    ) {
        let statement =
            create_underscore_underscore_es_module(&mut emit_context.factory.node_factory);
        emit_context.set_emit_flags(&statement, printer::EF_CUSTOM_PROLOGUE);
        statements.push(statement);
    }

    append_exported_names_preload(&mut statements, emit_context, &exported_names);
    if export_equals.is_none() {
        for (exported_name, local_name) in
            collect_exported_function_names(source, emit_context, root)
        {
            let factory = &mut emit_context.factory.node_factory;
            let left = exports_property_access(factory, &exported_name);
            let equals = factory.new_token(ast::Kind::EqualsToken);
            let assignment = factory.new_binary_expression(None, left, None, equals, local_name);
            let statement = factory.new_expression_statement(assignment);
            emit_context.mark_emit_node(&statement, printer::EF_CUSTOM_PROLOGUE);
            statements.push(statement);
            handled_exported_names.insert(exported_name);
        }
    }

    for statement in &source_statement_nodes[rest_start..] {
        if ast::is_variable_statement(source, *statement)
            && ast::has_syntactic_modifier(source, *statement, ast::ModifierFlags::EXPORT)
        {
            let transformed = transform_exported_variable_statement(
                source,
                emit_context,
                statement,
                None,
                &import_references,
                compiler_options,
                &source_file_name,
                file_module_format,
                source_file_contains_dynamic_import,
                &direct_exported_names,
                &local_export_specifiers,
                &facts,
            );
            if let Some(first) = transformed.first() {
                emit_context.assign_comment_and_source_map_ranges(first, statement);
            }
            statements.extend(transformed);
        } else if ast::is_class_declaration(source, *statement)
            && ast::has_syntactic_modifier(source, *statement, ast::ModifierFlags::EXPORT)
        {
            statements.push(transform_exported_class_declaration_to_common_js(
                file,
                source,
                emit_context,
                statement,
                None,
                &import_references,
                compiler_options,
                &source_file_name,
                file_module_format,
                source_file_contains_dynamic_import,
                &direct_exported_names,
                &local_export_specifiers,
                &facts,
            ));
            if export_equals.is_none() {
                if let Some(export_statement) =
                    create_export_statement_for_declaration(&source, emit_context, statement)
                {
                    statements.push(export_statement);
                }
            }
        } else if ast::is_function_declaration(source, *statement)
            && ast::has_syntactic_modifier(source, *statement, ast::ModifierFlags::EXPORT)
        {
            statements.push(transform_exported_function_declaration_to_common_js(
                source,
                emit_context,
                statement,
                &import_references,
                compiler_options,
                &source_file_name,
                file_module_format,
                source_file_contains_dynamic_import,
                &direct_exported_names,
                &local_export_specifiers,
                &facts,
            ));
        } else if ast::is_export_assignment(source, *statement) {
            if !source.is_export_equals(*statement).unwrap_or(false) {
                statements.push(transform_export_default_assignment(
                    &source,
                    emit_context,
                    statement,
                    &import_references,
                    compiler_options,
                    &source_file_name,
                    file_module_format,
                    source_file_contains_dynamic_import,
                    &direct_exported_names,
                    &local_export_specifiers,
                    &facts,
                ));
            }
        } else if ast::is_export_declaration(source, *statement) {
            statements.extend(transform_export_declaration_to_common_js(
                source,
                emit_context,
                statement,
                compiler_options,
            ));
        } else if ast::is_import_equals_declaration(source, *statement) {
            let mut importer =
                ast::AstImporter::new(source, &mut emit_context.factory.node_factory);
            let transformed = transform_import_equals_declaration_to_require(
                source,
                &mut importer,
                statement,
                compiler_options,
            );
            emit_context.set_original(&transformed, statement);
            emit_context.assign_comment_and_source_map_ranges(&transformed, statement);
            statements.push(transformed);
            if export_equals.is_none() {
                append_exports_of_import_equals_declaration(
                    &source,
                    emit_context,
                    *statement,
                    &local_export_specifiers,
                    &mut statements,
                );
            }
        } else if ast::is_import_declaration(source, *statement) {
            let transformed = transform_import_declaration_to_require(
                &source,
                emit_context,
                statement,
                compiler_options,
            );
            statements.extend(transformed);
            append_exports_of_import_declaration(
                &source,
                emit_context,
                *statement,
                &exported_names,
                &local_export_specifiers,
                &mut statements,
            );
        } else {
            let mut visitor = TopLevelNestedCommonJsVisitor {
                source,
                emit_context,
                import_references: &import_references,
                compiler_options,
                file_name: &source_file_name,
                module_format: file_module_format,
                source_file_contains_dynamic_import,
                local_export_specifiers: &local_export_specifiers,
                direct_exported_names: &direct_exported_names,
                module_transform_facts: &facts,
                handled_exported_names: &mut handled_exported_names,
            };
            statements.extend(visitor.visit_top_level_nested_statement(*statement));
        }
    }

    if let Some(export_equals) = export_equals {
        let expression = rewrite_common_js_expression(
            source,
            emit_context,
            &import_references,
            &direct_exported_names,
            &local_export_specifiers,
            &facts,
            compiler_options,
            &source_file_name,
            file_module_format,
            source_file_contains_dynamic_import,
            export_equals.expression,
        );
        let statement =
            create_module_exports_assignment(&mut emit_context.factory.node_factory, expression);
        emit_context.assign_comment_and_source_map_ranges(&statement, &export_equals.node);
        emit_context.mark_emit_node(&statement, printer::EF_NO_COMMENTS);
        statements.push(statement);
    }

    emit_context.add_requested_emit_helpers(&root);
    let mut statements = emit_context.end_and_merge_variable_environment(source, &statements);
    if let Some(external_helpers_import_declaration) =
        create_common_js_external_helpers_import_declaration_if_needed(
            file,
            root,
            emit_context,
            compiler_options,
            file_module_format,
        )
    {
        let prologue_end = {
            let store = emit_context.factory.node_factory.store();
            statements
                .iter()
                .position(|statement| !ast::is_prologue_directive(store, *statement))
                .unwrap_or(statements.len())
        };
        let custom_end = statements[prologue_end..]
            .iter()
            .position(|statement| {
                emit_context.emit_flags(statement) & printer::EF_CUSTOM_PROLOGUE == 0
            })
            .map(|index| prologue_end + index)
            .unwrap_or(statements.len());
        let prologue = &statements[..prologue_end];
        let custom = &statements[prologue_end..custom_end];
        let rest = &statements[custom_end..];
        let mut updated = Vec::with_capacity(statements.len() + 1);
        updated.extend_from_slice(prologue);
        updated.extend_from_slice(custom);
        updated.push(external_helpers_import_declaration);
        updated.extend_from_slice(rest);
        statements = updated;
    }
    let statements = flatten_syntax_list_statements(emit_context, statements);
    if root.store_id() == emit_context.factory.node_factory.store().store_id() {
        let statement_list = emit_context.factory.node_factory.new_node_list(
            source_statements_loc,
            source_statements_range,
            statements,
        );
        emit_context
            .factory
            .node_factory
            .update_source_file_in_current_store(
                root,
                statement_list,
                source_file_end_of_file_token,
            )
    } else {
        let mut importer = ast::AstImporter::new(source, &mut emit_context.factory.node_factory);
        let statement_list = importer.factory().new_node_list(
            source_statements_loc,
            source_statements_range,
            statements,
        );
        let end_of_file_token = importer.preserve_optional_node(source_file_end_of_file_token);
        importer.update_source_file(root, Some(statement_list), end_of_file_token)
    }
}

fn create_common_js_external_helpers_import_declaration_if_needed(
    file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
    compiler_options: &core::CompilerOptions,
    file_module_kind: core::ModuleKind,
) -> Option<ast::Node> {
    if !compiler_options.import_helpers.is_true()
        || !is_effective_external_module_root(root, emit_context, compiler_options)
    {
        return None;
    }

    let module_kind = compiler_options.get_emit_module_kind();
    if file_module_kind != core::ModuleKind::CommonJS
        && !(file_module_kind == core::ModuleKind::None
            && module_kind == core::ModuleKind::CommonJS)
    {
        return None;
    }

    let has_imported_helpers = emit_context
        .get_emit_helpers(&root)
        .into_iter()
        .any(|helper| !printer::helper_from_key(helper).scoped);
    if !has_imported_helpers {
        return None;
    }

    let external_helpers_module_name = emit_context
        .get_external_helpers_module_name(file)
        .unwrap_or_else(|| {
            let name = emit_context
                .factory
                .new_unique_name(externalmoduleinfo::EXTERNAL_HELPERS_MODULE_NAME_TEXT);
            emit_context.set_external_helpers_module_name(file, &name);
            name
        });
    let require = emit_context.factory.node_factory.new_identifier("require");
    let module_specifier = emit_context.factory.node_factory.new_string_literal(
        externalmoduleinfo::EXTERNAL_HELPERS_MODULE_NAME_TEXT,
        ast::TokenFlags::NONE,
    );
    let arguments = emit_context.factory.node_factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        vec![module_specifier],
    );
    let require_call = emit_context.factory.node_factory.new_call_expression(
        require,
        None,
        None,
        arguments,
        ast::NodeFlags::NONE,
    );
    let declaration = emit_context.factory.node_factory.new_variable_declaration(
        external_helpers_module_name,
        None,
        None,
        require_call,
    );
    let declarations = emit_context.factory.node_factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        vec![declaration],
    );
    let declaration_list = emit_context
        .factory
        .node_factory
        .new_variable_declaration_list(declarations, ast::NodeFlags::CONST);
    let statement = emit_context
        .factory
        .node_factory
        .new_variable_statement(None, declaration_list);
    emit_context.mark_emit_node(&statement, printer::EF_CUSTOM_PROLOGUE);
    Some(statement)
}

fn collect_import_references(
    source: &ast::AstStore,
    statements: &[ast::Node],
) -> HashMap<String, ImportReference> {
    let mut references = HashMap::new();
    add_import_references(source, statements, &mut references);
    references
}

fn add_import_references(
    source: &ast::AstStore,
    statements: &[ast::Node],
    references: &mut HashMap<String, ImportReference>,
) {
    for statement in statements {
        if ast::is_external_module_import_equals_declaration(source, *statement)
            && ast::has_syntactic_modifier(source, *statement, ast::ModifierFlags::EXPORT)
        {
            let Some(name) = source.name(*statement) else {
                continue;
            };
            references.insert(
                source.text(name),
                ImportReference {
                    import_declaration: *statement,
                    property_name: None,
                },
            );
            continue;
        }
        if !ast::is_import_declaration(source, *statement) {
            continue;
        }
        let Some(import_clause) = source.import_clause(*statement) else {
            continue;
        };
        if let Some(default_name) = source.name(import_clause) {
            references.insert(
                source.text(default_name),
                ImportReference {
                    import_declaration: *statement,
                    property_name: None,
                },
            );
        }
        let Some(named_bindings) = source.named_bindings(import_clause) else {
            continue;
        };
        if source.kind(named_bindings) != ast::Kind::NamedImports {
            continue;
        }
        let Some(elements) = source.source_elements(named_bindings) else {
            continue;
        };
        for specifier in elements.iter() {
            let Some(local_name) = source.name(specifier) else {
                continue;
            };
            references.insert(
                source.text(local_name),
                ImportReference {
                    import_declaration: *statement,
                    property_name: source.property_name_or_name(specifier),
                },
            );
        }
    }
}

fn transform_import_declaration_to_require(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    statement: &ast::Node,
    compiler_options: &core::CompilerOptions,
) -> Vec<ast::Node> {
    if source.import_clause(*statement).is_none() {
        let require_call = create_import_declaration_require_call(
            source,
            emit_context,
            statement,
            compiler_options,
        );
        let require_statement = emit_context
            .factory
            .node_factory
            .new_expression_statement(require_call);
        emit_context.set_original(&require_statement, statement);
        emit_context.assign_comment_and_source_map_ranges(&require_statement, statement);
        return vec![require_statement];
    }

    let name = import_declaration_require_binding_name(source, emit_context, *statement);
    let require_call =
        create_import_declaration_require_call(source, emit_context, statement, compiler_options);
    let require_call =
        get_helper_expression_for_import(source, emit_context, *statement, require_call);
    let declaration =
        emit_context
            .factory
            .node_factory
            .new_variable_declaration(name, None, None, require_call);
    let declarations = emit_context.factory.node_factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        vec![declaration],
    );
    let declaration_list = emit_context
        .factory
        .node_factory
        .new_variable_declaration_list(declarations, ast::NodeFlags::CONST);
    let var_statement = emit_context
        .factory
        .node_factory
        .new_variable_statement(None, declaration_list);
    emit_context.set_original(&var_statement, statement);
    emit_context.assign_comment_and_source_map_ranges(&var_statement, statement);
    vec![var_statement]
}

struct ActiveImportDeclarationToRequireInput {
    module_specifier_text: Option<String>,
    has_import_clause: bool,
    has_default_import: bool,
    namespace_binding_name: Option<String>,
    needs_import_star_helper: bool,
    needs_import_default_helper: bool,
}

#[derive(Clone)]
enum ActiveModuleExportName {
    Identifier(String),
    StringLiteral(String),
}

impl ActiveModuleExportName {
    fn from_node(source: &ast::AstStore, node: ast::Node) -> Self {
        if ast::is_string_literal(source, node) {
            Self::StringLiteral(source.text(node))
        } else {
            Self::Identifier(source.text(node))
        }
    }

    fn create_node(&self, factory: &mut ast::NodeFactory) -> ast::Node {
        match self {
            Self::Identifier(text) => factory.new_identifier(text),
            Self::StringLiteral(text) => factory.new_string_literal(text, ast::TokenFlags::NONE),
        }
    }
}

struct ActiveNamedExportSpecifier {
    original: ast::Node,
    export_name: ActiveModuleExportName,
    specifier_name: ActiveModuleExportName,
    export_needs_import_default: bool,
}

enum ActiveExportDeclarationKind {
    NoModuleSpecifier,
    ExportStar,
    Named(Vec<ActiveNamedExportSpecifier>),
    Namespace {
        export_name: ActiveModuleExportName,
        needs_import_star_helper: bool,
    },
}

struct ActiveExportDeclarationToCommonJsInput {
    module_specifier_text: Option<String>,
    kind: ActiveExportDeclarationKind,
}

fn active_import_declaration_to_require_input(
    emit_context: &printer::EmitContext,
    statement: ast::Node,
) -> Option<ActiveImportDeclarationToRequireInput> {
    let source = emit_context.store_for_node(statement);
    if !ast::is_import_declaration(source, statement) {
        return None;
    }

    let module_specifier_text =
        ast::get_external_module_name(source, statement).and_then(|module_name| {
            ast::is_string_literal(source, module_name).then(|| source.text(module_name))
        });
    let import_clause = source.import_clause(statement);
    let has_default_import =
        import_clause.is_some_and(|import_clause| source.name(import_clause).is_some());
    let namespace_binding_name = import_clause.and_then(|import_clause| {
        source
            .named_bindings(import_clause)
            .filter(|node| source.kind(*node) == ast::Kind::NamespaceImport)
            .and_then(|namespace_import| source.name(namespace_import))
            .map(|name| source.text(name))
    });
    Some(ActiveImportDeclarationToRequireInput {
        module_specifier_text,
        has_import_clause: import_clause.is_some(),
        has_default_import,
        namespace_binding_name,
        needs_import_star_helper: get_import_needs_import_star_helper(source, statement),
        needs_import_default_helper: get_import_needs_import_default_helper(source, statement),
    })
}

fn active_export_declaration_to_common_js_input(
    emit_context: &printer::EmitContext,
    statement: ast::Node,
) -> Option<ActiveExportDeclarationToCommonJsInput> {
    let source = emit_context.store_for_node(statement);
    if !ast::is_export_declaration(source, statement) {
        return None;
    }

    let module_specifier_text =
        ast::get_external_module_name(source, statement).and_then(|module_name| {
            ast::is_string_literal(source, module_name).then(|| source.text(module_name))
        });

    let kind = if source.module_specifier(statement).is_none() {
        ActiveExportDeclarationKind::NoModuleSpecifier
    } else {
        match source.export_clause(statement) {
            None => ActiveExportDeclarationKind::ExportStar,
            Some(export_clause) if ast::is_named_exports(source, export_clause) => {
                let specifiers = source
                    .source_elements(export_clause)
                    .map(|elements| {
                        elements
                            .iter()
                            .map(|specifier| {
                                let specifier_name = source
                                    .property_name_or_name(specifier)
                                    .expect("export specifier should have a property name or name");
                                let name = source
                                    .name(specifier)
                                    .expect("export specifier should have a name");
                                ActiveNamedExportSpecifier {
                                    original: specifier,
                                    export_name: ActiveModuleExportName::from_node(source, name),
                                    specifier_name: ActiveModuleExportName::from_node(
                                        source,
                                        specifier_name,
                                    ),
                                    export_needs_import_default: ast::module_export_name_is_default(
                                        source,
                                        specifier_name,
                                    ),
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                ActiveExportDeclarationKind::Named(specifiers)
            }
            Some(export_clause) => {
                let export_name = source.name(export_clause)?;
                ActiveExportDeclarationKind::Namespace {
                    export_name: ActiveModuleExportName::from_node(source, export_name),
                    needs_import_star_helper: get_export_needs_import_star_helper(
                        source, statement,
                    ),
                }
            }
        }
    };

    Some(ActiveExportDeclarationToCommonJsInput {
        module_specifier_text,
        kind,
    })
}

fn create_active_require_call(
    emit_context: &mut printer::EmitContext,
    module_specifier_text: Option<String>,
) -> ast::Node {
    let mut args = Vec::new();
    if let Some(module_specifier_text) = module_specifier_text {
        args.push(
            emit_context
                .factory
                .node_factory
                .new_string_literal(module_specifier_text, ast::TokenFlags::NONE),
        );
    }
    let require = emit_context.factory.node_factory.new_identifier("require");
    let arguments = emit_context.factory.node_factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        args,
    );
    emit_context.factory.node_factory.new_call_expression(
        require,
        None,
        None,
        arguments,
        ast::NodeFlags::NONE,
    )
}

fn transform_active_export_declaration_to_common_js(
    emit_context: &mut printer::EmitContext,
    statement: ast::Node,
    generated_name_node: ast::Node,
    input: ActiveExportDeclarationToCommonJsInput,
) -> Vec<ast::Node> {
    match input.kind {
        ActiveExportDeclarationKind::NoModuleSpecifier => Vec::new(),
        ActiveExportDeclarationKind::ExportStar => {
            let require_call =
                create_active_require_call(emit_context, input.module_specifier_text);
            let exports = emit_context.factory.node_factory.new_identifier("exports");
            let export_star = emit_context
                .factory
                .new_export_star_helper(require_call, exports);
            let statement_node = emit_context
                .factory
                .node_factory
                .new_expression_statement(export_star);
            emit_context.set_original(&statement_node, &statement);
            emit_context.assign_comment_and_source_map_ranges(&statement_node, &statement);
            vec![statement_node]
        }
        ActiveExportDeclarationKind::Named(specifiers) => {
            let mut statements = Vec::new();
            let generated_name = emit_context.new_generated_name_for_node(generated_name_node);
            let require_call =
                create_active_require_call(emit_context, input.module_specifier_text);
            let declaration = emit_context.factory.node_factory.new_variable_declaration(
                generated_name,
                None::<ast::Node>,
                None::<ast::Node>,
                require_call,
            );
            let declarations = emit_context.factory.node_factory.new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                vec![declaration],
            );
            let declaration_list = emit_context
                .factory
                .node_factory
                .new_variable_declaration_list(declarations, ast::NodeFlags::NONE);
            let var_statement = emit_context
                .factory
                .node_factory
                .new_variable_statement(None::<ast::ModifierList>, declaration_list);
            emit_context.set_original(&var_statement, &statement);
            emit_context.assign_comment_and_source_map_ranges(&var_statement, &statement);
            statements.push(var_statement);

            for specifier in specifiers {
                let target = if specifier.export_needs_import_default {
                    emit_context
                        .factory
                        .new_import_default_helper(generated_name)
                } else {
                    generated_name
                };
                let exported_value = match &specifier.specifier_name {
                    ActiveModuleExportName::StringLiteral(_) => {
                        let argument = specifier
                            .specifier_name
                            .create_node(&mut emit_context.factory.node_factory);
                        emit_context
                            .factory
                            .node_factory
                            .new_element_access_expression(
                                target,
                                None,
                                argument,
                                ast::NodeFlags::NONE,
                            )
                    }
                    ActiveModuleExportName::Identifier(_) => {
                        let name = specifier
                            .specifier_name
                            .create_node(&mut emit_context.factory.node_factory);
                        emit_context
                            .factory
                            .node_factory
                            .new_property_access_expression(
                                target,
                                None,
                                name,
                                ast::NodeFlags::NONE,
                            )
                    }
                };
                let export_name = specifier
                    .export_name
                    .create_node(&mut emit_context.factory.node_factory);
                let expression = create_live_binding_export_expression(
                    &mut emit_context.factory,
                    export_name,
                    exported_value,
                );
                let statement_node = emit_context
                    .factory
                    .node_factory
                    .new_expression_statement(expression);
                emit_context.set_original(&statement_node, &specifier.original);
                emit_context
                    .assign_comment_and_source_map_ranges(&statement_node, &specifier.original);
                statements.push(statement_node);
            }
            statements
        }
        ActiveExportDeclarationKind::Namespace {
            export_name,
            needs_import_star_helper,
        } => {
            let mut value = create_active_require_call(emit_context, input.module_specifier_text);
            if needs_import_star_helper {
                value = emit_context.factory.new_import_star_helper(value);
            }
            let export_name = export_name.create_node(&mut emit_context.factory.node_factory);
            let expression = create_export_expression_for_name(
                &mut emit_context.factory.node_factory,
                export_name,
                value,
            );
            let statement_node = emit_context
                .factory
                .node_factory
                .new_expression_statement(expression);
            emit_context.set_original(&statement_node, &statement);
            emit_context.assign_comment_and_source_map_ranges(&statement_node, &statement);
            vec![statement_node]
        }
    }
}

fn transform_active_import_declaration_to_require(
    emit_context: &mut printer::EmitContext,
    statement: ast::Node,
    generated_name_node: ast::Node,
    input: ActiveImportDeclarationToRequireInput,
) -> Vec<ast::Node> {
    let require_call = {
        let mut args = Vec::new();
        if let Some(module_specifier_text) = input.module_specifier_text {
            args.push(
                emit_context
                    .factory
                    .node_factory
                    .new_string_literal(module_specifier_text, ast::TokenFlags::NONE),
            );
        }
        let require = emit_context.factory.node_factory.new_identifier("require");
        let arguments = emit_context.factory.node_factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            args,
        );
        emit_context.factory.node_factory.new_call_expression(
            require,
            None,
            None,
            arguments,
            ast::NodeFlags::NONE,
        )
    };

    if !input.has_import_clause {
        let require_statement = emit_context
            .factory
            .node_factory
            .new_expression_statement(require_call);
        emit_context.set_original(&require_statement, &statement);
        emit_context.assign_comment_and_source_map_ranges(&require_statement, &statement);
        return vec![require_statement];
    }

    let name = if input.namespace_binding_name.is_some() && !input.has_default_import {
        emit_context
            .factory
            .node_factory
            .new_identifier(input.namespace_binding_name.as_ref().unwrap())
    } else {
        emit_context.new_generated_name_for_node(generated_name_node)
    };
    let require_call = if input.needs_import_star_helper {
        emit_context.factory.new_import_star_helper(require_call)
    } else if input.needs_import_default_helper {
        emit_context.factory.new_import_default_helper(require_call)
    } else {
        require_call
    };
    let mut declarations_vec = vec![emit_context.factory.node_factory.new_variable_declaration(
        name,
        None,
        None,
        require_call,
    )];
    if let Some(namespace_binding_name) = input
        .namespace_binding_name
        .filter(|_| input.has_default_import)
    {
        let namespace_binding_name = emit_context
            .factory
            .node_factory
            .new_identifier(namespace_binding_name);
        let generated_name = emit_context.new_generated_name_for_node(generated_name_node);
        declarations_vec.push(emit_context.factory.node_factory.new_variable_declaration(
            namespace_binding_name,
            None,
            None,
            generated_name,
        ));
    }
    let declarations = emit_context.factory.node_factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        declarations_vec,
    );
    let declaration_list = emit_context
        .factory
        .node_factory
        .new_variable_declaration_list(declarations, ast::NodeFlags::CONST);
    let var_statement = emit_context
        .factory
        .node_factory
        .new_variable_statement(None, declaration_list);
    emit_context.set_original(&var_statement, &statement);
    emit_context.assign_comment_and_source_map_ranges(&var_statement, &statement);
    vec![var_statement]
}

fn transform_export_declaration_to_common_js(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    statement: &ast::Node,
    compiler_options: &core::CompilerOptions,
) -> Vec<ast::Node> {
    if source.module_specifier(*statement).is_none() {
        // Elide export declarations with no module specifier as they are handled
        // elsewhere.
        return Vec::new();
    }
    if source.export_clause(*statement).is_none() {
        let require_call = create_import_declaration_require_call(
            source,
            emit_context,
            statement,
            compiler_options,
        );
        let exports = emit_context.factory.node_factory.new_identifier("exports");
        let export_star = emit_context
            .factory
            .new_export_star_helper(require_call, exports);
        let statement_node = emit_context
            .factory
            .node_factory
            .new_expression_statement(export_star);
        emit_context.set_original(&statement_node, statement);
        emit_context.assign_comment_and_source_map_ranges(&statement_node, statement);
        return vec![statement_node];
    }
    let Some(export_clause) = source.export_clause(*statement) else {
        return Vec::new();
    };
    if ast::is_named_exports(source, export_clause) {
        let mut statements = Vec::new();
        let generated_name = emit_context
            .factory
            .new_generated_name_for_node(source, statement);
        let require_call = create_import_declaration_require_call(
            source,
            emit_context,
            statement,
            compiler_options,
        );
        let declaration = emit_context.factory.node_factory.new_variable_declaration(
            generated_name,
            None::<ast::Node>,
            None::<ast::Node>,
            require_call,
        );
        let declarations = emit_context.factory.node_factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![declaration],
        );
        let declaration_list = emit_context
            .factory
            .node_factory
            .new_variable_declaration_list(declarations, ast::NodeFlags::NONE);
        let var_statement = emit_context
            .factory
            .node_factory
            .new_variable_statement(None::<ast::ModifierList>, declaration_list);
        emit_context.set_original(&var_statement, statement);
        emit_context.assign_comment_and_source_map_ranges(&var_statement, statement);
        statements.push(var_statement);

        if let Some(elements) = source.source_elements(export_clause) {
            for specifier in elements.iter() {
                let specifier_name = source
                    .property_name_or_name(specifier)
                    .expect("export specifier should have a property name or name");
                let export_needs_import_default =
                    ast::module_export_name_is_default(source, specifier_name);
                let target = if export_needs_import_default {
                    emit_context
                        .factory
                        .new_import_default_helper(generated_name)
                } else {
                    generated_name
                };
                let exported_value = if ast::is_string_literal(source, specifier_name) {
                    let argument = emit_context
                        .factory
                        .node_factory
                        .deep_clone_node_from_store_preserve_location(source, specifier_name);
                    emit_context
                        .factory
                        .node_factory
                        .new_element_access_expression(target, None, argument, ast::NodeFlags::NONE)
                } else {
                    let name = emit_context
                        .factory
                        .node_factory
                        .deep_clone_node_from_store_preserve_location(source, specifier_name);
                    emit_context
                        .factory
                        .node_factory
                        .new_property_access_expression(target, None, name, ast::NodeFlags::NONE)
                };
                let name = source
                    .name(specifier)
                    .expect("export specifier should have a name");
                let export_name = if ast::is_string_literal(source, name) {
                    emit_context
                        .factory
                        .node_factory
                        .deep_clone_node_from_store_preserve_location(source, name)
                } else {
                    emit_context.factory.get_export_name(source, &specifier)
                };
                let expression = create_live_binding_export_expression(
                    &mut emit_context.factory,
                    export_name,
                    exported_value,
                );
                let statement = emit_context
                    .factory
                    .node_factory
                    .new_expression_statement(expression);
                emit_context.set_original(&statement, &specifier);
                emit_context.assign_comment_and_source_map_ranges(&statement, &specifier);
                statements.push(statement);
            }
        }
        return statements;
    }

    // export * as ns from "mod";
    // export * as default from "mod";
    let Some(export_name) = source.name(export_clause) else {
        return Vec::new();
    };
    let export_name = if ast::is_string_literal(source, export_name) {
        emit_context
            .factory
            .node_factory
            .deep_clone_node_from_store_preserve_location(source, export_name)
    } else {
        emit_context
            .factory
            .node_factory
            .deep_clone_node_from_store_preserve_location(source, export_name)
    };
    let require_call =
        create_import_declaration_require_call(source, emit_context, statement, compiler_options);
    let value = get_helper_expression_for_export(source, emit_context, *statement, require_call);
    let expression = create_export_expression_for_name(
        &mut emit_context.factory.node_factory,
        export_name,
        value,
    );
    let statement_node = emit_context
        .factory
        .node_factory
        .new_expression_statement(expression);
    emit_context.set_original(&statement_node, statement);
    emit_context.assign_comment_and_source_map_ranges(&statement_node, statement);
    vec![statement_node]
}

fn append_exports_of_import_declaration(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    statement: ast::Node,
    exported_names: &[String],
    local_export_specifiers: &HashMap<String, Vec<String>>,
    statements: &mut Vec<ast::Node>,
) {
    let Some(import_clause) = source.import_clause(statement) else {
        return;
    };
    let mut seen = HashSet::new();
    if let Some(name) = source.name(import_clause) {
        let local_name = source.text(name);
        let reference = ImportReference {
            import_declaration: statement,
            property_name: None,
        };
        append_exports_of_import_binding(
            source,
            emit_context,
            statements,
            exported_names,
            local_export_specifiers,
            &mut seen,
            &local_name,
            false,
            |emit_context| imported_reference_expression(emit_context, reference),
        );
    }

    let Some(bindings) = source.named_bindings(import_clause) else {
        return;
    };
    if ast::is_namespace_import(source, bindings) {
        if let Some(name) = source.name(bindings) {
            let local_name = source.text(name);
            append_exports_of_import_binding(
                source,
                emit_context,
                statements,
                exported_names,
                local_export_specifiers,
                &mut seen,
                &local_name,
                false,
                |emit_context| {
                    emit_context
                        .factory
                        .node_factory
                        .new_identifier(&local_name)
                },
            );
        }
        return;
    }

    if ast::is_named_imports(source, bindings)
        && let Some(elements) = source.source_elements(bindings)
    {
        for import_binding in elements.iter() {
            let Some(name) = source.name(import_binding) else {
                continue;
            };
            let local_name = source.text(name);
            let reference = ImportReference {
                import_declaration: statement,
                property_name: source.property_name_or_name(import_binding),
            };
            append_exports_of_import_binding(
                source,
                emit_context,
                statements,
                exported_names,
                local_export_specifiers,
                &mut seen,
                &local_name,
                true,
                |emit_context| imported_reference_expression(emit_context, reference),
            );
        }
    }
}

fn append_exports_of_active_or_read_declaration(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    active_statement: ast::Node,
    read_statement: ast::Node,
    local_export_specifiers: &HashMap<String, Vec<String>>,
    statements: &mut Vec<ast::Node>,
) {
    if ast::is_import_declaration(source, read_statement) {
        append_exports_of_import_declaration(
            source,
            emit_context,
            active_statement,
            &[],
            local_export_specifiers,
            statements,
        );
    }
    let Some(name) = source.name(read_statement) else {
        return;
    };
    if !ast::is_identifier(source, name) {
        return;
    }
    let local_name = source.text(name);
    let Some(export_names) = local_export_specifiers.get(&local_name) else {
        return;
    };
    for export_name in export_names {
        statements.push(create_export_assignment_statement(
            &mut emit_context.factory.node_factory,
            export_name,
            &local_name,
            false,
        ));
    }
}

fn append_exports_of_import_binding(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    statements: &mut Vec<ast::Node>,
    _exported_names: &[String],
    local_export_specifiers: &HashMap<String, Vec<String>>,
    seen: &mut HashSet<String>,
    local_name: &str,
    live_binding: bool,
    mut expression: impl FnMut(&mut printer::EmitContext) -> ast::Node,
) {
    if let Some(export_names) = local_export_specifiers.get(local_name) {
        for export_name in export_names {
            let expression = expression(emit_context);
            append_export_of_import_binding(
                emit_context,
                statements,
                seen,
                export_name,
                expression,
                live_binding,
            );
        }
    }
    let _ = source;
}

fn append_exports_of_import_equals_declaration(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    statement: ast::Node,
    local_export_specifiers: &HashMap<String, Vec<String>>,
    statements: &mut Vec<ast::Node>,
) {
    let Some(name) = source.name(statement) else {
        return;
    };
    let local_name = source.text(name);
    let mut seen = HashSet::new();
    append_exports_of_import_binding(
        source,
        emit_context,
        statements,
        &[],
        local_export_specifiers,
        &mut seen,
        &local_name,
        false,
        |emit_context| {
            if ast::has_syntactic_modifier(source, statement, ast::ModifierFlags::EXPORT) {
                exports_property_access(&mut emit_context.factory.node_factory, &local_name)
            } else {
                emit_context
                    .factory
                    .node_factory
                    .new_identifier(&local_name)
            }
        },
    );
}

fn append_export_of_import_binding(
    emit_context: &mut printer::EmitContext,
    statements: &mut Vec<ast::Node>,
    seen: &mut HashSet<String>,
    export_name: &str,
    expression: ast::Node,
    live_binding: bool,
) {
    if !seen.insert(export_name.to_owned()) {
        return;
    }
    let expression = if live_binding {
        let export_name = emit_context
            .factory
            .node_factory
            .new_string_literal(export_name, ast::TokenFlags::NONE);
        create_live_binding_export_expression(&mut emit_context.factory, export_name, expression)
    } else {
        let left = exports_property_access(&mut emit_context.factory.node_factory, export_name);
        let equals = emit_context
            .factory
            .node_factory
            .new_token(ast::Kind::EqualsToken);
        emit_context
            .factory
            .node_factory
            .new_binary_expression(None, left, None, equals, expression)
    };
    statements.push(
        emit_context
            .factory
            .node_factory
            .new_expression_statement(expression),
    );
}

fn imported_reference_expression(
    emit_context: &mut printer::EmitContext,
    reference: ImportReference,
) -> ast::Node {
    let property_name_info = reference.property_name.map(|property_name| {
        let source = emit_context.store_for_node(property_name);
        (
            ast::is_string_literal(source, property_name),
            source.text(property_name),
        )
    });
    let target = emit_context.new_generated_name_for_node(reference.import_declaration);
    let property_name = match property_name_info {
        Some(property_name) => {
            if property_name.0 {
                let argument = emit_context
                    .factory
                    .node_factory
                    .new_string_literal(property_name.1, ast::TokenFlags::NONE);
                return emit_context
                    .factory
                    .node_factory
                    .new_element_access_expression(target, None, argument, ast::NodeFlags::NONE);
            }
            emit_context
                .factory
                .node_factory
                .new_identifier(property_name.1)
        }
        None => emit_context.factory.node_factory.new_identifier("default"),
    };
    emit_context
        .factory
        .node_factory
        .new_property_access_expression(target, None, property_name, ast::NodeFlags::NONE)
}

fn imported_reference_expression_from_identifier(
    emit_context: &mut printer::EmitContext,
    reference: ImportReference,
    node: ast::Node,
) -> ast::Node {
    let property_name_info = reference.property_name.map(|property_name| {
        let source = emit_context.store_for_node(property_name);
        (
            ast::is_string_literal(source, property_name),
            source.text(property_name),
        )
    });
    let target = emit_context.new_generated_name_for_node(reference.import_declaration);
    let reference = match property_name_info {
        Some((true, property_name_text)) => {
            let argument = emit_context
                .factory
                .node_factory
                .new_string_literal(property_name_text, ast::TokenFlags::NONE);
            emit_context
                .factory
                .node_factory
                .new_element_access_expression(target, None, argument, ast::NodeFlags::NONE)
        }
        Some((false, property_name_text)) => {
            let reference_name = emit_context
                .factory
                .node_factory
                .new_identifier(property_name_text);
            emit_context.mark_emit_node(
                &reference_name,
                printer::EF_NO_SOURCE_MAP | printer::EF_NO_COMMENTS,
            );
            emit_context
                .factory
                .node_factory
                .new_property_access_expression(target, None, reference_name, ast::NodeFlags::NONE)
        }
        None => {
            let property_name = emit_context.factory.node_factory.new_identifier("default");
            emit_context
                .factory
                .node_factory
                .new_property_access_expression(target, None, property_name, ast::NodeFlags::NONE)
        }
    };
    emit_context.assign_comment_and_source_map_ranges(&reference, &node);
    let loc = emit_context.store_for_node(node).loc(node);
    emit_context
        .factory
        .node_factory
        .place_transformed_node(reference, loc);
    reference
}

fn is_named_default_reference(source: &ast::AstStore, node: ast::Node) -> bool {
    source
        .property_name_or_name(node)
        .is_some_and(|name| ast::module_export_name_is_default(source, name))
}

fn contains_default_reference(source: &ast::AstStore, node: ast::Node) -> bool {
    (ast::is_named_imports(source, node) || ast::is_named_exports(source, node))
        && source.source_elements(node).is_some_and(|elements| {
            elements
                .iter()
                .any(|element| is_named_default_reference(source, element))
        })
}

fn get_import_needs_import_star_helper(source: &ast::AstStore, statement: ast::Node) -> bool {
    if ast::get_namespace_declaration_node(source, statement).is_some() {
        return true;
    }
    let Some(import_clause) = source.import_clause(statement) else {
        return false;
    };
    let Some(bindings) = source.named_bindings(import_clause) else {
        return false;
    };
    if !ast::is_named_imports(source, bindings) {
        return false;
    }
    let Some(elements) = source.source_elements(bindings) else {
        return false;
    };
    let default_ref_count = elements
        .iter()
        .filter(|binding| is_named_default_reference(source, *binding))
        .count();
    // Import star is required if there's default named refs mixed with non-default refs, or if theres non-default refs and it has a default import
    (default_ref_count > 0 && default_ref_count != elements.len())
        || ((elements.len() - default_ref_count) != 0 && ast::has_default_import(source, statement))
}

fn get_export_needs_import_star_helper(source: &ast::AstStore, statement: ast::Node) -> bool {
    ast::get_namespace_declaration_node(source, statement).is_some()
}

fn get_import_needs_import_default_helper(source: &ast::AstStore, statement: ast::Node) -> bool {
    // Import default is needed if there's a default import or a default ref and no other refs (meaning an import star helper wasn't requested)
    !get_import_needs_import_star_helper(source, statement)
        && (ast::has_default_import(source, statement)
            || source
                .import_clause(statement)
                .is_some_and(|import_clause| {
                    source
                        .named_bindings(import_clause)
                        .is_some_and(|bindings| {
                            ast::is_named_imports(source, bindings)
                                && contains_default_reference(source, bindings)
                        })
                }))
}

fn get_helper_expression_for_import(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    node: ast::Node,
    inner_expr: ast::Node,
) -> ast::Node {
    if get_import_needs_import_star_helper(source, node) {
        return emit_context.factory.new_import_star_helper(inner_expr);
    }
    if get_import_needs_import_default_helper(source, node) {
        return emit_context.factory.new_import_default_helper(inner_expr);
    }
    inner_expr
}

fn get_helper_expression_for_export(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    node: ast::Node,
    inner_expr: ast::Node,
) -> ast::Node {
    if get_export_needs_import_star_helper(source, node) {
        return emit_context.factory.new_import_star_helper(inner_expr);
    }
    inner_expr
}

fn import_declaration_require_binding_name(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    statement: ast::Node,
) -> ast::Node {
    let import_clause = source
        .import_clause(statement)
        .expect("import declaration with bindings should have an import clause");
    let namespace_import = source
        .named_bindings(import_clause)
        .filter(|node| source.kind(*node) == ast::Kind::NamespaceImport);
    if source.name(import_clause).is_none()
        && let Some(namespace_import) = namespace_import
        && let Some(name) = source.name(namespace_import)
    {
        return emit_context
            .factory
            .node_factory
            .deep_clone_node_from_store_preserve_location(source, name);
    }
    emit_context
        .factory
        .new_generated_name_for_node(source, &statement)
}

// Get the name of a target module from an import/export declaration as should be written in the emitted output.
// The emitted output name can be different from the input if:
//  1. The module has a /// <amd-module name="<new name>" />
//  2. --out or --outFile is used, making the name relative to the rootDir
//     3- The containing SourceFile has an entry in renamedDependencies for the import as requested by some module loaders (e.g. System).
//
// Otherwise, a new StringLiteral node representing the module name will be returned.
fn get_external_module_name_literal(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    import_node: ast::Node,
    compiler_options: &core::CompilerOptions,
) -> Option<ast::Node> {
    let module_name = ast::get_external_module_name(source, import_node)?;
    if !ast::is_string_literal(source, module_name) {
        return None;
    }

    let text = source.text(module_name);
    let text =
        crate::moduletransforms::utilities::rewrite_module_specifier_text(&text, compiler_options)
            .unwrap_or(text);
    Some(
        emit_context
            .factory
            .node_factory
            .new_string_literal(text, ast::TokenFlags::NONE),
    )
}

// Creates a `require()` call to import an external module.
fn create_import_declaration_require_call(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    statement: &ast::Node,
    compiler_options: &core::CompilerOptions,
) -> ast::Node {
    let mut args = Vec::new();
    if let Some(module_name) =
        get_external_module_name_literal(source, emit_context, *statement, compiler_options)
    {
        args.push(module_name);
    }

    let require = emit_context.factory.node_factory.new_identifier("require");
    let arguments = emit_context.factory.node_factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        args,
    );
    emit_context.factory.node_factory.new_call_expression(
        require,
        None,
        None,
        arguments,
        ast::NodeFlags::NONE,
    )
}

fn rewrite_common_js_statement(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    import_references: &HashMap<String, ImportReference>,
    direct_exported_names: &HashSet<String>,
    local_export_specifiers: &HashMap<String, Vec<String>>,
    module_transform_facts: &ModuleTransformFacts,
    compiler_options: &core::CompilerOptions,
    file_name: &str,
    module_format: core::ModuleKind,
    source_file_contains_dynamic_import: bool,
    statement: ast::Node,
) -> ast::Node {
    if import_references.is_empty()
        && direct_exported_names.is_empty()
        && local_export_specifiers.is_empty()
        && module_transform_facts
            .referenced_source_file_export_references
            .is_empty()
        && !source_file_contains_dynamic_import
        && !compiler_options
            .rewrite_relative_import_extensions
            .is_true()
    {
        if statement.store_id() == emit_context.factory.node_factory.store().store_id() {
            return statement;
        }
        return emit_context
            .factory
            .node_factory
            .deep_clone_node_from_store_preserve_location(source, statement);
    }
    refresh_active_emit_parent_links(emit_context, statement);
    let mut rewriter = CommonJsReferenceRewriter {
        source,
        emit_context,
        import_state: ast::AstImportState::new(),
        import_references,
        local_export_specifiers,
        direct_exported_names,
        module_transform_facts,
        compiler_options,
        file_name,
        module_format,
        source_file_contains_dynamic_import,
        current_node: None,
        parent_node: None,
    };
    rewriter
        .visit_node(Some(statement))
        .expect("statement is required")
}

fn refresh_active_emit_parent_links(emit_context: &mut printer::EmitContext, root: ast::Node) {
    let factory = &mut emit_context.factory.node_factory;
    let factory_store_id = factory.store().store_id();
    if root.store_id() != factory_store_id {
        return;
    }

    factory.link_emit_synthetic_parent(root, None);
    let mut stack = vec![root];
    while let Some(parent) = stack.pop() {
        let children = {
            let store = factory.store();
            let mut children = Vec::new();
            let _ = store.for_each_present_child(parent, |child| {
                if child.store_id() == factory_store_id {
                    children.push(child);
                }
                std::ops::ControlFlow::Continue(())
            });
            children
        };
        for child in children {
            factory.link_emit_synthetic_parent(child, Some(parent));
            stack.push(child);
        }
    }
}

fn rewrite_common_js_expression(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    import_references: &HashMap<String, ImportReference>,
    direct_exported_names: &HashSet<String>,
    local_export_specifiers: &HashMap<String, Vec<String>>,
    module_transform_facts: &ModuleTransformFacts,
    compiler_options: &core::CompilerOptions,
    file_name: &str,
    module_format: core::ModuleKind,
    source_file_contains_dynamic_import: bool,
    expression: ast::Node,
) -> ast::Node {
    if import_references.is_empty()
        && direct_exported_names.is_empty()
        && local_export_specifiers.is_empty()
        && module_transform_facts
            .referenced_source_file_export_references
            .is_empty()
        && !source_file_contains_dynamic_import
        && !compiler_options
            .rewrite_relative_import_extensions
            .is_true()
    {
        if expression.store_id() == emit_context.factory.node_factory.store().store_id() {
            return expression;
        }
        return emit_context
            .factory
            .node_factory
            .deep_clone_node_from_store_preserve_location(source, expression);
    }
    refresh_active_emit_parent_links(emit_context, expression);
    let mut rewriter = CommonJsReferenceRewriter {
        source,
        emit_context,
        import_state: ast::AstImportState::new(),
        import_references,
        local_export_specifiers,
        direct_exported_names,
        module_transform_facts,
        compiler_options,
        file_name,
        module_format,
        source_file_contains_dynamic_import,
        current_node: None,
        parent_node: None,
    };
    if ast::is_identifier(rewriter.store_for(expression), expression) {
        rewriter.visit_expression_identifier(expression)
    } else {
        rewriter
            .visit_node(Some(expression))
            .expect("expression is required")
    }
}

fn copy_originals_for_preserved_subtree_if_unset(
    emit_context: &mut printer::EmitContext,
    source: ast::Node,
    imported: ast::Node,
) {
    if source == imported {
        return;
    }

    if emit_context.original(&imported).is_none() {
        emit_context.set_original(&imported, &source);
    }

    let source_children =
        collect_child_nodes_for_original_copy(emit_context.store_for_node(source), source);
    let imported_children =
        collect_child_nodes_for_original_copy(emit_context.store_for_node(imported), imported);
    for (source_child, imported_child) in source_children.into_iter().zip(imported_children) {
        copy_originals_for_preserved_subtree_if_unset(emit_context, source_child, imported_child);
    }
}

fn copy_originals_for_matching_subtree_if_unset(
    emit_context: &mut printer::EmitContext,
    source: ast::Node,
    active: ast::Node,
) {
    if source == active {
        return;
    }

    let (same_kind, same_identifier_text, source_children, active_children) = {
        let source_store = emit_context.store_for_node(source);
        let active_store = emit_context.store_for_node(active);
        (
            source_store.kind(source) == active_store.kind(active),
            !ast::is_identifier(source_store, source)
                || ast::is_identifier(active_store, active)
                    && source_store.text(source) == active_store.text(active),
            collect_child_nodes_for_original_copy(source_store, source),
            collect_child_nodes_for_original_copy(active_store, active),
        )
    };
    if !same_kind || !same_identifier_text {
        return;
    }

    if emit_context.original(&active).is_none() {
        emit_context.set_original(&active, &source);
    }

    if source_children.len() != active_children.len() {
        return;
    }
    for (source_child, active_child) in source_children.into_iter().zip(active_children) {
        copy_originals_for_matching_subtree_if_unset(emit_context, source_child, active_child);
    }
}

fn collect_child_nodes_for_original_copy(store: &ast::AstStore, node: ast::Node) -> Vec<ast::Node> {
    let mut children = Vec::new();
    let _ = store.for_each_child(node, |child| {
        if let Some(child) = child {
            children.push(child);
        }
        std::ops::ControlFlow::Continue(())
    });
    children
}

struct CommonJsReferenceRewriter<'a, 'source> {
    source: &'source ast::AstStore,
    emit_context: &'a mut printer::EmitContext,
    import_state: ast::AstImportState,
    import_references: &'a HashMap<String, ImportReference>,
    local_export_specifiers: &'a HashMap<String, Vec<String>>,
    direct_exported_names: &'a HashSet<String>,
    module_transform_facts: &'a ModuleTransformFacts,
    compiler_options: &'a core::CompilerOptions,
    file_name: &'a str,
    module_format: core::ModuleKind,
    source_file_contains_dynamic_import: bool,
    current_node: Option<ast::Node>,
    parent_node: Option<ast::Node>,
}

impl CommonJsReferenceRewriter<'_, '_> {
    fn push_node(&mut self, node: ast::Node) -> Option<ast::Node> {
        let grandparent_node = self.parent_node;
        self.parent_node = self.current_node;
        self.current_node = Some(node);
        grandparent_node
    }

    fn pop_node(&mut self, grandparent_node: Option<ast::Node>) {
        self.current_node = self.parent_node;
        self.parent_node = grandparent_node;
    }

    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        ast::AstImportState::store_for(self.source, self.factory(), node)
    }

    fn preserve_source_node(&mut self, node: ast::Node) -> ast::Node {
        let mut import_state = std::mem::take(&mut self.import_state);
        let imported = import_state.preserve_node(
            self.source,
            &mut self.emit_context.factory.node_factory,
            node,
        );
        self.import_state = import_state;
        copy_originals_for_preserved_subtree_if_unset(self.emit_context, node, imported);
        imported
    }

    fn append_visited_node(
        &mut self,
        original: ast::Node,
        visited: Option<ast::Node>,
        out: &mut Vec<ast::Node>,
        changed: &mut bool,
    ) {
        match visited {
            Some(visited) if self.preserved_source_node_matches(Some(original), Some(visited)) => {
                out.push(self.preserve_source_node(original));
            }
            Some(visited) => {
                *changed = true;
                let store = self.store_for(visited);
                if store.kind(visited) == ast::Kind::SyntaxList {
                    let nodes = store
                        .syntax_list_children(visited)
                        .expect("SyntaxList should have children")
                        .iter()
                        .flatten()
                        .collect::<Vec<_>>();
                    for node in nodes {
                        out.push(self.preserve_node(node));
                    }
                } else {
                    out.push(self.preserve_node(visited));
                }
            }
            None => *changed = true,
        }
    }

    fn imported_reference_expression_from_identifier(
        &mut self,
        reference: ImportReference,
        node: ast::Node,
    ) -> ast::Node {
        imported_reference_expression_from_identifier(self.emit_context, reference, node)
    }

    fn should_substitute_exported_name(&mut self, node: ast::Node) -> bool {
        let Some(auto_generate) = self.emit_context.get_auto_generate_info(Some(&node)) else {
            return true;
        };
        auto_generate.flags.has_allow_name_substitution()
    }

    fn is_file_level_reserved_generated_identifier(&mut self, node: ast::Node) -> bool {
        self.emit_context
            .get_auto_generate_info(Some(&node))
            .is_some_and(|info| {
                info.flags.is_file_level()
                    && info.flags.is_optimistic()
                    && info.flags.is_reserved_in_nested_scopes()
            })
    }

    fn can_rewrite_exported_identifier_reference(&mut self, node: ast::Node) -> bool {
        let is_export_name = crate::utilities::is_export_name(self.emit_context, &node);
        self.should_substitute_exported_name(node)
            && !crate::utilities::is_helper_name(self.emit_context, &node)
            && !crate::utilities::is_local_name(self.emit_context, &node)
            && (is_export_name || !self.is_declaration_name_of_enum_or_namespace(node))
            && (!crate::utilities::is_generated_identifier(self.emit_context, &node)
                || self.is_file_level_reserved_generated_identifier(node))
    }

    fn can_rewrite_generated_import_identifier_reference(&mut self, node: ast::Node) -> bool {
        crate::utilities::is_generated_identifier(self.emit_context, &node)
            && self.should_substitute_exported_name(node)
            && !crate::utilities::is_helper_name(self.emit_context, &node)
            && !crate::utilities::is_local_name(self.emit_context, &node)
            && !self.is_declaration_name_of_enum_or_namespace(node)
    }

    fn can_rewrite_emitted_import_identifier_reference(&mut self, node: ast::Node) -> bool {
        let (loc, has_parent) = {
            let store = self.store_for(node);
            (store.loc(node), store.parent(node).is_some())
        };
        !crate::utilities::is_generated_identifier(self.emit_context, &node)
            && ast::range_is_synthesized(loc)
            && has_parent
            && self.should_substitute_exported_name(node)
            && !crate::utilities::is_helper_name(self.emit_context, &node)
            && !crate::utilities::is_local_name(self.emit_context, &node)
            && !self.is_declaration_name_of_enum_or_namespace(node)
    }

    fn is_declaration_name_of_enum_or_namespace(&self, node: ast::Node) -> bool {
        let original = self.emit_context.most_original(&node);
        let store = self.emit_context.store_for_node(original);
        let Some(parent) = store.parent(original) else {
            return false;
        };
        let parent_store = self.emit_context.store_for_node(parent);
        crate::moduletransforms::utilities::is_declaration_name_of_enum_or_namespace(
            match parent_store.kind(parent) {
                ast::Kind::EnumDeclaration => {
                    crate::moduletransforms::utilities::DeclarationParentKind::EnumDeclaration
                }
                ast::Kind::ModuleDeclaration => {
                    crate::moduletransforms::utilities::DeclarationParentKind::ModuleDeclaration
                }
                _ => crate::moduletransforms::utilities::DeclarationParentKind::Other,
            },
            parent_store.name(parent) == Some(original),
        )
    }

    fn get_exports(&mut self, node: ast::Node) -> Vec<String> {
        let store = self.store_for(node);
        let text = store.text(node);
        if !crate::utilities::is_generated_identifier(self.emit_context, &node) {
            let original = self.emit_context.most_original(&node);
            let original_store = self.emit_context.store_for_node(original);
            if let Some(exports) = self
                .module_transform_facts
                .referenced_export_bindings
                .get(&original_store.loc(original))
            {
                return exports.clone();
            }
            return Vec::new();
        }

        let mut exports = Vec::new();
        if self.is_file_level_reserved_generated_identifier(node) {
            if self.direct_exported_names.contains(&text) {
                exports.push(text.clone());
            }
            for export_name in self
                .local_export_specifiers
                .get(&text)
                .cloned()
                .unwrap_or_default()
            {
                if !exports.iter().any(|name| name == &export_name) {
                    exports.push(export_name);
                }
            }
        }
        exports
    }

    // destructuringNeedsFlattening checks whether a destructuring assignment target contains any
    // exported identifiers that need to be flattened into individual export assignments.
    fn destructuring_needs_flattening(&mut self, node: ast::Node) -> bool {
        match self.store_for(node).kind(node) {
            ast::Kind::ObjectLiteralExpression => {
                let properties = {
                    let Some(properties) = self.store_for(node).properties(node) else {
                        return false;
                    };
                    properties.iter().collect::<Vec<_>>()
                };
                for elem in properties {
                    match self.store_for(elem).kind(elem) {
                        ast::Kind::PropertyAssignment => {
                            if let Some(initializer) = self.store_for(elem).initializer(elem)
                                && self.destructuring_needs_flattening(initializer)
                            {
                                return true;
                            }
                        }
                        ast::Kind::ShorthandPropertyAssignment => {
                            if let Some(name) = self.store_for(elem).name(elem)
                                && self.destructuring_needs_flattening(name)
                            {
                                return true;
                            }
                        }
                        ast::Kind::SpreadAssignment => {
                            if let Some(expression) = self.store_for(elem).expression(elem)
                                && self.destructuring_needs_flattening(expression)
                            {
                                return true;
                            }
                        }
                        ast::Kind::MethodDeclaration
                        | ast::Kind::GetAccessor
                        | ast::Kind::SetAccessor => return false,
                        _ => {}
                    }
                }
                false
            }
            ast::Kind::ArrayLiteralExpression => {
                let elements = {
                    let Some(elements) = self.store_for(node).elements(node) else {
                        return false;
                    };
                    elements.iter().collect::<Vec<_>>()
                };
                for elem in elements {
                    if self.store_for(elem).kind(elem) == ast::Kind::SpreadElement {
                        if let Some(expression) = self.store_for(elem).expression(elem)
                            && self.destructuring_needs_flattening(expression)
                        {
                            return true;
                        }
                    } else if self.destructuring_needs_flattening(elem) {
                        return true;
                    }
                }
                false
            }
            ast::Kind::Identifier => {
                let exported_names = self.get_exports(node);
                let threshold = if crate::utilities::is_export_name(self.emit_context, &node) {
                    1
                } else {
                    0
                };
                exported_names.len() > threshold
            }
            _ => false,
        }
    }

    fn exports_property_access(&mut self, name: &str) -> ast::Node {
        exports_property_access(&mut self.emit_context.factory.node_factory, name)
    }

    fn create_export_expression(&mut self, export_name: &str, value: ast::Node) -> ast::Node {
        let left = self.exports_property_access(export_name);
        self.emit_context
            .factory
            .new_assignment_expression(left, value)
    }

    fn create_export_expression_for_name(
        &mut self,
        export_name: ast::Node,
        value: ast::Node,
        location: Option<core::TextRange>,
    ) -> ast::Node {
        let left = exports_property_access_for_name(self.factory_mut(), export_name);
        let expression = self
            .emit_context
            .factory
            .new_assignment_expression(left, value);
        if let Some(location) = location {
            self.emit_context.set_comment_range(&expression, location);
        }
        expression
    }

    fn visit_assignment_expression(&mut self, node: ast::Node) -> ast::Node {
        let left = self
            .store_for(node)
            .left(node)
            .expect("assignment expression should have a left operand");
        let left_text = self.store_for(left).text(left);
        let original_left = self.emit_context.most_original(&left);
        let original_left_store = self.emit_context.store_for_node(original_left);
        let references_direct_exported_variable = ast::is_identifier(self.store_for(left), left)
            && (self
                .module_transform_facts
                .references_direct_exported_variable(original_left_store, original_left)
                || original_left_store
                    .parent(original_left)
                    .is_some_and(|parent| {
                        ast::is_variable_declaration(original_left_store, parent)
                            && original_left_store.name(parent) == Some(original_left)
                            && is_direct_exported_variable_declaration(original_left_store, parent)
                    }));
        let exported_names = if ast::is_identifier(self.store_for(left), left)
            && (!crate::utilities::is_generated_identifier(self.emit_context, &left)
                || self.is_file_level_reserved_generated_identifier(left))
            && !crate::utilities::is_local_name(self.emit_context, &left)
        {
            self.get_exports(left)
        } else {
            Vec::new()
        };
        if exported_names.is_empty() {
            return self.generated_visit_each_child(&node);
        }

        let mut expression = if references_direct_exported_variable {
            let right = self
                .store_for(node)
                .right(node)
                .expect("assignment expression should have a right operand");
            let right = self
                .visit_node(Some(right))
                .expect("assignment expression should keep its right operand");
            self.create_export_expression(&left_text, right)
        } else {
            self.generated_visit_each_child(&node)
        };
        for export_name in exported_names {
            if references_direct_exported_variable && export_name == left_text {
                continue;
            }
            expression = self.create_export_expression(&export_name, expression);
            if export_name == left_text
                && let Some(left) = self.store_for(expression).left(expression)
                && let Some(name) = self.store_for(left).name(left)
            {
                self.emit_context
                    .set_source_map_range(&name, self.store_for(original_left).loc(original_left));
            }
        }
        expression
    }

    fn visit_expression_statement(&mut self, node: ast::Node) -> ast::Node {
        let store = self.store_for(node);
        let expression = store
            .expression(node)
            .expect("expression statement should have an expression");
        let expression = self.visit_node_with_discarded_value(Some(expression));
        if node.store_id() == self.source.store_id() {
            let source = self.source;
            self.factory_mut()
                .update_expression_statement_from_store(source, node, expression)
        } else {
            self.factory_mut()
                .update_expression_statement(node, expression)
        }
    }

    fn visit_parameter(&mut self, node: ast::Node) -> ast::Node {
        let (modifiers_input, dot_dot_dot_token, name, initializer) = {
            let store = self.store_for(node);
            (
                store
                    .source_modifiers(node)
                    .map(ast::SourceModifierListInput::from_source),
                store.dot_dot_dot_token(node),
                store.name(node),
                store.initializer(node),
            )
        };
        let modifiers = self.visit_modifiers_input(modifiers_input);
        let dot_dot_dot_token = dot_dot_dot_token.map(|token| self.preserve_node(token));
        let name = self.visit_node(name);
        let initializer = self.visit_node(initializer);
        if node.store_id() == self.source.store_id() {
            let source = self.source;
            self.factory_mut().update_parameter_declaration_from_store(
                source,
                node,
                modifiers,
                dot_dot_dot_token,
                name,
                None,
                None,
                initializer,
            )
        } else {
            self.factory_mut().update_parameter_declaration(
                node,
                modifiers,
                dot_dot_dot_token,
                name,
                None,
                None,
                initializer,
            )
        }
    }

    fn visit_arrow_function(&mut self, node: ast::Node) -> ast::Node {
        let (modifiers_input, parameters_input, equals_greater_than_token, body, from_source) = {
            let store = self.store_for(node);
            (
                store
                    .source_modifiers(node)
                    .map(ast::SourceModifierListInput::from_source),
                store
                    .source_parameters(node)
                    .map(ast::SourceNodeListInput::from_source),
                store.equals_greater_than_token(node),
                store.body(node),
                node.store_id() == self.source.store_id(),
            )
        };
        let modifiers = self.visit_modifiers_input(modifiers_input);
        let parameters = self
            .visit_nodes_input(parameters_input)
            .expect("arrow function parameters are required");
        let equals_greater_than_token =
            equals_greater_than_token.map(|token| self.preserve_node(token));
        let body = self.visit_node(body);
        if from_source {
            let source = self.source;
            self.factory_mut().update_arrow_function_from_store(
                source,
                node,
                modifiers,
                None::<ast::NodeList>,
                parameters,
                None::<ast::Node>,
                None::<ast::Node>,
                equals_greater_than_token,
                body,
            )
        } else {
            self.factory_mut().update_arrow_function(
                node,
                modifiers,
                None::<ast::NodeList>,
                parameters,
                None::<ast::Node>,
                None::<ast::Node>,
                equals_greater_than_token,
                body,
            )
        }
    }

    // Visits a parenthesized expression whose value may be discarded at runtime.
    fn visit_parenthesized_expression(
        &mut self,
        node: ast::Node,
        result_is_discarded: bool,
    ) -> ast::Node {
        let store = self.store_for(node);
        let expression = store
            .expression(node)
            .expect("parenthesized expression should have an expression");
        let expression = if result_is_discarded {
            self.visit_node_with_discarded_value(Some(expression))
        } else {
            self.visit_node(Some(expression))
        };
        if node.store_id() == self.source.store_id() {
            let source = self.source;
            self.factory_mut()
                .update_parenthesized_expression_from_store(source, node, expression)
        } else {
            self.factory_mut()
                .update_parenthesized_expression(node, expression)
        }
    }

    // Visits a partially emitted expression whose value may be discarded at runtime.
    fn visit_partially_emitted_expression(
        &mut self,
        node: ast::Node,
        result_is_discarded: bool,
    ) -> ast::Node {
        let store = self.store_for(node);
        let expression = store
            .expression(node)
            .expect("partially emitted expression should have an expression");
        let expression = if result_is_discarded {
            self.visit_node_with_discarded_value(Some(expression))
        } else {
            self.visit_node(Some(expression))
        };
        if node.store_id() == self.source.store_id() {
            let source = self.source;
            self.factory_mut()
                .update_partially_emitted_expression_from_store(source, node, expression)
        } else {
            self.factory_mut()
                .update_partially_emitted_expression(node, expression)
        }
    }

    fn visit_binary_expression(&mut self, node: ast::Node) -> ast::Node {
        let store = self.store_for(node);
        if ast::is_destructuring_assignment(store, node) {
            return self.visit_destructuring_assignment(node, false);
        }
        if ast::is_assignment_expression(store, node, false) {
            return self.visit_assignment_expression(node);
        }
        if ast::is_comma_sequence(store, node) {
            return self.visit_comma_expression(node, false);
        }
        self.generated_visit_each_child(&node)
    }

    // Visits a destructuring assignment which might target an exported identifier.
    fn visit_destructuring_assignment(
        &mut self,
        node: ast::Node,
        value_is_discarded: bool,
    ) -> ast::Node {
        let left = self
            .store_for(node)
            .left(node)
            .expect("destructuring assignment should have a left operand");
        if self.destructuring_needs_flattening(left) {
            let source = self.source;
            let direct_exported_names = self.direct_exported_names;
            let local_export_specifiers = self.local_export_specifiers;
            let mut import_state = ast::AstImportState::new();
            let mut create_all_export_expressions =
                |emit_context: &mut printer::EmitContext,
                 name: ast::Node,
                 value: ast::Node,
                 location: core::TextRange| {
                    let text = ast::AstImportState::store_for(
                        source,
                        &emit_context.factory.node_factory,
                        name,
                    )
                    .text(name);
                    let mut exported_names = Vec::new();
                    if direct_exported_names.contains(&text) {
                        exported_names.push(text.clone());
                    }
                    for export_name in local_export_specifiers
                        .get(&text)
                        .cloned()
                        .unwrap_or_default()
                    {
                        if !exported_names
                            .iter()
                            .any(|exported| exported == &export_name)
                        {
                            exported_names.push(export_name);
                        }
                    }
                    if exported_names.is_empty() {
                        return value;
                    }

                    let mut expression = if direct_exported_names.contains(&text) {
                        let export_name = emit_context.factory.node_factory.new_identifier(&text);
                        emit_context.mark_emit_node(
                            &export_name,
                            printer::EF_NO_COMMENTS | printer::EF_NO_SOURCE_MAP,
                        );
                        let property_access = exports_property_access_for_name(
                            &mut emit_context.factory.node_factory,
                            export_name,
                        );
                        emit_context.mark_emit_node(&property_access, printer::EF_NO_COMMENTS);
                        let expression = emit_context
                            .factory
                            .new_assignment_expression(property_access, value);
                        emit_context.assign_comment_and_source_map_ranges(&expression, &name);
                        expression
                    } else {
                        let name = import_state.preserve_node(
                            source,
                            &mut emit_context.factory.node_factory,
                            name,
                        );
                        emit_context.factory.new_assignment_expression(name, value)
                    };

                    for export_name in exported_names {
                        if direct_exported_names.contains(&text) && export_name == text {
                            continue;
                        }
                        let export_name = emit_context
                            .factory
                            .node_factory
                            .new_identifier(&export_name);
                        let left = exports_property_access_for_name(
                            &mut emit_context.factory.node_factory,
                            export_name,
                        );
                        expression = emit_context
                            .factory
                            .new_assignment_expression(left, expression);
                        emit_context.set_comment_range(&expression, location);
                    }
                    expression
                };
            return crate::destructuring::flatten_destructuring_assignment(
                source,
                self.emit_context,
                node,
                !value_is_discarded,
                crate::destructuring::FlattenLevel::All,
                Some(&mut create_all_export_expressions),
            );
        }
        self.generated_visit_each_child(&node)
    }

    // Visits a comma expression whose left-hand value is always discard, and whose right-hand value may be discarded at runtime.
    fn visit_comma_expression(&mut self, node: ast::Node, result_is_discarded: bool) -> ast::Node {
        let store = self.store_for(node);
        let left = store
            .left(node)
            .expect("comma expression should have a left operand");
        let right = store
            .right(node)
            .expect("comma expression should have a right operand");
        let operator_token = store
            .operator_token(node)
            .expect("comma expression should have an operator token");
        let left = self
            .visit_node_with_discarded_value(Some(left))
            .unwrap_or(left);
        let right = if result_is_discarded {
            self.visit_node_with_discarded_value(Some(right))
        } else {
            self.visit_node(Some(right))
        }
        .unwrap_or(right);
        let operator_token = self.preserve_node(operator_token);
        if node.store_id() == self.source.store_id() {
            let source = self.source;
            self.factory_mut().update_binary_expression_from_store(
                source,
                node,
                None::<ast::ModifierList>,
                left,
                None::<ast::Node>,
                operator_token,
                right,
            )
        } else {
            self.factory_mut().update_binary_expression(
                node,
                None::<ast::ModifierList>,
                left,
                None::<ast::Node>,
                operator_token,
                right,
            )
        }
    }

    // Visits a prefix unary expression that might modify an exported identifier.
    fn visit_prefix_unary_expression(
        &mut self,
        node: ast::Node,
        _result_is_discarded: bool,
    ) -> ast::Node {
        // When we see a prefix increment expression whose operand is an exported
        // symbol, we should ensure all exports of that symbol are updated with the correct
        // value.
        //
        // - We do not transform generated identifiers for any reason.
        // - We do not transform identifiers tagged with the LocalName flag.
        // - We do not transform identifiers that were originally the name of an enum or
        //   namespace due to how they are transformed in TypeScript.
        // - We only transform identifiers that are exported at the top level.
        let store = self.store_for(node);
        let operator = store
            .operator(node)
            .expect("prefix unary expression should have an operator");
        let operand = store
            .operand(node)
            .expect("prefix unary expression should have an operand");
        if matches!(
            operator,
            ast::Kind::PlusPlusToken | ast::Kind::MinusMinusToken
        ) && ast::is_identifier(self.store_for(operand), operand)
            && !crate::utilities::is_local_name(self.emit_context, &operand)
        {
            let exported_names = self.get_exports(operand);
            if !exported_names.is_empty() {
                // given:
                //   var x = 0;
                //   export { x }
                //   ++x;
                // emits:
                //   var x = 0;
                //   exports.x = x;
                //   exports.x = ++x;
                // note:
                //   after the operation, `exports.x` will hold the value of `x` after the increment.

                let visited_operand = self.visit_node(Some(operand));
                let mut expression = if node.store_id() == self.source.store_id() {
                    let source = self.source;
                    self.factory_mut()
                        .update_prefix_unary_expression_from_store(
                            source,
                            node,
                            operator,
                            visited_operand,
                        )
                } else {
                    self.factory_mut().update_prefix_unary_expression(
                        node,
                        operator,
                        visited_operand,
                    )
                };
                for export_name in exported_names {
                    expression = self.create_export_expression(&export_name, expression);
                    self.emit_context
                        .assign_comment_and_source_map_ranges(&expression, &node);
                }
                return expression;
            }
        }
        self.generated_visit_each_child(&node)
    }

    // Visits a postfix unary expression that might modify an exported identifier.
    fn visit_postfix_unary_expression(
        &mut self,
        node: ast::Node,
        result_is_discarded: bool,
    ) -> ast::Node {
        // When we see a postfix increment expression whose operand is an exported
        // symbol, we should ensure all exports of that symbol are updated with the correct
        // value.
        //
        // - We do not transform generated identifiers for any reason.
        // - We do not transform identifiers tagged with the LocalName flag.
        // - We do not transform identifiers that were originally the name of an enum or
        //   namespace due to how they are transformed in TypeScript.
        // - We only transform identifiers that are exported at the top level.
        let store = self.store_for(node);
        let operator = store
            .operator(node)
            .expect("postfix unary expression should have an operator");
        let operand = store
            .operand(node)
            .expect("postfix unary expression should have an operand");
        if matches!(
            operator,
            ast::Kind::PlusPlusToken | ast::Kind::MinusMinusToken
        ) && ast::is_identifier(self.store_for(operand), operand)
            && !crate::utilities::is_local_name(self.emit_context, &operand)
        {
            let exported_names = self.get_exports(operand);
            if !exported_names.is_empty() {
                // given (value is discarded):
                //   var x = 0;
                //   export { x }
                //   x++;
                // emits:
                //   var x = 0, y;
                //   exports.x = x;
                //   exports.x = (x++, x);
                // note:
                //   after the operation, `exports.x` will hold the value of `x` after the increment.
                //
                // given (value is not discarded):
                //   var x = 0, y;
                //   export { x }
                //   y = x++;
                // emits:
                //   var _a;
                //   var x = 0, y;
                //   exports.x = x;
                //   y = (exports.x = (_a = x++, x), _a);
                // note:
                //   after the operation, `exports.x` will hold the value of `x` after the increment, while
                //   `y` will hold the value of `x` before the increment.

                let mut temp = None;
                let visited_operand = self.visit_node(Some(operand));
                let mut expression = if node.store_id() == self.source.store_id() {
                    let source = self.source;
                    self.factory_mut()
                        .update_postfix_unary_expression_from_store(
                            source,
                            node,
                            visited_operand,
                            operator,
                        )
                } else {
                    self.factory_mut().update_postfix_unary_expression(
                        node,
                        visited_operand,
                        operator,
                    )
                };
                if !result_is_discarded {
                    let temp_node = self.emit_context.factory.new_temp_variable();
                    self.emit_context.add_variable_declaration(temp_node);

                    expression = self
                        .emit_context
                        .factory
                        .new_assignment_expression(temp_node, expression);
                    self.emit_context
                        .assign_comment_and_source_map_ranges(&expression, &node);
                    temp = Some(temp_node);
                }

                let operand = self.preserve_node(operand);
                expression = self
                    .emit_context
                    .factory
                    .new_comma_expression(expression, operand);
                self.emit_context
                    .assign_comment_and_source_map_ranges(&expression, &node);

                for export_name in exported_names {
                    expression = self.create_export_expression(&export_name, expression);
                    self.emit_context
                        .assign_comment_and_source_map_ranges(&expression, &node);
                }

                if let Some(temp) = temp {
                    expression = self
                        .emit_context
                        .factory
                        .new_comma_expression(expression, temp);
                    self.emit_context
                        .assign_comment_and_source_map_ranges(&expression, &node);
                }

                return expression;
            }
        }
        self.generated_visit_each_child(&node)
    }

    fn should_transform_import_call(&self) -> bool {
        ast::should_transform_import_call(self.file_name, self.compiler_options, self.module_format)
    }

    fn should_rewrite_import_or_require_call(&self, node: ast::Node) -> bool {
        let store = self.store_for(node);
        self.compiler_options
            .rewrite_relative_import_extensions
            .is_true()
            && store
                .arguments(node)
                .is_some_and(|arguments| !arguments.is_empty())
            && (ast::is_import_call(store, node)
                || (ast::is_in_js_file(store, node) && ast::is_require_call(store, node, false)))
    }

    fn rewrite_module_specifier(&mut self, argument: ast::Node) -> ast::Node {
        let store = self.store_for(argument);
        let Some(text) = crate::moduletransforms::utilities::rewrite_module_specifier_text(
            &store.text(argument),
            self.compiler_options,
        ) else {
            return argument;
        };
        let token_flags = store.token_flags(argument).unwrap_or(ast::TokenFlags::NONE);
        self.factory_mut().new_string_literal(text, token_flags)
    }

    fn rewrite_import_or_require_argument(
        &mut self,
        argument: ast::Node,
        visit_non_literal: bool,
    ) -> ast::Node {
        let store = self.store_for(argument);
        if ast::is_string_literal_like(store, argument) {
            return self.rewrite_module_specifier(argument);
        }
        let visited = if visit_non_literal {
            self.visit_node(Some(argument)).unwrap_or(argument)
        } else {
            self.preserve_node(argument)
        };
        let preserve_jsx = self.compiler_options.jsx == core::JsxEmit::Preserve;
        self.emit_context
            .factory
            .new_rewrite_relative_import_extensions_helper(visited, preserve_jsx)
    }

    fn visit_import_or_require_call(&mut self, node: ast::Node) -> ast::Node {
        let store = self.store_for(node);
        let source_expression = store.expression(node);
        let question_dot_token = store.question_dot_token(node);
        let type_arguments_input = store
            .type_arguments(node)
            .map(ast::SourceNodeListInput::from_source);
        let source_arguments = store
            .arguments(node)
            .expect("call expression should have arguments");
        let arguments_loc = source_arguments.loc();
        let arguments_range = source_arguments.range();
        let arguments_has_trailing_comma = source_arguments.has_trailing_comma();
        let source_arguments = source_arguments.iter().collect::<Vec<_>>();
        let flags = store.flags(node);

        let expression = self.visit_node(source_expression);
        let question_dot_token = question_dot_token.map(|token| self.preserve_node(token));
        let type_arguments = self.visit_nodes_input(type_arguments_input);
        let mut arguments = Vec::with_capacity(source_arguments.len());
        let mut iter = source_arguments.into_iter();
        if let Some(first_argument) = iter.next() {
            arguments.push(self.rewrite_import_or_require_argument(first_argument, false));
        }
        for argument in iter {
            if let Some(argument) = self.visit_node(Some(argument)) {
                arguments.push(argument);
            }
        }
        let arguments = self.factory_mut().new_node_list_with_trailing_comma(
            arguments_loc,
            arguments_range,
            arguments,
            arguments_has_trailing_comma,
        );
        if node.store_id() == self.source.store_id() {
            let source = self.source;
            self.factory_mut().update_call_expression_from_store(
                source,
                node,
                expression,
                question_dot_token,
                type_arguments,
                arguments,
                flags,
            )
        } else {
            self.factory_mut().update_call_expression(
                node,
                expression,
                question_dot_token,
                type_arguments,
                arguments,
                flags,
            )
        }
    }

    // Visits a call expression that might reference an imported symbol and thus require an indirect call, or that might
    // be an `import()` or `require()` call that may need to be rewritten.
    fn visit_call_expression(&mut self, node: ast::Node) -> ast::Node {
        let source_expression = self
            .store_for(node)
            .expression(node)
            .expect("call expression should have an expression");
        let source_expression_is_identifier =
            ast::is_identifier(self.store_for(source_expression), source_expression);
        let question_dot_token = self.store_for(node).question_dot_token(node);
        let type_argument_parts = self.store_for(node).type_arguments(node).map(|list| {
            (
                list.loc(),
                list.range(),
                list.has_trailing_comma(),
                list.iter().collect::<Vec<_>>(),
            )
        });
        let argument_parts = {
            let arguments = self
                .store_for(node)
                .arguments(node)
                .expect("call expression should have arguments");
            (
                arguments.loc(),
                arguments.range(),
                arguments.has_trailing_comma(),
                arguments.iter().collect::<Vec<_>>(),
            )
        };
        let flags = self.store_for(node).flags(node);
        let mut expression =
            if ast::is_identifier(self.store_for(source_expression), source_expression) {
                self.visit_expression_identifier(source_expression)
            } else {
                self.visit_node(Some(source_expression))
                    .unwrap_or(source_expression)
            };
        let expression_needs_indirect_call = source_expression_is_identifier
            && !ast::is_identifier(self.store_for(expression), expression)
            && !crate::utilities::is_helper_name(self.emit_context, &source_expression);
        let expression_was_start_on_new_line =
            self.emit_context.emit_flags(&node) & printer::EF_START_ON_NEW_LINE != 0;
        if expression_needs_indirect_call && expression_was_start_on_new_line {
            let zero = self
                .factory_mut()
                .new_numeric_literal("0", ast::TokenFlags::NONE);
            expression = self
                .emit_context
                .factory
                .new_comma_expression(zero, expression);
        }
        let question_dot_token = question_dot_token.map(|token| self.preserve_node(token));
        let type_arguments = type_argument_parts.map(|(loc, range, has_trailing_comma, nodes)| {
            let mut visited = Vec::with_capacity(nodes.len());
            for node in nodes {
                if let Some(node) = self.visit_node(Some(node)) {
                    visited.push(self.preserve_node(node));
                }
            }
            self.factory_mut().new_node_list_with_trailing_comma(
                loc,
                range,
                visited,
                has_trailing_comma,
            )
        });
        let (loc, range, has_trailing_comma, nodes) = argument_parts;
        let mut arguments = Vec::with_capacity(nodes.len());
        for node in nodes {
            if let Some(node) = self.visit_node(Some(node)) {
                arguments.push(self.preserve_node(node));
            }
        }
        let arguments = self.factory_mut().new_node_list_with_trailing_comma(
            loc,
            range,
            arguments,
            has_trailing_comma,
        );
        let updated = if node.store_id() == self.source.store_id() {
            let source = self.source;
            self.factory_mut().update_call_expression_from_store(
                source,
                node,
                expression,
                question_dot_token,
                type_arguments,
                arguments,
                flags,
            )
        } else {
            self.factory_mut().update_call_expression(
                node,
                expression,
                question_dot_token,
                type_arguments,
                arguments,
                flags,
            )
        };
        if expression_needs_indirect_call && !expression_was_start_on_new_line {
            self.emit_context
                .mark_emit_node(&updated, printer::EF_INDIRECT_CALL);
            let original = self.emit_context.most_original(&updated);
            if original != updated {
                self.emit_context
                    .mark_emit_node(&original, printer::EF_INDIRECT_CALL);
            }
        }
        updated
    }

    fn create_import_call_expression_common_js(&mut self, arg: Option<ast::Node>) -> ast::Node {
        let need_sync_eval = arg.is_some_and(|arg| {
            !crate::moduletransforms::utilities::is_simple_inlineable_expression(
                self.store_for(arg).kind(arg),
                ast::is_identifier(self.store_for(arg), arg),
            )
        });

        let promise_resolve_arguments = if need_sync_eval {
            let arg = arg.expect("sync dynamic import should have argument");
            let arg = self.preserve_node(arg);
            let head = self
                .factory_mut()
                .new_template_head("", "", ast::TokenFlags::NONE);
            let tail = self
                .factory_mut()
                .new_template_tail("", "", ast::TokenFlags::NONE);
            let span = self.factory_mut().new_template_span(arg, tail);
            let spans = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                vec![span],
            );
            let template = self.factory_mut().new_template_expression(head, spans);
            vec![template]
        } else {
            Vec::new()
        };
        let promise = self.factory_mut().new_identifier("Promise");
        let resolve = self.factory_mut().new_identifier("resolve");
        let promise_resolve = self.factory_mut().new_property_access_expression(
            promise,
            None::<ast::Node>,
            resolve,
            ast::NodeFlags::NONE,
        );
        let promise_args = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            promise_resolve_arguments,
        );
        let promise_resolve_call = self.factory_mut().new_call_expression(
            promise_resolve,
            None::<ast::Node>,
            None::<ast::NodeList>,
            promise_args,
            ast::NodeFlags::NONE,
        );

        let require_arguments = if need_sync_eval {
            vec![self.factory_mut().new_identifier("s")]
        } else {
            arg.into_iter().map(|arg| self.preserve_node(arg)).collect()
        };
        let require = self.factory_mut().new_identifier("require");
        let require_args = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            require_arguments,
        );
        let require_call = self.factory_mut().new_call_expression(
            require,
            None::<ast::Node>,
            None::<ast::NodeList>,
            require_args,
            ast::NodeFlags::NONE,
        );
        let require_call = self
            .emit_context
            .factory
            .new_import_star_helper(require_call);

        let parameters = if need_sync_eval {
            let name = self.factory_mut().new_identifier("s");
            let parameter = self.factory_mut().new_parameter_declaration(
                None::<ast::ModifierList>,
                None::<ast::Node>,
                name,
                None::<ast::Node>,
                None::<ast::Node>,
                None::<ast::Node>,
            );
            vec![parameter]
        } else {
            Vec::new()
        };
        let parameters = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            parameters,
        );
        let equals = self
            .factory_mut()
            .new_token(ast::Kind::EqualsGreaterThanToken);
        let function = self.factory_mut().new_arrow_function(
            None::<ast::ModifierList>,
            None::<ast::NodeList>,
            parameters,
            None::<ast::Node>,
            None::<ast::Node>,
            equals,
            require_call,
        );

        let then = self.factory_mut().new_identifier("then");
        let then_access = self.factory_mut().new_property_access_expression(
            promise_resolve_call,
            None::<ast::Node>,
            then,
            ast::NodeFlags::NONE,
        );
        let then_args = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![function],
        );
        self.factory_mut().new_call_expression(
            then_access,
            None::<ast::Node>,
            None::<ast::NodeList>,
            then_args,
            ast::NodeFlags::NONE,
        )
    }

    // Visits a shorthand property assignment that might reference an imported or exported symbol.
    fn visit_shorthand_property_assignment(&mut self, node: ast::Node) -> ast::Node {
        let node_is_output = node.store_id() == self.factory().store().store_id();
        let name = self
            .store_for(node)
            .name(node)
            .expect("shorthand property assignment should have name");
        let exported_or_imported_name = self.visit_expression_identifier(name);
        if !self.preserved_source_node_matches(Some(name), Some(exported_or_imported_name)) {
            // A shorthand property with an assignment initializer is probably part of a
            // destructuring assignment
            let mut expression = exported_or_imported_name;
            if let Some(initializer) = self.store_for(node).object_assignment_initializer(node) {
                let initializer = self
                    .visit_node(Some(initializer))
                    .expect("shorthand property initializer should visit to an expression");
                expression = self
                    .emit_context
                    .factory
                    .new_assignment_expression(expression, initializer);
            }
            let name = self.preserve_node(name);
            let node_loc = self.store_for(node).loc(node);
            let assignment = self
                .factory_mut()
                .new_property_assignment(None, name, None, None, expression);
            self.factory_mut()
                .place_transformed_node(assignment, node_loc);
            self.emit_context.set_original(&assignment, &node);
            self.emit_context
                .assign_comment_and_source_map_ranges(&assignment, &node);
            return assignment;
        }

        let name = self.preserve_node(exported_or_imported_name);
        let equals_token = self
            .store_for(node)
            .equals_token(node)
            .map(|token| self.preserve_node(token));
        let initializer = self.visit_node(self.store_for(node).object_assignment_initializer(node));
        if node_is_output {
            self.factory_mut().update_shorthand_property_assignment(
                node,
                None::<ast::ModifierList>,
                name,
                None::<ast::Node>,
                None::<ast::Node>,
                equals_token,
                initializer,
            )
        } else {
            let source = self.source;
            self.factory_mut()
                .update_shorthand_property_assignment_from_store(
                    source,
                    node,
                    None::<ast::ModifierList>,
                    name,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    equals_token,
                    initializer,
                )
        }
    }

    // Visits an identifier that, if it is in an expression position, might reference an imported or exported symbol.
    fn visit_identifier(&mut self, node: ast::Node) -> ast::Node {
        if let Some(parent) = self.parent_node
            && crate::utilities::is_identifier_reference(self.store_for(parent), &node, parent)
        {
            return self.visit_expression_identifier(node);
        }
        self.preserve_node(node)
    }

    // Visits an identifier in an expression position that might reference an imported or exported symbol.
    fn visit_expression_identifier(&mut self, node: ast::Node) -> ast::Node {
        let original = self.emit_context.most_original(&node);
        let original_store = self.emit_context.store_for_node(original);
        let original_loc = original_store.loc(original);
        let (text, references_source_file_export) = {
            let store = self.store_for(node);
            (
                store.text(node),
                self.module_transform_facts
                    .references_source_file_export(original_store, original),
            )
        };
        let has_import_reference = self
            .module_transform_facts
            .referenced_import_references
            .contains_key(&original_loc)
            || self.import_references.contains_key(&text);
        let is_export_name = crate::utilities::is_export_name(self.emit_context, &node);
        if self.can_rewrite_exported_identifier_reference(node)
            && (is_export_name
                || references_source_file_export
                || has_import_reference && self.direct_exported_names.contains(&text))
        {
            let loc = self.store_for(node).loc(node);
            let name = if node.store_id() == self.factory().store().store_id() {
                self.factory_mut()
                    .deep_clone_node_in_current_store_preserve_location(node)
            } else {
                let source = self.source;
                self.factory_mut()
                    .deep_clone_node_from_store_preserve_location(source, node)
            };
            let reference = exports_property_access_for_name(self.factory_mut(), name);
            self.emit_context
                .assign_comment_and_source_map_ranges(&reference, &node);
            self.factory_mut().place_transformed_node(reference, loc);
            return reference;
        }
        if let Some(reference) = self
            .module_transform_facts
            .referenced_import_references
            .get(&original_loc)
            .copied()
        {
            return self.imported_reference_expression_from_identifier(reference, node);
        }
        if self.can_rewrite_emitted_import_identifier_reference(node)
            && let Some(reference) = self.import_references.get(&text).copied()
        {
            return self.imported_reference_expression_from_identifier(reference, node);
        }
        if self.can_rewrite_generated_import_identifier_reference(node)
            && let Some(reference) = self.import_references.get(&text).copied()
        {
            return self.imported_reference_expression_from_identifier(reference, node);
        }
        if self.is_metadata_helper_reference(node)
            && let Some(reference) = self.import_references.get(&text).copied()
        {
            return self.imported_reference_expression_from_identifier(reference, node);
        }
        self.preserve_node(node)
    }

    fn is_metadata_helper_reference(&self, node: ast::Node) -> bool {
        let mut current = Some(node);
        while let Some(child) = current {
            let store = self.store_for(child);
            let Some(parent) = store.parent(child) else {
                return false;
            };
            let parent_store = self.store_for(parent);
            if parent_store.kind(parent) == ast::Kind::CallExpression
                && let Some(expression) = parent_store.expression(parent)
            {
                let expression_store = self.store_for(expression);
                if ast::is_identifier(expression_store, expression)
                    && expression_store.text(expression) == "__metadata"
                {
                    return true;
                }
            }
            current = Some(parent);
        }
        false
    }

    fn visit_node_with_discarded_value(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let node = node?;
        let store = self.store_for(node);
        if store.kind(node) == ast::Kind::BinaryExpression && ast::is_comma_sequence(store, node) {
            let grandparent_node = self.push_node(node);
            let visited = self.visit_comma_expression(node, true);
            self.pop_node(grandparent_node);
            return Some(visited);
        }
        if store.kind(node) == ast::Kind::ParenthesizedExpression {
            let grandparent_node = self.push_node(node);
            let visited = self.visit_parenthesized_expression(node, true);
            self.pop_node(grandparent_node);
            return Some(visited);
        }
        if store.kind(node) == ast::Kind::PartiallyEmittedExpression {
            let grandparent_node = self.push_node(node);
            let visited = self.visit_partially_emitted_expression(node, true);
            self.pop_node(grandparent_node);
            return Some(visited);
        }
        if store.kind(node) == ast::Kind::BinaryExpression
            && ast::is_destructuring_assignment(store, node)
        {
            let grandparent_node = self.push_node(node);
            let visited = self.visit_destructuring_assignment(node, true);
            self.pop_node(grandparent_node);
            return Some(visited);
        }
        if store.kind(node) == ast::Kind::PrefixUnaryExpression {
            let grandparent_node = self.push_node(node);
            let visited = self.visit_prefix_unary_expression(node, true);
            self.pop_node(grandparent_node);
            return Some(visited);
        }
        if store.kind(node) == ast::Kind::PostfixUnaryExpression {
            let grandparent_node = self.push_node(node);
            let visited = self.visit_postfix_unary_expression(node, true);
            self.pop_node(grandparent_node);
            return Some(visited);
        }
        self.visit_node(Some(node))
    }
}

impl<'source> ast::AstVisitEachChildRuntime<'source> for CommonJsReferenceRewriter<'_, 'source> {
    fn source_store(&self) -> &ast::AstStore {
        self.source
    }

    fn factory(&self) -> &ast::NodeFactory {
        &self.emit_context.factory.node_factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.emit_context.factory.node_factory
    }

    fn preserved_node(&self, source: ast::Node) -> Option<ast::Node> {
        self.import_state.preserved_node(self.factory(), source)
    }

    fn preserve_node(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
            node
        } else {
            self.preserve_source_node(node)
        }
    }

    fn record_preserved_node(&mut self, source: ast::Node, imported: ast::Node) -> ast::Node {
        let imported = self.preserve_node(imported);
        let mut import_state = std::mem::take(&mut self.import_state);
        let recorded = import_state.record_preserved_node(
            source.store_id(),
            &mut self.emit_context.factory.node_factory,
            source,
            imported,
        );
        self.import_state = import_state;
        recorded
    }

    fn preserved_source_node_matches(
        &self,
        source: Option<ast::Node>,
        output: Option<ast::Node>,
    ) -> bool {
        self.import_state
            .preserved_source_node_matches(self.factory(), source, output)
    }

    fn update_source_file_from_visited(
        &mut self,
        node: ast::Node,
        statements: Option<ast::NodeList>,
        end_of_file_token: Option<ast::Node>,
        source_unchanged: bool,
    ) -> ast::Node {
        if source_unchanged {
            let imported = self.preserve_source_node(node);
            return self.record_preserved_node(node, imported);
        }
        let mut import_state = std::mem::take(&mut self.import_state);
        let updated = import_state.update_source_file_from_store(
            self.source,
            &mut self.emit_context.factory.node_factory,
            node,
            statements,
            end_of_file_token,
        );
        self.import_state = import_state;
        updated
    }

    fn visit_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let node = node?;
        let grandparent_node = self.push_node(node);
        let store = self.store_for(node);
        let visited = if store.kind(node) == ast::Kind::VariableStatement
            && ast::has_syntactic_modifier(store, node, ast::ModifierFlags::EXPORT)
        {
            let statements = if node.store_id() == self.factory().store().store_id() {
                transform_exported_variable_statement_active(
                    self.source,
                    self.emit_context,
                    node,
                    self.emit_context.most_original(&node),
                    self.import_references,
                    self.compiler_options,
                    self.file_name,
                    self.module_format,
                    self.source_file_contains_dynamic_import,
                    self.direct_exported_names,
                    self.local_export_specifiers,
                    self.module_transform_facts,
                )
            } else {
                transform_exported_variable_statement(
                    self.source,
                    self.emit_context,
                    &node,
                    None,
                    self.import_references,
                    self.compiler_options,
                    self.file_name,
                    self.module_format,
                    self.source_file_contains_dynamic_import,
                    self.direct_exported_names,
                    self.local_export_specifiers,
                    self.module_transform_facts,
                )
            };
            single_or_syntax_list(self.factory_mut(), statements)
        } else if store.kind(node) == ast::Kind::ExpressionStatement {
            Some(self.visit_expression_statement(node))
        } else if store.kind(node) == ast::Kind::BinaryExpression {
            Some(self.visit_binary_expression(node))
        } else if store.kind(node) == ast::Kind::ParenthesizedExpression {
            Some(self.visit_parenthesized_expression(node, false))
        } else if store.kind(node) == ast::Kind::PartiallyEmittedExpression {
            Some(self.visit_partially_emitted_expression(node, false))
        } else if store.kind(node) == ast::Kind::PrefixUnaryExpression {
            Some(self.visit_prefix_unary_expression(node, false))
        } else if store.kind(node) == ast::Kind::PostfixUnaryExpression {
            Some(self.visit_postfix_unary_expression(node, false))
        } else if store.kind(node) == ast::Kind::CallExpression
            && ast::is_import_call(store, node)
            && self.should_transform_import_call()
            && commonjsmodule::should_lower_dynamic_import(
                self.compiler_options.get_emit_module_kind(),
                self.compiler_options.get_emit_script_target(),
            )
        {
            let rewrite_or_shim = self.should_rewrite_import_or_require_call(node);
            let first_argument = self
                .store_for(node)
                .arguments(node)
                .and_then(|arguments| arguments.iter().next())
                .and_then(|argument| {
                    if rewrite_or_shim {
                        Some(self.rewrite_import_or_require_argument(argument, true))
                    } else {
                        self.visit_node(Some(argument))
                    }
                });
            Some(self.create_import_call_expression_common_js(first_argument))
        } else if store.kind(node) == ast::Kind::CallExpression
            && self.should_rewrite_import_or_require_call(node)
        {
            Some(self.visit_import_or_require_call(node))
        } else if store.kind(node) == ast::Kind::CallExpression {
            Some(self.visit_call_expression(node))
        } else if store.kind(node) == ast::Kind::ArrowFunction {
            Some(self.visit_arrow_function(node))
        } else if store.kind(node) == ast::Kind::ShorthandPropertyAssignment {
            Some(self.visit_shorthand_property_assignment(node))
        } else if store.kind(node) == ast::Kind::Parameter {
            Some(self.visit_parameter(node))
        } else if store.kind(node) == ast::Kind::Identifier {
            Some(self.visit_identifier(node))
        } else if store.kind(node) == ast::Kind::PropertyAccessExpression {
            let expression = self.visit_node(store.expression(node));
            let question_dot_token = self
                .store_for(node)
                .question_dot_token(node)
                .map(|token| self.preserve_node(token));
            let name = self.preserve_node(
                self.store_for(node)
                    .name(node)
                    .expect("property access expression should have a name"),
            );
            let flags = self.store_for(node).flags(node);
            if node.store_id() == self.source.store_id() {
                let source = self.source;
                Some(
                    self.factory_mut()
                        .update_property_access_expression_from_store(
                            source,
                            node,
                            expression,
                            question_dot_token,
                            Some(name),
                            flags,
                        ),
                )
            } else {
                Some(self.factory_mut().update_property_access_expression(
                    node,
                    expression,
                    question_dot_token,
                    Some(name),
                    flags,
                ))
            }
        } else {
            Some(self.generated_visit_each_child(&node))
        };
        self.pop_node(grandparent_node);
        visited
    }

    fn visit_token(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        self.visit_node(node)
    }

    fn visit_nodes_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        let nodes = nodes?;
        let source_list = nodes.clone();
        let mut visited = Vec::with_capacity(source_list.len());
        let mut changed = false;
        for node in source_list.iter() {
            let result = self.visit_node(Some(node));
            self.append_visited_node(node, result, &mut visited, &mut changed);
        }
        if changed {
            Some(self.factory_mut().new_node_list_with_trailing_comma(
                source_list.loc(),
                source_list.range(),
                visited,
                source_list.has_trailing_comma(),
            ))
        } else {
            let mut import_state = std::mem::take(&mut self.import_state);
            let list = import_state.preserve_source_node_list_input(
                self.source,
                &mut self.emit_context.factory.node_factory,
                &nodes,
            );
            self.import_state = import_state;
            Some(list)
        }
    }

    fn visit_modifiers_input(
        &mut self,
        modifiers: Option<ast::SourceModifierListInput>,
    ) -> Option<ast::ModifierList> {
        let modifiers = modifiers?;
        let mut import_state = std::mem::take(&mut self.import_state);
        let list = import_state.preserve_source_modifier_list_input(
            self.source,
            &mut self.emit_context.factory.node_factory,
            &modifiers,
        );
        self.import_state = import_state;
        Some(list)
    }

    fn visit_parameters_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        self.visit_nodes_input(nodes)
    }

    fn visit_function_body(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        self.visit_node(node)
    }

    fn visit_iteration_body(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        self.visit_embedded_statement(node)
    }

    fn visit_top_level_statements_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        self.visit_nodes_input(nodes)
    }

    fn visit_embedded_statement(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let node = node?;
        let visited = self.visit_node(Some(node));
        let mut import_state = std::mem::take(&mut self.import_state);
        let lifted = import_state.lift_to_block(
            self.source,
            &mut self.emit_context.factory.node_factory,
            visited,
        );
        self.import_state = import_state;
        lifted
    }

    fn visit_raw_node_slice_input(
        &mut self,
        nodes: Option<ast::SourceRawNodeSliceInput>,
    ) -> Option<ast::RawNodeSlice> {
        let nodes = nodes?;
        let mut import_state = std::mem::take(&mut self.import_state);
        let list = import_state.preserve_source_raw_node_slice_input(
            self.source,
            &mut self.emit_context.factory.node_factory,
            &nodes,
        );
        self.import_state = import_state;
        Some(list)
    }
}

impl<'source> ast::AstGeneratedVisitEachChild<'source> for CommonJsReferenceRewriter<'_, 'source> {}

struct TopLevelNestedCommonJsVisitor<'a, 'source> {
    source: &'source ast::AstStore,
    emit_context: &'a mut printer::EmitContext,
    import_references: &'a HashMap<String, ImportReference>,
    compiler_options: &'a core::CompilerOptions,
    file_name: &'a str,
    module_format: core::ModuleKind,
    source_file_contains_dynamic_import: bool,
    local_export_specifiers: &'a HashMap<String, Vec<String>>,
    direct_exported_names: &'a HashSet<String>,
    module_transform_facts: &'a ModuleTransformFacts,
    handled_exported_names: &'a mut HashSet<String>,
}

impl TopLevelNestedCommonJsVisitor<'_, '_> {
    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.emit_context.factory.node_factory
    }

    fn preserve_node(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.emit_context.factory.node_factory.store().store_id() {
            node
        } else {
            let mut importer =
                ast::AstImporter::new(self.source, &mut self.emit_context.factory.node_factory);
            importer.preserve_node(node)
        }
    }

    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        if node.store_id() == self.source.store_id() {
            self.source
        } else {
            self.emit_context.factory.node_factory.store()
        }
    }

    fn visit_node(&mut self, node: ast::Node) -> ast::Node {
        rewrite_common_js_statement(
            self.source,
            self.emit_context,
            self.import_references,
            self.direct_exported_names,
            self.local_export_specifiers,
            self.module_transform_facts,
            self.compiler_options,
            self.file_name,
            self.module_format,
            self.source_file_contains_dynamic_import,
            node,
        )
    }

    fn visit_optional_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        node.map(|node| self.visit_node(node))
    }

    fn visit_top_level_nested_statement(&mut self, node: ast::Node) -> Vec<ast::Node> {
        let store = self.store_for(node);
        if ast::is_variable_statement(store, node) {
            return self.visit_top_level_nested_variable_statement(node);
        }
        if ast::is_for_statement(store, node) {
            return self.visit_top_level_nested_for_statement(node);
        }
        if ast::is_for_in_or_of_statement(store, Some(node)) {
            return vec![self.visit_top_level_nested_for_in_or_of_statement(node)];
        }
        if ast::is_do_statement(store, node) {
            return vec![self.visit_top_level_nested_do_statement(node)];
        }
        if ast::is_while_statement(store, node) {
            return vec![self.visit_top_level_nested_while_statement(node)];
        }
        if ast::is_labeled_statement(store, node) {
            return vec![self.visit_top_level_nested_labeled_statement(node)];
        }
        if ast::is_with_statement(store, node) {
            return vec![self.visit_top_level_nested_with_statement(node)];
        }
        if ast::is_if_statement(store, node) {
            return vec![self.visit_top_level_nested_if_statement(node)];
        }
        if ast::is_switch_statement(store, node) {
            return vec![self.visit_top_level_nested_switch_statement(node)];
        }
        if ast::is_try_statement(store, node) {
            return vec![self.visit_top_level_nested_try_statement(node)];
        }
        if ast::is_block(store, node) {
            return vec![self.visit_top_level_nested_block(node)];
        }
        if let Some((expression, loc)) = {
            if ast::is_export_assignment(store, node)
                && !store.is_export_equals(node).unwrap_or(false)
            {
                Some((
                    store
                        .expression(node)
                        .expect("export assignment should have expression"),
                    store.loc(node),
                ))
            } else {
                None
            }
        } {
            return vec![transform_export_default_assignment_expression(
                self.source,
                self.emit_context,
                expression,
                loc,
                self.import_references,
                self.compiler_options,
                self.file_name,
                self.module_format,
                self.source_file_contains_dynamic_import,
                self.direct_exported_names,
                self.local_export_specifiers,
                self.module_transform_facts,
            )];
        }
        if ast::is_class_declaration(store, node) {
            let mut statements = vec![self.visit_node(node)];
            self.append_exports_of_named_declaration(&mut statements, node);
            return statements;
        }
        if ast::is_function_declaration(store, node) {
            return vec![self.visit_node(node)];
        }
        let visited = self.visit_node(node);
        if visited.store_id() == self.emit_context.factory.node_factory.store().store_id() {
            vec![visited]
        } else {
            vec![self.preserve_node(visited)]
        }
    }

    fn visit_top_level_nested_statements(
        &mut self,
        statements: ast::SourceNodeList<'_>,
    ) -> Vec<ast::Node> {
        let mut result = Vec::new();
        for statement in statements.iter() {
            result.extend(self.visit_top_level_nested_statement(statement));
        }
        result
    }

    fn visit_embedded_statement(&mut self, node: ast::Node) -> Option<ast::Node> {
        let mut statements = self.visit_top_level_nested_statement(node);
        match statements.len() {
            0 => None,
            1 => statements.pop(),
            _ => {
                let list = self.factory_mut().new_node_list(
                    core::undefined_text_range(),
                    core::undefined_text_range(),
                    statements,
                );
                Some(self.factory_mut().new_block(list, true))
            }
        }
    }

    fn visit_iteration_body(&mut self, node: ast::Node) -> ast::Node {
        self.visit_embedded_statement(node)
            .unwrap_or_else(|| self.factory_mut().new_empty_statement())
    }

    fn visit_top_level_nested_variable_statement(&mut self, node: ast::Node) -> Vec<ast::Node> {
        let store = self.store_for(node);
        if ast::has_syntactic_modifier(store, node, ast::ModifierFlags::EXPORT) {
            return if node.store_id() == self.source.store_id() {
                transform_exported_variable_statement(
                    self.source,
                    self.emit_context,
                    &node,
                    None,
                    self.import_references,
                    self.compiler_options,
                    self.file_name,
                    self.module_format,
                    self.source_file_contains_dynamic_import,
                    self.direct_exported_names,
                    self.local_export_specifiers,
                    self.module_transform_facts,
                )
            } else {
                transform_exported_variable_statement_active(
                    self.source,
                    self.emit_context,
                    node,
                    self.emit_context.most_original(&node),
                    self.import_references,
                    self.compiler_options,
                    self.file_name,
                    self.module_format,
                    self.source_file_contains_dynamic_import,
                    self.direct_exported_names,
                    self.local_export_specifiers,
                    self.module_transform_facts,
                )
            };
        }
        let mut statements = vec![self.visit_node(node)];
        if node.store_id() == self.source.store_id() {
            self.append_exports_of_variable_statement(&mut statements, node);
        } else {
            let original_statement_count = statements.len();
            self.append_exports_of_variable_statement(&mut statements, node);
            let original = self.emit_context.most_original(&node);
            if statements.len() == original_statement_count
                && original.store_id() == self.source.store_id()
            {
                self.append_exports_of_variable_statement(&mut statements, original);
            }
        }
        statements
    }

    fn visit_top_level_nested_for_statement(&mut self, node: ast::Node) -> Vec<ast::Node> {
        let is_source_node = node.store_id() == self.source.store_id();
        let initializer = self.store_for(node).initializer(node);
        let condition = self.store_for(node).condition(node);
        let incrementor = self.store_for(node).incrementor(node);
        let statement = self
            .store_for(node)
            .statement(node)
            .expect("for statement should have body");

        if let Some(initializer) = initializer
            && ast::is_variable_declaration_list(self.store_for(initializer), initializer)
            && !self
                .store_for(initializer)
                .flags(initializer)
                .intersects(ast::NodeFlags::BLOCK_SCOPED)
        {
            let mut export_statements = Vec::new();
            self.append_exports_of_variable_declaration_list(
                &mut export_statements,
                initializer,
                false,
            );
            if !export_statements.is_empty() {
                let var_decl_list = self.visit_node(initializer);
                let var_statement = self
                    .factory_mut()
                    .new_variable_statement(None, var_decl_list);
                let condition = self.visit_optional_node(condition);
                let incrementor = self.visit_optional_node(incrementor);
                let body = self.visit_iteration_body(statement);
                let for_statement = if is_source_node {
                    let source = self.source;
                    self.factory_mut().update_for_statement_from_store(
                        source,
                        node,
                        None,
                        condition,
                        incrementor,
                        body,
                    )
                } else {
                    self.factory_mut().update_for_statement(
                        node,
                        None,
                        condition,
                        incrementor,
                        body,
                    )
                };
                let mut statements = vec![var_statement];
                statements.extend(export_statements);
                statements.push(for_statement);
                return statements;
            }
        }

        let initializer = self.visit_optional_node(initializer);
        let condition = self.visit_optional_node(condition);
        let incrementor = self.visit_optional_node(incrementor);
        let body = self.visit_iteration_body(statement);
        let for_statement = if is_source_node {
            let source = self.source;
            self.factory_mut().update_for_statement_from_store(
                source,
                node,
                initializer,
                condition,
                incrementor,
                body,
            )
        } else {
            self.factory_mut()
                .update_for_statement(node, initializer, condition, incrementor, body)
        };
        vec![for_statement]
    }

    fn visit_top_level_nested_for_in_or_of_statement(&mut self, node: ast::Node) -> ast::Node {
        let is_source_node = node.store_id() == self.source.store_id();
        let node_store = self.store_for(node);
        let initializer = node_store
            .initializer(node)
            .expect("for-in/of statement should have initializer");
        let expression = node_store
            .expression(node)
            .expect("for-in/of statement should have expression");
        let statement = node_store
            .statement(node)
            .expect("for-in/of statement should have body");
        let await_modifier = node_store.await_modifier(node);

        if ast::is_variable_declaration_list(self.store_for(initializer), initializer)
            && !self
                .store_for(initializer)
                .flags(initializer)
                .intersects(ast::NodeFlags::BLOCK_SCOPED)
        {
            let mut export_statements = Vec::new();
            self.append_exports_of_variable_declaration_list(
                &mut export_statements,
                initializer,
                true,
            );
            if !export_statements.is_empty() {
                let initializer = self.visit_node(initializer);
                let expression = self.visit_node(expression);
                let body = self.visit_iteration_body(statement);
                let body = self.prepend_statements_to_body(export_statements, body);
                let await_modifier = self.visit_optional_node(await_modifier);
                return if is_source_node {
                    let source = self.source;
                    self.factory_mut().update_for_in_or_of_statement_from_store(
                        source,
                        node,
                        await_modifier,
                        initializer,
                        expression,
                        body,
                    )
                } else {
                    self.factory_mut().update_for_in_or_of_statement(
                        node,
                        await_modifier,
                        initializer,
                        expression,
                        body,
                    )
                };
            }
        }

        let initializer = self.visit_node(initializer);
        let expression = self.visit_node(expression);
        let body = self.visit_iteration_body(statement);
        let await_modifier = self.visit_optional_node(await_modifier);
        if is_source_node {
            let source = self.source;
            self.factory_mut().update_for_in_or_of_statement_from_store(
                source,
                node,
                await_modifier,
                initializer,
                expression,
                body,
            )
        } else {
            self.factory_mut().update_for_in_or_of_statement(
                node,
                await_modifier,
                initializer,
                expression,
                body,
            )
        }
    }

    fn prepend_statements_to_body(
        &mut self,
        mut prefix: Vec<ast::Node>,
        body: ast::Node,
    ) -> ast::Node {
        let body_store = self.store_for(body);
        if ast::is_block(body_store, body) {
            let source_statements = body_store
                .source_statements(body)
                .expect("block should have statements");
            let loc = source_statements.loc();
            let range = source_statements.range();
            let multi_line = body_store.multi_line(body).unwrap_or(true);
            prefix.extend(source_statements.iter());
            let body_from_source = body.store_id() == self.source.store_id();
            let list = self.factory_mut().new_node_list(loc, range, prefix);
            if body_from_source {
                let source = self.source;
                self.factory_mut()
                    .update_block_from_store(source, body, list, multi_line)
            } else {
                self.factory_mut().update_block(body, list, multi_line)
            }
        } else {
            prefix.push(body);
            let list = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                prefix,
            );
            self.factory_mut().new_block(list, true)
        }
    }

    fn visit_top_level_nested_do_statement(&mut self, node: ast::Node) -> ast::Node {
        let (statement, expression) = {
            let store = self.store_for(node);
            (
                store
                    .statement(node)
                    .expect("do statement should have body"),
                store
                    .expression(node)
                    .expect("do statement should have expression"),
            )
        };
        let body = self.visit_iteration_body(statement);
        let expression = self.visit_node(expression);
        if node.store_id() == self.source.store_id() {
            let source = self.source;
            self.factory_mut()
                .update_do_statement_from_store(source, node, body, expression)
        } else {
            self.factory_mut()
                .update_do_statement(node, body, expression)
        }
    }

    fn visit_top_level_nested_while_statement(&mut self, node: ast::Node) -> ast::Node {
        let (expression, statement) = {
            let store = self.store_for(node);
            (
                store
                    .expression(node)
                    .expect("while statement should have expression"),
                store
                    .statement(node)
                    .expect("while statement should have body"),
            )
        };
        let expression = self.visit_node(expression);
        let body = self.visit_iteration_body(statement);
        if node.store_id() == self.source.store_id() {
            let source = self.source;
            self.factory_mut()
                .update_while_statement_from_store(source, node, expression, body)
        } else {
            self.factory_mut()
                .update_while_statement(node, expression, body)
        }
    }

    fn visit_top_level_nested_labeled_statement(&mut self, node: ast::Node) -> ast::Node {
        let (label, statement) = {
            let store = self.store_for(node);
            (
                store
                    .label(node)
                    .expect("labeled statement should have label"),
                store
                    .statement(node)
                    .expect("labeled statement should have body"),
            )
        };
        let label = self.visit_node(label);
        let statement = self
            .visit_embedded_statement(statement)
            .unwrap_or_else(|| self.factory_mut().new_empty_statement());
        if node.store_id() == self.source.store_id() {
            let source = self.source;
            self.factory_mut()
                .update_labeled_statement_from_store(source, node, label, statement)
        } else {
            self.factory_mut()
                .update_labeled_statement(node, label, statement)
        }
    }

    fn visit_top_level_nested_with_statement(&mut self, node: ast::Node) -> ast::Node {
        let (expression, statement) = {
            let store = self.store_for(node);
            (
                store
                    .expression(node)
                    .expect("with statement should have expression"),
                store
                    .statement(node)
                    .expect("with statement should have body"),
            )
        };
        let expression = self.visit_node(expression);
        let statement = self.visit_embedded_statement(statement);
        if node.store_id() == self.source.store_id() {
            let source = self.source;
            self.factory_mut()
                .update_with_statement_from_store(source, node, expression, statement)
        } else {
            self.factory_mut()
                .update_with_statement(node, expression, statement)
        }
    }

    fn visit_top_level_nested_if_statement(&mut self, node: ast::Node) -> ast::Node {
        let (expression_node, then_statement_node, else_statement_node, from_source) = {
            let store = self.store_for(node);
            (
                store
                    .expression(node)
                    .expect("if statement should have expression"),
                store
                    .then_statement(node)
                    .expect("if statement should have then statement"),
                store.else_statement(node),
                node.store_id() == self.source.store_id(),
            )
        };
        let expression = rewrite_common_js_expression(
            self.source,
            self.emit_context,
            self.import_references,
            self.direct_exported_names,
            self.local_export_specifiers,
            self.module_transform_facts,
            self.compiler_options,
            self.file_name,
            self.module_format,
            self.source_file_contains_dynamic_import,
            expression_node,
        );
        let then_statement = self
            .visit_embedded_statement(then_statement_node)
            .unwrap_or_else(|| {
                let list = self.factory_mut().new_node_list(
                    core::undefined_text_range(),
                    core::undefined_text_range(),
                    Vec::new(),
                );
                self.factory_mut().new_block(list, false)
            });
        let else_statement =
            else_statement_node.and_then(|statement| self.visit_embedded_statement(statement));
        if from_source {
            let source = self.source;
            self.factory_mut().update_if_statement_from_store(
                source,
                node,
                expression,
                then_statement,
                else_statement,
            )
        } else {
            self.factory_mut()
                .update_if_statement(node, expression, then_statement, else_statement)
        }
    }

    fn visit_top_level_nested_switch_statement(&mut self, node: ast::Node) -> ast::Node {
        let (expression, case_block) = {
            let store = self.store_for(node);
            (
                store
                    .expression(node)
                    .expect("switch statement should have expression"),
                store
                    .case_block(node)
                    .expect("switch statement should have case block"),
            )
        };
        let expression = self.visit_node(expression);
        let case_block = self.visit_top_level_nested_case_block(case_block);
        if node.store_id() == self.source.store_id() {
            let source = self.source;
            self.factory_mut()
                .update_switch_statement_from_store(source, node, expression, case_block)
        } else {
            self.factory_mut()
                .update_switch_statement(node, expression, case_block)
        }
    }

    fn visit_top_level_nested_case_block(&mut self, node: ast::Node) -> ast::Node {
        let (loc, range, clauses) = {
            let clauses = self
                .store_for(node)
                .clauses(node)
                .expect("case block should have clauses");
            (
                clauses.loc(),
                clauses.range(),
                clauses.iter().collect::<Vec<_>>(),
            )
        };
        let clauses = clauses
            .into_iter()
            .map(|clause| self.visit_top_level_nested_case_or_default_clause(clause))
            .collect::<Vec<_>>();
        let clauses = self.factory_mut().new_node_list(loc, range, clauses);
        if node.store_id() == self.source.store_id() {
            let source = self.source;
            self.factory_mut()
                .update_case_block_from_store(source, node, clauses)
        } else {
            self.factory_mut().update_case_block(node, clauses)
        }
    }

    fn visit_top_level_nested_case_or_default_clause(&mut self, node: ast::Node) -> ast::Node {
        let (expression, loc, range, statements) = {
            let store = self.store_for(node);
            let statements = store
                .source_statements(node)
                .expect("case/default clause should have statements");
            (
                store.expression(node),
                statements.loc(),
                statements.range(),
                statements.iter().collect::<Vec<_>>(),
            )
        };
        let expression = self.visit_optional_node(expression);
        let mut visited_statements = Vec::new();
        for statement in statements {
            visited_statements.extend(self.visit_top_level_nested_statement(statement));
        }
        let statements = visited_statements;
        let statements = self.factory_mut().new_node_list(loc, range, statements);
        if node.store_id() == self.source.store_id() {
            let source = self.source;
            self.factory_mut()
                .update_case_or_default_clause_from_store(source, node, expression, statements)
        } else {
            self.factory_mut()
                .update_case_or_default_clause(node, expression, statements)
        }
    }

    fn visit_top_level_nested_try_statement(&mut self, node: ast::Node) -> ast::Node {
        let store = self.store_for(node);
        let try_block = self.visit_top_level_nested_block(
            store
                .try_block(node)
                .expect("try statement should have try block"),
        );
        let catch_clause = self
            .store_for(node)
            .catch_clause(node)
            .map(|catch_clause| self.visit_top_level_nested_catch_clause(catch_clause));
        let finally_block = self
            .store_for(node)
            .finally_block(node)
            .map(|finally_block| self.visit_top_level_nested_block(finally_block));
        if node.store_id() == self.source.store_id() {
            let source = self.source;
            self.factory_mut().update_try_statement_from_store(
                source,
                node,
                try_block,
                catch_clause,
                finally_block,
            )
        } else {
            self.factory_mut()
                .update_try_statement(node, try_block, catch_clause, finally_block)
        }
    }

    fn visit_top_level_nested_catch_clause(&mut self, node: ast::Node) -> ast::Node {
        let variable_declaration = self.store_for(node).variable_declaration(node);
        let block = self
            .store_for(node)
            .block(node)
            .expect("catch clause should have block");
        let variable_declaration = variable_declaration.map(|node| self.visit_node(node));
        let block = self.visit_top_level_nested_block(block);
        if node.store_id() == self.source.store_id() {
            let source = self.source;
            self.factory_mut().update_catch_clause_from_store(
                source,
                node,
                variable_declaration,
                block,
            )
        } else {
            self.factory_mut()
                .update_catch_clause(node, variable_declaration, block)
        }
    }

    fn visit_top_level_nested_block(&mut self, node: ast::Node) -> ast::Node {
        let source_statements = self
            .store_for(node)
            .source_statements(node)
            .expect("block should have statements");
        let loc = source_statements.loc();
        let range = source_statements.range();
        let source_statements = source_statements.iter().collect::<Vec<_>>();
        let multi_line = self.store_for(node).multi_line(node).unwrap_or(true);
        let mut statements = Vec::new();
        for statement in source_statements {
            statements.extend(self.visit_top_level_nested_statement(statement));
        }
        let statements = self.factory_mut().new_node_list(loc, range, statements);
        if node.store_id() == self.source.store_id() {
            let source = self.source;
            self.factory_mut()
                .update_block_from_store(source, node, statements, multi_line)
        } else {
            self.factory_mut()
                .update_block(node, statements, multi_line)
        }
    }

    fn append_exports_of_variable_statement(
        &mut self,
        statements: &mut Vec<ast::Node>,
        node: ast::Node,
    ) {
        let store = self.store_for(node);
        let Some(declaration_list) = store.declaration_list(node) else {
            return;
        };
        self.append_exports_of_variable_declaration_list(statements, declaration_list, false);
    }

    fn append_exports_of_variable_declaration_list(
        &mut self,
        statements: &mut Vec<ast::Node>,
        node: ast::Node,
        is_for_in_or_of_initializer: bool,
    ) {
        let Some(declarations) = self.store_for(node).declarations(node) else {
            return;
        };
        let declarations: Vec<_> = declarations.iter().collect();
        for declaration in declarations {
            self.append_exports_of_binding_element(
                statements,
                declaration,
                is_for_in_or_of_initializer,
            );
        }
    }

    fn append_exports_of_binding_element(
        &mut self,
        statements: &mut Vec<ast::Node>,
        declaration: ast::Node,
        is_for_in_or_of_initializer: bool,
    ) {
        let store = self.store_for(declaration);
        let Some(name) = store.name(declaration) else {
            return;
        };
        if ast::is_binding_pattern(store, name) {
            if let Some(elements) = store.source_elements(name) {
                let elements: Vec<_> = elements.iter().collect();
                for element in elements {
                    if !ast::is_omitted_expression(self.store_for(element), element) {
                        self.append_exports_of_binding_element(
                            statements,
                            element,
                            is_for_in_or_of_initializer,
                        );
                    }
                }
            }
            return;
        }
        if !ast::is_identifier(store, name) {
            return;
        }
        if ast::is_variable_declaration(store, declaration)
            && store.initializer(declaration).is_none()
            && !is_for_in_or_of_initializer
        {
            return;
        }

        let local_name = store.text(name).to_owned();
        let Some(export_names) = self.local_export_specifiers.get(&local_name) else {
            return;
        };
        let initializer_keeps_local_binding =
            store.initializer(declaration).is_some_and(|initializer| {
                matches!(
                    store.kind(initializer),
                    ast::Kind::ArrowFunction
                        | ast::Kind::FunctionExpression
                        | ast::Kind::ClassExpression
                )
            });
        let use_exported_local =
            self.direct_exported_names.contains(&local_name) && !initializer_keeps_local_binding;
        for export_name in export_names {
            statements.push(create_export_assignment_statement(
                self.factory_mut(),
                export_name,
                local_name.as_str(),
                use_exported_local,
            ));
        }
    }

    fn append_exports_of_named_declaration(
        &mut self,
        statements: &mut Vec<ast::Node>,
        declaration: ast::Node,
    ) {
        let store = self.store_for(declaration);
        let Some(name) = store.name(declaration) else {
            return;
        };
        if !ast::is_identifier(store, name) {
            return;
        }

        let local_name = store.text(name).to_owned();
        let Some(export_names) = self.local_export_specifiers.get(&local_name) else {
            return;
        };
        for export_name in export_names {
            statements.push(create_export_assignment_statement(
                self.factory_mut(),
                export_name,
                local_name.as_str(),
                false,
            ));
        }
    }
}

fn collect_exported_names(
    source: &ast::AstStore,
    emit_context: &printer::EmitContext,
    file: ast::Node,
) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    for statement in source
        .parser_access()
        .source_file_statement_list(file)
        .iter()
    {
        if ast::is_variable_statement(source, statement) {
            if !ast::has_syntactic_modifier(source, statement, ast::ModifierFlags::EXPORT) {
                continue;
            }
            let Some(declaration_list) = source.declaration_list(statement) else {
                continue;
            };
            let declarations = source
                .declarations(declaration_list)
                .expect("variable declaration list should have declarations");
            for declaration in declarations.iter() {
                collect_exported_names_of_binding_element(
                    source,
                    declaration,
                    &mut names,
                    &mut seen,
                );
            }
        } else if ast::is_class_declaration(source, statement)
            && ast::has_syntactic_modifier(source, statement, ast::ModifierFlags::EXPORT)
        {
            if !ast::has_syntactic_modifier(source, statement, ast::ModifierFlags::DEFAULT)
                && let Some(name) = source.name(statement)
                && ast::is_identifier(source, name)
            {
                push_exported_name(&mut names, &mut seen, source.text(name));
            }
        } else if (ast::is_enum_declaration(source, statement)
            || ast::is_module_declaration(source, statement))
            && ast::has_syntactic_modifier(source, statement, ast::ModifierFlags::EXPORT)
            && let Some(name) = source.name(statement)
            && ast::is_identifier(source, name)
        {
            let name_text = source.text(name);
            if find_top_level_function_declaration(source, emit_context, file, &name_text).is_none()
            {
                push_exported_name(&mut names, &mut seen, name_text);
            }
        } else if ast::is_export_declaration(source, statement) {
            if source.is_type_only(statement).unwrap_or(false) {
                continue;
            }
            let Some(export_clause) = source.export_clause(statement) else {
                continue;
            };
            if !ast::is_named_exports(source, export_clause) {
                if source.module_specifier(statement).is_some()
                    && let Some(name) = source.name(export_clause)
                {
                    push_exported_name(&mut names, &mut seen, source.text(name));
                }
                continue;
            }
            let Some(elements) = source.source_elements(export_clause) else {
                continue;
            };
            for specifier in elements.iter() {
                if source.is_type_only(specifier).unwrap_or(false) {
                    continue;
                }
                let Some(name) = source.name(specifier) else {
                    continue;
                };
                if source.module_specifier(statement).is_some() {
                    push_exported_name(&mut names, &mut seen, source.text(name));
                    continue;
                }
                let local_name = source.property_name(specifier).unwrap_or(name);
                let local_name_text = source.text(local_name);
                if find_top_level_function_declaration(source, emit_context, file, &local_name_text)
                    .is_some()
                {
                    continue;
                }
                push_exported_name(&mut names, &mut seen, source.text(name));
            }
        }
    }
    names
}

fn collect_active_exported_names(
    source: &ast::AstStore,
    emit_context: &printer::EmitContext,
    source_statements: &[ActiveCommonJsStatement],
    facts: &ModuleTransformFacts,
) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    for statement in source_statements {
        if active_statement_has_kind(
            emit_context,
            statement.active,
            ast::Kind::NotEmittedStatement,
        ) {
            continue;
        }
        let (statement_source, statement_node) = match statement.parsed {
            Some(parsed) => (source, parsed),
            None => {
                let active = statement.active;
                (emit_context.store_for_node(active), active)
            }
        };
        if ast::is_variable_statement(statement_source, statement_node) {
            if !ast::has_syntactic_modifier(
                statement_source,
                statement_node,
                ast::ModifierFlags::EXPORT,
            ) {
                continue;
            }
            let Some(declaration_list) = statement_source.declaration_list(statement_node) else {
                continue;
            };
            let declarations = statement_source
                .declarations(declaration_list)
                .expect("variable declaration list should have declarations");
            for declaration in declarations.iter() {
                collect_exported_names_of_binding_element(
                    statement_source,
                    declaration,
                    &mut names,
                    &mut seen,
                );
            }
        } else if ast::is_class_declaration(statement_source, statement_node)
            && ast::has_syntactic_modifier(
                statement_source,
                statement_node,
                ast::ModifierFlags::EXPORT,
            )
        {
            if !ast::has_syntactic_modifier(
                statement_source,
                statement_node,
                ast::ModifierFlags::DEFAULT,
            ) && let Some(name) = statement_source.name(statement_node)
                && ast::is_identifier(statement_source, name)
            {
                push_exported_name(&mut names, &mut seen, statement_source.text(name));
            }
        } else if (ast::is_enum_declaration(statement_source, statement_node)
            || ast::is_module_declaration(statement_source, statement_node))
            && ast::has_syntactic_modifier(
                statement_source,
                statement_node,
                ast::ModifierFlags::EXPORT,
            )
            && let Some(name) = statement_source.name(statement_node)
            && ast::is_identifier(statement_source, name)
        {
            let name_text = statement_source.text(name);
            if find_active_top_level_function_declaration(
                emit_context,
                source_statements,
                &name_text,
            )
            .is_none()
            {
                push_exported_name(&mut names, &mut seen, name_text);
            }
        } else if ast::is_import_equals_declaration(statement_source, statement_node)
            && ast::has_syntactic_modifier(
                statement_source,
                statement_node,
                ast::ModifierFlags::EXPORT,
            )
            && !ast::is_external_module_import_equals_declaration(statement_source, statement_node)
            && let Some(name) = statement_source.name(statement_node)
            && ast::is_identifier(statement_source, name)
        {
            push_exported_name(&mut names, &mut seen, statement_source.text(name));
        } else if ast::is_export_declaration(statement_source, statement_node) {
            let (statement_source, statement_node) = if statement_source
                .module_specifier(statement_node)
                .is_some()
                && active_statement_has_kind(
                    emit_context,
                    statement.active,
                    ast::Kind::ExportDeclaration,
                ) {
                (
                    emit_context.store_for_node(statement.active),
                    statement.active,
                )
            } else {
                (statement_source, statement_node)
            };
            if statement_source
                .is_type_only(statement_node)
                .unwrap_or(false)
            {
                continue;
            }
            let Some(export_clause) = statement_source.export_clause(statement_node) else {
                continue;
            };
            if !ast::is_named_exports(statement_source, export_clause) {
                if statement_source.module_specifier(statement_node).is_some()
                    && let Some(name) = statement_source.name(export_clause)
                {
                    push_exported_name(&mut names, &mut seen, statement_source.text(name));
                }
                continue;
            }
            let Some(elements) = statement_source.source_elements(export_clause) else {
                continue;
            };
            for specifier in elements.iter() {
                if statement_source.is_type_only(specifier).unwrap_or(false) {
                    continue;
                }
                let Some(name) = statement_source.name(specifier) else {
                    continue;
                };
                if statement_source.module_specifier(statement_node).is_some() {
                    push_exported_name(&mut names, &mut seen, statement_source.text(name));
                    continue;
                }
                let export_name_text = statement_source.text(name);
                if !should_include_active_export_specifier(
                    emit_context,
                    specifier,
                    facts,
                    &export_name_text,
                ) {
                    continue;
                }
                let local_name = statement_source.property_name(specifier).unwrap_or(name);
                let local_name_text = statement_source.text(local_name);
                if find_active_top_level_function_declaration(
                    emit_context,
                    source_statements,
                    &local_name_text,
                )
                .is_some()
                {
                    continue;
                }
                push_exported_name(&mut names, &mut seen, export_name_text);
            }
        }
    }
    names
}

fn push_exported_name(names: &mut Vec<String>, seen: &mut HashSet<String>, name: String) {
    if seen.insert(name.clone()) {
        names.push(name);
    }
}

fn should_include_active_export_specifier(
    emit_context: &printer::EmitContext,
    specifier: ast::Node,
    facts: &ModuleTransformFacts,
    export_name_text: &str,
) -> bool {
    let parsed_specifier = emit_context.parse_node(&specifier);
    if parsed_specifier.is_some_and(|parsed| {
        let parsed_source = emit_context.store_for_node(parsed);
        ast::is_export_specifier(parsed_source, parsed)
    }) {
        return facts
            .value_export_specifier_names
            .contains(export_name_text);
    }
    true
}

fn collect_exported_names_of_binding_element(
    source: &ast::AstStore,
    declaration: ast::Node,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    let Some(name) = source.name(declaration) else {
        return;
    };

    if ast::is_binding_pattern(source, name) {
        if let Some(elements) = source.source_elements(name) {
            for element in elements.iter() {
                if !ast::is_omitted_expression(source, element) {
                    collect_exported_names_of_binding_element(source, element, names, seen);
                }
            }
        }
    } else if ast::is_identifier(source, name) {
        push_exported_name(names, seen, source.text(name));
    }
}

fn collect_direct_exported_names(source: &ast::AstStore, file: ast::Node) -> HashSet<String> {
    let mut names = HashSet::new();
    for statement in source
        .parser_access()
        .source_file_statement_list(file)
        .iter()
    {
        if ast::is_variable_statement(source, statement) {
            if !ast::has_syntactic_modifier(source, statement, ast::ModifierFlags::EXPORT) {
                continue;
            }
            let Some(declaration_list) = source.declaration_list(statement) else {
                continue;
            };
            let declarations = source
                .declarations(declaration_list)
                .expect("variable declaration list should have declarations");
            for declaration in declarations.iter() {
                collect_direct_exported_names_of_binding_element(source, declaration, &mut names);
            }
        } else if (ast::is_class_declaration(source, statement)
            || ast::is_function_declaration(source, statement))
            && ast::has_syntactic_modifier(source, statement, ast::ModifierFlags::EXPORT)
            && !ast::has_syntactic_modifier(source, statement, ast::ModifierFlags::DEFAULT)
            && (!ast::is_function_declaration(source, statement)
                || ast::node_is_present(source, source.body(statement)))
            && let Some(name) = source.name(statement)
            && ast::is_identifier(source, name)
        {
            names.insert(source.text(name));
        } else if ast::is_import_equals_declaration(source, statement)
            && ast::has_syntactic_modifier(source, statement, ast::ModifierFlags::EXPORT)
            && let Some(name) = source.name(statement)
            && ast::is_identifier(source, name)
        {
            names.insert(source.text(name));
        } else if (ast::is_enum_declaration(source, statement)
            || ast::is_module_declaration(source, statement))
            && ast::has_syntactic_modifier(source, statement, ast::ModifierFlags::EXPORT)
            && let Some(name) = source.name(statement)
            && ast::is_identifier(source, name)
        {
            names.insert(source.text(name));
        }
    }
    names
}

fn collect_direct_exported_names_of_binding_element(
    source: &ast::AstStore,
    declaration: ast::Node,
    names: &mut HashSet<String>,
) {
    let Some(name) = source.name(declaration) else {
        return;
    };

    if ast::is_binding_pattern(source, name) {
        if let Some(elements) = source.source_elements(name) {
            for element in elements.iter() {
                if !ast::is_omitted_expression(source, element) {
                    collect_direct_exported_names_of_binding_element(source, element, names);
                }
            }
        }
    } else if ast::is_identifier(source, name) {
        names.insert(source.text(name));
    }
}

fn collect_local_export_specifiers(
    source: &ast::AstStore,
    file: ast::Node,
    facts: &ModuleTransformFacts,
) -> HashMap<String, Vec<String>> {
    let mut specifiers = HashMap::new();
    let mut unique_exports = HashSet::new();
    for statement in source
        .parser_access()
        .source_file_statement_list(file)
        .iter()
    {
        if !ast::is_export_declaration(source, statement)
            || source.module_specifier(statement).is_some()
        {
            continue;
        }
        let Some(export_clause) = source.export_clause(statement) else {
            continue;
        };
        if !ast::is_named_exports(source, export_clause) {
            continue;
        }
        let Some(elements) = source.source_elements(export_clause) else {
            continue;
        };
        for specifier in elements.iter() {
            let Some(exported_name) = source.name(specifier) else {
                continue;
            };
            let specifier_name_text = source.text(exported_name);
            if !facts
                .value_export_specifier_names
                .contains(&specifier_name_text)
            {
                continue;
            }
            if !unique_exports.insert(specifier_name_text.clone()) {
                continue;
            }
            let local_name = source.property_name(specifier).unwrap_or(exported_name);
            specifiers
                .entry(source.text(local_name))
                .or_insert_with(Vec::new)
                .push(specifier_name_text);
        }
    }
    specifiers
}

fn merge_local_export_specifiers(
    target: &mut HashMap<String, Vec<String>>,
    source: HashMap<String, Vec<String>>,
) {
    for (local_name, export_names) in source {
        let target_export_names = target.entry(local_name).or_default();
        for export_name in export_names {
            if !target_export_names.iter().any(|name| name == &export_name) {
                target_export_names.push(export_name);
            }
        }
    }
}

fn collect_active_local_export_specifiers(
    emit_context: &printer::EmitContext,
    source_statements: &[ActiveCommonJsStatement],
    facts: &ModuleTransformFacts,
) -> HashMap<String, Vec<String>> {
    let mut specifiers = HashMap::new();
    let mut unique_exports = HashSet::new();
    for statement in source_statements {
        if let Some(parsed) = statement.parsed {
            collect_active_local_export_specifiers_of_statement(
                emit_context,
                parsed,
                &mut specifiers,
                &mut unique_exports,
                facts,
            );
        }
        let statement_nodes = active_statement_nodes(emit_context, statement.active);
        for statement_node in statement_nodes {
            collect_active_local_export_specifiers_of_statement(
                emit_context,
                statement_node,
                &mut specifiers,
                &mut unique_exports,
                facts,
            );
        }
    }
    specifiers
}

fn active_statement_nodes(
    emit_context: &printer::EmitContext,
    statement: ast::Node,
) -> Vec<ast::Node> {
    let statement_source = emit_context.store_for_node(statement);
    if statement_source.kind(statement) != ast::Kind::SyntaxList {
        return vec![statement];
    }
    statement_source
        .syntax_list_children(statement)
        .expect("SyntaxList should have children")
        .iter()
        .flatten()
        .collect()
}

fn collect_active_local_export_specifiers_of_statement(
    emit_context: &printer::EmitContext,
    statement_node: ast::Node,
    specifiers: &mut HashMap<String, Vec<String>>,
    unique_exports: &mut HashSet<String>,
    facts: &ModuleTransformFacts,
) {
    let statement_source = emit_context.store_for_node(statement_node);
    if !ast::is_export_declaration(statement_source, statement_node)
        || statement_source.module_specifier(statement_node).is_some()
        || statement_source
            .is_type_only(statement_node)
            .unwrap_or(false)
    {
        return;
    }
    let Some(export_clause) = statement_source.export_clause(statement_node) else {
        return;
    };
    if !ast::is_named_exports(statement_source, export_clause) {
        return;
    }
    let Some(elements) = statement_source.source_elements(export_clause) else {
        return;
    };
    for specifier in elements.iter() {
        if statement_source.is_type_only(specifier).unwrap_or(false) {
            continue;
        }
        let Some(exported_name) = statement_source.name(specifier) else {
            continue;
        };
        let specifier_name_text = statement_source.text(exported_name);
        if !should_include_active_export_specifier(
            emit_context,
            specifier,
            facts,
            &specifier_name_text,
        ) {
            continue;
        }
        if !unique_exports.insert(specifier_name_text.clone()) {
            continue;
        }
        let local_name = statement_source
            .property_name(specifier)
            .unwrap_or(exported_name);
        let local_name_text = statement_source.text(local_name);
        specifiers
            .entry(local_name_text)
            .or_insert_with(Vec::new)
            .push(specifier_name_text);
    }
}

fn find_top_level_function_declaration(
    source: &ast::AstStore,
    emit_context: &printer::EmitContext,
    file: ast::Node,
    name: &str,
) -> Option<ast::Node> {
    source
        .parser_access()
        .source_file_statement_list(file)
        .iter()
        .find_map(|statement| {
            let statement = if ast::is_not_emitted_statement(source, statement) {
                emit_context.most_original(&statement)
            } else {
                statement
            };
            let statement_source = emit_context.store_for_node(statement);
            if ast::is_function_declaration(statement_source, statement)
                && statement_source.name(statement).is_some_and(|node| {
                    ast::is_identifier(statement_source, node)
                        && statement_source.text(node) == name
                })
            {
                Some(statement)
            } else {
                None
            }
        })
}

fn find_top_level_default_class_or_function_declaration(
    source: &ast::AstStore,
    emit_context: &printer::EmitContext,
    file: ast::Node,
    name: &str,
) -> Option<ast::Node> {
    source
        .parser_access()
        .source_file_statement_list(file)
        .iter()
        .find_map(|statement| {
            let statement = if ast::is_not_emitted_statement(source, statement) {
                emit_context.most_original(&statement)
            } else {
                statement
            };
            let statement_source = emit_context.store_for_node(statement);
            if (ast::is_function_declaration(statement_source, statement)
                || ast::is_class_declaration(statement_source, statement))
                && ast::has_syntactic_modifier(
                    statement_source,
                    statement,
                    ast::ModifierFlags::DEFAULT,
                )
                && statement_source.name(statement).is_some_and(|node| {
                    ast::is_identifier(statement_source, node)
                        && statement_source.text(node) == name
                })
            {
                Some(statement)
            } else {
                None
            }
        })
}

fn find_active_top_level_function_declaration(
    emit_context: &printer::EmitContext,
    source_statements: &[ActiveCommonJsStatement],
    name: &str,
) -> Option<ast::Node> {
    source_statements.iter().find_map(|statement| {
        let statement = if active_statement_has_kind(
            emit_context,
            statement.active,
            ast::Kind::NotEmittedStatement,
        ) {
            statement.parsed.unwrap_or(statement.active)
        } else {
            statement.active
        };
        let statement_source = emit_context.store_for_node(statement);
        if ast::is_function_declaration(statement_source, statement)
            && statement_source.name(statement).is_some_and(|node| {
                ast::is_identifier(statement_source, node) && statement_source.text(node) == name
            })
        {
            Some(statement)
        } else {
            None
        }
    })
}

fn find_active_top_level_default_class_or_function_declaration(
    emit_context: &printer::EmitContext,
    source_statements: &[ActiveCommonJsStatement],
    name: &str,
) -> Option<ast::Node> {
    source_statements.iter().find_map(|statement| {
        let statement = if active_statement_has_kind(
            emit_context,
            statement.active,
            ast::Kind::NotEmittedStatement,
        ) {
            statement.parsed.unwrap_or(statement.active)
        } else {
            statement.active
        };
        let statement_source = emit_context.store_for_node(statement);
        if (ast::is_function_declaration(statement_source, statement)
            || ast::is_class_declaration(statement_source, statement))
            && ast::has_syntactic_modifier(statement_source, statement, ast::ModifierFlags::DEFAULT)
            && statement_source.name(statement).is_some_and(|node| {
                ast::is_identifier(statement_source, node) && statement_source.text(node) == name
            })
        {
            Some(statement)
        } else {
            None
        }
    })
}

fn find_active_source_top_level_function_declaration(
    source: &ast::AstStore,
    source_statements: &[ActiveCommonJsStatement],
    name: &str,
) -> Option<ast::Node> {
    source_statements.iter().find_map(|statement| {
        let statement = statement.parsed?;
        if ast::is_function_declaration(source, statement)
            && source
                .name(statement)
                .is_some_and(|node| ast::is_identifier(source, node) && source.text(node) == name)
        {
            Some(statement)
        } else {
            None
        }
    })
}

fn collect_exported_function_names(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    file: ast::Node,
) -> Vec<(String, ast::Node)> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    for statement in source
        .parser_access()
        .source_file_statement_list(file)
        .iter()
    {
        if ast::is_function_declaration(source, statement)
            && ast::has_syntactic_modifier(source, statement, ast::ModifierFlags::EXPORT)
            && !ast::has_syntactic_modifier(source, statement, ast::ModifierFlags::AMBIENT)
            && source.body(statement).is_some()
        {
            if ast::has_syntactic_modifier(source, statement, ast::ModifierFlags::DEFAULT) {
                let export_name = "default".to_owned();
                let local_name = emit_context.factory.get_local_name(source, &statement);
                names.push((export_name, local_name));
            } else {
                let Some(name) = source.name(statement) else {
                    continue;
                };
                let export_name = source.text(name);
                if seen.insert(export_name.clone()) {
                    let local_name = emit_context.factory.get_local_name(source, &statement);
                    names.push((export_name, local_name));
                }
            }
            continue;
        }

        if !ast::is_export_declaration(source, statement)
            || source.module_specifier(statement).is_some()
        {
            continue;
        }
        let Some(export_clause) = source.export_clause(statement) else {
            continue;
        };
        if !ast::is_named_exports(source, export_clause) {
            continue;
        }
        let Some(elements) = source.source_elements(export_clause) else {
            continue;
        };
        for specifier in elements.iter() {
            let Some(name) = source.name(specifier) else {
                continue;
            };
            let local_name = source.property_name(specifier).unwrap_or(name);
            let local_name_text = source.text(local_name);
            if let Some(declaration) =
                find_top_level_function_declaration(source, emit_context, file, &local_name_text)
            {
                let export_name = source.text(name);
                if seen.insert(export_name.clone()) {
                    let local_name = emit_context.factory.get_local_name(source, &declaration);
                    names.push((export_name, local_name));
                }
            }
        }
    }
    names
}

fn collect_active_exported_function_names(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    source_statements: &[ActiveCommonJsStatement],
) -> Vec<(String, ast::Node)> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    for statement in source_statements {
        let Some(statement_node) = statement.parsed else {
            continue;
        };
        if ast::is_function_declaration(source, statement_node)
            && ast::has_syntactic_modifier(source, statement_node, ast::ModifierFlags::EXPORT)
            && !ast::has_syntactic_modifier(source, statement_node, ast::ModifierFlags::AMBIENT)
            && source.body(statement_node).is_some()
        {
            if ast::has_syntactic_modifier(source, statement_node, ast::ModifierFlags::DEFAULT) {
                let export_name = "default".to_owned();
                let local_name = emit_context.factory.get_local_name(source, &statement_node);
                names.push((export_name, local_name));
            } else {
                let Some(name) = source.name(statement_node) else {
                    continue;
                };
                let export_name = source.text(name);
                if seen.insert(export_name.clone()) {
                    let local_name = emit_context.factory.get_local_name(source, &statement_node);
                    names.push((export_name, local_name));
                }
            }
            continue;
        }

        if !ast::is_export_declaration(source, statement_node)
            || source.module_specifier(statement_node).is_some()
        {
            continue;
        }
        let Some(export_clause) = source.export_clause(statement_node) else {
            continue;
        };
        if !ast::is_named_exports(source, export_clause) {
            continue;
        }
        let Some(elements) = source.source_elements(export_clause) else {
            continue;
        };
        for specifier in elements.iter() {
            let Some(name) = source.name(specifier) else {
                continue;
            };
            let local_name = source.property_name(specifier).unwrap_or(name);
            let local_name_text = source.text(local_name);
            if let Some(declaration) = find_active_source_top_level_function_declaration(
                source,
                source_statements,
                &local_name_text,
            ) {
                let export_name = source.text(name);
                if seen.insert(export_name.clone()) {
                    let local_name = emit_context.factory.get_local_name(source, &declaration);
                    names.push((export_name, local_name));
                }
            }
        }
    }
    names
}

struct ExportEqualsExpression {
    node: ast::Node,
    expression: ast::Node,
}

fn find_export_equals(
    source: &ast::AstStore,
    emit_context: &printer::EmitContext,
    file: ast::Node,
) -> Option<ExportEqualsExpression> {
    source
        .parser_access()
        .source_file_statement_list(file)
        .iter()
        .find_map(|statement| {
            // Look through NotEmittedStatement to find elided export= declarations
            // (e.g., `declare export = x` is elided by the type eraser but must still be collected)
            let statement = if ast::is_not_emitted_statement(source, statement) {
                emit_context.most_original(&statement)
            } else {
                statement
            };
            let statement_source = emit_context.store_for_node(statement);
            if ast::is_export_assignment(statement_source, statement)
                && statement_source
                    .is_export_equals(statement)
                    .unwrap_or(false)
            {
                let expression = statement_source.expression(statement)?;
                return Some(ExportEqualsExpression {
                    node: statement,
                    expression,
                });
            }
            None
        })
}

fn find_active_export_equals(
    emit_context: &printer::EmitContext,
    source_statements: &[ActiveCommonJsStatement],
) -> Option<ExportEqualsExpression> {
    source_statements.iter().find_map(|statement| {
        // Look through NotEmittedStatement to find elided export= declarations
        // (e.g., `declare export = x` is elided by the type eraser but must still be collected)
        let statement = if active_statement_has_kind(
            emit_context,
            statement.active,
            ast::Kind::NotEmittedStatement,
        ) {
            emit_context.most_original(&statement.active)
        } else {
            statement.active
        };
        let statement_source = emit_context.store_for_node(statement);
        if ast::is_export_assignment(statement_source, statement)
            && statement_source
                .is_export_equals(statement)
                .unwrap_or(false)
        {
            let expression = statement_source.expression(statement)?;
            return Some(ExportEqualsExpression {
                node: statement,
                expression,
            });
        }
        None
    })
}

fn should_emit_underscore_underscore_es_module(
    root: ast::Node,
    emit_context: &printer::EmitContext,
    export_equals_present: bool,
    source: &ast::AstStore,
    facts: &ModuleTransformFacts,
) -> bool {
    let (is_js, source_common_js_module_indicator, source_external_module_indicator) = emit_context
        .with_source_file_view(root, |source_file| {
            (
                source_file.is_js(),
                source_file.common_js_module_indicator(),
                source_file.external_module_indicator(),
            )
        });
    let common_js_module_indicator = facts
        .common_js_module_indicator
        .or(source_common_js_module_indicator);
    let external_module_indicator = facts
        .external_module_indicator
        .or(source_external_module_indicator);
    if is_js
        && common_js_module_indicator.is_some()
        && external_module_indicator.is_none_or(|indicator| {
            is_source_file_external_module_indicator(indicator, source, facts)
        })
    {
        return false;
    }
    !export_equals_present && external_module_indicator.is_some()
}

fn is_source_file_external_module_indicator(
    indicator: ast::Node,
    source: &ast::AstStore,
    facts: &ModuleTransformFacts,
) -> bool {
    if indicator.store_id() == source.store_id() {
        return source.kind(indicator) == ast::Kind::SourceFile;
    }
    facts.source_file_root.is_some_and(|root| indicator == root)
}

fn create_underscore_underscore_es_module(factory: &mut ast::NodeFactory) -> ast::Node {
    let object = factory.new_identifier("Object");
    let define_property = factory.new_identifier("defineProperty");
    let callee =
        factory.new_property_access_expression(object, None, define_property, ast::NodeFlags::NONE);

    let value_name = factory.new_identifier("value");
    let true_token = factory.new_token(ast::Kind::TrueKeyword);
    let value_property = factory.new_property_assignment(None, value_name, None, None, true_token);
    let value_properties = factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        vec![value_property],
    );
    let descriptor = factory.new_object_literal_expression(value_properties, false);

    let exports = factory.new_identifier("exports");
    let es_module = factory.new_string_literal("__esModule", ast::TokenFlags::NONE);
    let arguments = factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        vec![exports, es_module, descriptor],
    );
    let call = factory.new_call_expression(callee, None, None, arguments, ast::NodeFlags::NONE);
    factory.new_expression_statement(call)
}

fn exports_property_access(factory: &mut ast::NodeFactory, name: &str) -> ast::Node {
    let exports = factory.new_identifier("exports");
    if scanner::is_identifier_text(name, core::LANGUAGE_VARIANT_STANDARD) {
        let name = factory.new_identifier(name);
        factory.new_property_access_expression(exports, None, name, ast::NodeFlags::NONE)
    } else {
        let name = factory.new_string_literal(name, ast::TokenFlags::NONE);
        factory.new_element_access_expression(exports, None, name, ast::NodeFlags::NONE)
    }
}

fn exports_property_access_for_name(factory: &mut ast::NodeFactory, name: ast::Node) -> ast::Node {
    let exports = factory.new_identifier("exports");
    if ast::is_string_literal(factory.store(), name) {
        factory.new_element_access_expression(exports, None, name, ast::NodeFlags::NONE)
    } else {
        factory.new_property_access_expression(exports, None, name, ast::NodeFlags::NONE)
    }
}

fn clone_export_property_name(
    source: &ast::AstStore,
    factory: &mut ast::NodeFactory,
    name: ast::Node,
) -> ast::Node {
    if name.store_id() == factory.store().store_id() {
        factory.deep_clone_node_in_current_store_preserve_location(name)
    } else {
        factory.deep_clone_node_from_store_preserve_location(source, name)
    }
}

// Creates a call to the current file's export function to export a value.
//
//   - The `name` parameter is the bound name of the export.
//   - The `value` parameter is the exported value.
//   - The `location` parameter is the location to use for source maps and comments for the export.
//   - The `allowComments` parameter indicates whether to emit comments for the statement.
fn create_export_statement(
    emit_context: &mut printer::EmitContext,
    name: &str,
    value: ast::Node,
    location: Option<core::TextRange>,
    allow_comments: bool,
) -> ast::Node {
    let expression = create_export_expression(&mut emit_context.factory.node_factory, name, value);
    let statement = emit_context
        .factory
        .node_factory
        .new_expression_statement(expression);
    if let Some(location) = location {
        emit_context.set_comment_range(&statement, location);
    }
    emit_context.mark_emit_node(&statement, printer::EF_START_ON_NEW_LINE);
    if !allow_comments {
        emit_context.mark_emit_node(&statement, printer::EF_NO_COMMENTS);
    }
    statement
}

// Creates a call to the current file's export function to export a value.
//
//   - The `name` parameter is the bound name of the export.
//   - The `value` parameter is the exported value.
//   - The `location` parameter is the location to use for source maps and comments for the export.
fn create_export_expression(
    factory: &mut ast::NodeFactory,
    name: &str,
    value: ast::Node,
) -> ast::Node {
    let left = exports_property_access(factory, name);
    let equals = factory.new_token(ast::Kind::EqualsToken);
    factory.new_binary_expression(None, left, None, equals, value)
}

fn create_export_expression_for_name(
    factory: &mut ast::NodeFactory,
    name: ast::Node,
    value: ast::Node,
) -> ast::Node {
    let exports = factory.new_identifier("exports");
    let left = if ast::is_string_literal(factory.store(), name) {
        factory.new_element_access_expression(exports, None, name, ast::NodeFlags::NONE)
    } else {
        factory.new_property_access_expression(exports, None, name, ast::NodeFlags::NONE)
    };
    let equals = factory.new_token(ast::Kind::EqualsToken);
    factory.new_binary_expression(None, left, None, equals, value)
}

fn append_exported_names_preload(
    statements: &mut Vec<ast::Node>,
    emit_context: &mut printer::EmitContext,
    exported_names: &[String],
) {
    const CHUNK_SIZE: usize = 50;
    for chunk in exported_names.chunks(CHUNK_SIZE) {
        let factory = &mut emit_context.factory.node_factory;
        let zero = factory.new_numeric_literal("0", ast::TokenFlags::NONE);
        let mut right = factory.new_void_expression(zero);
        for exported_name in chunk {
            let left = exports_property_access(factory, exported_name);
            let equals = factory.new_token(ast::Kind::EqualsToken);
            right = factory.new_binary_expression(None, left, None, equals, right);
        }
        let statement = factory.new_expression_statement(right);
        emit_context.mark_emit_node(&statement, printer::EF_CUSTOM_PROLOGUE);
        statements.push(statement);
    }
}

fn create_export_assignment_statement(
    factory: &mut ast::NodeFactory,
    export_name: &str,
    local_name: &str,
    use_exported_local: bool,
) -> ast::Node {
    let left = exports_property_access(factory, export_name);
    let equals = factory.new_token(ast::Kind::EqualsToken);
    let right = if use_exported_local {
        exports_property_access(factory, local_name)
    } else {
        factory.new_identifier(local_name)
    };
    let assignment = factory.new_binary_expression(None, left, None, equals, right);
    factory.new_expression_statement(assignment)
}

fn create_live_binding_export_expression(
    factory: &mut printer::NodeFactory,
    export_name: ast::Node,
    value: ast::Node,
) -> ast::Node {
    let exports = factory.node_factory.new_identifier("exports");
    let export_name = if ast::is_string_literal(factory.node_factory.store(), export_name) {
        export_name
    } else {
        let text = factory.node_factory.store().text(export_name);
        factory
            .node_factory
            .new_string_literal(text, ast::TokenFlags::NONE)
    };
    let enumerable = factory.node_factory.new_identifier("enumerable");
    let true_token = factory.node_factory.new_token(ast::Kind::TrueKeyword);
    let enumerable_property = factory
        .node_factory
        .new_property_assignment(None, enumerable, None, None, true_token);

    let return_statement = factory.node_factory.new_return_statement(value);
    let statements = factory.node_factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        vec![return_statement],
    );
    let body = factory.node_factory.new_block(statements, false);
    let parameters = factory.node_factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        Vec::<ast::Node>::new(),
    );
    let get_function = factory.node_factory.new_function_expression(
        None::<ast::ModifierList>,
        None::<ast::Node>,
        None::<ast::Node>,
        None::<ast::NodeList>,
        parameters,
        None::<ast::Node>,
        None::<ast::Node>,
        body,
    );
    let get = factory.node_factory.new_identifier("get");
    let get_property =
        factory
            .node_factory
            .new_property_assignment(None, get, None, None, get_function);
    let properties = factory.node_factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        vec![enumerable_property, get_property],
    );
    let descriptor = factory
        .node_factory
        .new_object_literal_expression(properties, false);
    factory.new_object_define_property_call(exports, export_name, descriptor)
}

fn module_exports_property_access(factory: &mut ast::NodeFactory) -> ast::Node {
    let module = factory.new_identifier("module");
    let exports = factory.new_identifier("exports");
    factory.new_property_access_expression(module, None, exports, ast::NodeFlags::NONE)
}

fn strip_export_modifiers(
    importer: &mut ast::AstImporter<'_, '_>,
    modifiers: Option<ast::SourceModifierList<'_>>,
) -> Option<ast::ModifierList> {
    let modifiers = modifiers?;
    let source = modifiers.store();
    let modifier_list = modifiers;
    let modifier_nodes = modifier_list.nodes();
    let filtered: Vec<_> = modifier_nodes
        .iter()
        .filter(|modifier| {
            !matches!(
                source.kind(*modifier),
                ast::Kind::ExportKeyword | ast::Kind::DefaultKeyword
            )
        })
        .map(|modifier| importer.preserve_node(modifier))
        .collect();
    if filtered.is_empty() {
        None
    } else {
        Some(importer.factory().new_modifier_list(
            modifier_nodes.loc(),
            modifier_nodes.range(),
            filtered,
            modifier_list.modifier_flags(),
        ))
    }
}

fn active_statement_has_kind(
    emit_context: &printer::EmitContext,
    statement: ast::Node,
    kind: ast::Kind,
) -> bool {
    emit_context.factory.node_factory.store().kind(statement) == kind
}

fn flatten_syntax_list_statements(
    emit_context: &printer::EmitContext,
    statements: Vec<ast::Node>,
) -> Vec<ast::Node> {
    let mut flattened = Vec::with_capacity(statements.len());
    for statement in statements {
        let store = emit_context.store_for_node(statement);
        if store.kind(statement) == ast::Kind::SyntaxList {
            let children = store
                .syntax_list_children(statement)
                .expect("SyntaxList should have children");
            flattened.extend(children.into_iter().flatten());
        } else {
            flattened.push(statement);
        }
    }
    flattened
}

fn single_or_syntax_list(
    factory: &mut ast::NodeFactory,
    mut statements: Vec<ast::Node>,
) -> Option<ast::Node> {
    match statements.len() {
        0 => None,
        1 => statements.pop(),
        _ => Some(factory.new_syntax_list(statements)),
    }
}

fn transform_active_class_decorator_assignment_to_common_js(
    emit_context: &mut printer::EmitContext,
    statement: ast::Node,
    direct_exported_names: &HashSet<String>,
    local_export_specifiers: &HashMap<String, Vec<String>>,
) -> Option<ast::Node> {
    let source = emit_context.factory.node_factory.store();
    if !ast::is_expression_statement(source, statement) {
        return None;
    }
    let expression = source.expression(statement)?;
    if !ast::is_call_expression(source, expression) {
        return None;
    }
    let callee = source.expression(expression)?;
    if !ast::is_identifier(source, callee) || source.text(callee) != "__decorate" {
        return None;
    }
    let arguments = source.arguments(expression)?;
    if arguments.len() != 2 {
        return None;
    }
    let target = arguments.iter().nth(1)?;
    if !ast::is_identifier(source, target) {
        return None;
    }
    let target_text = source.text(target);
    let mut export_names = Vec::new();
    if direct_exported_names.contains(&target_text) {
        export_names.push(target_text.clone());
    }
    if let Some(specifiers) = local_export_specifiers.get(&target_text) {
        for specifier in specifiers {
            if !export_names.iter().any(|name| name == specifier) {
                export_names.push(specifier.clone());
            }
        }
    }
    if export_names.is_empty() {
        return None;
    }

    let target_assignment = emit_context
        .factory
        .new_assignment_expression(target, expression);
    let mut result = target_assignment;
    for export_name in export_names {
        let export_name = emit_context
            .factory
            .node_factory
            .new_identifier(&export_name);
        let left =
            exports_property_access_for_name(&mut emit_context.factory.node_factory, export_name);
        result = emit_context.factory.new_assignment_expression(left, result);
    }
    Some(
        emit_context
            .factory
            .node_factory
            .new_expression_statement(result),
    )
}

fn rewrite_active_common_js_statement_list(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    import_references: &HashMap<String, ImportReference>,
    direct_exported_names: &HashSet<String>,
    local_export_specifiers: &HashMap<String, Vec<String>>,
    module_transform_facts: &ModuleTransformFacts,
    compiler_options: &core::CompilerOptions,
    file_name: &str,
    module_format: core::ModuleKind,
    source_file_contains_dynamic_import: bool,
    statement: ast::Node,
) -> Option<Vec<ast::Node>> {
    enum ActiveStatementList {
        ExportedVariableStatement,
        OtherStatement,
        SyntaxList(Vec<ast::Node>),
    }

    let active_statement_list = {
        let active_source = emit_context.factory.node_factory.store();
        if statement.store_id() != active_source.store_id() {
            return None;
        }

        if active_source.kind(statement) == ast::Kind::SyntaxList {
            let children = active_source
                .syntax_list_children(statement)
                .expect("SyntaxList should have children")
                .iter()
                .flatten()
                .collect::<Vec<ast::Node>>();
            ActiveStatementList::SyntaxList(children)
        } else {
            let is_exported_variable_statement = active_source.kind(statement)
                == ast::Kind::VariableStatement
                && ast::has_syntactic_modifier(
                    active_source,
                    statement,
                    ast::ModifierFlags::EXPORT,
                );
            match is_exported_variable_statement {
                true => ActiveStatementList::ExportedVariableStatement,
                false => ActiveStatementList::OtherStatement,
            }
        }
    };

    let children = match active_statement_list {
        ActiveStatementList::ExportedVariableStatement => {
            return Some(transform_exported_variable_statement_active(
                source,
                emit_context,
                statement,
                emit_context.most_original(&statement),
                import_references,
                compiler_options,
                file_name,
                module_format,
                source_file_contains_dynamic_import,
                direct_exported_names,
                local_export_specifiers,
                module_transform_facts,
            ));
        }
        ActiveStatementList::OtherStatement => {
            if let Some((expression, loc)) = active_export_assignment_input(emit_context, statement)
            {
                return Some(vec![transform_export_default_assignment_expression(
                    source,
                    emit_context,
                    expression,
                    loc,
                    import_references,
                    compiler_options,
                    file_name,
                    module_format,
                    source_file_contains_dynamic_import,
                    direct_exported_names,
                    local_export_specifiers,
                    module_transform_facts,
                )]);
            }
            if let Some(active_export) =
                active_export_declaration_to_common_js_input(emit_context, statement)
            {
                return Some(transform_active_export_declaration_to_common_js(
                    emit_context,
                    statement,
                    statement,
                    active_export,
                ));
            }
            if is_common_js_top_level_nested_statement_kind(
                emit_context.store_for_node(statement).kind(statement),
            ) {
                let mut handled_exported_names = HashSet::new();
                let mut visitor = TopLevelNestedCommonJsVisitor {
                    source,
                    emit_context,
                    import_references,
                    compiler_options,
                    file_name,
                    module_format,
                    source_file_contains_dynamic_import,
                    local_export_specifiers,
                    direct_exported_names,
                    module_transform_facts,
                    handled_exported_names: &mut handled_exported_names,
                };
                return Some(visitor.visit_top_level_nested_statement(statement));
            }
            let mut statements = vec![rewrite_common_js_statement(
                source,
                emit_context,
                import_references,
                direct_exported_names,
                local_export_specifiers,
                module_transform_facts,
                compiler_options,
                file_name,
                module_format,
                source_file_contains_dynamic_import,
                statement,
            )];
            append_exports_of_active_variable_statement(
                emit_context,
                direct_exported_names,
                local_export_specifiers,
                &mut statements,
                statement,
            );
            return Some(statements);
        }
        ActiveStatementList::SyntaxList(children) => children,
    };
    let mut statements = Vec::new();
    for child in children {
        let (is_exported_variable_statement, active_export, active_export_assignment) = {
            let active_source = emit_context.factory.node_factory.store();
            (
                active_source.kind(child) == ast::Kind::VariableStatement
                    && ast::has_syntactic_modifier(
                        active_source,
                        child,
                        ast::ModifierFlags::EXPORT,
                    ),
                active_export_declaration_to_common_js_input(emit_context, child),
                active_export_assignment_input(emit_context, child),
            )
        };
        if is_exported_variable_statement {
            statements.extend(transform_exported_variable_statement_active(
                source,
                emit_context,
                child,
                emit_context.most_original(&child),
                import_references,
                compiler_options,
                file_name,
                module_format,
                source_file_contains_dynamic_import,
                direct_exported_names,
                local_export_specifiers,
                module_transform_facts,
            ));
            append_exports_of_active_variable_statement(
                emit_context,
                direct_exported_names,
                local_export_specifiers,
                &mut statements,
                child,
            );
        } else if let Some(active_export) = active_export {
            statements.extend(transform_active_export_declaration_to_common_js(
                emit_context,
                child,
                child,
                active_export,
            ));
        } else if let Some((expression, loc)) = active_export_assignment {
            statements.push(transform_export_default_assignment_expression(
                source,
                emit_context,
                expression,
                loc,
                import_references,
                compiler_options,
                file_name,
                module_format,
                source_file_contains_dynamic_import,
                direct_exported_names,
                local_export_specifiers,
                module_transform_facts,
            ));
        } else if is_common_js_top_level_nested_statement_kind(
            emit_context.store_for_node(child).kind(child),
        ) {
            let mut handled_exported_names = HashSet::new();
            let mut visitor = TopLevelNestedCommonJsVisitor {
                source,
                emit_context,
                import_references,
                compiler_options,
                file_name,
                module_format,
                source_file_contains_dynamic_import,
                local_export_specifiers,
                direct_exported_names,
                module_transform_facts,
                handled_exported_names: &mut handled_exported_names,
            };
            statements.extend(visitor.visit_top_level_nested_statement(child));
        } else {
            statements.push(rewrite_common_js_statement(
                source,
                emit_context,
                import_references,
                direct_exported_names,
                local_export_specifiers,
                module_transform_facts,
                compiler_options,
                file_name,
                module_format,
                source_file_contains_dynamic_import,
                child,
            ));
            append_exports_of_active_variable_statement(
                emit_context,
                direct_exported_names,
                local_export_specifiers,
                &mut statements,
                child,
            );
        }
    }
    Some(statements)
}

fn is_common_js_top_level_nested_statement_kind(kind: ast::Kind) -> bool {
    matches!(
        kind,
        ast::Kind::Block
            | ast::Kind::ClassDeclaration
            | ast::Kind::DoStatement
            | ast::Kind::ForInStatement
            | ast::Kind::ForOfStatement
            | ast::Kind::ForStatement
            | ast::Kind::IfStatement
            | ast::Kind::LabeledStatement
            | ast::Kind::SwitchStatement
            | ast::Kind::TryStatement
            | ast::Kind::WhileStatement
            | ast::Kind::WithStatement
    )
}

fn append_exports_of_active_variable_statement(
    emit_context: &mut printer::EmitContext,
    direct_exported_names: &HashSet<String>,
    local_export_specifiers: &HashMap<String, Vec<String>>,
    statements: &mut Vec<ast::Node>,
    statement: ast::Node,
) {
    let direct_export_original = {
        let original = emit_context.most_original(&statement);
        let original_source = emit_context.store_for_node(original);
        (ast::is_class_declaration(original_source, original)
            || ast::is_function_declaration(original_source, original))
            && ast::has_syntactic_modifier(original_source, original, ast::ModifierFlags::EXPORT)
            && !ast::has_syntactic_modifier(original_source, original, ast::ModifierFlags::DEFAULT)
    };
    let declaration_names = {
        let active_source = emit_context.factory.node_factory.store();
        if !ast::is_variable_statement(active_source, statement) {
            return;
        }
        let Some(declaration_list) = active_source.declaration_list(statement) else {
            return;
        };
        let Some(declarations) = active_source.declarations(declaration_list) else {
            return;
        };
        let mut names = Vec::new();
        collect_active_variable_declaration_names(active_source, declarations.iter(), &mut names);
        names
    };

    let mut seen = HashSet::new();
    for local_name in declaration_names {
        if direct_export_original
            && direct_exported_names.contains(&local_name)
            && seen.insert(local_name.clone())
        {
            statements.push(create_export_assignment_statement(
                &mut emit_context.factory.node_factory,
                &local_name,
                &local_name,
                false,
            ));
        }
        if let Some(export_names) = local_export_specifiers.get(&local_name) {
            for export_name in export_names {
                if !seen.insert(export_name.clone()) {
                    continue;
                }
                statements.push(create_export_assignment_statement(
                    &mut emit_context.factory.node_factory,
                    export_name,
                    &local_name,
                    false,
                ));
            }
        }
    }
}

fn collect_active_variable_declaration_names(
    source: &ast::AstStore,
    declarations: impl Iterator<Item = ast::Node>,
    names: &mut Vec<String>,
) {
    for declaration in declarations {
        if ast::is_variable_declaration(source, declaration)
            && source.initializer(declaration).is_none()
        {
            continue;
        }
        let Some(name) = source.name(declaration) else {
            continue;
        };
        if ast::is_identifier(source, name) {
            names.push(source.text(name));
        } else if ast::is_binding_pattern(source, name)
            && let Some(elements) = source.source_elements(name)
        {
            collect_active_variable_declaration_names(source, elements.iter(), names);
        }
    }
}

fn statement_needs_common_js_reference_rewrite(
    source: &ast::AstStore,
    statement: ast::Node,
    emit_context: &printer::EmitContext,
    compiler_options: &core::CompilerOptions,
    file_name: &str,
    module_format: core::ModuleKind,
    source_file_contains_dynamic_import: bool,
    module_transform_facts: &ModuleTransformFacts,
) -> bool {
    let mut stack = vec![statement];
    while let Some(node) = stack.pop() {
        if ast::is_identifier(source, node)
            && (module_transform_facts.references_source_file_export(source, node)
                || module_transform_facts
                    .referenced_export_bindings
                    .contains_key(&source.loc(node))
                || module_transform_facts
                    .referenced_import_references
                    .contains_key(&source.loc(node)))
        {
            return true;
        }

        if source_file_contains_dynamic_import
            && ast::should_transform_import_call(file_name, compiler_options, module_format)
            && ast::is_import_call(source, node)
        {
            return true;
        }

        if compiler_options
            .rewrite_relative_import_extensions
            .is_true()
            && (ast::is_import_call(source, node)
                || (ast::is_in_js_file(source, node) && ast::is_require_call(source, node, false)))
        {
            return true;
        }

        let _ = source.for_each_present_child(node, |child| {
            stack.push(child);
            std::ops::ControlFlow::Continue(())
        });
    }

    let _ = emit_context;
    false
}

fn variable_statement_has_binding_pattern(source: &ast::AstStore, statement: ast::Node) -> bool {
    let Some(declaration_list) = source.declaration_list(statement) else {
        return false;
    };
    let Some(declarations) = source.declarations(declaration_list) else {
        return false;
    };
    declarations.iter().any(|declaration| {
        source
            .name(declaration)
            .is_some_and(|name| ast::is_binding_pattern(source, name))
    })
}

fn active_stripped_export_modifiers(
    emit_context: &mut printer::EmitContext,
    statement: ast::Node,
) -> Option<ast::ModifierList> {
    let modifier_parts = {
        let source = emit_context.factory.node_factory.store();
        source.source_modifiers(statement).map(|modifiers| {
            let modifier_nodes = modifiers.nodes();
            let filtered = modifier_nodes
                .iter()
                .filter(|modifier| {
                    !matches!(
                        source.kind(*modifier),
                        ast::Kind::ExportKeyword | ast::Kind::DefaultKeyword
                    )
                })
                .collect::<Vec<_>>();
            (
                modifier_nodes.loc(),
                modifier_nodes.range(),
                filtered,
                modifiers.modifier_flags(),
            )
        })
    };
    let (loc, range, filtered, modifier_flags) = modifier_parts?;
    if filtered.is_empty() {
        None
    } else {
        Some(emit_context.factory.node_factory.new_modifier_list(
            loc,
            range,
            filtered,
            modifier_flags,
        ))
    }
}

fn active_optional_node_list(
    emit_context: &mut printer::EmitContext,
    list: Option<(core::TextRange, core::TextRange, Vec<ast::Node>, bool)>,
) -> Option<ast::NodeList> {
    list.map(|(loc, range, nodes, has_trailing_comma)| {
        emit_context
            .factory
            .node_factory
            .new_node_list_with_trailing_comma(loc, range, nodes, has_trailing_comma)
    })
}

fn active_node_list(
    emit_context: &mut printer::EmitContext,
    list: (core::TextRange, core::TextRange, Vec<ast::Node>, bool),
) -> ast::NodeList {
    active_optional_node_list(emit_context, Some(list)).expect("node list is required")
}

fn active_declaration_name(
    emit_context: &mut printer::EmitContext,
    statement: ast::Node,
) -> ast::Node {
    let name_text = {
        let source = emit_context.factory.node_factory.store();
        source.name(statement).map(|name| source.text(name))
    };
    if let Some(name_text) = name_text {
        emit_context.factory.node_factory.new_identifier(name_text)
    } else {
        emit_context.new_generated_name_for_node(statement)
    }
}

fn strip_export_from_active_class_declaration(
    file: &ast::SourceFile,
    emit_context: &mut printer::EmitContext,
    compiler_options: &core::CompilerOptions,
    statement: ast::Node,
) -> ast::Node {
    let (name, heritage_clauses, members) = {
        let source = emit_context.factory.node_factory.store();
        let name = source.name(statement);
        let heritage_clauses = source.source_heritage_clauses(statement).map(|list| {
            (
                list.loc(),
                list.range(),
                list.iter().collect::<Vec<_>>(),
                list.has_trailing_comma(),
            )
        });
        let members = source
            .source_members(statement)
            .expect("class declaration should have members");
        let mut member_nodes = members.iter().collect::<Vec<_>>();
        if crate::estransforms::classfields::class_field_transform_config(
            compiler_options.get_emit_script_target(),
            compiler_options.get_use_define_for_class_fields(),
            compiler_options.experimental_decorators.is_true(),
        )
        .is_some_and(|config| config.should_transform_initializers)
        {
            member_nodes
                .retain(|member| !is_public_static_property_with_initializer(source, *member));
        }
        let members = (
            members.loc(),
            members.range(),
            member_nodes,
            members.has_trailing_comma(),
        );
        (name, heritage_clauses, members)
    };
    let modifiers = active_stripped_export_modifiers(emit_context, statement);
    let name = name.unwrap_or_else(|| emit_context.new_generated_name_for_node(statement));
    emit_context.mark_emit_node(&name, printer::EF_NO_SOURCE_MAP);
    let heritage_clauses = active_optional_node_list(emit_context, heritage_clauses);
    let members = active_node_list(emit_context, members);
    let stripped = emit_context.factory.node_factory.update_class_declaration(
        statement,
        modifiers,
        name,
        None::<ast::NodeList>,
        heritage_clauses,
        members,
    );
    let stripped = crate::tstransforms::typeeraser::visit_source_file_root(
        file,
        stripped,
        emit_context,
        compiler_options,
    );
    stripped
}

fn is_public_static_property_with_initializer(source: &ast::AstStore, member: ast::Node) -> bool {
    if source.kind(member) != ast::Kind::PropertyDeclaration
        || !ast::has_static_modifier(source, member)
        || ast::is_auto_accessor_property_declaration(source, member)
        || source.initializer(member).is_none()
    {
        return false;
    }
    source
        .name(member)
        .is_some_and(|name| !ast::is_private_identifier(source, name))
}

fn strip_export_from_active_function_declaration(
    emit_context: &mut printer::EmitContext,
    statement: ast::Node,
) -> ast::Node {
    let (asterisk_token, name, parameters, body) = {
        let source = emit_context.factory.node_factory.store();
        let parameters = source
            .source_parameters(statement)
            .expect("function declaration should have parameters");
        let parameters = (
            parameters.loc(),
            parameters.range(),
            parameters.iter().collect::<Vec<_>>(),
            parameters.has_trailing_comma(),
        );
        (
            source.asterisk_token(statement),
            source.name(statement),
            parameters,
            source.body(statement),
        )
    };
    let modifiers = active_stripped_export_modifiers(emit_context, statement);
    let name = name.unwrap_or_else(|| emit_context.new_generated_name_for_node(statement));
    let parameters = active_node_list(emit_context, parameters);
    emit_context
        .factory
        .node_factory
        .update_function_declaration(
            statement,
            modifiers,
            asterisk_token,
            name,
            None::<ast::NodeList>,
            parameters,
            None::<ast::Node>,
            None::<ast::Node>,
            body,
        )
}

fn create_export_statement_for_active_or_read_declaration(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    active_statement: ast::Node,
    read_statement: ast::Node,
) -> Option<ast::Node> {
    let active_kind = {
        let active_source = emit_context.factory.node_factory.store();
        active_source.kind(active_statement)
    };
    if matches!(
        active_kind,
        ast::Kind::ClassDeclaration | ast::Kind::FunctionDeclaration
    ) {
        let active_source = emit_context.factory.node_factory.store();
        if ast::has_syntactic_modifier(source, read_statement, ast::ModifierFlags::DEFAULT)
            && !ast::has_syntactic_modifier(
                active_source,
                active_statement,
                ast::ModifierFlags::DEFAULT,
            )
        {
            return None;
        }
        let export_name =
            if ast::has_syntactic_modifier(source, read_statement, ast::ModifierFlags::DEFAULT) {
                emit_context.factory.node_factory.new_identifier("default")
            } else {
                active_declaration_name(emit_context, active_statement)
            };
        let right = active_declaration_name(emit_context, active_statement);
        let factory = &mut emit_context.factory.node_factory;
        let left = exports_property_access_for_name(factory, export_name);
        let equals = factory.new_token(ast::Kind::EqualsToken);
        let assignment = factory.new_binary_expression(None, left, None, equals, right);
        let statement = factory.new_expression_statement(assignment);
        emit_context.set_comment_range(&statement, source.loc(read_statement));
        emit_context.mark_emit_node(
            &statement,
            printer::EF_START_ON_NEW_LINE | printer::EF_NO_COMMENTS,
        );
        return Some(statement);
    }

    create_export_statement_for_declaration(source, emit_context, &read_statement)
}

fn transform_exported_variable_statement_active(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    statement: ast::Node,
    original_statement: ast::Node,
    import_references: &HashMap<String, ImportReference>,
    compiler_options: &core::CompilerOptions,
    file_name: &str,
    module_format: core::ModuleKind,
    source_file_contains_dynamic_import: bool,
    direct_exported_names: &HashSet<String>,
    local_export_specifiers: &HashMap<String, Vec<String>>,
    module_transform_facts: &ModuleTransformFacts,
) -> Vec<ast::Node> {
    let declaration_data = {
        let active_source = emit_context.factory.node_factory.store();
        let Some(declaration_list) = active_source.declaration_list(statement) else {
            return Vec::new();
        };
        let declarations = active_source
            .declarations(declaration_list)
            .expect("variable declaration list should have declarations");
        let declaration_list_flags = active_source.flags(declaration_list);
        let declarations_loc = declarations.loc();
        let declarations_range = declarations.range();
        let declarations_has_trailing_comma = declarations.has_trailing_comma();
        let declarations = declarations
            .iter()
            .map(|declaration| {
                let name = active_source.name(declaration);
                let name_text = name.map(|name| active_source.text(name));
                let initializer = active_source.initializer(declaration);
                let initializer_kind =
                    initializer.map(|initializer| active_source.kind(initializer));
                (
                    declaration,
                    name,
                    name_text,
                    initializer,
                    initializer_kind,
                    active_source.r#type(declaration),
                    active_source.exclamation_token(declaration),
                )
            })
            .collect::<Vec<_>>();
        (
            declaration_list_flags,
            declarations_loc,
            declarations_range,
            declarations_has_trailing_comma,
            declarations,
        )
    };

    let (
        declaration_list_flags,
        declarations_loc,
        declarations_range,
        declarations_has_trailing_comma,
        declaration_data,
    ) = declaration_data;
    let mut statements = Vec::new();
    let mut preserved_variables = Vec::new();

    for (
        declaration,
        name,
        name_text,
        initializer,
        initializer_kind,
        type_node,
        exclamation_token,
    ) in declaration_data
    {
        let Some(name) = name else {
            continue;
        };
        let name_text = name_text.unwrap_or_default();
        let is_identifier = {
            let active_source = emit_context.factory.node_factory.store();
            ast::is_identifier(active_source, name)
        };
        if !is_identifier {
            continue;
        }
        let initializer = initializer.map(|initializer| {
            rewrite_common_js_expression(
                source,
                emit_context,
                import_references,
                direct_exported_names,
                local_export_specifiers,
                module_transform_facts,
                compiler_options,
                file_name,
                module_format,
                source_file_contains_dynamic_import,
                initializer,
            )
        });

        if crate::utilities::is_local_name(emit_context, &name) {
            let initializer = initializer.map(|initializer| {
                let export_name = emit_context.factory.node_factory.new_identifier(&name_text);
                let left = exports_property_access_for_name(
                    &mut emit_context.factory.node_factory,
                    export_name,
                );
                let equals = emit_context
                    .factory
                    .node_factory
                    .new_token(ast::Kind::EqualsToken);
                emit_context.factory.node_factory.new_binary_expression(
                    None,
                    left,
                    None,
                    equals,
                    initializer,
                )
            });
            let declaration = emit_context
                .factory
                .node_factory
                .update_variable_declaration(
                    declaration,
                    name,
                    exclamation_token,
                    None::<ast::Node>,
                    initializer,
                );
            preserved_variables.push(declaration);
        } else if let Some(initializer) = initializer {
            if matches!(
                initializer_kind,
                Some(
                    ast::Kind::ArrowFunction
                        | ast::Kind::FunctionExpression
                        | ast::Kind::ClassExpression
                )
            ) {
                // preserve variable declarations for functions and classes to assign names

                let declaration = emit_context.factory.node_factory.new_variable_declaration(
                    name,
                    exclamation_token,
                    type_node,
                    Some(initializer),
                );
                preserved_variables.push(declaration);
                let export_name = clone_export_property_name(
                    source,
                    &mut emit_context.factory.node_factory,
                    name,
                );
                let left = exports_property_access_for_name(
                    &mut emit_context.factory.node_factory,
                    export_name,
                );
                emit_context.assign_comment_and_source_map_ranges(&export_name, &name);
                emit_context.assign_comment_and_source_map_ranges(&left, &name);
                let equals = emit_context
                    .factory
                    .node_factory
                    .new_token(ast::Kind::EqualsToken);
                let right = emit_context.factory.node_factory.new_identifier(&name_text);
                let assignment = emit_context
                    .factory
                    .node_factory
                    .new_binary_expression(None, left, None, equals, right);
                let assignment_statement = emit_context
                    .factory
                    .node_factory
                    .new_expression_statement(assignment);
                emit_context.assign_comment_and_source_map_ranges(
                    &assignment_statement,
                    &original_statement,
                );
                statements.push(assignment_statement);
            } else {
                let export_name = clone_export_property_name(
                    source,
                    &mut emit_context.factory.node_factory,
                    name,
                );
                let left = exports_property_access_for_name(
                    &mut emit_context.factory.node_factory,
                    export_name,
                );
                emit_context.assign_comment_and_source_map_ranges(&export_name, &name);
                emit_context.assign_comment_and_source_map_ranges(&left, &name);
                let equals = emit_context
                    .factory
                    .node_factory
                    .new_token(ast::Kind::EqualsToken);
                let assignment = emit_context.factory.node_factory.new_binary_expression(
                    None,
                    left,
                    None,
                    equals,
                    initializer,
                );
                let assignment_statement = emit_context
                    .factory
                    .node_factory
                    .new_expression_statement(assignment);
                emit_context.assign_comment_and_source_map_ranges(
                    &assignment_statement,
                    &original_statement,
                );
                statements.push(assignment_statement);
            }
            if let Some(export_names) = local_export_specifiers.get(&name_text) {
                let use_exported_local = !matches!(
                    initializer_kind,
                    Some(
                        ast::Kind::ArrowFunction
                            | ast::Kind::FunctionExpression
                            | ast::Kind::ClassExpression
                    )
                );
                for export_name in export_names {
                    statements.push(create_export_assignment_statement(
                        &mut emit_context.factory.node_factory,
                        export_name,
                        &name_text,
                        use_exported_local,
                    ));
                }
            }
        }
    }

    if !preserved_variables.is_empty() {
        for statement in &statements {
            emit_context.mark_emit_node(statement, printer::EF_NO_COMMENTS);
        }
        let modifiers = active_stripped_export_modifiers(emit_context, statement);
        let declarations = emit_context
            .factory
            .node_factory
            .new_node_list_with_trailing_comma(
                declarations_loc,
                declarations_range,
                preserved_variables,
                declarations_has_trailing_comma,
            );
        let declaration_list = emit_context
            .factory
            .node_factory
            .new_variable_declaration_list(declarations, declaration_list_flags);
        statements.insert(
            0,
            emit_context.factory.node_factory.update_variable_statement(
                statement,
                modifiers,
                declaration_list,
            ),
        );
    }

    statements
}

fn strip_export_from_class_declaration(
    file: &ast::SourceFile,
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    compiler_options: &core::CompilerOptions,
    statement: &ast::Node,
) -> ast::Node {
    let (modifiers, heritage_clauses, members) = {
        let mut importer = ast::AstImporter::new(source, &mut emit_context.factory.node_factory);
        (
            strip_export_modifiers(&mut importer, source.source_modifiers(*statement)),
            importer.preserve_optional_source_node_list(source.source_heritage_clauses(*statement)),
            {
                let members = source
                    .source_members(*statement)
                    .expect("class declaration should have members");
                let mut member_nodes = members.iter().collect::<Vec<_>>();
                if crate::estransforms::classfields::class_field_transform_config(
                    compiler_options.get_emit_script_target(),
                    compiler_options.get_use_define_for_class_fields(),
                    compiler_options.experimental_decorators.is_true(),
                )
                .is_some_and(|config| config.should_transform_initializers)
                {
                    member_nodes.retain(|member| {
                        !is_public_static_property_with_initializer(source, *member)
                    });
                }
                let preserved_members = member_nodes
                    .into_iter()
                    .map(|member| importer.preserve_node(member))
                    .collect::<Vec<_>>();
                let members = importer.factory().new_node_list(
                    members.loc(),
                    members.range(),
                    preserved_members,
                );
                members
            },
        )
    };
    let name = emit_context.factory.get_declaration_name(source, statement);
    let stripped = emit_context
        .factory
        .node_factory
        .update_class_declaration_from_store(
            source,
            *statement,
            modifiers,
            name,
            None,
            heritage_clauses,
            members,
        );
    crate::tstransforms::typeeraser::visit_source_file_root(
        file,
        stripped,
        emit_context,
        compiler_options,
    )
}

fn transform_exported_class_declaration_to_common_js(
    file: &ast::SourceFile,
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    statement: &ast::Node,
    active_statement: Option<ast::Node>,
    import_references: &HashMap<String, ImportReference>,
    compiler_options: &core::CompilerOptions,
    file_name: &str,
    module_format: core::ModuleKind,
    source_file_contains_dynamic_import: bool,
    direct_exported_names: &HashSet<String>,
    local_export_specifiers: &HashMap<String, Vec<String>>,
    module_transform_facts: &ModuleTransformFacts,
) -> ast::Node {
    let active_statement = active_statement.filter(|active_statement| {
        emit_context
            .factory
            .node_factory
            .store()
            .kind(*active_statement)
            == ast::Kind::ClassDeclaration
    });
    if import_references.is_empty()
        && direct_exported_names.is_empty()
        && local_export_specifiers.is_empty()
        && !source_file_contains_dynamic_import
        && !compiler_options
            .rewrite_relative_import_extensions
            .is_true()
    {
        if let Some(active_statement) = active_statement {
            return strip_export_from_active_class_declaration(
                file,
                emit_context,
                compiler_options,
                active_statement,
            );
        }
        return strip_export_from_class_declaration(
            file,
            source,
            emit_context,
            compiler_options,
            statement,
        );
    }

    let (modifiers, name) = if let Some(active_statement) = active_statement {
        (
            active_stripped_export_modifiers(emit_context, active_statement),
            active_declaration_name(emit_context, active_statement),
        )
    } else {
        let mut importer = ast::AstImporter::new(source, &mut emit_context.factory.node_factory);
        (
            strip_export_modifiers(&mut importer, source.source_modifiers(*statement)),
            emit_context.factory.get_declaration_name(source, statement),
        )
    };
    let mut rewriter = CommonJsReferenceRewriter {
        source,
        emit_context,
        import_state: ast::AstImportState::new(),
        import_references,
        local_export_specifiers,
        direct_exported_names,
        module_transform_facts,
        compiler_options,
        file_name,
        module_format,
        source_file_contains_dynamic_import,
        current_node: None,
        parent_node: None,
    };
    let transformed = if let Some(active_statement) = active_statement {
        let heritage_clauses = rewriter.visit_nodes_input(
            rewriter
                .factory()
                .store()
                .source_heritage_clauses(active_statement)
                .map(ast::SourceNodeListInput::from_source),
        );
        let members = rewriter
            .visit_nodes_input(
                rewriter
                    .factory()
                    .store()
                    .source_members(active_statement)
                    .map(ast::SourceNodeListInput::from_source),
            )
            .expect("class declaration should have members");
        rewriter.factory_mut().update_class_declaration(
            active_statement,
            modifiers,
            name,
            None,
            heritage_clauses,
            members,
        )
    } else {
        let heritage_clauses = rewriter.visit_nodes_input(
            (source.source_heritage_clauses(*statement)).map(ast::SourceNodeListInput::from_source),
        );
        let members = rewriter
            .visit_nodes_input(
                (source.source_members(*statement)).map(ast::SourceNodeListInput::from_source),
            )
            .expect("class declaration should have members");
        rewriter.factory_mut().update_class_declaration_from_store(
            source,
            *statement,
            modifiers,
            name,
            None,
            heritage_clauses,
            members,
        )
    };
    crate::tstransforms::typeeraser::visit_source_file_root(
        file,
        transformed,
        emit_context,
        compiler_options,
    )
}

fn strip_export_from_function_declaration(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    statement: &ast::Node,
) -> ast::Node {
    let (modifiers, asterisk_token, parameters, body) = {
        let mut importer = ast::AstImporter::new(source, &mut emit_context.factory.node_factory);
        (
            strip_export_modifiers(&mut importer, source.source_modifiers(*statement)),
            source
                .asterisk_token(*statement)
                .map(|node| importer.preserve_node(node)),
            importer.preserve_source_node_list(
                source
                    .source_parameters(*statement)
                    .expect("function declaration should have parameters"),
            ),
            source
                .body(*statement)
                .map(|node| importer.preserve_node(node)),
        )
    };
    let name = emit_context.factory.get_declaration_name(source, statement);
    emit_context
        .factory
        .node_factory
        .update_function_declaration_from_store(
            source,
            *statement,
            modifiers,
            asterisk_token,
            name,
            None,
            parameters,
            None,
            None,
            body,
        )
}

fn transform_exported_function_declaration_to_common_js(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    statement: &ast::Node,
    import_references: &HashMap<String, ImportReference>,
    compiler_options: &core::CompilerOptions,
    file_name: &str,
    module_format: core::ModuleKind,
    source_file_contains_dynamic_import: bool,
    direct_exported_names: &HashSet<String>,
    local_export_specifiers: &HashMap<String, Vec<String>>,
    module_transform_facts: &ModuleTransformFacts,
) -> ast::Node {
    if import_references.is_empty()
        && direct_exported_names.is_empty()
        && local_export_specifiers.is_empty()
        && !source_file_contains_dynamic_import
        && !compiler_options
            .rewrite_relative_import_extensions
            .is_true()
    {
        return strip_export_from_function_declaration(source, emit_context, statement);
    }

    let (modifiers, asterisk_token) = {
        let mut importer = ast::AstImporter::new(source, &mut emit_context.factory.node_factory);
        (
            strip_export_modifiers(&mut importer, source.source_modifiers(*statement)),
            source
                .asterisk_token(*statement)
                .map(|node| importer.preserve_node(node)),
        )
    };
    let name = emit_context.factory.get_declaration_name(source, statement);
    let mut rewriter = CommonJsReferenceRewriter {
        source,
        emit_context,
        import_state: ast::AstImportState::new(),
        import_references,
        local_export_specifiers,
        direct_exported_names,
        module_transform_facts,
        compiler_options,
        file_name,
        module_format,
        source_file_contains_dynamic_import,
        current_node: None,
        parent_node: None,
    };
    let parameters = rewriter
        .visit_nodes_input(
            (source.source_parameters(*statement)).map(ast::SourceNodeListInput::from_source),
        )
        .expect("function declaration should have parameters");
    let body = rewriter.visit_node(source.body(*statement));
    rewriter
        .factory_mut()
        .update_function_declaration_from_store(
            source,
            *statement,
            modifiers,
            asterisk_token,
            name,
            None,
            parameters,
            None,
            None,
            body,
        )
}

fn create_export_statement_for_declaration(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    statement: &ast::Node,
) -> Option<ast::Node> {
    let (export_name, declaration_name) = {
        let factory = &mut emit_context.factory;
        if ast::has_syntactic_modifier(source, *statement, ast::ModifierFlags::DEFAULT) {
            (factory.node_factory.new_identifier("default"), None)
        } else {
            let declaration_name = factory.get_declaration_name(source, statement);
            (declaration_name, Some(declaration_name))
        }
    };
    let right = emit_context.factory.get_local_name(source, statement);
    let factory = &mut emit_context.factory.node_factory;
    let left = exports_property_access_for_name(factory, export_name);
    let equals = factory.new_token(ast::Kind::EqualsToken);
    let assignment = factory.new_binary_expression(None, left, None, equals, right);
    let statement_node = factory.new_expression_statement(assignment);
    if let Some(declaration_name) = declaration_name {
        emit_context.assign_comment_and_source_map_ranges(&export_name, &declaration_name);
        emit_context.assign_comment_and_source_map_ranges(&left, &declaration_name);
        emit_context.assign_comment_and_source_map_ranges(&assignment, &declaration_name);
        emit_context.mark_emit_node(&assignment, printer::EF_NO_TRAILING_SOURCE_MAP);
    }
    emit_context.set_comment_range(&statement_node, source.loc(*statement));
    emit_context.mark_emit_node(
        &statement_node,
        printer::EF_START_ON_NEW_LINE | printer::EF_NO_COMMENTS,
    );
    Some(statement_node)
}

fn transform_exported_variable_statement(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    statement: &ast::Node,
    active_statement: Option<ast::Node>,
    import_references: &HashMap<String, ImportReference>,
    compiler_options: &core::CompilerOptions,
    file_name: &str,
    module_format: core::ModuleKind,
    source_file_contains_dynamic_import: bool,
    direct_exported_names: &HashSet<String>,
    local_export_specifiers: &HashMap<String, Vec<String>>,
    module_transform_facts: &ModuleTransformFacts,
) -> Vec<ast::Node> {
    let Some(declaration_list) = source.declaration_list(*statement) else {
        return Vec::new();
    };
    let mut statements = Vec::new();
    let declarations = source
        .declarations(declaration_list)
        .expect("variable declaration list should have declarations");
    let active_declarations = active_statement.and_then(|active_statement| {
        let active_source = emit_context.factory.node_factory.store();
        let declaration_list = active_source.declaration_list(active_statement)?;
        let declarations = active_source.declarations(declaration_list)?;
        Some(declarations.iter().collect::<Vec<_>>())
    });

    let mut preserved_variables = Vec::new();
    for (declaration_index, declaration_node) in declarations.iter().enumerate() {
        let Some(name) = source.name(declaration_node) else {
            continue;
        };

        if ast::is_identifier(source, name) && crate::utilities::is_local_name(emit_context, &name)
        {
            let initializer = source.initializer(declaration_node).map(|initializer| {
                let initializer = rewrite_common_js_expression(
                    source,
                    emit_context,
                    import_references,
                    direct_exported_names,
                    local_export_specifiers,
                    module_transform_facts,
                    compiler_options,
                    file_name,
                    module_format,
                    source_file_contains_dynamic_import,
                    initializer,
                );
                let mut importer =
                    ast::AstImporter::new(source, &mut emit_context.factory.node_factory);
                let initializer = importer.preserve_node(initializer);
                let export_name_text = source.text(name);
                let export_name = importer.factory().new_identifier(export_name_text);
                let left = exports_property_access_for_name(importer.factory(), export_name);
                let equals = importer.factory().new_token(ast::Kind::EqualsToken);
                importer
                    .factory()
                    .new_binary_expression(None, left, None, equals, initializer)
            });

            let mut importer =
                ast::AstImporter::new(source, &mut emit_context.factory.node_factory);
            let name = importer.preserve_node(name);
            let exclamation_token = source
                .exclamation_token(declaration_node)
                .map(|node| importer.preserve_node(node));
            let type_node = source
                .r#type(declaration_node)
                .map(|node| importer.preserve_node(node));
            let declaration = importer.factory().update_variable_declaration_from_store(
                source,
                declaration_node,
                name,
                exclamation_token,
                type_node,
                initializer,
            );
            preserved_variables.push(declaration);
        } else if let Some(initializer) = source.initializer(declaration_node)
            && ast::is_identifier(source, name)
            && (ast::is_arrow_function(source, initializer)
                || ast::is_function_expression(source, initializer)
                || ast::is_class_expression(source, initializer))
        {
            let initializer = rewrite_common_js_expression(
                source,
                emit_context,
                import_references,
                direct_exported_names,
                local_export_specifiers,
                module_transform_facts,
                compiler_options,
                file_name,
                module_format,
                source_file_contains_dynamic_import,
                initializer,
            );
            let (declaration, assignment_statement, left, export_name) = {
                let mut importer =
                    ast::AstImporter::new(source, &mut emit_context.factory.node_factory);
                let declaration_name = importer.preserve_node(name);
                let exclamation_token = source
                    .exclamation_token(declaration_node)
                    .map(|node| importer.preserve_node(node));
                let type_node = source
                    .r#type(declaration_node)
                    .map(|node| importer.preserve_node(node));
                let initializer = importer.preserve_node(initializer);
                let declaration = importer.factory().new_variable_declaration(
                    declaration_name,
                    exclamation_token,
                    type_node,
                    initializer,
                );

                let export_name_text = source.text(name);
                let export_name = importer.factory().new_identifier(export_name_text);
                let left = exports_property_access_for_name(importer.factory(), export_name);
                let equals = importer.factory().new_token(ast::Kind::EqualsToken);
                let right = importer.preserve_node(name);
                let assignment = importer
                    .factory()
                    .new_binary_expression(None, left, None, equals, right);
                let assignment_statement = importer.factory().new_expression_statement(assignment);
                (declaration, assignment_statement, left, export_name)
            };
            emit_context.assign_comment_and_source_map_ranges(&export_name, &name);
            emit_context.assign_comment_and_source_map_ranges(&left, &name);
            emit_context.assign_comment_and_source_map_ranges(&assignment_statement, &statement);
            preserved_variables.push(declaration);
            statements.push(assignment_statement);
        } else if let Some(initializer) = source.initializer(declaration_node)
            && ast::is_identifier(source, name)
        {
            let initializer = rewrite_common_js_expression(
                source,
                emit_context,
                import_references,
                direct_exported_names,
                local_export_specifiers,
                module_transform_facts,
                compiler_options,
                file_name,
                module_format,
                source_file_contains_dynamic_import,
                initializer,
            );
            let (assignment_statement, left, export_name) = {
                let mut importer =
                    ast::AstImporter::new(source, &mut emit_context.factory.node_factory);
                let initializer = importer.preserve_node(initializer);
                let export_name = clone_export_property_name(source, importer.factory(), name);
                let left = exports_property_access_for_name(importer.factory(), export_name);
                let equals = importer.factory().new_token(ast::Kind::EqualsToken);
                let assignment =
                    importer
                        .factory()
                        .new_binary_expression(None, left, None, equals, initializer);
                let assignment_statement = importer.factory().new_expression_statement(assignment);
                (assignment_statement, left, export_name)
            };
            emit_context.assign_comment_and_source_map_ranges(&export_name, &name);
            emit_context.assign_comment_and_source_map_ranges(&left, &name);
            emit_context.assign_comment_and_source_map_ranges(&assignment_statement, &statement);
            statements.push(assignment_statement);
        } else if let Some(_initializer) = source.initializer(declaration_node)
            && ast::is_binding_pattern(source, name)
        {
            // For binding patterns with export modifier, use flattenDestructuringAssignment
            // to decompose into individual export assignments
            let mut create_all_export_expressions =
                |emit_context: &mut printer::EmitContext,
                 name: ast::Node,
                 value: ast::Node,
                 location: core::TextRange| {
                    let name_text = emit_context.store_for_node(name).text(name).to_owned();
                    let left =
                        exports_property_access(&mut emit_context.factory.node_factory, &name_text);
                    let expression = emit_context.factory.new_assignment_expression(left, value);
                    emit_context
                        .factory
                        .node_factory
                        .place_emit_synthetic_node(expression, location);
                    emit_context.assign_comment_and_source_map_ranges(&expression, &name);
                    expression
                };
            let flatten_declaration = active_declarations
                .as_ref()
                .and_then(|declarations| declarations.get(declaration_index).copied())
                .unwrap_or(declaration_node);
            let expression = crate::destructuring::flatten_destructuring_assignment(
                source,
                emit_context,
                flatten_declaration,
                false,
                crate::destructuring::FlattenLevel::All,
                Some(&mut create_all_export_expressions),
            );
            let statement = emit_context
                .factory
                .node_factory
                .new_expression_statement(expression);
            statements.push(statement);
        }
    }

    if !preserved_variables.is_empty() {
        for statement in &statements {
            emit_context.mark_emit_node(statement, printer::EF_NO_COMMENTS);
        }
        let mut importer = ast::AstImporter::new(source, &mut emit_context.factory.node_factory);
        let modifiers = strip_export_modifiers(&mut importer, source.source_modifiers(*statement));
        let declarations = importer.factory().new_node_list(
            declarations.loc(),
            declarations.range(),
            preserved_variables,
        );
        let declaration_list = importer
            .factory()
            .new_variable_declaration_list(declarations, source.flags(declaration_list));
        let variable_statement = importer.factory().update_variable_statement_from_store(
            source,
            *statement,
            modifiers,
            declaration_list,
        );
        statements.insert(0, variable_statement);
    }
    statements
}

fn transform_export_default_assignment(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    statement: &ast::Node,
    import_references: &HashMap<String, ImportReference>,
    compiler_options: &core::CompilerOptions,
    file_name: &str,
    module_format: core::ModuleKind,
    source_file_contains_dynamic_import: bool,
    direct_exported_names: &HashSet<String>,
    local_export_specifiers: &HashMap<String, Vec<String>>,
    module_transform_facts: &ModuleTransformFacts,
) -> ast::Node {
    let expression = source
        .expression(*statement)
        .expect("export assignment should have expression");
    transform_export_default_assignment_expression(
        source,
        emit_context,
        expression,
        source.loc(*statement),
        import_references,
        compiler_options,
        file_name,
        module_format,
        source_file_contains_dynamic_import,
        direct_exported_names,
        local_export_specifiers,
        module_transform_facts,
    )
}

fn active_export_assignment_input(
    emit_context: &printer::EmitContext,
    statement: ast::Node,
) -> Option<(ast::Node, core::TextRange)> {
    let source = emit_context.store_for_node(statement);
    if !ast::is_export_assignment(source, statement)
        || source.is_export_equals(statement).unwrap_or(false)
    {
        return None;
    }
    Some((
        source
            .expression(statement)
            .expect("export assignment should have expression"),
        source.loc(statement),
    ))
}

fn transform_export_default_assignment_expression(
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
    expression: ast::Node,
    loc: core::TextRange,
    import_references: &HashMap<String, ImportReference>,
    compiler_options: &core::CompilerOptions,
    file_name: &str,
    module_format: core::ModuleKind,
    source_file_contains_dynamic_import: bool,
    direct_exported_names: &HashSet<String>,
    local_export_specifiers: &HashMap<String, Vec<String>>,
    module_transform_facts: &ModuleTransformFacts,
) -> ast::Node {
    let expression = {
        let expression_name = {
            let expression_source = ast::AstImportState::store_for(
                source,
                &emit_context.factory.node_factory,
                expression,
            );
            if ast::is_identifier(expression_source, expression) {
                Some(expression_source.text(expression).to_owned())
            } else {
                None
            }
        };
        if let Some(reference) = expression_name
            .as_deref()
            .and_then(|name| import_references.get(name))
            .copied()
        {
            imported_reference_expression_from_identifier(emit_context, reference, expression)
        } else {
            rewrite_common_js_expression(
                source,
                emit_context,
                import_references,
                direct_exported_names,
                local_export_specifiers,
                module_transform_facts,
                compiler_options,
                file_name,
                module_format,
                source_file_contains_dynamic_import,
                expression,
            )
        }
    };
    create_export_statement(emit_context, "default", expression, Some(loc), true)
}

fn transform_import_equals_declaration_to_require(
    source: &ast::AstStore,
    importer: &mut ast::AstImporter<'_, '_>,
    statement: &ast::Node,
    compiler_options: &core::CompilerOptions,
) -> ast::Node {
    let require_call = create_require_call(source, importer, statement, compiler_options);
    if ast::has_syntactic_modifier(source, *statement, ast::ModifierFlags::EXPORT) {
        let name = source
            .name(*statement)
            .expect("external import= declaration should have a name");
        let left = exports_property_access(importer.factory(), &source.text(name));
        let equals = importer.factory().new_token(ast::Kind::EqualsToken);
        let assignment =
            importer
                .factory()
                .new_binary_expression(None, left, None, equals, require_call);
        importer.factory().new_expression_statement(assignment)
    } else {
        let name = source
            .name(*statement)
            .expect("external import= declaration should have a name");
        let name = importer.preserve_node(name);
        let declaration =
            importer
                .factory()
                .new_variable_declaration(name, None, None, require_call);
        let declarations = importer.factory().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![declaration],
        );
        let declaration_list = importer
            .factory()
            .new_variable_declaration_list(declarations, ast::NodeFlags::CONST);
        importer
            .factory()
            .new_variable_statement(None, declaration_list)
    }
}

fn create_require_call(
    source: &ast::AstStore,
    importer: &mut ast::AstImporter<'_, '_>,
    statement: &ast::Node,
    compiler_options: &core::CompilerOptions,
) -> ast::Node {
    let require = importer.factory().new_identifier("require");
    let arguments = if let Some(module_name) =
        ast::get_external_module_import_equals_declaration_expression(source, *statement)
    {
        let text = source.text(module_name);
        let text = crate::moduletransforms::utilities::rewrite_module_specifier_text(
            &text,
            compiler_options,
        )
        .unwrap_or(text);
        vec![
            importer
                .factory()
                .new_string_literal(text, ast::TokenFlags::NONE),
        ]
    } else {
        Vec::new()
    };
    let arguments = importer.factory().new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        arguments,
    );
    importer
        .factory()
        .new_call_expression(require, None, None, arguments, ast::NodeFlags::NONE)
}

fn create_module_exports_assignment(
    factory: &mut ast::NodeFactory,
    expression: ast::Node,
) -> ast::Node {
    let left = module_exports_property_access(factory);
    let equals = factory.new_token(ast::Kind::EqualsToken);
    let assignment = factory.new_binary_expression(None, left, None, equals, expression);
    factory.new_expression_statement(assignment)
}

fn create_empty_imports(factory: &mut ast::NodeFactory) -> ast::Node {
    let elements = factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        Vec::<ast::Node>::new(),
    );
    let named_exports = factory.new_named_exports(elements);
    factory.new_export_declaration(None, false, named_exports, None, None)
}

pub(crate) fn visit_implied_module_source_file_root_output(
    file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
    compiler_options: &core::CompilerOptions,
    file_module_format: core::ModuleKind,
    facts: ModuleTransformFacts,
) -> Option<ast::Node> {
    let is_declaration_file =
        emit_context.with_source_file_view(root, |source_file| source_file.is_declaration_file());
    if is_declaration_file {
        return None;
    }

    if file_module_format >= core::ModuleKind::ES2015 {
        visit_es_module_source_file_root_output(
            file,
            root,
            emit_context,
            compiler_options,
            file_module_format,
        )
    } else {
        visit_common_js_module_source_file_root_output(
            file,
            root,
            emit_context,
            compiler_options,
            file_module_format,
            facts,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_file(file_name: &str, is_declaration_file: bool) -> ast::SourceFile {
        let mut factory = ast::NodeFactory::default();
        let statements = factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            Vec::new(),
        );
        let root = factory.new_source_file(
            ast::SourceFileParseOptions {
                file_name: file_name.to_string(),
                path: file_name.to_string(),
                ..Default::default()
            },
            String::new(),
            statements,
            None,
        );
        factory.finish_parsed_source_file(
            root,
            ast::ParsedSourceFileMetadata {
                is_declaration_file,
                ..Default::default()
            },
        )
    }

    fn transform_with(
        source_file_transformer: SourceFileTransformer,
        file: &ast::SourceFile,
    ) -> ast::SourceFile {
        let mut transformer = Transformer::default();
        transformer.new_source_file_transformer(
            source_file_transformer,
            Some(printer::new_emit_context()),
        );
        transformer.transform_source_file(file)
    }

    #[test]
    fn module_source_file_visitors_preserve_go_skip_gates() {
        let mut options = core::CompilerOptions::default();
        options.module = core::ModuleKind::CommonJS;

        let declaration_file = source_file("/a.d.ts", true);
        assert_eq!(
            transform_with(
                SourceFileTransformer::EsModule {
                    compiler_options: options.clone(),
                    file_module_format: options.get_emit_module_kind(),
                },
                &declaration_file,
            )
            .data()
            .file_name(),
            declaration_file.data().file_name()
        );
        assert_eq!(
            transform_with(
                SourceFileTransformer::CommonJsModule {
                    compiler_options: options.clone(),
                    facts: ModuleTransformFacts::default(),
                },
                &declaration_file,
            )
            .data()
            .file_name(),
            declaration_file.data().file_name()
        );

        let script_file = source_file("/a.ts", false);
        assert_eq!(
            transform_with(
                SourceFileTransformer::CommonJsModule {
                    compiler_options: options.clone(),
                    facts: ModuleTransformFacts::default(),
                },
                &script_file,
            )
            .data()
            .file_name(),
            script_file.data().file_name()
        );
    }

    #[test]
    fn implied_module_uses_go_file_format_worker() {
        let mut options = core::CompilerOptions::default();
        options.module = core::ModuleKind::NodeNext;

        let esm_file = source_file("/a.mts", false);
        let cjs_file = source_file("/a.cts", false);

        assert_eq!(
            ast::get_emit_module_format_of_file_worker(
                esm_file.data().file_name_ref(),
                &options,
                ast::SourceFileMetaData {
                    implied_node_format: ast::get_implied_node_format_for_file(
                        esm_file.data().file_name_ref(),
                        ""
                    ),
                    ..Default::default()
                }
            ),
            core::ModuleKind::ESNext
        );
        assert_eq!(
            ast::get_emit_module_format_of_file_worker(
                cjs_file.data().file_name_ref(),
                &options,
                ast::SourceFileMetaData {
                    implied_node_format: ast::get_implied_node_format_for_file(
                        cjs_file.data().file_name_ref(),
                        ""
                    ),
                    ..Default::default()
                }
            ),
            core::ModuleKind::CommonJS
        );

        assert_eq!(
            transform_with(
                SourceFileTransformer::ImpliedModule {
                    compiler_options: options.clone(),
                    file_module_format: ast::get_emit_module_format_of_file_worker(
                        esm_file.data().file_name_ref(),
                        &options,
                        ast::SourceFileMetaData {
                            implied_node_format: ast::get_implied_node_format_for_file(
                                esm_file.data().file_name_ref(),
                                ""
                            ),
                            ..Default::default()
                        },
                    ),
                    facts: ModuleTransformFacts::default(),
                },
                &esm_file,
            )
            .data()
            .file_name(),
            esm_file.data().file_name()
        );
        assert_eq!(
            transform_with(
                SourceFileTransformer::ImpliedModule {
                    compiler_options: options.clone(),
                    file_module_format: ast::get_emit_module_format_of_file_worker(
                        cjs_file.data().file_name_ref(),
                        &options,
                        ast::SourceFileMetaData {
                            implied_node_format: ast::get_implied_node_format_for_file(
                                cjs_file.data().file_name_ref(),
                                ""
                            ),
                            ..Default::default()
                        },
                    ),
                    facts: ModuleTransformFacts::default(),
                },
                &cjs_file,
            )
            .data()
            .file_name(),
            cjs_file.data().file_name()
        );
    }
}
