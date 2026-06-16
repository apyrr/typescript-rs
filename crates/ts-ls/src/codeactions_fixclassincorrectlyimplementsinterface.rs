use std::collections::HashSet;

use ts_ast as ast;
use ts_astnav as astnav;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_locale as locale;
use ts_lsproto as lsproto;
use ts_scanner as scanner;

use crate::autoimport;
use crate::change;
use crate::codeactions::{
    CodeAction, CodeFixContext, CodeFixProvider, CombinedCodeActions, contains_error_code,
};
use crate::codeactions_missingmemberfixer::{
    PRESERVE_OPTIONAL_FLAGS_ALL, new_missing_member_fixer,
};
use crate::diagnostics::get_all_diagnostics_with_checker;

pub const FIX_CLASS_INCORRECTLY_IMPLEMENTS_INTERFACE_FIX_ID: &str =
    "fixClassIncorrectlyImplementsInterface";

pub fn fix_class_incorrectly_implements_interface_error_codes() -> Vec<i32> {
    vec![
        diagnostics::CLASS_0_INCORRECTLY_IMPLEMENTS_INTERFACE_1.code(),
        diagnostics::CLASS_0_INCORRECTLY_IMPLEMENTS_CLASS_1_DID_YOU_MEAN_TO_EXTEND_1_AND_INHERIT_ITS_MEMBERS_AS_A_SUBCLASS.code(),
    ]
}

pub static FIX_CLASS_INCORRECTLY_IMPLEMENTS_INTERFACE_PROVIDER: CodeFixProvider = CodeFixProvider {
    error_codes: fix_class_incorrectly_implements_interface_error_codes,
    get_code_actions: get_code_actions_to_fix_class_incorrectly_implements_interface,
    fix_ids: &[FIX_CLASS_INCORRECTLY_IMPLEMENTS_INTERFACE_FIX_ID],
    get_all_code_actions: Some(get_all_code_actions_to_fix_class_incorrectly_implements_interface),
};

pub fn get_code_actions_to_fix_class_incorrectly_implements_interface(
    context: &core::Context,
    fix_context: &CodeFixContext,
) -> Result<Vec<CodeAction>, core::Error> {
    let Some(class_declaration) = get_class(fix_context.source_file, fix_context.span) else {
        return Ok(Vec::new());
    };

    let store = fix_context.source_file.store();
    let implements_types = ast::get_implements_type_nodes(store, &class_declaration);
    let loc = locale::und();

    fix_context.program.with_type_checker_for_file_using(
        compiler::CheckerAccess::context(context),
        fix_context.source_file,
        |type_checker| {
            let mut actions = Vec::new();
            for implemented_type_node in implements_types {
                let mut change_tracker = change::new_tracker(
                    context.clone(),
                    fix_context.program.options(),
                    fix_context.ls.format_options(),
                    &fix_context.ls.converters,
                );
                let mut import_adder = create_import_adder(context, fix_context)?;
                add_changes(
                    context,
                    fix_context,
                    &mut change_tracker,
                    import_adder.as_mut(),
                    type_checker,
                    &class_declaration,
                    &implemented_type_node,
                )?;
                let changes = get_changes(
                    &mut change_tracker,
                    import_adder.as_ref(),
                    fix_context.source_file,
                );
                if changes.is_empty() {
                    continue;
                }

                actions.push(CodeAction {
                    description: diagnostics::IMPLEMENT_INTERFACE_0.localize(
                        loc.clone(),
                        vec![Box::new(scanner::get_text_of_node(
                            fix_context.source_file,
                            &implemented_type_node,
                        ))],
                    ),
                    changes,
                    fix_id: FIX_CLASS_INCORRECTLY_IMPLEMENTS_INTERFACE_FIX_ID.to_string(),
                    fix_all_description: diagnostics::IMPLEMENT_ALL_UNIMPLEMENTED_INTERFACES
                        .localize(loc.clone(), vec![]),
                });
            }
            Ok(actions)
        },
    )
}

