use ts_ast as ast;
use ts_astnav as astnav;
use ts_checker as checker;
use ts_collections as collections;
use ts_compiler as compiler;
use ts_core as core;
use ts_lsproto as lsproto;
use ts_scanner as scanner;

use crate::LanguageService;
use crate::findallreferences::{RefInfo, get_context_node};
use crate::lsconv;
use crate::utilities::{
    create_range_from_node, get_containing_object_literal_element, get_reference_at_position,
    get_target_label, source_node_symbol_from_program, source_node_symbol_parent_from_program,
    to_context_range,
};

pub(crate) fn source_file_for_node<'a>(
    program: &'a compiler::Program,
    extra_source_files: &[&'a ast::SourceFile],
    node: ast::Node,
) -> Option<&'a ast::SourceFile> {
    extra_source_files
        .iter()
        .copied()
        .find(|file| file.store().store_id() == node.store_id())
        .or_else(|| {
            program
                .get_parsed_source_files_refs()
                .into_iter()
                .find(|file| file.store().store_id() == node.store_id())
        })
}

pub(crate) fn store_for_node<'a>(
    program: &'a compiler::Program,
    extra_source_files: &[&'a ast::SourceFile],
    node: ast::Node,
) -> Option<&'a ast::AstStore> {
    source_file_for_node(program, extra_source_files, node).map(|file| file.store())
}

