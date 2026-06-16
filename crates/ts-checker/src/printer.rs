use ts_collections::FastHashMap as HashMap;
use ts_printer as printer;

use crate::checker::*;
use crate::nodebuilder::VerbosityContext;
use crate::nodebuilderimpl::{
    DEFAULT_MAXIMUM_TRUNCATION_LENGTH, NO_TRUNCATION_MAXIMUM_TRUNCATION_LENGTH,
};
use crate::{ast, core, nodebuilder};

fn take_emit_context_for_print(emit_context: &mut printer::EmitContext) -> printer::EmitContext {
    std::mem::replace(emit_context, printer::new_emit_context())
}

fn binding_facts_for_emit(
    checker: &Checker<'_, '_>,
    source_file: Option<&ast::SourceFile>,
) -> Option<std::sync::Arc<dyn printer::EmitBindingFacts>> {
    source_file.map(
        |source_file| -> std::sync::Arc<dyn printer::EmitBindingFacts> {
            checker.source_file_binding_state_arc(source_file)
        },
    )
}

fn create_printer(
    options: printer::PrinterOptions,
    emit_context: &mut printer::EmitContext,
    binding_facts: Option<std::sync::Arc<dyn printer::EmitBindingFacts>>,
) -> printer::Printer {
    let mut printer = printer::new_printer(
        options,
        printer::PrintHandlers::default(),
        Some(take_emit_context_for_print(emit_context)),
    );
    printer.set_binding_facts(binding_facts);
    printer
}

// TODO: Memoize once per checker to retain threadsafety
fn create_printer_with_defaults<'a>(
    emit_context: &'a mut printer::EmitContext,
    binding_facts: Option<std::sync::Arc<dyn printer::EmitBindingFacts>>,
) -> printer::Printer {
    create_printer(
        printer::PrinterOptions::default(),
        emit_context,
        binding_facts,
    )
}

fn create_printer_with_remove_comments<'a>(
    emit_context: &'a mut printer::EmitContext,
    binding_facts: Option<std::sync::Arc<dyn printer::EmitBindingFacts>>,
) -> printer::Printer {
    create_printer(
        printer::PrinterOptions {
            remove_comments: true,
            ..Default::default()
        },
        emit_context,
        binding_facts,
    )
}

fn create_printer_with_remove_comments_omit_trailing_semicolon_never_ascii_escape<'a>(
    emit_context: &'a mut printer::EmitContext,
    binding_facts: Option<std::sync::Arc<dyn printer::EmitBindingFacts>>,
) -> printer::Printer {
    // TODO: OmitTrailingSemicolon support
    create_printer(
        printer::PrinterOptions {
            remove_comments: true,
            never_ascii_escape: true,
            ..Default::default()
        },
        emit_context,
        binding_facts,
    )
}

fn create_printer_with_remove_comments_never_ascii_escape<'a>(
    emit_context: &'a mut printer::EmitContext,
    binding_facts: Option<std::sync::Arc<dyn printer::EmitBindingFacts>>,
) -> printer::Printer {
    create_printer(
        printer::PrinterOptions {
            remove_comments: true,
            never_ascii_escape: true,
            ..Default::default()
        },
        emit_context,
        binding_facts,
    )
}

struct SemicolonRemoverWriter<'a> {
    has_pending_semicolon: bool,
    inner: Box<dyn printer::EmitTextWriter + 'a>,
}

impl<'a> SemicolonRemoverWriter<'a> {
    fn commit_semicolon(&mut self) {
        if self.has_pending_semicolon {
            self.inner.write_trailing_semicolon(";");
            self.has_pending_semicolon = false;
        }
    }

    fn clear(&mut self) {
        self.inner.clear();
    }

    fn decrease_indent(&mut self) {
        self.commit_semicolon();
        self.inner.decrease_indent();
    }

    fn get_column(&self) -> core::UTF16Offset {
        self.inner.get_column()
    }

    fn get_indent(&self) -> i32 {
        self.inner.get_indent()
    }

    fn get_line(&self) -> i32 {
        self.inner.get_line()
    }

    fn get_text_pos(&self) -> i32 {
        self.inner.get_text_pos()
    }

    fn has_trailing_comment(&self) -> bool {
        self.inner.has_trailing_comment()
    }

    fn has_trailing_whitespace(&self) -> bool {
        self.inner.has_trailing_whitespace()
    }

