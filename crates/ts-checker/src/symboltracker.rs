use ts_ast as ast;
use ts_nodebuilder as nodebuilder;

use crate::checker::*;
use crate::nodebuilderimpl::{NodeBuilderContext, TrackedSymbolArgs};

pub struct SymbolTrackerImpl<'a> {
    context: &'a mut NodeBuilderContext<'a>,
    inner: Option<Box<dyn nodebuilder::SymbolTracker + 'a>>,
    pub disable_track_symbol: bool,
}

pub fn new_symbol_tracker_impl<'a>(
    context: &'a mut NodeBuilderContext<'a>,
    tracker: Option<Box<dyn nodebuilder::SymbolTracker + 'a>>,
) -> SymbolTrackerImpl<'a> {
    SymbolTrackerImpl {
        context,
        inner: tracker,
        disable_track_symbol: false,
    }
}

impl<'a> nodebuilder::SymbolTracker for SymbolTrackerImpl<'a> {
    fn track_symbol(
        &mut self,
        symbol: ast::SymbolIdentity,
        symbol_flags: ast::SymbolFlags,
        enclosing_declaration: Option<ast::Node>,
        meaning: ast::SymbolFlags,
    ) -> bool {
        let symbol = SymbolIdentity::from_symbol_handle(symbol.symbol_handle());
        if !self.disable_track_symbol {
            if self.inner.is_some()
                && self.inner.as_mut().unwrap().track_symbol(
                    symbol.ast_identity(),
                    symbol_flags,
                    enclosing_declaration,
                    meaning,
                )
            {
                self.on_diagnostic_reported();
                return true;
            }
            // Skip recording type parameters as they dont contribute to late painted statements
            if !symbol_flags.intersects(ast::SYMBOL_FLAGS_TYPE_PARAMETER) {
                self.context.tracked_symbols.push(TrackedSymbolArgs {
                    symbol,
                    symbol_flags,
                    enclosing_declaration,
                    meaning,
                });
            }
        }
        false
    }

    fn report_inaccessible_this_error(&mut self) {
        self.on_diagnostic_reported();
        if self.inner.is_none() {
            return;
        }
        self.inner
            .as_mut()
            .unwrap()
            .report_inaccessible_this_error();
    }

    fn report_private_in_base_of_class_expression(&mut self, property_name: &str) {
        self.on_diagnostic_reported();
        if self.inner.is_none() {
            return;
        }
        self.inner
            .as_mut()
            .unwrap()
            .report_private_in_base_of_class_expression(property_name);
    }

    fn report_inaccessible_unique_symbol_error(&mut self) {
        self.on_diagnostic_reported();
        if self.inner.is_none() {
            return;
        }
        self.inner
            .as_mut()
            .unwrap()
            .report_inaccessible_unique_symbol_error();
    }

    fn report_cyclic_structure_error(&mut self) {
        self.on_diagnostic_reported();
        if self.inner.is_none() {
            return;
        }
        self.inner.as_mut().unwrap().report_cyclic_structure_error();
    }

    fn report_likely_unsafe_import_required_error(&mut self, specifier: &str, symbol_name: &str) {
        self.on_diagnostic_reported();
        if self.inner.is_none() {
            return;
        }
        self.inner
            .as_mut()
            .unwrap()
            .report_likely_unsafe_import_required_error(specifier, symbol_name);
    }

    fn report_truncation_error(&mut self) {
        self.on_diagnostic_reported();
        if self.inner.is_none() {
            return;
        }
        self.inner.as_mut().unwrap().report_truncation_error();
    }

    fn report_nonlocal_augmentation(
        &mut self,
        containing_file: &ast::SourceFile,
        parent_symbol: ast::SymbolIdentity,
        augmenting_symbol: ast::SymbolIdentity,
    ) {
        self.on_diagnostic_reported();
        if self.inner.is_none() {
            return;
        }
        self.inner.as_mut().unwrap().report_nonlocal_augmentation(
            containing_file,
            parent_symbol,
            augmenting_symbol,
        );
    }

    fn report_non_serializable_property(&mut self, property_name: &str) {
        self.on_diagnostic_reported();
        if self.inner.is_none() {
            return;
        }
        self.inner
            .as_mut()
            .unwrap()
            .report_non_serializable_property(property_name);
    }

    fn report_inference_fallback(&mut self, node: ast::Node) {
        if self.inner.is_none() {
            return;
        }
        self.inner.as_mut().unwrap().report_inference_fallback(node);
    }

    fn push_error_fallback_node(&mut self, node: ast::Node) {
        if self.inner.is_none() {
            return;
        }
        self.inner.as_mut().unwrap().push_error_fallback_node(node);
    }

    fn pop_error_fallback_node(&mut self) {
        if self.inner.is_none() {
            return;
        }
        self.inner.as_mut().unwrap().pop_error_fallback_node();
    }
}

impl<'a> SymbolTrackerImpl<'a> {
    fn on_diagnostic_reported(&mut self) {
        self.context.reported_diagnostic = true;
    }
}
