use crate::checker::*;
use crate::relater::{ContainingMessageChain, RelationKind};
use crate::semantic::{JsxElementLinksStoreExt, ValueSymbolLinksStoreExt};
use crate::{ast, core, diagnostics, jsnum, parser, scanner};

const JSX_FLAGS_NONE: JsxFlags = 0;
const JSX_FLAGS_INTRINSIC_NAMED_ELEMENT: JsxFlags = 1 << 0; // An element from a named property of the JSX.IntrinsicElements interface
const JSX_FLAGS_INTRINSIC_INDEXED_ELEMENT: JsxFlags = 1 << 1; // An element inferred from the string index signature of the JSX.IntrinsicElements interface
const JSX_FLAGS_INTRINSIC_ELEMENT: JsxFlags =
    JSX_FLAGS_INTRINSIC_NAMED_ELEMENT | JSX_FLAGS_INTRINSIC_INDEXED_ELEMENT;
pub(crate) const JSX_NAMES_INTRINSIC_ELEMENTS: &str = JsxNames::INTRINSIC_ELEMENTS;
pub(crate) const JSX_NAMES_INTRINSIC_ATTRIBUTES: &str = JsxNames::INTRINSIC_ATTRIBUTES;
pub(crate) const JSX_NAMES_INTRINSIC_CLASS_ATTRIBUTES: &str = JsxNames::INTRINSIC_CLASS_ATTRIBUTES;

#[repr(i32)]
enum JsxReferenceKind {
    Component,
    Function,
    Mixed,
}

struct JsxNames;

impl JsxNames {
    const JSX: &'static str = "JSX";
    const INTRINSIC_ELEMENTS: &'static str = "IntrinsicElements";
    const ELEMENT_CLASS: &'static str = "ElementClass";
    const ELEMENT_ATTRIBUTES_PROPERTY_NAME_CONTAINER: &'static str = "ElementAttributesProperty";
    const ELEMENT_CHILDREN_ATTRIBUTE_NAME_CONTAINER: &'static str = "ElementChildrenAttribute";
    const ELEMENT: &'static str = "Element";
    const ELEMENT_TYPE: &'static str = "ElementType";
    const INTRINSIC_ATTRIBUTES: &'static str = "IntrinsicAttributes";
    const INTRINSIC_CLASS_ATTRIBUTES: &'static str = "IntrinsicClassAttributes";
    const LIBRARY_MANAGED_ATTRIBUTES: &'static str = "LibraryManagedAttributes";
}

struct ReactNames;

impl ReactNames {
    const FRAGMENT: &'static str = "Fragment";
}

struct JsxElaborationElement<'a> {
    error_node: Option<ast::Node>,
    inner_expression: Option<ast::Node>,
    name_type: Option<TypeHandle>,
    create_diagnostic: Option<Box<dyn Fn(ast::Node) -> ast::Diagnostic + 'a>>, // Optional: creates a custom diagnostic for this element
}

impl<'a, 'state> Checker<'a, 'state> {
    fn source_symbol_name_for_node(
        &self,
        node: ast::Node,
        symbol: ast::SymbolHandle,
    ) -> ast::SymbolName {
        let _ = node;
        self.symbol_handle_name(symbol)
    }

    fn source_symbol_has_member_for_node(
        &self,
        node: ast::Node,
        symbol: ast::SymbolHandle,
        name: &str,
    ) -> bool {
        let _ = node;
        self.lookup_symbol_handle_member(symbol, name).is_some()
    }

    fn jsx_symbol_identity_name(&self, symbol: SymbolIdentity) -> ast::SymbolName {
        self.symbol_identity_name(symbol)
    }

    fn jsx_symbol_identity_value_declaration(&self, symbol: SymbolIdentity) -> Option<ast::Node> {
        self.missing_name_symbol_identity_value_declaration(symbol)
    }

    fn check_spread_prop_overrides_for_jsx_attributes(
        &mut self,
        t: TypeHandle,
        props: &SymbolIdentityTable,
        spread: ast::Node,
    ) {
        for right in self.get_properties_of_type(t) {
            if self.missing_name_symbol_identity_flags(right) & ast::SYMBOL_FLAGS_OPTIONAL != 0
                || self.missing_name_symbol_identity_check_flags(right) & ast::CHECK_FLAGS_PARTIAL
                    != 0
            {
                continue;
            }
            let right_name = self.missing_name_symbol_identity_name(right);
            if let Some(&left) = props.get(right_name.as_str()) {
                let value_declaration = self.jsx_symbol_identity_value_declaration(left);
                let name = self.jsx_symbol_identity_name(left).to_string();
                let mut diagnostic = self.create_error_diagnostic(
                    value_declaration,
                    &diagnostics::X_0_IS_SPECIFIED_MORE_THAN_ONCE_SO_THIS_USAGE_WILL_BE_OVERWRITTEN,
                    vec![DiagnosticArg::from(name)],
                );
                diagnostic.add_related_info(new_diagnostic_for_node(
                    self.store_for_node(spread),
                    Some(spread),
                    &diagnostics::THIS_SPREAD_ALWAYS_OVERWRITES_THIS_PROPERTY,
                    Vec::<DiagnosticArg>::new(),
                ));
                self.add_error_diagnostic(diagnostic);
            }
        }
    }

    pub(crate) fn check_jsx_element(
        &mut self,
        node: ast::Node,
        _check_mode: CheckMode,
    ) -> TypeHandle {
        self.check_node_deferred(node);
        self.get_jsx_element_type_at(node)
            .unwrap_or(self.semantic_state.semantic_handles().error_type)
    }

    pub(crate) fn check_jsx_element_deferred(&mut self, node: ast::Node) {
        let (opening_element, closing_element) = {
            let store = self.store_for_node(node);
            (
                store.opening_element(node).unwrap(),
                store.closing_element(node).unwrap(),
            )
        };
        self.check_jsx_opening_like_element_or_opening_fragment(opening_element);
        // Perform resolution on the closing tag so that rename/go to definition/etc work
        let closing_tag_name = self
            .store_for_node(closing_element)
            .tag_name(closing_element)
            .unwrap();
        if is_jsx_intrinsic_tag_name(self.store_for_node(closing_tag_name), closing_tag_name) {
            self.get_intrinsic_tag_symbol(closing_element);
        } else {
            self.check_expression(closing_tag_name);
        }
        self.check_jsx_children(node, CHECK_MODE_NORMAL);
    }

    pub(crate) fn check_jsx_expression(
        &mut self,
        node: ast::Node,
        check_mode: CheckMode,
    ) -> TypeHandle {
        self.check_grammar_jsx_expression(node);
        if self.node_expression(node).is_none() {
            return self.semantic_state.semantic_handles().error_type;
        }
        let expression = self.node_expression(node).unwrap();
        let t = self.check_expression_ex(expression, check_mode);
        if self.store_for_node(node).dot_dot_dot_token(node).is_some()
            && t != self.semantic_state.semantic_handles().any_type
            && !self.is_array_type(t)
        {
            self.error(
                node,
                &diagnostics::JSX_SPREAD_CHILD_MUST_BE_AN_ARRAY_TYPE,
                (),
            );
        }
        t
    }

    pub(crate) fn check_jsx_self_closing_element(
        &mut self,
        node: ast::Node,
        _check_mode: CheckMode,
    ) -> TypeHandle {
        self.check_node_deferred(node);
        self.get_jsx_element_type_at(node)
            .unwrap_or(self.semantic_state.semantic_handles().error_type)
    }

    pub(crate) fn check_jsx_self_closing_element_deferred(&mut self, node: ast::Node) {
        self.check_jsx_opening_like_element_or_opening_fragment(node);
    }

    pub(crate) fn check_jsx_fragment(&mut self, node: ast::Node) -> TypeHandle {
        let opening_fragment = {
            let store = self.store_for_node(node);
            store.opening_fragment(node).unwrap()
        };
        self.check_jsx_opening_like_element_or_opening_fragment(opening_fragment);
        // by default, jsx:'react' will use jsxFactory = React.createElement and jsxFragmentFactory = React.Fragment
        // if jsxFactory compiler option is provided, ensure jsxFragmentFactory compiler option or @jsxFrag pragma is provided too
        let node_source_file = Some(self.source_file_for_node(node));
        if self.compiler_options.get_jsx_transform_enabled()
            && (self.compiler_options.jsx_factory != ""
                || ast::get_pragma_from_source_file(node_source_file, "jsx").is_some())
            && self.compiler_options.jsx_fragment_factory == ""
            && ast::get_pragma_from_source_file(node_source_file, "jsxfrag").is_none()
        {
            let message: &'static diagnostics::Message = if self.compiler_options.jsx_factory != ""
            {
                &diagnostics::THE_JSX_FRAGMENT_FACTORY_COMPILER_OPTION_MUST_BE_PROVIDED_TO_USE_JSX_FRAGMENTS_WITH_THE_JSX_FACTORY_COMPILER_OPTION
            } else {
                &diagnostics::AN_JSX_FRAG_PRAGMA_IS_REQUIRED_WHEN_USING_AN_JSX_PRAGMA_WITH_JSX_FRAGMENTS
            };
            self.error(node, message, ());
        }
        self.check_jsx_children(node, CHECK_MODE_NORMAL);
        let t = self
            .get_jsx_element_type_at(node)
            .unwrap_or(self.semantic_state.semantic_handles().error_type);
        if self.is_error_type(t) {
            self.semantic_state.semantic_handles().any_type
        } else {
            t
        }
    }

    pub(crate) fn check_jsx_attributes(
        &mut self,
        node: ast::Node,
        check_mode: CheckMode,
    ) -> TypeHandle {
        self.create_jsx_attributes_type_from_attributes_property(
            self.node_parent(node).unwrap(),
            check_mode,
        )
    }

    fn check_jsx_opening_like_element_or_opening_fragment(&mut self, node: ast::Node) {
        let is_node_opening_like_element =
            ast::is_jsx_opening_like_element(self.store_for_node(node), node);
        if is_node_opening_like_element {
            self.check_grammar_jsx_element(node);
        }
        self.check_jsx_preconditions(node);
        self.mark_jsx_alias_referenced(node);
        let sig = self.get_resolved_signature(node, None, CHECK_MODE_NORMAL);
        self.check_deprecated_signature(sig, node);
        if is_node_opening_like_element {
            let element_type_constraint = self.get_jsx_element_type_type_at(node);
            if let Some(element_type_constraint) = element_type_constraint {
                let tag_name = self.store_for_node(node).tag_name(node).unwrap();
                let tag_type = if is_jsx_intrinsic_tag_name(self.store_for_node(tag_name), tag_name)
                {
                    let tag_name_text = self.node_text(tag_name);
                    self.get_string_literal_type(&tag_name_text)
                } else {
                    self.check_expression(tag_name)
                };
                let mut diags = Vec::new();
                if !self.check_type_related_to_ex(
                    tag_type,
                    element_type_constraint,
                    self.semantic_state.assignable_relation,
                    Some(tag_name),
                    Some(&diagnostics::ITS_TYPE_0_IS_NOT_A_VALID_JSX_ELEMENT_TYPE),
                    Some(&mut diags),
                ) {
                    let text =
                        scanner::get_text_of_node(self.source_file_for_node(tag_name), &tag_name);
                    let args: Vec<diagnostics::Argument> = vec![text.into()];
                    self.diagnostics().add(ast::new_diagnostic_chain(
                        Some(diags[0].clone()),
                        &diagnostics::X_0_CANNOT_BE_USED_AS_A_JSX_COMPONENT,
                        &args,
                    ));
                }
            } else {
                let ref_kind = self.get_jsx_reference_kind(node);
                let return_type = self.get_return_type_of_signature(sig);
                self.check_jsx_return_assignable_to_appropriate_bound(ref_kind, return_type, node);
            }
        }
    }

    fn check_jsx_preconditions(&mut self, error_node: ast::Node) {
        // Preconditions for using JSX
        if self.compiler_options.jsx == core::JSX_EMIT_NONE {
            self.error(
                error_node,
                &diagnostics::CANNOT_USE_JSX_UNLESS_THE_JSX_FLAG_IS_PROVIDED,
                (),
            );
        }
        if self.no_implicit_any() && self.get_jsx_element_type_at(error_node).is_none() {
            self.error(
                error_node,
                &diagnostics::JSX_ELEMENT_IMPLICITLY_HAS_TYPE_ANY_BECAUSE_THE_GLOBAL_TYPE_JSX_ELEMENT_DOES_NOT_EXIST,
                (),
            );
        }
    }

    fn check_jsx_return_assignable_to_appropriate_bound(
        &mut self,
        ref_kind: JsxReferenceKind,
        elem_instance_type: TypeHandle,
        opening_like_element: ast::Node,
    ) {
        let mut diags = Vec::new();
        let tag_name = self
            .store_for_node(opening_like_element)
            .tag_name(opening_like_element)
            .unwrap();
        match ref_kind {
            JsxReferenceKind::Function => {
                if let Some(sfc_return_constraint) =
                    self.get_jsx_stateless_element_type_at(opening_like_element)
                {
                    self.check_type_related_to_ex(
                        elem_instance_type,
                        sfc_return_constraint,
                        self.semantic_state.assignable_relation,
                        Some(tag_name),
                        Some(&diagnostics::ITS_RETURN_TYPE_0_IS_NOT_A_VALID_JSX_ELEMENT),
                        Some(&mut diags),
                    );
                }
            }
            JsxReferenceKind::Component => {
                if let Some(class_constraint) =
                    self.get_jsx_element_class_type_at(opening_like_element)
                {
                    // Issue an error if this return type isn't assignable to JSX.ElementClass, failing that
                    self.check_type_related_to_ex(
                        elem_instance_type,
                        class_constraint,
                        self.semantic_state.assignable_relation,
                        Some(tag_name),
                        Some(&diagnostics::ITS_INSTANCE_TYPE_0_IS_NOT_A_VALID_JSX_ELEMENT),
                        Some(&mut diags),
                    );
                }
            }
            JsxReferenceKind::Mixed => {
                let sfc_return_constraint =
                    self.get_jsx_stateless_element_type_at(opening_like_element);
                let class_constraint = self.get_jsx_element_class_type_at(opening_like_element);
                if sfc_return_constraint.is_none() || class_constraint.is_none() {
                    return;
                }
                let combined = self.get_union_type(vec![
                    sfc_return_constraint.unwrap(),
                    class_constraint.unwrap(),
                ]);
                self.check_type_related_to_ex(
                    elem_instance_type,
                    combined,
                    self.semantic_state.assignable_relation,
                    Some(tag_name),
                    Some(&diagnostics::ITS_ELEMENT_TYPE_0_IS_NOT_A_VALID_JSX_ELEMENT),
                    Some(&mut diags),
                );
            }
        }
        if !diags.is_empty() {
            let text = scanner::get_text_of_node(self.source_file_for_node(tag_name), &tag_name);
            let args: Vec<diagnostics::Argument> = vec![text.into()];
            self.diagnostics().add(ast::new_diagnostic_chain(
                Some(diags[0].clone()),
                &diagnostics::X_0_CANNOT_BE_USED_AS_A_JSX_COMPONENT,
                &args,
            ));
        }
    }

