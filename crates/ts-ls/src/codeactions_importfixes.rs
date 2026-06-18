use ts_ast as ast;
use ts_astnav as astnav;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_locale as locale;
use ts_lsproto as lsproto;
use ts_modulespecifiers::CheckerShape;
use ts_scanner as scanner;
use ts_tspath as tspath;

use crate::autoimport;
use crate::codeactions::{
    CodeAction, CodeFixContext, CodeFixProvider, CombinedCodeActions, contains_error_code,
};

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

pub fn import_fix_error_codes() -> Vec<i32> {
    vec![
        diagnostics::Cannot_find_name_0.code(),
        diagnostics::Cannot_find_name_0_Did_you_mean_1.code(),
        diagnostics::Cannot_find_name_0_Did_you_mean_the_instance_member_this_0.code(),
        diagnostics::Cannot_find_name_0_Did_you_mean_the_static_member_1_0.code(),
        diagnostics::Cannot_find_namespace_0.code(),
        diagnostics::X_0_refers_to_a_UMD_global_but_the_current_file_is_a_module_Consider_adding_an_import_instead.code(),
        diagnostics::X_0_only_refers_to_a_type_but_is_being_used_as_a_value_here.code(),
        diagnostics::No_value_exists_in_scope_for_the_shorthand_property_0_Either_declare_one_or_provide_an_initializer.code(),
        diagnostics::X_0_cannot_be_used_as_a_value_because_it_was_imported_using_import_type.code(),
        diagnostics::Cannot_find_name_0_Do_you_need_to_install_type_definitions_for_jQuery_Try_npm_i_save_dev_types_Slashjquery.code(),
        diagnostics::Cannot_find_name_0_Do_you_need_to_change_your_target_library_Try_changing_the_lib_compiler_option_to_1_or_later.code(),
        diagnostics::Cannot_find_name_0_Do_you_need_to_change_your_target_library_Try_changing_the_lib_compiler_option_to_include_dom.code(),
        diagnostics::Cannot_find_name_0_Do_you_need_to_install_type_definitions_for_a_test_runner_Try_npm_i_save_dev_types_Slashjest_or_npm_i_save_dev_types_Slashmocha_and_then_add_jest_or_mocha_to_the_types_field_in_your_tsconfig.code(),
        diagnostics::Cannot_find_name_0_Did_you_mean_to_write_this_in_an_async_function.code(),
        diagnostics::Cannot_find_name_0_Do_you_need_to_install_type_definitions_for_jQuery_Try_npm_i_save_dev_types_Slashjquery_and_then_add_jquery_to_the_types_field_in_your_tsconfig.code(),
        diagnostics::Cannot_find_name_0_Do_you_need_to_install_type_definitions_for_a_test_runner_Try_npm_i_save_dev_types_Slashjest_or_npm_i_save_dev_types_Slashmocha.code(),
        diagnostics::Cannot_find_name_0_Do_you_need_to_install_type_definitions_for_node_Try_npm_i_save_dev_types_Slashnode.code(),
        diagnostics::Cannot_find_name_0_Do_you_need_to_install_type_definitions_for_node_Try_npm_i_save_dev_types_Slashnode_and_then_add_node_to_the_types_field_in_your_tsconfig.code(),
        diagnostics::Cannot_find_namespace_0_Did_you_mean_1.code(),
        diagnostics::Cannot_extend_an_interface_0_Did_you_mean_implements.code(),
        diagnostics::This_JSX_tag_requires_0_to_be_in_scope_but_it_could_not_be_found.code(),
    ]
}

pub const IMPORT_FIX_ID: &str = "fixMissingImport";

pub static IMPORT_FIX_PROVIDER: CodeFixProvider = CodeFixProvider {
    error_codes: import_fix_error_codes,
    get_code_actions: get_import_code_actions,
    fix_ids: &[IMPORT_FIX_ID],
    get_all_code_actions: Some(get_all_import_code_actions),
};

#[derive(Clone, Debug, Default)]
pub struct FixInfo {
    pub fix: Option<autoimport::Fix>,
    pub symbol_name: String,
    pub error_identifier_text: String,
    pub is_jsx_namespace_fix: bool,
}