    fn increase_indent(&mut self) {
        self.commit_semicolon();
        self.inner.increase_indent();
    }

    fn is_at_start_of_line(&self) -> bool {
        self.inner.is_at_start_of_line()
    }

    fn raw_write(&mut self, s1: &str) {
        self.commit_semicolon();
        self.inner.raw_write(s1);
    }

    fn string(&mut self) -> String {
        self.inner.string()
    }

    fn write(&mut self, s1: &str) {
        self.commit_semicolon();
        self.inner.write(s1);
    }

    fn write_comment(&mut self, text: &str) {
        self.commit_semicolon();
        self.inner.write_comment(text);
    }

    fn write_keyword(&mut self, text: &str) {
        self.commit_semicolon();
        self.inner.write_keyword(text);
    }

    fn write_line(&mut self) {
        self.commit_semicolon();
        self.inner.write_line();
    }

    fn write_line_force(&mut self, force: bool) {
        self.commit_semicolon();
        self.inner.write_line_force(force);
    }

    fn write_literal(&mut self, s1: &str) {
        self.commit_semicolon();
        self.inner.write_literal(s1);
    }

    fn write_operator(&mut self, text: &str) {
        self.commit_semicolon();
        self.inner.write_operator(text);
    }

    fn write_parameter(&mut self, text: &str) {
        self.commit_semicolon();
        self.inner.write_parameter(text);
    }

    fn write_property(&mut self, text: &str) {
        self.commit_semicolon();
        self.inner.write_property(text);
    }

    fn write_punctuation(&mut self, text: &str) {
        self.commit_semicolon();
        self.inner.write_punctuation(text);
    }

    fn write_space(&mut self, text: &str) {
        self.commit_semicolon();
        self.inner.write_space(text);
    }

    fn write_string_literal(&mut self, text: &str) {
        self.commit_semicolon();
        self.inner.write_string_literal(text);
    }

    fn write_symbol(&mut self, text: &str, symbol: Option<ast::SymbolHandle>) {
        self.commit_semicolon();
        self.inner.write_symbol(text, symbol);
    }

    fn write_trailing_semicolon(&mut self, _text: &str) {
        self.has_pending_semicolon = true;
    }
}

impl<'a> printer::EmitTextWriter for SemicolonRemoverWriter<'a> {
    fn write(&mut self, s: &str) {
        SemicolonRemoverWriter::write(self, s)
    }

    fn write_trailing_semicolon(&mut self, text: &str) {
        SemicolonRemoverWriter::write_trailing_semicolon(self, text)
    }

    fn write_comment(&mut self, text: &str) {
        SemicolonRemoverWriter::write_comment(self, text)
    }

    fn write_keyword(&mut self, text: &str) {
        SemicolonRemoverWriter::write_keyword(self, text)
    }

    fn write_operator(&mut self, text: &str) {
        SemicolonRemoverWriter::write_operator(self, text)
    }

    fn write_punctuation(&mut self, text: &str) {
        SemicolonRemoverWriter::write_punctuation(self, text)
    }

    fn write_space(&mut self, text: &str) {
        SemicolonRemoverWriter::write_space(self, text)
    }

    fn write_string_literal(&mut self, text: &str) {
        SemicolonRemoverWriter::write_string_literal(self, text)
    }

    fn write_parameter(&mut self, text: &str) {
        SemicolonRemoverWriter::write_parameter(self, text)
    }

    fn write_property(&mut self, text: &str) {
        SemicolonRemoverWriter::write_property(self, text)
    }

    fn write_symbol(&mut self, text: &str, symbol: Option<ast::SymbolHandle>) {
        self.commit_semicolon();
        self.inner.write_symbol(text, symbol)
    }

    fn write_line(&mut self) {
        SemicolonRemoverWriter::write_line(self)
    }

    fn write_line_force(&mut self, force: bool) {
        SemicolonRemoverWriter::write_line_force(self, force)
    }

    fn increase_indent(&mut self) {
        SemicolonRemoverWriter::increase_indent(self)
    }

    fn decrease_indent(&mut self) {
        SemicolonRemoverWriter::decrease_indent(self)
    }

    fn clear(&mut self) {
        SemicolonRemoverWriter::clear(self)
    }

    fn string(&self) -> String {
        self.inner.string()
    }

    fn raw_write(&mut self, s: &str) {
        SemicolonRemoverWriter::raw_write(self, s)
    }

