use crate::checker::*;
use crate::collections::{FastHashMap as HashMap, FastHashMapExt};
use crate::semantic::{TemplateLiteralTypeRecord, TypeMapperList};
use crate::{ast, jsnum};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct InferenceKey {
    s: TypeId,
    t: TypeId,
}

pub struct InferenceState<'b> {
    inferences: &'b mut [InferenceInfo],
    original_source: Option<TypeHandle>,
    original_target: Option<TypeHandle>,
    priority: InferencePriority,
    inference_priority: InferencePriority,
    contravariant: bool,
    bivariant: bool,
    expanding_flags: ExpandingFlags,
    propagation_type: Option<TypeHandle>,
    visited: HashMap<InferenceKey, InferencePriority>,
    source_stack: Vec<TypeHandle>,
    target_stack: Vec<TypeHandle>,
}

impl<'a, 'b> InferenceState<'b> {
    fn new(inferences: &'b mut [InferenceInfo]) -> Self {
        Self {
            inferences,
            original_source: None,
            original_target: None,
            priority: INFERENCE_PRIORITY_NONE,
            inference_priority: INFERENCE_PRIORITY_MAX_VALUE,
            contravariant: false,
            bivariant: false,
            expanding_flags: EXPANDING_FLAGS_NONE,
            propagation_type: None,
            visited: HashMap::new(),
            source_stack: Vec::new(),
            target_stack: Vec::new(),
        }
    }
}

impl<'a, 'state> Checker<'a, 'state> {
    fn get_literal_type_from_property_identity(
        &mut self,
        prop: SymbolIdentity,
        include: TypeFlags,
        include_non_public: bool,
    ) -> TypeHandle {
        let name = self.symbol_identity_name(prop).to_string();
        let modifier_flags = self.declaration_modifier_flags_from_symbol_identity(prop);
        let is_known_symbol = name
            .strip_prefix(ast::INTERNAL_SYMBOL_NAME_PREFIX)
            .is_some_and(|name| name.starts_with('@'));
        if include_non_public
            || modifier_flags & ast::ModifierFlags::NON_PUBLIC_ACCESSIBILITY_MODIFIER == 0
        {
            let late_bound_symbol = self.get_late_bound_symbol_identity(prop);
            let mut t = self
                .semantic_state
                .value_symbol_name_type(&late_bound_symbol);
            if t.is_none() {
                if name == ast::InternalSymbolName::Default {
                    t = Some(self.get_string_literal_type("default"));
                } else {
                    let value_declaration =
                        self.missing_name_symbol_identity_value_declaration(prop);
                    let name_node = value_declaration.as_ref().and_then(|value_declaration| {
                        ast::get_name_of_declaration(
                            self.store_for_node(*value_declaration),
                            Some(*value_declaration),
                        )
                    });
                    if let Some(name_node) = name_node {
                        t = Some(self.get_literal_type_from_property_name(name_node));
                    }
                    if t.is_none() && !is_known_symbol {
                        t = Some(self.get_string_literal_type(&name));
                    }
                }
            }
            if let Some(t) = t {
                if self.type_flags(t) & include != 0 {
                    return t;
                }
            }
        }
        self.semantic_state.semantic_handles().never_type
    }

    pub(crate) fn infer_types(
        &mut self,
        inferences: &mut Vec<InferenceInfo>,
        original_source: TypeHandle,
        original_target: TypeHandle,
        priority: InferencePriority,
        contravariant: bool,
    ) {
        let mut n = InferenceState::new(inferences.as_mut_slice());
        n.original_source = Some(original_source);
        n.original_target = Some(original_target);
        n.priority = priority;
        n.inference_priority = INFERENCE_PRIORITY_MAX_VALUE;
        n.contravariant = contravariant;
        self.infer_from_types(&mut n, original_source, original_target);
    }

    fn infer_from_types(
        &mut self,
        n: &mut InferenceState<'_>,
        mut source: TypeHandle,
        mut target: TypeHandle,
    ) {
        if !self.could_contain_type_variables(target) || self.is_no_infer_type(target) {
            return;
        }
        if source == self.semantic_state.semantic_handles().wildcard_type
            || source == self.semantic_state.semantic_handles().blocked_string_type
        {
            // We are inferring from an 'any' type. We want to infer this type for every type parameter
            // referenced in the target type, so we record it as the propagation type and infer from the
            // target to itself. Then, as we find candidates we substitute the propagation type.
            let save_propagation_type = n.propagation_type;
            n.propagation_type = Some(source);
            self.infer_from_types(n, target, target);
            n.propagation_type = save_propagation_type;
            return;
        }
        if self.type_alias_record(source).is_some()
            && self.type_alias_record(target).is_some()
            && self.type_alias_record(source).unwrap().symbol
                == self.type_alias_record(target).unwrap().symbol
        {
            let source_alias = self.type_alias_record(source).unwrap().clone();
            let target_alias = self.type_alias_record(target).unwrap().clone();
            let alias_symbol = source_alias
                .symbol
                .expect("matching type alias must keep alias symbol");
            if !source_alias.type_arguments.is_empty() || !target_alias.type_arguments.is_empty() {
                // Source and target are types originating in the same generic type alias declaration.
                // Simply infer from source type arguments to target type arguments, with defaults applied.
                let params = self.semantic_state.type_alias_type_parameters(alias_symbol);
                let min_params = self.get_min_type_argument_count(&params);
                let node_is_in_js_file = {
                    let declaration = self
                        .missing_name_symbol_identity_value_declaration(alias_symbol)
                        .or_else(|| {
                            self.collect_symbol_identity_declarations(alias_symbol)
                                .first()
                                .copied()
                        });
                    declaration.is_some_and(|declaration| {
                        ast::is_in_js_file(self.store_for_node(declaration), declaration)
                    })
                };
                let source_types = self.fill_missing_type_arguments(
                    source_alias.type_arguments.clone(),
                    &params,
                    min_params,
                    node_is_in_js_file,
                );
                let target_types = self.fill_missing_type_arguments(
                    target_alias.type_arguments.clone(),
                    &params,
                    min_params,
                    node_is_in_js_file,
                );
                let variances = self.get_alias_variances_identity(alias_symbol);
                self.infer_from_type_arguments(n, &source_types, &target_types, variances);
            }
            // And if there weren't any type arguments, there's no reason to run inference as the types must be the same.
            return;
        }
        if source == target && self.type_flags(source) & TYPE_FLAGS_UNION_OR_INTERSECTION != 0 {
            // When source and target are the same union or intersection type, just relate each constituent
            // type to itself.
            for t in self.type_types(source) {
                self.infer_from_types(n, t, t);
            }
            return;
        }
        if self.type_flags(target) & TYPE_FLAGS_UNION != 0 {
            let source_types = if self.type_flags(source) & TYPE_FLAGS_UNION != 0 {
                self.type_types(source)
            } else {
                vec![source]
            };
            // First, infer between identically matching source and target constituents and remove the
            // matching types.
            let (temp_sources, temp_targets) = self.infer_from_matching_types(
                n,
                source_types,
                self.distributed_types(target),
                |checker, source, target| checker.is_type_or_base_identical_to(source, target),
            );
            // Next, infer between closely matching source and target constituents and remove
            // the matching types. Types closely match when they are instantiations of the same
            // object type or instantiations of the same type alias.
            let (sources, targets) = self.infer_from_matching_types(
                n,
                temp_sources,
                temp_targets,
                |checker, source, target| checker.is_type_closely_matched_by(source, target),
            );
            if targets.is_empty() {
                return;
            }
            target = self.get_union_type(targets);
            if sources.is_empty() {
                // All source constituents have been matched and there is nothing further to infer from.
                // However, simply making no inferences is undesirable because it could ultimately mean
                // inferring a type parameter constraint. Instead, make a lower priority inference from
                // the full source to whatever remains in the target. For example, when inferring from
                // string to 'string | T', make a lower priority inference of string for T.
                self.infer_with_priority(n, source, target, INFERENCE_PRIORITY_NAKED_TYPE_VARIABLE);
                return;
            }
            source = self.get_union_type(sources);
        } else if self.type_flags(target) & TYPE_FLAGS_INTERSECTION != 0
            && !self
                .type_types(target)
                .iter()
                .all(|&t| self.is_non_generic_object_type(t))
        {
            // We reduce intersection types unless they're simple combinations of object types. For example,
            // when inferring from 'string[] & { extra: any }' to 'string[] & T' we want to remove string[] and
            // infer { extra: any } for T. But when inferring to 'string[] & Iterable<T>' we want to keep the
            // string[] on the source side and infer string for T.
            if self.type_flags(source) & TYPE_FLAGS_UNION == 0 {
                let source_types = if self.type_flags(source) & TYPE_FLAGS_INTERSECTION != 0 {
                    self.type_types(source)
                } else {
                    vec![source]
                };
                // Infer between identically matching source and target constituents and remove the matching types.
                let (sources, targets) = self.infer_from_matching_types(
                    n,
                    source_types,
                    self.type_types(target),
                    |checker, source, target| checker.is_type_identical_to(source, target),
                );
                if sources.is_empty() || targets.is_empty() {
                    return;
                }
                source = self.get_intersection_type(sources);
                target = self.get_intersection_type(targets);
            }
        }
        if self.type_flags(target) & (TYPE_FLAGS_INDEXED_ACCESS | TYPE_FLAGS_SUBSTITUTION) != 0 {
            if self.is_no_infer_type(target) {
                return;
            }
            target = self.get_actual_type_variable(target);
        }
        if self.type_flags(target) & TYPE_FLAGS_TYPE_VARIABLE != 0 {
            // Skip inference if the source is "blocked", which is used by the language service to
            // prevent inference on nodes currently being edited.
            if self.is_from_inference_blocked_source(source) {
                return;
            }
            let inference_index = self.get_inference_info_index_for_type(n, target);
            if let Some(inference_index) = inference_index {
                // If target is a type parameter, make an inference, unless the source type contains
                // a "non-inferrable" type. Types with this flag set are markers used to prevent inference.
                //
                // For example:
                //     - anyFunctionType is a wildcard type that's used to avoid contextually typing functions;
                //       it's internal, so should not be exposed to the user by adding it as a candidate.
                //     - autoType (and autoArrayType) is a special "any" used in control flow; like anyFunctionType,
                //       it's internal and should not be observable.
                //     - silentNeverType is returned by getInferredType when instantiating a generic function for
                //       inference (and a type variable has no mapping).
                //
                // This flag is infectious; if we produce Box<never> (where never is silentNeverType), Box<never> is
                // also non-inferrable.
                //
                // As a special case, also ignore nonInferrableAnyType, which is a special form of the any type
                // used as a stand-in for binding elements when they are being inferred.
                if self.object_flags(source) & OBJECT_FLAGS_NON_INFERRABLE_TYPE != 0
                    || source
                        == self
                            .semantic_state
                            .semantic_handles()
                            .non_inferrable_any_type
                {
                    return;
                }
                let mut clear_cached = false;
                {
                    let inference = &mut n.inferences[inference_index];
                    if !inference.is_fixed {
                        let candidate = n.propagation_type.unwrap_or(source);
                        if candidate == self.semantic_state.semantic_handles().blocked_string_type {
                            return;
                        }
                        if n.priority < inference.priority {
                            inference.candidates.clear();
                            inference.candidates_present = false;
                            inference.contra_candidates.clear();
                            inference.contra_candidates_present = false;
                            inference.top_level = true;
                            inference.priority = n.priority;
                        }
                        if n.priority == inference.priority {
                            // We make contravariant inferences only if we are in a pure contravariant position,
                            // i.e. only if we have not descended into a bivariant position.
                            if n.contravariant && !n.bivariant {
                                inference.contra_candidates_present = true;
                                if !inference.contra_candidates.contains(&candidate) {
                                    inference.contra_candidates.push(candidate);
                                    clear_cached = true;
                                }
                            } else {
                                inference.candidates_present = true;
                                if !inference.candidates.contains(&candidate) {
                                    inference.candidates.push(candidate);
                                    clear_cached = true;
                                }
                            }
                        }
                        if n.priority & INFERENCE_PRIORITY_RETURN_TYPE == 0
                            && self.type_flags(target) & TYPE_FLAGS_TYPE_PARAMETER != 0
                            && inference.top_level
                            && !self.is_type_parameter_at_top_level(
                                n.original_target.unwrap(),
                                target,
                                0,
                            )
                        {
                            inference.top_level = false;
                            clear_cached = true;
                        }
                    }
                }
                if clear_cached {
                    clear_cached_inferences(&mut n.inferences);
                }
                n.inference_priority = n.inference_priority.min(n.priority);
                return;
            }
            // Infer to the simplified version of an indexed access, if possible, to (hopefully) expose more bare type parameters to the inference engine
            let simplified = self.get_simplified_type(target, false /*writing*/);
            if simplified != target {
                self.infer_from_types(n, source, simplified);
            } else if self.type_flags(target) & TYPE_FLAGS_INDEXED_ACCESS != 0 {
                let index_type = self.get_simplified_type(
                    self.type_record(target)
                        .as_indexed_access_type()
                        .index_type
                        .unwrap(),
                    false, /*writing*/
                );
                // Generally simplifications of instantiable indexes are avoided to keep relationship checking correct, however if our target is an access, we can consider
                // that key of that access to be "instantiated", since we're looking to find the infernce goal in any way we can.
                if self.type_flags(index_type) & TYPE_FLAGS_INSTANTIABLE != 0 {
                    let object_type = self.get_simplified_type(
                        self.type_record(target)
                            .as_indexed_access_type()
                            .object_type
                            .unwrap(),
                        false, /*writing*/
                    );
                    let simplified = self.distribute_index_over_object_type(
                        object_type,
                        index_type,
                        false, /*writing*/
                    );
                    if simplified.is_some() && simplified.unwrap() != target {
                        self.infer_from_types(n, source, simplified.unwrap());
                    }
                }
            }
        }
        if self.object_flags(source) & OBJECT_FLAGS_REFERENCE != 0
            && self.object_flags(target) & OBJECT_FLAGS_REFERENCE != 0
            && (self
                .type_record(source)
                .as_type_reference()
                .unwrap()
                .object
                .target
                == self
                    .type_record(target)
                    .as_type_reference()
                    .unwrap()
                    .object
                    .target
                || self.is_array_type(source) && self.is_array_type(target))
            && !(self
                .type_record(source)
                .as_type_reference()
                .unwrap()
                .node
                .is_some()
                && self
                    .type_record(target)
                    .as_type_reference()
                    .unwrap()
                    .node
                    .is_some())
        {
            // If source and target are references to the same generic type, infer from type arguments
            let variances = self.get_variances(
                self.type_record(source)
                    .as_type_reference()
                    .unwrap()
                    .object
                    .target
                    .unwrap(),
            );
            self.infer_from_type_reference_arguments(n, source, target, variances);
        } else if self.type_flags(source) & TYPE_FLAGS_INDEX != 0
            && self.type_flags(target) & TYPE_FLAGS_INDEX != 0
        {
            self.infer_from_contravariant_types(
                n,
                self.type_record(source).as_index_type().target.unwrap(),
                self.type_record(target).as_index_type().target.unwrap(),
            );
        } else if (self.is_literal_type(source) || self.type_flags(source) & TYPE_FLAGS_STRING != 0)
            && self.type_flags(target) & TYPE_FLAGS_INDEX != 0
        {
            let empty = self.create_empty_object_type_from_string_literal(source);
            self.infer_from_contravariant_types_with_priority(
                n,
                empty,
                self.type_record(target).as_index_type().target.unwrap(),
                INFERENCE_PRIORITY_LITERAL_KEYOF,
            );
        } else if self.type_flags(source) & TYPE_FLAGS_INDEXED_ACCESS != 0
            && self.type_flags(target) & TYPE_FLAGS_INDEXED_ACCESS != 0
        {
            self.infer_from_types(
                n,
                self.type_record(source)
                    .as_indexed_access_type()
                    .object_type
                    .unwrap(),
                self.type_record(target)
                    .as_indexed_access_type()
                    .object_type
                    .unwrap(),
            );
            self.infer_from_types(
                n,
                self.type_record(source)
                    .as_indexed_access_type()
                    .index_type
                    .unwrap(),
                self.type_record(target)
                    .as_indexed_access_type()
                    .index_type
                    .unwrap(),
            );
        } else if self.type_flags(source) & TYPE_FLAGS_STRING_MAPPING != 0
            && self.type_flags(target) & TYPE_FLAGS_STRING_MAPPING != 0
        {
            if self.same_optional_symbol_identity(
                self.type_symbol_identity(source),
                self.type_symbol_identity(target),
            ) {
                self.infer_from_types(
                    n,
                    self.type_record(source)
                        .as_string_mapping_type()
                        .target
                        .unwrap(),
                    self.type_record(target)
                        .as_string_mapping_type()
                        .target
                        .unwrap(),
                );
            }
        } else if self.type_flags(source) & TYPE_FLAGS_SUBSTITUTION != 0 {
            self.infer_from_types(
                n,
                self.type_record(source)
                    .as_substitution_type()
                    .base_type
                    .unwrap(),
                target,
            );
            // Make substitute inference at a lower priority
            let substitution = self.get_substitution_intersection(source);
            self.infer_with_priority(
                n,
                substitution,
                target,
                INFERENCE_PRIORITY_SUBSTITUTE_SOURCE,
            );
        } else if self.type_flags(target) & TYPE_FLAGS_CONDITIONAL != 0 {
            self.invoke_once(n, source, target, |checker, state, source, target| {
                checker.infer_to_conditional_type(state, source, target)
            });
        } else if self.type_flags(target) & TYPE_FLAGS_UNION_OR_INTERSECTION != 0 {
            self.infer_to_multiple_types(
                n,
                source,
                self.type_types(target),
                self.type_flags(target),
            );
        } else if self.type_flags(source) & TYPE_FLAGS_UNION != 0 {
            // Source is a union or intersection type, infer from each constituent type
            for source_type in self.type_types(source) {
                self.infer_from_types(n, source_type, target);
            }
        } else if self.type_flags(target) & TYPE_FLAGS_TEMPLATE_LITERAL != 0 {
            let target_record = self.type_record(target).as_template_literal_type().clone();
            self.infer_to_template_literal_type(n, source, &target_record);
        } else {
            source = self.get_reduced_type(source);
            if self.is_generic_mapped_type(source) && self.is_generic_mapped_type(target) {
                self.invoke_once(n, source, target, |checker, state, source, target| {
                    checker.infer_from_generic_mapped_types(state, source, target)
                });
            }
            if !(n.priority & INFERENCE_PRIORITY_NO_CONSTRAINTS != 0
                && self.type_flags(source) & (TYPE_FLAGS_INTERSECTION | TYPE_FLAGS_INSTANTIABLE)
                    != 0)
            {
                let apparent_source = self.get_apparent_type(source);
                // getApparentType can return _any_ type, since an indexed access or conditional may simplify to any other type.
                // If that occurs and it doesn't simplify to an object or intersection, we'll need to restart `inferFromTypes`
                // with the simplified source.
                if apparent_source != source
                    && self.type_flags(apparent_source)
                        & (TYPE_FLAGS_OBJECT | TYPE_FLAGS_INTERSECTION)
                        == 0
                {
                    self.infer_from_types(n, apparent_source, target);
                    return;
                }
                source = apparent_source;
            }
            if self.type_flags(source) & (TYPE_FLAGS_OBJECT | TYPE_FLAGS_INTERSECTION) != 0 {
                self.invoke_once(n, source, target, |checker, state, source, target| {
                    checker.infer_from_object_types(state, source, target)
                });
            }
        }
    }