pub fn get_all_code_actions_to_fix_class_incorrectly_implements_interface(
    context: &core::Context,
    fix_context: &CodeFixContext,
) -> Result<Option<CombinedCodeActions>, core::Error> {
    fix_context.program.with_type_checker_for_file_using(
        compiler::CheckerAccess::context(context),
        fix_context.source_file,
        |type_checker| {
            let mut change_tracker = change::new_tracker(
                context.clone(),
                fix_context.program.options(),
                fix_context.ls.format_options(),
                &fix_context.ls.converters,
            );
            let mut import_adder = create_import_adder(context, fix_context)?;

            let mut seen_class_declarations: HashSet<ast::Node> = HashSet::new();

            for diag in get_all_diagnostics_with_checker(
                context,
                fix_context.program,
                fix_context.source_file,
                type_checker,
            ) {
                if contains_error_code(
                    &fix_class_incorrectly_implements_interface_error_codes(),
                    diag.code(),
                ) {
                    let class_declaration = get_class(
                        fix_context.source_file,
                        core::new_text_range(diag.pos(), diag.end()),
                    );
                    let Some(class_declaration) = class_declaration else {
                        continue;
                    };
                    if seen_class_declarations.insert(class_declaration) {
                        let implements_types = ast::get_implements_type_nodes(
                            fix_context.source_file.store(),
                            &class_declaration,
                        );
                        for implemented_type_node in implements_types {
                            add_changes(
                                context,
                                fix_context,
                                &mut change_tracker,
                                import_adder.as_mut(),
                                type_checker,
                                &class_declaration,
                                &implemented_type_node,
                            )?;
                        }
                    }
                }
            }

            let changes = get_changes(
                &mut change_tracker,
                import_adder.as_ref(),
                fix_context.source_file,
            );
            if changes.is_empty() {
                return Ok(None);
            }

            Ok(Some(CombinedCodeActions {
                description: diagnostics::IMPLEMENT_ALL_UNIMPLEMENTED_INTERFACES
                    .localize(locale::und(), vec![]),
                changes,
            }))
        },
    )
}

pub fn add_changes<'a, 'checker>(
    _context: &core::Context,
    fix_context: &'a CodeFixContext<'a>,
    change_tracker: &mut change::Tracker<'a>,
    mut import_adder: Option<&mut autoimport::ImportAdder<'a>>,
    type_checker: &'checker mut checker::Checker<'a, '_>,
    class_declaration: &ast::Node,
    implemented_type_node: &ast::Node,
) -> Result<(), core::Error> {
    let store = fix_context.source_file.store();
    let constructor = get_constructor(store, class_declaration);
    let implemented_type = type_checker.get_type_at_location(*implemented_type_node);
    let class_type = type_checker.get_type_at_location(*class_declaration);

    if type_checker
        .get_number_index_type_public(class_type)
        .is_none()
    {
        let number_type = type_checker.get_number_type();
        let member = {
            let mut missing_member_fixer = new_missing_member_fixer(
                change_tracker,
                fix_context.program,
                type_checker,
                fix_context.ls.user_preferences(),
                import_adder.as_deref_mut(),
                locale::und(),
            );
            missing_member_fixer.create_index_signature_declaration_from_type(
                class_declaration,
                implemented_type,
                number_type,
            )
        };
        if let Some(member) = member {
            insert_interface_member_node(
                change_tracker,
                fix_context.source_file,
                class_declaration,
                constructor,
                &member,
            );
        }
    }

    if type_checker
        .get_string_index_type_public(class_type)
        .is_none()
    {
        let string_type = type_checker.get_string_type();
        let member = {
            let mut missing_member_fixer = new_missing_member_fixer(
                change_tracker,
                fix_context.program,
                type_checker,
                fix_context.ls.user_preferences(),
                import_adder.as_deref_mut(),
                locale::und(),
            );
            missing_member_fixer.create_index_signature_declaration_from_type(
                class_declaration,
                implemented_type,
                string_type,
            )
        };
        if let Some(member) = member {
            insert_interface_member_node(
                change_tracker,
                fix_context.source_file,
                class_declaration,
                constructor,
                &member,
            );
        }
    }

    let implemented_types = [implemented_type];
    let missing_members =
        get_missing_members(store, type_checker, class_declaration, &implemented_types);
    for member in missing_members {
        let member_nodes = {
            let mut missing_member_fixer = new_missing_member_fixer(
                change_tracker,
                fix_context.program,
                type_checker,
                fix_context.ls.user_preferences(),
                import_adder.as_deref_mut(),
                locale::und(),
            );
            missing_member_fixer.create_member_from_symbol(
                member.clone(),
                class_declaration,
                fix_context.source_file,
                None, /*body*/
                PRESERVE_OPTIONAL_FLAGS_ALL,
            )?
        };
        for member_node in member_nodes {
            insert_interface_member_node(
                change_tracker,
                fix_context.source_file,
                class_declaration,
                constructor,
                &member_node,
            );
        }
    }
    Ok(())
}

