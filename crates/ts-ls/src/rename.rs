use std::collections::HashMap;

use ts_ast as ast;
use ts_astnav as astnav;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_locale as locale;
use ts_lsproto as lsproto;
use ts_module as module;
use ts_modulespecifiers::CheckerShape;
use ts_tspath as tspath;

use crate::LanguageService;
use crate::crossproject::{CrossProjectOrchestrator, handle_cross_project};
use crate::findallreferences::{
    ENTRY_KIND_RANGE, ENTRY_KIND_SEARCHED_LOCAL_FOUND_PROPERTY,
    ENTRY_KIND_SEARCHED_PROPERTY_FOUND_LOCAL, ReferenceEntry, SymbolAndEntries,
    SymbolAndEntriesData, SymbolEntryTransformOptions,
};
use crate::lsconv;
use crate::lsutil;
use crate::symbols;
use crate::utilities::{
    get_adjusted_location, has_contextual_or_ancestor_string_literal_type,
    is_literal_name_of_property_declaration_or_index_access,
};

fn source_file_for_node<'a>(
    program: &'a compiler::Program,
    node: &ast::Node,
) -> Option<&'a ast::SourceFile> {
    program
        .get_parsed_source_files_refs()
        .into_iter()
        .find(|file| {
            ast::get_source_file_of_node(file.store(), Some(*node))
                .is_some_and(|source_file| source_file == file.as_node())
        })
}

// RenameInfo represents the result of a rename validation check.
// It is used by the `textDocument/prepareRename` LSP handler.
#[derive(Clone, Debug, Default)]
pub struct RenameInfo {
    pub can_rename: bool,
    pub localized_error_message: String,
    pub display_name: String,
    pub trigger_span: lsproto::Range,
    pub file_to_rename: String,
    pub new_file_name: String,
}

