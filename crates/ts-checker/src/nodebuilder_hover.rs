use crate::checker::*;
use crate::{ast, collections, core, nodebuilder, scanner};
use ts_evaluator as evaluator;

// isExpanding returns whether the node builder context is operating in hover-expansion mode.
pub(crate) fn is_expanding(ctx: &NodeBuilderContext) -> bool {
    ctx.max_expansion_depth != -1
}

impl<'a, 'state, 'c, 'e> NodeBuilderImpl<'a, 'state, 'c, 'e> {
    // expandSymbolForHover produces declaration nodes (class, interface, enum, module) for a symbol
    // for expandable hover. This is a focused alternative to the full symbolTableToDeclarationStatements
    // machinery used by declaration emit — it directly builds the declaration nodes hover needs
    // without the declaration-emit scaffolding (deferred privates, symbol name remapping, export
    // modifier computation, alias resolution, visited symbols tracking).
    pub(crate) fn expand_symbol_for_hover(&mut self, symbol: SymbolIdentity) -> Vec<ast::Node> {
        let mut results = Vec::new();
        let flags = self.ch.symbol_identity_flags(symbol);
        if flags & ast::SYMBOL_FLAGS_ENUM != 0 {
            if let Some(node) = self.expand_enum_decl(symbol) {
                results.push(node);
            }
        }
        if flags & ast::SYMBOL_FLAGS_CLASS != 0 {
            if let Some(node) = self.expand_class_decl(symbol) {
                results.push(node);
            }
        }
        // Module/namespace before interface (matching Strada ordering for merged declarations)
        if flags & (ast::SYMBOL_FLAGS_VALUE_MODULE | ast::SYMBOL_FLAGS_NAMESPACE_MODULE) != 0 {
            if let Some(node) = self.expand_module_decl(symbol) {
                results.push(node);
            }
        }
        if flags & ast::SYMBOL_FLAGS_INTERFACE != 0 && flags & ast::SYMBOL_FLAGS_CLASS == 0 {
            if let Some(node) = self.expand_interface_decl(symbol) {
                results.push(node);
            }
        }
        results
    }

    // expandEnumDecl produces an EnumDeclaration node with all members.
    fn expand_enum_decl(&mut self, symbol: SymbolIdentity) -> Option<ast::Node> {
        let name = self.ch.missing_name_symbol_identity_name(symbol);
        self.ctx.approximate_length += 9 + name.len();
        let symbol_type = self
            .ch
            .get_type_of_symbol_identity_at_location(symbol, None);
        let member_props = self
            .ch
            .get_properties_of_type(symbol_type)
            .into_iter()
            .filter(|p| self.ch.symbol_identity_flags(*p) & ast::SYMBOL_FLAGS_ENUM_MEMBER != 0)
            .collect::<Vec<_>>();
        let mut members: Vec<ast::Node> = Vec::new();
        for (i, p) in member_props.iter().enumerate() {
            if self.check_truncation_length_if_expanding() && i + 3 < member_props.len() - 1 {
                self.ctx.expansion_truncated = true;
                let name = self.e.factory.node_factory.new_string_literal(
                    format!(" ... {} more ... ", member_props.len() - i - 1),
                    ast::TOKEN_FLAGS_NONE,
                );
                members.push(self.e.factory.node_factory.new_enum_member(name, None));
                let last = member_props[member_props.len() - 1];
                let last_name = self.ch.missing_name_symbol_identity_name(last);
                let name = self.e.factory.node_factory.new_identifier(&last_name);
                let initializer = self.enum_member_initializer(last);
                members.push(
                    self.e
                        .factory
                        .node_factory
                        .new_enum_member(name, initializer),
                );
                break;
            }
            let declarations = self.ch.collect_symbol_identity_declarations(*p);
            let member_decl = declarations
                .iter()
                .copied()
                .find(|d| ast::is_enum_member(self.ch.store_for_node(*d), *d));
            let initializer = if let Some(member_decl) = member_decl.as_ref() {
                let member_store = self.ch.store_for_node(*member_decl);
                if let Some(initializer) = member_store.initializer(*member_decl) {
                    Some(hover_deep_clone_node(
                        &mut self.e.factory.node_factory,
                        member_store,
                        initializer,
                    ))
                } else {
                    self.enum_member_initializer(*p)
                }
            } else {
                self.enum_member_initializer(*p)
            };
            let p_name = self.ch.missing_name_symbol_identity_name(*p);
            self.ctx.approximate_length += 4 + p_name.len();
            if initializer.is_some() {
                self.ctx.approximate_length += 5; // " = " + value estimate
            }
            let name = self.e.factory.node_factory.new_identifier(&p_name);
            members.push(
                self.e
                    .factory
                    .node_factory
                    .new_enum_member(name, initializer),
            );
        }

        let const_modifier = if is_const_enum_symbol_identity(self.ch, symbol) {
            ast::MODIFIER_FLAGS_CONST
        } else {
            ast::MODIFIER_FLAGS_NONE
        };
        let mods = if const_modifier != 0 {
            Some(hover_new_factory_modifier_list(
                &mut self.e.factory.node_factory,
                const_modifier,
            ))
        } else {
            None
        };
        let name = self.e.factory.node_factory.new_identifier(name);
        let members = hover_new_factory_node_list(&mut self.e.factory.node_factory, members);
        Some(
            self.e
                .factory
                .node_factory
                .new_enum_declaration(mods, name, members),
        )
    }

    fn enum_member_initializer(&mut self, p: SymbolIdentity) -> Option<ast::Node> {
        let member_decl = self
            .ch
            .collect_symbol_identity_declarations(p)
            .iter()
            .copied()
            .find(|d| ast::is_enum_member(self.ch.store_for_node(*d), *d));
        let member_decl = member_decl;
        let member_decl = member_decl?;
        let val = self.ch.get_enum_member_value(member_decl).value;
        match val {
            evaluator::Value::String(v) => Some(
                self.e
                    .factory
                    .node_factory
                    .new_string_literal(v, ast::TOKEN_FLAGS_NONE),
            ),
            evaluator::Value::Number(v) => Some(
                self.e
                    .factory
                    .node_factory
                    .new_numeric_literal(v.to_string(), ast::TOKEN_FLAGS_NONE),
            ),
            _ => None,
        }
    }

