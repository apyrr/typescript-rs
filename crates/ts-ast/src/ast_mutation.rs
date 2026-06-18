// Phase-specific AST mutation APIs. Generated payload setters stay crate-private;
// external crates can only patch the semantic slots their phase owns.

use crate::*;
use ts_core as core;

pub struct ParserAstAccess<'a> {
    store: &'a AstStore,
}

impl<'a> ParserAstAccess<'a> {
    pub fn source_file_statement_nodes(self, source_file: Node) -> Vec<Node> {
        self.store
            .source_file_statement_nodes_for_parser(source_file)
    }

    pub fn source_file_statement_loc(self, source_file: Node) -> core::TextRange {
        self.store.source_file_statement_loc_for_parser(source_file)
    }

    pub fn source_file_statement_list(self, source_file: Node) -> SourceNodeList<'a> {
        self.store
            .source_file_statement_list_for_parser(source_file)
    }

    pub fn modifier_list_nodes(self, modifiers: ModifierList) -> Vec<Node> {
        self.store.modifier_list_nodes_for_parser(modifiers)
    }

    pub fn modifier_flags(self, modifiers: ModifierList) -> ModifierFlags {
        self.store.modifier_flags_for_parser(modifiers)
    }
}

pub struct TransformAstAccess<'a> {
    store: &'a AstStore,
}

impl TransformAstAccess<'_> {
    pub fn modifier_list_loc(self, modifiers: ModifierList) -> core::TextRange {
        self.store.modifier_list_loc_for_transform(modifiers)
    }

    pub fn modifier_list_range(self, modifiers: ModifierList) -> core::TextRange {
        self.store.modifier_list_range_for_transform(modifiers)
    }
}

