use ts_ast as ast;

pub fn modifier_is_allowed(kind: ast::Kind, allowed: ast::ModifierFlags) -> bool {
    let flags = ast::modifier_to_flag(kind);
    flags == ast::ModifierFlags::NONE || (flags & allowed) != ast::ModifierFlags::NONE
}

pub fn extract_modifiers(
    factory: &mut ast::NodeFactory,
    modifiers: Option<ast::SourceModifierList<'_>>,
    allowed: ast::ModifierFlags,
) -> Option<ast::ModifierList> {
    let modifiers = modifiers?;
    let source = modifiers.store();
    let nodes = modifiers.nodes();
    let loc = modifiers.loc();
    let range = modifiers.range();
    let modifier_flags = modifiers.modifier_flags();
    let mut import_state = ast::AstImportState::new();
    let filtered: Vec<_> = nodes
        .iter()
        .filter(|modifier| modifier_is_allowed(source.kind(*modifier), allowed))
        .map(|modifier| import_state.preserve_node(source, factory, modifier))
        .collect();

    if filtered.len() == nodes.len() {
        return Some(factory.new_modifier_list(loc, range, filtered, modifier_flags));
    }

    Some(factory.new_modifier_list(loc, range, filtered, modifier_flags))
}