    fn write_literal(&mut self, s: &str) {
        SemicolonRemoverWriter::write_literal(self, s)
    }

    fn get_text_pos(&self) -> i32 {
        SemicolonRemoverWriter::get_text_pos(self)
    }

    fn get_line(&self) -> i32 {
        SemicolonRemoverWriter::get_line(self)
    }

    fn get_column(&self) -> core::UTF16Offset {
        SemicolonRemoverWriter::get_column(self)
    }

    fn get_indent(&self) -> i32 {
        SemicolonRemoverWriter::get_indent(self)
    }

    fn is_at_start_of_line(&self) -> bool {
        SemicolonRemoverWriter::is_at_start_of_line(self)
    }

    fn has_trailing_comment(&self) -> bool {
        SemicolonRemoverWriter::has_trailing_comment(self)
    }

    fn has_trailing_whitespace(&self) -> bool {
        SemicolonRemoverWriter::has_trailing_whitespace(self)
    }
}

fn get_trailing_semicolon_deferring_writer(
    writer: Box<dyn printer::EmitTextWriter>,
) -> Box<dyn printer::EmitTextWriter> {
    Box::new(SemicolonRemoverWriter {
        has_pending_semicolon: false,
        inner: writer,
    })
}

impl<'a, 'state> Checker<'a, 'state> {
    pub fn type_to_string_public(&mut self, t: TypeHandle) -> String {
        self.type_to_string(t, None)
    }

    pub(crate) fn type_to_string(
        &mut self,
        t: TypeHandle,
        enclosing_declaration: Option<ast::Node>,
    ) -> String {
        self.type_to_string_ex(
            t,
            enclosing_declaration,
            TYPE_FORMAT_FLAGS_ALLOW_UNIQUE_ES_SYMBOL_TYPE
                | TYPE_FORMAT_FLAGS_USE_ALIAS_DEFINED_OUTSIDE_CURRENT_SCOPE,
            None,
        )
    }
}

fn to_node_builder_flags(flags: TypeFormatFlags) -> nodebuilder::Flags {
    flags & TYPE_FORMAT_FLAGS_NODE_BUILDER_FLAGS_MASK
}

fn source_file_from_node(checker: &Checker<'_, '_>, node: ast::Node) -> Option<ast::SourceFile> {
    checker
        .try_source_file_for_node(node)
        .map(ast::SourceFile::share_readonly)
}

fn source_file_for_emit(
    checker: &Checker<'_, '_>,
    enclosing_declaration: Option<ast::Node>,
) -> Option<ast::SourceFile> {
    enclosing_declaration
        .and_then(|node| source_file_from_node(checker, node))
        .or_else(|| {
            checker
                .current_source_file()
                .map(ast::SourceFile::share_readonly)
        })
}

fn kind_for_emit_node(checker: &Checker<'_, '_>, node: ast::Node) -> Option<ast::Kind> {
    if let Some(source_file) = checker.try_source_file_for_node(node) {
        return Some(source_file.store().kind(node));
    }

    let factory_store = checker.factory().store();
    if node.store_id() == factory_store.store_id() {
        return Some(factory_store.kind(node));
    }

    None
}