pub fn get_import_code_actions(
    context: &core::Context,
    fix_context: &CodeFixContext,
) -> Result<Vec<CodeAction>, core::Error> {
    let info = get_fix_infos(
        context,
        fix_context,
        fix_context.error_code,
        fix_context.span.pos(),
    )?;
    if info.is_empty() {
        return Ok(Vec::new());
    }

    let mut actions = Vec::new();
    for fix_info in info {
        let Some(fix) = fix_info.fix.as_ref() else {
            continue;
        };
        let (edits, description) = fix.edits(
            context.clone(),
            fix_context.source_file,
            fix_context.program.options(),
            fix_context.ls.format_options(),
            &fix_context.ls.converters,
            fix_context.ls.user_preferences(),
        );
        actions.push(CodeAction {
            description,
            changes: edits,
            fix_id: IMPORT_FIX_ID.to_string(),
            fix_all_description: diagnostics::Add_all_missing_imports
                .localize(locale::und(), vec![]),
        });
    }
    Ok(actions)
}

pub fn get_all_import_code_actions(
    context: &core::Context,
    fix_context: &CodeFixContext,
) -> Result<Option<CombinedCodeActions>, core::Error> {
    if tspath::is_dynamic_file_name(&fix_context.source_file.file_name()) {
        return Ok(None);
    }

    fix_context.program.with_type_checker_for_file_using(
        compiler::CheckerAccess::context(context),
        fix_context.source_file,
        |checker| {
            let all_diagnostics = fix_context.program.get_semantic_diagnostics_with_checker(
                context.clone(),
                checker,
                fix_context.source_file,
            );

            let mut import_diags = Vec::new();
            for diag in all_diagnostics {
                if contains_error_code(&import_fix_error_codes(), diag.code()) {
                    import_diags.push(diag);
                }
            }
            if import_diags.is_empty() {
                return Ok(None);
            }

            let view = match fix_context
                .ls
                .get_prepared_auto_import_view(fix_context.source_file)?
            {
                Some(view) => view,
                None => fix_context
                    .ls
                    .get_current_auto_import_view(fix_context.source_file),
            };

            let mut import_adder = autoimport::new_import_adder(
                context,
                fix_context.program,
                fix_context.source_file,
                view,
                fix_context.ls.format_options(),
                &fix_context.ls.converters,
                fix_context.ls.user_preferences(),
            );

            for diag in import_diags {
                add_import_from_diagnostic_with_checker(
                    &mut import_adder,
                    &diag,
                    fix_context,
                    checker,
                )?;
            }

            if !import_adder.has_fixes() {
                return Ok(None);
            }

            Ok(Some(CombinedCodeActions {
                description: diagnostics::Add_all_missing_imports.localize(locale::und(), vec![]),
                changes: import_adder.edits(),
            }))
        },
    )
}

pub fn add_import_from_diagnostic_with_checker<'a, 'state>(
    import_adder: &mut autoimport::ImportAdder,
    diag: &ast::Diagnostic,
    fix_context: &CodeFixContext<'a>,
    checker: &mut checker::Checker<'a, 'state>,
) -> Result<(), core::Error> {
    let diag_fix_context = CodeFixContext {
        source_file: fix_context.source_file,
        span: core::new_text_range(diag.pos(), diag.end()),
        error_code: diag.code(),
        program: fix_context.program,
        ls: fix_context.ls,
        diagnostic: None,
        params: None,
    };

    let infos = get_fix_infos_with_checker(&diag_fix_context, checker, diag.code(), diag.pos())?;
    if let Some(info) = infos.first() {
        if let Some(fix) = info.fix.as_ref() {
            import_adder.add_import_fix(fix);
        }
    }
    Ok(())
}

pub fn get_fix_infos(
    context: &core::Context,
    fix_context: &CodeFixContext,
    error_code: i32,
    pos: i32,
) -> Result<Vec<FixInfo>, core::Error> {
    fix_context.program.with_type_checker_for_file_using(
        compiler::CheckerAccess::context(context),
        fix_context.source_file,
        |checker| get_fix_infos_with_checker(fix_context, checker, error_code, pos),
    )
}

