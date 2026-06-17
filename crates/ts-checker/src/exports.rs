// package checker

use ts_ast as ast;
use ts_collections::FastHashMap as HashMap;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_evaluator as evaluator;
use ts_modulespecifiers as modulespecifiers;

use crate::checker::*;
use crate::nodebuilder;

fn to_checker_symbol(symbol: ast::SymbolIdentity) -> SymbolIdentity {
    SymbolIdentity::from(symbol)
}

fn to_ast_symbol(symbol: SymbolIdentity) -> ast::SymbolIdentity {
    symbol.into()
}

fn to_ast_symbols(symbols: Vec<SymbolIdentity>) -> Vec<ast::SymbolIdentity> {
    symbols.into_iter().map(to_ast_symbol).collect()
}

impl<'a, 'state> Checker<'a, 'state> {
    pub fn get_string_type(&self) -> TypeHandle {
        self.semantic_state.semantic_handles().string_type
    }

    pub fn get_number_type(&self) -> TypeHandle {
        self.semantic_state.semantic_handles().number_type
    }

    pub fn get_boolean_type(&self) -> TypeHandle {
        self.semantic_state.semantic_handles().boolean_type
    }

    pub fn get_void_type(&self) -> TypeHandle {
        self.semantic_state.semantic_handles().void_type
    }

    pub fn get_undefined_type(&self) -> TypeHandle {
        self.semantic_state.semantic_handles().undefined_type
    }

    pub fn get_null_type(&self) -> TypeHandle {
        self.semantic_state.semantic_handles().null_type
    }

    pub fn get_any_type(&self) -> TypeHandle {
        self.semantic_state.semantic_handles().any_type
    }

    pub fn get_error_type(&self) -> TypeHandle {
        self.semantic_state.semantic_handles().error_type
    }

    pub fn get_never_type(&self) -> TypeHandle {
        self.semantic_state.semantic_handles().never_type
    }

    pub fn get_unknown_type(&self) -> TypeHandle {
        self.semantic_state.semantic_handles().unknown_type
    }

    pub fn get_big_int_type(&self) -> TypeHandle {
        self.semantic_state.semantic_handles().bigint_type
    }

    pub fn get_es_symbol_type(&self) -> TypeHandle {
        self.semantic_state.semantic_handles().es_symbol_type
    }

    pub fn get_base_type_of_literal_type_public(&mut self, t: TypeHandle) -> TypeHandle {
        self.get_base_type_of_literal_type(t)
    }

    pub fn get_unknown_symbol(&self) -> ast::SymbolIdentity {
        to_ast_symbol(self.unknown_symbol_identity())
    }

    pub fn get_union_type_public(&mut self, types: Vec<TypeHandle>) -> TypeHandle {
        self.get_union_type(types)
    }

    pub fn get_promised_type_of_promise_public(&mut self, t: TypeHandle) -> Option<TypeHandle> {
        self.get_promised_type_of_promise(t)
    }

    pub(crate) fn get_name_type_of_symbol(&mut self, symbol: SymbolIdentity) -> Option<TypeHandle> {
        if !self.semantic_state.has_value_symbol_link(symbol) {
            return None;
        }
        self.semantic_state.try_value_symbol_name_type(symbol)
    }

    pub fn get_name_type_of_symbol_identity_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Option<TypeHandle> {
        self.get_name_type_of_symbol(to_checker_symbol(symbol))
    }
}

pub fn is_type_usable_as_property_name_public<'a>(
    checker: &Checker<'a, '_>,
    t: TypeHandle,
) -> bool {
    is_type_usable_as_property_name(checker, t)
}

pub(crate) fn is_type_usable_as_property_name<'a>(
    checker: &Checker<'a, '_>,
    t: TypeHandle,
) -> bool {
    crate::utilities::is_type_usable_as_property_name(checker, t)
}

pub fn get_property_name_from_type_public<'a>(checker: &Checker<'a, '_>, t: TypeHandle) -> String {
    get_property_name_from_type(checker, t)
}

pub(crate) fn get_property_name_from_type<'a>(checker: &Checker<'a, '_>, t: TypeHandle) -> String {
    crate::utilities::get_property_name_from_type(checker, t)
}

impl<'a, 'state> Checker<'a, 'state> {
    pub fn get_global_symbol_public(
        &mut self,
        name: &str,
        meaning: ast::SymbolFlags,
        diagnostic: Option<&'static diagnostics::Message>,
    ) -> Option<ast::SymbolIdentity> {
        self.get_global_symbol(name, meaning, diagnostic)
            .map(to_ast_symbol)
    }

    pub fn get_merged_symbol_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Option<ast::SymbolIdentity> {
        self.get_merged_symbol_identity(Some(to_checker_symbol(symbol)))
            .map(to_ast_symbol)
    }

    pub fn try_find_ambient_module_public(
        &mut self,
        module_name: &str,
    ) -> Option<ast::SymbolIdentity> {
        self.try_find_ambient_module(module_name, true /* withAugmentations */)
            .map(to_ast_symbol)
    }

    pub fn get_immediate_aliased_symbol_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Option<ast::SymbolIdentity> {
        self.immediate_alias_target_symbol(to_checker_symbol(symbol))
            .map(to_ast_symbol)
    }

    pub fn skip_alias_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Option<ast::SymbolIdentity> {
        let symbol = to_checker_symbol(symbol);
        if self
            .symbol_identity_flags(symbol)
            .intersects(ast::SYMBOL_FLAGS_ALIAS)
        {
            Some(to_ast_symbol(self.resolve_alias_identity(symbol)))
        } else {
            Some(to_ast_symbol(symbol))
        }
    }

