use std::collections::{HashMap, HashSet};

use ts_ast as ast;
use ts_ast::{AstGeneratedVisitEachChild as _, AstVisitEachChildRuntime as _};
use ts_core::{self as core, ScriptTarget};
use ts_printer::{self as printer, AutoGenerateOptions, GeneratedIdentifierFlags};
use ts_scanner as scanner;

use crate::estransforms::classthis;
use crate::moduletransforms::utilities as module_transform_utilities;
use crate::utilities::move_range_past_modifiers;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClassFieldTransformConfig {
    pub should_transform_initializers_using_set: bool,
    pub should_transform_initializers_using_define: bool,
    pub should_transform_initializers: bool,
    pub should_transform_private_elements_or_class_static_blocks: bool,
    pub should_transform_auto_accessors: bool,
    pub should_transform_this_in_static_initializers: bool,
    pub should_transform_super_in_static_initializers: bool,
    pub legacy_decorators: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClassFacts {
    pub class_was_decorated: bool,
    pub needs_class_constructor_reference: bool,
    pub needs_class_super_reference: bool,
    pub needs_substitution_for_this_in_class_static_field: bool,
    pub will_hoist_initializers_to_constructor: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrivateIdentifierKind {
    Field,
    Method,
    Accessor,
    AutoAccessor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassFieldsAction {
    Keep,
    VisitChildren,
    SkipTransformer,
    VisitSourceFile,
    TransformClassDeclaration,
    TransformClassExpression,
    VisitClassElementOnly,
    VisitNamedEvaluationSite,
    VisitPrivateIdentifier,
    VisitPrivateAccess,
    VisitUpdateExpression,
    VisitAssignmentExpression,
    VisitDiscardableExpression,
    VisitCallLike,
    VisitForStatement,
    EnterIterationStatement,
    VisitThisExpression,
    EnterFunctionBoundary,
    EnterClassElementBoundary,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClassFieldsFacts {
    pub subtree_contains_class_fields_or_private_identifiers: bool,
    pub node_has_private_static_transform_flag: bool,
    pub is_declaration_file: bool,
    pub current_class_is_expression_in_iteration: bool,
    pub current_class_has_non_static_computed_property: bool,
    pub current_class_element_is_static: bool,
    pub current_class_element_is_auto_accessor: bool,
    pub current_class_element_is_private: bool,
    pub current_class_element_is_field: bool,
    pub current_class_element_has_initializer: bool,
    pub inside_computed_property_name: bool,
    pub expression_contains_lexical_this: bool,
    pub expression_contains_lexical_super: bool,
}

pub fn class_field_transform_config(
    language_version: ScriptTarget,
    use_define_for_class_fields: bool,
    legacy_decorators: bool,
) -> Option<ClassFieldTransformConfig> {
    if language_version >= ScriptTarget::ESNext && use_define_for_class_fields {
        return None;
    }

    let should_transform_initializers_using_set = !use_define_for_class_fields;
    let should_transform_initializers_using_define =
        use_define_for_class_fields && language_version < ScriptTarget::ES2022;
    let should_transform_initializers =
        should_transform_initializers_using_set || should_transform_initializers_using_define;
    let should_transform_private_elements_or_class_static_blocks =
        language_version < ScriptTarget::ES2022;
    let should_transform_auto_accessors = language_version < ScriptTarget::ESNext;
    let should_transform_this_in_static_initializers = language_version < ScriptTarget::ES2022;

    Some(ClassFieldTransformConfig {
        should_transform_initializers_using_set,
        should_transform_initializers_using_define,
        should_transform_initializers,
        should_transform_private_elements_or_class_static_blocks,
        should_transform_auto_accessors,
        should_transform_this_in_static_initializers,
        should_transform_super_in_static_initializers: should_transform_this_in_static_initializers,
        legacy_decorators,
    })
}

pub fn class_fields_action_for_kind(kind: ast::Kind, facts: ClassFieldsFacts) -> ClassFieldsAction {
    if !facts.subtree_contains_class_fields_or_private_identifiers
        && !facts.node_has_private_static_transform_flag
    {
        return ClassFieldsAction::Keep;
    }

    match kind {
        ast::Kind::SourceFile => ClassFieldsAction::VisitSourceFile,
        ast::Kind::ClassDeclaration => ClassFieldsAction::TransformClassDeclaration,
        ast::Kind::ClassExpression => ClassFieldsAction::TransformClassExpression,
        ast::Kind::ClassStaticBlockDeclaration | ast::Kind::PropertyDeclaration => {
            ClassFieldsAction::VisitClassElementOnly
        }
        ast::Kind::PropertyAssignment
        | ast::Kind::VariableDeclaration
        | ast::Kind::Parameter
        | ast::Kind::BindingElement
        | ast::Kind::ExportAssignment => ClassFieldsAction::VisitNamedEvaluationSite,
        ast::Kind::PrivateIdentifier => ClassFieldsAction::VisitPrivateIdentifier,
        ast::Kind::PropertyAccessExpression | ast::Kind::ElementAccessExpression => {
            ClassFieldsAction::VisitPrivateAccess
        }
        ast::Kind::PrefixUnaryExpression | ast::Kind::PostfixUnaryExpression => {
            ClassFieldsAction::VisitUpdateExpression
        }
        ast::Kind::BinaryExpression => ClassFieldsAction::VisitAssignmentExpression,
        ast::Kind::ParenthesizedExpression | ast::Kind::ExpressionStatement => {
            ClassFieldsAction::VisitDiscardableExpression
        }
        ast::Kind::CallExpression | ast::Kind::TaggedTemplateExpression => {
            ClassFieldsAction::VisitCallLike
        }
        ast::Kind::ForStatement => ClassFieldsAction::VisitForStatement,
        ast::Kind::ForInStatement
        | ast::Kind::ForOfStatement
        | ast::Kind::DoStatement
        | ast::Kind::WhileStatement => ClassFieldsAction::EnterIterationStatement,
        ast::Kind::ThisKeyword => ClassFieldsAction::VisitThisExpression,
        ast::Kind::FunctionDeclaration | ast::Kind::FunctionExpression => {
            ClassFieldsAction::EnterFunctionBoundary
        }
        ast::Kind::Constructor
        | ast::Kind::MethodDeclaration
        | ast::Kind::GetAccessor
        | ast::Kind::SetAccessor => ClassFieldsAction::EnterClassElementBoundary,
        _ => ClassFieldsAction::VisitChildren,
    }
}

pub fn requires_block_scoped_var(facts: ClassFieldsFacts) -> bool {
    facts.current_class_is_expression_in_iteration
}

pub fn class_expression_needs_block_scoped_temp(facts: ClassFieldsFacts) -> bool {
    requires_block_scoped_var(facts) && facts.current_class_has_non_static_computed_property
}

pub fn should_transform_auto_accessors_in_current_class(
    config: ClassFieldTransformConfig,
    facts: ClassFieldsFacts,
) -> bool {
    config.should_transform_auto_accessors || facts.node_has_private_static_transform_flag
}

pub fn should_transform_class_element_to_weak_map(
    config: ClassFieldTransformConfig,
    facts: ClassFieldsFacts,
) -> bool {
    facts.current_class_element_is_private
        && (config.should_transform_private_elements_or_class_static_blocks
            || facts.node_has_private_static_transform_flag)
}

pub fn should_substitute_this_in_static_initializer(
    config: ClassFieldTransformConfig,
    facts: ClassFieldsFacts,
) -> bool {
    config.should_transform_this_in_static_initializers
        && facts.current_class_element_is_static
        && facts.expression_contains_lexical_this
}

pub fn should_substitute_super_in_static_initializer(
    config: ClassFieldTransformConfig,
    facts: ClassFieldsFacts,
) -> bool {
    config.should_transform_super_in_static_initializers
        && facts.current_class_element_is_static
        && facts.expression_contains_lexical_super
}

pub fn field_initializer_should_move_to_constructor(
    config: ClassFieldTransformConfig,
    facts: ClassFieldsFacts,
) -> bool {
    !facts.current_class_element_is_static
        && ((config.should_transform_initializers_using_define
            && facts.current_class_element_is_field)
            || (config.should_transform_initializers_using_set
                && facts.current_class_element_has_initializer)
            || (config.should_transform_private_elements_or_class_static_blocks
                && facts.current_class_element_is_private))
}

fn get_non_assignment_operator_for_compound_assignment(kind: ast::Kind) -> ast::Kind {
    match kind {
        ast::Kind::PlusEqualsToken => ast::Kind::PlusToken,
        ast::Kind::MinusEqualsToken => ast::Kind::MinusToken,
        ast::Kind::AsteriskAsteriskEqualsToken => ast::Kind::AsteriskAsteriskToken,
        ast::Kind::AsteriskEqualsToken => ast::Kind::AsteriskToken,
        ast::Kind::SlashEqualsToken => ast::Kind::SlashToken,
        ast::Kind::PercentEqualsToken => ast::Kind::PercentToken,
        ast::Kind::AmpersandEqualsToken => ast::Kind::AmpersandToken,
        ast::Kind::BarEqualsToken => ast::Kind::BarToken,
        ast::Kind::CaretEqualsToken => ast::Kind::CaretToken,
        ast::Kind::LessThanLessThanEqualsToken => ast::Kind::LessThanLessThanToken,
        ast::Kind::GreaterThanGreaterThanEqualsToken => ast::Kind::GreaterThanGreaterThanToken,
        ast::Kind::GreaterThanGreaterThanGreaterThanEqualsToken => {
            ast::Kind::GreaterThanGreaterThanGreaterThanToken
        }
        ast::Kind::BarBarEqualsToken => ast::Kind::BarBarToken,
        ast::Kind::AmpersandAmpersandEqualsToken => ast::Kind::AmpersandAmpersandToken,
        ast::Kind::QuestionQuestionEqualsToken => ast::Kind::QuestionQuestionToken,
        _ => unreachable!("Invalid compound assignment operator"),
    }
}

pub fn visit_source_file_root(
    source_file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
    config: ClassFieldTransformConfig,
) -> ast::Node {
    let mut runtime = ClassFieldsRuntime {
        source: source_file.store(),
        emit_context,
        import_state: ast::AstImportState::new(),
        config,
        current_class_has_lexical_environment_facts: false,
        current_class_will_hoist_initializers_to_constructor: false,
        current_class_is_legacy_decorated: false,
        current_legacy_decorated_static_initializer: false,
        current_class_container: None,
        in_iteration_statement: false,
        inside_computed_property_name: false,
        current_class_static_block_receiver: None,
        current_class_static_block_preserves_static_auto_accessor_this: false,
        current_class_static_super_context: None,
        previous_class_static_block_receiver: None,
        previous_class_static_super_context: None,
        private_accessor_stack: Vec::new(),
        private_static_field_stack: Vec::new(),
        class_alias_stack: Vec::new(),
        class_alias_shadow_stack: Vec::new(),
        class_expression_assigned_name_stack: Vec::new(),
        exported_variable_declaration_stack: Vec::new(),
        pending_expressions: Vec::new(),
        pending_statements: Vec::new(),
        pending_instance_variable_declarations: Vec::new(),
    };
    let root = runtime.visit_node(Some(root)).unwrap_or(root);
    runtime.emit_context.add_requested_emit_helpers(&root);
    root
}

struct ClassFieldsRuntime<'ctx, 'source> {
    source: &'source ast::AstStore,
    emit_context: &'ctx mut printer::EmitContext,
    import_state: ast::AstImportState,
    config: ClassFieldTransformConfig,
    current_class_has_lexical_environment_facts: bool,
    current_class_will_hoist_initializers_to_constructor: bool,
    current_class_is_legacy_decorated: bool,
    current_legacy_decorated_static_initializer: bool,
    current_class_container: Option<ast::Node>,
    in_iteration_statement: bool,
    inside_computed_property_name: bool,
    current_class_static_block_receiver: Option<ast::Node>,
    current_class_static_block_preserves_static_auto_accessor_this: bool,
    current_class_static_super_context: Option<StaticSuperContext>,
    previous_class_static_block_receiver: Option<ast::Node>,
    previous_class_static_super_context: Option<StaticSuperContext>,
    private_accessor_stack: Vec<PrivateAccessorEnvironment>,
    private_static_field_stack: Vec<PrivateStaticFieldEnvironment>,
    class_alias_stack: Vec<ClassAlias>,
    class_alias_shadow_stack: Vec<HashSet<String>>,
    class_expression_assigned_name_stack: Vec<ClassExpressionAssignedName>,
    exported_variable_declaration_stack: Vec<ast::Node>,
    pending_expressions: Vec<ast::Node>,
    pending_statements: Vec<ast::Node>,
    pending_instance_variable_declarations: Vec<ast::Node>,
}

#[derive(Clone, Copy)]
struct StaticSuperContext {
    class_constructor: ast::Node,
    super_class_reference: ast::Node,
}

#[derive(Clone)]
struct ClassAlias {
    name_text: String,
    alias: ast::Node,
}

#[derive(Clone, Copy)]
struct ClassExpressionAssignedName {
    class_expression: ast::Node,
    assigned_name: ast::Node,
}

#[derive(Clone)]
struct PrivateAccessorEnvironment {
    brand_check_identifier: ast::Node,
    accessors: Vec<PrivateAccessorInfo>,
}

#[derive(Clone)]
struct PrivateAccessorInfo {
    name_text: String,
    method_name: Option<ast::Node>,
    getter_name: Option<ast::Node>,
    setter_name: Option<ast::Node>,
    is_valid: bool,
    is_static: bool,
}

#[derive(Clone)]
struct PrivateStaticFieldEnvironment {
    brand_check_identifier: ast::Node,
    fields: Vec<PrivateStaticFieldInfo>,
}

#[derive(Clone)]
struct PrivateStaticFieldInfo {
    name_text: String,
    generated_name_source: Option<ast::Node>,
    storage_name: ast::Node,
    is_valid: bool,
    is_active: bool,
    is_static: bool,
    is_auto_accessor_storage: bool,
    order: usize,
}

#[derive(Clone, Copy)]
struct PrivateNameState {
    kind: PrivateNameStateKind,
    is_static: bool,
    has_getter: bool,
    has_setter: bool,
    is_valid: bool,
    order: usize,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum PrivateNameStateKind {
    Field,
    Method,
    Accessor,
}

impl ClassFieldsRuntime<'_, '_> {
    fn factory(&self) -> &ast::NodeFactory {
        &self.emit_context.factory.node_factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.emit_context.factory.node_factory
    }

    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        self.emit_context.store_for_node(node)
    }

    fn finish_class_element(&mut self, updated: ast::Node, original: ast::Node) -> ast::Node {
        let original_loc = self.store_for(original).loc(original);
        let source_map_range = {
            let existing = self.emit_context.source_map_range(&original);
            if existing != original_loc {
                existing
            } else {
                let source = self.store_for(original);
                crate::utilities::move_range_past_decorators(source, original)
            }
        };
        super::esdecorator::finish_class_element_with_source_map_range(
            updated,
            original,
            source_map_range,
            self.emit_context,
        )
    }

    fn is_derived_class(&self, class_node: ast::Node) -> bool {
        let source = self.store_for(class_node);
        let Some(extends_clause_element) =
            ast::get_class_extends_heritage_element(source, class_node)
        else {
            return false;
        };
        let expression = source
            .expression(extends_clause_element)
            .expect("extends heritage clause element should have an expression");
        let expression =
            ast::skip_outer_expressions(source, expression, ast::OuterExpressionKinds::ALL);
        source.kind(expression) != ast::Kind::NullKeyword
    }

    fn update_computed_property_name(
        &mut self,
        name: ast::Node,
        expression: ast::Node,
    ) -> ast::Node {
        let expression = self.preserve_node(expression);
        if name.store_id() == self.factory().store().store_id() {
            self.factory_mut()
                .update_computed_property_name(name, Some(expression))
        } else {
            assert_eq!(
                name.store_id(),
                self.source.store_id(),
                "computed property name cannot read unrelated AST store"
            );
            let source = self.source;
            self.factory_mut().update_computed_property_name_from_store(
                source,
                name,
                Some(expression),
            )
        }
    }

    fn get_property_name_for_transform_property(
        &mut self,
        property: ast::Node,
        name: ast::Node,
    ) -> Option<ast::Node> {
        // We generate a name here in order to reuse the value cached by the relocated computed name expression (which uses the same generated name)
        if ast::has_accessor_modifier(self.store_for(property), property) {
            let options = AutoGenerateOptions {
                suffix: "_accessor_storage",
                ..Default::default()
            };
            let name = self.emit_context.most_original(&name);
            return Some(
                self.emit_context
                    .new_generated_private_name_for_node_ex(name, options),
            );
        }

        let source = self.store_for(name);
        if source.kind(name) == ast::Kind::ComputedPropertyName {
            let expression = source.expression(name)?;
            let expression_source = self.store_for(expression);
            if !module_transform_utilities::is_simple_inlineable_expression(
                expression_source.kind(expression),
                ast::is_identifier(expression_source, expression),
            ) {
                let generated_name = if name.store_id() == self.factory().store().store_id() {
                    let original_name = self.emit_context.most_original(&name);
                    self.emit_context.new_generated_name_for_node(original_name)
                } else {
                    assert_eq!(
                        name.store_id(),
                        self.source.store_id(),
                        "property name cannot read unrelated AST store"
                    );
                    self.emit_context
                        .factory
                        .new_generated_name_for_node(self.source, &name)
                };
                return Some(self.update_computed_property_name(name, generated_name));
            }
        }

        Some(name)
    }

    fn inject_pending_expressions(
        &mut self,
        pending_expressions: &mut Vec<ast::Node>,
        expression: ast::Node,
    ) -> ast::Node {
        if pending_expressions.is_empty() {
            return expression;
        }
        let mut expressions = std::mem::take(pending_expressions);
        if self.store_for(expression).kind(expression) == ast::Kind::ParenthesizedExpression {
            let inner = self
                .store_for(expression)
                .expression(expression)
                .expect("parenthesized expression should have expression");
            expressions.push(self.preserve_node(inner));
            let inlined = self
                .emit_context
                .factory
                .inline_expressions(&expressions)
                .expect("pending expressions should not be empty");
            return if expression.store_id() == self.factory().store().store_id() {
                self.factory_mut()
                    .update_parenthesized_expression(expression, inlined)
            } else {
                let source = self.source;
                self.factory_mut()
                    .update_parenthesized_expression_from_store(source, expression, inlined)
            };
        }
        let expression = self.preserve_node(expression);
        expressions.push(expression);
        self.emit_context
            .factory
            .inline_expressions(&expressions)
            .expect("pending expressions should not be empty")
    }

    fn snapshot_optional_node_list(
        &self,
        list: Option<ast::SourceNodeList<'_>>,
    ) -> Option<(core::TextRange, core::TextRange, Vec<ast::Node>, bool)> {
        list.map(|list| {
            (
                list.loc(),
                list.range(),
                list.iter().collect::<Vec<_>>(),
                list.has_trailing_comma(),
            )
        })
    }

    fn snapshot_optional_modifier_list(
        &self,
        list: Option<ast::SourceModifierList<'_>>,
    ) -> Option<(
        core::TextRange,
        core::TextRange,
        Vec<ast::Node>,
        ast::ModifierFlags,
    )> {
        list.map(|list| {
            let nodes = list.nodes();
            (
                nodes.loc(),
                nodes.range(),
                nodes.iter().collect::<Vec<_>>(),
                list.modifier_flags(),
            )
        })
    }

    fn preserve_optional_node_list_snapshot(
        &mut self,
        snapshot: Option<(core::TextRange, core::TextRange, Vec<ast::Node>, bool)>,
    ) -> Option<ast::NodeList> {
        snapshot.map(|(loc, range, nodes, has_trailing_comma)| {
            let nodes = nodes
                .into_iter()
                .map(|node| self.preserve_node(node))
                .collect::<Vec<_>>();
            self.factory_mut().new_node_list_with_trailing_comma(
                loc,
                range,
                nodes,
                has_trailing_comma,
            )
        })
    }

    fn preserve_optional_modifier_list_snapshot(
        &mut self,
        snapshot: Option<(
            core::TextRange,
            core::TextRange,
            Vec<ast::Node>,
            ast::ModifierFlags,
        )>,
    ) -> Option<ast::ModifierList> {
        snapshot.map(|(loc, range, nodes, modifier_flags)| {
            let nodes = nodes
                .into_iter()
                .map(|node| self.preserve_node(node))
                .collect::<Vec<_>>();
            self.factory_mut()
                .new_modifier_list(loc, range, nodes, modifier_flags)
        })
    }

    fn preserve_optional_modifier_list_snapshot_with_allowed(
        &mut self,
        snapshot: Option<(
            core::TextRange,
            core::TextRange,
            Vec<ast::Node>,
            ast::ModifierFlags,
        )>,
        allowed: ast::ModifierFlags,
    ) -> Option<ast::ModifierList> {
        snapshot.map(|(loc, range, nodes, modifier_flags)| {
            let mut preserved = Vec::new();
            for node in nodes {
                if crate::modifiervisitor::modifier_is_allowed(
                    self.store_for(node).kind(node),
                    allowed,
                ) {
                    preserved.push(self.preserve_node(node));
                }
            }
            self.factory_mut()
                .new_modifier_list(loc, range, preserved, modifier_flags & allowed)
        })
    }

    fn preserve_optional_modifier_list_snapshot_without_accessor(
        &mut self,
        snapshot: Option<(
            core::TextRange,
            core::TextRange,
            Vec<ast::Node>,
            ast::ModifierFlags,
        )>,
    ) -> Option<ast::ModifierList> {
        if !self.config.should_transform_auto_accessors
            && !self.current_class_will_hoist_initializers_to_constructor
        {
            return self.preserve_optional_modifier_list_snapshot(snapshot);
        }
        snapshot.map(|(loc, range, nodes, modifier_flags)| {
            let mut preserved = Vec::new();
            for node in nodes {
                if self.store_for(node).kind(node) != ast::Kind::AccessorKeyword {
                    preserved.push(self.preserve_node(node));
                }
            }
            self.factory_mut().new_modifier_list(
                loc,
                range,
                preserved,
                modifier_flags & !ast::ModifierFlags::ACCESSOR,
            )
        })
    }

    fn visit(&mut self, node: &ast::Node) -> Option<ast::Node> {
        let kind = self.store_for(*node).kind(*node);
        if kind == ast::Kind::ThisKeyword {
            if let Some(receiver) = self.current_class_static_block_receiver {
                return Some(receiver);
            }
            if self.current_legacy_decorated_static_initializer {
                return Some(self.decorated_static_invalid_this());
            }
            return Some(*node);
        }
        if kind == ast::Kind::ComputedPropertyName {
            return Some(self.visit_computed_property_name(*node));
        }
        if kind == ast::Kind::Block {
            return Some(self.visit_block(*node));
        }
        if kind == ast::Kind::SyntaxList {
            return Some(self.visit_syntax_list(*node));
        }
        if kind == ast::Kind::ExportDeclaration {
            return Some(*node);
        }
        if kind == ast::Kind::Identifier
            && let Some(alias) = self.try_substitute_class_alias(*node)
        {
            return Some(alias);
        }
        if self.current_class_static_block_receiver.is_none()
            && !(matches!(
                kind,
                ast::Kind::ClassDeclaration | ast::Kind::ClassExpression
            ) && self.should_transform_class_members())
            && !self.contains_class_fields_or_lexical_this_or_super(*node)
        {
            if !self.pending_expressions.is_empty() {
                return Some(self.generated_visit_each_child(node));
            }
            if !self.class_alias_stack.is_empty() {
                return Some(self.generated_visit_each_child(node));
            }
            return Some(*node);
        }
        match kind {
            ast::Kind::VariableStatement => {
                return Some(self.visit_variable_statement(*node));
            }
            ast::Kind::VariableDeclaration => {
                return Some(self.visit_variable_declaration(*node));
            }
            ast::Kind::Parameter => {
                return Some(self.visit_parameter_declaration(*node));
            }
            ast::Kind::PropertyAssignment => {
                return Some(self.visit_property_assignment(*node));
            }
            ast::Kind::BindingElement => {
                return Some(self.visit_binding_element(*node));
            }
            ast::Kind::ExportAssignment => {
                return Some(self.visit_export_assignment(*node));
            }
            ast::Kind::ExpressionStatement => {
                return Some(self.visit_expression_statement(*node));
            }
            ast::Kind::ClassDeclaration if self.should_transform_class_members() => {
                return Some(self.transform_class_declaration(*node));
            }
            ast::Kind::ClassExpression if self.should_transform_class_members() => {
                return Some(self.transform_class_expression(*node));
            }
            ast::Kind::FunctionDeclaration
            | ast::Kind::FunctionExpression
            | ast::Kind::MethodDeclaration
            | ast::Kind::GetAccessor
            | ast::Kind::SetAccessor
            | ast::Kind::Constructor => {
                let saved_class_static_block_receiver = self.current_class_static_block_receiver;
                let saved_class_static_super_context = self.current_class_static_super_context;
                let saved_legacy_decorated_static_initializer =
                    self.current_legacy_decorated_static_initializer;
                self.current_class_static_block_receiver = None;
                self.current_class_static_super_context = None;
                self.current_legacy_decorated_static_initializer = false;
                let visited = self.generated_visit_each_child(node);
                self.current_class_static_block_receiver = saved_class_static_block_receiver;
                self.current_class_static_super_context = saved_class_static_super_context;
                self.current_legacy_decorated_static_initializer =
                    saved_legacy_decorated_static_initializer;
                return Some(visited);
            }
            ast::Kind::BinaryExpression => {
                if let Some(node) = self.transform_destructuring_assignment_expression(*node) {
                    return Some(node);
                }
                if let Some(node) =
                    self.transform_super_property_assignment_in_static_initializer(*node)
                {
                    return Some(node);
                }
                if let Some(node) = self.transform_private_identifier_binary_expression(*node) {
                    return Some(node);
                }
            }
            ast::Kind::PrefixUnaryExpression | ast::Kind::PostfixUnaryExpression => {
                if let Some(node) =
                    self.transform_private_identifier_update_expression(*node, false)
                {
                    return Some(node);
                }
                if let Some(node) =
                    self.transform_super_property_update_in_static_initializer(*node, false)
                {
                    return Some(node);
                }
            }
            ast::Kind::CallExpression => {
                if let Some(node) = self.transform_private_identifier_call_expression(*node) {
                    return Some(node);
                }
                if let Some(node) = self.transform_super_property_call_in_static_initializer(*node)
                {
                    return Some(node);
                }
            }
            ast::Kind::TaggedTemplateExpression => {
                if let Some(node) =
                    self.transform_private_identifier_tagged_template_expression(*node)
                {
                    return Some(node);
                }
                if let Some(node) =
                    self.transform_super_property_tagged_template_in_static_initializer(*node)
                {
                    return Some(node);
                }
            }
            ast::Kind::ForStatement => {
                return Some(self.visit_for_statement(*node));
            }
            ast::Kind::ForInStatement
            | ast::Kind::ForOfStatement
            | ast::Kind::DoStatement
            | ast::Kind::WhileStatement => {
                return Some(self.set_in_iteration_statement_and(true, *node));
            }
            ast::Kind::PrivateIdentifier => {
                return Some(self.visit_private_identifier(*node));
            }
            ast::Kind::PropertyAccessExpression => {
                if let Some(node) = self.transform_private_identifier_property_access(*node) {
                    return Some(node);
                }
                if let Some(node) =
                    self.transform_super_property_access_in_static_initializer(*node)
                {
                    return Some(node);
                }
            }
            ast::Kind::ElementAccessExpression => {
                if let Some(node) =
                    self.transform_super_property_access_in_static_initializer(*node)
                {
                    return Some(node);
                }
            }
            _ => {}
        }
        let visited = self.generated_visit_each_child(node);
        match self.store_for(visited).kind(visited) {
            ast::Kind::ClassDeclaration if self.should_transform_class_members() => {
                Some(self.transform_class_declaration(visited))
            }
            ast::Kind::ClassExpression if self.should_transform_class_members() => {
                Some(self.transform_class_expression(visited))
            }
            _ => Some(visited),
        }
    }

    fn set_in_iteration_statement_and(&mut self, in_iteration: bool, node: ast::Node) -> ast::Node {
        if self.in_iteration_statement == in_iteration {
            return self.generated_visit_each_child(&node);
        }
        let saved = self.in_iteration_statement;
        self.in_iteration_statement = in_iteration;
        let result = self.generated_visit_each_child(&node);
        self.in_iteration_statement = saved;
        result
    }

    fn visit_for_statement(&mut self, node: ast::Node) -> ast::Node {
        let (initializer, condition, incrementor, statement) = {
            let source = self.store_for(node);
            (
                source.initializer(node),
                source.condition(node),
                source.incrementor(node),
                source.statement(node),
            )
        };
        let initializer = self.visit_node(initializer);
        let condition = self.visit_node(condition);
        let incrementor = incrementor.and_then(|incrementor| {
            match self.store_for(incrementor).kind(incrementor) {
                ast::Kind::PrefixUnaryExpression | ast::Kind::PostfixUnaryExpression => self
                    .transform_private_identifier_update_expression(incrementor, true)
                    .or_else(|| {
                        self.transform_super_property_update_in_static_initializer(
                            incrementor,
                            true,
                        )
                    })
                    .or_else(|| self.visit_node(Some(incrementor))),
                ast::Kind::BinaryExpression => self
                    .transform_super_property_assignment_in_static_initializer_with_discard(
                        incrementor,
                        true,
                    )
                    .or_else(|| self.visit_node(Some(incrementor))),
                _ => self.visit_node(Some(incrementor)),
            }
        });
        let saved = self.in_iteration_statement;
        self.in_iteration_statement = true;
        let statement = self.visit_iteration_body(statement);
        self.in_iteration_statement = saved;

        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_for_statement(
                node,
                initializer,
                condition,
                incrementor,
                statement,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_for_statement_from_store(
                source,
                node,
                initializer,
                condition,
                incrementor,
                statement,
            )
        }
    }

    fn should_transform_class_members(&self) -> bool {
        self.config.should_transform_initializers
            || self.config.should_transform_auto_accessors
            || self
                .config
                .should_transform_private_elements_or_class_static_blocks
    }

    fn visit_variable_statement(&mut self, node: ast::Node) -> ast::Node {
        let saved_pending_statements = std::mem::take(&mut self.pending_statements);
        let exported_declarations = {
            let source = self.store_for(node);
            if ast::get_combined_modifier_flags(source, node).intersects(ast::ModifierFlags::EXPORT)
            {
                source
                    .declaration_list(node)
                    .and_then(|declaration_list| source.declarations(declaration_list))
                    .map(|declarations| declarations.iter().collect::<Vec<_>>())
                    .unwrap_or_default()
            } else {
                Vec::new()
            }
        };
        let exported_variable_declaration_stack_len =
            self.exported_variable_declaration_stack.len();
        self.exported_variable_declaration_stack
            .extend(exported_declarations);
        let visited = self.generated_visit_each_child(&node);
        self.exported_variable_declaration_stack
            .truncate(exported_variable_declaration_stack_len);
        let pending_statements =
            std::mem::replace(&mut self.pending_statements, saved_pending_statements);
        if pending_statements.is_empty() {
            return visited;
        }

        let mut statements = Vec::with_capacity(pending_statements.len() + 1);
        statements.push(visited);
        statements.extend(pending_statements);
        self.factory_mut().new_syntax_list(statements)
    }

    fn visit_syntax_list(&mut self, node: ast::Node) -> ast::Node {
        let children = {
            let source = self.store_for(node);
            source
                .syntax_list_children(node)
                .expect("SyntaxList should have children")
                .iter()
                .flatten()
                .collect::<Vec<_>>()
        };
        let mut changed = false;
        let mut nodes = Vec::with_capacity(children.len());
        for child in children {
            let visited = self.visit(&child);
            self.append_visited_node(child, visited, &mut nodes, &mut changed);
            if !self.pending_statements.is_empty() {
                changed = true;
                nodes.append(&mut self.pending_statements);
            }
        }
        if !changed {
            return node;
        }
        self.factory_mut().new_syntax_list(nodes)
    }

    fn contains_class_fields_or_lexical_this_or_super(&self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        if source.subtree_facts(node).intersects(
            ast::SubtreeFacts::CONTAINS_CLASS_FIELDS
                | ast::SubtreeFacts::CONTAINS_LEXICAL_THIS_OR_SUPER,
        ) {
            return true;
        }
        if matches!(
            source.kind(node),
            ast::Kind::PropertyDeclaration
                | ast::Kind::ClassStaticBlockDeclaration
                | ast::Kind::PrivateIdentifier
        ) {
            return true;
        }
        if ast::is_class_like(source, node)
            && source.source_members(node).is_some_and(|members| {
                members
                    .iter()
                    .any(|member| self.contains_class_fields_or_lexical_this_or_super(member))
            })
        {
            return true;
        }
        let mut found = false;
        let _ = source.for_each_present_child(node, |child| {
            if self.contains_class_fields_or_lexical_this_or_super(child) {
                found = true;
                std::ops::ControlFlow::Break(())
            } else {
                std::ops::ControlFlow::Continue(())
            }
        });
        found
    }

    fn visit_private_identifier(&mut self, node: ast::Node) -> ast::Node {
        if !self
            .config
            .should_transform_private_elements_or_class_static_blocks
        {
            return node;
        }
        let source = self.store_for(node);
        if source
            .parent(node)
            .is_some_and(|parent| ast::is_statement(self.store_for(parent), parent))
        {
            return node;
        }
        let result = self.factory_mut().new_identifier("");
        self.emit_context.set_original(&result, &node);
        result
    }

    fn visit_computed_property_name(&mut self, node: ast::Node) -> ast::Node {
        let expression = {
            let source = self.store_for(node);
            let Some(expression) = source.expression(node) else {
                return node;
            };
            expression
        };
        let saved_class_static_block_receiver = self.current_class_static_block_receiver;
        let saved_class_static_super_context = self.current_class_static_super_context;
        let saved_inside_computed_property_name = self.inside_computed_property_name;
        self.current_class_static_block_receiver = self.previous_class_static_block_receiver;
        self.current_class_static_super_context = self.previous_class_static_super_context;
        self.inside_computed_property_name = true;
        let expression = self
            .visit_node(Some(expression))
            .unwrap_or_else(|| self.preserve_node(expression));
        self.current_class_static_block_receiver = saved_class_static_block_receiver;
        self.current_class_static_super_context = saved_class_static_super_context;
        self.inside_computed_property_name = saved_inside_computed_property_name;

        let mut pending_expressions = std::mem::take(&mut self.pending_expressions);
        let expression = self.inject_pending_expressions(&mut pending_expressions, expression);
        self.pending_expressions = pending_expressions;
        self.update_computed_property_name(node, expression)
    }

    fn try_substitute_class_alias(&mut self, node: ast::Node) -> Option<ast::Node> {
        if self.inside_computed_property_name {
            return None;
        }
        let source = self.store_for(node);
        let text = source.text(node);
        if self
            .class_alias_shadow_stack
            .iter()
            .rev()
            .any(|shadows| shadows.contains(&text))
        {
            return None;
        }
        let alias = self
            .class_alias_stack
            .iter()
            .rev()
            .find(|alias| alias.name_text == text)?
            .alias;

        if let Some(parent) = source.parent(node) {
            let parent_source = self.store_for(parent);
            if parent_source.kind(parent) == ast::Kind::ComputedPropertyName {
                return None;
            }
            if parent_source.kind(parent) == ast::Kind::PropertyAccessExpression
                && parent_source.name(parent) == Some(node)
            {
                return None;
            }
            if parent_source.kind(parent) != ast::Kind::PropertyAccessExpression
                && parent_source.name(parent) == Some(node)
            {
                return None;
            }
        }

        let loc = source.loc(node);
        let result = self.clone_node_for_reuse(alias);
        self.emit_context.set_source_map_range(&result, loc);
        self.emit_context.set_comment_range(&result, loc);
        Some(result)
    }

    fn visit_block(&mut self, node: ast::Node) -> ast::Node {
        let shadows = self.collect_class_alias_shadows(node);
        if shadows.is_empty() {
            return self.generated_visit_each_child(&node);
        }

        self.class_alias_shadow_stack.push(shadows);
        let result = self.generated_visit_each_child(&node);
        self.class_alias_shadow_stack.pop();
        result
    }

    fn collect_class_alias_shadows(&self, node: ast::Node) -> HashSet<String> {
        let alias_names = self
            .class_alias_stack
            .iter()
            .map(|alias| alias.name_text.as_str())
            .collect::<HashSet<_>>();
        if alias_names.is_empty() {
            return HashSet::new();
        }

        let mut shadows = HashSet::new();
        self.collect_class_alias_shadows_worker(node, &alias_names, &mut shadows);
        shadows
    }

    fn collect_class_alias_shadows_worker(
        &self,
        node: ast::Node,
        alias_names: &HashSet<&str>,
        shadows: &mut HashSet<String>,
    ) {
        let source = self.store_for(node);
        if ast::is_declaration(source, node)
            && let Some(name) = source.name(node)
            && ast::is_identifier(self.store_for(name), name)
        {
            let text = self.store_for(name).text(name);
            if alias_names.contains(text.as_str()) {
                shadows.insert(text);
            }
        }

        let _ = source.for_each_present_child(node, |child| {
            self.collect_class_alias_shadows_worker(child, alias_names, shadows);
            std::ops::ControlFlow::Continue(())
        });
    }

    fn create_class_references_if_needed(
        &mut self,
        node: ast::Node,
    ) -> (Option<ast::Node>, Option<ast::Node>, Option<ast::Node>) {
        let needs_class_constructor_reference =
            self.class_declaration_needs_class_constructor_reference(node);
        let needs_class_super_reference = self.class_declaration_needs_class_super_reference(node);
        if !needs_class_constructor_reference && !needs_class_super_reference {
            return (None, None, None);
        }
        let source = self.store_for(node);
        let Some(name) = source.name(node) else {
            return (None, None, None);
        };
        let temp = self
            .emit_context
            .factory
            .new_temp_variable_ex(AutoGenerateOptions {
                flags: GeneratedIdentifierFlags::RESERVED_IN_NESTED_SCOPES,
                ..Default::default()
            });
        self.emit_context.add_variable_declaration(temp);
        let class_name = self.preserve_node(name);
        let assignment = self
            .emit_context
            .factory
            .new_assignment_expression(temp, class_name);
        let super_reference = if needs_class_super_reference {
            let temp = self
                .emit_context
                .factory
                .new_temp_variable_ex(AutoGenerateOptions {
                    flags: GeneratedIdentifierFlags::RESERVED_IN_NESTED_SCOPES,
                    ..Default::default()
                });
            self.emit_context.add_variable_declaration(temp);
            Some(temp)
        } else {
            None
        };
        (Some(temp), Some(assignment), super_reference)
    }

    fn class_was_decorated(&self, node: ast::Node) -> bool {
        let original = self.emit_context.most_original(&node);
        let original_source = self.store_for(original);
        ast::is_class_like(original_source, original)
            && ast::class_or_constructor_parameter_is_decorated(
                original_source,
                self.config.legacy_decorators,
                original,
            )
    }

    fn member_is_in_legacy_decorated_class(&self, member: ast::Node) -> bool {
        let _ = member;
        self.current_class_is_legacy_decorated
    }

    fn decorated_static_invalid_this(&mut self) -> ast::Node {
        let void_zero = self.emit_context.factory.new_void_zero_expression();
        self.factory_mut().new_parenthesized_expression(void_zero)
    }

    fn class_has_lexical_environment_facts(&self, node: ast::Node, members: &[ast::Node]) -> bool {
        self.class_was_decorated(node)
            || self.class_declaration_needs_class_constructor_reference(node)
            || self.class_declaration_needs_class_super_reference(node)
            || self.class_will_hoist_initializers_to_constructor(members)
    }

    fn class_declaration_needs_class_constructor_reference(&self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        let Some(members) = source.members(node) else {
            return false;
        };
        members.iter().any(|member| {
            let source = self.store_for(member);
            let is_static = ast::is_class_static_block_declaration(source, member)
                || ast::has_static_modifier(source, member);
            if !is_static {
                if self
                    .config
                    .should_transform_private_elements_or_class_static_blocks
                    && ast::is_private_identifier_class_element_declaration(source, member)
                    && self.member_contains_constructor_reference(member, node)
                {
                    return true;
                }
                return false;
            }
            if source.name(member).is_some()
                && (ast::is_auto_accessor_property_declaration(source, member)
                    || source
                        .name(member)
                        .is_some_and(|name| ast::is_private_identifier(self.store_for(name), name)))
                && self
                    .config
                    .should_transform_private_elements_or_class_static_blocks
            {
                return true;
            }
            if matches!(
                source.kind(member),
                ast::Kind::PropertyDeclaration | ast::Kind::ClassStaticBlockDeclaration
            ) {
                let class_was_decorated = self.class_was_decorated(node);
                let facts = source.subtree_facts(member);
                if self.config.should_transform_this_in_static_initializers
                    && (facts.intersects(ast::SubtreeFacts::CONTAINS_LEXICAL_THIS)
                        || source.initializer(member).is_some_and(|initializer| {
                            self.contains_computed_property_name_with_lexical_this_or_super(
                                initializer,
                                ast::SubtreeFacts::CONTAINS_LEXICAL_THIS,
                            )
                        }))
                    && !class_was_decorated
                {
                    return true;
                }
                if self.config.should_transform_super_in_static_initializers
                    && (facts.intersects(ast::SubtreeFacts::CONTAINS_LEXICAL_SUPER)
                        || source.initializer(member).is_some_and(|initializer| {
                            self.contains_computed_property_name_with_lexical_this_or_super(
                                initializer,
                                ast::SubtreeFacts::CONTAINS_LEXICAL_SUPER,
                            )
                        }))
                    && !class_was_decorated
                {
                    return true;
                }
            }
            false
        })
    }

    fn member_contains_constructor_reference(
        &self,
        member: ast::Node,
        class_node: ast::Node,
    ) -> bool {
        let class_source = self.store_for(class_node);
        let Some(class_name) = ast::get_name_of_declaration(class_source, Some(class_node)) else {
            return false;
        };
        let class_name_text = self.store_for(class_name).text(class_name);

        if let Some(body) = self.store_for(member).body(member)
            && self.node_contains_constructor_reference(body, &class_name_text)
        {
            return true;
        }
        if ast::is_property_declaration(self.store_for(member), member)
            && let Some(initializer) = self.store_for(member).initializer(member)
            && self.node_contains_constructor_reference(initializer, &class_name_text)
        {
            return true;
        }
        false
    }

    fn class_contains_constructor_reference(
        &self,
        class_node: ast::Node,
        members: &[ast::Node],
    ) -> bool {
        members
            .iter()
            .copied()
            .any(|member| self.member_contains_constructor_reference(member, class_node))
    }

    fn node_contains_constructor_reference(&self, node: ast::Node, class_name_text: &str) -> bool {
        let source = self.store_for(node);
        if ast::is_identifier(source, node) && source.text(node) == class_name_text {
            return true;
        }
        if ast::is_property_access_expression(source, node) {
            return source.expression(node).is_some_and(|expression| {
                self.node_contains_constructor_reference(expression, class_name_text)
            });
        }
        let mut found = false;
        let _ = source.for_each_present_child(node, |child| {
            if self.node_contains_constructor_reference(child, class_name_text) {
                found = true;
                std::ops::ControlFlow::Break(())
            } else {
                std::ops::ControlFlow::Continue(())
            }
        });
        found
    }

    fn contains_computed_property_name_with_lexical_this_or_super(
        &self,
        node: ast::Node,
        fact: ast::SubtreeFacts,
    ) -> bool {
        let source = self.store_for(node);
        if source.kind(node) == ast::Kind::ComputedPropertyName
            && source.expression(node).is_some_and(|expression| {
                self.store_for(expression)
                    .subtree_facts(expression)
                    .intersects(fact)
            })
        {
            return true;
        }
        let mut found = false;
        let _ = source.for_each_present_child(node, |child| {
            if self.contains_computed_property_name_with_lexical_this_or_super(child, fact) {
                found = true;
                std::ops::ControlFlow::Break(())
            } else {
                std::ops::ControlFlow::Continue(())
            }
        });
        found
    }

    fn class_declaration_needs_class_super_reference(&self, node: ast::Node) -> bool {
        if !self.config.should_transform_super_in_static_initializers {
            return false;
        }
        if self.class_was_decorated(node) {
            return false;
        }
        if !self.is_derived_class(node) {
            return false;
        }
        let source = self.store_for(node);
        let Some(members) = source.members(node) else {
            return false;
        };
        members.iter().any(|member| {
            let source = self.store_for(member);
            (ast::is_class_static_block_declaration(source, member)
                || ast::has_static_modifier(source, member))
                && matches!(
                    source.kind(member),
                    ast::Kind::PropertyDeclaration | ast::Kind::ClassStaticBlockDeclaration
                )
                && (source
                    .subtree_facts(member)
                    .intersects(ast::SubtreeFacts::CONTAINS_LEXICAL_SUPER)
                    || source.initializer(member).is_some_and(|initializer| {
                        self.contains_computed_property_name_with_lexical_this_or_super(
                            initializer,
                            ast::SubtreeFacts::CONTAINS_LEXICAL_SUPER,
                        )
                    }))
        })
    }

    fn preserve_or_transform_heritage_clauses(
        &mut self,
        snapshot: Option<(core::TextRange, core::TextRange, Vec<ast::Node>, bool)>,
        super_class_reference: Option<ast::Node>,
    ) -> Option<ast::NodeList> {
        let Some((loc, range, nodes, has_trailing_comma)) = snapshot else {
            return None;
        };
        let nodes = nodes
            .into_iter()
            .map(|clause| {
                if self.store_for(clause).kind(clause) == ast::Kind::HeritageClause {
                    let token = self.store_for(clause).token(clause);
                    if let Some(types) = self.store_for(clause).source_types(clause) {
                        let types_loc = types.loc();
                        let types_range = types.range();
                        let has_trailing_comma = types.has_trailing_comma();
                        let type_nodes = types.iter().collect::<Vec<_>>();
                        let mut updated_types = Vec::with_capacity(type_nodes.len());
                        for (index, type_node) in type_nodes.into_iter().enumerate() {
                            if self.store_for(type_node).kind(type_node)
                                == ast::Kind::ExpressionWithTypeArguments
                                && let Some(expression) =
                                    self.store_for(type_node).expression(type_node)
                            {
                                if index == 0
                                    && token == Some(ast::Kind::ExtendsKeyword)
                                    && let Some(super_class_reference) = super_class_reference
                                {
                                    let visited_expression =
                                        self.visit_node(Some(expression)).unwrap_or(expression);
                                    let expression =
                                        self.emit_context.factory.new_assignment_expression(
                                            super_class_reference,
                                            visited_expression,
                                        );
                                    let updated = if type_node.store_id()
                                        == self.factory().store().store_id()
                                    {
                                        self.factory_mut().update_expression_with_type_arguments(
                                            type_node,
                                            expression,
                                            None::<ast::NodeList>,
                                        )
                                    } else {
                                        let source = self.source;
                                        self.factory_mut()
                                            .update_expression_with_type_arguments_from_store(
                                                source,
                                                type_node,
                                                expression,
                                                None::<ast::NodeList>,
                                            )
                                    };
                                    updated_types.push(updated);
                                } else {
                                    let visited = self.generated_visit_each_child(&type_node);
                                    updated_types.push(self.preserve_node(visited));
                                }
                                continue;
                            }
                            updated_types.push(self.preserve_node(type_node));
                        }
                        let types = self.factory_mut().new_node_list_with_trailing_comma(
                            types_loc,
                            types_range,
                            updated_types,
                            has_trailing_comma,
                        );
                        let updated = if clause.store_id() == self.factory().store().store_id() {
                            self.factory_mut().update_heritage_clause(
                                clause,
                                token.unwrap_or(ast::Kind::ExtendsKeyword),
                                types,
                            )
                        } else {
                            let source = self.source;
                            self.factory_mut().update_heritage_clause_from_store(
                                source,
                                clause,
                                token.unwrap_or(ast::Kind::ExtendsKeyword),
                                types,
                            )
                        };
                        return updated;
                    }
                }
                self.preserve_node(clause)
            })
            .collect::<Vec<_>>();
        Some(self.factory_mut().new_node_list_with_trailing_comma(
            loc,
            range,
            nodes,
            has_trailing_comma,
        ))
    }

    fn transform_class_declaration(&mut self, node: ast::Node) -> ast::Node {
        let saved_pending_expressions = std::mem::take(&mut self.pending_expressions);
        let saved_current_class_container = self.current_class_container;
        let saved_class_static_block_receiver = self.current_class_static_block_receiver;
        let saved_class_static_super_context = self.current_class_static_super_context;
        let saved_previous_class_static_block_receiver = self.previous_class_static_block_receiver;
        let saved_previous_class_static_super_context = self.previous_class_static_super_context;
        let saved_legacy_decorated_static_initializer =
            self.current_legacy_decorated_static_initializer;
        self.current_class_container = Some(node);
        self.previous_class_static_block_receiver = saved_class_static_block_receiver;
        self.previous_class_static_super_context = saved_class_static_super_context;
        self.current_class_static_block_receiver = None;
        self.current_class_static_super_context = None;
        self.current_legacy_decorated_static_initializer = false;
        let result = self.transform_class_declaration_in_new_class_lexical_environment(node);
        self.current_class_static_block_receiver = saved_class_static_block_receiver;
        self.current_class_static_super_context = saved_class_static_super_context;
        self.previous_class_static_block_receiver = saved_previous_class_static_block_receiver;
        self.previous_class_static_super_context = saved_previous_class_static_super_context;
        self.current_legacy_decorated_static_initializer =
            saved_legacy_decorated_static_initializer;
        self.current_class_container = saved_current_class_container;
        self.pending_expressions = saved_pending_expressions;
        result
    }

    fn transform_class_declaration_in_new_class_lexical_environment(
        &mut self,
        node: ast::Node,
    ) -> ast::Node {
        let (
            members_loc,
            members_range,
            _original_member_count,
            is_derived_class,
            class_name,
            class_constructor_reference,
            class_constructor_reference_assignment,
            super_class_reference,
            private_accessor_brand_check_identifier,
            class_has_lexical_environment_facts,
            modifier_snapshot,
            type_parameters,
            heritage_clauses,
            member_nodes,
            is_export,
            is_default,
        ) = {
            let source = self.store_for(node);
            let members = source
                .members(node)
                .expect("class declaration should have members");
            let members_loc = members.loc();
            let members_range = members.range();
            let original_member_count = members.len();
            let is_derived_class = self.is_derived_class(node);
            let name = source.name(node);
            let modifier_snapshot =
                self.snapshot_optional_modifier_list(source.source_modifiers(node));
            let type_parameters =
                self.snapshot_optional_node_list(source.source_type_parameters(node));
            let heritage_clauses =
                self.snapshot_optional_node_list(source.source_heritage_clauses(node));
            let is_export = ast::has_syntactic_modifier(source, node, ast::ModifierFlags::EXPORT);
            let is_default = ast::has_syntactic_modifier(source, node, ast::ModifierFlags::DEFAULT);
            let member_nodes = members.iter().collect::<Vec<_>>();
            let class_name = name.map(|name| self.preserve_node(name));
            let private_accessor_brand_check_identifier = if self
                .config
                .should_transform_private_elements_or_class_static_blocks
                && self
                    .private_accessors(&member_nodes, false, false)
                    .is_some()
            {
                Some(self.create_hoisted_variable_for_class(class_name, "instances", ""))
            } else {
                None
            };
            let (
                class_constructor_reference,
                class_constructor_reference_assignment,
                super_class_reference,
            ) = self.create_class_references_if_needed(node);
            let class_has_lexical_environment_facts =
                self.class_has_lexical_environment_facts(node, &member_nodes);
            (
                members_loc,
                members_range,
                original_member_count,
                is_derived_class,
                class_name,
                class_constructor_reference,
                class_constructor_reference_assignment,
                super_class_reference,
                private_accessor_brand_check_identifier,
                class_has_lexical_environment_facts,
                modifier_snapshot,
                self.preserve_optional_node_list_snapshot(type_parameters),
                self.preserve_or_transform_heritage_clauses(
                    heritage_clauses,
                    super_class_reference,
                ),
                member_nodes,
                is_export,
                is_default,
            )
        };
        let saved_class_has_lexical_environment_facts =
            self.current_class_has_lexical_environment_facts;
        let saved_class_will_hoist_initializers_to_constructor =
            self.current_class_will_hoist_initializers_to_constructor;
        let saved_class_is_legacy_decorated = self.current_class_is_legacy_decorated;
        self.current_class_has_lexical_environment_facts = class_has_lexical_environment_facts;
        self.current_class_will_hoist_initializers_to_constructor =
            self.class_will_hoist_initializers_to_constructor(&member_nodes);
        self.current_class_is_legacy_decorated = self.class_was_decorated(node);
        let generated_class_name = if class_name.is_none()
            && member_nodes
                .iter()
                .any(|member| self.can_transform_static_property_or_class_static_block(*member))
        {
            Some(
                if node.store_id() == self.emit_context.factory.node_factory.store().store_id() {
                    self.emit_context.get_local_name(node)
                } else {
                    self.emit_context.factory.get_local_name(self.source, &node)
                },
            )
        } else {
            None
        };
        let class_name_for_transform = class_name.or(generated_class_name);
        let force_transform_static_private_elements = self.emit_context.emit_flags(&node)
            & printer::EF_TRANSFORM_PRIVATE_STATIC_ELEMENTS
            != 0
            || member_nodes.iter().any(|member| {
                ast::has_static_modifier(self.store_for(*member), *member)
                    && ast::is_private_identifier_class_element_declaration(
                        self.store_for(*member),
                        *member,
                    )
                    && self.should_always_transform_private_static_elements(*member)
            });
        let (private_static_field_environment, private_instance_field_environments) = self
            .collect_private_field_environments_in_member_order(
                &member_nodes,
                class_name_for_transform,
                class_constructor_reference.or(class_name_for_transform),
                force_transform_static_private_elements,
            );
        let (members, _instance_assignments, static_assignments, _changed, _, members_prologue) =
            self.transform_members(
                node,
                member_nodes,
                class_name_for_transform,
                class_constructor_reference,
                false,
                None,
                class_constructor_reference.or(class_name_for_transform),
                super_class_reference,
                class_name_for_transform,
                is_derived_class,
                private_accessor_brand_check_identifier,
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Some(private_instance_field_environments),
                private_static_field_environment,
            );
        self.current_class_has_lexical_environment_facts =
            saved_class_has_lexical_environment_facts;
        self.current_class_will_hoist_initializers_to_constructor =
            saved_class_will_hoist_initializers_to_constructor;
        self.current_class_is_legacy_decorated = saved_class_is_legacy_decorated;
        if let Some(class_constructor_reference_assignment) = class_constructor_reference_assignment
        {
            self.pending_expressions
                .insert(0, class_constructor_reference_assignment);
        }
        let member_list = self
            .factory_mut()
            .new_node_list(members_loc, members_range, members);
        let mut statements = Vec::new();
        if !self.pending_expressions.is_empty() {
            let pending_expressions = std::mem::take(&mut self.pending_expressions);
            let expression = self
                .emit_context
                .factory
                .inline_expressions(&pending_expressions)
                .expect("pending expressions should not be empty");
            statements.push(self.factory_mut().new_expression_statement(expression));
        }
        if let Some(members_prologue) = members_prologue {
            statements.push(
                self.factory_mut()
                    .new_expression_statement(members_prologue),
            );
        }
        statements.extend(static_assignments);
        let class_name = if !statements.is_empty() && class_name.is_none() {
            generated_class_name
        } else {
            class_name
        };
        let modifiers = if !statements.is_empty() && is_export && is_default {
            let allowed = if self.config.should_transform_auto_accessors
                || self.current_class_will_hoist_initializers_to_constructor
            {
                !ast::ModifierFlags::EXPORT_DEFAULT & !ast::ModifierFlags::ACCESSOR
            } else {
                !ast::ModifierFlags::EXPORT_DEFAULT
            };
            let modifiers = self
                .preserve_optional_modifier_list_snapshot_with_allowed(modifier_snapshot, allowed);
            let local_name = class_name.unwrap_or_else(|| {
                if node.store_id() == self.emit_context.factory.node_factory.store().store_id() {
                    self.emit_context.get_local_name(node)
                } else {
                    self.emit_context.factory.get_local_name(self.source, &node)
                }
            });
            let export_assignment = self.emit_context.factory.new_export_default(local_name);
            statements.push(export_assignment);
            modifiers
        } else {
            self.preserve_optional_modifier_list_snapshot_without_accessor(modifier_snapshot)
        };
        let updated_class = if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_class_declaration(
                node,
                modifiers,
                class_name,
                type_parameters,
                heritage_clauses,
                member_list,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_class_declaration_from_store(
                source,
                node,
                modifiers,
                class_name,
                type_parameters,
                heritage_clauses,
                member_list,
            )
        };
        if statements.is_empty() {
            return updated_class;
        }
        statements.insert(0, updated_class);
        self.factory_mut().new_syntax_list(statements)
    }

    fn transform_class_expression(&mut self, node: ast::Node) -> ast::Node {
        let saved_pending_expressions = std::mem::take(&mut self.pending_expressions);
        let saved_current_class_container = self.current_class_container;
        let saved_class_static_block_receiver = self.current_class_static_block_receiver;
        let saved_class_static_super_context = self.current_class_static_super_context;
        let saved_previous_class_static_block_receiver = self.previous_class_static_block_receiver;
        let saved_previous_class_static_super_context = self.previous_class_static_super_context;
        let saved_legacy_decorated_static_initializer =
            self.current_legacy_decorated_static_initializer;
        self.current_class_container = Some(node);
        self.previous_class_static_block_receiver = saved_class_static_block_receiver;
        self.previous_class_static_super_context = saved_class_static_super_context;
        self.current_class_static_block_receiver = None;
        self.current_class_static_super_context = None;
        self.current_legacy_decorated_static_initializer = false;
        let result = self.transform_class_expression_in_new_class_lexical_environment(node);
        self.current_class_static_block_receiver = saved_class_static_block_receiver;
        self.current_class_static_super_context = saved_class_static_super_context;
        self.previous_class_static_block_receiver = saved_previous_class_static_block_receiver;
        self.previous_class_static_super_context = saved_previous_class_static_super_context;
        self.current_legacy_decorated_static_initializer =
            saved_legacy_decorated_static_initializer;
        self.current_class_container = saved_current_class_container;
        self.pending_expressions = saved_pending_expressions;
        result
    }

    fn transform_class_expression_in_new_class_lexical_environment(
        &mut self,
        node: ast::Node,
    ) -> ast::Node {
        let (
            members_loc,
            members_range,
            _original_member_count,
            is_derived_class,
            needs_static_property_temp,
            class_name,
            storage_name_class_name,
            class_this,
            is_decorated_class_declaration,
            needs_class_constructor_reference,
            class_has_lexical_environment_facts,
            super_class_reference,
            modifiers,
            type_parameters,
            heritage_clauses,
            member_nodes,
            member_nodes_for_private_env,
        ) = {
            let (
                members_loc,
                members_range,
                original_member_count,
                member_nodes,
                name,
                modifiers,
                type_parameters,
                heritage_clauses,
            ) = {
                let source = self.store_for(node);
                let members = source
                    .members(node)
                    .expect("class expression should have members");
                (
                    members.loc(),
                    members.range(),
                    members.len(),
                    members.iter().collect::<Vec<_>>(),
                    source.name(node),
                    self.snapshot_optional_modifier_list(source.source_modifiers(node)),
                    self.snapshot_optional_node_list(source.source_type_parameters(node)),
                    self.snapshot_optional_node_list(source.source_heritage_clauses(node)),
                )
            };
            let is_derived_class = self.is_derived_class(node);
            let node_has_transform_private_static_elements_flag =
                self.emit_context.emit_flags(&node) & printer::EF_TRANSFORM_PRIVATE_STATIC_ELEMENTS
                    != 0;
            let needs_static_property_temp = (self
                .config
                .should_transform_private_elements_or_class_static_blocks
                || node_has_transform_private_static_elements_flag)
                && member_nodes.iter().copied().any(|member| {
                    let source = self.store_for(member);
                    ast::is_class_static_block_declaration(source, member)
                        || (ast::has_static_modifier(source, member)
                            && (ast::is_private_identifier_class_element_declaration(
                                source, member,
                            ) || (self.config.should_transform_initializers
                                && source.kind(member) == ast::Kind::PropertyDeclaration
                                && source.initializer(member).is_some())))
                });
            let has_transformable_static_private_accessors = self
                .private_accessors(
                    &member_nodes,
                    true,
                    node_has_transform_private_static_elements_flag,
                )
                .is_some();
            let class_name = name.map(|name| self.preserve_node(name));
            let class_this = self.emit_context.class_this(&node);
            let is_decorated_class_declaration = self.class_was_decorated(node);
            let needs_class_constructor_reference = self
                .class_declaration_needs_class_constructor_reference(node)
                || (self
                    .config
                    .should_transform_private_elements_or_class_static_blocks
                    && (self.class_has_class_this_assignment(node)
                        || self.class_has_explicitly_assigned_name(node)));
            let assigned_declaration_name = (needs_static_property_temp
                || has_transformable_static_private_accessors)
                .then(|| {
                    ast::get_name_of_declaration(self.store_for(node), Some(node)).and_then(
                        |name| {
                            ast::is_identifier(self.store_for(name), name)
                                .then(|| self.preserve_node(name))
                        },
                    )
                })
                .flatten();
            let storage_name_class_name = class_name
                .or(assigned_declaration_name)
                .or_else(|| self.class_expression_assigned_name_from_stack(node))
                .or_else(|| self.class_expression_assigned_name(node));
            let class_has_lexical_environment_facts =
                self.class_has_lexical_environment_facts(node, &member_nodes);
            let super_class_reference = if self.class_declaration_needs_class_super_reference(node)
            {
                let temp = self
                    .emit_context
                    .factory
                    .new_temp_variable_ex(AutoGenerateOptions {
                        flags: GeneratedIdentifierFlags::RESERVED_IN_NESTED_SCOPES,
                        ..Default::default()
                    });
                self.emit_context.add_variable_declaration(temp);
                Some(temp)
            } else {
                None
            };
            (
                members_loc,
                members_range,
                original_member_count,
                is_derived_class,
                needs_static_property_temp && !is_decorated_class_declaration,
                class_name,
                storage_name_class_name,
                class_this,
                is_decorated_class_declaration,
                needs_class_constructor_reference,
                class_has_lexical_environment_facts,
                super_class_reference,
                self.preserve_optional_modifier_list_snapshot_without_accessor(modifiers),
                self.preserve_optional_node_list_snapshot(type_parameters),
                self.preserve_or_transform_heritage_clauses(
                    heritage_clauses,
                    super_class_reference,
                ),
                member_nodes.clone(),
                member_nodes,
            )
        };
        let private_accessors = self.private_accessors(&member_nodes_for_private_env, false, false);
        let _has_private_accessors = private_accessors.is_some();
        let private_storage_name_class_name = storage_name_class_name;
        let weak_set_name = private_accessors.as_ref().map(|_| {
            self.create_hoisted_variable_for_class(private_storage_name_class_name, "instances", "")
        });
        let will_hoist_initializers_to_constructor =
            self.class_will_hoist_initializers_to_constructor(&member_nodes_for_private_env);
        let _should_transform_auto_accessors =
            self.config.should_transform_auto_accessors || will_hoist_initializers_to_constructor;
        // Private instance elements (fields, methods, accessors) transformed to
        // WeakMap/WeakSet will add initialization expressions to pendingExpressions
        // during transformClassMembers. Pre-detect this so we know whether the class
        // will be wrapped with a temp variable.
        let will_have_private_pending_expressions = self
            .config
            .should_transform_private_elements_or_class_static_blocks
            && member_nodes_for_private_env.iter().any(|member| {
                let source = self.store_for(*member);
                ast::is_private_identifier_class_element_declaration(source, *member)
                    && !ast::has_static_modifier(source, *member)
            });
        let is_class_with_constructor_reference =
            self.class_contains_constructor_reference(node, &member_nodes_for_private_env);
        let will_need_temp_wrapper =
            needs_static_property_temp || will_have_private_pending_expressions;
        // Register class alias BEFORE visiting members (Strada registers after, since its
        // onSubstituteNode runs at emit time). Only register when the class will be wrapped
        // with a temp, matching Strada's conditional registration.
        let (
            mut static_property_temp,
            mut static_property_temp_declared,
            defer_static_property_temp_declaration,
        ) = if class_this.is_none() && needs_class_constructor_reference {
            let temp = self
                .emit_context
                .factory
                .new_temp_variable_ex(AutoGenerateOptions {
                    flags: GeneratedIdentifierFlags::RESERVED_IN_NESTED_SCOPES,
                    ..Default::default()
                });
            self.add_class_expression_temp_declaration(temp);
            (Some(temp), true, false)
        } else if class_this.is_none()
            && !is_decorated_class_declaration
            && is_class_with_constructor_reference
            && will_need_temp_wrapper
        {
            // Create temp early so the alias is available during member visiting, even though in the Strada
            // reference the temp would be created later in the pendingExpressions branch.
            let temp = self
                .emit_context
                .factory
                .new_temp_variable_ex(AutoGenerateOptions {
                    flags: GeneratedIdentifierFlags::RESERVED_IN_NESTED_SCOPES,
                    ..Default::default()
                });
            // Defer AddVariableDeclaration to preserve Strada's variable declaration ordering.
            (Some(temp), false, true)
        } else {
            (None, false, false)
        };
        let force_transform_static_private_elements = self.emit_context.emit_flags(&node)
            & printer::EF_TRANSFORM_PRIVATE_STATIC_ELEMENTS
            != 0
            || member_nodes_for_private_env.iter().any(|member| {
                ast::has_static_modifier(self.store_for(*member), *member)
                    && ast::is_private_identifier_class_element_declaration(
                        self.store_for(*member),
                        *member,
                    )
                    && self.should_always_transform_private_static_elements(*member)
            });
        let pre_private_environment_variable_declaration_count =
            self.emit_context.current_variable_declaration_count();
        let (private_static_field_environment, private_instance_field_environments) = self
            .collect_private_field_environments_in_member_order(
                &member_nodes_for_private_env,
                private_storage_name_class_name,
                class_this.or(static_property_temp).or(class_name),
                force_transform_static_private_elements,
            );
        let private_accessor_environment = private_accessors.and_then(|private_accessors| {
            let private_name_validities =
                self.private_name_validities(&member_nodes_for_private_env);
            self.create_private_instance_accessor_environment(
                private_accessors,
                private_storage_name_class_name,
                weak_set_name.expect("private accessors should have a weak set name"),
                &private_name_validities,
            )
        });
        if static_property_temp.is_none()
            && class_this.is_none()
            && !is_decorated_class_declaration
            && will_have_private_pending_expressions
        {
            let temp = self
                .emit_context
                .factory
                .new_temp_variable_ex(AutoGenerateOptions {
                    flags: GeneratedIdentifierFlags::RESERVED_IN_NESTED_SCOPES,
                    ..Default::default()
                });
            if needs_static_property_temp {
                self.emit_context.insert_variable_declaration(
                    temp,
                    pre_private_environment_variable_declaration_count,
                );
            } else {
                self.emit_context.add_variable_declaration(temp);
            }
            static_property_temp = Some(temp);
            static_property_temp_declared = true;
        }
        let auto_accessor_storage_names = Vec::new();
        let precomputed_auto_accessor_storage_static_expressions = Vec::new();
        let saved_class_has_lexical_environment_facts =
            self.current_class_has_lexical_environment_facts;
        let saved_class_will_hoist_initializers_to_constructor =
            self.current_class_will_hoist_initializers_to_constructor;
        let saved_class_is_legacy_decorated = self.current_class_is_legacy_decorated;
        self.current_class_has_lexical_environment_facts = class_has_lexical_environment_facts;
        self.current_class_will_hoist_initializers_to_constructor =
            will_hoist_initializers_to_constructor;
        self.current_class_is_legacy_decorated = is_decorated_class_declaration;
        let class_name_for_transform = if is_decorated_class_declaration {
            storage_name_class_name.or_else(|| Some(self.emit_context.get_local_name(node)))
        } else {
            static_property_temp.or(class_name)
        };
        let (
            members,
            _assignments,
            static_assignments,
            _changed,
            pending_static_expression_statement_count,
            members_prologue,
        ) = self.transform_members(
            node,
            member_nodes,
            class_name_for_transform,
            static_property_temp,
            false,
            None,
            class_this,
            super_class_reference,
            storage_name_class_name,
            is_derived_class,
            None,
            private_accessor_environment,
            auto_accessor_storage_names,
            Vec::new(),
            precomputed_auto_accessor_storage_static_expressions,
            Some(private_instance_field_environments),
            private_static_field_environment,
        );
        self.current_class_has_lexical_environment_facts =
            saved_class_has_lexical_environment_facts;
        self.current_class_will_hoist_initializers_to_constructor =
            saved_class_will_hoist_initializers_to_constructor;
        self.current_class_is_legacy_decorated = saved_class_is_legacy_decorated;
        if defer_static_property_temp_declaration && let Some(temp) = static_property_temp {
            self.add_class_expression_temp_declaration(temp);
            static_property_temp_declared = true;
        }
        let mut static_assignments = static_assignments;
        let members = members;
        let class_expression_prologue = members_prologue;
        let mut pending_expression_statement_count = 0usize;
        if !self.pending_expressions.is_empty() {
            let pending_expressions = std::mem::take(&mut self.pending_expressions);
            if is_decorated_class_declaration {
                pending_expression_statement_count = pending_expressions.len();
                let mut statements =
                    Vec::with_capacity(pending_expressions.len() + static_assignments.len());
                for expression in pending_expressions {
                    statements.push(self.factory_mut().new_expression_statement(expression));
                }
                statements.extend(static_assignments);
                static_assignments = statements;
            } else if self
                .config
                .should_transform_private_elements_or_class_static_blocks
            {
                let expression = self
                    .emit_context
                    .factory
                    .inline_expressions(&pending_expressions)
                    .expect("pending expressions should not be empty");
                let statement = self.factory_mut().new_expression_statement(expression);
                static_assignments.insert(0, statement);
                pending_expression_statement_count = 1;
            }
        }
        let needs_late_static_property_temp =
            class_this.is_none() && static_property_temp.is_none() && needs_static_property_temp;
        let mut members = members;
        if !self
            .config
            .should_transform_private_elements_or_class_static_blocks
            && class_this.is_some()
        {
            if let Some(class_this) = class_this
                && !members
                    .iter()
                    .any(|member| self.is_class_this_assignment_block(*member))
            {
                members.insert(
                    0,
                    self.create_class_this_assignment_static_block(class_this),
                );
            }
            if let Some(assigned_name) = self.emit_context.assigned_name(&node)
                && self.is_anonymous_class_needing_assigned_name(node)
                && !members
                    .iter()
                    .any(|member| self.is_class_named_evaluation_helper_block(*member))
            {
                let insert_index = members
                    .iter()
                    .position(|member| !self.is_class_this_assignment_block(*member))
                    .unwrap_or(members.len());
                members.insert(
                    insert_index,
                    self.create_class_named_evaluation_helper_block(assigned_name),
                );
            }
        }
        let member_list = self
            .factory_mut()
            .new_node_list(members_loc, members_range, members);
        let class_expression = if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_class_expression(
                node,
                modifiers,
                class_name,
                type_parameters,
                heritage_clauses,
                member_list,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_class_expression_from_store(
                source,
                node,
                modifiers,
                class_name,
                type_parameters,
                heritage_clauses,
                member_list,
            )
        };
        let class_expression = if let Some(prologue) = class_expression_prologue {
            self.emit_context
                .mark_emit_node(&class_expression, printer::EF_INDENTED);
            let expressions = [prologue, class_expression];
            for expression in &expressions {
                self.emit_context
                    .mark_emit_node(expression, printer::EF_START_ON_NEW_LINE);
            }
            self.emit_context
                .factory
                .inline_expressions(&expressions)
                .expect("class expression prologue should contain expressions")
        } else {
            class_expression
        };
        let late_static_property_temp = if needs_late_static_property_temp {
            static_assignments.truncate(
                pending_expression_statement_count + pending_static_expression_statement_count,
            );
            let temp = self
                .emit_context
                .factory
                .new_temp_variable_ex(AutoGenerateOptions {
                    flags: GeneratedIdentifierFlags::RESERVED_IN_NESTED_SCOPES,
                    ..Default::default()
                });
            self.emit_context.add_variable_declaration(temp);
            for member in member_nodes_for_private_env.iter().copied() {
                let source = self.store_for(member);
                if !(ast::is_class_static_block_declaration(source, member)
                    || ast::has_static_modifier(source, member))
                {
                    continue;
                }
                let receiver = self.clone_node_for_reuse(temp);
                if let Some(statement) = self.transform_property_or_class_static_block(
                    member,
                    receiver,
                    super_class_reference,
                ) {
                    static_assignments.push(statement);
                }
            }
            Some(temp)
        } else {
            None
        };
        if static_assignments.is_empty() {
            if self
                .config
                .should_transform_private_elements_or_class_static_blocks
                && let Some(class_this) = class_this
            {
                if self.is_assignment_to_class_this(class_expression, class_this) {
                    return class_expression;
                }
                let class_this = self.clone_node_for_reuse(class_this);
                return self
                    .emit_context
                    .factory
                    .new_assignment_expression(class_this, class_expression);
            }
            return class_expression;
        }
        if is_decorated_class_declaration {
            // Decorated class declaration path: emit static properties as separate statements
            // via pendingStatements, matching the class declaration output structure.
            self.pending_statements.extend(static_assignments);
            if let Some(static_property_temp) = static_property_temp {
                return self
                    .emit_context
                    .factory
                    .new_assignment_expression(static_property_temp, class_expression);
            }
            if self
                .config
                .should_transform_private_elements_or_class_static_blocks
                && let Some(class_this) = class_this
            {
                if self.is_assignment_to_class_this(class_expression, class_this) {
                    return class_expression;
                }
                let class_this = self.clone_node_for_reuse(class_this);
                return self
                    .emit_context
                    .factory
                    .new_assignment_expression(class_this, class_expression);
            }
            return class_expression;
        }
        if let Some(class_this) = class_this {
            if let Some(assigned_name) = self.emit_context.assigned_name(&node) {
                let named_evaluation_insert_index =
                    pending_expression_statement_count + pending_static_expression_statement_count;
                let mut insert_index = named_evaluation_insert_index.min(static_assignments.len());
                while insert_index < static_assignments.len()
                    && self.is_function_assignment_statement(static_assignments[insert_index])
                {
                    insert_index += 1;
                }
                let set_function_name = self.emit_context.factory.new_set_function_name_helper(
                    class_this,
                    assigned_name,
                    "",
                );
                let set_function_name_statement = self
                    .factory_mut()
                    .new_expression_statement(set_function_name);
                static_assignments.insert(insert_index, set_function_name_statement);
            }
            self.pending_statements.extend(static_assignments);
            if self
                .config
                .should_transform_private_elements_or_class_static_blocks
            {
                if self.is_assignment_to_class_this(class_expression, class_this) {
                    return class_expression;
                }
                let class_this = self.clone_node_for_reuse(class_this);
                return self
                    .emit_context
                    .factory
                    .new_assignment_expression(class_this, class_expression);
            }
            return class_expression;
        }
        let (temp, declared_temp, declaration_index) = if let Some(temp) = static_property_temp {
            // Defer AddVariableDeclaration to preserve Strada's variable declaration ordering.
            (temp, static_property_temp_declared, None)
        } else if let Some(temp) = late_static_property_temp {
            (temp, true, None)
        } else {
            let temp = self
                .emit_context
                .factory
                .new_temp_variable_ex(AutoGenerateOptions {
                    flags: GeneratedIdentifierFlags::RESERVED_IN_NESTED_SCOPES,
                    ..Default::default()
                });
            self.emit_context.add_variable_declaration(temp);
            self.emit_context
                .move_current_variable_declarations_to_end(&[temp]);
            (temp, true, None)
        };
        if !declared_temp {
            if let Some(index) = declaration_index {
                self.emit_context.insert_variable_declaration(temp, index);
            } else {
                self.emit_context.add_variable_declaration(temp);
            }
        }
        let assigned_name = if self.class_has_explicitly_assigned_name(node) {
            None
        } else {
            self.emit_context.assigned_name(&node)
        };
        let mut expressions = Vec::with_capacity(
            static_assignments.len()
                + 2
                + usize::from(assigned_name.is_some())
                + usize::from(members_prologue.is_some()),
        );
        if let Some(members_prologue) = members_prologue {
            expressions.push(members_prologue);
        }
        expressions.push(
            self.emit_context
                .factory
                .new_assignment_expression(temp, class_expression),
        );
        let named_evaluation_insert_index =
            pending_expression_statement_count + pending_static_expression_statement_count;
        for statement in static_assignments
            .iter()
            .take(named_evaluation_insert_index)
        {
            if let Some(expression) = self.store_for(*statement).expression(*statement) {
                self.emit_context
                    .assign_comment_and_source_map_ranges(&expression, statement);
                self.append_comma_expression_elements(expression, &mut expressions);
            }
        }
        if let Some(assigned_name) = assigned_name {
            expressions.push(self.emit_context.factory.new_set_function_name_helper(
                temp,
                assigned_name,
                "",
            ));
        }
        for statement in static_assignments
            .iter()
            .skip(named_evaluation_insert_index)
        {
            if let Some(expression) = self.store_for(*statement).expression(*statement) {
                self.emit_context
                    .assign_comment_and_source_map_ranges(&expression, statement);
                self.append_comma_expression_elements(expression, &mut expressions);
            }
        }
        expressions.push(temp);
        if expressions.len() > 1 {
            self.emit_context
                .mark_emit_node(&class_expression, printer::EF_INDENTED);
            for expression in &expressions {
                self.emit_context
                    .mark_emit_node(expression, printer::EF_START_ON_NEW_LINE);
            }
        }
        self.emit_context
            .factory
            .inline_expressions(&expressions)
            .expect("class expression wrapper should contain expressions")
    }

    fn generate_initialized_property_expressions_or_class_static_block(
        &mut self,
        properties_or_class_static_blocks: &[ast::Node],
        receiver: ast::Node,
        super_class_reference: Option<ast::Node>,
    ) -> Vec<ast::Node> {
        let mut expressions = Vec::new();
        for property in properties_or_class_static_blocks {
            let source = self.store_for(*property);
            if !ast::is_class_static_block_declaration(source, *property)
                && !ast::has_static_modifier(source, *property)
            {
                continue;
            }
            let expression = if ast::is_class_static_block_declaration(source, *property) {
                self.transform_class_static_block_declaration(
                    *property,
                    receiver,
                    super_class_reference,
                )
            } else {
                self.transform_property(*property, receiver)
            };
            if let Some(expression) = expression {
                self.emit_context.set_original(&expression, property);
                self.emit_context
                    .assign_comment_and_source_map_ranges(&expression, property);
                expressions.push(expression);
            }
        }
        expressions
    }

    fn append_comma_expression_elements(
        &mut self,
        expression: ast::Node,
        out: &mut Vec<ast::Node>,
    ) {
        let store = self.store_for(expression);
        if store.kind(expression) == ast::Kind::BinaryExpression
            && store
                .operator_token(expression)
                .is_some_and(|operator| store.kind(operator) == ast::Kind::CommaToken)
        {
            let left = store.left(expression);
            let right = store.right(expression);
            if let Some(left) = left {
                self.append_comma_expression_elements(left, out);
            }
            if let Some(right) = right {
                self.append_comma_expression_elements(right, out);
            }
        } else {
            out.push(expression);
        }
    }

    fn class_expression_assigned_name(&mut self, node: ast::Node) -> Option<ast::Node> {
        let assigned_name = self.emit_context.assigned_name(&node)?;
        let assigned_name_source = self.store_for(assigned_name);
        if assigned_name_source.kind(assigned_name) != ast::Kind::StringLiteral {
            return None;
        }
        if let Some(text_source) = self.emit_context.text_source(&assigned_name) {
            if ast::is_identifier(self.store_for(text_source), text_source) {
                return Some(self.preserve_node(text_source));
            }
        }
        let text = assigned_name_source.text(assigned_name);
        if scanner::is_identifier_text(&text, core::LanguageVariant::Standard) {
            return Some(self.factory_mut().new_identifier(text));
        }
        None
    }

    fn class_expression_assigned_name_from_stack(&mut self, node: ast::Node) -> Option<ast::Node> {
        self.class_expression_assigned_name_stack
            .iter()
            .rev()
            .find_map(|entry| (entry.class_expression == node).then_some(entry.assigned_name))
            .map(|name| self.preserve_node(name))
    }

    fn class_expression_private_storage_assigned_name(
        &mut self,
        node: ast::Node,
    ) -> Option<ClassExpressionAssignedName> {
        let source = self.store_for(node);
        let name = source.name(node)?;
        if !ast::is_identifier(self.store_for(name), name) {
            return None;
        }
        let initializer = source.initializer(node)?;
        let initializer_source = self.store_for(initializer);
        if !ast::is_class_expression(initializer_source, initializer)
            || initializer_source.name(initializer).is_some()
        {
            return None;
        }
        let members = initializer_source.members(initializer)?;
        let member_nodes = members.iter().collect::<Vec<_>>();
        let node_has_transform_private_static_elements_flag =
            self.emit_context.emit_flags(&initializer)
                & printer::EF_TRANSFORM_PRIVATE_STATIC_ELEMENTS
                != 0;
        let has_static_private_accessors = self
            .private_accessors(
                &member_nodes,
                true,
                node_has_transform_private_static_elements_flag,
            )
            .is_some();
        let has_local_private_instance_storage = !self.is_exported_variable_declaration(node)
            && (self
                .private_accessors(&member_nodes, false, false)
                .is_some()
                || member_nodes.iter().any(|member| {
                    let source = self.store_for(*member);
                    source.kind(*member) == ast::Kind::PropertyDeclaration
                        && !ast::has_static_modifier(source, *member)
                        && source
                            .name(*member)
                            .is_some_and(|name| ast::is_private_identifier(source, name))
                }));
        (has_static_private_accessors || has_local_private_instance_storage).then_some(
            ClassExpressionAssignedName {
                class_expression: initializer,
                assigned_name: name,
            },
        )
    }

    fn is_exported_variable_declaration(&self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        self.exported_variable_declaration_stack.contains(&node)
            || ast::get_combined_modifier_flags(source, node).intersects(ast::ModifierFlags::EXPORT)
    }

    fn visit_variable_declaration(&mut self, node: ast::Node) -> ast::Node {
        // 14.3.1.2 RS: Evaluation
        //   LexicalBinding : BindingIdentifier Initializer
        //     ...
        //     3. If IsAnonymousFunctionDefinition(|Initializer|) is *true*, then
        //        a. Let _value_ be ? NamedEvaluation of |Initializer| with argument _bindingId_.
        //     ...
        //
        // 14.3.2.1 RS: Evaluation
        //   VariableDeclaration : BindingIdentifier Initializer
        //     ...
        //     3. If IsAnonymousFunctionDefinition(|Initializer|) is *true*, then
        //        a. Let _value_ be ? NamedEvaluation of |Initializer| with argument _bindingId_.
        //     ...

        let is_named_evaluation =
            self.is_named_evaluation_with_anonymous_class_needing_assigned_name(node);
        let node = if is_named_evaluation {
            self.transform_named_evaluation_of_variable_declaration(node)
        } else {
            node
        };
        let assigned_name_entry = (!is_named_evaluation)
            .then(|| self.class_expression_private_storage_assigned_name(node))
            .flatten();
        if let Some(entry) = assigned_name_entry {
            self.class_expression_assigned_name_stack.push(entry);
        }
        let result = self.generated_visit_each_child(&node);
        if assigned_name_entry.is_some() {
            self.class_expression_assigned_name_stack.pop();
        }
        result
    }

    fn visit_expression_statement(&mut self, node: ast::Node) -> ast::Node {
        let (expression, loc) = {
            let source = self.store_for(node);
            (source.expression(node), source.loc(node))
        };
        let Some(expression) = expression else {
            return self.generated_visit_each_child(&node);
        };
        if self
            .config
            .should_transform_private_elements_or_class_static_blocks
            && ast::is_private_identifier(self.store_for(expression), expression)
        {
            return node;
        }
        let expression = match self.store_for(expression).kind(expression) {
            ast::Kind::BinaryExpression => self
                .transform_super_property_assignment_in_static_initializer_with_discard(
                    expression, true,
                )
                .or_else(|| self.visit_node(Some(expression)))
                .unwrap_or(expression),
            ast::Kind::PrefixUnaryExpression | ast::Kind::PostfixUnaryExpression => self
                .transform_private_identifier_update_expression(expression, true)
                .or_else(|| {
                    self.transform_super_property_update_in_static_initializer(expression, true)
                })
                .or_else(|| self.visit_node(Some(expression)))
                .unwrap_or(expression),
            _ => self.visit_node(Some(expression)).unwrap_or(expression),
        };
        let updated = if node.store_id() == self.factory().store().store_id() {
            self.factory_mut()
                .update_expression_statement(node, expression)
        } else {
            let source = self.source;
            self.factory_mut()
                .update_expression_statement_from_store(source, node, expression)
        };
        self.factory_mut().place_emit_synthetic_node(updated, loc);
        updated
    }

    fn is_named_evaluation_with_anonymous_class_needing_assigned_name(
        &mut self,
        node: ast::Node,
    ) -> bool {
        let source = self.store_for(node);
        if !ast::is_named_evaluation_source(source, node) {
            return false;
        }
        let Some(initializer) = source.initializer(node) else {
            return false;
        };
        self.is_anonymous_class_needing_assigned_name(initializer)
    }

    fn is_anonymous_class_needing_assigned_name(&mut self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        let inner = ast::skip_outer_expressions(source, node, ast::OuterExpressionKinds::ALL);
        let inner_source = self.store_for(inner);
        if !ast::is_class_expression(inner_source, inner) || inner_source.name(inner).is_some() {
            return false;
        }
        let Some(members) = inner_source.members(inner) else {
            return false;
        };
        let member_nodes = members.iter().collect::<Vec<_>>();
        if member_nodes
            .iter()
            .any(|member| self.is_class_named_evaluation_helper_block(*member))
        {
            return false;
        }
        let has_transformable_statics = self
            .config
            .should_transform_private_elements_or_class_static_blocks
            || self.emit_context.emit_flags(&inner) & printer::EF_TRANSFORM_PRIVATE_STATIC_ELEMENTS
                != 0;
        if !has_transformable_statics {
            return false;
        }
        member_nodes
            .into_iter()
            .filter(|member| {
                let source = self.store_for(*member);
                ast::is_class_static_block_declaration(source, *member)
                    || (ast::is_property_declaration(source, *member)
                        && ast::has_static_modifier(source, *member))
            })
            .any(|member| {
                let source = self.store_for(member);
                ast::is_class_static_block_declaration(source, member)
                    || (ast::has_static_modifier(source, member)
                        && (ast::is_private_identifier_class_element_declaration(source, member)
                            || (self.config.should_transform_initializers
                                && ast::is_initialized_property(source, member))))
            })
    }

    fn is_class_named_evaluation_helper_block(&mut self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        if !ast::is_class_static_block_declaration(source, node) {
            return false;
        }
        let Some(assigned_name) = self.emit_context.assigned_name(&node) else {
            return false;
        };
        let Some(body) = source.body(node) else {
            return false;
        };
        let Some(statements) = self.store_for(body).statements(body) else {
            return false;
        };
        if statements.len() != 1 {
            return false;
        }
        let Some(statement) = statements.first() else {
            return false;
        };
        let statement_source = self.store_for(statement);
        if statement_source.kind(statement) != ast::Kind::ExpressionStatement {
            return false;
        }
        let Some(expression) = statement_source.expression(statement) else {
            return false;
        };
        if !self
            .emit_context
            .is_call_to_helper(&expression, "__setFunctionName")
        {
            return false;
        }
        let Some(arguments) = self.store_for(expression).arguments(expression) else {
            return false;
        };
        arguments.len() >= 2 && arguments.iter().nth(1) == Some(assigned_name)
    }

    fn class_has_class_this_assignment(&mut self, node: ast::Node) -> bool {
        let Some(members) = self.store_for(node).members(node) else {
            return false;
        };
        let member_nodes = members.iter().collect::<Vec<_>>();
        member_nodes
            .into_iter()
            .any(|member| self.is_class_this_assignment_block(member))
    }

    fn class_has_explicitly_assigned_name(&mut self, node: ast::Node) -> bool {
        if self.emit_context.assigned_name(&node).is_none() {
            return false;
        }
        let Some(members) = self.store_for(node).members(node) else {
            return false;
        };
        let member_nodes = members.iter().collect::<Vec<_>>();
        member_nodes
            .into_iter()
            .any(|member| self.is_class_named_evaluation_helper_block(member))
    }

    fn transform_named_evaluation_of_variable_declaration(&mut self, node: ast::Node) -> ast::Node {
        let source = self.store_for(node);
        let name = source
            .name(node)
            .expect("NamedEvaluation variable declaration should have a name");
        let initializer = source
            .initializer(node)
            .expect("NamedEvaluation variable declaration should have initializer");
        let assigned_name = self.get_assigned_name_of_identifier(name, initializer);
        let name = self.preserve_node(name);
        let initializer = self.finish_transform_named_evaluation(initializer, assigned_name, false);
        let inner = ast::skip_outer_expressions(
            self.store_for(initializer),
            initializer,
            ast::OuterExpressionKinds::ALL,
        );
        let assigned_name_entry = ast::is_class_expression(self.store_for(inner), inner).then_some(
            ClassExpressionAssignedName {
                class_expression: inner,
                assigned_name: name,
            },
        );
        if let Some(entry) = assigned_name_entry {
            self.class_expression_assigned_name_stack.push(entry);
        }
        let initializer = self.visit_node(Some(initializer)).unwrap_or(initializer);
        if assigned_name_entry.is_some() {
            self.class_expression_assigned_name_stack.pop();
        }
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut()
                .update_variable_declaration(node, name, None, None, initializer)
        } else {
            let source = self.source;
            self.factory_mut().update_variable_declaration_from_store(
                source,
                node,
                name,
                None,
                None,
                initializer,
            )
        }
    }

    fn visit_property_assignment(&mut self, node: ast::Node) -> ast::Node {
        // 13.2.5.5 RS: PropertyDefinitionEvaluation
        //   PropertyAssignment : PropertyName `:` AssignmentExpression
        //     ...
        //     5. If IsAnonymousFunctionDefinition(|AssignmentExpression|) is *true* and _isProtoSetter_ is *false*, then
        //        a. Let _popValue_ be ? NamedEvaluation of |AssignmentExpression| with argument _propKey_.
        //     ...
        if self.is_named_evaluation_with_anonymous_class_needing_assigned_name(node) {
            return self.transform_named_evaluation_of_property_assignment(node);
        }
        self.generated_visit_each_child(&node)
    }

    fn transform_named_evaluation_of_property_assignment(&mut self, node: ast::Node) -> ast::Node {
        let source = self.store_for(node);
        let name = source
            .name(node)
            .expect("NamedEvaluation property assignment should have a name");
        let initializer = source
            .initializer(node)
            .expect("NamedEvaluation property assignment should have initializer");
        let (assigned_name, name) = self.get_assigned_name_of_property_name(name);
        let initializer = self.finish_transform_named_evaluation(initializer, assigned_name, false);
        let initializer = self.visit_node(Some(initializer)).unwrap_or(initializer);
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_property_assignment(
                node,
                None,
                Some(name),
                None,
                None,
                Some(initializer),
            )
        } else {
            let source = self.source;
            self.factory_mut().update_property_assignment_from_store(
                source,
                node,
                None,
                Some(name),
                None,
                None,
                Some(initializer),
            )
        }
    }

    fn visit_parameter_declaration(&mut self, node: ast::Node) -> ast::Node {
        // 8.6.3 RS: IteratorBindingInitialization
        //   SingleNameBinding : BindingIdentifier Initializer?
        //     ...
        //     5. If |Initializer| is present and _v_ is *undefined*, then
        //        a. If IsAnonymousFunctionDefinition(|Initializer|) is *true*, then
        //           i. Set _v_ to ? NamedEvaluation of |Initializer| with argument _bindingId_.
        //     ...
        //
        // 14.3.3.3 RS: KeyedBindingInitialization
        //   SingleNameBinding : BindingIdentifier Initializer?
        //     ...
        //     4. If |Initializer| is present and _v_ is *undefined*, then
        //        a. If IsAnonymousFunctionDefinition(|Initializer|) is *true*, then
        //           i. Set _v_ to ? NamedEvaluation of |Initializer| with argument _bindingId_.
        //     ...
        if self.is_named_evaluation_with_anonymous_class_needing_assigned_name(node) {
            return self.transform_named_evaluation_of_parameter_declaration(node);
        }
        self.generated_visit_each_child(&node)
    }

    fn transform_named_evaluation_of_parameter_declaration(
        &mut self,
        node: ast::Node,
    ) -> ast::Node {
        let source = self.store_for(node);
        let name = source
            .name(node)
            .expect("NamedEvaluation parameter declaration should have a name");
        let initializer = source
            .initializer(node)
            .expect("NamedEvaluation parameter declaration should have initializer");
        let dot_dot_dot_token = source
            .dot_dot_dot_token(node)
            .map(|dot_dot_dot_token| self.preserve_node(dot_dot_dot_token));
        let name = self.preserve_node(name);
        let assigned_name = self.get_assigned_name_of_identifier(name, initializer);
        let initializer = self.finish_transform_named_evaluation(initializer, assigned_name, false);
        let initializer = self.visit_node(Some(initializer)).unwrap_or(initializer);
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_parameter_declaration(
                node,
                None::<ast::ModifierList>,
                dot_dot_dot_token,
                name,
                None::<ast::Node>,
                None::<ast::Node>,
                initializer,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_parameter_declaration_from_store(
                source,
                node,
                None::<ast::ModifierList>,
                dot_dot_dot_token,
                name,
                None::<ast::Node>,
                None::<ast::Node>,
                initializer,
            )
        }
    }

    fn visit_binding_element(&mut self, node: ast::Node) -> ast::Node {
        // 8.6.3 RS: IteratorBindingInitialization
        //   SingleNameBinding : BindingIdentifier Initializer?
        //     ...
        //     5. If |Initializer| is present and _v_ is *undefined*, then
        //        a. If IsAnonymousFunctionDefinition(|Initializer|) is *true*, then
        //           i. Set _v_ to ? NamedEvaluation of |Initializer| with argument _bindingId_.
        //     ...
        //
        // 14.3.3.3 RS: KeyedBindingInitialization
        //   SingleNameBinding : BindingIdentifier Initializer?
        //     ...
        //     4. If |Initializer| is present and _v_ is *undefined*, then
        //        a. If IsAnonymousFunctionDefinition(|Initializer|) is *true*, then
        //           i. Set _v_ to ? NamedEvaluation of |Initializer| with argument _bindingId_.
        //     ...
        if self.is_named_evaluation_with_anonymous_class_needing_assigned_name(node) {
            return self.transform_named_evaluation_of_binding_element(node);
        }
        self.generated_visit_each_child(&node)
    }

    fn transform_named_evaluation_of_binding_element(&mut self, node: ast::Node) -> ast::Node {
        let (dot_dot_dot_token, property_name, name, initializer) = {
            let source = self.store_for(node);
            (
                source.dot_dot_dot_token(node),
                source.property_name(node),
                source
                    .name(node)
                    .expect("NamedEvaluation binding element should have a name"),
                source
                    .initializer(node)
                    .expect("NamedEvaluation binding element should have initializer"),
            )
        };
        let dot_dot_dot_token =
            dot_dot_dot_token.map(|dot_dot_dot_token| self.preserve_node(dot_dot_dot_token));
        let property_name = property_name.map(|property_name| self.preserve_node(property_name));
        let name = self.preserve_node(name);
        let assigned_name = self.get_assigned_name_of_identifier(name, initializer);
        let initializer = self.finish_transform_named_evaluation(initializer, assigned_name, false);
        let initializer = self.visit_node(Some(initializer)).unwrap_or(initializer);
        let initializer = self.parenthesize_comma_expression_if_needed(initializer);
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_binding_element(
                node,
                dot_dot_dot_token,
                property_name,
                Some(name),
                Some(initializer),
            )
        } else {
            let source = self.source;
            self.factory_mut().update_binding_element_from_store(
                source,
                node,
                dot_dot_dot_token,
                property_name,
                Some(name),
                Some(initializer),
            )
        }
    }

    fn parenthesize_comma_expression_if_needed(&mut self, node: ast::Node) -> ast::Node {
        let source = self.store_for(node);
        if source.kind(node) == ast::Kind::BinaryExpression
            && source
                .operator_token(node)
                .is_some_and(|operator| source.kind(operator) == ast::Kind::CommaToken)
        {
            return self.factory_mut().new_parenthesized_expression(node);
        }
        node
    }

    fn visit_export_assignment(&mut self, node: ast::Node) -> ast::Node {
        self.generated_visit_each_child(&node)
    }

    // Injects a class `static {}` block used to dynamically set the name of a class, if one does not already exist.
    fn inject_class_named_evaluation_helper_block_if_missing(
        &mut self,
        node: ast::Node,
        assigned_name: ast::Node,
    ) -> ast::Node {
        // given:
        //
        //  let C = class {
        //  };
        //
        // produces:
        //
        //  let C = class {
        //      static { __setFunctionName(this, "C"); }
        //  };
        //
        // NOTE: If the class has a `_classThis` assignment block, this helper will be injected after that block.
        if self.class_has_explicitly_assigned_name(node) {
            return self.preserve_node(node);
        }

        let (
            modifiers,
            name,
            type_parameters,
            heritage_clauses,
            mut members,
            members_loc,
            members_range,
        ) = {
            let source = self.store_for(node);
            let modifiers = source
                .source_modifiers(node)
                .map(ast::SourceModifierListInput::from_source);
            let name = source.name(node);
            let type_parameters = source
                .source_type_parameters(node)
                .map(ast::SourceNodeListInput::from_source);
            let heritage_clauses = source
                .source_heritage_clauses(node)
                .map(ast::SourceNodeListInput::from_source);
            let members = source
                .source_members(node)
                .expect("class expression should have members");
            let members_loc = members.loc();
            let members_range = members.range();
            let members = members.iter().collect::<Vec<_>>();
            (
                modifiers,
                name,
                type_parameters,
                heritage_clauses,
                members,
                members_loc,
                members_range,
            )
        };

        let modifiers = modifiers.map(|modifiers| {
            self.import_state.preserve_source_modifier_list_input(
                self.source,
                &mut self.emit_context.factory.node_factory,
                &modifiers,
            )
        });
        let name = name.map(|name| self.preserve_node(name));
        let type_parameters = type_parameters.map(|type_parameters| {
            self.import_state.preserve_source_node_list_input(
                self.source,
                &mut self.emit_context.factory.node_factory,
                &type_parameters,
            )
        });
        let heritage_clauses = heritage_clauses.map(|heritage_clauses| {
            self.import_state.preserve_source_node_list_input(
                self.source,
                &mut self.emit_context.factory.node_factory,
                &heritage_clauses,
            )
        });

        for member in &mut members {
            *member = self.preserve_node(*member);
        }
        let insertion_index = members
            .iter()
            .position(|member| self.is_class_this_assignment_block(*member))
            .map(|index| index + 1)
            .unwrap_or(0);
        let named_evaluation_block = self.create_class_named_evaluation_helper_block(assigned_name);
        if let Some(name) = name {
            if let Some(body) = self
                .store_for(named_evaluation_block)
                .body(named_evaluation_block)
                && let Some(statements) = self.store_for(body).statements(body)
                && let Some(statement) = statements.first()
            {
                self.emit_context
                    .set_source_map_range(&statement, self.store_for(name).loc(name));
            }
        }
        members.insert(insertion_index, named_evaluation_block);
        let members = self
            .factory_mut()
            .new_node_list(members_loc, members_range, members);

        let updated = if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_class_expression(
                node,
                modifiers,
                name,
                type_parameters,
                heritage_clauses,
                members,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_class_expression_from_store(
                source,
                node,
                modifiers,
                name,
                type_parameters,
                heritage_clauses,
                members,
            )
        };
        self.emit_context
            .set_assigned_name(&updated, &assigned_name);
        if let Some(class_this) = self.emit_context.class_this(&node) {
            self.emit_context.set_class_this(&updated, &class_this);
        }
        updated
    }

    fn restore_outer_expressions(
        &mut self,
        outer_expression: ast::Node,
        inner: ast::Node,
    ) -> ast::Node {
        let source = self.store_for(outer_expression);
        if !ast::is_outer_expression(source, outer_expression, ast::OuterExpressionKinds::ALL) {
            return inner;
        }
        let Some(expression) = source.expression(outer_expression) else {
            return inner;
        };
        let restored = self.restore_outer_expressions(expression, inner);
        let outer_expression = self.preserve_node(outer_expression);
        let source = self.store_for(outer_expression);
        match source.kind(outer_expression) {
            ast::Kind::ParenthesizedExpression => self
                .factory_mut()
                .update_parenthesized_expression(outer_expression, restored),
            ast::Kind::TypeAssertionExpression => {
                let type_node = source
                    .r#type(outer_expression)
                    .expect("type assertion should have type");
                self.factory_mut()
                    .update_type_assertion(outer_expression, type_node, restored)
            }
            ast::Kind::AsExpression => {
                let type_node = source
                    .r#type(outer_expression)
                    .expect("as expression should have type");
                self.factory_mut()
                    .update_as_expression(outer_expression, restored, type_node)
            }
            ast::Kind::SatisfiesExpression => {
                let type_node = source
                    .r#type(outer_expression)
                    .expect("satisfies expression should have type");
                self.factory_mut().update_satisfies_expression(
                    outer_expression,
                    restored,
                    type_node,
                )
            }
            ast::Kind::NonNullExpression => {
                let flags = source.flags(outer_expression);
                self.factory_mut()
                    .update_non_null_expression(outer_expression, restored, flags)
            }
            ast::Kind::ExpressionWithTypeArguments => {
                let type_arguments = source
                    .source_type_arguments(outer_expression)
                    .map(|nodes| (nodes.loc(), nodes.range(), nodes.iter().collect::<Vec<_>>()));
                let type_arguments = type_arguments
                    .map(|(loc, range, nodes)| self.factory_mut().new_node_list(loc, range, nodes));
                self.factory_mut().update_expression_with_type_arguments(
                    outer_expression,
                    restored,
                    type_arguments,
                )
            }
            ast::Kind::PartiallyEmittedExpression => self
                .factory_mut()
                .update_partially_emitted_expression(outer_expression, restored),
            _ => restored,
        }
    }

    fn finish_transform_named_evaluation(
        &mut self,
        expression: ast::Node,
        assigned_name: ast::Node,
        ignore_empty_string_literal: bool,
    ) -> ast::Node {
        if ignore_empty_string_literal {
            let assigned_name_source = self.store_for(assigned_name);
            if assigned_name_source.kind(assigned_name) == ast::Kind::StringLiteral
                && assigned_name_source.text(assigned_name).is_empty()
            {
                return expression;
            }
        }

        let source = self.store_for(expression);
        let inner = ast::skip_outer_expressions(source, expression, ast::OuterExpressionKinds::ALL);
        let inner_source = self.store_for(inner);
        let updated = if ast::is_class_expression(inner_source, inner) {
            self.inject_class_named_evaluation_helper_block_if_missing(inner, assigned_name)
        } else {
            let inner = self.preserve_node(inner);
            self.emit_context
                .factory
                .new_set_function_name_helper(inner, assigned_name, "")
        };
        self.restore_outer_expressions(expression, updated)
    }

    fn get_assigned_name_of_identifier(
        &mut self,
        name: ast::Node,
        expression: ast::Node,
    ) -> ast::Node {
        let expression_source = self.store_for(expression);
        let inner = ast::skip_outer_expressions(
            expression_source,
            expression,
            ast::OuterExpressionKinds::ALL,
        );
        let original = self.emit_context.most_original(&inner);
        let original_source = self.store_for(original);
        if (ast::is_class_declaration(original_source, original)
            || ast::is_function_declaration(original_source, original))
            && original_source.name(original).is_none()
            && ast::has_syntactic_modifier(original_source, original, ast::ModifierFlags::DEFAULT)
        {
            return self
                .factory_mut()
                .new_string_literal("default", ast::TokenFlags::NONE);
        }
        self.new_string_literal_from_node(name)
    }

    fn get_assigned_name_of_property_name(&mut self, name: ast::Node) -> (ast::Node, ast::Node) {
        if ast::is_property_name_literal(self.store_for(name), name)
            || ast::is_private_identifier(self.store_for(name), name)
        {
            let assigned_name = self.new_string_literal_from_node(name);
            return (assigned_name, self.preserve_node(name));
        }

        assert!(
            ast::is_computed_property_name(self.store_for(name), name),
            "Expected computed property name"
        );
        let expression = self
            .store_for(name)
            .expression(name)
            .expect("computed property name should have expression");
        if ast::is_property_name_literal(self.store_for(expression), expression)
            && !ast::is_identifier(self.store_for(expression), expression)
        {
            let assigned_name = self.new_string_literal_from_node(expression);
            return (assigned_name, self.preserve_node(name));
        }

        let source = self.source;
        let assigned_name = self
            .emit_context
            .factory
            .new_generated_name_for_node(source, &name);
        self.emit_context.add_variable_declaration(assigned_name);
        let expression = self.visit_node(Some(expression)).unwrap_or(expression);
        let key = self.emit_context.factory.new_prop_key_helper(expression);
        let assignment = self
            .emit_context
            .factory
            .new_assignment_expression(assigned_name, key);
        let name = self.factory_mut().update_computed_property_name_from_store(
            source,
            name,
            Some(assignment),
        );
        (assigned_name, name)
    }

    fn new_string_literal_from_node(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.source.store_id() {
            let source = self.source;
            return self
                .emit_context
                .factory
                .new_string_literal_from_node(source, &node);
        }
        let text = self.store_for(node).text(node).to_owned();
        self.factory_mut()
            .new_string_literal(text, ast::TokenFlags::NONE)
    }

    fn can_transform_static_property_initializer(&mut self, member: ast::Node) -> bool {
        let source = self.store_for(member);
        if source.kind(member) != ast::Kind::PropertyDeclaration
            || !ast::has_static_modifier(source, member)
            || ast::is_auto_accessor_property_declaration(source, member)
            || source.initializer(member).is_none()
        {
            return false;
        }
        if source
            .name(member)
            .is_some_and(|name| ast::is_private_identifier(self.store_for(name), name))
        {
            if !self
                .config
                .should_transform_private_elements_or_class_static_blocks
                && self.emit_context.emit_flags(&member)
                    & printer::EF_TRANSFORM_PRIVATE_STATIC_ELEMENTS
                    == 0
            {
                return false;
            }
        } else if !self.config.should_transform_initializers {
            return false;
        }
        true
    }

    fn can_transform_static_property_or_class_static_block(&mut self, member: ast::Node) -> bool {
        let source = self.store_for(member);
        if ast::is_class_static_block_declaration(source, member) {
            return self
                .config
                .should_transform_private_elements_or_class_static_blocks;
        }
        if ast::has_static_modifier(source, member)
            && ast::is_private_identifier_class_element_declaration(source, member)
        {
            return self
                .config
                .should_transform_private_elements_or_class_static_blocks
                || self.emit_context.emit_flags(&member)
                    & printer::EF_TRANSFORM_PRIVATE_STATIC_ELEMENTS
                    != 0;
        }
        if self.is_static_private_accessor(member) {
            return self
                .config
                .should_transform_private_elements_or_class_static_blocks
                || self.emit_context.emit_flags(&member)
                    & printer::EF_TRANSFORM_PRIVATE_STATIC_ELEMENTS
                    != 0;
        }
        self.can_transform_static_property_initializer(member)
    }

    fn can_transform_public_static_property_initializer_to_class_static_block(
        &self,
        member: ast::Node,
    ) -> bool {
        let source = self.store_for(member);
        if !self.config.should_transform_initializers
            || self
                .config
                .should_transform_private_elements_or_class_static_blocks
            || source.kind(member) != ast::Kind::PropertyDeclaration
            || source
                .parent(member)
                .is_some_and(|parent| ast::is_class_expression(self.store_for(parent), parent))
            || !ast::has_static_modifier(source, member)
            || ast::is_auto_accessor_property_declaration(source, member)
        {
            return false;
        }
        source
            .name(member)
            .is_some_and(|name| !ast::is_private_identifier(source, name))
    }

    fn is_static_private_accessor(&self, member: ast::Node) -> bool {
        let source = self.store_for(member);
        ast::has_static_modifier(source, member)
            && matches!(
                source.kind(member),
                ast::Kind::GetAccessor | ast::Kind::SetAccessor
            )
            && source
                .name(member)
                .is_some_and(|name| ast::is_private_identifier(self.store_for(name), name))
    }

    fn create_hoisted_variable_for_class(
        &mut self,
        class_name: Option<ast::Node>,
        name_text: &str,
        suffix: &'static str,
    ) -> ast::Node {
        let prefix = class_name
            .map(|class_name| {
                let source = self.store_for(class_name);
                format!("_{}_", source.text(class_name))
            })
            .unwrap_or_else(|| "_".to_string());
        let identifier = self.emit_context.factory.new_unique_name_ex(
            &format!("{prefix}{name_text}"),
            AutoGenerateOptions {
                flags: GeneratedIdentifierFlags::OPTIMISTIC
                    | GeneratedIdentifierFlags::RESERVED_IN_NESTED_SCOPES,
                suffix,
                ..Default::default()
            },
        );
        self.add_private_name_declaration(identifier);
        identifier
    }

    fn create_hoisted_variable_for_class_from_node(
        &mut self,
        class_name: Option<ast::Node>,
        name: ast::Node,
        suffix: &'static str,
    ) -> ast::Node {
        let prefix = class_name
            .map(|class_name| {
                let source = self.store_for(class_name);
                format!("_{}_", source.text(class_name))
            })
            .unwrap_or_else(|| "_".to_string());
        let flags = GeneratedIdentifierFlags::OPTIMISTIC
            | GeneratedIdentifierFlags::RESERVED_IN_NESTED_SCOPES;
        let identifier = if name.store_id() == self.factory().store().store_id() {
            self.emit_context
                .factory
                .new_generated_name_for_factory_node_with_prefix_and_suffix(
                    &name, flags, &prefix, suffix,
                )
        } else {
            assert_eq!(
                name.store_id(),
                self.source.store_id(),
                "hoisted private name cannot read unrelated AST store"
            );
            self.emit_context
                .factory
                .new_generated_name_for_node_with_prefix_and_suffix(
                    self.source,
                    &name,
                    flags,
                    &prefix,
                    suffix,
                )
        };
        self.add_private_name_declaration(identifier);
        identifier
    }

    fn create_hoisted_variable_for_class_non_optimistic(
        &mut self,
        class_name: Option<ast::Node>,
        name_text: &str,
        suffix: &'static str,
    ) -> ast::Node {
        let prefix = class_name
            .map(|class_name| {
                let source = self.store_for(class_name);
                format!("_{}_", source.text(class_name))
            })
            .unwrap_or_else(|| "_".to_string());
        let identifier = self.emit_context.factory.new_unique_name_ex(
            &format!("{prefix}{name_text}"),
            AutoGenerateOptions {
                flags: GeneratedIdentifierFlags::RESERVED_IN_NESTED_SCOPES,
                suffix,
                ..Default::default()
            },
        );
        self.add_private_name_declaration(identifier);
        identifier
    }

    fn requires_block_scoped_var(&self) -> bool {
        self.in_iteration_statement
            && self.current_class_container.is_some_and(|class_node| {
                ast::is_class_expression(self.store_for(class_node), class_node)
            })
    }

    // classExpressionNeedsBlockScopedTemp returns true when the class expression's temp variable
    // must be block-scoped. This is more specific than requiresBlockScopedVar: the class temp only
    // needs to be block-scoped when the class expression has a non-static property with a computed
    // property name inside a loop (matching the checker's BlockScopedBindingInLoop on the class node).
    fn class_expression_needs_block_scoped_temp(&self) -> bool {
        if !self.requires_block_scoped_var() {
            return false;
        }
        let Some(class_node) = self.current_class_container else {
            return false;
        };
        let source = self.store_for(class_node);
        let Some(members) = source.members(class_node) else {
            return false;
        };
        members.iter().any(|member| {
            let member_source = self.store_for(member);
            ast::is_property_declaration(member_source, member)
                && !ast::has_static_modifier(member_source, member)
                && member_source
                    .name(member)
                    .is_some_and(|name| ast::is_computed_property_name(self.store_for(name), name))
        })
    }

    fn add_class_expression_temp_declaration(&mut self, temp: ast::Node) {
        if self.class_expression_needs_block_scoped_temp() {
            self.emit_context.add_lexical_declaration(temp);
        } else {
            self.emit_context.add_variable_declaration(temp);
        }
    }

    fn add_private_name_declaration(&mut self, identifier: ast::Node) {
        if self.requires_block_scoped_var() {
            self.emit_context.add_lexical_declaration(identifier);
        } else {
            self.emit_context.add_variable_declaration(identifier);
        }
    }

    fn create_hoisted_variable_for_private_name(
        &mut self,
        class_name: Option<ast::Node>,
        name: ast::Node,
        suffix: &'static str,
    ) -> ast::Node {
        // If the name is a generated identifier (e.g., auto-accessor backing field),
        // use node-based name generation so the emitter can resolve the name properly.
        if self.emit_context.has_auto_generate_info(Some(&name)) {
            return self.create_hoisted_variable_for_class_from_node(class_name, name, suffix);
        }
        let source = self.store_for(name);
        let text = source.text(name);
        let text = text.strip_prefix('#').unwrap_or(&text);
        self.create_hoisted_variable_for_class(class_name, text, suffix)
    }

    fn private_accessors(
        &mut self,
        members: &[ast::Node],
        is_static: bool,
        force_transform_static_private_elements: bool,
    ) -> Option<Vec<ast::Node>> {
        let should_transform_private_static_elements = is_static
            && (force_transform_static_private_elements
                || members
                    .iter()
                    .any(|member| self.should_always_transform_private_static_elements(*member)));
        if !self
            .config
            .should_transform_private_elements_or_class_static_blocks
            && !should_transform_private_static_elements
        {
            return None;
        }
        let private_accessors = members
            .iter()
            .copied()
            .filter(|member| {
                let source = self.store_for(*member);
                (matches!(
                    source.kind(*member),
                    ast::Kind::MethodDeclaration | ast::Kind::GetAccessor | ast::Kind::SetAccessor
                ) || ast::is_auto_accessor_property_declaration(source, *member))
                    && ast::has_static_modifier(source, *member) == is_static
                    && source
                        .name(*member)
                        .is_some_and(|name| ast::is_private_identifier(self.store_for(name), name))
            })
            .collect::<Vec<_>>();
        if private_accessors.is_empty() {
            None
        } else {
            Some(private_accessors)
        }
    }

    fn is_reserved_private_name(&self, name: ast::Node) -> bool {
        let source = self.store_for(name);
        !(ast::is_private_identifier(source, name)
            && self.emit_context.has_auto_generate_info(Some(&name)))
            && source.text(name) == "#constructor"
    }

    fn should_always_transform_private_static_elements(&mut self, node: ast::Node) -> bool {
        ast::has_static_modifier(self.store_for(node), node)
            && self.emit_context.emit_flags(&node) & printer::EF_TRANSFORM_PRIVATE_STATIC_ELEMENTS
                != 0
    }

    fn private_name_validities(&self, members: &[ast::Node]) -> HashMap<String, PrivateNameState> {
        let mut states: HashMap<String, PrivateNameState> = HashMap::new();
        for (order, member) in members.iter().enumerate() {
            let source = self.store_for(*member);
            if !ast::is_private_identifier_class_element_declaration(source, *member) {
                continue;
            }
            let Some(name) = source.name(*member) else {
                continue;
            };
            let name_text = self.store_for(name).text(name);
            let is_static = ast::has_static_modifier(source, *member);
            let previous = states.get(&name_text).copied();
            let is_valid = !self.is_reserved_private_name(name) && previous.is_none();
            let next_state = match source.kind(*member) {
                ast::Kind::PropertyDeclaration
                    if ast::is_auto_accessor_property_declaration(source, *member) =>
                {
                    PrivateNameState {
                        kind: PrivateNameStateKind::Accessor,
                        is_static,
                        has_getter: true,
                        has_setter: true,
                        is_valid,
                        order,
                    }
                }
                ast::Kind::PropertyDeclaration => PrivateNameState {
                    kind: PrivateNameStateKind::Field,
                    is_static,
                    has_getter: false,
                    has_setter: false,
                    is_valid,
                    order,
                },
                ast::Kind::MethodDeclaration => PrivateNameState {
                    kind: PrivateNameStateKind::Method,
                    is_static,
                    has_getter: false,
                    has_setter: false,
                    is_valid,
                    order,
                },
                ast::Kind::GetAccessor => {
                    if let Some(mut previous) = previous
                        && previous.kind == PrivateNameStateKind::Accessor
                        && previous.is_static == is_static
                        && !previous.has_getter
                    {
                        previous.has_getter = true;
                        previous
                    } else {
                        PrivateNameState {
                            kind: PrivateNameStateKind::Accessor,
                            is_static,
                            has_getter: true,
                            has_setter: false,
                            is_valid,
                            order,
                        }
                    }
                }
                ast::Kind::SetAccessor => {
                    if let Some(mut previous) = previous
                        && previous.kind == PrivateNameStateKind::Accessor
                        && previous.is_static == is_static
                        && !previous.has_setter
                    {
                        previous.has_setter = true;
                        previous
                    } else {
                        PrivateNameState {
                            kind: PrivateNameStateKind::Accessor,
                            is_static,
                            has_getter: false,
                            has_setter: true,
                            is_valid,
                            order,
                        }
                    }
                }
                _ => continue,
            };
            states.insert(name_text, next_state);
        }
        states
    }

    fn collect_private_static_field_environment(
        &mut self,
        members: &[ast::Node],
        class_name: Option<ast::Node>,
        brand_check_identifier: ast::Node,
        force_transform_static_private_elements: bool,
    ) -> Option<PrivateStaticFieldEnvironment> {
        if !self
            .config
            .should_transform_private_elements_or_class_static_blocks
            && !force_transform_static_private_elements
        {
            return None;
        }
        let private_name_validities = self.private_name_validities(members);
        let mut fields = Vec::new();
        for (order, member) in members.iter().enumerate() {
            let source = self.store_for(*member);
            if source.kind(*member) != ast::Kind::PropertyDeclaration
                || !ast::has_static_modifier(source, *member)
                || ast::is_auto_accessor_property_declaration(source, *member)
            {
                continue;
            }
            let Some(name) = source.name(*member) else {
                continue;
            };
            if !ast::is_private_identifier(self.store_for(name), name) {
                continue;
            }
            let name_text = self.store_for(name).text(name);
            let generated_name_source = self
                .emit_context
                .has_auto_generate_info(Some(&name))
                .then(|| self.emit_context.get_node_for_generated_name(&name));
            let final_state = private_name_validities.get(&name_text).copied();
            let storage_name = self.create_hoisted_variable_for_private_name(class_name, name, "");
            fields.push(PrivateStaticFieldInfo {
                is_active: final_state.is_none_or(|state| {
                    state.kind == PrivateNameStateKind::Field && state.order == order
                }),
                is_static: true,
                is_auto_accessor_storage: false,
                is_valid: final_state.is_none_or(|state| state.is_valid),
                name_text,
                generated_name_source,
                order,
                storage_name,
            });
        }
        if fields.is_empty() {
            None
        } else {
            Some(PrivateStaticFieldEnvironment {
                brand_check_identifier,
                fields,
            })
        }
    }

    fn collect_private_instance_field_environments(
        &mut self,
        members: &[ast::Node],
        class_name: Option<ast::Node>,
    ) -> Vec<PrivateStaticFieldEnvironment> {
        if !self
            .config
            .should_transform_private_elements_or_class_static_blocks
        {
            return Vec::new();
        }
        let private_name_validities = self.private_name_validities(members);
        let mut environments = Vec::new();
        for (order, member) in members.iter().enumerate() {
            let source = self.store_for(*member);
            if source.kind(*member) != ast::Kind::PropertyDeclaration
                || ast::has_static_modifier(source, *member)
                || ast::is_auto_accessor_property_declaration(source, *member)
            {
                continue;
            }
            let Some(name) = source.name(*member) else {
                continue;
            };
            if !ast::is_private_identifier(self.store_for(name), name) {
                continue;
            }
            let name_text = self.store_for(name).text(name);
            let generated_name_source = self
                .emit_context
                .has_auto_generate_info(Some(&name))
                .then(|| self.emit_context.get_node_for_generated_name(&name));
            let final_state = private_name_validities.get(&name_text).copied();
            let storage_name = self.create_hoisted_variable_for_private_name(class_name, name, "");
            environments.push(PrivateStaticFieldEnvironment {
                brand_check_identifier: storage_name,
                fields: vec![PrivateStaticFieldInfo {
                    is_active: final_state.is_none_or(|state| {
                        state.kind == PrivateNameStateKind::Field && state.order == order
                    }),
                    is_static: false,
                    is_auto_accessor_storage: false,
                    is_valid: final_state.is_none_or(|state| state.is_valid),
                    name_text,
                    generated_name_source,
                    order,
                    storage_name,
                }],
            });
        }
        environments
    }

    fn collect_private_field_environments_in_member_order(
        &mut self,
        members: &[ast::Node],
        class_name: Option<ast::Node>,
        brand_check_identifier: Option<ast::Node>,
        force_transform_static_private_elements: bool,
    ) -> (
        Option<PrivateStaticFieldEnvironment>,
        Vec<PrivateStaticFieldEnvironment>,
    ) {
        let should_transform_static_fields = self
            .config
            .should_transform_private_elements_or_class_static_blocks
            || force_transform_static_private_elements;
        let should_transform_instance_fields = self
            .config
            .should_transform_private_elements_or_class_static_blocks;
        if !should_transform_static_fields && !should_transform_instance_fields {
            return (None, Vec::new());
        }

        let private_name_validities = self.private_name_validities(members);
        let mut static_fields = Vec::new();
        let mut instance_environments = Vec::new();
        for (order, member) in members.iter().enumerate() {
            let (kind, is_auto_accessor, is_static, name) = {
                let source = self.store_for(*member);
                (
                    source.kind(*member),
                    ast::is_auto_accessor_property_declaration(source, *member),
                    ast::has_static_modifier(source, *member),
                    source.name(*member),
                )
            };
            if kind != ast::Kind::PropertyDeclaration {
                continue;
            }
            let Some(name) = name else {
                continue;
            };
            if is_auto_accessor {
                let should_transform_auto_accessor_storage =
                    self.config.should_transform_auto_accessors
                        || self.current_class_will_hoist_initializers_to_constructor;
                if !should_transform_auto_accessor_storage
                    || (is_static && !should_transform_static_fields)
                    || (!is_static && !should_transform_instance_fields)
                {
                    continue;
                }
                let storage_name = self.create_auto_accessor_private_storage_name(members, name);
                if !ast::is_private_identifier(self.store_for(storage_name), storage_name) {
                    continue;
                }
                if is_static {
                    if brand_check_identifier.is_none() {
                        continue;
                    }
                    let name_text = self.store_for(storage_name).text(storage_name);
                    let generated_name_source = self
                        .emit_context
                        .has_auto_generate_info(Some(&storage_name))
                        .then(|| self.emit_context.get_node_for_generated_name(&storage_name));
                    let storage_variable =
                        self.create_hoisted_variable_for_private_name(class_name, storage_name, "");
                    static_fields.push(PrivateStaticFieldInfo {
                        is_active: true,
                        is_static: true,
                        is_auto_accessor_storage: true,
                        is_valid: true,
                        name_text,
                        generated_name_source,
                        order,
                        storage_name: storage_variable,
                    });
                } else {
                    let name_text = self.store_for(storage_name).text(storage_name);
                    let generated_name_source = self
                        .emit_context
                        .has_auto_generate_info(Some(&storage_name))
                        .then(|| self.emit_context.get_node_for_generated_name(&storage_name));
                    let storage_variable =
                        self.create_hoisted_variable_for_private_name(class_name, storage_name, "");
                    instance_environments.push(PrivateStaticFieldEnvironment {
                        brand_check_identifier: storage_variable,
                        fields: vec![PrivateStaticFieldInfo {
                            is_active: true,
                            is_static: false,
                            is_auto_accessor_storage: true,
                            is_valid: true,
                            name_text,
                            generated_name_source,
                            order,
                            storage_name: storage_variable,
                        }],
                    });
                }
                continue;
            }
            if !ast::is_private_identifier(self.store_for(name), name) {
                continue;
            }
            let name_text = self.store_for(name).text(name);
            let generated_name_source = self
                .emit_context
                .has_auto_generate_info(Some(&name))
                .then(|| self.emit_context.get_node_for_generated_name(&name));
            let final_state = private_name_validities.get(&name_text).copied();
            if is_static {
                if !should_transform_static_fields {
                    continue;
                }
                if brand_check_identifier.is_none() {
                    continue;
                }
                let storage_name =
                    self.create_hoisted_variable_for_private_name(class_name, name, "");
                static_fields.push(PrivateStaticFieldInfo {
                    is_active: final_state.is_none_or(|state| {
                        state.kind == PrivateNameStateKind::Field && state.order == order
                    }),
                    is_static: true,
                    is_auto_accessor_storage: false,
                    is_valid: final_state.is_none_or(|state| state.is_valid),
                    name_text,
                    generated_name_source,
                    order,
                    storage_name,
                });
            } else {
                if !should_transform_instance_fields {
                    continue;
                }
                let storage_name =
                    self.create_hoisted_variable_for_private_name(class_name, name, "");
                instance_environments.push(PrivateStaticFieldEnvironment {
                    brand_check_identifier: storage_name,
                    fields: vec![PrivateStaticFieldInfo {
                        is_active: final_state.is_none_or(|state| {
                            state.kind == PrivateNameStateKind::Field && state.order == order
                        }),
                        is_static: false,
                        is_auto_accessor_storage: false,
                        is_valid: final_state.is_none_or(|state| state.is_valid),
                        name_text,
                        generated_name_source,
                        order,
                        storage_name,
                    }],
                });
            }
        }

        let static_environment = brand_check_identifier.and_then(|brand_check_identifier| {
            (!static_fields.is_empty()).then_some(PrivateStaticFieldEnvironment {
                brand_check_identifier,
                fields: static_fields,
            })
        });
        (static_environment, instance_environments)
    }

    fn create_private_instance_accessor_environment(
        &mut self,
        private_accessors: Vec<ast::Node>,
        class_name: Option<ast::Node>,
        brand_check_identifier: ast::Node,
        private_name_validities: &HashMap<String, PrivateNameState>,
    ) -> Option<PrivateAccessorEnvironment> {
        let mut accessors: Vec<PrivateAccessorInfo> = Vec::new();
        for member in private_accessors {
            let source = self.store_for(member);
            let Some(name) = source.name(member) else {
                continue;
            };
            let name_text = source.text(name);
            let is_static = ast::has_static_modifier(source, member);
            let previous_entry_index = accessors
                .iter()
                .position(|info| info.name_text == name_text);
            let is_valid = private_name_validities
                .get(&name_text)
                .is_none_or(|state| state.is_valid);
            let entry_index = previous_entry_index.unwrap_or_else(|| {
                accessors.push(PrivateAccessorInfo {
                    name_text: name_text.clone(),
                    method_name: None,
                    getter_name: None,
                    setter_name: None,
                    is_valid,
                    is_static,
                });
                accessors.len() - 1
            });
            accessors[entry_index].is_valid = is_valid;
            if ast::is_auto_accessor_property_declaration(source, member) {
                accessors[entry_index].getter_name =
                    Some(self.create_hoisted_variable_for_private_name(class_name, name, "_get"));
                accessors[entry_index].setter_name =
                    Some(self.create_hoisted_variable_for_private_name(class_name, name, "_set"));
            } else if source.kind(member) == ast::Kind::MethodDeclaration {
                accessors[entry_index].method_name =
                    Some(self.create_hoisted_variable_for_private_name(class_name, name, ""));
            } else if source.kind(member) == ast::Kind::GetAccessor {
                accessors[entry_index].getter_name =
                    Some(self.create_hoisted_variable_for_private_name(class_name, name, "_get"));
            } else {
                accessors[entry_index].setter_name =
                    Some(self.create_hoisted_variable_for_private_name(class_name, name, "_set"));
            }
        }
        if accessors.is_empty() {
            None
        } else {
            Some(PrivateAccessorEnvironment {
                brand_check_identifier,
                accessors,
            })
        }
    }

    fn collect_private_instance_accessor_environment(
        &mut self,
        members: &[ast::Node],
        class_name: Option<ast::Node>,
    ) -> Option<PrivateAccessorEnvironment> {
        let private_accessors = self.private_accessors(members, false, false)?;
        let weak_set_name = self.create_hoisted_variable_for_class(class_name, "instances", "");
        let private_name_validities = self.private_name_validities(members);
        self.create_private_instance_accessor_environment(
            private_accessors,
            class_name,
            weak_set_name,
            &private_name_validities,
        )
    }

    fn collect_private_static_accessor_environment(
        &mut self,
        members: &[ast::Node],
        class_name: Option<ast::Node>,
        brand_check_identifier: ast::Node,
        force_transform_static_private_elements: bool,
    ) -> Option<PrivateAccessorEnvironment> {
        let private_accessors =
            self.private_accessors(members, true, force_transform_static_private_elements)?;
        let private_name_validities = self.private_name_validities(members);
        self.create_private_instance_accessor_environment(
            private_accessors,
            class_name,
            brand_check_identifier,
            &private_name_validities,
        )
    }

    fn private_accessor_info(&self, name: ast::Node) -> Option<&PrivateAccessorInfo> {
        let source = self.store_for(name);
        let name_text = source.text(name);
        self.private_accessor_stack.iter().rev().find_map(|env| {
            env.accessors
                .iter()
                .find(|info| info.name_text == name_text)
        })
    }

    fn private_accessor_info_for_member(
        &self,
        name: ast::Node,
        is_static: bool,
    ) -> Option<&PrivateAccessorInfo> {
        let source = self.store_for(name);
        let name_text = source.text(name);
        self.private_accessor_stack.iter().find_map(|env| {
            env.accessors
                .iter()
                .find(|info| info.name_text == name_text && info.is_static == is_static)
        })
    }

    fn private_accessor_environment(&self, name: ast::Node) -> Option<&PrivateAccessorEnvironment> {
        let source = self.store_for(name);
        let name_text = source.text(name);
        self.private_accessor_stack
            .iter()
            .rev()
            .find(|env| env.accessors.iter().any(|info| info.name_text == name_text))
    }

    fn private_static_field_info(&self, name: ast::Node) -> Option<&PrivateStaticFieldInfo> {
        let source = self.store_for(name);
        let name_text = source.text(name);
        let generated_name_source = self
            .emit_context
            .has_auto_generate_info(Some(&name))
            .then(|| self.emit_context.get_node_for_generated_name(&name));
        self.private_static_field_stack
            .iter()
            .flat_map(|env| env.fields.iter())
            .filter(|info| {
                if let Some(generated_name_source) = generated_name_source {
                    info.generated_name_source == Some(generated_name_source)
                } else {
                    info.is_active && info.name_text == name_text
                }
            })
            .max_by_key(|info| info.order)
    }

    fn private_static_field_environment(
        &self,
        name: ast::Node,
    ) -> Option<&PrivateStaticFieldEnvironment> {
        let source = self.store_for(name);
        let name_text = source.text(name);
        let generated_name_source = self
            .emit_context
            .has_auto_generate_info(Some(&name))
            .then(|| self.emit_context.get_node_for_generated_name(&name));
        self.private_static_field_stack.iter().rev().find(|env| {
            env.fields.iter().any(|info| {
                if let Some(generated_name_source) = generated_name_source {
                    info.generated_name_source == Some(generated_name_source)
                } else {
                    info.name_text == name_text
                }
            })
        })
    }

    fn reorder_private_name_variable_declarations(
        &mut self,
        members: &[ast::Node],
        auto_accessor_storage_names: &[(ast::Node, ast::Node)],
    ) {
        let mut ordered_names = Vec::new();
        let mut pending_private_auto_accessor_storage_names = Vec::new();
        let mut deferred_auto_accessor_storage_names = Vec::new();
        for member in members {
            let source = self.store_for(*member);
            let mut pending_auto_accessor_storage_name = None;
            if source.kind(*member) == ast::Kind::PropertyDeclaration
                && ast::is_auto_accessor_property_declaration(source, *member)
                && let Some(storage_name) =
                    auto_accessor_storage_names
                        .iter()
                        .find_map(|(storage_member, storage_name)| {
                            (*storage_member == *member).then_some(*storage_name)
                        })
                && !ordered_names.contains(&storage_name)
            {
                pending_auto_accessor_storage_name = Some(
                    if ast::is_private_identifier(self.store_for(storage_name), storage_name) {
                        self.private_static_field_info(storage_name)
                            .map(|info| info.storage_name)
                            .unwrap_or(storage_name)
                    } else {
                        storage_name
                    },
                );
            }
            if !ast::is_private_identifier_class_element_declaration(source, *member) {
                if let Some(storage_name) = pending_auto_accessor_storage_name
                    && !ordered_names.contains(&storage_name)
                {
                    pending_private_auto_accessor_storage_names.push(storage_name);
                }
                continue;
            }
            let Some(name) = source.name(*member) else {
                continue;
            };
            if !ast::is_private_identifier(self.store_for(name), name) {
                continue;
            }
            let is_static = ast::has_static_modifier(source, *member);
            match source.kind(*member) {
                ast::Kind::PropertyDeclaration => {
                    if ast::is_auto_accessor_property_declaration(source, *member) {
                        if let Some(info) = self.private_accessor_info_for_member(name, is_static) {
                            if let Some(getter_name) = info.getter_name
                                && !ordered_names.contains(&getter_name)
                            {
                                ordered_names.push(getter_name);
                            }
                            if let Some(setter_name) = info.setter_name
                                && !ordered_names.contains(&setter_name)
                            {
                                ordered_names.push(setter_name);
                            }
                        }
                        if let Some(storage_name) = pending_auto_accessor_storage_name
                            && !ordered_names.contains(&storage_name)
                        {
                            pending_private_auto_accessor_storage_names.push(storage_name);
                        }
                    } else if let Some(info) = self.private_static_field_info(name) {
                        if !ordered_names.contains(&info.storage_name) {
                            ordered_names.push(info.storage_name);
                        }
                    }
                }
                ast::Kind::MethodDeclaration => {
                    if let Some(method_name) = self
                        .private_accessor_info_for_member(name, is_static)
                        .and_then(|info| info.method_name)
                    {
                        if !ordered_names.contains(&method_name) {
                            ordered_names.push(method_name);
                        }
                    }
                }
                ast::Kind::GetAccessor => {
                    if let Some(getter_name) = self
                        .private_accessor_info_for_member(name, is_static)
                        .and_then(|info| info.getter_name)
                    {
                        if !ordered_names.contains(&getter_name) {
                            ordered_names.push(getter_name);
                        }
                    }
                }
                ast::Kind::SetAccessor => {
                    if let Some(setter_name) = self
                        .private_accessor_info_for_member(name, is_static)
                        .and_then(|info| info.setter_name)
                    {
                        if !ordered_names.contains(&setter_name) {
                            ordered_names.push(setter_name);
                        }
                    }
                }
                _ => {}
            }
        }
        for storage_name in pending_private_auto_accessor_storage_names {
            if !ordered_names.contains(&storage_name) {
                ordered_names.push(storage_name);
                deferred_auto_accessor_storage_names.push(storage_name);
            }
        }
        self.emit_context
            .reorder_current_variable_declarations(&ordered_names);
        self.emit_context
            .move_current_variable_declarations_to_end(&deferred_auto_accessor_storage_names);
    }

    fn invalid_private_field_info(&self, member: ast::Node) -> Option<&PrivateStaticFieldInfo> {
        let source = self.store_for(member);
        if source.kind(member) != ast::Kind::PropertyDeclaration
            || ast::is_auto_accessor_property_declaration(source, member)
        {
            return None;
        }
        let name = source.name(member)?;
        if !ast::is_private_identifier(self.store_for(name), name) {
            return None;
        }
        self.private_static_field_info(name)
            .filter(|info| !info.is_valid)
    }

    fn invalid_private_non_field(&self, member: ast::Node) -> bool {
        let source = self.store_for(member);
        if source.kind(member) != ast::Kind::PropertyDeclaration
            || ast::is_auto_accessor_property_declaration(source, member)
        {
            return false;
        }
        let Some(name) = source.name(member) else {
            return false;
        };
        ast::is_private_identifier(self.store_for(name), name)
            && self.private_static_field_info(name).is_none()
            && self
                .private_accessor_info(name)
                .is_some_and(|info| !info.is_valid)
    }

    fn create_weak_set_initializer(&mut self, weak_set_name: ast::Node) -> ast::Node {
        let weak_set = self.factory_mut().new_identifier("WeakSet");
        let arguments = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            Vec::<ast::Node>::new(),
        );
        let weak_set = self
            .factory_mut()
            .new_new_expression(weak_set, None, Some(arguments));
        self.emit_context
            .factory
            .new_assignment_expression(weak_set_name, weak_set)
    }

    fn create_brand_check_initializer(&mut self, weak_set_name: ast::Node) -> ast::Node {
        let add_name = self.factory_mut().new_identifier("add");
        let this = self.factory_mut().new_token(ast::Kind::ThisKeyword);
        let call = self
            .emit_context
            .factory
            .new_method_call(&weak_set_name, &add_name, &[this]);
        self.factory_mut().new_expression_statement(call)
    }

    fn coalesce_variable_environment_declarations(
        &mut self,
        declarations: Vec<ast::Node>,
    ) -> Vec<ast::Node> {
        let mut variable_declarations = Vec::new();
        let mut other_declarations = Vec::new();
        for declaration in declarations {
            let source = self.store_for(declaration);
            if source.kind(declaration) == ast::Kind::VariableStatement
                && let Some(declaration_list) = source.declaration_list(declaration)
                && let Some(declarations) = source.declarations(declaration_list)
            {
                variable_declarations.extend(declarations.iter());
            } else {
                other_declarations.push(declaration);
            }
        }
        if variable_declarations.is_empty() {
            return other_declarations;
        }
        let declarations = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            variable_declarations,
        );
        let variable_list = self
            .factory_mut()
            .new_variable_declaration_list(declarations, ast::NodeFlags::NONE);
        self.emit_context
            .set_emit_flags(&variable_list, printer::EF_NO_COMMENTS);
        let variable_statement = self
            .factory_mut()
            .new_variable_statement(None::<ast::ModifierList>, variable_list);
        self.emit_context.set_emit_flags(
            &variable_statement,
            printer::EF_CUSTOM_PROLOGUE | printer::EF_NO_COMMENTS,
        );
        other_declarations.insert(0, variable_statement);
        other_declarations
    }

    fn transform_private_accessor_declaration(&mut self, member: ast::Node) -> Option<ast::Node> {
        let (kind, name, modifiers, asterisk_token, parameters, body) = {
            let source = self.store_for(member);
            let parameters = source.source_parameters(member).map(|parameters| {
                (
                    parameters.loc(),
                    parameters.range(),
                    parameters.has_trailing_comma(),
                    parameters.iter().collect::<Vec<_>>(),
                )
            });
            (
                source.kind(member),
                source.name(member)?,
                self.snapshot_optional_modifier_list(source.source_modifiers(member)),
                source.asterisk_token(member),
                parameters,
                source.body(member),
            )
        };
        if !ast::is_private_identifier(self.store_for(name), name) {
            return None;
        }
        let info = self.private_accessor_info(name)?;
        if !info.is_valid {
            return None;
        }
        let function_name = if kind == ast::Kind::MethodDeclaration {
            info.method_name?
        } else if kind == ast::Kind::GetAccessor {
            info.getter_name?
        } else {
            info.setter_name?
        };
        let old_flags = self.emit_context.begin_visit_parameters();
        let (loc, range, has_trailing_comma, parameter_nodes) =
            parameters.expect("method or accessor parameters should be present");
        let mut visited = Vec::with_capacity(parameter_nodes.len());
        let mut changed = false;
        for parameter in parameter_nodes {
            let result = self.visit(&parameter);
            self.append_visited_node(parameter, result, &mut visited, &mut changed);
        }
        let (visited, _) = self
            .emit_context
            .finish_visit_parameters(old_flags, visited, changed);
        let parameters = self.factory_mut().new_node_list_with_trailing_comma(
            loc,
            range,
            visited,
            has_trailing_comma,
        );
        let modifiers = self.preserve_optional_modifier_list_snapshot_with_allowed(
            modifiers,
            !(ast::ModifierFlags::STATIC | ast::ModifierFlags::ACCESSOR),
        );
        let asterisk_token = asterisk_token.map(|node| self.preserve_node(node));
        let body = self.visit_function_body(body);
        let function = self.factory_mut().new_function_expression(
            modifiers,
            asterisk_token,
            Some(function_name),
            None::<ast::NodeList>,
            parameters,
            None::<ast::Node>,
            None::<ast::Node>,
            body,
        );
        let assignment = self
            .emit_context
            .factory
            .new_assignment_expression(function_name, function);
        Some(self.factory_mut().new_expression_statement(assignment))
    }

    fn transform_private_identifier_binary_expression(
        &mut self,
        node: ast::Node,
    ) -> Option<ast::Node> {
        let source = self.store_for(node);
        let node_loc = source.loc(node);
        let operator = source.operator_token(node)?;
        let operator_kind = self.store_for(operator).kind(operator);
        if operator_kind == ast::Kind::InKeyword {
            let left = source.left(node)?;
            if ast::is_private_identifier(self.store_for(left), left) {
                let right_node = source.right(node);
                let right = self.visit_node(right_node)?;
                let right = self.clone_node_for_reuse(right);
                if let Some(environment) = self.private_accessor_environment(left) {
                    let brand_check_identifier =
                        self.clone_node_for_reuse(environment.brand_check_identifier);
                    return Some(
                        self.emit_context
                            .factory
                            .new_class_private_field_in_helper(brand_check_identifier, right),
                    );
                }
                let brand_check_identifier = self
                    .private_static_field_environment(left)?
                    .brand_check_identifier;
                let brand_check_identifier = self.clone_node_for_reuse(brand_check_identifier);
                return Some(
                    self.emit_context
                        .factory
                        .new_class_private_field_in_helper(brand_check_identifier, right),
                );
            }
        }
        if operator_kind != ast::Kind::EqualsToken && !ast::is_compound_assignment(operator_kind) {
            return None;
        }
        let left = source.left(node)?;
        let left_source = self.store_for(left);
        let left = ast::skip_outer_expressions(
            left_source,
            left,
            ast::OuterExpressionKinds::PARENTHESES
                | ast::OuterExpressionKinds::PARTIALLY_EMITTED_EXPRESSIONS
                | ast::OuterExpressionKinds::TYPE_ASSERTIONS,
        );
        let left_source = self.store_for(left);
        if !ast::is_property_access_expression(left_source, left) {
            return None;
        }
        let name = left_source.name(left)?;
        let receiver_node = left_source.expression(left);
        let right_node = source.right(node);
        if !ast::is_private_identifier(self.store_for(name), name) {
            return None;
        }
        let mut receiver = self.visit_node(receiver_node)?;
        let mut value = self.visit_node(right_node)?;
        if ast::is_compound_assignment(operator_kind) {
            let (read_expression, initialize_expression) =
                self.create_copiable_receiver_expr(receiver);
            receiver = initialize_expression.unwrap_or(read_expression);
            let access = self.create_private_identifier_access_helper(name, read_expression)?;
            let operator =
                self.factory_mut()
                    .new_token(get_non_assignment_operator_for_compound_assignment(
                        operator_kind,
                    ));
            value = self
                .factory_mut()
                .new_binary_expression(None, access, None, operator, value);
        }
        self.create_private_identifier_assignment_helper(name, receiver, value, node, node_loc)
    }

    fn create_private_identifier_assignment_helper(
        &mut self,
        name: ast::Node,
        receiver: ast::Node,
        value: ast::Node,
        original: ast::Node,
        location: core::TextRange,
    ) -> Option<ast::Node> {
        let receiver = self.clone_node_for_reuse(receiver);
        let value = self.clone_node_for_reuse(value);
        let receiver_end = self.store_for(receiver).loc(receiver).end();
        self.emit_context
            .set_comment_range(&receiver, core::new_text_range(-1, receiver_end));
        let result = if let Some(environment) = self.private_accessor_environment(name) {
            let brand_check_identifier =
                self.clone_node_for_reuse(environment.brand_check_identifier);
            let info = self.private_accessor_info(name)?;
            if info.method_name.is_some() {
                self.emit_context
                    .factory
                    .new_class_private_field_set_helper(
                        receiver,
                        brand_check_identifier,
                        value,
                        printer::PrivateIdentifierKind::Method,
                        None,
                    )
            } else {
                let setter_name = info
                    .setter_name
                    .map(|setter_name| self.clone_node_for_reuse(setter_name));
                self.emit_context
                    .factory
                    .new_class_private_field_set_helper(
                        receiver,
                        brand_check_identifier,
                        value,
                        printer::PrivateIdentifierKind::Accessor,
                        setter_name,
                    )
            }
        } else {
            let brand_check_identifier = self
                .private_static_field_environment(name)?
                .brand_check_identifier;
            let storage_name = self.private_static_field_info(name)?.storage_name;
            let receiver = if brand_check_identifier != storage_name
                && self.should_substitute_this_for_static_generated_auto_accessor_storage(
                    name, receiver,
                ) {
                self.clone_node_for_reuse(brand_check_identifier)
            } else {
                receiver
            };
            let has_storage_argument = brand_check_identifier != storage_name;
            let brand_check_identifier = self.clone_node_for_reuse(brand_check_identifier);
            let storage_name = self.clone_node_for_reuse(storage_name);
            let storage_argument = has_storage_argument.then_some(storage_name);
            self.emit_context
                .factory
                .new_class_private_field_set_helper(
                    receiver,
                    brand_check_identifier,
                    value,
                    printer::PrivateIdentifierKind::Field,
                    storage_argument,
                )
        };
        self.emit_context.set_original(&result, &original);
        self.factory_mut()
            .place_emit_synthetic_node(result, location);
        Some(result)
    }

    fn create_copiable_receiver_expr(
        &mut self,
        receiver: ast::Node,
    ) -> (ast::Node, Option<ast::Node>) {
        let clone = self.clone_node_for_reuse(receiver);
        if self.is_simple_inlineable_expression(receiver) {
            return (clone, None);
        }
        let read_expression = self.emit_context.factory.new_temp_variable();
        self.emit_context.add_variable_declaration(read_expression);
        let initialize_expression = self
            .emit_context
            .factory
            .new_assignment_expression(read_expression, clone);
        (read_expression, Some(initialize_expression))
    }

    fn transform_private_identifier_update_expression(
        &mut self,
        node: ast::Node,
        result_is_discarded: bool,
    ) -> Option<ast::Node> {
        let (kind, operator, operand, node_loc) = {
            let source = self.store_for(node);
            let kind = source.kind(node);
            let operator = source.operator(node)?;
            if !matches!(
                operator,
                ast::Kind::PlusPlusToken | ast::Kind::MinusMinusToken
            ) {
                return None;
            }
            let operand = source.operand(node)?;
            let operand = ast::skip_parentheses(self.store_for(operand), operand);
            if !ast::is_property_access_expression(self.store_for(operand), operand) {
                return None;
            }
            (kind, operator, operand, source.loc(node))
        };
        let source = self.store_for(operand);
        let name = source.name(operand)?;
        if !ast::is_private_identifier(self.store_for(name), name) {
            return None;
        }
        if self.private_accessor_environment(name).is_none()
            && self.private_static_field_environment(name).is_none()
        {
            return None;
        }

        let receiver_node = source.expression(operand);
        let receiver = self.visit_node(receiver_node)?;
        let (read_expression, initialize_expression) = self.create_copiable_receiver_expr(receiver);
        let access = self.create_private_identifier_access_helper(name, read_expression)?;
        let temp = if kind == ast::Kind::PostfixUnaryExpression && !result_is_discarded {
            let temp = self.emit_context.factory.new_temp_variable();
            self.emit_context.add_variable_declaration(temp);
            Some(temp)
        } else {
            None
        };
        let mut expression = self.expand_pre_or_postfix_increment_or_decrement_expression(
            node, kind, operator, access, temp,
        );
        let receiver = initialize_expression.unwrap_or(read_expression);
        expression = self.create_private_identifier_assignment_helper(
            name, receiver, expression, node, node_loc,
        )?;
        self.emit_context.set_original(&expression, &node);
        self.factory_mut()
            .place_emit_synthetic_node(expression, node_loc);
        if let Some(temp) = temp {
            let temp = self.clone_node_for_reuse(temp);
            expression = self
                .emit_context
                .factory
                .new_comma_expression(expression, temp);
            self.emit_context
                .set_source_map_range(&expression, node_loc);
        }
        Some(expression)
    }

    fn transform_destructuring_assignment_expression(
        &mut self,
        node: ast::Node,
    ) -> Option<ast::Node> {
        let source = self.store_for(node);
        if !ast::is_assignment_expression(source, node, true) {
            return None;
        }
        let left = source.left(node)?;
        if !matches!(
            self.store_for(left).kind(left),
            ast::Kind::ObjectLiteralExpression | ast::Kind::ArrayLiteralExpression
        ) {
            return None;
        }
        let operator_token = source.operator_token(node)?;
        let right = source.right(node);
        let saved_pending_expressions = std::mem::take(&mut self.pending_expressions);
        let left = self.visit_destructuring_assignment_target(left);
        let right = self.visit_node(right);
        let operator_token = Some(self.preserve_node(operator_token));
        let updated = if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_binary_expression(
                node,
                None,
                left,
                None,
                operator_token,
                right,
            )
        } else {
            let source = self.source;
            let factory = &mut self.emit_context.factory.node_factory;
            factory.update_binary_expression_from_store(
                source,
                node,
                None,
                left,
                None,
                operator_token,
                right,
            )
        };
        if self.pending_expressions.is_empty() {
            self.pending_expressions = saved_pending_expressions;
            return Some(updated);
        }
        let mut expressions = std::mem::take(&mut self.pending_expressions);
        self.pending_expressions = saved_pending_expressions;
        expressions.push(updated);
        self.emit_context.factory.inline_expressions(&expressions)
    }

    fn visit_destructuring_assignment_target(&mut self, node: ast::Node) -> ast::Node {
        if matches!(
            self.store_for(node).kind(node),
            ast::Kind::ObjectLiteralExpression | ast::Kind::ArrayLiteralExpression
        ) {
            return self.visit_assignment_pattern(node);
        }
        if ast::is_property_access_expression(self.store_for(node), node)
            && let Some(name) = self.store_for(node).name(node)
            && ast::is_private_identifier(self.store_for(name), name)
        {
            return self.wrap_private_identifier_for_destructuring_target(node);
        }
        if self.config.should_transform_super_in_static_initializers
            && self.current_class_static_super_context.is_some()
            && ast::is_super_property(self.store_for(node), node)
        {
            return self.wrap_super_property_for_destructuring_target(node);
        }
        self.generated_visit_each_child(&node)
    }

    fn wrap_super_property_for_destructuring_target(&mut self, node: ast::Node) -> ast::Node {
        let Some(context) = self.current_class_static_super_context else {
            return self.generated_visit_each_child(&node);
        };
        let Some(name) = self.super_property_name_in_static_initializer(node) else {
            return self.generated_visit_each_child(&node);
        };
        let temp = self.emit_context.factory.new_temp_variable();
        let target = self.clone_node_for_reuse(context.super_class_reference);
        let receiver = self.clone_node_for_reuse(context.class_constructor);
        let set_expr = self
            .emit_context
            .factory
            .new_reflect_set_call(target, name, temp, receiver);
        self.emit_context
            .factory
            .new_assignment_target_wrapper(temp, set_expr)
    }

    fn wrap_private_identifier_for_destructuring_target(&mut self, node: ast::Node) -> ast::Node {
        let (name, receiver_node) = {
            let source = self.store_for(node);
            let Some(name) = source.name(node) else {
                return self.generated_visit_each_child(&node);
            };
            let Some(receiver_node) = source.expression(node) else {
                return self.generated_visit_each_child(&node);
            };
            (name, receiver_node)
        };
        let parameter = self.generated_name_for_node(node);
        let has_private_identifier_info = self.private_accessor_info(name).is_some()
            || self.private_static_field_info(name).is_some();
        if !has_private_identifier_info {
            return self.generated_visit_each_child(&node);
        }
        let receiver_source = self.store_for(receiver_node);
        let is_this_or_super_property = matches!(
            receiver_source.kind(receiver_node),
            ast::Kind::ThisKeyword | ast::Kind::SuperKeyword
        );
        let receiver = if is_this_or_super_property
            || !crate::utilities::is_simple_copiable_expression(receiver_source, &receiver_node)
        {
            let temp = self
                .emit_context
                .factory
                .new_temp_variable_ex(AutoGenerateOptions {
                    flags: GeneratedIdentifierFlags::RESERVED_IN_NESTED_SCOPES,
                    ..Default::default()
                });
            self.emit_context.add_variable_declaration(temp);
            let visited_receiver = self
                .visit_node(Some(receiver_node))
                .unwrap_or_else(|| self.preserve_node(receiver_node));
            self.pending_expressions.push(
                self.emit_context
                    .factory
                    .new_assignment_expression(temp, visited_receiver),
            );
            temp
        } else {
            self.visit_node(Some(receiver_node))
                .unwrap_or_else(|| self.preserve_node(receiver_node))
        };
        let assign_expr = self.create_private_identifier_assignment(name, receiver, parameter);
        self.emit_context
            .factory
            .new_assignment_target_wrapper(parameter, assign_expr)
    }

    fn create_private_identifier_assignment(
        &mut self,
        name: ast::Node,
        receiver: ast::Node,
        value: ast::Node,
    ) -> ast::Node {
        let receiver = self.clone_node_for_reuse(receiver);
        let value = self.clone_node_for_reuse(value);
        let receiver_end = self.store_for(receiver).loc(receiver).end();
        self.emit_context
            .set_comment_range(&receiver, core::new_text_range(-1, receiver_end));
        if let Some(environment) = self.private_accessor_environment(name) {
            let brand_check_identifier =
                self.clone_node_for_reuse(environment.brand_check_identifier);
            let Some(info) = self.private_accessor_info(name) else {
                return self.generated_visit_each_child(&name);
            };
            if info.method_name.is_some() {
                return self
                    .emit_context
                    .factory
                    .new_class_private_field_set_helper(
                        receiver,
                        brand_check_identifier,
                        value,
                        printer::PrivateIdentifierKind::Method,
                        None,
                    );
            }
            let setter_name = info
                .setter_name
                .map(|setter_name| self.clone_node_for_reuse(setter_name));
            return self
                .emit_context
                .factory
                .new_class_private_field_set_helper(
                    receiver,
                    brand_check_identifier,
                    value,
                    printer::PrivateIdentifierKind::Accessor,
                    setter_name,
                );
        }
        let Some(environment) = self.private_static_field_environment(name) else {
            return self.generated_visit_each_child(&name);
        };
        let brand_check_identifier = environment.brand_check_identifier;
        let Some(storage_name) = self
            .private_static_field_info(name)
            .map(|info| info.storage_name)
        else {
            return self.generated_visit_each_child(&name);
        };
        let receiver = if brand_check_identifier != storage_name
            && self
                .should_substitute_this_for_static_generated_auto_accessor_storage(name, receiver)
        {
            self.clone_node_for_reuse(brand_check_identifier)
        } else {
            receiver
        };
        let has_storage_argument = brand_check_identifier != storage_name;
        let brand_check_identifier = self.clone_node_for_reuse(brand_check_identifier);
        let storage_name = self.clone_node_for_reuse(storage_name);
        let storage_argument = has_storage_argument.then_some(storage_name);
        self.emit_context
            .factory
            .new_class_private_field_set_helper(
                receiver,
                brand_check_identifier,
                value,
                printer::PrivateIdentifierKind::Field,
                storage_argument,
            )
    }

    fn generated_name_for_node(&mut self, node: ast::Node) -> ast::Node {
        self.emit_context.new_generated_name_for_node(node)
    }

    fn visit_assignment_element(&mut self, node: ast::Node) -> ast::Node {
        // 13.15.5.5 RS: IteratorDestructuringAssignmentEvaluation
        //   AssignmentElement : DestructuringAssignmentTarget Initializer?
        //     ...
        //     4. If |Initializer| is present and _value_ is *undefined*, then
        //        a. If IsAnonymousFunctionDefinition(|Initializer|) and IsIdentifierRef of |DestructuringAssignmentTarget| are both *true*, then
        //           i. Let _v_ be ? NamedEvaluation of |Initializer| with argument _lref_.[[ReferencedName]].
        //     ...
        let (is_assignment_expression, left_node, right_node, operator_token_node) = {
            let source = self.store_for(node);
            (
                ast::is_assignment_expression(source, node, true),
                source.left(node),
                source.right(node),
                source.operator_token(node),
            )
        };
        if is_assignment_expression {
            let left = left_node.map(|left| self.visit_destructuring_assignment_target(left));
            let right = right_node.and_then(|right| self.visit_node(Some(right)));
            let operator_token =
                operator_token_node.map(|operator_token| self.preserve_node(operator_token));
            if node.store_id() == self.factory().store().store_id() {
                return self.factory_mut().update_binary_expression(
                    node,
                    None,
                    left,
                    None,
                    operator_token,
                    right,
                );
            }
            let source = self.source;
            let factory = &mut self.emit_context.factory.node_factory;
            return factory.update_binary_expression_from_store(
                source,
                node,
                None,
                left,
                None,
                operator_token,
                right,
            );
        }
        self.visit_destructuring_assignment_target(node)
    }

    fn visit_assignment_rest_element(&mut self, node: ast::Node) -> ast::Node {
        let source = self.store_for(node);
        let Some(expression) = source.expression(node) else {
            return self.generated_visit_each_child(&node);
        };
        if ast::is_left_hand_side_expression(self.store_for(expression), expression) {
            let expression = Some(self.visit_destructuring_assignment_target(expression));
            if node.store_id() == self.factory().store().store_id() {
                return self.factory_mut().update_spread_element(node, expression);
            }
            let source = self.source;
            let factory = &mut self.emit_context.factory.node_factory;
            return factory.update_spread_element_from_store(source, node, expression);
        }
        self.generated_visit_each_child(&node)
    }

    fn visit_array_assignment_element(&mut self, node: ast::Node) -> ast::Node {
        match self.store_for(node).kind(node) {
            ast::Kind::SpreadElement => self.visit_assignment_rest_element(node),
            ast::Kind::OmittedExpression => self.generated_visit_each_child(&node),
            ast::Kind::BindingElement
            | ast::Kind::ArrayLiteralExpression
            | ast::Kind::ObjectLiteralExpression
            | ast::Kind::Identifier
            | ast::Kind::PropertyAccessExpression
            | ast::Kind::ElementAccessExpression
            | ast::Kind::BinaryExpression => self.visit_assignment_element(node),
            _ => self.generated_visit_each_child(&node),
        }
    }

    fn visit_assignment_property(&mut self, node: ast::Node) -> ast::Node {
        // AssignmentProperty : PropertyName `:` AssignmentElement
        // AssignmentElement : DestructuringAssignmentTarget Initializer?
        //
        // 13.15.5.6 RS: KeyedDestructuringAssignmentEvaluation
        //   AssignmentElement : DestructuringAssignmentTarget Initializer?
        //     ...
        //     3. If |Initializer| is present and _v_ is *undefined*, then
        //        a. If IsAnonymousfunctionDefinition(|Initializer|) and IsIdentifierRef of |DestructuringAssignmentTarget| are both *true*, then
        //           i. Let _rhsValue_ be ? NamedEvaluation of |Initializer| with argument _lref_.[[ReferencedName]].
        //     ...
        let (name_node, initializer) = {
            let source = self.store_for(node);
            (source.name(node), source.initializer(node))
        };
        let name = name_node.and_then(|name| self.visit_node(Some(name)));
        let Some(initializer) = initializer else {
            return self.generated_visit_each_child(&node);
        };
        let initializer =
            if ast::is_assignment_expression(self.store_for(initializer), initializer, true) {
                Some(self.visit_assignment_element(initializer))
            } else if ast::is_left_hand_side_expression(self.store_for(initializer), initializer) {
                Some(self.visit_destructuring_assignment_target(initializer))
            } else {
                return self.generated_visit_each_child(&node);
            };
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut()
                .update_property_assignment(node, None, name, None, None, initializer)
        } else {
            let source = self.source;
            let factory = &mut self.emit_context.factory.node_factory;
            factory.update_property_assignment_from_store(
                source,
                node,
                None,
                name,
                None,
                None,
                initializer,
            )
        }
    }

    fn visit_assignment_rest_property(&mut self, node: ast::Node) -> ast::Node {
        let source = self.store_for(node);
        let Some(expression) = source.expression(node) else {
            return self.generated_visit_each_child(&node);
        };
        if ast::is_left_hand_side_expression(self.store_for(expression), expression) {
            let expression = Some(self.visit_destructuring_assignment_target(expression));
            if node.store_id() == self.factory().store().store_id() {
                return self
                    .factory_mut()
                    .update_spread_assignment(node, expression);
            }
            let source = self.source;
            let factory = &mut self.emit_context.factory.node_factory;
            return factory.update_spread_assignment_from_store(source, node, expression);
        }
        self.generated_visit_each_child(&node)
    }

    fn visit_object_assignment_element(&mut self, node: ast::Node) -> ast::Node {
        match self.store_for(node).kind(node) {
            ast::Kind::SpreadAssignment => self.visit_assignment_rest_property(node),
            ast::Kind::PropertyAssignment => self.visit_assignment_property(node),
            ast::Kind::ShorthandPropertyAssignment => self.generated_visit_each_child(&node),
            _ => self.generated_visit_each_child(&node),
        }
    }

    fn visit_assignment_pattern(&mut self, node: ast::Node) -> ast::Node {
        let is_array_literal_expression =
            ast::is_array_literal_expression(self.store_for(node), node);
        if is_array_literal_expression {
            // Transforms private names in destructuring assignment array bindings.
            // Transforms SuperProperty assignments in destructuring assignment array bindings in static initializers.
            //
            // Source:
            // ([ this.#myProp ] = [ "hello" ]);
            //
            // Transformation:
            // [ { set value(x) { this.#myProp = x; } }.value ] = [ "hello" ];
            let (elements, loc, range, multi_line) = {
                let source = self.store_for(node);
                let Some(elements) = source.elements(node) else {
                    return self.generated_visit_each_child(&node);
                };
                (
                    elements.iter().collect::<Vec<_>>(),
                    elements.loc(),
                    elements.range(),
                    source.multi_line(node).unwrap_or(false),
                )
            };
            let visited = elements
                .into_iter()
                .map(|element| self.visit_array_assignment_element(element))
                .collect::<Vec<_>>();
            let element_list = self.factory_mut().new_node_list(loc, range, visited);
            if node.store_id() == self.factory().store().store_id() {
                return self.factory_mut().update_array_literal_expression(
                    node,
                    element_list,
                    multi_line,
                );
            }
            let source = self.source;
            let factory = &mut self.emit_context.factory.node_factory;
            return factory.update_array_literal_expression_from_store(
                source,
                node,
                element_list,
                multi_line,
            );
        }
        // Transforms private names in destructuring assignment object bindings.
        // Transforms SuperProperty assignments in destructuring assignment object bindings in static initializers.
        //
        // Source:
        // ({ stringProperty: this.#myProp } = { stringProperty: "hello" });
        //
        // Transformation:
        // ({ stringProperty: { set value(x) { this.#myProp = x; } }.value }) = { stringProperty: "hello" };
        let (properties, loc, range, multi_line) = {
            let source = self.store_for(node);
            let Some(properties) = source.properties(node) else {
                return self.generated_visit_each_child(&node);
            };
            (
                properties.iter().collect::<Vec<_>>(),
                properties.loc(),
                properties.range(),
                source.multi_line(node).unwrap_or(false),
            )
        };
        let visited = properties
            .into_iter()
            .map(|property| self.visit_object_assignment_element(property))
            .collect::<Vec<_>>();
        let property_list = self.factory_mut().new_node_list(loc, range, visited);
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut()
                .update_object_literal_expression(node, property_list, multi_line)
        } else {
            let source = self.source;
            let factory = &mut self.emit_context.factory.node_factory;
            factory.update_object_literal_expression_from_store(
                source,
                node,
                property_list,
                multi_line,
            )
        }
    }

    fn transform_private_identifier_property_access(
        &mut self,
        node: ast::Node,
    ) -> Option<ast::Node> {
        let source = self.store_for(node);
        let name = source.name(node)?;
        if !ast::is_private_identifier(self.store_for(name), name) {
            return None;
        }
        let receiver_node = source.expression(node);
        let receiver = self.visit_node(receiver_node)?;
        let receiver = self.clone_node_for_reuse(receiver);
        self.create_private_identifier_access_helper(name, receiver)
    }

    fn create_private_identifier_access_helper(
        &mut self,
        name: ast::Node,
        receiver: ast::Node,
    ) -> Option<ast::Node> {
        let receiver_end = self.store_for(receiver).loc(receiver).end();
        self.emit_context
            .set_comment_range(&receiver, core::new_text_range(-1, receiver_end));
        if let Some(environment) = self.private_accessor_environment(name) {
            let brand_check_identifier =
                self.clone_node_for_reuse(environment.brand_check_identifier);
            let info = self.private_accessor_info(name)?;
            if let Some(method_name) = info.method_name {
                let method_name = self.clone_node_for_reuse(method_name);
                return Some(
                    self.emit_context
                        .factory
                        .new_class_private_field_get_helper(
                            receiver,
                            brand_check_identifier,
                            printer::PrivateIdentifierKind::Method,
                            Some(method_name),
                        ),
                );
            }
            let getter_name = info
                .getter_name
                .map(|getter_name| self.clone_node_for_reuse(getter_name));
            return Some(
                self.emit_context
                    .factory
                    .new_class_private_field_get_helper(
                        receiver,
                        brand_check_identifier,
                        printer::PrivateIdentifierKind::Accessor,
                        getter_name,
                    ),
            );
        }
        let brand_check_identifier = self
            .private_static_field_environment(name)?
            .brand_check_identifier;
        let storage_name = self.private_static_field_info(name)?.storage_name;
        let receiver = if brand_check_identifier != storage_name
            && self
                .should_substitute_this_for_static_generated_auto_accessor_storage(name, receiver)
        {
            self.clone_node_for_reuse(brand_check_identifier)
        } else {
            receiver
        };
        let has_storage_argument = brand_check_identifier != storage_name;
        let brand_check_identifier = self.clone_node_for_reuse(brand_check_identifier);
        let storage_name = self.clone_node_for_reuse(storage_name);
        let storage_argument = has_storage_argument.then_some(storage_name);
        Some(
            self.emit_context
                .factory
                .new_class_private_field_get_helper(
                    receiver,
                    brand_check_identifier,
                    printer::PrivateIdentifierKind::Field,
                    storage_argument,
                ),
        )
    }

    fn create_call_binding(&mut self, node: ast::Node) -> (ast::Node, ast::Node) {
        if ast::is_super_property(self.store_for(node), node) {
            return (self.factory_mut().new_token(ast::Kind::ThisKeyword), node);
        }
        if ast::is_property_access_expression(self.store_for(node), node) {
            let (expression, name) = {
                let source = self.store_for(node);
                (source.expression(node), source.name(node))
            };
            if let Some(expression) = expression {
                if !self.should_be_captured_in_temp_variable(expression) {
                    return (expression, node);
                }
                let this_arg = self.emit_context.factory.new_temp_variable();
                self.emit_context.add_variable_declaration(this_arg);
                let assignment = self
                    .emit_context
                    .factory
                    .new_assignment_expression(this_arg, expression);
                let parenthesized = self.factory_mut().new_parenthesized_expression(assignment);
                if let Some(name) = name {
                    let target = self.factory_mut().new_property_access_expression(
                        parenthesized,
                        None,
                        name,
                        ast::NodeFlags::NONE,
                    );
                    return (this_arg, target);
                }
            }
        }
        (self.emit_context.factory.new_void_zero_expression(), node)
    }

    fn should_be_captured_in_temp_variable(&self, node: ast::Node) -> bool {
        !matches!(
            self.store_for(ast::skip_parentheses(self.store_for(node), node))
                .kind(ast::skip_parentheses(self.store_for(node), node)),
            ast::Kind::Identifier
                | ast::Kind::ThisKeyword
                | ast::Kind::NumericLiteral
                | ast::Kind::BigIntLiteral
                | ast::Kind::StringLiteral
        )
    }

    fn is_same_identifier_text(&self, left: ast::Node, right: ast::Node) -> bool {
        ast::is_identifier(self.store_for(left), left)
            && ast::is_identifier(self.store_for(right), right)
            && self.store_for(left).text(left) == self.store_for(right).text(right)
    }

    fn transform_private_identifier_call_expression(
        &mut self,
        node: ast::Node,
    ) -> Option<ast::Node> {
        let (expression, question_dot_token, flags, argument_nodes, accessor_brand) = {
            let source = self.store_for(node);
            let expression = source.expression(node)?;
            if !ast::is_property_access_expression(self.store_for(expression), expression) {
                return None;
            }
            let name = self.store_for(expression).name(expression)?;
            let accessor_brand = self
                .private_accessor_environment(name)
                .map(|env| env.brand_check_identifier);
            if !ast::is_private_identifier(self.store_for(name), name)
                || (accessor_brand.is_none() && self.private_static_field_info(name).is_none())
            {
                return None;
            }
            let argument_nodes = source
                .source_arguments(node)
                .map(|arguments| arguments.iter().collect::<Vec<_>>())
                .unwrap_or_default();
            (
                expression,
                source.question_dot_token(node),
                source.flags(node),
                argument_nodes,
                accessor_brand,
            )
        };
        let question_dot_token = question_dot_token.map(|token| self.preserve_node(token));
        let (this_arg, target) = self.create_call_binding(expression);
        let visited_target = self.visit_node(Some(target))?;
        let visited_this_arg = self.visit_node(Some(this_arg))?;
        let preserve_this_arg_comments = accessor_brand
            .is_some_and(|brand| self.is_same_identifier_text(visited_this_arg, brand));
        if !preserve_this_arg_comments {
            self.emit_context
                .mark_emit_node(&visited_this_arg, printer::EF_NO_COMMENTS);
        }
        let visited_this_arg = self.clone_node_for_reuse(visited_this_arg);
        let arguments = argument_nodes
            .into_iter()
            .map(|argument| self.visit_node(Some(argument)).unwrap_or(argument))
            .collect::<Vec<_>>();
        let call_name = self.factory_mut().new_identifier("call");
        if flags.contains(ast::NodeFlags::OPTIONAL_CHAIN) {
            let call_target = self.factory_mut().new_property_access_expression(
                visited_target,
                question_dot_token,
                call_name,
                ast::NodeFlags::OPTIONAL_CHAIN,
            );
            let mut all_args = Vec::with_capacity(1 + arguments.len());
            all_args.push(visited_this_arg);
            all_args.extend(arguments);
            let all_args = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                all_args,
            );
            let updated = if node.store_id() == self.factory().store().store_id() {
                self.factory_mut().update_call_expression(
                    node,
                    call_target,
                    None::<ast::Node>,
                    None::<ast::NodeList>,
                    all_args,
                    flags,
                )
            } else {
                let source = self.source;
                self.factory_mut().update_call_expression_from_store(
                    source,
                    node,
                    call_target,
                    None::<ast::Node>,
                    None::<ast::NodeList>,
                    all_args,
                    flags,
                )
            };
            return Some(updated);
        }
        let call_target = self.factory_mut().new_property_access_expression(
            visited_target,
            None::<ast::Node>,
            call_name,
            ast::NodeFlags::NONE,
        );
        let mut all_args = Vec::with_capacity(1 + arguments.len());
        all_args.push(visited_this_arg);
        all_args.extend(arguments);
        let all_args = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            all_args,
        );
        let updated = if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_call_expression(
                node,
                call_target,
                None::<ast::Node>,
                None::<ast::NodeList>,
                all_args,
                flags,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_call_expression_from_store(
                source,
                node,
                call_target,
                None::<ast::Node>,
                None::<ast::NodeList>,
                all_args,
                flags,
            )
        };
        Some(updated)
    }

    fn transform_private_identifier_tagged_template_expression(
        &mut self,
        node: ast::Node,
    ) -> Option<ast::Node> {
        let (tag, template, flags) = {
            let source = self.store_for(node);
            let tag = source.tag(node)?;
            if !ast::is_property_access_expression(self.store_for(tag), tag) {
                return None;
            }
            let name = self.store_for(tag).name(tag)?;
            if !ast::is_private_identifier(self.store_for(name), name)
                || (self.private_accessor_environment(name).is_none()
                    && self.private_static_field_info(name).is_none())
            {
                return None;
            }
            (tag, source.template(node)?, source.flags(node))
        };
        let (this_arg, target) = self.create_call_binding(tag);
        let visited_target = self.visit_node(Some(target))?;
        let visited_this_arg = self.visit_node(Some(this_arg))?;
        let visited_this_arg = self.clone_node_for_reuse(visited_this_arg);
        let bind_name = self.factory_mut().new_identifier("bind");
        let bind_target = self.factory_mut().new_property_access_expression(
            visited_target,
            None::<ast::Node>,
            bind_name,
            ast::NodeFlags::NONE,
        );
        let bind_arguments = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![visited_this_arg],
        );
        let bind_expr = self.factory_mut().new_call_expression(
            bind_target,
            None::<ast::Node>,
            None::<ast::NodeList>,
            bind_arguments,
            ast::NodeFlags::NONE,
        );
        let template = self.visit_node(Some(template));
        let updated = if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_tagged_template_expression(
                node,
                bind_expr,
                None::<ast::Node>,
                None::<ast::NodeList>,
                template,
                flags,
            )
        } else {
            let source = self.source;
            self.factory_mut()
                .update_tagged_template_expression_from_store(
                    source,
                    node,
                    bind_expr,
                    None::<ast::Node>,
                    None::<ast::NodeList>,
                    template,
                    flags,
                )
        };
        Some(updated)
    }

    fn transform_super_property_access_in_static_initializer(
        &mut self,
        node: ast::Node,
    ) -> Option<ast::Node> {
        if !self.config.should_transform_super_in_static_initializers {
            return None;
        }
        if self.current_legacy_decorated_static_initializer
            && ast::is_super_property(self.store_for(node), node)
        {
            return self.visit_invalid_super_property(node);
        }
        let context = self.current_class_static_super_context?;
        let (kind, name, argument, expression, expression_loc) = {
            let source = self.store_for(node);
            if !ast::is_super_property(source, node) {
                return None;
            }
            let expression = source.expression(node);
            (
                source.kind(node),
                source.name(node),
                source.argument_expression(node),
                expression,
                expression.map(|expression| source.loc(expression)),
            )
        };
        match kind {
            ast::Kind::PropertyAccessExpression => {
                let name = name?;
                if !ast::is_identifier(self.store_for(name), name) {
                    return None;
                }
                let name_text = self.store_for(name).text(name);
                let property_key = self
                    .factory_mut()
                    .new_string_literal(&name_text, ast::TokenFlags::NONE);
                let target = self.clone_node_for_reuse(context.super_class_reference);
                let receiver = self.clone_node_for_reuse(context.class_constructor);
                // converts `super.x` into `Reflect.get(_baseTemp, "x", _classTemp)`
                let super_property =
                    self.emit_context
                        .factory
                        .new_reflect_get_call(target, property_key, receiver);
                if let Some(expression) = expression {
                    self.emit_context.set_original(&super_property, &expression);
                    if let Some(expression_loc) = expression_loc {
                        self.emit_context
                            .set_source_map_range(&super_property, expression_loc);
                    }
                }
                Some(super_property)
            }
            ast::Kind::ElementAccessExpression => {
                let argument = argument?;
                let property_key = self.visit_node(Some(argument))?;
                let target = self.clone_node_for_reuse(context.super_class_reference);
                let receiver = self.clone_node_for_reuse(context.class_constructor);
                // converts `super[x]` into `Reflect.get(_baseTemp, x, _classTemp)`
                let super_property =
                    self.emit_context
                        .factory
                        .new_reflect_get_call(target, property_key, receiver);
                if let Some(expression) = expression {
                    self.emit_context.set_original(&super_property, &expression);
                    if let Some(expression_loc) = expression_loc {
                        self.emit_context
                            .set_source_map_range(&super_property, expression_loc);
                    }
                }
                Some(super_property)
            }
            _ => None,
        }
    }

    fn visit_invalid_super_property(&mut self, node: ast::Node) -> Option<ast::Node> {
        let invalid_super = self.decorated_static_invalid_this();
        let source = self.store_for(node);
        match source.kind(node) {
            ast::Kind::PropertyAccessExpression => {
                let name = source.name(node)?;
                if node.store_id() == self.factory().store().store_id() {
                    Some(self.factory_mut().update_property_access_expression(
                        node,
                        invalid_super,
                        None::<ast::Node>,
                        name,
                        ast::NodeFlags::NONE,
                    ))
                } else {
                    let source = self.source;
                    Some(
                        self.factory_mut()
                            .update_property_access_expression_from_store(
                                source,
                                node,
                                invalid_super,
                                None::<ast::Node>,
                                name,
                                ast::NodeFlags::NONE,
                            ),
                    )
                }
            }
            ast::Kind::ElementAccessExpression => {
                let argument = source.argument_expression(node);
                if node.store_id() == self.factory().store().store_id() {
                    Some(self.factory_mut().update_element_access_expression(
                        node,
                        invalid_super,
                        None::<ast::Node>,
                        argument,
                        ast::NodeFlags::NONE,
                    ))
                } else {
                    let source = self.source;
                    Some(
                        self.factory_mut()
                            .update_element_access_expression_from_store(
                                source,
                                node,
                                invalid_super,
                                None::<ast::Node>,
                                argument,
                                ast::NodeFlags::NONE,
                            ),
                    )
                }
            }
            _ => None,
        }
    }

    fn transform_super_property_assignment_in_static_initializer(
        &mut self,
        node: ast::Node,
    ) -> Option<ast::Node> {
        self.transform_super_property_assignment_in_static_initializer_with_discard(node, false)
    }

    fn transform_super_property_assignment_in_static_initializer_with_discard(
        &mut self,
        node: ast::Node,
        result_is_discarded: bool,
    ) -> Option<ast::Node> {
        if !self.config.should_transform_super_in_static_initializers {
            return None;
        }
        let context = self.current_class_static_super_context?;
        let (left, right, operator_kind, node_loc) = {
            let source = self.store_for(node);
            let operator = source.operator_token(node)?;
            let operator_kind = self.store_for(operator).kind(operator);
            if operator_kind != ast::Kind::EqualsToken
                && !ast::is_compound_assignment(operator_kind)
            {
                return None;
            }
            let left = source.left(node)?;
            if !ast::is_super_property(self.store_for(left), left) {
                return None;
            }
            (left, source.right(node)?, operator_kind, source.loc(node))
        };

        let mut setter_name = self.super_property_name_in_static_initializer(left)?;
        let mut expression = self.visit_node(Some(right))?;

        if ast::is_compound_assignment(operator_kind) {
            let getter_name = if !self.is_simple_inlineable_expression(setter_name) {
                let temp = self.emit_context.factory.new_temp_variable();
                self.emit_context.add_variable_declaration(temp);
                setter_name = self
                    .emit_context
                    .factory
                    .new_assignment_expression(temp, setter_name);
                temp
            } else {
                self.clone_node_for_reuse(setter_name)
            };
            let target = self.clone_node_for_reuse(context.super_class_reference);
            let receiver = self.clone_node_for_reuse(context.class_constructor);
            let super_property_get =
                self.emit_context
                    .factory
                    .new_reflect_get_call(target, getter_name, receiver);
            self.emit_context.set_original(&super_property_get, &left);
            self.emit_context
                .set_source_map_range(&super_property_get, self.store_for(left).loc(left));
            let operator =
                self.factory_mut()
                    .new_token(get_non_assignment_operator_for_compound_assignment(
                        operator_kind,
                    ));
            expression = self.factory_mut().new_binary_expression(
                None,
                super_property_get,
                None,
                operator,
                expression,
            );
            self.emit_context
                .set_source_map_range(&expression, node_loc);
        }

        let temp = if result_is_discarded {
            None
        } else {
            let temp = self.emit_context.factory.new_temp_variable();
            self.emit_context.add_variable_declaration(temp);
            expression = self
                .emit_context
                .factory
                .new_assignment_expression(temp, expression);
            self.emit_context
                .set_source_map_range(&expression, node_loc);
            Some(temp)
        };

        let target = self.clone_node_for_reuse(context.super_class_reference);
        let receiver = self.clone_node_for_reuse(context.class_constructor);
        // converts `super.x = 1` into `(Reflect.set(_baseTemp, "x", _a = 1, _classTemp), _a)`
        // converts `super[f()] = 1` into `(Reflect.set(_baseTemp, f(), _a = 1, _classTemp), _a)`
        // converts `super.x += 1` into `(Reflect.set(_baseTemp, "x", _a = Reflect.get(_baseTemp, "x", _classtemp) + 1, _classTemp), _a)`
        // converts `super[f()] += 1` into `(Reflect.set(_baseTemp, _a = f(), _b = Reflect.get(_baseTemp, _a, _classtemp) + 1, _classTemp), _b)`
        let expression = self.emit_context.factory.new_reflect_set_call(
            target,
            setter_name,
            expression,
            receiver,
        );
        self.emit_context.set_original(&expression, &node);
        self.emit_context
            .set_source_map_range(&expression, node_loc);
        if let Some(temp) = temp {
            let temp = self.clone_node_for_reuse(temp);
            let expression = self
                .emit_context
                .factory
                .new_comma_expression(expression, temp);
            self.emit_context
                .set_source_map_range(&expression, node_loc);
            Some(expression)
        } else {
            Some(expression)
        }
    }

    fn transform_super_property_update_in_static_initializer(
        &mut self,
        node: ast::Node,
        result_is_discarded: bool,
    ) -> Option<ast::Node> {
        if !self.config.should_transform_super_in_static_initializers {
            return None;
        }
        let context = self.current_class_static_super_context?;
        let (kind, operator, operand, node_loc) = {
            let source = self.store_for(node);
            let kind = source.kind(node);
            let operator = source.operator(node)?;
            if !matches!(
                operator,
                ast::Kind::PlusPlusToken | ast::Kind::MinusMinusToken
            ) {
                return None;
            }
            let operand = source.operand(node)?;
            let operand = ast::skip_parentheses(self.store_for(operand), operand);
            if !ast::is_super_property(self.store_for(operand), operand) {
                return None;
            }
            (kind, operator, operand, source.loc(node))
        };

        let setter_name;
        let getter_name;
        if ast::is_element_access_expression(self.store_for(operand), operand) {
            let argument = self.store_for(operand).argument_expression(operand)?;
            if self.is_simple_inlineable_expression(argument) {
                getter_name = self.preserve_node(argument);
                setter_name = self.clone_node_for_reuse(getter_name);
            } else {
                getter_name = self.emit_context.factory.new_temp_variable();
                self.emit_context.add_variable_declaration(getter_name);
                let visited_argument = self.visit_node(Some(argument))?;
                setter_name = self
                    .emit_context
                    .factory
                    .new_assignment_expression(getter_name, visited_argument);
            }
        } else {
            setter_name = self.super_property_name_in_static_initializer(operand)?;
            getter_name = self.clone_node_for_reuse(setter_name);
        }

        let target = self.clone_node_for_reuse(context.super_class_reference);
        let receiver = self.clone_node_for_reuse(context.class_constructor);
        let mut expression =
            self.emit_context
                .factory
                .new_reflect_get_call(target, getter_name, receiver);
        self.emit_context.set_original(&expression, &operand);
        self.emit_context
            .set_source_map_range(&expression, self.store_for(operand).loc(operand));

        let temp = if result_is_discarded {
            None
        } else {
            let temp = self.emit_context.factory.new_temp_variable();
            self.emit_context.add_variable_declaration(temp);
            Some(temp)
        };
        expression = self.expand_pre_or_postfix_increment_or_decrement_expression(
            node, kind, operator, expression, temp,
        );

        let target = self.clone_node_for_reuse(context.super_class_reference);
        let receiver = self.clone_node_for_reuse(context.class_constructor);
        expression = self.emit_context.factory.new_reflect_set_call(
            target,
            setter_name,
            expression,
            receiver,
        );
        self.emit_context.set_original(&expression, &node);
        self.emit_context
            .set_source_map_range(&expression, node_loc);

        if let Some(temp) = temp {
            let temp = self.clone_node_for_reuse(temp);
            expression = self
                .emit_context
                .factory
                .new_comma_expression(expression, temp);
            self.emit_context
                .set_source_map_range(&expression, node_loc);
        }
        Some(expression)
    }

    fn expand_pre_or_postfix_increment_or_decrement_expression(
        &mut self,
        node: ast::Node,
        kind: ast::Kind,
        operator: ast::Kind,
        expression: ast::Node,
        temp: Option<ast::Node>,
    ) -> ast::Node {
        let node_loc = self.store_for(node).loc(node);
        let operand_loc = self
            .store_for(node)
            .operand(node)
            .map(|operand| self.store_for(operand).loc(operand))
            .unwrap_or(node_loc);
        let value = self.emit_context.factory.new_temp_variable();
        self.emit_context.add_variable_declaration(value);
        let mut expression = self
            .emit_context
            .factory
            .new_assignment_expression(value, expression);
        self.emit_context
            .set_source_map_range(&expression, operand_loc);
        let value_for_update = self.clone_node_for_reuse(value);
        let mut operation = if kind == ast::Kind::PrefixUnaryExpression {
            self.factory_mut()
                .new_prefix_unary_expression(operator, value_for_update)
        } else {
            self.factory_mut()
                .new_postfix_unary_expression(value_for_update, operator)
        };
        self.factory_mut()
            .place_emit_synthetic_node(operation, node_loc);
        if let Some(temp) = temp {
            operation = self
                .emit_context
                .factory
                .new_assignment_expression(temp, operation);
            self.emit_context.set_source_map_range(&operation, node_loc);
        }
        expression = self
            .emit_context
            .factory
            .new_comma_expression(expression, operation);
        self.emit_context
            .set_source_map_range(&expression, node_loc);
        if kind == ast::Kind::PostfixUnaryExpression {
            let value = self.clone_node_for_reuse(value);
            expression = self
                .emit_context
                .factory
                .new_comma_expression(expression, value);
            self.emit_context
                .set_source_map_range(&expression, node_loc);
        }
        expression
    }

    fn transform_super_property_call_in_static_initializer(
        &mut self,
        node: ast::Node,
    ) -> Option<ast::Node> {
        if !self.config.should_transform_super_in_static_initializers {
            return None;
        }
        let context = self.current_class_static_super_context?;
        let (expression, argument_nodes, node_loc) = {
            let source = self.store_for(node);
            let expression = source.expression(node)?;
            if !ast::is_super_property(self.store_for(expression), expression) {
                return None;
            }
            let argument_nodes = source
                .source_arguments(node)
                .map(|arguments| arguments.iter().collect::<Vec<_>>())
                .unwrap_or_default();
            (expression, argument_nodes, source.loc(node))
        };

        // super.x()
        // super[x]()

        let visited_target = self.visit_node(Some(expression))?;
        let this_arg = self.clone_node_for_reuse(context.class_constructor);
        let arguments = argument_nodes
            .into_iter()
            .map(|argument| self.visit_node(Some(argument)).unwrap_or(argument))
            .collect::<Vec<_>>();
        // converts `super.f(...)` into `Reflect.get(_baseTemp, "f", _classTemp).call(_classTemp, ...)`
        let invocation = self.emit_context.factory.new_function_call_call(
            &visited_target,
            Some(&this_arg),
            &arguments,
        );
        self.emit_context.set_original(&invocation, &node);
        self.emit_context
            .set_source_map_range(&invocation, node_loc);
        Some(invocation)
    }

    fn transform_super_property_tagged_template_in_static_initializer(
        &mut self,
        node: ast::Node,
    ) -> Option<ast::Node> {
        if !self.config.should_transform_super_in_static_initializers {
            return None;
        }
        let context = self.current_class_static_super_context?;
        let (tag, template, node_loc, flags) = {
            let source = self.store_for(node);
            let tag = source.tag(node)?;
            if !ast::is_super_property(self.store_for(tag), tag) {
                return None;
            }
            (
                tag,
                source.template(node)?,
                source.loc(node),
                source.flags(node),
            )
        };

        let visited_tag = self.visit_node(Some(tag))?;
        let this_arg = self.clone_node_for_reuse(context.class_constructor);
        // converts `` super.f`x` `` into `` Reflect.get(_baseTemp, "f", _classTemp).bind(_classTemp)`x` ``
        let invocation =
            self.emit_context
                .factory
                .new_function_bind_call(visited_tag, this_arg, &[]);
        self.emit_context.set_original(&invocation, &node);
        self.emit_context
            .set_source_map_range(&invocation, node_loc);
        let template = self.visit_node(Some(template));
        let updated = if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_tagged_template_expression(
                node,
                invocation,
                None::<ast::Node>,
                None::<ast::NodeList>,
                template,
                flags,
            )
        } else {
            let source = self.source;
            self.factory_mut()
                .update_tagged_template_expression_from_store(
                    source,
                    node,
                    invocation,
                    None::<ast::Node>,
                    None::<ast::NodeList>,
                    template,
                    flags,
                )
        };
        Some(updated)
    }

    fn super_property_name_in_static_initializer(&mut self, node: ast::Node) -> Option<ast::Node> {
        let (kind, name, argument) = {
            let source = self.store_for(node);
            (
                source.kind(node),
                source.name(node),
                source.argument_expression(node),
            )
        };
        match kind {
            ast::Kind::PropertyAccessExpression => {
                let name = name?;
                if !ast::is_identifier(self.store_for(name), name) {
                    return None;
                }
                let name_text = self.store_for(name).text(name);
                Some(
                    self.factory_mut()
                        .new_string_literal(&name_text, ast::TokenFlags::NONE),
                )
            }
            ast::Kind::ElementAccessExpression => {
                let argument = argument?;
                self.visit_node(Some(argument))
            }
            _ => None,
        }
    }

    fn is_simple_inlineable_expression(&self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        module_transform_utilities::is_simple_inlineable_expression(
            source.kind(node),
            ast::is_identifier(source, node),
        )
    }

    fn precompute_nonstatic_auto_accessor_storage_names(
        &mut self,
        members: &[ast::Node],
        _storage_name_class_name: Option<ast::Node>,
        should_transform_auto_accessors: bool,
        private_names_only: bool,
        auto_accessor_storage_names: &mut Vec<(ast::Node, ast::Node)>,
        pending_auto_accessor_storage_expressions: &mut Vec<ast::Node>,
    ) -> bool {
        if !should_transform_auto_accessors {
            return false;
        }
        let mut changed = false;
        for member in members {
            if auto_accessor_storage_names
                .iter()
                .any(|(storage_member, _)| storage_member == member)
            {
                continue;
            }
            let source = self.store_for(*member);
            if !ast::is_auto_accessor_property_declaration(source, *member) {
                continue;
            }
            let Some(name) = source.name(*member) else {
                continue;
            };
            if private_names_only && !ast::is_private_identifier(self.store_for(name), name) {
                continue;
            }
            let is_static = ast::has_static_modifier(source, *member);
            let private_storage_name =
                self.create_auto_accessor_private_storage_name(members, name);
            let storage_info = self.private_static_field_info(private_storage_name);
            let storage_name = storage_info
                .map(|info| info.storage_name)
                .unwrap_or(private_storage_name);
            if !self.auto_accessor_storage_uses_weak_map(storage_name) {
                auto_accessor_storage_names.push((*member, storage_name));
                changed = true;
                continue;
            }
            if !is_static && storage_info.is_none() {
                let initializer = self.create_weak_map_initializer(storage_name);
                if let Some(expression) = self.store_for(initializer).expression(initializer) {
                    pending_auto_accessor_storage_expressions.push(expression);
                }
            }
            auto_accessor_storage_names.push((*member, storage_name));
            changed = true;
        }
        changed
    }

    fn create_colliding_auto_accessor_storage_name(
        &mut self,
        members: &[ast::Node],
        name: ast::Node,
    ) -> Option<ast::Node> {
        let source = self.store_for(name);
        if !ast::is_identifier(source, name) {
            return None;
        }
        let private_name = format!("#{}", source.text(name));
        if !members.iter().any(|member| {
            self.store_for(*member).name(*member).is_some_and(|name| {
                ast::is_private_identifier(self.store_for(name), name)
                    && self.store_for(name).text(name) == private_name
            })
        }) {
            return None;
        }
        let mut index = 1;
        loop {
            let candidate = format!("{private_name}_{index}_accessor_storage");
            if !members.iter().any(|member| {
                self.store_for(*member).name(*member).is_some_and(|name| {
                    ast::is_private_identifier(self.store_for(name), name)
                        && self.store_for(name).text(name) == candidate
                })
            }) {
                return Some(self.factory_mut().new_private_identifier(candidate));
            }
            index += 1;
        }
    }

    fn create_auto_accessor_private_storage_name(
        &mut self,
        members: &[ast::Node],
        name: ast::Node,
    ) -> ast::Node {
        if let Some(storage_name) = self.create_colliding_auto_accessor_storage_name(members, name)
        {
            return storage_name;
        }
        self.emit_context.new_generated_private_name_for_node_ex(
            self.emit_context.most_original(&name),
            AutoGenerateOptions {
                suffix: "_accessor_storage",
                ..Default::default()
            },
        )
    }

    fn transform_members(
        &mut self,
        class_node: ast::Node,
        members: Vec<ast::Node>,
        class_name: Option<ast::Node>,
        class_constructor_reference: Option<ast::Node>,
        should_declare_class_constructor_reference: bool,
        class_constructor_reference_assignment: Option<ast::Node>,
        class_this: Option<ast::Node>,
        super_class_reference: Option<ast::Node>,
        storage_name_class_name: Option<ast::Node>,
        is_derived_class: bool,
        private_accessor_brand_check_identifier: Option<ast::Node>,
        private_accessor_environment: Option<PrivateAccessorEnvironment>,
        mut auto_accessor_storage_names: Vec<(ast::Node, ast::Node)>,
        mut pending_auto_accessor_storage_expressions: Vec<ast::Node>,
        precomputed_auto_accessor_storage_static_expressions: Vec<ast::Node>,
        private_instance_field_environments: Option<Vec<PrivateStaticFieldEnvironment>>,
        private_static_field_environment: Option<PrivateStaticFieldEnvironment>,
    ) -> (
        Vec<ast::Node>,
        Vec<ast::Node>,
        Vec<ast::Node>,
        bool,
        usize,
        Option<ast::Node>,
    ) {
        let member_nodes = members;
        let force_transform_static_private_elements = self.emit_context.emit_flags(&class_node)
            & printer::EF_TRANSFORM_PRIVATE_STATIC_ELEMENTS
            != 0
            || member_nodes.iter().any(|member| {
                ast::has_static_modifier(self.store_for(*member), *member)
                    && ast::is_private_identifier_class_element_declaration(
                        self.store_for(*member),
                        *member,
                    )
                    && self.should_always_transform_private_static_elements(*member)
            });
        let will_hoist_initializers_to_constructor =
            self.class_will_hoist_initializers_to_constructor(&member_nodes);
        let should_transform_auto_accessors =
            self.config.should_transform_auto_accessors || will_hoist_initializers_to_constructor;
        let mut class_constructor_reference_variable_declared = false;
        let precomputed_auto_accessor_storage_static_expressions =
            precomputed_auto_accessor_storage_static_expressions;
        let private_instance_field_environments = private_instance_field_environments
            .unwrap_or_else(|| {
                self.collect_private_instance_field_environments(
                    &member_nodes,
                    storage_name_class_name,
                )
            });
        let private_accessor_environment = if private_accessor_environment.is_some() {
            private_accessor_environment
        } else if let Some(private_accessors) = self.private_accessors(&member_nodes, false, false)
        {
            let weak_set_name = private_accessor_brand_check_identifier.unwrap_or_else(|| {
                self.create_hoisted_variable_for_class(storage_name_class_name, "instances", "")
            });
            if should_declare_class_constructor_reference
                && let Some(class_constructor_reference) = class_constructor_reference
            {
                self.emit_context
                    .add_variable_declaration(class_constructor_reference);
                class_constructor_reference_variable_declared = true;
            }
            let private_name_validities = self.private_name_validities(&member_nodes);
            self.create_private_instance_accessor_environment(
                private_accessors,
                storage_name_class_name,
                weak_set_name,
                &private_name_validities,
            )
        } else {
            None
        };
        if let Some(class_constructor_reference) = class_constructor_reference
            && should_declare_class_constructor_reference
            && !class_constructor_reference_variable_declared
        {
            self.emit_context
                .add_variable_declaration(class_constructor_reference);
        }
        let private_static_field_environment = private_static_field_environment.or_else(|| {
            class_this
                .or(class_name)
                .and_then(|brand_check_identifier| {
                    self.collect_private_static_field_environment(
                        &member_nodes,
                        storage_name_class_name,
                        brand_check_identifier,
                        force_transform_static_private_elements,
                    )
                })
        });
        let private_static_accessor_environment = class_this
            .or(class_constructor_reference)
            .or(class_name)
            .and_then(|brand_check_identifier| {
                self.collect_private_static_accessor_environment(
                    &member_nodes,
                    storage_name_class_name,
                    brand_check_identifier,
                    force_transform_static_private_elements,
                )
            });
        let mut private_accessor_stack_count = 0;
        let mut private_static_field_stack_count = 0;
        let mut class_alias_stack_count = 0;
        if let Some(name) = self.store_for(class_node).name(class_node) {
            let aliases_named_class_expression_to_static_property_temp =
                ast::is_class_expression(self.store_for(class_node), class_node)
                    && !self
                        .config
                        .should_transform_private_elements_or_class_static_blocks
                    && class_constructor_reference.is_some()
                    && class_name.is_some_and(|class_name| class_name != name);
            let alias = if aliases_named_class_expression_to_static_property_temp {
                None
            } else {
                class_constructor_reference.or_else(|| {
                    (ast::is_class_expression(self.store_for(class_node), class_node)
                        && class_name.is_some_and(|class_name| class_name != name))
                    .then_some(class_name)
                    .flatten()
                })
            };
            if let Some(alias) = alias {
                let name_text = self.store_for(name).text(name);
                self.class_alias_stack.push(ClassAlias { name_text, alias });
                class_alias_stack_count += 1;
            }
        }
        if let Some(environment) = private_accessor_environment.clone() {
            self.private_accessor_stack.push(environment.clone());
            private_accessor_stack_count += 1;
        }
        if let Some(environment) = private_static_accessor_environment.clone() {
            self.private_accessor_stack.push(environment.clone());
            private_accessor_stack_count += 1;
        }
        if let Some(environment) = private_static_field_environment.clone() {
            self.private_static_field_stack.push(environment);
            private_static_field_stack_count += 1;
        }
        for environment in private_instance_field_environments.iter().cloned() {
            self.private_static_field_stack.push(environment);
            private_static_field_stack_count += 1;
        }
        let mut changed = self.precompute_nonstatic_auto_accessor_storage_names(
            &member_nodes,
            storage_name_class_name,
            should_transform_auto_accessors,
            false,
            &mut auto_accessor_storage_names,
            &mut pending_auto_accessor_storage_expressions,
        );
        self.reorder_private_name_variable_declarations(
            &member_nodes,
            &auto_accessor_storage_names,
        );
        let mut transformed_members = Vec::new();
        let mut assignments = Vec::new();
        let mut static_assignments = Vec::new();
        let mut private_accessor_static_assignments = Vec::new();
        let has_class_constructor_reference_assignment =
            class_constructor_reference_assignment.is_some();
        let mut pending_static_expressions = class_constructor_reference_assignment
            .into_iter()
            .collect::<Vec<_>>();
        let mut precomputed_auto_accessor_storage_static_expressions =
            precomputed_auto_accessor_storage_static_expressions;
        let mut pending_static_name_expressions = Vec::new();
        let mut pending_static_name_expression_index = None;
        let mut constructor_index = None;
        let mut delayed_static_block_assignments = Vec::new();
        let mut delayed_member_visit_indices = Vec::new();
        let emit_auto_accessor_storage_before_private_accessor_brand =
            self.current_class_is_legacy_decorated && !self.config.legacy_decorators;
        for environment in private_instance_field_environments
            .iter()
            .filter(|environment| {
                emit_auto_accessor_storage_before_private_accessor_brand
                    || !environment
                        .fields
                        .iter()
                        .any(|field| field.is_auto_accessor_storage)
            })
        {
            let initializer = self.create_weak_map_initializer(environment.brand_check_identifier);
            let expression = self
                .store_for(initializer)
                .expression(initializer)
                .unwrap_or(initializer);
            if ast::is_class_declaration(self.store_for(class_node), class_node) {
                self.pending_expressions.push(expression);
            } else if has_class_constructor_reference_assignment || class_this.is_some() {
                pending_static_expressions.push(expression);
            } else {
                self.pending_expressions.push(expression);
            }
            changed = true;
        }
        if let Some(environment) = private_accessor_environment.as_ref() {
            let initializer = self.create_weak_set_initializer(environment.brand_check_identifier);
            if ast::is_class_declaration(self.store_for(class_node), class_node) {
                self.pending_expressions.push(initializer);
            } else if has_class_constructor_reference_assignment {
                pending_static_expressions.push(initializer);
            } else {
                self.pending_expressions.push(initializer);
            }
            if ast::is_class_declaration(self.store_for(class_node), class_node) {
                self.pending_expressions.extend(std::mem::take(
                    &mut precomputed_auto_accessor_storage_static_expressions,
                ));
            }
            assignments
                .push(self.create_brand_check_initializer(environment.brand_check_identifier));
            changed = true;
        }
        if !emit_auto_accessor_storage_before_private_accessor_brand {
            for environment in private_instance_field_environments
                .iter()
                .filter(|environment| {
                    environment
                        .fields
                        .iter()
                        .any(|field| field.is_auto_accessor_storage)
                })
            {
                let initializer =
                    self.create_weak_map_initializer(environment.brand_check_identifier);
                let expression = self
                    .store_for(initializer)
                    .expression(initializer)
                    .unwrap_or(initializer);
                if ast::is_class_declaration(self.store_for(class_node), class_node) {
                    self.pending_expressions.push(expression);
                } else if has_class_constructor_reference_assignment || class_this.is_some() {
                    pending_static_expressions.push(expression);
                } else {
                    self.pending_expressions.push(expression);
                }
                changed = true;
            }
        }
        pending_static_expressions.extend(precomputed_auto_accessor_storage_static_expressions);

        for member in member_nodes.iter().copied() {
            let member_kind = self.store_for(member).kind(member);
            let is_untransformed_private_property = self
                .is_untransformed_private_property(member, force_transform_static_private_elements);
            if member_kind == ast::Kind::Constructor {
                constructor_index = Some(transformed_members.len());
                transformed_members.push(member);
                continue;
            }
            if matches!(
                member_kind,
                ast::Kind::MethodDeclaration | ast::Kind::GetAccessor | ast::Kind::SetAccessor
            ) && let Some(statement) = self.transform_private_accessor_declaration(member)
            {
                if ast::is_class_declaration(self.store_for(class_node), class_node)
                    && !ast::has_static_modifier(self.store_for(member), member)
                    && let Some(expression) = self.store_for(statement).expression(statement)
                {
                    self.pending_expressions.push(expression);
                    changed = true;
                    continue;
                }
                if let Some(expression) = self.store_for(statement).expression(statement)
                    && ast::is_class_expression(self.store_for(class_node), class_node)
                    && !self
                        .config
                        .should_transform_private_elements_or_class_static_blocks
                {
                    if pending_static_name_expression_index.is_none() {
                        pending_static_name_expression_index = Some(
                            transformed_members
                                .iter()
                                .position(|member| {
                                    !self.is_class_this_assignment_block(*member)
                                        && !self.is_class_named_evaluation_helper_block(*member)
                                })
                                .unwrap_or(transformed_members.len()),
                        );
                    }
                    pending_static_name_expressions.push(expression);
                } else if class_this.is_some()
                    && ast::has_static_modifier(self.store_for(member), member)
                {
                    if ast::is_class_declaration(self.store_for(class_node), class_node)
                        && let Some(expression) = self.store_for(statement).expression(statement)
                    {
                        self.pending_expressions.push(expression);
                    } else {
                        private_accessor_static_assignments.push(statement);
                    }
                } else if let Some(expression) = self.store_for(statement).expression(statement) {
                    if ast::is_class_expression(self.store_for(class_node), class_node)
                        && !self
                            .config
                            .should_transform_private_elements_or_class_static_blocks
                    {
                        if pending_static_name_expression_index.is_none() {
                            pending_static_name_expression_index = Some(
                                transformed_members
                                    .iter()
                                    .position(|member| {
                                        !self.is_class_this_assignment_block(*member)
                                            && !self.is_class_named_evaluation_helper_block(*member)
                                    })
                                    .unwrap_or(transformed_members.len()),
                            );
                        }
                        pending_static_name_expressions.push(expression);
                    } else if ast::is_class_declaration(self.store_for(class_node), class_node)
                        && !ast::has_static_modifier(self.store_for(member), member)
                    {
                        self.pending_expressions.push(expression);
                    } else {
                        pending_static_expressions.push(expression);
                    }
                } else {
                    static_assignments.push(statement);
                }
                changed = true;
                continue;
            }
            let is_auto_accessor =
                ast::is_auto_accessor_property_declaration(self.store_for(member), member);
            if is_auto_accessor && should_transform_auto_accessors {
                let is_private_auto_accessor = self
                    .store_for(member)
                    .name(member)
                    .is_some_and(|name| ast::is_private_identifier(self.store_for(name), name));
                let should_transform_static_private_auto_accessor =
                    ast::has_static_modifier(self.store_for(member), member)
                        && (is_private_auto_accessor
                            || self.should_always_transform_private_static_elements(member))
                        && (private_static_accessor_environment.is_some()
                            || self.should_always_transform_private_static_elements(member));
                let should_transform_private_auto_accessor =
                    if ast::has_static_modifier(self.store_for(member), member) {
                        should_transform_static_private_auto_accessor
                    } else {
                        is_private_auto_accessor && private_accessor_environment.is_some()
                    };
                let auto_accessor_storage_name = auto_accessor_storage_names.iter().find_map(
                    |(storage_member, storage_name)| {
                        (*storage_member == member).then_some(*storage_name)
                    },
                );
                if let Some((accessors, initializer, storage_initializer)) = self
                    .transform_auto_accessor(
                        class_node,
                        member,
                        class_name,
                        class_constructor_reference,
                        class_this,
                        storage_name_class_name,
                        should_transform_static_private_auto_accessor,
                        will_hoist_initializers_to_constructor,
                        auto_accessor_storage_name,
                        &mut pending_auto_accessor_storage_expressions,
                    )
                {
                    let mut storage_initializer = storage_initializer;
                    let mut transformed_accessors = Vec::new();
                    for accessor in accessors {
                        if self.store_for(accessor).kind(accessor) == ast::Kind::PropertyDeclaration
                        {
                            let is_untransformed_private_property = self
                                .is_untransformed_private_property(
                                    accessor,
                                    force_transform_static_private_elements,
                                );
                            if is_untransformed_private_property
                                && !will_hoist_initializers_to_constructor
                            {
                                let visited = self.update_property_declaration(accessor);
                                changed |= visited != accessor;
                                transformed_accessors.push(visited);
                                continue;
                            }
                            if let Some(statement) = self.transform_property_initializer(accessor) {
                                assignments.push(statement);
                                changed = true;
                                if self.invalid_private_field_info(accessor).is_some() {
                                    transformed_accessors.push(accessor);
                                } else if is_untransformed_private_property {
                                    transformed_accessors.push(
                                        self.preserve_property_declaration_without_initializer(
                                            accessor,
                                        ),
                                    );
                                }
                                continue;
                            }
                        }
                        transformed_accessors.push(accessor);
                    }
                    if should_transform_static_private_auto_accessor
                        && class_this.is_some()
                        && !self
                            .config
                            .should_transform_private_elements_or_class_static_blocks
                        && let Some(storage_initializer) = storage_initializer.take()
                    {
                        let statement_list = self.factory_mut().new_node_list(
                            core::undefined_text_range(),
                            core::undefined_text_range(),
                            vec![storage_initializer],
                        );
                        let body = self.factory_mut().new_block(statement_list, true);
                        let static_block = self
                            .factory_mut()
                            .new_class_static_block_declaration(None, Some(body));
                        transformed_members.push(static_block);
                    }
                    if should_transform_private_auto_accessor {
                        for accessor in transformed_accessors {
                            if let Some(statement) =
                                self.transform_private_accessor_declaration(accessor)
                            {
                                if let Some(expression) =
                                    self.store_for(statement).expression(statement)
                                    && ast::is_class_expression(
                                        self.store_for(class_node),
                                        class_node,
                                    )
                                    && !self
                                        .config
                                        .should_transform_private_elements_or_class_static_blocks
                                {
                                    if pending_static_name_expression_index.is_none() {
                                        pending_static_name_expression_index = Some(
                                            transformed_members
                                                .iter()
                                                .position(|member| {
                                                    !self.is_class_this_assignment_block(*member)
                                                        && !self
                                                            .is_class_named_evaluation_helper_block(
                                                                *member,
                                                            )
                                                })
                                                .unwrap_or(transformed_members.len()),
                                        );
                                    }
                                    pending_static_name_expressions.push(expression);
                                } else if ast::is_class_declaration(
                                    self.store_for(class_node),
                                    class_node,
                                ) {
                                    if let Some(expression) =
                                        self.store_for(statement).expression(statement)
                                    {
                                        self.pending_expressions.push(expression);
                                    } else {
                                        static_assignments.push(statement);
                                    }
                                } else if class_this.is_some() {
                                    if ast::is_class_declaration(
                                        self.store_for(class_node),
                                        class_node,
                                    ) && let Some(expression) =
                                        self.store_for(statement).expression(statement)
                                    {
                                        pending_static_expressions.push(expression);
                                    } else {
                                        private_accessor_static_assignments.push(statement);
                                    }
                                } else if let Some(expression) =
                                    self.store_for(statement).expression(statement)
                                {
                                    if ast::is_class_expression(
                                        self.store_for(class_node),
                                        class_node,
                                    ) && !self
                                        .config
                                        .should_transform_private_elements_or_class_static_blocks
                                    {
                                        if pending_static_name_expression_index.is_none() {
                                            pending_static_name_expression_index = Some(
                                                transformed_members
                                                    .iter()
                                                    .position(|member| {
                                                        !self.is_class_this_assignment_block(*member)
                                                            && !self
                                                                .is_class_named_evaluation_helper_block(*member)
                                                    })
                                                    .unwrap_or(transformed_members.len()),
                                            );
                                        }
                                        pending_static_name_expressions.push(expression);
                                    } else if ast::is_class_declaration(
                                        self.store_for(class_node),
                                        class_node,
                                    ) && !ast::has_static_modifier(
                                        self.store_for(accessor),
                                        accessor,
                                    ) {
                                        pending_static_expressions.push(expression);
                                    } else {
                                        pending_static_expressions.push(expression);
                                    }
                                } else {
                                    static_assignments.push(statement);
                                }
                            } else {
                                transformed_members.push(accessor);
                            }
                        }
                    } else {
                        for accessor in transformed_accessors {
                            let visited = self.visit(&accessor).unwrap_or(accessor);
                            changed |= visited != accessor;
                            transformed_members.push(visited);
                        }
                    }
                    if let Some(initializer) = initializer {
                        self.set_property_initializer_statement_ranges(initializer, member);
                        assignments.push(initializer);
                    }
                    changed = true;
                    if let Some(storage_initializer) = storage_initializer {
                        if ast::has_static_modifier(self.store_for(member), member) {
                            if ast::is_class_expression(self.store_for(class_node), class_node)
                                && let Some(expression) = self
                                    .store_for(storage_initializer)
                                    .expression(storage_initializer)
                            {
                                let loc = self.store_for(member).loc(member);
                                self.emit_context.set_original(&expression, &member);
                                self.emit_context.set_comment_range(&expression, loc);
                                self.emit_context.set_source_map_range(&expression, loc);
                                self.emit_context
                                    .set_emit_flags(&expression, printer::EF_NONE);
                                self.emit_context
                                    .set_original(&storage_initializer, &member);
                                self.emit_context
                                    .set_comment_range(&storage_initializer, loc);
                                self.emit_context
                                    .set_source_map_range(&storage_initializer, loc);
                                self.emit_context
                                    .set_emit_flags(&storage_initializer, printer::EF_NONE);
                            }
                            if ast::is_class_declaration(self.store_for(class_node), class_node) {
                                self.set_property_initializer_statement_ranges(
                                    storage_initializer,
                                    member,
                                );
                            }
                            static_assignments.push(storage_initializer);
                        } else if let Some(expression) = self
                            .store_for(storage_initializer)
                            .expression(storage_initializer)
                        {
                            pending_static_expressions.push(expression);
                        } else {
                            static_assignments.push(storage_initializer);
                        }
                    }
                    continue;
                }
            }
            if is_auto_accessor {
                transformed_members.push(member);
                continue;
            }
            if self.config.should_transform_initializers
                && ast::has_static_modifier(self.store_for(member), member)
                && let Some(pending_expression) =
                    self.get_property_name_expression_if_needed(member)
            {
                self.pending_expressions.push(pending_expression);
                changed = true;
            }
            if ast::is_class_expression(self.store_for(class_node), class_node)
                && class_this.is_none()
                && class_constructor_reference.is_none()
                && !self.current_class_is_legacy_decorated
                && (self
                    .config
                    .should_transform_private_elements_or_class_static_blocks
                    || self.emit_context.emit_flags(&class_node)
                        & printer::EF_TRANSFORM_PRIVATE_STATIC_ELEMENTS
                        != 0)
                && ((ast::is_class_static_block_declaration(self.store_for(member), member)
                    && self.can_transform_static_property_or_class_static_block(member))
                    || (member_kind == ast::Kind::PropertyDeclaration
                        && ast::has_static_modifier(self.store_for(member), member)
                        && self.can_transform_static_property_or_class_static_block(member)))
            {
                changed = true;
                continue;
            }
            if self.invalid_private_non_field(member) {
                transformed_members.push(member);
                continue;
            }
            if let Some(static_block) =
                self.transform_public_static_property_initializer_to_class_static_block(member)
            {
                transformed_members.push(static_block);
                changed = true;
                continue;
            }
            if let Some(static_block) = self
                .transform_private_static_property_initializer_to_class_static_block(
                    member,
                    force_transform_static_private_elements,
                )
            {
                if self.invalid_private_field_info(member).is_some() {
                    transformed_members.push(member);
                    static_assignments.push(static_block);
                    changed = true;
                    continue;
                }
                transformed_members.push(static_block);
                changed = true;
                continue;
            }
            if member_kind == ast::Kind::ClassStaticBlockDeclaration
                && !self
                    .config
                    .should_transform_private_elements_or_class_static_blocks
                && (private_static_accessor_environment.is_some()
                    || force_transform_static_private_elements)
            {
                let receiver = if self.emit_context.emit_flags(&member)
                    & printer::EF_TRANSFORM_PRIVATE_STATIC_ELEMENTS
                    != 0
                {
                    self.factory_mut().new_token(ast::Kind::ThisKeyword)
                } else {
                    class_this
                        .or(class_name)
                        .unwrap_or_else(|| self.factory_mut().new_token(ast::Kind::ThisKeyword))
                };
                if let Some(static_block) =
                    self.transform_native_class_static_block_declaration(member, receiver)
                {
                    transformed_members.push(static_block);
                    changed = true;
                    continue;
                }
            }
            if member_kind == ast::Kind::ClassStaticBlockDeclaration
                && !self
                    .config
                    .should_transform_private_elements_or_class_static_blocks
            {
                let visited = self.generated_visit_each_child(&member);
                changed |= visited != member;
                transformed_members.push(visited);
                continue;
            }
            let static_property_receiver =
                if ast::is_class_declaration(self.store_for(class_node), class_node) {
                    class_name.or(storage_name_class_name)
                } else {
                    class_this.or(class_name).or(storage_name_class_name)
                };
            let static_initializer_class_constructor = if self.current_class_is_legacy_decorated {
                class_constructor_reference
            } else {
                class_constructor_reference.or(class_this)
            };
            if let Some(statement) = self.transform_static_property_initializer(
                member,
                static_property_receiver,
                static_initializer_class_constructor,
                super_class_reference,
                force_transform_static_private_elements,
            ) {
                static_assignments.push(statement);
                changed = true;
                if self.invalid_private_field_info(member).is_some() {
                    transformed_members.push(member);
                }
                continue;
            }
            if member_kind == ast::Kind::ClassStaticBlockDeclaration
                && self
                    .config
                    .should_transform_private_elements_or_class_static_blocks
            {
                let receiver = if self.current_class_is_legacy_decorated {
                    class_constructor_reference.or(class_this).or(class_name)
                } else {
                    class_this.or(class_name)
                }
                .or(storage_name_class_name)
                .unwrap_or_else(|| self.factory_mut().new_token(ast::Kind::ThisKeyword));
                delayed_static_block_assignments.push((static_assignments.len(), member, receiver));
                changed = true;
                continue;
            }
            if is_untransformed_private_property && !will_hoist_initializers_to_constructor {
                transformed_members.push(member);
                continue;
            }
            let member_original_node = self.emit_context.most_original(&member);
            if self.is_parameter_property_declaration(member_original_node, member_original_node)
                && !will_hoist_initializers_to_constructor
            {
                if self.config.should_transform_initializers {
                    changed = true;
                    continue;
                }
                transformed_members.push(member);
                continue;
            }
            if let Some(statement) = self.transform_property_initializer(member) {
                assignments.push(statement);
                changed = true;
                if self.invalid_private_field_info(member).is_some() {
                    transformed_members.push(member);
                } else if is_untransformed_private_property {
                    transformed_members
                        .push(self.preserve_property_declaration_without_initializer(member));
                }
                continue;
            }
            if member_kind == ast::Kind::PropertyDeclaration {
                if is_untransformed_private_property {
                    transformed_members.push(member);
                    continue;
                }
                if !self.config.should_transform_initializers {
                    transformed_members.push(self.update_property_declaration(member));
                    changed = true;
                    continue;
                }
                continue;
            }
            if matches!(
                member_kind,
                ast::Kind::MethodDeclaration | ast::Kind::GetAccessor | ast::Kind::SetAccessor
            ) {
                if constructor_index.is_some() && will_hoist_initializers_to_constructor {
                    delayed_member_visit_indices.push(transformed_members.len());
                    transformed_members.push(member);
                    continue;
                }
                let visited = self.update_method_or_accessor_declaration(member);
                changed |= visited != member;
                transformed_members.push(visited);
                continue;
            }
            transformed_members.push(member);
        }

        if !pending_static_name_expressions.is_empty() {
            let expression = self
                .emit_context
                .factory
                .inline_expressions(&pending_static_name_expressions)
                .expect("pending static name expressions should not be empty");
            let statement = self.factory_mut().new_expression_statement(expression);
            if self
                .config
                .should_transform_private_elements_or_class_static_blocks
            {
                static_assignments.insert(0, statement);
            } else {
                let statement_list = self.factory_mut().new_node_list(
                    core::undefined_text_range(),
                    core::undefined_text_range(),
                    vec![statement],
                );
                let body = self.factory_mut().new_block(statement_list, false);
                let static_block = self
                    .factory_mut()
                    .new_class_static_block_declaration(None, Some(body));
                let insert_index = pending_static_name_expression_index.unwrap_or(0);
                transformed_members.insert(insert_index, static_block);
                if let Some(index) = constructor_index
                    && insert_index <= index
                {
                    constructor_index = Some(index + 1);
                }
                for index in &mut delayed_member_visit_indices {
                    if insert_index <= *index {
                        *index += 1;
                    }
                }
            }
        }

        if ast::is_class_declaration(self.store_for(class_node), class_node) {
            self.pending_expressions
                .extend(pending_auto_accessor_storage_expressions);
        } else {
            pending_static_expressions.extend(pending_auto_accessor_storage_expressions);
        }

        let mut static_assignment_prefix_count = 0usize;
        if !private_accessor_static_assignments.is_empty() {
            static_assignment_prefix_count += private_accessor_static_assignments.len();
            private_accessor_static_assignments.extend(static_assignments);
            static_assignments = private_accessor_static_assignments;
        }

        let mut pending_static_expression_statement_count = 0usize;
        if !pending_static_expressions.is_empty() {
            if ast::is_class_expression(self.store_for(class_node), class_node) {
                let statements = pending_static_expressions
                    .into_iter()
                    .map(|expression| self.factory_mut().new_expression_statement(expression))
                    .collect::<Vec<_>>();
                pending_static_expression_statement_count = statements.len();
                static_assignment_prefix_count += statements.len();
                static_assignments.splice(0..0, statements);
            } else {
                let expression = self
                    .emit_context
                    .factory
                    .inline_expressions(&pending_static_expressions)
                    .expect("pending static expressions should not be empty");
                let statement = self.factory_mut().new_expression_statement(expression);
                static_assignment_prefix_count += 1;
                static_assignments.insert(0, statement);
            }
        }

        let members_prologue = self.inject_pending_expression_static_block(
            &mut transformed_members,
            &mut constructor_index,
            &mut delayed_member_visit_indices,
        );

        if assignments.is_empty() {
            if let Some(index) = constructor_index {
                let member = transformed_members[index];
                let visited = self.visit(&member).unwrap_or(member);
                changed |= visited != member;
                transformed_members[index] = visited;
            }
            for index in delayed_member_visit_indices {
                let member = transformed_members[index];
                let visited = self.visit(&member).unwrap_or(member);
                changed |= visited != member;
                transformed_members[index] = visited;
            }
            for (index, member, receiver) in delayed_static_block_assignments.into_iter().rev() {
                if let Some(statement) = self.transform_property_or_class_static_block(
                    member,
                    receiver,
                    super_class_reference,
                ) {
                    static_assignments.insert(index + static_assignment_prefix_count, statement);
                }
            }
            let transformed_members = transformed_members
                .into_iter()
                .map(|member| self.preserve_node(member))
                .collect();
            for _ in 0..private_accessor_stack_count {
                self.private_accessor_stack.pop();
            }
            for _ in 0..private_static_field_stack_count {
                self.private_static_field_stack.pop();
            }
            for _ in 0..class_alias_stack_count {
                self.class_alias_stack.pop();
            }
            return (
                transformed_members,
                assignments,
                static_assignments,
                changed,
                pending_static_expression_statement_count,
                members_prologue,
            );
        }

        self.emit_context.start_variable_environment();

        if let Some(index) = constructor_index {
            let constructor_node = transformed_members[index];
            let mut statements = Vec::new();
            if let Some(body) = self.store_for(constructor_node).body(constructor_node) {
                let original_statements_view = self
                    .store_for(body)
                    .statements(body)
                    .expect("constructor body should have statements");
                let original_statements: Vec<_> = original_statements_view.iter().collect();
                for statement in &original_statements {
                    if ast::is_prologue_directive(self.store_for(*statement), *statement) {
                        statements.push(self.preserve_node(*statement));
                    } else {
                        break;
                    }
                }
                let mut statement_offset = statements.len();
                if let Some(super_path) =
                    self.find_super_statement_index_path(&original_statements, statement_offset)
                {
                    statements = self.transform_constructor_body_worker(
                        statements,
                        &original_statements,
                        statement_offset,
                        &super_path,
                        0,
                        &assignments,
                        constructor_node,
                    );
                } else {
                    // parameter-property assignments should occur immediately after the prologue and `super()`,
                    // so only count the statements that immediately follow.
                    let original_constructor = self.emit_context.most_original(&constructor_node);
                    while statement_offset < original_statements.len() {
                        let stmt = original_statements[statement_offset];
                        let orig = self.emit_context.most_original(&stmt);
                        if self.is_parameter_property_declaration(orig, original_constructor) {
                            statement_offset += 1;
                        } else {
                            break;
                        }
                    }
                    statements.extend(assignments.iter().copied());
                    let saved_pending_expressions = std::mem::take(&mut self.pending_expressions);
                    let visited =
                        self.visit_statement_slice(&original_statements[statement_offset..]);
                    self.pending_expressions = saved_pending_expressions;
                    statements.extend(visited);
                }
            } else if is_derived_class {
                statements.push(self.create_synthetic_super_call());
                statements.extend(assignments.iter().copied());
            } else {
                statements.extend(assignments.iter().copied());
            }
            let mut declarations = std::mem::take(&mut self.pending_instance_variable_declarations);
            declarations.extend(self.emit_context.end_variable_environment());
            let declarations = self.coalesce_variable_environment_declarations(declarations);
            let statements = self
                .emit_context
                .merge_environment(self.source, &statements, &declarations)
                .0;
            let (statements_loc, statements_range, body_loc) =
                if let Some(body) = self.store_for(constructor_node).body(constructor_node) {
                    let statements = self
                        .store_for(body)
                        .statements(body)
                        .expect("constructor body should have statements");
                    (
                        statements.loc(),
                        statements.range(),
                        Some(self.store_for(body).loc(body)),
                    )
                } else {
                    let members = self
                        .store_for(class_node)
                        .source_members(class_node)
                        .expect("class should have members");
                    (members.loc(), members.range(), None)
                };
            let statement_list =
                self.factory_mut()
                    .new_node_list(statements_loc, statements_range, statements);
            let body = self.factory_mut().new_block(statement_list, true);
            if let Some(body_loc) = body_loc {
                self.factory_mut().place_emit_synthetic_node(body, body_loc);
            }
            transformed_members[index] = {
                if constructor_node.store_id() == self.factory().store().store_id() {
                    let (type_parameters, parameters, type_node, full_signature) = {
                        let store = self.factory().store();
                        let type_parameters =
                            store.source_type_parameters(constructor_node).map(|nodes| {
                                (
                                    nodes.loc(),
                                    nodes.range(),
                                    nodes.iter().collect::<Vec<_>>(),
                                    nodes.has_trailing_comma(),
                                )
                            });
                        let parameters =
                            store.source_parameters(constructor_node).map(|parameters| {
                                (
                                    parameters.loc(),
                                    parameters.range(),
                                    parameters.iter().collect::<Vec<_>>(),
                                    parameters.has_trailing_comma(),
                                )
                            });
                        let type_node = store.r#type(constructor_node);
                        let full_signature = store.full_signature(constructor_node);
                        (type_parameters, parameters, type_node, full_signature)
                    };
                    if let Some((loc, range, parameter_nodes, has_trailing_comma)) = parameters {
                        let type_parameters =
                            type_parameters.map(|(loc, range, nodes, has_trailing_comma)| {
                                let nodes: Vec<ast::Node> = nodes
                                    .into_iter()
                                    .map(|node| self.clone_node_for_reuse(node))
                                    .collect();
                                self.factory_mut().new_node_list_with_trailing_comma(
                                    loc,
                                    range,
                                    nodes,
                                    has_trailing_comma,
                                )
                            });
                        let parameter_nodes: Vec<ast::Node> = parameter_nodes
                            .into_iter()
                            .map(|node| self.clone_node_for_reuse(node))
                            .collect();
                        let parameters = self.factory_mut().new_node_list_with_trailing_comma(
                            loc,
                            range,
                            parameter_nodes,
                            has_trailing_comma,
                        );
                        let type_node =
                            type_node.map(|type_node| self.clone_node_for_reuse(type_node));
                        let full_signature = full_signature
                            .map(|full_signature| self.clone_node_for_reuse(full_signature));
                        self.factory_mut().update_constructor_declaration(
                            constructor_node,
                            None,
                            type_parameters,
                            parameters,
                            type_node,
                            full_signature,
                            Some(body),
                        )
                    } else {
                        let parameters = self.factory_mut().new_node_list(
                            core::undefined_text_range(),
                            core::undefined_text_range(),
                            Vec::<ast::Node>::new(),
                        );
                        self.factory_mut().new_constructor_declaration(
                            None,
                            None,
                            parameters,
                            None,
                            None,
                            Some(body),
                        )
                    }
                } else {
                    let source = self.source;
                    let type_parameters = self.import_state.preserve_optional_source_node_list(
                        &mut self.emit_context.factory.node_factory,
                        source.source_type_parameters(constructor_node),
                    );
                    let parameters = self.import_state.preserve_source_node_list(
                        &mut self.emit_context.factory.node_factory,
                        source
                            .source_parameters(constructor_node)
                            .expect("constructor should have parameters"),
                    );
                    let type_node = source
                        .r#type(constructor_node)
                        .map(|type_node| self.preserve_node(type_node));
                    let full_signature = source
                        .full_signature(constructor_node)
                        .map(|full_signature| self.preserve_node(full_signature));
                    self.factory_mut()
                        .update_constructor_declaration_from_store(
                            source,
                            constructor_node,
                            None,
                            type_parameters,
                            parameters,
                            type_node,
                            full_signature,
                            Some(body),
                        )
                }
            };
            changed = true;
        } else {
            let mut statements = Vec::new();
            if is_derived_class {
                statements.push(self.create_synthetic_super_call());
            }
            statements.extend(assignments.iter().copied());
            let mut declarations = std::mem::take(&mut self.pending_instance_variable_declarations);
            declarations.extend(self.emit_context.end_variable_environment());
            let declarations = self.coalesce_variable_environment_declarations(declarations);
            let statements = self
                .emit_context
                .merge_environment(self.source, &statements, &declarations)
                .0;
            let members = self
                .store_for(class_node)
                .source_members(class_node)
                .expect("class should have members");
            let members_loc = members.loc();
            let members_range = members.range();
            let statement_list =
                self.factory_mut()
                    .new_node_list(members_loc, members_range, statements);
            let body = self.factory_mut().new_block(statement_list, true);
            let parameters = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                Vec::<ast::Node>::new(),
            );
            let constructor = self.factory_mut().new_constructor_declaration(
                None,
                None,
                parameters,
                None,
                None,
                Some(body),
            );
            let class_loc = self.store_for(class_node).loc(class_node);
            self.factory_mut()
                .place_emit_synthetic_node(constructor, class_loc);
            self.emit_context
                .mark_emit_node(&constructor, printer::EF_START_ON_NEW_LINE);
            transformed_members.insert(0, constructor);
            for index in &mut delayed_member_visit_indices {
                *index += 1;
            }
            changed = true;
        }

        for index in delayed_member_visit_indices {
            let member = transformed_members[index];
            let visited = self.visit(&member).unwrap_or(member);
            changed |= visited != member;
            transformed_members[index] = visited;
        }

        for (index, member, receiver) in delayed_static_block_assignments.into_iter().rev() {
            if let Some(statement) = self.transform_property_or_class_static_block(
                member,
                receiver,
                super_class_reference,
            ) {
                static_assignments.insert(index + static_assignment_prefix_count, statement);
            }
        }

        let transformed_members = transformed_members
            .into_iter()
            .map(|member| self.preserve_node(member))
            .collect();

        for _ in 0..private_accessor_stack_count {
            self.private_accessor_stack.pop();
        }
        for _ in 0..private_static_field_stack_count {
            self.private_static_field_stack.pop();
        }
        for _ in 0..class_alias_stack_count {
            self.class_alias_stack.pop();
        }

        (
            transformed_members,
            assignments,
            static_assignments,
            changed,
            pending_static_expression_statement_count,
            members_prologue,
        )
    }

    fn inject_pending_expression_static_block(
        &mut self,
        transformed_members: &mut Vec<ast::Node>,
        constructor_index: &mut Option<usize>,
        delayed_member_visit_indices: &mut [usize],
    ) -> Option<ast::Node> {
        if self
            .config
            .should_transform_private_elements_or_class_static_blocks
            || self.pending_expressions.is_empty()
        {
            return None;
        }

        let pending_expressions = std::mem::take(&mut self.pending_expressions);
        let expression = self
            .emit_context
            .factory
            .inline_expressions(&pending_expressions)
            .expect("pending expressions should not be empty");
        let mut statement = self.factory_mut().new_expression_statement(expression);
        let mut prologue = None;

        if self
            .store_for(statement)
            .subtree_facts(statement)
            .intersects(ast::SubtreeFacts::CONTAINS_LEXICAL_THIS_OR_SUPER)
        {
            let temp = self.emit_context.factory.new_temp_variable();
            self.emit_context.add_variable_declaration(temp);

            let parameters = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                Vec::<ast::Node>::new(),
            );
            let equals_greater_than = self
                .factory_mut()
                .new_token(ast::Kind::EqualsGreaterThanToken);
            let arrow_statements = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                vec![statement],
            );
            let arrow_body = self.factory_mut().new_block(arrow_statements, false);
            let arrow = self.factory_mut().new_arrow_function(
                None::<ast::ModifierList>,
                None::<ast::NodeList>,
                parameters,
                None::<ast::Node>,
                None::<ast::Node>,
                Some(equals_greater_than),
                arrow_body,
            );
            prologue = Some(
                self.emit_context
                    .factory
                    .new_assignment_expression(temp, arrow),
            );

            let arguments = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                Vec::<ast::Node>::new(),
            );
            let call = self.factory_mut().new_call_expression(
                temp,
                None::<ast::Node>,
                None::<ast::NodeList>,
                arguments,
                ast::NodeFlags::NONE,
            );
            statement = self.factory_mut().new_expression_statement(call);
        }

        let statement_list = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![statement],
        );
        let body = self.factory_mut().new_block(statement_list, false);
        let static_block = self
            .factory_mut()
            .new_class_static_block_declaration(None, Some(body));
        let insert_index = transformed_members
            .iter()
            .position(|member| {
                !self.is_class_this_assignment_block(*member)
                    && !self.is_class_named_evaluation_helper_block(*member)
                    && self.store_for(*member).kind(*member) != ast::Kind::Constructor
            })
            .unwrap_or(transformed_members.len());
        transformed_members.insert(insert_index, static_block);
        if let Some(index) = constructor_index
            && insert_index <= *index
        {
            *constructor_index = Some(*index + 1);
        }
        for index in delayed_member_visit_indices {
            if insert_index <= *index {
                *index += 1;
            }
        }

        prologue
    }

    fn class_will_hoist_initializers_to_constructor(&self, members: &[ast::Node]) -> bool {
        let mut contains_public_instance_fields = false;
        let mut contains_initialized_public_instance_fields = false;
        let mut contains_instance_private_elements = false;
        let mut contains_instance_auto_accessors = false;

        for member in members {
            let source = self.store_for(*member);
            if ast::has_static_modifier(source, *member) {
                continue;
            }
            if ast::has_abstract_modifier(source, *member) {
                continue;
            }
            if ast::is_auto_accessor_property_declaration(source, *member) {
                contains_instance_auto_accessors = true;
                if source
                    .name(*member)
                    .is_some_and(|name| ast::is_private_identifier(self.store_for(name), name))
                {
                    contains_instance_private_elements = true;
                }
            } else if source
                .name(*member)
                .is_some_and(|name| ast::is_private_identifier(self.store_for(name), name))
            {
                contains_instance_private_elements = true;
            } else if source.kind(*member) == ast::Kind::PropertyDeclaration {
                contains_public_instance_fields = true;
                contains_initialized_public_instance_fields |=
                    source.initializer(*member).is_some();
            }
        }

        (self.config.should_transform_initializers_using_define && contains_public_instance_fields)
            || (self.config.should_transform_initializers_using_set
                && contains_initialized_public_instance_fields)
            || (self
                .config
                .should_transform_private_elements_or_class_static_blocks
                && contains_instance_private_elements)
            || (self
                .config
                .should_transform_private_elements_or_class_static_blocks
                && self.config.should_transform_auto_accessors
                && contains_instance_auto_accessors)
    }

    fn transform_auto_accessor(
        &mut self,
        class_node: ast::Node,
        member: ast::Node,
        class_name: Option<ast::Node>,
        class_constructor_reference: Option<ast::Node>,
        class_this: Option<ast::Node>,
        _storage_name_class_name: Option<ast::Node>,
        _force_transform_static_private_elements: bool,
        _will_hoist_initializers_to_constructor: bool,
        storage_name: Option<ast::Node>,
        pending_expressions: &mut Vec<ast::Node>,
    ) -> Option<(Vec<ast::Node>, Option<ast::Node>, Option<ast::Node>)> {
        let (name, initializer) = {
            let source = self.store_for(member);
            (source.name(member)?, source.initializer(member))
        };
        // Since we're creating two declarations where there was previously one, cache
        // the expression for any computed property names.
        let mut getter_name = name;
        let mut setter_name = name;
        if ast::is_computed_property_name(self.store_for(name), name) {
            let expression = self
                .store_for(name)
                .expression(name)
                .expect("computed property name should have expression");
            let expression_source = self.store_for(expression);
            let inlinable = module_transform_utilities::is_simple_inlineable_expression(
                expression_source.kind(expression),
                ast::is_identifier(expression_source, expression),
            );
            if !inlinable {
                if let Some(cache_assignment) =
                    self.find_computed_property_name_cache_assignment(expression)
                {
                    let visited_expression = self
                        .visit_node(Some(expression))
                        .expect("computed property name expression should visit");
                    getter_name = self.update_computed_property_name(name, visited_expression);
                    let left = self
                        .store_for(cache_assignment)
                        .left(cache_assignment)
                        .expect("computed property name cache assignment should have left");
                    setter_name = self.update_computed_property_name(name, left);
                } else {
                    let temp = self.emit_context.factory.new_temp_variable();
                    let loc = self.store_for(expression).loc(expression);
                    self.emit_context.set_source_map_range(&temp, loc);
                    self.emit_context.add_variable_declaration(temp);
                    let visited_expression = self
                        .visit_node(Some(expression))
                        .expect("computed property name expression should visit");
                    let assignment = self
                        .emit_context
                        .factory
                        .new_assignment_expression(temp, visited_expression);
                    self.emit_context.set_source_map_range(&assignment, loc);
                    getter_name = self.update_computed_property_name(name, assignment);
                    setter_name = self.update_computed_property_name(name, temp);
                }
            }
        }
        let is_static = ast::has_static_modifier(self.store_for(member), member);
        let has_precomputed_storage_name = storage_name.is_some();
        let storage_name = storage_name.unwrap_or_else(|| {
            let storage_property_name = self
                .store_for(member)
                .name(member)
                .filter(|name| {
                    let source = self.store_for(*name);
                    ast::is_identifier(source, *name) || ast::is_private_identifier(source, *name)
                })
                .unwrap_or(getter_name);
            let private_storage_name =
                self.create_auto_accessor_private_storage_name(&[member], storage_property_name);
            self.private_static_field_info(private_storage_name)
                .map(|info| info.storage_name)
                .unwrap_or(private_storage_name)
        });
        if ast::is_computed_property_name(self.store_for(getter_name), getter_name) {
            let expression = self
                .store_for(getter_name)
                .expression(getter_name)
                .expect("computed property name should have expression");
            let expression = self.inject_pending_expressions(pending_expressions, expression);
            getter_name = self.update_computed_property_name(getter_name, expression);
        }
        let uses_weak_map_storage = self.auto_accessor_storage_uses_weak_map(storage_name);
        let static_this_receiver = if uses_weak_map_storage {
            class_this.or(class_constructor_reference).or(class_name)
        } else {
            class_this.or_else(|| {
                self.store_for(class_node)
                    .name(class_node)
                    .map(|name| self.preserve_node(name))
            })
        };
        let initializer = if is_static && self.config.should_transform_this_in_static_initializers {
            let saved_class_static_block_receiver = self.current_class_static_block_receiver;
            self.current_class_static_block_receiver = static_this_receiver;
            let initializer =
                initializer.and_then(|initializer| self.visit_node(Some(initializer)));
            self.current_class_static_block_receiver = saved_class_static_block_receiver;
            initializer
        } else {
            initializer.and_then(|initializer| self.visit_node(Some(initializer)))
        };
        let storage_initializer =
            if uses_weak_map_storage && (!has_precomputed_storage_name || is_static) {
                let initializer = initializer
                    .unwrap_or_else(|| self.emit_context.factory.new_void_zero_expression());
                Some(if is_static {
                    self.create_private_static_field_initializer(
                        storage_name,
                        initializer,
                        Some(member),
                    )
                } else {
                    self.create_weak_map_initializer(storage_name)
                })
            } else {
                None
            };
        let mut accessors = Vec::new();
        let instance_initializer = if uses_weak_map_storage && !is_static {
            let initializer =
                initializer.unwrap_or_else(|| self.emit_context.factory.new_void_zero_expression());
            Some(self.create_private_instance_field_initializer(storage_name, initializer))
        } else {
            None
        };
        if !uses_weak_map_storage {
            let modifiers = self.auto_accessor_redirect_modifiers(member);
            let backing_field = self.factory_mut().new_property_declaration(
                modifiers,
                storage_name,
                None,
                None,
                initializer,
            );
            let _comment_range = self.emit_context.comment_range(&member);
            let source_map_range = self.emit_context.source_map_range(&member);
            self.emit_context.set_original(&backing_field, &member);
            self.emit_context
                .mark_emit_node(&backing_field, printer::EF_NO_COMMENTS);
            self.emit_context
                .set_source_map_range(&backing_field, source_map_range);
            accessors.push(backing_field);
        }
        let static_private_brand =
            (is_static && uses_weak_map_storage).then_some(static_this_receiver);
        let receiver = if is_static {
            static_this_receiver
                .unwrap_or_else(|| self.factory_mut().new_token(ast::Kind::ThisKeyword))
        } else {
            self.factory_mut().new_token(ast::Kind::ThisKeyword)
        };
        let getter = self.create_auto_accessor_getter(
            member,
            getter_name,
            storage_name,
            receiver,
            static_private_brand.flatten(),
        );
        let receiver = if is_static {
            static_this_receiver
                .unwrap_or_else(|| self.factory_mut().new_token(ast::Kind::ThisKeyword))
        } else {
            self.factory_mut().new_token(ast::Kind::ThisKeyword)
        };
        let setter = self.create_auto_accessor_setter(
            member,
            setter_name,
            storage_name,
            receiver,
            static_private_brand.flatten(),
        );
        accessors.push(getter);
        accessors.push(setter);
        Some((accessors, instance_initializer, storage_initializer))
    }

    fn is_decorated_auto_accessor_extra_initializer(&self, initializer: ast::Node) -> bool {
        let source = self.store_for(initializer);
        if source.kind(initializer) != ast::Kind::ParenthesizedExpression {
            return false;
        }
        let Some(expression) = source.expression(initializer) else {
            return false;
        };
        let expression_source = self.store_for(expression);
        expression_source.kind(expression) == ast::Kind::BinaryExpression
            && expression_source
                .operator_token(expression)
                .is_some_and(|operator| {
                    self.store_for(operator).kind(operator) == ast::Kind::CommaToken
                })
    }

    fn create_auto_accessor_downlevel_storage_name(
        &mut self,
        class_name: Option<ast::Node>,
        property_name: ast::Node,
    ) -> ast::Node {
        assert!(
            property_name.store_id() == self.factory().store().store_id()
                || property_name.store_id() == self.source.store_id(),
            "auto-accessor storage name cannot read unrelated AST store"
        );
        if ast::is_private_identifier(self.store_for(property_name), property_name) {
            let text = self.store_for(property_name).text(property_name);
            let text = text.strip_prefix('#').unwrap_or(&text);
            return self.create_hoisted_variable_for_class_non_optimistic(
                class_name,
                text,
                "_accessor_storage",
            );
        }
        if ast::is_identifier(self.store_for(property_name), property_name) {
            let text = self.store_for(property_name).text(property_name);
            return self.create_hoisted_variable_for_class(class_name, &text, "_accessor_storage");
        }
        self.create_hoisted_variable_for_class_from_node(
            class_name,
            property_name,
            "_accessor_storage",
        )
    }

    fn auto_accessor_storage_uses_weak_map(&self, storage_name: ast::Node) -> bool {
        !ast::is_private_identifier(self.store_for(storage_name), storage_name)
    }

    fn should_substitute_this_for_static_generated_auto_accessor_storage(
        &self,
        name: ast::Node,
        receiver: ast::Node,
    ) -> bool {
        self.store_for(receiver).kind(receiver) == ast::Kind::ThisKeyword
            && !self.current_class_static_block_preserves_static_auto_accessor_this
            && self
                .emit_context
                .get_auto_generate_info(Some(&name))
                .is_some_and(|info| info.suffix == "_accessor_storage")
    }

    fn create_private_instance_field_initializer(
        &mut self,
        storage_name: ast::Node,
        initializer: ast::Node,
    ) -> ast::Node {
        let receiver = self.factory_mut().new_token(ast::Kind::ThisKeyword);
        self.create_private_instance_field_initializer_for_receiver(
            storage_name,
            receiver,
            initializer,
        )
    }

    fn create_private_instance_field_initializer_for_receiver(
        &mut self,
        storage_name: ast::Node,
        receiver: ast::Node,
        initializer: ast::Node,
    ) -> ast::Node {
        if !self.auto_accessor_storage_uses_weak_map(storage_name) {
            let access = self.factory_mut().new_property_access_expression(
                receiver,
                None,
                storage_name,
                ast::NodeFlags::NONE,
            );
            let equals_token = self.factory_mut().new_token(ast::Kind::EqualsToken);
            let assignment = self.factory_mut().new_binary_expression(
                None,
                access,
                None,
                equals_token,
                initializer,
            );
            return self.factory_mut().new_expression_statement(assignment);
        }
        let set_name = self.factory_mut().new_identifier("set");
        let call = self.emit_context.factory.new_method_call(
            &storage_name,
            &set_name,
            &[receiver, initializer],
        );
        self.factory_mut().new_expression_statement(call)
    }

    fn create_weak_map_initializer(&mut self, storage_name: ast::Node) -> ast::Node {
        let weak_map = self.factory_mut().new_identifier("WeakMap");
        let arguments = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            Vec::<ast::Node>::new(),
        );
        let weak_map = self
            .factory_mut()
            .new_new_expression(weak_map, None, Some(arguments));
        self.emit_context
            .mark_emit_node(&weak_map, printer::EF_NO_COMMENTS);
        self.emit_context
            .set_comment_range(&weak_map, core::undefined_text_range());
        let assignment = self
            .emit_context
            .factory
            .new_assignment_expression(storage_name, weak_map);
        self.emit_context
            .mark_emit_node(&assignment, printer::EF_NO_COMMENTS);
        self.emit_context
            .set_comment_range(&assignment, core::undefined_text_range());
        self.factory_mut().new_expression_statement(assignment)
    }

    fn create_private_static_field_initializer(
        &mut self,
        storage_name: ast::Node,
        initializer: ast::Node,
        property: Option<ast::Node>,
    ) -> ast::Node {
        let value_name = self.factory_mut().new_identifier("value");
        let value = self.factory_mut().new_property_assignment(
            None::<ast::ModifierList>,
            value_name,
            None,
            None,
            initializer,
        );
        let properties = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![value],
        );
        let object = self
            .factory_mut()
            .new_object_literal_expression(properties, false);
        self.emit_context
            .mark_emit_node(&object, printer::EF_NO_COMMENTS);
        self.emit_context
            .set_comment_range(&object, core::undefined_text_range());
        let assignment = self
            .emit_context
            .factory
            .new_assignment_expression(storage_name, object);
        let property_name = property.and_then(|property| self.store_for(property).name(property));
        if let Some(property_name) = property_name {
            let source_map_range = self.emit_context.source_map_range(&property_name);
            self.emit_context
                .set_source_map_range(&assignment, source_map_range);
        }
        self.emit_context
            .mark_emit_node(&assignment, printer::EF_NO_COMMENTS);
        self.emit_context
            .set_comment_range(&assignment, core::undefined_text_range());
        self.factory_mut().new_expression_statement(assignment)
    }

    fn create_auto_accessor_getter(
        &mut self,
        member: ast::Node,
        name: ast::Node,
        storage_name: ast::Node,
        receiver: ast::Node,
        static_private_brand: Option<ast::Node>,
    ) -> ast::Node {
        let modifiers = self.auto_accessor_redirect_modifiers(member);
        let name = self.preserve_node(name);
        let value = if let Some(static_private_brand) = static_private_brand {
            let receiver = self.clone_node_for_reuse(receiver);
            let static_private_brand = self.clone_node_for_reuse(static_private_brand);
            let storage_name = self.clone_node_for_reuse(storage_name);
            self.emit_context
                .factory
                .new_class_private_field_get_helper(
                    receiver,
                    static_private_brand,
                    printer::PrivateIdentifierKind::Field,
                    Some(storage_name),
                )
        } else if self.auto_accessor_storage_uses_weak_map(storage_name) {
            let receiver = self.clone_node_for_reuse(receiver);
            let storage_name = self.clone_node_for_reuse(storage_name);
            self.emit_context
                .factory
                .new_class_private_field_get_helper(
                    receiver,
                    storage_name,
                    printer::PrivateIdentifierKind::Field,
                    None,
                )
        } else {
            let receiver = self.clone_node_for_reuse(receiver);
            let storage_name = self.clone_node_for_reuse(storage_name);
            self.factory_mut().new_property_access_expression(
                receiver,
                None,
                storage_name,
                ast::NodeFlags::NONE,
            )
        };
        let return_statement = self.factory_mut().new_return_statement(value);
        let statements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![return_statement],
        );
        let body = self.factory_mut().new_block(statements, false);
        let parameters = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            Vec::<ast::Node>::new(),
        );
        let getter = self
            .factory_mut()
            .new_get_accessor_declaration(modifiers, name, None, parameters, None, None, body);
        let comment_range = self.emit_context.comment_range(&member);
        let source_map_range = self.emit_context.source_map_range(&member);
        self.emit_context.set_original(&getter, &member);
        self.emit_context.set_comment_range(&getter, comment_range);
        self.emit_context
            .set_source_map_range(&getter, source_map_range);
        getter
    }

    fn create_auto_accessor_setter(
        &mut self,
        member: ast::Node,
        name: ast::Node,
        storage_name: ast::Node,
        receiver: ast::Node,
        static_private_brand: Option<ast::Node>,
    ) -> ast::Node {
        let modifiers = self.auto_accessor_redirect_modifiers(member);
        let name = self.preserve_node(name);
        let value_name = self.factory_mut().new_identifier("value");
        let value_parameter = self
            .factory_mut()
            .new_parameter_declaration(None, None, value_name, None, None, None);
        let parameters = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![value_parameter],
        );
        let value = self.factory_mut().new_identifier("value");
        let set_expression = if let Some(static_private_brand) = static_private_brand {
            let receiver = self.clone_node_for_reuse(receiver);
            let static_private_brand = self.clone_node_for_reuse(static_private_brand);
            let storage_name = self.clone_node_for_reuse(storage_name);
            self.emit_context
                .factory
                .new_class_private_field_set_helper(
                    receiver,
                    static_private_brand,
                    value,
                    printer::PrivateIdentifierKind::Field,
                    Some(storage_name),
                )
        } else if self.auto_accessor_storage_uses_weak_map(storage_name) {
            let receiver = self.clone_node_for_reuse(receiver);
            let storage_name = self.clone_node_for_reuse(storage_name);
            let value = self.clone_node_for_reuse(value);
            self.emit_context
                .factory
                .new_class_private_field_set_helper(
                    receiver,
                    storage_name,
                    value,
                    printer::PrivateIdentifierKind::Field,
                    None,
                )
        } else {
            let receiver = self.clone_node_for_reuse(receiver);
            let storage_name = self.clone_node_for_reuse(storage_name);
            let value = self.clone_node_for_reuse(value);
            let access = self.factory_mut().new_property_access_expression(
                receiver,
                None,
                storage_name,
                ast::NodeFlags::NONE,
            );
            let equals_token = self.factory_mut().new_token(ast::Kind::EqualsToken);
            self.factory_mut()
                .new_binary_expression(None, access, None, equals_token, value)
        };
        let statement = self.factory_mut().new_expression_statement(set_expression);
        let statements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![statement],
        );
        let body = self.factory_mut().new_block(statements, false);
        let setter = self
            .factory_mut()
            .new_set_accessor_declaration(modifiers, name, None, parameters, None, None, body);
        let source_map_range = self.emit_context.source_map_range(&member);
        self.emit_context.set_original(&setter, &member);
        self.emit_context
            .mark_emit_node(&setter, printer::EF_NO_COMMENTS);
        self.emit_context
            .set_source_map_range(&setter, source_map_range);
        setter
    }

    fn auto_accessor_redirect_modifiers(&mut self, member: ast::Node) -> Option<ast::ModifierList> {
        let source = self.store_for(member);
        let modifier_flags = source
            .source_modifiers(member)
            .map(|modifiers| modifiers.modifier_flags())
            .unwrap_or(ast::ModifierFlags::NONE);
        if modifier_flags.is_empty() {
            return None;
        }
        let modifiers = ast::create_modifiers_from_modifier_flags(
            modifier_flags & !ast::ModifierFlags::ACCESSOR,
            |kind| self.factory_mut().new_token(kind),
        );
        Some(self.factory_mut().new_modifier_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            modifiers,
            modifier_flags,
        ))
    }

    fn transform_constructor_body_worker(
        &mut self,
        mut statements_out: Vec<ast::Node>,
        statements_in: &[ast::Node],
        mut statement_offset: usize,
        super_path: &[usize],
        super_path_depth: usize,
        initializer_statements: &[ast::Node],
        constructor: ast::Node,
    ) -> Vec<ast::Node> {
        let super_statement_index = super_path[super_path_depth];
        let super_statement = statements_in[super_statement_index];

        // Visit statements before super
        let visited =
            self.visit_statement_slice(&statements_in[statement_offset..super_statement_index]);
        statements_out.extend(visited);
        statement_offset = super_statement_index + 1;

        if ast::is_try_statement(self.store_for(super_statement), super_statement) {
            let source = self.store_for(super_statement);
            let try_block = source
                .try_block(super_statement)
                .expect("try statement should have try block");
            let try_block_source = self.store_for(try_block);
            let try_block_statements_view = try_block_source
                .statements(try_block)
                .expect("try block should have statements");
            let try_block_statements = try_block_statements_view.iter().collect::<Vec<_>>();
            let try_block_statement_loc = try_block_statements_view.loc();
            let try_block_statement_range = try_block_statements_view.range();
            let try_block_multi_line = try_block_source.multi_line(try_block).unwrap_or(true);

            let try_block_statements = self.transform_constructor_body_worker(
                Vec::new(),
                &try_block_statements,
                0,
                super_path,
                super_path_depth + 1,
                initializer_statements,
                constructor,
            );
            let try_statement_list = self.factory_mut().new_node_list(
                try_block_statement_loc,
                try_block_statement_range,
                try_block_statements,
            );
            let updated_try_block = if try_block.store_id() == self.factory().store().store_id() {
                self.factory_mut()
                    .update_block(try_block, try_statement_list, try_block_multi_line)
            } else {
                let source = self.source;
                self.factory_mut().update_block_from_store(
                    source,
                    try_block,
                    try_statement_list,
                    try_block_multi_line,
                )
            };

            let (catch_clause, finally_block) = {
                let source = self.store_for(super_statement);
                (
                    source.catch_clause(super_statement),
                    source.finally_block(super_statement),
                )
            };
            let catch_clause = self.visit_node(catch_clause);
            let finally_block = self.visit_node(finally_block);
            let updated = if super_statement.store_id() == self.factory().store().store_id() {
                self.factory_mut().update_try_statement(
                    super_statement,
                    Some(updated_try_block),
                    catch_clause,
                    finally_block,
                )
            } else {
                let source = self.source;
                self.factory_mut().update_try_statement_from_store(
                    source,
                    super_statement,
                    Some(updated_try_block),
                    catch_clause,
                    finally_block,
                )
            };
            statements_out.push(updated);
        } else {
            let visited = self.visit_statement_slice(
                &statements_in[super_statement_index..super_statement_index + 1],
            );
            statements_out.extend(visited);

            // Add the property initializers. Transforms this:
            //
            //  public x = 1;
            //
            // Into this:
            //
            //  constructor() {
            //      this.x = 1;
            //  }
            //
            // If we do useDefineForClassFields, they'll be converted elsewhere.
            // We instead *remove* them from the transformed output at this stage.

            // parameter-property assignments should occur immediately after the prologue and `super()`,
            // so only count the statements that immediately follow.
            let original_constructor = self.emit_context.most_original(&constructor);
            while statement_offset < statements_in.len() {
                let stmt = statements_in[statement_offset];
                let orig = self.emit_context.most_original(&stmt);
                if self.is_parameter_property_declaration(orig, original_constructor) {
                    statement_offset += 1;
                } else {
                    break;
                }
            }

            statements_out.extend(initializer_statements.iter().copied());
        }

        // Visit remaining statements
        let visited = self.visit_statement_slice(&statements_in[statement_offset..]);
        statements_out.extend(visited);
        statements_out
    }

    fn find_super_statement_index_path(
        &self,
        statements: &[ast::Node],
        start: usize,
    ) -> Option<Vec<usize>> {
        self.find_super_statement_index_path_worker(statements, start)
    }

    fn find_super_statement_index_path_worker(
        &self,
        statements: &[ast::Node],
        start: usize,
    ) -> Option<Vec<usize>> {
        for i in start..statements.len() {
            let statement = statements[i];
            if self.get_super_call_from_statement(&statement) {
                return Some(vec![i]);
            } else if ast::is_try_statement(self.store_for(statement), statement) {
                let source = self.store_for(statement);
                let Some(try_block) = source.try_block(statement) else {
                    continue;
                };
                let Some(try_block_statements) = self.store_for(try_block).statements(try_block)
                else {
                    continue;
                };
                let try_block_statements = try_block_statements.iter().collect::<Vec<_>>();
                if let Some(mut result) =
                    self.find_super_statement_index_path_worker(&try_block_statements, 0)
                {
                    result.insert(0, i);
                    return Some(result);
                }
            }
        }
        None
    }

    fn visit_statement_slice(&mut self, statements: &[ast::Node]) -> Vec<ast::Node> {
        let mut out = Vec::new();
        let mut changed = false;
        for statement in statements {
            let visited = self.visit(statement);
            self.append_visited_node(*statement, visited, &mut out, &mut changed);
        }
        out
    }

    fn get_super_call_from_statement(&self, statement: &ast::Node) -> bool {
        if !ast::is_expression_statement(self.store_for(*statement), *statement) {
            return false;
        }
        let source = self.store_for(*statement);
        source.expression(*statement).is_some_and(|expression| {
            let source = self.store_for(expression);
            let expression = ast::skip_parentheses(source, expression);
            ast::is_super_call(self.store_for(expression), expression)
        })
    }

    fn create_synthetic_super_call(&mut self) -> ast::Node {
        let arguments = self.factory_mut().new_identifier("arguments");
        let argument = self.factory_mut().new_spread_element(Some(arguments));
        let super_token = self.factory_mut().new_token(ast::Kind::SuperKeyword);
        let arguments = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![argument],
        );
        let call = self.factory_mut().new_call_expression(
            Some(super_token),
            None,
            None,
            arguments,
            ast::NodeFlags::NONE,
        );
        self.factory_mut().new_expression_statement(Some(call))
    }

    fn is_untransformed_private_property(
        &self,
        member: ast::Node,
        force_transform_static_private_elements: bool,
    ) -> bool {
        let source = self.store_for(member);
        if source.kind(member) != ast::Kind::PropertyDeclaration
            || self
                .config
                .should_transform_private_elements_or_class_static_blocks
        {
            return false;
        }
        if force_transform_static_private_elements
            && ast::has_static_modifier(source, member)
            && source
                .name(member)
                .is_some_and(|name| ast::is_private_identifier(self.store_for(name), name))
        {
            return false;
        }
        source
            .name(member)
            .is_some_and(|name| ast::is_private_identifier(self.store_for(name), name))
    }

    fn preserve_property_declaration_without_initializer(
        &mut self,
        member: ast::Node,
    ) -> ast::Node {
        if member.store_id() == self.factory().store().store_id() {
            let (modifiers, name) = {
                let source = self.factory().store();
                let modifiers = source.source_modifiers(member).map(|modifiers| {
                    (
                        modifiers.loc(),
                        modifiers.range(),
                        modifiers.iter().collect::<Vec<_>>(),
                        modifiers.modifier_flags(),
                    )
                });
                (modifiers, source.name(member))
            };
            let modifiers = modifiers.map(|(loc, range, nodes, modifier_flags)| {
                self.factory_mut()
                    .new_modifier_list(loc, range, nodes, modifier_flags)
            });
            let updated = self.factory_mut().update_property_declaration(
                member,
                modifiers,
                name,
                None::<ast::Node>,
                None::<ast::Node>,
                None::<ast::Node>,
            );
            return self.finish_class_element(updated, member);
        }

        let (modifiers, name) = {
            let source = self.source;
            (
                source.source_modifiers(member).map(|modifiers| {
                    let nodes = modifiers.nodes();
                    (
                        nodes.loc(),
                        nodes.range(),
                        nodes.iter().collect::<Vec<_>>(),
                        modifiers.modifier_flags(),
                    )
                }),
                source.name(member),
            )
        };
        let modifiers = modifiers.map(|(loc, range, nodes, modifier_flags)| {
            let nodes = nodes
                .into_iter()
                .map(|node| self.preserve_node(node))
                .collect::<Vec<_>>();
            self.factory_mut()
                .new_modifier_list(loc, range, nodes, modifier_flags)
        });
        let name = name.map(|name| self.preserve_node(name));
        let source = self.source;
        let updated = self.factory_mut().update_property_declaration_from_store(
            source,
            member,
            modifiers,
            name,
            None::<ast::Node>,
            None::<ast::Node>,
            None::<ast::Node>,
        );
        self.finish_class_element(updated, member)
    }

    fn transform_static_property_initializer(
        &mut self,
        member: ast::Node,
        receiver: Option<ast::Node>,
        class_constructor: Option<ast::Node>,
        super_class_reference: Option<ast::Node>,
        force_transform_static_private_elements: bool,
    ) -> Option<ast::Node> {
        let source = self.store_for(member);
        if source.kind(member) != ast::Kind::PropertyDeclaration
            || !ast::has_static_modifier(source, member)
            || ast::is_auto_accessor_property_declaration(source, member)
        {
            return None;
        }
        let name = source.name(member)?;
        let is_private_name = ast::is_private_identifier(source, name);
        let initializer = source.initializer(member);
        if !is_private_name && !self.config.should_transform_initializers {
            return None;
        }
        if is_private_name
            && !self
                .config
                .should_transform_private_elements_or_class_static_blocks
            && !force_transform_static_private_elements
        {
            return None;
        }
        let legacy_decorated_static_initializer = self.member_is_in_legacy_decorated_class(member);
        if is_private_name && let Some(info) = self.private_static_field_info(name) {
            let info_storage_name = info.storage_name;
            let info_is_static = info.is_static;
            let storage_name = self.clone_node_for_reuse(info_storage_name);
            if let Some(initializer) = initializer
                && self.is_anonymous_class_needing_assigned_name(initializer)
            {
                let (assigned_name, _) = self.get_assigned_name_of_property_name(name);
                self.emit_context
                    .set_assigned_name(&initializer, &assigned_name);
            }
            let initializer = if let Some(initializer) = initializer {
                if self.config.should_transform_this_in_static_initializers {
                    let class_constructor = if legacy_decorated_static_initializer {
                        class_constructor.unwrap_or_else(|| self.decorated_static_invalid_this())
                    } else {
                        class_constructor?
                    };
                    let saved_class_static_block_receiver =
                        self.current_class_static_block_receiver;
                    let saved_class_static_super_context = self.current_class_static_super_context;
                    let saved_legacy_decorated_static_initializer =
                        self.current_legacy_decorated_static_initializer;
                    self.current_class_static_block_receiver = Some(class_constructor);
                    self.current_class_static_super_context =
                        (!legacy_decorated_static_initializer)
                            .then_some(())
                            .and_then(|_| {
                                super_class_reference.map(|super_class_reference| {
                                    StaticSuperContext {
                                        class_constructor,
                                        super_class_reference,
                                    }
                                })
                            });
                    self.current_legacy_decorated_static_initializer =
                        legacy_decorated_static_initializer;
                    let initializer = self.visit_node(Some(initializer))?;
                    self.current_class_static_block_receiver = saved_class_static_block_receiver;
                    self.current_class_static_super_context = saved_class_static_super_context;
                    self.current_legacy_decorated_static_initializer =
                        saved_legacy_decorated_static_initializer;
                    initializer
                } else {
                    self.visit_node(Some(initializer))?
                }
            } else {
                self.emit_context.factory.new_void_zero_expression()
            };
            let statement = if info_is_static {
                self.create_private_static_field_initializer(
                    storage_name,
                    initializer,
                    Some(member),
                )
            } else {
                let receiver = receiver.or(class_constructor)?;
                self.create_private_instance_field_initializer_for_receiver(
                    storage_name,
                    receiver,
                    initializer,
                )
            };
            self.set_property_initializer_statement_ranges(statement, member);
            return Some(statement);
        }
        let receiver = receiver?;
        let statement = if self.config.should_transform_initializers_using_define {
            let saved_class_static_block_receiver = self.current_class_static_block_receiver;
            let saved_class_static_super_context = self.current_class_static_super_context;
            let saved_legacy_decorated_static_initializer =
                self.current_legacy_decorated_static_initializer;
            let initializer_this = if legacy_decorated_static_initializer {
                class_constructor.unwrap_or_else(|| self.decorated_static_invalid_this())
            } else {
                class_constructor.unwrap_or(receiver)
            };
            self.current_class_static_block_receiver = Some(initializer_this);
            self.current_class_static_super_context = (!legacy_decorated_static_initializer)
                .then_some(())
                .and_then(|_| {
                    super_class_reference.map(|super_class_reference| StaticSuperContext {
                        class_constructor: initializer_this,
                        super_class_reference,
                    })
                });
            self.current_legacy_decorated_static_initializer = legacy_decorated_static_initializer;
            let expression = self.transform_property(member, receiver)?;
            self.current_class_static_block_receiver = saved_class_static_block_receiver;
            self.current_class_static_super_context = saved_class_static_super_context;
            self.current_legacy_decorated_static_initializer =
                saved_legacy_decorated_static_initializer;
            self.factory_mut().new_expression_statement(expression)
        } else {
            let initializer = initializer?;
            let initializer = if self.config.should_transform_this_in_static_initializers {
                let saved_class_static_block_receiver = self.current_class_static_block_receiver;
                let saved_class_static_super_context = self.current_class_static_super_context;
                let saved_legacy_decorated_static_initializer =
                    self.current_legacy_decorated_static_initializer;
                let initializer_this = if legacy_decorated_static_initializer {
                    class_constructor.unwrap_or_else(|| self.decorated_static_invalid_this())
                } else {
                    class_constructor.unwrap_or(receiver)
                };
                self.current_class_static_block_receiver = Some(initializer_this);
                self.current_class_static_super_context = (!legacy_decorated_static_initializer)
                    .then_some(())
                    .and_then(|_| {
                        super_class_reference.map(|super_class_reference| StaticSuperContext {
                            class_constructor: initializer_this,
                            super_class_reference,
                        })
                    });
                self.current_legacy_decorated_static_initializer =
                    legacy_decorated_static_initializer;
                let initializer = self.visit_node(Some(initializer))?;
                self.current_class_static_block_receiver = saved_class_static_block_receiver;
                self.current_class_static_super_context = saved_class_static_super_context;
                self.current_legacy_decorated_static_initializer =
                    saved_legacy_decorated_static_initializer;
                initializer
            } else {
                self.visit_node(Some(initializer))?
            };
            let name = self.get_property_name_for_transform_property(member, name)?;
            let access = self.create_member_access_for_property_name(receiver, name)?;
            let equals_token = self.factory_mut().new_token(ast::Kind::EqualsToken);
            let assignment = self.factory_mut().new_binary_expression(
                None,
                access,
                None,
                equals_token,
                initializer,
            );
            let assignment = self.finish_transform_property(member, assignment);
            self.factory_mut().new_expression_statement(assignment)
        };
        self.set_property_initializer_statement_ranges(statement, member);
        Some(statement)
    }

    fn transform_private_static_property_initializer_to_class_static_block(
        &mut self,
        member: ast::Node,
        force_transform_static_private_elements: bool,
    ) -> Option<ast::Node> {
        if !force_transform_static_private_elements
            || self
                .config
                .should_transform_private_elements_or_class_static_blocks
        {
            return None;
        }
        let (name, initializer) = {
            let source = self.store_for(member);
            if source.kind(member) != ast::Kind::PropertyDeclaration
                || !ast::has_static_modifier(source, member)
                || ast::is_auto_accessor_property_declaration(source, member)
            {
                return None;
            }
            let name = source.name(member)?;
            if !ast::is_private_identifier(source, name) {
                return None;
            }
            (name, source.initializer(member))
        };
        let storage_name = {
            let info = self.private_static_field_info(name)?;
            self.clone_node_for_reuse(info.storage_name)
        };
        let initializer = initializer
            .and_then(|initializer| self.visit_node(Some(initializer)))
            .unwrap_or_else(|| self.emit_context.factory.new_void_zero_expression());
        let statement =
            self.create_private_static_field_initializer(storage_name, initializer, Some(member));
        self.set_property_initializer_statement_ranges(statement, member);
        let statement_list = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![statement],
        );
        let body = self.factory_mut().new_block(statement_list, true);
        Some(
            self.factory_mut()
                .new_class_static_block_declaration(None, Some(body)),
        )
    }

    fn transform_public_static_property_initializer_to_class_static_block(
        &mut self,
        member: ast::Node,
    ) -> Option<ast::Node> {
        let source = self.store_for(member);
        if !self.config.should_transform_initializers
            || source.kind(member) != ast::Kind::PropertyDeclaration
            || !ast::has_static_modifier(source, member)
            || ast::is_auto_accessor_property_declaration(source, member)
            || self
                .config
                .should_transform_private_elements_or_class_static_blocks
        {
            return None;
        }
        let name = source.name(member)?;
        if ast::is_private_identifier(source, name) {
            return None;
        }
        let this = self.factory_mut().new_token(ast::Kind::ThisKeyword);
        let statement = self.transform_property_or_class_static_block(member, this, None)?;
        self.emit_context
            .mark_emit_node(&statement, printer::EF_NO_COMMENTS);
        let statement_list = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![statement],
        );
        let body = self.factory_mut().new_block(statement_list, false);
        let static_block = self
            .factory_mut()
            .new_class_static_block_declaration(None, Some(body));
        let loc = self.store_for(member).loc(member);
        self.emit_context.set_original(&static_block, &member);
        self.emit_context.set_comment_range(&static_block, loc);
        Some(static_block)
    }

    fn update_property_declaration(&mut self, member: ast::Node) -> ast::Node {
        let same_store = member.store_id() == self.factory().store().store_id();
        let (modifiers, name, initializer) = {
            let source = self.store_for(member);
            (
                source.source_modifiers(member).map(|modifiers| {
                    let nodes = modifiers.nodes();
                    (
                        nodes.loc(),
                        nodes.range(),
                        nodes.iter().collect::<Vec<_>>(),
                        modifiers.modifier_flags(),
                    )
                }),
                source.name(member),
                source.initializer(member),
            )
        };
        let name = name.and_then(|name| self.visit_node(Some(name)));
        let initializer = initializer.and_then(|initializer| self.visit_node(Some(initializer)));
        let modifiers = modifiers.map(|(loc, range, nodes, modifier_flags)| {
            let nodes = nodes
                .into_iter()
                .map(|node| {
                    if same_store {
                        node
                    } else {
                        self.preserve_node(node)
                    }
                })
                .collect::<Vec<_>>();
            self.factory_mut()
                .new_modifier_list(loc, range, nodes, modifier_flags)
        });

        let updated = if same_store {
            self.factory_mut().update_property_declaration(
                member,
                modifiers,
                name,
                None::<ast::Node>,
                None::<ast::Node>,
                initializer,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_property_declaration_from_store(
                source,
                member,
                modifiers,
                name,
                None::<ast::Node>,
                None::<ast::Node>,
                initializer,
            )
        };
        self.finish_class_element(updated, member)
    }

    fn update_method_or_accessor_declaration(&mut self, member: ast::Node) -> ast::Node {
        let same_store = member.store_id() == self.factory().store().store_id();
        let (
            kind,
            modifiers,
            name,
            asterisk_token,
            postfix_token,
            type_parameters,
            parameters,
            type_node,
            full_signature,
            body,
        ) = {
            let source = self.store_for(member);
            (
                source.kind(member),
                self.snapshot_optional_modifier_list(source.source_modifiers(member)),
                source.name(member),
                source.asterisk_token(member),
                source.postfix_token(member),
                self.snapshot_optional_node_list(source.type_parameters(member)),
                self.snapshot_optional_node_list(source.parameters(member))
                    .expect("method or accessor should have parameters"),
                source.type_node(member),
                source.full_signature(member),
                source.body(member),
            )
        };

        let modifiers = self.preserve_optional_modifier_list_snapshot_with_allowed(
            modifiers,
            !ast::ModifierFlags::ACCESSOR,
        );
        let preserve_invalid_private_name = name.is_some_and(|name| {
            ast::is_private_identifier(self.store_for(name), name)
                && self
                    .private_accessor_info(name)
                    .is_some_and(|info| !info.is_valid)
        });
        let name = name.and_then(|name| {
            if preserve_invalid_private_name {
                Some(self.preserve_node(name))
            } else {
                self.visit_node(Some(name))
            }
        });
        let asterisk_token = asterisk_token.map(|node| self.preserve_node(node));
        let postfix_token = postfix_token.map(|node| self.preserve_node(node));
        let type_parameters = self.preserve_optional_node_list_snapshot(type_parameters);
        let parameters = {
            let old_flags = self.emit_context.begin_visit_parameters();
            let (loc, range, parameter_nodes, has_trailing_comma) = parameters;
            let mut visited = Vec::with_capacity(parameter_nodes.len());
            let mut changed = false;
            for parameter in parameter_nodes {
                let result = self.visit(&parameter);
                self.append_visited_node(parameter, result, &mut visited, &mut changed);
            }
            let (visited, _) = self
                .emit_context
                .finish_visit_parameters(old_flags, visited, changed);
            self.factory_mut().new_node_list_with_trailing_comma(
                loc,
                range,
                visited,
                has_trailing_comma,
            )
        };
        let type_node = type_node.map(|node| self.preserve_node(node));
        let full_signature = full_signature.map(|node| self.preserve_node(node));
        let body = self.visit_function_body(body);

        let updated = match kind {
            ast::Kind::MethodDeclaration => {
                if same_store {
                    self.factory_mut().update_method_declaration(
                        member,
                        modifiers,
                        asterisk_token,
                        name,
                        postfix_token,
                        type_parameters,
                        parameters,
                        type_node,
                        full_signature,
                        body,
                    )
                } else {
                    let source = self.source;
                    self.factory_mut().update_method_declaration_from_store(
                        source,
                        member,
                        modifiers,
                        asterisk_token,
                        name,
                        postfix_token,
                        type_parameters,
                        parameters,
                        type_node,
                        full_signature,
                        body,
                    )
                }
            }
            ast::Kind::GetAccessor => {
                if same_store {
                    self.factory_mut().update_get_accessor_declaration(
                        member,
                        modifiers,
                        name,
                        type_parameters,
                        parameters,
                        type_node,
                        full_signature,
                        body,
                    )
                } else {
                    let source = self.source;
                    self.factory_mut()
                        .update_get_accessor_declaration_from_store(
                            source,
                            member,
                            modifiers,
                            name,
                            type_parameters,
                            parameters,
                            type_node,
                            full_signature,
                            body,
                        )
                }
            }
            ast::Kind::SetAccessor => {
                if same_store {
                    self.factory_mut().update_set_accessor_declaration(
                        member,
                        modifiers,
                        name,
                        type_parameters,
                        parameters,
                        type_node,
                        full_signature,
                        body,
                    )
                } else {
                    let source = self.source;
                    self.factory_mut()
                        .update_set_accessor_declaration_from_store(
                            source,
                            member,
                            modifiers,
                            name,
                            type_parameters,
                            parameters,
                            type_node,
                            full_signature,
                            body,
                        )
                }
            }
            _ => member,
        };
        self.finish_class_element(updated, member)
    }

    fn transform_property_or_class_static_block(
        &mut self,
        property: ast::Node,
        receiver: ast::Node,
        super_class_reference: Option<ast::Node>,
    ) -> Option<ast::Node> {
        let expression =
            if ast::is_class_static_block_declaration(self.store_for(property), property) {
                self.transform_class_static_block_declaration(
                    property,
                    receiver,
                    super_class_reference,
                )?
            } else {
                self.transform_property(property, receiver)?
            };
        let statement = self.factory_mut().new_expression_statement(expression);
        self.set_property_initializer_statement_ranges(statement, property);

        // `setOriginalNode` *copies* the `emitNode` from `property`, so now both
        // `statement` and `expression` have a copy of the synthesized comments.
        // Drop the comments from expression to avoid printing them twice.
        self.emit_context
            .set_synthetic_leading_comments(&expression, Vec::new());
        self.emit_context
            .set_synthetic_trailing_comments(&expression, Vec::new());
        Some(statement)
    }

    fn transform_class_static_block_declaration(
        &mut self,
        node: ast::Node,
        receiver: ast::Node,
        super_class_reference: Option<ast::Node>,
    ) -> Option<ast::Node> {
        if !self
            .config
            .should_transform_private_elements_or_class_static_blocks
        {
            return None;
        }

        if self.is_class_this_assignment_block(node) {
            let saved_class_static_block_receiver = self.current_class_static_block_receiver;
            let saved_class_static_super_context = self.current_class_static_super_context;
            self.current_class_static_block_receiver = Some(receiver);
            self.current_class_static_super_context =
                super_class_reference.map(|super_class_reference| StaticSuperContext {
                    class_constructor: receiver,
                    super_class_reference,
                });
            let result = self.visit_node(self.first_statement_expression(node))?;
            self.current_class_static_block_receiver = saved_class_static_block_receiver;
            self.current_class_static_super_context = saved_class_static_super_context;
            // If the generated `_classThis` assignment is a noop (i.e., `_classThis = _classThis`), we can
            // eliminate the expression
            if ast::is_assignment_expression(self.store_for(result), result, true) {
                let source = self.store_for(result);
                if source.left(result) == source.right(result) {
                    return None;
                }
            }
            return Some(result);
        }

        if self.is_class_named_evaluation_helper_block(node) {
            let saved_class_static_block_receiver = self.current_class_static_block_receiver;
            let saved_class_static_super_context = self.current_class_static_super_context;
            self.current_class_static_block_receiver = Some(receiver);
            self.current_class_static_super_context =
                super_class_reference.map(|super_class_reference| StaticSuperContext {
                    class_constructor: receiver,
                    super_class_reference,
                });
            let result = self.visit_node(self.first_statement_expression(node));
            self.current_class_static_block_receiver = saved_class_static_block_receiver;
            self.current_class_static_super_context = saved_class_static_super_context;
            return result;
        }

        let (statement_nodes, statements_loc, statements_range) = {
            let body = self.store_for(node).body(node)?;
            let statements_list = self.store_for(body).statements(body)?;
            (
                statements_list.iter().collect::<Vec<_>>(),
                statements_list.loc(),
                statements_list.range(),
            )
        };
        let mut statements = Vec::with_capacity(statement_nodes.len());
        let mut changed = false;

        self.emit_context.start_variable_environment();
        let saved_class_static_block_receiver = self.current_class_static_block_receiver;
        let saved_class_static_super_context = self.current_class_static_super_context;
        self.current_class_static_block_receiver = Some(receiver);
        self.current_class_static_super_context =
            super_class_reference.map(|super_class_reference| StaticSuperContext {
                class_constructor: receiver,
                super_class_reference,
            });
        for statement in statement_nodes {
            let visited = self.visit(&statement);
            self.append_visited_node(statement, visited, &mut statements, &mut changed);
        }
        self.current_class_static_block_receiver = saved_class_static_block_receiver;
        self.current_class_static_super_context = saved_class_static_super_context;
        let statements = self
            .emit_context
            .end_and_merge_variable_environment(self.source, &statements);

        let iife = self.new_immediately_invoked_arrow_function(
            &statements,
            statements_loc,
            statements_range,
        );
        let callee = self.store_for(iife).expression(iife)?;
        let arrow_function = ast::skip_parentheses(self.store_for(callee), callee);
        self.emit_context.set_original(&arrow_function, &node);
        self.emit_context
            .mark_emit_node(&arrow_function, printer::EF_NO_LEXICAL_ARGUMENTS);
        self.emit_context.set_original(&iife, &node);
        self.emit_context.assign_source_map_range(&iife, &node);
        Some(iife)
    }

    fn transform_native_class_static_block_declaration(
        &mut self,
        node: ast::Node,
        receiver: ast::Node,
    ) -> Option<ast::Node> {
        if self.is_class_this_assignment_block(node)
            || self.is_class_named_evaluation_helper_block(node)
        {
            return None;
        }

        let (statement_nodes, statements_loc, statements_range, multi_line) = {
            let body = self.store_for(node).body(node)?;
            let statements_list = self.store_for(body).statements(body)?;
            (
                statements_list.iter().collect::<Vec<_>>(),
                statements_list.loc(),
                statements_list.range(),
                self.store_for(body).multi_line(body).unwrap_or(false),
            )
        };
        let mut statements = Vec::with_capacity(statement_nodes.len());
        let mut changed = false;

        let saved_class_static_block_receiver = self.current_class_static_block_receiver;
        let saved_preserves_static_auto_accessor_this =
            self.current_class_static_block_preserves_static_auto_accessor_this;
        self.current_class_static_block_receiver = Some(receiver);
        self.current_class_static_block_preserves_static_auto_accessor_this =
            self.emit_context.emit_flags(&node) & printer::EF_TRANSFORM_PRIVATE_STATIC_ELEMENTS
                != 0;
        for statement in statement_nodes {
            let visited = self.visit(&statement);
            self.append_visited_node(statement, visited, &mut statements, &mut changed);
        }
        self.current_class_static_block_receiver = saved_class_static_block_receiver;
        self.current_class_static_block_preserves_static_auto_accessor_this =
            saved_preserves_static_auto_accessor_this;

        if !changed {
            return None;
        }
        let statement_list =
            self.factory_mut()
                .new_node_list(statements_loc, statements_range, statements);
        let body = self.factory_mut().new_block(statement_list, multi_line);
        if node.store_id() == self.factory().store().store_id() {
            Some(self.factory_mut().update_class_static_block_declaration(
                node,
                None::<ast::ModifierList>,
                Some(body),
            ))
        } else {
            let source = self.source;
            let factory = &mut self.emit_context.factory.node_factory;
            Some(factory.update_class_static_block_declaration_from_store(
                source,
                node,
                None::<ast::ModifierList>,
                Some(body),
            ))
        }
    }

    fn first_statement_expression(&self, node: ast::Node) -> Option<ast::Node> {
        let body = self.store_for(node).body(node)?;
        let statements = self.store_for(body).statements(body)?;
        let statement = statements.first()?;
        self.store_for(statement).expression(statement)
    }

    fn is_function_assignment_statement(&self, node: ast::Node) -> bool {
        let expression = self.store_for(node).expression(node);
        let Some(expression) = expression else {
            return false;
        };
        if !ast::is_assignment_expression(self.store_for(expression), expression, true) {
            return false;
        }
        self.store_for(expression)
            .right(expression)
            .is_some_and(|right| self.store_for(right).kind(right) == ast::Kind::FunctionExpression)
    }

    fn is_assignment_to_class_this(&self, node: ast::Node, class_this: ast::Node) -> bool {
        if !ast::is_assignment_expression(self.store_for(node), node, true) {
            return false;
        }
        self.store_for(node)
            .left(node)
            .is_some_and(|left| left == class_this)
    }

    fn is_class_this_assignment_block(&mut self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        let Some(body) = source.body(node) else {
            return false;
        };
        let Some(statements) = self.store_for(body).statements(body) else {
            return false;
        };
        let Some(statement) = statements.first() else {
            return false;
        };
        let statement_source = self.store_for(statement);
        let expression = statement_source.expression(statement);
        let expression_source = expression.map(|expression| self.store_for(expression));
        let left = expression.and_then(|expression| expression_source?.left(expression));
        let right = expression.and_then(|expression| expression_source?.right(expression));
        let class_this = self.emit_context.class_this(&node);

        classthis::is_class_this_assignment_block_shape(
            ast::is_class_static_block_declaration(source, node),
            statements.len(),
            statement_source.kind(statement) == ast::Kind::ExpressionStatement,
            expression.is_some_and(|expression| {
                ast::is_assignment_expression(self.store_for(expression), expression, true)
            }),
            left.is_some_and(|left| ast::is_identifier(self.store_for(left), left)),
            left.zip(class_this)
                .is_some_and(|(left, class_this)| left == class_this),
            right.is_some_and(|right| self.store_for(right).kind(right) == ast::Kind::ThisKeyword),
        )
    }

    fn create_class_this_assignment_static_block(&mut self, class_this: ast::Node) -> ast::Node {
        let class_this = self.clone_node_for_reuse(class_this);
        let this = self.factory_mut().new_token(ast::Kind::ThisKeyword);
        let assignment = self
            .emit_context
            .factory
            .new_assignment_expression(class_this, this);
        let statement = self.factory_mut().new_expression_statement(assignment);
        let statement_list = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![statement],
        );
        let body = self.factory_mut().new_block(statement_list, false);
        let static_block = self
            .factory_mut()
            .new_class_static_block_declaration(None, Some(body));
        self.emit_context.set_class_this(&static_block, &class_this);
        static_block
    }

    fn create_class_named_evaluation_helper_block(
        &mut self,
        assigned_name: ast::Node,
    ) -> ast::Node {
        let this = self.factory_mut().new_token(ast::Kind::ThisKeyword);
        let set_function_name =
            self.emit_context
                .factory
                .new_set_function_name_helper(this, assigned_name, "");
        let statement = self
            .factory_mut()
            .new_expression_statement(set_function_name);
        let statement_list = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![statement],
        );
        let body = self.factory_mut().new_block(statement_list, false);
        let static_block = self
            .factory_mut()
            .new_class_static_block_declaration(None, Some(body));
        self.emit_context
            .set_assigned_name(&static_block, &assigned_name);
        static_block
    }

    fn new_immediately_invoked_arrow_function(
        &mut self,
        statements: &[ast::Node],
        statements_loc: core::TextRange,
        statements_range: core::TextRange,
    ) -> ast::Node {
        let parameters = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            Vec::<ast::Node>::new(),
        );
        let equals_greater_than = self
            .factory_mut()
            .new_token(ast::Kind::EqualsGreaterThanToken);
        let statement_list =
            self.factory_mut()
                .new_node_list(statements_loc, statements_range, statements.to_vec());
        let body = self.factory_mut().new_block(statement_list, true);
        let arrow = self.factory_mut().new_arrow_function(
            None::<ast::ModifierList>,
            None::<ast::NodeList>,
            parameters,
            None::<ast::Node>,
            None::<ast::Node>,
            Some(equals_greater_than),
            body,
        );
        let paren = self.factory_mut().new_parenthesized_expression(arrow);
        let arguments = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            Vec::<ast::Node>::new(),
        );
        self.factory_mut().new_call_expression(
            paren,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    // transformProperty transforms a property initializer into an assignment expression.
    fn transform_property(
        &mut self,
        property: ast::Node,
        receiver: ast::Node,
    ) -> Option<ast::Node> {
        if self.store_for(property).kind(property) != ast::Kind::PropertyDeclaration
            || ast::is_auto_accessor_property_declaration(self.store_for(property), property)
        {
            return None;
        }
        let name = self.store_for(property).name(property)?;
        let property_name = self.get_property_name_for_transform_property(property, name)?;
        let property_name_is_private =
            ast::is_private_identifier(self.store_for(property_name), property_name);
        let initializer_node = self.store_for(property).initializer(property);
        if let Some(initializer) = initializer_node
            && self.is_anonymous_class_needing_assigned_name(initializer)
        {
            let (assigned_name, _) = self.get_assigned_name_of_property_name(property_name);
            self.emit_context
                .set_assigned_name(&initializer, &assigned_name);
        }
        if property_name_is_private
            && ast::has_static_modifier(self.store_for(property), property)
            && !self
                .config
                .should_transform_private_elements_or_class_static_blocks
        {
            return None;
        }
        if property_name_is_private
            && let Some(info) = self.private_static_field_info(property_name)
        {
            let info_storage_name = info.storage_name;
            let info_is_static = info.is_static;
            let storage_name = self.clone_node_for_reuse(info_storage_name);
            let initializer = initializer_node.and_then(|initializer| {
                if !ast::has_static_modifier(self.store_for(property), property) {
                    let saved_class_static_block_receiver =
                        self.current_class_static_block_receiver;
                    let saved_class_static_super_context = self.current_class_static_super_context;
                    let saved_legacy_decorated_static_initializer =
                        self.current_legacy_decorated_static_initializer;
                    self.current_class_static_block_receiver = None;
                    self.current_class_static_super_context = None;
                    self.current_legacy_decorated_static_initializer = false;
                    let initializer = self.visit_node(Some(initializer));
                    self.current_class_static_block_receiver = saved_class_static_block_receiver;
                    self.current_class_static_super_context = saved_class_static_super_context;
                    self.current_legacy_decorated_static_initializer =
                        saved_legacy_decorated_static_initializer;
                    initializer
                } else {
                    self.visit_node(Some(initializer))
                }
            });
            let initializer = match initializer {
                Some(initializer) => initializer,
                None => self.emit_context.factory.new_void_zero_expression(),
            };
            let statement = if info_is_static {
                self.create_private_static_field_initializer(
                    storage_name,
                    initializer,
                    Some(property),
                )
            } else {
                self.create_private_instance_field_initializer_for_receiver(
                    storage_name,
                    receiver,
                    initializer,
                )
            };
            return Some(
                self.store_for(statement)
                    .expression(statement)
                    .unwrap_or(statement),
            );
        }
        if (property_name_is_private
            || ast::has_static_modifier(self.store_for(property), property))
            && self.store_for(property).initializer(property).is_none()
        {
            return None;
        }
        let property_original_node = self.emit_context.most_original(&property);
        // TODO: can we get rid of this original checking and better coordinate with runtimesyntax?
        if ast::has_abstract_modifier(
            self.store_for(property_original_node),
            property_original_node,
        ) {
            return None;
        }
        let mut initializer = if !ast::has_static_modifier(self.store_for(property), property) {
            let saved_class_static_block_receiver = self.current_class_static_block_receiver;
            let saved_class_static_super_context = self.current_class_static_super_context;
            let saved_legacy_decorated_static_initializer =
                self.current_legacy_decorated_static_initializer;
            self.current_class_static_block_receiver = None;
            self.current_class_static_super_context = None;
            self.current_legacy_decorated_static_initializer = false;
            let initializer =
                initializer_node.and_then(|initializer| self.visit_node(Some(initializer)));
            self.current_class_static_block_receiver = saved_class_static_block_receiver;
            self.current_class_static_super_context = saved_class_static_super_context;
            self.current_legacy_decorated_static_initializer =
                saved_legacy_decorated_static_initializer;
            initializer
        } else {
            initializer_node.and_then(|initializer| self.visit_node(Some(initializer)))
        };
        let is_parameter_property =
            self.is_parameter_property_declaration(property_original_node, property_original_node);
        if is_parameter_property && ast::is_identifier(self.store_for(property_name), property_name)
        {
            // A parameter-property declaration always overrides the initializer. The only time a parameter-property
            // declaration *should* have an initializer is when decorators have added initializers that need to run before
            // any other initializer
            let local_name = self.clone_node_preserve_location(property_name);
            if let Some(existing_initializer) = initializer {
                initializer = self
                    .emit_context
                    .factory
                    .inline_expressions(&[existing_initializer, local_name]);
            } else {
                initializer = Some(local_name);
            }
            self.emit_context.mark_emit_node(
                &property_name,
                printer::EF_NO_COMMENTS | printer::EF_NO_SOURCE_MAP,
            );
            let original_name = self
                .store_for(property_original_node)
                .name(property_original_node)
                .expect("parameter property should have a name");
            let loc = self.store_for(original_name).loc(original_name);
            self.emit_context.set_source_map_range(&local_name, loc);
            self.emit_context
                .mark_emit_node(&local_name, printer::EF_NO_COMMENTS);
        } else if initializer.is_none() {
            initializer = Some(self.emit_context.factory.new_void_zero_expression());
        }
        let initializer = initializer?;
        if !self.config.should_transform_initializers_using_define
            || ast::is_private_identifier(self.store_for(property_name), property_name)
        {
            let access = self.create_member_access_for_property_name(receiver, property_name)?;
            self.emit_context
                .mark_emit_node(&access, printer::EF_NO_LEADING_COMMENTS);
            let equals_token = self.factory_mut().new_token(ast::Kind::EqualsToken);
            let expression = self.factory_mut().new_binary_expression(
                None,
                access,
                None,
                equals_token,
                initializer,
            );
            Some(self.finish_transform_property(property, expression))
        } else {
            let name = self.create_define_property_name_expression(property_name)?;
            let descriptor = self.create_property_descriptor(initializer);
            let expression = self
                .emit_context
                .factory
                .new_object_define_property_call(receiver, name, descriptor);
            Some(self.finish_transform_property(property, expression))
        }
    }

    fn finish_transform_property(
        &mut self,
        property: ast::Node,
        expression: ast::Node,
    ) -> ast::Node {
        if ast::has_static_modifier(self.store_for(property), property)
            && self.current_class_has_lexical_environment_facts
            && let Some(name) = self.store_for(property).name(property)
        {
            // capture the lexical environment for the member
            let source_map_range = self.emit_context.source_map_range(&name);
            self.emit_context.set_original(&expression, &property);
            self.emit_context
                .set_source_map_range(&expression, source_map_range);
        }
        expression
    }

    fn create_define_property_name_expression(&mut self, name: ast::Node) -> Option<ast::Node> {
        let source = self.store_for(name);
        match source.kind(name) {
            ast::Kind::ComputedPropertyName => {
                let expression = source.expression(name)?;
                if let Some(cache_assignment) =
                    self.find_computed_property_name_cache_assignment(expression)
                    && let Some(left) = self.store_for(cache_assignment).left(cache_assignment)
                {
                    Some(self.preserve_node(left))
                } else {
                    Some(self.preserve_node(expression))
                }
            }
            ast::Kind::Identifier => {
                let text = source.text(name);
                Some(
                    self.factory_mut()
                        .new_string_literal(&text, ast::TokenFlags::NONE),
                )
            }
            ast::Kind::StringLiteral | ast::Kind::NumericLiteral | ast::Kind::BigIntLiteral => {
                Some(self.preserve_node(name))
            }
            ast::Kind::PrivateIdentifier => Some(self.preserve_node(name)),
            _ => None,
        }
    }

    fn create_property_descriptor(&mut self, value: ast::Node) -> ast::Node {
        let enumerable = self.emit_context.factory.new_true_expression();
        let configurable = self.emit_context.factory.new_true_expression();
        let writable = self.emit_context.factory.new_true_expression();
        let properties = vec![
            self.create_property_descriptor_assignment("enumerable", enumerable),
            self.create_property_descriptor_assignment("configurable", configurable),
            self.create_property_descriptor_assignment("writable", writable),
            self.create_property_descriptor_assignment("value", value),
        ];
        let properties = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            properties,
        );
        self.factory_mut()
            .new_object_literal_expression(properties, true)
    }

    fn create_property_descriptor_assignment(
        &mut self,
        name: &str,
        initializer: ast::Node,
    ) -> ast::Node {
        let name = self.factory_mut().new_identifier(name);
        self.factory_mut().new_property_assignment(
            None::<ast::ModifierList>,
            name,
            None,
            None,
            initializer,
        )
    }

    fn create_member_access_for_property_name(
        &mut self,
        receiver: ast::Node,
        name: ast::Node,
    ) -> Option<ast::Node> {
        let source = self.store_for(name);
        let location = source.loc(name);
        let expression = match source.kind(name) {
            ast::Kind::Identifier | ast::Kind::PrivateIdentifier => {
                let name_emit_flags = self.emit_context.emit_flags(&name);
                let name = self.preserve_node(name);
                if name_emit_flags != printer::EF_NONE {
                    self.emit_context.mark_emit_node(&name, name_emit_flags);
                }
                self.factory_mut().new_property_access_expression(
                    receiver,
                    None,
                    name,
                    ast::NodeFlags::NONE,
                )
            }
            ast::Kind::StringLiteral | ast::Kind::NumericLiteral | ast::Kind::BigIntLiteral => {
                let name_emit_flags = self.emit_context.emit_flags(&name);
                let name = self.preserve_node(name);
                if name_emit_flags != printer::EF_NONE {
                    self.emit_context.mark_emit_node(&name, name_emit_flags);
                }
                self.factory_mut().new_element_access_expression(
                    receiver,
                    None,
                    name,
                    ast::NodeFlags::NONE,
                )
            }
            ast::Kind::ComputedPropertyName => {
                let expression = source.expression(name)?;
                let expression = self.preserve_node(expression);
                let result = self.factory_mut().new_element_access_expression(
                    receiver,
                    None,
                    expression,
                    ast::NodeFlags::NONE,
                );
                self.factory_mut()
                    .place_emit_synthetic_node(result, location);
                return Some(result);
            }
            _ => return None,
        };
        self.emit_context.set_comment_range(&expression, location);
        self.emit_context
            .set_source_map_range(&expression, location);
        self.emit_context
            .mark_emit_node(&expression, printer::EF_NO_NESTED_SOURCE_MAPS);
        Some(expression)
    }

    fn transform_property_initializer(&mut self, member: ast::Node) -> Option<ast::Node> {
        if !self.config.should_transform_initializers {
            return None;
        }
        let (name, initializer, initializer_is_none) = {
            let source = self.store_for(member);
            if source.kind(member) != ast::Kind::PropertyDeclaration
                || ast::has_static_modifier(source, member)
                || ast::is_auto_accessor_property_declaration(source, member)
            {
                return None;
            }
            let name = source.name(member)?;
            let initializer = source.initializer(member);
            (name, initializer, initializer.is_none())
        };
        let property_original_node = self.emit_context.most_original(&member);
        let is_parameter_property =
            self.is_parameter_property_declaration(property_original_node, property_original_node);
        if ast::is_private_identifier(self.store_for(name), name)
            && let Some(info) = self.private_static_field_info(name)
        {
            let storage_name = info.storage_name;
            let info_is_static = info.is_static;
            if let Some(initializer) = initializer
                && self.is_anonymous_class_needing_assigned_name(initializer)
            {
                let (assigned_name, _) = self.get_assigned_name_of_property_name(name);
                self.emit_context
                    .set_assigned_name(&initializer, &assigned_name);
            }
            self.emit_context.start_variable_environment();
            let initializer = initializer
                .and_then(|initializer| self.visit_node(Some(initializer)))
                .unwrap_or_else(|| self.emit_context.factory.new_void_zero_expression());
            self.pending_instance_variable_declarations
                .extend(self.emit_context.end_variable_environment());
            let storage_name = self.clone_node_for_reuse(storage_name);
            let statement = if info_is_static {
                self.create_private_static_field_initializer(
                    storage_name,
                    initializer,
                    Some(member),
                )
            } else {
                self.create_private_instance_field_initializer(storage_name, initializer)
            };
            self.set_property_initializer_statement_ranges(statement, member);
            return Some(statement);
        }
        if let Some(expression) = self.get_property_name_expression_if_needed(member) {
            let mut expressions = Vec::new();
            self.flatten_comma_list(expression, &mut expressions);
            self.pending_expressions.extend(expressions);
        }
        if self.config.should_transform_initializers_using_set
            && initializer_is_none
            && !is_parameter_property
        {
            return None;
        }
        let this = self.factory_mut().new_token(ast::Kind::ThisKeyword);
        self.emit_context.start_variable_environment();
        let expression = self.transform_property(member, this);
        self.pending_instance_variable_declarations
            .extend(self.emit_context.end_variable_environment());
        let expression = expression?;
        let statement = self.factory_mut().new_expression_statement(expression);
        self.set_property_initializer_statement_ranges(statement, member);
        Some(statement)
    }

    fn find_computed_property_name_cache_assignment(
        &mut self,
        expression: ast::Node,
    ) -> Option<ast::Node> {
        let mut node = expression;
        loop {
            node = ast::skip_outer_expressions(
                self.store_for(node),
                node,
                ast::OuterExpressionKinds(0),
            );
            let source = self.store_for(node);
            if source.kind(node) == ast::Kind::BinaryExpression
                && source.operator_token(node).is_some_and(|operator| {
                    self.store_for(operator).kind(operator) == ast::Kind::CommaToken
                })
                && let Some(right) = source.right(node)
            {
                node = right;
                continue;
            }
            if ast::is_assignment_expression(source, node, true)
                && source
                    .left(node)
                    .is_some_and(|left| ast::is_identifier(self.store_for(left), left))
            {
                return Some(node);
            }
            break;
        }
        None
    }

    // flattenCommaList decomposes a comma expression tree into a sequence of expressions.
    fn flatten_comma_list(&self, node: ast::Node, out: &mut Vec<ast::Node>) {
        let source = self.store_for(node);
        if ast::is_parenthesized_expression(source, node)
            && ast::node_is_synthesized(source, node)
            && let Some(expression) = source.expression(node)
        {
            self.flatten_comma_list(expression, out);
        } else if source.kind(node) == ast::Kind::BinaryExpression
            && source.operator_token(node).is_some_and(|operator| {
                self.store_for(operator).kind(operator) == ast::Kind::CommaToken
            })
            && let (Some(left), Some(right)) = (source.left(node), source.right(node))
        {
            self.flatten_comma_list(left, out);
            self.flatten_comma_list(right, out);
        } else {
            out.push(node);
        }
    }

    // getPropertyNameExpressionIfNeeded transforms a computed property name, then either returns an expression
    // which caches the value of the result or the expression itself if the value is either unused or safe to
    // inline into multiple locations.
    // shouldHoist indicates whether the expression needs to be reused (i.e., for an initializer or a decorator).
    fn get_property_name_expression_if_needed(&mut self, member: ast::Node) -> Option<ast::Node> {
        let (name, should_hoist, expression) = {
            let source = self.store_for(member);
            if source.kind(member) != ast::Kind::PropertyDeclaration {
                return None;
            }
            let name = source.name(member)?;
            if !ast::is_computed_property_name(source, name) {
                return None;
            }
            (
                name,
                source.initializer(member).is_some()
                    || self.config.should_transform_initializers_using_define,
                source.expression(name)?,
            )
        };
        let cache_assignment = self.find_computed_property_name_cache_assignment(expression);
        // Switch to outer lex env for computed property name expressions, matching
        // Strada reference's onEmitNode behavior for ComputedPropertyName.
        let saved_class_static_block_receiver = self.current_class_static_block_receiver;
        let saved_class_static_super_context = self.current_class_static_super_context;
        let saved_inside_computed_property_name = self.inside_computed_property_name;
        self.current_class_static_block_receiver = self.previous_class_static_block_receiver;
        self.current_class_static_super_context = self.previous_class_static_super_context;
        self.inside_computed_property_name = true;
        let expression = self.visit_node(Some(expression));
        self.current_class_static_block_receiver = saved_class_static_block_receiver;
        self.current_class_static_super_context = saved_class_static_super_context;
        self.inside_computed_property_name = saved_inside_computed_property_name;
        let expression = expression?;
        let expression_source = self.store_for(expression);
        let inner_expression =
            ast::skip_partially_emitted_expressions(expression_source, expression);
        let inner_source = self.store_for(inner_expression);
        let inner_kind = inner_source.kind(inner_expression);
        let inner_is_identifier = ast::is_identifier(inner_source, inner_expression);
        let inner_is_generated_assignment =
            ast::is_assignment_expression(inner_source, inner_expression, true)
                && inner_source.left(inner_expression).is_some_and(|left| {
                    ast::is_identifier(self.store_for(left), left)
                        && self.emit_context.has_auto_generate_info(Some(&left))
                });
        let inlinable = module_transform_utilities::is_simple_inlineable_expression(
            inner_kind,
            inner_is_identifier,
        );
        let already_transformed = cache_assignment.is_some() || inner_is_generated_assignment;
        if !already_transformed && !inlinable && should_hoist {
            let original_name = self.emit_context.most_original(&name);
            let generated_name = self.emit_context.new_generated_name_for_node(original_name);
            if self.requires_block_scoped_var() {
                self.emit_context.add_lexical_declaration(generated_name);
            } else {
                self.emit_context.add_variable_declaration(generated_name);
            }
            return Some(
                self.emit_context
                    .factory
                    .new_assignment_expression(generated_name, expression),
            );
        }
        if inlinable || inner_is_identifier {
            None
        } else {
            Some(expression)
        }
    }

    fn set_property_initializer_statement_ranges(
        &mut self,
        statement: ast::Node,
        property: ast::Node,
    ) {
        let loc = self.store_for(property).loc(property);
        self.emit_context.set_original(&statement, &property);
        let emit_flags = self.emit_context.emit_flags(&property) & printer::EF_NO_COMMENTS;
        self.emit_context.mark_emit_node(&statement, emit_flags);
        self.emit_context.set_comment_range(&statement, loc);

        let property_original_node = self.emit_context.most_original(&property);
        if ast::is_parameter_declaration(
            self.store_for(property_original_node),
            property_original_node,
        ) {
            let loc = self
                .store_for(property_original_node)
                .loc(property_original_node);
            self.emit_context.set_source_map_range(&statement, loc);
            self.emit_context
                .mark_emit_node(&statement, printer::EF_NO_COMMENTS);
        } else {
            let source_map_range = move_range_past_modifiers(self.store_for(property), property);
            self.emit_context
                .set_source_map_range(&statement, source_map_range);
        }

        // If the property was originally an auto-accessor, don't emit comments here since they will be attached to
        // the synthesized getter.
        if ast::has_accessor_modifier(
            self.store_for(property_original_node),
            property_original_node,
        ) {
            self.emit_context
                .mark_emit_node(&statement, printer::EF_NO_COMMENTS);
        }
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
                out.push(self.preserve_node(original));
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

    fn lift_to_block_or_empty(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let Some(node) = node else {
            let statements = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                Vec::<ast::Node>::new(),
            );
            return Some(self.factory_mut().new_block(statements, true));
        };
        Some(self.lift_to_block(node))
    }

    fn lift_to_block(&mut self, node: ast::Node) -> ast::Node {
        let store = self.store_for(node);
        let nodes = if store.kind(node) == ast::Kind::SyntaxList {
            store
                .syntax_list_children(node)
                .expect("SyntaxList should have children")
                .iter()
                .flatten()
                .collect::<Vec<_>>()
        } else {
            vec![node]
        };
        let nodes = nodes
            .into_iter()
            .map(|node| self.preserve_node(node))
            .collect::<Vec<_>>();
        if nodes.len() == 1 {
            nodes[0]
        } else {
            let statements = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                nodes,
            );
            self.factory_mut().new_block(statements, true)
        }
    }

    fn clone_node_for_reuse(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
            return self.factory_mut().clone_node(node);
        }
        self.preserve_node(node)
    }

    fn clone_node_preserve_location(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
            return self
                .factory_mut()
                .deep_clone_node_in_current_store_preserve_location(node);
        }
        assert_eq!(
            node.store_id(),
            self.source.store_id(),
            "class fields transform cannot clone unrelated AST store"
        );
        let source = self.source;
        self.factory_mut()
            .deep_clone_node_from_store_preserve_location(source, node)
    }

    fn is_parameter_property_declaration(&self, node: ast::Node, parent: ast::Node) -> bool {
        let store = self.store_for(node);
        let parent = if parent.store_id() == node.store_id()
            && store.kind(parent) == ast::Kind::Constructor
        {
            parent
        } else if let Some(parent) = store.parent(node) {
            parent
        } else {
            return false;
        };
        ast::is_parameter_declaration(store, node)
            && ast::has_syntactic_modifier(
                store,
                node,
                ast::ModifierFlags::PARAMETER_PROPERTY_MODIFIER,
            )
            && store.kind(parent) == ast::Kind::Constructor
    }
}

