use ts_printer as printer;

use crate::checker::*;
use crate::{ast, debug, nodebuilder, pseudochecker};

impl<'a, 'state, 'c, 'e> NodeBuilderImpl<'a, 'state, 'c, 'e> {
    fn pseudo_store_for_node(&self, node: ast::Node) -> &ast::AstStore {
        if node.store_id() == self.e.factory.node_factory.store().store_id() {
            self.e.factory.node_factory.store()
        } else {
            self.ch.store_for_node(node)
        }
    }

    fn pseudo_node_parent(&self, node: ast::Node) -> Option<ast::Node> {
        self.pseudo_store_for_node(node).parent(node)
    }

    fn pseudo_node_name(&self, node: ast::Node) -> Option<ast::Node> {
        self.pseudo_store_for_node(node).name(node)
    }

    fn pseudo_node_symbol(&self, node: ast::Node) -> Option<SymbolIdentity> {
        if node.store_id() == self.e.factory.node_factory.store().store_id() {
            self.ch.semantic_state.synthetic_node_symbol_identity(node)
        } else {
            self.ch
                .node_symbol(node)
                .map(SymbolIdentity::from_symbol_handle)
        }
    }

    fn pseudo_node_symbol_name(&self, symbol: SymbolIdentity) -> ast::SymbolName {
        self.ch.symbol_identity_name(symbol)
    }

    fn pseudo_parameter_to_parameter_declaration_name(
        &mut self,
        parameter_symbol: SymbolIdentity,
        parameter_declaration: Option<ast::Node>,
    ) -> ast::Node {
        let parameter_name = parameter_declaration
            .and_then(|declaration| self.store_for_node(declaration).name(declaration));
        if parameter_name.is_none() {
            let name = self.ch.symbol_identity_name(parameter_symbol).to_string();
            let id = Self::node_value(self.e.factory.node_factory.new_identifier(name));
            self.id_to_symbol.insert(id, parameter_symbol);
            return id;
        }

        let name = parameter_name.unwrap();
        match self.store_for_node(name).kind(name) {
            ast::KIND_IDENTIFIER => {
                let cloned = Self::node_value(self.deep_clone_node(name));
                self.e
                    .set_emit_flags(&cloned, printer::EF_NO_ASCII_ESCAPING);
                self.id_to_symbol.insert(cloned, parameter_symbol);
                cloned
            }
            ast::KIND_QUALIFIED_NAME => {
                let right = {
                    let name_store = self.store_for_node(name);
                    name_store.right(name).unwrap()
                };
                let cloned = Self::node_value(self.deep_clone_node(right));
                self.e
                    .set_emit_flags(&cloned, printer::EF_NO_ASCII_ESCAPING);
                self.id_to_symbol.insert(cloned, parameter_symbol);
                cloned
            }
            _ => self.clone_binding_name(name),
        }
    }

    // pseudoTypeToNodeWithCheckerFallback is like pseudoTypeToNode but when the top-level pseudo type
    // is PseudoTypeInferred, it reports any error nodes and then serializes from the checker's type.
    // This avoids incorrect type output when PseudoTypeInferred would derive the type from the
    // original declaration expression in an instantiated context.
    pub(crate) fn pseudo_type_to_node_with_checker_fallback(
        &mut self,
        t: &pseudochecker::PseudoType,
        checker_type: TypeHandle,
    ) -> Option<ast::Node> {
        if t.kind == pseudochecker::PseudoTypeKind::Inferred {
            if !self.ctx.suppress_report_inference_fallback {
                let inferred = t.as_pseudo_type_inferred();
                if !inferred.error_nodes.is_empty() {
                    for n in inferred.error_nodes.iter() {
                        self.report_inference_fallback(*n);
                    }
                } else {
                    self.report_inference_fallback(inferred.expression);
                }
            }
            let old_suppress = self.ctx.suppress_report_inference_fallback;
            self.ctx.suppress_report_inference_fallback = true;
            let result = self.type_to_type_node(checker_type);
            self.ctx.suppress_report_inference_fallback = old_suppress;
            return result;
        } else if t.kind == pseudochecker::PseudoTypeKind::Direct {
            let existing = &t.as_pseudo_type_direct().type_node;
            let has_invalid_jsdoc_nullable_type = self
                .ch
                .invalid_jsdoc_type_token(*existing)
                .is_some_and(|(token, _)| token == '?')
                || self
                    .ch
                    .store_for_node(*existing)
                    .r#type(*existing)
                    .is_some_and(|type_node| {
                        self.ch
                            .invalid_jsdoc_type_token(type_node)
                            .is_some_and(|(token, _)| token == '?')
                    });
            if has_invalid_jsdoc_nullable_type
                || !self.existing_type_node_is_not_reference_or_is_reference_with_compatible_type_argument_count(*existing, checker_type)
            {
                if !self.ctx.suppress_report_inference_fallback {
                    self.report_inference_fallback(*existing);
                }
                let old_suppress = self.ctx.suppress_report_inference_fallback;
                self.ctx.suppress_report_inference_fallback = true;
                let result = self.type_to_type_node(checker_type);
                self.ctx.suppress_report_inference_fallback = old_suppress;
                return result;
            }
        }
        self.pseudo_type_to_node(t)
    }

