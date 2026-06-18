use ts_ast as ast;
use ts_astnav as astnav;
use ts_checker as checker;
use ts_collections as collections;
use ts_compiler as compiler;
use ts_core as core;
use ts_lsproto as lsproto;
use ts_scanner as scanner;
use ts_stringutil as stringutil;

use crate::crossproject::{combine_implementations, combine_references, handle_cross_project};
use crate::importTracker::ImpExpKind;
use crate::lsconv;
use crate::rename::node_is_eligible_for_rename;
use crate::utilities::{
    get_adjusted_location, get_local_symbol_for_export_specifier, get_meaning_from_location,
    get_target_label, is_expression_of_external_module_import_equals_declaration,
    is_jump_statement_target, is_label_of_labeled_statement,
    is_literal_name_of_property_declaration_or_index_access, is_name_of_module_declaration,
    is_readonly_type_operator, is_type_keyword, source_node_symbol_from_program,
};
use crate::{CrossProjectOrchestrator, LanguageService};

fn source_file_for_node_from_files<'a>(
    source_files: &[&'a ast::SourceFile],
    node: ast::Node,
) -> Option<&'a ast::SourceFile> {
    source_files
        .iter()
        .copied()
        .find(|source_file| source_file.store().store_id() == node.store_id())
}

fn store_for_node_from_files<'a>(
    source_files: &[&'a ast::SourceFile],
    node: ast::Node,
) -> Option<&'a ast::AstStore> {
    source_file_for_node_from_files(source_files, node).map(|source_file| source_file.store())
}

fn node_modifier_flags(store: &ast::AstStore, node: ast::Node) -> ast::ModifierFlags {
    store
        .modifiers(node)
        .map(|modifiers| modifiers.modifier_flags())
        .unwrap_or(ast::ModifierFlags::NONE)
}

fn symbol_and_entries_to_references_callback(
    ls: &LanguageService<'_>,
    ctx: &core::Context,
    params: lsproto::ReferenceParams,
    data: SymbolAndEntriesData,
    options: SymbolEntryTransformOptions,
) -> Result<lsproto::ReferencesResponse, core::Error> {
    ls.symbol_and_entries_to_references(ctx, params, data, options)
}

fn symbol_and_entries_to_implementations_callback(
    ls: &LanguageService<'_>,
    ctx: &core::Context,
    params: lsproto::ImplementationParams,
    data: SymbolAndEntriesData,
    options: SymbolEntryTransformOptions,
) -> Result<lsproto::ImplementationResponse, core::Error> {
    ls.symbol_and_entries_to_implementations(ctx, params, data, options)
}

// === types for settings ===
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ReferenceUse {
    #[default]
    None = 0,
    Other = 1,
    References = 2,
    Rename = 3,
}

pub const REFERENCE_USE_NONE: ReferenceUse = ReferenceUse::None;
pub const REFERENCE_USE_OTHER: ReferenceUse = ReferenceUse::Other;
pub const REFERENCE_USE_REFERENCES: ReferenceUse = ReferenceUse::References;
pub const REFERENCE_USE_RENAME: ReferenceUse = ReferenceUse::Rename;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RefOptions {
    pub find_in_strings: bool,
    pub find_in_comments: bool,
    pub use_: ReferenceUse, // other, references, rename
    pub implementations: bool,
    pub use_aliases_for_rename: bool, // renamed from providePrefixAndSuffixTextForRename. default: true
}

// === types for results ===
#[derive(Clone, Default)]
pub(crate) struct RefInfo<'a> {
    pub file: Option<&'a ast::SourceFile>,
    pub file_name: String,
    pub reference: Option<&'a ast::FileReference>,
    pub unverified: bool,
}

#[derive(Clone, Default)]
pub(crate) struct SymbolAndEntries {
    pub definition: Option<Definition>,
    pub references: Vec<ReferenceEntry>,
}

