use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};

use crate::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SubtreeFacts(pub u32);

impl SubtreeFacts {
    // Facts
    // - Flags used to indicate that a node or subtree contains syntax relevant to a specific transform

    pub const CONTAINS_TYPE_SCRIPT: SubtreeFacts = SubtreeFacts(1 << 0);
    pub const CONTAINS_JSX: SubtreeFacts = SubtreeFacts(1 << 1);
    pub const CONTAINS_ES_DECORATORS: SubtreeFacts = SubtreeFacts(1 << 2);
    pub const CONTAINS_USING: SubtreeFacts = SubtreeFacts(1 << 3);
    pub const CONTAINS_CLASS_STATIC_BLOCKS: SubtreeFacts = SubtreeFacts(1 << 4);
    pub const CONTAINS_ES_CLASS_FIELDS: SubtreeFacts = SubtreeFacts(1 << 5);
    pub const CONTAINS_LOGICAL_ASSIGNMENTS: SubtreeFacts = SubtreeFacts(1 << 6);
    pub const CONTAINS_NULLISH_COALESCING: SubtreeFacts = SubtreeFacts(1 << 7);
    pub const CONTAINS_OPTIONAL_CHAINING: SubtreeFacts = SubtreeFacts(1 << 8);
    pub const CONTAINS_MISSING_CATCH_CLAUSE_VARIABLE: SubtreeFacts = SubtreeFacts(1 << 9);
    pub const CONTAINS_ES_OBJECT_REST_OR_SPREAD: SubtreeFacts = SubtreeFacts(1 << 10); // subtree has a `...` somewhere inside it, never cleared
    #[allow(non_upper_case_globals)]
    pub const ContainsESObjectRestOrSpread: SubtreeFacts = Self::CONTAINS_ES_OBJECT_REST_OR_SPREAD;
    pub const CONTAINS_FOR_AWAIT_OR_ASYNC_GENERATOR: SubtreeFacts = SubtreeFacts(1 << 11);
    pub const CONTAINS_ANY_AWAIT: SubtreeFacts = SubtreeFacts(1 << 12);
    pub const CONTAINS_EXPONENTIATION_OPERATOR: SubtreeFacts = SubtreeFacts(1 << 13);

    // Markers
    // - Flags used to indicate that a node or subtree contains a particular kind of syntax.

    pub const CONTAINS_LEXICAL_THIS: SubtreeFacts = SubtreeFacts(1 << 14);
    pub const CONTAINS_LEXICAL_SUPER: SubtreeFacts = SubtreeFacts(1 << 15);
    pub const CONTAINS_REST_OR_SPREAD: SubtreeFacts = SubtreeFacts(1 << 16); // marker on any `...` - cleared on binding pattern exit
    pub const CONTAINS_OBJECT_REST_OR_SPREAD: SubtreeFacts = SubtreeFacts(1 << 17); // marker on any `{...x}` - cleared on most scope exits
    #[allow(non_upper_case_globals)]
    pub const ContainsObjectRestOrSpread: SubtreeFacts = Self::CONTAINS_OBJECT_REST_OR_SPREAD;
    pub const CONTAINS_AWAIT: SubtreeFacts = SubtreeFacts(1 << 18);
    pub const CONTAINS_DYNAMIC_IMPORT: SubtreeFacts = SubtreeFacts(1 << 19);
    pub const CONTAINS_CLASS_FIELDS: SubtreeFacts = SubtreeFacts(1 << 20);
    pub const CONTAINS_DECORATORS: SubtreeFacts = SubtreeFacts(1 << 21);
    pub const CONTAINS_IDENTIFIER: SubtreeFacts = SubtreeFacts(1 << 22);
    pub const CONTAINS_PRIVATE_IDENTIFIER_IN_EXPRESSION: SubtreeFacts = SubtreeFacts(1 << 23);
    pub const CONTAINS_INVALID_TEMPLATE_ESCAPE: SubtreeFacts = SubtreeFacts(1 << 24);

    pub const COMPUTED: SubtreeFacts = SubtreeFacts(1 << 25); // NOTE: This should always be last
    pub const NONE: SubtreeFacts = SubtreeFacts(0);

    // Aliases (unused, for documentation purposes only - correspond to combinations in transformers/estransforms/definitions.go)

    pub const CONTAINS_ES_NEXT: SubtreeFacts =
        SubtreeFacts(Self::CONTAINS_ES_DECORATORS.0 | Self::CONTAINS_USING.0);
    pub const CONTAINS_ES2022: SubtreeFacts =
        SubtreeFacts(Self::CONTAINS_CLASS_STATIC_BLOCKS.0 | Self::CONTAINS_ES_CLASS_FIELDS.0);
    pub const CONTAINS_ES2021: SubtreeFacts = Self::CONTAINS_LOGICAL_ASSIGNMENTS;
    pub const CONTAINS_ES2020: SubtreeFacts =
        SubtreeFacts(Self::CONTAINS_NULLISH_COALESCING.0 | Self::CONTAINS_OPTIONAL_CHAINING.0);
    pub const CONTAINS_ES2019: SubtreeFacts = Self::CONTAINS_MISSING_CATCH_CLAUSE_VARIABLE;
    pub const CONTAINS_ES2018: SubtreeFacts = SubtreeFacts(
        Self::CONTAINS_ES_OBJECT_REST_OR_SPREAD.0
            | Self::CONTAINS_FOR_AWAIT_OR_ASYNC_GENERATOR.0
            | Self::CONTAINS_INVALID_TEMPLATE_ESCAPE.0,
    );
    pub const CONTAINS_ES2017: SubtreeFacts = Self::CONTAINS_ANY_AWAIT;
    pub const CONTAINS_ES2016: SubtreeFacts = Self::CONTAINS_EXPONENTIATION_OPERATOR;

