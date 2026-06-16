use ts_ast as ast;
use ts_ast::{AstGeneratedVisitEachChild as _, AstVisitEachChildRuntime as _};
use ts_core::{self as core, ScriptTarget};
use ts_printer::{self as printer, AutoGenerateOptions, GeneratedIdentifierFlags};
use ts_scanner as scanner;

use crate::estransforms::utilities;

// Class/Decorator evaluation order, as it pertains to this transformer:
//
// 1. Class decorators are evaluated outside of the private name scope of the class.
//    - 15.8.20 RS: BindingClassDeclarationEvaluation
//    - 15.8.21 RS: Evaluation
//    - 8.3.5 RS: NamedEvaluation
// 2. ClassHeritage clause is evaluated outside of the private name scope of the class.
//    - 15.8.19 RS: ClassDefinitionEvaluation, Step 8.c.
// 3. The name of the class is assigned.
// 4. For each member:
//    a. Member Decorators are evaluated.
//       - 15.8.19 RS: ClassDefinitionEvaluation, Step 23.
//       - Probably 15.7.13 RS: ClassElementEvaluation, but it's missing from spec text.
//    b. Computed Property name is evaluated
//       - 15.8.19 RS: ClassDefinitionEvaluation, Step 23.
//       - 15.8.15 RS: ClassFieldDefinitionEvaluation, Step 1.
//       - 15.4.5 RS: MethodDefinitionEvaluation, Step 1.
// 5. Static non-field (method/getter/setter/auto-accessor) element decorators are applied
// 6. Non-static non-field (method/getter/setter/auto-accessor) element decorators are applied
// 7. Static field (excl. auto-accessor) element decorators are applied
// 8. Non-static field (excl. auto-accessor) element decorators are applied
// 9. Class decorators are applied
// 10. Class binding is initialized
// 11. Static method extra initializers are evaluated
// 12. Static fields are initialized (incl. extra initializers) and static blocks are evaluated
// 13. Class extra initializers are evaluated
//
// Class constructor evaluation order, as it pertains to this transformer:
//
// 1. Instance method extra initializers are evaluated
// 2. For each instance field/auto-accessor:
//    a. The field is initialized and defined on the instance.
//    b. Extra initializers for the field are evaluated.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EsDecoratorAction {
    Keep,
    VisitChildren,
    SkipTransformer,
    VisitSourceFile,
    ElideDecorator,
    TransformClassDeclaration,
    TransformClassExpression,
    VisitClassElementOnly,
    VisitParameter,
    VisitNamedEvaluationSite,
    VisitExportAssignment,
    SubstituteThis,
    VisitDiscardableExpression,
    VisitCallLike,
    VisitPropertyOrElementAccess,
    VisitComputedPropertyName,
    EnterOtherScope,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LexicalEntryKind {
    Class,
    ClassElement,
    Name,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecorationBucket {
    StaticNonField,
    NonStaticNonField,
    StaticField,
    NonStaticField,
    Class,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EsDecoratorFacts {
    pub experimental_decorators: bool,
    pub emit_script_target: ScriptTarget,
    pub use_define_for_class_fields: bool,
    pub subtree_contains_decorators: bool,
    pub class_this_available: bool,
    pub class_super_available: bool,
    pub subtree_contains_lexical_this: bool,
    pub subtree_contains_lexical_super: bool,
    pub class_has_decorators: bool,
    pub member_has_decorators: bool,
    pub member_is_static: bool,
    pub member_is_field: bool,
    pub member_is_private_or_auto_accessor: bool,
    pub class_has_static_private_elements: bool,
}

pub fn es_decorator_transformer_action(facts: EsDecoratorFacts) -> EsDecoratorAction {
    if facts.experimental_decorators
        || (facts.emit_script_target >= ScriptTarget::ESNext && facts.use_define_for_class_fields)
    {
        EsDecoratorAction::SkipTransformer
    } else {
        EsDecoratorAction::VisitChildren
    }
}

pub fn new_es_decorator_transformer_enabled(
    experimental_decorators: bool,
    emit_script_target: ScriptTarget,
    use_define_for_class_fields: bool,
) -> bool {
    // When experimentalDecorators is set, the legacy decorator transformer handles all
    // decorators. When targeting ESNext with useDefineForClassFields, there's nothing to
    // transform. In either case every node would be returned unchanged, so skip entirely.
    !(experimental_decorators
        || (emit_script_target >= ScriptTarget::ESNext && use_define_for_class_fields))
}

pub fn es_decorator_action_for_kind(kind: ast::Kind, facts: EsDecoratorFacts) -> EsDecoratorAction {
    if kind == ast::Kind::SourceFile {
        return EsDecoratorAction::VisitSourceFile;
    }
    if !should_visit_node(facts) {
        return EsDecoratorAction::Keep;
    }

    match kind {
        ast::Kind::Decorator => EsDecoratorAction::ElideDecorator,
        ast::Kind::ClassDeclaration => EsDecoratorAction::TransformClassDeclaration,
        ast::Kind::ClassExpression => EsDecoratorAction::TransformClassExpression,
        ast::Kind::Constructor
        | ast::Kind::PropertyDeclaration
        | ast::Kind::ClassStaticBlockDeclaration => EsDecoratorAction::VisitClassElementOnly,
        ast::Kind::Parameter => EsDecoratorAction::VisitParameter,
        ast::Kind::PropertyAssignment
        | ast::Kind::VariableDeclaration
        | ast::Kind::BindingElement => EsDecoratorAction::VisitNamedEvaluationSite,
        ast::Kind::ExportAssignment => EsDecoratorAction::VisitExportAssignment,
        ast::Kind::ThisKeyword => EsDecoratorAction::SubstituteThis,
        ast::Kind::BinaryExpression
        | ast::Kind::ExpressionStatement
        | ast::Kind::ParenthesizedExpression
        | ast::Kind::PartiallyEmittedExpression
        | ast::Kind::PrefixUnaryExpression
        | ast::Kind::PostfixUnaryExpression => EsDecoratorAction::VisitDiscardableExpression,
        ast::Kind::CallExpression | ast::Kind::TaggedTemplateExpression => {
            EsDecoratorAction::VisitCallLike
        }
        ast::Kind::PropertyAccessExpression | ast::Kind::ElementAccessExpression => {
            EsDecoratorAction::VisitPropertyOrElementAccess
        }
        ast::Kind::ComputedPropertyName => EsDecoratorAction::VisitComputedPropertyName,
        ast::Kind::MethodDeclaration
        | ast::Kind::SetAccessor
        | ast::Kind::GetAccessor
        | ast::Kind::FunctionExpression
        | ast::Kind::FunctionDeclaration => EsDecoratorAction::EnterOtherScope,
        _ => EsDecoratorAction::VisitChildren,
    }
}

pub fn should_visit_node(facts: EsDecoratorFacts) -> bool {
    facts.subtree_contains_decorators
        || (facts.class_this_available && facts.subtree_contains_lexical_this)
        || (facts.class_this_available
            && facts.class_super_available
            && facts.subtree_contains_lexical_super)
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

pub fn decoration_bucket(
    member_is_static: bool,
    member_is_field: bool,
    class_level: bool,
) -> DecorationBucket {
    if class_level {
        DecorationBucket::Class
    } else if member_is_static && member_is_field {
        DecorationBucket::StaticField
    } else if member_is_static {
        DecorationBucket::StaticNonField
    } else if member_is_field {
        DecorationBucket::NonStaticField
    } else {
        DecorationBucket::NonStaticNonField
    }
}

pub fn should_transform_private_static_elements_in_file(facts: EsDecoratorFacts) -> bool {
    facts.class_has_decorators && facts.class_has_static_private_elements
}

pub fn member_requires_static_private_tracking(facts: EsDecoratorFacts) -> bool {
    facts.member_is_static && facts.member_is_private_or_auto_accessor
}

pub fn finish_class_element(
    updated: ast::Node,
    original: ast::Node,
    source: &ast::AstStore,
    emit_context: &mut printer::EmitContext,
) -> ast::Node {
    finish_class_element_with_source_map_range(
        updated,
        original,
        crate::utilities::move_range_past_decorators(source, original),
        emit_context,
    )
}

pub fn finish_class_element_with_source_map_range(
    updated: ast::Node,
    original: ast::Node,
    source_map_range: core::TextRange,
    emit_context: &mut printer::EmitContext,
) -> ast::Node {
    if updated != original {
        // While we emit the source map for the node after skipping decorators and modifiers,
        // we need to emit the comments for the original range.
        emit_context.assign_comment_range(&updated, &original);
    }
    if updated != original || emit_context.source_map_range(&updated) != source_map_range {
        emit_context.set_source_map_range(&updated, source_map_range);
    }
    updated
}

pub fn visit_source_file_root(
    source_file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
    compiler_options: &core::CompilerOptions,
) -> ast::Node {
    if !new_es_decorator_transformer_enabled(
        compiler_options.experimental_decorators.is_true(),
        compiler_options.get_emit_script_target(),
        compiler_options.get_use_define_for_class_fields(),
    ) {
        return root;
    }

    let mut runtime = EsDecoratorRuntime {
        source: source_file.store(),
        emit_context,
        import_state: ast::AstImportState::new(),
        current_class_this: None,
        outer_this: None,
        capture_outer_this: false,
        emit_module_kind: compiler_options.get_emit_module_kind(),
    };
    runtime.visit_node(Some(root)).unwrap_or(root)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DecoratedMemberKind {
    Method,
    Field,
    AutoAccessor,
    Getter,
    Setter,
}

struct FieldInfo {
    member: ast::Node,
    name: ast::Node,
    referenced_name: Option<ast::Node>,
    decorators_name: ast::Node,
    descriptor_name: Option<ast::Node>,
    initializers_name: Option<ast::Node>,
    extra_initializers_name: ast::Node,
    is_static: bool,
    is_private: bool,
    kind: DecoratedMemberKind,
    name_omits_leading_comments: bool,
    this_arg: Option<ast::Node>,
}

#[derive(Clone, Copy)]
struct ClassInfo {
    decorators_name: ast::Node,
    descriptor_name: ast::Node,
    extra_initializers_name: ast::Node,
    class_this: ast::Node,
    class_super: Option<ast::Node>,
}

impl FieldInfo {
    fn is_non_field(&self) -> bool {
        matches!(
            self.kind,
            DecoratedMemberKind::Method
                | DecoratedMemberKind::AutoAccessor
                | DecoratedMemberKind::Getter
                | DecoratedMemberKind::Setter
        )
    }

    fn decorator_kind(&self) -> &'static str {
        match self.kind {
            DecoratedMemberKind::Method => "method",
            DecoratedMemberKind::Field => "field",
            DecoratedMemberKind::AutoAccessor => "accessor",
            DecoratedMemberKind::Getter => "getter",
            DecoratedMemberKind::Setter => "setter",
        }
    }
}

struct EsDecoratorRuntime<'ctx, 'source> {
    source: &'source ast::AstStore,
    emit_context: &'ctx mut printer::EmitContext,
    import_state: ast::AstImportState,
    current_class_this: Option<ast::Node>,
    outer_this: Option<ast::Node>,
    capture_outer_this: bool,
    emit_module_kind: core::ModuleKind,
}

impl EsDecoratorRuntime<'_, '_> {
    fn factory(&self) -> &ast::NodeFactory {
        &self.emit_context.factory.node_factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.emit_context.factory.node_factory
    }

    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        ast::AstTraversalState::store_for(self.source, self.factory(), node)
    }

    fn store_for_known_node(&self, node: ast::Node) -> &ast::AstStore {
        self.emit_context.store_for_node(node)
    }

    fn outer_this(&mut self) -> ast::Node {
        if let Some(outer_this) = self.outer_this {
            return outer_this;
        }
        let outer_this = self.emit_context.factory.new_unique_name_ex(
            "_outerThis",
            AutoGenerateOptions {
                flags: GeneratedIdentifierFlags::OPTIMISTIC,
                ..Default::default()
            },
        );
        self.outer_this = Some(outer_this);
        outer_this
    }

    fn outer_this_visit(&mut self, node: ast::Node) -> ast::Node {
        let source = self.store_for(node);
        if source.kind(node) != ast::Kind::ThisKeyword
            && source.subtree_facts(node) & ast::SubtreeFacts::CONTAINS_LEXICAL_THIS
                == ast::SubtreeFacts::NONE
        {
            return self.preserve_node(node);
        }

        let saved_capture_outer_this = self.capture_outer_this;
        let saved_class_this = self.current_class_this;
        self.capture_outer_this = true;
        self.current_class_this = None;
        let visited = self
            .visit_node(Some(node))
            .unwrap_or_else(|| self.preserve_node(node));
        self.current_class_this = saved_class_this;
        self.capture_outer_this = saved_capture_outer_this;
        visited
    }

    fn visit(&mut self, node: &ast::Node) -> Option<ast::Node> {
        let source = self.store_for(*node);
        match source.kind(*node) {
            ast::Kind::ClassDeclaration if self.is_decorated_class_like(*node) => {
                Some(self.visit_class_declaration(*node))
            }
            ast::Kind::ClassExpression if self.is_decorated_class_like(*node) => {
                Some(self.visit_class_expression(*node))
            }
            ast::Kind::PropertyAssignment => Some(self.visit_property_assignment(*node)),
            ast::Kind::ShorthandPropertyAssignment => {
                Some(self.visit_shorthand_property_assignment(*node))
            }
            ast::Kind::VariableDeclaration => Some(self.visit_named_evaluation_site(*node)),
            ast::Kind::Parameter => Some(self.visit_named_evaluation_site(*node)),
            ast::Kind::BindingElement => Some(self.visit_named_evaluation_site(*node)),
            ast::Kind::PropertyDeclaration => Some(self.visit_property_declaration(*node)),
            ast::Kind::BinaryExpression => Some(self.visit_binary_expression(*node)),
            ast::Kind::ExportAssignment => Some(self.visit_export_assignment(*node)),
            ast::Kind::ThisKeyword if self.capture_outer_this => Some(self.outer_this()),
            ast::Kind::ThisKeyword => Some(self.current_class_this.unwrap_or(*node)),
            // Decorators are elided. In Strada, a separate `modifierVisitor` drops decorators
            // before they reach `visitor` via visitEachChild. Here, `visit` serves as both
            // visitors, so decorators from modifier lists reach it directly.
            ast::Kind::Decorator => None,
            _ => self.generated_visit_each_child(node).into(),
        }
    }

    fn is_decorated_class_like(&self, node: ast::Node) -> bool {
        ast::has_decorators(self.store_for(node), node)
            || !self.decorated_public_class_elements(node).is_empty()
    }

    fn decorated_public_class_elements(&self, node: ast::Node) -> Vec<ast::Node> {
        let source = self.store_for(node);
        source
            .members(node)
            .map(|members| {
                members
                    .iter()
                    .filter(|member| {
                        matches!(
                            source.kind(*member),
                            ast::Kind::PropertyDeclaration
                                | ast::Kind::MethodDeclaration
                                | ast::Kind::GetAccessor
                                | ast::Kind::SetAccessor
                        ) && ast::has_decorators(source, *member)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn class_has_static_private_elements(&self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        source.members(node).is_some_and(|members| {
            members.iter().any(|member| {
                ast::has_static_modifier(source, member)
                    && (ast::is_private_identifier_class_element_declaration(source, member)
                        || ast::is_auto_accessor_property_declaration(source, member))
            })
        })
    }

    fn get_local_class_reference(&mut self, node: ast::Node) -> ast::Node {
        self.emit_context.get_local_name(node)
    }

    fn add_transform_private_static_elements_flags(&mut self, class_expr: ast::Node) {
        self.emit_context
            .mark_emit_node(&class_expr, printer::EF_TRANSFORM_PRIVATE_STATIC_ELEMENTS);
        let Some(members) = self.store_for(class_expr).members(class_expr) else {
            return;
        };
        let members = members.iter().collect::<Vec<_>>();
        for member in members {
            let source = self.store_for(member);
            if ast::has_static_modifier(source, member)
                && (ast::is_private_identifier_class_element_declaration(source, member)
                    || ast::is_auto_accessor_property_declaration(source, member))
            {
                self.emit_context
                    .mark_emit_node(&member, printer::EF_TRANSFORM_PRIVATE_STATIC_ELEMENTS);
            }
        }
    }

    fn create_common_js_default_export(&mut self, expression: ast::Node) -> ast::Node {
        let exports = self.factory_mut().new_identifier("exports");
        let default_name = self.factory_mut().new_identifier("default");
        let left = self.factory_mut().new_property_access_expression(
            exports,
            None,
            default_name,
            ast::NodeFlags::NONE,
        );
        let equals = self.factory_mut().new_token(ast::Kind::EqualsToken);
        let assignment = self
            .factory_mut()
            .new_binary_expression(None, left, None, equals, expression);
        self.factory_mut().new_expression_statement(assignment)
    }

    fn visit_class_declaration(&mut self, node: ast::Node) -> ast::Node {
        let (has_name, is_export, is_default, source_map_range) = {
            let source = self.store_for(node);
            (
                source.name(node).is_some(),
                ast::has_syntactic_modifier(source, node, ast::ModifierFlags::EXPORT),
                ast::has_syntactic_modifier(source, node, ast::ModifierFlags::DEFAULT),
                crate::utilities::move_range_past_decorators(source, node),
            )
        };
        if !has_name && !(is_export && is_default) {
            return self.generated_visit_each_child(&node);
        }
        if !has_name {
            let assigned_name = self
                .factory_mut()
                .new_string_literal("default", ast::TokenFlags::NONE);
            self.emit_context.set_assigned_name(&node, &assigned_name);
        }

        let iife = self.transform_class_like(node);

        if is_export && is_default {
            if !has_name {
                let export_statement = if self.emit_module_kind == core::ModuleKind::CommonJS {
                    self.create_common_js_default_export(iife)
                } else {
                    let export_statement = self.emit_context.factory.new_export_default(iife);
                    self.emit_context.set_original(&export_statement, &node);
                    self.emit_context
                        .assign_comment_range(&export_statement, &node);
                    self.emit_context
                        .set_source_map_range(&export_statement, source_map_range);
                    export_statement
                };
                return self.factory_mut().new_syntax_list(vec![export_statement]);
            }

            let decl_name = self.get_local_class_reference(node);
            let var_decl = self
                .factory_mut()
                .new_variable_declaration(decl_name, None, None, iife);
            self.emit_context.set_original(&var_decl, &node);
            let var_decl_list = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                vec![var_decl],
            );
            let var_decls = self
                .factory_mut()
                .new_variable_declaration_list(var_decl_list, ast::NodeFlags::LET);
            let var_statement = self
                .factory_mut()
                .new_variable_statement(None::<ast::ModifierList>, var_decls);
            self.emit_context.set_original(&var_statement, &node);

            let declaration_name = {
                let source = self.store_for(node);
                source
                    .name(node)
                    .expect("default exported class should have a name")
            };
            let declaration_name = self.preserve_node(declaration_name);
            let export_statement = if self.emit_module_kind == core::ModuleKind::CommonJS {
                self.create_common_js_default_export(declaration_name)
            } else {
                let export_statement = self
                    .emit_context
                    .factory
                    .new_export_default(declaration_name);
                self.emit_context.set_original(&export_statement, &node);
                self.emit_context
                    .assign_comment_range(&export_statement, &node);
                self.emit_context
                    .set_source_map_range(&export_statement, source_map_range);
                export_statement
            };
            return self
                .factory_mut()
                .new_syntax_list(vec![var_statement, export_statement]);
        }

        let decl_name = self.emit_context.get_local_name_ex(
            node,
            printer::AssignedNameOptions {
                allow_source_maps: true,
                ..Default::default()
            },
        );
        let var_decl = self
            .factory_mut()
            .new_variable_declaration(decl_name, None, None, iife);
        self.emit_context.set_original(&var_decl, &node);
        let var_decl_list = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![var_decl],
        );
        let var_decls = self
            .factory_mut()
            .new_variable_declaration_list(var_decl_list, ast::NodeFlags::LET);
        let var_statement = self
            .factory_mut()
            .new_variable_statement(None::<ast::ModifierList>, var_decls);
        self.emit_context.set_original(&var_statement, &node);
        self.emit_context
            .assign_comment_range(&var_statement, &node);

        if !is_export {
            return var_statement;
        }

        let export_statement = self
            .emit_context
            .factory
            .new_external_module_export(decl_name);
        self.emit_context.set_original(&export_statement, &node);
        self.factory_mut()
            .new_syntax_list(vec![var_statement, export_statement])
    }

    fn visit_class_expression(&mut self, node: ast::Node) -> ast::Node {
        let iife = self.transform_class_expression_with_static_blocks(node);
        self.emit_context.set_original(&iife, &node);
        iife
    }

    fn transform_class_expression_with_static_blocks(&mut self, node: ast::Node) -> ast::Node {
        // When a class has class decorators we end up transforming it into a statement that would otherwise give it an
        // assigned name. If the class doesn't have an assigned name, we'll give it an assigned name of `""`.
        if self.store_for(node).name(node).is_none()
            && self.emit_context.assigned_name(&node).is_none()
            && ast::class_or_constructor_parameter_is_decorated(self.store_for(node), false, node)
        {
            let assigned_name = self
                .factory_mut()
                .new_string_literal("", ast::TokenFlags::NONE);
            self.emit_context.set_assigned_name(&node, &assigned_name);
        }
        self.emit_context.start_variable_environment();

        // Before visiting we perform a first pass to collect information we'll need
        // as we descend.
        let decorated_members = self.decorated_public_class_elements(node);
        let class_has_decorators = ast::has_decorators(self.store_for(node), node);
        let _class_name = self.store_for(node).name(node);
        let should_transform_private_static_elements_in_class =
            class_has_decorators && self.class_has_static_private_elements(node);
        let needs_static_method_extra_initializers = decorated_members.iter().any(|member| {
            let source = self.store_for(*member);
            ast::has_static_modifier(source, *member)
                && matches!(
                    source.kind(*member),
                    ast::Kind::MethodDeclaration | ast::Kind::GetAccessor | ast::Kind::SetAccessor
                )
        });
        let needs_instance_method_extra_initializers = decorated_members.iter().any(|member| {
            let source = self.store_for(*member);
            !ast::has_static_modifier(source, *member)
                && matches!(
                    source.kind(*member),
                    ast::Kind::MethodDeclaration | ast::Kind::GetAccessor | ast::Kind::SetAccessor
                )
        });
        let static_method_extra_initializers_name = needs_static_method_extra_initializers
            .then(|| self.create_class_helper_variable("staticExtraInitializers"));
        let instance_method_extra_initializers_name = needs_instance_method_extra_initializers
            .then(|| self.create_class_helper_variable("instanceExtraInitializers"));
        if static_method_extra_initializers_name.is_some()
            || instance_method_extra_initializers_name.is_some()
        {
            self.emit_context.factory.request_run_initializers_helper();
        }
        let mut field_infos = decorated_members
            .into_iter()
            .map(|member| {
                self.create_field_info(
                    member,
                    static_method_extra_initializers_name,
                    instance_method_extra_initializers_name,
                )
            })
            .collect::<Vec<_>>();

        // 1. Class decorators are evaluated outside the private name scope of the class.
        //
        // - Since class decorators don't have privileged access to private names defined inside the class,
        //   they must be evaluated outside of the class body.
        // - Since a class decorator can replace the class constructor, we must define a variable to keep track
        //   of the mutated class.
        // - Since a class decorator can add extra initializers, we must define a variable to keep track of
        //   extra initializers.
        let class_super = self
            .class_extends_expression(node)
            .map(|_| self.create_class_helper_variable("classSuper"));
        let class_info = class_has_decorators.then(|| ClassInfo {
            decorators_name: self.create_class_helper_variable("classDecorators"),
            descriptor_name: self.create_class_helper_variable("classDescriptor"),
            extra_initializers_name: self.create_class_helper_variable("classExtraInitializers"),
            class_this: self.create_class_helper_variable("classThis"),
            class_super,
        });
        if let Some(class_info) = class_info {
            for info in field_infos.iter_mut().filter(|info| {
                info.is_static
                    && matches!(
                        info.kind,
                        DecoratedMemberKind::Field | DecoratedMemberKind::AutoAccessor
                    )
            }) {
                info.this_arg = Some(class_info.class_this);
            }
        }
        let mut class_definition_statements = Vec::new();
        if let Some(class_info) = class_info {
            let decorators =
                self.transform_all_decorators_of_declaration(self.get_decorators(node));
            let decorators_array = self.new_array_literal(decorators);
            class_definition_statements
                .push(self.create_let(class_info.decorators_name, Some(decorators_array)));
            class_definition_statements.push(self.create_let(class_info.descriptor_name, None));
            let extra_initializers = self.new_array_literal(Vec::new());
            class_definition_statements.push(
                self.create_let(class_info.extra_initializers_name, Some(extra_initializers)),
            );
            class_definition_statements.push(self.create_let(class_info.class_this, None));
        }

        // 2. ClassHeritage clause is evaluated outside of the private name scope of the class.
        if let Some(class_super) = class_super
            && let Some(extends_expression) = self.class_extends_expression(node)
        {
            let extends_expression = self
                .visit_node(Some(extends_expression))
                .unwrap_or(self.preserve_node(extends_expression));
            let safe_extends_expression = self.safe_extends_expression(extends_expression);
            class_definition_statements
                .push(self.create_let(class_super, Some(safe_extends_expression)));
        }
        let referenced_names = field_infos
            .iter()
            .filter_map(|info| info.referenced_name)
            .collect::<Vec<_>>();
        if !referenced_names.is_empty() {
            class_definition_statements
                .push(self.create_var_declaration_statement(&referenced_names));
        }
        if let Some(name) = static_method_extra_initializers_name {
            let initializers = self.new_array_literal(Vec::new());
            class_definition_statements.push(self.create_let(name, Some(initializers)));
        }
        if let Some(name) = instance_method_extra_initializers_name {
            let initializers = self.new_array_literal(Vec::new());
            class_definition_statements.push(self.create_let(name, Some(initializers)));
        }
        for info in field_infos.iter().filter(|info| info.is_static) {
            class_definition_statements.push(self.create_let(info.decorators_name, None));
            if let Some(initializers_name) = info.initializers_name {
                let initializers = self.new_array_literal(Vec::new());
                class_definition_statements
                    .push(self.create_let(initializers_name, Some(initializers)));
                let extra_initializers = self.new_array_literal(Vec::new());
                class_definition_statements
                    .push(self.create_let(info.extra_initializers_name, Some(extra_initializers)));
            }
            if let Some(descriptor_name) = info.descriptor_name {
                class_definition_statements.push(self.create_let(descriptor_name, None));
            }
        }
        for info in field_infos.iter().filter(|info| !info.is_static) {
            class_definition_statements.push(self.create_let(info.decorators_name, None));
            if let Some(initializers_name) = info.initializers_name {
                let initializers = self.new_array_literal(Vec::new());
                class_definition_statements
                    .push(self.create_let(initializers_name, Some(initializers)));
                let extra_initializers = self.new_array_literal(Vec::new());
                class_definition_statements
                    .push(self.create_let(info.extra_initializers_name, Some(extra_initializers)));
            }
            if let Some(descriptor_name) = info.descriptor_name {
                class_definition_statements.push(self.create_let(descriptor_name, None));
            }
        }

        // Since the constructor can appear anywhere in the class body and its transform depends on other class elements,
        // we must first visit all non-constructor members, then visit the constructor, all while maintaining document order.
        let saved_outer_this = self.outer_this;
        self.outer_this = None;
        let class_expr = self.create_class_expression_with_decorator_static_blocks(
            node,
            &field_infos,
            class_info,
            class_super,
            static_method_extra_initializers_name,
            instance_method_extra_initializers_name,
            should_transform_private_static_elements_in_class,
        );
        if let Some(outer_this) = self.outer_this {
            let this = self.emit_context.factory.new_this_expression();
            let outer_this_declaration = self.create_let(outer_this, Some(this));
            class_definition_statements.insert(0, outer_this_declaration);
        }
        self.outer_this = saved_outer_this;
        if should_transform_private_static_elements_in_class {
            self.add_transform_private_static_elements_flags(node);
            self.add_transform_private_static_elements_flags(class_expr);
        }
        if let Some(class_info) = class_info {
            let class_reference = self.get_local_class_reference(node);
            class_definition_statements.push(self.create_var(class_reference, Some(class_expr)));
            let class_reference = self.get_local_class_reference(node);
            let return_expr = self
                .emit_context
                .factory
                .new_assignment_expression(class_reference, class_info.class_this);
            class_definition_statements.push(self.factory_mut().new_return_statement(return_expr));
        } else {
            // produces:
            //   return <classExpression>;
            class_definition_statements.push(self.factory_mut().new_return_statement(class_expr));
        }
        let declarations = self.emit_context.end_variable_environment();
        let (class_definition_statements, _) = self
            .emit_context
            .merge_environment_for_resolved_nodes(&class_definition_statements, &declarations);
        self.new_immediately_invoked_arrow_function(
            &class_definition_statements,
            core::undefined_text_range(),
            core::undefined_text_range(),
        )
    }

    fn transform_class_like(&mut self, node: ast::Node) -> ast::Node {
        self.transform_class_expression_with_static_blocks(node)
    }

    fn create_field_info(
        &mut self,
        member: ast::Node,
        static_method_extra_initializers_name: Option<ast::Node>,
        instance_method_extra_initializers_name: Option<ast::Node>,
    ) -> FieldInfo {
        let (name, kind, is_static, is_private, name_omits_leading_comments) = {
            let source = self.store_for(member);
            let name = source
                .name(member)
                .expect("decorated class element should have a name");
            // Determine decorator kind
            let kind = match source.kind(member) {
                ast::Kind::MethodDeclaration => DecoratedMemberKind::Method,
                ast::Kind::GetAccessor => DecoratedMemberKind::Getter,
                ast::Kind::SetAccessor => DecoratedMemberKind::Setter,
                ast::Kind::PropertyDeclaration
                    if ast::is_auto_accessor_property_declaration(source, member) =>
                {
                    DecoratedMemberKind::AutoAccessor
                }
                ast::Kind::PropertyDeclaration => DecoratedMemberKind::Field,
                kind => panic!("Unexpected decorated class element kind {kind:?}"),
            };
            let is_static = ast::has_static_modifier(source, member);
            let is_private = ast::is_private_identifier(source, name);
            let has_non_decorator_modifier =
                source.source_modifiers(member).is_some_and(|modifiers| {
                    modifiers
                        .iter()
                        .any(|modifier| source.kind(modifier) != ast::Kind::Decorator)
                });
            let name_omits_leading_comments = matches!(
                kind,
                DecoratedMemberKind::Method
                    | DecoratedMemberKind::Field
                    | DecoratedMemberKind::AutoAccessor
            ) && !has_non_decorator_modifier;
            (
                name,
                kind,
                is_static,
                is_private,
                name_omits_leading_comments,
            )
        };
        let create_name =
            |runtime: &mut Self, suffix: &str| runtime.create_helper_variable(member, suffix);
        let initializers_name = matches!(
            kind,
            DecoratedMemberKind::Field | DecoratedMemberKind::AutoAccessor
        )
        .then(|| create_name(self, "initializers"));
        let descriptor_name = (is_private
            && matches!(
                kind,
                DecoratedMemberKind::Method
                    | DecoratedMemberKind::AutoAccessor
                    | DecoratedMemberKind::Getter
                    | DecoratedMemberKind::Setter
            ))
        .then(|| create_name(self, "descriptor"));
        let referenced_name = self
            .computed_property_name_needs_reference(name)
            .then(|| self.emit_context.new_generated_name_for_node(name));
        let extra_initializers_name = if matches!(
            kind,
            DecoratedMemberKind::Method | DecoratedMemberKind::Getter | DecoratedMemberKind::Setter
        ) {
            if is_static {
                static_method_extra_initializers_name
                    .expect("static method extra initializers should be defined")
            } else {
                instance_method_extra_initializers_name
                    .expect("instance method extra initializers should be defined")
            }
        } else {
            create_name(self, "extraInitializers")
        };
        FieldInfo {
            member,
            name,
            referenced_name,
            decorators_name: create_name(self, "decorators"),
            descriptor_name,
            initializers_name,
            extra_initializers_name,
            is_static,
            is_private,
            kind,
            name_omits_leading_comments,
            this_arg: None,
        }
    }

    fn get_helper_variable_name(&self, node: ast::Node) -> String {
        let source = self.store_for(node);
        let name = source.name(node);
        let mut declaration_name = match name {
            Some(name)
                if ast::is_identifier(source, name)
                    && !self.emit_context.has_auto_generate_info(Some(&name)) =>
            {
                source.text(name)
            }
            Some(name)
                if ast::is_private_identifier(source, name)
                    && !self.emit_context.has_auto_generate_info(Some(&name)) =>
            {
                source
                    .text(name)
                    .strip_prefix('#')
                    .unwrap_or("")
                    .to_string()
            }
            Some(name)
                if ast::is_string_literal(source, name)
                    && scanner::is_identifier_text(
                        &source.text(name),
                        core::LANGUAGE_VARIANT_STANDARD,
                    ) =>
            {
                source.text(name)
            }
            _ if ast::is_class_like(source, node) => "class".to_string(),
            _ => "member".to_string(),
        };

        if ast::is_get_accessor_declaration(source, node) {
            declaration_name = format!("get_{declaration_name}");
        }
        if ast::is_set_accessor_declaration(source, node) {
            declaration_name = format!("set_{declaration_name}");
        }
        if name.is_some_and(|name| ast::is_private_identifier(source, name)) {
            declaration_name = format!("private_{declaration_name}");
        }
        if ast::has_static_modifier(source, node) {
            declaration_name = format!("static_{declaration_name}");
        }
        format!("_{declaration_name}")
    }

    fn create_helper_variable(&mut self, node: ast::Node, suffix: &str) -> ast::Node {
        self.emit_context.factory.new_unique_name_ex(
            &format!("{}_{}", self.get_helper_variable_name(node), suffix),
            AutoGenerateOptions {
                flags: GeneratedIdentifierFlags::OPTIMISTIC
                    | GeneratedIdentifierFlags::RESERVED_IN_NESTED_SCOPES,
                ..Default::default()
            },
        )
    }

    fn computed_property_name_needs_reference(&self, name: ast::Node) -> bool {
        let source = self.store_for(name);
        if source.kind(name) != ast::Kind::ComputedPropertyName {
            return false;
        }
        let Some(expression) = source.expression(name) else {
            return false;
        };
        !ast::is_property_name_literal(source, expression) || ast::is_identifier(source, expression)
    }

    fn member_has_non_simple_computed_property_name(&self, member: ast::Node) -> bool {
        let source = self.store_for(member);
        let Some(name) = source.name(member) else {
            return false;
        };
        if source.kind(name) != ast::Kind::ComputedPropertyName {
            return false;
        }
        let Some(expression) = source.expression(name) else {
            return false;
        };
        !self.is_simple_inlineable_expression(expression)
    }

    fn create_class_helper_variable(&mut self, suffix: &str) -> ast::Node {
        self.emit_context.factory.new_unique_name_ex(
            &format!("_{suffix}"),
            AutoGenerateOptions {
                flags: GeneratedIdentifierFlags::OPTIMISTIC
                    | GeneratedIdentifierFlags::RESERVED_IN_NESTED_SCOPES,
                ..Default::default()
            },
        )
    }

    fn class_extends_expression(&self, node: ast::Node) -> Option<ast::Node> {
        let source = self.store_for(node);
        let heritage_clauses = source.source_heritage_clauses(node)?;
        for clause in heritage_clauses.iter() {
            if source.token(clause) != Some(ast::Kind::ExtendsKeyword) {
                continue;
            }
            let types = source.source_types(clause)?;
            let first_type = types.iter().next()?;
            if ast::is_expression_with_type_arguments(self.store_for(first_type), first_type) {
                return self.store_for(first_type).expression(first_type);
            }
        }
        None
    }

    fn safe_extends_expression(&mut self, extends_expression: ast::Node) -> ast::Node {
        let unwrapped = ast::skip_outer_expressions(
            self.store_for(extends_expression),
            extends_expression,
            ast::OuterExpressionKinds::ALL,
        );
        let unwrapped_source = self.store_for(unwrapped);
        let needs_comma = (ast::is_class_expression(unwrapped_source, unwrapped)
            && unwrapped_source.name(unwrapped).is_none())
            || (ast::is_function_expression(unwrapped_source, unwrapped)
                && unwrapped_source.name(unwrapped).is_none())
            || ast::is_arrow_function(unwrapped_source, unwrapped);
        if !needs_comma {
            return extends_expression;
        }
        let zero = self
            .factory_mut()
            .new_numeric_literal("0", ast::TokenFlags::NONE);
        self.emit_context
            .factory
            .new_comma_expression(zero, extends_expression)
    }

    fn transform_class_heritage_clauses(
        &mut self,
        heritage_clauses: Option<Vec<ast::Node>>,
        class_super: Option<ast::Node>,
    ) -> Option<ast::NodeList> {
        heritage_clauses.map(|clauses| {
            let clauses = clauses
                .into_iter()
                .map(|clause| self.transform_heritage_clause_for_class_super(clause, class_super))
                .collect::<Vec<_>>();
            self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                clauses,
            )
        })
    }

    fn transform_heritage_clause_for_class_super(
        &mut self,
        clause: ast::Node,
        class_super: Option<ast::Node>,
    ) -> ast::Node {
        let Some(class_super) = class_super else {
            return self.preserve_node(clause);
        };
        if self.store_for(clause).token(clause) != Some(ast::Kind::ExtendsKeyword) {
            return self.preserve_node(clause);
        }
        let Some(types) = self.store_for(clause).source_types(clause) else {
            return self.preserve_node(clause);
        };
        let types_loc = types.loc();
        let types_range = types.range();
        let has_trailing_comma = types.has_trailing_comma();
        let type_nodes = types.iter().collect::<Vec<_>>();
        let mut updated_types = Vec::with_capacity(type_nodes.len());
        for (index, type_node) in type_nodes.into_iter().enumerate() {
            if index == 0
                && ast::is_expression_with_type_arguments(self.store_for(type_node), type_node)
            {
                let updated = if type_node.store_id() == self.factory().store().store_id() {
                    self.factory_mut().update_expression_with_type_arguments(
                        type_node,
                        class_super,
                        None::<ast::NodeList>,
                    )
                } else {
                    let source = self.source;
                    self.factory_mut()
                        .update_expression_with_type_arguments_from_store(
                            source,
                            type_node,
                            class_super,
                            None::<ast::NodeList>,
                        )
                };
                updated_types.push(updated);
            } else {
                updated_types.push(self.preserve_node(type_node));
            }
        }
        let types = self.factory_mut().new_node_list_with_trailing_comma(
            types_loc,
            types_range,
            updated_types,
            has_trailing_comma,
        );
        if clause.store_id() == self.factory().store().store_id() {
            self.factory_mut()
                .update_heritage_clause(clause, ast::Kind::ExtendsKeyword, types)
        } else {
            let source = self.source;
            self.factory_mut().update_heritage_clause_from_store(
                source,
                clause,
                ast::Kind::ExtendsKeyword,
                types,
            )
        }
    }

    fn create_class_expression_with_decorator_static_blocks(
        &mut self,
        node: ast::Node,
        field_infos: &[FieldInfo],
        class_info: Option<ClassInfo>,
        class_super: Option<ast::Node>,
        static_method_extra_initializers_name: Option<ast::Node>,
        instance_method_extra_initializers_name: Option<ast::Node>,
        should_transform_private_static_elements_in_class: bool,
    ) -> ast::Node {
        let (class_name, class_members, heritage_clauses) = {
            let source = self.store_for(node);
            let class_name = source.name(node);
            let class_members = source
                .members(node)
                .expect("class should have members")
                .iter()
                .collect::<Vec<_>>();
            let heritage_clauses = source
                .source_heritage_clauses(node)
                .map(|clauses| clauses.iter().collect::<Vec<_>>());
            (class_name, class_members, heritage_clauses)
        };
        let mut decorator_assignments_inlined = vec![false; field_infos.len()];
        let mut decorator_assignment_inline_starts = vec![None; field_infos.len()];
        let mut decorator_assignment_inline_ranges = vec![None; class_members.len()];
        let mut next_pending_decorator = 0;
        let mut pending_decorator_end = 0;
        for (member_index, member) in class_members.iter().copied().enumerate() {
            // Member decorators require privileged access to private names. However, computed property
            // evaluation occurs interspersed with decorator evaluation. This means that if we encounter
            // a computed property name we must inline decorator evaluation.
            if let Some((info_index, info)) = field_infos
                .iter()
                .enumerate()
                .find(|(_, info)| info.member == member)
            {
                pending_decorator_end = info_index + 1;
                if info.referenced_name.is_some() {
                    decorator_assignment_inline_starts[info_index] = Some(next_pending_decorator);
                    for inlined in decorator_assignments_inlined
                        .iter_mut()
                        .take(info_index + 1)
                        .skip(next_pending_decorator)
                    {
                        *inlined = true;
                    }
                    next_pending_decorator = info_index + 1;
                    pending_decorator_end = next_pending_decorator;
                }
            } else if next_pending_decorator < pending_decorator_end
                && self.member_has_non_simple_computed_property_name(member)
            {
                decorator_assignment_inline_ranges[member_index] =
                    Some((next_pending_decorator, pending_decorator_end));
                for inlined in decorator_assignments_inlined
                    .iter_mut()
                    .take(pending_decorator_end)
                    .skip(next_pending_decorator)
                {
                    *inlined = true;
                }
                next_pending_decorator = pending_decorator_end;
            }
        }

        let existing_named_evaluation_helper_block = class_members
            .iter()
            .copied()
            .find(|member| self.is_class_named_evaluation_helper_block(*member));

        let mut members = Vec::new();
        if let Some(class_info) = class_info
            && !should_transform_private_static_elements_in_class
        {
            members.push(self.create_class_this_assignment_static_block(class_info.class_this));
            if existing_named_evaluation_helper_block.is_none()
                && let Some(assigned_name) = self.emit_context.assigned_name(&node)
            {
                members.push(
                    self.create_class_named_evaluation_helper_block_with_this_arg(
                        assigned_name,
                        class_info.class_this,
                    ),
                );
            }
        } else if existing_named_evaluation_helper_block.is_none()
            && let Some(assigned_name) = self.emit_context.assigned_name(&node)
        {
            members.push(self.create_class_named_evaluation_helper_block(assigned_name));
        }
        if let Some(existing_named_evaluation_helper_block) = existing_named_evaluation_helper_block
        {
            members.push(self.preserve_node(existing_named_evaluation_helper_block));
        }
        let static_extra_initializers_in_decorator_block = (!self
            .class_has_static_initializers(node))
        .then(|| {
            field_infos
                .iter()
                .filter(|info| info.is_static && info.is_non_field())
                .last()
                .map(|info| info.extra_initializers_name)
        })
        .flatten();
        let class_extra_initializers_in_decorator_block = (!self
            .class_has_static_initializers(node))
        .then_some(class_info)
        .flatten()
        .map(|class_info| (class_info.class_this, class_info.extra_initializers_name));

        // 5. Static non-field element decorators are applied
        // 6. Non-static non-field element decorators are applied
        // 7. Static field element decorators are applied
        // 8. Non-static field element decorators are applied
        let decorator_static_block = self.create_decorator_static_block_with_inlined(
            field_infos,
            &decorator_assignments_inlined,
            static_extra_initializers_in_decorator_block,
            class_extra_initializers_in_decorator_block,
            class_info,
            class_super,
            node,
            class_name,
        );
        if should_transform_private_static_elements_in_class {
            self.emit_context.mark_emit_node(
                &decorator_static_block,
                printer::EF_TRANSFORM_PRIVATE_STATIC_ELEMENTS,
            );
        }
        members.push(decorator_static_block);

        let mut last_static_extra_initializer = static_method_extra_initializers_name;
        let mut last_instance_extra_initializer = instance_method_extra_initializers_name;
        let mut constructor_member = None;
        let mut constructor_member_index = None;
        for (member_index, mut member) in class_members.into_iter().enumerate() {
            if Some(member) == existing_named_evaluation_helper_block {
                continue;
            }
            let mut member_updated = false;
            if let Some((info_index, info)) = field_infos
                .iter()
                .enumerate()
                .find(|(_, info)| info.member == member)
            {
                let previous_extra_initializer = if info.is_static {
                    last_static_extra_initializer
                } else {
                    last_instance_extra_initializer
                };
                let previous_extra_initializer_source_map_range = previous_extra_initializer
                    .filter(|previous_extra_initializer| {
                        Some(*previous_extra_initializer) == static_method_extra_initializers_name
                            || Some(*previous_extra_initializer)
                                == instance_method_extra_initializers_name
                    })
                    .map(|_| self.class_initializers_source_map_range(node, class_name));
                let inlined_infos =
                    if let Some(first_inlined) = decorator_assignment_inline_starts[info_index] {
                        &field_infos[first_inlined..=info_index]
                    } else {
                        &field_infos[0..0]
                    };
                let updated = self.update_decorated_class_element(
                    info,
                    previous_extra_initializer,
                    previous_extra_initializer_source_map_range,
                    inlined_infos,
                );
                self.append_class_member_result(updated, &mut members);
                if matches!(
                    info.kind,
                    DecoratedMemberKind::Field | DecoratedMemberKind::AutoAccessor
                ) {
                    if info.is_static {
                        last_static_extra_initializer = Some(info.extra_initializers_name);
                    } else {
                        last_instance_extra_initializer = Some(info.extra_initializers_name);
                    }
                }
            } else if self.store_for(member).kind(member) == ast::Kind::Constructor {
                constructor_member = Some(member);
                constructor_member_index = Some(members.len());
                members.push(self.preserve_node(member));
            } else {
                if let Some((start, end)) = decorator_assignment_inline_ranges[member_index] {
                    member = self.update_class_element_with_decorator_assignments(
                        member,
                        &field_infos[start..end],
                    );
                    member_updated = true;
                }
                if self.store_for(member).kind(member) == ast::Kind::PropertyDeclaration {
                    let is_static = ast::has_static_modifier(self.store_for(member), member);
                    let pending = if is_static {
                        &mut last_static_extra_initializer
                    } else {
                        &mut last_instance_extra_initializer
                    };
                    if let Some(extra_initializers) = pending.take() {
                        member = self.update_property_with_pending_initializers(
                            member,
                            extra_initializers,
                            is_static,
                            class_info,
                        );
                        member_updated = true;
                    }
                }

                if let Some(class_info) = class_info
                    && let Some(class_super) = class_info.class_super
                    && self.store_for(member).kind(member) == ast::Kind::ClassStaticBlockDeclaration
                {
                    members.push(self.transform_static_block_with_class_super(
                        member,
                        class_info.class_this,
                        class_super,
                    ));
                } else if let Some(class_info) = class_info
                    && self.store_for(member).kind(member) == ast::Kind::ClassStaticBlockDeclaration
                {
                    members.push(
                        self.transform_static_block_with_class_this(member, class_info.class_this),
                    );
                } else if let Some(class_info) = class_info
                    && let Some(class_super) = class_info.class_super
                    && self.store_for(member).kind(member) == ast::Kind::PropertyDeclaration
                    && ast::has_static_modifier(self.store_for(member), member)
                {
                    members.push(self.transform_static_property_with_class_super(
                        member,
                        class_info.class_this,
                        class_super,
                    ));
                } else if let Some(class_info) = class_info
                    && self.store_for(member).kind(member) == ast::Kind::PropertyDeclaration
                    && ast::has_static_modifier(self.store_for(member), member)
                {
                    members.push(
                        self.transform_static_property_with_class_this(
                            member,
                            class_info.class_this,
                        ),
                    );
                } else if member_updated {
                    members.push(member);
                } else {
                    members.push(self.preserve_node(member));
                }
            }
        }

        if let Some(extra_initializers) = last_instance_extra_initializer {
            if let Some(index) = constructor_member_index {
                members[index] = self.visit_constructor_declaration_with_extra_initializers(
                    constructor_member.expect("constructor member index has a constructor member"),
                    extra_initializers,
                    class_super.is_some(),
                );
            } else {
                members.push(self.create_extra_initializers_constructor(
                    extra_initializers,
                    class_super.is_some(),
                ));
            }
        }
        let trailing_static_extra_initializers = last_static_extra_initializer
            .filter(|_| static_extra_initializers_in_decorator_block.is_none());
        let trailing_class_extra_initializers =
            class_info.filter(|_| class_extra_initializers_in_decorator_block.is_none());
        match (
            trailing_static_extra_initializers,
            trailing_class_extra_initializers,
        ) {
            (Some(extra_initializers), Some(class_info)) => {
                let static_statement = self.create_extra_initializers_statement_with_this_arg(
                    class_info.class_this,
                    extra_initializers,
                );
                let class_statement = self.create_class_extra_initializers_statement(
                    class_info.class_this,
                    class_info.extra_initializers_name,
                    node,
                    class_name,
                );
                members.push(self.new_static_block(vec![static_statement, class_statement], true));
            }
            (Some(extra_initializers), None) => {
                members.push(self.create_extra_initializers_static_block(extra_initializers));
            }
            (None, Some(class_info)) => {
                members.push(self.create_class_extra_initializers_static_block(
                    class_info.class_this,
                    class_info.extra_initializers_name,
                    node,
                    class_name,
                ));
            }
            (None, None) => {}
        }

        let members = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            members,
        );
        let name = if class_info.is_some() {
            None
        } else {
            self.preserve_optional_node(class_name)
        };
        let heritage_clauses = self.transform_class_heritage_clauses(heritage_clauses, class_super);
        let class_expr = self.factory_mut().new_class_expression(
            None::<ast::ModifierList>,
            name,
            None::<ast::NodeList>,
            heritage_clauses,
            members,
        );
        self.emit_context.set_original(&class_expr, &node);
        if let Some(class_info) = class_info {
            self.emit_context
                .set_class_this(&class_expr, &class_info.class_this);
        }
        class_expr
    }

    fn append_class_member_result(&mut self, result: ast::Node, members: &mut Vec<ast::Node>) {
        let store = self.store_for(result);
        if store.kind(result) == ast::Kind::SyntaxList {
            let nodes = store
                .syntax_list_children(result)
                .expect("SyntaxList should have children")
                .iter()
                .flatten()
                .collect::<Vec<_>>();
            members.extend(nodes.into_iter().map(|node| self.preserve_node(node)));
        } else {
            members.push(self.preserve_node(result));
        }
    }

    // Gets whether a node is a `static {}` block containing only a single call to the `__setFunctionName` helper where that
    // call's second argument is the value stored in the `assignedName` property of the block's `EmitNode`.
    // @internal
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

    fn create_class_named_evaluation_helper_block(
        &mut self,
        assigned_name: ast::Node,
    ) -> ast::Node {
        let class_this = self.emit_context.factory.new_this_expression();
        self.create_class_named_evaluation_helper_block_with_this_arg(assigned_name, class_this)
    }

    fn create_class_named_evaluation_helper_block_with_this_arg(
        &mut self,
        assigned_name: ast::Node,
        class_this: ast::Node,
    ) -> ast::Node {
        let set_function_name =
            self.emit_context
                .factory
                .new_set_function_name_helper(class_this, assigned_name, "");
        let statement = self
            .factory_mut()
            .new_expression_statement(set_function_name);
        let static_block = self.new_static_block(vec![statement], false);
        self.emit_context
            .set_assigned_name(&static_block, &assigned_name);
        static_block
    }

    fn visit_named_evaluation_site(&mut self, node: ast::Node) -> ast::Node {
        if ast::is_named_evaluation_source(self.store_for(node), node)
            && let Some(initializer) = self.store_for(node).initializer(node)
            && let Some(class_expr) = self.anonymous_class_needing_assigned_name(initializer)
            && let Some(name) = self.store_for(node).name(node)
        {
            let assigned_name = self.get_assigned_name_of_identifier(name, initializer);
            self.finish_transform_named_evaluation(initializer, class_expr, assigned_name);
        }
        self.generated_visit_each_child(&node)
    }

    fn visit_property_assignment(&mut self, node: ast::Node) -> ast::Node {
        // 13.2.5.5 RS: PropertyDefinitionEvaluation
        //   PropertyAssignment : PropertyName `:` AssignmentExpression
        //     ...
        //     5. If IsAnonymousFunctionDefinition(|AssignmentExpression|) is *true* and _isProtoSetter_ is *false*, then
        //        a. Let _popValue_ be ? NamedEvaluation of |AssignmentExpression| with argument _propKey_.
        //     ...
        if !ast::is_named_evaluation_source(self.store_for(node), node) {
            return self.generated_visit_each_child(&node);
        }
        let (modifiers_input, name, postfix_token, type_node, initializer) = {
            let source = self.store_for(node);
            (
                source
                    .source_modifiers(node)
                    .map(ast::SourceModifierListInput::from_source),
                source
                    .name(node)
                    .expect("NamedEvaluation property assignment should have a name"),
                source.postfix_token(node),
                source.r#type(node),
                source
                    .initializer(node)
                    .expect("NamedEvaluation property assignment should have initializer"),
            )
        };
        let Some(class_expr) = self.anonymous_class_needing_assigned_name(initializer) else {
            return self.generated_visit_each_child(&node);
        };
        let (assigned_name, name) = self.get_assigned_name_of_property_name(name);
        self.finish_transform_named_evaluation(initializer, class_expr, assigned_name);
        let modifiers = self.visit_modifiers_input(modifiers_input);
        let postfix_token = postfix_token.map(|token| self.preserve_node(token));
        let type_node = type_node.map(|type_node| self.preserve_node(type_node));
        let initializer = self
            .visit_node(Some(initializer))
            .unwrap_or_else(|| self.preserve_node(initializer));
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_property_assignment(
                node,
                modifiers,
                Some(name),
                postfix_token,
                type_node,
                Some(initializer),
            )
        } else {
            let source = self.source;
            self.factory_mut().update_property_assignment_from_store(
                source,
                node,
                modifiers,
                Some(name),
                postfix_token,
                type_node,
                Some(initializer),
            )
        }
    }

    fn visit_shorthand_property_assignment(&mut self, node: ast::Node) -> ast::Node {
        // 13.15.5.3 RS: PropertyDestructuringAssignmentEvaluation
        //   AssignmentProperty : IdentifierReference Initializer?
        //     ...
        //     4. If |Initializer?| is present and _v_ is *undefined*, then
        //        a. If IsAnonymousFunctionDefinition(|Initializer|) is *true*, then
        //           i. Set _v_ to ? NamedEvaluation of |Initializer| with argument _P_.
        //     ...
        if !ast::is_named_evaluation_source(self.store_for(node), node) {
            return self.generated_visit_each_child(&node);
        }
        let (modifiers_input, name, postfix_token, type_node, equals_token, initializer) = {
            let source = self.store_for(node);
            (
                source
                    .source_modifiers(node)
                    .map(ast::SourceModifierListInput::from_source),
                source
                    .name(node)
                    .expect("NamedEvaluation shorthand assignment should have a name"),
                source.postfix_token(node),
                source.r#type(node),
                source.equals_token(node),
                source
                    .object_assignment_initializer(node)
                    .expect("NamedEvaluation shorthand assignment should have initializer"),
            )
        };
        let Some(class_expr) = self.anonymous_class_needing_assigned_name(initializer) else {
            return self.generated_visit_each_child(&node);
        };
        let assigned_name = self.get_assigned_name_of_identifier(name, initializer);
        self.finish_transform_named_evaluation(initializer, class_expr, assigned_name);
        let modifiers = self.visit_modifiers_input(modifiers_input);
        let name = self.preserve_node(name);
        let postfix_token = postfix_token.map(|token| self.preserve_node(token));
        let type_node = type_node.map(|type_node| self.preserve_node(type_node));
        let equals_token = equals_token.map(|token| self.preserve_node(token));
        let initializer = self
            .visit_node(Some(initializer))
            .unwrap_or_else(|| self.preserve_node(initializer));
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_shorthand_property_assignment(
                node,
                modifiers,
                Some(name),
                postfix_token,
                type_node,
                equals_token,
                Some(initializer),
            )
        } else {
            let source = self.source;
            self.factory_mut()
                .update_shorthand_property_assignment_from_store(
                    source,
                    node,
                    modifiers,
                    Some(name),
                    postfix_token,
                    type_node,
                    equals_token,
                    Some(initializer),
                )
        }
    }

    fn visit_property_declaration(&mut self, node: ast::Node) -> ast::Node {
        // 10.2.1.3 RS: EvaluateBody
        //   Initializer : `=` AssignmentExpression
        //     ...
        //     3. If IsAnonymousFunctionDefinition(|AssignmentExpression|) is *true*, then
        //        a. Let _value_ be ? NamedEvaluation of |Initializer| with argument _functionObject_.[[ClassFieldInitializerName]].
        //     ...
        if !ast::is_named_evaluation_source(self.store_for(node), node) {
            return self.generated_visit_each_child(&node);
        }
        let (modifiers_input, name, postfix_token, type_node, initializer) = {
            let source = self.store_for(node);
            (
                source
                    .source_modifiers(node)
                    .map(ast::SourceModifierListInput::from_source),
                source
                    .name(node)
                    .expect("NamedEvaluation property declaration should have a name"),
                source.postfix_token(node),
                source.r#type(node),
                source
                    .initializer(node)
                    .expect("NamedEvaluation property declaration should have initializer"),
            )
        };
        let Some(class_expr) = self.anonymous_class_needing_assigned_name(initializer) else {
            return self.generated_visit_each_child(&node);
        };
        let (assigned_name, name) = self.get_assigned_name_of_property_name(name);
        self.finish_transform_named_evaluation(initializer, class_expr, assigned_name);
        let modifiers = self.visit_modifiers_input(modifiers_input);
        let postfix_token = postfix_token.map(|token| self.preserve_node(token));
        let type_node = type_node.map(|type_node| self.preserve_node(type_node));
        let initializer = self
            .visit_node(Some(initializer))
            .unwrap_or_else(|| self.preserve_node(initializer));
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_property_declaration(
                node,
                modifiers,
                Some(name),
                postfix_token,
                type_node,
                Some(initializer),
            )
        } else {
            let source = self.source;
            self.factory_mut().update_property_declaration_from_store(
                source,
                node,
                modifiers,
                Some(name),
                postfix_token,
                type_node,
                Some(initializer),
            )
        }
    }

    fn visit_binary_expression(&mut self, node: ast::Node) -> ast::Node {
        if ast::is_named_evaluation_source(self.store_for(node), node)
            && let Some(right) = self.store_for(node).right(node)
            && let Some(class_expr) = self.anonymous_class_needing_assigned_name(right)
            && let Some(left) = self.store_for(node).left(node)
        {
            let assigned_name = self.get_assigned_name_of_identifier(left, right);
            self.finish_transform_named_evaluation(right, class_expr, assigned_name);
        }
        self.generated_visit_each_child(&node)
    }

    fn visit_export_assignment(&mut self, node: ast::Node) -> ast::Node {
        // 16.2.3.7 RS: Evaluation
        //   ExportDeclaration : `export` `default` AssignmentExpression `;`
        //     1. If IsAnonymousFunctionDefinition(|AssignmentExpression|) is *true*, then
        //        a. Let _value_ be ? NamedEvaluation of |AssignmentExpression| with argument `"default"`.
        //     ...
        let Some(expression) = self.store_for(node).expression(node) else {
            return self.generated_visit_each_child(&node);
        };
        if let Some(class_expr) = self.anonymous_class_needing_assigned_name(expression) {
            let assigned_name = if self
                .store_for(node)
                .is_export_equals(node)
                .expect("export assignment should have export-equals flag")
            {
                self.factory_mut()
                    .new_string_literal("", ast::TokenFlags::NONE)
            } else {
                self.factory_mut()
                    .new_string_literal("default", ast::TokenFlags::NONE)
            };
            self.finish_transform_named_evaluation(expression, class_expr, assigned_name);
        }
        self.generated_visit_each_child(&node)
    }

    fn anonymous_class_needing_assigned_name(&self, node: ast::Node) -> Option<ast::Node> {
        let source = self.store_for(node);
        let inner = ast::skip_outer_expressions(source, node, ast::OuterExpressionKinds::ALL);
        let inner_source = self.store_for(inner);
        (ast::is_class_expression(inner_source, inner)
            && inner_source.name(inner).is_none()
            && self.is_decorated_class_like(inner))
        .then_some(inner)
    }

    fn finish_transform_named_evaluation(
        &mut self,
        expression: ast::Node,
        class_expr: ast::Node,
        assigned_name: ast::Node,
    ) {
        if self.can_ignore_empty_string_literal_in_assigned_name(expression)
            && ast::is_string_literal(self.store_for(assigned_name), assigned_name)
            && self.store_for(assigned_name).text(assigned_name).is_empty()
        {
            return;
        }
        self.emit_context
            .set_assigned_name(&class_expr, &assigned_name);
    }

    fn can_ignore_empty_string_literal_in_assigned_name(&self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        let inner = ast::skip_outer_expressions(source, node, ast::OuterExpressionKinds::ALL);
        let inner_source = self.store_for(inner);
        ast::is_class_expression(inner_source, inner)
            && inner_source.name(inner).is_none()
            && !ast::class_or_constructor_parameter_is_decorated(inner_source, false, inner)
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

        let Some(expression) = self.store_for(name).expression(name) else {
            let assigned_name = self
                .factory_mut()
                .new_string_literal("", ast::TokenFlags::NONE);
            return (assigned_name, self.preserve_node(name));
        };
        if ast::is_property_name_literal(self.store_for(expression), expression)
            && !ast::is_identifier(self.store_for(expression), expression)
        {
            let assigned_name = self.new_string_literal_from_node(expression);
            return (assigned_name, self.preserve_node(name));
        }

        let assigned_name = self.emit_context.new_generated_name_for_node(name);
        self.emit_context.add_variable_declaration(assigned_name);
        let expression = self
            .visit_node(Some(expression))
            .unwrap_or_else(|| self.preserve_node(expression));
        let key = self.emit_context.factory.new_prop_key_helper(expression);
        let assignment = self
            .emit_context
            .factory
            .new_assignment_expression(assigned_name, key);
        let name = if name.store_id() == self.factory().store().store_id() {
            self.factory_mut()
                .update_computed_property_name(name, Some(assignment))
        } else {
            let source = self.source;
            self.factory_mut().update_computed_property_name_from_store(
                source,
                name,
                Some(assignment),
            )
        };
        (assigned_name, name)
    }

    fn new_string_literal_from_node(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.source.store_id() {
            return self
                .emit_context
                .factory
                .new_string_literal_from_node(self.source, &node);
        }
        let text = self.store_for(node).text(node).to_owned();
        self.factory_mut()
            .new_string_literal(text, ast::TokenFlags::NONE)
    }

    fn create_class_this_assignment_static_block(&mut self, class_this: ast::Node) -> ast::Node {
        let this = self.emit_context.factory.new_this_expression();
        let assignment = self
            .emit_context
            .factory
            .new_assignment_expression(class_this, this);
        let statement = self.factory_mut().new_expression_statement(assignment);
        let static_block = self.new_static_block(vec![statement], false);
        self.emit_context.set_class_this(&static_block, &class_this);
        static_block
    }

    fn transform_static_block_with_class_this(
        &mut self,
        node: ast::Node,
        class_this: ast::Node,
    ) -> ast::Node {
        let Some(body) = self.store_for(node).body(node) else {
            return self.preserve_node(node);
        };
        let body_store = self.store_for(body);
        let multi_line = body_store.multi_line(body).unwrap_or(true);
        let Some(statements) = body_store.source_statements(body) else {
            return self.preserve_node(node);
        };
        let statement_nodes = statements.iter().collect::<Vec<_>>();
        self.emit_context.start_variable_environment();
        let saved_class_this = self.current_class_this;
        self.current_class_this = Some(class_this);
        let mut statements = statement_nodes
            .iter()
            .copied()
            .map(|statement| self.visit_node(Some(statement)).unwrap_or(statement))
            .collect::<Vec<_>>();
        self.current_class_this = saved_class_this;
        let mut var_statements = self.emit_context.end_variable_environment();
        if !var_statements.is_empty() {
            var_statements.extend(statements);
            statements = var_statements;
        }
        self.new_static_block(statements, multi_line)
    }

    fn transform_static_block_with_class_super(
        &mut self,
        node: ast::Node,
        class_this: ast::Node,
        class_super: ast::Node,
    ) -> ast::Node {
        let Some(body) = self.store_for(node).body(node) else {
            return self.preserve_node(node);
        };
        let Some(statements) = self.store_for(body).source_statements(body) else {
            return self.preserve_node(node);
        };
        let statement_nodes = statements.iter().collect::<Vec<_>>();
        self.emit_context.start_variable_environment();
        let mut statements = statement_nodes
            .iter()
            .copied()
            .map(|statement| {
                self.transform_static_super_node(statement, class_this, class_super)
                    .unwrap_or_else(|| self.preserve_node(statement))
            })
            .collect::<Vec<_>>();
        let mut var_statements = self.emit_context.end_variable_environment();
        if !var_statements.is_empty() {
            var_statements.extend(statements);
            statements = var_statements;
        }
        self.new_static_block(statements, true)
    }

    fn transform_static_property_with_class_this(
        &mut self,
        node: ast::Node,
        class_this: ast::Node,
    ) -> ast::Node {
        let Some(initializer) = self.store_for(node).initializer(node) else {
            return self.preserve_node(node);
        };
        let saved_class_this = self.current_class_this;
        self.current_class_this = Some(class_this);
        let initializer = self
            .visit_node(Some(initializer))
            .unwrap_or_else(|| self.preserve_node(initializer));
        self.current_class_this = saved_class_this;
        self.update_static_property_initializer(node, initializer)
    }

    fn transform_static_property_with_class_super(
        &mut self,
        node: ast::Node,
        class_this: ast::Node,
        class_super: ast::Node,
    ) -> ast::Node {
        let Some(initializer) = self.store_for(node).initializer(node) else {
            return self.preserve_node(node);
        };
        self.emit_context.start_variable_environment();
        let Some(initializer) =
            self.transform_static_super_node(initializer, class_this, class_super)
        else {
            self.emit_context.end_variable_environment();
            return self.preserve_node(node);
        };
        let declarations = self.emit_context.end_variable_environment();
        let initializer = if declarations.is_empty() {
            initializer
        } else {
            let mut statements = declarations;
            statements.push(self.factory_mut().new_return_statement(Some(initializer)));
            self.new_immediately_invoked_arrow_function(
                &statements,
                core::undefined_text_range(),
                core::undefined_text_range(),
            )
        };
        self.update_static_property_initializer(node, initializer)
    }

    fn update_static_property_initializer(
        &mut self,
        node: ast::Node,
        initializer: ast::Node,
    ) -> ast::Node {
        let (modifiers_input, name, postfix_token, type_node) = {
            let source = self.store_for(node);
            (
                source
                    .source_modifiers(node)
                    .map(ast::SourceModifierListInput::from_source),
                source.name(node),
                source.postfix_token(node),
                source.r#type(node),
            )
        };
        let modifiers = self.visit_modifiers_input(modifiers_input);
        let name = name.map(|name| self.preserve_node(name));
        let postfix_token = postfix_token.map(|token| self.preserve_node(token));
        let type_node = type_node.map(|type_node| self.preserve_node(type_node));
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_property_declaration(
                node,
                modifiers,
                name,
                postfix_token,
                type_node,
                initializer,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_property_declaration_from_store(
                source,
                node,
                modifiers,
                name,
                postfix_token,
                type_node,
                initializer,
            )
        }
    }

    fn transform_static_super_node(
        &mut self,
        node: ast::Node,
        class_this: ast::Node,
        class_super: ast::Node,
    ) -> Option<ast::Node> {
        match self.store_for(node).kind(node) {
            ast::Kind::ThisKeyword => Some(self.clone_node_for_reuse(class_this)),
            ast::Kind::ExpressionStatement => {
                let expression = self.store_for(node).expression(node)?;
                let expression = self.transform_static_super_node_with_discard(
                    expression,
                    class_this,
                    class_super,
                    true,
                )?;
                Some(self.factory_mut().new_expression_statement(expression))
            }
            ast::Kind::ParenthesizedExpression => {
                let expression = self.store_for(node).expression(node)?;
                let expression =
                    self.transform_static_super_node(expression, class_this, class_super)?;
                Some(if node.store_id() == self.factory().store().store_id() {
                    self.factory_mut()
                        .update_parenthesized_expression(node, expression)
                } else {
                    let source = self.source;
                    self.factory_mut()
                        .update_parenthesized_expression_from_store(source, node, expression)
                })
            }
            ast::Kind::BinaryExpression => {
                self.transform_static_super_binary_expression(node, class_this, class_super, false)
            }
            ast::Kind::PrefixUnaryExpression | ast::Kind::PostfixUnaryExpression => {
                self.transform_static_super_update_expression(node, class_this, class_super, false)
            }
            ast::Kind::CallExpression => {
                let expression = self.store_for(node).expression(node)?;
                if !ast::is_super_property(self.store_for(expression), expression) {
                    return None;
                }
                let target =
                    self.transform_super_property_get(expression, class_this, class_super)?;
                let this_arg = self.clone_node_for_reuse(class_this);
                let argument_nodes = self
                    .store_for(node)
                    .source_arguments(node)
                    .map(|arguments| arguments.iter().collect::<Vec<_>>())
                    .unwrap_or_default();
                let arguments = argument_nodes
                    .into_iter()
                    .map(|argument| self.preserve_node(argument))
                    .collect::<Vec<_>>();
                Some(self.emit_context.factory.new_function_call_call(
                    &target,
                    Some(&this_arg),
                    &arguments,
                ))
            }
            ast::Kind::TaggedTemplateExpression => {
                let tag = self.store_for(node).tag(node)?;
                if !ast::is_super_property(self.store_for(tag), tag) {
                    return None;
                }
                let target = self.transform_super_property_get(tag, class_this, class_super)?;
                let this_arg = self.clone_node_for_reuse(class_this);
                let invocation =
                    self.emit_context
                        .factory
                        .new_function_bind_call(target, this_arg, &[]);
                let template = self
                    .store_for(node)
                    .template(node)
                    .map(|template| self.preserve_node(template));
                let flags = self.store_for(node).flags(node);
                Some(if node.store_id() == self.factory().store().store_id() {
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
                })
            }
            ast::Kind::PropertyAccessExpression | ast::Kind::ElementAccessExpression => {
                self.transform_super_property_get(node, class_this, class_super)
            }
            _ => None,
        }
    }

    fn transform_static_super_node_with_discard(
        &mut self,
        node: ast::Node,
        class_this: ast::Node,
        class_super: ast::Node,
        result_is_discarded: bool,
    ) -> Option<ast::Node> {
        match self.store_for(node).kind(node) {
            ast::Kind::ParenthesizedExpression => {
                let expression = self.store_for(node).expression(node)?;
                let expression = self.transform_static_super_node_with_discard(
                    expression,
                    class_this,
                    class_super,
                    result_is_discarded,
                )?;
                Some(if node.store_id() == self.factory().store().store_id() {
                    self.factory_mut()
                        .update_parenthesized_expression(node, expression)
                } else {
                    let source = self.source;
                    self.factory_mut()
                        .update_parenthesized_expression_from_store(source, node, expression)
                })
            }
            ast::Kind::BinaryExpression => self.transform_static_super_binary_expression(
                node,
                class_this,
                class_super,
                result_is_discarded,
            ),
            ast::Kind::PrefixUnaryExpression | ast::Kind::PostfixUnaryExpression => self
                .transform_static_super_update_expression(
                    node,
                    class_this,
                    class_super,
                    result_is_discarded,
                ),
            _ => self.transform_static_super_node(node, class_this, class_super),
        }
    }

    fn transform_static_super_binary_expression(
        &mut self,
        node: ast::Node,
        class_this: ast::Node,
        class_super: ast::Node,
        result_is_discarded: bool,
    ) -> Option<ast::Node> {
        let (left, right, operator_kind, node_loc) = {
            let source = self.store_for(node);
            let operator = source.operator_token(node)?;
            let operator_kind = self.store_for(operator).kind(operator);
            let left = source.left(node)?;
            let right = source.right(node)?;
            (left, right, operator_kind, source.loc(node))
        };

        if ast::is_destructuring_assignment(self.store_for(node), node) {
            let left = self.transform_static_super_destructuring_assignment_target(
                left,
                class_this,
                class_super,
            );
            let right = self.preserve_node(right);
            let operator = self.factory_mut().new_token(operator_kind);
            let expression = self
                .factory_mut()
                .new_binary_expression(None, left, None, operator, right);
            self.emit_context
                .set_source_map_range(&expression, node_loc);
            return Some(expression);
        }

        if operator_kind != ast::Kind::EqualsToken && !ast::is_compound_assignment(operator_kind) {
            return None;
        }
        if !ast::is_super_property(self.store_for(left), left) {
            return None;
        }

        let mut setter_name = self.transform_static_super_property_name(left)?;
        let mut expression = self.preserve_node(right);

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
            let target = self.clone_node_for_reuse(class_super);
            let receiver = self.clone_node_for_reuse(class_this);
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

        let target = self.clone_node_for_reuse(class_super);
        let receiver = self.clone_node_for_reuse(class_this);
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

    fn transform_static_super_update_expression(
        &mut self,
        node: ast::Node,
        class_this: ast::Node,
        class_super: ast::Node,
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
            if !ast::is_super_property(self.store_for(operand), operand) {
                return None;
            }
            (kind, operator, operand, source.loc(node))
        };

        let mut setter_name = self.transform_static_super_property_name(operand)?;
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

        let target = self.clone_node_for_reuse(class_super);
        let receiver = self.clone_node_for_reuse(class_this);
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

        let target = self.clone_node_for_reuse(class_super);
        let receiver = self.clone_node_for_reuse(class_this);
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

    fn transform_static_super_destructuring_assignment_target(
        &mut self,
        node: ast::Node,
        class_this: ast::Node,
        class_super: ast::Node,
    ) -> ast::Node {
        if matches!(
            self.store_for(node).kind(node),
            ast::Kind::ObjectLiteralExpression | ast::Kind::ArrayLiteralExpression
        ) {
            return self.transform_static_super_assignment_pattern(node, class_this, class_super);
        }
        if ast::is_super_property(self.store_for(node), node) {
            return self.wrap_static_super_property_for_destructuring_target(
                node,
                class_this,
                class_super,
            );
        }
        self.preserve_node(node)
    }

    fn transform_static_super_assignment_pattern(
        &mut self,
        node: ast::Node,
        class_this: ast::Node,
        class_super: ast::Node,
    ) -> ast::Node {
        match self.store_for(node).kind(node) {
            ast::Kind::ObjectLiteralExpression => {
                let (properties, multi_line) = {
                    let source = self.store_for(node);
                    (
                        source.source_properties(node).map(|properties| {
                            (
                                properties.loc(),
                                properties.range(),
                                properties.has_trailing_comma(),
                                properties.iter().collect::<Vec<_>>(),
                            )
                        }),
                        source.multi_line(node).unwrap_or(false),
                    )
                };
                let Some((loc, range, has_trailing_comma, properties)) = properties else {
                    return self.preserve_node(node);
                };
                let properties = properties
                    .into_iter()
                    .map(|property| {
                        self.transform_static_super_assignment_pattern_property(
                            property,
                            class_this,
                            class_super,
                        )
                    })
                    .collect::<Vec<_>>();
                let properties = self.factory_mut().new_node_list_with_trailing_comma(
                    loc,
                    range,
                    properties,
                    has_trailing_comma,
                );
                if node.store_id() == self.factory().store().store_id() {
                    self.factory_mut()
                        .update_object_literal_expression(node, properties, multi_line)
                } else {
                    let source = self.source;
                    self.factory_mut()
                        .update_object_literal_expression_from_store(
                            source, node, properties, multi_line,
                        )
                }
            }
            ast::Kind::ArrayLiteralExpression => {
                let (elements, multi_line) = {
                    let source = self.store_for(node);
                    (
                        source.source_elements(node).map(|elements| {
                            (
                                elements.loc(),
                                elements.range(),
                                elements.has_trailing_comma(),
                                elements.iter().collect::<Vec<_>>(),
                            )
                        }),
                        source.multi_line(node).unwrap_or(false),
                    )
                };
                let Some((loc, range, has_trailing_comma, elements)) = elements else {
                    return self.preserve_node(node);
                };
                let elements = elements
                    .into_iter()
                    .map(|element| {
                        self.transform_static_super_destructuring_assignment_target(
                            element,
                            class_this,
                            class_super,
                        )
                    })
                    .collect::<Vec<_>>();
                let elements = self.factory_mut().new_node_list_with_trailing_comma(
                    loc,
                    range,
                    elements,
                    has_trailing_comma,
                );
                if node.store_id() == self.factory().store().store_id() {
                    self.factory_mut()
                        .update_array_literal_expression(node, elements, multi_line)
                } else {
                    let source = self.source;
                    self.factory_mut()
                        .update_array_literal_expression_from_store(
                            source, node, elements, multi_line,
                        )
                }
            }
            _ => self.transform_static_super_destructuring_assignment_target(
                node,
                class_this,
                class_super,
            ),
        }
    }

    fn transform_static_super_assignment_pattern_property(
        &mut self,
        property: ast::Node,
        class_this: ast::Node,
        class_super: ast::Node,
    ) -> ast::Node {
        match self.store_for(property).kind(property) {
            ast::Kind::ShorthandPropertyAssignment => {
                let (name, postfix_token, type_node, equals_token, object_assignment_initializer) = {
                    let source = self.store_for(property);
                    (
                        source.name(property),
                        source.postfix_token(property),
                        source.r#type(property),
                        source.equals_token(property),
                        source.object_assignment_initializer(property),
                    )
                };
                let object_assignment_initializer =
                    object_assignment_initializer.and_then(|initializer| {
                        self.transform_static_super_node(initializer, class_this, class_super)
                            .or_else(|| Some(self.preserve_node(initializer)))
                    });
                if property.store_id() == self.factory().store().store_id() {
                    self.factory_mut().update_shorthand_property_assignment(
                        property,
                        None::<ast::ModifierList>,
                        name,
                        postfix_token,
                        type_node,
                        equals_token,
                        object_assignment_initializer,
                    )
                } else {
                    let source = self.source;
                    self.factory_mut()
                        .update_shorthand_property_assignment_from_store(
                            source,
                            property,
                            None::<ast::ModifierList>,
                            name,
                            postfix_token,
                            type_node,
                            equals_token,
                            object_assignment_initializer,
                        )
                }
            }
            ast::Kind::PropertyAssignment => {
                let (name, postfix_token, type_node, initializer) = {
                    let source = self.store_for(property);
                    (
                        source.name(property),
                        source.postfix_token(property),
                        source.r#type(property),
                        source.initializer(property),
                    )
                };
                let initializer = initializer.map(|initializer| {
                    self.transform_static_super_destructuring_assignment_target(
                        initializer,
                        class_this,
                        class_super,
                    )
                });
                if property.store_id() == self.factory().store().store_id() {
                    self.factory_mut().update_property_assignment(
                        property,
                        None::<ast::ModifierList>,
                        name,
                        postfix_token,
                        type_node,
                        initializer,
                    )
                } else {
                    let source = self.source;
                    self.factory_mut().update_property_assignment_from_store(
                        source,
                        property,
                        None::<ast::ModifierList>,
                        name,
                        postfix_token,
                        type_node,
                        initializer,
                    )
                }
            }
            ast::Kind::SpreadAssignment => {
                let expression = self.store_for(property).expression(property);
                let expression = expression.map(|expression| {
                    self.transform_static_super_destructuring_assignment_target(
                        expression,
                        class_this,
                        class_super,
                    )
                });
                if property.store_id() == self.factory().store().store_id() {
                    self.factory_mut()
                        .update_spread_assignment(property, expression)
                } else {
                    let source = self.source;
                    self.factory_mut()
                        .update_spread_assignment_from_store(source, property, expression)
                }
            }
            _ => self.preserve_node(property),
        }
    }

    fn wrap_static_super_property_for_destructuring_target(
        &mut self,
        node: ast::Node,
        class_this: ast::Node,
        class_super: ast::Node,
    ) -> ast::Node {
        let Some(name) = self.transform_static_super_property_name(node) else {
            return self.preserve_node(node);
        };
        let temp = self.emit_context.factory.new_temp_variable();
        let target = self.clone_node_for_reuse(class_super);
        let receiver = self.clone_node_for_reuse(class_this);
        let set_expr = self
            .emit_context
            .factory
            .new_reflect_set_call(target, name, temp, receiver);
        self.emit_context
            .factory
            .new_assignment_target_wrapper(temp, set_expr)
    }

    fn transform_static_super_property_name(&mut self, node: ast::Node) -> Option<ast::Node> {
        match self.store_for(node).kind(node) {
            ast::Kind::PropertyAccessExpression => {
                let name = self.store_for(node).name(node)?;
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
                let argument = self.store_for(node).argument_expression(node)?;
                Some(self.preserve_node(argument))
            }
            _ => None,
        }
    }

    fn is_simple_inlineable_expression(&self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        crate::moduletransforms::utilities::is_simple_inlineable_expression(
            source.kind(node),
            ast::is_identifier(source, node),
        )
    }

    fn transform_super_property_get(
        &mut self,
        node: ast::Node,
        class_this: ast::Node,
        class_super: ast::Node,
    ) -> Option<ast::Node> {
        if !ast::is_super_property(self.store_for(node), node) {
            return None;
        }
        let property_key = match self.store_for(node).kind(node) {
            ast::Kind::PropertyAccessExpression => {
                let name = self.store_for(node).name(node)?;
                let name_text = self.store_for(name).text(name);
                self.factory_mut()
                    .new_string_literal(&name_text, ast::TokenFlags::NONE)
            }
            ast::Kind::ElementAccessExpression => {
                let argument = self.store_for(node).argument_expression(node)?;
                self.preserve_node(argument)
            }
            _ => return None,
        };
        let target = self.clone_node_for_reuse(class_super);
        let receiver = self.clone_node_for_reuse(class_this);
        Some(
            self.emit_context
                .factory
                .new_reflect_get_call(target, property_key, receiver),
        )
    }

    fn create_class_extra_initializers_static_block(
        &mut self,
        class_this: ast::Node,
        extra_initializers: ast::Node,
        class_node: ast::Node,
        class_name: Option<ast::Node>,
    ) -> ast::Node {
        let statement = self.create_class_extra_initializers_statement(
            class_this,
            extra_initializers,
            class_node,
            class_name,
        );
        self.new_static_block(vec![statement], true)
    }

    fn create_class_extra_initializers_statement(
        &mut self,
        class_this: ast::Node,
        extra_initializers: ast::Node,
        class_node: ast::Node,
        class_name: Option<ast::Node>,
    ) -> ast::Node {
        let run_initializers = self.emit_context.factory.new_run_initializers_helper(
            class_this,
            extra_initializers,
            None,
        );
        let statement = self
            .factory_mut()
            .new_expression_statement(run_initializers);
        let source_map_range = if let Some(class_name) = class_name {
            let source = self.store_for(class_name);
            source.loc(class_name)
        } else {
            let source = self.store_for(class_node);
            crate::utilities::move_range_past_decorators(source, class_node)
        };
        self.emit_context
            .set_source_map_range(&statement, source_map_range);
        statement
    }

    fn create_decorator_static_block_with_inlined(
        &mut self,
        field_infos: &[FieldInfo],
        decorator_assignments_inlined: &[bool],
        static_extra_initializers: Option<ast::Node>,
        class_extra_initializers: Option<(ast::Node, ast::Node)>,
        class_info: Option<ClassInfo>,
        class_super: Option<ast::Node>,
        class_node: ast::Node,
        class_name: Option<ast::Node>,
    ) -> ast::Node {
        let (metadata_statement, metadata_reference) = self.create_metadata(class_super);
        let mut statements = vec![metadata_statement];
        for (index, info) in field_infos.iter().enumerate() {
            if decorator_assignments_inlined
                .get(index)
                .copied()
                .unwrap_or(false)
            {
                continue;
            }
            let decorators = self.transform_all_decorators_of_declaration_with_outer_this(
                self.get_decorators(info.member),
            );
            let decorators_array = self.new_array_literal(decorators);
            let assignment = self
                .emit_context
                .factory
                .new_assignment_expression(info.decorators_name, decorators_array);
            statements.push(self.factory_mut().new_expression_statement(assignment));
        }
        for info in field_infos
            .iter()
            .filter(|info| info.is_static && info.is_non_field())
        {
            // 5. Static non-field element decorators are applied
            statements.push(
                self.generate_class_element_decoration_expression_for_class_body(
                    info,
                    metadata_reference,
                ),
            );
        }
        for info in field_infos
            .iter()
            .filter(|info| !info.is_static && info.is_non_field())
        {
            // 6. Non-static non-field element decorators are applied
            statements.push(
                self.generate_class_element_decoration_expression_for_class_body(
                    info,
                    metadata_reference,
                ),
            );
        }
        for info in field_infos
            .iter()
            .filter(|info| info.is_static && !info.is_non_field())
        {
            // 7. Static field element decorators are applied
            statements.push(
                self.generate_class_element_decoration_expression_for_class_body(
                    info,
                    metadata_reference,
                ),
            );
        }
        for info in field_infos
            .iter()
            .filter(|info| !info.is_static && !info.is_non_field())
        {
            // 8. Non-static field element decorators are applied
            statements.push(
                self.generate_class_element_decoration_expression_for_class_body(
                    info,
                    metadata_reference,
                ),
            );
        }
        let metadata_target = if let Some(class_info) = class_info {
            // 9. Class decorators are applied
            // 10. Class binding is initialized
            statements.push(self.generate_class_decoration_expression(
                class_node,
                class_info,
                metadata_reference,
            ));
            statements.push(self.create_class_descriptor_value_assignment(class_node, class_info));
            class_info.class_this
        } else {
            self.emit_context.factory.new_this_expression()
        };
        statements.push(self.create_symbol_metadata(metadata_target, metadata_reference));
        let class_initializers_source_map_range =
            self.class_initializers_source_map_range(class_node, class_name);
        if let Some(static_extra_initializers) = static_extra_initializers {
            let class_this = self.emit_context.factory.new_this_expression();
            let run_initializers = self.emit_context.factory.new_run_initializers_helper(
                class_this,
                static_extra_initializers,
                None,
            );
            let statement = self
                .factory_mut()
                .new_expression_statement(run_initializers);
            self.emit_context
                .set_source_map_range(&statement, class_initializers_source_map_range);
            // 11. Static extra initializers
            // 12. Static fields are initialized
            statements.push(statement);
        }
        if let Some((class_this, class_extra_initializers)) = class_extra_initializers {
            let run_initializers = self.emit_context.factory.new_run_initializers_helper(
                class_this,
                class_extra_initializers,
                None,
            );
            let statement = self
                .factory_mut()
                .new_expression_statement(run_initializers);
            self.emit_context
                .set_source_map_range(&statement, class_initializers_source_map_range);
            // 13. Class extra initializers
            statements.push(statement);
        }
        self.new_static_block(statements, true)
    }

    fn class_initializers_source_map_range(
        &self,
        class_node: ast::Node,
        class_name: Option<ast::Node>,
    ) -> core::TextRange {
        if let Some(class_name) = class_name {
            let source = self.store_for(class_name);
            source.loc(class_name)
        } else {
            let source = self.store_for(class_node);
            crate::utilities::move_range_past_decorators(source, class_node)
        }
    }

    fn generate_class_decoration_expression(
        &mut self,
        class_node: ast::Node,
        class_info: ClassInfo,
        metadata_reference: ast::Node,
    ) -> ast::Node {
        let value = self.create_value_property("value", class_info.class_this);
        let properties = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![value],
        );
        let descriptor_object = self
            .factory_mut()
            .new_object_literal_expression(properties, false);
        let descriptor = self
            .emit_context
            .factory
            .new_assignment_expression(class_info.descriptor_name, descriptor_object);
        let name = self.factory_mut().new_identifier("name");
        let class_name = self.factory_mut().new_property_access_expression(
            class_info.class_this,
            None,
            name,
            ast::NodeFlags::NONE,
        );
        let context = self
            .emit_context
            .factory
            .new_es_decorate_class_context_object(class_name, metadata_reference);
        let null_ctor = self.factory_mut().new_token(ast::Kind::NullKeyword);
        let null_initializers = self.factory_mut().new_token(ast::Kind::NullKeyword);
        let es_decorate = self.emit_context.factory.new_es_decorate_helper(
            null_ctor,
            descriptor,
            class_info.decorators_name,
            context,
            null_initializers,
            class_info.extra_initializers_name,
        );
        let statement = self.factory_mut().new_expression_statement(es_decorate);
        let source_map_range = {
            let source = self.store_for(class_node);
            crate::utilities::move_range_past_decorators(source, class_node)
        };
        self.emit_context
            .set_source_map_range(&statement, source_map_range);
        statement
    }

    fn create_class_descriptor_value_assignment(
        &mut self,
        class_node: ast::Node,
        class_info: ClassInfo,
    ) -> ast::Node {
        let class_reference = self.get_local_class_reference(class_node);
        let value = self.factory_mut().new_identifier("value");
        let descriptor_value = self.factory_mut().new_property_access_expression(
            class_info.descriptor_name,
            None,
            value,
            ast::NodeFlags::NONE,
        );
        let class_this_assignment = self
            .emit_context
            .factory
            .new_assignment_expression(class_info.class_this, descriptor_value);
        let class_reference_assignment = self
            .emit_context
            .factory
            .new_assignment_expression(class_reference, class_this_assignment);
        self.factory_mut()
            .new_expression_statement(class_reference_assignment)
    }

    fn class_has_static_initializers(&mut self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        let Some(members) = source.members(node) else {
            return false;
        };
        let members = members.iter().collect::<Vec<_>>();
        members.into_iter().any(|member| {
            let source = self.store_for(member);
            if ast::is_class_static_block_declaration(source, member) {
                !self.is_class_named_evaluation_helper_block(member)
            } else {
                source.kind(member) == ast::Kind::PropertyDeclaration
                    && ast::has_static_modifier(source, member)
                    && (source.initializer(member).is_some() || ast::has_decorators(source, member))
            }
        })
    }

    fn generate_class_element_decoration_expression_for_class_body(
        &mut self,
        info: &FieldInfo,
        metadata_reference: ast::Node,
    ) -> ast::Node {
        let kind = info.decorator_kind();
        let context_obj = self.create_public_member_context_object(info, metadata_reference, kind);
        let ctor = if matches!(
            info.kind,
            DecoratedMemberKind::Method
                | DecoratedMemberKind::AutoAccessor
                | DecoratedMemberKind::Getter
                | DecoratedMemberKind::Setter
        ) {
            self.emit_context.factory.new_this_expression()
        } else {
            self.factory_mut().new_token(ast::Kind::NullKeyword)
        };
        let descriptor = if let Some(descriptor_name) = info.descriptor_name {
            let descriptor_object = self.create_accessor_descriptor_object(info);
            self.emit_context
                .factory
                .new_assignment_expression(descriptor_name, descriptor_object)
        } else {
            self.factory_mut().new_token(ast::Kind::NullKeyword)
        };
        let initializers = info
            .initializers_name
            .unwrap_or_else(|| self.factory_mut().new_token(ast::Kind::NullKeyword));
        let es_decorate = self.emit_context.factory.new_es_decorate_helper(
            ctor,
            descriptor,
            info.decorators_name,
            context_obj,
            initializers,
            info.extra_initializers_name,
        );
        let statement = self.factory_mut().new_expression_statement(es_decorate);
        let source_map_range = {
            let source = self.store_for_known_node(info.member);
            crate::utilities::move_range_past_decorators(source, info.member)
        };
        self.emit_context
            .set_source_map_range(&statement, source_map_range);
        statement
    }

    fn update_decorated_class_element(
        &mut self,
        info: &FieldInfo,
        previous_extra_initializer: Option<ast::Node>,
        previous_extra_initializer_source_map_range: Option<core::TextRange>,
        inlined_decorator_infos: &[FieldInfo],
    ) -> ast::Node {
        if matches!(
            info.kind,
            DecoratedMemberKind::Field | DecoratedMemberKind::AutoAccessor
        ) {
            return self.update_decorated_property_for_class_fields(
                info,
                previous_extra_initializer,
                previous_extra_initializer_source_map_range,
                inlined_decorator_infos,
            );
        }
        if info.descriptor_name.is_some() {
            let updated = self.create_accessor_descriptor_forwarder(info);
            let source_map_range = {
                let source = self.store_for_known_node(info.member);
                crate::utilities::move_range_past_decorators(source, info.member)
            };
            return finish_class_element_with_source_map_range(
                updated,
                info.member,
                source_map_range,
                self.emit_context,
            );
        }

        let member_is_factory_node = info.member.store_id() == self.factory().store().store_id();
        let (modifiers_input, name_node, parameters_input, body_node, asterisk_token_node) = {
            let source = self.store_for_known_node(info.member);
            (
                source
                    .source_modifiers(info.member)
                    .map(ast::SourceModifierListInput::from_source),
                source.name(info.member),
                source
                    .source_parameters(info.member)
                    .map(ast::SourceNodeListInput::from_source),
                source.body(info.member),
                source.asterisk_token(info.member),
            )
        };
        let modifiers = self.visit_modifiers_input(modifiers_input);
        let name = name_node.map(|name| {
            self.visit_class_element_property_name(info, name, inlined_decorator_infos)
        });
        if let (true, Some(name)) = (info.name_omits_leading_comments, name.as_ref()) {
            // Don't emit leading comments on the name for methods and properties without modifiers, otherwise we
            // will end up printing duplicate comments.
            self.emit_context
                .set_emit_flags(name, printer::EF_NO_LEADING_COMMENTS);
        }
        let parameters = self
            .visit_nodes_input(parameters_input)
            .expect("method/accessor parameters are required");
        let body = self.visit_node(body_node);
        let updated = match info.kind {
            DecoratedMemberKind::Method => {
                let asterisk_token = asterisk_token_node.map(|token| self.preserve_node(token));
                if member_is_factory_node {
                    self.factory_mut().update_method_declaration(
                        info.member,
                        modifiers,
                        asterisk_token,
                        name,
                        None::<ast::Node>,
                        None::<ast::NodeList>,
                        parameters,
                        None::<ast::Node>,
                        None::<ast::Node>,
                        body,
                    )
                } else {
                    let source = self.source;
                    self.factory_mut().update_method_declaration_from_store(
                        source,
                        info.member,
                        modifiers,
                        asterisk_token,
                        name,
                        None::<ast::Node>,
                        None::<ast::NodeList>,
                        parameters,
                        None::<ast::Node>,
                        None::<ast::Node>,
                        body,
                    )
                }
            }
            DecoratedMemberKind::Getter => {
                if member_is_factory_node {
                    self.factory_mut().update_get_accessor_declaration(
                        info.member,
                        modifiers,
                        name,
                        None,
                        parameters,
                        None,
                        None,
                        body,
                    )
                } else {
                    let source = self.source;
                    self.factory_mut()
                        .update_get_accessor_declaration_from_store(
                            source,
                            info.member,
                            modifiers,
                            name,
                            None,
                            parameters,
                            None,
                            None,
                            body,
                        )
                }
            }
            DecoratedMemberKind::Setter => {
                if member_is_factory_node {
                    self.factory_mut().update_set_accessor_declaration(
                        info.member,
                        modifiers,
                        name,
                        None,
                        parameters,
                        None,
                        None,
                        body,
                    )
                } else {
                    let source = self.source;
                    self.factory_mut()
                        .update_set_accessor_declaration_from_store(
                            source,
                            info.member,
                            modifiers,
                            name,
                            None,
                            parameters,
                            None,
                            None,
                            body,
                        )
                }
            }
            _ => unreachable!("decorated field handled earlier"),
        };
        let source_map_range = {
            let source = self.store_for_known_node(info.member);
            crate::utilities::move_range_past_decorators(source, info.member)
        };
        finish_class_element_with_source_map_range(
            updated,
            info.member,
            source_map_range,
            self.emit_context,
        )
    }

    fn update_property_with_pending_initializers(
        &mut self,
        member: ast::Node,
        extra_initializers: ast::Node,
        is_static: bool,
        class_info: Option<ClassInfo>,
    ) -> ast::Node {
        let (modifiers_input, name, postfix_token, type_node, initializer) = {
            let source = self.store_for(member);
            (
                source
                    .source_modifiers(member)
                    .map(ast::SourceModifierListInput::from_source),
                source.name(member),
                source.postfix_token(member),
                source.r#type(member),
                source.initializer(member),
            )
        };
        let modifiers = self.visit_modifiers_input(modifiers_input);
        let name = name.map(|name| {
            self.visit_node(Some(name))
                .unwrap_or_else(|| self.preserve_node(name))
        });
        let postfix_token = postfix_token.map(|token| self.preserve_node(token));
        let type_node = type_node.map(|type_node| self.preserve_node(type_node));
        let initializer = initializer.and_then(|initializer| {
            self.visit_node(Some(initializer))
                .or_else(|| Some(self.preserve_node(initializer)))
        });
        let this_arg = if is_static {
            class_info
                .map(|class_info| class_info.class_this)
                .unwrap_or_else(|| self.emit_context.factory.new_this_expression())
        } else {
            self.emit_context.factory.new_this_expression()
        };
        let run_initializers = self.emit_context.factory.new_run_initializers_helper(
            this_arg,
            extra_initializers,
            None,
        );
        let initializer = if let Some(initializer) = initializer {
            let inline = self
                .emit_context
                .factory
                .inline_expressions(&[run_initializers, initializer])
                .expect("pending property initializers should have inline expressions");
            self.factory_mut().new_parenthesized_expression(inline)
        } else {
            run_initializers
        };

        if member.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_property_declaration(
                member,
                modifiers,
                name,
                postfix_token,
                type_node,
                initializer,
            )
        } else {
            self.emit_context
                .factory
                .node_factory
                .update_property_declaration_from_store(
                    self.source,
                    member,
                    modifiers,
                    name,
                    postfix_token,
                    type_node,
                    initializer,
                )
        }
    }

    fn update_decorated_property_for_class_fields(
        &mut self,
        info: &FieldInfo,
        previous_extra_initializer: Option<ast::Node>,
        previous_extra_initializer_source_map_range: Option<core::TextRange>,
        inlined_decorator_infos: &[FieldInfo],
    ) -> ast::Node {
        let (modifiers_input, name, postfix_token, type_node) = {
            let source = self.store_for(info.member);
            (
                source
                    .source_modifiers(info.member)
                    .map(ast::SourceModifierListInput::from_source),
                source.name(info.member),
                source.postfix_token(info.member),
                source.r#type(info.member),
            )
        };
        let modifiers = self.visit_modifiers_input(modifiers_input);
        let name = name.map(|name| {
            self.visit_class_element_property_name(info, name, inlined_decorator_infos)
        });
        if let (true, Some(name)) = (info.name_omits_leading_comments, name.as_ref()) {
            // Don't emit leading comments on the name for methods and properties without modifiers, otherwise we
            // will end up printing duplicate comments.
            self.emit_context
                .set_emit_flags(name, printer::EF_NO_LEADING_COMMENTS);
        }
        let postfix_token = postfix_token.map(|token| self.preserve_node(token));
        let type_node = type_node.map(|type_node| self.preserve_node(type_node));
        let initializer = self.create_decorated_property_initializer(
            info,
            previous_extra_initializer,
            previous_extra_initializer_source_map_range,
        );
        if info.kind == DecoratedMemberKind::AutoAccessor && info.descriptor_name.is_some() {
            return self.update_decorated_auto_accessor_property_for_class_fields(
                info,
                name.expect("auto-accessor should have a name"),
                initializer,
            );
        }
        let source_map_range = {
            let source = self.store_for(info.member);
            crate::utilities::move_range_past_decorators(source, info.member)
        };
        if info.member.store_id() == self.factory().store().store_id() {
            let updated = self.factory_mut().update_property_declaration(
                info.member,
                modifiers,
                name,
                postfix_token,
                type_node,
                initializer,
            );
            return finish_class_element_with_source_map_range(
                updated,
                info.member,
                source_map_range,
                self.emit_context,
            );
        }
        let updated = self
            .emit_context
            .factory
            .node_factory
            .update_property_declaration_from_store(
                self.source,
                info.member,
                modifiers,
                name,
                postfix_token,
                type_node,
                initializer,
            );
        finish_class_element_with_source_map_range(
            updated,
            info.member,
            source_map_range,
            self.emit_context,
        )
    }

    fn update_decorated_auto_accessor_property_for_class_fields(
        &mut self,
        info: &FieldInfo,
        name: ast::Node,
        initializer: ast::Node,
    ) -> ast::Node {
        let comment_range = self.emit_context.comment_range(&info.member);
        let source_map_range = self.emit_context.source_map_range(&info.member);
        let modifiers_input = self
            .store_for(info.member)
            .source_modifiers(info.member)
            .map(ast::SourceModifierListInput::from_source);
        let modifiers_without_accessor = self.accessor_stripping_modifiers(modifiers_input);

        // given:
        //  accessor #x = 1;
        //
        // emits:
        //  static {
        //      _esDecorate(null, _private_x_descriptor = { get() { return this.#x_1; }, set(value) { this.#x_1 = value; } }, ...)
        //  }
        //  ...
        //  #x_1 = 1;
        //  get #x() { return _private_x_descriptor.get.call(this); }
        //  set #x(value) { _private_x_descriptor.set.call(this, value); }
        let backing_field = utilities::create_accessor_property_backing_field(
            &mut self.emit_context,
            self.source,
            info.member,
            modifiers_without_accessor.clone(),
            Some(initializer),
        );
        self.emit_context.set_original(&backing_field, &info.member);
        self.emit_context
            .mark_emit_node(&backing_field, printer::EF_NO_COMMENTS);
        self.emit_context
            .set_source_map_range(&backing_field, source_map_range);
        if let Some(backing_name) = self.store_for(backing_field).name(backing_field) {
            let name_source_map_range = self.emit_context.source_map_range(&name);
            self.emit_context
                .set_source_map_range(&backing_name, name_source_map_range);
        }

        let descriptor_name = info
            .descriptor_name
            .expect("private auto-accessor should have a descriptor name");
        let getter_modifiers_input = self
            .store_for(info.member)
            .source_modifiers(info.member)
            .map(ast::SourceModifierListInput::from_source);
        let getter_modifiers = self.static_only_modifiers(getter_modifiers_input);
        let getter =
            self.create_get_accessor_descriptor_forwarder(getter_modifiers, name, descriptor_name);
        self.emit_context.set_original(&getter, &info.member);
        self.emit_context.set_comment_range(&getter, comment_range);
        self.emit_context
            .set_source_map_range(&getter, source_map_range);

        let setter_name = self.clone_node_for_reuse(name);
        let setter_modifiers_input = self
            .store_for(info.member)
            .source_modifiers(info.member)
            .map(ast::SourceModifierListInput::from_source);
        let setter_modifiers = self.static_only_modifiers(setter_modifiers_input);
        let setter = self.create_set_accessor_descriptor_forwarder(
            setter_modifiers,
            setter_name,
            descriptor_name,
        );
        self.emit_context.set_original(&setter, &info.member);
        self.emit_context
            .mark_emit_node(&setter, printer::EF_NO_COMMENTS);
        self.emit_context
            .set_source_map_range(&setter, source_map_range);

        self.factory_mut()
            .new_syntax_list(vec![backing_field, getter, setter])
    }

    fn create_decorated_property_initializer(
        &mut self,
        info: &FieldInfo,
        previous_extra_initializer: Option<ast::Node>,
        previous_extra_initializer_source_map_range: Option<core::TextRange>,
    ) -> ast::Node {
        let this_arg = info
            .this_arg
            .unwrap_or_else(|| self.emit_context.factory.new_this_expression());
        let initializer = self
            .store_for(info.member)
            .initializer(info.member)
            .map(|initializer| self.transform_decorated_property_initializer(info, initializer))
            .unwrap_or_else(|| self.emit_context.factory.new_void_zero_expression());
        let run_initializers = self.emit_context.factory.new_run_initializers_helper(
            this_arg,
            info.initializers_name
                .expect("decorated property should have initializers"),
            Some(initializer),
        );
        if let Some(previous_extra_initializer) = previous_extra_initializer {
            let this_arg = info
                .this_arg
                .unwrap_or_else(|| self.emit_context.factory.new_this_expression());
            let run_extra_initializers = self.emit_context.factory.new_run_initializers_helper(
                this_arg,
                previous_extra_initializer,
                None,
            );
            if let Some(source_map_range) = previous_extra_initializer_source_map_range {
                self.emit_context
                    .set_source_map_range(&run_extra_initializers, source_map_range);
            }
            let inline = self
                .emit_context
                .factory
                .inline_expressions(&[run_extra_initializers, run_initializers])
                .expect("decorated property initializer should have inline expressions");
            return self.factory_mut().new_parenthesized_expression(inline);
        }
        run_initializers
    }

    fn transform_decorated_property_initializer(
        &mut self,
        info: &FieldInfo,
        initializer: ast::Node,
    ) -> ast::Node {
        if let Some(class_expr) = self.anonymous_class_needing_assigned_name(initializer) {
            let assigned_name = self.get_assigned_name_of_field_info(info);
            self.finish_transform_named_evaluation(initializer, class_expr, assigned_name);
        }
        self.visit_node(Some(initializer))
            .unwrap_or_else(|| self.preserve_node(initializer))
    }

    fn get_assigned_name_of_field_info(&mut self, info: &FieldInfo) -> ast::Node {
        if let Some(referenced_name) = info.referenced_name {
            return self.preserve_node(referenced_name);
        }
        let name = info.name;
        if ast::is_property_name_literal(self.store_for(name), name)
            || ast::is_private_identifier(self.store_for(name), name)
        {
            return self.new_string_literal_from_node(name);
        }
        let Some(expression) = self.store_for(name).expression(name) else {
            return self
                .factory_mut()
                .new_string_literal("", ast::TokenFlags::NONE);
        };
        if ast::is_property_name_literal(self.store_for(expression), expression)
            && !ast::is_identifier(self.store_for(expression), expression)
        {
            return self.new_string_literal_from_node(expression);
        }
        self.factory_mut()
            .new_string_literal("", ast::TokenFlags::NONE)
    }

    fn create_extra_initializers_constructor(
        &mut self,
        extra_initializers: ast::Node,
        is_derived_class: bool,
    ) -> ast::Node {
        let statement = self.create_extra_initializers_statement(extra_initializers);
        let mut statements = Vec::new();
        if is_derived_class {
            let arguments = self.factory_mut().new_identifier("arguments");
            let spread_arguments = self.factory_mut().new_spread_element(arguments);
            let arguments = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                vec![spread_arguments],
            );
            let super_keyword = self
                .factory_mut()
                .new_keyword_expression(ast::Kind::SuperKeyword);
            let super_call = self.factory_mut().new_call_expression(
                super_keyword,
                None::<ast::Node>,
                None::<ast::NodeList>,
                arguments,
                ast::NodeFlags::NONE,
            );
            statements.push(self.factory_mut().new_expression_statement(super_call));
        }
        statements.push(statement);
        let statements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            statements,
        );
        let body = self.factory_mut().new_block(statements, true);
        let parameters = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            Vec::<ast::Node>::new(),
        );
        self.factory_mut()
            .new_constructor_declaration(None, None, parameters, None, None, body)
    }

    fn create_extra_initializers_statement(&mut self, extra_initializers: ast::Node) -> ast::Node {
        let this_arg = self.emit_context.factory.new_this_expression();
        self.create_extra_initializers_statement_with_this_arg(this_arg, extra_initializers)
    }

    fn create_extra_initializers_statement_with_this_arg(
        &mut self,
        this_arg: ast::Node,
        extra_initializers: ast::Node,
    ) -> ast::Node {
        let run_initializers = self.emit_context.factory.new_run_initializers_helper(
            this_arg,
            extra_initializers,
            None,
        );
        self.factory_mut()
            .new_expression_statement(run_initializers)
    }

    fn create_extra_initializers_static_block(
        &mut self,
        extra_initializers: ast::Node,
    ) -> ast::Node {
        let this_arg = self.emit_context.factory.new_this_expression();
        let run_initializers = self.emit_context.factory.new_run_initializers_helper(
            this_arg,
            extra_initializers,
            None,
        );
        let statement = self
            .factory_mut()
            .new_expression_statement(run_initializers);
        self.new_static_block(vec![statement], true)
    }

    fn new_static_block(&mut self, statements: Vec<ast::Node>, multi_line: bool) -> ast::Node {
        let statement_list = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            statements,
        );
        let body = self.factory_mut().new_block(statement_list, multi_line);
        self.factory_mut()
            .new_class_static_block_declaration(None, Some(body))
    }

    fn create_class_expression_with_decorated_fields(
        &mut self,
        node: ast::Node,
        field_infos: &[FieldInfo],
    ) -> ast::Node {
        let source = self.source;
        let mut members = Vec::new();
        let mut found_constructor = false;
        for member in source.members(node).expect("class should have members") {
            if field_infos.iter().any(|info| info.member == member) {
                continue;
            }
            if source.kind(member) == ast::Kind::Constructor {
                found_constructor = true;
                members.push(self.visit_constructor_declaration(member, field_infos));
            } else {
                members.push(self.preserve_node(member));
            }
        }
        if !found_constructor {
            members.insert(0, self.create_synthetic_constructor(field_infos));
        }
        let members = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            members,
        );
        let name = self.preserve_optional_node(source.name(node));
        let heritage_clauses =
            self.preserve_optional_source_node_list(source.source_heritage_clauses(node));
        self.factory_mut().new_class_expression(
            None::<ast::ModifierList>,
            name,
            None::<ast::NodeList>,
            heritage_clauses,
            members,
        )
    }

    fn visit_constructor_declaration(
        &mut self,
        node: ast::Node,
        field_infos: &[FieldInfo],
    ) -> ast::Node {
        let node_is_factory_node = node.store_id() == self.factory().store().store_id();
        let (parameters_loc, parameters_range, parameter_nodes, original_statements) = {
            let source = self.store_for(node);
            let parameters = source
                .source_parameters(node)
                .expect("constructor should have parameters");
            let parameters_loc = parameters.loc();
            let parameters_range = parameters.range();
            let parameter_nodes = parameters.iter().collect::<Vec<_>>();
            let original_statements = source
                .body(node)
                .and_then(|body| source.statements(body))
                .map(|statements| statements.iter().collect::<Vec<_>>())
                .unwrap_or_default();
            (
                parameters_loc,
                parameters_range,
                parameter_nodes,
                original_statements,
            )
        };
        let parameters =
            self.strip_parameter_decorators(parameters_loc, parameters_range, parameter_nodes);
        let mut statements = Vec::new();
        statements.extend(self.create_instance_initializer_statements(field_infos));
        statements.extend(
            original_statements
                .into_iter()
                .map(|statement| self.preserve_node(statement)),
        );
        let statements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            statements,
        );
        let body = self.factory_mut().new_block(statements, true);
        if node_is_factory_node {
            self.factory_mut().update_constructor_declaration(
                node,
                None::<ast::ModifierList>,
                None::<ast::NodeList>,
                parameters,
                None::<ast::Node>,
                None::<ast::Node>,
                body,
            )
        } else {
            let source = self.source;
            self.factory_mut()
                .update_constructor_declaration_from_store(
                    source,
                    node,
                    None::<ast::ModifierList>,
                    None::<ast::NodeList>,
                    parameters,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    body,
                )
        }
    }

    fn visit_constructor_declaration_with_extra_initializers(
        &mut self,
        node: ast::Node,
        extra_initializers: ast::Node,
        is_derived_class: bool,
    ) -> ast::Node {
        let node_is_factory_node = node.store_id() == self.factory().store().store_id();
        let (parameters_loc, parameters_range, parameter_nodes, original_statements) = {
            let source = self.store_for(node);
            let parameters = source
                .source_parameters(node)
                .expect("constructor should have parameters");
            let parameters_loc = parameters.loc();
            let parameters_range = parameters.range();
            let parameter_nodes = parameters.iter().collect::<Vec<_>>();
            let original_statements = source
                .body(node)
                .and_then(|body| source.statements(body))
                .map(|statements| statements.iter().collect::<Vec<_>>())
                .unwrap_or_default();
            (
                parameters_loc,
                parameters_range,
                parameter_nodes,
                original_statements,
            )
        };
        let parameters =
            self.strip_parameter_decorators(parameters_loc, parameters_range, parameter_nodes);
        let initializer_statement = self.create_extra_initializers_statement(extra_initializers);
        let statements = self.constructor_statements_with_extra_initializers(
            original_statements,
            initializer_statement,
            is_derived_class,
        );
        let statements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            statements,
        );
        let body = self.factory_mut().new_block(statements, true);
        if node_is_factory_node {
            self.factory_mut().update_constructor_declaration(
                node,
                None::<ast::ModifierList>,
                None::<ast::NodeList>,
                parameters,
                None::<ast::Node>,
                None::<ast::Node>,
                body,
            )
        } else {
            let source = self.source;
            self.factory_mut()
                .update_constructor_declaration_from_store(
                    source,
                    node,
                    None::<ast::ModifierList>,
                    None::<ast::NodeList>,
                    parameters,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    body,
                )
        }
    }

    fn constructor_statements_with_extra_initializers(
        &mut self,
        original_statements: Vec<ast::Node>,
        initializer_statement: ast::Node,
        is_derived_class: bool,
    ) -> Vec<ast::Node> {
        let (prologue, rest) = self.split_standard_prologue(&original_statements);
        let mut statements = prologue
            .iter()
            .map(|statement| self.preserve_node(*statement))
            .collect::<Vec<_>>();

        if is_derived_class
            && let Some(super_statement_index) = self.find_super_statement_index(rest)
        {
            statements.extend(
                rest[..=super_statement_index]
                    .iter()
                    .map(|statement| self.preserve_node(*statement)),
            );
            statements.push(initializer_statement);
            statements.extend(
                rest[super_statement_index + 1..]
                    .iter()
                    .map(|statement| self.preserve_node(*statement)),
            );
            return statements;
        }

        statements.push(initializer_statement);
        statements.extend(rest.iter().map(|statement| self.preserve_node(*statement)));
        statements
    }

    fn find_super_statement_index(&self, statements: &[ast::Node]) -> Option<usize> {
        statements
            .iter()
            .position(|statement| self.get_super_call_from_statement(*statement))
    }

    fn split_standard_prologue<'a>(
        &self,
        statements: &'a [ast::Node],
    ) -> (&'a [ast::Node], &'a [ast::Node]) {
        for (index, statement) in statements.iter().enumerate() {
            if !ast::is_prologue_directive(self.store_for(*statement), *statement) {
                return (&statements[..index], &statements[index..]);
            }
        }
        (statements, &[])
    }

    fn get_super_call_from_statement(&self, statement: ast::Node) -> bool {
        let source = self.store_for(statement);
        let Some(expression) = source.expression(statement) else {
            return false;
        };
        if source.kind(expression) != ast::Kind::CallExpression {
            return false;
        }
        let Some(callee) = source.expression(expression) else {
            return false;
        };
        self.store_for(callee).kind(callee) == ast::Kind::SuperKeyword
    }

    fn create_synthetic_constructor(&mut self, field_infos: &[FieldInfo]) -> ast::Node {
        let initializer_statements = self.create_instance_initializer_statements(field_infos);
        let statements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            initializer_statements,
        );
        let body = self.factory_mut().new_block(statements, true);
        let parameters = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            Vec::<ast::Node>::new(),
        );
        self.factory_mut()
            .new_constructor_declaration(None, None, parameters, None, None, body)
    }

    fn create_instance_initializer_statements(
        &mut self,
        field_infos: &[FieldInfo],
    ) -> Vec<ast::Node> {
        let mut statements = Vec::new();
        for info in field_infos {
            let this_arg = self.emit_context.factory.new_this_expression();
            let this_for_value = self.emit_context.factory.new_this_expression();
            let initializer = self
                .source
                .initializer(info.member)
                .map(|initializer| self.preserve_node(initializer))
                .unwrap_or_else(|| self.emit_context.factory.new_void_zero_expression());
            let run_initializers = self.emit_context.factory.new_run_initializers_helper(
                this_arg,
                info.initializers_name
                    .expect("decorated property should have initializers"),
                Some(initializer),
            );
            let name = self.preserve_node(info.name);
            let property_access = self.factory_mut().new_property_access_expression(
                this_for_value,
                None::<ast::Node>,
                name,
                ast::NodeFlags::NONE,
            );
            let assignment = self
                .emit_context
                .factory
                .new_assignment_expression(property_access, run_initializers);
            statements.push(self.factory_mut().new_expression_statement(assignment));

            let this_arg = self.emit_context.factory.new_this_expression();
            let extra_initializers = self.emit_context.factory.new_run_initializers_helper(
                this_arg,
                info.extra_initializers_name,
                None,
            );
            statements.push(
                self.factory_mut()
                    .new_expression_statement(extra_initializers),
            );
        }
        statements
    }

    fn create_decoration_statements(
        &mut self,
        _class_node: ast::Node,
        field_infos: &[FieldInfo],
        metadata_statement: ast::Node,
        metadata_reference: ast::Node,
        class_reference: ast::Node,
    ) -> Vec<ast::Node> {
        let mut statements = vec![metadata_statement];
        for info in field_infos {
            let decorators = self.transform_all_decorators_of_declaration_with_outer_this(
                self.get_decorators(info.member),
            );
            let decorators_array = self.new_array_literal(decorators);
            let assignment = self
                .emit_context
                .factory
                .new_assignment_expression(info.decorators_name, decorators_array);
            statements.push(self.factory_mut().new_expression_statement(assignment));
            statements
                .push(self.generate_class_element_decoration_expression(info, metadata_reference));
        }
        statements.push(self.create_symbol_metadata(class_reference, metadata_reference));
        statements
    }

    fn generate_class_element_decoration_expression(
        &mut self,
        info: &FieldInfo,
        metadata_reference: ast::Node,
    ) -> ast::Node {
        let context_obj =
            self.create_public_member_context_object(info, metadata_reference, "field");
        let null = self.factory_mut().new_token(ast::Kind::NullKeyword);
        let descriptor = self.factory_mut().new_token(ast::Kind::NullKeyword);
        let es_decorate = self.emit_context.factory.new_es_decorate_helper(
            null,
            descriptor,
            info.decorators_name,
            context_obj,
            info.initializers_name
                .expect("decorated property should have initializers"),
            info.extra_initializers_name,
        );
        self.factory_mut().new_expression_statement(es_decorate)
    }

    fn create_public_member_context_object(
        &mut self,
        info: &FieldInfo,
        metadata_reference: ast::Node,
        kind: &str,
    ) -> ast::Node {
        // Determine the property name for the context
        let (name_computed, name_expr) = if let Some(referenced_name) = info.referenced_name {
            (true, referenced_name)
        } else {
            self.create_es_decorate_property_name(info.name)
        };
        if info.name_omits_leading_comments {
            // Don't emit leading comments on the name for methods and properties without modifiers, otherwise we
            // will end up printing duplicate comments.
            self.emit_context
                .set_emit_flags(&name_expr, printer::EF_NO_LEADING_COMMENTS);
        }
        self.emit_context
            .factory
            .new_es_decorate_class_element_context_object(
                self.source,
                kind,
                name_computed,
                name_expr,
                info.is_static,
                info.is_private,
                // 15.7.3 CreateDecoratorAccessObject (kind, name)
                // 2. If _kind_ is ~field~, ~method~, ~accessor~, or ~getter~, then ...
                matches!(
                    info.kind,
                    DecoratedMemberKind::Method
                        | DecoratedMemberKind::Field
                        | DecoratedMemberKind::AutoAccessor
                        | DecoratedMemberKind::Getter
                ),
                // 3. If _kind_ is ~field~, ~accessor~, or ~setter~, then ...
                matches!(
                    info.kind,
                    DecoratedMemberKind::Field
                        | DecoratedMemberKind::AutoAccessor
                        | DecoratedMemberKind::Setter
                ),
                metadata_reference,
            )
    }

    fn static_only_modifiers(
        &mut self,
        modifiers: Option<ast::SourceModifierListInput>,
    ) -> Option<ast::ModifierList> {
        let mut nodes = Vec::new();
        if let Some(modifiers) = modifiers {
            for modifier in modifiers.iter() {
                let source = self.store_for(modifier);
                if source.kind(modifier) == ast::Kind::StaticKeyword {
                    nodes.push(self.preserve_node(modifier));
                }
            }
        }
        if nodes.is_empty() {
            None
        } else {
            Some(self.factory_mut().new_modifier_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                nodes,
                ast::ModifierFlags::STATIC,
            ))
        }
    }

    fn static_only_modifiers_from_nodes(
        &mut self,
        modifiers: Vec<ast::Node>,
    ) -> Option<ast::ModifierList> {
        let mut nodes = Vec::new();
        for modifier in modifiers {
            let source = self.store_for(modifier);
            if source.kind(modifier) == ast::Kind::StaticKeyword {
                nodes.push(self.preserve_node(modifier));
            }
        }
        if nodes.is_empty() {
            None
        } else {
            Some(self.factory_mut().new_modifier_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                nodes,
                ast::ModifierFlags::STATIC,
            ))
        }
    }

    fn async_only_modifiers(
        &mut self,
        modifiers: Option<ast::SourceModifierList<'_>>,
    ) -> Option<ast::ModifierList> {
        let nodes = modifiers
            .map(|modifiers| {
                let source = modifiers.store();
                modifiers
                    .iter()
                    .filter(|modifier| source.kind(*modifier) == ast::Kind::AsyncKeyword)
                    .map(|modifier| self.preserve_node(modifier))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if nodes.is_empty() {
            None
        } else {
            Some(self.factory_mut().new_modifier_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                nodes,
                ast::ModifierFlags::ASYNC,
            ))
        }
    }

    fn async_only_modifiers_input(
        &mut self,
        modifiers: Option<ast::SourceModifierListInput>,
    ) -> Option<ast::ModifierList> {
        let mut nodes = Vec::new();
        if let Some(modifiers) = modifiers {
            for modifier in modifiers.iter() {
                if self.store_for(modifier).kind(modifier) == ast::Kind::AsyncKeyword {
                    nodes.push(self.preserve_node(modifier));
                }
            }
        }
        if nodes.is_empty() {
            None
        } else {
            Some(self.factory_mut().new_modifier_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                nodes,
                ast::ModifierFlags::ASYNC,
            ))
        }
    }

    fn accessor_stripping_modifiers(
        &mut self,
        modifiers: Option<ast::SourceModifierListInput>,
    ) -> Option<ast::ModifierList> {
        let modifiers = modifiers?;
        let mut nodes = Vec::new();
        for modifier in modifiers.iter() {
            let should_preserve = {
                let source = self.store_for(modifier);
                !matches!(
                    source.kind(modifier),
                    ast::Kind::AccessorKeyword | ast::Kind::Decorator
                )
            };
            if should_preserve {
                nodes.push(self.preserve_node(modifier));
            }
        }
        if nodes.is_empty() {
            None
        } else {
            Some(self.factory_mut().new_modifier_list(
                modifiers.loc(),
                modifiers.range(),
                nodes,
                modifiers.modifier_flags() & !ast::ModifierFlags::ACCESSOR,
            ))
        }
    }

    // Creates a "value", "get", or "set" method for a pseudo-PropertyDescriptor object created for
    // a private element.
    fn create_descriptor_method(
        &mut self,
        original: ast::Node,
        name: ast::Node,
        modifiers: Option<ast::ModifierList>,
        asterisk_token: Option<ast::Node>,
        kind: &str,
        parameters: ast::NodeList,
        body: Option<ast::Node>,
    ) -> ast::Node {
        let body = body.unwrap_or_else(|| {
            let statements = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                Vec::<ast::Node>::new(),
            );
            self.factory_mut().new_block(statements, false)
        });
        let source_map_range = {
            let source = self.store_for_known_node(original);
            crate::utilities::move_range_past_decorators(source, original)
        };
        let func_expr = self.factory_mut().new_function_expression(
            modifiers,
            asterisk_token,
            None::<ast::Node>,
            None::<ast::NodeList>,
            parameters,
            None::<ast::Node>,
            None::<ast::Node>,
            body,
        );
        self.emit_context.set_original(&func_expr, &original);
        self.emit_context
            .set_source_map_range(&func_expr, source_map_range);
        self.emit_context
            .mark_emit_node(&func_expr, printer::EF_NO_COMMENTS);

        let function_name = if name.store_id() == self.factory().store().store_id() {
            let function_name_text = self.factory().store().text(name).to_string();
            self.factory_mut()
                .new_string_literal(&function_name_text, ast::TokenFlags::NONE)
        } else {
            self.emit_context
                .factory
                .new_string_literal_from_node(self.source, &name)
        };
        let prefix = if kind == "get" || kind == "set" {
            kind
        } else {
            ""
        };
        let named_function = self.emit_context.factory.new_set_function_name_helper(
            func_expr,
            function_name,
            prefix,
        );
        let method_name = self.factory_mut().new_identifier(kind);
        let method = self.factory_mut().new_property_assignment(
            None::<ast::ModifierList>,
            method_name,
            None::<ast::Node>,
            None::<ast::Node>,
            named_function,
        );
        self.emit_context.set_original(&method, &original);
        self.emit_context
            .set_source_map_range(&method, source_map_range);
        self.emit_context
            .mark_emit_node(&method, printer::EF_NO_COMMENTS);
        method
    }

    // Creates a pseudo-PropertyDescriptor object used when decorating a private MethodDeclaration.
    fn create_method_descriptor_object(&mut self, member: ast::Node) -> ast::Node {
        let (name, modifiers_input, asterisk_token, parameters_input, body_node) = {
            let source = self.store_for(member);
            (
                source
                    .name(member)
                    .expect("private method should have a name"),
                source
                    .source_modifiers(member)
                    .map(ast::SourceModifierListInput::from_source),
                source.asterisk_token(member),
                source
                    .source_parameters(member)
                    .map(ast::SourceNodeListInput::from_source),
                source.body(member),
            )
        };
        let modifiers = self.async_only_modifiers_input(modifiers_input);
        let asterisk_token = asterisk_token.map(|token| self.preserve_node(token));
        let parameters = self
            .visit_nodes_input(parameters_input)
            .expect("method parameters should be present");
        let body = self.visit_node(body_node);
        let method = self.create_descriptor_method(
            member,
            name,
            modifiers,
            asterisk_token,
            "value",
            parameters,
            body,
        );
        let props = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![method],
        );
        self.factory_mut()
            .new_object_literal_expression(props, false)
    }

    // Creates a pseudo-PropertyDescriptor object used when decorating a private GetAccessor.
    fn create_get_accessor_descriptor_object(&mut self, member: ast::Node) -> ast::Node {
        let (name, modifiers_input, body_node) = {
            let source = self.store_for(member);
            (
                source
                    .name(member)
                    .expect("private get accessor should have a name"),
                source
                    .source_modifiers(member)
                    .map(ast::SourceModifierListInput::from_source),
                source.body(member),
            )
        };
        let modifiers = self.async_only_modifiers_input(modifiers_input);
        let parameters = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            Vec::<ast::Node>::new(),
        );
        let body = self.visit_node(body_node);
        let method =
            self.create_descriptor_method(member, name, modifiers, None, "get", parameters, body);
        let props = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![method],
        );
        self.factory_mut()
            .new_object_literal_expression(props, false)
    }

    // Creates a pseudo-PropertyDescriptor object used when decorating a private SetAccessor.
    fn create_set_accessor_descriptor_object(&mut self, member: ast::Node) -> ast::Node {
        let (name, modifiers_input, parameters_input, body_node) = {
            let source = self.store_for(member);
            (
                source
                    .name(member)
                    .expect("private set accessor should have a name"),
                source
                    .source_modifiers(member)
                    .map(ast::SourceModifierListInput::from_source),
                source
                    .source_parameters(member)
                    .map(ast::SourceNodeListInput::from_source),
                source.body(member),
            )
        };
        let modifiers = self.async_only_modifiers_input(modifiers_input);
        let parameters = self
            .visit_nodes_input(parameters_input)
            .expect("set accessor parameters should be present");
        let body = self.visit_node(body_node);
        let method =
            self.create_descriptor_method(member, name, modifiers, None, "set", parameters, body);
        let props = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![method],
        );
        self.factory_mut()
            .new_object_literal_expression(props, false)
    }

    // Creates a pseudo-PropertyDescriptor object used when decorating a private auto-accessor
    // PropertyDeclaration. The descriptor contains get/set methods that access the generated
    // backing field.
    fn create_accessor_property_descriptor_object(&mut self, member: ast::Node) -> ast::Node {
        let name = self
            .store_for(member)
            .name(member)
            .expect("private auto-accessor should have a name");
        let backing_field_source_name = self
            .store_for(member)
            .name(member)
            .expect("private auto-accessor should have a name");
        let backing_field_name = self.emit_context.new_generated_private_name_for_node_ex(
            backing_field_source_name,
            AutoGenerateOptions {
                suffix: utilities::accessor_backing_field_suffix(),
                ..Default::default()
            },
        );

        let this = self.emit_context.factory.new_this_expression();
        let backing_field_name_for_get = self.clone_node_for_reuse(backing_field_name);
        let access = self.factory_mut().new_property_access_expression(
            this,
            None::<ast::Node>,
            backing_field_name_for_get,
            ast::NodeFlags::NONE,
        );
        let return_statement = self.factory_mut().new_return_statement(access);
        let statements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![return_statement],
        );
        let get_body = self.factory_mut().new_block(statements, false);
        let get_parameters = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            Vec::<ast::Node>::new(),
        );
        let get_method = self.create_descriptor_method(
            member,
            name,
            None,
            None,
            "get",
            get_parameters,
            Some(get_body),
        );

        let value_name = self.factory_mut().new_identifier("value");
        let value_parameter = self.factory_mut().new_parameter_declaration(
            None::<ast::ModifierList>,
            None::<ast::Node>,
            value_name,
            None::<ast::Node>,
            None::<ast::Node>,
            None::<ast::Node>,
        );
        let set_parameters = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![value_parameter],
        );
        let this = self.emit_context.factory.new_this_expression();
        let backing_field_name_for_set = self.clone_node_for_reuse(backing_field_name);
        let access = self.factory_mut().new_property_access_expression(
            this,
            None::<ast::Node>,
            backing_field_name_for_set,
            ast::NodeFlags::NONE,
        );
        let value = self.factory_mut().new_identifier("value");
        let assignment = self
            .emit_context
            .factory
            .new_assignment_expression(access, value);
        let statement = self.factory_mut().new_expression_statement(assignment);
        let statements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![statement],
        );
        let set_body = self.factory_mut().new_block(statements, false);
        let set_method = self.create_descriptor_method(
            member,
            name,
            None,
            None,
            "set",
            set_parameters,
            Some(set_body),
        );

        let props = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![get_method, set_method],
        );
        self.factory_mut()
            .new_object_literal_expression(props, false)
    }

    fn create_accessor_descriptor_object(&mut self, info: &FieldInfo) -> ast::Node {
        match info.kind {
            DecoratedMemberKind::Method => self.create_method_descriptor_object(info.member),
            DecoratedMemberKind::AutoAccessor => {
                self.create_accessor_property_descriptor_object(info.member)
            }
            DecoratedMemberKind::Getter => self.create_get_accessor_descriptor_object(info.member),
            DecoratedMemberKind::Setter => self.create_set_accessor_descriptor_object(info.member),
            _ => unreachable!("descriptor object is only created for private accessors"),
        }
    }

    // Creates a GetAccessor that forwards its invocation to a PropertyDescriptor object.
    fn create_method_descriptor_forwarder(
        &mut self,
        modifiers: Option<ast::ModifierList>,
        name: ast::Node,
        descriptor_name: ast::Node,
    ) -> ast::Node {
        let name = self.preserve_node(name);
        let value_name = self.factory_mut().new_identifier("value");
        let descriptor_value = self.factory_mut().new_property_access_expression(
            descriptor_name,
            None::<ast::Node>,
            value_name,
            ast::NodeFlags::NONE,
        );
        let return_statement = self.factory_mut().new_return_statement(descriptor_value);
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
        self.factory_mut()
            .new_get_accessor_declaration(modifiers, name, None, parameters, None, None, body)
    }

    // Creates a GetAccessor that forwards its invocation to a PropertyDescriptor object.
    fn create_get_accessor_descriptor_forwarder(
        &mut self,
        modifiers: Option<ast::ModifierList>,
        name: ast::Node,
        descriptor_name: ast::Node,
    ) -> ast::Node {
        let name = self.preserve_node(name);
        let get_name = self.factory_mut().new_identifier("get");
        let descriptor_get = self.factory_mut().new_property_access_expression(
            descriptor_name,
            None::<ast::Node>,
            get_name,
            ast::NodeFlags::NONE,
        );
        let this_arg = self.emit_context.factory.new_this_expression();
        let call =
            self.emit_context
                .factory
                .new_function_call_call(&descriptor_get, Some(&this_arg), &[]);
        let return_statement = self.factory_mut().new_return_statement(call);
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
        self.factory_mut()
            .new_get_accessor_declaration(modifiers, name, None, parameters, None, None, body)
    }

    // Creates a SetAccessor that forwards its invocation to a PropertyDescriptor object.
    fn create_set_accessor_descriptor_forwarder(
        &mut self,
        modifiers: Option<ast::ModifierList>,
        name: ast::Node,
        descriptor_name: ast::Node,
    ) -> ast::Node {
        let name = self.preserve_node(name);
        let value_name = self.factory_mut().new_identifier("value");
        let value_parameter = self.factory_mut().new_parameter_declaration(
            None::<ast::ModifierList>,
            None::<ast::Node>,
            value_name,
            None::<ast::Node>,
            None::<ast::Node>,
            None::<ast::Node>,
        );
        let parameters = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![value_parameter],
        );
        let set_name = self.factory_mut().new_identifier("set");
        let descriptor_set = self.factory_mut().new_property_access_expression(
            descriptor_name,
            None::<ast::Node>,
            set_name,
            ast::NodeFlags::NONE,
        );
        let this_arg = self.emit_context.factory.new_this_expression();
        let value = self.factory_mut().new_identifier("value");
        let call = self.emit_context.factory.new_function_call_call(
            &descriptor_set,
            Some(&this_arg),
            &[value],
        );
        let return_statement = self.factory_mut().new_return_statement(call);
        let statements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![return_statement],
        );
        let body = self.factory_mut().new_block(statements, false);
        self.factory_mut()
            .new_set_accessor_declaration(modifiers, name, None, parameters, None, None, body)
    }

    fn create_accessor_descriptor_forwarder(&mut self, info: &FieldInfo) -> ast::Node {
        let (name, modifier_nodes) = {
            let source = self.store_for_known_node(info.member);
            (
                source
                    .name(info.member)
                    .expect("private accessor should have a name"),
                source
                    .source_modifiers(info.member)
                    .map(|modifiers| modifiers.iter().collect::<Vec<_>>())
                    .unwrap_or_default(),
            )
        };
        let modifiers = self.static_only_modifiers_from_nodes(modifier_nodes);
        let descriptor_name = info
            .descriptor_name
            .expect("private accessor should have a descriptor name");
        match info.kind {
            DecoratedMemberKind::Method => {
                self.create_method_descriptor_forwarder(modifiers, name, descriptor_name)
            }
            DecoratedMemberKind::Getter => {
                self.create_get_accessor_descriptor_forwarder(modifiers, name, descriptor_name)
            }
            DecoratedMemberKind::Setter => {
                self.create_set_accessor_descriptor_forwarder(modifiers, name, descriptor_name)
            }
            _ => unreachable!("descriptor forwarder is only created for private accessors"),
        }
    }

    fn create_es_decorate_property_name(&mut self, name: ast::Node) -> (bool, ast::Node) {
        let source = self.store_for(name);
        match source.kind(name) {
            ast::Kind::Identifier | ast::Kind::PrivateIdentifier => {
                (false, self.preserve_node(name))
            }
            ast::Kind::StringLiteral | ast::Kind::NumericLiteral => {
                let text = source.text(name);
                let literal = self
                    .factory_mut()
                    .new_string_literal(text, ast::TokenFlags::NONE);
                (true, literal)
            }
            ast::Kind::ComputedPropertyName => {
                let expression = source
                    .expression(name)
                    .expect("computed property name should have expression");
                let expression_source = self.store_for(expression);
                if ast::is_property_name_literal(expression_source, expression)
                    && !ast::is_identifier(expression_source, expression)
                {
                    let text = expression_source.text(expression);
                    let literal = self
                        .factory_mut()
                        .new_string_literal(text, ast::TokenFlags::NONE);
                    (true, literal)
                } else {
                    (true, self.preserve_node(expression))
                }
            }
            _ => (true, self.preserve_node(name)),
        }
    }

    fn visit_class_element_property_name(
        &mut self,
        info: &FieldInfo,
        name: ast::Node,
        inlined_decorator_infos: &[FieldInfo],
    ) -> ast::Node {
        let Some(referenced_name) = info.referenced_name else {
            return self.preserve_node(name);
        };
        let expression = self
            .store_for(name)
            .expression(name)
            .expect("computed property name should have expression");
        let visited_expression = self.preserve_node(expression);
        let prop_key = self
            .emit_context
            .factory
            .new_prop_key_helper(visited_expression);
        let reference_assignment = self
            .emit_context
            .factory
            .new_assignment_expression(referenced_name, prop_key);
        let mut expressions = inlined_decorator_infos
            .iter()
            .map(|info| self.create_decorator_assignment(info, false))
            .collect::<Vec<_>>();
        expressions.push(reference_assignment);
        let expression = self
            .emit_context
            .factory
            .inline_expressions(&expressions)
            .expect("computed property name should have inline expressions");
        if name.store_id() == self.factory().store().store_id() {
            self.factory_mut()
                .update_computed_property_name(name, Some(expression))
        } else {
            let source = self.source;
            self.factory_mut().update_computed_property_name_from_store(
                source,
                name,
                Some(expression),
            )
        }
    }

    fn update_class_element_with_decorator_assignments(
        &mut self,
        member: ast::Node,
        inlined_decorator_infos: &[FieldInfo],
    ) -> ast::Node {
        let (kind, name) = {
            let source = self.store_for(member);
            (source.kind(member), source.name(member))
        };
        let Some(name) = name else {
            return self.preserve_node(member);
        };
        if self.store_for(name).kind(name) != ast::Kind::ComputedPropertyName {
            return self.preserve_node(member);
        }
        let name = self
            .visit_computed_property_name_with_decorator_assignments(name, inlined_decorator_infos);

        match kind {
            ast::Kind::PropertyDeclaration => {
                let (modifiers_input, postfix_token, type_node, initializer) = {
                    let source = self.store_for(member);
                    (
                        source
                            .source_modifiers(member)
                            .map(ast::SourceModifierListInput::from_source),
                        source.postfix_token(member),
                        source.r#type(member),
                        source.initializer(member),
                    )
                };
                let modifiers = self.visit_modifiers_input(modifiers_input);
                let postfix_token = postfix_token.map(|token| self.preserve_node(token));
                let type_node = type_node.map(|type_node| self.preserve_node(type_node));
                let initializer = initializer.and_then(|initializer| {
                    self.visit_node(Some(initializer))
                        .or_else(|| Some(self.preserve_node(initializer)))
                });
                if member.store_id() == self.factory().store().store_id() {
                    self.factory_mut().update_property_declaration(
                        member,
                        modifiers,
                        Some(name),
                        postfix_token,
                        type_node,
                        initializer,
                    )
                } else {
                    let source = self.source;
                    self.factory_mut().update_property_declaration_from_store(
                        source,
                        member,
                        modifiers,
                        Some(name),
                        postfix_token,
                        type_node,
                        initializer,
                    )
                }
            }
            ast::Kind::MethodDeclaration => {
                let (modifiers_input, parameters_input, body_node, asterisk_token_node) = {
                    let source = self.store_for(member);
                    (
                        source
                            .source_modifiers(member)
                            .map(ast::SourceModifierListInput::from_source),
                        source
                            .source_parameters(member)
                            .map(ast::SourceNodeListInput::from_source),
                        source.body(member),
                        source.asterisk_token(member),
                    )
                };
                let modifiers = self.visit_modifiers_input(modifiers_input);
                let asterisk_token = asterisk_token_node.map(|token| self.preserve_node(token));
                let parameters = self
                    .visit_nodes_input(parameters_input)
                    .expect("method parameters are required");
                let body = self.visit_node(body_node);
                if member.store_id() == self.factory().store().store_id() {
                    self.factory_mut().update_method_declaration(
                        member,
                        modifiers,
                        asterisk_token,
                        Some(name),
                        None::<ast::Node>,
                        None::<ast::NodeList>,
                        parameters,
                        None::<ast::Node>,
                        None::<ast::Node>,
                        body,
                    )
                } else {
                    let source = self.source;
                    self.factory_mut().update_method_declaration_from_store(
                        source,
                        member,
                        modifiers,
                        asterisk_token,
                        Some(name),
                        None::<ast::Node>,
                        None::<ast::NodeList>,
                        parameters,
                        None::<ast::Node>,
                        None::<ast::Node>,
                        body,
                    )
                }
            }
            ast::Kind::GetAccessor => {
                let (modifiers_input, parameters_input, body_node) = {
                    let source = self.store_for(member);
                    (
                        source
                            .source_modifiers(member)
                            .map(ast::SourceModifierListInput::from_source),
                        source
                            .source_parameters(member)
                            .map(ast::SourceNodeListInput::from_source),
                        source.body(member),
                    )
                };
                let modifiers = self.visit_modifiers_input(modifiers_input);
                let parameters = self
                    .visit_nodes_input(parameters_input)
                    .expect("get accessor parameters are required");
                let body = self.visit_node(body_node);
                if member.store_id() == self.factory().store().store_id() {
                    self.factory_mut().update_get_accessor_declaration(
                        member,
                        modifiers,
                        Some(name),
                        None,
                        parameters,
                        None,
                        None,
                        body,
                    )
                } else {
                    let source = self.source;
                    self.factory_mut()
                        .update_get_accessor_declaration_from_store(
                            source,
                            member,
                            modifiers,
                            Some(name),
                            None,
                            parameters,
                            None,
                            None,
                            body,
                        )
                }
            }
            ast::Kind::SetAccessor => {
                let (modifiers_input, parameters_input, body_node) = {
                    let source = self.store_for(member);
                    (
                        source
                            .source_modifiers(member)
                            .map(ast::SourceModifierListInput::from_source),
                        source
                            .source_parameters(member)
                            .map(ast::SourceNodeListInput::from_source),
                        source.body(member),
                    )
                };
                let modifiers = self.visit_modifiers_input(modifiers_input);
                let parameters = self
                    .visit_nodes_input(parameters_input)
                    .expect("set accessor parameters are required");
                let body = self.visit_node(body_node);
                if member.store_id() == self.factory().store().store_id() {
                    self.factory_mut().update_set_accessor_declaration(
                        member,
                        modifiers,
                        Some(name),
                        None,
                        parameters,
                        None,
                        None,
                        body,
                    )
                } else {
                    let source = self.source;
                    self.factory_mut()
                        .update_set_accessor_declaration_from_store(
                            source,
                            member,
                            modifiers,
                            Some(name),
                            None,
                            parameters,
                            None,
                            None,
                            body,
                        )
                }
            }
            _ => self.preserve_node(member),
        }
    }

    fn visit_computed_property_name_with_decorator_assignments(
        &mut self,
        name: ast::Node,
        inlined_decorator_infos: &[FieldInfo],
    ) -> ast::Node {
        let Some(expression) = self.store_for(name).expression(name) else {
            return self.preserve_node(name);
        };
        let expression = self
            .visit_node(Some(expression))
            .unwrap_or_else(|| self.preserve_node(expression));
        let mut expressions = inlined_decorator_infos
            .iter()
            .map(|info| self.create_decorator_assignment(info, false))
            .collect::<Vec<_>>();
        expressions.push(expression);
        let expression = self
            .emit_context
            .factory
            .inline_expressions(&expressions)
            .expect("computed property name should have inline expressions");
        if name.store_id() == self.factory().store().store_id() {
            self.factory_mut()
                .update_computed_property_name(name, Some(expression))
        } else {
            let source = self.source;
            self.factory_mut().update_computed_property_name_from_store(
                source,
                name,
                Some(expression),
            )
        }
    }

    fn create_decorator_assignment(
        &mut self,
        info: &FieldInfo,
        capture_outer_this: bool,
    ) -> ast::Node {
        let decorators = self.transform_all_decorators_of_declaration_worker(
            self.get_decorators(info.member),
            capture_outer_this,
        );
        let decorators_array = self.new_array_literal(decorators);
        self.emit_context
            .factory
            .new_assignment_expression(info.decorators_name, decorators_array)
    }

    fn create_public_field_access_object(&mut self, name_text: &str) -> ast::Node {
        let props = vec![
            self.create_public_field_access_has_method(name_text),
            self.create_public_field_access_get_method(name_text),
            self.create_public_field_access_set_method(name_text),
        ];
        let props = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            props,
        );
        self.factory_mut()
            .new_object_literal_expression(props, false)
    }

    fn create_public_field_access_has_method(&mut self, name_text: &str) -> ast::Node {
        let obj_name = self.factory_mut().new_identifier("obj");
        let obj_param = self
            .factory_mut()
            .new_parameter_declaration(None, None, obj_name, None, None, None);
        let parameters = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![obj_param],
        );
        let property_name = self
            .factory_mut()
            .new_string_literal(name_text, ast::TokenFlags::NONE);
        let in_token = self.factory_mut().new_token(ast::Kind::InKeyword);
        let obj = self.factory_mut().new_identifier("obj");
        let body =
            self.factory_mut()
                .new_binary_expression(None, property_name, None, in_token, obj);
        let equals_greater_than = self
            .factory_mut()
            .new_token(ast::Kind::EqualsGreaterThanToken);
        let arrow = self.factory_mut().new_arrow_function(
            None::<ast::ModifierList>,
            None::<ast::NodeList>,
            parameters,
            None::<ast::Node>,
            None::<ast::Node>,
            Some(equals_greater_than),
            body,
        );
        self.create_value_property("has", arrow)
    }

    fn create_public_field_access_get_method(&mut self, name_text: &str) -> ast::Node {
        let obj_name = self.factory_mut().new_identifier("obj");
        let obj_param = self
            .factory_mut()
            .new_parameter_declaration(None, None, obj_name, None, None, None);
        let parameters = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![obj_param],
        );
        let obj = self.factory_mut().new_identifier("obj");
        let name = self.factory_mut().new_identifier(name_text);
        let body = self.factory_mut().new_property_access_expression(
            obj,
            None::<ast::Node>,
            name,
            ast::NodeFlags::NONE,
        );
        let equals_greater_than = self
            .factory_mut()
            .new_token(ast::Kind::EqualsGreaterThanToken);
        let arrow = self.factory_mut().new_arrow_function(
            None::<ast::ModifierList>,
            None::<ast::NodeList>,
            parameters,
            None::<ast::Node>,
            None::<ast::Node>,
            Some(equals_greater_than),
            body,
        );
        self.create_value_property("get", arrow)
    }

    fn create_public_field_access_set_method(&mut self, name_text: &str) -> ast::Node {
        let obj_name = self.factory_mut().new_identifier("obj");
        let obj_param = self
            .factory_mut()
            .new_parameter_declaration(None, None, obj_name, None, None, None);
        let value_name = self.factory_mut().new_identifier("value");
        let value_param = self
            .factory_mut()
            .new_parameter_declaration(None, None, value_name, None, None, None);
        let parameters = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![obj_param, value_param],
        );
        let obj = self.factory_mut().new_identifier("obj");
        let name = self.factory_mut().new_identifier(name_text);
        let access = self.factory_mut().new_property_access_expression(
            obj,
            None::<ast::Node>,
            name,
            ast::NodeFlags::NONE,
        );
        let value = self.factory_mut().new_identifier("value");
        let assignment = self
            .emit_context
            .factory
            .new_assignment_expression(access, value);
        let statement = self.factory_mut().new_expression_statement(assignment);
        let statements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![statement],
        );
        let body = self.factory_mut().new_block(statements, false);
        let equals_greater_than = self
            .factory_mut()
            .new_token(ast::Kind::EqualsGreaterThanToken);
        let arrow = self.factory_mut().new_arrow_function(
            None::<ast::ModifierList>,
            None::<ast::NodeList>,
            parameters,
            None::<ast::Node>,
            None::<ast::Node>,
            Some(equals_greater_than),
            body,
        );
        self.create_value_property("set", arrow)
    }

    // Transforms all of the decorators for a declaration into an array of expressions.
    fn transform_all_decorators_of_declaration(
        &mut self,
        decorators: Vec<ast::Node>,
    ) -> Vec<ast::Node> {
        self.transform_all_decorators_of_declaration_worker(decorators, false)
    }

    fn transform_all_decorators_of_declaration_with_outer_this(
        &mut self,
        decorators: Vec<ast::Node>,
    ) -> Vec<ast::Node> {
        self.transform_all_decorators_of_declaration_worker(decorators, true)
    }

    fn transform_all_decorators_of_declaration_worker(
        &mut self,
        decorators: Vec<ast::Node>,
        capture_outer_this: bool,
    ) -> Vec<ast::Node> {
        decorators
            .into_iter()
            .filter_map(|decorator| {
                let expression = self.store_for(decorator).expression(decorator)?;
                let expression = if capture_outer_this {
                    self.outer_this_visit(expression)
                } else {
                    self.visit_node(Some(expression))
                        .unwrap_or_else(|| self.preserve_node(expression))
                };
                self.emit_context
                    .set_emit_flags(&expression, printer::EF_NO_COMMENTS);
                Some(self.transform_decorator(expression))
            })
            .collect()
    }

    // Transforms a decorator into an expression.
    fn transform_decorator(&mut self, expression: ast::Node) -> ast::Node {
        let inner_expression = {
            let source = self.store_for(expression);
            ast::skip_outer_expressions(source, expression, ast::OuterExpressionKinds::ALL)
        };
        // preserve the 'this' binding for an access expression
        if ast::is_access_expression(self.store_for(inner_expression), inner_expression) {
            let (target, this_arg) = self.create_call_binding(expression);
            let bind_call = self
                .emit_context
                .factory
                .new_function_bind_call(target, this_arg, &[]);
            return self.restore_outer_expressions(expression, bind_call);
        }
        expression
    }

    fn create_call_binding(&mut self, expression: ast::Node) -> (ast::Node, ast::Node) {
        let callee = {
            let source = self.store_for(expression);
            ast::skip_outer_expressions(source, expression, ast::OuterExpressionKinds::ALL)
        };
        if ast::is_super_property(self.store_for(callee), callee)
            || self.store_for(callee).kind(callee) == ast::Kind::SuperKeyword
        {
            return (self.preserve_node(callee), self.outer_this());
        }
        if self.emit_context.emit_flags(&callee) & printer::EF_HELPER_NAME != 0 {
            let void_zero = self.emit_context.factory.new_void_zero_expression();
            return (self.preserve_node(callee), void_zero);
        }
        match self.store_for(callee).kind(callee) {
            ast::Kind::PropertyAccessExpression => {
                let (receiver, question_dot_token, name, flags) = {
                    let source = self.store_for(callee);
                    (
                        source
                            .expression(callee)
                            .expect("property access should have expression"),
                        source.question_dot_token(callee),
                        source
                            .name(callee)
                            .expect("property access should have name"),
                        source.flags(callee),
                    )
                };
                if self.should_be_captured_in_temp_variable(receiver) {
                    let this_arg = self.emit_context.factory.new_temp_variable();
                    self.emit_context.add_variable_declaration(this_arg);
                    let receiver = self.preserve_node(receiver);
                    let assign = self
                        .emit_context
                        .factory
                        .new_assignment_expression(this_arg, receiver);
                    let question_dot_token =
                        question_dot_token.map(|token| self.preserve_node(token));
                    let name = self.preserve_node(name);
                    let target = self.factory_mut().new_property_access_expression(
                        assign,
                        question_dot_token,
                        name,
                        flags,
                    );
                    return (target, this_arg);
                }
                (self.preserve_node(callee), self.preserve_node(receiver))
            }
            ast::Kind::ElementAccessExpression => {
                let (receiver, question_dot_token, argument, flags) = {
                    let source = self.store_for(callee);
                    (
                        source
                            .expression(callee)
                            .expect("element access should have expression"),
                        source.question_dot_token(callee),
                        source
                            .argument_expression(callee)
                            .expect("element access should have argument"),
                        source.flags(callee),
                    )
                };
                if self.should_be_captured_in_temp_variable(receiver) {
                    let this_arg = self.emit_context.factory.new_temp_variable();
                    self.emit_context.add_variable_declaration(this_arg);
                    let receiver = self.preserve_node(receiver);
                    let assign = self
                        .emit_context
                        .factory
                        .new_assignment_expression(this_arg, receiver);
                    let question_dot_token =
                        question_dot_token.map(|token| self.preserve_node(token));
                    let argument = self.preserve_node(argument);
                    let target = self.factory_mut().new_element_access_expression(
                        assign,
                        question_dot_token,
                        argument,
                        flags,
                    );
                    return (target, this_arg);
                }
                (self.preserve_node(callee), self.preserve_node(receiver))
            }
            _ => {
                let void_zero = self.emit_context.factory.new_void_zero_expression();
                (expression, void_zero)
            }
        }
    }

    fn should_be_captured_in_temp_variable(&self, node: ast::Node) -> bool {
        // This is a simplified version of the general shouldBeCapturedInTempVariable from
        // nodeFactory with cacheIdentifiers=true, since createCallBinding in this transform
        // always caches identifiers.
        let source = self.store_for(node);
        let target = ast::skip_parentheses(source, node);
        match self.store_for(target).kind(target) {
            // cacheIdentifiers is always true for this transform's createCallBinding
            ast::Kind::Identifier => true,
            ast::Kind::ThisKeyword
            | ast::Kind::NumericLiteral
            | ast::Kind::BigIntLiteral
            | ast::Kind::StringLiteral => false,
            _ => true,
        }
    }

    fn restore_outer_expressions(
        &mut self,
        outer_expression: ast::Node,
        inner: ast::Node,
    ) -> ast::Node {
        let (kind, expression) = {
            let source = self.store_for(outer_expression);
            (
                source.kind(outer_expression),
                source.expression(outer_expression),
            )
        };
        if kind != ast::Kind::ParenthesizedExpression {
            return inner;
        }
        let Some(expression) = expression else {
            return inner;
        };
        let restored = self.restore_outer_expressions(expression, inner);
        if outer_expression.store_id() == self.factory().store().store_id() {
            self.factory_mut()
                .update_parenthesized_expression(outer_expression, restored)
        } else {
            let source = self.source;
            self.factory_mut()
                .update_parenthesized_expression_from_store(source, outer_expression, restored)
        }
    }

    fn get_decorators(&self, node: ast::Node) -> Vec<ast::Node> {
        let source = self.store_for(node);
        source
            .source_modifiers(node)
            .map(|modifiers| {
                modifiers
                    .iter()
                    .filter_map(|modifier| {
                        (self.store_for(modifier).kind(modifier) == ast::Kind::Decorator)
                            .then_some(modifier)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn create_metadata(&mut self, class_super: Option<ast::Node>) -> (ast::Node, ast::Node) {
        let metadata = self.emit_context.factory.new_unique_name_ex(
            "_metadata",
            AutoGenerateOptions {
                flags: GeneratedIdentifierFlags::OPTIMISTIC | GeneratedIdentifierFlags::FILE_LEVEL,
                ..Default::default()
            },
        );
        let initializer = self.create_metadata_initialization_expression(class_super);
        let declaration =
            self.factory_mut()
                .new_variable_declaration(metadata, None, None, initializer);
        let declaration_list = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![declaration],
        );
        let declarations = self
            .factory_mut()
            .new_variable_declaration_list(declaration_list, ast::NodeFlags::CONST);
        let statement = self
            .factory_mut()
            .new_variable_statement(None::<ast::ModifierList>, declarations);
        (statement, metadata)
    }

    fn create_metadata_initialization_expression(
        &mut self,
        class_super: Option<ast::Node>,
    ) -> ast::Node {
        let symbol = self.factory_mut().new_identifier("Symbol");
        let typeof_symbol = self.factory_mut().new_type_of_expression(symbol);
        let function_string = self
            .factory_mut()
            .new_string_literal("function", ast::TokenFlags::NONE);
        let type_check = self
            .emit_context
            .factory
            .new_strict_equality_expression(typeof_symbol, function_string);
        let symbol = self.factory_mut().new_identifier("Symbol");
        let metadata = self.factory_mut().new_identifier("metadata");
        let symbol_metadata = self.factory_mut().new_property_access_expression(
            symbol,
            None::<ast::Node>,
            metadata,
            ast::NodeFlags::NONE,
        );
        let condition = self
            .emit_context
            .factory
            .new_logical_and_expression(type_check, symbol_metadata);
        let object = self.factory_mut().new_identifier("Object");
        let create = self.factory_mut().new_identifier("create");
        let object_create = self.factory_mut().new_property_access_expression(
            object,
            None::<ast::Node>,
            create,
            ast::NodeFlags::NONE,
        );
        let prototype = if let Some(class_super) = class_super {
            let symbol = self.factory_mut().new_identifier("Symbol");
            let metadata = self.factory_mut().new_identifier("metadata");
            let symbol_metadata = self.factory_mut().new_property_access_expression(
                symbol,
                None::<ast::Node>,
                metadata,
                ast::NodeFlags::NONE,
            );
            let super_metadata = self.factory_mut().new_element_access_expression(
                class_super,
                None::<ast::Node>,
                Some(symbol_metadata),
                ast::NodeFlags::NONE,
            );
            let null = self.factory_mut().new_token(ast::Kind::NullKeyword);
            let operator = self
                .factory_mut()
                .new_token(ast::Kind::QuestionQuestionToken);
            self.factory_mut()
                .new_binary_expression(None, super_metadata, None, operator, null)
        } else {
            self.factory_mut().new_token(ast::Kind::NullKeyword)
        };
        let arguments = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![prototype],
        );
        let when_true = self.factory_mut().new_call_expression(
            object_create,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        );
        let question = self.factory_mut().new_token(ast::Kind::QuestionToken);
        let colon = self.factory_mut().new_token(ast::Kind::ColonToken);
        let when_false = self.emit_context.factory.new_void_zero_expression();
        self.factory_mut()
            .new_conditional_expression(condition, question, when_true, colon, when_false)
    }

    fn create_symbol_metadata(
        &mut self,
        class_reference: ast::Node,
        metadata_reference: ast::Node,
    ) -> ast::Node {
        // Object.defineProperty(target, Symbol.metadata, { configurable: true, writable: true, enumerable: true, value })
        let object = self.factory_mut().new_identifier("Object");
        let define_property = self.factory_mut().new_identifier("defineProperty");
        let object_define_property = self.factory_mut().new_property_access_expression(
            object,
            None::<ast::Node>,
            define_property,
            ast::NodeFlags::NONE,
        );
        let symbol = self.factory_mut().new_identifier("Symbol");
        let metadata = self.factory_mut().new_identifier("metadata");
        let symbol_metadata = self.factory_mut().new_property_access_expression(
            symbol,
            None::<ast::Node>,
            metadata,
            ast::NodeFlags::NONE,
        );
        let descriptor = self.create_symbol_metadata_descriptor(metadata_reference);
        let arguments = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![class_reference, symbol_metadata, descriptor],
        );
        let call = self.factory_mut().new_call_expression(
            object_define_property,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        );
        let statement = self.factory_mut().new_expression_statement(call);
        let if_statement = self
            .factory_mut()
            .new_if_statement(metadata_reference, statement, None);
        self.emit_context
            .mark_emit_node(&if_statement, printer::EF_SINGLE_LINE);
        if_statement
    }

    fn create_symbol_metadata_descriptor(&mut self, metadata_reference: ast::Node) -> ast::Node {
        let props = vec![
            self.create_boolean_property("enumerable", true),
            self.create_boolean_property("configurable", true),
            self.create_boolean_property("writable", true),
            self.create_value_property("value", metadata_reference),
        ];
        let props = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            props,
        );
        self.factory_mut()
            .new_object_literal_expression(props, false)
    }

    fn create_boolean_property(&mut self, name: &str, value: bool) -> ast::Node {
        let name = self.factory_mut().new_identifier(name);
        let value = if value {
            self.emit_context.factory.new_true_expression()
        } else {
            self.emit_context.factory.new_false_expression()
        };
        self.factory_mut().new_property_assignment(
            None::<ast::ModifierList>,
            name,
            None,
            None,
            value,
        )
    }

    fn create_value_property(&mut self, name: &str, value: ast::Node) -> ast::Node {
        let name = self.factory_mut().new_identifier(name);
        self.factory_mut().new_property_assignment(
            None::<ast::ModifierList>,
            name,
            None,
            None,
            value,
        )
    }

    fn create_let(&mut self, name: ast::Node, initializer: Option<ast::Node>) -> ast::Node {
        let declaration =
            self.factory_mut()
                .new_variable_declaration(name, None, None, initializer);
        let declarations = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![declaration],
        );
        let declarations = self
            .factory_mut()
            .new_variable_declaration_list(declarations, ast::NodeFlags::LET);
        self.factory_mut()
            .new_variable_statement(None::<ast::ModifierList>, declarations)
    }

    fn create_var(&mut self, name: ast::Node, initializer: Option<ast::Node>) -> ast::Node {
        let declaration =
            self.factory_mut()
                .new_variable_declaration(name, None, None, initializer);
        let declarations = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![declaration],
        );
        let declarations = self
            .factory_mut()
            .new_variable_declaration_list(declarations, ast::NodeFlags::NONE);
        self.factory_mut()
            .new_variable_statement(None::<ast::ModifierList>, declarations)
    }

    fn create_var_declaration_statement(&mut self, names: &[ast::Node]) -> ast::Node {
        let declarations = names
            .iter()
            .copied()
            .map(|name| {
                self.factory_mut()
                    .new_variable_declaration(name, None, None, None::<ast::Node>)
            })
            .collect::<Vec<_>>();
        let declarations = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            declarations,
        );
        let declarations = self
            .factory_mut()
            .new_variable_declaration_list(declarations, ast::NodeFlags::NONE);
        self.factory_mut()
            .new_variable_statement(None::<ast::ModifierList>, declarations)
    }

    fn new_array_literal(&mut self, elements: Vec<ast::Node>) -> ast::Node {
        let elements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            elements,
        );
        self.factory_mut()
            .new_array_literal_expression(elements, false)
    }

    fn strip_parameter_decorators(
        &mut self,
        loc: core::TextRange,
        range: core::TextRange,
        parameters: Vec<ast::Node>,
    ) -> ast::NodeList {
        let params: Vec<_> = parameters
            .into_iter()
            .map(|parameter| {
                let parameter_is_factory_node =
                    parameter.store_id() == self.factory().store().store_id();
                let (dot_dot_dot_token, name, initializer) = {
                    let source = self.store_for(parameter);
                    (
                        source.dot_dot_dot_token(parameter),
                        source.name(parameter),
                        source.initializer(parameter),
                    )
                };
                let dot_dot_dot_token = dot_dot_dot_token.map(|token| self.preserve_node(token));
                let name = name.map(|name| self.preserve_node(name));
                let initializer = initializer.map(|initializer| self.preserve_node(initializer));
                if parameter_is_factory_node {
                    self.factory_mut().update_parameter_declaration(
                        parameter,
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
                        parameter,
                        None::<ast::ModifierList>,
                        dot_dot_dot_token,
                        name,
                        None::<ast::Node>,
                        None::<ast::Node>,
                        initializer,
                    )
                }
            })
            .collect();
        self.factory_mut().new_node_list(loc, range, params)
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

    fn preserve_optional_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        node.map(|node| self.preserve_node(node))
    }

    fn preserve_optional_source_node_list(
        &mut self,
        nodes: Option<ast::SourceNodeList<'_>>,
    ) -> Option<ast::NodeList> {
        nodes.map(|nodes| {
            self.import_state
                .preserve_source_node_list(&mut self.emit_context.factory.node_factory, nodes)
        })
    }

    fn preserve_node(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
            node
        } else {
            self.import_state.preserve_node(
                self.source,
                &mut self.emit_context.factory.node_factory,
                node,
            )
        }
    }

    fn clone_node_for_reuse(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
            return self.factory_mut().clone_node(node);
        }
        self.preserve_node(node)
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
}

impl<'source> ast::AstVisitEachChildRuntime<'source> for EsDecoratorRuntime<'_, 'source> {
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
        EsDecoratorRuntime::preserve_node(self, node)
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
            let result = self.visit(&node);
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
        let updated = self.visit_node(node);
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

impl<'source> ast::AstGeneratedVisitEachChild<'source> for EsDecoratorRuntime<'_, 'source> {}
