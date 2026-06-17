use std::sync::Arc;

use ts_ast as ast;
use ts_collections::{self as collections, FastHashMapExt};
use ts_core as core;
use ts_scanner as scanner;
use ts_sourcemap as sourcemap;
use ts_stringutil as stringutil;
use ts_tspath as tspath;

use crate::utilities::{
    GetLiteralTextFlags, LineCharacterCache, QuoteChar, calculate_indent,
    escape_jsx_attribute_string, escape_non_ascii_string, get_literal_text, is_binary_operation,
    is_immediately_invoked_function_expression_or_arrow_function, is_jsdoc_like_text,
    is_new_expression_without_arguments, is_pinned_comment, is_recognized_triple_slash_comment,
    mixing_binary_operators_requires_parentheses, new_line_character_cache,
    skip_synthesized_parentheses,
};
use crate::{
    AutoGenerateOptions, EF_EXTERNAL_HELPERS, EF_HELPER_NAME, EF_INDENTED, EF_INDIRECT_CALL,
    EF_MULTI_LINE, EF_NO_ASCII_ESCAPING, EF_NO_INDENTATION, EF_NO_LEADING_COMMENTS,
    EF_NO_NESTED_COMMENTS, EF_NO_SOURCE_MAP, EF_NO_TRAILING_COMMENTS, EF_REUSE_TEMP_VARIABLE_SCOPE,
    EF_SINGLE_LINE, EF_START_ON_NEW_LINE, EmitContext, EmitFlags, EmitHelper, EmitTextWriter,
    GeneratedIdentifierFlags, NameGenerator, SharedEmitTextWriter, SharedEmitTextWriterHandle,
    compare_emit_helpers, get_default_indent_size, get_indent_string, helpers::helper_from_key,
    namegenerator::LocalNameBindingFacts, new_emit_context, new_shared_text_writer,
};

// Package printer exports a Printer for pretty-printing TS ASTs and writer interfaces and implementations for using them
// Intended ultimate usage:
//
//	func nodeToInlineStr(node *ast.Node) {
//		// Reuse singleton single-line writer.
//		p = printer.NewPrinter(printer.PrinterOptions{ RemoveComments: true }, printer.PrintHandlers{})
//		p.Write(node, nil /*sourceFile*/, printer.SingleLineTextWriter)
//		return printer.SingleLineTextWriter.getText()
//	}
//
// // or
//
//	func nodeToStr(node *ast.Node, options CompilerOptions) {
//		// Use own writer
//		p := printer.NewPrinter(printer.PrinterOptions{ NewLine: options.NewLine}, printer.PrintHandlers{})
//		return p.Emit(node, nil /*sourceFile*/)
//	}

#[derive(Clone, Copy, Default)]
pub struct PrinterOptions {
    pub remove_comments: bool,
    pub new_line: core::NewLineKind,
    // pub omit_trailing_semicolon: bool,
    pub no_emit_helpers: bool,
    // pub module: core::ModuleKind,
    // pub module_resolution: core::ModuleResolutionKind,
    pub target: core::ScriptTarget,
    pub source_map: bool,
    pub inline_source_map: bool,
    pub inline_sources: bool,
    pub omit_brace_source_map_positions: bool,
    // pub extended_diagnostics: bool,
    pub only_print_jsdoc_style: bool,
    pub never_ascii_escape: bool,
    // pub strip_internal: bool,
    pub preserve_source_newlines: bool,
    pub terminate_unterminated_literals: bool,
}

pub type NodeEmitHandler = Box<dyn for<'a> FnMut(Option<&'a ast::Node>)>;
pub type NodeListEmitHandler = Box<dyn for<'a> FnMut(Option<ast::SourceNodeList<'a>>)>;

struct EmitNodeList {
    nodes: Vec<ast::Node>,
    loc: core::TextRange,
    is_missing: bool,
    has_trailing_comma: bool,
}

impl EmitNodeList {
    fn from_source(list: ast::SourceNodeList<'_>) -> Self {
        Self {
            nodes: list.nodes(),
            loc: list.loc(),
            is_missing: list.is_missing(),
            has_trailing_comma: list.has_trailing_comma(),
        }
    }

    fn loc(&self) -> core::TextRange {
        self.loc
    }

    fn is_missing(&self) -> bool {
        self.is_missing
    }

    fn has_trailing_comma(&self) -> bool {
        self.has_trailing_comma
    }
}

#[derive(Default)]
pub struct PrintHandlers {
    // A hook used by the Printer when generating unique names to avoid collisions with
    // globally defined names that exist outside of the current source file.
    pub has_global_name: Option<fn(String) -> bool>,

    // Hooks intentionally mirror the currently disabled Go handler fields.
    ////// A hook used by the Printer to provide notifications prior to emitting a node. A
    ////// compatible implementation **must** invoke `emitCallback` with the provided `hint` and
    ////// `node` values.
    ////// @param hint A hint indicating the intended purpose of the node.
    ////// @param node The node to emit.
    ////// @param emitCallback A callback that, when invoked, will emit the node.
    ////// @example
    ////// ```ts
    ////// var printer = createPrinter(printerOptions, {
    //////   onEmitNode(hint, node, emitCallback) {
    //////     // set up or track state prior to emitting the node...
    //////     emitCallback(hint, node);
    //////     // restore state after emitting the node...
    //////   }
    ////// });
    ////// ```
    //// OnEmitNode

    // Hooks intentionally mirror the currently disabled Go handler fields.
    ////// A hook used to check if an emit notification is required for a node.
    ////// @param node The node to emit.
    //// IsEmitNotificationEnabled

    // Hooks intentionally mirror the currently disabled Go handler fields.
    ////// A hook used by the Printer to perform just-in-time substitution of a node. This is
    ////// primarily used by node transformations that need to substitute one node for another,
    ////// such as replacing `myExportedVar` with `exports.myExportedVar`.
    ////// @param hint A hint indicating the intended purpose of the node.
    ////// @param node The node to emit.
    ////// @example
    ////// ```ts
    ////// var printer = createPrinter(printerOptions, {
    //////   substituteNode(hint, node) {
    //////     // perform substitution if necessary...
    //////     return node;
    //////   }
    ////// });
    ////// ```
    //// SubstituteNode

    // Hooks intentionally mirror the currently disabled Go handler fields.
    //// OnEmitSourceMapOfNode
    //// OnEmitSourceMapOfToken
    //// OnEmitSourceMapOfPosition
    pub on_before_emit_node: Option<NodeEmitHandler>,
    pub on_after_emit_node: Option<NodeEmitHandler>,
    pub on_before_emit_node_list: Option<NodeListEmitHandler>,
    pub on_after_emit_node_list: Option<NodeListEmitHandler>,
    pub on_before_emit_token: Option<NodeEmitHandler>,
    pub on_after_emit_token: Option<NodeEmitHandler>,
}

fn share_source_file_option(source_file: &Option<ast::SourceFile>) -> Option<ast::SourceFile> {
    source_file.as_ref().map(ast::SourceFile::share_readonly)
}

pub struct Printer {
    pub print_handlers: PrintHandlers,
    pub options: PrinterOptions,
    emit_context: EmitContext,
    current_source_file: Option<ast::SourceFile>,
    unique_helper_names: Option<collections::FastHashMap<String, ast::Node>>,
    external_helpers_module_name: Option<ast::Node>,
    next_list_element_pos: i32,
    writer: Option<SharedEmitTextWriter>,
    own_writer: Option<SharedEmitTextWriter>,
    write_kind: WriteKind,
    source_maps_disabled: bool,
    source_map_generator: Option<sourcemap::Generator>,
    source_map_source: Option<SourceMapSource>,
    source_map_source_index: Option<sourcemap::SourceIndex>,
    source_map_source_is_json: bool,
    source_map_line_char_cache: Option<LineCharacterCache>,
    most_recent_source_map_source: Option<SourceMapSource>,
    most_recent_source_map_source_index: Option<sourcemap::SourceIndex>,
    container_pos: i32,
    container_end: i32,
    declaration_list_container_end: i32,
    detached_comments_info: core::Stack<DetachedCommentsInfo>,
    comments_disabled: bool,
    in_extends: bool, // whether we are emitting the `extends` clause of a ConditionalTypeNode or InferTypeNode
    name_generator: NameGenerator,
    binding_facts: Option<Arc<dyn crate::EmitBindingFacts>>,
    comment_state_arena: core::Arena<CommentState>,
    source_map_state_arena: core::Arena<SourceMapState>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CommentSeparator {
    None,
    Before,
    After,
}

#[derive(Clone, Default)]
struct DetachedCommentsInfo {
    node_pos: i32,
    detached_comment_end_pos: i32,
}

#[derive(Clone, Default)]
struct CommentState {
    emit_flags: EmitFlags,          // holds the emit flags for the current node
    comment_range: core::TextRange, // holds the comment range calculated for the current node
    container_pos: i32,             // captures the value of containerPos prior to entering an node
    container_end: i32,             // captures the value of containerEnd prior to entering an node
    declaration_list_container_end: i32, // captures the value of declarationListContainerEnd prior to entering an node
}

#[derive(Clone, Default)]
struct SourceMapState {
    emit_flags: EmitFlags,             // holds the emit flags for the current node
    source_map_range: core::TextRange, // holds the source map range calculated for the current node
    has_token_source_map_range: bool, // captures whether the source map range was set for the current node
}

#[derive(Clone)]
struct SourceMapSource {
    text: Arc<str>,
    file_name: Arc<str>,
    ecma_line_map: Arc<[core::TextPos]>,
}

impl SourceMapSource {
    fn from_source_file(source_file: &ast::SourceFile) -> Self {
        Self {
            text: Arc::from(source_file.text()),
            file_name: Arc::from(source_file.file_name()),
            ecma_line_map: source_file.ecma_line_map(),
        }
    }

    fn same_source(&self, other: &Self) -> bool {
        self.file_name == other.file_name
    }
}

impl sourcemap::Source for SourceMapSource {
    fn text(&self) -> String {
        self.text.to_string()
    }

    fn file_name(&self) -> String {
        self.file_name.to_string()
    }

    fn ecma_line_map(&self) -> Arc<[core::TextPos]> {
        Arc::clone(&self.ecma_line_map)
    }
}

#[derive(Default)]
struct PrinterState {
    comment_state: Option<CommentState>,
    source_map_state: Option<SourceMapState>,
}

pub fn new_printer(
    options: PrinterOptions,
    handlers: PrintHandlers,
    emit_context: Option<EmitContext>,
) -> Printer {
    let mut printer = Printer {
        print_handlers: handlers,
        options,
        emit_context: emit_context.unwrap_or_else(new_emit_context),
        current_source_file: None,
        unique_helper_names: None,
        external_helpers_module_name: None,
        next_list_element_pos: 0,
        writer: None,
        own_writer: None,
        write_kind: WriteKind::None,
        source_maps_disabled: false,
        source_map_generator: None,
        source_map_source: None,
        source_map_source_index: None,
        source_map_source_is_json: false,
        source_map_line_char_cache: None,
        most_recent_source_map_source: None,
        most_recent_source_map_source_index: None,
        container_pos: -1,
        container_end: -1,
        declaration_list_container_end: -1,
        detached_comments_info: core::Stack::default(),
        comments_disabled: options.remove_comments,
        in_extends: false,
        name_generator: NameGenerator::default(),
        binding_facts: None,
        comment_state_arena: core::Arena::default(),
        source_map_state_arena: core::Arena::default(),
    };
    // wire up name generator
    printer.name_generator.context = Some(printer.emit_context.state_ref());
    printer
}

impl Printer {
    pub fn into_emit_context(self) -> EmitContext {
        self.emit_context
    }

    fn activate_emit_context(&mut self) {
        self.name_generator.context = Some(self.emit_context.state_ref());
    }

    fn store_for_node(&self, node: &ast::Node) -> &ast::AstStore {
        self.emit_context.store_for_node(*node)
    }

    pub fn set_binding_facts(&mut self, binding_facts: Option<Arc<dyn crate::EmitBindingFacts>>) {
        self.binding_facts = binding_facts;
    }

    fn binding_source_node(&self, node: &ast::Node) -> Option<ast::Node> {
        let binding_facts = self.binding_facts.as_deref()?;
        let source_node = self
            .emit_context
            .most_original(&self.emit_context.get_node_for_generated_name(node));
        let binding_root = binding_facts.root();
        if source_node.store_id() != binding_root.store_id() {
            return None;
        }
        let store = self.emit_context.store_for_node(source_node);
        if source_node != binding_root
            && ast::get_source_file_of_node(store, Some(source_node)) != Some(binding_root)
        {
            return None;
        }
        Some(source_node)
    }

    fn binding_facts_for_node_owned(
        &self,
        node: &ast::Node,
    ) -> Option<Arc<dyn crate::EmitBindingFacts>> {
        let binding_facts = self.binding_facts.as_ref()?;
        self.binding_source_node(node)
            .map(|_| Arc::clone(binding_facts))
    }

    fn node_symbol(&self, node: &ast::Node) -> Option<ast::SymbolHandle> {
        let binding_facts = self.binding_facts.as_deref()?;
        let source_node = self.binding_source_node(node)?;
        if source_node == binding_facts.root() {
            return binding_facts.source_symbol();
        }
        binding_facts.symbol(source_node)
    }

    fn kind(&self, node: &ast::Node) -> ast::Kind {
        self.store_for_node(node).kind(*node)
    }

    fn loc(&self, node: &ast::Node) -> core::TextRange {
        self.store_for_node(node).loc(*node)
    }

    fn node_list(&self, list: ast::SourceNodeList<'_>) -> Vec<ast::Node> {
        list.into_iter().collect()
    }

    fn optional_node_list(list: Option<ast::SourceNodeList<'_>>) -> Vec<ast::Node> {
        list.map(|list| list.iter().collect()).unwrap_or_default()
    }

    fn optional_modifier_list(list: Option<ast::SourceModifierList<'_>>) -> Vec<ast::Node> {
        list.map(|list| list.into_iter().collect())
            .unwrap_or_default()
    }

    fn modifiers(&self, owner: &ast::Node) -> Vec<ast::Node> {
        Self::optional_modifier_list(self.store_for_node(owner).source_modifiers(*owner))
    }

    fn emit_block_node(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let state = self.enter_node_to_writer(node, writer);
        self.emit_block(node, node, writer);
        self.exit_node_to_writer(node, state, writer);
    }

    fn emit_function_body_node(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        self.emit_function_body(node, node, writer);
    }

    fn emit_variable_declaration_list_node(
        &mut self,
        node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_variable_declaration_list(node, node, writer);
    }

    fn parent_is_absent_or_matches(&self, child: &ast::Node, parent: &ast::Node) -> bool {
        self.store_for_node(child)
            .parent(*child)
            .is_none_or(|child_parent| child_parent == *parent)
    }

    fn nodes_have_same_parent(&self, left: &ast::Node, right: &ast::Node) -> bool {
        let left_parent = self.store_for_node(left).original_parent(*left);
        let Some(left_parent) = left_parent else {
            return false;
        };
        if ast::node_is_synthesized(self.store_for_node(&left_parent), left_parent) {
            return false;
        }
        Some(left_parent) == self.store_for_node(right).original_parent(*right)
    }

    fn nodes_are_adjacent_in_list(
        &self,
        list: Option<ast::SourceNodeList<'_>>,
        previous_node: &ast::Node,
        next_node: &ast::Node,
    ) -> bool {
        let Some(list) = list else {
            return false;
        };
        let nodes: Vec<_> = list.iter().collect();
        let Some(previous_index) = nodes.iter().position(|node| node == previous_node) else {
            return false;
        };
        nodes.get(previous_index + 1) == Some(next_node)
    }

    fn modifiers_are_adjacent_in_list(
        &self,
        list: Option<ast::SourceModifierList<'_>>,
        previous_node: &ast::Node,
        next_node: &ast::Node,
    ) -> bool {
        let Some(list) = list else {
            return false;
        };
        let nodes: Vec<_> = list.iter().collect();
        let Some(previous_index) = nodes.iter().position(|node| node == previous_node) else {
            return false;
        };
        nodes.get(previous_index + 1) == Some(next_node)
    }

    fn jsx_child_kind(kind: ast::Kind) -> bool {
        matches!(
            kind,
            ast::Kind::JsxText
                | ast::Kind::JsxTextAllWhiteSpaces
                | ast::Kind::JsxExpression
                | ast::Kind::JsxElement
                | ast::Kind::JsxSelfClosingElement
                | ast::Kind::JsxFragment
        )
    }

    fn sibling_nodes_are_adjacent_in_containing_list(
        &self,
        previous_node: &ast::Node,
        next_node: &ast::Node,
    ) -> bool {
        let store = self.store_for_node(previous_node);
        if store.store_id() != self.store_for_node(next_node).store_id() {
            return false;
        }

        let Some(parent) = store.parent(*previous_node) else {
            return false;
        };
        if Some(parent) != store.parent(*next_node) {
            return false;
        }

        match store.kind(*previous_node) {
            ast::Kind::TypeParameter => {
                if ast::is_function_like(store, Some(parent))
                    || ast::is_class_like(store, parent)
                    || ast::is_interface_declaration(store, parent)
                    || ast::is_type_or_js_type_alias_declaration(store, parent)
                {
                    return self.nodes_are_adjacent_in_list(
                        store.source_type_parameters(parent),
                        previous_node,
                        next_node,
                    );
                }
                if !ast::is_infer_type_node(store, parent) {
                    panic!("unexpected TypeParameter parent: {:?}", store.kind(parent));
                }
            }
            ast::Kind::Parameter => {
                return self.nodes_are_adjacent_in_list(
                    store.source_parameters(parent),
                    previous_node,
                    next_node,
                );
            }
            ast::Kind::TemplateLiteralTypeSpan | ast::Kind::TemplateSpan => {
                return self.nodes_are_adjacent_in_list(
                    store.source_template_spans(parent),
                    previous_node,
                    next_node,
                );
            }
            ast::Kind::HeritageClause => {
                return self.nodes_are_adjacent_in_list(
                    store.source_heritage_clauses(parent),
                    previous_node,
                    next_node,
                );
            }
            _ => {}
        }

        match store.kind(parent) {
            ast::Kind::TypeLiteral | ast::Kind::InterfaceDeclaration
                if ast::is_type_element(store, previous_node) =>
            {
                self.nodes_are_adjacent_in_list(
                    store.source_members(parent),
                    previous_node,
                    next_node,
                )
            }
            ast::Kind::UnionType | ast::Kind::IntersectionType => self.nodes_are_adjacent_in_list(
                store.source_types(parent),
                previous_node,
                next_node,
            ),
            ast::Kind::ArrayLiteralExpression
            | ast::Kind::TupleType
            | ast::Kind::NamedImports
            | ast::Kind::NamedExports => self.nodes_are_adjacent_in_list(
                store.source_elements(parent),
                previous_node,
                next_node,
            ),
            ast::Kind::ObjectLiteralExpression | ast::Kind::JsxAttributes => self
                .nodes_are_adjacent_in_list(
                    store.source_properties(parent),
                    previous_node,
                    next_node,
                ),
            ast::Kind::CallExpression | ast::Kind::NewExpression => {
                if ast::is_type_node(store, *previous_node) {
                    self.nodes_are_adjacent_in_list(
                        store.source_type_arguments(parent),
                        previous_node,
                        next_node,
                    )
                } else if Some(*previous_node) != store.expression(parent) {
                    self.nodes_are_adjacent_in_list(
                        store.source_arguments(parent),
                        previous_node,
                        next_node,
                    )
                } else {
                    false
                }
            }
            ast::Kind::JsxElement | ast::Kind::JsxFragment
                if Self::jsx_child_kind(store.kind(*previous_node)) =>
            {
                self.nodes_are_adjacent_in_list(
                    Some(store.source_jsx_children(parent)),
                    previous_node,
                    next_node,
                )
            }
            ast::Kind::JsxOpeningElement | ast::Kind::JsxSelfClosingElement
                if ast::is_type_node(store, *previous_node) =>
            {
                self.nodes_are_adjacent_in_list(
                    store.source_type_arguments(parent),
                    previous_node,
                    next_node,
                )
            }
            ast::Kind::Block
            | ast::Kind::ModuleBlock
            | ast::Kind::CaseClause
            | ast::Kind::DefaultClause
            | ast::Kind::SourceFile => self.nodes_are_adjacent_in_list(
                store.source_statements(parent),
                previous_node,
                next_node,
            ),
            ast::Kind::CaseBlock => self.nodes_are_adjacent_in_list(
                store.source_clauses(parent),
                previous_node,
                next_node,
            ),
            ast::Kind::ClassDeclaration | ast::Kind::ClassExpression
                if ast::is_class_element(store, *previous_node) =>
            {
                self.nodes_are_adjacent_in_list(
                    store.source_members(parent),
                    previous_node,
                    next_node,
                )
            }
            ast::Kind::EnumDeclaration if ast::is_enum_member(store, *previous_node) => self
                .nodes_are_adjacent_in_list(store.source_members(parent), previous_node, next_node),
            _ if ast::is_modifier(store, *previous_node) => self.modifiers_are_adjacent_in_list(
                store.source_modifiers(parent),
                previous_node,
                next_node,
            ),
            _ => false,
        }
    }

    fn sibling_node_positions_are_comparable(
        &self,
        previous_node: &ast::Node,
        next_node: &ast::Node,
    ) -> bool {
        self.loc(next_node).pos() >= self.loc(previous_node).end()
            && self.nodes_have_same_parent(previous_node, next_node)
            && self.sibling_nodes_are_adjacent_in_containing_list(previous_node, next_node)
    }

    pub fn emit(&mut self, node: &ast::Node, source_file: Option<&ast::SourceFile>) -> String {
        if self.own_writer.is_none() {
            self.own_writer = Some(new_shared_text_writer(
                self.options.new_line.get_new_line_character().to_string(),
                0,
            ));
        }

        let writer = self.own_writer.as_ref().unwrap().clone();
        self.write_node(Some(node), source_file, writer.clone(), None);
        let text = writer.borrow().string();
        writer.borrow_mut().clear();
        text
    }

    pub fn emit_source_file(&mut self, source_file: &ast::SourceFile) -> String {
        self.emit(&source_file.as_node(), Some(source_file))
    }

    pub fn write_node(
        &mut self,
        node: Option<&ast::Node>,
        source_file: Option<&ast::SourceFile>,
        writer: SharedEmitTextWriter,
        source_map_generator: Option<sourcemap::Generator>,
    ) -> Option<sourcemap::Generator> {
        self.activate_emit_context();
        let saved_current_source_file = share_source_file_option(&self.current_source_file);
        let saved_writer = self.writer.take();
        let saved_unique_helper_names = std::mem::take(&mut self.unique_helper_names);
        let saved_source_maps_disabled = self.source_maps_disabled;
        let saved_source_map_generator = self.source_map_generator.take();
        let saved_source_map_source = self.source_map_source.take();
        let saved_source_map_source_index = self.source_map_source_index;
        let saved_source_map_source_is_json = self.source_map_source_is_json;
        let saved_source_map_line_char_cache = self.source_map_line_char_cache.take();

        self.source_maps_disabled = source_map_generator.is_none();
        self.source_map_generator = source_map_generator;
        self.source_map_source = None;
        self.source_map_source_index = None;
        self.source_map_line_char_cache = None;
        self.set_source_file(source_file.map(ast::SourceFile::share_readonly));
        self.writer = Some(writer.clone());

        writer.borrow_mut().clear();
        let mut writer_handle = SharedEmitTextWriterHandle::new(writer);
        self.write_node_worker(node, &mut writer_handle);
        let source_map_generator = self.source_map_generator.take();

        self.current_source_file = saved_current_source_file;
        self.name_generator.set_source_file(
            self.current_source_file.as_ref(),
            self.print_handlers.has_global_name,
        );
        self.writer = saved_writer;
        self.unique_helper_names = saved_unique_helper_names;
        self.source_maps_disabled = saved_source_maps_disabled;
        self.source_map_generator = saved_source_map_generator;
        self.source_map_source = saved_source_map_source;
        self.source_map_source_index = saved_source_map_source_index;
        self.source_map_source_is_json = saved_source_map_source_is_json;
        self.source_map_line_char_cache = saved_source_map_line_char_cache;
        source_map_generator
    }

    fn set_source_file(&mut self, source_file: Option<ast::SourceFile>) {
        self.current_source_file = source_file;
        self.name_generator.set_source_file(
            self.current_source_file.as_ref(),
            self.print_handlers.has_global_name,
        );
        self.emit_context
            .set_source_file(self.current_source_file.as_ref());
        self.unique_helper_names = None;
        self.external_helpers_module_name = None;
        if let Some(source_file) = self.current_source_file.as_ref() {
            if self
                .emit_context
                .emit_flags(&self.emit_context.most_original(&source_file.as_node()))
                & EF_EXTERNAL_HELPERS
                != 0
            {
                self.unique_helper_names = Some(collections::FastHashMap::new());
            }
            self.external_helpers_module_name = self
                .emit_context
                .get_external_helpers_module_name(source_file);
            self.set_source_map_source(SourceMapSource::from_source_file(source_file));
        }
    }

    fn write_node_worker(&mut self, node: Option<&ast::Node>, writer: &mut dyn EmitTextWriter) {
        let Some(node) = node else {
            return;
        };
        let should_enter_node = self.kind(node) != ast::Kind::FunctionDeclaration;
        let state = should_enter_node.then(|| self.enter_node_to_writer(node, writer));

        macro_rules! payload {
            ($payload:ident) => {
                *node
            };
            (ExpressionStatement) => {
                self.store_for_node(node)
                    .as_expression_statement(*node)
                    .clone()
            };
            (ForStatement) => {
                self.store_for_node(node).as_for_statement(*node).clone()
            };
            (VariableStatement) => {
                self.store_for_node(node)
                    .as_variable_statement(*node)
                    .clone()
            };
            (FunctionDeclaration) => {
                self.store_for_node(node)
                    .as_function_declaration(*node)
                    .clone()
            };
            (ModuleDeclaration) => {
                self.store_for_node(node)
                    .as_module_declaration(*node)
                    .clone()
            };
            (ModuleBlock) => {
                self.store_for_node(node).as_module_block(*node).clone()
            };
            (BreakStatement) => {
                self.store_for_node(node).as_break_statement(*node).clone()
            };
            (ContinueStatement) => {
                self.store_for_node(node)
                    .as_continue_statement(*node)
                    .clone()
            };
            (ExportAssignment) => {
                self.store_for_node(node)
                    .as_export_assignment(*node)
                    .clone()
            };
            (ExportDeclaration) => {
                self.store_for_node(node)
                    .as_export_declaration(*node)
                    .clone()
            };
            (VariableDeclarationList) => {
                self.store_for_node(node)
                    .as_variable_declaration_list(*node)
                    .clone()
            };
            (VariableDeclaration) => {
                self.store_for_node(node)
                    .as_variable_declaration(*node)
                    .clone()
            };
            (ClassDeclaration) => {
                self.store_for_node(node)
                    .as_class_declaration(*node)
                    .clone()
            };
            (InterfaceDeclaration) => {
                self.store_for_node(node)
                    .as_interface_declaration(*node)
                    .clone()
            };
            (TypeAliasDeclaration) => {
                self.store_for_node(node)
                    .as_type_alias_declaration(*node)
                    .clone()
            };
            (PropertyDeclaration) => {
                self.store_for_node(node)
                    .as_property_declaration(*node)
                    .clone()
            };
            (MethodDeclaration) => {
                self.store_for_node(node)
                    .as_method_declaration(*node)
                    .clone()
            };
            (GetAccessorDeclaration) => {
                self.store_for_node(node)
                    .as_get_accessor_declaration(*node)
                    .clone()
            };
            (SetAccessorDeclaration) => {
                self.store_for_node(node)
                    .as_set_accessor_declaration(*node)
                    .clone()
            };
            (ConstructorDeclaration) => {
                self.store_for_node(node)
                    .as_constructor_declaration(*node)
                    .clone()
            };
            (PropertySignatureDeclaration) => {
                self.store_for_node(node)
                    .as_property_signature_declaration(*node)
                    .clone()
            };
            (MethodSignatureDeclaration) => {
                self.store_for_node(node)
                    .as_method_signature_declaration(*node)
                    .clone()
            };
            (CallSignatureDeclaration) => {
                self.store_for_node(node)
                    .as_call_signature_declaration(*node)
                    .clone()
            };
            (ConstructSignatureDeclaration) => {
                self.store_for_node(node)
                    .as_construct_signature_declaration(*node)
                    .clone()
            };
            (IndexSignatureDeclaration) => {
                self.store_for_node(node)
                    .as_index_signature_declaration(*node)
                    .clone()
            };
            (ParameterDeclaration) => {
                self.store_for_node(node)
                    .as_parameter_declaration(*node)
                    .clone()
            };
            (Block) => {
                self.store_for_node(node).as_block(*node).clone()
            };
            (ReturnStatement) => {
                self.store_for_node(node).as_return_statement(*node).clone()
            };
            (TypeParameterDeclaration) => {
                self.store_for_node(node)
                    .as_type_parameter_declaration(*node)
                    .clone()
            };
            (HeritageClause) => {
                self.store_for_node(node).as_heritage_clause(*node).clone()
            };
            (ExpressionWithTypeArguments) => {
                self.store_for_node(node)
                    .as_expression_with_type_arguments(*node)
                    .clone()
            };
            (QualifiedName) => {
                self.store_for_node(node).as_qualified_name(*node).clone()
            };
            (TypeReferenceNode) => {
                self.store_for_node(node)
                    .as_type_reference_node(*node)
                    .clone()
            };
            (TypeQueryNode) => {
                self.store_for_node(node).as_type_query_node(*node).clone()
            };
            (ArrayTypeNode) => {
                self.store_for_node(node).as_array_type_node(*node).clone()
            };
            (TupleTypeNode) => {
                self.store_for_node(node).as_tuple_type_node(*node).clone()
            };
            (UnionTypeNode) => {
                self.store_for_node(node).as_union_type_node(*node).clone()
            };
            (IntersectionTypeNode) => {
                self.store_for_node(node)
                    .as_intersection_type_node(*node)
                    .clone()
            };
            (ParenthesizedTypeNode) => {
                self.store_for_node(node)
                    .as_parenthesized_type_node(*node)
                    .clone()
            };
            (OptionalTypeNode) => {
                self.store_for_node(node)
                    .as_optional_type_node(*node)
                    .clone()
            };
            (RestTypeNode) => {
                self.store_for_node(node).as_rest_type_node(*node).clone()
            };
            (FunctionTypeNode) => {
                self.store_for_node(node)
                    .as_function_type_node(*node)
                    .clone()
            };
            (TypeLiteralNode) => {
                self.store_for_node(node)
                    .as_type_literal_node(*node)
                    .clone()
            };
            (TypePredicateNode) => {
                self.store_for_node(node)
                    .as_type_predicate_node(*node)
                    .clone()
            };
            (PropertyAssignment) => {
                self.store_for_node(node)
                    .as_property_assignment(*node)
                    .clone()
            };
            (VoidExpression) => {
                self.store_for_node(node).as_void_expression(*node).clone()
            };
            (PropertyAccessExpression) => {
                self.store_for_node(node)
                    .as_property_access_expression(*node)
                    .clone()
            };
            (ElementAccessExpression) => {
                self.store_for_node(node)
                    .as_element_access_expression(*node)
                    .clone()
            };
            (CallExpression) => {
                self.store_for_node(node).as_call_expression(*node).clone()
            };
            (SpreadElement) => {
                self.store_for_node(node).as_spread_element(*node).clone()
            };
            (NewExpression) => {
                self.store_for_node(node).as_new_expression(*node).clone()
            };
            (FunctionExpression) => {
                self.store_for_node(node)
                    .as_function_expression(*node)
                    .clone()
            };
            (ArrowFunction) => {
                self.store_for_node(node).as_arrow_function(*node).clone()
            };
            (ObjectLiteralExpression) => {
                self.store_for_node(node)
                    .as_object_literal_expression(*node)
                    .clone()
            };
            (ArrayLiteralExpression) => {
                self.store_for_node(node)
                    .as_array_literal_expression(*node)
                    .clone()
            };
            (BinaryExpression) => {
                self.store_for_node(node)
                    .as_binary_expression(*node)
                    .clone()
            };
            (ClassExpression) => {
                self.store_for_node(node).as_class_expression(*node).clone()
            };
            (SemicolonClassElement) => {
                self.store_for_node(node)
                    .as_semicolon_class_element(*node)
                    .clone()
            };
            (LiteralTypeNode) => {
                self.store_for_node(node)
                    .as_literal_type_node(*node)
                    .clone()
            };
            (ImportDeclaration) => {
                self.store_for_node(node)
                    .as_import_declaration(*node)
                    .clone()
            };
            (ImportClause) => {
                self.store_for_node(node).as_import_clause(*node).clone()
            };
            (NamedImports) => {
                self.store_for_node(node).as_named_imports(*node).clone()
            };
            (NamespaceImport) => {
                self.store_for_node(node).as_namespace_import(*node).clone()
            };
            (ImportSpecifier) => {
                self.store_for_node(node).as_import_specifier(*node).clone()
            };
            (NamedExports) => {
                self.store_for_node(node).as_named_exports(*node).clone()
            };
            (ExportSpecifier) => {
                self.store_for_node(node).as_export_specifier(*node).clone()
            };
            (PrefixUnaryExpression) => {
                self.store_for_node(node)
                    .as_prefix_unary_expression(*node)
                    .clone()
            };
            (PostfixUnaryExpression) => {
                self.store_for_node(node)
                    .as_postfix_unary_expression(*node)
                    .clone()
            };
            (ParenthesizedExpression) => {
                self.store_for_node(node)
                    .as_parenthesized_expression(*node)
                    .clone()
            };
        }

        match self.kind(node) {
            ast::Kind::SourceFile => {
                self.emit_source_file_node(node, writer);
            }
            ast::Kind::ExpressionStatement => {
                let data = payload!(ExpressionStatement);
                self.emit_expression_statement(node, &data, writer)
            }
            ast::Kind::ForStatement => {
                let data = payload!(ForStatement);
                self.emit_for_statement(node, &data, writer)
            }
            ast::Kind::VariableStatement => {
                let data = payload!(VariableStatement);
                self.emit_variable_statement(node, &data, writer)
            }
            ast::Kind::FunctionDeclaration => {
                let data = payload!(FunctionDeclaration);
                self.emit_function_declaration(node, &data, writer)
            }
            ast::Kind::ModuleDeclaration => {
                let data = payload!(ModuleDeclaration);
                self.emit_module_declaration(node, &data, writer)
            }
            ast::Kind::ModuleBlock => {
                let data = payload!(ModuleBlock);
                self.emit_module_block(node, &data, writer)
            }
            ast::Kind::Decorator => self.emit_decorator(node, writer),
            ast::Kind::EmptyStatement => self.emit_empty_statement(writer, false),
            ast::Kind::DebuggerStatement => self.emit_debugger_statement(writer),
            ast::Kind::LabeledStatement => self.emit_labeled_statement(node, writer),
            ast::Kind::IfStatement => self.emit_if_statement(node, writer),
            ast::Kind::DoStatement => self.emit_do_statement(node, writer),
            ast::Kind::WhileStatement => self.emit_while_statement(node, writer),
            ast::Kind::WithStatement => self.emit_with_statement(node, writer),
            ast::Kind::ForInStatement => self.emit_for_in_statement(node, writer),
            ast::Kind::ForOfStatement => self.emit_for_of_statement(node, writer),
            ast::Kind::SwitchStatement => self.emit_switch_statement(node, writer),
            ast::Kind::CaseBlock => self.emit_case_block(node, writer),
            ast::Kind::CaseClause => self.emit_case_clause(node, writer),
            ast::Kind::DefaultClause => self.emit_default_clause(node, writer),
            ast::Kind::ThrowStatement => self.emit_throw_statement(node, writer),
            ast::Kind::TryStatement => self.emit_try_statement(node, writer),
            ast::Kind::CatchClause => self.emit_catch_clause(node, writer),
            ast::Kind::NotEmittedStatement
            | ast::Kind::NotEmittedTypeElement
            | ast::Kind::MissingDeclaration => {}
            ast::Kind::BreakStatement => {
                self.emit_break_statement(node, writer);
            }
            ast::Kind::ContinueStatement => {
                self.emit_continue_statement(node, writer);
            }
            ast::Kind::ExportAssignment => {
                let data = payload!(ExportAssignment);
                self.emit_export_assignment(node, &data, writer)
            }
            ast::Kind::ExportDeclaration => {
                let data = payload!(ExportDeclaration);
                self.emit_export_declaration(node, &data, writer)
            }
            ast::Kind::NamespaceExportDeclaration => {
                self.emit_namespace_export_declaration(node, writer)
            }
            ast::Kind::ImportEqualsDeclaration => self.emit_import_equals_declaration(node, writer),
            ast::Kind::ExternalModuleReference => self.emit_external_module_reference(node, writer),
            ast::Kind::VariableDeclarationList => {
                let data = payload!(VariableDeclarationList);
                self.emit_variable_declaration_list(node, &data, writer)
            }
            ast::Kind::VariableDeclaration => {
                let data = payload!(VariableDeclaration);
                self.emit_variable_declaration(node, &data, writer)
            }
            ast::Kind::ClassDeclaration => {
                let data = payload!(ClassDeclaration);
                self.emit_class_declaration(node, &data, writer)
            }
            ast::Kind::InterfaceDeclaration => {
                let data = payload!(InterfaceDeclaration);
                self.emit_interface_declaration(node, &data, writer)
            }
            ast::Kind::TypeAliasDeclaration | ast::Kind::JSTypeAliasDeclaration => {
                let data = payload!(TypeAliasDeclaration);
                self.emit_type_alias_declaration(node, &data, writer)
            }
            ast::Kind::PropertyDeclaration => {
                let data = payload!(PropertyDeclaration);
                self.emit_property_declaration(node, &data, writer)
            }
            ast::Kind::MethodDeclaration => {
                let data = payload!(MethodDeclaration);
                self.emit_method_declaration(node, &data, writer)
            }
            ast::Kind::GetAccessor => {
                let data = payload!(GetAccessorDeclaration);
                self.emit_get_accessor_declaration(node, &data, writer)
            }
            ast::Kind::SetAccessor => {
                let data = payload!(SetAccessorDeclaration);
                self.emit_set_accessor_declaration(node, &data, writer)
            }
            ast::Kind::Constructor => {
                let data = payload!(ConstructorDeclaration);
                self.emit_constructor_declaration(node, &data, writer)
            }
            ast::Kind::PropertySignature => {
                let data = payload!(PropertySignatureDeclaration);
                self.emit_property_signature(node, &data, writer)
            }
            ast::Kind::MethodSignature => {
                let data = payload!(MethodSignatureDeclaration);
                self.emit_method_signature(node, &data, writer)
            }
            ast::Kind::CallSignature => {
                let data = payload!(CallSignatureDeclaration);
                self.emit_call_signature(node, &data, writer)
            }
            ast::Kind::ConstructSignature => {
                let data = payload!(ConstructSignatureDeclaration);
                self.emit_construct_signature(node, &data, writer)
            }
            ast::Kind::IndexSignature => {
                let data = payload!(IndexSignatureDeclaration);
                self.emit_index_signature(node, &data, writer)
            }
            ast::Kind::Parameter => {
                let data = payload!(ParameterDeclaration);
                self.emit_parameter(node, &data, writer)
            }
            ast::Kind::Block => self.emit_block_node(node, writer),
            ast::Kind::ReturnStatement => {
                let data = payload!(ReturnStatement);
                self.emit_return_statement(node, &data, writer)
            }
            ast::Kind::TypeParameter => {
                let data = payload!(TypeParameterDeclaration);
                self.emit_type_parameter(node, &data, writer)
            }
            ast::Kind::HeritageClause => {
                let data = payload!(HeritageClause);
                self.emit_heritage_clause(node, &data, writer)
            }
            ast::Kind::ExpressionWithTypeArguments => {
                let data = payload!(ExpressionWithTypeArguments);
                self.emit_expression_with_type_arguments(node, &data, writer)
            }
            ast::Kind::QualifiedName => {
                let data = payload!(QualifiedName);
                self.emit_qualified_name(node, &data, writer)
            }
            ast::Kind::TypeReference => {
                let data = payload!(TypeReferenceNode);
                self.emit_type_reference(node, &data, writer)
            }
            ast::Kind::TypeQuery => {
                let data = payload!(TypeQueryNode);
                self.emit_type_query(node, &data, writer)
            }
            ast::Kind::ArrayType => {
                let data = payload!(ArrayTypeNode);
                self.emit_array_type(node, &data, writer)
            }
            ast::Kind::TupleType => {
                let data = payload!(TupleTypeNode);
                self.emit_tuple_type(node, &data, writer)
            }
            ast::Kind::UnionType => {
                let data = payload!(UnionTypeNode);
                self.emit_union_type(node, &data, writer)
            }
            ast::Kind::IntersectionType => {
                let data = payload!(IntersectionTypeNode);
                self.emit_intersection_type(node, &data, writer)
            }
            ast::Kind::ParenthesizedType => {
                let data = payload!(ParenthesizedTypeNode);
                self.emit_parenthesized_type(node, &data, writer)
            }
            ast::Kind::OptionalType => {
                let data = payload!(OptionalTypeNode);
                self.emit_optional_type(node, &data, writer)
            }
            ast::Kind::RestType => {
                let data = payload!(RestTypeNode);
                self.emit_rest_type(node, &data, writer)
            }
            ast::Kind::FunctionType => {
                let data = payload!(FunctionTypeNode);
                self.emit_function_type(node, &data, writer)
            }
            ast::Kind::TypeLiteral => {
                let data = payload!(TypeLiteralNode);
                self.emit_type_literal(node, &data, writer)
            }
            ast::Kind::ConstructorType => self.emit_constructor_type(node, writer),
            ast::Kind::ConditionalType => self.emit_conditional_type(node, writer),
            ast::Kind::InferType => self.emit_infer_type(node, writer),
            ast::Kind::TypeOperator => self.emit_type_operator(node, writer),
            ast::Kind::IndexedAccessType => self.emit_indexed_access_type(node, writer),
            ast::Kind::MappedType => self.emit_mapped_type(node, writer),
            ast::Kind::NamedTupleMember => self.emit_named_tuple_member(node, writer),
            ast::Kind::TemplateLiteralType => self.emit_template_literal_type(node, writer),
            ast::Kind::TemplateLiteralTypeSpan => {
                self.emit_template_literal_type_span(node, writer)
            }
            ast::Kind::ImportType => self.emit_import_type(node, writer),
            ast::Kind::TypePredicate => {
                let data = payload!(TypePredicateNode);
                self.emit_type_predicate(node, &data, writer)
            }
            ast::Kind::ThisType => writer.write_keyword("this"),
            ast::Kind::PropertyAssignment => {
                let data = payload!(PropertyAssignment);
                self.emit_property_assignment(node, &data, writer)
            }
            ast::Kind::ShorthandPropertyAssignment => {
                self.emit_shorthand_property_assignment(node, writer)
            }
            ast::Kind::SpreadAssignment => self.emit_spread_assignment(node, writer),
            ast::Kind::ComputedPropertyName => self.emit_computed_property_name(node, writer),
            ast::Kind::ObjectBindingPattern => self.emit_object_binding_pattern(node, writer),
            ast::Kind::ArrayBindingPattern => self.emit_array_binding_pattern(node, writer),
            ast::Kind::BindingElement => self.emit_binding_element(node, writer),
            ast::Kind::Identifier => self.emit_identifier_name(node, writer),
            ast::Kind::PrivateIdentifier => self.emit_private_identifier(node, writer),
            ast::Kind::JsxNamespacedName => self.emit_jsx_namespaced_name(node, writer),
            ast::Kind::JsxText | ast::Kind::JsxTextAllWhiteSpaces => {
                writer.write_literal(&self.store_for_node(node).text(*node));
            }
            ast::Kind::StringLiteral
            | ast::Kind::NumericLiteral
            | ast::Kind::BigIntLiteral
            | ast::Kind::NoSubstitutionTemplateLiteral
            | ast::Kind::RegularExpressionLiteral
            | ast::Kind::TemplateHead
            | ast::Kind::TemplateMiddle
            | ast::Kind::TemplateTail => {
                let current_source_file = share_source_file_option(&self.current_source_file);
                writer.write_literal(&self.get_literal_text_of_node(
                    node,
                    current_source_file.as_ref(),
                    GetLiteralTextFlags::NONE,
                ));
            }
            ast::Kind::TrueKeyword
            | ast::Kind::FalseKeyword
            | ast::Kind::NullKeyword
            | ast::Kind::ThisKeyword
            | ast::Kind::SuperKeyword
            | ast::Kind::AssertsKeyword
            | ast::Kind::AnyKeyword
            | ast::Kind::UnknownKeyword
            | ast::Kind::NumberKeyword
            | ast::Kind::BigIntKeyword
            | ast::Kind::ObjectKeyword
            | ast::Kind::BooleanKeyword
            | ast::Kind::StringKeyword
            | ast::Kind::SymbolKeyword
            | ast::Kind::VoidKeyword
            | ast::Kind::UndefinedKeyword
            | ast::Kind::NeverKeyword
            | ast::Kind::IntrinsicKeyword => {
                self.emit_token_text_to_writer(self.kind(node), WriteKind::Keyword, writer)
            }
            ast::Kind::VoidExpression => {
                let data = payload!(VoidExpression);
                self.emit_void_expression(node, &data, writer)
            }
            ast::Kind::PropertyAccessExpression => {
                let data = payload!(PropertyAccessExpression);
                self.emit_property_access_expression(node, &data, writer)
            }
            ast::Kind::ElementAccessExpression => {
                let data = payload!(ElementAccessExpression);
                self.emit_element_access_expression(node, &data, writer)
            }
            ast::Kind::CallExpression => {
                let data = payload!(CallExpression);
                self.emit_call_expression(node, &data, writer)
            }
            ast::Kind::TaggedTemplateExpression => {
                self.emit_tagged_template_expression(node, writer)
            }
            ast::Kind::TemplateExpression => self.emit_template_expression(node, writer),
            ast::Kind::TemplateSpan => self.emit_template_span(node, writer),
            ast::Kind::SpreadElement => {
                let data = payload!(SpreadElement);
                self.emit_spread_element(node, &data, writer)
            }
            ast::Kind::NewExpression => {
                let data = payload!(NewExpression);
                self.emit_new_expression(node, &data, writer)
            }
            ast::Kind::FunctionExpression => {
                let data = payload!(FunctionExpression);
                self.emit_function_expression(node, &data, writer)
            }
            ast::Kind::ArrowFunction => {
                let data = payload!(ArrowFunction);
                self.emit_arrow_function(node, &data, writer)
            }
            ast::Kind::ObjectLiteralExpression => {
                let data = payload!(ObjectLiteralExpression);
                self.emit_object_literal_expression(node, &data, writer)
            }
            ast::Kind::ArrayLiteralExpression => {
                let data = payload!(ArrayLiteralExpression);
                self.emit_array_literal_expression(node, &data, writer)
            }
            ast::Kind::BinaryExpression => {
                let data = payload!(BinaryExpression);
                self.emit_binary_expression(node, &data, writer)
            }
            ast::Kind::ClassExpression => {
                let data = payload!(ClassExpression);
                self.emit_class_expression(node, &data, writer)
            }
            ast::Kind::EnumDeclaration => self.emit_enum_declaration(node, writer),
            ast::Kind::EnumMember => self.emit_enum_member(node, writer),
            ast::Kind::ClassStaticBlockDeclaration => {
                self.emit_class_static_block_declaration(node, writer)
            }
            ast::Kind::SemicolonClassElement => {
                let data = payload!(SemicolonClassElement);
                self.emit_semicolon_class_element(&data, writer)
            }
            ast::Kind::LiteralType => self.emit_literal_type(node, writer),
            ast::Kind::ImportDeclaration | ast::Kind::JSImportDeclaration => {
                let data = payload!(ImportDeclaration);
                self.emit_import_declaration(node, &data, writer)
            }
            ast::Kind::ImportClause => {
                let data = payload!(ImportClause);
                self.emit_import_clause(node, &data, writer)
            }
            ast::Kind::NamedImports => {
                let data = payload!(NamedImports);
                self.emit_named_imports(node, &data, writer)
            }
            ast::Kind::NamespaceImport => {
                let data = payload!(NamespaceImport);
                self.emit_namespace_import(node, &data, writer)
            }
            ast::Kind::ImportSpecifier => {
                let data = payload!(ImportSpecifier);
                self.emit_import_specifier(node, &data, writer)
            }
            ast::Kind::NamedExports => {
                let data = payload!(NamedExports);
                self.emit_named_exports(node, &data, writer)
            }
            ast::Kind::NamespaceExport => self.emit_namespace_export(node, writer),
            ast::Kind::ExportSpecifier => {
                let data = payload!(ExportSpecifier);
                self.emit_export_specifier(node, &data, writer)
            }
            ast::Kind::ImportAttributes => self.emit_import_attributes(node, writer),
            ast::Kind::ImportAttribute => self.emit_import_attribute(node, writer),
            ast::Kind::PrefixUnaryExpression => self.emit_prefix_unary_expression(node, writer),
            ast::Kind::PostfixUnaryExpression => {
                let data = payload!(PostfixUnaryExpression);
                self.emit_postfix_unary_expression(node, &data, writer)
            }
            ast::Kind::ParenthesizedExpression => {
                let data = payload!(ParenthesizedExpression);
                self.emit_parenthesized_expression(node, &data, writer)
            }
            ast::Kind::TypeAssertionExpression => self.emit_type_assertion_expression(node, writer),
            ast::Kind::ConditionalExpression => self.emit_conditional_expression(node, writer),
            ast::Kind::AsExpression => self.emit_assertion_like_expression(node, "as", writer),
            ast::Kind::SatisfiesExpression => {
                self.emit_assertion_like_expression(node, "satisfies", writer)
            }
            ast::Kind::NonNullExpression => self.emit_non_null_expression(node, writer),
            ast::Kind::OmittedExpression => {}
            ast::Kind::MetaProperty => self.emit_meta_property(node, writer),
            ast::Kind::PartiallyEmittedExpression => {
                self.emit_partially_emitted_expression(node, writer)
            }
            ast::Kind::SyntheticExpression => self.emit_synthetic_expression(node, writer),
            ast::Kind::SyntheticReferenceExpression => {
                self.emit_synthetic_reference_expression(node, writer)
            }
            ast::Kind::DeleteExpression => {
                self.emit_unary_keyword_expression(ast::Kind::DeleteKeyword, node, writer)
            }
            ast::Kind::TypeOfExpression => {
                self.emit_unary_keyword_expression(ast::Kind::TypeOfKeyword, node, writer)
            }
            ast::Kind::AwaitExpression => {
                self.emit_unary_keyword_expression(ast::Kind::AwaitKeyword, node, writer)
            }
            ast::Kind::YieldExpression => self.emit_yield_expression(node, writer),
            ast::Kind::JsxElement => self.emit_jsx_element(node, writer),
            ast::Kind::JsxSelfClosingElement => self.emit_jsx_self_closing_element(node, writer),
            ast::Kind::JsxOpeningElement => self.emit_jsx_opening_element(node, writer),
            ast::Kind::JsxClosingElement => self.emit_jsx_closing_element(node, writer),
            ast::Kind::JsxFragment => self.emit_jsx_fragment(node, writer),
            ast::Kind::JsxOpeningFragment => {
                writer.write_punctuation("<");
                writer.write_punctuation(">");
            }
            ast::Kind::JsxClosingFragment => {
                writer.write_punctuation("</");
                writer.write_punctuation(">");
            }
            ast::Kind::JsxAttribute => self.emit_jsx_attribute(node, writer),
            ast::Kind::JsxAttributes => self.emit_jsx_attributes(node, writer),
            ast::Kind::JsxSpreadAttribute => self.emit_jsx_spread_attribute(node, writer),
            ast::Kind::JsxExpression => self.emit_jsx_expression(node, writer),
            kind if ast::is_keyword_kind(kind) => {
                self.emit_token_text_to_writer(kind, WriteKind::Keyword, writer)
            }
            kind if is_punctuation_kind(kind) => {
                self.emit_token_text_to_writer(kind, WriteKind::Punctuation, writer)
            }
            _ => panic!("unhandled Node: {}", self.kind(node)),
        };
        if let Some(state) = state {
            self.exit_node_to_writer(node, state, writer);
        }
    }

    fn emit_token_text_to_writer(
        &mut self,
        token: ast::Kind,
        write_kind: WriteKind,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.write_token_text_to_writer(token, write_kind, -1, writer);
    }

    fn write_token_text_to_writer(
        &mut self,
        token: ast::Kind,
        write_kind: WriteKind,
        pos: i32,
        writer: &mut dyn EmitTextWriter,
    ) -> i32 {
        let token_string = scanner::token_to_string(token);
        match write_kind {
            WriteKind::None => writer.write(&token_string),
            WriteKind::Keyword => writer.write_keyword(&token_string),
            WriteKind::Operator => writer.write_operator(&token_string),
            WriteKind::Punctuation => writer.write_punctuation(&token_string),
            WriteKind::StringLiteral => writer.write_string_literal(&token_string),
            WriteKind::Parameter => writer.write_parameter(&token_string),
            WriteKind::Property => writer.write_property(&token_string),
            WriteKind::Comment => writer.write_comment(&token_string),
            WriteKind::Literal => writer.write_literal(&token_string),
        }
        if ast::position_is_synthesized(pos) {
            pos
        } else {
            pos + token_string.len() as i32
        }
    }

    fn emit_token_with_comment_to_writer(
        &mut self,
        token: ast::Kind,
        mut pos: i32,
        write_kind: WriteKind,
        context_node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) -> i32 {
        let start_pos = pos;
        if let Some(current_source_file) = self.current_source_file.as_ref()
            && !ast::position_is_synthesized(pos)
        {
            pos = scanner::skip_trivia(current_source_file.text(), pos.max(0) as usize) as i32;
        }

        let parse_node = self.emit_context.parse_node(context_node);
        let is_similar_node = parse_node
            .as_ref()
            .is_some_and(|node| self.kind(node) == self.kind(context_node));
        if is_similar_node && self.loc(context_node).pos() != start_pos {
            self.emit_leading_comments_to_writer(start_pos, false, writer);
        }

        let end_pos = self.write_token_text_to_writer(token, write_kind, pos, writer);
        if is_similar_node && self.loc(context_node).end() != end_pos {
            let separator = if self.kind(context_node) == ast::Kind::JsxExpression {
                CommentSeparator::None
            } else {
                CommentSeparator::Before
            };
            self.emit_trailing_comments_to_writer(end_pos, separator, writer);
        }
        end_pos
    }

    fn emit_token_ex_to_writer(
        &mut self,
        token: ast::Kind,
        pos: i32,
        write_kind: WriteKind,
        context_node: &ast::Node,
        flags: TokenEmitFlags,
        writer: &mut dyn EmitTextWriter,
    ) -> i32 {
        let (state, pos) = self.enter_token_to_writer(token, pos, context_node, flags, writer);
        let pos = self.write_token_text_to_writer(token, write_kind, pos, writer);
        self.exit_token_to_writer(token, pos, context_node, state, writer);
        pos
    }

    fn enter_token_to_writer(
        &mut self,
        token: ast::Kind,
        pos: i32,
        context_node: &ast::Node,
        flags: TokenEmitFlags,
        writer: &mut dyn EmitTextWriter,
    ) -> (PrinterState, i32) {
        let mut state = PrinterState::default();
        let (comment_state, pos) =
            self.emit_comments_before_token_to_writer(token, pos, context_node, flags, writer);
        state.comment_state = comment_state;
        state.source_map_state =
            self.emit_source_maps_before_token_to_writer(token, pos, context_node, flags, writer);
        (state, pos)
    }

    fn exit_token_to_writer(
        &mut self,
        token: ast::Kind,
        pos: i32,
        context_node: &ast::Node,
        previous_state: PrinterState,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_source_maps_after_token_to_writer(
            token,
            pos,
            context_node,
            previous_state.source_map_state,
            writer,
        );
        self.emit_comments_after_token_to_writer(
            token,
            pos,
            context_node,
            previous_state.comment_state,
            writer,
        );
    }

    fn token_end_at_node_pos(&self, node: &ast::Node) -> i32 {
        if let Some(source_file) = self.current_source_file.as_ref()
            && !ast::position_is_synthesized(self.loc(node).pos())
        {
            let pos =
                scanner::skip_trivia(source_file.text(), self.loc(node).pos().max(0) as usize);
            return scanner::get_range_of_token_at_position(source_file, pos).end();
        }
        self.loc(node).end()
    }

    fn token_end_before_trailing_comments(&self, node: &ast::Node) -> i32 {
        let Some(source_file) = self.current_source_file.as_ref() else {
            return self.token_end_at_node_pos(node);
        };
        let loc = self.loc(node);
        if ast::position_is_synthesized(loc.pos()) || ast::position_is_synthesized(loc.end()) {
            return self.token_end_at_node_pos(node);
        }

        let text = source_file.text();
        let start = scanner::skip_trivia(text, loc.pos().max(0) as usize);
        let limit = loc.end().max(start as i32) as usize;
        let mut scanner =
            scanner::Scanner::new(text.to_string(), core::SCRIPT_TARGET_LATEST_STANDARD);
        scanner.language_variant = source_file.data().language_variant();
        scanner.pos = start;

        let mut last_end = start;
        loop {
            let token = scanner.scan();
            if token == ast::Kind::EndOfFile || scanner.token_start >= limit {
                break;
            }
            if scanner.pos <= limit {
                last_end = scanner.pos;
            } else {
                break;
            }
        }

        last_end as i32
    }

    fn emit_source_file_node(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let saved_current_source_file = share_source_file_option(&self.current_source_file);
        let saved_comments_disabled = self.comments_disabled;

        let (
            statements,
            statement_list_loc,
            script_kind,
            is_declaration_file,
            triple_slash_directives,
        ) = {
            let source = self.store_for_node(node);
            let source_file = source.as_source_file(*node);
            let statement_list = source.parser_access().source_file_statement_list(*node);
            (
                statement_list.iter().collect::<Vec<_>>(),
                statement_list.loc(),
                source_file.script_kind(),
                source_file.is_declaration_file(),
                (
                    source_file.referenced_files().to_vec(),
                    source_file.type_reference_directives().to_vec(),
                    source_file.lib_reference_directives().to_vec(),
                ),
            )
        };

        writer.write_line();
        self.push_name_generation_scope(node);
        self.generate_all_names(&statements);

        let detached_state;
        let index;
        if script_kind != core::ScriptKind::JSON {
            self.emit_shebang_if_needed(node, writer);
            index = self.emit_prologue_directives(&statements, writer);
            if !writer.is_at_start_of_line() {
                writer.write_line();
            }
            detached_state =
                self.emit_detached_comments_before_statement_list(node, statement_list_loc, writer);
            self.emit_helpers(node, writer);
            if is_declaration_file {
                self.emit_triple_slash_directives(&triple_slash_directives, writer);
            }
        } else {
            index = 0;
            detached_state =
                self.emit_detached_comments_before_statement_list(node, statement_list_loc, writer);
        }

        self.emit_source_file_statements(node, &statements, statement_list_loc, index, writer);
        self.pop_name_generation_scope(node);
        self.emit_detached_comments_after_statement_list(
            node,
            statement_list_loc,
            detached_state,
            writer,
        );

        self.current_source_file = saved_current_source_file;
        self.name_generator.set_source_file(
            self.current_source_file.as_ref(),
            self.print_handlers.has_global_name,
        );
        self.comments_disabled = saved_comments_disabled;
    }

    fn emit_shebang_if_needed(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let store = self.store_for_node(node);
        if ast::node_is_synthesized(store, *node) {
            return;
        }
        let shebang = scanner::get_shebang(store.as_source_file(*node).text());
        if !shebang.is_empty() {
            writer.write_comment(&shebang);
            writer.write_line();
        }
    }

    fn should_reuse_temp_variable_scope(&mut self, node: &ast::Node) -> bool {
        self.emit_context.emit_flags(node) & EF_REUSE_TEMP_VARIABLE_SCOPE != 0
    }

    fn push_name_generation_scope(&mut self, node: &ast::Node) {
        let reuse_temp_variable_scope = self.should_reuse_temp_variable_scope(node);
        self.name_generator.push_scope(reuse_temp_variable_scope);
    }

    fn pop_name_generation_scope(&mut self, node: &ast::Node) {
        let reuse_temp_variable_scope = self.should_reuse_temp_variable_scope(node);
        self.name_generator.pop_scope(reuse_temp_variable_scope);
    }

    fn generate_all_names(&mut self, nodes: &[ast::Node]) {
        for node in nodes {
            self.generate_names(*node);
        }
    }

    fn generate_names_opt(&mut self, node: Option<ast::Node>) {
        if let Some(node) = node {
            self.generate_names(node);
        }
    }

    fn generate_names(&mut self, node: ast::Node) {
        match self.kind(&node) {
            ast::Kind::Block | ast::Kind::CaseClause | ast::Kind::DefaultClause => {
                let nodes =
                    Self::optional_node_list(self.store_for_node(&node).source_statements(node));
                self.generate_all_names(&nodes);
            }
            ast::Kind::LabeledStatement
            | ast::Kind::WithStatement
            | ast::Kind::DoStatement
            | ast::Kind::WhileStatement => {
                self.generate_names_opt(self.store_for_node(&node).statement(node));
            }
            ast::Kind::IfStatement => {
                let then_statement = self.store_for_node(&node).then_statement(node);
                let else_statement = self.store_for_node(&node).else_statement(node);
                self.generate_names_opt(then_statement);
                self.generate_names_opt(else_statement);
            }
            ast::Kind::ForStatement | ast::Kind::ForOfStatement | ast::Kind::ForInStatement => {
                let initializer = self.store_for_node(&node).initializer(node);
                let statement = self.store_for_node(&node).statement(node);
                self.generate_names_opt(initializer);
                self.generate_names_opt(statement);
            }
            ast::Kind::SwitchStatement => {
                self.generate_names_opt(self.store_for_node(&node).case_block(node));
            }
            ast::Kind::CaseBlock => {
                let nodes =
                    Self::optional_node_list(self.store_for_node(&node).source_clauses(node));
                self.generate_all_names(&nodes);
            }
            ast::Kind::TryStatement => {
                let try_block = self.store_for_node(&node).try_block(node);
                let catch_clause = self.store_for_node(&node).catch_clause(node);
                let finally_block = self.store_for_node(&node).finally_block(node);
                self.generate_names_opt(try_block);
                self.generate_names_opt(catch_clause);
                self.generate_names_opt(finally_block);
            }
            ast::Kind::CatchClause => {
                let variable_declaration = self.store_for_node(&node).variable_declaration(node);
                let block = self.store_for_node(&node).block(node);
                self.generate_names_opt(variable_declaration);
                self.generate_names_opt(block);
            }
            ast::Kind::VariableStatement => {
                self.generate_names_opt(self.store_for_node(&node).declaration_list(node));
            }
            ast::Kind::VariableDeclarationList => {
                let nodes =
                    Self::optional_node_list(self.store_for_node(&node).source_declarations(node));
                self.generate_all_names(&nodes);
            }
            ast::Kind::VariableDeclaration
            | ast::Kind::Parameter
            | ast::Kind::BindingElement
            | ast::Kind::ClassDeclaration => {
                self.generate_name_if_needed(self.store_for_node(&node).name(node));
            }
            ast::Kind::FunctionDeclaration => {
                self.generate_name_if_needed(self.store_for_node(&node).name(node));
                if self.should_reuse_temp_variable_scope(&node) {
                    let parameters = Self::optional_node_list(
                        self.store_for_node(&node).source_parameters(node),
                    );
                    let body = self.store_for_node(&node).body(node);
                    self.generate_all_names(&parameters);
                    self.generate_names_opt(body);
                }
            }
            ast::Kind::ObjectBindingPattern | ast::Kind::ArrayBindingPattern => {
                let nodes =
                    Self::optional_node_list(self.store_for_node(&node).source_elements(node));
                self.generate_all_names(&nodes);
            }
            ast::Kind::ImportDeclaration | ast::Kind::JSImportDeclaration => {
                self.generate_names_opt(self.store_for_node(&node).import_clause(node));
            }
            ast::Kind::ImportClause => {
                let name = self.store_for_node(&node).name(node);
                let named_bindings = self.store_for_node(&node).named_bindings(node);
                self.generate_name_if_needed(name);
                self.generate_names_opt(named_bindings);
            }
            ast::Kind::NamespaceImport | ast::Kind::NamespaceExport => {
                self.generate_name_if_needed(self.store_for_node(&node).name(node));
            }
            ast::Kind::NamedImports => {
                let nodes =
                    Self::optional_node_list(self.store_for_node(&node).source_elements(node));
                self.generate_all_names(&nodes);
            }
            ast::Kind::ImportSpecifier => {
                let name = self
                    .store_for_node(&node)
                    .property_name(node)
                    .or_else(|| self.store_for_node(&node).name(node));
                self.generate_name_if_needed(name);
            }
            _ => {}
        }
    }

    fn generate_all_member_names(&mut self, nodes: &[ast::Node]) {
        for node in nodes {
            self.generate_member_names(*node);
        }
    }

    fn generate_member_names(&mut self, node: ast::Node) {
        match self.kind(&node) {
            ast::Kind::PropertyAssignment
            | ast::Kind::ShorthandPropertyAssignment
            | ast::Kind::PropertyDeclaration
            | ast::Kind::PropertySignature
            | ast::Kind::MethodDeclaration
            | ast::Kind::MethodSignature
            | ast::Kind::GetAccessor
            | ast::Kind::SetAccessor => {
                self.generate_name_if_needed(self.store_for_node(&node).name(node));
            }
            _ => {}
        }
    }

    fn generate_name_if_needed(&mut self, name: Option<ast::Node>) {
        let Some(name) = name else {
            return;
        };
        let store = self.store_for_node(&name);
        if ast::is_member_name(store, name) {
            self.generate_name(&name);
        } else if ast::is_binding_pattern(store, name) {
            self.generate_names(name);
        }
    }

    fn generate_name(&mut self, name: &ast::Node) {
        let emit_context = &self.emit_context;
        let store = emit_context.store_for_node(*name);
        let binding_facts = self.binding_facts_for_node_owned(name);
        let name_generator = &mut self.name_generator;
        let _ = name_generator.generate_name_with_resolver_and_binding_facts(
            store,
            name,
            |node| emit_context.store_for_node(node),
            binding_facts
                .as_deref()
                .map(|facts| facts as &dyn LocalNameBindingFacts),
        );
    }

    fn emit_source_file_statements(
        &mut self,
        parent_node: &ast::Node,
        statements: &[ast::Node],
        statement_list_loc: core::TextRange,
        start: usize,
        writer: &mut dyn EmitTextWriter,
    ) {
        if start >= statements.len() {
            return;
        }
        self.emit_list_items(
            parent_node,
            &statements[start..],
            ListFormat::MULTI_LINE,
            false,
            statement_list_loc,
            writer,
        );
    }

    fn emit_prologue_directives(
        &mut self,
        statements: &[ast::Node],
        writer: &mut dyn EmitTextWriter,
    ) -> usize {
        let mut index = 0;
        while index < statements.len()
            && ast::is_prologue_directive(
                self.store_for_node(&statements[index]),
                statements[index],
            )
        {
            writer.write_line();
            self.write_node_worker(Some(&statements[index]), writer);
            index += 1;
        }
        index
    }

    fn emit_helpers(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) -> bool {
        let should_skip = self.options.no_emit_helpers
            || self
                .current_source_file
                .as_ref()
                .map(|source_file| self.emit_context.has_recorded_external_helpers(source_file))
                .unwrap_or(false);
        let mut helpers = self
            .emit_context
            .get_emit_helpers(node)
            .into_iter()
            .map(helper_from_key)
            .collect::<Vec<_>>();
        if helpers.is_empty() {
            return false;
        }

        helpers.sort_by(|left, right| compare_emit_helpers(left, right).cmp(&0));
        let mut emitted = false;
        for helper in helpers {
            if !helper.scoped && should_skip {
                continue;
            }
            let text = self.emit_helper_text(helper);
            self.write_lines_to_writer(&text, writer);
            emitted = true;
        }
        emitted
    }

    fn emit_helper_text(&mut self, helper: &EmitHelper) -> String {
        if let Some(text_callback) = helper.text_callback {
            let mut helper_name_generator = self.name_generator.clone();
            let mut make_unique_name =
                |name: &str| helper_name_generator.make_file_level_optimistic_unique_name(name);
            text_callback(&mut make_unique_name)
        } else {
            helper.text.to_string()
        }
    }

    fn write_lines_to_writer(&mut self, text: &str, writer: &mut dyn EmitTextWriter) {
        let lines = stringutil::split_lines(text);
        let indentation = stringutil::guess_indentation(&lines);
        for mut line in lines {
            if indentation > 0 {
                line = &line[indentation..];
            }
            if !line.is_empty() {
                writer.write_line();
                writer.write(line);
            } else {
                writer.write_line();
            }
        }
    }

    fn emit_triple_slash_directives(
        &mut self,
        refs: &(
            Vec<ast::FileReference>,
            Vec<ast::FileReference>,
            Vec<ast::FileReference>,
        ),
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_directive("path", &refs.0, writer);
        self.emit_directive("types", &refs.1, writer);
        self.emit_directive("lib", &refs.2, writer);
    }

    fn emit_directive(
        &mut self,
        kind: &str,
        refs: &[ast::FileReference],
        writer: &mut dyn EmitTextWriter,
    ) {
        for file_ref in refs {
            let resolution_mode = if file_ref.resolution_mode != core::ResolutionMode::None {
                let mode = if file_ref.resolution_mode == core::ResolutionMode::ESNext {
                    "import"
                } else {
                    "require"
                };
                format!("resolution-mode=\"{}\" ", mode)
            } else {
                String::new()
            };
            let preserve = if file_ref.preserve {
                "preserve=\"true\" "
            } else {
                ""
            };
            writer.write_comment(&format!(
                "/// <reference {}=\"{}\" {}{}/>",
                kind, file_ref.file_name, resolution_mode, preserve
            ));
            writer.write_line();
        }
    }

    fn emit_detached_comments_before_statement_list(
        &mut self,
        node: &ast::Node,
        detached_range: core::TextRange,
        writer: &mut dyn EmitTextWriter,
    ) -> Option<CommentState> {
        if !self.should_emit_detached_comments(node) {
            return None;
        }

        let emit_flags = self.emit_context.emit_flags(node);
        let container_pos = self.container_pos;
        let container_end = self.container_end;
        let declaration_list_container_end = self.declaration_list_container_end;
        let skip_leading_comments = ast::position_is_synthesized(detached_range.pos())
            || emit_flags & EF_NO_LEADING_COMMENTS != 0;

        if !skip_leading_comments {
            self.emit_detached_comments_and_update_comments_info(detached_range, writer);
        }

        if emit_flags & EF_NO_NESTED_COMMENTS != 0 {
            self.comments_disabled = true;
        }

        Some(CommentState {
            emit_flags,
            comment_range: detached_range,
            container_pos,
            container_end,
            declaration_list_container_end,
        })
    }

    fn emit_detached_comments_after_statement_list(
        &mut self,
        _node: &ast::Node,
        detached_range: core::TextRange,
        state: Option<CommentState>,
        writer: &mut dyn EmitTextWriter,
    ) {
        let Some(state) = state else {
            return;
        };

        let skip_trailing_comments = self.comments_disabled
            || ast::position_is_synthesized(detached_range.end())
            || state.emit_flags & EF_NO_TRAILING_COMMENTS != 0;

        if !skip_trailing_comments {
            let has_written_comment =
                self.emit_leading_comments_to_writer(detached_range.end(), false, writer);
            if has_written_comment && !writer.is_at_start_of_line() {
                writer.write_line();
            }
        }

        if state.emit_flags & EF_NO_NESTED_COMMENTS != 0 {
            self.comments_disabled = false;
        }
    }

    fn emit_detached_comments_and_update_comments_info(
        &mut self,
        text_range: core::TextRange,
        writer: &mut dyn EmitTextWriter,
    ) {
        if let Some(info) = self.emit_detached_comments(text_range, writer) {
            self.detached_comments_info.push(info);
        }
    }

    fn emit_detached_comments(
        &mut self,
        text_range: core::TextRange,
        writer: &mut dyn EmitTextWriter,
    ) -> Option<DetachedCommentsInfo> {
        let source_file = self.current_source_file.as_ref()?;
        let text = source_file.text();
        let line_map = source_file.ecma_line_map();

        let leading_comments = if self.comments_disabled {
            if text_range.pos() == 0 {
                scanner::get_leading_comment_ranges(text, text_range.pos())
                    .into_iter()
                    .filter(|comment| is_pinned_comment(text, *comment))
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            }
        } else {
            scanner::get_leading_comment_ranges(text, text_range.pos())
        };

        if leading_comments.is_empty() {
            return None;
        }

        let mut detached_comments = Vec::new();
        let mut last_comment: Option<ast::CommentRange> = None;
        for comment in leading_comments.iter().copied() {
            if let Some(last_comment) = last_comment {
                let last_comment_line =
                    scanner::compute_line_of_position(&line_map, last_comment.end() as usize);
                let comment_line =
                    scanner::compute_line_of_position(&line_map, comment.pos() as usize);
                if comment_line >= last_comment_line + 2 {
                    break;
                }
            }

            if self.should_write_comment(comment) {
                detached_comments.push(comment);
            }
            last_comment = Some(comment);
        }

        let last_detached_comment = detached_comments.last().copied()?;
        let last_comment_line =
            scanner::compute_line_of_position(&line_map, last_detached_comment.end() as usize);
        let node_line = scanner::compute_line_of_position(
            &line_map,
            scanner::skip_trivia(text, text_range.pos().max(0) as usize),
        );
        if node_line < last_comment_line + 2 {
            return None;
        }

        if self.should_emit_new_line_before_leading_comment_of_position(
            text_range.pos(),
            leading_comments[0].pos(),
        ) {
            writer.write_line();
        }

        self.emit_comments_to_writer(detached_comments, CommentSeparator::After, writer);
        Some(DetachedCommentsInfo {
            node_pos: text_range.pos(),
            detached_comment_end_pos: last_detached_comment.end(),
        })
    }

    fn emit_leading_comments_to_writer(
        &mut self,
        mut pos: i32,
        elided: bool,
        writer: &mut dyn EmitTextWriter,
    ) -> bool {
        if self.comments_disabled
            || self.current_source_file.is_none()
            || ast::position_is_synthesized(pos)
            || pos == self.container_pos
        {
            return false;
        }

        let mut triple_slash = core::TSUnknown;
        if !elided {
            if pos == 0
                && self
                    .current_source_file
                    .as_ref()
                    .is_some_and(|source_file| source_file.data().is_declaration_file())
            {
                triple_slash = core::TSFalse;
            }
        } else if pos == 0 {
            triple_slash = core::TSTrue;
        } else {
            return false;
        }

        if self.detached_comments_info.len() > 0 {
            let info = self.detached_comments_info.peek();
            if info.node_pos == pos {
                pos = self.detached_comments_info.pop().detached_comment_end_pos;
            }
        }

        let source_file = self.current_source_file.as_ref().unwrap();
        let comments = scanner::get_leading_comment_ranges(source_file.text(), pos)
            .into_iter()
            .filter(|comment| {
                self.should_write_comment(*comment)
                    && self.should_emit_comment_if_triple_slash(*comment, triple_slash)
            })
            .collect::<Vec<_>>();

        if !comments.is_empty()
            && self.should_emit_new_line_before_leading_comment_of_position(pos, comments[0].pos())
        {
            writer.write_line();
        }

        self.emit_comments_to_writer(comments, CommentSeparator::After, writer)
    }

    fn emit_comments_to_writer(
        &mut self,
        comments: Vec<ast::CommentRange>,
        comment_separator: CommentSeparator,
        writer: &mut dyn EmitTextWriter,
    ) -> bool {
        let mut intervening_separator = false;
        if comments.is_empty() {
            return false;
        }
        if comment_separator == CommentSeparator::Before {
            writer.write_space(" ");
        }

        for comment in comments {
            if intervening_separator {
                writer.write_space(" ");
                intervening_separator = false;
            }

            self.emit_comment_to_writer(comment, writer);

            if comment.kind == ast::Kind::SingleLineCommentTrivia
                || (comment.has_trailing_new_line && comment_separator != CommentSeparator::None)
            {
                writer.write_line();
            } else {
                intervening_separator = comment_separator != CommentSeparator::None;
            }
        }

        if intervening_separator && comment_separator == CommentSeparator::After {
            writer.write_space(" ");
        }

        true
    }

    fn emit_comment_to_writer(
        &mut self,
        comment: ast::CommentRange,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_pos_to_writer(comment.pos(), writer);
        if let Some(current_source_file) = self.current_source_file.as_ref() {
            let text = current_source_file.text().to_string();
            let line_map = current_source_file.ecma_line_map();
            self.write_comment_range_worker_to_writer(
                &text,
                &line_map,
                comment.kind,
                comment.text_range,
                writer,
            );
        }
        self.emit_pos_to_writer(comment.end(), writer);
    }

    fn enter_node_to_writer(
        &mut self,
        node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) -> PrinterState {
        let mut state = PrinterState::default();

        if let Some(on_before_emit_node) = self.print_handlers.on_before_emit_node.as_mut() {
            on_before_emit_node(Some(node));
        }

        state.comment_state = self.emit_comments_before_node_to_writer(node, writer);
        state.source_map_state = self.emit_source_maps_before_node_to_writer(node, writer);
        state
    }

    fn exit_node_to_writer(
        &mut self,
        node: &ast::Node,
        previous_state: PrinterState,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_source_maps_after_node_to_writer(node, previous_state.source_map_state, writer);
        self.emit_comments_after_node_to_writer(node, previous_state.comment_state, writer);

        if let Some(on_after_emit_node) = self.print_handlers.on_after_emit_node.as_mut() {
            on_after_emit_node(Some(node));
        }
    }

    fn emit_comments_before_node_to_writer(
        &mut self,
        node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) -> Option<CommentState> {
        if !self.should_emit_comments(node) {
            return None;
        }

        let emit_flags = self.emit_context.emit_flags(node);
        let comment_range = self.emit_context.comment_range(node);
        let container_pos = self.container_pos;
        let container_end = self.container_end;
        let declaration_list_container_end = self.declaration_list_container_end;

        self.emit_leading_comments_of_node_to_writer(node, emit_flags, comment_range, writer);
        self.emit_leading_synthetic_comments_of_node_to_writer(node, emit_flags, writer);
        if emit_flags & EF_NO_NESTED_COMMENTS != 0 {
            self.comments_disabled = true;
        }

        let state = CommentState {
            emit_flags,
            comment_range,
            container_pos,
            container_end,
            declaration_list_container_end,
        };
        *self.comment_state_arena.new_item() = state.clone();
        Some(state)
    }

    fn emit_comments_after_node_to_writer(
        &mut self,
        node: &ast::Node,
        state: Option<CommentState>,
        writer: &mut dyn EmitTextWriter,
    ) {
        let Some(state) = state else {
            return;
        };

        if state.emit_flags & EF_NO_NESTED_COMMENTS != 0 {
            self.comments_disabled = false;
        }

        self.emit_trailing_synthetic_comments_of_node_to_writer(node, state.emit_flags, writer);
        self.emit_trailing_comments_of_node_to_writer(
            node,
            state.emit_flags,
            state.comment_range,
            state.container_pos,
            state.container_end,
            state.declaration_list_container_end,
            writer,
        );

        if let Some(type_node) = self.emit_context.get_type_node(node) {
            self.emit_trailing_comments_of_node_to_writer(
                node,
                state.emit_flags,
                self.loc(&type_node),
                state.container_pos,
                state.container_end,
                state.declaration_list_container_end,
                writer,
            );
        }
    }

    fn emit_leading_comments_of_node_to_writer(
        &mut self,
        node: &ast::Node,
        emit_flags: EmitFlags,
        comment_range: core::TextRange,
        writer: &mut dyn EmitTextWriter,
    ) {
        let pos = comment_range.pos();
        let end = comment_range.end();

        if (!ast::position_is_synthesized(pos) || !ast::position_is_synthesized(end)) && pos != end
        {
            let skip_leading_comments = ast::position_is_synthesized(pos)
                || emit_flags & EF_NO_LEADING_COMMENTS != 0
                || self.kind(node) == ast::Kind::JsxText;
            let skip_trailing_comments = ast::position_is_synthesized(end)
                || emit_flags & EF_NO_TRAILING_COMMENTS != 0
                || self.kind(node) == ast::Kind::JsxText;

            if !skip_leading_comments {
                self.emit_leading_comments_to_writer(
                    pos,
                    self.kind(node) == ast::Kind::NotEmittedStatement,
                    writer,
                );
            }

            if !skip_leading_comments || (pos >= 0 && emit_flags & EF_NO_LEADING_COMMENTS != 0) {
                self.container_pos = pos;
            }

            if !skip_trailing_comments || (end >= 0 && emit_flags & EF_NO_TRAILING_COMMENTS != 0) {
                self.container_end = end;
                if self.kind(node) == ast::Kind::VariableDeclarationList {
                    self.declaration_list_container_end = end;
                }
            }
        }
    }

    fn emit_trailing_comments_of_node_to_writer(
        &mut self,
        node: &ast::Node,
        emit_flags: EmitFlags,
        comment_range: core::TextRange,
        container_pos: i32,
        container_end: i32,
        declaration_list_container_end: i32,
        writer: &mut dyn EmitTextWriter,
    ) {
        let pos = comment_range.pos();
        let end = comment_range.end();
        let skip_trailing_comments = end < 0
            || emit_flags & EF_NO_TRAILING_COMMENTS != 0
            || self.kind(node) == ast::Kind::JsxText;

        if (!ast::position_is_synthesized(pos) || !ast::position_is_synthesized(end)) && pos != end
        {
            self.container_pos = container_pos;
            self.container_end = container_end;
            self.declaration_list_container_end = declaration_list_container_end;

            if !skip_trailing_comments && self.kind(node) != ast::Kind::NotEmittedStatement {
                self.emit_trailing_comments_to_writer(end, CommentSeparator::Before, writer);
            }
        }
    }

    fn emit_leading_synthetic_comments_of_node_to_writer(
        &mut self,
        node: &ast::Node,
        emit_flags: EmitFlags,
        writer: &mut dyn EmitTextWriter,
    ) {
        if emit_flags & EF_NO_LEADING_COMMENTS != 0 {
            return;
        }
        for comment in self.emit_context.get_synthetic_leading_comments(node) {
            self.emit_leading_synthesized_comment_to_writer(comment, writer);
        }
    }

    fn emit_trailing_synthetic_comments_of_node_to_writer(
        &mut self,
        node: &ast::Node,
        emit_flags: EmitFlags,
        writer: &mut dyn EmitTextWriter,
    ) {
        if emit_flags & EF_NO_TRAILING_COMMENTS != 0 {
            return;
        }
        for comment in self.emit_context.get_synthetic_trailing_comments(node) {
            self.emit_trailing_synthesized_comment_to_writer(comment, writer);
        }
    }

    fn emit_leading_synthesized_comment_to_writer(
        &mut self,
        comment: crate::emitcontext::SynthesizedComment,
        writer: &mut dyn EmitTextWriter,
    ) {
        if comment.has_leading_new_line || comment.kind == ast::Kind::SingleLineCommentTrivia {
            writer.write_line();
        }
        self.write_synthesized_comment_to_writer(comment.clone(), writer);
        if comment.has_trailing_new_line || comment.kind == ast::Kind::SingleLineCommentTrivia {
            writer.write_line();
        } else {
            writer.write_space(" ");
        }
    }

    fn emit_trailing_synthesized_comment_to_writer(
        &mut self,
        comment: crate::emitcontext::SynthesizedComment,
        writer: &mut dyn EmitTextWriter,
    ) {
        if !writer.is_at_start_of_line() {
            writer.write_space(" ");
        }
        self.write_synthesized_comment_to_writer(comment.clone(), writer);
        if comment.has_trailing_new_line {
            writer.write_line();
        }
    }

    fn write_synthesized_comment_to_writer(
        &mut self,
        comment: crate::emitcontext::SynthesizedComment,
        writer: &mut dyn EmitTextWriter,
    ) {
        let text = if comment.kind == ast::Kind::MultiLineCommentTrivia {
            format!("/*{}*/", comment.text)
        } else {
            format!("//{}", comment.text)
        };
        let line_map = if comment.kind == ast::Kind::MultiLineCommentTrivia {
            core::compute_ecma_line_starts(&text)
        } else {
            Vec::new()
        };
        self.write_comment_range_worker_to_writer(
            &text,
            &line_map,
            comment.kind,
            core::new_text_range(0, text.len() as i32),
            writer,
        );
    }

    fn emit_trailing_comments_to_writer(
        &mut self,
        pos: i32,
        comment_separator: CommentSeparator,
        writer: &mut dyn EmitTextWriter,
    ) {
        if self.comments_disabled
            || self.current_source_file.is_none()
            || (self.container_end != -1
                && (pos == self.container_end || pos == self.declaration_list_container_end))
        {
            return;
        }

        let source_file = self.current_source_file.as_ref().unwrap();
        let comments = scanner::get_trailing_comment_ranges(source_file.text(), pos)
            .into_iter()
            .filter(|comment| self.should_write_comment(*comment))
            .collect();
        self.emit_comments_to_writer(comments, comment_separator, writer);
    }

    fn emit_trailing_comments_of_position_to_writer(
        &mut self,
        pos: i32,
        prefix_space: bool,
        force_no_newline: bool,
        writer: &mut dyn EmitTextWriter,
    ) {
        if self.comments_disabled
            || self.current_source_file.is_none()
            || (self.container_end != -1
                && (pos == self.container_end || pos == self.declaration_list_container_end))
        {
            return;
        }

        let source_file = self.current_source_file.as_ref().unwrap();
        let comments = scanner::get_trailing_comment_ranges(source_file.text(), pos);
        if comments.is_empty() {
            return;
        }

        for comment in comments {
            if prefix_space {
                if !self.should_write_comment(comment) {
                    continue;
                }
                if !writer.is_at_start_of_line() {
                    writer.write_space(" ");
                }
                self.emit_comment_to_writer(comment, writer);
                if comment.has_trailing_new_line {
                    writer.write_line();
                }
                continue;
            }

            self.emit_comment_to_writer(comment, writer);
            if force_no_newline {
                if comment.kind == ast::Kind::SingleLineCommentTrivia {
                    writer.write_line();
                }
            } else if comment.has_trailing_new_line {
                writer.write_line();
            } else {
                writer.write_space(" ");
            }
        }
    }

    fn emit_pos_to_writer(&mut self, pos: i32, writer: &dyn EmitTextWriter) {
        if self.source_maps_disabled
            || self.source_map_source.is_none()
            || self.source_map_generator.is_none()
            || self.source_map_source_is_json
            || ast::position_is_synthesized(pos)
        {
            return;
        }

        let Some(source_index) = self.source_map_source_index else {
            return;
        };
        let Some(line_char_cache) = self.source_map_line_char_cache.as_mut() else {
            return;
        };
        let (source_line, source_character) = line_char_cache.get_line_and_character(pos);
        self.source_map_generator
            .as_mut()
            .unwrap()
            .add_source_mapping(
                writer.get_line(),
                writer.get_column(),
                source_index,
                source_line as i32,
                source_character,
            )
            .unwrap_or_else(|err| panic!("{err}"));
    }

    fn emit_source_pos_to_writer(&mut self, pos: i32, writer: &dyn EmitTextWriter) {
        if self.source_map_line_char_cache.is_none()
            && let Some(source) = self.source_map_source.clone()
        {
            self.set_source_map_source(source);
        }
        self.emit_pos_to_writer(pos, writer);
    }

    fn emit_source_maps_before_node_to_writer(
        &mut self,
        node: &ast::Node,
        writer: &dyn EmitTextWriter,
    ) -> Option<SourceMapState> {
        if !self.should_emit_source_maps(node) {
            return None;
        }

        let emit_flags = self.emit_context.emit_flags(node);
        let loc = self.emit_context.source_map_range(node);

        if !ast::is_not_emitted_statement(self.store_for_node(node), *node)
            && emit_flags & crate::EF_NO_LEADING_SOURCE_MAP == 0
            && self.current_source_file.is_some()
            && !ast::position_is_synthesized(loc.pos())
        {
            let source_file = self.current_source_file.as_ref().unwrap();
            let pos = scanner::skip_trivia(source_file.text(), loc.pos().max(0) as usize) as i32;
            self.emit_source_pos_to_writer(pos, writer);
        }

        if emit_flags & crate::EF_NO_NESTED_SOURCE_MAPS != 0 {
            self.source_maps_disabled = true;
        }

        let state = SourceMapState {
            emit_flags,
            source_map_range: loc,
            has_token_source_map_range: false,
        };
        *self.source_map_state_arena.new_item() = state.clone();
        Some(state)
    }

    fn emit_source_maps_after_node_to_writer(
        &mut self,
        node: &ast::Node,
        previous_state: Option<SourceMapState>,
        writer: &dyn EmitTextWriter,
    ) {
        let Some(previous_state) = previous_state else {
            return;
        };

        if previous_state.emit_flags & crate::EF_NO_NESTED_SOURCE_MAPS != 0 {
            self.source_maps_disabled = false;
        }

        if !ast::is_not_emitted_statement(self.store_for_node(node), *node)
            && previous_state.emit_flags & crate::EF_NO_TRAILING_SOURCE_MAP == 0
            && !ast::position_is_synthesized(previous_state.source_map_range.end())
        {
            self.emit_source_pos_to_writer(previous_state.source_map_range.end(), writer);
        }
    }

    fn emit_source_maps_before_token_to_writer(
        &mut self,
        token: ast::Kind,
        mut pos: i32,
        context_node: &ast::Node,
        flags: TokenEmitFlags,
        writer: &dyn EmitTextWriter,
    ) -> Option<SourceMapState> {
        if !self.should_emit_token_source_maps(token, context_node, flags) {
            return None;
        }

        let emit_flags = self.emit_context.emit_flags(context_node);
        let loc = self
            .emit_context
            .token_source_map_range(context_node, token);
        if let Some(loc) = loc {
            pos = loc.pos();
        }
        if pos >= 0
            && let Some(source_file) = self.current_source_file.as_ref()
        {
            pos = scanner::skip_trivia(source_file.text(), pos as usize) as i32;
        }
        if emit_flags & crate::EF_NO_TOKEN_LEADING_SOURCE_MAPS == 0 && pos >= 0 {
            self.emit_source_pos_to_writer(pos, writer);
        }

        let state = SourceMapState {
            emit_flags,
            source_map_range: loc.unwrap_or_default(),
            has_token_source_map_range: loc.is_some(),
        };
        *self.source_map_state_arena.new_item() = state.clone();
        Some(state)
    }

    fn emit_source_maps_after_token_to_writer(
        &mut self,
        _token: ast::Kind,
        mut pos: i32,
        _context_node: &ast::Node,
        previous_state: Option<SourceMapState>,
        writer: &dyn EmitTextWriter,
    ) {
        let Some(previous_state) = previous_state else {
            return;
        };
        if previous_state.emit_flags & crate::EF_NO_TOKEN_TRAILING_SOURCE_MAPS == 0 {
            if previous_state.has_token_source_map_range {
                pos = previous_state.source_map_range.end();
            }
            if pos >= 0 {
                self.emit_source_pos_to_writer(pos, writer);
            }
        }
    }

    fn emit_comments_before_token_to_writer(
        &mut self,
        _token: ast::Kind,
        mut pos: i32,
        context_node: &ast::Node,
        flags: TokenEmitFlags,
        writer: &mut dyn EmitTextWriter,
    ) -> (Option<CommentState>, i32) {
        if flags.contains(TokenEmitFlags::NO_COMMENTS) || self.comments_disabled {
            if let Some(current_source_file) = self.current_source_file.as_ref()
                && !ast::position_is_synthesized(pos)
            {
                pos = scanner::skip_trivia(current_source_file.text(), pos.max(0) as usize) as i32;
            }
            return (None, pos);
        }

        let start_pos = pos;
        if let Some(current_source_file) = self.current_source_file.as_ref() {
            pos =
                scanner::skip_trivia(current_source_file.text(), start_pos.max(0) as usize) as i32;
        }

        let node = self.emit_context.parse_node(context_node);
        let is_similar_node = node
            .as_ref()
            .is_some_and(|node| self.kind(node) == self.kind(context_node));
        if !is_similar_node {
            return (None, pos);
        }

        if self.loc(context_node).pos() != start_pos {
            let indent_leading = flags.contains(TokenEmitFlags::INDENT_LEADING_COMMENTS);
            let needs_indent = indent_leading
                && self
                    .current_source_file
                    .as_ref()
                    .is_some_and(|source_file| {
                        !positions_are_on_same_line(start_pos, pos, source_file)
                    });
            if needs_indent {
                writer.increase_indent();
            }
            self.emit_leading_comments_to_writer(start_pos, false, writer);
            if needs_indent {
                writer.decrease_indent();
            }
        }

        let state = CommentState::default();
        *self.comment_state_arena.new_item() = state.clone();
        (Some(state), pos)
    }

    fn emit_comments_after_token_to_writer(
        &mut self,
        _token: ast::Kind,
        pos: i32,
        context_node: &ast::Node,
        state: Option<CommentState>,
        writer: &mut dyn EmitTextWriter,
    ) {
        if state.is_none() {
            return;
        }
        if self.loc(context_node).end() != pos {
            let separator = if self.kind(context_node) == ast::Kind::JsxExpression {
                CommentSeparator::None
            } else {
                CommentSeparator::Before
            };
            self.emit_trailing_comments_to_writer(pos, separator, writer);
        }
    }

    fn emit_statement(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        self.write_node_worker(Some(node), writer);
    }

    fn emit_expression_statement(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let state = self.enter_node_to_writer(original, writer);
        let current_source_file = share_source_file_option(&self.current_source_file);
        let expression = self.store_for_node(original).expression(*original);
        if let Some(expression) = self.store_for_node(original).expression(*original) {
            if current_source_file.as_ref().is_some_and(|source_file| {
                source_file.data().script_kind() == core::ScriptKind::JSON
            }) {
                self.emit_expression_with_precedence(
                    &expression,
                    ast::OPERATOR_PRECEDENCE_COMMA,
                    writer,
                );
            } else if is_immediately_invoked_function_expression_or_arrow_function(
                self.store_for_node(&expression),
                &expression,
            ) {
                self.emit_iife_with_parenthesized_callee(&expression, writer);
            } else {
                let leftmost_expression = ast::get_leftmost_expression(
                    self.store_for_node(&expression),
                    &expression,
                    false,
                );
                match self.kind(&leftmost_expression) {
                    ast::Kind::FunctionExpression | ast::Kind::ObjectLiteralExpression => {
                        self.emit_expression_with_precedence(
                            &expression,
                            ast::OPERATOR_PRECEDENCE_PARENTHESES,
                            writer,
                        );
                    }
                    _ => self.emit_expression_with_precedence(
                        &expression,
                        ast::OPERATOR_PRECEDENCE_COMMA,
                        writer,
                    ),
                }
            }
        }
        if current_source_file
            .as_ref()
            .is_none_or(|source_file| source_file.data().script_kind() != core::ScriptKind::JSON)
            || expression.is_some_and(|expression| {
                ast::node_is_synthesized(self.store_for_node(&expression), expression)
            })
        {
            writer.write_trailing_semicolon(";");
        }
        self.exit_node_to_writer(original, state, writer);
    }

    fn emit_iife_with_parenthesized_callee(
        &mut self,
        node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let call = ast::skip_partially_emitted_expressions(self.store_for_node(node), *node);
        writer.write_punctuation("(");
        if let Some(expression) = self.store_for_node(&call).expression(call) {
            self.emit_expression_with_precedence(
                &expression,
                ast::OPERATOR_PRECEDENCE_LOWEST,
                writer,
            );
        }
        writer.write_punctuation(")");
        self.write_node_worker(
            self.store_for_node(&call).question_dot_token(call).as_ref(),
            writer,
        );
        self.emit_type_arguments(
            &call,
            Self::optional_node_list(self.store_for_node(&call).source_type_arguments(call)),
            writer,
        );
        if let Some(arguments) = self.store_for_node(&call).source_arguments(call) {
            let argument_nodes: Vec<_> = arguments.iter().collect();
            self.emit_list_range(
                &call,
                &argument_nodes,
                arguments.loc(),
                false,
                ListFormat::CALL_EXPRESSION_ARGUMENTS,
                writer,
            );
        }
    }

    fn emit_trailing_source_comments(&mut self, pos: i32, writer: &mut dyn EmitTextWriter) {
        if self.comments_disabled || ast::position_is_synthesized(pos) {
            return;
        }
        let Some(source_file) = self.current_source_file.as_ref() else {
            return;
        };
        let text = source_file.text().to_string();
        let line_map = source_file.ecma_line_map();
        let comments = scanner::get_trailing_comment_ranges(&text, pos)
            .into_iter()
            .filter(|comment| self.should_write_comment(*comment))
            .collect::<Vec<_>>();
        if comments.is_empty() {
            return;
        }

        for comment in comments {
            writer.write_space(" ");
            self.write_comment_range_worker_to_writer(
                &text,
                &line_map,
                comment.kind,
                comment.text_range,
                writer,
            );
            if comment.kind == ast::Kind::SingleLineCommentTrivia || comment.has_trailing_new_line {
                break;
            }
        }
    }

    fn emit_continue_statement(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        self.emit_token_with_comment_to_writer(
            ast::Kind::ContinueKeyword,
            self.loc(node).pos(),
            WriteKind::Keyword,
            node,
            writer,
        );
        let label = self.store_for_node(node).label(*node);
        if let Some(label) = label {
            writer.write_space(" ");
            self.write_node_worker(Some(&label), writer);
        }
        writer.write_trailing_semicolon(";");
    }

    fn emit_break_statement(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        self.emit_token_with_comment_to_writer(
            ast::Kind::BreakKeyword,
            self.loc(node).pos(),
            WriteKind::Keyword,
            node,
            writer,
        );
        let label = self.store_for_node(node).label(*node);
        if let Some(label) = label {
            writer.write_space(" ");
            self.write_node_worker(Some(&label), writer);
        }
        writer.write_trailing_semicolon(";");
    }

    fn emit_debugger_statement(&mut self, writer: &mut dyn EmitTextWriter) {
        writer.write_keyword("debugger");
        writer.write_trailing_semicolon(";");
    }

    fn emit_labeled_statement(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let mut colon_pos = self.loc(node).pos();
        if let Some(label) = self.store_for_node(node).label(*node) {
            self.write_node_worker(Some(&label), writer);
            colon_pos = self.loc(&label).end();
        }
        self.emit_token(
            ast::Kind::ColonToken,
            colon_pos,
            WriteKind::Punctuation,
            node,
        );
        writer.write_space(" ");
        if let Some(statement) = self.store_for_node(node).statement(*node) {
            self.emit_statement(&statement, writer);
        }
    }

    fn emit_if_statement(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let state = self.enter_node_to_writer(node, writer);
        let pos = self.emit_token_with_comment_to_writer(
            ast::Kind::IfKeyword,
            self.loc(node).pos(),
            WriteKind::Keyword,
            node,
            writer,
        );
        writer.write_space(" ");
        self.emit_token_with_comment_to_writer(
            ast::Kind::OpenParenToken,
            pos,
            WriteKind::Punctuation,
            node,
            writer,
        );
        let expression = self.store_for_node(node).expression(*node);
        if let Some(expression) = expression.as_ref() {
            self.emit_expression(expression, writer);
        }
        self.emit_token_with_comment_to_writer(
            ast::Kind::CloseParenToken,
            expression
                .as_ref()
                .map(|expression| self.loc(expression).end())
                .unwrap_or(pos),
            WriteKind::Punctuation,
            node,
            writer,
        );
        let then_statement = self.store_for_node(node).then_statement(*node);
        self.emit_embedded_statement(node, then_statement.as_ref(), writer);
        if let Some(else_statement) = self.store_for_node(node).else_statement(*node) {
            if let Some(then_statement) = then_statement.as_ref() {
                self.write_line_or_space(node, then_statement, &else_statement, writer);
            } else {
                writer.write_space(" ");
            }
            self.emit_token_with_comment_to_writer(
                ast::Kind::ElseKeyword,
                then_statement
                    .as_ref()
                    .map(|then_statement| self.loc(then_statement).end())
                    .unwrap_or_else(|| self.loc(node).pos()),
                WriteKind::Keyword,
                node,
                writer,
            );
            if self.kind(&else_statement) == ast::Kind::IfStatement {
                writer.write_space(" ");
                self.emit_if_statement(&else_statement, writer);
            } else {
                self.emit_embedded_statement(node, Some(&else_statement), writer);
            }
        }
        self.exit_node_to_writer(node, state, writer);
    }

    fn emit_do_statement(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        self.emit_token_with_comment_to_writer(
            ast::Kind::DoKeyword,
            self.loc(node).pos(),
            WriteKind::Keyword,
            node,
            writer,
        );
        let statement = self.store_for_node(node).statement(*node);
        self.emit_embedded_statement(node, statement.as_ref(), writer);
        if statement
            .is_some_and(|statement| ast::is_block(self.store_for_node(&statement), statement))
            && !self.options.preserve_source_newlines
        {
            writer.write_space(" ");
        } else if let (Some(statement), Some(expression)) = (
            statement.as_ref(),
            self.store_for_node(node).expression(*node),
        ) {
            self.write_line_or_space(node, statement, &expression, writer);
        } else {
            writer.write_space(" ");
        }
        if let Some(expression) = self.store_for_node(node).expression(*node) {
            let start_pos = statement
                .as_ref()
                .map(|statement| self.loc(statement).end())
                .unwrap_or_else(|| self.loc(node).pos());
            self.emit_while_clause(node, &expression, start_pos, writer);
        }
        writer.write_trailing_semicolon(";");
    }

    fn emit_while_statement(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        if let Some(expression) = self.store_for_node(node).expression(*node) {
            self.emit_while_clause(node, &expression, self.loc(node).pos(), writer);
        }
        let statement = self.store_for_node(node).statement(*node);
        self.emit_embedded_statement(node, statement.as_ref(), writer);
    }

    fn emit_while_clause(
        &mut self,
        node: &ast::Node,
        expression: &ast::Node,
        start_pos: i32,
        writer: &mut dyn EmitTextWriter,
    ) {
        let pos = self.emit_token_with_comment_to_writer(
            ast::Kind::WhileKeyword,
            start_pos,
            WriteKind::Keyword,
            node,
            writer,
        );
        writer.write_space(" ");
        self.emit_token_with_comment_to_writer(
            ast::Kind::OpenParenToken,
            pos,
            WriteKind::Punctuation,
            node,
            writer,
        );
        self.emit_expression(expression, writer);
        self.emit_token_with_comment_to_writer(
            ast::Kind::CloseParenToken,
            self.loc(expression).end(),
            WriteKind::Punctuation,
            node,
            writer,
        );
    }

    fn emit_with_statement(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let pos = self.emit_token(
            ast::Kind::WithKeyword,
            self.loc(node).pos(),
            WriteKind::Keyword,
            node,
        );
        writer.write_space(" ");
        self.emit_token(ast::Kind::OpenParenToken, pos, WriteKind::Punctuation, node);
        let mut close_paren_pos = pos;
        if let Some(expression) = self.store_for_node(node).expression(*node) {
            self.emit_expression(&expression, writer);
            close_paren_pos = self.loc(&expression).end();
        }
        self.emit_token(
            ast::Kind::CloseParenToken,
            close_paren_pos,
            WriteKind::Punctuation,
            node,
        );
        let statement = self.store_for_node(node).statement(*node);
        self.emit_embedded_statement(node, statement.as_ref(), writer);
    }

    fn emit_for_in_statement(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let open_paren_pos = self.emit_token_with_comment_to_writer(
            ast::Kind::ForKeyword,
            self.loc(node).pos(),
            WriteKind::Keyword,
            node,
            writer,
        );
        writer.write_space(" ");
        self.emit_token_with_comment_to_writer(
            ast::Kind::OpenParenToken,
            open_paren_pos,
            WriteKind::Punctuation,
            node,
            writer,
        );
        let mut in_pos = open_paren_pos;
        if let Some(initializer) = self.store_for_node(node).initializer(*node) {
            self.emit_for_initializer(&initializer, writer);
            in_pos = self.loc(&initializer).end();
        }
        writer.write_space(" ");
        self.emit_token_with_comment_to_writer(
            ast::Kind::InKeyword,
            in_pos,
            WriteKind::Keyword,
            node,
            writer,
        );
        writer.write_space(" ");
        let mut close_paren_pos = in_pos;
        if let Some(expression) = self.store_for_node(node).expression(*node) {
            self.emit_expression(&expression, writer);
            close_paren_pos = self.loc(&expression).end();
        }
        self.emit_token_with_comment_to_writer(
            ast::Kind::CloseParenToken,
            close_paren_pos,
            WriteKind::Punctuation,
            node,
            writer,
        );
        let statement = self.store_for_node(node).statement(*node);
        self.emit_embedded_statement(node, statement.as_ref(), writer);
    }

    fn emit_for_of_statement(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let open_paren_pos = self.emit_token_with_comment_to_writer(
            ast::Kind::ForKeyword,
            self.loc(node).pos(),
            WriteKind::Keyword,
            node,
            writer,
        );
        writer.write_space(" ");
        if let Some(await_modifier) = self.store_for_node(node).await_modifier(*node) {
            self.emit_token_with_comment_to_writer(
                ast::Kind::AwaitKeyword,
                self.loc(&await_modifier).pos(),
                WriteKind::Keyword,
                &await_modifier,
                writer,
            );
            writer.write_space(" ");
        }
        self.emit_token_with_comment_to_writer(
            ast::Kind::OpenParenToken,
            open_paren_pos,
            WriteKind::Punctuation,
            node,
            writer,
        );
        let mut of_pos = open_paren_pos;
        if let Some(initializer) = self.store_for_node(node).initializer(*node) {
            self.emit_for_initializer(&initializer, writer);
            of_pos = self.loc(&initializer).end();
        }
        writer.write_space(" ");
        self.emit_token_with_comment_to_writer(
            ast::Kind::OfKeyword,
            of_pos,
            WriteKind::Keyword,
            node,
            writer,
        );
        writer.write_space(" ");
        let mut close_paren_pos = of_pos;
        if let Some(expression) = self.store_for_node(node).expression(*node) {
            self.emit_expression(&expression, writer);
            close_paren_pos = self.loc(&expression).end();
        }
        self.emit_token_with_comment_to_writer(
            ast::Kind::CloseParenToken,
            close_paren_pos,
            WriteKind::Punctuation,
            node,
            writer,
        );
        let statement = self.store_for_node(node).statement(*node);
        self.emit_embedded_statement(node, statement.as_ref(), writer);
    }

    fn emit_switch_statement(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let pos = self.emit_token(
            ast::Kind::SwitchKeyword,
            self.loc(node).pos(),
            WriteKind::Keyword,
            node,
        );
        writer.write_space(" ");
        self.emit_token(ast::Kind::OpenParenToken, pos, WriteKind::Punctuation, node);
        let mut close_paren_pos = pos;
        if let Some(expression) = self.store_for_node(node).expression(*node) {
            self.emit_expression(&expression, writer);
            close_paren_pos = self.loc(&expression).end();
        }
        self.emit_token(
            ast::Kind::CloseParenToken,
            close_paren_pos,
            WriteKind::Punctuation,
            node,
        );
        writer.write_space(" ");
        if let Some(case_block) = self.store_for_node(node).case_block(*node) {
            self.emit_case_block(&case_block, writer);
        }
    }

    fn emit_case_block(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let source_clauses = self.store_for_node(node).source_clauses(*node);
        let clauses = source_clauses.expect("case block should have clauses");
        let clauses_loc = clauses.loc();
        let clauses_end = clauses.end();
        let clause_nodes: Vec<_> = clauses.iter().collect();
        self.emit_token(
            ast::Kind::OpenBraceToken,
            self.loc(node).pos(),
            WriteKind::Punctuation,
            node,
        );
        self.emit_list_items(
            node,
            &clause_nodes,
            ListFormat::CASE_BLOCK_CLAUSES,
            false,
            clauses_loc,
            writer,
        );
        self.emit_token_ex(
            ast::Kind::CloseBraceToken,
            clauses_end,
            WriteKind::Punctuation,
            node,
            TokenEmitFlags::INDENT_LEADING_COMMENTS,
        );
    }

    fn emit_case_clause(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        self.emit_token(
            ast::Kind::CaseKeyword,
            self.loc(node).pos(),
            WriteKind::Keyword,
            node,
        );
        writer.write_space(" ");
        let mut colon_pos = self.loc(node).pos();
        if let Some(expression) = self.store_for_node(node).expression(*node) {
            self.emit_expression(&expression, writer);
            colon_pos = self.loc(&expression).end();
        }
        self.emit_case_or_default_clause_statements(node, colon_pos, writer);
    }

    fn emit_default_clause(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let pos = self.emit_token(
            ast::Kind::DefaultKeyword,
            self.loc(node).pos(),
            WriteKind::Keyword,
            node,
        );
        self.emit_case_or_default_clause_statements(node, pos, writer);
    }

    fn emit_case_or_default_clause_statements(
        &mut self,
        node: &ast::Node,
        colon_pos: i32,
        writer: &mut dyn EmitTextWriter,
    ) {
        let source_statements = self.store_for_node(node).source_statements(*node);
        let Some(source_statements) = source_statements else {
            let colon_end = self.emit_token(
                ast::Kind::ColonToken,
                colon_pos,
                WriteKind::Punctuation,
                node,
            );
            if !writer.has_trailing_comment() {
                self.emit_trailing_comments_to_writer(colon_end, CommentSeparator::Before, writer);
            }
            return;
        };
        let statements = EmitNodeList::from_source(source_statements);
        if statements.nodes.is_empty() {
            let colon_end = self.emit_token(
                ast::Kind::ColonToken,
                colon_pos,
                WriteKind::Punctuation,
                node,
            );
            if !writer.has_trailing_comment() {
                self.emit_trailing_comments_to_writer(colon_end, CommentSeparator::Before, writer);
            }
            return;
        }
        let emit_as_single_statement = statements.nodes.len() == 1
            && (self.current_source_file.is_none()
                || ast::node_is_synthesized(self.store_for_node(node), *node)
                || ast::node_is_synthesized(
                    self.store_for_node(&statements.nodes[0]),
                    statements.nodes[0],
                )
                || self
                    .current_source_file
                    .as_ref()
                    .is_some_and(|source_file| {
                        range_start_positions_are_on_same_line(
                            self.loc(node),
                            self.loc(&statements.nodes[0]),
                            source_file,
                        )
                    }));

        let mut format = ListFormat::CASE_OR_DEFAULT_CLAUSE_STATEMENTS;
        if emit_as_single_statement {
            self.write_token_text_to_writer(
                ast::Kind::ColonToken,
                WriteKind::Punctuation,
                colon_pos,
                writer,
            );
            writer.write_space(" ");
            format = format & !(ListFormat::MULTI_LINE | ListFormat::INDENTED);
        } else {
            self.emit_token(
                ast::Kind::ColonToken,
                colon_pos,
                WriteKind::Punctuation,
                node,
            );
        }

        self.emit_list_items(
            node,
            &statements.nodes,
            format,
            false,
            statements.loc(),
            writer,
        );
    }

    fn emit_throw_statement(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        self.emit_token(
            ast::Kind::ThrowKeyword,
            self.loc(node).pos(),
            WriteKind::Keyword,
            node,
        );
        if let Some(expression) = self.store_for_node(node).expression(*node) {
            writer.write_space(" ");
            self.emit_expression_no_asi_with_precedence(
                &expression,
                ast::OPERATOR_PRECEDENCE_LOWEST,
                writer,
            );
        }
        writer.write_trailing_semicolon(";");
    }

    fn emit_try_statement(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let state = self.enter_node_to_writer(node, writer);
        self.emit_token(
            ast::Kind::TryKeyword,
            self.loc(node).pos(),
            WriteKind::Keyword,
            node,
        );
        writer.write_space(" ");
        let try_block = self.store_for_node(node).try_block(*node);
        if let Some(try_block) = try_block {
            self.emit_block_node(&try_block, writer);
        }
        if let Some(catch_clause) = self.store_for_node(node).catch_clause(*node) {
            if let Some(try_block) = try_block.as_ref() {
                self.write_line_or_space(node, try_block, &catch_clause, writer);
            } else {
                writer.write_space(" ");
            }
            self.emit_catch_clause(&catch_clause, writer);
        }
        if let Some(finally_block) = self.store_for_node(node).finally_block(*node) {
            let previous = self.store_for_node(node).catch_clause(*node).or(try_block);
            if let Some(previous) = previous.as_ref() {
                self.write_line_or_space(node, previous, &finally_block, writer);
            } else {
                writer.write_space(" ");
            }
            let finally_pos = previous
                .as_ref()
                .map_or_else(|| self.loc(node).pos(), |previous| self.loc(previous).end());
            self.emit_token(
                ast::Kind::FinallyKeyword,
                finally_pos,
                WriteKind::Keyword,
                node,
            );
            writer.write_space(" ");
            self.emit_block_node(&finally_block, writer);
        }
        self.exit_node_to_writer(node, state, writer);
    }

    fn emit_catch_clause(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let state = self.enter_node_to_writer(node, writer);
        let open_paren_pos = self.emit_token(
            ast::Kind::CatchKeyword,
            self.loc(node).pos(),
            WriteKind::Keyword,
            node,
        );
        if let Some(variable_declaration) = self.store_for_node(node).variable_declaration(*node) {
            writer.write_space(" ");
            self.emit_token(
                ast::Kind::OpenParenToken,
                open_paren_pos,
                WriteKind::Punctuation,
                node,
            );
            self.emit_variable_declaration(&variable_declaration, &variable_declaration, writer);
            self.emit_token(
                ast::Kind::CloseParenToken,
                self.loc(&variable_declaration).end(),
                WriteKind::Punctuation,
                node,
            );
        }
        writer.write_space(" ");
        if let Some(block) = self.store_for_node(node).block(*node) {
            self.emit_block_node(&block, writer);
        }
        self.exit_node_to_writer(node, state, writer);
    }

    fn emit_for_statement(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let mut pos = self.emit_token_with_comment_to_writer(
            ast::Kind::ForKeyword,
            self.loc(original).pos(),
            WriteKind::Keyword,
            original,
            writer,
        );
        writer.write_space(" ");
        pos = self.emit_token_with_comment_to_writer(
            ast::Kind::OpenParenToken,
            pos,
            WriteKind::Punctuation,
            original,
            writer,
        );
        if let Some(initializer) = self.store_for_node(original).initializer(*original) {
            self.emit_for_initializer(&initializer, writer);
            pos = self.loc(&initializer).end();
        }
        pos = self.emit_token_with_comment_to_writer(
            ast::Kind::SemicolonToken,
            pos,
            WriteKind::Punctuation,
            original,
            writer,
        );
        if let Some(condition) = self.store_for_node(original).condition(*original) {
            writer.write_space(" ");
            self.emit_expression(&condition, writer);
            pos = self.loc(&condition).end();
        }
        pos = self.emit_token_with_comment_to_writer(
            ast::Kind::SemicolonToken,
            pos,
            WriteKind::Punctuation,
            original,
            writer,
        );
        if let Some(incrementor) = self.store_for_node(original).incrementor(*original) {
            writer.write_space(" ");
            self.emit_expression(&incrementor, writer);
            pos = self.loc(&incrementor).end();
        }
        self.emit_token_with_comment_to_writer(
            ast::Kind::CloseParenToken,
            pos,
            WriteKind::Punctuation,
            original,
            writer,
        );
        let statement = self.store_for_node(original).statement(*original);
        self.emit_embedded_statement(original, statement.as_ref(), writer);
    }

    fn emit_for_initializer(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        if self.kind(node) == ast::Kind::VariableDeclarationList {
            self.emit_variable_declaration_list_node(node, writer);
        } else {
            self.emit_expression(node, writer);
        }
    }

    fn emit_embedded_statement(
        &mut self,
        parent_node: &ast::Node,
        statement: Option<&ast::Node>,
        writer: &mut dyn EmitTextWriter,
    ) {
        let Some(statement) = statement else {
            writer.write_punctuation(";");
            return;
        };
        if self.kind(statement) == ast::Kind::Block
            || self.should_emit_on_single_line(parent_node)
            || (self.options.preserve_source_newlines
                && self.get_leading_line_terminator_count(
                    parent_node,
                    Some(statement),
                    ListFormat::NONE,
                ) == 0)
        {
            writer.write_space(" ");
            self.emit_statement(statement, writer);
        } else {
            writer.increase_indent();
            writer.write_line();
            if self.kind(statement) == ast::Kind::EmptyStatement {
                self.emit_empty_statement(writer, true);
            } else {
                self.emit_statement(statement, writer);
            }
            writer.decrease_indent();
        }
    }

    fn emit_empty_statement(
        &mut self,
        writer: &mut dyn EmitTextWriter,
        is_embedded_statement: bool,
    ) {
        if is_embedded_statement {
            writer.write_punctuation(";");
        } else {
            writer.write_trailing_semicolon(";");
        }
    }

    fn emit_return_statement(
        &mut self,
        _original: &ast::Node,
        node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        writer.write_keyword("return");
        if let Some(expression) = self.store_for_node(node).expression(*node) {
            writer.write_space(" ");
            self.emit_expression_no_asi_with_precedence(
                &expression,
                ast::OPERATOR_PRECEDENCE_LOWEST,
                writer,
            );
        }
        writer.write_trailing_semicolon(";");
    }

    fn emit_block(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.generate_names(*original);
        self.emit_token(
            ast::Kind::OpenBraceToken,
            self.loc(original).pos(),
            WriteKind::Punctuation,
            original,
        );
        let (is_multi_line, statements_loc, statement_nodes) = {
            let store = self.store_for_node(original);
            let statements = store
                .source_statements(*original)
                .expect("block should have statements");
            (
                store.multi_line(*original).unwrap_or(false),
                statements.loc(),
                statements.iter().collect::<Vec<_>>(),
            )
        };
        let format = if (!is_multi_line && self.is_empty_block(original, &statement_nodes))
            || self.should_emit_on_single_line(original)
        {
            ListFormat::SINGLE_LINE_BLOCK_STATEMENTS
        } else {
            ListFormat::MULTI_LINE_BLOCK_STATEMENTS
        };
        self.emit_list_range(
            original,
            &statement_nodes,
            statements_loc,
            false,
            format,
            writer,
        );
        self.emit_token_ex_to_writer(
            ast::Kind::CloseBraceToken,
            statements_loc.end(),
            WriteKind::Punctuation,
            original,
            if format.contains(ListFormat::MULTI_LINE) {
                TokenEmitFlags::INDENT_LEADING_COMMENTS
            } else {
                TokenEmitFlags::NONE
            },
            writer,
        );
    }

    fn emit_function_body(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_context.mark_emit_node(original, EF_NO_SOURCE_MAP);
        if let Some(on_before_emit_node) = self.print_handlers.on_before_emit_node.as_mut() {
            on_before_emit_node(Some(original));
        }

        self.generate_names(*original);
        writer.write_punctuation("{");
        let (statements_loc, statement_nodes) = {
            let statements = self
                .store_for_node(original)
                .source_statements(*original)
                .expect("block should have statements");
            (statements.loc(), statements.iter().collect::<Vec<_>>())
        };

        writer.increase_indent();
        let detached_state =
            self.emit_detached_comments_before_statement_list(original, statements_loc, writer);
        let statement_offset = self.emit_prologue_directives(&statement_nodes, writer);
        let position_after_prologue = writer.get_text_pos();
        self.emit_helpers(original, writer);

        if self.should_emit_block_function_body_on_single_line(original)
            && statement_offset == 0
            && position_after_prologue == writer.get_text_pos()
        {
            writer.decrease_indent();
            self.emit_list_items(
                original,
                &statement_nodes,
                ListFormat::SINGLE_LINE_FUNCTION_BODY_STATEMENTS,
                false,
                statements_loc,
                writer,
            );
            writer.increase_indent();
        } else {
            self.emit_list_items(
                original,
                &statement_nodes[statement_offset.min(statement_nodes.len())..],
                ListFormat::MULTI_LINE_FUNCTION_BODY_STATEMENTS,
                false,
                statements_loc,
                writer,
            );
        }
        self.emit_detached_comments_after_statement_list(
            original,
            statements_loc,
            detached_state,
            writer,
        );
        writer.decrease_indent();
        self.emit_token_ex(
            ast::Kind::CloseBraceToken,
            statements_loc.end(),
            WriteKind::Punctuation,
            original,
            TokenEmitFlags::NO_COMMENTS,
        );

        if let Some(on_after_emit_node) = self.print_handlers.on_after_emit_node.as_mut() {
            on_after_emit_node(Some(original));
        }
    }

    fn emit_module_declaration(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_modifier_list(original, self.modifiers(original), false, writer);
        let keyword = self
            .store_for_node(original)
            .keyword(*original)
            .expect("module declaration should have keyword");
        if keyword != ast::Kind::GlobalKeyword {
            writer.write_keyword(if keyword == ast::Kind::NamespaceKeyword {
                "namespace"
            } else {
                "module"
            });
            writer.write_space(" ");
        }
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.emit_module_name(&name, writer);
        }
        let mut body = self.store_for_node(original).body(*original);
        while let Some(current_body) = body
            .as_ref()
            .filter(|body| self.kind(body) == ast::Kind::ModuleDeclaration)
        {
            let next_body = {
                writer.write_punctuation(".");
                if let Some(name) = self.store_for_node(current_body).name(*current_body) {
                    self.emit_nested_module_name(&name, writer);
                }
                self.store_for_node(current_body).body(*current_body)
            };
            body = next_body;
        }
        if let Some(body) = body.as_ref() {
            writer.write_space(" ");
            self.emit_module_block(body, body, writer);
        } else {
            writer.write_trailing_semicolon(";");
        }
    }

    fn emit_module_name(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        match self.kind(node) {
            ast::Kind::Identifier => self.emit_binding_identifier(node, writer),
            ast::Kind::StringLiteral => {
                let current_source_file = share_source_file_option(&self.current_source_file);
                writer.write_string_literal(&self.get_literal_text_of_node(
                    node,
                    current_source_file.as_ref(),
                    GetLiteralTextFlags::NONE,
                ));
            }
            _ => self.write_node_worker(Some(node), writer),
        }
    }

    fn emit_nested_module_name(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        match self.kind(node) {
            ast::Kind::Identifier => self.emit_identifier_name(node, writer),
            _ => self.emit_module_name(node, writer),
        }
    }

    fn emit_module_block(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let state = self.enter_node_to_writer(original, writer);
        self.generate_names(*original);
        self.emit_token(
            ast::Kind::OpenBraceToken,
            self.loc(original).pos(),
            WriteKind::Punctuation,
            original,
        );
        let (statements_loc, statement_nodes) = {
            let statements = self
                .store_for_node(original)
                .source_statements(*original)
                .expect("module block should have statements");
            (statements.loc(), statements.iter().collect::<Vec<_>>())
        };
        let format = if self.is_empty_block(original, &statement_nodes)
            || !self.should_emit_on_multiple_lines(original)
                && self.should_emit_on_single_line(original)
        {
            ListFormat::SINGLE_LINE_BLOCK_STATEMENTS
        } else {
            ListFormat::MULTI_LINE_BLOCK_STATEMENTS
        };
        self.emit_list_items(
            original,
            &statement_nodes,
            format,
            false,
            statements_loc,
            writer,
        );
        self.emit_token_ex(
            ast::Kind::CloseBraceToken,
            statements_loc.end(),
            WriteKind::Punctuation,
            original,
            if format.intersects(ListFormat::MULTI_LINE) {
                TokenEmitFlags::INDENT_LEADING_COMMENTS
            } else {
                TokenEmitFlags::NONE
            },
        );
        self.exit_node_to_writer(original, state, writer);
    }

    fn emit_export_assignment(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let pos = self.emit_token_with_comment_to_writer(
            ast::Kind::ExportKeyword,
            self.loc(original).pos(),
            WriteKind::Keyword,
            original,
            writer,
        );
        writer.write_space(" ");
        if self
            .store_for_node(original)
            .is_export_equals(*original)
            .unwrap_or(false)
        {
            self.emit_token_with_comment_to_writer(
                ast::Kind::EqualsToken,
                pos,
                WriteKind::Operator,
                original,
                writer,
            );
        } else {
            self.emit_token_with_comment_to_writer(
                ast::Kind::DefaultKeyword,
                pos,
                WriteKind::Keyword,
                original,
                writer,
            );
        }
        writer.write_space(" ");
        if let Some(expression) = self.store_for_node(original).expression(*original) {
            if self
                .store_for_node(original)
                .is_export_equals(*original)
                .unwrap_or(false)
            {
                self.emit_expression_with_precedence(
                    &expression,
                    ast::OPERATOR_PRECEDENCE_ASSIGNMENT,
                    writer,
                );
            } else {
                // parenthesize `class` and `function` expressions so as not to conflict with exported `class` and `function` declarations
                let leftmost_expression = ast::get_leftmost_expression(
                    self.store_for_node(&expression),
                    &expression,
                    false,
                );
                if matches!(
                    self.kind(&leftmost_expression),
                    ast::Kind::ClassExpression | ast::Kind::FunctionExpression
                ) {
                    self.emit_expression_with_precedence(
                        &expression,
                        ast::OPERATOR_PRECEDENCE_PARENTHESES,
                        writer,
                    );
                } else {
                    self.emit_expression_with_precedence(
                        &expression,
                        ast::OPERATOR_PRECEDENCE_ASSIGNMENT,
                        writer,
                    );
                }
            }
        }
        writer.write_trailing_semicolon(";");
    }

    fn emit_export_declaration(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let state = self.enter_node_to_writer(original, writer);
        let modifiers = self.modifiers(original);
        self.emit_modifier_list(original, modifiers.clone(), false, writer);
        let mut pos = self.emit_token_with_comment_to_writer(
            ast::Kind::ExportKeyword,
            greatest_end_nodes(self.loc(original).pos(), &modifiers, self),
            WriteKind::Keyword,
            original,
            writer,
        );
        writer.write_space(" ");
        if self
            .store_for_node(original)
            .is_type_only(*original)
            .unwrap_or(false)
        {
            pos = self.emit_token_with_comment_to_writer(
                ast::Kind::TypeKeyword,
                pos,
                WriteKind::Keyword,
                original,
                writer,
            );
            writer.write_space(" ");
        }
        let mut from_pos = pos;
        if let Some(export_clause) = self.store_for_node(original).export_clause(*original) {
            self.write_node_worker(Some(&export_clause), writer);
            from_pos = from_pos.max(self.loc(&export_clause).end());
        } else {
            from_pos = self.emit_token_with_comment_to_writer(
                ast::Kind::AsteriskToken,
                pos,
                WriteKind::Punctuation,
                original,
                writer,
            );
        }
        if let Some(module_specifier) = self.store_for_node(original).module_specifier(*original) {
            writer.write_space(" ");
            self.emit_token_with_comment_to_writer(
                ast::Kind::FromKeyword,
                from_pos,
                WriteKind::Keyword,
                original,
                writer,
            );
            writer.write_space(" ");
            self.emit_expression(&module_specifier, writer);
        }
        if let Some(attributes) = self.store_for_node(original).attributes(*original) {
            writer.write_space(" ");
            self.write_node_worker(Some(&attributes), writer);
        }
        writer.write_trailing_semicolon(";");
        self.exit_node_to_writer(original, state, writer);
    }

    fn emit_named_exports(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let state = self.enter_node_to_writer(original, writer);
        writer.write_punctuation("{");
        let source_elements = self
            .store_for_node(original)
            .source_elements(*original)
            .expect("named exports should have elements");
        let has_trailing_comma = self.has_trailing_comma(
            original,
            source_elements.has_trailing_comma(),
            source_elements.position_key(),
        );
        let elements = self.node_list(source_elements);
        self.emit_list_items(
            original,
            &elements,
            ListFormat::NAMED_IMPORTS_OR_EXPORTS_ELEMENTS,
            has_trailing_comma,
            source_elements.loc(),
            writer,
        );
        writer.write_punctuation("}");
        self.exit_node_to_writer(original, state, writer);
    }

    fn emit_export_specifier(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        if self
            .store_for_node(original)
            .is_type_only(*original)
            .unwrap_or(false)
        {
            self.emit_token_with_comment_to_writer(
                ast::Kind::TypeKeyword,
                self.loc(original).pos(),
                WriteKind::Keyword,
                original,
                writer,
            );
            writer.write_space(" ");
        }
        if let Some(property_name) = self.store_for_node(original).property_name(*original) {
            self.write_node_worker(Some(&property_name), writer);
            writer.write_space(" ");
            self.emit_token_with_comment_to_writer(
                ast::Kind::AsKeyword,
                self.token_end_at_node_pos(&property_name),
                WriteKind::Keyword,
                original,
                writer,
            );
            writer.write_space(" ");
        }
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.write_node_worker(Some(&name), writer);
        }
    }

    fn emit_namespace_export(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let pos = self.emit_token_with_comment_to_writer(
            ast::Kind::AsteriskToken,
            self.loc(original).pos(),
            WriteKind::Punctuation,
            original,
            writer,
        );
        writer.write_space(" ");
        self.emit_token_with_comment_to_writer(
            ast::Kind::AsKeyword,
            pos,
            WriteKind::Keyword,
            original,
            writer,
        );
        writer.write_space(" ");
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.write_node_worker(Some(&name), writer);
        }
    }

    fn emit_namespace_export_declaration(
        &mut self,
        original: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_modifier_list(original, self.modifiers(original), false, writer);
        writer.write_keyword("export");
        writer.write_space(" ");
        writer.write_keyword("as");
        writer.write_space(" ");
        writer.write_keyword("namespace");
        writer.write_space(" ");
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.emit_binding_identifier(&name, writer);
        }
        writer.write_trailing_semicolon(";");
    }

    fn emit_external_module_reference(
        &mut self,
        original: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        writer.write_keyword("require");
        writer.write_punctuation("(");
        if let Some(expression) = self.store_for_node(original).expression(*original) {
            self.emit_expression(&expression, writer);
        }
        writer.write_punctuation(")");
    }

    fn emit_import_equals_declaration(
        &mut self,
        original: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_modifier_list(original, self.modifiers(original), false, writer);
        writer.write_keyword("import");
        writer.write_space(" ");
        if self
            .store_for_node(original)
            .is_type_only(*original)
            .unwrap_or(false)
        {
            writer.write_keyword("type");
            writer.write_space(" ");
        }
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.emit_binding_identifier(&name, writer);
        }
        writer.write_space(" ");
        writer.write_punctuation("=");
        writer.write_space(" ");
        if let Some(module_reference) = self.store_for_node(original).module_reference(*original) {
            self.emit_module_reference(&module_reference, writer);
        }
        writer.write_trailing_semicolon(";");
    }

    fn emit_import_attributes(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let token = self
            .store_for_node(original)
            .token(*original)
            .unwrap_or(ast::Kind::WithKeyword);
        self.emit_token_text_to_writer(token, WriteKind::Keyword, writer);
        writer.write_space(" ");
        let attributes =
            Self::optional_node_list(self.store_for_node(original).source_attributes(*original));
        self.emit_list_range(
            original,
            &attributes,
            core::new_text_range(-1, -1),
            attributes.is_empty(),
            ListFormat::IMPORT_ATTRIBUTES,
            writer,
        );
    }

    fn emit_import_attribute(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.write_node_worker(Some(&name), writer);
        }
        writer.write_punctuation(":");
        writer.write_space(" ");
        if let Some(value) = self.store_for_node(original).value(*original) {
            self.emit_expression_with_precedence(
                &value,
                ast::OPERATOR_PRECEDENCE_DISALLOW_COMMA,
                writer,
            );
        }
    }

    fn emit_variable_statement(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let state = self.enter_node_to_writer(original, writer);
        self.emit_modifier_list(original, self.modifiers(original), false, writer);
        if let Some(declaration_list) = self.store_for_node(original).declaration_list(*original) {
            self.emit_variable_declaration_list_node(&declaration_list, writer);
        }
        writer.write_trailing_semicolon(";");
        self.exit_node_to_writer(original, state, writer);
    }

    fn emit_modifier_list(
        &mut self,
        original: &ast::Node,
        modifiers: Vec<ast::Node>,
        allow_decorators: bool,
        writer: &mut dyn EmitTextWriter,
    ) -> i32 {
        if modifiers.is_empty() {
            return self.loc(original).pos();
        }
        let modifiers_text_range = self
            .store_for_node(original)
            .source_modifiers(*original)
            .map(|modifiers| modifiers.loc())
            .unwrap_or_else(|| core::new_text_range(-1, -1));

        let emit_modifier_like =
            |printer: &mut Self, modifier: &ast::Node, writer: &mut dyn EmitTextWriter| {
                printer.write_node_worker(Some(modifier), writer);
            };

        let all_modifiers = modifiers
            .iter()
            .all(|modifier| ast::is_modifier(self.store_for_node(modifier), *modifier));
        if all_modifiers {
            self.emit_list_items(
                original,
                &modifiers,
                ListFormat::MODIFIERS,
                false,
                modifiers_text_range,
                writer,
            );
            return greatest_end_nodes(self.loc(original).pos(), &modifiers, self);
        }

        let all_decorators = modifiers
            .iter()
            .all(|modifier| ast::is_decorator(self.store_for_node(modifier), *modifier));
        if all_decorators {
            if !allow_decorators {
                return self.loc(original).pos();
            }
            self.emit_list_items(
                original,
                &modifiers,
                ListFormat::DECORATORS,
                false,
                modifiers_text_range,
                writer,
            );
            return greatest_end_nodes(self.loc(original).pos(), &modifiers, self);
        }

        let mut start = 0;
        while start < modifiers.len() {
            let chunk_is_decorator =
                ast::is_decorator(self.store_for_node(&modifiers[start]), modifiers[start]);
            let mut end = start + 1;
            while end < modifiers.len()
                && ast::is_decorator(self.store_for_node(&modifiers[end]), modifiers[end])
                    == chunk_is_decorator
            {
                end += 1;
            }

            let chunk = &modifiers[start..end];
            let format = if chunk_is_decorator {
                ListFormat::DECORATORS
            } else {
                ListFormat::MODIFIERS
            };

            if allow_decorators || !chunk_is_decorator {
                let mut text_range = core::new_text_range(-1, -1);
                if start == 0 {
                    text_range = core::new_text_range(modifiers_text_range.pos(), text_range.end());
                }
                if end == modifiers.len() {
                    text_range = core::new_text_range(text_range.pos(), modifiers_text_range.end());
                }
                self.emit_list_items_with(
                    original,
                    chunk,
                    format,
                    false,
                    text_range,
                    writer,
                    |printer, modifier, writer| emit_modifier_like(printer, modifier, writer),
                );
            }

            start = end;
        }
        greatest_end_nodes(self.loc(original).pos(), &modifiers, self)
    }

    fn emit_variable_declaration_list(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let state = self.enter_node_to_writer(original, writer);
        let store = self.store_for_node(original);
        if ast::is_let(store, original) {
            writer.write_keyword("let");
        } else if ast::is_var_const(store, original) {
            writer.write_keyword("const");
        } else if ast::is_var_await_using(store, *original) {
            writer.write_keyword("await");
            writer.write_space(" ");
            writer.write_keyword("using");
        } else if ast::is_var_using(store, *original) {
            writer.write_keyword("using");
        } else {
            writer.write_keyword("var");
        }
        writer.write_space(" ");
        let declarations = self.node_list(
            self.store_for_node(original)
                .source_declarations(*original)
                .expect("variable declaration list should have declarations"),
        );
        self.emit_list_items(
            original,
            &declarations,
            ListFormat::VARIABLE_DECLARATION_LIST,
            false,
            core::new_text_range(-1, -1),
            writer,
        );
        self.exit_node_to_writer(original, state, writer);
    }

    fn emit_variable_declaration(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let state = self.enter_node_to_writer(original, writer);
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.emit_binding_name(&name, writer);
        }
        self.write_node_worker(
            self.store_for_node(original)
                .postfix_token(*original)
                .as_ref(),
            writer,
        );
        if let Some(type_node) = self.store_for_node(original).r#type(*original) {
            writer.write_punctuation(":");
            writer.write_space(" ");
            self.write_node_worker(Some(&type_node), writer);
        }
        if let Some(initializer) = self.store_for_node(original).initializer(*original) {
            let equal_token_pos = self.store_for_node(original).name(*original).map_or(
                self.loc(original).pos(),
                |name| {
                    let mut ranges = Vec::new();
                    if let Some(type_node) = self.store_for_node(original).r#type(*original) {
                        ranges.push(self.loc(&type_node));
                    }
                    let saved_type_node = self.emit_context.get_type_node(&name);
                    if let Some(type_node) = saved_type_node {
                        ranges.push(self.loc(&type_node));
                    }
                    greatest_end(self.loc(&name).end(), &ranges)
                },
            );
            self.emit_initializer(&initializer, equal_token_pos, original, writer);
        }
        self.exit_node_to_writer(original, state, writer);
    }

    fn emit_enum_declaration(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        self.emit_modifier_list(node, self.modifiers(node), false, writer);
        writer.write_keyword("enum");
        writer.write_space(" ");
        if let Some(name) = self.store_for_node(node).name(*node) {
            self.emit_binding_identifier(&name, writer);
        }
        writer.write_space(" ");
        writer.write_punctuation("{");
        let members_view = self.store_for_node(node).members(*node);
        let (members, members_loc, has_trailing_comma) =
            if let Some(members_view) = members_view.as_ref() {
                (
                    members_view.iter().collect::<Vec<_>>(),
                    members_view.loc(),
                    members_view.has_trailing_comma(),
                )
            } else {
                (Vec::new(), core::new_text_range(-1, -1), false)
            };
        self.emit_list_range_with_trailing_comma(
            node,
            &members,
            members_loc,
            false,
            ListFormat::ENUM_MEMBERS,
            has_trailing_comma,
            writer,
        );
        writer.write_punctuation("}");
    }

    fn emit_enum_member(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        if let Some(name) = self.store_for_node(node).name(*node) {
            self.write_node_worker(Some(&name), writer);
        }
        if let Some(initializer) = self.store_for_node(node).initializer(*node) {
            writer.write_space(" ");
            writer.write_operator("=");
            writer.write_space(" ");
            self.emit_expression(&initializer, writer);
        }
    }

    fn emit_jsx_element(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        if let Some(opening) = self
            .store_for_node(original)
            .jsx_opening_element(*original)
            .into()
        {
            self.write_node_worker(Some(&opening), writer);
        }
        let children = self.store_for_node(original).jsx_children(*original);
        let child_nodes: Vec<_> = children.iter().collect();
        self.emit_list_items(
            original,
            &child_nodes,
            ListFormat::JSX_ELEMENT_OR_FRAGMENT_CHILDREN,
            false,
            children.loc(),
            writer,
        );
        if let Some(closing) = self
            .store_for_node(original)
            .jsx_closing_element(*original)
            .into()
        {
            self.write_node_worker(Some(&closing), writer);
        }
    }

    fn emit_jsx_self_closing_element(
        &mut self,
        original: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        writer.write_punctuation("<");
        if let Some(tag_name) = self.store_for_node(original).tag_name(*original) {
            self.emit_jsx_tag_name(&tag_name, writer);
        }
        self.emit_type_arguments(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_type_arguments(*original),
            ),
            writer,
        );
        writer.write_space(" ");
        let attributes = self.store_for_node(original).attributes(*original);
        if let Some(attributes) = attributes {
            self.write_node_worker(Some(&attributes), writer);
        }
        writer.write_punctuation("/>");
    }

    fn emit_jsx_opening_element(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        writer.write_punctuation("<");
        if let Some(tag_name) = self.store_for_node(original).tag_name(*original) {
            let indented = self.write_line_separators_and_indent_before(&tag_name, original);
            self.emit_jsx_tag_name(&tag_name, writer);
            self.decrease_indent_if(indented);
        }
        self.emit_type_arguments(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_type_arguments(*original),
            ),
            writer,
        );
        let attributes = self.store_for_node(original).attributes(*original);
        if attributes.is_some_and(|attributes| {
            !Self::optional_node_list(
                self.store_for_node(&attributes)
                    .source_properties(attributes),
            )
            .is_empty()
        }) {
            writer.write_space(" ");
        }
        if let Some(attributes) = attributes {
            self.write_node_worker(Some(&attributes), writer);
            self.write_line_separators_after(&attributes, original);
        }
        writer.write_punctuation(">");
    }

    fn emit_jsx_closing_element(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        writer.write_punctuation("</");
        if let Some(tag_name) = self.store_for_node(original).tag_name(*original) {
            self.emit_jsx_tag_name(&tag_name, writer);
        }
        writer.write_punctuation(">");
    }

    fn emit_jsx_fragment(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        if let Some(opening) = self.store_for_node(original).opening_fragment(*original) {
            self.write_node_worker(Some(&opening), writer);
        }
        let children = self.store_for_node(original).jsx_children(*original);
        let child_nodes: Vec<_> = children.iter().collect();
        self.emit_list_items(
            original,
            &child_nodes,
            ListFormat::JSX_ELEMENT_OR_FRAGMENT_CHILDREN,
            false,
            children.loc(),
            writer,
        );
        if let Some(closing) = self.store_for_node(original).closing_fragment(*original) {
            self.write_node_worker(Some(&closing), writer);
        }
    }

    fn emit_jsx_attributes(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let properties = self
            .store_for_node(original)
            .source_properties(*original)
            .expect("jsx attributes should have properties");
        let property_nodes: Vec<_> = properties.iter().collect();
        self.emit_list_items(
            original,
            &property_nodes,
            ListFormat::JSX_ELEMENT_ATTRIBUTES,
            false,
            properties.loc(),
            writer,
        );
    }

    fn emit_jsx_attribute(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.emit_jsx_attribute_name(&name, writer);
        }
        if let Some(initializer) = self.store_for_node(original).initializer(*original) {
            writer.write_punctuation("=");
            self.write_node_worker(Some(&initializer), writer);
        }
    }

    fn emit_jsx_spread_attribute(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        writer.write_punctuation("{...");
        if let Some(expression) = self.store_for_node(original).expression(*original) {
            self.emit_expression(&expression, writer);
        }
        writer.write_punctuation("}");
    }

    fn emit_jsx_expression(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let expression = self.store_for_node(original).expression(*original);
        let node_loc = self.loc(original);
        let has_comments_in_empty_expression = !self.comments_disabled
            && !ast::node_is_synthesized(self.store_for_node(original), *original)
            && self.has_comments_at_position(node_loc.pos());
        if expression.is_some() || has_comments_in_empty_expression {
            let indented = self
                .current_source_file
                .as_ref()
                .is_some_and(|source_file| {
                    !ast::node_is_synthesized(self.store_for_node(original), *original)
                        && get_lines_between_positions(source_file, node_loc.pos(), node_loc.end())
                            != 0
                });
            if indented {
                writer.increase_indent();
            }
            writer.write_punctuation("{");
            let open_brace_end = if ast::position_is_synthesized(node_loc.pos()) {
                node_loc.pos()
            } else {
                node_loc.pos() + 1
            };
            let dot_dot_dot_token = self.store_for_node(original).dot_dot_dot_token(*original);
            self.write_node_worker(dot_dot_dot_token.as_ref(), writer);
            if let Some(expression) = expression {
                self.emit_expression_with_precedence(
                    &expression,
                    ast::OPERATOR_PRECEDENCE_DISALLOW_COMMA,
                    writer,
                );
            } else if !self.comments_disabled
                && !ast::node_is_synthesized(self.store_for_node(original), *original)
            {
                self.emit_trailing_comments_to_writer(
                    open_brace_end,
                    CommentSeparator::None,
                    writer,
                );
                self.emit_leading_comments_to_writer(open_brace_end, false, writer);
            }
            let close_brace_pos = greatest_end(
                open_brace_end,
                &[
                    expression
                        .map(|expression| self.loc(&expression))
                        .unwrap_or_else(|| core::new_text_range(-1, -1)),
                    dot_dot_dot_token
                        .map(|token| self.loc(&token))
                        .unwrap_or_else(|| core::new_text_range(-1, -1)),
                ],
            );
            if expression.is_some() || dot_dot_dot_token.is_some() {
                self.emit_leading_comments_to_writer(close_brace_pos, false, writer);
            }
            writer.write_punctuation("}");
            if indented {
                writer.decrease_indent();
            }
        }
    }

    fn emit_jsx_namespaced_name(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        if let Some(namespace) = self.store_for_node(original).namespace(*original) {
            self.emit_identifier_name(&namespace, writer);
        }
        writer.write_punctuation(":");
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.emit_binding_identifier(&name, writer);
        }
    }

    fn emit_jsx_tag_name(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        match self.kind(node) {
            ast::Kind::Identifier => self.emit_identifier_reference(node, writer),
            ast::Kind::PrivateIdentifier => self.emit_identifier_name(node, writer),
            ast::Kind::ThisKeyword => writer.write_keyword("this"),
            ast::Kind::JsxNamespacedName => self.emit_jsx_namespaced_name(node, writer),
            ast::Kind::PropertyAccessExpression => self.emit_expression(node, writer),
            _ => self.write_node_worker(Some(node), writer),
        }
    }

    fn emit_jsx_attribute_name(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        match self.kind(node) {
            ast::Kind::Identifier | ast::Kind::PrivateIdentifier => {
                self.emit_identifier_name(node, writer)
            }
            ast::Kind::JsxNamespacedName => self.emit_jsx_namespaced_name(node, writer),
            _ => self.write_node_worker(Some(node), writer),
        }
    }

    fn emit_import_declaration(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let state = self.enter_node_to_writer(original, writer);
        let modifiers = self.modifiers(original);
        self.emit_modifier_list(original, modifiers.clone(), false, writer);
        self.emit_token_with_comment_to_writer(
            ast::Kind::ImportKeyword,
            greatest_end_nodes(self.loc(original).pos(), &modifiers, self),
            WriteKind::Keyword,
            original,
            writer,
        );
        writer.write_space(" ");
        if let Some(import_clause) = self.store_for_node(original).import_clause(*original) {
            self.emit_import_clause(&import_clause, &import_clause, writer);
            writer.write_space(" ");
            let from_pos = self
                .store_for_node(&import_clause)
                .named_bindings(import_clause)
                .or_else(|| self.store_for_node(&import_clause).name(import_clause))
                .map(|node| self.token_end_before_trailing_comments(&node))
                .unwrap_or_else(|| self.loc(&import_clause).end());
            self.emit_token_with_comment_to_writer(
                ast::Kind::FromKeyword,
                from_pos,
                WriteKind::Keyword,
                original,
                writer,
            );
            writer.write_space(" ");
        }
        if let Some(module_specifier) = self.store_for_node(original).module_specifier(*original) {
            self.emit_expression(&module_specifier, writer);
        }
        if let Some(attributes) = self.store_for_node(original).attributes(*original) {
            writer.write_space(" ");
            self.write_node_worker(Some(&attributes), writer);
        }
        writer.write_trailing_semicolon(";");
        self.exit_node_to_writer(original, state, writer);
    }

    fn emit_import_clause(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let state = self.enter_node_to_writer(original, writer);
        if let Some(phase_modifier) = self
            .store_for_node(original)
            .phase_modifier(*original)
            .filter(|kind| *kind != ast::Kind::Unknown)
        {
            self.emit_token_text_to_writer(phase_modifier, WriteKind::Keyword, writer);
            writer.write_space(" ");
        }
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.emit_binding_identifier(&name, writer);
            if self
                .store_for_node(original)
                .named_bindings(*original)
                .is_some()
            {
                self.emit_token_with_comment_to_writer(
                    ast::Kind::CommaToken,
                    self.token_end_before_trailing_comments(&name),
                    WriteKind::Punctuation,
                    original,
                    writer,
                );
                writer.write_space(" ");
            }
        }
        if let Some(named_bindings) = self.store_for_node(original).named_bindings(*original) {
            match self.kind(&named_bindings) {
                ast::Kind::NamespaceImport => {
                    self.emit_namespace_import(&named_bindings, &named_bindings, writer)
                }
                ast::Kind::NamedImports => {
                    self.emit_named_imports(&named_bindings, &named_bindings, writer)
                }
                _ => panic!(
                    "unhandled NamedImportBindings: {}",
                    self.kind(&named_bindings)
                ),
            }
        }
        self.exit_node_to_writer(original, state, writer);
    }

    fn emit_namespace_import(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let state = self.enter_node_to_writer(original, writer);
        let pos = self.emit_token_with_comment_to_writer(
            ast::Kind::AsteriskToken,
            self.loc(original).pos(),
            WriteKind::Punctuation,
            original,
            writer,
        );
        writer.write_space(" ");
        self.emit_token_with_comment_to_writer(
            ast::Kind::AsKeyword,
            pos,
            WriteKind::Keyword,
            original,
            writer,
        );
        writer.write_space(" ");
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.emit_binding_identifier(&name, writer);
        }
        self.exit_node_to_writer(original, state, writer);
    }

    fn emit_named_imports(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let state = self.enter_node_to_writer(original, writer);
        writer.write_punctuation("{");
        let source_elements = self
            .store_for_node(original)
            .source_elements(*original)
            .expect("named imports should have elements");
        let has_trailing_comma = self.has_trailing_comma(
            original,
            source_elements.has_trailing_comma(),
            source_elements.position_key(),
        );
        let elements = self.node_list(source_elements);
        self.emit_list_items(
            original,
            &elements,
            ListFormat::NAMED_IMPORTS_OR_EXPORTS_ELEMENTS,
            has_trailing_comma,
            source_elements.loc(),
            writer,
        );
        writer.write_punctuation("}");
        self.exit_node_to_writer(original, state, writer);
    }

    fn emit_import_specifier(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        if self
            .store_for_node(original)
            .is_type_only(*original)
            .unwrap_or(false)
        {
            self.emit_token_with_comment_to_writer(
                ast::Kind::TypeKeyword,
                self.loc(original).pos(),
                WriteKind::Keyword,
                original,
                writer,
            );
            writer.write_space(" ");
        }
        if let Some(property_name) = self.store_for_node(original).property_name(*original) {
            self.write_node_worker(Some(&property_name), writer);
            writer.write_space(" ");
            self.emit_token_with_comment_to_writer(
                ast::Kind::AsKeyword,
                self.token_end_at_node_pos(&property_name),
                WriteKind::Keyword,
                original,
                writer,
            );
            writer.write_space(" ");
        }
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.emit_binding_identifier(&name, writer);
        }
    }

    fn emit_expression(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        if self.kind(node) == ast::Kind::Identifier {
            let state = self.enter_node_to_writer(node, writer);
            self.emit_identifier_reference(node, writer);
            self.exit_node_to_writer(node, state, writer);
            return;
        }
        self.write_node_worker(Some(node), writer);
    }

    fn emit_literal_type(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let literal = self.store_for_node(node).literal(*node);
        if let Some(literal) = literal {
            self.emit_expression(&literal, writer);
        }
    }

    fn emit_initializer(
        &mut self,
        node: &ast::Node,
        equal_token_pos: i32,
        context_node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        writer.write_space(" ");
        self.emit_token_with_comment_to_writer(
            ast::Kind::EqualsToken,
            equal_token_pos,
            WriteKind::Operator,
            context_node,
            writer,
        );
        writer.write_space(" ");
        self.emit_expression_with_precedence(node, ast::OPERATOR_PRECEDENCE_DISALLOW_COMMA, writer);
    }

    fn emit_expression_no_asi_with_precedence(
        &mut self,
        node: &ast::Node,
        precedence: ast::OperatorPrecedence,
        writer: &mut dyn EmitTextWriter,
    ) {
        let node = self.parenthesize_expression_for_no_asi(node);
        self.emit_expression_with_precedence(&node, precedence, writer);
    }

    fn comment_will_emit_new_line(&self, comment: ast::CommentRange) -> bool {
        comment.kind == ast::Kind::SingleLineCommentTrivia || comment.has_trailing_new_line
    }

    fn synthetic_comment_will_emit_new_line(
        &self,
        comment: &crate::emitcontext::SynthesizedComment,
    ) -> bool {
        comment.kind == ast::Kind::SingleLineCommentTrivia || comment.has_trailing_new_line
    }

    fn will_emit_leading_new_line(&mut self, node: &ast::Node) -> bool {
        let Some(current_source_file) = self.current_source_file.as_ref() else {
            return false;
        };
        let text = current_source_file.text();
        let mut has_leading_comment_ranges = false;
        let mut has_new_line_comment = false;
        for comment in scanner::get_leading_comment_ranges(text, self.loc(node).pos()) {
            has_leading_comment_ranges = true;
            if self.comment_will_emit_new_line(comment) {
                has_new_line_comment = true;
            }
        }
        if has_leading_comment_ranges && let Some(parse_node) = self.emit_context.parse_node(node) {
            let store = self.store_for_node(&parse_node);
            if store
                .parent(parse_node)
                .is_some_and(|parent| store.kind(parent) == ast::Kind::ParenthesizedExpression)
            {
                return true;
            }
        }
        if has_leading_comment_ranges {
            let mut current = *node;
            while let Some(parent) = self.store_for_node(&current).parent(current) {
                if self.kind(&parent) == ast::Kind::PartiallyEmittedExpression
                    && self.loc(&parent).pos() < self.loc(node).pos()
                {
                    let pos = scanner::skip_trivia(text, self.loc(&parent).pos().max(0) as usize);
                    if text.as_bytes().get(pos) == Some(&b'(') {
                        return true;
                    }
                }
                current = parent;
            }
        }
        if has_new_line_comment {
            return true;
        }
        if self
            .emit_context
            .get_synthetic_leading_comments(node)
            .iter()
            .any(|comment| self.synthetic_comment_will_emit_new_line(comment))
        {
            return true;
        }
        if self.kind(node) == ast::Kind::PartiallyEmittedExpression {
            let source = self.store_for_node(node);
            let Some(expression) = source.expression(*node) else {
                return false;
            };
            if self.loc(node).pos() != self.loc(&expression).pos() {
                for comment in
                    scanner::get_trailing_comment_ranges(text, self.loc(&expression).pos())
                {
                    if self.comment_will_emit_new_line(comment) {
                        return true;
                    }
                }
            }
            return self.will_emit_leading_new_line(&expression);
        }
        false
    }

    fn parenthesize_expression_for_no_asi(&mut self, node: &ast::Node) -> ast::Node {
        if self.comments_disabled {
            return *node;
        }
        match self.kind(node) {
            ast::Kind::PartiallyEmittedExpression => {
                let Some(expression) = self.store_for_node(node).expression(*node) else {
                    return *node;
                };
                if self.will_emit_leading_new_line(node) {
                    if let Some(parse_node) = self.emit_context.parse_node(node)
                        && self.store_for_node(&parse_node).kind(parse_node)
                            == ast::Kind::ParenthesizedExpression
                    {
                        let expression = self.emit_context.import_node_for_emit(&expression);
                        let parens = self
                            .emit_context
                            .factory
                            .node_factory
                            .new_parenthesized_expression(expression);
                        self.emit_context.set_original(&parens, node);
                        let loc = self.store_for_node(&parse_node).loc(parse_node);
                        self.emit_context
                            .factory
                            .node_factory
                            .place_emit_synthetic_node(parens, loc);
                        return parens;
                    }
                    let node = self.emit_context.import_node_for_emit(node);
                    return self
                        .emit_context
                        .factory
                        .node_factory
                        .new_parenthesized_expression(node);
                }
                let updated_expression = self.parenthesize_expression_for_no_asi(&expression);
                if updated_expression == expression {
                    return *node;
                }
                let node_for_update = self.emit_context.import_node_for_emit(node);
                let updated_expression =
                    self.emit_context.import_node_for_emit(&updated_expression);
                self.emit_context
                    .factory
                    .node_factory
                    .update_partially_emitted_expression(node_for_update, updated_expression)
            }
            ast::Kind::PropertyAccessExpression => {
                let Some(expression) = self.store_for_node(node).expression(*node) else {
                    return *node;
                };
                let question_dot_token = self.store_for_node(node).question_dot_token(*node);
                let name = self.store_for_node(node).name(*node);
                let flags = self.store_for_node(node).flags(*node);
                let updated_expression = self.parenthesize_expression_for_no_asi(&expression);
                if updated_expression == expression {
                    return *node;
                }
                let node_for_update = self.emit_context.import_node_for_emit(node);
                let updated_expression =
                    self.emit_context.import_node_for_emit(&updated_expression);
                let question_dot_token =
                    question_dot_token.map(|node| self.emit_context.import_node_for_emit(&node));
                let name = name.map(|node| self.emit_context.import_node_for_emit(&node));
                self.emit_context
                    .factory
                    .node_factory
                    .update_property_access_expression(
                        node_for_update,
                        updated_expression,
                        question_dot_token,
                        name,
                        flags,
                    )
            }
            ast::Kind::ElementAccessExpression => {
                let Some(expression) = self.store_for_node(node).expression(*node) else {
                    return *node;
                };
                let question_dot_token = self.store_for_node(node).question_dot_token(*node);
                let argument_expression = self.store_for_node(node).argument_expression(*node);
                let flags = self.store_for_node(node).flags(*node);
                let updated_expression = self.parenthesize_expression_for_no_asi(&expression);
                if updated_expression == expression {
                    return *node;
                }
                let node_for_update = self.emit_context.import_node_for_emit(node);
                let updated_expression =
                    self.emit_context.import_node_for_emit(&updated_expression);
                let question_dot_token =
                    question_dot_token.map(|node| self.emit_context.import_node_for_emit(&node));
                let argument_expression =
                    argument_expression.map(|node| self.emit_context.import_node_for_emit(&node));
                self.emit_context
                    .factory
                    .node_factory
                    .update_element_access_expression(
                        node_for_update,
                        updated_expression,
                        question_dot_token,
                        argument_expression,
                        flags,
                    )
            }
            ast::Kind::CallExpression => {
                let Some(expression) = self.store_for_node(node).expression(*node) else {
                    return *node;
                };
                let question_dot_token = self.store_for_node(node).question_dot_token(*node);
                let type_argument_nodes = self
                    .store_for_node(node)
                    .type_arguments(*node)
                    .map(|list| list.into_iter().collect::<Vec<_>>());
                let type_arguments = type_argument_nodes.map(|nodes| {
                    let nodes = nodes
                        .into_iter()
                        .map(|node| self.emit_context.import_node_for_emit(&node))
                        .collect::<Vec<_>>();
                    self.emit_context.factory.new_node_list(nodes)
                });
                let arguments = {
                    let nodes: Vec<_> = self
                        .store_for_node(node)
                        .arguments(*node)
                        .unwrap()
                        .into_iter()
                        .collect();
                    let nodes = nodes
                        .into_iter()
                        .map(|node| self.emit_context.import_node_for_emit(&node))
                        .collect::<Vec<_>>();
                    self.emit_context.factory.new_node_list(nodes)
                };
                let flags = self.store_for_node(node).flags(*node);
                let updated_expression = self.parenthesize_expression_for_no_asi(&expression);
                if updated_expression == expression {
                    return *node;
                }
                let node_for_update = self.emit_context.import_node_for_emit(node);
                let updated_expression =
                    self.emit_context.import_node_for_emit(&updated_expression);
                let question_dot_token =
                    question_dot_token.map(|node| self.emit_context.import_node_for_emit(&node));
                self.emit_context
                    .factory
                    .node_factory
                    .update_call_expression(
                        node_for_update,
                        updated_expression,
                        question_dot_token,
                        type_arguments,
                        arguments,
                        flags,
                    )
            }
            ast::Kind::TaggedTemplateExpression => {
                let Some(tag) = self.store_for_node(node).tag(*node) else {
                    return *node;
                };
                let question_dot_token = self.store_for_node(node).question_dot_token(*node);
                let type_argument_nodes = self
                    .store_for_node(node)
                    .type_arguments(*node)
                    .map(|list| list.into_iter().collect::<Vec<_>>());
                let type_arguments = type_argument_nodes.map(|nodes| {
                    let nodes = nodes
                        .into_iter()
                        .map(|node| self.emit_context.import_node_for_emit(&node))
                        .collect::<Vec<_>>();
                    self.emit_context.factory.new_node_list(nodes)
                });
                let template = self.store_for_node(node).template(*node);
                let flags = self.store_for_node(node).flags(*node);
                let updated_tag = self.parenthesize_expression_for_no_asi(&tag);
                if updated_tag == tag {
                    return *node;
                }
                let node_for_update = self.emit_context.import_node_for_emit(node);
                let updated_tag = self.emit_context.import_node_for_emit(&updated_tag);
                let question_dot_token =
                    question_dot_token.map(|node| self.emit_context.import_node_for_emit(&node));
                let template = template.map(|node| self.emit_context.import_node_for_emit(&node));
                self.emit_context
                    .factory
                    .node_factory
                    .update_tagged_template_expression(
                        node_for_update,
                        updated_tag,
                        question_dot_token,
                        type_arguments,
                        template,
                        flags,
                    )
            }
            ast::Kind::PostfixUnaryExpression => {
                let Some(operand) = self.store_for_node(node).operand(*node) else {
                    return *node;
                };
                let operator = self
                    .store_for_node(node)
                    .operator(*node)
                    .unwrap_or(ast::Kind::Unknown);
                let updated_operand = self.parenthesize_expression_for_no_asi(&operand);
                if updated_operand == operand {
                    return *node;
                }
                let node_for_update = self.emit_context.import_node_for_emit(node);
                let updated_operand = self.emit_context.import_node_for_emit(&updated_operand);
                self.emit_context
                    .factory
                    .node_factory
                    .update_postfix_unary_expression(node_for_update, updated_operand, operator)
            }
            ast::Kind::BinaryExpression => {
                let Some(left) = self.store_for_node(node).left(*node) else {
                    return *node;
                };
                let modifier_nodes = self
                    .store_for_node(node)
                    .modifiers(*node)
                    .map(|list| list.into_iter().collect::<Vec<_>>());
                let modifiers = modifier_nodes.map(|nodes| {
                    let nodes = nodes
                        .into_iter()
                        .map(|node| self.emit_context.import_node_for_emit(&node))
                        .collect::<Vec<_>>();
                    self.emit_context.factory.new_modifier_list(nodes)
                });
                let type_node = self.store_for_node(node).r#type(*node);
                let operator_token = self.store_for_node(node).operator_token(*node);
                let right = self.store_for_node(node).right(*node);
                let updated_left = self.parenthesize_expression_for_no_asi(&left);
                if updated_left == left {
                    return *node;
                }
                let node_for_update = self.emit_context.import_node_for_emit(node);
                let updated_left = self.emit_context.import_node_for_emit(&updated_left);
                let type_node = type_node.map(|node| self.emit_context.import_node_for_emit(&node));
                let operator_token =
                    operator_token.map(|node| self.emit_context.import_node_for_emit(&node));
                let right = right.map(|node| self.emit_context.import_node_for_emit(&node));
                self.emit_context
                    .factory
                    .node_factory
                    .update_binary_expression(
                        node_for_update,
                        modifiers,
                        updated_left,
                        type_node,
                        operator_token,
                        right,
                    )
            }
            ast::Kind::ConditionalExpression => {
                let Some(condition) = self.store_for_node(node).condition(*node) else {
                    return *node;
                };
                let question_token = self.store_for_node(node).question_token(*node);
                let when_true = self.store_for_node(node).when_true(*node);
                let colon_token = self.store_for_node(node).colon_token(*node);
                let when_false = self.store_for_node(node).when_false(*node);
                let updated_condition = self.parenthesize_expression_for_no_asi(&condition);
                if updated_condition == condition {
                    return *node;
                }
                let node_for_update = self.emit_context.import_node_for_emit(node);
                let updated_condition = self.emit_context.import_node_for_emit(&updated_condition);
                let question_token =
                    question_token.map(|node| self.emit_context.import_node_for_emit(&node));
                let when_true = when_true.map(|node| self.emit_context.import_node_for_emit(&node));
                let colon_token =
                    colon_token.map(|node| self.emit_context.import_node_for_emit(&node));
                let when_false =
                    when_false.map(|node| self.emit_context.import_node_for_emit(&node));
                self.emit_context
                    .factory
                    .node_factory
                    .update_conditional_expression(
                        node_for_update,
                        updated_condition,
                        question_token,
                        when_true,
                        colon_token,
                        when_false,
                    )
            }
            ast::Kind::AsExpression => {
                let Some(expression) = self.store_for_node(node).expression(*node) else {
                    return *node;
                };
                let type_node = self.store_for_node(node).r#type(*node);
                let updated_expression = self.parenthesize_expression_for_no_asi(&expression);
                if updated_expression == expression {
                    return *node;
                }
                let node_for_update = self.emit_context.import_node_for_emit(node);
                let updated_expression =
                    self.emit_context.import_node_for_emit(&updated_expression);
                let type_node = type_node.map(|node| self.emit_context.import_node_for_emit(&node));
                self.emit_context.factory.node_factory.update_as_expression(
                    node_for_update,
                    updated_expression,
                    type_node,
                )
            }
            ast::Kind::SatisfiesExpression => {
                let Some(expression) = self.store_for_node(node).expression(*node) else {
                    return *node;
                };
                let type_node = self.store_for_node(node).r#type(*node);
                let updated_expression = self.parenthesize_expression_for_no_asi(&expression);
                if updated_expression == expression {
                    return *node;
                }
                let node_for_update = self.emit_context.import_node_for_emit(node);
                let updated_expression =
                    self.emit_context.import_node_for_emit(&updated_expression);
                let type_node = type_node.map(|node| self.emit_context.import_node_for_emit(&node));
                self.emit_context
                    .factory
                    .node_factory
                    .update_satisfies_expression(node_for_update, updated_expression, type_node)
            }
            ast::Kind::NonNullExpression => {
                let Some(expression) = self.store_for_node(node).expression(*node) else {
                    return *node;
                };
                let flags = self.store_for_node(node).flags(*node);
                let updated_expression = self.parenthesize_expression_for_no_asi(&expression);
                if updated_expression == expression {
                    return *node;
                }
                let node_for_update = self.emit_context.import_node_for_emit(node);
                let updated_expression =
                    self.emit_context.import_node_for_emit(&updated_expression);
                self.emit_context
                    .factory
                    .node_factory
                    .update_non_null_expression(node_for_update, updated_expression, flags)
            }
            _ => *node,
        }
    }

    fn emit_short_circuit_expression(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let store = self.store_for_node(node);
        let precedence = if is_binary_operation(store, node, ast::Kind::QuestionQuestionToken) {
            ast::OPERATOR_PRECEDENCE_COALESCE
        } else {
            ast::OPERATOR_PRECEDENCE_LOGICAL_OR
        };
        self.emit_expression_with_precedence(node, precedence, writer);
    }

    fn emit_conditional_expression(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let condition = self.store_for_node(node).condition(*node);
        let question_token = self.store_for_node(node).question_token(*node);
        let when_true = self.store_for_node(node).when_true(*node);
        let colon_token = self.store_for_node(node).colon_token(*node);
        let when_false = self.store_for_node(node).when_false(*node);

        let lines_before_question = condition
            .zip(question_token)
            .map(|(condition, question_token)| {
                self.get_lines_between_nodes(node, &condition, &question_token)
            })
            .unwrap_or(0);
        let lines_after_question = question_token
            .zip(when_true)
            .map(|(question_token, when_true)| {
                self.get_lines_between_nodes(node, &question_token, &when_true)
            })
            .unwrap_or(0);
        let lines_before_colon = when_true
            .zip(colon_token)
            .map(|(when_true, colon_token)| {
                self.get_lines_between_nodes(node, &when_true, &colon_token)
            })
            .unwrap_or(0);
        let lines_after_colon = colon_token
            .zip(when_false)
            .map(|(colon_token, when_false)| {
                self.get_lines_between_nodes(node, &colon_token, &when_false)
            })
            .unwrap_or(0);

        if let Some(condition) = condition {
            self.emit_short_circuit_expression(&condition, writer);
        }
        write_lines_and_indent_to_writer(writer, lines_before_question, true);
        self.write_node_worker(question_token.as_ref(), writer);
        write_lines_and_indent_to_writer(writer, lines_after_question, true);
        if let Some(when_true) = when_true {
            self.emit_expression_with_precedence(
                &when_true,
                ast::OPERATOR_PRECEDENCE_YIELD,
                writer,
            );
        }
        if lines_after_question > 0 {
            writer.decrease_indent();
        }
        if lines_before_question > 0 {
            writer.decrease_indent();
        }
        write_lines_and_indent_to_writer(writer, lines_before_colon, true);
        self.write_node_worker(colon_token.as_ref(), writer);
        write_lines_and_indent_to_writer(writer, lines_after_colon, true);
        if let Some(when_false) = when_false {
            self.emit_expression_with_precedence(
                &when_false,
                ast::OPERATOR_PRECEDENCE_YIELD,
                writer,
            );
        }
        if lines_after_colon > 0 {
            writer.decrease_indent();
        }
        if lines_before_colon > 0 {
            writer.decrease_indent();
        }
    }

    fn emit_assertion_like_expression(
        &mut self,
        node: &ast::Node,
        keyword: &str,
        writer: &mut dyn EmitTextWriter,
    ) {
        if let Some(expression) = self.store_for_node(node).expression(*node) {
            self.emit_expression_with_precedence(
                &expression,
                ast::OPERATOR_PRECEDENCE_RELATIONAL,
                writer,
            );
        }
        writer.write_space(" ");
        writer.write_keyword(keyword);
        writer.write_space(" ");
        if let Some(type_node) = self.store_for_node(node).r#type(*node) {
            self.write_node_worker(Some(&type_node), writer);
        }
    }

    fn emit_non_null_expression(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        if let Some(expression) = self.store_for_node(node).expression(*node) {
            self.emit_expression_with_precedence(
                &expression,
                ast::OPERATOR_PRECEDENCE_MEMBER,
                writer,
            );
        }
        writer.write_punctuation("!");
    }

    fn emit_unary_keyword_expression(
        &mut self,
        keyword: ast::Kind,
        node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_token_with_comment_to_writer(
            keyword,
            self.loc(node).pos(),
            WriteKind::Keyword,
            node,
            writer,
        );
        writer.write_space(" ");
        if let Some(expression) = self.store_for_node(node).expression(*node) {
            self.emit_expression_with_precedence(
                &expression,
                ast::OPERATOR_PRECEDENCE_UNARY,
                writer,
            );
        }
    }

    fn emit_yield_expression(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        self.emit_token_with_comment_to_writer(
            ast::Kind::YieldKeyword,
            self.loc(node).pos(),
            WriteKind::Keyword,
            node,
            writer,
        );
        let asterisk_token = self.store_for_node(node).asterisk_token(*node);
        self.emit_punctuation_node_to_writer(asterisk_token.as_ref(), writer);
        if let Some(expression) = self.store_for_node(node).expression(*node) {
            writer.write_space(" ");
            self.emit_expression_no_asi_with_precedence(
                &expression,
                ast::OPERATOR_PRECEDENCE_DISALLOW_COMMA,
                writer,
            );
        }
    }

    fn emit_prefix_unary_expression(
        &mut self,
        original: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let operator = self
            .store_for_node(original)
            .operator(*original)
            .expect("prefix unary expression should have operator");
        self.emit_token_text_to_writer(operator, WriteKind::Operator, writer);
        if let Some(operand) = self.store_for_node(original).operand(*original) {
            if self.kind(&operand) == ast::Kind::PrefixUnaryExpression {
                let inner = self
                    .store_for_node(&operand)
                    .operator(operand)
                    .expect("prefix unary expression should have operator");
                if (operator == ast::Kind::PlusToken
                    && matches!(inner, ast::Kind::PlusToken | ast::Kind::PlusPlusToken))
                    || (operator == ast::Kind::MinusToken
                        && matches!(inner, ast::Kind::MinusToken | ast::Kind::MinusMinusToken))
                {
                    writer.write_space(" ");
                }
            }
            self.emit_expression_with_precedence(&operand, ast::OPERATOR_PRECEDENCE_UNARY, writer);
        }
    }

    fn emit_postfix_unary_expression(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        if let Some(operand) = self.store_for_node(original).operand(*original) {
            self.emit_expression_with_precedence(&operand, ast::OPERATOR_PRECEDENCE_UPDATE, writer);
        }
        let operator = self
            .store_for_node(original)
            .operator(*original)
            .expect("postfix unary expression should have operator");
        self.emit_token_text_to_writer(operator, WriteKind::Operator, writer);
    }

    fn emit_type_parameters(
        &mut self,
        original: &ast::Node,
        type_parameters: Vec<ast::Node>,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_list_range_with(
            original,
            &type_parameters,
            core::new_text_range(-1, -1),
            type_parameters.is_empty(),
            ListFormat::TYPE_PARAMETERS,
            false,
            writer,
            |printer, child, writer| printer.emit_type_parameter_declaration_node(child, writer),
        );
    }

    fn emit_type_arguments(
        &mut self,
        original: &ast::Node,
        type_arguments: Vec<ast::Node>,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_list_range_with(
            original,
            &type_arguments,
            core::new_text_range(-1, -1),
            type_arguments.is_empty(),
            ListFormat::TYPE_ARGUMENTS,
            false,
            writer,
            |printer, child, writer| printer.emit_type_parameter_declaration_node(child, writer),
        );
    }

    fn emit_type_parameter_declaration_node(
        &mut self,
        node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        // QuickInfo uses TypeFormatFlagsWriteTypeArgumentsOfSignature to instruct the NodeBuilder to
        // store type arguments (i.e. type nodes) instead of type parameter declarations in the type
        // parameter list.
        if ast::is_type_parameter_declaration(self.store_for_node(node), *node) {
            self.emit_type_parameter(node, node, writer);
        } else {
            self.emit_type_node_outside_extends(node, writer);
        }
    }

    fn emit_qualified_name(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        if let Some(left) = self.store_for_node(original).left(*original) {
            self.emit_entity_name(&left, writer);
        }
        writer.write_punctuation(".");
        if let Some(right) = self.store_for_node(original).right(*original) {
            self.emit_identifier_name(&right, writer);
        }
    }

    fn emit_entity_name(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        match self.kind(node) {
            ast::Kind::Identifier => self.emit_identifier_reference(node, writer),
            ast::Kind::QualifiedName => self.write_node_worker(Some(node), writer),
            ast::Kind::PropertyAccessExpression => self.emit_expression(node, writer),
            _ => self.write_node_worker(Some(node), writer),
        }
    }

    fn emit_module_reference(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        match self.kind(node) {
            ast::Kind::Identifier | ast::Kind::QualifiedName => self.emit_entity_name(node, writer),
            ast::Kind::ExternalModuleReference => self.emit_external_module_reference(node, writer),
            _ => self.write_node_worker(Some(node), writer),
        }
    }

    fn emit_type_reference(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        if let Some(type_name) = self.store_for_node(original).type_name(*original) {
            self.emit_entity_name(&type_name, writer);
        }
        self.emit_type_arguments(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_type_arguments(*original),
            ),
            writer,
        );
    }

    fn emit_type_query(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        writer.write_keyword("typeof");
        writer.write_space(" ");
        if let Some(expr_name) = self.store_for_node(original).expr_name(*original) {
            self.emit_entity_name(&expr_name, writer);
        }
        self.emit_type_arguments(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_type_arguments(*original),
            ),
            writer,
        );
    }

    fn emit_array_type(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        if let Some(element_type) = self.store_for_node(original).element_type(*original) {
            self.emit_type_node(&element_type, ast::TYPE_PRECEDENCE_POSTFIX, writer);
        }
        writer.write_punctuation("[");
        writer.write_punctuation("]");
    }

    fn emit_tuple_type(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        writer.write_punctuation("[");
        let elements_view = self
            .store_for_node(original)
            .source_elements(*original)
            .expect("tuple type should have elements");
        let elements_loc = elements_view.loc();
        let has_trailing_comma = elements_view.has_trailing_comma();
        let elements: Vec<_> = elements_view.iter().collect();
        let format = if self.should_emit_on_single_line(original) {
            ListFormat::SINGLE_LINE_TUPLE_TYPE_ELEMENTS
        } else {
            ListFormat::MULTI_LINE_TUPLE_TYPE_ELEMENTS
        } | ListFormat::NO_SPACE_IF_EMPTY;
        self.emit_list_range_with(
            original,
            &elements,
            elements_loc,
            false,
            format,
            has_trailing_comma,
            writer,
            |printer, child, writer| printer.emit_type_node_outside_extends(child, writer),
        );
        writer.write_punctuation("]");
    }

    fn emit_union_type(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_type_list_with_format(
            original,
            self.node_list(
                self.store_for_node(original)
                    .source_types(*original)
                    .expect("union type should have types"),
            ),
            ListFormat::UNION_TYPE_CONSTITUENTS,
            writer,
        );
    }

    fn emit_intersection_type(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_type_list_with_format(
            original,
            self.node_list(
                self.store_for_node(original)
                    .source_types(*original)
                    .expect("intersection type should have types"),
            ),
            ListFormat::INTERSECTION_TYPE_CONSTITUENTS,
            writer,
        );
    }

    fn emit_type_list_with_format(
        &mut self,
        original: &ast::Node,
        types: Vec<ast::Node>,
        format: ListFormat,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_list_items_with(
            original,
            &types,
            format,
            false,
            core::new_text_range(-1, -1),
            writer,
            |printer, child, writer| {
                printer.emit_type_node(child, ast::TYPE_PRECEDENCE_TYPE_OPERATOR, writer)
            },
        );
    }

    fn emit_parenthesized_type(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        writer.write_punctuation("(");
        if let Some(ty) = self.store_for_node(original).r#type(*original) {
            self.emit_type_node_outside_extends(&ty, writer);
        }
        writer.write_punctuation(")");
    }

    fn emit_optional_type(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        if let Some(ty) = self.store_for_node(original).r#type(*original) {
            self.emit_type_node(&ty, ast::TYPE_PRECEDENCE_POSTFIX, writer);
        }
        writer.write_punctuation("?");
    }

    fn emit_rest_type(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        writer.write_punctuation("...");
        if let Some(ty) = self.store_for_node(original).r#type(*original) {
            self.emit_type_node_outside_extends(&ty, writer);
        }
    }

    fn emit_function_type(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let indented = self.should_emit_indented(original);
        self.increase_indent_if(indented);
        self.push_name_generation_scope(original);
        self.emit_type_parameters(
            original,
            Self::optional_node_list(self.store_for_node(original).type_parameters(*original)),
            writer,
        );
        self.emit_parameters(
            original,
            EmitNodeList::from_source(
                self.store_for_node(original)
                    .source_parameters(*original)
                    .expect("function type should have parameters"),
            ),
            writer,
        );
        writer.write_space(" ");
        self.emit_return_type(self.store_for_node(original).r#type(*original), writer);
        self.pop_name_generation_scope(original);
        self.decrease_indent_if(indented);
    }

    fn emit_type_literal(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.push_name_generation_scope(original);
        let (members_loc, member_nodes) = {
            let members = self
                .store_for_node(original)
                .source_members(*original)
                .expect("type literal should have members");
            (members.loc(), members.iter().collect::<Vec<_>>())
        };
        self.generate_all_member_names(&member_nodes);
        writer.write_punctuation("{");
        let format = if self.should_emit_on_single_line(original) {
            ListFormat::SINGLE_LINE_TYPE_LITERAL_MEMBERS
        } else {
            ListFormat::MULTI_LINE_TYPE_LITERAL_MEMBERS
        } | ListFormat::NO_SPACE_IF_EMPTY;
        self.emit_list_range(original, &member_nodes, members_loc, false, format, writer);
        writer.write_punctuation("}");
        self.pop_name_generation_scope(original);
    }

    fn emit_constructor_type(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        self.emit_modifier_list(original, self.modifiers(original), false, writer);
        writer.write_keyword("new");
        writer.write_space(" ");
        let indented = self.should_emit_indented(original);
        self.increase_indent_if(indented);
        self.push_name_generation_scope(original);
        self.emit_type_parameters(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_type_parameters(*original),
            ),
            writer,
        );
        self.emit_parameters(
            original,
            EmitNodeList::from_source(
                self.store_for_node(original)
                    .source_parameters(*original)
                    .expect("constructor type should have parameters"),
            ),
            writer,
        );
        writer.write_space(" ");
        self.emit_return_type(self.store_for_node(original).r#type(*original), writer);
        self.pop_name_generation_scope(original);
        self.decrease_indent_if(indented);
    }

    fn emit_return_type(&mut self, ty: Option<ast::Node>, writer: &mut dyn EmitTextWriter) {
        let Some(ty) = ty else {
            return;
        };
        writer.write_punctuation("=>");
        writer.write_space(" ");
        let needs_parens = self.in_extends
            && self.kind(&ty) == ast::Kind::InferType
            && self
                .store_for_node(&ty)
                .type_parameter(ty)
                .is_some_and(|type_parameter| {
                    self.store_for_node(&type_parameter)
                        .constraint(type_parameter)
                        .is_some()
                });
        if needs_parens {
            self.emit_type_node_preserving_extends(&ty, ast::TYPE_PRECEDENCE_HIGHEST, writer);
        } else {
            self.emit_type_node_preserving_extends(&ty, ast::TYPE_PRECEDENCE_LOWEST, writer);
        }
    }

    fn emit_conditional_type(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        if let Some(check_type) = self.store_for_node(original).check_type(*original) {
            self.emit_type_node(&check_type, ast::TYPE_PRECEDENCE_UNION, writer);
        }
        writer.write_space(" ");
        writer.write_keyword("extends");
        writer.write_space(" ");
        if let Some(extends_type) = self.store_for_node(original).extends_type(*original) {
            self.emit_type_node_in_extends(&extends_type, writer);
        }
        writer.write_space(" ");
        writer.write_punctuation("?");
        writer.write_space(" ");
        if let Some(true_type) = self.store_for_node(original).true_type(*original) {
            self.emit_type_node_outside_extends(&true_type, writer);
        }
        writer.write_space(" ");
        writer.write_punctuation(":");
        writer.write_space(" ");
        if let Some(false_type) = self.store_for_node(original).false_type(*original) {
            self.emit_type_node_outside_extends(&false_type, writer);
        }
    }

    fn emit_infer_type(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        writer.write_keyword("infer");
        writer.write_space(" ");
        if let Some(type_parameter) = self.store_for_node(original).type_parameter(*original) {
            if let Some(name) = self.store_for_node(&type_parameter).name(type_parameter) {
                self.emit_binding_identifier(&name, writer);
            }
            if let Some(constraint) = self
                .store_for_node(&type_parameter)
                .constraint(type_parameter)
            {
                writer.write_space(" ");
                writer.write_keyword("extends");
                writer.write_space(" ");
                self.emit_type_node_in_extends(&constraint, writer);
            }
        }
    }

    fn emit_type_operator(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        if let Some(operator) = self.store_for_node(original).operator(*original) {
            self.emit_token_text_to_writer(operator, WriteKind::Keyword, writer);
            writer.write_space(" ");
        }
        if let Some(ty) = self.store_for_node(original).r#type(*original) {
            let precedence = if self
                .store_for_node(original)
                .operator(*original)
                .is_some_and(|operator| operator == ast::Kind::ReadonlyKeyword)
            {
                ast::TYPE_PRECEDENCE_POSTFIX
            } else {
                ast::TYPE_PRECEDENCE_TYPE_OPERATOR
            };
            self.emit_type_node(&ty, precedence, writer);
        }
    }

    fn emit_indexed_access_type(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        if let Some(object_type) = self.store_for_node(original).object_type(*original) {
            self.emit_type_node(&object_type, ast::TYPE_PRECEDENCE_POSTFIX, writer);
        }
        writer.write_punctuation("[");
        if let Some(index_type) = self.store_for_node(original).index_type(*original) {
            self.emit_type_node_outside_extends(&index_type, writer);
        }
        writer.write_punctuation("]");
    }

    fn emit_mapped_type(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let single_line = self.should_emit_on_single_line(original);
        writer.write_punctuation("{");
        if single_line {
            writer.write_space(" ");
        } else {
            writer.write_line();
            writer.increase_indent();
        }
        if let Some(readonly_token) = self.store_for_node(original).readonly_token(*original) {
            self.write_node_worker(Some(&readonly_token), writer);
            if self.kind(&readonly_token) != ast::Kind::ReadonlyKeyword {
                writer.write_keyword("readonly");
            }
            writer.write_space(" ");
        }
        writer.write_punctuation("[");
        if let Some(type_parameter) = self.store_for_node(original).type_parameter(*original) {
            if let Some(name) = self.store_for_node(&type_parameter).name(type_parameter) {
                self.emit_binding_identifier(&name, writer);
            }
            writer.write_space(" ");
            writer.write_keyword("in");
            writer.write_space(" ");
            if let Some(constraint) = self
                .store_for_node(&type_parameter)
                .constraint(type_parameter)
            {
                self.emit_type_node_outside_extends(&constraint, writer);
            }
        }
        if let Some(name_type) = self.store_for_node(original).name_type(*original) {
            writer.write_space(" ");
            writer.write_keyword("as");
            writer.write_space(" ");
            self.emit_type_node_outside_extends(&name_type, writer);
        }
        writer.write_punctuation("]");
        if let Some(question_token) = self.store_for_node(original).question_token(*original) {
            self.write_node_worker(Some(&question_token), writer);
            if self.kind(&question_token) != ast::Kind::QuestionToken {
                writer.write_punctuation("?");
            }
        }
        writer.write_punctuation(":");
        writer.write_space(" ");
        if let Some(ty) = self.store_for_node(original).r#type(*original) {
            self.emit_type_node_outside_extends(&ty, writer);
        }
        writer.write_trailing_semicolon(";");
        if let Some(members) = self.store_for_node(original).source_members(*original) {
            let member_nodes: Vec<_> = members.iter().collect();
            if !member_nodes.is_empty() {
                if single_line {
                    writer.write_space(" ");
                } else {
                    writer.write_line();
                }
                self.emit_list_items(
                    original,
                    &member_nodes,
                    ListFormat::PRESERVE_LINES,
                    false,
                    members.loc(),
                    writer,
                );
            }
        }
        if single_line {
            writer.write_space(" ");
        } else {
            writer.write_line();
            writer.decrease_indent();
        }
        writer.write_punctuation("}");
    }

    fn emit_named_tuple_member(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        self.write_node_worker(
            self.store_for_node(original)
                .dot_dot_dot_token(*original)
                .as_ref(),
            writer,
        );
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.emit_identifier_name(&name, writer);
        }
        self.write_node_worker(
            self.store_for_node(original)
                .question_token(*original)
                .as_ref(),
            writer,
        );
        writer.write_punctuation(":");
        writer.write_space(" ");
        if let Some(ty) = self.store_for_node(original).r#type(*original) {
            self.emit_type_node_outside_extends(&ty, writer);
        }
    }

    fn emit_template_literal_type(
        &mut self,
        original: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        if let Some(head) = self.store_for_node(original).head(*original) {
            self.write_node_worker(Some(&head), writer);
        }
        let source_spans = self
            .store_for_node(original)
            .source_template_spans(*original);
        let spans = source_spans.expect("template literal type should have spans");
        let span_nodes: Vec<_> = spans.iter().collect();
        self.emit_list_items_with(
            original,
            &span_nodes,
            ListFormat::TEMPLATE_EXPRESSION_SPANS,
            false,
            spans.loc(),
            writer,
            |printer, child, writer| printer.emit_template_literal_type_span(child, writer),
        );
    }

    fn emit_template_literal_type_span(
        &mut self,
        original: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        if let Some(ty) = self.store_for_node(original).r#type(*original) {
            self.emit_type_node_outside_extends(&ty, writer);
        }
        if let Some(literal) = self.store_for_node(original).literal(*original) {
            self.write_node_worker(Some(&literal), writer);
        }
    }

    fn emit_import_type(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        if self
            .store_for_node(original)
            .is_type_of(*original)
            .unwrap_or(false)
        {
            writer.write_keyword("typeof");
            writer.write_space(" ");
        }
        writer.write_keyword("import");
        writer.write_punctuation("(");
        if let Some(argument) = self.store_for_node(original).argument(*original) {
            self.write_node_worker(Some(&argument), writer);
        }
        if let Some(attributes) = self.store_for_node(original).attributes(*original) {
            writer.write_punctuation(",");
            writer.write_space(" ");
            self.emit_import_type_node_attributes(&attributes, writer);
        }
        writer.write_punctuation(")");
        if let Some(qualifier) = self.store_for_node(original).qualifier(*original) {
            writer.write_punctuation(".");
            self.write_node_worker(Some(&qualifier), writer);
        }
        self.emit_type_arguments(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_type_arguments(*original),
            ),
            writer,
        );
    }

    fn emit_import_type_node_attributes(
        &mut self,
        original: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        writer.write_punctuation("{");
        writer.write_space(" ");
        let token = self
            .store_for_node(original)
            .token(*original)
            .unwrap_or(ast::Kind::WithKeyword);
        writer.write_keyword(if token == ast::Kind::AssertKeyword {
            "assert"
        } else {
            "with"
        });
        writer.write_punctuation(":");
        writer.write_space(" ");
        let attributes =
            Self::optional_node_list(self.store_for_node(original).source_attributes(*original));
        self.emit_list_range(
            original,
            &attributes,
            core::new_text_range(-1, -1),
            attributes.is_empty(),
            ListFormat::IMPORT_ATTRIBUTES,
            writer,
        );
        writer.write_space(" ");
        writer.write_punctuation("}");
    }

    fn emit_type_predicate(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        if let Some(asserts_modifier) = self.store_for_node(original).asserts_modifier(*original) {
            self.write_node_worker(Some(&asserts_modifier), writer);
            writer.write_space(" ");
        }
        if let Some(parameter_name) = self.store_for_node(original).parameter_name(*original) {
            self.write_node_worker(Some(&parameter_name), writer);
        }
        if let Some(ty) = self.store_for_node(original).r#type(*original) {
            writer.write_space(" ");
            writer.write_keyword("is");
            writer.write_space(" ");
            self.emit_type_node_outside_extends(&ty, writer);
        }
    }

    fn emit_type_annotation(
        &mut self,
        original: &ast::Node,
        _ty: Option<ast::Node>,
        writer: &mut dyn EmitTextWriter,
    ) {
        if let Some(ty) = self.store_for_node(original).r#type(*original) {
            writer.write_punctuation(":");
            writer.write_space(" ");
            self.emit_type_node_outside_extends(&ty, writer);
        }
    }

    fn emit_signature_like(
        &mut self,
        original: &ast::Node,
        type_parameters: Vec<ast::Node>,
        parameters: EmitNodeList,
        ty: Option<ast::Node>,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.push_name_generation_scope(original);
        self.emit_type_parameters(original, type_parameters, writer);
        self.emit_parameters(original, parameters, writer);
        self.emit_type_annotation(original, ty, writer);
        self.pop_name_generation_scope(original);
    }

    fn emit_property_signature(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_modifier_list(original, self.modifiers(original), false, writer);
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.emit_property_name(&name, writer);
        }
        let postfix_token = self.store_for_node(original).postfix_token(*original);
        self.write_node_worker(postfix_token.as_ref(), writer);
        self.emit_type_annotation(
            original,
            self.store_for_node(original).r#type(*original),
            writer,
        );
        writer.write_trailing_semicolon(";");
    }

    fn emit_method_signature(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_modifier_list(original, self.modifiers(original), false, writer);
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.emit_property_name(&name, writer);
        }
        let postfix_token = self.store_for_node(original).postfix_token(*original);
        self.write_node_worker(postfix_token.as_ref(), writer);
        self.emit_signature_like(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_type_parameters(*original),
            ),
            EmitNodeList::from_source(
                self.store_for_node(original)
                    .source_parameters(*original)
                    .expect("method signature should have parameters"),
            ),
            self.store_for_node(original).r#type(*original),
            writer,
        );
        writer.write_trailing_semicolon(";");
    }

    fn emit_property_name(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let saved_write_kind = self.write_kind;
        self.write_kind = WriteKind::Property;

        match self.kind(node) {
            ast::Kind::Identifier => self.emit_identifier_name(node, writer),
            ast::Kind::PrivateIdentifier => self.emit_private_identifier(node, writer),
            ast::Kind::StringLiteral
            | ast::Kind::NoSubstitutionTemplateLiteral
            | ast::Kind::NumericLiteral
            | ast::Kind::BigIntLiteral => {
                let state = self.enter_node_to_writer(node, writer);
                let current_source_file = share_source_file_option(&self.current_source_file);
                writer.write_string_literal(&self.get_literal_text_of_node(
                    node,
                    current_source_file.as_ref(),
                    GetLiteralTextFlags::NONE,
                ));
                self.exit_node_to_writer(node, state, writer);
            }
            ast::Kind::ComputedPropertyName => self.emit_computed_property_name(node, writer),
            _ => panic!("unexpected PropertyName: {:?}", self.kind(node)),
        }

        self.write_kind = saved_write_kind;
    }

    fn emit_member_name(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        match self.kind(node) {
            ast::Kind::Identifier => self.emit_identifier_name(node, writer),
            ast::Kind::PrivateIdentifier => self.emit_private_identifier(node, writer),
            _ => panic!("unexpected MemberName: {:?}", self.kind(node)),
        }
    }

    fn emit_call_signature(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_signature_like(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_type_parameters(*original),
            ),
            EmitNodeList::from_source(
                self.store_for_node(original)
                    .source_parameters(*original)
                    .expect("call signature should have parameters"),
            ),
            self.store_for_node(original).r#type(*original),
            writer,
        );
        writer.write_trailing_semicolon(";");
    }

    fn emit_construct_signature(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        writer.write_keyword("new");
        writer.write_space(" ");
        self.emit_signature_like(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_type_parameters(*original),
            ),
            EmitNodeList::from_source(
                self.store_for_node(original)
                    .source_parameters(*original)
                    .expect("construct signature should have parameters"),
            ),
            self.store_for_node(original).r#type(*original),
            writer,
        );
        writer.write_trailing_semicolon(";");
    }

    fn emit_index_signature(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_modifier_list(original, self.modifiers(original), false, writer);
        let indented = self.should_emit_indented(original);
        self.increase_indent_if(indented);
        self.push_name_generation_scope(original);
        self.emit_parameters_for_index_signature(
            original,
            EmitNodeList::from_source(
                self.store_for_node(original)
                    .source_parameters(*original)
                    .expect("index signature should have parameters"),
            ),
            writer,
        );
        self.emit_type_annotation(
            original,
            self.store_for_node(original).r#type(*original),
            writer,
        );
        writer.write_trailing_semicolon(";");
        self.pop_name_generation_scope(original);
        self.decrease_indent_if(indented);
    }

    fn emit_type_parameter(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_modifier_list(original, self.modifiers(original), false, writer);
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.emit_identifier_name(&name, writer);
        }
        if let Some(constraint) = self.store_for_node(original).constraint(*original) {
            writer.write_space(" ");
            writer.write_keyword("extends");
            writer.write_space(" ");
            self.emit_type_node_outside_extends(&constraint, writer);
        }
        if let Some(default_type) = self.store_for_node(original).default_type(*original) {
            writer.write_space(" ");
            writer.write_operator("=");
            writer.write_space(" ");
            self.emit_type_node_outside_extends(&default_type, writer);
        }
    }

    fn emit_heritage_clause(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        writer.write_space(" ");
        self.emit_token_text_to_writer(
            self.store_for_node(original)
                .token(*original)
                .expect("heritage clause should have token"),
            WriteKind::Keyword,
            writer,
        );
        writer.write_space(" ");
        let types = self.node_list(
            self.store_for_node(original)
                .source_types(*original)
                .expect("heritage clause should have types"),
        );
        self.emit_list_items(
            original,
            &types,
            ListFormat::HERITAGE_CLAUSE_TYPES,
            false,
            core::new_text_range(-1, -1),
            writer,
        );
    }

    fn emit_heritage_clauses(
        &mut self,
        original: &ast::Node,
        heritage_clauses: Vec<ast::Node>,
        format: ListFormat,
        writer: &mut dyn EmitTextWriter,
    ) {
        if heritage_clauses.is_empty() {
            return;
        }
        self.emit_list_range(
            original,
            &heritage_clauses,
            core::new_text_range(-1, -1),
            false,
            format,
            writer,
        );
    }

    fn emit_expression_with_type_arguments(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        if let Some(expression) = self.store_for_node(original).expression(*original) {
            self.emit_expression_with_precedence(
                &expression,
                ast::OPERATOR_PRECEDENCE_MEMBER,
                writer,
            );
        }
        self.emit_type_arguments(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_type_arguments(*original),
            ),
            writer,
        );
    }

    fn emit_class_declaration(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.generate_name_if_needed(self.store_for_node(original).name(*original));
        let pos = self.emit_modifier_list(original, self.modifiers(original), true, writer);
        self.emit_token_with_comment_to_writer(
            ast::Kind::ClassKeyword,
            pos,
            WriteKind::Keyword,
            original,
            writer,
        );
        let name = self.store_for_node(original).name(*original);
        if name.is_some_and(|name| {
            ast::node_is_present(self.store_for_node(original), Some(name))
                && !self.store_for_node(original).text(name).is_empty()
        }) {
            let name = name.expect("present class expression name");
            writer.write_space(" ");
            self.emit_identifier_name(&name, writer);
        }
        let indented = self.should_emit_indented(original);
        if indented {
            writer.increase_indent();
        }
        self.emit_type_parameters(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_type_parameters(*original),
            ),
            writer,
        );
        self.emit_heritage_clauses(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_heritage_clauses(*original),
            ),
            ListFormat::CLASS_HERITAGE_CLAUSES,
            writer,
        );
        writer.write_space(" ");
        self.push_name_generation_scope(original);
        let members = self
            .store_for_node(original)
            .source_members(*original)
            .expect("class declaration should have members");
        let members_loc = members.loc();
        let members: Vec<_> = members.iter().collect();
        self.generate_all_member_names(&members);
        self.emit_class_members(original, members_loc, members, writer);
        self.pop_name_generation_scope(original);
        if indented {
            writer.decrease_indent();
        }
    }

    fn emit_function_declaration(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let state = self.enter_node_to_writer(original, writer);
        self.emit_modifier_list(original, self.modifiers(original), false, writer);
        writer.write_keyword("function");
        let asterisk_token = self.store_for_node(original).asterisk_token(*original);
        self.emit_token_node(asterisk_token.as_ref());
        writer.write_space(" ");
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.emit_identifier_name(&name, writer);
        }
        self.emit_type_parameters(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_type_parameters(*original),
            ),
            writer,
        );
        self.push_name_generation_scope(original);
        self.emit_parameters(
            original,
            EmitNodeList::from_source(
                self.store_for_node(original)
                    .source_parameters(*original)
                    .expect("function declaration should have parameters"),
            ),
            writer,
        );
        self.emit_type_annotation(
            original,
            self.store_for_node(original).r#type(*original),
            writer,
        );
        if let Some(body) = self.store_for_node(original).body(*original) {
            writer.write_space(" ");
            self.emit_function_body_node(&body, writer);
        } else {
            writer.write_punctuation(";");
        }
        self.pop_name_generation_scope(original);
        self.exit_node_to_writer(original, state, writer);
    }

    fn emit_interface_declaration(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_modifier_list(original, self.modifiers(original), false, writer);
        writer.write_keyword("interface");
        writer.write_space(" ");
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.emit_binding_identifier(&name, writer);
        }
        self.emit_type_parameters(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_type_parameters(*original),
            ),
            writer,
        );
        self.emit_heritage_clauses(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_heritage_clauses(*original),
            ),
            ListFormat::HERITAGE_CLAUSES,
            writer,
        );
        writer.write_space(" ");
        writer.write_punctuation("{");
        self.push_name_generation_scope(original);
        let members = self
            .store_for_node(original)
            .source_members(*original)
            .expect("interface declaration should have members");
        let members_loc = members.loc();
        let members: Vec<_> = members.iter().collect();
        self.generate_all_member_names(&members);
        self.emit_list_range(
            original,
            &members,
            members_loc,
            false,
            ListFormat::INTERFACE_MEMBERS,
            writer,
        );
        self.pop_name_generation_scope(original);
        writer.write_punctuation("}");
    }

    fn emit_type_alias_declaration(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_modifier_list(original, self.modifiers(original), false, writer);
        writer.write_keyword("type");
        writer.write_space(" ");
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.emit_binding_identifier(&name, writer);
        }
        self.emit_type_parameters(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_type_parameters(*original),
            ),
            writer,
        );
        writer.write_space(" ");
        writer.write_punctuation("=");
        writer.write_space(" ");
        if let Some(type_node) = self.store_for_node(original).r#type(*original) {
            self.write_node_worker(Some(&type_node), writer);
        }
        writer.write_trailing_semicolon(";");
    }

    fn emit_class_expression(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let state = self.enter_node_to_writer(original, writer);
        self.generate_name_if_needed(self.store_for_node(original).name(*original));
        let pos = self.emit_modifier_list(original, self.modifiers(original), true, writer);
        self.emit_token_with_comment_to_writer(
            ast::Kind::ClassKeyword,
            pos,
            WriteKind::Keyword,
            original,
            writer,
        );
        let name = self.store_for_node(original).name(*original);
        if name.is_some_and(|name| {
            ast::node_is_present(self.store_for_node(original), Some(name))
                && !self.store_for_node(original).text(name).is_empty()
        }) {
            let name = name.expect("present class expression name");
            writer.write_space(" ");
            self.emit_identifier_name(&name, writer);
        }
        let indented = self.should_emit_indented(original);
        if indented {
            writer.increase_indent();
        }
        self.emit_type_parameters(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_type_parameters(*original),
            ),
            writer,
        );
        self.emit_heritage_clauses(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_heritage_clauses(*original),
            ),
            ListFormat::CLASS_HERITAGE_CLAUSES,
            writer,
        );
        writer.write_space(" ");
        self.push_name_generation_scope(original);
        let members = self
            .store_for_node(original)
            .source_members(*original)
            .expect("class expression should have members");
        let members_loc = members.loc();
        let members: Vec<_> = members.iter().collect();
        self.generate_all_member_names(&members);
        self.emit_class_members(original, members_loc, members, writer);
        self.pop_name_generation_scope(original);
        if indented {
            writer.decrease_indent();
        }
        self.exit_node_to_writer(original, state, writer);
    }

    fn emit_class_members(
        &mut self,
        original: &ast::Node,
        members_loc: core::TextRange,
        members: Vec<ast::Node>,
        writer: &mut dyn EmitTextWriter,
    ) {
        writer.write_punctuation("{");
        self.emit_list_range(
            original,
            &members,
            members_loc,
            false,
            ListFormat::CLASS_MEMBERS,
            writer,
        );
        writer.write_punctuation("}");
    }

    fn emit_semicolon_class_element(&mut self, _node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        writer.write_punctuation(";");
    }

    fn emit_class_static_block_declaration(
        &mut self,
        original: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        writer.write_keyword("static");
        writer.write_space(" ");
        self.push_name_generation_scope(original);
        if let Some(body) = self.store_for_node(original).body(*original) {
            self.emit_function_body_node(&body, writer);
        } else {
            writer.write_punctuation("{");
            writer.write_punctuation("}");
        }
        self.pop_name_generation_scope(original);
    }

    fn emit_property_declaration(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_modifier_list(original, self.modifiers(original), true, writer);
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.write_node_worker(Some(&name), writer);
        }
        self.write_node_worker(
            self.store_for_node(original)
                .postfix_token(*original)
                .as_ref(),
            writer,
        );
        if let Some(type_node) = self.store_for_node(original).r#type(*original) {
            writer.write_punctuation(":");
            writer.write_space(" ");
            self.write_node_worker(Some(&type_node), writer);
        }
        if let Some(initializer) = self.store_for_node(original).initializer(*original) {
            writer.write_space(" ");
            writer.write_operator("=");
            writer.write_space(" ");
            self.emit_expression(&initializer, writer);
        }
        writer.write_punctuation(";");
    }

    fn emit_method_declaration(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_modifier_list(original, self.modifiers(original), true, writer);
        self.write_node_worker(
            self.store_for_node(original)
                .asterisk_token(*original)
                .as_ref(),
            writer,
        );
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.write_node_worker(Some(&name), writer);
        }
        self.write_node_worker(
            self.store_for_node(original)
                .postfix_token(*original)
                .as_ref(),
            writer,
        );
        self.push_name_generation_scope(original);
        self.emit_signature_like(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_type_parameters(*original),
            ),
            EmitNodeList::from_source(
                self.store_for_node(original)
                    .source_parameters(*original)
                    .expect("method declaration should have parameters"),
            ),
            self.store_for_node(original).r#type(*original),
            writer,
        );
        if let Some(body) = self.store_for_node(original).body(*original) {
            writer.write_space(" ");
            self.emit_function_body_node(&body, writer);
        } else {
            writer.write_trailing_semicolon(";");
        }
        self.pop_name_generation_scope(original);
    }

    fn emit_get_accessor_declaration(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_accessor_declaration(
            original,
            ast::Kind::GetKeyword,
            self.modifiers(original),
            self.store_for_node(original).name(*original),
            EmitNodeList::from_source(
                self.store_for_node(original)
                    .source_parameters(*original)
                    .expect("get accessor should have parameters"),
            ),
            self.store_for_node(original).r#type(*original),
            self.store_for_node(original).body(*original),
            writer,
        );
    }

    fn emit_set_accessor_declaration(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_accessor_declaration(
            original,
            ast::Kind::SetKeyword,
            self.modifiers(original),
            self.store_for_node(original).name(*original),
            EmitNodeList::from_source(
                self.store_for_node(original)
                    .source_parameters(*original)
                    .expect("set accessor should have parameters"),
            ),
            self.store_for_node(original).r#type(*original),
            self.store_for_node(original).body(*original),
            writer,
        );
    }

    fn emit_accessor_declaration(
        &mut self,
        original: &ast::Node,
        keyword: ast::Kind,
        modifiers: Vec<ast::Node>,
        _name: Option<ast::Node>,
        parameters: EmitNodeList,
        type_node: Option<ast::Node>,
        body: Option<ast::Node>,
        writer: &mut dyn EmitTextWriter,
    ) {
        let pos = self.emit_modifier_list(original, modifiers, true, writer);
        self.emit_token_with_comment_to_writer(keyword, pos, WriteKind::Keyword, original, writer);
        writer.write_space(" ");
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.write_node_worker(Some(&name), writer);
        }
        self.push_name_generation_scope(original);
        self.emit_parameters(original, parameters, writer);
        self.emit_type_annotation(original, type_node, writer);
        if let Some(body) = body {
            writer.write_space(" ");
            self.emit_function_body_node(&body, writer);
        } else {
            writer.write_trailing_semicolon(";");
        }
        self.pop_name_generation_scope(original);
    }

    fn emit_constructor_declaration(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_modifier_list(original, self.modifiers(original), false, writer);
        writer.write_keyword("constructor");
        self.push_name_generation_scope(original);
        self.emit_parameters(
            original,
            EmitNodeList::from_source(
                self.store_for_node(original)
                    .source_parameters(*original)
                    .expect("constructor should have parameters"),
            ),
            writer,
        );
        if let Some(type_node) = self.store_for_node(original).r#type(*original) {
            writer.write_punctuation(":");
            writer.write_space(" ");
            self.write_node_worker(Some(&type_node), writer);
        }
        if let Some(body) = self.store_for_node(original).body(*original) {
            writer.write_space(" ");
            self.emit_function_body_node(&body, writer);
        } else {
            writer.write_trailing_semicolon(";");
        }
        self.pop_name_generation_scope(original);
    }

    fn emit_trailing_synthetic_comments_with_writer(
        &mut self,
        node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let emit_flags = self.emit_context.emit_flags(node);
        if emit_flags & EF_NO_TRAILING_COMMENTS != 0 {
            return;
        }
        for comment in self.synthetic_trailing_comments_with_originals(node) {
            if !writer.is_at_start_of_line() {
                writer.write_space(" ");
            }
            match comment.kind {
                ast::Kind::MultiLineCommentTrivia => {
                    writer.write_comment(&format!("/*{}*/", comment.text));
                }
                ast::Kind::SingleLineCommentTrivia => {
                    writer.write_comment(&format!("//{}", comment.text));
                }
                _ => {}
            }
            if comment.has_trailing_new_line {
                writer.write_line();
            }
        }
    }

    fn synthetic_trailing_comments_with_originals(
        &mut self,
        node: &ast::Node,
    ) -> Vec<crate::emitcontext::SynthesizedComment> {
        let comments = self.emit_context.get_synthetic_trailing_comments(node);
        if !comments.is_empty() {
            return comments;
        }
        let mut original = self.emit_context.original(node);
        while let Some(node) = original {
            let comments = self.emit_context.get_synthetic_trailing_comments(&node);
            if !comments.is_empty() {
                return comments;
            }
            original = self.emit_context.original(&node);
        }
        Vec::new()
    }

    fn emit_function_expression(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_modifier_list(original, self.modifiers(original), false, writer);
        writer.write_keyword("function");
        if self
            .store_for_node(original)
            .asterisk_token(*original)
            .is_some()
        {
            writer.write_operator("*");
        }
        let name = self.store_for_node(original).name(*original);
        if let Some(name) = name {
            writer.write_space(" ");
            self.emit_identifier_name(&name, writer);
        } else {
            writer.write_space(" ");
        }
        self.emit_type_parameters(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_type_parameters(*original),
            ),
            writer,
        );
        self.push_name_generation_scope(original);
        self.emit_parameters(
            original,
            EmitNodeList::from_source(
                self.store_for_node(original)
                    .source_parameters(*original)
                    .expect("function expression should have parameters"),
            ),
            writer,
        );
        self.emit_type_annotation(
            original,
            self.store_for_node(original).r#type(*original),
            writer,
        );
        writer.write_space(" ");
        if let Some(body) = self.store_for_node(original).body(*original) {
            self.emit_function_body_node(&body, writer);
        } else {
            writer.write_punctuation("{}");
        }
        self.pop_name_generation_scope(original);
    }

    fn can_emit_simple_arrow_head(&self, original: &ast::Node) -> bool {
        let store = self.store_for_node(original);
        let parameters = store
            .source_parameters(*original)
            .expect("arrow function should have parameters");
        if parameters.len() != 1 {
            return false;
        };

        let Some(parameter_node) = parameters.first() else {
            return false;
        };
        if !ast::is_parameter_declaration(store, parameter_node) {
            return false;
        }
        store.loc(parameter_node).pos() == store.loc(*original).pos()
            && store.type_parameters(*original).is_none()
            && store.r#type(*original).is_none()
            && store
                .source_modifiers(*original)
                .is_none_or(|modifiers| modifiers.is_empty())
            && store
                .source_modifiers(parameter_node)
                .is_none_or(|modifiers| modifiers.is_empty())
            && store.dot_dot_dot_token(parameter_node).is_none()
            && store.question_token(parameter_node).is_none()
            && store.r#type(parameter_node).is_none()
            && store.initializer(parameter_node).is_none()
            && store
                .name(parameter_node)
                .is_some_and(|name| ast::is_identifier(store, name))
    }

    fn emit_parameters_for_arrow(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        if self.can_emit_simple_arrow_head(original) {
            let parameters = self
                .store_for_node(original)
                .source_parameters(*original)
                .expect("arrow function should have parameters");
            let parameters = parameters.iter().collect::<Vec<_>>();
            self.generate_all_names(&parameters);
            self.emit_list_range(
                original,
                &parameters,
                core::new_text_range(-1, -1),
                parameters.is_empty(),
                ListFormat::SINGLE_ARROW_PARAMETER,
                writer,
            );
        } else {
            self.emit_parameters(
                original,
                EmitNodeList::from_source(
                    self.store_for_node(original)
                        .source_parameters(*original)
                        .expect("arrow function should have parameters"),
                ),
                writer,
            );
        }
    }

    fn emit_concise_body(&mut self, body: Option<ast::Node>, writer: &mut dyn EmitTextWriter) {
        let Some(body) = body else {
            return;
        };
        if self.kind(&body) == ast::Kind::Block {
            self.emit_function_body_node(&body, writer);
        } else if ast::is_object_literal_expression(
            self.store_for_node(&body),
            ast::get_leftmost_expression(self.store_for_node(&body), &body, false),
        ) {
            // Wrap in ParenthesizedExpression to ensure parens are emitted after any leading
            // PartiallyEmittedExpression comments, matching TypeScript's factory-time wrapping
            // via parenthesizeConciseBodyOfArrowFunction.
            let loc = self.loc(&body);
            let body = self.emit_context.import_node_for_emit(&body);
            let paren = self
                .emit_context
                .factory
                .node_factory
                .new_parenthesized_expression(body);
            self.emit_context
                .factory
                .node_factory
                .place_emit_synthetic_node(paren, loc);
            self.emit_expression(&paren, writer);
        } else {
            self.emit_expression_with_precedence(&body, ast::OPERATOR_PRECEDENCE_YIELD, writer);
        }
    }

    fn emit_arrow_function(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_modifier_list(original, self.modifiers(original), false, writer);
        self.push_name_generation_scope(original);
        self.emit_type_parameters(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_type_parameters(*original),
            ),
            writer,
        );
        self.emit_parameters_for_arrow(original, original, writer);
        self.emit_type_annotation(
            original,
            self.store_for_node(original).r#type(*original),
            writer,
        );
        writer.write_space(" ");
        self.emit_token_node(
            self.store_for_node(original)
                .equals_greater_than_token(*original)
                .as_ref(),
        );
        writer.write_space(" ");
        self.emit_concise_body(self.store_for_node(original).body(*original), writer);
        self.pop_name_generation_scope(original);
    }

    fn emit_parameter(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_modifier_list(original, self.modifiers(original), true, writer);
        if let Some(dot_dot_dot_token) = self.store_for_node(original).dot_dot_dot_token(*original)
        {
            self.write_node_worker(Some(&dot_dot_dot_token), writer);
        }
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.emit_binding_name(&name, writer);
        }
        let question_token = self.store_for_node(original).question_token(*original);
        self.write_node_worker(question_token.as_ref(), writer);
        self.emit_type_annotation(
            original,
            self.store_for_node(original).r#type(*original),
            writer,
        );
        if let Some(initializer) = self.store_for_node(original).initializer(*original) {
            writer.write_space(" ");
            writer.write_operator("=");
            writer.write_space(" ");
            self.emit_expression(&initializer, writer);
        }
    }

    fn emit_parameters(
        &mut self,
        original: &ast::Node,
        parameters: EmitNodeList,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.generate_all_names(&parameters.nodes);
        self.emit_list_range_with_trailing_comma(
            original,
            &parameters.nodes,
            parameters.loc(),
            parameters.is_missing(),
            ListFormat::PARAMETERS,
            parameters.has_trailing_comma(),
            writer,
        );
    }

    fn emit_parameters_for_index_signature(
        &mut self,
        original: &ast::Node,
        parameters: EmitNodeList,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.generate_all_names(&parameters.nodes);
        self.emit_list_range_with_trailing_comma(
            original,
            &parameters.nodes,
            parameters.loc(),
            parameters.is_missing(),
            ListFormat::INDEX_SIGNATURE_PARAMETERS,
            parameters.has_trailing_comma(),
            writer,
        );
    }

    fn emit_identifier_text(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let _symbol = self.node_symbol(node);
        let text = self.get_text_of_node(node, false);
        writer.write_symbol(&text, None);
        if let Some(type_arguments) = self.emit_context.get_identifier_type_arguments(node) {
            let type_arguments = self
                .emit_context
                .factory
                .node_factory
                .emit_node_list_nodes(type_arguments);
            self.emit_list_range_with(
                node,
                &type_arguments,
                core::new_text_range(-1, -1),
                type_arguments.is_empty(),
                ListFormat::TYPE_PARAMETERS,
                false,
                writer,
                |printer, child, writer| {
                    printer.emit_type_parameter_declaration_node(child, writer)
                },
            );
        }
    }

    fn emit_identifier_name(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let state = self.enter_node_to_writer(node, writer);
        self.emit_identifier_text(node, writer);
        self.exit_node_to_writer(node, state, writer);
    }

    fn emit_private_identifier(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let state = self.enter_node_to_writer(node, writer);
        let text = self.get_text_of_node(node, false);
        writer.write(&text);
        self.exit_node_to_writer(node, state, writer);
    }

    fn get_unique_helper_name(&mut self, name: &str) -> ast::Node {
        if let Some(helper_name) = self
            .unique_helper_names
            .as_ref()
            .and_then(|helper_names| helper_names.get(name).copied())
        {
            return self.emit_context.clone_node_for_emit(&helper_name);
        }

        let helper_name = self.emit_context.factory.new_unique_name_ex(
            name,
            AutoGenerateOptions {
                flags: GeneratedIdentifierFlags::FILE_LEVEL | GeneratedIdentifierFlags::OPTIMISTIC,
                ..Default::default()
            },
        );
        self.generate_name(&helper_name);
        self.unique_helper_names
            .as_mut()
            .expect("unique helper names should be allocated before helper substitution")
            .insert(name.to_string(), helper_name);
        helper_name
    }

    fn emit_identifier_reference(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        if (self.external_helpers_module_name.is_some() || self.unique_helper_names.is_some())
            && self.emit_context.emit_flags(node) & EF_HELPER_NAME != 0
        {
            if let Some(external_helpers_module_name) = self.external_helpers_module_name {
                let external_helpers_module_name = self
                    .emit_context
                    .clone_node_for_emit(&external_helpers_module_name);
                let helper_name = self.emit_context.clone_node_for_emit(node);
                let helper = self
                    .emit_context
                    .factory
                    .node_factory
                    .new_property_access_expression(
                        external_helpers_module_name,
                        None,
                        helper_name,
                        ast::NodeFlags::NONE,
                    );
                self.emit_context
                    .assign_comment_and_source_map_ranges(&helper, node);
                self.emit_property_access_expression(&helper, &helper, writer);
                return;
            }

            let name = self.store_for_node(node).text(*node);
            let helper_name = self.get_unique_helper_name(&name);
            self.emit_context
                .assign_comment_and_source_map_ranges(&helper_name, node);
            let state = self.enter_node_to_writer(&helper_name, writer);
            self.emit_identifier_text(&helper_name, writer);
            self.exit_node_to_writer(&helper_name, state, writer);
            return;
        }

        let state = self.enter_node_to_writer(node, writer);
        self.emit_identifier_text(node, writer);
        self.exit_node_to_writer(node, state, writer);
    }

    fn emit_binding_identifier(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let node = if self.unique_helper_names.is_some()
            && self.emit_context.emit_flags(node) & EF_HELPER_NAME != 0
        {
            let name = self.store_for_node(node).text(*node);
            let helper_name = self.get_unique_helper_name(&name);
            self.emit_context
                .assign_comment_and_source_map_ranges(&helper_name, node);
            helper_name
        } else {
            *node
        };

        let state = self.enter_node_to_writer(&node, writer);
        self.emit_identifier_text(&node, writer);
        self.exit_node_to_writer(&node, state, writer);
    }

    fn emit_binding_name(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        match self.kind(node) {
            ast::Kind::Identifier => self.emit_binding_identifier(node, writer),
            ast::Kind::ObjectBindingPattern => self.emit_object_binding_pattern(node, writer),
            ast::Kind::ArrayBindingPattern => self.emit_array_binding_pattern(node, writer),
            _ => self.write_node_worker(Some(node), writer),
        }
    }

    fn emit_parameter_name(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let saved_write_kind = self.write_kind;
        self.write_kind = WriteKind::Parameter;
        self.emit_binding_name(node, writer);
        self.write_kind = saved_write_kind;
    }

    fn emit_property_access_expression(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let state = self.enter_node_to_writer(original, writer);
        if let Some(expression) = self.store_for_node(original).expression(*original) {
            if is_new_expression_without_arguments(self.store_for_node(&expression), &expression) {
                self.emit_expression_with_precedence(
                    &expression,
                    ast::OPERATOR_PRECEDENCE_PARENTHESES,
                    writer,
                );
            } else {
                let precedence = if ast::is_optional_chain(self.store_for_node(original), *original)
                {
                    ast::OPERATOR_PRECEDENCE_OPTIONAL_CHAIN
                } else {
                    ast::OPERATOR_PRECEDENCE_MEMBER
                };
                self.emit_expression_with_precedence(&expression, precedence, writer);
            }
        }
        let question_dot_token = self.store_for_node(original).question_dot_token(*original);
        let name = self.store_for_node(original).name(*original);
        let expression = self.store_for_node(original).expression(*original);
        let token = match question_dot_token.as_ref() {
            Some(token) => Some(*token),
            None => expression.as_ref().map(|expression| {
                let token = self.emit_context.factory.new_token(ast::Kind::DotToken);
                let token_end = name
                    .as_ref()
                    .map(|name| self.loc(name).pos())
                    .unwrap_or_else(|| self.loc(original).end());
                self.emit_context
                    .factory
                    .node_factory
                    .place_emit_synthetic_node(
                        token,
                        core::new_text_range(self.loc(expression).end(), token_end),
                    );
                self.emit_context.mark_emit_node(&token, EF_NO_SOURCE_MAP);
                token
            }),
        };
        let lines_before_dot = match (expression.as_ref(), token.as_ref()) {
            (Some(expression), Some(token)) => {
                self.get_lines_between_nodes(original, expression, token)
            }
            _ => 0,
        };
        let lines_after_dot = match (token.as_ref(), name.as_ref()) {
            (Some(token), Some(name)) => self.get_lines_between_nodes(original, token, name),
            _ => 0,
        };
        for _ in 0..lines_before_dot {
            writer.write_line();
        }
        if lines_before_dot > 0 {
            writer.increase_indent();
        }
        let should_emit_dot_dot = question_dot_token.is_none()
            && expression
                .as_ref()
                .is_some_and(|expression| self.may_need_dot_dot_for_property_access(expression))
            && !writer.has_trailing_comment()
            && !writer.has_trailing_whitespace();
        if should_emit_dot_dot {
            writer.write_punctuation(".");
        }
        if let Some(token) = question_dot_token.as_ref() {
            self.write_node_worker(Some(token), writer);
        } else if let Some(expression) = expression {
            self.emit_token_with_comment_to_writer(
                ast::Kind::DotToken,
                self.loc(&expression).end(),
                WriteKind::Punctuation,
                original,
                writer,
            );
        }
        for _ in 0..lines_after_dot {
            writer.write_line();
        }
        if lines_after_dot > 0 {
            writer.increase_indent();
        }
        if let Some(name) = name {
            self.emit_member_name(&name, writer);
        }
        if lines_after_dot > 0 {
            writer.decrease_indent();
        }
        if lines_before_dot > 0 {
            writer.decrease_indent();
        }
        self.exit_node_to_writer(original, state, writer);
    }

    // 1..toString is a valid property access, emit a dot after the literal
    fn may_need_dot_dot_for_property_access(&mut self, expression: &ast::Node) -> bool {
        let expression =
            ast::skip_partially_emitted_expressions(self.store_for_node(expression), *expression);
        let store = self.store_for_node(&expression);
        if ast::is_numeric_literal(store, expression) {
            let has_specifier = store
                .token_flags(expression)
                .is_some_and(|flags| flags.intersects(ast::TokenFlags::WITH_SPECIFIER));
            // check if numeric literal is a decimal literal that was originally written with a dot
            let text = self.get_literal_text_of_node(
                &expression,
                None,
                GetLiteralTextFlags::NEVER_ASCII_ESCAPE,
            );
            // If the number will be printed verbatim and it doesn't already contain a dot or an exponent indicator, add one
            // if the expression doesn't have any comments that will be emitted.
            return !has_specifier
                && !text.contains(".")
                && !text.contains("E")
                && !text.contains("e");
        }
        false
    }

    fn emit_element_access_expression(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let expression = self.store_for_node(original).expression(*original);
        let question_dot_token = self.store_for_node(original).question_dot_token(*original);
        let argument_expression = self.store_for_node(original).argument_expression(*original);
        if let Some(expression) = expression.as_ref() {
            let precedence = if ast::is_optional_chain(self.store_for_node(original), *original) {
                ast::OPERATOR_PRECEDENCE_OPTIONAL_CHAIN
            } else {
                ast::OPERATOR_PRECEDENCE_MEMBER
            };
            self.emit_expression_with_precedence(expression, precedence, writer);
        }
        if let Some(question_dot_token) = question_dot_token.as_ref() {
            self.write_node_worker(Some(question_dot_token), writer);
        }

        let mut open_bracket_preceding_nodes = Vec::with_capacity(2);
        if let Some(expression) = expression {
            open_bracket_preceding_nodes.push(expression);
        }
        if let Some(question_dot_token) = question_dot_token {
            open_bracket_preceding_nodes.push(question_dot_token);
        }
        self.emit_token_with_comment_to_writer(
            ast::Kind::OpenBracketToken,
            greatest_end_nodes(-1, &open_bracket_preceding_nodes, self),
            WriteKind::Punctuation,
            original,
            writer,
        );
        if let Some(argument_expression) = argument_expression.as_ref() {
            self.emit_expression_with_precedence(
                argument_expression,
                ast::OPERATOR_PRECEDENCE_COMMA,
                writer,
            );
        }
        self.emit_token_with_comment_to_writer(
            ast::Kind::CloseBracketToken,
            argument_expression
                .as_ref()
                .map(|argument_expression| self.loc(argument_expression).end())
                .unwrap_or_else(|| self.loc(original).end()),
            WriteKind::Punctuation,
            original,
            writer,
        );
    }

    fn emit_call_expression(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        if let Some(expression) = self.store_for_node(original).expression(*original) {
            self.emit_callee(&expression, original, writer);
        }
        if self
            .store_for_node(original)
            .question_dot_token(*original)
            .is_some()
        {
            writer.write_punctuation("?.");
        }
        self.emit_type_arguments(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_type_arguments(*original),
            ),
            writer,
        );
        let source_arguments = self
            .store_for_node(original)
            .source_arguments(*original)
            .expect("call expression should have arguments");
        let arguments = self.node_list(source_arguments);
        self.emit_list_range_with(
            original,
            &arguments,
            source_arguments.loc(),
            false,
            ListFormat::CALL_EXPRESSION_ARGUMENTS,
            false,
            writer,
            |printer, child, writer| {
                printer.emit_expression_with_precedence(
                    child,
                    ast::OPERATOR_PRECEDENCE_SPREAD,
                    writer,
                );
            },
        );
    }

    fn emit_callee(
        &mut self,
        callee: &ast::Node,
        parent_node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        if self.should_emit_indirect_call(parent_node) {
            writer.write_punctuation("(");
            writer.write_literal("0");
            writer.write_punctuation(",");
            writer.write_space(" ");
            self.emit_expression_with_precedence(callee, ast::OPERATOR_PRECEDENCE_COMMA, writer);
            writer.write_punctuation(")");
        } else if self.kind(parent_node) == ast::Kind::CallExpression
            && is_new_expression_without_arguments(self.store_for_node(callee), callee)
        {
            // Parenthesize `new C` inside of a CallExpression so it is treated as `(new C)()` and not `new C()`
            self.emit_expression_with_precedence(
                callee,
                ast::OPERATOR_PRECEDENCE_PARENTHESES,
                writer,
            );
        } else {
            let precedence =
                if ast::is_optional_chain(self.store_for_node(parent_node), *parent_node) {
                    ast::OPERATOR_PRECEDENCE_OPTIONAL_CHAIN
                } else {
                    ast::OPERATOR_PRECEDENCE_MEMBER
                };
            self.emit_expression_with_precedence(callee, precedence, writer);
        }
    }

    fn emit_tagged_template_expression(
        &mut self,
        original: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        if let Some(tag) = self.store_for_node(original).tag(*original) {
            self.emit_callee(&tag, original, writer);
        }
        self.emit_type_arguments(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_type_arguments(*original),
            ),
            writer,
        );
        writer.write_space(" ");
        if let Some(template) = self.store_for_node(original).template(*original) {
            self.write_node_worker(Some(&template), writer);
        }
    }

    fn emit_template_expression(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        if let Some(head) = self.store_for_node(original).head(*original) {
            self.write_node_worker(Some(&head), writer);
        }
        let source_spans = self
            .store_for_node(original)
            .source_template_spans(*original);
        let spans = source_spans.expect("template expression should have spans");
        let span_nodes: Vec<_> = spans.iter().collect();
        self.emit_list_items(
            original,
            &span_nodes,
            ListFormat::TEMPLATE_EXPRESSION_SPANS,
            false,
            spans.loc(),
            writer,
        );
    }

    fn emit_template_span(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        if let Some(expression) = self.store_for_node(original).expression(*original) {
            self.emit_expression(&expression, writer);
        }
        if let Some(literal) = self.store_for_node(original).literal(*original) {
            self.write_node_worker(Some(&literal), writer);
        }
    }

    fn emit_spread_element(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_token_with_comment_to_writer(
            ast::Kind::DotDotDotToken,
            self.loc(original).pos(),
            WriteKind::Punctuation,
            original,
            writer,
        );
        if let Some(expression) = self.store_for_node(original).expression(*original) {
            self.emit_expression_with_precedence(
                &expression,
                ast::OPERATOR_PRECEDENCE_DISALLOW_COMMA,
                writer,
            );
        }
    }

    fn emit_new_expression(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_token_with_comment_to_writer(
            ast::Kind::NewKeyword,
            self.loc(original).pos(),
            WriteKind::Keyword,
            original,
            writer,
        );
        writer.write_space(" ");
        if let Some(expression) = self.store_for_node(original).expression(*original) {
            let expression_without_partially_emitted_expressions =
                ast::skip_partially_emitted_expressions(
                    self.store_for_node(&expression),
                    expression,
                );
            if self.kind(&expression_without_partially_emitted_expressions)
                == ast::Kind::CallExpression
            {
                self.emit_expression_with_precedence(
                    &expression,
                    ast::OPERATOR_PRECEDENCE_PARENTHESES,
                    writer,
                );
            } else {
                self.emit_expression_with_precedence(
                    &expression,
                    ast::OPERATOR_PRECEDENCE_MEMBER,
                    writer,
                );
            }
        }
        self.emit_type_arguments(
            original,
            Self::optional_node_list(
                self.store_for_node(original)
                    .source_type_arguments(*original),
            ),
            writer,
        );
        if let Some(source_arguments) = self.store_for_node(original).source_arguments(*original) {
            let arguments = self.node_list(source_arguments);
            self.emit_list_range_with(
                original,
                &arguments,
                source_arguments.loc(),
                false,
                ListFormat::NEW_EXPRESSION_ARGUMENTS,
                false,
                writer,
                |printer, child, writer| {
                    printer.emit_expression_with_precedence(
                        child,
                        ast::OPERATOR_PRECEDENCE_SPREAD,
                        writer,
                    );
                },
            );
        }
    }

    fn emit_object_literal_expression(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let multi_line = self
            .store_for_node(original)
            .multi_line(*original)
            .unwrap_or(false);
        let source_properties = self
            .store_for_node(original)
            .source_properties(*original)
            .expect("object literal should have properties");
        let properties = source_properties.iter().collect::<Vec<_>>();
        let properties_loc = source_properties.loc();
        let properties_missing = source_properties.is_missing();
        let has_trailing_comma = self.has_trailing_comma(
            original,
            source_properties.has_trailing_comma(),
            source_properties.position_key(),
        );
        let mut format =
            if self.should_allow_trailing_comma(original, source_properties.position_key()) {
                ListFormat::OBJECT_LITERAL_EXPRESSION_PROPERTIES | ListFormat::ALLOW_TRAILING_COMMA
            } else {
                ListFormat::OBJECT_LITERAL_EXPRESSION_PROPERTIES
            };
        if multi_line {
            format |= ListFormat::PREFER_NEW_LINE;
        }
        let indented = self.should_emit_indented(original);
        self.increase_indent_if(indented);
        self.push_name_generation_scope(original);
        self.generate_all_member_names(&properties);
        self.emit_list_range_with_trailing_comma(
            original,
            &properties,
            properties_loc,
            properties_missing,
            format,
            has_trailing_comma,
            writer,
        );
        self.pop_name_generation_scope(original);
        self.decrease_indent_if(indented);
    }

    fn emit_array_literal_expression(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let source_elements = self
            .store_for_node(original)
            .source_elements(*original)
            .expect("array literal should have elements");
        let elements = source_elements.iter().collect::<Vec<_>>();
        let has_trailing_comma = source_elements.has_trailing_comma()
            || elements.last().is_some_and(|element| {
                ast::is_omitted_expression(self.store_for_node(element), *element)
            });
        let prefer_new_line = if self
            .store_for_node(original)
            .multi_line(*original)
            .unwrap_or(false)
        {
            ListFormat::PREFER_NEW_LINE
        } else {
            ListFormat::NONE
        };
        self.emit_list_range_with(
            original,
            &elements,
            source_elements.loc(),
            source_elements.is_missing(),
            ListFormat::ARRAY_LITERAL_EXPRESSION_ELEMENTS | prefer_new_line,
            has_trailing_comma,
            writer,
            |printer, child, writer| {
                printer.emit_expression_with_precedence(
                    child,
                    ast::OPERATOR_PRECEDENCE_SPREAD,
                    writer,
                );
            },
        );
    }

    fn emit_property_assignment(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.write_node_worker(Some(&name), writer);
        }
        writer.write_punctuation(":");
        writer.write_space(" ");
        if let Some(initializer) = self.store_for_node(original).initializer(*original) {
            // This is to ensure that we emit comment in the following case:
            //      For example:
            //          obj = {
            //              id: /*comment1*/ ()=>void
            //          }
            // "comment1" is not considered to be leading comment for node.initializer
            // but rather a trailing comment on the previous node.
            if self.emit_context.emit_flags(&initializer) & EF_NO_LEADING_COMMENTS == 0 {
                let comment_range = self.emit_context.comment_range(&initializer);
                self.emit_trailing_comments_of_position_to_writer(
                    comment_range.pos(),
                    false,
                    false,
                    writer,
                );
            }
            self.emit_expression_with_precedence(
                &initializer,
                ast::OPERATOR_PRECEDENCE_DISALLOW_COMMA,
                writer,
            );
        }
    }

    fn emit_shorthand_property_assignment(
        &mut self,
        original: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.write_node_worker(Some(&name), writer);
        }
        if let Some(initializer) = self
            .store_for_node(original)
            .object_assignment_initializer(*original)
        {
            writer.write_space(" ");
            writer.write_punctuation("=");
            writer.write_space(" ");
            self.emit_expression(&initializer, writer);
        }
    }

    fn emit_spread_assignment(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        if let Some(expression) = self.store_for_node(original).expression(*original) {
            self.emit_token_with_comment_to_writer(
                ast::Kind::DotDotDotToken,
                self.loc(original).pos(),
                WriteKind::Punctuation,
                original,
                writer,
            );
            self.emit_expression(&expression, writer);
        }
    }

    fn emit_computed_property_name(
        &mut self,
        original: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        writer.write_punctuation("[");
        if let Some(expression) = self.store_for_node(original).expression(*original) {
            self.emit_expression_with_precedence(
                &expression,
                ast::OPERATOR_PRECEDENCE_DISALLOW_COMMA,
                writer,
            );
        }
        writer.write_punctuation("]");
    }

    fn emit_object_binding_pattern(
        &mut self,
        original: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let state = self.enter_node_to_writer(original, writer);
        writer.write_punctuation("{");
        let source_elements = self
            .store_for_node(original)
            .source_elements(*original)
            .expect("object binding pattern should have elements");
        let elements = source_elements;
        let element_nodes: Vec<_> = elements.iter().collect();
        self.emit_list_items(
            original,
            &element_nodes,
            ListFormat::OBJECT_BINDING_PATTERN_ELEMENTS,
            elements.has_trailing_comma(),
            elements.loc(),
            writer,
        );
        writer.write_punctuation("}");
        self.exit_node_to_writer(original, state, writer);
    }

    fn emit_array_binding_pattern(
        &mut self,
        original: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let state = self.enter_node_to_writer(original, writer);
        writer.write_punctuation("[");
        let source_elements = self
            .store_for_node(original)
            .source_elements(*original)
            .expect("array binding pattern should have elements");
        let elements = source_elements;
        let element_nodes: Vec<_> = elements.iter().collect();
        self.emit_list_items(
            original,
            &element_nodes,
            ListFormat::ARRAY_BINDING_PATTERN_ELEMENTS,
            elements.has_trailing_comma(),
            elements.loc(),
            writer,
        );
        writer.write_punctuation("]");
        self.exit_node_to_writer(original, state, writer);
    }

    fn emit_binding_element(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        self.write_node_worker(
            self.store_for_node(original)
                .dot_dot_dot_token(*original)
                .as_ref(),
            writer,
        );
        if let Some(property_name) = self.store_for_node(original).property_name(*original) {
            self.write_node_worker(Some(&property_name), writer);
            writer.write_punctuation(":");
            writer.write_space(" ");
        }
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.emit_binding_name(&name, writer);
            if let Some(initializer) = self.store_for_node(original).initializer(*original) {
                self.emit_initializer(&initializer, self.loc(&name).end(), original, writer);
            }
        }
    }

    fn emit_void_expression(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_token_with_comment_to_writer(
            ast::Kind::VoidKeyword,
            self.loc(original).pos(),
            WriteKind::Keyword,
            original,
            writer,
        );
        writer.write_space(" ");
        if let Some(expression) = self.store_for_node(original).expression(*original) {
            self.emit_expression_with_precedence(
                &expression,
                ast::OPERATOR_PRECEDENCE_UNARY,
                writer,
            );
        }
    }

    fn emit_parenthesized_expression(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let open_paren_pos = self.emit_token_with_comment_to_writer(
            ast::Kind::OpenParenToken,
            self.loc(original).pos(),
            WriteKind::Punctuation,
            original,
            writer,
        );
        let mut close_paren_pos = open_paren_pos;
        if let Some(expression) = self.store_for_node(original).expression(*original) {
            let indented = self.write_line_separators_and_indent_before(&expression, original);
            self.emit_expression(&expression, writer);
            self.write_line_separators_after(&expression, original);
            self.decrease_indent_if(indented);
            close_paren_pos = self.loc(&expression).end();
        }
        self.emit_token_with_comment_to_writer(
            ast::Kind::CloseParenToken,
            close_paren_pos,
            WriteKind::Punctuation,
            original,
            writer,
        );
    }

    fn emit_decorator(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let state = self.enter_node_to_writer(original, writer);
        writer.write_punctuation("@");
        if let Some(expression) = self.store_for_node(original).expression(*original) {
            self.emit_expression_with_precedence(
                &expression,
                ast::OPERATOR_PRECEDENCE_LEFT_HAND_SIDE,
                writer,
            );
        }
        self.exit_node_to_writer(original, state, writer);
    }

    fn emit_type_assertion_expression(
        &mut self,
        original: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        writer.write_punctuation("<");
        if let Some(ty) = self.store_for_node(original).r#type(*original) {
            self.write_node_worker(Some(&ty), writer);
        }
        writer.write_punctuation(">");
        if let Some(expression) = self.store_for_node(original).expression(*original) {
            self.emit_expression_with_precedence(
                &expression,
                ast::OPERATOR_PRECEDENCE_UPDATE,
                writer,
            );
        }
    }

    fn emit_meta_property(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        if let Some(keyword_token) = self.store_for_node(original).keyword_token(*original) {
            self.emit_token_text_to_writer(keyword_token, WriteKind::Keyword, writer);
        }
        writer.write_punctuation(".");
        if let Some(name) = self.store_for_node(original).name(*original) {
            self.emit_identifier_name(&name, writer);
        }
    }

    fn emit_partially_emitted_expression(
        &mut self,
        original: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let mut stack: Vec<(ast::Node, PrinterState)> = Vec::new();
        let mut node = *original;
        loop {
            let state = self.enter_node(&node);
            let emit_flags = self.emit_context.emit_flags(&node);
            let store = self.store_for_node(&node);
            let expression = store
                .expression(node)
                .expect("partially emitted expression should have expression");
            if emit_flags & EF_NO_LEADING_COMMENTS == 0
                && self.loc(&node).pos() != self.loc(&expression).pos()
            {
                self.emit_trailing_comments_of_position(self.loc(&expression).pos(), false, false);
            }
            stack.push((node, state));
            if !ast::is_partially_emitted_expression(self.store_for_node(&expression), expression) {
                break;
            }
            node = expression;
        }

        let expression = self
            .store_for_node(&node)
            .expression(node)
            .expect("partially emitted expression should have expression");
        self.emit_expression_with_precedence(&expression, ast::OPERATOR_PRECEDENCE_LOWEST, writer);

        while let Some((entry_node, state)) = stack.pop() {
            let emit_flags = self.emit_context.emit_flags(&entry_node);
            let expression = self
                .store_for_node(&entry_node)
                .expression(entry_node)
                .expect("partially emitted expression should have expression");
            if emit_flags & EF_NO_TRAILING_COMMENTS == 0
                && self.loc(&entry_node).end() != self.loc(&expression).end()
            {
                self.emit_leading_comments_of_position(self.loc(&expression).end());
            }
            self.exit_node(&entry_node, state);
            node = entry_node;
        }
    }

    fn emit_synthetic_expression(&mut self, original: &ast::Node, writer: &mut dyn EmitTextWriter) {
        if self
            .store_for_node(original)
            .is_spread(*original)
            .unwrap_or(false)
        {
            writer.write_punctuation("...");
        }
    }

    fn emit_synthetic_reference_expression(
        &mut self,
        original: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        if let Some(expression) = self.store_for_node(original).expression(*original) {
            self.emit_expression(&expression, writer);
        }
    }

    fn emit_binary_expression(
        &mut self,
        original: &ast::Node,
        _node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        enum BinaryAction {
            Enter {
                node: ast::Node,
                precedence: ast::OperatorPrecedence,
                root: bool,
            },
            Operator {
                node: ast::Node,
                lines_before: i32,
                lines_after: i32,
            },
            Exit {
                node: ast::Node,
                state: Option<PrinterState>,
                lines_before: i32,
                lines_after: i32,
                parenthesized: bool,
            },
        }

        let mut stack = vec![BinaryAction::Enter {
            node: *original,
            precedence: ast::OPERATOR_PRECEDENCE_LOWEST,
            root: true,
        }];

        while let Some(action) = stack.pop() {
            match action {
                BinaryAction::Enter {
                    node,
                    precedence,
                    root,
                } => {
                    let emitted = {
                        let store = self.store_for_node(&node);
                        ast::skip_partially_emitted_expressions(store, node)
                    };
                    let node_precedence =
                        ast::get_expression_precedence(self.store_for_node(&emitted), &emitted);
                    let parenthesized = !root && node_precedence < precedence;
                    if parenthesized {
                        writer.write_punctuation("(");
                    }
                    if self.kind(&node) != ast::Kind::BinaryExpression {
                        self.emit_expression(&node, writer);
                        if parenthesized {
                            writer.write_punctuation(")");
                        }
                        continue;
                    }

                    let state = Some(self.enter_node_to_writer(&node, writer));
                    let (mut left_precedence, mut right_precedence) =
                        self.get_binary_expression_precedence(&node);
                    let store = self.store_for_node(&node);
                    let left = store.left(node);
                    let operator_token = store.operator_token(node);
                    let right = store.right(node);
                    let outer_operator_kind = operator_token.map(|operator| self.kind(&operator));

                    if let (Some(operator_kind), Some(left)) = (outer_operator_kind, left) {
                        let left_store = self.store_for_node(&left);
                        let emitted_left =
                            ast::skip_partially_emitted_expressions(left_store, left);
                        if ast::node_is_synthesized(left_store, emitted_left)
                            && left_store.kind(emitted_left) == ast::Kind::BinaryExpression
                            && left_store.operator_token(emitted_left).is_some_and(
                                |inner_operator| {
                                    mixing_binary_operators_requires_parentheses(
                                        operator_kind,
                                        left_store.kind(inner_operator),
                                    )
                                },
                            )
                        {
                            left_precedence = ast::OPERATOR_PRECEDENCE_HIGHEST;
                        }
                    }
                    if let (Some(operator_kind), Some(right)) = (outer_operator_kind, right) {
                        let right_store = self.store_for_node(&right);
                        let emitted_right =
                            ast::skip_partially_emitted_expressions(right_store, right);
                        if ast::node_is_synthesized(right_store, emitted_right)
                            && right_store.kind(emitted_right) == ast::Kind::BinaryExpression
                            && right_store.operator_token(emitted_right).is_some_and(
                                |inner_operator| {
                                    mixing_binary_operators_requires_parentheses(
                                        operator_kind,
                                        right_store.kind(inner_operator),
                                    )
                                },
                            )
                        {
                            right_precedence = ast::OPERATOR_PRECEDENCE_HIGHEST;
                        }
                    }

                    let lines_before = operator_token
                        .zip(left)
                        .map(|(operator, left)| {
                            self.get_lines_between_nodes(&node, &left, &operator)
                        })
                        .unwrap_or(0);
                    let lines_after = operator_token
                        .zip(right)
                        .map(|(operator, right)| {
                            self.get_lines_between_nodes(&node, &operator, &right)
                        })
                        .unwrap_or(0);

                    stack.push(BinaryAction::Exit {
                        node,
                        state,
                        lines_before,
                        lines_after,
                        parenthesized,
                    });
                    if let Some(right) = right {
                        stack.push(BinaryAction::Enter {
                            node: right,
                            precedence: right_precedence,
                            root: false,
                        });
                    }
                    if let Some(operator_token) = operator_token {
                        stack.push(BinaryAction::Operator {
                            node: operator_token,
                            lines_before,
                            lines_after,
                        });
                    } else {
                        writer.write_space(" ");
                    }
                    if let Some(left) = left {
                        stack.push(BinaryAction::Enter {
                            node: left,
                            precedence: left_precedence,
                            root: false,
                        });
                    }
                }
                BinaryAction::Operator {
                    node,
                    lines_before,
                    lines_after,
                } => {
                    let kind = self.kind(&node);
                    write_lines_and_indent_to_writer(
                        writer,
                        lines_before,
                        kind != ast::Kind::CommaToken,
                    );
                    if ast::is_keyword(kind) {
                        self.emit_keyword_node_ex_to_writer(
                            Some(&node),
                            TokenEmitFlags::NO_SOURCE_MAPS,
                            writer,
                        );
                    } else {
                        self.emit_punctuation_node_ex_to_writer(
                            Some(&node),
                            TokenEmitFlags::NO_SOURCE_MAPS,
                            writer,
                        );
                    }
                    write_lines_and_indent_to_writer(writer, lines_after, true);
                }
                BinaryAction::Exit {
                    node,
                    state,
                    lines_before,
                    lines_after,
                    parenthesized,
                } => {
                    if lines_after > 0 {
                        writer.decrease_indent();
                    }
                    if lines_before > 0 {
                        writer.decrease_indent();
                    }
                    if let Some(state) = state {
                        self.exit_node_to_writer(&node, state, writer);
                    }
                    if parenthesized {
                        writer.write_punctuation(")");
                    }
                }
            }
        }
    }

    fn get_binary_expression_precedence(
        &self,
        original: &ast::Node,
    ) -> (ast::OperatorPrecedence, ast::OperatorPrecedence) {
        let precedence = ast::get_expression_precedence(self.store_for_node(original), original);
        let mut left_precedence = precedence;
        let mut right_precedence = precedence;
        match precedence {
            ast::OPERATOR_PRECEDENCE_COMMA
            | ast::OPERATOR_PRECEDENCE_BITWISE_OR
            | ast::OPERATOR_PRECEDENCE_BITWISE_XOR
            | ast::OPERATOR_PRECEDENCE_BITWISE_AND => {}
            ast::OPERATOR_PRECEDENCE_ASSIGNMENT => {
                left_precedence = ast::OPERATOR_PRECEDENCE_CONDITIONAL;
                right_precedence = ast::OPERATOR_PRECEDENCE_YIELD;
            }
            ast::OPERATOR_PRECEDENCE_LOGICAL_OR => {
                right_precedence = ast::OPERATOR_PRECEDENCE_LOGICAL_AND;
            }
            ast::OPERATOR_PRECEDENCE_LOGICAL_AND => {
                right_precedence = ast::OPERATOR_PRECEDENCE_BITWISE_OR;
            }
            ast::OPERATOR_PRECEDENCE_EQUALITY => {
                right_precedence = ast::OPERATOR_PRECEDENCE_RELATIONAL;
            }
            ast::OPERATOR_PRECEDENCE_RELATIONAL => {
                right_precedence = ast::OPERATOR_PRECEDENCE_SHIFT;
            }
            ast::OPERATOR_PRECEDENCE_SHIFT => {
                right_precedence = ast::OPERATOR_PRECEDENCE_ADDITIVE;
            }
            ast::OPERATOR_PRECEDENCE_ADDITIVE => {
                right_precedence = ast::OPERATOR_PRECEDENCE_MULTIPLICATIVE;
            }
            ast::OPERATOR_PRECEDENCE_MULTIPLICATIVE => {
                if self
                    .store_for_node(original)
                    .operator_token(*original)
                    .is_none_or(|token| self.kind(&token) != ast::Kind::AsteriskToken)
                    || self
                        .store_for_node(original)
                        .right(*original)
                        .is_none_or(|right| {
                            if self.kind(&right) != ast::Kind::BinaryExpression {
                                return true;
                            }
                            self.store_for_node(&right)
                                .operator_token(right)
                                .is_none_or(|token| self.kind(&token) != ast::Kind::AsteriskToken)
                        })
                {
                    right_precedence = ast::OPERATOR_PRECEDENCE_EXPONENTIATION;
                }
            }
            ast::OPERATOR_PRECEDENCE_EXPONENTIATION => {
                left_precedence = ast::OPERATOR_PRECEDENCE_UPDATE;
            }
            _ => {}
        }
        (left_precedence, right_precedence)
    }

    fn emit_expression_with_precedence(
        &mut self,
        node: &ast::Node,
        precedence: ast::OperatorPrecedence,
        writer: &mut dyn EmitTextWriter,
    ) {
        let node_precedence = {
            let store = self.store_for_node(node);
            let node = ast::skip_partially_emitted_expressions(store, *node);
            let store = self.store_for_node(&node);
            ast::get_expression_precedence(store, &node)
        };
        if node_precedence < precedence {
            writer.write_punctuation("(");
            self.emit_expression(node, writer);
            writer.write_punctuation(")");
        } else {
            self.emit_expression(node, writer);
        }
    }

    fn emit_type_node_in_extends(&mut self, node: &ast::Node, writer: &mut dyn EmitTextWriter) {
        let saved_in_extends = self.in_extends;
        self.in_extends = true;
        self.emit_type_node_preserving_extends(node, ast::TYPE_PRECEDENCE_LOWEST, writer);
        self.in_extends = saved_in_extends;
    }

    fn emit_type_node_outside_extends(
        &mut self,
        node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        let saved_in_extends = self.in_extends;
        self.in_extends = false;
        self.emit_type_node_preserving_extends(node, ast::TYPE_PRECEDENCE_LOWEST, writer);
        self.in_extends = saved_in_extends;
    }

    fn emit_type_node_preserving_extends(
        &mut self,
        node: &ast::Node,
        precedence: ast::TypePrecedence,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_type_node(node, precedence, writer);
    }

    fn emit_type_node(
        &mut self,
        node: &ast::Node,
        mut precedence: ast::TypePrecedence,
        writer: &mut dyn EmitTextWriter,
    ) {
        if self.in_extends && precedence <= ast::TYPE_PRECEDENCE_CONDITIONAL {
            precedence = ast::TYPE_PRECEDENCE_FUNCTION;
        }

        let node_precedence = {
            let store = self.store_for_node(node);
            ast::get_type_node_precedence(store, node)
        };
        if node_precedence < precedence {
            writer.write_punctuation("(");
            self.write_node_worker(Some(node), writer);
            writer.write_punctuation(")");
        } else {
            self.write_node_worker(Some(node), writer);
        }
    }

    pub fn get_literal_text_of_node(
        &mut self,
        node: &ast::Node,
        source_file: Option<&ast::SourceFile>,
        flags: GetLiteralTextFlags,
    ) -> String {
        if ast::is_string_literal(self.store_for_node(node), *node) {
            if let Some(text_source_node) = self.emit_context.text_source(node) {
                let text = match self.kind(&text_source_node) {
                    ast::Kind::NumericLiteral => self
                        .store_for_node(&text_source_node)
                        .text(text_source_node),
                    ast::Kind::Identifier
                    | ast::Kind::PrivateIdentifier
                    | ast::Kind::JsxNamespacedName => {
                        self.get_text_of_node(&text_source_node, false)
                    }
                    _ => {
                        let store = self.store_for_node(&text_source_node);
                        let source_file_node =
                            ast::get_source_file_of_node(store, Some(text_source_node));
                        let source_file = share_source_file_option(&self.current_source_file)
                            .filter(|source_file| source_file_node == Some(source_file.as_node()));
                        return self.get_literal_text_of_node(
                            &text_source_node,
                            source_file.as_ref(),
                            flags,
                        );
                    }
                };

                if flags.contains(GetLiteralTextFlags::JSX_ATTRIBUTE_ESCAPE) {
                    return format!(
                        "\"{}\"",
                        escape_jsx_attribute_string(text, QuoteChar::DoubleQuote)
                    );
                } else if flags.contains(GetLiteralTextFlags::NEVER_ASCII_ESCAPE)
                    || self.emit_context.emit_flags(node) & EF_NO_ASCII_ESCAPING != 0
                {
                    return format!("\"{}\"", crate::escape_string(text, QuoteChar::DoubleQuote));
                } else {
                    return format!(
                        "\"{}\"",
                        escape_non_ascii_string(text, QuoteChar::DoubleQuote)
                    );
                }
            }
        }
        let mut flags = flags;
        if self.emit_context.emit_flags(node) & EF_NO_ASCII_ESCAPING != 0 {
            flags |= GetLiteralTextFlags::NEVER_ASCII_ESCAPE;
        }
        if self.options.target >= core::ScriptTarget::ES2021 {
            flags |= GetLiteralTextFlags::ALLOW_NUMERIC_SEPARATOR;
        }
        get_literal_text(
            self.store_for_node(node),
            node,
            source_file.or(self.current_source_file.as_ref()),
            flags,
        )
    }

    // `node` must be one of Identifier | PrivateIdentifier | LiteralExpression | JsxNamespacedName
    fn node_parent_chain_is_in_same_store(&self, store: &ast::AstStore, node: ast::Node) -> bool {
        let mut current = node;
        while let Some(parent) = store.parent(current) {
            if parent.store_id() != store.store_id() {
                return false;
            }
            current = parent;
        }
        true
    }

    pub fn get_text_of_node(&mut self, node: &ast::Node, include_trivia: bool) -> String {
        if self.emit_context.has_auto_generate_info(Some(node))
            && ast::is_member_name(self.store_for_node(node), *node)
        {
            let store = self.emit_context.store_for_node(*node);
            let emit_context = &self.emit_context;
            let binding_facts = self.binding_facts_for_node_owned(node);
            return self
                .name_generator
                .generate_name_with_resolver_and_binding_facts(
                    store,
                    node,
                    |node| emit_context.store_for_node(node),
                    binding_facts
                        .as_deref()
                        .map(|facts| facts as &dyn LocalNameBindingFacts),
                );
        }

        if ast::is_string_literal(self.store_for_node(node), *node) {
            if let Some(text_source_node) = self.emit_context.text_source(node) {
                return self.get_text_of_node(&text_source_node, include_trivia);
            }
        }

        let can_use_source_file = {
            let store = self.store_for_node(node);
            self.current_source_file.is_some()
                && store.parent(*node).is_some()
                && self.node_parent_chain_is_in_same_store(store, *node)
                && !ast::node_is_synthesized(store, *node)
                && ast::is_parse_tree_node(store, *node)
        };

        match self.kind(node) {
            ast::Kind::Identifier | ast::Kind::PrivateIdentifier | ast::Kind::JsxNamespacedName => {
                let source_file_mismatch = if can_use_source_file {
                    if let Some(current_source_file) = &self.current_source_file {
                        let most_original = self
                            .emit_context
                            .most_original(&current_source_file.as_node());
                        let most_original_file_name = self
                            .store_for_node(&most_original)
                            .as_source_file(most_original)
                            .file_name();
                        let store = self.store_for_node(node);
                        ast::get_source_file_of_node(store, Some(*node)).is_some_and(
                            |source_file| {
                                store.as_source_file(source_file).file_name()
                                    != most_original_file_name
                            },
                        )
                    } else {
                        false
                    }
                } else {
                    false
                };
                if !can_use_source_file || source_file_mismatch {
                    return self.store_for_node(node).text(*node);
                }
            }
            ast::Kind::StringLiteral
            | ast::Kind::NumericLiteral
            | ast::Kind::BigIntLiteral
            | ast::Kind::NoSubstitutionTemplateLiteral
            | ast::Kind::TemplateHead
            | ast::Kind::TemplateMiddle
            | ast::Kind::TemplateTail => {
                return self.get_literal_text_of_node(node, None, GetLiteralTextFlags::NONE);
            }
            _ => panic!("unexpected node: {}", self.kind(node)),
        }
        if let Some(current_source_file) = &self.current_source_file {
            return scanner::get_source_text_of_node_from_source_file(
                current_source_file,
                node,
                include_trivia,
            );
        }
        self.store_for_node(node).text(*node)
    }

    //
    // Low-level writing
    //

    pub(crate) fn write_as(&mut self, text: &str, write_kind: WriteKind) {
        match write_kind {
            WriteKind::None => self.writer.as_ref().unwrap().borrow_mut().write(text),
            WriteKind::Parameter => self.write_parameter(text),
            WriteKind::Keyword => self.write_keyword(text),
            WriteKind::Operator => self.write_operator(text),
            WriteKind::Property => self.write_property(text),
            WriteKind::Punctuation => self.write_punctuation(text),
            WriteKind::StringLiteral => self
                .writer
                .as_ref()
                .unwrap()
                .borrow_mut()
                .write_string_literal(text),
            WriteKind::Comment => self.write_comment(text),
            WriteKind::Literal => self.write_literal(text),
        }
    }

    pub(crate) fn write(&mut self, text: &str) {
        self.write_as(text, self.write_kind)
    }

    pub(crate) fn set_write_kind(&mut self, kind: WriteKind) -> WriteKind {
        let previous = self.write_kind;
        self.write_kind = kind;
        previous
    }

    pub(crate) fn write_symbol(&mut self, text: &str, opt_symbol: Option<ast::SymbolHandle>) {
        if opt_symbol.is_none() {
            self.write(text)
        } else {
            let _ = opt_symbol;
            self.writer
                .as_ref()
                .unwrap()
                .borrow_mut()
                .write_symbol(text, None)
        }
    }

    pub(crate) fn write_literal(&mut self, text: &str) {
        self.writer
            .as_ref()
            .unwrap()
            .borrow_mut()
            .write_literal(text)
    }

    pub(crate) fn write_punctuation(&mut self, text: &str) {
        self.writer
            .as_ref()
            .unwrap()
            .borrow_mut()
            .write_punctuation(text)
    }

    pub(crate) fn write_operator(&mut self, text: &str) {
        self.writer
            .as_ref()
            .unwrap()
            .borrow_mut()
            .write_operator(text)
    }

    pub(crate) fn write_keyword(&mut self, text: &str) {
        self.writer
            .as_ref()
            .unwrap()
            .borrow_mut()
            .write_keyword(text)
    }

    pub(crate) fn write_property(&mut self, text: &str) {
        self.writer
            .as_ref()
            .unwrap()
            .borrow_mut()
            .write_property(text)
    }

    pub(crate) fn write_parameter(&mut self, text: &str) {
        self.writer
            .as_ref()
            .unwrap()
            .borrow_mut()
            .write_parameter(text)
    }

    pub(crate) fn write_comment(&mut self, text: &str) {
        self.writer
            .as_ref()
            .unwrap()
            .borrow_mut()
            .write_comment(text)
    }

    pub(crate) fn write_space(&mut self) {
        self.writer.as_ref().unwrap().borrow_mut().write_space(" ")
    }

    pub(crate) fn write_line(&mut self) {
        self.writer.as_ref().unwrap().borrow_mut().write_line()
    }

    pub(crate) fn write_line_repeat(&mut self, count: i32) {
        for _ in 0..count {
            self.write_line()
        }
    }

    pub(crate) fn write_lines(&mut self, text: &str) {
        let lines = stringutil::split_lines(text);
        let indentation = stringutil::guess_indentation(&lines);
        for mut line in lines {
            if indentation > 0 {
                line = &line[indentation..];
            }
            if !line.is_empty() {
                self.write_line();
                self.write(&line);
            }
        }
    }

    pub(crate) fn write_trailing_semicolon(&mut self) {
        self.writer
            .as_ref()
            .unwrap()
            .borrow_mut()
            .write_trailing_semicolon(";")
    }

    pub(crate) fn increase_indent(&mut self) {
        self.writer.as_ref().unwrap().borrow_mut().increase_indent()
    }

    pub(crate) fn decrease_indent(&mut self) {
        self.writer.as_ref().unwrap().borrow_mut().decrease_indent()
    }

    pub(crate) fn increase_indent_if(&mut self, indent_requested: bool) {
        if indent_requested {
            self.increase_indent()
        }
    }

    pub(crate) fn decrease_indent_if(&mut self, indent_requested: bool) {
        if indent_requested {
            self.decrease_indent()
        }
    }

    pub(crate) fn write_line_or_space(
        &mut self,
        parent_node: &ast::Node,
        prev_child_node: &ast::Node,
        next_child_node: &ast::Node,
        writer: &mut dyn EmitTextWriter,
    ) {
        if self.should_emit_on_single_line(parent_node) {
            writer.write_space(" ")
        } else if self.options.preserve_source_newlines {
            let lines = self.get_lines_between_nodes(parent_node, prev_child_node, next_child_node);
            if lines > 0 {
                for _ in 0..lines {
                    writer.write_line();
                }
            } else {
                writer.write_space(" ")
            }
        } else {
            writer.write_line()
        }
    }

    pub(crate) fn write_lines_and_indent(
        &mut self,
        line_count: i32,
        write_space_if_not_indenting: bool,
    ) {
        if line_count > 0 {
            self.increase_indent();
            self.write_line_repeat(line_count);
        } else if write_space_if_not_indenting {
            self.write_space()
        }
    }

    pub(crate) fn write_line_separators_and_indent_before(
        &mut self,
        node: &ast::Node,
        parent: &ast::Node,
    ) -> bool {
        if self.options.preserve_source_newlines {
            let leading_newlines =
                self.get_leading_line_terminator_count(parent, Some(node), ListFormat::NONE);
            if leading_newlines > 0 {
                self.write_lines_and_indent(leading_newlines, false);
                return true;
            }
        }
        false
    }

    pub(crate) fn write_line_separators_after(&mut self, node: &ast::Node, parent: &ast::Node) {
        if self.options.preserve_source_newlines {
            let trailing_newlines = self.get_closing_line_terminator_count(
                parent,
                Some(node),
                ListFormat::NONE,
                core::new_text_range(-1, -1),
            );
            if trailing_newlines > 0 {
                self.write_line_repeat(trailing_newlines);
            }
        }
    }

    pub(crate) fn get_lines_between_nodes(
        &mut self,
        parent: &ast::Node,
        node1: &ast::Node,
        node2: &ast::Node,
    ) -> i32 {
        if self.should_elide_indentation(parent) {
            return 0;
        }

        let parent = skip_synthesized_parentheses(self.store_for_node(parent), parent);
        let node1 = skip_synthesized_parentheses(self.store_for_node(node1), node1);
        let node2 = skip_synthesized_parentheses(self.store_for_node(node2), node2);

        // Always use a newline for synthesized code if the synthesizer desires it.
        if self.should_emit_on_new_line(&node2, ListFormat::NONE) {
            return 1;
        }

        if self.current_source_file.is_some()
            && !ast::node_is_synthesized(self.store_for_node(&parent), parent)
            && !ast::node_is_synthesized(self.store_for_node(&node1), node1)
            && !ast::node_is_synthesized(self.store_for_node(&node2), node2)
        {
            if self.options.preserve_source_newlines {
                let current_source_file = self.current_source_file.as_ref().unwrap();
                let node1_loc = self.loc(&node1);
                let node2_loc = self.loc(&node2);
                return self.get_effective_lines(|include_comments| {
                    get_lines_between_range_end_and_range_start(
                        node1_loc,
                        node2_loc,
                        current_source_file,
                        include_comments,
                    )
                });
            }
            let node1_loc = self.loc(&node1);
            let node2_loc = self.loc(&node2);
            return if range_end_is_on_same_line_as_range_start(
                node1_loc,
                node2_loc,
                self.current_source_file.as_ref().unwrap(),
            ) {
                0
            } else {
                1
            };
        }

        0
    }

    pub(crate) fn get_effective_lines(&self, get_line_difference: impl Fn(bool) -> i32) -> i32 {
        // If 'preserveSourceNewlines' is disabled, we should never call this function
        // because it could be more expensive than alternative approximations.
        if !self.options.preserve_source_newlines {
            panic!("Should not be called when preserveSourceNewlines is false")
        }
        // We start by measuring the line difference from a position to its adjacent comments,
        // so that this is counted as a one-line difference, not two:
        //
        //   node1;
        //   // NODE2 COMMENT
        //   node2;
        let lines = get_line_difference(true);
        if lines == 0 {
            // However, if the line difference considering comments was 0, we might have this:
            //
            //   node1; // NODE2 COMMENT
            //   node2;
            //
            // in which case we should be ignoring node2's comment, so this too is counted as
            // a one-line difference, not zero.
            return get_line_difference(false);
        }
        lines
    }

    fn emit_list_range(
        &mut self,
        parent_node: &ast::Node,
        children: &[ast::Node],
        children_text_range: core::TextRange,
        children_missing: bool,
        format: ListFormat,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_list_range_with_trailing_comma(
            parent_node,
            children,
            children_text_range,
            children_missing,
            format,
            false,
            writer,
        );
    }

    fn emit_list_range_with_trailing_comma(
        &mut self,
        parent_node: &ast::Node,
        children: &[ast::Node],
        children_text_range: core::TextRange,
        children_missing: bool,
        format: ListFormat,
        has_trailing_comma: bool,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_list_range_with(
            parent_node,
            children,
            children_text_range,
            children_missing,
            format,
            has_trailing_comma,
            writer,
            |printer, child, writer| printer.write_node_worker(Some(child), writer),
        );
    }

    fn emit_list_range_with<F>(
        &mut self,
        parent_node: &ast::Node,
        children: &[ast::Node],
        children_text_range: core::TextRange,
        children_missing: bool,
        format: ListFormat,
        has_trailing_comma: bool,
        writer: &mut dyn EmitTextWriter,
        mut emit_child: F,
    ) where
        F: FnMut(&mut Self, &ast::Node, &mut dyn EmitTextWriter),
    {
        let is_empty = children.is_empty();
        if is_empty && format.contains(ListFormat::OPTIONAL_IF_EMPTY) {
            if let Some(on_before_emit_node_list) =
                self.print_handlers.on_before_emit_node_list.as_mut()
            {
                on_before_emit_node_list(None);
            }
            if let Some(on_after_emit_node_list) =
                self.print_handlers.on_after_emit_node_list.as_mut()
            {
                on_after_emit_node_list(None);
            }
            return;
        }

        if format.intersects(ListFormat::BRACKETS_MASK) {
            writer.write_punctuation(format.opening_bracket());
            if is_empty && !children_missing {
                self.emit_trailing_comments_to_writer(
                    children_text_range.pos(),
                    CommentSeparator::Before,
                    writer,
                );
            }
        }

        if let Some(on_before_emit_node_list) =
            self.print_handlers.on_before_emit_node_list.as_mut()
        {
            on_before_emit_node_list(None);
        }

        if is_empty {
            if format.contains(ListFormat::MULTI_LINE)
                && !(self.options.preserve_source_newlines
                    && self
                        .current_source_file
                        .as_ref()
                        .is_some_and(|source_file| {
                            range_is_on_single_line(self.loc(parent_node), source_file)
                        }))
            {
                writer.write_line();
            } else if format.contains(ListFormat::SPACE_BETWEEN_BRACES)
                && !format.contains(ListFormat::NO_SPACE_IF_EMPTY)
            {
                writer.write_space(" ");
            }
        } else {
            self.emit_list_items_with(
                parent_node,
                children,
                format,
                has_trailing_comma,
                children_text_range,
                writer,
                &mut emit_child,
            );
        }

        if let Some(on_after_emit_node_list) = self.print_handlers.on_after_emit_node_list.as_mut()
        {
            on_after_emit_node_list(None);
        }

        if format.intersects(ListFormat::BRACKETS_MASK) {
            if is_empty && !children_missing {
                self.emit_leading_comments_to_writer(children_text_range.end(), false, writer);
            }
            writer.write_punctuation(format.closing_bracket());
        }
    }

    fn emit_list_items(
        &mut self,
        parent_node: &ast::Node,
        children: &[ast::Node],
        format: ListFormat,
        has_trailing_comma: bool,
        children_text_range: core::TextRange,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_list_items_with(
            parent_node,
            children,
            format,
            has_trailing_comma,
            children_text_range,
            writer,
            |printer, child, writer| printer.write_node_worker(Some(child), writer),
        );
    }

    fn emit_list_items_with<F>(
        &mut self,
        parent_node: &ast::Node,
        children: &[ast::Node],
        format: ListFormat,
        has_trailing_comma: bool,
        children_text_range: core::TextRange,
        writer: &mut dyn EmitTextWriter,
        mut emit_child: F,
    ) where
        F: FnMut(&mut Self, &ast::Node, &mut dyn EmitTextWriter),
    {
        if children.is_empty() {
            if format.contains(ListFormat::MULTI_LINE) {
                writer.write_line();
            } else if format.contains(ListFormat::SPACE_BETWEEN_BRACES)
                && !format.contains(ListFormat::NO_SPACE_IF_EMPTY)
            {
                writer.write_space(" ");
            }
            return;
        }

        let may_emit_intervening_comments = !format.contains(ListFormat::NO_INTERVENING_COMMENTS);
        let mut should_emit_intervening_comments = may_emit_intervening_comments;

        let leading_line_terminator_count = children.first().map_or(0, |first_child| {
            self.get_leading_line_terminator_count(parent_node, Some(first_child), format)
        });
        if leading_line_terminator_count > 0 {
            for _ in 0..leading_line_terminator_count {
                writer.write_line();
            }
            should_emit_intervening_comments = false;
        } else if format.contains(ListFormat::SPACE_BETWEEN_BRACES) {
            writer.write_space(" ");
        }

        if format.contains(ListFormat::INDENTED) {
            writer.increase_indent();
        }

        let parent_end = self.loc(parent_node).end();
        let mut previous_sibling: Option<ast::Node> = None;
        let mut should_decrease_indent_after_emit = false;

        for child in children {
            if format.contains(ListFormat::ASTERISK_DELIMITED) {
                writer.write_line();
                self.write_delimiter(format, writer);
            } else if let Some(previous) = previous_sibling.as_ref() {
                if format.intersects(ListFormat::DELIMITERS_MASK)
                    && self.loc(previous).end() != parent_end
                    && !self.comments_disabled
                    && self.should_emit_trailing_comments(previous)
                {
                    self.emit_leading_comments_to_writer(self.loc(previous).end(), false, writer);
                }

                self.write_delimiter(format, writer);

                let separating_line_terminator_count =
                    self.get_separating_line_terminator_count(Some(previous), Some(child), format);
                if separating_line_terminator_count > 0 {
                    if format.mask_eq(
                        ListFormat::LINES_MASK | ListFormat::INDENTED,
                        ListFormat::SINGLE_LINE,
                    ) {
                        writer.increase_indent();
                        should_decrease_indent_after_emit = true;
                    }

                    if should_emit_intervening_comments
                        && format.intersects(ListFormat::DELIMITERS_MASK)
                        && !ast::position_is_synthesized(self.loc(child).pos())
                        && self.should_emit_leading_comments(child)
                    {
                        let comment_range = self.emit_context.comment_range(child);
                        self.emit_trailing_comments_of_position_to_writer(
                            comment_range.pos(),
                            format.contains(ListFormat::SPACE_BETWEEN_SIBLINGS),
                            true,
                            writer,
                        );
                    }

                    for _ in 0..separating_line_terminator_count {
                        writer.write_line();
                    }
                    should_emit_intervening_comments = false;
                } else if format.contains(ListFormat::SPACE_BETWEEN_SIBLINGS) {
                    writer.write_space(" ");
                }
            }

            if should_emit_intervening_comments && self.should_emit_leading_comments(child) {
                let comment_range = self.emit_context.comment_range(child);
                self.emit_trailing_comments_of_position_to_writer(
                    comment_range.pos(),
                    false,
                    false,
                    writer,
                );
            } else {
                should_emit_intervening_comments = may_emit_intervening_comments;
            }

            self.next_list_element_pos = self.loc(child).pos();
            emit_child(self, child, writer);

            if should_decrease_indent_after_emit {
                writer.decrease_indent();
                should_decrease_indent_after_emit = false;
            }

            previous_sibling = Some(*child);
        }

        let skip_trailing_comments = previous_sibling.as_ref().is_none_or(|previous| {
            self.comments_disabled || !self.should_emit_trailing_comments(previous)
        });
        let emit_trailing_comma = has_trailing_comma
            && format.contains(ListFormat::ALLOW_TRAILING_COMMA)
            && format.contains(ListFormat::COMMA_DELIMITED);
        if emit_trailing_comma {
            if let Some(previous) = previous_sibling.as_ref() {
                if !skip_trailing_comments {
                    self.emit_token_with_comment_to_writer(
                        ast::Kind::CommaToken,
                        self.loc(previous).end(),
                        WriteKind::Punctuation,
                        previous,
                        writer,
                    );
                } else {
                    writer.write_punctuation(",");
                }
            } else {
                writer.write_punctuation(",");
            }
        }

        if let Some(previous) = previous_sibling.as_ref() {
            if parent_end != self.loc(previous).end()
                && format.intersects(ListFormat::DELIMITERS_MASK)
                && !skip_trailing_comments
            {
                let comments_pos = if emit_trailing_comma && children_text_range.end() > 0 {
                    children_text_range.end()
                } else {
                    self.loc(previous).end()
                };
                self.emit_leading_comments_to_writer(comments_pos, false, writer);
            }
        }

        if format.contains(ListFormat::INDENTED) {
            writer.decrease_indent();
        }

        let closing_line_terminator_count = self.get_closing_line_terminator_count(
            parent_node,
            children.last(),
            format,
            children_text_range,
        );
        if closing_line_terminator_count > 0 {
            for _ in 0..closing_line_terminator_count {
                writer.write_line();
            }
        } else if format.intersects(ListFormat::SPACE_AFTER_LIST | ListFormat::SPACE_BETWEEN_BRACES)
        {
            writer.write_space(" ");
        }
    }

    fn write_delimiter(&mut self, format: ListFormat, writer: &mut dyn EmitTextWriter) {
        match format & ListFormat::DELIMITERS_MASK {
            ListFormat::NONE => {}
            ListFormat::COMMA_DELIMITED => writer.write_punctuation(","),
            ListFormat::BAR_DELIMITED => {
                writer.write_space(" ");
                writer.write_punctuation("|");
            }
            ListFormat::ASTERISK_DELIMITED => {
                writer.write_space(" ");
                writer.write_punctuation("*");
                writer.write_space(" ");
            }
            ListFormat::AMPERSAND_DELIMITED => {
                writer.write_space(" ");
                writer.write_punctuation("&");
            }
            _ => panic!("unexpected list delimiter format: {format:?}"),
        }
    }

    fn get_leading_line_terminator_count(
        &mut self,
        parent_node: &ast::Node,
        first_child: Option<&ast::Node>,
        format: ListFormat,
    ) -> i32 {
        if format.contains(ListFormat::PRESERVE_LINES) || self.options.preserve_source_newlines {
            if format.contains(ListFormat::PREFER_NEW_LINE) {
                return 1;
            }

            if first_child.is_none() {
                return if self
                    .current_source_file
                    .as_ref()
                    .is_some_and(|source_file| {
                        range_is_on_single_line(self.loc(parent_node), source_file)
                    }) {
                    0
                } else {
                    1
                };
            }
            if self.next_list_element_pos > 0
                && self.loc(first_child.unwrap()).pos() == self.next_list_element_pos
            {
                // If this child starts at the beginning of a list item in a parent list, its leading
                // line terminators have already been written as the separating line terminators of the
                // parent list. Example:
                //
                // class Foo {
                //   constructor() {}
                //   public foo() {}
                // }
                //
                // The outer list is the list of class members, with one line terminator between the
                // constructor and the method. The constructor is written, the separating line terminator
                // is written, and then we start emitting the method. Its modifiers ([public]) constitute an inner
                // list, so we look for its leading line terminators. If we didn't know that we had already
                // written a newline as part of the parent list, it would appear that we need to write a
                // leading newline to start the modifiers.
                return 0;
            }
            let first_child = first_child.unwrap();
            if self.kind(first_child) == ast::Kind::JsxText {
                // JsxText will be written with its leading whitespace, so don't add more manually.
                return 0;
            }
            if let Some(current_source_file) = self.current_source_file.as_ref() {
                let parent_loc = self.loc(parent_node);
                let first_child_loc = self.loc(first_child);
                if !ast::position_is_synthesized(parent_loc.pos())
                    && !ast::node_is_synthesized(self.store_for_node(first_child), *first_child)
                    && self.parent_is_absent_or_matches(first_child, parent_node)
                {
                    if self.options.preserve_source_newlines {
                        return self.get_effective_lines(|include_comments| {
                            get_lines_between_position_and_preceding_non_whitespace_character(
                                first_child_loc.pos(),
                                parent_loc.pos(),
                                current_source_file,
                                include_comments,
                            )
                        });
                    }
                    return if range_start_positions_are_on_same_line(
                        parent_loc,
                        first_child_loc,
                        current_source_file,
                    ) {
                        0
                    } else {
                        1
                    };
                }
            }
            if self.should_emit_on_new_line(first_child, format) {
                return 1;
            }
        }
        if format.contains(ListFormat::MULTI_LINE) {
            1
        } else {
            0
        }
    }

    fn get_separating_line_terminator_count(
        &mut self,
        previous_node: Option<&ast::Node>,
        next_node: Option<&ast::Node>,
        format: ListFormat,
    ) -> i32 {
        if format.contains(ListFormat::PRESERVE_LINES) || self.options.preserve_source_newlines {
            let (Some(previous_node), Some(next_node)) = (previous_node, next_node) else {
                return 0;
            };
            if self.kind(next_node) == ast::Kind::JsxText {
                // JsxText will be written with its leading whitespace, so don't add more manually.
                return 0;
            } else if format.contains(ListFormat::PREFER_NEW_LINE)
                && (ast::node_is_synthesized(self.store_for_node(previous_node), *previous_node)
                    || ast::node_is_synthesized(self.store_for_node(next_node), *next_node))
            {
                return 1;
            } else if let Some(current_source_file) = self.current_source_file.as_ref() {
                if !ast::node_is_synthesized(self.store_for_node(previous_node), *previous_node)
                    && !ast::node_is_synthesized(self.store_for_node(next_node), *next_node)
                {
                    if self.options.preserve_source_newlines
                        && self.sibling_node_positions_are_comparable(previous_node, next_node)
                    {
                        let previous_loc = self.loc(previous_node);
                        let next_loc = self.loc(next_node);
                        return self.get_effective_lines(|include_comments| {
                            get_lines_between_range_end_and_range_start(
                                previous_loc,
                                next_loc,
                                current_source_file,
                                include_comments,
                            )
                        });
                    } else if !self.options.preserve_source_newlines
                        && self.nodes_have_same_parent(previous_node, next_node)
                    {
                        let previous_loc = self.loc(previous_node);
                        let next_loc = self.loc(next_node);
                        // If `preserveSourceNewlines` is `false`, preserve at most one line terminator
                        // for sibling nodes that were on separate source lines.
                        return if range_end_is_on_same_line_as_range_start(
                            previous_loc,
                            next_loc,
                            current_source_file,
                        ) {
                            0
                        } else {
                            1
                        };
                    }
                    // If the two nodes are not comparable, add a line terminator based on the format.
                    return if format.contains(ListFormat::PREFER_NEW_LINE) {
                        1
                    } else {
                        0
                    };
                }
            } else if self.should_emit_on_new_line(previous_node, format)
                || self.should_emit_on_new_line(next_node, format)
            {
                return 1;
            }
        } else if let Some(next_node) = next_node {
            if self.should_emit_on_new_line(next_node, ListFormat::NONE) {
                return 1;
            }
        }
        if format.contains(ListFormat::MULTI_LINE) {
            1
        } else {
            0
        }
    }

    fn get_closing_line_terminator_count(
        &mut self,
        parent_node: &ast::Node,
        last_child: Option<&ast::Node>,
        format: ListFormat,
        children_text_range: core::TextRange,
    ) -> i32 {
        if format.contains(ListFormat::PRESERVE_LINES) || self.options.preserve_source_newlines {
            if format.contains(ListFormat::PREFER_NEW_LINE) {
                return 1;
            }
            let Some(last_child) = last_child else {
                return if self
                    .current_source_file
                    .as_ref()
                    .is_some_and(|source_file| {
                        range_is_on_single_line(self.loc(parent_node), source_file)
                    }) {
                    0
                } else {
                    1
                };
            };
            if let Some(current_source_file) = self.current_source_file.as_ref() {
                let parent_loc = self.loc(parent_node);
                let last_child_loc = self.loc(last_child);
                if !ast::position_is_synthesized(parent_loc.pos())
                    && !ast::node_is_synthesized(self.store_for_node(last_child), *last_child)
                    && self.parent_is_absent_or_matches(last_child, parent_node)
                {
                    if self.options.preserve_source_newlines {
                        let end = greatest_end(last_child_loc.end(), &[children_text_range]);
                        return self.get_effective_lines(|include_comments| {
                            get_lines_between_position_and_next_non_whitespace_character(
                                end,
                                parent_loc.end(),
                                current_source_file,
                                include_comments,
                            )
                        });
                    }
                    return if range_end_positions_are_on_same_line(
                        parent_loc,
                        last_child_loc,
                        current_source_file,
                    ) {
                        0
                    } else {
                        1
                    };
                }
            }
            if self.should_emit_on_new_line(last_child, format) {
                return 1;
            }
        }
        if format.contains(ListFormat::MULTI_LINE)
            && !format.contains(ListFormat::NO_TRAILING_NEW_LINE)
        {
            return 1;
        }
        0
    }

    pub(crate) fn write_comment_range(&mut self, comment: ast::CommentRange) {
        let Some(current_source_file) = self.current_source_file.as_ref() else {
            return;
        };

        let text = current_source_file.text().to_string();
        let line_map = current_source_file.ecma_line_map();
        self.write_comment_range_worker(&text, &line_map, comment.kind, comment.text_range);
    }

    pub(crate) fn write_comment_range_worker(
        &mut self,
        text: &str,
        line_map: &[core::TextPos],
        kind: ast::Kind,
        loc: core::TextRange,
    ) {
        if kind == ast::Kind::MultiLineCommentTrivia {
            let indent_size = get_default_indent_size();
            let first_line = scanner::compute_line_of_position(line_map, loc.pos() as usize);
            let line_count = line_map.len();
            let mut first_comment_line_indent = -1;
            let mut pos = loc.pos();
            let mut current_line = first_line;
            while pos < loc.end() {
                let next_line_start = if current_line + 1 == line_count {
                    text.len() as i32 + 1
                } else {
                    line_map[current_line + 1]
                };

                if pos != loc.pos() {
                    if first_comment_line_indent == -1 {
                        first_comment_line_indent = calculate_indent(
                            text,
                            line_map[first_line] as usize,
                            loc.pos() as usize,
                        );
                    }

                    let current_writer_indent_spacing =
                        self.writer.as_ref().unwrap().borrow().get_indent() * indent_size;
                    let spaces_to_emit = current_writer_indent_spacing - first_comment_line_indent
                        + calculate_indent(text, pos as usize, next_line_start as usize);
                    if spaces_to_emit > 0 {
                        let mut number_of_single_spaces_to_emit = spaces_to_emit % indent_size;
                        let indent_size_space_string = get_indent_string(
                            (spaces_to_emit - number_of_single_spaces_to_emit) / indent_size,
                            indent_size,
                        );
                        self.writer
                            .as_ref()
                            .unwrap()
                            .borrow_mut()
                            .raw_write(&indent_size_space_string);
                        while number_of_single_spaces_to_emit > 0 {
                            self.writer.as_ref().unwrap().borrow_mut().raw_write(" ");
                            number_of_single_spaces_to_emit -= 1;
                        }
                    } else {
                        self.writer.as_ref().unwrap().borrow_mut().raw_write("");
                    }
                }

                let end = loc.end().min(next_line_start - 1);
                let current_line_text = text[pos as usize..end as usize].trim();
                if !current_line_text.is_empty() {
                    self.write_comment(current_line_text);
                    if end != loc.end() {
                        self.write_line();
                    }
                } else {
                    self.writer
                        .as_ref()
                        .unwrap()
                        .borrow_mut()
                        .write_line_force(true);
                }

                pos = next_line_start;
                current_line += 1;
            }
        } else {
            self.write_comment(&text[loc.pos() as usize..loc.end() as usize]);
        }
    }

    pub(crate) fn write_comment_range_worker_to_writer(
        &mut self,
        text: &str,
        line_map: &[core::TextPos],
        kind: ast::Kind,
        loc: core::TextRange,
        writer: &mut dyn EmitTextWriter,
    ) {
        if kind == ast::Kind::MultiLineCommentTrivia {
            let indent_size = get_default_indent_size();
            let first_line = scanner::compute_line_of_position(line_map, loc.pos() as usize);
            let line_count = line_map.len();
            let mut first_comment_line_indent = -1;
            let mut pos = loc.pos();
            let mut current_line = first_line;
            while pos < loc.end() {
                let next_line_start = if current_line + 1 == line_count {
                    text.len() as i32 + 1
                } else {
                    line_map[current_line + 1]
                };

                if pos != loc.pos() {
                    if first_comment_line_indent == -1 {
                        first_comment_line_indent = calculate_indent(
                            text,
                            line_map[first_line] as usize,
                            loc.pos() as usize,
                        );
                    }

                    let current_writer_indent_spacing = writer.get_indent() * indent_size;
                    let spaces_to_emit = current_writer_indent_spacing - first_comment_line_indent
                        + calculate_indent(text, pos as usize, next_line_start as usize);
                    if spaces_to_emit > 0 {
                        let mut number_of_single_spaces_to_emit = spaces_to_emit % indent_size;
                        let indent_size_space_string = get_indent_string(
                            (spaces_to_emit - number_of_single_spaces_to_emit) / indent_size,
                            indent_size,
                        );
                        writer.raw_write(&indent_size_space_string);
                        while number_of_single_spaces_to_emit > 0 {
                            writer.raw_write(" ");
                            number_of_single_spaces_to_emit -= 1;
                        }
                    } else {
                        writer.raw_write("");
                    }
                }

                let end = loc.end().min(next_line_start - 1);
                let current_line_text = text[pos as usize..end as usize].trim();
                if !current_line_text.is_empty() {
                    writer.write_comment(current_line_text);
                    if end != loc.end() {
                        writer.write_line();
                    }
                } else {
                    writer.write_line_force(true);
                }

                pos = next_line_start;
                current_line += 1;
            }
        } else {
            writer.write_comment(&text[loc.pos() as usize..loc.end() as usize]);
        }
    }

    pub(crate) fn should_emit_comments(&self, node: &ast::Node) -> bool {
        !self.comments_disabled
            && self.current_source_file.is_some()
            && !ast::is_source_file(self.store_for_node(node), *node)
    }

    pub(crate) fn should_write_comment(&self, comment: ast::CommentRange) -> bool {
        !self.options.only_print_jsdoc_style
            || self
                .current_source_file
                .as_ref()
                .is_some_and(|source_file| {
                    is_jsdoc_like_text(source_file.text(), comment)
                        || is_pinned_comment(source_file.text(), comment)
                })
    }

    pub(crate) fn should_emit_indented(&mut self, node: &ast::Node) -> bool {
        self.emit_context.emit_flags(node) & EF_INDENTED != 0
    }

    fn should_emit_on_single_line(&mut self, parent_node: &ast::Node) -> bool {
        self.emit_context.emit_flags(parent_node) & EF_SINGLE_LINE != 0
    }

    fn should_elide_indentation(&mut self, parent_node: &ast::Node) -> bool {
        self.emit_context.emit_flags(parent_node) & EF_NO_INDENTATION != 0
    }

    pub(crate) fn should_emit_on_multiple_lines(&mut self, node: &ast::Node) -> bool {
        self.emit_context.emit_flags(node) & EF_MULTI_LINE != 0
    }

    pub(crate) fn should_emit_block_function_body_on_single_line(
        &mut self,
        body: &ast::Node,
    ) -> bool {
        if self.should_emit_on_single_line(body) {
            return true;
        }

        let store = self.store_for_node(body);
        if ast::is_block(store, *body) && store.multi_line(*body).unwrap_or(false) {
            return false;
        }

        if !ast::node_is_synthesized(store, *body)
            && self
                .current_source_file
                .as_ref()
                .is_some_and(|source_file| !range_is_on_single_line(store.loc(*body), source_file))
        {
            return false;
        }

        let statements = store
            .source_statements(*body)
            .expect("block-like node should have statements");
        let first_statement = statements.first();
        let last_statement = statements.last();
        let statements_loc = statements.loc();
        let statement_nodes: Vec<_> = statements.into_iter().collect();
        if self.get_leading_line_terminator_count(
            body,
            first_statement.as_ref(),
            ListFormat::PRESERVE_LINES,
        ) > 0
            || self.get_closing_line_terminator_count(
                body,
                last_statement.as_ref(),
                ListFormat::PRESERVE_LINES,
                statements_loc,
            ) > 0
        {
            return false;
        }

        let mut previous_statement: Option<ast::Node> = None;
        for statement in statement_nodes {
            if self.get_separating_line_terminator_count(
                previous_statement.as_ref(),
                Some(&statement),
                ListFormat::PRESERVE_LINES,
            ) > 0
            {
                return false;
            }
            previous_statement = Some(statement);
        }

        true
    }

    fn is_empty_block(&self, block: &ast::Node, statements: &[ast::Node]) -> bool {
        statements.is_empty()
            && self.current_source_file.as_ref().is_none_or(|source_file| {
                range_end_is_on_same_line_as_range_start(
                    self.loc(block),
                    self.loc(block),
                    source_file,
                )
            })
    }

    fn should_emit_on_new_line(&mut self, node: &ast::Node, format: ListFormat) -> bool {
        self.emit_context.emit_flags(node) & EF_START_ON_NEW_LINE != 0
            || format.contains(ListFormat::PREFER_NEW_LINE)
    }

    pub(crate) fn should_emit_source_maps(&mut self, node: &ast::Node) -> bool {
        !self.source_maps_disabled
            && self.source_map_source.is_some()
            && !ast::is_source_file(self.store_for_node(node), *node)
            && !ast::is_in_json_file(self.store_for_node(node), *node)
    }

    pub(crate) fn should_emit_token_source_maps(
        &mut self,
        token: ast::Kind,
        context_node: &ast::Node,
        flags: TokenEmitFlags,
    ) -> bool {
        !flags.contains(TokenEmitFlags::NO_SOURCE_MAPS)
            && self.should_emit_source_maps(context_node)
            && !self.options.omit_brace_source_map_positions
            && (token == ast::Kind::OpenBraceToken || token == ast::Kind::CloseBraceToken)
    }

    pub(crate) fn should_emit_leading_comments(&mut self, node: &ast::Node) -> bool {
        self.emit_context.emit_flags(node) & EF_NO_LEADING_COMMENTS == 0
    }

    pub(crate) fn should_emit_trailing_comments(&mut self, node: &ast::Node) -> bool {
        self.emit_context.emit_flags(node) & EF_NO_TRAILING_COMMENTS == 0
    }

    pub(crate) fn should_emit_nested_comments(&mut self, node: &ast::Node) -> bool {
        self.emit_context.emit_flags(node) & EF_NO_NESTED_COMMENTS == 0
    }

    pub(crate) fn should_emit_detached_comments(&self, node: &ast::Node) -> bool {
        if !ast::is_source_file(self.store_for_node(node), *node) {
            return true;
        }

        let statements: Vec<_> = self
            .store_for_node(node)
            .parser_access()
            .source_file_statement_list(*node)
            .into_iter()
            .collect();
        statements.is_empty()
            || !ast::is_prologue_directive(self.store_for_node(node), statements[0])
            || ast::node_is_synthesized(self.store_for_node(node), statements[0])
    }

    pub(crate) fn has_comments_at_position(&self, pos: i32) -> bool {
        let Some(current_source_file) = self.current_source_file.as_ref() else {
            return false;
        };
        let text = current_source_file.text();
        !scanner::get_trailing_comment_ranges(text, pos + 1).is_empty()
            || !scanner::get_leading_comment_ranges(text, pos + 1).is_empty()
    }

    fn emit_comments_before_node(&mut self, node: &ast::Node) -> Option<CommentState> {
        if !self.should_emit_comments(node) {
            return None;
        }

        let emit_flags = self.emit_context.emit_flags(node);
        let comment_range = self.emit_context.comment_range(node);
        let container_pos = self.container_pos;
        let container_end = self.container_end;
        let declaration_list_container_end = self.declaration_list_container_end;

        self.emit_leading_comments_of_node(node, emit_flags, comment_range);
        self.emit_leading_synthetic_comments_of_node(node, emit_flags);
        if emit_flags & EF_NO_NESTED_COMMENTS != 0 {
            self.comments_disabled = true;
        }

        let state = CommentState {
            emit_flags,
            comment_range,
            container_pos,
            container_end,
            declaration_list_container_end,
        };
        *self.comment_state_arena.new_item() = state.clone();
        Some(state)
    }

    fn emit_comments_after_node(&mut self, node: &ast::Node, state: Option<CommentState>) {
        let Some(state) = state else {
            return;
        };

        if state.emit_flags & EF_NO_NESTED_COMMENTS != 0 {
            self.comments_disabled = false;
        }

        self.emit_trailing_synthetic_comments_of_node(node, state.emit_flags);
        self.emit_trailing_comments_of_node(
            node,
            state.emit_flags,
            state.comment_range,
            state.container_pos,
            state.container_end,
            state.declaration_list_container_end,
        );

        if let Some(type_node) = self.emit_context.get_type_node(node) {
            self.emit_trailing_comments_of_node(
                node,
                state.emit_flags,
                self.loc(&type_node),
                state.container_pos,
                state.container_end,
                state.declaration_list_container_end,
            );
        }
    }

    fn emit_comments_before_token(
        &mut self,
        _token: ast::Kind,
        mut pos: i32,
        context_node: &ast::Node,
        flags: TokenEmitFlags,
    ) -> (Option<CommentState>, i32) {
        if flags.contains(TokenEmitFlags::NO_COMMENTS) || self.comments_disabled {
            if let Some(current_source_file) = self.current_source_file.as_ref()
                && !ast::position_is_synthesized(pos)
            {
                pos = scanner::skip_trivia(current_source_file.text(), pos.max(0) as usize) as i32;
            }
            return (None, pos);
        }

        let start_pos = pos;
        if let Some(current_source_file) = self.current_source_file.as_ref() {
            pos =
                scanner::skip_trivia(current_source_file.text(), start_pos.max(0) as usize) as i32;
        }

        let node = self.emit_context.parse_node(context_node);
        let is_similar_node = node
            .as_ref()
            .is_some_and(|node| self.kind(node) == self.kind(context_node));
        if !is_similar_node {
            return (None, pos);
        }

        if self.loc(context_node).pos() != start_pos {
            let indent_leading = flags.contains(TokenEmitFlags::INDENT_LEADING_COMMENTS);
            let needs_indent = indent_leading
                && self
                    .current_source_file
                    .as_ref()
                    .is_some_and(|source_file| {
                        !positions_are_on_same_line(start_pos, pos, source_file)
                    });
            self.increase_indent_if(needs_indent);
            self.emit_leading_comments(start_pos, false);
            self.decrease_indent_if(needs_indent);
        }

        let state = CommentState::default();
        *self.comment_state_arena.new_item() = state.clone();
        (Some(state), pos)
    }

    fn emit_comments_after_token(
        &mut self,
        _token: ast::Kind,
        pos: i32,
        context_node: &ast::Node,
        state: Option<CommentState>,
    ) {
        if state.is_none() {
            return;
        }
        if self.loc(context_node).end() != pos {
            let separator = if self.kind(context_node) == ast::Kind::JsxExpression {
                CommentSeparator::None
            } else {
                CommentSeparator::Before
            };
            self.emit_trailing_comments(pos, separator);
        }
    }

    fn emit_leading_comments_of_node(
        &mut self,
        node: &ast::Node,
        emit_flags: EmitFlags,
        comment_range: core::TextRange,
    ) {
        let pos = comment_range.pos();
        let end = comment_range.end();

        if (!ast::position_is_synthesized(pos) || !ast::position_is_synthesized(end)) && pos != end
        {
            let skip_leading_comments = ast::position_is_synthesized(pos)
                || emit_flags & EF_NO_LEADING_COMMENTS != 0
                || self.kind(node) == ast::Kind::JsxText;
            let skip_trailing_comments = ast::position_is_synthesized(end)
                || emit_flags & EF_NO_TRAILING_COMMENTS != 0
                || self.kind(node) == ast::Kind::JsxText;

            if !skip_leading_comments {
                self.emit_leading_comments(pos, self.kind(node) == ast::Kind::NotEmittedStatement);
            }

            if !skip_leading_comments || (pos >= 0 && emit_flags & EF_NO_LEADING_COMMENTS != 0) {
                self.container_pos = pos;
            }

            if !skip_trailing_comments || (end >= 0 && emit_flags & EF_NO_TRAILING_COMMENTS != 0) {
                self.container_end = end;
                if self.kind(node) == ast::Kind::VariableDeclarationList {
                    self.declaration_list_container_end = end;
                }
            }
        }
    }

    fn emit_trailing_comments_of_node(
        &mut self,
        node: &ast::Node,
        emit_flags: EmitFlags,
        comment_range: core::TextRange,
        container_pos: i32,
        container_end: i32,
        declaration_list_container_end: i32,
    ) {
        let pos = comment_range.pos();
        let end = comment_range.end();
        let skip_trailing_comments = end < 0
            || emit_flags & EF_NO_TRAILING_COMMENTS != 0
            || self.kind(node) == ast::Kind::JsxText;
        if (!ast::position_is_synthesized(pos) || !ast::position_is_synthesized(end)) && pos != end
        {
            self.container_pos = container_pos;
            self.container_end = container_end;
            self.declaration_list_container_end = declaration_list_container_end;

            if !skip_trailing_comments && self.kind(node) != ast::Kind::NotEmittedStatement {
                self.emit_trailing_comments(end, CommentSeparator::Before);
            }
        }
    }

    fn emit_leading_synthetic_comments_of_node(&mut self, node: &ast::Node, emit_flags: EmitFlags) {
        if emit_flags & EF_NO_LEADING_COMMENTS != 0 {
            return;
        }
        for comment in self.emit_context.get_synthetic_leading_comments(node) {
            self.emit_leading_synthesized_comment(comment);
        }
    }

    fn emit_leading_synthesized_comment(
        &mut self,
        comment: crate::emitcontext::SynthesizedComment,
    ) {
        if comment.has_leading_new_line || comment.kind == ast::Kind::SingleLineCommentTrivia {
            self.write_line();
        }
        self.write_synthesized_comment(comment.clone());
        if comment.has_trailing_new_line || comment.kind == ast::Kind::SingleLineCommentTrivia {
            self.write_line();
        } else {
            self.write_space();
        }
    }

    fn emit_trailing_synthetic_comments_of_node(
        &mut self,
        node: &ast::Node,
        emit_flags: EmitFlags,
    ) {
        if emit_flags & EF_NO_TRAILING_COMMENTS != 0 {
            return;
        }
        for comment in self.emit_context.get_synthetic_trailing_comments(node) {
            self.emit_trailing_synthesized_comment(comment);
        }
    }

    fn emit_trailing_synthesized_comment(
        &mut self,
        comment: crate::emitcontext::SynthesizedComment,
    ) {
        if !self.writer.as_ref().unwrap().borrow().is_at_start_of_line() {
            self.write_space();
        }
        self.write_synthesized_comment(comment.clone());
        if comment.has_trailing_new_line {
            self.write_line();
        }
    }

    fn write_synthesized_comment(&mut self, comment: crate::emitcontext::SynthesizedComment) {
        let text = if comment.kind == ast::Kind::MultiLineCommentTrivia {
            format!("/*{}*/", comment.text)
        } else {
            format!("//{}", comment.text)
        };
        let line_map = if comment.kind == ast::Kind::MultiLineCommentTrivia {
            core::compute_ecma_line_starts(&text)
        } else {
            Vec::new()
        };
        self.write_comment_range_worker(
            &text,
            &line_map,
            comment.kind,
            core::new_text_range(0, text.len() as i32),
        );
    }

    fn emit_leading_comments(&mut self, mut pos: i32, elided: bool) -> bool {
        if self.comments_disabled
            || self.current_source_file.is_none()
            || ast::position_is_synthesized(pos)
            || pos == self.container_pos
        {
            return false;
        }

        let mut triple_slash = core::TSUnknown;
        if !elided {
            if pos == 0
                && self
                    .current_source_file
                    .as_ref()
                    .is_some_and(|source_file| source_file.data().is_declaration_file())
            {
                triple_slash = core::TSFalse;
            }
        } else if pos == 0 {
            triple_slash = core::TSTrue;
        } else {
            return false;
        }

        if self.detached_comments_info.len() > 0 {
            let info = self.detached_comments_info.peek();
            if info.node_pos == pos {
                pos = self.detached_comments_info.pop().detached_comment_end_pos;
            }
        }

        let source_file = self.current_source_file.as_ref().unwrap();
        let mut comments = Vec::new();
        for comment in scanner::get_leading_comment_ranges(source_file.text(), pos) {
            if self.should_write_comment(comment)
                && self.should_emit_comment_if_triple_slash(comment, triple_slash)
            {
                comments.push(comment);
            }
        }

        if !comments.is_empty()
            && self.should_emit_new_line_before_leading_comment_of_position(pos, comments[0].pos())
        {
            self.write_line();
        }

        self.emit_comments(comments, CommentSeparator::After)
    }

    fn should_emit_comment_if_triple_slash(
        &self,
        comment: ast::CommentRange,
        triple_slash: core::Tristate,
    ) -> bool {
        if triple_slash == core::TSTrue {
            self.is_triple_slash_comment(comment)
        } else if triple_slash == core::TSFalse {
            !self.is_triple_slash_comment(comment)
        } else {
            true
        }
    }

    fn should_emit_new_line_before_leading_comment_of_position(
        &self,
        pos: i32,
        comment_pos: i32,
    ) -> bool {
        self.current_source_file
            .as_ref()
            .is_some_and(|source_file| {
                pos != comment_pos
                    && scanner::compute_line_of_position(&source_file.ecma_line_map(), pos)
                        != scanner::compute_line_of_position(
                            &source_file.ecma_line_map(),
                            comment_pos,
                        )
            })
    }

    fn emit_trailing_comments(&mut self, pos: i32, comment_separator: CommentSeparator) {
        if self.comments_disabled
            || self.current_source_file.is_none()
            || (self.container_end != -1
                && (pos == self.container_end || pos == self.declaration_list_container_end))
        {
            return;
        }

        let source_file = self.current_source_file.as_ref().unwrap();
        let comments = scanner::get_trailing_comment_ranges(source_file.text(), pos)
            .into_iter()
            .filter(|comment| self.should_write_comment(*comment))
            .collect();
        self.emit_comments(comments, comment_separator);
    }

    fn emit_leading_comments_of_position(&mut self, pos: i32) {
        if self.comments_disabled || pos == -1 {
            return;
        }
        self.emit_leading_comments(pos, false);
    }

    fn emit_trailing_comments_of_position(
        &mut self,
        pos: i32,
        prefix_space: bool,
        force_no_newline: bool,
    ) {
        if self.comments_disabled
            || self.current_source_file.is_none()
            || (self.container_end != -1
                && (pos == self.container_end || pos == self.declaration_list_container_end))
        {
            return;
        }

        let source_file = self.current_source_file.as_ref().unwrap();
        let comments = scanner::get_trailing_comment_ranges(source_file.text(), pos);
        if comments.is_empty() {
            return;
        }

        for comment in comments {
            if prefix_space {
                if !self.should_write_comment(comment) {
                    continue;
                }
                if !self.writer.as_ref().unwrap().borrow().is_at_start_of_line() {
                    self.write_space();
                }
                self.emit_comment(comment);
                if comment.has_trailing_new_line {
                    self.write_line();
                }
                continue;
            }

            self.emit_comment(comment);
            if force_no_newline {
                if comment.kind == ast::Kind::SingleLineCommentTrivia {
                    self.write_line();
                }
            } else if comment.has_trailing_new_line {
                self.write_line();
            } else {
                self.write_space();
            }
        }
    }

    fn emit_comments(
        &mut self,
        comments: Vec<ast::CommentRange>,
        comment_separator: CommentSeparator,
    ) -> bool {
        let mut intervening_separator = false;
        if comments.is_empty() {
            return false;
        }
        if comment_separator == CommentSeparator::Before {
            self.write_space();
        }

        for comment in comments {
            if intervening_separator {
                self.write_space();
                intervening_separator = false;
            }

            self.emit_comment(comment);

            if comment.kind == ast::Kind::SingleLineCommentTrivia
                || (comment.has_trailing_new_line && comment_separator != CommentSeparator::None)
            {
                self.write_line();
            } else {
                intervening_separator = comment_separator != CommentSeparator::None;
            }
        }

        if intervening_separator && comment_separator == CommentSeparator::After {
            self.write_space();
        }

        true
    }

    fn emit_comment(&mut self, comment: ast::CommentRange) {
        self.emit_pos(comment.pos());
        self.write_comment_range(comment);
        self.emit_pos(comment.end());
    }

    fn is_triple_slash_comment(&self, comment: ast::CommentRange) -> bool {
        self.current_source_file
            .as_ref()
            .is_some_and(|source_file| {
                is_recognized_triple_slash_comment(source_file.text(), comment)
            })
    }

    fn emit_pos(&mut self, pos: i32) {
        if self.source_maps_disabled
            || self.source_map_source.is_none()
            || self.source_map_generator.is_none()
            || self.source_map_source_is_json
            || ast::position_is_synthesized(pos)
        {
            return;
        }

        let Some(source_index) = self.source_map_source_index else {
            return;
        };
        let Some(line_char_cache) = self.source_map_line_char_cache.as_mut() else {
            return;
        };
        let (source_line, source_character) = line_char_cache.get_line_and_character(pos);
        self.source_map_generator
            .as_mut()
            .unwrap()
            .add_source_mapping(
                self.writer.as_ref().unwrap().borrow().get_line(),
                self.writer.as_ref().unwrap().borrow().get_column(),
                source_index,
                source_line as i32,
                source_character,
            )
            .unwrap_or_else(|err| panic!("{err}"));
    }

    fn set_source_map_source(&mut self, source: SourceMapSource) {
        if self.source_maps_disabled {
            return;
        }
        self.source_map_source = Some(source.clone());
        self.source_map_line_char_cache = Some(new_line_character_cache(&source));
        if let Some(most_recent_source) = self.most_recent_source_map_source.as_ref()
            && source.same_source(most_recent_source)
        {
            self.source_map_source_index = self.most_recent_source_map_source_index;
            return;
        }

        self.source_map_source_is_json =
            tspath::file_extension_is(source.file_name.as_ref(), tspath::EXTENSION_JSON);
        if self.source_map_source_is_json {
            return;
        }
        self.source_map_source_index = Some(
            self.source_map_generator
                .as_mut()
                .unwrap()
                .add_source(source.file_name.to_string()),
        );
        if self.options.inline_sources
            && let Some(source_index) = self.source_map_source_index
        {
            self.source_map_generator
                .as_mut()
                .unwrap()
                .set_source_content(source_index, source.text.to_string())
                .unwrap_or_else(|err| panic!("{err}"));
        }
        self.most_recent_source_map_source = Some(source);
        self.most_recent_source_map_source_index = self.source_map_source_index;
    }

    fn emit_source_pos(&mut self, pos: i32) {
        if self.source_map_line_char_cache.is_none()
            && let Some(source) = self.source_map_source.clone()
        {
            self.set_source_map_source(source);
        }
        self.emit_pos(pos);
    }

    fn emit_source_maps_before_node(&mut self, node: &ast::Node) -> Option<SourceMapState> {
        if !self.should_emit_source_maps(node) {
            return None;
        }

        let emit_flags = self.emit_context.emit_flags(node);
        let loc = self.emit_context.source_map_range(node);

        if !ast::is_not_emitted_statement(self.store_for_node(node), *node)
            && emit_flags & crate::EF_NO_LEADING_SOURCE_MAP != 0
        {
            // handled by the negated check below
        }
        if !ast::is_not_emitted_statement(self.store_for_node(node), *node)
            && emit_flags & crate::EF_NO_LEADING_SOURCE_MAP == 0
            && self.current_source_file.is_some()
            && !ast::position_is_synthesized(loc.pos())
        {
            let source_file = self.current_source_file.as_ref().unwrap();
            let pos = scanner::skip_trivia(source_file.text(), loc.pos().max(0) as usize) as i32;
            self.emit_source_pos(pos);
        }

        if emit_flags & crate::EF_NO_NESTED_SOURCE_MAPS != 0 {
            self.source_maps_disabled = true;
        }

        let state = SourceMapState {
            emit_flags,
            source_map_range: loc,
            has_token_source_map_range: false,
        };
        *self.source_map_state_arena.new_item() = state.clone();
        Some(state)
    }

    fn emit_source_maps_after_node(
        &mut self,
        node: &ast::Node,
        previous_state: Option<SourceMapState>,
    ) {
        let Some(previous_state) = previous_state else {
            return;
        };

        if previous_state.emit_flags & crate::EF_NO_NESTED_SOURCE_MAPS != 0 {
            self.source_maps_disabled = false;
        }

        if !ast::is_not_emitted_statement(self.store_for_node(node), *node)
            && previous_state.emit_flags & crate::EF_NO_TRAILING_SOURCE_MAP == 0
            && !ast::position_is_synthesized(previous_state.source_map_range.end())
        {
            self.emit_source_pos(previous_state.source_map_range.end());
        }
    }

    fn emit_source_maps_before_token(
        &mut self,
        token: ast::Kind,
        mut pos: i32,
        context_node: &ast::Node,
        flags: TokenEmitFlags,
    ) -> Option<SourceMapState> {
        if !self.should_emit_token_source_maps(token, context_node, flags) {
            return None;
        }

        let emit_flags = self.emit_context.emit_flags(context_node);
        let loc = self
            .emit_context
            .token_source_map_range(context_node, token);
        if let Some(loc) = loc {
            pos = loc.pos();
        }
        if pos >= 0
            && let Some(source_file) = self.current_source_file.as_ref()
        {
            pos = scanner::skip_trivia(source_file.text(), pos as usize) as i32;
        }
        if emit_flags & crate::EF_NO_TOKEN_LEADING_SOURCE_MAPS == 0 && pos >= 0 {
            self.emit_source_pos(pos);
        }

        let state = SourceMapState {
            emit_flags,
            source_map_range: loc.unwrap_or_default(),
            has_token_source_map_range: loc.is_some(),
        };
        *self.source_map_state_arena.new_item() = state.clone();
        Some(state)
    }

    fn emit_source_maps_after_token(
        &mut self,
        _token: ast::Kind,
        mut pos: i32,
        _context_node: &ast::Node,
        previous_state: Option<SourceMapState>,
    ) {
        let Some(previous_state) = previous_state else {
            return;
        };
        if previous_state.emit_flags & crate::EF_NO_TOKEN_TRAILING_SOURCE_MAPS == 0 {
            if previous_state.has_token_source_map_range {
                pos = previous_state.source_map_range.end();
            }
            if pos >= 0 {
                self.emit_source_pos(pos);
            }
        }
    }

    pub(crate) fn should_emit_indirect_call(&mut self, node: &ast::Node) -> bool {
        self.emit_context.emit_flags(node) & EF_INDIRECT_CALL != 0
    }

    pub(crate) fn should_allow_trailing_comma(
        &self,
        node: &ast::Node,
        list_position_key: ast::NodeListPositionKey,
    ) -> bool {
        if self
            .current_source_file
            .as_ref()
            .is_none_or(|source_file| source_file.data().script_kind() == core::ScriptKind::JSON)
        {
            return false;
        }

        match self.kind(node) {
            ast::Kind::ObjectLiteralExpression
            | ast::Kind::ArrayLiteralExpression
            | ast::Kind::ArrowFunction
            | ast::Kind::Constructor
            | ast::Kind::GetAccessor
            | ast::Kind::SetAccessor
            | ast::Kind::TypeAliasDeclaration
            | ast::Kind::JSTypeAliasDeclaration
            | ast::Kind::FunctionType
            | ast::Kind::ConstructorType
            | ast::Kind::CallSignature
            | ast::Kind::ConstructSignature
            | ast::Kind::TaggedTemplateExpression
            | ast::Kind::ObjectBindingPattern
            | ast::Kind::ArrayBindingPattern
            | ast::Kind::NamedImports
            | ast::Kind::NamedExports
            | ast::Kind::ImportAttributes
            | ast::Kind::FunctionDeclaration
            | ast::Kind::FunctionExpression
            | ast::Kind::MethodDeclaration
            | ast::Kind::CallExpression
            | ast::Kind::NewExpression => true,
            ast::Kind::ClassExpression
            | ast::Kind::ClassDeclaration
            | ast::Kind::InterfaceDeclaration => {
                let store = self.store_for_node(node);
                match self.kind(node) {
                    ast::Kind::ClassExpression
                    | ast::Kind::ClassDeclaration
                    | ast::Kind::InterfaceDeclaration => store
                        .source_type_parameters(*node)
                        .is_some_and(|type_parameters| {
                            type_parameters.position_key() == list_position_key
                        }),
                    _ => false,
                }
            }
            _ => false,
        }
    }

    pub(crate) fn has_trailing_comma(
        &self,
        parent_node: &ast::Node,
        children_has_trailing_comma: bool,
        _children_position_key: ast::NodeListPositionKey,
    ) -> bool {
        // NodeList.HasTrailingComma() is unreliable on transformed nodes as some nodes may have been removed. In the event
        // we believe we may need to emit a trailing comma, we must first look to the respective node list on the original
        // node first.
        if !children_has_trailing_comma {
            return false;
        }

        let original_parent = self.emit_context.most_original(parent_node);
        if original_parent == *parent_node {
            // if this node is the original node, we can trust the result
            return true;
        }

        let original_store = self.store_for_node(&original_parent);
        if original_store.kind(original_parent) != self.kind(parent_node) {
            // if the original node is some other kind of node, we cannot correlate the list
            return false;
        }

        // find the respective node list on the original parent
        match self.kind(parent_node) {
            ast::Kind::ObjectLiteralExpression => original_store
                .source_properties(original_parent)
                .is_some_and(|original_list| original_list.has_trailing_comma()),
            ast::Kind::NamedImports | ast::Kind::NamedExports => original_store
                .source_elements(original_parent)
                .is_some_and(|original_list| original_list.has_trailing_comma()),
            _ => false,
        }
    }

    pub(crate) fn write_token_text(
        &mut self,
        token: ast::Kind,
        write_kind: WriteKind,
        pos: i32,
    ) -> i32 {
        let token_string = scanner::token_to_string(token);
        self.write_as(&token_string, write_kind);
        if ast::position_is_synthesized(pos) {
            pos
        } else {
            pos + token_string.len() as i32
        }
    }

    pub(crate) fn emit_token(
        &mut self,
        token: ast::Kind,
        pos: i32,
        write_kind: WriteKind,
        context_node: &ast::Node,
    ) -> i32 {
        self.emit_token_ex(token, pos, write_kind, context_node, TokenEmitFlags::NONE)
    }

    pub(crate) fn emit_token_ex(
        &mut self,
        token: ast::Kind,
        pos: i32,
        write_kind: WriteKind,
        context_node: &ast::Node,
        flags: TokenEmitFlags,
    ) -> i32 {
        let (state, pos) = self.enter_token(token, pos, context_node, flags);
        let pos = self.write_token_text(token, write_kind, pos);
        self.exit_token(token, pos, context_node, state);
        pos
    }

    pub(crate) fn emit_keyword_node(&mut self, node: Option<&ast::Node>) {
        self.emit_keyword_node_ex(node, TokenEmitFlags::NONE)
    }

    pub(crate) fn emit_keyword_node_ex(&mut self, node: Option<&ast::Node>, flags: TokenEmitFlags) {
        let Some(node) = node else {
            return;
        };

        let state = self.enter_token_node(node, flags);
        self.write_token_text(self.kind(node), WriteKind::Keyword, self.loc(node).pos());
        self.exit_token_node(node, state);
    }

    fn emit_keyword_node_ex_to_writer(
        &mut self,
        node: Option<&ast::Node>,
        flags: TokenEmitFlags,
        writer: &mut dyn EmitTextWriter,
    ) {
        let Some(node) = node else {
            return;
        };

        let state = self.enter_token_node_to_writer(node, flags, writer);
        self.write_token_text_to_writer(
            self.kind(node),
            WriteKind::Keyword,
            self.loc(node).pos(),
            writer,
        );
        self.exit_token_node_to_writer(node, state, writer);
    }

    pub(crate) fn emit_punctuation_node(&mut self, node: Option<&ast::Node>) {
        self.emit_punctuation_node_ex(node, TokenEmitFlags::NONE)
    }

    pub(crate) fn emit_punctuation_node_ex(
        &mut self,
        node: Option<&ast::Node>,
        flags: TokenEmitFlags,
    ) {
        let Some(node) = node else {
            return;
        };

        let state = self.enter_token_node(node, flags);
        self.write_token_text(
            self.kind(node),
            WriteKind::Punctuation,
            self.loc(node).pos(),
        );
        self.exit_token_node(node, state);
    }

    fn emit_punctuation_node_to_writer(
        &mut self,
        node: Option<&ast::Node>,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_punctuation_node_ex_to_writer(node, TokenEmitFlags::NONE, writer)
    }

    fn emit_punctuation_node_ex_to_writer(
        &mut self,
        node: Option<&ast::Node>,
        flags: TokenEmitFlags,
        writer: &mut dyn EmitTextWriter,
    ) {
        let Some(node) = node else {
            return;
        };

        let state = self.enter_token_node_to_writer(node, flags, writer);
        self.write_token_text_to_writer(
            self.kind(node),
            WriteKind::Punctuation,
            self.loc(node).pos(),
            writer,
        );
        self.exit_token_node_to_writer(node, state, writer);
    }

    pub(crate) fn emit_token_node(&mut self, node: Option<&ast::Node>) {
        self.emit_token_node_ex(node, TokenEmitFlags::NONE)
    }

    pub(crate) fn emit_token_node_ex(&mut self, node: Option<&ast::Node>, flags: TokenEmitFlags) {
        let Some(node) = node else {
            return;
        };

        let kind = self.kind(node);
        if ast::is_keyword(kind) {
            self.emit_keyword_node_ex(Some(node), flags);
        } else if is_punctuation_kind(kind) {
            self.emit_punctuation_node_ex(Some(node), flags);
        } else {
            panic!("unexpected TokenNode: {}", kind);
        }
    }

    pub(crate) fn emit_literal(&mut self, node: &ast::Node, mut flags: GetLiteralTextFlags) {
        if self.options.never_ascii_escape {
            flags |= GetLiteralTextFlags::NEVER_ASCII_ESCAPE;
        }
        if self.options.terminate_unterminated_literals {
            flags |= GetLiteralTextFlags::TERMINATE_UNTERMINATED_LITERALS;
        }

        let text = self.get_literal_text_of_node(node, None, flags);
        self.writer
            .as_ref()
            .unwrap()
            .borrow_mut()
            .write_string_literal(&text);
    }

    pub(crate) fn emit_numeric_literal(&mut self, node: &ast::Node) {
        let state = self.enter_node(node);
        self.emit_literal(node, GetLiteralTextFlags::NONE);
        self.exit_node(node, state);
    }

    pub(crate) fn emit_big_int_literal(&mut self, node: &ast::Node) {
        let state = self.enter_node(node);
        self.emit_literal(node, GetLiteralTextFlags::NONE);
        self.exit_node(node, state);
    }

    pub(crate) fn emit_string_literal(&mut self, node: &ast::Node) {
        let state = self.enter_node(node);
        self.emit_literal(node, GetLiteralTextFlags::NONE);
        self.exit_node(node, state);
    }

    pub(crate) fn emit_no_substitution_template_literal(&mut self, node: &ast::Node) {
        let state = self.enter_node(node);
        self.emit_literal(node, GetLiteralTextFlags::NONE);
        self.exit_node(node, state);
    }

    pub(crate) fn emit_regular_expression_literal(&mut self, node: &ast::Node) {
        let state = self.enter_node(node);
        self.emit_literal(node, GetLiteralTextFlags::NONE);
        self.exit_node(node, state);
    }

    pub(crate) fn emit_template_head(&mut self, node: &ast::Node) {
        let state = self.enter_node(node);
        self.emit_literal(node, GetLiteralTextFlags::NONE);
        self.exit_node(node, state);
    }

    pub(crate) fn emit_template_middle(&mut self, node: &ast::Node) {
        let state = self.enter_node(node);
        self.emit_literal(node, GetLiteralTextFlags::NONE);
        self.exit_node(node, state);
    }

    pub(crate) fn emit_template_tail(&mut self, node: &ast::Node) {
        let state = self.enter_node(node);
        self.emit_literal(node, GetLiteralTextFlags::NONE);
        self.exit_node(node, state);
    }

    pub(crate) fn emit_template_middle_tail(&mut self, node: &ast::Node) {
        match self.kind(node) {
            ast::Kind::TemplateMiddle => self.emit_template_middle(node),
            ast::Kind::TemplateTail => self.emit_template_tail(node),
            _ => {}
        }
    }

    fn enter_node(&mut self, node: &ast::Node) -> PrinterState {
        let mut state = PrinterState::default();

        if let Some(on_before_emit_node) = self.print_handlers.on_before_emit_node.as_mut() {
            on_before_emit_node(Some(node));
        }

        state.comment_state = self.emit_comments_before_node(node);
        state.source_map_state = self.emit_source_maps_before_node(node);
        state
    }

    fn exit_node(&mut self, node: &ast::Node, previous_state: PrinterState) {
        self.emit_source_maps_after_node(node, previous_state.source_map_state);
        self.emit_comments_after_node(node, previous_state.comment_state);

        if let Some(on_after_emit_node) = self.print_handlers.on_after_emit_node.as_mut() {
            on_after_emit_node(Some(node));
        }
    }

    fn enter_token(
        &mut self,
        token: ast::Kind,
        pos: i32,
        context_node: &ast::Node,
        flags: TokenEmitFlags,
    ) -> (PrinterState, i32) {
        let mut state = PrinterState::default();
        let (comment_state, pos) = self.emit_comments_before_token(token, pos, context_node, flags);
        state.comment_state = comment_state;
        state.source_map_state =
            self.emit_source_maps_before_token(token, pos, context_node, flags);
        (state, pos)
    }

    fn exit_token(
        &mut self,
        token: ast::Kind,
        pos: i32,
        context_node: &ast::Node,
        previous_state: PrinterState,
    ) {
        self.emit_source_maps_after_token(
            token,
            pos,
            context_node,
            previous_state.source_map_state,
        );
        self.emit_comments_after_token(token, pos, context_node, previous_state.comment_state);
    }

    fn enter_token_node(&mut self, node: &ast::Node, flags: TokenEmitFlags) -> PrinterState {
        let mut state = PrinterState::default();

        if let Some(on_before_emit_token) = self.print_handlers.on_before_emit_token.as_mut() {
            on_before_emit_token(Some(node));
        }

        if !flags.contains(TokenEmitFlags::NO_COMMENTS) {
            state.comment_state = self.emit_comments_before_node(node);
        }
        if !flags.contains(TokenEmitFlags::NO_SOURCE_MAPS) {
            state.source_map_state = self.emit_source_maps_before_node(node);
        }
        state
    }

    fn exit_token_node(&mut self, node: &ast::Node, previous_state: PrinterState) {
        self.emit_source_maps_after_node(node, previous_state.source_map_state);
        self.emit_comments_after_node(node, previous_state.comment_state);

        if let Some(on_after_emit_token) = self.print_handlers.on_after_emit_token.as_mut() {
            on_after_emit_token(Some(node));
        }
    }

    fn enter_token_node_to_writer(
        &mut self,
        node: &ast::Node,
        flags: TokenEmitFlags,
        writer: &mut dyn EmitTextWriter,
    ) -> PrinterState {
        let mut state = PrinterState::default();

        if let Some(on_before_emit_token) = self.print_handlers.on_before_emit_token.as_mut() {
            on_before_emit_token(Some(node));
        }

        if !flags.contains(TokenEmitFlags::NO_COMMENTS) {
            state.comment_state = self.emit_comments_before_node_to_writer(node, writer);
        }
        if !flags.contains(TokenEmitFlags::NO_SOURCE_MAPS) {
            state.source_map_state = self.emit_source_maps_before_node_to_writer(node, writer);
        }
        state
    }

    fn exit_token_node_to_writer(
        &mut self,
        node: &ast::Node,
        previous_state: PrinterState,
        writer: &mut dyn EmitTextWriter,
    ) {
        self.emit_source_maps_after_node_to_writer(node, previous_state.source_map_state, writer);
        self.emit_comments_after_node_to_writer(node, previous_state.comment_state, writer);

        if let Some(on_after_emit_token) = self.print_handlers.on_after_emit_token.as_mut() {
            on_after_emit_token(Some(node));
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum WriteKind {
    None = 0,
    Keyword,
    Operator,
    Punctuation,
    StringLiteral,
    Parameter,
    Property,
    Comment,
    Literal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ListFormat(u32);

impl ListFormat {
    const NONE: Self = Self(0);
    const SINGLE_LINE: Self = Self(0);
    const MULTI_LINE: Self = Self(1 << 0);
    const PRESERVE_LINES: Self = Self(1 << 1);
    const LINES_MASK: Self =
        Self(Self::SINGLE_LINE.0 | Self::MULTI_LINE.0 | Self::PRESERVE_LINES.0);

    const BAR_DELIMITED: Self = Self(1 << 2);
    const AMPERSAND_DELIMITED: Self = Self(1 << 3);
    const COMMA_DELIMITED: Self = Self(1 << 4);
    const ASTERISK_DELIMITED: Self = Self(1 << 5);
    const DELIMITERS_MASK: Self = Self(
        Self::BAR_DELIMITED.0
            | Self::AMPERSAND_DELIMITED.0
            | Self::COMMA_DELIMITED.0
            | Self::ASTERISK_DELIMITED.0,
    );

    const ALLOW_TRAILING_COMMA: Self = Self(1 << 6);

    const INDENTED: Self = Self(1 << 7);
    const SPACE_BETWEEN_BRACES: Self = Self(1 << 8);
    const SPACE_BETWEEN_SIBLINGS: Self = Self(1 << 9);

    const BRACES: Self = Self(1 << 10);
    const PARENTHESIS: Self = Self(1 << 11);
    const ANGLE_BRACKETS: Self = Self(1 << 12);
    const SQUARE_BRACKETS: Self = Self(1 << 13);
    const BRACKETS_MASK: Self = Self(
        Self::BRACES.0 | Self::PARENTHESIS.0 | Self::ANGLE_BRACKETS.0 | Self::SQUARE_BRACKETS.0,
    );

    const OPTIONAL_IF_NIL: Self = Self(1 << 14);
    const OPTIONAL_IF_EMPTY: Self = Self(1 << 15);
    const OPTIONAL: Self = Self(Self::OPTIONAL_IF_NIL.0 | Self::OPTIONAL_IF_EMPTY.0);

    const PREFER_NEW_LINE: Self = Self(1 << 16);
    const NO_TRAILING_NEW_LINE: Self = Self(1 << 17);
    const NO_INTERVENING_COMMENTS: Self = Self(1 << 18);
    const NO_SPACE_IF_EMPTY: Self = Self(1 << 19);
    const SINGLE_ELEMENT: Self = Self(1 << 20);
    const SPACE_AFTER_LIST: Self = Self(1 << 21);

    const SINGLE_LINE_TUPLE_TYPE_ELEMENTS: Self =
        Self(Self::COMMA_DELIMITED.0 | Self::SPACE_BETWEEN_SIBLINGS.0 | Self::SINGLE_LINE.0);
    const MULTI_LINE_TUPLE_TYPE_ELEMENTS: Self = Self(
        Self::COMMA_DELIMITED.0
            | Self::INDENTED.0
            | Self::SPACE_BETWEEN_SIBLINGS.0
            | Self::MULTI_LINE.0,
    );
    const UNION_TYPE_CONSTITUENTS: Self =
        Self(Self::BAR_DELIMITED.0 | Self::SPACE_BETWEEN_SIBLINGS.0 | Self::SINGLE_LINE.0);
    const INTERSECTION_TYPE_CONSTITUENTS: Self =
        Self(Self::AMPERSAND_DELIMITED.0 | Self::SPACE_BETWEEN_SIBLINGS.0 | Self::SINGLE_LINE.0);
    const HERITAGE_CLAUSES: Self = Self(Self::SINGLE_LINE.0 | Self::SPACE_BETWEEN_SIBLINGS.0);
    const CLASS_HERITAGE_CLAUSES: Self = Self(Self::SINGLE_LINE.0);
    const OBJECT_LITERAL_EXPRESSION_PROPERTIES: Self = Self(
        Self::PRESERVE_LINES.0
            | Self::COMMA_DELIMITED.0
            | Self::SPACE_BETWEEN_SIBLINGS.0
            | Self::SPACE_BETWEEN_BRACES.0
            | Self::INDENTED.0
            | Self::BRACES.0
            | Self::NO_SPACE_IF_EMPTY.0,
    );
    const IMPORT_ATTRIBUTES: Self = Self(
        Self::PRESERVE_LINES.0
            | Self::COMMA_DELIMITED.0
            | Self::SPACE_BETWEEN_SIBLINGS.0
            | Self::SPACE_BETWEEN_BRACES.0
            | Self::INDENTED.0
            | Self::BRACES.0
            | Self::NO_SPACE_IF_EMPTY.0,
    );
    const ARRAY_LITERAL_EXPRESSION_ELEMENTS: Self = Self(
        Self::PRESERVE_LINES.0
            | Self::COMMA_DELIMITED.0
            | Self::SPACE_BETWEEN_SIBLINGS.0
            | Self::ALLOW_TRAILING_COMMA.0
            | Self::INDENTED.0
            | Self::SQUARE_BRACKETS.0,
    );
    const CALL_EXPRESSION_ARGUMENTS: Self = Self(
        Self::COMMA_DELIMITED.0
            | Self::SPACE_BETWEEN_SIBLINGS.0
            | Self::SINGLE_LINE.0
            | Self::PARENTHESIS.0,
    );
    const NEW_EXPRESSION_ARGUMENTS: Self = Self(
        Self::COMMA_DELIMITED.0
            | Self::SPACE_BETWEEN_SIBLINGS.0
            | Self::SINGLE_LINE.0
            | Self::PARENTHESIS.0
            | Self::OPTIONAL_IF_NIL.0,
    );
    const VARIABLE_DECLARATION_LIST: Self =
        Self(Self::COMMA_DELIMITED.0 | Self::SPACE_BETWEEN_SIBLINGS.0 | Self::SINGLE_LINE.0);
    const ENUM_MEMBERS: Self =
        Self(Self::COMMA_DELIMITED.0 | Self::INDENTED.0 | Self::MULTI_LINE.0);
    const SINGLE_LINE_TYPE_LITERAL_MEMBERS: Self =
        Self(Self::SINGLE_LINE.0 | Self::SPACE_BETWEEN_BRACES.0 | Self::SPACE_BETWEEN_SIBLINGS.0);
    const MULTI_LINE_TYPE_LITERAL_MEMBERS: Self =
        Self(Self::MULTI_LINE.0 | Self::INDENTED.0 | Self::OPTIONAL_IF_EMPTY.0);
    const OBJECT_BINDING_PATTERN_ELEMENTS: Self = Self(
        Self::SINGLE_LINE.0
            | Self::ALLOW_TRAILING_COMMA.0
            | Self::SPACE_BETWEEN_BRACES.0
            | Self::COMMA_DELIMITED.0
            | Self::SPACE_BETWEEN_SIBLINGS.0
            | Self::NO_SPACE_IF_EMPTY.0,
    );
    const ARRAY_BINDING_PATTERN_ELEMENTS: Self = Self(
        Self::SINGLE_LINE.0
            | Self::ALLOW_TRAILING_COMMA.0
            | Self::COMMA_DELIMITED.0
            | Self::SPACE_BETWEEN_SIBLINGS.0
            | Self::NO_SPACE_IF_EMPTY.0,
    );
    const TEMPLATE_EXPRESSION_SPANS: Self =
        Self(Self::SINGLE_LINE.0 | Self::NO_INTERVENING_COMMENTS.0);
    const SINGLE_LINE_BLOCK_STATEMENTS: Self =
        Self(Self::SPACE_BETWEEN_BRACES.0 | Self::SPACE_BETWEEN_SIBLINGS.0 | Self::SINGLE_LINE.0);
    const MULTI_LINE_BLOCK_STATEMENTS: Self = Self(Self::INDENTED.0 | Self::MULTI_LINE.0);
    const SINGLE_LINE_FUNCTION_BODY_STATEMENTS: Self =
        Self(Self::SINGLE_LINE.0 | Self::SPACE_BETWEEN_SIBLINGS.0 | Self::SPACE_BETWEEN_BRACES.0);
    const MULTI_LINE_FUNCTION_BODY_STATEMENTS: Self = Self(Self::MULTI_LINE.0);
    const MODIFIERS: Self = Self(
        Self::SINGLE_LINE.0
            | Self::SPACE_BETWEEN_SIBLINGS.0
            | Self::NO_INTERVENING_COMMENTS.0
            | Self::SPACE_AFTER_LIST.0,
    );
    const DECORATORS: Self = Self(Self::MULTI_LINE.0 | Self::OPTIONAL.0 | Self::SPACE_AFTER_LIST.0);
    const CLASS_MEMBERS: Self = Self(Self::INDENTED.0 | Self::MULTI_LINE.0);
    const INTERFACE_MEMBERS: Self = Self(Self::INDENTED.0 | Self::MULTI_LINE.0);
    const CASE_BLOCK_CLAUSES: Self = Self(Self::INDENTED.0 | Self::MULTI_LINE.0);
    const NAMED_IMPORTS_OR_EXPORTS_ELEMENTS: Self = Self(
        Self::COMMA_DELIMITED.0
            | Self::SPACE_BETWEEN_SIBLINGS.0
            | Self::ALLOW_TRAILING_COMMA.0
            | Self::SINGLE_LINE.0
            | Self::SPACE_BETWEEN_BRACES.0
            | Self::NO_SPACE_IF_EMPTY.0,
    );
    const JSX_ELEMENT_OR_FRAGMENT_CHILDREN: Self =
        Self(Self::SINGLE_LINE.0 | Self::NO_INTERVENING_COMMENTS.0);
    const JSX_ELEMENT_ATTRIBUTES: Self = Self(
        Self::SINGLE_LINE.0 | Self::SPACE_BETWEEN_SIBLINGS.0 | Self::NO_INTERVENING_COMMENTS.0,
    );
    const CASE_OR_DEFAULT_CLAUSE_STATEMENTS: Self = Self(
        Self::INDENTED.0
            | Self::MULTI_LINE.0
            | Self::NO_TRAILING_NEW_LINE.0
            | Self::OPTIONAL_IF_EMPTY.0,
    );
    const HERITAGE_CLAUSE_TYPES: Self =
        Self(Self::COMMA_DELIMITED.0 | Self::SPACE_BETWEEN_SIBLINGS.0 | Self::SINGLE_LINE.0);
    const SOURCE_FILE_STATEMENTS: Self = Self(Self::MULTI_LINE.0 | Self::NO_TRAILING_NEW_LINE.0);
    const TYPE_ARGUMENTS: Self = Self(
        Self::COMMA_DELIMITED.0
            | Self::SPACE_BETWEEN_SIBLINGS.0
            | Self::SINGLE_LINE.0
            | Self::ANGLE_BRACKETS.0
            | Self::OPTIONAL.0,
    );
    const TYPE_PARAMETERS: Self = Self(
        Self::COMMA_DELIMITED.0
            | Self::SPACE_BETWEEN_SIBLINGS.0
            | Self::SINGLE_LINE.0
            | Self::ANGLE_BRACKETS.0
            | Self::OPTIONAL.0,
    );
    const PARAMETERS: Self = Self(
        Self::COMMA_DELIMITED.0
            | Self::SPACE_BETWEEN_SIBLINGS.0
            | Self::SINGLE_LINE.0
            | Self::PARENTHESIS.0,
    );
    const SINGLE_ARROW_PARAMETER: Self =
        Self(Self::COMMA_DELIMITED.0 | Self::SPACE_BETWEEN_SIBLINGS.0 | Self::SINGLE_LINE.0);
    const INDEX_SIGNATURE_PARAMETERS: Self = Self(
        Self::COMMA_DELIMITED.0
            | Self::SPACE_BETWEEN_SIBLINGS.0
            | Self::SINGLE_LINE.0
            | Self::INDENTED.0
            | Self::SQUARE_BRACKETS.0,
    );

    fn contains(self, rhs: Self) -> bool {
        self.0 & rhs.0 != 0
    }

    fn intersects(self, rhs: Self) -> bool {
        self.0 & rhs.0 != 0
    }

    fn mask_eq(self, mask: Self, expected: Self) -> bool {
        self & mask == expected
    }

    fn opening_bracket(self) -> &'static str {
        match self & Self::BRACKETS_MASK {
            Self::BRACES => "{",
            Self::PARENTHESIS => "(",
            Self::ANGLE_BRACKETS => "<",
            Self::SQUARE_BRACKETS => "[",
            _ => panic!("unexpected opening bracket format: {self:?}"),
        }
    }

    fn closing_bracket(self) -> &'static str {
        match self & Self::BRACKETS_MASK {
            Self::BRACES => "}",
            Self::PARENTHESIS => ")",
            Self::ANGLE_BRACKETS => ">",
            Self::SQUARE_BRACKETS => "]",
            _ => panic!("unexpected closing bracket format: {self:?}"),
        }
    }
}

impl std::ops::BitOr for ListFormat {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for ListFormat {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl std::ops::BitAnd for ListFormat {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl std::ops::Not for ListFormat {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenEmitFlags(u32);

impl TokenEmitFlags {
    pub const NONE: Self = Self(0);
    pub const NO_COMMENTS: Self = Self(1 << 0);
    pub const INDENT_LEADING_COMMENTS: Self = Self(1 << 1);
    pub const NO_SOURCE_MAPS: Self = Self(1 << 2);

    fn contains(self, rhs: Self) -> bool {
        self.0 & rhs.0 != 0
    }
}

pub fn range_is_on_single_line(r: core::TextRange, source_file: &ast::SourceFile) -> bool {
    range_start_is_on_same_line_as_range_end(r, r, source_file)
}

pub fn range_start_positions_are_on_same_line(
    range1: core::TextRange,
    range2: core::TextRange,
    source_file: &ast::SourceFile,
) -> bool {
    positions_are_on_same_line(
        get_start_position_of_range(range1, source_file, false),
        get_start_position_of_range(range2, source_file, false),
        source_file,
    )
}

fn range_end_positions_are_on_same_line(
    range1: core::TextRange,
    range2: core::TextRange,
    source_file: &ast::SourceFile,
) -> bool {
    positions_are_on_same_line(range1.end(), range2.end(), source_file)
}

fn range_start_is_on_same_line_as_range_end(
    range1: core::TextRange,
    range2: core::TextRange,
    source_file: &ast::SourceFile,
) -> bool {
    positions_are_on_same_line(
        get_start_position_of_range(range1, source_file, false),
        range2.end(),
        source_file,
    )
}

fn range_end_is_on_same_line_as_range_start(
    range1: core::TextRange,
    range2: core::TextRange,
    source_file: &ast::SourceFile,
) -> bool {
    positions_are_on_same_line(
        range1.end(),
        get_start_position_of_range(range2, source_file, false),
        source_file,
    )
}

fn get_start_position_of_range(
    r: core::TextRange,
    source_file: &ast::SourceFile,
    include_comments: bool,
) -> i32 {
    if ast::position_is_synthesized(r.pos()) {
        return -1;
    }
    scanner::skip_trivia_ex(
        source_file.text(),
        r.pos() as usize,
        Some(&scanner::SkipTriviaOptions {
            stop_after_line_break: false,
            stop_at_comments: include_comments,
        }),
    ) as i32
}

pub fn positions_are_on_same_line(pos1: i32, pos2: i32, source_file: &ast::SourceFile) -> bool {
    get_lines_between_positions(source_file, pos1, pos2) == 0
}

pub fn get_lines_between_positions(source_file: &ast::SourceFile, pos1: i32, pos2: i32) -> i32 {
    if pos1 == pos2 {
        return 0;
    }
    let line_starts = source_file.ecma_line_map();
    let lower = if pos1 < pos2 { pos1 } else { pos2 };
    let is_negative = lower == pos2;
    let upper = if is_negative { pos1 } else { pos2 };
    let lower_line = scanner::compute_line_of_position(&line_starts, lower as usize);
    let upper_line =
        lower_line + scanner::compute_line_of_position(&line_starts[lower_line..], upper as usize);
    if is_negative {
        lower_line as i32 - upper_line as i32
    } else {
        upper_line as i32 - lower_line as i32
    }
}

fn get_lines_between_range_end_and_range_start(
    range1: core::TextRange,
    range2: core::TextRange,
    source_file: &ast::SourceFile,
    include_second_range_comments: bool,
) -> i32 {
    let range2_start =
        get_start_position_of_range(range2, source_file, include_second_range_comments);
    get_lines_between_positions(source_file, range1.end(), range2_start)
}

fn get_lines_between_position_and_preceding_non_whitespace_character(
    pos: i32,
    stop_pos: i32,
    source_file: &ast::SourceFile,
    include_comments: bool,
) -> i32 {
    let start_pos = scanner::skip_trivia_ex(
        source_file.text(),
        pos as usize,
        Some(&scanner::SkipTriviaOptions {
            stop_after_line_break: false,
            stop_at_comments: include_comments,
        }),
    ) as i32;
    let prev_pos = get_previous_non_whitespace_position(start_pos, stop_pos, source_file);
    get_lines_between_positions(
        source_file,
        if prev_pos >= 0 { prev_pos } else { stop_pos },
        start_pos,
    )
}

fn get_lines_between_position_and_next_non_whitespace_character(
    pos: i32,
    stop_pos: i32,
    source_file: &ast::SourceFile,
    include_comments: bool,
) -> i32 {
    let next_pos = scanner::skip_trivia_ex(
        source_file.text(),
        pos as usize,
        Some(&scanner::SkipTriviaOptions {
            stop_after_line_break: false,
            stop_at_comments: include_comments,
        }),
    ) as i32;
    get_lines_between_positions(
        source_file,
        pos,
        if stop_pos < next_pos {
            stop_pos
        } else {
            next_pos
        },
    )
}

fn get_previous_non_whitespace_position(
    mut pos: i32,
    stop_pos: i32,
    source_file: &ast::SourceFile,
) -> i32 {
    let text = source_file.text();
    while pos >= stop_pos {
        if let Some(ch) = text
            .get(pos as usize..)
            .and_then(|text| text.chars().next())
        {
            if !stringutil::is_white_space_like(ch) {
                return pos;
            }
        }
        pos -= 1;
    }
    -1
}

fn write_lines_and_indent_to_writer(
    writer: &mut dyn EmitTextWriter,
    line_count: i32,
    write_space_if_not_indenting: bool,
) {
    if line_count > 0 {
        writer.increase_indent();
        for _ in 0..line_count {
            writer.write_line();
        }
    } else if write_space_if_not_indenting {
        writer.write_space(" ");
    }
}

fn is_punctuation_kind(kind: ast::Kind) -> bool {
    ast::Kind::FirstPunctuation <= kind && kind <= ast::Kind::LastPunctuation
}

fn greatest_end(end: i32, ranges: &[core::TextRange]) -> i32 {
    ranges
        .iter()
        .rev()
        .fold(end, |end, range| end.max(range.end()))
}

fn greatest_end_nodes(end: i32, nodes: &[ast::Node], printer: &Printer) -> i32 {
    nodes
        .iter()
        .rev()
        .fold(end, |end, node| end.max(printer.loc(node).end()))
}