impl<'a, 'state> Checker<'a, 'state> {
    pub fn type_to_baseline_string_public(&mut self, t: TypeHandle, node: ast::Node) -> String {
        let (enclosing_declaration, is_type_alias_name, node_text) = {
            let node_store = self.store_for_node(node);
            let enclosing_declaration = node_store.parent(node);
            let is_type_alias_name = ast::is_identifier(node_store, node)
                && enclosing_declaration.is_some_and(|parent| {
                    ast::is_type_alias_declaration(node_store, parent)
                        && node_store.name(parent) == Some(node)
                });
            let node_text = if ast::is_identifier(node_store, node) {
                node_store.text(node).to_string()
            } else {
                String::new()
            };
            (enclosing_declaration, is_type_alias_name, node_text)
        };
        let type_format_flags = TYPE_FORMAT_FLAGS_NO_TRUNCATION
            | TYPE_FORMAT_FLAGS_ALLOW_UNIQUE_ES_SYMBOL_TYPE
            | TYPE_FORMAT_FLAGS_GENERATE_NAMES_FOR_SHADOWED_TYPE_PARAMS;
        let node_builder_flags =
            to_node_builder_flags(type_format_flags) | nodebuilder::FLAGS_IGNORE_ERRORS;
        let source_file = source_file_for_emit(self, enclosing_declaration);
        let binding_facts = binding_facts_for_emit(self, source_file.as_ref());
        let mut node_builder = self.get_node_builder();

        let mut type_node = node_builder
            .type_to_type_node(
                t,
                enclosing_declaration,
                node_builder_flags,
                nodebuilder::INTERNAL_FLAGS_ALLOW_UNRESOLVED_NAMES,
                None,
            )
            .expect("type baseline should always get a type node");

        if is_type_alias_name {
            let is_same_identifier = {
                let type_node_store = node_builder.store();
                ast::is_identifier(type_node_store, type_node)
                    && type_node_store.text(type_node) == node_text
            };
            if is_same_identifier {
                type_node = node_builder
                    .type_to_type_node(
                        t,
                        enclosing_declaration,
                        to_node_builder_flags(type_format_flags | TYPE_FORMAT_FLAGS_IN_TYPE_ALIAS)
                            | nodebuilder::FLAGS_IGNORE_ERRORS,
                        nodebuilder::INTERNAL_FLAGS_ALLOW_UNRESOLVED_NAMES,
                        None,
                    )
                    .expect("type alias baseline should always get a type node");
            }
        }

        let writer = printer::share_text_writer(printer::new_text_writer(String::new(), 0));
        let mut p = create_printer_with_remove_comments(node_builder.emit_context(), binding_facts);
        p.write_node(Some(&type_node), source_file.as_ref(), writer.clone(), None);
        writer.borrow().string()
    }

    pub fn type_to_string_ex_public(
        &mut self,
        t: TypeHandle,
        enclosing_declaration: Option<ast::Node>,
        flags: TypeFormatFlags,
        vc: Option<&mut VerbosityContext>,
    ) -> String {
        let enclosing_declaration = enclosing_declaration;
        self.type_to_string_ex(t, enclosing_declaration, flags, vc)
    }

    pub fn type_to_string_ex(
        &mut self,
        t: TypeHandle,
        enclosing_declaration: Option<ast::Node>,
        flags: TypeFormatFlags,
        mut vc: Option<&mut VerbosityContext>,
    ) -> String {
        let mut new_line = "";
        if flags & TYPE_FORMAT_FLAGS_MULTILINE_OBJECT_LITERALS != 0 {
            new_line = "\n";
        }
        let writer = printer::share_text_writer(printer::new_text_writer(new_line.to_string(), 0));
        let no_truncation = ((vc.is_none() || vc.as_ref().unwrap().max_truncation_length == 0)
            && self.compiler_options.no_error_truncation == core::TSTrue)
            || (flags & TYPE_FORMAT_FLAGS_NO_TRUNCATION != 0);
        let mut combined_flags = to_node_builder_flags(flags) | nodebuilder::FLAGS_IGNORE_ERRORS;
        if no_truncation {
            combined_flags |= nodebuilder::FLAGS_NO_TRUNCATION;
        }
        let is_unresolved_type = t == self.semantic_state.semantic_handles().unresolved_type;
        let source_file = source_file_for_emit(self, enclosing_declaration);
        let binding_facts = binding_facts_for_emit(self, source_file.as_ref());
        let mut node_builder = self.get_node_builder();
        if let Some(vc) = vc.as_deref_mut() {
            node_builder.set_verbosity(vc);
        }
        let type_node = node_builder.type_to_type_node(
            t,
            enclosing_declaration,
            combined_flags,
            nodebuilder::INTERNAL_FLAGS_NONE,
            None,
        );
        if let Some(vc) = vc.as_deref_mut() {
            node_builder.write_verbosity(vc);
        }
        if type_node.is_none() {
            panic!("should always get typenode");
        }
        let type_node = node_builder.prepare_optional_node_for_emit(type_node);
        // The unresolved type gets a synthesized comment on `any` to hint to users that it's not a plain `any`.
        // Otherwise, we always strip comments out.
        let mut p = if is_unresolved_type {
            create_printer_with_defaults(node_builder.emit_context(), binding_facts)
        } else {
            create_printer_with_remove_comments(node_builder.emit_context(), binding_facts)
        };
        p.write_node(
            type_node.as_ref(),
            source_file.as_ref(),
            writer.clone(),
            None,
        );
        let result = writer.borrow().string();

        let mut max_length = DEFAULT_MAXIMUM_TRUNCATION_LENGTH * 2;
        if let Some(vc) = vc.as_ref() {
            if vc.max_truncation_length > 0 {
                max_length = (vc.max_truncation_length as usize) * 10; // hard cutoff matching Strada's absoluteMaximumLength
            }
        }
        if no_truncation {
            max_length = NO_TRUNCATION_MAXIMUM_TRUNCATION_LENGTH * 2;
        }
        if max_length > 0 && !result.is_empty() && result.len() >= max_length {
            if let Some(vc) = vc {
                vc.truncated = true;
            }
            return format!("{}...", &result[..max_length - "...".len()]);
        }
        result
    }