impl AstStore {
    pub fn parser_access(&self) -> ParserAstAccess<'_> {
        ParserAstAccess { store: self }
    }

    pub fn transform_access(&self) -> TransformAstAccess<'_> {
        TransformAstAccess { store: self }
    }

    pub(crate) fn node_list_is_missing_for_parser(&self, list: NodeList) -> bool {
        self.node_list(list.id()).is_missing()
    }

    pub(crate) fn source_file_statement_nodes_for_parser(&self, source_file: Node) -> Vec<Node> {
        self.source_file_statement_list_for_parser(source_file)
            .nodes()
    }

    pub(crate) fn source_file_statement_loc_for_parser(
        &self,
        source_file: Node,
    ) -> core::TextRange {
        self.source_file_statement_list_for_parser(source_file)
            .loc()
    }

    pub(crate) fn source_file_statement_list_for_parser(
        &self,
        source_file: Node,
    ) -> SourceNodeList<'_> {
        SourceNodeList::new(self, self.as_source_file(source_file).statements)
    }

    pub(crate) fn node_list_nodes_for_checker(&self, list: SourceNodeList<'_>) -> Vec<Node> {
        list.nodes()
    }

    pub(crate) fn node_list_pos_for_checker(&self, list: SourceNodeList<'_>) -> i32 {
        list.pos()
    }

    pub(crate) fn node_list_is_empty_for_checker(&self, list: SourceNodeList<'_>) -> bool {
        list.is_empty()
    }

    pub(crate) fn checker_modifiers_for_update(
        &self,
        node: Node,
    ) -> Option<SourceModifierList<'_>> {
        self.source_modifiers(node)
    }

    pub(crate) fn checker_parameters_for_update(&self, node: Node) -> Option<SourceNodeList<'_>> {
        self.source_parameters(node)
    }

    pub(crate) fn checker_type_arguments_for_update(
        &self,
        node: Node,
    ) -> Option<SourceNodeList<'_>> {
        self.source_type_arguments(node)
    }

    pub(crate) fn checker_type_parameters_for_update(
        &self,
        node: Node,
    ) -> Option<SourceNodeList<'_>> {
        self.source_type_parameters(node)
    }

    pub(crate) fn node_list_nodes_for_ls(&self, list: SourceNodeList<'_>) -> Vec<Node> {
        list.nodes()
    }

    pub(crate) fn node_list_loc_for_ls(&self, list: SourceNodeList<'_>) -> core::TextRange {
        list.loc()
    }

    pub(crate) fn node_list_range_for_ls(&self, list: SourceNodeList<'_>) -> core::TextRange {
        list.range()
    }

    pub(crate) fn ls_attributes_for_update(&self, node: Node) -> Option<SourceNodeList<'_>> {
        self.source_attributes(node)
    }

    pub(crate) fn ls_attribute_nodes(&self, node: Node) -> Vec<Node> {
        self.source_attributes(node)
            .map(|attributes| attributes.nodes())
            .unwrap_or_default()
    }

    pub(crate) fn ls_modifiers_for_update(&self, node: Node) -> Option<SourceModifierList<'_>> {
        self.source_modifiers(node)
    }

    pub(crate) fn ls_parameters_for_update(&self, node: Node) -> Option<SourceNodeList<'_>> {
        self.source_parameters(node)
    }

    pub(crate) fn ls_type_arguments_for_update(&self, node: Node) -> Option<SourceNodeList<'_>> {
        self.source_type_arguments(node)
    }

    pub(crate) fn ls_type_parameters_for_update(&self, node: Node) -> Option<SourceNodeList<'_>> {
        self.source_type_parameters(node)
    }

    pub(crate) fn source_node_list_for_nodecopy(&self, list: NodeListId) -> SourceNodeList<'_> {
        SourceNodeList::new(self, list)
    }

    pub(crate) fn source_modifier_list_for_nodecopy(
        &self,
        modifiers: ModifierListId,
    ) -> SourceModifierList<'_> {
        SourceModifierList::new(self, modifiers)
    }

    pub(crate) fn source_raw_node_slice_for_nodecopy(
        &self,
        nodes: RawNodeSliceId,
    ) -> SourceRawNodeSlice<'_> {
        SourceRawNodeSlice::new(self, nodes)
    }

    pub(crate) fn source_node_list_for_auto_import(&self, list: NodeListId) -> SourceNodeList<'_> {
        SourceNodeList::new(self, list)
    }

    pub(crate) fn source_modifier_list_for_auto_import(
        &self,
        modifiers: ModifierListId,
    ) -> SourceModifierList<'_> {
        SourceModifierList::new(self, modifiers)
    }

    pub(crate) fn source_raw_node_slice_for_auto_import(
        &self,
        nodes: RawNodeSliceId,
    ) -> SourceRawNodeSlice<'_> {
        SourceRawNodeSlice::new(self, nodes)
    }

    pub(crate) fn modifier_list_nodes_for_parser(&self, modifiers: ModifierList) -> Vec<Node> {
        self.modifier_list(modifiers.id()).nodes().iter().collect()
    }

    pub(crate) fn modifier_flags_for_parser(&self, modifiers: ModifierList) -> ModifierFlags {
        self.modifier_list(modifiers.id()).modifier_flags()
    }

    pub(crate) fn modifier_list_loc_for_transform(
        &self,
        modifiers: ModifierList,
    ) -> core::TextRange {
        self.source_modifier_list_from_handle(modifiers).loc()
    }

    pub(crate) fn modifier_list_range_for_transform(
        &self,
        modifiers: ModifierList,
    ) -> core::TextRange {
        self.source_modifier_list_from_handle(modifiers).range()
    }
}

impl NodeFactory {
    pub fn finish_parsed_node_header(
        &mut self,
        node: Node,
        loc: core::TextRange,
        context_flags: NodeFlags,
        has_parse_error: bool,
    ) {
        self.set_loc(node, loc);
        self.add_flags(node, context_flags);
        if has_parse_error {
            self.add_flags(node, NodeFlags::THIS_NODE_HAS_ERROR);
        }
    }

    pub fn mark_parsed_modifier_ambient(&mut self, node: Node) {
        self.add_flags(node, NodeFlags::AMBIENT);
    }