pub fn get_changes(
    change_tracker: &mut change::Tracker,
    import_adder: Option<&autoimport::ImportAdder>,
    source_file: &ast::SourceFile,
) -> Vec<lsproto::TextEdit> {
    let mut file_changes = change_tracker
        .get_changes()
        .remove(&source_file.file_name())
        .unwrap_or_default();
    if import_adder.is_some_and(|import_adder| import_adder.has_fixes()) {
        file_changes.extend(import_adder.unwrap().edits());
    }
    file_changes
}

pub fn insert_interface_member_node<'a>(
    change_tracker: &mut change::Tracker<'a>,
    source_file: &'a ast::SourceFile,
    class_declaration: &ast::Node,
    constructor: Option<ast::Node>,
    member: &ast::Node,
) {
    if constructor.is_none() {
        change_tracker.insert_member_at_start(source_file, *class_declaration, *member);
    } else {
        change_tracker.insert_node_after(source_file, constructor.unwrap(), *member);
    }
}

pub fn get_class(source_file: &ast::SourceFile, span: core::TextRange) -> Option<ast::Node> {
    let token = astnav::get_token_at_position(source_file, span.pos());
    token
        .as_ref()
        .and_then(|token| ast::get_containing_class(source_file.store(), *token))
}

pub fn get_constructor<'a>(
    store: &ast::AstStore,
    class_declaration: &'a ast::Node,
) -> Option<ast::Node> {
    if let Some(members) = store.members(*class_declaration) {
        for member in members {
            if ast::is_constructor_declaration(store, member) {
                return Some(member);
            }
        }
    }
    None
}

fn get_declaration_modifier_flags_from_symbol_identity(
    type_checker: &mut checker::Checker<'_, '_>,
    symbol: ast::SymbolIdentity,
) -> ast::ModifierFlags {
    let flags = type_checker
        .symbol_flags_public(symbol)
        .unwrap_or(ast::SYMBOL_FLAGS_NONE);
    let check_flags = type_checker
        .symbol_check_flags_public(symbol)
        .unwrap_or(ast::CHECK_FLAGS_NONE);
    let Some(value_declaration) = type_checker.symbol_value_declaration_public(symbol) else {
        if check_flags & ast::CHECK_FLAGS_SYNTHETIC != 0 {
            let access_modifier = if check_flags & ast::CHECK_FLAGS_CONTAINS_PRIVATE != 0 {
                ast::ModifierFlags::Private
            } else if check_flags & ast::CHECK_FLAGS_CONTAINS_PUBLIC != 0 {
                ast::ModifierFlags::Public
            } else {
                ast::ModifierFlags::Protected
            };
            let static_modifier = if check_flags & ast::CHECK_FLAGS_CONTAINS_STATIC != 0 {
                ast::ModifierFlags::Static
            } else {
                ast::ModifierFlags::None
            };
            return access_modifier | static_modifier;
        }
        if flags & ast::SYMBOL_FLAGS_PROTOTYPE != 0 {
            return ast::ModifierFlags::Public | ast::ModifierFlags::Static;
        }
        return ast::ModifierFlags::None;
    };

    let declarations = type_checker.collect_symbol_declarations_public(symbol);
    let declaration = if flags & ast::SYMBOL_FLAGS_GET_ACCESSOR != 0 {
        declarations
            .iter()
            .copied()
            .find(|declaration| {
                type_checker
                    .try_source_file_for_node_public(*declaration)
                    .is_some_and(|source_file| {
                        ast::is_get_accessor_declaration(source_file.store(), *declaration)
                    })
            })
            .unwrap_or(value_declaration)
    } else {
        value_declaration
    };
    let declaration_store = type_checker
        .try_source_file_for_node_public(declaration)
        .map(|source_file| source_file.store())
        .expect("symbol declaration should belong to a checker source file");
    let modifier_flags = ast::get_combined_modifier_flags(declaration_store, declaration);
    let parent_is_class = type_checker
        .symbol_parent_public(symbol)
        .and_then(|parent| type_checker.symbol_flags_public(parent))
        .is_some_and(|parent_flags| parent_flags & ast::SYMBOL_FLAGS_CLASS != 0);
    if parent_is_class {
        modifier_flags
    } else {
        modifier_flags & !ast::ModifierFlags::AccessibilityModifier
    }
}

