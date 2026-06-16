pub mod diagnostics;
pub mod tracker;
pub mod transform;
pub mod util;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use ts_ast as ast;
use ts_core as core;
use ts_diagnostics as diagnostic_messages;
use ts_evaluator as evaluator;
use ts_nodebuilder as nodebuilder;
use ts_outputpaths as outputpaths;
use ts_printer as printer;
use ts_scanner as scanner;
use ts_tspath as tspath;

use crate::declarations::transform::{DeclarationSourceFileState, DeclarationTransformFacts};

pub trait DeclarationEmitHost {
    fn get_effective_declaration_flags(
        &mut self,
        node: ast::Node,
        flags: ast::ModifierFlags,
    ) -> ast::ModifierFlags;
    fn is_declaration_visible(&mut self, node: ast::Node) -> bool;
    fn is_literal_const_declaration(&mut self, node: ast::Node) -> bool;
    fn is_import_required_by_augmentation(&mut self, decl: ast::Node) -> bool;
    fn create_literal_const_value(
        &mut self,
        emit_context: &mut printer::EmitContext,
        node: ast::Node,
    ) -> Option<ast::Node>;
    fn get_enum_member_value(&mut self, node: ast::Node) -> evaluator::Result;
    fn create_type_of_expression(
        &mut self,
        emit_context: &mut printer::EmitContext,
        expression: ast::Node,
        enclosing_declaration: ast::Node,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Option<ast::Node>;
    fn create_type_of_declaration(
        &mut self,
        emit_context: &mut printer::EmitContext,
        declaration: ast::Node,
        enclosing_declaration: ast::Node,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Option<ast::Node>;
    fn get_declaration_statements_for_source_file(
        &mut self,
        emit_context: &mut printer::EmitContext,
        source_file: ast::Node,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Vec<ast::Node>;
    fn create_return_type_of_signature_declaration(
        &mut self,
        emit_context: &mut printer::EmitContext,
        declaration: ast::Node,
        enclosing_declaration: ast::Node,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Option<ast::Node>;
    fn create_late_bound_index_signatures(
        &mut self,
        emit_context: &mut printer::EmitContext,
        container: ast::Node,
        enclosing_declaration: ast::Node,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Vec<ast::Node>;
    fn create_signature_declaration_with_synthetic_rest_parameter(
        &mut self,
        emit_context: &mut printer::EmitContext,
        declaration: ast::Node,
        kind: ast::Kind,
        modifiers: Vec<ast::Node>,
        name: Option<ast::Node>,
        enclosing_declaration: ast::Node,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Option<ast::Node>;
    fn is_entity_name_visible(
        &mut self,
        entity_name: ast::Node,
        enclosing_declaration: ast::Node,
    ) -> printer::SymbolAccessibilityResult;
    fn is_definitely_reference_to_global_symbol_object(&mut self, node: ast::Node) -> bool;
    fn is_late_bound(&mut self, node: ast::Node) -> bool;
    fn is_optional_parameter(&mut self, node: ast::Node) -> bool;
    fn requires_adding_implicit_undefined(
        &mut self,
        node: ast::Node,
        enclosing_declaration: ast::Node,
    ) -> bool;
    fn is_implementation_of_overload(&mut self, node: ast::Node) -> bool;
    fn is_expando_function_declaration(&mut self, node: ast::Node) -> bool;
    fn should_emit_function_properties(&mut self, node: ast::Node) -> bool;
    fn get_properties_of_container_function(&mut self, node: ast::Node)
    -> Vec<ast::SymbolIdentity>;
    fn is_assignment_declaration(&mut self, node: ast::Node) -> bool;
    fn get_symbol_name(&mut self, symbol: ast::SymbolIdentity) -> String;
    fn get_symbol_value_declaration(&mut self, symbol: ast::SymbolIdentity) -> Option<ast::Node>;
    fn get_referenced_value_declaration(&mut self, node: ast::Node) -> Option<ast::Node>;
    fn get_referenced_member_value_declaration(&mut self, node: ast::Node) -> Option<ast::Node>;
    fn get_element_access_expression_name(&mut self, expression: ast::Node) -> String;
    fn set_expando_namespace_metadata(
        &mut self,
        synthesized_namespace: ast::Node,
        declaration: ast::Node,
        properties: &[ast::SymbolIdentity],
    );
    fn is_first_declaration_of_symbol(&mut self, node: ast::Node) -> bool;
    fn precalculate_declaration_emit_visibility(&mut self, file: &ast::SourceFile);
    fn is_common_js_alias_export(&mut self, node: ast::Node) -> bool;
    fn source_file_is_external_or_common_js_module(&self, file: &ast::SourceFile) -> bool;
    fn source_file_common_js_module_indicator(&self, file: &ast::SourceFile) -> Option<ast::Node>;
    fn source_file_export_equals_declarations(&self, file: &ast::SourceFile) -> Vec<ast::Node>;
    fn source_file_nested_cjs_exports(&self, file: &ast::SourceFile) -> Vec<ast::Node>;
    fn get_current_directory(&self) -> String;
    fn use_case_sensitive_file_names(&self) -> bool;
    fn get_resolution_mode_override(&mut self, node: ast::Node) -> core::ResolutionMode;
    fn get_source_file_from_reference(
        &self,
        origin: &ast::SourceFile,
        r#ref: &ast::FileReference,
    ) -> Option<ast::SourceFile>;
    fn get_output_paths_for(
        &self,
        file: &ast::SourceFile,
        force_dts_paths: bool,
    ) -> outputpaths::OutputPaths;
}

impl<T: printer::EmitHost + outputpaths::OutputPathsHost> DeclarationEmitHost for T {
    fn get_effective_declaration_flags(
        &mut self,
        node: ast::Node,
        flags: ast::ModifierFlags,
    ) -> ast::ModifierFlags {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(resolver.get_effective_declaration_flags(node, flags));
        });
        result.expect("emit resolver callback must run")
    }

    fn is_declaration_visible(&mut self, node: ast::Node) -> bool {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(resolver.is_declaration_visible(node));
        });
        result.expect("emit resolver callback must run")
    }

    fn is_literal_const_declaration(&mut self, node: ast::Node) -> bool {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(resolver.is_literal_const_declaration(node));
        });
        result.expect("emit resolver callback must run")
    }

    fn is_import_required_by_augmentation(&mut self, decl: ast::Node) -> bool {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(resolver.is_import_required_by_augmentation(decl));
        });
        result.expect("emit resolver callback must run")
    }

    fn is_definitely_reference_to_global_symbol_object(&mut self, node: ast::Node) -> bool {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(resolver.is_definitely_reference_to_global_symbol_object(node));
        });
        result.expect("emit resolver callback must run")
    }

    fn create_literal_const_value(
        &mut self,
        emit_context: &mut printer::EmitContext,
        node: ast::Node,
    ) -> Option<ast::Node> {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(resolver.create_literal_const_value(
                emit_context,
                node,
                Box::new(NullSymbolTracker),
            ));
        });
        result.expect("emit resolver callback must run")
    }

    fn get_enum_member_value(&mut self, node: ast::Node) -> evaluator::Result {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(resolver.get_enum_member_value(node));
        });
        result.expect("emit resolver callback must run")
    }

    fn create_type_of_expression(
        &mut self,
        emit_context: &mut printer::EmitContext,
        expression: ast::Node,
        enclosing_declaration: ast::Node,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Option<ast::Node> {
        let mut tracker = Some(tracker);
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = resolver.create_type_of_expression(
                emit_context,
                expression,
                enclosing_declaration,
                declaration_emit_node_builder_flags(),
                declaration_emit_internal_node_builder_flags()
                    | nodebuilder::INTERNAL_FLAGS_NO_SYNTACTIC_PRINTER,
                tracker
                    .take()
                    .expect("emit resolver callback must run once"),
            );
        });
        result
    }

    fn create_type_of_declaration(
        &mut self,
        emit_context: &mut printer::EmitContext,
        declaration: ast::Node,
        enclosing_declaration: ast::Node,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Option<ast::Node> {
        let mut tracker = Some(tracker);
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(
                resolver.create_type_of_declaration(
                    emit_context,
                    declaration,
                    enclosing_declaration,
                    declaration_emit_node_builder_flags(),
                    internal_flags,
                    tracker
                        .take()
                        .expect("emit resolver callback must run once"),
                ),
            );
        });
        result.expect("emit resolver callback must run")
    }

    fn get_declaration_statements_for_source_file(
        &mut self,
        emit_context: &mut printer::EmitContext,
        source_file: ast::Node,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Vec<ast::Node> {
        let mut tracker = Some(tracker);
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(
                resolver.get_declaration_statements_for_source_file(
                    emit_context,
                    source_file,
                    declaration_emit_node_builder_flags(),
                    declaration_emit_internal_node_builder_flags(),
                    tracker
                        .take()
                        .expect("emit resolver callback must run once"),
                ),
            );
        });
        result.expect("emit resolver callback must run")
    }

    fn create_return_type_of_signature_declaration(
        &mut self,
        emit_context: &mut printer::EmitContext,
        declaration: ast::Node,
        enclosing_declaration: ast::Node,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Option<ast::Node> {
        let mut tracker = Some(tracker);
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(
                resolver.create_return_type_of_signature_declaration(
                    emit_context,
                    declaration,
                    enclosing_declaration,
                    declaration_emit_node_builder_flags(),
                    declaration_emit_internal_node_builder_flags(),
                    tracker
                        .take()
                        .expect("emit resolver callback must run once"),
                ),
            );
        });
        result.expect("emit resolver callback must run")
    }

    fn create_late_bound_index_signatures(
        &mut self,
        emit_context: &mut printer::EmitContext,
        container: ast::Node,
        enclosing_declaration: ast::Node,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Vec<ast::Node> {
        let mut result = None;
        let mut tracker = Some(tracker);
        self.with_emit_resolver(&mut |resolver| {
            result = Some(
                resolver.create_late_bound_index_signatures(
                    emit_context,
                    container,
                    enclosing_declaration,
                    declaration_emit_node_builder_flags(),
                    declaration_emit_internal_node_builder_flags(),
                    tracker
                        .take()
                        .expect("emit resolver callback must run once"),
                ),
            );
        });
        result.expect("emit resolver callback must run")
    }

    fn create_signature_declaration_with_synthetic_rest_parameter(
        &mut self,
        emit_context: &mut printer::EmitContext,
        declaration: ast::Node,
        kind: ast::Kind,
        modifiers: Vec<ast::Node>,
        name: Option<ast::Node>,
        enclosing_declaration: ast::Node,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Option<ast::Node> {
        let mut tracker = Some(tracker);
        let mut modifiers = Some(modifiers);
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = resolver.create_signature_declaration_with_synthetic_rest_parameter(
                emit_context,
                declaration,
                kind,
                modifiers
                    .take()
                    .expect("emit resolver callback must run once"),
                name,
                enclosing_declaration,
                declaration_emit_node_builder_flags(),
                declaration_emit_internal_node_builder_flags(),
                tracker
                    .take()
                    .expect("emit resolver callback must run once"),
            );
        });
        result
    }

    fn is_entity_name_visible(
        &mut self,
        entity_name: ast::Node,
        enclosing_declaration: ast::Node,
    ) -> printer::SymbolAccessibilityResult {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(resolver.is_entity_name_visible(entity_name, enclosing_declaration));
        });
        result.expect("emit resolver callback must run")
    }

    fn is_late_bound(&mut self, node: ast::Node) -> bool {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(resolver.is_late_bound(node));
        });
        result.expect("emit resolver callback must run")
    }

    fn is_optional_parameter(&mut self, node: ast::Node) -> bool {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(resolver.is_optional_parameter(node));
        });
        result.expect("emit resolver callback must run")
    }

    fn requires_adding_implicit_undefined(
        &mut self,
        node: ast::Node,
        enclosing_declaration: ast::Node,
    ) -> bool {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(
                resolver.requires_adding_implicit_undefined(node, Some(enclosing_declaration)),
            );
        });
        result.expect("emit resolver callback must run")
    }

    fn is_implementation_of_overload(&mut self, node: ast::Node) -> bool {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(resolver.is_implementation_of_overload(node));
        });
        result.expect("emit resolver callback must run")
    }

    fn is_expando_function_declaration(&mut self, node: ast::Node) -> bool {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(resolver.is_expando_function_declaration(node));
        });
        result.expect("emit resolver callback must run")
    }

    fn should_emit_function_properties(&mut self, node: ast::Node) -> bool {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(resolver.should_emit_function_properties(node));
        });
        result.expect("emit resolver callback must run")
    }

    fn get_properties_of_container_function(
        &mut self,
        node: ast::Node,
    ) -> Vec<ast::SymbolIdentity> {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(resolver.get_properties_of_container_function(node));
        });
        result.expect("emit resolver callback must run")
    }

    fn get_symbol_name(&mut self, symbol: ast::SymbolIdentity) -> String {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(resolver.get_symbol_name(symbol));
        });
        result.expect("emit resolver callback must run")
    }

    fn get_symbol_value_declaration(&mut self, symbol: ast::SymbolIdentity) -> Option<ast::Node> {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(resolver.get_symbol_value_declaration(symbol));
        });
        result.expect("emit resolver callback must run")
    }

    fn is_assignment_declaration(&mut self, node: ast::Node) -> bool {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(resolver.is_assignment_declaration(node));
        });
        result.expect("emit resolver callback must run")
    }

    fn get_referenced_value_declaration(&mut self, node: ast::Node) -> Option<ast::Node> {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(resolver.get_referenced_value_declaration(node));
        });
        result.expect("emit resolver callback must run")
    }

    fn get_referenced_member_value_declaration(&mut self, node: ast::Node) -> Option<ast::Node> {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(resolver.get_referenced_member_value_declaration(node));
        });
        result.expect("emit resolver callback must run")
    }

    fn get_element_access_expression_name(&mut self, expression: ast::Node) -> String {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(printer::EmitResolver::get_element_access_expression_name(
                resolver, expression,
            ));
        });
        result.expect("emit resolver callback must run")
    }

    fn set_expando_namespace_metadata(
        &mut self,
        synthesized_namespace: ast::Node,
        declaration: ast::Node,
        properties: &[ast::SymbolIdentity],
    ) {
        self.with_emit_resolver(&mut |resolver| {
            resolver.set_expando_namespace_metadata(synthesized_namespace, declaration, properties);
        });
    }

    fn is_first_declaration_of_symbol(&mut self, node: ast::Node) -> bool {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(resolver.is_first_declaration_of_symbol(node));
        });
        result.expect("emit resolver callback must run")
    }

    fn precalculate_declaration_emit_visibility(&mut self, file: &ast::SourceFile) {
        self.with_emit_resolver(&mut |resolver| {
            resolver.precalculate_declaration_emit_visibility(file);
        });
    }

    fn is_common_js_alias_export(&mut self, node: ast::Node) -> bool {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(resolver.is_common_js_alias_export(node));
        });
        result.expect("emit resolver callback must run")
    }

    fn source_file_is_external_or_common_js_module(&self, file: &ast::SourceFile) -> bool {
        printer::EmitHost::source_file_external_module_indicator(self, file).is_some()
            || printer::EmitHost::source_file_common_js_module_indicator(self, file).is_some()
    }

    fn source_file_common_js_module_indicator(&self, file: &ast::SourceFile) -> Option<ast::Node> {
        printer::EmitHost::source_file_common_js_module_indicator(self, file)
    }

    fn source_file_export_equals_declarations(&self, file: &ast::SourceFile) -> Vec<ast::Node> {
        printer::EmitHost::source_file_export_equals_declarations(self, file)
    }

    fn source_file_nested_cjs_exports(&self, file: &ast::SourceFile) -> Vec<ast::Node> {
        printer::EmitHost::source_file_nested_cjs_exports(self, file)
    }

    fn get_current_directory(&self) -> String {
        printer::EmitHost::get_current_directory(self)
    }

    fn use_case_sensitive_file_names(&self) -> bool {
        printer::EmitHost::use_case_sensitive_file_names(self)
    }

    fn get_resolution_mode_override(&mut self, node: ast::Node) -> core::ResolutionMode {
        let mut result = None;
        self.with_emit_resolver(&mut |resolver| {
            result = Some(resolver.get_resolution_mode_override(node));
        });
        result.expect("emit resolver callback must run")
    }

    fn get_source_file_from_reference(
        &self,
        origin: &ast::SourceFile,
        r#ref: &ast::FileReference,
    ) -> Option<ast::SourceFile> {
        printer::EmitHost::get_source_file_from_reference(self, origin, r#ref)
    }

    fn get_output_paths_for(
        &self,
        file: &ast::SourceFile,
        force_dts_paths: bool,
    ) -> outputpaths::OutputPaths {
        let output_source_file = file.share_readonly();
        let output_options = self.options().unwrap_or_default();
        outputpaths::get_output_paths_for(
            &output_source_file,
            &output_options,
            self,
            force_dts_paths,
        )
    }
}

struct NullSymbolTracker;

impl nodebuilder::SymbolTracker for NullSymbolTracker {
    fn track_symbol(
        &mut self,
        _symbol: nodebuilder::SymbolIdentity,
        _symbol_flags: nodebuilder::SymbolFlags,
        _enclosing_declaration: Option<nodebuilder::Node>,
        _meaning: nodebuilder::SymbolFlags,
    ) -> bool {
        false
    }

    fn report_inaccessible_this_error(&mut self) {}

    fn report_private_in_base_of_class_expression(&mut self, _property_name: &str) {}

    fn report_inaccessible_unique_symbol_error(&mut self) {}

    fn report_cyclic_structure_error(&mut self) {}

    fn report_likely_unsafe_import_required_error(&mut self, _specifier: &str, _symbol_name: &str) {
    }

    fn report_truncation_error(&mut self) {}

    fn report_nonlocal_augmentation(
        &mut self,
        _containing_file: &nodebuilder::SourceFile,
        _parent_symbol: nodebuilder::SymbolIdentity,
        _augmenting_symbol: nodebuilder::SymbolIdentity,
    ) {
    }

    fn report_non_serializable_property(&mut self, _property_name: &str) {}

    fn report_inference_fallback(&mut self, _node: nodebuilder::Node) {}

    fn push_error_fallback_node(&mut self, _node: nodebuilder::Node) {}

    fn pop_error_fallback_node(&mut self) {}
}

struct DeclarationDiagnosticTracker {
    diagnostics: Rc<RefCell<Vec<ast::Diagnostic>>>,
    late_marked_statements: Rc<RefCell<Vec<ast::Node>>>,
    isolated_declarations: bool,
    inference_fallback_expando_diagnostics: Option<Vec<ast::Diagnostic>>,
    file: Option<ast::DiagnosticFile>,
    loc: core::TextRange,
    declaration_name: String,
    declaration_kind: ast::Kind,
    declaration: ast::Node,
    declaration_requires_implicit_undefined: bool,
    parameters_requiring_implicit_undefined: Vec<ast::Node>,
    error_name_node: Option<ast::Node>,
    error_fallback_nodes: Vec<ast::Node>,
}

struct SymbolAccessibilityDiagnosticTarget {
    kind: ast::Kind,
    parent_kind: Option<ast::Kind>,
    is_static: bool,
    file: Option<ast::DiagnosticFile>,
    loc: core::TextRange,
    declaration_name: String,
}

impl DeclarationDiagnosticTracker {
    fn new(
        diagnostics: Rc<RefCell<Vec<ast::Diagnostic>>>,
        late_marked_statements: Rc<RefCell<Vec<ast::Node>>>,
        isolated_declarations: bool,
        source: &ast::AstStore,
        declaration: ast::Node,
    ) -> Self {
        Self::new_with_error_fallback_node(
            diagnostics,
            late_marked_statements,
            isolated_declarations,
            source,
            declaration,
            None,
        )
    }

    fn new_with_error_fallback_node(
        diagnostics: Rc<RefCell<Vec<ast::Diagnostic>>>,
        late_marked_statements: Rc<RefCell<Vec<ast::Node>>>,
        isolated_declarations: bool,
        source: &ast::AstStore,
        declaration: ast::Node,
        error_fallback_node: Option<ast::Node>,
    ) -> Self {
        let name = ast::get_name_of_declaration(source, Some(declaration));
        let error_node = name.unwrap_or(declaration);
        let source_file = ast::get_source_file_of_node(source, Some(error_node));
        let (file, loc, declaration_name) = if let Some(source_file) = source_file {
            let source_file = source.source_file_view(source_file);
            (
                Some(ast::DiagnosticFile::from_source_file_view(&source_file)),
                scanner::get_error_range_for_node(&source_file, &declaration),
                scanner::declaration_name_to_string(&source_file, name.as_ref()),
            )
        } else {
            (None, source.loc(error_node), "(Missing)".to_owned())
        };
        Self {
            diagnostics,
            late_marked_statements,
            isolated_declarations,
            inference_fallback_expando_diagnostics: None,
            file,
            loc,
            declaration_name,
            declaration_kind: source.kind(declaration),
            declaration,
            declaration_requires_implicit_undefined: false,
            parameters_requiring_implicit_undefined: Vec::new(),
            error_name_node: name,
            error_fallback_nodes: error_fallback_node.into_iter().collect(),
        }
    }

    fn with_declaration_requires_implicit_undefined(
        mut self,
        requires_implicit_undefined: bool,
    ) -> Self {
        self.declaration_requires_implicit_undefined = requires_implicit_undefined;
        self
    }

    fn with_parameters_requiring_implicit_undefined(mut self, parameters: Vec<ast::Node>) -> Self {
        self.parameters_requiring_implicit_undefined = parameters;
        self
    }

    fn with_inference_fallback_expando_diagnostics(
        mut self,
        diagnostics: Vec<ast::Diagnostic>,
    ) -> Self {
        self.inference_fallback_expando_diagnostics = Some(diagnostics);
        self
    }

    fn error_target(&self) -> Option<ast::Node> {
        self.error_name_node
            .or_else(|| self.error_fallback_nodes.last().copied())
    }

    fn error_context(&self) -> Option<(Option<ast::DiagnosticFile>, core::TextRange, String)> {
        let target = self.error_target()?;
        let Some(file) = &self.file else {
            return Some((None, self.loc, self.declaration_name.clone()));
        };
        let Some(view) = file.upgrade() else {
            return Some((self.file.clone(), self.loc, self.declaration_name.clone()));
        };
        let source_file = view.source_file_view();
        if ast::get_source_file_of_node(source_file.store(), Some(target)) != Some(view.root()) {
            return Some((self.file.clone(), self.loc, self.declaration_name.clone()));
        }

        let name = if self.error_name_node == Some(target) {
            Some(target)
        } else {
            ast::get_name_of_declaration(source_file.store(), Some(target))
        };
        let declaration_name = if let Some(name) = name {
            scanner::declaration_name_to_string(&source_file, Some(&name))
        } else if ast::is_export_assignment(source_file.store(), target) {
            if source_file
                .store()
                .is_export_equals(target)
                .unwrap_or_default()
            {
                "export=".to_owned()
            } else {
                "default".to_owned()
            }
        } else {
            "(Missing)".to_owned()
        };

        Some((
            self.file.clone(),
            scanner::get_error_range_for_node(&source_file, &target),
            declaration_name,
        ))
    }

    fn add_diagnostic_with_args(
        &mut self,
        message: &diagnostic_messages::Message,
        args: &[diagnostic_messages::Argument],
    ) {
        let Some((file, loc, _)) = self.error_context() else {
            return;
        };
        self.diagnostics
            .borrow_mut()
            .push(ast::new_diagnostic_with_file(file, loc, message, args));
    }

    fn add_inferred_type_diagnostic(&mut self, message: &diagnostic_messages::Message) {
        let Some((file, loc, declaration_name)) = self.error_context() else {
            return;
        };
        let args = vec![diagnostic_messages::Argument::from(declaration_name)];
        self.diagnostics
            .borrow_mut()
            .push(ast::new_diagnostic_with_file(file, loc, message, &args));
    }

    fn create_diagnostic_for_node(
        &self,
        source_file: &impl ast::SourceFileStoreLike,
        node: ast::Node,
        message: &diagnostic_messages::Message,
        args: &[diagnostic_messages::Argument],
    ) -> ast::Diagnostic {
        ast::new_diagnostic_with_file(
            self.file.clone(),
            scanner::get_error_range_for_node(source_file, &node),
            message,
            args,
        )
    }