pub(crate) fn new_symbol_and_entries(
    kind: DefinitionKind,
    node: Option<ast::Node>,
    symbol: Option<ast::SymbolIdentity>,
    references: Vec<ReferenceEntry>,
) -> SymbolAndEntries {
    SymbolAndEntries {
        definition: Some(Definition {
            kind,
            node,
            symbol,
            symbol_declarations: Vec::new(),
            triple_slash_file_ref: None,
        }),
        references,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DefinitionKind {
    #[default]
    Symbol = 0,
    Label = 1,
    Keyword = 2,
    This = 3,
    String = 4,
    TripleSlashReference = 5,
}

#[derive(Clone, Default)]
pub(crate) struct Definition {
    pub kind: DefinitionKind,
    pub(crate) symbol: Option<ast::SymbolIdentity>,
    pub(crate) symbol_declarations: Vec<ast::Node>,
    pub node: Option<ast::Node>,
    pub triple_slash_file_ref: Option<TripleSlashDefinition>,
}

#[derive(Clone, Default)]
pub(crate) struct TripleSlashDefinition {
    pub reference: Option<&'static ast::FileReference>,
    pub file: Option<&'static ast::SourceFile>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum EntryKind {
    #[default]
    None = 0,
    Range = 1,
    Node = 2,
    StringLiteral = 3,
    SearchedLocalFoundProperty = 4,
    SearchedPropertyFoundLocal = 5,
}

pub const ENTRY_KIND_NONE: EntryKind = EntryKind::None;
pub const ENTRY_KIND_RANGE: EntryKind = EntryKind::Range;
pub const ENTRY_KIND_NODE: EntryKind = EntryKind::Node;
pub const ENTRY_KIND_STRING_LITERAL: EntryKind = EntryKind::StringLiteral;
pub const ENTRY_KIND_SEARCHED_LOCAL_FOUND_PROPERTY: EntryKind =
    EntryKind::SearchedLocalFoundProperty;
pub const ENTRY_KIND_SEARCHED_PROPERTY_FOUND_LOCAL: EntryKind =
    EntryKind::SearchedPropertyFoundLocal;

#[derive(Clone)]
pub(crate) struct ReferenceEntry {
    pub kind: EntryKind,
    pub node: ast::Node,
    pub context: Option<ast::Node>,
    pub file_name: String,
    pub text_range: Option<core::TextRange>,
    pub lsp_range: Option<lsproto::Location>,
}

impl SymbolAndEntries {
    pub(crate) fn can_use_definition_symbol(&self) -> bool {
        let Some(definition) = &self.definition else {
            return false;
        };

        match definition.kind {
            DefinitionKind::Symbol | DefinitionKind::This => definition.symbol.is_some(),
            DefinitionKind::TripleSlashReference => false,
            _ => false,
        }
    }
}

impl LanguageService<'_> {
    pub(crate) fn get_range_of_entry(&self, entry: &ReferenceEntry) -> lsproto::Range {
        self.resolve_entry(entry).lsp_range.unwrap().range
    }

    pub(crate) fn get_file_name_of_entry(&self, entry: &ReferenceEntry) -> lsproto::DocumentUri {
        self.resolve_entry(entry).lsp_range.unwrap().uri
    }

    pub(crate) fn get_location_of_entry(&self, entry: &ReferenceEntry) -> lsproto::Location {
        self.resolve_entry(entry).lsp_range.unwrap()
    }

    pub(crate) fn resolve_entry(&self, entry: &ReferenceEntry) -> ReferenceEntry {
        let mut entry = entry.clone();
        if entry.text_range.is_none() {
            let source_file = self
                .get_program()
                .get_parsed_source_files_refs()
                .into_iter()
                .find(|file| file.store().store_id() == entry.node.store_id())
                .expect("reference entry node should belong to a program source file");
            let text_range = get_range_of_node(entry.node, source_file, None);
            entry.text_range = Some(text_range);
            entry.file_name = source_file.file_name().to_string();
        }
        if entry.lsp_range.is_none() {
            let location = self.get_mapped_location(&entry.file_name, entry.text_range.unwrap());
            entry.lsp_range = Some(location);
        }
        entry
    }
}

pub(crate) fn new_node_entry_with_kind(node: ast::Node, kind: EntryKind) -> ReferenceEntry {
    let mut entry = new_node_entry(node);
    entry.kind = kind;
    entry
}

pub(crate) fn new_node_entry(node: ast::Node) -> ReferenceEntry {
    // creates nodeEntry with `kind == entryKindNode`
    ReferenceEntry {
        kind: EntryKind::Node,
        node,
        context: None,
        file_name: String::new(),
        text_range: None,
        lsp_range: None,
    }
}

pub(crate) fn get_context_node_for_node_entry(node: ast::Node) -> Option<ast::Node> {
    let _ = node;
    None
}

pub(crate) fn get_context_node(node: Option<ast::Node>) -> Option<ast::Node> {
    node
}

// utils
impl LanguageService<'_> {
    pub(crate) fn get_lsp_range_of_node(
        &self,
        node: ast::Node,
        source_file: &ast::SourceFile,
        end_node: Option<ast::Node>,
    ) -> lsproto::Range {
        let text_range = get_range_of_node(node, source_file, end_node);
        self.create_lsp_range_from_bounds(text_range.pos(), text_range.end(), source_file)
    }
}

pub(crate) fn get_range_of_node(
    node: ast::Node,
    source_file: &ast::SourceFile,
    end_node: Option<ast::Node>,
) -> core::TextRange {
    let mut start = scanner::get_token_pos_of_node(&node, source_file, false);
    let store = source_file.store();
    let mut end = store.loc(end_node.unwrap_or(node)).end();
    if ast::is_string_literal_like(store, node) && (end - start as i32) > 2 {
        if end_node.is_some() {
            panic!("end_node is not nil for string_literal_like");
        }
        start += 1;
        end -= 1;
    }
    if end_node.is_some_and(|end_node| ast::is_case_block(store, end_node)) {
        end = store.loc(end_node.unwrap()).pos();
    }
    core::new_text_range(start as i32, end)
}

pub(crate) fn is_valid_reference_position(
    store: &ast::AstStore,
    node: ast::Node,
    search_symbol_name: &str,
) -> bool {
    match store.kind(node) {
        ast::Kind::PrivateIdentifier => store.text(node).len() == search_symbol_name.len(),
        ast::Kind::Identifier => store.text(node).len() == search_symbol_name.len(),
        ast::Kind::NoSubstitutionTemplateLiteral | ast::Kind::StringLiteral => {
            store.text(node).len() == search_symbol_name.len()
                && (is_literal_name_of_property_declaration_or_index_access(store, node)
                    || is_name_of_module_declaration(store, node)
                    || is_expression_of_external_module_import_equals_declaration(store, node)
                    || store.parent(node).is_some_and(|parent| {
                        ast::bindable_object_define_property_call_property_name_argument(
                            store, parent,
                        )
                        .is_some_and(|property_name| property_name == node)
                    })
                    || store
                        .parent(node)
                        .as_ref()
                        .is_some_and(|parent| ast::is_import_or_export_specifier(store, *parent)))
        }
        ast::Kind::NumericLiteral => {
            is_literal_name_of_property_declaration_or_index_access(store, node)
                && store.text(node).len() == search_symbol_name.len()
        }
        ast::Kind::DefaultKeyword => "default".len() == search_symbol_name.len(),
        _ => false,
    }
}

pub(crate) fn is_for_rename_with_prefix_and_suffix_text(options: RefOptions) -> bool {
    options.use_ == ReferenceUse::Rename && options.use_aliases_for_rename
}