    pub fn get_type_only_alias_declaration_for_symbol_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Option<ast::Node> {
        self.get_type_only_alias_declaration_identity(to_checker_symbol(symbol))
    }

    pub fn resolve_external_module_name_public(
        &mut self,
        module_specifier: ast::Node,
    ) -> Option<ast::SymbolIdentity> {
        self.resolve_external_module_name_worker(
            module_specifier,
            module_specifier,
            None,
            true,  /*ignoreErrors*/
            false, /*isForAugmentation*/
        )
        .map(to_ast_symbol)
    }

    pub fn resolve_external_module_symbol_public(
        &mut self,
        module_symbol: ast::SymbolIdentity,
    ) -> Option<ast::SymbolIdentity> {
        self.resolve_external_module_symbol_identity(
            to_checker_symbol(module_symbol),
            false, /*dontResolveAlias*/
        )
        .map(to_ast_symbol)
    }

    pub fn get_type_from_type_node_public(&mut self, node: ast::Node) -> TypeHandle {
        self.get_type_from_type_node(node)
    }

    pub fn try_get_this_type_at_ex_public(
        &mut self,
        node: ast::Node,
        include_global_this: bool,
        container: Option<ast::Node>,
    ) -> Option<TypeHandle> {
        let node = node;
        let container = container;
        self.try_get_this_type_at_ex(node, include_global_this, container)
    }

    pub fn is_array_like_type_public(&mut self, t: TypeHandle) -> bool {
        self.is_array_like_type(t)
    }

    pub fn get_properties_of_type_public(&mut self, t: TypeHandle) -> Vec<ast::SymbolIdentity> {
        to_ast_symbols(self.get_property_identities_of_type_for_services(t))
    }

    pub fn get_property_symbol_identities_of_type_public(
        &mut self,
        t: TypeHandle,
    ) -> Vec<ast::SymbolIdentity> {
        self.get_properties_of_type_public(t)
    }

    pub fn get_property_of_type_public(
        &mut self,
        t: TypeHandle,
        name: &str,
    ) -> Option<ast::SymbolIdentity> {
        self.get_property_identities_of_type_for_services(t)
            .into_iter()
            .find(|symbol| self.symbol_identity_name(*symbol).as_str() == name)
            .map(to_ast_symbol)
    }

    pub fn type_has_call_or_construct_signatures_public(&mut self, t: TypeHandle) -> bool {
        self.type_has_call_or_construct_signatures(t)
    }

    // Checks if a property can be accessed in a location.
    // The location is given by the `node` parameter.
    // The node does not need to be a property access.
    // @param node location where to check property accessibility
    // @param isSuper whether to consider this a `super` property access, e.g. `super.foo`.
    // @param isWrite whether this is a write access, e.g. `++foo.x`.
    // @param containingType type where the property comes from.
    // @param property property symbol.
    pub fn is_property_accessible_public(
        &mut self,
        node: ast::Node,
        is_super: bool,
        is_write: bool,
        containing_type: TypeHandle,
        property: ast::SymbolIdentity,
    ) -> bool {
        self.is_property_accessible_identity(
            node,
            is_super,
            is_write,
            containing_type,
            to_checker_symbol(property),
        )
    }

    pub fn get_type_of_property_of_contextual_type_public(
        &mut self,
        t: TypeHandle,
        name: &str,
    ) -> Option<TypeHandle> {
        self.get_type_of_property_of_contextual_type(t, name)
    }
}

impl<'a, 'state> Checker<'a, 'state> {
    pub fn was_canceled(&self) -> bool {
        self.semantic_state.was_canceled()
    }

    pub fn get_signatures_of_type_public(
        &mut self,
        t: TypeHandle,
        kind: SignatureKind,
    ) -> Vec<SignatureHandle> {
        self.get_signatures_of_type(t, kind)
    }

    pub fn get_constraint_of_type_parameter_public(
        &mut self,
        type_parameter: TypeHandle,
    ) -> Option<TypeHandle> {
        self.get_constraint_of_type_parameter(type_parameter)
    }

    pub fn get_default_from_type_parameter_public(
        &mut self,
        type_parameter: TypeHandle,
    ) -> Option<TypeHandle> {
        self.get_default_from_type_parameter(type_parameter)
    }

    pub fn get_resolution_mode_override_public(
        &mut self,
        node: ast::Node,
        report_errors: bool,
    ) -> core::ResolutionMode {
        self.get_resolution_mode_override(node, report_errors)
    }

    pub fn get_effective_declaration_flags_public(
        &mut self,
        n: ast::Node,
        flags_to_check: ast::ModifierFlags,
    ) -> ast::ModifierFlags {
        self.get_effective_declaration_flags(n, flags_to_check)
    }

    pub fn get_base_constraint_of_type_public(&mut self, t: TypeHandle) -> Option<TypeHandle> {
        self.get_base_constraint_of_type(t)
    }

    pub fn get_non_nullable_type_public(&mut self, t: TypeHandle) -> TypeHandle {
        self.get_non_nullable_type(t)
    }

    pub fn is_nullable_type_public(&mut self, t: TypeHandle) -> bool {
        self.is_nullable_type(t)
    }

    pub fn get_type_predicate_of_signature_public(
        &mut self,
        sig: SignatureHandle,
    ) -> Option<TypePredicateHandle> {
        self.get_type_predicate_of_signature(sig)
    }
}

pub fn try_get_module_specifier_from_declaration<'a>(
    store: &'a ast::AstStore,
    node: Option<ast::Node>,
) -> Option<ast::Node> {
    crate::nodebuilderimpl::try_get_module_specifier_from_declaration(store, node)
}

