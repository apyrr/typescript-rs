use std::collections::{BTreeMap, BTreeSet};

use ts_core::ModuleKind;

pub const EXTERNAL_HELPERS_MODULE_NAME_TEXT: &str = "tslib";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExternalModuleInfo {
    pub external_imports: Vec<String>,
    pub export_specifiers: BTreeMap<String, Vec<String>>,
    pub exported_bindings: BTreeMap<String, Vec<String>>,
    pub exported_names: Vec<String>,
    pub exported_functions: BTreeSet<String>,
    pub export_equals: Option<String>,
    pub has_export_stars_to_export_values: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExternalModuleInfoCollector {
    unique_exports: BTreeSet<String>,
    has_export_default: bool,
    output: ExternalModuleInfo,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ImportHelperFacts {
    pub has_namespace_import: bool,
    pub has_default_import: bool,
    pub named_default_reference_count: usize,
    pub named_reference_count: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExternalHelpersFacts {
    pub import_helpers: bool,
    pub is_effective_external_module: bool,
    pub compiler_module_kind: ModuleKind,
    pub file_module_kind: ModuleKind,
    pub unscoped_helper_count: usize,
    pub has_export_stars_to_export_values: bool,
    pub has_import_star: bool,
    pub has_import_default: bool,
    pub has_existing_external_helpers_module_name: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalHelpersImportKind {
    None,
    CommonJsImportEquals,
    EsNamedImports,
}

impl ExternalModuleInfoCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn finish(self) -> ExternalModuleInfo {
        self.output
    }

    pub fn add_unique_export(&mut self, name: impl Into<String>) -> bool {
        self.unique_exports.insert(name.into())
    }

    pub fn remove_unique_export(&mut self, name: &str) -> bool {
        self.unique_exports.remove(name)
    }

    pub fn add_external_import(&mut self, import: impl Into<String>) {
        self.output.external_imports.push(import.into());
    }

    pub fn add_exported_name(&mut self, name: impl Into<String>) {
        self.output.exported_names.push(name.into());
    }

    pub fn add_exported_binding(
        &mut self,
        declaration: impl Into<String>,
        name: impl Into<String>,
    ) {
        self.output
            .exported_bindings
            .entry(declaration.into())
            .or_default()
            .push(name.into());
    }

    pub fn add_export_specifier(
        &mut self,
        local_name: impl Into<String>,
        specifier: impl Into<String>,
    ) {
        self.output
            .export_specifiers
            .entry(local_name.into())
            .or_default()
            .push(specifier.into());
    }

    pub fn add_exported_function_declaration(
        &mut self,
        declaration: impl Into<String>,
        export_name: Option<String>,
        is_default: bool,
    ) {
        let declaration = declaration.into();
        self.output.exported_functions.insert(declaration.clone());
        if is_default {
            if !self.has_export_default {
                self.add_exported_binding(
                    declaration,
                    export_name.unwrap_or_else(|| "default".to_owned()),
                );
                self.has_export_default = true;
            }
            return;
        }

        let name = export_name.unwrap_or_else(|| declaration.clone());
        if self.add_unique_export(name.clone()) {
            self.add_exported_binding(declaration, name);
        }
    }
}

pub fn contains_default_reference(names: &[String]) -> bool {
    names.iter().any(|name| name == "default")
}

pub fn get_import_needs_import_star_helper(facts: ImportHelperFacts) -> bool {
    if facts.has_namespace_import {
        return true;
    }

    let non_default_count = facts
        .named_reference_count
        .saturating_sub(facts.named_default_reference_count);
    (facts.named_default_reference_count > 0
        && facts.named_default_reference_count != facts.named_reference_count)
        || (non_default_count != 0 && facts.has_default_import)
}

pub fn get_import_needs_import_default_helper(facts: ImportHelperFacts) -> bool {
    !get_import_needs_import_star_helper(facts)
        && (facts.has_default_import || facts.named_default_reference_count != 0)
}

pub fn get_or_create_external_helpers_module_name_needed(facts: ExternalHelpersFacts) -> bool {
    facts.has_existing_external_helpers_module_name
        || facts.unscoped_helper_count > 0
        || ((facts.has_export_stars_to_export_values
            || facts.has_import_star
            || facts.has_import_default)
            && facts.file_module_kind < ModuleKind::System)
}

pub fn external_helpers_import_kind(facts: ExternalHelpersFacts) -> ExternalHelpersImportKind {
    if !facts.import_helpers || !facts.is_effective_external_module {
        return ExternalHelpersImportKind::None;
    }

    let is_common_js_file = facts.file_module_kind == ModuleKind::CommonJS
        || (facts.file_module_kind == ModuleKind::None
            && facts.compiler_module_kind == ModuleKind::CommonJS);
    if is_common_js_file {
        if get_or_create_external_helpers_module_name_needed(facts) {
            ExternalHelpersImportKind::CommonJsImportEquals
        } else {
            ExternalHelpersImportKind::None
        }
    } else if facts.unscoped_helper_count > 0 {
        ExternalHelpersImportKind::EsNamedImports
    } else {
        ExternalHelpersImportKind::None
    }
}