pub fn get_fix_infos_with_checker<'a, 'state>(
    fix_context: &CodeFixContext<'a>,
    checker: &mut checker::Checker<'a, 'state>,
    error_code: i32,
    pos: i32,
) -> Result<Vec<FixInfo>, core::Error> {
    if tspath::is_dynamic_file_name(&fix_context.source_file.file_name()) {
        return Ok(Vec::new());
    }

    let symbol_token = astnav::get_token_at_position(fix_context.source_file, pos);

    let mut view = None;
    let info = if error_code
        == diagnostics::X_0_refers_to_a_UMD_global_but_the_current_file_is_a_module_Consider_adding_an_import_instead
            .code()
    {
        let current_view = fix_context.ls.get_current_auto_import_view(fix_context.source_file);
        let result =
            get_fixes_info_for_umd_import(fix_context, checker, symbol_token.as_ref(), &current_view);
        view = Some(current_view);
        result
    } else if !symbol_token
        .as_ref()
        .is_some_and(|token| ast::is_identifier(fix_context.source_file.store(), *token))
    {
        Vec::new()
    } else if error_code
        == diagnostics::X_0_cannot_be_used_as_a_value_because_it_was_imported_using_import_type.code()
    {
        let compiler_options = fix_context.program.options();
        let Some(symbol_token) = symbol_token else {
            return Ok(Vec::new());
        };
        let symbol_names = get_symbol_names_to_import(
            fix_context.program,
            fix_context.source_file,
            checker,
            &symbol_token,
            compiler_options,
        );

        let mut all_type_only_fixes = Vec::new();
        for sn in symbol_names {
            if !sn.is_type_only {
                continue;
            }
            if let Some(fix) = get_type_only_promotion_fix(
                fix_context.source_file,
                &symbol_token,
                &sn.name,
                fix_context.program,
                checker,
            )
            {
                all_type_only_fixes.push(FixInfo {
                    fix: Some(fix),
                    symbol_name: sn.name,
                    error_identifier_text: fix_context.source_file.store().text(symbol_token),
                    is_jsx_namespace_fix: false,
                });
            }
        }

        let diagnostic_message = fix_context
            .diagnostic
            .map(|d| d.message.clone())
            .unwrap_or_default();
        let mut info = Vec::new();
        if all_type_only_fixes.len() > 1 && !diagnostic_message.is_empty() {
            for fi in &all_type_only_fixes {
                if diagnostic_message.contains(&format!("'{}'", fi.symbol_name)) {
                    info.push(fi.clone());
                }
            }
        }
        let info = if info.is_empty() {
            all_type_only_fixes
        } else {
            info
        };
        return Ok(info);
    } else {
        match fix_context.ls.get_prepared_auto_import_view(fix_context.source_file)? {
            Some(prepared_view) => {
                let result = get_fixes_info_for_non_umd_import(
                    fix_context,
                    checker,
                    &symbol_token.unwrap(),
                    &prepared_view,
                );
                view = Some(prepared_view);
                result
            }
            None => Vec::new(),
        }
    };

    let view = view.unwrap_or_else(|| {
        fix_context
            .ls
            .get_current_auto_import_view(fix_context.source_file)
    });
    Ok(sort_fix_info(info, fix_context, &view))
}

pub fn get_fixes_info_for_umd_import<'a, 'state>(
    fix_context: &CodeFixContext<'a>,
    checker: &mut checker::Checker<'a, 'state>,
    token: Option<&ast::Node>,
    view: &autoimport::View,
) -> Vec<FixInfo> {
    let Some(token) = token else {
        return Vec::new();
    };
    let store = fix_context.source_file.store();
    let Some(umd_symbol) = get_umd_symbol(store, token, checker) else {
        return Vec::new();
    };

    let export = autoimport::symbol_identity_to_export(umd_symbol, checker);
    let symbol_name = checker.symbol_name_public(umd_symbol).unwrap_or_default();
    let is_valid_type_only_use_site = ast::is_valid_type_only_alias_use_site(store, token);

    let mut result = Vec::new();
    if let Some(export) = export {
        for fix in view.get_fixes(checker, &export, false, is_valid_type_only_use_site, None) {
            let error_identifier_text = if ast::is_identifier(store, *token) {
                store.text(*token)
            } else {
                String::new()
            };
            result.push(FixInfo {
                fix: Some(fix),
                symbol_name: symbol_name.clone(),
                error_identifier_text,
                is_jsx_namespace_fix: false,
            });
        }
    }
    result
}

pub(crate) fn get_umd_symbol<'a, 'state>(
    store: &ast::AstStore,
    token: &ast::Node,
    checker: &mut checker::Checker<'a, 'state>,
) -> Option<ast::SymbolIdentity> {
    let umd_symbol = if ast::is_identifier(store, *token) {
        checker.get_resolved_symbol_public(*token)
    } else {
        None
    };
    if is_umd_export_symbol(umd_symbol.clone(), checker) {
        return umd_symbol;
    }

    let parent = store.parent(*token)?;
    if (ast::is_jsx_opening_like_element(store, parent) && store.tag_name(parent) == Some(*token))
        || ast::is_jsx_opening_fragment(store, parent)
    {
        let location = if ast::is_jsx_opening_like_element(store, parent) {
            token
        } else {
            &parent
        };
        let jsx_namespace = checker.get_jsx_namespace_public(parent);
        let parent_symbol =
            checker.resolve_name_public(&jsx_namespace, *location, ast::SYMBOL_FLAGS_VALUE, false);
        if is_umd_export_symbol(parent_symbol.clone(), checker) {
            return parent_symbol;
        }
    }
    None
}