impl LanguageService<'_> {
    pub fn provide_definition(
        &self,
        ctx: &core::Context,
        document_uri: lsproto::DocumentUri,
        position: lsproto::Position,
    ) -> Result<lsproto::DefinitionResponse, core::Error> {
        if self.user_preferences().prefer_go_to_source_definition {
            return self.provide_source_definition(ctx, document_uri, position);
        }
        self.provide_definition_worker(ctx, document_uri, position)
    }

    pub fn provide_definition_worker(
        &self,
        ctx: &core::Context,
        document_uri: lsproto::DocumentUri,
        position: lsproto::Position,
    ) -> Result<lsproto::DefinitionResponse, core::Error> {
        let caps = lsproto::get_client_capabilities(ctx);
        let client_supports_link = caps.text_document.definition.link_support;

        let (program, file) = self.get_program_and_file(document_uri);
        let pos = self
            .converters
            .line_and_character_to_position(file, position) as i32;
        let Some(node) = astnav::get_touching_property_name(file, pos) else {
            return Ok(lsproto::LocationOrLocationsOrDefinitionLinksOrNull::default());
        };
        let reference = get_reference_at_position(file, pos, program);

        if file.store().kind(node) == ast::Kind::SourceFile {
            return Ok(lsproto::LocationOrLocationsOrDefinitionLinksOrNull::default());
        }

        let origin_selection_range = self.create_lsp_range_from_node(node, file);
        if reference
            .as_ref()
            .is_some_and(|reference| reference.file.is_some())
        {
            return Ok(self.create_definition_locations(
                origin_selection_range,
                client_supports_link,
                Vec::new(),
                reference.as_ref(),
                &[],
            ));
        }

        program.with_type_checker_for_file_using(compiler::CheckerAccess::context(ctx), file, |c| {
            if file.store().kind(node) == ast::Kind::OverrideKeyword {
                if let Some(sym) = get_symbol_for_overridden_member(c, file.store(), node) {
                    let result = self.create_definition_locations_from_nodes(
                        origin_selection_range,
                        client_supports_link,
                        declarations_from_symbol(c, sym),
                        None, /*reference*/
                        &[],
                    );
                    return Ok(result);
                }
            }

            if ast::is_jump_statement_target(file.store(), &node) {
                if let Some(parent) = file.store().parent(node)
                    && let Some(label) =
                        get_target_label(file.store(), parent, &file.store().text(node))
                {
                    let result = self.create_definition_locations_from_nodes(
                        origin_selection_range,
                        client_supports_link,
                        vec![label],
                        None, /*reference*/
                        &[],
                    );
                    return Ok(result);
                }
            }

            if file.store().kind(node) == ast::Kind::CaseKeyword
                || file.store().kind(node) == ast::Kind::DefaultKeyword
                    && file
                        .store()
                        .parent(node)
                        .as_ref()
                        .is_some_and(|parent| ast::is_default_clause(file.store(), *parent))
            {
                if let Some(stmt) =
                    ast::find_ancestor(file.store(), file.store().parent(node), |store, node| {
                        ast::is_switch_statement(store, node)
                    })
                {
                    let result = self.create_location_from_file_and_range(
                        file,
                        scanner::get_range_of_token_at_position(
                            file,
                            file.store().loc(stmt).pos() as usize,
                        ),
                    );
                    return Ok(result);
                }
            }

            if file.store().kind(node) == ast::Kind::ReturnKeyword
                || file.store().kind(node) == ast::Kind::YieldKeyword
                || file.store().kind(node) == ast::Kind::AwaitKeyword
            {
                if let Some(fun) = ast::find_ancestor(file.store(), Some(node), |store, node| {
                    ast::is_function_like_declaration(store, Some(node))
                }) {
                    let result = self.create_definition_locations_from_nodes(
                        origin_selection_range,
                        client_supports_link,
                        vec![fun],
                        None, /*reference*/
                        &[],
                    );
                    return Ok(result);
                }
            }

            let mut declarations = get_declarations_from_location(c, program, node);
            let called_declaration = try_get_signature_declaration(c, program, node);
            if let Some(called_declaration) = called_declaration {
                if !(file
                    .store()
                    .parent(node)
                    .as_ref()
                    .is_some_and(|parent| ast::is_jsx_opening_like_element(file.store(), *parent))
                    && store_for_node(program, &[], called_declaration)
                        .is_some_and(|store| is_jsx_constructor_like(store, called_declaration)))
                {
                    let declaration_name = get_declaration_name_for_keyword(file.store(), node);
                    let symbol = c.get_symbol_at_location_public(declaration_name);
                    if let Some(symbol) = symbol {
                        if c.get_root_symbols_public(symbol).iter().any(|root_symbol| {
                            symbol_matches_signature(
                                program,
                                Some(*root_symbol),
                                Some(called_declaration),
                            )
                        }) {
                            if !store_for_node(program, &[], called_declaration).is_some_and(
                                |store| ast::is_constructor_declaration(store, called_declaration),
                            ) {
                                declarations.clear();
                            } else {
                                declarations.retain(|decl| {
                                    *decl != called_declaration
                                        && store_for_node(program, &[], *decl).is_some_and(
                                            |store| {
                                                ast::is_class_declaration(store, *decl)
                                                    || ast::is_class_expression(store, *decl)
                                            },
                                        )
                                });
                            }
                        } else {
                            declarations.retain(|decl| *decl != called_declaration);
                        }
                    } else {
                        declarations.retain(|decl| *decl != called_declaration);
                    }
                    declarations.push(called_declaration);
                }
            }
            let result = self.create_definition_locations_from_nodes(
                origin_selection_range,
                client_supports_link,
                declarations,
                reference.as_ref(),
                &[],
            );
            Ok(result)
        })
    }

    pub fn provide_type_definition(
        &self,
        ctx: &core::Context,
        document_uri: lsproto::DocumentUri,
        position: lsproto::Position,
    ) -> Result<lsproto::TypeDefinitionResponse, core::Error> {
        let caps = lsproto::get_client_capabilities(ctx);
        let client_supports_link = caps.text_document.type_definition.link_support;

        let (program, file) = self.get_program_and_file(document_uri);
        let Some(mut node) = astnav::get_touching_property_name(
            file,
            self.converters
                .line_and_character_to_position(file, position) as i32,
        ) else {
            return Ok(lsproto::LocationOrLocationsOrDefinitionLinksOrNull::default());
        };
        if file.store().kind(node) == ast::Kind::SourceFile {
            return Ok(lsproto::LocationOrLocationsOrDefinitionLinksOrNull::default());
        }
        let origin_selection_range = self.create_lsp_range_from_node(node, file);

        program.with_type_checker_for_file_using(compiler::CheckerAccess::context(ctx), file, |c| {
            node = get_declaration_name_for_keyword(file.store(), node);

            if let Some(symbol) = c.get_symbol_at_location_public(node) {
                let symbol_type = get_type_of_symbol_at_location(c, program, symbol.clone(), node);
                let mut declarations = get_declarations_from_type(c, symbol_type);
                if let Some(type_argument) = c.get_first_type_argument_from_known_type(symbol_type)
                {
                    let mut type_argument_declarations =
                        get_declarations_from_type(c, type_argument);
                    type_argument_declarations.extend(declarations);
                    declarations = type_argument_declarations;
                }
                if !declarations.is_empty() {
                    let result = self.create_definition_locations_from_nodes(
                        origin_selection_range,
                        client_supports_link,
                        declarations,
                        None, /*reference*/
                        &[],
                    );
                    return Ok(result);
                }
                let Some(symbol_flags) = c.symbol_flags_public(symbol) else {
                    return Ok(lsproto::LocationOrLocationsOrDefinitionLinksOrNull::default());
                };
                if symbol_flags & ast::SYMBOL_FLAGS_VALUE == ast::SYMBOL_FLAGS_NONE
                    && symbol_flags & ast::SYMBOL_FLAGS_TYPE != ast::SYMBOL_FLAGS_NONE
                {
                    let result = self.create_definition_locations_from_nodes(
                        origin_selection_range,
                        client_supports_link,
                        c.collect_symbol_declarations_public(symbol),
                        None, /*reference*/
                        &[],
                    );
                    return Ok(result);
                }
            }
            Ok(lsproto::LocationOrLocationsOrDefinitionLinksOrNull::default())
        })
    }

    pub(crate) fn create_definition_locations(
        &self,
        origin_selection_range: lsproto::Range,
        client_supports_link: bool,
        declarations: Vec<ast::Node>,
        reference: Option<&RefInfo<'_>>,
        extra_source_files: &[&ast::SourceFile],
    ) -> lsproto::DefinitionResponse {
        self.create_definition_locations_from_nodes(
            origin_selection_range,
            client_supports_link,
            declarations,
            reference,
            extra_source_files,
        )
    }

    pub(crate) fn create_definition_locations_from_nodes(
        &self,
        origin_selection_range: lsproto::Range,
        client_supports_link: bool,
        declarations: Vec<ast::Node>,
        reference: Option<&RefInfo<'_>>,
        extra_source_files: &[&ast::SourceFile],
    ) -> lsproto::DefinitionResponse {
        let mut locations = Vec::new();
        let mut location_ranges = collections::Set::new();

        if let Some(reference) = reference {
            let target_range = lsproto::Range {
                start: lsproto::Position {
                    line: 0,
                    character: 0,
                },
                end: lsproto::Position {
                    line: 0,
                    character: 0,
                },
            };
            locations.push(lsproto::LocationLink {
                origin_selection_range: Some(origin_selection_range.clone()),
                target_uri: lsconv::file_name_to_document_uri(&reference.file_name),
                target_range: target_range.clone(),
                target_selection_range: target_range,
            });
        }

        let program = self.get_program();
        for decl in declarations {
            let Some(file) = source_file_for_node(program, extra_source_files, decl) else {
                continue;
            };
            let file_name = file.file_name();
            let name = ast::get_name_of_declaration(file.store(), Some(decl)).unwrap_or(decl);
            let name_range = if file.store().kind(name) == ast::Kind::EmptyStatement {
                let pos = file.store().loc(name).pos();
                core::new_text_range(pos, pos)
            } else {
                create_range_from_node(name, &file)
            };
            if location_ranges.add_if_absent(FileRange {
                file_name: file_name.to_string(),
                file_range: name_range,
            }) {
                let context_node_storage = get_context_node(Some(decl));
                let context_node = context_node_storage.unwrap_or(decl);
                let context_range = to_context_range(Some(name_range), &file, Some(context_node))
                    .unwrap_or(name_range);
                let target_selection_loc = self.get_mapped_location(&file_name, name_range);
                let target_loc = self.get_mapped_location(&file_name, context_range);
                locations.push(lsproto::LocationLink {
                    origin_selection_range: Some(origin_selection_range.clone()),
                    target_selection_range: target_selection_loc.range,
                    target_uri: target_loc.uri,
                    target_range: target_loc.range,
                });
            }
        }

        if client_supports_link {
            return lsproto::LocationOrLocationsOrDefinitionLinksOrNull {
                definition_links: Some(locations.into_iter().map(Some).collect()),
                ..Default::default()
            };
        }
        create_locations_from_links(locations)
    }

    pub fn create_location_from_file_and_range(
        &self,
        file: &ast::SourceFile,
        text_range: core::TextRange,
    ) -> lsproto::DefinitionResponse {
        let mapped_location = self.get_mapped_location(&file.file_name(), text_range);
        lsproto::LocationOrLocationsOrDefinitionLinksOrNull {
            location: Some(mapped_location),
            ..Default::default()
        }
    }
}

