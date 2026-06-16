use ts_ast as ast;
use ts_binder as binder;
use ts_core as core;
use ts_evaluator as evaluator;
use ts_nodebuilder as nodebuilder;

use crate::EmitContext;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum SymbolAccessibility {
    #[default]
    Accessible = 0,
    NotAccessible,
    CannotBeNamed,
    NotResolved,
}

#[derive(Default, Clone)]
pub struct SymbolAccessibilityResult {
    pub accessibility: SymbolAccessibility,
    pub aliases_to_make_visible: Vec<ast::Node>, // aliases that need to have this symbol visible
    pub error_symbol_name: String,               // Optional - symbol name that results in error
    pub error_node: Option<ast::Node>,           // Optional - node that results in error
    pub error_module_name: String, // Optional - If the symbol is not visible from module, module's name
}

/**
 * Indicates how to serialize the name for a TypeReferenceNode when emitting decorator metadata
 *
 * @internal
 */
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum TypeReferenceSerializationKind {
    // The TypeReferenceNode could not be resolved.
    // The type name should be emitted using a safe fallback.
    Unknown = 0,

    // The TypeReferenceNode resolves to a type with a constructor
    // function that can be reached at runtime (e.g. a `class`
    // declaration or a `var` declaration for the static side
    // of a type, such as the global `Promise` type in lib.d.ts).
    TypeWithConstructSignatureAndValue,

    // The TypeReferenceNode resolves to a Void-like, Nullable, or Never type.
    VoidNullableOrNeverType,

    // The TypeReferenceNode resolves to a Number-like type.
    NumberLikeType,

    // The TypeReferenceNode resolves to a BigInt-like type.
    BigIntLikeType,

    // The TypeReferenceNode resolves to a String-like type.
    StringLikeType,

    // The TypeReferenceNode resolves to a Boolean-like type.
    BooleanType,

    // The TypeReferenceNode resolves to an Array-like type.
    ArrayLikeType,

    // The TypeReferenceNode resolves to the ESSymbol type.
    ESSymbolType,

    // The TypeReferenceNode resolved to the global Promise constructor symbol.
    Promise,

    // The TypeReferenceNode resolves to a Function type or a type with call signatures.
    TypeWithCallSignature,

    // The TypeReferenceNode resolves to any other type.
    ObjectType,
}

pub trait EmitResolver: binder::BinderReferenceResolver {
    fn source_file_store(&self, node: ast::Node) -> Option<&ast::AstStore>;
    fn is_referenced_alias_declaration(&mut self, node: ast::Node) -> bool;
    fn is_value_alias_declaration(&mut self, node: ast::Node) -> bool;
    fn is_top_level_value_import_equals_with_entity_name(&mut self, node: ast::Node) -> bool;
    fn mark_linked_references_recursively(&mut self, file: &ast::SourceFile);
    fn get_external_module_file_from_declaration(
        &mut self,
        node: ast::Node,
    ) -> Option<ast::SourceFile>;
    fn get_effective_declaration_flags(
        &mut self,
        node: ast::Node,
        flags: ast::ModifierFlags,
    ) -> ast::ModifierFlags;
    fn get_resolution_mode_override(&mut self, node: ast::Node) -> core::ResolutionMode;

    // decorator metadata
    fn get_type_reference_serialization_kind(
        &mut self,
        type_name: ast::Node,
        serial_scope: ast::Node,
    ) -> TypeReferenceSerializationKind;

    // const enum inlining
    fn get_constant_value(&mut self, node: ast::Node) -> Option<ts_evaluator::Value>;

    // JSX Emit
    fn get_jsx_factory_entity(&mut self, location: ast::Node) -> Option<ast::Node>;
    fn get_jsx_fragment_factory_entity(&mut self, location: ast::Node) -> Option<ast::Node>;
    fn get_jsx_factory_entity_text(&mut self, location: ast::Node) -> Option<String>;
    fn get_jsx_fragment_factory_entity_text(&mut self, location: ast::Node) -> Option<String>;
    fn get_referenced_export_container_for_identifier_text(
        &mut self,
        location: ast::Node,
        name: &str,
        prefix_locals: bool,
    ) -> Option<ast::Node>;
    fn set_referenced_import_declaration(
        &self,
        node: ast::IdentifierNode,
        ref_declaration: ast::Declaration,
    ); // for overriding the reference resolver behavior for generated identifiers

