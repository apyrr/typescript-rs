use ts_ast as ast;
use ts_astnav as astnav;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as core;
use ts_lsproto as lsproto;
use ts_module as module;
use ts_modulespecifiers as modulespecifiers;
use ts_modulespecifiers::ModuleSpecifierGenerationHost as _;
use ts_packagejson as packagejson;
use ts_printer as printer;
use ts_stringutil as stringutil;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use crate::LanguageService;
use crate::completions::{
    ArgumentInfoForCompletions, SortTextLocationPriority, create_completion_details,
    get_argument_info_for_completions, get_constraint_of_type_argument_property,
    get_default_commit_characters, get_properties_for_completion,
    get_properties_for_object_expression,
};
use crate::format::is_in_comment;
use crate::lsutil;
use crate::utilities::{get_contextual_type_from_parent, skip_constraint};

#[derive(Clone, Default)]
pub(crate) struct CompletionsFromTypes {
    types: Vec<checker::TypeHandle>,
    is_new_identifier: bool,
}

#[derive(Clone, Default)]
pub(crate) struct CompletionsFromProperties {
    symbols: Vec<ast::SymbolIdentity>,
    has_index_signature: bool,
}

#[derive(Clone, Default)]
pub(crate) struct PathCompletion {
    name: String,
    kind: lsutil::ScriptElementKind,
    extension: String,
    text_range: Option<core::TextRange>,
}

#[derive(Clone, Default)]
pub(crate) struct StringLiteralCompletions {
    from_types: Option<CompletionsFromTypes>,
    from_properties: Option<CompletionsFromProperties>,
    from_paths: Option<Vec<PathCompletion>>,
}