pub(crate) fn skip_past_export_or_import_specifier_or_union(
    store: &ast::AstStore,
    symbol: ast::SymbolIdentity,
    node: Option<&ast::Node>,
    checker: &mut checker::Checker<'_, '_>,
    use_local_symbol_for_export_specifier: bool,
) -> Option<ast::SymbolIdentity> {
    let node = node?;
    let parent = store.parent(*node);
    if parent
        .as_ref()
        .is_some_and(|parent| store.kind(*parent) == ast::Kind::ExportSpecifier)
        && use_local_symbol_for_export_specifier
    {
        let local_symbol =
            get_local_symbol_for_export_specifier(store, *node, symbol, parent.unwrap(), checker);
        return Some(local_symbol);
    }
    let symbol_declarations = checker.collect_symbol_declarations_public(symbol);
    let symbol_flags = checker.symbol_flags_public(symbol)?;
    for declaration in &symbol_declarations {
        let Some(decl_parent) = store.parent(*declaration) else {
            if symbol_flags & (ast::SYMBOL_FLAGS_TRANSIENT | ast::SYMBOL_FLAGS_MODULE_EXPORTS) != 0
            {
                return None;
            }
            panic!(
                "Unexpected symbol at {:?}: {}",
                store.kind(*node),
                checker.symbol_name_public(symbol).unwrap_or_default()
            );
        };
        if store.kind(decl_parent) == ast::Kind::TypeLiteral
            && store
                .parent(decl_parent)
                .as_ref()
                .is_some_and(|parent| store.kind(*parent) == ast::Kind::UnionType)
        {
            return None;
        }
    }
    None
}

// === functions on (*ls) ===
#[derive(Clone, Debug, Default)]
pub(crate) struct Position {
    pub uri: lsproto::DocumentUri,
    pub pos: lsproto::Position,
}

impl lsproto::HasTextDocumentUri for Position {
    fn text_document_uri(&self) -> lsproto::DocumentUri {
        self.uri.clone()
    }
}

impl lsproto::HasTextDocumentPosition for Position {
    fn text_document_position(&self) -> lsproto::Position {
        self.pos
    }
}

#[derive(Clone)]
pub(crate) struct NonLocalDefinition {
    pub position: Position,
    pub get_source_position: fn() -> Option<Position>,
    pub get_generated_position: fn() -> Option<Position>,
}

pub(crate) fn get_file_and_start_pos_from_declaration<'a>(
    source_files: &[&'a ast::SourceFile],
    declaration: ast::Node,
) -> (&'a ast::SourceFile, core::TextPos) {
    let file = source_file_for_node_from_files(source_files, declaration)
        .expect("declaration should belong to a program source file");
    let store = file.store();
    let name = ast::get_name_of_declaration(store, Some(declaration)).unwrap_or(declaration);
    let text_range = get_range_of_node(name, file, None);
    (file, text_range.pos())
}

impl LanguageService<'_> {
    pub(crate) fn get_non_local_definition(
        &self,
        ctx: &core::Context,
        entry: &SymbolAndEntries,
    ) -> Result<Option<NonLocalDefinition>, core::Error> {
        if !entry.can_use_definition_symbol() {
            return Ok(None);
        }

        let program = self.get_program();
        let source_files_storage = program.source_files();
        let source_files: Vec<&ast::SourceFile> = source_files_storage.iter().collect();
        let Some(_symbol) = entry
            .definition
            .as_ref()
            .and_then(|definition| definition.symbol.clone())
        else {
            return Ok(None);
        };
        let declarations = entry
            .definition
            .as_ref()
            .map(|definition| definition.symbol_declarations.clone())
            .unwrap_or_default();
        if declarations.is_empty() {
            return Ok(None);
        }
        let checker_file = source_file_for_node_from_files(&source_files, declarations[0])
            .expect("declaration should belong to a program source file");
        program.with_type_checker_for_file_using(
            compiler::CheckerAccess::context(ctx),
            checker_file,
            |checker| {
                let mut result = None;
                for declaration in &declarations {
                    let source_file = source_file_for_node_from_files(&source_files, *declaration)
                        .expect("declaration should belong to a program source file");
                    if is_definition_visible(source_file.store(), checker, declaration) {
                        let (file, start_pos) =
                            get_file_and_start_pos_from_declaration(&source_files, *declaration);
                        let file_name = file.file_name().to_string();
                        result = Some(NonLocalDefinition {
                            position: Position {
                                uri: lsconv::file_name_to_document_uri(&file_name),
                                pos: self
                                    .converters
                                    .position_to_line_and_character(file, start_pos),
                            },
                            get_source_position: || None,
                            get_generated_position: || None,
                        });
                        break;
                    }
                }
                Ok(result)
            },
        )
    }

    pub(crate) fn for_each_original_definition_location(
        &self,
        _ctx: &core::Context,
        entry: &SymbolAndEntries,
        mut cb: impl FnMut(lsproto::DocumentUri, lsproto::Position),
    ) {
        if !entry.can_use_definition_symbol() {
            return;
        }

        if let Some(declarations) = entry
            .definition
            .as_ref()
            .map(|definition| definition.symbol_declarations.as_slice())
        {
            let program = self.get_program();
            let source_files_storage = program.source_files();
            let source_files: Vec<&ast::SourceFile> = source_files_storage.iter().collect();
            for declaration in declarations {
                let (file, start_pos) =
                    get_file_and_start_pos_from_declaration(&source_files, *declaration);
                cb(
                    lsconv::file_name_to_document_uri(&file.file_name()),
                    self.converters
                        .position_to_line_and_character(file, start_pos),
                );
            }
        }
    }
}

