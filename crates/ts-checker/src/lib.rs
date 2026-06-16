#![forbid(unsafe_code)]

pub use ts_ast as ast;
pub use ts_astnav as astnav;
pub use ts_binder as binder;
pub use ts_collections as collections;
pub use ts_core as core;
pub use ts_debug as debug;
pub use ts_diagnostics as diagnostics;
pub use ts_evaluator as evaluator;
pub use ts_jsnum as jsnum;
pub use ts_module as module;
pub use ts_modulespecifiers as modulespecifiers;
pub use ts_parser as parser;
pub use ts_pseudochecker as pseudochecker;
pub use ts_scanner as scanner;
pub use ts_stringutil as stringutil;
pub use ts_tracing as tracing;
pub use ts_tsoptions as tsoptions;
pub use ts_tspath as tspath;

#[expect(
    dead_code,
    private_interfaces,
    unused_assignments,
    unused_imports,
    unused_mut,
    unused_parens,
    unused_variables,
    reason = "ported checker implementation is ahead of current callers"
)]
pub(crate) mod checker;
#[expect(
    dead_code,
    private_interfaces,
    reason = "ported emit resolver implementation is ahead of current callers"
)]
pub(crate) mod emitresolver;
pub(crate) mod exports;
#[expect(
    dead_code,
    reason = "ported flow graph helpers are ahead of current callers"
)]
pub(crate) mod flow;
#[expect(
    dead_code,
    unused_variables,
    reason = "ported grammar checks retain upstream callback shape"
)]
pub(crate) mod grammarchecks;
#[expect(
    dead_code,
    reason = "ported inference helpers are ahead of current callers"
)]
pub(crate) mod inference;
#[expect(
    dead_code,
    reason = "ported JSX checker helpers are ahead of current callers"
)]
pub(crate) mod jsx;
#[expect(
    dead_code,
    reason = "ported node builder helpers are ahead of current callers"
)]
pub(crate) mod nodebuilder;
pub(crate) mod nodebuilder_hover;
#[expect(
    dead_code,
    private_interfaces,
    unreachable_code,
    unused_assignments,
    unused_mut,
    unused_variables,
    reason = "ported node builder implementation is ahead of current callers"
)]
pub(crate) mod nodebuilderimpl;
pub(crate) mod nodebuilderscopes;
#[expect(
    dead_code,
    noop_method_call,
    reason = "ported node copy helpers are ahead of current callers"
)]
pub(crate) mod nodecopy;
#[expect(
    dead_code,
    private_interfaces,
    unused_mut,
    reason = "ported checker printer helpers are ahead of current callers"
)]
pub(crate) mod printer;
#[expect(
    unreachable_code,
    unreachable_patterns,
    unused_variables,
    reason = "ported pseudotype node builder keeps defensive upstream fallback shape"
)]
pub(crate) mod pseudotypenodebuilder;
#[expect(
    dead_code,
    unused_assignments,
    unused_doc_comments,
    unused_mut,
    reason = "ported relation checker is ahead of current callers"
)]
pub(crate) mod relater;
#[expect(
    dead_code,
    non_upper_case_globals,
    private_interfaces,
    unused_macros,
    reason = "ported semantic model is ahead of current callers"
)]
pub(crate) mod semantic;
#[expect(
    dead_code,
    private_interfaces,
    reason = "ported service APIs expose checker-internal identities"
)]
pub(crate) mod services;
#[expect(
    dead_code,
    reason = "generated checker stringer is ahead of current callers"
)]
pub(crate) mod stringer_generated;
#[expect(
    private_interfaces,
    reason = "ported symbol accessibility APIs expose checker-internal identities"
)]
pub(crate) mod symbolaccessibility;
#[expect(
    dead_code,
    reason = "ported symbol tracker is ahead of current callers"
)]
pub(crate) mod symboltracker;
#[expect(
    dead_code,
    reason = "ported tracer helpers are ahead of current callers"
)]
pub(crate) mod tracer;
#[expect(
    dead_code,
    reason = "ported checker type model is ahead of current callers"
)]
pub(crate) mod types;
#[expect(
    dead_code,
    reason = "ported checker utilities are ahead of current callers"
)]
pub(crate) mod utilities;