    // expandClassDecl produces a ClassDeclaration node with heritage clauses and members.
    fn expand_class_decl(&mut self, symbol: SymbolIdentity) -> Option<ast::Node> {
        let name = self.ch.missing_name_symbol_identity_name(symbol);
        let symbol_declarations = self.ch.collect_symbol_identity_declarations(symbol);
        self.ctx.approximate_length += 9 + name.len();

        let class_like_declarations = symbol_declarations
            .iter()
            .filter(|declaration| {
                ast::is_class_like(self.ch.store_for_node(**declaration), **declaration)
            })
            .map(|declaration| *declaration)
            .collect::<Vec<_>>();
        let original_decl = class_like_declarations.first();
        let old_enclosing = self.ctx.enclosing_declaration;
        if let Some(original_decl) = original_decl {
            self.ctx.enclosing_declaration = Some(*original_decl);
        }

        let local_params =
            self.hover_local_type_parameters_of_class_or_interface_or_type_alias(symbol);
        let type_param_decls = local_params
            .into_iter()
            .map(|p| self.type_parameter_to_declaration(p))
            .collect::<Vec<_>>();

        let declared_type = self
            .ch
            .get_declared_type_of_symbol_identity_or_error(symbol);
        let class_type = self
            .ch
            .get_type_with_this_argument(declared_type, None, false);
        let base_types = self.ch.get_base_types(self.ch.get_target_type(class_type));
        let static_type = self
            .ch
            .get_type_of_symbol_identity_at_location(symbol, None);
        let is_class = self
            .ch
            .type_symbol_identity(static_type)
            .is_some_and(|symbol| {
                self.ch
                    .missing_name_symbol_identity_value_declaration(symbol)
                    .is_some_and(|value_declaration| {
                        ast::is_class_like(
                            self.ch.store_for_node(value_declaration),
                            value_declaration,
                        )
                    })
            });
        let static_base_type = if is_class {
            self.ch.get_base_constructor_type_of_class(declared_type)
        } else {
            self.ch.semantic_state.semantic_handles().any_type
        };

        // Heritage clauses
        let heritage_clauses = self.hover_heritage_clauses(class_like_declarations);

        // Instance members via addPropertyToElementList (reusing existing serialization),
        // then convert TypeElements to ClassElements and add class-specific modifiers
        let all_props = self
            .ch
            .resolve_structured_type_members(class_type)
            .collect_properties();
        let symbol_props =
            self.filter_inherited_property_identities(class_type, base_types.clone(), all_props);
        let public_props = symbol_props
            .iter()
            .copied()
            .filter(|s| !is_hash_private_identity(self.ch, *s))
            .collect::<Vec<_>>();
        let has_private = symbol_props
            .iter()
            .any(|s| is_hash_private_identity(self.ch, *s));

        let mut instance_members = Vec::new();
        instance_members =
            self.serialize_property_identities_with_truncation(public_props, instance_members);
        instance_members =
            type_elements_to_class_elements(&mut self.e.factory.node_factory, instance_members);
        instance_members = self.add_class_modifiers(instance_members, false);

        // Static members
        let static_props = self
            .ch
            .resolve_structured_type_members(static_type)
            .collect_properties()
            .into_iter()
            .filter(|p| {
                let flags = self.ch.symbol_identity_flags(*p);
                flags & ast::SYMBOL_FLAGS_PROTOTYPE == 0
                    && self.ch.missing_name_symbol_identity_name(*p) != "prototype"
                    && !self.is_namespace_member(*p)
            })
            .collect::<Vec<_>>();
        let mut static_members = Vec::new();
        static_members =
            self.serialize_property_identities_with_truncation(static_props, static_members);
        static_members =
            type_elements_to_class_elements(&mut self.e.factory.node_factory, static_members);
        static_members = self.add_class_modifiers(static_members, true);

        // Hash-private members
        let mut private_members = Vec::new();
        if has_private {
            private_members = self.serialize_property_identities_with_truncation(
                symbol_props
                    .into_iter()
                    .filter(|symbol| is_hash_private_identity(self.ch, *symbol))
                    .collect(),
                private_members,
            );
            private_members =
                type_elements_to_class_elements(&mut self.e.factory.node_factory, private_members);
        }

        // Constructors
        let constructors =
            self.serialize_constructors(static_type, Some(static_base_type), is_class, symbol);

        // Index signatures
        let index_sigs =
            self.serialize_index_signatures_of_type(class_type, base_types.first().copied());

        let mut all_members = Vec::with_capacity(
            index_sigs.len()
                + static_members.len()
                + constructors.len()
                + instance_members.len()
                + private_members.len(),
        );
        all_members.extend(index_sigs);
        all_members.extend(static_members);
        all_members.extend(constructors);
        all_members.extend(instance_members);
        all_members.extend(private_members);

        self.ctx.enclosing_declaration = old_enclosing;
        let type_param_decls = type_param_decls.into_iter().collect::<Vec<_>>();
        let heritage_clauses = heritage_clauses.into_iter().collect::<Vec<_>>();
        let all_members = all_members.into_iter().collect::<Vec<_>>();
        let name = self.e.factory.node_factory.new_identifier(name);
        let type_param_decls =
            hover_new_factory_node_list(&mut self.e.factory.node_factory, type_param_decls);
        let heritage_clauses =
            hover_new_factory_node_list(&mut self.e.factory.node_factory, heritage_clauses);
        let all_members =
            hover_new_factory_node_list(&mut self.e.factory.node_factory, all_members);
        Some(self.e.factory.node_factory.new_class_declaration(
            None,
            Some(name),
            Some(type_param_decls),
            Some(heritage_clauses),
            all_members,
        ))
    }

    // addClassModifiers post-processes class member nodes to add class-specific modifiers
    // (private, protected, public, abstract, static) based on the original symbol declarations.
    fn add_class_modifiers(
        &mut self,
        mut members: Vec<ast::Node>,
        is_static: bool,
    ) -> Vec<ast::Node> {
        for i in 0..members.len() {
            // Find the symbol for this member by matching the property name
            let mut member_symbol = None;
            let member_name = self.e.factory.node_factory.store().name(members[i]);
            if let Some(member_name) = member_name {
                if ast::is_identifier(self.e.factory.node_factory.store(), member_name) {
                    let member_name_text = self.e.factory.node_factory.store().text(member_name);
                    member_symbol = self.id_to_symbol.iter().find_map(|(id, symbol)| {
                        (self.e.factory.node_factory.store().text(*id) == member_name_text)
                            .then_some(*symbol)
                    });
                }
            }
            let Some(member_symbol) = member_symbol else {
                continue;
            };
            let mut mod_flags = self
                .ch
                .declaration_modifier_flags_from_symbol_identity(member_symbol)
                & !ast::MODIFIER_FLAGS_ASYNC;
            if is_static {
                mod_flags |= ast::MODIFIER_FLAGS_STATIC;
            }
            if mod_flags != 0
                && ast::can_have_modifiers(self.e.factory.node_factory.store(), members[i])
            {
                let existing = ast::get_combined_modifier_flags(
                    self.e.factory.node_factory.store(),
                    members[i],
                );
                if mod_flags != existing {
                    let modifier_list = hover_new_factory_modifier_list(
                        &mut self.e.factory.node_factory,
                        mod_flags | existing,
                    );
                    let replaced = ast::replace_modifiers(
                        &mut self.e.factory.node_factory,
                        members[i],
                        Some(modifier_list),
                    );
                    members[i] = replaced;
                }
            }
        }
        members
    }