pub(crate) fn get_declaration_name_for_keyword(
    store: &ast::AstStore,
    node: ast::Node,
) -> ast::Node {
    let kind = store.kind(node);
    if kind >= ast::Kind::FirstKeyword && kind <= ast::Kind::LastKeyword {
        if let Some(parent) = store.parent(node) {
            if ast::is_variable_declaration_list(store, parent) {
                if let Some(decl) = store
                    .declarations(parent)
                    .and_then(|declarations| declarations.iter().next())
                    && let Some(name) = store.name(decl)
                {
                    return name;
                }
            } else if store.has_declaration_base(parent) && store.name(parent).is_some() {
                let name = store.name(parent).unwrap();
                if store.loc(node).pos() < store.loc(name).pos() {
                    return name;
                }
            }
        }
    }
    node
}

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct FileRange {
    pub file_name: String,
    pub file_range: core::TextRange,
}

pub fn create_locations_from_links(
    links: Vec<lsproto::LocationLink>,
) -> lsproto::DefinitionResponse {
    let locations = links
        .into_iter()
        .map(|link| lsproto::Location {
            uri: link.target_uri,
            range: link.target_selection_range,
        })
        .collect();
    lsproto::LocationOrLocationsOrDefinitionLinksOrNull {
        locations: Some(locations),
        ..Default::default()
    }
}

