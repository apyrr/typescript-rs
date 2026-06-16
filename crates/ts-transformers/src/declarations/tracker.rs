#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeclarationTrackerDiagnostic {
    CyclicStructure {
        declaration_name: String,
    },
    InaccessibleThis {
        declaration_name: String,
    },
    InaccessibleUniqueSymbol {
        declaration_name: String,
    },
    InferenceFallback {
        node: String,
    },
    LikelyUnsafeImportRequired {
        declaration_name: String,
        specifier: String,
        symbol_name: Option<String>,
    },
    NonSerializableProperty {
        property_name: String,
    },
    NonlocalAugmentation {
        augmentation: String,
        primary: String,
    },
    PrivateInBaseOfClassExpression {
        declaration_name: String,
        property_name: String,
        needs_variable_annotation: bool,
    },
    Truncation,
    SymbolAccessibility {
        symbol_name: String,
        module_name: String,
        type_name: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolAccessibility {
    Accessible,
    NotResolved,
    CannotBeNamed,
    NotAccessible,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SymbolAccessibilityResult {
    pub accessibility: Option<SymbolAccessibility>,
    pub aliases_to_make_visible: Vec<String>,
    pub error_symbol_name: Option<String>,
    pub error_module_name: Option<String>,
    pub type_name: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SymbolTrackerSharedState {
    pub late_marked_statements: Vec<String>,
    pub diagnostics: Vec<DeclarationTrackerDiagnostic>,
    pub error_name_node: Option<String>,
    pub isolated_declarations: bool,
    pub strip_internal: bool,
    pub current_source_file: Option<String>,
    pub current_source_file_is_js: bool,
}

impl SymbolTrackerSharedState {
    pub fn add_diagnostic(&mut self, diagnostic: DeclarationTrackerDiagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn mark_aliases_visible(&mut self, aliases: &[String]) {
        for alias in aliases {
            if !self.late_marked_statements.contains(alias) {
                self.late_marked_statements.push(alias.clone());
            }
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeclarationTracker {
    pub state: SymbolTrackerSharedState,
    fallback_stack: Vec<String>,
}

impl DeclarationTracker {
    pub fn new(state: SymbolTrackerSharedState) -> Self {
        Self {
            state,
            fallback_stack: Vec::new(),
        }
    }

    pub fn push_error_fallback_node(&mut self, node: impl Into<String>) {
        self.fallback_stack.push(node.into());
    }

    pub fn pop_error_fallback_node(&mut self) {
        self.fallback_stack.pop();
    }

    pub fn error_location(&self) -> Option<&str> {
        self.state
            .error_name_node
            .as_deref()
            .or_else(|| self.fallback_stack.last().map(String::as_str))
    }

    pub fn error_declaration_name_with_fallback(&self) -> &str {
        self.error_location().unwrap_or("(Missing)")
    }

    pub fn report_cyclic_structure_error(&mut self) {
        let declaration_name = self.error_declaration_name_with_fallback().to_owned();
        self.state
            .add_diagnostic(DeclarationTrackerDiagnostic::CyclicStructure { declaration_name });
    }

    pub fn report_inaccessible_this_error(&mut self) {
        let declaration_name = self.error_declaration_name_with_fallback().to_owned();
        self.state
            .add_diagnostic(DeclarationTrackerDiagnostic::InaccessibleThis { declaration_name });
    }

    pub fn report_inaccessible_unique_symbol_error(&mut self) {
        let declaration_name = self.error_declaration_name_with_fallback().to_owned();
        self.state
            .add_diagnostic(DeclarationTrackerDiagnostic::InaccessibleUniqueSymbol {
                declaration_name,
            });
    }

    pub fn report_inference_fallback(
        &mut self,
        node: impl Into<String>,
        node_source_file: Option<&str>,
        is_expando_function_declaration_unsafe: bool,
    ) -> bool {
        if !self.state.isolated_declarations || self.state.current_source_file_is_js {
            return false;
        }

        if node_source_file != self.state.current_source_file.as_deref() {
            return false;
        }

        let node = node.into();
        if is_expando_function_declaration_unsafe {
            self.state
                .add_diagnostic(DeclarationTrackerDiagnostic::NonSerializableProperty {
                    property_name: node,
                });
        } else {
            self.state
                .add_diagnostic(DeclarationTrackerDiagnostic::InferenceFallback { node });
        }
        true
    }

    pub fn report_likely_unsafe_import_required_error(
        &mut self,
        specifier: impl Into<String>,
        symbol_name: Option<String>,
    ) {
        let declaration_name = self.error_declaration_name_with_fallback().to_owned();
        self.state
            .add_diagnostic(DeclarationTrackerDiagnostic::LikelyUnsafeImportRequired {
                declaration_name,
                specifier: specifier.into(),
                symbol_name,
            });
    }

    pub fn report_nonlocal_augmentation(
        &mut self,
        augmentation: impl Into<String>,
        primary: impl Into<String>,
    ) {
        self.state
            .add_diagnostic(DeclarationTrackerDiagnostic::NonlocalAugmentation {
                augmentation: augmentation.into(),
                primary: primary.into(),
            });
    }

    pub fn report_private_in_base_of_class_expression(
        &mut self,
        property_name: impl Into<String>,
        needs_variable_annotation: bool,
    ) {
        let declaration_name = self.error_declaration_name_with_fallback().to_owned();
        self.state.add_diagnostic(
            DeclarationTrackerDiagnostic::PrivateInBaseOfClassExpression {
                declaration_name,
                property_name: property_name.into(),
                needs_variable_annotation,
            },
        );
    }

    pub fn report_truncation_error(&mut self) {
        self.state
            .add_diagnostic(DeclarationTrackerDiagnostic::Truncation);
    }

    pub fn track_symbol(
        &mut self,
        is_type_parameter: bool,
        accessibility: SymbolAccessibilityResult,
    ) -> bool {
        if is_type_parameter {
            return false;
        }
        self.handle_symbol_accessibility_error(accessibility)
    }

    pub fn handle_symbol_accessibility_error(&mut self, result: SymbolAccessibilityResult) -> bool {
        match result.accessibility {
            Some(SymbolAccessibility::Accessible) => {
                self.state
                    .mark_aliases_visible(&result.aliases_to_make_visible);
                false
            }
            Some(SymbolAccessibility::NotResolved) | None => false,
            Some(SymbolAccessibility::CannotBeNamed | SymbolAccessibility::NotAccessible) => {
                let symbol_name = result.error_symbol_name.unwrap_or_default();
                let module_name = result.error_module_name.unwrap_or_default();
                self.state
                    .add_diagnostic(DeclarationTrackerDiagnostic::SymbolAccessibility {
                        symbol_name,
                        module_name,
                        type_name: result.type_name,
                    });
                true
            }
        }
    }
}