    fn isolated_declaration_message_for_kind(
        kind: ast::Kind,
    ) -> Option<&'static diagnostic_messages::Message> {
        Some(match kind {
            ast::Kind::FunctionExpression
            | ast::Kind::FunctionDeclaration
            | ast::Kind::ArrowFunction => {
                &diagnostic_messages::Function_must_have_an_explicit_return_type_annotation_with_isolatedDeclarations
            }
            ast::Kind::MethodDeclaration | ast::Kind::ConstructSignature => {
                &diagnostic_messages::Method_must_have_an_explicit_return_type_annotation_with_isolatedDeclarations
            }
            ast::Kind::GetAccessor | ast::Kind::SetAccessor => {
                &diagnostic_messages::At_least_one_accessor_must_have_an_explicit_type_annotation_with_isolatedDeclarations
            }
            ast::Kind::Parameter => {
                &diagnostic_messages::Parameter_must_have_an_explicit_type_annotation_with_isolatedDeclarations
            }
            ast::Kind::VariableDeclaration => {
                &diagnostic_messages::Variable_must_have_an_explicit_type_annotation_with_isolatedDeclarations
            }
            ast::Kind::PropertyDeclaration | ast::Kind::PropertySignature => {
                &diagnostic_messages::Property_must_have_an_explicit_type_annotation_with_isolatedDeclarations
            }
            ast::Kind::ComputedPropertyName => {
                &diagnostic_messages::Computed_property_names_on_class_or_object_literals_cannot_be_inferred_with_isolatedDeclarations
            }
            ast::Kind::SpreadAssignment => {
                &diagnostic_messages::Objects_that_contain_spread_assignments_can_t_be_inferred_with_isolatedDeclarations
            }
            ast::Kind::ShorthandPropertyAssignment => {
                &diagnostic_messages::Objects_that_contain_shorthand_properties_can_t_be_inferred_with_isolatedDeclarations
            }
            ast::Kind::ArrayLiteralExpression => {
                &diagnostic_messages::Only_const_arrays_can_be_inferred_with_isolatedDeclarations
            }
            ast::Kind::ExportAssignment => {
                &diagnostic_messages::Default_exports_can_t_be_inferred_with_isolatedDeclarations
            }
            ast::Kind::SpreadElement => {
                &diagnostic_messages::Arrays_with_spread_elements_can_t_inferred_with_isolatedDeclarations
            }
            _ => return None,
        })
    }

    fn related_suggestion_message_for_kind(
        kind: ast::Kind,
    ) -> Option<&'static diagnostic_messages::Message> {
        Some(match kind {
            ast::Kind::ArrowFunction | ast::Kind::FunctionExpression => {
                &diagnostic_messages::Add_a_return_type_to_the_function_expression
            }
            ast::Kind::MethodDeclaration => &diagnostic_messages::Add_a_return_type_to_the_method,
            ast::Kind::GetAccessor => {
                &diagnostic_messages::Add_a_return_type_to_the_get_accessor_declaration
            }
            ast::Kind::SetAccessor => {
                &diagnostic_messages::Add_a_type_to_parameter_of_the_set_accessor_declaration
            }
            ast::Kind::FunctionDeclaration | ast::Kind::ConstructSignature => {
                &diagnostic_messages::Add_a_return_type_to_the_function_declaration
            }
            ast::Kind::Parameter => {
                &diagnostic_messages::Add_a_type_annotation_to_the_parameter_0
            }
            ast::Kind::VariableDeclaration => {
                &diagnostic_messages::Add_a_type_annotation_to_the_variable_0
            }
            ast::Kind::PropertyDeclaration | ast::Kind::PropertySignature => {
                &diagnostic_messages::Add_a_type_annotation_to_the_property_0
            }
            ast::Kind::ExportAssignment => {
                &diagnostic_messages::Move_the_expression_in_default_export_to_a_variable_and_add_a_type_annotation_to_it
            }
            _ => return None,
        })
    }

    fn is_declaration_enough_for_errors(source: &ast::AstStore, node: ast::Node) -> bool {
        ast::is_export_assignment(source, node)
            || ast::is_statement(source, node)
            || ast::is_variable_declaration(source, node)
            || ast::is_property_declaration(source, node)
            || ast::is_parameter_declaration(source, node)
    }

    fn find_nearest_declaration(source: &ast::AstStore, node: ast::Node) -> Option<ast::Node> {
        let mut current = Some(node);
        while let Some(node) = current {
            if Self::is_declaration_enough_for_errors(source, node) {
                if ast::is_export_assignment(source, node) {
                    return Some(node);
                }
                if ast::is_return_statement(source, node) {
                    let mut ancestor = source.parent(node);
                    while let Some(ancestor_node) = ancestor {
                        if ast::is_function_like(source, Some(ancestor_node))
                            && source.kind(ancestor_node) != ast::Kind::Constructor
                        {
                            return Some(ancestor_node);
                        }
                        ancestor = source.parent(ancestor_node);
                    }
                    return None;
                }
                if ast::is_statement(source, node) {
                    return None;
                }
                return Some(node);
            }
            current = source.parent(node);
        }
        None
    }

    fn add_parent_declaration_related_info(
        &self,
        source_file: &impl ast::SourceFileStoreLike,
        node: ast::Node,
        diagnostic: &mut ast::Diagnostic,
    ) {
        let source = source_file.store();
        let Some(parent_declaration) = Self::find_nearest_declaration(source, node) else {
            return;
        };
        let Some(message) =
            Self::related_suggestion_message_for_kind(source.kind(parent_declaration))
        else {
            return;
        };
        let target = if !ast::is_export_assignment(source, parent_declaration) {
            ast::get_name_of_declaration(source, Some(parent_declaration))
                .map(|name| scanner::get_text_of_node(source_file, &name))
                .unwrap_or_default()
        } else {
            String::new()
        };
        let args = if target.is_empty() {
            Vec::new()
        } else {
            vec![diagnostic_messages::Argument::from(target)]
        };
        diagnostic.add_related_info(self.create_diagnostic_for_node(
            source_file,
            parent_declaration,
            message,
            &args,
        ));
    }

    fn create_entity_in_type_node_error(
        &self,
        source_file: &impl ast::SourceFileStoreLike,
        node: ast::Node,
    ) -> ast::Diagnostic {
        let text = scanner::get_text_of_node(source_file, &node);
        let mut diagnostic = self.create_diagnostic_for_node(
            source_file,
            node,
            &diagnostic_messages::Type_containing_private_name_0_can_t_be_used_with_isolatedDeclarations,
            &[diagnostic_messages::Argument::from(text)],
        );
        self.add_parent_declaration_related_info(source_file, node, &mut diagnostic);
        diagnostic
    }

    fn is_parent_for_id_diagnostic(source: &ast::AstStore, node: ast::Node) -> Option<ast::Node> {
        let mut current = source.parent(node);
        while let Some(parent) = current {
            if ast::is_export_assignment(source, parent) {
                return Some(parent);
            }
            if ast::is_statement(source, parent) {
                return None;
            }
            if !ast::is_parenthesized_expression(source, parent)
                && !ast::is_assertion_expression(source, parent)
            {
                return Some(parent);
            }
            current = source.parent(parent);
        }
        None
    }

    fn create_expression_error_ex(
        &self,
        source_file: &impl ast::SourceFileStoreLike,
        node: ast::Node,
        diagnostic_message: Option<&'static diagnostic_messages::Message>,
    ) -> ast::Diagnostic {
        let source = source_file.store();
        let Some(parent_declaration) = Self::find_nearest_declaration(source, node) else {
            let message = diagnostic_message.unwrap_or(
                &diagnostic_messages::Expression_type_can_t_be_inferred_with_isolatedDeclarations,
            );
            return self.create_diagnostic_for_node(source_file, node, message, &[]);
        };
        if Self::is_parent_for_id_diagnostic(source, node) == Some(parent_declaration) {
            let message = diagnostic_message.unwrap_or_else(|| {
                Self::isolated_declaration_message_for_kind(source.kind(parent_declaration)).unwrap_or(
                    &diagnostic_messages::Expression_type_can_t_be_inferred_with_isolatedDeclarations,
                )
            });
            let mut diagnostic = self.create_diagnostic_for_node(source_file, node, message, &[]);
            self.add_parent_declaration_related_info(source_file, node, &mut diagnostic);
            return diagnostic;
        }

        let message = diagnostic_message.unwrap_or(
            &diagnostic_messages::Expression_type_can_t_be_inferred_with_isolatedDeclarations,
        );
        let mut diagnostic = self.create_diagnostic_for_node(source_file, node, message, &[]);
        self.add_parent_declaration_related_info(source_file, node, &mut diagnostic);
        diagnostic.add_related_info(self.create_diagnostic_for_node(
            source_file,
            node,
            &diagnostic_messages::Add_satisfies_and_a_type_assertion_to_this_expression_satisfies_T_as_T_to_make_the_type_explicit,
            &[],
        ));
        diagnostic
    }

    fn create_expression_error(
        &self,
        source_file: &impl ast::SourceFileStoreLike,
        node: ast::Node,
    ) -> ast::Diagnostic {
        self.create_expression_error_ex(source_file, node, None)
    }

    fn create_class_expression_error(
        &self,
        source_file: &impl ast::SourceFileStoreLike,
        node: ast::Node,
    ) -> ast::Diagnostic {
        self.create_expression_error_ex(
            source_file,
            node,
            Some(
                &diagnostic_messages::Inference_from_class_expressions_is_not_supported_with_isolatedDeclarations,
            ),
        )
    }

    fn create_accessor_type_error(
        &self,
        source_file: &impl ast::SourceFileStoreLike,
        node: ast::Node,
    ) -> ast::Diagnostic {
        let source = source_file.store();
        let target_node = if source.kind(node) == ast::Kind::SetAccessor {
            source
                .source_parameters(node)
                .and_then(|parameters| parameters.first())
                .unwrap_or(node)
        } else {
            node
        };
        let message = Self::isolated_declaration_message_for_kind(source.kind(node)).unwrap();
        let mut diagnostic =
            self.create_diagnostic_for_node(source_file, target_node, message, &[]);
        let declarations = source
            .parent(node)
            .and_then(|parent| source.source_members(parent))
            .map(|members| members.nodes())
            .unwrap_or_else(|| vec![node]);
        let all_declarations = ast::get_all_accessor_declarations(source, &declarations, node);
        if let Some(set_accessor) = all_declarations.set_accessor {
            diagnostic.add_related_info(self.create_diagnostic_for_node(
                source_file,
                set_accessor,
                Self::related_suggestion_message_for_kind(source.kind(set_accessor)).unwrap(),
                &[],
            ));
        }
        if let Some(get_accessor) = all_declarations.get_accessor {
            diagnostic.add_related_info(self.create_diagnostic_for_node(
                source_file,
                get_accessor,
                Self::related_suggestion_message_for_kind(source.kind(get_accessor)).unwrap(),
                &[],
            ));
        }
        diagnostic
    }

    fn create_parameter_error(
        &self,
        source_file: &impl ast::SourceFileStoreLike,
        node: ast::Node,
    ) -> ast::Diagnostic {
        let source = source_file.store();
        if source
            .parent(node)
            .is_some_and(|parent| ast::is_set_accessor_declaration(source, parent))
        {
            return self.create_accessor_type_error(
                source_file,
                source.parent(node).expect("parameter should have a parent"),
            );
        }
        let add_undefined = node == self.declaration
            && self.declaration_requires_implicit_undefined
            || self.parameters_requiring_implicit_undefined.contains(&node);
        if !add_undefined && source.initializer(node).is_some() {
            return self.create_expression_error(source_file, node);
        }
        let message = if add_undefined {
            &diagnostic_messages::Declaration_emit_for_this_parameter_requires_implicitly_adding_undefined_to_its_type_This_is_not_supported_with_isolatedDeclarations
        } else {
            Self::isolated_declaration_message_for_kind(source.kind(node)).unwrap()
        };
        let mut diagnostic = self.create_diagnostic_for_node(source_file, node, message, &[]);
        let target = source
            .name(node)
            .map(|name| scanner::get_text_of_node(source_file, &name))
            .unwrap_or_default();
        diagnostic.add_related_info(self.create_diagnostic_for_node(
            source_file,
            node,
            Self::related_suggestion_message_for_kind(source.kind(node)).unwrap(),
            &[diagnostic_messages::Argument::from(target)],
        ));
        diagnostic
    }

    fn add_isolated_declaration_diagnostic(&mut self, node: ast::Node) {
        if !self.isolated_declarations {
            return;
        }
        let Some(file) = &self.file else {
            return;
        };
        let Some(view) = file.upgrade() else {
            return;
        };
        let source_file = view.source_file_view();
        if matches!(
            source_file.script_kind(),
            core::ScriptKind::JS | core::ScriptKind::JSX
        ) {
            return;
        }
        let source = source_file.store();
        if ast::get_source_file_of_node(source, Some(node)) != Some(view.root()) {
            return;
        }
        if ast::is_part_of_type_node(source, node) || ast::is_type_query_node(source, node) {
            self.diagnostics
                .borrow_mut()
                .push(self.create_entity_in_type_node_error(&source_file, node));
            return;
        }
        if ast::is_entity_name(source, &node) || ast::is_entity_name_expression(source, node) {
            self.diagnostics
                .borrow_mut()
                .push(self.create_entity_in_type_node_error(&source_file, node));
            return;
        }
        if source.kind(node) == ast::Kind::ClassExpression {
            self.diagnostics
                .borrow_mut()
                .push(self.create_class_expression_error(&source_file, node));
            return;
        }
        if source.kind(node) == ast::Kind::BindingElement {
            self.diagnostics
                .borrow_mut()
                .push(self.create_diagnostic_for_node(
                    &source_file,
                    node,
                    &diagnostic_messages::Binding_elements_can_t_be_exported_directly_with_isolatedDeclarations,
                    &[],
                ));
            return;
        }
        if source.kind(node) == ast::Kind::PropertyAssignment {
            if let Some(initializer) = source.initializer(node) {
                self.diagnostics
                    .borrow_mut()
                    .push(self.create_expression_error(&source_file, initializer));
            }
            return;
        }
        if matches!(
            source.kind(node),
            ast::Kind::GetAccessor | ast::Kind::SetAccessor
        ) {
            self.diagnostics
                .borrow_mut()
                .push(self.create_accessor_type_error(&source_file, node));
            return;
        }
        if source.kind(node) == ast::Kind::Parameter {
            self.diagnostics
                .borrow_mut()
                .push(self.create_parameter_error(&source_file, node));
            return;
        }
        if Self::isolated_declaration_message_for_kind(source.kind(node)).is_none() {
            self.diagnostics
                .borrow_mut()
                .push(self.create_expression_error(&source_file, node));
            return;
        }
        let message = Self::isolated_declaration_message_for_kind(source.kind(node)).unwrap_or(
            &diagnostic_messages::Expression_type_can_t_be_inferred_with_isolatedDeclarations,
        );
        let mut diagnostic = self.create_diagnostic_for_node(&source_file, node, message, &[]);
        match source.kind(node) {
            ast::Kind::ComputedPropertyName
            | ast::Kind::SpreadAssignment
            | ast::Kind::ShorthandPropertyAssignment
            | ast::Kind::ArrayLiteralExpression
            | ast::Kind::SpreadElement => {
                self.add_parent_declaration_related_info(&source_file, node, &mut diagnostic);
            }
            ast::Kind::MethodDeclaration
            | ast::Kind::ConstructSignature
            | ast::Kind::FunctionExpression
            | ast::Kind::ArrowFunction
            | ast::Kind::FunctionDeclaration => {
                self.add_parent_declaration_related_info(&source_file, node, &mut diagnostic);
                if let Some(message) = Self::related_suggestion_message_for_kind(source.kind(node))
                {
                    diagnostic.add_related_info(self.create_diagnostic_for_node(
                        &source_file,
                        node,
                        message,
                        &[],
                    ));
                }
            }
            ast::Kind::VariableDeclaration
            | ast::Kind::PropertyDeclaration
            | ast::Kind::PropertySignature
            | ast::Kind::Parameter => {
                if let Some(message) = Self::related_suggestion_message_for_kind(source.kind(node))
                {
                    let target = ast::get_name_of_declaration(source, Some(node))
                        .map(|name| scanner::get_text_of_node(&source_file, &name))
                        .unwrap_or_default();
                    let args = if target.is_empty() {
                        Vec::new()
                    } else {
                        vec![diagnostic_messages::Argument::from(target)]
                    };
                    diagnostic.add_related_info(self.create_diagnostic_for_node(
                        &source_file,
                        node,
                        message,
                        &args,
                    ));
                }
            }
            _ => {}
        }
        self.diagnostics.borrow_mut().push(diagnostic);
    }

    fn default_symbol_accessibility_diagnostic_target(
        &self,
    ) -> SymbolAccessibilityDiagnosticTarget {
        SymbolAccessibilityDiagnosticTarget {
            kind: self.declaration_kind,
            parent_kind: None,
            is_static: false,
            file: self.file.clone(),
            loc: self.loc,
            declaration_name: self.declaration_name.clone(),
        }
    }

    fn symbol_accessibility_diagnostic_target(
        &self,
        node: Option<ast::Node>,
    ) -> SymbolAccessibilityDiagnosticTarget {
        let default_target = self.default_symbol_accessibility_diagnostic_target();
        let Some(node) = node else {
            return default_target;
        };
        let Some(file) = &self.file else {
            return default_target;
        };
        let Some(view) = file.upgrade() else {
            return default_target;
        };
        let source_file = view.source_file_view();
        let source = source_file.store();
        if ast::get_source_file_of_node(source, Some(node)) != Some(view.root()) {
            return default_target;
        }
        let loc = scanner::get_error_range_for_node(&source_file, &node);
        let kind = source.kind(node);
        if !util::can_produce_diagnostics(kind) {
            return SymbolAccessibilityDiagnosticTarget {
                loc,
                ..default_target
            };
        }
        let name = source
            .property_name_or_name(node)
            .or_else(|| ast::get_name_of_declaration(source, Some(node)));
        SymbolAccessibilityDiagnosticTarget {
            kind,
            parent_kind: source.parent(node).map(|parent| source.kind(parent)),
            is_static: ast::is_static(source, node),
            file: self.file.clone(),
            loc,
            declaration_name: scanner::declaration_name_to_string(&source_file, name.as_ref()),
        }
    }

    fn add_symbol_accessibility_diagnostic(
        &mut self,
        accessibility: nodebuilder::SymbolAccessibility,
        symbol_name: &str,
        module_name: &str,
        error_node: Option<ast::Node>,
    ) -> bool {
        let target = self.symbol_accessibility_diagnostic_target(error_node);
        macro_rules! add_return_type_diagnostic {
            ($external_module_cannot_be_named:expr, $private_module:expr, $private_name:expr, $no_name_check:expr) => {{
                let (message, include_module_name) = if module_name.is_empty() {
                    ($private_name, false)
                } else if !$no_name_check
                    && accessibility == nodebuilder::SymbolAccessibility::CannotBeNamed
                {
                    ($external_module_cannot_be_named, true)
                } else {
                    ($private_module, true)
                };
                let mut args = vec![diagnostic_messages::Argument::from(symbol_name.to_owned())];
                if include_module_name {
                    args.push(diagnostic_messages::Argument::from(module_name.to_owned()));
                }
                self.diagnostics
                    .borrow_mut()
                    .push(ast::new_diagnostic_with_file(
                        target.file,
                        target.loc,
                        message,
                        &args,
                    ));
                return true;
            }};
        }
        match target.kind {
            ast::Kind::ConstructSignature => {
                // Interfaces cannot have return types that cannot be named
                add_return_type_diagnostic!(
                    &diagnostic_messages::Return_type_of_constructor_signature_from_exported_interface_has_or_is_using_name_0_from_private_module_1,
                    &diagnostic_messages::Return_type_of_constructor_signature_from_exported_interface_has_or_is_using_name_0_from_private_module_1,
                    &diagnostic_messages::Return_type_of_constructor_signature_from_exported_interface_has_or_is_using_private_name_0,
                    true
                )
            }
            ast::Kind::CallSignature => {
                // Interfaces cannot have return types that cannot be named
                add_return_type_diagnostic!(
                    &diagnostic_messages::Return_type_of_call_signature_from_exported_interface_has_or_is_using_name_0_from_private_module_1,
                    &diagnostic_messages::Return_type_of_call_signature_from_exported_interface_has_or_is_using_name_0_from_private_module_1,
                    &diagnostic_messages::Return_type_of_call_signature_from_exported_interface_has_or_is_using_private_name_0,
                    true
                )
            }
            ast::Kind::IndexSignature => {
                // Interfaces cannot have return types that cannot be named
                add_return_type_diagnostic!(
                    &diagnostic_messages::Return_type_of_index_signature_from_exported_interface_has_or_is_using_name_0_from_private_module_1,
                    &diagnostic_messages::Return_type_of_index_signature_from_exported_interface_has_or_is_using_name_0_from_private_module_1,
                    &diagnostic_messages::Return_type_of_index_signature_from_exported_interface_has_or_is_using_private_name_0,
                    true
                )
            }
            ast::Kind::MethodDeclaration | ast::Kind::MethodSignature if target.is_static => {
                add_return_type_diagnostic!(
                    &diagnostic_messages::Return_type_of_public_static_method_from_exported_class_has_or_is_using_name_0_from_external_module_1_but_cannot_be_named,
                    &diagnostic_messages::Return_type_of_public_static_method_from_exported_class_has_or_is_using_name_0_from_private_module_1,
                    &diagnostic_messages::Return_type_of_public_static_method_from_exported_class_has_or_is_using_private_name_0,
                    false
                )
            }
            ast::Kind::MethodDeclaration | ast::Kind::MethodSignature
                if target.parent_kind == Some(ast::Kind::ClassDeclaration) =>
            {
                add_return_type_diagnostic!(
                    &diagnostic_messages::Return_type_of_public_method_from_exported_class_has_or_is_using_name_0_from_external_module_1_but_cannot_be_named,
                    &diagnostic_messages::Return_type_of_public_method_from_exported_class_has_or_is_using_name_0_from_private_module_1,
                    &diagnostic_messages::Return_type_of_public_method_from_exported_class_has_or_is_using_private_name_0,
                    false
                )
            }
            ast::Kind::MethodDeclaration | ast::Kind::MethodSignature => {
                // Interfaces cannot have return types that cannot be named
                add_return_type_diagnostic!(
                    &diagnostic_messages::Return_type_of_method_from_exported_interface_has_or_is_using_name_0_from_private_module_1,
                    &diagnostic_messages::Return_type_of_method_from_exported_interface_has_or_is_using_name_0_from_private_module_1,
                    &diagnostic_messages::Return_type_of_method_from_exported_interface_has_or_is_using_private_name_0,
                    true
                )
            }
            ast::Kind::FunctionDeclaration => add_return_type_diagnostic!(
                &diagnostic_messages::Return_type_of_exported_function_has_or_is_using_name_0_from_external_module_1_but_cannot_be_named,
                &diagnostic_messages::Return_type_of_exported_function_has_or_is_using_name_0_from_private_module_1,
                &diagnostic_messages::Return_type_of_exported_function_has_or_is_using_private_name_0,
                false
            ),
            _ => {}
        }
        let (message, include_module_name) = match (target.kind, accessibility, module_name.is_empty()) {
            (
                ast::Kind::VariableDeclaration,
                nodebuilder::SymbolAccessibility::CannotBeNamed,
                false,
            ) => (
                &diagnostic_messages::Exported_variable_0_has_or_is_using_name_1_from_external_module_2_but_cannot_be_named,
                true,
            ),
            (
                ast::Kind::VariableDeclaration,
                nodebuilder::SymbolAccessibility::NotAccessible,
                false,
            ) => (
                &diagnostic_messages::Exported_variable_0_has_or_is_using_name_1_from_private_module_2,
                true,
            ),
            (ast::Kind::VariableDeclaration, _, true) => (
                &diagnostic_messages::Exported_variable_0_has_or_is_using_private_name_1,
                false,
            ),
            (
                ast::Kind::TypeAliasDeclaration | ast::Kind::JSTypeAliasDeclaration,
                _,
                false,
            ) => (
                &diagnostic_messages::Exported_type_alias_0_has_or_is_using_private_name_1_from_module_2,
                true,
            ),
            (
                ast::Kind::TypeAliasDeclaration | ast::Kind::JSTypeAliasDeclaration,
                _,
                true,
            ) => (
                &diagnostic_messages::Exported_type_alias_0_has_or_is_using_private_name_1,
                false,
            ),
            (
                ast::Kind::PropertyDeclaration
                | ast::Kind::PropertySignature
                | ast::Kind::PropertyAccessExpression
                | ast::Kind::ElementAccessExpression
                | ast::Kind::BinaryExpression,
                _,
                true,
            ) if target.is_static => (
                &diagnostic_messages::Public_static_property_0_of_exported_class_has_or_is_using_private_name_1,
                false,
            ),
            (
                ast::Kind::PropertyDeclaration
                | ast::Kind::PropertySignature
                | ast::Kind::PropertyAccessExpression
                | ast::Kind::ElementAccessExpression
                | ast::Kind::BinaryExpression,
                nodebuilder::SymbolAccessibility::CannotBeNamed,
                false,
            ) if target.is_static => (
                &diagnostic_messages::Public_static_property_0_of_exported_class_has_or_is_using_name_1_from_external_module_2_but_cannot_be_named,
                true,
            ),
            (
                ast::Kind::PropertyDeclaration
                | ast::Kind::PropertySignature
                | ast::Kind::PropertyAccessExpression
                | ast::Kind::ElementAccessExpression
                | ast::Kind::BinaryExpression,
                nodebuilder::SymbolAccessibility::NotAccessible,
                false,
            ) if target.is_static => (
                &diagnostic_messages::Public_static_property_0_of_exported_class_has_or_is_using_name_1_from_private_module_2,
                true,
            ),
            (
                ast::Kind::PropertyDeclaration
                | ast::Kind::PropertySignature
                | ast::Kind::PropertyAccessExpression
                | ast::Kind::ElementAccessExpression
                | ast::Kind::BinaryExpression,
                _,
                true,
            ) if target.parent_kind == Some(ast::Kind::ClassDeclaration) => (
                &diagnostic_messages::Public_property_0_of_exported_class_has_or_is_using_private_name_1,
                false,
            ),
            (
                ast::Kind::PropertyDeclaration
                | ast::Kind::PropertySignature
                | ast::Kind::PropertyAccessExpression
                | ast::Kind::ElementAccessExpression
                | ast::Kind::BinaryExpression,
                nodebuilder::SymbolAccessibility::CannotBeNamed,
                false,
            ) if target.parent_kind == Some(ast::Kind::ClassDeclaration) => (
                &diagnostic_messages::Public_property_0_of_exported_class_has_or_is_using_name_1_from_external_module_2_but_cannot_be_named,
                true,
            ),
            (
                ast::Kind::PropertyDeclaration
                | ast::Kind::PropertySignature
                | ast::Kind::PropertyAccessExpression
                | ast::Kind::ElementAccessExpression
                | ast::Kind::BinaryExpression,
                nodebuilder::SymbolAccessibility::NotAccessible,
                false,
            ) if target.parent_kind == Some(ast::Kind::ClassDeclaration) => (
                &diagnostic_messages::Public_property_0_of_exported_class_has_or_is_using_name_1_from_private_module_2,
                true,
            ),
            (
                ast::Kind::PropertyDeclaration
                | ast::Kind::PropertySignature
                | ast::Kind::PropertyAccessExpression
                | ast::Kind::ElementAccessExpression
                | ast::Kind::BinaryExpression,
                _,
                false,
            ) => (
                &diagnostic_messages::Property_0_of_exported_interface_has_or_is_using_name_1_from_private_module_2,
                true,
            ),
            (
                ast::Kind::PropertyDeclaration
                | ast::Kind::PropertySignature
                | ast::Kind::PropertyAccessExpression
                | ast::Kind::ElementAccessExpression
                | ast::Kind::BinaryExpression,
                _,
                true,
            ) => (
                &diagnostic_messages::Property_0_of_exported_interface_has_or_is_using_private_name_1,
                false,
            ),
            _ => return false,
        };
        let mut args = vec![
            diagnostic_messages::Argument::from(target.declaration_name),
            diagnostic_messages::Argument::from(symbol_name.to_owned()),
        ];
        if include_module_name {
            args.push(diagnostic_messages::Argument::from(module_name.to_owned()));
        }
        self.diagnostics
            .borrow_mut()
            .push(ast::new_diagnostic_with_file(
                target.file,
                target.loc,
                message,
                &args,
            ));
        true
    }
}