impl<'a, 'state> Checker<'a, 'state> {
    pub fn type_flags_public(&self, t: TypeHandle) -> TypeFlags {
        self.type_flags(t)
    }

    pub fn object_flags_public(&self, t: TypeHandle) -> ObjectFlags {
        self.object_flags(t)
    }

    pub fn type_symbol_public(&self, t: TypeHandle) -> Option<ast::SymbolIdentity> {
        self.type_symbol_identity(t).map(to_ast_symbol)
    }

    pub fn type_symbol_identity_public(&self, t: TypeHandle) -> Option<ast::SymbolIdentity> {
        self.type_symbol_public(t)
    }

    pub fn type_types_public(&self, t: TypeHandle) -> Vec<TypeHandle> {
        self.type_types(t)
    }

    pub fn distributed_types_public(&self, t: TypeHandle) -> Vec<TypeHandle> {
        self.distributed_types(t)
    }

    pub fn type_target_public(&self, t: TypeHandle) -> TypeHandle {
        self.type_target(t)
    }

    pub fn object_target_public(&self, t: TypeHandle) -> Option<TypeHandle> {
        self.type_record(t)
            .as_object_type()
            .and_then(|object| object.target)
    }

    pub fn structured_properties_public(&self, t: TypeHandle) -> Vec<ast::SymbolIdentity> {
        if self.type_record(t).data.as_structured_type().is_none() {
            return Vec::new();
        }
        to_ast_symbols(self.collect_structured_type_properties(t))
    }

    pub fn interface_type_parameters_public(&self, t: TypeHandle) -> Vec<TypeHandle> {
        self.interface_type_parameters(t)
    }

    pub fn interface_outer_type_parameters_public(&self, t: TypeHandle) -> Vec<TypeHandle> {
        self.interface_outer_type_parameters(t)
    }

    pub fn interface_local_type_parameters_public(&self, t: TypeHandle) -> Vec<TypeHandle> {
        self.interface_local_type_parameters(t)
    }

    pub fn is_interface_type_public(&self, t: TypeHandle) -> bool {
        self.type_record(t).as_interface_type().is_some()
    }

    pub fn is_string_type_public(&self, t: TypeHandle) -> bool {
        self.type_flags(t) & TYPE_FLAGS_STRING != 0
    }

    pub fn is_string_like_type_public(&self, t: TypeHandle) -> bool {
        self.type_flags(t) & TYPE_FLAGS_STRING_LIKE != 0
    }

    pub fn is_boolean_like_type_public(&self, t: TypeHandle) -> bool {
        self.type_flags(t) & TYPE_FLAGS_BOOLEAN_LIKE != 0
    }

    pub fn is_string_literal_type_public(&self, t: TypeHandle) -> bool {
        self.type_flags(t) & TYPE_FLAGS_STRING_LITERAL != 0
    }

    pub fn is_number_literal_type_public(&self, t: TypeHandle) -> bool {
        self.type_flags(t) & TYPE_FLAGS_NUMBER_LITERAL != 0
    }

    pub fn is_big_int_literal_type_public(&self, t: TypeHandle) -> bool {
        self.type_flags(t) & TYPE_FLAGS_BIG_INT_LITERAL != 0
    }

    pub fn is_enum_literal_type_public(&self, t: TypeHandle) -> bool {
        self.type_flags(t) & TYPE_FLAGS_ENUM_LITERAL != 0
    }

    pub fn is_union_type_public(&self, t: TypeHandle) -> bool {
        self.type_flags(t) & TYPE_FLAGS_UNION != 0
    }

    pub fn is_intersection_type_public(&self, t: TypeHandle) -> bool {
        self.type_flags(t) & TYPE_FLAGS_INTERSECTION != 0
    }

    pub fn is_type_parameter_public(&self, t: TypeHandle) -> bool {
        self.type_flags(t) & TYPE_FLAGS_TYPE_PARAMETER != 0
    }

    pub fn is_class_type_public(&self, t: TypeHandle) -> bool {
        self.object_flags(t) & OBJECT_FLAGS_CLASS != 0
    }

    pub fn literal_value_public(&self, t: TypeHandle) -> LiteralValue {
        self.type_record(t).as_literal_type().value.clone()
    }

    pub fn intrinsic_type_name_public(&self, t: TypeHandle) -> Option<String> {
        if self.type_flags(t) & TYPE_FLAGS_INTRINSIC == 0 {
            return None;
        }
        Some(
            self.type_record(t)
                .as_intrinsic_type()
                .intrinsic_name
                .clone(),
        )
    }

    pub fn is_tuple_type_public(&self, t: TypeHandle) -> bool {
        self.is_tuple_type(t)
    }

    pub fn get_string_literal_value_public(&self, t: TypeHandle) -> String {
        self.get_string_literal_value(t)
    }

    pub fn tuple_element_flags_public(&self, t: TypeHandle) -> Vec<ElementFlags> {
        self.target_tuple_type_record(t)
            .element_infos
            .iter()
            .map(|info| info.flags)
            .collect()
    }

    pub fn tuple_labeled_declarations_public(&self, t: TypeHandle) -> Vec<Option<ast::Node>> {
        self.target_tuple_type_record(t)
            .element_infos
            .iter()
            .map(|info| info.labeled_declaration())
            .collect()
    }

    pub fn tuple_fixed_length_public(&self, t: TypeHandle) -> usize {
        self.target_tuple_type_record(t).fixed_length
    }

    pub fn tuple_readonly_public(&self, t: TypeHandle) -> bool {
        self.target_tuple_type_record(t).readonly
    }