impl LanguageService<'_> {
    pub fn provide_rename(
        &self,
        ctx: &core::Context,
        params: &lsproto::RenameParams,
        orchestrator: Option<&dyn CrossProjectOrchestrator>,
    ) -> Result<lsproto::RenameResponse, core::Error> {
        handle_cross_project(
            self,
            ctx,
            params.clone(),
            orchestrator,
            |ls, ctx, params, data, options| {
                ls.symbol_and_entries_to_rename(ctx, params, data, options)
            },
            |results| crate::crossproject::combine_rename_response(results),
            true,  /*is_rename*/
            false, /*implementations*/
            SymbolEntryTransformOptions::default(),
        )
    }

    pub fn get_rename_info(
        &self,
        ctx: &core::Context,
        new_name: &str,
        document_uri: lsproto::DocumentUri,
        position: lsproto::Position,
    ) -> Result<RenameInfo, core::Error> {
        let (program, source_file) = self.get_program_and_file(document_uri);
        let pos = self
            .converters
            .line_and_character_to_position(source_file, position) as i32;

        let Some(mut node) = astnav::get_touching_property_name(source_file, pos) else {
            return Ok(get_rename_info_error(
                ctx,
                &*diagnostics::YOU_CANNOT_RENAME_THIS_ELEMENT,
            ));
        };
        node = get_adjusted_location(
            source_file.store(),
            node,
            true, /*for_rename*/
            Some(source_file),
        );

        if node_is_eligible_for_rename(source_file.store(), node) {
            if let Some(rename_info) =
                self.get_rename_info_for_node(ctx, new_name, node, source_file, program)?
            {
                return Ok(rename_info);
            }
        }
        Ok(get_rename_info_error(
            ctx,
            &*diagnostics::YOU_CANNOT_RENAME_THIS_ELEMENT,
        ))
    }

    pub(crate) fn symbol_and_entries_to_rename(
        &self,
        ctx: &core::Context,
        params: lsproto::RenameParams,
        data: SymbolAndEntriesData,
        _options: SymbolEntryTransformOptions,
    ) -> Result<lsproto::RenameResponse, core::Error> {
        let Some(original_node) = data.original_node else {
            return Ok(lsproto::RenameResponse::default());
        };

        let program = self.get_program();

        // Defense-in-depth: validate rename eligibility even if the client skipped prepareRename.
        // Use getRenameInfoForNode directly with the already-resolved node to avoid
        // re-resolving the position and polluting state baselines.
        let source_file = source_file_for_node(program, &original_node)
            .expect("rename original node should belong to a program source file");
        if !node_is_eligible_for_rename(source_file.store(), original_node) {
            return Ok(lsproto::RenameResponse::default());
        }
        if self
            .get_rename_info_for_node(ctx, &params.new_name, original_node, source_file, program)?
            .as_ref()
            .is_none_or(|info| !info.can_rename)
        {
            return Ok(lsproto::RenameResponse::default());
        }

        let entries = data
            .symbols_and_entries
            .iter()
            .flat_map(|s: &SymbolAndEntries| s.references.iter().cloned())
            .collect::<Vec<_>>();
        program.with_type_checker_for_file_using(
            compiler::CheckerAccess::context(ctx),
            source_file,
            |ch| {
                let mut changes: HashMap<lsproto::DocumentUri, Vec<lsproto::TextEdit>> =
                    HashMap::new();

                let quote_preference =
                    lsutil::get_quote_preference(source_file, &self.user_preferences());

                for entry in &entries {
                    let uri = self.get_file_name_of_entry(entry);
                    let entry_store = ch
                        .source_file_store(entry.node)
                        .expect("rename entry should belong to a checker source file");
                    if self.user_preferences().allow_rename_of_import_path != core::Tristate::True
                        && ast::is_string_literal_like(entry_store, entry.node)
                        && ast::try_get_import_from_module_specifier(entry_store, &entry.node)
                            .is_some()
                    {
                        continue;
                    }
                    let text_edit = lsproto::TextEdit {
                        range: self.get_range_of_entry(entry),
                        new_text: self.get_text_for_rename(
                            original_node,
                            entry,
                            &params.new_name,
                            ch,
                            quote_preference,
                        ),
                    };
                    changes.entry(uri).or_default().push(text_edit);
                }
                Ok(lsproto::RenameResponse {
                    workspace_edit: Some(lsproto::WorkspaceEdit {
                        changes: Some(changes),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
            },
        )
    }

    // getRenameInfoForNode performs detailed validation for a rename operation on a specific node.
    pub(crate) fn get_rename_info_for_node(
        &self,
        ctx: &core::Context,
        new_name: &str,
        node: ast::Node,
        source_file: &ast::SourceFile,
        program: &compiler::Program,
    ) -> Result<Option<RenameInfo>, core::Error> {
        program.with_type_checker_for_file_using(
            compiler::CheckerAccess::context(ctx),
            source_file,
            |ch| {
                let symbol = ch.get_symbol_at_location_public(node);
                if symbol.is_none() {
                    if ast::is_string_literal_like(source_file.store(), node) {
                        // Allow renaming of string literal types with contextual string literal types
                        if has_contextual_or_ancestor_string_literal_type(
                            source_file.store(),
                            node,
                            ch,
                        ) {
                            return Ok(Some(get_rename_info_success(
                                node,
                                source_file,
                                source_file.store().text(node),
                                &self.converters,
                            )));
                        }
                    } else if ast::is_label_name(source_file.store(), node) {
                        let name = source_file.store().text(node);
                        return Ok(Some(get_rename_info_success(
                            node,
                            source_file,
                            name,
                            &self.converters,
                        )));
                    }
                    return Ok(None);
                }
                let symbol = symbol.unwrap();

                let Some(symbol_name) = ch.symbol_name_public(symbol) else {
                    return Ok(None);
                };
                let symbol_declarations = ch.collect_symbol_declarations_public(symbol);

                // Only allow a symbol to be renamed if it actually has at least one declaration.
                if symbol_declarations.is_empty() {
                    return Ok(None);
                }

                if let Some(msg) =
                    self.rename_blocked_reason(source_file, node, symbol, ch, program)
                {
                    return Ok(Some(get_rename_info_error(ctx, msg)));
                }

                if ast::is_string_literal_like(source_file.store(), node)
                    && ast::try_get_import_from_module_specifier(source_file.store(), node)
                        .is_some()
                {
                    if self
                        .user_preferences()
                        .allow_rename_of_import_path
                        .is_true()
                    {
                        let result = self.get_rename_info_for_module(
                            ctx,
                            new_name,
                            &node,
                            source_file,
                            &symbol_declarations,
                        );
                        return Ok(result);
                    }
                    return Ok(None);
                }

                let result =
                    get_rename_info_success(node, source_file, symbol_name, &self.converters);
                Ok(Some(result))
            },
        )
    }

    // renameBlockedReason returns a non-nil diagnostic message if the rename should be blocked
    // because the symbol is a library definition, a default keyword, or would cross node_modules boundaries.
    pub(crate) fn rename_blocked_reason<'a>(
        &self,
        source_file: &ast::SourceFile,
        node: ast::Node,
        symbol: ast::SymbolIdentity,
        ch: &mut checker::Checker<'a, '_>,
        program: &compiler::Program,
    ) -> Option<&'static diagnostics::Message> {
        for declaration in ch.collect_symbol_declarations_public(symbol) {
            if is_defined_in_library_file(program, declaration) {
                return Some(
                    &*diagnostics::YOU_CANNOT_RENAME_ELEMENTS_THAT_ARE_DEFINED_IN_THE_STANDARD_TYPE_SCRIPT_LIBRARY,
                );
            }
        }

        // Cannot rename `default` as in `import { default as foo } from "./someModule"`
        if ast::is_identifier(source_file.store(), node)
            && source_file.store().text(node) == "default"
            && ch
                .symbol_parent_public(symbol)
                .and_then(|parent| ch.symbol_flags_public(parent))
                .is_some_and(|flags| flags & ast::SYMBOL_FLAGS_MODULE != 0)
        {
            return Some(&*diagnostics::YOU_CANNOT_RENAME_THIS_ELEMENT);
        }

        if let Some(msg) =
            would_rename_in_other_node_modules(source_file, symbol, ch, &self.user_preferences())
        {
            return Some(msg);
        }

        None
    }

    // getRenameInfoForModule handles rename validation for module specifiers.
    pub(crate) fn get_rename_info_for_module(
        &self,
        ctx: &core::Context,
        new_name: &str,
        specifier: &ast::StringLiteralLike,
        source_file: &ast::SourceFile,
        module_declarations: &[ast::Node],
    ) -> Option<RenameInfo> {
        let specifier_text = source_file.store().text(*specifier);
        if !tspath::is_external_module_name_relative(&specifier_text) {
            return Some(get_rename_info_error(
                ctx,
                &*diagnostics::YOU_CANNOT_RENAME_A_MODULE_VIA_A_GLOBAL_IMPORT,
            ));
        }
        if !client_supports_document_changes(ctx)
            || !client_supports_rename_resource_operations(ctx)
        {
            return Some(get_rename_info_error(
                ctx,
                &*diagnostics::FILE_RENAME_IS_NOT_SUPPORTED_BY_THE_EDITOR,
            ));
        }

        let module_source_file = module_declarations
            .iter()
            .find_map(|declaration| source_file_for_node(self.get_program(), declaration))?;
        let file_name = module_source_file.file_name();
        let mut without_index = String::new();
        if !specifier_text.ends_with("/index") && !specifier_text.ends_with("/index.js") {
            let candidate = tspath::remove_file_extension(&file_name);
            if let Some(trimmed) = candidate.strip_suffix("/index") {
                without_index = trimmed.to_string();
            }
        }

        let mut display_name = file_name.clone();
        if !without_index.is_empty() {
            display_name = without_index;
        }
        let new_file_name =
            self.get_new_file_name_for_module_rename(&display_name, &specifier_text, new_name);

        // Span should only be the last component of the path. + 1 to account for the quote character.
        let index_after_last_slash = specifier_text.rfind('/').map(|i| i + 1).unwrap_or(0);
        let start = source_file.store().loc(*specifier).pos() + 1 + index_after_last_slash as i32;
        let length = specifier_text.len() as i32 - index_after_last_slash as i32;

        Some(RenameInfo {
            can_rename: true,
            display_name: specifier_text[index_after_last_slash..].to_string(),
            trigger_span: self
                .converters
                .to_lsp_range(source_file, core::new_text_range(start, start + length)),
            file_to_rename: display_name,
            new_file_name,
            ..Default::default()
        })
    }

    // Adjust the new name based on the old path that an import specifier resolves to.
    // For example, if specifier "a.js" resolves to file a.ts, renaming "a.js" -> "b.js" should mean file rename a.ts -> b.ts.
    pub(crate) fn get_new_file_name_for_module_rename(
        &self,
        old_path: &str,
        specifier_text: &str,
        new_name: &str,
    ) -> String {
        let mut new_path =
            tspath::combine_paths(&tspath::get_directory_path(old_path), &[new_name]);
        let ignore_case = !self.use_case_sensitive_file_names();
        let old_ext = if tspath::is_declaration_file_name(old_path) {
            tspath::get_declaration_file_extension(old_path)
        } else {
            tspath::get_any_extension_from_path(old_path, &[], ignore_case)
        };
        if !tspath::has_extension(&new_path) {
            new_path.push_str(&old_ext);
        } else if tspath::get_any_extension_from_path(&new_path, &[], ignore_case)
            == tspath::get_any_extension_from_path(specifier_text, &[], ignore_case)
        {
            new_path = tspath::change_any_extension(&new_path, &old_ext, None, ignore_case);
        }
        new_path
    }

    pub(crate) fn get_text_for_rename(
        &self,
        original_node: ast::Node,
        entry: &ReferenceEntry,
        new_text: &str,
        ch: &mut checker::Checker,
        quote_preference: lsutil::QuotePreference,
    ) -> String {
        if entry.kind != ENTRY_KIND_RANGE
            && (ast::is_identifier(
                ch.source_file_store(original_node)
                    .expect("rename original node should belong to a checker source file"),
                original_node,
            ) || ast::is_string_literal_like(
                ch.source_file_store(original_node)
                    .expect("rename original node should belong to a checker source file"),
                original_node,
            ))
        {
            let entry_store = ch
                .source_file_store(entry.node)
                .expect("rename entry should belong to a checker source file");
            let original_store = ch
                .source_file_store(original_node)
                .expect("rename original node should belong to a checker source file");
            let node = ast::get_reparsed_node_for_node(&entry_store, &entry.node);
            let kind = entry.kind;
            let parent = entry_store.parent(node).unwrap();
            let name = original_store.text(original_node);
            let is_shorthand_assignment =
                ast::is_shorthand_property_assignment(&entry_store, parent);
            match () {
                _ if is_shorthand_assignment
                    || (is_object_binding_element_without_property_name(&entry_store, &parent)
                        && entry_store.name(parent).is_some_and(|name| name == node)
                        && entry_store.dot_dot_dot_token(parent).is_none()) =>
                {
                    if kind == ENTRY_KIND_SEARCHED_LOCAL_FOUND_PROPERTY {
                        return format!("{name}: {new_text}");
                    }
                    if kind == ENTRY_KIND_SEARCHED_PROPERTY_FOUND_LOCAL {
                        return format!("{new_text}: {name}");
                    }
                    // In `const o = { x }; o.x`, symbolAtLocation at `x` in `{ x }` is the property symbol.
                    // For a binding element `const { x } = o;`, symbolAtLocation at `x` is the property symbol.
                    if is_shorthand_assignment {
                        let grand_parent = entry_store.parent(parent).unwrap();
                        if ast::is_object_literal_expression(&entry_store, grand_parent)
                            && entry_store
                                .parent(grand_parent)
                                .as_ref()
                                .is_some_and(|parent| {
                                    ast::is_binary_expression(&entry_store, *parent)
                                })
                            && entry_store
                                .left(entry_store.parent(grand_parent).unwrap())
                                .is_some_and(|left| {
                                    ast::is_module_exports_access_expression(&entry_store, left)
                                })
                        {
                            return format!("{name}: {new_text}");
                        }
                        return format!("{new_text}: {name}");
                    }
                    return format!("{name}: {new_text}");
                }
                _ if ast::is_import_specifier(&entry_store, parent)
                    && entry_store.property_name(parent).is_none() =>
                {
                    // If the original symbol was using this alias, just rename the alias.
                    let original_parent = original_store.parent(original_node).unwrap();
                    let original_symbol =
                        if ast::is_export_specifier(&original_store, original_parent) {
                            ch.get_export_specifier_local_target_symbol_public(original_parent)
                        } else {
                            ch.get_symbol_at_location_public(original_node)
                        };
                    if original_symbol.is_some_and(|original_symbol| {
                        ch.collect_symbol_declarations_public(original_symbol)
                            .iter()
                            .any(|decl| *decl == parent)
                    }) {
                        return format!("{name} as {new_text}");
                    }
                    return new_text.to_string();
                }
                _ if ast::is_export_specifier(&entry_store, parent)
                    && entry_store.property_name(parent).is_none() =>
                {
                    // If the symbol for the node is same as declared node symbol use prefix text
                    if original_node == entry.node
                        || ch
                            .get_symbol_at_location_public(original_node)
                            .zip(ch.get_symbol_at_location_public(entry.node))
                            .is_some_and(|(left, right)| left == right)
                    {
                        return format!("{name} as {new_text}");
                    }
                    return format!("{new_text} as {name}");
                }
                _ => {}
            }
        }

        // If the node is a numerical indexing literal, then add quotes around the property access.
        if entry.kind != ENTRY_KIND_RANGE
            && ast::is_numeric_literal(
                ch.source_file_store(entry.node)
                    .expect("rename entry should belong to a checker source file"),
                entry.node,
            )
            && ch
                .source_file_store(entry.node)
                .expect("rename entry should belong to a checker source file")
                .parent(entry.node)
                .as_ref()
                .is_some_and(|parent| {
                    ast::is_access_expression(
                        ch.source_file_store(entry.node)
                            .expect("rename entry should belong to a checker source file"),
                        *parent,
                    )
                })
        {
            let quote = get_quote_from_preference(quote_preference);
            return format!("{quote}{new_text}{quote}");
        }

        new_text.to_string()
    }
}

pub(crate) fn node_is_eligible_for_rename(store: &ast::AstStore, node: ast::Node) -> bool {
    match store.kind(node) {
        ast::Kind::Identifier
        | ast::Kind::PrivateIdentifier
        | ast::Kind::StringLiteral
        | ast::Kind::NoSubstitutionTemplateLiteral
        | ast::Kind::ThisKeyword => true,
        ast::Kind::NumericLiteral => {
            is_literal_name_of_property_declaration_or_index_access(store, node)
        }
        _ => false,
    }
}

// isDefinedInLibraryFile checks if a declaration is from a default library file (e.g., lib.d.ts).
pub(crate) fn is_defined_in_library_file(
    program: &compiler::Program,
    declaration: ast::Node,
) -> bool {
    let decl_source_file = source_file_for_node(program, &declaration)
        .expect("declaration should belong to a program source file");
    program.is_source_file_default_library(decl_source_file.path())
        && tspath::is_declaration_file_name(&decl_source_file.file_name())
}

// wouldRenameInOtherNodeModules checks if renaming the symbol would affect node_modules.
pub(crate) fn would_rename_in_other_node_modules<'a>(
    original_file: &ast::SourceFile,
    symbol: ast::SymbolIdentity,
    ch: &mut checker::Checker<'a, '_>,
    preferences: &lsutil::UserPreferences,
) -> Option<&'static diagnostics::Message> {
    let mut declarations = ch.collect_symbol_declarations_public(symbol);
    if !preferences.use_aliases_for_rename.is_true()
        && ch
            .symbol_flags_public(symbol)
            .is_some_and(|flags| flags & ast::SYMBOL_FLAGS_ALIAS != 0)
    {
        let has_import_specifier = declarations.iter().any(|declaration| {
            ch.source_file_store(*declaration).is_some_and(|store| {
                ast::is_import_specifier(store, *declaration)
                    && store.property_name(*declaration).is_none()
            })
        });
        if has_import_specifier {
            let aliased = ch.skip_alias_public(symbol);
            declarations = aliased.map_or_else(Vec::new, |aliased| {
                ch.collect_symbol_declarations_public(aliased)
            });
        }
    }

    if declarations.is_empty() {
        return None;
    }

    let original_package =
        module::parse_node_module_from_path(&original_file.file_name(), false /*is_folder*/);
    if original_package.is_empty() {
        // Original source file is not in node_modules.
        for declaration in declarations {
            let store = ch
                .source_file_store(declaration)
                .expect("declaration should belong to a checker source file");
            let source_file = ast::get_source_file_of_node(store, Some(declaration)).unwrap();
            if symbols::is_inside_node_modules(&store.as_source_file(source_file).file_name()) {
                return Some(
                    &*diagnostics::YOU_CANNOT_RENAME_ELEMENTS_THAT_ARE_DEFINED_IN_A_NODE_MODULES_FOLDER,
                );
            }
        }
        return None;
    }

    // Original source file is in node_modules.
    for declaration in declarations {
        let store = ch
            .source_file_store(declaration)
            .expect("declaration should belong to a checker source file");
        let source_file = ast::get_source_file_of_node(store, Some(declaration)).unwrap();
        let decl_package = module::parse_node_module_from_path(
            &store.as_source_file(source_file).file_name(),
            false, /*is_folder*/
        );
        if !decl_package.is_empty() && decl_package != original_package {
            return Some(
                &*diagnostics::YOU_CANNOT_RENAME_ELEMENTS_THAT_ARE_DEFINED_IN_ANOTHER_NODE_MODULES_FOLDER,
            );
        }
    }
    None
}