impl LanguageService<'_> {
    pub fn get_string_literal_completions<'a>(
        &self,
        ctx: &core::Context,
        file: &'a ast::SourceFile,
        position: i32,
        context_token: ast::Node,
        checker: &mut checker::Checker<'a, '_>,
        compiler_options: &core::CompilerOptions,
    ) -> Option<lsproto::CompletionList> {
        if is_in_reference_comment(file, position) {
            let entries = self.get_triple_slash_reference_completions(
                file,
                position,
                self.get_program(),
                checker,
            );
            return Some(self.convert_path_completions(ctx, &entries, file, position));
        }
        if crate::completions::is_in_string_or_regular_expression_or_template_literal(
            file.store(),
            &context_token,
            position,
        ) {
            if !ast::is_string_literal_like(file.store(), context_token) {
                return None;
            }
            let entries = self.get_string_literal_completion_entries(
                ctx,
                file,
                &context_token,
                position,
                checker,
            );
            return self.convert_string_literal_completions(
                ctx,
                entries.as_ref(),
                &context_token,
                file,
                position,
                checker,
                compiler_options,
            );
        }
        None
    }

    pub(crate) fn convert_string_literal_completions<'a>(
        &self,
        ctx: &core::Context,
        completion: Option<&StringLiteralCompletions>,
        context_token: &ast::StringLiteralLike,
        file: &'a ast::SourceFile,
        position: i32,
        type_checker: &mut checker::Checker<'a, '_>,
        options: &core::CompilerOptions,
    ) -> Option<lsproto::CompletionList> {
        let completion = completion?;
        let optional_replacement_range =
            self.create_range_from_string_literal_like_content(file, context_token, position);

        if let Some(path_completions) = &completion.from_paths {
            return Some(self.convert_path_completions(ctx, path_completions, file, position));
        }

        if let Some(properties) = &completion.from_properties {
            let data = crate::completions::CompletionDataData {
                symbols: properties.symbols.clone(),
                completion_kind: crate::completions::COMPLETION_KIND_STRING,
                is_new_identifier_location: properties.has_index_signature,
                location: Some(file.as_node()),
                context_token: Some(*context_token),
                ..Default::default()
            };
            let (_, items) = self.get_completion_entries_from_symbols(
                ctx,
                type_checker,
                &data,
                Some(context_token),
                position,
                file,
                options,
            );
            let mut items = items;
            let default_commit_characters =
                get_default_commit_characters(properties.has_index_signature);
            let item_defaults = self.set_item_defaults(
                ctx,
                position,
                file,
                &mut items,
                Some(&default_commit_characters),
                optional_replacement_range,
            );
            return Some(lsproto::CompletionList {
                is_incomplete: false,
                item_defaults,
                items,
                ..Default::default()
            });
        }

        if let Some(types) = &completion.from_types {
            let store = file.store();
            let quote_char =
                if store.kind(*context_token) == ast::Kind::NoSubstitutionTemplateLiteral {
                    printer::QuoteChar::Backtick
                } else if store.text(*context_token).starts_with('\'') {
                    printer::QuoteChar::SingleQuote
                } else {
                    printer::QuoteChar::DoubleQuote
                };
            let items = types
                .types
                .iter()
                .map(|t| {
                    let name = printer::escape_string(
                        type_checker.get_string_literal_value_public(*t),
                        quote_char,
                    );
                    self.create_lsp_completion_item(
                        ctx,
                        name,
                        String::new(),
                        String::new(),
                        SortTextLocationPriority.to_string(),
                        lsutil::ScriptElementKind::String,
                        lsutil::SCRIPT_ELEMENT_KIND_MODIFIER_NONE,
                        self.get_replacement_range_for_context_token(
                            file,
                            Some(context_token),
                            position,
                        ),
                        None,
                        None,
                        file,
                        position,
                        false,
                        false,
                        false,
                        false,
                        String::new(),
                        None,
                        None,
                    )
                })
                .collect::<Vec<_>>();
            let mut items = items;
            let default_commit_characters = get_default_commit_characters(types.is_new_identifier);
            let item_defaults = self.set_item_defaults(
                ctx,
                position,
                file,
                &mut items,
                Some(&default_commit_characters),
                None,
            );
            return Some(lsproto::CompletionList {
                is_incomplete: false,
                item_defaults,
                items,
                ..Default::default()
            });
        }

        None
    }

    pub(crate) fn convert_path_completions(
        &self,
        ctx: &core::Context,
        path_completions: &[PathCompletion],
        file: &ast::SourceFile,
        position: i32,
    ) -> lsproto::CompletionList {
        let is_new_identifier_location = true;
        let default_commit_characters = get_default_commit_characters(is_new_identifier_location);
        let mut items = path_completions
            .iter()
            .map(|path_completion| {
                let replacement_span = path_completion.text_range.map(|text_range| {
                    self.create_lsp_range_from_bounds(text_range.pos(), text_range.end(), file)
                });
                let mut detail = path_completion.name.clone();
                if !path_completion.name.ends_with(&path_completion.extension) {
                    detail.push_str(&path_completion.extension);
                }
                self.create_lsp_completion_item(
                    ctx,
                    path_completion.name.clone(),
                    String::new(),
                    String::new(),
                    SortTextLocationPriority.to_string(),
                    path_completion.kind,
                    kind_modifiers_from_extension(&path_completion.extension),
                    replacement_span,
                    None,
                    None,
                    file,
                    position,
                    false,
                    false,
                    false,
                    false,
                    String::new(),
                    None,
                    Some(detail),
                )
            })
            .collect::<Vec<_>>();
        let item_defaults = self.set_item_defaults(
            ctx,
            position,
            file,
            &mut items,
            Some(&default_commit_characters),
            None,
        );
        lsproto::CompletionList {
            is_incomplete: false,
            item_defaults,
            items,
            ..Default::default()
        }
    }

    pub(crate) fn get_string_literal_completion_entries<'a>(
        &self,
        _ctx: &core::Context,
        file: &'a ast::SourceFile,
        node: &ast::StringLiteralLike,
        position: i32,
        type_checker: &mut checker::Checker<'a, '_>,
    ) -> Option<StringLiteralCompletions> {
        let store = file.store();
        let node_parent = store.parent(*node)?;
        let parent_storage = walk_up_parentheses(store, Some(node_parent));
        let parent = &parent_storage;
        match store.kind(*parent) {
            ast::Kind::LiteralType => {
                let parent_parent = store.parent(*parent)?;
                let grandparent_storage = walk_up_parentheses(store, Some(parent_parent));
                let grandparent = &grandparent_storage;
                if store.kind(*grandparent) == ast::Kind::ImportType {
                    return self.get_string_literal_completions_from_module_names(
                        file,
                        *node,
                        self.get_program(),
                        type_checker,
                    );
                }
                from_unionable_literal_type(store, grandparent, parent, position, type_checker)
            }
            ast::Kind::PropertyAssignment => {
                let parent_parent = store.parent(*parent);
                if parent_parent.as_ref().is_some_and(|parent_parent| {
                    ast::is_object_literal_expression(store, *parent_parent)
                }) && store
                    .name(*parent)
                    .as_ref()
                    .is_some_and(|name| name == node)
                {
                    return Some(StringLiteralCompletions {
                        from_properties: string_literal_completions_for_object_literal(
                            store,
                            type_checker,
                            parent_parent.unwrap(),
                        ),
                        ..Default::default()
                    });
                }
                if ast::find_ancestor(store, store.parent(*parent), |store, n| {
                    ast::is_call_like_expression(store, &n)
                })
                .is_some()
                {
                    let mut string_literal_types = Vec::new();
                    string_literal_types.extend(get_string_literal_types(
                        type_checker.get_contextual_type_public(*node, checker::CONTEXT_FLAGS_NONE),
                        None,
                        type_checker,
                    ));
                    string_literal_types.extend(get_string_literal_types(
                        type_checker.get_contextual_type_public(
                            *node,
                            checker::CONTEXT_FLAGS_IGNORE_NODE_INFERENCES,
                        ),
                        None,
                        type_checker,
                    ));
                    return to_string_literal_completions_from_types(string_literal_types);
                }
                Some(StringLiteralCompletions {
                    from_types: from_contextual_type(
                        store,
                        checker::CONTEXT_FLAGS_NONE,
                        node,
                        type_checker,
                    ),
                    ..Default::default()
                })
            }
            ast::Kind::ElementAccessExpression => {
                let expression = store.expression(*parent).unwrap();
                let argument_expression = store.argument_expression(*parent)?;
                if *node == ast::skip_parentheses(store, argument_expression) {
                    let ty = type_checker.get_type_at_location(expression);
                    return Some(StringLiteralCompletions {
                        from_properties: string_literal_completions_from_properties(
                            store,
                            ty,
                            type_checker,
                        ),
                        ..Default::default()
                    });
                }
                None
            }
            ast::Kind::CallExpression | ast::Kind::NewExpression | ast::Kind::JsxAttribute => {
                if !is_require_call_argument(store, *node) && !ast::is_import_call(store, *parent) {
                    let jsx_parent = if store.kind(*parent) == ast::Kind::JsxAttribute {
                        store.parent(*parent)
                    } else {
                        None
                    };
                    let argument_node_storage;
                    let argument_node: &ast::Node =
                        if store.kind(*parent) == ast::Kind::JsxAttribute {
                            argument_node_storage = jsx_parent?;
                            &argument_node_storage
                        } else {
                            node
                        };
                    let argument_info = get_argument_info_for_completions(
                        argument_node,
                        position,
                        file,
                        type_checker,
                    );
                    let argument_info = argument_info?;
                    let result = get_string_literal_completions_from_signature(
                        store,
                        &argument_info.invocation,
                        node,
                        &argument_info,
                        type_checker,
                    );
                    if result.is_some() {
                        return Some(StringLiteralCompletions {
                            from_types: result,
                            ..Default::default()
                        });
                    }
                    return Some(StringLiteralCompletions {
                        from_types: from_contextual_type(
                            store,
                            checker::CONTEXT_FLAGS_NONE,
                            node,
                            type_checker,
                        ),
                        ..Default::default()
                    });
                }
                self.get_string_literal_completions_from_module_names(
                    file,
                    *node,
                    self.get_program(),
                    type_checker,
                )
            }
            ast::Kind::ImportDeclaration
            | ast::Kind::ExportDeclaration
            | ast::Kind::ExternalModuleReference => self
                .get_string_literal_completions_from_module_names(
                    file,
                    *node,
                    self.get_program(),
                    type_checker,
                ),
            ast::Kind::CaseClause => {
                let contextual_types = from_contextual_type(
                    store,
                    checker::CONTEXT_FLAGS_IGNORE_NODE_INFERENCES,
                    node,
                    type_checker,
                )?;
                let literals = contextual_types.types;
                Some(StringLiteralCompletions {
                    from_types: Some(CompletionsFromTypes {
                        types: literals,
                        is_new_identifier: false,
                    }),
                    ..Default::default()
                })
            }
            ast::Kind::ImportSpecifier | ast::Kind::ExportSpecifier => None,
            ast::Kind::BinaryExpression => {
                if store
                    .operator_token(*parent)
                    .is_some_and(|operator| store.kind(operator) == ast::Kind::InKeyword)
                {
                    let right = store.right(*parent).unwrap();
                    let ty = type_checker.get_type_at_location(right);
                    return Some(StringLiteralCompletions {
                        from_properties: Some(CompletionsFromProperties {
                            symbols: get_properties_for_completion(ty, type_checker)
                                .into_iter()
                                .filter(|s| {
                                    !type_checker
                                        .symbol_value_declaration_public(*s)
                                        .as_ref()
                                        .is_some_and(|value_declaration| {
                                            ast::is_private_identifier_class_element_declaration(
                                                store,
                                                *value_declaration,
                                            )
                                        })
                                })
                                .collect(),
                            has_index_signature: false,
                        }),
                        ..Default::default()
                    });
                }
                Some(StringLiteralCompletions {
                    from_types: from_contextual_type(
                        store,
                        checker::CONTEXT_FLAGS_NONE,
                        node,
                        type_checker,
                    ),
                    ..Default::default()
                })
            }
            _ => {
                let result = from_contextual_type(
                    store,
                    checker::CONTEXT_FLAGS_IGNORE_NODE_INFERENCES,
                    node,
                    type_checker,
                );
                Some(StringLiteralCompletions {
                    from_types: result.or_else(|| {
                        from_contextual_type(store, checker::CONTEXT_FLAGS_NONE, node, type_checker)
                    }),
                    ..Default::default()
                })
            }
        }
    }

    pub(crate) fn get_string_literal_completions_from_module_names<'a>(
        &self,
        file: &'a ast::SourceFile,
        node: ast::Node,
        program: &compiler::Program,
        checker: &mut checker::Checker<'a, '_>,
    ) -> Option<StringLiteralCompletions> {
        let name_and_kinds = self
            .get_string_literal_completions_from_module_names_worker(file, node, program, checker);
        let text_start = astnav::get_start_of_node(node, file) + 1;
        Some(StringLiteralCompletions {
            from_paths: Some(add_replacement_spans(
                &file.store().text(node),
                text_start,
                &name_and_kinds,
            )),
            ..Default::default()
        })
    }

    pub(crate) fn get_string_literal_completions_from_module_names_worker<'a>(
        &self,
        file: &'a ast::SourceFile,
        node: ast::Node,
        program: &compiler::Program,
        checker: &mut checker::Checker<'a, '_>,
    ) -> Vec<ModuleCompletionNameAndKind> {
        let literal_value = tspath::normalize_slashes(&file.store().text(node));
        let mut mode = core::ResolutionMode::None;
        if ast::is_string_literal_like(file.store(), node) {
            mode = program.get_mode_for_usage_location(file, &node);
        }
        let script_path = file.path();
        let script_directory = tspath::get_directory_path(&script_path);
        let options = program.options();
        let extension_options = self.get_extension_options(
            options,
            ReferenceKind::ModuleSpecifier,
            file,
            mode,
            Some(checker),
        );
        if is_path_relative_to_script(&literal_value)
            || (options.paths.is_empty()
                && (tspath::is_rooted_disk_path(&literal_value) || tspath::is_url(&literal_value)))
        {
            self.get_completion_entries_for_relative_modules(
                &literal_value,
                &script_directory,
                program,
                script_path,
                &extension_options,
            )
        } else {
            self.get_completion_entries_for_non_relative_modules(
                &literal_value,
                &script_directory,
                mode,
                program,
                checker,
                &extension_options,
            )
        }
    }
}