    pub fn index_type_target_public(&self, t: TypeHandle) -> Option<TypeHandle> {
        self.type_record(t).as_index_type().target
    }

    pub fn indexed_access_object_type_public(&self, t: TypeHandle) -> Option<TypeHandle> {
        self.type_record(t).as_indexed_access_type().object_type
    }

    pub fn indexed_access_index_type_public(&self, t: TypeHandle) -> Option<TypeHandle> {
        self.type_record(t).as_indexed_access_type().index_type
    }

    pub fn conditional_check_type_public(&self, t: TypeHandle) -> Option<TypeHandle> {
        self.type_record(t).as_conditional_type().check_type
    }

    pub fn conditional_extends_type_public(&self, t: TypeHandle) -> Option<TypeHandle> {
        self.type_record(t).as_conditional_type().extends_type
    }

    pub fn substitution_base_type_public(&self, t: TypeHandle) -> Option<TypeHandle> {
        self.type_record(t).as_substitution_type().base_type
    }

    pub fn substitution_constraint_public(&self, t: TypeHandle) -> Option<TypeHandle> {
        self.type_record(t).as_substitution_type().constraint
    }

    pub fn template_literal_texts_public(&self, t: TypeHandle) -> Vec<String> {
        self.type_record(t)
            .as_template_literal_type()
            .texts
            .to_vec()
    }

    pub fn string_mapping_target_public(&self, t: TypeHandle) -> Option<TypeHandle> {
        self.type_record(t).as_string_mapping_type().target
    }

    pub fn get_return_type_of_signature_public(&mut self, sig: SignatureHandle) -> TypeHandle {
        self.get_return_type_of_signature(sig)
    }

    pub fn signature_flags_public(&self, sig: SignatureHandle) -> u32 {
        self.signature_record(sig).flags as u32
    }

    pub fn signature_declaration_public(&self, sig: SignatureHandle) -> Option<ast::Node> {
        self.signature_record(sig).declaration
    }

    pub fn signature_type_parameters_public(&self, sig: SignatureHandle) -> Vec<TypeHandle> {
        self.signature_record(sig).type_parameters.clone()
    }

    pub fn signature_parameters_public(&self, sig: SignatureHandle) -> Vec<ast::SymbolIdentity> {
        to_ast_symbols(self.signature_parameter_identities(sig))
    }

    pub fn signature_parameter_symbol_identities_public(
        &mut self,
        sig: SignatureHandle,
    ) -> Vec<ast::SymbolIdentity> {
        to_ast_symbols(self.signature_parameter_identities(sig))
    }

    pub fn signature_min_argument_count_public(&self, sig: SignatureHandle) -> i32 {
        self.signature_record(sig).min_argument_count
    }

    pub fn signature_this_parameter_public(
        &self,
        sig: SignatureHandle,
    ) -> Option<ast::SymbolIdentity> {
        self.signature_this_parameter_identity(sig)
            .map(to_ast_symbol)
    }

    pub fn signature_this_parameter_symbol_identity_public(
        &mut self,
        sig: SignatureHandle,
    ) -> Option<ast::SymbolIdentity> {
        self.signature_this_parameter_public(sig)
    }

    pub fn signature_target_public(&self, sig: SignatureHandle) -> Option<SignatureHandle> {
        self.signature_record(sig).target
    }

    pub fn signature_has_rest_parameter_public(&self, sig: SignatureHandle) -> bool {
        self.signature_has_rest_parameter(sig)
    }

    pub fn type_predicate_type_public(&self, predicate: TypePredicateHandle) -> Option<TypeHandle> {
        self.type_predicate_record(predicate).t
    }

    pub fn type_predicate_kind_public(&self, predicate: TypePredicateHandle) -> i32 {
        self.type_predicate_record(predicate).kind as i32
    }

    pub fn type_predicate_parameter_index_public(&self, predicate: TypePredicateHandle) -> i32 {
        self.type_predicate_record(predicate).parameter_index
    }

    pub fn type_predicate_parameter_name_public(&self, predicate: TypePredicateHandle) -> String {
        self.type_predicate_record(predicate).parameter_name.clone()
    }

    pub fn index_info_key_type_public(&self, index_info: IndexInfoHandle) -> Option<TypeHandle> {
        self.index_info_record(index_info).key_type
    }

    pub fn index_info_value_type_public(&self, index_info: IndexInfoHandle) -> Option<TypeHandle> {
        self.index_info_record(index_info).value_type
    }

    pub fn index_info_is_readonly_public(&self, index_info: IndexInfoHandle) -> bool {
        self.index_info_record(index_info).is_readonly
    }

    pub fn has_effective_rest_parameter_public(&mut self, signature: SignatureHandle) -> bool {
        self.has_effective_rest_parameter(signature)
    }

    pub fn get_local_type_parameters_of_class_or_interface_or_type_alias_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Vec<TypeHandle> {
        self.get_local_type_parameters_of_symbol_identity_public(symbol)
    }

    pub fn get_local_type_parameters_of_symbol_identity_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Vec<TypeHandle> {
        let symbol = symbol.symbol_handle();
        self.get_local_type_parameters_of_class_or_interface_or_type_alias_handle(symbol)
    }

    pub fn get_contextual_type_for_object_literal_element_public(
        &mut self,
        element: ast::Node,
        context_flags: ContextFlags,
    ) -> Option<TypeHandle> {
        let element = element;
        self.get_contextual_type_for_object_literal_element(element, context_flags)
    }

    pub fn type_predicate_to_string_public(&mut self, t: TypePredicateHandle) -> String {
        self.type_predicate_to_string(t)
    }