    pub fn symbol_identity_to_string_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Option<String> {
        Some(self.symbol_identity_to_string(SymbolIdentity::from(symbol)))
    }

    pub(crate) fn symbol_identity_to_string(&mut self, symbol: SymbolIdentity) -> String {
        self.symbol_identity_to_string_ex(
            symbol,
            None,
            ast::SYMBOL_FLAGS_ALL,
            SYMBOL_FORMAT_FLAGS_ALLOW_ANY_NODE_KIND,
        )
    }

    pub fn symbol_identity_to_string_ex_public(
        &mut self,
        symbol: ast::SymbolIdentity,
        enclosing_declaration: Option<ast::Node>,
        meaning: ast::SymbolFlags,
        flags: SymbolFormatFlags,
    ) -> Option<String> {
        Some(self.symbol_identity_to_string_ex(
            SymbolIdentity::from(symbol),
            enclosing_declaration,
            meaning,
            flags,
        ))
    }

    pub(crate) fn symbol_identity_to_string_ex(
        &mut self,
        symbol: SymbolIdentity,
        enclosing_declaration: Option<ast::Node>,
        meaning: ast::SymbolFlags,
        flags: SymbolFormatFlags,
    ) -> String {
        let writer = printer::get_single_line_string_writer();

        let mut node_flags = nodebuilder::FLAGS_IGNORE_ERRORS;
        let mut internal_node_flags = nodebuilder::INTERNAL_FLAGS_NONE;
        if flags & SYMBOL_FORMAT_FLAGS_USE_ONLY_EXTERNAL_ALIASING != 0 {
            node_flags |= nodebuilder::FLAGS_USE_ONLY_EXTERNAL_ALIASING;
        }
        if flags & SYMBOL_FORMAT_FLAGS_WRITE_TYPE_PARAMETERS_OR_ARGUMENTS != 0 {
            node_flags |= nodebuilder::FLAGS_WRITE_TYPE_PARAMETERS_IN_QUALIFIED_NAME;
        }
        if flags & SYMBOL_FORMAT_FLAGS_USE_ALIAS_DEFINED_OUTSIDE_CURRENT_SCOPE != 0 {
            node_flags |= nodebuilder::FLAGS_USE_ALIAS_DEFINED_OUTSIDE_CURRENT_SCOPE;
        }
        if flags & SYMBOL_FORMAT_FLAGS_DO_NOT_INCLUDE_SYMBOL_CHAIN != 0 {
            internal_node_flags |= nodebuilder::INTERNAL_FLAGS_DO_NOT_INCLUDE_SYMBOL_CHAIN;
        }
        if flags & SYMBOL_FORMAT_FLAGS_WRITE_COMPUTED_PROPS != 0 {
            internal_node_flags |= nodebuilder::INTERNAL_FLAGS_WRITE_COMPUTED_PROPS;
        }

        let source_file = source_file_for_emit(self, enclosing_declaration);
        let binding_facts = binding_facts_for_emit(self, source_file.as_ref());
        let enclosing_is_source_file = enclosing_declaration
            .is_some_and(|node| kind_for_emit_node(self, node) == Some(ast::Kind::SourceFile));
        let mut node_builder = self.get_node_builder();
        let entity = if flags & SYMBOL_FORMAT_FLAGS_ALLOW_ANY_NODE_KIND != 0 {
            node_builder.symbol_to_node(
                symbol,
                meaning,
                enclosing_declaration,
                node_flags,
                internal_node_flags,
                None,
            ) // TODO: GH#18217
        } else {
            node_builder.symbol_to_entity_name(
                symbol,
                meaning,
                enclosing_declaration,
                node_flags,
                internal_node_flags,
                None,
            )
        };
        let entity = node_builder.prepare_optional_node_for_emit(entity);
        let mut printer_ = if enclosing_is_source_file {
            // add neverAsciiEscape for GH#39027
            create_printer_with_remove_comments_never_ascii_escape(
                node_builder.emit_context(),
                binding_facts,
            )
        } else {
            create_printer_with_remove_comments(node_builder.emit_context(), binding_facts)
        };
        let writer = printer::share_text_writer(writer);
        let deferred_writer = printer::share_text_writer(get_trailing_semicolon_deferring_writer(
            Box::new(printer::SharedEmitTextWriterHandle::new(writer.clone())),
        ));
        printer_.write_node(
            entity.as_ref(), /*sourceFile*/
            source_file.as_ref(),
            deferred_writer,
            None,
        ); // TODO: GH#18217
        let result = writer.borrow().string();
        result
    }