pub(crate) fn is_umd_export_symbol(
    symbol: Option<ast::SymbolIdentity>,
    checker: &mut checker::Checker<'_, '_>,
) -> bool {
    symbol.is_some_and(|symbol| {
        let declarations = checker.collect_symbol_declarations_public(symbol);
        !declarations.is_empty()
            && declarations.first().is_some_and(|declaration| {
                checker
                    .source_file_store(*declaration)
                    .is_some_and(|store| ast::is_namespace_export_declaration(store, *declaration))
            })
    })
}

pub fn get_fixes_info_for_non_umd_import<'a, 'state>(
    fix_context: &CodeFixContext<'a>,
    checker: &mut checker::Checker<'a, 'state>,
    symbol_token: &ast::Node,
    view: &autoimport::View,
) -> Vec<FixInfo> {
    let compiler_options = fix_context.program.options();
    let store = fix_context.source_file.store();
    let is_valid_type_only_use_site = ast::is_valid_type_only_alias_use_site(store, symbol_token);
    let symbol_names = get_symbol_names_to_import(
        fix_context.program,
        fix_context.source_file,
        checker,
        symbol_token,
        compiler_options,
    );
    let usage_position = fix_context.ls.converters.position_to_line_and_character(
        fix_context.source_file,
        scanner::get_token_pos_of_node(symbol_token, fix_context.source_file, false) as i32,
    );

    let mut all_info = Vec::new();
    for sn in symbol_names {
        if sn.is_type_only {
            continue;
        }
        let symbol_name = sn.name;
        if symbol_name == "default" {
            continue;
        }

        let symbol_token_text = store.text(*symbol_token);
        let is_jsx_tag_name =
            symbol_name == symbol_token_text && ast::is_jsx_tag_name(store, symbol_token);
        let query_kind = if is_jsx_tag_name {
            autoimport::QueryKind::CaseInsensitiveMatch
        } else {
            autoimport::QueryKind::ExactMatch
        };

        let exports = view.search(&symbol_name, query_kind);
        for export in exports {
            if is_jsx_tag_name && !(export.name() == symbol_name || export.is_renameable()) {
                continue;
            }
            for fix in view.get_fixes(
                checker,
                &export,
                is_jsx_tag_name,
                is_valid_type_only_use_site,
                Some(&usage_position),
            ) {
                all_info.push(FixInfo {
                    fix: Some(fix),
                    symbol_name: symbol_name.clone(),
                    error_identifier_text: String::new(),
                    is_jsx_namespace_fix: symbol_name != symbol_token_text,
                });
            }
        }
    }
    all_info
}

pub fn get_type_only_promotion_fix<'a, 'state>(
    source_file: &'a ast::SourceFile,
    symbol_token: &ast::Node,
    symbol_name: &str,
    program: &'a compiler::Program,
    checker: &mut checker::Checker<'a, 'state>,
) -> Option<autoimport::Fix> {
    let Some(symbol) =
        checker.resolve_name_public(symbol_name, *symbol_token, ast::SYMBOL_FLAGS_VALUE, true)
    else {
        return None;
    };
    let Some(type_only_alias_declaration) =
        checker.get_type_only_alias_declaration_for_symbol_public(symbol)
    else {
        return None;
    };
    let is_same_source_file =
        source_file_of_node(program, &type_only_alias_declaration) == source_file;

    if !is_same_source_file {
        return None;
    }

    Some(autoimport::Fix {
        auto_import_fix: Some(lsproto::AutoImportFix {
            kind: lsproto::AutoImportFixKind::PromoteTypeOnly,
            ..Default::default()
        }),
        type_only_alias_declaration: Some(type_only_alias_declaration.clone()),
        ..Default::default()
    })
}

#[derive(Clone, Debug, Default)]
pub struct SymbolNameInfo {
    pub name: String,
    pub is_type_only: bool,
}