fn declarations_from_symbol<'a>(
    checker: &mut checker::Checker<'a, '_>,
    symbol: ast::SymbolIdentity,
) -> Vec<ast::Node> {
    checker.collect_symbol_declarations_public(symbol)
}

pub fn get_declarations_from_location<'a>(
    c: &mut checker::Checker<'a, '_>,
    program: &'a compiler::Program,
    node: ast::Node,
) -> Vec<ast::Node> {
    let Some(store) = store_for_node(program, &[], node) else {
        return Vec::new();
    };
    if ast::is_identifier(store, node)
        && store
            .parent(node)
            .as_ref()
            .is_some_and(|parent| ast::is_shorthand_property_assignment(store, *parent))
    {
        // Because name in short-hand property assignment has two different meanings: property name and property value,
        // using go-to-definition at such position should go to the variable declaration of the property value rather than
        // go to the declaration of the property name (in this case stay at the same position). However, if go-to-definition
        // is performed at the location of property access, we would like to go to definition of the property in the short-hand
        // assignment. This case and others are handled by the following code.
        // and the contextual type's property declarations
        let shorthand_symbol = c.get_resolved_symbol_public(node);
        let mut declarations: Vec<ast::Node> = Vec::new();
        if let Some(shorthand_symbol) = shorthand_symbol {
            declarations.extend(declarations_from_symbol(c, shorthand_symbol));
        }
        let contextual_declarations =
            get_declarations_from_object_literal_element(c, program, node);
        declarations.extend(contextual_declarations);
        return declarations;
    }

    let parent = store.parent(node);
    let parent_parent = parent.as_ref().and_then(|parent| store.parent(*parent));
    if ast::is_property_name(store, &node)
        && parent
            .as_ref()
            .is_some_and(|parent| ast::is_binding_element(store, *parent))
        && parent_parent
            .as_ref()
            .is_some_and(|parent| ast::is_object_binding_pattern(store, *parent))
    {
        // If the node is the name of a BindingElement within an ObjectBindingPattern instead of just returning the
        // declaration of the symbol (which is itself), we should try to get to the original type of the
        // ObjectBindingPattern and return the property declaration for the referenced property.
        // For example:
        //      import('./foo').then(({ bar }) => undefined); => should navigate to the declaration in file "./foo"
        //
        //      function bar<T>(onfulfilled: (value: T) => void) { }
        //      interface Test { prop1: number }
        //      bar<Test>(({ prop1 }) => {});  => should navigate to prop1 in Test
        let parent = parent.unwrap();
        let binding_name = store.name(parent);
        let property_name = store.property_name(parent);
        let Some(property_name) = property_name.as_ref().or(binding_name.as_ref()) else {
            return Vec::new();
        };
        if store.dot_dot_dot_token(parent).is_none() && node == *property_name {
            let (name, ok) = ast::try_get_text_of_property_name(store, node);
            if ok {
                let parent_parent = parent_parent.unwrap();
                let t = c.get_type_at_location(parent_parent);
                let types = if c.is_union_type_public(t) {
                    c.type_types_public(t)
                } else {
                    vec![t]
                };
                let mut result = Vec::new();
                for union_type in types {
                    if let Some(prop) = c.get_property_of_type_public(union_type, &name) {
                        result.extend(declarations_from_symbol(c, prop));
                    }
                }
                return result;
            }
        }
    }

    let declaration_name = get_declaration_name_for_keyword(store, node);
    if let Some(mut symbol) = c.get_symbol_at_location_public(declaration_name) {
        let Some(symbol_flags) = c.symbol_flags_public(symbol) else {
            return Vec::new();
        };
        if symbol_flags & ast::SYMBOL_FLAGS_CLASS != ast::SYMBOL_FLAGS_NONE
            && symbol_flags & (ast::SYMBOL_FLAGS_FUNCTION | ast::SYMBOL_FLAGS_VARIABLE)
                == ast::SYMBOL_FLAGS_NONE
            && store.kind(declaration_name) == ast::Kind::ConstructorKeyword
        {
            let constructor = c.symbol_member_public(symbol, ast::INTERNAL_SYMBOL_NAME_CONSTRUCTOR);
            if let Some(constructor) = constructor {
                symbol = constructor;
            }
        }
        if c.symbol_flags_public(symbol)
            .is_some_and(|flags| flags & ast::SYMBOL_FLAGS_ALIAS != ast::SYMBOL_FLAGS_NONE)
        {
            if let Some(resolved) = c.skip_alias_public(symbol) {
                symbol = resolved;
            }
        }
        let object_literal_element_declarations =
            get_declarations_from_object_literal_element(c, program, node);
        if !object_literal_element_declarations.is_empty() {
            return object_literal_element_declarations;
        }
        let declarations = declarations_from_symbol(c, symbol);
        if !declarations.is_empty() {
            return declarations;
        }
    }
    let index_infos = c.get_index_signatures_at_location_public(node);
    if !index_infos.is_empty() {
        return index_infos;
    }
    Vec::new()
}