    pub fn mark_parsed_optional_chain(&mut self, node: Node) {
        self.add_flags(node, NodeFlags::OPTIONAL_CHAIN);
    }

    pub fn mark_parsed_implicit_export(&mut self, node: Node, loc: core::TextRange) {
        self.set_loc(node, loc);
        self.set_flags(node, NodeFlags::REPARSED);
    }

    pub fn link_parsed_parent(&mut self, node: Node, parent: Option<Node>) {
        self.set_parent(node, parent);
    }

    pub fn parsed_type_arguments_for_update(&self, node: Node) -> Option<NodeList> {
        self.store.type_arguments_id(node).map(NodeList::from_id)
    }

    pub fn parsed_jsx_children_for_update(&self, node: Node) -> NodeList {
        NodeList::from_id(self.store.jsx_children_id(node))
    }

    pub fn parsed_node_list_is_missing(&self, list: NodeList) -> bool {
        self.store.node_list(list.id()).is_missing()
    }

    pub fn parsed_node_list_loc(&self, list: NodeList) -> core::TextRange {
        self.store.node_list(list.id()).loc()
    }

    pub fn parsed_node_list_nodes(&self, list: NodeList) -> Vec<Node> {
        self.store.node_list(list.id()).iter().collect()
    }

    pub fn emit_node_list_range(&self, list: NodeList) -> core::TextRange {
        self.store.node_list(list.id()).range()
    }

    pub fn emit_node_list_loc(&self, list: NodeList) -> core::TextRange {
        self.store.node_list(list.id()).loc()
    }

    pub fn emit_node_list_nodes(&self, list: NodeList) -> Vec<Node> {
        self.store.node_list(list.id()).iter().collect()
    }

    pub fn parsed_node_list_first(&self, list: NodeList) -> Option<Node> {
        self.store.node_list(list.id()).first()
    }

    pub fn parsed_node_list_last(&self, list: NodeList) -> Option<Node> {
        self.store.node_list(list.id()).iter().last()
    }

    pub fn parsed_modifier_list_nodes(&self, modifiers: ModifierList) -> Vec<Node> {
        self.store
            .modifier_list(modifiers.id())
            .nodes()
            .iter()
            .collect()
    }

    pub fn parsed_modifier_flags(&self, modifiers: ModifierList) -> ModifierFlags {
        self.store.modifier_list(modifiers.id()).modifier_flags()
    }

    pub fn adopt_parsed_children(&mut self, node: Node) {
        self.set_parent_in_children(node);
    }

    pub fn finish_reparsed_node_from_source(
        &mut self,
        node: Node,
        source_node: Node,
        context_flags: NodeFlags,
    ) {
        let loc = self.store.loc(source_node);
        self.set_loc(node, loc);
        self.set_flags(node, context_flags | NodeFlags::REPARSED);
    }

    pub fn finish_reparsed_synthetic_node_at(
        &mut self,
        node: Node,
        loc: core::TextRange,
        context_flags: NodeFlags,
    ) {
        self.set_loc(node, loc);
        self.set_flags(node, context_flags | NodeFlags::REPARSED);
    }

    pub fn link_reparsed_parent(&mut self, node: Node, parent: Option<Node>) {
        self.set_parent(node, parent);
    }

    pub fn adopt_reparsed_children(&mut self, node: Node) {
        self.set_parent_in_children(node);
    }

    pub fn place_checker_synthetic_node(&mut self, node: Node, loc: core::TextRange) {
        self.set_loc(node, loc);
    }

    pub fn mark_checker_synthesized(&mut self, node: Node) {
        self.add_flags(node, NODE_FLAGS_SYNTHESIZED);
    }

    pub fn link_checker_synthetic_parent(&mut self, node: Node, parent: Option<Node>) {
        self.store.set_synthetic_parent(node, parent);
    }

    pub fn adopt_checker_synthetic_children(&mut self, node: Node) {
        self.set_parent_in_children(node);
    }