    pub(crate) fn signature_to_string(&mut self, signature: SignatureHandle) -> String {
        self.signature_to_string_ex(signature, None, TYPE_FORMAT_FLAGS_NONE, None, None)
    }

    pub fn signature_to_string_ex_public(
        &mut self,
        signature: SignatureHandle,
        enclosing_declaration: Option<ast::Node>,
        flags: TypeFormatFlags,
        kind: Option<SignatureKind>,
        vc: Option<&mut VerbosityContext>,
    ) -> String {
        self.signature_to_string_ex(signature, enclosing_declaration, flags, kind, vc)
    }

    fn signature_to_string_ex(
        &mut self,
        signature: SignatureHandle,
        enclosing_declaration: Option<ast::Node>,
        flags: TypeFormatFlags,
        kind: Option<SignatureKind>,
        vc: Option<&mut VerbosityContext>,
    ) -> String {
        let source_file = source_file_for_emit(self, enclosing_declaration);
        let binding_facts = binding_facts_for_emit(self, source_file.as_ref());
        let is_construct_kind = kind == Some(SIGNATURE_KIND_CONSTRUCT);
        let sig_output = if flags & TYPE_FORMAT_FLAGS_WRITE_ARROW_STYLE_SIGNATURE != 0 {
            if is_construct_kind {
                ast::Kind::ConstructorType
            } else {
                ast::Kind::FunctionType
            }
        } else if is_construct_kind {
            ast::Kind::ConstructSignature
        } else {
            ast::Kind::CallSignature
        };

        let mut node_builder = self.get_node_builder();
        let mut vc = vc;
        if let Some(ref vc) = vc {
            node_builder.set_verbosity(vc);
        }
        let combined_flags = to_node_builder_flags(flags)
            | nodebuilder::FLAGS_IGNORE_ERRORS
            | nodebuilder::FLAGS_WRITE_TYPE_PARAMETERS_IN_QUALIFIED_NAME;
        let sig = node_builder.signature_to_signature_declaration(
            signature,
            sig_output,
            enclosing_declaration,
            combined_flags,
            nodebuilder::INTERNAL_FLAGS_SIGNATURE_TO_STRING,
            None,
        );
        if let Some(vc) = vc.as_deref_mut() {
            node_builder.write_verbosity(vc);
        }
        let sig = node_builder.prepare_optional_node_for_emit(sig);
        let mut p = create_printer_with_remove_comments_omit_trailing_semicolon_never_ascii_escape(
            node_builder.emit_context(),
            binding_facts,
        );
        if flags & TYPE_FORMAT_FLAGS_MULTILINE_OBJECT_LITERALS != 0 {
            let writer = printer::new_text_writer("\n".to_string(), 0);
            let writer = printer::share_text_writer(writer);
            let deferred_writer =
                printer::share_text_writer(get_trailing_semicolon_deferring_writer(Box::new(
                    printer::SharedEmitTextWriterHandle::new(writer.clone()),
                )));
            p.write_node(sig.as_ref(), source_file.as_ref(), deferred_writer, None);
            return writer.borrow().string();
        }
        let writer = printer::get_single_line_string_writer();
        let writer = printer::share_text_writer(writer);
        let deferred_writer = printer::share_text_writer(get_trailing_semicolon_deferring_writer(
            Box::new(printer::SharedEmitTextWriterHandle::new(writer.clone())),
        ));
        p.write_node(sig.as_ref(), source_file.as_ref(), deferred_writer, None);
        let result = writer.borrow().string();
        result
    }