fn get_missing_members<'a>(
    store: &ast::AstStore,
    type_checker: &mut checker::Checker<'a, '_>,
    class_declaration: &ast::Node,
    implemented_types: &[checker::TypeHandle],
) -> Vec<ast::SymbolIdentity> {
    let inherited_members = get_inherited_members(store, type_checker, class_declaration);
    let mut seen_members: HashSet<String> = HashSet::new();

    let class_symbol = type_checker.source_node_symbol_public(*class_declaration);

    let mut missing_members = Vec::new();
    for implemented_type in implemented_types {
        for symbol in type_checker.get_properties_of_type_public(*implemented_type) {
            let Some(symbol_name) = type_checker.symbol_name_public(symbol) else {
                continue;
            };
            if class_symbol.as_ref().is_some_and(|class_symbol| {
                let class_symbol = *class_symbol;
                type_checker.symbol_has_member_public(class_symbol, &symbol_name)
            }) {
                continue;
            }
            if inherited_members.contains(&symbol_name) || seen_members.contains(&symbol_name) {
                continue;
            }
            let flags = get_declaration_modifier_flags_from_symbol_identity(type_checker, symbol);
            if flags & ast::ModifierFlags::Private == ast::ModifierFlags::None {
                seen_members.insert(symbol_name);
                missing_members.push(symbol);
            }
        }
    }
    missing_members
}

pub fn get_inherited_members(
    store: &ast::AstStore,
    type_checker: &mut checker::Checker,
    class_declaration: &ast::Node,
) -> HashSet<String> {
    let Some(type_node) = ast::get_class_extends_heritage_element(store, class_declaration) else {
        return HashSet::default();
    };

    let base_type = type_checker.get_type_at_location(type_node);

    let mut inherited_members = HashSet::default();
    for symbol in type_checker.get_properties_of_type_public(base_type) {
        let Some(symbol_name) = type_checker.symbol_name_public(symbol) else {
            continue;
        };
        let flags = get_declaration_modifier_flags_from_symbol_identity(type_checker, symbol);
        if flags & ast::ModifierFlags::Private == ast::ModifierFlags::None {
            inherited_members.insert(symbol_name);
        }
    }
    inherited_members
}

pub fn create_import_adder<'a>(
    context: &core::Context,
    fix_context: &'a CodeFixContext<'a>,
) -> Result<Option<autoimport::ImportAdder<'a>>, core::Error> {
    let view = fix_context
        .ls
        .get_prepared_auto_import_view(fix_context.source_file)?;
    let Some(view) = view else {
        return Ok(None);
    };
    Ok(Some(autoimport::new_import_adder(
        context,
        fix_context.program,
        fix_context.source_file,
        view,
        fix_context.ls.format_options(),
        &fix_context.ls.converters,
        fix_context.ls.user_preferences(),
    )))
}