    // expandInterfaceDecl produces an InterfaceDeclaration with members.
    // Reuses addPropertyToElementList for property serialization and
    // signatureToSignatureDeclarationHelper for signatures.
    fn expand_interface_decl(&mut self, symbol: SymbolIdentity) -> Option<ast::Node> {
        let name = self.ch.missing_name_symbol_identity_name(symbol);
        let symbol_declarations = self.ch.collect_symbol_identity_declarations(symbol);
        self.ctx.approximate_length += 14 + name.len();

        let interface_type = self
            .ch
            .get_declared_type_of_symbol_identity_or_error(symbol);
        let interface_declarations = symbol_declarations
            .iter()
            .filter(|declaration| {
                ast::is_interface_declaration(self.ch.store_for_node(**declaration), **declaration)
            })
            .copied()
            .collect::<Vec<_>>();
        let local_params =
            self.hover_local_type_parameters_of_class_or_interface_or_type_alias(symbol);
        let type_param_decls = local_params
            .into_iter()
            .map(|p| self.type_parameter_to_declaration(p))
            .collect::<Vec<_>>();
        let base_types = self.ch.get_base_types(interface_type);
        let base_type = if !base_types.is_empty() {
            Some(self.ch.get_intersection_type(base_types.clone()))
        } else {
            None
        };

        // Members: reuse existing serialization functions
        let resolved = self
            .ch
            .resolve_structured_type_members(interface_type)
            .clone();
        let mut members = Vec::new();

        // Index signatures, filtering those identical to base
        members.extend(self.serialize_index_signatures_of_type(interface_type, base_type));
        // Construct signatures (skip abstract)
        for sig in resolved.collect_construct_signatures() {
            if self.ch.signature_record(sig).flags & SIGNATURE_FLAGS_ABSTRACT != 0 {
                continue;
            }
            members.push(self.signature_to_signature_declaration_helper(
                sig,
                ast::KIND_CONSTRUCT_SIGNATURE,
                None,
            ));
        }
        // Call signatures
        for sig in resolved.collect_call_signatures() {
            members.push(self.signature_to_signature_declaration_helper(
                sig,
                ast::KIND_CALL_SIGNATURE,
                None,
            ));
        }
        // Properties, filtering inherited
        let filtered_props = self.filter_inherited_property_identities(
            interface_type,
            base_types,
            resolved.properties.clone(),
        );
        members = self.serialize_property_identities_with_truncation(filtered_props, members);

        // Heritage clauses
        let heritage_clauses = self.hover_heritage_clauses(interface_declarations);

        let type_param_decls = type_param_decls.into_iter().collect::<Vec<_>>();
        let heritage_clauses = heritage_clauses.into_iter().collect::<Vec<_>>();
        let members = members.into_iter().collect::<Vec<_>>();
        let name = self.e.factory.node_factory.new_identifier(name.as_str());
        let type_param_decls =
            hover_new_factory_node_list(&mut self.e.factory.node_factory, type_param_decls);
        let heritage_clauses =
            hover_new_factory_node_list(&mut self.e.factory.node_factory, heritage_clauses);
        let members = hover_new_factory_node_list(&mut self.e.factory.node_factory, members);
        Some(self.e.factory.node_factory.new_interface_declaration(
            None,
            name,
            Some(type_param_decls),
            Some(heritage_clauses),
            members,
        ))
    }

    fn hover_heritage_clauses(&mut self, declarations: Vec<ast::Node>) -> Vec<ast::Node> {
        let mut extends_types = Vec::new();
        let mut implements_types = Vec::new();
        for declaration in declarations {
            let source = self.ch.store_for_node(declaration);
            for heritage_element in ast::get_extends_heritage_clause_elements(source, declaration) {
                extends_types.push(hover_deep_clone_node(
                    &mut self.e.factory.node_factory,
                    source,
                    heritage_element,
                ));
            }
            for heritage_element in
                ast::get_implements_heritage_clause_elements(source, declaration)
            {
                implements_types.push(hover_deep_clone_node(
                    &mut self.e.factory.node_factory,
                    source,
                    heritage_element,
                ));
            }
        }

        let mut heritage_clauses: Vec<ast::Node> = Vec::new();
        if !extends_types.is_empty() {
            let types =
                hover_new_factory_node_list(&mut self.e.factory.node_factory, extends_types);
            heritage_clauses.push(
                self.e
                    .factory
                    .node_factory
                    .new_heritage_clause(ast::KIND_EXTENDS_KEYWORD, types),
            );
        }
        if !implements_types.is_empty() {
            let types =
                hover_new_factory_node_list(&mut self.e.factory.node_factory, implements_types);
            heritage_clauses.push(
                self.e
                    .factory
                    .node_factory
                    .new_heritage_clause(ast::KIND_IMPLEMENTS_KEYWORD, types),
            );
        }
        heritage_clauses
    }

    // serializePropertiesWithTruncation iterates properties using addPropertyToElementList,
    // with truncation checks matching Strada's createTypeNodesFromResolvedType behavior.
    fn serialize_property_identities_with_truncation(
        &mut self,
        properties: Vec<SymbolIdentity>,
        mut elements: Vec<ast::Node>,
    ) -> Vec<ast::Node> {
        let properties = properties
            .into_iter()
            .filter(|p| self.ch.symbol_identity_flags(*p) & ast::SYMBOL_FLAGS_PROTOTYPE == 0)
            .collect::<Vec<_>>();
        for (i, p) in properties.iter().enumerate() {
            if self.check_truncation_length_if_expanding() && i + 3 < properties.len() - 1 {
                self.ctx.expansion_truncated = true;
                let text = format!("... {} more ...", properties.len() - i - 1);
                let name = self.e.factory.node_factory.new_identifier(text);
                elements.push(
                    self.e
                        .factory
                        .node_factory
                        .new_property_signature_declaration(None, name, None, None, None),
                );
                elements = self.add_property_identity_to_element_list(
                    properties[properties.len() - 1],
                    elements,
                );
                break;
            }
            elements = self.add_property_identity_to_element_list(*p, elements);
        }
        elements
    }

