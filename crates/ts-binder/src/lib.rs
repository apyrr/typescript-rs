#![forbid(unsafe_code)]

#[expect(dead_code, reason = "ported binder API is ahead of current callers")]
mod binder;
#[cfg(test)]
mod binder_test;
mod nameresolver;
#[expect(
    dead_code,
    reason = "ported binder state API is ahead of current callers"
)]
mod program_binding_state;
#[expect(
    dead_code,
    reason = "ported reference resolver API is ahead of current callers"
)]
mod referenceresolver;

pub use binder::{
    CONTAINER_FLAGS_IS_CONTAINER, ContainerFlags, bind_parsed_source_file, bind_source_file,
    bind_source_file_view, find_use_strict_prologue, get_container_flags,
    get_symbol_name_for_private_identifier,
};
pub use nameresolver::{NameResolver, NameResolverHooks, get_local_symbol_for_export_default};
pub use program_binding_state::{BinderFlagUpdate, ProgramBindingState};
pub use referenceresolver::{
    BinderReferenceResolver, ReferenceResolver, ReferenceResolverHooks, new_reference_resolver,
};