    pub fn get_expanded_parameters_public(
        &mut self,
        signature: SignatureHandle,
        skip_union_expanding: bool,
    ) -> Vec<Vec<ast::SymbolIdentity>> {
        self.get_expanded_parameters(signature, skip_union_expanding)
            .into_iter()
            .map(to_ast_symbols)
            .collect()
    }

    pub fn symbol_to_parameter_declaration_public(
        &mut self,
        symbol: ast::SymbolIdentity,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
    ) -> Option<ast::Node> {
        let symbol = to_checker_symbol(symbol);
        let (mut emit_context, done) = ts_printer::get_emit_context();
        let result = {
            let mut builder =
                crate::nodebuilder::new_node_builder_ex(self, &mut emit_context, None);
            builder.symbol_to_parameter_declaration(
                symbol,
                enclosing_declaration,
                flags,
                internal_flags,
                None,
            )
        };
        done(emit_context);
        result
    }

    pub fn symbol_identity_to_parameter_declaration_public(
        &mut self,
        symbol: ast::SymbolIdentity,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
    ) -> Option<ast::Node> {
        self.symbol_to_parameter_declaration_public(
            symbol,
            enclosing_declaration,
            flags,
            internal_flags,
        )
    }

    pub fn type_to_type_node_for_ls_public(
        &mut self,
        emit_context: &mut ts_printer::EmitContext,
        t: TypeHandle,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
    ) -> (Option<ast::Node>, HashMap<ast::Node, ast::SymbolIdentity>) {
        let enclosing_declaration = enclosing_declaration.map(|node| node);
        let mut node_builder = crate::nodebuilder::new_node_builder_ex(self, emit_context, None);
        let node =
            node_builder.type_to_type_node(t, enclosing_declaration, flags, internal_flags, None);
        let id_to_symbol = node_builder
            .id_to_symbol_identities()
            .into_iter()
            .map(|(node, symbol)| (node, to_ast_symbol(symbol)))
            .collect();
        drop(node_builder);
        (node, id_to_symbol)
    }

    pub fn type_predicate_to_type_predicate_node_for_ls_public(
        &mut self,
        emit_context: &mut ts_printer::EmitContext,
        type_predicate: TypePredicateHandle,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
    ) -> Option<ast::Node> {
        let enclosing_declaration = enclosing_declaration.map(|node| node);
        let mut node_builder = crate::nodebuilder::new_node_builder_ex(self, emit_context, None);
        let node = node_builder.type_predicate_to_type_predicate_node(
            type_predicate,
            enclosing_declaration,
            flags,
            nodebuilder::INTERNAL_FLAGS_NONE,
            None,
        );
        drop(node_builder);
        node
    }

    pub fn signature_to_signature_declaration_for_ls_public(
        &mut self,
        emit_context: &mut ts_printer::EmitContext,
        signature: SignatureHandle,
        kind: ast::Kind,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
    ) -> (Option<ast::Node>, HashMap<ast::Node, ast::SymbolIdentity>) {
        let enclosing_declaration = enclosing_declaration.map(|node| node);
        let mut node_builder = crate::nodebuilder::new_node_builder_ex(self, emit_context, None);
        let node = node_builder.signature_to_signature_declaration(
            signature,
            kind,
            enclosing_declaration,
            flags,
            internal_flags,
            None,
        );
        let id_to_symbol = node_builder
            .id_to_symbol_identities()
            .into_iter()
            .map(|(node, symbol)| (node, to_ast_symbol(symbol)))
            .collect();
        drop(node_builder);
        (node, id_to_symbol)
    }

    pub fn index_info_to_index_signature_declaration_for_ls_public(
        &mut self,
        emit_context: &mut ts_printer::EmitContext,
        index_info: IndexInfoHandle,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
    ) -> Option<ast::Node> {
        let enclosing_declaration = enclosing_declaration.map(|node| node);
        let mut builder = crate::nodebuilder::new_node_builder(self, emit_context);
        builder.index_info_to_index_signature_declaration(
            index_info,
            enclosing_declaration,
            flags,
            internal_flags,
            None,
        )
    }

    pub fn type_parameter_to_declaration_public(
        &mut self,
        ty: TypeHandle,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
    ) -> Option<ast::Node> {
        let (mut emit_context, done) = ts_printer::get_emit_context();
        let result = {
            let mut builder = crate::nodebuilder::new_node_builder(self, &mut emit_context);
            builder.type_parameter_to_declaration(
                ty,
                enclosing_declaration,
                flags,
                internal_flags,
                None,
            )
        };
        done(emit_context);
        result
    }

    pub fn get_resolved_signature_public(&mut self, node: ast::Node) -> Option<SignatureHandle> {
        Some(self.get_resolved_signature(node, None, CHECK_MODE_NORMAL))
    }

    // Return the type of the given property in the given type, or nil if no such property exists
    pub fn get_type_of_property_of_type_public(
        &mut self,
        t: TypeHandle,
        name: &str,
    ) -> Option<TypeHandle> {
        self.get_type_of_property_of_type(t, name)
    }

    pub fn get_contextual_type_for_argument_at_index_public(
        &mut self,
        node: ast::Node,
        arg_index: isize,
    ) -> Option<TypeHandle> {
        let node = node;
        self.get_contextual_type_for_argument_at_index(node, arg_index)
    }

    pub fn get_index_signatures_at_location_public(&mut self, node: ast::Node) -> Vec<ast::Node> {
        self.get_index_signatures_at_location(node)
    }

    pub fn get_resolved_symbol_public(&mut self, node: ast::Node) -> Option<ast::SymbolIdentity> {
        Some(to_ast_symbol(self.get_resolved_symbol(node)))
    }