    // serializeConstructors builds constructor signature(s) for a class, with base type filtering.
    fn serialize_constructors(
        &mut self,
        static_type: TypeHandle,
        static_base_type: Option<TypeHandle>,
        is_class: bool,
        symbol: SymbolIdentity,
    ) -> Vec<ast::Node> {
        let value_declaration = self
            .ch
            .missing_name_symbol_identity_value_declaration(symbol);
        let is_non_constructable = !is_class
            && value_declaration.is_some()
            && ast::is_in_js_file(
                self.ch
                    .store_for_node(value_declaration.as_ref().copied().unwrap()),
                value_declaration.as_ref().copied().unwrap(),
            )
            && self
                .ch
                .get_signatures_of_type(static_type, SIGNATURE_KIND_CONSTRUCT)
                .is_empty();
        if is_non_constructable {
            self.ctx.approximate_length += 21;
            let modifier_list = hover_new_factory_modifier_list(
                &mut self.e.factory.node_factory,
                ast::MODIFIER_FLAGS_PRIVATE,
            );
            let parameters =
                hover_new_factory_node_list(&mut self.e.factory.node_factory, Vec::new());
            return vec![self.e.factory.node_factory.new_constructor_declaration(
                Some(modifier_list),
                None,
                parameters,
                None,
                None,
                None,
            )];
        }
        let signatures = self
            .ch
            .get_signatures_of_type(static_type, SIGNATURE_KIND_CONSTRUCT);
        if let Some(static_base_type) = static_base_type {
            let base_sigs = self
                .ch
                .get_signatures_of_type(static_base_type, SIGNATURE_KIND_CONSTRUCT);
            if base_sigs.is_empty()
                && signatures
                    .iter()
                    .all(|sig| self.ch.signature_record(*sig).parameters.is_empty())
            {
                return Vec::new();
            }
            if base_sigs.len() == signatures.len() {
                let mut all_match = true;
                for i in 0..base_sigs.len() {
                    if self.ch.compare_signatures_identical(
                        signatures[i],
                        base_sigs[i],
                        false,
                        false,
                        true,
                        Checker::compare_types_identical,
                    ) != TERNARY_TRUE
                    {
                        all_match = false;
                        break;
                    }
                }
                if all_match {
                    return Vec::new();
                }
            }
            let mut private_protected = ast::MODIFIER_FLAGS_NONE;
            for sig in &signatures {
                if let Some(declaration) = self.ch.signature_record(*sig).declaration {
                    private_protected |= self
                        .ch
                        .store_for_node(declaration)
                        .modifiers(declaration)
                        .map(|modifiers| modifiers.modifier_flags())
                        .unwrap_or(ast::MODIFIER_FLAGS_NONE)
                        & (ast::MODIFIER_FLAGS_PRIVATE | ast::MODIFIER_FLAGS_PROTECTED);
                }
            }
            if private_protected != 0 {
                let modifier_list = hover_new_factory_modifier_list(
                    &mut self.e.factory.node_factory,
                    private_protected,
                );
                let parameters =
                    hover_new_factory_node_list(&mut self.e.factory.node_factory, Vec::new());
                return vec![self.e.factory.node_factory.new_constructor_declaration(
                    Some(modifier_list),
                    None,
                    parameters,
                    None,
                    None,
                    None,
                )];
            }
        } else if signatures
            .iter()
            .all(|sig| self.ch.signature_record(*sig).parameters.is_empty())
        {
            return Vec::new();
        }
        let mut result = Vec::new();
        for sig in signatures {
            self.ctx.approximate_length += 1;
            result.push(self.signature_to_signature_declaration_helper(
                sig,
                ast::KIND_CONSTRUCTOR,
                None,
            ));
        }
        result
    }

    // serializeIndexSignaturesOfType builds index signature declarations, filtering those identical to baseType.
    fn serialize_index_signatures_of_type(
        &mut self,
        input: TypeHandle,
        base_type: Option<TypeHandle>,
    ) -> Vec<ast::Node> {
        let mut result = Vec::new();
        for info in self.ch.get_index_infos_of_type(input) {
            let info_record = self.ch.index_info_record(info).clone();
            if let Some(base_type) = base_type {
                let base_info = self
                    .ch
                    .get_index_info_of_type(base_type, info_record.key_type.unwrap());
                if let (Some(value_type), Some(base_info)) = (info_record.value_type, base_info) {
                    if let Some(base_value_type) = self.ch.index_info_record(base_info).value_type {
                        if self.ch.is_type_identical_to(value_type, base_value_type) {
                            continue;
                        }
                    }
                }
            }
            result.push(self.index_info_to_index_signature_declaration_helper(info, None));
        }
        result
    }

    // serializeNamespaceMember produces the appropriate declaration node for a namespace member
    // based on its symbol flags (type alias, enum, class, interface, nested namespace, or variable).
    fn serialize_namespace_member(
        &mut self,
        resolved: SymbolIdentity,
        name: &str,
    ) -> Option<ast::Node> {
        let resolved_flags = self.ch.symbol_identity_flags(resolved);
        if resolved_flags & ast::SYMBOL_FLAGS_TYPE_ALIAS != 0 {
            return self.serialize_type_alias_for_namespace(resolved, name);
        }
        if resolved_flags & ast::SYMBOL_FLAGS_ENUM != 0 {
            return self.expand_enum_decl(resolved);
        }
        if resolved_flags & ast::SYMBOL_FLAGS_CLASS != 0 {
            return self.expand_class_decl(resolved);
        }
        if resolved_flags & ast::SYMBOL_FLAGS_INTERFACE != 0 {
            return self.expand_interface_decl(resolved);
        }
        if resolved_flags & (ast::SYMBOL_FLAGS_VALUE_MODULE | ast::SYMBOL_FLAGS_NAMESPACE_MODULE)
            != 0
        {
            return self.expand_module_decl(resolved);
        }
        let symbol_type = self.ch.get_type_of_symbol_identity(resolved);
        let t = self.ch.get_widened_type(symbol_type);
        self.ctx.approximate_length += name.len() + 5;
        let type_node = self
            .serialize_type_for_declaration_for_symbol_identity(None, Some(t), Some(resolved), true)
            .clone();
        let name = self.e.factory.node_factory.new_identifier(name);
        let declaration =
            self.e
                .factory
                .node_factory
                .new_variable_declaration(name, None, Some(type_node), None);
        let declarations =
            hover_new_factory_node_list(&mut self.e.factory.node_factory, vec![declaration]);
        let declaration_list = self
            .e
            .factory
            .node_factory
            .new_variable_declaration_list(declarations, ast::NODE_FLAGS_LET);
        Some(
            self.e
                .factory
                .node_factory
                .new_variable_statement(None, declaration_list),
        )
    }