fn from_contextual_type<'a>(
    store: &ast::AstStore,
    context_flags: checker::ContextFlags,
    node: &ast::Node,
    type_checker: &mut checker::Checker<'a, '_>,
) -> Option<CompletionsFromTypes> {
    let contextual_type =
        get_contextual_type_from_parent(store, *node, type_checker, context_flags);
    let literal_types = get_string_literal_types(contextual_type, None, type_checker);
    to_completions_from_types(literal_types)
}

fn to_completions_from_types<'a>(types: Vec<checker::TypeHandle>) -> Option<CompletionsFromTypes> {
    if types.is_empty() {
        return None;
    }
    Some(CompletionsFromTypes {
        types,
        is_new_identifier: false,
    })
}

fn to_string_literal_completions_from_types<'a>(
    types: Vec<checker::TypeHandle>,
) -> Option<StringLiteralCompletions> {
    let result = to_completions_from_types(types)?;
    Some(StringLiteralCompletions {
        from_types: Some(result),
        ..Default::default()
    })
}

fn from_unionable_literal_type<'a>(
    store: &'a ast::AstStore,
    grandparent: &ast::Node,
    parent: &ast::Node,
    position: i32,
    type_checker: &mut checker::Checker<'a, '_>,
) -> Option<StringLiteralCompletions> {
    match store.kind(*grandparent) {
        ast::Kind::CallExpression
        | ast::Kind::ExpressionWithTypeArguments
        | ast::Kind::JsxOpeningElement
        | ast::Kind::JsxSelfClosingElement
        | ast::Kind::NewExpression
        | ast::Kind::TaggedTemplateExpression
        | ast::Kind::TypeReference => {
            let type_argument = ast::find_ancestor(store, Some(*parent), |store, n| {
                store.parent(n).as_ref().is_some_and(|p| p == grandparent)
            })?;
            let constraint = type_checker.get_type_argument_constraint_public(type_argument);
            Some(StringLiteralCompletions {
                from_types: Some(CompletionsFromTypes {
                    types: get_string_literal_types(constraint, None, type_checker),
                    is_new_identifier: false,
                }),
                ..Default::default()
            })
        }
        ast::Kind::IndexedAccessType => {
            let index_type = store.index_type(*grandparent)?;
            if !store.loc(index_type).contains_inclusive(position) {
                return None;
            }
            let object_type = store.object_type(*grandparent)?;
            let ty = type_checker.get_type_from_type_node_public(object_type);
            Some(StringLiteralCompletions {
                from_properties: string_literal_completions_from_properties(
                    store,
                    ty,
                    type_checker,
                ),
                ..Default::default()
            })
        }
        ast::Kind::UnionType => {
            let grandparent_parent = store.parent(*grandparent)?;
            let grandparent_parent = walk_up_parentheses(store, Some(grandparent_parent));
            let result = from_unionable_literal_type(
                store,
                &grandparent_parent,
                parent,
                position,
                type_checker,
            )?;
            let already_used_types =
                get_already_used_types_in_string_literal_union(store, *grandparent, *parent);
            if let Some(properties) = result.from_properties {
                return Some(StringLiteralCompletions {
                    from_properties: Some(CompletionsFromProperties {
                        symbols: properties
                            .symbols
                            .into_iter()
                            .filter(|s| {
                                type_checker
                                    .symbol_name_public(*s)
                                    .is_none_or(|name| !already_used_types.contains(&name))
                            })
                            .collect(),
                        has_index_signature: properties.has_index_signature,
                    }),
                    ..Default::default()
                });
            }
            if let Some(types) = result.from_types {
                return Some(StringLiteralCompletions {
                    from_types: Some(CompletionsFromTypes {
                        types: types
                            .types
                            .into_iter()
                            .filter(|t| {
                                !already_used_types
                                    .contains(&type_checker.get_string_literal_value_public(*t))
                            })
                            .collect(),
                        is_new_identifier: false,
                    }),
                    ..Default::default()
                });
            }
            None
        }
        ast::Kind::PropertySignature => Some(StringLiteralCompletions {
            from_types: Some(CompletionsFromTypes {
                types: {
                    let constraint = get_constraint_of_type_argument_property(
                        store,
                        Some(grandparent),
                        type_checker,
                    );
                    get_string_literal_types(constraint, None, type_checker)
                },
                is_new_identifier: false,
            }),
            ..Default::default()
        }),
        _ => None,
    }
}

pub fn string_literal_completions_for_object_literal<'a>(
    store: &ast::AstStore,
    type_checker: &mut checker::Checker<'a, '_>,
    object_literal_expression: ast::Node,
) -> Option<CompletionsFromProperties> {
    let contextual_type = type_checker
        .get_contextual_type_public(object_literal_expression, checker::CONTEXT_FLAGS_NONE)?;
    let completions_type = type_checker.get_contextual_type_public(
        object_literal_expression,
        checker::CONTEXT_FLAGS_IGNORE_NODE_INFERENCES,
    );
    let has_index_signature = has_index_signature(contextual_type, type_checker);
    let symbols = get_properties_for_object_expression(
        store,
        contextual_type,
        completions_type,
        &object_literal_expression,
        type_checker,
    );
    Some(CompletionsFromProperties {
        symbols,
        has_index_signature,
    })
}

pub fn string_literal_completions_from_properties<'a>(
    store: &ast::AstStore,
    ty: checker::TypeHandle,
    type_checker: &mut checker::Checker<'a, '_>,
) -> Option<CompletionsFromProperties> {
    let has_index_signature = has_index_signature(ty, type_checker);
    Some(CompletionsFromProperties {
        symbols: get_properties_for_completion(ty, type_checker)
            .into_iter()
            .filter(|s| {
                !type_checker
                    .symbol_value_declaration_public(*s)
                    .as_ref()
                    .is_some_and(|value_declaration| {
                        ast::is_private_identifier_class_element_declaration(
                            store,
                            *value_declaration,
                        )
                    })
            })
            .collect(),
        has_index_signature,
    })
}

fn add_replacement_spans(
    text: &str,
    text_start: i32,
    names: &[ModuleCompletionNameAndKind],
) -> Vec<PathCompletion> {
    let text_range = get_directory_fragment_range(text, text_start);
    names
        .iter()
        .map(|name_and_kind| PathCompletion {
            name: name_and_kind.name.clone(),
            kind: modulet_to_script_element_kind(name_and_kind.kind),
            extension: name_and_kind.extension.clone(),
            text_range,
        })
        .collect()
}

pub fn modulet_to_script_element_kind(kind: ModuleCompletionKind) -> lsutil::ScriptElementKind {
    match kind {
        ModuleCompletionKind::Directory => lsutil::ScriptElementKind::Directory,
        ModuleCompletionKind::File => lsutil::ScriptElementKind::ScriptElement,
        ModuleCompletionKind::ExternalModuleName => lsutil::ScriptElementKind::ExternalModuleName,
    }
}

pub fn is_any_directory_separator(r: char) -> bool {
    r == '/' || r == '\\'
}

pub fn get_directory_fragment_range(text: &str, text_start: i32) -> Option<core::TextRange> {
    let offset = text
        .rfind(is_any_directory_separator)
        .map(|i| i + 1)
        .unwrap_or(0);
    let length = text.len() as i32 - offset as i32;
    if length == 0 {
        return None;
    }
    Some(core::new_text_range(
        text_start + offset as i32,
        text_start + offset as i32 + length,
    ))
}