pub use checker::{Checker, Host, Program};
pub use exports::{
    get_property_name_from_type_public, is_type_usable_as_property_name_public,
    try_get_module_specifier_from_declaration,
};
pub use nodebuilder::VerbosityContext;
pub use semantic::{
    CheckerGeneration, CheckerSlotId, CheckerState, CheckerStateIdentity, IndexInfoHandle,
    LiteralValue, SignatureHandle, SourceFileIdentity, TypeHandle, TypeId,
    TypeMapperAllocationCounters, TypeMapperPerfCounters, TypePredicateHandle,
    UNION_REDUCTION_NONE,
};
pub use services::get_resolved_signature_for_signature_help;
pub use types::{
    CONTEXT_FLAGS_IGNORE_NODE_INFERENCES, CONTEXT_FLAGS_NONE, ContextFlags, ELEMENT_FLAGS_REQUIRED,
    ElementFlags, OBJECT_FLAGS_CLASS_OR_INTERFACE, OBJECT_FLAGS_REFERENCE, OBJECT_FLAGS_TUPLE,
    SIGNATURE_KIND_CALL, SIGNATURE_KIND_CONSTRUCT, SYMBOL_FORMAT_FLAGS_ALLOW_ANY_NODE_KIND,
    SYMBOL_FORMAT_FLAGS_USE_ALIAS_DEFINED_OUTSIDE_CURRENT_SCOPE,
    SYMBOL_FORMAT_FLAGS_USE_ONLY_EXTERNAL_ALIASING,
    SYMBOL_FORMAT_FLAGS_WRITE_TYPE_PARAMETERS_OR_ARGUMENTS, SignatureKind, SymbolFormatFlags,
    TYPE_FLAGS_ANY, TYPE_FLAGS_ANY_OR_UNKNOWN, TYPE_FLAGS_CONDITIONAL, TYPE_FLAGS_INDEX,
    TYPE_FLAGS_INDEXED_ACCESS, TYPE_FLAGS_LITERAL, TYPE_FLAGS_NONE, TYPE_FLAGS_OBJECT,
    TYPE_FLAGS_PRIMITIVE, TYPE_FLAGS_STRING_LIKE, TYPE_FLAGS_STRING_MAPPING,
    TYPE_FLAGS_STRUCTURED_TYPE, TYPE_FLAGS_SUBSTITUTION, TYPE_FLAGS_TEMPLATE_LITERAL,
    TYPE_FLAGS_UNDEFINED, TYPE_FLAGS_UNION, TYPE_FLAGS_UNION_OR_INTERSECTION,
    TYPE_FLAGS_UNIQUE_ES_SYMBOL, TYPE_FLAGS_VOID, TYPE_FORMAT_FLAGS_ALLOW_UNIQUE_ES_SYMBOL_TYPE,
    TYPE_FORMAT_FLAGS_MULTILINE_OBJECT_LITERALS,
    TYPE_FORMAT_FLAGS_USE_ALIAS_DEFINED_OUTSIDE_CURRENT_SCOPE,
    TYPE_FORMAT_FLAGS_USE_INSTANTIATION_EXPRESSIONS, TYPE_FORMAT_FLAGS_WRITE_CALL_STYLE_SIGNATURE,
    TYPE_FORMAT_FLAGS_WRITE_TYPE_ARGUMENTS_OF_SIGNATURE, TypeFormatFlags,
};
pub use utilities::{
    create_mode_mismatch_details, create_module_not_found_chain, get_set_accessor_value_parameter,
    is_external_module_symbol, is_in_type_query, is_type_any,
};
