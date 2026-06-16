pub(crate) mod aliasresolver;
pub(crate) mod export;
mod export_stringer_generated;
pub(crate) mod extract;
pub(crate) mod fix;
pub(crate) mod import_adder;
pub(crate) mod index;
#[cfg(test)]
mod index_test;
pub(crate) mod registry;
pub(crate) mod specifiers;
#[cfg(test)]
mod testmain_test;
pub(crate) mod util;
#[cfg(test)]
mod util_test;
pub(crate) mod view;

pub use aliasresolver::RegistryCloneHost;
pub(crate) use aliasresolver::{PathAndFileName, new_alias_resolver};
pub(crate) use export::{Export, ExportId, ModuleId, symbol_identity_to_export};
pub(crate) use export_stringer_generated::ExportSyntax;
pub(crate) use extract::{new_export_extractor, new_symbol_extractor};
pub(crate) use fix::{
    ExistingImport, Fix, NewImportBinding, add_namespace_qualifier, add_to_existing_import,
    get_add_to_existing_import_fix, make_new_import_text_from_bindings,
};
pub(crate) use import_adder::{
    ImportAdder, new_import_adder, try_get_auto_importable_reference_from_type_node,
    try_get_auto_importable_reference_from_type_node_from_identifiers,
    type_node_to_auto_importable_type_node,
};
pub(crate) use index::Index;
pub(crate) use registry::RegistryBucket;
pub use registry::{BucketStats, CacheStats, Registry, RegistryChange, new_registry};
pub(crate) use util::{
    add_package_json_dependencies, add_project_reference_output_mappings, get_module_resolver,
    get_package_names_in_node_modules, get_package_realpath_funcs, get_resolved_package_names,
    word_indices,
};
pub(crate) use view::{FixAndExport, QueryKind, View, new_view};