    pub(crate) fn infer_jsx_type_arguments(
        &mut self,
        node: ast::Node,
        signature: SignatureHandle,
        check_mode: CheckMode,
        context: InferenceContextRef,
    ) -> Vec<TypeHandle> {
        let param_type = self.get_effective_first_argument_for_jsx_signature(signature, node);
        let check_attr_type = self.check_expression_with_contextual_type(
            self.store_for_node(node).attributes(node).unwrap(),
            param_type,
            Some(context.clone()),
            check_mode,
        );
        let mut inferences =
            std::mem::take(&mut self.inference_context_record_mut(context).inferences);
        self.infer_types(
            &mut inferences,
            check_attr_type,
            param_type,
            INFERENCE_PRIORITY_NONE,
            false,
        );
        self.inference_context_record_mut(context).inferences = inferences;
        self.get_inferred_types(context)
    }

    pub(crate) fn get_contextual_type_for_jsx_expression(
        &mut self,
        node: ast::Node,
        context_flags: ContextFlags,
    ) -> Option<TypeHandle> {
        let parent = self.node_parent(node).unwrap();
        if ast::is_jsx_attribute_like(self.store_for_node(parent), &parent) {
            return self.get_contextual_type(node, context_flags);
        }
        if ast::is_jsx_element(self.store_for_node(parent), parent) {
            return self.get_contextual_type_for_child_jsx_expression(parent, node, context_flags);
        }
        None
    }

    pub(crate) fn get_contextual_type_for_jsx_attribute(
        &mut self,
        attribute: ast::Node,
        context_flags: ContextFlags,
    ) -> Option<TypeHandle> {
        // When we trying to resolve JsxOpeningLikeElement as a stateless function element, we will already give its attributes a contextual type
        // which is a type of the parameter of the signature we are trying out.
        // If there is no contextual type (e.g. we are trying to resolve stateful component), get attributes type from resolving element's tagName
        if ast::is_jsx_attribute(self.store_for_node(attribute), attribute) {
            let attributes_type = self.get_apparent_type_of_contextual_type(
                self.node_parent(attribute).unwrap(),
                context_flags,
            );
            if attributes_type.is_none() || is_type_any(self, attributes_type) {
                return None;
            }
            let attribute_name = self.node_name(attribute).unwrap();
            let attribute_name_text = self.node_text(attribute_name);
            return self.get_type_of_property_of_contextual_type(
                attributes_type.unwrap(),
                &attribute_name_text,
            );
        }
        self.get_contextual_type(self.node_parent(attribute).unwrap(), context_flags)
    }

    pub(crate) fn get_contextual_jsx_element_attributes_type(
        &mut self,
        node: ast::Node,
        context_flags: ContextFlags,
    ) -> Option<TypeHandle> {
        if ast::is_jsx_opening_element(self.store_for_node(node), node)
            && context_flags != CONTEXT_FLAGS_IGNORE_NODE_INFERENCES
        {
            let index = self.semantic_state.find_contextual_node_index(
                self.node_parent(node).unwrap(),
                context_flags == CONTEXT_FLAGS_NONE,
            );
            if let Some(index) = index {
                // Contextually applied type is moved from attributes up to the outer jsx attributes so when walking up from the children they get hit
                // _However_ to hit them from the _attributes_ we must look for them here; otherwise we'll used the declared type
                // (as below) instead!
                return self.semantic_state.contextual_info_type_at(index);
            }
        }
        self.get_contextual_type_for_argument_at_index(node, 0)
    }

    fn get_contextual_type_for_child_jsx_expression(
        &mut self,
        node: ast::Node,
        child: ast::Node,
        context_flags: ContextFlags,
    ) -> Option<TypeHandle> {
        let opening_element = {
            let store = self.store_for_node(node);
            store.opening_element(node).unwrap()
        };
        let attributes = self
            .store_for_node(opening_element)
            .attributes(opening_element)
            .unwrap();
        let attributes_type = self.get_apparent_type_of_contextual_type(attributes, context_flags);
        // JSX expression is in children of JSX Element, we will look for an "children" attribute (we get the name from JSX.ElementAttributesProperty)
        let jsx_namespace = self.get_jsx_namespace_at(Some(node));
        let jsx_children_property_name = self.get_jsx_element_children_property_name(jsx_namespace);
        if !(attributes_type.is_some()
            && !is_type_any(self, attributes_type)
            && jsx_children_property_name != ast::INTERNAL_SYMBOL_NAME_MISSING
            && jsx_children_property_name != "")
        {
            return None;
        }
        let real_children = {
            let store = self.store_for_node(node);
            let children = store
                .children(node)
                .unwrap()
                .into_iter()
                .collect::<Vec<_>>();
            ast::get_semantic_jsx_children(store, &children)
        };
        let child_index = real_children
            .iter()
            .position(|c| {
                ast::get_node_id(self.store_for_node(*c), *c)
                    == ast::get_node_id(self.store_for_node(child), child)
            })
            .unwrap_or(usize::MAX);
        let child_field_type = self.get_type_of_property_of_contextual_type(
            attributes_type.unwrap(),
            &jsx_children_property_name,
        );
        if child_field_type.is_none() {
            return None;
        }
        if real_children.len() == 1 {
            return child_field_type;
        }
        Some(self.map_type_ex(
            child_field_type.unwrap(),
            |checker, t| {
                if checker.is_array_like_type(t) {
                    let index_type =
                        checker.get_number_literal_type(jsnum::Number(child_index as f64));
                    return checker.get_indexed_access_type(t, index_type);
                }
                t
            },
            true, /*noReductions*/
        ))
    }

    pub(crate) fn discriminate_contextual_type_by_jsx_attributes(
        &mut self,
        node: ast::Node,
        contextual_type: TypeHandle,
    ) -> TypeHandle {
        let key = DiscriminatedContextualTypeKey {
            node_id: ast::get_node_id(self.store_for_node(node), node),
            type_id: self.type_id(contextual_type),
        };
        if let Some(discriminated) = self.semantic_state.discriminated_contextual_type(&key) {
            return discriminated;
        }
        let jsx_namespace = self.get_jsx_namespace_at(Some(node));
        let jsx_children_property_name = self.get_jsx_element_children_property_name(jsx_namespace);
        let mut discriminant_properties = Vec::new();
        // PORT NOTE: reshaped for borrowck. TS-Go filters with closures that call
        // back into Checker; keep iteration order and predicate order.
        for p in self
            .store_for_node(node)
            .properties(node)
            .unwrap()
            .iter()
            .collect::<Vec<_>>()
        {
            let symbol = self.node_symbol(p);
            if symbol.is_none() || !ast::is_jsx_attribute(self.store_for_node(p), p) {
                continue;
            }
            let initializer = self.node_initializer(p);
            if (initializer.is_none() || self.is_possibly_discriminant_value(initializer.unwrap()))
                && {
                    let symbol = symbol.unwrap();
                    let symbol_name = self.source_symbol_name_for_node(p, symbol);
                    self.is_discriminant_property(contextual_type, &symbol_name)
                }
            {
                discriminant_properties.push(p);
            }
        }
        let mut discriminant_members = Vec::new();
        for s in self.get_properties_of_type(contextual_type) {
            let is_optional =
                self.missing_name_symbol_identity_flags(s) & ast::SYMBOL_FLAGS_OPTIONAL != 0;
            let name = self.missing_name_symbol_identity_name(s);
            let node_symbol = self.node_symbol(node);
            if !is_optional || node_symbol.is_none() {
                continue;
            }
            let element = self.node_parent(self.node_parent(node).unwrap()).unwrap();
            if name == jsx_children_property_name
                && ast::is_jsx_element(self.store_for_node(element), element)
                && {
                    let store = self.store_for_node(element);
                    let children = store
                        .children(element)
                        .unwrap()
                        .into_iter()
                        .collect::<Vec<_>>();
                    !ast::get_semantic_jsx_children(store, &children).is_empty()
                }
            {
                continue;
            }
            let node_symbol = node_symbol.unwrap();
            let is_missing = !self.source_symbol_has_member_for_node(node, node_symbol, &name);
            if is_missing && self.is_discriminant_property(contextual_type, &name) {
                discriminant_members.push(s);
            }
        }
        let discriminator =
            ObjectLiteralDiscriminator::new(self, discriminant_properties, discriminant_members);
        let discriminated =
            self.discriminate_type_by_discriminable_items(contextual_type, &discriminator);
        self.semantic_state
            .set_discriminated_contextual_type(key, discriminated);
        discriminated
    }

    pub(crate) fn elaborate_jsx_components(
        &mut self,
        node: ast::Node,
        source: TypeHandle,
        target: TypeHandle,
        relation: RelationKind,
        containing_message_chain: ContainingMessageChain<'_>,
        diagnostic_output: Option<&mut Vec<ast::Diagnostic>>,
    ) -> bool {
        let mut reported_error = false;
        let mut diagnostic_output = diagnostic_output;
        for prop in self
            .store_for_node(node)
            .properties(node)
            .unwrap()
            .iter()
            .collect::<Vec<_>>()
        {
            if ast::is_jsx_spread_attribute(self.store_for_node(prop), prop) {
                continue;
            }
            let prop_name = self.node_name(prop).unwrap();
            let prop_name_text = self.node_text(prop_name);
            if !is_hyphenated_jsx_name(&prop_name_text) {
                let name_type = self.get_string_literal_type(&prop_name_text);
                if self.type_flags(name_type) & TYPE_FLAGS_NEVER == 0 {
                    reported_error = self.elaborate_element_with_chain(
                        source,
                        target,
                        relation,
                        prop_name,
                        self.node_initializer(prop),
                        name_type,
                        None,
                        None,
                        containing_message_chain,
                        diagnostic_output.as_mut().map(|v| &mut **v),
                    ) || reported_error;
                }
            }
        }
        let parent = self.node_parent(node).unwrap();
        if ast::is_jsx_opening_element(self.store_for_node(parent), parent)
            && self
                .node_parent(parent)
                .is_some_and(|parent| ast::is_jsx_element(self.store_for_node(parent), parent))
        {
            let containing_element = self.node_parent(parent).unwrap(); // Containing JSXElement
            let jsx_namespace = self.get_jsx_namespace_at(Some(node));
            let mut children_prop_name = self
                .get_jsx_element_children_property_name(jsx_namespace)
                .to_string();
            if children_prop_name == ast::INTERNAL_SYMBOL_NAME_MISSING {
                children_prop_name = "children".to_string();
            }
            let children_name_type = self.get_string_literal_type(&children_prop_name);
            let children_target_type = self.get_indexed_access_type(target, children_name_type);
            let valid_children = {
                let store = self.store_for_node(containing_element);
                let children = store
                    .children(containing_element)
                    .unwrap()
                    .into_iter()
                    .collect::<Vec<_>>();
                ast::get_semantic_jsx_children(store, &children)
            };
            if valid_children.is_empty() {
                return reported_error;
            }
            let more_than_one_real_children = valid_children.len() > 1;
            let iterable_type = {
                let resolver = (self.semantic_state.get_global_iterable_type).clone();
                self.resolve_global_type(resolver)
            };
            let (array_like_target_parts, non_array_like_target_parts) =
                if iterable_type != self.semantic_state.semantic_handles().empty_generic_type {
                    let any_iterable =
                        self.create_iterable_type(self.semantic_state.semantic_handles().any_type);
                    (
                        self.filter_type_with_checker(children_target_type, move |checker, t| {
                            checker.is_type_assignable_to(t, any_iterable)
                        }),
                        self.filter_type_with_checker(children_target_type, move |checker, t| {
                            !checker.is_type_assignable_to(t, any_iterable)
                        }),
                    )
                } else {
                    (
                        self.filter_type_with_checker(children_target_type, move |checker, t| {
                            checker.is_array_or_tuple_like_type(t)
                        }),
                        self.filter_type_with_checker(children_target_type, move |checker, t| {
                            !checker.is_array_or_tuple_like_type(t)
                        }),
                    )
                };
            let mut invalid_text_diagnostic: Option<&'static diagnostics::Message> = None;
            let mut invalid_text_diagnostic_args: Vec<DiagnosticArg> = Vec::new();
            let parent_tag_name = self.store_for_node(parent).tag_name(parent).unwrap();
            let parent_tag_name_text = scanner::get_text_of_node(
                self.source_file_for_node(parent_tag_name),
                &parent_tag_name,
            );
            let children_prop_name_for_diagnostic = children_prop_name.clone();
            let children_target_type_string = self.type_to_string(children_target_type, None);
            let mut get_invalid_textual_child_diagnostic = move || {
                if invalid_text_diagnostic.is_none() {
                    invalid_text_diagnostic = Some(
                        &diagnostics::X_0_COMPONENTS_DON_T_ACCEPT_TEXT_AS_CHILD_ELEMENTS_TEXT_IN_JSX_HAS_THE_TYPE_STRING_BUT_THE_EXPECTED_TYPE_OF_1_IS_2,
                    );
                    invalid_text_diagnostic_args = vec![
                        parent_tag_name_text.clone().into(),
                        children_prop_name_for_diagnostic.clone().into(),
                        children_target_type_string.clone().into(),
                    ];
                }
                (
                    invalid_text_diagnostic.unwrap(),
                    invalid_text_diagnostic_args.clone(),
                )
            };
            if more_than_one_real_children {
                if array_like_target_parts != self.semantic_state.semantic_handles().never_type {
                    let checked_children =
                        self.check_jsx_children(containing_element, CHECK_MODE_NORMAL);
                    let real_source = self.create_tuple_type(checked_children);
                    let children = self.generate_jsx_children(
                        containing_element,
                        &mut get_invalid_textual_child_diagnostic,
                    );
                    reported_error = self.elaborate_iterable_or_array_like_target_elementwise(
                        children,
                        real_source,
                        array_like_target_parts,
                        relation,
                        containing_message_chain,
                        diagnostic_output.as_mut().map(|v| &mut **v),
                    ) || reported_error;
                } else {
                    let source_children_type =
                        self.get_indexed_access_type(source, children_name_type);
                    if !self.is_type_related_to(
                        source_children_type,
                        children_target_type,
                        relation,
                    ) {
                        // arity mismatch
                        let arity_error_node = {
                            let store = self.store_for_node(containing_element);
                            let opening_element =
                                store.opening_element(containing_element).unwrap();
                            self.store_for_node(opening_element)
                                .tag_name(opening_element)
                                .unwrap()
                        };
                        let children_target_string =
                            self.type_to_string(children_target_type, None);
                        let diag = self.error(
                        Some(arity_error_node),
                        &diagnostics::THIS_JSX_TAG_S_0_PROP_EXPECTS_A_SINGLE_CHILD_OF_TYPE_1_BUT_MULTIPLE_CHILDREN_WERE_PROVIDED,
                        Vec::<DiagnosticArg>::from([
                            children_prop_name.clone().into(),
                            children_target_string.into(),
                        ]),
                    );
                        self.report_diagnostic(
                            Some(diag),
                            diagnostic_output.as_mut().map(|v| &mut **v),
                        );
                        reported_error = true;
                    }
                }
            } else if non_array_like_target_parts
                != self.semantic_state.semantic_handles().never_type
            {
                let child = valid_children[0].clone();
                let e = self.get_elaboration_element_for_jsx_child(
                    child,
                    children_name_type,
                    &mut get_invalid_textual_child_diagnostic,
                );
                if let Some(error_node) = e.error_node {
                    reported_error = self.elaborate_element_with_chain(
                        source,
                        target,
                        relation,
                        error_node,
                        e.inner_expression,
                        e.name_type.unwrap(),
                        None,
                        e.create_diagnostic,
                        containing_message_chain,
                        diagnostic_output.as_mut().map(|v| &mut **v),
                    ) || reported_error;
                }
            } else {
                let source_children_type = self.get_indexed_access_type(source, children_name_type);
                if !self.is_type_related_to(source_children_type, children_target_type, relation) {
                    // arity mismatch
                    let arity_error_node = {
                        let store = self.store_for_node(containing_element);
                        let opening_element = store.opening_element(containing_element).unwrap();
                        self.store_for_node(opening_element)
                            .tag_name(opening_element)
                            .unwrap()
                    };
                    let children_target_string = self.type_to_string(children_target_type, None);
                    let diag = self.error(
                    Some(arity_error_node),
                    &diagnostics::THIS_JSX_TAG_S_0_PROP_EXPECTS_TYPE_1_WHICH_REQUIRES_MULTIPLE_CHILDREN_BUT_ONLY_A_SINGLE_CHILD_WAS_PROVIDED,
                    Vec::<DiagnosticArg>::from([
                        children_prop_name.clone().into(),
                        children_target_string.into(),
                    ]),
                );
                    self.report_diagnostic(
                        Some(diag),
                        diagnostic_output.as_mut().map(|v| &mut **v),
                    );
                    reported_error = true;
                }
            }
        }
        reported_error
    }