    // declaration emit checker functionality projections
    fn precalculate_declaration_emit_visibility(&mut self, file: &ast::SourceFile);
    fn is_symbol_accessible(
        &mut self,
        symbol: ast::SymbolIdentity,
        enclosing_declaration: ast::Node,
        meaning: ast::SymbolFlags,
        should_compute_alias_to_mark_visible: bool,
    ) -> SymbolAccessibilityResult;
    fn is_entity_name_visible(
        &mut self,
        entity_name: ast::Node,
        enclosing_declaration: ast::Node,
    ) -> SymbolAccessibilityResult; // previously SymbolVisibilityResult in strada - ErrorModuleName never set
    fn is_expando_function_declaration(&mut self, node: ast::Node) -> bool;
    fn is_expando_function_declaration_unsafe(&mut self, node: ast::Node) -> bool;
    fn should_emit_function_properties(&mut self, node: ast::Node) -> bool;
    fn get_symbol_name(&mut self, symbol: ast::SymbolIdentity) -> String;
    fn get_symbol_value_declaration(&mut self, symbol: ast::SymbolIdentity) -> Option<ast::Node>;
    fn set_expando_namespace_metadata(
        &mut self,
        synthesized_namespace: ast::Node,
        declaration: ast::Node,
        properties: &[ast::SymbolIdentity],
    );
    fn is_literal_const_declaration(&mut self, node: ast::Node) -> bool;
    fn requires_adding_implicit_undefined_with_symbol(
        &mut self,
        node: ast::Node,
        symbol: ast::SymbolIdentity,
        enclosing_declaration: Option<ast::Node>,
    ) -> bool;
    fn requires_adding_implicit_undefined(
        &mut self,
        node: ast::Node,
        enclosing_declaration: Option<ast::Node>,
    ) -> bool;
    fn is_declaration_visible(&mut self, node: ast::Node) -> bool;
    fn is_import_required_by_augmentation(&mut self, decl: ast::Node) -> bool;
    fn is_definitely_reference_to_global_symbol_object(&mut self, node: ast::Node) -> bool;
    fn is_implementation_of_overload(&mut self, node: ast::Node) -> bool;
    fn is_first_declaration_of_symbol(&mut self, node: ast::Node) -> bool;
    fn is_assignment_declaration(&mut self, node: ast::Node) -> bool;
    fn is_common_js_alias_export(&mut self, node: ast::Node) -> bool;
    fn get_element_access_expression_name(&mut self, expression: ast::Node) -> String;
    fn get_enum_member_value(&mut self, node: ast::Node) -> evaluator::Result;
    fn is_late_bound(&mut self, node: ast::Node) -> bool;
    fn is_optional_parameter(&mut self, node: ast::Node) -> bool;

    // isolatedDeclarations-specific declaration emit
    fn get_properties_of_container_function(&mut self, node: ast::Node)
    -> Vec<ast::SymbolIdentity>;
    fn requires_adding_implicit_undefined_unsafe(
        &mut self,
        node: ast::Node,
        symbol: ast::SymbolIdentity,
        enclosing_declaration: Option<ast::Node>,
    ) -> bool;

    // Node construction for declaration emit
    fn create_type_of_declaration(
        &mut self,
        emit_context: &mut EmitContext,
        declaration: ast::Node,
        enclosing_declaration: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Option<ast::Node>;
    fn get_declaration_statements_for_source_file(
        &mut self,
        emit_context: &mut EmitContext,
        source_file: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Vec<ast::Node>;
    fn create_return_type_of_signature_declaration(
        &mut self,
        emit_context: &mut EmitContext,
        signature_declaration: ast::Node,
        enclosing_declaration: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Option<ast::Node>;
    fn create_signature_declaration_with_synthetic_rest_parameter(
        &mut self,
        emit_context: &mut EmitContext,
        declaration: ast::Node,
        kind: ast::Kind,
        modifiers: Vec<ast::Node>,
        name: Option<ast::Node>,
        enclosing_declaration: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Option<ast::Node>;
    fn create_type_parameters_of_signature_declaration(
        &mut self,
        emit_context: &mut EmitContext,
        signature_declaration: ast::Node,
        enclosing_declaration: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Vec<ast::Node>;
    fn create_literal_const_value(
        &mut self,
        emit_context: &mut EmitContext,
        node: ast::Node,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Option<ast::Node>;
    fn create_type_of_expression(
        &mut self,
        emit_context: &mut EmitContext,
        expression: ast::Node,
        enclosing_declaration: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Option<ast::Node>;
    fn create_late_bound_index_signatures(
        &mut self,
        emit_context: &mut EmitContext,
        container: ast::Node,
        enclosing_declaration: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Vec<ast::Node>;
    fn try_js_type_node_to_type_node(
        &mut self,
        emit_context: &mut EmitContext,
        type_node: ast::Node,
        enclosing_declaration: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: &mut dyn nodebuilder::SymbolTracker,
    ) -> Option<ast::Node>;
}