    // expandModuleDecl produces a ModuleDeclaration with exported members.
    fn expand_module_decl(&mut self, symbol: SymbolIdentity) -> Option<ast::Node> {
        let exports = self
            .ch
            .with_symbol_identity_export_table(symbol, |exports| {
                exports
                    .map(|exports| {
                        let mut symbols = Vec::with_capacity(exports.len());
                        exports.for_each_value(|symbol| symbols.push(symbol));
                        symbols
                    })
                    .unwrap_or_default()
            });
        let mut members = Vec::new();
        for sym in exports {
            // Filter to namespace-relevant members
            if !self.is_namespace_member(sym) {
                continue;
            }
            let sym_name = self.ch.missing_name_symbol_identity_name(sym);
            if !scanner::is_identifier_text(&sym_name, core::LANGUAGE_VARIANT_STANDARD) {
                continue;
            }
            members.push(sym);
        }
        self.sort_hover_symbol_identities(&mut members);
        self.ctx.approximate_length += 14;

        // Use the same name as symbol display.
        let old_flags = self.ctx.flags;
        self.ctx.flags |= nodebuilder::FLAGS_WRITE_TYPE_PARAMETERS_IN_QUALIFIED_NAME
            | nodebuilder::Flags::from(SYMBOL_FORMAT_FLAGS_USE_ONLY_EXTERNAL_ALIASING);
        let local_name = self
            .symbol_identity_to_expression(symbol, ast::SYMBOL_FLAGS_ALL)
            .unwrap_or_else(|| {
                let name = self.ch.missing_name_symbol_identity_name(symbol);
                self.e.factory.node_factory.new_identifier(name)
            });
        self.ctx.flags = old_flags;

        struct HoverStatement {
            node: ast::Node,
            is_local: bool, // local declarations (e.g. alias targets) should not get export modifier
        }
        let mut body_stmts = Vec::new();
        let mut emitted_locals = collections::Set::new();
        let mut i = 0;
        while i < members.len() {
            let m = members[i];
            if self.check_truncation_length_if_expanding() && i + 3 < members.len() - 1 {
                self.ctx.expansion_truncated = true;
                let name = self
                    .e
                    .factory
                    .node_factory
                    .new_identifier(format!("... ({} more) ...", members.len() - i - 1));
                let statement = self.e.factory.node_factory.new_expression_statement(name);
                body_stmts.push(HoverStatement {
                    node: statement,
                    is_local: false,
                });
                i = members.len() - 2; // skip to last member after i++ at end of iteration
                i += 1;
                continue;
            }

            // Handle alias/re-export symbols
            let m_flags = self.ch.symbol_identity_flags(m);
            if m_flags & ast::SYMBOL_FLAGS_ALIAS != 0 {
                let target = {
                    let alias_decl = self.get_declaration_of_alias_symbol_identity(m);
                    let alias_target = self.ch.get_target_of_alias_declaration(
                        alias_decl, true, /*dontRecursivelyResolve*/
                    );
                    self.ch.get_merged_symbol_identity(alias_target)
                };
                if let Some(target) = target {
                    let target_flags = self.ch.symbol_identity_flags(target);
                    // If the alias target is a local symbol (not itself an export), emit its declaration first
                    if target_flags
                        & (ast::SYMBOL_FLAGS_BLOCK_SCOPED_VARIABLE
                            | ast::SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE
                            | ast::SYMBOL_FLAGS_PROPERTY)
                        != 0
                    {
                        if emitted_locals.add_if_absent(target) {
                            let target_type = self.ch.get_type_of_symbol_identity(target);
                            let target_name = self.ch.missing_name_symbol_identity_name(target);
                            let local_type = self.ch.get_widened_type(target_type);
                            self.ctx.approximate_length += target_name.len() + 5;
                            let local_type_node = self
                                .serialize_type_for_declaration_for_symbol_identity(
                                    None,
                                    Some(local_type),
                                    Some(target),
                                    true,
                                )
                                .clone();
                            let local_name = self
                                .e
                                .factory
                                .node_factory
                                .new_identifier(target_name.as_str());
                            let local_declaration =
                                self.e.factory.node_factory.new_variable_declaration(
                                    local_name,
                                    None,
                                    Some(local_type_node),
                                    None,
                                );
                            let local_declarations = hover_new_factory_node_list(
                                &mut self.e.factory.node_factory,
                                vec![local_declaration],
                            );
                            let local_declaration_list =
                                self.e.factory.node_factory.new_variable_declaration_list(
                                    local_declarations,
                                    ast::NODE_FLAGS_LET,
                                );
                            let local_stmt = self
                                .e
                                .factory
                                .node_factory
                                .new_variable_statement(None, local_declaration_list);
                            body_stmts.push(HoverStatement {
                                node: local_stmt,
                                is_local: true,
                            });
                        }
                    }
                    let target_name = self.ch.missing_name_symbol_identity_name(target);
                    let m_name = self.ch.missing_name_symbol_identity_name(m);
                    self.ctx.approximate_length += 16 + m_name.len();
                    let property_name = if m_name != target_name {
                        Some(
                            self.e
                                .factory
                                .node_factory
                                .new_identifier(target_name.as_str()),
                        )
                    } else {
                        None
                    };
                    let name = self.e.factory.node_factory.new_identifier(m_name.as_str());
                    let specifier = self.e.factory.node_factory.new_export_specifier(
                        false,
                        property_name,
                        name,
                    );
                    let specifiers = hover_new_factory_node_list(
                        &mut self.e.factory.node_factory,
                        vec![specifier],
                    );
                    let export_clause = self.e.factory.node_factory.new_named_exports(specifiers);
                    let stmt = self.e.factory.node_factory.new_export_declaration(
                        None,
                        false,
                        Some(export_clause),
                        None,
                        None,
                    );
                    body_stmts.push(HoverStatement {
                        node: stmt,
                        is_local: false,
                    });
                    i += 1;
                    continue;
                }
            }

            let resolved = self.resolve_hover_symbol_identity(m);
            let resolved_flags = self.ch.symbol_identity_flags(resolved);

            // Handle functions as function declarations
            if resolved_flags & (ast::SYMBOL_FLAGS_FUNCTION | ast::SYMBOL_FLAGS_METHOD) != 0 {
                let t = self.ch.get_type_of_symbol_identity(resolved);
                let sigs = self.ch.get_signatures_of_type(t, SIGNATURE_KIND_CALL);
                for sig in sigs {
                    self.ctx.approximate_length += 1;
                    let name = self
                        .e
                        .factory
                        .node_factory
                        .new_identifier(self.ch.missing_name_symbol_identity_name(m).as_str());
                    let decl = self.signature_to_signature_declaration_helper(
                        sig,
                        ast::KIND_FUNCTION_DECLARATION,
                        Some(SignatureToSignatureDeclarationOptions {
                            name: Some(name),
                            ..Default::default()
                        }),
                    );
                    body_stmts.push(HoverStatement {
                        node: decl,
                        is_local: false,
                    });
                }
                // If the function also has namespace characteristics, emit an empty namespace.
                let merged = self.ch.get_merged_symbol_identity(Some(resolved));
                let has_module_exports = merged.is_some_and(|merged| {
                    self.ch.symbol_identity_flags(merged)
                        & (ast::SYMBOL_FLAGS_VALUE_MODULE | ast::SYMBOL_FLAGS_NAMESPACE_MODULE)
                        != 0
                        && self
                            .ch
                            .collect_symbol_identity_export_table(merged)
                            .is_some_and(|exports| !exports.is_empty())
                });
                if !has_module_exports {
                    let name = self
                        .e
                        .factory
                        .node_factory
                        .new_identifier(self.ch.missing_name_symbol_identity_name(m).as_str());
                    let statements =
                        hover_new_factory_node_list(&mut self.e.factory.node_factory, Vec::new());
                    let body = self.e.factory.node_factory.new_module_block(statements);
                    body_stmts.push(HoverStatement {
                        node: self.e.factory.node_factory.new_module_declaration(
                            None,
                            ast::KIND_NAMESPACE_KEYWORD,
                            name,
                            Some(body),
                        ),
                        is_local: false,
                    });
                }
                i += 1;
                continue;
            }

            // Handle remaining member kinds (type alias, enum, class, interface, namespace, variable)
            let m_name = self.ch.missing_name_symbol_identity_name(m);
            if let Some(node) = self.serialize_namespace_member(resolved, &m_name) {
                body_stmts.push(HoverStatement {
                    node,
                    is_local: false,
                });
            }
            i += 1;
        }

        // Add export modifier to exported statements (skip local declarations and ExportDeclarations).
        for s in &mut body_stmts {
            if s.is_local || ast::is_export_declaration(self.e.factory.node_factory.store(), s.node)
            {
                continue;
            }
            if ast::can_have_modifiers(self.e.factory.node_factory.store(), s.node) {
                let mf =
                    ast::get_combined_modifier_flags(self.e.factory.node_factory.store(), s.node)
                        | ast::MODIFIER_FLAGS_EXPORT;
                let modifier_list =
                    hover_new_factory_modifier_list(&mut self.e.factory.node_factory, mf);
                s.node = ast::replace_modifiers(
                    &mut self.e.factory.node_factory,
                    s.node,
                    Some(modifier_list),
                );
            }
        }

        // Collect nodes, stripping export if all statements are exported.
        let mut body_statements = body_stmts.iter().map(|s| s.node).collect::<Vec<_>>();
        let all_exported = !body_statements.is_empty()
            && body_statements.iter().all(|d| {
                ast::has_syntactic_modifier(
                    self.e.factory.node_factory.store(),
                    *d,
                    ast::MODIFIER_FLAGS_EXPORT,
                )
            });
        if all_exported {
            for stmt in &mut body_statements {
                if ast::can_have_modifiers(self.e.factory.node_factory.store(), *stmt) {
                    let mf = ast::get_combined_modifier_flags(
                        self.e.factory.node_factory.store(),
                        *stmt,
                    ) & !ast::MODIFIER_FLAGS_EXPORT;
                    let modifier_list =
                        hover_new_factory_modifier_list(&mut self.e.factory.node_factory, mf);
                    *stmt = ast::replace_modifiers(
                        &mut self.e.factory.node_factory,
                        *stmt,
                        Some(modifier_list),
                    );
                }
            }
        }

        let mut keyword = ast::KIND_NAMESPACE_KEYWORD;
        if !ast::is_identifier(self.e.factory.node_factory.store(), local_name.clone()) {
            keyword = ast::KIND_MODULE_KEYWORD;
        }
        let body_statements = body_statements.into_iter().collect::<Vec<_>>();
        let statements =
            hover_new_factory_node_list(&mut self.e.factory.node_factory, body_statements);
        let body = self.e.factory.node_factory.new_module_block(statements);
        Some(self.e.factory.node_factory.new_module_declaration(
            None,
            keyword,
            local_name.clone(),
            Some(body),
        ))
    }