    pub fn place_emit_synthetic_node(&mut self, node: Node, loc: core::TextRange) {
        self.set_loc(node, loc);
    }

    pub fn clear_emit_synthetic_node(&mut self, node: Node) {
        self.remove_flags(node, NODE_FLAGS_SYNTHESIZED);
    }

    pub fn link_emit_synthetic_parent(&mut self, node: Node, parent: Option<Node>) {
        self.store.set_synthetic_parent(node, parent);
    }

    pub fn adopt_emit_synthetic_children(&mut self, node: Node) {
        self.set_parent_in_children(node);
    }

    pub fn place_change_tracker_node(&mut self, node: Node, loc: core::TextRange) {
        self.set_loc(node, loc);
    }

    pub fn mark_change_tracker_ambient_export_context(&mut self, node: Node) {
        self.set_flags(
            node,
            NodeFlags::AMBIENT | NodeFlags::EXPORT_CONTEXT | NodeFlags::CONTEXT_FLAGS,
        );
    }

    pub fn link_change_tracker_parent(&mut self, node: Node, parent: Option<Node>) {
        self.set_parent(node, parent);
    }

    pub fn adopt_change_tracker_children(&mut self, node: Node) {
        self.set_parent_in_children(node);
    }

    pub fn place_transformed_node(&mut self, node: Node, loc: core::TextRange) {
        self.set_loc(node, loc);
    }

    pub fn link_external_helper_parent(&mut self, node: Node, parent: Option<Node>) {
        self.store.set_synthetic_parent(node, parent);
    }

    pub fn set_parsed_string_literal_text(&mut self, node: Node, value: String) {
        self.store.set_generated_string_literal_text(node, value);
    }

    pub fn set_parsed_no_substitution_template_literal_text(&mut self, node: Node, value: String) {
        self.store
            .set_generated_no_substitution_template_literal_text(node, value);
    }

    pub fn set_parsed_numeric_literal_text(&mut self, node: Node, value: String) {
        self.store.set_generated_numeric_literal_text(node, value);
    }

    pub fn force_cloned_string_literal_single_quote_from(
        &mut self,
        cloned: Node,
        source_token_flags: TokenFlags,
    ) {
        self.store.set_generated_string_literal_token_flags(
            cloned,
            source_token_flags | TOKEN_FLAGS_SINGLE_QUOTE,
        );
    }

    pub fn force_cloned_string_literal_double_quote_from(
        &mut self,
        cloned: Node,
        source_token_flags: TokenFlags,
    ) {
        self.store.set_generated_string_literal_token_flags(
            cloned,
            TokenFlags(source_token_flags.0 & !TOKEN_FLAGS_SINGLE_QUOTE.0),
        );
    }

    pub fn clear_synthetic_class_declaration_name(&mut self, node: Node) {
        self.store
            .set_generated_class_declaration_name(node, None::<Node>);
    }

    pub fn set_reparsed_type_alias_type_parameters(&mut self, node: Node, value: Option<NodeList>) {
        self.store
            .set_generated_type_alias_declaration_type_parameters(node, value);
    }

    pub fn set_reparsed_type_alias_type_node(
        &mut self,
        node: Node,
        value: impl Into<Option<Node>>,
    ) {
        self.store
            .set_generated_type_alias_declaration_type_node(node, value);
    }

    pub fn set_reparsed_import_clause_phase_modifier(&mut self, node: Node, value: Option<Kind>) {
        self.store
            .set_generated_import_clause_phase_modifier(node, value);
    }

    pub fn set_reparsed_shorthand_object_assignment_initializer(
        &mut self,
        node: Node,
        value: impl Into<Option<Node>>,
    ) {
        self.store
            .set_generated_shorthand_property_assignment_object_assignment_initializer(node, value);
    }

    pub fn set_reparsed_binary_expression_right(
        &mut self,
        node: Node,
        value: impl Into<Option<Node>>,
    ) {
        self.store
            .set_generated_binary_expression_right(node, value);
    }