    // Scope Exclusions
    // - Bitmasks that exclude flags from propagating out of a specific context
    //   into the subtree flags of their container.

    pub const EXCLUSIONS_NODE: SubtreeFacts = Self::COMPUTED;
    pub const EXCLUSIONS_ERASEABLE: SubtreeFacts = SubtreeFacts(!Self::CONTAINS_TYPE_SCRIPT.0);
    pub const EXCLUSIONS_OUTER_EXPRESSION: SubtreeFacts = Self::EXCLUSIONS_NODE;
    pub const EXCLUSIONS_PROPERTY_ACCESS: SubtreeFacts = Self::EXCLUSIONS_NODE;
    pub const EXCLUSIONS_ELEMENT_ACCESS: SubtreeFacts = Self::EXCLUSIONS_NODE;
    pub const EXCLUSIONS_ARROW_FUNCTION: SubtreeFacts = SubtreeFacts(
        Self::EXCLUSIONS_NODE.0 | Self::CONTAINS_AWAIT.0 | Self::CONTAINS_OBJECT_REST_OR_SPREAD.0,
    );
    pub const EXCLUSIONS_FUNCTION: SubtreeFacts = SubtreeFacts(
        Self::EXCLUSIONS_NODE.0
            | Self::CONTAINS_LEXICAL_THIS.0
            | Self::CONTAINS_LEXICAL_SUPER.0
            | Self::CONTAINS_AWAIT.0
            | Self::CONTAINS_OBJECT_REST_OR_SPREAD.0,
    );
    pub const EXCLUSIONS_CONSTRUCTOR: SubtreeFacts = Self::EXCLUSIONS_FUNCTION;
    pub const EXCLUSIONS_METHOD: SubtreeFacts = Self::EXCLUSIONS_FUNCTION;
    pub const EXCLUSIONS_ACCESSOR: SubtreeFacts = Self::EXCLUSIONS_FUNCTION;
    pub const EXCLUSIONS_PROPERTY: SubtreeFacts = SubtreeFacts(
        Self::EXCLUSIONS_NODE.0 | Self::CONTAINS_LEXICAL_THIS.0 | Self::CONTAINS_LEXICAL_SUPER.0,
    );
    pub const EXCLUSIONS_CLASS: SubtreeFacts = Self::EXCLUSIONS_NODE;
    pub const EXCLUSIONS_MODULE: SubtreeFacts = SubtreeFacts(
        Self::EXCLUSIONS_NODE.0 | Self::CONTAINS_LEXICAL_THIS.0 | Self::CONTAINS_LEXICAL_SUPER.0,
    );
    pub const EXCLUSIONS_OBJECT_LITERAL: SubtreeFacts =
        SubtreeFacts(Self::EXCLUSIONS_NODE.0 | Self::CONTAINS_OBJECT_REST_OR_SPREAD.0);
    pub const EXCLUSIONS_ARRAY_LITERAL: SubtreeFacts = Self::EXCLUSIONS_NODE;
    pub const EXCLUSIONS_CALL: SubtreeFacts = Self::EXCLUSIONS_NODE;
    pub const EXCLUSIONS_NEW: SubtreeFacts = Self::EXCLUSIONS_NODE;
    pub const EXCLUSIONS_VARIABLE_DECLARATION_LIST: SubtreeFacts =
        SubtreeFacts(Self::EXCLUSIONS_NODE.0 | Self::CONTAINS_OBJECT_REST_OR_SPREAD.0);
    pub const EXCLUSIONS_PARAMETER: SubtreeFacts = Self::EXCLUSIONS_NODE;
    pub const EXCLUSIONS_CATCH_CLAUSE: SubtreeFacts =
        SubtreeFacts(Self::EXCLUSIONS_NODE.0 | Self::CONTAINS_OBJECT_REST_OR_SPREAD.0);
    pub const EXCLUSIONS_BINDING_PATTERN: SubtreeFacts =
        SubtreeFacts(Self::EXCLUSIONS_NODE.0 | Self::CONTAINS_REST_OR_SPREAD.0);

    // Masks
    // - Additional bitmasks

    pub const CONTAINS_LEXICAL_THIS_OR_SUPER: SubtreeFacts =
        SubtreeFacts(Self::CONTAINS_LEXICAL_THIS.0 | Self::CONTAINS_LEXICAL_SUPER.0);

    pub fn contains(self, other: SubtreeFacts) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn intersects(self, other: SubtreeFacts) -> bool {
        self.0 & other.0 != 0
    }

    pub fn from_bits_retain(bits: u32) -> SubtreeFacts {
        SubtreeFacts(bits)
    }

    pub fn bits(self) -> u32 {
        self.0
    }
}

impl BitOr for SubtreeFacts {
    type Output = SubtreeFacts;