    // serializeTypeAliasForNamespace produces a TypeAliasDeclaration for a type alias inside a namespace body.
    fn serialize_type_alias_for_namespace(
        &mut self,
        symbol: SymbolIdentity,
        name: &str,
    ) -> Option<ast::Node> {
        let alias_type = self
            .ch
            .get_declared_type_of_symbol_identity_or_error(symbol);
        let type_params =
            self.hover_local_type_parameters_of_class_or_interface_or_type_alias(symbol);
        let type_param_decls = type_params
            .into_iter()
            .map(|p| self.type_parameter_to_declaration(p))
            .collect::<Vec<_>>();
        let old_flags = self.ctx.flags;
        let old_internal_flags = self.ctx.internal_flags;
        let old_depth = self.ctx.depth;
        self.ctx.flags |= nodebuilder::FLAGS_IN_TYPE_ALIAS;
        let type_node = self.type_to_type_node(alias_type);
        self.ctx.flags = old_flags;
        self.ctx.internal_flags = old_internal_flags;
        self.ctx.depth = old_depth;
        self.ctx.approximate_length += 8 + name.len();
        let type_param_decls = type_param_decls.into_iter().collect::<Vec<_>>();
        let name = self.e.factory.node_factory.new_identifier(name);
        let type_param_decls =
            hover_new_factory_node_list(&mut self.e.factory.node_factory, type_param_decls);
        Some(self.e.factory.node_factory.new_type_alias_declaration(
            None,
            name,
            Some(type_param_decls),
            type_node.unwrap().clone(),
        ))
    }

