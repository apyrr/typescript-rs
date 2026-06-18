use ts_ast as ast;
use ts_core as core;
use ts_tspath as tspath;

#[derive(Default)]
pub(crate) struct ParsedExternalModuleReferences {
    pub(crate) imports: Vec<ast::Node>,
    pub(crate) module_augmentations: Vec<ast::Node>,
    pub(crate) ambient_module_names: Vec<String>,
    pub(crate) uses_uri_style_node_core_modules: core::Tristate,
}

pub(crate) fn collect_external_module_references(
    store: &ast::AstStore,
    root: ast::Node,
    source_flags: ast::NodeFlags,
    is_declaration_file: bool,
    external_module_indicator: Option<ast::Node>,
) -> ParsedExternalModuleReferences {
    let mut result = ParsedExternalModuleReferences {
        uses_uri_style_node_core_modules: core::Tristate::Unknown,
        ..Default::default()
    };

    let statements = store.parser_access().source_file_statement_nodes(root);
    for node in statements {
        collect_module_references(
            store,
            node,
            false,
            is_declaration_file,
            external_module_indicator.is_some(),
            &mut result,
        );
    }

    let file_flags = store.flags(root) | source_flags;
    if file_flags.contains(ast::NodeFlags::POSSIBLY_CONTAINS_DYNAMIC_IMPORT)
        || file_flags.contains(ast::NodeFlags::JAVA_SCRIPT_FILE)
    {
        ast::for_each_dynamic_import_or_require_call(
            store,
            root,
            true,
            true,
            |_, module_specifier| {
                result.imports.push(module_specifier);
                false
            },
        );
    }

    result
}

fn collect_module_references(
    store: &ast::AstStore,
    node: ast::Node,
    in_ambient_module: bool,
    is_declaration_file: bool,
    is_external_module: bool,
    result: &mut ParsedExternalModuleReferences,
) {
    if ast::is_any_import_or_re_export(store, node) {
        let module_name_expr = ast::get_external_module_name(store, node);
        // TypeScript 1.0 spec (April 2014): 12.1.6
        // An ExternalImportDeclaration in an AmbientExternalModuleDeclaration may reference other external modules
        // only through top-level external module names. Relative external module names are not permitted.
        if let Some(module_name_expr) = module_name_expr
            && ast::is_string_literal(store, module_name_expr)
        {
            let module_name = store.text(module_name_expr);
            if !module_name.is_empty()
                && (!in_ambient_module || !tspath::is_external_module_name_relative(&module_name))
            {
                result.imports.push(module_name_expr);
                if result.uses_uri_style_node_core_modules != core::Tristate::True
                    && !is_declaration_file
                {
                    if module_name.starts_with("node:")
                        && !core::EXCLUSIVELY_PREFIXED_NODE_CORE_MODULES
                            .contains(&module_name.as_str())
                    {
                        // Presence of `node:` prefix takes precedence over unprefixed node core modules.
                        result.uses_uri_style_node_core_modules = core::Tristate::True;
                    } else if result.uses_uri_style_node_core_modules == core::Tristate::Unknown
                        && core::UNPREFIXED_NODE_CORE_MODULES.contains(&module_name.as_str())
                    {
                        // Avoid `unprefixedNodeCoreModules.has` for every import.
                        result.uses_uri_style_node_core_modules = core::Tristate::False;
                    }
                }
            }
        }
        return;
    }

    if ast::is_ambient_module(store, node)
        && (in_ambient_module
            || ast::has_syntactic_modifier(store, node, ast::ModifierFlags::AMBIENT)
            || is_declaration_file)
    {
        let Some(name) = store.name(node) else {
            return;
        };
        let name_text = store.text(name);
        // Ambient module declarations can be interpreted as augmentations for some existing external modules.
        // This will happen in two cases:
        // - if current file is external module then module augmentation is a ambient module declaration defined in the top level scope
        // - if current file is not external module then module augmentation is a ambient module declaration with non-relative module name
        //   immediately nested in top level ambient module declaration .
        if is_external_module
            || (in_ambient_module && !tspath::is_external_module_name_relative(&name_text))
        {
            result.module_augmentations.push(name);
        } else if !in_ambient_module {
            result.ambient_module_names.push(name_text);
            // An AmbientExternalModuleDeclaration declares an external module.
            // This type of declaration is permitted only in the global module.
            // The StringLiteral must specify a top-level external module name.
            // Relative external module names are not permitted.
            // NOTE: body of ambient module is always a module block, if it exists.
            if let Some(body) = store.body(node) {
                let statements = store
                    .statements(body)
                    .expect("ModuleBlock.statements")
                    .iter()
                    .collect::<Vec<_>>();
                for statement in statements {
                    collect_module_references(
                        store,
                        statement,
                        true,
                        is_declaration_file,
                        is_external_module,
                        result,
                    );
                }
            }
        }
    }
}