    fn bitor(self, rhs: SubtreeFacts) -> SubtreeFacts {
        SubtreeFacts(self.0 | rhs.0)
    }
}

impl BitOrAssign for SubtreeFacts {
    fn bitor_assign(&mut self, rhs: SubtreeFacts) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for SubtreeFacts {
    type Output = SubtreeFacts;

    fn bitand(self, rhs: SubtreeFacts) -> SubtreeFacts {
        SubtreeFacts(self.0 & rhs.0)
    }
}

impl BitAndAssign for SubtreeFacts {
    fn bitand_assign(&mut self, rhs: SubtreeFacts) {
        self.0 &= rhs.0;
    }
}

impl Not for SubtreeFacts {
    type Output = SubtreeFacts;

    fn not(self) -> SubtreeFacts {
        SubtreeFacts(!self.0)
    }
}

pub(crate) fn propagate_eraseable_syntax_list_subtree_facts(
    children: OptionalNodeListId,
) -> SubtreeFacts {
    if children.get().is_some() {
        SubtreeFacts::CONTAINS_TYPE_SCRIPT
    } else {
        SubtreeFacts::NONE
    }
}

fn propagate_eraseable_syntax_optional_list_subtree_facts(
    children: Option<NodeListId>,
) -> SubtreeFacts {
    if children.is_some() {
        SubtreeFacts::CONTAINS_TYPE_SCRIPT
    } else {
        SubtreeFacts::NONE
    }
}

pub fn propagate_eraseable_syntax_subtree_facts(child: Option<&Node>) -> SubtreeFacts {
    if child.is_some() {
        SubtreeFacts::CONTAINS_TYPE_SCRIPT
    } else {
        SubtreeFacts::NONE
    }
}

pub fn propagate_object_binding_element_subtree_facts(
    store: &AstStore,
    child: Option<&Node>,
) -> SubtreeFacts {
    let mut facts = propagate_subtree_facts(store, child);
    if facts.contains(SubtreeFacts::CONTAINS_REST_OR_SPREAD) {
        facts &= !SubtreeFacts::CONTAINS_REST_OR_SPREAD;
        facts |= SubtreeFacts::CONTAINS_OBJECT_REST_OR_SPREAD
            | SubtreeFacts::CONTAINS_ES_OBJECT_REST_OR_SPREAD;
    }
    facts
}

pub fn propagate_binding_element_subtree_facts(
    store: &AstStore,
    child: Option<&Node>,
) -> SubtreeFacts {
    propagate_subtree_facts(store, child) & !SubtreeFacts::CONTAINS_REST_OR_SPREAD
}

pub fn propagate_subtree_facts(store: &AstStore, child: Option<&Node>) -> SubtreeFacts {
    let Some(child) = child else {
        return SubtreeFacts::NONE;
    };
    let node = *child;
    let facts = compute_subtree_facts(store, node) & !subtree_exclusions(store, node);
    match store.kind(node) {
        Kind::MethodDeclaration
        | Kind::GetAccessor
        | Kind::SetAccessor
        | Kind::PropertyDeclaration => {
            facts | propagate_subtree_facts(store, store.name(node).as_ref())
        }
        _ => facts,
    }
}

pub fn compute_subtree_facts(store: &AstStore, node: Node) -> SubtreeFacts {
    if store.kind(node) == Kind::BinaryExpression {
        return compute_binary_expression_subtree_facts(store, node);
    }

    match store.kind(node) {
        Kind::CallExpression => {
            let call = store.as_call_expression(node);
            let expression = store.optional_node_from_id(call.expression);
            return propagate_subtree_facts(store, expression.as_ref())
                | propagate_subtree_facts(store, store.question_dot_token(node).as_ref())
                | propagate_eraseable_syntax_optional_list_subtree_facts(
                    call.type_arguments.get(),
                )
                | propagate_node_list_subtree_facts(
                    store,
                    call.arguments,
                    propagate_subtree_facts,
                )
                | if expression
                    .is_some_and(|expression| store.kind(expression) == Kind::ImportKeyword)
                {
                    SubtreeFacts::CONTAINS_DYNAMIC_IMPORT
                } else {
                    SubtreeFacts::NONE
                };
        }
        Kind::FunctionDeclaration => {
            if store.body(node).is_none()
                || has_syntactic_modifier(store, node, ModifierFlags::AMBIENT)
            {
                return SubtreeFacts::CONTAINS_TYPE_SCRIPT;
            }
            let is_async = has_syntactic_modifier(store, node, ModifierFlags::ASYNC);
            let is_generator = store.asterisk_token(node).is_some();
            return propagate_modifier_list_subtree_facts(
                store,
                OptionalModifierListId::from_option(store.modifiers_id(node)),
            ) | propagate_subtree_facts(store, store.asterisk_token(node).as_ref())
                | propagate_subtree_facts(store, store.name(node).as_ref())
                | propagate_eraseable_syntax_optional_list_subtree_facts(
                    store.type_parameters_id(node),
                )
                | propagate_node_list_subtree_facts(
                    store,
                    store
                        .parameters_id(node)
                        .expect("function parameters are required"),
                    propagate_subtree_facts,
                )
                | propagate_eraseable_syntax_subtree_facts(store.r#type(node).as_ref())
                | propagate_eraseable_syntax_subtree_facts(store.full_signature(node).as_ref())
                | propagate_subtree_facts(store, store.body(node).as_ref())
                | if is_async && is_generator {
                    SubtreeFacts::CONTAINS_FOR_AWAIT_OR_ASYNC_GENERATOR
                } else {
                    SubtreeFacts::NONE
                }
                | if is_async && !is_generator {
                    SubtreeFacts::CONTAINS_ANY_AWAIT
                } else {
                    SubtreeFacts::NONE
                };
        }
        Kind::FunctionExpression => {
            let is_async = has_syntactic_modifier(store, node, ModifierFlags::ASYNC);
            let is_generator = store.asterisk_token(node).is_some();
            return propagate_modifier_list_subtree_facts(
                store,
                OptionalModifierListId::from_option(store.modifiers_id(node)),
            ) | propagate_subtree_facts(store, store.asterisk_token(node).as_ref())
                | propagate_subtree_facts(store, store.name(node).as_ref())
                | propagate_eraseable_syntax_optional_list_subtree_facts(
                    store.type_parameters_id(node),
                )
                | propagate_node_list_subtree_facts(
                    store,
                    store
                        .parameters_id(node)
                        .expect("function parameters are required"),
                    propagate_subtree_facts,
                )
                | propagate_eraseable_syntax_subtree_facts(store.r#type(node).as_ref())
                | propagate_eraseable_syntax_subtree_facts(store.full_signature(node).as_ref())
                | propagate_subtree_facts(store, store.body(node).as_ref())
                | if is_async && is_generator {
                    SubtreeFacts::CONTAINS_FOR_AWAIT_OR_ASYNC_GENERATOR
                } else {
                    SubtreeFacts::NONE
                }
                | if is_async && !is_generator {
                    SubtreeFacts::CONTAINS_ANY_AWAIT
                } else {
                    SubtreeFacts::NONE
                };
        }
        Kind::ArrowFunction => {
            let is_async = has_syntactic_modifier(store, node, ModifierFlags::ASYNC);
            return propagate_modifier_list_subtree_facts(
                store,
                OptionalModifierListId::from_option(store.modifiers_id(node)),
            ) | propagate_eraseable_syntax_optional_list_subtree_facts(
                store.type_parameters_id(node),
            ) | propagate_node_list_subtree_facts(
                store,
                store
                    .parameters_id(node)
                    .expect("arrow function parameters are required"),
                propagate_subtree_facts,
            ) | propagate_eraseable_syntax_subtree_facts(store.r#type(node).as_ref())
                | propagate_eraseable_syntax_subtree_facts(store.full_signature(node).as_ref())
                | propagate_subtree_facts(store, store.body(node).as_ref())
                | if is_async {
                    SubtreeFacts::CONTAINS_ANY_AWAIT
                } else {
                    SubtreeFacts::NONE
                };
        }
        Kind::Constructor => {
            if store.body(node).is_none() {
                return SubtreeFacts::CONTAINS_TYPE_SCRIPT;
            }
            return propagate_modifier_list_subtree_facts(
                store,
                OptionalModifierListId::from_option(store.modifiers_id(node)),
            ) | propagate_eraseable_syntax_optional_list_subtree_facts(
                store.type_parameters_id(node),
            ) | propagate_node_list_subtree_facts(
                store,
                store
                    .parameters_id(node)
                    .expect("constructor parameters are required"),
                propagate_subtree_facts,
            ) | propagate_eraseable_syntax_subtree_facts(store.r#type(node).as_ref())
                | propagate_eraseable_syntax_subtree_facts(store.full_signature(node).as_ref())
                | propagate_subtree_facts(store, store.body(node).as_ref());
        }
        Kind::GetAccessor | Kind::SetAccessor => {
            if store.body(node).is_none() {
                return SubtreeFacts::CONTAINS_TYPE_SCRIPT;
            }
            return propagate_modifier_list_subtree_facts(
                store,
                OptionalModifierListId::from_option(store.modifiers_id(node)),
            ) | propagate_subtree_facts(store, store.name(node).as_ref())
                | propagate_eraseable_syntax_optional_list_subtree_facts(
                    store.type_parameters_id(node),
                )
                | propagate_node_list_subtree_facts(
                    store,
                    store
                        .parameters_id(node)
                        .expect("accessor parameters are required"),
                    propagate_subtree_facts,
                )
                | propagate_eraseable_syntax_subtree_facts(store.r#type(node).as_ref())
                | propagate_eraseable_syntax_subtree_facts(store.full_signature(node).as_ref())
                | propagate_subtree_facts(store, store.body(node).as_ref());
        }
        Kind::Parameter => {
            if is_this_parameter(store, node) {
                return SubtreeFacts::CONTAINS_TYPE_SCRIPT;
            }
            return propagate_modifier_list_subtree_facts(
                store,
                OptionalModifierListId::from_option(store.modifiers_id(node)),
            ) | propagate_subtree_facts(store, store.name(node).as_ref())
                | propagate_eraseable_syntax_subtree_facts(store.question_token(node).as_ref())
                | propagate_eraseable_syntax_subtree_facts(store.r#type(node).as_ref())
                | propagate_subtree_facts(store, store.initializer(node).as_ref());
        }
        Kind::PropertyDeclaration => {
            return propagate_modifier_list_subtree_facts(
                store,
                OptionalModifierListId::from_option(store.modifiers_id(node)),
            ) | propagate_subtree_facts(store, store.name(node).as_ref())
                | propagate_eraseable_syntax_subtree_facts(store.postfix_token(node).as_ref())
                | propagate_eraseable_syntax_subtree_facts(store.r#type(node).as_ref())
                | propagate_subtree_facts(store, store.initializer(node).as_ref())
                | SubtreeFacts::CONTAINS_CLASS_FIELDS;
        }
        Kind::MethodDeclaration => {
            if store.body(node).is_none() {
                return SubtreeFacts::CONTAINS_TYPE_SCRIPT;
            }
            let is_async = has_syntactic_modifier(store, node, ModifierFlags::ASYNC);
            let is_generator = store.asterisk_token(node).is_some();
            return propagate_modifier_list_subtree_facts(
                store,
                OptionalModifierListId::from_option(store.modifiers_id(node)),
            ) | propagate_subtree_facts(store, store.asterisk_token(node).as_ref())
                | propagate_subtree_facts(store, store.name(node).as_ref())
                | propagate_eraseable_syntax_subtree_facts(store.postfix_token(node).as_ref())
                | propagate_eraseable_syntax_optional_list_subtree_facts(
                    store.type_parameters_id(node),
                )
                | propagate_node_list_subtree_facts(
                    store,
                    store
                        .parameters_id(node)
                        .expect("method parameters are required"),
                    propagate_subtree_facts,
                )
                | propagate_subtree_facts(store, store.body(node).as_ref())
                | propagate_eraseable_syntax_subtree_facts(store.r#type(node).as_ref())
                | propagate_eraseable_syntax_subtree_facts(store.full_signature(node).as_ref())
                | if is_async && is_generator {
                    SubtreeFacts::CONTAINS_FOR_AWAIT_OR_ASYNC_GENERATOR
                } else {
                    SubtreeFacts::NONE
                }
                | if is_async && !is_generator {
                    SubtreeFacts::CONTAINS_ANY_AWAIT
                } else {
                    SubtreeFacts::NONE
                };
        }
        Kind::ClassDeclaration | Kind::ClassExpression => {
            if has_syntactic_modifier(store, node, ModifierFlags::AMBIENT) {
                return SubtreeFacts::CONTAINS_TYPE_SCRIPT;
            }
            return propagate_modifier_list_subtree_facts(
                store,
                OptionalModifierListId::from_option(store.modifiers_id(node)),
            ) | propagate_subtree_facts(store, store.name(node).as_ref())
                | propagate_eraseable_syntax_optional_list_subtree_facts(
                    store.type_parameters_id(node),
                )
                | store
                    .heritage_clauses_id(node)
                    .map_or(SubtreeFacts::NONE, |clauses| {
                        propagate_node_list_subtree_facts(store, clauses, propagate_subtree_facts)
                    })
                | propagate_node_list_subtree_facts(
                    store,
                    store.members_id(node).expect("class members are required"),
                    propagate_subtree_facts,
                );
        }
        Kind::VariableStatement => {
            if has_syntactic_modifier(store, node, ModifierFlags::AMBIENT) {
                return SubtreeFacts::CONTAINS_TYPE_SCRIPT;
            }
        }
        Kind::ImportEqualsDeclaration => {
            if store.is_type_only(node).unwrap_or(false)
                || !store
                    .module_reference(node)
                    .is_some_and(|module_reference| {
                        is_external_module_reference(store, module_reference)
                    })
            {
                return SubtreeFacts::CONTAINS_TYPE_SCRIPT;
            }
            return propagate_modifier_list_subtree_facts(
                store,
                OptionalModifierListId::from_option(store.modifiers_id(node)),
            ) | propagate_subtree_facts(store, store.name(node).as_ref())
                | propagate_subtree_facts(store, store.module_reference(node).as_ref());
        }
        Kind::ImportSpecifier => {
            if store.is_type_only(node).unwrap_or(false) {
                return SubtreeFacts::CONTAINS_TYPE_SCRIPT;
            }
            return propagate_subtree_facts(store, store.property_name(node).as_ref())
                | propagate_subtree_facts(store, store.name(node).as_ref());
        }
        Kind::ImportClause => {
            if store.phase_modifier(node) == Some(Kind::TypeKeyword) {
                return SubtreeFacts::CONTAINS_TYPE_SCRIPT;
            }
            return propagate_subtree_facts(store, store.name(node).as_ref())
                | propagate_subtree_facts(store, store.named_bindings(node).as_ref());
        }
        Kind::ExportDeclaration => {
            return propagate_modifier_list_subtree_facts(
                store,
                OptionalModifierListId::from_option(store.modifiers_id(node)),
            ) | propagate_subtree_facts(store, store.export_clause(node).as_ref())
                | propagate_subtree_facts(store, store.module_specifier(node).as_ref())
                | propagate_subtree_facts(store, store.attributes(node).as_ref())
                | if store.is_type_only(node).unwrap_or(false) {
                    SubtreeFacts::CONTAINS_TYPE_SCRIPT
                } else {
                    SubtreeFacts::NONE
                };
        }
        Kind::ExportSpecifier => {
            if store.is_type_only(node).unwrap_or(false) {
                return SubtreeFacts::CONTAINS_TYPE_SCRIPT;
            }
            return propagate_subtree_facts(store, store.property_name(node).as_ref())
                | propagate_subtree_facts(store, store.name(node).as_ref());
        }
        _ => {}
    }

    let mut facts = subtree_facts_for_kind(store.kind(node));
    if store.kind(node) == Kind::VariableDeclarationList
        && store.flags(node).intersects(NodeFlags::USING)
    {
        facts |= SubtreeFacts::CONTAINS_USING;
    }
    if matches!(
        store.kind(node),
        Kind::NoSubstitutionTemplateLiteral
            | Kind::TemplateHead
            | Kind::TemplateMiddle
            | Kind::TemplateTail
    ) && store
        .template_flags(node)
        .is_some_and(|flags| flags.intersects(TokenFlags::CONTAINS_INVALID_ESCAPE))
    {
        facts |= SubtreeFacts::CONTAINS_INVALID_TEMPLATE_ESCAPE;
    }
    if store.kind(node) == Kind::ObjectBindingPattern {
        return propagate_node_list_subtree_facts(
            store,
            store.as_binding_pattern(node).elements,
            propagate_object_binding_element_subtree_facts,
        );
    }
    if store.kind(node) == Kind::ArrayBindingPattern {
        return propagate_node_list_subtree_facts(
            store,
            store.as_binding_pattern(node).elements,
            propagate_binding_element_subtree_facts,
        );
    }
    if store.kind(node) == Kind::BindingElement && store.dot_dot_dot_token(node).is_some() {
        facts |= SubtreeFacts::CONTAINS_REST_OR_SPREAD;
    }
    if store.kind(node) == Kind::CatchClause && store.variable_declaration(node).is_none() {
        facts |= SubtreeFacts::CONTAINS_MISSING_CATCH_CLAUSE_VARIABLE;
    }
    if store.kind(node) == Kind::ArrowFunction
        && has_syntactic_modifier(store, node, ModifierFlags::ASYNC)
    {
        facts |= SubtreeFacts::CONTAINS_ANY_AWAIT;
    }
    if store.kind(node) == Kind::HeritageClause {
        let clause = store.as_heritage_clause(node);
        return match clause.token {
            Kind::ExtendsKeyword => {
                propagate_node_list_subtree_facts(store, clause.types, propagate_subtree_facts)
            }
            Kind::ImplementsKeyword => SubtreeFacts::CONTAINS_TYPE_SCRIPT,
            _ => SubtreeFacts::NONE,
        };
    }
    if store.kind(node) == Kind::PropertyAccessExpression {
        if store
            .name(node)
            .is_some_and(|name| !is_identifier(store, name))
        {
            facts |= SubtreeFacts::CONTAINS_PRIVATE_IDENTIFIER_IN_EXPRESSION;
        }
    }
    let _ = store.for_each_present_child(node, |child| {
        facts |= propagate_subtree_facts(store, Some(&child));
        std::ops::ControlFlow::Continue(())
    });
    facts
}

fn compute_binary_expression_subtree_facts(store: &AstStore, node: Node) -> SubtreeFacts {
    let mut facts = SubtreeFacts::NONE;
    let mut stack = vec![node];

    while let Some(node) = stack.pop() {
        let binary = store.as_binary_expression(node);
        let left = store.node_from_id(binary.left);
        let right = store.node_from_id(binary.right);
        let operator_token = store.node_from_id(binary.operator_token);

        facts |= propagate_modifier_list_subtree_facts(store, binary.modifiers);
        facts |=
            propagate_subtree_facts(store, store.optional_node_from_id(binary.r#type).as_ref());
        facts |= propagate_subtree_facts(store, Some(&operator_token));
        if store.kind(operator_token) == Kind::InKeyword && is_private_identifier(store, left) {
            facts |= SubtreeFacts::CONTAINS_CLASS_FIELDS
                | SubtreeFacts::CONTAINS_PRIVATE_IDENTIFIER_IN_EXPRESSION;
        }
        if store.kind(operator_token) == Kind::EqualsToken
            && (is_object_literal_expression(store, left)
                || is_array_literal_expression(store, left))
            && contains_object_rest_or_spread(store, left)
        {
            facts |= SubtreeFacts::CONTAINS_OBJECT_REST_OR_SPREAD;
        }

        for child in [left, right] {
            if store.kind(child) == Kind::BinaryExpression {
                stack.push(child);
            } else {
                facts |= propagate_subtree_facts(store, Some(&child));
            }
        }
    }

    facts
}

/**
 * Walk an AssignmentPattern to determine if it contains object rest (`...`) syntax. We cannot rely on
 * propagation of `TransformFlags.ContainsObjectRestOrSpread` since it isn't propagated by default in
 * ObjectLiteralExpression and ArrayLiteralExpression since we do not know whether they belong to an
 * AssignmentPattern at the time the nodes are parsed.
 */
pub fn contains_object_rest_or_spread(store: &AstStore, node: Node) -> bool {
    if store
        .subtree_facts(node)
        .contains(SubtreeFacts::CONTAINS_OBJECT_REST_OR_SPREAD)
    {
        return true;
    }
    if store
        .subtree_facts(node)
        .contains(SubtreeFacts::CONTAINS_ES_OBJECT_REST_OR_SPREAD)
    {
        // check for nested spread assignments, otherwise '{ x: { a, ...b } = foo } = c'
        // will not be correctly interpreted by the rest/spread transformer
        for element in elements_of_binding_or_assignment_pattern(store, node) {
            if let Some(target) = target_of_binding_or_assignment_element(store, element)
                && is_assignment_pattern(store, target)
            {
                if store
                    .subtree_facts(target)
                    .contains(SubtreeFacts::CONTAINS_OBJECT_REST_OR_SPREAD)
                {
                    return true;
                }
                if store
                    .subtree_facts(target)
                    .contains(SubtreeFacts::CONTAINS_ES_OBJECT_REST_OR_SPREAD)
                    && contains_object_rest_or_spread(store, target)
                {
                    return true;
                }
            }
        }
    }
    false
}

fn elements_of_binding_or_assignment_pattern(store: &AstStore, pattern: Node) -> Vec<Node> {
    match store.kind(pattern) {
        Kind::ObjectBindingPattern | Kind::ArrayBindingPattern | Kind::ArrayLiteralExpression => {
            store
                .elements(pattern)
                .map(|elements| elements.iter().collect())
                .unwrap_or_default()
        }
        Kind::ObjectLiteralExpression => store
            .properties(pattern)
            .map(|properties| properties.iter().collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn target_of_binding_or_assignment_element(store: &AstStore, element: Node) -> Option<Node> {
    match store.kind(element) {
        Kind::VariableDeclaration | Kind::Parameter | Kind::BindingElement => store.name(element),
        Kind::PropertyAssignment => store
            .initializer(element)
            .and_then(|initializer| target_of_binding_or_assignment_element(store, initializer)),
        Kind::ShorthandPropertyAssignment => store.name(element),
        Kind::SpreadAssignment => store
            .expression(element)
            .and_then(|expression| target_of_binding_or_assignment_element(store, expression)),
        Kind::BinaryExpression if is_assignment_expression(store, element, true) => store
            .left(element)
            .and_then(|left| target_of_binding_or_assignment_element(store, left)),
        Kind::SpreadElement => store
            .expression(element)
            .and_then(|expression| target_of_binding_or_assignment_element(store, expression)),
        _ => Some(element),
    }
}

fn is_assignment_pattern(store: &AstStore, node: Node) -> bool {
    is_array_literal_expression(store, node) || is_object_literal_expression(store, node)
}

fn subtree_facts_for_kind(kind: Kind) -> SubtreeFacts {
    let mut facts = SubtreeFacts::NONE;

    if is_type_node_kind(kind) {
        facts |= SubtreeFacts::CONTAINS_TYPE_SCRIPT;
    }

    facts
        | match kind {
            Kind::UsingKeyword => SubtreeFacts::CONTAINS_USING,
            Kind::PublicKeyword
            | Kind::PrivateKeyword
            | Kind::ProtectedKeyword
            | Kind::ReadonlyKeyword
            | Kind::AbstractKeyword
            | Kind::DeclareKeyword
            | Kind::ConstKeyword
            | Kind::AnyKeyword
            | Kind::NumberKeyword
            | Kind::BigIntKeyword
            | Kind::NeverKeyword
            | Kind::ObjectKeyword
            | Kind::InKeyword
            | Kind::OutKeyword
            | Kind::OverrideKeyword
            | Kind::StringKeyword
            | Kind::BooleanKeyword
            | Kind::SymbolKeyword
            | Kind::VoidKeyword
            | Kind::UnknownKeyword
            | Kind::UndefinedKeyword
            | Kind::ExportKeyword
            | Kind::EnumDeclaration
            | Kind::EnumMember
            | Kind::ModuleDeclaration
            | Kind::ImportEqualsDeclaration
            | Kind::NamespaceExportDeclaration
            | Kind::TypeAliasDeclaration
            | Kind::JSTypeAliasDeclaration
            | Kind::InterfaceDeclaration
            | Kind::ShorthandPropertyAssignment
            | Kind::AsExpression
            | Kind::SatisfiesExpression
            | Kind::NonNullExpression
            | Kind::TypeAssertionExpression => SubtreeFacts::CONTAINS_TYPE_SCRIPT,
            Kind::AccessorKeyword
            | Kind::PrivateIdentifier
            | Kind::PropertyDeclaration
            | Kind::ClassStaticBlockDeclaration => SubtreeFacts::CONTAINS_CLASS_FIELDS,
            Kind::StaticKeyword => SubtreeFacts::CONTAINS_CLASS_STATIC_BLOCKS,
            Kind::AsyncKeyword => SubtreeFacts::CONTAINS_ANY_AWAIT,
            Kind::SuperKeyword => SubtreeFacts::CONTAINS_LEXICAL_SUPER,
            Kind::ThisKeyword => SubtreeFacts::CONTAINS_LEXICAL_THIS,
            Kind::AsteriskAsteriskToken | Kind::AsteriskAsteriskEqualsToken => {
                SubtreeFacts::CONTAINS_EXPONENTIATION_OPERATOR
            }
            Kind::QuestionQuestionToken => SubtreeFacts::CONTAINS_NULLISH_COALESCING,
            Kind::QuestionDotToken => SubtreeFacts::CONTAINS_OPTIONAL_CHAINING,
            Kind::QuestionQuestionEqualsToken
            | Kind::BarBarEqualsToken
            | Kind::AmpersandAmpersandEqualsToken => SubtreeFacts::CONTAINS_LOGICAL_ASSIGNMENTS,
            Kind::Identifier => SubtreeFacts::CONTAINS_IDENTIFIER,
            Kind::Decorator => {
                SubtreeFacts::CONTAINS_TYPE_SCRIPT | SubtreeFacts::CONTAINS_DECORATORS
            }
            Kind::AwaitExpression => {
                SubtreeFacts::CONTAINS_AWAIT
                    | SubtreeFacts::CONTAINS_ANY_AWAIT
                    | SubtreeFacts::CONTAINS_FOR_AWAIT_OR_ASYNC_GENERATOR
            }
            Kind::YieldExpression | Kind::ReturnStatement => {
                SubtreeFacts::CONTAINS_FOR_AWAIT_OR_ASYNC_GENERATOR
            }
            Kind::SpreadElement => SubtreeFacts::CONTAINS_REST_OR_SPREAD,
            Kind::SpreadAssignment => {
                SubtreeFacts::CONTAINS_ES_OBJECT_REST_OR_SPREAD
                    | SubtreeFacts::CONTAINS_OBJECT_REST_OR_SPREAD
            }
            Kind::JsxElement
            | Kind::JsxSelfClosingElement
            | Kind::JsxFragment
            | Kind::JsxOpeningElement
            | Kind::JsxClosingElement
            | Kind::JsxOpeningFragment
            | Kind::JsxClosingFragment
            | Kind::JsxExpression
            | Kind::JsxText
            | Kind::JsxTextAllWhiteSpaces
            | Kind::JsxAttribute
            | Kind::JsxAttributes
            | Kind::JsxSpreadAttribute
            | Kind::JsxNamespacedName => SubtreeFacts::CONTAINS_JSX,
            _ => SubtreeFacts::NONE,
        }
}

fn subtree_exclusions(store: &AstStore, node: Node) -> SubtreeFacts {
    match store.kind(node) {
        Kind::ArrowFunction => SubtreeFacts::EXCLUSIONS_ARROW_FUNCTION,
        Kind::FunctionDeclaration | Kind::FunctionExpression => SubtreeFacts::EXCLUSIONS_FUNCTION,
        Kind::Constructor => SubtreeFacts::EXCLUSIONS_CONSTRUCTOR,
        Kind::MethodDeclaration => SubtreeFacts::EXCLUSIONS_METHOD,
        Kind::GetAccessor | Kind::SetAccessor => SubtreeFacts::EXCLUSIONS_ACCESSOR,
        Kind::PropertyDeclaration => SubtreeFacts::EXCLUSIONS_PROPERTY,
        Kind::ClassDeclaration | Kind::ClassExpression => SubtreeFacts::EXCLUSIONS_CLASS,
        Kind::ModuleDeclaration => SubtreeFacts::EXCLUSIONS_MODULE,
        Kind::ObjectLiteralExpression => SubtreeFacts::EXCLUSIONS_OBJECT_LITERAL,
        Kind::ArrayLiteralExpression => SubtreeFacts::EXCLUSIONS_ARRAY_LITERAL,
        Kind::CallExpression => SubtreeFacts::EXCLUSIONS_CALL,
        Kind::NewExpression => SubtreeFacts::EXCLUSIONS_NEW,
        Kind::VariableDeclarationList => SubtreeFacts::EXCLUSIONS_VARIABLE_DECLARATION_LIST,
        Kind::Parameter => SubtreeFacts::EXCLUSIONS_PARAMETER,
        Kind::CatchClause => SubtreeFacts::EXCLUSIONS_CATCH_CLAUSE,
        Kind::ObjectBindingPattern | Kind::ArrayBindingPattern => {
            SubtreeFacts::EXCLUSIONS_BINDING_PATTERN
        }
        Kind::AsExpression
        | Kind::SatisfiesExpression
        | Kind::TypeAssertionExpression
        | Kind::PropertyAccessExpression
        | Kind::ElementAccessExpression => SubtreeFacts::EXCLUSIONS_OUTER_EXPRESSION,
        _ => SubtreeFacts::EXCLUSIONS_NODE,
    }
}

pub(crate) fn propagate_node_list_subtree_facts(
    store: &AstStore,
    children: NodeListId,
    propagate: fn(&AstStore, Option<&Node>) -> SubtreeFacts,
) -> SubtreeFacts {
    let children = store.node_list(children);
    let mut facts = children.iter().fold(SubtreeFacts::NONE, |facts, child| {
        facts | propagate(store, Some(&child))
    });
    facts &= !SubtreeFacts::EXCLUSIONS_NODE;
    facts
}

pub(crate) fn propagate_modifier_list_subtree_facts(
    store: &AstStore,
    children: OptionalModifierListId,
) -> SubtreeFacts {
    let Some(children) = store.optional_modifier_list(children) else {
        return SubtreeFacts::NONE;
    };
    children
        .nodes()
        .iter()
        .fold(SubtreeFacts::NONE, |facts, child| {
            facts | propagate_subtree_facts(store, Some(&child))
        })
}