// getDeclarationsFromObjectLiteralElement returns declarations from the contextual type
// of an object literal element, if available.
pub fn get_declarations_from_object_literal_element<'a>(
    c: &mut checker::Checker<'a, '_>,
    program: &'a compiler::Program,
    node: ast::Node,
) -> Vec<ast::Node> {
    let Some(store) = store_for_node(program, &[], node) else {
        return Vec::new();
    };
    let Some(element) = get_containing_object_literal_element(store, node) else {
        return Vec::new();
    };
    let Some(parent) = store.parent(element) else {
        return Vec::new();
    };

    let contextual_type = c.get_contextual_type_public(parent, checker::CONTEXT_FLAGS_NONE);
    let Some(contextual_type) = contextual_type else {
        return Vec::new();
    };

    let mut properties =
        c.get_property_symbols_from_contextual_type(element, contextual_type, false);
    if properties.iter().any(|p| {
        c.symbol_value_declaration_public(*p)
            .as_ref()
            .is_some_and(|value_declaration| {
                let Some(value_store) = store_for_node(program, &[], *value_declaration) else {
                    return false;
                };
                value_store
                    .parent(*value_declaration)
                    .as_ref()
                    .is_some_and(|parent| ast::is_object_literal_expression(value_store, *parent))
                    && ast::is_object_literal_element(value_store, value_declaration)
                    && value_store
                        .name(*value_declaration)
                        .is_some_and(|name| name == node)
            })
    }) {
        if let Some(without_node_inferences_type) =
            c.get_contextual_type_public(parent, checker::CONTEXT_FLAGS_IGNORE_NODE_INFERENCES)
        {
            let without_node_inferences_properties = c.get_property_symbols_from_contextual_type(
                element,
                without_node_inferences_type,
                false,
            );
            if !without_node_inferences_properties.is_empty() {
                properties = without_node_inferences_properties;
            }
        }
    }

    let mut result = Vec::new();
    for prop in properties {
        result.extend(declarations_from_symbol(c, prop));
    }
    result
}