    fn infer_from_type_arguments(
        &mut self,
        n: &mut InferenceState<'_>,
        source_types: &[TypeHandle],
        target_types: &[TypeHandle],
        variances: Vec<VarianceFlags>,
    ) {
        for i in 0..source_types.len().min(target_types.len()) {
            if i < variances.len()
                && variances[i] & VARIANCE_FLAGS_VARIANCE_MASK == VARIANCE_FLAGS_CONTRAVARIANT
            {
                self.infer_from_contravariant_types(n, source_types[i], target_types[i]);
            } else {
                self.infer_from_types(n, source_types[i], target_types[i]);
            }
        }
    }

    fn infer_from_type_reference_arguments(
        &mut self,
        n: &mut InferenceState<'_>,
        source: TypeHandle,
        target: TypeHandle,
        variances: Vec<VarianceFlags>,
    ) {
        let source_type_arguments = self.ensure_type_arguments_available(source);
        let target_type_arguments = self.ensure_type_arguments_available(target);
        let source_len = self.type_arguments_len_from(source, source_type_arguments.as_deref());
        let target_len = self.type_arguments_len_from(target, target_type_arguments.as_deref());
        for i in 0..source_len.min(target_len) {
            let source_type =
                self.type_argument_at_from(source, source_type_arguments.as_deref(), i);
            let target_type =
                self.type_argument_at_from(target, target_type_arguments.as_deref(), i);
            if i < variances.len()
                && variances[i] & VARIANCE_FLAGS_VARIANCE_MASK == VARIANCE_FLAGS_CONTRAVARIANT
            {
                self.infer_from_contravariant_types(n, source_type, target_type);
            } else {
                self.infer_from_types(n, source_type, target_type);
            }
        }
    }

    fn infer_with_priority(
        &mut self,
        n: &mut InferenceState<'_>,
        source: TypeHandle,
        target: TypeHandle,
        new_priority: InferencePriority,
    ) {
        let save_priority = n.priority;
        n.priority |= new_priority;
        self.infer_from_types(n, source, target);
        n.priority = save_priority;
    }

    fn infer_from_contravariant_types_with_priority(
        &mut self,
        n: &mut InferenceState<'_>,
        source: TypeHandle,
        target: TypeHandle,
        new_priority: InferencePriority,
    ) {
        let save_priority = n.priority;
        n.priority |= new_priority;
        self.infer_from_contravariant_types(n, source, target);
        n.priority = save_priority;
    }

    fn infer_from_contravariant_types(
        &mut self,
        n: &mut InferenceState<'_>,
        source: TypeHandle,
        target: TypeHandle,
    ) {
        n.contravariant = !n.contravariant;
        self.infer_from_types(n, source, target);
        n.contravariant = !n.contravariant;
    }

    fn infer_from_contravariant_types_if_strict_function_types(
        &mut self,
        n: &mut InferenceState<'_>,
        source: TypeHandle,
        target: TypeHandle,
    ) {
        if self.strict_function_types() || n.priority & INFERENCE_PRIORITY_ALWAYS_STRICT != 0 {
            self.infer_from_contravariant_types(n, source, target);
        } else {
            self.infer_from_types(n, source, target);
        }
    }