    pub fn get_jsx_namespace_public(&mut self, location: ast::Node) -> String {
        self.get_jsx_namespace(Some(location)).to_string()
    }

    pub fn get_jsx_fragment_factory(&mut self, location: ast::Node) -> String {
        let entity = self.get_jsx_fragment_factory_entity(Some(location));
        if let Some(entity) = entity {
            let first_identifier =
                ast::get_first_identifier(self.store_for_node(entity), entity).unwrap();
            return self.store_for_node(first_identifier).text(first_identifier);
        }
        String::new()
    }

    pub fn get_enum_member_value_public(&mut self, node: ast::Node) -> evaluator::Value {
        self.get_enum_member_value(node).value
    }

    pub fn resolve_name_public(
        &mut self,
        name: &str,
        location: ast::Node,
        meaning: ast::SymbolFlags,
        exclude_globals: bool,
    ) -> Option<ast::SymbolIdentity> {
        let symbol =
            self.resolve_name(Some(location), name, meaning, None, true, exclude_globals)?;
        Some(ast::SymbolIdentity::from_symbol_handle(symbol))
    }

    pub fn resolve_name_symbol_identity_public(
        &mut self,
        name: &str,
        location: ast::Node,
        meaning: ast::SymbolFlags,
        exclude_globals: bool,
    ) -> Option<ast::SymbolIdentity> {
        self.resolve_name_public(name, location, meaning, exclude_globals)
    }

    pub fn get_type_of_symbol_at_source_file_public(
        &mut self,
        symbol: ast::SymbolIdentity,
        file: &ast::SourceFile,
    ) -> Option<TypeHandle> {
        let file_node = file.as_node();
        Some(self.get_type_of_symbol_at_location(to_checker_symbol(symbol), Some(file_node)))
    }

    pub fn get_type_of_symbol_identity_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Option<TypeHandle> {
        Some(self.get_type_of_symbol_identity(to_checker_symbol(symbol)))
    }

    pub fn get_type_alias_type_parameters_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Vec<TypeHandle> {
        let symbol = to_checker_symbol(symbol);
        if !self
            .symbol_identity_flags(symbol)
            .intersects(ast::SYMBOL_FLAGS_TYPE_ALIAS)
        {
            panic!("Attempted to fetch type alias parameters for non-type-alias symbol");
        }
        self.get_declared_type_of_symbol_identity_or_error(symbol);
        self.semantic_state.type_alias_type_parameters(symbol)
    }

    pub fn get_declared_type_of_symbol_identity_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Option<TypeHandle> {
        Some(self.get_declared_type_of_symbol_identity_or_error(to_checker_symbol(symbol)))
    }

    pub fn get_type_of_symbol_identity_at_location_public(
        &mut self,
        symbol: ast::SymbolIdentity,
        location: Option<ast::Node>,
    ) -> Option<TypeHandle> {
        let symbol = to_checker_symbol(symbol);
        self.symbol_identity_flags(symbol);
        Some(self.get_type_of_symbol_identity_at_location(symbol, location))
    }

    pub fn module_symbol_data_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Option<modulespecifiers::ModuleSymbolData> {
        let symbol = to_checker_symbol(symbol);
        Some(modulespecifiers::ModuleSymbolData::new(
            symbol.ast_identity(),
            self.symbol_identity_name(symbol),
            self.collect_symbol_identity_declarations(symbol),
            self.missing_name_symbol_identity_value_declaration(symbol),
        ))
    }

    pub fn symbol_name_public(&mut self, symbol: ast::SymbolIdentity) -> Option<String> {
        Some(
            self.symbol_identity_name(to_checker_symbol(symbol))
                .to_string(),
        )
    }

    pub fn symbol_id_public(&self, symbol: ast::SymbolIdentity) -> ast::SymbolId {
        self.symbol_handle_id(symbol.symbol_handle())
    }

    pub fn collect_symbol_declarations_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Vec<ast::Node> {
        self.collect_symbol_identity_declarations(to_checker_symbol(symbol))
    }

    pub fn symbol_exports_snapshot_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Vec<(ast::SymbolName, ast::SymbolIdentity)> {
        self.with_symbol_handle_exports(symbol.symbol_handle(), |exports| {
            let Some(exports) = exports else {
                return Vec::new();
            };
            exports
                .collect_identities()
                .into_iter()
                .map(|(name, symbol)| (name, to_ast_symbol(symbol)))
                .collect()
        })
    }

    pub fn symbol_value_declaration_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Option<ast::Node> {
        self.missing_name_symbol_identity_value_declaration(to_checker_symbol(symbol))
    }

    pub fn symbol_flags_public(&mut self, symbol: ast::SymbolIdentity) -> Option<ast::SymbolFlags> {
        Some(self.missing_name_symbol_identity_flags(to_checker_symbol(symbol)))
    }

    pub fn symbol_combined_local_and_export_flags_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Option<ast::SymbolFlags> {
        Some(self.symbol_identity_combined_local_and_export_flags(to_checker_symbol(symbol)))
    }

    pub fn symbol_check_flags_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Option<ast::CheckFlags> {
        Some(self.symbol_identity_check_flags(to_checker_symbol(symbol)))
    }

    pub fn symbol_parent_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Option<ast::SymbolIdentity> {
        let handle = symbol.symbol_handle();
        self.symbol_handle_parent(handle)
            .map(ast::SymbolIdentity::from_symbol_handle)
    }

    pub fn is_external_module_symbol_public(&mut self, symbol: ast::SymbolIdentity) -> bool {
        self.symbol_flags_public(symbol)
            .is_some_and(|flags| flags.intersects(ast::SYMBOL_FLAGS_MODULE))
            && self
                .symbol_name_public(symbol)
                .is_some_and(|name| name.starts_with('"'))
    }