    // Maps a pseudochecker's pseudotypes into ast nodes and reports any inference fallback errors the pseudotype structure implies
    fn pseudo_type_to_node(&mut self, t: &pseudochecker::PseudoType) -> Option<ast::Node> {
        match t.kind {
            pseudochecker::PseudoTypeKind::Direct => {
                let type_node = t.as_pseudo_type_direct().type_node;
                self.reuse_type_node(Some(type_node))
            }
            pseudochecker::PseudoTypeKind::Inferred => {
                let inferred = t.as_pseudo_type_inferred();
                let node = &inferred.expression;
                if !inferred.error_nodes.is_empty() {
                    for n in inferred.error_nodes.iter() {
                        self.report_inference_fallback(*n);
                    }
                } else if ast::is_entity_name_expression(self.pseudo_store_for_node(*node), *node)
                    && self.pseudo_node_parent(*node).is_some_and(|parent| {
                        ast::is_declaration(self.pseudo_store_for_node(parent), parent)
                    })
                {
                    self.report_inference_fallback(Self::node_value(
                        self.pseudo_node_parent(*node).unwrap(),
                    ));
                } else {
                    self.report_inference_fallback(*node);
                }
                // use symbol type from parent declaration to automatically handle expression type widening without duplicating logic
                let parent = Self::node_value(self.pseudo_node_parent(*node).unwrap());
                if ast::is_return_statement(self.pseudo_store_for_node(parent), parent) {
                    let enclosing =
                        ast::get_containing_function(self.pseudo_store_for_node(*node), node)
                            .map(Self::node_value);
                    if enclosing.as_ref().is_some_and(|node| {
                        ast::is_accessor(self.pseudo_store_for_node(*node), *node)
                    }) {
                        return Some(
                            self.serialize_type_for_declaration(enclosing, None, None, false),
                        );
                    }
                    let signature = self.ch.get_signature_from_declaration(enclosing.unwrap());
                    return self.serialize_return_type_for_signature(signature, false);
                }
                if ast::is_arrow_function(self.pseudo_store_for_node(parent), parent)
                    && self
                        .pseudo_store_for_node(parent)
                        .body(parent)
                        .is_some_and(|body| body == *node)
                {
                    let signature = self.ch.get_signature_from_declaration(parent);
                    return self.serialize_return_type_for_signature(signature, false);
                }
                if ast::is_declaration(self.pseudo_store_for_node(parent), parent) {
                    return Some(self.serialize_type_for_declaration(
                        Some(parent),
                        None,
                        None,
                        false,
                    ));
                }
                // This might be effectively unreachable. If it's not, it may need more widening rules to mirror checker behavior for whatever expressions are serialized here
                let ty = self.ch.get_type_of_expression(*node);
                self.type_to_type_node(ty)
            }
            pseudochecker::PseudoTypeKind::NoResult => {
                let node = &t.as_pseudo_type_no_result().declaration;
                self.report_inference_fallback(*node);
                if ast::is_function_like(self.pseudo_store_for_node(*node), Some(*node))
                    && !ast::is_accessor(self.pseudo_store_for_node(*node), *node)
                {
                    let signature = self.ch.get_signature_from_declaration(*node);
                    return self.serialize_return_type_for_signature(signature, false);
                }
                Some(self.serialize_type_for_declaration(Some(*node), None, None, false))
            }
            pseudochecker::PseudoTypeKind::MaybeConstLocation => {
                let d = t.as_pseudo_type_maybe_const_location();
                // see checkExpressionWithContextualType for general literal widening rules which need to be emulated here, plus
                // checkTemplateLiteralExpression for template literal widening rules if the pseudochecker ever supports literalized templates
                let mut is_in_const_context = self.ch.is_const_context(d.node);
                if !is_in_const_context
                    && pseudochecker::is_in_const_context(self.ch.store_for_node(d.node), d.node)
                {
                    // Only consult the contextual type if the pseudochecker's syntactic check also puts us in a const context.
                    // getContextualType returns post-inference results at node-printing time which may not have existed
                    // during initial checking (e.g. when the contextual type depends on inference), causing incorrect
                    // literal type preservation.
                    let contextual_type = self.ch.get_contextual_type(d.node, CONTEXT_FLAGS_NONE);
                    // PORT NOTE: reshaped for borrowck. Keep the recursive
                    // pseudotype conversion result in a local before continuing
                    // with contextual-type checks in source order.
                    let t = self.pseudo_type_to_type(&d.const_type);
                    if t.is_some() {
                        let instantiated = self.ch.instantiate_contextual_type(
                            contextual_type,
                            d.node,
                            CONTEXT_FLAGS_NONE,
                        );
                        if self
                            .ch
                            .is_literal_of_contextual_type(t.unwrap(), instantiated)
                        {
                            is_in_const_context = true;
                        }
                    }
                }
                if is_in_const_context {
                    self.pseudo_type_to_node(&d.const_type)
                } else {
                    self.pseudo_type_to_node(&d.regular_type)
                }
            }
            pseudochecker::PseudoTypeKind::Union => {
                let mut res = Vec::new();
                let mut has_elided_type = false;
                let members = &t.as_pseudo_type_union().types;
                for m in members.iter() {
                    if !self.ch.strict_null_checks()
                        && (m.kind == pseudochecker::PseudoTypeKind::Undefined
                            || m.kind == pseudochecker::PseudoTypeKind::Null)
                    {
                        has_elided_type = true;
                        continue;
                    }
                    res.push(self.pseudo_type_to_node(m).unwrap().clone());
                }
                if res.len() == 1 {
                    return Some(Self::node_value(res.pop().unwrap()));
                }
                if res.is_empty() {
                    if has_elided_type {
                        return Some(Self::node_value(
                            self.e
                                .factory
                                .node_factory
                                .new_keyword_type_node(ast::Kind::AnyKeyword),
                        ));
                    }
                    return Some(Self::node_value(
                        self.e
                            .factory
                            .node_factory
                            .new_keyword_type_node(ast::Kind::NeverKeyword),
                    ));
                }
                let types = self.new_factory_node_list(res);
                Some(Self::node_value(
                    self.e.factory.node_factory.new_union_type_node(types),
                ))
            }
            pseudochecker::PseudoTypeKind::Undefined => {
                if !self.ch.strict_null_checks() {
                    return Some(Self::node_value(
                        self.e
                            .factory
                            .node_factory
                            .new_keyword_type_node(ast::Kind::AnyKeyword),
                    ));
                }
                Some(Self::node_value(
                    self.e
                        .factory
                        .node_factory
                        .new_keyword_type_node(ast::Kind::UndefinedKeyword),
                ))
            }
            pseudochecker::PseudoTypeKind::Null => {
                if !self.ch.strict_null_checks() {
                    return Some(Self::node_value(
                        self.e
                            .factory
                            .node_factory
                            .new_keyword_type_node(ast::Kind::AnyKeyword),
                    ));
                }
                let literal = self
                    .e
                    .factory
                    .node_factory
                    .new_keyword_expression(ast::Kind::NullKeyword);
                Some(Self::node_value(
                    self.e.factory.node_factory.new_literal_type_node(literal),
                ))
            }
            pseudochecker::PseudoTypeKind::Any => Some(Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_keyword_type_node(ast::Kind::AnyKeyword),
            )),
            pseudochecker::PseudoTypeKind::String => Some(Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_keyword_type_node(ast::Kind::StringKeyword),
            )),
            pseudochecker::PseudoTypeKind::Number => Some(Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_keyword_type_node(ast::Kind::NumberKeyword),
            )),
            pseudochecker::PseudoTypeKind::BigInt => Some(Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_keyword_type_node(ast::Kind::BigIntKeyword),
            )),
            pseudochecker::PseudoTypeKind::Boolean => Some(Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_keyword_type_node(ast::Kind::BooleanKeyword),
            )),
            pseudochecker::PseudoTypeKind::False => Some(Self::node_value({
                let literal = self
                    .e
                    .factory
                    .node_factory
                    .new_keyword_expression(ast::Kind::FalseKeyword);
                self.e.factory.node_factory.new_literal_type_node(literal)
            })),
            pseudochecker::PseudoTypeKind::True => Some(Self::node_value({
                let literal = self
                    .e
                    .factory
                    .node_factory
                    .new_keyword_expression(ast::Kind::TrueKeyword);
                self.e.factory.node_factory.new_literal_type_node(literal)
            })),
            pseudochecker::PseudoTypeKind::SingleCallSignature => {
                let d = t.as_pseudo_type_single_call_signature();
                let signature = self.ch.get_signature_from_declaration(d.signature);
                let expanded_params = self
                    .ch
                    .get_expanded_parameters(signature, true /*skipUnionExpanding*/)[0]
                    .clone();
                let signature_record = self.ch.signature_record(signature).clone();
                let mapper = signature_record.mapper;
                let cleanup = self.enter_new_scope(
                    Some(d.signature),
                    Some(expanded_params),
                    signature_record.type_parameters,
                    Some(signature_record.parameters.to_vec()),
                    mapper,
                );
                let mut type_params = None;
                if !d.type_parameters.is_empty() {
                    let mut res = Vec::with_capacity(d.type_parameters.len());
                    for tp in d.type_parameters.iter() {
                        res.push(
                            self.reuse_node(Some(*tp))
                                .expect("type parameter should be reusable"),
                        );
                    }
                    type_params = Some(self.new_factory_node_list(res));
                }
                let params = self.pseudo_parameters_to_node_list(&d.parameters);
                let return_type = self.pseudo_type_to_node(&d.return_type);
                self.exit_scope(cleanup);
                Some(Self::node_value(
                    self.e.factory.node_factory.new_function_type_node(
                        type_params,
                        params,
                        return_type,
                    ),
                ))
            }
            pseudochecker::PseudoTypeKind::Tuple => {
                let mut res = Vec::new();
                for e in t.as_pseudo_type_tuple().elements.iter() {
                    res.push(self.pseudo_type_to_node(e).unwrap().clone());
                }
                // pseudo-tuples are implicitly `readonly` since they originate from `as const` contexts
                // but strada *sometimes* fails to add the `readonly` modifier to the generated node.
                let elements = self.new_factory_node_list(res);
                let result = self.e.factory.node_factory.new_tuple_type_node(elements);
                self.e.mark_emit_node(&result, printer::EF_SINGLE_LINE);
                Some(Self::node_value(
                    self.e
                        .factory
                        .node_factory
                        .new_type_operator_node(ast::Kind::ReadonlyKeyword, result),
                ))
            }
            pseudochecker::PseudoTypeKind::ObjectLiteral => {
                let elements = &t.as_pseudo_type_object_literal().elements;
                if elements.is_empty() {
                    let members = self.new_factory_node_list([]);
                    let result = self.e.factory.node_factory.new_type_literal_node(members);
                    self.e.mark_emit_node(&result, printer::EF_SINGLE_LINE);
                    return Some(Self::node_value(result));
                }
                // NOTE: using the checker's `isConstContext` instead of the pseudochecker's `isInConstContext`
                // results in different results here. The checker one is more "correct" but means we'll mark
                // objects in parameter positions contextually typed by const type parameters as readonly -
                // something a true syntactic ID emitter couldn't possibly know (since the signature could
                // be from across files). This can't *really* happen in any cases ID doesn't already error on, though.
                // Just something to keep in mind if the ID checker keeps growing.
                let is_const = self.ch.is_const_context(elements[0].name);
                let mut new_elements = Vec::with_capacity(elements.len());

                for e in elements.iter() {
                    let mut modifiers = None;
                    if is_const
                        || (e.kind == pseudochecker::PseudoObjectElementKind::PropertyAssignment
                            && e.as_pseudo_property_assignment().readonly)
                    {
                        modifiers =
                            Some(self.new_factory_modifier_list(ast::ModifierFlags::READONLY));
                    }
                    let mut cleanup = None;
                    if e.kind != pseudochecker::PseudoObjectElementKind::PropertyAssignment {
                        let signature = self
                            .ch
                            .get_signature_from_declaration(e.signature().unwrap());
                        let expanded_params = self
                            .ch
                            .get_expanded_parameters(signature, true /*skipUnionExpanding*/)[0]
                            .clone();
                        let signature_record = self.ch.signature_record(signature).clone();
                        let mapper = signature_record.mapper;
                        cleanup = Some(self.enter_new_scope(
                            e.signature(),
                            Some(expanded_params),
                            signature_record.type_parameters,
                            Some(signature_record.parameters.to_vec()),
                            mapper,
                        ));
                    }
                    let new_prop = match e.kind {
                        pseudochecker::PseudoObjectElementKind::Method => {
                            let d = e.as_pseudo_object_method();
                            let mut type_params = None;
                            if !d.type_parameters.is_empty() {
                                let mut res = Vec::with_capacity(d.type_parameters.len());
                                for tp in d.type_parameters.iter() {
                                    res.push(
                                        self.reuse_node(Some(*tp))
                                            .expect("type parameter should be reusable"),
                                    );
                                }
                                type_params = Some(self.new_factory_node_list(res));
                            }
                            if is_const {
                                let name = self.reuse_name(Some(e.name)).unwrap().clone();
                                let parameters = self.pseudo_parameters_to_node_list(&d.parameters);
                                let return_type = self.pseudo_type_to_node(&d.return_type);
                                let function_type = self
                                    .e
                                    .factory
                                    .node_factory
                                    .new_function_type_node(type_params, parameters, return_type);
                                self.e
                                    .factory
                                    .node_factory
                                    .new_property_signature_declaration(
                                        modifiers,
                                        name,
                                        None,
                                        Some(function_type),
                                        None,
                                    )
                            } else {
                                let name = self.reuse_name(Some(e.name)).unwrap().clone();
                                let parameters = self.pseudo_parameters_to_node_list(&d.parameters);
                                let return_type = self.pseudo_type_to_node(&d.return_type);
                                self.e
                                    .factory
                                    .node_factory
                                    .new_method_signature_declaration(
                                        modifiers,
                                        name,
                                        None,
                                        type_params,
                                        parameters,
                                        return_type,
                                    )
                            }
                        }
                        pseudochecker::PseudoObjectElementKind::PropertyAssignment => {
                            let d = e.as_pseudo_property_assignment();
                            let name = self.reuse_name(Some(e.name)).unwrap().clone();
                            let type_node = self.pseudo_type_to_node(&d.type_);
                            self.e
                                .factory
                                .node_factory
                                .new_property_signature_declaration(
                                    modifiers, name, None, type_node, None,
                                )
                        }
                        pseudochecker::PseudoObjectElementKind::SetAccessor => {
                            let d = e.as_pseudo_set_accessor();
                            let name = self.reuse_name(Some(e.name)).unwrap().clone();
                            let parameter = self.pseudo_parameter_to_node(&d.parameter);
                            let parameters = self.new_factory_node_list([parameter]);
                            self.e.factory.node_factory.new_set_accessor_declaration(
                                None, name, None, parameters, None, None, None,
                            )
                        }
                        pseudochecker::PseudoObjectElementKind::GetAccessor => {
                            let d = e.as_pseudo_get_accessor();
                            let name = self.reuse_name(Some(e.name)).unwrap().clone();
                            let parameters = self.new_factory_node_list([]);
                            let type_node = self.pseudo_type_to_node(&d.type_);
                            self.e.factory.node_factory.new_get_accessor_declaration(
                                None, name, None, parameters, type_node, None, None,
                            )
                        }
                    };
                    if let Some(cleanup) = cleanup {
                        self.exit_scope(cleanup);
                    }
                    let is_same_source_file = ast::get_source_file_of_node(
                        self.pseudo_store_for_node(e.name),
                        Some(e.name),
                    )
                    .zip(self.ctx.enclosing_file)
                    .is_some_and(|(source_file, enclosing_file)| {
                        let source_store = self.pseudo_store_for_node(source_file);
                        SourceFileIdentity::from_root(source_file) == enclosing_file
                    });
                    if is_same_source_file {
                        let name_parent = self.pseudo_node_parent(e.name).unwrap();
                        let comment_range = {
                            let name_parent_store = self.pseudo_store_for_node(name_parent);
                            name_parent_store.loc(name_parent)
                        };
                        self.e.set_comment_range(&new_prop, comment_range);
                    }
                    new_elements.push(new_prop);
                }
                let members = self.new_factory_node_list(new_elements);
                let result = self.e.factory.node_factory.new_type_literal_node(members);
                if self.ctx.flags & nodebuilder::FLAGS_MULTILINE_OBJECT_LITERALS == 0 {
                    self.e.mark_emit_node(&result, printer::EF_SINGLE_LINE);
                }
                Some(Self::node_value(result))
            }
            pseudochecker::PseudoTypeKind::StringLiteral
            | pseudochecker::PseudoTypeKind::NumericLiteral
            | pseudochecker::PseudoTypeKind::BigIntLiteral => {
                let source = &t.as_pseudo_type_literal().node;
                let literal = self.reuse_node(Some(*source)).unwrap().clone();
                Some(Self::node_value(
                    self.e.factory.node_factory.new_literal_type_node(literal),
                ))
            }
            _ => {
                debug::assert_never(
                    &format!("{:?}", t.kind),
                    Some("Unhandled pseudotype kind in pseudotype node construction".to_string()),
                );
                None
            }
        }
    }

    fn pseudo_parameters_to_node_list(
        &mut self,
        params: &[pseudochecker::PseudoParameter],
    ) -> ast::NodeList {
        let mut res = Vec::with_capacity(params.len());
        for p in params.iter() {
            res.push(self.pseudo_parameter_to_node(p));
        }
        self.new_factory_node_list(res)
    }

    fn pseudo_parameter_to_node(&mut self, p: &pseudochecker::PseudoParameter) -> ast::Node {
        let mut dot_dot_dot = None;
        let mut question_mark = None;
        if p.rest {
            dot_dot_dot = Some(
                self.e
                    .factory
                    .node_factory
                    .new_token(ast::Kind::DotDotDotToken),
            );
        }
        if p.optional {
            question_mark = Some(
                self.e
                    .factory
                    .node_factory
                    .new_token(ast::Kind::QuestionToken),
            );
        }
        let parameter_parent = self.pseudo_node_parent(p.name).unwrap();
        let parameter_name = self
            .pseudo_parameter_to_parameter_declaration_name(
                self.pseudo_node_symbol(parameter_parent).unwrap(),
                Some(parameter_parent),
            )
            .clone();
        let type_node = self.pseudo_type_to_node(&p.type_);
        let parameter = self.e.factory.node_factory.new_parameter_declaration(
            None,
            dot_dot_dot,
            // matches strada behavior of always reserializing param names from scratch
            parameter_name,
            question_mark,
            type_node,
            None,
        );
        parameter
    }

    // see `typeNodeIsEquivalentToType` in strada, but applied more broadly here, so is setup to handle more equivalences - strada only used it via
    // the `canReuseTypeNodeAnnotation` host hook and not the `canReuseTypeNode` hook, which meant locations using the later were reliant on
    // over-invalidation by the ID inference engine to not emit incorrect types.
    pub(crate) fn pseudo_type_equivalent_to_type(
        &mut self,
        t: &pseudochecker::PseudoType,
        type_: Option<TypeHandle>,
        is_optional_annotated: bool,
        report_errors: bool,
    ) -> bool {
        // if type_ resolves to an error, we charitably assume equality, since we might be in a single-file checking mode
        if type_.is_some() && self.ch.is_error_type(type_.unwrap()) {
            return true;
        }
        // If we can easily operate on just types, we should
        let type_from_pseudo = self.pseudo_type_to_type(t); // note: cannot convert complex types like objects, which must be validated separately
        if type_from_pseudo == type_ {
            return true;
        }
        if let (Some(type_from_pseudo), Some(type_)) = (type_from_pseudo, type_) {
            if is_optional_annotated {
                let undefined_stripped =
                    self.ch.get_type_with_facts(type_, TYPE_FACTS_NE_UNDEFINED);
                if Some(undefined_stripped) == Some(type_from_pseudo) {
                    return true;
                }
                if self.ch.type_flags(type_from_pseudo) & TYPE_FLAGS_UNION != 0
                    && self.ch.type_flags(undefined_stripped) & TYPE_FLAGS_UNION != 0
                {
                    // does union comparison in general, since the unions may not be `==` identical due to aliasing and the like
                    if self
                        .ch
                        .compare_types_identical(type_from_pseudo, undefined_stripped)
                        == TERNARY_TRUE
                    {
                        return true;
                    }
                }
            }
            // handles freshness mismatches (e.g., fresh true vs regular true in as const)
            if self.ch.get_regular_type_of_literal_type(type_from_pseudo)
                == self.ch.get_regular_type_of_literal_type(type_)
            {
                return true;
            }
            if self.ch.type_flags(type_from_pseudo) & TYPE_FLAGS_UNION != 0
                && self.ch.type_flags(type_) & TYPE_FLAGS_UNION != 0
            {
                // handles union comparison in general, since unions may not be `==` identical due to aliasing
                if self.ch.compare_types_identical(type_from_pseudo, type_) == TERNARY_TRUE {
                    return true;
                }
            }
        }
        // otherwise, fallback to actual pseudo/type cross-comparisons
        match t.kind {
            pseudochecker::PseudoTypeKind::Inferred => {
                // PseudoTypeInferred with error nodes identifies specific problematic children.
                // Report fine-grained errors on them, then return false so the parent falls back
                // to checker-based serialization (avoiding issues like reusing raw JSON string
                // literal property names from the pseudochecker's AST).
                let inferred = t.as_pseudo_type_inferred();
                if !inferred.error_nodes.is_empty() {
                    if report_errors {
                        for n in inferred.error_nodes.iter() {
                            self.report_inference_fallback(*n);
                        }
                    }
                    return false;
                }
                if report_errors {
                    self.report_inference_fallback(inferred.expression);
                }
                false
            }
            pseudochecker::PseudoTypeKind::ObjectLiteral => {
                let pt = t.as_pseudo_type_object_literal();
                let Some(type_) = type_ else {
                    return false;
                };
                let target_props = self.ch.get_properties_of_type(type_);
                // Count total declarations across all target prop symbols to handle getter/setter pairs,
                // which are two elements in pt.Elements but only one symbol in targetProps.
                let mut target_decl_count = 0;
                for prop in target_props.iter() {
                    target_decl_count += self.ch.collect_symbol_identity_declarations(*prop).len();
                }
                if pt.elements.len() != target_decl_count {
                    return false;
                }
                for e in pt.elements.iter() {
                    let elem_parent = self.pseudo_node_parent(e.name).unwrap();
                    let mut target_prop = None;
                    let elem_symbol = self.pseudo_node_symbol(elem_parent);
                    if let Some(elem_symbol) = elem_symbol {
                        let elem_symbol_name = self.pseudo_node_symbol_name(elem_symbol);
                        target_prop = self.ch.get_property_of_type(type_, &elem_symbol_name);
                    }
                    if target_prop.is_none() {
                        // Name lookup failed or returned no result; search target properties
                        // for one whose declaration name node matches the one we have
                        for prop in target_props.iter() {
                            if self
                                .ch
                                .missing_name_symbol_identity_value_declaration(*prop)
                                .as_ref()
                                .is_some_and(|declaration| {
                                    self.pseudo_node_name(*declaration) == Some(e.name)
                                })
                            {
                                target_prop = Some(*prop);
                                break;
                            }
                        }
                        if target_prop.is_none() {
                            if report_errors {
                                let parent = Self::node_value(elem_parent);
                                self.report_inference_fallback(parent);
                            }
                            return false;
                        }
                    }
                    let target_prop = target_prop.unwrap();
                    let target_is_optional =
                        self.ch.missing_name_symbol_identity_flags(target_prop)
                            & ast::SYMBOL_FLAGS_OPTIONAL
                            != 0;
                    if e.optional != target_is_optional {
                        if report_errors {
                            let parent = Self::node_value(elem_parent);
                            self.report_inference_fallback(parent);
                        }
                        return false;
                    }
                    let mut prop_type = self.ch.get_type_of_symbol_identity(target_prop);
                    prop_type = self.ch.remove_missing_type(prop_type, target_is_optional);
                    match e.kind {
                        pseudochecker::PseudoObjectElementKind::PropertyAssignment => {
                            let d = e.as_pseudo_property_assignment();
                            if !self.pseudo_type_equivalent_to_type(
                                &d.type_,
                                Some(prop_type),
                                e.optional,
                                false,
                            ) {
                                if report_errors {
                                    if d.type_.kind == pseudochecker::PseudoTypeKind::Inferred
                                        && !d.type_.as_pseudo_type_inferred().error_nodes.is_empty()
                                    {
                                        // Re-report the fine-grained error nodes; the recursive call used reportErrors=false
                                        for n in
                                            d.type_.as_pseudo_type_inferred().error_nodes.iter()
                                        {
                                            self.report_inference_fallback(*n);
                                        }
                                    } else if !is_structural_pseudo_type(&d.type_) {
                                        let parent = Self::node_value(elem_parent);
                                        self.report_inference_fallback(parent);
                                    }
                                }
                                return false;
                            }
                        }
                        pseudochecker::PseudoObjectElementKind::Method => {
                            let d = e.as_pseudo_object_method();
                            let target_sig = self.ch.get_single_call_signature(prop_type);
                            if target_sig.is_none() {
                                // Target property type doesn't have a single call signature; can't validate
                                continue;
                            }
                            let target_sig = target_sig.unwrap();
                            let target_sig_record = self.ch.signature_record(target_sig).clone();
                            if target_sig_record.parameters.len() != d.parameters.len() {
                                if report_errors {
                                    let parent = Self::node_value(elem_parent);
                                    self.report_inference_fallback(parent);
                                }
                                return false;
                            }
                            for (i, p) in d.parameters.iter().enumerate() {
                                let target_param =
                                    self.ch.signature_parameter(target_sig, i).unwrap();
                                let param_type = self.ch.get_type_of_parameter(target_param);
                                if !self.pseudo_type_equivalent_to_type(
                                    &p.type_,
                                    Some(param_type),
                                    p.optional,
                                    false,
                                ) {
                                    if report_errors {
                                        let parent = Self::node_value(elem_parent);
                                        self.report_inference_fallback(parent);
                                    }
                                    return false;
                                }
                            }
                            let target_predicate =
                                self.ch.get_type_predicate_of_signature(target_sig);
                            if let Some(target_predicate) = target_predicate {
                                if !self.pseudo_return_type_matches_predicate(
                                    &d.return_type,
                                    target_predicate,
                                ) {
                                    if report_errors {
                                        let parent = Self::node_value(elem_parent);
                                        self.report_inference_fallback(parent);
                                    }
                                    return false;
                                }
                            } else {
                                let target_return_type =
                                    self.ch.get_return_type_of_signature(target_sig);
                                if !self.pseudo_type_equivalent_to_type(
                                    &d.return_type,
                                    Some(target_return_type),
                                    false,
                                    false,
                                ) {
                                    if report_errors {
                                        let parent = Self::node_value(elem_parent);
                                        self.report_inference_fallback(parent);
                                    }
                                    return false;
                                }
                            }
                        }
                        pseudochecker::PseudoObjectElementKind::GetAccessor => {
                            let d = e.as_pseudo_get_accessor();
                            if !self.pseudo_type_equivalent_to_type(
                                &d.type_,
                                Some(prop_type),
                                false,
                                false,
                            ) {
                                if report_errors {
                                    let parent = Self::node_value(elem_parent);
                                    self.report_inference_fallback(parent);
                                }
                                return false;
                            }
                        }
                        pseudochecker::PseudoObjectElementKind::SetAccessor => {
                            let d = e.as_pseudo_set_accessor();
                            let target_prop_handle = target_prop.symbol_handle();
                            let write_type =
                                self.ch.get_write_type_of_symbol_handle(target_prop_handle);
                            if !self.pseudo_type_equivalent_to_type(
                                &d.parameter.type_,
                                Some(write_type),
                                false,
                                false,
                            ) {
                                if report_errors {
                                    let parent = Self::node_value(elem_parent);
                                    self.report_inference_fallback(parent);
                                }
                                return false;
                            }
                        }
                    }
                }
                true
            }
            pseudochecker::PseudoTypeKind::Tuple => {
                let pt = t.as_pseudo_type_tuple();
                if type_.is_none() || !self.ch.is_tuple_type(type_.unwrap()) {
                    return false;
                }
                let tuple_target = self.ch.target_tuple_type_record(type_.unwrap());
                // Pseudo-tuples come from `as const` array literals, so they only ever have required elements.
                // If the target tuple has optional, rest, or variadic elements, the structures can't match.
                if tuple_target.combined_flags & ELEMENT_FLAGS_NON_REQUIRED != 0 {
                    return false;
                }
                let element_types = self.ch.get_type_arguments(type_.unwrap());
                if pt.elements.len() != element_types.len() {
                    return false;
                }
                for (i, elem) in pt.elements.iter().enumerate() {
                    if !self.pseudo_type_equivalent_to_type(
                        elem,
                        Some(element_types[i]),
                        false,
                        report_errors,
                    ) {
                        return false;
                    }
                }
                true
            }
            pseudochecker::PseudoTypeKind::SingleCallSignature => {
                let target_sig = self.ch.get_single_call_signature(type_.unwrap());
                if target_sig.is_none() {
                    return false;
                }
                let target_sig = target_sig.unwrap();
                let target_sig_record = self.ch.signature_record(target_sig).clone();
                let pt = t.as_pseudo_type_single_call_signature();
                if target_sig_record.type_parameters.len() != pt.type_parameters.len() {
                    if report_errors {
                        self.report_inference_fallback(pt.signature);
                    }
                    return false;
                }
                if target_sig_record.parameters.len() != pt.parameters.len() {
                    if report_errors {
                        self.report_inference_fallback(pt.signature);
                    }
                    return false; // TODO: spread tuple params may mess with this check
                }
                for (i, p) in pt.parameters.iter().enumerate() {
                    let target_param = self.ch.signature_parameter(target_sig, i).unwrap();
                    let value_declaration = self
                        .ch
                        .missing_name_symbol_identity_value_declaration(target_param)
                        .unwrap();
                    let value_declaration = value_declaration;
                    if p.optional != self.ch.is_optional_parameter(value_declaration) {
                        if report_errors {
                            let parent = Self::node_value(self.pseudo_node_parent(p.name).unwrap());
                            self.report_inference_fallback(parent);
                        }
                        return false;
                    }
                    let param_type = self.ch.get_type_of_parameter(target_param);
                    if !self.pseudo_type_equivalent_to_type(
                        &p.type_,
                        Some(param_type),
                        p.optional,
                        false,
                    ) {
                        if report_errors {
                            let parent = Self::node_value(self.pseudo_node_parent(p.name).unwrap());
                            self.report_inference_fallback(parent);
                        }
                        return false;
                    }
                }
                let target_predicate = self.ch.get_type_predicate_of_signature(target_sig);
                if let Some(target_predicate) = target_predicate {
                    if !self.pseudo_return_type_matches_predicate(&pt.return_type, target_predicate)
                    {
                        if report_errors {
                            self.report_inference_fallback(pt.signature);
                        }
                        return false;
                    }
                } else {
                    let target_return_type = self.ch.get_return_type_of_signature(target_sig);
                    if !self.pseudo_type_equivalent_to_type(
                        &pt.return_type,
                        Some(target_return_type),
                        false,
                        report_errors,
                    ) {
                        // error reported within the return type
                        return false;
                    }
                }
                true
            }
            pseudochecker::PseudoTypeKind::NoResult => {
                if report_errors {
                    self.report_inference_fallback(t.as_pseudo_type_no_result().declaration);
                }
                false
            }
            _ => false,
        }
    }

    // pseudoReturnTypeMatchesPredicate checks if a pseudo return type (which should be a Direct type
    // wrapping a TypePredicate) matches the given type predicate from the checker.
    pub(crate) fn pseudo_return_type_matches_predicate(
        &mut self,
        rt: &pseudochecker::PseudoType,
        predicate: TypePredicateHandle,
    ) -> bool {
        let predicate = self.ch.type_predicate_record(predicate).clone();
        if rt.kind != pseudochecker::PseudoTypeKind::Direct {
            return false;
        }
        let node = &rt.as_pseudo_type_direct().type_node;
        if !ast::is_type_predicate_node(self.pseudo_store_for_node(*node), *node) {
            return false;
        }
        let (is_asserts, parameter_name, type_node) = {
            let store = self.pseudo_store_for_node(*node);
            (
                store.asserts_modifier(*node).is_some(),
                store.parameter_name(*node),
                store.r#type(*node),
            )
        };
        // Check asserts modifier matches
        let predicate_is_asserts = predicate.kind == TYPE_PREDICATE_KIND_ASSERTS_THIS
            || predicate.kind == TYPE_PREDICATE_KIND_ASSERTS_IDENTIFIER;
        if is_asserts != predicate_is_asserts {
            return false;
        }
        // Check this vs identifier matches
        let is_this = parameter_name
            .as_ref()
            .is_some_and(|name| ast::is_this_type_node(self.pseudo_store_for_node(*name), *name));
        let predicate_is_this = predicate.kind == TYPE_PREDICATE_KIND_THIS
            || predicate.kind == TYPE_PREDICATE_KIND_ASSERTS_THIS;
        if is_this != predicate_is_this {
            return false;
        }
        // For identifier predicates, check parameter name matches
        if !is_this
            && parameter_name.as_ref().is_none_or(|name| {
                self.pseudo_store_for_node(*name).text(*name) != predicate.parameter_name
            })
        {
            return false;
        }
        // Check the narrowed type, if any
        if let Some(predicate_t) = predicate.t {
            let Some(type_node) = type_node else {
                return false;
            };
            let type_node = type_node;
            let predicate_type_from_node = self.ch.get_type_from_type_node(type_node);
            if predicate_type_from_node != predicate_t
                && self
                    .ch
                    .compare_types_identical(predicate_type_from_node, predicate_t)
                    != TERNARY_TRUE
            {
                return false;
            }
        } else if type_node.is_some() {
            return false;
        }
        true
    }

    pub(crate) fn pseudo_type_to_type(
        &mut self,
        t: &pseudochecker::PseudoType,
    ) -> Option<TypeHandle> {
        // !!! TODO: only literal types currently mapped because this is only used to determine if literal contextual typing need apply to the pseudotype
        // If this is used more broadly, the implementation needs to be filled out more to handle the structural pseudotypes - signatures, objects, tuples, etc
        match t.kind {
            pseudochecker::PseudoTypeKind::Direct => Some(
                self.ch
                    .get_type_from_type_node(t.as_pseudo_type_direct().type_node),
            ),
            pseudochecker::PseudoTypeKind::Inferred => {
                let node = &t.as_pseudo_type_inferred().expression;
                let regular_type = self.ch.get_regular_type_of_expression(*node);
                let ty = self.ch.get_widened_type(regular_type);
                Some(ty)
            }
            pseudochecker::PseudoTypeKind::NoResult => None, // TODO: extract type selection logic from `serializeTypeForDeclaration`, not needed for current usecases but needed if completeness becomes required
            pseudochecker::PseudoTypeKind::MaybeConstLocation => {
                let d = t.as_pseudo_type_maybe_const_location();
                if self.ch.is_const_context(d.node) {
                    return self.pseudo_type_to_type(&d.const_type);
                }
                self.pseudo_type_to_type(&d.regular_type)
            }
            pseudochecker::PseudoTypeKind::Union => {
                let mut res = Vec::new();
                let mut has_elided_type = false;
                for m in t.as_pseudo_type_union().types.iter() {
                    if !self.ch.strict_null_checks()
                        && (m.kind == pseudochecker::PseudoTypeKind::Undefined
                            || m.kind == pseudochecker::PseudoTypeKind::Null)
                    {
                        has_elided_type = true;
                        continue;
                    }
                    let t = self.pseudo_type_to_type(m)?;
                    res.push(t);
                }
                if res.len() == 1 {
                    return Some(res[0]);
                }
                if res.is_empty() {
                    if has_elided_type {
                        return Some(self.ch.semantic_state.semantic_handles().any_type);
                    }
                    return Some(self.ch.semantic_state.semantic_handles().never_type);
                }
                Some(self.ch.get_union_type(res))
            }
            pseudochecker::PseudoTypeKind::Undefined => Some(
                self.ch
                    .semantic_state
                    .semantic_handles()
                    .undefined_widening_type,
            ),
            pseudochecker::PseudoTypeKind::Null => {
                Some(self.ch.semantic_state.semantic_handles().null_widening_type)
            }
            pseudochecker::PseudoTypeKind::Any => {
                Some(self.ch.semantic_state.semantic_handles().any_type)
            }
            pseudochecker::PseudoTypeKind::String => {
                Some(self.ch.semantic_state.semantic_handles().string_type)
            }
            pseudochecker::PseudoTypeKind::Number => {
                Some(self.ch.semantic_state.semantic_handles().number_type)
            }
            pseudochecker::PseudoTypeKind::BigInt => {
                Some(self.ch.semantic_state.semantic_handles().bigint_type)
            }
            pseudochecker::PseudoTypeKind::Boolean => {
                Some(self.ch.semantic_state.semantic_handles().boolean_type)
            }
            pseudochecker::PseudoTypeKind::False => {
                Some(self.ch.semantic_state.semantic_handles().false_type)
            }
            pseudochecker::PseudoTypeKind::True => {
                Some(self.ch.semantic_state.semantic_handles().true_type)
            }
            pseudochecker::PseudoTypeKind::StringLiteral
            | pseudochecker::PseudoTypeKind::NumericLiteral
            | pseudochecker::PseudoTypeKind::BigIntLiteral => {
                let source = &t.as_pseudo_type_literal().node;
                Some(self.ch.get_regular_type_of_expression(*source)) // big shortcut, uses cached expression types where possible
            }
            pseudochecker::PseudoTypeKind::ObjectLiteral
            | pseudochecker::PseudoTypeKind::SingleCallSignature
            | pseudochecker::PseudoTypeKind::Tuple => None, // no simple mapping to a type, since these are structural types
            _ => {
                debug::fail("Unhandled pseudochecker.PseudoTypeKind in pseudoTypeToType");
                None
            }
        }
    }
}

fn is_structural_pseudo_type(t: &pseudochecker::PseudoType) -> bool {
    match t.kind {
        pseudochecker::PseudoTypeKind::ObjectLiteral
        | pseudochecker::PseudoTypeKind::Tuple
        | pseudochecker::PseudoTypeKind::SingleCallSignature => true,
        pseudochecker::PseudoTypeKind::MaybeConstLocation => {
            let d = t.as_pseudo_type_maybe_const_location();
            is_structural_pseudo_type(&d.const_type) || is_structural_pseudo_type(&d.regular_type)
        }
        _ => false,
    }
}