impl nodebuilder::SymbolTracker for DeclarationDiagnosticTracker {
    fn track_symbol(
        &mut self,
        _symbol: nodebuilder::SymbolIdentity,
        _symbol_flags: nodebuilder::SymbolFlags,
        _enclosing_declaration: Option<nodebuilder::Node>,
        _meaning: nodebuilder::SymbolFlags,
    ) -> bool {
        false
    }

    fn report_inaccessible_this_error(&mut self) {
        let Some((file, loc, declaration_name)) = self.error_context() else {
            return;
        };
        let args = vec![
            diagnostic_messages::Argument::from(declaration_name),
            diagnostic_messages::Argument::from("this"),
        ];
        self.diagnostics
            .borrow_mut()
            .push(ast::new_diagnostic_with_file(
                file,
                loc,
                &diagnostic_messages::The_inferred_type_of_0_references_an_inaccessible_1_type_A_type_annotation_is_necessary,
                &args,
            ));
    }

    fn report_private_in_base_of_class_expression(&mut self, property_name: &str) {
        let Some((file, loc, declaration_name)) = self.error_context() else {
            return;
        };
        let mut diagnostic = ast::new_diagnostic_with_file(
            file.clone(),
            loc,
            &diagnostic_messages::Property_0_of_exported_anonymous_class_type_may_not_be_private_or_protected,
            &[diagnostic_messages::Argument::from(property_name.to_owned())],
        );
        if let Some(location) = self.error_target()
            && let Some(file) = &self.file
            && let Some(view) = file.upgrade()
        {
            let source_file = view.source_file_view();
            let source = source_file.store();
            if ast::get_source_file_of_node(source, Some(location)) == Some(view.root())
                && source
                    .parent(location)
                    .is_some_and(|parent| ast::is_variable_declaration(source, parent))
            {
                diagnostic.add_related_info(self.create_diagnostic_for_node(
                    &source_file,
                    location,
                    &diagnostic_messages::Add_a_type_annotation_to_the_variable_0,
                    &[diagnostic_messages::Argument::from(declaration_name)],
                ));
            }
        }
        self.diagnostics.borrow_mut().push(diagnostic);
    }

    fn report_inaccessible_unique_symbol_error(&mut self) {
        let Some((file, loc, declaration_name)) = self.error_context() else {
            return;
        };
        let args = vec![
            diagnostic_messages::Argument::from(declaration_name),
            diagnostic_messages::Argument::from("unique symbol"),
        ];
        self.diagnostics
            .borrow_mut()
            .push(ast::new_diagnostic_with_file(
                file,
                loc,
                &diagnostic_messages::The_inferred_type_of_0_references_an_inaccessible_1_type_A_type_annotation_is_necessary,
                &args,
            ));
    }

    fn report_cyclic_structure_error(&mut self) {
        self.add_inferred_type_diagnostic(
            &diagnostic_messages::The_inferred_type_of_0_references_a_type_with_a_cyclic_structure_which_cannot_be_trivially_serialized_A_type_annotation_is_necessary,
        );
    }

    fn report_likely_unsafe_import_required_error(&mut self, specifier: &str, symbol_name: &str) {
        let Some((file, loc, declaration_name)) = self.error_context() else {
            return;
        };
        let (message, args) = if symbol_name.is_empty() {
            (
                &diagnostic_messages::The_inferred_type_of_0_cannot_be_named_without_a_reference_to_1_This_is_likely_not_portable_A_type_annotation_is_necessary,
                vec![
                    diagnostic_messages::Argument::from(declaration_name),
                    diagnostic_messages::Argument::from(specifier.to_owned()),
                ],
            )
        } else {
            (
                &diagnostic_messages::The_inferred_type_of_0_cannot_be_named_without_a_reference_to_2_from_1_This_is_likely_not_portable_A_type_annotation_is_necessary,
                vec![
                    diagnostic_messages::Argument::from(declaration_name),
                    diagnostic_messages::Argument::from(specifier.to_owned()),
                    diagnostic_messages::Argument::from(symbol_name.to_owned()),
                ],
            )
        };
        self.diagnostics
            .borrow_mut()
            .push(ast::new_diagnostic_with_file(file, loc, message, &args));
    }

    fn report_truncation_error(&mut self) {
        self.add_diagnostic_with_args(
            &diagnostic_messages::The_inferred_type_of_this_node_exceeds_the_maximum_length_the_compiler_will_serialize_An_explicit_type_annotation_is_needed,
            &[],
        );
    }

    fn report_nonlocal_augmentation(
        &mut self,
        _containing_file: &nodebuilder::SourceFile,
        _parent_symbol: nodebuilder::SymbolIdentity,
        _augmenting_symbol: nodebuilder::SymbolIdentity,
    ) {
    }

    fn report_non_serializable_property(&mut self, property_name: &str) {
        let args = vec![diagnostic_messages::Argument::from(
            property_name.to_owned(),
        )];
        self.add_diagnostic_with_args(
            &diagnostic_messages::The_type_of_this_node_cannot_be_serialized_because_its_property_0_cannot_be_serialized,
            &args,
        );
    }

    fn mark_aliases_visible(&mut self, aliases: &[nodebuilder::Node]) {
        let mut late_marked_statements = self.late_marked_statements.borrow_mut();
        for alias in aliases {
            if !late_marked_statements.contains(alias) {
                late_marked_statements.push(*alias);
            }
        }
    }

    fn report_symbol_accessibility_error(
        &mut self,
        accessibility: nodebuilder::SymbolAccessibility,
        error_symbol_name: &str,
        error_module_name: &str,
        error_node: Option<nodebuilder::Node>,
    ) -> bool {
        self.add_symbol_accessibility_diagnostic(
            accessibility,
            error_symbol_name,
            error_module_name,
            error_node,
        )
    }

    fn report_inference_fallback(&mut self, node: nodebuilder::Node) {
        if node == self.declaration
            && let Some(diagnostics) = self.inference_fallback_expando_diagnostics.take()
        {
            self.diagnostics.borrow_mut().extend(diagnostics);
            return;
        }
        self.add_isolated_declaration_diagnostic(node);
    }

    fn push_error_fallback_node(&mut self, node: nodebuilder::Node) {
        self.error_fallback_nodes.push(node);
    }

    fn pop_error_fallback_node(&mut self) {
        self.error_fallback_nodes.pop();
    }
}

struct DeclarationImporter<'source> {
    source: &'source ast::AstStore,
    state: ast::AstImportState,
}

impl<'source> DeclarationImporter<'source> {
    fn new(source: &'source ast::AstStore) -> Self {
        Self {
            source,
            state: ast::AstImportState::new(),
        }
    }

    fn factory<'context>(
        &mut self,
        emit_context: &'context mut printer::EmitContext,
    ) -> &'context mut ast::NodeFactory {
        &mut emit_context.factory.node_factory
    }

    fn preserve_node(
        &mut self,
        emit_context: &mut printer::EmitContext,
        node: ast::Node,
    ) -> ast::Node {
        self.state
            .preserve_node(self.source, &mut emit_context.factory.node_factory, node)
    }

    fn preserve_optional_node(
        &mut self,
        emit_context: &mut printer::EmitContext,
        node: Option<ast::Node>,
    ) -> Option<ast::Node> {
        node.map(|node| self.preserve_node(emit_context, node))
    }

    fn preserve_optional_source_node_list(
        &mut self,
        emit_context: &mut printer::EmitContext,
        list: Option<ast::SourceNodeList<'_>>,
    ) -> Option<ast::NodeList> {
        self.state
            .preserve_optional_source_node_list(&mut emit_context.factory.node_factory, list)
    }

    fn preserve_optional_source_modifier_list(
        &mut self,
        emit_context: &mut printer::EmitContext,
        modifiers: Option<ast::SourceModifierList<'_>>,
    ) -> Option<ast::ModifierList> {
        self.state.preserve_optional_source_modifier_list(
            &mut emit_context.factory.node_factory,
            modifiers,
        )
    }

    fn update_source_file(
        &mut self,
        emit_context: &mut printer::EmitContext,
        node: ast::Node,
        statements: impl ast::IntoOptionalNodeList,
        end_of_file_token: impl Into<Option<ast::Node>>,
    ) -> ast::Node {
        self.state.update_source_file_from_store(
            self.source,
            &mut emit_context.factory.node_factory,
            node,
            statements,
            end_of_file_token,
        )
    }
}

pub struct DeclarationTransformer<'a> {
    host: &'a mut dyn DeclarationEmitHost,
    compiler_options: core::CompilerOptions,
    declaration_file_path: String,
    declaration_map_path: String,
    state: DeclarationSourceFileState,
    diagnostics: Vec<ast::Diagnostic>,
    late_marked_statements: Rc<RefCell<Vec<ast::Node>>>,
    late_statement_replacement_map: HashMap<ast::NodeId, Vec<ast::Node>>,
    expando_hosts: HashMap<ast::NodeId, Vec<ast::Node>>,
}

impl<'a> DeclarationTransformer<'a> {
    pub fn declaration_file_path(&self) -> &str {
        &self.declaration_file_path
    }

    pub fn declaration_map_path(&self) -> &str {
        &self.declaration_map_path
    }

    pub fn transform_source_file(&mut self, file: &ast::SourceFile) -> ast::SourceFile {
        let mut emit_context = printer::new_emit_context();
        emit_context.activate();
        emit_context.set_source_file(Some(file));
        self.transform_source_file_with_emit_context(file, &mut emit_context)
    }

    pub fn transform_source_file_with_emit_context(
        &mut self,
        file: &ast::SourceFile,
        emit_context: &mut printer::EmitContext,
    ) -> ast::SourceFile {
        let source = file.store();
        if !transform::visit_source_file_should_transform(DeclarationTransformFacts {
            is_declaration_file: file.is_declaration_file(),
            is_external_or_common_js_module: self
                .host
                .source_file_is_external_or_common_js_module(file),
            result_has_external_module_indicator: self.state.result_has_external_module_indicator,
            needs_scope_fix_marker: self.state.needs_scope_fix_marker,
            result_has_scope_marker: self.state.result_has_scope_marker,
            ..Default::default()
        }) {
            return file.share_readonly();
        }

        self.state = DeclarationSourceFileState::for_source_file();
        self.state.needs_scope_fix_marker = false;
        self.state.result_has_external_module_indicator = false;
        self.late_marked_statements.borrow_mut().clear();
        self.late_statement_replacement_map.clear();
        self.expando_hosts.clear();
        self.host.precalculate_declaration_emit_visibility(file);

        emit_context.activate();
        emit_context.set_source_file(Some(file));
        let mut importer = DeclarationImporter::new(source);
        let enclosing_declaration = file.as_node();
        let source_file_data = source.as_source_file(file.as_node());
        let source_statements = source
            .parser_access()
            .source_file_statement_list(file.as_node());
        let statement_pairs: Vec<(ast::Node, Vec<ast::Node>)> = source_statements
            .iter()
            .map(|statement| {
                let transformed = self.transform_declaration_statement(
                    file,
                    source,
                    &statement,
                    &mut importer,
                    emit_context,
                    enclosing_declaration,
                );
                if ast::is_late_visibility_painted_statement(source, statement) {
                    let original = emit_context.most_original(&statement);
                    let id = emit_context.store_for_node(original).get_node_id(original);
                    self.late_statement_replacement_map
                        .insert(id, transformed.clone());
                }
                (statement, transformed)
            })
            .collect();
        let mut statements = self.transform_and_replace_late_painted_statements(
            file,
            source,
            statement_pairs,
            &mut importer,
            emit_context,
            enclosing_declaration,
        );
        if transform::declaration_output_needs_empty_export_marker(DeclarationTransformFacts {
            is_external_or_common_js_module: self
                .host
                .source_file_is_external_or_common_js_module(file),
            result_has_external_module_indicator: self.state.result_has_external_module_indicator,
            needs_scope_fix_marker: self.state.needs_scope_fix_marker,
            result_has_scope_marker: self.state.result_has_scope_marker,
            ..Default::default()
        }) {
            statements.push(create_empty_exports(importer.factory(emit_context)));
        }
        if self.host.source_file_is_external_or_common_js_module(file)
            && ast::is_in_js_file(source, file.as_node())
        {
            let export_equals_declarations = self.host.source_file_export_equals_declarations(file);
            if export_equals_declarations.len() > 1 {
                for declaration in export_equals_declarations {
                    self.add_declaration_diagnostic(
                        file,
                        declaration,
                        &diagnostic_messages::Multiple_module_exports_assignments_cannot_be_serialized_for_declaration_emit,
                    );
                }
            }
            for declaration in self.host.source_file_nested_cjs_exports(file) {
                self.add_declaration_diagnostic(
                    file,
                    declaration,
                    &diagnostic_messages::Nested_CommonJS_export_constructs_cannot_be_serialized_for_declaration_emit,
                );
            }
        }
        let source_statement_loc = source_statements.loc();
        let statement_start =
            scanner::skip_trivia(file.text(), source_statement_loc.pos().max(0) as usize) as i32;
        let statement_loc = core::new_text_range(statement_start, source_statement_loc.end());
        let statements =
            importer
                .factory(emit_context)
                .new_node_list(statement_loc, statement_loc, statements);
        let end_of_file_token =
            importer.preserve_optional_node(emit_context, source_file_data.end_of_file_token());
        let updated = importer.update_source_file(
            emit_context,
            file.as_node(),
            Some(statements),
            end_of_file_token,
        );
        let output_file_path =
            tspath::get_directory_path(&tspath::normalize_slashes(&self.declaration_file_path));
        let referenced_files =
            self.get_referenced_files(file, source_file_data.referenced_files(), &output_file_path);
        let type_reference_directives =
            Self::get_synthetic_references(source_file_data.type_reference_directives());
        let lib_reference_directives =
            Self::get_synthetic_references(source_file_data.lib_reference_directives());
        emit_context
            .factory
            .node_factory
            .set_source_file_declaration_metadata(
                updated,
                referenced_files,
                type_reference_directives,
                lib_reference_directives,
            );
        crate::transformer::finish_declaration_source_file_output(file, emit_context, updated)
    }

    fn get_referenced_files(
        &self,
        source_file: &ast::SourceFile,
        references: &[ast::FileReference],
        output_file_path: &str,
    ) -> Vec<ast::FileReference> {
        references
            .iter()
            .filter_map(|reference| {
                if !reference.preserve {
                    return None;
                }

                let file = self
                    .host
                    .get_source_file_from_reference(source_file, reference)?;
                let decl_file_name = if file.is_declaration_file() {
                    file.file_name().to_owned()
                } else {
                    let paths = self.host.get_output_paths_for(&file, true);
                    if !paths.declaration_file_path().is_empty() {
                        paths.declaration_file_path().to_owned()
                    } else if !paths.js_file_path().is_empty() {
                        paths.js_file_path().to_owned()
                    } else {
                        file.file_name().to_owned()
                    }
                };
                if decl_file_name.is_empty() {
                    return None;
                }

                let file_name = tspath::get_relative_path_to_directory_or_url(
                    output_file_path,
                    &decl_file_name,
                    false,
                    &tspath::ComparePathsOptions {
                        current_directory: self.host.get_current_directory(),
                        use_case_sensitive_file_names: self.host.use_case_sensitive_file_names(),
                    },
                );
                Some(ast::FileReference {
                    text_range: core::undefined_text_range(),
                    file_name,
                    resolution_mode: reference.resolution_mode,
                    preserve: reference.preserve,
                })
            })
            .collect()
    }

    fn get_synthetic_references(references: &[ast::FileReference]) -> Vec<ast::FileReference> {
        references
            .iter()
            .filter(|reference| reference.preserve)
            .map(|reference| ast::FileReference {
                text_range: core::undefined_text_range(),
                file_name: reference.file_name.clone(),
                resolution_mode: reference.resolution_mode,
                preserve: reference.preserve,
            })
            .collect()
    }

    fn add_declaration_diagnostic(
        &mut self,
        file: &ast::SourceFile,
        node: ast::Node,
        message: &diagnostic_messages::Message,
    ) {
        let loc = scanner::get_error_range_for_node(file, &node);
        self.diagnostics
            .push(ast::new_diagnostic(Some(file), loc, message, &[]));
    }

    fn should_strip_internal(
        &self,
        file: &ast::SourceFile,
        node: &ast::Node,
        emit_context: &printer::EmitContext,
    ) -> bool {
        if !self.compiler_options.strip_internal.is_true() {
            return false;
        }
        let Some(parse_node) = emit_context.parse_node(node) else {
            return false;
        };
        let store = emit_context.store_for_node(parse_node);
        scanner::get_leading_comment_ranges(file.text(), store.loc(parse_node).pos())
            .into_iter()
            .any(|comment| has_internal_annotation(file, comment))
    }

    fn transform_and_replace_late_painted_statements(
        &mut self,
        file: &ast::SourceFile,
        source: &ast::AstStore,
        statement_pairs: Vec<(ast::Node, Vec<ast::Node>)>,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        enclosing_declaration: ast::Node,
    ) -> Vec<ast::Node> {
        loop {
            let next = {
                let mut late_marked_statements = self.late_marked_statements.borrow_mut();
                if late_marked_statements.is_empty() {
                    None
                } else {
                    Some(late_marked_statements.remove(0))
                }
            };
            let Some(next) = next else {
                break;
            };
            assert!(
                ast::is_late_visibility_painted_statement(source, next),
                "late replaced statement was not declaration-transformable"
            );
            self.late_marked_statements
                .borrow_mut()
                .retain(|statement| *statement != next);

            let saved_needs_declare = self.state.needs_declare;
            self.state.needs_declare = source
                .parent(next)
                .is_some_and(|parent| ast::is_source_file(source, parent));
            let replacement = self.transform_declaration_statement(
                file,
                source,
                &next,
                importer,
                emit_context,
                enclosing_declaration,
            );
            self.state.needs_declare = saved_needs_declare;

            let original = emit_context.most_original(&next);
            let id = emit_context.store_for_node(original).get_node_id(original);
            self.late_statement_replacement_map.insert(id, replacement);
        }

        let mut statements = Vec::new();
        let parent_is_source_file = ast::is_source_file(source, enclosing_declaration);
        for (original, transformed) in statement_pairs {
            let most_original = emit_context.most_original(&original);
            let id = emit_context
                .store_for_node(most_original)
                .get_node_id(most_original);
            if ast::is_late_visibility_painted_statement(source, original) {
                if let Some(replacement) = self.late_statement_replacement_map.remove(&id) {
                    for node in replacement {
                        self.append_declaration_output_statement(
                            emit_context,
                            &mut statements,
                            node,
                            parent_is_source_file,
                        );
                    }
                    continue;
                }
            }
            for node in transformed {
                self.append_declaration_output_statement(
                    emit_context,
                    &mut statements,
                    node,
                    parent_is_source_file,
                );
            }
        }
        statements
    }

    fn append_declaration_output_statement(
        &mut self,
        emit_context: &mut printer::EmitContext,
        statements: &mut Vec<ast::Node>,
        node: ast::Node,
        parent_is_source_file: bool,
    ) {
        let store = emit_context.store_for_node(node);
        if store.kind(node) == ast::Kind::SyntaxList {
            let children = store
                .syntax_list_children(node)
                .expect("SyntaxList should have children")
                .into_iter()
                .flatten();
            for child in children {
                self.record_declaration_output_state(
                    emit_context.store_for_node(child),
                    child,
                    parent_is_source_file,
                );
                statements.push(child);
            }
        } else {
            self.record_declaration_output_state(store, node, parent_is_source_file);
            statements.push(node);
        }
    }

    fn transform_declaration_statement(
        &mut self,
        file: &ast::SourceFile,
        source: &ast::AstStore,
        statement: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        enclosing_declaration: ast::Node,
    ) -> Vec<ast::Node> {
        let kind = source.kind(*statement);
        if transform::declaration_transform_action_for_kind(kind)
            == transform::DeclarationTransformAction::ElideStatement
        {
            return vec![];
        }
        let is_expression_statement = kind == ast::Kind::ExpressionStatement;
        let is_expando_assignment = is_expression_statement
            && source.expression(*statement).is_some_and(|expression| {
                ast::get_assignment_declaration_kind(source, expression)
                    == ast::JSDeclarationKind::Property
            });
        let is_commonjs_export_assignment = is_expression_statement
            && source.expression(*statement).is_some_and(|expression| {
                self.host
                    .source_file_common_js_module_indicator(file)
                    .is_some()
                    && matches!(
                        ast::get_assignment_declaration_kind(source, expression),
                        ast::JSDeclarationKind::ModuleExports
                            | ast::JSDeclarationKind::ExportsProperty
                            | ast::JSDeclarationKind::ObjectDefinePropertyExports
                    )
            });
        if !ast::is_source_file_js(file)
            && !util::is_preserved_declaration_statement(kind)
            && !is_expando_assignment
            && !is_commonjs_export_assignment
        {
            return vec![];
        }
        if self.should_strip_internal(file, statement, emit_context) {
            return vec![];
        }
        let original = emit_context.most_original(statement);
        let id = emit_context.store_for_node(original).get_node_id(original);
        if let Some(expando_host) = self.expando_hosts.get(&id) {
            return expando_host.clone();
        }
        if matches!(
            kind,
            ast::Kind::FunctionDeclaration
                | ast::Kind::ModuleDeclaration
                | ast::Kind::InterfaceDeclaration
                | ast::Kind::ClassDeclaration
                | ast::Kind::TypeAliasDeclaration
                | ast::Kind::JSTypeAliasDeclaration
                | ast::Kind::EnumDeclaration
        ) && !self.host.is_declaration_visible(*statement)
        {
            return vec![];
        }

        let enclosing_declaration = if util::is_enclosing_declaration(kind) {
            *statement
        } else {
            enclosing_declaration
        };

        let transformed: Vec<_> = match kind {
            ast::Kind::VariableStatement => self
                .transform_variable_statement(
                    source,
                    statement,
                    importer,
                    emit_context,
                    enclosing_declaration,
                )
                .into_iter()
                .collect(),
            ast::Kind::ImportEqualsDeclaration => self
                .transform_import_equals_declaration(
                    source,
                    statement,
                    importer,
                    emit_context,
                    enclosing_declaration,
                )
                .into_iter()
                .collect(),
            ast::Kind::ImportDeclaration => self
                .transform_import_declaration(file, source, statement, importer, emit_context)
                .into_iter()
                .collect(),
            ast::Kind::ExportDeclaration => self
                .transform_export_declaration(source, statement, importer, emit_context)
                .into_iter()
                .collect(),
            ast::Kind::ExportAssignment => {
                self.transform_export_assignment(source, statement, importer, emit_context)
            }
            ast::Kind::ClassDeclaration => self
                .transform_class_declaration(file, source, statement, importer, emit_context)
                .into_iter()
                .collect(),
            ast::Kind::InterfaceDeclaration => self
                .transform_interface_declaration(file, source, statement, importer, emit_context)
                .into_iter()
                .collect(),
            ast::Kind::TypeAliasDeclaration | ast::Kind::JSTypeAliasDeclaration => self
                .transform_type_alias_declaration(source, statement, importer, emit_context)
                .into_iter()
                .collect(),
            ast::Kind::FunctionDeclaration => self
                .transform_function_declaration(
                    file,
                    source,
                    statement,
                    importer,
                    emit_context,
                    enclosing_declaration,
                )
                .into_iter()
                .collect(),
            ast::Kind::ModuleDeclaration => vec![self.transform_module_declaration(
                file,
                source,
                statement,
                importer,
                emit_context,
            )],
            ast::Kind::EnumDeclaration => vec![self.transform_enum_declaration(
                file,
                source,
                statement,
                importer,
                emit_context,
            )],
            ast::Kind::ExpressionStatement => self.transform_expression_statement(
                file,
                source,
                statement,
                importer,
                emit_context,
                enclosing_declaration,
            ),
            _ => vec![importer.preserve_node(emit_context, *statement)],
        };
        transformed
    }