    pub(crate) fn type_predicate_to_string(
        &mut self,
        type_predicate: TypePredicateHandle,
    ) -> String {
        self.type_predicate_to_string_ex(
            type_predicate,
            None,
            TYPE_FORMAT_FLAGS_USE_ALIAS_DEFINED_OUTSIDE_CURRENT_SCOPE,
        )
    }

    fn type_predicate_to_string_ex(
        &mut self,
        type_predicate: TypePredicateHandle,
        enclosing_declaration: Option<ast::Node>,
        flags: TypeFormatFlags,
    ) -> String {
        let mut writer = printer::get_single_line_string_writer();
        let source_file = source_file_for_emit(self, enclosing_declaration);
        let binding_facts = binding_facts_for_emit(self, source_file.as_ref());
        let mut node_builder = self.get_node_builder();
        let combined_flags = to_node_builder_flags(flags)
            | nodebuilder::FLAGS_IGNORE_ERRORS
            | nodebuilder::FLAGS_WRITE_TYPE_PARAMETERS_IN_QUALIFIED_NAME;
        let predicate = node_builder.type_predicate_to_type_predicate_node(
            type_predicate,
            enclosing_declaration,
            combined_flags,
            nodebuilder::INTERNAL_FLAGS_NONE,
            None,
        ); // TODO: GH#18217
        let predicate = node_builder.prepare_optional_node_for_emit(predicate);
        let mut printer_ =
            create_printer_with_remove_comments(node_builder.emit_context(), binding_facts);
        let writer = printer::share_text_writer(writer);
        printer_.write_node(
            predicate.as_ref(), /*sourceFile*/
            source_file.as_ref(),
            writer.clone(),
            None,
        );
        let result = writer.borrow().string();
        result
    }

    pub(crate) fn value_to_string(&self, value: crate::evaluator::Value) -> String {
        match value {
            crate::evaluator::Value::String(value) => {
                format!(
                    "\"{}\"",
                    printer::escape_string(value, printer::QuoteChar::DoubleQuote)
                )
            }
            crate::evaluator::Value::Number(value) => value.to_string(),
            crate::evaluator::Value::Bool(value) => {
                if value { "true" } else { "false" }.to_string()
            }
            crate::evaluator::Value::PseudoBigInt(value) => value.to_string() + "n",
            crate::evaluator::Value::None => "undefined".to_string(),
        }
    }

    pub(crate) fn format_union_types(
        &mut self,
        types: Vec<TypeHandle>,
        expanding_enum: bool,
    ) -> Vec<TypeHandle> {
        let mut result = Vec::new();
        let mut flags = TYPE_FLAGS_NONE;
        let mut i = 0;
        while i < types.len() {
            let t = types[i];
            let type_flags = self.type_flags(t);
            flags |= type_flags;
            if type_flags & TYPE_FLAGS_NULLABLE == 0 {
                if type_flags & TYPE_FLAGS_BOOLEAN_LITERAL != 0
                    || (!expanding_enum && type_flags & TYPE_FLAGS_ENUM_LIKE != 0)
                {
                    let base_type = if type_flags & TYPE_FLAGS_BOOLEAN_LITERAL != 0 {
                        self.semantic_state.semantic_handles().boolean_type
                    } else {
                        self.get_base_type_of_enum_like_type(t)
                    };
                    if self.type_flags(base_type) & TYPE_FLAGS_UNION != 0 {
                        let base_types = self.type_types(base_type);
                        let count = base_types.len();
                        if i + count <= types.len()
                            && self.get_regular_type_of_literal_type(types[i + count - 1])
                                == self.get_regular_type_of_literal_type(base_types[count - 1])
                        {
                            result.push(base_type);
                            i += count;
                            continue;
                        }
                    }
                }
                result.push(t);
            }
            i += 1;
        }
        if flags & TYPE_FLAGS_NULL != 0 {
            result.push(self.semantic_state.semantic_handles().null_type);
        }
        if flags & TYPE_FLAGS_UNDEFINED != 0 {
            result.push(self.semantic_state.semantic_handles().undefined_type);
        }
        result
    }