    pub fn set_reparsed_parameter_question_token(
        &mut self,
        node: Node,
        value: impl Into<Option<Node>>,
    ) {
        self.store
            .set_generated_parameter_declaration_question_token(node, value);
    }

    pub fn set_reparsed_heritage_clause_types(&mut self, node: Node, value: NodeList) {
        self.store.set_generated_heritage_clause_types(node, value);
    }

    pub fn set_reparsed_expression_with_type_arguments_type_arguments(
        &mut self,
        node: Node,
        value: Option<NodeList>,
    ) {
        self.store
            .set_generated_expression_with_type_arguments_type_arguments(node, value);
    }

    pub fn set_reparsed_class_type_parameters(&mut self, node: Node, value: Option<NodeList>) {
        match self.kind(node) {
            Kind::ClassDeclaration => self
                .store
                .set_generated_class_declaration_type_parameters(node, value),
            Kind::ClassExpression => self
                .store
                .set_generated_class_expression_type_parameters(node, value),
            _ => panic!(
                "Unhandled case in NodeFactory::set_reparsed_class_type_parameters: {}",
                self.kind(node)
            ),
        }
    }

    pub fn set_reparsed_class_heritage_clauses(&mut self, node: Node, value: Option<NodeList>) {
        match self.kind(node) {
            Kind::ClassDeclaration => self
                .store
                .set_generated_class_declaration_heritage_clauses(node, value),
            Kind::ClassExpression => self
                .store
                .set_generated_class_expression_heritage_clauses(node, value),
            _ => panic!(
                "Unhandled case in NodeFactory::set_reparsed_class_heritage_clauses: {}",
                self.kind(node)
            ),
        }
    }

    pub fn set_reparsed_type_node(&mut self, node: Node, type_node: Option<Node>) {
        match self.kind(node) {
            Kind::VariableDeclaration => self
                .store
                .set_generated_variable_declaration_type_node(node, type_node),
            Kind::Parameter => self
                .store
                .set_generated_parameter_declaration_type_node(node, type_node),
            Kind::PropertySignature => self
                .store
                .set_generated_property_signature_declaration_type_node(node, type_node),
            Kind::PropertyDeclaration => self
                .store
                .set_generated_property_declaration_type_node(node, type_node),
            Kind::PropertyAssignment => self
                .store
                .set_generated_property_assignment_type_node(node, type_node),
            Kind::ShorthandPropertyAssignment => self
                .store
                .set_generated_shorthand_property_assignment_type_node(node, type_node),
            Kind::ExportAssignment => self
                .store
                .set_generated_export_assignment_type_node(node, type_node),
            Kind::BinaryExpression => self
                .store
                .set_generated_binary_expression_type_node(node, type_node),
            Kind::TypeAliasDeclaration | Kind::JSTypeAliasDeclaration => self
                .store
                .set_generated_type_alias_declaration_type_node(node, type_node),
            Kind::GetAccessor => self
                .store
                .set_generated_get_accessor_declaration_type_node(node, type_node),
            Kind::SetAccessor => self
                .store
                .set_generated_set_accessor_declaration_type_node(node, type_node),
            Kind::MethodDeclaration => self
                .store
                .set_generated_method_declaration_type_node(node, type_node),
            Kind::FunctionDeclaration => self
                .store
                .set_generated_function_declaration_type_node(node, type_node),
            Kind::FunctionExpression => self
                .store
                .set_generated_function_expression_type_node(node, type_node),
            Kind::ArrowFunction => self
                .store
                .set_generated_arrow_function_type_node(node, type_node),
            Kind::Constructor => self
                .store
                .set_generated_constructor_declaration_type_node(node, type_node),
            Kind::FunctionType => self
                .store
                .set_generated_function_type_node_type_node(node, type_node),
            Kind::ConstructorType => self
                .store
                .set_generated_constructor_type_node_type_node(node, type_node),
            _ => panic!(
                "Unhandled case in NodeFactory::set_reparsed_type_node: {}",
                self.kind(node)
            ),
        }
    }