    fn generate_jsx_children<F>(
        &mut self,
        node: ast::Node,
        get_invalid_text_diagnostic: &mut F,
    ) -> Vec<JsxElaborationElement<'a>>
    where
        F: FnMut() -> (&'static diagnostics::Message, Vec<DiagnosticArg>),
    {
        let mut result = Vec::new();
        let mut member_offset = 0;
        let children = self
            .store_for_node(node)
            .children(node)
            .unwrap()
            .iter()
            .collect::<Vec<_>>();
        for (i, child) in children.into_iter().enumerate() {
            let name_type = self.get_number_literal_type(jsnum::Number((i - member_offset) as f64));
            let e = self.get_elaboration_element_for_jsx_child(
                child,
                name_type,
                get_invalid_text_diagnostic,
            );
            if e.error_node.is_some() {
                result.push(e);
            } else {
                member_offset += 1;
            }
        }
        result
    }

    fn get_elaboration_element_for_jsx_child<F>(
        &mut self,
        child: ast::Node,
        name_type: TypeHandle,
        get_invalid_text_diagnostic: &mut F,
    ) -> JsxElaborationElement<'a>
    where
        F: FnMut() -> (&'static diagnostics::Message, Vec<DiagnosticArg>),
    {
        match self.store_for_node(child).kind(child) {
            ast::KIND_JSX_EXPRESSION => {
                // child is of the type of the expression
                JsxElaborationElement {
                    error_node: Some(child),
                    inner_expression: self.node_expression(child),
                    name_type: Some(name_type),
                    create_diagnostic: None,
                }
            }
            ast::KIND_JSX_TEXT => {
                if self
                    .store_for_node(child)
                    .contains_only_trivia_white_spaces(child)
                    .unwrap_or(false)
                {
                    // Whitespace only jsx text isn't real jsx text
                    return JsxElaborationElement {
                        error_node: None,
                        inner_expression: None,
                        name_type: None,
                        create_diagnostic: None,
                    };
                }
                // child is a string
                let (error_message, error_args) = get_invalid_text_diagnostic();
                let store = self.store_for_node(child);
                JsxElaborationElement {
                    error_node: Some(child),
                    inner_expression: None,
                    name_type: Some(name_type),
                    create_diagnostic: Some(Box::new(move |prop| {
                        new_diagnostic_for_node(
                            store,
                            Some(prop),
                            error_message,
                            error_args.clone(),
                        )
                    })),
                }
            }
            ast::KIND_JSX_ELEMENT | ast::KIND_JSX_SELF_CLOSING_ELEMENT | ast::KIND_JSX_FRAGMENT => {
                // child is of type JSX.Element
                JsxElaborationElement {
                    error_node: Some(child),
                    inner_expression: Some(child),
                    name_type: Some(name_type),
                    create_diagnostic: None,
                }
            }
            _ => panic!("Unhandled case in getElaborationElementForJsxChild"),
        }
    }

    fn elaborate_iterable_or_array_like_target_elementwise(
        &mut self,
        iterator: Vec<JsxElaborationElement<'a>>,
        source: TypeHandle,
        target: TypeHandle,
        relation: RelationKind,
        containing_message_chain: ContainingMessageChain<'_>,
        diagnostic_output: Option<&mut Vec<ast::Diagnostic>>,
    ) -> bool {
        let mut diagnostic_output = diagnostic_output;
        let tuple_or_array_like_target_parts = self
            .filter_type_with_checker(target, |checker, t| checker.is_array_or_tuple_like_type(t));
        let non_tuple_or_array_like_target_parts = self
            .filter_type_with_checker(target, |checker, t| !checker.is_array_or_tuple_like_type(t));
        // If `nonTupleOrArrayLikeTargetParts` is not `never`, then that should mean `Iterable` is defined.
        let mut iteration_type = None;
        if non_tuple_or_array_like_target_parts != self.semantic_state.semantic_handles().never_type
        {
            iteration_type = self.get_iteration_type_of_iterable(
                ITERATION_USE_FOR_OF,
                ITERATION_TYPE_KIND_YIELD,
                non_tuple_or_array_like_target_parts,
                None, /*errorNode*/
            );
        }
        let mut reported_error = false;
        for e in iterator {
            let prop = e.error_node.unwrap();
            let next = e.inner_expression;
            let name_type = e.name_type.unwrap();
            let mut target_prop_type = iteration_type;
            let mut target_indexed_prop_type = None;
            if tuple_or_array_like_target_parts != self.semantic_state.semantic_handles().never_type
            {
                target_indexed_prop_type = self.get_best_match_indexed_access_type_or_undefined(
                    source,
                    tuple_or_array_like_target_parts,
                    name_type,
                );
            }
            if target_indexed_prop_type.is_some()
                && self.type_flags(target_indexed_prop_type.unwrap()) & TYPE_FLAGS_INDEXED_ACCESS
                    == 0
            {
                target_prop_type =
                    if let Some(iteration_type) = iteration_type {
                        Some(self.get_union_type(vec![
                            iteration_type,
                            target_indexed_prop_type.unwrap(),
                        ]))
                    } else {
                        target_indexed_prop_type
                    };
            }
            if target_prop_type.is_none() {
                continue;
            }
            let mut source_prop_type = self.get_indexed_access_type_or_undefined(
                source,
                name_type,
                ACCESS_FLAGS_NONE,
                None,
                None,
            );
            if source_prop_type.is_none() {
                continue;
            }
            let prop_name = self.get_property_name_from_index(name_type, None /*accessNode*/);
            if !self.check_type_related_to(
                source_prop_type.unwrap(),
                target_prop_type.unwrap(),
                relation,
                None, /*errorNode*/
            ) {
                let elaborated = next.is_some()
                    && self.elaborate_error(
                        next,
                        source_prop_type.unwrap(),
                        target_prop_type.unwrap(),
                        relation,
                        None, /*headMessage*/
                        containing_message_chain,
                        diagnostic_output.as_mut().map(|v| &mut **v),
                    );
                reported_error = true;
                if !elaborated {
                    // Issue error on the prop itself, since the prop couldn't elaborate the error. Use the expression type, if available.
                    let mut specific_source = source_prop_type.unwrap();
                    if let Some(next) = next {
                        specific_source = self
                            .check_expression_for_mutable_location_with_contextual_type(
                                next,
                                source_prop_type.unwrap(),
                            );
                    }
                    if let Some(create_diagnostic) = e.create_diagnostic {
                        // Use the custom diagnostic factory if provided (e.g., for JSX text children with dynamic error messages)
                        self.report_diagnostic(
                            Some(create_diagnostic(prop)),
                            diagnostic_output.as_mut().map(|v| &mut **v),
                        );
                    } else if self.exact_optional_property_types()
                        && self.is_exact_optional_property_mismatch(
                            Some(specific_source),
                            target_prop_type,
                        )
                    {
                        let diag = create_diagnostic_for_node_with_args(
                                self.store_for_node(prop),
	                            prop,
	                            &diagnostics::TYPE_0_IS_NOT_ASSIGNABLE_TO_TYPE_1_WITH_EXACT_OPTIONAL_PROPERTY_TYPES_COLON_TRUE_CONSIDER_ADDING_UNDEFINED_TO_THE_TYPE_OF_THE_TARGET,
                            Vec::<DiagnosticArg>::from([
                                self.type_to_string(specific_source, None).into(),
                                self.type_to_string(target_prop_type.unwrap(), None).into(),
                            ]),
                        );
                        self.report_diagnostic(
                            Some(diag),
                            diagnostic_output.as_mut().map(|v| &mut **v),
                        );
                    } else {
                        let target_is_optional = if prop_name != ast::INTERNAL_SYMBOL_NAME_MISSING {
                            let symbol = self
                                .get_property_of_type(tuple_or_array_like_target_parts, &prop_name)
                                .unwrap_or(self.unknown_symbol());
                            self.missing_name_symbol_identity_flags(symbol)
                                & ast::SYMBOL_FLAGS_OPTIONAL
                                != 0
                        } else {
                            false
                        };
                        let source_is_optional = if prop_name != ast::INTERNAL_SYMBOL_NAME_MISSING {
                            let symbol = self
                                .get_property_of_type(source, &prop_name)
                                .unwrap_or(self.unknown_symbol());
                            self.missing_name_symbol_identity_flags(symbol)
                                & ast::SYMBOL_FLAGS_OPTIONAL
                                != 0
                        } else {
                            false
                        };
                        let target_prop_type =
                            self.remove_missing_type(target_prop_type.unwrap(), target_is_optional);
                        source_prop_type = Some(self.remove_missing_type(
                            source_prop_type.unwrap(),
                            target_is_optional && source_is_optional,
                        ));
                        let result = self.check_type_related_to_with_chain(
                            specific_source,
                            target_prop_type,
                            relation,
                            Some(prop),
                            None,
                            containing_message_chain,
                            diagnostic_output.as_mut().map(|v| &mut **v),
                        );
                        if result && specific_source != source_prop_type.unwrap() {
                            // If for whatever reason the expression type doesn't yield an error, make sure we still issue an error on the sourcePropType
                            self.check_type_related_to_with_chain(
                                source_prop_type.unwrap(),
                                target_prop_type,
                                relation,
                                Some(prop),
                                None,
                                containing_message_chain,
                                diagnostic_output.as_mut().map(|v| &mut **v),
                            );
                        }
                    }
                }
            }
        }
        reported_error
    }

    pub(crate) fn get_suggested_symbol_for_nonexistent_jsx_attribute(
        &mut self,
        name: &str,
        containing_type: TypeHandle,
    ) -> Option<SymbolIdentity> {
        let properties = self.get_properties_of_type(containing_type);
        let jsx_specific = match name {
            "for" => properties
                .iter()
                .find(|&&x| self.missing_name_symbol_identity_name(x) == "htmlFor"),
            "class" => properties
                .iter()
                .find(|&&x| self.missing_name_symbol_identity_name(x) == "className"),
            _ => None,
        };
        if jsx_specific.is_some() {
            return jsx_specific.copied();
        }
        self.get_spelling_suggestion_for_name(name, &properties, ast::SYMBOL_FLAGS_VALUE)
    }

    fn get_jsx_fragment_type(&mut self, node: ast::Node) -> TypeHandle {
        // An opening fragment is required in order for `getJsxNamespace` to give the fragment factory
        let source_file = self.source_file_for_node(node);
        if let Some(jsx_fragment_type) = self
            .semantic_state
            .source_file_jsx_fragment_type(source_file)
        {
            return jsx_fragment_type;
        }
        let jsx_fragment_factory_name = self.get_jsx_namespace(Some(node));
        // #38720/60122, allow null as jsxFragmentFactory
        let should_resolve_factory_reference = (self.compiler_options.jsx == core::JSX_EMIT_REACT
            || self.compiler_options.jsx_fragment_factory != "")
            && jsx_fragment_factory_name != "null";
        if !should_resolve_factory_reference {
            self.semantic_state.set_source_file_jsx_fragment_type(
                source_file,
                self.semantic_state.semantic_handles().any_type,
            );
            return self.semantic_state.semantic_handles().any_type;
        }
        let mut jsx_factory_symbol =
            self.get_jsx_namespace_container_for_implicit_import(Some(node));
        if jsx_factory_symbol.is_none() {
            let should_module_ref_err = self.compiler_options.jsx != core::JSX_EMIT_PRESERVE
                && self.compiler_options.jsx != core::JSX_EMIT_REACT_NATIVE;
            let mut flags = ast::SYMBOL_FLAGS_VALUE;
            if !should_module_ref_err {
                flags &= !ast::SYMBOL_FLAGS_ENUM;
            }
            let jsx_factory_symbol_handle = self.resolve_name(
                Some(node),
                &jsx_fragment_factory_name,
                flags,
                Some(
                    &diagnostics::USING_JSX_FRAGMENTS_REQUIRES_FRAGMENT_FACTORY_0_TO_BE_IN_SCOPE_BUT_IT_COULD_NOT_BE_FOUND,
                ),
                true, /*isUse*/
                false, /*excludeGlobals*/
            );
            jsx_factory_symbol = jsx_factory_symbol_handle.map(SymbolIdentity::from_symbol_handle);
        }
        let Some(jsx_factory_symbol) = jsx_factory_symbol else {
            self.semantic_state.set_source_file_jsx_fragment_type(
                source_file,
                self.semantic_state.semantic_handles().error_type,
            );
            return self.semantic_state.semantic_handles().error_type;
        };
        {
            if self.missing_name_symbol_identity_name(jsx_factory_symbol) == ReactNames::FRAGMENT {
                let fragment_type = self.get_type_of_symbol_identity(jsx_factory_symbol);
                self.semantic_state
                    .set_source_file_jsx_fragment_type(source_file, fragment_type);
                return fragment_type;
            }
        }
        let mut resolved_alias = jsx_factory_symbol;
        if self.missing_name_symbol_identity_flags(jsx_factory_symbol) & ast::SYMBOL_FLAGS_ALIAS
            != 0
        {
            resolved_alias = self.resolve_alias_identity(jsx_factory_symbol);
        }

        let type_symbol = self.get_export_of_symbol_identity_by_meaning(
            resolved_alias,
            ReactNames::FRAGMENT,
            ast::SYMBOL_FLAGS_BLOCK_SCOPED_VARIABLE,
        );
        let jsx_fragment_type = if let Some(type_symbol) = type_symbol {
            self.get_type_of_symbol_identity_at_location(type_symbol, None)
        } else {
            self.semantic_state.semantic_handles().error_type
        };
        self.semantic_state
            .set_source_file_jsx_fragment_type(source_file, jsx_fragment_type);
        jsx_fragment_type
    }

    pub(crate) fn resolve_jsx_opening_like_element(
        &mut self,
        node: ast::Node,
        candidates_out_array: Option<&mut Vec<SignatureHandle>>,
        check_mode: CheckMode,
    ) -> SignatureHandle {
        let is_jsx_open_fragment = ast::is_jsx_opening_fragment(self.store_for_node(node), node);
        let expr_types = if !is_jsx_open_fragment {
            let tag_name = self.store_for_node(node).tag_name(node).unwrap();
            let tag_name_ref = tag_name;
            if is_jsx_intrinsic_tag_name(self.store_for_node(tag_name_ref), tag_name_ref) {
                let result = self.get_intrinsic_attributes_type_from_jsx_opening_like_element(node);
                let fake_signature = self.create_signature_for_jsx_intrinsic(node, result);
                let attributes = self.store_for_node(node).attributes(node).unwrap();
                let attributes_ref = attributes;
                let contextual_type =
                    self.get_effective_first_argument_for_jsx_signature(fake_signature, node);
                let attributes_type = self.check_expression_with_contextual_type(
                    attributes_ref,
                    contextual_type,
                    None, /*inferenceContext*/
                    CHECK_MODE_NORMAL,
                );
                self.check_type_assignable_to_and_optionally_elaborate(
                    attributes_type,
                    result,
                    Some(tag_name_ref),
                    Some(attributes_ref),
                    None,
                    None,
                );
                let type_arguments = self.store_for_node(node).type_arguments(node);
                if let Some(type_arguments) = type_arguments
                    && !type_arguments.is_empty()
                {
                    let type_arguments_refs = type_arguments.iter().collect::<Vec<_>>();
                    self.check_source_elements(&type_arguments_refs);
                    let diag_args: Vec<diagnostics::Argument> =
                        vec![0.into(), type_arguments_refs.len().into()];
                    self.diagnostics().add(ast::new_diagnostic(
                        Some(self.source_file_for_node(node)),
                        type_arguments.loc(),
                        &diagnostics::EXPECTED_0_TYPE_ARGUMENTS_BUT_GOT_1,
                        &diag_args,
                    ));
                }
                return fake_signature;
            }
            self.check_expression(tag_name_ref)
        } else {
            self.get_jsx_fragment_type(node)
        };
        let apparent_type = self.get_apparent_type(expr_types);
        if self.is_error_type(apparent_type) {
            return self.resolve_error_call(node);
        }
        let signatures = self.get_uninstantiated_jsx_signatures_of_type(expr_types, node);
        if self.is_untyped_function_call(
            expr_types,
            apparent_type,
            signatures.len(),
            0, /*constructSignatures*/
        ) {
            return self.resolve_untyped_call(node);
        }
        if signatures.is_empty() {
            // We found no signatures at all, which is an error
            if is_jsx_open_fragment {
                self.error(
                    Some(node),
                    &diagnostics::JSX_ELEMENT_TYPE_0_DOES_NOT_HAVE_ANY_CONSTRUCT_OR_CALL_SIGNATURES,
                    scanner::get_text_of_node(self.source_file_for_node(node), &node),
                );
            } else {
                let tag_name = self.store_for_node(node).tag_name(node).unwrap();
                self.error(
                    Some(tag_name),
                    &diagnostics::JSX_ELEMENT_TYPE_0_DOES_NOT_HAVE_ANY_CONSTRUCT_OR_CALL_SIGNATURES,
                    scanner::get_text_of_node(self.source_file_for_node(tag_name), &tag_name),
                );
            }
            return self.resolve_error_call(node);
        }
        self.resolve_call(
            node,
            &signatures,
            candidates_out_array,
            check_mode,
            SIGNATURE_FLAGS_NONE,
            None,
        )
    }

    // Check if the given signature can possibly be a signature called by the JSX opening-like element.
    // @param node a JSX opening-like element we are trying to figure its call signature
    // @param signature a candidate signature we are trying whether it is a call signature
    // @param relation a relationship to check parameter and argument type
    pub(crate) fn check_applicable_signature_for_jsx_call_like_element(
        &mut self,
        node: ast::Node,
        signature: SignatureHandle,
        relation: RelationKind,
        check_mode: CheckMode,
        report_errors: bool,
        containing_message_chain: ContainingMessageChain<'_>,
        diagnostic_output: &mut Vec<ast::Diagnostic>,
    ) -> bool {
        // Stateless function components can have maximum of three arguments: "props", "context", and "updater".
        // However "context" and "updater" are implicit and can't be specify by users. Only the first parameter, props,
        // can be specified by users through attributes property.
        let param_type = self.get_effective_first_argument_for_jsx_signature(signature, node);
        let attributes_type = if ast::is_jsx_opening_fragment(self.store_for_node(node), node) {
            self.create_jsx_attributes_type_from_attributes_property(node, CHECK_MODE_NORMAL)
        } else {
            let attributes = self.store_for_node(node).attributes(node).unwrap();
            self.check_expression_with_contextual_type(
                attributes, param_type, None, /*inferenceContext*/
                check_mode,
            )
        };
        let mut check_tag_name_does_not_expect_too_many_arguments = |checker: &mut Checker<
            'a,
            '_,
        >|
         -> bool {
            if checker
                .get_jsx_namespace_container_for_implicit_import(Some(node))
                .is_some()
            {
                return true; // factory is implicitly jsx/jsxdev - assume it fits the bill, since we don't strongly look for the jsx/jsxs/jsxDEV factory APIs anywhere else (at least not yet)
            }
            // We assume fragments have the correct arity since the node does not have attributes
            let mut tag_type = None;
            let tag_name = checker.store_for_node(node).tag_name(node);
            if (ast::is_jsx_opening_element(checker.store_for_node(node), node)
                || ast::is_jsx_self_closing_element(checker.store_for_node(node), node))
                && !(is_jsx_intrinsic_tag_name(
                    checker.store_for_node(tag_name.unwrap()),
                    tag_name.unwrap(),
                ) || ast::is_jsx_namespaced_name(
                    checker.store_for_node(tag_name.unwrap()),
                    tag_name.unwrap(),
                ))
            {
                tag_type = Some(checker.check_expression(tag_name.unwrap()));
            }
            let Some(tag_type) = tag_type else {
                return true;
            };
            let tag_call_signatures = checker.get_signatures_of_type(tag_type, SIGNATURE_KIND_CALL);
            if tag_call_signatures.is_empty() {
                return true;
            }
            let factory = checker.get_jsx_factory_entity(Some(node));
            if factory.is_none() {
                return true;
            }
            let factory_symbol = checker.resolve_entity_name(
                factory.unwrap(),
                ast::SYMBOL_FLAGS_VALUE,
                true,  /*ignoreErrors*/
                false, /*dontResolveAlias*/
                Some(node),
            );
            if factory_symbol.is_none() {
                return true;
            }

            let factory_symbol = factory_symbol.unwrap();
            let factory_type =
                checker.get_type_of_symbol_identity_at_location(factory_symbol, None);
            let call_signatures = checker.get_signatures_of_type(factory_type, SIGNATURE_KIND_CALL);
            if call_signatures.is_empty() {
                return true;
            }
            let mut has_first_param_signatures = false;
            let mut max_param_count = 0;
            // Check that _some_ first parameter expects a FC-like thing, and that some overload of the SFC expects an acceptable number of arguments
            for sig in call_signatures {
                let firstparam = checker.get_type_at_position(sig, 0);
                let signatures_of_param =
                    checker.get_signatures_of_type(firstparam, SIGNATURE_KIND_CALL);
                if signatures_of_param.is_empty() {
                    continue;
                }
                for param_sig in signatures_of_param {
                    has_first_param_signatures = true;
                    if checker.has_effective_rest_parameter(param_sig) {
                        return true; // some signature has a rest param, so function components can have an arbitrary number of arguments
                    }
                    let param_count = checker.get_parameter_count(param_sig);
                    if param_count > max_param_count {
                        max_param_count = param_count;
                    }
                }
            }
            if !has_first_param_signatures {
                // Not a single signature had a first parameter which expected a signature - for back compat, and
                // to guard against generic factories which won't have signatures directly, do not error
                return true;
            }
            let mut absolute_min_arg_count = usize::MAX;
            for tag_sig in tag_call_signatures {
                let tag_required_arg_count = checker.get_min_argument_count(tag_sig);
                if tag_required_arg_count < absolute_min_arg_count {
                    absolute_min_arg_count = tag_required_arg_count;
                }
            }
            if absolute_min_arg_count <= max_param_count {
                return true; // some signature accepts the number of arguments the function component provides
            }
            if report_errors {
                let tag_name = checker.store_for_node(node).tag_name(node).unwrap();
                // We will not report errors in this function for fragments, since we do not check them in this function
                let tag_name_string =
                    entity_name_to_string(checker.store_for_node(tag_name), tag_name);
                let factory_string =
                    entity_name_to_string(checker.factory().store(), factory.unwrap());
                let mut diag = new_diagnostic_for_node(
                        checker.store_for_node(tag_name),
                        Some(tag_name),
                        &diagnostics::TAG_0_EXPECTS_AT_LEAST_1_ARGUMENTS_BUT_THE_JSX_FACTORY_2_PROVIDES_AT_MOST_3,
                        Vec::<DiagnosticArg>::from([
                            tag_name_string.clone().into(),
                            absolute_min_arg_count.into(),
                            factory_string.into(),
                            max_param_count.into(),
                        ]),
                    );
                let tag_name_symbol = checker.get_symbol_at_location(tag_name, false);
                if let Some(tag_name_symbol) = tag_name_symbol {
                    if let Some(value_declaration) =
                        checker.missing_name_symbol_identity_value_declaration(tag_name_symbol)
                    {
                        diag.add_related_info(new_diagnostic_for_node(
                            checker.store_for_node(value_declaration),
                            Some(value_declaration),
                            &diagnostics::X_0_IS_DECLARED_HERE,
                            tag_name_string,
                        ));
                    }
                }
                diagnostic_output.push(diag);
            }
            false
        };
        let check_attributes_type = if check_mode & CHECK_MODE_SKIP_CONTEXT_SENSITIVE != 0 {
            self.get_regular_type_of_object_literal(attributes_type)
        } else {
            attributes_type
        };
        if !check_tag_name_does_not_expect_too_many_arguments(self) {
            return false;
        }
        let error_node = if report_errors {
            if ast::is_jsx_opening_fragment(self.store_for_node(node), node) {
                Some(node)
            } else {
                Some(self.store_for_node(node).tag_name(node).unwrap())
            }
        } else {
            None
        };
        let attributes = if !ast::is_jsx_opening_fragment(self.store_for_node(node), node) {
            Some(self.store_for_node(node).attributes(node).unwrap())
        } else {
            None
        };
        let mut diagnostic_output_refs = Vec::new();
        let result = self.check_type_related_to_and_optionally_elaborate_with_chain(
            check_attributes_type,
            param_type,
            relation,
            error_node,
            attributes,
            None,
            containing_message_chain,
            Some(&mut diagnostic_output_refs),
        );
        diagnostic_output.extend(diagnostic_output_refs);
        result
    }

    // Get attributes type of the JSX opening-like element. The result is from resolving "attributes" property of the opening-like element.
    //
    // @param openingLikeElement a JSX opening-like element
    // @param filter a function to remove attributes that will not participate in checking whether attributes are assignable
    // @return an anonymous type (similar to the one returned by checkObjectLiteral) in which its properties are attributes property.
    // @remarks Because this function calls getSpreadType, it needs to use the same checks as checkObjectLiteral,
    // which also calls getSpreadType.
    fn create_jsx_attributes_type_from_attributes_property(
        &mut self,
        opening_like_element: ast::Node,
        check_mode: CheckMode,
    ) -> TypeHandle {
        let mut all_attributes_table = if self.strict_null_checks() {
            Some(SymbolIdentityTable::default())
        } else {
            None
        };
        let mut attributes_table = SymbolIdentityTable::default();
        let mut attributes_symbol_identity: Option<SymbolIdentity> = None;
        let mut attribute_parent = opening_like_element;
        let mut spread = self.semantic_state.semantic_handles().empty_jsx_object_type;
        let mut has_spread_any_type = false;
        let mut type_to_intersect = None;
        let mut explicitly_specify_children_attribute = false;
        let mut object_flags = OBJECT_FLAGS_JSX_ATTRIBUTES;
        let jsx_namespace = self.get_jsx_namespace_at(Some(opening_like_element));
        let jsx_children_property_name = self.get_jsx_element_children_property_name(jsx_namespace);
        let is_jsx_open_fragment = ast::is_jsx_opening_fragment(
            self.store_for_node(opening_like_element),
            opening_like_element,
        );
        if !is_jsx_open_fragment {
            let attributes = self
                .store_for_node(opening_like_element)
                .attributes(opening_like_element)
                .unwrap();
            attributes_symbol_identity = self
                .node_symbol(attributes)
                .map(|symbol| self.symbol_handle_identity(symbol));
            attribute_parent = attributes;
            let contextual_type = self.get_contextual_type(attributes, CONTEXT_FLAGS_NONE);
            // Create anonymous type from given attributes symbol table.
            // @param symbol a symbol of JsxAttributes containing attributes corresponding to attributesTable
            // @param attributesTable a symbol table of attributes property
            for attribute_decl in self
                .store_for_node(attributes)
                .properties(attributes)
                .unwrap()
                .iter()
            {
                let attribute_decl = attribute_decl;
                if ast::is_jsx_attribute(self.store_for_node(attribute_decl), attribute_decl) {
                    let member = self.node_symbol(attribute_decl).unwrap();
                    let expr_type = self.check_jsx_attribute(attribute_decl, check_mode);
                    object_flags |= self.object_flags(expr_type) & OBJECT_FLAGS_PROPAGATING_FLAGS;
                    let attribute_symbol = self.new_symbol(
                        ast::SYMBOL_FLAGS_PROPERTY | self.symbol_handle_flags(member),
                        self.symbol_handle_name(member),
                    );
                    let attribute_symbol = self.transient_symbol_handle(attribute_symbol);
                    self.set_transient_symbol_declarations(
                        attribute_symbol,
                        self.collect_symbol_handle_declarations(member),
                    );
                    if let Some(value_declaration) = self.symbol_handle_value_declaration(member) {
                        self.set_transient_symbol_value_declaration(
                            attribute_symbol,
                            Some(value_declaration),
                        );
                    }
                    let attribute_symbol_name =
                        self.symbol_handle_name(attribute_symbol).to_string();
                    self.semantic_state
                        .set_value_symbol_resolved_type(attribute_symbol.clone(), Some(expr_type));
                    let member_identity = self.symbol_handle_identity(member);
                    self.set_value_symbol_target(attribute_symbol.clone(), Some(member_identity));
                    let attribute_symbol_identity = self.symbol_handle_identity(attribute_symbol);
                    attributes_table.insert(
                        attribute_symbol_name.clone().into(),
                        attribute_symbol_identity,
                    );
                    if let Some(all_attributes_table) = all_attributes_table.as_mut() {
                        all_attributes_table
                            .insert(attribute_symbol_name.into(), attribute_symbol_identity);
                    }
                    let attribute_name = self.node_name(attribute_decl).unwrap();
                    let attribute_name_text = self.node_text(attribute_name);
                    if attribute_name_text == jsx_children_property_name {
                        explicitly_specify_children_attribute = true;
                    }
                    if let Some(contextual_type) = contextual_type {
                        let member_name = self.symbol_handle_name(member);
                        let prop = self.get_property_of_type(contextual_type, &member_name);
                        if let Some(prop) = prop
                            && ast::is_identifier(
                                self.store_for_node(attribute_name),
                                attribute_name,
                            )
                            && !self.symbol_identity_declarations_are_empty(prop)
                        {
                            let parent = self.symbol_identity_parent(prop);
                            let declaration_count = self.symbol_identity_declaration_count(prop);
                            let parent_is_interface = parent.is_some_and(|parent| {
                                self.missing_name_symbol_identity_flags(parent)
                                    .intersects(ast::SYMBOL_FLAGS_INTERFACE)
                            });
                            let is_deprecated =
                                if parent.is_some() && declaration_count > 1 && parent_is_interface
                                {
                                    self.any_symbol_identity_declaration(prop, |checker, d| {
                                        checker.is_deprecated_declaration(d)
                                    })
                                } else if parent.is_some() && declaration_count > 1 {
                                    !self.any_symbol_identity_declaration(prop, |checker, d| {
                                        !checker.is_deprecated_declaration(d)
                                    })
                                } else {
                                    self.missing_name_symbol_identity_value_declaration(prop)
                                        .as_ref()
                                        .is_some_and(|d| self.is_deprecated_declaration(*d))
                                        || declaration_count != 0
                                            && !self.any_symbol_identity_declaration(
                                                prop,
                                                |checker, d| !checker.is_deprecated_declaration(d),
                                            )
                                };
                            if is_deprecated {
                                let declarations = self.collect_symbol_identity_declarations(prop);
                                self.add_deprecated_suggestion(
                                    attribute_name,
                                    &declarations,
                                    &attribute_name_text,
                                );
                            }
                        }
                    }
                    if contextual_type.is_some()
                        && check_mode & CHECK_MODE_INFERENTIAL != 0
                        && check_mode & CHECK_MODE_SKIP_CONTEXT_SENSITIVE == 0
                        && self.is_context_sensitive(attribute_decl)
                    {
                        let inference_context = self.get_inference_context(attributes).unwrap();
                        // In CheckMode.Inferential we should always have an inference context
                        let initializer = self.node_initializer(attribute_decl).unwrap();
                        let inference_node = self.node_expression(initializer).unwrap();
                        self.inference_context_record_mut(inference_context)
                            .intra_expression_inference_sites
                            .push(IntraExpressionInferenceSite {
                                node: inference_node,
                                t: expr_type,
                            });
                    }
                } else {
                    debug_assert!(
                        self.store_for_node(attribute_decl).kind(attribute_decl)
                            == ast::KIND_JSX_SPREAD_ATTRIBUTE
                    );
                    if !attributes_table.is_empty() {
                        let attr_type = self.create_jsx_attributes_object(
                            attributes_symbol_identity,
                            std::mem::take(&mut attributes_table),
                            &mut object_flags,
                        );
                        spread = self.get_spread_type(
                            spread,
                            attr_type,
                            attributes_symbol_identity,
                            object_flags,
                            false, /*readonly*/
                        );
                        attributes_table = SymbolIdentityTable::default();
                    }
                    let spread_expression = self.node_expression(attribute_decl).unwrap();
                    let checked_expression = self.check_expression_ex(
                        spread_expression,
                        check_mode & CHECK_MODE_INFERENTIAL,
                    );
                    let expr_type = self.get_reduced_type(checked_expression);
                    if is_type_any(self, Some(expr_type)) {
                        has_spread_any_type = true;
                    }
                    if self.is_valid_spread_type(expr_type) {
                        spread = self.get_spread_type(
                            spread,
                            expr_type,
                            attributes_symbol_identity,
                            object_flags,
                            false, /*readonly*/
                        );
                        if let Some(all_attributes_table) = all_attributes_table.as_ref() {
                            self.check_spread_prop_overrides_for_jsx_attributes(
                                expr_type,
                                all_attributes_table,
                                attribute_decl,
                            );
                        }
                    } else {
                        self.error(
                            Some(spread_expression),
                            &diagnostics::SPREAD_TYPES_MAY_ONLY_BE_CREATED_FROM_OBJECT_TYPES,
                            (),
                        );
                        type_to_intersect =
                            Some(if let Some(type_to_intersect) = type_to_intersect {
                                self.get_intersection_type(vec![type_to_intersect, expr_type])
                            } else {
                                expr_type
                            });
                    }
                }
            }
            if !has_spread_any_type && !attributes_table.is_empty() {
                let attr_type = self.create_jsx_attributes_object(
                    attributes_symbol_identity,
                    std::mem::take(&mut attributes_table),
                    &mut object_flags,
                );
                spread = self.get_spread_type(
                    spread,
                    attr_type,
                    attributes_symbol_identity,
                    object_flags,
                    false, /*readonly*/
                );
            }
        }
        if self.parent_has_semantic_jsx_children(opening_like_element) {
            let parent = self.node_parent(opening_like_element).unwrap();
            let child_types = self.check_jsx_children(parent, check_mode);
            if !has_spread_any_type
                && jsx_children_property_name != ast::INTERNAL_SYMBOL_NAME_MISSING
                && jsx_children_property_name != ""
            {
                // Error if there is a attribute named "children" explicitly specified and children element.
                // This is because children element will overwrite the value from attributes.
                // Note: we will not warn "children" attribute overwritten if "children" attribute is specified in object spread.
                if explicitly_specify_children_attribute {
                    self.error(
                        Some(attribute_parent),
                        &diagnostics::X_0_ARE_SPECIFIED_TWICE_THE_ATTRIBUTE_NAMED_0_WILL_BE_OVERWRITTEN,
                        jsx_children_property_name.clone(),
                    );
                }
                let mut children_contextual_type = None;
                if ast::is_jsx_opening_element(
                    self.store_for_node(opening_like_element),
                    opening_like_element,
                ) {
                    if let Some(contextual_type) = self.get_apparent_type_of_contextual_type(
                        self.store_for_node(opening_like_element)
                            .attributes(opening_like_element)
                            .unwrap(),
                        CONTEXT_FLAGS_NONE,
                    ) {
                        children_contextual_type = self.get_type_of_property_of_contextual_type(
                            contextual_type,
                            &jsx_children_property_name,
                        );
                    }
                }
                // If there are children in the body of JSX element, create dummy attribute "children" with the union of children types so that it will pass the attribute checking process
                let children_prop_symbol = self.new_symbol(
                    ast::SYMBOL_FLAGS_PROPERTY,
                    jsx_children_property_name.to_string(),
                );
                let children_prop_symbol = self.transient_symbol_handle(children_prop_symbol);
                let children_type = match child_types.len() {
                    1 => child_types[0],
                    _ if children_contextual_type.is_some()
                        && self.some_type_is_tuple_like(children_contextual_type.unwrap()) =>
                    {
                        self.create_tuple_type(child_types.clone())
                    }
                    _ => {
                        let union_type = self.get_union_type(child_types.clone());
                        self.create_array_type(union_type)
                    }
                };
                self.semantic_state.set_value_symbol_resolved_type(
                    children_prop_symbol.clone(),
                    Some(children_type),
                );
                // Fake up a property declaration for the children
                {
                    let name = self
                        .factory_mut()
                        .new_identifier(jsx_children_property_name.clone());
                    let value_declaration = self.factory_mut().new_property_signature_declaration(
                        None, name, None, /*postfixToken*/
                        None, /*type*/
                        None, /*initializer*/
                    );
                    self.factory_mut()
                        .link_checker_synthetic_parent(name, Some(value_declaration));
                    self.factory_mut()
                        .link_checker_synthetic_parent(value_declaration, Some(attribute_parent));
                    self.semantic_state.set_synthetic_node_symbol_identity(
                        value_declaration,
                        self.symbol_handle_identity(children_prop_symbol),
                    );

                    self.set_transient_symbol_value_declaration(
                        children_prop_symbol,
                        Some(value_declaration),
                    );
                }
                let mut child_prop_map = SymbolIdentityTable::default();
                child_prop_map.insert(
                    jsx_children_property_name.into(),
                    self.symbol_handle_identity(children_prop_symbol),
                );
                let child_attributes_type = self.new_anonymous_type_from_identities(
                    attributes_symbol_identity,
                    child_prop_map,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                );
                let child_object_flags = object_flags
                    | self.get_propagating_flags_of_types(&child_types, TYPE_FLAGS_NONE);
                spread = self.get_spread_type(
                    spread,
                    child_attributes_type,
                    attributes_symbol_identity,
                    child_object_flags,
                    false, /*readonly*/
                );
            }
        }
        if has_spread_any_type {
            return self.semantic_state.semantic_handles().any_type;
        }
        if let Some(type_to_intersect) = type_to_intersect {
            if spread != self.semantic_state.semantic_handles().empty_jsx_object_type {
                return self.get_intersection_type(vec![type_to_intersect, spread]);
            }
            return type_to_intersect;
        }
        if spread == self.semantic_state.semantic_handles().empty_jsx_object_type {
            object_flags |= OBJECT_FLAGS_FRESH_LITERAL;
            let result = self.new_object_type_from_identity(
                OBJECT_FLAGS_ANONYMOUS
                    | object_flags
                    | OBJECT_FLAGS_OBJECT_LITERAL
                    | OBJECT_FLAGS_CONTAINS_OBJECT_OR_ARRAY_LITERAL,
                attributes_symbol_identity,
            );
            let members = std::mem::take(&mut attributes_table);
            let properties = members.values().copied().collect();
            self.set_structured_type_member_identities(
                result,
                members,
                properties,
                Vec::new(),
                0,
                Vec::new(),
            );
            return result;
        }
        spread
    }

    fn create_jsx_attributes_object(
        &mut self,
        attributes_symbol: Option<SymbolIdentity>,
        attributes_table: SymbolIdentityTable,
        object_flags: &mut ObjectFlags,
    ) -> TypeHandle {
        *object_flags |= OBJECT_FLAGS_FRESH_LITERAL;
        let result = self.new_object_type_from_identity(
            OBJECT_FLAGS_ANONYMOUS
                | *object_flags
                | OBJECT_FLAGS_OBJECT_LITERAL
                | OBJECT_FLAGS_CONTAINS_OBJECT_OR_ARRAY_LITERAL,
            attributes_symbol,
        );
        let properties = attributes_table.values().copied().collect();
        self.set_structured_type_member_identities(
            result,
            attributes_table,
            properties,
            Vec::new(),
            0,
            Vec::new(),
        );
        result
    }

    fn some_type_is_tuple_like(&mut self, t: TypeHandle) -> bool {
        if self.type_flags(t) & TYPE_FLAGS_UNION != 0 {
            for ty in self.type_types(t) {
                if self.is_tuple_like_type(ty) {
                    return true;
                }
            }
            return false;
        }
        self.is_tuple_like_type(t)
    }

    fn parent_has_semantic_jsx_children(&mut self, opening_like_element: ast::Node) -> bool {
        // Handle children attribute
        let Some(parent) = self.node_parent(opening_like_element) else {
            return false;
        };
        let mut children = Vec::new();
        if ast::is_jsx_element(self.store_for_node(parent), parent) {
            // We have to check that openingElement of the parent is the one we are visiting as this may not be true for selfClosingElement
            let store = self.store_for_node(parent);
            if store
                .opening_element(parent)
                .is_some_and(|opening| opening == opening_like_element)
            {
                children = self
                    .store_for_node(parent)
                    .children(parent)
                    .unwrap()
                    .into_iter()
                    .collect();
            }
        } else if ast::is_jsx_fragment(self.store_for_node(parent), parent) {
            let store = self.store_for_node(parent);
            if store
                .opening_fragment(parent)
                .is_some_and(|opening| opening == opening_like_element)
            {
                children = self
                    .store_for_node(parent)
                    .children(parent)
                    .unwrap()
                    .into_iter()
                    .collect();
            }
        }
        !ast::get_semantic_jsx_children(self.store_for_node(parent), &children).is_empty()
    }

    pub(crate) fn check_jsx_attribute(
        &mut self,
        node: ast::Node,
        check_mode: CheckMode,
    ) -> TypeHandle {
        if let Some(initializer) = self.node_initializer(node) {
            return self.check_expression_for_mutable_location(initializer, check_mode);
        }
        // <Elem attr /> is sugar for <Elem attr={true} />
        self.semantic_state.semantic_handles().true_type
    }

    fn check_jsx_children(&mut self, node: ast::Node, check_mode: CheckMode) -> Vec<TypeHandle> {
        let mut child_types = Vec::new();
        for child in self.store_for_node(node).children(node).unwrap().iter() {
            let child = child;
            // In React, JSX text that contains only whitespaces will be ignored so we don't want to type-check that
            // because then type of children property will have constituent of string type.
            if ast::is_jsx_text(self.store_for_node(child), child) {
                if !self
                    .store_for_node(child)
                    .contains_only_trivia_white_spaces(child)
                    .unwrap_or(false)
                {
                    child_types.push(self.semantic_state.semantic_handles().string_type);
                }
            } else if ast::is_jsx_expression(self.store_for_node(child), child)
                && self.node_expression(child).is_none()
            {
                // empty jsx expressions don't *really* count as present children
                continue;
            } else {
                child_types.push(self.check_expression_for_mutable_location(child, check_mode));
            }
        }
        child_types
    }

    fn get_uninstantiated_jsx_signatures_of_type(
        &mut self,
        element_type: TypeHandle,
        caller: ast::Node,
    ) -> Vec<SignatureHandle> {
        if self.type_flags(element_type) & TYPE_FLAGS_STRING != 0 {
            return vec![self.semantic_state.semantic_handles().any_signature];
        }
        if self.type_flags(element_type) & TYPE_FLAGS_STRING_LITERAL != 0 {
            let intrinsic_type =
                self.get_intrinsic_attributes_type_from_string_literal_type(element_type, caller);
            if intrinsic_type.is_none() {
                self.error(
                    Some(caller),
                    &diagnostics::PROPERTY_0_DOES_NOT_EXIST_ON_TYPE_1,
                    Vec::<DiagnosticArg>::from([
                        self.get_string_literal_value(element_type).into(),
                        format!("JSX.{}", JsxNames::INTRINSIC_ELEMENTS).into(),
                    ]),
                );
                return Vec::new();
            }
            let fake_signature =
                self.create_signature_for_jsx_intrinsic(caller, intrinsic_type.unwrap());
            return vec![fake_signature];
        }
        let apparent_elem_type = self.get_apparent_type(element_type);
        // Resolve the signatures, preferring constructor
        let mut signatures =
            self.get_signatures_of_type(apparent_elem_type, SIGNATURE_KIND_CONSTRUCT);
        if signatures.is_empty() {
            // No construct signatures, try call signatures
            signatures = self.get_signatures_of_type(apparent_elem_type, SIGNATURE_KIND_CALL);
        }
        if signatures.is_empty() && self.type_flags(apparent_elem_type) & TYPE_FLAGS_UNION != 0 {
            // If each member has some combination of new/call signatures; make a union signature list for those
            let mut signature_lists = Vec::new();
            // PORT NOTE: reshaped for borrowck. This is TS-Go's map over union
            // constituents, preserving constituent order.
            for t in self.type_types(apparent_elem_type) {
                signature_lists.push(self.get_uninstantiated_jsx_signatures_of_type(t, caller));
            }
            signatures = self.get_union_signatures(signature_lists);
        }
        signatures
    }

    pub(crate) fn get_effective_first_argument_for_jsx_signature(
        &mut self,
        signature: SignatureHandle,
        node: ast::Node,
    ) -> TypeHandle {
        if ast::is_jsx_opening_fragment(self.store_for_node(node), node)
            || !matches!(
                self.get_jsx_reference_kind(node),
                JsxReferenceKind::Component
            )
        {
            return self.get_jsx_props_type_from_call_signature(signature, node);
        }
        self.get_jsx_props_type_from_class_type(signature, node)
    }

    fn get_jsx_props_type_from_call_signature(
        &mut self,
        sig: SignatureHandle,
        context: ast::Node,
    ) -> TypeHandle {
        let mut props_type = self.get_type_of_first_parameter_of_signature_with_fallback(
            sig,
            self.semantic_state.semantic_handles().unknown_type,
        );
        let ns = self.get_jsx_namespace_at(Some(context));
        props_type =
            self.get_jsx_managed_attributes_from_located_attributes(context, ns, props_type);
        let intrinsic_attribs = self.get_jsx_type(JsxNames::INTRINSIC_ATTRIBUTES, context);
        if !self.is_error_type(intrinsic_attribs) {
            props_type = self
                .intersect_types(Some(intrinsic_attribs), Some(props_type))
                .unwrap();
        }
        props_type
    }

    fn get_jsx_props_type_from_class_type(
        &mut self,
        sig: SignatureHandle,
        context: ast::Node,
    ) -> TypeHandle {
        let ns = self.get_jsx_namespace_at(Some(context));
        let forced_lookup_location = self.get_jsx_element_properties_name(ns.clone());
        let attributes_type = match forced_lookup_location.as_str() {
            ast::INTERNAL_SYMBOL_NAME_MISSING => {
                Some(self.get_type_of_first_parameter_of_signature_with_fallback(
                    sig,
                    self.semantic_state.semantic_handles().unknown_type,
                ))
            }
            "" => Some(self.get_return_type_of_signature(sig)),
            _ => {
                let attributes_type =
                    self.get_jsx_props_type_for_signature_from_member(sig, &forced_lookup_location);
                if attributes_type.is_none() && {
                    let store = self.store_for_node(context);
                    store
                        .properties(store.attributes(context).unwrap())
                        .is_some_and(|properties| !properties.is_empty())
                } {
                    // There is no property named 'props' on this instance type
                    self.error(
                        Some(context),
                        &diagnostics::JSX_ELEMENT_CLASS_DOES_NOT_SUPPORT_ATTRIBUTES_BECAUSE_IT_DOES_NOT_HAVE_A_0_PROPERTY,
                        forced_lookup_location.clone(),
                    );
                }
                attributes_type
            }
        };
        let Some(mut attributes_type) = attributes_type else {
            return self.semantic_state.semantic_handles().unknown_type;
        };
        attributes_type =
            self.get_jsx_managed_attributes_from_located_attributes(context, ns, attributes_type);
        if is_type_any(self, Some(attributes_type)) {
            // Props is of type 'any' or unknown
            return attributes_type;
        }
        // Normal case -- add in IntrinsicClassAttributes<T> and IntrinsicAttributes
        let mut apparent_attributes_type = attributes_type;
        let intrinsic_class_attribs =
            self.get_jsx_type(JsxNames::INTRINSIC_CLASS_ATTRIBUTES, context);
        if !self.is_error_type(intrinsic_class_attribs) {
            let type_param_count = if self
                .type_record(intrinsic_class_attribs)
                .as_interface_type()
                .is_some()
            {
                self.interface_type_parameter_count(intrinsic_class_attribs)
            } else {
                0
            };
            let host_class_type = self.get_return_type_of_signature(sig);
            let library_managed_attribute_type = if type_param_count != 0 {
                // apply JSX.IntrinsicClassAttributes<hostClassType, ...>
                let type_params = self.interface_type_parameters(intrinsic_class_attribs);
                let min_type_argument_count = self.get_min_type_argument_count(&type_params);
                let inferred_args = self.fill_missing_type_arguments(
                    vec![host_class_type],
                    &type_params,
                    min_type_argument_count,
                    ast::is_in_js_file(self.store_for_node(context), context),
                );
                let mapper = self.new_type_mapper_handle(type_params, inferred_args);
                self.instantiate_type_with_mapper_handle(
                    Some(intrinsic_class_attribs),
                    Some(mapper),
                )
                .unwrap()
            } else {
                intrinsic_class_attribs
            };
            apparent_attributes_type = self
                .intersect_types(
                    Some(library_managed_attribute_type),
                    Some(apparent_attributes_type),
                )
                .unwrap();
        }
        let intrinsic_attribs = self.get_jsx_type(JsxNames::INTRINSIC_ATTRIBUTES, context);
        if !self.is_error_type(intrinsic_attribs) {
            apparent_attributes_type = self
                .intersect_types(Some(intrinsic_attribs), Some(apparent_attributes_type))
                .unwrap();
        }
        apparent_attributes_type
    }

    fn get_jsx_props_type_for_signature_from_member(
        &mut self,
        sig: SignatureHandle,
        forced_lookup_location: &str,
    ) -> Option<TypeHandle> {
        if let Some(composite) = self.signature_record(sig).composite.clone() {
            // JSX Elements using the legacy `props`-field based lookup (eg, react class components) need to treat the `props` member as an input
            // instead of an output position when resolving the signature. We need to go back to the input signatures of the composite signature,
            // get the type of `props` on each return type individually, and then _intersect them_, rather than union them (as would normally occur
            // for a union signature). It's an unfortunate quirk of looking in the output of the signature for the type we want to use for the input.
            // The default behavior of `getTypeOfFirstParameterOfSignatureWithFallback` when no `props` member name is defined is much more sane.
            let mut results = Vec::new();
            for signature in composite.signatures.clone() {
                let instance = self.get_return_type_of_signature(signature);
                if is_type_any(self, Some(instance)) {
                    return Some(instance);
                }
                let prop_type = self.get_type_of_property_of_type(instance, forced_lookup_location);
                prop_type?;
                results.push(prop_type.unwrap());
            }
            return Some(self.get_intersection_type(results));
            // Same result for both union and intersection signatures
        }
        let instance_type = self.get_return_type_of_signature(sig);
        if is_type_any(self, Some(instance_type)) {
            return Some(instance_type);
        }
        self.get_type_of_property_of_type(instance_type, forced_lookup_location)
    }

    fn get_jsx_managed_attributes_from_located_attributes(
        &mut self,
        context: ast::Node,
        ns: Option<SymbolIdentity>,
        attributes_type: TypeHandle,
    ) -> TypeHandle {
        let managed_sym = self.get_jsx_library_managed_attributes(ns);
        if let Some(managed_sym) = managed_sym {
            let ctor_type = self.get_static_type_of_referenced_jsx_constructor(context);
            let result = self.instantiate_alias_or_interface_with_defaults(
                managed_sym,
                vec![ctor_type, attributes_type],
                ast::is_in_js_file(self.store_for_node(context), context),
            );
            if let Some(result) = result {
                return result;
            }
        }
        attributes_type
    }

    fn instantiate_alias_or_interface_with_defaults(
        &mut self,
        managed_sym: SymbolIdentity,
        type_arguments: Vec<TypeHandle>,
        in_java_script: bool,
    ) -> Option<TypeHandle> {
        // fetches interface type, or initializes symbol links type parameters
        let declared_managed_type = self.get_declared_type_of_symbol_identity_or_error(managed_sym);
        let managed_sym_flags = self.missing_name_symbol_identity_flags(managed_sym);
        if managed_sym_flags & ast::SYMBOL_FLAGS_TYPE_ALIAS != 0 {
            let params = self.semantic_state.type_alias_type_parameters(managed_sym);
            if params.len() >= type_arguments.len() {
                let type_argument_count = type_arguments.len();
                let args = self.fill_missing_type_arguments(
                    type_arguments.clone(),
                    &params,
                    type_argument_count,
                    in_java_script,
                );
                if args.is_empty() {
                    return Some(declared_managed_type);
                }
                let managed_handle = managed_sym.symbol_handle();
                return Some(self.get_type_alias_instantiation_handle(managed_handle, args, None));
            }
        }
        let type_parameter_count = self.interface_type_parameter_count(declared_managed_type);
        if type_parameter_count >= type_arguments.len() {
            let type_parameters = self.interface_type_parameters(declared_managed_type);
            let type_argument_count = type_arguments.len();
            let args = self.fill_missing_type_arguments(
                type_arguments.clone(),
                &type_parameters,
                type_argument_count,
                in_java_script,
            );
            return Some(self.create_type_reference(declared_managed_type, args));
        }
        None
    }

    fn get_jsx_library_managed_attributes(
        &mut self,
        jsx_namespace: Option<SymbolIdentity>,
    ) -> Option<SymbolIdentity> {
        if let Some(jsx_namespace) = jsx_namespace {
            return self.get_export_of_symbol_identity_by_meaning(
                jsx_namespace,
                JsxNames::LIBRARY_MANAGED_ATTRIBUTES,
                ast::SYMBOL_FLAGS_TYPE,
            );
        }
        None
    }

    fn get_jsx_element_type_symbol(
        &mut self,
        jsx_namespace: Option<SymbolIdentity>,
    ) -> Option<SymbolIdentity> {
        // JSX.ElementType [symbol]
        if let Some(jsx_namespace) = jsx_namespace {
            return self.get_export_of_symbol_identity_by_meaning(
                jsx_namespace,
                JsxNames::ELEMENT_TYPE,
                ast::SYMBOL_FLAGS_TYPE,
            );
        }
        None
    }

    // e.g. "props" for React.d.ts,
    // or InternalSymbolNameMissing if ElementAttributesProperty doesn't exist (which means all
    //
    //	non-intrinsic elements' attributes type is 'any'),
    //
    // or "" if it has 0 properties (which means every
    //
    //	non-intrinsic elements' attributes type is the element instance type)
    fn get_jsx_element_properties_name(&mut self, jsx_namespace: Option<SymbolIdentity>) -> String {
        self.get_name_from_jsx_element_attributes_container(
            JsxNames::ELEMENT_ATTRIBUTES_PROPERTY_NAME_CONTAINER,
            jsx_namespace,
        )
    }

    fn get_jsx_element_children_property_name(
        &mut self,
        jsx_namespace: Option<SymbolIdentity>,
    ) -> String {
        if self.compiler_options.jsx == core::JSX_EMIT_REACT_JSX
            || self.compiler_options.jsx == core::JSX_EMIT_REACT_JSX_DEV
        {
            // In these JsxEmit modes the children property is fixed to 'children'
            return "children".to_string();
        }
        self.get_name_from_jsx_element_attributes_container(
            JsxNames::ELEMENT_CHILDREN_ATTRIBUTE_NAME_CONTAINER,
            jsx_namespace,
        )
    }

    // Look into JSX namespace and then look for container with matching name as nameOfAttribPropContainer.
    // Get a single property from that container if existed. Report an error if there are more than one property.
    //
    // @param nameOfAttribPropContainer a string of value JsxNames.ElementAttributesPropertyNameContainer or JsxNames.ElementChildrenAttributeNameContainer
    //
    //	if other string is given or the container doesn't exist, return undefined.
    fn get_name_from_jsx_element_attributes_container(
        &mut self,
        name_of_attrib_prop_container: &str,
        jsx_namespace: Option<SymbolIdentity>,
    ) -> String {
        // JSX.ElementAttributesProperty | JSX.ElementChildrenAttribute [symbol]
        if let Some(jsx_namespace) = jsx_namespace {
            let jsx_element_attrib_prop_interface_sym = self
                .get_export_of_symbol_identity_by_meaning(
                    jsx_namespace,
                    name_of_attrib_prop_container,
                    ast::SYMBOL_FLAGS_TYPE,
                );
            if let Some(jsx_element_attrib_prop_interface_sym) =
                jsx_element_attrib_prop_interface_sym
            {
                let jsx_element_attrib_prop_interface_type = self
                    .get_declared_type_of_symbol_identity_or_error(
                        jsx_element_attrib_prop_interface_sym,
                    );
                let properties_of_jsx_element_attrib_prop_interface =
                    self.get_properties_of_type(jsx_element_attrib_prop_interface_type);
                // Element Attributes has zero properties, so the element attributes type will be the class instance type
                if properties_of_jsx_element_attrib_prop_interface.is_empty() {
                    return String::new();
                }
                if properties_of_jsx_element_attrib_prop_interface.len() == 1 {
                    return self.missing_name_symbol_identity_name(
                        properties_of_jsx_element_attrib_prop_interface[0],
                    );
                }
                if properties_of_jsx_element_attrib_prop_interface.len() > 1
                    && let Some(declaration) = self
                        .first_symbol_identity_declaration(jsx_element_attrib_prop_interface_sym)
                {
                    // More than one property on ElementAttributesProperty is an error
                    self.error(
                        Some(declaration),
                        &diagnostics::THE_GLOBAL_TYPE_JSX_0_MAY_NOT_HAVE_MORE_THAN_ONE_PROPERTY,
                        name_of_attrib_prop_container.to_string(),
                    );
                }
            }
        }
        ast::INTERNAL_SYMBOL_NAME_MISSING.to_string()
    }

    fn get_static_type_of_referenced_jsx_constructor(&mut self, context: ast::Node) -> TypeHandle {
        if ast::is_jsx_opening_fragment(self.store_for_node(context), context) {
            return self.get_jsx_fragment_type(context);
        }
        let tag_name = self.store_for_node(context).tag_name(context).unwrap();
        if is_jsx_intrinsic_tag_name(self.store_for_node(tag_name), tag_name) {
            let result = self.get_intrinsic_attributes_type_from_jsx_opening_like_element(context);
            let fake_signature = self.create_signature_for_jsx_intrinsic(context, result);
            return self.get_or_create_type_from_signature(fake_signature);
        }
        let tag_type = self.check_expression_cached(tag_name);
        if self.type_flags(tag_type) & TYPE_FLAGS_STRING_LITERAL != 0 {
            let result =
                self.get_intrinsic_attributes_type_from_string_literal_type(tag_type, context);
            if result.is_none() {
                return self.semantic_state.semantic_handles().error_type;
            }
            let fake_signature = self.create_signature_for_jsx_intrinsic(context, result.unwrap());
            return self.get_or_create_type_from_signature(fake_signature);
        }
        tag_type
    }

    fn get_intrinsic_attributes_type_from_string_literal_type(
        &mut self,
        t: TypeHandle,
        location: ast::Node,
    ) -> Option<TypeHandle> {
        // If the elemType is a stringLiteral type, we can then provide a check to make sure that the string literal type is one of the Jsx intrinsic element type
        // For example:
        //      var CustomTag: "h1" = "h1";
        //      <CustomTag> Hello World </CustomTag>
        let intrinsic_elements_type = self.get_jsx_type(JsxNames::INTRINSIC_ELEMENTS, location);
        if !self.is_error_type(intrinsic_elements_type) {
            let string_literal_type_name = self.get_string_literal_value(t);
            let intrinsic_prop =
                self.get_property_of_type(intrinsic_elements_type, &string_literal_type_name);
            if let Some(intrinsic_prop) = intrinsic_prop {
                return Some(self.get_type_of_symbol_identity_at_location(intrinsic_prop, None));
            }
            let index_signature_type = self.get_index_type_of_type(
                intrinsic_elements_type,
                self.semantic_state.semantic_handles().string_type,
            );
            if index_signature_type.is_some() {
                return index_signature_type;
            }
            return None;
        }
        // If we need to report an error, we already done so here. So just return any to prevent any more error downstream
        Some(self.semantic_state.semantic_handles().any_type)
    }

    fn get_jsx_reference_kind(&mut self, node: ast::Node) -> JsxReferenceKind {
        let tag_name = self.store_for_node(node).tag_name(node).unwrap();
        if is_jsx_intrinsic_tag_name(self.store_for_node(tag_name), tag_name) {
            return JsxReferenceKind::Mixed;
        }
        let checked_tag_type = self.check_expression(tag_name);
        let tag_type = self.get_apparent_type(checked_tag_type);
        if !self
            .get_signatures_of_type(tag_type, SIGNATURE_KIND_CONSTRUCT)
            .is_empty()
        {
            return JsxReferenceKind::Component;
        }
        if !self
            .get_signatures_of_type(tag_type, SIGNATURE_KIND_CALL)
            .is_empty()
        {
            return JsxReferenceKind::Function;
        }
        JsxReferenceKind::Mixed
    }

    fn create_signature_for_jsx_intrinsic(
        &mut self,
        node: ast::Node,
        result: TypeHandle,
    ) -> SignatureHandle {
        let mut element_type = self.semantic_state.semantic_handles().error_type;
        if let Some(namespace) = self.get_jsx_namespace_at(Some(node)) {
            if let Some(type_symbol) = self.get_export_of_symbol_identity_by_meaning(
                namespace,
                JsxNames::ELEMENT,
                ast::SYMBOL_FLAGS_TYPE,
            ) {
                element_type = self.get_type_of_symbol_identity_at_location(type_symbol, None);
            }
        }
        // returnNode := typeSymbol && c.nodeBuilder.symbolToEntityName(typeSymbol, ast.SymbolFlagsType, node)
        // declaration := factory.createFunctionTypeNode(nil, []ParameterDeclaration{factory.createParameterDeclaration(nil, nil /*dotDotDotToken*/, "props", nil /*questionToken*/, c.nodeBuilder.typeToTypeNode(result, node))}, ifElse(returnNode != nil, factory.createTypeReferenceNode(returnNode, nil /*typeArguments*/), factory.createKeywordTypeNode(ast.KindAnyKeyword)))
        let parameter_symbol = self.new_symbol(
            ast::SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE,
            "props".to_string(),
        );
        let parameter_symbol = self.transient_symbol_handle(parameter_symbol);
        self.semantic_state
            .set_value_symbol_resolved_type(parameter_symbol.clone(), Some(result));
        self.new_signature_from_identities(
            SIGNATURE_FLAGS_NONE,
            None,
            Vec::new(),
            None,
            vec![self.symbol_handle_identity(parameter_symbol)],
            Some(element_type),
            None,
            1,
        )
    }

    // Get attributes type of the given intrinsic opening-like Jsx element by resolving the tag name.
    // The function is intended to be called from a function which has checked that the opening element is an intrinsic element.
    // @param node an intrinsic JSX opening-like element
    fn get_intrinsic_attributes_type_from_jsx_opening_like_element(
        &mut self,
        node: ast::Node,
    ) -> TypeHandle {
        let tag_name = self.store_for_node(node).tag_name(node).unwrap();
        debug_assert!(is_jsx_intrinsic_tag_name(
            self.store_for_node(tag_name),
            tag_name
        ));
        if let Some(resolved_jsx_element_attributes_type) = self
            .semantic_state
            .jsx_element_resolved_attributes_type(node)
        {
            return resolved_jsx_element_attributes_type;
        }
        let symbol = self.get_intrinsic_tag_symbol(node);
        if self.semantic_state.jsx_element_flags(node) & JSX_FLAGS_INTRINSIC_NAMED_ELEMENT != 0 {
            let resolved = self.get_type_of_symbol_identity_at_location(symbol, None);
            self.semantic_state
                .set_jsx_element_resolved_attributes_type(node, resolved);
            return resolved;
        }
        if self.semantic_state.jsx_element_flags(node) & JSX_FLAGS_INTRINSIC_INDEXED_ELEMENT != 0 {
            let intrinsic_elements_type = self.get_jsx_type(JsxNames::INTRINSIC_ELEMENTS, node);
            let tag_name_text = self.node_text(tag_name);
            let index_info =
                self.get_applicable_index_info_for_name(intrinsic_elements_type, &tag_name_text);
            if let Some(index_info) = index_info {
                let value_type = self.index_info_record(index_info).value_type.unwrap();
                self.semantic_state
                    .set_jsx_element_resolved_attributes_type(node, value_type);
                return value_type;
            }
        }
        self.semantic_state
            .set_jsx_element_resolved_attributes_type(
                node,
                self.semantic_state.semantic_handles().error_type,
            );
        self.semantic_state.semantic_handles().error_type
    }

    // Looks up an intrinsic tag name and returns a symbol that either points to an intrinsic
    // property (in which case nodeLinks.jsxFlags will be IntrinsicNamedElement) or an intrinsic
    // string index signature (in which case nodeLinks.jsxFlags will be IntrinsicIndexedElement).
    // May also return unknownSymbol if both of these lookups fail.
    pub(crate) fn get_intrinsic_tag_symbol(&mut self, node: ast::Node) -> SymbolIdentity {
        if let Some(resolved_symbol) = self.node_resolved_symbol_identity(node) {
            return resolved_symbol;
        }
        let intrinsic_elements_type = self.get_jsx_type(JsxNames::INTRINSIC_ELEMENTS, node);
        if !self.is_error_type(intrinsic_elements_type) {
            // Property case
            let tag_name = self.store_for_node(node).tag_name(node).unwrap();
            if !ast::is_identifier(self.store_for_node(tag_name), tag_name)
                && !ast::is_jsx_namespaced_name(self.store_for_node(tag_name), tag_name)
            {
                panic!("Invalid tag name");
            }
            let prop_name = self.node_text(tag_name);
            let intrinsic_prop = self.get_property_of_type(intrinsic_elements_type, &prop_name);
            if let Some(intrinsic_prop) = intrinsic_prop {
                self.semantic_state
                    .add_jsx_element_flags(node, JSX_FLAGS_INTRINSIC_NAMED_ELEMENT);
                self.set_node_resolved_symbol_identity(node, Some(intrinsic_prop));
                return intrinsic_prop;
            }
            // Intrinsic string indexer case
            let string_literal_type = self.get_string_literal_type(&prop_name);
            let index_symbol =
                self.get_applicable_index_symbol(intrinsic_elements_type, string_literal_type);
            if let Some(index_symbol) = index_symbol {
                self.semantic_state
                    .add_jsx_element_flags(node, JSX_FLAGS_INTRINSIC_INDEXED_ELEMENT);
                self.set_node_resolved_symbol_identity(node, Some(index_symbol));
                return index_symbol;
            }
            if self
                .get_type_of_property_or_index_signature_of_type(
                    intrinsic_elements_type,
                    &prop_name,
                )
                .is_some()
            {
                self.semantic_state
                    .add_jsx_element_flags(node, JSX_FLAGS_INTRINSIC_INDEXED_ELEMENT);
                let symbol_identity = self.type_symbol_identity(intrinsic_elements_type).unwrap();
                self.set_node_resolved_symbol_identity(node, Some(symbol_identity));
                return symbol_identity;
            }
            // Wasn't found
            self.error(
                Some(node),
                &diagnostics::PROPERTY_0_DOES_NOT_EXIST_ON_TYPE_1,
                Vec::<DiagnosticArg>::from([
                    prop_name.into(),
                    format!("JSX.{}", JsxNames::INTRINSIC_ELEMENTS).into(),
                ]),
            );
            let unknown_identity = self.unknown_symbol_identity();
            self.set_node_resolved_symbol_identity(node, Some(unknown_identity));
            return unknown_identity;
        }
        if self.no_implicit_any() {
            self.error(
                Some(node),
                &diagnostics::JSX_ELEMENT_IMPLICITLY_HAS_TYPE_ANY_BECAUSE_NO_INTERFACE_JSX_0_EXISTS,
                JsxNames::INTRINSIC_ELEMENTS.to_string(),
            );
        }
        let unknown_identity = self.unknown_symbol_identity();
        self.set_node_resolved_symbol_identity(node, Some(unknown_identity));
        unknown_identity
    }

    fn get_jsx_stateless_element_type_at(&mut self, location: ast::Node) -> Option<TypeHandle> {
        let jsx_element_type = self.get_jsx_element_type_at(location);
        if jsx_element_type.is_none() {
            return None;
        }
        Some(self.get_union_type(vec![
            jsx_element_type.unwrap(),
            self.semantic_state.semantic_handles().null_type,
        ]))
    }

    fn get_jsx_element_class_type_at(&mut self, location: ast::Node) -> Option<TypeHandle> {
        let t = self.get_jsx_type(JsxNames::ELEMENT_CLASS, location);
        if self.is_error_type(t) {
            return None;
        }
        Some(t)
    }

    fn get_jsx_element_type_at(&mut self, location: ast::Node) -> Option<TypeHandle> {
        Some(self.get_jsx_type(JsxNames::ELEMENT, location))
    }

    fn get_jsx_element_type_type_at(&mut self, location: ast::Node) -> Option<TypeHandle> {
        let ns = self.get_jsx_namespace_at(Some(location))?;
        let sym = self.get_jsx_element_type_symbol(Some(ns))?;
        let t = self.instantiate_alias_or_interface_with_defaults(
            sym,
            Vec::new(),
            ast::is_in_js_file(self.store_for_node(location), location),
        );
        if t.is_none() || self.is_error_type(t.unwrap()) {
            return None;
        }
        t
    }

    pub(crate) fn get_jsx_type(&mut self, name: &str, location: ast::Node) -> TypeHandle {
        if let Some(namespace) = self.get_jsx_namespace_at(Some(location)) {
            if let Some(type_symbol) = self.get_export_of_symbol_identity_by_meaning(
                namespace,
                name,
                ast::SYMBOL_FLAGS_TYPE,
            ) {
                return self.get_declared_type_of_symbol_identity_or_error(type_symbol);
            }
        }
        self.semantic_state.semantic_handles().error_type
    }

    fn get_jsx_namespace_at(&mut self, location: Option<ast::Node>) -> Option<SymbolIdentity> {
        let cached_namespace =
            location.and_then(|location| self.semantic_state.jsx_element_namespace(location));
        if let Some(cached_namespace) = cached_namespace {
            if !self.is_unknown_symbol_identity(cached_namespace) {
                return Some(cached_namespace);
            }
        }
        if location.is_none()
            || cached_namespace
                .is_none_or(|cached_namespace| !self.is_unknown_symbol_identity(cached_namespace))
        {
            let mut resolved_namespace =
                self.get_jsx_namespace_container_for_implicit_import(location);
            if resolved_namespace.is_none_or(|resolved_namespace| {
                self.is_unknown_symbol_identity(resolved_namespace)
            }) {
                let namespace_name = self.get_jsx_namespace(location);
                resolved_namespace = self
                    .resolve_name(
                        location,
                        &namespace_name,
                        ast::SYMBOL_FLAGS_NAMESPACE,
                        None,  /*nameNotFoundMessage*/
                        false, /*isUse*/
                        false, /*excludeGlobals*/
                    )
                    .map(SymbolIdentity::from_symbol_handle);
            }
            if let Some(resolved_namespace) = resolved_namespace {
                let resolved_namespace = self.resolve_symbol_identity(resolved_namespace, false);
                let candidate = self.get_export_of_symbol_identity_by_meaning(
                    resolved_namespace,
                    JsxNames::JSX,
                    ast::SYMBOL_FLAGS_NAMESPACE,
                );
                if let Some(candidate) = candidate {
                    let candidate = self.resolve_symbol_identity(candidate, false);
                    if !self.is_unknown_symbol_identity(candidate) {
                        if let Some(location) = location {
                            self.semantic_state
                                .set_jsx_element_namespace(location, candidate);
                        }
                        return Some(candidate);
                    }
                }
            }
            if let Some(location) = location {
                self.semantic_state
                    .set_jsx_element_namespace(location, self.unknown_symbol_identity());
            }
        }
        // JSX global fallback
        let s = self.get_global_symbol(
            JsxNames::JSX,
            ast::SYMBOL_FLAGS_NAMESPACE,
            None, /*diagnostic*/
        );
        if let Some(s) = s {
            let s = self.resolve_symbol_identity(s, false);
            if !self.is_unknown_symbol_identity(s) {
                return Some(s);
            }
        }
        None
    }

    pub(crate) fn get_jsx_namespace(&mut self, location: Option<ast::Node>) -> String {
        if let Some(location) = location {
            if let Some(file) = self.try_source_file_for_node(location) {
                if ast::is_jsx_opening_fragment(self.store_for_output_node(location), location) {
                    let local_jsx_fragment_namespace = self
                        .semantic_state
                        .source_file_local_jsx_fragment_namespace(file);
                    if local_jsx_fragment_namespace != "" {
                        return local_jsx_fragment_namespace;
                    }
                    let jsx_fragment_pragma =
                        ast::get_pragma_from_source_file(Some(file), "jsxfrag");
                    if let Some(jsx_fragment_pragma) = jsx_fragment_pragma {
                        let local_jsx_fragment_factory = self
                            .parse_isolated_entity_name(&jsx_fragment_pragma.args["factory"].value);
                        self.semantic_state
                            .set_source_file_local_jsx_fragment_factory(
                                file,
                                local_jsx_fragment_factory,
                            );
                        if let Some(local_jsx_fragment_factory) = local_jsx_fragment_factory {
                            let local_jsx_fragment_namespace =
                                self.factory_node_first_identifier_text(local_jsx_fragment_factory);
                            self.semantic_state
                                .set_source_file_local_jsx_fragment_namespace(
                                    file,
                                    local_jsx_fragment_namespace.clone(),
                                );
                            return local_jsx_fragment_namespace;
                        }
                    }
                    let entity = self.get_jsx_fragment_factory_entity(Some(location));
                    if let Some(entity) = entity {
                        let local_jsx_fragment_namespace =
                            self.factory_node_first_identifier_text(entity);
                        self.semantic_state
                            .set_source_file_local_jsx_fragment_factory(file, Some(entity));
                        self.semantic_state
                            .set_source_file_local_jsx_fragment_namespace(
                                file,
                                local_jsx_fragment_namespace.clone(),
                            );
                        return local_jsx_fragment_namespace;
                    }
                } else {
                    let local_jsx_namespace = self.get_local_jsx_namespace(file);
                    if local_jsx_namespace != "" {
                        self.semantic_state
                            .set_source_file_local_jsx_namespace(file, local_jsx_namespace.clone());
                        return local_jsx_namespace;
                    }
                }
            }
        }
        if self.jsx_namespace().is_empty() {
            self.set_jsx_namespace("React".to_string());
            if self.compiler_options.jsx_factory != "" {
                let jsx_factory_entity =
                    self.parse_isolated_entity_name(&self.compiler_options.jsx_factory.clone());
                self.set_jsx_factory_entity(jsx_factory_entity);
                if let Some(jsx_factory_entity) = self.jsx_factory_entity() {
                    let jsx_namespace = self.factory_node_first_identifier_text(jsx_factory_entity);
                    self.set_jsx_namespace(jsx_namespace);
                }
            } else if self.compiler_options.react_namespace != "" {
                self.set_jsx_namespace(self.compiler_options.react_namespace.clone());
            }
        }
        if self.jsx_factory_entity().is_none() {
            let jsx_namespace = self.jsx_namespace().to_string();
            let left = self.new_synthetic_identifier(&jsx_namespace);
            let right = self.new_synthetic_identifier("createElement");
            let jsx_factory_entity = self.new_synthetic_qualified_name(left, right);
            self.set_jsx_factory_entity(Some(jsx_factory_entity));
        }
        self.jsx_namespace().to_string()
    }

    fn get_local_jsx_namespace(&mut self, file: &ast::SourceFile) -> String {
        let local_jsx_namespace = self.semantic_state.source_file_local_jsx_namespace(file);
        if local_jsx_namespace != "" {
            return local_jsx_namespace;
        }
        let jsx_pragma = ast::get_pragma_from_source_file(Some(file), "jsx");
        if let Some(jsx_pragma) = jsx_pragma {
            let local_jsx_factory =
                self.parse_isolated_entity_name(&jsx_pragma.args["factory"].value);
            self.semantic_state
                .set_source_file_local_jsx_factory(file, local_jsx_factory);
            if let Some(local_jsx_factory) = local_jsx_factory {
                let local_jsx_namespace =
                    self.factory_node_first_identifier_text(local_jsx_factory);
                self.semantic_state
                    .set_source_file_local_jsx_namespace(file, local_jsx_namespace.clone());
                return local_jsx_namespace;
            }
        }
        String::new()
    }

    pub(crate) fn get_jsx_factory_entity(
        &mut self,
        location: Option<ast::Node>,
    ) -> Option<ast::Node> {
        if let Some(location) = location {
            self.get_jsx_namespace(Some(location));
            if let Some(file) = self.try_source_file_for_node(location)
                && let Some(local_jsx_factory) =
                    self.semantic_state.source_file_local_jsx_factory(file)
            {
                return Some(local_jsx_factory);
            }
        }
        self.jsx_factory_entity()
    }

    pub(crate) fn get_jsx_fragment_factory_entity(
        &mut self,
        location: Option<ast::Node>,
    ) -> Option<ast::Node> {
        if let Some(location) = location {
            if let Some(file) = self.try_source_file_for_node(location) {
                if let Some(local_jsx_fragment_factory) = self
                    .semantic_state
                    .source_file_local_jsx_fragment_factory(file)
                {
                    return Some(local_jsx_fragment_factory);
                }
                let jsx_frag_pragma = ast::get_pragma_from_source_file(Some(file), "jsxfrag");
                if let Some(jsx_frag_pragma) = jsx_frag_pragma {
                    let local_jsx_fragment_factory =
                        self.parse_isolated_entity_name(&jsx_frag_pragma.args["factory"].value);
                    self.semantic_state
                        .set_source_file_local_jsx_fragment_factory(
                            file,
                            local_jsx_fragment_factory,
                        );
                    return local_jsx_fragment_factory;
                }
            }
        }
        if self.compiler_options.jsx_fragment_factory != "" {
            let jsx_fragment_factory = self.compiler_options.jsx_fragment_factory.clone();
            return self.parse_isolated_entity_name(&jsx_fragment_factory);
        }
        None
    }

    fn parse_isolated_entity_name(&mut self, name: &str) -> Option<ast::Node> {
        let parsed = parser::parse_isolated_entity_name(name)?;
        Some(self.clone_entity_name_from_store(&parsed.store, parsed.node))
    }

    fn clone_entity_name_from_store(
        &mut self,
        source_store: &ast::AstStore,
        node: ast::Node,
    ) -> ast::Node {
        match source_store.kind(node) {
            ast::Kind::Identifier => self.new_synthetic_identifier(&source_store.text(node)),
            ast::Kind::QualifiedName => {
                let left = self.clone_entity_name_from_store(
                    source_store,
                    source_store.left(node).expect("qualified name left"),
                );
                let right = self.clone_entity_name_from_store(
                    source_store,
                    source_store.right(node).expect("qualified name right"),
                );
                self.new_synthetic_qualified_name(left, right)
            }
            _ => panic!("Unhandled case in clone_entity_name_from_store"),
        }
    }

    fn new_synthetic_identifier(&mut self, text: &str) -> ast::Node {
        let node = self.factory_mut().new_identifier(text);
        self.mark_factory_node_synthetic(node)
    }

    fn new_synthetic_qualified_name(&mut self, left: ast::Node, right: ast::Node) -> ast::Node {
        let node = self
            .factory_mut()
            .new_qualified_name(Some(left), Some(right));
        let node = self.mark_factory_node_synthetic(node);
        self.factory_mut().adopt_checker_synthetic_children(node);
        node
    }

    fn mark_factory_node_synthetic(&mut self, node: ast::Node) -> ast::Node {
        let loc = core::new_text_range(-1, -1);
        self.factory_mut().place_checker_synthetic_node(node, loc);
        node
    }

    fn factory_node_first_identifier_text(&self, node: ast::Node) -> String {
        let store = self.factory().store();
        let first_identifier = ast::get_first_identifier(store, node).unwrap();
        store.text(first_identifier)
    }

    pub(crate) fn get_jsx_namespace_container_for_implicit_import(
        &mut self,
        location: Option<ast::Node>,
    ) -> Option<SymbolIdentity> {
        let mut file = None;
        let mut file_node = None;
        if let Some(location) = location {
            if let Some(source_file) = self.try_source_file_for_node(location) {
                file = Some(source_file.share_readonly());
                file_node = Some(source_file.as_node());
            }
        }
        if let Some(file_node) = file_node {
            let jsx_implicit_import_container = self
                .semantic_state
                .jsx_element_implicit_import_container(file_node);
            if let Some(jsx_implicit_import_container) = jsx_implicit_import_container {
                return if self.is_unknown_symbol_identity(jsx_implicit_import_container) {
                    None
                } else {
                    Some(jsx_implicit_import_container)
                };
            }
        }
        let (module_reference, specifier) = self.get_jsx_runtime_import_specifier(file);
        if module_reference == "" {
            return None;
        }
        let error_message = &diagnostics::THIS_JSX_TAG_REQUIRES_THE_MODULE_PATH_0_TO_EXIST_BUT_NONE_COULD_BE_FOUND_MAKE_SURE_YOU_HAVE_TYPES_FOR_THE_APPROPRIATE_PACKAGE_INSTALLED;
        let Some(module_location) = specifier
            .filter(|specifier| self.try_source_file_for_node(*specifier).is_some())
            .or(location)
        else {
            return None;
        };
        let module = self.resolve_external_module(
            module_location,
            &module_reference,
            Some(error_message),
            location,
            false,
        );
        let mut result = None;
        if let Some(module) = module {
            if !self.is_unknown_symbol_identity(module) {
                let resolved_module = self.resolve_symbol_identity(module, false);
                result = self.get_merged_symbol_identity(Some(resolved_module));
            }
        }
        if let Some(file_node) = file_node {
            self.semantic_state
                .set_jsx_element_implicit_import_container(
                    file_node,
                    result.unwrap_or_else(|| self.unknown_symbol_identity()),
                );
        }
        result
    }

    fn get_jsx_runtime_import_specifier(
        &mut self,
        file: Option<ast::SourceFile>,
    ) -> (String, Option<ast::Node>) {
        self.program
            .get_jsx_runtime_import_specifier(file.as_ref().unwrap().path())
    }
}