pub(crate) fn is_definition_visible(
    store: &ast::AstStore,
    checker: &mut checker::Checker<'_, '_>,
    declaration: &ast::Node,
) -> bool {
    if checker.is_declaration_visible_public(*declaration) {
        return true;
    }
    let Some(parent) = store.parent(*declaration) else {
        return false;
    };

    if ast::has_initializer(store, &parent)
        && store
            .initializer(parent)
            .is_some_and(|initializer| initializer == *declaration)
    {
        return is_definition_visible(store, checker, &parent);
    }

    match store.kind(*declaration) {
        ast::Kind::PropertyDeclaration
        | ast::Kind::GetAccessor
        | ast::Kind::SetAccessor
        | ast::Kind::MethodDeclaration => {
            if ast::has_modifier(store, declaration, ast::ModifierFlags::PRIVATE)
                || store
                    .name(*declaration)
                    .as_ref()
                    .is_some_and(|name| ast::is_private_identifier(store, *name))
            {
                false
            } else {
                is_definition_visible(store, checker, &parent)
            }
        }
        ast::Kind::Constructor
        | ast::Kind::PropertyAssignment
        | ast::Kind::ShorthandPropertyAssignment
        | ast::Kind::ObjectLiteralExpression
        | ast::Kind::ClassExpression
        | ast::Kind::ArrowFunction
        | ast::Kind::FunctionExpression => is_definition_visible(store, checker, &parent),
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SymbolEntryTransformOptions {
    pub require_locations_result: bool,
    pub drop_origin_nodes: bool,
}

#[derive(Clone, Default)]
pub(crate) struct SymbolAndEntriesData {
    pub original_node: Option<ast::Node>,
    pub symbols_and_entries: Vec<SymbolAndEntries>,
    pub position: i32,
}

impl LanguageService<'_> {
    pub(crate) fn provide_symbols_and_entries(
        &self,
        ctx: &core::Context,
        uri: lsproto::DocumentUri,
        document_position: lsproto::Position,
        is_rename: bool,
        implementations: bool,
    ) -> Result<(SymbolAndEntriesData, bool), core::Error> {
        // `findReferencedSymbols` except only computes the information needed to return reference locations
        let (program, source_file) = self.get_program_and_file(uri);
        let position =
            self.converters
                .line_and_character_to_position(source_file, document_position) as i32;

        let Some(mut node) = astnav::get_touching_property_name(source_file, position) else {
            return Ok((
                SymbolAndEntriesData {
                    position,
                    ..Default::default()
                },
                false,
            ));
        };
        let adjusted_node_storage;
        if is_rename {
            // Adjust modifier/keyword nodes to the declaration name, matching Strada's findRenameLocations.
            adjusted_node_storage = get_adjusted_location(
                source_file.store(),
                node,
                true, /*for_rename*/
                Some(source_file),
            );
            node = adjusted_node_storage;
        }
        if (is_rename && !node_is_eligible_for_rename(source_file.store(), node))
            || (implementations && ast::is_source_file(source_file.store(), node))
        {
            return Ok((
                SymbolAndEntriesData {
                    original_node: Some(node),
                    position,
                    ..Default::default()
                },
                false,
            ));
        }

        let entries =
            self.get_symbol_and_entries(ctx, position, node, program, is_rename, implementations)?;
        Ok((
            SymbolAndEntriesData {
                original_node: Some(node),
                symbols_and_entries: entries,
                position,
            },
            true,
        ))
    }

    pub(crate) fn get_symbol_and_entries(
        &self,
        ctx: &core::Context,
        position: i32,
        node: ast::Node,
        program: &compiler::Program,
        is_rename: bool,
        implementations: bool,
    ) -> Result<Vec<SymbolAndEntries>, core::Error> {
        let mut options = RefOptions::default();
        if !is_rename {
            options.use_ = ReferenceUse::References;
            if implementations {
                options.implementations = true;
            }
        } else {
            options.use_ = ReferenceUse::Rename;
            options.use_aliases_for_rename = true;
        }
        let source_files_storage = program.source_files();
        let source_files: Vec<&ast::SourceFile> = source_files_storage.iter().collect();
        self.get_referenced_symbols_for_node(ctx, position, node, program, &source_files, options)
    }

    pub fn provide_references(
        &self,
        ctx: &core::Context,
        params: &lsproto::ReferenceParams,
        orchestrator: Option<&dyn CrossProjectOrchestrator>,
    ) -> Result<lsproto::ReferencesResponse, core::Error> {
        handle_cross_project(
            self,
            ctx,
            params.clone(),
            orchestrator,
            symbol_and_entries_to_references_callback,
            combine_references,
            false, /*is_rename*/
            false, /*implementations*/
            SymbolEntryTransformOptions::default(),
        )
    }

    pub(crate) fn symbol_and_entries_to_references(
        &self,
        _ctx: &core::Context,
        params: lsproto::ReferenceParams,
        data: SymbolAndEntriesData,
        _options: SymbolEntryTransformOptions,
    ) -> Result<lsproto::ReferencesResponse, core::Error> {
        // `findReferencedSymbols` except only computes the information needed to return reference locations
        let mut locations = Vec::new();
        for symbol_and_entries in &data.symbols_and_entries {
            locations.extend(self.convert_symbol_and_entries_to_locations(
                symbol_and_entries,
                params.context.include_declaration,
            ));
        }
        Ok(lsproto::LocationsOrNull {
            locations: Some(locations),
            ..Default::default()
        })
    }

    pub fn provide_implementations(
        &self,
        ctx: &core::Context,
        params: &lsproto::ImplementationParams,
        orchestrator: Option<&dyn CrossProjectOrchestrator>,
    ) -> Result<lsproto::ImplementationResponse, core::Error> {
        self.provide_implementations_ex(
            ctx,
            params,
            SymbolEntryTransformOptions::default(),
            orchestrator,
        )
    }

    pub(crate) fn provide_implementations_ex(
        &self,
        ctx: &core::Context,
        params: &lsproto::ImplementationParams,
        options: SymbolEntryTransformOptions,
        orchestrator: Option<&dyn CrossProjectOrchestrator>,
    ) -> Result<lsproto::ImplementationResponse, core::Error> {
        handle_cross_project(
            self,
            ctx,
            params.clone(),
            orchestrator,
            symbol_and_entries_to_implementations_callback,
            combine_implementations,
            false, /*is_rename*/
            true,  /*implementations*/
            options,
        )
    }

    pub(crate) fn symbol_and_entries_to_implementations(
        &self,
        ctx: &core::Context,
        params: lsproto::ImplementationParams,
        data: SymbolAndEntriesData,
        options: SymbolEntryTransformOptions,
    ) -> Result<lsproto::ImplementationResponse, core::Error> {
        let _ = (ctx, params);
        let mut seen_nodes = collections::Set::new();
        let mut entries = Vec::new();
        for symbol_and_entries in &data.symbols_and_entries {
            for reference in &symbol_and_entries.references {
                if seen_nodes.add_if_absent(reference.node)
                    && (!options.drop_origin_nodes
                        || !self
                            .get_program()
                            .source_files()
                            .iter()
                            .find(|source_file| {
                                source_file.store().store_id() == reference.node.store_id()
                            })
                            .is_some_and(|source_file| {
                                source_file
                                    .store()
                                    .loc(reference.node)
                                    .contains_inclusive(data.position)
                            }))
                {
                    entries.push(reference.clone());
                }
            }
        }

        if !options.require_locations_result
            && lsproto::get_client_capabilities(ctx)
                .text_document
                .implementation
                .link_support
        {
            let links = self.convert_entries_to_location_links(&entries);
            return Ok(lsproto::LocationOrLocationsOrDefinitionLinksOrNull {
                definition_links: Some(links.into_iter().map(Some).collect()),
                ..Default::default()
            });
        }
        let locations = self.convert_entries_to_locations(&entries);
        Ok(lsproto::LocationOrLocationsOrDefinitionLinksOrNull {
            locations: Some(locations),
            ..Default::default()
        })
    }

    // == functions for conversions ==
    pub(crate) fn convert_symbol_and_entries_to_locations(
        &self,
        symbol_and_entries: &SymbolAndEntries,
        include_declarations: bool,
    ) -> Vec<lsproto::Location> {
        let mut references = symbol_and_entries.references.clone();

        if !include_declarations {
            if let Some(declarations) = symbol_and_entries
                .definition
                .as_ref()
                .map(|definition| definition.symbol_declarations.as_slice())
            {
                let source_files_storage = self.get_program().source_files();
                let source_files: Vec<&ast::SourceFile> = source_files_storage.iter().collect();
                references.retain(|entry| {
                    let store = store_for_node_from_files(&source_files, entry.node)
                        .expect("reference entry node should belong to a program source file");
                    !is_declaration_of_symbol(store, entry.node, declarations)
                });
            }
        }

        self.convert_entries_to_locations(&references)
    }

    pub(crate) fn convert_entries_to_locations(
        &self,
        entries: &[ReferenceEntry],
    ) -> Vec<lsproto::Location> {
        entries
            .iter()
            .map(|entry| self.get_location_of_entry(entry))
            .collect()
    }

    pub(crate) fn convert_entries_to_location_links(
        &self,
        entries: &[ReferenceEntry],
    ) -> Vec<lsproto::LocationLink> {
        let mut links = Vec::with_capacity(entries.len());
        for entry in entries {
            // Get the selection range (the actual reference)
            let loc = self.get_location_of_entry(entry);
            let target_selection_range = loc.range;
            let target_range = target_selection_range;

            links.push(lsproto::LocationLink {
                target_uri: lsconv::file_name_to_document_uri(&entry.file_name),
                target_range,
                target_selection_range,
                ..Default::default()
            });
        }
        links
    }

    pub(crate) fn merge_references(
        &self,
        _program: &compiler::Program,
        references_to_merge: &[Vec<SymbolAndEntries>],
    ) -> Vec<SymbolAndEntries> {
        let mut result = Vec::new();
        for references in references_to_merge {
            result.extend(references.clone());
        }
        result.sort_by(|left, right| {
            let l = left
                .references
                .first()
                .map(|entry| entry.file_name.clone())
                .unwrap_or_default();
            let r = right
                .references
                .first()
                .map(|entry| entry.file_name.clone())
                .unwrap_or_default();
            l.cmp(&r)
        });
        result
    }

    // === functions for find all ref implementation ===
    pub(crate) fn get_referenced_symbols_for_node(
        &self,
        ctx: &core::Context,
        position: i32,
        node: ast::Node,
        program: &compiler::Program,
        source_files: &[&ast::SourceFile],
        options: RefOptions,
    ) -> Result<Vec<SymbolAndEntries>, core::Error> {
        let _ = position;
        let mut node = node;
        let checker_file = source_file_for_node_from_files(source_files, node)
            .expect("reference node should belong to a searched source file");
        if options.use_ == ReferenceUse::References || options.use_ == ReferenceUse::Rename {
            node = get_adjusted_location(
                checker_file.store(),
                node,
                options.use_ == ReferenceUse::Rename,
                Some(checker_file),
            );
        }

        if let Some(special) = get_referenced_symbols_special(program, node, source_files) {
            return Ok(special);
        }

        program.with_type_checker_for_file_using(
            compiler::CheckerAccess::context(ctx),
            checker_file,
            |checker| {
                let search_store = store_for_node_from_files(source_files, node)
                    .expect("reference node should belong to a searched source file");
                let search_location = if search_store.kind(node) == ast::Kind::Constructor
                    && search_store
                        .parent(node)
                        .and_then(|parent| search_store.name(parent))
                        .is_some()
                {
                    search_store
                        .parent(node)
                        .and_then(|parent| search_store.name(parent))
                        .unwrap()
                } else {
                    node
                };
                let symbol = checker.get_symbol_at_location_public(search_location);
                let Some(symbol) = symbol else {
                    return Ok(Vec::new());
                };

                let symbol_name = checker.symbol_name_public(symbol).unwrap_or_default();
                let search_text = stringutil::strip_quotes(&symbol_name);
                let mut references = Vec::new();
                for source_file in source_files {
                    for reference_location in
                        get_possible_symbol_reference_nodes(source_file, &search_text, None)
                    {
                        if !is_valid_reference_position(
                            source_file.store(),
                            reference_location,
                            &search_text,
                        ) {
                            continue;
                        }
                        if get_meaning_from_location(source_file.store(), reference_location).0 == 0
                        {
                            continue;
                        }
                        if checker
                            .get_symbol_at_location_public(reference_location)
                            .is_some_and(|reference_symbol| reference_symbol == symbol)
                        {
                            references.push(new_node_entry(reference_location));
                        }
                    }
                }

                if references.is_empty() {
                    Ok(Vec::new())
                } else {
                    Ok(vec![SymbolAndEntries {
                        definition: Some(Definition {
                            kind: DefinitionKind::Symbol,
                            node: None,
                            symbol: Some(symbol),
                            symbol_declarations: checker.collect_symbol_declarations_public(symbol),
                            triple_slash_file_ref: None,
                        }),
                        references,
                    }])
                }
            },
        )
    }
}

pub(crate) fn is_declaration_of_symbol(
    store: &ast::AstStore,
    node: ast::Node,
    target_declarations: &[ast::Node],
) -> bool {
    let source = if let Some(decl) = ast::get_declaration_from_name(store, Some(node)) {
        Some(decl)
    } else if store.kind(node) == ast::Kind::DefaultKeyword {
        store.parent(node)
    } else if ast::is_literal_computed_property_declaration_name(store, &node) {
        store.parent(node).and_then(|parent| store.parent(parent))
    } else if store.kind(node) == ast::Kind::ConstructorKeyword
        && store
            .parent(node)
            .as_ref()
            .is_some_and(|parent| ast::is_constructor_declaration(store, *parent))
    {
        store.parent(node).and_then(|parent| store.parent(parent))
    } else {
        None
    };

    source.is_some_and(|source| {
        target_declarations
            .iter()
            .any(|declaration| *declaration == source)
    })
}

pub(crate) fn get_referenced_symbols_special(
    program: &compiler::Program,
    node: ast::Node,
    source_files: &[&ast::SourceFile],
) -> Option<Vec<SymbolAndEntries>> {
    let source_file = source_file_for_node_from_files(source_files, node)
        .expect("reference node should belong to a searched source file");
    let store = source_file.store();
    let node_kind = store.kind(node);
    if is_type_keyword(node_kind) {
        if node_kind == ast::Kind::VoidKeyword
            && store
                .parent(node)
                .is_some_and(|parent| store.kind(parent) == ast::Kind::VoidExpression)
        {
            return None;
        }
        if node_kind == ast::Kind::ReadonlyKeyword && !is_readonly_type_operator(store, node) {
            return None;
        }
        return get_all_references_for_keyword(
            source_files,
            node_kind,
            node_kind == ast::Kind::ReadonlyKeyword,
        );
    }

    if store
        .parent(node)
        .as_ref()
        .is_some_and(|parent| ast::is_import_meta(store, *parent))
        && store
            .parent(node)
            .and_then(|parent| store.name(parent))
            .as_ref()
            .is_some_and(|name| *name == node)
    {
        return get_all_references_for_import_meta(source_files);
    }

    if node_kind == ast::Kind::StaticKeyword
        && store
            .parent(node)
            .as_ref()
            .is_some_and(|parent| store.kind(*parent) == ast::Kind::ClassStaticBlockDeclaration)
    {
        return Some(vec![SymbolAndEntries {
            definition: Some(Definition {
                kind: DefinitionKind::Keyword,
                node: Some(node),
                ..Default::default()
            }),
            references: vec![new_node_entry(node)],
        }]);
    }

    if is_jump_statement_target(store, node) {
        if let Some(parent) = store.parent(node)
            && let Some(label_definition) = get_target_label(store, parent, &store.text(node))
        {
            let Some(container) = store.parent(label_definition) else {
                return None;
            };
            return Some(get_label_references_in_node(
                store,
                source_file,
                container,
                label_definition,
            ));
        }
        return None;
    }

    if is_label_of_labeled_statement(store, node) {
        if let Some(parent) = store.parent(node) {
            return Some(get_label_references_in_node(
                store,
                source_file,
                parent,
                node,
            ));
        }
        return None;
    }

    if is_this(store, node) {
        return Some(get_references_for_this_keyword(
            program,
            store,
            node,
            source_files,
        ));
    }

    if node_kind == ast::Kind::SuperKeyword {
        return Some(get_references_for_super_keyword(
            program,
            store,
            node,
            source_files,
        ));
    }

    None
}

pub(crate) fn get_label_references_in_node(
    store: &ast::AstStore,
    source_file: &ast::SourceFile,
    container: ast::Node,
    target_label: ast::Node,
) -> Vec<SymbolAndEntries> {
    let label_name = store.text(target_label);
    let references = get_possible_symbol_reference_nodes(source_file, &label_name, Some(container))
        .into_iter()
        .filter(|node| {
            *node == target_label
                || (is_jump_statement_target(source_file.store(), *node)
                    && get_target_label(source_file.store(), *node, &label_name)
                        .is_some_and(|label| label == target_label))
        })
        .map(new_node_entry)
        .collect();
    vec![new_symbol_and_entries(
        DefinitionKind::Label,
        Some(target_label),
        None,
        references,
    )]
}

pub(crate) fn get_references_for_this_keyword(
    program: &compiler::Program,
    store: &ast::AstStore,
    this_or_super_keyword: ast::Node,
    source_files: &[&ast::SourceFile],
) -> Vec<SymbolAndEntries> {
    let Some(mut search_space_node) = ast::get_this_container(
        store,
        this_or_super_keyword,
        false, /*include_arrow_functions*/
        false, /*include_class_computed_property_name*/
    ) else {
        return Vec::new();
    };

    let mut static_flag = ast::ModifierFlags::STATIC;
    let is_parameter_name = |store: &ast::AstStore, node: &ast::Node| {
        store.kind(*node) == ast::Kind::Identifier
            && store
                .parent(*node)
                .is_some_and(|parent| store.kind(parent) == ast::Kind::Parameter)
            && store
                .parent(*node)
                .and_then(|parent| store.name(parent))
                .as_ref()
                .is_some_and(|name| *name == *node)
    };

    match store.kind(search_space_node) {
        ast::Kind::MethodDeclaration
        | ast::Kind::MethodSignature
        | ast::Kind::PropertyDeclaration
        | ast::Kind::PropertySignature
        | ast::Kind::Constructor
        | ast::Kind::GetAccessor
        | ast::Kind::SetAccessor => {
            if matches!(
                store.kind(search_space_node),
                ast::Kind::MethodDeclaration | ast::Kind::MethodSignature
            ) && ast::is_object_literal_method(store, Some(search_space_node))
            {
                static_flag = static_flag & node_modifier_flags(store, search_space_node);
                let Some(parent) = store.parent(search_space_node) else {
                    return Vec::new();
                };
                search_space_node = parent;
            } else {
                static_flag = static_flag & node_modifier_flags(store, search_space_node);
                let Some(parent) = store.parent(search_space_node) else {
                    return Vec::new();
                };
                search_space_node = parent;
            }
        }
        ast::Kind::SourceFile => {
            if source_file_for_node_from_files(source_files, search_space_node)
                .is_some_and(ast::is_external_module)
                || is_parameter_name(store, &this_or_super_keyword)
            {
                return Vec::new();
            }
        }
        ast::Kind::FunctionDeclaration | ast::Kind::FunctionExpression => {}
        _ => return Vec::new(),
    }

    let mut references = Vec::new();
    let files_to_search: Vec<&ast::SourceFile> =
        if store.kind(search_space_node) != ast::Kind::SourceFile {
            let source_file = source_file_for_node_from_files(source_files, search_space_node)
                .expect("search space node should belong to a searched source file");
            vec![source_file]
        } else {
            source_files.to_vec()
        };
    let search_symbol =
        source_file_for_node_from_files(source_files, search_space_node).and_then(|source_file| {
            source_node_symbol_from_program(program, source_file, search_space_node)
        });
    for source_file in files_to_search {
        let reference_store = source_file.store();
        let container: Option<ast::Node> = if store.kind(search_space_node) == ast::Kind::SourceFile
        {
            None
        } else {
            Some(search_space_node)
        };
        for node in get_possible_symbol_reference_nodes(source_file, "this", container) {
            if !is_this(reference_store, node) {
                continue;
            }
            let Some(container) = ast::get_this_container(reference_store, node, false, false)
            else {
                continue;
            };
            if !ast::can_have_symbol(reference_store, container) {
                continue;
            }
            let matches = match store.kind(search_space_node) {
                ast::Kind::FunctionExpression | ast::Kind::FunctionDeclaration => {
                    search_symbol
                        == source_node_symbol_from_program(program, source_file, container)
                }
                ast::Kind::MethodDeclaration | ast::Kind::MethodSignature => {
                    ast::is_object_literal_method(store, Some(search_space_node))
                        && search_symbol
                            == source_node_symbol_from_program(program, source_file, container)
                }
                ast::Kind::ClassExpression
                | ast::Kind::ClassDeclaration
                | ast::Kind::ObjectLiteralExpression => reference_store
                    .parent(container)
                    .as_ref()
                    .is_some_and(|parent| {
                        ast::can_have_symbol(reference_store, parent)
                            && search_symbol
                                == source_node_symbol_from_program(program, source_file, *parent)
                            && ast::is_static(reference_store, container)
                                == (static_flag != ast::ModifierFlags::NONE)
                    }),
                ast::Kind::SourceFile => {
                    reference_store.kind(container) == ast::Kind::SourceFile
                        && source_file_for_node_from_files(source_files, container)
                            .is_some_and(|source_file| !ast::is_external_module(source_file))
                        && !is_parameter_name(reference_store, &node)
                }
                _ => false,
            };
            if matches {
                references.push(new_node_entry(node));
            }
        }
    }

    let this_parameter = references
        .iter()
        .find(|reference| {
            let reference_store = store_for_node_from_files(source_files, reference.node)
                .expect("reference entry node should belong to a searched source file");
            reference_store
                .parent(reference.node)
                .is_some_and(|parent| reference_store.kind(parent) == ast::Kind::Parameter)
        })
        .map(|reference| reference.node)
        .unwrap_or(this_or_super_keyword);

    vec![new_symbol_and_entries(
        DefinitionKind::This,
        Some(this_parameter),
        search_symbol,
        references,
    )]
}

pub(crate) fn get_references_for_super_keyword(
    program: &compiler::Program,
    store: &ast::AstStore,
    super_keyword: ast::Node,
    source_files: &[&ast::SourceFile],
) -> Vec<SymbolAndEntries> {
    let Some(mut search_space_node) = ast::get_super_container(store, &super_keyword, false) else {
        return Vec::new();
    };
    let mut static_flag = ast::ModifierFlags::STATIC;
    match store.kind(search_space_node) {
        ast::Kind::PropertyDeclaration
        | ast::Kind::PropertySignature
        | ast::Kind::MethodDeclaration
        | ast::Kind::MethodSignature
        | ast::Kind::Constructor
        | ast::Kind::GetAccessor
        | ast::Kind::SetAccessor => {
            static_flag = static_flag & node_modifier_flags(store, search_space_node);
            let Some(parent) = store.parent(search_space_node) else {
                return Vec::new();
            };
            search_space_node = parent;
        }
        _ => return Vec::new(),
    }

    let source_file = source_file_for_node_from_files(source_files, search_space_node)
        .expect("search space node should belong to a searched source file");
    let reference_store = source_file.store();
    let search_symbol = source_node_symbol_from_program(program, source_file, search_space_node);
    let references =
        get_possible_symbol_reference_nodes(source_file, "super", Some(search_space_node))
            .into_iter()
            .filter(|node| {
                if reference_store.kind(*node) != ast::Kind::SuperKeyword {
                    return false;
                }
                ast::get_super_container(reference_store, node, false).is_some_and(|container| {
                    ast::is_static(reference_store, container)
                        == (static_flag != ast::ModifierFlags::NONE)
                        && reference_store.parent(container).and_then(|parent| {
                            source_node_symbol_from_program(program, source_file, parent)
                        }) == search_symbol
                })
            })
            .map(new_node_entry)
            .collect();

    vec![new_symbol_and_entries(
        DefinitionKind::Symbol,
        None,
        search_symbol,
        references,
    )]
}

pub(crate) fn get_all_references_for_import_meta(
    source_files: &[&ast::SourceFile],
) -> Option<Vec<SymbolAndEntries>> {
    let mut references = Vec::new();
    for source_file in source_files {
        let store = source_file.store();
        for node in get_possible_symbol_reference_nodes(source_file, "meta", None) {
            let parent = store.parent(node);
            if parent
                .as_ref()
                .is_some_and(|parent| ast::is_import_meta(store, *parent))
            {
                references.push(new_node_entry(parent.unwrap()));
            }
        }
    }
    if references.is_empty() {
        return None;
    }
    Some(vec![SymbolAndEntries {
        definition: Some(Definition {
            kind: DefinitionKind::Keyword,
            node: Some(references[0].node),
            ..Default::default()
        }),
        references,
    }])
}

pub(crate) fn get_all_references_for_keyword(
    source_files: &[&ast::SourceFile],
    keyword_kind: ast::Kind,
    filter_readonly_type_operator: bool,
) -> Option<Vec<SymbolAndEntries>> {
    let keyword = scanner::token_to_string(keyword_kind);
    let mut references = Vec::new();
    for source_file in source_files {
        for reference_location in get_possible_symbol_reference_nodes(source_file, &keyword, None) {
            if source_file.store().kind(reference_location) == keyword_kind
                && (!filter_readonly_type_operator
                    || is_readonly_type_operator(source_file.store(), reference_location))
            {
                references.push(new_node_entry(reference_location));
            }
        }
    }
    if references.is_empty() {
        return None;
    }
    let first_reference_node = references[0].node;
    Some(vec![new_symbol_and_entries(
        DefinitionKind::Keyword,
        Some(first_reference_node),
        None,
        references,
    )])
}

pub(crate) fn get_possible_symbol_reference_nodes(
    source_file: &ast::SourceFile,
    symbol_name: &str,
    container: Option<ast::Node>,
) -> Vec<ast::Node> {
    get_possible_symbol_reference_positions(source_file, symbol_name, container)
        .into_iter()
        .filter_map(|position| {
            let reference_location = astnav::get_touching_property_name(source_file, position)?;
            (reference_location != source_file.as_node()).then_some(reference_location)
        })
        .collect()
}

pub(crate) fn get_possible_symbol_reference_positions(
    source_file: &ast::SourceFile,
    symbol_name: &str,
    container: Option<ast::Node>,
) -> Vec<i32> {
    let mut positions = Vec::new();

    // Cache symbol existence for files to save text search.
    // Also, need to make this work for unicode escapes.

    // Be resilient in the face of a symbol with no name or zero length name
    if symbol_name.is_empty() {
        return positions;
    }

    let text = source_file.text();
    let source_length = text.len() as i32;
    let symbol_name_length = symbol_name.len() as i32;
    let source_file_node = source_file.as_node();
    let container = container.unwrap_or(source_file_node);
    let store = source_file.store();
    let container_loc = store.loc(container);
    let end_pos = container_loc.end();

    let mut position = text[container_loc.pos() as usize..]
        .find(symbol_name)
        .map(|index| container_loc.pos() + index as i32)
        .unwrap_or(-1);
    while position >= 0 && position < end_pos {
        // We found a match.  Make sure it's not part of a larger word (i.e. the char
        // before and after it have to be a non-identifier char).
        let end_position = position + symbol_name_length;

        if (position == 0
            || !scanner::is_identifier_part(text.as_bytes()[(position - 1) as usize] as char))
            && (end_position == source_length
                || !scanner::is_identifier_part(text.as_bytes()[end_position as usize] as char))
        {
            // Found a real match.  Keep searching.
            positions.push(position);
        }
        let start_index = position + symbol_name_length + 1;
        if start_index > source_length {
            break;
        }
        position = text[start_index as usize..]
            .find(symbol_name)
            .map(|found_index| start_index + found_index as i32)
            .unwrap_or(-1);
    }

    positions
}

// findFirstJsxNode recursively searches for the first JSX element, self-closing element, or fragment
pub(crate) fn find_first_jsx_node(store: &ast::AstStore, root: ast::Node) -> Option<ast::Node> {
    fn visit(store: &ast::AstStore, node: &ast::Node) -> Option<ast::Node> {
        // Check if this is a JSX node we're looking for
        match store.kind(*node) {
            ast::Kind::JsxElement | ast::Kind::JsxSelfClosingElement | ast::Kind::JsxFragment => {
                return Some(*node);
            }
            _ => {}
        }

        // Skip subtree if it doesn't contain JSX
        if !store
            .subtree_facts(*node)
            .intersects(ast::SubtreeFacts::CONTAINS_JSX)
        {
            return None;
        }

        // Traverse children to find JSX node
        if let Some(children) = store.children(*node) {
            for child in children.iter() {
                if let Some(result) = visit(store, &child) {
                    return Some(result);
                }
            }
        }
        None
    }

    visit(store, &root)
}

pub(crate) fn get_references_for_non_module(
    _referenced_file: &ast::SourceFile,
    _program: &compiler::Program,
) -> Vec<ReferenceEntry> {
    Vec::new()
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RefSearch {
    pub coming_from: ImpExpKind, // import, export
    pub(crate) symbol: Option<ast::SymbolIdentity>,
    pub text: String,
    pub escaped_text: String,
    pub(crate) parents: Vec<ast::SymbolIdentity>,
    pub(crate) all_search_symbols: Vec<ast::SymbolIdentity>,
}

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq)]
pub(crate) struct InheritKey {
    pub symbol: Option<ast::SymbolIdentity>,
    pub parent: Option<ast::SymbolIdentity>,
}