    pub fn set_reparsed_expression_node(&mut self, node: Node, expression: Option<Node>) {
        match self.kind(node) {
            Kind::ReturnStatement => self
                .store
                .set_generated_return_statement_expression(node, expression),
            Kind::ParenthesizedExpression => self
                .store
                .set_generated_parenthesized_expression_expression(node, expression),
            Kind::ExportAssignment => self
                .store
                .set_generated_export_assignment_expression(node, expression),
            Kind::ExpressionWithTypeArguments => self
                .store
                .set_generated_expression_with_type_arguments_expression(node, expression),
            _ => panic!(
                "Unhandled case in NodeFactory::set_reparsed_expression_node: {}",
                self.kind(node)
            ),
        }
    }

    pub fn set_reparsed_initializer(&mut self, node: Node, initializer: Option<Node>) {
        match self.kind(node) {
            Kind::VariableDeclaration => self
                .store
                .set_generated_variable_declaration_initializer(node, initializer),
            Kind::Parameter => self
                .store
                .set_generated_parameter_declaration_initializer(node, initializer),
            Kind::PropertyDeclaration => self
                .store
                .set_generated_property_declaration_initializer(node, initializer),
            Kind::PropertyAssignment => self
                .store
                .set_generated_property_assignment_initializer(node, initializer),
            _ => panic!(
                "Unhandled case in NodeFactory::set_reparsed_initializer: {}",
                self.kind(node)
            ),
        }
    }

    pub fn set_reparsed_modifiers(&mut self, node: Node, modifiers: Option<ModifierList>) {
        match self.kind(node) {
            Kind::PropertyDeclaration => self
                .store
                .set_generated_property_declaration_modifiers(node, modifiers),
            Kind::MethodDeclaration => self
                .store
                .set_generated_method_declaration_modifiers(node, modifiers),
            Kind::GetAccessor => self
                .store
                .set_generated_get_accessor_declaration_modifiers(node, modifiers),
            Kind::SetAccessor => self
                .store
                .set_generated_set_accessor_declaration_modifiers(node, modifiers),
            Kind::BinaryExpression => self
                .store
                .set_generated_binary_expression_modifiers(node, modifiers),
            _ => panic!(
                "Unhandled case in NodeFactory::set_reparsed_modifiers: {}",
                self.kind(node)
            ),
        }
    }

    pub fn set_reparsed_function_like_type_parameters(
        &mut self,
        node: Node,
        type_parameters: Option<NodeList>,
    ) {
        match self.kind(node) {
            Kind::FunctionDeclaration => self
                .store
                .set_generated_function_declaration_type_parameters(node, type_parameters),
            Kind::Constructor => self
                .store
                .set_generated_constructor_declaration_type_parameters(node, type_parameters),
            Kind::GetAccessor => self
                .store
                .set_generated_get_accessor_declaration_type_parameters(node, type_parameters),
            Kind::SetAccessor => self
                .store
                .set_generated_set_accessor_declaration_type_parameters(node, type_parameters),
            Kind::MethodDeclaration => self
                .store
                .set_generated_method_declaration_type_parameters(node, type_parameters),
            Kind::ArrowFunction => self
                .store
                .set_generated_arrow_function_type_parameters(node, type_parameters),
            Kind::FunctionExpression => self
                .store
                .set_generated_function_expression_type_parameters(node, type_parameters),
            Kind::CallSignature => self
                .store
                .set_generated_call_signature_declaration_type_parameters(node, type_parameters),
            Kind::ConstructSignature => self
                .store
                .set_generated_construct_signature_declaration_type_parameters(
                    node,
                    type_parameters,
                ),
            Kind::IndexSignature => self
                .store
                .set_generated_index_signature_declaration_type_parameters(node, type_parameters),
            Kind::MethodSignature => self
                .store
                .set_generated_method_signature_declaration_type_parameters(node, type_parameters),
            Kind::FunctionType => self
                .store
                .set_generated_function_type_node_type_parameters(node, type_parameters),
            Kind::ConstructorType => self
                .store
                .set_generated_constructor_type_node_type_parameters(node, type_parameters),
            _ => panic!(
                "Unhandled case in NodeFactory::set_reparsed_function_like_type_parameters: {}",
                self.kind(node)
            ),
        }
    }