    fn hover_local_type_parameters_of_class_or_interface_or_type_alias(
        &mut self,
        symbol: SymbolIdentity,
    ) -> Vec<TypeHandle> {
        let declarations = self.ch.collect_symbol_identity_declarations(symbol);
        let mut results = Vec::new();
        for node in declarations {
            let store = self.ch.store_for_node(node);
            if ast::node_kind_is(
                store,
                node,
                &[
                    ast::Kind::InterfaceDeclaration,
                    ast::Kind::ClassDeclaration,
                    ast::Kind::ClassExpression,
                ],
            ) || crate::utilities::is_type_alias(store, node)
            {
                results = self.ch.append_type_parameters(
                    results,
                    store.type_parameters(node).into_iter().flatten().collect(),
                );
            }
        }
        results
    }

    fn resolve_hover_symbol_identity(&mut self, symbol: SymbolIdentity) -> SymbolIdentity {
        let merged = self
            .ch
            .get_merged_symbol_identity(Some(symbol))
            .unwrap_or(symbol);
        self.ch.resolve_symbol_identity(merged, false)
    }

    fn sort_hover_symbol_identities(&self, symbols: &mut [SymbolIdentity]) {
        symbols.sort_by(|&left, &right| self.compare_hover_symbol_identities(left, right));
    }

    fn compare_hover_symbol_identities(
        &self,
        left: SymbolIdentity,
        right: SymbolIdentity,
    ) -> std::cmp::Ordering {
        if left == right {
            return std::cmp::Ordering::Equal;
        }
        let left_declarations = self.ch.collect_symbol_identity_declarations(left);
        let right_declarations = self.ch.collect_symbol_identity_declarations(right);
        let declaration_order = self.compare_hover_declarations(
            left_declarations.first().copied(),
            right_declarations.first().copied(),
        );
        if declaration_order != std::cmp::Ordering::Equal {
            return declaration_order;
        }
        self.ch
            .missing_name_symbol_identity_name_ref(left)
            .cmp(self.ch.missing_name_symbol_identity_name_ref(right))
            .then_with(|| self.ch.compare_symbol_identity_tiebreaker(left, right))
    }

    fn compare_hover_declarations(
        &self,
        left: Option<ast::Node>,
        right: Option<ast::Node>,
    ) -> std::cmp::Ordering {
        if left == right {
            return std::cmp::Ordering::Equal;
        }
        let (Some(left), Some(right)) = (left, right) else {
            return if left.is_some() {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        };
        let left_store = self.ch.store_for_node(left);
        let right_store = self.ch.store_for_node(right);
        let left_file = ast::get_source_file_of_node(left_store, Some(left));
        let right_file = ast::get_source_file_of_node(right_store, Some(right));
        let left_file_name = left_file
            .map(|file| left_store.source_file_view(file).file_name())
            .unwrap_or_default();
        let right_file_name = right_file
            .map(|file| right_store.source_file_view(file).file_name())
            .unwrap_or_default();
        left_file_name
            .cmp(&right_file_name)
            .then_with(|| {
                left_store
                    .loc(left)
                    .pos()
                    .cmp(&right_store.loc(right).pos())
            })
            .then_with(|| {
                left_store
                    .loc(left)
                    .end()
                    .cmp(&right_store.loc(right).end())
            })
            .then_with(|| {
                ast::get_node_id(left_store, left).cmp(&ast::get_node_id(right_store, right))
            })
    }

    // filterInheritedProperties removes properties already present in base types.
    fn filter_inherited_property_identities(
        &mut self,
        t: TypeHandle,
        base_types: Vec<TypeHandle>,
        properties: Vec<SymbolIdentity>,
    ) -> Vec<SymbolIdentity> {
        if base_types.is_empty() {
            return properties;
        }
        // Build a lookup from property name to symbol for parent-identity comparison.
        let mut props_by_name = std::collections::HashMap::with_capacity(properties.len());
        for &p in &properties {
            let name = self.ch.symbol_identity_name(p).to_string();
            props_by_name.insert(name, p);
        }
        // Collect names of properties inherited unchanged from base types.
        let mut inherited = collections::Set::new();
        for base in base_types {
            let target_type = self.ch.get_target_type(t);
            let this_type = self.ch.interface_record(target_type).this_type;
            let base_with_this = self.ch.get_type_with_this_argument(base, this_type, false);
            for prop in self
                .ch
                .resolve_structured_type_members(base_with_this)
                .collect_properties()
            {
                let prop_name = self.ch.symbol_identity_name(prop).to_string();
                if let Some(existing) = props_by_name.get(prop_name.as_str()) {
                    if self.ch.symbol_identity_parent(prop)
                        == self.ch.symbol_identity_parent(*existing)
                    {
                        inherited.add(prop_name);
                    }
                }
            }
        }
        if inherited.len() == 0 {
            return properties;
        }
        properties
            .into_iter()
            .filter(|p| {
                let name = self.ch.symbol_identity_name(*p).to_string();
                !inherited.has(&name)
            })
            .collect()
    }

    fn get_declaration_of_alias_symbol_identity(
        &self,
        symbol: SymbolIdentity,
    ) -> Option<ast::Node> {
        self.ch
            .collect_symbol_identity_declarations(symbol)
            .into_iter()
            .rev()
            .find(|declaration| {
                ast::is_alias_symbol_declaration(self.ch.store_for_node(*declaration), *declaration)
            })
    }

    fn is_namespace_member(&self, p: SymbolIdentity) -> bool {
        let flags = self.ch.symbol_identity_flags(p);
        flags & (ast::SYMBOL_FLAGS_TYPE | ast::SYMBOL_FLAGS_NAMESPACE | ast::SYMBOL_FLAGS_ALIAS)
            != 0
            || !(flags & ast::SYMBOL_FLAGS_PROTOTYPE != 0
                || self.ch.missing_name_symbol_identity_name(p) == "prototype"
                || self
                    .ch
                    .missing_name_symbol_identity_value_declaration(p)
                    .as_ref()
                    .is_some_and(|value_declaration| {
                        let store = self.ch.store_for_node(*value_declaration);
                        ast::has_static_modifier(store, *value_declaration)
                            && store
                                .parent(*value_declaration)
                                .as_ref()
                                .is_some_and(|parent| ast::is_class_like(store, *parent))
                    }))
    }
}

// typeElementsToClassElements converts TypeElement nodes (PropertySignature, MethodSignature)
// to their ClassElement equivalents (PropertyDeclaration, MethodDeclaration) so they can be
// used as members of a ClassDeclaration. Nodes that are already ClassElements pass through unchanged.
fn type_elements_to_class_elements<'a>(
    f: &mut ast::NodeFactory,
    mut members: Vec<ast::Node>,
) -> Vec<ast::Node> {
    for i in 0..members.len() {
        match f.store().kind(members[i]) {
            ast::KIND_PROPERTY_SIGNATURE => {
                let (modifiers, name, postfix_token, type_node) = {
                    let store = f.store();
                    (
                        store
                            .source_modifiers(members[i])
                            .map(hover_snapshot_source_modifier_list),
                        store.name(members[i]).unwrap(),
                        store.postfix_token(members[i]),
                        store.r#type(members[i]),
                    )
                };
                let modifiers = modifiers
                    .map(|modifiers| hover_new_factory_modifier_list_from_snapshot(f, modifiers));
                members[i] =
                    f.new_property_declaration(modifiers, name, postfix_token, type_node, None);
            }
            ast::KIND_METHOD_SIGNATURE => {
                let (modifiers, name, postfix_token, type_parameters, parameters, type_node) = {
                    let store = f.store();
                    (
                        store
                            .source_modifiers(members[i])
                            .map(hover_snapshot_source_modifier_list),
                        store.name(members[i]).unwrap(),
                        store.postfix_token(members[i]),
                        store
                            .source_type_parameters(members[i])
                            .map(hover_snapshot_source_node_list),
                        store
                            .source_parameters(members[i])
                            .map(hover_snapshot_source_node_list),
                        store.r#type(members[i]),
                    )
                };
                let modifiers = modifiers
                    .map(|modifiers| hover_new_factory_modifier_list_from_snapshot(f, modifiers));
                let type_parameters = type_parameters.map(|type_parameters| {
                    hover_new_factory_node_list_from_snapshot(f, type_parameters)
                });
                let parameters = parameters
                    .map(|parameters| hover_new_factory_node_list_from_snapshot(f, parameters))
                    .unwrap_or_else(|| hover_new_factory_node_list(f, Vec::new()));
                members[i] = f.new_method_declaration(
                    modifiers,
                    None,
                    name,
                    postfix_token,
                    type_parameters,
                    parameters,
                    type_node,
                    None,
                    None,
                );
            }
            _ => {}
        }
    }
    members
}