#[derive(Default)]
pub(crate) struct RefState {
    pub result: Vec<SymbolAndEntries>,
}

pub(crate) fn get_merged_aliased_symbol_of_namespace_export_declaration(
    store: &ast::AstStore,
    node: ast::Node,
    symbol: ast::SymbolIdentity,
    checker: &mut checker::Checker<'_, '_>,
) -> Option<ast::SymbolIdentity> {
    if store
        .parent(node)
        .as_ref()
        .is_some_and(|parent| store.kind(*parent) == ast::Kind::NamespaceExportDeclaration)
    {
        let (aliased_symbol, ok) = checker.resolve_alias_public(Some(symbol));
        if ok {
            let aliased_symbol = aliased_symbol.unwrap();
            let target_symbol = checker.get_merged_symbol_public(aliased_symbol);
            if Some(aliased_symbol) != target_symbol {
                return target_symbol;
            }
        }
    }
    None
}

pub(crate) fn get_special_search_kind(
    store: &ast::AstStore,
    node: Option<ast::Node>,
) -> &'static str {
    let Some(node) = node else {
        return "none";
    };
    match store.kind(node) {
        ast::Kind::Constructor | ast::Kind::ConstructorKeyword => "constructor",
        ast::Kind::Identifier
            if store
                .parent(node)
                .as_ref()
                .is_some_and(|parent| ast::is_class_like(store, *parent)) =>
        {
            "class"
        }
        _ => "none",
    }
}

fn is_this(store: &ast::AstStore, node: ast::Node) -> bool {
    crate::utilities::is_this(store, node)
}
