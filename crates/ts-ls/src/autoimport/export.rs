use ts_ast as ast;
use ts_checker as checker;
use ts_tspath as tspath;

use crate::autoimport::{ExportSyntax, new_symbol_extractor};
use crate::lsutil;

// ModuleID uniquely identifies a module across multiple declarations.
// If the export is from an ambient module declaration, this is the module name.
// If the export is from a module augmentation, this is the Path() of the resolved module file.
// Otherwise this is the Path() of the exporting source file.
pub type ModuleId = String;

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct ExportId {
    pub module_id: ModuleId,
    pub export_name: String,
}

#[derive(Clone, Debug, Default)]
pub struct Export {
    pub export_id: ExportId,
    pub module_file_name: String,
    pub syntax: ExportSyntax,
    pub flags: ast::SymbolFlags,
    pub local_name: String,
    // through is the name of the module symbol's export that this export was found on,
    // either 'export=', InternalSymbolNameExportStar, or empty string.
    pub through: String,

    // Checker-set fields
    pub target: ExportId,
    pub is_type_only: bool,
    pub script_element_kind: lsutil::ScriptElementKind,
    pub script_element_kind_modifiers: lsutil::ScriptElementKindModifier,

    // The file where the export was found.
    pub path: tspath::Path,

    pub package_name: String,
}

impl Export {
    pub fn module_id(&self) -> &str {
        &self.export_id.module_id
    }

    pub fn export_name(&self) -> &str {
        &self.export_id.export_name
    }

    pub fn name(&self) -> &str {
        if !self.local_name.is_empty() {
            return &self.local_name;
        }
        if self.export_id.export_name == ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS {
            return &self.target.export_name;
        }
        &self.export_id.export_name
    }

    pub fn is_renameable(&self) -> bool {
        self.export_id.export_name == ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS
            || self.export_id.export_name == ast::INTERNAL_SYMBOL_NAME_DEFAULT
    }

    pub fn ambient_module_name(&self) -> &str {
        if !tspath::is_external_module_name_relative(self.module_id()) {
            return self.module_id();
        }
        ""
    }

    pub fn is_unresolved_alias(&self) -> bool {
        self.flags == ast::SYMBOL_FLAGS_ALIAS
    }
}

impl crate::autoimport::index::Named for Export {
    fn name(&self) -> &str {
        Export::name(self)
    }
}

pub(crate) fn symbol_identity_to_export<'a>(
    symbol: ast::SymbolIdentity,
    ch: &mut checker::Checker<'a, '_>,
) -> Option<Export> {
    let symbol_name = ch.symbol_name_public(symbol)?;
    if let Some(parent) = ch.symbol_parent_public(symbol) {
        if ch.is_external_module_symbol_public(parent) {
            let (module_id, module_file_name, ok) =
                crate::autoimport::util::try_get_module_id_and_file_name_of_module_symbol(
                    ch, parent,
                );
            if ok {
                let declaration = ch.symbol_value_declaration_public(parent).or_else(|| {
                    ch.collect_symbol_declarations_public(parent)
                        .first()
                        .copied()
                })?;
                let source_file = ch.try_source_file_for_node_public(declaration)?;
                let store = source_file.store();
                if let Some(file) = ast::get_source_file_of_node(store, Some(declaration)) {
                    let file = store.source_file_view(file);
                    return extract_first_export(symbol, ch, module_id, &module_file_name, file);
                }
            }
            return None;
        }
    }

    let declaration = ch
        .collect_symbol_declarations_public(symbol)
        .first()
        .copied()?;
    let source_file = ch.try_source_file_for_node_public(declaration)?;
    let store = source_file.store();
    let file = ast::get_source_file_of_node(store, Some(declaration))?;
    let file = store.source_file_view(file);
    let file_symbol = ch.source_node_symbol_public(file.as_node())?;
    let module_symbol = ch.get_merged_symbol_public(file_symbol)?;
    let module_id = file.path();
    let module_file_name = file.file_name();
    let skipped = ch.skip_alias_public(symbol)?;
    let target = ch.get_merged_symbol_public(skipped)?;

    if let Some(export) = try_get_module_export(
        ast::INTERNAL_SYMBOL_NAME_DEFAULT,
        target,
        module_symbol,
        ch,
        module_id.clone(),
        &module_file_name,
        file,
    ) {
        return Some(export);
    }
    if let Some(export) = try_get_module_export(
        ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS,
        target,
        module_symbol,
        ch,
        module_id.clone(),
        &module_file_name,
        file,
    ) {
        return Some(export);
    }
    try_get_module_export(
        &symbol_name,
        target,
        module_symbol,
        ch,
        module_id,
        &module_file_name,
        file,
    )
}

pub(crate) fn try_get_module_export<'a>(
    export_name: &str,
    target: ast::SymbolIdentity,
    module_symbol: ast::SymbolIdentity,
    ch: &mut checker::Checker<'a, '_>,
    module_id: ModuleId,
    module_file_name: &str,
    file: ast::SourceFileView<'_>,
) -> Option<Export> {
    let exported =
        ch.try_get_member_in_module_exports_and_properties(export_name, module_symbol)?;
    let skipped = ch.skip_alias_public(exported)?;
    let exported_target = ch.get_merged_symbol_public(skipped)?;
    if exported_target == target {
        return extract_first_export(exported, ch, module_id, module_file_name, file);
    }
    None
}

pub fn extract_first_export<'a>(
    symbol: ast::SymbolIdentity,
    ch: &mut checker::Checker<'a, '_>,
    module_id: ModuleId,
    module_file_name: &str,
    file: ast::SourceFileView<'_>,
) -> Option<Export> {
    let mut exports = Vec::new();
    let extractor = new_symbol_extractor(String::new(), ch, None, None);
    let symbol_name = extractor.checker.borrow_mut().symbol_name_public(symbol)?;
    extractor.extract_from_symbol_identity(
        &symbol_name,
        symbol,
        module_id,
        module_file_name,
        &file,
        &mut exports,
    );
    exports.into_iter().next()
}