pub fn get_symbol_names_to_import<'a, 'state>(
    program: &'a compiler::Program,
    source_file: &'a ast::SourceFile,
    checker: &mut checker::Checker<'a, 'state>,
    symbol_token: &ast::Node,
    compiler_options: &core::CompilerOptions,
) -> Vec<SymbolNameInfo> {
    let store = source_file.store();
    let symbol_token_text = store.text(*symbol_token);
    let parent = store.parent(*symbol_token);
    if parent.as_ref().is_some_and(|parent| {
        (ast::is_jsx_opening_like_element(store, parent)
            || ast::is_jsx_closing_element(store, *parent))
            && store.tag_name(*parent) == Some(*symbol_token)
    }) && jsx_mode_needs_explicit_import(compiler_options.jsx)
    {
        let source_file_node = source_file.as_node();
        let jsx_namespace = checker.get_jsx_namespace_public(source_file_node);
        if needs_jsx_namespace_fix(program, &jsx_namespace, symbol_token, checker) {
            let mut result = Vec::new();
            if !scanner::is_intrinsic_jsx_name(&symbol_token_text) {
                let comp_symbol = checker.resolve_name_public(
                    &symbol_token_text,
                    *symbol_token,
                    ast::SYMBOL_FLAGS_VALUE,
                    false,
                );
                if comp_symbol.is_none() {
                    result.push(SymbolNameInfo {
                        name: symbol_token_text.clone(),
                        is_type_only: false,
                    });
                } else if {
                    let comp_symbol = comp_symbol.unwrap();
                    checker
                        .get_type_only_alias_declaration_for_symbol_public(comp_symbol)
                        .is_some()
                } {
                    result.push(SymbolNameInfo {
                        name: symbol_token_text.clone(),
                        is_type_only: true,
                    });
                }
            }

            let ns_is_type_only = checker
                .resolve_name_public(&jsx_namespace, *symbol_token, ast::SYMBOL_FLAGS_VALUE, true)
                .is_some_and(|ns_symbol| {
                    checker
                        .get_type_only_alias_declaration_for_symbol_public(ns_symbol)
                        .is_some()
                });
            result.push(SymbolNameInfo {
                name: jsx_namespace.to_string(),
                is_type_only: ns_is_type_only,
            });
            return result;
        }
    }

    let token_is_type_only = checker
        .resolve_name_public(
            &symbol_token_text,
            *symbol_token,
            ast::SYMBOL_FLAGS_VALUE,
            true,
        )
        .is_some_and(|sym| {
            checker
                .get_type_only_alias_declaration_for_symbol_public(sym)
                .is_some()
        });
    vec![SymbolNameInfo {
        name: symbol_token_text,
        is_type_only: token_is_type_only,
    }]
}

pub fn needs_jsx_namespace_fix<'a, 'state>(
    program: &'a compiler::Program,
    jsx_namespace: &str,
    symbol_token: &ast::Node,
    checker: &mut checker::Checker<'a, 'state>,
) -> bool {
    let store = store_for_node(program, symbol_token);
    if scanner::is_intrinsic_jsx_name(&store.text(*symbol_token)) {
        return true;
    }
    let namespace_symbol =
        checker.resolve_name_public(jsx_namespace, *symbol_token, ast::SYMBOL_FLAGS_VALUE, true);
    if namespace_symbol.is_none() {
        return true;
    }
    let namespace_symbol = namespace_symbol.unwrap();
    let declarations = checker.collect_symbol_declarations_public(namespace_symbol);
    if declarations.iter().any(|declaration| {
        let store = store_for_node(program, declaration);
        ast::is_type_only_import_or_export_declaration(store, *declaration)
    }) {
        let flags = checker
            .symbol_flags_public(namespace_symbol)
            .unwrap_or(ast::SYMBOL_FLAGS_NONE);
        return (flags & ast::SYMBOL_FLAGS_VALUE) == 0;
    }
    false
}

pub fn jsx_mode_needs_explicit_import(jsx: core::JsxEmit) -> bool {
    jsx == core::JsxEmit::React || jsx == core::JsxEmit::ReactNative
}

pub fn sort_fix_info(
    fixes: Vec<FixInfo>,
    _fix_context: &CodeFixContext,
    view: &autoimport::View,
) -> Vec<FixInfo> {
    if fixes.is_empty() {
        return fixes;
    }
    let mut sorted = fixes;
    sorted.sort_by(|a, b| {
        let cmp = core::compare_booleans(a.is_jsx_namespace_fix, b.is_jsx_namespace_fix);
        if cmp != 0 {
            return cmp.cmp(&0);
        }
        match (a.fix.as_ref(), b.fix.as_ref()) {
            (Some(a_fix), Some(b_fix)) => view.compare_fixes_for_sorting(a_fix, b_fix).cmp(&0),
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
        }
    });
    sorted
}
