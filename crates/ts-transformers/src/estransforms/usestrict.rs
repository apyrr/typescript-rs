use ts_ast as ast;
use ts_core::{CompilerOptions, ModuleKind, ScriptKind};
use ts_printer as printer;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UseStrictAction {
    Keep,
    EnsureUseStrict,
}

pub fn use_strict_action(
    script_kind: ScriptKind,
    is_external_module: bool,
    emit_module_kind: ModuleKind,
    file_module_format: ModuleKind,
) -> UseStrictAction {
    if script_kind == ScriptKind::JSON {
        return UseStrictAction::Keep;
    }

    // ESM is always strict. If the file is ESM, and CJS emit
    // has not been requested, then skip adding "use strict".
    let esm_is_already_strict = is_external_module
        && emit_module_kind >= ModuleKind::ES2015
        && (emit_module_kind == ModuleKind::Preserve || file_module_format >= ModuleKind::ES2015);
    if esm_is_already_strict {
        UseStrictAction::Keep
    } else {
        UseStrictAction::EnsureUseStrict
    }
}

pub fn should_insert_use_strict(
    script_kind: ScriptKind,
    is_external_module: bool,
    emit_module_kind: ModuleKind,
    file_module_format: ModuleKind,
) -> bool {
    use_strict_action(
        script_kind,
        is_external_module,
        emit_module_kind,
        file_module_format,
    ) == UseStrictAction::EnsureUseStrict
}

pub(crate) fn visit_source_file_output(
    node: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
    compiler_options: &CompilerOptions,
    file_module_format: ModuleKind,
) -> Option<ast::Node> {
    let factory_store_id = emit_context.factory.node_factory.store().store_id();
    let root_is_active = root.store_id() == factory_store_id;
    let script_kind = if root_is_active {
        emit_context
            .factory
            .node_factory
            .store()
            .source_file_view(root)
            .script_kind()
    } else {
        assert_eq!(
            root.store_id(),
            node.store().store_id(),
            "UseStrict transform root must come from the input source or active emit factory"
        );
        node.script_kind()
    };
    if script_kind == ScriptKind::JSON {
        return None;
    }

    let is_external_module = if root_is_active {
        let source_file = emit_context
            .factory
            .node_factory
            .store()
            .source_file_view(root);
        ast::is_external_module(&source_file)
    } else {
        ast::is_external_module(node)
    };
    let module_kind = compiler_options.get_emit_module_kind();

    if use_strict_action(
        script_kind,
        is_external_module,
        module_kind,
        file_module_format,
    ) == UseStrictAction::Keep
    {
        return None;
    }

    if root_is_active {
        let (statements_loc, statements_range, mut statements, end_of_file_token, has_use_strict) = {
            let source = emit_context.factory.node_factory.store();
            let statement_list = source
                .source_statements(root)
                .expect("source file should have statements");
            let statement_nodes: Vec<_> = statement_list.iter().collect();
            let has_use_strict = statement_nodes.first().is_some_and(|statement| {
                ast::is_prologue_directive(source, *statement)
                    && source
                        .expression(*statement)
                        .is_some_and(|expr| source.text(expr) == "use strict")
            });
            (
                statement_list.loc(),
                statement_list.range(),
                statement_nodes,
                source.source_file_view(root).end_of_file_token(),
                has_use_strict,
            )
        };
        if !has_use_strict {
            let use_strict_literal = emit_context
                .factory
                .node_factory
                .new_string_literal("use strict", ast::TokenFlags::NONE);
            let use_strict_prologue = emit_context
                .factory
                .node_factory
                .new_expression_statement(use_strict_literal);
            statements.insert(0, use_strict_prologue);
        }
        let statements = emit_context.factory.node_factory.new_node_list(
            statements_loc,
            statements_range,
            statements,
        );
        return Some(
            emit_context
                .factory
                .node_factory
                .update_source_file_in_current_store(root, statements, end_of_file_token),
        );
    }

    let source = node.store();
    let source_file = source.as_source_file(root);
    let statement_list = source
        .source_statements(root)
        .expect("source file should have statements");
    let statement_nodes: Vec<_> = statement_list.iter().collect();
    let statements = emit_context
        .factory
        .ensure_use_strict(source, &statement_nodes)
        .into_iter()
        .collect::<Vec<_>>();
    let mut preserved_pairs = Vec::new();
    let updated = {
        let mut importer = ast::AstImporter::new(source, &mut emit_context.factory.node_factory);
        let statements: Vec<_> = statements
            .into_iter()
            .map(|statement| {
                let imported = importer.preserve_node(statement);
                preserved_pairs.push((statement, imported));
                imported
            })
            .collect();
        let statements = importer.factory().new_node_list(
            statement_list.loc(),
            statement_list.range(),
            statements,
        );
        let end_of_file_token = importer.preserve_optional_node(source_file.end_of_file_token());
        importer.update_source_file(root, Some(statements), end_of_file_token)
    };
    for (original, imported) in preserved_pairs {
        if original.store_id() == source.store_id() && original.store_id() != imported.store_id() {
            crate::utilities::copy_originals_for_preserved_subtree(
                emit_context,
                original,
                imported,
            );
        }
    }
    Some(updated)
}