// Returns a CallLikeExpression where `node` is the target being invoked.
pub fn get_ancestor_call_like_expression(
    program: &compiler::Program,
    node: ast::Node,
) -> Option<ast::Node> {
    let store = store_for_node(program, &[], node)?;
    let target = ast::find_ancestor(store, Some(node), |store, n| {
        !ast::is_right_side_of_property_access(store, n)
    })?;
    let call_like = store.parent(target)?;
    let invoked_expression = ast::get_invoked_expression(store, &call_like)?;
    if ast::is_call_like_expression(store, &call_like) && invoked_expression == target {
        return Some(call_like);
    }
    None
}

pub fn try_get_signature_declaration<'a>(
    type_checker: &mut checker::Checker<'a, '_>,
    program: &'a compiler::Program,
    node: ast::Node,
) -> Option<ast::Node> {
    let call_like = get_ancestor_call_like_expression(program, node);
    let signature =
        call_like.and_then(|call_like| type_checker.get_resolved_signature_public(call_like));
    // Don't go to a function type, go to the value having that type.
    if let Some(signature) = signature {
        let declaration = type_checker.signature_declaration_public(signature);
        if let Some(declaration) = declaration
            && store_for_node(program, &[], declaration).is_some_and(|store| {
                ast::is_function_like(store, Some(declaration))
                    && !ast::is_function_type_node(store, declaration)
            })
        {
            return Some(declaration);
        }
    }
    None
}

pub(crate) fn is_jsx_constructor_like(store: &ast::AstStore, node: ast::Node) -> bool {
    match () {
        _ if ast::is_constructor_declaration(store, node)
            || ast::is_constructor_type_node(store, node)
            || ast::is_call_signature_declaration(store, node)
            || ast::is_construct_signature_declaration(store, node) =>
        {
            true
        }
        _ => false,
    }
}

pub(crate) fn symbol_matches_signature(
    program: &compiler::Program,
    symbol: Option<ast::SymbolIdentity>,
    called_declaration: Option<ast::Node>,
) -> bool {
    let (Some(symbol), Some(called_declaration)) = (symbol, called_declaration) else {
        return false;
    };
    let Some(store) = store_for_node(program, &[], called_declaration) else {
        return false;
    };
    let Some(source_file) = source_file_for_node(program, &[], called_declaration) else {
        return false;
    };
    let called_symbol = source_node_symbol_from_program(program, source_file, called_declaration);
    if called_symbol.is_some_and(|called_symbol| symbol == called_symbol)
        || source_node_symbol_parent_from_program(program, source_file, called_declaration)
            .is_some_and(|parent| symbol == parent)
    {
        return true;
    }
    let parent = store.parent(called_declaration);
    parent.as_ref().is_some_and(|parent| {
        ast::is_assignment_expression(store, *parent, false /*excludeCompoundAssignment*/)
            || !ast::is_call_like_expression(store, parent)
                && ast::can_have_symbol(store, parent)
                && source_node_symbol_from_program(program, source_file, *parent)
                    .is_some_and(|parent_symbol| symbol == parent_symbol)
    })
}