pub fn get_fragment_directory(fragment: &str) -> String {
    if !contains_slash(fragment) {
        return String::new();
    }
    if tspath::has_trailing_directory_separator(fragment) {
        fragment.to_string()
    } else {
        tspath::get_directory_path(fragment)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ModuleCompletionKind {
    Directory = 0,
    File = 1,
    ExternalModuleName = 2,
}

impl Default for ModuleCompletionKind {
    fn default() -> Self {
        Self::File
    }
}

#[derive(Clone, Default)]
pub struct ModuleCompletionNameAndKind {
    pub name: String,
    pub kind: ModuleCompletionKind,
    pub extension: String,
}

#[derive(Clone, Default)]
pub struct ModuleCompletionNameAndKindSet {
    pub names: std::collections::HashMap<String, ModuleCompletionNameAndKind>,
}

impl ModuleCompletionNameAndKindSet {
    pub fn add(&mut self, entry: ModuleCompletionNameAndKind) {
        let replace = self
            .names
            .get(&entry.name)
            .is_none_or(|existing| existing.kind < entry.kind);
        if replace {
            self.names.insert(entry.name.clone(), entry);
        }
    }
}

#[derive(Clone, Default)]
pub struct ExtensionOptions<'a> {
    pub extensions_to_search: Vec<String>,
    pub reference_kind: ReferenceKind,
    pub importing_source_file: Option<&'a ast::SourceFile>,
    pub ending_preference: modulespecifiers::ImportModuleSpecifierEndingPreference,
    pub resolution_mode: core::ResolutionMode,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReferenceKind {
    FileName = 0,
    #[default]
    ModuleSpecifier = 1,
}

impl LanguageService<'_> {
    pub fn get_completion_entries_for_non_relative_modules<'a>(
        &self,
        fragment: &str,
        script_path: &str,
        _mode: core::ResolutionMode,
        program: &compiler::Program,
        type_checker: &mut checker::Checker<'a, '_>,
        extension_options: &ExtensionOptions<'a>,
    ) -> Vec<ModuleCompletionNameAndKind> {
        let compiler_options = program.options();
        let mut result = ModuleCompletionNameAndKindSet::default();
        if !compiler_options.paths.is_empty() {
            let absolute = compiler_options.get_paths_base_path(&program.current_directory());
            self.add_completion_entries_from_paths(
                &mut result,
                program,
                fragment,
                &absolute,
                extension_options,
                &compiler_options.paths,
            );
        }
        let fragment_directory = get_fragment_directory(fragment);
        for ambient_name in
            get_ambient_module_completions(fragment, fragment_directory.clone(), type_checker)
        {
            result.add(ModuleCompletionNameAndKind {
                name: ambient_name,
                kind: ModuleCompletionKind::ExternalModuleName,
                extension: String::new(),
            });
        }
        self.get_completion_entries_from_typings(
            program,
            script_path,
            fragment_directory.clone(),
            extension_options,
            &mut result,
        );
        if module_resolution_uses_node_modules(compiler_options.get_module_resolution_kind()) {
            let mut found_global = false;
            if fragment_directory.is_empty() {
                for module_name in self.enumerate_node_modules_visible_to_script(script_path) {
                    if !result.names.contains_key(&module_name) {
                        found_global = true;
                        result.add(ModuleCompletionNameAndKind {
                            name: module_name,
                            kind: ModuleCompletionKind::ExternalModuleName,
                            extension: String::new(),
                        });
                    }
                }
            }
            if !found_global {
                let global_cache_location = program.global_typings_cache_location();
                tspath::for_each_ancestor_directory_stopping_at_global_cache(
                    &global_cache_location,
                    script_path.to_string(),
                    |ancestor| {
                        let node_modules = tspath::combine_paths(ancestor, &["node_modules"]);
                        if self.directory_exists(&node_modules) {
                            self.get_completion_entries_for_directory_fragment(
                                fragment,
                                &node_modules,
                                extension_options,
                                program,
                                false,
                                "",
                                &mut result,
                            );
                        }
                        ((), false)
                    },
                );
            }
        }
        result.names.into_values().collect()
    }

    pub fn get_completion_entries_from_typings<'a>(
        &self,
        program: &compiler::Program,
        script_path: &str,
        fragment_directory: String,
        extension_options: &ExtensionOptions<'a>,
        result: &mut ModuleCompletionNameAndKindSet,
    ) {
        let options = program.options();
        let mut seen = std::collections::HashMap::new();
        let (type_roots, _) = options.get_effective_type_roots(&program.current_directory());
        for root in type_roots {
            self.get_completion_entries_from_typings_directories(
                &root,
                options,
                fragment_directory.clone(),
                extension_options,
                program,
                &mut seen,
                result,
            );
        }
        let global_cache_location = program.global_typings_cache_location();
        tspath::for_each_ancestor_directory_stopping_at_global_cache(
            &global_cache_location,
            script_path.to_string(),
            |directory| {
                let types_dir = tspath::combine_paths(directory, &["node_modules", "@types"]);
                self.get_completion_entries_from_typings_directories(
                    &types_dir,
                    options,
                    fragment_directory.clone(),
                    extension_options,
                    program,
                    &mut seen,
                    result,
                );
                ((), false)
            },
        );
    }

    pub fn get_completion_entries_from_typings_directories<'a>(
        &self,
        directory: &str,
        options: &core::CompilerOptions,
        fragment_directory: String,
        extension_options: &ExtensionOptions<'a>,
        program: &compiler::Program,
        seen: &mut std::collections::HashMap<String, bool>,
        result: &mut ModuleCompletionNameAndKindSet,
    ) {
        if !self.directory_exists(directory) {
            return;
        }
        for type_directory_name in self.get_directories(directory) {
            let package_name = module::unmangle_scoped_package_name(&type_directory_name);
            if !options.types.is_empty() && !options.types.contains(&package_name) {
                continue;
            }
            if fragment_directory.is_empty() {
                if !seen.contains_key(&package_name) {
                    result.add(ModuleCompletionNameAndKind {
                        name: package_name.clone(),
                        kind: ModuleCompletionKind::ExternalModuleName,
                        extension: String::new(),
                    });
                    seen.insert(package_name, true);
                }
            } else {
                let base_directory = tspath::combine_paths(directory, &[&type_directory_name]);
                if let Some(remaining_fragment) = try_remove_directory_prefix(
                    &fragment_directory,
                    &package_name,
                    program.use_case_sensitive_file_names(),
                ) {
                    self.get_completion_entries_for_directory_fragment(
                        &remaining_fragment,
                        &base_directory,
                        extension_options,
                        program,
                        false,
                        "",
                        result,
                    );
                }
            }
        }
    }

    pub fn enumerate_node_modules_visible_to_script(&self, script_path: &str) -> Vec<String> {
        let mut result = Vec::new();
        let global_cache_location = self.get_program().global_typings_cache_location();
        tspath::for_each_ancestor_directory_stopping_at_global_cache(
            &global_cache_location,
            script_path.to_string(),
            |directory| {
                let package_json_path = tspath::combine_paths(directory, &["package.json"]);
                if let Some(package_json_info) =
                    self.get_program().package_json_info(&package_json_path)
                {
                    if package_json_info.exists() {
                        if let Some(contents) = package_json_info.contents.as_ref() {
                            contents.fields.dependency_fields.range_dependencies(
                                |name, _version, _dependency_field| {
                                    if !name.starts_with("@types/") {
                                        result.push(name.to_string());
                                    }
                                    true
                                },
                            );
                        }
                    }
                }
                ((), false)
            },
        );
        result
    }

    pub fn get_extension_options<'a>(
        &self,
        options: &core::CompilerOptions,
        reference_kind: ReferenceKind,
        file: &'a ast::SourceFile,
        mode: core::ResolutionMode,
        checker: Option<&mut checker::Checker<'_, '_>>,
    ) -> ExtensionOptions<'a> {
        let extensions_to_search = get_supported_extensions_for_module_resolution(options, checker);
        ExtensionOptions {
            extensions_to_search,
            reference_kind,
            importing_source_file: Some(file),
            ending_preference: self.user_preferences().import_module_specifier_ending,
            resolution_mode: mode,
        }
    }

    pub fn get_completion_entries_for_relative_modules<'a>(
        &self,
        literal_value: &str,
        script_directory: &str,
        program: &compiler::Program,
        script_path: tspath::Path,
        extension_options: &ExtensionOptions<'a>,
    ) -> Vec<ModuleCompletionNameAndKind> {
        let options = program.options();
        if !options.root_dirs.is_empty() {
            self.get_completion_entries_for_directory_fragment_with_root_dirs(
                &options.root_dirs,
                literal_value,
                script_directory,
                program,
                script_path.as_str(),
                extension_options,
            )
        } else {
            let mut result = ModuleCompletionNameAndKindSet::default();
            self.get_completion_entries_for_directory_fragment(
                literal_value,
                script_directory,
                extension_options,
                program,
                true,
                script_path.as_str(),
                &mut result,
            );
            result.names.into_values().collect()
        }
    }

    pub fn get_completion_entries_for_directory_fragment_with_root_dirs<'a>(
        &self,
        root_dirs: &[String],
        fragment: &str,
        script_directory: &str,
        program: &compiler::Program,
        exclude: &str,
        extension_options: &ExtensionOptions<'a>,
    ) -> Vec<ModuleCompletionNameAndKind> {
        let options = program.options();
        let base_path = if !options.project.is_empty() {
            options.project.clone()
        } else {
            program.current_directory()
        };
        let base_directories = get_base_directories_from_root_dirs(
            root_dirs,
            &base_path,
            script_directory,
            !program.use_case_sensitive_file_names(),
        );
        let mut all_completions = Vec::new();
        for base_directory in base_directories {
            let mut result = ModuleCompletionNameAndKindSet::default();
            self.get_completion_entries_for_directory_fragment(
                fragment,
                &base_directory,
                extension_options,
                program,
                true,
                exclude,
                &mut result,
            );
            all_completions.extend(result.names.into_values());
        }
        deduplicate_module_completions(all_completions)
    }

    pub fn get_completion_entries_for_directory_fragment<'a>(
        &self,
        fragment: &str,
        script_directory: &str,
        extension_options: &ExtensionOptions<'a>,
        program: &compiler::Program,
        _module_specifier_is_relative: bool,
        exclude: &str,
        result: &mut ModuleCompletionNameAndKindSet,
    ) {
        let mut fragment = tspath::normalize_slashes(fragment);
        if !tspath::has_trailing_directory_separator(&fragment) {
            fragment = tspath::get_directory_path(&fragment);
        }
        if fragment.is_empty() {
            fragment = ".".to_string();
        }
        fragment = tspath::ensure_trailing_directory_separator(&fragment);
        let base_directory = tspath::resolve_path(script_directory, &[&fragment]);
        if !self.directory_exists(&base_directory) {
            return;
        }

        let files = self.read_directory(
            &base_directory,
            &extension_options.extensions_to_search,
            &["./*".to_string()],
        );
        for file_path in files {
            if tspath::compare_paths(
                exclude,
                &file_path,
                &tspath::ComparePathsOptions {
                    use_case_sensitive_file_names: program.use_case_sensitive_file_names(),
                    current_directory: program.current_directory(),
                    ..Default::default()
                },
            ) == std::cmp::Ordering::Equal
            {
                continue;
            }
            let (name, extension) = get_filename_with_extension_option(
                &tspath::get_base_file_name(&file_path),
                program,
                extension_options,
                false,
            );
            result.add(ModuleCompletionNameAndKind {
                name,
                kind: ModuleCompletionKind::File,
                extension,
            });
        }

        for directory in self.get_directories(&base_directory) {
            let directory_name = tspath::get_base_file_name(&directory);
            if directory_name != "@types" {
                result.add(ModuleCompletionNameAndKind {
                    name: directory_name,
                    kind: ModuleCompletionKind::Directory,
                    extension: String::new(),
                });
            }
        }
    }

    pub fn add_completion_entries_from_paths<'a>(
        &self,
        result: &mut ModuleCompletionNameAndKindSet,
        program: &compiler::Program,
        fragment: &str,
        base_directory: &str,
        extension_options: &ExtensionOptions<'a>,
        paths: &ts_collections::OrderedMap<String, Vec<String>>,
    ) -> bool {
        let mut matched_path = false;
        for key in paths.keys() {
            if key == "." {
                continue;
            }
            let normalized_key = key.strip_prefix("./").unwrap_or(key);
            let pattern = core::try_parse_pattern(normalized_key);
            if !pattern.is_valid() {
                continue;
            }
            let patterns = paths.get_or_zero(key);
            if patterns.is_empty() {
                continue;
            }
            matched_path |= pattern.matches(fragment);
            for completion in self.get_completions_for_path_mapping(
                normalized_key,
                &patterns,
                fragment,
                base_directory,
                false,
                false,
                extension_options,
                program,
            ) {
                result.add(completion);
            }
        }
        matched_path
    }

    pub fn add_completion_entries_from_paths_or_exports_or_imports<'a>(
        &self,
        result: &mut ModuleCompletionNameAndKindSet,
        program: &compiler::Program,
        is_exports: bool,
        is_imports: bool,
        fragment: &str,
        base_directory: &str,
        extension_options: &ExtensionOptions<'a>,
    ) -> bool {
        let before = result.names.len();
        if is_exports || is_imports {
            self.get_completion_entries_for_directory_fragment(
                fragment,
                base_directory,
                extension_options,
                program,
                false,
                "",
                result,
            );
        }
        result.names.len() != before
    }

    pub fn get_completions_for_path_mapping<'a>(
        &self,
        path: &str,
        patterns: &[String],
        fragment: &str,
        package_directory: &str,
        is_exports: bool,
        is_imports: bool,
        extension_options: &ExtensionOptions<'a>,
        program: &compiler::Program,
    ) -> Vec<ModuleCompletionNameAndKind> {
        let parsed_path = core::try_parse_pattern(path);
        if !parsed_path.is_valid() {
            return Vec::new();
        }
        if parsed_path.star_index == -1 {
            let extension = patterns
                .first()
                .map(|pattern| get_file_extension(pattern))
                .unwrap_or_default();
            if path.starts_with(fragment) {
                return vec![ModuleCompletionNameAndKind {
                    name: tspath::remove_trailing_directory_separator(path).to_string(),
                    kind: ModuleCompletionKind::File,
                    extension,
                }];
            }
            return Vec::new();
        }
        let star_index = parsed_path.star_index as usize;
        let path_prefix = &parsed_path.text[..star_index];
        let path_suffix = &parsed_path.text[star_index + 1..];
        if !fragment.starts_with(path_prefix) && !path_prefix.starts_with(fragment) {
            return Vec::new();
        }
        let remaining_fragment = fragment.strip_prefix(path_prefix).unwrap_or("");
        let mut completions = Vec::new();
        for pattern in patterns {
            let mut modules = self.get_modules_for_paths_pattern(
                remaining_fragment,
                package_directory,
                pattern,
                is_exports,
                is_imports,
                extension_options,
                program,
            );
            for module in &mut modules {
                if module.kind == ModuleCompletionKind::File {
                    module.name.push_str(path_suffix);
                }
            }
            completions.extend(modules);
        }
        completions
    }

    pub fn get_modules_for_paths_pattern<'a>(
        &self,
        fragment: &str,
        package_directory: &str,
        pattern: &str,
        _is_exports: bool,
        _is_imports: bool,
        extension_options: &ExtensionOptions<'a>,
        program: &compiler::Program,
    ) -> Vec<ModuleCompletionNameAndKind> {
        let parsed = core::try_parse_pattern(pattern);
        if !parsed.is_valid() || parsed.star_index == -1 {
            return Vec::new();
        }
        let prefix = &parsed.text[..parsed.star_index as usize];
        let suffix = &parsed.text[parsed.star_index as usize + 1..];
        let normalized_prefix = tspath::resolve_path(prefix, &[]);
        let (normalized_prefix_directory, normalized_prefix_base) =
            if tspath::has_trailing_directory_separator(prefix) {
                (normalized_prefix, String::new())
            } else {
                (
                    tspath::get_directory_path(&normalized_prefix),
                    tspath::get_base_file_name(&normalized_prefix),
                )
            };
        let fragment_directory = if contains_slash(fragment) {
            if tspath::has_trailing_directory_separator(fragment) {
                fragment.to_string()
            } else {
                tspath::get_directory_path(fragment)
            }
        } else {
            String::new()
        };
        let expanded_prefix_directory = if !fragment_directory.is_empty() {
            tspath::combine_paths(
                &normalized_prefix_directory,
                &[&(normalized_prefix_base.clone() + &fragment_directory)],
            )
        } else {
            normalized_prefix_directory
        };
        let base_directory = tspath::normalize_path(&tspath::combine_paths(
            package_directory,
            &[&expanded_prefix_directory],
        ));
        let normalized_suffix = tspath::normalize_path(suffix);
        let include_globs = if normalized_suffix.is_empty() {
            vec!["./*".to_string()]
        } else {
            vec![format!("**/*{normalized_suffix}")]
        };
        let mut result = Vec::new();
        for file_path in self.read_directory(
            &base_directory,
            &extension_options.extensions_to_search,
            &include_globs,
        ) {
            let normalized = tspath::normalize_path(&file_path);
            let Some(inner) =
                without_start_and_end(&normalized, &base_directory, &normalized_suffix)
            else {
                continue;
            };
            let trimmed = remove_leading_directory_separator(&inner);
            if contains_slash(&trimmed) {
                let path_components = tspath::get_path_components(&trimmed, "");
                if path_components.len() > 1 {
                    result.push(ModuleCompletionNameAndKind {
                        name: path_components[1].clone(),
                        kind: ModuleCompletionKind::Directory,
                        extension: String::new(),
                    });
                }
            } else {
                let (name, mut extension) =
                    get_filename_with_extension_option(&trimmed, program, extension_options, false);
                if extension.is_empty() {
                    extension = get_file_extension(&file_path);
                }
                result.push(ModuleCompletionNameAndKind {
                    name,
                    kind: ModuleCompletionKind::File,
                    extension,
                });
            }
        }
        if normalized_suffix.is_empty() {
            for dir in self.get_directories(&base_directory) {
                if dir != "node_modules" {
                    result.push(ModuleCompletionNameAndKind {
                        name: dir,
                        kind: ModuleCompletionKind::Directory,
                        extension: String::new(),
                    });
                }
            }
        }
        result
    }

    pub fn get_string_literal_completion_details<'a>(
        &self,
        ctx: &core::Context,
        checker: &mut checker::Checker<'a, '_>,
        item: lsproto::CompletionItem,
        name: &str,
        file: &'a ast::SourceFile,
        position: i32,
        context_token: Option<ast::Node>,
        doc_format: lsproto::MarkupKind,
    ) -> lsproto::CompletionItem {
        let Some(context_token) = context_token else {
            return item;
        };
        if !ast::is_string_literal_like(file.store(), context_token) {
            return item;
        }
        let completions = self.get_string_literal_completion_entries(
            ctx,
            file,
            &context_token,
            position,
            checker,
        );
        let Some(completions) = completions else {
            return item;
        };
        self.string_literal_completion_details(
            item,
            name,
            context_token,
            position,
            &completions,
            file,
            checker,
            doc_format,
        )
    }

    pub(crate) fn string_literal_completion_details<'a>(
        &self,
        mut item: lsproto::CompletionItem,
        name: &str,
        location: ast::Node,
        _position: i32,
        completion: &StringLiteralCompletions,
        _file: &'a ast::SourceFile,
        checker: &mut checker::Checker<'a, '_>,
        doc_format: lsproto::MarkupKind,
    ) -> lsproto::CompletionItem {
        if completion.from_paths.is_some() {
            return item;
        }
        if let Some(properties) = &completion.from_properties {
            for symbol in &properties.symbols {
                if checker
                    .symbol_name_public(*symbol)
                    .is_some_and(|symbol_name| symbol_name == name)
                {
                    let mut vc = checker::VerbosityContext {
                        level: 0,
                        max_truncation_length: 0,
                        can_increase_verbosity: false,
                        truncated: false,
                    };
                    let (quick_info, documentation) = self
                        .get_quick_info_and_documentation_for_symbol(
                            checker,
                            Some(*symbol),
                            location,
                            None,
                            doc_format.clone(),
                            &mut vc,
                        );
                    create_completion_details(&mut item, &quick_info, &documentation, doc_format);
                    return item;
                }
            }
        }
        if let Some(types) = &completion.from_types {
            for ty in &types.types {
                if checker.get_string_literal_value_public(*ty) == name {
                    create_completion_details(&mut item, name, "", doc_format);
                    return item;
                }
            }
        }
        item
    }

    pub(crate) fn get_triple_slash_reference_completions<'a>(
        &self,
        file: &'a ast::SourceFile,
        position: i32,
        program: &compiler::Program,
        checker: &mut checker::Checker<'a, '_>,
    ) -> Vec<PathCompletion> {
        let compiler_options = program.options();
        let text_before_position = &file.text()[..position as usize];
        let line_start = text_before_position
            .rfind(['\r', '\n'])
            .map(|index| index + 1)
            .unwrap_or(0);
        let text = &file.text()[line_start..position as usize];
        let (prefix, kind, to_complete, ok) = parse_triple_slash_directive_fragment(text);
        if !ok {
            return Vec::new();
        }
        let script_path = tspath::get_directory_path(file.path().as_str());
        let names = match kind.as_str() {
            "path" => {
                let extension_options = self.get_extension_options(
                    compiler_options,
                    ReferenceKind::FileName,
                    file,
                    core::ResolutionMode::None,
                    None,
                );
                let mut result = ModuleCompletionNameAndKindSet::default();
                self.get_completion_entries_for_directory_fragment(
                    &to_complete,
                    &script_path,
                    &extension_options,
                    program,
                    true,
                    file.path().as_str(),
                    &mut result,
                );
                result.names.into_values().collect()
            }
            "types" => {
                let extension_options = self.get_extension_options(
                    compiler_options,
                    ReferenceKind::ModuleSpecifier,
                    file,
                    core::ResolutionMode::None,
                    Some(checker),
                );
                let mut result = ModuleCompletionNameAndKindSet::default();
                self.get_completion_entries_from_typings(
                    program,
                    &script_path,
                    get_fragment_directory(&to_complete),
                    &extension_options,
                    &mut result,
                );
                result.names.into_values().collect()
            }
            _ => Vec::new(),
        };
        add_replacement_spans(
            &to_complete,
            line_start as i32 + prefix.len() as i32,
            &names,
        )
    }
}