    // Ensure an inference action is performed only once for the given source and target types.
    // This includes two things:
    // Avoiding inferring between the same pair of source and target types,
    // and avoiding circularly inferring between source and target types.
    // For an example of the last, consider if we are inferring between source type
    // `type Deep<T> = { next: Deep<Deep<T>> }` and target type `type Loop<U> = { next: Loop<U> }`.
    // We would then infer between the types of the `next` property: `Deep<Deep<T>>` = `{ next: Deep<Deep<Deep<T>>> }` and `Loop<U>` = `{ next: Loop<U> }`.
    // We will then infer again between the types of the `next` property:
    // `Deep<Deep<Deep<T>>>` and `Loop<U>`, and so on, such that we would be forever inferring
    // between instantiations of the same types `Deep` and `Loop`.
    // In particular, we would be inferring from increasingly deep instantiations of `Deep` to `Loop`,
    // such that we would go on inferring forever, even though we would never infer
    // between the same pair of types.
    fn invoke_once(
        &mut self,
        n: &mut InferenceState<'_>,
        source: TypeHandle,
        target: TypeHandle,
        action: fn(&mut Checker<'a, 'state>, &mut InferenceState<'_>, TypeHandle, TypeHandle),
    ) {
        let key = InferenceKey {
            s: self.type_id(source),
            t: self.type_id(target),
        };
        if let Some(status) = n.visited.get(&key) {
            n.inference_priority = n.inference_priority.min(*status);
            return;
        }
        n.visited.insert(key, INFERENCE_PRIORITY_CIRCULARITY);
        let save_inference_priority = n.inference_priority;
        n.inference_priority = INFERENCE_PRIORITY_MAX_VALUE;
        // We stop inferring and report a circularity if we encounter duplicate recursion identities on both
        // the source side and the target side.
        let save_expanding_flags = n.expanding_flags;
        n.source_stack.push(source);
        n.target_stack.push(target);
        if self.is_deeply_nested_type(source, &n.source_stack, 2) {
            n.expanding_flags |= EXPANDING_FLAGS_SOURCE;
        }
        if self.is_deeply_nested_type(target, &n.target_stack, 2) {
            n.expanding_flags |= EXPANDING_FLAGS_TARGET;
        }
        if n.expanding_flags != EXPANDING_FLAGS_BOTH {
            action(self, n, source, target);
        } else {
            n.inference_priority = INFERENCE_PRIORITY_CIRCULARITY;
        }
        n.target_stack.pop();
        n.source_stack.pop();
        n.expanding_flags = save_expanding_flags;
        n.visited.insert(key, n.inference_priority);
        n.inference_priority = n.inference_priority.min(save_inference_priority);
    }

    fn infer_from_matching_types(
        &mut self,
        n: &mut InferenceState<'_>,
        mut sources: Vec<TypeHandle>,
        mut targets: Vec<TypeHandle>,
        matches: fn(&mut Checker<'a, 'state>, TypeHandle, TypeHandle) -> bool,
    ) -> (Vec<TypeHandle>, Vec<TypeHandle>) {
        let mut matched_sources = Vec::new();
        let mut matched_targets = Vec::new();
        for &t in &targets {
            for &s in &sources {
                if matches(self, s, t) {
                    self.infer_from_types(n, s, t);
                    if !matched_sources.contains(&s) {
                        matched_sources.push(s);
                    }
                    if !matched_targets.contains(&t) {
                        matched_targets.push(t);
                    }
                }
            }
        }
        if !matched_sources.is_empty() {
            sources.retain(|t| !matched_sources.contains(t));
        }
        if !matched_targets.is_empty() {
            targets.retain(|t| !matched_targets.contains(t));
        }
        (sources, targets)
    }

    fn infer_to_multiple_types(
        &mut self,
        n: &mut InferenceState<'_>,
        source: TypeHandle,
        targets: Vec<TypeHandle>,
        target_flags: TypeFlags,
    ) {
        let mut type_variable_count = 0;
        if target_flags & TYPE_FLAGS_UNION != 0 {
            let mut naked_type_variable = None;
            let sources = if self.type_flags(source) & TYPE_FLAGS_UNION != 0 {
                self.type_types(source)
            } else {
                vec![source]
            };
            let mut matched = vec![false; sources.len()];
            let mut inference_circularity = false;
            // First infer to types that are not naked type variables. For each source type we
            // track whether inferences were made from that particular type to some target with
            // equal priority (i.e. of equal quality) to what we would infer for a naked type
            // parameter.
            for t in &targets {
                if self.get_inference_info_for_type(n, *t).is_some() {
                    naked_type_variable = Some(*t);
                    type_variable_count += 1;
                } else {
                    for i in 0..sources.len() {
                        let save_inference_priority = n.inference_priority;
                        n.inference_priority = INFERENCE_PRIORITY_MAX_VALUE;
                        self.infer_from_types(n, sources[i], *t);
                        if n.inference_priority == n.priority {
                            matched[i] = true;
                        }
                        inference_circularity = inference_circularity
                            || n.inference_priority == INFERENCE_PRIORITY_CIRCULARITY;
                        n.inference_priority = n.inference_priority.min(save_inference_priority);
                    }
                }
            }
            if type_variable_count == 0 {
                // If every target is an intersection of types containing a single naked type variable,
                // make a lower priority inference to that type variable. This handles inferring from
                // 'A | B' to 'T & (X | Y)' where we want to infer 'A | B' for T.
                if let Some(intersection_type_variable) =
                    self.get_single_type_variable_from_intersection_types(n, &targets)
                {
                    self.infer_with_priority(
                        n,
                        source,
                        intersection_type_variable,
                        INFERENCE_PRIORITY_NAKED_TYPE_VARIABLE,
                    );
                }
                return;
            }
            // If the target has a single naked type variable and no inference circularities were
            // encountered above (meaning we explored the types fully), create a union of the source
            // types from which no inferences have been made so far and infer from that union to the
            // naked type variable.
            if type_variable_count == 1 && !inference_circularity {
                let unmatched: Vec<_> = sources
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &s)| if !matched[i] { Some(s) } else { None })
                    .collect();
                if !unmatched.is_empty() {
                    let unmatched_type = self.get_union_type(unmatched);
                    self.infer_from_types(n, unmatched_type, naked_type_variable.unwrap());
                    return;
                }
            }
        } else {
            // We infer from types that are not naked type variables first so that inferences we
            // make from nested naked type variables and given slightly higher priority by virtue
            // of being first in the candidates array.
            for t in &targets {
                if self.get_inference_info_for_type(n, *t).is_some() {
                    type_variable_count += 1;
                } else {
                    self.infer_from_types(n, source, *t);
                }
            }
        }
        // Inferences directly to naked type variables are given lower priority as they are
        // less specific. For example, when inferring from Promise<string> to T | Promise<T>,
        // we want to infer string for T, not Promise<string> | string. For intersection types
        // we only infer to single naked type variables.
        if target_flags & TYPE_FLAGS_INTERSECTION != 0 && type_variable_count == 1
            || target_flags & TYPE_FLAGS_INTERSECTION == 0 && type_variable_count > 0
        {
            for t in targets {
                if self.get_inference_info_for_type(n, t).is_some() {
                    self.infer_with_priority(n, source, t, INFERENCE_PRIORITY_NAKED_TYPE_VARIABLE);
                }
            }
        }
    }

    fn infer_to_multiple_types_with_priority(
        &mut self,
        n: &mut InferenceState<'_>,
        source: TypeHandle,
        targets: Vec<TypeHandle>,
        target_flags: TypeFlags,
        new_priority: InferencePriority,
    ) {
        let save_priority = n.priority;
        n.priority |= new_priority;
        self.infer_to_multiple_types(n, source, targets, target_flags);
        n.priority = save_priority;
    }

    fn infer_to_conditional_type(
        &mut self,
        n: &mut InferenceState<'_>,
        source: TypeHandle,
        target: TypeHandle,
    ) {
        if self.type_flags(source) & TYPE_FLAGS_CONDITIONAL != 0 {
            let source_true_type = self.get_true_type_from_conditional_type(source);
            let target_true_type = self.get_true_type_from_conditional_type(target);
            let source_false_type = self.get_false_type_from_conditional_type(source);
            let target_false_type = self.get_false_type_from_conditional_type(target);
            self.infer_from_types(
                n,
                self.type_record(source)
                    .as_conditional_type()
                    .check_type
                    .unwrap(),
                self.type_record(target)
                    .as_conditional_type()
                    .check_type
                    .unwrap(),
            );
            self.infer_from_types(
                n,
                self.type_record(source)
                    .as_conditional_type()
                    .extends_type
                    .unwrap(),
                self.type_record(target)
                    .as_conditional_type()
                    .extends_type
                    .unwrap(),
            );
            self.infer_from_types(n, source_true_type, target_true_type);
            self.infer_from_types(n, source_false_type, target_false_type);
        } else {
            let target_types = vec![
                self.get_true_type_from_conditional_type(target),
                self.get_false_type_from_conditional_type(target),
            ];
            self.infer_to_multiple_types_with_priority(
                n,
                source,
                target_types,
                self.type_flags(target),
                if n.contravariant {
                    INFERENCE_PRIORITY_CONTRAVARIANT_CONDITIONAL
                } else {
                    0
                },
            );
        }
    }

    fn infer_to_template_literal_type(
        &mut self,
        n: &mut InferenceState<'_>,
        source: TypeHandle,
        target: &TemplateLiteralTypeRecord,
    ) {
        let matches = self.infer_types_from_template_literal_type(source, target);
        let has_matches = matches.as_ref().is_some_and(|matches| !matches.is_empty());
        let types = &target.types;
        // When the target template literal contains only placeholders (meaning that inference is intended to extract
        // single characters and remainder strings) and inference fails to produce matches, we want to infer 'never' for
        // each placeholder such that instantiation with the inferred value(s) produces 'never', a type for which an
        // assignment check will fail. If we make no inferences, we'll likely end up with the constraint 'string' which,
        // upon instantiation, would collapse all the placeholders to just 'string', and an assignment check might
        // succeed. That would be a pointless and confusing outcome.
        if has_matches || target.texts.iter().all(|s| s.is_empty()) {
            for (i, &target_type) in types.iter().enumerate() {
                let source_type = if has_matches {
                    matches.as_ref().unwrap()[i]
                } else {
                    self.semantic_state.semantic_handles().never_type
                };
                // If we are inferring from a string literal type to a type variable whose constraint includes one of the
                // allowed template literal placeholder types, infer from a literal type corresponding to the constraint.
                if self.type_flags(source_type) & TYPE_FLAGS_STRING_LITERAL != 0
                    && self.type_flags(target_type) & TYPE_FLAGS_TYPE_VARIABLE != 0
                {
                    if let Some(inference_context) =
                        self.get_inference_info_for_type(n, target_type)
                    {
                        if let Some(constraint) =
                            self.get_base_constraint_of_type(inference_context.type_parameter)
                        {
                            if !is_type_any(self, Some(constraint)) {
                                let mut all_type_flags = TYPE_FLAGS_NONE;
                                for t in self.distributed_types(constraint) {
                                    all_type_flags |= self.type_flags(t);
                                }
                                // If the constraint contains `string`, we don't need to look for a more preferred type
                                if all_type_flags & TYPE_FLAGS_STRING == 0 {
                                    let str_value = self.get_string_literal_value(source_type);
                                    // If the type contains `number` or a number literal and the string isn't a valid number, exclude numbers
                                    if all_type_flags & TYPE_FLAGS_NUMBER_LIKE != 0
                                        && !is_valid_number_string(
                                            &str_value, true, /*roundTripOnly*/
                                        )
                                    {
                                        all_type_flags &= !TYPE_FLAGS_NUMBER_LIKE;
                                    }
                                    // If the type contains `bigint` or a bigint literal and the string isn't a valid bigint, exclude bigints
                                    if all_type_flags & TYPE_FLAGS_BIG_INT_LIKE != 0
                                        && !is_valid_big_int_string(
                                            &str_value, true, /*roundTripOnly*/
                                        )
                                    {
                                        all_type_flags &= !TYPE_FLAGS_BIG_INT_LIKE;
                                    }
                                    let mut matching_type =
                                        self.semantic_state.semantic_handles().never_type;
                                    for t in self.distributed_types(constraint) {
                                        matching_type = self.choose_template_literal_match(
                                            matching_type,
                                            t,
                                            source_type,
                                            all_type_flags,
                                            &str_value,
                                        );
                                    }
                                    if self.type_flags(matching_type) & TYPE_FLAGS_NEVER == 0 {
                                        self.infer_from_types(n, matching_type, target_type);
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                }
                self.infer_from_types(n, source_type, target_type);
            }
        }
    }

    fn choose_template_literal_match(
        &mut self,
        left: TypeHandle,
        right: TypeHandle,
        source: TypeHandle,
        all_type_flags: TypeFlags,
        str_value: &str,
    ) -> TypeHandle {
        let right_template_literal = if self.type_flags(right) & TYPE_FLAGS_TEMPLATE_LITERAL != 0 {
            Some(self.type_record(right).as_template_literal_type().clone())
        } else {
            None
        };
        match () {
            _ if self.type_flags(right) & all_type_flags == 0 => left,
            _ if self.type_flags(left) & TYPE_FLAGS_STRING != 0 => left,
            _ if self.type_flags(right) & TYPE_FLAGS_STRING != 0 => source,
            _ if self.type_flags(left) & TYPE_FLAGS_TEMPLATE_LITERAL != 0 => left,
            _ if self.type_flags(right) & TYPE_FLAGS_TEMPLATE_LITERAL != 0
                && self.is_type_matched_by_template_literal_type(
                    source,
                    right_template_literal.as_ref().unwrap(),
                    self.semantic_state.compare_types_assignable,
                ) =>
            {
                source
            }
            _ if self.type_flags(left) & TYPE_FLAGS_STRING_MAPPING != 0 => left,
            _ if self.type_flags(right) & TYPE_FLAGS_STRING_MAPPING != 0 && {
                let symbol = self
                    .type_symbol_identity(right)
                    .expect("string mapping type must have symbol identity");
                let symbol_name = self.symbol_identity_name(symbol);
                str_value == apply_string_mapping_by_name(&symbol_name, str_value)
            } =>
            {
                source
            }
            _ if self.type_flags(left) & TYPE_FLAGS_STRING_LITERAL != 0 => left,
            _ if self.type_flags(right) & TYPE_FLAGS_STRING_LITERAL != 0
                && self.get_string_literal_value(right) == str_value =>
            {
                right
            }
            _ if self.type_flags(left) & TYPE_FLAGS_NUMBER != 0 => left,
            _ if self.type_flags(right) & TYPE_FLAGS_NUMBER != 0 => {
                self.get_number_literal_type(jsnum::from_string(str_value))
            }
            _ if self.type_flags(left) & TYPE_FLAGS_ENUM != 0 => left,
            _ if self.type_flags(right) & TYPE_FLAGS_ENUM != 0 => {
                self.get_number_literal_type(jsnum::from_string(str_value))
            }
            _ if self.type_flags(left) & TYPE_FLAGS_NUMBER_LITERAL != 0 => left,
            _ if self.type_flags(right) & TYPE_FLAGS_NUMBER_LITERAL != 0
                && self.get_number_literal_value(right) == jsnum::from_string(str_value) =>
            {
                right
            }
            _ if self.type_flags(left) & TYPE_FLAGS_BIG_INT != 0 => left,
            _ if self.type_flags(right) & TYPE_FLAGS_BIG_INT != 0 => {
                self.parse_big_int_literal_type(str_value)
            }
            _ if self.type_flags(left) & TYPE_FLAGS_BIG_INT_LITERAL != 0 => left,
            _ if self.type_flags(right) & TYPE_FLAGS_BIG_INT_LITERAL != 0
                && pseudo_big_int_to_string(self.get_big_int_literal_value(right)) == str_value =>
            {
                right
            }
            _ if self.type_flags(left) & TYPE_FLAGS_BOOLEAN != 0 => left,
            _ if self.type_flags(right) & TYPE_FLAGS_BOOLEAN != 0 => match str_value {
                "true" => self.semantic_state.semantic_handles().true_type,
                "false" => self.semantic_state.semantic_handles().false_type,
                _ => self.semantic_state.semantic_handles().boolean_type,
            },
            _ if self.type_flags(left) & TYPE_FLAGS_BOOLEAN_LITERAL != 0 => left,
            _ if self.type_flags(right) & TYPE_FLAGS_BOOLEAN_LITERAL != 0
                && if self.get_boolean_literal_value(right) {
                    "true"
                } else {
                    "false"
                } == str_value =>
            {
                right
            }
            _ if self.type_flags(left) & TYPE_FLAGS_UNDEFINED != 0 => left,
            _ if self.type_flags(right) & TYPE_FLAGS_UNDEFINED != 0
                && self.type_record(right).as_intrinsic_type().intrinsic_name == str_value =>
            {
                right
            }
            _ if self.type_flags(left) & TYPE_FLAGS_NULL != 0 => left,
            _ if self.type_flags(right) & TYPE_FLAGS_NULL != 0
                && self.type_record(right).as_intrinsic_type().intrinsic_name == str_value =>
            {
                right
            }
            _ => left,
        }
    }

    fn infer_from_generic_mapped_types(
        &mut self,
        n: &mut InferenceState<'_>,
        source: TypeHandle,
        target: TypeHandle,
    ) {
        // The source and target types are generic types { [P in S]: X } and { [P in T]: Y }, so we infer
        // from S to T and from X to Y.
        let source_constraint = self.get_constraint_type_from_mapped_type(source);
        let target_constraint = self.get_constraint_type_from_mapped_type(target);
        self.infer_from_types(n, source_constraint, target_constraint);
        let source_template = self.get_template_type_from_mapped_type(source);
        let target_template = self.get_template_type_from_mapped_type(target);
        self.infer_from_types(n, source_template, target_template);
        let source_name_type = self.get_name_type_from_mapped_type(source);
        let target_name_type = self.get_name_type_from_mapped_type(target);
        if let (Some(source_name_type), Some(target_name_type)) =
            (source_name_type, target_name_type)
        {
            self.infer_from_types(n, source_name_type, target_name_type);
        }
    }

    fn infer_from_object_types(
        &mut self,
        n: &mut InferenceState<'_>,
        source: TypeHandle,
        target: TypeHandle,
    ) {
        if self.object_flags(source) & OBJECT_FLAGS_REFERENCE != 0
            && self.object_flags(target) & OBJECT_FLAGS_REFERENCE != 0
            && (self.type_target(source) == self.type_target(target)
                || self.is_array_type(source) && self.is_array_type(target))
        {
            // If source and target are references to the same generic type, infer from type arguments
            let variances = self.get_variances(self.type_target(source));
            self.infer_from_type_reference_arguments(n, source, target, variances);
            return;
        }
        if self.is_generic_mapped_type(source) && self.is_generic_mapped_type(target) {
            self.infer_from_generic_mapped_types(n, source, target);
        }
        if self.object_flags(target) & OBJECT_FLAGS_MAPPED != 0 && {
            let declaration = self
                .type_record(target)
                .as_mapped_type()
                .declaration
                .unwrap();
            self.store_for_node(declaration)
                .name_type(declaration)
                .is_none()
        } {
            let constraint_type = self.get_constraint_type_from_mapped_type(target);
            if self.infer_to_mapped_type(n, source, target, constraint_type) {
                return;
            }
        }
        // Infer from the members of source and target only if the two types are possibly related
        if self.types_definitely_unrelated(source, target) {
            return;
        }
        if self.is_array_or_tuple_type(source) {
            if self.is_tuple_type(target) {
                self.infer_from_tuple_to_tuple(n, source, target);
                return;
            }
            if self.is_array_type(target) {
                self.infer_from_index_types(n, source, target);
                return;
            }
        }
        self.infer_from_properties(n, source, target);
        self.infer_from_signatures(n, source, target, SIGNATURE_KIND_CALL);
        self.infer_from_signatures(n, source, target, SIGNATURE_KIND_CONSTRUCT);
        self.infer_from_index_types(n, source, target);
    }

    fn infer_from_tuple_to_tuple(
        &mut self,
        n: &mut InferenceState<'_>,
        source: TypeHandle,
        target: TypeHandle,
    ) {
        let source_arity = self.get_type_reference_arity(source);
        let target_arity = self.get_type_reference_arity(target);
        let source_type_arguments = self.ensure_type_arguments_available(source);
        let element_types = self.ensure_type_arguments_available(target);
        let element_infos = self.target_tuple_type_record(target).element_infos.clone();
        // When source and target are tuple types with the same structure (fixed, variadic, and rest are matched
        // to the same kind in each position), simply infer between the element types.
        if self.is_tuple_type(source) && self.is_tuple_type_structure_matching(source, target) {
            for i in 0..target_arity {
                let source_type =
                    self.type_argument_at_from(source, source_type_arguments.as_deref(), i);
                let target_type = self.type_argument_at_from(target, element_types.as_deref(), i);
                self.infer_from_types(n, source_type, target_type);
            }
            return;
        }
        let mut start_length = 0;
        let mut end_length = 0;
        if self.is_tuple_type(source) {
            start_length = self
                .target_tuple_type_record(source)
                .fixed_length
                .min(self.target_tuple_type_record(target).fixed_length);
            if self.target_tuple_type_record(target).combined_flags & ELEMENT_FLAGS_VARIABLE != 0 {
                end_length = get_end_element_count(
                    self.target_tuple_type_record(source),
                    ELEMENT_FLAGS_FIXED,
                )
                .min(get_end_element_count(
                    self.target_tuple_type_record(target),
                    ELEMENT_FLAGS_FIXED,
                ));
            }
        }
        // Infer between starting fixed elements.
        for i in 0..start_length {
            let source_type =
                self.type_argument_at_from(source, source_type_arguments.as_deref(), i);
            let target_type = self.type_argument_at_from(target, element_types.as_deref(), i);
            self.infer_from_types(n, source_type, target_type);
        }
        if !self.is_tuple_type(source)
            || source_arity - start_length - end_length == 1
                && self.target_tuple_type_record(source).element_infos[start_length].flags
                    & ELEMENT_FLAGS_REST
                    != 0
        {
            // Single rest element remains in source, infer from that to every element in target
            let rest_type =
                self.type_argument_at_from(source, source_type_arguments.as_deref(), start_length);
            for i in start_length..target_arity - end_length {
                let t = if element_infos[i].flags & ELEMENT_FLAGS_VARIADIC != 0 {
                    self.create_array_type(rest_type)
                } else {
                    rest_type
                };
                let target_type = self.type_argument_at_from(target, element_types.as_deref(), i);
                self.infer_from_types(n, t, target_type);
            }
        } else {
            let middle_length = target_arity - start_length - end_length;
            if middle_length == 2 {
                if element_infos[start_length].flags
                    & element_infos[start_length + 1].flags
                    & ELEMENT_FLAGS_VARIADIC
                    != 0
                {
                    // Middle of target is [...T, ...U] and source is tuple type
                    let first_target_type =
                        self.type_argument_at_from(target, element_types.as_deref(), start_length);
                    if let Some(target_info) =
                        self.get_inference_info_for_type(n, first_target_type)
                    {
                        let implied_arity = target_info.implied_arity;
                        if implied_arity >= 0 {
                            // Infer slices from source based on implied arity of T.
                            let first_slice = self.slice_tuple_type(
                                source,
                                start_length,
                                end_length as isize + source_arity as isize - implied_arity,
                            );
                            self.infer_from_types(n, first_slice, first_target_type);
                            let second_slice = self.slice_tuple_type(
                                source,
                                start_length + implied_arity as usize,
                                end_length as isize,
                            );
                            let second_target_type = self.type_argument_at_from(
                                target,
                                element_types.as_deref(),
                                start_length + 1,
                            );
                            self.infer_from_types(n, second_slice, second_target_type);
                        }
                    }
                } else if element_infos[start_length].flags & ELEMENT_FLAGS_VARIADIC != 0
                    && element_infos[start_length + 1].flags & ELEMENT_FLAGS_REST != 0
                {
                    // Middle of target is [...T, ...rest] and source is tuple type
                    // if T is constrained by a fixed-size tuple we might be able to use its arity to infer T
                    let target_type =
                        self.type_argument_at_from(target, element_types.as_deref(), start_length);
                    if let Some(info) = self.get_inference_info_for_type(n, target_type) {
                        let constraint = self.get_base_constraint_of_type(info.type_parameter);
                        if constraint.is_some_and(|constraint| self.is_tuple_type(constraint))
                            && self
                                .target_tuple_type_record(constraint.unwrap())
                                .combined_flags
                                & ELEMENT_FLAGS_VARIABLE
                                == 0
                        {
                            let implied_arity = self
                                .target_tuple_type_record(constraint.unwrap())
                                .fixed_length;
                            let variadic_slice = self.slice_tuple_type(
                                source,
                                start_length,
                                source_arity as isize - (start_length + implied_arity) as isize,
                            );
                            let rest_slice = self
                                .get_element_type_of_slice_of_tuple_type(
                                    source,
                                    start_length + implied_arity,
                                    end_length,
                                    false,
                                    false,
                                )
                                .unwrap();
                            self.infer_from_types(n, variadic_slice, target_type);
                            let rest_target_type = self.type_argument_at_from(
                                target,
                                element_types.as_deref(),
                                start_length + 1,
                            );
                            self.infer_from_types(n, rest_slice, rest_target_type);
                        }
                    }
                } else if element_infos[start_length].flags & ELEMENT_FLAGS_REST != 0
                    && element_infos[start_length + 1].flags & ELEMENT_FLAGS_VARIADIC != 0
                {
                    // Middle of target is [...rest, ...T] and source is tuple type
                    // if T is constrained by a fixed-size tuple we might be able to use its arity to infer T
                    let trailing_target_type = self.type_argument_at_from(
                        target,
                        element_types.as_deref(),
                        start_length + 1,
                    );
                    if let Some(info) = self.get_inference_info_for_type(n, trailing_target_type) {
                        let constraint = self.get_base_constraint_of_type(info.type_parameter);
                        if constraint.is_some_and(|constraint| self.is_tuple_type(constraint))
                            && self
                                .target_tuple_type_record(constraint.unwrap())
                                .combined_flags
                                & ELEMENT_FLAGS_VARIABLE
                                == 0
                        {
                            let implied_arity = self
                                .target_tuple_type_record(constraint.unwrap())
                                .fixed_length;
                            let end_index = source_arity
                                - get_end_element_count(
                                    self.target_tuple_type_record(target),
                                    ELEMENT_FLAGS_FIXED,
                                );
                            let start_index = end_index - implied_arity;
                            let mut trailing_type_arguments =
                                Vec::with_capacity(end_index - start_index);
                            for i in start_index..end_index {
                                trailing_type_arguments.push(self.type_argument_at_from(
                                    source,
                                    source_type_arguments.as_deref(),
                                    i,
                                ));
                            }
                            let trailing_slice = self.create_tuple_type_ex(
                                trailing_type_arguments,
                                self.target_tuple_type_record(source).element_infos
                                    [start_index..end_index]
                                    .to_vec(),
                                false, /*readonly*/
                            );
                            let rest_slice = self
                                .get_element_type_of_slice_of_tuple_type(
                                    source,
                                    start_length,
                                    end_length + implied_arity,
                                    false,
                                    false,
                                )
                                .unwrap();
                            let rest_target_type = self.type_argument_at_from(
                                target,
                                element_types.as_deref(),
                                start_length,
                            );
                            self.infer_from_types(n, rest_slice, rest_target_type);
                            self.infer_from_types(n, trailing_slice, trailing_target_type);
                        }
                    }
                }
            } else if middle_length == 1
                && element_infos[start_length].flags & ELEMENT_FLAGS_VARIADIC != 0
            {
                // Middle of target is exactly one variadic element. Infer the slice between the fixed parts in the source.
                // If target ends in optional element(s), make a lower priority a speculative inference.
                let priority =
                    if element_infos[target_arity - 1].flags & ELEMENT_FLAGS_OPTIONAL != 0 {
                        INFERENCE_PRIORITY_SPECULATIVE_TUPLE
                    } else {
                        0
                    };
                let source_slice = self.slice_tuple_type(source, start_length, end_length as isize);
                let target_type =
                    self.type_argument_at_from(target, element_types.as_deref(), start_length);
                self.infer_with_priority(n, source_slice, target_type, priority);
            } else if middle_length == 1
                && element_infos[start_length].flags & ELEMENT_FLAGS_REST != 0
            {
                // Middle of target is exactly one rest element. If middle of source is not empty, infer union of middle element types.
                if let Some(rest_type) = self.get_element_type_of_slice_of_tuple_type(
                    source,
                    start_length,
                    end_length,
                    false,
                    false,
                ) {
                    let target_type =
                        self.type_argument_at_from(target, element_types.as_deref(), start_length);
                    self.infer_from_types(n, rest_type, target_type);
                }
            }
        }
        // Infer between ending fixed elements
        for i in 0..end_length {
            let source_type = self.type_argument_at_from(
                source,
                source_type_arguments.as_deref(),
                source_arity - i - 1,
            );
            let target_type =
                self.type_argument_at_from(target, element_types.as_deref(), target_arity - i - 1);
            self.infer_from_types(n, source_type, target_type);
        }
    }

    fn infer_from_properties(
        &mut self,
        n: &mut InferenceState<'_>,
        source: TypeHandle,
        target: TypeHandle,
    ) {
        let properties = self.get_properties_of_object_type(target);
        for target_prop in properties {
            let target_prop_name = self.symbol_identity_name(target_prop).to_string();
            let source_prop = self.get_property_of_type(source, &target_prop_name);
            if let Some(source_prop) = source_prop {
                let skip_direct_inference = {
                    self.collect_symbol_identity_declarations(source_prop)
                        .into_iter()
                        .any(|d| self.is_skip_direct_inference_node(d))
                };
                if skip_direct_inference {
                    continue;
                }
                let source_type = self.get_type_of_symbol_identity(source_prop);
                let source_optional = self
                    .missing_name_symbol_identity_flags(source_prop)
                    .intersects(ast::SYMBOL_FLAGS_OPTIONAL);
                let source_type = self.remove_missing_type(source_type, source_optional);
                let target_type = self.get_type_of_symbol_identity(target_prop);
                let target_optional = self
                    .missing_name_symbol_identity_flags(target_prop)
                    .intersects(ast::SYMBOL_FLAGS_OPTIONAL);
                let target_type = self.remove_missing_type(target_type, target_optional);
                self.infer_from_types(n, source_type, target_type);
            }
        }
    }

    fn infer_from_signatures(
        &mut self,
        n: &mut InferenceState<'_>,
        source: TypeHandle,
        target: TypeHandle,
        kind: SignatureKind,
    ) {
        let source_signatures = self.get_signatures_of_type(source, kind);
        let source_len = source_signatures.len();
        if source_len > 0 {
            // We match source and target signatures from the bottom up, and if the source has fewer signatures
            // than the target, we infer from the first source signature to the excess target signatures.
            let target_signatures = self.get_signatures_of_type(target, kind);
            let target_len = target_signatures.len();
            for i in 0..target_len {
                let source_index =
                    (source_len as isize - target_len as isize + i as isize).max(0) as usize;
                let source_signature = self.get_base_signature(source_signatures[source_index]);
                let target_signature = self.get_erased_signature(target_signatures[i]);
                self.infer_from_signature(n, source_signature, target_signature);
            }
        }
    }

    fn infer_from_signature(
        &mut self,
        n: &mut InferenceState<'_>,
        source: SignatureHandle,
        target: SignatureHandle,
    ) {
        if self.signature_record(source).flags & SIGNATURE_FLAGS_IS_NON_INFERRABLE == 0 {
            let save_bivariant = n.bivariant;
            let mut kind = ast::KIND_UNKNOWN;
            if let Some(declaration) = self.signature_record(target).declaration {
                kind = self.store_for_node(declaration).kind(declaration);
            }
            // Once we descend into a bivariant signature we remain bivariant for all nested inferences
            n.bivariant = n.bivariant
                || kind == ast::KIND_METHOD_DECLARATION
                || kind == ast::KIND_METHOD_SIGNATURE
                || kind == ast::KIND_CONSTRUCTOR;
            self.apply_to_parameter_types(source, target, |checker, s, t| {
                checker.infer_from_contravariant_types_if_strict_function_types(n, s, t)
            });
            n.bivariant = save_bivariant;
        }
        self.apply_to_return_types(source, target, |checker, s, t| {
            checker.infer_from_types(n, s, t)
        });
    }

    pub(crate) fn apply_to_parameter_types(
        &mut self,
        source: SignatureHandle,
        target: SignatureHandle,
        mut callback: impl FnMut(&mut Checker<'a, '_>, TypeHandle, TypeHandle),
    ) {
        let source_count = self.get_parameter_count(source);
        let target_count = self.get_parameter_count(target);
        let source_rest_type = self.get_effective_rest_type(source);
        let target_rest_type = self.get_effective_rest_type(target);
        let mut target_non_rest_count = target_count;
        if target_rest_type.is_some() {
            target_non_rest_count -= 1;
        }
        let mut param_count = target_non_rest_count;
        if source_rest_type.is_none() {
            param_count = source_count.min(target_non_rest_count);
        }
        if let Some(source_this_type) = self.get_this_type_of_signature(source) {
            if let Some(target_this_type) = self.get_this_type_of_signature(target) {
                callback(self, source_this_type, target_this_type);
            }
        }
        for i in 0..param_count {
            let source_type = self.get_type_at_position(source, i);
            let target_type = self.get_type_at_position(target, i);
            callback(self, source_type, target_type);
        }
        if let Some(target_rest_type) = target_rest_type {
            let readonly = self.is_const_type_variable(Some(target_rest_type), 0)
                && !some_type(self, target_rest_type, |checker, t| {
                    checker.is_mutable_array_like_type(t)
                });
            let source_rest_type = self
                .get_rest_type_at_position(source, param_count, readonly)
                .unwrap();
            callback(self, source_rest_type, target_rest_type);
        }
    }

    pub(crate) fn apply_to_return_types(
        &mut self,
        source: SignatureHandle,
        target: SignatureHandle,
        mut callback: impl FnMut(&mut Checker<'a, '_>, TypeHandle, TypeHandle),
    ) {
        let target_type_predicate = self.get_type_predicate_of_signature(target);
        if let Some(target_type_predicate) = target_type_predicate {
            let source_type_predicate = self.get_type_predicate_of_signature(source);
            if let Some(source_type_predicate) = source_type_predicate
                && self.type_predicate_kinds_match(source_type_predicate, target_type_predicate)
            {
                let source_type_predicate = self.type_predicate_record(source_type_predicate);
                let target_type_predicate = self.type_predicate_record(target_type_predicate);
                if source_type_predicate.t.is_some() && target_type_predicate.t.is_some() {
                    callback(
                        self,
                        source_type_predicate.t.unwrap(),
                        target_type_predicate.t.unwrap(),
                    );
                    return;
                }
            }
        }
        let target_return_type = self.get_return_type_of_signature(target);
        if self.could_contain_type_variables(target_return_type) {
            let source_return_type = self.get_return_type_of_signature(source);
            callback(self, source_return_type, target_return_type);
        }
    }

    fn infer_from_index_types(
        &mut self,
        n: &mut InferenceState<'_>,
        source: TypeHandle,
        target: TypeHandle,
    ) {
        // Inferences across mapped type index signatures are pretty much the same a inferences to homomorphic variables
        let mut priority = INFERENCE_PRIORITY_NONE;
        if self.object_flags(source) & self.object_flags(target) & OBJECT_FLAGS_MAPPED != 0 {
            priority = INFERENCE_PRIORITY_HOMOMORPHIC_MAPPED_TYPE;
        }
        let index_infos = self.get_index_infos_of_type(target);
        if self.is_object_type_with_inferable_index(source) {
            for target_info in &index_infos {
                let mut prop_types = Vec::new();
                let target_key_type = self.index_info_record(*target_info).key_type.unwrap();
                let target_value_type = self.index_info_record(*target_info).value_type.unwrap();
                for prop in self.get_properties_of_type(source) {
                    let prop_name_type = self.get_literal_type_from_property_identity(
                        prop,
                        TYPE_FLAGS_STRING_OR_NUMBER_LITERAL_OR_UNIQUE,
                        false,
                    );
                    if self.is_applicable_index_type(prop_name_type, target_key_type) {
                        let mut prop_type = self.get_type_of_symbol_identity(prop);
                        if self
                            .missing_name_symbol_identity_flags(prop)
                            .intersects(ast::SYMBOL_FLAGS_OPTIONAL)
                        {
                            prop_type = self.remove_missing_or_undefined_type(prop_type);
                        }
                        prop_types.push(prop_type);
                    }
                }
                for info in self.get_index_infos_of_type(source) {
                    let info_key_type = self.index_info_record(info).key_type.unwrap();
                    let info_value_type = self.index_info_record(info).value_type.unwrap();
                    if self.is_applicable_index_type(info_key_type, target_key_type) {
                        prop_types.push(info_value_type);
                    }
                }
                if !prop_types.is_empty() {
                    let prop_type = self.get_union_type(prop_types);
                    self.infer_with_priority(n, prop_type, target_value_type, priority);
                }
            }
        }
        for target_info in index_infos {
            let target_key_type = self.index_info_record(target_info).key_type.unwrap();
            let target_value_type = self.index_info_record(target_info).value_type.unwrap();
            let source_info = self.get_applicable_index_info(source, target_key_type);
            if let Some(source_info) = source_info {
                self.infer_with_priority(
                    n,
                    self.index_info_record(source_info).value_type.unwrap(),
                    target_value_type,
                    priority,
                );
            }
        }
    }

    fn infer_to_mapped_type(
        &mut self,
        n: &mut InferenceState<'_>,
        source: TypeHandle,
        target: TypeHandle,
        constraint_type: TypeHandle,
    ) -> bool {
        if self.type_flags(constraint_type) & TYPE_FLAGS_UNION != 0
            || self.type_flags(constraint_type) & TYPE_FLAGS_INTERSECTION != 0
        {
            let mut result = false;
            for t in self.type_types(constraint_type) {
                result = self.infer_to_mapped_type(n, source, target, t) || result;
            }
            return result;
        }
        if self.type_flags(constraint_type) & TYPE_FLAGS_INDEX != 0 {
            // We're inferring from some source type S to a homomorphic mapped type { [P in keyof T]: X },
            // where T is a type variable. Use inferTypeForHomomorphicMappedType to infer a suitable source
            // type and then make a secondary inference from that type to T. We make a secondary inference
            // such that direct inferences to T get priority over inferences to Partial<T>, for example.
            let constraint_target = self
                .type_record(constraint_type)
                .as_index_type()
                .target
                .unwrap();
            let inference = self.get_inference_info_for_type(n, constraint_target);
            if inference.is_some()
                && !inference.unwrap().is_fixed
                && !self.is_from_inference_blocked_source(source)
            {
                let inferred_type =
                    self.infer_type_for_homomorphic_mapped_type(source, target, constraint_type);
                if let Some(inferred_type) = inferred_type {
                    // We assign a lower priority to inferences made from types containing non-inferrable
                    // types because we may only have a partial result (i.e. we may have failed to make
                    // reverse inferences for some properties).
                    self.infer_with_priority(
                        n,
                        inferred_type,
                        inference.unwrap().type_parameter,
                        if self.object_flags(source) & OBJECT_FLAGS_NON_INFERRABLE_TYPE != 0 {
                            INFERENCE_PRIORITY_PARTIAL_HOMOMORPHIC_MAPPED_TYPE
                        } else {
                            INFERENCE_PRIORITY_HOMOMORPHIC_MAPPED_TYPE
                        },
                    );
                }
            }
            return true;
        }
        if self.type_flags(constraint_type) & TYPE_FLAGS_TYPE_PARAMETER != 0 {
            // We're inferring from some source type S to a mapped type { [P in K]: X }, where K is a type
            // parameter. First infer from 'keyof S' to K.
            let source_index_type = self.get_index_type_ex(
                source,
                if self.semantic_state.has_pattern_for_type(source) {
                    INDEX_FLAGS_NO_INDEX_SIGNATURES
                } else {
                    INDEX_FLAGS_NONE
                },
            );
            self.infer_with_priority(
                n,
                source_index_type,
                constraint_type,
                INFERENCE_PRIORITY_MAPPED_TYPE_CONSTRAINT,
            );
            // If K is constrained to a type C, also infer to C. Thus, for a mapped type { [P in K]: X },
            // where K extends keyof T, we make the same inferences as for a homomorphic mapped type
            // { [P in keyof T]: X }. This enables us to make meaningful inferences when the target is a
            // Pick<T, K>.
            if let Some(extended_constraint) = self.get_constraint_of_type(constraint_type) {
                if self.infer_to_mapped_type(n, source, target, extended_constraint) {
                    return true;
                }
            }
            // If no inferences can be made to K's constraint, infer from a union of the property types
            // in the source to the template type X.
            let prop_types = self
                .get_properties_of_type(source)
                .into_iter()
                .map(|p| self.get_type_of_symbol_identity(p))
                .collect::<Vec<_>>();
            let index_types = self
                .get_index_infos_of_type(source)
                .into_iter()
                .map(|info| {
                    if info
                        != self
                            .semantic_state
                            .semantic_handles()
                            .enum_number_index_info
                    {
                        self.index_info_record(info).value_type.unwrap()
                    } else {
                        self.semantic_state.semantic_handles().never_type
                    }
                })
                .collect::<Vec<_>>();
            let source_type = self.get_union_type([prop_types, index_types].concat());
            let target_type = self.get_template_type_from_mapped_type(target);
            self.infer_from_types(n, source_type, target_type);
            return true;
        }
        false
    }

    // Infer a suitable input type for a homomorphic mapped type { [P in keyof T]: X }. We construct
    // an object type with the same set of properties as the source type, where the type of each
    // property is computed by inferring from the source property type to X for the type
    // variable T[P] (i.e. we treat the type T[P] as the type variable we're inferring for).
    pub(crate) fn infer_type_for_homomorphic_mapped_type(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        constraint: TypeHandle,
    ) -> Option<TypeHandle> {
        let key = ReverseMappedTypeKey {
            source_id: self.type_id(source),
            target_id: self.type_id(target),
            constraint_id: self.type_id(constraint),
        };
        if let Some(cached) = self.semantic_state.reverse_homomorphic_mapped_type(key) {
            return cached;
        }
        let t = self.create_reverse_mapped_type(source, target, constraint);
        self.semantic_state
            .set_reverse_homomorphic_mapped_type(key, t);
        t
    }

    fn create_reverse_mapped_type(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        constraint: TypeHandle,
    ) -> Option<TypeHandle> {
        // We consider a source type reverse mappable if it has a string index signature or if
        // it has one or more properties and is of a partially inferable type.
        if !(self
            .get_index_info_of_type(source, self.semantic_state.semantic_handles().string_type)
            .is_some()
            || !self.get_properties_of_type(source).is_empty()
                && self.is_partially_inferable_type(source))
        {
            return None;
        }
        // For arrays and tuples we infer new arrays and tuples where the reverse mapping has been
        // applied to the element type(s).
        if self.is_array_type(source) {
            let source_element_type = self.type_argument_at(source, 0);
            let element_type =
                self.infer_reverse_mapped_type(source_element_type, target, constraint)?;
            let readonly = self.is_readonly_array_type(source);
            return Some(self.create_array_type_ex(element_type, readonly));
        }
        if self.is_tuple_type(source) {
            let element_types = self
                .get_element_types(source)
                .into_iter()
                .map(|t| self.infer_reverse_mapped_type(t, target, constraint))
                .collect::<Vec<_>>();
            if !element_types.iter().all(|t| t.is_some()) {
                return None;
            }
            let mut element_infos = self.target_tuple_type_record(source).element_infos.clone();
            if self.get_mapped_type_modifiers(target) & MAPPED_TYPE_MODIFIERS_INCLUDE_OPTIONAL != 0
            {
                element_infos = element_infos
                    .into_iter()
                    .map(|info| {
                        if info.flags & ELEMENT_FLAGS_OPTIONAL != 0 {
                            TupleElementInfo {
                                flags: ELEMENT_FLAGS_REQUIRED,
                                labeled_declaration: info.labeled_declaration,
                            }
                        } else {
                            info
                        }
                    })
                    .collect();
            }
            return Some(self.create_tuple_type_ex(
                element_types.into_iter().map(Option::unwrap).collect(),
                element_infos,
                self.target_tuple_type_record(source).readonly,
            ));
        }
        // For all other object types we infer a new object type where the reverse mapping has been
        // applied to the type of each property.
        let reversed = self.new_object_type_from_identity(
            OBJECT_FLAGS_REVERSE_MAPPED | OBJECT_FLAGS_ANONYMOUS,
            None, /*symbol*/
        );
        self.set_reverse_mapped_type_links(reversed, source, target, constraint);
        Some(reversed)
    }

    // We consider a type to be partially inferable if it isn't marked non-inferable or if it is
    // an object literal type with at least one property of an inferable type. For example, an object
    // literal { a: 123, b: x => true } is marked non-inferable because it contains a context sensitive
    // arrow function, but is considered partially inferable because property 'a' has an inferable type.
    fn is_partially_inferable_type(&mut self, t: TypeHandle) -> bool {
        self.object_flags(t) & OBJECT_FLAGS_NON_INFERRABLE_TYPE == 0
            || is_object_literal_type(self, t)
                && self.get_properties_of_type(t).into_iter().any(|prop| {
                    let prop_type = self.get_type_of_symbol_identity(prop);
                    self.is_partially_inferable_type(prop_type)
                })
            || self.is_tuple_type(t)
                && self
                    .get_element_types(t)
                    .into_iter()
                    .any(|t| self.is_partially_inferable_type(t))
    }

    fn infer_reverse_mapped_type(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        constraint: TypeHandle,
    ) -> Option<TypeHandle> {
        let key = ReverseMappedTypeKey {
            source_id: self.type_id(source),
            target_id: self.type_id(target),
            constraint_id: self.type_id(constraint),
        };
        if let Some(cached) = self.semantic_state.reverse_mapped_type(key) {
            return Some(cached.unwrap_or(self.semantic_state.semantic_handles().unknown_type));
        }
        self.semantic_state
            .push_reverse_mapped_types(source, target);
        let save_expanding_flags = self.semantic_state.reverse_expanding_flags;
        let source_stack = self.semantic_state.reverse_mapped_source_stack_snapshot();
        if self.is_deeply_nested_type(source, &source_stack, 2) {
            self.semantic_state.reverse_expanding_flags |= EXPANDING_FLAGS_SOURCE;
        }
        let target_stack = self.semantic_state.reverse_mapped_target_stack_snapshot();
        if self.is_deeply_nested_type(target, &target_stack, 2) {
            self.semantic_state.reverse_expanding_flags |= EXPANDING_FLAGS_TARGET;
        }
        let mut t = None;
        if self.semantic_state.reverse_expanding_flags != EXPANDING_FLAGS_BOTH {
            t = Some(self.infer_reverse_mapped_type_worker(source, target, constraint));
        }
        self.semantic_state.pop_reverse_mapped_types();
        self.semantic_state.reverse_expanding_flags = save_expanding_flags;
        self.semantic_state.set_reverse_mapped_type(key, t);
        t
    }

    fn infer_reverse_mapped_type_worker(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        constraint: TypeHandle,
    ) -> TypeHandle {
        let mapped_type_parameter = self.get_type_parameter_from_mapped_type(target);
        let type_parameter = self.get_indexed_access_type(
            self.type_record(constraint).as_index_type().target.unwrap(),
            mapped_type_parameter,
        );
        let template_type = self.get_template_type_from_mapped_type(target);
        let mut inferences = vec![new_inference_info(type_parameter)];
        self.infer_types(
            &mut inferences,
            source,
            template_type,
            INFERENCE_PRIORITY_NONE,
            false,
        );
        let inferred_type = self
            .get_type_from_inference(&inferences[0])
            .unwrap_or(self.semantic_state.semantic_handles().unknown_type);
        self.get_widened_type(inferred_type)
    }

    pub(crate) fn resolve_reverse_mapped_type_members(&mut self, t: TypeHandle) {
        let r = self.type_record(t).as_reverse_mapped_type();
        let source = r.source.unwrap();
        let mapped_type = r.mapped_type.unwrap();
        let constraint_type = r.constraint_type.unwrap();
        let index_info =
            self.get_index_info_of_type(source, self.semantic_state.semantic_handles().string_type);
        let modifiers = self.get_mapped_type_modifiers(mapped_type);
        let readonly_mask = modifiers & MAPPED_TYPE_MODIFIERS_INCLUDE_READONLY == 0;
        let optional_mask = if modifiers & MAPPED_TYPE_MODIFIERS_INCLUDE_OPTIONAL != 0 {
            0
        } else {
            ast::SYMBOL_FLAGS_OPTIONAL
        };
        let mut index_infos = Vec::new();
        if let Some(index_info) = index_info {
            let inferred_type = self
                .infer_reverse_mapped_type(
                    self.index_info_record(index_info).value_type.unwrap(),
                    mapped_type,
                    constraint_type,
                )
                .unwrap_or(self.semantic_state.semantic_handles().unknown_type);
            index_infos.push(self.new_index_info(
                self.semantic_state.semantic_handles().string_type,
                inferred_type,
                readonly_mask && self.index_info_record(index_info).is_readonly,
                None,
                None,
            ));
        }
        let mut members = SymbolIdentityTable::default();
        let limited_constraint = self.get_limited_constraint(t);
        for prop in self.get_properties_of_type(source) {
            let prop_identity = prop;
            // In case of a reverse mapped type with an intersection constraint, if we were able to
            // extract the filtering type literals we skip those properties that are not assignable to them,
            // because the extra properties wouldn't get through the application of the mapped type anyway
            if let Some(limited_constraint) = limited_constraint {
                let property_name_type = self.get_literal_type_from_property_identity(
                    prop_identity,
                    TYPE_FLAGS_STRING_OR_NUMBER_LITERAL_OR_UNIQUE,
                    false,
                );
                if !self.is_type_assignable_to(property_name_type, limited_constraint) {
                    continue;
                }
            }
            let check_flags = ast::CHECK_FLAGS_REVERSE_MAPPED
                | if readonly_mask && self.is_readonly_symbol_identity(prop_identity) {
                    ast::CHECK_FLAGS_READONLY
                } else {
                    0
                };
            let prop_name = self.symbol_identity_name(prop_identity);
            let prop_flags = self.missing_name_symbol_identity_flags(prop_identity);
            let inferred_prop = self.new_symbol_ex(
                ast::SYMBOL_FLAGS_PROPERTY | prop_flags & optional_mask,
                prop_name.clone(),
                check_flags,
            );
            let inferred_prop = self.transient_symbol_handle(inferred_prop);
            let declarations = self.collect_symbol_identity_declarations(prop_identity);
            self.set_transient_symbol_declarations(inferred_prop, declarations);
            let prop_name_type = self.semantic_state.value_symbol_name_type(&prop_identity);
            self.semantic_state
                .set_value_symbol_name_type(&inferred_prop, prop_name_type);
            let prop_type = self.get_type_of_symbol_identity(prop_identity);
            let constraint_target = self
                .type_record(constraint_type)
                .as_index_type()
                .target
                .unwrap();
            let (link_mapped_type, link_constraint_type) = if self.type_flags(constraint_target)
                & TYPE_FLAGS_INDEXED_ACCESS
                != 0
            {
                let constraint_indexed_access =
                    self.type_record(constraint_target).as_indexed_access_type();
                let constraint_object_type = constraint_indexed_access.object_type.unwrap();
                let constraint_index_type = constraint_indexed_access.index_type.unwrap();
                if !(self.type_flags(constraint_object_type) & TYPE_FLAGS_TYPE_PARAMETER != 0
                    && self.type_flags(constraint_index_type) & TYPE_FLAGS_TYPE_PARAMETER != 0)
                {
                    (mapped_type, constraint_type)
                } else {
                    // A reverse mapping of `{[K in keyof T[K_1]]: T[K_1]}` is the same as that of `{[K in keyof T]: T}`, since all we care about is
                    // inferring to the "type parameter" (or indexed access) shared by the constraint and template. So, to reduce the number of
                    // type identities produced, we simplify such indexed access occurrences
                    let new_type_param = constraint_object_type;
                    let new_mapped_type =
                        self.replace_indexed_access(mapped_type, constraint_target, new_type_param);
                    let new_constraint_type = self.get_index_type(new_type_param);
                    (new_mapped_type, new_constraint_type)
                }
            } else {
                (mapped_type, constraint_type)
            };
            self.semantic_state.set_reverse_mapped_symbol_link_types(
                &inferred_prop,
                Some(prop_type),
                Some(link_mapped_type),
                Some(link_constraint_type),
            );
            members.insert(prop_name.into(), self.symbol_handle_identity(inferred_prop));
        }
        self.set_structured_type_members_from_identities(
            t,
            members,
            Vec::new(),
            Vec::new(),
            index_infos,
        );
    }

    pub(crate) fn get_type_of_reverse_mapped_symbol(
        &mut self,
        symbol: SymbolIdentity,
    ) -> TypeHandle {
        let handle = self
            .semantic_state
            .reverse_mapped_symbol_link_handle(symbol);
        if self
            .semantic_state
            .reverse_mapped_resolved_type_by_handle(handle)
            .is_none()
        {
            let (property_type, mapped_type, constraint_type) = self
                .semantic_state
                .reverse_mapped_symbol_link_types_by_handle(handle);
            let resolved_type = self
                .infer_reverse_mapped_type(
                    property_type.unwrap(),
                    mapped_type.unwrap(),
                    constraint_type.unwrap(),
                )
                .unwrap_or(self.semantic_state.semantic_handles().unknown_type);
            self.semantic_state
                .set_reverse_mapped_resolved_type_by_handle(handle, Some(resolved_type));
        }
        self.semantic_state
            .reverse_mapped_resolved_type_by_handle(handle)
            .unwrap()
    }

    // If the original mapped type had an intersection constraint we extract its components,
    // and we make an attempt to do so even if the intersection has been reduced to a union.
    // This entire process allows us to possibly retrieve the filtering type literals.
    // e.g. { [K in keyof U & ("a" | "b") ] } -> "a" | "b"
    fn get_limited_constraint(&mut self, t: TypeHandle) -> Option<TypeHandle> {
        let reverse_mapped_type = self.type_record(t).as_reverse_mapped_type();
        let mapped_type = reverse_mapped_type.mapped_type.unwrap();
        let constraint_type = reverse_mapped_type.constraint_type.unwrap();
        let constraint = self.get_constraint_type_from_mapped_type(mapped_type);
        if !(self.type_flags(constraint) & TYPE_FLAGS_UNION != 0
            || self.type_flags(constraint) & TYPE_FLAGS_INTERSECTION != 0)
        {
            return None;
        }
        let mut origin = constraint;
        if self.type_flags(constraint) & TYPE_FLAGS_UNION != 0 {
            origin = self.type_record(constraint).as_union_type().origin?;
        }
        if self.type_flags(origin) & TYPE_FLAGS_INTERSECTION == 0 {
            return None;
        }
        let limited_constraint = self.get_intersection_type(
            self.type_types(origin)
                .into_iter()
                .filter(|&t| t != constraint_type)
                .collect(),
        );
        if limited_constraint != self.semantic_state.semantic_handles().never_type {
            return Some(limited_constraint);
        }
        None
    }

    fn replace_indexed_access(
        &mut self,
        instantiable: TypeHandle,
        t: TypeHandle,
        replacement: TypeHandle,
    ) -> TypeHandle {
        // map type.indexType to 0
        // map type.objectType to `[TReplacement]`
        // thus making the indexed access `[TReplacement][0]` or `TReplacement`
        let number_zero = self.get_number_literal_type(jsnum::Number::from(0));
        let replacement_tuple = self.create_tuple_type(vec![replacement]);
        let mapper = self.new_type_mapper_handle(
            [
                self.type_record(t)
                    .as_indexed_access_type()
                    .index_type
                    .unwrap(),
                self.type_record(t)
                    .as_indexed_access_type()
                    .object_type
                    .unwrap(),
            ],
            [number_zero, replacement_tuple],
        );
        self.instantiate_type_with_mapper_handle(Some(instantiable), Some(mapper))
            .unwrap()
    }

    fn types_definitely_unrelated(&mut self, source: TypeHandle, target: TypeHandle) -> bool {
        // Two tuple types with incompatible arities are definitely unrelated.
        // Two object types that each have a property that is unmatched in the other are definitely unrelated.
        if self.is_tuple_type(source) && self.is_tuple_type(target) {
            return self.tuple_types_definitely_unrelated(source, target);
        }
        self.get_unmatched_property(
            source, target, false, /*requireOptionalProperties*/
            true,  /*matchDiscriminantProperties*/
        )
        .is_some()
            && self
                .get_unmatched_property(
                    target, source, false, /*requireOptionalProperties*/
                    false, /*matchDiscriminantProperties*/
                )
                .is_some()
    }

    fn is_tuple_type_structure_matching(&mut self, t1: TypeHandle, t2: TypeHandle) -> bool {
        if self.get_type_reference_arity(t1) != self.get_type_reference_arity(t2) {
            return false;
        }
        for (i, e) in self
            .target_tuple_type_record(t1)
            .element_infos
            .iter()
            .enumerate()
        {
            if e.flags & ELEMENT_FLAGS_VARIABLE
                != self.target_tuple_type_record(t2).element_infos[i].flags & ELEMENT_FLAGS_VARIABLE
            {
                return false;
            }
        }
        true
    }

    fn is_type_or_base_identical_to(&mut self, s: TypeHandle, t: TypeHandle) -> bool {
        if t == self.semantic_state.semantic_handles().missing_type {
            return s == t;
        }
        self.is_type_identical_to(s, t)
            || self.type_flags(t) & TYPE_FLAGS_STRING != 0
                && self.type_flags(s) & TYPE_FLAGS_STRING_LITERAL != 0
            || self.type_flags(t) & TYPE_FLAGS_NUMBER != 0
                && self.type_flags(s) & TYPE_FLAGS_NUMBER_LITERAL != 0
    }

    fn is_type_closely_matched_by(&mut self, s: TypeHandle, t: TypeHandle) -> bool {
        self.type_flags(s) & TYPE_FLAGS_OBJECT != 0
            && self.type_flags(t) & TYPE_FLAGS_OBJECT != 0
            && self.type_symbol_identity(s).is_some()
            && self.same_optional_symbol_identity(
                self.type_symbol_identity(s),
                self.type_symbol_identity(t),
            )
            || self.type_alias_record(s).is_some()
                && self.type_alias_record(t).is_some()
                && !self.type_alias_record(s).unwrap().type_arguments.is_empty()
                && self.type_alias_record(s).unwrap().symbol
                    == self.type_alias_record(t).unwrap().symbol
    }

    // Create an object with properties named in the string literal type. Every property has type `any`.
    fn create_empty_object_type_from_string_literal(&mut self, t: TypeHandle) -> TypeHandle {
        let mut members = SymbolIdentityTable::default();
        for t in self.distributed_types(t) {
            if self.type_flags(t) & TYPE_FLAGS_STRING_LITERAL == 0 {
                continue;
            }
            let name = self.get_string_literal_value(t);
            let literal_prop = self.new_symbol(ast::SYMBOL_FLAGS_PROPERTY, name.clone());
            let literal_prop = self.transient_symbol_handle(literal_prop);
            self.semantic_state.set_value_symbol_resolved_type(
                &literal_prop,
                Some(self.semantic_state.semantic_handles().any_type),
            );
            if let Some(symbol) = self.type_symbol_identity(t) {
                self.set_transient_symbol_declarations(
                    literal_prop,
                    self.collect_symbol_identity_declarations(symbol),
                );
                self.set_transient_symbol_value_declaration(
                    literal_prop,
                    self.missing_name_symbol_identity_value_declaration(symbol),
                );
            }
            members.insert(name.into(), self.symbol_handle_identity(literal_prop));
        }
        let mut index_infos = Vec::new();
        if self.type_flags(t) & TYPE_FLAGS_STRING != 0 {
            index_infos.push(self.new_index_info(
                self.semantic_state.semantic_handles().string_type,
                self.semantic_state.semantic_handles().empty_object_type,
                false, /*isReadonly*/
                None,
                None,
            ));
        }
        let result = self.new_object_type_from_identity(OBJECT_FLAGS_ANONYMOUS, None);
        let properties = members.values().copied().collect();
        self.set_structured_type_member_identities(
            result,
            members,
            properties,
            Vec::new(),
            0,
            index_infos,
        );
        result
    }

    pub(crate) fn new_inference_context(
        &mut self,
        type_parameters: Vec<TypeHandle>,
        signature: Option<SignatureHandle>,
        flags: InferenceFlags,
        compare_types: Option<TypeComparer>,
    ) -> InferenceContextRef {
        let compare_types = compare_types.unwrap_or(self.semantic_state.compare_types_assignable);
        self.new_inference_context_worker(
            type_parameters
                .into_iter()
                .map(new_inference_info)
                .collect(),
            signature,
            flags,
            compare_types,
        )
    }

    pub(crate) fn clone_inference_context(
        &mut self,
        n: Option<InferenceContextRef>,
        extra_flags: InferenceFlags,
    ) -> Option<InferenceContextRef> {
        let n = self.inference_context_record(n?).clone();
        Some(
            self.new_inference_context_worker(
                n.inferences
                    .iter()
                    .map(|info| clone_inference_info(info))
                    .collect(),
                n.signature,
                n.flags | extra_flags,
                n.compare_types,
            ),
        )
    }

    pub(crate) fn clone_inference_context_for_node(
        &mut self,
        node: ast::Node,
        extra_flags: InferenceFlags,
    ) -> Option<InferenceContextRef> {
        let context = self.get_inference_context(node)?;
        self.clone_inference_context(Some(context), extra_flags)
    }

    pub(crate) fn clone_inferred_part_of_context(
        &mut self,
        n: InferenceContextRef,
    ) -> Option<InferenceContextRef> {
        let n = self.inference_context_record(n).clone();
        let inferences = n
            .inferences
            .iter()
            .filter(|info| has_inference_candidates(info))
            .map(|info| clone_inference_info(info))
            .collect::<Vec<_>>();
        if inferences.is_empty() {
            return None;
        }
        Some(self.new_inference_context_worker(inferences, n.signature, n.flags, n.compare_types))
    }

    fn new_inference_context_worker(
        &mut self,
        inferences: Vec<InferenceInfo>,
        signature: Option<SignatureHandle>,
        flags: InferenceFlags,
        compare_types: TypeComparer,
    ) -> InferenceContextRef {
        let n = self
            .semantic_state
            .alloc_inference_context(InferenceContextRecord {
                inferences,
                signature,
                flags,
                compare_types,
                mapper: None,
                non_fixing_mapper: None,
                return_mapper: None,
                outer_return_mapper: None,
                inferred_type_parameters: Vec::new(),
                intra_expression_inference_sites: Vec::new(),
            });
        let mapper = self.new_inference_type_mapper_handle(n, true /*fixing*/);
        let non_fixing_mapper = self.new_inference_type_mapper_handle(n, false /*fixing*/);
        {
            let record = self.semantic_state.inference_context_record_mut(n);
            record.mapper = Some(mapper);
            record.non_fixing_mapper = Some(non_fixing_mapper);
        }
        n
    }

    pub(crate) fn add_intra_expression_inference_site(
        &mut self,
        n: InferenceContextRef,
        node: ast::Node,
        t: TypeHandle,
    ) {
        self.inference_context_record_mut(n)
            .intra_expression_inference_sites
            .push(IntraExpressionInferenceSite { node, t });
    }

    // We collect intra-expression inference sites within object and array literals to handle cases where
    // inferred types flow between context sensitive element expressions. For example:
    //
    //	declare function foo<T>(arg: [(n: number) => T, (x: T) => void]): void;
    //	foo([_a => 0, n => n.toFixed()]);
    //
    // Above, both arrow functions in the tuple argument are context sensitive, thus both are omitted from the
    // pass that collects inferences from the non-context sensitive parts of the arguments. In the subsequent
    // pass where nothing is omitted, we need to commit to an inference for T in order to contextually type the
    // parameter in the second arrow function, but we want to first infer from the return type of the first
    // arrow function. This happens automatically when the arrow functions are discrete arguments (because we
    // infer from each argument before processing the next), but when the arrow functions are elements of an
    // object or array literal, we need to perform intra-expression inferences early.
    pub(crate) fn infer_from_intra_expression_sites(&mut self, n: InferenceContextRef) {
        let sites = std::mem::take(
            &mut self
                .inference_context_record_mut(n)
                .intra_expression_inference_sites,
        );
        for site in &sites {
            let site_store = self.store_for_node(site.node);
            let contextual_type = if ast::is_method_declaration(site_store, site.node) {
                self.get_contextual_type_for_object_literal_method(
                    site.node,
                    CONTEXT_FLAGS_NO_CONSTRAINTS,
                )
            } else {
                self.get_contextual_type(site.node, CONTEXT_FLAGS_NO_CONSTRAINTS)
            };
            if let Some(contextual_type) = contextual_type {
                let mut inferences =
                    std::mem::take(&mut self.inference_context_record_mut(n).inferences);
                self.infer_types(
                    &mut inferences,
                    site.t,
                    contextual_type,
                    INFERENCE_PRIORITY_NONE,
                    false,
                );
                self.inference_context_record_mut(n).inferences = inferences;
            }
        }
    }

    pub(crate) fn get_inferred_type(&mut self, n: InferenceContextRef, index: usize) -> TypeHandle {
        let context = self.inference_context_record(n).clone();
        let non_fixing_mapper = context.non_fixing_mapper.unwrap();
        let inference = {
            let inferences = &context.inferences;
            if let Some(inferred_type) = inferences[index].inferred_type {
                return inferred_type;
            }
            inferences[index].clone()
        };
        if inference.type_parameter == self.semantic_state.semantic_handles().error_type {
            return inference.type_parameter;
        }
        let mut inferred_type = None;
        let mut fallback_type = None;
        if let Some(signature) = context.signature {
            let mut inferred_covariant_type = None;
            if !inference.candidates.is_empty() {
                inferred_covariant_type = Some(self.get_covariant_inference(&inference, signature));
            }
            let mut inferred_contravariant_type = None;
            if !inference.contra_candidates.is_empty() {
                inferred_contravariant_type = Some(self.get_contravariant_inference(&inference));
            }
            if inferred_covariant_type.is_some() || inferred_contravariant_type.is_some() {
                let prefer_covariant_type = inferred_covariant_type.is_some()
                    && (inferred_contravariant_type.is_none()
                        || self.type_flags(inferred_covariant_type.unwrap())
                            & (TYPE_FLAGS_NEVER | TYPE_FLAGS_ANY)
                            == 0
                            && inference.contra_candidates.iter().any(|&t| {
                                self.is_type_assignable_to(inferred_covariant_type.unwrap(), t)
                            })
                            && context.inferences.iter().enumerate().all(
                                |(other_index, other)| {
                                    if other_index != index
                                        && self
                                            .get_constraint_of_type_parameter(other.type_parameter)
                                            != Some(inference.type_parameter)
                                    {
                                        return true;
                                    }
                                    other.candidates.iter().all(|&t| {
                                        self.is_type_assignable_to(
                                            t,
                                            inferred_covariant_type.unwrap(),
                                        )
                                    })
                                },
                            ));
                if prefer_covariant_type {
                    inferred_type = inferred_covariant_type;
                    fallback_type = inferred_contravariant_type;
                } else {
                    inferred_type = inferred_contravariant_type;
                    fallback_type = inferred_covariant_type;
                }
            } else if context.flags & INFERENCE_FLAGS_NO_DEFAULT != 0 {
                // We use silentNeverType as the wildcard that signals no inferences.
                inferred_type = Some(self.semantic_state.semantic_handles().silent_never_type);
            } else if let Some(default_type) =
                self.get_default_from_type_parameter(inference.type_parameter)
            {
                // Instantiate the default type. Any forward reference to a type
                // parameter should be instantiated to the empty object type.
                let backreference_mapper = self.new_array_to_single_type_mapper_handle(
                    std::iter::once(inference.type_parameter)
                        .chain(
                            context.inferences[index + 1..]
                                .iter()
                                .map(|i| i.type_parameter),
                        )
                        .collect::<TypeMapperList>(),
                    self.semantic_state.semantic_handles().unknown_type,
                );
                let mapper =
                    self.merge_type_mapper_handles(Some(backreference_mapper), non_fixing_mapper);
                inferred_type =
                    self.instantiate_type_with_mapper_handle(Some(default_type), Some(mapper));
            }
        } else {
            inferred_type = self.get_type_from_inference(&inference);
        }
        let mut cached_inferred_type =
            inferred_type.unwrap_or(if context.flags & INFERENCE_FLAGS_ANY_DEFAULT != 0 {
                self.semantic_state.semantic_handles().any_type
            } else {
                self.semantic_state.semantic_handles().unknown_type
            });
        {
            let inferences = &mut self.inference_context_record_mut(n).inferences;
            if inferences[index].inferred_type.is_none() {
                inferences[index].inferred_type = Some(cached_inferred_type);
            }
        }
        if let Some(constraint) = self.get_constraint_of_type_parameter(inference.type_parameter) {
            let non_fixing_mapper = self.inference_context_record(n).non_fixing_mapper;
            let instantiated_constraint =
                self.instantiate_type_with_mapper_handle(Some(constraint), non_fixing_mapper);
            if let Some(inferred) = inferred_type {
                let constraint_with_this = self.get_type_with_this_argument(
                    instantiated_constraint.unwrap(),
                    Some(inferred),
                    false,
                );
                if (context.compare_types)(self, inferred, constraint_with_this, false)
                    == TERNARY_FALSE
                {
                    let mut filtered_by_constraint = None;
                    if inference.priority == INFERENCE_PRIORITY_RETURN_TYPE {
                        filtered_by_constraint = Some(self.map_type(inferred, |checker, t| {
                            if (context.compare_types)(checker, t, constraint_with_this, false)
                                != TERNARY_FALSE
                            {
                                t
                            } else {
                                checker.semantic_state.semantic_handles().never_type
                            }
                        }));
                    }
                    inferred_type = if filtered_by_constraint.is_some()
                        && self.type_flags(filtered_by_constraint.unwrap()) & TYPE_FLAGS_NEVER == 0
                    {
                        filtered_by_constraint
                    } else {
                        None
                    };
                }
            }
            if inferred_type.is_none() {
                let fallback_constraint_with_this = fallback_type.map(|fallback_type| {
                    self.get_type_with_this_argument(
                        instantiated_constraint.unwrap(),
                        Some(fallback_type),
                        false,
                    )
                });
                inferred_type = Some(
                    if fallback_type.is_some()
                        && (context.compare_types)(
                            self,
                            fallback_type.unwrap(),
                            fallback_constraint_with_this.unwrap(),
                            false,
                        ) != TERNARY_FALSE
                    {
                        fallback_type.unwrap()
                    } else {
                        instantiated_constraint.unwrap()
                    },
                );
            }
            cached_inferred_type = inferred_type.unwrap();
        }
        self.inference_context_record_mut(n).inferences[index].inferred_type =
            Some(cached_inferred_type);
        self.clear_active_mapper_caches();
        cached_inferred_type
    }

    pub(crate) fn get_inferred_types(&mut self, n: InferenceContextRef) -> Vec<TypeHandle> {
        let len = self.inference_context_record(n).inferences.len();
        let mut result = Vec::with_capacity(len);
        for i in 0..len {
            result.push(self.get_inferred_type(n, i));
        }
        result
    }

    pub(crate) fn get_mapper_from_context(
        &self,
        n: Option<InferenceContextRef>,
    ) -> Option<TypeMapperHandle> {
        n.and_then(|n| self.inference_context_record(n).mapper)
    }

    // Return a type mapper that combines the context's return mapper with a mapper that erases any additional type parameters
    // to their inferences at the time of creation.
    pub(crate) fn create_outer_return_mapper(
        &mut self,
        context: InferenceContextRef,
    ) -> TypeMapperHandle {
        if self
            .inference_context_record(context)
            .outer_return_mapper
            .is_none()
        {
            let cloned_context = self
                .clone_inference_context(Some(context), INFERENCE_FLAGS_NONE)
                .unwrap();
            let mut mapper = self
                .inference_context_record(cloned_context)
                .mapper
                .unwrap();
            if let Some(return_mapper) = self.inference_context_record(context).return_mapper {
                mapper = self.semantic_state.alloc_mapper(TypeMapperRecord {
                    data: TypeMapperRecordData::Merged(MergedTypeMapperRecord {
                        left: return_mapper,
                        right: mapper,
                    }),
                });
            }
            self.inference_context_record_mut(context)
                .outer_return_mapper = Some(mapper);
        }
        self.inference_context_record(context)
            .outer_return_mapper
            .unwrap()
    }

    pub(crate) fn create_outer_return_mapper_for_node(
        &mut self,
        node: ast::Node,
    ) -> Option<TypeMapperHandle> {
        let context = self.get_inference_context(node)?;
        Some(self.create_outer_return_mapper(context))
    }

    pub(crate) fn get_covariant_inference(
        &mut self,
        inference: &InferenceInfo,
        signature: SignatureHandle,
    ) -> TypeHandle {
        // Extract all object and array literal types and replace them with a single widened and normalized type.
        let candidates =
            self.union_object_and_array_literal_candidates(inference.candidates.clone());
        // We widen inferred literal types if
        // all inferences were made to top-level occurrences of the type parameter, and
        // the type parameter has no constraint or its constraint includes no primitive or literal types, and
        // the type parameter was fixed during inference or does not occur at top-level in the return type.
        let primitive_constraint = self.has_primitive_constraint(inference.type_parameter)
            || self.is_const_type_variable(Some(inference.type_parameter), 0);
        let widen_literal_types = !primitive_constraint
            && inference.top_level
            && (inference.is_fixed
                || !self.is_type_parameter_at_top_level_in_return_type(
                    signature,
                    inference.type_parameter,
                ));
        let base_candidates = if primitive_constraint {
            candidates
                .into_iter()
                .map(|t| self.get_regular_type_of_literal_type(t))
                .collect()
        } else if widen_literal_types {
            candidates
                .into_iter()
                .map(|t| self.get_widened_literal_type(t))
                .collect()
        } else {
            candidates
        };
        // If all inferences were made from a position that implies a combined result, infer a union type.
        // Otherwise, infer a common supertype.
        let unwidened_type =
            if inference.priority & INFERENCE_PRIORITY_PRIORITY_IMPLIES_COMBINATION != 0 {
                self.get_union_type_ex(base_candidates, UNION_REDUCTION_SUBTYPE, None, None)
            } else {
                self.get_common_supertype(base_candidates)
            };
        self.get_widened_type(unwidened_type)
    }

    pub(crate) fn get_contravariant_inference(&mut self, inference: &InferenceInfo) -> TypeHandle {
        if inference.priority & INFERENCE_PRIORITY_PRIORITY_IMPLIES_COMBINATION != 0 {
            return self.get_intersection_type(inference.contra_candidates.clone());
        }
        self.get_common_subtype(inference.contra_candidates.clone())
    }

    fn union_object_and_array_literal_candidates(
        &mut self,
        candidates: Vec<TypeHandle>,
    ) -> Vec<TypeHandle> {
        if candidates.len() > 1 {
            let object_literals = candidates
                .iter()
                .copied()
                .filter(|&t| is_object_or_array_literal_type(self, t))
                .collect::<Vec<_>>();
            if !object_literals.is_empty() {
                let literals_type =
                    self.get_union_type_ex(object_literals, UNION_REDUCTION_SUBTYPE, None, None);
                let mut non_literal_types = candidates
                    .into_iter()
                    .filter(|&t| !is_object_or_array_literal_type(self, t))
                    .collect::<Vec<_>>();
                non_literal_types.push(literals_type);
                return non_literal_types;
            }
        }
        candidates
    }

    fn has_primitive_constraint(&mut self, t: TypeHandle) -> bool {
        if let Some(mut constraint) = self.get_constraint_of_type_parameter(t) {
            if self.type_flags(constraint) & TYPE_FLAGS_CONDITIONAL != 0 {
                constraint = self.get_default_constraint_of_conditional_type(constraint);
            }
            return self.maybe_type_of_kind(
                constraint,
                TYPE_FLAGS_PRIMITIVE
                    | TYPE_FLAGS_INDEX
                    | TYPE_FLAGS_TEMPLATE_LITERAL
                    | TYPE_FLAGS_STRING_MAPPING,
            );
        }
        false
    }

    fn is_type_parameter_at_top_level(
        &mut self,
        t: TypeHandle,
        tp: TypeHandle,
        depth: usize,
    ) -> bool {
        t == tp
            || self.type_flags(t) & TYPE_FLAGS_UNION_OR_INTERSECTION != 0
                && self
                    .type_types(t)
                    .into_iter()
                    .any(|t| self.is_type_parameter_at_top_level(t, tp, depth))
            || depth < 3 && self.type_flags(t) & TYPE_FLAGS_CONDITIONAL != 0 && {
                let true_type = self.get_true_type_from_conditional_type(t);
                let false_type = self.get_false_type_from_conditional_type(t);
                self.is_type_parameter_at_top_level(true_type, tp, depth + 1)
                    || self.is_type_parameter_at_top_level(false_type, tp, depth + 1)
            }
    }

    fn is_type_parameter_at_top_level_in_return_type(
        &mut self,
        signature: SignatureHandle,
        type_parameter: TypeHandle,
    ) -> bool {
        let type_predicate = self.get_type_predicate_of_signature(signature);
        if let Some(type_predicate) = type_predicate {
            let type_predicate = self.type_predicate_record(type_predicate);
            return type_predicate.t.is_some()
                && self.is_type_parameter_at_top_level(
                    type_predicate.t.unwrap(),
                    type_parameter,
                    0,
                );
        }
        let return_type = self.get_return_type_of_signature(signature);
        self.is_type_parameter_at_top_level(return_type, type_parameter, 0)
    }

    pub(crate) fn get_type_from_inference(
        &mut self,
        inference: &InferenceInfo,
    ) -> Option<TypeHandle> {
        if inference.candidates_present {
            return Some(self.get_union_type_ex(
                inference.candidates.clone(),
                UNION_REDUCTION_SUBTYPE,
                None,
                None,
            ));
        }
        if inference.contra_candidates_present {
            return Some(self.get_intersection_type(inference.contra_candidates.clone()));
        }
        None
    }

    fn get_common_supertype(&mut self, types: Vec<TypeHandle>) -> TypeHandle {
        if types.len() == 1 {
            return types[0];
        }
        // Remove nullable types from each of the candidates.
        let primary_types = if self.strict_null_checks() {
            types
                .iter()
                .map(|&t| {
                    self.filter_type_with_checker(t, |checker, u| {
                        checker.type_flags(u) & TYPE_FLAGS_NULLABLE == 0
                    })
                })
                .collect::<Vec<_>>()
        } else {
            types.clone()
        };
        // When the candidate types are all literal types with the same base type, return a union
        // of those literal types. Otherwise, return the leftmost type for which no type to the
        // right is a supertype.
        let supertype = if self.literal_types_with_same_base_type(primary_types.clone()) {
            self.get_union_type(primary_types.clone())
        } else {
            self.get_single_common_supertype(primary_types.clone())
        };
        // Add any nullable types that occurred in the candidates back to the result.
        if primary_types == types {
            return supertype;
        }
        let nullable_flags = self.get_combined_type_flags(types) & TYPE_FLAGS_NULLABLE;
        self.get_nullable_type(supertype, nullable_flags)
    }

    fn get_single_common_supertype(&mut self, types: Vec<TypeHandle>) -> TypeHandle {
        // First, find the leftmost type for which no type to the right is a strict supertype, and if that
        // type is a strict supertype of all other candidates, return it. Otherwise, return the leftmost type
        // for which no type to the right is a (regular) supertype.
        let candidate = self.find_leftmost_type(types.clone(), |checker, left, right| {
            checker.is_type_strict_subtype_of(left, right)
        });
        if types
            .iter()
            .all(|&t| t == candidate || self.is_type_strict_subtype_of(t, candidate))
        {
            return candidate;
        }
        self.find_leftmost_type(types, |checker, left, right| {
            checker.is_type_subtype_of(left, right)
        })
    }

    fn find_leftmost_type(
        &mut self,
        types: Vec<TypeHandle>,
        f: fn(&mut Checker<'a, 'state>, TypeHandle, TypeHandle) -> bool,
    ) -> TypeHandle {
        let mut candidate = None;
        for t in types {
            if candidate.is_none() || f(self, candidate.unwrap(), t) {
                candidate = Some(t);
            }
        }
        candidate.unwrap()
    }

    // Return the leftmost type for which no type to the right is a subtype.
    fn get_common_subtype(&mut self, types: Vec<TypeHandle>) -> TypeHandle {
        let mut subtype = None;
        for t in types {
            if subtype.is_none() || self.is_type_subtype_of(t, subtype.unwrap()) {
                subtype = Some(t);
            }
        }
        subtype.unwrap()
    }

    fn get_combined_type_flags(&mut self, types: Vec<TypeHandle>) -> TypeFlags {
        let mut flags = TYPE_FLAGS_NONE;
        for t in types {
            if self.type_flags(t) & TYPE_FLAGS_UNION != 0 {
                flags |= self.get_combined_type_flags(self.type_types(t));
            } else {
                flags |= self.type_flags(t);
            }
        }
        flags
    }

    fn literal_types_with_same_base_type(&mut self, types: Vec<TypeHandle>) -> bool {
        let mut common_base_type = None;
        for t in types {
            if self.type_flags(t) & TYPE_FLAGS_NEVER == 0 {
                let base_type = self.get_base_type_of_literal_type(t);
                if common_base_type.is_none() {
                    common_base_type = Some(base_type);
                }
                if base_type == t || Some(base_type) != common_base_type {
                    return false;
                }
            }
        }
        true
    }

    fn is_from_inference_blocked_source(&mut self, t: TypeHandle) -> bool {
        self.type_symbol_identity(t).is_some_and(|symbol| {
            self.collect_symbol_identity_declarations(symbol)
                .iter()
                .any(|d| self.is_skip_direct_inference_node(*d))
        })
    }

    fn get_single_type_variable_from_intersection_types(
        &self,
        n: &InferenceState<'_>,
        types: &[TypeHandle],
    ) -> Option<TypeHandle> {
        let mut type_variable = None;
        for &t in types {
            if self.type_flags(t) & TYPE_FLAGS_INTERSECTION == 0 {
                return None;
            }
            let v = self
                .type_types(t)
                .into_iter()
                .find(|&t| self.get_inference_info_for_type(n, t).is_some());
            if v.is_none() || type_variable.is_some() && v != type_variable {
                return None;
            }
            type_variable = v;
        }
        type_variable
    }

    fn tuple_types_definitely_unrelated(&self, source: TypeHandle, target: TypeHandle) -> bool {
        let s = self.target_tuple_type_record(source);
        let t = self.target_tuple_type_record(target);
        t.combined_flags & ELEMENT_FLAGS_VARIADIC == 0 && t.min_length > s.min_length
            || t.combined_flags & ELEMENT_FLAGS_VARIABLE == 0
                && (s.combined_flags & ELEMENT_FLAGS_VARIABLE != 0
                    || t.fixed_length < s.fixed_length)
    }

    fn get_inference_info_for_type<'b>(
        &self,
        n: &'b InferenceState<'_>,
        t: TypeHandle,
    ) -> Option<&'b InferenceInfo> {
        if self.type_flags(t) & TYPE_FLAGS_TYPE_VARIABLE != 0 {
            for inference in n.inferences.iter() {
                if t == inference.type_parameter {
                    return Some(inference);
                }
            }
        }
        None
    }

    fn get_inference_info_index_for_type(
        &self,
        n: &InferenceState<'_>,
        t: TypeHandle,
    ) -> Option<usize> {
        if self.type_flags(t) & TYPE_FLAGS_TYPE_VARIABLE != 0 {
            for (index, inference) in n.inferences.iter().enumerate() {
                if t == inference.type_parameter {
                    return Some(index);
                }
            }
        }
        None
    }

    pub(crate) fn merge_inferences(
        &mut self,
        target: &mut [InferenceInfo],
        source: &[InferenceInfo],
    ) {
        for i in 0..target.len() {
            if !has_inference_candidates(&target[i]) && has_inference_candidates(&source[i]) {
                target[i] = clone_inference_info(&source[i]);
            }
        }
    }
}

pub(crate) fn new_inference_info<'a>(type_parameter: TypeHandle) -> InferenceInfo {
    InferenceInfo {
        type_parameter,
        candidates: Vec::new(),
        candidates_present: false,
        contra_candidates: Vec::new(),
        contra_candidates_present: false,
        inferred_type: None,
        priority: INFERENCE_PRIORITY_MAX_VALUE,
        top_level: true,
        is_fixed: false,
        implied_arity: -1,
    }
}

pub(crate) fn clone_inference_info<'a>(info: &InferenceInfo) -> InferenceInfo {
    InferenceInfo {
        type_parameter: info.type_parameter,
        candidates: info.candidates.clone(),
        candidates_present: info.candidates_present,
        contra_candidates: info.contra_candidates.clone(),
        contra_candidates_present: info.contra_candidates_present,
        inferred_type: info.inferred_type,
        priority: info.priority,
        top_level: info.top_level,
        is_fixed: info.is_fixed,
        implied_arity: info.implied_arity,
    }
}