    pub fn set_reparsed_function_like_parameters(&mut self, node: Node, parameters: NodeList) {
        match self.kind(node) {
            Kind::FunctionDeclaration => self
                .store
                .set_generated_function_declaration_parameters(node, parameters),
            Kind::Constructor => self
                .store
                .set_generated_constructor_declaration_parameters(node, parameters),
            Kind::GetAccessor => self
                .store
                .set_generated_get_accessor_declaration_parameters(node, parameters),
            Kind::SetAccessor => self
                .store
                .set_generated_set_accessor_declaration_parameters(node, parameters),
            Kind::MethodDeclaration => self
                .store
                .set_generated_method_declaration_parameters(node, parameters),
            Kind::ArrowFunction => self
                .store
                .set_generated_arrow_function_parameters(node, parameters),
            Kind::FunctionExpression => self
                .store
                .set_generated_function_expression_parameters(node, parameters),
            Kind::CallSignature => self
                .store
                .set_generated_call_signature_declaration_parameters(node, parameters),
            Kind::ConstructSignature => self
                .store
                .set_generated_construct_signature_declaration_parameters(node, parameters),
            Kind::IndexSignature => self
                .store
                .set_generated_index_signature_declaration_parameters(node, parameters),
            Kind::MethodSignature => self
                .store
                .set_generated_method_signature_declaration_parameters(node, parameters),
            Kind::FunctionType => self
                .store
                .set_generated_function_type_node_parameters(node, parameters),
            Kind::ConstructorType => self
                .store
                .set_generated_constructor_type_node_parameters(node, parameters),
            _ => panic!(
                "Unhandled case in NodeFactory::set_reparsed_function_like_parameters: {}",
                self.kind(node)
            ),
        }
    }

    pub fn set_reparsed_function_like_type(&mut self, node: Node, type_node: Option<Node>) {
        self.set_reparsed_type_node(node, type_node);
    }

    pub fn set_reparsed_function_like_full_signature(
        &mut self,
        node: Node,
        full_signature: Option<Node>,
    ) {
        match self.kind(node) {
            Kind::FunctionDeclaration => self
                .store
                .set_generated_function_declaration_full_signature(node, full_signature),
            Kind::Constructor => self
                .store
                .set_generated_constructor_declaration_full_signature(node, full_signature),
            Kind::GetAccessor => self
                .store
                .set_generated_get_accessor_declaration_full_signature(node, full_signature),
            Kind::SetAccessor => self
                .store
                .set_generated_set_accessor_declaration_full_signature(node, full_signature),
            Kind::MethodDeclaration => self
                .store
                .set_generated_method_declaration_full_signature(node, full_signature),
            Kind::ArrowFunction => self
                .store
                .set_generated_arrow_function_full_signature(node, full_signature),
            Kind::FunctionExpression => self
                .store
                .set_generated_function_expression_full_signature(node, full_signature),
            Kind::CallSignature => self
                .store
                .set_generated_call_signature_declaration_full_signature(node, full_signature),
            Kind::ConstructSignature => self
                .store
                .set_generated_construct_signature_declaration_full_signature(node, full_signature),
            Kind::IndexSignature => self
                .store
                .set_generated_index_signature_declaration_full_signature(node, full_signature),
            Kind::MethodSignature => self
                .store
                .set_generated_method_signature_declaration_full_signature(node, full_signature),
            Kind::FunctionType => self
                .store
                .set_generated_function_type_node_full_signature(node, full_signature),
            Kind::ConstructorType => self
                .store
                .set_generated_constructor_type_node_full_signature(node, full_signature),
            _ => panic!(
                "Unhandled case in NodeFactory::set_reparsed_function_like_full_signature: {}",
                self.kind(node)
            ),
        }
    }
}