pub fn get_pattern_from_first_matching_condition(
    target: &packagejson::ExportsOrImports,
    conditions: &[String],
) -> String {
    fn from_value(value: &serde_json::Value, conditions: &[String]) -> String {
        if let Some(value) = value.as_str() {
            return value.to_string();
        }
        if let Some(obj) = value.as_object() {
            for (condition, nested) in obj {
                if condition == "default"
                    || conditions.contains(condition)
                    || (conditions.iter().any(|c| c == "types")
                        && module::is_applicable_versioned_types_key(condition))
                {
                    let pattern = from_value(nested, conditions);
                    if !pattern.is_empty() {
                        return pattern;
                    }
                }
            }
        }
        String::new()
    }
    from_value(&target.json_value.value, conditions)
}

pub fn get_ambient_module_completions<'a>(
    fragment: &str,
    fragment_directory: String,
    type_checker: &mut checker::Checker<'a, '_>,
) -> Vec<String> {
    let mut non_relative_module_names: Vec<String> = Vec::new();
    for sym in type_checker.get_ambient_modules() {
        if let Some(name) = type_checker.symbol_name_public(sym) {
            let module_name = stringutil::strip_quotes(&name);
            if module_name.starts_with(fragment) && !module_name.contains('*') {
                non_relative_module_names.push(module_name.to_string());
            }
        }
    }
    if !fragment_directory.is_empty() {
        let module_name_with_separator =
            tspath::ensure_trailing_directory_separator(&fragment_directory);
        for module_name in &mut non_relative_module_names {
            if let Some(trimmed) = module_name.strip_prefix(&module_name_with_separator) {
                *module_name = trimmed.to_string();
            }
        }
    }
    non_relative_module_names
}