impl<'source> ast::AstVisitEachChildRuntime<'source> for ClassFieldsRuntime<'_, 'source> {
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
            return node;
        }
        let source = self.source;
        self.import_state
            .preserve_node(source, &mut self.emit_context.factory.node_factory, node)
    }

    fn record_preserved_node(&mut self, source: ast::Node, imported: ast::Node) -> ast::Node {
        let imported = self.preserve_node(imported);
        self.import_state.record_preserved_node(
            source.store_id(),
            &mut self.emit_context.factory.node_factory,
            source,
            imported,
        )
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
        if node.store_id() == self.factory().store().store_id() {
            if source_unchanged {
                return node;
            }
            return self.factory_mut().update_source_file_in_current_store(
                node,
                statements.expect("source file statements cannot be removed"),
                end_of_file_token,
            );
        }
        let source = self.source;
        if source_unchanged {
            let imported = self.preserve_node(node);
            return self.record_preserved_node(node, imported);
        }
        self.import_state.update_source_file_from_store(
            source,
            &mut self.emit_context.factory.node_factory,
            node,
            statements.expect("source file statements cannot be removed"),
            end_of_file_token,
        )
    }

    fn visit_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let node = node?;
        let mut visited = self.visit(&node)?;
        let store = self.store_for(visited);
        if store.kind(visited) == ast::Kind::SyntaxList {
            let mut nodes = store
                .syntax_list_children(visited)
                .expect("SyntaxList should have children")
                .iter();
            let visited_slot = nodes
                .next()
                .expect("expected only a single node to be written to output");
            assert!(
                nodes.next().is_none(),
                "expected only a single node to be written to output"
            );
            visited = visited_slot?;
        }
        Some(self.preserve_node(visited))
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
            let result = self.visit(&node);
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
            Some(self.import_state.preserve_source_node_list_input(
                self.source,
                &mut self.emit_context.factory.node_factory,
                &nodes,
            ))
        }
    }

    fn visit_modifiers_input(
        &mut self,
        modifiers: Option<ast::SourceModifierListInput>,
    ) -> Option<ast::ModifierList> {
        let modifiers = modifiers?;
        let modifier_nodes = modifiers.nodes();
        let mut visited = Vec::with_capacity(modifier_nodes.len());
        let mut changed = false;
        for node in modifier_nodes.iter() {
            let result = if self.store_for(*node).kind(*node) == ast::Kind::AccessorKeyword
                && (self.config.should_transform_auto_accessors
                    || self.current_class_will_hoist_initializers_to_constructor)
                && self.store_for(*node).parent(*node).is_some_and(|parent| {
                    ast::is_class_like(self.store_for(parent), parent)
                        || ast::is_class_element(self.store_for(parent), parent)
                }) {
                None
            } else {
                self.visit(&node)
            };
            self.append_visited_node(*node, result, &mut visited, &mut changed);
        }
        if changed {
            Some(self.factory_mut().new_modifier_list(
                modifiers.loc(),
                modifiers.range(),
                visited,
                ast::ModifierFlags::NONE,
            ))
        } else {
            Some(self.import_state.preserve_source_modifier_list_input(
                self.source,
                &mut self.emit_context.factory.node_factory,
                &modifiers,
            ))
        }
    }

    fn visit_parameters_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        let nodes = nodes?;
        let old_flags = self.emit_context.begin_visit_parameters();
        let source_list = nodes.clone();
        let mut visited = Vec::with_capacity(source_list.len());
        let mut changed = false;
        for node in source_list.iter() {
            let result = self.visit(&node);
            self.append_visited_node(node, result, &mut visited, &mut changed);
        }
        let (visited, changed) = self
            .emit_context
            .finish_visit_parameters(old_flags, visited, changed);
        if changed {
            Some(self.factory_mut().new_node_list_with_trailing_comma(
                source_list.loc(),
                source_list.range(),
                visited,
                source_list.has_trailing_comma(),
            ))
        } else {
            Some(self.import_state.preserve_source_node_list_input(
                self.source,
                &mut self.emit_context.factory.node_factory,
                &nodes,
            ))
        }
    }

    fn visit_function_body(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let saved_pending_expressions = std::mem::take(&mut self.pending_expressions);
        let updated = self.visit_node(node);
        self.pending_expressions = saved_pending_expressions;
        self.emit_context.finish_visit_function_body(updated)
    }

    fn visit_iteration_body(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        node?;
        self.emit_context.begin_visit_iteration_body();
        let updated = self.visit_embedded_statement(node);
        self.emit_context.finish_visit_iteration_body(updated)
    }

    fn visit_top_level_statements_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        let nodes = nodes?;
        let source_list = nodes.clone();
        let mut visited = Vec::with_capacity(source_list.len());
        let mut changed = false;

        self.emit_context.start_variable_environment();
        for node in source_list.iter() {
            let result = self.visit(&node);
            self.append_visited_node(node, result, &mut visited, &mut changed);
        }
        let declarations = self.emit_context.end_variable_environment();
        let (visited, environment_changed) = self
            .emit_context
            .merge_environment_for_resolved_nodes(&visited, &declarations);

        if changed || environment_changed {
            Some(self.factory_mut().new_node_list_with_trailing_comma(
                source_list.loc(),
                source_list.range(),
                visited,
                source_list.has_trailing_comma(),
            ))
        } else {
            Some(self.import_state.preserve_source_node_list_input(
                self.source,
                &mut self.emit_context.factory.node_factory,
                &nodes,
            ))
        }
    }

    fn visit_embedded_statement(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        match node {
            Some(node) => {
                let visited = self.visit(&node);
                let lifted = self.lift_to_block_or_empty(visited);
                let updated = self
                    .emit_context
                    .finish_visit_embedded_statement(&node, lifted);
                updated.map(|updated| self.preserve_node(updated))
            }
            None => None,
        }
    }

    fn visit_raw_node_slice_input(
        &mut self,
        nodes: Option<ast::SourceRawNodeSliceInput>,
    ) -> Option<ast::RawNodeSlice> {
        let nodes = nodes?;
        let source_nodes = nodes.clone();
        let mut visited = Vec::with_capacity(source_nodes.iter().len());
        let mut changed = false;
        for node in source_nodes.iter() {
            let result = node.and_then(|node| self.visit_node(Some(node)));
            match (node, result) {
                (Some(original), Some(result))
                    if self.preserved_source_node_matches(Some(original), Some(result)) =>
                {
                    visited.push(Some(self.preserve_node(original)));
                }
                (_, Some(result)) => {
                    changed = true;
                    visited.push(Some(self.preserve_node(result)));
                }
                (None, None) => visited.push(None),
                (Some(_), None) => {
                    changed = true;
                    visited.push(None);
                }
            }
        }
        if changed {
            Some(self.factory_mut().new_raw_node_slice(visited))
        } else {
            Some(self.import_state.preserve_source_raw_node_slice_input(
                self.source,
                &mut self.emit_context.factory.node_factory,
                &nodes,
            ))
        }
    }
}

impl<'source> ast::AstGeneratedVisitEachChild<'source> for ClassFieldsRuntime<'_, 'source> {}