pub(crate) trait ClearCachedInference {
    fn clear_cached_inference(&mut self);
}

impl ClearCachedInference for InferenceInfo {
    fn clear_cached_inference(&mut self) {
        if !self.is_fixed {
            self.inferred_type = None;
        }
    }
}

impl ClearCachedInference for &mut InferenceInfo {
    fn clear_cached_inference(&mut self) {
        if !self.is_fixed {
            self.inferred_type = None;
        }
    }
}

pub(crate) fn clear_cached_inferences<T: ClearCachedInference>(inferences: &mut [T]) {
    for inference in inferences {
        inference.clear_cached_inference();
    }
}

pub(crate) fn has_inference_candidates(info: &InferenceInfo) -> bool {
    !info.candidates.is_empty() || !info.contra_candidates.is_empty()
}

pub(crate) fn has_inference_candidates_or_default<'a>(
    checker: &Checker<'a, '_>,
    info: &InferenceInfo,
) -> bool {
    info.candidates_present
        || info.contra_candidates_present
        || has_type_parameter_default(checker, info.type_parameter)
}

pub(crate) fn has_type_parameter_default<'a>(checker: &Checker<'a, '_>, tp: TypeHandle) -> bool {
    if let Some(symbol) = checker.type_symbol_identity(tp) {
        for d in checker.collect_symbol_identity_declarations(symbol) {
            let store = checker.store_for_node(d);
            if ast::is_type_parameter_declaration(store, d) && store.default_type(d).is_some() {
                return true;
            }
        }
    }
    false
}

pub(crate) fn has_overlapping_inferences(a: &[InferenceInfo], b: &[InferenceInfo]) -> bool {
    for i in 0..a.len() {
        if has_inference_candidates(&a[i]) && has_inference_candidates(&b[i]) {
            return true;
        }
    }
    false
}