    pub fn symbol_has_member_public(&mut self, symbol: ast::SymbolIdentity, name: &str) -> bool {
        self.symbol_member_public(symbol, name).is_some()
    }

    pub fn symbol_member_public(
        &mut self,
        symbol: ast::SymbolIdentity,
        name: &str,
    ) -> Option<ast::SymbolIdentity> {
        self.lookup_symbol_identity_member(to_checker_symbol(symbol), name)
            .map(to_ast_symbol)
    }

    pub fn symbol_export_public(
        &mut self,
        symbol: ast::SymbolIdentity,
        name: &str,
    ) -> Option<ast::SymbolIdentity> {
        self.lookup_symbol_identity_export(to_checker_symbol(symbol), name)
            .map(to_ast_symbol)
    }

    pub fn symbol_has_export_public(&mut self, symbol: ast::SymbolIdentity, name: &str) -> bool {
        self.symbol_export_public(symbol, name).is_some()
    }

    pub fn symbol_members_snapshot_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Vec<ast::SymbolIdentity> {
        self.collect_symbol_identity_member_table(to_checker_symbol(symbol))
            .map(|members| members.values().copied().collect())
            .map(to_ast_symbols)
            .unwrap_or_default()
    }

    pub fn symbol_export_values_snapshot_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Vec<ast::SymbolIdentity> {
        self.collect_symbol_identity_export_table(to_checker_symbol(symbol))
            .map(|exports| exports.values().copied().collect())
            .map(to_ast_symbols)
            .unwrap_or_default()
    }

    pub fn symbol_export_symbol_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Option<ast::SymbolIdentity> {
        self.symbol_identity_export_symbol(to_checker_symbol(symbol))
            .map(to_ast_symbol)
    }

    pub fn source_node_symbol_public(&mut self, node: ast::Node) -> Option<ast::SymbolIdentity> {
        self.node_symbol(node)
            .map(ast::SymbolIdentity::from_symbol_handle)
    }

    pub fn source_node_declaration_symbol_public(
        &mut self,
        node: ast::Node,
    ) -> Option<ast::SymbolIdentity> {
        self.get_symbol_of_declaration(node)
            .map(ast::SymbolIdentity::from_symbol_handle)
    }

    pub fn source_file_has_global_exports_public(&self, source_file: &ast::SourceFile) -> bool {
        self.source_file_has_global_exports(source_file)
    }

    pub fn source_node_has_global_exports_public(&self, node: ast::Node) -> bool {
        self.try_source_file_for_node_public(node)
            .is_some_and(|source_file| {
                node == source_file.as_node() && self.source_file_has_global_exports(source_file)
            })
    }

    pub fn source_node_local_public(
        &mut self,
        node: ast::Node,
        name: &str,
    ) -> Option<ast::SymbolIdentity> {
        self.lookup_node_local(node, name, ast::SYMBOL_FLAGS_ALL)
            .map(ast::SymbolIdentity::from_symbol_handle)
    }

    pub fn for_each_source_node_local_public(
        &mut self,
        node: ast::Node,
        exported_from: Option<ast::SymbolIdentity>,
        mut cb: impl FnMut(&str, ast::SymbolIdentity, bool),
    ) {
        let exported_from = exported_from.map(|symbol| symbol.symbol_handle());
        let _ = self.with_node_locals(node, |locals| {
            for (name, &symbol) in locals {
                let is_exported = exported_from.is_some_and(|exported_from| {
                    self.lookup_symbol_handle_export(exported_from, name.as_str())
                        .is_some_and(|export| {
                            self.symbol_handle_name(export).as_str() == name.as_str()
                        })
                });
                cb(
                    name.as_str(),
                    ast::SymbolIdentity::from_symbol_handle(symbol),
                    is_exported,
                );
            }
        });
    }

    pub fn source_node_non_alias_resolved_name_public(
        &mut self,
        node: ast::Node,
        name: &str,
        meaning: ast::SymbolFlags,
    ) -> Option<ast::SymbolIdentity> {
        let store = self.try_source_file_for_node_public(node)?.store();
        let local = {
            let mut resolver = self.create_name_resolver(store, Some(node));
            resolver.resolve(Some(node), name, meaning, None, false, false)
        }?;
        let local_flags = self.symbol_handle_flags(local);
        if local_flags & (ast::SYMBOL_FLAGS_ALIAS | ast::SYMBOL_FLAGS_NONE)
            == ast::SYMBOL_FLAGS_ALIAS
            || local_flags & ast::SYMBOL_FLAGS_ALIAS != ast::SYMBOL_FLAGS_NONE
                && local_flags & ast::SYMBOL_FLAGS_ASSIGNMENT != ast::SYMBOL_FLAGS_NONE
        {
            return None;
        }
        Some(ast::SymbolIdentity::from_symbol_handle(local))
    }

    pub fn local_symbol_for_export_default_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Option<ast::SymbolIdentity> {
        let symbol = symbol.symbol_handle();
        let first_declaration = self.first_symbol_handle_declaration(symbol)?;
        let declaration_store = self.store_for_node(first_declaration);
        if !ast::has_syntactic_modifier(
            declaration_store,
            first_declaration,
            ast::ModifierFlags::Default,
        ) {
            return None;
        }
        self.collect_symbol_handle_declarations(symbol)
            .into_iter()
            .find_map(|decl| self.node_local_symbol(decl))
            .map(ast::SymbolIdentity::from_symbol_handle)
    }

