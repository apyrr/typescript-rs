use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Once};

use smallvec::SmallVec;
use ts_ast as ast;
use ts_collections as collections;
use ts_collections::{Arena, Idx, Set};
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_evaluator as evaluator;
use ts_jsnum as jsnum;
use ts_scanner as scanner;
use xxhash_rust::xxh3;

use crate::emitresolver::{DeclarationFileLinks, DeclarationLinks, JSXLinks};
use crate::flow::{FlowState, SharedFlow};
use crate::relater::{ExpandingFlags, Relation, RelationComparisonResult, RelationKind};
use crate::types::{
    ElementFlags, IndexFlags, OBJECT_FLAGS_NONE, ObjectFlags, SIGNATURE_FLAGS_NONE, SignatureFlags,
    TYPE_FLAGS_ANY, TYPE_FLAGS_UNIQUE_ES_SYMBOL, TYPE_PREDICATE_KIND_IDENTIFIER, Ternary,
    TupleElementInfo, TypeFlags, TypePredicateKind,
};
macro_rules! semantic_handle {
    ($handle:ident, $record:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub struct $handle(Idx<$record>);

        impl $handle {
            pub(crate) fn new(idx: Idx<$record>) -> Self {
                Self(idx)
            }

            pub(crate) fn idx(self) -> Idx<$record> {
                self.0
            }
        }
    };
}

macro_rules! semantic_ordered_handle {
    ($handle:ident, $record:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $handle(Idx<$record>);

        impl $handle {
            pub(crate) fn new(idx: Idx<$record>) -> Self {
                Self(idx)
            }

            pub(crate) fn idx(self) -> Idx<$record> {
                self.0
            }
        }
    };
}

semantic_ordered_handle!(TypeHandle, TypeRecord);
semantic_ordered_handle!(SignatureHandle, SignatureRecord);
semantic_ordered_handle!(IndexInfoHandle, IndexInfoRecord);
semantic_ordered_handle!(TypeAliasHandle, TypeAliasRecord);
semantic_ordered_handle!(TypePredicateHandle, TypePredicateRecord);
semantic_ordered_handle!(ConditionalRootHandle, ConditionalRootRecord);
semantic_ordered_handle!(InferenceContextHandle, InferenceContextRecord);
semantic_ordered_handle!(TypeMapperHandle, TypeMapperRecord);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TransientSymbolHandle(ast::SymbolHandle);

impl TransientSymbolHandle {
    pub(crate) fn new(symbol: ast::SymbolHandle) -> Self {
        Self(symbol)
    }

    pub(crate) fn symbol(self) -> ast::SymbolHandle {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CheckerSlotId(u32);

impl CheckerSlotId {
    pub const fn new(slot: u32) -> Self {
        Self(slot)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CheckerGeneration(u32);

impl CheckerGeneration {
    pub const fn initial() -> Self {
        Self(0)
    }

    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CheckerStateIdentity {
    slot: CheckerSlotId,
    generation: CheckerGeneration,
}

impl CheckerStateIdentity {
    pub const fn new(slot: CheckerSlotId, generation: CheckerGeneration) -> Self {
        Self { slot, generation }
    }

    pub const fn slot(self) -> CheckerSlotId {
        self.slot
    }

    pub const fn generation(self) -> CheckerGeneration {
        self.generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckerDerivedOptions {
    pub language_version: core::ScriptTarget,
    pub module_kind: core::ModuleKind,
    pub module_resolution_kind: core::ModuleResolutionKind,
    pub legacy_decorators: bool,
    pub emit_standard_class_fields: bool,
    pub strict_null_checks: bool,
    pub strict_function_types: bool,
    pub strict_bind_call_apply: bool,
    pub strict_property_initialization: bool,
    pub strict_builtin_iterator_return: bool,
    pub no_implicit_any: bool,
    pub no_implicit_this: bool,
    pub use_unknown_in_catch_variables: bool,
    pub exact_optional_property_types: bool,
}

impl Default for CheckerDerivedOptions {
    fn default() -> Self {
        Self {
            language_version: core::ScriptTarget::None,
            module_kind: core::ModuleKind::None,
            module_resolution_kind: core::ModuleResolutionKind::Unknown,
            legacy_decorators: false,
            emit_standard_class_fields: false,
            strict_null_checks: false,
            strict_function_types: false,
            strict_bind_call_apply: false,
            strict_property_initialization: false,
            strict_builtin_iterator_return: false,
            no_implicit_any: false,
            no_implicit_this: false,
            use_unknown_in_catch_variables: false,
            exact_optional_property_types: false,
        }
    }
}

impl CheckerDerivedOptions {
    pub fn from_compiler_options(options: &core::CompilerOptions) -> Self {
        Self {
            language_version: options.get_emit_script_target(),
            module_kind: options.get_emit_module_kind(),
            module_resolution_kind: options.get_module_resolution_kind(),
            legacy_decorators: options.experimental_decorators == core::TSTrue,
            emit_standard_class_fields: options.get_emit_standard_class_fields(),
            strict_null_checks: options.get_strict_option_value(options.strict_null_checks),
            strict_function_types: options.get_strict_option_value(options.strict_function_types),
            strict_bind_call_apply: options.get_strict_option_value(options.strict_bind_call_apply),
            strict_property_initialization: options
                .get_strict_option_value(options.strict_property_initialization),
            strict_builtin_iterator_return: options
                .get_strict_option_value(options.strict_builtin_iterator_return),
            no_implicit_any: options.get_strict_option_value(options.no_implicit_any),
            no_implicit_this: options.get_strict_option_value(options.no_implicit_this),
            use_unknown_in_catch_variables: options
                .get_strict_option_value(options.use_unknown_in_catch_variables),
            exact_optional_property_types: options.exact_optional_property_types == core::TSTrue,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct SymbolIdentity(ast::SymbolIdentity);

impl SymbolIdentity {
    pub(crate) const fn from_symbol_handle(handle: ast::SymbolHandle) -> Self {
        Self(ast::SymbolIdentity::from_symbol_handle(handle))
    }

    pub(crate) const fn ast_identity(self) -> ast::SymbolIdentity {
        self.0
    }

    pub(crate) const fn symbol_handle(self) -> ast::SymbolHandle {
        self.0.symbol_handle()
    }
}

impl From<ast::SymbolIdentity> for SymbolIdentity {
    fn from(identity: ast::SymbolIdentity) -> Self {
        Self(identity)
    }
}

impl From<SymbolIdentity> for ast::SymbolIdentity {
    fn from(identity: SymbolIdentity) -> Self {
        identity.ast_identity()
    }
}

pub(crate) type SymbolIdentityTable =
    indexmap::IndexMap<ast::SymbolName, SymbolIdentity, collections::GxBuildHasher>;

#[derive(Clone, Copy)]
pub(crate) struct GlobalSymbolTableView<'a> {
    globals: &'a SymbolIdentityTable,
}

impl<'a> GlobalSymbolTableView<'a> {
    pub(crate) fn len(self) -> usize {
        self.globals.len()
    }

    pub(crate) fn is_empty(self) -> bool {
        self.globals.is_empty()
    }

    pub(crate) fn get(self, name: &str) -> Option<SymbolIdentity> {
        self.globals.get(name).copied()
    }

    pub(crate) fn contains_key(self, name: &str) -> bool {
        self.globals.contains_key(name)
    }

    pub(crate) fn get_index(self, index: usize) -> Option<(ast::SymbolName, SymbolIdentity)> {
        self.globals
            .get_index(index)
            .map(|(name, &symbol)| (name.clone(), symbol))
    }

    pub(crate) fn for_each(self, mut f: impl FnMut(&ast::SymbolName, SymbolIdentity)) {
        for (name, &symbol) in self.globals {
            f(name, symbol);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SymbolOrigin {
    ProgramHandle(ast::SymbolHandle),
    Transient(ast::SymbolHandle),
}

#[derive(Default)]
pub struct SymbolRegistry;

impl SymbolRegistry {
    pub(crate) fn intern_handle(&mut self, symbol: ast::SymbolHandle) -> SymbolIdentity {
        SymbolIdentity::from_symbol_handle(symbol)
    }

    pub(crate) fn intern_transient(&mut self, symbol: ast::SymbolHandle) -> SymbolIdentity {
        SymbolIdentity::from_symbol_handle(symbol)
    }

    pub(crate) fn origin(&self, identity: SymbolIdentity) -> Option<SymbolOrigin> {
        let symbol = identity.symbol_handle();
        match symbol.domain() {
            ast::SymbolDomain::Program => Some(SymbolOrigin::ProgramHandle(symbol)),
            ast::SymbolDomain::CheckerTransient => Some(SymbolOrigin::Transient(symbol)),
        }
    }
}

impl core::IntoLinkKey<SymbolIdentity> for ast::SymbolHandle {
    fn into_link_key(self) -> SymbolIdentity {
        SymbolIdentity::from_symbol_handle(self)
    }
}

impl core::IntoLinkKey<SymbolIdentity> for &ast::SymbolHandle {
    fn into_link_key(self) -> SymbolIdentity {
        SymbolIdentity::from_symbol_handle(*self)
    }
}

pub type Number = jsnum::Number;
pub type PseudoBigInt = jsnum::PseudoBigInt;
pub(crate) type CacheHashKey = u128;
pub type TypeId = u32;

pub type ExternalEmitHelpers = u32;

pub const EXTERNAL_EMIT_HELPERS_REST: ExternalEmitHelpers = 1 << 0; // __rest (used by ESNext object rest transformation)
pub const EXTERNAL_EMIT_HELPERS_DECORATE: ExternalEmitHelpers = 1 << 1; // __decorate (used by TypeScript decorators transformation)
pub const EXTERNAL_EMIT_HELPERS_METADATA: ExternalEmitHelpers = 1 << 2; // __metadata (used by TypeScript decorators transformation)
pub const EXTERNAL_EMIT_HELPERS_PARAM: ExternalEmitHelpers = 1 << 3; // __param (used by TypeScript decorators transformation)
pub const EXTERNAL_EMIT_HELPERS_AWAITER: ExternalEmitHelpers = 1 << 4; // __awaiter (used by ES2017 async functions transformation)
pub const EXTERNAL_EMIT_HELPERS_AWAIT: ExternalEmitHelpers = 1 << 5; // __await (used by ES2017 async generator transformation)
pub const EXTERNAL_EMIT_HELPERS_ASYNC_GENERATOR: ExternalEmitHelpers = 1 << 6; // __asyncGenerator (used by ES2017 async generator transformation)
pub const EXTERNAL_EMIT_HELPERS_ASYNC_DELEGATOR: ExternalEmitHelpers = 1 << 7; // __asyncDelegator (used by ES2017 async generator yield* transformation)
pub const EXTERNAL_EMIT_HELPERS_ASYNC_VALUES: ExternalEmitHelpers = 1 << 8; // __asyncValues (used by ES2017 for, ..await, ..of transformation)
pub const EXTERNAL_EMIT_HELPERS_EXPORT_STAR: ExternalEmitHelpers = 1 << 9; // __exportStar (used by CommonJS/AMD/UMD module transformation)
pub const EXTERNAL_EMIT_HELPERS_IMPORT_STAR: ExternalEmitHelpers = 1 << 10; // __importStar (used by CommonJS/AMD/UMD module transformation)
pub const EXTERNAL_EMIT_HELPERS_IMPORT_DEFAULT: ExternalEmitHelpers = 1 << 11; // __importDefault (used by CommonJS/AMD/UMD module transformation)
pub const EXTERNAL_EMIT_HELPERS_MAKE_TEMPLATE_OBJECT: ExternalEmitHelpers = 1 << 12; // __makeTemplateObject (used for constructing template string array objects)
pub const EXTERNAL_EMIT_HELPERS_CLASS_PRIVATE_FIELD_GET: ExternalEmitHelpers = 1 << 13; // __classPrivateFieldGet (used by the class private field transformation)
pub const EXTERNAL_EMIT_HELPERS_CLASS_PRIVATE_FIELD_SET: ExternalEmitHelpers = 1 << 14; // __classPrivateFieldSet (used by the class private field transformation)
pub const EXTERNAL_EMIT_HELPERS_CLASS_PRIVATE_FIELD_IN: ExternalEmitHelpers = 1 << 15; // __classPrivateFieldIn (used by the class private field transformation)
pub const EXTERNAL_EMIT_HELPERS_SET_FUNCTION_NAME: ExternalEmitHelpers = 1 << 16; // __setFunctionName (used by class fields and ECMAScript decorators)
pub const EXTERNAL_EMIT_HELPERS_PROP_KEY: ExternalEmitHelpers = 1 << 17; // __propKey (used by class fields and ECMAScript decorators)
pub const EXTERNAL_EMIT_HELPERS_ADD_DISPOSABLE_RESOURCE_AND_DISPOSE_RESOURCES: ExternalEmitHelpers =
    1 << 18; // __addDisposableResource and __disposeResources (used by ESNext transformations)
pub const EXTERNAL_EMIT_HELPERS_REWRITE_RELATIVE_IMPORT_EXTENSION: ExternalEmitHelpers = 1 << 19; // __rewriteRelativeImportExtension (used by --rewriteRelativeImportExtensions)
pub const EXTERNAL_EMIT_HELPERS_ES_DECORATE_AND_RUN_INITIALIZERS: ExternalEmitHelpers =
    EXTERNAL_EMIT_HELPERS_DECORATE; // __esDecorate and __runInitializers (used by ECMAScript decorators transformation)

pub const EXTERNAL_EMIT_HELPERS_FIRST_EMIT_HELPER: ExternalEmitHelpers = EXTERNAL_EMIT_HELPERS_REST;
pub const EXTERNAL_EMIT_HELPERS_LAST_EMIT_HELPER: ExternalEmitHelpers =
    EXTERNAL_EMIT_HELPERS_REWRITE_RELATIVE_IMPORT_EXTENSION;

pub const EXTERNAL_EMIT_HELPERS_FOR_AWAIT_OF_INCLUDES: ExternalEmitHelpers =
    EXTERNAL_EMIT_HELPERS_ASYNC_VALUES;
pub const EXTERNAL_EMIT_HELPERS_ASYNC_GENERATOR_INCLUDES: ExternalEmitHelpers =
    EXTERNAL_EMIT_HELPERS_AWAIT | EXTERNAL_EMIT_HELPERS_ASYNC_GENERATOR;
pub const EXTERNAL_EMIT_HELPERS_ASYNC_DELEGATOR_INCLUDES: ExternalEmitHelpers =
    EXTERNAL_EMIT_HELPERS_AWAIT
        | EXTERNAL_EMIT_HELPERS_ASYNC_DELEGATOR
        | EXTERNAL_EMIT_HELPERS_ASYNC_VALUES;

pub const EXTERNAL_HELPERS_MODULE_NAME_TEXT: &str = "tslib";

#[derive(Default)]
pub(crate) struct SymbolReferenceLinks {
    reference_kinds: ast::SymbolFlags,
}

#[derive(Default)]
pub(crate) struct ValueSymbolLinks {
    resolved_type: Option<TypeHandle>,
    write_type: Option<TypeHandle>,
    target: Option<SymbolIdentity>,
    mapper: Option<TypeMapperHandle>,
    name_type: Option<TypeHandle>,
    containing_type: Option<TypeHandle>,
    function_or_constructor_checked: bool,
    cjs_export_merged: Option<SymbolIdentity>,
    inferred_class_symbol: HashMap<SymbolIdentity, SymbolIdentity>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ValueSymbolInstantiationSnapshot {
    pub(crate) resolved_type: Option<TypeHandle>,
    pub(crate) write_type: Option<TypeHandle>,
    pub(crate) target: Option<SymbolIdentity>,
    pub(crate) mapper: Option<TypeMapperHandle>,
    pub(crate) name_type: Option<TypeHandle>,
}

#[derive(Default)]
pub(crate) struct MappedSymbolLinks {
    key_type: Option<TypeHandle>,
    synthetic_origin: Option<SymbolIdentity>,
}

#[derive(Default)]
pub(crate) struct DeferredSymbolLinks {
    parent: Option<TypeHandle>,
    constituents: Vec<TypeHandle>,
    write_constituents: Vec<TypeHandle>,
}

#[derive(Default)]
pub(crate) struct AliasSymbolLinks {
    immediate_target: Option<SymbolIdentity>,
    alias_target: Option<SymbolIdentity>,
    referenced: bool,
    type_only_declaration: Option<ast::Node>,
    type_only_export_star_name: Option<String>,
}

#[derive(Default)]
pub(crate) struct ModuleSymbolLinks {
    resolved_exports: Option<SymbolIdentityTable>,
    type_only_export_star_map: HashMap<String, ast::Node>,
    exports_checked: bool,
}

#[derive(Default)]
pub(crate) struct ReverseMappedSymbolLinks {
    resolved_type: Option<TypeHandle>,
    property_type: Option<TypeHandle>,
    mapped_type: Option<TypeHandle>,
    constraint_type: Option<TypeHandle>,
}

#[derive(Default)]
pub(crate) struct LateBoundLinks {
    late_symbol: Option<SymbolIdentity>,
}

#[derive(Default)]
pub(crate) struct ExportTypeLinks {
    target: Option<SymbolIdentity>,
    originating_import: Option<ast::Node>,
}

#[derive(Default)]
pub(crate) struct TypeAliasLinks {
    declared_type: Option<TypeHandle>,
    type_parameters: Vec<TypeHandle>,
    instantiations: HashMap<CacheHashKey, TypeHandle>,
    is_constructor_declared_property: bool,
}

#[derive(Default)]
pub(crate) struct DeclaredTypeLinks {
    declared_type: Option<TypeHandle>,
    interface_checked: bool,
    index_signatures_checked: bool,
    type_parameters_checked: bool,
    enum_checked: bool,
}

pub type ExhaustiveState = u8;

pub const EXHAUSTIVE_STATE_UNKNOWN: ExhaustiveState = 0;
pub const EXHAUSTIVE_STATE_COMPUTING: ExhaustiveState = 1;
pub const EXHAUSTIVE_STATE_FALSE: ExhaustiveState = 2;
pub const EXHAUSTIVE_STATE_TRUE: ExhaustiveState = 3;

#[derive(Default)]
pub(crate) struct SwitchStatementLinks {
    exhaustive_state: ExhaustiveState,
    switch_types_computed: bool,
    witnesses_computed: bool,
    switch_types: Vec<TypeHandle>,
    witnesses: Option<Vec<String>>,
}

pub(crate) struct ArrayLiteralLinks {
    indices_computed: bool,
    first_spread_index: isize,
    last_spread_index: isize,
}

impl Default for ArrayLiteralLinks {
    fn default() -> Self {
        Self {
            indices_computed: false,
            first_spread_index: -1,
            last_spread_index: -1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(usize)]
pub(crate) enum MembersOrExportsResolutionKind {
    ResolvedExports,
    ResolvedMembers,
}

pub(crate) type MembersAndExportsLinks = [Option<SymbolIdentityTable>; 2];

#[derive(Default)]
pub(crate) struct SpreadLinks {
    left_spread: Option<SymbolIdentity>,
    right_spread: Option<SymbolIdentity>,
}

#[derive(Default)]
pub(crate) struct VarianceLinks {
    variances: VarianceCacheState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum VarianceCacheState {
    Uncomputed,
    Computing,
    Computed(Vec<VarianceFlags>),
}

impl Default for VarianceCacheState {
    fn default() -> Self {
        Self::Uncomputed
    }
}

impl VarianceCacheState {
    pub(crate) const fn is_computing(&self) -> bool {
        matches!(self, Self::Computing)
    }

    pub(crate) fn into_variances_or_empty(self) -> Vec<VarianceFlags> {
        match self {
            Self::Computed(variances) => variances,
            Self::Uncomputed | Self::Computing => Vec::new(),
        }
    }
}

pub type VarianceFlags = u32;

pub const VARIANCE_FLAGS_INVARIANT: VarianceFlags = 0;
pub const VARIANCE_FLAGS_COVARIANT: VarianceFlags = 1 << 0;
pub const VARIANCE_FLAGS_CONTRAVARIANT: VarianceFlags = 1 << 1;
pub const VARIANCE_FLAGS_BIVARIANT: VarianceFlags =
    VARIANCE_FLAGS_COVARIANT | VARIANCE_FLAGS_CONTRAVARIANT;
pub const VARIANCE_FLAGS_INDEPENDENT: VarianceFlags = 1 << 2;
pub const VARIANCE_FLAGS_VARIANCE_MASK: VarianceFlags = VARIANCE_FLAGS_INVARIANT
    | VARIANCE_FLAGS_COVARIANT
    | VARIANCE_FLAGS_CONTRAVARIANT
    | VARIANCE_FLAGS_INDEPENDENT;
pub const VARIANCE_FLAGS_UNMEASURABLE: VarianceFlags = 1 << 3;
pub const VARIANCE_FLAGS_UNRELIABLE: VarianceFlags = 1 << 4;
pub const VARIANCE_FLAGS_ALLOWS_STRUCTURAL_FALLBACK: VarianceFlags =
    VARIANCE_FLAGS_UNMEASURABLE | VARIANCE_FLAGS_UNRELIABLE;

pub fn variance_flags_string(v: VarianceFlags) -> String {
    let variance = v & VARIANCE_FLAGS_VARIANCE_MASK;
    let mut result = match variance {
        VARIANCE_FLAGS_INVARIANT => "in out".to_string(),
        VARIANCE_FLAGS_BIVARIANT => "[bivariant]".to_string(),
        VARIANCE_FLAGS_CONTRAVARIANT => "in".to_string(),
        VARIANCE_FLAGS_COVARIANT => "out".to_string(),
        VARIANCE_FLAGS_INDEPENDENT => "[independent]".to_string(),
        _ => String::new(),
    };
    if v & VARIANCE_FLAGS_UNMEASURABLE != 0 {
        result.push_str(" (unmeasurable)");
    } else if v & VARIANCE_FLAGS_UNRELIABLE != 0 {
        result.push_str(" (unreliable)");
    }
    result
}

pub struct VarianceFlagsDisplay(pub VarianceFlags);

impl std::fmt::Display for VarianceFlagsDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&variance_flags_string(self.0))
    }
}

#[derive(Default)]
pub(crate) struct MarkedAssignmentSymbolLinks {
    last_assignment_pos: i32,
    has_definite_assignment: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AccessibleChainCacheKey {
    pub use_only_external_aliasing: bool,
    pub first_relevant_location: Option<ast::Node>,
    pub meaning: ast::SymbolFlags,
}

#[derive(Default)]
pub(crate) struct ContainingSymbolLinks {
    extended_containers_by_file: HashMap<ast::NodeId, Vec<SymbolIdentity>>,
    extended_containers: Option<Vec<SymbolIdentity>>,
    accessible_chain_cache: HashMap<AccessibleChainCacheKey, Vec<SymbolIdentity>>,
}

pub type AccessFlags = u32;

pub const ACCESS_FLAGS_NONE: AccessFlags = 0;
pub const ACCESS_FLAGS_INCLUDE_UNDEFINED: AccessFlags = 1 << 0;
pub const ACCESS_FLAGS_NO_INDEX_SIGNATURES: AccessFlags = 1 << 1;
pub const ACCESS_FLAGS_WRITING: AccessFlags = 1 << 2;
pub const ACCESS_FLAGS_CACHE_SYMBOL: AccessFlags = 1 << 3;
pub const ACCESS_FLAGS_ALLOW_MISSING: AccessFlags = 1 << 4;
pub const ACCESS_FLAGS_EXPRESSION_POSITION: AccessFlags = 1 << 5;
pub const ACCESS_FLAGS_REPORT_DEPRECATED: AccessFlags = 1 << 6;
pub const ACCESS_FLAGS_SUPPRESS_NO_IMPLICIT_ANY_ERROR: AccessFlags = 1 << 7;
pub const ACCESS_FLAGS_CONTEXTUAL: AccessFlags = 1 << 8;
pub const ACCESS_FLAGS_PERSISTENT: AccessFlags = ACCESS_FLAGS_INCLUDE_UNDEFINED;

pub type NodeCheckFlags = u32;

pub const NODE_CHECK_FLAGS_NONE: NodeCheckFlags = 0;
pub const NODE_CHECK_FLAGS_TYPE_CHECKED: NodeCheckFlags = 1 << 0;
pub const NODE_CHECK_FLAGS_CONTEXT_CHECKED: NodeCheckFlags = 1 << 6;
pub const NODE_CHECK_FLAGS_ENUM_VALUES_COMPUTED: NodeCheckFlags = 1 << 10;
pub const NODE_CHECK_FLAGS_ASSIGNMENTS_MARKED: NodeCheckFlags = 1 << 17;
pub const NODE_CHECK_FLAGS_CONTAINS_CLASS_WITH_PRIVATE_IDENTIFIERS: NodeCheckFlags = 1 << 20;
pub const NODE_CHECK_FLAGS_CONTAINS_SUPER_PROPERTY_IN_STATIC_INITIALIZER: NodeCheckFlags = 1 << 21;
pub const NODE_CHECK_FLAGS_IN_CHECK_IDENTIFIER: NodeCheckFlags = 1 << 22;
pub const NODE_CHECK_FLAGS_INITIALIZER_IS_UNDEFINED: NodeCheckFlags = 1 << 24;
pub const NODE_CHECK_FLAGS_INITIALIZER_IS_UNDEFINED_COMPUTED: NodeCheckFlags = 1 << 25;

#[derive(Default)]
pub(crate) struct NodeLinks {
    flags: NodeCheckFlags,
    declaration_requires_scope_change: core::Tristate,
    has_reported_statement_in_ambient_context: bool,
}

#[derive(Default)]
pub(crate) struct SymbolNodeLinks {
    resolved_symbol: Option<SymbolIdentity>,
}

#[derive(Default)]
pub(crate) struct TypeNodeLinks {
    resolved_type: Option<TypeHandle>,
    outer_type_parameters: Option<Arc<[TypeHandle]>>,
}

#[derive(Default)]
pub(crate) struct EnumMemberLinks {
    value: evaluator::Result,
}

#[derive(Default)]
pub(crate) struct AssertionLinks {
    expr_type: Option<TypeHandle>,
}

#[derive(Default)]
pub(crate) struct SourceFileLinks {
    type_checked: bool,
    unused_checked: bool,
    external_helpers_module: Option<SymbolIdentity>,
    requested_external_emit_helpers: ExternalEmitHelpers,
    deferred_nodes: collections::OrderedSet<ast::Node>,
    identifier_check_nodes: Vec<ast::Node>,
    local_jsx_namespace: String,
    local_jsx_fragment_namespace: String,
    local_jsx_factory: Option<ast::EntityName>,
    local_jsx_fragment_factory: Option<ast::EntityName>,
    jsx_fragment_type: Option<TypeHandle>,
}

#[derive(Default)]
pub(crate) struct SignatureLinks {
    resolved_signature: Option<SignatureHandle>,
    effects_signature: Option<SignatureHandle>,
    decorator_signature: Option<SignatureHandle>,
}

pub(crate) type JsxFlags = u32;

#[derive(Default)]
pub(crate) struct JsxElementLinks {
    jsx_flags: JsxFlags, // Flags for the JSX element
    resolved_jsx_element_attributes_type: Option<TypeHandle>, // Resolved element attributes type of a JSX opening-like element
    jsx_namespace: Option<SymbolIdentity>, // Resolved JSX namespace symbol for this node
    jsx_implicit_import_container: Option<SymbolIdentity>, // Resolved module symbol the implicit JSX import of this file should refer to
}

pub(crate) trait FlagBits {
    fn intersects(self, other: Self) -> bool;
    fn is_empty(self) -> bool;
}

impl FlagBits for u32 {
    fn intersects(self, other: Self) -> bool {
        self & other != 0
    }

    fn is_empty(self) -> bool {
        self == 0
    }
}

impl FlagBits for i32 {
    fn intersects(self, other: Self) -> bool {
        self & other != 0
    }

    fn is_empty(self) -> bool {
        self == 0
    }
}

pub const LANGUAGE_FEATURE_MINIMUM_TARGET_EXPONENTIATION: core::ScriptTarget =
    core::ScriptTarget::ES2016;
pub const LANGUAGE_FEATURE_MINIMUM_TARGET_ASYNC_FUNCTIONS: core::ScriptTarget =
    core::ScriptTarget::ES2017;
pub const LANGUAGE_FEATURE_MINIMUM_TARGET_FOR_AWAIT_OF: core::ScriptTarget =
    core::ScriptTarget::ES2018;
pub const LANGUAGE_FEATURE_MINIMUM_TARGET_ASYNC_GENERATORS: core::ScriptTarget =
    core::ScriptTarget::ES2018;
pub const LANGUAGE_FEATURE_MINIMUM_TARGET_ASYNC_ITERATION: core::ScriptTarget =
    core::ScriptTarget::ES2018;
pub const LANGUAGE_FEATURE_MINIMUM_TARGET_OBJECT_SPREAD_REST: core::ScriptTarget =
    core::ScriptTarget::ES2018;
pub const LANGUAGE_FEATURE_MINIMUM_TARGET_REGULAR_EXPRESSION_FLAGS_DOT_ALL: core::ScriptTarget =
    core::ScriptTarget::ES2018;
pub const LANGUAGE_FEATURE_MINIMUM_TARGET_BINDINGLESS_CATCH: core::ScriptTarget =
    core::ScriptTarget::ES2019;
pub const LANGUAGE_FEATURE_MINIMUM_TARGET_BIG_INT: core::ScriptTarget = core::ScriptTarget::ES2020;
pub const LANGUAGE_FEATURE_MINIMUM_TARGET_NULLISH_COALESCE: core::ScriptTarget =
    core::ScriptTarget::ES2020;
pub const LANGUAGE_FEATURE_MINIMUM_TARGET_OPTIONAL_CHAINING: core::ScriptTarget =
    core::ScriptTarget::ES2020;
pub const LANGUAGE_FEATURE_MINIMUM_TARGET_LOGICAL_ASSIGNMENT: core::ScriptTarget =
    core::ScriptTarget::ES2021;
pub const LANGUAGE_FEATURE_MINIMUM_TARGET_TOP_LEVEL_AWAIT: core::ScriptTarget =
    core::ScriptTarget::ES2022;
pub const LANGUAGE_FEATURE_MINIMUM_TARGET_CLASS_FIELDS: core::ScriptTarget =
    core::ScriptTarget::ES2022;
pub const LANGUAGE_FEATURE_MINIMUM_TARGET_PRIVATE_NAMES_AND_CLASS_STATIC_BLOCKS:
    core::ScriptTarget = core::ScriptTarget::ES2022;
pub const LANGUAGE_FEATURE_MINIMUM_TARGET_REGULAR_EXPRESSION_FLAGS_HAS_INDICES: core::ScriptTarget =
    core::ScriptTarget::ES2022;
pub const LANGUAGE_FEATURE_MINIMUM_TARGET_SHEBANG_COMMENTS: core::ScriptTarget =
    core::ScriptTarget::ESNext;
pub const LANGUAGE_FEATURE_MINIMUM_TARGET_USING_AND_AWAIT_USING: core::ScriptTarget =
    core::ScriptTarget::ESNext;
pub const LANGUAGE_FEATURE_MINIMUM_TARGET_CLASS_AND_CLASS_ELEMENT_DECORATORS: core::ScriptTarget =
    core::ScriptTarget::ESNext;
pub const LANGUAGE_FEATURE_MINIMUM_TARGET_REGULAR_EXPRESSION_FLAGS_UNICODE_SETS:
    core::ScriptTarget = core::ScriptTarget::ESNext;

pub type PredicateSemantics = u32;
pub const PREDICATE_SEMANTICS_NONE: PredicateSemantics = 0;
pub const PREDICATE_SEMANTICS_ALWAYS: PredicateSemantics = 1 << 0;
pub const PREDICATE_SEMANTICS_NEVER: PredicateSemantics = 1 << 1;
pub const PREDICATE_SEMANTICS_SOMETIMES: PredicateSemantics =
    PREDICATE_SEMANTICS_ALWAYS | PREDICATE_SEMANTICS_NEVER;

pub type IntersectionFlags = u32;
pub const INTERSECTION_FLAGS_NONE: IntersectionFlags = 0;
pub const INTERSECTION_FLAGS_NO_SUPERTYPE_REDUCTION: IntersectionFlags = 1 << 0;
pub const INTERSECTION_FLAGS_NO_CONSTRAINT_REDUCTION: IntersectionFlags = 1 << 1;

pub const TypeFlagsUniqueESSymbol: TypeFlags = TYPE_FLAGS_UNIQUE_ES_SYMBOL;

pub type CheckMode = u32;

pub const CHECK_MODE_NORMAL: CheckMode = 0;
pub const CHECK_MODE_CONTEXTUAL: CheckMode = 1 << 0;
pub const CHECK_MODE_INFERENTIAL: CheckMode = 1 << 1;
pub const CHECK_MODE_SKIP_CONTEXT_SENSITIVE: CheckMode = 1 << 2;
pub const CHECK_MODE_SKIP_GENERIC_FUNCTIONS: CheckMode = 1 << 3;
pub const CHECK_MODE_IS_FOR_SIGNATURE_HELP: CheckMode = 1 << 4;
pub const CHECK_MODE_REST_BINDING_ELEMENT: CheckMode = 1 << 5;
pub const CHECK_MODE_TYPE_ONLY: CheckMode = 1 << 6;
pub const CHECK_MODE_FORCE_TUPLE: CheckMode = 1 << 7;

pub type WideningKind = i32;

pub const WIDENING_KIND_NORMAL: WideningKind = 0;
pub const WIDENING_KIND_FUNCTION_RETURN: WideningKind = 1;
pub const WIDENING_KIND_GENERATOR_NEXT: WideningKind = 2;
pub const WIDENING_KIND_GENERATOR_YIELD: WideningKind = 3;

pub type DeclarationMeaning = u32;

pub const DECLARATION_MEANING_GET_ACCESSOR: DeclarationMeaning = 1 << 0;
pub const DECLARATION_MEANING_SET_ACCESSOR: DeclarationMeaning = 1 << 1;
pub const DECLARATION_MEANING_PROPERTY_ASSIGNMENT: DeclarationMeaning = 1 << 2;
pub const DECLARATION_MEANING_METHOD: DeclarationMeaning = 1 << 3;
pub const DECLARATION_MEANING_PRIVATE_STATIC: DeclarationMeaning = 1 << 4;
pub const DECLARATION_MEANING_GET_OR_SET_ACCESSOR: DeclarationMeaning =
    DECLARATION_MEANING_GET_ACCESSOR | DECLARATION_MEANING_SET_ACCESSOR;
pub const DECLARATION_MEANING_PROPERTY_ASSIGNMENT_OR_METHOD: DeclarationMeaning =
    DECLARATION_MEANING_PROPERTY_ASSIGNMENT | DECLARATION_MEANING_METHOD;

pub type DeclarationSpaces = i32;

pub const DECLARATION_SPACES_NONE: DeclarationSpaces = 0;
pub const DECLARATION_SPACES_EXPORT_VALUE: DeclarationSpaces = 1 << 0;
pub const DECLARATION_SPACES_EXPORT_TYPE: DeclarationSpaces = 1 << 1;
pub const DECLARATION_SPACES_EXPORT_NAMESPACE: DeclarationSpaces = 1 << 2;

pub type IntrinsicTypeKind = i32;

pub const INTRINSIC_TYPE_KIND_UNKNOWN: IntrinsicTypeKind = 0;
pub const INTRINSIC_TYPE_KIND_UPPERCASE: IntrinsicTypeKind = 1;
pub const INTRINSIC_TYPE_KIND_LOWERCASE: IntrinsicTypeKind = 2;
pub const INTRINSIC_TYPE_KIND_CAPITALIZE: IntrinsicTypeKind = 3;
pub const INTRINSIC_TYPE_KIND_UNCAPITALIZE: IntrinsicTypeKind = 4;
pub const INTRINSIC_TYPE_KIND_NO_INFER: IntrinsicTypeKind = 5;

pub fn intrinsic_type_kinds() -> HashMap<&'static str, IntrinsicTypeKind> {
    HashMap::from([
        ("Uppercase", INTRINSIC_TYPE_KIND_UPPERCASE),
        ("Lowercase", INTRINSIC_TYPE_KIND_LOWERCASE),
        ("Capitalize", INTRINSIC_TYPE_KIND_CAPITALIZE),
        ("Uncapitalize", INTRINSIC_TYPE_KIND_UNCAPITALIZE),
        ("NoInfer", INTRINSIC_TYPE_KIND_NO_INFER),
    ])
}

pub type MappedTypeModifiers = u32;

pub const MAPPED_TYPE_MODIFIERS_INCLUDE_READONLY: MappedTypeModifiers = 1 << 0;
pub const MAPPED_TYPE_MODIFIERS_EXCLUDE_READONLY: MappedTypeModifiers = 1 << 1;
pub const MAPPED_TYPE_MODIFIERS_INCLUDE_OPTIONAL: MappedTypeModifiers = 1 << 2;
pub const MAPPED_TYPE_MODIFIERS_EXCLUDE_OPTIONAL: MappedTypeModifiers = 1 << 3;
pub const MAPPED_TYPE_MODIFIERS_NONE: MappedTypeModifiers = 0;

pub type MappedTypeNameTypeKind = i32;

pub const MAPPED_TYPE_NAME_TYPE_KIND_NONE: MappedTypeNameTypeKind = 0;
pub const MAPPED_TYPE_NAME_TYPE_KIND_FILTERING: MappedTypeNameTypeKind = 1;
pub const MAPPED_TYPE_NAME_TYPE_KIND_REMAPPING: MappedTypeNameTypeKind = 2;

pub type ReferenceHint = i32;

pub const REFERENCE_HINT_UNSPECIFIED: ReferenceHint = 0;
pub const REFERENCE_HINT_IDENTIFIER: ReferenceHint = 1;
pub const REFERENCE_HINT_PROPERTY: ReferenceHint = 2;
pub const REFERENCE_HINT_EXPORT_ASSIGNMENT: ReferenceHint = 3;
pub const REFERENCE_HINT_JSX: ReferenceHint = 4;
pub const REFERENCE_HINT_EXPORT_IMPORT_EQUALS: ReferenceHint = 5;
pub const REFERENCE_HINT_EXPORT_SPECIFIER: ReferenceHint = 6;
pub const REFERENCE_HINT_DECORATOR: ReferenceHint = 7;

pub type UnusedKind = i32;
pub const UNUSED_KIND_LOCAL: UnusedKind = 0;
pub const UNUSED_KIND_PARAMETER: UnusedKind = 1;

pub type UnionReduction = i32;
pub const UNION_REDUCTION_NONE: UnionReduction = 0;
pub const UNION_REDUCTION_LITERAL: UnionReduction = 1;
pub const UNION_REDUCTION_SUBTYPE: UnionReduction = 2;

pub type ThisAssignmentDeclarationKind = i32;
pub const THIS_ASSIGNMENT_DECLARATION_NONE: ThisAssignmentDeclarationKind = 0;
pub const THIS_ASSIGNMENT_DECLARATION_TYPED: ThisAssignmentDeclarationKind = 1;
pub const THIS_ASSIGNMENT_DECLARATION_CONSTRUCTOR: ThisAssignmentDeclarationKind = 2;
pub const THIS_ASSIGNMENT_DECLARATION_METHOD: ThisAssignmentDeclarationKind = 3;

pub type TypeFacts = u32;

pub const TYPE_FACTS_NONE: TypeFacts = 0;
pub const TYPE_FACTS_TYPEOF_EQ_STRING: TypeFacts = 1 << 0;
pub const TYPE_FACTS_TYPEOF_EQ_NUMBER: TypeFacts = 1 << 1;
pub const TYPE_FACTS_TYPEOF_EQ_BIG_INT: TypeFacts = 1 << 2;
pub const TYPE_FACTS_TYPEOF_EQ_BOOLEAN: TypeFacts = 1 << 3;
pub const TYPE_FACTS_TYPEOF_EQ_SYMBOL: TypeFacts = 1 << 4;
pub const TYPE_FACTS_TYPEOF_EQ_OBJECT: TypeFacts = 1 << 5;
pub const TYPE_FACTS_TYPEOF_EQ_FUNCTION: TypeFacts = 1 << 6;
pub const TYPE_FACTS_TYPEOF_EQ_HOST_OBJECT: TypeFacts = 1 << 7;
pub const TYPE_FACTS_TYPEOF_NE_STRING: TypeFacts = 1 << 8;
pub const TYPE_FACTS_TYPEOF_NE_NUMBER: TypeFacts = 1 << 9;
pub const TYPE_FACTS_TYPEOF_NE_BIG_INT: TypeFacts = 1 << 10;
pub const TYPE_FACTS_TYPEOF_NE_BOOLEAN: TypeFacts = 1 << 11;
pub const TYPE_FACTS_TYPEOF_NE_SYMBOL: TypeFacts = 1 << 12;
pub const TYPE_FACTS_TYPEOF_NE_OBJECT: TypeFacts = 1 << 13;
pub const TYPE_FACTS_TYPEOF_NE_FUNCTION: TypeFacts = 1 << 14;
pub const TYPE_FACTS_TYPEOF_NE_HOST_OBJECT: TypeFacts = 1 << 15;
pub const TYPE_FACTS_EQ_UNDEFINED: TypeFacts = 1 << 16;
pub const TYPE_FACTS_EQ_NULL: TypeFacts = 1 << 17;
pub const TYPE_FACTS_EQ_UNDEFINED_OR_NULL: TypeFacts = 1 << 18;
pub const TYPE_FACTS_NE_UNDEFINED: TypeFacts = 1 << 19;
pub const TYPE_FACTS_NE_NULL: TypeFacts = 1 << 20;
pub const TYPE_FACTS_NE_UNDEFINED_OR_NULL: TypeFacts = 1 << 21;
pub const TYPE_FACTS_TRUTHY: TypeFacts = 1 << 22;
pub const TYPE_FACTS_FALSY: TypeFacts = 1 << 23;
pub const TYPE_FACTS_IS_UNDEFINED: TypeFacts = 1 << 24;
pub const TYPE_FACTS_IS_NULL: TypeFacts = 1 << 25;
pub const TYPE_FACTS_IS_UNDEFINED_OR_NULL: TypeFacts = TYPE_FACTS_IS_UNDEFINED | TYPE_FACTS_IS_NULL;
pub const TYPE_FACTS_ALL: TypeFacts = (1 << 27) - 1;

pub const TYPE_FACTS_BASE_STRING_STRICT_FACTS: TypeFacts = TYPE_FACTS_TYPEOF_EQ_STRING
    | TYPE_FACTS_TYPEOF_NE_NUMBER
    | TYPE_FACTS_TYPEOF_NE_BIG_INT
    | TYPE_FACTS_TYPEOF_NE_BOOLEAN
    | TYPE_FACTS_TYPEOF_NE_SYMBOL
    | TYPE_FACTS_TYPEOF_NE_OBJECT
    | TYPE_FACTS_TYPEOF_NE_FUNCTION
    | TYPE_FACTS_TYPEOF_NE_HOST_OBJECT
    | TYPE_FACTS_NE_UNDEFINED
    | TYPE_FACTS_NE_NULL
    | TYPE_FACTS_NE_UNDEFINED_OR_NULL;
pub const TYPE_FACTS_BASE_STRING_FACTS: TypeFacts = TYPE_FACTS_BASE_STRING_STRICT_FACTS
    | TYPE_FACTS_EQ_UNDEFINED
    | TYPE_FACTS_EQ_NULL
    | TYPE_FACTS_EQ_UNDEFINED_OR_NULL
    | TYPE_FACTS_FALSY;
pub const TYPE_FACTS_STRING_STRICT_FACTS: TypeFacts =
    TYPE_FACTS_BASE_STRING_STRICT_FACTS | TYPE_FACTS_TRUTHY | TYPE_FACTS_FALSY;
pub const TYPE_FACTS_STRING_FACTS: TypeFacts = TYPE_FACTS_BASE_STRING_FACTS | TYPE_FACTS_TRUTHY;
pub const TYPE_FACTS_EMPTY_STRING_STRICT_FACTS: TypeFacts =
    TYPE_FACTS_BASE_STRING_STRICT_FACTS | TYPE_FACTS_FALSY;
pub const TYPE_FACTS_EMPTY_STRING_FACTS: TypeFacts = TYPE_FACTS_BASE_STRING_FACTS;
pub const TYPE_FACTS_NON_EMPTY_STRING_STRICT_FACTS: TypeFacts =
    TYPE_FACTS_BASE_STRING_STRICT_FACTS | TYPE_FACTS_TRUTHY;
pub const TYPE_FACTS_NON_EMPTY_STRING_FACTS: TypeFacts =
    TYPE_FACTS_BASE_STRING_FACTS | TYPE_FACTS_TRUTHY;
pub const TYPE_FACTS_BASE_NUMBER_STRICT_FACTS: TypeFacts = TYPE_FACTS_TYPEOF_EQ_NUMBER
    | TYPE_FACTS_TYPEOF_NE_STRING
    | TYPE_FACTS_TYPEOF_NE_BIG_INT
    | TYPE_FACTS_TYPEOF_NE_BOOLEAN
    | TYPE_FACTS_TYPEOF_NE_SYMBOL
    | TYPE_FACTS_TYPEOF_NE_OBJECT
    | TYPE_FACTS_TYPEOF_NE_FUNCTION
    | TYPE_FACTS_TYPEOF_NE_HOST_OBJECT
    | TYPE_FACTS_NE_UNDEFINED
    | TYPE_FACTS_NE_NULL
    | TYPE_FACTS_NE_UNDEFINED_OR_NULL;
pub const TYPE_FACTS_BASE_NUMBER_FACTS: TypeFacts = TYPE_FACTS_BASE_NUMBER_STRICT_FACTS
    | TYPE_FACTS_EQ_UNDEFINED
    | TYPE_FACTS_EQ_NULL
    | TYPE_FACTS_EQ_UNDEFINED_OR_NULL
    | TYPE_FACTS_FALSY;
pub const TYPE_FACTS_NUMBER_STRICT_FACTS: TypeFacts =
    TYPE_FACTS_BASE_NUMBER_STRICT_FACTS | TYPE_FACTS_TRUTHY | TYPE_FACTS_FALSY;
pub const TYPE_FACTS_NUMBER_FACTS: TypeFacts = TYPE_FACTS_BASE_NUMBER_FACTS | TYPE_FACTS_TRUTHY;
pub const TYPE_FACTS_ZERO_NUMBER_STRICT_FACTS: TypeFacts =
    TYPE_FACTS_BASE_NUMBER_STRICT_FACTS | TYPE_FACTS_FALSY;
pub const TYPE_FACTS_ZERO_NUMBER_FACTS: TypeFacts = TYPE_FACTS_BASE_NUMBER_FACTS;
pub const TYPE_FACTS_NON_ZERO_NUMBER_STRICT_FACTS: TypeFacts =
    TYPE_FACTS_BASE_NUMBER_STRICT_FACTS | TYPE_FACTS_TRUTHY;
pub const TYPE_FACTS_NON_ZERO_NUMBER_FACTS: TypeFacts =
    TYPE_FACTS_BASE_NUMBER_FACTS | TYPE_FACTS_TRUTHY;
pub const TYPE_FACTS_BASE_BIG_INT_STRICT_FACTS: TypeFacts = TYPE_FACTS_TYPEOF_EQ_BIG_INT
    | TYPE_FACTS_TYPEOF_NE_STRING
    | TYPE_FACTS_TYPEOF_NE_NUMBER
    | TYPE_FACTS_TYPEOF_NE_BOOLEAN
    | TYPE_FACTS_TYPEOF_NE_SYMBOL
    | TYPE_FACTS_TYPEOF_NE_OBJECT
    | TYPE_FACTS_TYPEOF_NE_FUNCTION
    | TYPE_FACTS_TYPEOF_NE_HOST_OBJECT
    | TYPE_FACTS_NE_UNDEFINED
    | TYPE_FACTS_NE_NULL
    | TYPE_FACTS_NE_UNDEFINED_OR_NULL;
pub const TYPE_FACTS_BASE_BIG_INT_FACTS: TypeFacts = TYPE_FACTS_BASE_BIG_INT_STRICT_FACTS
    | TYPE_FACTS_EQ_UNDEFINED
    | TYPE_FACTS_EQ_NULL
    | TYPE_FACTS_EQ_UNDEFINED_OR_NULL
    | TYPE_FACTS_FALSY;
pub const TYPE_FACTS_BIG_INT_STRICT_FACTS: TypeFacts =
    TYPE_FACTS_BASE_BIG_INT_STRICT_FACTS | TYPE_FACTS_TRUTHY | TYPE_FACTS_FALSY;
pub const TYPE_FACTS_BIG_INT_FACTS: TypeFacts = TYPE_FACTS_BASE_BIG_INT_FACTS | TYPE_FACTS_TRUTHY;
pub const TYPE_FACTS_ZERO_BIG_INT_STRICT_FACTS: TypeFacts =
    TYPE_FACTS_BASE_BIG_INT_STRICT_FACTS | TYPE_FACTS_FALSY;
pub const TYPE_FACTS_ZERO_BIG_INT_FACTS: TypeFacts = TYPE_FACTS_BASE_BIG_INT_FACTS;
pub const TYPE_FACTS_NON_ZERO_BIG_INT_STRICT_FACTS: TypeFacts =
    TYPE_FACTS_BASE_BIG_INT_STRICT_FACTS | TYPE_FACTS_TRUTHY;
pub const TYPE_FACTS_NON_ZERO_BIG_INT_FACTS: TypeFacts =
    TYPE_FACTS_BASE_BIG_INT_FACTS | TYPE_FACTS_TRUTHY;
pub const TYPE_FACTS_BASE_BOOLEAN_STRICT_FACTS: TypeFacts = TYPE_FACTS_TYPEOF_EQ_BOOLEAN
    | TYPE_FACTS_TYPEOF_NE_STRING
    | TYPE_FACTS_TYPEOF_NE_NUMBER
    | TYPE_FACTS_TYPEOF_NE_BIG_INT
    | TYPE_FACTS_TYPEOF_NE_SYMBOL
    | TYPE_FACTS_TYPEOF_NE_OBJECT
    | TYPE_FACTS_TYPEOF_NE_FUNCTION
    | TYPE_FACTS_TYPEOF_NE_HOST_OBJECT
    | TYPE_FACTS_NE_UNDEFINED
    | TYPE_FACTS_NE_NULL
    | TYPE_FACTS_NE_UNDEFINED_OR_NULL;
pub const TYPE_FACTS_BASE_BOOLEAN_FACTS: TypeFacts = TYPE_FACTS_BASE_BOOLEAN_STRICT_FACTS
    | TYPE_FACTS_EQ_UNDEFINED
    | TYPE_FACTS_EQ_NULL
    | TYPE_FACTS_EQ_UNDEFINED_OR_NULL
    | TYPE_FACTS_FALSY;
pub const TYPE_FACTS_BOOLEAN_STRICT_FACTS: TypeFacts =
    TYPE_FACTS_BASE_BOOLEAN_STRICT_FACTS | TYPE_FACTS_TRUTHY | TYPE_FACTS_FALSY;
pub const TYPE_FACTS_BOOLEAN_FACTS: TypeFacts = TYPE_FACTS_BASE_BOOLEAN_FACTS | TYPE_FACTS_TRUTHY;
pub const TYPE_FACTS_FALSE_STRICT_FACTS: TypeFacts =
    TYPE_FACTS_BASE_BOOLEAN_STRICT_FACTS | TYPE_FACTS_FALSY;
pub const TYPE_FACTS_FALSE_FACTS: TypeFacts = TYPE_FACTS_BASE_BOOLEAN_FACTS;
pub const TYPE_FACTS_TRUE_STRICT_FACTS: TypeFacts =
    TYPE_FACTS_BASE_BOOLEAN_STRICT_FACTS | TYPE_FACTS_TRUTHY;
pub const TYPE_FACTS_TRUE_FACTS: TypeFacts = TYPE_FACTS_BASE_BOOLEAN_FACTS | TYPE_FACTS_TRUTHY;
pub const TYPE_FACTS_SYMBOL_STRICT_FACTS: TypeFacts = TYPE_FACTS_TYPEOF_EQ_SYMBOL
    | TYPE_FACTS_TYPEOF_NE_STRING
    | TYPE_FACTS_TYPEOF_NE_NUMBER
    | TYPE_FACTS_TYPEOF_NE_BIG_INT
    | TYPE_FACTS_TYPEOF_NE_BOOLEAN
    | TYPE_FACTS_TYPEOF_NE_OBJECT
    | TYPE_FACTS_TYPEOF_NE_FUNCTION
    | TYPE_FACTS_TYPEOF_NE_HOST_OBJECT
    | TYPE_FACTS_NE_UNDEFINED
    | TYPE_FACTS_NE_NULL
    | TYPE_FACTS_NE_UNDEFINED_OR_NULL
    | TYPE_FACTS_TRUTHY;
pub const TYPE_FACTS_SYMBOL_FACTS: TypeFacts = TYPE_FACTS_SYMBOL_STRICT_FACTS
    | TYPE_FACTS_EQ_UNDEFINED
    | TYPE_FACTS_EQ_NULL
    | TYPE_FACTS_EQ_UNDEFINED_OR_NULL
    | TYPE_FACTS_FALSY;
pub const TYPE_FACTS_OBJECT_STRICT_FACTS: TypeFacts = TYPE_FACTS_TYPEOF_EQ_OBJECT
    | TYPE_FACTS_TYPEOF_EQ_HOST_OBJECT
    | TYPE_FACTS_TYPEOF_NE_STRING
    | TYPE_FACTS_TYPEOF_NE_NUMBER
    | TYPE_FACTS_TYPEOF_NE_BIG_INT
    | TYPE_FACTS_TYPEOF_NE_BOOLEAN
    | TYPE_FACTS_TYPEOF_NE_SYMBOL
    | TYPE_FACTS_TYPEOF_NE_FUNCTION
    | TYPE_FACTS_NE_UNDEFINED
    | TYPE_FACTS_NE_NULL
    | TYPE_FACTS_NE_UNDEFINED_OR_NULL
    | TYPE_FACTS_TRUTHY;
pub const TYPE_FACTS_OBJECT_FACTS: TypeFacts = TYPE_FACTS_OBJECT_STRICT_FACTS
    | TYPE_FACTS_EQ_UNDEFINED
    | TYPE_FACTS_EQ_NULL
    | TYPE_FACTS_EQ_UNDEFINED_OR_NULL
    | TYPE_FACTS_FALSY;
pub const TYPE_FACTS_FUNCTION_STRICT_FACTS: TypeFacts = TYPE_FACTS_TYPEOF_EQ_FUNCTION
    | TYPE_FACTS_TYPEOF_EQ_HOST_OBJECT
    | TYPE_FACTS_TYPEOF_NE_STRING
    | TYPE_FACTS_TYPEOF_NE_NUMBER
    | TYPE_FACTS_TYPEOF_NE_BIG_INT
    | TYPE_FACTS_TYPEOF_NE_BOOLEAN
    | TYPE_FACTS_TYPEOF_NE_SYMBOL
    | TYPE_FACTS_TYPEOF_NE_OBJECT
    | TYPE_FACTS_NE_UNDEFINED
    | TYPE_FACTS_NE_NULL
    | TYPE_FACTS_NE_UNDEFINED_OR_NULL
    | TYPE_FACTS_TRUTHY;
pub const TYPE_FACTS_FUNCTION_FACTS: TypeFacts = TYPE_FACTS_FUNCTION_STRICT_FACTS
    | TYPE_FACTS_EQ_UNDEFINED
    | TYPE_FACTS_EQ_NULL
    | TYPE_FACTS_EQ_UNDEFINED_OR_NULL
    | TYPE_FACTS_FALSY;
pub const TYPE_FACTS_VOID_FACTS: TypeFacts = TYPE_FACTS_TYPEOF_NE_STRING
    | TYPE_FACTS_TYPEOF_NE_NUMBER
    | TYPE_FACTS_TYPEOF_NE_BIG_INT
    | TYPE_FACTS_TYPEOF_NE_BOOLEAN
    | TYPE_FACTS_TYPEOF_NE_SYMBOL
    | TYPE_FACTS_TYPEOF_NE_OBJECT
    | TYPE_FACTS_TYPEOF_NE_FUNCTION
    | TYPE_FACTS_TYPEOF_NE_HOST_OBJECT
    | TYPE_FACTS_EQ_UNDEFINED
    | TYPE_FACTS_EQ_UNDEFINED_OR_NULL
    | TYPE_FACTS_NE_NULL
    | TYPE_FACTS_FALSY;
pub const TYPE_FACTS_UNDEFINED_FACTS: TypeFacts = TYPE_FACTS_TYPEOF_NE_STRING
    | TYPE_FACTS_TYPEOF_NE_NUMBER
    | TYPE_FACTS_TYPEOF_NE_BIG_INT
    | TYPE_FACTS_TYPEOF_NE_BOOLEAN
    | TYPE_FACTS_TYPEOF_NE_SYMBOL
    | TYPE_FACTS_TYPEOF_NE_OBJECT
    | TYPE_FACTS_TYPEOF_NE_FUNCTION
    | TYPE_FACTS_TYPEOF_NE_HOST_OBJECT
    | TYPE_FACTS_EQ_UNDEFINED
    | TYPE_FACTS_EQ_UNDEFINED_OR_NULL
    | TYPE_FACTS_NE_NULL
    | TYPE_FACTS_FALSY
    | TYPE_FACTS_IS_UNDEFINED;
pub const TYPE_FACTS_NULL_FACTS: TypeFacts = TYPE_FACTS_TYPEOF_EQ_OBJECT
    | TYPE_FACTS_TYPEOF_NE_STRING
    | TYPE_FACTS_TYPEOF_NE_NUMBER
    | TYPE_FACTS_TYPEOF_NE_BIG_INT
    | TYPE_FACTS_TYPEOF_NE_BOOLEAN
    | TYPE_FACTS_TYPEOF_NE_SYMBOL
    | TYPE_FACTS_TYPEOF_NE_FUNCTION
    | TYPE_FACTS_TYPEOF_NE_HOST_OBJECT
    | TYPE_FACTS_EQ_NULL
    | TYPE_FACTS_EQ_UNDEFINED_OR_NULL
    | TYPE_FACTS_NE_UNDEFINED
    | TYPE_FACTS_FALSY
    | TYPE_FACTS_IS_NULL;
pub const TYPE_FACTS_EMPTY_OBJECT_STRICT_FACTS: TypeFacts = TYPE_FACTS_ALL
    & !(TYPE_FACTS_EQ_UNDEFINED
        | TYPE_FACTS_EQ_NULL
        | TYPE_FACTS_EQ_UNDEFINED_OR_NULL
        | TYPE_FACTS_IS_UNDEFINED_OR_NULL);
pub const TYPE_FACTS_EMPTY_OBJECT_FACTS: TypeFacts =
    TYPE_FACTS_ALL & !TYPE_FACTS_IS_UNDEFINED_OR_NULL;
pub const TYPE_FACTS_UNKNOWN_FACTS: TypeFacts = TYPE_FACTS_ALL & !TYPE_FACTS_IS_UNDEFINED_OR_NULL;
pub const TYPE_FACTS_ALL_TYPEOF_NE: TypeFacts = TYPE_FACTS_TYPEOF_NE_STRING
    | TYPE_FACTS_TYPEOF_NE_NUMBER
    | TYPE_FACTS_TYPEOF_NE_BIG_INT
    | TYPE_FACTS_TYPEOF_NE_BOOLEAN
    | TYPE_FACTS_TYPEOF_NE_SYMBOL
    | TYPE_FACTS_TYPEOF_NE_OBJECT
    | TYPE_FACTS_TYPEOF_NE_FUNCTION
    | TYPE_FACTS_NE_UNDEFINED;
pub const TYPE_FACTS_OR_FACTS_MASK: TypeFacts =
    TYPE_FACTS_TYPEOF_EQ_FUNCTION | TYPE_FACTS_TYPEOF_NE_OBJECT;
pub const TYPE_FACTS_AND_FACTS_MASK: TypeFacts = TYPE_FACTS_ALL & !TYPE_FACTS_OR_FACTS_MASK;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct EnumRelationKey {
    pub(crate) source: SymbolIdentity,
    pub(crate) target: SymbolIdentity,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum LiteralValue {
    None,
    String(String),
    Number(Number),
    Bool(bool),
    BigInt(PseudoBigInt),
    PseudoBigInt(PseudoBigInt),
    Symbol(SymbolIdentity),
    Node(ast::Node),
    Type(TypeHandle),
    Signature(SignatureHandle),
}

impl Default for LiteralValue {
    fn default() -> Self {
        Self::None
    }
}

impl LiteralValue {
    pub(crate) fn as_string(&self) -> &str {
        match self {
            Self::String(value) => value,
            _ => panic!("LiteralValue is not a string"),
        }
    }

    pub(crate) fn as_number(&self) -> Number {
        match self {
            Self::Number(value) => *value,
            _ => panic!("LiteralValue is not a number"),
        }
    }

    pub(crate) fn as_big_int(&self) -> PseudoBigInt {
        match self {
            Self::BigInt(value) | Self::PseudoBigInt(value) => value.clone(),
            _ => panic!("LiteralValue is not a big int"),
        }
    }

    pub(crate) fn as_bool(&self) -> bool {
        match self {
            Self::Bool(value) => *value,
            _ => panic!("LiteralValue is not a bool"),
        }
    }
}

impl From<String> for LiteralValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<ast::SymbolName> for LiteralValue {
    fn from(value: ast::SymbolName) -> Self {
        Self::String(value.to_string())
    }
}

impl From<&str> for LiteralValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<usize> for LiteralValue {
    fn from(value: usize) -> Self {
        Self::String(value.to_string())
    }
}

impl From<u64> for LiteralValue {
    fn from(value: u64) -> Self {
        Self::String(value.to_string())
    }
}

impl From<isize> for LiteralValue {
    fn from(value: isize) -> Self {
        Self::String(value.to_string())
    }
}

impl From<evaluator::Value> for LiteralValue {
    fn from(value: evaluator::Value) -> Self {
        match value {
            evaluator::Value::None => Self::None,
            evaluator::Value::String(value) => Self::String(value),
            evaluator::Value::Number(value) => Self::Number(value),
            evaluator::Value::Bool(value) => Self::Bool(value),
            evaluator::Value::PseudoBigInt(value) => Self::PseudoBigInt(value),
        }
    }
}

impl std::fmt::Display for LiteralValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => Ok(()),
            Self::String(value) => f.write_str(value),
            Self::Number(value) => write!(f, "{value}"),
            Self::Bool(value) => write!(f, "{value}"),
            Self::BigInt(value) | Self::PseudoBigInt(value) => write!(f, "{value}"),
            Self::Symbol(_) => f.write_str("[symbol]"),
            Self::Node(_) => f.write_str("[node]"),
            Self::Type(_) => f.write_str("[type]"),
            Self::Signature(_) => f.write_str("[signature]"),
        }
    }
}

pub(crate) fn literal_value_to_evaluator_value(value: &LiteralValue) -> evaluator::Value {
    match value {
        LiteralValue::None => evaluator::Value::None,
        LiteralValue::String(value) => evaluator::Value::String(value.clone()),
        LiteralValue::Number(value) => evaluator::Value::Number(*value),
        LiteralValue::Bool(value) => evaluator::Value::Bool(*value),
        LiteralValue::BigInt(value) | LiteralValue::PseudoBigInt(value) => {
            evaluator::Value::PseudoBigInt(value.clone())
        }
        LiteralValue::Symbol(_)
        | LiteralValue::Node(_)
        | LiteralValue::Type(_)
        | LiteralValue::Signature(_) => evaluator::Value::None,
    }
}

pub type ConstantValue = LiteralValue;
pub type Any = LiteralValue;
pub(crate) type DiagnosticArg = Any;

pub(crate) trait IntoDiagnosticArgs {
    fn into_diagnostic_args(self) -> Vec<DiagnosticArg>;
}

impl IntoDiagnosticArgs for &[DiagnosticArg] {
    fn into_diagnostic_args(self) -> Vec<DiagnosticArg> {
        self.to_vec()
    }
}

impl IntoDiagnosticArgs for () {
    fn into_diagnostic_args(self) -> Vec<DiagnosticArg> {
        Vec::new()
    }
}

impl IntoDiagnosticArgs for DiagnosticArg {
    fn into_diagnostic_args(self) -> Vec<DiagnosticArg> {
        vec![self]
    }
}

impl IntoDiagnosticArgs for String {
    fn into_diagnostic_args(self) -> Vec<DiagnosticArg> {
        vec![DiagnosticArg::from(self)]
    }
}

impl IntoDiagnosticArgs for &str {
    fn into_diagnostic_args(self) -> Vec<DiagnosticArg> {
        vec![DiagnosticArg::from(self)]
    }
}

impl IntoDiagnosticArgs for usize {
    fn into_diagnostic_args(self) -> Vec<DiagnosticArg> {
        vec![DiagnosticArg::from(self)]
    }
}

impl IntoDiagnosticArgs for isize {
    fn into_diagnostic_args(self) -> Vec<DiagnosticArg> {
        vec![DiagnosticArg::from(self)]
    }
}

impl IntoDiagnosticArgs for u64 {
    fn into_diagnostic_args(self) -> Vec<DiagnosticArg> {
        vec![DiagnosticArg::from(self)]
    }
}

impl<const N: usize> IntoDiagnosticArgs for &[DiagnosticArg; N] {
    fn into_diagnostic_args(self) -> Vec<DiagnosticArg> {
        self.to_vec()
    }
}

impl IntoDiagnosticArgs for &[String] {
    fn into_diagnostic_args(self) -> Vec<DiagnosticArg> {
        self.iter()
            .map(|value| DiagnosticArg::from(value.as_str()))
            .collect()
    }
}

impl<const N: usize> IntoDiagnosticArgs for &[String; N] {
    fn into_diagnostic_args(self) -> Vec<DiagnosticArg> {
        self.iter()
            .map(|value| DiagnosticArg::from(value.as_str()))
            .collect()
    }
}

impl IntoDiagnosticArgs for &[&str] {
    fn into_diagnostic_args(self) -> Vec<DiagnosticArg> {
        self.iter().copied().map(DiagnosticArg::from).collect()
    }
}

impl<const N: usize> IntoDiagnosticArgs for &[&str; N] {
    fn into_diagnostic_args(self) -> Vec<DiagnosticArg> {
        self.iter().copied().map(DiagnosticArg::from).collect()
    }
}

impl IntoDiagnosticArgs for Vec<DiagnosticArg> {
    fn into_diagnostic_args(self) -> Vec<DiagnosticArg> {
        self
    }
}

impl IntoDiagnosticArgs for Vec<String> {
    fn into_diagnostic_args(self) -> Vec<DiagnosticArg> {
        self.into_iter().map(DiagnosticArg::from).collect()
    }
}

pub(crate) type TypeComparer = for<'program, 'state> fn(
    &mut crate::checker::Checker<'program, 'state>,
    TypeHandle,
    TypeHandle,
    bool,
) -> Ternary;

pub(crate) fn compare_types_assignable_worker_entry<'program, 'state>(
    checker: &mut crate::checker::Checker<'program, 'state>,
    source: TypeHandle,
    target: TypeHandle,
    report_errors: bool,
) -> Ternary {
    checker.compare_types_assignable_worker(source, target, report_errors)
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum TypeResolver {
    Bootstrap,
    GlobalType {
        name: &'static str,
        arity: usize,
        report_errors: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum SymbolResolver {
    Bootstrap,
    GlobalTypeAlias {
        name: &'static str,
        arity: usize,
        report_errors: bool,
    },
    GlobalValueSymbol {
        name: &'static str,
        report_errors: bool,
    },
    GlobalTypeSymbol {
        name: &'static str,
        report_errors: bool,
    },
    GlobalAwaited {
        report_errors: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum TypesResolver {
    Bootstrap,
    GlobalTypes {
        names: &'static [&'static str],
        arity: usize,
        report_errors: bool,
    },
}

#[derive(Clone, Copy)]
pub(crate) enum TypeSystemEntity {
    None,
    Symbol(SymbolIdentity),
    Node(ast::Node),
    Type(TypeHandle),
    Signature(SignatureHandle),
}

impl TypeSystemEntity {
    pub(crate) fn symbol_identity(self) -> SymbolIdentity {
        match self {
            Self::Symbol(value) => value,
            _ => panic!("Expected symbol type-resolution target"),
        }
    }

    pub(crate) fn as_node(self) -> ast::Node {
        match self {
            Self::Node(value) => value,
            _ => panic!("Expected node type-resolution target"),
        }
    }

    pub(crate) fn as_type(self) -> TypeHandle {
        match self {
            Self::Type(value) => value,
            _ => panic!("Expected type type-resolution target"),
        }
    }

    pub(crate) fn as_signature(self) -> SignatureHandle {
        match self {
            Self::Signature(value) => value,
            _ => panic!("Expected signature type-resolution target"),
        }
    }
}

impl PartialEq for TypeSystemEntity {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::None, Self::None) => true,
            (Self::Symbol(left), Self::Symbol(right)) => left == right,
            (Self::Node(left), Self::Node(right)) => left == right,
            (Self::Type(left), Self::Type(right)) => left == right,
            (Self::Signature(left), Self::Signature(right)) => left == right,
            _ => false,
        }
    }
}

impl Eq for TypeSystemEntity {}

impl Default for TypeSystemEntity {
    fn default() -> Self {
        Self::None
    }
}

impl From<ast::SymbolHandle> for TypeSystemEntity {
    fn from(value: ast::SymbolHandle) -> Self {
        Self::Symbol(SymbolIdentity::from_symbol_handle(value))
    }
}

impl From<ast::Node> for TypeSystemEntity {
    fn from(value: ast::Node) -> Self {
        Self::Node(value)
    }
}

impl From<TypeHandle> for TypeSystemEntity {
    fn from(value: TypeHandle) -> Self {
        Self::Type(value)
    }
}

impl From<SignatureHandle> for TypeSystemEntity {
    fn from(value: SignatureHandle) -> Self {
        Self::Signature(value)
    }
}

pub type TypeSystemPropertyName = i32;

pub const TYPE_SYSTEM_PROPERTY_NAME_TYPE: TypeSystemPropertyName = 0;
pub const TYPE_SYSTEM_PROPERTY_NAME_RESOLVED_BASE_CONSTRUCTOR_TYPE: TypeSystemPropertyName = 1;
pub const TYPE_SYSTEM_PROPERTY_NAME_DECLARED_TYPE: TypeSystemPropertyName = 2;
pub const TYPE_SYSTEM_PROPERTY_NAME_RESOLVED_RETURN_TYPE: TypeSystemPropertyName = 3;
pub const TYPE_SYSTEM_PROPERTY_NAME_RESOLVED_BASE_CONSTRAINT: TypeSystemPropertyName = 4;
pub const TYPE_SYSTEM_PROPERTY_NAME_RESOLVED_TYPE_ARGUMENTS: TypeSystemPropertyName = 5;
pub const TYPE_SYSTEM_PROPERTY_NAME_RESOLVED_BASE_TYPES: TypeSystemPropertyName = 6;
pub const TYPE_SYSTEM_PROPERTY_NAME_WRITE_TYPE: TypeSystemPropertyName = 7;
pub const TYPE_SYSTEM_PROPERTY_NAME_INITIALIZER_IS_UNDEFINED: TypeSystemPropertyName = 8;
pub const TYPE_SYSTEM_PROPERTY_NAME_ALIAS_TARGET: TypeSystemPropertyName = 9;

#[derive(Clone)]
pub struct TypeResolution {
    pub(crate) target: TypeSystemEntity,
    pub(crate) property_name: TypeSystemPropertyName,
    pub(crate) result: bool,
}

impl Default for TypeResolution {
    fn default() -> Self {
        Self {
            target: TypeSystemEntity::None,
            property_name: TYPE_SYSTEM_PROPERTY_NAME_TYPE,
            result: false,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct ContextualInfo {
    node: Option<ast::Node>,
    t: Option<TypeHandle>,
    is_cache: bool,
}

pub struct WideningContext {
    pub(crate) is_child: bool,
    pub(crate) property_name: String,
    pub(crate) siblings: Vec<TypeHandle>,
    pub(crate) resolved_properties: Vec<SymbolIdentity>,
    pub(crate) child_contexts: HashMap<String, WideningContext>,
    pub(crate) widened_types: HashMap<TypeHandle, TypeHandle>,
}

#[derive(Clone, Default)]
pub struct InferenceContextInfo {
    pub(crate) node: Option<ast::Node>,
    pub(crate) context: Option<InferenceContextRef>,
}

#[derive(Clone, Eq, Hash, PartialEq)]
pub struct EnumLiteralKey {
    pub(crate) enum_symbol: SymbolIdentity,
    pub(crate) value: Any,
}

pub type CachedTypeKind = i32;

pub const CACHED_TYPE_KIND_LITERAL_UNION_BASE_TYPE: CachedTypeKind = 0;
pub const CACHED_TYPE_KIND_INDEX_TYPE: CachedTypeKind = 1;
pub const CACHED_TYPE_KIND_STRING_INDEX_TYPE: CachedTypeKind = 2;
pub const CACHED_TYPE_KIND_EQUIVALENT_BASE_TYPE: CachedTypeKind = 3;
pub const CACHED_TYPE_KIND_APPARENT_TYPE: CachedTypeKind = 4;
pub const CACHED_TYPE_KIND_AWAITED_TYPE: CachedTypeKind = 5;
pub const CACHED_TYPE_KIND_EVOLVING_ARRAY_TYPE: CachedTypeKind = 6;
pub const CACHED_TYPE_KIND_ARRAY_LITERAL_TYPE: CachedTypeKind = 7;
pub const CACHED_TYPE_KIND_PERMISSIVE_INSTANTIATION: CachedTypeKind = 8;
pub const CACHED_TYPE_KIND_RESTRICTIVE_INSTANTIATION: CachedTypeKind = 9;
pub const CACHED_TYPE_KIND_RESTRICTIVE_TYPE_PARAMETER: CachedTypeKind = 10;
pub const CACHED_TYPE_KIND_INDEXED_ACCESS_FOR_READING: CachedTypeKind = 11;
pub const CACHED_TYPE_KIND_INDEXED_ACCESS_FOR_WRITING: CachedTypeKind = 12;
pub const CACHED_TYPE_KIND_WIDENED: CachedTypeKind = 13;
pub const CACHED_TYPE_KIND_REGULAR_OBJECT_LITERAL: CachedTypeKind = 14;
pub const CACHED_TYPE_KIND_PROMISED_TYPE_OF_PROMISE: CachedTypeKind = 15;
pub const CACHED_TYPE_KIND_DEFAULT_ONLY_TYPE: CachedTypeKind = 16;
pub const CACHED_TYPE_KIND_SYNTHETIC_TYPE: CachedTypeKind = 17;
pub const CACHED_TYPE_KIND_DECORATOR_CONTEXT: CachedTypeKind = 18;
pub const CACHED_TYPE_KIND_DECORATOR_CONTEXT_STATIC: CachedTypeKind = 19;
pub const CACHED_TYPE_KIND_DECORATOR_CONTEXT_PRIVATE: CachedTypeKind = 20;
pub const CACHED_TYPE_KIND_DECORATOR_CONTEXT_PRIVATE_STATIC: CachedTypeKind = 21;

pub const CachedTypeKindLiteralUnionBaseType: CachedTypeKind =
    CACHED_TYPE_KIND_LITERAL_UNION_BASE_TYPE;
pub const CachedTypeKindIndexType: CachedTypeKind = CACHED_TYPE_KIND_INDEX_TYPE;
pub const CachedTypeKindStringIndexType: CachedTypeKind = CACHED_TYPE_KIND_STRING_INDEX_TYPE;
pub const CachedTypeKindEquivalentBaseType: CachedTypeKind = CACHED_TYPE_KIND_EQUIVALENT_BASE_TYPE;
pub const CachedTypeKindApparentType: CachedTypeKind = CACHED_TYPE_KIND_APPARENT_TYPE;
pub const CachedTypeKindAwaitedType: CachedTypeKind = CACHED_TYPE_KIND_AWAITED_TYPE;
pub const CachedTypeKindEvolvingArrayType: CachedTypeKind = CACHED_TYPE_KIND_EVOLVING_ARRAY_TYPE;
pub const CachedTypeKindArrayLiteralType: CachedTypeKind = CACHED_TYPE_KIND_ARRAY_LITERAL_TYPE;
pub const CachedTypeKindPermissiveInstantiation: CachedTypeKind =
    CACHED_TYPE_KIND_PERMISSIVE_INSTANTIATION;
pub const CachedTypeKindRestrictiveInstantiation: CachedTypeKind =
    CACHED_TYPE_KIND_RESTRICTIVE_INSTANTIATION;
pub const CachedTypeKindRestrictiveTypeParameter: CachedTypeKind =
    CACHED_TYPE_KIND_RESTRICTIVE_TYPE_PARAMETER;
pub const CachedTypeKindIndexedAccessForReading: CachedTypeKind =
    CACHED_TYPE_KIND_INDEXED_ACCESS_FOR_READING;
pub const CachedTypeKindIndexedAccessForWriting: CachedTypeKind =
    CACHED_TYPE_KIND_INDEXED_ACCESS_FOR_WRITING;
pub const CachedTypeKindWidened: CachedTypeKind = CACHED_TYPE_KIND_WIDENED;
pub const CachedTypeKindRegularObjectLiteral: CachedTypeKind =
    CACHED_TYPE_KIND_REGULAR_OBJECT_LITERAL;
pub const CachedTypeKindPromisedTypeOfPromise: CachedTypeKind =
    CACHED_TYPE_KIND_PROMISED_TYPE_OF_PROMISE;
pub const CachedTypeKindDefaultOnlyType: CachedTypeKind = CACHED_TYPE_KIND_DEFAULT_ONLY_TYPE;
pub const CachedTypeKindSyntheticType: CachedTypeKind = CACHED_TYPE_KIND_SYNTHETIC_TYPE;
pub const CachedTypeKindDecoratorContext: CachedTypeKind = CACHED_TYPE_KIND_DECORATOR_CONTEXT;
pub const CachedTypeKindDecoratorContextStatic: CachedTypeKind =
    CACHED_TYPE_KIND_DECORATOR_CONTEXT_STATIC;
pub const CachedTypeKindDecoratorContextPrivate: CachedTypeKind =
    CACHED_TYPE_KIND_DECORATOR_CONTEXT_PRIVATE;
pub const CachedTypeKindDecoratorContextPrivateStatic: CachedTypeKind =
    CACHED_TYPE_KIND_DECORATOR_CONTEXT_PRIVATE_STATIC;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CachedTypeKey {
    pub(crate) kind: CachedTypeKind,
    pub(crate) type_id: TypeId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NarrowedTypeKey {
    pub(crate) t: TypeHandle,
    pub(crate) candidate: TypeHandle,
    pub(crate) assume_true: bool,
    pub(crate) check_derived: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct UnionOfUnionKey {
    pub(crate) id1: TypeId,
    pub(crate) id2: TypeId,
    pub(crate) r: UnionReduction,
    pub(crate) a: CacheHashKey,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CachedSignatureKey {
    pub(crate) sig: SignatureHandle,
    pub(crate) key: CacheHashKey,
}

pub static SIGNATURE_KEY_ERASED: LazyLock<CacheHashKey> =
    LazyLock::new(|| xxh3::xxh3_128("-".as_bytes()));
pub static SIGNATURE_KEY_CANONICAL: LazyLock<CacheHashKey> =
    LazyLock::new(|| xxh3::xxh3_128("*".as_bytes()));
pub static SIGNATURE_KEY_BASE: LazyLock<CacheHashKey> =
    LazyLock::new(|| xxh3::xxh3_128("#".as_bytes()));
pub static SIGNATURE_KEY_INNER: LazyLock<CacheHashKey> =
    LazyLock::new(|| xxh3::xxh3_128("<".as_bytes()));
pub static SIGNATURE_KEY_OUTER: LazyLock<CacheHashKey> =
    LazyLock::new(|| xxh3::xxh3_128(">".as_bytes()));

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StringMappingKey {
    pub(crate) s: SymbolIdentity,
    pub(crate) t: TypeHandle,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AssignmentReducedKey {
    pub(crate) id1: TypeId,
    pub(crate) id2: TypeId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DiscriminatedContextualTypeKey {
    pub(crate) node_id: ast::NodeId,
    pub(crate) type_id: TypeId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct InstantiationExpressionKey {
    pub(crate) node_id: ast::NodeId,
    pub(crate) type_id: TypeId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SubstitutionTypeKey {
    pub(crate) base_id: TypeId,
    pub(crate) constraint_id: TypeId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ReverseMappedTypeKey {
    pub(crate) source_id: TypeId,
    pub(crate) target_id: TypeId,
    pub(crate) constraint_id: TypeId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct IterationTypesKey {
    pub(crate) type_id: TypeId,
    pub(crate) use_: IterationUse,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PropertiesTypesKey {
    pub(crate) type_id: TypeId,
    pub(crate) include: TypeFlags,
    pub(crate) include_origin: bool,
    pub(crate) unresolved_members: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct NonExistentPropertyKey {
    pub(crate) prop_node: ast::Node,
    pub(crate) containing_type: TypeHandle,
    pub(crate) is_unchecked_js: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FlowLoopKey {
    pub(crate) flow_node: ast::FlowRef,
    pub(crate) ref_key: CacheHashKey,
}

pub struct FlowLoopInfo {
    pub(crate) key: FlowLoopKey,
    pub(crate) types: Vec<TypeHandle>,
}

pub(crate) enum DeferredDiagnostic {
    TypeNotIterable {
        error_node: ast::Node,
        t: TypeHandle,
        allow_async_iterables: bool,
        related_info: Vec<ast::Diagnostic>,
    },
    WeakMapSetCollision {
        node: ast::Node,
    },
    ReflectCollision {
        node: ast::Node,
    },
}

pub type InferenceFlags = u32;

pub const INFERENCE_FLAGS_NONE: InferenceFlags = 0;
pub const INFERENCE_FLAGS_NO_DEFAULT: InferenceFlags = 1 << 0;
pub const INFERENCE_FLAGS_ANY_DEFAULT: InferenceFlags = 1 << 1;
pub const INFERENCE_FLAGS_SKIPPED_GENERIC_FUNCTION: InferenceFlags = 1 << 2;

pub type InferenceContextRef = InferenceContextHandle;

#[derive(Clone)]
pub struct InferenceInfo {
    pub(crate) type_parameter: TypeHandle,
    pub(crate) candidates: Vec<TypeHandle>,
    pub(crate) candidates_present: bool,
    pub(crate) contra_candidates: Vec<TypeHandle>,
    pub(crate) contra_candidates_present: bool,
    pub(crate) inferred_type: Option<TypeHandle>,
    pub(crate) priority: InferencePriority,
    pub(crate) top_level: bool,
    pub(crate) is_fixed: bool,
    pub(crate) implied_arity: isize,
}

pub type InferencePriority = i32;

pub const INFERENCE_PRIORITY_NONE: InferencePriority = 0;
pub const INFERENCE_PRIORITY_NAKED_TYPE_VARIABLE: InferencePriority = 1 << 0;
pub const INFERENCE_PRIORITY_SPECULATIVE_TUPLE: InferencePriority = 1 << 1;
pub const INFERENCE_PRIORITY_SUBSTITUTE_SOURCE: InferencePriority = 1 << 2;
pub const INFERENCE_PRIORITY_HOMOMORPHIC_MAPPED_TYPE: InferencePriority = 1 << 3;
pub const INFERENCE_PRIORITY_PARTIAL_HOMOMORPHIC_MAPPED_TYPE: InferencePriority = 1 << 4;
pub const INFERENCE_PRIORITY_MAPPED_TYPE_CONSTRAINT: InferencePriority = 1 << 5;
pub const INFERENCE_PRIORITY_CONTRAVARIANT_CONDITIONAL: InferencePriority = 1 << 6;
pub const INFERENCE_PRIORITY_RETURN_TYPE: InferencePriority = 1 << 7;
pub const INFERENCE_PRIORITY_LITERAL_KEYOF: InferencePriority = 1 << 8;
pub const INFERENCE_PRIORITY_NO_CONSTRAINTS: InferencePriority = 1 << 9;
pub const INFERENCE_PRIORITY_ALWAYS_STRICT: InferencePriority = 1 << 10;
pub const INFERENCE_PRIORITY_MAX_VALUE: InferencePriority = 1 << 11;
pub const INFERENCE_PRIORITY_CIRCULARITY: InferencePriority = -1;

pub const INFERENCE_PRIORITY_PRIORITY_IMPLIES_COMBINATION: InferencePriority =
    INFERENCE_PRIORITY_RETURN_TYPE
        | INFERENCE_PRIORITY_MAPPED_TYPE_CONSTRAINT
        | INFERENCE_PRIORITY_LITERAL_KEYOF;

#[derive(Clone)]
pub struct IntraExpressionInferenceSite {
    pub(crate) node: ast::Node,
    pub(crate) t: TypeHandle,
}

pub type IterationUse = u32;

pub const ITERATION_USE_ALLOWS_SYNC_ITERABLES_FLAG: IterationUse = 1 << 0;
pub const ITERATION_USE_ALLOWS_ASYNC_ITERABLES_FLAG: IterationUse = 1 << 1;
pub const ITERATION_USE_ALLOWS_STRING_INPUT_FLAG: IterationUse = 1 << 2;
pub const ITERATION_USE_FOR_OF_FLAG: IterationUse = 1 << 3;
pub const ITERATION_USE_YIELD_STAR_FLAG: IterationUse = 1 << 4;
pub const ITERATION_USE_SPREAD_FLAG: IterationUse = 1 << 5;
pub const ITERATION_USE_DESTRUCTURING_FLAG: IterationUse = 1 << 6;
pub const ITERATION_USE_POSSIBLY_OUT_OF_BOUNDS: IterationUse = 1 << 7;
pub const ITERATION_USE_ELEMENT: IterationUse = ITERATION_USE_ALLOWS_SYNC_ITERABLES_FLAG;
pub const ITERATION_USE_SPREAD: IterationUse =
    ITERATION_USE_ALLOWS_SYNC_ITERABLES_FLAG | ITERATION_USE_SPREAD_FLAG;
pub const ITERATION_USE_DESTRUCTURING: IterationUse =
    ITERATION_USE_ALLOWS_SYNC_ITERABLES_FLAG | ITERATION_USE_DESTRUCTURING_FLAG;
pub const ITERATION_USE_FOR_OF: IterationUse = ITERATION_USE_ALLOWS_SYNC_ITERABLES_FLAG
    | ITERATION_USE_ALLOWS_STRING_INPUT_FLAG
    | ITERATION_USE_FOR_OF_FLAG;
pub const ITERATION_USE_FOR_AWAIT_OF: IterationUse = ITERATION_USE_ALLOWS_SYNC_ITERABLES_FLAG
    | ITERATION_USE_ALLOWS_ASYNC_ITERABLES_FLAG
    | ITERATION_USE_ALLOWS_STRING_INPUT_FLAG
    | ITERATION_USE_FOR_OF_FLAG;
pub const ITERATION_USE_YIELD_STAR: IterationUse =
    ITERATION_USE_ALLOWS_SYNC_ITERABLES_FLAG | ITERATION_USE_YIELD_STAR_FLAG;
pub const ITERATION_USE_ASYNC_YIELD_STAR: IterationUse = ITERATION_USE_ALLOWS_SYNC_ITERABLES_FLAG
    | ITERATION_USE_ALLOWS_ASYNC_ITERABLES_FLAG
    | ITERATION_USE_YIELD_STAR_FLAG;
pub const ITERATION_USE_GENERATOR_RETURN_TYPE: IterationUse =
    ITERATION_USE_ALLOWS_SYNC_ITERABLES_FLAG;
pub const ITERATION_USE_ASYNC_GENERATOR_RETURN_TYPE: IterationUse =
    ITERATION_USE_ALLOWS_ASYNC_ITERABLES_FLAG;
pub const ITERATION_USE_CACHE_FLAGS: IterationUse = ITERATION_USE_ALLOWS_SYNC_ITERABLES_FLAG
    | ITERATION_USE_ALLOWS_ASYNC_ITERABLES_FLAG
    | ITERATION_USE_FOR_OF_FLAG;

#[derive(Clone, Copy, Default)]
pub struct IterationTypes {
    pub(crate) yield_type: Option<TypeHandle>,
    pub(crate) return_type: Option<TypeHandle>,
    pub(crate) next_type: Option<TypeHandle>,
}

pub type IterationTypeKind = i32;

pub const ITERATION_TYPE_KIND_YIELD: IterationTypeKind = 0;
pub const ITERATION_TYPE_KIND_RETURN: IterationTypeKind = 1;
pub const ITERATION_TYPE_KIND_NEXT: IterationTypeKind = 2;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum IterationTypeResolver {
    Identity,
    Awaited,
}

impl IterationTypeResolver {
    pub(crate) fn resolve<'a>(
        self,
        checker: &mut crate::checker::Checker<'a, '_>,
        t: TypeHandle,
        error_node: Option<ast::Node>,
    ) -> Option<TypeHandle> {
        match self {
            Self::Identity => Some(t),
            Self::Awaited => checker.get_awaited_type_ex(
                t,
                error_node,
                Some(&diagnostics::TYPE_OF_AWAIT_OPERAND_MUST_EITHER_BE_A_VALID_PROMISE_OR_MUST_NOT_CONTAIN_A_CALLABLE_THEN_MEMBER),
                Vec::<String>::new(),
            ),
        }
    }
}

#[derive(Clone)]
pub struct IterationTypesResolver {
    pub(crate) iterator_symbol_name: String,
    pub(crate) get_global_iterator_type: TypeResolver,
    pub(crate) get_global_iterable_type: TypeResolver,
    pub(crate) get_global_iterable_type_checked: TypeResolver,
    pub(crate) get_global_iterable_iterator_type: TypeResolver,
    pub(crate) get_global_iterable_iterator_type_checked: TypeResolver,
    pub(crate) get_global_iterator_object_type: TypeResolver,
    pub(crate) get_global_generator_type: TypeResolver,
    pub(crate) get_global_builtin_iterator_types: TypesResolver,
    pub(crate) resolve_iteration_type: IterationTypeResolver,
    pub(crate) must_have_a_next_method_diagnostic: &'static diagnostics::Message,
    pub(crate) must_be_a_method_diagnostic: &'static diagnostics::Message,
    pub(crate) must_have_a_value_diagnostic: &'static diagnostics::Message,
}

impl IterationTypesResolver {
    pub(crate) fn get_resolved_iteration_types<'a>(
        &self,
        yield_type: TypeHandle,
        return_type: TypeHandle,
        next_type: TypeHandle,
        checker: &mut crate::checker::Checker<'a, '_>,
    ) -> IterationTypes {
        IterationTypes {
            yield_type: self
                .resolve_iteration_type
                .resolve(checker, yield_type, None)
                .or(Some(yield_type)),
            return_type: self
                .resolve_iteration_type
                .resolve(checker, return_type, None)
                .or(Some(return_type)),
            next_type: Some(next_type),
        }
    }
}

impl IterationTypes {
    pub(crate) fn has_types(&self) -> bool {
        self.yield_type.is_some() || self.return_type.is_some() || self.next_type.is_some()
    }

    pub(crate) fn get_type(&self, type_kind: IterationTypeKind) -> Option<TypeHandle> {
        match type_kind {
            ITERATION_TYPE_KIND_YIELD => self.yield_type,
            ITERATION_TYPE_KIND_RETURN => self.return_type,
            ITERATION_TYPE_KIND_NEXT => self.next_type,
            _ => panic!("Unhandled case in getType(IterationTypeKind)"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourceFileIdentity(ast::Node);

impl SourceFileIdentity {
    pub const fn from_root(root: ast::Node) -> Self {
        Self(root)
    }

    pub fn from_source_file(file: &ast::SourceFile) -> Self {
        Self(file.as_node())
    }

    pub const fn root(self) -> ast::Node {
        self.0
    }
}

#[derive(Clone, Default)]
pub(crate) struct PatternAmbientModuleRecord {
    pub(crate) pattern: core::Pattern,
    pub(crate) symbol: Option<SymbolIdentity>,
}

impl core::IntoLinkKey<SourceFileIdentity> for &ast::SourceFile {
    fn into_link_key(self) -> SourceFileIdentity {
        SourceFileIdentity::from_source_file(self)
    }
}

impl core::IntoLinkKey<SourceFileIdentity> for &mut ast::SourceFile {
    fn into_link_key(self) -> SourceFileIdentity {
        SourceFileIdentity::from_source_file(self)
    }
}

pub(crate) struct SparseLinkStore<K, V>(core::LinkStore<K, V>);

impl<K, V> Default for SparseLinkStore<K, V> {
    fn default() -> Self {
        Self(core::LinkStore::default())
    }
}

impl<K, V> SparseLinkStore<K, V>
where
    K: Eq + std::hash::Hash,
    V: Default,
{
    fn ensure_handle<Q>(&self, key: Q) -> core::LinkHandle<V>
    where
        Q: core::IntoLinkKey<K>,
    {
        self.0.ensure_handle(key)
    }

    fn has<Q>(&self, key: Q) -> bool
    where
        Q: core::IntoLinkKey<K>,
    {
        self.0.has(key)
    }

    fn try_handle<Q>(&self, key: Q) -> Option<core::LinkHandle<V>>
    where
        Q: core::IntoLinkKey<K>,
    {
        self.0.try_handle(key)
    }

    fn allocate_unkeyed_handle(&self) -> core::LinkHandle<V> {
        self.0.allocate_unkeyed_handle()
    }

    fn with_by_handle<R>(&self, handle: core::LinkHandle<V>, f: impl FnOnce(&V) -> R) -> R {
        self.0.with_by_handle(handle, f)
    }

    fn with_by_handle_mut<R>(&self, handle: core::LinkHandle<V>, f: impl FnOnce(&mut V) -> R) -> R {
        self.0.with_by_handle_mut(handle, f)
    }
}

pub(crate) struct NodeLinkStore<V> {
    // TypeScript-Go stores node links in core.LinkStore keyed by *ast.Node.
    // Rust nodes already carry dense store/node identities, so keep the same
    // get-or-create link semantics with a node side table for keys and an
    // unkeyed link arena for values.
    handles: RefCell<ast::NodeSideTable<core::LinkHandle<V>>>,
    links: core::LinkStore<(), V>,
    last_handle: Cell<Option<(ast::Node, core::LinkHandle<V>)>>,
}

impl<V> Default for NodeLinkStore<V> {
    fn default() -> Self {
        Self {
            handles: RefCell::new(ast::NodeSideTable::default()),
            links: core::LinkStore::default(),
            last_handle: Cell::new(None),
        }
    }
}

impl<V> NodeLinkStore<V>
where
    V: Default,
{
    fn ensure_handle(&self, node: ast::Node) -> core::LinkHandle<V> {
        if let Some((cached_node, handle)) = self.last_handle.get()
            && cached_node == node
        {
            return handle;
        }
        if let Some(handle) = self.handles.borrow().get_copied(node) {
            self.last_handle.set(Some((node, handle)));
            return handle;
        }
        let handle = self.links.allocate_unkeyed_handle();
        self.handles.borrow_mut().insert(node, handle);
        self.last_handle.set(Some((node, handle)));
        handle
    }

    fn has(&self, node: ast::Node) -> bool {
        if let Some((cached_node, _)) = self.last_handle.get()
            && cached_node == node
        {
            return true;
        }
        if let Some(handle) = self.handles.borrow().get_copied(node) {
            self.last_handle.set(Some((node, handle)));
            return true;
        }
        false
    }

    fn try_handle(&self, node: ast::Node) -> Option<core::LinkHandle<V>> {
        if let Some((cached_node, handle)) = self.last_handle.get()
            && cached_node == node
        {
            return Some(handle);
        }
        let handle = self.handles.borrow().get_copied(node)?;
        self.last_handle.set(Some((node, handle)));
        Some(handle)
    }

    fn with_by_handle<R>(&self, handle: core::LinkHandle<V>, f: impl FnOnce(&V) -> R) -> R {
        self.links.with_by_handle(handle, f)
    }

    fn with_by_handle_mut<R>(&self, handle: core::LinkHandle<V>, f: impl FnOnce(&mut V) -> R) -> R {
        self.links.with_by_handle_mut(handle, f)
    }
}

struct DenseSymbolOwnerTable<T> {
    base: Option<u64>,
    values: Vec<Option<T>>,
}

impl<T> Default for DenseSymbolOwnerTable<T> {
    fn default() -> Self {
        Self {
            base: None,
            values: Vec::new(),
        }
    }
}

impl<T> DenseSymbolOwnerTable<T> {
    fn index_for(&self, owner_key: ast::SymbolOwnerKey) -> Option<usize> {
        let base = self.base?;
        usize::try_from(owner_key.as_u64().checked_sub(base)?).ok()
    }

    fn get(&self, owner_key: ast::SymbolOwnerKey) -> Option<&T> {
        let index = self.index_for(owner_key)?;
        self.values.get(index).and_then(Option::as_ref)
    }

    fn get_or_insert_default(&mut self, owner_key: ast::SymbolOwnerKey) -> &mut T
    where
        T: Default,
    {
        let index = self.ensure_index(owner_key);
        self.values[index].get_or_insert_with(T::default)
    }

    fn ensure_index(&mut self, owner_key: ast::SymbolOwnerKey) -> usize {
        let raw = owner_key.as_u64();
        let Some(base) = self.base else {
            self.base = Some(raw);
            self.values.push(None);
            return 0;
        };
        if raw < base {
            let prepend = usize::try_from(base - raw)
                .expect("symbol owner id range exceeds addressable memory");
            self.values.splice(0..0, (0..prepend).map(|_| None));
            self.base = Some(raw);
            return 0;
        }
        let index =
            usize::try_from(raw - base).expect("symbol owner id range exceeds addressable memory");
        if index >= self.values.len() {
            self.values.resize_with(index + 1, || None);
        }
        index
    }
}

pub(crate) struct SymbolLinkStore<V> {
    // TypeScript-Go stores symbol links in core.LinkStore keyed by *ast.Symbol.
    // Rust symbol handles already carry the same stable identity as
    // owner/index, so side tables avoid hashing the full SymbolIdentity on hot
    // symbol-link paths.
    links: core::LinkStore<(), V>,
    last_handle: Cell<Option<(SymbolIdentity, core::LinkHandle<V>)>>,
    owner_handles: RefCell<DenseSymbolOwnerTable<Vec<Option<core::LinkHandle<V>>>>>,
    transient_handles: RefCell<Vec<Option<(ast::SymbolOwnerKey, core::LinkHandle<V>)>>>,
}

pub(crate) struct MergedSymbolStore {
    // TypeScript-Go stores merged symbol redirects in a pointer-keyed map. Rust
    // symbol handles expose the equivalent owner/index identity, so this avoids
    // hashing SymbolIdentity on the getMergedSymbol hot path.
    last_value: Cell<Option<(SymbolIdentity, Option<SymbolIdentity>)>>,
    owner_values: RefCell<DenseSymbolOwnerTable<Vec<Option<SymbolIdentity>>>>,
    transient_values: RefCell<Vec<Option<(ast::SymbolOwnerKey, SymbolIdentity)>>>,
}

impl Default for MergedSymbolStore {
    fn default() -> Self {
        Self {
            last_value: Cell::new(None),
            owner_values: RefCell::new(DenseSymbolOwnerTable::default()),
            transient_values: RefCell::new(Vec::new()),
        }
    }
}

impl MergedSymbolStore {
    fn get_owner_value(
        &self,
        owner_key: ast::SymbolOwnerKey,
        index: usize,
    ) -> Option<SymbolIdentity> {
        self.owner_values
            .borrow()
            .get(owner_key)
            .and_then(|values| values.get(index).copied().flatten())
    }

    fn set_owner_value(
        &self,
        owner_key: ast::SymbolOwnerKey,
        index: usize,
        target: SymbolIdentity,
    ) {
        let mut values_by_owner = self.owner_values.borrow_mut();
        let values = values_by_owner.get_or_insert_default(owner_key);
        if values.len() <= index {
            values.resize(index + 1, None);
        }
        values[index] = Some(target);
    }

    fn get_transient_value(
        &self,
        owner_key: ast::SymbolOwnerKey,
        index: usize,
    ) -> Option<SymbolIdentity> {
        let transient_values = self.transient_values.borrow();
        if let Some(Some((existing_owner, target))) = transient_values.get(index)
            && *existing_owner == owner_key
        {
            return Some(*target);
        }
        None
    }

    fn set_transient_value(
        &self,
        owner_key: ast::SymbolOwnerKey,
        index: usize,
        target: SymbolIdentity,
    ) {
        let mut transient_values = self.transient_values.borrow_mut();
        if transient_values.len() <= index {
            transient_values.resize(index + 1, None);
        }
        match transient_values[index] {
            Some((existing_owner, _)) if existing_owner == owner_key => {
                transient_values[index] = Some((owner_key, target));
            }
            Some(_) => {
                drop(transient_values);
                self.set_owner_value(owner_key, index, target);
            }
            None => {
                transient_values[index] = Some((owner_key, target));
            }
        }
    }

    fn get<Q>(&self, key: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let source = key.into_link_key();
        if let Some((cached_source, cached_target)) = self.last_value.get()
            && cached_source == source
        {
            return cached_target;
        }
        let symbol_handle = source.symbol_handle();
        let index = symbol_handle.symbol_index();
        let owner_key = symbol_handle.owner_key();
        let target = match symbol_handle.domain() {
            ast::SymbolDomain::Program => self.get_owner_value(owner_key, index),
            ast::SymbolDomain::CheckerTransient => self
                .get_transient_value(owner_key, index)
                .or_else(|| self.get_owner_value(owner_key, index)),
        };
        self.last_value.set(Some((source, target)));
        target
    }

    fn insert<Q>(&self, key: Q, target: SymbolIdentity)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let source = key.into_link_key();
        let symbol_handle = source.symbol_handle();
        let index = symbol_handle.symbol_index();
        let owner_key = symbol_handle.owner_key();
        match symbol_handle.domain() {
            ast::SymbolDomain::Program => self.set_owner_value(owner_key, index, target),
            ast::SymbolDomain::CheckerTransient => {
                self.set_transient_value(owner_key, index, target);
            }
        }
        self.last_value.set(Some((source, Some(target))));
    }
}

impl<V> Default for SymbolLinkStore<V> {
    fn default() -> Self {
        Self {
            links: core::LinkStore::default(),
            last_handle: Cell::new(None),
            owner_handles: RefCell::new(DenseSymbolOwnerTable::default()),
            transient_handles: RefCell::new(Vec::new()),
        }
    }
}

impl<V> SymbolLinkStore<V>
where
    V: Default,
{
    fn ensure_handle_in_table(
        &self,
        handles: &mut Vec<Option<core::LinkHandle<V>>>,
        index: usize,
    ) -> core::LinkHandle<V> {
        if handles.len() <= index {
            handles.resize(index + 1, None);
        }
        if let Some(handle) = handles[index] {
            return handle;
        }
        let handle = self.links.allocate_unkeyed_handle();
        handles[index] = Some(handle);
        handle
    }

    fn ensure_owner_handle(
        &self,
        owner_key: ast::SymbolOwnerKey,
        index: usize,
    ) -> core::LinkHandle<V> {
        let mut handles_by_owner = self.owner_handles.borrow_mut();
        let owner_handles = handles_by_owner.get_or_insert_default(owner_key);
        self.ensure_handle_in_table(owner_handles, index)
    }

    fn ensure_transient_handle(
        &self,
        owner_key: ast::SymbolOwnerKey,
        index: usize,
    ) -> core::LinkHandle<V> {
        let mut transient_handles = self.transient_handles.borrow_mut();
        if transient_handles.len() <= index {
            transient_handles.resize(index + 1, None);
        }
        match transient_handles[index] {
            Some((existing_owner, handle)) if existing_owner == owner_key => handle,
            Some(_) => {
                drop(transient_handles);
                self.ensure_owner_handle(owner_key, index)
            }
            None => {
                let handle = self.links.allocate_unkeyed_handle();
                transient_handles[index] = Some((owner_key, handle));
                handle
            }
        }
    }

    fn allocate_fresh_transient_symbol_handle(
        &self,
        symbol: SymbolIdentity,
    ) -> core::LinkHandle<V> {
        let symbol_handle = symbol.symbol_handle();
        debug_assert_eq!(
            symbol_handle.domain(),
            ast::SymbolDomain::CheckerTransient,
            "fresh transient symbol links are only valid for checker-owned symbols"
        );
        let index = symbol_handle.symbol_index();
        let owner_key = symbol_handle.owner_key();
        let mut transient_handles = self.transient_handles.borrow_mut();
        if transient_handles.len() <= index {
            transient_handles.resize(index + 1, None);
        }
        debug_assert!(
            transient_handles[index].is_none(),
            "fresh transient symbol already has links"
        );
        let handle = self.links.allocate_unkeyed_handle();
        transient_handles[index] = Some((owner_key, handle));
        self.last_handle.set(Some((symbol, handle)));
        handle
    }

    fn try_handle_in_table(
        handles: &[Option<core::LinkHandle<V>>],
        index: usize,
    ) -> Option<core::LinkHandle<V>> {
        handles.get(index).and_then(|handle| *handle)
    }

    fn ensure_symbol_handle(&self, symbol: SymbolIdentity) -> core::LinkHandle<V> {
        if let Some((cached_symbol, handle)) = self.last_handle.get()
            && cached_symbol == symbol
        {
            return handle;
        }
        let symbol_handle = symbol.symbol_handle();
        let index = symbol_handle.symbol_index();
        let owner_key = symbol_handle.owner_key();
        let handle = if symbol_handle.domain() == ast::SymbolDomain::Program {
            self.ensure_owner_handle(owner_key, index)
        } else if symbol_handle.domain() == ast::SymbolDomain::CheckerTransient {
            self.ensure_transient_handle(owner_key, index)
        } else {
            unreachable!("unknown symbol domain")
        };
        self.last_handle.set(Some((symbol, handle)));
        handle
    }

    fn ensure_handle<Q>(&self, symbol: Q) -> core::LinkHandle<V>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let symbol = symbol.into_link_key();
        self.ensure_symbol_handle(symbol)
    }

    fn has<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let symbol = symbol.into_link_key();
        if let Some((cached_symbol, _)) = self.last_handle.get()
            && cached_symbol == symbol
        {
            return true;
        }
        let symbol_handle = symbol.symbol_handle();
        let index = symbol_handle.symbol_index();
        let owner_key = symbol_handle.owner_key();
        if symbol_handle.domain() == ast::SymbolDomain::CheckerTransient {
            let transient_handles = self.transient_handles.borrow();
            if let Some(Some((existing_owner, _))) = transient_handles.get(index)
                && *existing_owner == owner_key
            {
                return true;
            }
        }
        if symbol_handle.domain() == ast::SymbolDomain::Program
            || symbol_handle.domain() == ast::SymbolDomain::CheckerTransient
        {
            return self
                .owner_handles
                .borrow()
                .get(owner_key)
                .is_some_and(|handles| Self::try_handle_in_table(handles, index).is_some());
        }
        false
    }

    fn try_handle<Q>(&self, symbol: Q) -> Option<core::LinkHandle<V>>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let symbol = symbol.into_link_key();
        if let Some((cached_symbol, handle)) = self.last_handle.get()
            && cached_symbol == symbol
        {
            return Some(handle);
        }
        let symbol_handle = symbol.symbol_handle();
        let index = symbol_handle.symbol_index();
        let owner_key = symbol_handle.owner_key();
        let handle = if symbol_handle.domain() == ast::SymbolDomain::CheckerTransient {
            {
                let transient_handles = self.transient_handles.borrow();
                if let Some(Some((existing_owner, handle))) = transient_handles.get(index)
                    && *existing_owner == owner_key
                {
                    Some(*handle)
                } else {
                    None
                }
            }
            .or_else(|| {
                self.owner_handles
                    .borrow()
                    .get(owner_key)
                    .and_then(|handles| Self::try_handle_in_table(handles, index))
            })
        } else if symbol_handle.domain() == ast::SymbolDomain::Program {
            self.owner_handles
                .borrow()
                .get(owner_key)
                .and_then(|handles| Self::try_handle_in_table(handles, index))
        } else {
            None
        };
        if let Some(handle) = handle {
            self.last_handle.set(Some((symbol, handle)));
        }
        handle
    }

    fn with_by_handle<R>(&self, handle: core::LinkHandle<V>, f: impl FnOnce(&V) -> R) -> R {
        self.links.with_by_handle(handle, f)
    }

    fn with_by_handle_mut<R>(&self, handle: core::LinkHandle<V>, f: impl FnOnce(&mut V) -> R) -> R {
        self.links.with_by_handle_mut(handle, f)
    }
}

pub(crate) struct SourceFileLinkStore<V> {
    sparse: SparseLinkStore<SourceFileIdentity, V>,
    last_handle: Cell<Option<(SourceFileIdentity, core::LinkHandle<V>)>>,
}

impl<V> Default for SourceFileLinkStore<V> {
    fn default() -> Self {
        Self {
            sparse: SparseLinkStore::default(),
            last_handle: Cell::new(None),
        }
    }
}

impl<V> SourceFileLinkStore<V>
where
    V: Default,
{
    fn ensure_handle<Q>(&self, source_file: Q) -> core::LinkHandle<V>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let source_file = source_file.into_link_key();
        if let Some((cached_source_file, handle)) = self.last_handle.get()
            && cached_source_file == source_file
        {
            return handle;
        }
        let handle = self.sparse.ensure_handle(source_file);
        self.last_handle.set(Some((source_file, handle)));
        handle
    }

    fn has<Q>(&self, source_file: Q) -> bool
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        self.sparse.has(source_file)
    }

    fn try_handle<Q>(&self, source_file: Q) -> Option<core::LinkHandle<V>>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        self.sparse.try_handle(source_file)
    }

    fn with_by_handle<R>(&self, handle: core::LinkHandle<V>, f: impl FnOnce(&V) -> R) -> R {
        self.sparse.with_by_handle(handle, f)
    }

    fn with_by_handle_mut<R>(&self, handle: core::LinkHandle<V>, f: impl FnOnce(&mut V) -> R) -> R {
        self.sparse.with_by_handle_mut(handle, f)
    }
}

pub(crate) trait NodeLinksStoreExt {
    fn node_link_handle(&self, node: ast::Node) -> core::LinkHandle<NodeLinks>;
    fn node_link_flags(&self, node: ast::Node) -> NodeCheckFlags;
    fn node_link_flags_by_handle(&self, handle: core::LinkHandle<NodeLinks>) -> NodeCheckFlags;
    fn set_node_link_flags(&self, node: ast::Node, flags: NodeCheckFlags);
    fn set_node_link_flags_by_handle(
        &self,
        handle: core::LinkHandle<NodeLinks>,
        flags: NodeCheckFlags,
    );
    fn add_node_link_flags(&self, node: ast::Node, flags: NodeCheckFlags);
    fn add_node_link_flags_by_handle(
        &self,
        handle: core::LinkHandle<NodeLinks>,
        flags: NodeCheckFlags,
    );
    fn remove_node_link_flags(&self, node: ast::Node, flags: NodeCheckFlags);
    fn remove_node_link_flags_by_handle(
        &self,
        handle: core::LinkHandle<NodeLinks>,
        flags: NodeCheckFlags,
    );
    fn has_node_link_flags(&self, node: ast::Node, flags: NodeCheckFlags) -> bool;
    fn has_node_link_flags_by_handle(
        &self,
        handle: core::LinkHandle<NodeLinks>,
        flags: NodeCheckFlags,
    ) -> bool;
    fn node_declaration_requires_scope_change(&self, node: ast::Node) -> core::Tristate;
    fn node_declaration_requires_scope_change_by_handle(
        &self,
        handle: core::LinkHandle<NodeLinks>,
    ) -> core::Tristate;
    fn set_node_declaration_requires_scope_change(&self, node: ast::Node, value: core::Tristate);
    fn set_node_declaration_requires_scope_change_by_handle(
        &self,
        handle: core::LinkHandle<NodeLinks>,
        value: core::Tristate,
    );
    fn node_has_reported_statement_in_ambient_context(&self, node: ast::Node) -> bool;
    fn set_node_has_reported_statement_in_ambient_context(&self, node: ast::Node, value: bool);
}

impl NodeLinksStoreExt for NodeLinkStore<NodeLinks> {
    fn node_link_handle(&self, node: ast::Node) -> core::LinkHandle<NodeLinks> {
        self.ensure_handle(node)
    }

    fn node_link_flags(&self, node: ast::Node) -> NodeCheckFlags {
        let handle = self.node_link_handle(node);
        self.node_link_flags_by_handle(handle)
    }

    fn node_link_flags_by_handle(&self, handle: core::LinkHandle<NodeLinks>) -> NodeCheckFlags {
        self.with_by_handle(handle, |links| links.flags)
    }

    fn set_node_link_flags(&self, node: ast::Node, flags: NodeCheckFlags) {
        let handle = self.node_link_handle(node);
        self.set_node_link_flags_by_handle(handle, flags);
    }

    fn set_node_link_flags_by_handle(
        &self,
        handle: core::LinkHandle<NodeLinks>,
        flags: NodeCheckFlags,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.flags = flags;
        });
    }

    fn add_node_link_flags(&self, node: ast::Node, flags: NodeCheckFlags) {
        let handle = self.node_link_handle(node);
        self.add_node_link_flags_by_handle(handle, flags);
    }

    fn add_node_link_flags_by_handle(
        &self,
        handle: core::LinkHandle<NodeLinks>,
        flags: NodeCheckFlags,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.flags |= flags;
        });
    }

    fn remove_node_link_flags(&self, node: ast::Node, flags: NodeCheckFlags) {
        let handle = self.node_link_handle(node);
        self.remove_node_link_flags_by_handle(handle, flags);
    }

    fn remove_node_link_flags_by_handle(
        &self,
        handle: core::LinkHandle<NodeLinks>,
        flags: NodeCheckFlags,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.flags &= !flags;
        });
    }

    fn has_node_link_flags(&self, node: ast::Node, flags: NodeCheckFlags) -> bool {
        let handle = self.node_link_handle(node);
        self.has_node_link_flags_by_handle(handle, flags)
    }

    fn has_node_link_flags_by_handle(
        &self,
        handle: core::LinkHandle<NodeLinks>,
        flags: NodeCheckFlags,
    ) -> bool {
        self.node_link_flags_by_handle(handle) & flags != 0
    }

    fn node_declaration_requires_scope_change(&self, node: ast::Node) -> core::Tristate {
        let handle = self.node_link_handle(node);
        self.node_declaration_requires_scope_change_by_handle(handle)
    }

    fn node_declaration_requires_scope_change_by_handle(
        &self,
        handle: core::LinkHandle<NodeLinks>,
    ) -> core::Tristate {
        self.with_by_handle(handle, |links| links.declaration_requires_scope_change)
    }

    fn set_node_declaration_requires_scope_change(&self, node: ast::Node, value: core::Tristate) {
        let handle = self.node_link_handle(node);
        self.set_node_declaration_requires_scope_change_by_handle(handle, value);
    }

    fn set_node_declaration_requires_scope_change_by_handle(
        &self,
        handle: core::LinkHandle<NodeLinks>,
        value: core::Tristate,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.declaration_requires_scope_change = value;
        });
    }

    fn node_has_reported_statement_in_ambient_context(&self, node: ast::Node) -> bool {
        let handle = self.node_link_handle(node);
        self.with_by_handle(handle, |links| {
            links.has_reported_statement_in_ambient_context
        })
    }

    fn set_node_has_reported_statement_in_ambient_context(&self, node: ast::Node, value: bool) {
        let handle = self.node_link_handle(node);
        self.with_by_handle_mut(handle, |links| {
            links.has_reported_statement_in_ambient_context = value;
        });
    }
}

pub(crate) trait SymbolNodeLinksStoreExt {
    fn symbol_node_link_handle(&self, node: ast::Node) -> core::LinkHandle<SymbolNodeLinks>;
    fn node_resolved_symbol_identity(&self, node: ast::Node) -> Option<SymbolIdentity>;
    fn node_resolved_symbol_identity_by_handle(
        &self,
        handle: core::LinkHandle<SymbolNodeLinks>,
    ) -> Option<SymbolIdentity>;
    fn set_node_resolved_symbol_identity(&self, node: ast::Node, symbol: Option<SymbolIdentity>);
    fn set_node_resolved_symbol_identity_by_handle(
        &self,
        handle: core::LinkHandle<SymbolNodeLinks>,
        symbol: Option<SymbolIdentity>,
    );
}

impl SymbolNodeLinksStoreExt for NodeLinkStore<SymbolNodeLinks> {
    fn symbol_node_link_handle(&self, node: ast::Node) -> core::LinkHandle<SymbolNodeLinks> {
        self.ensure_handle(node)
    }

    fn node_resolved_symbol_identity(&self, node: ast::Node) -> Option<SymbolIdentity> {
        let handle = self.symbol_node_link_handle(node);
        self.node_resolved_symbol_identity_by_handle(handle)
    }

    fn node_resolved_symbol_identity_by_handle(
        &self,
        handle: core::LinkHandle<SymbolNodeLinks>,
    ) -> Option<SymbolIdentity> {
        self.with_by_handle(handle, |links| links.resolved_symbol)
    }

    fn set_node_resolved_symbol_identity(&self, node: ast::Node, symbol: Option<SymbolIdentity>) {
        let handle = self.symbol_node_link_handle(node);
        self.set_node_resolved_symbol_identity_by_handle(handle, symbol);
    }

    fn set_node_resolved_symbol_identity_by_handle(
        &self,
        handle: core::LinkHandle<SymbolNodeLinks>,
        symbol: Option<SymbolIdentity>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.resolved_symbol = symbol;
        });
    }
}

pub(crate) trait TypeNodeLinksStoreExt {
    fn type_node_link_handle(&self, node: ast::Node) -> core::LinkHandle<TypeNodeLinks>;
    fn type_node_resolved_type(&self, node: ast::Node) -> Option<TypeHandle>;
    fn type_node_resolved_type_by_handle(
        &self,
        handle: core::LinkHandle<TypeNodeLinks>,
    ) -> Option<TypeHandle>;
    fn try_type_node_resolved_type(&self, node: ast::Node) -> Option<TypeHandle>;
    fn set_type_node_resolved_type(&self, node: ast::Node, resolved_type: Option<TypeHandle>);
    fn set_type_node_resolved_type_by_handle(
        &self,
        handle: core::LinkHandle<TypeNodeLinks>,
        resolved_type: Option<TypeHandle>,
    );
    fn type_node_outer_type_parameters(&self, node: ast::Node) -> Option<Arc<[TypeHandle]>>;
    fn type_node_outer_type_parameters_by_handle(
        &self,
        handle: core::LinkHandle<TypeNodeLinks>,
    ) -> Option<Arc<[TypeHandle]>>;
    fn set_type_node_outer_type_parameters<T>(&self, node: ast::Node, outer_type_parameters: T)
    where
        T: Into<Arc<[TypeHandle]>>;
    fn set_type_node_outer_type_parameters_by_handle<T>(
        &self,
        handle: core::LinkHandle<TypeNodeLinks>,
        outer_type_parameters: T,
    ) where
        T: Into<Arc<[TypeHandle]>>;
}

impl TypeNodeLinksStoreExt for NodeLinkStore<TypeNodeLinks> {
    fn type_node_link_handle(&self, node: ast::Node) -> core::LinkHandle<TypeNodeLinks> {
        self.ensure_handle(node)
    }

    fn type_node_resolved_type(&self, node: ast::Node) -> Option<TypeHandle> {
        let handle = self.type_node_link_handle(node);
        self.type_node_resolved_type_by_handle(handle)
    }

    fn type_node_resolved_type_by_handle(
        &self,
        handle: core::LinkHandle<TypeNodeLinks>,
    ) -> Option<TypeHandle> {
        self.with_by_handle(handle, |links| links.resolved_type)
    }

    fn try_type_node_resolved_type(&self, node: ast::Node) -> Option<TypeHandle> {
        let handle = self.try_handle(node)?;
        self.type_node_resolved_type_by_handle(handle)
    }

    fn set_type_node_resolved_type(&self, node: ast::Node, resolved_type: Option<TypeHandle>) {
        let handle = self.type_node_link_handle(node);
        self.set_type_node_resolved_type_by_handle(handle, resolved_type);
    }

    fn set_type_node_resolved_type_by_handle(
        &self,
        handle: core::LinkHandle<TypeNodeLinks>,
        resolved_type: Option<TypeHandle>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.resolved_type = resolved_type;
        });
    }

    fn type_node_outer_type_parameters(&self, node: ast::Node) -> Option<Arc<[TypeHandle]>> {
        let handle = self.type_node_link_handle(node);
        self.type_node_outer_type_parameters_by_handle(handle)
    }

    fn type_node_outer_type_parameters_by_handle(
        &self,
        handle: core::LinkHandle<TypeNodeLinks>,
    ) -> Option<Arc<[TypeHandle]>> {
        self.with_by_handle(handle, |links| links.outer_type_parameters.clone())
    }

    fn set_type_node_outer_type_parameters<T>(&self, node: ast::Node, outer_type_parameters: T)
    where
        T: Into<Arc<[TypeHandle]>>,
    {
        let handle = self.type_node_link_handle(node);
        self.set_type_node_outer_type_parameters_by_handle(handle, outer_type_parameters);
    }

    fn set_type_node_outer_type_parameters_by_handle<T>(
        &self,
        handle: core::LinkHandle<TypeNodeLinks>,
        outer_type_parameters: T,
    ) where
        T: Into<Arc<[TypeHandle]>>,
    {
        self.with_by_handle_mut(handle, |links| {
            links.outer_type_parameters = Some(outer_type_parameters.into());
        });
    }
}

pub(crate) trait AssertionLinksStoreExt {
    fn assertion_link_handle(&self, node: ast::Node) -> core::LinkHandle<AssertionLinks>;
    fn assertion_expression_type(&self, node: ast::Node) -> Option<TypeHandle>;
    fn assertion_expression_type_by_handle(
        &self,
        handle: core::LinkHandle<AssertionLinks>,
    ) -> Option<TypeHandle>;
    fn set_assertion_expression_type(&self, node: ast::Node, expr_type: Option<TypeHandle>);
    fn set_assertion_expression_type_by_handle(
        &self,
        handle: core::LinkHandle<AssertionLinks>,
        expr_type: Option<TypeHandle>,
    );
}

impl AssertionLinksStoreExt for NodeLinkStore<AssertionLinks> {
    fn assertion_link_handle(&self, node: ast::Node) -> core::LinkHandle<AssertionLinks> {
        self.ensure_handle(node)
    }

    fn assertion_expression_type(&self, node: ast::Node) -> Option<TypeHandle> {
        let handle = self.assertion_link_handle(node);
        self.assertion_expression_type_by_handle(handle)
    }

    fn assertion_expression_type_by_handle(
        &self,
        handle: core::LinkHandle<AssertionLinks>,
    ) -> Option<TypeHandle> {
        self.with_by_handle(handle, |links| links.expr_type)
    }

    fn set_assertion_expression_type(&self, node: ast::Node, expr_type: Option<TypeHandle>) {
        let handle = self.assertion_link_handle(node);
        self.set_assertion_expression_type_by_handle(handle, expr_type);
    }

    fn set_assertion_expression_type_by_handle(
        &self,
        handle: core::LinkHandle<AssertionLinks>,
        expr_type: Option<TypeHandle>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.expr_type = expr_type;
        });
    }
}

pub(crate) trait SignatureLinksStoreExt {
    fn signature_link_handle(&self, node: ast::Node) -> core::LinkHandle<SignatureLinks>;
    fn resolved_signature(&self, node: ast::Node) -> Option<SignatureHandle>;
    fn resolved_signature_by_handle(
        &self,
        handle: core::LinkHandle<SignatureLinks>,
    ) -> Option<SignatureHandle>;
    fn set_resolved_signature(&self, node: ast::Node, signature: Option<SignatureHandle>);
    fn set_resolved_signature_by_handle(
        &self,
        handle: core::LinkHandle<SignatureLinks>,
        signature: Option<SignatureHandle>,
    );
    fn replace_resolved_signature(
        &self,
        node: ast::Node,
        signature: Option<SignatureHandle>,
    ) -> Option<SignatureHandle>;
    fn replace_resolved_signature_by_handle(
        &self,
        handle: core::LinkHandle<SignatureLinks>,
        signature: Option<SignatureHandle>,
    ) -> Option<SignatureHandle>;
    fn effects_signature(&self, node: ast::Node) -> Option<SignatureHandle>;
    fn effects_signature_by_handle(
        &self,
        handle: core::LinkHandle<SignatureLinks>,
    ) -> Option<SignatureHandle>;
    fn set_effects_signature(&self, node: ast::Node, signature: Option<SignatureHandle>);
    fn set_effects_signature_by_handle(
        &self,
        handle: core::LinkHandle<SignatureLinks>,
        signature: Option<SignatureHandle>,
    );
    fn decorator_signature(&self, node: ast::Node) -> Option<SignatureHandle>;
    fn decorator_signature_by_handle(
        &self,
        handle: core::LinkHandle<SignatureLinks>,
    ) -> Option<SignatureHandle>;
    fn set_decorator_signature(&self, node: ast::Node, signature: Option<SignatureHandle>);
    fn set_decorator_signature_by_handle(
        &self,
        handle: core::LinkHandle<SignatureLinks>,
        signature: Option<SignatureHandle>,
    );
}

impl SignatureLinksStoreExt for NodeLinkStore<SignatureLinks> {
    fn signature_link_handle(&self, node: ast::Node) -> core::LinkHandle<SignatureLinks> {
        self.ensure_handle(node)
    }

    fn resolved_signature(&self, node: ast::Node) -> Option<SignatureHandle> {
        let handle = self.signature_link_handle(node);
        self.resolved_signature_by_handle(handle)
    }

    fn resolved_signature_by_handle(
        &self,
        handle: core::LinkHandle<SignatureLinks>,
    ) -> Option<SignatureHandle> {
        self.with_by_handle(handle, |links| links.resolved_signature)
    }

    fn set_resolved_signature(&self, node: ast::Node, signature: Option<SignatureHandle>) {
        let handle = self.signature_link_handle(node);
        self.set_resolved_signature_by_handle(handle, signature);
    }

    fn set_resolved_signature_by_handle(
        &self,
        handle: core::LinkHandle<SignatureLinks>,
        signature: Option<SignatureHandle>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.resolved_signature = signature;
        });
    }

    fn replace_resolved_signature(
        &self,
        node: ast::Node,
        signature: Option<SignatureHandle>,
    ) -> Option<SignatureHandle> {
        let handle = self.signature_link_handle(node);
        self.replace_resolved_signature_by_handle(handle, signature)
    }

    fn replace_resolved_signature_by_handle(
        &self,
        handle: core::LinkHandle<SignatureLinks>,
        signature: Option<SignatureHandle>,
    ) -> Option<SignatureHandle> {
        self.with_by_handle_mut(handle, |links| {
            let previous = links.resolved_signature;
            links.resolved_signature = signature;
            previous
        })
    }

    fn effects_signature(&self, node: ast::Node) -> Option<SignatureHandle> {
        let handle = self.signature_link_handle(node);
        self.effects_signature_by_handle(handle)
    }

    fn effects_signature_by_handle(
        &self,
        handle: core::LinkHandle<SignatureLinks>,
    ) -> Option<SignatureHandle> {
        self.with_by_handle(handle, |links| links.effects_signature)
    }

    fn set_effects_signature(&self, node: ast::Node, signature: Option<SignatureHandle>) {
        let handle = self.signature_link_handle(node);
        self.set_effects_signature_by_handle(handle, signature);
    }

    fn set_effects_signature_by_handle(
        &self,
        handle: core::LinkHandle<SignatureLinks>,
        signature: Option<SignatureHandle>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.effects_signature = signature;
        });
    }

    fn decorator_signature(&self, node: ast::Node) -> Option<SignatureHandle> {
        let handle = self.signature_link_handle(node);
        self.decorator_signature_by_handle(handle)
    }

    fn decorator_signature_by_handle(
        &self,
        handle: core::LinkHandle<SignatureLinks>,
    ) -> Option<SignatureHandle> {
        self.with_by_handle(handle, |links| links.decorator_signature)
    }

    fn set_decorator_signature(&self, node: ast::Node, signature: Option<SignatureHandle>) {
        let handle = self.signature_link_handle(node);
        self.set_decorator_signature_by_handle(handle, signature);
    }

    fn set_decorator_signature_by_handle(
        &self,
        handle: core::LinkHandle<SignatureLinks>,
        signature: Option<SignatureHandle>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.decorator_signature = signature;
        });
    }
}

pub(crate) trait JsxElementLinksStoreExt {
    fn jsx_element_link_handle(&self, node: ast::Node) -> core::LinkHandle<JsxElementLinks>;
    fn jsx_element_resolved_attributes_type(&self, node: ast::Node) -> Option<TypeHandle>;
    fn jsx_element_resolved_attributes_type_by_handle(
        &self,
        handle: core::LinkHandle<JsxElementLinks>,
    ) -> Option<TypeHandle>;
    fn set_jsx_element_resolved_attributes_type(&self, node: ast::Node, resolved_type: TypeHandle);
    fn set_jsx_element_resolved_attributes_type_by_handle(
        &self,
        handle: core::LinkHandle<JsxElementLinks>,
        resolved_type: TypeHandle,
    );
    fn jsx_element_flags(&self, node: ast::Node) -> JsxFlags;
    fn jsx_element_flags_by_handle(&self, handle: core::LinkHandle<JsxElementLinks>) -> JsxFlags;
    fn add_jsx_element_flags(&self, node: ast::Node, flags: JsxFlags);
    fn add_jsx_element_flags_by_handle(
        &self,
        handle: core::LinkHandle<JsxElementLinks>,
        flags: JsxFlags,
    );
    fn jsx_element_namespace(&self, node: ast::Node) -> Option<SymbolIdentity>;
    fn jsx_element_namespace_by_handle(
        &self,
        handle: core::LinkHandle<JsxElementLinks>,
    ) -> Option<SymbolIdentity>;
    fn set_jsx_element_namespace(&self, node: ast::Node, namespace: SymbolIdentity);
    fn set_jsx_element_namespace_by_handle(
        &self,
        handle: core::LinkHandle<JsxElementLinks>,
        namespace: SymbolIdentity,
    );
    fn jsx_element_implicit_import_container(&self, node: ast::Node) -> Option<SymbolIdentity>;
    fn jsx_element_implicit_import_container_by_handle(
        &self,
        handle: core::LinkHandle<JsxElementLinks>,
    ) -> Option<SymbolIdentity>;
    fn set_jsx_element_implicit_import_container(&self, node: ast::Node, container: SymbolIdentity);
    fn set_jsx_element_implicit_import_container_by_handle(
        &self,
        handle: core::LinkHandle<JsxElementLinks>,
        container: SymbolIdentity,
    );
}

impl JsxElementLinksStoreExt for NodeLinkStore<JsxElementLinks> {
    fn jsx_element_link_handle(&self, node: ast::Node) -> core::LinkHandle<JsxElementLinks> {
        self.ensure_handle(node)
    }

    fn jsx_element_resolved_attributes_type(&self, node: ast::Node) -> Option<TypeHandle> {
        let handle = self.jsx_element_link_handle(node);
        self.jsx_element_resolved_attributes_type_by_handle(handle)
    }

    fn jsx_element_resolved_attributes_type_by_handle(
        &self,
        handle: core::LinkHandle<JsxElementLinks>,
    ) -> Option<TypeHandle> {
        self.with_by_handle(handle, |links| links.resolved_jsx_element_attributes_type)
    }

    fn set_jsx_element_resolved_attributes_type(&self, node: ast::Node, resolved_type: TypeHandle) {
        let handle = self.jsx_element_link_handle(node);
        self.set_jsx_element_resolved_attributes_type_by_handle(handle, resolved_type);
    }

    fn set_jsx_element_resolved_attributes_type_by_handle(
        &self,
        handle: core::LinkHandle<JsxElementLinks>,
        resolved_type: TypeHandle,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.resolved_jsx_element_attributes_type = Some(resolved_type);
        });
    }

    fn jsx_element_flags(&self, node: ast::Node) -> JsxFlags {
        let handle = self.jsx_element_link_handle(node);
        self.jsx_element_flags_by_handle(handle)
    }

    fn jsx_element_flags_by_handle(&self, handle: core::LinkHandle<JsxElementLinks>) -> JsxFlags {
        self.with_by_handle(handle, |links| links.jsx_flags)
    }

    fn add_jsx_element_flags(&self, node: ast::Node, flags: JsxFlags) {
        let handle = self.jsx_element_link_handle(node);
        self.add_jsx_element_flags_by_handle(handle, flags);
    }

    fn add_jsx_element_flags_by_handle(
        &self,
        handle: core::LinkHandle<JsxElementLinks>,
        flags: JsxFlags,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.jsx_flags |= flags;
        });
    }

    fn jsx_element_namespace(&self, node: ast::Node) -> Option<SymbolIdentity> {
        let handle = self.jsx_element_link_handle(node);
        self.jsx_element_namespace_by_handle(handle)
    }

    fn jsx_element_namespace_by_handle(
        &self,
        handle: core::LinkHandle<JsxElementLinks>,
    ) -> Option<SymbolIdentity> {
        self.with_by_handle(handle, |links| links.jsx_namespace)
    }

    fn set_jsx_element_namespace(&self, node: ast::Node, namespace: SymbolIdentity) {
        let handle = self.jsx_element_link_handle(node);
        self.set_jsx_element_namespace_by_handle(handle, namespace);
    }

    fn set_jsx_element_namespace_by_handle(
        &self,
        handle: core::LinkHandle<JsxElementLinks>,
        namespace: SymbolIdentity,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.jsx_namespace = Some(namespace);
        });
    }

    fn jsx_element_implicit_import_container(&self, node: ast::Node) -> Option<SymbolIdentity> {
        let handle = self.jsx_element_link_handle(node);
        self.jsx_element_implicit_import_container_by_handle(handle)
    }

    fn jsx_element_implicit_import_container_by_handle(
        &self,
        handle: core::LinkHandle<JsxElementLinks>,
    ) -> Option<SymbolIdentity> {
        self.with_by_handle(handle, |links| links.jsx_implicit_import_container)
    }

    fn set_jsx_element_implicit_import_container(
        &self,
        node: ast::Node,
        container: SymbolIdentity,
    ) {
        let handle = self.jsx_element_link_handle(node);
        self.set_jsx_element_implicit_import_container_by_handle(handle, container);
    }

    fn set_jsx_element_implicit_import_container_by_handle(
        &self,
        handle: core::LinkHandle<JsxElementLinks>,
        container: SymbolIdentity,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.jsx_implicit_import_container = Some(container);
        });
    }
}

pub(crate) trait SwitchStatementLinksStoreExt {
    fn switch_statement_link_handle(
        &self,
        node: ast::Node,
    ) -> core::LinkHandle<SwitchStatementLinks>;
    fn exhaustive_state(&self, node: ast::Node) -> ExhaustiveState;
    fn exhaustive_state_by_handle(
        &self,
        handle: core::LinkHandle<SwitchStatementLinks>,
    ) -> ExhaustiveState;
    fn set_exhaustive_state(&self, node: ast::Node, state: ExhaustiveState);
    fn set_exhaustive_state_by_handle(
        &self,
        handle: core::LinkHandle<SwitchStatementLinks>,
        state: ExhaustiveState,
    );
    fn mark_exhaustive_computing(&self, node: ast::Node);
    fn mark_exhaustive_computing_by_handle(&self, handle: core::LinkHandle<SwitchStatementLinks>);
    fn mark_exhaustive_false(&self, node: ast::Node);
    fn mark_exhaustive_false_by_handle(&self, handle: core::LinkHandle<SwitchStatementLinks>);
    fn mark_exhaustive_true(&self, node: ast::Node);
    fn mark_exhaustive_true_by_handle(&self, handle: core::LinkHandle<SwitchStatementLinks>);
    fn set_exhaustive_result_if_computing(&self, node: ast::Node, is_exhaustive: bool);
    fn set_exhaustive_result_if_computing_by_handle(
        &self,
        handle: core::LinkHandle<SwitchStatementLinks>,
        is_exhaustive: bool,
    );
    fn switch_types_state(&self, node: ast::Node) -> (bool, Vec<TypeHandle>);
    fn switch_types_state_by_handle(
        &self,
        handle: core::LinkHandle<SwitchStatementLinks>,
    ) -> (bool, Vec<TypeHandle>);
    fn set_switch_types(&self, node: ast::Node, switch_types: Vec<TypeHandle>);
    fn set_switch_types_by_handle(
        &self,
        handle: core::LinkHandle<SwitchStatementLinks>,
        switch_types: Vec<TypeHandle>,
    );
    fn witnesses_state(&self, node: ast::Node) -> (bool, Option<Vec<String>>);
    fn witnesses_state_by_handle(
        &self,
        handle: core::LinkHandle<SwitchStatementLinks>,
    ) -> (bool, Option<Vec<String>>);
    fn set_witnesses(&self, node: ast::Node, witnesses: Option<Vec<String>>);
    fn set_witnesses_by_handle(
        &self,
        handle: core::LinkHandle<SwitchStatementLinks>,
        witnesses: Option<Vec<String>>,
    );
}

impl SwitchStatementLinksStoreExt for NodeLinkStore<SwitchStatementLinks> {
    fn switch_statement_link_handle(
        &self,
        node: ast::Node,
    ) -> core::LinkHandle<SwitchStatementLinks> {
        self.ensure_handle(node)
    }

    fn exhaustive_state(&self, node: ast::Node) -> ExhaustiveState {
        let handle = self.switch_statement_link_handle(node);
        self.exhaustive_state_by_handle(handle)
    }

    fn exhaustive_state_by_handle(
        &self,
        handle: core::LinkHandle<SwitchStatementLinks>,
    ) -> ExhaustiveState {
        self.with_by_handle(handle, |links| links.exhaustive_state)
    }

    fn set_exhaustive_state(&self, node: ast::Node, state: ExhaustiveState) {
        let handle = self.switch_statement_link_handle(node);
        self.set_exhaustive_state_by_handle(handle, state);
    }

    fn set_exhaustive_state_by_handle(
        &self,
        handle: core::LinkHandle<SwitchStatementLinks>,
        state: ExhaustiveState,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.exhaustive_state = state;
        });
    }

    fn mark_exhaustive_computing(&self, node: ast::Node) {
        let handle = self.switch_statement_link_handle(node);
        self.mark_exhaustive_computing_by_handle(handle);
    }

    fn mark_exhaustive_computing_by_handle(&self, handle: core::LinkHandle<SwitchStatementLinks>) {
        self.set_exhaustive_state_by_handle(handle, EXHAUSTIVE_STATE_COMPUTING);
    }

    fn mark_exhaustive_false(&self, node: ast::Node) {
        let handle = self.switch_statement_link_handle(node);
        self.mark_exhaustive_false_by_handle(handle);
    }

    fn mark_exhaustive_false_by_handle(&self, handle: core::LinkHandle<SwitchStatementLinks>) {
        self.set_exhaustive_state_by_handle(handle, EXHAUSTIVE_STATE_FALSE);
    }

    fn mark_exhaustive_true(&self, node: ast::Node) {
        let handle = self.switch_statement_link_handle(node);
        self.mark_exhaustive_true_by_handle(handle);
    }

    fn mark_exhaustive_true_by_handle(&self, handle: core::LinkHandle<SwitchStatementLinks>) {
        self.set_exhaustive_state_by_handle(handle, EXHAUSTIVE_STATE_TRUE);
    }

    fn set_exhaustive_result_if_computing(&self, node: ast::Node, is_exhaustive: bool) {
        let handle = self.switch_statement_link_handle(node);
        self.set_exhaustive_result_if_computing_by_handle(handle, is_exhaustive);
    }

    fn set_exhaustive_result_if_computing_by_handle(
        &self,
        handle: core::LinkHandle<SwitchStatementLinks>,
        is_exhaustive: bool,
    ) {
        self.with_by_handle_mut(handle, |links| {
            if links.exhaustive_state == EXHAUSTIVE_STATE_COMPUTING {
                links.exhaustive_state =
                    core::if_else(is_exhaustive, EXHAUSTIVE_STATE_TRUE, EXHAUSTIVE_STATE_FALSE);
            }
        });
    }

    fn switch_types_state(&self, node: ast::Node) -> (bool, Vec<TypeHandle>) {
        let handle = self.switch_statement_link_handle(node);
        self.switch_types_state_by_handle(handle)
    }

    fn switch_types_state_by_handle(
        &self,
        handle: core::LinkHandle<SwitchStatementLinks>,
    ) -> (bool, Vec<TypeHandle>) {
        self.with_by_handle(handle, |links| {
            (links.switch_types_computed, links.switch_types.clone())
        })
    }

    fn set_switch_types(&self, node: ast::Node, switch_types: Vec<TypeHandle>) {
        let handle = self.switch_statement_link_handle(node);
        self.set_switch_types_by_handle(handle, switch_types);
    }

    fn set_switch_types_by_handle(
        &self,
        handle: core::LinkHandle<SwitchStatementLinks>,
        switch_types: Vec<TypeHandle>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.switch_types = switch_types;
            links.switch_types_computed = true;
        });
    }

    fn witnesses_state(&self, node: ast::Node) -> (bool, Option<Vec<String>>) {
        let handle = self.switch_statement_link_handle(node);
        self.witnesses_state_by_handle(handle)
    }

    fn witnesses_state_by_handle(
        &self,
        handle: core::LinkHandle<SwitchStatementLinks>,
    ) -> (bool, Option<Vec<String>>) {
        self.with_by_handle(handle, |links| {
            (links.witnesses_computed, links.witnesses.clone())
        })
    }

    fn set_witnesses(&self, node: ast::Node, witnesses: Option<Vec<String>>) {
        let handle = self.switch_statement_link_handle(node);
        self.set_witnesses_by_handle(handle, witnesses);
    }

    fn set_witnesses_by_handle(
        &self,
        handle: core::LinkHandle<SwitchStatementLinks>,
        witnesses: Option<Vec<String>>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.witnesses = witnesses;
            links.witnesses_computed = true;
        });
    }
}

pub(crate) trait ArrayLiteralLinksStoreExt {
    fn array_literal_link_handle(&self, node: ast::Node) -> core::LinkHandle<ArrayLiteralLinks>;
    fn array_literal_indices_computed(&self, node: ast::Node) -> bool;
    fn array_literal_indices_computed_by_handle(
        &self,
        handle: core::LinkHandle<ArrayLiteralLinks>,
    ) -> bool;
    fn set_array_literal_indices_computed(&self, node: ast::Node, computed: bool);
    fn set_array_literal_indices_computed_by_handle(
        &self,
        handle: core::LinkHandle<ArrayLiteralLinks>,
        computed: bool,
    );
    fn array_literal_spread_indices(&self, node: ast::Node) -> (isize, isize);
    fn array_literal_spread_indices_by_handle(
        &self,
        handle: core::LinkHandle<ArrayLiteralLinks>,
    ) -> (isize, isize);
    fn set_array_literal_spread_indices(
        &self,
        node: ast::Node,
        first_spread_index: isize,
        last_spread_index: isize,
    );
    fn set_array_literal_spread_indices_by_handle(
        &self,
        handle: core::LinkHandle<ArrayLiteralLinks>,
        first_spread_index: isize,
        last_spread_index: isize,
    );
}

impl ArrayLiteralLinksStoreExt for NodeLinkStore<ArrayLiteralLinks> {
    fn array_literal_link_handle(&self, node: ast::Node) -> core::LinkHandle<ArrayLiteralLinks> {
        self.ensure_handle(node)
    }

    fn array_literal_indices_computed(&self, node: ast::Node) -> bool {
        let handle = self.array_literal_link_handle(node);
        self.array_literal_indices_computed_by_handle(handle)
    }

    fn array_literal_indices_computed_by_handle(
        &self,
        handle: core::LinkHandle<ArrayLiteralLinks>,
    ) -> bool {
        self.with_by_handle(handle, |links| links.indices_computed)
    }

    fn set_array_literal_indices_computed(&self, node: ast::Node, computed: bool) {
        let handle = self.array_literal_link_handle(node);
        self.set_array_literal_indices_computed_by_handle(handle, computed);
    }

    fn set_array_literal_indices_computed_by_handle(
        &self,
        handle: core::LinkHandle<ArrayLiteralLinks>,
        computed: bool,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.indices_computed = computed;
        });
    }

    fn array_literal_spread_indices(&self, node: ast::Node) -> (isize, isize) {
        let handle = self.array_literal_link_handle(node);
        self.array_literal_spread_indices_by_handle(handle)
    }

    fn array_literal_spread_indices_by_handle(
        &self,
        handle: core::LinkHandle<ArrayLiteralLinks>,
    ) -> (isize, isize) {
        self.with_by_handle(handle, |links| {
            (links.first_spread_index, links.last_spread_index)
        })
    }

    fn set_array_literal_spread_indices(
        &self,
        node: ast::Node,
        first_spread_index: isize,
        last_spread_index: isize,
    ) {
        let handle = self.array_literal_link_handle(node);
        self.set_array_literal_spread_indices_by_handle(
            handle,
            first_spread_index,
            last_spread_index,
        );
    }

    fn set_array_literal_spread_indices_by_handle(
        &self,
        handle: core::LinkHandle<ArrayLiteralLinks>,
        first_spread_index: isize,
        last_spread_index: isize,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.first_spread_index = first_spread_index;
            links.last_spread_index = last_spread_index;
        });
    }
}

pub(crate) trait EnumMemberLinksStoreExt {
    fn enum_member_link_handle(&self, node: ast::Node) -> core::LinkHandle<EnumMemberLinks>;
    fn enum_member_value(&self, node: ast::Node) -> evaluator::Result;
    fn enum_member_value_by_handle(
        &self,
        handle: core::LinkHandle<EnumMemberLinks>,
    ) -> evaluator::Result;
    fn set_enum_member_value(&self, node: ast::Node, value: evaluator::Result);
    fn set_enum_member_value_by_handle(
        &self,
        handle: core::LinkHandle<EnumMemberLinks>,
        value: evaluator::Result,
    );
}

impl EnumMemberLinksStoreExt for NodeLinkStore<EnumMemberLinks> {
    fn enum_member_link_handle(&self, node: ast::Node) -> core::LinkHandle<EnumMemberLinks> {
        self.ensure_handle(node)
    }

    fn enum_member_value(&self, node: ast::Node) -> evaluator::Result {
        let handle = self.enum_member_link_handle(node);
        self.enum_member_value_by_handle(handle)
    }

    fn enum_member_value_by_handle(
        &self,
        handle: core::LinkHandle<EnumMemberLinks>,
    ) -> evaluator::Result {
        self.with_by_handle(handle, |links| links.value.clone())
    }

    fn set_enum_member_value(&self, node: ast::Node, value: evaluator::Result) {
        let handle = self.enum_member_link_handle(node);
        self.set_enum_member_value_by_handle(handle, value);
    }

    fn set_enum_member_value_by_handle(
        &self,
        handle: core::LinkHandle<EnumMemberLinks>,
        value: evaluator::Result,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.value = value;
        });
    }
}

pub(crate) trait DeclarationLinksStoreExt {
    fn declaration_link_handle(&self, node: ast::Node) -> core::LinkHandle<DeclarationLinks>;
    fn declaration_is_visible(&self, node: ast::Node) -> core::Tristate;
    fn declaration_is_visible_by_handle(
        &self,
        handle: core::LinkHandle<DeclarationLinks>,
    ) -> core::Tristate;
    fn set_declaration_is_visible(&self, node: ast::Node, is_visible: core::Tristate);
    fn set_declaration_is_visible_by_handle(
        &self,
        handle: core::LinkHandle<DeclarationLinks>,
        is_visible: core::Tristate,
    );
}

impl DeclarationLinksStoreExt for NodeLinkStore<DeclarationLinks> {
    fn declaration_link_handle(&self, node: ast::Node) -> core::LinkHandle<DeclarationLinks> {
        self.ensure_handle(node)
    }

    fn declaration_is_visible(&self, node: ast::Node) -> core::Tristate {
        let handle = self.declaration_link_handle(node);
        self.declaration_is_visible_by_handle(handle)
    }

    fn declaration_is_visible_by_handle(
        &self,
        handle: core::LinkHandle<DeclarationLinks>,
    ) -> core::Tristate {
        self.with_by_handle(handle, |links| links.is_visible())
    }

    fn set_declaration_is_visible(&self, node: ast::Node, is_visible: core::Tristate) {
        let handle = self.declaration_link_handle(node);
        self.set_declaration_is_visible_by_handle(handle, is_visible);
    }

    fn set_declaration_is_visible_by_handle(
        &self,
        handle: core::LinkHandle<DeclarationLinks>,
        is_visible: core::Tristate,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.set_is_visible(is_visible);
        });
    }
}

pub(crate) trait DeclarationFileLinksStoreExt {
    fn declaration_file_link_handle<Q>(
        &self,
        source_file: Q,
    ) -> core::LinkHandle<DeclarationFileLinks>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn declaration_file_aliases_marked<Q>(&self, source_file: Q) -> bool
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn declaration_file_aliases_marked_by_handle(
        &self,
        handle: core::LinkHandle<DeclarationFileLinks>,
    ) -> bool;
    fn set_declaration_file_aliases_marked<Q>(&self, source_file: Q, aliases_marked: bool)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn set_declaration_file_aliases_marked_by_handle(
        &self,
        handle: core::LinkHandle<DeclarationFileLinks>,
        aliases_marked: bool,
    );
}

impl DeclarationFileLinksStoreExt for SourceFileLinkStore<DeclarationFileLinks> {
    fn declaration_file_link_handle<Q>(
        &self,
        source_file: Q,
    ) -> core::LinkHandle<DeclarationFileLinks>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        self.ensure_handle(source_file)
    }

    fn declaration_file_aliases_marked<Q>(&self, source_file: Q) -> bool
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.declaration_file_link_handle(source_file);
        self.declaration_file_aliases_marked_by_handle(handle)
    }

    fn declaration_file_aliases_marked_by_handle(
        &self,
        handle: core::LinkHandle<DeclarationFileLinks>,
    ) -> bool {
        self.with_by_handle(handle, |links| links.aliases_marked())
    }

    fn set_declaration_file_aliases_marked<Q>(&self, source_file: Q, aliases_marked: bool)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.declaration_file_link_handle(source_file);
        self.set_declaration_file_aliases_marked_by_handle(handle, aliases_marked);
    }

    fn set_declaration_file_aliases_marked_by_handle(
        &self,
        handle: core::LinkHandle<DeclarationFileLinks>,
        aliases_marked: bool,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.set_aliases_marked(aliases_marked);
        });
    }
}

pub(crate) trait SymbolReferenceLinksStoreExt {
    fn symbol_reference_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<SymbolReferenceLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn symbol_reference_kinds<Q>(&self, symbol: Q) -> ast::SymbolFlags
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn symbol_reference_kinds_by_handle(
        &self,
        handle: core::LinkHandle<SymbolReferenceLinks>,
    ) -> ast::SymbolFlags;
    fn set_symbol_reference_kinds<Q>(&self, symbol: Q, kinds: ast::SymbolFlags)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_symbol_reference_kinds_by_handle(
        &self,
        handle: core::LinkHandle<SymbolReferenceLinks>,
        kinds: ast::SymbolFlags,
    );
    fn add_symbol_reference_kinds<Q>(&self, symbol: Q, kinds: ast::SymbolFlags)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn add_symbol_reference_kinds_by_handle(
        &self,
        handle: core::LinkHandle<SymbolReferenceLinks>,
        kinds: ast::SymbolFlags,
    );
    fn has_symbol_reference_kinds<Q>(&self, symbol: Q, kinds: ast::SymbolFlags) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn has_symbol_reference_kinds_by_handle(
        &self,
        handle: core::LinkHandle<SymbolReferenceLinks>,
        kinds: ast::SymbolFlags,
    ) -> bool;
}

impl SymbolReferenceLinksStoreExt for SymbolLinkStore<SymbolReferenceLinks> {
    fn symbol_reference_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<SymbolReferenceLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.ensure_handle(symbol)
    }

    fn symbol_reference_kinds<Q>(&self, symbol: Q) -> ast::SymbolFlags
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.symbol_reference_link_handle(symbol);
        self.symbol_reference_kinds_by_handle(handle)
    }

    fn symbol_reference_kinds_by_handle(
        &self,
        handle: core::LinkHandle<SymbolReferenceLinks>,
    ) -> ast::SymbolFlags {
        self.with_by_handle(handle, |links| links.reference_kinds)
    }

    fn set_symbol_reference_kinds<Q>(&self, symbol: Q, kinds: ast::SymbolFlags)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.symbol_reference_link_handle(symbol);
        self.set_symbol_reference_kinds_by_handle(handle, kinds);
    }

    fn set_symbol_reference_kinds_by_handle(
        &self,
        handle: core::LinkHandle<SymbolReferenceLinks>,
        kinds: ast::SymbolFlags,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.reference_kinds = kinds;
        });
    }

    fn add_symbol_reference_kinds<Q>(&self, symbol: Q, kinds: ast::SymbolFlags)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.symbol_reference_link_handle(symbol);
        self.add_symbol_reference_kinds_by_handle(handle, kinds);
    }

    fn add_symbol_reference_kinds_by_handle(
        &self,
        handle: core::LinkHandle<SymbolReferenceLinks>,
        kinds: ast::SymbolFlags,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.reference_kinds |= kinds;
        });
    }

    fn has_symbol_reference_kinds<Q>(&self, symbol: Q, kinds: ast::SymbolFlags) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.symbol_reference_link_handle(symbol);
        self.has_symbol_reference_kinds_by_handle(handle, kinds)
    }

    fn has_symbol_reference_kinds_by_handle(
        &self,
        handle: core::LinkHandle<SymbolReferenceLinks>,
        kinds: ast::SymbolFlags,
    ) -> bool {
        self.symbol_reference_kinds_by_handle(handle) & kinds != 0
    }
}

pub(crate) trait ModuleSymbolLinksStoreExt {
    fn module_symbol_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<ModuleSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn module_resolved_exports_is_resolved<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn module_resolved_exports_is_resolved_by_handle(
        &self,
        handle: core::LinkHandle<ModuleSymbolLinks>,
    ) -> bool;
    fn with_module_resolved_exports<Q, R>(
        &self,
        symbol: Q,
        f: impl FnOnce(Option<&SymbolIdentityTable>) -> R,
    ) -> R
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn with_module_resolved_exports_by_handle<R>(
        &self,
        handle: core::LinkHandle<ModuleSymbolLinks>,
        f: impl FnOnce(Option<&SymbolIdentityTable>) -> R,
    ) -> R;
    fn set_module_resolved_exports<Q>(
        &self,
        symbol: Q,
        exports: SymbolIdentityTable,
        type_only_export_star_map: Option<HashMap<String, ast::Node>>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_module_resolved_exports_by_handle(
        &self,
        handle: core::LinkHandle<ModuleSymbolLinks>,
        exports: SymbolIdentityTable,
        type_only_export_star_map: Option<HashMap<String, ast::Node>>,
    );
    fn module_type_only_export_star_declaration<Q>(
        &self,
        symbol: Q,
        name: &str,
    ) -> Option<ast::Node>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn module_type_only_export_star_declaration_by_handle(
        &self,
        handle: core::LinkHandle<ModuleSymbolLinks>,
        name: &str,
    ) -> Option<ast::Node>;
    fn module_exports_checked<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn module_exports_checked_by_handle(&self, handle: core::LinkHandle<ModuleSymbolLinks>)
    -> bool;
    fn set_module_exports_checked<Q>(&self, symbol: Q, exports_checked: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_module_exports_checked_by_handle(
        &self,
        handle: core::LinkHandle<ModuleSymbolLinks>,
        exports_checked: bool,
    );
    fn mark_module_exports_checked<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn mark_module_exports_checked_by_handle(&self, handle: core::LinkHandle<ModuleSymbolLinks>);
}

impl ModuleSymbolLinksStoreExt for SymbolLinkStore<ModuleSymbolLinks> {
    fn module_symbol_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<ModuleSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.ensure_handle(symbol)
    }

    fn module_resolved_exports_is_resolved<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.module_symbol_link_handle(symbol);
        self.module_resolved_exports_is_resolved_by_handle(handle)
    }

    fn module_resolved_exports_is_resolved_by_handle(
        &self,
        handle: core::LinkHandle<ModuleSymbolLinks>,
    ) -> bool {
        self.with_by_handle(handle, |links| links.resolved_exports.is_some())
    }

    fn with_module_resolved_exports<Q, R>(
        &self,
        symbol: Q,
        f: impl FnOnce(Option<&SymbolIdentityTable>) -> R,
    ) -> R
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.module_symbol_link_handle(symbol);
        self.with_module_resolved_exports_by_handle(handle, f)
    }

    fn with_module_resolved_exports_by_handle<R>(
        &self,
        handle: core::LinkHandle<ModuleSymbolLinks>,
        f: impl FnOnce(Option<&SymbolIdentityTable>) -> R,
    ) -> R {
        self.with_by_handle(handle, |links| f(links.resolved_exports.as_ref()))
    }

    fn set_module_resolved_exports<Q>(
        &self,
        symbol: Q,
        exports: SymbolIdentityTable,
        type_only_export_star_map: Option<HashMap<String, ast::Node>>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.module_symbol_link_handle(symbol);
        self.set_module_resolved_exports_by_handle(handle, exports, type_only_export_star_map);
    }

    fn set_module_resolved_exports_by_handle(
        &self,
        handle: core::LinkHandle<ModuleSymbolLinks>,
        exports: SymbolIdentityTable,
        type_only_export_star_map: Option<HashMap<String, ast::Node>>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.resolved_exports = Some(exports);
            links.type_only_export_star_map = type_only_export_star_map.unwrap_or_default();
        });
    }

    fn module_type_only_export_star_declaration<Q>(
        &self,
        symbol: Q,
        name: &str,
    ) -> Option<ast::Node>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.module_symbol_link_handle(symbol);
        self.module_type_only_export_star_declaration_by_handle(handle, name)
    }

    fn module_type_only_export_star_declaration_by_handle(
        &self,
        handle: core::LinkHandle<ModuleSymbolLinks>,
        name: &str,
    ) -> Option<ast::Node> {
        self.with_by_handle(handle, |links| {
            links.type_only_export_star_map.get(name).copied()
        })
    }

    fn module_exports_checked<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.module_symbol_link_handle(symbol);
        self.module_exports_checked_by_handle(handle)
    }

    fn module_exports_checked_by_handle(
        &self,
        handle: core::LinkHandle<ModuleSymbolLinks>,
    ) -> bool {
        self.with_by_handle(handle, |links| links.exports_checked)
    }

    fn set_module_exports_checked<Q>(&self, symbol: Q, exports_checked: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.module_symbol_link_handle(symbol);
        self.set_module_exports_checked_by_handle(handle, exports_checked);
    }

    fn set_module_exports_checked_by_handle(
        &self,
        handle: core::LinkHandle<ModuleSymbolLinks>,
        exports_checked: bool,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.exports_checked = exports_checked;
        });
    }

    fn mark_module_exports_checked<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.module_symbol_link_handle(symbol);
        self.mark_module_exports_checked_by_handle(handle);
    }

    fn mark_module_exports_checked_by_handle(&self, handle: core::LinkHandle<ModuleSymbolLinks>) {
        self.set_module_exports_checked_by_handle(handle, true);
    }
}

pub(crate) trait MembersAndExportsLinksStoreExt {
    fn members_and_exports_link_handle<Q>(
        &self,
        symbol: Q,
    ) -> core::LinkHandle<MembersAndExportsLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn clear_resolved_members_and_exports<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn clear_resolved_members_and_exports_by_handle(
        &self,
        handle: core::LinkHandle<MembersAndExportsLinks>,
    );
    fn members_or_exports_slot_is_resolved<Q>(
        &self,
        symbol: Q,
        resolution_kind: MembersOrExportsResolutionKind,
    ) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn members_or_exports_slot_is_resolved_by_handle(
        &self,
        handle: core::LinkHandle<MembersAndExportsLinks>,
        resolution_kind: MembersOrExportsResolutionKind,
    ) -> bool;
    fn with_resolved_members_or_exports<Q, R>(
        &self,
        symbol: Q,
        resolution_kind: MembersOrExportsResolutionKind,
        f: impl FnOnce(Option<&SymbolIdentityTable>) -> R,
    ) -> R
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn with_resolved_members_or_exports_by_handle<R>(
        &self,
        handle: core::LinkHandle<MembersAndExportsLinks>,
        resolution_kind: MembersOrExportsResolutionKind,
        f: impl FnOnce(Option<&SymbolIdentityTable>) -> R,
    ) -> R;
    fn set_resolved_members_or_exports<Q>(
        &self,
        symbol: Q,
        resolution_kind: MembersOrExportsResolutionKind,
        table: SymbolIdentityTable,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_resolved_members_or_exports_by_handle(
        &self,
        handle: core::LinkHandle<MembersAndExportsLinks>,
        resolution_kind: MembersOrExportsResolutionKind,
        table: SymbolIdentityTable,
    );
}

impl MembersAndExportsLinksStoreExt for SymbolLinkStore<MembersAndExportsLinks> {
    fn members_and_exports_link_handle<Q>(
        &self,
        symbol: Q,
    ) -> core::LinkHandle<MembersAndExportsLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.ensure_handle(symbol)
    }

    fn clear_resolved_members_and_exports<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.members_and_exports_link_handle(symbol);
        self.clear_resolved_members_and_exports_by_handle(handle);
    }

    fn clear_resolved_members_and_exports_by_handle(
        &self,
        handle: core::LinkHandle<MembersAndExportsLinks>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            *links = Default::default();
        });
    }

    fn members_or_exports_slot_is_resolved<Q>(
        &self,
        symbol: Q,
        resolution_kind: MembersOrExportsResolutionKind,
    ) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.members_and_exports_link_handle(symbol);
        self.members_or_exports_slot_is_resolved_by_handle(handle, resolution_kind)
    }

    fn members_or_exports_slot_is_resolved_by_handle(
        &self,
        handle: core::LinkHandle<MembersAndExportsLinks>,
        resolution_kind: MembersOrExportsResolutionKind,
    ) -> bool {
        self.with_by_handle(handle, |links| links[resolution_kind as usize].is_some())
    }

    fn with_resolved_members_or_exports<Q, R>(
        &self,
        symbol: Q,
        resolution_kind: MembersOrExportsResolutionKind,
        f: impl FnOnce(Option<&SymbolIdentityTable>) -> R,
    ) -> R
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.members_and_exports_link_handle(symbol);
        self.with_resolved_members_or_exports_by_handle(handle, resolution_kind, f)
    }

    fn with_resolved_members_or_exports_by_handle<R>(
        &self,
        handle: core::LinkHandle<MembersAndExportsLinks>,
        resolution_kind: MembersOrExportsResolutionKind,
        f: impl FnOnce(Option<&SymbolIdentityTable>) -> R,
    ) -> R {
        self.with_by_handle(handle, |links| f(links[resolution_kind as usize].as_ref()))
    }

    fn set_resolved_members_or_exports<Q>(
        &self,
        symbol: Q,
        resolution_kind: MembersOrExportsResolutionKind,
        table: SymbolIdentityTable,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.members_and_exports_link_handle(symbol);
        self.set_resolved_members_or_exports_by_handle(handle, resolution_kind, table);
    }

    fn set_resolved_members_or_exports_by_handle(
        &self,
        handle: core::LinkHandle<MembersAndExportsLinks>,
        resolution_kind: MembersOrExportsResolutionKind,
        table: SymbolIdentityTable,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links[resolution_kind as usize] = Some(table);
        });
    }
}

pub(crate) trait LateBoundLinksStoreExt {
    fn late_bound_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<LateBoundLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn late_bound_symbol<Q>(&self, symbol: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn late_bound_symbol_by_handle(
        &self,
        handle: core::LinkHandle<LateBoundLinks>,
    ) -> Option<SymbolIdentity>;
    fn set_late_bound_symbol<Q>(&self, symbol: Q, late_symbol: Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_late_bound_symbol_by_handle(
        &self,
        handle: core::LinkHandle<LateBoundLinks>,
        late_symbol: Option<SymbolIdentity>,
    );
}

impl LateBoundLinksStoreExt for SymbolLinkStore<LateBoundLinks> {
    fn late_bound_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<LateBoundLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.ensure_handle(symbol)
    }

    fn late_bound_symbol<Q>(&self, symbol: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.late_bound_link_handle(symbol);
        self.late_bound_symbol_by_handle(handle)
    }

    fn late_bound_symbol_by_handle(
        &self,
        handle: core::LinkHandle<LateBoundLinks>,
    ) -> Option<SymbolIdentity> {
        self.with_by_handle(handle, |links| links.late_symbol)
    }

    fn set_late_bound_symbol<Q>(&self, symbol: Q, late_symbol: Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.late_bound_link_handle(symbol);
        self.set_late_bound_symbol_by_handle(handle, late_symbol);
    }

    fn set_late_bound_symbol_by_handle(
        &self,
        handle: core::LinkHandle<LateBoundLinks>,
        late_symbol: Option<SymbolIdentity>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.late_symbol = late_symbol;
        });
    }
}

pub(crate) trait ExportTypeLinksStoreExt {
    fn export_type_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<ExportTypeLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn export_type_target<Q>(&self, symbol: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn export_type_target_by_handle(
        &self,
        handle: core::LinkHandle<ExportTypeLinks>,
    ) -> Option<SymbolIdentity>;
    fn set_export_type_target<Q>(&self, symbol: Q, target: Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_export_type_target_by_handle(
        &self,
        handle: core::LinkHandle<ExportTypeLinks>,
        target: Option<SymbolIdentity>,
    );
    fn export_type_originating_import<Q>(&self, symbol: Q) -> Option<ast::Node>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn export_type_originating_import_by_handle(
        &self,
        handle: core::LinkHandle<ExportTypeLinks>,
    ) -> Option<ast::Node>;
    fn set_export_type_originating_import<Q>(
        &self,
        symbol: Q,
        originating_import: Option<ast::Node>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_export_type_originating_import_by_handle(
        &self,
        handle: core::LinkHandle<ExportTypeLinks>,
        originating_import: Option<ast::Node>,
    );
}

impl ExportTypeLinksStoreExt for SymbolLinkStore<ExportTypeLinks> {
    fn export_type_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<ExportTypeLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.ensure_handle(symbol)
    }

    fn export_type_target<Q>(&self, symbol: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.export_type_link_handle(symbol);
        self.export_type_target_by_handle(handle)
    }

    fn export_type_target_by_handle(
        &self,
        handle: core::LinkHandle<ExportTypeLinks>,
    ) -> Option<SymbolIdentity> {
        self.with_by_handle(handle, |links| links.target)
    }

    fn set_export_type_target<Q>(&self, symbol: Q, target: Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.export_type_link_handle(symbol);
        self.set_export_type_target_by_handle(handle, target);
    }

    fn set_export_type_target_by_handle(
        &self,
        handle: core::LinkHandle<ExportTypeLinks>,
        target: Option<SymbolIdentity>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.target = target;
        });
    }

    fn export_type_originating_import<Q>(&self, symbol: Q) -> Option<ast::Node>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.export_type_link_handle(symbol);
        self.export_type_originating_import_by_handle(handle)
    }

    fn export_type_originating_import_by_handle(
        &self,
        handle: core::LinkHandle<ExportTypeLinks>,
    ) -> Option<ast::Node> {
        self.with_by_handle(handle, |links| links.originating_import)
    }

    fn set_export_type_originating_import<Q>(
        &self,
        symbol: Q,
        originating_import: Option<ast::Node>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.export_type_link_handle(symbol);
        self.set_export_type_originating_import_by_handle(handle, originating_import);
    }

    fn set_export_type_originating_import_by_handle(
        &self,
        handle: core::LinkHandle<ExportTypeLinks>,
        originating_import: Option<ast::Node>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.originating_import = originating_import;
        });
    }
}

pub(crate) trait TypeAliasLinksStoreExt {
    fn type_alias_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<TypeAliasLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn type_alias_declared_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn type_alias_declared_type_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
    ) -> Option<TypeHandle>;
    fn set_type_alias_declared_type<Q>(&self, symbol: Q, declared_type: Option<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_type_alias_declared_type_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
        declared_type: Option<TypeHandle>,
    );
    fn type_alias_type_parameters<Q>(&self, symbol: Q) -> Vec<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn type_alias_type_parameters_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
    ) -> Vec<TypeHandle>;
    fn set_type_alias_type_parameters<Q>(&self, symbol: Q, type_parameters: Vec<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_type_alias_type_parameters_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
        type_parameters: Vec<TypeHandle>,
    );
    fn type_alias_type_parameter_count<Q>(&self, symbol: Q) -> usize
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn type_alias_type_parameter_count_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
    ) -> usize;
    fn type_alias_has_type_parameters<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn type_alias_has_type_parameters_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
    ) -> bool;
    fn type_alias_instantiation<Q>(&self, symbol: Q, key: CacheHashKey) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn type_alias_instantiation_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
        key: CacheHashKey,
    ) -> Option<TypeHandle>;
    fn set_type_alias_instantiations<Q>(
        &self,
        symbol: Q,
        instantiations: HashMap<CacheHashKey, TypeHandle>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_type_alias_instantiations_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
        instantiations: HashMap<CacheHashKey, TypeHandle>,
    );
    fn insert_type_alias_instantiation<Q>(&self, symbol: Q, key: CacheHashKey, ty: TypeHandle)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn insert_type_alias_instantiation_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
        key: CacheHashKey,
        ty: TypeHandle,
    );
    fn type_alias_is_constructor_declared_property<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn type_alias_is_constructor_declared_property_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
    ) -> bool;
    fn set_type_alias_is_constructor_declared_property<Q>(&self, symbol: Q, value: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_type_alias_is_constructor_declared_property_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
        value: bool,
    );
}

impl TypeAliasLinksStoreExt for SymbolLinkStore<TypeAliasLinks> {
    fn type_alias_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<TypeAliasLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.ensure_handle(symbol)
    }

    fn type_alias_declared_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.type_alias_link_handle(symbol);
        self.type_alias_declared_type_by_handle(handle)
    }

    fn type_alias_declared_type_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
    ) -> Option<TypeHandle> {
        self.with_by_handle(handle, |links| links.declared_type)
    }

    fn set_type_alias_declared_type<Q>(&self, symbol: Q, declared_type: Option<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.type_alias_link_handle(symbol);
        self.set_type_alias_declared_type_by_handle(handle, declared_type);
    }

    fn set_type_alias_declared_type_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
        declared_type: Option<TypeHandle>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.declared_type = declared_type;
        });
    }

    fn type_alias_type_parameters<Q>(&self, symbol: Q) -> Vec<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.type_alias_link_handle(symbol);
        self.type_alias_type_parameters_by_handle(handle)
    }

    fn type_alias_type_parameters_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
    ) -> Vec<TypeHandle> {
        self.with_by_handle(handle, |links| links.type_parameters.clone())
    }

    fn set_type_alias_type_parameters<Q>(&self, symbol: Q, type_parameters: Vec<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.type_alias_link_handle(symbol);
        self.set_type_alias_type_parameters_by_handle(handle, type_parameters);
    }

    fn set_type_alias_type_parameters_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
        type_parameters: Vec<TypeHandle>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.type_parameters = type_parameters;
        });
    }

    fn type_alias_type_parameter_count<Q>(&self, symbol: Q) -> usize
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.type_alias_link_handle(symbol);
        self.type_alias_type_parameter_count_by_handle(handle)
    }

    fn type_alias_type_parameter_count_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
    ) -> usize {
        self.with_by_handle(handle, |links| links.type_parameters.len())
    }

    fn type_alias_has_type_parameters<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.type_alias_link_handle(symbol);
        self.type_alias_has_type_parameters_by_handle(handle)
    }

    fn type_alias_has_type_parameters_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
    ) -> bool {
        self.type_alias_type_parameter_count_by_handle(handle) != 0
    }

    fn type_alias_instantiation<Q>(&self, symbol: Q, key: CacheHashKey) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.type_alias_link_handle(symbol);
        self.type_alias_instantiation_by_handle(handle, key)
    }

    fn type_alias_instantiation_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
        key: CacheHashKey,
    ) -> Option<TypeHandle> {
        self.with_by_handle(handle, |links| links.instantiations.get(&key).copied())
    }

    fn set_type_alias_instantiations<Q>(
        &self,
        symbol: Q,
        instantiations: HashMap<CacheHashKey, TypeHandle>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.type_alias_link_handle(symbol);
        self.set_type_alias_instantiations_by_handle(handle, instantiations);
    }

    fn set_type_alias_instantiations_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
        instantiations: HashMap<CacheHashKey, TypeHandle>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.instantiations = instantiations;
        });
    }

    fn insert_type_alias_instantiation<Q>(&self, symbol: Q, key: CacheHashKey, ty: TypeHandle)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.type_alias_link_handle(symbol);
        self.insert_type_alias_instantiation_by_handle(handle, key, ty);
    }

    fn insert_type_alias_instantiation_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
        key: CacheHashKey,
        ty: TypeHandle,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.instantiations.insert(key, ty);
        });
    }

    fn type_alias_is_constructor_declared_property<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.type_alias_link_handle(symbol);
        self.type_alias_is_constructor_declared_property_by_handle(handle)
    }

    fn type_alias_is_constructor_declared_property_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
    ) -> bool {
        self.with_by_handle(handle, |links| links.is_constructor_declared_property)
    }

    fn set_type_alias_is_constructor_declared_property<Q>(&self, symbol: Q, value: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.type_alias_link_handle(symbol);
        self.set_type_alias_is_constructor_declared_property_by_handle(handle, value);
    }

    fn set_type_alias_is_constructor_declared_property_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
        value: bool,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.is_constructor_declared_property = value;
        });
    }
}

pub(crate) trait DeclaredTypeLinksStoreExt {
    fn declared_type_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<DeclaredTypeLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn declared_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn try_declared_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn declared_type_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
    ) -> Option<TypeHandle>;
    fn set_declared_type<Q>(&self, symbol: Q, declared_type: Option<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_declared_type_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
        declared_type: Option<TypeHandle>,
    );
    fn interface_checked<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn interface_checked_by_handle(&self, handle: core::LinkHandle<DeclaredTypeLinks>) -> bool;
    fn set_interface_checked<Q>(&self, symbol: Q, checked: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_interface_checked_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
        checked: bool,
    );
    fn mark_interface_checked<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn mark_interface_checked_by_handle(&self, handle: core::LinkHandle<DeclaredTypeLinks>);
    fn index_signatures_checked<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn index_signatures_checked_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
    ) -> bool;
    fn set_index_signatures_checked<Q>(&self, symbol: Q, checked: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_index_signatures_checked_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
        checked: bool,
    );
    fn mark_index_signatures_checked<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn mark_index_signatures_checked_by_handle(&self, handle: core::LinkHandle<DeclaredTypeLinks>);
    fn type_parameters_checked<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn type_parameters_checked_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
    ) -> bool;
    fn set_type_parameters_checked<Q>(&self, symbol: Q, checked: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_type_parameters_checked_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
        checked: bool,
    );
    fn mark_type_parameters_checked<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn mark_type_parameters_checked_by_handle(&self, handle: core::LinkHandle<DeclaredTypeLinks>);
    fn enum_checked<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn enum_checked_by_handle(&self, handle: core::LinkHandle<DeclaredTypeLinks>) -> bool;
    fn set_enum_checked<Q>(&self, symbol: Q, checked: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_enum_checked_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
        checked: bool,
    );
    fn mark_enum_checked<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn mark_enum_checked_by_handle(&self, handle: core::LinkHandle<DeclaredTypeLinks>);
}

impl DeclaredTypeLinksStoreExt for SymbolLinkStore<DeclaredTypeLinks> {
    fn declared_type_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<DeclaredTypeLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.ensure_handle(symbol)
    }

    fn declared_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.declared_type_link_handle(symbol);
        self.declared_type_by_handle(handle)
    }

    fn try_declared_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.try_handle(symbol)?;
        self.declared_type_by_handle(handle)
    }

    fn declared_type_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
    ) -> Option<TypeHandle> {
        self.with_by_handle(handle, |links| links.declared_type)
    }

    fn set_declared_type<Q>(&self, symbol: Q, declared_type: Option<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.declared_type_link_handle(symbol);
        self.set_declared_type_by_handle(handle, declared_type);
    }

    fn set_declared_type_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
        declared_type: Option<TypeHandle>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.declared_type = declared_type;
        });
    }

    fn interface_checked<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.declared_type_link_handle(symbol);
        self.interface_checked_by_handle(handle)
    }

    fn interface_checked_by_handle(&self, handle: core::LinkHandle<DeclaredTypeLinks>) -> bool {
        self.with_by_handle(handle, |links| links.interface_checked)
    }

    fn set_interface_checked<Q>(&self, symbol: Q, checked: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.declared_type_link_handle(symbol);
        self.set_interface_checked_by_handle(handle, checked);
    }

    fn set_interface_checked_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
        checked: bool,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.interface_checked = checked;
        });
    }

    fn mark_interface_checked<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.declared_type_link_handle(symbol);
        self.mark_interface_checked_by_handle(handle);
    }

    fn mark_interface_checked_by_handle(&self, handle: core::LinkHandle<DeclaredTypeLinks>) {
        self.set_interface_checked_by_handle(handle, true);
    }

    fn index_signatures_checked<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.declared_type_link_handle(symbol);
        self.index_signatures_checked_by_handle(handle)
    }

    fn index_signatures_checked_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
    ) -> bool {
        self.with_by_handle(handle, |links| links.index_signatures_checked)
    }

    fn set_index_signatures_checked<Q>(&self, symbol: Q, checked: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.declared_type_link_handle(symbol);
        self.set_index_signatures_checked_by_handle(handle, checked);
    }

    fn set_index_signatures_checked_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
        checked: bool,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.index_signatures_checked = checked;
        });
    }

    fn mark_index_signatures_checked<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.declared_type_link_handle(symbol);
        self.mark_index_signatures_checked_by_handle(handle);
    }

    fn mark_index_signatures_checked_by_handle(&self, handle: core::LinkHandle<DeclaredTypeLinks>) {
        self.set_index_signatures_checked_by_handle(handle, true);
    }

    fn type_parameters_checked<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.declared_type_link_handle(symbol);
        self.type_parameters_checked_by_handle(handle)
    }

    fn type_parameters_checked_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
    ) -> bool {
        self.with_by_handle(handle, |links| links.type_parameters_checked)
    }

    fn set_type_parameters_checked<Q>(&self, symbol: Q, checked: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.declared_type_link_handle(symbol);
        self.set_type_parameters_checked_by_handle(handle, checked);
    }

    fn set_type_parameters_checked_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
        checked: bool,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.type_parameters_checked = checked;
        });
    }

    fn mark_type_parameters_checked<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.declared_type_link_handle(symbol);
        self.mark_type_parameters_checked_by_handle(handle);
    }

    fn mark_type_parameters_checked_by_handle(&self, handle: core::LinkHandle<DeclaredTypeLinks>) {
        self.set_type_parameters_checked_by_handle(handle, true);
    }

    fn enum_checked<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.declared_type_link_handle(symbol);
        self.enum_checked_by_handle(handle)
    }

    fn enum_checked_by_handle(&self, handle: core::LinkHandle<DeclaredTypeLinks>) -> bool {
        self.with_by_handle(handle, |links| links.enum_checked)
    }

    fn set_enum_checked<Q>(&self, symbol: Q, checked: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.declared_type_link_handle(symbol);
        self.set_enum_checked_by_handle(handle, checked);
    }

    fn set_enum_checked_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
        checked: bool,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.enum_checked = checked;
        });
    }

    fn mark_enum_checked<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.declared_type_link_handle(symbol);
        self.mark_enum_checked_by_handle(handle);
    }

    fn mark_enum_checked_by_handle(&self, handle: core::LinkHandle<DeclaredTypeLinks>) {
        self.set_enum_checked_by_handle(handle, true);
    }
}

pub(crate) trait ValueSymbolLinksStoreExt {
    fn value_symbol_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<ValueSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn value_symbol_instantiation_snapshot_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> ValueSymbolInstantiationSnapshot;
    fn set_instantiated_value_symbol_links_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        target: Option<SymbolIdentity>,
        mapper: Option<TypeMapperHandle>,
        name_type: Option<TypeHandle>,
    );
    fn value_symbol_resolved_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn try_value_symbol_resolved_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn value_symbol_resolved_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<TypeHandle>;
    fn set_value_symbol_resolved_type<Q>(&self, symbol: Q, resolved_type: Option<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_value_symbol_resolved_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        resolved_type: Option<TypeHandle>,
    );
    fn value_symbol_write_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn try_value_symbol_write_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn value_symbol_write_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<TypeHandle>;
    fn set_value_symbol_write_type<Q>(&self, symbol: Q, write_type: Option<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_value_symbol_write_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        write_type: Option<TypeHandle>,
    );
    fn value_symbol_write_type_or_resolved_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn value_symbol_write_type_or_resolved_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<TypeHandle>;
    fn value_symbol_target<Q>(&self, symbol: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn value_symbol_target_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<SymbolIdentity>;
    fn set_value_symbol_target<Q>(&self, symbol: Q, target: Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_value_symbol_target_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        target: Option<SymbolIdentity>,
    );
    fn value_symbol_mapper<Q>(&self, symbol: Q) -> Option<TypeMapperHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn try_value_symbol_mapper<Q>(&self, symbol: Q) -> Option<TypeMapperHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn value_symbol_mapper_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<TypeMapperHandle>;
    fn set_value_symbol_mapper<Q>(&self, symbol: Q, mapper: Option<TypeMapperHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_value_symbol_mapper_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        mapper: Option<TypeMapperHandle>,
    );
    fn value_symbol_name_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn try_value_symbol_name_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn value_symbol_name_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<TypeHandle>;
    fn set_value_symbol_name_type<Q>(&self, symbol: Q, name_type: Option<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_value_symbol_name_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        name_type: Option<TypeHandle>,
    );
    fn value_symbol_containing_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn try_value_symbol_containing_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn value_symbol_containing_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<TypeHandle>;
    fn set_value_symbol_containing_type<Q>(&self, symbol: Q, containing_type: Option<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_value_symbol_containing_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        containing_type: Option<TypeHandle>,
    );
    fn function_or_constructor_checked<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn function_or_constructor_checked_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> bool;
    fn set_function_or_constructor_checked<Q>(&self, symbol: Q, checked: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_function_or_constructor_checked_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        checked: bool,
    );
    fn cjs_export_merged<Q>(&self, symbol: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn cjs_export_merged_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<SymbolIdentity>;
    fn set_cjs_export_merged<Q>(&self, symbol: Q, merged: Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_cjs_export_merged_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        merged: Option<SymbolIdentity>,
    );
    fn get_inferred_class_symbol<Q>(
        &self,
        symbol: Q,
        target: SymbolIdentity,
    ) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn get_inferred_class_symbol_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        target: SymbolIdentity,
    ) -> Option<SymbolIdentity>;
    fn insert_inferred_class_symbol<Q>(
        &self,
        symbol: Q,
        target: SymbolIdentity,
        inferred: SymbolIdentity,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn insert_inferred_class_symbol_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        target: SymbolIdentity,
        inferred: SymbolIdentity,
    );
}

impl ValueSymbolLinksStoreExt for SymbolLinkStore<ValueSymbolLinks> {
    fn value_symbol_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<ValueSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.ensure_handle(symbol)
    }

    fn value_symbol_instantiation_snapshot_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> ValueSymbolInstantiationSnapshot {
        self.with_by_handle(handle, |links| ValueSymbolInstantiationSnapshot {
            resolved_type: links.resolved_type,
            write_type: links.write_type,
            target: links.target,
            mapper: links.mapper,
            name_type: links.name_type,
        })
    }

    fn set_instantiated_value_symbol_links_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        target: Option<SymbolIdentity>,
        mapper: Option<TypeMapperHandle>,
        name_type: Option<TypeHandle>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.target = target;
            links.mapper = mapper;
            links.name_type = name_type;
        });
    }

    fn value_symbol_resolved_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.value_symbol_link_handle(symbol);
        self.value_symbol_resolved_type_by_handle(handle)
    }

    fn try_value_symbol_resolved_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.try_handle(symbol)?;
        self.value_symbol_resolved_type_by_handle(handle)
    }

    fn value_symbol_resolved_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<TypeHandle> {
        self.with_by_handle(handle, |links| links.resolved_type)
    }

    fn set_value_symbol_resolved_type<Q>(&self, symbol: Q, resolved_type: Option<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.value_symbol_link_handle(symbol);
        self.set_value_symbol_resolved_type_by_handle(handle, resolved_type);
    }

    fn set_value_symbol_resolved_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        resolved_type: Option<TypeHandle>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.resolved_type = resolved_type;
        });
    }

    fn value_symbol_write_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.value_symbol_link_handle(symbol);
        self.value_symbol_write_type_by_handle(handle)
    }

    fn try_value_symbol_write_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.try_handle(symbol)?;
        self.value_symbol_write_type_by_handle(handle)
    }

    fn value_symbol_write_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<TypeHandle> {
        self.with_by_handle(handle, |links| links.write_type)
    }

    fn set_value_symbol_write_type<Q>(&self, symbol: Q, write_type: Option<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.value_symbol_link_handle(symbol);
        self.set_value_symbol_write_type_by_handle(handle, write_type);
    }

    fn set_value_symbol_write_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        write_type: Option<TypeHandle>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.write_type = write_type;
        });
    }

    fn value_symbol_write_type_or_resolved_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.value_symbol_link_handle(symbol);
        self.value_symbol_write_type_or_resolved_type_by_handle(handle)
    }

    fn value_symbol_write_type_or_resolved_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<TypeHandle> {
        self.with_by_handle(handle, |links| links.write_type.or(links.resolved_type))
    }

    fn value_symbol_target<Q>(&self, symbol: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.value_symbol_link_handle(symbol);
        self.value_symbol_target_by_handle(handle)
    }

    fn value_symbol_target_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<SymbolIdentity> {
        self.with_by_handle(handle, |links| links.target)
    }

    fn set_value_symbol_target<Q>(&self, symbol: Q, target: Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.value_symbol_link_handle(symbol);
        self.set_value_symbol_target_by_handle(handle, target);
    }

    fn set_value_symbol_target_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        target: Option<SymbolIdentity>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.target = target;
        });
    }

    fn value_symbol_mapper<Q>(&self, symbol: Q) -> Option<TypeMapperHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.value_symbol_link_handle(symbol);
        self.value_symbol_mapper_by_handle(handle)
    }

    fn try_value_symbol_mapper<Q>(&self, symbol: Q) -> Option<TypeMapperHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.try_handle(symbol)?;
        self.value_symbol_mapper_by_handle(handle)
    }

    fn value_symbol_mapper_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<TypeMapperHandle> {
        self.with_by_handle(handle, |links| links.mapper)
    }

    fn set_value_symbol_mapper<Q>(&self, symbol: Q, mapper: Option<TypeMapperHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.value_symbol_link_handle(symbol);
        self.set_value_symbol_mapper_by_handle(handle, mapper);
    }

    fn set_value_symbol_mapper_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        mapper: Option<TypeMapperHandle>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.mapper = mapper;
        });
    }

    fn value_symbol_name_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.value_symbol_link_handle(symbol);
        self.value_symbol_name_type_by_handle(handle)
    }

    fn try_value_symbol_name_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.try_handle(symbol)?;
        self.value_symbol_name_type_by_handle(handle)
    }

    fn value_symbol_name_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<TypeHandle> {
        self.with_by_handle(handle, |links| links.name_type)
    }

    fn set_value_symbol_name_type<Q>(&self, symbol: Q, name_type: Option<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.value_symbol_link_handle(symbol);
        self.set_value_symbol_name_type_by_handle(handle, name_type);
    }

    fn set_value_symbol_name_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        name_type: Option<TypeHandle>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.name_type = name_type;
        });
    }

    fn value_symbol_containing_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.value_symbol_link_handle(symbol);
        self.value_symbol_containing_type_by_handle(handle)
    }

    fn try_value_symbol_containing_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.try_handle(symbol)?;
        self.value_symbol_containing_type_by_handle(handle)
    }

    fn value_symbol_containing_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<TypeHandle> {
        self.with_by_handle(handle, |links| links.containing_type)
    }

    fn set_value_symbol_containing_type<Q>(&self, symbol: Q, containing_type: Option<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.value_symbol_link_handle(symbol);
        self.set_value_symbol_containing_type_by_handle(handle, containing_type);
    }

    fn set_value_symbol_containing_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        containing_type: Option<TypeHandle>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.containing_type = containing_type;
        });
    }

    fn function_or_constructor_checked<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.value_symbol_link_handle(symbol);
        self.function_or_constructor_checked_by_handle(handle)
    }

    fn function_or_constructor_checked_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> bool {
        self.with_by_handle(handle, |links| links.function_or_constructor_checked)
    }

    fn set_function_or_constructor_checked<Q>(&self, symbol: Q, checked: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.value_symbol_link_handle(symbol);
        self.set_function_or_constructor_checked_by_handle(handle, checked);
    }

    fn set_function_or_constructor_checked_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        checked: bool,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.function_or_constructor_checked = checked;
        });
    }

    fn cjs_export_merged<Q>(&self, symbol: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.value_symbol_link_handle(symbol);
        self.cjs_export_merged_by_handle(handle)
    }

    fn cjs_export_merged_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<SymbolIdentity> {
        self.with_by_handle(handle, |links| links.cjs_export_merged)
    }

    fn set_cjs_export_merged<Q>(&self, symbol: Q, merged: Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.value_symbol_link_handle(symbol);
        self.set_cjs_export_merged_by_handle(handle, merged);
    }

    fn set_cjs_export_merged_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        merged: Option<SymbolIdentity>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.cjs_export_merged = merged;
        });
    }

    fn get_inferred_class_symbol<Q>(
        &self,
        symbol: Q,
        target: SymbolIdentity,
    ) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.value_symbol_link_handle(symbol);
        self.get_inferred_class_symbol_by_handle(handle, target)
    }

    fn get_inferred_class_symbol_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        target: SymbolIdentity,
    ) -> Option<SymbolIdentity> {
        self.with_by_handle(handle, |links| {
            links.inferred_class_symbol.get(&target).copied()
        })
    }

    fn insert_inferred_class_symbol<Q>(
        &self,
        symbol: Q,
        target: SymbolIdentity,
        inferred: SymbolIdentity,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.value_symbol_link_handle(symbol);
        self.insert_inferred_class_symbol_by_handle(handle, target, inferred);
    }

    fn insert_inferred_class_symbol_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        target: SymbolIdentity,
        inferred: SymbolIdentity,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.inferred_class_symbol.insert(target, inferred);
        });
    }
}

pub(crate) trait MappedSymbolLinksStoreExt {
    fn mapped_symbol_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<MappedSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn mapped_key_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn mapped_key_type_by_handle(
        &self,
        handle: core::LinkHandle<MappedSymbolLinks>,
    ) -> Option<TypeHandle>;
    fn set_mapped_key_type<Q>(&self, symbol: Q, key_type: Option<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_mapped_key_type_by_handle(
        &self,
        handle: core::LinkHandle<MappedSymbolLinks>,
        key_type: Option<TypeHandle>,
    );
    fn mapped_synthetic_origin<Q>(&self, symbol: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn mapped_synthetic_origin_by_handle(
        &self,
        handle: core::LinkHandle<MappedSymbolLinks>,
    ) -> Option<SymbolIdentity>;
    fn set_mapped_synthetic_origin<Q>(&self, symbol: Q, origin: Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_mapped_synthetic_origin_by_handle(
        &self,
        handle: core::LinkHandle<MappedSymbolLinks>,
        origin: Option<SymbolIdentity>,
    );
}

impl MappedSymbolLinksStoreExt for SymbolLinkStore<MappedSymbolLinks> {
    fn mapped_symbol_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<MappedSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.ensure_handle(symbol)
    }

    fn mapped_key_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.mapped_symbol_link_handle(symbol);
        self.mapped_key_type_by_handle(handle)
    }

    fn mapped_key_type_by_handle(
        &self,
        handle: core::LinkHandle<MappedSymbolLinks>,
    ) -> Option<TypeHandle> {
        self.with_by_handle(handle, |links| links.key_type)
    }

    fn set_mapped_key_type<Q>(&self, symbol: Q, key_type: Option<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.mapped_symbol_link_handle(symbol);
        self.set_mapped_key_type_by_handle(handle, key_type);
    }

    fn set_mapped_key_type_by_handle(
        &self,
        handle: core::LinkHandle<MappedSymbolLinks>,
        key_type: Option<TypeHandle>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.key_type = key_type;
        });
    }

    fn mapped_synthetic_origin<Q>(&self, symbol: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.mapped_symbol_link_handle(symbol);
        self.mapped_synthetic_origin_by_handle(handle)
    }

    fn mapped_synthetic_origin_by_handle(
        &self,
        handle: core::LinkHandle<MappedSymbolLinks>,
    ) -> Option<SymbolIdentity> {
        self.with_by_handle(handle, |links| links.synthetic_origin)
    }

    fn set_mapped_synthetic_origin<Q>(&self, symbol: Q, origin: Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.mapped_symbol_link_handle(symbol);
        self.set_mapped_synthetic_origin_by_handle(handle, origin);
    }

    fn set_mapped_synthetic_origin_by_handle(
        &self,
        handle: core::LinkHandle<MappedSymbolLinks>,
        origin: Option<SymbolIdentity>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.synthetic_origin = origin;
        });
    }
}

pub(crate) trait ReverseMappedSymbolLinksStoreExt {
    fn reverse_mapped_symbol_link_handle<Q>(
        &self,
        symbol: Q,
    ) -> core::LinkHandle<ReverseMappedSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn try_reverse_mapped_symbol_link_handle<Q>(
        &self,
        symbol: Q,
    ) -> Option<core::LinkHandle<ReverseMappedSymbolLinks>>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn has_reverse_mapped_symbol_links<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn try_reverse_mapped_symbol_link_types<Q>(
        &self,
        symbol: Q,
    ) -> Option<(Option<TypeHandle>, Option<TypeHandle>, Option<TypeHandle>)>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn reverse_mapped_symbol_link_types_by_handle(
        &self,
        handle: core::LinkHandle<ReverseMappedSymbolLinks>,
    ) -> (Option<TypeHandle>, Option<TypeHandle>, Option<TypeHandle>);
    fn reverse_mapped_resolved_type_by_handle(
        &self,
        handle: core::LinkHandle<ReverseMappedSymbolLinks>,
    ) -> Option<TypeHandle>;
    fn set_reverse_mapped_resolved_type_by_handle(
        &self,
        handle: core::LinkHandle<ReverseMappedSymbolLinks>,
        resolved_type: Option<TypeHandle>,
    );
    fn reverse_mapped_property_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn reverse_mapped_property_type_by_handle(
        &self,
        handle: core::LinkHandle<ReverseMappedSymbolLinks>,
    ) -> Option<TypeHandle>;
    fn reverse_mapped_mapped_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn reverse_mapped_mapped_type_by_handle(
        &self,
        handle: core::LinkHandle<ReverseMappedSymbolLinks>,
    ) -> Option<TypeHandle>;
    fn reverse_mapped_constraint_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn reverse_mapped_constraint_type_by_handle(
        &self,
        handle: core::LinkHandle<ReverseMappedSymbolLinks>,
    ) -> Option<TypeHandle>;
    fn set_reverse_mapped_symbol_link_types<Q>(
        &self,
        symbol: Q,
        property_type: Option<TypeHandle>,
        mapped_type: Option<TypeHandle>,
        constraint_type: Option<TypeHandle>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_reverse_mapped_symbol_link_types_by_handle(
        &self,
        handle: core::LinkHandle<ReverseMappedSymbolLinks>,
        property_type: Option<TypeHandle>,
        mapped_type: Option<TypeHandle>,
        constraint_type: Option<TypeHandle>,
    );
}

impl ReverseMappedSymbolLinksStoreExt for SymbolLinkStore<ReverseMappedSymbolLinks> {
    fn reverse_mapped_symbol_link_handle<Q>(
        &self,
        symbol: Q,
    ) -> core::LinkHandle<ReverseMappedSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.ensure_handle(symbol)
    }

    fn try_reverse_mapped_symbol_link_handle<Q>(
        &self,
        symbol: Q,
    ) -> Option<core::LinkHandle<ReverseMappedSymbolLinks>>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.try_handle(symbol)
    }

    fn has_reverse_mapped_symbol_links<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.has(symbol)
    }

    fn try_reverse_mapped_symbol_link_types<Q>(
        &self,
        symbol: Q,
    ) -> Option<(Option<TypeHandle>, Option<TypeHandle>, Option<TypeHandle>)>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.try_reverse_mapped_symbol_link_handle(symbol)?;
        Some(self.reverse_mapped_symbol_link_types_by_handle(handle))
    }

    fn reverse_mapped_symbol_link_types_by_handle(
        &self,
        handle: core::LinkHandle<ReverseMappedSymbolLinks>,
    ) -> (Option<TypeHandle>, Option<TypeHandle>, Option<TypeHandle>) {
        self.with_by_handle(handle, |links| {
            (
                links.property_type,
                links.mapped_type,
                links.constraint_type,
            )
        })
    }

    fn reverse_mapped_resolved_type_by_handle(
        &self,
        handle: core::LinkHandle<ReverseMappedSymbolLinks>,
    ) -> Option<TypeHandle> {
        self.with_by_handle(handle, |links| links.resolved_type)
    }

    fn set_reverse_mapped_resolved_type_by_handle(
        &self,
        handle: core::LinkHandle<ReverseMappedSymbolLinks>,
        resolved_type: Option<TypeHandle>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.resolved_type = resolved_type;
        });
    }

    fn reverse_mapped_property_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.reverse_mapped_symbol_link_handle(symbol);
        self.reverse_mapped_property_type_by_handle(handle)
    }

    fn reverse_mapped_property_type_by_handle(
        &self,
        handle: core::LinkHandle<ReverseMappedSymbolLinks>,
    ) -> Option<TypeHandle> {
        self.with_by_handle(handle, |links| links.property_type)
    }

    fn reverse_mapped_mapped_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.reverse_mapped_symbol_link_handle(symbol);
        self.reverse_mapped_mapped_type_by_handle(handle)
    }

    fn reverse_mapped_mapped_type_by_handle(
        &self,
        handle: core::LinkHandle<ReverseMappedSymbolLinks>,
    ) -> Option<TypeHandle> {
        self.with_by_handle(handle, |links| links.mapped_type)
    }

    fn reverse_mapped_constraint_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.reverse_mapped_symbol_link_handle(symbol);
        self.reverse_mapped_constraint_type_by_handle(handle)
    }

    fn reverse_mapped_constraint_type_by_handle(
        &self,
        handle: core::LinkHandle<ReverseMappedSymbolLinks>,
    ) -> Option<TypeHandle> {
        self.with_by_handle(handle, |links| links.constraint_type)
    }

    fn set_reverse_mapped_symbol_link_types<Q>(
        &self,
        symbol: Q,
        property_type: Option<TypeHandle>,
        mapped_type: Option<TypeHandle>,
        constraint_type: Option<TypeHandle>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.reverse_mapped_symbol_link_handle(symbol);
        self.set_reverse_mapped_symbol_link_types_by_handle(
            handle,
            property_type,
            mapped_type,
            constraint_type,
        );
    }

    fn set_reverse_mapped_symbol_link_types_by_handle(
        &self,
        handle: core::LinkHandle<ReverseMappedSymbolLinks>,
        property_type: Option<TypeHandle>,
        mapped_type: Option<TypeHandle>,
        constraint_type: Option<TypeHandle>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.property_type = property_type;
            links.mapped_type = mapped_type;
            links.constraint_type = constraint_type;
        });
    }
}

pub(crate) trait MarkedAssignmentSymbolLinksStoreExt {
    fn marked_assignment_symbol_link_handle<Q>(
        &self,
        symbol: Q,
    ) -> core::LinkHandle<MarkedAssignmentSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn marked_assignment_last_assignment_pos<Q>(&self, symbol: Q) -> i32
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn marked_assignment_last_assignment_pos_by_handle(
        &self,
        handle: core::LinkHandle<MarkedAssignmentSymbolLinks>,
    ) -> i32;
    fn set_marked_assignment_last_assignment_pos<Q>(&self, symbol: Q, last_assignment_pos: i32)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_marked_assignment_last_assignment_pos_by_handle(
        &self,
        handle: core::LinkHandle<MarkedAssignmentSymbolLinks>,
        last_assignment_pos: i32,
    );
    fn marked_assignment_has_definite_assignment<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn marked_assignment_has_definite_assignment_by_handle(
        &self,
        handle: core::LinkHandle<MarkedAssignmentSymbolLinks>,
    ) -> bool;
    fn set_marked_assignment_has_definite_assignment<Q>(
        &self,
        symbol: Q,
        has_definite_assignment: bool,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_marked_assignment_has_definite_assignment_by_handle(
        &self,
        handle: core::LinkHandle<MarkedAssignmentSymbolLinks>,
        has_definite_assignment: bool,
    );
    fn mark_marked_assignment_has_definite_assignment<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn mark_marked_assignment_has_definite_assignment_by_handle(
        &self,
        handle: core::LinkHandle<MarkedAssignmentSymbolLinks>,
    );
}

impl MarkedAssignmentSymbolLinksStoreExt for SymbolLinkStore<MarkedAssignmentSymbolLinks> {
    fn marked_assignment_symbol_link_handle<Q>(
        &self,
        symbol: Q,
    ) -> core::LinkHandle<MarkedAssignmentSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.ensure_handle(symbol)
    }

    fn marked_assignment_last_assignment_pos<Q>(&self, symbol: Q) -> i32
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.marked_assignment_symbol_link_handle(symbol);
        self.marked_assignment_last_assignment_pos_by_handle(handle)
    }

    fn marked_assignment_last_assignment_pos_by_handle(
        &self,
        handle: core::LinkHandle<MarkedAssignmentSymbolLinks>,
    ) -> i32 {
        self.with_by_handle(handle, |links| links.last_assignment_pos)
    }

    fn set_marked_assignment_last_assignment_pos<Q>(&self, symbol: Q, last_assignment_pos: i32)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.marked_assignment_symbol_link_handle(symbol);
        self.set_marked_assignment_last_assignment_pos_by_handle(handle, last_assignment_pos);
    }

    fn set_marked_assignment_last_assignment_pos_by_handle(
        &self,
        handle: core::LinkHandle<MarkedAssignmentSymbolLinks>,
        last_assignment_pos: i32,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.last_assignment_pos = last_assignment_pos;
        });
    }

    fn marked_assignment_has_definite_assignment<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.marked_assignment_symbol_link_handle(symbol);
        self.marked_assignment_has_definite_assignment_by_handle(handle)
    }

    fn marked_assignment_has_definite_assignment_by_handle(
        &self,
        handle: core::LinkHandle<MarkedAssignmentSymbolLinks>,
    ) -> bool {
        self.with_by_handle(handle, |links| links.has_definite_assignment)
    }

    fn set_marked_assignment_has_definite_assignment<Q>(
        &self,
        symbol: Q,
        has_definite_assignment: bool,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.marked_assignment_symbol_link_handle(symbol);
        self.set_marked_assignment_has_definite_assignment_by_handle(
            handle,
            has_definite_assignment,
        );
    }

    fn set_marked_assignment_has_definite_assignment_by_handle(
        &self,
        handle: core::LinkHandle<MarkedAssignmentSymbolLinks>,
        has_definite_assignment: bool,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.has_definite_assignment = has_definite_assignment;
        });
    }

    fn mark_marked_assignment_has_definite_assignment<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.marked_assignment_symbol_link_handle(symbol);
        self.mark_marked_assignment_has_definite_assignment_by_handle(handle);
    }

    fn mark_marked_assignment_has_definite_assignment_by_handle(
        &self,
        handle: core::LinkHandle<MarkedAssignmentSymbolLinks>,
    ) {
        self.set_marked_assignment_has_definite_assignment_by_handle(handle, true);
    }
}

pub(crate) trait ContainingSymbolLinksStoreExt {
    fn containing_symbol_link_handle<Q>(
        &self,
        symbol: Q,
    ) -> core::LinkHandle<ContainingSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn alternative_containing_modules_for_file<Q>(
        &self,
        symbol: Q,
        file_id: ast::NodeId,
    ) -> Option<Vec<SymbolIdentity>>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn record_alternative_containing_modules_for_file<Q>(
        &self,
        symbol: Q,
        file_id: ast::NodeId,
        modules: Vec<SymbolIdentity>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn extended_containers<Q>(&self, symbol: Q) -> Option<Vec<SymbolIdentity>>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_extended_containers<Q>(&self, symbol: Q, containers: Vec<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn accessible_chain_cache_entry<Q>(
        &self,
        symbol: Q,
        key: &AccessibleChainCacheKey,
    ) -> Option<Vec<SymbolIdentity>>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn record_accessible_chain_cache_entry<Q>(
        &self,
        symbol: Q,
        key: AccessibleChainCacheKey,
        chain: Vec<SymbolIdentity>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>;
}

impl ContainingSymbolLinksStoreExt for SymbolLinkStore<ContainingSymbolLinks> {
    fn containing_symbol_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<ContainingSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.ensure_handle(symbol)
    }

    fn alternative_containing_modules_for_file<Q>(
        &self,
        symbol: Q,
        file_id: ast::NodeId,
    ) -> Option<Vec<SymbolIdentity>>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.containing_symbol_link_handle(symbol);
        self.with_by_handle(handle, |links| {
            links.extended_containers_by_file.get(&file_id).cloned()
        })
    }

    fn record_alternative_containing_modules_for_file<Q>(
        &self,
        symbol: Q,
        file_id: ast::NodeId,
        modules: Vec<SymbolIdentity>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        if modules.is_empty() {
            return;
        }
        let handle = self.containing_symbol_link_handle(symbol);
        self.with_by_handle_mut(handle, |links| {
            links.extended_containers_by_file.insert(file_id, modules);
        });
    }

    fn extended_containers<Q>(&self, symbol: Q) -> Option<Vec<SymbolIdentity>>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.containing_symbol_link_handle(symbol);
        self.with_by_handle(handle, |links| links.extended_containers.clone())
    }

    fn set_extended_containers<Q>(&self, symbol: Q, containers: Vec<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.containing_symbol_link_handle(symbol);
        self.with_by_handle_mut(handle, |links| {
            links.extended_containers = Some(containers);
        });
    }

    fn accessible_chain_cache_entry<Q>(
        &self,
        symbol: Q,
        key: &AccessibleChainCacheKey,
    ) -> Option<Vec<SymbolIdentity>>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.containing_symbol_link_handle(symbol);
        self.with_by_handle(handle, |links| {
            links.accessible_chain_cache.get(key).cloned()
        })
    }

    fn record_accessible_chain_cache_entry<Q>(
        &self,
        symbol: Q,
        key: AccessibleChainCacheKey,
        chain: Vec<SymbolIdentity>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.containing_symbol_link_handle(symbol);
        self.with_by_handle_mut(handle, |links| {
            links.accessible_chain_cache.insert(key, chain);
        });
    }
}

pub(crate) trait DeferredSymbolLinksStoreExt {
    fn deferred_symbol_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<DeferredSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn deferred_symbol_links_by_handle(
        &self,
        handle: core::LinkHandle<DeferredSymbolLinks>,
    ) -> (Option<TypeHandle>, Vec<TypeHandle>, Vec<TypeHandle>);
    fn set_deferred_symbol_links<Q>(
        &self,
        symbol: Q,
        parent: Option<TypeHandle>,
        constituents: Vec<TypeHandle>,
        write_constituents: Vec<TypeHandle>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_deferred_symbol_links_by_handle(
        &self,
        handle: core::LinkHandle<DeferredSymbolLinks>,
        parent: Option<TypeHandle>,
        constituents: Vec<TypeHandle>,
        write_constituents: Vec<TypeHandle>,
    );
}

impl DeferredSymbolLinksStoreExt for SymbolLinkStore<DeferredSymbolLinks> {
    fn deferred_symbol_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<DeferredSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.ensure_handle(symbol)
    }

    fn deferred_symbol_links_by_handle(
        &self,
        handle: core::LinkHandle<DeferredSymbolLinks>,
    ) -> (Option<TypeHandle>, Vec<TypeHandle>, Vec<TypeHandle>) {
        self.with_by_handle(handle, |links| {
            (
                links.parent,
                links.constituents.clone(),
                links.write_constituents.clone(),
            )
        })
    }

    fn set_deferred_symbol_links<Q>(
        &self,
        symbol: Q,
        parent: Option<TypeHandle>,
        constituents: Vec<TypeHandle>,
        write_constituents: Vec<TypeHandle>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.deferred_symbol_link_handle(symbol);
        self.set_deferred_symbol_links_by_handle(handle, parent, constituents, write_constituents);
    }

    fn set_deferred_symbol_links_by_handle(
        &self,
        handle: core::LinkHandle<DeferredSymbolLinks>,
        parent: Option<TypeHandle>,
        constituents: Vec<TypeHandle>,
        write_constituents: Vec<TypeHandle>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.parent = parent;
            links.constituents = constituents;
            links.write_constituents = write_constituents;
        });
    }
}

pub(crate) trait SpreadLinksStoreExt {
    fn spread_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<SpreadLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn spread_symbols<Q>(&self, symbol: Q) -> (Option<SymbolIdentity>, Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn spread_symbols_by_handle(
        &self,
        handle: core::LinkHandle<SpreadLinks>,
    ) -> (Option<SymbolIdentity>, Option<SymbolIdentity>);
    fn set_spread_symbols<Q>(
        &self,
        symbol: Q,
        left: Option<SymbolIdentity>,
        right: Option<SymbolIdentity>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_spread_symbols_by_handle(
        &self,
        handle: core::LinkHandle<SpreadLinks>,
        left: Option<SymbolIdentity>,
        right: Option<SymbolIdentity>,
    );
}

impl SpreadLinksStoreExt for SymbolLinkStore<SpreadLinks> {
    fn spread_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<SpreadLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.ensure_handle(symbol)
    }

    fn spread_symbols<Q>(&self, symbol: Q) -> (Option<SymbolIdentity>, Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.spread_link_handle(symbol);
        self.spread_symbols_by_handle(handle)
    }

    fn spread_symbols_by_handle(
        &self,
        handle: core::LinkHandle<SpreadLinks>,
    ) -> (Option<SymbolIdentity>, Option<SymbolIdentity>) {
        self.with_by_handle(handle, |links| (links.left_spread, links.right_spread))
    }

    fn set_spread_symbols<Q>(
        &self,
        symbol: Q,
        left: Option<SymbolIdentity>,
        right: Option<SymbolIdentity>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.spread_link_handle(symbol);
        self.set_spread_symbols_by_handle(handle, left, right);
    }

    fn set_spread_symbols_by_handle(
        &self,
        handle: core::LinkHandle<SpreadLinks>,
        left: Option<SymbolIdentity>,
        right: Option<SymbolIdentity>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.left_spread = left;
            links.right_spread = right;
        });
    }
}

pub(crate) trait VarianceLinksStoreExt {
    fn variance_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<VarianceLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn variance_cache_state<Q>(&self, symbol: Q) -> VarianceCacheState
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn variance_cache_state_by_handle(
        &self,
        handle: core::LinkHandle<VarianceLinks>,
    ) -> VarianceCacheState;
    fn mark_variances_computing<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn mark_variances_computing_by_handle(&self, handle: core::LinkHandle<VarianceLinks>);
    fn set_variances_computed<Q>(&self, symbol: Q, variances: Vec<VarianceFlags>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_variances_computed_by_handle(
        &self,
        handle: core::LinkHandle<VarianceLinks>,
        variances: Vec<VarianceFlags>,
    );
}

impl VarianceLinksStoreExt for SymbolLinkStore<VarianceLinks> {
    fn variance_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<VarianceLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.ensure_handle(symbol)
    }

    fn variance_cache_state<Q>(&self, symbol: Q) -> VarianceCacheState
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.variance_link_handle(symbol);
        self.variance_cache_state_by_handle(handle)
    }

    fn variance_cache_state_by_handle(
        &self,
        handle: core::LinkHandle<VarianceLinks>,
    ) -> VarianceCacheState {
        self.with_by_handle(handle, |links| links.variances.clone())
    }

    fn mark_variances_computing<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.variance_link_handle(symbol);
        self.mark_variances_computing_by_handle(handle);
    }

    fn mark_variances_computing_by_handle(&self, handle: core::LinkHandle<VarianceLinks>) {
        self.with_by_handle_mut(handle, |links| {
            links.variances = VarianceCacheState::Computing;
        });
    }

    fn set_variances_computed<Q>(&self, symbol: Q, variances: Vec<VarianceFlags>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.variance_link_handle(symbol);
        self.set_variances_computed_by_handle(handle, variances);
    }

    fn set_variances_computed_by_handle(
        &self,
        handle: core::LinkHandle<VarianceLinks>,
        variances: Vec<VarianceFlags>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.variances = VarianceCacheState::Computed(variances);
        });
    }
}

pub(crate) trait SourceFileLinksStoreExt {
    fn source_file_link_handle<Q>(&self, source_file: Q) -> core::LinkHandle<SourceFileLinks>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn source_file_type_checked<Q>(&self, source_file: Q) -> bool
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn source_file_type_checked_by_handle(&self, handle: core::LinkHandle<SourceFileLinks>)
    -> bool;
    fn set_source_file_type_checked<Q>(&self, source_file: Q, checked: bool)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn set_source_file_type_checked_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        checked: bool,
    );
    fn source_file_unused_checked<Q>(&self, source_file: Q) -> bool
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn source_file_unused_checked_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> bool;
    fn set_source_file_unused_checked<Q>(&self, source_file: Q, checked: bool)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn set_source_file_unused_checked_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        checked: bool,
    );
    fn source_file_external_helpers_module<Q>(&self, source_file: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn source_file_external_helpers_module_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> Option<SymbolIdentity>;
    fn set_source_file_external_helpers_module<Q>(
        &self,
        source_file: Q,
        symbol: Option<SymbolIdentity>,
    ) where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn set_source_file_external_helpers_module_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        symbol: Option<SymbolIdentity>,
    );
    fn source_file_requested_external_emit_helpers<Q>(&self, source_file: Q) -> ExternalEmitHelpers
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn source_file_requested_external_emit_helpers_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> ExternalEmitHelpers;
    fn set_source_file_requested_external_emit_helpers<Q>(
        &self,
        source_file: Q,
        helpers: ExternalEmitHelpers,
    ) where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn set_source_file_requested_external_emit_helpers_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        helpers: ExternalEmitHelpers,
    );
    fn add_source_file_requested_external_emit_helpers<Q>(
        &self,
        source_file: Q,
        helpers: ExternalEmitHelpers,
    ) where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn add_source_file_requested_external_emit_helpers_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        helpers: ExternalEmitHelpers,
    );
    fn source_file_has_requested_external_emit_helpers<Q>(
        &self,
        source_file: Q,
        helpers: ExternalEmitHelpers,
    ) -> bool
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn source_file_has_requested_external_emit_helpers_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        helpers: ExternalEmitHelpers,
    ) -> bool;
    fn source_file_local_jsx_namespace<Q>(&self, source_file: Q) -> String
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn source_file_local_jsx_namespace_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> String;
    fn set_source_file_local_jsx_namespace<Q>(&self, source_file: Q, namespace: String)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn set_source_file_local_jsx_namespace_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        namespace: String,
    );
    fn source_file_local_jsx_fragment_namespace<Q>(&self, source_file: Q) -> String
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn source_file_local_jsx_fragment_namespace_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> String;
    fn set_source_file_local_jsx_fragment_namespace<Q>(&self, source_file: Q, namespace: String)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn set_source_file_local_jsx_fragment_namespace_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        namespace: String,
    );
    fn source_file_local_jsx_factory<Q>(&self, source_file: Q) -> Option<ast::EntityName>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn source_file_local_jsx_factory_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> Option<ast::EntityName>;
    fn set_source_file_local_jsx_factory<Q>(
        &self,
        source_file: Q,
        factory: Option<ast::EntityName>,
    ) where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn set_source_file_local_jsx_factory_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        factory: Option<ast::EntityName>,
    );
    fn source_file_local_jsx_fragment_factory<Q>(&self, source_file: Q) -> Option<ast::EntityName>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn source_file_local_jsx_fragment_factory_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> Option<ast::EntityName>;
    fn set_source_file_local_jsx_fragment_factory<Q>(
        &self,
        source_file: Q,
        factory: Option<ast::EntityName>,
    ) where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn set_source_file_local_jsx_fragment_factory_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        factory: Option<ast::EntityName>,
    );
    fn source_file_jsx_fragment_type<Q>(&self, source_file: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn source_file_jsx_fragment_type_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> Option<TypeHandle>;
    fn set_source_file_jsx_fragment_type<Q>(&self, source_file: Q, jsx_fragment_type: TypeHandle)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn set_source_file_jsx_fragment_type_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        jsx_fragment_type: TypeHandle,
    );
    fn add_deferred_node<Q>(&self, source_file: Q, node: ast::Node)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn add_deferred_node_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        node: ast::Node,
    );
    fn next_deferred_node<Q>(&self, source_file: Q, index: usize) -> Option<ast::Node>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn next_deferred_node_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        index: usize,
    ) -> Option<ast::Node>;
    fn clear_deferred_nodes<Q>(&self, source_file: Q)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn clear_deferred_nodes_by_handle(&self, handle: core::LinkHandle<SourceFileLinks>);
    fn push_identifier_check_node<Q>(&self, source_file: Q, node: ast::Node)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn push_identifier_check_node_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        node: ast::Node,
    );
    fn identifier_check_nodes<Q>(&self, source_file: Q) -> Vec<ast::Node>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>;
    fn identifier_check_nodes_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> Vec<ast::Node>;
}

impl SourceFileLinksStoreExt for SourceFileLinkStore<SourceFileLinks> {
    fn source_file_link_handle<Q>(&self, source_file: Q) -> core::LinkHandle<SourceFileLinks>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        self.ensure_handle(source_file)
    }

    fn source_file_type_checked<Q>(&self, source_file: Q) -> bool
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.source_file_type_checked_by_handle(handle)
    }

    fn source_file_type_checked_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> bool {
        self.with_by_handle(handle, |links| links.type_checked)
    }

    fn set_source_file_type_checked<Q>(&self, source_file: Q, checked: bool)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.set_source_file_type_checked_by_handle(handle, checked);
    }

    fn set_source_file_type_checked_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        checked: bool,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.type_checked = checked;
        });
    }

    fn source_file_unused_checked<Q>(&self, source_file: Q) -> bool
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.source_file_unused_checked_by_handle(handle)
    }

    fn source_file_unused_checked_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> bool {
        self.with_by_handle(handle, |links| links.unused_checked)
    }

    fn set_source_file_unused_checked<Q>(&self, source_file: Q, checked: bool)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.set_source_file_unused_checked_by_handle(handle, checked);
    }

    fn set_source_file_unused_checked_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        checked: bool,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.unused_checked = checked;
        });
    }

    fn source_file_external_helpers_module<Q>(&self, source_file: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.source_file_external_helpers_module_by_handle(handle)
    }

    fn source_file_external_helpers_module_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> Option<SymbolIdentity> {
        self.with_by_handle(handle, |links| links.external_helpers_module)
    }

    fn set_source_file_external_helpers_module<Q>(
        &self,
        source_file: Q,
        symbol: Option<SymbolIdentity>,
    ) where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.set_source_file_external_helpers_module_by_handle(handle, symbol);
    }

    fn set_source_file_external_helpers_module_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        symbol: Option<SymbolIdentity>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.external_helpers_module = symbol;
        });
    }

    fn source_file_requested_external_emit_helpers<Q>(&self, source_file: Q) -> ExternalEmitHelpers
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.source_file_requested_external_emit_helpers_by_handle(handle)
    }

    fn source_file_requested_external_emit_helpers_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> ExternalEmitHelpers {
        self.with_by_handle(handle, |links| links.requested_external_emit_helpers)
    }

    fn set_source_file_requested_external_emit_helpers<Q>(
        &self,
        source_file: Q,
        helpers: ExternalEmitHelpers,
    ) where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.set_source_file_requested_external_emit_helpers_by_handle(handle, helpers);
    }

    fn set_source_file_requested_external_emit_helpers_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        helpers: ExternalEmitHelpers,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.requested_external_emit_helpers = helpers;
        });
    }

    fn add_source_file_requested_external_emit_helpers<Q>(
        &self,
        source_file: Q,
        helpers: ExternalEmitHelpers,
    ) where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.add_source_file_requested_external_emit_helpers_by_handle(handle, helpers);
    }

    fn add_source_file_requested_external_emit_helpers_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        helpers: ExternalEmitHelpers,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.requested_external_emit_helpers |= helpers;
        });
    }

    fn source_file_has_requested_external_emit_helpers<Q>(
        &self,
        source_file: Q,
        helpers: ExternalEmitHelpers,
    ) -> bool
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.source_file_has_requested_external_emit_helpers_by_handle(handle, helpers)
    }

    fn source_file_has_requested_external_emit_helpers_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        helpers: ExternalEmitHelpers,
    ) -> bool {
        self.source_file_requested_external_emit_helpers_by_handle(handle) & helpers == helpers
    }

    fn source_file_local_jsx_namespace<Q>(&self, source_file: Q) -> String
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.source_file_local_jsx_namespace_by_handle(handle)
    }

    fn source_file_local_jsx_namespace_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> String {
        self.with_by_handle(handle, |links| links.local_jsx_namespace.clone())
    }

    fn set_source_file_local_jsx_namespace<Q>(&self, source_file: Q, namespace: String)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.set_source_file_local_jsx_namespace_by_handle(handle, namespace);
    }

    fn set_source_file_local_jsx_namespace_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        namespace: String,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.local_jsx_namespace = namespace;
        });
    }

    fn source_file_local_jsx_fragment_namespace<Q>(&self, source_file: Q) -> String
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.source_file_local_jsx_fragment_namespace_by_handle(handle)
    }

    fn source_file_local_jsx_fragment_namespace_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> String {
        self.with_by_handle(handle, |links| links.local_jsx_fragment_namespace.clone())
    }

    fn set_source_file_local_jsx_fragment_namespace<Q>(&self, source_file: Q, namespace: String)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.set_source_file_local_jsx_fragment_namespace_by_handle(handle, namespace);
    }

    fn set_source_file_local_jsx_fragment_namespace_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        namespace: String,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.local_jsx_fragment_namespace = namespace;
        });
    }

    fn source_file_local_jsx_factory<Q>(&self, source_file: Q) -> Option<ast::EntityName>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.source_file_local_jsx_factory_by_handle(handle)
    }

    fn source_file_local_jsx_factory_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> Option<ast::EntityName> {
        self.with_by_handle(handle, |links| links.local_jsx_factory)
    }

    fn set_source_file_local_jsx_factory<Q>(&self, source_file: Q, factory: Option<ast::EntityName>)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.set_source_file_local_jsx_factory_by_handle(handle, factory);
    }

    fn set_source_file_local_jsx_factory_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        factory: Option<ast::EntityName>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.local_jsx_factory = factory;
        });
    }

    fn source_file_local_jsx_fragment_factory<Q>(&self, source_file: Q) -> Option<ast::EntityName>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.source_file_local_jsx_fragment_factory_by_handle(handle)
    }

    fn source_file_local_jsx_fragment_factory_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> Option<ast::EntityName> {
        self.with_by_handle(handle, |links| links.local_jsx_fragment_factory)
    }

    fn set_source_file_local_jsx_fragment_factory<Q>(
        &self,
        source_file: Q,
        factory: Option<ast::EntityName>,
    ) where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.set_source_file_local_jsx_fragment_factory_by_handle(handle, factory);
    }

    fn set_source_file_local_jsx_fragment_factory_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        factory: Option<ast::EntityName>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.local_jsx_fragment_factory = factory;
        });
    }

    fn source_file_jsx_fragment_type<Q>(&self, source_file: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.source_file_jsx_fragment_type_by_handle(handle)
    }

    fn source_file_jsx_fragment_type_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> Option<TypeHandle> {
        self.with_by_handle(handle, |links| links.jsx_fragment_type)
    }

    fn set_source_file_jsx_fragment_type<Q>(&self, source_file: Q, jsx_fragment_type: TypeHandle)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.set_source_file_jsx_fragment_type_by_handle(handle, jsx_fragment_type);
    }

    fn set_source_file_jsx_fragment_type_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        jsx_fragment_type: TypeHandle,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.jsx_fragment_type = Some(jsx_fragment_type);
        });
    }

    fn add_deferred_node<Q>(&self, source_file: Q, node: ast::Node)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.add_deferred_node_by_handle(handle, node);
    }

    fn add_deferred_node_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        node: ast::Node,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.deferred_nodes.add(node);
        });
    }

    fn next_deferred_node<Q>(&self, source_file: Q, index: usize) -> Option<ast::Node>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.next_deferred_node_by_handle(handle, index)
    }

    fn next_deferred_node_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        index: usize,
    ) -> Option<ast::Node> {
        self.with_by_handle(handle, |links| {
            links.deferred_nodes.values().nth(index).copied()
        })
    }

    fn clear_deferred_nodes<Q>(&self, source_file: Q)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.clear_deferred_nodes_by_handle(handle);
    }

    fn clear_deferred_nodes_by_handle(&self, handle: core::LinkHandle<SourceFileLinks>) {
        self.with_by_handle_mut(handle, |links| {
            links.deferred_nodes.clear();
        });
    }

    fn push_identifier_check_node<Q>(&self, source_file: Q, node: ast::Node)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.push_identifier_check_node_by_handle(handle, node);
    }

    fn push_identifier_check_node_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        node: ast::Node,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.identifier_check_nodes.push(node);
        });
    }

    fn identifier_check_nodes<Q>(&self, source_file: Q) -> Vec<ast::Node>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        let handle = self.source_file_link_handle(source_file);
        self.identifier_check_nodes_by_handle(handle)
    }

    fn identifier_check_nodes_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> Vec<ast::Node> {
        self.with_by_handle(handle, |links| links.identifier_check_nodes.clone())
    }
}

pub(crate) trait AliasSymbolLinksStoreExt {
    fn alias_symbol_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<AliasSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn alias_symbol_target<Q>(&self, symbol: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn alias_symbol_target_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
    ) -> Option<SymbolIdentity>;
    fn set_alias_symbol_target<Q>(&self, symbol: Q, target: Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_alias_symbol_target_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
        target: Option<SymbolIdentity>,
    );
    fn alias_symbol_immediate_target<Q>(&self, symbol: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn alias_symbol_immediate_target_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
    ) -> Option<SymbolIdentity>;
    fn set_alias_symbol_immediate_target<Q>(&self, symbol: Q, target: Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_alias_symbol_immediate_target_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
        target: Option<SymbolIdentity>,
    );
    fn alias_symbol_referenced<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn alias_symbol_referenced_by_handle(&self, handle: core::LinkHandle<AliasSymbolLinks>)
    -> bool;
    fn set_alias_symbol_referenced<Q>(&self, symbol: Q, referenced: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_alias_symbol_referenced_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
        referenced: bool,
    );
    fn mark_alias_symbol_referenced<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn mark_alias_symbol_referenced_by_handle(&self, handle: core::LinkHandle<AliasSymbolLinks>);
    fn alias_symbol_type_only_declaration<Q>(&self, symbol: Q) -> Option<ast::Node>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn alias_symbol_type_only_declaration_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
    ) -> Option<ast::Node>;
    fn set_alias_symbol_type_only_declaration<Q>(
        &self,
        symbol: Q,
        type_only_declaration: Option<ast::Node>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_alias_symbol_type_only_declaration_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
        type_only_declaration: Option<ast::Node>,
    );
    fn set_alias_symbol_type_only_declaration_if_none<Q>(
        &self,
        symbol: Q,
        type_only_declaration: Option<ast::Node>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_alias_symbol_type_only_declaration_if_none_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
        type_only_declaration: Option<ast::Node>,
    );
    fn alias_symbol_type_only_export_star_name<Q>(&self, symbol: Q) -> Option<String>
    where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn alias_symbol_type_only_export_star_name_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
    ) -> Option<String>;
    fn set_alias_symbol_type_only_export_star_name<Q>(
        &self,
        symbol: Q,
        type_only_export_star_name: Option<String>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>;
    fn set_alias_symbol_type_only_export_star_name_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
        type_only_export_star_name: Option<String>,
    );
}

impl AliasSymbolLinksStoreExt for SymbolLinkStore<AliasSymbolLinks> {
    fn alias_symbol_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<AliasSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.ensure_handle(symbol)
    }

    fn alias_symbol_target<Q>(&self, symbol: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.alias_symbol_link_handle(symbol);
        self.alias_symbol_target_by_handle(handle)
    }

    fn alias_symbol_target_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
    ) -> Option<SymbolIdentity> {
        self.with_by_handle(handle, |links| links.alias_target)
    }

    fn set_alias_symbol_target<Q>(&self, symbol: Q, target: Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.alias_symbol_link_handle(symbol);
        self.set_alias_symbol_target_by_handle(handle, target);
    }

    fn set_alias_symbol_target_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
        target: Option<SymbolIdentity>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.alias_target = target;
        });
    }

    fn alias_symbol_immediate_target<Q>(&self, symbol: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.alias_symbol_link_handle(symbol);
        self.alias_symbol_immediate_target_by_handle(handle)
    }

    fn alias_symbol_immediate_target_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
    ) -> Option<SymbolIdentity> {
        self.with_by_handle(handle, |links| links.immediate_target)
    }

    fn set_alias_symbol_immediate_target<Q>(&self, symbol: Q, target: Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.alias_symbol_link_handle(symbol);
        self.set_alias_symbol_immediate_target_by_handle(handle, target);
    }

    fn set_alias_symbol_immediate_target_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
        target: Option<SymbolIdentity>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.immediate_target = target;
        });
    }

    fn alias_symbol_referenced<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.alias_symbol_link_handle(symbol);
        self.alias_symbol_referenced_by_handle(handle)
    }

    fn alias_symbol_referenced_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
    ) -> bool {
        self.with_by_handle(handle, |links| links.referenced)
    }

    fn set_alias_symbol_referenced<Q>(&self, symbol: Q, referenced: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.alias_symbol_link_handle(symbol);
        self.set_alias_symbol_referenced_by_handle(handle, referenced);
    }

    fn set_alias_symbol_referenced_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
        referenced: bool,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.referenced = referenced;
        });
    }

    fn mark_alias_symbol_referenced<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.alias_symbol_link_handle(symbol);
        self.mark_alias_symbol_referenced_by_handle(handle);
    }

    fn mark_alias_symbol_referenced_by_handle(&self, handle: core::LinkHandle<AliasSymbolLinks>) {
        self.set_alias_symbol_referenced_by_handle(handle, true);
    }

    fn alias_symbol_type_only_declaration<Q>(&self, symbol: Q) -> Option<ast::Node>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.alias_symbol_link_handle(symbol);
        self.alias_symbol_type_only_declaration_by_handle(handle)
    }

    fn alias_symbol_type_only_declaration_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
    ) -> Option<ast::Node> {
        self.with_by_handle(handle, |links| links.type_only_declaration)
    }

    fn set_alias_symbol_type_only_declaration<Q>(
        &self,
        symbol: Q,
        type_only_declaration: Option<ast::Node>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.alias_symbol_link_handle(symbol);
        self.set_alias_symbol_type_only_declaration_by_handle(handle, type_only_declaration);
    }

    fn set_alias_symbol_type_only_declaration_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
        type_only_declaration: Option<ast::Node>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.type_only_declaration = type_only_declaration;
        });
    }

    fn set_alias_symbol_type_only_declaration_if_none<Q>(
        &self,
        symbol: Q,
        type_only_declaration: Option<ast::Node>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.alias_symbol_link_handle(symbol);
        self.set_alias_symbol_type_only_declaration_if_none_by_handle(
            handle,
            type_only_declaration,
        );
    }

    fn set_alias_symbol_type_only_declaration_if_none_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
        type_only_declaration: Option<ast::Node>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            if links.type_only_declaration.is_none() {
                links.type_only_declaration = type_only_declaration;
            }
        });
    }

    fn alias_symbol_type_only_export_star_name<Q>(&self, symbol: Q) -> Option<String>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.alias_symbol_link_handle(symbol);
        self.alias_symbol_type_only_export_star_name_by_handle(handle)
    }

    fn alias_symbol_type_only_export_star_name_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
    ) -> Option<String> {
        self.with_by_handle(handle, |links| links.type_only_export_star_name.clone())
    }

    fn set_alias_symbol_type_only_export_star_name<Q>(
        &self,
        symbol: Q,
        type_only_export_star_name: Option<String>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        let handle = self.alias_symbol_link_handle(symbol);
        self.set_alias_symbol_type_only_export_star_name_by_handle(
            handle,
            type_only_export_star_name,
        );
    }

    fn set_alias_symbol_type_only_export_star_name_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
        type_only_export_star_name: Option<String>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.type_only_export_star_name = type_only_export_star_name;
        });
    }
}

#[derive(Clone)]
pub(crate) struct CheckerBootstrapHandles {
    pub(crate) bootstrap_type: TypeHandle,
    pub(crate) bootstrap_mapper: TypeMapperHandle,
    pub(crate) bootstrap_type_predicate: TypePredicateHandle,
    pub(crate) bootstrap_signature: SignatureHandle,
    pub(crate) bootstrap_index_info: IndexInfoHandle,
}

pub struct CheckerSemanticHandles {
    pub(crate) any_type: TypeHandle,
    pub(crate) auto_type: TypeHandle,
    pub(crate) wildcard_type: TypeHandle,
    pub(crate) blocked_string_type: TypeHandle,
    pub(crate) error_type: TypeHandle,
    pub(crate) unresolved_type: TypeHandle,
    pub(crate) non_inferrable_any_type: TypeHandle,
    pub(crate) intrinsic_marker_type: TypeHandle,
    pub(crate) unknown_type: TypeHandle,
    pub(crate) undefined_type: TypeHandle,
    pub(crate) undefined_widening_type: TypeHandle,
    pub(crate) missing_type: TypeHandle,
    pub(crate) undefined_or_missing_type: TypeHandle,
    pub(crate) optional_type: TypeHandle,
    pub(crate) null_type: TypeHandle,
    pub(crate) null_widening_type: TypeHandle,
    pub(crate) string_type: TypeHandle,
    pub(crate) number_type: TypeHandle,
    pub(crate) bigint_type: TypeHandle,
    pub(crate) regular_false_type: TypeHandle,
    pub(crate) false_type: TypeHandle,
    pub(crate) regular_true_type: TypeHandle,
    pub(crate) true_type: TypeHandle,
    pub(crate) boolean_type: TypeHandle,
    pub(crate) es_symbol_type: TypeHandle,
    pub(crate) void_type: TypeHandle,
    pub(crate) never_type: TypeHandle,
    pub(crate) silent_never_type: TypeHandle,
    pub(crate) implicit_never_type: TypeHandle,
    pub(crate) unreachable_never_type: TypeHandle,
    pub(crate) non_primitive_type: TypeHandle,
    pub(crate) string_or_number_type: TypeHandle,
    pub(crate) string_number_symbol_type: TypeHandle,
    pub(crate) number_or_big_int_type: TypeHandle,
    pub(crate) template_constraint_type: TypeHandle,
    pub(crate) numeric_string_type: TypeHandle,
    pub(crate) unique_literal_type: TypeHandle,
    pub(crate) unique_literal_mapper: TypeMapperHandle,
    pub(crate) report_unreliable_mapper: TypeMapperHandle,
    pub(crate) report_unmeasurable_mapper: TypeMapperHandle,
    pub(crate) restrictive_mapper: TypeMapperHandle,
    pub(crate) permissive_mapper: TypeMapperHandle,
    pub(crate) empty_object_type: TypeHandle,
    pub(crate) empty_jsx_object_type: TypeHandle,
    pub(crate) empty_fresh_jsx_object_type: TypeHandle,
    pub(crate) empty_type_literal_type: TypeHandle,
    pub(crate) unknown_empty_object_type: TypeHandle,
    pub(crate) unknown_union_type: TypeHandle,
    pub(crate) empty_generic_type: TypeHandle,
    pub(crate) any_function_type: TypeHandle,
    pub(crate) no_constraint_type: TypeHandle,
    pub(crate) circular_constraint_type: TypeHandle,
    pub(crate) resolving_default_type: TypeHandle,
    pub(crate) marker_super_type: TypeHandle,
    pub(crate) marker_sub_type: TypeHandle,
    pub(crate) marker_other_type: TypeHandle,
    pub(crate) marker_super_type_for_check: TypeHandle,
    pub(crate) marker_sub_type_for_check: TypeHandle,
    pub(crate) no_type_predicate: TypePredicateHandle,
    pub(crate) any_signature: SignatureHandle,
    pub(crate) unknown_signature: SignatureHandle,
    pub(crate) resolving_signature: SignatureHandle,
    pub(crate) silent_never_signature: SignatureHandle,
    pub(crate) enum_number_index_info: IndexInfoHandle,
    pub(crate) any_base_type_index_info: IndexInfoHandle,
    pub(crate) global_object_type: TypeHandle,
    pub(crate) global_function_type: TypeHandle,
    pub(crate) global_callable_function_type: TypeHandle,
    pub(crate) global_newable_function_type: TypeHandle,
    pub(crate) global_array_type: TypeHandle,
    pub(crate) global_readonly_array_type: TypeHandle,
    pub(crate) global_string_type: TypeHandle,
    pub(crate) global_number_type: TypeHandle,
    pub(crate) global_boolean_type: TypeHandle,
    pub(crate) global_reg_exp_type: TypeHandle,
    pub(crate) global_this_type: TypeHandle,
    pub(crate) any_array_type: TypeHandle,
    pub(crate) auto_array_type: TypeHandle,
    pub(crate) any_readonly_array_type: TypeHandle,
    pub(crate) deferred_global_import_meta_expression_type: Option<TypeHandle>,
    pub(crate) empty_string_type: TypeHandle,
    pub(crate) zero_type: TypeHandle,
    pub(crate) zero_big_int_type: TypeHandle,
    pub(crate) typeof_type: TypeHandle,
}

impl CheckerSemanticHandles {
    pub(crate) fn from_bootstrap(handles: &CheckerBootstrapHandles) -> Self {
        let bootstrap_type = handles.bootstrap_type;
        let bootstrap_mapper = handles.bootstrap_mapper;
        let bootstrap_type_predicate = handles.bootstrap_type_predicate;
        let bootstrap_signature = handles.bootstrap_signature;
        let bootstrap_index_info = handles.bootstrap_index_info;
        Self {
            any_type: bootstrap_type,
            auto_type: bootstrap_type,
            wildcard_type: bootstrap_type,
            blocked_string_type: bootstrap_type,
            error_type: bootstrap_type,
            unresolved_type: bootstrap_type,
            non_inferrable_any_type: bootstrap_type,
            intrinsic_marker_type: bootstrap_type,
            unknown_type: bootstrap_type,
            undefined_type: bootstrap_type,
            undefined_widening_type: bootstrap_type,
            missing_type: bootstrap_type,
            undefined_or_missing_type: bootstrap_type,
            optional_type: bootstrap_type,
            null_type: bootstrap_type,
            null_widening_type: bootstrap_type,
            string_type: bootstrap_type,
            number_type: bootstrap_type,
            bigint_type: bootstrap_type,
            regular_false_type: bootstrap_type,
            false_type: bootstrap_type,
            regular_true_type: bootstrap_type,
            true_type: bootstrap_type,
            boolean_type: bootstrap_type,
            es_symbol_type: bootstrap_type,
            void_type: bootstrap_type,
            never_type: bootstrap_type,
            silent_never_type: bootstrap_type,
            implicit_never_type: bootstrap_type,
            unreachable_never_type: bootstrap_type,
            non_primitive_type: bootstrap_type,
            string_or_number_type: bootstrap_type,
            string_number_symbol_type: bootstrap_type,
            number_or_big_int_type: bootstrap_type,
            template_constraint_type: bootstrap_type,
            numeric_string_type: bootstrap_type,
            unique_literal_type: bootstrap_type,
            unique_literal_mapper: bootstrap_mapper,
            report_unreliable_mapper: bootstrap_mapper,
            report_unmeasurable_mapper: bootstrap_mapper,
            restrictive_mapper: bootstrap_mapper,
            permissive_mapper: bootstrap_mapper,
            empty_object_type: bootstrap_type,
            empty_jsx_object_type: bootstrap_type,
            empty_fresh_jsx_object_type: bootstrap_type,
            empty_type_literal_type: bootstrap_type,
            unknown_empty_object_type: bootstrap_type,
            unknown_union_type: bootstrap_type,
            empty_generic_type: bootstrap_type,
            any_function_type: bootstrap_type,
            no_constraint_type: bootstrap_type,
            circular_constraint_type: bootstrap_type,
            resolving_default_type: bootstrap_type,
            marker_super_type: bootstrap_type,
            marker_sub_type: bootstrap_type,
            marker_other_type: bootstrap_type,
            marker_super_type_for_check: bootstrap_type,
            marker_sub_type_for_check: bootstrap_type,
            no_type_predicate: bootstrap_type_predicate,
            any_signature: bootstrap_signature,
            unknown_signature: bootstrap_signature,
            resolving_signature: bootstrap_signature,
            silent_never_signature: bootstrap_signature,
            enum_number_index_info: bootstrap_index_info,
            any_base_type_index_info: bootstrap_index_info,
            global_object_type: bootstrap_type,
            global_function_type: bootstrap_type,
            global_callable_function_type: bootstrap_type,
            global_newable_function_type: bootstrap_type,
            global_array_type: bootstrap_type,
            global_readonly_array_type: bootstrap_type,
            global_string_type: bootstrap_type,
            global_number_type: bootstrap_type,
            global_boolean_type: bootstrap_type,
            global_reg_exp_type: bootstrap_type,
            global_this_type: bootstrap_type,
            any_array_type: bootstrap_type,
            auto_array_type: bootstrap_type,
            any_readonly_array_type: bootstrap_type,
            deferred_global_import_meta_expression_type: None,
            empty_string_type: bootstrap_type,
            zero_type: bootstrap_type,
            zero_big_int_type: bootstrap_type,
            typeof_type: bootstrap_type,
        }
    }
}

struct CheckerSemanticCaches {
    string_literal_types: HashMap<String, TypeHandle>,
    number_literal_types: HashMap<Number, TypeHandle>,
    bigint_literal_types: HashMap<PseudoBigInt, TypeHandle>,
    enum_literal_types: HashMap<EnumLiteralKey, TypeHandle>,
    indexed_access_types: HashMap<CacheHashKey, TypeHandle>,
    template_literal_types: HashMap<CacheHashKey, TypeHandle>,
    string_mapping_types: HashMap<StringMappingKey, TypeHandle>,
    unique_es_symbol_types: HashMap<SymbolIdentity, TypeHandle>,
    subtype_reduction_cache: HashMap<CacheHashKey, Vec<TypeHandle>>,
    cached_types: HashMap<CachedTypeKey, TypeHandle>,
    cached_signatures: HashMap<CachedSignatureKey, SignatureHandle>,
    narrowed_types: HashMap<NarrowedTypeKey, TypeHandle>,
    assignment_reduced_types: HashMap<AssignmentReducedKey, TypeHandle>,
    discriminated_contextual_types: HashMap<DiscriminatedContextualTypeKey, TypeHandle>,
    instantiation_expression_types: HashMap<InstantiationExpressionKey, TypeHandle>,
    substitution_types: HashMap<SubstitutionTypeKey, TypeHandle>,
    reverse_mapped_cache: HashMap<ReverseMappedTypeKey, Option<TypeHandle>>,
    reverse_homomorphic_mapped_cache: HashMap<ReverseMappedTypeKey, Option<TypeHandle>>,
    iteration_types_cache: HashMap<IterationTypesKey, IterationTypes>,
    error_types: HashMap<CacheHashKey, TypeHandle>,
    tuple_types: HashMap<CacheHashKey, TypeHandle>,
    union_types: HashMap<CacheHashKey, TypeHandle>,
    union_of_union_types: HashMap<UnionOfUnionKey, TypeHandle>,
    intersection_types: HashMap<CacheHashKey, TypeHandle>,
    properties_types: HashMap<PropertiesTypesKey, TypeHandle>,
    signature_links: NodeLinkStore<SignatureLinks>,
    type_node_links: NodeLinkStore<TypeNodeLinks>,
    assertion_links: NodeLinkStore<AssertionLinks>,
    switch_statement_links: NodeLinkStore<SwitchStatementLinks>,
    jsx_element_links: NodeLinkStore<JsxElementLinks>,
    value_symbol_links: SymbolLinkStore<ValueSymbolLinks>,
    mapped_symbol_links: SymbolLinkStore<MappedSymbolLinks>,
    deferred_symbol_links: SymbolLinkStore<DeferredSymbolLinks>,
    members_and_exports_links: SymbolLinkStore<MembersAndExportsLinks>,
    type_alias_links: SymbolLinkStore<TypeAliasLinks>,
    declared_type_links: SymbolLinkStore<DeclaredTypeLinks>,
    reverse_mapped_symbol_links: SymbolLinkStore<ReverseMappedSymbolLinks>,
    source_file_links: SourceFileLinkStore<SourceFileLinks>,
    pattern_for_type: HashMap<TypeHandle, ast::Node>,
    context_free_types: HashMap<ast::Node, TypeHandle>,
    binary_expression_results: HashMap<ast::Node, TypeHandle>,
    synthetic_expression_types: HashMap<ast::Node, TypeHandle>,
    flow_loop_cache: HashMap<FlowLoopKey, TypeHandle>,
    antecedent_types: Vec<TypeHandle>,
    flow_type_cache: HashMap<ast::Node, TypeHandle>,
    contextual_infos: Vec<ContextualInfo>,
    reverse_mapped_source_stack: Vec<TypeHandle>,
    reverse_mapped_target_stack: Vec<TypeHandle>,
    subtype_relation_cache: Relation,
    strict_subtype_relation_cache: Relation,
    assignable_relation_cache: Relation,
    comparable_relation_cache: Relation,
    identity_relation_cache: Relation,
    non_existent_properties: collections::Set<NonExistentPropertyKey>,
}

impl Default for CheckerSemanticCaches {
    fn default() -> Self {
        Self {
            string_literal_types: HashMap::new(),
            number_literal_types: HashMap::new(),
            bigint_literal_types: HashMap::new(),
            enum_literal_types: HashMap::new(),
            indexed_access_types: HashMap::new(),
            template_literal_types: HashMap::new(),
            string_mapping_types: HashMap::new(),
            unique_es_symbol_types: HashMap::new(),
            subtype_reduction_cache: HashMap::new(),
            cached_types: HashMap::new(),
            cached_signatures: HashMap::new(),
            narrowed_types: HashMap::new(),
            assignment_reduced_types: HashMap::new(),
            discriminated_contextual_types: HashMap::new(),
            instantiation_expression_types: HashMap::new(),
            substitution_types: HashMap::new(),
            reverse_mapped_cache: HashMap::new(),
            reverse_homomorphic_mapped_cache: HashMap::new(),
            iteration_types_cache: HashMap::new(),
            error_types: HashMap::new(),
            tuple_types: HashMap::new(),
            union_types: HashMap::new(),
            union_of_union_types: HashMap::new(),
            intersection_types: HashMap::new(),
            properties_types: HashMap::new(),
            signature_links: NodeLinkStore::default(),
            type_node_links: NodeLinkStore::default(),
            assertion_links: NodeLinkStore::default(),
            switch_statement_links: NodeLinkStore::default(),
            jsx_element_links: NodeLinkStore::default(),
            value_symbol_links: SymbolLinkStore::default(),
            mapped_symbol_links: SymbolLinkStore::default(),
            deferred_symbol_links: SymbolLinkStore::default(),
            members_and_exports_links: SymbolLinkStore::default(),
            type_alias_links: SymbolLinkStore::default(),
            declared_type_links: SymbolLinkStore::default(),
            reverse_mapped_symbol_links: SymbolLinkStore::default(),
            source_file_links: SourceFileLinkStore::default(),
            pattern_for_type: HashMap::new(),
            context_free_types: HashMap::new(),
            binary_expression_results: HashMap::new(),
            synthetic_expression_types: HashMap::new(),
            flow_loop_cache: HashMap::new(),
            antecedent_types: Vec::new(),
            flow_type_cache: HashMap::new(),
            contextual_infos: Vec::new(),
            reverse_mapped_source_stack: Vec::new(),
            reverse_mapped_target_stack: Vec::new(),
            subtype_relation_cache: Relation::new(),
            strict_subtype_relation_cache: Relation::new(),
            assignable_relation_cache: Relation::new(),
            comparable_relation_cache: Relation::new(),
            identity_relation_cache: Relation::new(),
            non_existent_properties: collections::Set::default(),
        }
    }
}

#[derive(Clone)]
pub struct TypeRecord {
    pub ts_id: TypeId,
    pub flags: TypeFlags,
    pub object_flags: ObjectFlags,
    pub symbol: Option<SymbolIdentity>,
    pub alias: Option<TypeAliasHandle>,
    pub data: TypeRecordData,
}

#[derive(Clone)]
pub enum TypeRecordData {
    Intrinsic(IntrinsicTypeRecord),
    Literal(LiteralTypeRecord),
    UniqueESSymbol(UniqueESSymbolTypeRecord),
    Object(ObjectTypeRecord),
    TypeReference(TypeReferenceRecord),
    Interface(InterfaceTypeRecord),
    Tuple(TupleTypeRecord),
    InstantiationExpression(InstantiationExpressionTypeRecord),
    Mapped(MappedTypeRecord),
    ReverseMapped(ReverseMappedTypeRecord),
    EvolvingArray(EvolvingArrayTypeRecord),
    TypeParameter(TypeParameterRecord),
    Union(UnionTypeRecord),
    Intersection(IntersectionTypeRecord),
    Index(IndexTypeRecord),
    IndexedAccess(IndexedAccessTypeRecord),
    TemplateLiteral(TemplateLiteralTypeRecord),
    StringMapping(StringMappingTypeRecord),
    Substitution(SubstitutionTypeRecord),
    Conditional(ConditionalTypeRecord),
}

impl TypeRecord {
    pub fn flags(&self) -> TypeFlags {
        self.flags
    }

    pub fn object_flags(&self) -> ObjectFlags {
        self.object_flags
    }

    pub fn as_intrinsic_type(&self) -> &IntrinsicTypeRecord {
        self.data.as_intrinsic_type()
    }

    pub fn as_literal_type(&self) -> &LiteralTypeRecord {
        self.data.as_literal_type()
    }

    pub fn as_unique_es_symbol_type(&self) -> &UniqueESSymbolTypeRecord {
        self.data.as_unique_es_symbol_type()
    }

    pub fn as_tuple_type(&self) -> &TupleTypeRecord {
        self.data.as_tuple_type()
    }

    pub fn as_instantiation_expression_type(&self) -> &InstantiationExpressionTypeRecord {
        self.data.as_instantiation_expression_type()
    }

    pub fn as_mapped_type(&self) -> &MappedTypeRecord {
        self.data.as_mapped_type()
    }

    pub fn as_reverse_mapped_type(&self) -> &ReverseMappedTypeRecord {
        self.data.as_reverse_mapped_type()
    }

    pub fn as_evolving_array_type(&self) -> &EvolvingArrayTypeRecord {
        self.data.as_evolving_array_type()
    }

    pub fn as_evolving_array_type_mut(&mut self) -> &mut EvolvingArrayTypeRecord {
        self.data.as_evolving_array_type_mut()
    }

    pub fn as_type_parameter(&self) -> &TypeParameterRecord {
        self.data.as_type_parameter()
    }

    pub fn as_union_type(&self) -> &UnionTypeRecord {
        self.data.as_union_type()
    }

    pub fn as_intersection_type(&self) -> &IntersectionTypeRecord {
        self.data.as_intersection_type()
    }

    pub fn as_index_type(&self) -> &IndexTypeRecord {
        self.data.as_index_type()
    }

    pub fn as_indexed_access_type(&self) -> &IndexedAccessTypeRecord {
        self.data.as_indexed_access_type()
    }

    pub fn as_constrained_type(&self) -> Option<&ConstrainedTypeRecord> {
        self.data.as_constrained_type()
    }

    pub fn as_constrained_type_mut(&mut self) -> Option<&mut ConstrainedTypeRecord> {
        self.data.as_constrained_type_mut()
    }

    pub fn as_template_literal_type(&self) -> &TemplateLiteralTypeRecord {
        self.data.as_template_literal_type()
    }

    pub fn as_string_mapping_type(&self) -> &StringMappingTypeRecord {
        self.data.as_string_mapping_type()
    }

    pub fn as_substitution_type(&self) -> &SubstitutionTypeRecord {
        self.data.as_substitution_type()
    }

    pub fn as_conditional_type(&self) -> &ConditionalTypeRecord {
        self.data.as_conditional_type()
    }

    pub fn as_object_type(&self) -> Option<&ObjectTypeRecord> {
        self.data.as_object_type()
    }

    pub fn as_type_reference(&self) -> Option<&TypeReferenceRecord> {
        self.data.as_type_reference()
    }

    pub fn as_interface_type(&self) -> Option<&InterfaceTypeRecord> {
        self.data.as_interface_type()
    }

    pub fn as_union_or_intersection_type(&self) -> Option<&UnionOrIntersectionTypeRecord> {
        self.data.as_union_or_intersection_type()
    }
}

impl TypeRecordData {
    pub fn as_intrinsic_type(&self) -> &IntrinsicTypeRecord {
        match self {
            Self::Intrinsic(record) => record,
            _ => panic!("type record is not intrinsic"),
        }
    }

    pub fn as_literal_type(&self) -> &LiteralTypeRecord {
        match self {
            Self::Literal(record) => record,
            _ => panic!("type record is not literal"),
        }
    }

    pub fn as_literal_type_mut(&mut self) -> &mut LiteralTypeRecord {
        match self {
            Self::Literal(record) => record,
            _ => panic!("type record is not literal"),
        }
    }

    pub fn as_unique_es_symbol_type(&self) -> &UniqueESSymbolTypeRecord {
        match self {
            Self::UniqueESSymbol(record) => record,
            _ => panic!("type record is not unique ES symbol"),
        }
    }

    pub fn as_tuple_type(&self) -> &TupleTypeRecord {
        match self {
            Self::Tuple(record) => record,
            _ => panic!("type record is not tuple"),
        }
    }

    pub fn as_tuple_type_mut(&mut self) -> &mut TupleTypeRecord {
        match self {
            Self::Tuple(record) => record,
            _ => panic!("type record is not tuple"),
        }
    }

    pub fn as_instantiation_expression_type(&self) -> &InstantiationExpressionTypeRecord {
        match self {
            Self::InstantiationExpression(record) => record,
            _ => panic!("type record is not instantiation expression"),
        }
    }

    pub fn as_instantiation_expression_type_mut(
        &mut self,
    ) -> &mut InstantiationExpressionTypeRecord {
        match self {
            Self::InstantiationExpression(record) => record,
            _ => panic!("type record is not instantiation expression"),
        }
    }

    pub fn as_mapped_type(&self) -> &MappedTypeRecord {
        match self {
            Self::Mapped(record) => record,
            _ => panic!("type record is not mapped"),
        }
    }

    pub fn as_mapped_type_mut(&mut self) -> &mut MappedTypeRecord {
        match self {
            Self::Mapped(record) => record,
            _ => panic!("type record is not mapped"),
        }
    }

    pub fn as_reverse_mapped_type(&self) -> &ReverseMappedTypeRecord {
        match self {
            Self::ReverseMapped(record) => record,
            _ => panic!("type record is not reverse mapped"),
        }
    }

    pub fn as_reverse_mapped_type_mut(&mut self) -> &mut ReverseMappedTypeRecord {
        match self {
            Self::ReverseMapped(record) => record,
            _ => panic!("type record is not reverse mapped"),
        }
    }

    pub fn as_evolving_array_type(&self) -> &EvolvingArrayTypeRecord {
        match self {
            Self::EvolvingArray(record) => record,
            _ => panic!("type record is not evolving array"),
        }
    }

    pub fn as_evolving_array_type_mut(&mut self) -> &mut EvolvingArrayTypeRecord {
        match self {
            Self::EvolvingArray(record) => record,
            _ => panic!("type record is not evolving array"),
        }
    }

    pub fn as_type_parameter(&self) -> &TypeParameterRecord {
        match self {
            Self::TypeParameter(record) => record,
            _ => panic!("type record is not type parameter"),
        }
    }

    pub fn as_type_parameter_mut(&mut self) -> &mut TypeParameterRecord {
        match self {
            Self::TypeParameter(record) => record,
            _ => panic!("type record is not type parameter"),
        }
    }

    pub fn as_constrained_type(&self) -> Option<&ConstrainedTypeRecord> {
        match self {
            Self::Object(record) => Some(&record.structured.constrained),
            Self::TypeReference(record) => Some(&record.object.structured.constrained),
            Self::Interface(record) => Some(&record.type_reference.object.structured.constrained),
            Self::Tuple(record) => Some(
                &record
                    .interface
                    .type_reference
                    .object
                    .structured
                    .constrained,
            ),
            Self::InstantiationExpression(record) => Some(&record.object.structured.constrained),
            Self::Mapped(record) => Some(&record.object.structured.constrained),
            Self::ReverseMapped(record) => Some(&record.object.structured.constrained),
            Self::EvolvingArray(record) => Some(&record.object.structured.constrained),
            Self::TypeParameter(record) => Some(&record.constrained),
            Self::Union(record) => Some(&record.union_or_intersection.structured.constrained),
            Self::Intersection(record) => {
                Some(&record.union_or_intersection.structured.constrained)
            }
            Self::Index(record) => Some(&record.constrained),
            Self::IndexedAccess(record) => Some(&record.constrained),
            Self::TemplateLiteral(record) => Some(&record.constrained),
            Self::StringMapping(record) => Some(&record.constrained),
            Self::Substitution(record) => Some(&record.constrained),
            Self::Conditional(record) => Some(&record.constrained),
            Self::Intrinsic(_) | Self::Literal(_) | Self::UniqueESSymbol(_) => None,
        }
    }

    pub fn as_constrained_type_mut(&mut self) -> Option<&mut ConstrainedTypeRecord> {
        match self {
            Self::Object(record) => Some(&mut record.structured.constrained),
            Self::TypeReference(record) => Some(&mut record.object.structured.constrained),
            Self::Interface(record) => {
                Some(&mut record.type_reference.object.structured.constrained)
            }
            Self::Tuple(record) => Some(
                &mut record
                    .interface
                    .type_reference
                    .object
                    .structured
                    .constrained,
            ),
            Self::InstantiationExpression(record) => {
                Some(&mut record.object.structured.constrained)
            }
            Self::Mapped(record) => Some(&mut record.object.structured.constrained),
            Self::ReverseMapped(record) => Some(&mut record.object.structured.constrained),
            Self::EvolvingArray(record) => Some(&mut record.object.structured.constrained),
            Self::TypeParameter(record) => Some(&mut record.constrained),
            Self::Union(record) => Some(&mut record.union_or_intersection.structured.constrained),
            Self::Intersection(record) => {
                Some(&mut record.union_or_intersection.structured.constrained)
            }
            Self::Index(record) => Some(&mut record.constrained),
            Self::IndexedAccess(record) => Some(&mut record.constrained),
            Self::TemplateLiteral(record) => Some(&mut record.constrained),
            Self::StringMapping(record) => Some(&mut record.constrained),
            Self::Substitution(record) => Some(&mut record.constrained),
            Self::Conditional(record) => Some(&mut record.constrained),
            Self::Intrinsic(_) | Self::Literal(_) | Self::UniqueESSymbol(_) => None,
        }
    }

    pub fn as_union_type(&self) -> &UnionTypeRecord {
        match self {
            Self::Union(record) => record,
            _ => panic!("type record is not union"),
        }
    }

    pub fn as_union_type_mut(&mut self) -> &mut UnionTypeRecord {
        match self {
            Self::Union(record) => record,
            _ => panic!("type record is not union"),
        }
    }

    pub fn as_intersection_type(&self) -> &IntersectionTypeRecord {
        match self {
            Self::Intersection(record) => record,
            _ => panic!("type record is not intersection"),
        }
    }

    pub fn as_intersection_type_mut(&mut self) -> &mut IntersectionTypeRecord {
        match self {
            Self::Intersection(record) => record,
            _ => panic!("type record is not intersection"),
        }
    }

    pub fn as_index_type(&self) -> &IndexTypeRecord {
        match self {
            Self::Index(record) => record,
            _ => panic!("type record is not index"),
        }
    }

    pub fn as_indexed_access_type(&self) -> &IndexedAccessTypeRecord {
        match self {
            Self::IndexedAccess(record) => record,
            _ => panic!("type record is not indexed access"),
        }
    }

    pub fn as_template_literal_type(&self) -> &TemplateLiteralTypeRecord {
        match self {
            Self::TemplateLiteral(record) => record,
            _ => panic!("type record is not template literal"),
        }
    }

    pub fn as_string_mapping_type(&self) -> &StringMappingTypeRecord {
        match self {
            Self::StringMapping(record) => record,
            _ => panic!("type record is not string mapping"),
        }
    }

    pub fn as_substitution_type(&self) -> &SubstitutionTypeRecord {
        match self {
            Self::Substitution(record) => record,
            _ => panic!("type record is not substitution"),
        }
    }

    pub fn as_conditional_type(&self) -> &ConditionalTypeRecord {
        match self {
            Self::Conditional(record) => record,
            _ => panic!("type record is not conditional"),
        }
    }

    pub fn as_conditional_type_mut(&mut self) -> &mut ConditionalTypeRecord {
        match self {
            Self::Conditional(record) => record,
            _ => panic!("type record is not conditional"),
        }
    }

    pub fn as_object_type(&self) -> Option<&ObjectTypeRecord> {
        match self {
            Self::Object(record) => Some(record),
            Self::TypeReference(record) => Some(&record.object),
            Self::Interface(record) => Some(&record.type_reference.object),
            Self::Tuple(record) => Some(&record.interface.type_reference.object),
            Self::InstantiationExpression(record) => Some(&record.object),
            Self::Mapped(record) => Some(&record.object),
            Self::ReverseMapped(record) => Some(&record.object),
            Self::EvolvingArray(record) => Some(&record.object),
            _ => None,
        }
    }

    pub fn as_structured_type(&self) -> Option<&StructuredTypeRecord> {
        match self {
            Self::Object(record) => Some(&record.structured),
            Self::TypeReference(record) => Some(&record.object.structured),
            Self::Interface(record) => Some(&record.type_reference.object.structured),
            Self::Tuple(record) => Some(&record.interface.type_reference.object.structured),
            Self::InstantiationExpression(record) => Some(&record.object.structured),
            Self::Mapped(record) => Some(&record.object.structured),
            Self::ReverseMapped(record) => Some(&record.object.structured),
            Self::EvolvingArray(record) => Some(&record.object.structured),
            Self::Union(record) => Some(&record.union_or_intersection.structured),
            Self::Intersection(record) => Some(&record.union_or_intersection.structured),
            _ => None,
        }
    }

    pub fn as_object_type_mut(&mut self) -> Option<&mut ObjectTypeRecord> {
        match self {
            Self::Object(record) => Some(record),
            Self::TypeReference(record) => Some(&mut record.object),
            Self::Interface(record) => Some(&mut record.type_reference.object),
            Self::Tuple(record) => Some(&mut record.interface.type_reference.object),
            Self::InstantiationExpression(record) => Some(&mut record.object),
            Self::Mapped(record) => Some(&mut record.object),
            Self::ReverseMapped(record) => Some(&mut record.object),
            Self::EvolvingArray(record) => Some(&mut record.object),
            _ => None,
        }
    }

    pub fn as_structured_type_mut(&mut self) -> Option<&mut StructuredTypeRecord> {
        match self {
            Self::Object(record) => Some(&mut record.structured),
            Self::TypeReference(record) => Some(&mut record.object.structured),
            Self::Interface(record) => Some(&mut record.type_reference.object.structured),
            Self::Tuple(record) => Some(&mut record.interface.type_reference.object.structured),
            Self::InstantiationExpression(record) => Some(&mut record.object.structured),
            Self::Mapped(record) => Some(&mut record.object.structured),
            Self::ReverseMapped(record) => Some(&mut record.object.structured),
            Self::EvolvingArray(record) => Some(&mut record.object.structured),
            Self::Union(record) => Some(&mut record.union_or_intersection.structured),
            Self::Intersection(record) => Some(&mut record.union_or_intersection.structured),
            _ => None,
        }
    }

    pub fn as_type_reference(&self) -> Option<&TypeReferenceRecord> {
        match self {
            Self::TypeReference(record) => Some(record),
            Self::Interface(record) => Some(&record.type_reference),
            Self::Tuple(record) => Some(&record.interface.type_reference),
            _ => None,
        }
    }

    pub fn as_type_reference_mut(&mut self) -> Option<&mut TypeReferenceRecord> {
        match self {
            Self::TypeReference(record) => Some(record),
            Self::Interface(record) => Some(&mut record.type_reference),
            Self::Tuple(record) => Some(&mut record.interface.type_reference),
            _ => None,
        }
    }

    pub fn as_interface_type(&self) -> Option<&InterfaceTypeRecord> {
        match self {
            Self::Interface(record) => Some(record),
            Self::Tuple(record) => Some(&record.interface),
            _ => None,
        }
    }

    pub fn as_interface_type_mut(&mut self) -> Option<&mut InterfaceTypeRecord> {
        match self {
            Self::Interface(record) => Some(record),
            Self::Tuple(record) => Some(&mut record.interface),
            _ => None,
        }
    }

    pub fn as_union_or_intersection_type(&self) -> Option<&UnionOrIntersectionTypeRecord> {
        match self {
            Self::Union(record) => Some(&record.union_or_intersection),
            Self::Intersection(record) => Some(&record.union_or_intersection),
            _ => None,
        }
    }

    pub fn as_union_or_intersection_type_mut(
        &mut self,
    ) -> Option<&mut UnionOrIntersectionTypeRecord> {
        match self {
            Self::Union(record) => Some(&mut record.union_or_intersection),
            Self::Intersection(record) => Some(&mut record.union_or_intersection),
            _ => None,
        }
    }
}

#[derive(Clone, Default)]
pub struct IntrinsicTypeRecord {
    pub intrinsic_name: String,
}

#[derive(Clone)]
pub struct LiteralTypeRecord {
    pub value: Any,
    pub fresh_type: Option<TypeHandle>,
    pub regular_type: Option<TypeHandle>,
}

#[derive(Clone, Default)]
pub struct UniqueESSymbolTypeRecord {
    pub name: String,
}

#[derive(Clone, Default)]
pub struct ConstrainedTypeRecord {
    pub resolved_base_constraint: Option<TypeHandle>,
}

#[derive(Clone, Default)]
pub struct StructuredTypeRecord {
    pub constrained: ConstrainedTypeRecord,
    pub members: SymbolIdentityTable,
    pub properties: Vec<SymbolIdentity>,
    pub signatures: Vec<SignatureHandle>,
    pub call_signature_count: usize,
    pub index_infos: Vec<IndexInfoHandle>,
    pub object_type_without_abstract_construct_signatures: Option<TypeHandle>,
}

impl StructuredTypeRecord {
    pub fn properties(&self) -> &[SymbolIdentity] {
        &self.properties
    }

    pub fn collect_properties(&self) -> Vec<SymbolIdentity> {
        self.properties().to_vec()
    }

    pub fn call_signatures(&self) -> &[SignatureHandle] {
        &self.signatures[..self.call_signature_count]
    }

    pub fn collect_call_signatures(&self) -> Vec<SignatureHandle> {
        self.call_signatures().to_vec()
    }

    pub fn construct_signatures(&self) -> &[SignatureHandle] {
        &self.signatures[self.call_signature_count..]
    }

    pub fn collect_construct_signatures(&self) -> Vec<SignatureHandle> {
        self.construct_signatures().to_vec()
    }

    pub fn index_infos(&self) -> &[IndexInfoHandle] {
        &self.index_infos
    }

    pub fn collect_index_infos(&self) -> Vec<IndexInfoHandle> {
        self.index_infos().to_vec()
    }
}

#[derive(Clone, Default)]
pub struct ObjectTypeRecord {
    pub structured: StructuredTypeRecord,
    pub target: Option<TypeHandle>,
    pub mapper: Option<TypeMapperHandle>,
    pub instantiations: HashMap<CacheHashKey, TypeHandle>,
}

#[derive(Clone, Default)]
pub struct TypeReferenceRecord {
    pub object: ObjectTypeRecord,
    pub node: Option<ast::Node>,
    pub resolved_type_arguments: Option<Vec<TypeHandle>>,
}

#[derive(Clone, Default)]
pub struct InterfaceTypeRecord {
    pub type_reference: TypeReferenceRecord,
    pub all_type_parameters: Vec<TypeHandle>,
    pub outer_type_parameter_count: usize,
    pub this_type: Option<TypeHandle>,
    pub base_types_resolved: bool,
    pub declared_members_resolved: bool,
    pub resolved_base_constructor_type: Option<TypeHandle>,
    pub resolved_base_types: Vec<TypeHandle>,
    pub declared_members: SymbolIdentityTable,
    pub declared_call_signatures: Vec<SignatureHandle>,
    pub declared_construct_signatures: Vec<SignatureHandle>,
    pub declared_index_infos: Vec<IndexInfoHandle>,
}

#[derive(Clone, Default)]
pub struct TupleTypeRecord {
    pub interface: InterfaceTypeRecord,
    pub element_infos: Vec<TupleElementInfo>,
    pub min_length: usize,
    pub fixed_length: usize,
    pub combined_flags: ElementFlags,
    pub readonly: bool,
}

#[derive(Clone, Default)]
pub struct InstantiationExpressionTypeRecord {
    pub object: ObjectTypeRecord,
    pub node: Option<ast::Node>,
}

#[derive(Clone, Default)]
pub struct MappedTypeRecord {
    pub object: ObjectTypeRecord,
    pub declaration: Option<ast::Node>,
    pub type_parameter: Option<TypeHandle>,
    pub constraint_type: Option<TypeHandle>,
    pub name_type: Option<TypeHandle>,
    pub template_type: Option<TypeHandle>,
    pub modifiers_type: Option<TypeHandle>,
    pub resolved_apparent_type: Option<TypeHandle>,
    pub contains_error: bool,
}

#[derive(Clone, Default)]
pub struct ReverseMappedTypeRecord {
    pub object: ObjectTypeRecord,
    pub source: Option<TypeHandle>,
    pub mapped_type: Option<TypeHandle>,
    pub constraint_type: Option<TypeHandle>,
}

#[derive(Clone, Default)]
pub struct EvolvingArrayTypeRecord {
    pub object: ObjectTypeRecord,
    pub element_type: Option<TypeHandle>,
    pub final_array_type: Option<TypeHandle>,
}

#[derive(Clone, Default)]
pub struct UnionOrIntersectionTypeRecord {
    pub structured: StructuredTypeRecord,
    pub types: Vec<TypeHandle>,
    pub property_cache: SymbolIdentityTable,
    pub property_cache_without_function_property_augment: SymbolIdentityTable,
    pub resolved_properties: Vec<SymbolIdentity>,
}

#[derive(Clone, Default)]
pub struct UnionTypeRecord {
    pub union_or_intersection: UnionOrIntersectionTypeRecord,
    pub resolved_reduced_type: Option<TypeHandle>,
    pub regular_type: Option<TypeHandle>,
    pub origin: Option<TypeHandle>,
    pub key_property_name: String,
    pub constituent_map: HashMap<TypeHandle, TypeHandle>,
}

#[derive(Clone, Default)]
pub struct IntersectionTypeRecord {
    pub union_or_intersection: UnionOrIntersectionTypeRecord,
    pub resolved_apparent_type: Option<TypeHandle>,
    pub unique_literal_filled_instantiation: Option<TypeHandle>,
}

#[derive(Clone, Default)]
pub struct TypeParameterRecord {
    pub constrained: ConstrainedTypeRecord,
    pub constraint: Option<TypeHandle>,
    pub target: Option<TypeHandle>,
    pub mapper: Option<TypeMapperHandle>,
    pub is_this_type: bool,
    pub resolved_default_type: Option<TypeHandle>,
}

#[derive(Clone, Default)]
pub struct IndexTypeRecord {
    pub constrained: ConstrainedTypeRecord,
    pub target: Option<TypeHandle>,
    pub index_flags: IndexFlags,
}

#[derive(Clone, Default)]
pub struct IndexedAccessTypeRecord {
    pub constrained: ConstrainedTypeRecord,
    pub object_type: Option<TypeHandle>,
    pub index_type: Option<TypeHandle>,
    pub access_flags: AccessFlags,
}

#[derive(Clone, Default)]
pub struct TemplateLiteralTypeRecord {
    pub constrained: ConstrainedTypeRecord,
    pub texts: Arc<[String]>,
    pub types: Arc<[TypeHandle]>,
}

impl TemplateLiteralTypeRecord {
    pub fn texts_equal(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.texts, &other.texts) || self.texts.as_ref() == other.texts.as_ref()
    }
}

#[derive(Clone, Default)]
pub struct StringMappingTypeRecord {
    pub constrained: ConstrainedTypeRecord,
    pub target: Option<TypeHandle>,
}

#[derive(Clone, Default)]
pub struct SubstitutionTypeRecord {
    pub constrained: ConstrainedTypeRecord,
    pub base_type: Option<TypeHandle>,
    pub constraint: Option<TypeHandle>,
}

#[derive(Clone, Default)]
pub struct ConditionalTypeRecord {
    pub constrained: ConstrainedTypeRecord,
    pub root: Option<ConditionalRootHandle>,
    pub check_type: Option<TypeHandle>,
    pub extends_type: Option<TypeHandle>,
    pub resolved_true_type: Option<TypeHandle>,
    pub resolved_false_type: Option<TypeHandle>,
    pub resolved_inferred_true_type: Option<TypeHandle>,
    pub resolved_default_constraint: Option<TypeHandle>,
    pub resolved_constraint_of_distributive: Option<TypeHandle>,
    pub mapper: Option<TypeMapperHandle>,
    pub combined_mapper: Option<TypeMapperHandle>,
}

#[derive(Clone, Debug)]
pub struct SignatureRecord {
    pub flags: SignatureFlags,
    pub min_argument_count: i32,
    pub resolved_min_argument_count: i32,
    pub declaration: Option<ast::Node>,
    pub type_parameters: Vec<TypeHandle>,
    pub parameters: Arc<[SymbolIdentity]>,
    pub this_parameter: Option<SymbolIdentity>,
    pub resolved_return_type: Option<TypeHandle>,
    pub resolved_type_predicate: Option<TypePredicateHandle>,
    pub target: Option<SignatureHandle>,
    pub mapper: Option<TypeMapperHandle>,
    pub isolated_signature_type: Option<TypeHandle>,
    pub composite: Option<CompositeSignatureRecord>,
}

#[derive(Clone, Debug)]
pub struct CompositeSignatureRecord {
    pub is_union: bool,
    pub signatures: Vec<SignatureHandle>,
}

#[derive(Clone)]
pub struct CompositeSignature {
    pub is_union: bool,
    pub signatures: Vec<SignatureHandle>,
}

impl From<&CompositeSignature> for CompositeSignatureRecord {
    fn from(composite: &CompositeSignature) -> Self {
        Self {
            is_union: composite.is_union,
            signatures: composite.signatures.clone(),
        }
    }
}

#[derive(Clone)]
pub struct TypeMapperRecord {
    pub data: TypeMapperRecordData,
}

pub type TypeMapperList = SmallVec<[TypeHandle; 2]>;

#[derive(Clone)]
pub enum TypeMapperRecordData {
    Identity,
    Simple(SimpleTypeMapperRecord),
    Array(ArrayTypeMapperRecord),
    ArrayToSingle(ArrayToSingleTypeMapperRecord),
    Deferred(DeferredTypeMapperRecord),
    Function(FunctionTypeMapperRecord),
    Merged(MergedTypeMapperRecord),
    Composite(CompositeTypeMapperRecord),
    Inference(InferenceTypeMapperRecord),
}

#[derive(Clone)]
pub struct SimpleTypeMapperRecord {
    pub source: TypeHandle,
    pub target: TypeHandle,
}

#[derive(Clone)]
pub struct ArrayTypeMapperRecord {
    pub sources: TypeMapperList,
    pub targets: TypeMapperList,
}

#[derive(Clone)]
pub struct ArrayToSingleTypeMapperRecord {
    pub sources: TypeMapperList,
    pub target: TypeHandle,
}

#[derive(Clone)]
pub struct DeferredTypeMapperRecord {
    pub identity: u64,
    pub sources: TypeMapperList,
    pub targets: Vec<DeferredTypeMapperTarget>,
}

#[derive(Clone)]
pub enum DeferredTypeMapperTarget {
    EffectiveTypeArgumentAtIndex {
        parent: ast::Node,
        type_parameters: Vec<TypeHandle>,
        index: usize,
    },
}

#[derive(Clone)]
pub struct FunctionTypeMapperRecord {
    pub kind: FunctionTypeMapperRecordKind,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FunctionTypeMapperRecordKind {
    UniqueLiteral,
    ReportUnreliable,
    ReportUnmeasurable,
    Restrictive,
    Permissive,
}

#[derive(Clone)]
pub struct MergedTypeMapperRecord {
    pub left: TypeMapperHandle,
    pub right: TypeMapperHandle,
}

#[derive(Clone)]
pub struct CompositeTypeMapperRecord {
    pub left: TypeMapperHandle,
    pub right: TypeMapperHandle,
}

#[derive(Clone)]
pub struct InferenceTypeMapperRecord {
    pub identity: u64,
    pub context: InferenceContextHandle,
    pub fixing: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TypeMapperAllocationCounters {
    pub identity: u64,
    pub simple: u64,
    pub array: u64,
    pub array_to_single: u64,
    pub deferred: u64,
    pub function: u64,
    pub merged: u64,
    pub composite: u64,
    pub inference: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TypeMapperPerfCounters {
    pub allocations: TypeMapperAllocationCounters,
    pub max_chain_depth: u32,
    pub total_chain_depth: u64,
}

impl TypeMapperPerfCounters {
    pub fn accumulate(&mut self, other: Self) {
        self.allocations.identity += other.allocations.identity;
        self.allocations.simple += other.allocations.simple;
        self.allocations.array += other.allocations.array;
        self.allocations.array_to_single += other.allocations.array_to_single;
        self.allocations.deferred += other.allocations.deferred;
        self.allocations.function += other.allocations.function;
        self.allocations.merged += other.allocations.merged;
        self.allocations.composite += other.allocations.composite;
        self.allocations.inference += other.allocations.inference;
        self.max_chain_depth = self.max_chain_depth.max(other.max_chain_depth);
        self.total_chain_depth += other.total_chain_depth;
    }

    fn record_allocation(&mut self, data: &TypeMapperRecordData, chain_depth: u32) {
        match data {
            TypeMapperRecordData::Identity => self.allocations.identity += 1,
            TypeMapperRecordData::Simple(_) => self.allocations.simple += 1,
            TypeMapperRecordData::Array(_) => self.allocations.array += 1,
            TypeMapperRecordData::ArrayToSingle(_) => self.allocations.array_to_single += 1,
            TypeMapperRecordData::Deferred(_) => self.allocations.deferred += 1,
            TypeMapperRecordData::Function(_) => self.allocations.function += 1,
            TypeMapperRecordData::Merged(_) => self.allocations.merged += 1,
            TypeMapperRecordData::Composite(_) => self.allocations.composite += 1,
            TypeMapperRecordData::Inference(_) => self.allocations.inference += 1,
        }
        self.max_chain_depth = self.max_chain_depth.max(chain_depth);
        self.total_chain_depth += u64::from(chain_depth);
    }
}

#[derive(Clone, Debug)]
pub struct IndexInfoRecord {
    pub key_type: Option<TypeHandle>,
    pub value_type: Option<TypeHandle>,
    pub is_readonly: bool,
    pub declaration: Option<ast::Node>,
    pub index_symbol: Option<SymbolIdentity>,
    pub components: Vec<ast::Node>,
}

#[derive(Clone, Debug)]
pub struct TypeAliasRecord {
    pub symbol: Option<SymbolIdentity>,
    pub type_arguments: Vec<TypeHandle>,
}

#[derive(Clone, Debug)]
pub struct TypePredicateRecord {
    pub kind: TypePredicateKind,
    pub parameter_index: i32,
    pub parameter_name: String,
    pub t: Option<TypeHandle>,
}

#[derive(Clone, Debug)]
pub struct ConditionalRootRecord {
    pub node: Option<ast::ConditionalTypeNodeNode>,
    pub check_type: Option<TypeHandle>,
    pub extends_type: Option<TypeHandle>,
    pub is_distributive: bool,
    pub infer_type_parameters: Vec<TypeHandle>,
    pub outer_type_parameters: Vec<TypeHandle>,
    pub instantiations: HashMap<CacheHashKey, TypeHandle>,
    pub alias: Option<TypeAliasHandle>,
}

#[derive(Clone)]
pub struct InferenceContextRecord {
    pub inferences: Vec<InferenceInfo>,
    pub signature: Option<SignatureHandle>,
    pub flags: InferenceFlags,
    pub compare_types: TypeComparer,
    pub mapper: Option<TypeMapperHandle>,
    pub non_fixing_mapper: Option<TypeMapperHandle>,
    pub return_mapper: Option<TypeMapperHandle>,
    pub outer_return_mapper: Option<TypeMapperHandle>,
    pub inferred_type_parameters: Vec<TypeHandle>,
    pub intra_expression_inference_sites: Vec<IntraExpressionInferenceSite>,
}

pub struct CheckerState {
    identity: CheckerStateIdentity,
    type_count: TypeId,
    symbol_count: u32,
    total_instantiation_count: u32,
    instantiation_count: u32,
    instantiation_depth: u32,
    next_type_mapper_identity: u64,
    reliability_flags: RelationComparisonResult,
    is_inference_partially_blocked: bool,
    inline_level: isize,
    was_canceled: bool,
    save_deferred_diagnostics: bool,
    can_collect_symbol_alias_accessibility_data: bool,
    ctx: core::Context,
    options: CheckerDerivedOptions,
    factory: ast::NodeFactory,
    diagnostics: ast::DiagnosticsCollection,
    suggestion_diagnostics: ast::DiagnosticsCollection,
    globals: SymbolIdentityTable,
    symbols: SymbolRegistry,
    undefined_symbol: Option<SymbolIdentity>,
    arguments_symbol: Option<SymbolIdentity>,
    require_symbol: Option<SymbolIdentity>,
    unknown_symbol: Option<SymbolIdentity>,
    global_this_symbol: Option<SymbolIdentity>,
    pattern_ambient_module_augmentations: SymbolIdentityTable,
    pattern_ambient_modules: Vec<PatternAmbientModuleRecord>,
    ambient_modules_once: Once,
    ambient_modules: Vec<SymbolIdentity>,
    node_links: NodeLinkStore<NodeLinks>,
    symbol_node_links: NodeLinkStore<SymbolNodeLinks>,
    enum_member_links: NodeLinkStore<EnumMemberLinks>,
    array_literal_links: NodeLinkStore<ArrayLiteralLinks>,
    jsx_links: NodeLinkStore<JSXLinks>,
    declaration_links: NodeLinkStore<DeclarationLinks>,
    declaration_file_links: SourceFileLinkStore<DeclarationFileLinks>,
    jsx_namespace: String,
    jsx_factory_entity: Option<ast::Node>,
    symbol_reference_links: SymbolLinkStore<SymbolReferenceLinks>,
    alias_symbol_links: SymbolLinkStore<AliasSymbolLinks>,
    module_symbol_links: SymbolLinkStore<ModuleSymbolLinks>,
    late_bound_links: SymbolLinkStore<LateBoundLinks>,
    export_type_links: SymbolLinkStore<ExportTypeLinks>,
    spread_links: SymbolLinkStore<SpreadLinks>,
    variance_links: SymbolLinkStore<VarianceLinks>,
    marked_assignment_symbol_links: SymbolLinkStore<MarkedAssignmentSymbolLinks>,
    symbol_container_links: SymbolLinkStore<ContainingSymbolLinks>,
    array_variances: Vec<VarianceFlags>,
    undefined_properties: HashMap<String, SymbolIdentity>,
    unresolved_symbols: HashMap<String, SymbolIdentity>,
    merged_symbols: MergedSymbolStore,
    primitive_type_alias_suggestions: HashMap<&'static str, SymbolIdentity>,
    this_expando_kinds: HashMap<SymbolIdentity, ThisAssignmentDeclarationKind>,
    this_expando_locations: HashMap<SymbolIdentity, Option<ast::Node>>,
    marker_types: HashSet<TypeHandle>,
    resolving_union_or_intersection_properties: Vec<(TypeId, String, bool)>,
    awaited_type_stack: Vec<TypeId>,
    cached_arguments_referenced: HashMap<ast::Node, bool>,
    contextual_binding_patterns: Vec<ast::Node>,
    renamed_binding_elements_in_types: Vec<ast::Node>,
    skip_direct_inference_nodes: Set<ast::Node>,
    reported_unreachable_nodes: Set<ast::Node>,
    synthetic_node_symbols: HashMap<ast::Node, SymbolIdentity>,
    synthetic_node_locals: HashMap<ast::Node, SymbolIdentityTable>,
    packages_map: HashMap<String, bool>,
    current_node: Option<ast::Node>,
    current_source_file: Option<SourceFileIdentity>,
    reg_exp_scanner: Option<scanner::Scanner>,
    within_unreachable_code: bool,
    flow_analysis_disabled: bool,
    flow_invocation_count: isize,
    last_flow_node: Option<ast::FlowRef>,
    last_flow_node_reachable: bool,
    flow_node_reachable: ast::FlowRefSideTable<bool>,
    flow_node_post_super: ast::FlowRefSideTable<bool>,
    resolution_start: isize,
    in_variance_computation: bool,
    apparent_argument_count: Option<isize>,
    enum_relation: HashMap<EnumRelationKey, RelationComparisonResult>,
    last_get_combined_node_flags_node: Option<ast::Node>,
    last_get_combined_node_flags_result: ast::NodeFlags,
    last_get_combined_modifier_flags_node: Option<ast::Node>,
    last_get_combined_modifier_flags_result: ast::ModifierFlags,
    declaration_modifier_flags_cache:
        [Option<DeclarationModifierFlagsCacheEntry>; DECLARATION_MODIFIER_FLAGS_CACHE_SIZE],
    declaration_modifier_flags_cache_next: usize,
    types: Arena<TypeRecord>,
    types_by_ts_id: HashMap<TypeId, TypeHandle>,
    signatures: Arena<SignatureRecord>,
    mappers: Arena<TypeMapperRecord>,
    mapper_chain_depths: Vec<u32>,
    mapper_perf_counters: TypeMapperPerfCounters,
    index_infos: Arena<IndexInfoRecord>,
    type_aliases: Arena<TypeAliasRecord>,
    type_predicates: Arena<TypePredicateRecord>,
    conditional_roots: Arena<ConditionalRootRecord>,
    transient_symbol_store: ast::TransientSymbolStore,
    inference_contexts: Arena<InferenceContextRecord>,
    semantic_handles: CheckerSemanticHandles,
    semantic_caches: CheckerSemanticCaches,
    semantic_initialized: bool,
    global_type_resolutions: HashMap<TypeResolver, TypeHandle>,
    global_symbol_resolutions: HashMap<SymbolResolver, Option<SymbolIdentity>>,
    global_types_resolutions: HashMap<TypesResolver, Vec<TypeHandle>>,
    pub(crate) get_global_es_symbol_type: TypeResolver,
    pub(crate) get_global_big_int_type: TypeResolver,
    pub(crate) get_global_import_meta_type: TypeResolver,
    pub(crate) get_global_import_attributes_type: TypeResolver,
    pub(crate) get_global_import_attributes_type_checked: TypeResolver,
    pub(crate) get_global_non_nullable_type_alias_or_nil: SymbolResolver,
    pub(crate) get_global_extract_symbol: SymbolResolver,
    pub(crate) get_global_disposable_type: TypeResolver,
    pub(crate) get_global_async_disposable_type: TypeResolver,
    pub(crate) get_global_awaited_symbol: SymbolResolver,
    pub(crate) get_global_awaited_symbol_or_nil: SymbolResolver,
    pub(crate) get_global_nan_symbol_or_nil: SymbolResolver,
    pub(crate) get_global_record_symbol: SymbolResolver,
    pub(crate) get_global_template_strings_array_type: TypeResolver,
    pub(crate) get_global_es_symbol_constructor_symbol_or_nil: SymbolResolver,
    pub(crate) get_global_es_symbol_constructor_type_symbol_or_nil: SymbolResolver,
    pub(crate) get_global_import_call_options_type: TypeResolver,
    pub(crate) get_global_import_call_options_type_checked: TypeResolver,
    pub(crate) get_global_promise_type: TypeResolver,
    pub(crate) get_global_promise_type_checked: TypeResolver,
    pub(crate) get_global_promise_like_type: TypeResolver,
    pub(crate) get_global_promise_constructor_symbol: SymbolResolver,
    pub(crate) get_global_promise_constructor_symbol_or_nil: SymbolResolver,
    pub(crate) get_global_omit_symbol: SymbolResolver,
    pub(crate) get_global_no_infer_symbol_or_nil: SymbolResolver,
    pub(crate) get_global_iterator_type: TypeResolver,
    pub(crate) get_global_iterable_type: TypeResolver,
    pub(crate) get_global_iterable_type_checked: TypeResolver,
    pub(crate) get_global_iterable_iterator_type: TypeResolver,
    pub(crate) get_global_iterable_iterator_type_checked: TypeResolver,
    pub(crate) get_global_iterator_object_type: TypeResolver,
    pub(crate) get_global_generator_type: TypeResolver,
    pub(crate) get_global_async_iterator_type: TypeResolver,
    pub(crate) get_global_async_iterable_type: TypeResolver,
    pub(crate) get_global_async_iterable_type_checked: TypeResolver,
    pub(crate) get_global_async_iterable_iterator_type: TypeResolver,
    pub(crate) get_global_async_iterable_iterator_type_checked: TypeResolver,
    pub(crate) get_global_async_iterator_object_type: TypeResolver,
    pub(crate) get_global_async_generator_type: TypeResolver,
    pub(crate) get_global_iterator_yield_result_type: TypeResolver,
    pub(crate) get_global_iterator_return_result_type: TypeResolver,
    pub(crate) get_global_typed_property_descriptor_type: TypeResolver,
    pub(crate) get_global_class_decorator_context_type: TypeResolver,
    pub(crate) get_global_class_method_decorator_context_type: TypeResolver,
    pub(crate) get_global_class_getter_decorator_context_type: TypeResolver,
    pub(crate) get_global_class_setter_decorator_context_type: TypeResolver,
    pub(crate) get_global_class_accessor_decorator_context_type: TypeResolver,
    pub(crate) get_global_class_accessor_decorator_target_type: TypeResolver,
    pub(crate) get_global_class_accessor_decorator_result_type: TypeResolver,
    pub(crate) get_global_class_field_decorator_context_type: TypeResolver,
    pub(crate) sync_iteration_types_resolver: Option<IterationTypesResolver>,
    pub(crate) async_iteration_types_resolver: Option<IterationTypesResolver>,
    pub(crate) variance_type_parameter: Option<TypeHandle>,
    pub(crate) free_flow_states: Vec<FlowState>,
    pub(crate) type_resolutions: Vec<TypeResolution>,
    pub(crate) flow_loop_stack: Vec<FlowLoopInfo>,
    pub(crate) shared_flows: Vec<SharedFlow>,
    pub(crate) inference_context_infos: Vec<InferenceContextInfo>,
    pub(crate) reverse_expanding_flags: ExpandingFlags,
    pub(crate) subtype_relation: RelationKind,
    pub(crate) strict_subtype_relation: RelationKind,
    pub(crate) assignable_relation: RelationKind,
    pub(crate) comparable_relation: RelationKind,
    pub(crate) identity_relation: RelationKind,
    pub(crate) compare_types_assignable: TypeComparer,
    pub(crate) active_mappers: Vec<TypeMapperHandle>,
    pub(crate) active_type_mappers_caches: Vec<HashMap<CacheHashKey, TypeHandle>>,
    pub(crate) deferred_diagnostics: Vec<DeferredDiagnostic>,
}

const DECLARATION_MODIFIER_FLAGS_CACHE_SIZE: usize = 8;

#[derive(Clone, Copy)]
struct DeclarationModifierFlagsCacheEntry {
    symbol: SymbolIdentity,
    is_write: bool,
    flags: ast::ModifierFlags,
}

impl CheckerState {
    pub fn new_for_slot_index(slot_index: usize) -> Self {
        let slot_number = slot_index
            .checked_add(1)
            .expect("checker slot index must not overflow");
        let slot = u32::try_from(slot_number).expect("checker slot index must fit in u32");
        Self::new(CheckerStateIdentity::new(
            CheckerSlotId::new(slot),
            CheckerGeneration::initial(),
        ))
    }

    pub fn next_generation(&self) -> Self {
        Self::new(CheckerStateIdentity::new(
            self.identity.slot(),
            self.identity.generation().next(),
        ))
    }

    fn new(identity: CheckerStateIdentity) -> Self {
        let bootstrap_type_resolver = TypeResolver::Bootstrap;
        let bootstrap_symbol_resolver = SymbolResolver::Bootstrap;
        let mut types = Arena::default();
        let mut mappers = Arena::default();
        let mut type_predicates = Arena::default();
        let mut signatures = Arena::default();
        let mut index_infos = Arena::default();

        let bootstrap_type = TypeHandle::new(types.alloc(TypeRecord {
            ts_id: 0,
            flags: TYPE_FLAGS_ANY,
            object_flags: OBJECT_FLAGS_NONE,
            symbol: None,
            alias: None,
            data: TypeRecordData::Intrinsic(IntrinsicTypeRecord {
                intrinsic_name: String::new(),
            }),
        }));
        let bootstrap_mapper = TypeMapperHandle::new(mappers.alloc(TypeMapperRecord {
            data: TypeMapperRecordData::Identity,
        }));
        let mapper_chain_depths = vec![1];
        let bootstrap_type_predicate =
            TypePredicateHandle::new(type_predicates.alloc(TypePredicateRecord {
                kind: TYPE_PREDICATE_KIND_IDENTIFIER,
                parameter_index: 0,
                parameter_name: String::new(),
                t: Some(bootstrap_type),
            }));
        let bootstrap_signature = SignatureHandle::new(signatures.alloc(SignatureRecord {
            flags: SIGNATURE_FLAGS_NONE,
            min_argument_count: 0,
            resolved_min_argument_count: 0,
            declaration: None,
            type_parameters: Vec::new(),
            parameters: Arc::from([]),
            this_parameter: None,
            resolved_return_type: Some(bootstrap_type),
            resolved_type_predicate: Some(bootstrap_type_predicate),
            target: None,
            mapper: None,
            isolated_signature_type: None,
            composite: None,
        }));
        let bootstrap_index_info = IndexInfoHandle::new(index_infos.alloc(IndexInfoRecord {
            key_type: Some(bootstrap_type),
            value_type: Some(bootstrap_type),
            is_readonly: false,
            declaration: None,
            index_symbol: None,
            components: Vec::new(),
        }));
        let bootstrap_handles = CheckerBootstrapHandles {
            bootstrap_type,
            bootstrap_mapper,
            bootstrap_type_predicate,
            bootstrap_signature,
            bootstrap_index_info,
        };
        let semantic_handles = CheckerSemanticHandles::from_bootstrap(&bootstrap_handles);
        let mut types_by_ts_id = HashMap::new();
        types_by_ts_id.insert(0, bootstrap_type);
        Self {
            identity,
            type_count: 0,
            symbol_count: 0,
            total_instantiation_count: 0,
            instantiation_count: 0,
            instantiation_depth: 0,
            next_type_mapper_identity: 1,
            reliability_flags: 0,
            is_inference_partially_blocked: false,
            inline_level: 0,
            was_canceled: false,
            save_deferred_diagnostics: false,
            can_collect_symbol_alias_accessibility_data: false,
            ctx: core::Context::default(),
            options: CheckerDerivedOptions::default(),
            factory: ast::NodeFactory::default(),
            diagnostics: ast::DiagnosticsCollection::default(),
            suggestion_diagnostics: ast::DiagnosticsCollection::default(),
            globals: SymbolIdentityTable::default(),
            symbols: SymbolRegistry::default(),
            undefined_symbol: None,
            arguments_symbol: None,
            require_symbol: None,
            unknown_symbol: None,
            global_this_symbol: None,
            pattern_ambient_module_augmentations: SymbolIdentityTable::default(),
            pattern_ambient_modules: Vec::new(),
            ambient_modules_once: Once::new(),
            ambient_modules: Vec::new(),
            node_links: NodeLinkStore::default(),
            symbol_node_links: NodeLinkStore::default(),
            enum_member_links: NodeLinkStore::default(),
            array_literal_links: NodeLinkStore::default(),
            jsx_links: NodeLinkStore::default(),
            declaration_links: NodeLinkStore::default(),
            declaration_file_links: SourceFileLinkStore::default(),
            jsx_namespace: String::new(),
            jsx_factory_entity: None,
            symbol_reference_links: SymbolLinkStore::default(),
            alias_symbol_links: SymbolLinkStore::default(),
            module_symbol_links: SymbolLinkStore::default(),
            late_bound_links: SymbolLinkStore::default(),
            export_type_links: SymbolLinkStore::default(),
            spread_links: SymbolLinkStore::default(),
            variance_links: SymbolLinkStore::default(),
            marked_assignment_symbol_links: SymbolLinkStore::default(),
            symbol_container_links: SymbolLinkStore::default(),
            array_variances: vec![VARIANCE_FLAGS_COVARIANT],
            undefined_properties: HashMap::new(),
            unresolved_symbols: HashMap::new(),
            merged_symbols: MergedSymbolStore::default(),
            primitive_type_alias_suggestions: HashMap::new(),
            this_expando_kinds: HashMap::new(),
            this_expando_locations: HashMap::new(),
            marker_types: HashSet::new(),
            resolving_union_or_intersection_properties: Vec::new(),
            awaited_type_stack: Vec::new(),
            cached_arguments_referenced: HashMap::new(),
            contextual_binding_patterns: Vec::new(),
            renamed_binding_elements_in_types: Vec::new(),
            skip_direct_inference_nodes: Set::default(),
            reported_unreachable_nodes: Set::default(),
            synthetic_node_symbols: HashMap::new(),
            synthetic_node_locals: HashMap::new(),
            packages_map: HashMap::new(),
            current_node: None,
            current_source_file: None,
            reg_exp_scanner: None,
            within_unreachable_code: false,
            flow_analysis_disabled: false,
            flow_invocation_count: 0,
            last_flow_node: None,
            last_flow_node_reachable: false,
            flow_node_reachable: ast::FlowRefSideTable::default(),
            flow_node_post_super: ast::FlowRefSideTable::default(),
            resolution_start: 0,
            in_variance_computation: false,
            apparent_argument_count: None,
            enum_relation: HashMap::new(),
            last_get_combined_node_flags_node: None,
            last_get_combined_node_flags_result: ast::NodeFlags::None,
            last_get_combined_modifier_flags_node: None,
            last_get_combined_modifier_flags_result: ast::ModifierFlags::None,
            declaration_modifier_flags_cache: [None; DECLARATION_MODIFIER_FLAGS_CACHE_SIZE],
            declaration_modifier_flags_cache_next: 0,
            types,
            types_by_ts_id,
            signatures,
            mappers,
            mapper_chain_depths,
            mapper_perf_counters: TypeMapperPerfCounters::default(),
            index_infos,
            type_aliases: Arena::default(),
            type_predicates,
            conditional_roots: Arena::default(),
            transient_symbol_store: ast::TransientSymbolStore::new(),
            inference_contexts: Arena::default(),
            semantic_handles,
            semantic_caches: CheckerSemanticCaches::default(),
            semantic_initialized: false,
            global_type_resolutions: HashMap::new(),
            global_symbol_resolutions: HashMap::new(),
            global_types_resolutions: HashMap::new(),
            get_global_es_symbol_type: bootstrap_type_resolver,
            get_global_big_int_type: bootstrap_type_resolver,
            get_global_import_meta_type: bootstrap_type_resolver,
            get_global_import_attributes_type: bootstrap_type_resolver,
            get_global_import_attributes_type_checked: bootstrap_type_resolver,
            get_global_non_nullable_type_alias_or_nil: bootstrap_symbol_resolver,
            get_global_extract_symbol: bootstrap_symbol_resolver,
            get_global_disposable_type: bootstrap_type_resolver,
            get_global_async_disposable_type: bootstrap_type_resolver,
            get_global_awaited_symbol: bootstrap_symbol_resolver,
            get_global_awaited_symbol_or_nil: bootstrap_symbol_resolver,
            get_global_nan_symbol_or_nil: bootstrap_symbol_resolver,
            get_global_record_symbol: bootstrap_symbol_resolver,
            get_global_template_strings_array_type: bootstrap_type_resolver,
            get_global_es_symbol_constructor_symbol_or_nil: bootstrap_symbol_resolver,
            get_global_es_symbol_constructor_type_symbol_or_nil: bootstrap_symbol_resolver,
            get_global_import_call_options_type: bootstrap_type_resolver,
            get_global_import_call_options_type_checked: bootstrap_type_resolver,
            get_global_promise_type: bootstrap_type_resolver,
            get_global_promise_type_checked: bootstrap_type_resolver,
            get_global_promise_like_type: bootstrap_type_resolver,
            get_global_promise_constructor_symbol: bootstrap_symbol_resolver,
            get_global_promise_constructor_symbol_or_nil: bootstrap_symbol_resolver,
            get_global_omit_symbol: bootstrap_symbol_resolver,
            get_global_no_infer_symbol_or_nil: bootstrap_symbol_resolver,
            get_global_iterator_type: bootstrap_type_resolver,
            get_global_iterable_type: bootstrap_type_resolver,
            get_global_iterable_type_checked: bootstrap_type_resolver,
            get_global_iterable_iterator_type: bootstrap_type_resolver,
            get_global_iterable_iterator_type_checked: bootstrap_type_resolver,
            get_global_iterator_object_type: bootstrap_type_resolver,
            get_global_generator_type: bootstrap_type_resolver,
            get_global_async_iterator_type: bootstrap_type_resolver,
            get_global_async_iterable_type: bootstrap_type_resolver,
            get_global_async_iterable_type_checked: bootstrap_type_resolver,
            get_global_async_iterable_iterator_type: bootstrap_type_resolver,
            get_global_async_iterable_iterator_type_checked: bootstrap_type_resolver,
            get_global_async_iterator_object_type: bootstrap_type_resolver,
            get_global_async_generator_type: bootstrap_type_resolver,
            get_global_iterator_yield_result_type: bootstrap_type_resolver,
            get_global_iterator_return_result_type: bootstrap_type_resolver,
            get_global_typed_property_descriptor_type: bootstrap_type_resolver,
            get_global_class_decorator_context_type: bootstrap_type_resolver,
            get_global_class_method_decorator_context_type: bootstrap_type_resolver,
            get_global_class_getter_decorator_context_type: bootstrap_type_resolver,
            get_global_class_setter_decorator_context_type: bootstrap_type_resolver,
            get_global_class_accessor_decorator_context_type: bootstrap_type_resolver,
            get_global_class_accessor_decorator_target_type: bootstrap_type_resolver,
            get_global_class_accessor_decorator_result_type: bootstrap_type_resolver,
            get_global_class_field_decorator_context_type: bootstrap_type_resolver,
            sync_iteration_types_resolver: None,
            async_iteration_types_resolver: None,
            variance_type_parameter: None,
            free_flow_states: Vec::new(),
            type_resolutions: Vec::new(),
            flow_loop_stack: Vec::new(),
            shared_flows: Vec::new(),
            inference_context_infos: Vec::new(),
            reverse_expanding_flags: Default::default(),
            subtype_relation: RelationKind::Subtype,
            strict_subtype_relation: RelationKind::StrictSubtype,
            assignable_relation: RelationKind::Assignable,
            comparable_relation: RelationKind::Comparable,
            identity_relation: RelationKind::Identity,
            compare_types_assignable: compare_types_assignable_worker_entry,
            active_mappers: Vec::new(),
            active_type_mappers_caches: Vec::new(),
            deferred_diagnostics: Vec::new(),
        }
    }

    pub const fn identity(&self) -> CheckerStateIdentity {
        self.identity
    }

    pub(crate) fn semantic_initialized(&self) -> bool {
        self.semantic_initialized
    }

    pub(crate) fn set_semantic_initialized(&mut self) {
        self.semantic_initialized = true;
    }

    pub(crate) fn global_type_resolution(&self, resolver: TypeResolver) -> Option<TypeHandle> {
        self.global_type_resolutions.get(&resolver).copied()
    }

    pub(crate) fn set_global_type_resolution(&mut self, resolver: TypeResolver, value: TypeHandle) {
        self.global_type_resolutions.insert(resolver, value);
    }

    pub(crate) fn global_symbol_resolution(
        &self,
        resolver: SymbolResolver,
    ) -> Option<Option<SymbolIdentity>> {
        self.global_symbol_resolutions.get(&resolver).copied()
    }

    pub(crate) fn set_global_symbol_resolution(
        &mut self,
        resolver: SymbolResolver,
        value: Option<SymbolIdentity>,
    ) {
        self.global_symbol_resolutions.insert(resolver, value);
    }

    pub(crate) fn collect_global_types_resolution(
        &self,
        resolver: TypesResolver,
    ) -> Option<Vec<TypeHandle>> {
        self.global_types_resolutions.get(&resolver).cloned()
    }

    pub(crate) fn set_global_types_resolution(
        &mut self,
        resolver: TypesResolver,
        value: Vec<TypeHandle>,
    ) {
        self.global_types_resolutions.insert(resolver, value);
    }

    pub fn next_type_id(&mut self) -> TypeId {
        self.type_count += 1;
        self.type_count
    }

    pub fn next_type_mapper_identity(&mut self) -> u64 {
        let identity = self.next_type_mapper_identity;
        self.next_type_mapper_identity += 1;
        identity
    }

    pub const fn ts_type_count(&self) -> TypeId {
        self.type_count
    }

    pub const fn symbol_count(&self) -> u32 {
        self.symbol_count
    }

    pub const fn total_instantiation_count(&self) -> u32 {
        self.total_instantiation_count
    }

    pub const fn instantiation_count(&self) -> u32 {
        self.instantiation_count
    }

    pub const fn instantiation_depth(&self) -> u32 {
        self.instantiation_depth
    }

    pub const fn mapper_perf_counters(&self) -> TypeMapperPerfCounters {
        self.mapper_perf_counters
    }

    pub fn reset_instantiation_count(&mut self) {
        self.instantiation_count = 0;
    }

    pub fn enter_instantiation(&mut self) {
        self.total_instantiation_count += 1;
        self.instantiation_count += 1;
        self.instantiation_depth += 1;
    }

    pub fn exit_instantiation(&mut self) {
        self.instantiation_depth -= 1;
    }

    pub const fn reliability_flags(&self) -> RelationComparisonResult {
        self.reliability_flags
    }

    pub fn set_reliability_flags(&mut self, value: RelationComparisonResult) {
        self.reliability_flags = value;
    }

    pub fn add_reliability_flags(&mut self, value: RelationComparisonResult) {
        self.reliability_flags |= value;
    }

    pub const fn is_inference_partially_blocked(&self) -> bool {
        self.is_inference_partially_blocked
    }

    pub fn set_inference_partially_blocked(&mut self, value: bool) {
        self.is_inference_partially_blocked = value;
    }

    pub const fn inline_level(&self) -> isize {
        self.inline_level
    }

    pub fn enter_inline(&mut self) {
        self.inline_level += 1;
    }

    pub fn exit_inline(&mut self) {
        self.inline_level -= 1;
    }

    pub const fn was_canceled(&self) -> bool {
        self.was_canceled
    }

    pub fn mark_canceled(&mut self) {
        self.was_canceled = true;
    }

    pub const fn save_deferred_diagnostics(&self) -> bool {
        self.save_deferred_diagnostics
    }

    pub fn set_save_deferred_diagnostics(&mut self, value: bool) {
        self.save_deferred_diagnostics = value;
    }

    pub const fn can_collect_symbol_alias_accessibility_data(&self) -> bool {
        self.can_collect_symbol_alias_accessibility_data
    }

    pub fn set_can_collect_symbol_alias_accessibility_data(&mut self, value: bool) {
        self.can_collect_symbol_alias_accessibility_data = value;
    }

    pub fn context(&self) -> &core::Context {
        &self.ctx
    }

    pub fn set_context(&mut self, ctx: core::Context) {
        self.ctx = ctx;
    }

    pub fn reset_context(&mut self) {
        self.ctx = core::Context::default();
    }

    pub const fn options(&self) -> CheckerDerivedOptions {
        self.options
    }

    pub fn set_options(&mut self, options: CheckerDerivedOptions) {
        self.options = options;
    }

    pub fn set_options_from_compiler_options(&mut self, options: &core::CompilerOptions) {
        self.options = CheckerDerivedOptions::from_compiler_options(options);
    }

    pub const fn language_version(&self) -> core::ScriptTarget {
        self.options.language_version
    }

    pub const fn module_kind(&self) -> core::ModuleKind {
        self.options.module_kind
    }

    pub const fn module_resolution_kind(&self) -> core::ModuleResolutionKind {
        self.options.module_resolution_kind
    }

    pub const fn legacy_decorators(&self) -> bool {
        self.options.legacy_decorators
    }

    pub const fn emit_standard_class_fields(&self) -> bool {
        self.options.emit_standard_class_fields
    }

    pub const fn strict_null_checks(&self) -> bool {
        self.options.strict_null_checks
    }

    pub const fn strict_function_types(&self) -> bool {
        self.options.strict_function_types
    }

    pub const fn strict_bind_call_apply(&self) -> bool {
        self.options.strict_bind_call_apply
    }

    pub const fn strict_property_initialization(&self) -> bool {
        self.options.strict_property_initialization
    }

    pub const fn strict_builtin_iterator_return(&self) -> bool {
        self.options.strict_builtin_iterator_return
    }

    pub const fn no_implicit_any(&self) -> bool {
        self.options.no_implicit_any
    }

    pub const fn no_implicit_this(&self) -> bool {
        self.options.no_implicit_this
    }

    pub const fn use_unknown_in_catch_variables(&self) -> bool {
        self.options.use_unknown_in_catch_variables
    }

    pub const fn exact_optional_property_types(&self) -> bool {
        self.options.exact_optional_property_types
    }

    pub fn factory(&self) -> &ast::NodeFactory {
        &self.factory
    }

    pub fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.factory
    }

    pub(crate) fn synthetic_node_symbol_identity(&self, node: ast::Node) -> Option<SymbolIdentity> {
        self.synthetic_node_symbols.get(&node).copied()
    }

    pub(crate) fn set_synthetic_node_symbol_identity(
        &mut self,
        node: ast::Node,
        symbol: SymbolIdentity,
    ) {
        self.synthetic_node_symbols.insert(node, symbol);
    }

    pub(crate) fn collect_synthetic_node_locals(
        &self,
        node: ast::Node,
    ) -> Option<SymbolIdentityTable> {
        self.synthetic_node_locals.get(&node).cloned()
    }

    pub(crate) fn has_synthetic_node_locals(&self, node: ast::Node) -> bool {
        self.synthetic_node_locals.contains_key(&node)
    }

    pub(crate) fn set_synthetic_node_locals(
        &mut self,
        node: ast::Node,
        locals: SymbolIdentityTable,
    ) {
        self.synthetic_node_locals.insert(node, locals);
    }

    pub const fn diagnostics(&self) -> &ast::DiagnosticsCollection {
        &self.diagnostics
    }

    pub const fn suggestion_diagnostics(&self) -> &ast::DiagnosticsCollection {
        &self.suggestion_diagnostics
    }

    pub(crate) fn begin_global_symbol_table_initialization(&mut self, capacity: usize) {
        self.globals = SymbolIdentityTable::with_capacity_and_hasher(capacity, Default::default());
        self.global_this_symbol = None;
        self.pattern_ambient_modules.clear();
    }

    pub(crate) fn global_symbol_handles(&self) -> impl Iterator<Item = ast::SymbolHandle> + '_ {
        self.globals.values().map(|&symbol| symbol.symbol_handle())
    }

    pub(crate) fn global_symbol_identity_len(&self) -> usize {
        self.globals.len()
    }

    pub(crate) fn global_symbol_identity_at(&self, index: usize) -> Option<SymbolIdentity> {
        self.globals.get_index(index).map(|(_, &symbol)| symbol)
    }

    pub(crate) fn global_symbol_identity_entry_at(
        &self,
        index: usize,
    ) -> Option<(ast::SymbolName, SymbolIdentity)> {
        self.globals
            .get_index(index)
            .map(|(name, &symbol)| (name.clone(), symbol))
    }

    pub(crate) fn with_global_symbols<R>(
        &self,
        f: impl FnOnce(GlobalSymbolTableView<'_>) -> R,
    ) -> R {
        f(GlobalSymbolTableView {
            globals: &self.globals,
        })
    }

    pub(crate) fn global_symbol_identity(&self, name: &str) -> Option<SymbolIdentity> {
        self.globals.get(name).copied()
    }

    pub(crate) fn has_global_symbol_identity(&self, name: &str) -> bool {
        self.globals.contains_key(name)
    }

    pub(crate) fn insert_global_symbol_handle(
        &mut self,
        name: impl Into<ast::SymbolName>,
        symbol: ast::SymbolHandle,
    ) {
        let symbol = self.intern_symbol_handle(symbol);
        self.insert_global_symbol_identity(name, symbol);
    }

    pub(crate) fn insert_global_symbol_identity(
        &mut self,
        name: impl Into<ast::SymbolName>,
        symbol: SymbolIdentity,
    ) {
        self.globals.insert(name.into(), symbol);
    }

    pub(crate) fn set_undefined_symbol_identity(&mut self, symbol: SymbolIdentity) {
        self.undefined_symbol = Some(symbol);
    }

    pub(crate) fn undefined_symbol_identity(&self) -> SymbolIdentity {
        self.undefined_symbol
            .expect("undefined symbol must be initialized")
    }

    pub(crate) fn set_arguments_symbol_identity(&mut self, symbol: SymbolIdentity) {
        self.arguments_symbol = Some(symbol);
    }

    pub(crate) fn arguments_symbol_identity(&self) -> SymbolIdentity {
        self.arguments_symbol
            .expect("arguments symbol must be initialized")
    }

    pub(crate) fn set_require_symbol_identity(&mut self, symbol: SymbolIdentity) {
        self.require_symbol = Some(symbol);
    }

    pub(crate) fn require_symbol_identity(&self) -> SymbolIdentity {
        self.require_symbol
            .expect("require symbol must be initialized")
    }

    pub(crate) fn unknown_symbol_identity(&self) -> SymbolIdentity {
        self.unknown_symbol
            .expect("unknown symbol must be initialized")
    }

    pub(crate) fn set_unknown_symbol_identity(&mut self, symbol: SymbolIdentity) {
        self.unknown_symbol = Some(symbol);
    }

    pub(crate) fn set_global_this_symbol_identity(&mut self, symbol: SymbolIdentity) {
        self.global_this_symbol = Some(symbol);
    }

    pub(crate) fn global_this_symbol_identity(&self) -> SymbolIdentity {
        self.global_this_symbol
            .expect("global this symbol must be initialized")
    }

    pub(crate) fn global_this_symbol_identity_if_present(&self) -> Option<SymbolIdentity> {
        self.global_this_symbol
    }

    pub(crate) fn insert_pattern_ambient_module_augmentation_handle(
        &mut self,
        name: impl Into<ast::SymbolName>,
        symbol: ast::SymbolHandle,
    ) {
        let symbol = self.intern_symbol_handle(symbol);
        self.insert_pattern_ambient_module_augmentation_identity(name, symbol);
    }

    pub(crate) fn insert_pattern_ambient_module_augmentation_identity(
        &mut self,
        name: impl Into<ast::SymbolName>,
        symbol: SymbolIdentity,
    ) {
        self.pattern_ambient_module_augmentations
            .insert(name.into(), symbol);
    }

    pub(crate) fn pattern_ambient_module_augmentation_identity(
        &self,
        name: &str,
    ) -> Option<SymbolIdentity> {
        self.pattern_ambient_module_augmentations.get(name).copied()
    }

    pub(crate) fn record_pattern_ambient_module(&mut self, module: &ast::PatternAmbientModule) {
        let symbol = module.symbol.map(SymbolIdentity::from_symbol_handle);
        self.pattern_ambient_modules
            .push(PatternAmbientModuleRecord {
                pattern: module.pattern.clone(),
                symbol,
            });
    }

    pub(crate) fn pattern_ambient_modules(&self) -> &[PatternAmbientModuleRecord] {
        &self.pattern_ambient_modules
    }

    pub(crate) fn pattern_ambient_module_symbol_identity(
        &self,
        module: &PatternAmbientModuleRecord,
    ) -> Option<SymbolIdentity> {
        module.symbol
    }

    pub(crate) fn collect_ambient_module_identities(&mut self) -> Vec<SymbolIdentity> {
        let ambient_modules = &mut self.ambient_modules;
        let globals = &self.globals;
        self.ambient_modules_once.call_once(|| {
            ambient_modules.extend(globals.iter().filter_map(|(name, symbol)| {
                (name.starts_with('"') && name.ends_with('"')).then_some(*symbol)
            }));
        });
        self.ambient_modules.clone()
    }

    #[cfg(test)]
    pub(crate) fn node_links(&self) -> &NodeLinkStore<NodeLinks> {
        &self.node_links
    }

    #[cfg(test)]
    pub(crate) fn symbol_node_links(&self) -> &NodeLinkStore<SymbolNodeLinks> {
        &self.symbol_node_links
    }

    #[cfg(test)]
    pub(crate) fn enum_member_links(&self) -> &NodeLinkStore<EnumMemberLinks> {
        &self.enum_member_links
    }

    #[cfg(test)]
    pub(crate) fn array_literal_links(&self) -> &NodeLinkStore<ArrayLiteralLinks> {
        &self.array_literal_links
    }

    #[cfg(test)]
    pub(crate) fn jsx_links(&self) -> &NodeLinkStore<JSXLinks> {
        &self.jsx_links
    }

    #[cfg(test)]
    pub(crate) fn declaration_links(&self) -> &NodeLinkStore<DeclarationLinks> {
        &self.declaration_links
    }

    #[cfg(test)]
    pub(crate) fn declaration_file_links(&self) -> &SourceFileLinkStore<DeclarationFileLinks> {
        &self.declaration_file_links
    }

    pub fn jsx_namespace(&self) -> &str {
        &self.jsx_namespace
    }

    pub fn set_jsx_namespace(&mut self, namespace: String) {
        self.jsx_namespace = namespace;
    }

    pub const fn jsx_factory_entity(&self) -> Option<ast::Node> {
        self.jsx_factory_entity
    }

    pub fn set_jsx_factory_entity(&mut self, entity: Option<ast::Node>) {
        self.jsx_factory_entity = entity;
    }

    #[cfg(test)]
    pub(crate) fn symbol_reference_links(&self) -> &SymbolLinkStore<SymbolReferenceLinks> {
        &self.symbol_reference_links
    }

    #[cfg(test)]
    pub(crate) fn alias_symbol_links(&self) -> &SymbolLinkStore<AliasSymbolLinks> {
        &self.alias_symbol_links
    }

    #[cfg(test)]
    pub(crate) fn module_symbol_links(&self) -> &SymbolLinkStore<ModuleSymbolLinks> {
        &self.module_symbol_links
    }

    #[cfg(test)]
    pub(crate) fn late_bound_links(&self) -> &SymbolLinkStore<LateBoundLinks> {
        &self.late_bound_links
    }

    #[cfg(test)]
    pub(crate) fn export_type_links(&self) -> &SymbolLinkStore<ExportTypeLinks> {
        &self.export_type_links
    }

    #[cfg(test)]
    pub(crate) fn signature_links(&self) -> &NodeLinkStore<SignatureLinks> {
        &self.semantic_caches.signature_links
    }

    #[cfg(test)]
    pub(crate) fn type_node_links(&self) -> &NodeLinkStore<TypeNodeLinks> {
        &self.semantic_caches.type_node_links
    }

    #[cfg(test)]
    pub(crate) fn assertion_links(&self) -> &NodeLinkStore<AssertionLinks> {
        &self.semantic_caches.assertion_links
    }

    #[cfg(test)]
    pub(crate) fn switch_statement_links(&self) -> &NodeLinkStore<SwitchStatementLinks> {
        &self.semantic_caches.switch_statement_links
    }

    #[cfg(test)]
    pub(crate) fn jsx_element_links(&self) -> &NodeLinkStore<JsxElementLinks> {
        &self.semantic_caches.jsx_element_links
    }

    #[cfg(test)]
    pub(crate) fn value_symbol_links(&self) -> &SymbolLinkStore<ValueSymbolLinks> {
        &self.semantic_caches.value_symbol_links
    }

    pub(crate) fn allocate_fresh_transient_value_symbol_link_handle(
        &self,
        symbol: ast::SymbolHandle,
    ) -> core::LinkHandle<ValueSymbolLinks> {
        self.semantic_caches
            .value_symbol_links
            .allocate_fresh_transient_symbol_handle(SymbolIdentity::from_symbol_handle(symbol))
    }

    #[cfg(test)]
    pub(crate) fn mapped_symbol_links(&self) -> &SymbolLinkStore<MappedSymbolLinks> {
        &self.semantic_caches.mapped_symbol_links
    }

    #[cfg(test)]
    pub(crate) fn members_and_exports_links(&self) -> &SymbolLinkStore<MembersAndExportsLinks> {
        &self.semantic_caches.members_and_exports_links
    }

    #[cfg(test)]
    pub(crate) fn deferred_symbol_links(&self) -> &SymbolLinkStore<DeferredSymbolLinks> {
        &self.semantic_caches.deferred_symbol_links
    }

    #[cfg(test)]
    pub(crate) fn reverse_mapped_symbol_links(&self) -> &SymbolLinkStore<ReverseMappedSymbolLinks> {
        &self.semantic_caches.reverse_mapped_symbol_links
    }

    #[cfg(test)]
    pub(crate) fn type_alias_links(&self) -> &SymbolLinkStore<TypeAliasLinks> {
        &self.semantic_caches.type_alias_links
    }

    #[cfg(test)]
    pub(crate) fn declared_type_links(&self) -> &SymbolLinkStore<DeclaredTypeLinks> {
        &self.semantic_caches.declared_type_links
    }

    #[cfg(test)]
    pub(crate) fn source_file_links(&self) -> &SourceFileLinkStore<SourceFileLinks> {
        &self.semantic_caches.source_file_links
    }

    #[cfg(test)]
    pub(crate) fn spread_links(&self) -> &SymbolLinkStore<SpreadLinks> {
        &self.spread_links
    }

    #[cfg(test)]
    pub(crate) fn variance_links(&self) -> &SymbolLinkStore<VarianceLinks> {
        &self.variance_links
    }

    #[cfg(test)]
    pub(crate) fn marked_assignment_symbol_links(
        &self,
    ) -> &SymbolLinkStore<MarkedAssignmentSymbolLinks> {
        &self.marked_assignment_symbol_links
    }

    #[cfg(test)]
    pub(crate) fn symbol_container_links(&self) -> &SymbolLinkStore<ContainingSymbolLinks> {
        &self.symbol_container_links
    }

    pub(crate) fn has_enum_member_link(&self, node: ast::Node) -> bool {
        self.enum_member_links.has(node)
    }

    pub(crate) fn has_export_type_link<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.export_type_links.has(symbol)
    }

    pub(crate) fn has_spread_link<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.spread_links.has(symbol)
    }

    pub(crate) fn has_value_symbol_link<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.semantic_caches.value_symbol_links.has(symbol)
    }

    pub(crate) fn has_mapped_symbol_link<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.semantic_caches.mapped_symbol_links.has(symbol)
    }

    pub fn array_variances(&self) -> &[VarianceFlags] {
        &self.array_variances
    }

    pub(crate) fn undefined_property_identity(&self, name: &str) -> Option<SymbolIdentity> {
        self.undefined_properties.get(name).copied()
    }

    pub(crate) fn insert_undefined_property_identity(
        &mut self,
        name: String,
        symbol: SymbolIdentity,
    ) {
        self.undefined_properties.insert(name, symbol);
    }

    pub(crate) fn unresolved_symbol_identity(&self, path: &str) -> Option<SymbolIdentity> {
        self.unresolved_symbols.get(path).copied()
    }

    pub(crate) fn insert_unresolved_symbol_identity(
        &mut self,
        path: String,
        symbol: SymbolIdentity,
    ) {
        self.unresolved_symbols.insert(path, symbol);
    }

    pub(crate) fn merged_symbol_identity<Q>(&self, key: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.merged_symbols.get(key)
    }

    pub(crate) fn record_merged_symbol_identity<Q>(&self, key: Q, symbol: SymbolIdentity)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        self.merged_symbols.insert(key, symbol);
    }

    pub(crate) fn primitive_type_alias_suggestion(
        &mut self,
        builtin_name: &str,
    ) -> Option<ast::SymbolHandle> {
        let (builtin_name, primitive_name) = primitive_type_alias_suggestion_name(builtin_name)?;
        if let Some(&identity) = self.primitive_type_alias_suggestions.get(builtin_name) {
            return Some(identity.symbol_handle());
        }

        let handle =
            self.new_transient_symbol(ast::SYMBOL_FLAGS_TYPE_ALIAS, primitive_name.to_string());
        let symbol = self.transient_symbol_handle(handle);
        let identity = self.transient_symbol_identity(handle);
        self.primitive_type_alias_suggestions
            .insert(builtin_name, identity);
        Some(symbol)
    }

    pub fn this_expando(
        &self,
        symbol: SymbolIdentity,
    ) -> Option<(ThisAssignmentDeclarationKind, Option<ast::Node>)> {
        let key = symbol;
        self.this_expando_kinds.get(&key).copied().map(|kind| {
            let location = self
                .this_expando_locations
                .get(&key)
                .copied()
                .unwrap_or_else(|| {
                    panic!("location should be cached whenever this expando symbol is cached")
                });
            (kind, location)
        })
    }

    pub fn record_this_expando(
        &mut self,
        symbol: SymbolIdentity,
        kind: ThisAssignmentDeclarationKind,
        location: Option<ast::Node>,
    ) {
        let key = symbol;
        self.this_expando_kinds.insert(key, kind);
        self.this_expando_locations.insert(key, location);
    }

    pub fn record_marker_type(&mut self, handle: TypeHandle) {
        self.marker_types.insert(handle);
    }

    pub fn is_marker_type(&self, handle: TypeHandle) -> bool {
        self.marker_types.contains(&handle)
    }

    pub fn is_resolving_union_or_intersection_property(
        &self,
        type_id: TypeId,
        name: &str,
        skip_object_function_property_augment: bool,
    ) -> bool {
        self.resolving_union_or_intersection_properties.iter().any(
            |(existing_type_id, existing_name, existing_skip)| {
                *existing_type_id == type_id
                    && existing_name == name
                    && *existing_skip == skip_object_function_property_augment
            },
        )
    }

    pub fn enter_resolving_union_or_intersection_property(
        &mut self,
        type_id: TypeId,
        name: String,
        skip_object_function_property_augment: bool,
    ) {
        self.resolving_union_or_intersection_properties.push((
            type_id,
            name,
            skip_object_function_property_augment,
        ));
    }

    pub fn exit_resolving_union_or_intersection_property(&mut self) {
        let _ = self.resolving_union_or_intersection_properties.pop();
    }

    pub fn is_awaiting_type(&self, type_id: TypeId) -> bool {
        self.awaited_type_stack.contains(&type_id)
    }

    pub fn enter_awaited_type(&mut self, type_id: TypeId) {
        self.awaited_type_stack.push(type_id);
    }

    pub fn exit_awaited_type(&mut self) {
        let _ = self.awaited_type_stack.pop();
    }

    pub fn cached_arguments_reference(&self, node: ast::Node) -> Option<bool> {
        self.cached_arguments_referenced.get(&node).copied()
    }

    pub fn record_cached_arguments_reference(&mut self, node: ast::Node, contains: bool) {
        self.cached_arguments_referenced.insert(node, contains);
    }

    pub fn is_contextual_binding_pattern(&self, node: ast::Node) -> bool {
        self.contextual_binding_patterns.contains(&node)
    }

    pub fn enter_contextual_binding_pattern(&mut self, node: ast::Node) {
        self.contextual_binding_patterns.push(node);
    }

    pub fn exit_contextual_binding_pattern(&mut self) {
        let _ = self.contextual_binding_patterns.pop();
    }

    pub fn clear_renamed_binding_elements_in_types(&mut self) {
        self.renamed_binding_elements_in_types.clear();
    }

    pub fn record_renamed_binding_element_in_type(&mut self, node: ast::Node) {
        self.renamed_binding_elements_in_types.push(node);
    }

    pub fn renamed_binding_elements_in_types(&self) -> Vec<ast::Node> {
        self.renamed_binding_elements_in_types.clone()
    }

    pub fn is_skip_direct_inference_node(&self, node: ast::Node) -> bool {
        self.skip_direct_inference_nodes.has(&node)
    }

    pub fn record_skip_direct_inference_node(&mut self, node: ast::Node) {
        self.skip_direct_inference_nodes.add(node);
    }

    pub fn clear_skip_direct_inference_nodes(&mut self) {
        self.skip_direct_inference_nodes.clear();
    }

    pub fn is_reported_unreachable_node(&self, node: ast::Node) -> bool {
        self.reported_unreachable_nodes.has(&node)
    }

    pub fn record_reported_unreachable_node(&mut self, node: ast::Node) {
        self.reported_unreachable_nodes.add(node);
    }

    pub fn clear_reported_unreachable_nodes(&mut self) {
        self.reported_unreachable_nodes.clear();
    }

    pub(crate) fn mark_nonexistent_property_reported(
        &mut self,
        prop_node: ast::Node,
        containing_type: TypeHandle,
        is_unchecked_js: bool,
    ) -> bool {
        let key = NonExistentPropertyKey {
            prop_node,
            containing_type,
            is_unchecked_js,
        };
        if self.semantic_caches.non_existent_properties.has(&key) {
            return false;
        }
        self.semantic_caches.non_existent_properties.add(key);
        true
    }

    #[cfg(test)]
    pub(crate) fn packages_map(&self) -> &HashMap<String, bool> {
        &self.packages_map
    }

    pub fn packages_map_is_empty(&self) -> bool {
        self.packages_map.is_empty()
    }

    pub fn package_map_entry(&self, package_name: &str) -> Option<bool> {
        self.packages_map.get(package_name).copied()
    }

    pub fn package_map_contains(&self, package_name: &str) -> bool {
        self.packages_map.contains_key(package_name)
    }

    pub fn record_package_map_entry(&mut self, package_name: String, bundles_types: bool) {
        self.packages_map.insert(package_name, bundles_types);
    }

    pub const fn current_node(&self) -> Option<ast::Node> {
        self.current_node
    }

    pub fn set_current_node(&mut self, node: Option<ast::Node>) {
        self.current_node = node;
    }

    pub const fn current_source_file(&self) -> Option<SourceFileIdentity> {
        self.current_source_file
    }

    pub fn set_current_source_file(&mut self, file: Option<&ast::SourceFile>) {
        self.current_source_file = file.map(SourceFileIdentity::from_source_file);
    }

    pub fn reg_exp_scanner_mut(&mut self) -> &mut scanner::Scanner {
        self.reg_exp_scanner
            .get_or_insert_with(scanner::new_scanner)
    }

    pub const fn within_unreachable_code(&self) -> bool {
        self.within_unreachable_code
    }

    pub fn set_within_unreachable_code(&mut self, value: bool) {
        self.within_unreachable_code = value;
    }

    pub const fn flow_analysis_disabled(&self) -> bool {
        self.flow_analysis_disabled
    }

    pub fn set_flow_analysis_disabled(&mut self, value: bool) {
        self.flow_analysis_disabled = value;
    }

    pub const fn flow_invocation_count(&self) -> isize {
        self.flow_invocation_count
    }

    pub fn enter_flow_invocation(&mut self) {
        self.flow_invocation_count += 1;
    }

    pub fn record_last_flow_node(&mut self, flow: ast::FlowRef, reachable: bool) {
        self.last_flow_node = Some(flow);
        self.last_flow_node_reachable = reachable;
    }

    pub fn clear_last_flow_node(&mut self) {
        self.last_flow_node = None;
    }

    pub fn last_flow_node_reachable(&self, flow: ast::FlowRef) -> Option<bool> {
        if self.last_flow_node == Some(flow) {
            return Some(self.last_flow_node_reachable);
        }
        None
    }

    pub fn flow_node_reachable(&self, flow: ast::FlowRef) -> Option<bool> {
        self.flow_node_reachable.get(flow).copied()
    }

    pub fn record_flow_node_reachable(&mut self, flow: ast::FlowRef, reachable: bool) {
        self.flow_node_reachable.insert(flow, reachable);
    }

    pub fn flow_node_post_super(&self, flow: ast::FlowRef) -> Option<bool> {
        self.flow_node_post_super.get(flow).copied()
    }

    pub fn record_flow_node_post_super(&mut self, flow: ast::FlowRef, post_super: bool) {
        self.flow_node_post_super.insert(flow, post_super);
    }

    pub const fn resolution_start(&self) -> isize {
        self.resolution_start
    }

    pub fn set_resolution_start(&mut self, value: isize) {
        self.resolution_start = value;
    }

    pub const fn in_variance_computation(&self) -> bool {
        self.in_variance_computation
    }

    pub fn set_in_variance_computation(&mut self, value: bool) {
        self.in_variance_computation = value;
    }

    pub const fn apparent_argument_count(&self) -> Option<isize> {
        self.apparent_argument_count
    }

    pub fn set_apparent_argument_count(&mut self, value: isize) {
        self.apparent_argument_count = Some(value);
    }

    pub fn clear_apparent_argument_count(&mut self) {
        self.apparent_argument_count = None;
    }

    pub fn enum_relation_result(&self, key: &EnumRelationKey) -> Option<RelationComparisonResult> {
        self.enum_relation.get(key).copied()
    }

    pub fn set_enum_relation_result(
        &mut self,
        key: EnumRelationKey,
        result: RelationComparisonResult,
    ) {
        self.enum_relation.insert(key, result);
    }

    #[cfg(test)]
    pub(crate) fn enum_relation(&self) -> &HashMap<EnumRelationKey, RelationComparisonResult> {
        &self.enum_relation
    }

    #[cfg(test)]
    pub(crate) fn enum_relation_mut(
        &mut self,
    ) -> &mut HashMap<EnumRelationKey, RelationComparisonResult> {
        &mut self.enum_relation
    }

    pub fn combined_node_flags_cache(&self, node: ast::Node) -> Option<ast::NodeFlags> {
        if self.last_get_combined_node_flags_node == Some(node) {
            return Some(self.last_get_combined_node_flags_result);
        }
        None
    }

    pub fn record_combined_node_flags_cache(&mut self, node: ast::Node, flags: ast::NodeFlags) {
        self.last_get_combined_node_flags_node = Some(node);
        self.last_get_combined_node_flags_result = flags;
    }

    pub fn combined_modifier_flags_cache(&self, node: ast::Node) -> Option<ast::ModifierFlags> {
        if self.last_get_combined_modifier_flags_node == Some(node) {
            return Some(self.last_get_combined_modifier_flags_result);
        }
        None
    }

    pub fn record_combined_modifier_flags_cache(
        &mut self,
        node: ast::Node,
        flags: ast::ModifierFlags,
    ) {
        self.last_get_combined_modifier_flags_node = Some(node);
        self.last_get_combined_modifier_flags_result = flags;
    }

    pub fn declaration_modifier_flags_cache(
        &self,
        symbol: SymbolIdentity,
        is_write: bool,
    ) -> Option<ast::ModifierFlags> {
        self.declaration_modifier_flags_cache
            .iter()
            .flatten()
            .find(|entry| entry.symbol == symbol && entry.is_write == is_write)
            .map(|entry| entry.flags)
    }

    pub fn record_declaration_modifier_flags_cache(
        &mut self,
        symbol: SymbolIdentity,
        is_write: bool,
        flags: ast::ModifierFlags,
    ) {
        if let Some(entry) = self
            .declaration_modifier_flags_cache
            .iter_mut()
            .flatten()
            .find(|entry| entry.symbol == symbol && entry.is_write == is_write)
        {
            entry.flags = flags;
            return;
        }
        self.declaration_modifier_flags_cache[self.declaration_modifier_flags_cache_next] =
            Some(DeclarationModifierFlagsCacheEntry {
                symbol,
                is_write,
                flags,
            });
        self.declaration_modifier_flags_cache_next = (self.declaration_modifier_flags_cache_next
            + 1)
            % DECLARATION_MODIFIER_FLAGS_CACHE_SIZE;
    }

    fn clear_declaration_modifier_flags_cache(&mut self) {
        self.declaration_modifier_flags_cache = [None; DECLARATION_MODIFIER_FLAGS_CACHE_SIZE];
        self.declaration_modifier_flags_cache_next = 0;
    }

    pub fn alloc_type(&mut self, record: TypeRecord) -> TypeHandle {
        let ts_id = record.ts_id;
        let handle = TypeHandle::new(self.types.alloc(record));
        assert!(
            self.types_by_ts_id.insert(ts_id, handle).is_none(),
            "duplicate checker TypeId {ts_id}"
        );
        handle
    }

    pub fn type_record(&self, handle: TypeHandle) -> &TypeRecord {
        &self.types[handle.idx()]
    }

    pub fn type_handle_by_id(&self, ts_id: TypeId) -> Option<TypeHandle> {
        self.types_by_ts_id.get(&ts_id).copied()
    }

    pub(crate) fn type_record_mut(&mut self, handle: TypeHandle) -> &mut TypeRecord {
        &mut self.types[handle.idx()]
    }

    pub(crate) fn intern_symbol_handle(&mut self, symbol: ast::SymbolHandle) -> SymbolIdentity {
        self.symbols.intern_handle(symbol)
    }

    pub(crate) fn symbol_handle(&self, identity: SymbolIdentity) -> ast::SymbolHandle {
        identity.symbol_handle()
    }

    pub(crate) fn symbol_origin(&self, identity: SymbolIdentity) -> Option<SymbolOrigin> {
        self.symbols.origin(identity)
    }

    pub(crate) fn type_symbol_identity(&self, handle: TypeHandle) -> Option<SymbolIdentity> {
        self.type_record(handle).symbol
    }

    pub(crate) fn set_type_symbol_identity(
        &mut self,
        handle: TypeHandle,
        symbol: Option<SymbolIdentity>,
    ) {
        self.type_record_mut(handle).symbol = symbol;
    }

    pub fn alloc_signature(&mut self, record: SignatureRecord) -> SignatureHandle {
        SignatureHandle::new(self.signatures.alloc(record))
    }

    pub fn signature_record(&self, handle: SignatureHandle) -> &SignatureRecord {
        &self.signatures[handle.idx()]
    }

    pub(crate) fn signature_record_mut(&mut self, handle: SignatureHandle) -> &mut SignatureRecord {
        &mut self.signatures[handle.idx()]
    }

    pub fn alloc_mapper(&mut self, record: TypeMapperRecord) -> TypeMapperHandle {
        let chain_depth = self.mapper_record_data_chain_depth(&record.data);
        self.mapper_perf_counters
            .record_allocation(&record.data, chain_depth);
        debug_assert_eq!(self.mapper_chain_depths.len(), self.mappers.len());
        let handle = TypeMapperHandle::new(self.mappers.alloc(record));
        self.mapper_chain_depths.push(chain_depth);
        debug_assert_eq!(
            self.mapper_chain_depths.len(),
            self.mappers.len(),
            "mapper chain-depth side table must stay aligned with mapper arena"
        );
        handle
    }

    pub fn mapper_record(&self, handle: TypeMapperHandle) -> &TypeMapperRecord {
        &self.mappers[handle.idx()]
    }

    fn mapper_record_data_chain_depth(&self, data: &TypeMapperRecordData) -> u32 {
        match data {
            TypeMapperRecordData::Merged(mapper) => {
                1 + self
                    .mapper_chain_depth(mapper.left)
                    .max(self.mapper_chain_depth(mapper.right))
            }
            TypeMapperRecordData::Composite(mapper) => {
                1 + self
                    .mapper_chain_depth(mapper.left)
                    .max(self.mapper_chain_depth(mapper.right))
            }
            _ => 1,
        }
    }

    fn mapper_chain_depth(&self, handle: TypeMapperHandle) -> u32 {
        self.mapper_chain_depths[handle.idx().into_raw().into_usize()]
    }

    pub fn alloc_index_info(&mut self, record: IndexInfoRecord) -> IndexInfoHandle {
        IndexInfoHandle::new(self.index_infos.alloc(record))
    }

    pub fn index_info_record(&self, handle: IndexInfoHandle) -> &IndexInfoRecord {
        &self.index_infos[handle.idx()]
    }

    pub(crate) fn index_info_record_mut(
        &mut self,
        handle: IndexInfoHandle,
    ) -> &mut IndexInfoRecord {
        &mut self.index_infos[handle.idx()]
    }

    pub fn alloc_type_alias(&mut self, record: TypeAliasRecord) -> TypeAliasHandle {
        TypeAliasHandle::new(self.type_aliases.alloc(record))
    }

    pub fn type_alias_record(&self, handle: TypeAliasHandle) -> &TypeAliasRecord {
        &self.type_aliases[handle.idx()]
    }

    pub fn alloc_type_predicate(&mut self, record: TypePredicateRecord) -> TypePredicateHandle {
        TypePredicateHandle::new(self.type_predicates.alloc(record))
    }

    pub fn type_predicate_record(&self, handle: TypePredicateHandle) -> &TypePredicateRecord {
        &self.type_predicates[handle.idx()]
    }

    pub fn alloc_conditional_root(
        &mut self,
        record: ConditionalRootRecord,
    ) -> ConditionalRootHandle {
        ConditionalRootHandle::new(self.conditional_roots.alloc(record))
    }

    pub fn conditional_root_record(&self, handle: ConditionalRootHandle) -> &ConditionalRootRecord {
        &self.conditional_roots[handle.idx()]
    }

    pub(crate) fn conditional_root_record_mut(
        &mut self,
        handle: ConditionalRootHandle,
    ) -> &mut ConditionalRootRecord {
        &mut self.conditional_roots[handle.idx()]
    }

    fn alloc_transient_symbol(&mut self, symbol: ast::SymbolHandle) -> TransientSymbolHandle {
        debug_assert!(self.transient_symbol_store.owns(symbol));
        self.symbols.intern_transient(symbol);
        TransientSymbolHandle::new(symbol)
    }

    pub(crate) fn new_transient_symbol(
        &mut self,
        flags: ast::SymbolFlags,
        name: impl Into<ast::SymbolName>,
    ) -> TransientSymbolHandle {
        self.symbol_count += 1;
        let symbol = self
            .transient_symbol_store
            .create_transient_symbol(flags | ast::SYMBOL_FLAGS_TRANSIENT, name);
        self.alloc_transient_symbol(symbol)
    }

    pub(crate) fn new_transient_symbol_ex(
        &mut self,
        flags: ast::SymbolFlags,
        name: impl Into<ast::SymbolName>,
        check_flags: ast::CheckFlags,
    ) -> TransientSymbolHandle {
        self.symbol_count += 1;
        let symbol = self
            .transient_symbol_store
            .create_transient_symbol_with_check_flags(
                flags | ast::SYMBOL_FLAGS_TRANSIENT,
                name,
                check_flags,
            );
        self.alloc_transient_symbol(symbol)
    }

    pub(crate) fn new_transient_symbol_from_instantiation(
        &mut self,
        flags: ast::SymbolFlags,
        name: impl Into<ast::SymbolName>,
        check_flags: ast::CheckFlags,
        declarations: ast::SymbolDeclarations,
        value_declaration: Option<ast::Node>,
        parent: Option<ast::SymbolHandle>,
    ) -> TransientSymbolHandle {
        self.symbol_count += 1;
        let symbol = self
            .transient_symbol_store
            .create_transient_symbol_from_instantiation(
                flags | ast::SYMBOL_FLAGS_TRANSIENT,
                name,
                check_flags,
                declarations,
                value_declaration,
                parent,
            );
        self.alloc_transient_symbol(symbol)
    }

    #[cfg(test)]
    pub(crate) fn transient_symbol_store(&self) -> &ast::TransientSymbolStore {
        &self.transient_symbol_store
    }

    pub(crate) fn is_transient_symbol_handle(&self, symbol: ast::SymbolHandle) -> bool {
        self.transient_symbol_store.owns(symbol)
    }

    pub(crate) fn with_transient_symbol_store<R>(
        &self,
        symbol: ast::SymbolHandle,
        f: impl FnOnce(&ast::TransientSymbolStore) -> R,
    ) -> Option<R> {
        self.is_transient_symbol_handle(symbol)
            .then(|| f(&self.transient_symbol_store))
    }

    pub(crate) fn transient_symbol_flags_if_owned(
        &self,
        symbol: ast::SymbolHandle,
    ) -> Option<ast::SymbolFlags> {
        self.is_transient_symbol_handle(symbol)
            .then(|| self.transient_symbol_store.flags_for_owned_handle(symbol))
    }

    pub(crate) fn transient_symbol_check_flags_if_owned(
        &self,
        symbol: ast::SymbolHandle,
    ) -> Option<ast::CheckFlags> {
        self.is_transient_symbol_handle(symbol).then(|| {
            self.transient_symbol_store
                .check_flags_for_owned_handle(symbol)
        })
    }

    pub(crate) fn transient_symbol_name_if_owned(
        &self,
        symbol: ast::SymbolHandle,
    ) -> Option<&ast::SymbolName> {
        self.is_transient_symbol_handle(symbol)
            .then(|| self.transient_symbol_store.name_for_owned_handle(symbol))
    }

    pub(crate) fn with_transient_symbol_declarations_owned<R>(
        &self,
        symbol: ast::SymbolHandle,
        f: impl FnOnce(&[ast::Node]) -> R,
    ) -> R {
        self.transient_symbol_store
            .with_declarations_for_owned_handle(symbol, f)
    }

    #[inline]
    pub(crate) fn share_transient_symbol_declarations_owned(
        &self,
        symbol: ast::SymbolHandle,
    ) -> ast::SymbolDeclarations {
        self.transient_symbol_store
            .share_declarations_for_owned_handle(symbol)
    }

    #[inline]
    pub(crate) fn transient_symbol_first_declaration_if_owned(
        &self,
        symbol: ast::SymbolHandle,
    ) -> Option<Option<ast::Node>> {
        self.is_transient_symbol_handle(symbol).then(|| {
            self.transient_symbol_store
                .first_declaration_for_owned_handle(symbol)
        })
    }

    pub(crate) fn transient_symbol_value_declaration_if_owned(
        &self,
        symbol: ast::SymbolHandle,
    ) -> Option<Option<ast::Node>> {
        self.is_transient_symbol_handle(symbol).then(|| {
            self.transient_symbol_store
                .value_declaration_for_owned_handle(symbol)
        })
    }

    pub(crate) fn transient_symbol_value_declaration_snapshot_if_owned(
        &self,
        symbol: ast::SymbolHandle,
    ) -> Option<ast::SymbolValueDeclarationSnapshot> {
        self.is_transient_symbol_handle(symbol).then(|| {
            self.transient_symbol_store
                .value_declaration_snapshot_for_owned_handle(symbol)
        })
    }

    pub(crate) fn transient_symbol_parent_if_owned(
        &self,
        symbol: ast::SymbolHandle,
    ) -> Option<Option<ast::SymbolHandle>> {
        self.is_transient_symbol_handle(symbol)
            .then(|| self.transient_symbol_store.parent_for_owned_handle(symbol))
    }

    pub(crate) fn transient_symbol_instantiation_header_if_owned(
        &self,
        symbol: ast::SymbolHandle,
    ) -> Option<ast::SymbolInstantiationHeader> {
        self.is_transient_symbol_handle(symbol).then(|| {
            self.transient_symbol_store
                .instantiation_header_for_owned_handle(symbol)
        })
    }

    pub(crate) fn transient_symbol_instantiation_snapshot_if_owned(
        &self,
        symbol: ast::SymbolHandle,
    ) -> Option<ast::SymbolInstantiationSnapshot> {
        self.is_transient_symbol_handle(symbol).then(|| {
            self.transient_symbol_store
                .instantiation_snapshot_for_owned_handle(symbol)
        })
    }

    pub(crate) fn transient_symbol_export_symbol_if_owned(
        &self,
        symbol: ast::SymbolHandle,
    ) -> Option<Option<ast::SymbolHandle>> {
        self.is_transient_symbol_handle(symbol).then(|| {
            self.transient_symbol_store
                .export_symbol_for_owned_handle(symbol)
        })
    }

    pub(crate) fn with_transient_symbol_members_owned<R>(
        &self,
        symbol: ast::SymbolHandle,
        f: impl FnOnce(Option<&ast::SymbolHandleTable>) -> R,
    ) -> R {
        self.transient_symbol_store
            .with_members_for_owned_handle(symbol, f)
    }

    pub(crate) fn with_transient_symbol_exports_owned<R>(
        &self,
        symbol: ast::SymbolHandle,
        f: impl FnOnce(Option<&ast::SymbolHandleTable>) -> R,
    ) -> R {
        self.transient_symbol_store
            .with_exports_for_owned_handle(symbol, f)
    }

    pub(crate) fn transient_symbol_record_handle(
        &self,
        symbol: ast::SymbolHandle,
    ) -> Option<TransientSymbolHandle> {
        self.transient_symbol_store
            .owns(symbol)
            .then(|| TransientSymbolHandle::new(symbol))
    }

    pub(crate) fn transient_symbol_handle(
        &self,
        handle: TransientSymbolHandle,
    ) -> ast::SymbolHandle {
        handle.symbol()
    }

    pub(crate) fn transient_symbol_identity(
        &self,
        handle: TransientSymbolHandle,
    ) -> SymbolIdentity {
        SymbolIdentity::from_symbol_handle(self.transient_symbol_handle(handle))
    }

    pub(crate) fn transient_symbol_check_flags(
        &self,
        handle: TransientSymbolHandle,
    ) -> ast::CheckFlags {
        self.transient_symbol_store
            .check_flags_for_owned_handle(self.transient_symbol_handle(handle))
    }

    pub fn set_transient_symbol_check_flags(
        &mut self,
        handle: TransientSymbolHandle,
        check_flags: ast::CheckFlags,
    ) {
        self.clear_declaration_modifier_flags_cache();
        let symbol = self.transient_symbol_handle(handle);
        self.transient_symbol_store
            .set_transient_check_flags(symbol, check_flags);
    }

    pub fn add_transient_symbol_check_flags(
        &mut self,
        handle: TransientSymbolHandle,
        check_flags: ast::CheckFlags,
    ) {
        self.clear_declaration_modifier_flags_cache();
        let symbol = self.transient_symbol_handle(handle);
        self.transient_symbol_store
            .add_transient_check_flags(symbol, check_flags);
    }

    pub(crate) fn add_transient_symbol_flags_for_symbol(
        &mut self,
        symbol: ast::SymbolHandle,
        flags: ast::SymbolFlags,
    ) {
        assert!(
            self.transient_symbol_store.owns(symbol),
            "checker-created transient SymbolHandle mutation requires a checker transient symbol"
        );
        self.clear_declaration_modifier_flags_cache();
        self.transient_symbol_store
            .add_transient_flags(symbol, flags);
    }

    pub(crate) fn remove_transient_symbol_flags_for_symbol(
        &mut self,
        symbol: ast::SymbolHandle,
        flags: ast::SymbolFlags,
    ) {
        assert!(
            self.transient_symbol_store.owns(symbol),
            "checker-created transient SymbolHandle mutation requires a checker transient symbol"
        );
        self.clear_declaration_modifier_flags_cache();
        self.transient_symbol_store
            .remove_transient_flags(symbol, flags);
    }

    pub(crate) fn add_transient_symbol_check_flags_for_symbol(
        &mut self,
        symbol: ast::SymbolHandle,
        check_flags: ast::CheckFlags,
    ) {
        assert!(
            self.transient_symbol_store.owns(symbol),
            "checker-created transient SymbolHandle mutation requires a checker transient symbol"
        );
        self.clear_declaration_modifier_flags_cache();
        self.transient_symbol_store
            .add_transient_check_flags(symbol, check_flags);
    }

    pub(crate) fn set_transient_symbol_check_flags_for_symbol(
        &mut self,
        symbol: ast::SymbolHandle,
        check_flags: ast::CheckFlags,
    ) {
        assert!(
            self.transient_symbol_store.owns(symbol),
            "checker-created transient SymbolHandle mutation requires a checker transient symbol"
        );
        self.clear_declaration_modifier_flags_cache();
        self.transient_symbol_store
            .set_transient_check_flags(symbol, check_flags);
    }

    pub(crate) fn set_transient_symbol_declarations_for_symbol(
        &mut self,
        symbol: ast::SymbolHandle,
        declarations: Vec<ast::Node>,
    ) {
        assert!(
            self.transient_symbol_store.owns(symbol),
            "checker-created transient SymbolHandle mutation requires a checker transient symbol"
        );
        self.clear_declaration_modifier_flags_cache();
        self.transient_symbol_store
            .set_transient_declarations(symbol, declarations);
    }

    pub(crate) fn add_transient_symbol_declaration_for_symbol(
        &mut self,
        symbol: ast::SymbolHandle,
        declaration: ast::Node,
    ) {
        assert!(
            self.transient_symbol_store.owns(symbol),
            "checker-created transient SymbolHandle mutation requires a checker transient symbol"
        );
        self.clear_declaration_modifier_flags_cache();
        self.transient_symbol_store
            .add_transient_declaration(symbol, declaration);
    }

    pub(crate) fn set_transient_symbol_value_declaration_for_symbol(
        &mut self,
        symbol: ast::SymbolHandle,
        value_declaration: Option<ast::Node>,
    ) {
        assert!(
            self.transient_symbol_store.owns(symbol),
            "checker-created transient SymbolHandle mutation requires a checker transient symbol"
        );
        self.clear_declaration_modifier_flags_cache();
        self.transient_symbol_store
            .set_transient_value_declaration(symbol, value_declaration);
    }

    pub(crate) fn set_transient_symbol_parent_for_symbol(
        &mut self,
        symbol: ast::SymbolHandle,
        parent: Option<ast::SymbolHandle>,
    ) {
        assert!(
            self.transient_symbol_store.owns(symbol),
            "checker-created transient SymbolHandle mutation requires a checker transient symbol"
        );
        self.clear_declaration_modifier_flags_cache();
        self.transient_symbol_store
            .set_transient_parent(symbol, parent);
    }

    pub(crate) fn set_transient_symbol_members_for_symbol(
        &mut self,
        symbol: ast::SymbolHandle,
        members: Option<ast::SymbolHandleTable>,
    ) {
        assert!(
            self.transient_symbol_store.owns(symbol),
            "checker-created transient SymbolHandle mutation requires a checker transient symbol"
        );
        self.transient_symbol_store
            .set_transient_members(symbol, members);
    }

    pub(crate) fn insert_transient_symbol_member_for_symbol(
        &mut self,
        symbol: ast::SymbolHandle,
        name: impl Into<ast::SymbolName>,
        member: ast::SymbolHandle,
    ) {
        assert!(
            self.transient_symbol_store.owns(symbol),
            "checker-created transient SymbolHandle mutation requires a checker transient symbol"
        );
        self.transient_symbol_store
            .insert_member(symbol, name, member);
    }

    pub(crate) fn set_transient_symbol_exports_for_symbol(
        &mut self,
        symbol: ast::SymbolHandle,
        exports: Option<ast::SymbolHandleTable>,
    ) {
        assert!(
            self.transient_symbol_store.owns(symbol),
            "checker-created transient SymbolHandle mutation requires a checker transient symbol"
        );
        self.transient_symbol_store
            .set_transient_exports(symbol, exports);
    }

    pub(crate) fn insert_transient_symbol_export_for_symbol(
        &mut self,
        symbol: ast::SymbolHandle,
        name: impl Into<ast::SymbolName>,
        export: ast::SymbolHandle,
    ) {
        assert!(
            self.transient_symbol_store.owns(symbol),
            "checker-created transient SymbolHandle mutation requires a checker transient symbol"
        );
        self.transient_symbol_store
            .insert_export(symbol, name, export);
    }

    pub(crate) fn set_transient_symbol_export_symbol_for_symbol(
        &mut self,
        symbol: ast::SymbolHandle,
        export_symbol: Option<ast::SymbolHandle>,
    ) {
        assert!(
            self.transient_symbol_store.owns(symbol),
            "checker-created transient SymbolHandle mutation requires a checker transient symbol"
        );
        self.transient_symbol_store
            .set_transient_export_symbol(symbol, export_symbol);
    }

    pub fn alloc_inference_context(
        &mut self,
        record: InferenceContextRecord,
    ) -> InferenceContextHandle {
        InferenceContextHandle::new(self.inference_contexts.alloc(record))
    }

    pub fn inference_context_record(
        &self,
        handle: InferenceContextHandle,
    ) -> &InferenceContextRecord {
        &self.inference_contexts[handle.idx()]
    }

    pub(crate) fn inference_context_record_mut(
        &mut self,
        handle: InferenceContextHandle,
    ) -> &mut InferenceContextRecord {
        &mut self.inference_contexts[handle.idx()]
    }

    pub fn type_count(&self) -> usize {
        self.types.len()
    }

    pub fn signature_count(&self) -> usize {
        self.signatures.len()
    }

    pub(crate) fn semantic_handles(&self) -> &CheckerSemanticHandles {
        &self.semantic_handles
    }

    pub(crate) fn semantic_handles_mut(&mut self) -> &mut CheckerSemanticHandles {
        &mut self.semantic_handles
    }

    pub(crate) fn string_literal_type(&self, value: &str) -> Option<TypeHandle> {
        self.semantic_caches
            .string_literal_types
            .get(value)
            .copied()
    }

    pub(crate) fn set_string_literal_type(&mut self, value: String, t: TypeHandle) {
        self.semantic_caches.string_literal_types.insert(value, t);
    }

    pub(crate) fn number_literal_type(&self, value: Number) -> Option<TypeHandle> {
        self.semantic_caches
            .number_literal_types
            .get(&value)
            .copied()
    }

    pub(crate) fn set_number_literal_type(&mut self, value: Number, t: TypeHandle) {
        self.semantic_caches.number_literal_types.insert(value, t);
    }

    pub(crate) fn bigint_literal_type(&self, value: &PseudoBigInt) -> Option<TypeHandle> {
        self.semantic_caches
            .bigint_literal_types
            .get(value)
            .copied()
    }

    pub(crate) fn set_bigint_literal_type(&mut self, value: PseudoBigInt, t: TypeHandle) {
        self.semantic_caches.bigint_literal_types.insert(value, t);
    }

    pub(crate) fn enum_literal_type(&self, key: &EnumLiteralKey) -> Option<TypeHandle> {
        self.semantic_caches.enum_literal_types.get(key).copied()
    }

    pub(crate) fn set_enum_literal_type(&mut self, key: EnumLiteralKey, t: TypeHandle) {
        self.semantic_caches.enum_literal_types.insert(key, t);
    }

    pub(crate) fn clear_literal_type_caches(&mut self) {
        self.semantic_caches.string_literal_types = HashMap::new();
        self.semantic_caches.number_literal_types = HashMap::new();
        self.semantic_caches.bigint_literal_types = HashMap::new();
        self.semantic_caches.enum_literal_types = HashMap::new();
    }

    pub(crate) fn indexed_access_type(&self, key: CacheHashKey) -> Option<TypeHandle> {
        self.semantic_caches.indexed_access_types.get(&key).copied()
    }

    pub(crate) fn set_indexed_access_type(&mut self, key: CacheHashKey, t: TypeHandle) {
        self.semantic_caches.indexed_access_types.insert(key, t);
    }

    pub(crate) fn clear_indexed_access_types(&mut self) {
        self.semantic_caches.indexed_access_types = HashMap::new();
    }

    pub(crate) fn template_literal_type(&self, key: CacheHashKey) -> Option<TypeHandle> {
        self.semantic_caches
            .template_literal_types
            .get(&key)
            .copied()
    }

    pub(crate) fn set_template_literal_type(&mut self, key: CacheHashKey, t: TypeHandle) {
        self.semantic_caches.template_literal_types.insert(key, t);
    }

    pub(crate) fn clear_template_literal_types(&mut self) {
        self.semantic_caches.template_literal_types = HashMap::new();
    }

    pub(crate) fn string_mapping_type(&self, key: &StringMappingKey) -> Option<TypeHandle> {
        self.semantic_caches.string_mapping_types.get(key).copied()
    }

    pub(crate) fn set_string_mapping_type(&mut self, key: StringMappingKey, t: TypeHandle) {
        self.semantic_caches.string_mapping_types.insert(key, t);
    }

    pub(crate) fn clear_string_mapping_types(&mut self) {
        self.semantic_caches.string_mapping_types = HashMap::new();
    }

    pub(crate) fn substitution_type(&self, key: &SubstitutionTypeKey) -> Option<TypeHandle> {
        self.semantic_caches.substitution_types.get(key).copied()
    }

    pub(crate) fn set_substitution_type(&mut self, key: SubstitutionTypeKey, t: TypeHandle) {
        self.semantic_caches.substitution_types.insert(key, t);
    }

    pub(crate) fn clear_substitution_types(&mut self) {
        self.semantic_caches.substitution_types = HashMap::new();
    }

    pub(crate) fn reverse_homomorphic_mapped_type(
        &self,
        key: ReverseMappedTypeKey,
    ) -> Option<Option<TypeHandle>> {
        self.semantic_caches
            .reverse_homomorphic_mapped_cache
            .get(&key)
            .copied()
    }

    pub(crate) fn set_reverse_homomorphic_mapped_type(
        &mut self,
        key: ReverseMappedTypeKey,
        t: Option<TypeHandle>,
    ) {
        self.semantic_caches
            .reverse_homomorphic_mapped_cache
            .insert(key, t);
    }

    pub(crate) fn clear_reverse_homomorphic_mapped_cache(&mut self) {
        self.semantic_caches.reverse_homomorphic_mapped_cache = HashMap::new();
    }

    pub(crate) fn reverse_mapped_type(
        &self,
        key: ReverseMappedTypeKey,
    ) -> Option<Option<TypeHandle>> {
        self.semantic_caches.reverse_mapped_cache.get(&key).copied()
    }

    pub(crate) fn set_reverse_mapped_type(
        &mut self,
        key: ReverseMappedTypeKey,
        t: Option<TypeHandle>,
    ) {
        self.semantic_caches.reverse_mapped_cache.insert(key, t);
    }

    pub(crate) fn clear_reverse_mapped_cache(&mut self) {
        self.semantic_caches.reverse_mapped_cache = HashMap::new();
    }

    pub(crate) fn push_reverse_mapped_types(&mut self, source: TypeHandle, target: TypeHandle) {
        self.semantic_caches
            .reverse_mapped_source_stack
            .push(source);
        self.semantic_caches
            .reverse_mapped_target_stack
            .push(target);
    }

    pub(crate) fn pop_reverse_mapped_types(&mut self) {
        self.semantic_caches.reverse_mapped_source_stack.pop();
        self.semantic_caches.reverse_mapped_target_stack.pop();
    }

    pub(crate) fn reverse_mapped_source_stack_snapshot(&self) -> Vec<TypeHandle> {
        self.semantic_caches.reverse_mapped_source_stack.clone()
    }

    pub(crate) fn reverse_mapped_target_stack_snapshot(&self) -> Vec<TypeHandle> {
        self.semantic_caches.reverse_mapped_target_stack.clone()
    }

    pub(crate) fn iteration_types(&self, key: IterationTypesKey) -> Option<IterationTypes> {
        self.semantic_caches
            .iteration_types_cache
            .get(&key)
            .copied()
    }

    pub(crate) fn set_iteration_types(&mut self, key: IterationTypesKey, types: IterationTypes) {
        self.semantic_caches
            .iteration_types_cache
            .insert(key, types);
    }

    pub(crate) fn clear_iteration_types_cache(&mut self) {
        self.semantic_caches.iteration_types_cache = HashMap::new();
    }

    fn relation_cache(&self, relation: RelationKind) -> &Relation {
        match relation {
            RelationKind::Subtype => &self.semantic_caches.subtype_relation_cache,
            RelationKind::StrictSubtype => &self.semantic_caches.strict_subtype_relation_cache,
            RelationKind::Assignable => &self.semantic_caches.assignable_relation_cache,
            RelationKind::Comparable => &self.semantic_caches.comparable_relation_cache,
            RelationKind::Identity => &self.semantic_caches.identity_relation_cache,
        }
    }

    fn relation_cache_mut(&mut self, relation: RelationKind) -> &mut Relation {
        match relation {
            RelationKind::Subtype => &mut self.semantic_caches.subtype_relation_cache,
            RelationKind::StrictSubtype => &mut self.semantic_caches.strict_subtype_relation_cache,
            RelationKind::Assignable => &mut self.semantic_caches.assignable_relation_cache,
            RelationKind::Comparable => &mut self.semantic_caches.comparable_relation_cache,
            RelationKind::Identity => &mut self.semantic_caches.identity_relation_cache,
        }
    }

    pub(crate) fn relation_cache_size(&self, relation: RelationKind) -> usize {
        self.relation_cache(relation).size()
    }

    pub(crate) fn relation_result(
        &self,
        relation: RelationKind,
        key: CacheHashKey,
    ) -> RelationComparisonResult {
        self.relation_cache(relation).get(key)
    }

    pub(crate) fn set_relation_result(
        &mut self,
        relation: RelationKind,
        key: CacheHashKey,
        result: RelationComparisonResult,
    ) {
        self.relation_cache_mut(relation).set(key, result);
    }

    pub(crate) fn clear_relation_caches(&mut self) {
        self.semantic_caches.subtype_relation_cache = Relation::new();
        self.semantic_caches.strict_subtype_relation_cache = Relation::new();
        self.semantic_caches.assignable_relation_cache = Relation::new();
        self.semantic_caches.comparable_relation_cache = Relation::new();
        self.semantic_caches.identity_relation_cache = Relation::new();
    }

    pub(crate) fn unique_es_symbol_type(&self, symbol: SymbolIdentity) -> Option<TypeHandle> {
        self.semantic_caches
            .unique_es_symbol_types
            .get(&symbol)
            .copied()
    }

    pub(crate) fn set_unique_es_symbol_type(&mut self, symbol: SymbolIdentity, t: TypeHandle) {
        self.semantic_caches
            .unique_es_symbol_types
            .insert(symbol, t);
    }

    pub(crate) fn clear_unique_es_symbol_types(&mut self) {
        self.semantic_caches.unique_es_symbol_types = HashMap::new();
    }

    pub(crate) fn subtype_reduction(&self, key: CacheHashKey) -> Option<Vec<TypeHandle>> {
        self.semantic_caches
            .subtype_reduction_cache
            .get(&key)
            .cloned()
    }

    pub(crate) fn set_subtype_reduction(&mut self, key: CacheHashKey, types: Vec<TypeHandle>) {
        self.semantic_caches
            .subtype_reduction_cache
            .insert(key, types);
    }

    pub(crate) fn clear_subtype_reduction_cache(&mut self) {
        self.semantic_caches.subtype_reduction_cache = HashMap::new();
    }

    pub(crate) fn error_type(&self, key: CacheHashKey) -> Option<TypeHandle> {
        self.semantic_caches.error_types.get(&key).copied()
    }

    pub(crate) fn set_error_type(&mut self, key: CacheHashKey, t: TypeHandle) {
        self.semantic_caches.error_types.insert(key, t);
    }

    pub(crate) fn clear_error_types(&mut self) {
        self.semantic_caches.error_types = HashMap::new();
    }

    pub(crate) fn tuple_type(&self, key: CacheHashKey) -> Option<TypeHandle> {
        self.semantic_caches.tuple_types.get(&key).copied()
    }

    pub(crate) fn set_tuple_type(&mut self, key: CacheHashKey, t: TypeHandle) {
        self.semantic_caches.tuple_types.insert(key, t);
    }

    pub(crate) fn clear_tuple_types(&mut self) {
        self.semantic_caches.tuple_types = HashMap::new();
    }

    pub(crate) fn union_type(&self, key: CacheHashKey) -> Option<TypeHandle> {
        self.semantic_caches.union_types.get(&key).copied()
    }

    pub(crate) fn set_union_type(&mut self, key: CacheHashKey, t: TypeHandle) {
        self.semantic_caches.union_types.insert(key, t);
    }

    pub(crate) fn union_types(&self) -> impl Iterator<Item = TypeHandle> + '_ {
        self.semantic_caches.union_types.values().copied()
    }

    pub(crate) fn clear_union_types(&mut self) {
        self.semantic_caches.union_types = HashMap::new();
    }

    pub(crate) fn union_of_union_type(&self, key: &UnionOfUnionKey) -> Option<TypeHandle> {
        self.semantic_caches.union_of_union_types.get(key).copied()
    }

    pub(crate) fn set_union_of_union_type(&mut self, key: UnionOfUnionKey, t: TypeHandle) {
        self.semantic_caches.union_of_union_types.insert(key, t);
    }

    pub(crate) fn clear_union_of_union_types(&mut self) {
        self.semantic_caches.union_of_union_types = HashMap::new();
    }

    pub(crate) fn intersection_type(&self, key: CacheHashKey) -> Option<TypeHandle> {
        self.semantic_caches.intersection_types.get(&key).copied()
    }

    pub(crate) fn set_intersection_type(&mut self, key: CacheHashKey, t: TypeHandle) {
        self.semantic_caches.intersection_types.insert(key, t);
    }

    pub(crate) fn clear_intersection_types(&mut self) {
        self.semantic_caches.intersection_types = HashMap::new();
    }

    pub(crate) fn properties_type(&self, key: &PropertiesTypesKey) -> Option<TypeHandle> {
        self.semantic_caches.properties_types.get(key).copied()
    }

    pub(crate) fn set_properties_type(&mut self, key: PropertiesTypesKey, t: TypeHandle) {
        self.semantic_caches.properties_types.insert(key, t);
    }

    pub(crate) fn clear_properties_types(&mut self) {
        self.semantic_caches.properties_types = HashMap::new();
    }

    pub(crate) fn cached_type(&self, key: CachedTypeKey) -> Option<TypeHandle> {
        self.semantic_caches.cached_types.get(&key).copied()
    }

    pub(crate) fn set_cached_type(&mut self, key: CachedTypeKey, t: TypeHandle) {
        self.semantic_caches.cached_types.insert(key, t);
    }

    pub(crate) fn clear_cached_types(&mut self) {
        self.semantic_caches.cached_types = HashMap::new();
    }

    pub(crate) fn cached_signature(&self, key: CachedSignatureKey) -> Option<SignatureHandle> {
        self.semantic_caches.cached_signatures.get(&key).copied()
    }

    pub(crate) fn set_cached_signature(
        &mut self,
        key: CachedSignatureKey,
        signature: SignatureHandle,
    ) {
        self.semantic_caches
            .cached_signatures
            .insert(key, signature);
    }

    pub(crate) fn clear_cached_signatures(&mut self) {
        self.semantic_caches.cached_signatures = HashMap::new();
    }

    pub(crate) fn narrowed_type(&self, key: NarrowedTypeKey) -> Option<TypeHandle> {
        self.semantic_caches.narrowed_types.get(&key).copied()
    }

    pub(crate) fn set_narrowed_type(&mut self, key: NarrowedTypeKey, t: TypeHandle) {
        self.semantic_caches.narrowed_types.insert(key, t);
    }

    pub(crate) fn clear_narrowed_types(&mut self) {
        self.semantic_caches.narrowed_types = HashMap::new();
    }

    pub(crate) fn assignment_reduced_type(&self, key: AssignmentReducedKey) -> Option<TypeHandle> {
        self.semantic_caches
            .assignment_reduced_types
            .get(&key)
            .copied()
    }

    pub(crate) fn set_assignment_reduced_type(&mut self, key: AssignmentReducedKey, t: TypeHandle) {
        self.semantic_caches.assignment_reduced_types.insert(key, t);
    }

    pub(crate) fn clear_assignment_reduced_types(&mut self) {
        self.semantic_caches.assignment_reduced_types = HashMap::new();
    }

    pub(crate) fn discriminated_contextual_type(
        &self,
        key: &DiscriminatedContextualTypeKey,
    ) -> Option<TypeHandle> {
        self.semantic_caches
            .discriminated_contextual_types
            .get(key)
            .copied()
    }

    pub(crate) fn set_discriminated_contextual_type(
        &mut self,
        key: DiscriminatedContextualTypeKey,
        t: TypeHandle,
    ) {
        self.semantic_caches
            .discriminated_contextual_types
            .insert(key, t);
    }

    pub(crate) fn clear_discriminated_contextual_types(&mut self) {
        self.semantic_caches.discriminated_contextual_types = HashMap::new();
    }

    pub(crate) fn instantiation_expression_type(
        &self,
        key: &InstantiationExpressionKey,
    ) -> Option<TypeHandle> {
        self.semantic_caches
            .instantiation_expression_types
            .get(key)
            .copied()
    }

    pub(crate) fn set_instantiation_expression_type(
        &mut self,
        key: InstantiationExpressionKey,
        t: TypeHandle,
    ) {
        self.semantic_caches
            .instantiation_expression_types
            .insert(key, t);
    }

    pub(crate) fn clear_instantiation_expression_types(&mut self) {
        self.semantic_caches.instantiation_expression_types = HashMap::new();
    }

    pub(crate) fn pattern_for_type(&self, t: TypeHandle) -> Option<ast::Node> {
        self.semantic_caches.pattern_for_type.get(&t).copied()
    }

    pub(crate) fn has_pattern_for_type(&self, t: TypeHandle) -> bool {
        self.semantic_caches.pattern_for_type.contains_key(&t)
    }

    pub(crate) fn set_pattern_for_type(&mut self, t: TypeHandle, pattern: ast::Node) {
        self.semantic_caches.pattern_for_type.insert(t, pattern);
    }

    pub(crate) fn clear_pattern_for_type(&mut self) {
        self.semantic_caches.pattern_for_type = HashMap::new();
    }

    pub(crate) fn context_free_type(&self, node: ast::Node) -> Option<TypeHandle> {
        self.semantic_caches.context_free_types.get(&node).copied()
    }

    pub(crate) fn set_context_free_type(&mut self, node: ast::Node, t: TypeHandle) {
        self.semantic_caches.context_free_types.insert(node, t);
    }

    pub(crate) fn clear_context_free_types(&mut self) {
        self.semantic_caches.context_free_types = HashMap::new();
    }

    pub(crate) fn flow_type(&self, node: ast::Node) -> Option<TypeHandle> {
        self.semantic_caches.flow_type_cache.get(&node).copied()
    }

    pub(crate) fn set_flow_type(&mut self, node: ast::Node, t: TypeHandle) {
        self.semantic_caches.flow_type_cache.insert(node, t);
    }

    pub(crate) fn suspend_flow_type_cache(&mut self) -> HashMap<ast::Node, TypeHandle> {
        std::mem::take(&mut self.semantic_caches.flow_type_cache)
    }

    pub(crate) fn restore_flow_type_cache(&mut self, cache: HashMap<ast::Node, TypeHandle>) {
        self.semantic_caches.flow_type_cache = cache;
    }

    pub(crate) fn flow_loop_type(&self, key: &FlowLoopKey) -> Option<TypeHandle> {
        self.semantic_caches.flow_loop_cache.get(key).copied()
    }

    pub(crate) fn set_flow_loop_type(&mut self, key: FlowLoopKey, t: TypeHandle) {
        self.semantic_caches.flow_loop_cache.insert(key, t);
    }

    pub(crate) fn clear_flow_loop_cache(&mut self) {
        self.semantic_caches.flow_loop_cache = HashMap::new();
    }

    pub(crate) fn antecedent_type_checkpoint(&self) -> usize {
        self.semantic_caches.antecedent_types.len()
    }

    pub(crate) fn has_antecedent_type_since(&self, start: usize, t: TypeHandle) -> bool {
        self.semantic_caches.antecedent_types[start..].contains(&t)
    }

    pub(crate) fn push_antecedent_type(&mut self, t: TypeHandle) {
        self.semantic_caches.antecedent_types.push(t);
    }

    pub(crate) fn antecedent_types_since(&self, start: usize) -> Vec<TypeHandle> {
        self.semantic_caches.antecedent_types[start..].to_vec()
    }

    pub(crate) fn truncate_antecedent_types(&mut self, len: usize) {
        self.semantic_caches.antecedent_types.truncate(len);
    }

    pub(crate) fn synthetic_expression_type(&self, node: ast::Node) -> Option<TypeHandle> {
        self.semantic_caches
            .synthetic_expression_types
            .get(&node)
            .copied()
    }

    pub(crate) fn set_synthetic_expression_type(&mut self, node: ast::Node, t: TypeHandle) {
        self.semantic_caches
            .synthetic_expression_types
            .insert(node, t);
    }

    pub(crate) fn clear_synthetic_expression_types(&mut self) {
        self.semantic_caches.synthetic_expression_types = HashMap::new();
    }

    pub(crate) fn binary_expression_result(&self, node: ast::Node) -> Option<TypeHandle> {
        self.semantic_caches
            .binary_expression_results
            .get(&node)
            .copied()
    }

    pub(crate) fn has_binary_expression_result(&self, node: ast::Node) -> bool {
        self.semantic_caches
            .binary_expression_results
            .contains_key(&node)
    }

    pub(crate) fn set_binary_expression_result(&mut self, node: ast::Node, t: TypeHandle) {
        self.semantic_caches
            .binary_expression_results
            .insert(node, t);
    }

    pub(crate) fn remove_binary_expression_result(
        &mut self,
        node: ast::Node,
    ) -> Option<TypeHandle> {
        self.semantic_caches.binary_expression_results.remove(&node)
    }

    pub(crate) fn clear_binary_expression_results(&mut self) {
        self.semantic_caches.binary_expression_results = HashMap::new();
    }

    pub(crate) fn push_contextual_info(
        &mut self,
        node: ast::Node,
        t: Option<TypeHandle>,
        is_cache: bool,
    ) {
        self.semantic_caches.contextual_infos.push(ContextualInfo {
            node: Some(node),
            t,
            is_cache,
        });
    }

    pub(crate) fn pop_contextual_info(&mut self) {
        let last_index = self
            .semantic_caches
            .contextual_infos
            .len()
            .checked_sub(1)
            .expect("contextual info stack must not be empty when popped");
        self.semantic_caches.contextual_infos[last_index] = ContextualInfo::default();
        self.semantic_caches.contextual_infos.truncate(last_index);
    }

    pub(crate) fn find_contextual_node_index(
        &self,
        node: ast::Node,
        include_caches: bool,
    ) -> Option<usize> {
        self.semantic_caches
            .contextual_infos
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, info)| {
                (info.node == Some(node) && (include_caches || !info.is_cache)).then_some(index)
            })
    }

    pub(crate) fn contextual_info_type_at(&self, index: usize) -> Option<TypeHandle> {
        self.semantic_caches
            .contextual_infos
            .get(index)
            .and_then(|info| info.t)
    }
}

pub(crate) const PRIMITIVE_TYPE_ALIAS_SUGGESTION_BUILTINS: &[&str] =
    &["String", "Number", "Boolean", "Object", "BigInt", "Symbol"];

fn primitive_type_alias_suggestion_name(
    builtin_name: &str,
) -> Option<(&'static str, &'static str)> {
    match builtin_name {
        "String" => Some(("String", "string")),
        "Number" => Some(("Number", "number")),
        "Boolean" => Some(("Boolean", "boolean")),
        "Object" => Some(("Object", "object")),
        "BigInt" => Some(("BigInt", "bigint")),
        "Symbol" => Some(("Symbol", "symbol")),
        _ => None,
    }
}

// CheckerState is the production boundary for checker link caches. Keep
// these delegations narrow: callsites use typed helpers, not whole stores.
impl NodeLinksStoreExt for CheckerState {
    fn node_link_handle(&self, node: ast::Node) -> core::LinkHandle<NodeLinks> {
        <NodeLinkStore<NodeLinks> as NodeLinksStoreExt>::node_link_handle(&self.node_links, node)
    }

    fn node_link_flags(&self, node: ast::Node) -> NodeCheckFlags {
        <NodeLinkStore<NodeLinks> as NodeLinksStoreExt>::node_link_flags(&self.node_links, node)
    }

    fn node_link_flags_by_handle(&self, handle: core::LinkHandle<NodeLinks>) -> NodeCheckFlags {
        <NodeLinkStore<NodeLinks> as NodeLinksStoreExt>::node_link_flags_by_handle(
            &self.node_links,
            handle,
        )
    }

    fn set_node_link_flags(&self, node: ast::Node, flags: NodeCheckFlags) {
        <NodeLinkStore<NodeLinks> as NodeLinksStoreExt>::set_node_link_flags(
            &self.node_links,
            node,
            flags,
        )
    }

    fn set_node_link_flags_by_handle(
        &self,
        handle: core::LinkHandle<NodeLinks>,
        flags: NodeCheckFlags,
    ) {
        <NodeLinkStore<NodeLinks> as NodeLinksStoreExt>::set_node_link_flags_by_handle(
            &self.node_links,
            handle,
            flags,
        )
    }

    fn add_node_link_flags(&self, node: ast::Node, flags: NodeCheckFlags) {
        <NodeLinkStore<NodeLinks> as NodeLinksStoreExt>::add_node_link_flags(
            &self.node_links,
            node,
            flags,
        )
    }

    fn add_node_link_flags_by_handle(
        &self,
        handle: core::LinkHandle<NodeLinks>,
        flags: NodeCheckFlags,
    ) {
        <NodeLinkStore<NodeLinks> as NodeLinksStoreExt>::add_node_link_flags_by_handle(
            &self.node_links,
            handle,
            flags,
        )
    }

    fn remove_node_link_flags(&self, node: ast::Node, flags: NodeCheckFlags) {
        <NodeLinkStore<NodeLinks> as NodeLinksStoreExt>::remove_node_link_flags(
            &self.node_links,
            node,
            flags,
        )
    }

    fn remove_node_link_flags_by_handle(
        &self,
        handle: core::LinkHandle<NodeLinks>,
        flags: NodeCheckFlags,
    ) {
        <NodeLinkStore<NodeLinks> as NodeLinksStoreExt>::remove_node_link_flags_by_handle(
            &self.node_links,
            handle,
            flags,
        )
    }

    fn has_node_link_flags(&self, node: ast::Node, flags: NodeCheckFlags) -> bool {
        <NodeLinkStore<NodeLinks> as NodeLinksStoreExt>::has_node_link_flags(
            &self.node_links,
            node,
            flags,
        )
    }

    fn has_node_link_flags_by_handle(
        &self,
        handle: core::LinkHandle<NodeLinks>,
        flags: NodeCheckFlags,
    ) -> bool {
        <NodeLinkStore<NodeLinks> as NodeLinksStoreExt>::has_node_link_flags_by_handle(
            &self.node_links,
            handle,
            flags,
        )
    }

    fn node_declaration_requires_scope_change(&self, node: ast::Node) -> core::Tristate {
        <NodeLinkStore<NodeLinks> as NodeLinksStoreExt>::node_declaration_requires_scope_change(
            &self.node_links,
            node,
        )
    }

    fn node_declaration_requires_scope_change_by_handle(
        &self,
        handle: core::LinkHandle<NodeLinks>,
    ) -> core::Tristate {
        <NodeLinkStore<NodeLinks> as NodeLinksStoreExt>::node_declaration_requires_scope_change_by_handle(&self.node_links, handle)
    }

    fn set_node_declaration_requires_scope_change(&self, node: ast::Node, value: core::Tristate) {
        <NodeLinkStore<NodeLinks> as NodeLinksStoreExt>::set_node_declaration_requires_scope_change(
            &self.node_links,
            node,
            value,
        )
    }

    fn set_node_declaration_requires_scope_change_by_handle(
        &self,
        handle: core::LinkHandle<NodeLinks>,
        value: core::Tristate,
    ) {
        <NodeLinkStore<NodeLinks> as NodeLinksStoreExt>::set_node_declaration_requires_scope_change_by_handle(&self.node_links, handle, value)
    }

    fn node_has_reported_statement_in_ambient_context(&self, node: ast::Node) -> bool {
        <NodeLinkStore<NodeLinks> as NodeLinksStoreExt>::node_has_reported_statement_in_ambient_context(&self.node_links, node)
    }

    fn set_node_has_reported_statement_in_ambient_context(&self, node: ast::Node, value: bool) {
        <NodeLinkStore<NodeLinks> as NodeLinksStoreExt>::set_node_has_reported_statement_in_ambient_context(&self.node_links, node, value)
    }
}

impl SymbolNodeLinksStoreExt for CheckerState {
    fn symbol_node_link_handle(&self, node: ast::Node) -> core::LinkHandle<SymbolNodeLinks> {
        <NodeLinkStore<SymbolNodeLinks> as SymbolNodeLinksStoreExt>::symbol_node_link_handle(
            &self.symbol_node_links,
            node,
        )
    }

    fn node_resolved_symbol_identity(&self, node: ast::Node) -> Option<SymbolIdentity> {
        <NodeLinkStore<SymbolNodeLinks> as SymbolNodeLinksStoreExt>::node_resolved_symbol_identity(
            &self.symbol_node_links,
            node,
        )
    }

    fn node_resolved_symbol_identity_by_handle(
        &self,
        handle: core::LinkHandle<SymbolNodeLinks>,
    ) -> Option<SymbolIdentity> {
        <NodeLinkStore<SymbolNodeLinks> as SymbolNodeLinksStoreExt>::node_resolved_symbol_identity_by_handle(&self.symbol_node_links, handle)
    }

    fn set_node_resolved_symbol_identity(&self, node: ast::Node, symbol: Option<SymbolIdentity>) {
        <NodeLinkStore<SymbolNodeLinks> as SymbolNodeLinksStoreExt>::set_node_resolved_symbol_identity(&self.symbol_node_links, node, symbol)
    }

    fn set_node_resolved_symbol_identity_by_handle(
        &self,
        handle: core::LinkHandle<SymbolNodeLinks>,
        symbol: Option<SymbolIdentity>,
    ) {
        <NodeLinkStore<SymbolNodeLinks> as SymbolNodeLinksStoreExt>::set_node_resolved_symbol_identity_by_handle(&self.symbol_node_links, handle, symbol)
    }
}

impl TypeNodeLinksStoreExt for CheckerState {
    fn type_node_link_handle(&self, node: ast::Node) -> core::LinkHandle<TypeNodeLinks> {
        <NodeLinkStore<TypeNodeLinks> as TypeNodeLinksStoreExt>::type_node_link_handle(
            &self.semantic_caches.type_node_links,
            node,
        )
    }

    fn type_node_resolved_type(&self, node: ast::Node) -> Option<TypeHandle> {
        <NodeLinkStore<TypeNodeLinks> as TypeNodeLinksStoreExt>::type_node_resolved_type(
            &self.semantic_caches.type_node_links,
            node,
        )
    }

    fn type_node_resolved_type_by_handle(
        &self,
        handle: core::LinkHandle<TypeNodeLinks>,
    ) -> Option<TypeHandle> {
        <NodeLinkStore<TypeNodeLinks> as TypeNodeLinksStoreExt>::type_node_resolved_type_by_handle(
            &self.semantic_caches.type_node_links,
            handle,
        )
    }

    fn try_type_node_resolved_type(&self, node: ast::Node) -> Option<TypeHandle> {
        <NodeLinkStore<TypeNodeLinks> as TypeNodeLinksStoreExt>::try_type_node_resolved_type(
            &self.semantic_caches.type_node_links,
            node,
        )
    }

    fn set_type_node_resolved_type(&self, node: ast::Node, resolved_type: Option<TypeHandle>) {
        <NodeLinkStore<TypeNodeLinks> as TypeNodeLinksStoreExt>::set_type_node_resolved_type(
            &self.semantic_caches.type_node_links,
            node,
            resolved_type,
        )
    }

    fn set_type_node_resolved_type_by_handle(
        &self,
        handle: core::LinkHandle<TypeNodeLinks>,
        resolved_type: Option<TypeHandle>,
    ) {
        <NodeLinkStore<TypeNodeLinks> as TypeNodeLinksStoreExt>::set_type_node_resolved_type_by_handle(&self.semantic_caches.type_node_links, handle, resolved_type)
    }

    fn type_node_outer_type_parameters(&self, node: ast::Node) -> Option<Arc<[TypeHandle]>> {
        <NodeLinkStore<TypeNodeLinks> as TypeNodeLinksStoreExt>::type_node_outer_type_parameters(
            &self.semantic_caches.type_node_links,
            node,
        )
    }

    fn type_node_outer_type_parameters_by_handle(
        &self,
        handle: core::LinkHandle<TypeNodeLinks>,
    ) -> Option<Arc<[TypeHandle]>> {
        <NodeLinkStore<TypeNodeLinks> as TypeNodeLinksStoreExt>::type_node_outer_type_parameters_by_handle(&self.semantic_caches.type_node_links, handle)
    }

    fn set_type_node_outer_type_parameters<T>(&self, node: ast::Node, outer_type_parameters: T)
    where
        T: Into<Arc<[TypeHandle]>>,
    {
        <NodeLinkStore<TypeNodeLinks> as TypeNodeLinksStoreExt>::set_type_node_outer_type_parameters(
            &self.semantic_caches.type_node_links,
            node,
            outer_type_parameters,
        )
    }

    fn set_type_node_outer_type_parameters_by_handle<T>(
        &self,
        handle: core::LinkHandle<TypeNodeLinks>,
        outer_type_parameters: T,
    ) where
        T: Into<Arc<[TypeHandle]>>,
    {
        <NodeLinkStore<TypeNodeLinks> as TypeNodeLinksStoreExt>::set_type_node_outer_type_parameters_by_handle(&self.semantic_caches.type_node_links, handle, outer_type_parameters)
    }
}

impl AssertionLinksStoreExt for CheckerState {
    fn assertion_link_handle(&self, node: ast::Node) -> core::LinkHandle<AssertionLinks> {
        <NodeLinkStore<AssertionLinks> as AssertionLinksStoreExt>::assertion_link_handle(
            &self.semantic_caches.assertion_links,
            node,
        )
    }

    fn assertion_expression_type(&self, node: ast::Node) -> Option<TypeHandle> {
        <NodeLinkStore<AssertionLinks> as AssertionLinksStoreExt>::assertion_expression_type(
            &self.semantic_caches.assertion_links,
            node,
        )
    }

    fn assertion_expression_type_by_handle(
        &self,
        handle: core::LinkHandle<AssertionLinks>,
    ) -> Option<TypeHandle> {
        <NodeLinkStore<AssertionLinks> as AssertionLinksStoreExt>::assertion_expression_type_by_handle(&self.semantic_caches.assertion_links, handle)
    }

    fn set_assertion_expression_type(&self, node: ast::Node, expr_type: Option<TypeHandle>) {
        <NodeLinkStore<AssertionLinks> as AssertionLinksStoreExt>::set_assertion_expression_type(
            &self.semantic_caches.assertion_links,
            node,
            expr_type,
        )
    }

    fn set_assertion_expression_type_by_handle(
        &self,
        handle: core::LinkHandle<AssertionLinks>,
        expr_type: Option<TypeHandle>,
    ) {
        <NodeLinkStore<AssertionLinks> as AssertionLinksStoreExt>::set_assertion_expression_type_by_handle(&self.semantic_caches.assertion_links, handle, expr_type)
    }
}

impl SignatureLinksStoreExt for CheckerState {
    fn signature_link_handle(&self, node: ast::Node) -> core::LinkHandle<SignatureLinks> {
        <NodeLinkStore<SignatureLinks> as SignatureLinksStoreExt>::signature_link_handle(
            &self.semantic_caches.signature_links,
            node,
        )
    }

    fn resolved_signature(&self, node: ast::Node) -> Option<SignatureHandle> {
        <NodeLinkStore<SignatureLinks> as SignatureLinksStoreExt>::resolved_signature(
            &self.semantic_caches.signature_links,
            node,
        )
    }

    fn resolved_signature_by_handle(
        &self,
        handle: core::LinkHandle<SignatureLinks>,
    ) -> Option<SignatureHandle> {
        <NodeLinkStore<SignatureLinks> as SignatureLinksStoreExt>::resolved_signature_by_handle(
            &self.semantic_caches.signature_links,
            handle,
        )
    }

    fn set_resolved_signature(&self, node: ast::Node, signature: Option<SignatureHandle>) {
        <NodeLinkStore<SignatureLinks> as SignatureLinksStoreExt>::set_resolved_signature(
            &self.semantic_caches.signature_links,
            node,
            signature,
        )
    }

    fn set_resolved_signature_by_handle(
        &self,
        handle: core::LinkHandle<SignatureLinks>,
        signature: Option<SignatureHandle>,
    ) {
        <NodeLinkStore<SignatureLinks> as SignatureLinksStoreExt>::set_resolved_signature_by_handle(
            &self.semantic_caches.signature_links,
            handle,
            signature,
        )
    }

    fn replace_resolved_signature(
        &self,
        node: ast::Node,
        signature: Option<SignatureHandle>,
    ) -> Option<SignatureHandle> {
        <NodeLinkStore<SignatureLinks> as SignatureLinksStoreExt>::replace_resolved_signature(
            &self.semantic_caches.signature_links,
            node,
            signature,
        )
    }

    fn replace_resolved_signature_by_handle(
        &self,
        handle: core::LinkHandle<SignatureLinks>,
        signature: Option<SignatureHandle>,
    ) -> Option<SignatureHandle> {
        <NodeLinkStore<SignatureLinks> as SignatureLinksStoreExt>::replace_resolved_signature_by_handle(&self.semantic_caches.signature_links, handle, signature)
    }

    fn effects_signature(&self, node: ast::Node) -> Option<SignatureHandle> {
        <NodeLinkStore<SignatureLinks> as SignatureLinksStoreExt>::effects_signature(
            &self.semantic_caches.signature_links,
            node,
        )
    }

    fn effects_signature_by_handle(
        &self,
        handle: core::LinkHandle<SignatureLinks>,
    ) -> Option<SignatureHandle> {
        <NodeLinkStore<SignatureLinks> as SignatureLinksStoreExt>::effects_signature_by_handle(
            &self.semantic_caches.signature_links,
            handle,
        )
    }

    fn set_effects_signature(&self, node: ast::Node, signature: Option<SignatureHandle>) {
        <NodeLinkStore<SignatureLinks> as SignatureLinksStoreExt>::set_effects_signature(
            &self.semantic_caches.signature_links,
            node,
            signature,
        )
    }

    fn set_effects_signature_by_handle(
        &self,
        handle: core::LinkHandle<SignatureLinks>,
        signature: Option<SignatureHandle>,
    ) {
        <NodeLinkStore<SignatureLinks> as SignatureLinksStoreExt>::set_effects_signature_by_handle(
            &self.semantic_caches.signature_links,
            handle,
            signature,
        )
    }

    fn decorator_signature(&self, node: ast::Node) -> Option<SignatureHandle> {
        <NodeLinkStore<SignatureLinks> as SignatureLinksStoreExt>::decorator_signature(
            &self.semantic_caches.signature_links,
            node,
        )
    }

    fn decorator_signature_by_handle(
        &self,
        handle: core::LinkHandle<SignatureLinks>,
    ) -> Option<SignatureHandle> {
        <NodeLinkStore<SignatureLinks> as SignatureLinksStoreExt>::decorator_signature_by_handle(
            &self.semantic_caches.signature_links,
            handle,
        )
    }

    fn set_decorator_signature(&self, node: ast::Node, signature: Option<SignatureHandle>) {
        <NodeLinkStore<SignatureLinks> as SignatureLinksStoreExt>::set_decorator_signature(
            &self.semantic_caches.signature_links,
            node,
            signature,
        )
    }

    fn set_decorator_signature_by_handle(
        &self,
        handle: core::LinkHandle<SignatureLinks>,
        signature: Option<SignatureHandle>,
    ) {
        <NodeLinkStore<SignatureLinks> as SignatureLinksStoreExt>::set_decorator_signature_by_handle(
            &self.semantic_caches.signature_links,
            handle,
            signature,
        )
    }
}

impl JsxElementLinksStoreExt for CheckerState {
    fn jsx_element_link_handle(&self, node: ast::Node) -> core::LinkHandle<JsxElementLinks> {
        <NodeLinkStore<JsxElementLinks> as JsxElementLinksStoreExt>::jsx_element_link_handle(
            &self.semantic_caches.jsx_element_links,
            node,
        )
    }

    fn jsx_element_resolved_attributes_type(&self, node: ast::Node) -> Option<TypeHandle> {
        <NodeLinkStore<JsxElementLinks> as JsxElementLinksStoreExt>::jsx_element_resolved_attributes_type(&self.semantic_caches.jsx_element_links, node)
    }

    fn jsx_element_resolved_attributes_type_by_handle(
        &self,
        handle: core::LinkHandle<JsxElementLinks>,
    ) -> Option<TypeHandle> {
        <NodeLinkStore<JsxElementLinks> as JsxElementLinksStoreExt>::jsx_element_resolved_attributes_type_by_handle(&self.semantic_caches.jsx_element_links, handle)
    }

    fn set_jsx_element_resolved_attributes_type(&self, node: ast::Node, resolved_type: TypeHandle) {
        <NodeLinkStore<JsxElementLinks> as JsxElementLinksStoreExt>::set_jsx_element_resolved_attributes_type(&self.semantic_caches.jsx_element_links, node, resolved_type)
    }

    fn set_jsx_element_resolved_attributes_type_by_handle(
        &self,
        handle: core::LinkHandle<JsxElementLinks>,
        resolved_type: TypeHandle,
    ) {
        <NodeLinkStore<JsxElementLinks> as JsxElementLinksStoreExt>::set_jsx_element_resolved_attributes_type_by_handle(&self.semantic_caches.jsx_element_links, handle, resolved_type)
    }

    fn jsx_element_flags(&self, node: ast::Node) -> JsxFlags {
        <NodeLinkStore<JsxElementLinks> as JsxElementLinksStoreExt>::jsx_element_flags(
            &self.semantic_caches.jsx_element_links,
            node,
        )
    }

    fn jsx_element_flags_by_handle(&self, handle: core::LinkHandle<JsxElementLinks>) -> JsxFlags {
        <NodeLinkStore<JsxElementLinks> as JsxElementLinksStoreExt>::jsx_element_flags_by_handle(
            &self.semantic_caches.jsx_element_links,
            handle,
        )
    }

    fn add_jsx_element_flags(&self, node: ast::Node, flags: JsxFlags) {
        <NodeLinkStore<JsxElementLinks> as JsxElementLinksStoreExt>::add_jsx_element_flags(
            &self.semantic_caches.jsx_element_links,
            node,
            flags,
        )
    }

    fn add_jsx_element_flags_by_handle(
        &self,
        handle: core::LinkHandle<JsxElementLinks>,
        flags: JsxFlags,
    ) {
        <NodeLinkStore<JsxElementLinks> as JsxElementLinksStoreExt>::add_jsx_element_flags_by_handle(
            &self.semantic_caches.jsx_element_links,
            handle,
            flags,
        )
    }

    fn jsx_element_namespace(&self, node: ast::Node) -> Option<SymbolIdentity> {
        <NodeLinkStore<JsxElementLinks> as JsxElementLinksStoreExt>::jsx_element_namespace(
            &self.semantic_caches.jsx_element_links,
            node,
        )
    }

    fn jsx_element_namespace_by_handle(
        &self,
        handle: core::LinkHandle<JsxElementLinks>,
    ) -> Option<SymbolIdentity> {
        <NodeLinkStore<JsxElementLinks> as JsxElementLinksStoreExt>::jsx_element_namespace_by_handle(
            &self.semantic_caches.jsx_element_links,
            handle,
        )
    }

    fn set_jsx_element_namespace(&self, node: ast::Node, namespace: SymbolIdentity) {
        <NodeLinkStore<JsxElementLinks> as JsxElementLinksStoreExt>::set_jsx_element_namespace(
            &self.semantic_caches.jsx_element_links,
            node,
            namespace,
        )
    }

    fn set_jsx_element_namespace_by_handle(
        &self,
        handle: core::LinkHandle<JsxElementLinks>,
        namespace: SymbolIdentity,
    ) {
        <NodeLinkStore<JsxElementLinks> as JsxElementLinksStoreExt>::set_jsx_element_namespace_by_handle(&self.semantic_caches.jsx_element_links, handle, namespace)
    }

    fn jsx_element_implicit_import_container(&self, node: ast::Node) -> Option<SymbolIdentity> {
        <NodeLinkStore<JsxElementLinks> as JsxElementLinksStoreExt>::jsx_element_implicit_import_container(&self.semantic_caches.jsx_element_links, node)
    }

    fn jsx_element_implicit_import_container_by_handle(
        &self,
        handle: core::LinkHandle<JsxElementLinks>,
    ) -> Option<SymbolIdentity> {
        <NodeLinkStore<JsxElementLinks> as JsxElementLinksStoreExt>::jsx_element_implicit_import_container_by_handle(&self.semantic_caches.jsx_element_links, handle)
    }

    fn set_jsx_element_implicit_import_container(
        &self,
        node: ast::Node,
        container: SymbolIdentity,
    ) {
        <NodeLinkStore<JsxElementLinks> as JsxElementLinksStoreExt>::set_jsx_element_implicit_import_container(&self.semantic_caches.jsx_element_links, node, container)
    }

    fn set_jsx_element_implicit_import_container_by_handle(
        &self,
        handle: core::LinkHandle<JsxElementLinks>,
        container: SymbolIdentity,
    ) {
        <NodeLinkStore<JsxElementLinks> as JsxElementLinksStoreExt>::set_jsx_element_implicit_import_container_by_handle(&self.semantic_caches.jsx_element_links, handle, container)
    }
}

impl SwitchStatementLinksStoreExt for CheckerState {
    fn switch_statement_link_handle(
        &self,
        node: ast::Node,
    ) -> core::LinkHandle<SwitchStatementLinks> {
        <NodeLinkStore<SwitchStatementLinks> as SwitchStatementLinksStoreExt>::switch_statement_link_handle(&self.semantic_caches.switch_statement_links, node)
    }

    fn exhaustive_state(&self, node: ast::Node) -> ExhaustiveState {
        <NodeLinkStore<SwitchStatementLinks> as SwitchStatementLinksStoreExt>::exhaustive_state(
            &self.semantic_caches.switch_statement_links,
            node,
        )
    }

    fn exhaustive_state_by_handle(
        &self,
        handle: core::LinkHandle<SwitchStatementLinks>,
    ) -> ExhaustiveState {
        <NodeLinkStore<SwitchStatementLinks> as SwitchStatementLinksStoreExt>::exhaustive_state_by_handle(&self.semantic_caches.switch_statement_links, handle)
    }

    fn set_exhaustive_state(&self, node: ast::Node, state: ExhaustiveState) {
        <NodeLinkStore<SwitchStatementLinks> as SwitchStatementLinksStoreExt>::set_exhaustive_state(
            &self.semantic_caches.switch_statement_links,
            node,
            state,
        )
    }

    fn set_exhaustive_state_by_handle(
        &self,
        handle: core::LinkHandle<SwitchStatementLinks>,
        state: ExhaustiveState,
    ) {
        <NodeLinkStore<SwitchStatementLinks> as SwitchStatementLinksStoreExt>::set_exhaustive_state_by_handle(&self.semantic_caches.switch_statement_links, handle, state)
    }

    fn mark_exhaustive_computing(&self, node: ast::Node) {
        <NodeLinkStore<SwitchStatementLinks> as SwitchStatementLinksStoreExt>::mark_exhaustive_computing(&self.semantic_caches.switch_statement_links, node)
    }

    fn mark_exhaustive_computing_by_handle(&self, handle: core::LinkHandle<SwitchStatementLinks>) {
        <NodeLinkStore<SwitchStatementLinks> as SwitchStatementLinksStoreExt>::mark_exhaustive_computing_by_handle(&self.semantic_caches.switch_statement_links, handle)
    }

    fn mark_exhaustive_false(&self, node: ast::Node) {
        <NodeLinkStore<SwitchStatementLinks> as SwitchStatementLinksStoreExt>::mark_exhaustive_false(
            &self.semantic_caches.switch_statement_links,
            node,
        )
    }

    fn mark_exhaustive_false_by_handle(&self, handle: core::LinkHandle<SwitchStatementLinks>) {
        <NodeLinkStore<SwitchStatementLinks> as SwitchStatementLinksStoreExt>::mark_exhaustive_false_by_handle(&self.semantic_caches.switch_statement_links, handle)
    }

    fn mark_exhaustive_true(&self, node: ast::Node) {
        <NodeLinkStore<SwitchStatementLinks> as SwitchStatementLinksStoreExt>::mark_exhaustive_true(
            &self.semantic_caches.switch_statement_links,
            node,
        )
    }

    fn mark_exhaustive_true_by_handle(&self, handle: core::LinkHandle<SwitchStatementLinks>) {
        <NodeLinkStore<SwitchStatementLinks> as SwitchStatementLinksStoreExt>::mark_exhaustive_true_by_handle(&self.semantic_caches.switch_statement_links, handle)
    }

    fn set_exhaustive_result_if_computing(&self, node: ast::Node, is_exhaustive: bool) {
        <NodeLinkStore<SwitchStatementLinks> as SwitchStatementLinksStoreExt>::set_exhaustive_result_if_computing(&self.semantic_caches.switch_statement_links, node, is_exhaustive)
    }

    fn set_exhaustive_result_if_computing_by_handle(
        &self,
        handle: core::LinkHandle<SwitchStatementLinks>,
        is_exhaustive: bool,
    ) {
        <NodeLinkStore<SwitchStatementLinks> as SwitchStatementLinksStoreExt>::set_exhaustive_result_if_computing_by_handle(&self.semantic_caches.switch_statement_links, handle, is_exhaustive)
    }

    fn switch_types_state(&self, node: ast::Node) -> (bool, Vec<TypeHandle>) {
        <NodeLinkStore<SwitchStatementLinks> as SwitchStatementLinksStoreExt>::switch_types_state(
            &self.semantic_caches.switch_statement_links,
            node,
        )
    }

    fn switch_types_state_by_handle(
        &self,
        handle: core::LinkHandle<SwitchStatementLinks>,
    ) -> (bool, Vec<TypeHandle>) {
        <NodeLinkStore<SwitchStatementLinks> as SwitchStatementLinksStoreExt>::switch_types_state_by_handle(&self.semantic_caches.switch_statement_links, handle)
    }

    fn set_switch_types(&self, node: ast::Node, switch_types: Vec<TypeHandle>) {
        <NodeLinkStore<SwitchStatementLinks> as SwitchStatementLinksStoreExt>::set_switch_types(
            &self.semantic_caches.switch_statement_links,
            node,
            switch_types,
        )
    }

    fn set_switch_types_by_handle(
        &self,
        handle: core::LinkHandle<SwitchStatementLinks>,
        switch_types: Vec<TypeHandle>,
    ) {
        <NodeLinkStore<SwitchStatementLinks> as SwitchStatementLinksStoreExt>::set_switch_types_by_handle(&self.semantic_caches.switch_statement_links, handle, switch_types)
    }

    fn witnesses_state(&self, node: ast::Node) -> (bool, Option<Vec<String>>) {
        <NodeLinkStore<SwitchStatementLinks> as SwitchStatementLinksStoreExt>::witnesses_state(
            &self.semantic_caches.switch_statement_links,
            node,
        )
    }

    fn witnesses_state_by_handle(
        &self,
        handle: core::LinkHandle<SwitchStatementLinks>,
    ) -> (bool, Option<Vec<String>>) {
        <NodeLinkStore<SwitchStatementLinks> as SwitchStatementLinksStoreExt>::witnesses_state_by_handle(&self.semantic_caches.switch_statement_links, handle)
    }

    fn set_witnesses(&self, node: ast::Node, witnesses: Option<Vec<String>>) {
        <NodeLinkStore<SwitchStatementLinks> as SwitchStatementLinksStoreExt>::set_witnesses(
            &self.semantic_caches.switch_statement_links,
            node,
            witnesses,
        )
    }

    fn set_witnesses_by_handle(
        &self,
        handle: core::LinkHandle<SwitchStatementLinks>,
        witnesses: Option<Vec<String>>,
    ) {
        <NodeLinkStore<SwitchStatementLinks> as SwitchStatementLinksStoreExt>::set_witnesses_by_handle(&self.semantic_caches.switch_statement_links, handle, witnesses)
    }
}

impl ArrayLiteralLinksStoreExt for CheckerState {
    fn array_literal_link_handle(&self, node: ast::Node) -> core::LinkHandle<ArrayLiteralLinks> {
        <NodeLinkStore<ArrayLiteralLinks> as ArrayLiteralLinksStoreExt>::array_literal_link_handle(
            &self.array_literal_links,
            node,
        )
    }

    fn array_literal_indices_computed(&self, node: ast::Node) -> bool {
        <NodeLinkStore<ArrayLiteralLinks> as ArrayLiteralLinksStoreExt>::array_literal_indices_computed(&self.array_literal_links, node)
    }

    fn array_literal_indices_computed_by_handle(
        &self,
        handle: core::LinkHandle<ArrayLiteralLinks>,
    ) -> bool {
        <NodeLinkStore<ArrayLiteralLinks> as ArrayLiteralLinksStoreExt>::array_literal_indices_computed_by_handle(&self.array_literal_links, handle)
    }

    fn set_array_literal_indices_computed(&self, node: ast::Node, computed: bool) {
        <NodeLinkStore<ArrayLiteralLinks> as ArrayLiteralLinksStoreExt>::set_array_literal_indices_computed(&self.array_literal_links, node, computed)
    }

    fn set_array_literal_indices_computed_by_handle(
        &self,
        handle: core::LinkHandle<ArrayLiteralLinks>,
        computed: bool,
    ) {
        <NodeLinkStore<ArrayLiteralLinks> as ArrayLiteralLinksStoreExt>::set_array_literal_indices_computed_by_handle(&self.array_literal_links, handle, computed)
    }

    fn array_literal_spread_indices(&self, node: ast::Node) -> (isize, isize) {
        <NodeLinkStore<ArrayLiteralLinks> as ArrayLiteralLinksStoreExt>::array_literal_spread_indices(&self.array_literal_links, node)
    }

    fn array_literal_spread_indices_by_handle(
        &self,
        handle: core::LinkHandle<ArrayLiteralLinks>,
    ) -> (isize, isize) {
        <NodeLinkStore<ArrayLiteralLinks> as ArrayLiteralLinksStoreExt>::array_literal_spread_indices_by_handle(&self.array_literal_links, handle)
    }

    fn set_array_literal_spread_indices(
        &self,
        node: ast::Node,
        first_spread_index: isize,
        last_spread_index: isize,
    ) {
        <NodeLinkStore<ArrayLiteralLinks> as ArrayLiteralLinksStoreExt>::set_array_literal_spread_indices(&self.array_literal_links, node, first_spread_index, last_spread_index)
    }

    fn set_array_literal_spread_indices_by_handle(
        &self,
        handle: core::LinkHandle<ArrayLiteralLinks>,
        first_spread_index: isize,
        last_spread_index: isize,
    ) {
        <NodeLinkStore<ArrayLiteralLinks> as ArrayLiteralLinksStoreExt>::set_array_literal_spread_indices_by_handle(&self.array_literal_links, handle, first_spread_index, last_spread_index)
    }
}

impl EnumMemberLinksStoreExt for CheckerState {
    fn enum_member_link_handle(&self, node: ast::Node) -> core::LinkHandle<EnumMemberLinks> {
        <NodeLinkStore<EnumMemberLinks> as EnumMemberLinksStoreExt>::enum_member_link_handle(
            &self.enum_member_links,
            node,
        )
    }

    fn enum_member_value(&self, node: ast::Node) -> evaluator::Result {
        <NodeLinkStore<EnumMemberLinks> as EnumMemberLinksStoreExt>::enum_member_value(
            &self.enum_member_links,
            node,
        )
    }

    fn enum_member_value_by_handle(
        &self,
        handle: core::LinkHandle<EnumMemberLinks>,
    ) -> evaluator::Result {
        <NodeLinkStore<EnumMemberLinks> as EnumMemberLinksStoreExt>::enum_member_value_by_handle(
            &self.enum_member_links,
            handle,
        )
    }

    fn set_enum_member_value(&self, node: ast::Node, value: evaluator::Result) {
        <NodeLinkStore<EnumMemberLinks> as EnumMemberLinksStoreExt>::set_enum_member_value(
            &self.enum_member_links,
            node,
            value,
        )
    }

    fn set_enum_member_value_by_handle(
        &self,
        handle: core::LinkHandle<EnumMemberLinks>,
        value: evaluator::Result,
    ) {
        <NodeLinkStore<EnumMemberLinks> as EnumMemberLinksStoreExt>::set_enum_member_value_by_handle(
            &self.enum_member_links,
            handle,
            value,
        )
    }
}

impl DeclarationLinksStoreExt for CheckerState {
    fn declaration_link_handle(&self, node: ast::Node) -> core::LinkHandle<DeclarationLinks> {
        <NodeLinkStore<DeclarationLinks> as DeclarationLinksStoreExt>::declaration_link_handle(
            &self.declaration_links,
            node,
        )
    }

    fn declaration_is_visible(&self, node: ast::Node) -> core::Tristate {
        <NodeLinkStore<DeclarationLinks> as DeclarationLinksStoreExt>::declaration_is_visible(
            &self.declaration_links,
            node,
        )
    }

    fn declaration_is_visible_by_handle(
        &self,
        handle: core::LinkHandle<DeclarationLinks>,
    ) -> core::Tristate {
        <NodeLinkStore<DeclarationLinks> as DeclarationLinksStoreExt>::declaration_is_visible_by_handle(&self.declaration_links, handle)
    }

    fn set_declaration_is_visible(&self, node: ast::Node, is_visible: core::Tristate) {
        <NodeLinkStore<DeclarationLinks> as DeclarationLinksStoreExt>::set_declaration_is_visible(
            &self.declaration_links,
            node,
            is_visible,
        )
    }

    fn set_declaration_is_visible_by_handle(
        &self,
        handle: core::LinkHandle<DeclarationLinks>,
        is_visible: core::Tristate,
    ) {
        <NodeLinkStore<DeclarationLinks> as DeclarationLinksStoreExt>::set_declaration_is_visible_by_handle(&self.declaration_links, handle, is_visible)
    }
}

impl DeclarationFileLinksStoreExt for CheckerState {
    fn declaration_file_link_handle<Q>(
        &self,
        source_file: Q,
    ) -> core::LinkHandle<DeclarationFileLinks>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<DeclarationFileLinks> as DeclarationFileLinksStoreExt>::declaration_file_link_handle(&self.declaration_file_links, source_file)
    }

    fn declaration_file_aliases_marked<Q>(&self, source_file: Q) -> bool
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<DeclarationFileLinks> as DeclarationFileLinksStoreExt>::declaration_file_aliases_marked(&self.declaration_file_links, source_file)
    }

    fn declaration_file_aliases_marked_by_handle(
        &self,
        handle: core::LinkHandle<DeclarationFileLinks>,
    ) -> bool {
        <SourceFileLinkStore<DeclarationFileLinks> as DeclarationFileLinksStoreExt>::declaration_file_aliases_marked_by_handle(&self.declaration_file_links, handle)
    }

    fn set_declaration_file_aliases_marked<Q>(&self, source_file: Q, aliases_marked: bool)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<DeclarationFileLinks> as DeclarationFileLinksStoreExt>::set_declaration_file_aliases_marked(&self.declaration_file_links, source_file, aliases_marked)
    }

    fn set_declaration_file_aliases_marked_by_handle(
        &self,
        handle: core::LinkHandle<DeclarationFileLinks>,
        aliases_marked: bool,
    ) {
        <SourceFileLinkStore<DeclarationFileLinks> as DeclarationFileLinksStoreExt>::set_declaration_file_aliases_marked_by_handle(&self.declaration_file_links, handle, aliases_marked)
    }
}

impl SymbolReferenceLinksStoreExt for CheckerState {
    fn symbol_reference_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<SymbolReferenceLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<SymbolReferenceLinks> as SymbolReferenceLinksStoreExt>::symbol_reference_link_handle(&self.symbol_reference_links, symbol)
    }

    fn symbol_reference_kinds<Q>(&self, symbol: Q) -> ast::SymbolFlags
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<SymbolReferenceLinks> as SymbolReferenceLinksStoreExt>::symbol_reference_kinds(&self.symbol_reference_links, symbol)
    }

    fn symbol_reference_kinds_by_handle(
        &self,
        handle: core::LinkHandle<SymbolReferenceLinks>,
    ) -> ast::SymbolFlags {
        <SymbolLinkStore<SymbolReferenceLinks> as SymbolReferenceLinksStoreExt>::symbol_reference_kinds_by_handle(&self.symbol_reference_links, handle)
    }

    fn set_symbol_reference_kinds<Q>(&self, symbol: Q, kinds: ast::SymbolFlags)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<SymbolReferenceLinks> as SymbolReferenceLinksStoreExt>::set_symbol_reference_kinds(&self.symbol_reference_links, symbol, kinds)
    }

    fn set_symbol_reference_kinds_by_handle(
        &self,
        handle: core::LinkHandle<SymbolReferenceLinks>,
        kinds: ast::SymbolFlags,
    ) {
        <SymbolLinkStore<SymbolReferenceLinks> as SymbolReferenceLinksStoreExt>::set_symbol_reference_kinds_by_handle(&self.symbol_reference_links, handle, kinds)
    }

    fn add_symbol_reference_kinds<Q>(&self, symbol: Q, kinds: ast::SymbolFlags)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<SymbolReferenceLinks> as SymbolReferenceLinksStoreExt>::add_symbol_reference_kinds(&self.symbol_reference_links, symbol, kinds)
    }

    fn add_symbol_reference_kinds_by_handle(
        &self,
        handle: core::LinkHandle<SymbolReferenceLinks>,
        kinds: ast::SymbolFlags,
    ) {
        <SymbolLinkStore<SymbolReferenceLinks> as SymbolReferenceLinksStoreExt>::add_symbol_reference_kinds_by_handle(&self.symbol_reference_links, handle, kinds)
    }

    fn has_symbol_reference_kinds<Q>(&self, symbol: Q, kinds: ast::SymbolFlags) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<SymbolReferenceLinks> as SymbolReferenceLinksStoreExt>::has_symbol_reference_kinds(&self.symbol_reference_links, symbol, kinds)
    }

    fn has_symbol_reference_kinds_by_handle(
        &self,
        handle: core::LinkHandle<SymbolReferenceLinks>,
        kinds: ast::SymbolFlags,
    ) -> bool {
        <SymbolLinkStore<SymbolReferenceLinks> as SymbolReferenceLinksStoreExt>::has_symbol_reference_kinds_by_handle(&self.symbol_reference_links, handle, kinds)
    }
}

impl ModuleSymbolLinksStoreExt for CheckerState {
    fn module_symbol_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<ModuleSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ModuleSymbolLinks> as ModuleSymbolLinksStoreExt>::module_symbol_link_handle(
            &self.module_symbol_links,
            symbol,
        )
    }

    fn module_resolved_exports_is_resolved<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ModuleSymbolLinks> as ModuleSymbolLinksStoreExt>::module_resolved_exports_is_resolved(
            &self.module_symbol_links,
            symbol,
        )
    }

    fn module_resolved_exports_is_resolved_by_handle(
        &self,
        handle: core::LinkHandle<ModuleSymbolLinks>,
    ) -> bool {
        <SymbolLinkStore<ModuleSymbolLinks> as ModuleSymbolLinksStoreExt>::module_resolved_exports_is_resolved_by_handle(&self.module_symbol_links, handle)
    }

    fn with_module_resolved_exports<Q, R>(
        &self,
        symbol: Q,
        f: impl FnOnce(Option<&SymbolIdentityTable>) -> R,
    ) -> R
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ModuleSymbolLinks> as ModuleSymbolLinksStoreExt>::with_module_resolved_exports(&self.module_symbol_links, symbol, f)
    }

    fn with_module_resolved_exports_by_handle<R>(
        &self,
        handle: core::LinkHandle<ModuleSymbolLinks>,
        f: impl FnOnce(Option<&SymbolIdentityTable>) -> R,
    ) -> R {
        <SymbolLinkStore<ModuleSymbolLinks> as ModuleSymbolLinksStoreExt>::with_module_resolved_exports_by_handle(&self.module_symbol_links, handle, f)
    }

    fn set_module_resolved_exports<Q>(
        &self,
        symbol: Q,
        exports: SymbolIdentityTable,
        type_only_export_star_map: Option<HashMap<String, ast::Node>>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ModuleSymbolLinks> as ModuleSymbolLinksStoreExt>::set_module_resolved_exports(&self.module_symbol_links, symbol, exports, type_only_export_star_map)
    }

    fn set_module_resolved_exports_by_handle(
        &self,
        handle: core::LinkHandle<ModuleSymbolLinks>,
        exports: SymbolIdentityTable,
        type_only_export_star_map: Option<HashMap<String, ast::Node>>,
    ) {
        <SymbolLinkStore<ModuleSymbolLinks> as ModuleSymbolLinksStoreExt>::set_module_resolved_exports_by_handle(&self.module_symbol_links, handle, exports, type_only_export_star_map)
    }

    fn module_type_only_export_star_declaration<Q>(
        &self,
        symbol: Q,
        name: &str,
    ) -> Option<ast::Node>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ModuleSymbolLinks> as ModuleSymbolLinksStoreExt>::module_type_only_export_star_declaration(&self.module_symbol_links, symbol, name)
    }

    fn module_type_only_export_star_declaration_by_handle(
        &self,
        handle: core::LinkHandle<ModuleSymbolLinks>,
        name: &str,
    ) -> Option<ast::Node> {
        <SymbolLinkStore<ModuleSymbolLinks> as ModuleSymbolLinksStoreExt>::module_type_only_export_star_declaration_by_handle(&self.module_symbol_links, handle, name)
    }

    fn module_exports_checked<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ModuleSymbolLinks> as ModuleSymbolLinksStoreExt>::module_exports_checked(
            &self.module_symbol_links,
            symbol,
        )
    }

    fn module_exports_checked_by_handle(
        &self,
        handle: core::LinkHandle<ModuleSymbolLinks>,
    ) -> bool {
        <SymbolLinkStore<ModuleSymbolLinks> as ModuleSymbolLinksStoreExt>::module_exports_checked_by_handle(&self.module_symbol_links, handle)
    }

    fn set_module_exports_checked<Q>(&self, symbol: Q, exports_checked: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ModuleSymbolLinks> as ModuleSymbolLinksStoreExt>::set_module_exports_checked(&self.module_symbol_links, symbol, exports_checked)
    }

    fn set_module_exports_checked_by_handle(
        &self,
        handle: core::LinkHandle<ModuleSymbolLinks>,
        exports_checked: bool,
    ) {
        <SymbolLinkStore<ModuleSymbolLinks> as ModuleSymbolLinksStoreExt>::set_module_exports_checked_by_handle(&self.module_symbol_links, handle, exports_checked)
    }

    fn mark_module_exports_checked<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ModuleSymbolLinks> as ModuleSymbolLinksStoreExt>::mark_module_exports_checked(&self.module_symbol_links, symbol)
    }

    fn mark_module_exports_checked_by_handle(&self, handle: core::LinkHandle<ModuleSymbolLinks>) {
        <SymbolLinkStore<ModuleSymbolLinks> as ModuleSymbolLinksStoreExt>::mark_module_exports_checked_by_handle(&self.module_symbol_links, handle)
    }
}

impl MembersAndExportsLinksStoreExt for CheckerState {
    fn members_and_exports_link_handle<Q>(
        &self,
        symbol: Q,
    ) -> core::LinkHandle<MembersAndExportsLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<MembersAndExportsLinks> as MembersAndExportsLinksStoreExt>::members_and_exports_link_handle(&self.semantic_caches.members_and_exports_links, symbol)
    }

    fn clear_resolved_members_and_exports<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<MembersAndExportsLinks> as MembersAndExportsLinksStoreExt>::clear_resolved_members_and_exports(&self.semantic_caches.members_and_exports_links, symbol)
    }

    fn clear_resolved_members_and_exports_by_handle(
        &self,
        handle: core::LinkHandle<MembersAndExportsLinks>,
    ) {
        <SymbolLinkStore<MembersAndExportsLinks> as MembersAndExportsLinksStoreExt>::clear_resolved_members_and_exports_by_handle(&self.semantic_caches.members_and_exports_links, handle)
    }

    fn members_or_exports_slot_is_resolved<Q>(
        &self,
        symbol: Q,
        resolution_kind: MembersOrExportsResolutionKind,
    ) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<MembersAndExportsLinks> as MembersAndExportsLinksStoreExt>::members_or_exports_slot_is_resolved(&self.semantic_caches.members_and_exports_links, symbol, resolution_kind)
    }

    fn members_or_exports_slot_is_resolved_by_handle(
        &self,
        handle: core::LinkHandle<MembersAndExportsLinks>,
        resolution_kind: MembersOrExportsResolutionKind,
    ) -> bool {
        <SymbolLinkStore<MembersAndExportsLinks> as MembersAndExportsLinksStoreExt>::members_or_exports_slot_is_resolved_by_handle(&self.semantic_caches.members_and_exports_links, handle, resolution_kind)
    }

    fn with_resolved_members_or_exports<Q, R>(
        &self,
        symbol: Q,
        resolution_kind: MembersOrExportsResolutionKind,
        f: impl FnOnce(Option<&SymbolIdentityTable>) -> R,
    ) -> R
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<MembersAndExportsLinks> as MembersAndExportsLinksStoreExt>::with_resolved_members_or_exports(&self.semantic_caches.members_and_exports_links, symbol, resolution_kind, f)
    }

    fn with_resolved_members_or_exports_by_handle<R>(
        &self,
        handle: core::LinkHandle<MembersAndExportsLinks>,
        resolution_kind: MembersOrExportsResolutionKind,
        f: impl FnOnce(Option<&SymbolIdentityTable>) -> R,
    ) -> R {
        <SymbolLinkStore<MembersAndExportsLinks> as MembersAndExportsLinksStoreExt>::with_resolved_members_or_exports_by_handle(&self.semantic_caches.members_and_exports_links, handle, resolution_kind, f)
    }

    fn set_resolved_members_or_exports<Q>(
        &self,
        symbol: Q,
        resolution_kind: MembersOrExportsResolutionKind,
        table: SymbolIdentityTable,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<MembersAndExportsLinks> as MembersAndExportsLinksStoreExt>::set_resolved_members_or_exports(&self.semantic_caches.members_and_exports_links, symbol, resolution_kind, table)
    }

    fn set_resolved_members_or_exports_by_handle(
        &self,
        handle: core::LinkHandle<MembersAndExportsLinks>,
        resolution_kind: MembersOrExportsResolutionKind,
        table: SymbolIdentityTable,
    ) {
        <SymbolLinkStore<MembersAndExportsLinks> as MembersAndExportsLinksStoreExt>::set_resolved_members_or_exports_by_handle(&self.semantic_caches.members_and_exports_links, handle, resolution_kind, table)
    }
}

impl LateBoundLinksStoreExt for CheckerState {
    fn late_bound_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<LateBoundLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<LateBoundLinks> as LateBoundLinksStoreExt>::late_bound_link_handle(
            &self.late_bound_links,
            symbol,
        )
    }

    fn late_bound_symbol<Q>(&self, symbol: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<LateBoundLinks> as LateBoundLinksStoreExt>::late_bound_symbol(
            &self.late_bound_links,
            symbol,
        )
    }

    fn late_bound_symbol_by_handle(
        &self,
        handle: core::LinkHandle<LateBoundLinks>,
    ) -> Option<SymbolIdentity> {
        <SymbolLinkStore<LateBoundLinks> as LateBoundLinksStoreExt>::late_bound_symbol_by_handle(
            &self.late_bound_links,
            handle,
        )
    }

    fn set_late_bound_symbol<Q>(&self, symbol: Q, late_symbol: Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<LateBoundLinks> as LateBoundLinksStoreExt>::set_late_bound_symbol(
            &self.late_bound_links,
            symbol,
            late_symbol,
        )
    }

    fn set_late_bound_symbol_by_handle(
        &self,
        handle: core::LinkHandle<LateBoundLinks>,
        late_symbol: Option<SymbolIdentity>,
    ) {
        <SymbolLinkStore<LateBoundLinks> as LateBoundLinksStoreExt>::set_late_bound_symbol_by_handle(
            &self.late_bound_links,
            handle,
            late_symbol,
        )
    }
}

impl ExportTypeLinksStoreExt for CheckerState {
    fn export_type_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<ExportTypeLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ExportTypeLinks> as ExportTypeLinksStoreExt>::export_type_link_handle(
            &self.export_type_links,
            symbol,
        )
    }

    fn export_type_target<Q>(&self, symbol: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ExportTypeLinks> as ExportTypeLinksStoreExt>::export_type_target(
            &self.export_type_links,
            symbol,
        )
    }

    fn export_type_target_by_handle(
        &self,
        handle: core::LinkHandle<ExportTypeLinks>,
    ) -> Option<SymbolIdentity> {
        <SymbolLinkStore<ExportTypeLinks> as ExportTypeLinksStoreExt>::export_type_target_by_handle(
            &self.export_type_links,
            handle,
        )
    }

    fn set_export_type_target<Q>(&self, symbol: Q, target: Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ExportTypeLinks> as ExportTypeLinksStoreExt>::set_export_type_target(
            &self.export_type_links,
            symbol,
            target,
        )
    }

    fn set_export_type_target_by_handle(
        &self,
        handle: core::LinkHandle<ExportTypeLinks>,
        target: Option<SymbolIdentity>,
    ) {
        <SymbolLinkStore<ExportTypeLinks> as ExportTypeLinksStoreExt>::set_export_type_target_by_handle(&self.export_type_links, handle, target)
    }

    fn export_type_originating_import<Q>(&self, symbol: Q) -> Option<ast::Node>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ExportTypeLinks> as ExportTypeLinksStoreExt>::export_type_originating_import(&self.export_type_links, symbol)
    }

    fn export_type_originating_import_by_handle(
        &self,
        handle: core::LinkHandle<ExportTypeLinks>,
    ) -> Option<ast::Node> {
        <SymbolLinkStore<ExportTypeLinks> as ExportTypeLinksStoreExt>::export_type_originating_import_by_handle(&self.export_type_links, handle)
    }

    fn set_export_type_originating_import<Q>(
        &self,
        symbol: Q,
        originating_import: Option<ast::Node>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ExportTypeLinks> as ExportTypeLinksStoreExt>::set_export_type_originating_import(&self.export_type_links, symbol, originating_import)
    }

    fn set_export_type_originating_import_by_handle(
        &self,
        handle: core::LinkHandle<ExportTypeLinks>,
        originating_import: Option<ast::Node>,
    ) {
        <SymbolLinkStore<ExportTypeLinks> as ExportTypeLinksStoreExt>::set_export_type_originating_import_by_handle(&self.export_type_links, handle, originating_import)
    }
}

impl TypeAliasLinksStoreExt for CheckerState {
    fn type_alias_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<TypeAliasLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::type_alias_link_handle(
            &self.semantic_caches.type_alias_links,
            symbol,
        )
    }

    fn type_alias_declared_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::type_alias_declared_type(
            &self.semantic_caches.type_alias_links,
            symbol,
        )
    }

    fn type_alias_declared_type_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
    ) -> Option<TypeHandle> {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::type_alias_declared_type_by_handle(&self.semantic_caches.type_alias_links, handle)
    }

    fn set_type_alias_declared_type<Q>(&self, symbol: Q, declared_type: Option<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::set_type_alias_declared_type(
            &self.semantic_caches.type_alias_links,
            symbol,
            declared_type,
        )
    }

    fn set_type_alias_declared_type_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
        declared_type: Option<TypeHandle>,
    ) {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::set_type_alias_declared_type_by_handle(&self.semantic_caches.type_alias_links, handle, declared_type)
    }

    fn type_alias_type_parameters<Q>(&self, symbol: Q) -> Vec<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::type_alias_type_parameters(
            &self.semantic_caches.type_alias_links,
            symbol,
        )
    }

    fn type_alias_type_parameters_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
    ) -> Vec<TypeHandle> {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::type_alias_type_parameters_by_handle(&self.semantic_caches.type_alias_links, handle)
    }

    fn set_type_alias_type_parameters<Q>(&self, symbol: Q, type_parameters: Vec<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::set_type_alias_type_parameters(
            &self.semantic_caches.type_alias_links,
            symbol,
            type_parameters,
        )
    }

    fn set_type_alias_type_parameters_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
        type_parameters: Vec<TypeHandle>,
    ) {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::set_type_alias_type_parameters_by_handle(&self.semantic_caches.type_alias_links, handle, type_parameters)
    }

    fn type_alias_type_parameter_count<Q>(&self, symbol: Q) -> usize
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::type_alias_type_parameter_count(
            &self.semantic_caches.type_alias_links,
            symbol,
        )
    }

    fn type_alias_type_parameter_count_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
    ) -> usize {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::type_alias_type_parameter_count_by_handle(&self.semantic_caches.type_alias_links, handle)
    }

    fn type_alias_has_type_parameters<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::type_alias_has_type_parameters(
            &self.semantic_caches.type_alias_links,
            symbol,
        )
    }

    fn type_alias_has_type_parameters_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
    ) -> bool {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::type_alias_has_type_parameters_by_handle(&self.semantic_caches.type_alias_links, handle)
    }

    fn type_alias_instantiation<Q>(&self, symbol: Q, key: CacheHashKey) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::type_alias_instantiation(
            &self.semantic_caches.type_alias_links,
            symbol,
            key,
        )
    }

    fn type_alias_instantiation_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
        key: CacheHashKey,
    ) -> Option<TypeHandle> {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::type_alias_instantiation_by_handle(&self.semantic_caches.type_alias_links, handle, key)
    }

    fn set_type_alias_instantiations<Q>(
        &self,
        symbol: Q,
        instantiations: HashMap<CacheHashKey, TypeHandle>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::set_type_alias_instantiations(
            &self.semantic_caches.type_alias_links,
            symbol,
            instantiations,
        )
    }

    fn set_type_alias_instantiations_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
        instantiations: HashMap<CacheHashKey, TypeHandle>,
    ) {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::set_type_alias_instantiations_by_handle(&self.semantic_caches.type_alias_links, handle, instantiations)
    }

    fn insert_type_alias_instantiation<Q>(&self, symbol: Q, key: CacheHashKey, ty: TypeHandle)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::insert_type_alias_instantiation(
            &self.semantic_caches.type_alias_links,
            symbol,
            key,
            ty,
        )
    }

    fn insert_type_alias_instantiation_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
        key: CacheHashKey,
        ty: TypeHandle,
    ) {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::insert_type_alias_instantiation_by_handle(&self.semantic_caches.type_alias_links, handle, key, ty)
    }

    fn type_alias_is_constructor_declared_property<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::type_alias_is_constructor_declared_property(&self.semantic_caches.type_alias_links, symbol)
    }

    fn type_alias_is_constructor_declared_property_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
    ) -> bool {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::type_alias_is_constructor_declared_property_by_handle(&self.semantic_caches.type_alias_links, handle)
    }

    fn set_type_alias_is_constructor_declared_property<Q>(&self, symbol: Q, value: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::set_type_alias_is_constructor_declared_property(&self.semantic_caches.type_alias_links, symbol, value)
    }

    fn set_type_alias_is_constructor_declared_property_by_handle(
        &self,
        handle: core::LinkHandle<TypeAliasLinks>,
        value: bool,
    ) {
        <SymbolLinkStore<TypeAliasLinks> as TypeAliasLinksStoreExt>::set_type_alias_is_constructor_declared_property_by_handle(&self.semantic_caches.type_alias_links, handle, value)
    }
}

impl DeclaredTypeLinksStoreExt for CheckerState {
    fn declared_type_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<DeclaredTypeLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::declared_type_link_handle(
            &self.semantic_caches.declared_type_links,
            symbol,
        )
    }

    fn declared_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::declared_type(
            &self.semantic_caches.declared_type_links,
            symbol,
        )
    }

    fn try_declared_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::try_declared_type(
            &self.semantic_caches.declared_type_links,
            symbol,
        )
    }

    fn declared_type_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
    ) -> Option<TypeHandle> {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::declared_type_by_handle(
            &self.semantic_caches.declared_type_links,
            handle,
        )
    }

    fn set_declared_type<Q>(&self, symbol: Q, declared_type: Option<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::set_declared_type(
            &self.semantic_caches.declared_type_links,
            symbol,
            declared_type,
        )
    }

    fn set_declared_type_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
        declared_type: Option<TypeHandle>,
    ) {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::set_declared_type_by_handle(&self.semantic_caches.declared_type_links, handle, declared_type)
    }

    fn interface_checked<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::interface_checked(
            &self.semantic_caches.declared_type_links,
            symbol,
        )
    }

    fn interface_checked_by_handle(&self, handle: core::LinkHandle<DeclaredTypeLinks>) -> bool {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::interface_checked_by_handle(&self.semantic_caches.declared_type_links, handle)
    }

    fn set_interface_checked<Q>(&self, symbol: Q, checked: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::set_interface_checked(
            &self.semantic_caches.declared_type_links,
            symbol,
            checked,
        )
    }

    fn set_interface_checked_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
        checked: bool,
    ) {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::set_interface_checked_by_handle(&self.semantic_caches.declared_type_links, handle, checked)
    }

    fn mark_interface_checked<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::mark_interface_checked(
            &self.semantic_caches.declared_type_links,
            symbol,
        )
    }

    fn mark_interface_checked_by_handle(&self, handle: core::LinkHandle<DeclaredTypeLinks>) {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::mark_interface_checked_by_handle(&self.semantic_caches.declared_type_links, handle)
    }

    fn index_signatures_checked<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::index_signatures_checked(
            &self.semantic_caches.declared_type_links,
            symbol,
        )
    }

    fn index_signatures_checked_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
    ) -> bool {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::index_signatures_checked_by_handle(&self.semantic_caches.declared_type_links, handle)
    }

    fn set_index_signatures_checked<Q>(&self, symbol: Q, checked: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::set_index_signatures_checked(&self.semantic_caches.declared_type_links, symbol, checked)
    }

    fn set_index_signatures_checked_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
        checked: bool,
    ) {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::set_index_signatures_checked_by_handle(&self.semantic_caches.declared_type_links, handle, checked)
    }

    fn mark_index_signatures_checked<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::mark_index_signatures_checked(&self.semantic_caches.declared_type_links, symbol)
    }

    fn mark_index_signatures_checked_by_handle(&self, handle: core::LinkHandle<DeclaredTypeLinks>) {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::mark_index_signatures_checked_by_handle(&self.semantic_caches.declared_type_links, handle)
    }

    fn type_parameters_checked<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::type_parameters_checked(
            &self.semantic_caches.declared_type_links,
            symbol,
        )
    }

    fn type_parameters_checked_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
    ) -> bool {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::type_parameters_checked_by_handle(&self.semantic_caches.declared_type_links, handle)
    }

    fn set_type_parameters_checked<Q>(&self, symbol: Q, checked: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::set_type_parameters_checked(&self.semantic_caches.declared_type_links, symbol, checked)
    }

    fn set_type_parameters_checked_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
        checked: bool,
    ) {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::set_type_parameters_checked_by_handle(&self.semantic_caches.declared_type_links, handle, checked)
    }

    fn mark_type_parameters_checked<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::mark_type_parameters_checked(&self.semantic_caches.declared_type_links, symbol)
    }

    fn mark_type_parameters_checked_by_handle(&self, handle: core::LinkHandle<DeclaredTypeLinks>) {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::mark_type_parameters_checked_by_handle(&self.semantic_caches.declared_type_links, handle)
    }

    fn enum_checked<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::enum_checked(
            &self.semantic_caches.declared_type_links,
            symbol,
        )
    }

    fn enum_checked_by_handle(&self, handle: core::LinkHandle<DeclaredTypeLinks>) -> bool {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::enum_checked_by_handle(
            &self.semantic_caches.declared_type_links,
            handle,
        )
    }

    fn set_enum_checked<Q>(&self, symbol: Q, checked: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::set_enum_checked(
            &self.semantic_caches.declared_type_links,
            symbol,
            checked,
        )
    }

    fn set_enum_checked_by_handle(
        &self,
        handle: core::LinkHandle<DeclaredTypeLinks>,
        checked: bool,
    ) {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::set_enum_checked_by_handle(&self.semantic_caches.declared_type_links, handle, checked)
    }

    fn mark_enum_checked<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::mark_enum_checked(
            &self.semantic_caches.declared_type_links,
            symbol,
        )
    }

    fn mark_enum_checked_by_handle(&self, handle: core::LinkHandle<DeclaredTypeLinks>) {
        <SymbolLinkStore<DeclaredTypeLinks> as DeclaredTypeLinksStoreExt>::mark_enum_checked_by_handle(&self.semantic_caches.declared_type_links, handle)
    }
}

impl ValueSymbolLinksStoreExt for CheckerState {
    fn value_symbol_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<ValueSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::value_symbol_link_handle(
            &self.semantic_caches.value_symbol_links,
            symbol,
        )
    }

    fn value_symbol_instantiation_snapshot_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> ValueSymbolInstantiationSnapshot {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::value_symbol_instantiation_snapshot_by_handle(
            &self.semantic_caches.value_symbol_links,
            handle,
        )
    }

    fn set_instantiated_value_symbol_links_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        target: Option<SymbolIdentity>,
        mapper: Option<TypeMapperHandle>,
        name_type: Option<TypeHandle>,
    ) {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::set_instantiated_value_symbol_links_by_handle(
            &self.semantic_caches.value_symbol_links,
            handle,
            target,
            mapper,
            name_type,
        )
    }

    fn value_symbol_resolved_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::value_symbol_resolved_type(
            &self.semantic_caches.value_symbol_links,
            symbol,
        )
    }

    fn try_value_symbol_resolved_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::try_value_symbol_resolved_type(&self.semantic_caches.value_symbol_links, symbol)
    }

    fn value_symbol_resolved_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<TypeHandle> {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::value_symbol_resolved_type_by_handle(&self.semantic_caches.value_symbol_links, handle)
    }

    fn set_value_symbol_resolved_type<Q>(&self, symbol: Q, resolved_type: Option<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::set_value_symbol_resolved_type(&self.semantic_caches.value_symbol_links, symbol, resolved_type)
    }

    fn set_value_symbol_resolved_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        resolved_type: Option<TypeHandle>,
    ) {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::set_value_symbol_resolved_type_by_handle(&self.semantic_caches.value_symbol_links, handle, resolved_type)
    }

    fn value_symbol_write_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::value_symbol_write_type(
            &self.semantic_caches.value_symbol_links,
            symbol,
        )
    }

    fn try_value_symbol_write_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::try_value_symbol_write_type(
            &self.semantic_caches.value_symbol_links,
            symbol,
        )
    }

    fn value_symbol_write_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<TypeHandle> {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::value_symbol_write_type_by_handle(&self.semantic_caches.value_symbol_links, handle)
    }

    fn set_value_symbol_write_type<Q>(&self, symbol: Q, write_type: Option<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::set_value_symbol_write_type(
            &self.semantic_caches.value_symbol_links,
            symbol,
            write_type,
        )
    }

    fn set_value_symbol_write_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        write_type: Option<TypeHandle>,
    ) {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::set_value_symbol_write_type_by_handle(&self.semantic_caches.value_symbol_links, handle, write_type)
    }

    fn value_symbol_write_type_or_resolved_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::value_symbol_write_type_or_resolved_type(&self.semantic_caches.value_symbol_links, symbol)
    }

    fn value_symbol_write_type_or_resolved_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<TypeHandle> {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::value_symbol_write_type_or_resolved_type_by_handle(&self.semantic_caches.value_symbol_links, handle)
    }

    fn value_symbol_target<Q>(&self, symbol: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::value_symbol_target(
            &self.semantic_caches.value_symbol_links,
            symbol,
        )
    }

    fn value_symbol_target_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<SymbolIdentity> {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::value_symbol_target_by_handle(&self.semantic_caches.value_symbol_links, handle)
    }

    fn set_value_symbol_target<Q>(&self, symbol: Q, target: Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::set_value_symbol_target(
            &self.semantic_caches.value_symbol_links,
            symbol,
            target,
        )
    }

    fn set_value_symbol_target_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        target: Option<SymbolIdentity>,
    ) {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::set_value_symbol_target_by_handle(&self.semantic_caches.value_symbol_links, handle, target)
    }

    fn value_symbol_mapper<Q>(&self, symbol: Q) -> Option<TypeMapperHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::value_symbol_mapper(
            &self.semantic_caches.value_symbol_links,
            symbol,
        )
    }

    fn try_value_symbol_mapper<Q>(&self, symbol: Q) -> Option<TypeMapperHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::try_value_symbol_mapper(
            &self.semantic_caches.value_symbol_links,
            symbol,
        )
    }

    fn value_symbol_mapper_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<TypeMapperHandle> {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::value_symbol_mapper_by_handle(&self.semantic_caches.value_symbol_links, handle)
    }

    fn set_value_symbol_mapper<Q>(&self, symbol: Q, mapper: Option<TypeMapperHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::set_value_symbol_mapper(
            &self.semantic_caches.value_symbol_links,
            symbol,
            mapper,
        )
    }

    fn set_value_symbol_mapper_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        mapper: Option<TypeMapperHandle>,
    ) {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::set_value_symbol_mapper_by_handle(&self.semantic_caches.value_symbol_links, handle, mapper)
    }

    fn value_symbol_name_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::value_symbol_name_type(
            &self.semantic_caches.value_symbol_links,
            symbol,
        )
    }

    fn try_value_symbol_name_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::try_value_symbol_name_type(
            &self.semantic_caches.value_symbol_links,
            symbol,
        )
    }

    fn value_symbol_name_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<TypeHandle> {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::value_symbol_name_type_by_handle(&self.semantic_caches.value_symbol_links, handle)
    }

    fn set_value_symbol_name_type<Q>(&self, symbol: Q, name_type: Option<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::set_value_symbol_name_type(
            &self.semantic_caches.value_symbol_links,
            symbol,
            name_type,
        )
    }

    fn set_value_symbol_name_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        name_type: Option<TypeHandle>,
    ) {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::set_value_symbol_name_type_by_handle(&self.semantic_caches.value_symbol_links, handle, name_type)
    }

    fn value_symbol_containing_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::value_symbol_containing_type(&self.semantic_caches.value_symbol_links, symbol)
    }

    fn try_value_symbol_containing_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::try_value_symbol_containing_type(&self.semantic_caches.value_symbol_links, symbol)
    }

    fn value_symbol_containing_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<TypeHandle> {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::value_symbol_containing_type_by_handle(&self.semantic_caches.value_symbol_links, handle)
    }

    fn set_value_symbol_containing_type<Q>(&self, symbol: Q, containing_type: Option<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::set_value_symbol_containing_type(&self.semantic_caches.value_symbol_links, symbol, containing_type)
    }

    fn set_value_symbol_containing_type_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        containing_type: Option<TypeHandle>,
    ) {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::set_value_symbol_containing_type_by_handle(&self.semantic_caches.value_symbol_links, handle, containing_type)
    }

    fn function_or_constructor_checked<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::function_or_constructor_checked(&self.semantic_caches.value_symbol_links, symbol)
    }

    fn function_or_constructor_checked_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> bool {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::function_or_constructor_checked_by_handle(&self.semantic_caches.value_symbol_links, handle)
    }

    fn set_function_or_constructor_checked<Q>(&self, symbol: Q, checked: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::set_function_or_constructor_checked(&self.semantic_caches.value_symbol_links, symbol, checked)
    }

    fn set_function_or_constructor_checked_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        checked: bool,
    ) {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::set_function_or_constructor_checked_by_handle(&self.semantic_caches.value_symbol_links, handle, checked)
    }

    fn cjs_export_merged<Q>(&self, symbol: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::cjs_export_merged(
            &self.semantic_caches.value_symbol_links,
            symbol,
        )
    }

    fn cjs_export_merged_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
    ) -> Option<SymbolIdentity> {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::cjs_export_merged_by_handle(
            &self.semantic_caches.value_symbol_links,
            handle,
        )
    }

    fn set_cjs_export_merged<Q>(&self, symbol: Q, merged: Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::set_cjs_export_merged(
            &self.semantic_caches.value_symbol_links,
            symbol,
            merged,
        )
    }

    fn set_cjs_export_merged_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        merged: Option<SymbolIdentity>,
    ) {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::set_cjs_export_merged_by_handle(&self.semantic_caches.value_symbol_links, handle, merged)
    }

    fn get_inferred_class_symbol<Q>(
        &self,
        symbol: Q,
        target: SymbolIdentity,
    ) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::get_inferred_class_symbol(
            &self.semantic_caches.value_symbol_links,
            symbol,
            target,
        )
    }

    fn get_inferred_class_symbol_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        target: SymbolIdentity,
    ) -> Option<SymbolIdentity> {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::get_inferred_class_symbol_by_handle(&self.semantic_caches.value_symbol_links, handle, target)
    }

    fn insert_inferred_class_symbol<Q>(
        &self,
        symbol: Q,
        target: SymbolIdentity,
        inferred: SymbolIdentity,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::insert_inferred_class_symbol(&self.semantic_caches.value_symbol_links, symbol, target, inferred)
    }

    fn insert_inferred_class_symbol_by_handle(
        &self,
        handle: core::LinkHandle<ValueSymbolLinks>,
        target: SymbolIdentity,
        inferred: SymbolIdentity,
    ) {
        <SymbolLinkStore<ValueSymbolLinks> as ValueSymbolLinksStoreExt>::insert_inferred_class_symbol_by_handle(&self.semantic_caches.value_symbol_links, handle, target, inferred)
    }
}

impl MappedSymbolLinksStoreExt for CheckerState {
    fn mapped_symbol_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<MappedSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<MappedSymbolLinks> as MappedSymbolLinksStoreExt>::mapped_symbol_link_handle(
            &self.semantic_caches.mapped_symbol_links,
            symbol,
        )
    }

    fn mapped_key_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<MappedSymbolLinks> as MappedSymbolLinksStoreExt>::mapped_key_type(
            &self.semantic_caches.mapped_symbol_links,
            symbol,
        )
    }

    fn mapped_key_type_by_handle(
        &self,
        handle: core::LinkHandle<MappedSymbolLinks>,
    ) -> Option<TypeHandle> {
        <SymbolLinkStore<MappedSymbolLinks> as MappedSymbolLinksStoreExt>::mapped_key_type_by_handle(
            &self.semantic_caches.mapped_symbol_links,
            handle,
        )
    }

    fn set_mapped_key_type<Q>(&self, symbol: Q, key_type: Option<TypeHandle>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<MappedSymbolLinks> as MappedSymbolLinksStoreExt>::set_mapped_key_type(
            &self.semantic_caches.mapped_symbol_links,
            symbol,
            key_type,
        )
    }

    fn set_mapped_key_type_by_handle(
        &self,
        handle: core::LinkHandle<MappedSymbolLinks>,
        key_type: Option<TypeHandle>,
    ) {
        <SymbolLinkStore<MappedSymbolLinks> as MappedSymbolLinksStoreExt>::set_mapped_key_type_by_handle(&self.semantic_caches.mapped_symbol_links, handle, key_type)
    }

    fn mapped_synthetic_origin<Q>(&self, symbol: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<MappedSymbolLinks> as MappedSymbolLinksStoreExt>::mapped_synthetic_origin(
            &self.semantic_caches.mapped_symbol_links,
            symbol,
        )
    }

    fn mapped_synthetic_origin_by_handle(
        &self,
        handle: core::LinkHandle<MappedSymbolLinks>,
    ) -> Option<SymbolIdentity> {
        <SymbolLinkStore<MappedSymbolLinks> as MappedSymbolLinksStoreExt>::mapped_synthetic_origin_by_handle(&self.semantic_caches.mapped_symbol_links, handle)
    }

    fn set_mapped_synthetic_origin<Q>(&self, symbol: Q, origin: Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<MappedSymbolLinks> as MappedSymbolLinksStoreExt>::set_mapped_synthetic_origin(&self.semantic_caches.mapped_symbol_links, symbol, origin)
    }

    fn set_mapped_synthetic_origin_by_handle(
        &self,
        handle: core::LinkHandle<MappedSymbolLinks>,
        origin: Option<SymbolIdentity>,
    ) {
        <SymbolLinkStore<MappedSymbolLinks> as MappedSymbolLinksStoreExt>::set_mapped_synthetic_origin_by_handle(&self.semantic_caches.mapped_symbol_links, handle, origin)
    }
}

impl ReverseMappedSymbolLinksStoreExt for CheckerState {
    fn reverse_mapped_symbol_link_handle<Q>(
        &self,
        symbol: Q,
    ) -> core::LinkHandle<ReverseMappedSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ReverseMappedSymbolLinks> as ReverseMappedSymbolLinksStoreExt>::reverse_mapped_symbol_link_handle(&self.semantic_caches.reverse_mapped_symbol_links, symbol)
    }

    fn try_reverse_mapped_symbol_link_handle<Q>(
        &self,
        symbol: Q,
    ) -> Option<core::LinkHandle<ReverseMappedSymbolLinks>>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ReverseMappedSymbolLinks> as ReverseMappedSymbolLinksStoreExt>::try_reverse_mapped_symbol_link_handle(&self.semantic_caches.reverse_mapped_symbol_links, symbol)
    }

    fn has_reverse_mapped_symbol_links<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ReverseMappedSymbolLinks> as ReverseMappedSymbolLinksStoreExt>::has_reverse_mapped_symbol_links(&self.semantic_caches.reverse_mapped_symbol_links, symbol)
    }

    fn try_reverse_mapped_symbol_link_types<Q>(
        &self,
        symbol: Q,
    ) -> Option<(Option<TypeHandle>, Option<TypeHandle>, Option<TypeHandle>)>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ReverseMappedSymbolLinks> as ReverseMappedSymbolLinksStoreExt>::try_reverse_mapped_symbol_link_types(&self.semantic_caches.reverse_mapped_symbol_links, symbol)
    }

    fn reverse_mapped_symbol_link_types_by_handle(
        &self,
        handle: core::LinkHandle<ReverseMappedSymbolLinks>,
    ) -> (Option<TypeHandle>, Option<TypeHandle>, Option<TypeHandle>) {
        <SymbolLinkStore<ReverseMappedSymbolLinks> as ReverseMappedSymbolLinksStoreExt>::reverse_mapped_symbol_link_types_by_handle(&self.semantic_caches.reverse_mapped_symbol_links, handle)
    }

    fn reverse_mapped_resolved_type_by_handle(
        &self,
        handle: core::LinkHandle<ReverseMappedSymbolLinks>,
    ) -> Option<TypeHandle> {
        <SymbolLinkStore<ReverseMappedSymbolLinks> as ReverseMappedSymbolLinksStoreExt>::reverse_mapped_resolved_type_by_handle(&self.semantic_caches.reverse_mapped_symbol_links, handle)
    }

    fn set_reverse_mapped_resolved_type_by_handle(
        &self,
        handle: core::LinkHandle<ReverseMappedSymbolLinks>,
        resolved_type: Option<TypeHandle>,
    ) {
        <SymbolLinkStore<ReverseMappedSymbolLinks> as ReverseMappedSymbolLinksStoreExt>::set_reverse_mapped_resolved_type_by_handle(&self.semantic_caches.reverse_mapped_symbol_links, handle, resolved_type)
    }

    fn reverse_mapped_property_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ReverseMappedSymbolLinks> as ReverseMappedSymbolLinksStoreExt>::reverse_mapped_property_type(&self.semantic_caches.reverse_mapped_symbol_links, symbol)
    }

    fn reverse_mapped_property_type_by_handle(
        &self,
        handle: core::LinkHandle<ReverseMappedSymbolLinks>,
    ) -> Option<TypeHandle> {
        <SymbolLinkStore<ReverseMappedSymbolLinks> as ReverseMappedSymbolLinksStoreExt>::reverse_mapped_property_type_by_handle(&self.semantic_caches.reverse_mapped_symbol_links, handle)
    }

    fn reverse_mapped_mapped_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ReverseMappedSymbolLinks> as ReverseMappedSymbolLinksStoreExt>::reverse_mapped_mapped_type(&self.semantic_caches.reverse_mapped_symbol_links, symbol)
    }

    fn reverse_mapped_mapped_type_by_handle(
        &self,
        handle: core::LinkHandle<ReverseMappedSymbolLinks>,
    ) -> Option<TypeHandle> {
        <SymbolLinkStore<ReverseMappedSymbolLinks> as ReverseMappedSymbolLinksStoreExt>::reverse_mapped_mapped_type_by_handle(&self.semantic_caches.reverse_mapped_symbol_links, handle)
    }

    fn reverse_mapped_constraint_type<Q>(&self, symbol: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ReverseMappedSymbolLinks> as ReverseMappedSymbolLinksStoreExt>::reverse_mapped_constraint_type(&self.semantic_caches.reverse_mapped_symbol_links, symbol)
    }

    fn reverse_mapped_constraint_type_by_handle(
        &self,
        handle: core::LinkHandle<ReverseMappedSymbolLinks>,
    ) -> Option<TypeHandle> {
        <SymbolLinkStore<ReverseMappedSymbolLinks> as ReverseMappedSymbolLinksStoreExt>::reverse_mapped_constraint_type_by_handle(&self.semantic_caches.reverse_mapped_symbol_links, handle)
    }

    fn set_reverse_mapped_symbol_link_types<Q>(
        &self,
        symbol: Q,
        property_type: Option<TypeHandle>,
        mapped_type: Option<TypeHandle>,
        constraint_type: Option<TypeHandle>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ReverseMappedSymbolLinks> as ReverseMappedSymbolLinksStoreExt>::set_reverse_mapped_symbol_link_types(&self.semantic_caches.reverse_mapped_symbol_links, symbol, property_type, mapped_type, constraint_type)
    }

    fn set_reverse_mapped_symbol_link_types_by_handle(
        &self,
        handle: core::LinkHandle<ReverseMappedSymbolLinks>,
        property_type: Option<TypeHandle>,
        mapped_type: Option<TypeHandle>,
        constraint_type: Option<TypeHandle>,
    ) {
        <SymbolLinkStore<ReverseMappedSymbolLinks> as ReverseMappedSymbolLinksStoreExt>::set_reverse_mapped_symbol_link_types_by_handle(&self.semantic_caches.reverse_mapped_symbol_links, handle, property_type, mapped_type, constraint_type)
    }
}

impl MarkedAssignmentSymbolLinksStoreExt for CheckerState {
    fn marked_assignment_symbol_link_handle<Q>(
        &self,
        symbol: Q,
    ) -> core::LinkHandle<MarkedAssignmentSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<MarkedAssignmentSymbolLinks> as MarkedAssignmentSymbolLinksStoreExt>::marked_assignment_symbol_link_handle(&self.marked_assignment_symbol_links, symbol)
    }

    fn marked_assignment_last_assignment_pos<Q>(&self, symbol: Q) -> i32
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<MarkedAssignmentSymbolLinks> as MarkedAssignmentSymbolLinksStoreExt>::marked_assignment_last_assignment_pos(&self.marked_assignment_symbol_links, symbol)
    }

    fn marked_assignment_last_assignment_pos_by_handle(
        &self,
        handle: core::LinkHandle<MarkedAssignmentSymbolLinks>,
    ) -> i32 {
        <SymbolLinkStore<MarkedAssignmentSymbolLinks> as MarkedAssignmentSymbolLinksStoreExt>::marked_assignment_last_assignment_pos_by_handle(&self.marked_assignment_symbol_links, handle)
    }

    fn set_marked_assignment_last_assignment_pos<Q>(&self, symbol: Q, last_assignment_pos: i32)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<MarkedAssignmentSymbolLinks> as MarkedAssignmentSymbolLinksStoreExt>::set_marked_assignment_last_assignment_pos(&self.marked_assignment_symbol_links, symbol, last_assignment_pos)
    }

    fn set_marked_assignment_last_assignment_pos_by_handle(
        &self,
        handle: core::LinkHandle<MarkedAssignmentSymbolLinks>,
        last_assignment_pos: i32,
    ) {
        <SymbolLinkStore<MarkedAssignmentSymbolLinks> as MarkedAssignmentSymbolLinksStoreExt>::set_marked_assignment_last_assignment_pos_by_handle(&self.marked_assignment_symbol_links, handle, last_assignment_pos)
    }

    fn marked_assignment_has_definite_assignment<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<MarkedAssignmentSymbolLinks> as MarkedAssignmentSymbolLinksStoreExt>::marked_assignment_has_definite_assignment(&self.marked_assignment_symbol_links, symbol)
    }

    fn marked_assignment_has_definite_assignment_by_handle(
        &self,
        handle: core::LinkHandle<MarkedAssignmentSymbolLinks>,
    ) -> bool {
        <SymbolLinkStore<MarkedAssignmentSymbolLinks> as MarkedAssignmentSymbolLinksStoreExt>::marked_assignment_has_definite_assignment_by_handle(&self.marked_assignment_symbol_links, handle)
    }

    fn set_marked_assignment_has_definite_assignment<Q>(
        &self,
        symbol: Q,
        has_definite_assignment: bool,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<MarkedAssignmentSymbolLinks> as MarkedAssignmentSymbolLinksStoreExt>::set_marked_assignment_has_definite_assignment(&self.marked_assignment_symbol_links, symbol, has_definite_assignment)
    }

    fn set_marked_assignment_has_definite_assignment_by_handle(
        &self,
        handle: core::LinkHandle<MarkedAssignmentSymbolLinks>,
        has_definite_assignment: bool,
    ) {
        <SymbolLinkStore<MarkedAssignmentSymbolLinks> as MarkedAssignmentSymbolLinksStoreExt>::set_marked_assignment_has_definite_assignment_by_handle(&self.marked_assignment_symbol_links, handle, has_definite_assignment)
    }

    fn mark_marked_assignment_has_definite_assignment<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<MarkedAssignmentSymbolLinks> as MarkedAssignmentSymbolLinksStoreExt>::mark_marked_assignment_has_definite_assignment(&self.marked_assignment_symbol_links, symbol)
    }

    fn mark_marked_assignment_has_definite_assignment_by_handle(
        &self,
        handle: core::LinkHandle<MarkedAssignmentSymbolLinks>,
    ) {
        <SymbolLinkStore<MarkedAssignmentSymbolLinks> as MarkedAssignmentSymbolLinksStoreExt>::mark_marked_assignment_has_definite_assignment_by_handle(&self.marked_assignment_symbol_links, handle)
    }
}

impl ContainingSymbolLinksStoreExt for CheckerState {
    fn containing_symbol_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<ContainingSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ContainingSymbolLinks> as ContainingSymbolLinksStoreExt>::containing_symbol_link_handle(&self.symbol_container_links, symbol)
    }

    fn alternative_containing_modules_for_file<Q>(
        &self,
        symbol: Q,
        file_id: ast::NodeId,
    ) -> Option<Vec<SymbolIdentity>>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ContainingSymbolLinks> as ContainingSymbolLinksStoreExt>::alternative_containing_modules_for_file(&self.symbol_container_links, symbol, file_id)
    }

    fn record_alternative_containing_modules_for_file<Q>(
        &self,
        symbol: Q,
        file_id: ast::NodeId,
        modules: Vec<SymbolIdentity>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ContainingSymbolLinks> as ContainingSymbolLinksStoreExt>::record_alternative_containing_modules_for_file(&self.symbol_container_links, symbol, file_id, modules)
    }

    fn extended_containers<Q>(&self, symbol: Q) -> Option<Vec<SymbolIdentity>>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ContainingSymbolLinks> as ContainingSymbolLinksStoreExt>::extended_containers(&self.symbol_container_links, symbol)
    }

    fn set_extended_containers<Q>(&self, symbol: Q, containers: Vec<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ContainingSymbolLinks> as ContainingSymbolLinksStoreExt>::set_extended_containers(&self.symbol_container_links, symbol, containers)
    }

    fn accessible_chain_cache_entry<Q>(
        &self,
        symbol: Q,
        key: &AccessibleChainCacheKey,
    ) -> Option<Vec<SymbolIdentity>>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ContainingSymbolLinks> as ContainingSymbolLinksStoreExt>::accessible_chain_cache_entry(&self.symbol_container_links, symbol, key)
    }

    fn record_accessible_chain_cache_entry<Q>(
        &self,
        symbol: Q,
        key: AccessibleChainCacheKey,
        chain: Vec<SymbolIdentity>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<ContainingSymbolLinks> as ContainingSymbolLinksStoreExt>::record_accessible_chain_cache_entry(&self.symbol_container_links, symbol, key, chain)
    }
}

impl DeferredSymbolLinksStoreExt for CheckerState {
    fn deferred_symbol_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<DeferredSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<DeferredSymbolLinks> as DeferredSymbolLinksStoreExt>::deferred_symbol_link_handle(&self.semantic_caches.deferred_symbol_links, symbol)
    }

    fn deferred_symbol_links_by_handle(
        &self,
        handle: core::LinkHandle<DeferredSymbolLinks>,
    ) -> (Option<TypeHandle>, Vec<TypeHandle>, Vec<TypeHandle>) {
        <SymbolLinkStore<DeferredSymbolLinks> as DeferredSymbolLinksStoreExt>::deferred_symbol_links_by_handle(&self.semantic_caches.deferred_symbol_links, handle)
    }

    fn set_deferred_symbol_links<Q>(
        &self,
        symbol: Q,
        parent: Option<TypeHandle>,
        constituents: Vec<TypeHandle>,
        write_constituents: Vec<TypeHandle>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<DeferredSymbolLinks> as DeferredSymbolLinksStoreExt>::set_deferred_symbol_links(&self.semantic_caches.deferred_symbol_links, symbol, parent, constituents, write_constituents)
    }

    fn set_deferred_symbol_links_by_handle(
        &self,
        handle: core::LinkHandle<DeferredSymbolLinks>,
        parent: Option<TypeHandle>,
        constituents: Vec<TypeHandle>,
        write_constituents: Vec<TypeHandle>,
    ) {
        <SymbolLinkStore<DeferredSymbolLinks> as DeferredSymbolLinksStoreExt>::set_deferred_symbol_links_by_handle(&self.semantic_caches.deferred_symbol_links, handle, parent, constituents, write_constituents)
    }
}

impl SpreadLinksStoreExt for CheckerState {
    fn spread_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<SpreadLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<SpreadLinks> as SpreadLinksStoreExt>::spread_link_handle(
            &self.spread_links,
            symbol,
        )
    }

    fn spread_symbols<Q>(&self, symbol: Q) -> (Option<SymbolIdentity>, Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<SpreadLinks> as SpreadLinksStoreExt>::spread_symbols(
            &self.spread_links,
            symbol,
        )
    }

    fn spread_symbols_by_handle(
        &self,
        handle: core::LinkHandle<SpreadLinks>,
    ) -> (Option<SymbolIdentity>, Option<SymbolIdentity>) {
        <SymbolLinkStore<SpreadLinks> as SpreadLinksStoreExt>::spread_symbols_by_handle(
            &self.spread_links,
            handle,
        )
    }

    fn set_spread_symbols<Q>(
        &self,
        symbol: Q,
        left: Option<SymbolIdentity>,
        right: Option<SymbolIdentity>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<SpreadLinks> as SpreadLinksStoreExt>::set_spread_symbols(
            &self.spread_links,
            symbol,
            left,
            right,
        )
    }

    fn set_spread_symbols_by_handle(
        &self,
        handle: core::LinkHandle<SpreadLinks>,
        left: Option<SymbolIdentity>,
        right: Option<SymbolIdentity>,
    ) {
        <SymbolLinkStore<SpreadLinks> as SpreadLinksStoreExt>::set_spread_symbols_by_handle(
            &self.spread_links,
            handle,
            left,
            right,
        )
    }
}

impl VarianceLinksStoreExt for CheckerState {
    fn variance_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<VarianceLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<VarianceLinks> as VarianceLinksStoreExt>::variance_link_handle(
            &self.variance_links,
            symbol,
        )
    }

    fn variance_cache_state<Q>(&self, symbol: Q) -> VarianceCacheState
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<VarianceLinks> as VarianceLinksStoreExt>::variance_cache_state(
            &self.variance_links,
            symbol,
        )
    }

    fn variance_cache_state_by_handle(
        &self,
        handle: core::LinkHandle<VarianceLinks>,
    ) -> VarianceCacheState {
        <SymbolLinkStore<VarianceLinks> as VarianceLinksStoreExt>::variance_cache_state_by_handle(
            &self.variance_links,
            handle,
        )
    }

    fn mark_variances_computing<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<VarianceLinks> as VarianceLinksStoreExt>::mark_variances_computing(
            &self.variance_links,
            symbol,
        )
    }

    fn mark_variances_computing_by_handle(&self, handle: core::LinkHandle<VarianceLinks>) {
        <SymbolLinkStore<VarianceLinks> as VarianceLinksStoreExt>::mark_variances_computing_by_handle(&self.variance_links, handle)
    }

    fn set_variances_computed<Q>(&self, symbol: Q, variances: Vec<VarianceFlags>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<VarianceLinks> as VarianceLinksStoreExt>::set_variances_computed(
            &self.variance_links,
            symbol,
            variances,
        )
    }

    fn set_variances_computed_by_handle(
        &self,
        handle: core::LinkHandle<VarianceLinks>,
        variances: Vec<VarianceFlags>,
    ) {
        <SymbolLinkStore<VarianceLinks> as VarianceLinksStoreExt>::set_variances_computed_by_handle(
            &self.variance_links,
            handle,
            variances,
        )
    }
}

impl SourceFileLinksStoreExt for CheckerState {
    fn source_file_link_handle<Q>(&self, source_file: Q) -> core::LinkHandle<SourceFileLinks>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::source_file_link_handle(
            &self.semantic_caches.source_file_links,
            source_file,
        )
    }

    fn source_file_type_checked<Q>(&self, source_file: Q) -> bool
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::source_file_type_checked(
            &self.semantic_caches.source_file_links,
            source_file,
        )
    }

    fn source_file_type_checked_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> bool {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::source_file_type_checked_by_handle(&self.semantic_caches.source_file_links, handle)
    }

    fn set_source_file_type_checked<Q>(&self, source_file: Q, checked: bool)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::set_source_file_type_checked(&self.semantic_caches.source_file_links, source_file, checked)
    }

    fn set_source_file_type_checked_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        checked: bool,
    ) {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::set_source_file_type_checked_by_handle(&self.semantic_caches.source_file_links, handle, checked)
    }

    fn source_file_unused_checked<Q>(&self, source_file: Q) -> bool
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::source_file_unused_checked(&self.semantic_caches.source_file_links, source_file)
    }

    fn source_file_unused_checked_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> bool {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::source_file_unused_checked_by_handle(&self.semantic_caches.source_file_links, handle)
    }

    fn set_source_file_unused_checked<Q>(&self, source_file: Q, checked: bool)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::set_source_file_unused_checked(&self.semantic_caches.source_file_links, source_file, checked)
    }

    fn set_source_file_unused_checked_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        checked: bool,
    ) {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::set_source_file_unused_checked_by_handle(&self.semantic_caches.source_file_links, handle, checked)
    }

    fn source_file_external_helpers_module<Q>(&self, source_file: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::source_file_external_helpers_module(&self.semantic_caches.source_file_links, source_file)
    }

    fn source_file_external_helpers_module_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> Option<SymbolIdentity> {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::source_file_external_helpers_module_by_handle(&self.semantic_caches.source_file_links, handle)
    }

    fn set_source_file_external_helpers_module<Q>(
        &self,
        source_file: Q,
        symbol: Option<SymbolIdentity>,
    ) where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::set_source_file_external_helpers_module(&self.semantic_caches.source_file_links, source_file, symbol)
    }

    fn set_source_file_external_helpers_module_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        symbol: Option<SymbolIdentity>,
    ) {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::set_source_file_external_helpers_module_by_handle(&self.semantic_caches.source_file_links, handle, symbol)
    }

    fn source_file_requested_external_emit_helpers<Q>(&self, source_file: Q) -> ExternalEmitHelpers
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::source_file_requested_external_emit_helpers(&self.semantic_caches.source_file_links, source_file)
    }

    fn source_file_requested_external_emit_helpers_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> ExternalEmitHelpers {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::source_file_requested_external_emit_helpers_by_handle(&self.semantic_caches.source_file_links, handle)
    }

    fn set_source_file_requested_external_emit_helpers<Q>(
        &self,
        source_file: Q,
        helpers: ExternalEmitHelpers,
    ) where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::set_source_file_requested_external_emit_helpers(&self.semantic_caches.source_file_links, source_file, helpers)
    }

    fn set_source_file_requested_external_emit_helpers_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        helpers: ExternalEmitHelpers,
    ) {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::set_source_file_requested_external_emit_helpers_by_handle(&self.semantic_caches.source_file_links, handle, helpers)
    }

    fn add_source_file_requested_external_emit_helpers<Q>(
        &self,
        source_file: Q,
        helpers: ExternalEmitHelpers,
    ) where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::add_source_file_requested_external_emit_helpers(&self.semantic_caches.source_file_links, source_file, helpers)
    }

    fn add_source_file_requested_external_emit_helpers_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        helpers: ExternalEmitHelpers,
    ) {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::add_source_file_requested_external_emit_helpers_by_handle(&self.semantic_caches.source_file_links, handle, helpers)
    }

    fn source_file_has_requested_external_emit_helpers<Q>(
        &self,
        source_file: Q,
        helpers: ExternalEmitHelpers,
    ) -> bool
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::source_file_has_requested_external_emit_helpers(&self.semantic_caches.source_file_links, source_file, helpers)
    }

    fn source_file_has_requested_external_emit_helpers_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        helpers: ExternalEmitHelpers,
    ) -> bool {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::source_file_has_requested_external_emit_helpers_by_handle(&self.semantic_caches.source_file_links, handle, helpers)
    }

    fn source_file_local_jsx_namespace<Q>(&self, source_file: Q) -> String
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::source_file_local_jsx_namespace(&self.semantic_caches.source_file_links, source_file)
    }

    fn source_file_local_jsx_namespace_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> String {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::source_file_local_jsx_namespace_by_handle(&self.semantic_caches.source_file_links, handle)
    }

    fn set_source_file_local_jsx_namespace<Q>(&self, source_file: Q, namespace: String)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::set_source_file_local_jsx_namespace(&self.semantic_caches.source_file_links, source_file, namespace)
    }

    fn set_source_file_local_jsx_namespace_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        namespace: String,
    ) {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::set_source_file_local_jsx_namespace_by_handle(&self.semantic_caches.source_file_links, handle, namespace)
    }

    fn source_file_local_jsx_fragment_namespace<Q>(&self, source_file: Q) -> String
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::source_file_local_jsx_fragment_namespace(&self.semantic_caches.source_file_links, source_file)
    }

    fn source_file_local_jsx_fragment_namespace_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> String {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::source_file_local_jsx_fragment_namespace_by_handle(&self.semantic_caches.source_file_links, handle)
    }

    fn set_source_file_local_jsx_fragment_namespace<Q>(&self, source_file: Q, namespace: String)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::set_source_file_local_jsx_fragment_namespace(&self.semantic_caches.source_file_links, source_file, namespace)
    }

    fn set_source_file_local_jsx_fragment_namespace_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        namespace: String,
    ) {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::set_source_file_local_jsx_fragment_namespace_by_handle(&self.semantic_caches.source_file_links, handle, namespace)
    }

    fn source_file_local_jsx_factory<Q>(&self, source_file: Q) -> Option<ast::EntityName>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::source_file_local_jsx_factory(&self.semantic_caches.source_file_links, source_file)
    }

    fn source_file_local_jsx_factory_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> Option<ast::EntityName> {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::source_file_local_jsx_factory_by_handle(&self.semantic_caches.source_file_links, handle)
    }

    fn set_source_file_local_jsx_factory<Q>(&self, source_file: Q, factory: Option<ast::EntityName>)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::set_source_file_local_jsx_factory(&self.semantic_caches.source_file_links, source_file, factory)
    }

    fn set_source_file_local_jsx_factory_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        factory: Option<ast::EntityName>,
    ) {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::set_source_file_local_jsx_factory_by_handle(&self.semantic_caches.source_file_links, handle, factory)
    }

    fn source_file_local_jsx_fragment_factory<Q>(&self, source_file: Q) -> Option<ast::EntityName>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::source_file_local_jsx_fragment_factory(&self.semantic_caches.source_file_links, source_file)
    }

    fn source_file_local_jsx_fragment_factory_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> Option<ast::EntityName> {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::source_file_local_jsx_fragment_factory_by_handle(&self.semantic_caches.source_file_links, handle)
    }

    fn set_source_file_local_jsx_fragment_factory<Q>(
        &self,
        source_file: Q,
        factory: Option<ast::EntityName>,
    ) where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::set_source_file_local_jsx_fragment_factory(&self.semantic_caches.source_file_links, source_file, factory)
    }

    fn set_source_file_local_jsx_fragment_factory_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        factory: Option<ast::EntityName>,
    ) {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::set_source_file_local_jsx_fragment_factory_by_handle(&self.semantic_caches.source_file_links, handle, factory)
    }

    fn source_file_jsx_fragment_type<Q>(&self, source_file: Q) -> Option<TypeHandle>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::source_file_jsx_fragment_type(&self.semantic_caches.source_file_links, source_file)
    }

    fn source_file_jsx_fragment_type_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> Option<TypeHandle> {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::source_file_jsx_fragment_type_by_handle(&self.semantic_caches.source_file_links, handle)
    }

    fn set_source_file_jsx_fragment_type<Q>(&self, source_file: Q, jsx_fragment_type: TypeHandle)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::set_source_file_jsx_fragment_type(&self.semantic_caches.source_file_links, source_file, jsx_fragment_type)
    }

    fn set_source_file_jsx_fragment_type_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        jsx_fragment_type: TypeHandle,
    ) {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::set_source_file_jsx_fragment_type_by_handle(&self.semantic_caches.source_file_links, handle, jsx_fragment_type)
    }

    fn add_deferred_node<Q>(&self, source_file: Q, node: ast::Node)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::add_deferred_node(
            &self.semantic_caches.source_file_links,
            source_file,
            node,
        )
    }

    fn add_deferred_node_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        node: ast::Node,
    ) {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::add_deferred_node_by_handle(&self.semantic_caches.source_file_links, handle, node)
    }

    fn next_deferred_node<Q>(&self, source_file: Q, index: usize) -> Option<ast::Node>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::next_deferred_node(
            &self.semantic_caches.source_file_links,
            source_file,
            index,
        )
    }

    fn next_deferred_node_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        index: usize,
    ) -> Option<ast::Node> {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::next_deferred_node_by_handle(&self.semantic_caches.source_file_links, handle, index)
    }

    fn clear_deferred_nodes<Q>(&self, source_file: Q)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::clear_deferred_nodes(
            &self.semantic_caches.source_file_links,
            source_file,
        )
    }

    fn clear_deferred_nodes_by_handle(&self, handle: core::LinkHandle<SourceFileLinks>) {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::clear_deferred_nodes_by_handle(&self.semantic_caches.source_file_links, handle)
    }

    fn push_identifier_check_node<Q>(&self, source_file: Q, node: ast::Node)
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::push_identifier_check_node(&self.semantic_caches.source_file_links, source_file, node)
    }

    fn push_identifier_check_node_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
        node: ast::Node,
    ) {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::push_identifier_check_node_by_handle(&self.semantic_caches.source_file_links, handle, node)
    }

    fn identifier_check_nodes<Q>(&self, source_file: Q) -> Vec<ast::Node>
    where
        Q: core::IntoLinkKey<SourceFileIdentity>,
    {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::identifier_check_nodes(
            &self.semantic_caches.source_file_links,
            source_file,
        )
    }

    fn identifier_check_nodes_by_handle(
        &self,
        handle: core::LinkHandle<SourceFileLinks>,
    ) -> Vec<ast::Node> {
        <SourceFileLinkStore<SourceFileLinks> as SourceFileLinksStoreExt>::identifier_check_nodes_by_handle(&self.semantic_caches.source_file_links, handle)
    }
}

impl AliasSymbolLinksStoreExt for CheckerState {
    fn alias_symbol_link_handle<Q>(&self, symbol: Q) -> core::LinkHandle<AliasSymbolLinks>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::alias_symbol_link_handle(
            &self.alias_symbol_links,
            symbol,
        )
    }

    fn alias_symbol_target<Q>(&self, symbol: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::alias_symbol_target(
            &self.alias_symbol_links,
            symbol,
        )
    }

    fn alias_symbol_target_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
    ) -> Option<SymbolIdentity> {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::alias_symbol_target_by_handle(&self.alias_symbol_links, handle)
    }

    fn set_alias_symbol_target<Q>(&self, symbol: Q, target: Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::set_alias_symbol_target(
            &self.alias_symbol_links,
            symbol,
            target,
        )
    }

    fn set_alias_symbol_target_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
        target: Option<SymbolIdentity>,
    ) {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::set_alias_symbol_target_by_handle(&self.alias_symbol_links, handle, target)
    }

    fn alias_symbol_immediate_target<Q>(&self, symbol: Q) -> Option<SymbolIdentity>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::alias_symbol_immediate_target(&self.alias_symbol_links, symbol)
    }

    fn alias_symbol_immediate_target_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
    ) -> Option<SymbolIdentity> {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::alias_symbol_immediate_target_by_handle(&self.alias_symbol_links, handle)
    }

    fn set_alias_symbol_immediate_target<Q>(&self, symbol: Q, target: Option<SymbolIdentity>)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::set_alias_symbol_immediate_target(&self.alias_symbol_links, symbol, target)
    }

    fn set_alias_symbol_immediate_target_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
        target: Option<SymbolIdentity>,
    ) {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::set_alias_symbol_immediate_target_by_handle(&self.alias_symbol_links, handle, target)
    }

    fn alias_symbol_referenced<Q>(&self, symbol: Q) -> bool
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::alias_symbol_referenced(
            &self.alias_symbol_links,
            symbol,
        )
    }

    fn alias_symbol_referenced_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
    ) -> bool {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::alias_symbol_referenced_by_handle(&self.alias_symbol_links, handle)
    }

    fn set_alias_symbol_referenced<Q>(&self, symbol: Q, referenced: bool)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::set_alias_symbol_referenced(
            &self.alias_symbol_links,
            symbol,
            referenced,
        )
    }

    fn set_alias_symbol_referenced_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
        referenced: bool,
    ) {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::set_alias_symbol_referenced_by_handle(&self.alias_symbol_links, handle, referenced)
    }

    fn mark_alias_symbol_referenced<Q>(&self, symbol: Q)
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::mark_alias_symbol_referenced(&self.alias_symbol_links, symbol)
    }

    fn mark_alias_symbol_referenced_by_handle(&self, handle: core::LinkHandle<AliasSymbolLinks>) {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::mark_alias_symbol_referenced_by_handle(&self.alias_symbol_links, handle)
    }

    fn alias_symbol_type_only_declaration<Q>(&self, symbol: Q) -> Option<ast::Node>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::alias_symbol_type_only_declaration(&self.alias_symbol_links, symbol)
    }

    fn alias_symbol_type_only_declaration_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
    ) -> Option<ast::Node> {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::alias_symbol_type_only_declaration_by_handle(&self.alias_symbol_links, handle)
    }

    fn set_alias_symbol_type_only_declaration<Q>(
        &self,
        symbol: Q,
        type_only_declaration: Option<ast::Node>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::set_alias_symbol_type_only_declaration(&self.alias_symbol_links, symbol, type_only_declaration)
    }

    fn set_alias_symbol_type_only_declaration_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
        type_only_declaration: Option<ast::Node>,
    ) {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::set_alias_symbol_type_only_declaration_by_handle(&self.alias_symbol_links, handle, type_only_declaration)
    }

    fn set_alias_symbol_type_only_declaration_if_none<Q>(
        &self,
        symbol: Q,
        type_only_declaration: Option<ast::Node>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::set_alias_symbol_type_only_declaration_if_none(&self.alias_symbol_links, symbol, type_only_declaration)
    }

    fn set_alias_symbol_type_only_declaration_if_none_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
        type_only_declaration: Option<ast::Node>,
    ) {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::set_alias_symbol_type_only_declaration_if_none_by_handle(&self.alias_symbol_links, handle, type_only_declaration)
    }

    fn alias_symbol_type_only_export_star_name<Q>(&self, symbol: Q) -> Option<String>
    where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::alias_symbol_type_only_export_star_name(&self.alias_symbol_links, symbol)
    }

    fn alias_symbol_type_only_export_star_name_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
    ) -> Option<String> {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::alias_symbol_type_only_export_star_name_by_handle(&self.alias_symbol_links, handle)
    }

    fn set_alias_symbol_type_only_export_star_name<Q>(
        &self,
        symbol: Q,
        type_only_export_star_name: Option<String>,
    ) where
        Q: core::IntoLinkKey<SymbolIdentity>,
    {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::set_alias_symbol_type_only_export_star_name(&self.alias_symbol_links, symbol, type_only_export_star_name)
    }

    fn set_alias_symbol_type_only_export_star_name_by_handle(
        &self,
        handle: core::LinkHandle<AliasSymbolLinks>,
        type_only_export_star_name: Option<String>,
    ) {
        <SymbolLinkStore<AliasSymbolLinks> as AliasSymbolLinksStoreExt>::set_alias_symbol_type_only_export_star_name_by_handle(&self.alias_symbol_links, handle, type_only_export_star_name)
    }
}

impl crate::emitresolver::JSXLinksStoreExt for NodeLinkStore<JSXLinks> {
    fn jsx_link_handle(&self, node: ast::Node) -> core::LinkHandle<JSXLinks> {
        self.ensure_handle(node)
    }

    fn jsx_import_ref(&self, node: ast::Node) -> Option<ast::Node> {
        let handle = self.jsx_link_handle(node);
        self.jsx_import_ref_by_handle(handle)
    }

    fn jsx_import_ref_by_handle(&self, handle: core::LinkHandle<JSXLinks>) -> Option<ast::Node> {
        self.with_by_handle(handle, |links| links.import_ref())
    }

    fn set_jsx_import_ref(&self, node: ast::Node, import_ref: Option<ast::Node>) {
        let handle = self.jsx_link_handle(node);
        self.set_jsx_import_ref_by_handle(handle, import_ref);
    }

    fn set_jsx_import_ref_by_handle(
        &self,
        handle: core::LinkHandle<JSXLinks>,
        import_ref: Option<ast::Node>,
    ) {
        self.with_by_handle_mut(handle, |links| {
            links.set_import_ref(import_ref);
        });
    }
}

impl crate::emitresolver::JSXLinksStoreExt for CheckerState {
    fn jsx_link_handle(&self, node: ast::Node) -> core::LinkHandle<JSXLinks> {
        <NodeLinkStore<JSXLinks> as crate::emitresolver::JSXLinksStoreExt>::jsx_link_handle(
            &self.jsx_links,
            node,
        )
    }

    fn jsx_import_ref(&self, node: ast::Node) -> Option<ast::Node> {
        <NodeLinkStore<JSXLinks> as crate::emitresolver::JSXLinksStoreExt>::jsx_import_ref(
            &self.jsx_links,
            node,
        )
    }

    fn jsx_import_ref_by_handle(&self, handle: core::LinkHandle<JSXLinks>) -> Option<ast::Node> {
        <NodeLinkStore<JSXLinks> as crate::emitresolver::JSXLinksStoreExt>::jsx_import_ref_by_handle(
            &self.jsx_links,
            handle,
        )
    }

    fn set_jsx_import_ref(&self, node: ast::Node, import_ref: Option<ast::Node>) {
        <NodeLinkStore<JSXLinks> as crate::emitresolver::JSXLinksStoreExt>::set_jsx_import_ref(
            &self.jsx_links,
            node,
            import_ref,
        )
    }

    fn set_jsx_import_ref_by_handle(
        &self,
        handle: core::LinkHandle<JSXLinks>,
        import_ref: Option<ast::Node>,
    ) {
        <NodeLinkStore<JSXLinks> as crate::emitresolver::JSXLinksStoreExt>::set_jsx_import_ref_by_handle(
            &self.jsx_links,
            handle,
            import_ref,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emitresolver::JSXLinksStoreExt;
    use crate::types::{OBJECT_FLAGS_NONE, SIGNATURE_FLAGS_NONE, TYPE_FLAGS_ANY};
    use ts_diagnostics as diagnostics;

    fn test_state() -> CheckerState {
        CheckerState::new_for_slot_index(0)
    }

    #[test]
    fn symbol_link_store_separates_program_symbol_owners() {
        let links = SymbolLinkStore::<ValueSymbolLinks>::default();
        let mut first_store = ast::ProgramSymbolStore::new();
        let mut second_store = ast::ProgramSymbolStore::new();
        let first = first_store.create_binding_symbol(ast::SYMBOL_FLAGS_PROPERTY, "sameIndex");
        let second = second_store.create_binding_symbol(ast::SYMBOL_FLAGS_PROPERTY, "sameIndex");
        let first_identity = SymbolIdentity::from_symbol_handle(first);

        assert_eq!(first.symbol_index(), second.symbol_index());
        assert_ne!(first.owner_key(), second.owner_key());

        let first_handle = links.value_symbol_link_handle(&first);
        let second_handle = links.value_symbol_link_handle(&second);

        assert_ne!(first_handle, second_handle);
        links.set_value_symbol_target_by_handle(first_handle, Some(first_identity));
        assert_eq!(links.value_symbol_target(&first), Some(first_identity));
        assert_eq!(links.value_symbol_target(&second), None);
    }

    #[test]
    fn symbol_link_store_lookup_respects_program_symbol_owners() {
        let links = SymbolLinkStore::<ValueSymbolLinks>::default();
        let mut first_store = ast::ProgramSymbolStore::new();
        let mut second_store = ast::ProgramSymbolStore::new();
        let first = first_store.create_binding_symbol(ast::SYMBOL_FLAGS_PROPERTY, "sameIndex");
        let second = second_store.create_binding_symbol(ast::SYMBOL_FLAGS_PROPERTY, "sameIndex");

        assert_eq!(first.symbol_index(), second.symbol_index());
        assert_ne!(first.owner_key(), second.owner_key());

        let first_handle = links.value_symbol_link_handle(&first);

        assert!(links.has(&first));
        assert!(!links.has(&second));
        assert_eq!(links.try_handle(&first), Some(first_handle));
        assert_eq!(links.try_handle(&second), None);
    }

    #[test]
    fn symbol_link_store_separates_transient_symbol_owners() {
        let links = SymbolLinkStore::<ValueSymbolLinks>::default();
        let mut first_store = ast::TransientSymbolStore::new();
        let mut second_store = ast::TransientSymbolStore::new();
        let first = first_store.create_transient_symbol(ast::SYMBOL_FLAGS_PROPERTY, "sameIndex");
        let second = second_store.create_transient_symbol(ast::SYMBOL_FLAGS_PROPERTY, "sameIndex");
        let first_identity = SymbolIdentity::from_symbol_handle(first);

        assert_eq!(first.symbol_index(), second.symbol_index());
        assert_ne!(first.owner_key(), second.owner_key());

        let first_handle = links.value_symbol_link_handle(&first);
        let second_handle = links.value_symbol_link_handle(&second);

        assert_ne!(first_handle, second_handle);
        links.set_value_symbol_target_by_handle(first_handle, Some(first_identity));
        assert_eq!(links.value_symbol_target(&first), Some(first_identity));
        assert_eq!(links.value_symbol_target(&second), None);
    }

    #[test]
    fn symbol_link_store_lookup_respects_transient_symbol_owners() {
        let links = SymbolLinkStore::<ValueSymbolLinks>::default();
        let mut first_store = ast::TransientSymbolStore::new();
        let mut second_store = ast::TransientSymbolStore::new();
        let first = first_store.create_transient_symbol(ast::SYMBOL_FLAGS_PROPERTY, "sameIndex");
        let second = second_store.create_transient_symbol(ast::SYMBOL_FLAGS_PROPERTY, "sameIndex");

        assert_eq!(first.symbol_index(), second.symbol_index());
        assert_ne!(first.owner_key(), second.owner_key());

        let first_handle = links.value_symbol_link_handle(&first);

        assert!(links.has(&first));
        assert!(!links.has(&second));
        assert_eq!(links.try_handle(&first), Some(first_handle));
        assert_eq!(links.try_handle(&second), None);

        let second_handle = links.value_symbol_link_handle(&second);

        assert_ne!(first_handle, second_handle);
        assert_eq!(links.try_handle(&first), Some(first_handle));
        assert_eq!(links.try_handle(&second), Some(second_handle));
    }

    #[test]
    fn merged_symbol_store_separates_program_symbol_owners() {
        let merged_symbols = MergedSymbolStore::default();
        let mut first_store = ast::ProgramSymbolStore::new();
        let mut second_store = ast::ProgramSymbolStore::new();
        let first = first_store.create_binding_symbol(ast::SYMBOL_FLAGS_PROPERTY, "sameIndex");
        let second = second_store.create_binding_symbol(ast::SYMBOL_FLAGS_PROPERTY, "sameIndex");
        let target = first_store.create_binding_symbol(ast::SYMBOL_FLAGS_PROPERTY, "target");
        let target_identity = SymbolIdentity::from_symbol_handle(target);

        assert_eq!(first.symbol_index(), second.symbol_index());
        assert_ne!(first.owner_key(), second.owner_key());

        merged_symbols.insert(first, target_identity);

        assert_eq!(merged_symbols.get(first), Some(target_identity));
        assert_eq!(merged_symbols.get(second), None);
    }

    #[test]
    fn merged_symbol_store_separates_transient_symbol_owners() {
        let merged_symbols = MergedSymbolStore::default();
        let mut first_store = ast::TransientSymbolStore::new();
        let mut second_store = ast::TransientSymbolStore::new();
        let first = first_store.create_transient_symbol(ast::SYMBOL_FLAGS_PROPERTY, "sameIndex");
        let second = second_store.create_transient_symbol(ast::SYMBOL_FLAGS_PROPERTY, "sameIndex");
        let first_target =
            first_store.create_transient_symbol(ast::SYMBOL_FLAGS_PROPERTY, "firstTarget");
        let second_target =
            second_store.create_transient_symbol(ast::SYMBOL_FLAGS_PROPERTY, "secondTarget");
        let first_target_identity = SymbolIdentity::from_symbol_handle(first_target);
        let second_target_identity = SymbolIdentity::from_symbol_handle(second_target);

        assert_eq!(first.symbol_index(), second.symbol_index());
        assert_ne!(first.owner_key(), second.owner_key());

        merged_symbols.insert(first, first_target_identity);
        merged_symbols.insert(second, second_target_identity);

        assert_eq!(merged_symbols.get(first), Some(first_target_identity));
        assert_eq!(merged_symbols.get(second), Some(second_target_identity));
    }

    #[test]
    fn merged_symbol_store_updates_cached_miss() {
        let merged_symbols = MergedSymbolStore::default();
        let mut store = ast::ProgramSymbolStore::new();
        let source = store.create_binding_symbol(ast::SYMBOL_FLAGS_PROPERTY, "source");
        let target = store.create_binding_symbol(ast::SYMBOL_FLAGS_PROPERTY, "target");
        let target_identity = SymbolIdentity::from_symbol_handle(target);

        assert_eq!(merged_symbols.get(source), None);

        merged_symbols.insert(source, target_identity);

        assert_eq!(merged_symbols.get(source), Some(target_identity));
    }

    #[test]
    fn checker_state_keeps_handle_domains_separate() {
        let mut state = test_state();
        let ty = state.alloc_type(TypeRecord {
            ts_id: 1,
            flags: TYPE_FLAGS_ANY,
            object_flags: OBJECT_FLAGS_NONE,
            symbol: None,
            alias: None,
            data: TypeRecordData::Intrinsic(IntrinsicTypeRecord {
                intrinsic_name: "any".to_string(),
            }),
        });
        let sig = state.alloc_signature(SignatureRecord {
            flags: SIGNATURE_FLAGS_NONE,
            min_argument_count: 0,
            resolved_min_argument_count: 0,
            declaration: None,
            type_parameters: Vec::new(),
            parameters: Arc::from([]),
            this_parameter: None,
            resolved_return_type: Some(ty),
            resolved_type_predicate: None,
            target: None,
            mapper: None,
            isolated_signature_type: None,
            composite: None,
        });

        state.record_marker_type(ty);

        assert_eq!(state.type_record(ty).ts_id, 1);
        assert_eq!(state.signature_record(sig).resolved_return_type, Some(ty));
        assert!(state.is_marker_type(ty));
        assert_eq!(state.identity().slot().get(), 1);
    }

    #[test]
    fn conditional_root_record_keeps_instantiations_by_cache_key() {
        let mut state = test_state();
        let ty = state.alloc_type(TypeRecord {
            ts_id: 1,
            flags: TYPE_FLAGS_ANY,
            object_flags: OBJECT_FLAGS_NONE,
            symbol: None,
            alias: None,
            data: TypeRecordData::Conditional(ConditionalTypeRecord::default()),
        });
        let root = state.alloc_conditional_root(ConditionalRootRecord {
            node: None,
            check_type: Some(ty),
            extends_type: Some(ty),
            is_distributive: true,
            infer_type_parameters: Vec::new(),
            outer_type_parameters: Vec::new(),
            instantiations: HashMap::new(),
            alias: None,
        });

        state
            .conditional_root_record_mut(root)
            .instantiations
            .insert(7, ty);

        assert_eq!(
            state
                .conditional_root_record(root)
                .instantiations
                .get(&7)
                .copied(),
            Some(ty)
        );
    }

    #[test]
    fn checker_state_owns_ts_type_ids() {
        let mut state = test_state();

        assert_eq!(state.next_type_id(), 1);
        assert_eq!(state.next_type_id(), 2);
        assert_eq!(state.ts_type_count(), 2);
    }

    #[test]
    fn checker_state_owns_checker_counters_and_flags() {
        let mut state = test_state();

        let _ = state.new_transient_symbol(ast::SYMBOL_FLAGS_PROPERTY, "counted".to_string());
        assert_eq!(state.symbol_count(), 1);

        state.enter_instantiation();
        assert_eq!(state.total_instantiation_count(), 1);
        assert_eq!(state.instantiation_count(), 1);
        assert_eq!(state.instantiation_depth(), 1);

        state.exit_instantiation();
        assert_eq!(state.instantiation_depth(), 0);

        let source = state.semantic_handles().any_type;
        let target = state.semantic_handles().unknown_type;
        let simple_mapper = state.alloc_mapper(TypeMapperRecord {
            data: TypeMapperRecordData::Simple(SimpleTypeMapperRecord { source, target }),
        });
        let array_mapper = state.alloc_mapper(TypeMapperRecord {
            data: TypeMapperRecordData::Array(ArrayTypeMapperRecord {
                sources: [source, target].into(),
                targets: [target, source].into(),
            }),
        });
        let merged_mapper = state.alloc_mapper(TypeMapperRecord {
            data: TypeMapperRecordData::Merged(MergedTypeMapperRecord {
                left: simple_mapper,
                right: array_mapper,
            }),
        });
        state.alloc_mapper(TypeMapperRecord {
            data: TypeMapperRecordData::Composite(CompositeTypeMapperRecord {
                left: merged_mapper,
                right: simple_mapper,
            }),
        });
        let mapper_counters = state.mapper_perf_counters();
        assert_eq!(mapper_counters.allocations.simple, 1);
        assert_eq!(mapper_counters.allocations.array, 1);
        assert_eq!(mapper_counters.allocations.merged, 1);
        assert_eq!(mapper_counters.allocations.composite, 1);
        assert_eq!(mapper_counters.max_chain_depth, 3);

        state.enter_inline();
        assert_eq!(state.inline_level(), 1);
        state.exit_inline();
        assert_eq!(state.inline_level(), 0);

        state.set_save_deferred_diagnostics(true);
        assert!(state.save_deferred_diagnostics());

        state.set_can_collect_symbol_alias_accessibility_data(true);
        assert!(state.can_collect_symbol_alias_accessibility_data());

        state.set_context(core::Context::background());
        assert!(state.context().err().is_none());
        state.reset_context();
        assert!(state.context().err().is_none());

        state.mark_canceled();
        assert!(state.was_canceled());

        state.set_inference_partially_blocked(true);
        assert!(state.is_inference_partially_blocked());

        state.add_reliability_flags(crate::relater::RELATION_COMPARISON_RESULT_REPORTS_UNRELIABLE);
        assert_eq!(
            state.reliability_flags(),
            crate::relater::RELATION_COMPARISON_RESULT_REPORTS_UNRELIABLE
        );
        state.set_reliability_flags(0);
        assert_eq!(state.reliability_flags(), 0);

        let mut symbols = ast::ProgramSymbolStore::new();
        let source = symbols.create_binding_symbol(ast::SYMBOL_FLAGS_PROPERTY, "source");
        let target = symbols.create_binding_symbol(ast::SYMBOL_FLAGS_PROPERTY, "target");
        let enum_key = EnumRelationKey {
            source: SymbolIdentity::from_symbol_handle(source),
            target: SymbolIdentity::from_symbol_handle(target),
        };
        state.set_enum_relation_result(
            enum_key,
            crate::relater::RELATION_COMPARISON_RESULT_SUCCEEDED,
        );
        assert_eq!(
            state.enum_relation_result(&enum_key),
            Some(crate::relater::RELATION_COMPARISON_RESULT_SUCCEEDED)
        );
    }

    #[test]
    fn checker_state_owns_derived_compiler_options() {
        let mut state = test_state();
        let mut options = core::CompilerOptions {
            target: core::ScriptTarget::ES2020,
            module: core::ModuleKind::Node16,
            experimental_decorators: core::TSTrue,
            exact_optional_property_types: core::TSTrue,
            ..Default::default()
        };
        options.strict = core::TSTrue;

        state.set_options_from_compiler_options(&options);

        assert_eq!(state.language_version(), core::ScriptTarget::ES2020);
        assert_eq!(state.module_kind(), core::ModuleKind::Node16);
        assert_eq!(
            state.module_resolution_kind(),
            core::ModuleResolutionKind::Node16
        );
        assert!(state.legacy_decorators());
        assert!(state.strict_null_checks());
        assert!(state.strict_function_types());
        assert!(state.strict_bind_call_apply());
        assert!(state.strict_property_initialization());
        assert!(state.strict_builtin_iterator_return());
        assert!(state.no_implicit_any());
        assert!(state.no_implicit_this());
        assert!(state.use_unknown_in_catch_variables());
        assert!(state.exact_optional_property_types());
    }

    #[test]
    fn checker_state_owns_resolution_recursion_guards() {
        let mut state = test_state();

        state.enter_resolving_union_or_intersection_property(13, "p".to_string(), true);
        state.enter_awaited_type(17);

        assert!(state.is_resolving_union_or_intersection_property(13, "p", true));
        assert!(state.is_awaiting_type(17));

        state.exit_awaited_type();
        state.exit_resolving_union_or_intersection_property();

        assert!(!state.is_resolving_union_or_intersection_property(13, "p", true));
        assert!(!state.is_awaiting_type(17));
    }

    #[test]
    fn checker_state_owns_ast_node_scratch() {
        let mut state = test_state();
        let mut factory = ast::NodeFactory::default();
        let node = factory.new_identifier("value");
        let mut symbols = ast::ProgramSymbolStore::new();
        let symbol = symbols.create_binding_symbol(ast::SYMBOL_FLAGS_PROPERTY, "value");
        let symbol_identity = state.intern_symbol_handle(symbol);

        state.set_current_node(Some(node));
        assert_eq!(state.current_node(), Some(node));

        let node_links_handle = state.node_links().node_link_handle(node);
        state
            .node_links()
            .add_node_link_flags_by_handle(node_links_handle, NODE_CHECK_FLAGS_TYPE_CHECKED);
        assert!(
            state
                .node_links()
                .has_node_link_flags(node, NODE_CHECK_FLAGS_TYPE_CHECKED)
        );
        assert_eq!(
            state
                .node_links()
                .node_link_flags_by_handle(node_links_handle),
            NODE_CHECK_FLAGS_TYPE_CHECKED
        );
        state
            .node_links()
            .remove_node_link_flags(node, NODE_CHECK_FLAGS_TYPE_CHECKED);
        assert_eq!(
            state.node_links().node_link_flags(node),
            NODE_CHECK_FLAGS_NONE
        );
        assert_eq!(
            state
                .node_links()
                .node_declaration_requires_scope_change_by_handle(node_links_handle),
            core::TSUnknown
        );
        state
            .node_links()
            .set_node_declaration_requires_scope_change_by_handle(node_links_handle, core::TSTrue);
        assert_eq!(
            state
                .node_links()
                .node_declaration_requires_scope_change(node),
            core::TSTrue
        );
        state
            .node_links()
            .set_node_declaration_requires_scope_change(node, core::TSFalse);
        assert_eq!(
            state
                .node_links()
                .node_declaration_requires_scope_change_by_handle(node_links_handle),
            core::TSFalse
        );
        state
            .node_links()
            .set_node_declaration_requires_scope_change(node, core::TSUnknown);
        assert_eq!(
            state
                .node_links()
                .node_declaration_requires_scope_change_by_handle(node_links_handle),
            core::TSUnknown
        );

        let symbol_node_links_handle = state.symbol_node_links().symbol_node_link_handle(node);
        assert_eq!(
            state
                .symbol_node_links()
                .node_resolved_symbol_identity_by_handle(symbol_node_links_handle),
            None
        );
        state
            .symbol_node_links()
            .set_node_resolved_symbol_identity_by_handle(
                symbol_node_links_handle,
                Some(symbol_identity),
            );
        assert_eq!(
            state
                .symbol_node_links()
                .node_resolved_symbol_identity(node)
                .map(|symbol| symbol.ast_identity()),
            Some(symbol_identity.ast_identity())
        );
        state
            .symbol_node_links()
            .set_node_resolved_symbol_identity(node, None);
        assert_eq!(
            state
                .symbol_node_links()
                .node_resolved_symbol_identity_by_handle(symbol_node_links_handle),
            None
        );

        let type_node_links_handle = state.type_node_links().type_node_link_handle(node);
        assert_eq!(
            state
                .type_node_links()
                .type_node_outer_type_parameters_by_handle(type_node_links_handle),
            None
        );
        state
            .type_node_links()
            .set_type_node_outer_type_parameters_by_handle(type_node_links_handle, Vec::new());
        assert!(
            state
                .type_node_links()
                .type_node_outer_type_parameters(node)
                .is_some_and(|type_parameters| type_parameters.is_empty())
        );
        let any_type = state.semantic_handles().any_type;
        state
            .type_node_links()
            .set_type_node_outer_type_parameters(node, vec![any_type]);
        assert_eq!(
            state
                .type_node_links()
                .type_node_outer_type_parameters_by_handle(type_node_links_handle)
                .as_deref(),
            Some(&[any_type][..])
        );

        let array_literal_links_handle =
            state.array_literal_links().array_literal_link_handle(node);
        assert_eq!(
            state
                .array_literal_links()
                .array_literal_spread_indices_by_handle(array_literal_links_handle),
            (-1, -1)
        );
        assert!(
            !state
                .array_literal_links()
                .array_literal_indices_computed_by_handle(array_literal_links_handle)
        );
        state
            .array_literal_links()
            .set_array_literal_spread_indices_by_handle(array_literal_links_handle, 1, 3);
        state
            .array_literal_links()
            .set_array_literal_indices_computed_by_handle(array_literal_links_handle, true);
        assert_eq!(
            state
                .array_literal_links()
                .array_literal_spread_indices(node),
            (1, 3)
        );
        assert!(
            state
                .array_literal_links()
                .array_literal_indices_computed(node)
        );

        state.enum_member_links().enum_member_link_handle(node);
        assert!(state.enum_member_links().has(node));

        state.jsx_links().jsx_link_handle(node);
        assert!(state.jsx_links().has(node));

        state.declaration_links().declaration_link_handle(node);
        assert!(state.declaration_links().has(node));

        let statements = factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            Vec::new(),
        );
        let source_file = factory.new_source_file(
            ast::SourceFileParseOptions {
                file_name: "/checker_state.ts".to_string(),
                path: "/checker_state.ts".to_string(),
                ..Default::default()
            },
            String::new(),
            statements,
            None,
        );
        let source_file_key = SourceFileIdentity::from_root(source_file);
        state
            .declaration_file_links()
            .declaration_file_link_handle(source_file_key);
        assert!(state.declaration_file_links().has(source_file_key));

        state.set_jsx_namespace("React".to_string());
        assert_eq!(state.jsx_namespace(), "React");
        state.set_jsx_factory_entity(Some(node));
        assert_eq!(state.jsx_factory_entity(), Some(node));

        state.record_combined_node_flags_cache(node, ast::NodeFlags::Const);
        assert_eq!(
            state.combined_node_flags_cache(node),
            Some(ast::NodeFlags::Const)
        );

        state.record_combined_modifier_flags_cache(node, ast::MODIFIER_FLAGS_EXPORT);
        assert_eq!(
            state.combined_modifier_flags_cache(node),
            Some(ast::MODIFIER_FLAGS_EXPORT)
        );
    }

    #[test]
    fn checker_state_owns_diagnostic_collections() {
        let state = test_state();
        let diagnostic = ast::new_compiler_diagnostic(&diagnostics::Default_library, &[]);

        state.diagnostics().add(diagnostic.clone());

        let stored = state.diagnostics().get_global_diagnostics();
        assert!(stored.len() == 1 && ast::equal_diagnostics(&stored[0], &diagnostic));
    }

    #[test]
    fn checker_state_owns_output_node_factory() {
        let mut state = test_state();
        let node = state.factory_mut().new_identifier("synthetic");
        let loc = core::new_text_range(1, 2);

        state.factory_mut().place_checker_synthetic_node(node, loc);

        assert_eq!(state.factory().store().text(node), "synthetic");
        assert_eq!(state.factory().store().loc(node), loc);
    }

    #[test]
    fn checker_state_owns_symbol_caches() {
        let mut state = test_state();
        let mut symbols = ast::ProgramSymbolStore::new();
        let symbol = symbols.create_binding_symbol(ast::SYMBOL_FLAGS_PROPERTY, "cached");
        let related_symbol = symbols.create_binding_symbol(ast::SYMBOL_FLAGS_PROPERTY, "related");
        let other_symbol = symbols.create_binding_symbol(ast::SYMBOL_FLAGS_PROPERTY, "other");
        let symbol_identity = state.intern_symbol_handle(symbol);
        let related_symbol_identity = state.intern_symbol_handle(related_symbol);
        let other_symbol_identity = state.intern_symbol_handle(other_symbol);
        let originating_import = state.factory_mut().new_identifier("originatingImport");
        let type_only_declaration = state.factory_mut().new_identifier("typeOnly");
        let other_type_only_declaration = state.factory_mut().new_identifier("otherTypeOnly");

        state.insert_undefined_property_identity("p".to_string(), symbol_identity);
        state.insert_unresolved_symbol_identity("M.T".to_string(), symbol_identity);
        state.record_merged_symbol_identity(&symbol, symbol_identity);

        assert_eq!(state.array_variances(), &[VARIANCE_FLAGS_COVARIANT]);
        assert_eq!(
            state.undefined_property_identity("p"),
            Some(symbol_identity)
        );
        assert_eq!(
            state.unresolved_symbol_identity("M.T"),
            Some(symbol_identity)
        );
        assert_eq!(state.merged_symbol_identity(&symbol), Some(symbol_identity));
        state.record_this_expando(symbol_identity, THIS_ASSIGNMENT_DECLARATION_TYPED, None);
        assert_eq!(
            state.this_expando(symbol_identity),
            Some((THIS_ASSIGNMENT_DECLARATION_TYPED, None))
        );

        let symbol_reference_handle = state
            .symbol_reference_links()
            .symbol_reference_link_handle(&symbol);
        state
            .symbol_reference_links()
            .add_symbol_reference_kinds_by_handle(symbol_reference_handle, ast::SYMBOL_FLAGS_VALUE);
        assert!(
            state
                .symbol_reference_links()
                .has_symbol_reference_kinds(&symbol, ast::SYMBOL_FLAGS_VALUE)
        );
        assert_eq!(
            state
                .symbol_reference_links()
                .symbol_reference_kinds_by_handle(symbol_reference_handle),
            ast::SYMBOL_FLAGS_VALUE
        );
        state
            .symbol_reference_links()
            .set_symbol_reference_kinds(&symbol, ast::SYMBOL_FLAGS_TYPE);
        assert_eq!(
            state
                .symbol_reference_links()
                .symbol_reference_kinds(&symbol),
            ast::SYMBOL_FLAGS_TYPE
        );

        let alias_symbol_links_handle =
            state.alias_symbol_links().alias_symbol_link_handle(&symbol);
        assert!(
            !state
                .alias_symbol_links()
                .alias_symbol_referenced_by_handle(alias_symbol_links_handle)
        );
        state
            .alias_symbol_links()
            .mark_alias_symbol_referenced_by_handle(alias_symbol_links_handle);
        assert!(state.alias_symbol_links().alias_symbol_referenced(&symbol));
        state
            .alias_symbol_links()
            .set_alias_symbol_referenced(&symbol, false);
        assert!(
            !state
                .alias_symbol_links()
                .alias_symbol_referenced_by_handle(alias_symbol_links_handle)
        );
        assert_eq!(
            state
                .alias_symbol_links()
                .alias_symbol_type_only_declaration_by_handle(alias_symbol_links_handle),
            None
        );
        state
            .alias_symbol_links()
            .set_alias_symbol_type_only_declaration_by_handle(
                alias_symbol_links_handle,
                Some(type_only_declaration),
            );
        assert_eq!(
            state
                .alias_symbol_links()
                .alias_symbol_type_only_declaration(&symbol),
            Some(type_only_declaration)
        );
        state
            .alias_symbol_links()
            .set_alias_symbol_type_only_declaration_if_none(
                &symbol,
                Some(other_type_only_declaration),
            );
        assert_eq!(
            state
                .alias_symbol_links()
                .alias_symbol_type_only_declaration_by_handle(alias_symbol_links_handle),
            Some(type_only_declaration)
        );
        state
            .alias_symbol_links()
            .set_alias_symbol_type_only_export_star_name_by_handle(
                alias_symbol_links_handle,
                Some("exported".to_string()),
            );
        assert_eq!(
            state
                .alias_symbol_links()
                .alias_symbol_type_only_export_star_name(&symbol)
                .as_deref(),
            Some("exported")
        );
        state
            .alias_symbol_links()
            .set_alias_symbol_type_only_declaration(&symbol, None);
        assert_eq!(
            state
                .alias_symbol_links()
                .alias_symbol_type_only_declaration_by_handle(alias_symbol_links_handle),
            None
        );
        assert_eq!(
            state
                .alias_symbol_links()
                .alias_symbol_target_by_handle(alias_symbol_links_handle),
            None
        );
        state
            .alias_symbol_links()
            .set_alias_symbol_target_by_handle(
                alias_symbol_links_handle,
                Some(related_symbol_identity),
            );
        assert_eq!(
            state.alias_symbol_links().alias_symbol_target(&symbol),
            Some(related_symbol_identity)
        );
        state
            .alias_symbol_links()
            .set_alias_symbol_immediate_target(&symbol, Some(other_symbol_identity));
        assert_eq!(
            state
                .alias_symbol_links()
                .alias_symbol_immediate_target_by_handle(alias_symbol_links_handle),
            Some(other_symbol_identity)
        );

        let module_symbol_links_handle = state
            .module_symbol_links()
            .module_symbol_link_handle(&symbol);
        assert!(
            !state
                .module_symbol_links()
                .module_resolved_exports_is_resolved_by_handle(module_symbol_links_handle)
        );
        state
            .module_symbol_links()
            .with_module_resolved_exports_by_handle(module_symbol_links_handle, |exports| {
                assert!(exports.is_none());
            });
        state
            .module_symbol_links()
            .set_module_resolved_exports_by_handle(
                module_symbol_links_handle,
                SymbolIdentityTable::default(),
                None,
            );
        assert!(
            state
                .module_symbol_links()
                .module_resolved_exports_is_resolved(&symbol)
        );
        state
            .module_symbol_links()
            .with_module_resolved_exports(&symbol, |exports| {
                assert!(exports.is_some_and(SymbolIdentityTable::is_empty));
            });
        let mut resolved_exports = SymbolIdentityTable::default();
        resolved_exports.insert("cached".into(), related_symbol_identity);
        let mut type_only_export_star_map = HashMap::new();
        type_only_export_star_map.insert("cached".to_string(), type_only_declaration);
        state
            .module_symbol_links()
            .set_module_resolved_exports_by_handle(
                module_symbol_links_handle,
                resolved_exports.clone(),
                Some(type_only_export_star_map),
            );
        state
            .module_symbol_links()
            .with_module_resolved_exports(&symbol, |exports| {
                assert_eq!(exports, Some(&resolved_exports));
            });
        assert_eq!(
            state
                .module_symbol_links()
                .module_type_only_export_star_declaration(&symbol, "cached"),
            Some(type_only_declaration)
        );
        assert_eq!(
            state
                .module_symbol_links()
                .module_type_only_export_star_declaration_by_handle(
                    module_symbol_links_handle,
                    "missing",
                ),
            None
        );
        assert!(
            !state
                .module_symbol_links()
                .module_exports_checked_by_handle(module_symbol_links_handle)
        );
        state
            .module_symbol_links()
            .mark_module_exports_checked_by_handle(module_symbol_links_handle);
        assert!(state.module_symbol_links().module_exports_checked(&symbol));
        state
            .module_symbol_links()
            .set_module_exports_checked(&symbol, false);
        assert!(
            !state
                .module_symbol_links()
                .module_exports_checked_by_handle(module_symbol_links_handle)
        );

        let members_and_exports_links_handle = state
            .members_and_exports_links()
            .members_and_exports_link_handle(&symbol);
        assert!(
            !state
                .members_and_exports_links()
                .members_or_exports_slot_is_resolved_by_handle(
                    members_and_exports_links_handle,
                    MembersOrExportsResolutionKind::ResolvedMembers,
                )
        );
        state
            .members_and_exports_links()
            .with_resolved_members_or_exports_by_handle(
                members_and_exports_links_handle,
                MembersOrExportsResolutionKind::ResolvedMembers,
                |members| {
                    assert!(members.is_none());
                },
            );
        state
            .members_and_exports_links()
            .set_resolved_members_or_exports_by_handle(
                members_and_exports_links_handle,
                MembersOrExportsResolutionKind::ResolvedMembers,
                SymbolIdentityTable::default(),
            );
        assert!(
            state
                .members_and_exports_links()
                .members_or_exports_slot_is_resolved(
                    &symbol,
                    MembersOrExportsResolutionKind::ResolvedMembers,
                )
        );
        state
            .members_and_exports_links()
            .with_resolved_members_or_exports(
                &symbol,
                MembersOrExportsResolutionKind::ResolvedMembers,
                |members| {
                    assert!(members.is_some_and(SymbolIdentityTable::is_empty));
                },
            );
        state
            .members_and_exports_links()
            .clear_resolved_members_and_exports_by_handle(members_and_exports_links_handle);
        assert!(
            !state
                .members_and_exports_links()
                .members_or_exports_slot_is_resolved_by_handle(
                    members_and_exports_links_handle,
                    MembersOrExportsResolutionKind::ResolvedMembers,
                )
        );

        let late_bound_links_handle = state.late_bound_links().late_bound_link_handle(&symbol);
        assert_eq!(
            state
                .late_bound_links()
                .late_bound_symbol_by_handle(late_bound_links_handle),
            None
        );
        state
            .late_bound_links()
            .set_late_bound_symbol_by_handle(late_bound_links_handle, Some(symbol_identity));
        assert_eq!(
            state
                .late_bound_links()
                .late_bound_symbol(&symbol)
                .map(|symbol| symbol.ast_identity()),
            Some(symbol_identity.ast_identity())
        );
        state
            .late_bound_links()
            .set_late_bound_symbol(&symbol, None);
        assert_eq!(
            state
                .late_bound_links()
                .late_bound_symbol_by_handle(late_bound_links_handle),
            None
        );

        let export_type_links_handle = state.export_type_links().export_type_link_handle(&symbol);
        assert_eq!(
            state
                .export_type_links()
                .export_type_target_by_handle(export_type_links_handle),
            None
        );
        state
            .export_type_links()
            .set_export_type_target_by_handle(export_type_links_handle, Some(symbol_identity));
        assert_eq!(
            state
                .export_type_links()
                .export_type_target(&symbol)
                .map(|symbol| symbol.ast_identity()),
            Some(symbol_identity.ast_identity())
        );
        state
            .export_type_links()
            .set_export_type_target(&symbol, None);
        assert_eq!(
            state
                .export_type_links()
                .export_type_target_by_handle(export_type_links_handle),
            None
        );
        assert_eq!(
            state
                .export_type_links()
                .export_type_originating_import_by_handle(export_type_links_handle),
            None
        );
        state
            .export_type_links()
            .set_export_type_originating_import_by_handle(
                export_type_links_handle,
                Some(originating_import),
            );
        assert_eq!(
            state
                .export_type_links()
                .export_type_originating_import(&symbol),
            Some(originating_import)
        );
        state
            .export_type_links()
            .set_export_type_originating_import(&symbol, None);
        assert_eq!(
            state
                .export_type_links()
                .export_type_originating_import_by_handle(export_type_links_handle),
            None
        );

        let value_symbol_links_handle =
            state.value_symbol_links().value_symbol_link_handle(&symbol);
        assert_eq!(
            state
                .value_symbol_links()
                .value_symbol_target_by_handle(value_symbol_links_handle),
            None
        );
        state
            .value_symbol_links()
            .set_value_symbol_target_by_handle(value_symbol_links_handle, Some(symbol_identity));
        assert_eq!(
            state
                .value_symbol_links()
                .value_symbol_target_by_handle(value_symbol_links_handle),
            Some(symbol_identity)
        );
        assert_eq!(
            state.value_symbol_links().value_symbol_target(&symbol),
            Some(symbol_identity)
        );
        state
            .value_symbol_links()
            .set_value_symbol_target_by_handle(value_symbol_links_handle, None);
        assert_eq!(
            state
                .value_symbol_links()
                .value_symbol_target_by_handle(value_symbol_links_handle),
            None
        );
        state
            .value_symbol_links()
            .set_value_symbol_target(&symbol, Some(other_symbol_identity));
        assert_eq!(
            state
                .value_symbol_links()
                .value_symbol_target_by_handle(value_symbol_links_handle),
            Some(other_symbol_identity)
        );
        state
            .value_symbol_links()
            .set_value_symbol_target(&symbol, None);
        assert_eq!(
            state
                .value_symbol_links()
                .value_symbol_target_by_handle(value_symbol_links_handle),
            None
        );
        assert_eq!(
            state
                .value_symbol_links()
                .cjs_export_merged_by_handle(value_symbol_links_handle),
            None
        );
        state.value_symbol_links().set_cjs_export_merged_by_handle(
            value_symbol_links_handle,
            Some(related_symbol_identity),
        );
        assert_eq!(
            state.value_symbol_links().cjs_export_merged(&symbol),
            Some(related_symbol_identity)
        );
        state
            .value_symbol_links()
            .set_cjs_export_merged(&symbol, None);
        assert_eq!(
            state
                .value_symbol_links()
                .cjs_export_merged_by_handle(value_symbol_links_handle),
            None
        );

        let mapped_symbol_links_handle = state
            .mapped_symbol_links()
            .mapped_symbol_link_handle(&symbol);
        assert_eq!(
            state
                .mapped_symbol_links()
                .mapped_synthetic_origin_by_handle(mapped_symbol_links_handle),
            None
        );
        state
            .mapped_symbol_links()
            .set_mapped_synthetic_origin_by_handle(
                mapped_symbol_links_handle,
                Some(related_symbol_identity),
            );
        assert_eq!(
            state
                .mapped_symbol_links()
                .mapped_synthetic_origin_by_handle(mapped_symbol_links_handle),
            Some(related_symbol_identity)
        );
        assert_eq!(
            state.mapped_symbol_links().mapped_synthetic_origin(&symbol),
            Some(related_symbol_identity)
        );
        state
            .mapped_symbol_links()
            .set_mapped_synthetic_origin(&symbol, None);
        assert_eq!(
            state
                .mapped_symbol_links()
                .mapped_synthetic_origin_by_handle(mapped_symbol_links_handle),
            None
        );
        state
            .mapped_symbol_links()
            .set_mapped_synthetic_origin(&symbol, Some(other_symbol_identity));
        assert_eq!(
            state
                .mapped_symbol_links()
                .mapped_synthetic_origin_by_handle(mapped_symbol_links_handle),
            Some(other_symbol_identity)
        );

        let spread_links_handle = state.spread_links().spread_link_handle(&symbol);
        assert_eq!(
            state
                .spread_links()
                .spread_symbols_by_handle(spread_links_handle),
            (None, None)
        );
        state.spread_links().set_spread_symbols_by_handle(
            spread_links_handle,
            Some(symbol_identity),
            Some(related_symbol_identity),
        );
        assert_eq!(
            state
                .spread_links()
                .spread_symbols_by_handle(spread_links_handle),
            (Some(symbol_identity), Some(related_symbol_identity))
        );
        assert_eq!(
            state.spread_links().spread_symbols(&symbol),
            (Some(symbol_identity), Some(related_symbol_identity))
        );
        state.spread_links().set_spread_symbols(&symbol, None, None);
        assert_eq!(
            state
                .spread_links()
                .spread_symbols_by_handle(spread_links_handle),
            (None, None)
        );
        state.spread_links().set_spread_symbols(
            &symbol,
            Some(other_symbol_identity),
            Some(symbol_identity),
        );
        assert_eq!(
            state
                .spread_links()
                .spread_symbols_by_handle(spread_links_handle),
            (Some(other_symbol_identity), Some(symbol_identity))
        );

        let mut factory = ast::NodeFactory::default();
        let statements = factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            Vec::new(),
        );
        let source_file = factory.new_source_file(
            ast::SourceFileParseOptions {
                file_name: "/checker_state.ts".to_string(),
                path: "/checker_state.ts".to_string(),
                ..Default::default()
            },
            String::new(),
            statements,
            None,
        );
        let source_file_key = SourceFileIdentity::from_root(source_file);
        let source_file_links_handle = state
            .source_file_links()
            .source_file_link_handle(source_file_key);
        assert!(
            !state
                .source_file_links()
                .source_file_type_checked_by_handle(source_file_links_handle)
        );
        state
            .source_file_links()
            .set_source_file_type_checked_by_handle(source_file_links_handle, true);
        assert!(
            state
                .source_file_links()
                .source_file_type_checked(source_file_key)
        );
        assert!(
            !state
                .source_file_links()
                .source_file_unused_checked_by_handle(source_file_links_handle)
        );
        state
            .source_file_links()
            .set_source_file_unused_checked(source_file_key, true);
        assert!(
            state
                .source_file_links()
                .source_file_unused_checked_by_handle(source_file_links_handle)
        );
        assert_eq!(
            state
                .source_file_links()
                .source_file_external_helpers_module_by_handle(source_file_links_handle),
            None
        );
        state
            .source_file_links()
            .set_source_file_external_helpers_module_by_handle(
                source_file_links_handle,
                Some(related_symbol_identity),
            );
        assert_eq!(
            state
                .source_file_links()
                .source_file_external_helpers_module_by_handle(source_file_links_handle),
            Some(related_symbol_identity)
        );
        assert_eq!(
            state
                .source_file_links()
                .source_file_external_helpers_module(source_file_key),
            Some(related_symbol_identity)
        );
        state
            .source_file_links()
            .set_source_file_external_helpers_module(source_file_key, None);
        assert_eq!(
            state
                .source_file_links()
                .source_file_external_helpers_module_by_handle(source_file_links_handle),
            None
        );
        state
            .source_file_links()
            .set_source_file_external_helpers_module(source_file_key, Some(other_symbol_identity));
        assert_eq!(
            state
                .source_file_links()
                .source_file_external_helpers_module_by_handle(source_file_links_handle),
            Some(other_symbol_identity)
        );
        assert_eq!(
            state
                .source_file_links()
                .source_file_requested_external_emit_helpers_by_handle(source_file_links_handle),
            0
        );
        state
            .source_file_links()
            .set_source_file_requested_external_emit_helpers(
                source_file_key,
                EXTERNAL_EMIT_HELPERS_REST,
            );
        assert!(
            state
                .source_file_links()
                .source_file_has_requested_external_emit_helpers_by_handle(
                    source_file_links_handle,
                    EXTERNAL_EMIT_HELPERS_REST,
                )
        );
        assert!(
            !state
                .source_file_links()
                .source_file_has_requested_external_emit_helpers(
                    source_file_key,
                    EXTERNAL_EMIT_HELPERS_AWAITER,
                )
        );
        state
            .source_file_links()
            .add_source_file_requested_external_emit_helpers_by_handle(
                source_file_links_handle,
                EXTERNAL_EMIT_HELPERS_AWAITER,
            );
        assert_eq!(
            state
                .source_file_links()
                .source_file_requested_external_emit_helpers(source_file_key),
            EXTERNAL_EMIT_HELPERS_REST | EXTERNAL_EMIT_HELPERS_AWAITER
        );

        let variance_links_handle = state.variance_links().variance_link_handle(&symbol);
        assert_eq!(
            state
                .variance_links()
                .variance_cache_state_by_handle(variance_links_handle),
            VarianceCacheState::Uncomputed
        );
        state
            .variance_links()
            .mark_variances_computing_by_handle(variance_links_handle);
        assert_eq!(
            state
                .variance_links()
                .variance_cache_state_by_handle(variance_links_handle),
            VarianceCacheState::Computing
        );
        assert_eq!(
            state
                .variance_links()
                .variance_cache_state_by_handle(variance_links_handle)
                .into_variances_or_empty(),
            Vec::<VarianceFlags>::new()
        );
        state.variance_links().set_variances_computed_by_handle(
            variance_links_handle,
            vec![VARIANCE_FLAGS_COVARIANT],
        );
        assert_eq!(
            state.variance_links().variance_cache_state(&symbol),
            VarianceCacheState::Computed(vec![VARIANCE_FLAGS_COVARIANT])
        );
        assert_eq!(
            state
                .variance_links()
                .variance_cache_state(&symbol)
                .into_variances_or_empty(),
            vec![VARIANCE_FLAGS_COVARIANT]
        );
        state
            .variance_links()
            .set_variances_computed(&symbol, Vec::new());
        assert_eq!(
            state.variance_links().variance_cache_state(&symbol),
            VarianceCacheState::Computed(Vec::new())
        );

        state
            .marked_assignment_symbol_links()
            .mark_marked_assignment_has_definite_assignment(&symbol);
        assert!(
            state
                .marked_assignment_symbol_links()
                .marked_assignment_has_definite_assignment(&symbol)
        );

        state
            .symbol_container_links()
            .set_extended_containers(&symbol, vec![symbol_identity]);
        assert_eq!(
            state
                .symbol_container_links()
                .extended_containers(&symbol)
                .as_deref()
                .map(|symbols| symbols.len()),
            Some(1)
        );
    }

    #[test]
    fn checker_state_owns_global_symbol_tables() {
        let mut state = test_state();
        let mut symbol_store = ast::ProgramSymbolStore::new();
        let undefined =
            symbol_store.create_binding_symbol(ast::SYMBOL_FLAGS_PROPERTY, "undefined".to_string());
        let global_this =
            symbol_store.create_binding_symbol(ast::SYMBOL_FLAGS_MODULE, "globalThis".to_string());

        state.begin_global_symbol_table_initialization(2);
        let undefined_identity = state.intern_symbol_handle(undefined);
        let global_this_identity = state.intern_symbol_handle(global_this);
        state.set_undefined_symbol_identity(undefined_identity);
        state.set_global_this_symbol_identity(global_this_identity);
        state.insert_global_symbol_identity("undefined".to_string(), undefined_identity);
        state.insert_pattern_ambient_module_augmentation_identity(
            "\"pkg\"".to_string(),
            global_this_identity,
        );
        let program_handle = symbol_store
            .create_binding_symbol(ast::SYMBOL_FLAGS_PROPERTY, "programHandle".to_string());
        state.insert_global_symbol_handle("programHandle".to_string(), program_handle);

        assert_eq!(
            state.symbol_origin(SymbolIdentity::from_symbol_handle(undefined)),
            Some(SymbolOrigin::ProgramHandle(undefined))
        );
        assert_eq!(
            state.symbol_origin(SymbolIdentity::from_symbol_handle(program_handle)),
            Some(SymbolOrigin::ProgramHandle(program_handle))
        );
        assert_eq!(
            state.global_symbol_identity("programHandle"),
            Some(SymbolIdentity::from_symbol_handle(program_handle))
        );
        assert_eq!(
            state.symbol_handle(SymbolIdentity::from_symbol_handle(program_handle)),
            program_handle
        );
        assert_eq!(
            state.undefined_symbol_identity(),
            SymbolIdentity::from_symbol_handle(undefined)
        );
        assert_eq!(
            state.global_this_symbol_identity(),
            SymbolIdentity::from_symbol_handle(global_this)
        );
        assert_eq!(
            state.global_symbol_identity("undefined"),
            Some(SymbolIdentity::from_symbol_handle(undefined))
        );

        let ambient = symbol_store
            .create_binding_symbol(ast::SYMBOL_FLAGS_VALUE_MODULE, "\"pkg\"".to_string());
        state.insert_global_symbol_handle("\"pkg\"".to_string(), ambient);
        let ambient_modules = state.collect_ambient_module_identities();
        assert_eq!(ambient_modules.len(), 1);
        assert_eq!(
            ambient_modules[0],
            SymbolIdentity::from_symbol_handle(ambient)
        );
    }

    #[test]
    fn checker_state_owns_transient_symbols_by_handle() {
        let mut state = test_state();
        let handle = state.new_transient_symbol(ast::SYMBOL_FLAGS_PROPERTY, "value".to_string());
        state.set_transient_symbol_check_flags(handle, ast::CHECK_FLAGS_SYNTHETIC_PROPERTY);
        let symbol = state.transient_symbol_handle(handle);

        assert_eq!(state.transient_symbol_record_handle(symbol), Some(handle));
        assert_eq!(
            state.transient_symbol_identity(handle),
            SymbolIdentity::from_symbol_handle(symbol)
        );
        assert_eq!(
            state.symbol_origin(SymbolIdentity::from_symbol_handle(symbol)),
            Some(SymbolOrigin::Transient(symbol))
        );
        assert_eq!(
            state.transient_symbol_check_flags(handle),
            ast::CHECK_FLAGS_SYNTHETIC_PROPERTY
        );
        assert_eq!(state.transient_symbol_store().name(symbol), "value");
    }

    #[test]
    fn checker_state_keeps_primitive_alias_suggestions_stable() {
        let mut state = test_state();

        let first = state
            .primitive_type_alias_suggestion("String")
            .expect("String primitive suggestion exists");
        let second = state
            .primitive_type_alias_suggestion("String")
            .expect("String primitive suggestion exists");

        assert_eq!(
            SymbolIdentity::from_symbol_handle(first),
            SymbolIdentity::from_symbol_handle(second)
        );
        assert_eq!(state.transient_symbol_store().name(first), "string");
        assert!(state.transient_symbol_record_handle(first).is_some());
    }
}
