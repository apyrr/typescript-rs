use ts_ast as ast;
use ts_checker as checker;
use ts_collections as collections;

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
#[repr(i32)]
pub enum ScriptElementKind {
    #[default]
    Unknown = 0,
    Warning,
    // predefined type (void) or keyword (class)
    Keyword,
    // top level script node
    ScriptElement,
    // module foo {}
    ModuleElement,
    // class X {}
    ClassElement,
    // var x = class X {}
    LocalClassElement,
    // interface Y {}
    InterfaceElement,
    // type T = ...
    TypeElement,
    // enum E {}
    EnumElement,
    EnumMemberElement,
    // Inside module and script only.
    // const v = ...
    VariableElement,
    // Inside function.
    LocalVariableElement,
    // using foo = ...
    VariableUsingElement,
    // await using foo = ...
    VariableAwaitUsingElement,
    // Inside module and script only.
    // function f() {}
    FunctionElement,
    // Inside function.
    LocalFunctionElement,
    // class X { [public|private]* foo() {} }
    MemberFunctionElement,
    // class X { [public|private]* [get|set] foo:number; }
    MemberGetAccessorElement,
    MemberSetAccessorElement,
    // class X { [public|private]* foo:number; }
    // interface Y { foo:number; }
    MemberVariableElement,
    // class X { [public|private]* accessor foo: number; }
    MemberAccessorVariableElement,
    // class X { constructor() { } }
    // class X { static { } }
    ConstructorImplementationElement,
    // interface Y { ():number; }
    CallSignatureElement,
    // interface Y { []:number; }
    IndexSignatureElement,
    // interface Y { new():Y; }
    ConstructSignatureElement,
    // function foo(*Y*: string)
    ParameterElement,
    TypeParameterElement,
    PrimitiveType,
    Label,
    Alias,
    ConstElement,
    LetElement,
    Directory,
    ExternalModuleName,
    // String literal
    String,
    // Link display punctuation/text around a linked entity.
    Link,
    // Link display entity name.
    LinkName,
    // Link display text.
    LinkText,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScriptElementKindModifier(pub u32);

impl ScriptElementKindModifier {
    pub const NONE: Self = Self(0);
    pub const PUBLIC: Self = Self(1 << 0);
    pub const PRIVATE: Self = Self(1 << 1);
    pub const PROTECTED: Self = Self(1 << 2);
    pub const EXPORTED: Self = Self(1 << 3);
    pub const AMBIENT: Self = Self(1 << 4);
    pub const STATIC: Self = Self(1 << 5);
    pub const ABSTRACT: Self = Self(1 << 6);
    pub const OPTIONAL: Self = Self(1 << 7);
    pub const DEPRECATED: Self = Self(1 << 8);
    pub const DTS: Self = Self(1 << 9);
    pub const TS: Self = Self(1 << 10);
    pub const TSX: Self = Self(1 << 11);
    pub const JS: Self = Self(1 << 12);
    pub const JSX: Self = Self(1 << 13);
    pub const JSON: Self = Self(1 << 14);
    pub const DMTS: Self = Self(1 << 15);
    pub const MTS: Self = Self(1 << 16);
    pub const MJS: Self = Self(1 << 17);
    pub const DCTS: Self = Self(1 << 18);
    pub const CTS: Self = Self(1 << 19);
    pub const CJS: Self = Self(1 << 20);