    fn record_declaration_output_state(
        &mut self,
        source: &ast::AstStore,
        statement: ast::Node,
        parent_is_source_file: bool,
    ) {
        if declaration_output_statement_needs_scope_marker(source, statement) {
            self.state.needs_scope_fix_marker = true;
        }
        if parent_is_source_file && is_external_module_indicator_statement(source, statement) {
            self.state.result_has_external_module_indicator = true;
        }
        if source.kind(statement) == ast::Kind::ExportAssignment
            || source.kind(statement) == ast::Kind::ExportDeclaration
        {
            self.state.result_has_scope_marker = true;
        }
    }

    fn transform_export_assignment(
        &mut self,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> Vec<ast::Node> {
        if source
            .parent(*original)
            .is_some_and(|parent| ast::is_source_file(source, parent))
        {
            self.state.result_has_external_module_indicator = true;
        }
        self.state.result_has_scope_marker = true;
        let expression = source
            .expression(*original)
            .expect("export assignment should have expression");
        let is_export_equals = source.is_export_equals(*original).unwrap_or(false);
        if ast::is_identifier(source, expression) {
            let expression = importer.preserve_node(emit_context, expression);
            let export_assignment = importer.factory(emit_context).new_export_assignment(
                None,
                is_export_equals,
                None,
                expression,
            );
            return vec![export_assignment];
        }

        // expression is non-identifier, create _default typed variable to reference
        let default_name = emit_context.factory.new_unique_name_ex(
            "_default",
            printer::AutoGenerateOptions {
                flags: printer::GeneratedIdentifierFlags::OPTIMISTIC,
                ..Default::default()
            },
        );
        let skipped_expression = ast::skip_parentheses(source, expression);
        let mut initializer = None;
        if ast::is_primitive_literal_value(source, skipped_expression, true) {
            initializer = self
                .host
                .create_literal_const_value(emit_context, *original);
        }
        let type_node = if initializer.is_none() {
            self.ensure_type_with_error_fallback_node(
                source,
                original,
                importer,
                emit_context,
                *original,
                false,
                Some(*original),
            )
        } else {
            None
        };
        let declaration = importer.factory(emit_context).new_variable_declaration(
            default_name.clone(),
            None,
            type_node,
            initializer,
        );
        let declarations = importer.factory(emit_context).new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![declaration],
        );
        let declaration_list = importer
            .factory(emit_context)
            .new_variable_declaration_list(declarations, ast::NodeFlags::CONST);
        let modifiers = if self.state.needs_declare {
            let declare_modifier = importer
                .factory(emit_context)
                .new_modifier(ast::Kind::DeclareKeyword);
            Some(importer.factory(emit_context).new_modifier_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                vec![declare_modifier],
                ast::ModifierFlags::AMBIENT,
            ))
        } else {
            Some(importer.factory(emit_context).new_modifier_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                vec![],
                ast::ModifierFlags::NONE,
            ))
        };
        let declaration_statement = importer
            .factory(emit_context)
            .new_variable_statement(modifiers, declaration_list);
        let export_statement = importer.factory(emit_context).new_export_assignment(
            None,
            is_export_equals,
            None,
            default_name,
        );
        vec![declaration_statement, export_statement]
    }

    fn transform_export_declaration(
        &mut self,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> Option<ast::Node> {
        if source
            .parent(*original)
            .is_some_and(|parent| ast::is_source_file(source, parent))
        {
            self.state.result_has_external_module_indicator = true;
        }
        self.state.result_has_scope_marker = true;
        let modifiers = importer.preserve_optional_source_modifier_list(
            emit_context,
            source.source_modifiers(*original),
        );
        // Rewrite external module names if necessary
        let module_specifier = self.rewrite_module_specifier(
            source,
            *original,
            source.module_specifier(*original),
            importer,
            emit_context,
        );
        let attributes = self.try_get_resolution_mode_override(
            source.attributes(*original),
            importer,
            emit_context,
        );
        Some(
            importer
                .factory(emit_context)
                .update_export_declaration_from_store(
                    source,
                    *original,
                    modifiers,
                    source.is_type_only(*original).unwrap_or(false),
                    source.export_clause(*original),
                    module_specifier,
                    attributes,
                ),
        )
    }

    fn transform_import_equals_declaration(
        &mut self,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        enclosing_declaration: ast::Node,
    ) -> Option<ast::Node> {
        if !self.host.is_declaration_visible(*original) {
            return None;
        }
        if source
            .module_reference(*original)
            .is_some_and(|module_reference| {
                source.kind(module_reference) == ast::Kind::ExternalModuleReference
            })
        {
            // Rewrite external module names if necessary
            let specifier =
                ast::get_external_module_import_equals_declaration_expression(source, *original);
            let module_reference = source
                .module_reference(*original)
                .expect("external import equals declaration should have a module reference");
            let specifier =
                self.rewrite_module_specifier(source, *original, specifier, importer, emit_context);
            let module_reference = importer
                .factory(emit_context)
                .update_external_module_reference_from_store(source, module_reference, specifier);
            let modifiers = importer.preserve_optional_source_modifier_list(
                emit_context,
                source.source_modifiers(*original),
            );
            let name = importer.preserve_optional_node(emit_context, source.name(*original));
            return Some(
                importer
                    .factory(emit_context)
                    .update_import_equals_declaration_from_store(
                        source,
                        *original,
                        modifiers,
                        source.is_type_only(*original).unwrap_or(false),
                        name,
                        module_reference,
                    ),
            );
        }

        let module_reference = source
            .module_reference(*original)
            .expect("import equals declaration should have a module reference");
        self.check_entity_name_visibility(
            source,
            module_reference,
            enclosing_declaration,
            *original,
        );
        Some(importer.preserve_node(emit_context, *original))
    }

    fn transform_type_alias_declaration(
        &mut self,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> Option<ast::Node> {
        if !self.host.is_declaration_visible(*original) {
            return None;
        }

        let previous_needs_declare = self.state.needs_declare;
        self.state.needs_declare = false;

        let modifiers = self.ensure_modifiers(source, original, importer, emit_context);
        let name = importer.preserve_optional_node(emit_context, source.name(*original));
        let type_parameters = self.preserve_type_parameters(
            source,
            source.type_parameters(*original),
            *original,
            importer,
            emit_context,
        );
        let type_node = source.r#type(*original).map(|type_node| {
            self.check_type_node_visibility(source, type_node, *original, *original);
            self.preserve_declaration_type_node(
                source,
                type_node,
                importer,
                emit_context,
                *original,
            )
        });
        let updated = importer
            .factory(emit_context)
            .update_type_alias_declaration_from_store(
                source,
                *original,
                modifiers,
                name,
                type_parameters,
                type_node,
            );

        self.state.needs_declare = previous_needs_declare;
        Some(updated)
    }

    fn transform_interface_declaration(
        &mut self,
        file: &ast::SourceFile,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> Option<ast::Node> {
        if !self.host.is_declaration_visible(*original) {
            return None;
        }

        let modifiers = self.ensure_modifiers(source, original, importer, emit_context);
        let name = importer.preserve_optional_node(emit_context, source.name(*original));
        let type_parameters = self.preserve_type_parameters(
            source,
            source.type_parameters(*original),
            *original,
            importer,
            emit_context,
        );
        let heritage_clauses = self.transform_heritage_clauses(
            source,
            source.heritage_clauses(*original),
            importer,
            emit_context,
            *original,
        );
        let members = source
            .members(*original)
            .expect("interface declaration should have members");
        let member_nodes = members
            .iter()
            .filter_map(|member| {
                self.transform_interface_member(
                    Some(file),
                    source,
                    &member,
                    importer,
                    emit_context,
                    *original,
                )
            })
            .collect::<Vec<_>>();
        let members = importer.factory(emit_context).new_node_list(
            members.loc(),
            members.range(),
            member_nodes,
        );

        Some(
            importer
                .factory(emit_context)
                .update_interface_declaration_from_store(
                    source,
                    *original,
                    modifiers,
                    name,
                    type_parameters,
                    heritage_clauses,
                    members,
                ),
        )
    }

    fn transform_variable_statement(
        &mut self,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        enclosing_declaration: ast::Node,
    ) -> Option<ast::Node> {
        let Some(declaration_list_node) = source.declaration_list(*original) else {
            return Some(importer.preserve_node(emit_context, *original));
        };
        let input_declarations = source
            .declarations(declaration_list_node)
            .expect("variable declaration list should have declarations");
        let visible = input_declarations
            .iter()
            .any(|declaration| self.get_binding_name_visible(source, &declaration));
        if !visible {
            return None;
        }

        let mut declarations = Vec::new();
        for declaration in input_declarations.iter() {
            let Some(transformed) = self.transform_variable_declaration(
                source,
                &declaration,
                importer,
                emit_context,
                enclosing_declaration,
            ) else {
                continue;
            };
            let transformed_store = emit_context.store_for_node(transformed);
            if transformed_store.kind(transformed) == ast::Kind::SyntaxList {
                declarations.extend(
                    transformed_store
                        .syntax_list_children(transformed)
                        .expect("SyntaxList should have children")
                        .iter()
                        .flatten(),
                );
            } else {
                declarations.push(transformed);
            }
        }
        if declarations.is_empty() {
            return None;
        }

        let modifiers = self.ensure_modifiers(source, original, importer, emit_context);
        let Some(declaration_list) = source.declaration_list(*original) else {
            return Some(importer.preserve_node(emit_context, *original));
        };
        let declaration_list = if ast::is_var_using(source, declaration_list)
            || ast::is_var_await_using(source, declaration_list)
        {
            let declarations = importer.factory(emit_context).new_node_list(
                input_declarations.loc(),
                input_declarations.range(),
                declarations,
            );
            importer
                .factory(emit_context)
                .new_variable_declaration_list(declarations, ast::NodeFlags::CONST)
        } else {
            let declarations = importer.factory(emit_context).new_node_list(
                input_declarations.loc(),
                input_declarations.range(),
                declarations,
            );
            importer
                .factory(emit_context)
                .update_variable_declaration_list_from_store(
                    source,
                    declaration_list,
                    declarations,
                    source.flags(declaration_list),
                )
        };
        Some(
            importer
                .factory(emit_context)
                .update_variable_statement_from_store(
                    source,
                    *original,
                    modifiers,
                    declaration_list,
                ),
        )
    }

    fn transform_variable_declaration(
        &mut self,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        enclosing_declaration: ast::Node,
    ) -> Option<ast::Node> {
        if !self.get_binding_name_visible(source, original) {
            return None;
        }
        if let Some(name) = source.name(*original)
            && ast::is_binding_pattern(source, name)
        {
            return self.recreate_binding_pattern(source, name, importer, emit_context);
        }
        let old_suppress_new_diagnostic_contexts = self.state.suppress_new_diagnostic_contexts;
        self.state.suppress_new_diagnostic_contexts = true;
        let initializer = self.ensure_no_initializer(source, original, emit_context);
        let name = importer.preserve_optional_node(emit_context, source.name(*original));
        let exclamation_token =
            importer.preserve_optional_node(emit_context, source.exclamation_token(*original));
        let type_node = self.ensure_declaration_type(
            source,
            original,
            importer,
            emit_context,
            enclosing_declaration,
        );
        self.state.suppress_new_diagnostic_contexts = old_suppress_new_diagnostic_contexts;
        Some(
            importer
                .factory(emit_context)
                .update_variable_declaration_from_store(
                    source,
                    *original,
                    name,
                    exclamation_token,
                    type_node,
                    initializer,
                ),
        )
    }

    fn recreate_binding_pattern(
        &mut self,
        source: &ast::AstStore,
        input: ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> Option<ast::Node> {
        let mut results = Vec::new();
        let elements = source
            .source_elements(input)
            .expect("binding pattern should have elements");
        for elem in elements.iter() {
            let Some(result) = self.recreate_binding_element(source, &elem, importer, emit_context)
            else {
                continue;
            };
            let result_store = emit_context.store_for_node(result);
            if result_store.kind(result) == ast::Kind::SyntaxList {
                results.extend(
                    result_store
                        .syntax_list_children(result)
                        .expect("SyntaxList should have children")
                        .iter()
                        .flatten(),
                );
            } else {
                results.push(result);
            }
        }
        match results.len() {
            0 => None,
            1 => results.into_iter().next(),
            _ => Some(importer.factory(emit_context).new_syntax_list(results)),
        }
    }

    fn recreate_binding_element(
        &mut self,
        source: &ast::AstStore,
        e: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> Option<ast::Node> {
        let name = source.name(*e)?;
        if !self.get_binding_name_visible(source, e) {
            return None;
        }
        if ast::is_binding_pattern(source, name) {
            return self.recreate_binding_pattern(source, name, importer, emit_context);
        }
        let name = importer.preserve_node(emit_context, name);
        let type_node = self.ensure_type(source, e, importer, emit_context, *e, false);
        Some(
            importer
                .factory(emit_context)
                .new_variable_declaration(name, None, type_node, None),
        )
    }

    fn transform_class_declaration(
        &mut self,
        file: &ast::SourceFile,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> Vec<ast::Node> {
        if !self.host.is_declaration_visible(*original) {
            return Vec::new();
        }

        let members = source
            .members(*original)
            .expect("class declaration should have members");
        let mut member_nodes = Vec::new();
        // When the class has at least one private identifier, create a unique constant identifier to retain the nominal typing behavior
        // Prevents other classes with the same public members from being used in place of the current class
        if members.iter().any(|member| {
            source
                .name(member)
                .is_some_and(|name| ast::is_private_identifier(source, name))
        }) {
            let private_name = importer
                .factory(emit_context)
                .new_private_identifier("#private");
            member_nodes.push(importer.factory(emit_context).new_property_declaration(
                None,
                private_name,
                None,
                None,
                None,
            ));
        }
        member_nodes.extend({
            let diagnostics = Rc::new(RefCell::new(Vec::new()));
            let tracker = DeclarationDiagnosticTracker::new_with_error_fallback_node(
                diagnostics.clone(),
                self.late_marked_statements.clone(),
                self.compiler_options.isolated_declarations.is_true(),
                source,
                *original,
                Some(*original),
            );
            let member_nodes = self.host.create_late_bound_index_signatures(
                emit_context,
                *original,
                *original,
                Box::new(tracker),
            );
            self.diagnostics.extend(diagnostics.borrow_mut().drain(..));
            member_nodes
        });
        member_nodes.extend(self.collect_parameter_properties(
            source,
            original,
            importer,
            emit_context,
        ));
        // Collect this.x property assignments from constructors and static blocks in JS files
        if ast::is_in_js_file(source, *original) {
            member_nodes.extend(self.collect_this_property_assignments(
                source,
                original,
                importer,
                emit_context,
            ));
        }
        for member in members.iter() {
            if let Some(transformed) = self.transform_class_member(
                file,
                source,
                &member,
                importer,
                emit_context,
                *original,
            ) {
                member_nodes.push(transformed);
            }
        }
        let members = importer.factory(emit_context).new_node_list(
            members.loc(),
            members.range(),
            member_nodes,
        );
        let modifiers = self.ensure_modifiers(source, original, importer, emit_context);
        let name = importer.preserve_optional_node(emit_context, source.name(*original));
        let type_parameters = self.preserve_type_parameters(
            source,
            source.type_parameters(*original),
            *original,
            importer,
            emit_context,
        );
        let extends_clause = get_effective_base_type_node(source, *original);

        if let Some(extends_clause) = extends_clause {
            let expression = source
                .expression(extends_clause)
                .expect("ExpressionWithTypeArguments should have an expression");
            if !ast::is_entity_name_expression(source, expression)
                && source.kind(expression) != ast::Kind::NullKeyword
            {
                let old_id = source
                    .name(*original)
                    .filter(|name| {
                        ast::is_identifier(source, *name) && !source.text(*name).is_empty()
                    })
                    .map(|name| source.text(name))
                    .unwrap_or_else(|| "default".to_owned());
                let new_id = emit_context.factory.new_unique_name_ex(
                    &format!("{old_id}_base"),
                    printer::AutoGenerateOptions {
                        flags: printer::GeneratedIdentifierFlags::OPTIMISTIC,
                        ..Default::default()
                    },
                );
                let diagnostics = Rc::new(RefCell::new(Vec::new()));
                let type_node = self.host.create_type_of_expression(
                    emit_context,
                    expression,
                    *original,
                    Box::new(DeclarationDiagnosticTracker::new_with_error_fallback_node(
                        diagnostics.clone(),
                        self.late_marked_statements.clone(),
                        self.compiler_options.isolated_declarations.is_true(),
                        source,
                        *original,
                        Some(*original),
                    )),
                );
                self.diagnostics.extend(diagnostics.borrow_mut().drain(..));
                let var_decl = importer.factory(emit_context).new_variable_declaration(
                    new_id.clone(),
                    None,
                    type_node,
                    None,
                );
                let declarations = importer.factory(emit_context).new_node_list(
                    core::undefined_text_range(),
                    core::undefined_text_range(),
                    vec![var_decl],
                );
                let declaration_list = importer
                    .factory(emit_context)
                    .new_variable_declaration_list(declarations, ast::NodeFlags::CONST);
                let var_modifiers = if self.state.needs_declare {
                    let declare_modifier = importer
                        .factory(emit_context)
                        .new_modifier(ast::Kind::DeclareKeyword);
                    Some(importer.factory(emit_context).new_modifier_list(
                        core::undefined_text_range(),
                        core::undefined_text_range(),
                        vec![declare_modifier],
                        ast::ModifierFlags::AMBIENT,
                    ))
                } else {
                    None
                };
                let statement = importer
                    .factory(emit_context)
                    .new_variable_statement(var_modifiers, declaration_list);

                let type_arguments = self.preserve_optional_type_node_list(
                    source,
                    source.source_type_arguments(extends_clause),
                    *original,
                    importer,
                    emit_context,
                );
                let new_extends_type = importer
                    .factory(emit_context)
                    .update_expression_with_type_arguments_from_store(
                        source,
                        extends_clause,
                        new_id,
                        type_arguments,
                    );
                let extends_parent = source
                    .parent(extends_clause)
                    .expect("extends type should have heritage clause parent");
                let extends_types = importer.factory(emit_context).new_node_list(
                    core::undefined_text_range(),
                    core::undefined_text_range(),
                    vec![new_extends_type],
                );
                let new_extends_clause = importer
                    .factory(emit_context)
                    .update_heritage_clause_from_store(
                        source,
                        extends_parent,
                        source
                            .token(extends_parent)
                            .expect("heritage clause should have a token"),
                        extends_types,
                    );
                let mut heritage_nodes = vec![new_extends_clause];
                if let Some(heritage_clauses) = source.heritage_clauses(*original) {
                    for clause in heritage_clauses.iter() {
                        if source.kind(clause) != ast::Kind::HeritageClause
                            || source.token(clause) != Some(ast::Kind::ExtendsKeyword)
                        {
                            heritage_nodes.push(importer.preserve_node(emit_context, clause));
                        }
                    }
                }
                let heritage_clauses = importer.factory(emit_context).new_node_list(
                    core::undefined_text_range(),
                    core::undefined_text_range(),
                    heritage_nodes,
                );
                let updated = importer
                    .factory(emit_context)
                    .update_class_declaration_from_store(
                        source,
                        *original,
                        modifiers,
                        name,
                        type_parameters,
                        heritage_clauses,
                        members,
                    );
                return vec![statement, updated];
            }
        }

        let heritage_clauses = self.transform_heritage_clauses(
            source,
            source.heritage_clauses(*original),
            importer,
            emit_context,
            *original,
        );

        let updated = importer
            .factory(emit_context)
            .update_class_declaration_from_store(
                source,
                *original,
                modifiers,
                name,
                type_parameters,
                heritage_clauses,
                members,
            );
        vec![updated]
    }

    fn collect_parameter_properties(
        &mut self,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> Vec<ast::Node> {
        let Some(ctor) = ast::get_first_constructor_with_body(source, *original) else {
            return Vec::new();
        };
        let Some(parameters) = source.source_parameters(ctor) else {
            return Vec::new();
        };

        let mut parameter_properties = Vec::new();
        for param in parameters.iter() {
            if !ast::has_syntactic_modifier(
                source,
                param,
                ast::ModifierFlags::PARAMETER_PROPERTY_MODIFIER,
            ) {
                continue;
            }
            if source
                .name(param)
                .is_some_and(|name| ast::is_identifier(source, name))
            {
                let modifiers = self.ensure_modifiers(source, &param, importer, emit_context);
                let name = importer.preserve_optional_node(emit_context, source.name(param));
                let question_token =
                    importer.preserve_optional_node(emit_context, source.question_token(param));
                let type_node =
                    self.ensure_type(source, &param, importer, emit_context, *original, false);
                let initializer = self.ensure_no_initializer(source, &param, emit_context);
                let updated = importer.factory(emit_context).new_property_declaration(
                    modifiers,
                    name,
                    question_token,
                    type_node,
                    initializer,
                );
                emit_context.set_original(&updated, &param);
                emit_context.assign_comment_range(&updated, &param);
                parameter_properties.push(updated);
            } else if let Some(name) = source.name(param) {
                // Pattern - this is currently an error, but we emit declarations for it somewhat correctly
                parameter_properties.extend(self.walk_binding_pattern(
                    source,
                    name,
                    param,
                    importer,
                    emit_context,
                ));
            }
        }
        parameter_properties
    }

    fn walk_binding_pattern(
        &mut self,
        source: &ast::AstStore,
        pattern: ast::Node,
        param: ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> Vec<ast::Node> {
        let Some(elements) = source.elements(pattern) else {
            return Vec::new();
        };
        let mut elems = Vec::new();
        for elem in elements.iter() {
            if ast::is_omitted_expression(source, elem) {
                continue;
            }
            let Some(name) = source.name(elem) else {
                continue;
            };
            if ast::is_binding_pattern(source, name) {
                elems.extend(self.walk_binding_pattern(
                    source,
                    name,
                    param,
                    importer,
                    emit_context,
                ));
                continue;
            }
            let modifiers = self.ensure_modifiers(source, &param, importer, emit_context);
            let name = importer.preserve_node(emit_context, name);
            let type_node = self.ensure_type(source, &elem, importer, emit_context, param, false);
            elems.push(
                importer
                    .factory(emit_context)
                    .new_property_declaration(modifiers, name, None, type_node, None),
            );
        }
        elems
    }

    fn transform_enum_declaration(
        &mut self,
        file: &ast::SourceFile,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> ast::Node {
        let modifiers = self.ensure_modifiers(source, original, importer, emit_context);
        let name = importer.preserve_optional_node(emit_context, source.name(*original));
        let member_list = source
            .members(*original)
            .expect("enum declaration should have members");
        let members = member_list
            .iter()
            .filter_map(|member| {
                if self.should_strip_internal(file, &member, emit_context) {
                    return None;
                }
                // Rewrite enum values to their constants, if available
                let enum_value = self.host.get_enum_member_value(member);
                if self.compiler_options.isolated_declarations.is_true()
                    && source.initializer(member).is_some()
                    && enum_value.has_external_references
                    // This will be its own compiler error instead, so don't report.
                    && source
                        .name(member)
                        .is_none_or(|name| !ast::is_computed_property_name(source, name))
                {
                    let loc = scanner::get_error_range_for_node(file, &member);
                    self.diagnostics.push(ast::new_diagnostic(
                        Some(file),
                        loc,
                        &diagnostic_messages::Enum_member_initializers_must_be_computable_without_references_to_external_symbols_with_isolatedDeclarations,
                        &[],
                    ));
                }

                let initializer = match enum_value.value {
                    evaluator::Value::Number(value) if value.0 >= 0.0 => Some(
                        importer
                            .factory(emit_context)
                            .new_numeric_literal(value.to_string(), ast::TokenFlags::NONE),
                    ),
                    evaluator::Value::Number(value) => {
                        let operand = importer
                            .factory(emit_context)
                            .new_numeric_literal((-value).to_string(), ast::TokenFlags::NONE);
                        Some(
                            importer
                                .factory(emit_context)
                                .new_prefix_unary_expression(ast::Kind::MinusToken, operand),
                        )
                    }
                    evaluator::Value::String(value) => Some(
                        importer
                            .factory(emit_context)
                            .new_string_literal(value, ast::TokenFlags::NONE),
                    ),
                    _ => None,
                };
                let name = importer.preserve_optional_node(emit_context, source.name(member));
                Some(importer.factory(emit_context).update_enum_member_from_store(
                    source,
                    member,
                    name,
                    initializer,
                ))
            })
            .collect::<Vec<_>>();
        let members = importer.factory(emit_context).new_node_list(
            member_list.loc(),
            member_list.range(),
            members,
        );
        importer
            .factory(emit_context)
            .update_enum_declaration_from_store(source, *original, modifiers, name, members)
    }

    fn create_expando_function_error_diagnostics(
        &mut self,
        source_file: &impl ast::SourceFileStoreLike,
        diagnostic_file: ast::DiagnosticFile,
        source: &ast::AstStore,
        node: ast::Node,
    ) -> Vec<ast::Diagnostic> {
        self.host
            .get_properties_of_container_function(node)
            .into_iter()
            .filter_map(|property| {
                let value_declaration = self.host.get_symbol_value_declaration(property)?;
                if !ast::is_expando_property_declaration(source, Some(value_declaration)) {
                    return None;
                }
                let error_target = if ast::is_binary_expression(source, value_declaration) {
                    source.left(value_declaration).unwrap_or(value_declaration)
                } else {
                    value_declaration
                };
                Some(ast::new_diagnostic_with_file(
                    Some(diagnostic_file.clone()),
                    scanner::get_error_range_for_node(source_file, &error_target),
                    &diagnostic_messages::Assigning_properties_to_functions_without_declaring_them_is_not_supported_with_isolatedDeclarations_Add_an_explicit_declaration_for_the_properties_assigned_to_this_function,
                    &[],
                ))
            })
            .collect()
    }

    fn report_expando_function_errors(
        &mut self,
        file: &ast::SourceFile,
        source: &ast::AstStore,
        node: ast::Node,
    ) {
        let diagnostics = self.create_expando_function_error_diagnostics(
            file,
            file.diagnostic_file(),
            source,
            node,
        );
        self.diagnostics.extend(diagnostics);
    }

    fn transform_function_declaration(
        &mut self,
        file: &ast::SourceFile,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        enclosing_declaration: ast::Node,
    ) -> Vec<ast::Node> {
        if !self.host.is_declaration_visible(*original) {
            return Vec::new();
        }

        // Elide implementation signatures from overload sets
        if self.host.is_implementation_of_overload(*original) {
            return Vec::new();
        }

        if self.host.is_expando_function_declaration(*original)
            && self.compiler_options.isolated_declarations.is_true()
        {
            self.report_expando_function_errors(file, source, *original);
        }

        let modifiers = self.ensure_modifiers(source, original, importer, emit_context);
        let name = importer.preserve_optional_node(emit_context, source.name(*original));
        let type_parameters = self.preserve_type_parameters(
            source,
            source.type_parameters(*original),
            *original,
            importer,
            emit_context,
        );
        let parameters = self.update_param_list(source, original, importer, emit_context);
        let type_node = self.ensure_declaration_type(
            source,
            original,
            importer,
            emit_context,
            enclosing_declaration,
        );
        let clean = importer
            .factory(emit_context)
            .update_function_declaration_from_store(
                source,
                *original,
                modifiers,
                None,
                name,
                type_parameters,
                parameters,
                type_node,
                None,
                None,
            );

        vec![clean]
    }

    fn transform_module_declaration(
        &mut self,
        file: &ast::SourceFile,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> ast::Node {
        let modifiers = self.ensure_modifiers(source, original, importer, emit_context);
        let name = importer.preserve_optional_node(emit_context, source.name(*original));
        let mut keyword = source
            .keyword(*original)
            .expect("module declaration should have a keyword");
        if keyword != ast::Kind::GlobalKeyword
            && source
                .name(*original)
                .is_none_or(|name| !ast::is_string_literal(source, name))
        {
            keyword = ast::Kind::NamespaceKeyword;
        }

        let saved_needs_declare = self.state.needs_declare;
        let saved_strip_export_modifiers = self.state.strip_export_modifiers;
        self.state.needs_declare = false;

        let body = source.body(*original).map(|body| {
            if source.kind(body) != ast::Kind::ModuleBlock {
                return self.transform_module_declaration(
                    file,
                    source,
                    &body,
                    importer,
                    emit_context,
                );
            }

            let statements = source
                .statements(body)
                .expect("module block should have statements");
            let old_needs_scope_fix_marker = self.state.needs_scope_fix_marker;
            let old_result_has_scope_marker = self.state.result_has_scope_marker;
            self.state.needs_scope_fix_marker = false;
            self.state.result_has_scope_marker = false;
            let statement_pairs = statements
                .iter()
                .map(|statement| {
                    let transformed = self.transform_declaration_statement(
                        file,
                        source,
                        &statement,
                        importer,
                        emit_context,
                        *original,
                    );
                    (statement, transformed)
                })
                .collect::<Vec<_>>();
            let mut transformed = self.transform_and_replace_late_painted_statements(
                file,
                source,
                statement_pairs,
                importer,
                emit_context,
                *original,
            );
            let is_ambient_module = source.flags(*original).contains(ast::NodeFlags::AMBIENT);
            if is_ambient_module {
                self.state.needs_scope_fix_marker = false;
            }
            if !ast::is_global_scope_augmentation(source, *original)
                && !has_scope_marker(importer.factory(emit_context).store(), &transformed)
                && !self.state.result_has_scope_marker
            {
                if self.state.needs_scope_fix_marker {
                    transformed.push(create_empty_exports(importer.factory(emit_context)));
                } else {
                    self.state.strip_export_modifiers = true;
                    transformed = transformed
                        .into_iter()
                        .map(|statement| {
                            self.strip_export_modifiers_from_statement(
                                statement,
                                importer,
                                emit_context,
                            )
                        })
                        .collect();
                }
            }
            self.state.needs_scope_fix_marker = old_needs_scope_fix_marker;
            self.state.result_has_scope_marker = old_result_has_scope_marker;
            self.state.strip_export_modifiers = saved_strip_export_modifiers;

            let transformed_is_empty = transformed.is_empty();
            let statements = importer.factory(emit_context).new_node_list(
                statements.loc(),
                statements.range(),
                transformed,
            );
            let updated_body = importer
                .factory(emit_context)
                .update_module_block_from_store(source, body, statements);
            if transformed_is_empty {
                emit_context.mark_emit_node(&body, printer::EF_MULTI_LINE);
                emit_context.mark_emit_node(&updated_body, printer::EF_MULTI_LINE);
            }
            updated_body
        });

        self.state.needs_declare = saved_needs_declare;
        self.state.strip_export_modifiers = saved_strip_export_modifiers;

        let updated = importer
            .factory(emit_context)
            .update_module_declaration_from_store(
                source, *original, modifiers, keyword, name, body,
            );
        updated
    }

    fn strip_export_modifiers_from_statement(
        &mut self,
        statement: ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> ast::Node {
        let output_store = importer.factory(emit_context).store();
        if ast::is_import_equals_declaration(output_store, statement)
            || !ast::can_have_modifiers(output_store, statement)
            || ast::has_syntactic_modifier(output_store, statement, ast::ModifierFlags::DEFAULT)
        {
            // `export import` statements should remain as-is, as imports are _not_ implicitly exported in an ambient namespace
            // Likewise, `export default` classes and the like and just be `default`, so we preserve their `export` modifiers, too
            return statement;
        }
        let old_flags = ast::get_combined_modifier_flags(output_store, statement);
        if !old_flags.intersects(ast::ModifierFlags::EXPORT) {
            return statement;
        }
        let new_flags = old_flags & !ast::ModifierFlags::EXPORT;
        let modifier_nodes = ast::create_modifiers_from_modifier_flags(new_flags, |kind| {
            importer.factory(emit_context).new_modifier(kind)
        });
        let modifiers = importer.factory(emit_context).new_modifier_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            modifier_nodes,
            new_flags,
        );
        ast::replace_modifiers(importer.factory(emit_context), statement, Some(modifiers))
    }

    fn mark_aliases_visible(&mut self, aliases: &[ast::Node]) {
        let mut late_marked_statements = self.late_marked_statements.borrow_mut();
        for alias in aliases {
            if !late_marked_statements.contains(alias) {
                late_marked_statements.push(*alias);
            }
        }
    }

    fn handle_symbol_accessibility_result(
        &mut self,
        source: &ast::AstStore,
        declaration: ast::Node,
        result: printer::SymbolAccessibilityResult,
    ) {
        if result.accessibility == printer::SymbolAccessibility::Accessible {
            self.mark_aliases_visible(&result.aliases_to_make_visible);
            return;
        }

        let accessibility = match result.accessibility {
            printer::SymbolAccessibility::NotAccessible => {
                nodebuilder::SymbolAccessibility::NotAccessible
            }
            printer::SymbolAccessibility::CannotBeNamed => {
                nodebuilder::SymbolAccessibility::CannotBeNamed
            }
            _ => return,
        };
        let diagnostics = Rc::new(RefCell::new(Vec::new()));
        let mut tracker = DeclarationDiagnosticTracker::new(
            diagnostics.clone(),
            self.late_marked_statements.clone(),
            self.compiler_options.isolated_declarations.is_true(),
            source,
            declaration,
        );
        nodebuilder::SymbolTracker::report_symbol_accessibility_error(
            &mut tracker,
            accessibility,
            &result.error_symbol_name,
            &result.error_module_name,
            result.error_node,
        );
        self.diagnostics.extend(diagnostics.borrow_mut().drain(..));
    }

    fn check_entity_name_visibility(
        &mut self,
        source: &ast::AstStore,
        entity_name: ast::Node,
        enclosing_declaration: ast::Node,
        diagnostic_declaration: ast::Node,
    ) {
        let result = self
            .host
            .is_entity_name_visible(entity_name, enclosing_declaration);
        self.handle_symbol_accessibility_result(source, diagnostic_declaration, result);
    }

    fn check_name(
        &mut self,
        source: &ast::AstStore,
        node: ast::Node,
        enclosing_declaration: ast::Node,
    ) {
        let Some(name) = ast::get_name_of_declaration(source, Some(node)) else {
            return;
        };
        debug_assert!(ast::has_dynamic_name(source, node)); // Should only be called with dynamic names
        let entity_name = source.expression(name).unwrap_or(name);
        self.check_entity_name_visibility(source, entity_name, enclosing_declaration, node);
    }

    fn check_type_node_visibility(
        &mut self,
        source: &ast::AstStore,
        node: ast::Node,
        enclosing_declaration: ast::Node,
        diagnostic_declaration: ast::Node,
    ) {
        match source.kind(node) {
            ast::Kind::TypeReference => {
                if let Some(type_name) = source.type_name(node) {
                    self.check_entity_name_visibility(
                        source,
                        type_name,
                        enclosing_declaration,
                        diagnostic_declaration,
                    );
                }
            }
            ast::Kind::ExpressionWithTypeArguments => {
                if let Some(expression) = source.expression(node) {
                    if ast::is_entity_name(source, &expression)
                        || ast::is_entity_name_expression(source, expression)
                    {
                        self.check_entity_name_visibility(
                            source,
                            expression,
                            enclosing_declaration,
                            diagnostic_declaration,
                        );
                    }
                }
            }
            ast::Kind::TypeQuery => {
                if let Some(expr_name) = source.expr_name(node) {
                    self.check_entity_name_visibility(
                        source,
                        expr_name,
                        enclosing_declaration,
                        diagnostic_declaration,
                    );
                }
            }
            _ => {}
        }

        let _ = source.for_each_present_child(node, |child| {
            self.check_type_node_visibility(
                source,
                child,
                enclosing_declaration,
                diagnostic_declaration,
            );
            std::ops::ControlFlow::Continue(())
        });
    }

    fn preserve_declaration_type_node(
        &mut self,
        source: &ast::AstStore,
        node: ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        enclosing_declaration: ast::Node,
    ) -> ast::Node {
        let result = self.transform_declaration_type_node(
            source,
            node,
            importer,
            emit_context,
            enclosing_declaration,
        );
        self.copy_declaration_type_node_emit_flags(source, node, result, emit_context);
        result
    }

    fn transform_declaration_type_node(
        &mut self,
        source: &ast::AstStore,
        node: ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        enclosing_declaration: ast::Node,
    ) -> ast::Node {
        if self.is_nullable_type_marker_node(source, node) {
            return self.transform_nullable_type_marker(
                source,
                node,
                importer,
                emit_context,
                enclosing_declaration,
            );
        }

        self.transform_declaration_type_node_worker(
            source,
            node,
            importer,
            emit_context,
            enclosing_declaration,
        )
    }

    fn transform_declaration_type_node_worker(
        &mut self,
        source: &ast::AstStore,
        node: ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        enclosing_declaration: ast::Node,
    ) -> ast::Node {
        match source.kind(node) {
            ast::Kind::TypeLiteral => {
                let members = source
                    .members(node)
                    .expect("type literal should have members");
                let member_nodes = members
                    .iter()
                    .filter_map(|member| {
                        self.transform_interface_member(
                            None,
                            source,
                            &member,
                            importer,
                            emit_context,
                            enclosing_declaration,
                        )
                    })
                    .collect::<Vec<_>>();
                let members = importer.factory(emit_context).new_node_list(
                    members.loc(),
                    members.range(),
                    member_nodes,
                );
                importer
                    .factory(emit_context)
                    .update_type_literal_node_from_store(source, node, members)
            }
            ast::Kind::MappedType => {
                // handle missing template type nodes, since the printer does not
                let type_node = match source.r#type(node) {
                    Some(type_node) => self.transform_declaration_type_node(
                        source,
                        type_node,
                        importer,
                        emit_context,
                        enclosing_declaration,
                    ),
                    None => importer
                        .factory(emit_context)
                        .new_keyword_type_node(ast::Kind::AnyKeyword),
                };
                let readonly_token =
                    importer.preserve_optional_node(emit_context, source.readonly_token(node));
                let type_parameter =
                    importer.preserve_optional_node(emit_context, source.type_parameter(node));
                let name_type =
                    importer.preserve_optional_node(emit_context, source.name_type(node));
                let question_token =
                    importer.preserve_optional_node(emit_context, source.question_token(node));
                importer
                    .factory(emit_context)
                    .update_mapped_type_node_from_store(
                        source,
                        node,
                        readonly_token,
                        type_parameter,
                        name_type,
                        question_token,
                        type_node,
                        None,
                    )
            }
            ast::Kind::TupleType => {
                let elements = source
                    .elements(node)
                    .expect("tuple type nodes should have elements");
                let element_nodes = elements
                    .iter()
                    .map(|element| {
                        self.transform_declaration_type_node(
                            source,
                            element,
                            importer,
                            emit_context,
                            enclosing_declaration,
                        )
                    })
                    .collect::<Vec<_>>();
                let elements = importer.factory(emit_context).new_node_list(
                    elements.loc(),
                    elements.range(),
                    element_nodes,
                );
                importer
                    .factory(emit_context)
                    .update_tuple_type_node_from_store(source, node, elements)
            }
            ast::Kind::NamedTupleMember => {
                let dot_dot_dot_token =
                    importer.preserve_optional_node(emit_context, source.dot_dot_dot_token(node));
                let name = importer.preserve_optional_node(emit_context, source.name(node));
                let question_token =
                    importer.preserve_optional_node(emit_context, source.question_token(node));
                let type_node = source.r#type(node).map(|type_node| {
                    self.transform_declaration_type_node(
                        source,
                        type_node,
                        importer,
                        emit_context,
                        enclosing_declaration,
                    )
                });
                importer
                    .factory(emit_context)
                    .update_named_tuple_member_from_store(
                        source,
                        node,
                        dot_dot_dot_token,
                        name,
                        question_token,
                        type_node,
                    )
            }
            ast::Kind::OptionalType => {
                let type_node = source.r#type(node).map(|type_node| {
                    self.transform_declaration_type_node(
                        source,
                        type_node,
                        importer,
                        emit_context,
                        enclosing_declaration,
                    )
                });
                importer
                    .factory(emit_context)
                    .update_optional_type_node_from_store(source, node, type_node)
            }
            ast::Kind::RestType => {
                let type_node = source.r#type(node).map(|type_node| {
                    self.transform_declaration_type_node(
                        source,
                        type_node,
                        importer,
                        emit_context,
                        enclosing_declaration,
                    )
                });
                importer
                    .factory(emit_context)
                    .update_rest_type_node_from_store(source, node, type_node)
            }
            ast::Kind::ParenthesizedType => {
                let type_node = source.r#type(node).map(|type_node| {
                    self.transform_declaration_type_node(
                        source,
                        type_node,
                        importer,
                        emit_context,
                        enclosing_declaration,
                    )
                });
                importer
                    .factory(emit_context)
                    .update_parenthesized_type_node_from_store(source, node, type_node)
            }
            ast::Kind::ArrayType => {
                let element_type = source.element_type(node).map(|type_node| {
                    self.transform_declaration_type_node(
                        source,
                        type_node,
                        importer,
                        emit_context,
                        enclosing_declaration,
                    )
                });
                importer
                    .factory(emit_context)
                    .update_array_type_node_from_store(source, node, element_type)
            }
            ast::Kind::FunctionType => {
                let type_parameters = self.preserve_type_parameters(
                    source,
                    source.type_parameters(node),
                    node,
                    importer,
                    emit_context,
                );
                let parameters = self.update_param_list(source, &node, importer, emit_context);
                let type_node = source.r#type(node).map(|type_node| {
                    self.transform_declaration_type_node(
                        source,
                        type_node,
                        importer,
                        emit_context,
                        enclosing_declaration,
                    )
                });
                importer
                    .factory(emit_context)
                    .update_function_type_node_from_store(
                        source,
                        node,
                        type_parameters,
                        parameters,
                        type_node,
                    )
            }
            ast::Kind::ConstructorType => {
                let modifiers = self.ensure_modifiers(source, &node, importer, emit_context);
                let type_parameters = self.preserve_type_parameters(
                    source,
                    source.type_parameters(node),
                    node,
                    importer,
                    emit_context,
                );
                let parameters = self.update_param_list(source, &node, importer, emit_context);
                let type_node = source.r#type(node).map(|type_node| {
                    self.transform_declaration_type_node(
                        source,
                        type_node,
                        importer,
                        emit_context,
                        enclosing_declaration,
                    )
                });
                importer
                    .factory(emit_context)
                    .update_constructor_type_node_from_store(
                        source,
                        node,
                        modifiers,
                        type_parameters,
                        parameters,
                        type_node,
                    )
            }
            ast::Kind::ConditionalType => {
                let check_type = source.check_type(node).map(|type_node| {
                    self.transform_declaration_type_node(
                        source,
                        type_node,
                        importer,
                        emit_context,
                        enclosing_declaration,
                    )
                });
                let extends_type = source.extends_type(node).map(|type_node| {
                    self.transform_declaration_type_node(
                        source,
                        type_node,
                        importer,
                        emit_context,
                        enclosing_declaration,
                    )
                });
                let true_type = source.true_type(node).map(|type_node| {
                    self.transform_declaration_type_node(
                        source,
                        type_node,
                        importer,
                        emit_context,
                        enclosing_declaration,
                    )
                });
                let false_type = source.false_type(node).map(|type_node| {
                    self.transform_declaration_type_node(
                        source,
                        type_node,
                        importer,
                        emit_context,
                        enclosing_declaration,
                    )
                });
                importer
                    .factory(emit_context)
                    .update_conditional_type_node_from_store(
                        source,
                        node,
                        check_type,
                        extends_type,
                        true_type,
                        false_type,
                    )
            }
            ast::Kind::UnionType | ast::Kind::IntersectionType => {
                let types = source
                    .types(node)
                    .expect("union and intersection type nodes should have types");
                let type_nodes = types
                    .iter()
                    .map(|type_node| {
                        self.transform_declaration_type_node(
                            source,
                            type_node,
                            importer,
                            emit_context,
                            enclosing_declaration,
                        )
                    })
                    .collect::<Vec<_>>();
                let types = importer.factory(emit_context).new_node_list(
                    types.loc(),
                    types.range(),
                    type_nodes,
                );
                if source.kind(node) == ast::Kind::UnionType {
                    importer
                        .factory(emit_context)
                        .update_union_type_node_from_store(source, node, types)
                } else {
                    importer
                        .factory(emit_context)
                        .update_intersection_type_node_from_store(source, node, types)
                }
            }
            ast::Kind::TypeParameter => self.transform_type_parameter_declaration(
                source,
                &node,
                importer,
                emit_context,
                enclosing_declaration,
            ),
            _ => importer.preserve_node(emit_context, node),
        }
    }

    fn transform_nullable_type_marker(
        &mut self,
        source: &ast::AstStore,
        node: ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        enclosing_declaration: ast::Node,
    ) -> ast::Node {
        let type_node = self.transform_declaration_type_node_worker(
            source,
            node,
            importer,
            emit_context,
            enclosing_declaration,
        );
        let null_literal = importer
            .factory(emit_context)
            .new_keyword_expression(ast::Kind::NullKeyword);
        let null_type = importer
            .factory(emit_context)
            .new_literal_type_node(null_literal);
        let types = importer.factory(emit_context).new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![type_node, null_type],
        );
        let replacement = importer.factory(emit_context).new_union_type_node(types);
        emit_context.set_original(&replacement, &node);
        replacement
    }

    fn is_nullable_type_marker_node(&self, source: &ast::AstStore, node: ast::Node) -> bool {
        if matches!(
            source.kind(node),
            ast::Kind::OptionalType | ast::Kind::RestType | ast::Kind::NamedTupleMember
        ) {
            return false;
        }

        if source
            .parent(node)
            .is_some_and(|parent| source.kind(parent) == ast::Kind::OptionalType)
        {
            return false;
        }

        let Some(marker_pos) = self.nullable_type_marker_pos(source, node) else {
            return false;
        };

        let mut child_has_marker = false;
        let _ = source.for_each_present_child(node, |child| {
            if self.nullable_type_marker_pos(source, child) == Some(marker_pos) {
                child_has_marker = true;
                std::ops::ControlFlow::Break(())
            } else {
                std::ops::ControlFlow::Continue(())
            }
        });

        !child_has_marker
    }

    fn nullable_type_marker_pos(&self, source: &ast::AstStore, node: ast::Node) -> Option<i32> {
        let loc = source.loc(node);
        if loc.pos() < 0 || loc.end() <= loc.pos() {
            return None;
        }

        let source_file = ast::get_source_file_of_node(source, Some(node))?;
        let source_file = source.source_file_view(source_file);
        let text = source_file.text();
        let start = loc.pos() as usize;
        let end = loc.end() as usize;
        if end > text.len() {
            return None;
        }

        let bytes = text.as_bytes();
        if bytes.get(start) == Some(&b'?') {
            Some(loc.pos())
        } else if bytes.get(end - 1) == Some(&b'?') {
            Some(loc.end() - 1)
        } else {
            None
        }
    }

    fn copy_declaration_type_node_emit_flags(
        &mut self,
        source: &ast::AstStore,
        source_node: ast::Node,
        output_node: ast::Node,
        emit_context: &mut printer::EmitContext,
    ) {
        if source.kind(source_node) != emit_context.factory.node_factory.store().kind(output_node) {
            return;
        }

        if source.kind(source_node) == ast::Kind::TupleType
            && crate::utilities::is_original_node_single_line(
                source,
                emit_context,
                Some(source_node),
            )
        {
            emit_context.mark_emit_node(&output_node, printer::EF_SINGLE_LINE);
        }

        let mut source_children = Vec::new();
        let _ = source.for_each_present_child(source_node, |child| {
            source_children.push(child);
            std::ops::ControlFlow::Continue(())
        });

        let output_children = {
            let output_store = emit_context.factory.node_factory.store();
            let mut output_children = Vec::new();
            let _ = output_store.for_each_present_child(output_node, |child| {
                output_children.push(child);
                std::ops::ControlFlow::Continue(())
            });
            output_children
        };

        for (source_child, output_child) in source_children.into_iter().zip(output_children) {
            self.copy_declaration_type_node_emit_flags(
                source,
                source_child,
                output_child,
                emit_context,
            );
        }
    }

    fn transform_expression_statement(
        &mut self,
        file: &ast::SourceFile,
        source: &ast::AstStore,
        statement: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        enclosing_declaration: ast::Node,
    ) -> Vec<ast::Node> {
        let Some(expression) = source.expression(*statement) else {
            return Vec::new();
        };
        match ast::get_assignment_declaration_kind(source, expression) {
            ast::JSDeclarationKind::ModuleExports => {
                if self
                    .host
                    .source_file_common_js_module_indicator(file)
                    .is_some()
                {
                    let Some(right) = source.right(expression) else {
                        return Vec::new();
                    };
                    return self.transform_export_assignment_like(
                        source,
                        *statement,
                        expression,
                        right,
                        true, /*isExportEquals*/
                        importer,
                        emit_context,
                    );
                }
            }
            ast::JSDeclarationKind::ExportsProperty => {
                if self
                    .host
                    .source_file_common_js_module_indicator(file)
                    .is_some()
                {
                    let Some(left) = source.left(expression) else {
                        return Vec::new();
                    };
                    let Some(name) = ast::get_element_or_property_access_name(source, left) else {
                        return Vec::new();
                    };
                    let emitted_name = self.get_name_expression_preferring_identifier(
                        source,
                        name,
                        importer,
                        emit_context,
                    );
                    return self.transform_common_js_export(
                        source,
                        expression,
                        name,
                        emitted_name,
                        importer,
                        emit_context,
                    );
                }
            }
            ast::JSDeclarationKind::Property => {
                return self.transform_expando_assignment(
                    source,
                    expression,
                    importer,
                    emit_context,
                    enclosing_declaration,
                );
            }
            ast::JSDeclarationKind::ObjectDefinePropertyExports => {
                if self
                    .host
                    .source_file_common_js_module_indicator(file)
                    .is_some()
                {
                    let Some(arguments) = source.arguments(expression) else {
                        return Vec::new();
                    };
                    let Some(name) = arguments.iter().nth(1) else {
                        return Vec::new();
                    };
                    let emitted_name = self.get_name_expression_preferring_identifier(
                        source,
                        name,
                        importer,
                        emit_context,
                    );
                    return self.transform_common_js_export(
                        source,
                        expression,
                        name,
                        emitted_name,
                        importer,
                        emit_context,
                    );
                }
            }
            _ => {}
        }
        Vec::new()
    }

    fn get_name_expression_preferring_identifier(
        &mut self,
        source: &ast::AstStore,
        name_expr: ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> ast::Node {
        if ast::is_string_literal_like(source, name_expr)
            && scanner::is_identifier_text(&source.text(name_expr), core::LanguageVariant::Standard)
        {
            let result = emit_context
                .factory
                .node_factory
                .new_identifier(source.text(name_expr));
            let kw_kind =
                scanner::identifier_to_keyword_kind(emit_context.store_for_node(result), result);
            // keep keywords as strings, except `default`, which has special reformulations in the transformer
            if kw_kind == ast::Kind::Unknown || kw_kind == ast::Kind::DefaultKeyword {
                return result;
            }
        }
        importer.preserve_node(emit_context, name_expr)
    }

    fn transform_export_assignment_like(
        &mut self,
        source: &ast::AstStore,
        input: ast::Node,
        assignment: ast::Node,
        expression: ast::Node,
        is_export_equals: bool,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> Vec<ast::Node> {
        if source
            .parent(input)
            .is_some_and(|parent| ast::is_source_file(source, parent))
        {
            self.state.result_has_external_module_indicator = true;
        }
        self.state.result_has_scope_marker = true;
        if ast::is_identifier(source, expression) {
            let expression = importer.preserve_node(emit_context, expression);
            let export_assignment = importer.factory(emit_context).new_export_assignment(
                None,
                is_export_equals,
                None,
                expression,
            );
            return vec![export_assignment];
        }

        // expression is non-identifier, create _default typed variable to reference
        let default_name = emit_context.factory.new_unique_name_ex(
            "_default",
            printer::AutoGenerateOptions {
                flags: printer::GeneratedIdentifierFlags::OPTIMISTIC,
                ..Default::default()
            },
        );
        let skipped_expression = ast::skip_parentheses(source, expression);
        let mut initializer = None;
        if ast::is_primitive_literal_value(source, skipped_expression, true) {
            initializer = self
                .host
                .create_literal_const_value(emit_context, assignment);
        }
        let type_node = if initializer.is_none() {
            self.ensure_type_with_error_fallback_node(
                source,
                &assignment,
                importer,
                emit_context,
                assignment,
                false,
                Some(assignment),
            )
        } else {
            None
        };
        let declaration = importer.factory(emit_context).new_variable_declaration(
            default_name.clone(),
            None,
            type_node,
            initializer,
        );
        let declarations = importer.factory(emit_context).new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![declaration],
        );
        let declaration_list = importer
            .factory(emit_context)
            .new_variable_declaration_list(declarations, ast::NodeFlags::CONST);
        let modifiers = if self.state.needs_declare {
            let declare_modifier = importer
                .factory(emit_context)
                .new_modifier(ast::Kind::DeclareKeyword);
            Some(importer.factory(emit_context).new_modifier_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                vec![declare_modifier],
                ast::ModifierFlags::AMBIENT,
            ))
        } else {
            Some(importer.factory(emit_context).new_modifier_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                vec![],
                ast::ModifierFlags::NONE,
            ))
        };
        let declaration_statement = importer
            .factory(emit_context)
            .new_variable_statement(modifiers, declaration_list);
        let export_statement = importer.factory(emit_context).new_export_assignment(
            None,
            is_export_equals,
            None,
            default_name,
        );
        vec![declaration_statement, export_statement]
    }

    fn transform_common_js_export(
        &mut self,
        source: &ast::AstStore,
        input: ast::Node,
        lookup_name: ast::Node,
        name: ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> Vec<ast::Node> {
        self.state.result_has_external_module_indicator = true;
        self.state.result_has_scope_marker = true;
        if self.host.is_common_js_alias_export(input) {
            // export { name }
            // export { source as name }
            let Some(property_name) = source.right(input) else {
                return Vec::new();
            };
            let property_name = if ast::is_identifier(emit_context.store_for_node(name), name)
                && source.text(property_name) == emit_context.store_for_node(name).text(name)
            {
                None
            } else {
                Some(importer.preserve_node(emit_context, property_name))
            };
            let export_specifier = importer.factory(emit_context).new_export_specifier(
                false,
                property_name,
                Some(name),
            );
            let export_specifiers = importer.factory(emit_context).new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                vec![export_specifier],
            );
            let named_exports = importer
                .factory(emit_context)
                .new_named_exports(export_specifiers);
            return vec![importer.factory(emit_context).new_export_declaration(
                None::<ast::ModifierList>,
                false,
                Some(named_exports),
                None::<ast::Node>,
                None::<ast::Node>,
            )];
        } else if ast::is_identifier(emit_context.store_for_node(name), name) {
            let name_text = emit_context.store_for_node(name).text(name);
            if name_text == "default" {
                // const _default: Type; export default _default;
                let default_name = emit_context.factory.new_unique_name_ex(
                    "_default",
                    printer::AutoGenerateOptions {
                        flags: printer::GeneratedIdentifierFlags::OPTIMISTIC,
                        ..Default::default()
                    },
                );
                let type_node = self.ensure_type_with_error_fallback_node(
                    source,
                    &input,
                    importer,
                    emit_context,
                    input,
                    false,
                    Some(input),
                );
                let declaration = importer.factory(emit_context).new_variable_declaration(
                    default_name.clone(),
                    None,
                    type_node,
                    None,
                );
                let declarations = importer.factory(emit_context).new_node_list(
                    core::undefined_text_range(),
                    core::undefined_text_range(),
                    vec![declaration],
                );
                let declaration_list = importer
                    .factory(emit_context)
                    .new_variable_declaration_list(declarations, ast::NodeFlags::CONST);
                let modifiers = if self.state.needs_declare {
                    let declare_modifier = importer
                        .factory(emit_context)
                        .new_modifier(ast::Kind::DeclareKeyword);
                    Some(importer.factory(emit_context).new_modifier_list(
                        core::undefined_text_range(),
                        core::undefined_text_range(),
                        vec![declare_modifier],
                        ast::ModifierFlags::AMBIENT,
                    ))
                } else {
                    Some(importer.factory(emit_context).new_modifier_list(
                        core::undefined_text_range(),
                        core::undefined_text_range(),
                        vec![],
                        ast::ModifierFlags::NONE,
                    ))
                };
                let declaration_statement = importer
                    .factory(emit_context)
                    .new_variable_statement(modifiers, declaration_list);
                let export_assignment = importer.factory(emit_context).new_export_assignment(
                    None,
                    false,
                    None,
                    default_name,
                );
                return vec![declaration_statement, export_assignment];
            } else if self.host.get_referenced_value_declaration(lookup_name) == Some(input)
                || self
                    .host
                    .get_referenced_value_declaration(lookup_name)
                    .is_none()
            {
                // only inline to a export var if the `name` lookup points at this assignment or nothing - if it points at something else, we must use a temp name
                // export var name: Type
                let type_node = self.ensure_type_with_error_fallback_node(
                    source,
                    &input,
                    importer,
                    emit_context,
                    input,
                    false,
                    Some(input),
                );
                let declaration = importer
                    .factory(emit_context)
                    .new_variable_declaration(name, None, type_node, None);
                let declarations = importer.factory(emit_context).new_node_list(
                    core::undefined_text_range(),
                    core::undefined_text_range(),
                    vec![declaration],
                );
                let declaration_list = importer
                    .factory(emit_context)
                    .new_variable_declaration_list(declarations, ast::NodeFlags::NONE);
                let mut modifiers = vec![
                    importer
                        .factory(emit_context)
                        .new_modifier(ast::Kind::ExportKeyword),
                ];
                let mut modifier_flags = ast::ModifierFlags::EXPORT;
                if self.state.needs_declare {
                    modifiers.push(
                        importer
                            .factory(emit_context)
                            .new_modifier(ast::Kind::DeclareKeyword),
                    );
                    modifier_flags |= ast::ModifierFlags::AMBIENT;
                }
                let modifiers = Some(importer.factory(emit_context).new_modifier_list(
                    core::undefined_text_range(),
                    core::undefined_text_range(),
                    modifiers,
                    modifier_flags,
                ));
                return vec![
                    importer
                        .factory(emit_context)
                        .new_variable_statement(modifiers, declaration_list),
                ];
            }
        }
        // const _exported: Type; export {_exported as "name"};
        let exported_name = emit_context.factory.new_unique_name_ex(
            "_exported",
            printer::AutoGenerateOptions {
                flags: printer::GeneratedIdentifierFlags::OPTIMISTIC,
                ..Default::default()
            },
        );
        let type_node = self.ensure_type_with_error_fallback_node(
            source,
            &input,
            importer,
            emit_context,
            input,
            false,
            Some(input),
        );
        let declaration = importer.factory(emit_context).new_variable_declaration(
            exported_name.clone(),
            None,
            type_node,
            None,
        );
        let declarations = importer.factory(emit_context).new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![declaration],
        );
        let declaration_list = importer
            .factory(emit_context)
            .new_variable_declaration_list(declarations, ast::NodeFlags::CONST);
        let modifiers = if self.state.needs_declare {
            let declare_modifier = importer
                .factory(emit_context)
                .new_modifier(ast::Kind::DeclareKeyword);
            Some(importer.factory(emit_context).new_modifier_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                vec![declare_modifier],
                ast::ModifierFlags::AMBIENT,
            ))
        } else {
            Some(importer.factory(emit_context).new_modifier_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                vec![],
                ast::ModifierFlags::NONE,
            ))
        };
        let declaration_statement = importer
            .factory(emit_context)
            .new_variable_statement(modifiers, declaration_list);
        let export_specifier = importer.factory(emit_context).new_export_specifier(
            false,
            Some(exported_name),
            Some(name),
        );
        let export_specifiers = importer.factory(emit_context).new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![export_specifier],
        );
        let named_exports = importer
            .factory(emit_context)
            .new_named_exports(export_specifiers);
        let export_declaration = importer.factory(emit_context).new_export_declaration(
            None::<ast::ModifierList>,
            false,
            Some(named_exports),
            None::<ast::Node>,
            None::<ast::Node>,
        );
        vec![declaration_statement, export_declaration]
    }

    fn transform_expando_assignment(
        &mut self,
        source: &ast::AstStore,
        node: ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        _enclosing_declaration: ast::Node,
    ) -> Vec<ast::Node> {
        if !self.host.is_assignment_declaration(node) {
            return Vec::new();
        }
        let Some(left) = source.left(node) else {
            return Vec::new();
        };
        let ns = ast::get_leftmost_access_expression(source, left);
        if !ast::is_identifier(source, ns) {
            return Vec::new();
        }
        let Some(declaration) = self.host.get_referenced_value_declaration(ns) else {
            return Vec::new();
        };
        if ast::is_variable_declaration(source, declaration) && source.r#type(declaration).is_some()
        {
            return Vec::new();
        }
        if ast::is_function_declaration(source, declaration)
            && source.full_signature(declaration).is_some()
        {
            return Vec::new();
        }
        if ast::is_variable_declaration(source, declaration)
            && !source
                .initializer(declaration)
                .is_some_and(|initializer| ast::is_function_like(source, Some(initializer)))
        {
            return Vec::new(); // We're going to add a type, no need to dupe members with a namespace
        }
        let name = importer
            .factory(emit_context)
            .new_identifier(source.text(ns));
        let property = self.try_get_property_name(source, left);
        if property.is_empty()
            || !scanner::is_identifier_text(&property, core::LANGUAGE_VARIANT_STANDARD)
        {
            return Vec::new();
        }
        if !self.host.is_declaration_visible(declaration) {
            return Vec::new();
        }
        self.transform_expando_host(source, name, declaration, importer, emit_context);

        if ast::is_function_declaration(source, declaration)
            && !self.host.should_emit_function_properties(declaration)
        {
            return Vec::new();
        }

        let is_non_contextual_keyword_name =
            ast::is_non_contextual_keyword(scanner::string_to_token(&property));
        let export_name = if is_non_contextual_keyword_name {
            emit_context
                .factory
                .new_generated_name_for_node(source, &left)
        } else {
            importer
                .factory(emit_context)
                .new_identifier(property.clone())
        };

        let empty_statements = importer.factory(emit_context).new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            Vec::<ast::Node>::new(),
        );
        let empty_body = importer
            .factory(emit_context)
            .new_module_block(empty_statements);
        let synthesized_namespace = importer.factory(emit_context).new_module_declaration(
            None::<ast::ModifierList>,
            ast::Kind::NamespaceKeyword,
            Some(name),
            Some(empty_body),
        );
        emit_context
            .factory
            .node_factory
            .link_emit_synthetic_parent(synthesized_namespace, Some(_enclosing_declaration));
        self.host
            .set_expando_namespace_metadata(synthesized_namespace, declaration, &[]);

        let diagnostics = Rc::new(RefCell::new(Vec::new()));
        let tracker = DeclarationDiagnosticTracker::new(
            diagnostics.clone(),
            self.late_marked_statements.clone(),
            self.compiler_options.isolated_declarations.is_true(),
            source,
            node,
        );
        let type_node = self.host.create_type_of_expression(
            emit_context,
            left,
            synthesized_namespace,
            Box::new(tracker),
        );
        self.diagnostics.extend(diagnostics.borrow_mut().drain(..));
        let Some(type_node) = type_node else {
            return Vec::new();
        };

        let variable_declaration = importer.factory(emit_context).new_variable_declaration(
            export_name,
            None,
            Some(type_node),
            None,
        );
        let declarations = importer.factory(emit_context).new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![variable_declaration],
        );
        let declaration_list = importer
            .factory(emit_context)
            .new_variable_declaration_list(declarations, ast::NodeFlags::NONE);
        let mut statements = vec![
            importer
                .factory(emit_context)
                .new_variable_statement(None::<ast::ModifierList>, declaration_list),
        ];

        if is_non_contextual_keyword_name {
            let property_name = export_name;
            let export_name = importer.factory(emit_context).new_identifier(property);
            let export_specifier = importer.factory(emit_context).new_export_specifier(
                false,
                Some(property_name),
                Some(export_name),
            );
            let export_specifiers = importer.factory(emit_context).new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                vec![export_specifier],
            );
            let named_exports = importer
                .factory(emit_context)
                .new_named_exports(export_specifiers);
            statements.push(importer.factory(emit_context).new_export_declaration(
                None::<ast::ModifierList>,
                false,
                Some(named_exports),
                None::<ast::Node>,
                None::<ast::Node>,
            ));
        }

        let flags = self
            .host
            .get_effective_declaration_flags(declaration, ast::ModifierFlags::ALL);
        let mut modifier_flags = ast::ModifierFlags::AMBIENT;
        if flags.intersects(ast::ModifierFlags::EXPORT) {
            if !flags.intersects(ast::ModifierFlags::DEFAULT) {
                modifier_flags |= ast::ModifierFlags::EXPORT;
            }
            self.state.result_has_scope_marker = true;
            self.state.result_has_external_module_indicator = true;
        }
        let modifiers = ast::create_modifiers_from_modifier_flags(modifier_flags, |kind| {
            importer.factory(emit_context).new_modifier(kind)
        });
        let modifiers = Some(importer.factory(emit_context).new_modifier_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            modifiers,
            modifier_flags,
        ));
        let statements = importer.factory(emit_context).new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            statements,
        );
        let body = importer.factory(emit_context).new_module_block(statements);
        vec![importer.factory(emit_context).new_module_declaration(
            modifiers,
            ast::Kind::NamespaceKeyword,
            Some(name),
            Some(body),
        )]
    }

    fn transform_expando_host(
        &mut self,
        source: &ast::AstStore,
        name: ast::Node,
        declaration: ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) {
        let root = if ast::is_variable_declaration(source, declaration) {
            source
                .parent(declaration)
                .and_then(|parent| source.parent(parent))
                .unwrap_or(declaration)
        } else {
            declaration
        };
        let original = emit_context.most_original(&root);
        let id = emit_context.store_for_node(original).get_node_id(original);
        if self.expando_hosts.contains_key(&id) {
            return;
        }

        let saved_needs_declare = self.state.needs_declare;
        self.state.needs_declare = true;
        let mut modifier_flags = self.ensure_modifier_flags(source, &root, true);
        let default_export = modifier_flags.intersects(ast::ModifierFlags::EXPORT)
            && modifier_flags.intersects(ast::ModifierFlags::DEFAULT);
        self.state.needs_declare = saved_needs_declare;

        if default_export {
            modifier_flags |= ast::ModifierFlags::AMBIENT;
            modifier_flags = modifier_flags ^ ast::ModifierFlags::DEFAULT;
            modifier_flags = modifier_flags ^ ast::ModifierFlags::EXPORT;
        }

        let modifiers = ast::create_modifiers_from_modifier_flags(modifier_flags, |kind| {
            importer.factory(emit_context).new_modifier(kind)
        });
        let modifiers = Some(importer.factory(emit_context).new_modifier_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            modifiers,
            modifier_flags,
        ));
        let mut replacement = Vec::new();

        if ast::is_function_declaration(source, declaration) {
            let asterisk_token =
                importer.preserve_optional_node(emit_context, source.asterisk_token(declaration));
            let declaration_name =
                importer.preserve_optional_node(emit_context, source.name(declaration));
            let type_parameters = self.preserve_optional_node_list(
                source.type_parameters(declaration),
                importer,
                emit_context,
            );
            let parameters = self.update_param_list(source, &declaration, importer, emit_context);
            let type_node = self.ensure_type(
                source,
                &declaration,
                importer,
                emit_context,
                declaration,
                false,
            );
            replacement.push(
                importer
                    .factory(emit_context)
                    .update_function_declaration_from_store(
                        source,
                        declaration,
                        modifiers,
                        asterisk_token,
                        declaration_name,
                        type_parameters,
                        parameters,
                        type_node,
                        None,
                        None,
                    ),
            );
        } else if ast::is_variable_declaration(source, declaration)
            && let Some(fn_node) = source.initializer(declaration)
            && ast::is_function_expression_or_arrow_function(source, fn_node)
        {
            let asterisk_token =
                importer.preserve_optional_node(emit_context, source.asterisk_token(fn_node));
            let type_parameters = self.preserve_optional_node_list(
                source.type_parameters(fn_node),
                importer,
                emit_context,
            );
            let parameters = self.update_param_list(source, &fn_node, importer, emit_context);
            let type_node =
                self.ensure_type(source, &fn_node, importer, emit_context, fn_node, false);
            replacement.push(importer.factory(emit_context).new_function_declaration(
                modifiers,
                asterisk_token,
                Some(name),
                type_parameters,
                parameters,
                type_node,
                None,
                None,
            ));
        } else {
            return;
        }

        if default_export {
            replacement.push(importer.factory(emit_context).new_export_assignment(
                None::<ast::ModifierList>,
                false,
                None::<ast::Node>,
                Some(name),
            ));
        }

        // store host result to be added to the output when it's actually visited
        self.expando_hosts.insert(id, replacement.clone());
        if self.late_statement_replacement_map.contains_key(&id) {
            // host already included in output, revise it
            self.late_statement_replacement_map.insert(id, replacement);
        }
    }

    fn try_get_property_name(&mut self, source: &ast::AstStore, left: ast::Node) -> String {
        if ast::is_element_access_expression(source, left) {
            return self.host.get_element_access_expression_name(left);
        }
        if ast::is_property_access_expression(source, left) {
            return source
                .name(left)
                .map(|name| source.text(name))
                .unwrap_or_default();
        }
        String::new()
    }

    fn transform_class_member(
        &mut self,
        file: &ast::SourceFile,
        source: &ast::AstStore,
        member: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        enclosing_declaration: ast::Node,
    ) -> Option<ast::Node> {
        if self.should_strip_internal(file, member, emit_context) {
            return None;
        }
        if source
            .name(*member)
            .is_some_and(|name| ast::is_private_identifier(source, name))
        {
            return None;
        }
        if ast::has_dynamic_name(source, *member) {
            let name = ast::get_name_of_declaration(source, Some(*member))?;
            let expression = source.expression(name)?;
            if self.compiler_options.isolated_declarations.is_true() {
                if !self
                    .host
                    .is_definitely_reference_to_global_symbol_object(expression)
                {
                    if source.kind(*member) == ast::Kind::MethodDeclaration {
                        let diagnostics = Rc::new(RefCell::new(Vec::new()));
                        if source.r#type(*member).is_none() {
                            let mut tracker = DeclarationDiagnosticTracker::new(
                                diagnostics.clone(),
                                self.late_marked_statements.clone(),
                                true,
                                source,
                                *member,
                            );
                            tracker.add_isolated_declaration_diagnostic(*member);
                        }
                        if let Some(parameters) = source.source_parameters(*member) {
                            for parameter in parameters.iter() {
                                if source.r#type(parameter).is_none() {
                                    let mut tracker = DeclarationDiagnosticTracker::new(
                                        diagnostics.clone(),
                                        self.late_marked_statements.clone(),
                                        true,
                                        source,
                                        parameter,
                                    );
                                    tracker.add_isolated_declaration_diagnostic(parameter);
                                }
                            }
                        }
                        self.diagnostics.extend(diagnostics.borrow_mut().drain(..));
                    }
                    self.add_declaration_diagnostic(
                        file,
                        *member,
                        &diagnostic_messages::Computed_property_names_on_class_or_object_literals_cannot_be_inferred_with_isolatedDeclarations,
                    );
                    return None;
                }
            } else {
                if !ast::is_entity_name_expression(source, expression) {
                    return None;
                }
                if !self.host.is_late_bound(*member) {
                    return None;
                }
            }
            self.check_entity_name_visibility(source, expression, enclosing_declaration, *member);
        }

        // Elide implementation signatures from overload sets
        if ast::is_function_like(source, Some(*member))
            && self.host.is_implementation_of_overload(*member)
        {
            return None;
        }

        // Emit methods which are private as properties with no type information
        if matches!(
            source.kind(*member),
            ast::Kind::MethodDeclaration | ast::Kind::MethodSignature
        ) && ast::has_syntactic_modifier(source, *member, ast::ModifierFlags::PRIVATE)
        {
            if !self.host.is_first_declaration_of_symbol(*member) {
                return None;
            }
            let modifiers = self.ensure_modifiers(source, member, importer, emit_context);
            let name = importer.preserve_optional_node(emit_context, source.name(*member));
            return Some(
                importer
                    .factory(emit_context)
                    .new_property_declaration(modifiers, name, None, None, None),
            );
        }

        match source.kind(*member) {
            ast::Kind::SemicolonClassElement => None,
            ast::Kind::ClassStaticBlockDeclaration => None,
            ast::Kind::GetAccessor => Some(self.transform_get_accessor_declaration(
                source,
                member,
                importer,
                emit_context,
                enclosing_declaration,
            )),
            ast::Kind::SetAccessor => Some(self.transform_set_accessor_declaration(
                source,
                member,
                importer,
                emit_context,
            )),
            ast::Kind::MethodDeclaration => Some(self.transform_method_declaration(
                source,
                member,
                importer,
                emit_context,
                *member,
            )),
            ast::Kind::PropertyDeclaration => Some(self.transform_property_declaration(
                source,
                member,
                importer,
                emit_context,
                enclosing_declaration,
            )),
            ast::Kind::Constructor => {
                Some(self.transform_constructor_declaration(source, member, importer, emit_context))
            }
            _ => Some(importer.preserve_node(emit_context, *member)),
        }
    }

    // collectThisPropertyAssignments finds `this.x = expr` assignments in constructors, methods, and static blocks
    // of JS classes and synthesizes PropertyDeclaration nodes for each unique property name.
    fn collect_this_property_assignments(
        &mut self,
        source: &ast::AstStore,
        input: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> Vec<ast::Node> {
        let mut result = Vec::new();
        let mut seen = Vec::new();
        // Pre-populate seen with existing direct member nodes to avoid duplicates
        if let Some(members) = source.members(*input) {
            for member in members.iter() {
                if source.name(member).is_some() {
                    seen.push(member);
                }
            }
            for member in members.iter() {
                let mut is_static = false;
                let body = match source.kind(member) {
                    ast::Kind::Constructor => source.body(member),
                    ast::Kind::MethodDeclaration => {
                        if ast::has_static_modifier(source, member) {
                            is_static = true;
                        }
                        source.body(member)
                    }
                    ast::Kind::ClassStaticBlockDeclaration => {
                        is_static = true;
                        source.body(member)
                    }
                    _ => continue,
                };
                let Some(body) = body else {
                    continue;
                };
                let Some(statements) = source.statements(body) else {
                    continue;
                };
                for stmt in statements.iter() {
                    if source.kind(stmt) != ast::Kind::ExpressionStatement {
                        continue;
                    }
                    let Some(expr) = source.expression(stmt) else {
                        continue;
                    };
                    if source.kind(expr) != ast::Kind::BinaryExpression {
                        continue;
                    }
                    if ast::get_assignment_declaration_kind(source, expr)
                        != ast::JSDeclarationKind::ThisProperty
                    {
                        continue;
                    }
                    let Some(mut name) = ast::get_name_of_declaration(source, Some(expr)) else {
                        continue;
                    };
                    let Some(base) = self.host.get_referenced_member_value_declaration(expr) else {
                        continue;
                    };
                    if seen.contains(&base) {
                        continue;
                    }
                    seen.push(base);

                    let modifiers = if is_static {
                        let modifier = importer
                            .factory(emit_context)
                            .new_modifier(ast::Kind::StaticKeyword);
                        Some(importer.factory(emit_context).new_modifier_list(
                            core::undefined_text_range(),
                            core::undefined_text_range(),
                            [modifier],
                            ast::ModifierFlags::NONE,
                        ))
                    } else {
                        None
                    };
                    if ast::has_dynamic_name(source, expr) {
                        let is_simple_inlineable =
                            crate::moduletransforms::utilities::is_simple_inlineable_expression(
                                source.kind(name),
                                ast::is_identifier(source, name),
                            );
                        if !is_simple_inlineable {
                            continue;
                        }
                        self.check_name(source, expr, *input);
                        let expression = importer.preserve_node(emit_context, name);
                        name = importer
                            .factory(emit_context)
                            .new_computed_property_name(expression);
                    }
                    if ast::get_text_of_property_name(source, name) == "constructor" {
                        continue;
                    }
                    if ast::is_identifier(source, name)
                        && !scanner::is_identifier_text(
                            &source.text(name),
                            core::LanguageVariant::Standard,
                        )
                    {
                        name = emit_context
                            .factory
                            .new_string_literal_from_node(source, &name);
                    } else {
                        name = importer.preserve_node(emit_context, name);
                    }
                    let type_node =
                        self.ensure_type(source, &expr, importer, emit_context, *input, false);
                    let prop = importer
                        .factory(emit_context)
                        .new_property_declaration(modifiers, name, None, type_node, None);
                    result.push(prop);
                }
            }
        }
        result
    }

    fn transform_interface_member(
        &mut self,
        file: Option<&ast::SourceFile>,
        source: &ast::AstStore,
        member: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        enclosing_declaration: ast::Node,
    ) -> Option<ast::Node> {
        if file.is_some_and(|file| self.should_strip_internal(file, member, emit_context)) {
            return None;
        }
        if source
            .name(*member)
            .is_some_and(|name| ast::is_private_identifier(source, name))
        {
            return None;
        }
        if ast::has_dynamic_name(source, *member) {
            let name = ast::get_name_of_declaration(source, Some(*member))?;
            let expression = source.expression(name)?;
            if self.compiler_options.isolated_declarations.is_true() {
                if !self
                    .host
                    .is_definitely_reference_to_global_symbol_object(expression)
                    && !ast::is_entity_name_expression(source, expression)
                {
                    if let Some(file) = file {
                        self.add_declaration_diagnostic(
                            file,
                            *member,
                            &diagnostic_messages::Computed_properties_must_be_number_or_string_literals_variables_or_dotted_expressions_with_isolatedDeclarations,
                        );
                    }
                    return None;
                }
            } else {
                if !ast::is_entity_name_expression(source, expression) {
                    return None;
                }
                if !self.host.is_late_bound(*member) {
                    return None;
                }
            }
            self.check_entity_name_visibility(source, expression, enclosing_declaration, *member);
        }
        match source.kind(*member) {
            ast::Kind::MethodSignature => Some(self.transform_method_signature_declaration(
                source,
                member,
                importer,
                emit_context,
            )),
            ast::Kind::PropertySignature => Some(self.transform_property_signature_declaration(
                source,
                member,
                importer,
                emit_context,
            )),
            ast::Kind::CallSignature => Some(self.transform_call_signature_declaration(
                source,
                member,
                importer,
                emit_context,
            )),
            ast::Kind::ConstructSignature => Some(self.transform_construct_signature_declaration(
                source,
                member,
                importer,
                emit_context,
            )),
            ast::Kind::IndexSignature => Some(self.transform_index_signature_declaration(
                source,
                member,
                importer,
                emit_context,
            )),
            _ => Some(importer.preserve_node(emit_context, *member)),
        }
    }

    fn transform_method_signature_declaration(
        &mut self,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> ast::Node {
        let modifiers = self.ensure_modifiers(source, original, importer, emit_context);
        let name = importer.preserve_optional_node(emit_context, source.name(*original));
        let postfix_token =
            importer.preserve_optional_node(emit_context, source.postfix_token(*original));
        let type_parameters = self.preserve_type_parameters(
            source,
            source.type_parameters(*original),
            *original,
            importer,
            emit_context,
        );
        let parameters = self.update_param_list(source, original, importer, emit_context);
        let type_node =
            self.ensure_type(source, original, importer, emit_context, *original, false);
        importer
            .factory(emit_context)
            .update_method_signature_declaration_from_store(
                source,
                *original,
                modifiers,
                name,
                postfix_token,
                type_parameters,
                parameters,
                type_node,
            )
    }

    fn transform_call_signature_declaration(
        &mut self,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> ast::Node {
        let type_parameters = self.preserve_type_parameters(
            source,
            source.type_parameters(*original),
            *original,
            importer,
            emit_context,
        );
        let parameters = self.update_param_list(source, original, importer, emit_context);
        let type_node =
            self.ensure_type(source, original, importer, emit_context, *original, false);
        importer
            .factory(emit_context)
            .update_call_signature_declaration_from_store(
                source,
                *original,
                type_parameters,
                parameters,
                type_node,
            )
    }

    fn transform_property_signature_declaration(
        &mut self,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> ast::Node {
        let modifiers = self.ensure_modifiers(source, original, importer, emit_context);
        let name = importer.preserve_optional_node(emit_context, source.name(*original));
        let postfix_token =
            importer.preserve_optional_node(emit_context, source.postfix_token(*original));
        let type_node =
            self.ensure_type(source, original, importer, emit_context, *original, false);
        let initializer = self.ensure_no_initializer(source, original, emit_context);
        importer
            .factory(emit_context)
            .update_property_signature_declaration_from_store(
                source,
                *original,
                modifiers,
                name,
                postfix_token,
                type_node,
                initializer,
            )
    }

    fn transform_index_signature_declaration(
        &mut self,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> ast::Node {
        let modifiers = self.ensure_modifiers(source, original, importer, emit_context);
        let parameters = self.update_param_list(source, original, importer, emit_context);
        let type_node =
            self.ensure_type(source, original, importer, emit_context, *original, false);
        importer
            .factory(emit_context)
            .update_index_signature_declaration_from_store(
                source, *original, modifiers, parameters, type_node,
            )
    }

    fn transform_get_accessor_declaration(
        &mut self,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        enclosing_declaration: ast::Node,
    ) -> ast::Node {
        let modifiers = self.ensure_modifiers(source, original, importer, emit_context);
        let name = importer.preserve_optional_node(emit_context, source.name(*original));
        let parameters = self.update_param_list(source, original, importer, emit_context);
        let type_node = self.ensure_declaration_type(
            source,
            original,
            importer,
            emit_context,
            enclosing_declaration,
        );
        importer
            .factory(emit_context)
            .update_get_accessor_declaration_from_store(
                source, *original, modifiers, name, None, parameters, type_node, None, None,
            )
    }

    fn transform_set_accessor_declaration(
        &mut self,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> ast::Node {
        let modifiers = self.ensure_modifiers(source, original, importer, emit_context);
        let name = importer.preserve_optional_node(emit_context, source.name(*original));
        let parameters = self.update_accessor_param_list(source, original, importer, emit_context);
        importer
            .factory(emit_context)
            .update_set_accessor_declaration_from_store(
                source, *original, modifiers, name, None, parameters, None, None, None,
            )
    }

    fn transform_method_declaration(
        &mut self,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        enclosing_declaration: ast::Node,
    ) -> ast::Node {
        let modifiers = self.ensure_modifiers(source, original, importer, emit_context);
        let name = importer.preserve_optional_node(emit_context, source.name(*original));
        let type_parameters = self.preserve_type_parameters(
            source,
            source.type_parameters(*original),
            *original,
            importer,
            emit_context,
        );
        let parameters = self.update_param_list(source, original, importer, emit_context);
        let type_node = self.ensure_declaration_type(
            source,
            original,
            importer,
            emit_context,
            enclosing_declaration,
        );
        let postfix_token =
            importer.preserve_optional_node(emit_context, source.postfix_token(*original));
        importer
            .factory(emit_context)
            .update_method_declaration_from_store(
                source,
                *original,
                modifiers,
                None,
                name,
                postfix_token,
                type_parameters,
                parameters,
                type_node,
                None,
                None,
            )
    }

    fn transform_property_declaration(
        &mut self,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        enclosing_declaration: ast::Node,
    ) -> ast::Node {
        let modifiers = self.ensure_modifiers(source, original, importer, emit_context);
        let name = importer.preserve_optional_node(emit_context, source.name(*original));
        let postfix_token = source.postfix_token(*original).and_then(|token| {
            (source.kind(token) != ast::Kind::ExclamationToken)
                .then(|| importer.preserve_node(emit_context, token))
        });
        let type_node = self.ensure_declaration_type(
            source,
            original,
            importer,
            emit_context,
            enclosing_declaration,
        );
        let initializer = self.ensure_no_initializer(source, original, emit_context);
        importer
            .factory(emit_context)
            .update_property_declaration_from_store(
                source,
                *original,
                modifiers,
                name,
                postfix_token,
                type_node,
                initializer,
            )
    }

    fn transform_constructor_declaration(
        &mut self,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> ast::Node {
        let modifiers = self.ensure_modifiers(source, original, importer, emit_context);
        let parameters = self.update_param_list(source, original, importer, emit_context);
        importer
            .factory(emit_context)
            .update_constructor_declaration_from_store(
                source, *original, modifiers, None, parameters, None, None, None,
            )
    }

    fn transform_construct_signature_declaration(
        &mut self,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> ast::Node {
        let type_parameters = self.preserve_type_parameters(
            source,
            source.type_parameters(*original),
            *original,
            importer,
            emit_context,
        );
        let parameters = self.update_param_list(source, original, importer, emit_context);
        let type_node =
            self.ensure_type(source, original, importer, emit_context, *original, false);
        importer
            .factory(emit_context)
            .update_construct_signature_declaration_from_store(
                source,
                *original,
                type_parameters,
                parameters,
                type_node,
            )
    }

    fn preserve_optional_node_list(
        &mut self,
        list: Option<ast::SourceNodeList<'_>>,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> Option<ast::NodeList> {
        list.map(|list| {
            let nodes = list
                .iter()
                .map(|node| importer.preserve_node(emit_context, node))
                .collect::<Vec<_>>();
            importer
                .factory(emit_context)
                .new_node_list(list.loc(), list.range(), nodes)
        })
    }

    fn preserve_optional_type_node_list(
        &mut self,
        source: &ast::AstStore,
        list: Option<ast::SourceNodeList<'_>>,
        enclosing_declaration: ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> Option<ast::NodeList> {
        list.map(|list| {
            let nodes = list
                .iter()
                .map(|node| {
                    self.check_type_node_visibility(
                        source,
                        node,
                        enclosing_declaration,
                        enclosing_declaration,
                    );
                    importer.preserve_node(emit_context, node)
                })
                .collect::<Vec<_>>();
            importer
                .factory(emit_context)
                .new_node_list(list.loc(), list.range(), nodes)
        })
    }

    fn transform_heritage_clauses(
        &mut self,
        source: &ast::AstStore,
        nodes: Option<ast::SourceNodeList<'_>>,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        enclosing_declaration: ast::Node,
    ) -> Option<ast::NodeList> {
        nodes.map(|nodes| {
            let clauses = nodes
                .iter()
                .filter_map(|clause| {
                    let types = source.types(clause)?;
                    let token = source.token(clause)?;
                    let type_nodes = types
                        .iter()
                        .filter_map(|t| {
                            let expression = source.expression(t)?;
                            if ast::is_entity_name_expression(source, expression)
                                || (token == ast::Kind::ExtendsKeyword
                                    && source.kind(expression) == ast::Kind::NullKeyword)
                            {
                                self.check_type_node_visibility(
                                    source,
                                    t,
                                    enclosing_declaration,
                                    enclosing_declaration,
                                );
                                Some(importer.preserve_node(emit_context, t))
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();
                    if type_nodes.is_empty() {
                        return None;
                    }
                    let type_nodes = importer.factory(emit_context).new_node_list(
                        types.loc(),
                        types.range(),
                        type_nodes,
                    );
                    Some(
                        importer
                            .factory(emit_context)
                            .update_heritage_clause_from_store(source, clause, token, type_nodes),
                    )
                })
                .collect::<Vec<_>>();
            importer
                .factory(emit_context)
                .new_node_list(nodes.loc(), nodes.range(), clauses)
        })
    }

    fn preserve_type_parameters(
        &mut self,
        source: &ast::AstStore,
        list: Option<ast::SourceNodeList<'_>>,
        enclosing_declaration: ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> Option<ast::NodeList> {
        list.map(|list| {
            let nodes = list
                .iter()
                .map(|node| {
                    if let Some(constraint) = source.constraint(node) {
                        self.check_type_node_visibility(
                            source,
                            constraint,
                            enclosing_declaration,
                            enclosing_declaration,
                        );
                    }
                    if let Some(default_type) = source.default_type(node) {
                        self.check_type_node_visibility(
                            source,
                            default_type,
                            enclosing_declaration,
                            enclosing_declaration,
                        );
                    }
                    self.transform_type_parameter_declaration(
                        source,
                        &node,
                        importer,
                        emit_context,
                        enclosing_declaration,
                    )
                })
                .collect::<Vec<_>>();
            importer
                .factory(emit_context)
                .new_node_list(list.loc(), list.range(), nodes)
        })
    }

    fn transform_type_parameter_declaration(
        &mut self,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        enclosing_declaration: ast::Node,
    ) -> ast::Node {
        let modifiers = importer.preserve_optional_source_modifier_list(
            emit_context,
            source.source_modifiers(*original),
        );
        let name = importer.preserve_optional_node(emit_context, source.name(*original));
        let expression =
            importer.preserve_optional_node(emit_context, source.expression(*original));
        if is_private_method_type_parameter(source, self.host, *original)
            && (source.default_type(*original).is_some() || source.constraint(*original).is_some())
        {
            return importer
                .factory(emit_context)
                .update_type_parameter_declaration_from_store(
                    source, *original, modifiers, name, None, expression, None,
                );
        }

        let constraint = source.constraint(*original).map(|constraint| {
            self.preserve_declaration_type_node(
                source,
                constraint,
                importer,
                emit_context,
                enclosing_declaration,
            )
        });
        let default_type = source.default_type(*original).map(|default_type| {
            self.preserve_declaration_type_node(
                source,
                default_type,
                importer,
                emit_context,
                enclosing_declaration,
            )
        });
        importer
            .factory(emit_context)
            .update_type_parameter_declaration_from_store(
                source,
                *original,
                modifiers,
                name,
                constraint,
                expression,
                default_type,
            )
    }

    fn update_param_list(
        &mut self,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> ast::NodeList {
        let Some(parameters) = source.source_parameters(*original) else {
            return importer.factory(emit_context).new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                Vec::<ast::Node>::new(),
            );
        };
        let is_private = self
            .host
            .get_effective_declaration_flags(*original, ast::ModifierFlags::Private)
            != ast::ModifierFlags::NONE;
        if is_private || parameters.is_empty() {
            return importer.factory(emit_context).new_node_list(
                parameters.loc(),
                parameters.range(),
                Vec::<ast::Node>::new(),
            );
        }
        let nodes = parameters
            .iter()
            .map(|parameter| {
                self.ensure_parameter(source, &parameter, importer, emit_context, *original)
            })
            .collect::<Vec<_>>();
        importer
            .factory(emit_context)
            .new_node_list(parameters.loc(), parameters.range(), nodes)
    }

    fn update_accessor_param_list(
        &mut self,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> ast::NodeList {
        let is_private = self
            .host
            .get_effective_declaration_flags(*original, ast::ModifierFlags::Private)
            != ast::ModifierFlags::NONE;
        let parameters = source.source_parameters(*original);
        let mut nodes = Vec::new();
        if !is_private {
            if let Some(parameters) = parameters {
                nodes.extend(
                    parameters
                        .iter()
                        .take(if source.kind(*original) == ast::Kind::SetAccessor {
                            1
                        } else {
                            usize::MAX
                        })
                        .map(|parameter| {
                            self.ensure_parameter(
                                source,
                                &parameter,
                                importer,
                                emit_context,
                                *original,
                            )
                        }),
                );
            }
        }
        if source.kind(*original) == ast::Kind::SetAccessor && nodes.is_empty() {
            let value_name = importer.factory(emit_context).new_identifier("value");
            let type_node = (!is_private).then(|| {
                importer
                    .factory(emit_context)
                    .new_keyword_type_node(ast::Kind::AnyKeyword)
            });
            nodes.push(
                importer
                    .factory(emit_context)
                    .new_parameter_declaration(None, None, value_name, None, type_node, None),
            );
        }
        let loc = parameters
            .map(|parameters| parameters.loc())
            .unwrap_or_else(core::undefined_text_range);
        let range = parameters
            .map(|parameters| parameters.range())
            .unwrap_or_else(core::undefined_text_range);
        importer
            .factory(emit_context)
            .new_node_list(loc, range, nodes)
    }

    fn ensure_parameter(
        &mut self,
        source: &ast::AstStore,
        parameter: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        enclosing_declaration: ast::Node,
    ) -> ast::Node {
        let dot_dot_dot_token =
            importer.preserve_optional_node(emit_context, source.dot_dot_dot_token(*parameter));
        let name = source.name(*parameter).map(|name| {
            self.filter_binding_pattern_initializers(
                source,
                name,
                *parameter,
                importer,
                emit_context,
            )
        });
        let question_token = if self.host.is_optional_parameter(*parameter) {
            importer
                .preserve_optional_node(emit_context, source.question_token(*parameter))
                .or_else(|| {
                    Some(
                        importer
                            .factory(emit_context)
                            .new_token(ast::Kind::QuestionToken),
                    )
                })
        } else {
            None
        };
        let type_node = self.ensure_type(
            source,
            parameter,
            importer,
            emit_context,
            enclosing_declaration,
            true,
        );
        let initializer = self.ensure_no_initializer(source, parameter, emit_context);
        importer
            .factory(emit_context)
            .update_parameter_declaration_from_store(
                source,
                *parameter,
                None,
                dot_dot_dot_token,
                name,
                question_token,
                type_node,
                initializer,
            )
    }

    fn filter_binding_pattern_initializers(
        &mut self,
        source: &ast::AstStore,
        name: ast::Node,
        enclosing_declaration: ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> ast::Node {
        if ast::is_identifier(source, name) || ast::is_omitted_expression(source, name) {
            return importer.preserve_node(emit_context, name);
        }
        let source_elements = source
            .source_elements(name)
            .expect("binding pattern should have elements");
        let elements = source_elements
            .iter()
            .map(|element| {
                self.visit_binding_element(
                    source,
                    element,
                    enclosing_declaration,
                    importer,
                    emit_context,
                )
            })
            .collect::<Vec<_>>();
        let elements = importer
            .factory(emit_context)
            .new_node_list_with_trailing_comma(
                source_elements.loc(),
                source_elements.range(),
                elements,
                source_elements.has_trailing_comma(),
            );
        importer
            .factory(emit_context)
            .update_binding_pattern_from_store(source, name, elements)
    }

    fn visit_binding_element(
        &mut self,
        source: &ast::AstStore,
        element: ast::Node,
        enclosing_declaration: ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> ast::Node {
        if ast::is_omitted_expression(source, element) {
            return importer.preserve_node(emit_context, element);
        }
        if let Some(property_name) = source.property_name(element)
            && ast::is_computed_property_name(source, property_name)
            && let Some(expression) = source.expression(property_name)
            && ast::is_entity_name_expression(source, expression)
        {
            self.check_entity_name_visibility(
                source,
                expression,
                enclosing_declaration,
                property_name,
            );
        }
        let dot_dot_dot_token =
            importer.preserve_optional_node(emit_context, source.dot_dot_dot_token(element));
        let property_name =
            importer.preserve_optional_node(emit_context, source.property_name(element));
        let name = source.name(element).map(|name| {
            self.filter_binding_pattern_initializers(
                source,
                name,
                enclosing_declaration,
                importer,
                emit_context,
            )
        });
        importer
            .factory(emit_context)
            .update_binding_element_from_store(
                source,
                element,
                dot_dot_dot_token,
                property_name,
                name,
                None,
            )
    }

    fn ensure_type(
        &mut self,
        source: &ast::AstStore,
        node: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        enclosing_declaration: ast::Node,
        ignore_private: bool,
    ) -> Option<ast::Node> {
        self.ensure_type_with_error_fallback_node(
            source,
            node,
            importer,
            emit_context,
            enclosing_declaration,
            ignore_private,
            None,
        )
    }

    fn collect_parameters_requiring_implicit_undefined(
        &mut self,
        source: &ast::AstStore,
        node: ast::Node,
        enclosing_declaration: ast::Node,
    ) -> Vec<ast::Node> {
        let mut result = Vec::new();
        let mut stack = vec![node];
        while let Some(current) = stack.pop() {
            if ast::is_parameter_declaration(source, current)
                && self
                    .host
                    .requires_adding_implicit_undefined(current, enclosing_declaration)
            {
                result.push(current);
            }
            let _ = source.for_each_present_child(current, |child| {
                stack.push(child);
                std::ops::ControlFlow::Continue(())
            });
        }
        result
    }

    fn ensure_type_with_error_fallback_node(
        &mut self,
        source: &ast::AstStore,
        node: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        enclosing_declaration: ast::Node,
        ignore_private: bool,
        error_fallback_node: Option<ast::Node>,
    ) -> Option<ast::Node> {
        if !ignore_private
            && self
                .host
                .get_effective_declaration_flags(*node, ast::ModifierFlags::PRIVATE)
                != ast::ModifierFlags::NONE
        {
            return None;
        }
        if self.should_print_with_initializer(source, node, emit_context) {
            return None;
        }
        if !ast::is_export_assignment(source, *node)
            && !ast::is_binding_element(source, *node)
            && source.r#type(*node).is_some()
            && (!ast::is_parameter_declaration(source, *node)
                || !self
                    .host
                    .requires_adding_implicit_undefined(*node, enclosing_declaration))
        {
            let type_node = source.r#type(*node).unwrap();
            self.check_type_node_visibility(source, type_node, *node, *node);
            return Some(self.preserve_declaration_type_node(
                source,
                type_node,
                importer,
                emit_context,
                *node,
            ));
        }
        if ast::has_inferred_type(source, *node) {
            let diagnostics = Rc::new(RefCell::new(Vec::new()));
            let requires_implicit_undefined = ast::is_parameter_declaration(source, *node)
                && self
                    .host
                    .requires_adding_implicit_undefined(*node, enclosing_declaration);
            let parameters_requiring_implicit_undefined = self
                .collect_parameters_requiring_implicit_undefined(
                    source,
                    *node,
                    enclosing_declaration,
                );
            let tracker = DeclarationDiagnosticTracker::new_with_error_fallback_node(
                diagnostics.clone(),
                self.late_marked_statements.clone(),
                self.compiler_options.isolated_declarations.is_true(),
                source,
                *node,
                error_fallback_node,
            )
            .with_declaration_requires_implicit_undefined(requires_implicit_undefined)
            .with_parameters_requiring_implicit_undefined(parameters_requiring_implicit_undefined);
            let result = self.host.create_type_of_declaration(
                emit_context,
                *node,
                enclosing_declaration,
                declaration_emit_internal_node_builder_flags(),
                Box::new(tracker),
            );
            self.diagnostics.extend(diagnostics.borrow_mut().drain(..));
            return result.or_else(|| {
                Some(
                    importer
                        .factory(emit_context)
                        .new_keyword_type_node(ast::Kind::AnyKeyword),
                )
            });
        }
        if ast::is_function_like(source, Some(*node)) {
            let diagnostics = Rc::new(RefCell::new(Vec::new()));
            let result = self.host.create_return_type_of_signature_declaration(
                emit_context,
                *node,
                enclosing_declaration,
                Box::new(DeclarationDiagnosticTracker::new(
                    diagnostics.clone(),
                    self.late_marked_statements.clone(),
                    self.compiler_options.isolated_declarations.is_true(),
                    source,
                    *node,
                )),
            );
            self.diagnostics.extend(diagnostics.borrow_mut().drain(..));
            return result;
        }
        Some(
            importer
                .factory(emit_context)
                .new_keyword_type_node(ast::Kind::AnyKeyword),
        )
    }

    fn ensure_declaration_type(
        &mut self,
        source: &ast::AstStore,
        node: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        enclosing_declaration: ast::Node,
    ) -> Option<ast::Node> {
        if self
            .host
            .get_effective_declaration_flags(*node, ast::ModifierFlags::PRIVATE)
            != ast::ModifierFlags::NONE
        {
            // Private nodes emit no types (except private parameter properties, whose parameter types are actually visible)
            return None;
        }
        if self.should_print_with_initializer(source, node, emit_context) {
            return None;
        }
        if !ast::is_export_assignment(source, *node)
            && !ast::is_binding_element(source, *node)
            && source.r#type(*node).is_some()
            && (!ast::is_parameter_declaration(source, *node)
                || !self
                    .host
                    .requires_adding_implicit_undefined(*node, enclosing_declaration))
        {
            let type_node = source.r#type(*node).unwrap();
            self.check_type_node_visibility(source, type_node, enclosing_declaration, *node);
            return Some(self.preserve_declaration_type_node(
                source,
                type_node,
                importer,
                emit_context,
                enclosing_declaration,
            ));
        }
        if ast::has_inferred_type(source, *node) {
            let diagnostics = Rc::new(RefCell::new(Vec::new()));
            let requires_implicit_undefined = ast::is_parameter_declaration(source, *node)
                && self
                    .host
                    .requires_adding_implicit_undefined(*node, enclosing_declaration);
            let parameters_requiring_implicit_undefined = self
                .collect_parameters_requiring_implicit_undefined(
                    source,
                    *node,
                    enclosing_declaration,
                );
            let mut tracker = DeclarationDiagnosticTracker::new(
                diagnostics.clone(),
                self.late_marked_statements.clone(),
                self.compiler_options.isolated_declarations.is_true(),
                source,
                *node,
            )
            .with_declaration_requires_implicit_undefined(requires_implicit_undefined)
            .with_parameters_requiring_implicit_undefined(parameters_requiring_implicit_undefined);
            if self.compiler_options.isolated_declarations.is_true()
                && ast::is_variable_declaration(source, *node)
                && self.host.is_expando_function_declaration(*node)
                && let Some(source_file) = ast::get_source_file_of_node(source, Some(*node))
                    .map(|source_file| source.source_file_view(source_file))
            {
                let expando_diagnostics = self.create_expando_function_error_diagnostics(
                    &source_file,
                    ast::DiagnosticFile::from_source_file_view(&source_file),
                    source,
                    *node,
                );
                tracker = tracker.with_inference_fallback_expando_diagnostics(expando_diagnostics);
            }
            let result = self.host.create_type_of_declaration(
                emit_context,
                *node,
                enclosing_declaration,
                declaration_emit_internal_node_builder_flags(),
                Box::new(tracker),
            );
            self.diagnostics.extend(diagnostics.borrow_mut().drain(..));
            return result;
        }
        if ast::is_function_like(source, Some(*node)) {
            let diagnostics = Rc::new(RefCell::new(Vec::new()));
            let result = self.host.create_return_type_of_signature_declaration(
                emit_context,
                *node,
                enclosing_declaration,
                Box::new(DeclarationDiagnosticTracker::new(
                    diagnostics.clone(),
                    self.late_marked_statements.clone(),
                    self.compiler_options.isolated_declarations.is_true(),
                    source,
                    *node,
                )),
            );
            self.diagnostics.extend(diagnostics.borrow_mut().drain(..));
            return result;
        }
        Some(
            importer
                .factory(emit_context)
                .new_keyword_type_node(ast::Kind::AnyKeyword),
        )
    }

    fn ensure_no_initializer(
        &mut self,
        source: &ast::AstStore,
        node: &ast::Node,
        emit_context: &mut printer::EmitContext,
    ) -> Option<ast::Node> {
        if self.should_print_with_initializer(source, node, emit_context) {
            let initializer = source
                .initializer(*node)
                .expect("literal const declaration should have an initializer");
            let unwrapped_initializer = ast::skip_parentheses(source, initializer);
            if !ast::is_primitive_literal_value(source, unwrapped_initializer, true) {
                let diagnostics = Rc::new(RefCell::new(Vec::new()));
                let mut tracker = DeclarationDiagnosticTracker::new(
                    diagnostics.clone(),
                    self.late_marked_statements.clone(),
                    self.compiler_options.isolated_declarations.is_true(),
                    source,
                    *node,
                );
                nodebuilder::SymbolTracker::report_inference_fallback(&mut tracker, *node);
                self.diagnostics.extend(diagnostics.borrow_mut().drain(..));
            }
            self.host.create_literal_const_value(emit_context, *node)
        } else {
            None
        }
    }

    fn should_print_with_initializer(
        &mut self,
        source: &ast::AstStore,
        node: &ast::Node,
        emit_context: &printer::EmitContext,
    ) -> bool {
        source.initializer(*node).is_some()
            && crate::declarations::util::can_have_literal_initializer(
                source.kind(*node),
                self.host
                    .get_effective_declaration_flags(*node, ast::ModifierFlags::Private)
                    != ast::ModifierFlags::NONE,
            )
            && self
                .host
                .is_literal_const_declaration(emit_context.most_original(node))
    }

    fn get_binding_name_visible(&mut self, source: &ast::AstStore, elem: &ast::Node) -> bool {
        if ast::is_omitted_expression(source, *elem) {
            return false;
        }
        let Some(name) = source.name(*elem) else {
            return false;
        };
        if ast::is_binding_pattern(source, name) {
            source.source_elements(name).is_some_and(|elements| {
                elements
                    .iter()
                    .any(|elem| self.get_binding_name_visible(source, &elem))
            })
        } else {
            self.host.is_declaration_visible(*elem)
        }
    }

    fn rewrite_module_specifier(
        &mut self,
        source: &ast::AstStore,
        parent: ast::Node,
        input: Option<ast::Node>,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> Option<ast::Node> {
        let input = input?;
        let is_indicator = source.kind(parent) != ast::Kind::ModuleDeclaration
            && source.kind(parent) != ast::Kind::ImportType;
        self.state.result_has_external_module_indicator |= is_indicator;
        Some(importer.preserve_node(emit_context, input))
    }

    fn try_get_resolution_mode_override(
        &mut self,
        input: Option<ast::Node>,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> Option<ast::Node> {
        let input = input?;
        if self.host.get_resolution_mode_override(input) != core::RESOLUTION_MODE_NONE {
            Some(importer.preserve_node(emit_context, input))
        } else {
            None
        }
    }

    fn transform_import_declaration(
        &mut self,
        file: &ast::SourceFile,
        source: &ast::AstStore,
        original: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> Option<ast::Node> {
        let Some(import_clause) = source.import_clause(*original) else {
            let modifiers = importer.preserve_optional_source_modifier_list(
                emit_context,
                source.source_modifiers(*original),
            );
            let module_specifier = self.rewrite_module_specifier(
                source,
                *original,
                source.module_specifier(*original),
                importer,
                emit_context,
            );
            let attributes = self.try_get_resolution_mode_override(
                source.attributes(*original),
                importer,
                emit_context,
            );
            return Some(
                importer
                    .factory(emit_context)
                    .update_import_declaration_from_store(
                        source,
                        *original,
                        modifiers,
                        source.import_clause(*original),
                        module_specifier,
                        attributes,
                    ),
            );
        };

        let phase_modifier = source
            .phase_modifier(import_clause)
            .filter(|kind| *kind != ast::Kind::DeferKeyword);
        let visible_default_binding = source
            .name(import_clause)
            .filter(|_| self.host.is_declaration_visible(import_clause))
            .map(|node| importer.preserve_node(emit_context, node));

        let named_bindings = match source.named_bindings(import_clause) {
            None => None,
            Some(bindings) if source.kind(bindings) == ast::Kind::NamespaceImport => self
                .host
                .is_declaration_visible(bindings)
                .then(|| importer.preserve_node(emit_context, bindings)),
            Some(bindings) => {
                let Some(source_bindings) = source.source_elements(bindings) else {
                    let imported_clause = importer.preserve_node(emit_context, import_clause);
                    let modifiers = importer.preserve_optional_source_modifier_list(
                        emit_context,
                        source.source_modifiers(*original),
                    );
                    let module_specifier = self.rewrite_module_specifier(
                        source,
                        *original,
                        source.module_specifier(*original),
                        importer,
                        emit_context,
                    );
                    let attributes = self.try_get_resolution_mode_override(
                        source.attributes(*original),
                        importer,
                        emit_context,
                    );
                    return Some(
                        importer
                            .factory(emit_context)
                            .update_import_declaration_from_store(
                                source,
                                *original,
                                modifiers,
                                Some(imported_clause),
                                module_specifier,
                                attributes,
                            ),
                    );
                };
                let binding_list: Vec<_> = source_bindings
                    .iter()
                    .filter(|binding| self.host.is_declaration_visible(*binding))
                    .map(|binding| importer.preserve_node(emit_context, binding))
                    .collect();
                if binding_list.is_empty() {
                    None
                } else {
                    let binding_list = importer.factory(emit_context).new_node_list(
                        source_bindings.loc(),
                        source_bindings.range(),
                        binding_list,
                    );
                    Some(
                        importer
                            .factory(emit_context)
                            .update_named_imports_from_store(source, bindings, binding_list),
                    )
                }
            }
        };

        if visible_default_binding.is_none() && named_bindings.is_none() {
            // Augmentation of export depends on import
            if self.host.is_import_required_by_augmentation(*original) {
                if self.compiler_options.isolated_declarations.is_true() {
                    self.add_declaration_diagnostic(
                        file,
                        *original,
                        &diagnostic_messages::Declaration_emit_for_this_file_requires_preserving_this_import_for_augmentations_This_is_not_supported_with_isolatedDeclarations,
                    );
                }
                let modifiers = importer.preserve_optional_source_modifier_list(
                    emit_context,
                    source.source_modifiers(*original),
                );
                let module_specifier = self.rewrite_module_specifier(
                    source,
                    *original,
                    source.module_specifier(*original),
                    importer,
                    emit_context,
                );
                let attributes = self.try_get_resolution_mode_override(
                    source.attributes(*original),
                    importer,
                    emit_context,
                );
                return Some(
                    importer
                        .factory(emit_context)
                        .update_import_declaration_from_store(
                            source,
                            *original,
                            modifiers,
                            None,
                            module_specifier,
                            attributes,
                        ),
                );
            }
            return None;
        }

        let import_clause = importer
            .factory(emit_context)
            .update_import_clause_from_store(
                source,
                import_clause,
                phase_modifier,
                visible_default_binding,
                named_bindings,
            );
        let modifiers = importer.preserve_optional_source_modifier_list(
            emit_context,
            source.source_modifiers(*original),
        );
        let module_specifier = self.rewrite_module_specifier(
            source,
            *original,
            source.module_specifier(*original),
            importer,
            emit_context,
        );
        let attributes = self.try_get_resolution_mode_override(
            source.attributes(*original),
            importer,
            emit_context,
        );
        Some(
            importer
                .factory(emit_context)
                .update_import_declaration_from_store(
                    source,
                    *original,
                    modifiers,
                    Some(import_clause),
                    module_specifier,
                    attributes,
                ),
        )
    }

    fn ensure_modifiers(
        &mut self,
        source: &ast::AstStore,
        node: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> Option<ast::ModifierList> {
        let parent_is_file = source
            .parent(*node)
            .is_some_and(|parent| source.kind(parent) == ast::Kind::SourceFile);
        self.ensure_modifiers_with_parent_is_file(
            source,
            node,
            importer,
            emit_context,
            parent_is_file,
        )
    }

    fn ensure_modifiers_with_parent_is_file(
        &mut self,
        source: &ast::AstStore,
        node: &ast::Node,
        importer: &mut DeclarationImporter<'_>,
        emit_context: &mut printer::EmitContext,
        parent_is_file: bool,
    ) -> Option<ast::ModifierList> {
        let current_flags = self
            .host
            .get_effective_declaration_flags(*node, ast::ModifierFlags::ALL);
        let new_flags = self.ensure_modifier_flags(source, node, parent_is_file);
        if current_flags == new_flags {
            if let Some(mods) = source.source_modifiers(*node) {
                let nodes = mods.nodes();
                let modifiers = nodes
                    .iter()
                    .filter(|modifier| ast::is_modifier(source, *modifier))
                    .map(|modifier| importer.preserve_node(emit_context, modifier))
                    .collect::<Vec<_>>();
                return Some(importer.factory(emit_context).new_modifier_list(
                    mods.loc(),
                    mods.range(),
                    modifiers,
                    mods.modifier_flags(),
                ));
            }
            if new_flags.is_empty() {
                return None;
            }
            let result = ast::create_modifiers_from_modifier_flags(new_flags, |kind| {
                importer.factory(emit_context).new_modifier(kind)
            });
            return Some(importer.factory(emit_context).new_modifier_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                result,
                new_flags,
            ));
        }
        let result = ast::create_modifiers_from_modifier_flags(new_flags, |kind| {
            importer.factory(emit_context).new_modifier(kind)
        });
        if result.is_empty() {
            None
        } else {
            Some(importer.factory(emit_context).new_modifier_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                result,
                new_flags,
            ))
        }
    }

    fn ensure_modifier_flags(
        &mut self,
        source: &ast::AstStore,
        node: &ast::Node,
        parent_is_file: bool,
    ) -> ast::ModifierFlags {
        let mut mask = ast::ModifierFlags::ALL
            ^ (ast::ModifierFlags::PUBLIC
                | ast::ModifierFlags::ASYNC
                | ast::ModifierFlags::OVERRIDE);
        let mut additions = ast::ModifierFlags::NONE;
        if self.state.needs_declare
            && !crate::declarations::util::is_always_type(source.kind(*node))
        {
            additions = ast::ModifierFlags::AMBIENT;
        }
        if !parent_is_file {
            mask = mask ^ ast::ModifierFlags::AMBIENT;
            additions = ast::ModifierFlags::NONE;
        }
        let mut flags = crate::declarations::util::mask_modifier_flags(
            self.host.get_effective_declaration_flags(*node, mask) | additions,
        );
        if self.state.strip_export_modifiers {
            flags &= !ast::ModifierFlags::EXPORT;
        }
        flags
    }

    pub fn get_diagnostics(self) -> Vec<ast::Diagnostic> {
        self.diagnostics
    }
}

fn create_empty_exports(factory: &mut ast::NodeFactory) -> ast::Node {
    let elements = factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        Vec::<ast::Node>::new(),
    );
    let named_exports = factory.new_named_exports(elements);
    factory.new_export_declaration(None, false, named_exports, None, None)
}

fn declaration_output_statement_needs_scope_marker(store: &ast::AstStore, node: ast::Node) -> bool {
    !ast::is_any_import_or_re_export(store, node)
        && !ast::is_export_assignment(store, node)
        && !ast::has_syntactic_modifier(store, node, ast::ModifierFlags::EXPORT)
        && !ast::is_ambient_module(store, node)
}

fn is_scope_marker(store: &ast::AstStore, node: ast::Node) -> bool {
    ast::is_export_assignment(store, node) || ast::is_export_declaration(store, node)
}

fn has_scope_marker(store: &ast::AstStore, statements: &[ast::Node]) -> bool {
    statements
        .iter()
        .any(|statement| is_scope_marker(store, *statement))
}

fn get_effective_base_type_node(source: &ast::AstStore, node: ast::Node) -> Option<ast::Node> {
    let heritage_clauses = source.heritage_clauses(node)?;
    for clause in heritage_clauses.iter() {
        if source.kind(clause) == ast::Kind::HeritageClause
            && source.token(clause) == Some(ast::Kind::ExtendsKeyword)
        {
            return source.types(clause).and_then(|types| types.first());
        }
    }
    None
}

fn is_private_method_type_parameter(
    source: &ast::AstStore,
    host: &mut dyn DeclarationEmitHost,
    node: ast::Node,
) -> bool {
    source
        .parent(node)
        .is_some_and(|parent| source.kind(parent) == ast::Kind::MethodDeclaration)
        && host.get_effective_declaration_flags(
            source
                .parent(node)
                .expect("private method type parameter should have parent"),
            ast::ModifierFlags::Private,
        ) != ast::ModifierFlags::NONE
}

fn is_external_module_indicator_statement(source: &ast::AstStore, node: ast::Node) -> bool {
    ast::has_syntactic_modifier(source, node, ast::ModifierFlags::EXPORT)
        || (ast::is_import_equals_declaration(source, node)
            && source
                .module_reference(node)
                .is_some_and(|module_reference| {
                    ast::is_external_module_reference(source, module_reference)
                }))
        || ast::is_import_declaration(source, node)
        || ast::is_export_assignment(source, node)
        || ast::is_export_declaration(source, node)
}

fn declaration_emit_node_builder_flags() -> nodebuilder::Flags {
    nodebuilder::FLAGS_MULTILINE_OBJECT_LITERALS
        | nodebuilder::FLAGS_WRITE_CLASS_EXPRESSION_AS_TYPE_LITERAL
        | nodebuilder::FLAGS_USE_TYPE_OF_FUNCTION
        | nodebuilder::FLAGS_USE_STRUCTURAL_FALLBACK
        | nodebuilder::FLAGS_ALLOW_EMPTY_TUPLE
        | nodebuilder::FLAGS_GENERATE_NAMES_FOR_SHADOWED_TYPE_PARAMS
        | nodebuilder::FLAGS_NO_TRUNCATION
}

fn declaration_emit_internal_node_builder_flags() -> nodebuilder::InternalFlags {
    nodebuilder::INTERNAL_FLAGS_ALLOW_UNRESOLVED_NAMES
}

fn has_internal_annotation(file: &ast::SourceFile, comment: ast::CommentRange) -> bool {
    let start = comment.pos().max(0) as usize;
    let end = comment.end().max(comment.pos()).max(0) as usize;
    file.text()
        .get(start..end)
        .is_some_and(|text| text.contains("@internal"))
}

pub fn new_declaration_transformer<'a>(
    host: &'a mut dyn DeclarationEmitHost,
    _emit_context: Option<&mut printer::EmitContext>,
    options: &core::CompilerOptions,
    declaration_file_path: &str,
    declaration_map_path: &str,
) -> DeclarationTransformer<'a> {
    DeclarationTransformer {
        host,
        compiler_options: options.clone(),
        declaration_file_path: declaration_file_path.to_owned(),
        declaration_map_path: declaration_map_path.to_owned(),
        state: DeclarationSourceFileState::default(),
        diagnostics: Vec::new(),
        late_marked_statements: Rc::new(RefCell::new(Vec::new())),
        late_statement_replacement_map: HashMap::new(),
        expando_hosts: HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDeclarationEmitHost;

    impl DeclarationEmitHost for TestDeclarationEmitHost {
        fn get_effective_declaration_flags(
            &mut self,
            _node: ast::Node,
            _flags: ast::ModifierFlags,
        ) -> ast::ModifierFlags {
            ast::ModifierFlags::NONE
        }

        fn is_declaration_visible(&mut self, _node: ast::Node) -> bool {
            true
        }

        fn is_literal_const_declaration(&mut self, _node: ast::Node) -> bool {
            false
        }

        fn is_import_required_by_augmentation(&mut self, _decl: ast::Node) -> bool {
            false
        }

        fn create_literal_const_value(
            &mut self,
            _emit_context: &mut printer::EmitContext,
            _node: ast::Node,
        ) -> Option<ast::Node> {
            None
        }

        fn get_enum_member_value(&mut self, _node: ast::Node) -> evaluator::Result {
            evaluator::Result::default()
        }

        fn create_type_of_expression(
            &mut self,
            emit_context: &mut printer::EmitContext,
            _expression: ast::Node,
            _enclosing_declaration: ast::Node,
            _tracker: Box<dyn nodebuilder::SymbolTracker>,
        ) -> Option<ast::Node> {
            Some(
                emit_context
                    .factory
                    .new_keyword_type_node(ast::Kind::AnyKeyword),
            )
        }

        fn create_type_of_declaration(
            &mut self,
            emit_context: &mut printer::EmitContext,
            _declaration: ast::Node,
            _enclosing_declaration: ast::Node,
            _internal_flags: nodebuilder::InternalFlags,
            _tracker: Box<dyn nodebuilder::SymbolTracker>,
        ) -> Option<ast::Node> {
            Some(
                emit_context
                    .factory
                    .new_keyword_type_node(ast::Kind::AnyKeyword),
            )
        }

        fn get_declaration_statements_for_source_file(
            &mut self,
            _emit_context: &mut printer::EmitContext,
            _source_file: ast::Node,
            _tracker: Box<dyn nodebuilder::SymbolTracker>,
        ) -> Vec<ast::Node> {
            Vec::new()
        }

        fn create_return_type_of_signature_declaration(
            &mut self,
            emit_context: &mut printer::EmitContext,
            _declaration: ast::Node,
            _enclosing_declaration: ast::Node,
            _tracker: Box<dyn nodebuilder::SymbolTracker>,
        ) -> Option<ast::Node> {
            Some(
                emit_context
                    .factory
                    .new_keyword_type_node(ast::Kind::AnyKeyword),
            )
        }

        fn create_late_bound_index_signatures(
            &mut self,
            _emit_context: &mut printer::EmitContext,
            _container: ast::Node,
            _enclosing_declaration: ast::Node,
            _tracker: Box<dyn nodebuilder::SymbolTracker>,
        ) -> Vec<ast::Node> {
            Vec::new()
        }

        fn create_signature_declaration_with_synthetic_rest_parameter(
            &mut self,
            _emit_context: &mut printer::EmitContext,
            _declaration: ast::Node,
            _kind: ast::Kind,
            _modifiers: Vec<ast::Node>,
            _name: Option<ast::Node>,
            _enclosing_declaration: ast::Node,
            _tracker: Box<dyn nodebuilder::SymbolTracker>,
        ) -> Option<ast::Node> {
            None
        }

        fn is_entity_name_visible(
            &mut self,
            _entity_name: ast::Node,
            _enclosing_declaration: ast::Node,
        ) -> printer::SymbolAccessibilityResult {
            printer::SymbolAccessibilityResult {
                accessibility: printer::SYMBOL_ACCESSIBILITY_ACCESSIBLE,
                aliases_to_make_visible: Vec::new(),
                error_symbol_name: String::new(),
                error_node: None,
                error_module_name: String::new(),
            }
        }

        fn is_definitely_reference_to_global_symbol_object(&mut self, _node: ast::Node) -> bool {
            false
        }

        fn is_late_bound(&mut self, _node: ast::Node) -> bool {
            false
        }

        fn is_optional_parameter(&mut self, _node: ast::Node) -> bool {
            false
        }

        fn precalculate_declaration_emit_visibility(&mut self, _file: &ast::SourceFile) {}

        fn is_common_js_alias_export(&mut self, _node: ast::Node) -> bool {
            false
        }

        fn source_file_is_external_or_common_js_module(&self, file: &ast::SourceFile) -> bool {
            ast::is_external_or_common_js_module(file)
        }

        fn source_file_common_js_module_indicator(
            &self,
            file: &ast::SourceFile,
        ) -> Option<ast::Node> {
            file.common_js_module_indicator()
        }

        fn source_file_export_equals_declarations(
            &self,
            _file: &ast::SourceFile,
        ) -> Vec<ast::Node> {
            Vec::new()
        }

        fn source_file_nested_cjs_exports(&self, _file: &ast::SourceFile) -> Vec<ast::Node> {
            Vec::new()
        }

        fn get_current_directory(&self) -> String {
            "/".to_owned()
        }

        fn use_case_sensitive_file_names(&self) -> bool {
            true
        }

        fn get_resolution_mode_override(&mut self, _node: ast::Node) -> core::ResolutionMode {
            core::RESOLUTION_MODE_NONE
        }

        fn is_implementation_of_overload(&mut self, _node: ast::Node) -> bool {
            false
        }

        fn is_expando_function_declaration(&mut self, _node: ast::Node) -> bool {
            false
        }

        fn should_emit_function_properties(&mut self, _node: ast::Node) -> bool {
            true
        }

        fn get_properties_of_container_function(
            &mut self,
            _node: ast::Node,
        ) -> Vec<ast::SymbolIdentity> {
            Vec::new()
        }

        fn get_symbol_name(&mut self, _symbol: ast::SymbolIdentity) -> String {
            String::new()
        }

        fn get_symbol_value_declaration(
            &mut self,
            _symbol: ast::SymbolIdentity,
        ) -> Option<ast::Node> {
            None
        }

        fn get_referenced_value_declaration(&mut self, _node: ast::Node) -> Option<ast::Node> {
            None
        }

        fn get_referenced_member_value_declaration(
            &mut self,
            _node: ast::Node,
        ) -> Option<ast::Node> {
            None
        }

        fn get_element_access_expression_name(&mut self, _expression: ast::Node) -> String {
            String::new()
        }

        fn set_expando_namespace_metadata(
            &mut self,
            _synthesized_namespace: ast::Node,
            _declaration: ast::Node,
            _properties: &[ast::SymbolIdentity],
        ) {
        }

        fn requires_adding_implicit_undefined(
            &mut self,
            _node: ast::Node,
            _enclosing_declaration: ast::Node,
        ) -> bool {
            false
        }

        fn is_first_declaration_of_symbol(&mut self, _node: ast::Node) -> bool {
            true
        }

        fn is_assignment_declaration(&mut self, _node: ast::Node) -> bool {
            false
        }

        fn get_source_file_from_reference(
            &self,
            _origin: &ast::SourceFile,
            _ref: &ast::FileReference,
        ) -> Option<ast::SourceFile> {
            None
        }

        fn get_output_paths_for(
            &self,
            _file: &ast::SourceFile,
            _force_dts_paths: bool,
        ) -> outputpaths::OutputPaths {
            outputpaths::OutputPaths::default()
        }
    }

    fn source_file(is_declaration_file: bool) -> ast::SourceFile {
        let mut factory = ast::NodeFactory::default();
        let statements = factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            Vec::new(),
        );
        let file = factory.new_source_file(
            ast::SourceFileParseOptions {
                file_name: "/decl.ts".to_string(),
                path: "/decl.ts".to_string(),
                ..Default::default()
            },
            String::new(),
            statements,
            None,
        );
        factory.finish_parsed_source_file(
            file,
            ast::ParsedSourceFileMetadata {
                is_declaration_file,
                ..Default::default()
            },
        )
    }

    #[test]
    fn declaration_transformer_keeps_constructor_inputs() {
        let mut host = TestDeclarationEmitHost;
        let options = core::CompilerOptions::default();
        let transformer = new_declaration_transformer(
            &mut host,
            None,
            &options,
            "/out/a.d.ts",
            "/out/a.d.ts.map",
        );

        assert_eq!(transformer.declaration_file_path, "/out/a.d.ts");
        assert_eq!(transformer.declaration_file_path(), "/out/a.d.ts");
        assert_eq!(transformer.declaration_map_path(), "/out/a.d.ts.map");
        assert_eq!(transformer.state, DeclarationSourceFileState::default());
    }

    #[test]
    fn declaration_transformer_skips_declaration_files_like_go() {
        let mut host = TestDeclarationEmitHost;
        let options = core::CompilerOptions::default();
        let mut transformer = new_declaration_transformer(&mut host, None, &options, "", "");

        transformer.transform_source_file(&source_file(true));
        assert_eq!(transformer.state, DeclarationSourceFileState::default());

        transformer.transform_source_file(&source_file(false));
        assert!(transformer.state.needs_declare);
    }

    #[test]
    fn declaration_transformer_preserves_single_quote_literal_type_nodes() {
        let file = ts_parser::parse_source_file(
            ast::SourceFileParseOptions {
                file_name: "/decl.ts".to_string(),
                path: "/decl.ts".to_string(),
                ..Default::default()
            },
            "type A = { kind: 'a' };".to_string(),
            core::ScriptKind::TS,
        );
        let mut host = TestDeclarationEmitHost;
        let options = core::CompilerOptions {
            declaration: core::Tristate::True,
            ..Default::default()
        };
        let mut transformer = new_declaration_transformer(&mut host, None, &options, "", "");
        let transformed = transformer.transform_source_file(&file);
        let mut printer = printer::new_printer(
            printer::PrinterOptions::default(),
            printer::PrintHandlers::default(),
            None,
        );
        let output = printer.emit(&transformed.as_node(), Some(&transformed));

        assert!(output.contains("kind: 'a';"), "{output}");
    }
}