pub fn try_remove_directory_prefix(
    path: &str,
    prefix: &str,
    use_case_sensitive_file_names: bool,
) -> Option<String> {
    let canonical_path = tspath::get_canonical_file_name(path, use_case_sensitive_file_names);
    let canonical_prefix = tspath::get_canonical_file_name(prefix, use_case_sensitive_file_names);
    if canonical_path.starts_with(&canonical_prefix) {
        let mut without_prefix = path[prefix.len()..].to_string();
        if without_prefix.starts_with('/') || without_prefix.starts_with('\\') {
            without_prefix.remove(0);
        }
        Some(without_prefix)
    } else {
        None
    }
}

pub fn get_supported_extensions_for_module_resolution(
    options: &core::CompilerOptions,
    checker: Option<&mut checker::Checker<'_, '_>>,
) -> Vec<String> {
    let mut extensions = Vec::new();
    if let Some(checker) = checker {
        for module in checker.get_ambient_modules() {
            if let Some(module_name) = checker.symbol_name_public(module) {
                let name = stringutil::strip_quotes(&module_name);
                if name.starts_with("*.") && !name.contains('/') {
                    extensions.push(name[1..].to_string());
                }
            }
        }
    }
    for ext in tsoptions::get_supported_extensions(options, &[]) {
        extensions.extend(ext);
    }
    if module_resolution_uses_node_modules(options.get_module_resolution_kind()) {
        return tsoptions::get_supported_extensions_with_json_if_resolve_json_module(
            Some(options),
            vec![extensions],
        )
        .into_iter()
        .flatten()
        .collect();
    }
    extensions
}