    pub fn strings(self) -> collections::Set<String> {
        let mut result = collections::Set::default();
        for entry in SCRIPT_ELEMENT_KIND_MODIFIER_NAMES {
            if self & entry.flag != Self::NONE {
                result.add(entry.name.to_string());
            }
        }
        result
    }
}

pub const SCRIPT_ELEMENT_KIND_MODIFIER_NONE: ScriptElementKindModifier =
    ScriptElementKindModifier::NONE;
pub const SCRIPT_ELEMENT_KIND_MODIFIER_DTS: ScriptElementKindModifier =
    ScriptElementKindModifier::DTS;
pub const SCRIPT_ELEMENT_KIND_MODIFIER_TS: ScriptElementKindModifier =
    ScriptElementKindModifier::TS;
pub const SCRIPT_ELEMENT_KIND_MODIFIER_TSX: ScriptElementKindModifier =
    ScriptElementKindModifier::TSX;
pub const SCRIPT_ELEMENT_KIND_MODIFIER_JS: ScriptElementKindModifier =
    ScriptElementKindModifier::JS;
pub const SCRIPT_ELEMENT_KIND_MODIFIER_JSX: ScriptElementKindModifier =
    ScriptElementKindModifier::JSX;
pub const SCRIPT_ELEMENT_KIND_MODIFIER_JSON: ScriptElementKindModifier =
    ScriptElementKindModifier::JSON;
pub const SCRIPT_ELEMENT_KIND_MODIFIER_DMTS: ScriptElementKindModifier =
    ScriptElementKindModifier::DMTS;
pub const SCRIPT_ELEMENT_KIND_MODIFIER_MTS: ScriptElementKindModifier =
    ScriptElementKindModifier::MTS;
pub const SCRIPT_ELEMENT_KIND_MODIFIER_MJS: ScriptElementKindModifier =
    ScriptElementKindModifier::MJS;
pub const SCRIPT_ELEMENT_KIND_MODIFIER_DCTS: ScriptElementKindModifier =
    ScriptElementKindModifier::DCTS;
pub const SCRIPT_ELEMENT_KIND_MODIFIER_CTS: ScriptElementKindModifier =
    ScriptElementKindModifier::CTS;
pub const SCRIPT_ELEMENT_KIND_MODIFIER_CJS: ScriptElementKindModifier =
    ScriptElementKindModifier::CJS;

impl std::ops::BitOr for ScriptElementKindModifier {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for ScriptElementKindModifier {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl std::ops::BitAnd for ScriptElementKindModifier {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl std::ops::Not for ScriptElementKindModifier {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

struct ScriptElementKindModifierName {
    flag: ScriptElementKindModifier,
    name: &'static str,
}

const SCRIPT_ELEMENT_KIND_MODIFIER_NAMES: &[ScriptElementKindModifierName] = &[
    ScriptElementKindModifierName {
        flag: ScriptElementKindModifier::PUBLIC,
        name: "public",
    },
    ScriptElementKindModifierName {
        flag: ScriptElementKindModifier::PRIVATE,
        name: "private",
    },
    ScriptElementKindModifierName {
        flag: ScriptElementKindModifier::PROTECTED,
        name: "protected",
    },
    ScriptElementKindModifierName {
        flag: ScriptElementKindModifier::EXPORTED,
        name: "export",
    },
    ScriptElementKindModifierName {
        flag: ScriptElementKindModifier::AMBIENT,
        name: "declare",
    },
    ScriptElementKindModifierName {
        flag: ScriptElementKindModifier::STATIC,
        name: "static",
    },
    ScriptElementKindModifierName {
        flag: ScriptElementKindModifier::ABSTRACT,
        name: "abstract",
    },
    ScriptElementKindModifierName {
        flag: ScriptElementKindModifier::OPTIONAL,
        name: "optional",
    },
    ScriptElementKindModifierName {
        flag: ScriptElementKindModifier::DEPRECATED,
        name: "deprecated",
    },
    ScriptElementKindModifierName {
        flag: ScriptElementKindModifier::DTS,
        name: ".d.ts",
    },
    ScriptElementKindModifierName {
        flag: ScriptElementKindModifier::TS,
        name: ".ts",
    },
    ScriptElementKindModifierName {
        flag: ScriptElementKindModifier::TSX,
        name: ".tsx",
    },
    ScriptElementKindModifierName {
        flag: ScriptElementKindModifier::JS,
        name: ".js",
    },
    ScriptElementKindModifierName {
        flag: ScriptElementKindModifier::JSX,
        name: ".jsx",
    },
    ScriptElementKindModifierName {
        flag: ScriptElementKindModifier::JSON,
        name: ".json",
    },
    ScriptElementKindModifierName {
        flag: ScriptElementKindModifier::DMTS,
        name: ".d.mts",
    },
    ScriptElementKindModifierName {
        flag: ScriptElementKindModifier::MTS,
        name: ".mts",
    },
    ScriptElementKindModifierName {
        flag: ScriptElementKindModifier::MJS,
        name: ".mjs",
    },
    ScriptElementKindModifierName {
        flag: ScriptElementKindModifier::DCTS,
        name: ".d.cts",
    },
    ScriptElementKindModifierName {
        flag: ScriptElementKindModifier::CTS,
        name: ".cts",
    },
    ScriptElementKindModifierName {
        flag: ScriptElementKindModifier::CJS,
        name: ".cjs",
    },
];

pub const FILE_EXTENSION_KIND_MODIFIERS: ScriptElementKindModifier = ScriptElementKindModifier(
    ScriptElementKindModifier::DTS.0
        | ScriptElementKindModifier::TS.0
        | ScriptElementKindModifier::TSX.0
        | ScriptElementKindModifier::JS.0
        | ScriptElementKindModifier::JSX.0
        | ScriptElementKindModifier::JSON.0
        | ScriptElementKindModifier::DMTS.0
        | ScriptElementKindModifier::MTS.0
        | ScriptElementKindModifier::MJS.0
        | ScriptElementKindModifier::DCTS.0
        | ScriptElementKindModifier::CTS.0
        | ScriptElementKindModifier::CJS.0,
);

pub fn get_symbol_kind<'a>(
    store: &ast::AstStore,
    type_checker: &mut checker::Checker<'a, '_>,
    symbol: ast::SymbolIdentity,
    location: ast::Node,
) -> ScriptElementKind {
    let roots = type_checker.get_root_symbols_public(symbol);
    let symbol_flags = type_checker.symbol_flags_public(symbol).unwrap_or_default();
    let flags = type_checker
        .symbol_combined_local_and_export_flags_public(symbol)
        .unwrap_or_default();
    let check_flags = type_checker
        .symbol_check_flags_public(symbol)
        .unwrap_or_default();
    let value_declaration = type_checker.symbol_value_declaration_public(symbol);
    let declarations = type_checker.collect_symbol_declarations_public(symbol);

    let is_single_method_root = roots.len() == 1 && {
        type_checker
            .symbol_flags_public(roots[0])
            .unwrap_or(symbol_flags)
            & ast::SYMBOL_FLAGS_METHOD
            != 0
    };
    if roots.len() == 1 && is_single_method_root && {
        let type_at_location = type_checker
            .get_type_of_symbol_identity_at_location_public(symbol, Some(location))
            .unwrap_or_else(|| type_checker.get_error_type());
        let non_nullable = type_checker.get_non_nullable_type_public(type_at_location);
        !type_checker.get_call_signatures(non_nullable).is_empty()
    } {
        return ScriptElementKind::MemberFunctionElement;
    }

    if type_checker.is_undefined_symbol(symbol) {
        return ScriptElementKind::VariableElement;
    }
    if type_checker.is_arguments_symbol(symbol) {
        return ScriptElementKind::LocalVariableElement;
    }
    if store.kind(location) == ast::Kind::ThisKeyword && ast::is_expression(store, location)
        || ast::is_this_in_type_query(store, &location)
    {
        return ScriptElementKind::ParameterElement;
    }

    if flags & ast::SYMBOL_FLAGS_VARIABLE != 0 {
        if is_first_declaration_of_symbol_parameter(store, &declarations) {
            return ScriptElementKind::ParameterElement;
        } else if value_declaration.is_some_and(|declaration| ast::is_var_const(store, declaration))
        {
            return ScriptElementKind::ConstElement;
        } else if value_declaration.is_some_and(|declaration| ast::is_var_using(store, declaration))
        {
            return ScriptElementKind::VariableUsingElement;
        } else if value_declaration
            .is_some_and(|declaration| ast::is_var_await_using(store, declaration))
        {
            return ScriptElementKind::VariableAwaitUsingElement;
        } else if declarations
            .iter()
            .any(|declaration| ast::is_let(store, declaration))
        {
            return ScriptElementKind::LetElement;
        }
        if is_local_variable_or_function_info(
            store,
            &declarations,
            symbol_parent_exists(type_checker, symbol),
        ) {
            return ScriptElementKind::LocalVariableElement;
        }
        return ScriptElementKind::VariableElement;
    }
    if flags & ast::SYMBOL_FLAGS_FUNCTION != 0 {
        if is_local_variable_or_function_info(
            store,
            &declarations,
            symbol_parent_exists(type_checker, symbol),
        ) {
            return ScriptElementKind::LocalFunctionElement;
        }
        return ScriptElementKind::FunctionElement;
    }
    if flags & ast::SYMBOL_FLAGS_GET_ACCESSOR != 0 {
        return ScriptElementKind::MemberGetAccessorElement;
    }
    if flags & ast::SYMBOL_FLAGS_SET_ACCESSOR != 0 {
        return ScriptElementKind::MemberSetAccessorElement;
    }
    if flags & ast::SYMBOL_FLAGS_METHOD != 0 {
        return ScriptElementKind::MemberFunctionElement;
    }
    if flags & ast::SYMBOL_FLAGS_CONSTRUCTOR != 0 {
        return ScriptElementKind::ConstructorImplementationElement;
    }
    if flags & ast::SYMBOL_FLAGS_SIGNATURE != 0 {
        return ScriptElementKind::IndexSignatureElement;
    }
    if flags & ast::SYMBOL_FLAGS_PROPERTY != 0 {
        if flags & ast::SYMBOL_FLAGS_TRANSIENT != 0 && check_flags & ast::CHECK_FLAGS_SYNTHETIC != 0
        {
            let mut union_property_kind = ScriptElementKind::Unknown;
            for root_symbol in &roots {
                if type_checker
                    .symbol_flags_public(*root_symbol)
                    .unwrap_or_default()
                    & (ast::SYMBOL_FLAGS_PROPERTY_OR_ACCESSOR | ast::SYMBOL_FLAGS_VARIABLE)
                    != 0
                {
                    union_property_kind = ScriptElementKind::MemberVariableElement;
                    break;
                }
            }
            if union_property_kind == ScriptElementKind::Unknown {
                let type_of_union_property = type_checker
                    .get_type_of_symbol_identity_at_location_public(symbol, Some(location))
                    .unwrap_or_else(|| type_checker.get_error_type());
                if !type_checker
                    .get_call_signatures(type_of_union_property)
                    .is_empty()
                {
                    return ScriptElementKind::MemberFunctionElement;
                }
                return ScriptElementKind::MemberVariableElement;
            }
            return union_property_kind;
        }
        return ScriptElementKind::MemberVariableElement;
    }
    if flags & ast::SYMBOL_FLAGS_CLASS != 0 {
        if get_declaration_of_kind(store, &declarations, ast::Kind::ClassExpression).is_some() {
            return ScriptElementKind::LocalClassElement;
        }
        return ScriptElementKind::ClassElement;
    }
    if flags & ast::SYMBOL_FLAGS_ENUM != 0 {
        return ScriptElementKind::EnumElement;
    }
    if flags & ast::SYMBOL_FLAGS_TYPE_ALIAS != 0 {
        return ScriptElementKind::TypeElement;
    }
    if flags & ast::SYMBOL_FLAGS_INTERFACE != 0 {
        return ScriptElementKind::InterfaceElement;
    }
    if flags & ast::SYMBOL_FLAGS_TYPE_PARAMETER != 0 {
        return ScriptElementKind::TypeParameterElement;
    }
    if flags & ast::SYMBOL_FLAGS_ENUM_MEMBER != 0 {
        return ScriptElementKind::EnumMemberElement;
    }
    if flags & ast::SYMBOL_FLAGS_ALIAS != 0 {
        return ScriptElementKind::Alias;
    }
    if flags & ast::SYMBOL_FLAGS_MODULE != 0 {
        return ScriptElementKind::ModuleElement;
    }

    ScriptElementKind::Unknown
}

pub fn get_symbol_modifiers<'a>(
    store: &ast::AstStore,
    type_checker: &mut checker::Checker<'a, '_>,
    symbol: ast::SymbolIdentity,
) -> ScriptElementKindModifier {
    let symbol_flags = type_checker.symbol_flags_public(symbol).unwrap_or_default();
    let mut modifiers = get_normalized_symbol_modifiers(store, type_checker, symbol);
    if symbol_flags & ast::SYMBOL_FLAGS_ALIAS != 0
        && let Some(resolved_symbol) = type_checker.get_immediate_aliased_symbol_public(symbol)
        && resolved_symbol != symbol
        && let Some(declaration) = type_checker
            .collect_symbol_declarations_public(resolved_symbol)
            .first()
    {
        modifiers |= get_node_modifiers(store, None, *declaration);
    }
    if symbol_flags & ast::SYMBOL_FLAGS_OPTIONAL != 0 {
        modifiers |= ScriptElementKindModifier::OPTIONAL;
    }

    modifiers
}

fn get_normalized_symbol_modifiers<'a>(
    store: &ast::AstStore,
    type_checker: &mut checker::Checker<'a, '_>,
    symbol: ast::SymbolIdentity,
) -> ScriptElementKindModifier {
    let declarations = type_checker.collect_symbol_declarations_public(symbol);
    let Some(&declaration) = declarations.first() else {
        return ScriptElementKindModifier::NONE;
    };
    get_node_modifiers(store, Some(type_checker), declaration)
}

fn symbol_parent_exists(
    type_checker: &mut checker::Checker<'_, '_>,
    symbol: ast::SymbolIdentity,
) -> bool {
    type_checker.symbol_parent_public(symbol).is_some()
}

fn get_declaration_of_kind(
    store: &ast::AstStore,
    declarations: &[ast::Node],
    kind: ast::Kind,
) -> Option<ast::Node> {
    declarations
        .iter()
        .copied()
        .find(|declaration| store.kind(*declaration) == kind)
}

fn is_first_declaration_of_symbol_parameter(
    store: &ast::AstStore,
    declarations: &[ast::Node],
) -> bool {
    let declaration = declarations.first();
    let result = declaration.and_then(|declaration| {
        ast::find_ancestor_or_quit(store, Some(*declaration), |store, n| {
            if ast::is_parameter_declaration(store, n) {
                return ast::FindAncestorResult::True;
            }
            if ast::is_binding_element(store, n)
                || ast::is_object_binding_pattern(store, n)
                || ast::is_array_binding_pattern(store, n)
            {
                return ast::FindAncestorResult::False;
            }
            ast::FindAncestorResult::Quit
        })
    });

    result.is_some()
}

fn is_local_variable_or_function_info(
    store: &ast::AstStore,
    declarations: &[ast::Node],
    has_parent: bool,
) -> bool {
    if has_parent {
        return false;
    }

    for decl in declarations {
        if store.kind(*decl) == ast::Kind::FunctionExpression {
            return true;
        }

        if store.kind(*decl) != ast::Kind::VariableDeclaration
            && store.kind(*decl) != ast::Kind::FunctionDeclaration
        {
            continue;
        }

        let mut parent = store.parent(*decl);
        while parent
            .as_ref()
            .is_some_and(|parent| !ast::is_function_block(store, Some(*parent)))
        {
            if parent.as_ref().is_some_and(|parent| {
                store.kind(*parent) == ast::Kind::SourceFile
                    || store.kind(*parent) == ast::Kind::ModuleBlock
            }) {
                break;
            }
            parent = store.parent(parent.unwrap());
        }

        if parent
            .as_ref()
            .is_some_and(|parent| ast::is_function_block(store, Some(*parent)))
        {
            return true;
        }
    }
    false
}

pub fn is_deprecated_declaration<'a>(
    store: &ast::AstStore,
    type_checker: Option<&mut checker::Checker<'a, '_>>,
    declaration: ast::Node,
) -> bool {
    if let Some(type_checker) = type_checker {
        return type_checker.is_deprecated_declaration(declaration);
    }
    ast::is_deprecated_declaration(store, &declaration)
}

pub fn get_node_modifiers<'a>(
    store: &ast::AstStore,
    type_checker: Option<&mut checker::Checker<'a, '_>>,
    node: ast::Node,
) -> ScriptElementKindModifier {
    let mut result = ScriptElementKindModifier::NONE;
    let mut flags = ast::ModifierFlags::NONE;
    if ast::is_declaration(store, node) {
        flags = ast::get_combined_modifier_flags(store, node);
        let _ = type_checker;
    }

    if flags.contains(ast::ModifierFlags::PRIVATE) {
        result |= ScriptElementKindModifier::PRIVATE;
    }
    if flags.contains(ast::ModifierFlags::PROTECTED) {
        result |= ScriptElementKindModifier::PROTECTED;
    }
    if flags.contains(ast::ModifierFlags::PUBLIC) {
        result |= ScriptElementKindModifier::PUBLIC;
    }
    if flags.contains(ast::ModifierFlags::STATIC) {
        result |= ScriptElementKindModifier::STATIC;
    }
    if flags.contains(ast::ModifierFlags::ABSTRACT) {
        result |= ScriptElementKindModifier::ABSTRACT;
    }
    if flags.contains(ast::ModifierFlags::EXPORT) {
        result |= ScriptElementKindModifier::EXPORTED;
    }
    if flags.contains(ast::ModifierFlags::AMBIENT) {
        result |= ScriptElementKindModifier::AMBIENT;
    }
    if store.flags(node).contains(ast::NodeFlags::AMBIENT) {
        result |= ScriptElementKindModifier::AMBIENT;
    }
    if store.kind(node) == ast::Kind::ExportAssignment {
        result |= ScriptElementKindModifier::EXPORTED;
    }

    result
}