struct HoverSourceNodeListSnapshot {
    loc: core::TextRange,
    range: core::TextRange,
    nodes: Vec<ast::Node>,
}

struct HoverSourceModifierListSnapshot {
    loc: core::TextRange,
    range: core::TextRange,
    nodes: Vec<ast::Node>,
    modifier_flags: ast::ModifierFlags,
}

fn hover_snapshot_source_node_list(list: ast::SourceNodeList<'_>) -> HoverSourceNodeListSnapshot {
    HoverSourceNodeListSnapshot {
        loc: list.loc(),
        range: list.range(),
        nodes: list.nodes(),
    }
}

fn hover_snapshot_source_modifier_list(
    modifiers: ast::SourceModifierList<'_>,
) -> HoverSourceModifierListSnapshot {
    HoverSourceModifierListSnapshot {
        loc: modifiers.loc(),
        range: modifiers.range(),
        nodes: modifiers.nodes().nodes(),
        modifier_flags: modifiers.modifier_flags(),
    }
}

fn is_hash_private_identity(ch: &Checker<'_, '_>, s: SymbolIdentity) -> bool {
    ch.missing_name_symbol_identity_value_declaration(s)
        .as_ref()
        .is_some_and(|value_declaration| {
            ch.store_for_node(*value_declaration)
                .name(*value_declaration)
                .is_some_and(|name| {
                    ast::is_private_identifier(ch.store_for_node(*value_declaration), name)
                })
        })
}

fn is_const_enum_symbol_identity(ch: &Checker<'_, '_>, symbol: SymbolIdentity) -> bool {
    ch.symbol_identity_flags(symbol) & ast::SYMBOL_FLAGS_CONST_ENUM != 0
}

fn hover_new_factory_node_list(
    f: &mut ast::NodeFactory,
    nodes: impl IntoIterator<Item = ast::Node>,
) -> ast::NodeList {
    f.new_node_list(
        core::new_text_range(-1, -1),
        core::new_text_range(-1, -1),
        nodes,
    )
}

fn hover_new_factory_node_list_from_snapshot(
    f: &mut ast::NodeFactory,
    list: HoverSourceNodeListSnapshot,
) -> ast::NodeList {
    f.new_node_list(list.loc, list.range, list.nodes)
}

fn hover_new_factory_modifier_list(
    f: &mut ast::NodeFactory,
    flags: ast::ModifierFlags,
) -> ast::ModifierList {
    let modifiers = hover_create_modifiers_from_modifier_flags(f, flags);
    f.new_modifier_list(
        core::new_text_range(-1, -1),
        core::new_text_range(-1, -1),
        modifiers,
        flags,
    )
}

fn hover_new_factory_modifier_list_from_snapshot(
    f: &mut ast::NodeFactory,
    modifiers: HoverSourceModifierListSnapshot,
) -> ast::ModifierList {
    f.new_modifier_list(
        modifiers.loc,
        modifiers.range,
        modifiers.nodes,
        modifiers.modifier_flags,
    )
}

fn hover_create_modifiers_from_modifier_flags(
    f: &mut ast::NodeFactory,
    flags: ast::ModifierFlags,
) -> Vec<ast::Node> {
    let mut result = Vec::new();
    if (flags & ast::MODIFIER_FLAGS_EXPORT) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::ExportKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_AMBIENT) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::DeclareKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_DEFAULT) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::DefaultKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_CONST) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::ConstKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_PUBLIC) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::PublicKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_PRIVATE) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::PrivateKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_PROTECTED) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::ProtectedKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_ABSTRACT) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::AbstractKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_STATIC) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::StaticKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_OVERRIDE) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::OverrideKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_READONLY) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::ReadonlyKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_ACCESSOR) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::AccessorKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_ASYNC) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::AsyncKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_IN) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::InKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_OUT) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::OutKeyword));
    }
    result
}

fn hover_deep_clone_node(
    f: &mut ast::NodeFactory,
    source: impl AsRef<ast::AstStore>,
    node: ast::Node,
) -> ast::Node {
    f.deep_clone_node_from_store(source, node)
}