pub fn module_resolution_uses_node_modules(module_resolution: core::ModuleResolutionKind) -> bool {
    (module_resolution >= core::ModuleResolutionKind::Node16
        && module_resolution <= core::ModuleResolutionKind::NodeNext)
        || module_resolution == core::ModuleResolutionKind::Bundler
}

pub fn is_path_relative_to_script(path: &str) -> bool {
    path.starts_with("./") || path.starts_with("../")
}

pub fn get_base_directories_from_root_dirs(
    root_dirs: &[String],
    base_path: &str,
    script_directory: &str,
    ignore_case: bool,
) -> Vec<String> {
    let normalized_root_dirs = root_dirs
        .iter()
        .map(|root_directory| {
            let normalized_path = if tspath::is_rooted_disk_path(root_directory) {
                root_directory.clone()
            } else {
                tspath::combine_paths(base_path, &[root_directory])
            };
            tspath::ensure_trailing_directory_separator(&tspath::normalize_path(&normalized_path))
        })
        .collect::<Vec<_>>();
    let compare_paths_options = tspath::ComparePathsOptions {
        use_case_sensitive_file_names: !ignore_case,
        current_directory: base_path.to_string(),
        ..Default::default()
    };
    let mut relative_directory = String::new();
    for root_directory in &normalized_root_dirs {
        if tspath::contains_path(root_directory, script_directory, &compare_paths_options) {
            if root_directory.len() <= script_directory.len() {
                relative_directory = script_directory[root_directory.len()..].to_string();
            }
            break;
        }
    }
    let mut directories = normalized_root_dirs
        .iter()
        .map(|root_directory| {
            tspath::remove_trailing_directory_separator(&tspath::combine_paths(
                root_directory,
                &[&relative_directory],
            ))
            .to_string()
        })
        .collect::<Vec<_>>();
    directories.push(tspath::remove_trailing_directory_separator(script_directory).to_string());
    deduplicate_strings(directories)
}

pub fn deduplicate_strings(slice: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for s in slice {
        if seen.insert(s.clone()) {
            result.push(s);
        }
    }
    result
}

pub fn deduplicate_module_completions(
    completions: Vec<ModuleCompletionNameAndKind>,
) -> Vec<ModuleCompletionNameAndKind> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for completion in completions {
        let key = (
            completion.name.clone(),
            completion.kind as i32,
            completion.extension.clone(),
        );
        if seen.insert(key) {
            result.push(completion);
        }
    }
    result
}

pub fn contains_slash(fragment: &str) -> bool {
    fragment.contains(tspath::DIRECTORY_SEPARATOR)
}

pub fn without_start_and_end(s: &str, start: &str, end: &str) -> Option<String> {
    if s.starts_with(start) && s.ends_with(end) && s.len() >= start.len() + end.len() {
        Some(s[start.len()..s.len() - end.len()].to_string())
    } else {
        None
    }
}

pub fn remove_leading_directory_separator(path: &str) -> String {
    path.trim_start_matches(tspath::DIRECTORY_SEPARATOR)
        .to_string()
}

pub fn get_possible_original_input_path_without_changing_ext(
    file_path: &str,
    ignore_case: bool,
    output_dir: &str,
    get_common_source_directory: impl Fn() -> String,
) -> String {
    if !output_dir.is_empty() {
        let relative_path = tspath::get_relative_path_from_directory(
            output_dir,
            file_path,
            &tspath::ComparePathsOptions {
                use_case_sensitive_file_names: !ignore_case,
                ..Default::default()
            },
        );
        return tspath::resolve_path(&get_common_source_directory(), &[&relative_path]);
    }
    file_path.to_string()
}

pub fn get_file_extension(file_name: &str) -> String {
    let extension = tspath::try_get_extension_from_path(file_name);
    if !extension.is_empty() {
        return extension.to_string();
    }
    tspath::get_any_extension_from_path(file_name, &[], false)
}

pub fn get_filename_with_extension_option<'a>(
    name: &str,
    program: &compiler::Program,
    extension_options: &ExtensionOptions<'a>,
    is_exports_or_imports_wildcard: bool,
) -> (String, String) {
    let non_js_result =
        modulespecifiers::try_get_real_file_name_for_non_js_declaration_file_name(name);
    if !non_js_result.is_empty() {
        let ext = tspath::try_get_extension_from_path(&non_js_result);
        return (non_js_result, ext.to_string());
    }
    if extension_options.reference_kind == ReferenceKind::FileName {
        return (
            name.to_string(),
            tspath::try_get_extension_from_path(name).to_string(),
        );
    }
    let Some(importing_source_file) = extension_options.importing_source_file else {
        return (
            name.to_string(),
            tspath::try_get_extension_from_path(name).to_string(),
        );
    };
    let mut allowed_endings = modulespecifiers::get_allowed_endings_in_preferred_order(
        &modulespecifiers::UserPreferences {
            import_module_specifier_ending: extension_options.ending_preference,
            ..Default::default()
        },
        program,
        program.options(),
        importing_source_file,
        "",
        extension_options.resolution_mode,
    );
    if is_exports_or_imports_wildcard {
        allowed_endings.retain(|e| {
            *e != modulespecifiers::ModuleSpecifierEnding::Minimal
                && *e != modulespecifiers::ModuleSpecifierEnding::Index
        });
    }
    if allowed_endings
        .first()
        .is_some_and(|e| *e == modulespecifiers::ModuleSpecifierEnding::TsExtension)
    {
        if tspath::file_extension_is_one_of(name, tspath::SUPPORTED_TS_IMPLEMENTATION_EXTENSIONS) {
            return (
                name.to_string(),
                tspath::try_get_extension_from_path(name).to_string(),
            );
        }
        let output_extension = module::try_get_js_extension_for_file(
            name,
            program.options().jsx == core::JsxEmit::Preserve,
        );
        if !output_extension.is_empty() {
            return (
                tspath::change_extension(name, &output_extension),
                output_extension,
            );
        }
        return (
            name.to_string(),
            tspath::try_get_extension_from_path(name).to_string(),
        );
    }
    if !is_exports_or_imports_wildcard
        && allowed_endings.first().is_some_and(|e| {
            *e == modulespecifiers::ModuleSpecifierEnding::Minimal
                || *e == modulespecifiers::ModuleSpecifierEnding::Index
        })
        && tspath::file_extension_is_one_of(
            name,
            &[
                tspath::EXTENSION_JS,
                tspath::EXTENSION_JSX,
                tspath::EXTENSION_TS,
                tspath::EXTENSION_TSX,
                tspath::EXTENSION_DTS,
            ],
        )
    {
        return (
            tspath::remove_file_extension(name),
            tspath::try_get_extension_from_path(name).to_string(),
        );
    }
    let output_extension = module::try_get_js_extension_for_file(
        name,
        program.options().jsx == core::JsxEmit::Preserve,
    );
    if !output_extension.is_empty() {
        return (
            tspath::change_extension(name, &output_extension),
            output_extension,
        );
    }
    (
        name.to_string(),
        tspath::try_get_extension_from_path(name).to_string(),
    )
}

pub(crate) fn walk_up_parentheses(store: &ast::AstStore, node: Option<ast::Node>) -> ast::Node {
    let node = node.unwrap();
    match store.kind(node) {
        ast::Kind::ParenthesizedType => {
            ast::walk_up_parenthesized_types(store, Some(node)).unwrap_or(node)
        }
        ast::Kind::ParenthesizedExpression => {
            ast::walk_up_parenthesized_expressions(store, Some(node)).unwrap_or(node)
        }
        _ => node,
    }
}