    pub fn type_to_type_node(
        &mut self,
        t: TypeHandle,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        id_to_symbol: Option<HashMap<ast::IdentifierNode, SymbolIdentity>>,
    ) -> Option<ast::TypeNode> {
        let mut node_builder = self.get_node_builder_ex(id_to_symbol);
        node_builder.type_to_type_node(
            t,
            enclosing_declaration,
            flags,
            nodebuilder::INTERNAL_FLAGS_NONE,
            None,
        )
    }

    pub fn signature_to_signature_declaration(
        &mut self,
        signature: SignatureHandle,
        kind: ast::Kind,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
    ) -> Option<ast::Node> {
        let mut node_builder = self.get_node_builder();
        node_builder.signature_to_signature_declaration(
            signature,
            kind,
            enclosing_declaration,
            flags,
            nodebuilder::INTERNAL_FLAGS_NONE,
            None,
        )
    }

    // ExpandSymbolForHover produces declaration strings for a symbol with verbosity support for expandable hover.
    pub(crate) fn expand_symbol_identity_for_hover(
        &mut self,
        symbol: SymbolIdentity,
        meaning: ast::SymbolFlags,
        vc: Option<&mut VerbosityContext>,
    ) -> String {
        let source_file = self
            .missing_name_symbol_identity_value_declaration(symbol)
            .and_then(|node| source_file_from_node(self, node));
        let binding_facts = binding_facts_for_emit(self, source_file.as_ref());
        let mut node_builder = self.get_node_builder();
        let mut vc = vc;
        if let Some(ref vc) = vc {
            node_builder.set_verbosity(vc);
        }
        let nodes = node_builder.expand_symbol_identity_for_hover(symbol, meaning);
        if let Some(vc) = vc.as_deref_mut() {
            node_builder.write_verbosity(vc);
        }
        if nodes.is_empty() {
            return String::new();
        }
        let nodes = node_builder.prepare_nodes_for_emit(nodes);
        let mut p = create_printer_with_remove_comments(node_builder.emit_context(), binding_facts);
        let mut b = String::new();
        for (i, node) in nodes.iter().enumerate() {
            if i > 0 {
                b.push('\n');
            }
            b.push_str(&p.emit(node, source_file.as_ref()));
        }
        b
    }

    // TypeParameterToStringEx renders a type parameter declaration (e.g. "T extends Foo") with optional verbosity support.
    pub fn type_parameter_to_string_ex(
        &mut self,
        t: TypeHandle,
        enclosing_declaration: Option<ast::Node>,
        vc: Option<&mut VerbosityContext>,
    ) -> String {
        let source_file = source_file_for_emit(self, enclosing_declaration);
        let binding_facts = binding_facts_for_emit(self, source_file.as_ref());
        let mut node_builder = self.get_node_builder();
        let mut vc = vc;
        if let Some(ref vc) = vc {
            node_builder.set_verbosity(vc);
        }
        let type_param_node = node_builder.type_parameter_to_declaration(
            t,
            enclosing_declaration,
            nodebuilder::FLAGS_IGNORE_ERRORS,
            nodebuilder::INTERNAL_FLAGS_NONE,
            None,
        );
        if let Some(vc) = vc.as_deref_mut() {
            node_builder.write_verbosity(vc);
        }
        if type_param_node.is_none() {
            return self.type_to_string_public(t);
        }
        let type_param_node = node_builder.prepare_optional_node_for_emit(type_param_node);
        let mut p = create_printer_with_remove_comments(node_builder.emit_context(), binding_facts);
        p.emit(&type_param_node.unwrap(), source_file.as_ref())
    }

    pub fn type_to_type_node_ex(
        &mut self,
        t: TypeHandle,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        id_to_symbol: Option<HashMap<ast::IdentifierNode, SymbolIdentity>>,
    ) -> Option<ast::TypeNode> {
        let mut node_builder = self.get_node_builder_ex(id_to_symbol);
        node_builder.type_to_type_node(t, enclosing_declaration, flags, internal_flags, None)
    }

    pub fn type_predicate_to_type_predicate_node(
        &mut self,
        t: TypePredicateHandle,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        id_to_symbol: Option<HashMap<ast::IdentifierNode, SymbolIdentity>>,
    ) -> Option<ast::TypePredicateNodeNode> {
        let mut node_builder = self.get_node_builder_ex(id_to_symbol);
        node_builder.type_predicate_to_type_predicate_node(
            t,
            enclosing_declaration,
            flags,
            nodebuilder::INTERNAL_FLAGS_NONE,
            None,
        )
    }
}