    pub fn default_like_export_name_public(&mut self, symbol: ast::SymbolIdentity) -> String {
        let symbol = symbol.symbol_handle();
        for declaration in self.collect_symbol_handle_declarations(symbol) {
            let store = self.store_for_node(declaration);
            if ast::is_export_assignment(store, declaration) {
                if let Some(expression) = store.expression(declaration) {
                    let inner = ast::skip_outer_expressions(store, expression, ast::OEK_ALL);
                    if store.kind(inner) == ast::Kind::Identifier {
                        return store.text(inner);
                    }
                }
                continue;
            }
            if ast::is_export_specifier(store, declaration)
                && self.symbol_handle_flags(symbol) == ast::SYMBOL_FLAGS_ALIAS
                && store.property_name(declaration).is_some()
            {
                let property_name = store.property_name(declaration).unwrap();
                if store.kind(property_name) == ast::Kind::Identifier {
                    return store.text(property_name);
                }
                continue;
            }
            if let Some(name) = ast::get_name_of_declaration(store, Some(declaration)) {
                if store.kind(name) == ast::Kind::Identifier {
                    return store.text(name);
                }
            }
            if let Some(parent) = self.symbol_handle_parent(symbol) {
                let parent_name = self.symbol_handle_name(parent);
                let parent_is_external_module =
                    self.symbol_handle_flags(parent) & ast::SYMBOL_FLAGS_MODULE != 0
                        && parent_name.chars().next() == Some('"');
                if !parent_is_external_module {
                    return parent_name.to_string();
                }
            }
        }
        String::new()
    }

    pub fn get_base_types_public(&mut self, t: TypeHandle) -> Vec<TypeHandle> {
        self.get_base_types(t)
    }

    pub fn get_apparent_type_public(&mut self, t: TypeHandle) -> TypeHandle {
        self.get_apparent_type(t)
    }

    pub fn get_base_constructor_type_of_class_public(&mut self, t: TypeHandle) -> TypeHandle {
        self.get_base_constructor_type_of_class(t)
    }

    pub fn get_rest_type_of_signature_public(&mut self, sig: SignatureHandle) -> TypeHandle {
        self.get_rest_type_of_signature(sig)
    }

    pub fn get_type_arguments_public(&mut self, t: TypeHandle) -> Vec<TypeHandle> {
        self.get_type_arguments(t)
    }

    pub fn get_index_info_of_type_public(
        &mut self,
        t: TypeHandle,
        key_type: TypeHandle,
    ) -> Option<IndexInfoHandle> {
        self.get_index_info_of_type(t, key_type)
    }

    pub fn get_index_infos_of_type_public(&mut self, t: TypeHandle) -> Vec<IndexInfoHandle> {
        self.get_index_infos_of_type(t)
    }

    pub fn is_context_sensitive_public(&mut self, node: ast::Node) -> bool {
        self.is_context_sensitive(node)
    }

    pub fn fill_missing_type_arguments_public(
        &mut self,
        type_arguments: Vec<TypeHandle>,
        type_parameters: Vec<TypeHandle>,
        min_type_argument_count: usize,
        is_javascript_implicit_any: bool,
    ) -> Vec<TypeHandle> {
        self.fill_missing_type_arguments(
            type_arguments,
            &type_parameters,
            min_type_argument_count,
            is_javascript_implicit_any,
        )
    }

    pub fn get_min_type_argument_count_public(
        &mut self,
        type_parameters: Vec<TypeHandle>,
    ) -> usize {
        self.get_min_type_argument_count(&type_parameters)
    }

    pub fn get_widened_literal_type_public(&mut self, t: TypeHandle) -> TypeHandle {
        self.get_widened_literal_type(t)
    }

    pub fn is_type_assignable_to_public(&mut self, source: TypeHandle, target: TypeHandle) -> bool {
        self.is_type_assignable_to(source, target)
    }

    pub fn get_union_type_ex_public(
        &mut self,
        types: Vec<TypeHandle>,
        union_reduction: UnionReduction,
    ) -> TypeHandle {
        self.get_union_type_ex(types, union_reduction, None, None)
    }

    pub fn requires_adding_implicit_undefined_public(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        let mut enclosing_declaration = ast::find_ancestor(store, Some(node), |store, node| {
            ast::is_declaration(store, node)
        });
        if enclosing_declaration.is_none() {
            enclosing_declaration = ast::get_source_file_of_node(store, Some(node));
        }
        let symbol = self.get_symbol_of_declaration(node);
        let Some(symbol) = symbol else {
            return false;
        };
        let source_file_node;
        let enclosing_declaration =
            if let Some(enclosing_declaration) = enclosing_declaration.as_ref() {
                enclosing_declaration
            } else {
                source_file_node = ast::get_source_file_of_node(store, Some(node)).unwrap();
                &source_file_node
            };
        self.requires_adding_implicit_undefined(
            node,
            Some(SymbolIdentity::from_symbol_handle(symbol)),
            Some(*enclosing_declaration),
        )
    }

    pub fn remove_missing_or_undefined_type_public(&mut self, t: TypeHandle) -> TypeHandle {
        self.remove_missing_or_undefined_type(t)
    }

    pub fn get_widened_type_public(&mut self, t: TypeHandle) -> TypeHandle {
        self.get_widened_type(t)
    }

    pub fn get_string_index_type_public(&mut self, t: TypeHandle) -> Option<TypeHandle> {
        self.get_string_index_type(t)
    }

    pub fn get_number_index_type_public(&mut self, t: TypeHandle) -> Option<TypeHandle> {
        self.get_number_index_type(t)
    }

    pub fn get_non_optional_type_public(&mut self, t: TypeHandle) -> TypeHandle {
        self.get_non_optional_type(t)
    }
}