pub fn client_supports_will_rename_files(ctx: &core::Context) -> bool {
    lsproto::get_client_capabilities(ctx)
        .workspace
        .file_operations
        .will_rename
}

pub fn client_supports_document_changes(ctx: &core::Context) -> bool {
    lsproto::get_client_capabilities(ctx)
        .workspace
        .workspace_edit
        .document_changes
}

pub(crate) fn client_supports_rename_resource_operations(ctx: &core::Context) -> bool {
    lsproto::get_client_capabilities(ctx)
        .workspace
        .workspace_edit
        .resource_operations
        .contains(&lsproto::ResourceOperationKind::Rename)
}

pub(crate) fn get_quote_from_preference(quote_preference: lsutil::QuotePreference) -> &'static str {
    if quote_preference == lsutil::QuotePreference::Single {
        return "'";
    }
    "\""
}

pub(crate) fn get_rename_info_error(
    _ctx: &core::Context,
    message: &'static diagnostics::Message,
) -> RenameInfo {
    RenameInfo {
        can_rename: false,
        localized_error_message: message.localize(locale::und(), vec![]),
        ..Default::default()
    }
}

pub(crate) fn get_rename_info_success(
    node: ast::Node,
    source_file: &ast::SourceFile,
    display_name: String,
    converters: &lsconv::Converters,
) -> RenameInfo {
    let mut start = astnav::get_start_of_node(node, source_file);
    let mut end = source_file.store().loc(node).end();
    if ast::is_string_literal_like(source_file.store(), node) {
        // Exclude the quotes
        start += 1;
        end -= 1;
    }
    RenameInfo {
        can_rename: true,
        display_name,
        trigger_span: converters.to_lsp_range(source_file, core::new_text_range(start, end)),
        ..Default::default()
    }
}

fn is_object_binding_element_without_property_name(
    store: &ast::AstStore,
    node: &ast::Node,
) -> bool {
    ast::is_binding_element(store, *node) && store.property_name(*node).is_none()
}