pub(crate) fn get_symbol_for_overridden_member<'a>(
    type_checker: &mut checker::Checker<'a, '_>,
    store: &'a ast::AstStore,
    node: ast::Node,
) -> Option<ast::SymbolIdentity> {
    let class_element = ast::find_ancestor(store, Some(node), |store, node| {
        ast::is_class_element(store, node)
    })?;
    store.name(class_element)?;
    let base_declaration = ast::find_ancestor(store, Some(class_element), |store, node| {
        ast::is_class_like(store, node)
    })?;
    let base_type_node = ast::get_class_extends_heritage_element(store, &base_declaration)?;
    let expression = store.expression(base_type_node)?;
    let expression = ast::skip_parentheses(store, expression);
    let base = if ast::is_class_expression(store, expression) {
        type_checker.source_node_symbol_public(expression)
    } else {
        type_checker.get_symbol_at_location_public(expression)
    };
    let base = base?;
    let name_node = store.name(class_element)?;
    let name = ast::get_text_of_property_name(store, &name_node);
    if ast::has_static_modifier(store, class_element) {
        let symbol_type = type_checker.get_type_of_symbol_identity_public(base)?;
        return type_checker.get_property_of_type_public(symbol_type, &name);
    }
    let declared_type = type_checker.get_declared_type_of_symbol_identity_public(base)?;
    type_checker.get_property_of_type_public(declared_type, &name)
}

pub(crate) fn get_type_of_symbol_at_location<'a>(
    c: &mut checker::Checker<'a, '_>,
    program: &'a compiler::Program,
    symbol: ast::SymbolIdentity,
    node: ast::Node,
) -> checker::TypeHandle {
    let t = c
        .get_type_of_symbol_identity_at_location_public(symbol, Some(node))
        .unwrap_or_else(|| c.get_error_type());
    // If the type is just a function's inferred type, go-to-type should go to the return type instead since
    // go-to-definition takes you to the function anyway.
    let t_symbol = c.type_symbol_public(t);
    let value_declaration = c.symbol_value_declaration_public(symbol);
    if t_symbol
        .as_ref()
        .is_some_and(|t_symbol| *t_symbol == symbol)
        || t_symbol.is_some()
            && value_declaration.as_ref().is_some_and(|declaration| {
                store_for_node(program, &[], *declaration)
                    .is_some_and(|store| ast::is_variable_declaration(store, *declaration))
            })
            && value_declaration
                .as_ref()
                .and_then(|value_declaration| {
                    store_for_node(program, &[], *value_declaration)
                        .and_then(|store| store.initializer(*value_declaration))
                })
                .is_some_and(|initializer| {
                    t_symbol
                        .and_then(|t_symbol| c.symbol_value_declaration_public(t_symbol))
                        .is_some_and(|decl| initializer == decl)
                })
    {
        let sigs = c.get_call_signatures(t);
        if sigs.len() == 1 {
            return c.get_return_type_of_signature_public(sigs[0]);
        }
    }
    t
}

pub(crate) fn get_declarations_from_type<'a>(
    c: &mut checker::Checker<'a, '_>,
    t: checker::TypeHandle,
) -> Vec<ast::Node> {
    let mut result: Vec<ast::Node> = Vec::new();
    for t in c.distributed_types_public(t) {
        if let Some(symbol) = c.type_symbol_public(t) {
            let declarations = declarations_from_symbol(c, symbol);
            for decl in declarations {
                if !result.iter().any(|existing| *existing == decl) {
                    result.push(decl);
                }
            }
        }
    }
    result
}
