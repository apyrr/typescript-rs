use ts_ast::{self as ast, ModifierFlags};

pub fn needs_scope_marker(
    is_import_or_reexport: bool,
    is_export_assignment: bool,
    has_export_modifier: bool,
    is_ambient_module: bool,
) -> bool {
    !is_import_or_reexport && !is_export_assignment && !has_export_modifier && !is_ambient_module
}

pub fn can_have_literal_initializer(kind: ast::Kind, has_private_modifier: bool) -> bool {
    match kind {
        ast::Kind::PropertyDeclaration | ast::Kind::PropertySignature => !has_private_modifier,
        ast::Kind::Parameter | ast::Kind::VariableDeclaration => true,
        _ => false,
    }
}

pub fn can_produce_diagnostics(kind: ast::Kind) -> bool {
    matches!(
        kind,
        ast::Kind::VariableDeclaration
            | ast::Kind::PropertyDeclaration
            | ast::Kind::PropertySignature
            | ast::Kind::BindingElement
            | ast::Kind::SetAccessor
            | ast::Kind::GetAccessor
            | ast::Kind::ConstructSignature
            | ast::Kind::CallSignature
            | ast::Kind::MethodDeclaration
            | ast::Kind::MethodSignature
            | ast::Kind::FunctionDeclaration
            | ast::Kind::Parameter
            | ast::Kind::TypeParameter
            | ast::Kind::ExpressionWithTypeArguments
            | ast::Kind::ImportEqualsDeclaration
            | ast::Kind::TypeAliasDeclaration
            | ast::Kind::JSTypeAliasDeclaration
            | ast::Kind::Constructor
            | ast::Kind::IndexSignature
            | ast::Kind::PropertyAccessExpression
            | ast::Kind::ElementAccessExpression
            | ast::Kind::BinaryExpression
    )
}

pub fn is_preserved_declaration_statement(kind: ast::Kind) -> bool {
    matches!(
        kind,
        ast::Kind::FunctionDeclaration
            | ast::Kind::ModuleDeclaration
            | ast::Kind::ImportEqualsDeclaration
            | ast::Kind::InterfaceDeclaration
            | ast::Kind::ClassDeclaration
            | ast::Kind::TypeAliasDeclaration
            | ast::Kind::JSTypeAliasDeclaration
            | ast::Kind::EnumDeclaration
            | ast::Kind::VariableStatement
            | ast::Kind::ImportDeclaration
            | ast::Kind::JSImportDeclaration
            | ast::Kind::ExportDeclaration
            | ast::Kind::ExportAssignment
    )
}

pub fn declaration_visibility_action(
    kind: ast::Kind,
    declaration_is_visible: bool,
    binding_name_visible: bool,
) -> bool {
    match kind {
        ast::Kind::FunctionDeclaration
        | ast::Kind::ModuleDeclaration
        | ast::Kind::InterfaceDeclaration
        | ast::Kind::ClassDeclaration
        | ast::Kind::TypeAliasDeclaration
        | ast::Kind::JSTypeAliasDeclaration
        | ast::Kind::EnumDeclaration => !declaration_is_visible,
        ast::Kind::VariableDeclaration => !binding_name_visible,
        ast::Kind::ImportEqualsDeclaration
        | ast::Kind::ImportDeclaration
        | ast::Kind::JSImportDeclaration
        | ast::Kind::ExportDeclaration
        | ast::Kind::ExportAssignment => false,
        ast::Kind::ClassStaticBlockDeclaration => true,
        _ => false,
    }
}

pub fn is_enclosing_declaration(kind: ast::Kind) -> bool {
    matches!(
        kind,
        ast::Kind::SourceFile
            | ast::Kind::TypeAliasDeclaration
            | ast::Kind::JSTypeAliasDeclaration
            | ast::Kind::ModuleDeclaration
            | ast::Kind::ClassDeclaration
            | ast::Kind::InterfaceDeclaration
            | ast::Kind::FunctionDeclaration
            | ast::Kind::FunctionExpression
            | ast::Kind::ArrowFunction
            | ast::Kind::MethodDeclaration
            | ast::Kind::Constructor
            | ast::Kind::GetAccessor
            | ast::Kind::SetAccessor
            | ast::Kind::IndexSignature
            | ast::Kind::MappedType
    )
}

pub fn is_always_type(kind: ast::Kind) -> bool {
    kind == ast::Kind::InterfaceDeclaration
}

pub fn mask_modifier_flags(mut flags: ModifierFlags) -> ModifierFlags {
    if (flags & ModifierFlags::DEFAULT) != ModifierFlags::NONE
        && (flags & ModifierFlags::EXPORT) == ModifierFlags::NONE
    {
        flags = ModifierFlags(flags.0 ^ ModifierFlags::EXPORT.0);
    }
    if (flags & ModifierFlags::DEFAULT) != ModifierFlags::NONE
        && (flags & ModifierFlags::AMBIENT) != ModifierFlags::NONE
    {
        flags = ModifierFlags(flags.0 ^ ModifierFlags::AMBIENT.0);
    }
    flags
}

pub fn is_private_method_type_parameter(
    parent_kind: ast::Kind,
    parent_has_private_modifier: bool,
) -> bool {
    parent_kind == ast::Kind::MethodDeclaration && parent_has_private_modifier
}

pub fn should_emit_function_properties(
    function_has_body: bool,
    any_overload_has_body: bool,
) -> bool {
    function_has_body || any_overload_has_body
}

pub fn is_scope_marker(kind: ast::Kind) -> bool {
    matches!(
        kind,
        ast::Kind::ExportAssignment | ast::Kind::ExportDeclaration
    )
}

pub fn has_scope_marker(statement_kinds: &[ast::Kind]) -> bool {
    statement_kinds.iter().copied().any(is_scope_marker)
}

pub fn is_declaration_emit_visible(_name: &str) -> bool {
    true
}