pub fn get_string_literal_types<'a>(
    mut ty: Option<checker::TypeHandle>,
    mut uniques: Option<std::collections::HashSet<String>>,
    type_checker: &mut checker::Checker<'a, '_>,
) -> Vec<checker::TypeHandle> {
    let Some(mut ty_val) = ty.take() else {
        return Vec::new();
    };
    if uniques.is_none() {
        uniques = Some(std::collections::HashSet::new());
    }
    ty_val = skip_constraint(ty_val, type_checker);
    if type_checker.is_union_type_public(ty_val) {
        let mut result = Vec::new();
        for element_type in type_checker.type_types_public(ty_val) {
            result.extend(get_string_literal_types(
                Some(element_type),
                uniques.clone(),
                type_checker,
            ));
        }
        return result;
    }
    if type_checker.is_string_literal_type_public(ty_val)
        && !type_checker.is_enum_literal_type_public(ty_val)
        && uniques
            .as_mut()
            .unwrap()
            .insert(type_checker.get_string_literal_value_public(ty_val))
    {
        return vec![ty_val];
    }
    Vec::new()
}

pub fn get_already_used_types_in_string_literal_union(
    store: &ast::AstStore,
    union: ast::Node,
    current: ast::Node,
) -> Vec<String> {
    let Some(types) = store.types(union) else {
        return Vec::new();
    };
    types
        .into_iter()
        .filter(|type_node| {
            *type_node != current
                && ast::is_literal_type_node(store, *type_node)
                && store
                    .literal(*type_node)
                    .is_some_and(|literal| ast::is_string_literal(store, literal))
        })
        .map(|type_node| {
            let literal = store.literal(type_node).unwrap();
            store.text(literal)
        })
        .collect()
}

pub fn has_index_signature<'a>(
    ty: checker::TypeHandle,
    type_checker: &mut checker::Checker<'a, '_>,
) -> bool {
    type_checker.get_string_index_type_public(ty).is_some()
        || type_checker.get_number_index_type_public(ty).is_some()
}

pub(crate) fn is_require_call_argument(store: &ast::AstStore, node: ast::Node) -> bool {
    let Some(parent) = store.parent(node) else {
        return false;
    };
    ast::is_call_expression(store, parent)
        && store
            .arguments(parent)
            .is_some_and(|arguments| arguments.first() == Some(node))
        && store
            .expression(parent)
            .as_ref()
            .is_some_and(|expression| ast::is_identifier(store, *expression))
        && store.text_eq(store.expression(parent).unwrap(), "require")
}

pub fn kind_modifiers_from_extension(extension: &str) -> lsutil::ScriptElementKindModifier {
    match extension {
        tspath::EXTENSION_DTS => lsutil::SCRIPT_ELEMENT_KIND_MODIFIER_DTS,
        tspath::EXTENSION_JS => lsutil::SCRIPT_ELEMENT_KIND_MODIFIER_JS,
        tspath::EXTENSION_JSON => lsutil::SCRIPT_ELEMENT_KIND_MODIFIER_JSON,
        tspath::EXTENSION_JSX => lsutil::SCRIPT_ELEMENT_KIND_MODIFIER_JSX,
        tspath::EXTENSION_TS => lsutil::SCRIPT_ELEMENT_KIND_MODIFIER_TS,
        tspath::EXTENSION_TSX => lsutil::SCRIPT_ELEMENT_KIND_MODIFIER_TSX,
        tspath::EXTENSION_DMTS => lsutil::SCRIPT_ELEMENT_KIND_MODIFIER_DMTS,
        tspath::EXTENSION_MJS => lsutil::SCRIPT_ELEMENT_KIND_MODIFIER_MJS,
        tspath::EXTENSION_MTS => lsutil::SCRIPT_ELEMENT_KIND_MODIFIER_MTS,
        tspath::EXTENSION_DCTS => lsutil::SCRIPT_ELEMENT_KIND_MODIFIER_DCTS,
        tspath::EXTENSION_CJS => lsutil::SCRIPT_ELEMENT_KIND_MODIFIER_CJS,
        tspath::EXTENSION_CTS => lsutil::SCRIPT_ELEMENT_KIND_MODIFIER_CTS,
        tspath::EXTENSION_TS_BUILD_INFO => panic!("unsupported extension"),
        _ => lsutil::SCRIPT_ELEMENT_KIND_MODIFIER_NONE,
    }
}

fn get_string_literal_completions_from_signature<'a>(
    store: &ast::AstStore,
    call: &ast::CallLikeExpression,
    arg: &ast::StringLiteralLike,
    argument_info: &ArgumentInfoForCompletions,
    type_checker: &mut checker::Checker<'a, '_>,
) -> Option<CompletionsFromTypes> {
    let mut is_new_identifier = false;
    let mut uniques = std::collections::HashSet::new();
    let editing_argument = if ast::is_jsx_opening_like_element(store, *call) {
        let parent = store.parent(*arg);
        ast::find_ancestor(store, parent, |store, node| {
            ast::is_jsx_attribute(store, node)
        })?
    } else {
        *arg
    };
    let candidates = type_checker
        .get_candidate_signatures_for_string_literal_completions(call, editing_argument);
    let mut types: Vec<checker::TypeHandle> = Vec::new();
    for candidate in candidates {
        if !type_checker.signature_has_rest_parameter_public(candidate)
            && argument_info.argument_count as usize
                > type_checker.signature_parameters_public(candidate).len()
        {
            continue;
        }
        let mut ty = type_checker
            .get_type_parameter_at_position(candidate, argument_info.argument_index as usize);
        if ast::is_jsx_opening_like_element(store, *call) {
            let attribute_name = store.name(editing_argument)?;
            if let Some(prop_type) =
                type_checker.get_type_of_property_of_type_public(ty, &store.text(attribute_name))
            {
                ty = prop_type;
            }
        }
        is_new_identifier = is_new_identifier || type_checker.is_string_type_public(ty);
        types.extend(get_string_literal_types(
            Some(ty),
            Some(uniques.clone()),
            type_checker,
        ));
        for ty in &types {
            uniques.insert(type_checker.get_string_literal_value_public(*ty));
        }
    }
    if types.is_empty() {
        None
    } else {
        Some(CompletionsFromTypes {
            types,
            is_new_identifier,
        })
    }
}

pub fn is_in_reference_comment(file: &ast::SourceFile, position: i32) -> bool {
    let token_at_position = astnav::get_token_at_position(file, position);
    let comment_range = is_in_comment(file, position, token_at_position.as_ref());
    let Some(comment_range) = comment_range else {
        return false;
    };
    let comment_text = &file.text()
        [comment_range.text_range.pos() as usize..comment_range.text_range.end() as usize];
    has_triple_slash_prefix(comment_text)
}

pub fn has_triple_slash_prefix(comment_text: &str) -> bool {
    comment_text.starts_with("///") && comment_text[3..].trim_start().starts_with('<')
}

pub fn parse_triple_slash_directive_fragment(text: &str) -> (String, String, String, bool) {
    let mut rest = text;
    if !rest.starts_with("///") {
        return (String::new(), String::new(), String::new(), false);
    }
    rest = &rest[3..];
    rest = rest.trim_start_matches(stringutil::is_white_space_like);
    if !rest.starts_with("<reference") {
        return (String::new(), String::new(), String::new(), false);
    }
    rest = &rest["<reference".len()..];
    if rest.is_empty() || !stringutil::is_white_space_like(rest.as_bytes()[0] as char) {
        return (String::new(), String::new(), String::new(), false);
    }
    rest = rest.trim_start_matches(stringutil::is_white_space_like);
    let kind;
    if rest.starts_with("path") {
        kind = "path";
        rest = &rest["path".len()..];
    } else if rest.starts_with("types") {
        kind = "types";
        rest = &rest["types".len()..];
    } else {
        return (String::new(), String::new(), String::new(), false);
    }
    rest = rest.trim_start_matches(stringutil::is_white_space_like);
    if !rest.starts_with('=') {
        return (String::new(), String::new(), String::new(), false);
    }
    rest = &rest[1..];
    rest = rest.trim_start_matches(stringutil::is_white_space_like);
    if rest.is_empty() || (rest.as_bytes()[0] != b'\'' && rest.as_bytes()[0] != b'\"') {
        return (String::new(), String::new(), String::new(), false);
    }
    rest = &rest[1..];
    if rest.contains(['\'', '\"']) {
        return (String::new(), String::new(), String::new(), false);
    }
    let to_complete = rest.to_string();
    let prefix = text[..text.len() - to_complete.len()].to_string();
    (prefix, kind.to_string(), to_complete, true)
}
