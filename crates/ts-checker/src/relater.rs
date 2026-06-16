use std::cell::Cell;
use std::collections::HashMap;

use crate::checker::*;
use crate::jsx::{JSX_NAMES_INTRINSIC_ATTRIBUTES, JSX_NAMES_INTRINSIC_CLASS_ATTRIBUTES};
use crate::semantic::{
    ExportTypeLinksStoreExt, IndexInfoHandle, SignatureHandle, SymbolIdentity,
    TemplateLiteralTypeRecord, TypeHandle, TypeMapperList, TypePredicateHandle,
    TypePredicateRecord, VarianceCacheState, VarianceLinksStoreExt,
};
use crate::utilities::is_late_bound_name;
use crate::{ast, collections, core, diagnostics, jsnum, tracing};

pub(crate) type SignatureCheckMode = u32;

pub(crate) const SIGNATURE_CHECK_MODE_NONE: SignatureCheckMode = 0;
pub(crate) const SIGNATURE_CHECK_MODE_BIVARIANT_CALLBACK: SignatureCheckMode = 1 << 0;
pub(crate) const SIGNATURE_CHECK_MODE_STRICT_CALLBACK: SignatureCheckMode = 1 << 1;
pub(crate) const SIGNATURE_CHECK_MODE_IGNORE_RETURN_TYPES: SignatureCheckMode = 1 << 2;
pub(crate) const SIGNATURE_CHECK_MODE_STRICT_ARITY: SignatureCheckMode = 1 << 3;
pub(crate) const SIGNATURE_CHECK_MODE_STRICT_TOP_SIGNATURE: SignatureCheckMode = 1 << 4;
pub(crate) const SIGNATURE_CHECK_MODE_CALLBACK: SignatureCheckMode =
    SIGNATURE_CHECK_MODE_BIVARIANT_CALLBACK | SIGNATURE_CHECK_MODE_STRICT_CALLBACK;

pub(crate) type MinArgumentCountFlags = u32;

pub(crate) const MIN_ARGUMENT_COUNT_FLAGS_NONE: MinArgumentCountFlags = 0;
pub(crate) const MIN_ARGUMENT_COUNT_FLAGS_STRONG_ARITY_FOR_UNTYPED_JS: MinArgumentCountFlags =
    1 << 0;
pub(crate) const MIN_ARGUMENT_COUNT_FLAGS_VOID_IS_NON_OPTIONAL: MinArgumentCountFlags = 1 << 1;

pub(crate) type IntersectionState = u32;

pub(crate) const INTERSECTION_STATE_NONE: IntersectionState = 0;
pub(crate) const INTERSECTION_STATE_SOURCE: IntersectionState = 1 << 0; // Source type is a constituent of an outer intersection
pub(crate) const INTERSECTION_STATE_TARGET: IntersectionState = 1 << 1; // Target type is a constituent of an outer intersection

pub(crate) type RecursionFlags = u32;

pub(crate) const RECURSION_FLAGS_NONE: RecursionFlags = 0;
pub(crate) const RECURSION_FLAGS_SOURCE: RecursionFlags = 1 << 0;
pub(crate) const RECURSION_FLAGS_TARGET: RecursionFlags = 1 << 1;
pub(crate) const RECURSION_FLAGS_BOTH: RecursionFlags =
    RECURSION_FLAGS_SOURCE | RECURSION_FLAGS_TARGET;

pub type ExpandingFlags = u8;

pub(crate) const EXPANDING_FLAGS_NONE: ExpandingFlags = 0;
pub(crate) const EXPANDING_FLAGS_SOURCE: ExpandingFlags = 1 << 0;
pub(crate) const EXPANDING_FLAGS_TARGET: ExpandingFlags = 1 << 1;
pub(crate) const EXPANDING_FLAGS_BOTH: ExpandingFlags =
    EXPANDING_FLAGS_SOURCE | EXPANDING_FLAGS_TARGET;

pub type RelationComparisonResult = u32;

pub const RELATION_COMPARISON_RESULT_NONE: RelationComparisonResult = 0;
pub const RELATION_COMPARISON_RESULT_SUCCEEDED: RelationComparisonResult = 1 << 0;
pub const RELATION_COMPARISON_RESULT_FAILED: RelationComparisonResult = 1 << 1;
pub const RELATION_COMPARISON_RESULT_REPORTS_UNMEASURABLE: RelationComparisonResult = 1 << 3;
pub const RELATION_COMPARISON_RESULT_REPORTS_UNRELIABLE: RelationComparisonResult = 1 << 4;
pub const RELATION_COMPARISON_RESULT_COMPLEXITY_OVERFLOW: RelationComparisonResult = 1 << 5;
pub const RELATION_COMPARISON_RESULT_STACK_DEPTH_OVERFLOW: RelationComparisonResult = 1 << 6;
pub const RELATION_COMPARISON_RESULT_REPORTS_MASK: RelationComparisonResult =
    RELATION_COMPARISON_RESULT_REPORTS_UNMEASURABLE | RELATION_COMPARISON_RESULT_REPORTS_UNRELIABLE;
pub const RELATION_COMPARISON_RESULT_OVERFLOW: RelationComparisonResult =
    RELATION_COMPARISON_RESULT_COMPLEXITY_OVERFLOW
        | RELATION_COMPARISON_RESULT_STACK_DEPTH_OVERFLOW;

#[inline]
pub(crate) fn starts_with_text(text: &str, prefix: &str) -> bool {
    let prefix_len = prefix.len();
    if prefix_len == 0 {
        return true;
    }
    let text_bytes = text.as_bytes();
    let prefix_bytes = prefix.as_bytes();
    if text_bytes.len() < prefix_len || text_bytes[0] != prefix_bytes[0] {
        return false;
    }
    if prefix_len == 1 {
        return true;
    }
    if text_bytes[prefix_len - 1] != prefix_bytes[prefix_len - 1] {
        return false;
    }
    text_bytes[..prefix_len] == prefix_bytes[..]
}

#[inline]
pub(crate) fn ends_with_text(text: &str, suffix: &str) -> bool {
    let suffix_len = suffix.len();
    if suffix_len == 0 {
        return true;
    }
    let text_bytes = text.as_bytes();
    let suffix_bytes = suffix.as_bytes();
    if text_bytes.len() < suffix_len
        || text_bytes[text_bytes.len() - 1] != suffix_bytes[suffix_len - 1]
    {
        return false;
    }
    if suffix_len == 1 {
        return true;
    }
    let start = text_bytes.len() - suffix_len;
    if text_bytes[start] != suffix_bytes[0] {
        return false;
    }
    text_bytes[start..] == suffix_bytes[..]
}

pub(crate) struct DiagnosticAndArguments {
    message: &'static diagnostics::Message,
    arguments: Vec<DiagnosticArg>,
}

pub(crate) struct ErrorOutputContainer {
    errors: Vec<ast::Diagnostic>,
    skip_logging: bool,
}

struct ElaborationElement {
    error_node: ast::Node,
    inner_expression: Option<ast::Node>,
    name_type: TypeHandle,
    error_message: Option<&'static diagnostics::Message>,
}

pub(crate) type ContainingMessageChain<'d> = Option<&'d dyn ContainingMessageChainSource>;

pub(crate) trait ContainingMessageChainSource {
    fn prepend_to(&self, diagnostic: ast::Diagnostic) -> ast::Diagnostic;
}

pub(crate) struct SharedContainingMessageChain {
    chain: Cell<Option<ast::Diagnostic>>,
}

impl SharedContainingMessageChain {
    pub(crate) fn new(chain: ast::Diagnostic) -> Self {
        Self {
            chain: Cell::new(Some(chain)),
        }
    }

    pub(crate) fn final_diagnostic_for(&self, diagnostic: &ast::Diagnostic) -> ast::Diagnostic {
        let mut final_diagnostic = self
            .chain
            .take()
            .expect("shared containing message chain must hold diagnostic");
        final_diagnostic.set_source_from_diagnostic(diagnostic);
        final_diagnostic.set_related_info(diagnostic.related_information().to_vec());
        let result = final_diagnostic.clone();
        self.chain.set(Some(final_diagnostic));
        result
    }
}

impl ContainingMessageChainSource for SharedContainingMessageChain {
    fn prepend_to(&self, diagnostic: ast::Diagnostic) -> ast::Diagnostic {
        let chain = self
            .chain
            .take()
            .expect("shared containing message chain must hold diagnostic");
        let chain = append_diagnostic_message_chain(chain, diagnostic);
        let result = chain.clone();
        self.chain.set(Some(chain));
        result
    }
}

pub(crate) struct FreshContainingMessageChain {
    chain: ast::Diagnostic,
}

impl FreshContainingMessageChain {
    pub(crate) fn new(chain: ast::Diagnostic) -> Self {
        Self { chain }
    }
}

impl ContainingMessageChainSource for FreshContainingMessageChain {
    fn prepend_to(&self, diagnostic: ast::Diagnostic) -> ast::Diagnostic {
        append_diagnostic_message_chain(self.chain.clone(), diagnostic)
    }
}

pub(crate) type ErrorReporter<'a> =
    Box<dyn FnMut(&'static diagnostics::Message, Vec<DiagnosticArg>) + 'a>;

fn message_is(
    message: &diagnostics::Message,
    expected: &std::sync::LazyLock<diagnostics::Message>,
) -> bool {
    std::ptr::eq(message, &**expected)
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct RecursionId {
    value: RecursionIdValue,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) enum RecursionIdValue {
    Node(ast::NodeId),
    Symbol(SymbolIdentity),
    Type(TypeHandle),
}

// This function exists to constrain the types of values that can be used as recursion IDs.
pub(crate) fn as_recursion_id(value: RecursionIdValue) -> RecursionId {
    RecursionId { value }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum RelationKind {
    Subtype,
    StrictSubtype,
    Assignable,
    Comparable,
    Identity,
}

pub struct Relation {
    results: HashMap<CacheHashKey, RelationComparisonResult>,
}

impl Relation {
    pub fn new() -> Self {
        Self {
            results: HashMap::new(),
        }
    }

    pub(crate) fn get(&self, key: CacheHashKey) -> RelationComparisonResult {
        *self
            .results
            .get(&key)
            .unwrap_or(&RELATION_COMPARISON_RESULT_NONE)
    }

    pub(crate) fn set(&mut self, key: CacheHashKey, result: RelationComparisonResult) {
        self.results.insert(key, result);
    }

    pub(crate) fn size(&self) -> usize {
        self.results.len()
    }
}

enum RelationPropertyList {
    Structured(TypeHandle),
    Owned(Vec<SymbolIdentity>),
    Empty,
}

impl RelationPropertyList {
    fn len<'a, 'state>(&self, c: &Checker<'a, 'state>) -> usize {
        match self {
            RelationPropertyList::Structured(t) => c.structured_type_properties_len(*t),
            RelationPropertyList::Owned(properties) => properties.len(),
            RelationPropertyList::Empty => 0,
        }
    }

    fn get<'a, 'state>(&self, c: &Checker<'a, 'state>, index: usize) -> SymbolIdentity {
        match self {
            RelationPropertyList::Structured(t) => c.structured_type_property_at(*t, index),
            RelationPropertyList::Owned(properties) => properties[index],
            RelationPropertyList::Empty => panic!("cannot index empty relation property list"),
        }
    }

    fn to_vec<'a, 'state>(&self, c: &Checker<'a, 'state>) -> Vec<SymbolIdentity> {
        (0..self.len(c)).map(|index| self.get(c, index)).collect()
    }

    fn excluded_len<'a, 'state>(
        &self,
        c: &Checker<'a, 'state>,
        excluded_properties: &collections::Set<String>,
    ) -> usize {
        if excluded_properties.len() == 0 {
            return self.len(c);
        }
        let mut len = 0;
        for index in 0..self.len(c) {
            let property = self.get(c, index);
            if !property_identity_is_excluded(c, property, excluded_properties) {
                len += 1;
            }
        }
        len
    }
}

#[derive(Clone, Copy)]
enum RelationSignatureList {
    Structured { t: TypeHandle, kind: SignatureKind },
    Empty,
}

impl RelationSignatureList {
    fn len<'a, 'state>(&self, c: &Checker<'a, 'state>) -> usize {
        match self {
            RelationSignatureList::Structured { t, kind } => {
                c.structured_type_signatures_len(*t, *kind)
            }
            RelationSignatureList::Empty => 0,
        }
    }

    fn is_empty<'a, 'state>(&self, c: &Checker<'a, 'state>) -> bool {
        self.len(c) == 0
    }

    fn get<'a, 'state>(&self, c: &Checker<'a, 'state>, index: usize) -> SignatureHandle {
        match self {
            RelationSignatureList::Structured { t, kind } => {
                c.structured_type_signature_at(*t, *kind, index)
            }
            RelationSignatureList::Empty => panic!("cannot index empty relation signature list"),
        }
    }
}

#[derive(Clone, Copy)]
enum RelationIndexInfoList {
    Structured(TypeHandle),
    Empty,
}

impl RelationIndexInfoList {
    fn len<'a, 'state>(&self, c: &Checker<'a, 'state>) -> usize {
        match self {
            RelationIndexInfoList::Structured(t) => c.structured_type_index_infos_len(*t),
            RelationIndexInfoList::Empty => 0,
        }
    }

    fn get<'a, 'state>(&self, c: &Checker<'a, 'state>, index: usize) -> IndexInfoHandle {
        match self {
            RelationIndexInfoList::Structured(t) => c.structured_type_index_info_at(*t, index),
            RelationIndexInfoList::Empty => panic!("cannot index empty relation index info list"),
        }
    }

    fn find_by_key_type<'a, 'state>(
        &self,
        c: &Checker<'a, 'state>,
        key_type: TypeHandle,
    ) -> Option<IndexInfoHandle> {
        for index in 0..self.len(c) {
            let info = self.get(c, index);
            if c.index_info_record(info).key_type.unwrap() == key_type {
                return Some(info);
            }
        }
        None
    }
}

fn relater_get_start_element_count(
    t: &crate::semantic::TupleTypeRecord,
    flags: ElementFlags,
) -> usize {
    for (index, info) in t.element_infos.iter().enumerate() {
        if info.flags & flags == 0 {
            return index;
        }
    }
    t.element_infos.len()
}

impl<'a, 'state> Checker<'a, 'state> {
    fn relation_cache_size(&self, relation: RelationKind) -> usize {
        self.semantic_state.relation_cache_size(relation)
    }

    fn relation_result(
        &self,
        relation: RelationKind,
        key: CacheHashKey,
    ) -> RelationComparisonResult {
        self.semantic_state.relation_result(relation, key)
    }

    fn set_relation_result(
        &mut self,
        relation: RelationKind,
        key: CacheHashKey,
        result: RelationComparisonResult,
    ) {
        self.semantic_state
            .set_relation_result(relation, key, result);
    }

    pub(crate) fn is_type_identical_to(&mut self, source: TypeHandle, target: TypeHandle) -> bool {
        self.is_type_related_to(source, target, self.semantic_state.identity_relation)
    }

    pub(crate) fn compare_types_identical(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
    ) -> Ternary {
        if self.is_type_related_to(source, target, self.semantic_state.identity_relation) {
            return TERNARY_TRUE;
        }
        TERNARY_FALSE
    }

    fn compare_types_assignable_simple(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
    ) -> Ternary {
        if self.is_type_related_to(source, target, self.semantic_state.assignable_relation) {
            return TERNARY_TRUE;
        }
        TERNARY_FALSE
    }

    pub(crate) fn compare_types_assignable_worker(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        _report_errors: bool,
    ) -> Ternary {
        if self.is_type_related_to(source, target, self.semantic_state.assignable_relation) {
            return TERNARY_TRUE;
        }
        TERNARY_FALSE
    }

    fn compare_types_subtype_of(&mut self, source: TypeHandle, target: TypeHandle) -> Ternary {
        if self.is_type_related_to(source, target, self.semantic_state.subtype_relation) {
            return TERNARY_TRUE;
        }
        TERNARY_FALSE
    }

    pub(crate) fn is_type_assignable_to(&mut self, source: TypeHandle, target: TypeHandle) -> bool {
        self.is_type_related_to(source, target, self.semantic_state.assignable_relation)
    }

    pub(crate) fn is_type_subtype_of(&mut self, source: TypeHandle, target: TypeHandle) -> bool {
        self.is_type_related_to(source, target, self.semantic_state.subtype_relation)
    }

    pub(crate) fn is_type_strict_subtype_of(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
    ) -> bool {
        self.is_type_related_to(source, target, self.semantic_state.strict_subtype_relation)
    }

    pub(crate) fn is_type_comparable_to(&mut self, source: TypeHandle, target: TypeHandle) -> bool {
        self.is_type_related_to(source, target, self.semantic_state.comparable_relation)
    }

    pub(crate) fn are_types_comparable(&mut self, type1: TypeHandle, type2: TypeHandle) -> bool {
        self.is_type_comparable_to(type1, type2) || self.is_type_comparable_to(type2, type1)
    }

    pub(crate) fn is_type_related_to(
        &mut self,
        mut source: TypeHandle,
        mut target: TypeHandle,
        relation: RelationKind,
    ) -> bool {
        if self.is_fresh_literal_type(source) {
            source = self
                .type_record(source)
                .as_literal_type()
                .regular_type
                .unwrap();
        }
        if self.is_fresh_literal_type(target) {
            target = self
                .type_record(target)
                .as_literal_type()
                .regular_type
                .unwrap();
        }
        if source == target {
            return true;
        }
        if relation != self.semantic_state.identity_relation {
            if relation == self.semantic_state.comparable_relation
                && self.type_flags(target) & TYPE_FLAGS_NEVER == 0
                && self.is_simple_type_related_to(target, source, relation, None)
                || self.is_simple_type_related_to(source, target, relation, None)
            {
                return true;
            }
        } else if (self.type_flags(source) | self.type_flags(target))
            & (TYPE_FLAGS_UNION_OR_INTERSECTION
                | TYPE_FLAGS_INDEXED_ACCESS
                | TYPE_FLAGS_CONDITIONAL
                | TYPE_FLAGS_SUBSTITUTION)
            == 0
        {
            // We have excluded types that may simplify to other forms, so types must have identical flags
            if self.type_flags(source) != self.type_flags(target) {
                return false;
            }
            if self.type_flags(source) & TYPE_FLAGS_SINGLETON != 0 {
                return true;
            }
        }
        if self.type_flags(source) & TYPE_FLAGS_OBJECT != 0
            && self.type_flags(target) & TYPE_FLAGS_OBJECT != 0
        {
            let (id, _) = get_relation_key(
                self,
                source,
                target,
                INTERSECTION_STATE_NONE,
                relation == self.semantic_state.identity_relation,
                false,
            );
            let related = self.relation_result(relation, id);
            if related != RELATION_COMPARISON_RESULT_NONE {
                return related & RELATION_COMPARISON_RESULT_SUCCEEDED != 0;
            }
        }
        if self.type_flags(source) & TYPE_FLAGS_STRUCTURED_OR_INSTANTIABLE != 0
            || self.type_flags(target) & TYPE_FLAGS_STRUCTURED_OR_INSTANTIABLE != 0
        {
            return self.check_type_related_to(source, target, relation, None);
        }
        false
    }

    fn is_simple_type_related_to<'reporter>(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        relation: RelationKind,
        mut error_reporter: Option<ErrorReporter<'reporter>>,
    ) -> bool {
        let s = self.type_flags(source);
        let t = self.type_flags(target);
        if t & TYPE_FLAGS_ANY != 0
            || s & TYPE_FLAGS_NEVER != 0
            || source == self.semantic_state.semantic_handles().wildcard_type
        {
            return true;
        }
        if t & TYPE_FLAGS_UNKNOWN != 0
            && !(relation == self.semantic_state.strict_subtype_relation && s & TYPE_FLAGS_ANY != 0)
        {
            return true;
        }
        if t & TYPE_FLAGS_NEVER != 0 {
            return false;
        }
        if s & TYPE_FLAGS_STRING_LIKE != 0 && t & TYPE_FLAGS_STRING != 0 {
            return true;
        }
        if s & TYPE_FLAGS_STRING_LITERAL != 0
            && s & TYPE_FLAGS_ENUM_LITERAL != 0
            && t & TYPE_FLAGS_STRING_LITERAL != 0
            && t & TYPE_FLAGS_ENUM_LITERAL == 0
            && self.type_record(source).as_literal_type().value
                == self.type_record(target).as_literal_type().value
        {
            return true;
        }
        if s & TYPE_FLAGS_NUMBER_LIKE != 0 && t & TYPE_FLAGS_NUMBER != 0 {
            return true;
        }
        if s & TYPE_FLAGS_NUMBER_LITERAL != 0
            && s & TYPE_FLAGS_ENUM_LITERAL != 0
            && t & TYPE_FLAGS_NUMBER_LITERAL != 0
            && t & TYPE_FLAGS_ENUM_LITERAL == 0
            && self.type_record(source).as_literal_type().value
                == self.type_record(target).as_literal_type().value
        {
            return true;
        }
        if s & TYPE_FLAGS_BIG_INT_LIKE != 0 && t & TYPE_FLAGS_BIG_INT != 0 {
            return true;
        }
        if s & TYPE_FLAGS_BOOLEAN_LIKE != 0 && t & TYPE_FLAGS_BOOLEAN != 0 {
            return true;
        }
        if s & TYPE_FLAGS_ES_SYMBOL_LIKE != 0 && t & TYPE_FLAGS_ES_SYMBOL != 0 {
            return true;
        }
        if s & TYPE_FLAGS_ENUM != 0
            && t & TYPE_FLAGS_ENUM != 0
            && self
                .type_symbol_identity(source)
                .map(|symbol| self.symbol_identity_name(symbol))
                == self
                    .type_symbol_identity(target)
                    .map(|symbol| self.symbol_identity_name(symbol))
            && self.is_enum_type_related_to_types(source, target, error_reporter.as_mut())
        {
            return true;
        }
        if s & TYPE_FLAGS_ENUM_LITERAL != 0 && t & TYPE_FLAGS_ENUM_LITERAL != 0 {
            if s & TYPE_FLAGS_UNION != 0
                && t & TYPE_FLAGS_UNION != 0
                && self.is_enum_type_related_to_types(source, target, error_reporter.as_mut())
            {
                return true;
            }
            if s & TYPE_FLAGS_LITERAL != 0
                && t & TYPE_FLAGS_LITERAL != 0
                && self.type_record(source).as_literal_type().value
                    == self.type_record(target).as_literal_type().value
                && self.is_enum_type_related_to_types(source, target, error_reporter.as_mut())
            {
                return true;
            }
        }
        // In non-strictNullChecks mode, `undefined` and `null` are assignable to anything except `never`.
        // Since unions and intersections may reduce to `never`, we exclude them here.
        if s & TYPE_FLAGS_UNDEFINED != 0
            && ((!self.strict_null_checks() && t & TYPE_FLAGS_UNION_OR_INTERSECTION == 0)
                || t & (TYPE_FLAGS_UNDEFINED | TYPE_FLAGS_VOID) != 0)
        {
            return true;
        }
        if s & TYPE_FLAGS_NULL != 0
            && ((!self.strict_null_checks() && t & TYPE_FLAGS_UNION_OR_INTERSECTION == 0)
                || t & TYPE_FLAGS_NULL != 0)
        {
            return true;
        }
        if s & TYPE_FLAGS_OBJECT != 0
            && t & TYPE_FLAGS_NON_PRIMITIVE != 0
            && !(relation == self.semantic_state.strict_subtype_relation
                && self.is_empty_anonymous_object_type(source)
                && self.object_flags(source) & OBJECT_FLAGS_FRESH_LITERAL == 0)
        {
            return true;
        }
        if relation == self.semantic_state.assignable_relation
            || relation == self.semantic_state.comparable_relation
        {
            if s & TYPE_FLAGS_ANY != 0 {
                return true;
            }
            // Type number is assignable to any computed numeric enum type or any numeric enum literal type, and
            // a numeric literal type is assignable any computed numeric enum type or any numeric enum literal type
            // with a matching value. These rules exist such that enums can be used for bit-flag purposes.
            if s & TYPE_FLAGS_NUMBER != 0
                && (t & TYPE_FLAGS_ENUM != 0
                    || t & TYPE_FLAGS_NUMBER_LITERAL != 0 && t & TYPE_FLAGS_ENUM_LITERAL != 0)
            {
                return true;
            }
            if s & TYPE_FLAGS_NUMBER_LITERAL != 0
                && s & TYPE_FLAGS_ENUM_LITERAL == 0
                && (t & TYPE_FLAGS_ENUM != 0
                    || t & TYPE_FLAGS_NUMBER_LITERAL != 0
                        && t & TYPE_FLAGS_ENUM_LITERAL != 0
                        && self.type_record(source).as_literal_type().value
                            == self.type_record(target).as_literal_type().value)
            {
                return true;
            }
            // Anything is assignable to a union containing undefined, null, and {}
            if self.is_unknown_like_union_type(target) {
                return true;
            }
        }
        false
    }

    fn is_enum_type_related_to<'reporter>(
        &mut self,
        source: SymbolIdentity,
        target: SymbolIdentity,
        mut error_reporter: Option<&mut ErrorReporter<'reporter>>,
    ) -> bool {
        let source_is_member =
            self.symbol_identity_flags(source) & ast::SYMBOL_FLAGS_ENUM_MEMBER != 0;
        let source_symbol = if source_is_member {
            self.relater_get_parent_of_symbol_identity(source).unwrap()
        } else {
            source
        };
        let target_is_member =
            self.symbol_identity_flags(target) & ast::SYMBOL_FLAGS_ENUM_MEMBER != 0;
        let target_symbol = if target_is_member {
            self.relater_get_parent_of_symbol_identity(target).unwrap()
        } else {
            target
        };
        if source_symbol == target_symbol {
            return true;
        }
        let source_name = self.symbol_identity_name(source_symbol).to_string();
        let target_name = self.symbol_identity_name(target_symbol).to_string();
        let source_flags = self.symbol_identity_flags(source_symbol);
        let target_flags = self.symbol_identity_flags(target_symbol);
        if source_name != target_name
            || source_flags & ast::SYMBOL_FLAGS_REGULAR_ENUM == 0
            || target_flags & ast::SYMBOL_FLAGS_REGULAR_ENUM == 0
        {
            return false;
        }
        let key = EnumRelationKey {
            source: source_symbol,
            target: target_symbol,
        };
        if let Some(entry) = self.enum_relation_result(&key) {
            if entry != RELATION_COMPARISON_RESULT_NONE
                && !(entry & RELATION_COMPARISON_RESULT_FAILED != 0 && error_reporter.is_some())
            {
                return entry & RELATION_COMPARISON_RESULT_SUCCEEDED != 0;
            }
        }
        let target_enum_type = self.get_type_of_symbol_identity(target_symbol);
        let source_enum_type = self.get_type_of_symbol_identity(source_symbol);
        let source_properties = self.relater_get_properties_of_type_identities(source_enum_type);
        for source_property in source_properties {
            let source_property_flags = self.symbol_identity_flags(source_property);
            if source_property_flags & ast::SYMBOL_FLAGS_ENUM_MEMBER != 0 {
                let source_property_name = self.symbol_identity_name(source_property).to_string();
                let target_property = self
                    .relater_get_property_of_type_identity(target_enum_type, &source_property_name);
                if target_property.is_none()
                    || self.symbol_identity_flags(target_property.unwrap())
                        & ast::SYMBOL_FLAGS_ENUM_MEMBER
                        == 0
                {
                    if let Some(error_reporter) = error_reporter.as_mut() {
                        let source_property_text =
                            self.symbol_identity_name(source_property).to_string();
                        let target_declared_type = self.get_declared_type_of_symbol(target_symbol);
                        let target_type_text = self.type_to_string_ex(
                            target_declared_type,
                            None,
                            TYPE_FORMAT_FLAGS_USE_FULLY_QUALIFIED_TYPE,
                            None,
                        );
                        error_reporter(
                            &*diagnostics::PROPERTY_0_IS_MISSING_IN_TYPE_1,
                            vec![source_property_text.into(), target_type_text.into()],
                        );
                    }
                    self.set_enum_relation_result(key, RELATION_COMPARISON_RESULT_FAILED);
                    return false;
                }
                let target_property = target_property.unwrap();
                let source_declaration = self
                    .relater_get_declaration_of_kind_from_symbol_identity(
                        source_property,
                        ast::Kind::EnumMember,
                    )
                    .unwrap();
                let source_value = self.get_enum_member_value(source_declaration).value;
                let target_declaration = self
                    .relater_get_declaration_of_kind_from_symbol_identity(
                        target_property,
                        ast::Kind::EnumMember,
                    )
                    .unwrap();
                let target_value = self.get_enum_member_value(target_declaration).value;
                if source_value != target_value {
                    // If we have 2 enums with *known* values that differ, they are incompatible.
                    if source_value.is_some() && target_value.is_some() {
                        if let Some(error_reporter) = error_reporter.as_mut() {
                            error_reporter(
                                &*diagnostics::EACH_DECLARATION_OF_0_1_DIFFERS_IN_ITS_VALUE_WHERE_2_WAS_EXPECTED_BUT_3_WAS_GIVEN,
                                vec![
                                    target_name.clone().into(),
                                    self.symbol_identity_name(target_property)
                                        .to_string()
                                        .into(),
                                    self.value_to_string(target_value.into()).into(),
                                    self.value_to_string(source_value.into()).into(),
                                ],
                            );
                        }
                        self.set_enum_relation_result(key, RELATION_COMPARISON_RESULT_FAILED);
                        return false;
                    }
                    // At this point we know that at least one of the values is 'undefined'.
                    // This may mean that we have an opaque member from an ambient enum declaration,
                    // or that we were not able to calculate it (which is basically an error).
                    //
                    // Either way, we can assume that it's numeric.
                    // If the other is a string, we have a mismatch in types.
                    let source_is_string = source_value.is_string();
                    let target_is_string = target_value.is_string();
                    if source_is_string || target_is_string {
                        if let Some(error_reporter) = error_reporter.as_mut() {
                            let known_string_value = if source_value.is_some() {
                                source_value.clone()
                            } else {
                                target_value.clone()
                            };
                            error_reporter(
                                &*diagnostics::ONE_VALUE_OF_0_1_IS_THE_STRING_2_AND_THE_OTHER_IS_ASSUMED_TO_BE_AN_UNKNOWN_NUMERIC_VALUE,
                                vec![
                                    target_name.clone().into(),
                                    self.symbol_identity_name(target_property)
                                        .to_string()
                                        .into(),
                                    self.value_to_string(known_string_value.into()).into(),
                                ],
                            );
                        }
                        self.set_enum_relation_result(key, RELATION_COMPARISON_RESULT_FAILED);
                        return false;
                    }
                }
            }
        }
        self.set_enum_relation_result(key, RELATION_COMPARISON_RESULT_SUCCEEDED);
        true
    }

    fn is_enum_type_related_to_types<'reporter>(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        error_reporter: Option<&mut ErrorReporter<'reporter>>,
    ) -> bool {
        let Some(source_symbol) = self.type_symbol_identity(source) else {
            return false;
        };
        let Some(target_symbol) = self.type_symbol_identity(target) else {
            return false;
        };
        self.is_enum_type_related_to(source_symbol, target_symbol, error_reporter)
    }

    fn relater_get_parent_of_symbol_identity(
        &mut self,
        symbol: SymbolIdentity,
    ) -> Option<SymbolIdentity> {
        self.get_merged_symbol_identity(self.symbol_identity_parent(symbol))
    }

    fn relater_get_properties_of_type_identities(&mut self, t: TypeHandle) -> Vec<SymbolIdentity> {
        self.relater_get_properties_of_type_list(t).to_vec(self)
    }

    fn relater_get_properties_of_type_list(&mut self, mut t: TypeHandle) -> RelationPropertyList {
        t = self.get_reduced_apparent_type(t);
        if self.type_flags(t) & TYPE_FLAGS_OBJECT != 0 {
            self.resolve_structured_type_members(t);
            return RelationPropertyList::Structured(t);
        }
        RelationPropertyList::Owned(self.get_properties_of_type(t))
    }

    fn relater_get_properties_of_object_type_identities(
        &mut self,
        t: TypeHandle,
    ) -> Vec<SymbolIdentity> {
        self.relater_get_properties_of_object_type_list(t)
            .to_vec(self)
    }

    fn relater_get_properties_of_object_type_list(
        &mut self,
        t: TypeHandle,
    ) -> RelationPropertyList {
        if self.type_flags(t) & TYPE_FLAGS_OBJECT != 0 {
            self.resolve_structured_type_members(t);
            return RelationPropertyList::Structured(t);
        }
        RelationPropertyList::Empty
    }

    fn relater_get_signatures_of_type_list(
        &mut self,
        t: TypeHandle,
        kind: SignatureKind,
    ) -> RelationSignatureList {
        let reduced = self.get_reduced_apparent_type(t);
        if self.type_flags(reduced) & TYPE_FLAGS_STRUCTURED_TYPE == 0 {
            return RelationSignatureList::Empty;
        }
        self.resolve_structured_type_members(reduced);
        RelationSignatureList::Structured { t: reduced, kind }
    }

    fn relater_get_index_infos_of_type_list(&mut self, t: TypeHandle) -> RelationIndexInfoList {
        let reduced_apparent_type = self.get_reduced_apparent_type(t);
        if self.type_flags(reduced_apparent_type) & TYPE_FLAGS_STRUCTURED_TYPE == 0 {
            return RelationIndexInfoList::Empty;
        }
        self.resolve_structured_type_members(reduced_apparent_type);
        RelationIndexInfoList::Structured(reduced_apparent_type)
    }

    fn relater_get_property_of_type_identity(
        &mut self,
        t: TypeHandle,
        name: &str,
    ) -> Option<SymbolIdentity> {
        self.get_property_of_type(t, name)
    }

    fn relater_get_property_of_object_type_identity(
        &mut self,
        t: TypeHandle,
        name: &str,
    ) -> Option<SymbolIdentity> {
        if self.type_flags(t) & TYPE_FLAGS_OBJECT == 0 {
            return None;
        }
        let symbol = {
            let resolved = self.resolve_structured_type_members(t);
            resolved.members.get(name).copied()
        }?;
        self.missing_name_symbol_identity_flags(symbol)
            .intersects(ast::SYMBOL_FLAGS_VALUE)
            .then_some(symbol)
    }

    fn relater_get_declaration_of_kind_from_symbol_identity(
        &self,
        symbol: SymbolIdentity,
        kind: ast::Kind,
    ) -> Option<ast::Node> {
        self.with_symbol_identity_declarations(symbol, |declarations| {
            declarations
                .iter()
                .copied()
                .find(|declaration| self.store_for_node(*declaration).kind(*declaration) == kind)
        })
    }

    pub(crate) fn check_type_assignable_to(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        error_node: Option<ast::Node>,
        head_message: Option<&'static diagnostics::Message>,
    ) -> bool {
        self.check_type_related_to_ex(
            source,
            target,
            self.semantic_state.assignable_relation,
            error_node,
            head_message,
            None,
        )
    }

    pub(crate) fn check_type_assignable_to_ex(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        error_node: Option<ast::Node>,
        head_message: Option<&'static diagnostics::Message>,
        diagnostic_output: Option<&mut Vec<ast::Diagnostic>>,
    ) -> bool {
        self.check_type_related_to_ex(
            source,
            target,
            self.semantic_state.assignable_relation,
            error_node,
            head_message,
            diagnostic_output,
        )
    }

    pub(crate) fn check_type_comparable_to(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        error_node: Option<ast::Node>,
        head_message: Option<&'static diagnostics::Message>,
    ) -> bool {
        self.check_type_related_to_ex(
            source,
            target,
            self.semantic_state.comparable_relation,
            error_node,
            head_message,
            None,
        )
    }

    pub(crate) fn check_type_related_to(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        relation: RelationKind,
        error_node: Option<ast::Node>,
    ) -> bool {
        self.check_type_related_to_ex(source, target, relation, error_node, None, None)
    }

    // Check that source is related to target according to the given relation. When errorNode is non-nil, errors are
    // reported to the checker's diagnostic collection or through diagnosticOutput when non-nil. Callers can assume that
    // this function only reports zero or one error to diagnosticOutput (unlike checkTypeRelatedToAndOptionallyElaborate).
    pub(crate) fn check_type_related_to_ex(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        relation: RelationKind,
        mut error_node: Option<ast::Node>,
        head_message: Option<&'static diagnostics::Message>,
        diagnostic_output: Option<&mut Vec<ast::Diagnostic>>,
    ) -> bool {
        self.check_type_related_to_with_chain(
            source,
            target,
            relation,
            error_node,
            head_message,
            None,
            diagnostic_output,
        )
    }

    pub(crate) fn check_type_related_to_with_chain(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        relation: RelationKind,
        mut error_node: Option<ast::Node>,
        head_message: Option<&'static diagnostics::Message>,
        containing_message_chain: ContainingMessageChain<'_>,
        diagnostic_output: Option<&mut Vec<ast::Diagnostic>>,
    ) -> bool {
        let relation_size = self.relation_cache_size(relation);
        let mut r = self.get_relater();
        r.set_relation(relation);
        r.error_node = error_node;
        r.relation_count = (16_000_000 - relation_size as i32) / 8;
        let result = r.is_related_to_ex(
            source,
            target,
            RECURSION_FLAGS_BOTH,
            error_node.is_some(), /*reportErrors*/
            head_message,
            INTERSECTION_STATE_NONE,
        );
        if r.overflow {
            // Record this relation as having failed such that we don't attempt the overflowing operation again.
            let is_identity = relation == r.c.semantic_state.identity_relation;
            let (id, _) = get_relation_key(
                &mut *r.c,
                source,
                target,
                INTERSECTION_STATE_NONE,
                is_identity,
                false, /*ignoreConstraints*/
            );
            r.c.set_relation_result(
                relation,
                id,
                RELATION_COMPARISON_RESULT_FAILED
                    | if r.relation_count <= 0 {
                        RELATION_COMPARISON_RESULT_COMPLEXITY_OVERFLOW
                    } else {
                        RELATION_COMPARISON_RESULT_STACK_DEPTH_OVERFLOW
                    },
            );
            if let Some(tr) = r.c.tracer {
                tr.instant(
                    tracing::PHASE_CHECK_TYPES,
                    "checkTypeRelatedTo_DepthLimit",
                    tracing::args([
                        ("sourceId", serde_json::json!(r.c.type_id(source))),
                        ("targetId", serde_json::json!(r.c.type_id(target))),
                        ("depth", serde_json::json!(r.source_stack.len())),
                        ("targetDepth", serde_json::json!(r.target_stack.len())),
                    ]),
                );
            }
            let message: &'static diagnostics::Message = if r.relation_count <= 0 {
                &*diagnostics::EXCESSIVE_COMPLEXITY_COMPARING_TYPES_0_AND_1
            } else {
                &*diagnostics::EXCESSIVE_STACK_DEPTH_COMPARING_TYPES_0_AND_1
            };
            if error_node.is_none() {
                error_node = r.c.current_node();
            }
            let source_text = r.c.type_to_string(source, None);
            let target_text = r.c.type_to_string(target, None);
            let diagnostic = new_diagnostic_for_node_from_checker(
                r.c,
                error_node,
                message,
                Vec::<DiagnosticArg>::from([source_text.into(), target_text.into()]),
            );
            r.c.report_diagnostic(Some(diagnostic), diagnostic_output);
        } else if r.error_chain.is_some() {
            // Check if we should issue an extra diagnostic to produce a quickfix for a slightly incorrect import statement
            if head_message.is_some()
                && error_node.is_some()
                && result == TERNARY_FALSE
                && r.c
                    .type_symbol_identity(source)
                    .as_ref()
                    .is_some_and(|symbol| r.c.semantic_state.has_export_type_link(symbol))
            {
                let source_symbol = r.c.type_symbol_identity(source).unwrap();
                let originating_import =
                    r.c.semantic_state
                        .export_type_originating_import(source_symbol);
                let target_symbol = r.c.semantic_state.export_type_target(source_symbol);
                if originating_import.is_some()
                    && !ast::is_import_call(
                        r.c.store_for_node(originating_import.unwrap()),
                        originating_import.unwrap(),
                    )
                {
                    let retry_source = r.c.get_type_of_symbol_identity(target_symbol.unwrap());
                    let helpful_retry = r.c.check_type_related_to(
                        retry_source,
                        target,
                        relation, /*errorNode*/
                        None,
                    );
                    if helpful_retry {
                        // Likely an incorrect import. Issue a helpful diagnostic to produce a quickfix to change the import
                        let originating_import = originating_import.unwrap();
                        r.related_info.push(create_diagnostic_for_node(
                            r.c.store_for_node(originating_import),
                            originating_import,
                            &*diagnostics::TYPE_ORIGINATES_AT_THIS_IMPORT_A_NAMESPACE_STYLE_IMPORT_CANNOT_BE_CALLED_OR_CONSTRUCTED_AND_WILL_CAUSE_A_FAILURE_AT_RUNTIME_CONSIDER_USING_A_DEFAULT_IMPORT_OR_IMPORT_REQUIRE_HERE_INSTEAD,
                        ));
                    }
                }
            }
            let diagnostic = prepend_containing_message_chain(
                create_diagnostic_chain_from_error_chain(
                    Some(r.c),
                    r.c.files[0].store(),
                    &r.error_chains,
                    r.error_chain,
                    r.error_node,
                    r.related_info,
                ),
                containing_message_chain,
            );
            r.c.report_diagnostic(diagnostic, diagnostic_output);
        }
        result != TERNARY_FALSE
    }

    pub(crate) fn report_diagnostic(
        &mut self,
        diagnostic: Option<ast::Diagnostic>,
        diagnostic_output: Option<&mut Vec<ast::Diagnostic>>,
    ) {
        if let Some(diagnostic) = diagnostic {
            if let Some(diagnostic_output) = diagnostic_output {
                diagnostic_output.push(diagnostic);
            } else {
                self.diagnostics().add(diagnostic);
            }
        }
    }
}

fn new_diagnostic_for_node_from_checker(
    checker: &Checker<'_, '_>,
    node: Option<ast::Node>,
    message: &'static diagnostics::Message,
    args: Vec<DiagnosticArg>,
) -> ast::Diagnostic {
    if let Some(node) = node
        && node.store_id() == checker.factory().store().store_id()
    {
        let file = checker
            .try_source_file_for_node(node)
            .map(ast::SourceFile::diagnostic_file);
        let args = args
            .into_diagnostic_args()
            .into_iter()
            .map(|arg| Box::new(arg) as diagnostics::Argument)
            .collect::<Vec<_>>();
        return ast::new_diagnostic_with_file(
            file,
            checker.factory().store().loc(node),
            message,
            &args,
        );
    }
    if let Some(node) = node {
        return new_diagnostic_for_node(checker.store_for_node(node), Some(node), message, args);
    }
    new_diagnostic_for_node(checker.files[0].store(), None, message, args)
}

pub(crate) fn create_diagnostic_chain_from_error_chain<'a>(
    checker: Option<&Checker<'_, '_>>,
    store: &ast::AstStore,
    chains: &[ErrorChain],
    chain: Option<ErrorChainHandle>,
    error_node: Option<ast::Node>,
    related_info: Vec<ast::Diagnostic>,
) -> Option<ast::Diagnostic> {
    let mut chain = chain;
    while let Some(c) = chain {
        let record = &chains[c.0];
        if !record.message.elided_in_compatibility_pyramid() {
            break;
        }
        chain = record.next;
    }
    let chain = chain?;
    let record = &chains[chain.0];
    let next = create_diagnostic_chain_from_error_chain(
        checker,
        store,
        chains,
        record.next,
        error_node,
        related_info.clone(),
    );
    if next.is_none() {
        let mut diagnostic = if let Some(checker) = checker {
            new_diagnostic_for_node_from_checker(
                checker,
                error_node,
                record.message,
                record.args.clone(),
            )
        } else {
            new_diagnostic_for_node(store, error_node, record.message, record.args.clone())
        };
        diagnostic.set_related_info(related_info);
        return Some(diagnostic);
    }
    Some(new_diagnostic_chain_for_node(
        store,
        next,
        error_node,
        record.message,
        record.args.clone(),
    ))
}

fn append_diagnostic_message_chain(
    mut head: ast::Diagnostic,
    tail: ast::Diagnostic,
) -> ast::Diagnostic {
    head.set_source_from_diagnostic(&tail);
    let related_information = tail.related_information().to_vec();
    if head.message_chain().is_empty() {
        head.set_message_chain(vec![tail]);
    } else {
        let mut message_chain = head.message_chain().to_vec();
        let last = message_chain
            .pop()
            .expect("non-empty diagnostic chain must have a last entry");
        message_chain.push(append_diagnostic_message_chain(last, tail));
        head.set_message_chain(message_chain);
    }
    if !related_information.is_empty() {
        head.set_related_info(related_information);
    }
    head
}

fn prepend_containing_message_chain(
    diagnostic: Option<ast::Diagnostic>,
    containing_message_chain: ContainingMessageChain<'_>,
) -> Option<ast::Diagnostic> {
    match (containing_message_chain, diagnostic) {
        (Some(containing_message_chain), Some(diagnostic)) => {
            Some(containing_message_chain.prepend_to(diagnostic))
        }
        (_, diagnostic) => diagnostic,
    }
}

impl<'a, 'state> Checker<'a, 'state> {
    pub(crate) fn check_type_assignable_to_and_optionally_elaborate(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        error_node: Option<ast::Node>,
        expr: Option<ast::Node>,
        head_message: Option<&'static diagnostics::Message>,
        diagnostic_output: Option<&mut Vec<ast::Diagnostic>>,
    ) -> bool {
        self.check_type_related_to_and_optionally_elaborate(
            source,
            target,
            self.semantic_state.assignable_relation,
            error_node,
            expr,
            head_message,
            diagnostic_output,
        )
    }

    pub(crate) fn check_type_related_to_and_optionally_elaborate(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        relation: RelationKind,
        error_node: Option<ast::Node>,
        expr: Option<ast::Node>,
        head_message: Option<&'static diagnostics::Message>,
        mut diagnostic_output: Option<&mut Vec<ast::Diagnostic>>,
    ) -> bool {
        self.check_type_related_to_and_optionally_elaborate_with_chain(
            source,
            target,
            relation,
            error_node,
            expr,
            head_message,
            None,
            diagnostic_output.as_mut().map(|v| &mut **v),
        )
    }

    pub(crate) fn check_type_related_to_and_optionally_elaborate_with_chain(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        relation: RelationKind,
        error_node: Option<ast::Node>,
        expr: Option<ast::Node>,
        head_message: Option<&'static diagnostics::Message>,
        containing_message_chain: ContainingMessageChain<'_>,
        mut diagnostic_output: Option<&mut Vec<ast::Diagnostic>>,
    ) -> bool {
        if self.is_type_related_to(source, target, relation) {
            return true;
        }
        if error_node.is_some()
            && !self.elaborate_error(
                expr,
                source,
                target,
                relation,
                head_message,
                containing_message_chain,
                diagnostic_output.as_mut().map(|v| &mut **v),
            )
        {
            return self.check_type_related_to_with_chain(
                source,
                target,
                relation,
                error_node,
                head_message,
                containing_message_chain,
                diagnostic_output,
            );
        }
        false
    }

    pub(crate) fn elaborate_error(
        &mut self,
        node: Option<ast::Node>,
        source: TypeHandle,
        target: TypeHandle,
        relation: RelationKind,
        head_message: Option<&'static diagnostics::Message>,
        containing_message_chain: ContainingMessageChain<'_>,
        mut diagnostic_output: Option<&mut Vec<ast::Diagnostic>>,
    ) -> bool {
        let Some(node) = node else {
            return false;
        };
        if self.is_or_has_generic_conditional(target) {
            return false;
        }
        if !self.check_type_related_to(source, target, relation, None)
            && (self.elaborate_did_you_mean_to_call_or_construct(
                Some(node),
                source,
                target,
                relation,
                SIGNATURE_KIND_CONSTRUCT,
                head_message,
                containing_message_chain,
                diagnostic_output.as_mut().map(|v| &mut **v),
            ) || self.elaborate_did_you_mean_to_call_or_construct(
                Some(node),
                source,
                target,
                relation,
                SIGNATURE_KIND_CALL,
                head_message,
                containing_message_chain,
                diagnostic_output.as_mut().map(|v| &mut **v),
            ))
        {
            return true;
        }
        if node.store_id() == self.factory().store().store_id() {
            match self.factory().store().kind(node) {
                ast::Kind::AsExpression
                | ast::Kind::JsxExpression
                | ast::Kind::ParenthesizedExpression
                | ast::Kind::BinaryExpression
                | ast::Kind::ObjectLiteralExpression
                | ast::Kind::ArrayLiteralExpression
                | ast::Kind::ArrowFunction
                | ast::Kind::JsxAttributes => {}
                _ => return false,
            }
        }
        let store = self.store_for_node(node);
        match store.kind(node) {
            ast::Kind::AsExpression => {
                if !ast::is_const_assertion(store, node) {
                    return false;
                }
                let expression = store.expression(node);
                self.elaborate_error(
                    expression,
                    source,
                    target,
                    relation,
                    head_message,
                    containing_message_chain,
                    diagnostic_output,
                )
            }
            ast::Kind::JsxExpression | ast::Kind::ParenthesizedExpression => {
                let expression = store.expression(node);
                self.elaborate_error(
                    expression,
                    source,
                    target,
                    relation,
                    head_message,
                    containing_message_chain,
                    diagnostic_output,
                )
            }
            ast::Kind::BinaryExpression => {
                let operator_token = store.operator_token(node).unwrap();
                match store.kind(operator_token) {
                    ast::Kind::EqualsToken | ast::Kind::CommaToken => {
                        let right = store.right(node);
                        self.elaborate_error(
                            right,
                            source,
                            target,
                            relation,
                            head_message,
                            containing_message_chain,
                            diagnostic_output,
                        )
                    }
                    _ => false,
                }
            }
            ast::Kind::ObjectLiteralExpression => self.elaborate_object_literal(
                node,
                source,
                target,
                relation,
                containing_message_chain,
                diagnostic_output,
            ),
            ast::Kind::ArrayLiteralExpression => self.elaborate_array_literal(
                node,
                source,
                target,
                relation,
                containing_message_chain,
                diagnostic_output,
            ),
            ast::Kind::ArrowFunction => self.elaborate_arrow_function(
                node,
                source,
                target,
                relation,
                containing_message_chain,
                diagnostic_output,
            ),
            ast::Kind::JsxAttributes => self.elaborate_jsx_components(
                node,
                source,
                target,
                relation,
                containing_message_chain,
                diagnostic_output,
            ),
            _ => false,
        }
    }

    fn is_or_has_generic_conditional(&mut self, t: TypeHandle) -> bool {
        if self.type_flags(t) & TYPE_FLAGS_CONDITIONAL != 0 {
            return true;
        }
        if self.type_flags(t) & TYPE_FLAGS_INTERSECTION != 0 {
            let types_len = self.type_types_len(t);
            for index in 0..types_len {
                let ty = self.type_type_at(t, index);
                if self.is_or_has_generic_conditional(ty) {
                    return true;
                }
            }
        }
        false
    }

    fn elaborate_did_you_mean_to_call_or_construct(
        &mut self,
        node: Option<ast::Node>,
        source: TypeHandle,
        target: TypeHandle,
        relation: RelationKind,
        kind: SignatureKind,
        head_message: Option<&'static diagnostics::Message>,
        containing_message_chain: ContainingMessageChain<'_>,
        diagnostic_output: Option<&mut Vec<ast::Diagnostic>>,
    ) -> bool {
        if self.get_signatures_of_type(source, kind).iter().any(|s| {
            let return_type = self.get_return_type_of_signature(*s);
            self.type_flags(return_type) & (TYPE_FLAGS_ANY | TYPE_FLAGS_NEVER) == 0
                && self.check_type_related_to(
                    return_type,
                    target,
                    relation,
                    None, /*errorNode*/
                )
        }) {
            let mut diags = Vec::new();
            self.check_type_related_to_with_chain(
                source,
                target,
                relation,
                node,
                head_message,
                containing_message_chain,
                Some(&mut diags),
            );
            if !diags.is_empty() {
                let mut diagnostic = diags.remove(0);
                let message = if kind == SIGNATURE_KIND_CONSTRUCT {
                    &*diagnostics::DID_YOU_MEAN_TO_USE_NEW_WITH_THIS_EXPRESSION
                } else {
                    &*diagnostics::DID_YOU_MEAN_TO_CALL_THIS_EXPRESSION
                };
                let node = node.unwrap();
                diagnostic.add_related_info(create_diagnostic_for_node(
                    self.store_for_node(node),
                    node,
                    message,
                ));
                self.report_diagnostic(Some(diagnostic), diagnostic_output);
                return true;
            }
        }
        false
    }

    fn elaborate_object_literal(
        &mut self,
        node: ast::Node,
        source: TypeHandle,
        target: TypeHandle,
        relation: RelationKind,
        containing_message_chain: ContainingMessageChain<'_>,
        mut diagnostic_output: Option<&mut Vec<ast::Diagnostic>>,
    ) -> bool {
        if self.type_flags(target) & (TYPE_FLAGS_PRIMITIVE | TYPE_FLAGS_NEVER) != 0 {
            return false;
        }
        let mut reported_error = false;
        let store = self.store_for_node(node);
        let properties = store
            .properties(node)
            .expect("object literal expression must have properties");
        for prop in properties.iter() {
            if ast::is_spread_assignment(store, prop) {
                continue;
            }
            let prop_store = self.store_for_node(prop);
            let property_symbol = self.get_symbol_of_declaration(prop).unwrap();
            let name_type = self.relater_get_literal_type_from_property_handle(
                property_symbol,
                TYPE_FLAGS_STRING_OR_NUMBER_LITERAL_OR_UNIQUE,
                false,
            );
            if self.type_flags(name_type) & TYPE_FLAGS_NEVER != 0 {
                continue;
            }
            match prop_store.kind(prop) {
                ast::Kind::SetAccessor
                | ast::Kind::GetAccessor
                | ast::Kind::MethodDeclaration
                | ast::Kind::ShorthandPropertyAssignment => {
                    reported_error = self.elaborate_element_with_chain(
                        source,
                        target,
                        relation,
                        prop_store.name(prop).unwrap(),
                        None,
                        name_type,
                        None,
                        None,
                        containing_message_chain,
                        diagnostic_output.as_mut().map(|v| &mut **v),
                    ) || reported_error;
                }
                ast::Kind::PropertyAssignment => {
                    let prop_name = prop_store.name(prop).unwrap();
                    let message = if ast::is_computed_non_literal_name(prop_store, prop_name) {
                        Some(&*diagnostics::TYPE_OF_COMPUTED_PROPERTY_S_VALUE_IS_0_WHICH_IS_NOT_ASSIGNABLE_TO_TYPE_1)
                    } else {
                        None
                    };
                    reported_error = self.elaborate_element_with_chain(
                        source,
                        target,
                        relation,
                        prop_name,
                        prop_store.initializer(prop),
                        name_type,
                        message,
                        None,
                        containing_message_chain,
                        diagnostic_output.as_mut().map(|v| &mut **v),
                    ) || reported_error;
                }
                _ => {}
            }
        }
        reported_error
    }

    fn elaborate_array_literal(
        &mut self,
        node: ast::Node,
        mut source: TypeHandle,
        target: TypeHandle,
        relation: RelationKind,
        containing_message_chain: ContainingMessageChain<'_>,
        mut diagnostic_output: Option<&mut Vec<ast::Diagnostic>>,
    ) -> bool {
        if self.type_flags(target) & (TYPE_FLAGS_PRIMITIVE | TYPE_FLAGS_NEVER) != 0 {
            return false;
        }
        let elements = self.generate_limited_tuple_elements(node, target);
        if self.is_tuple_like_type(source) {
            return self.elaborate_elementwise(
                elements,
                source,
                target,
                relation,
                containing_message_chain,
                diagnostic_output,
            );
        }
        if !self.is_tuple_like_type(source) {
            self.push_contextual_type(node, Some(target), false /*isCache*/);
            source = self.check_array_literal(node, CHECK_MODE_CONTEXTUAL | CHECK_MODE_FORCE_TUPLE);
            self.pop_contextual_type();
            if !self.is_tuple_like_type(source) {
                return false;
            }
        }
        self.elaborate_elementwise(
            elements,
            source,
            target,
            relation,
            containing_message_chain,
            diagnostic_output,
        )
    }

    fn generate_limited_tuple_elements(
        &mut self,
        node: ast::Node,
        target: TypeHandle,
    ) -> Vec<ElaborationElement> {
        let store = self.store_for_node(node);
        let elements = store
            .elements(node)
            .expect("array literal expression must have elements");
        let mut result = Vec::new();
        for (i, element) in elements.iter().enumerate() {
            // Skip elements which do not exist in the target - a length error on the tuple overall is likely better than an error on a mismatched index signature
            if self.is_tuple_like_type(target)
                && self
                    .get_property_of_type(target, &jsnum::Number::from(i as i32).to_string())
                    .is_none()
            {
                continue;
            }
            if ast::is_omitted_expression(store, element) {
                continue;
            }
            let name_type = self.get_number_literal_type(jsnum::Number::from(i as i32));
            let check_node = self.get_effective_check_node(element);
            result.push(ElaborationElement {
                error_node: check_node,
                inner_expression: Some(check_node),
                name_type,
                error_message: None,
            });
        }
        result
    }

    fn elaborate_elementwise(
        &mut self,
        elements: Vec<ElaborationElement>,
        source: TypeHandle,
        target: TypeHandle,
        relation: RelationKind,
        containing_message_chain: ContainingMessageChain<'_>,
        mut diagnostic_output: Option<&mut Vec<ast::Diagnostic>>,
    ) -> bool {
        let mut reported_error = false;
        for element in elements {
            reported_error = self.elaborate_element_with_chain(
                source,
                target,
                relation,
                element.error_node,
                element.inner_expression,
                element.name_type,
                element.error_message,
                None,
                containing_message_chain,
                diagnostic_output.as_mut().map(|v| &mut **v),
            ) || reported_error;
        }
        reported_error
    }
}

impl<'a, 'state> Checker<'a, 'state> {
    pub(crate) fn elaborate_element(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        relation: RelationKind,
        prop: ast::Node,
        next: Option<ast::Node>,
        name_type: TypeHandle,
        error_message: Option<&'static diagnostics::Message>,
        diagnostic_factory: Option<Box<dyn Fn(ast::Node) -> ast::Diagnostic + 'a>>,
        diagnostic_output: Option<&mut Vec<ast::Diagnostic>>,
    ) -> bool {
        self.elaborate_element_with_chain(
            source,
            target,
            relation,
            prop,
            next,
            name_type,
            error_message,
            diagnostic_factory,
            None,
            diagnostic_output,
        )
    }

    pub(crate) fn elaborate_element_with_chain(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        relation: RelationKind,
        prop: ast::Node,
        next: Option<ast::Node>,
        name_type: TypeHandle,
        error_message: Option<&'static diagnostics::Message>,
        diagnostic_factory: Option<Box<dyn Fn(ast::Node) -> ast::Diagnostic + 'a>>,
        containing_message_chain: ContainingMessageChain<'_>,
        mut diagnostic_output: Option<&mut Vec<ast::Diagnostic>>,
    ) -> bool {
        let mut target_prop_type =
            self.get_best_match_indexed_access_type_or_undefined(source, target, name_type);
        if target_prop_type.is_none()
            || self.type_flags(target_prop_type.unwrap()) & TYPE_FLAGS_INDEXED_ACCESS != 0
        {
            // Don't elaborate on indexes on generic variables
            return false;
        }
        let mut source_prop_type = self.get_indexed_access_type_or_undefined(
            source,
            name_type,
            ACCESS_FLAGS_NONE,
            None,
            None,
        );
        if source_prop_type.is_none()
            || self.check_type_related_to(
                source_prop_type.unwrap(),
                target_prop_type.unwrap(),
                relation,
                None, /*errorNode*/
            )
        {
            // Don't elaborate on indexes on generic variables or when types match
            return false;
        }
        if let Some(next) = next {
            if self.elaborate_error(
                Some(next),
                source_prop_type.unwrap(),
                target_prop_type.unwrap(),
                relation,
                None, /*headMessage*/
                containing_message_chain,
                diagnostic_output.as_mut().map(|v| &mut **v),
            ) {
                return true;
            }
        }
        // Issue error on the prop itself, since the prop couldn't elaborate the error
        let mut diags = Vec::new();
        // Use the expression type, if available
        let mut specific_source = source_prop_type.unwrap();
        if let Some(next) = next {
            specific_source = self.check_expression_for_mutable_location_with_contextual_type(
                next,
                source_prop_type.unwrap(),
            );
        }
        if let Some(diagnostic_factory) = diagnostic_factory {
            // Use the custom diagnostic factory if provided (e.g., for JSX text children with dynamic error messages)
            diags.push(diagnostic_factory(prop));
        } else if self.exact_optional_property_types()
            && self.is_exact_optional_property_mismatch(Some(specific_source), target_prop_type)
        {
            let source_text = self.type_to_string(specific_source, None);
            let target_text = self.type_to_string(target_prop_type.unwrap(), None);
            diags.push(create_diagnostic_for_node_with_args(
                self.store_for_node(prop),
                prop,
                &*diagnostics::TYPE_0_IS_NOT_ASSIGNABLE_TO_TYPE_1_WITH_EXACT_OPTIONAL_PROPERTY_TYPES_COLON_TRUE_CONSIDER_ADDING_UNDEFINED_TO_THE_TYPE_OF_THE_TARGET,
                Vec::<DiagnosticArg>::from([source_text.into(), target_text.into()]),
            ));
        } else {
            let prop_name = self.get_property_name_from_index(name_type, None /*accessNode*/);
            let target_is_optional =
                self.get_property_of_type(target, &prop_name)
                    .is_some_and(|prop| {
                        self.missing_name_symbol_identity_flags(prop) & ast::SYMBOL_FLAGS_OPTIONAL
                            != 0
                    });
            let source_is_optional =
                self.get_property_of_type(source, &prop_name)
                    .is_some_and(|prop| {
                        self.missing_name_symbol_identity_flags(prop) & ast::SYMBOL_FLAGS_OPTIONAL
                            != 0
                    });
            target_prop_type =
                Some(self.remove_missing_type(target_prop_type.unwrap(), target_is_optional));
            source_prop_type = Some(self.remove_missing_type(
                source_prop_type.unwrap(),
                target_is_optional && source_is_optional,
            ));
            let result = self.check_type_related_to_with_chain(
                specific_source,
                target_prop_type.unwrap(),
                relation,
                Some(prop),
                error_message,
                containing_message_chain,
                Some(&mut diags),
            );
            if result && specific_source != source_prop_type.unwrap() {
                // If for whatever reason the expression type doesn't yield an error, make sure we still issue an error on the sourcePropType
                self.check_type_related_to_with_chain(
                    source_prop_type.unwrap(),
                    target_prop_type.unwrap(),
                    relation,
                    Some(prop),
                    error_message,
                    containing_message_chain,
                    Some(&mut diags),
                );
            }
        }
        if diags.is_empty() {
            return false;
        }
        let mut diagnostic = diags.remove(0);
        let mut property_name = String::new();
        let mut target_prop = None;
        if self.is_type_usable_as_property_name(name_type) {
            property_name = self.get_property_name_from_type(name_type);
            target_prop = self.get_property_of_type(target, &property_name);
        }
        let mut issued_elaboration = false;
        if target_prop.is_none() {
            let index_info = self.get_applicable_index_info(target, name_type);
            if index_info.is_some()
                && self
                    .index_info_record(index_info.unwrap())
                    .declaration
                    .is_some()
            {
                let declaration = self
                    .index_info_record(index_info.unwrap())
                    .declaration
                    .unwrap();
                let declaration_store = self.store_for_node(declaration);
                let source_file =
                    ast::get_source_file_of_node(declaration_store, Some(declaration)).unwrap();
                if !self.program.is_source_file_default_library(
                    declaration_store.as_source_file(source_file).path(),
                ) {
                    issued_elaboration = true;
                    diagnostic.add_related_info(create_diagnostic_for_node(
                        self.store_for_node(declaration),
                        declaration,
                        &*diagnostics::THE_EXPECTED_TYPE_COMES_FROM_THIS_INDEX_SIGNATURE,
                    ));
                }
            }
        }
        let target_node = target_prop
            .and_then(|target_prop| self.first_symbol_identity_declaration(target_prop))
            .or_else(|| {
                self.type_symbol_identity(target)
                    .and_then(|symbol| self.first_symbol_identity_declaration(symbol))
            });
        if !issued_elaboration && let Some(target_node) = target_node {
            if property_name.is_empty()
                || self.type_flags(name_type) & TYPE_FLAGS_UNIQUE_ES_SYMBOL != 0
            {
                property_name = self.type_to_string(name_type, None);
            }
            let target_node_store = self.store_for_node(target_node);
            let source_file =
                ast::get_source_file_of_node(target_node_store, Some(target_node)).unwrap();
            if !self.program.is_source_file_default_library(
                target_node_store.as_source_file(source_file).path(),
            ) {
                diagnostic.add_related_info(create_diagnostic_for_node_with_args(
                    self.store_for_node(target_node),
                    target_node,
                    &*diagnostics::THE_EXPECTED_TYPE_COMES_FROM_PROPERTY_0_WHICH_IS_DECLARED_HERE_ON_TYPE_1,
                    Vec::<DiagnosticArg>::from([
                        property_name.into(),
                        self.type_to_string(target, None).into(),
                    ]),
                ));
            }
        }
        self.report_diagnostic(Some(diagnostic), diagnostic_output);
        true
    }

    pub(crate) fn get_best_match_indexed_access_type_or_undefined(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        name_type: TypeHandle,
    ) -> Option<TypeHandle> {
        let idx = self.get_indexed_access_type_or_undefined(
            target,
            name_type,
            ACCESS_FLAGS_NONE,
            None,
            None,
        );
        if idx.is_some() {
            return idx;
        }
        if self.type_flags(target) & TYPE_FLAGS_UNION != 0 {
            let best = self.get_best_matching_type(
                source,
                target,
                Checker::compare_types_assignable_simple,
            );
            if let Some(best) = best {
                return self.get_indexed_access_type_or_undefined(
                    best,
                    name_type,
                    ACCESS_FLAGS_NONE,
                    None,
                    None,
                );
            }
        }
        None
    }

    pub(crate) fn check_expression_for_mutable_location_with_contextual_type(
        &mut self,
        next: ast::Node,
        source_prop_type: TypeHandle,
    ) -> TypeHandle {
        self.push_contextual_type(next, Some(source_prop_type), false /*isCache*/);
        let result = self.check_expression_for_mutable_location(next, CHECK_MODE_CONTEXTUAL);
        self.pop_contextual_type();
        result
    }

    fn elaborate_arrow_function(
        &mut self,
        node: ast::Node,
        source: TypeHandle,
        target: TypeHandle,
        relation: RelationKind,
        containing_message_chain: ContainingMessageChain<'_>,
        mut diagnostic_output: Option<&mut Vec<ast::Diagnostic>>,
    ) -> bool {
        // Don't elaborate blocks or functions with annotated parameter types
        let store = self.store_for_node(node);
        if ast::is_block(store, store.body(node).unwrap())
            || store
                .parameters(node)
                .unwrap()
                .iter()
                .any(|param| has_type(self.store_for_node(param), param))
        {
            return false;
        }
        let source_sig = self.get_single_call_signature(source);
        if source_sig.is_none() {
            return false;
        }
        let target_signatures = self.get_signatures_of_type(target, SIGNATURE_KIND_CALL);
        if target_signatures.is_empty() {
            return false;
        }
        let return_expression = store.body(node);
        let source_return = self.get_return_type_of_signature(source_sig.unwrap());
        let mut target_return_types = Vec::with_capacity(target_signatures.len());
        for s in target_signatures.iter() {
            target_return_types.push(self.get_return_type_of_signature(*s));
        }
        let target_return = self.get_union_type(target_return_types);
        if self.check_type_related_to(
            source_return,
            target_return,
            relation,
            None, /*errorNode*/
        ) {
            return false;
        }
        if let Some(return_expression) = return_expression {
            if self.elaborate_error(
                Some(return_expression),
                source_return,
                target_return,
                relation,
                None, /*headMessage*/
                containing_message_chain,
                diagnostic_output.as_mut().map(|v| &mut **v),
            ) {
                return true;
            }
        }
        let mut diags = Vec::new();
        self.check_type_related_to_with_chain(
            source_return,
            target_return,
            relation,
            return_expression,
            None, /*headMessage*/
            containing_message_chain,
            Some(&mut diags),
        );
        if !diags.is_empty() {
            let mut diagnostic = diags.remove(0);
            if let Some(target_symbol_declaration) = self
                .type_symbol_identity(target)
                .and_then(|symbol| self.first_symbol_identity_declaration(symbol))
            {
                diagnostic.add_related_info(create_diagnostic_for_node(
                    self.store_for_node(target_symbol_declaration),
                    target_symbol_declaration,
                    &*diagnostics::THE_EXPECTED_TYPE_COMES_FROM_THE_RETURN_TYPE_OF_THIS_SIGNATURE,
                ));
            }
            if ast::get_function_flags(store, Some(node)) & ast::FUNCTION_FLAGS_ASYNC == 0
                && self
                    .get_type_of_property_of_type(source_return, "then")
                    .is_none()
                && {
                    let promise_source_return = self.create_promise_type(source_return);
                    self.check_type_related_to(
                        promise_source_return,
                        target_return,
                        relation,
                        None, /*errorNode*/
                    )
                }
            {
                diagnostic.add_related_info(create_diagnostic_for_node(
                    self.store_for_node(node),
                    node,
                    &*diagnostics::DID_YOU_MEAN_TO_MARK_THIS_FUNCTION_AS_ASYNC,
                ));
            }
            self.report_diagnostic(Some(diagnostic), diagnostic_output);
            return true;
        }
        false
    }

    // A type is 'weak' if it is an object type with at least one optional property
    // and no required properties, call/construct signatures or index signatures
    fn is_weak_type(&mut self, t: TypeHandle) -> bool {
        if self.type_flags(t) & TYPE_FLAGS_OBJECT != 0 {
            let (signatures_empty, index_infos_empty, properties) = {
                let resolved = self.resolve_structured_type_members(t);
                (
                    resolved.signatures.is_empty(),
                    resolved.index_infos.is_empty(),
                    resolved.properties.clone(),
                )
            };
            return signatures_empty
                && index_infos_empty
                && !properties.is_empty()
                && properties
                    .iter()
                    .all(|p| self.symbol_identity_flags(*p) & ast::SYMBOL_FLAGS_OPTIONAL != 0);
        }
        if self.type_flags(t) & TYPE_FLAGS_SUBSTITUTION != 0 {
            return self.is_weak_type(
                self.type_record(t)
                    .as_substitution_type()
                    .base_type
                    .unwrap(),
            );
        }
        if self.type_flags(t) & TYPE_FLAGS_INTERSECTION != 0 {
            let types_len = self.type_types_len(t);
            for index in 0..types_len {
                let ty = self.type_type_at(t, index);
                if !self.is_weak_type(ty) {
                    return false;
                }
            }
            return true;
        }
        false
    }

    fn has_common_properties(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        is_comparing_jsx_attributes: bool,
    ) -> bool {
        for prop in self.get_properties_of_type(source) {
            let prop_name = self.missing_name_symbol_identity_name(prop);
            if self.is_known_property(target, &prop_name, is_comparing_jsx_attributes) {
                return true;
            }
        }
        false
    }

    /**
     * Check if a property with the given name is known anywhere in the given type. In an object type, a property
     * is considered known if
     * 1. the object type is empty and the check is for assignability, or
     * 2. if the object type has index signatures, or
     * 3. if the property is actually declared in the object type
     *    (this means that 'toString', for example, is not usually a known property).
     * 4. In a union or intersection type,
     *    a property is considered known if it is known in any constituent type.
     * @param targetType a type to search a given name in
     * @param name a property name to search
     * @param isComparingJsxAttributes a boolean flag indicating whether we are searching in JsxAttributesType
     */
    fn is_known_property(
        &mut self,
        target_type: TypeHandle,
        name: &str,
        is_comparing_jsx_attributes: bool,
    ) -> bool {
        if self.type_flags(target_type) & TYPE_FLAGS_OBJECT != 0 {
            // For backwards compatibility a symbol-named property is satisfied by a string index signature. This
            // is incorrect and inconsistent with element access expressions, where it is an error, so eventually
            // we should remove this exception.
            let object_property = self.get_property_of_object_type(target_type, name);
            let applicable_index_info = self.get_applicable_index_info_for_name(target_type, name);
            let string_index_info = if is_late_bound_name(name) {
                self.get_index_info_of_type(
                    target_type,
                    self.semantic_state.semantic_handles().string_type,
                )
            } else {
                None
            };
            if object_property.is_some()
                || applicable_index_info.is_some()
                || is_late_bound_name(name) && string_index_info.is_some()
                || is_comparing_jsx_attributes && is_hyphenated_jsx_name(name)
            {
                // For JSXAttributes, if the attribute has a hyphenated name, consider that the attribute to be known.
                return true;
            }
        }
        if self.type_flags(target_type) & TYPE_FLAGS_SUBSTITUTION != 0 {
            return self.is_known_property(
                self.type_record(target_type)
                    .as_substitution_type()
                    .base_type
                    .unwrap(),
                name,
                is_comparing_jsx_attributes,
            );
        }
        if self.type_flags(target_type) & TYPE_FLAGS_UNION_OR_INTERSECTION != 0
            && is_excess_property_check_target(self, target_type)
        {
            let types_len = self.type_types_len(target_type);
            for index in 0..types_len {
                let t = self.type_type_at(target_type, index);
                if self.is_known_property(t, name, is_comparing_jsx_attributes) {
                    return true;
                }
            }
        }
        false
    }

    pub(crate) fn is_deeply_nested_type(
        &mut self,
        mut t: TypeHandle,
        stack: &[TypeHandle],
        max_depth: usize,
    ) -> bool {
        if stack.len() >= max_depth {
            if self.object_flags(t) & OBJECT_FLAGS_INSTANTIATED_MAPPED
                == OBJECT_FLAGS_INSTANTIATED_MAPPED
            {
                t = self.get_mapped_target_with_symbol(t);
            }
            if self.type_flags(t) & TYPE_FLAGS_INTERSECTION != 0 {
                let types_len = self.type_types_len(t);
                for index in 0..types_len {
                    let ty = self.type_type_at(t, index);
                    if self.is_deeply_nested_type(ty, stack, max_depth) {
                        return true;
                    }
                }
            }
            let identity = get_recursion_identity(self, t);
            let mut count = 0;
            let mut last_type_id: TypeId = 0;
            for t in stack.iter() {
                if self.has_matching_recursion_identity(*t, identity.clone()) {
                    // We only count occurrences with a higher type id than the previous occurrence, since higher
                    // type ids are an indicator of newer instantiations caused by recursion.
                    let type_id = self.type_id(*t);
                    if type_id >= last_type_id {
                        count += 1;
                        if count >= max_depth {
                            return true;
                        }
                    }
                    last_type_id = type_id;
                }
            }
        }
        false
    }

    // Unwrap nested homomorphic mapped types and return the deepest target type that has a symbol. This better
    // preserves unique type identities for mapped types applied to explicitly written object literals. For example
    // in `Mapped<{ x: Mapped<{ x: Mapped<{ x: string }>}>}>`, each of the mapped type applications will have a
    // unique recursion identity (that of their target object type literal) and thus avoid appearing deeply nested.
    fn get_mapped_target_with_symbol(&mut self, mut t: TypeHandle) -> TypeHandle {
        loop {
            if self.object_flags(t) & OBJECT_FLAGS_INSTANTIATED_MAPPED
                == OBJECT_FLAGS_INSTANTIATED_MAPPED
            {
                let target = self.get_modifiers_type_from_mapped_type(t);
                if self.type_symbol_identity(target).is_some()
                    || self.type_flags(target) & TYPE_FLAGS_INTERSECTION != 0 && {
                        let types_len = self.type_types_len(target);
                        let mut has_symbol = false;
                        for index in 0..types_len {
                            let ty = self.type_type_at(target, index);
                            if self.type_symbol_identity(ty).is_some() {
                                has_symbol = true;
                                break;
                            }
                        }
                        has_symbol
                    }
                {
                    t = target;
                    continue;
                }
            }
            return t;
        }
    }

    fn has_matching_recursion_identity(
        &mut self,
        mut t: TypeHandle,
        identity: RecursionId,
    ) -> bool {
        if self.object_flags(t) & OBJECT_FLAGS_INSTANTIATED_MAPPED
            == OBJECT_FLAGS_INSTANTIATED_MAPPED
        {
            t = self.get_mapped_target_with_symbol(t);
        }
        if self.type_flags(t) & TYPE_FLAGS_INTERSECTION != 0 {
            let types_len = self.type_types_len(t);
            for index in 0..types_len {
                let ty = self.type_type_at(t, index);
                if self.has_matching_recursion_identity(ty, identity.clone()) {
                    return true;
                }
            }
            return false;
        }
        get_recursion_identity(self, t) == identity
    }

    fn get_best_matching_type(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        is_related_to: fn(&mut Checker<'a, 'state>, TypeHandle, TypeHandle) -> Ternary,
    ) -> Option<TypeHandle> {
        if let Some(t) = self.find_matching_discriminant_type(source, target, is_related_to) {
            return Some(t);
        }
        if let Some(t) = self.find_matching_type_reference_or_type_alias_reference(source, target) {
            return Some(t);
        }
        if let Some(t) = self.find_best_type_for_object_literal(source, target) {
            return Some(t);
        }
        if let Some(t) = self.find_best_type_for_invokable(source, target, SIGNATURE_KIND_CALL) {
            return Some(t);
        }
        if let Some(t) = self.find_best_type_for_invokable(source, target, SIGNATURE_KIND_CONSTRUCT)
        {
            return Some(t);
        }
        self.find_most_overlappy_type(source, target)
    }

    fn find_matching_type_reference_or_type_alias_reference(
        &mut self,
        source: TypeHandle,
        union_target: TypeHandle,
    ) -> Option<TypeHandle> {
        let source_object_flags = self.object_flags(source);
        if source_object_flags & (OBJECT_FLAGS_REFERENCE | OBJECT_FLAGS_ANONYMOUS) != 0
            && self.type_flags(union_target) & TYPE_FLAGS_UNION != 0
        {
            let target_types_len = self.type_types_len(union_target);
            for index in 0..target_types_len {
                let target = self.type_type_at(union_target, index);
                if self.type_flags(target) & TYPE_FLAGS_OBJECT != 0 {
                    let overlap_obj_flags = source_object_flags & self.object_flags(target);
                    if overlap_obj_flags & OBJECT_FLAGS_REFERENCE != 0
                        && self.type_target(source) == self.type_target(target)
                    {
                        return Some(target);
                    }
                    if overlap_obj_flags & OBJECT_FLAGS_ANONYMOUS != 0
                        && self.type_alias_record(source).is_some()
                        && self.type_alias_record(target).is_some()
                        && self.type_alias_record(source).unwrap().symbol
                            == self.type_alias_record(target).unwrap().symbol
                    {
                        return Some(target);
                    }
                }
            }
        }
        None
    }

    fn find_best_type_for_invokable(
        &mut self,
        source: TypeHandle,
        union_target: TypeHandle,
        kind: SignatureKind,
    ) -> Option<TypeHandle> {
        if !self.get_signatures_of_type(source, kind).is_empty() {
            let target_types_len = self.type_types_len(union_target);
            for index in 0..target_types_len {
                let target = self.type_type_at(union_target, index);
                if !self.get_signatures_of_type(target, kind).is_empty() {
                    return Some(target);
                }
            }
        }
        None
    }

    fn find_most_overlappy_type(
        &mut self,
        source: TypeHandle,
        union_target: TypeHandle,
    ) -> Option<TypeHandle> {
        let mut best_match = None;
        if self.type_flags(source) & (TYPE_FLAGS_PRIMITIVE | TYPE_FLAGS_INSTANTIABLE_PRIMITIVE) == 0
        {
            let mut matching_count = 0;
            let target_types_len = self.type_types_len(union_target);
            for index in 0..target_types_len {
                let target = self.type_type_at(union_target, index);
                if self.type_flags(target)
                    & (TYPE_FLAGS_PRIMITIVE | TYPE_FLAGS_INSTANTIABLE_PRIMITIVE)
                    == 0
                {
                    let source_index_type = self.get_index_type(source);
                    let target_index_type = self.get_index_type(target);
                    let overlap =
                        self.get_intersection_type(vec![source_index_type, target_index_type]);
                    if self.type_flags(overlap) & TYPE_FLAGS_INDEX != 0 {
                        // perfect overlap of keys
                        return Some(target);
                    } else if self.is_unit_type(overlap)
                        || self.type_flags(overlap) & TYPE_FLAGS_UNION != 0
                    {
                        // We only want to account for literal types otherwise.
                        // If we have a union of index types, it seems likely that we
                        // needed to elaborate between two generic mapped types anyway.
                        let mut length = 1;
                        if self.type_flags(overlap) & TYPE_FLAGS_UNION != 0 {
                            length = 0;
                            let overlap_types_len = self.type_types_len(overlap);
                            for index in 0..overlap_types_len {
                                let ty = self.type_type_at(overlap, index);
                                if self.is_unit_type(ty) {
                                    length += 1;
                                }
                            }
                        }
                        if length >= matching_count {
                            best_match = Some(target);
                            matching_count = length;
                        }
                    }
                }
            }
        }
        best_match
    }

    fn find_best_type_for_object_literal(
        &mut self,
        source: TypeHandle,
        union_target: TypeHandle,
    ) -> Option<TypeHandle> {
        if self.object_flags(source) & OBJECT_FLAGS_OBJECT_LITERAL != 0 {
            let mut has_array_like = false;
            let target_types_len = self.type_types_len(union_target);
            for index in 0..target_types_len {
                let t = self.type_type_at(union_target, index);
                if self.is_array_like_type(t) {
                    has_array_like = true;
                    break;
                }
            }
            if has_array_like {
                for index in 0..target_types_len {
                    let t = self.type_type_at(union_target, index);
                    if !self.is_array_like_type(t) {
                        return Some(t);
                    }
                }
            }
        }
        None
    }

    fn should_report_unmatched_property_error(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
    ) -> bool {
        let type_call_signatures =
            self.get_signatures_of_structured_type(source, SIGNATURE_KIND_CALL);
        let type_construct_signatures =
            self.get_signatures_of_structured_type(source, SIGNATURE_KIND_CONSTRUCT);
        let type_properties = self.get_properties_of_object_type(source);
        if (!type_call_signatures.is_empty() || !type_construct_signatures.is_empty())
            && type_properties.is_empty()
        {
            if (!self
                .get_signatures_of_type(target, SIGNATURE_KIND_CALL)
                .is_empty()
                && !type_call_signatures.is_empty())
                || !self
                    .get_signatures_of_type(target, SIGNATURE_KIND_CONSTRUCT)
                    .is_empty()
                    && !type_construct_signatures.is_empty()
            {
                // target has similar signature kinds to source, still focus on the unmatched property
                return true;
            }
            return false;
        }
        true
    }
}

pub(crate) fn is_hyphenated_jsx_name(name: &str) -> bool {
    name.contains('-')
}

pub(crate) fn is_excess_property_check_target<'a, 'state>(
    checker: &Checker<'a, 'state>,
    t: TypeHandle,
) -> bool {
    checker.type_flags(t) & TYPE_FLAGS_OBJECT != 0
        && checker.object_flags(t) & OBJECT_FLAGS_OBJECT_LITERAL_PATTERN_WITH_COMPUTED_PROPERTIES
            == 0
        || checker.type_flags(t) & TYPE_FLAGS_NON_PRIMITIVE != 0
        || checker.type_flags(t) & TYPE_FLAGS_SUBSTITUTION != 0
            && is_excess_property_check_target(
                checker,
                checker
                    .type_record(t)
                    .as_substitution_type()
                    .base_type
                    .unwrap(),
            )
        || checker.type_flags(t) & TYPE_FLAGS_UNION != 0
            && checker
                .type_types_slice(t)
                .iter()
                .copied()
                .any(|t| is_excess_property_check_target(checker, t))
        || checker.type_flags(t) & TYPE_FLAGS_INTERSECTION != 0
            && checker
                .type_types_slice(t)
                .iter()
                .copied()
                .all(|t| is_excess_property_check_target(checker, t))
}

// The recursion identity of a type is an object identity that is shared among multiple instantiations of the type.
// We track recursion identities in order to identify deeply nested and possibly infinite type instantiations with
// the same origin. For example, when type parameters are in scope in an object type such as { x: T }, all
// instantiations of that type have the same recursion identity. The default recursion identity is the object
// identity of the type, meaning that every type is unique. Generally, types with constituents that could circularly
// reference the type have a recursion identity that differs from the object identity.
pub(crate) fn get_recursion_identity<'a, 'state>(
    checker: &Checker<'a, 'state>,
    mut t: TypeHandle,
) -> RecursionId {
    // Object and array literals are known not to contain recursive references and don't need a recursion identity.
    if checker.type_flags(t) & TYPE_FLAGS_OBJECT != 0
        && !is_object_or_array_literal_type(checker, t)
    {
        if checker.object_flags(t) & OBJECT_FLAGS_REFERENCE != 0
            && checker
                .type_record(t)
                .as_type_reference()
                .unwrap()
                .node
                .is_some()
        {
            // Deferred type references are tracked through their associated AST node. This gives us finer
            // granularity than using their associated target because each manifest type reference has a
            // unique AST node.
            let node = checker
                .type_record(t)
                .as_type_reference()
                .unwrap()
                .node
                .unwrap();
            return as_recursion_id(RecursionIdValue::Node(ast::get_node_id(
                checker.store_for_node(node),
                node,
            )));
        }
        if let Some(symbol) = checker.type_symbol_identity(t) {
            if !(checker.object_flags(t) & OBJECT_FLAGS_ANONYMOUS != 0
                && checker.symbol_identity_flags(symbol) & ast::SYMBOL_FLAGS_CLASS != 0)
                && checker.object_flags(t) & OBJECT_FLAGS_FROM_TYPE_NODE == 0
            {
                // We track object types that have a symbol by that symbol (representing the origin of the type), but
                // exclude the static sides of classes (since they share their symbols with the instance sides) and type
                // references that originate in resolution of AST type nodes (since such type nodes cannot be the source
                // of generative recursion without first being instantiated).
                return as_recursion_id(RecursionIdValue::Symbol(symbol));
            }
        }
        if checker.is_tuple_type(t) && checker.object_flags(t) & OBJECT_FLAGS_FROM_TYPE_NODE == 0 {
            return as_recursion_id(RecursionIdValue::Type(checker.type_target(t)));
        }
    }
    if checker.type_flags(t) & TYPE_FLAGS_TYPE_PARAMETER != 0 {
        if let Some(symbol) = checker.type_symbol_identity(t) {
            // We use the symbol of the type parameter such that all "fresh" instantiations of that type parameter
            // have the same recursion identity.
            return as_recursion_id(RecursionIdValue::Symbol(symbol));
        }
    }
    if checker.type_flags(t) & TYPE_FLAGS_INDEXED_ACCESS != 0 {
        // Identity is the leftmost object type in a chain of indexed accesses, eg, in A[P1][P2][P3] it is A.
        t = checker
            .type_record(t)
            .as_indexed_access_type()
            .object_type
            .unwrap();
        while checker.type_flags(t) & TYPE_FLAGS_INDEXED_ACCESS != 0 {
            t = checker
                .type_record(t)
                .as_indexed_access_type()
                .object_type
                .unwrap();
        }
        return as_recursion_id(RecursionIdValue::Type(t));
    }
    if checker.type_flags(t) & TYPE_FLAGS_CONDITIONAL != 0 {
        // The root object represents the origin of the conditional type
        let root = checker.type_record(t).as_conditional_type().root.unwrap();
        let node = checker
            .semantic_state
            .conditional_root_record(root)
            .node
            .unwrap();
        return as_recursion_id(RecursionIdValue::Node(ast::get_node_id(
            checker.store_for_node(node),
            node,
        )));
    }
    as_recursion_id(RecursionIdValue::Type(t))
}

impl<'a, 'state> Checker<'a, 'state> {
    pub(crate) fn get_unmatched_property(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        require_optional_properties: bool,
        match_discriminant_properties: bool,
    ) -> Option<SymbolIdentity> {
        self.get_unmatched_properties_worker(
            source,
            target,
            require_optional_properties,
            match_discriminant_properties,
            None,
        )
    }

    fn get_unmatched_properties(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        require_optional_properties: bool,
        match_discriminant_properties: bool,
    ) -> Vec<SymbolIdentity> {
        let mut props = Vec::new();
        self.get_unmatched_properties_worker(
            source,
            target,
            require_optional_properties,
            match_discriminant_properties,
            Some(&mut props),
        );
        props
    }

    fn get_unmatched_properties_worker(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        require_optional_properties: bool,
        match_discriminant_properties: bool,
        mut props_out: Option<&mut Vec<SymbolIdentity>>,
    ) -> Option<SymbolIdentity> {
        let properties = self.relater_get_properties_of_type_identities(target);
        for target_prop in properties {
            // TODO: remove this when we support static private identifier fields and find other solutions to get privateNamesAndStaticFields test to pass
            if self.is_static_private_identifier_property_identity(target_prop) {
                continue;
            }
            let target_prop_flags = self.missing_name_symbol_identity_flags(target_prop);
            let target_prop_check_flags =
                self.missing_name_symbol_identity_check_flags(target_prop);
            if require_optional_properties
                || target_prop_flags & ast::SYMBOL_FLAGS_OPTIONAL == 0
                    && target_prop_check_flags & ast::CHECK_FLAGS_PARTIAL == 0
            {
                let target_prop_name = self.missing_name_symbol_identity_name(target_prop);
                let source_prop =
                    self.relater_get_property_of_type_identity(source, &target_prop_name);
                if source_prop.is_none() {
                    if let Some(props_out) = props_out.as_deref_mut() {
                        props_out.push(target_prop);
                    } else {
                        return Some(target_prop);
                    }
                } else if match_discriminant_properties {
                    let target_type = self.get_type_of_symbol_identity(target_prop);
                    if self.type_flags(target_type) & TYPE_FLAGS_UNIT != 0 {
                        let source_type = self.get_type_of_symbol_identity(source_prop.unwrap());
                        if !(self.type_flags(source_type) & TYPE_FLAGS_ANY != 0
                            || self.get_regular_type_of_literal_type(source_type)
                                == self.get_regular_type_of_literal_type(target_type))
                        {
                            if let Some(props_out) = props_out.as_deref_mut() {
                                props_out.push(target_prop);
                            } else {
                                return Some(target_prop);
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn is_static_private_identifier_property_identity(&self, symbol: SymbolIdentity) -> bool {
        let Some(value_declaration) = self.missing_name_symbol_identity_value_declaration(symbol)
        else {
            return false;
        };
        let store = self.store_for_node(value_declaration);
        ast::is_private_identifier_class_element_declaration(store, value_declaration)
            && ast::is_static(store, value_declaration)
    }

    // Keep this up-to-date with the same logic within `getApparentTypeOfContextualType`, since they should behave similarly
    fn find_matching_discriminant_type(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        is_related_to: fn(&mut Checker<'a, 'state>, TypeHandle, TypeHandle) -> Ternary,
    ) -> Option<TypeHandle> {
        if self.type_flags(target) & TYPE_FLAGS_UNION != 0
            && self.type_flags(source) & (TYPE_FLAGS_INTERSECTION | TYPE_FLAGS_OBJECT) != 0
        {
            if let Some(match_) = self.get_matching_union_constituent_for_type(target, source) {
                return Some(match_);
            }
            let source_properties = self.relater_get_properties_of_type_identities(source);
            let discriminant_properties =
                self.find_discriminant_properties(source_properties, target);
            if !discriminant_properties.is_empty() {
                let discriminator = TypeDiscriminator {
                    names: discriminant_properties
                        .iter()
                        .map(|prop| self.missing_name_symbol_identity_name(*prop))
                        .collect(),
                    props: discriminant_properties,
                    is_related_to,
                };
                let discriminated =
                    self.discriminate_type_by_discriminable_items(target, &discriminator);
                if discriminated != target {
                    return Some(discriminated);
                }
            }
        }
        None
    }

    fn find_discriminant_properties(
        &mut self,
        source_properties: Vec<SymbolIdentity>,
        target: TypeHandle,
    ) -> Vec<SymbolIdentity> {
        let mut result = Vec::new();
        for source_property in source_properties {
            let name = self.missing_name_symbol_identity_name(source_property);
            if self.is_discriminant_property(target, &name) {
                result.push(source_property);
            }
        }
        result
    }

    pub(crate) fn is_discriminant_property(&mut self, t: TypeHandle, name: &str) -> bool {
        if self.type_flags(t) & TYPE_FLAGS_UNION != 0 {
            let prop = self.get_union_or_intersection_property(
                t, name, false, /*skipObjectFunctionPropertyAugment*/
            );
            if let Some(prop) = prop {
                let check_flags = self.symbol_identity_check_flags(prop);
                if check_flags & ast::CHECK_FLAGS_SYNTHETIC_PROPERTY != 0 {
                    if check_flags & ast::CHECK_FLAGS_IS_DISCRIMINANT_COMPUTED == 0 {
                        self.add_transient_symbol_check_flags(
                            prop.symbol_handle(),
                            ast::CHECK_FLAGS_IS_DISCRIMINANT_COMPUTED,
                        );
                        let prop_type = self.relater_get_type_of_symbol(prop);
                        let check_flags = self.symbol_identity_check_flags(prop);
                        let is_non_uniform_and_literal = check_flags
                            & ast::CHECK_FLAGS_NON_UNIFORM_AND_LITERAL
                            == ast::CHECK_FLAGS_NON_UNIFORM_AND_LITERAL
                            && !self.is_generic_type(prop_type);
                        if is_non_uniform_and_literal {
                            self.add_transient_symbol_check_flags(
                                prop.symbol_handle(),
                                ast::CHECK_FLAGS_IS_DISCRIMINANT,
                            );
                        }
                    }
                    return self.symbol_identity_check_flags(prop)
                        & ast::CHECK_FLAGS_IS_DISCRIMINANT
                        != 0;
                }
            }
        }
        false
    }

    fn get_matching_union_constituent_for_type(
        &mut self,
        union_type: TypeHandle,
        t: TypeHandle,
    ) -> Option<TypeHandle> {
        let key_property_name = self.get_key_property_name(union_type);
        if key_property_name.is_empty() {
            return None;
        }
        let prop_type = self.get_type_of_property_of_type(t, &key_property_name)?;
        self.get_constituent_type_for_key_type(union_type, prop_type)
    }

    // Return the name of a discriminant property for which it was possible and feasible to construct a map of
    // constituent types keyed by the literal types of the property by that name in each constituent type. Return
    // an empty string if no such discriminant property exists.
    pub(crate) fn get_key_property_name(&mut self, t: TypeHandle) -> String {
        if self
            .type_record(t)
            .as_union_type()
            .key_property_name
            .is_empty()
        {
            let (key_property_name, constituent_map) = self.compute_key_property_name_and_map(t);
            let u = self
                .semantic_state
                .type_record_mut(t)
                .data
                .as_union_type_mut();
            u.key_property_name = key_property_name;
            u.constituent_map = constituent_map.unwrap_or_default();
        }
        let key_property_name = self
            .type_record(t)
            .as_union_type()
            .key_property_name
            .clone();
        if key_property_name == ast::INTERNAL_SYMBOL_NAME_MISSING {
            return String::new();
        }
        key_property_name
    }

    // Given a union type for which getKeyPropertyName returned a non-empty string, return the constituent
    // that corresponds to the given key type for that property name.
    pub(crate) fn get_constituent_type_for_key_type(
        &mut self,
        t: TypeHandle,
        key_type: TypeHandle,
    ) -> Option<TypeHandle> {
        let regular_key_type = self.get_regular_type_of_literal_type(key_type);
        let result = self
            .type_record(t)
            .as_union_type()
            .constituent_map
            .get(&regular_key_type)
            .copied();
        if result != Some(self.semantic_state.semantic_handles().unknown_type) {
            return result;
        }
        None
    }

    fn compute_key_property_name_and_map(
        &mut self,
        t: TypeHandle,
    ) -> (String, Option<HashMap<TypeHandle, TypeHandle>>) {
        let types_len = {
            let types = self.type_types_slice(t);
            if types.len() < 10 || self.object_flags(t) & OBJECT_FLAGS_PRIMITIVE_UNION != 0 {
                return (ast::INTERNAL_SYMBOL_NAME_MISSING.to_string(), None);
            }
            if types
                .iter()
                .copied()
                .filter(|ty| is_object_or_instantiable_non_primitive(self, *ty))
                .count()
                < 10
            {
                return (ast::INTERNAL_SYMBOL_NAME_MISSING.to_string(), None);
            }
            types.len()
        };
        let key_property_name = self.get_key_property_candidate_name(t, types_len);
        if key_property_name.is_empty() {
            return (ast::INTERNAL_SYMBOL_NAME_MISSING.to_string(), None);
        }
        let map_by_key_property = self.map_types_by_key_property(t, types_len, &key_property_name);
        if map_by_key_property.is_none() {
            return (ast::INTERNAL_SYMBOL_NAME_MISSING.to_string(), None);
        }
        (key_property_name, map_by_key_property)
    }

    fn get_key_property_candidate_name(
        &mut self,
        union_type: TypeHandle,
        types_len: usize,
    ) -> String {
        for index in 0..types_len {
            let t = self.type_type_at(union_type, index);
            if self.type_flags(t) & (TYPE_FLAGS_OBJECT | TYPE_FLAGS_INSTANTIABLE_NON_PRIMITIVE) != 0
            {
                for p in self.relater_get_properties_of_type_identities(t) {
                    let property_type = self.relater_get_type_of_symbol(p);
                    if is_unit_type(self, property_type) {
                        return self.missing_name_symbol_identity_name(p);
                    }
                }
            }
        }
        String::new()
    }

    // Given a set of constituent types and a property name, create and return a map keyed by the literal
    // types of the property by that name in each constituent type. No map is returned if some key property
    // has a non-literal type or if less than 10 or less than 50% of the constituents have a unique key.
    // Entries with duplicate keys have unknownType as the value.
    fn map_types_by_key_property(
        &mut self,
        union_type: TypeHandle,
        types_len: usize,
        key_property_name: &str,
    ) -> Option<HashMap<TypeHandle, TypeHandle>> {
        let mut types_by_key = HashMap::new();
        let mut count = 0;
        for index in 0..types_len {
            let t = self.type_type_at(union_type, index);
            if self.type_flags(t)
                & (TYPE_FLAGS_OBJECT
                    | TYPE_FLAGS_INTERSECTION
                    | TYPE_FLAGS_INSTANTIABLE_NON_PRIMITIVE)
                != 0
            {
                let discriminant = self.get_type_of_property_of_type(t, key_property_name);
                if discriminant.is_none() || !is_literal_type(self, discriminant.unwrap()) {
                    return None;
                }
                let mut duplicate = false;
                for d in self.distributed_types(discriminant.unwrap()) {
                    let key = self.get_regular_type_of_literal_type(d);
                    let key = key;
                    if !types_by_key.contains_key(&key) {
                        types_by_key.insert(key, t);
                    } else if types_by_key.get(&key).copied()
                        != Some(self.semantic_state.semantic_handles().unknown_type)
                    {
                        types_by_key
                            .insert(key, self.semantic_state.semantic_handles().unknown_type);
                        duplicate = true;
                    }
                }
                if !duplicate {
                    count += 1;
                }
            }
        }
        if count >= 10 && count * 2 >= types_len {
            return Some(types_by_key);
        }
        None
    }

    pub(crate) fn discriminate_type_by_discriminable_items(
        &mut self,
        target: TypeHandle,
        discriminator: &dyn Discriminator<'a, 'state>,
    ) -> TypeHandle {
        let types_len = self.type_types_len(target);
        let mut include = vec![TERNARY_FALSE; types_len];
        for i in 0..types_len {
            let t = self.type_type_at(target, i);
            let reduced = self.get_reduced_type(t);
            if self.type_flags(t) & TYPE_FLAGS_PRIMITIVE == 0
                && self.type_flags(reduced) & TYPE_FLAGS_NEVER == 0
            {
                include[i] = TERNARY_TRUE;
            }
        }
        for n in 0..discriminator.len() {
            let discriminator_name = discriminator.name(n);
            // If the remaining target types include at least one with a matching discriminant, eliminate those that
            // have non-matching discriminants. This ensures that we ignore erroneous discriminators and gradually
            // refine the target set without eliminating every constituent (which would lead to `never`).
            let mut matched = false;
            for i in 0..types_len {
                if include[i] != TERNARY_FALSE {
                    let t = self.type_type_at(target, i);
                    let target_type =
                        self.get_type_of_property_or_index_signature_of_type(t, discriminator_name);
                    if let Some(target_type) = target_type {
                        if discriminator.matches(self, n, target_type) {
                            matched = true;
                        } else {
                            include[i] = TERNARY_MAYBE;
                        }
                    }
                }
            }
            // Turn each Ternary.Maybe into Ternary.False if there was a match. Otherwise, revert to Ternary.True.
            for i in 0..types_len {
                if include[i] == TERNARY_MAYBE {
                    if matched {
                        include[i] = TERNARY_FALSE;
                    } else {
                        include[i] = TERNARY_TRUE;
                    }
                }
            }
        }
        if include.contains(&TERNARY_FALSE) {
            let mut filtered_types = Vec::new();
            for i in 0..types_len {
                if include[i] == TERNARY_TRUE {
                    let t = self.type_type_at(target, i);
                    filtered_types.push(t);
                }
            }
            let filtered = self.get_union_type_ex(filtered_types, UNION_REDUCTION_NONE, None, None);
            if self.type_flags(filtered) & TYPE_FLAGS_NEVER == 0 {
                return filtered;
            }
        }
        target
    }

    fn filter_primitives_if_contains_non_primitive(
        &mut self,
        union_type: TypeHandle,
    ) -> TypeHandle {
        if self.maybe_type_of_kind(union_type, TYPE_FLAGS_NON_PRIMITIVE) {
            let result = self.filter_type_with_checker(union_type, |checker, t| {
                checker.type_flags(t) & TYPE_FLAGS_PRIMITIVE == 0
            });
            if self.type_flags(result) & TYPE_FLAGS_NEVER == 0 {
                return result;
            }
        }
        union_type
    }

    pub(crate) fn get_type_names_for_error_display(
        &mut self,
        left: TypeHandle,
        right: TypeHandle,
    ) -> (String, String) {
        let mut left_str = if self
            .symbol_value_declaration_is_context_sensitive(self.type_symbol_identity(left))
        {
            self.type_to_string(
                left,
                self.missing_name_symbol_identity_value_declaration(
                    self.type_symbol_identity(left).unwrap(),
                ),
            )
        } else {
            self.type_to_string_public(left)
        };
        let mut right_str = if self
            .symbol_value_declaration_is_context_sensitive(self.type_symbol_identity(right))
        {
            self.type_to_string(
                right,
                self.missing_name_symbol_identity_value_declaration(
                    self.type_symbol_identity(right).unwrap(),
                ),
            )
        } else {
            self.type_to_string_public(right)
        };
        if left_str == right_str {
            left_str = self.get_type_name_for_error_display(left);
            right_str = self.get_type_name_for_error_display(right);
        }
        (left_str, right_str)
    }

    pub(crate) fn get_type_name_for_error_display(&mut self, t: TypeHandle) -> String {
        self.type_to_string_ex(
            t,
            None, /*enclosingDeclaration*/
            TYPE_FORMAT_FLAGS_USE_FULLY_QUALIFIED_TYPE,
            None,
        )
    }

    fn symbol_value_declaration_is_context_sensitive(
        &mut self,
        symbol: Option<SymbolIdentity>,
    ) -> bool {
        let value_declaration =
            symbol.and_then(|symbol| self.missing_name_symbol_identity_value_declaration(symbol));
        value_declaration.is_some()
            && ast::is_expression(
                self.store_for_node(*value_declaration.as_ref().unwrap()),
                *value_declaration.as_ref().unwrap(),
            )
            && !self.is_context_sensitive(value_declaration.unwrap())
    }

    fn type_could_have_top_level_singleton_types(&mut self, t: TypeHandle) -> bool {
        // Okay, yes, 'boolean' is a union of 'true | false', but that's not useful
        // in error reporting scenarios. If you need to use this function but that detail matters,
        // feel free to add a flag.
        if self.type_flags(t) & TYPE_FLAGS_BOOLEAN != 0 {
            return false;
        }
        if self.type_flags(t) & TYPE_FLAGS_UNION_OR_INTERSECTION != 0 {
            let types_len = self.type_types_len(t);
            for index in 0..types_len {
                let ty = self.type_type_at(t, index);
                if self.type_could_have_top_level_singleton_types(ty) {
                    return true;
                }
            }
            return false;
        }
        if self.type_flags(t) & TYPE_FLAGS_INSTANTIABLE != 0 {
            let constraint = self.get_constraint_of_type(t);
            if constraint.is_some() && constraint.unwrap() != t {
                return self.type_could_have_top_level_singleton_types(constraint.unwrap());
            }
        }
        is_unit_type(self, t)
            || self.type_flags(t) & TYPE_FLAGS_TEMPLATE_LITERAL != 0
            || self.type_flags(t) & TYPE_FLAGS_STRING_MAPPING != 0
    }
}

fn exclude_property_identities<'a, 'state>(
    checker: &Checker<'a, 'state>,
    properties: Vec<SymbolIdentity>,
    excluded_properties: collections::Set<String>,
) -> Vec<SymbolIdentity> {
    if excluded_properties.len() == 0 || properties.is_empty() {
        return properties;
    }
    let mut reduced = Vec::new();
    let mut excluded = false;
    for (i, prop) in properties.iter().enumerate() {
        if !excluded_properties.has(&checker.missing_name_symbol_identity_name(*prop)) {
            if excluded {
                reduced.push(*prop);
            }
        } else if !excluded {
            reduced = properties[..i].to_vec();
            excluded = true;
        }
    }
    if excluded {
        return reduced;
    }
    properties
}

fn property_identity_is_excluded<'a, 'state>(
    checker: &Checker<'a, 'state>,
    property: SymbolIdentity,
    excluded_properties: &collections::Set<String>,
) -> bool {
    excluded_properties.len() != 0
        && excluded_properties.has(&checker.missing_name_symbol_identity_name(property))
}

pub(crate) struct TypeDiscriminator<'a, 'state> {
    props: Vec<SymbolIdentity>,
    names: Vec<String>,
    is_related_to: fn(&mut Checker<'a, 'state>, TypeHandle, TypeHandle) -> Ternary,
}

impl<'a, 'state> TypeDiscriminator<'a, 'state> {
    fn len(&self) -> usize {
        self.props.len()
    }

    fn name(&self, index: usize) -> &str {
        &self.names[index]
    }

    fn matches(&self, checker: &mut Checker<'a, 'state>, index: usize, t: TypeHandle) -> bool {
        let prop_type = checker.get_type_of_symbol_identity(self.props[index]);
        for s in checker.distributed_types(prop_type) {
            if (self.is_related_to)(checker, s, t) != TERNARY_FALSE {
                return true;
            }
        }
        false
    }
}

pub(crate) trait Discriminator<'a, 'state> {
    fn len(&self) -> usize; // Number of discriminant properties
    fn name(&self, index: usize) -> &str; // Property name of index-th discriminator
    fn matches(&self, checker: &mut Checker<'a, 'state>, index: usize, t: TypeHandle) -> bool; // True if index-th discriminator matches the given type
}

impl<'a, 'state> Discriminator<'a, 'state> for TypeDiscriminator<'a, 'state> {
    fn len(&self) -> usize {
        TypeDiscriminator::len(self)
    }

    fn name(&self, index: usize) -> &str {
        TypeDiscriminator::name(self, index)
    }

    fn matches(&self, checker: &mut Checker<'a, 'state>, index: usize, t: TypeHandle) -> bool {
        TypeDiscriminator::matches(self, checker, index, t)
    }
}

pub(crate) fn is_object_or_instantiable_non_primitive<'a, 'state>(
    checker: &Checker<'a, 'state>,
    t: TypeHandle,
) -> bool {
    checker.type_flags(t) & (TYPE_FLAGS_OBJECT | TYPE_FLAGS_INSTANTIABLE_NON_PRIMITIVE) != 0
}

impl<'a, 'state> Checker<'a, 'state> {
    pub(crate) fn get_variances(&mut self, t: TypeHandle) -> Vec<VarianceFlags> {
        self.get_variances_state(t).into_variances_or_empty()
    }

    pub(crate) fn get_variances_state(&mut self, t: TypeHandle) -> VarianceCacheState {
        // Arrays and tuples are known to be covariant, no need to spend time computing this.
        if t == self.semantic_state.semantic_handles().global_array_type
            || t == self
                .semantic_state
                .semantic_handles()
                .global_readonly_array_type
            || self.object_flags(t) & OBJECT_FLAGS_TUPLE != 0
        {
            return VarianceCacheState::Computed(self.array_variances().to_vec());
        }
        let Some(symbol) = self.type_symbol_identity(t) else {
            return VarianceCacheState::Computed(Vec::new());
        };
        if self.type_record(t).as_interface_type().is_none() {
            return VarianceCacheState::Computed(Vec::new());
        }
        let type_parameter_count = self.interface_type_parameter_count(t);
        self.get_variances_worker(symbol, type_parameter_count, |checker, index| {
            checker.interface_type_parameter_at(t, index)
        })
    }

    pub(crate) fn get_alias_variances_identity(
        &mut self,
        symbol: SymbolIdentity,
    ) -> Vec<VarianceFlags> {
        self.get_alias_variances_identity_state(symbol)
            .into_variances_or_empty()
    }

    pub(crate) fn get_alias_variances_identity_state(
        &mut self,
        symbol: SymbolIdentity,
    ) -> VarianceCacheState {
        let type_parameters = self.semantic_state.type_alias_type_parameters(symbol);
        self.get_variances_worker(symbol, type_parameters.len(), |_, index| {
            type_parameters[index]
        })
    }

    // Return an array containing the variance of each type parameter. The variance is effectively
    // a digest of the type comparisons that occur for each type argument when instantiations of the
    // generic type are structurally compared. We infer the variance information by comparing
    // instantiations of the generic type for type arguments with known relations. The function
    // returns a computing sentinel when invoked recursively for the given generic type.
    fn get_variances_worker(
        &mut self,
        symbol: SymbolIdentity,
        type_parameter_count: usize,
        mut type_parameter_at: impl FnMut(&mut Self, usize) -> TypeHandle,
    ) -> VarianceCacheState {
        let variance_links_handle = self.semantic_state.variance_link_handle(symbol);
        if matches!(
            self.semantic_state
                .variance_cache_state_by_handle(variance_links_handle),
            VarianceCacheState::Uncomputed
        ) {
            let mut trace_args = None;
            let mut pop_fn = None;
            if let Some(tr) = self.tracer {
                let symbol_type = self.get_type_of_symbol_identity(symbol);
                let id = self.type_id(symbol_type);
                trace_args = Some(tracing::args([
                    ("arity", serde_json::json!(type_parameter_count)),
                    ("id", serde_json::json!(id)),
                ]));
                pop_fn = Some(tr.push(
                    tracing::PHASE_CHECK_TYPES,
                    "getVariancesWorker",
                    trace_args.as_ref().unwrap().clone(),
                    true,
                ));
            }
            let old_variance_computation = self.in_variance_computation();
            let save_resolution_start = self.resolution_start();
            if !self.in_variance_computation() {
                self.set_in_variance_computation(true);
                self.set_resolution_start(self.semantic_state.type_resolutions.len() as isize);
            }
            self.semantic_state
                .mark_variances_computing_by_handle(variance_links_handle);
            let mut variances = vec![VARIANCE_FLAGS_INVARIANT; type_parameter_count];
            for i in 0..type_parameter_count {
                let tp = type_parameter_at(self, i);
                let modifiers = self.get_type_parameter_modifiers(tp);
                let mut variance;
                if modifiers & ast::MODIFIER_FLAGS_OUT != 0 {
                    if modifiers & ast::MODIFIER_FLAGS_IN != 0 {
                        variance = VARIANCE_FLAGS_INVARIANT;
                    } else {
                        variance = VARIANCE_FLAGS_COVARIANT;
                    }
                } else if modifiers & ast::MODIFIER_FLAGS_IN != 0 {
                    variance = VARIANCE_FLAGS_CONTRAVARIANT;
                } else {
                    let save_reliability_flags = self.reliability_flags();
                    self.set_reliability_flags(0);
                    // We first compare instantiations where the type parameter is replaced with
                    // marker types that have a known subtype relationship. From this we can infer
                    // invariance, covariance, contravariance or bivariance.
                    let marker_super_type =
                        self.semantic_state.semantic_handles().marker_super_type;
                    let marker_sub_type = self.semantic_state.semantic_handles().marker_sub_type;
                    let marker_other_type =
                        self.semantic_state.semantic_handles().marker_other_type;
                    let type_with_super =
                        self.create_marker_type_identity(symbol, tp, marker_super_type);
                    let type_with_sub =
                        self.create_marker_type_identity(symbol, tp, marker_sub_type);
                    variance = (if self.is_type_assignable_to(type_with_sub, type_with_super) {
                        VARIANCE_FLAGS_COVARIANT
                    } else {
                        0
                    }) | (if self.is_type_assignable_to(type_with_super, type_with_sub) {
                        VARIANCE_FLAGS_CONTRAVARIANT
                    } else {
                        0
                    });
                    // If the instantiations appear to be related bivariantly it may be because the
                    // type parameter is independent (i.e. it isn't witnessed anywhere in the generic
                    // type). To determine this we compare instantiations where the type parameter is
                    // replaced with marker types that are known to be unrelated.
                    if variance == VARIANCE_FLAGS_BIVARIANT {
                        let type_with_other =
                            self.create_marker_type_identity(symbol, tp, marker_other_type);
                        if self.is_type_assignable_to(type_with_other, type_with_super) {
                            variance = VARIANCE_FLAGS_INDEPENDENT;
                        }
                    }
                    if self.reliability_flags() & RELATION_COMPARISON_RESULT_REPORTS_UNMEASURABLE
                        != 0
                    {
                        variance |= VARIANCE_FLAGS_UNMEASURABLE;
                    }
                    if self.reliability_flags() & RELATION_COMPARISON_RESULT_REPORTS_UNRELIABLE != 0
                    {
                        variance |= VARIANCE_FLAGS_UNRELIABLE;
                    }
                    self.set_reliability_flags(save_reliability_flags);
                }
                variances[i] = variance;
            }
            if !old_variance_computation {
                self.set_in_variance_computation(false);
                self.set_resolution_start(save_resolution_start);
            }
            self.semantic_state
                .set_variances_computed_by_handle(variance_links_handle, variances);
            if let Some(pop_fn) = pop_fn {
                if let Some(trace_args) = trace_args.as_mut() {
                    let variances = self
                        .semantic_state
                        .variance_cache_state_by_handle(variance_links_handle)
                        .into_variances_or_empty();
                    let formatted = variances.iter().map(|v| v.to_string()).collect::<Vec<_>>();
                    trace_args.insert("variances".to_string(), serde_json::json!(formatted));
                }
                pop_fn();
            }
        }
        self.semantic_state
            .variance_cache_state_by_handle(variance_links_handle)
    }

    fn create_marker_type_identity(
        &mut self,
        symbol: SymbolIdentity,
        source: TypeHandle,
        target: TypeHandle,
    ) -> TypeHandle {
        let symbol_handle = symbol.symbol_handle();
        self.create_marker_type_handle(symbol_handle, source, target)
    }

    fn create_marker_type_handle(
        &mut self,
        symbol: ast::SymbolHandle,
        source: TypeHandle,
        target: TypeHandle,
    ) -> TypeHandle {
        let mapper = self.new_simple_type_mapper_handle(source, target);
        let t = self.get_declared_type_of_symbol_handle(symbol);
        if self.is_error_type(t) {
            return t;
        }
        let result = if self.symbol_identity_flags(SymbolIdentity::from_symbol_handle(symbol))
            & ast::SYMBOL_FLAGS_TYPE_ALIAS
            != 0
        {
            let type_parameters = self
                .semantic_state
                .type_alias_type_parameters(SymbolIdentity::from_symbol_handle(symbol));
            let type_arguments = self.instantiate_types_with_mapper_handle(type_parameters, mapper);
            self.get_type_alias_instantiation_handle(symbol, type_arguments, None)
        } else {
            let type_arguments = self
                .instantiate_types_with_mapper_handle(self.interface_type_parameters(t), mapper);
            self.create_type_reference(t, type_arguments)
        };
        self.record_marker_type(result);
        result
    }

    fn is_marker_type(&self, t: TypeHandle) -> bool {
        self.is_marker_type_handle(t)
    }

    pub(crate) fn get_type_parameter_modifiers(&self, tp: TypeHandle) -> ast::ModifierFlags {
        let mut flags = ast::MODIFIER_FLAGS_NONE;
        if let Some(symbol) = self.type_symbol_identity(tp) {
            flags |= self.with_symbol_identity_declarations(symbol, |declarations| {
                declarations
                    .iter()
                    .fold(ast::MODIFIER_FLAGS_NONE, |flags, d| {
                        flags
                            | self
                                .store_for_node(*d)
                                .modifiers(*d)
                                .map_or(ast::MODIFIER_FLAGS_NONE, |modifiers| {
                                    modifiers.modifier_flags()
                                })
                    })
            });
        }
        flags & (ast::MODIFIER_FLAGS_IN | ast::MODIFIER_FLAGS_OUT | ast::MODIFIER_FLAGS_CONST)
    }

    // Return true if the given type reference has a 'void' type argument for a covariant type parameter.
    // See comment at call in recursiveTypeRelatedTo for when this case matters.
    fn has_covariant_void_argument(
        &self,
        type_arguments: &[TypeHandle],
        variances: &[VarianceFlags],
    ) -> bool {
        for (i, v) in variances.iter().enumerate() {
            if v & VARIANCE_FLAGS_VARIANCE_MASK == VARIANCE_FLAGS_COVARIANT
                && self.type_flags(type_arguments[i]) & TYPE_FLAGS_VOID != 0
            {
                return true;
            }
        }
        false
    }

    fn has_covariant_void_argument_for_type_reference(
        &mut self,
        t: TypeHandle,
        variances: &[VarianceFlags],
    ) -> bool {
        let type_arguments = self.ensure_type_arguments_available(t);
        for (i, v) in variances.iter().enumerate() {
            let type_argument = type_arguments
                .as_ref()
                .map_or_else(|| self.cached_type_argument_at(t, i), |args| args[i]);
            if v & VARIANCE_FLAGS_VARIANCE_MASK == VARIANCE_FLAGS_COVARIANT
                && self.type_flags(type_argument) & TYPE_FLAGS_VOID != 0
            {
                return true;
            }
        }
        false
    }

    pub(crate) fn is_signature_assignable_to(
        &mut self,
        source: SignatureHandle,
        target: SignatureHandle,
        ignore_return_types: bool,
    ) -> bool {
        let check_mode = if ignore_return_types {
            SIGNATURE_CHECK_MODE_IGNORE_RETURN_TYPES
        } else {
            SIGNATURE_CHECK_MODE_NONE
        };
        self.compare_signatures_related(
            source,
            target,
            check_mode,
            false, /*reportErrors*/
            None,  /*errorReporter*/
            self.semantic_state.compare_types_assignable,
            None, /*reportUnreliableMarkers*/
        ) != TERNARY_FALSE
    }

    fn compare_signatures_related<'reporter>(
        &mut self,
        mut source: SignatureHandle,
        mut target: SignatureHandle,
        check_mode: SignatureCheckMode,
        report_errors: bool,
        mut error_reporter: Option<ErrorReporter<'reporter>>,
        compare_types: TypeComparer,
        report_unreliable_markers: Option<TypeMapperHandle>,
    ) -> Ternary {
        if source == target {
            return TERNARY_TRUE;
        }
        if !(check_mode & SIGNATURE_CHECK_MODE_STRICT_TOP_SIGNATURE != 0
            && self.is_top_signature(source))
            && self.is_top_signature(target)
        {
            return TERNARY_TRUE;
        }
        if check_mode & SIGNATURE_CHECK_MODE_STRICT_TOP_SIGNATURE != 0
            && self.is_top_signature(source)
            && !self.is_top_signature(target)
        {
            return TERNARY_FALSE;
        }
        let target_count = self.get_parameter_count(target);
        let mut source_has_more_parameters = false;
        if !self.has_effective_rest_parameter(target) {
            if check_mode & SIGNATURE_CHECK_MODE_STRICT_ARITY != 0 {
                source_has_more_parameters = self.has_effective_rest_parameter(source)
                    || self.get_parameter_count(source) > target_count;
            } else {
                source_has_more_parameters = self.get_min_argument_count(source) > target_count;
            }
        }
        if source_has_more_parameters {
            if report_errors && (check_mode & SIGNATURE_CHECK_MODE_STRICT_ARITY == 0) {
                // the second condition should be redundant, because there is no error reporting when comparing signatures by strict arity
                // since it is only done for subtype reduction
                if let Some(error_reporter) = error_reporter.as_mut() {
                    error_reporter(&*diagnostics::TARGET_SIGNATURE_PROVIDES_TOO_FEW_ARGUMENTS_EXPECTED_0_OR_MORE_BUT_GOT_1, vec![self.get_min_argument_count(source).into(), target_count.into()]);
                }
            }
            return TERNARY_FALSE;
        }
        let source_type_parameters = self.signature_record(source).type_parameters.clone();
        let target_type_parameters = self.signature_record(target).type_parameters.clone();
        if !source_type_parameters.is_empty()
            && !core::same(&source_type_parameters, &target_type_parameters)
        {
            target = self.get_canonical_signature(target);
            source = self.instantiate_signature_in_context_of(
                source,
                target, /*inferenceContext*/
                None,
                compare_types,
            );
        }
        let source_count = self.get_parameter_count(source);
        let source_rest_type = self.get_non_array_rest_type(source);
        let target_rest_type = self.get_non_array_rest_type(target);
        if source_rest_type.is_some() || target_rest_type.is_some() {
            self.instantiate_type_with_mapper_handle(
                source_rest_type.or(target_rest_type),
                report_unreliable_markers,
            );
        }
        let mut kind = ast::Kind::Unknown;
        if let Some(declaration) = self.signature_record(target).declaration {
            kind = self.store_for_node(declaration).kind(declaration);
        }
        let strict_variance = check_mode & SIGNATURE_CHECK_MODE_CALLBACK == 0
            && self.strict_function_types()
            && kind != ast::Kind::MethodDeclaration
            && kind != ast::Kind::MethodSignature
            && kind != ast::Kind::Constructor;
        let mut result = TERNARY_TRUE;
        let source_this_type = self.get_this_type_of_signature(source);
        if source_this_type.is_some()
            && source_this_type != Some(self.semantic_state.semantic_handles().void_type)
        {
            let target_this_type = self.get_this_type_of_signature(target);
            if let Some(target_this_type) = target_this_type {
                // void sources are assignable to anything.
                let mut related = TERNARY_FALSE;
                if !strict_variance {
                    related = compare_types(
                        self,
                        source_this_type.unwrap(),
                        target_this_type,
                        false, /*reportErrors*/
                    );
                }
                if related == TERNARY_FALSE {
                    related = compare_types(
                        self,
                        target_this_type,
                        source_this_type.unwrap(),
                        report_errors,
                    );
                }
                if related == TERNARY_FALSE {
                    if report_errors {
                        if let Some(error_reporter) = error_reporter.as_mut() {
                            error_reporter(
                                &*diagnostics::THE_THIS_TYPES_OF_EACH_SIGNATURE_ARE_INCOMPATIBLE,
                                vec![],
                            );
                        }
                    }
                    return TERNARY_FALSE;
                }
                result &= related;
            }
        }
        let param_count = if source_rest_type.is_some() || target_rest_type.is_some() {
            source_count.min(target_count)
        } else {
            source_count.max(target_count)
        };
        let rest_index = if source_rest_type.is_some() || target_rest_type.is_some() {
            param_count as i32 - 1
        } else {
            -1
        };
        for i in 0..param_count {
            let source_type = if i as i32 == rest_index {
                self.get_rest_or_any_type_at_position(source, i)
            } else {
                self.try_get_type_at_position(source, i)
            };
            let target_type = if i as i32 == rest_index {
                self.get_rest_or_any_type_at_position(target, i)
            } else {
                self.try_get_type_at_position(target, i)
            };
            if source_type.is_some()
                && target_type.is_some()
                && (source_type != target_type
                    || check_mode & SIGNATURE_CHECK_MODE_STRICT_ARITY != 0)
            {
                let mut source_sig = None;
                let mut target_sig = None;
                if check_mode & SIGNATURE_CHECK_MODE_CALLBACK == 0
                    && !self.is_instantiated_generic_parameter(source, i)
                {
                    let non_nullable_source_type = self.get_non_nullable_type(source_type.unwrap());
                    source_sig = self.get_single_call_signature(non_nullable_source_type);
                }
                if check_mode & SIGNATURE_CHECK_MODE_CALLBACK == 0
                    && !self.is_instantiated_generic_parameter(target, i)
                {
                    let non_nullable_target_type = self.get_non_nullable_type(target_type.unwrap());
                    target_sig = self.get_single_call_signature(non_nullable_target_type);
                }
                let callbacks = source_sig.is_some()
                    && target_sig.is_some()
                    && self
                        .get_type_predicate_of_signature(source_sig.unwrap())
                        .is_none()
                    && self
                        .get_type_predicate_of_signature(target_sig.unwrap())
                        .is_none()
                    && self.get_type_facts(source_type.unwrap(), TYPE_FACTS_IS_UNDEFINED_OR_NULL)
                        == self
                            .get_type_facts(target_type.unwrap(), TYPE_FACTS_IS_UNDEFINED_OR_NULL);
                let mut related = TERNARY_FALSE;
                if callbacks {
                    related = self.compare_signatures_related(
                        target_sig.unwrap(),
                        source_sig.unwrap(),
                        check_mode & SIGNATURE_CHECK_MODE_STRICT_ARITY
                            | if strict_variance {
                                SIGNATURE_CHECK_MODE_STRICT_CALLBACK
                            } else {
                                SIGNATURE_CHECK_MODE_BIVARIANT_CALLBACK
                            },
                        report_errors,
                        None,
                        compare_types,
                        report_unreliable_markers,
                    );
                } else {
                    if check_mode & SIGNATURE_CHECK_MODE_CALLBACK == 0 && !strict_variance {
                        related = compare_types(
                            self,
                            source_type.unwrap(),
                            target_type.unwrap(),
                            false, /*reportErrors*/
                        );
                    }
                    if related == TERNARY_FALSE {
                        related = compare_types(
                            self,
                            target_type.unwrap(),
                            source_type.unwrap(),
                            report_errors,
                        );
                    }
                }
                // With strict arity, (x: number | undefined) => void is a subtype of (x?: number | undefined) => void
                if related != TERNARY_FALSE
                    && check_mode & SIGNATURE_CHECK_MODE_STRICT_ARITY != 0
                    && i >= self.get_min_argument_count(source)
                    && i < self.get_min_argument_count(target)
                    && compare_types(
                        self,
                        source_type.unwrap(),
                        target_type.unwrap(),
                        false, /*reportErrors*/
                    ) != TERNARY_FALSE
                {
                    related = TERNARY_FALSE;
                }
                if related == TERNARY_FALSE {
                    if report_errors {
                        if let Some(error_reporter) = error_reporter.as_mut() {
                            error_reporter(
                                &*diagnostics::TYPES_OF_PARAMETERS_0_AND_1_ARE_INCOMPATIBLE,
                                vec![
                                    self.get_parameter_name_at_position(source, i).into(),
                                    self.get_parameter_name_at_position(target, i).into(),
                                ],
                            );
                        }
                    }
                    return TERNARY_FALSE;
                }
                result &= related;
            }
        }
        if check_mode & SIGNATURE_CHECK_MODE_IGNORE_RETURN_TYPES == 0 {
            // If a signature resolution is already in-flight, skip issuing a circularity error
            // here and just use the `any` type directly
            let target_return_type = self.get_non_circular_return_type_of_signature(target);
            if target_return_type == self.semantic_state.semantic_handles().void_type
                || target_return_type == self.semantic_state.semantic_handles().any_type
            {
                return result;
            }
            let source_return_type = self.get_non_circular_return_type_of_signature(source);
            // The following block preserves behavior forbidding boolean returning functions from being assignable to type guard returning functions
            let target_type_predicate = self.get_type_predicate_of_signature(target);
            if let Some(target_type_predicate) = target_type_predicate {
                let source_type_predicate = self.get_type_predicate_of_signature(source);
                if let Some(source_type_predicate) = source_type_predicate {
                    result &= self.compare_type_predicate_related_to(
                        source_type_predicate,
                        target_type_predicate,
                        report_errors,
                        error_reporter,
                        compare_types,
                    );
                } else if {
                    let target_record = self.type_predicate_record(target_type_predicate);
                    target_record.kind == TYPE_PREDICATE_KIND_IDENTIFIER
                        || target_record.kind == TYPE_PREDICATE_KIND_THIS
                } {
                    if report_errors {
                        if let Some(mut error_reporter) = error_reporter {
                            error_reporter(
                                &*diagnostics::SIGNATURE_0_MUST_BE_A_TYPE_PREDICATE,
                                vec![self.signature_to_string(source).into()],
                            );
                        }
                    }
                    return TERNARY_FALSE;
                }
            } else {
                let mut related = TERNARY_FALSE;
                if check_mode & SIGNATURE_CHECK_MODE_BIVARIANT_CALLBACK != 0 {
                    related = compare_types(
                        self,
                        target_return_type,
                        source_return_type,
                        false, /*reportErrors*/
                    );
                }
                if related == TERNARY_FALSE {
                    related =
                        compare_types(self, source_return_type, target_return_type, report_errors);
                }
                result &= related;
                if result == TERNARY_FALSE && report_errors {
                    let source_record = self.signature_record(source);
                    let target_record = self.signature_record(target);
                    let message = if source_record.parameters.is_empty()
                        && target_record.parameters.is_empty()
                    {
                        if source_record.flags & SIGNATURE_FLAGS_CONSTRUCT != 0 {
                            &*diagnostics::CONSTRUCT_SIGNATURES_WITH_NO_ARGUMENTS_HAVE_INCOMPATIBLE_RETURN_TYPES_0_AND_1
                        } else {
                            &*diagnostics::CALL_SIGNATURES_WITH_NO_ARGUMENTS_HAVE_INCOMPATIBLE_RETURN_TYPES_0_AND_1
                        }
                    } else if source_record.flags & SIGNATURE_FLAGS_CONSTRUCT != 0 {
                        &*diagnostics::CONSTRUCT_SIGNATURE_RETURN_TYPES_0_AND_1_ARE_INCOMPATIBLE
                    } else {
                        &*diagnostics::CALL_SIGNATURE_RETURN_TYPES_0_AND_1_ARE_INCOMPATIBLE
                    };
                    if let Some(mut error_reporter) = error_reporter {
                        error_reporter(
                            message,
                            vec![
                                self.type_to_string(source_return_type, None).into(),
                                self.type_to_string(target_return_type, None).into(),
                            ],
                        );
                    }
                }
            }
        }
        result
    }

    fn compare_type_predicate_related_to<'reporter>(
        &mut self,
        source: TypePredicateHandle,
        target: TypePredicateHandle,
        report_errors: bool,
        mut error_reporter: Option<ErrorReporter<'reporter>>,
        compare_types: TypeComparer,
    ) -> Ternary {
        let source_record = self.type_predicate_record(source).clone();
        let target_record = self.type_predicate_record(target).clone();
        if source_record.kind != target_record.kind {
            if report_errors {
                if let Some(error_reporter) = error_reporter.as_mut() {
                    error_reporter(&*diagnostics::A_THIS_BASED_TYPE_GUARD_IS_NOT_COMPATIBLE_WITH_A_PARAMETER_BASED_TYPE_GUARD, vec![]);
                    error_reporter(
                        &*diagnostics::TYPE_PREDICATE_0_IS_NOT_ASSIGNABLE_TO_1,
                        vec![
                            self.type_predicate_to_string(source).into(),
                            self.type_predicate_to_string(target).into(),
                        ],
                    );
                }
            }
            return TERNARY_FALSE;
        }
        if source_record.kind == TYPE_PREDICATE_KIND_IDENTIFIER
            || source_record.kind == TYPE_PREDICATE_KIND_ASSERTS_IDENTIFIER
        {
            if source_record.parameter_index != target_record.parameter_index {
                if report_errors {
                    if let Some(error_reporter) = error_reporter.as_mut() {
                        error_reporter(
                            &*diagnostics::PARAMETER_0_IS_NOT_IN_THE_SAME_POSITION_AS_PARAMETER_1,
                            vec![
                                source_record.parameter_name.clone().into(),
                                target_record.parameter_name.clone().into(),
                            ],
                        );
                        error_reporter(
                            &*diagnostics::TYPE_PREDICATE_0_IS_NOT_ASSIGNABLE_TO_1,
                            vec![
                                self.type_predicate_to_string(source).into(),
                                self.type_predicate_to_string(target).into(),
                            ],
                        );
                    }
                }
                return TERNARY_FALSE;
            }
        }
        let related = match (source_record.t, target_record.t) {
            (s, t) if s == t => TERNARY_TRUE,
            (Some(s), Some(t)) => compare_types(self, s, t, report_errors),
            _ => TERNARY_FALSE,
        };
        if related == TERNARY_FALSE && report_errors {
            if let Some(error_reporter) = error_reporter.as_mut() {
                error_reporter(
                    &*diagnostics::TYPE_PREDICATE_0_IS_NOT_ASSIGNABLE_TO_1,
                    vec![
                        self.type_predicate_to_string(source).into(),
                        self.type_predicate_to_string(target).into(),
                    ],
                );
            }
        }
        related
    }

    // Returns true if `s` is `(...args: A) => R` where `A` is `any`, `any[]`, `never`, or `never[]`, and `R` is `any` or `unknown`.
    fn is_top_signature(&mut self, s: SignatureHandle) -> bool {
        let signature = self.signature_record(s).clone();
        let this_parameter_type = self
            .signature_this_parameter_identity(s)
            .map(|this_parameter| self.relater_get_type_of_parameter_identity(this_parameter));
        if signature.type_parameters.is_empty()
            && (signature.this_parameter.is_none() || is_type_any(self, this_parameter_type))
            && signature.parameters.len() == 1
            && self.signature_has_rest_parameter(s)
        {
            let param_type = self.relater_get_type_of_parameter_identity(
                self.signature_parameter_identity(s, 0).unwrap(),
            );
            let rest_type = if self.is_array_type(param_type) {
                self.type_argument_at(param_type, 0)
            } else {
                param_type
            };
            let return_type = self.get_return_type_of_signature(s);
            return self.type_flags(rest_type) & (TYPE_FLAGS_ANY | TYPE_FLAGS_NEVER) != 0
                && self.type_flags(return_type) & TYPE_FLAGS_ANY_OR_UNKNOWN != 0;
        }
        false
    }

    // Return the number of parameters in a signature. The rest parameter, if present, counts as one
    // parameter. For example, the parameter count of (x: number, y: number, ...z: string[]) is 3 and
    // the parameter count of (x: number, ...args: [number...string[], boolean])) is also 3. In the
    // latter example, the effective rest type is [...string[], boolean].
    pub(crate) fn get_parameter_count(&mut self, signature: SignatureHandle) -> usize {
        let parameters = self.signature_parameter_identities(signature);
        let length = parameters.len();
        if self.signature_has_rest_parameter(signature) {
            let rest_type = self.get_type_of_symbol_identity(parameters[length - 1]);
            if self.is_tuple_type(rest_type) {
                let tuple = self.target_tuple_type_record(rest_type);
                return length + tuple.fixed_length
                    - if tuple.combined_flags & ELEMENT_FLAGS_VARIABLE != 0 {
                        0
                    } else {
                        1
                    };
            }
        }
        length
    }

    pub(crate) fn get_min_argument_count(&mut self, signature: SignatureHandle) -> usize {
        self.get_min_argument_count_ex(signature, MIN_ARGUMENT_COUNT_FLAGS_NONE)
    }

    pub(crate) fn get_min_argument_count_ex(
        &mut self,
        signature: SignatureHandle,
        flags: MinArgumentCountFlags,
    ) -> usize {
        let strong_arity_for_untyped_js =
            flags & MIN_ARGUMENT_COUNT_FLAGS_STRONG_ARITY_FOR_UNTYPED_JS;
        let void_is_non_optional = flags & MIN_ARGUMENT_COUNT_FLAGS_VOID_IS_NON_OPTIONAL;
        if void_is_non_optional != 0
            || self.signature_record(signature).resolved_min_argument_count == -1
        {
            let mut min_argument_count = -1;
            let parameters = self.signature_parameter_identities(signature);
            if self.signature_has_rest_parameter(signature) {
                let rest_type = self.get_type_of_symbol_identity(parameters[parameters.len() - 1]);
                if self.is_tuple_type(rest_type) {
                    let tuple = self.target_tuple_type_record(rest_type);
                    let first_optional_index = tuple
                        .element_infos
                        .iter()
                        .position(|info| info.flags & ELEMENT_FLAGS_REQUIRED == 0)
                        .map(|i| i as i32)
                        .unwrap_or(-1);
                    let mut required_count = first_optional_index;
                    if first_optional_index < 0 {
                        required_count = tuple.fixed_length as i32;
                    }
                    if required_count > 0 {
                        min_argument_count = parameters.len() as i32 - 1 + required_count;
                    }
                }
            }
            if min_argument_count == -1 {
                if strong_arity_for_untyped_js == 0
                    && self.signature_record(signature).flags
                        & SIGNATURE_FLAGS_IS_UNTYPED_SIGNATURE_IN_JS_FILE
                        != 0
                {
                    return 0;
                }
                min_argument_count = self.signature_record(signature).min_argument_count;
            }
            if void_is_non_optional != 0 {
                return min_argument_count as usize;
            }
            for i in (0..min_argument_count).rev() {
                let t = self.get_type_at_position(signature, i as usize);
                if !some_type(self, t, |checker, t| {
                    checker.type_flags(t) & TYPE_FLAGS_VOID != 0
                }) {
                    break;
                }
                min_argument_count = i;
            }
            self.semantic_state
                .signature_record_mut(signature)
                .resolved_min_argument_count = min_argument_count;
        }
        self.signature_record(signature).resolved_min_argument_count as usize
    }

    pub(crate) fn has_effective_rest_parameter(&mut self, signature: SignatureHandle) -> bool {
        if self.signature_has_rest_parameter(signature) {
            let parameters = self.signature_parameter_identities(signature);
            let rest_type = self.get_type_of_symbol_identity(parameters[parameters.len() - 1]);
            return !self.is_tuple_type(rest_type)
                || self.target_tuple_type_record(rest_type).combined_flags
                    & ELEMENT_FLAGS_VARIABLE
                    != 0;
        }
        false
    }

    pub fn get_type_at_position(&mut self, signature: SignatureHandle, pos: usize) -> TypeHandle {
        if let Some(t) = self.try_get_type_at_position(signature, pos) {
            return t;
        }
        self.semantic_state.semantic_handles().any_type
    }

    pub(crate) fn try_get_type_at_position(
        &mut self,
        signature: SignatureHandle,
        pos: usize,
    ) -> Option<TypeHandle> {
        let parameters = self.signature_parameter_identities(signature);
        let param_count = parameters.len()
            - if self.signature_has_rest_parameter(signature) {
                1
            } else {
                0
            };
        if pos < param_count {
            return Some(self.relater_get_type_of_parameter_identity(parameters[pos]));
        }
        if self.signature_has_rest_parameter(signature) {
            // We want to return the value undefined for an out of bounds parameter position,
            // so we need to check bounds here before calling getIndexedAccessType (which
            // otherwise would return the type 'undefined').
            let rest_type = self.get_type_of_symbol_identity(parameters[param_count]);
            let index = pos - param_count;
            if !self.is_tuple_type(rest_type)
                || self.target_tuple_type_record(rest_type).combined_flags & ELEMENT_FLAGS_VARIABLE
                    != 0
                || index < self.target_tuple_type_record(rest_type).fixed_length
            {
                let index_type = self.get_number_literal_type(jsnum::Number::from(index as i32));
                return Some(self.get_indexed_access_type(rest_type, index_type));
            }
        }
        None
    }

    // Return the rest type at the given position, transforming `any[]` into just `any`. We do this because
    // in signatures we want `any[]` in a rest position to be compatible with anything, but `any[]` isn't
    // assignable to tuple types with required elements.
    fn get_rest_or_any_type_at_position(
        &mut self,
        source: SignatureHandle,
        pos: usize,
    ) -> Option<TypeHandle> {
        let rest_type = self.get_rest_type_at_position(source, pos, false);
        if let Some(rest_type) = rest_type {
            if let Some(element_type) = self.get_element_type_of_array_type(rest_type) {
                if is_type_any(self, Some(element_type)) {
                    return Some(self.semantic_state.semantic_handles().any_type);
                }
            }
        }
        rest_type
    }

    pub(crate) fn get_rest_type_at_position(
        &mut self,
        source: SignatureHandle,
        pos: usize,
        readonly: bool,
    ) -> Option<TypeHandle> {
        let parameter_count = self.get_parameter_count(source);
        let min_argument_count = self.get_min_argument_count(source);
        let rest_type = self.get_effective_rest_type(source);
        if let Some(rest_type) = rest_type {
            if pos >= parameter_count - 1 {
                if pos == parameter_count - 1 {
                    return Some(rest_type);
                } else {
                    let indexed_rest_type = self.get_indexed_access_type(
                        rest_type,
                        self.semantic_state.semantic_handles().number_type,
                    );
                    return Some(self.create_array_type(indexed_rest_type));
                }
            }
        }
        let mut types =
            vec![self.semantic_state.semantic_handles().any_type; parameter_count - pos];
        let mut infos = vec![TupleElementInfo::default(); parameter_count - pos];
        for i in 0..types.len() {
            let flags;
            if rest_type.is_none() || i < types.len() - 1 {
                types[i] = self.get_type_at_position(source, i + pos);
                flags = if i + pos < min_argument_count {
                    ELEMENT_FLAGS_REQUIRED
                } else {
                    ELEMENT_FLAGS_OPTIONAL
                };
            } else {
                types[i] = rest_type.unwrap();
                flags = ELEMENT_FLAGS_VARIADIC;
            }
            let labeled_declaration = self.get_nameable_declaration_at_position(source, i + pos);
            infos[i] = TupleElementInfo {
                flags,
                labeled_declaration,
            };
        }
        Some(self.create_tuple_type_ex(types, infos, readonly))
    }

    fn get_nameable_declaration_at_position(
        &mut self,
        signature: SignatureHandle,
        pos: usize,
    ) -> Option<ast::Node> {
        let parameters = self.signature_parameter_identities(signature);
        let param_count = parameters.len()
            - if self.signature_has_rest_parameter(signature) {
                1
            } else {
                0
            };
        if pos < param_count {
            let decl = self.missing_name_symbol_identity_value_declaration(parameters[pos]);
            if decl.is_some() && self.is_valid_declaration_for_tuple_label(decl.unwrap()) {
                return decl;
            }
            return None;
        }
        if self.signature_has_rest_parameter(signature) {
            let rest_parameter = parameters[param_count];
            let rest_type = self.get_type_of_symbol_identity(rest_parameter);
            if self.is_tuple_type(rest_type) {
                let element_infos = &self.target_tuple_type_record(rest_type).element_infos;
                let index = pos - param_count;
                if index < element_infos.len() {
                    return element_infos[index].labeled_declaration;
                }
                return None;
            }
            let rest_parameter_value_declaration =
                self.missing_name_symbol_identity_value_declaration(rest_parameter);
            if rest_parameter_value_declaration.is_some()
                && self.is_valid_declaration_for_tuple_label(
                    rest_parameter_value_declaration.clone().unwrap(),
                )
            {
                return rest_parameter_value_declaration;
            }
        }
        None
    }

    fn is_valid_declaration_for_tuple_label(&self, d: ast::Node) -> bool {
        let store = self.store_for_node(d);
        ast::is_named_tuple_member(store, d)
            || ast::is_parameter_declaration(store, d)
                && store.name(d).is_some()
                && ast::is_identifier(store, store.name(d).unwrap())
    }

    pub(crate) fn get_non_array_rest_type(
        &mut self,
        signature: SignatureHandle,
    ) -> Option<TypeHandle> {
        let rest_type = self.get_effective_rest_type(signature);
        if rest_type.is_some()
            && !self.is_array_type(rest_type.unwrap())
            && !is_type_any(self, rest_type)
        {
            return rest_type;
        }
        None
    }

    pub(crate) fn get_effective_rest_type(
        &mut self,
        signature: SignatureHandle,
    ) -> Option<TypeHandle> {
        if self.signature_has_rest_parameter(signature) {
            let parameters = self.signature_parameter_identities(signature);
            let rest_type = self.get_type_of_symbol_identity(parameters[parameters.len() - 1]);
            if !self.is_tuple_type(rest_type) {
                if is_type_any(self, Some(rest_type)) {
                    return Some(self.semantic_state.semantic_handles().any_array_type);
                }
                return Some(rest_type);
            }
            let tuple = self.target_tuple_type_record(rest_type);
            if tuple.combined_flags & ELEMENT_FLAGS_VARIABLE != 0 {
                return Some(self.slice_tuple_type(rest_type, tuple.fixed_length, 0));
            }
        }
        None
    }

    pub(crate) fn slice_tuple_type(
        &mut self,
        t: TypeHandle,
        index: usize,
        end_skip_count: isize,
    ) -> TypeHandle {
        let target = self.target_tuple_type_record(t);
        let fixed_length = target.fixed_length;
        let element_infos = target.element_infos.clone();
        let end_index = self
            .get_type_reference_arity(t)
            .saturating_sub(end_skip_count.max(0) as usize);
        if index > fixed_length {
            if let Some(rest_array_type) = self.get_rest_array_type_of_tuple_type(t) {
                return rest_array_type;
            }
            return self.create_tuple_type(Vec::new());
        }
        if index >= end_index {
            return self.create_tuple_type(Vec::new());
        }
        let cached_type_arguments = self.ensure_type_arguments_available(t);
        let mut type_arguments = Vec::with_capacity(end_index - index);
        for i in index..end_index {
            type_arguments.push(
                cached_type_arguments
                    .as_ref()
                    .map_or_else(|| self.cached_type_argument_at(t, i), |args| args[i]),
            );
        }
        let element_infos = element_infos[index..end_index].to_vec();
        self.create_tuple_type_ex(type_arguments, element_infos, false /*readonly*/)
    }

    pub(crate) fn get_known_keys_of_tuple_type(&mut self, t: TypeHandle) -> TypeHandle {
        let tuple = self.target_tuple_type_record(t);
        let fixed_length = tuple.fixed_length;
        let readonly = tuple.readonly;
        let mut keys = Vec::with_capacity(fixed_length + 1);
        for i in 0..fixed_length {
            keys.push(self.get_string_literal_type(&i.to_string()));
        }
        keys.push(self.get_index_type(if readonly {
            self.semantic_state
                .semantic_handles()
                .global_readonly_array_type
        } else {
            self.semantic_state.semantic_handles().global_array_type
        }));
        self.get_union_type(keys)
    }

    fn get_rest_array_type_of_tuple_type(&mut self, t: TypeHandle) -> Option<TypeHandle> {
        if let Some(rest_type) = self.get_rest_type_of_tuple_type(t) {
            return Some(self.create_array_type(rest_type));
        }
        None
    }

    pub(crate) fn get_this_type_of_signature(
        &mut self,
        signature: SignatureHandle,
    ) -> Option<TypeHandle> {
        if let Some(this_parameter) = self.signature_this_parameter_identity(signature) {
            return Some(self.get_type_of_symbol_identity(this_parameter));
        }
        None
    }

    fn is_instantiated_generic_parameter(
        &mut self,
        signature: SignatureHandle,
        pos: usize,
    ) -> bool {
        let Some(target) = self.signature_record(signature).target else {
            return false;
        };
        let t = self.try_get_type_at_position(target, pos);
        t.is_some() && self.is_generic_type(t.unwrap())
    }

    pub(crate) fn get_parameter_name_at_position(
        &mut self,
        signature: SignatureHandle,
        pos: usize,
    ) -> String {
        let parameters = self.signature_parameter_identities(signature);
        let param_count = parameters.len()
            - if self.signature_has_rest_parameter(signature) {
                1
            } else {
                0
            };
        if pos < param_count {
            return self.missing_name_symbol_identity_name(parameters[pos]);
        }
        let rest_parameter = parameters[param_count];
        let rest_type = self.get_type_of_symbol_identity(rest_parameter);
        if self.is_tuple_type(rest_type) {
            let index = pos - param_count;
            self.relater_get_tuple_element_label_from_symbol_identity(
                self.target_tuple_type_record(rest_type).element_infos[index],
                Some(rest_parameter),
                index,
            );
        }
        self.missing_name_symbol_identity_name(rest_parameter)
    }

    pub(crate) fn get_tuple_element_label(
        &mut self,
        element_info: TupleElementInfo,
        rest_symbol: Option<SymbolIdentity>,
        index: usize,
    ) -> String {
        if let Some(labeled_declaration) = element_info.labeled_declaration {
            let store = self.store_for_node(labeled_declaration);
            let name = store.name(labeled_declaration).unwrap();
            return store.text(name).to_string();
        }
        let rest_value_declaration = rest_symbol.and_then(|rest_symbol| {
            self.missing_name_symbol_identity_value_declaration(rest_symbol)
        });
        if rest_value_declaration.as_ref().is_some_and(|declaration| {
            ast::is_parameter_declaration(self.store_for_node(*declaration), *declaration)
        }) {
            return self.get_tuple_element_label_from_binding_element(
                rest_value_declaration.unwrap(),
                index,
                element_info.flags,
            );
        }
        let root_name = rest_symbol
            .map(|rest_symbol| self.missing_name_symbol_identity_name(rest_symbol))
            .unwrap_or_else(|| "arg".to_string());
        format!("{}_{}", root_name, index)
    }

    fn relater_get_tuple_element_label_from_symbol_identity(
        &mut self,
        element_info: TupleElementInfo,
        rest_symbol: Option<SymbolIdentity>,
        index: usize,
    ) -> String {
        if let Some(labeled_declaration) = element_info.labeled_declaration {
            let store = self.store_for_node(labeled_declaration);
            let name = store.name(labeled_declaration).unwrap();
            return store.text(name).to_string();
        }
        let rest_value_declaration = rest_symbol.and_then(|rest_symbol| {
            self.missing_name_symbol_identity_value_declaration(rest_symbol)
        });
        if rest_value_declaration.as_ref().is_some_and(|declaration| {
            ast::is_parameter_declaration(self.store_for_node(*declaration), *declaration)
        }) {
            return self.get_tuple_element_label_from_binding_element(
                rest_value_declaration.unwrap(),
                index,
                element_info.flags,
            );
        }
        let root_name = rest_symbol
            .map(|rest_symbol| self.missing_name_symbol_identity_name(rest_symbol))
            .unwrap_or_else(|| "arg".to_string());
        format!("{}_{}", root_name, index)
    }

    fn get_tuple_element_label_from_binding_element(
        &mut self,
        node: ast::Node,
        index: usize,
        element_flags: ElementFlags,
    ) -> String {
        let store = self.store_for_node(node);
        if let Some(name_node) = store.name(node) {
            match store.kind(name_node) {
                ast::Kind::Identifier => {
                    let name = store.text(name_node).to_string();
                    if has_dot_dot_dot_token(store, node) {
                        // given
                        //   (...[x, y, ...z]: [number, number, ...number[]]) => ...
                        // this produces
                        //   (x: number, y: number, ...z: number[]) => ...
                        // which preserves rest elements of 'z'

                        // given
                        //   (...[x, y, ...z]: [number, number, ...[...number[], number]]) => ...
                        // this produces
                        //   (x: number, y: number, ...z: number[], z_1: number) => ...
                        // which preserves rest elements of z but gives distinct numbers to fixed elements of 'z'
                        if element_flags & ELEMENT_FLAGS_VARIABLE != 0 {
                            return name;
                        }
                        return format!("{}_{}", name, index);
                    }
                    // given
                    //   (...[x]: [number]) => ...
                    // this produces
                    //   (x: number) => ...
                    // which preserves fixed elements of 'x'

                    // given
                    //   (...[x]: ...number[]) => ...
                    // this produces
                    //   (x_0: number) => ...
                    // which which numbers fixed elements of 'x' whose tuple element type is variable
                    if element_flags & ELEMENT_FLAGS_FIXED != 0 {
                        return name;
                    }
                    return format!("{}_n", name);
                }
                ast::Kind::ArrayBindingPattern => {
                    if has_dot_dot_dot_token(store, node) {
                        let elements_view = store
                            .elements(name_node)
                            .expect("array binding pattern must have elements");
                        let elements = elements_view.iter().collect::<Vec<_>>();
                        let last_element = elements.last();
                        let last_element_is_binding_element_rest = last_element.is_some()
                            && ast::is_binding_element(
                                self.store_for_node(*last_element.unwrap()),
                                *last_element.unwrap(),
                            )
                            && has_dot_dot_dot_token(
                                self.store_for_node(*last_element.unwrap()),
                                *last_element.unwrap(),
                            );
                        let element_count = elements.len()
                            - if last_element_is_binding_element_rest {
                                1
                            } else {
                                0
                            };
                        if index < element_count {
                            let element = elements[index];
                            if ast::is_binding_element(self.store_for_node(element), element) {
                                return self.get_tuple_element_label_from_binding_element(
                                    element,
                                    index,
                                    element_flags,
                                );
                            }
                        } else if last_element_is_binding_element_rest {
                            return self.get_tuple_element_label_from_binding_element(
                                *last_element.unwrap(),
                                index - element_count,
                                element_flags,
                            );
                        }
                    }
                }
                _ => {}
            }
        }
        format!("arg_{}", index)
    }
}

impl<'a, 'state> Checker<'a, 'state> {
    fn set_signature_resolved_type_predicate_handle(
        &mut self,
        sig: SignatureHandle,
        predicate: Option<TypePredicateHandle>,
    ) {
        self.semantic_state
            .signature_record_mut(sig)
            .resolved_type_predicate = predicate;
    }

    pub fn get_type_predicate_of_signature(
        &mut self,
        sig: SignatureHandle,
    ) -> Option<TypePredicateHandle> {
        let no_type_predicate = self.semantic_state.semantic_handles().no_type_predicate;
        if self.signature_record(sig).resolved_type_predicate.is_none() {
            if let Some(target) = self.signature_record(sig).target {
                let target_type_predicate = self.get_type_predicate_of_signature(target);
                if let Some(target_type_predicate) = target_type_predicate {
                    let mapper = self.signature_record(sig).mapper;
                    let instantiated = self.instantiate_type_predicate_with_mapper_handle(
                        target_type_predicate,
                        mapper,
                    );
                    self.set_signature_resolved_type_predicate_handle(sig, Some(instantiated));
                }
            } else if let Some(composite) = self.signature_record(sig).composite.clone() {
                let predicate = self.get_union_or_intersection_type_predicate(
                    composite.signatures.clone(),
                    composite.is_union,
                );
                self.set_signature_resolved_type_predicate_handle(sig, predicate);
            } else if let Some(declaration) = self.signature_record(sig).declaration {
                let declaration_store = self.store_for_output_node(declaration);
                let type_node = declaration_store.r#type(declaration);
                if let Some(type_node) = type_node {
                    let type_node = type_node;
                    if ast::is_type_predicate_node(declaration_store, type_node) {
                        let predicate =
                            self.create_type_predicate_from_type_predicate_node(type_node, sig);
                        self.set_signature_resolved_type_predicate_handle(sig, Some(predicate));
                    }
                } else if ast::is_function_like_declaration(declaration_store, Some(declaration))
                    && (self.signature_record(sig).resolved_return_type.is_none()
                        || self
                            .type_flags(self.signature_record(sig).resolved_return_type.unwrap())
                            & TYPE_FLAGS_BOOLEAN
                            != 0)
                    && self.get_parameter_count(sig) > 0
                {
                    self.set_signature_resolved_type_predicate_handle(sig, Some(no_type_predicate)); // avoid infinite loop
                    let predicate = self.get_type_predicate_from_body(declaration);
                    self.set_signature_resolved_type_predicate_handle(sig, predicate);
                }
            }
            if self.signature_record(sig).resolved_type_predicate.is_none() {
                self.set_signature_resolved_type_predicate_handle(sig, Some(no_type_predicate));
            }
        }
        let resolved = self.signature_record(sig).resolved_type_predicate;
        if resolved == Some(no_type_predicate) {
            return None;
        }
        resolved
    }

    fn get_union_or_intersection_type_predicate(
        &mut self,
        signatures: Vec<SignatureHandle>,
        is_union: bool,
    ) -> Option<TypePredicateHandle> {
        let mut last: Option<TypePredicateHandle> = None;
        let mut types = Vec::new();
        for sig in signatures {
            let pred = self.get_type_predicate_of_signature(sig);
            if let Some(pred) = pred {
                // Constituent type predicates must all have matching kinds. We don't create composite type predicates for assertions.
                let pred_record = self.type_predicate_record(pred).clone();
                if (pred_record.kind != TYPE_PREDICATE_KIND_THIS
                    && pred_record.kind != TYPE_PREDICATE_KIND_IDENTIFIER)
                    || (last
                        .as_ref()
                        .is_some_and(|last| !self.type_predicate_kinds_match(*last, pred)))
                {
                    return None;
                }
                last = Some(pred);
                types.push(pred_record.t.unwrap());
            } else {
                // In composite union signatures we permit and ignore signatures with a return type `false`.
                let mut return_type = None;
                if is_union {
                    return_type = Some(self.get_return_type_of_signature(sig));
                }
                if return_type != Some(self.semantic_state.semantic_handles().false_type)
                    && return_type
                        != Some(self.semantic_state.semantic_handles().regular_false_type)
                {
                    return None;
                }
            }
        }
        let last = last?;
        let composite_type =
            self.get_union_or_intersection_type(types, is_union, UNION_REDUCTION_LITERAL);
        Some(self.new_type_predicate(
            self.type_predicate_record(last).kind,
            self.type_predicate_record(last).parameter_name.clone(),
            self.type_predicate_record(last).parameter_index,
            Some(composite_type),
        ))
    }

    pub(crate) fn type_predicate_kinds_match(
        &self,
        a: TypePredicateHandle,
        b: TypePredicateHandle,
    ) -> bool {
        let a = self.type_predicate_record(a);
        let b = self.type_predicate_record(b);
        a.kind == b.kind && a.parameter_index == b.parameter_index
    }

    fn create_type_predicate_from_type_predicate_node(
        &mut self,
        node: ast::Node,
        signature: SignatureHandle,
    ) -> TypePredicateHandle {
        let store = self.store_for_node(node);
        let mut t = None;
        if let Some(type_node) = store.r#type(node) {
            t = Some(self.get_type_from_type_node(type_node));
        }
        let parameter_name = store.parameter_name(node).unwrap();
        if ast::is_this_type_node(store, parameter_name) {
            let kind = if store.asserts_modifier(node).is_some() {
                TYPE_PREDICATE_KIND_ASSERTS_THIS
            } else {
                TYPE_PREDICATE_KIND_THIS
            };
            return self.new_type_predicate(
                kind,
                String::new(), /*parameterName*/
                0,             /*parameterIndex*/
                t,
            );
        }
        let kind = if store.asserts_modifier(node).is_some() {
            TYPE_PREDICATE_KIND_ASSERTS_IDENTIFIER
        } else {
            TYPE_PREDICATE_KIND_IDENTIFIER
        };
        let name = store.text(parameter_name).to_string();
        let index = self
            .signature_parameter_identities(signature)
            .iter()
            .position(|p| self.missing_name_symbol_identity_name(*p) == name)
            .map(|i| i as i32)
            .unwrap_or(-1);
        self.new_type_predicate(kind, name, index, t)
    }

    pub(crate) fn instantiate_type_predicate_with_mapper_handle(
        &mut self,
        predicate: TypePredicateHandle,
        mapper: Option<TypeMapperHandle>,
    ) -> TypePredicateHandle {
        let predicate_record = self.type_predicate_record(predicate).clone();
        let t = self.instantiate_type_with_mapper_handle(predicate_record.t, mapper);
        if t == predicate_record.t {
            return predicate;
        }
        self.new_type_predicate(
            predicate_record.kind,
            predicate_record.parameter_name,
            predicate_record.parameter_index,
            t,
        )
    }

    pub(crate) fn new_type_predicate(
        &mut self,
        kind: TypePredicateKind,
        parameter_name: String,
        parameter_index: i32,
        t: Option<TypeHandle>,
    ) -> TypePredicateHandle {
        self.semantic_state
            .alloc_type_predicate(TypePredicateRecord {
                kind,
                parameter_index,
                parameter_name,
                t,
            })
    }

    pub(crate) fn is_resolving_return_type_of_signature(
        &mut self,
        signature: SignatureHandle,
    ) -> bool {
        let composite = self.signature_record(signature).composite.clone();
        if composite.as_ref().is_some_and(|composite| {
            composite
                .signatures
                .iter()
                .any(|s| self.is_resolving_return_type_of_signature(*s))
        }) {
            return true;
        }
        self.signature_record(signature)
            .resolved_return_type
            .is_none()
            && self.find_resolution_cycle_start_index(
                TypeSystemEntity::Signature(signature),
                TYPE_SYSTEM_PROPERTY_NAME_RESOLVED_RETURN_TYPE,
            ) >= 0
    }

    pub(crate) fn find_matching_signatures(
        &mut self,
        signature_lists: &[Vec<SignatureHandle>],
        signature: SignatureHandle,
        list_index: usize,
    ) -> Option<Vec<SignatureHandle>> {
        if !self.signature_record(signature).type_parameters.is_empty() {
            // We require an exact match for generic signatures, so we only return signatures from the first
            // signature list and only if they have exact matches in the other signature lists.
            if list_index > 0 {
                return None;
            }
            for i in 1..signature_lists.len() {
                if self
                    .find_matching_signature(
                        &signature_lists[i],
                        signature,
                        false, /*partialMatch*/
                        false, /*ignoreThisTypes*/
                        false, /*ignoreReturnTypes*/
                    )
                    .is_none()
                {
                    return None;
                }
            }
            return Some(vec![signature]);
        }
        let mut result = Vec::new();
        for i in 0..signature_lists.len() {
            // Allow matching non-generic signatures to have excess parameters (as a fallback if exact parameter match is not found) and different return types.
            // Prefer matching this types if possible.
            let mut match_ = None;
            if i == list_index {
                match_ = Some(signature);
            } else {
                match_ = self.find_matching_signature(
                    &signature_lists[i],
                    signature,
                    false, /*partialMatch*/
                    false, /*ignoreThisTypes*/
                    true,  /*ignoreReturnTypes*/
                );
                if match_.is_none() {
                    match_ = self.find_matching_signature(
                        &signature_lists[i],
                        signature,
                        true,  /*partialMatch*/
                        false, /*ignoreThisTypes*/
                        true,  /*ignoreReturnTypes*/
                    );
                }
            }
            if match_.is_none() {
                return None;
            }
            result = core::append_if_unique(&result, match_.unwrap());
        }
        Some(result)
    }

    pub(crate) fn find_matching_signature(
        &mut self,
        signature_list: &[SignatureHandle],
        signature: SignatureHandle,
        partial_match: bool,
        ignore_this_types: bool,
        ignore_return_types: bool,
    ) -> Option<SignatureHandle> {
        let compare_types = if partial_match {
            Checker::compare_types_subtype_of
        } else {
            Checker::compare_types_identical
        };
        for s in signature_list.iter() {
            if self.compare_signatures_identical(
                *s,
                signature,
                partial_match,
                ignore_this_types,
                ignore_return_types,
                compare_types,
            ) != 0
            {
                return Some(*s);
            }
        }
        None
    }

    /**
     * See signatureRelatedTo, compareSignaturesIdentical
     */
    pub(crate) fn compare_signatures_identical(
        &mut self,
        mut source: SignatureHandle,
        target: SignatureHandle,
        partial_match: bool,
        ignore_this_types: bool,
        ignore_return_types: bool,
        compare_types: fn(&mut Checker<'a, 'state>, TypeHandle, TypeHandle) -> Ternary,
    ) -> Ternary {
        if source == target {
            return TERNARY_TRUE;
        }
        if !self.is_matching_signature(source, target, partial_match) {
            return TERNARY_FALSE;
        }
        // Check that the two signatures have the same number of type parameters.
        if self.signature_record(source).type_parameters.len()
            != self.signature_record(target).type_parameters.len()
        {
            return TERNARY_FALSE;
        }
        // Check that type parameter constraints and defaults match. If they do, instantiate the source
        // signature with the type parameters of the target signature and continue the comparison.
        if !self.signature_record(target).type_parameters.is_empty() {
            let source_type_parameters = self.signature_record(source).type_parameters.clone();
            let target_type_parameters = self.signature_record(target).type_parameters.clone();
            let mapper = self.new_type_mapper_handle(
                source_type_parameters.clone(),
                target_type_parameters.clone(),
            );
            for i in 0..target_type_parameters.len() {
                let s = source_type_parameters[i];
                let t = target_type_parameters[i];
                if !(s == t || {
                    let source_constraint = self.get_constraint_or_unknown_from_type_parameter(s);
                    let source_constraint = self
                        .instantiate_type_with_mapper_handle(Some(source_constraint), Some(mapper))
                        .unwrap();
                    let target_constraint = self.get_constraint_or_unknown_from_type_parameter(t);
                    compare_types(self, source_constraint, target_constraint) != TERNARY_FALSE && {
                        let source_default = self.get_default_or_unknown_from_type_parameter(s);
                        let source_default = self
                            .instantiate_type_with_mapper_handle(Some(source_default), Some(mapper))
                            .unwrap();
                        let target_default = self.get_default_or_unknown_from_type_parameter(t);
                        compare_types(self, source_default, target_default) != TERNARY_FALSE
                    }
                }) {
                    return TERNARY_FALSE;
                }
            }
            source = self.instantiate_signature_ex_with_mapper_handle(
                source, mapper, true, /*eraseTypeParameters*/
            );
        }
        let mut result = TERNARY_TRUE;
        if !ignore_this_types {
            let source_this_type = self.get_this_type_of_signature(source);
            if let Some(source_this_type) = source_this_type {
                let target_this_type = self.get_this_type_of_signature(target);
                if let Some(target_this_type) = target_this_type {
                    let related = compare_types(self, source_this_type, target_this_type);
                    if related == TERNARY_FALSE {
                        return TERNARY_FALSE;
                    }
                    result &= related;
                }
            }
        }
        for i in 0..self.get_parameter_count(target) {
            let s = self.get_type_at_position(source, i);
            let t = self.get_type_at_position(target, i);
            let related = compare_types(self, t, s);
            if related == TERNARY_FALSE {
                return TERNARY_FALSE;
            }
            result &= related;
        }
        if !ignore_return_types {
            let source_type_predicate = self.get_type_predicate_of_signature(source);
            let target_type_predicate = self.get_type_predicate_of_signature(target);
            if source_type_predicate.is_some() || target_type_predicate.is_some() {
                result &= self.compare_type_predicates_identical(
                    source_type_predicate,
                    target_type_predicate,
                    compare_types,
                );
            } else {
                let source_return_type = self.get_return_type_of_signature(source);
                let target_return_type = self.get_return_type_of_signature(target);
                result &= compare_types(self, source_return_type, target_return_type);
            }
        }
        result
    }

    fn is_matching_signature(
        &mut self,
        source: SignatureHandle,
        target: SignatureHandle,
        partial_match: bool,
    ) -> bool {
        let source_parameter_count = self.get_parameter_count(source);
        let target_parameter_count = self.get_parameter_count(target);
        let source_min_argument_count = self.get_min_argument_count(source);
        let target_min_argument_count = self.get_min_argument_count(target);
        let source_has_rest_parameter = self.has_effective_rest_parameter(source);
        let target_has_rest_parameter = self.has_effective_rest_parameter(target);
        // A source signature matches a target signature if the two signatures have the same number of required,
        // optional, and rest parameters.
        if source_parameter_count == target_parameter_count
            && source_min_argument_count == target_min_argument_count
            && source_has_rest_parameter == target_has_rest_parameter
        {
            return true;
        }
        // A source signature partially matches a target signature if the target signature has no fewer required
        // parameters
        if partial_match && source_min_argument_count <= target_min_argument_count {
            return true;
        }
        false
    }

    pub(crate) fn compare_type_parameters_identical(
        &mut self,
        source_params: Vec<TypeHandle>,
        target_params: Vec<TypeHandle>,
    ) -> bool {
        if source_params.len() != target_params.len() {
            return false;
        }
        let mapper = self.new_type_mapper_handle(target_params.clone(), source_params.clone());
        for i in 0..source_params.len() {
            let source = source_params[i];
            let target = target_params[i];
            if source == target {
                continue;
            }
            // We instantiate the target type parameter constraints into the source types so we can recognize `<T, U extends T>` as the same as `<A, B extends A>`
            let source_constraint = self
                .get_constraint_from_type_parameter(source)
                .unwrap_or(self.semantic_state.semantic_handles().unknown_type);
            let target_constraint = self
                .get_constraint_from_type_parameter(target)
                .unwrap_or(self.semantic_state.semantic_handles().unknown_type);
            let target_constraint = self
                .instantiate_type_with_mapper_handle(Some(target_constraint), Some(mapper))
                .unwrap();
            if !self.is_type_identical_to(source_constraint, target_constraint) {
                return false;
            }
            // We don't compare defaults - we just use the type parameter defaults from the first signature that seems to match.
            // It might make sense to combine these defaults in the future, but doing so intelligently requires knowing
            // if the parameter is used covariantly or contravariantly (so we intersect if it's used like a parameter or union if used like a return type)
            // and, since it's just an inference _default_, just picking one arbitrarily works OK.
        }
        true
    }

    fn compare_type_predicates_identical(
        &mut self,
        source: Option<TypePredicateHandle>,
        target: Option<TypePredicateHandle>,
        compare_types: fn(&mut Checker<'a, 'state>, TypeHandle, TypeHandle) -> Ternary,
    ) -> Ternary {
        match (source, target) {
            (Some(source), Some(target)) if self.type_predicate_kinds_match(source, target) => {
                let source_record = self.type_predicate_record(source);
                let target_record = self.type_predicate_record(target);
                if source_record.t == target_record.t {
                    TERNARY_TRUE
                } else if source_record.t.is_some() && target_record.t.is_some() {
                    compare_types(self, source_record.t.unwrap(), target_record.t.unwrap())
                } else {
                    TERNARY_FALSE
                }
            }
            _ => TERNARY_FALSE,
        }
    }

    fn get_effective_constraint_of_intersection(
        &mut self,
        source: TypeHandle,
        target_is_union: bool,
    ) -> Option<TypeHandle> {
        let mut constraints = Vec::new();
        let mut has_disjoint_domain_type = false;
        let source_is_intersection = self.type_flags(source) & TYPE_FLAGS_INTERSECTION != 0;
        let types_len = if source_is_intersection {
            self.type_types_len(source)
        } else {
            1
        };
        for index in 0..types_len {
            let t = if source_is_intersection {
                self.type_type_at(source, index)
            } else {
                source
            };
            if self.type_flags(t) & TYPE_FLAGS_INSTANTIABLE != 0 {
                // We keep following constraints as long as we have an instantiable type that is known
                // not to be circular or infinite (hence we stop on index access types).
                let mut constraint = self.get_constraint_of_type(t);
                while constraint.is_some()
                    && self.type_flags(constraint.unwrap())
                        & (TYPE_FLAGS_TYPE_PARAMETER | TYPE_FLAGS_INDEX | TYPE_FLAGS_CONDITIONAL)
                        != 0
                {
                    constraint = self.get_constraint_of_type(constraint.unwrap());
                }
                if let Some(constraint) = constraint {
                    constraints.push(constraint);
                    if target_is_union {
                        constraints.push(t);
                    }
                }
            } else if self.type_flags(t) & TYPE_FLAGS_DISJOINT_DOMAINS != 0
                || self.is_empty_anonymous_object_type(t)
            {
                has_disjoint_domain_type = true;
            }
        }
        // If the target is a union type or if we are intersecting with types belonging to one of the
        // disjoint domains, we may end up producing a constraint that hasn't been examined before.
        if !constraints.is_empty() && (target_is_union || has_disjoint_domain_type) {
            if has_disjoint_domain_type {
                // We add any types belong to one of the disjoint domains because they might cause the final
                // intersection operation to reduce the union constraints.
                for index in 0..types_len {
                    let t = if source_is_intersection {
                        self.type_type_at(source, index)
                    } else {
                        source
                    };
                    if self.type_flags(t) & TYPE_FLAGS_DISJOINT_DOMAINS != 0
                        || self.is_empty_anonymous_object_type(t)
                    {
                        constraints.push(t);
                    }
                }
            }
            // The source types were normalized; ensure the result is normalized too.
            let intersection = self.get_intersection_type_ex(
                constraints,
                INTERSECTION_FLAGS_NO_CONSTRAINT_REDUCTION,
                None,
            );
            return Some(self.get_normalized_type(intersection, false /*writing*/));
        }
        None
    }

    fn template_literal_types_definitely_unrelated(
        &self,
        source: &TemplateLiteralTypeRecord,
        target: &TemplateLiteralTypeRecord,
    ) -> bool {
        if source.texts_equal(target) {
            return false;
        }
        // Two template literal types with differences in their starting or ending text spans are definitely unrelated.
        let source_start = &source.texts[0];
        let target_start = &target.texts[0];
        let source_end = &source.texts[source.texts.len() - 1];
        let target_end = &target.texts[target.texts.len() - 1];
        let start_len = source_start.len().min(target_start.len());
        let end_len = source_end.len().min(target_end.len());
        source_start[..start_len] != target_start[..start_len]
            || source_end[source_end.len() - end_len..] != target_end[target_end.len() - end_len..]
    }

    pub(crate) fn is_type_matched_by_template_literal_type(
        &mut self,
        source: TypeHandle,
        target: &TemplateLiteralTypeRecord,
        compare_types: TypeComparer,
    ) -> bool {
        let inferences = self.infer_types_from_template_literal_type(source, target);
        if let Some(inferences) = inferences {
            for (i, inference) in inferences.iter().enumerate() {
                if !self.is_valid_type_for_template_literal_placeholder(
                    *inference,
                    target.types[i],
                    compare_types,
                ) {
                    return false;
                }
            }
            return true;
        }
        false
    }

    pub(crate) fn infer_types_from_template_literal_type(
        &mut self,
        source: TypeHandle,
        target: &TemplateLiteralTypeRecord,
    ) -> Option<Vec<TypeHandle>> {
        if self.type_flags(source) & TYPE_FLAGS_STRING_LITERAL != 0 {
            let source_text = self.get_string_literal_value(source);
            return self.infer_from_literal_parts_to_template_literal(
                std::slice::from_ref(&source_text),
                &[],
                target,
            );
        }
        if self.type_flags(source) & TYPE_FLAGS_TEMPLATE_LITERAL != 0 {
            let source_template = self.type_record(source).as_template_literal_type().clone();
            if source_template.texts_equal(target) {
                return Some(
                    source_template
                        .types
                        .iter()
                        .enumerate()
                        .map(|(i, s)| {
                            let source_constraint = self.get_base_constraint_or_type(*s);
                            let target_constraint =
                                self.get_base_constraint_or_type(target.types[i]);
                            if self.is_type_assignable_to(source_constraint, target_constraint) {
                                *s
                            } else {
                                self.get_string_like_type_for_type(*s)
                            }
                        })
                        .collect(),
                );
            }
            return self.infer_from_literal_parts_to_template_literal(
                &source_template.texts,
                &source_template.types,
                target,
            );
        }
        None
    }

    // This function infers from the text parts and type parts of a source literal to a target template literal. The number
    // of text parts is always one more than the number of type parts, and a source string literal is treated as a source
    // with one text part and zero type parts. The function returns an array of inferred string or template literal types
    // corresponding to the placeholders in the target template literal, or undefined if the source doesn't match the target.
    //
    // We first check that the starting source text part matches the starting target text part, and that the ending source
    // text part ends matches the ending target text part. We then iterate through the remaining target text parts, finding
    // a match for each in the source and inferring string or template literal types created from the segments of the source
    // that occur between the matches. During this iteration, seg holds the index of the current text part in the sourceTexts
    // array and pos holds the current character position in the current text part.
    //
    // Consider inference from type `<<${string}>.<${number}-${number}>>` to type `<${string}.${string}>`, i.e.
    //
    //	sourceTexts = ['<<', '>.<', '-', '>>']
    //	sourceTypes = [string, number, number]
    //	target.texts = ['<', '.', '>']
    //
    // We first match '<' in the target to the start of '<<' in the source and '>' in the target to the end of '>>'. The first match for the '.' in target occurs at character 1 in the source text part at index 1, and thus
    // the first inference is the template literal type `<${string}>`. The remainder of the source makes up the second
    // inference, the template literal type `<${number}-${number}>`.
    fn infer_from_literal_parts_to_template_literal(
        &mut self,
        source_texts: &[String],
        source_types: &[TypeHandle],
        target: &TemplateLiteralTypeRecord,
    ) -> Option<Vec<TypeHandle>> {
        let last_source_index = source_texts.len() - 1;
        let source_start_text = &source_texts[0];
        let source_end_text = &source_texts[last_source_index];
        let target_texts = &target.texts;
        let last_target_index = target_texts.len() - 1;
        let target_start_text = &target_texts[0];
        let target_end_text = &target_texts[last_target_index];
        if (last_source_index == 0
            && source_start_text.len() < target_start_text.len() + target_end_text.len())
            || !starts_with_text(source_start_text, target_start_text)
            || !ends_with_text(source_end_text, target_end_text)
        {
            return None;
        }
        let remaining_end_text = &source_end_text[..source_end_text.len() - target_end_text.len()];
        let mut seg = 0usize;
        let mut pos = target_start_text.len();
        let mut matches = Vec::with_capacity(target.types.len());
        let get_source_text = |index: usize| -> &str {
            if index < last_source_index {
                &source_texts[index]
            } else {
                remaining_end_text
            }
        };
        let add_match = |c: &mut Checker<'a, 'state>,
                         s: usize,
                         p: usize,
                         seg: &mut usize,
                         pos: &mut usize,
                         matches: &mut Vec<TypeHandle>| {
            let match_type = if s == *seg {
                c.get_string_literal_type(&get_source_text(s)[*pos..p])
            } else {
                let mut match_texts = Vec::with_capacity(s - *seg + 1);
                match_texts.push(&source_texts[*seg][*pos..]);
                for src in &source_texts[*seg + 1..s] {
                    match_texts.push(src.as_str());
                }
                match_texts.push(&get_source_text(s)[..p]);
                c.get_template_literal_type_from_parts(&match_texts, &source_types[*seg..s])
            };
            matches.push(match_type);
            *seg = s;
            *pos = p;
        };
        for i in 1..last_target_index {
            let delim = &target_texts[i];
            if !delim.is_empty() {
                let mut s = seg;
                let mut p = pos;
                loop {
                    if let Some(d) = get_source_text(s)[p..].find(delim) {
                        p += d;
                        break;
                    }
                    s += 1;
                    if s == source_texts.len() {
                        return None;
                    }
                    p = 0;
                }
                add_match(self, s, p, &mut seg, &mut pos, &mut matches);
                pos += delim.len();
            } else if pos < get_source_text(seg).len() {
                let size = get_source_text(seg)[pos..]
                    .chars()
                    .next()
                    .unwrap()
                    .len_utf8();
                add_match(self, seg, pos + size, &mut seg, &mut pos, &mut matches);
            } else if seg < last_source_index {
                add_match(self, seg + 1, 0, &mut seg, &mut pos, &mut matches);
            } else {
                return None;
            }
        }
        add_match(
            self,
            last_source_index,
            get_source_text(last_source_index).len(),
            &mut seg,
            &mut pos,
            &mut matches,
        );
        Some(matches)
    }

    fn get_string_like_type_for_type(&mut self, t: TypeHandle) -> TypeHandle {
        if self.type_flags(t) & (TYPE_FLAGS_ANY | TYPE_FLAGS_STRING_LIKE) != 0 {
            return t;
        }
        self.get_template_literal_type_from_parts(&[String::new(), String::new()], &[t])
    }

    fn is_valid_type_for_template_literal_placeholder(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        compare_types: TypeComparer,
    ) -> bool {
        if self.type_flags(target) & TYPE_FLAGS_INTERSECTION != 0 {
            let target_types_len = self.type_types_len(target);
            for index in 0..target_types_len {
                let ty = self.type_type_at(target, index);
                if ty
                    != self
                        .semantic_state
                        .semantic_handles()
                        .empty_type_literal_type
                    && !self.is_valid_type_for_template_literal_placeholder(
                        source,
                        ty,
                        compare_types,
                    )
                {
                    return false;
                }
            }
            return true;
        }
        if self.type_flags(target) & TYPE_FLAGS_STRING != 0
            || compare_types(self, source, target, false) != TERNARY_FALSE
        {
            return true;
        }
        if self.type_flags(source) & TYPE_FLAGS_STRING_LITERAL != 0 {
            let value = self.get_string_literal_value(source);
            let target_flags = self.type_flags(target);
            if target_flags & TYPE_FLAGS_NUMBER != 0
                && is_valid_number_string(&value, false /*roundTripOnly*/)
                || target_flags & TYPE_FLAGS_BIG_INT != 0
                    && is_valid_big_int_string(&value, false /*roundTripOnly*/)
                || target_flags & (TYPE_FLAGS_BOOLEAN_LITERAL | TYPE_FLAGS_NULLABLE) != 0
                    && value == self.type_record(target).as_intrinsic_type().intrinsic_name
                || target_flags & TYPE_FLAGS_STRING_MAPPING != 0
                    && self.is_member_of_string_mapping(source, target)
            {
                return true;
            }
            if target_flags & TYPE_FLAGS_TEMPLATE_LITERAL != 0 {
                let target_template = self.type_record(target).as_template_literal_type().clone();
                return self.is_type_matched_by_template_literal_type(
                    source,
                    &target_template,
                    compare_types,
                );
            }
            return false;
        }
        if self.type_flags(source) & TYPE_FLAGS_TEMPLATE_LITERAL != 0 {
            let source_template = self.type_record(source).as_template_literal_type().clone();
            let texts = &source_template.texts;
            return texts.len() == 2
                && texts[0].is_empty()
                && texts[1].is_empty()
                && compare_types(self, source_template.types[0], target, false) != TERNARY_FALSE;
        }
        false
    }

    pub(crate) fn is_member_of_string_mapping(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
    ) -> bool {
        if self.type_flags(target) & TYPE_FLAGS_ANY != 0 {
            return true;
        }
        if self.type_flags(target) & (TYPE_FLAGS_STRING | TYPE_FLAGS_TEMPLATE_LITERAL) != 0 {
            return self.is_type_assignable_to(source, target);
        }
        if self.type_flags(target) & TYPE_FLAGS_STRING_MAPPING != 0 {
            // We need to see whether applying the same mappings of the target
            // onto the source would produce an identical type *and* that
            // it's compatible with the inner-most non-string-mapped type.
            //
            // The intuition here is that if same mappings don't affect the source at all,
            // and the source is compatible with the unmapped target, then they must
            // still reside in the same domain.
            let (mapped, inner) = self.apply_target_string_mapping_to_source(source, target);
            return mapped == source && self.is_member_of_string_mapping(source, inner);
        }
        false
    }

    fn apply_target_string_mapping_to_source(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
    ) -> (TypeHandle, TypeHandle) {
        let mut inner = self
            .type_record(target)
            .as_string_mapping_type()
            .target
            .unwrap();
        let mut source = source;
        if self.type_flags(inner) & TYPE_FLAGS_STRING_MAPPING != 0 {
            let pair = self.apply_target_string_mapping_to_source(source, inner);
            source = pair.0;
            inner = pair.1;
        }
        (
            self.get_string_mapping_type(self.type_symbol_identity(target).unwrap(), source),
            inner,
        )
    }
}

pub(crate) fn visibility_to_string(flags: ast::ModifierFlags) -> &'static str {
    if flags == ast::MODIFIER_FLAGS_PRIVATE {
        return "private";
    }
    if flags == ast::MODIFIER_FLAGS_PROTECTED {
        return "protected";
    }
    "public"
}

pub(crate) struct ErrorState {
    error_chain: Option<ErrorChainHandle>,
    related_info: Vec<ast::Diagnostic>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ErrorChainHandle(usize);

pub(crate) struct ErrorChain {
    next: Option<ErrorChainHandle>,
    message: &'static diagnostics::Message,
    args: Vec<DiagnosticArg>,
}

pub struct Relater<'a, 'state, 'c> {
    c: &'c mut Checker<'a, 'state>,
    relation: Option<RelationKind>,
    error_node: Option<ast::Node>,
    error_chain: Option<ErrorChainHandle>,
    error_chains: Vec<ErrorChain>,
    skip_parent_counter: usize,
    related_info: Vec<ast::Diagnostic>,
    maybe_keys: Vec<CacheHashKey>,
    maybe_keys_set: collections::Set<CacheHashKey>,
    source_stack: Vec<TypeHandle>,
    target_stack: Vec<TypeHandle>,
    expanding_flags: ExpandingFlags,
    overflow: bool,
    relation_count: i32,
}

impl<'a, 'state> Checker<'a, 'state> {
    fn relater_get_type_of_symbol(&mut self, symbol: SymbolIdentity) -> TypeHandle {
        self.get_type_of_symbol_identity_at_location(symbol, None)
    }

    fn relater_get_type_of_parameter_identity(&mut self, symbol: SymbolIdentity) -> TypeHandle {
        let symbol_type = self.get_type_of_symbol_identity(symbol);
        let is_optional = self
            .missing_name_symbol_identity_value_declaration(symbol)
            .is_some_and(|declaration| {
                let declaration_store = self.store_for_node(declaration);
                declaration_store.initializer(declaration).is_some()
                    || is_optional_declaration(declaration_store, declaration)
            });
        self.add_optionality_ex(symbol_type, false, is_optional)
    }

    fn relater_get_non_missing_type_of_symbol(&mut self, symbol: SymbolIdentity) -> TypeHandle {
        let optional = self
            .missing_name_symbol_identity_flags(symbol)
            .intersects(ast::SYMBOL_FLAGS_OPTIONAL);
        let symbol_type = self.relater_get_type_of_symbol(symbol);
        self.remove_missing_type(symbol_type, optional)
    }

    fn relater_symbol_identity_to_string(&mut self, symbol: SymbolIdentity) -> String {
        self.symbol_identity_to_string(symbol)
    }

    fn relater_get_symbol_name_for_private_identifier_from_identity(
        &self,
        containing_class_symbol: SymbolIdentity,
        description: &str,
    ) -> Option<String> {
        let symbol_handle = containing_class_symbol.symbol_handle();
        Some(self.private_identifier_symbol_name_for_symbol_handle(symbol_handle, description))
    }

    fn relater_get_literal_type_from_property(
        &mut self,
        prop: SymbolIdentity,
        include: TypeFlags,
        include_non_public: bool,
    ) -> TypeHandle {
        let prop = prop.symbol_handle();
        self.relater_get_literal_type_from_property_handle(prop, include, include_non_public)
    }

    fn relater_get_literal_type_from_property_handle(
        &mut self,
        prop: ast::SymbolHandle,
        include: TypeFlags,
        include_non_public: bool,
    ) -> TypeHandle {
        let modifier_flags = self.relater_declaration_modifier_flags_from_symbol_handle(prop);
        let name = self.symbol_handle_name(prop).to_string();
        let value_declaration = self.symbol_handle_value_declaration(prop);
        let is_known_symbol = is_late_bound_name(&name);
        if include_non_public
            || modifier_flags & ast::ModifierFlags::NON_PUBLIC_ACCESSIBILITY_MODIFIER == 0
        {
            let mut t = self
                .semantic_state
                .value_symbol_name_type(SymbolIdentity::from_symbol_handle(prop));
            if t.is_none() {
                if name == ast::InternalSymbolName::Default {
                    t = Some(self.get_string_literal_type("default"));
                } else {
                    let name_node = value_declaration.and_then(|value_declaration| {
                        ast::get_name_of_declaration(
                            self.store_for_node(value_declaration),
                            Some(value_declaration),
                        )
                    });
                    if let Some(name_node) = name_node {
                        t = Some(self.get_literal_type_from_property_name(name_node));
                    }
                    if t.is_none() && !is_known_symbol {
                        t = Some(self.get_string_literal_type(&name));
                    }
                }
            }
            if let Some(t) = t {
                if self.type_flags(t) & include != 0 {
                    return t;
                }
            }
        }
        self.semantic_state.semantic_handles().never_type
    }

    fn relater_declaration_modifier_flags_from_symbol_handle(
        &mut self,
        prop: ast::SymbolHandle,
    ) -> ast::ModifierFlags {
        let flags = self.symbol_handle_flags(prop);
        let check_flags = self.symbol_handle_check_flags(prop);
        let Some(value_declaration) = self.symbol_handle_value_declaration(prop) else {
            if check_flags.intersects(ast::CHECK_FLAGS_SYNTHETIC) {
                let access_modifier = if check_flags.intersects(ast::CHECK_FLAGS_CONTAINS_PRIVATE) {
                    ast::ModifierFlags::Private
                } else if check_flags.intersects(ast::CHECK_FLAGS_CONTAINS_PUBLIC) {
                    ast::ModifierFlags::Public
                } else {
                    ast::ModifierFlags::Protected
                };
                let static_modifier = if check_flags.intersects(ast::CHECK_FLAGS_CONTAINS_STATIC) {
                    ast::ModifierFlags::Static
                } else {
                    ast::ModifierFlags::None
                };
                return access_modifier | static_modifier;
            }
            if flags.intersects(ast::SYMBOL_FLAGS_PROTOTYPE) {
                return ast::ModifierFlags::Public | ast::ModifierFlags::Static;
            }
            return ast::ModifierFlags::None;
        };
        let declaration = if flags.intersects(ast::SYMBOL_FLAGS_GET_ACCESSOR) {
            self.with_symbol_handle_declarations(prop, |declarations| {
                declarations
                    .iter()
                    .copied()
                    .find(|declaration| {
                        ast::is_get_accessor_declaration(
                            self.store_for_node(*declaration),
                            *declaration,
                        )
                    })
                    .unwrap_or(value_declaration)
            })
        } else {
            value_declaration
        };
        let modifier_flags = self.get_combined_modifier_flags_cached(declaration);
        if self.symbol_handle_parent(prop).is_some_and(|parent| {
            self.symbol_handle_flags(parent)
                .intersects(ast::SYMBOL_FLAGS_CLASS)
        }) {
            return modifier_flags;
        }
        modifier_flags & !ast::ModifierFlags::AccessibilityModifier
    }

    fn relater_declaration_modifier_flags_from_symbol_identity(
        &mut self,
        prop: SymbolIdentity,
    ) -> ast::ModifierFlags {
        let prop = prop.symbol_handle();
        self.relater_declaration_modifier_flags_from_symbol_handle(prop)
    }

    fn get_relater(&mut self) -> Relater<'a, 'state, '_> {
        Relater::new(self)
    }

    fn get_type_of_property_in_types(&mut self, types: Vec<TypeHandle>, name: &str) -> TypeHandle {
        let mut prop_types = Vec::new();
        for t in types {
            prop_types.push(self.get_type_of_property_in_type(t, name));
        }
        self.get_union_type(prop_types)
    }

    fn get_type_of_property_in_type(&mut self, mut t: TypeHandle, name: &str) -> TypeHandle {
        t = self.get_apparent_type(t);
        let prop = if self.type_flags(t) & TYPE_FLAGS_UNION_OR_INTERSECTION != 0 {
            self.get_property_of_union_or_intersection_type(t, name, false)
        } else {
            self.get_property_of_object_type(t, name)
        };
        if let Some(prop) = prop {
            return self.relater_get_type_of_symbol(prop);
        }
        let index_info = self.get_applicable_index_info_for_name(t, name);
        if let Some(index_info) = index_info {
            return self.index_info_record(index_info).value_type.unwrap();
        }
        self.semantic_state.semantic_handles().undefined_type
    }

    pub(crate) fn is_type_subset_of(&mut self, source: TypeHandle, target: TypeHandle) -> bool {
        source == target
            || self.type_flags(source) & TYPE_FLAGS_NEVER != 0
            || self.type_flags(target) & TYPE_FLAGS_UNION != 0
                && self.is_type_subset_of_union(source, target)
    }

    fn is_type_subset_of_union(&mut self, source: TypeHandle, target: TypeHandle) -> bool {
        if self.type_flags(source) & TYPE_FLAGS_UNION != 0 {
            let source_types_len = self.type_types_len(source);
            for index in 0..source_types_len {
                let t = self.type_type_at(source, index);
                if !self.type_types_contains(target, t) {
                    return false;
                }
            }
            return true;
        }
        if self.type_flags(source) & TYPE_FLAGS_ENUM_LIKE != 0
            && self.get_base_type_of_enum_like_type(source) == target
        {
            return true;
        }
        self.type_types_contains(target, source)
    }
}

impl<'a, 'state, 'c> Relater<'a, 'state, 'c> {
    fn new(c: &'c mut Checker<'a, 'state>) -> Self {
        Relater {
            c,
            relation: None,
            error_node: None,
            error_chain: None,
            error_chains: Vec::new(),
            skip_parent_counter: 0,
            related_info: Vec::new(),
            maybe_keys: Vec::new(),
            maybe_keys_set: collections::Set::new(),
            source_stack: Vec::new(),
            target_stack: Vec::new(),
            expanding_flags: EXPANDING_FLAGS_NONE,
            overflow: false,
            relation_count: 0,
        }
    }

    fn set_relation(&mut self, relation: RelationKind) {
        self.relation = Some(relation);
    }

    fn relation(&self) -> RelationKind {
        self.relation
            .expect("relater relation is set before comparison")
    }

    fn relation_result(&self, key: CacheHashKey) -> RelationComparisonResult {
        self.c.relation_result(self.relation(), key)
    }

    fn set_relation_result(&mut self, key: CacheHashKey, result: RelationComparisonResult) {
        self.c.set_relation_result(self.relation(), key, result);
    }

    fn relation_is(&self, relation: RelationKind) -> bool {
        self.relation() == relation
    }

    fn is_related_to_simple(&mut self, source: TypeHandle, target: TypeHandle) -> Ternary {
        self.is_related_to_ex(
            source,
            target,
            RECURSION_FLAGS_BOTH,
            false, /*reportErrors*/
            None,  /*headMessage*/
            INTERSECTION_STATE_NONE,
        )
    }

    fn is_related_to_worker(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        report_errors: bool,
    ) -> Ternary {
        self.is_related_to_ex(
            source,
            target,
            RECURSION_FLAGS_BOTH,
            report_errors,
            None,
            INTERSECTION_STATE_NONE,
        )
    }

    fn is_related_to(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        recursion_flags: RecursionFlags,
        report_errors: bool,
    ) -> Ternary {
        self.is_related_to_ex(
            source,
            target,
            recursion_flags,
            report_errors,
            None,
            INTERSECTION_STATE_NONE,
        )
    }

    fn is_related_to_ex(
        &mut self,
        original_source: TypeHandle,
        original_target: TypeHandle,
        recursion_flags: RecursionFlags,
        report_errors: bool,
        head_message: Option<&'static diagnostics::Message>,
        intersection_state: IntersectionState,
    ) -> Ternary {
        if original_source == original_target {
            return TERNARY_TRUE;
        }
        // Before normalization: if `source` is type an object type, and `target` is primitive,
        // skip all the checks we don't need and just return `isSimpleTypeRelatedTo` result
        if self.c.type_flags(original_source) & TYPE_FLAGS_OBJECT != 0
            && self.c.type_flags(original_target) & TYPE_FLAGS_PRIMITIVE != 0
        {
            // PORT NOTE: reshaped for borrowck. TS-Go evaluates the same
            // relation pointer and optional reporter while mutating Checker.
            let relation = self.relation();
            let comparable_related = self.relation_is(self.c.semantic_state.comparable_relation)
                && self.c.type_flags(original_target) & TYPE_FLAGS_NEVER == 0
                && self.c.is_simple_type_related_to(
                    original_target,
                    original_source,
                    relation,
                    None,
                );
            let reporter = if report_errors {
                Some(Self::report_error_as_reporter(
                    &mut self.error_chains,
                    &mut self.error_chain,
                ))
            } else {
                None
            };
            if comparable_related
                || self.c.is_simple_type_related_to(
                    original_source,
                    original_target,
                    relation,
                    reporter,
                )
            {
                return TERNARY_TRUE;
            }
            if report_errors {
                self.report_error_results(
                    original_source,
                    original_target,
                    original_source,
                    original_target,
                    head_message,
                );
            }
            return TERNARY_FALSE;
        }
        // Normalize the source and target types: Turn fresh literal types into regular literal types,
        // turn deferred type references into regular type references, simplify indexed access and
        // conditional types, and resolve substitution types to either the substitution (on the source
        // side) or the type variable (on the target side).
        let source = self
            .c
            .get_normalized_type(original_source, false /*writing*/);
        let mut target = self
            .c
            .get_normalized_type(original_target, true /*writing*/);
        if source == target {
            return TERNARY_TRUE;
        }
        if self.relation_is(self.c.semantic_state.identity_relation) {
            if self.c.type_flags(source) != self.c.type_flags(target) {
                return TERNARY_FALSE;
            }
            if self.c.type_flags(source) & TYPE_FLAGS_SINGLETON != 0 {
                return TERNARY_TRUE;
            }
            self.trace_unions_or_intersections_too_large(source, target);
            return self.recursive_type_related_to(
                source,
                target,
                false, /*reportErrors*/
                INTERSECTION_STATE_NONE,
                recursion_flags,
            );
        }
        if self.c.type_flags(source) & TYPE_FLAGS_TYPE_PARAMETER != 0
            && self.c.get_constraint_of_type(source) == Some(target)
        {
            return TERNARY_TRUE;
        }
        if self.c.type_flags(source) & TYPE_FLAGS_DEFINITELY_NON_NULLABLE != 0
            && self.c.type_flags(target) & TYPE_FLAGS_UNION != 0
        {
            let types_len = self.c.type_types_len(target);
            let mut candidate = None;
            if types_len == 2
                && self.c.type_flags(self.c.type_type_at(target, 0)) & TYPE_FLAGS_NULLABLE != 0
            {
                candidate = Some(self.c.type_type_at(target, 1));
            } else if types_len == 3
                && self.c.type_flags(self.c.type_type_at(target, 0)) & TYPE_FLAGS_NULLABLE != 0
                && self.c.type_flags(self.c.type_type_at(target, 1)) & TYPE_FLAGS_NULLABLE != 0
            {
                candidate = Some(self.c.type_type_at(target, 2));
            }
            if let Some(candidate) = candidate {
                if self.c.type_flags(candidate) & TYPE_FLAGS_NULLABLE == 0 {
                    target = self.c.get_normalized_type(candidate /*writing*/, true);
                    if source == target {
                        return TERNARY_TRUE;
                    }
                }
            }
        }
        // PORT NOTE: reshaped for borrowck. This preserves TS-Go's two-step
        // simple relation fast path while avoiding overlapping borrows.
        let relation = self.relation();
        let comparable_related = self.relation_is(self.c.semantic_state.comparable_relation)
            && self.c.type_flags(target) & TYPE_FLAGS_NEVER == 0
            && self
                .c
                .is_simple_type_related_to(target, source, relation, None);
        let reporter = if report_errors {
            Some(Self::report_error_as_reporter(
                &mut self.error_chains,
                &mut self.error_chain,
            ))
        } else {
            None
        };
        if comparable_related
            || self
                .c
                .is_simple_type_related_to(source, target, relation, reporter)
        {
            return TERNARY_TRUE;
        }
        if self.c.type_flags(source) & TYPE_FLAGS_STRUCTURED_OR_INSTANTIABLE != 0
            || self.c.type_flags(target) & TYPE_FLAGS_STRUCTURED_OR_INSTANTIABLE != 0
        {
            let is_performing_excess_property_checks =
                intersection_state & INTERSECTION_STATE_TARGET == 0
                    && is_object_literal_type(self.c, source)
                    && self.c.object_flags(source) & OBJECT_FLAGS_FRESH_LITERAL != 0;
            if is_performing_excess_property_checks
                && self.has_excess_properties(source, target, report_errors)
            {
                if report_errors {
                    self.report_relation_error(
                        head_message,
                        source,
                        if self.c.type_alias_record(original_target).is_some() {
                            original_target
                        } else {
                            target
                        },
                    );
                }
                return TERNARY_FALSE;
            }
            let is_performing_common_property_checks = (!self
                .relation_is(self.c.semantic_state.comparable_relation)
                || is_unit_type(self.c, source))
                && intersection_state & INTERSECTION_STATE_TARGET == 0
                && self.c.type_flags(source)
                    & (TYPE_FLAGS_PRIMITIVE | TYPE_FLAGS_OBJECT | TYPE_FLAGS_INTERSECTION)
                    != 0
                && source != self.c.semantic_state.semantic_handles().global_object_type
                && self.c.type_flags(target) & (TYPE_FLAGS_OBJECT | TYPE_FLAGS_INTERSECTION) != 0
                && self.c.is_weak_type(target)
                && (!self.c.get_properties_of_type(source).is_empty()
                    || self.c.type_has_call_or_construct_signatures(source));
            let is_comparing_jsx_attributes =
                self.c.object_flags(source) & OBJECT_FLAGS_JSX_ATTRIBUTES != 0;
            if is_performing_common_property_checks
                && !self
                    .c
                    .has_common_properties(source, target, is_comparing_jsx_attributes)
            {
                if report_errors {
                    let source_string = self.c.type_to_string_public(
                        if self.c.type_alias_record(original_source).is_some() {
                            original_source
                        } else {
                            source
                        },
                    );
                    let target_string = self.c.type_to_string_public(
                        if self.c.type_alias_record(original_target).is_some() {
                            original_target
                        } else {
                            target
                        },
                    );
                    let calls = self.c.get_signatures_of_type(source, SIGNATURE_KIND_CALL);
                    let constructs = self
                        .c
                        .get_signatures_of_type(source, SIGNATURE_KIND_CONSTRUCT);
                    // PORT NOTE: reshaped for borrowck. TS-Go computes the
                    // return type and immediately recurses through the Relater.
                    let call_related = if calls.is_empty() {
                        false
                    } else {
                        let return_type = self.c.get_return_type_of_signature(calls[0]);
                        self.is_related_to(
                            return_type,
                            target,
                            RECURSION_FLAGS_SOURCE,
                            false, /*reportErrors*/
                        ) != TERNARY_FALSE
                    };
                    let construct_related = if constructs.is_empty() {
                        false
                    } else {
                        let return_type = self.c.get_return_type_of_signature(constructs[0]);
                        self.is_related_to(
                            return_type,
                            target,
                            RECURSION_FLAGS_SOURCE,
                            false, /*reportErrors*/
                        ) != TERNARY_FALSE
                    };
                    if call_related || construct_related {
                        self.report_error(&*diagnostics::VALUE_OF_TYPE_0_HAS_NO_PROPERTIES_IN_COMMON_WITH_TYPE_1_DID_YOU_MEAN_TO_CALL_IT, vec![source_string.into(), target_string.into()]);
                    } else {
                        self.report_error(
                            &*diagnostics::TYPE_0_HAS_NO_PROPERTIES_IN_COMMON_WITH_TYPE_1,
                            vec![source_string.into(), target_string.into()],
                        );
                    }
                }
                return TERNARY_FALSE;
            }
            self.trace_unions_or_intersections_too_large(source, target);
            let skip_caching = self.c.type_flags(source) & TYPE_FLAGS_UNION != 0
                && self.c.type_types_len(source) < 4
                && self.c.type_flags(target) & TYPE_FLAGS_UNION == 0
                || self.c.type_flags(target) & TYPE_FLAGS_UNION != 0
                    && self.c.type_types_len(target) < 4
                    && self.c.type_flags(source) & TYPE_FLAGS_STRUCTURED_OR_INSTANTIABLE == 0;
            let result = if skip_caching {
                self.union_or_intersection_related_to(
                    source,
                    target,
                    report_errors,
                    intersection_state,
                )
            } else {
                self.recursive_type_related_to(
                    source,
                    target,
                    report_errors,
                    intersection_state,
                    recursion_flags,
                )
            };
            if result != TERNARY_FALSE {
                return result;
            }
        }
        if report_errors {
            self.report_error_results(
                original_source,
                original_target,
                source,
                target,
                head_message,
            );
        }
        TERNARY_FALSE
    }

    fn has_excess_properties(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        report_errors: bool,
    ) -> bool {
        if !is_excess_property_check_target(self.c, target)
            || !self.c.no_implicit_any()
                && self.c.object_flags(target) & OBJECT_FLAGS_JS_LITERAL != 0
        {
            // Disable excess property checks on JS literals to simulate having an implicit "index signature" - but only outside of noImplicitAny
            return false;
        }
        let is_comparing_jsx_attributes =
            self.c.object_flags(source) & OBJECT_FLAGS_JSX_ATTRIBUTES != 0;
        let global_object_type = self.c.semantic_state.semantic_handles().global_object_type;
        let target_includes_global_object = self.c.is_type_subset_of(global_object_type, target);
        let target_is_empty_object =
            !is_comparing_jsx_attributes && self.c.is_empty_object_type(target);
        if (self.relation_is(self.c.semantic_state.assignable_relation)
            || self.relation_is(self.c.semantic_state.comparable_relation))
            && (target_includes_global_object || target_is_empty_object)
        {
            return false;
        }
        let mut reduced_target = target;
        let mut check_types = Vec::new();
        if self.c.type_flags(target) & TYPE_FLAGS_UNION != 0 {
            reduced_target = if let Some(matching) = self.c.find_matching_discriminant_type(
                source,
                target,
                Checker::compare_types_assignable_simple,
            ) {
                matching
            } else {
                self.c.filter_primitives_if_contains_non_primitive(target)
            };
            check_types = self.c.distributed_types(reduced_target);
        }
        for prop in self.c.get_properties_of_type(source) {
            let prop_identity = prop;
            let should_check = should_check_as_excess_property(
                self.c,
                prop_identity,
                self.c.type_symbol_identity(source),
            );
            if should_check && !is_ignored_jsx_property(self.c, source, prop_identity) {
                let prop_name = self.c.missing_name_symbol_identity_name(prop_identity);
                let known_property = self.c.is_known_property(
                    reduced_target,
                    &prop_name,
                    is_comparing_jsx_attributes,
                );
                if !known_property {
                    if report_errors {
                        // Report error in terms of object types in the target as those are the only ones
                        // we check in isKnownProperty.
                        let error_target = self
                            .c
                            .filter_type_with_checker(reduced_target, |checker, t| {
                                is_excess_property_check_target(checker, t)
                            });
                        if self.error_node.is_none() {
                            panic!("No errorNode in hasExcessProperties");
                        }
                        let error_node = self.error_node.unwrap();
                        let error_store = self.c.store_for_node(error_node);
                        let error_parent = error_store.parent(error_node);
                        let jsx_error_node = ast::is_jsx_attributes(error_store, error_node)
                            || ast::is_jsx_opening_like_element(error_store, error_node)
                            || {
                                let error_parent = error_parent.expect("JSX error node has parent");
                                ast::is_jsx_opening_like_element(error_store, error_parent)
                            };
                        if jsx_error_node {
                            let value_declaration = self
                                .c
                                .missing_name_symbol_identity_value_declaration(prop_identity);
                            if value_declaration.is_some()
                                && ast::is_jsx_attribute(
                                    self.c.store_for_output_node(value_declaration.unwrap()),
                                    value_declaration.unwrap(),
                                )
                                && ast::get_source_file_of_node(
                                    self.c.store_for_output_node(self.error_node.unwrap()),
                                    self.error_node,
                                ) == ast::get_source_file_of_node(
                                    self.c.store_for_output_node(value_declaration.unwrap()),
                                    value_declaration.and_then(|value_declaration| {
                                        self.c
                                            .store_for_output_node(value_declaration)
                                            .name(value_declaration)
                                    }),
                                )
                            {
                                let value_declaration = value_declaration.unwrap();
                                self.error_node = self
                                    .c
                                    .store_for_output_node(value_declaration)
                                    .name(value_declaration);
                            }
                            let suggestion_symbol =
                                self.c.get_suggested_symbol_for_nonexistent_jsx_attribute(
                                    &prop_name,
                                    error_target,
                                );
                            if let Some(suggestion_symbol) = suggestion_symbol {
                                let error_target_string =
                                    self.c.type_to_string_public(error_target);
                                let suggestion_string =
                                    self.c.symbol_identity_to_string(suggestion_symbol);
                                self.report_error(
                                    &*diagnostics::PROPERTY_0_DOES_NOT_EXIST_ON_TYPE_1_DID_YOU_MEAN_2,
                                    vec![
                                        prop_name.into(),
                                        error_target_string.into(),
                                        suggestion_string.into(),
                                    ],
                                );
                            } else {
                                let error_target_string =
                                    self.c.type_to_string_public(error_target);
                                self.report_error(
                                    &*diagnostics::PROPERTY_0_DOES_NOT_EXIST_ON_TYPE_1,
                                    vec![prop_name.into(), error_target_string.into()],
                                );
                            }
                        } else {
                            let mut object_literal_declaration = None;
                            if let Some(symbol) = self.c.type_symbol_identity(source) {
                                object_literal_declaration =
                                    self.c.first_symbol_identity_declaration(symbol);
                            }
                            let mut suggestion = String::new();
                            let value_declaration = self
                                .c
                                .missing_name_symbol_identity_value_declaration(prop_identity);
                            if value_declaration.is_some()
                                && ast::is_object_literal_element(
                                    self.c.store_for_output_node(value_declaration.unwrap()),
                                    value_declaration.unwrap(),
                                )
                                && ast::find_ancestor(
                                    self.c.store_for_output_node(value_declaration.unwrap()),
                                    value_declaration,
                                    |_, d| {
                                        object_literal_declaration
                                            .as_ref()
                                            .is_some_and(|declaration| *declaration == d)
                                    },
                                )
                                .is_some()
                                && object_literal_declaration.as_ref().is_some_and(
                                    |object_literal_declaration| {
                                        ast::get_source_file_of_node(
                                            self.c
                                                .store_for_output_node(*object_literal_declaration),
                                            Some(*object_literal_declaration),
                                        ) == ast::get_source_file_of_node(
                                            self.c.store_for_output_node(self.error_node.unwrap()),
                                            self.error_node,
                                        )
                                    },
                                )
                            {
                                let value_declaration = value_declaration.unwrap();
                                let value_declaration_store =
                                    self.c.store_for_output_node(value_declaration);
                                let name = value_declaration_store.name(value_declaration).unwrap();
                                self.error_node = Some(name);
                                if ast::is_identifier(value_declaration_store, name) {
                                    suggestion = self.c.get_suggestion_for_nonexistent_property(
                                        value_declaration_store.text(name).into(),
                                        error_target,
                                    );
                                }
                            }
                            let prop_string = self.c.symbol_identity_to_string(prop_identity);
                            if !suggestion.is_empty() {
                                let error_target_string =
                                    self.c.type_to_string_public(error_target);
                                self.report_parent_skipped_error(&*diagnostics::OBJECT_LITERAL_MAY_ONLY_SPECIFY_KNOWN_PROPERTIES_BUT_0_DOES_NOT_EXIST_IN_TYPE_1_DID_YOU_MEAN_TO_WRITE_2, vec![prop_string.into(), error_target_string.into(), suggestion.into()]);
                            } else {
                                let error_target_string =
                                    self.c.type_to_string_public(error_target);
                                self.report_parent_skipped_error(&*diagnostics::OBJECT_LITERAL_MAY_ONLY_SPECIFY_KNOWN_PROPERTIES_AND_0_DOES_NOT_EXIST_IN_TYPE_1, vec![prop_string.into(), error_target_string.into()]);
                            }
                        }
                    }
                    return true;
                }
                if !check_types.is_empty() && {
                    let source_type = self.c.relater_get_type_of_symbol(prop_identity);
                    let target_type = self
                        .c
                        .get_type_of_property_in_types(check_types.clone(), &prop_name);
                    self.is_related_to(
                        source_type,
                        target_type,
                        RECURSION_FLAGS_BOTH,
                        report_errors,
                    ) == TERNARY_FALSE
                } {
                    if report_errors {
                        let prop_string = self.c.symbol_identity_to_string(prop_identity);
                        self.report_error(
                            &*diagnostics::TYPES_OF_PROPERTY_0_ARE_INCOMPATIBLE,
                            vec![prop_string.into()],
                        );
                    }
                    return true;
                }
            }
        }
        false
    }

    fn union_or_intersection_related_to(
        &mut self,
        mut source: TypeHandle,
        target: TypeHandle,
        report_errors: bool,
        intersection_state: IntersectionState,
    ) -> Ternary {
        // Note that these checks are specifically ordered to produce correct results. In particular,
        // we need to deconstruct unions before intersections (because unions are always at the top),
        // and we need to handle "each" relations before "some" relations for the same kind of type.
        if self.c.type_flags(source) & TYPE_FLAGS_UNION != 0 {
            if self.c.type_flags(target) & TYPE_FLAGS_UNION != 0 {
                let source_origin = self.c.type_record(source).as_union_type().origin;
                if source_origin.is_some()
                    && self.c.type_flags(source_origin.unwrap()) & TYPE_FLAGS_INTERSECTION != 0
                    && self.c.type_alias_record(target).is_some()
                    && self
                        .c
                        .type_types_contains_exact(source_origin.unwrap(), target)
                {
                    return TERNARY_TRUE;
                }
                let target_origin = self.c.type_record(target).as_union_type().origin;
                if target_origin.is_some()
                    && self.c.type_flags(target_origin.unwrap()) & TYPE_FLAGS_UNION != 0
                    && self.c.type_alias_record(source).is_some()
                    && self
                        .c
                        .type_types_contains_exact(target_origin.unwrap(), source)
                {
                    return TERNARY_TRUE;
                }
            }
            if self.relation_is(self.c.semantic_state.comparable_relation) {
                return self.some_type_related_to_type(
                    source,
                    target,
                    report_errors && self.c.type_flags(source) & TYPE_FLAGS_PRIMITIVE == 0,
                    intersection_state,
                );
            }
            return self.each_type_related_to_type(
                source,
                target,
                report_errors && self.c.type_flags(source) & TYPE_FLAGS_PRIMITIVE == 0,
                intersection_state,
            );
        }
        if self.c.type_flags(target) & TYPE_FLAGS_UNION != 0 {
            let report_errors = report_errors
                && self.c.type_flags(source) & TYPE_FLAGS_PRIMITIVE == 0
                && self.c.type_flags(target) & TYPE_FLAGS_PRIMITIVE == 0;
            let source = self.c.get_regular_type_of_object_literal(source);
            return self.type_related_to_some_type(
                source,
                target,
                report_errors,
                intersection_state,
            );
        }
        if self.c.type_flags(target) & TYPE_FLAGS_INTERSECTION != 0 {
            return self.type_related_to_each_type(
                source,
                target,
                report_errors,
                INTERSECTION_STATE_TARGET,
            );
        }
        if self.relation_is(self.c.semantic_state.comparable_relation)
            && self.c.type_flags(target) & TYPE_FLAGS_PRIMITIVE != 0
        {
            let source_types_len = self.c.type_types_len(source);
            let mut constraints = Vec::with_capacity(source_types_len);
            let mut changed = false;
            // PORT NOTE: reshaped for borrowck. This is TS-Go's sameMap over
            // the source types, with Checker calls kept outside a Fn closure.
            for index in 0..source_types_len {
                let t = self.c.type_type_at(source, index);
                if self.c.type_flags(t) & TYPE_FLAGS_INSTANTIABLE != 0 {
                    let constraint = self
                        .c
                        .get_base_constraint_of_type(t)
                        .unwrap_or(self.c.semantic_state.semantic_handles().unknown_type);
                    changed |= constraint != t;
                    constraints.push(constraint);
                } else {
                    constraints.push(t);
                }
            }
            if changed {
                source = self.c.get_intersection_type(constraints);
                if self.c.type_flags(source) & TYPE_FLAGS_NEVER != 0 {
                    return TERNARY_FALSE;
                }
                if self.c.type_flags(source) & TYPE_FLAGS_INTERSECTION == 0 {
                    let result = self.is_related_to(
                        source,
                        target,
                        RECURSION_FLAGS_SOURCE,
                        false, /*reportErrors*/
                    );
                    if result != TERNARY_FALSE {
                        return result;
                    }
                    return self.is_related_to(
                        target,
                        source,
                        RECURSION_FLAGS_SOURCE,
                        false, /*reportErrors*/
                    );
                }
            }
        }
        self.some_type_related_to_type(
            source,
            target,
            false, /*reportErrors*/
            INTERSECTION_STATE_SOURCE,
        )
    }

    fn some_type_related_to_type(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        report_errors: bool,
        intersection_state: IntersectionState,
    ) -> Ternary {
        if self.c.type_flags(source) & TYPE_FLAGS_UNION != 0
            && self.c.type_types_contains(source, target)
        {
            return TERNARY_TRUE;
        }
        let source_types_len = self.c.type_types_len(source);
        for i in 0..source_types_len {
            let t = self.c.type_type_at(source, i);
            let related = self.is_related_to_ex(
                t,
                target,
                RECURSION_FLAGS_SOURCE,
                report_errors && i == source_types_len - 1,
                None, /*headMessage*/
                intersection_state,
            );
            if related != TERNARY_FALSE {
                return related;
            }
        }
        TERNARY_FALSE
    }

    fn each_type_related_to_type(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        report_errors: bool,
        intersection_state: IntersectionState,
    ) -> Ternary {
        let mut result = TERNARY_TRUE;
        let source_types_len = self.c.type_types_len(source);
        let stripped_target = self.get_undefined_stripped_target_if_needed(source, target);
        let stripped_target_is_union = self.c.type_flags(stripped_target) & TYPE_FLAGS_UNION != 0;
        let stripped_types_len = if stripped_target_is_union {
            self.c.type_types_len(stripped_target)
        } else {
            0
        };
        for i in 0..source_types_len {
            let source_type = self.c.type_type_at(source, i);
            if stripped_target_is_union
                && stripped_types_len != 0
                && source_types_len >= stripped_types_len
                && source_types_len % stripped_types_len == 0
            {
                let stripped_type = self.c.type_type_at(stripped_target, i % stripped_types_len);
                let related = self.is_related_to_ex(
                    source_type,
                    stripped_type,
                    RECURSION_FLAGS_BOTH,
                    false, /*reportErrors*/
                    None,  /*headMessage*/
                    intersection_state,
                );
                if related != TERNARY_FALSE {
                    result &= related;
                    continue;
                }
            }
            let related = self.is_related_to_ex(
                source_type,
                target,
                RECURSION_FLAGS_SOURCE,
                report_errors,
                None, /*headMessage*/
                intersection_state,
            );
            if related == TERNARY_FALSE {
                return TERNARY_FALSE;
            }
            result &= related;
        }
        result
    }

    fn get_undefined_stripped_target_if_needed(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
    ) -> TypeHandle {
        if self.c.type_flags(source) & TYPE_FLAGS_UNION != 0
            && self.c.type_flags(target) & TYPE_FLAGS_UNION != 0
            && self.c.type_flags(self.c.type_type_at(source, 0)) & TYPE_FLAGS_UNDEFINED == 0
            && self.c.type_flags(self.c.type_type_at(target, 0)) & TYPE_FLAGS_UNDEFINED != 0
        {
            return self.c.extract_types_of_kind(target, !TYPE_FLAGS_UNDEFINED);
        }
        target
    }
}

pub(crate) fn should_check_as_excess_property(
    checker: &Checker<'_, '_>,
    prop: SymbolIdentity,
    container: Option<SymbolIdentity>,
) -> bool {
    if checker.missing_name_symbol_identity_name(prop) == ast::INTERNAL_SYMBOL_NAME_MISSING {
        return false;
    }
    let prop_value_declaration = checker.missing_name_symbol_identity_value_declaration(prop);
    let container_value_declaration =
        container.and_then(|c| checker.missing_name_symbol_identity_value_declaration(c));
    prop_value_declaration.is_some()
        && container_value_declaration.is_some()
        && prop_value_declaration.as_ref().is_some_and(|declaration| {
            let store = checker.store_for_output_node(*declaration);
            store.parent(*declaration).is_some_and(|parent| {
                container_value_declaration
                    .as_ref()
                    .is_some_and(|declaration| *declaration == parent)
            })
        })
}

pub(crate) fn is_ignored_jsx_property(
    checker: &Checker<'_, '_>,
    source: TypeHandle,
    source_prop: SymbolIdentity,
) -> bool {
    checker.object_flags(source) & OBJECT_FLAGS_JSX_ATTRIBUTES != 0
        && is_hyphenated_jsx_name(&checker.missing_name_symbol_identity_name(source_prop))
}

impl<'a, 'state, 'c> Relater<'a, 'state, 'c> {
    fn type_related_to_some_type(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        report_errors: bool,
        intersection_state: IntersectionState,
    ) -> Ternary {
        let target_types_len = self.c.type_types_len(target);
        if self.c.type_flags(target) & TYPE_FLAGS_UNION != 0 {
            if self.c.type_types_contains(target, source) {
                return TERNARY_TRUE;
            }
            if !self.relation_is(self.c.semantic_state.comparable_relation)
                && self.c.object_flags(target) & OBJECT_FLAGS_PRIMITIVE_UNION != 0
                && self.c.type_flags(source) & TYPE_FLAGS_ENUM_LITERAL == 0
                && (self.c.type_flags(source)
                    & (TYPE_FLAGS_STRING_LITERAL
                        | TYPE_FLAGS_BOOLEAN_LITERAL
                        | TYPE_FLAGS_BIG_INT_LITERAL)
                    != 0
                    || (self.relation_is(self.c.semantic_state.subtype_relation)
                        || self.relation_is(self.c.semantic_state.strict_subtype_relation))
                        && self.c.type_flags(source) & TYPE_FLAGS_NUMBER_LITERAL != 0)
            {
                let source_literal = self.c.type_record(source).as_literal_type();
                let alternate_form = if Some(source) == source_literal.regular_type {
                    source_literal.fresh_type
                } else {
                    source_literal.regular_type
                };
                let mut primitive = None;
                if self.c.type_flags(source) & TYPE_FLAGS_STRING_LITERAL != 0 {
                    primitive = Some(self.c.semantic_state.semantic_handles().string_type);
                } else if self.c.type_flags(source) & TYPE_FLAGS_NUMBER_LITERAL != 0 {
                    primitive = Some(self.c.semantic_state.semantic_handles().number_type);
                } else if self.c.type_flags(source) & TYPE_FLAGS_BIG_INT_LITERAL != 0 {
                    primitive = Some(self.c.semantic_state.semantic_handles().bigint_type);
                }
                if primitive.is_some() && self.c.type_types_contains(target, primitive.unwrap())
                    || alternate_form.is_some_and(|alternate_form| {
                        self.c.type_types_contains(target, alternate_form)
                    })
                {
                    return TERNARY_TRUE;
                }
                return TERNARY_FALSE;
            }
            if let Some(match_) = self
                .c
                .get_matching_union_constituent_for_type(target, source)
            {
                let related = self.is_related_to_ex(
                    source,
                    match_,
                    RECURSION_FLAGS_TARGET,
                    false, /*reportErrors*/
                    None,  /*headMessage*/
                    intersection_state,
                );
                if related != TERNARY_FALSE {
                    return related;
                }
            }
        }
        for index in 0..target_types_len {
            let t = self.c.type_type_at(target, index);
            let related = self.is_related_to_ex(
                source,
                t,
                RECURSION_FLAGS_TARGET,
                false, /*reportErrors*/
                None,  /*headMessage*/
                intersection_state,
            );
            if related != TERNARY_FALSE {
                return related;
            }
        }
        if report_errors {
            // Elaborate only if we can find a best matching type in the target union
            let best_matching_type = self.c.get_best_matching_type(
                source,
                target,
                Checker::compare_types_assignable_simple,
            );
            if let Some(best_matching_type) = best_matching_type {
                self.is_related_to_ex(
                    source,
                    best_matching_type,
                    RECURSION_FLAGS_TARGET,
                    true, /*reportErrors*/
                    None, /*headMessage*/
                    intersection_state,
                );
            }
        }
        TERNARY_FALSE
    }

    fn type_related_to_each_type(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        report_errors: bool,
        intersection_state: IntersectionState,
    ) -> Ternary {
        let mut result = TERNARY_TRUE;
        let target_types_len = self.c.type_types_len(target);
        for index in 0..target_types_len {
            let target_type = self.c.type_type_at(target, index);
            let related = self.is_related_to_ex(
                source,
                target_type,
                RECURSION_FLAGS_TARGET,
                report_errors, /*headMessage*/
                None,
                intersection_state,
            );
            if related == TERNARY_FALSE {
                return TERNARY_FALSE;
            }
            result &= related;
        }
        result
    }

    fn each_type_related_to_some_type(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
    ) -> Ternary {
        let mut result = TERNARY_TRUE;
        let source_types_len = self.c.type_types_len(source);
        for index in 0..source_types_len {
            let source_type = self.c.type_type_at(source, index);
            let related = self.type_related_to_some_type(
                source_type,
                target,
                false, /*reportErrors*/
                INTERSECTION_STATE_NONE,
            );
            if related == TERNARY_FALSE {
                return TERNARY_FALSE;
            }
            result &= related;
        }
        result
    }

    fn reset_maybe_stack(
        &mut self,
        maybe_start: usize,
        propagating_variance_flags: RelationComparisonResult,
        mark_all_as_succeeded: bool,
    ) {
        for i in maybe_start..self.maybe_keys.len() {
            let maybe_key = self.maybe_keys[i];
            self.maybe_keys_set.delete(&maybe_key);
            if mark_all_as_succeeded {
                self.set_relation_result(
                    maybe_key,
                    RELATION_COMPARISON_RESULT_SUCCEEDED | propagating_variance_flags,
                );
                self.relation_count -= 1;
            }
        }
        self.maybe_keys.truncate(maybe_start);
    }

    fn get_error_state(&self) -> ErrorState {
        ErrorState {
            error_chain: self.error_chain,
            related_info: self.related_info.clone(),
        }
    }

    fn restore_error_state(&mut self, e: &ErrorState) {
        self.error_chain = e.error_chain;
        self.related_info = e.related_info.clone();
    }

    fn is_source_intersection_needing_extra_check(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
    ) -> bool {
        if self.c.type_flags(source) & TYPE_FLAGS_INTERSECTION == 0 {
            return false;
        }
        let apparent = self.c.get_apparent_type(source);
        if self.c.type_flags(apparent) & TYPE_FLAGS_STRUCTURED_TYPE == 0 {
            return false;
        }
        let source_types_len = self.c.type_types_len(source);
        for index in 0..source_types_len {
            let t = self.c.type_type_at(source, index);
            if t == target || self.c.object_flags(t) & OBJECT_FLAGS_NON_INFERRABLE_TYPE != 0 {
                return false;
            }
        }
        true
    }

    fn try_elaborate_array_like_errors(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        report_errors: bool,
    ) -> bool {
        /**
         * The spec for elaboration is:
         * - If the source is a readonly tuple and the target is a mutable array or tuple, elaborate on mutability and skip property elaborations.
         * - If the source is a tuple then skip property elaborations if the target is an array or tuple.
         * - If the source is a readonly array and the target is a mutable array or tuple, elaborate on mutability and skip property elaborations.
         * - If the source an array then skip property elaborations if the target is a tuple.
         */
        if self.c.is_tuple_type(source) {
            if self.c.target_tuple_type_record(source).readonly
                && self.c.is_mutable_array_or_tuple(target)
            {
                if report_errors {
                    let source_string = self.c.type_to_string_public(source);
                    let target_string = self.c.type_to_string_public(target);
                    self.report_error(&diagnostics::THE_TYPE_0_IS_READONLY_AND_CANNOT_BE_ASSIGNED_TO_THE_MUTABLE_TYPE_1, vec![source_string.into(), target_string.into()]);
                }
                return false;
            }
            return self.c.is_array_or_tuple_type(target);
        }
        if self.c.is_readonly_array_type(source) && self.c.is_mutable_array_or_tuple(target) {
            if report_errors {
                let source_string = self.c.type_to_string_public(source);
                let target_string = self.c.type_to_string_public(target);
                self.report_error(&diagnostics::THE_TYPE_0_IS_READONLY_AND_CANNOT_BE_ASSIGNED_TO_THE_MUTABLE_TYPE_1, vec![source_string.into(), target_string.into()]);
            }
            return false;
        }
        if self.c.is_tuple_type(target) {
            return self.c.is_array_type(source);
        }
        true
    }

    fn try_elaborate_errors_for_primitives_and_objects(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
    ) {
        if (source == self.c.semantic_state.semantic_handles().global_string_type
            && target == self.c.semantic_state.semantic_handles().string_type)
            || (source == self.c.semantic_state.semantic_handles().global_number_type
                && target == self.c.semantic_state.semantic_handles().number_type)
            || (source == self.c.semantic_state.semantic_handles().global_boolean_type
                && target == self.c.semantic_state.semantic_handles().boolean_type)
            || {
                let resolver = (self.c.semantic_state.get_global_es_symbol_type).clone();
                source == self.c.resolve_global_type(resolver)
                    && target == self.c.semantic_state.semantic_handles().es_symbol_type
            }
        {
            let target_string = self.c.type_to_string_public(target);
            let source_string = self.c.type_to_string_public(source);
            self.report_error(&*diagnostics::X_0_IS_A_PRIMITIVE_BUT_1_IS_A_WRAPPER_OBJECT_PREFER_USING_0_WHEN_POSSIBLE, vec![target_string.into(), source_string.into()]);
        }
    }

    fn constructor_visibilities_are_compatible(
        &mut self,
        source_signature: SignatureHandle,
        target_signature: SignatureHandle,
        report_errors: bool,
    ) -> bool {
        let source_signature_record = self.c.signature_record(source_signature);
        let target_signature_record = self.c.signature_record(target_signature);
        if source_signature_record.declaration.is_none()
            || target_signature_record.declaration.is_none()
        {
            return true;
        }
        let source_declaration = source_signature_record.declaration.unwrap();
        let target_declaration = target_signature_record.declaration.unwrap();
        let source_accessibility = self
            .c
            .store_for_node(source_declaration)
            .modifiers(source_declaration)
            .map_or(ast::MODIFIER_FLAGS_NONE, |modifiers| {
                modifiers.modifier_flags()
            })
            & ast::MODIFIER_FLAGS_NON_PUBLIC_ACCESSIBILITY_MODIFIER;
        let target_accessibility = self
            .c
            .store_for_node(target_declaration)
            .modifiers(target_declaration)
            .map_or(ast::MODIFIER_FLAGS_NONE, |modifiers| {
                modifiers.modifier_flags()
            })
            & ast::MODIFIER_FLAGS_NON_PUBLIC_ACCESSIBILITY_MODIFIER;
        // A public, protected and private signature is assignable to a private signature.
        if target_accessibility == ast::MODIFIER_FLAGS_PRIVATE {
            return true;
        }
        // A public and protected signature is assignable to a protected signature.
        if target_accessibility == ast::MODIFIER_FLAGS_PROTECTED
            && source_accessibility != ast::MODIFIER_FLAGS_PRIVATE
        {
            return true;
        }
        // Only a public signature is assignable to public signature.
        if target_accessibility != ast::MODIFIER_FLAGS_PROTECTED && source_accessibility == 0 {
            return true;
        }
        if report_errors {
            self.report_error(
                &*diagnostics::CANNOT_ASSIGN_A_0_CONSTRUCTOR_TYPE_TO_A_1_CONSTRUCTOR_TYPE,
                vec![
                    visibility_to_string(source_accessibility).into(),
                    visibility_to_string(target_accessibility).into(),
                ],
            );
        }
        false
    }

    // See signatureAssignableTo, compareSignaturesIdentical
    fn signature_related_to(
        &mut self,
        mut source: SignatureHandle,
        mut target: SignatureHandle,
        erase: bool,
        report_errors: bool,
        intersection_state: IntersectionState,
    ) -> Ternary {
        let mut check_mode = SIGNATURE_CHECK_MODE_NONE;
        if self.relation_is(self.c.semantic_state.subtype_relation) {
            check_mode = SIGNATURE_CHECK_MODE_STRICT_TOP_SIGNATURE;
        } else if self.relation_is(self.c.semantic_state.strict_subtype_relation) {
            check_mode =
                SIGNATURE_CHECK_MODE_STRICT_TOP_SIGNATURE | SIGNATURE_CHECK_MODE_STRICT_ARITY;
        }
        if erase {
            source = self.c.get_erased_signature(source);
            target = self.c.get_erased_signature(target);
        }
        self.compare_signatures_related_in_current_relation(
            source,
            target,
            check_mode,
            report_errors,
            Some(
                self.c
                    .semantic_state
                    .semantic_handles()
                    .report_unreliable_mapper,
            ),
            intersection_state,
        )
    }

    fn compare_signature_types_in_current_relation(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        report_errors: bool,
        intersection_state: IntersectionState,
    ) -> Ternary {
        self.is_related_to_ex(
            source,
            target,
            RECURSION_FLAGS_BOTH,
            report_errors,
            None,
            intersection_state,
        )
    }

    fn compare_signatures_related_in_current_relation(
        &mut self,
        mut source: SignatureHandle,
        mut target: SignatureHandle,
        check_mode: SignatureCheckMode,
        report_errors: bool,
        report_unreliable_markers: Option<TypeMapperHandle>,
        intersection_state: IntersectionState,
    ) -> Ternary {
        if source == target {
            return TERNARY_TRUE;
        }
        if !(check_mode & SIGNATURE_CHECK_MODE_STRICT_TOP_SIGNATURE != 0
            && self.c.is_top_signature(source))
            && self.c.is_top_signature(target)
        {
            return TERNARY_TRUE;
        }
        if check_mode & SIGNATURE_CHECK_MODE_STRICT_TOP_SIGNATURE != 0
            && self.c.is_top_signature(source)
            && !self.c.is_top_signature(target)
        {
            return TERNARY_FALSE;
        }
        let target_count = self.c.get_parameter_count(target);
        let mut source_has_more_parameters = false;
        if !self.c.has_effective_rest_parameter(target) {
            if check_mode & SIGNATURE_CHECK_MODE_STRICT_ARITY != 0 {
                source_has_more_parameters = self.c.has_effective_rest_parameter(source)
                    || self.c.get_parameter_count(source) > target_count;
            } else {
                source_has_more_parameters = self.c.get_min_argument_count(source) > target_count;
            }
        }
        if source_has_more_parameters {
            if report_errors && (check_mode & SIGNATURE_CHECK_MODE_STRICT_ARITY == 0) {
                let min_argument_count = self.c.get_min_argument_count(source);
                self.report_error(
                    &*diagnostics::TARGET_SIGNATURE_PROVIDES_TOO_FEW_ARGUMENTS_EXPECTED_0_OR_MORE_BUT_GOT_1,
                    vec![min_argument_count.into(), target_count.into()],
                );
            }
            return TERNARY_FALSE;
        }
        let source_type_parameters = self.c.signature_record(source).type_parameters.clone();
        let target_type_parameters = self.c.signature_record(target).type_parameters.clone();
        if !source_type_parameters.is_empty()
            && !core::same(&source_type_parameters, &target_type_parameters)
        {
            target = self.c.get_canonical_signature(target);
            source = self.instantiate_signature_in_context_of_current_relation(
                source,
                target,
                None,
                intersection_state,
            );
        }
        let source_count = self.c.get_parameter_count(source);
        let source_rest_type = self.c.get_non_array_rest_type(source);
        let target_rest_type = self.c.get_non_array_rest_type(target);
        if source_rest_type.is_some() || target_rest_type.is_some() {
            self.c.instantiate_type_with_mapper_handle(
                source_rest_type.or(target_rest_type),
                report_unreliable_markers,
            );
        }
        let mut kind = ast::Kind::Unknown;
        if let Some(declaration) = self.c.signature_record(target).declaration {
            kind = self.c.store_for_node(declaration).kind(declaration);
        }
        let strict_variance = check_mode & SIGNATURE_CHECK_MODE_CALLBACK == 0
            && self.c.strict_function_types()
            && kind != ast::Kind::MethodDeclaration
            && kind != ast::Kind::MethodSignature
            && kind != ast::Kind::Constructor;
        let mut result = TERNARY_TRUE;
        let source_this_type = self.c.get_this_type_of_signature(source);
        if source_this_type.is_some()
            && source_this_type != Some(self.c.semantic_state.semantic_handles().void_type)
        {
            let target_this_type = self.c.get_this_type_of_signature(target);
            if let Some(target_this_type) = target_this_type {
                let mut related = TERNARY_FALSE;
                if !strict_variance {
                    related = self.compare_signature_types_in_current_relation(
                        source_this_type.unwrap(),
                        target_this_type,
                        false,
                        intersection_state,
                    );
                }
                if related == TERNARY_FALSE {
                    related = self.compare_signature_types_in_current_relation(
                        target_this_type,
                        source_this_type.unwrap(),
                        report_errors,
                        intersection_state,
                    );
                }
                if related == TERNARY_FALSE {
                    if report_errors {
                        self.report_error(
                            &*diagnostics::THE_THIS_TYPES_OF_EACH_SIGNATURE_ARE_INCOMPATIBLE,
                            vec![],
                        );
                    }
                    return TERNARY_FALSE;
                }
                result &= related;
            }
        }
        let param_count = if source_rest_type.is_some() || target_rest_type.is_some() {
            source_count.min(target_count)
        } else {
            source_count.max(target_count)
        };
        let rest_index = if source_rest_type.is_some() || target_rest_type.is_some() {
            param_count as i32 - 1
        } else {
            -1
        };
        for i in 0..param_count {
            let source_type = if i as i32 == rest_index {
                self.c.get_rest_or_any_type_at_position(source, i)
            } else {
                self.c.try_get_type_at_position(source, i)
            };
            let target_type = if i as i32 == rest_index {
                self.c.get_rest_or_any_type_at_position(target, i)
            } else {
                self.c.try_get_type_at_position(target, i)
            };
            if source_type.is_some()
                && target_type.is_some()
                && (source_type != target_type
                    || check_mode & SIGNATURE_CHECK_MODE_STRICT_ARITY != 0)
            {
                let mut source_sig = None;
                let mut target_sig = None;
                if check_mode & SIGNATURE_CHECK_MODE_CALLBACK == 0
                    && !self.c.is_instantiated_generic_parameter(source, i)
                {
                    let non_nullable_source_type =
                        self.c.get_non_nullable_type(source_type.unwrap());
                    source_sig = self.c.get_single_call_signature(non_nullable_source_type);
                }
                if check_mode & SIGNATURE_CHECK_MODE_CALLBACK == 0
                    && !self.c.is_instantiated_generic_parameter(target, i)
                {
                    let non_nullable_target_type =
                        self.c.get_non_nullable_type(target_type.unwrap());
                    target_sig = self.c.get_single_call_signature(non_nullable_target_type);
                }
                let callbacks = source_sig.is_some()
                    && target_sig.is_some()
                    && self
                        .c
                        .get_type_predicate_of_signature(source_sig.unwrap())
                        .is_none()
                    && self
                        .c
                        .get_type_predicate_of_signature(target_sig.unwrap())
                        .is_none()
                    && self
                        .c
                        .get_type_facts(source_type.unwrap(), TYPE_FACTS_IS_UNDEFINED_OR_NULL)
                        == self
                            .c
                            .get_type_facts(target_type.unwrap(), TYPE_FACTS_IS_UNDEFINED_OR_NULL);
                let mut related = TERNARY_FALSE;
                if callbacks {
                    related = self.compare_signatures_related_in_current_relation(
                        target_sig.unwrap(),
                        source_sig.unwrap(),
                        check_mode & SIGNATURE_CHECK_MODE_STRICT_ARITY
                            | if strict_variance {
                                SIGNATURE_CHECK_MODE_STRICT_CALLBACK
                            } else {
                                SIGNATURE_CHECK_MODE_BIVARIANT_CALLBACK
                            },
                        report_errors,
                        report_unreliable_markers,
                        intersection_state,
                    );
                } else {
                    if check_mode & SIGNATURE_CHECK_MODE_CALLBACK == 0 && !strict_variance {
                        related = self.compare_signature_types_in_current_relation(
                            source_type.unwrap(),
                            target_type.unwrap(),
                            false,
                            intersection_state,
                        );
                    }
                    if related == TERNARY_FALSE {
                        related = self.compare_signature_types_in_current_relation(
                            target_type.unwrap(),
                            source_type.unwrap(),
                            report_errors,
                            intersection_state,
                        );
                    }
                }
                if related != TERNARY_FALSE
                    && check_mode & SIGNATURE_CHECK_MODE_STRICT_ARITY != 0
                    && i >= self.c.get_min_argument_count(source)
                    && i < self.c.get_min_argument_count(target)
                    && self.compare_signature_types_in_current_relation(
                        source_type.unwrap(),
                        target_type.unwrap(),
                        false,
                        intersection_state,
                    ) != TERNARY_FALSE
                {
                    related = TERNARY_FALSE;
                }
                if related == TERNARY_FALSE {
                    if report_errors {
                        let source_name = self.c.get_parameter_name_at_position(source, i);
                        let target_name = self.c.get_parameter_name_at_position(target, i);
                        self.report_error(
                            &*diagnostics::TYPES_OF_PARAMETERS_0_AND_1_ARE_INCOMPATIBLE,
                            vec![source_name.into(), target_name.into()],
                        );
                    }
                    return TERNARY_FALSE;
                }
                result &= related;
            }
        }
        if check_mode & SIGNATURE_CHECK_MODE_IGNORE_RETURN_TYPES == 0 {
            let target_return_type = self.c.get_non_circular_return_type_of_signature(target);
            if target_return_type == self.c.semantic_state.semantic_handles().void_type
                || target_return_type == self.c.semantic_state.semantic_handles().any_type
            {
                return result;
            }
            let source_return_type = self.c.get_non_circular_return_type_of_signature(source);
            let target_type_predicate = self.c.get_type_predicate_of_signature(target);
            if let Some(target_type_predicate) = target_type_predicate {
                let source_type_predicate = self.c.get_type_predicate_of_signature(source);
                if let Some(source_type_predicate) = source_type_predicate {
                    result &= self.compare_type_predicate_related_to_in_current_relation(
                        source_type_predicate,
                        target_type_predicate,
                        report_errors,
                        intersection_state,
                    );
                } else if {
                    let target_record = self.c.type_predicate_record(target_type_predicate);
                    target_record.kind == TYPE_PREDICATE_KIND_IDENTIFIER
                        || target_record.kind == TYPE_PREDICATE_KIND_THIS
                } {
                    if report_errors {
                        let source_string = self.c.signature_to_string(source);
                        self.report_error(
                            &*diagnostics::SIGNATURE_0_MUST_BE_A_TYPE_PREDICATE,
                            vec![source_string.into()],
                        );
                    }
                    return TERNARY_FALSE;
                }
            } else {
                let mut related = TERNARY_FALSE;
                if check_mode & SIGNATURE_CHECK_MODE_BIVARIANT_CALLBACK != 0 {
                    related = self.compare_signature_types_in_current_relation(
                        target_return_type,
                        source_return_type,
                        false,
                        intersection_state,
                    );
                }
                if related == TERNARY_FALSE {
                    related = self.compare_signature_types_in_current_relation(
                        source_return_type,
                        target_return_type,
                        report_errors,
                        intersection_state,
                    );
                }
                result &= related;
                if result == TERNARY_FALSE && report_errors {
                    let source_record = self.c.signature_record(source);
                    let target_record = self.c.signature_record(target);
                    let message = if source_record.parameters.is_empty()
                        && target_record.parameters.is_empty()
                    {
                        if source_record.flags & SIGNATURE_FLAGS_CONSTRUCT != 0 {
                            &*diagnostics::CONSTRUCT_SIGNATURES_WITH_NO_ARGUMENTS_HAVE_INCOMPATIBLE_RETURN_TYPES_0_AND_1
                        } else {
                            &*diagnostics::CALL_SIGNATURES_WITH_NO_ARGUMENTS_HAVE_INCOMPATIBLE_RETURN_TYPES_0_AND_1
                        }
                    } else if source_record.flags & SIGNATURE_FLAGS_CONSTRUCT != 0 {
                        &*diagnostics::CONSTRUCT_SIGNATURE_RETURN_TYPES_0_AND_1_ARE_INCOMPATIBLE
                    } else {
                        &*diagnostics::CALL_SIGNATURE_RETURN_TYPES_0_AND_1_ARE_INCOMPATIBLE
                    };
                    let source_type_string = self.c.type_to_string(source_return_type, None);
                    let target_type_string = self.c.type_to_string(target_return_type, None);
                    self.report_error(
                        message,
                        vec![source_type_string.into(), target_type_string.into()],
                    );
                }
            }
        }
        result
    }

    fn instantiate_signature_in_context_of_current_relation(
        &mut self,
        signature: SignatureHandle,
        contextual_signature: SignatureHandle,
        inference_context: Option<InferenceContextRef>,
        intersection_state: IntersectionState,
    ) -> SignatureHandle {
        let type_parameters = self.c.get_type_parameters_for_mapper(signature);
        let context = self.c.new_inference_context(
            type_parameters,
            Some(signature),
            INFERENCE_FLAGS_NONE,
            Some(self.c.semantic_state.compare_types_assignable),
        );
        let rest_type = self.c.get_effective_rest_type(contextual_signature);
        let mut mapper = None;
        if let Some(inference_context) = inference_context.as_ref() {
            if rest_type.is_some()
                && self.c.type_flags(rest_type.unwrap()) & TYPE_FLAGS_TYPE_PARAMETER != 0
            {
                mapper = self
                    .c
                    .inference_context_record(*inference_context)
                    .non_fixing_mapper;
            } else {
                mapper = self.c.inference_context_record(*inference_context).mapper;
            }
        }
        let source_signature = if let Some(mapper) = mapper {
            self.c
                .instantiate_signature_with_mapper_handle(contextual_signature, mapper)
        } else {
            contextual_signature
        };
        let mut inferences =
            std::mem::take(&mut self.c.inference_context_record_mut(context).inferences);
        self.c
            .apply_to_parameter_types(source_signature, signature, |checker, source, target| {
                checker.infer_types(
                    &mut inferences,
                    source,
                    target,
                    INFERENCE_PRIORITY_NONE,
                    false,
                );
            });
        if inference_context.is_none() {
            self.c.apply_to_return_types(
                contextual_signature,
                signature,
                |checker, source, target| {
                    checker.infer_types(
                        &mut inferences,
                        source,
                        target,
                        INFERENCE_PRIORITY_RETURN_TYPE,
                        false,
                    );
                },
            );
        }
        self.c.inference_context_record_mut(context).inferences = inferences;
        let inferred_types =
            self.get_inferred_types_in_current_relation(context, intersection_state);
        self.c.get_signature_instantiation(
            signature,
            inferred_types,
            self.c
                .signature_record(contextual_signature)
                .declaration
                .is_some_and(|declaration| {
                    ast::is_in_js_file(self.c.store_for_node(declaration), declaration)
                }),
            Vec::new(), /*inferredTypeParameters*/
        )
    }

    fn get_inferred_types_in_current_relation(
        &mut self,
        context: InferenceContextRef,
        intersection_state: IntersectionState,
    ) -> Vec<TypeHandle> {
        let len = self.c.inference_context_record(context).inferences.len();
        let mut result = Vec::with_capacity(len);
        for i in 0..len {
            result.push(self.get_inferred_type_in_current_relation(context, i, intersection_state));
        }
        result
    }

    fn get_inferred_type_in_current_relation(
        &mut self,
        context_ref: InferenceContextRef,
        index: usize,
        intersection_state: IntersectionState,
    ) -> TypeHandle {
        let context = self.c.inference_context_record(context_ref).clone();
        let non_fixing_mapper = context.non_fixing_mapper.unwrap();
        let inference = {
            let inferences = &context.inferences;
            if let Some(inferred_type) = inferences[index].inferred_type {
                return inferred_type;
            }
            inferences[index].clone()
        };
        if inference.type_parameter == self.c.semantic_state.semantic_handles().error_type {
            return inference.type_parameter;
        }
        let mut inferred_type = None;
        let mut fallback_type = None;
        if let Some(signature) = context.signature {
            let mut inferred_covariant_type = None;
            if !inference.candidates.is_empty() {
                inferred_covariant_type =
                    Some(self.c.get_covariant_inference(&inference, signature));
            }
            let mut inferred_contravariant_type = None;
            if !inference.contra_candidates.is_empty() {
                inferred_contravariant_type = Some(self.c.get_contravariant_inference(&inference));
            }
            if inferred_covariant_type.is_some() || inferred_contravariant_type.is_some() {
                let mut contra_candidate_related = false;
                if let Some(inferred_covariant_type) = inferred_covariant_type {
                    for candidate in inference.contra_candidates.iter().copied() {
                        if self
                            .c
                            .is_type_assignable_to(inferred_covariant_type, candidate)
                        {
                            contra_candidate_related = true;
                            break;
                        }
                    }
                }
                let mut other_inferences_compatible = true;
                if let Some(inferred_covariant_type) = inferred_covariant_type {
                    for (other_index, other) in context.inferences.iter().enumerate() {
                        if other_index != index
                            && self
                                .c
                                .get_constraint_of_type_parameter(other.type_parameter)
                                != Some(inference.type_parameter)
                        {
                            continue;
                        }
                        for candidate in other.candidates.iter().copied() {
                            if !self
                                .c
                                .is_type_assignable_to(candidate, inferred_covariant_type)
                            {
                                other_inferences_compatible = false;
                                break;
                            }
                        }
                        if !other_inferences_compatible {
                            break;
                        }
                    }
                }
                let prefer_covariant_type = inferred_covariant_type.is_some()
                    && (inferred_contravariant_type.is_none()
                        || self.c.type_flags(inferred_covariant_type.unwrap())
                            & (TYPE_FLAGS_NEVER | TYPE_FLAGS_ANY)
                            == 0
                            && contra_candidate_related
                            && other_inferences_compatible);
                if prefer_covariant_type {
                    inferred_type = inferred_covariant_type;
                    fallback_type = inferred_contravariant_type;
                } else {
                    inferred_type = inferred_contravariant_type;
                    fallback_type = inferred_covariant_type;
                }
            } else if context.flags & INFERENCE_FLAGS_NO_DEFAULT != 0 {
                inferred_type = Some(self.c.semantic_state.semantic_handles().silent_never_type);
            } else if let Some(default_type) = self
                .c
                .get_default_from_type_parameter(inference.type_parameter)
            {
                let backreference_mapper = self.c.new_array_to_single_type_mapper_handle(
                    std::iter::once(inference.type_parameter)
                        .chain(
                            context.inferences[index + 1..]
                                .iter()
                                .map(|i| i.type_parameter),
                        )
                        .collect::<TypeMapperList>(),
                    self.c.semantic_state.semantic_handles().unknown_type,
                );
                let mapper = self
                    .c
                    .merge_type_mapper_handles(Some(backreference_mapper), non_fixing_mapper);
                inferred_type = self
                    .c
                    .instantiate_type_with_mapper_handle(Some(default_type), Some(mapper));
            }
        } else {
            inferred_type = self.c.get_type_from_inference(&inference);
        }
        let mut cached_inferred_type =
            inferred_type.unwrap_or(if context.flags & INFERENCE_FLAGS_ANY_DEFAULT != 0 {
                self.c.semantic_state.semantic_handles().any_type
            } else {
                self.c.semantic_state.semantic_handles().unknown_type
            });
        {
            let inferences = &mut self.c.inference_context_record_mut(context_ref).inferences;
            if inferences[index].inferred_type.is_none() {
                inferences[index].inferred_type = Some(cached_inferred_type);
            }
        }
        if let Some(constraint) = self
            .c
            .get_constraint_of_type_parameter(inference.type_parameter)
        {
            let non_fixing_mapper = self
                .c
                .inference_context_record(context_ref)
                .non_fixing_mapper;
            let instantiated_constraint = self
                .c
                .instantiate_type_with_mapper_handle(Some(constraint), non_fixing_mapper)
                .unwrap();
            if let Some(inferred) = inferred_type {
                let constraint_with_this = self.c.get_type_with_this_argument(
                    instantiated_constraint,
                    Some(inferred),
                    false,
                );
                if self.compare_types_for_inference(
                    inferred,
                    constraint_with_this,
                    false,
                    intersection_state,
                ) == TERNARY_FALSE
                {
                    let mut filtered_by_constraint = None;
                    if inference.priority == INFERENCE_PRIORITY_RETURN_TYPE {
                        filtered_by_constraint = Some(self.filter_type_by_current_relation(
                            inferred,
                            constraint_with_this,
                            intersection_state,
                        ));
                    }
                    inferred_type = if filtered_by_constraint.is_some()
                        && self.c.type_flags(filtered_by_constraint.unwrap()) & TYPE_FLAGS_NEVER
                            == 0
                    {
                        filtered_by_constraint
                    } else {
                        None
                    };
                }
            }
            if inferred_type.is_none() {
                let fallback_constraint_with_this = fallback_type.map(|fallback_type| {
                    self.c.get_type_with_this_argument(
                        instantiated_constraint,
                        Some(fallback_type),
                        false,
                    )
                });
                inferred_type = Some(
                    if fallback_type.is_some()
                        && self.compare_types_for_inference(
                            fallback_type.unwrap(),
                            fallback_constraint_with_this.unwrap(),
                            false,
                            intersection_state,
                        ) != TERNARY_FALSE
                    {
                        fallback_type.unwrap()
                    } else {
                        instantiated_constraint
                    },
                );
            }
            cached_inferred_type = inferred_type.unwrap();
        }
        self.c.inference_context_record_mut(context_ref).inferences[index].inferred_type =
            Some(cached_inferred_type);
        self.c.clear_active_mapper_caches();
        cached_inferred_type
    }

    fn compare_types_for_inference(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        report_errors: bool,
        intersection_state: IntersectionState,
    ) -> Ternary {
        self.is_related_to_ex(
            source,
            target,
            RECURSION_FLAGS_BOTH,
            report_errors,
            None,
            intersection_state,
        )
    }

    fn filter_type_by_current_relation(
        &mut self,
        t: TypeHandle,
        constraint: TypeHandle,
        intersection_state: IntersectionState,
    ) -> TypeHandle {
        if self.c.type_flags(t) & TYPE_FLAGS_NEVER != 0 {
            return t;
        }
        if self.c.type_flags(t) & TYPE_FLAGS_UNION == 0 {
            return if self.compare_types_for_inference(t, constraint, false, intersection_state)
                != TERNARY_FALSE
            {
                t
            } else {
                self.c.semantic_state.semantic_handles().never_type
            };
        }
        let origin = self.c.type_record(t).as_union_type().origin;
        let types_source =
            if origin.is_some() && self.c.type_flags(origin.unwrap()) & TYPE_FLAGS_UNION != 0 {
                origin.unwrap()
            } else {
                t
            };
        let types_len = self.c.type_types_len(types_source);
        let mut mapped_types = Vec::with_capacity(types_len);
        let mut changed = false;
        for index in 0..types_len {
            let ty = self.c.type_type_at(types_source, index);
            let mapped = self.filter_type_by_current_relation(ty, constraint, intersection_state);
            if mapped != ty {
                changed = true;
            }
            mapped_types.push(mapped);
        }
        if changed {
            if mapped_types.is_empty() {
                return self.c.semantic_state.semantic_handles().never_type;
            }
            return self
                .c
                .get_union_type_ex(mapped_types, UNION_REDUCTION_LITERAL, None, None);
        }
        t
    }

    fn compare_type_predicate_related_to_in_current_relation(
        &mut self,
        source: TypePredicateHandle,
        target: TypePredicateHandle,
        report_errors: bool,
        intersection_state: IntersectionState,
    ) -> Ternary {
        let source_record = self.c.type_predicate_record(source).clone();
        let target_record = self.c.type_predicate_record(target).clone();
        if source_record.kind != target_record.kind {
            if report_errors {
                self.report_error(
                    &*diagnostics::A_THIS_BASED_TYPE_GUARD_IS_NOT_COMPATIBLE_WITH_A_PARAMETER_BASED_TYPE_GUARD,
                    vec![],
                );
                let source_string = self.c.type_predicate_to_string(source);
                let target_string = self.c.type_predicate_to_string(target);
                self.report_error(
                    &*diagnostics::TYPE_PREDICATE_0_IS_NOT_ASSIGNABLE_TO_1,
                    vec![source_string.into(), target_string.into()],
                );
            }
            return TERNARY_FALSE;
        }
        if (source_record.kind == TYPE_PREDICATE_KIND_IDENTIFIER
            || source_record.kind == TYPE_PREDICATE_KIND_ASSERTS_IDENTIFIER)
            && source_record.parameter_index != target_record.parameter_index
        {
            if report_errors {
                self.report_error(
                    &*diagnostics::PARAMETER_0_IS_NOT_IN_THE_SAME_POSITION_AS_PARAMETER_1,
                    vec![
                        source_record.parameter_name.clone().into(),
                        target_record.parameter_name.clone().into(),
                    ],
                );
                let source_string = self.c.type_predicate_to_string(source);
                let target_string = self.c.type_predicate_to_string(target);
                self.report_error(
                    &*diagnostics::TYPE_PREDICATE_0_IS_NOT_ASSIGNABLE_TO_1,
                    vec![source_string.into(), target_string.into()],
                );
            }
            return TERNARY_FALSE;
        }
        let related = match (source_record.t, target_record.t) {
            (s, t) if s == t => TERNARY_TRUE,
            (Some(s), Some(t)) => self.compare_signature_types_in_current_relation(
                s,
                t,
                report_errors,
                intersection_state,
            ),
            _ => TERNARY_FALSE,
        };
        if related == TERNARY_FALSE && report_errors {
            let source_string = self.c.type_predicate_to_string(source);
            let target_string = self.c.type_predicate_to_string(target);
            self.report_error(
                &*diagnostics::TYPE_PREDICATE_0_IS_NOT_ASSIGNABLE_TO_1,
                vec![source_string.into(), target_string.into()],
            );
        }
        related
    }

    fn get_chain_message(&self, mut index: usize) -> Option<&'static diagnostics::Message> {
        let mut e = self.error_chain;
        loop {
            let Some(chain) = e else {
                return None;
            };
            let chain = &self.error_chains[chain.0];
            if index == 0 {
                return Some(chain.message);
            }
            e = chain.next;
            index -= 1;
        }
    }

    // Return true if the arguments of the first entry on the error chain match the
    // given arguments (where nil acts as a wildcard).
    fn chain_args_match(&self, args: Vec<Option<DiagnosticArg>>) -> bool {
        let Some(chain) = self.error_chain else {
            return false;
        };
        let chain = &self.error_chains[chain.0];
        for (i, a) in args.into_iter().enumerate() {
            if a.is_some() && Some(chain.args[i].clone()) != a {
                return false;
            }
        }
        true
    }

    fn chain_message(
        error_chains: &[ErrorChain],
        error_chain: Option<ErrorChainHandle>,
        mut index: usize,
    ) -> Option<&'static diagnostics::Message> {
        let mut e = error_chain;
        loop {
            let Some(chain) = e else {
                return None;
            };
            let chain = &error_chains[chain.0];
            if index == 0 {
                return Some(chain.message);
            }
            e = chain.next;
            index -= 1;
        }
    }

    fn count_message_chain_breadth(
        error_chains: &[ErrorChain],
        error_chain: Option<ErrorChainHandle>,
    ) -> usize {
        let mut count = 0;
        let mut e = error_chain;
        while let Some(chain) = e {
            let chain = &error_chains[chain.0];
            count += 1;
            e = chain.next;
        }
        count
    }

    fn report_error_to_chain(
        error_chains: &mut Vec<ErrorChain>,
        error_chain: &mut Option<ErrorChainHandle>,
        mut message: &'static diagnostics::Message,
        mut args: Vec<DiagnosticArg>,
    ) {
        if message_is(message, &diagnostics::TYPES_OF_PROPERTY_0_ARE_INCOMPATIBLE) {
            match Self::chain_message(error_chains, *error_chain, 0) {
                Some(m) if message_is(m, &diagnostics::OBJECT_LITERAL_MAY_ONLY_SPECIFY_KNOWN_PROPERTIES_AND_0_DOES_NOT_EXIST_IN_TYPE_1) || message_is(m, &diagnostics::OBJECT_LITERAL_MAY_ONLY_SPECIFY_KNOWN_PROPERTIES_BUT_0_DOES_NOT_EXIST_IN_TYPE_1_DID_YOU_MEAN_TO_WRITE_2) => return,
                _ => {}
            }
            let mut arg = String::new();
            match Self::chain_message(error_chains, *error_chain, 1) {
                Some(m) if message_is(m, &diagnostics::CALL_SIGNATURES_WITH_NO_ARGUMENTS_HAVE_INCOMPATIBLE_RETURN_TYPES_0_AND_1) => {
                    arg = format!("{}()", get_property_name_arg(args[0].clone()));
                }
                Some(m) if message_is(m, &diagnostics::CONSTRUCT_SIGNATURES_WITH_NO_ARGUMENTS_HAVE_INCOMPATIBLE_RETURN_TYPES_0_AND_1) => {
                    arg = format!("new {}()", get_property_name_arg(args[0].clone()));
                }
                Some(m) if message_is(m, &diagnostics::CALL_SIGNATURE_RETURN_TYPES_0_AND_1_ARE_INCOMPATIBLE) => {
                    arg = format!("{}(...)", get_property_name_arg(args[0].clone()));
                }
                Some(m) if message_is(m, &diagnostics::CONSTRUCT_SIGNATURE_RETURN_TYPES_0_AND_1_ARE_INCOMPATIBLE) => {
                    arg = format!("new {}(...)", get_property_name_arg(args[0].clone()));
                }
                _ => {}
            }
            if !arg.is_empty() {
                message =
                    &diagnostics::THE_TYPES_RETURNED_BY_0_ARE_INCOMPATIBLE_BETWEEN_THESE_TYPES;
                args[0] = arg.into();
                *error_chain = error_chain
                    .and_then(|chain| error_chains[chain.0].next)
                    .and_then(|next| error_chains[next.0].next);
            }
            match Self::chain_message(error_chains, *error_chain, 1) {
                Some(m)
                    if message_is(m, &diagnostics::TYPES_OF_PROPERTY_0_ARE_INCOMPATIBLE)
                        || message_is(m, &diagnostics::THE_TYPES_OF_0_ARE_INCOMPATIBLE_BETWEEN_THESE_TYPES)
                        || message_is(m, &diagnostics::THE_TYPES_RETURNED_BY_0_ARE_INCOMPATIBLE_BETWEEN_THESE_TYPES) =>
                {
                    let head = get_property_name_arg(args[0].clone());
                    let tail_handle = error_chains[error_chain.unwrap().0].next.unwrap();
                    let tail = get_property_name_arg(error_chains[tail_handle.0].args[0].clone());
                    let arg = add_to_dotted_name(&head, &tail);
                    *error_chain = error_chains[tail_handle.0].next;
                    let message = if message_is(message, &diagnostics::TYPES_OF_PROPERTY_0_ARE_INCOMPATIBLE) {
                        &diagnostics::THE_TYPES_OF_0_ARE_INCOMPATIBLE_BETWEEN_THESE_TYPES
                    } else {
                        message
                    };
                    Self::report_error_to_chain(error_chains, error_chain, message, vec![arg.into()]);
                    return;
                }
                _ => {}
            }
        }
        let handle = ErrorChainHandle(error_chains.len());
        error_chains.push(ErrorChain {
            next: *error_chain,
            message,
            args,
        });
        *error_chain = Some(handle);
    }

    fn report_error(&mut self, message: &'static diagnostics::Message, args: Vec<DiagnosticArg>) {
        if self.skip_parent_counter == 0 {
            Self::report_error_to_chain(
                &mut self.error_chains,
                &mut self.error_chain,
                message,
                args,
            );
        } else {
            self.skip_parent_counter -= 1;
        }
    }

    fn report_parent_skipped_error(
        &mut self,
        message: &'static diagnostics::Message,
        args: Vec<DiagnosticArg>,
    ) {
        self.report_error(message, args);
        self.skip_parent_counter += 1;
    }

    fn report_error_as_reporter<'reporter>(
        error_chains: &'reporter mut Vec<ErrorChain>,
        error_chain: &'reporter mut Option<ErrorChainHandle>,
    ) -> ErrorReporter<'reporter> {
        Box::new(move |message, args| {
            Self::report_error_to_chain(error_chains, error_chain, message, args)
        })
    }

    fn compare_properties_with_simple_related(
        &mut self,
        source_prop: SymbolIdentity,
        target_prop: SymbolIdentity,
    ) -> Ternary {
        if self.c.same_symbol_identity(source_prop, target_prop) {
            return TERNARY_TRUE;
        }
        let source_prop_accessibility = self
            .c
            .relater_declaration_modifier_flags_from_symbol_identity(source_prop)
            & ast::ModifierFlags::NON_PUBLIC_ACCESSIBILITY_MODIFIER;
        let target_prop_accessibility = self
            .c
            .relater_declaration_modifier_flags_from_symbol_identity(target_prop)
            & ast::ModifierFlags::NON_PUBLIC_ACCESSIBILITY_MODIFIER;
        if source_prop_accessibility != target_prop_accessibility {
            return TERNARY_FALSE;
        }
        if source_prop_accessibility != ast::ModifierFlags::None {
            let source_target_symbol = self.c.get_target_symbol_identity(source_prop);
            let target_target_symbol = self.c.get_target_symbol_identity(target_prop);
            if !self
                .c
                .same_symbol_identity(source_target_symbol, target_target_symbol)
            {
                return TERNARY_FALSE;
            }
        } else if (self.c.symbol_identity_flags(source_prop) & ast::SYMBOL_FLAGS_OPTIONAL)
            != (self.c.symbol_identity_flags(target_prop) & ast::SYMBOL_FLAGS_OPTIONAL)
        {
            return TERNARY_FALSE;
        }
        if self.c.is_readonly_symbol_identity(source_prop)
            != self.c.is_readonly_symbol_identity(target_prop)
        {
            return TERNARY_FALSE;
        }
        let source_type = self.c.get_non_missing_type_of_symbol_identity(source_prop);
        let target_type = self.c.get_non_missing_type_of_symbol_identity(target_prop);
        self.is_related_to_simple(source_type, target_type)
    }

    fn trace_unions_or_intersections_too_large(&mut self, source: TypeHandle, target: TypeHandle) {
        let tr = self.c.tracer;
        let Some(tr) = tr else {
            return;
        };
        if self.c.type_flags(source) & TYPE_FLAGS_UNION_OR_INTERSECTION != 0
            && self.c.type_flags(target) & TYPE_FLAGS_UNION_OR_INTERSECTION != 0
        {
            if self.c.object_flags(source)
                & self.c.object_flags(target)
                & OBJECT_FLAGS_PRIMITIVE_UNION
                != 0
            {
                // There's a fast path for comparing primitive unions
                return;
            }
            let source_size = self.c.type_types_len(source);
            let target_size = self.c.type_types_len(target);
            if source_size * target_size > 1_000_000 {
                tr.instant(
                    tracing::PHASE_CHECK_TYPES,
                    "traceUnionsOrIntersectionsTooLarge_DepthLimit",
                    tracing::args([
                        ("sourceId", serde_json::json!(self.c.type_id(source))),
                        ("sourceSize", serde_json::json!(source_size)),
                        ("targetId", serde_json::json!(self.c.type_id(target))),
                        ("targetSize", serde_json::json!(target_size)),
                    ]),
                );
            }
        }
    }
}

impl<'a, 'state> Checker<'a, 'state> {
    // An object type S is considered to be derived from an object type T if
    // S is a union type and every constituent of S is derived from T,
    // T is a union type and S is derived from at least one constituent of T, or
    // S is an intersection type and some constituent of S is derived from T, or
    // S is a type variable with a base constraint that is derived from T, or
    // T is {} and S is an object-like type (ensuring {} is less derived than Object), or
    // T is one of the global types Object and Function and S is a subtype of T, or
    // T occurs directly or indirectly in an 'extends' clause of S.
    // Note that this check ignores type parameters and only considers the
    // inheritance hierarchy.
    pub(crate) fn is_type_derived_from(&mut self, source: TypeHandle, target: TypeHandle) -> bool {
        if self.type_flags(source) & TYPE_FLAGS_UNION != 0 {
            let source_types_len = self.type_types_len(source);
            for index in 0..source_types_len {
                let ty = self.type_type_at(source, index);
                if !self.is_type_derived_from(ty, target) {
                    return false;
                }
            }
            return true;
        }
        if self.type_flags(target) & TYPE_FLAGS_UNION != 0 {
            let target_types_len = self.type_types_len(target);
            for index in 0..target_types_len {
                let ty = self.type_type_at(target, index);
                if self.is_type_derived_from(source, ty) {
                    return true;
                }
            }
            return false;
        }
        if self.type_flags(source) & TYPE_FLAGS_INTERSECTION != 0 {
            let source_types_len = self.type_types_len(source);
            for index in 0..source_types_len {
                let ty = self.type_type_at(source, index);
                if self.is_type_derived_from(ty, target) {
                    return true;
                }
            }
            return false;
        }
        if self.type_flags(source) & TYPE_FLAGS_INSTANTIABLE_NON_PRIMITIVE != 0 {
            let constraint = self
                .get_base_constraint_of_type(source)
                .unwrap_or(self.semantic_state.semantic_handles().unknown_type);
            return self.is_type_derived_from(constraint, target);
        }
        if self.is_empty_anonymous_object_type(target) {
            return self.type_flags(source) & (TYPE_FLAGS_OBJECT | TYPE_FLAGS_NON_PRIMITIVE) != 0;
        }
        if target == self.semantic_state.semantic_handles().global_object_type {
            return self.type_flags(source) & (TYPE_FLAGS_OBJECT | TYPE_FLAGS_NON_PRIMITIVE) != 0
                && !self.is_empty_anonymous_object_type(source);
        }
        if target == self.semantic_state.semantic_handles().global_function_type {
            return self.type_flags(source) & TYPE_FLAGS_OBJECT != 0
                && self.is_function_object_type(source);
        }
        let target_type = self.get_target_type(target);
        self.has_base_type(source, target_type)
            || (self.is_array_type(target)
                && !self.is_readonly_array_type(target)
                && self.is_type_derived_from(
                    source,
                    self.semantic_state
                        .semantic_handles()
                        .global_readonly_array_type,
                ))
    }

    fn is_distribution_dependent(&mut self, root: &crate::semantic::ConditionalRootRecord) -> bool {
        let root_node = root.node.unwrap();
        let root_store = self.store_for_node(root_node);
        root.is_distributive
            && (self.is_type_parameter_possibly_referenced(
                root.check_type.unwrap(),
                root_store.true_type(root_node).unwrap(),
            ) || self.is_type_parameter_possibly_referenced(
                root.check_type.unwrap(),
                root_store.false_type(root_node).unwrap(),
            ))
    }

    pub(crate) fn is_object_type_with_inferable_index(&mut self, t: TypeHandle) -> bool {
        if self.type_flags(t) & TYPE_FLAGS_INTERSECTION != 0 {
            let types_len = self.type_types_len(t);
            for index in 0..types_len {
                let ty = self.type_type_at(t, index);
                if !self.is_object_type_with_inferable_index(ty) {
                    return false;
                }
            }
            return true;
        }
        self.type_symbol_identity(t)
            .map(|symbol| self.symbol_identity_flags(symbol))
            .is_some_and(|flags| {
                flags
                    & (ast::SYMBOL_FLAGS_OBJECT_LITERAL
                        | ast::SYMBOL_FLAGS_TYPE_LITERAL
                        | ast::SYMBOL_FLAGS_ENUM
                        | ast::SYMBOL_FLAGS_VALUE_MODULE)
                    != 0
                    && flags & ast::SYMBOL_FLAGS_CLASS == 0
            })
            && !self.type_has_call_or_construct_signatures(t)
            || self.object_flags(t) & (OBJECT_FLAGS_JS_LITERAL | OBJECT_FLAGS_OBJECT_REST_TYPE) != 0
            || self.object_flags(t) & OBJECT_FLAGS_REVERSE_MAPPED != 0
                && self.is_object_type_with_inferable_index(
                    self.type_record(t).as_reverse_mapped_type().source.unwrap(),
                )
    }
}

pub(crate) fn add_to_dotted_name(head: &str, tail: &str) -> String {
    let mut head = head.to_string();
    if head.starts_with("new ") {
        head = format!("({})", head);
    }
    let mut pos = 0;
    loop {
        if tail[pos..].starts_with('(') {
            pos += 1;
        } else if tail[pos..].starts_with("new ") {
            pos += 4;
        } else {
            break;
        }
    }
    let prefix = &tail[..pos];
    let suffix = &tail[pos..];
    if suffix.starts_with('[') {
        return format!("{}{}{}", prefix, head, suffix);
    }
    format!("{}{}.{}", prefix, head, suffix)
}

pub(crate) fn get_property_name_arg(arg: DiagnosticArg) -> String {
    let s = arg.to_string();
    if !s.is_empty()
        && (s.as_bytes()[0] == b'"' || s.as_bytes()[0] == b'\'' || s.as_bytes()[0] == b'`')
    {
        return format!("[{}]", s);
    }
    s
}

pub(crate) fn is_conversion_or_interface_implementation_message(
    message: &diagnostics::Message,
) -> bool {
    message_is(message, &diagnostics::CLASS_0_INCORRECTLY_IMPLEMENTS_INTERFACE_1)
        || message_is(message, &diagnostics::CLASS_0_INCORRECTLY_IMPLEMENTS_CLASS_1_DID_YOU_MEAN_TO_EXTEND_1_AND_INHERIT_ITS_MEMBERS_AS_A_SUBCLASS)
        || message_is(message, &diagnostics::CONVERSION_OF_TYPE_0_TO_TYPE_1_MAY_BE_A_MISTAKE_BECAUSE_NEITHER_TYPE_SUFFICIENTLY_OVERLAPS_WITH_THE_OTHER_IF_THIS_WAS_INTENTIONAL_CONVERT_THE_EXPRESSION_TO_UNKNOWN_FIRST)
        || message_is(message, &diagnostics::ITS_INSTANCE_TYPE_0_IS_NOT_A_VALID_JSX_ELEMENT)
        || message_is(message, &diagnostics::ITS_RETURN_TYPE_0_IS_NOT_A_VALID_JSX_ELEMENT)
        || message_is(message, &diagnostics::ITS_ELEMENT_TYPE_0_IS_NOT_A_VALID_JSX_ELEMENT)
}

pub(crate) fn chain_depth(chains: &[ErrorChain], mut chain: Option<ErrorChainHandle>) -> usize {
    let mut depth = 0;
    while let Some(c) = chain {
        let c = &chains[c.0];
        depth += 1;
        chain = c.next;
    }
    depth
}

impl<'a, 'state, 'c> Relater<'a, 'state, 'c> {
    // Determine if possibly recursive types are related. First, check if the result is already available in the global cache.
    // Second, check if we have already started a comparison of the given two types in which case we assume the result to be true.
    // Third, check if both types are part of deeply nested chains of generic type instantiations and if so assume the types are
    // equal and infinitely expanding. Fourth, if we have reached a depth of 100 nested comparisons, assume we have runaway recursion
    // and issue an error. Otherwise, actually compare the structure of the two types.
    fn recursive_type_related_to(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        report_errors: bool,
        intersection_state: IntersectionState,
        recursion_flags: RecursionFlags,
    ) -> Ternary {
        if self.overflow {
            // Note that stack depth overflows can cause _any_ relation involving structured types to become false, so it is
            // important to have well-defined behavior even in cases that shouldn't normally occur.
            return TERNARY_FALSE;
        }
        if self.source_stack.len() == 100 || self.target_stack.len() == 100 {
            self.overflow = true;
            return TERNARY_FALSE;
        }
        let is_identity = self.relation_is(self.c.semantic_state.identity_relation);
        let (id, constrained) = get_relation_key(
            &mut *self.c,
            source,
            target,
            intersection_state,
            is_identity,
            false, /*ignoreConstraints*/
        );
        let entry = self.relation_result(id);
        if entry != RELATION_COMPARISON_RESULT_NONE {
            if report_errors
                && entry & RELATION_COMPARISON_RESULT_FAILED != 0
                && entry & RELATION_COMPARISON_RESULT_OVERFLOW == 0
            {
                // We are elaborating errors and the cached result is a failure not due to a comparison overflow,
                // so we will do the comparison again to generate an error message.
            } else {
                self.c.add_reliability_flags(
                    entry
                        & (RELATION_COMPARISON_RESULT_REPORTS_UNMEASURABLE
                            | RELATION_COMPARISON_RESULT_REPORTS_UNRELIABLE),
                );
                if report_errors && entry & RELATION_COMPARISON_RESULT_OVERFLOW != 0 {
                    let message = if entry & RELATION_COMPARISON_RESULT_COMPLEXITY_OVERFLOW != 0 {
                        &diagnostics::EXCESSIVE_COMPLEXITY_COMPARING_TYPES_0_AND_1
                    } else {
                        &diagnostics::EXCESSIVE_STACK_DEPTH_COMPARING_TYPES_0_AND_1
                    };
                    let source_string = self.c.type_to_string_public(source);
                    let target_string = self.c.type_to_string_public(target);
                    self.report_error(message, vec![source_string.into(), target_string.into()]);
                }
                return if entry & RELATION_COMPARISON_RESULT_SUCCEEDED != 0 {
                    TERNARY_TRUE
                } else {
                    TERNARY_FALSE
                };
            }
        }
        if self.relation_count <= 0 {
            self.overflow = true;
            return TERNARY_FALSE;
        }
        // If source and target are already being compared, consider them related with assumptions
        if self.maybe_keys_set.has(&id) {
            return TERNARY_MAYBE;
        }
        // A constrained key indicates that we have type references that reference constrained
        // type parameters. For such keys we also check against the key we would have gotten if all type parameters
        // were unconstrained.
        if constrained {
            let (broadest_equivalent_id, _) = get_relation_key(
                &mut *self.c,
                source,
                target,
                intersection_state,
                is_identity,
                true, /*ignoreConstraints*/
            );
            if self.maybe_keys_set.has(&broadest_equivalent_id) {
                return TERNARY_MAYBE;
            }
        }
        let maybe_start = self.maybe_keys.len();
        self.maybe_keys.push(id);
        self.maybe_keys_set.add(id);
        let save_expanding_flags = self.expanding_flags;
        if recursion_flags & RECURSION_FLAGS_SOURCE != 0 {
            self.source_stack.push(source);
            if self.expanding_flags & EXPANDING_FLAGS_SOURCE == 0
                && self.c.is_deeply_nested_type(source, &self.source_stack, 3)
            {
                self.expanding_flags |= EXPANDING_FLAGS_SOURCE;
            }
        }
        if recursion_flags & RECURSION_FLAGS_TARGET != 0 {
            self.target_stack.push(target);
            if self.expanding_flags & EXPANDING_FLAGS_TARGET == 0
                && self.c.is_deeply_nested_type(target, &self.target_stack, 3)
            {
                self.expanding_flags |= EXPANDING_FLAGS_TARGET;
            }
        }
        let save_reliability_flags = self.c.reliability_flags();
        self.c.set_reliability_flags(0);
        let result = if self.expanding_flags == EXPANDING_FLAGS_BOTH {
            if let Some(tr) = self.c.tracer {
                tr.instant(
                    tracing::PHASE_CHECK_TYPES,
                    "recursiveTypeRelatedTo_DepthLimit",
                    tracing::args([
                        ("sourceId", serde_json::json!(self.c.type_id(source))),
                        ("targetId", serde_json::json!(self.c.type_id(target))),
                        ("depth", serde_json::json!(self.source_stack.len())),
                        ("targetDepth", serde_json::json!(self.target_stack.len())),
                    ]),
                );
            }
            TERNARY_MAYBE
        } else {
            if let Some(tr) = self.c.tracer {
                let _pop = tr.push(
                    tracing::PHASE_CHECK_TYPES,
                    "structuredTypeRelatedTo",
                    tracing::args([
                        ("sourceId", serde_json::json!(self.c.type_id(source))),
                        ("targetId", serde_json::json!(self.c.type_id(target))),
                    ]),
                    false,
                );
            }
            self.structured_type_related_to(source, target, report_errors, intersection_state)
        };
        let propagating_variance_flags = self.c.reliability_flags();
        self.c
            .set_reliability_flags(propagating_variance_flags | save_reliability_flags);
        if recursion_flags & RECURSION_FLAGS_SOURCE != 0 {
            self.source_stack.pop();
        }
        if recursion_flags & RECURSION_FLAGS_TARGET != 0 {
            self.target_stack.pop();
        }
        self.expanding_flags = save_expanding_flags;
        if result != TERNARY_FALSE {
            if result == TERNARY_TRUE
                || (self.source_stack.is_empty() && self.target_stack.is_empty())
            {
                self.reset_maybe_stack(
                    maybe_start,
                    propagating_variance_flags,
                    result == TERNARY_TRUE || result == TERNARY_MAYBE,
                );
            }
        } else {
            self.set_relation_result(
                id,
                RELATION_COMPARISON_RESULT_FAILED | propagating_variance_flags,
            );
            self.relation_count -= 1;
            self.reset_maybe_stack(maybe_start, propagating_variance_flags, false);
        }
        result
    }

    fn structured_type_related_to(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        report_errors: bool,
        intersection_state: IntersectionState,
    ) -> Ternary {
        let save_error_state = self.get_error_state();
        let mut result = self.structured_type_related_to_worker(
            source,
            target,
            report_errors,
            intersection_state,
        );
        if !self.relation_is(self.c.semantic_state.identity_relation) {
            if result == TERNARY_FALSE
                && (self.c.type_flags(source) & TYPE_FLAGS_INTERSECTION != 0
                    || self.c.type_flags(source) & TYPE_FLAGS_TYPE_PARAMETER != 0
                        && self.c.type_flags(target) & TYPE_FLAGS_UNION != 0)
            {
                let constraint = self.c.get_effective_constraint_of_intersection(
                    source,
                    self.c.type_flags(target) & TYPE_FLAGS_UNION != 0,
                );
                if let Some(constraint) = constraint
                    && every_type(self.c, constraint, |_, c| c != source)
                {
                    // TODO: Stack errors so we get a pyramid for the "normal" comparison above, _and_ a second for this
                    result = self.is_related_to_ex(
                        constraint,
                        target,
                        RECURSION_FLAGS_SOURCE,
                        false, /*reportErrors*/
                        None,  /*headMessage*/
                        intersection_state,
                    );
                }
            }
            if result != TERNARY_FALSE
                && intersection_state & INTERSECTION_STATE_TARGET == 0
                && self.c.type_flags(target) & TYPE_FLAGS_INTERSECTION != 0
                && !self.c.is_generic_object_type(target)
                && self.c.type_flags(source) & (TYPE_FLAGS_OBJECT | TYPE_FLAGS_INTERSECTION) != 0
            {
                result &= self.properties_related_to(
                    source,
                    target,
                    report_errors,
                    collections::Set::new(), /*excludedProperties*/
                    false,                   /*optionalsOnly*/
                    INTERSECTION_STATE_NONE,
                );
                if result != 0
                    && self.c.object_flags(source) & OBJECT_FLAGS_OBJECT_LITERAL != 0
                    && self.c.object_flags(source) & OBJECT_FLAGS_FRESH_LITERAL != 0
                {
                    result &= self.index_signatures_related_to(
                        source,
                        target,
                        false, /*sourceIsPrimitive*/
                        report_errors,
                        INTERSECTION_STATE_NONE,
                    );
                }
            } else if result != 0
                && self.c.is_non_generic_object_type(target)
                && !self.c.is_array_or_tuple_type(target)
                && self.is_source_intersection_needing_extra_check(source, target)
            {
                result &= self.properties_related_to(
                    source,
                    target,
                    report_errors,
                    collections::Set::new(), /*excludedProperties*/
                    true,                    /*optionalsOnly*/
                    intersection_state,
                );
            }
        }
        if result != TERNARY_FALSE {
            self.restore_error_state(&save_error_state);
        }
        result
    }

    fn structured_type_related_to_worker(
        &mut self,
        mut source: TypeHandle,
        target: TypeHandle,
        report_errors: bool,
        intersection_state: IntersectionState,
    ) -> Ternary {
        let save_error_state = self.get_error_state();
        let mut result = TERNARY_FALSE;
        let mut variance_check_failed = false;
        let mut original_error_chain = None;
        if self.relation_is(self.c.semantic_state.identity_relation) {
            if self.c.type_flags(source) & TYPE_FLAGS_UNION_OR_INTERSECTION != 0 {
                result = self.each_type_related_to_some_type(source, target);
                if result != TERNARY_FALSE {
                    result &= self.each_type_related_to_some_type(target, source);
                }
                return result;
            }
            if self.c.type_flags(source) & TYPE_FLAGS_INDEX != 0 {
                let source_index = self.c.type_record(source).as_index_type().clone();
                let target_index = self.c.type_record(target).as_index_type().clone();
                return self.is_related_to(
                    source_index.target.unwrap(),
                    target_index.target.unwrap(),
                    RECURSION_FLAGS_BOTH,
                    false, /*reportErrors*/
                );
            }
            if self.c.type_flags(source) & TYPE_FLAGS_INDEXED_ACCESS != 0 {
                let source_indexed_access =
                    self.c.type_record(source).as_indexed_access_type().clone();
                let target_indexed_access =
                    self.c.type_record(target).as_indexed_access_type().clone();
                result = self.is_related_to(
                    source_indexed_access.object_type.unwrap(),
                    target_indexed_access.object_type.unwrap(),
                    RECURSION_FLAGS_BOTH,
                    false, /*reportErrors*/
                );
                if result != TERNARY_FALSE {
                    result &= self.is_related_to(
                        source_indexed_access.index_type.unwrap(),
                        target_indexed_access.index_type.unwrap(),
                        RECURSION_FLAGS_BOTH,
                        false, /*reportErrors*/
                    );
                    if result != TERNARY_FALSE {
                        return result;
                    }
                }
            }
            if self.c.type_flags(source) & TYPE_FLAGS_CONDITIONAL != 0 {
                let source_conditional = self.c.type_record(source).as_conditional_type().clone();
                let target_conditional = self.c.type_record(target).as_conditional_type().clone();
                let source_root = source_conditional.root.unwrap();
                let target_root = target_conditional.root.unwrap();
                if self
                    .c
                    .semantic_state
                    .conditional_root_record(source_root)
                    .is_distributive
                    == self
                        .c
                        .semantic_state
                        .conditional_root_record(target_root)
                        .is_distributive
                {
                    result = self.is_related_to_ex(
                        source_conditional.check_type.unwrap(),
                        target_conditional.check_type.unwrap(),
                        RECURSION_FLAGS_BOTH,
                        false, /*reportErrors*/
                        None,  /*headMessage*/
                        intersection_state,
                    );
                    if result != TERNARY_FALSE {
                        result &= self.is_related_to_ex(
                            source_conditional.extends_type.unwrap(),
                            target_conditional.extends_type.unwrap(),
                            RECURSION_FLAGS_BOTH,
                            false, /*reportErrors*/
                            None,  /*headMessage*/
                            intersection_state,
                        );
                        if result != TERNARY_FALSE {
                            let source_true = self.c.get_true_type_from_conditional_type(source);
                            let target_true = self.c.get_true_type_from_conditional_type(target);
                            result &= self.is_related_to_ex(
                                source_true,
                                target_true,
                                RECURSION_FLAGS_BOTH,
                                false, /*reportErrors*/
                                None,  /*headMessage*/
                                intersection_state,
                            );
                            if result != TERNARY_FALSE {
                                let source_false =
                                    self.c.get_false_type_from_conditional_type(source);
                                let target_false =
                                    self.c.get_false_type_from_conditional_type(target);
                                result &= self.is_related_to_ex(
                                    source_false,
                                    target_false,
                                    RECURSION_FLAGS_BOTH,
                                    false, /*reportErrors*/
                                    None,  /*headMessage*/
                                    intersection_state,
                                );
                                if result != TERNARY_FALSE {
                                    return result;
                                }
                            }
                        }
                    }
                }
            }
            if self.c.type_flags(source) & TYPE_FLAGS_TEMPLATE_LITERAL != 0 {
                let source_template = self
                    .c
                    .type_record(source)
                    .as_template_literal_type()
                    .clone();
                let target_template = self
                    .c
                    .type_record(target)
                    .as_template_literal_type()
                    .clone();
                if source_template.texts_equal(&target_template) {
                    result = TERNARY_TRUE;
                    for (source_type, target_type) in source_template
                        .types
                        .iter()
                        .copied()
                        .zip(target_template.types.iter())
                        .map(|(source_type, target_type)| (source_type, *target_type))
                    {
                        result &= self.is_related_to_ex(
                            source_type,
                            target_type,
                            RECURSION_FLAGS_BOTH,
                            false, /*reportErrors*/
                            None,  /*headMessage*/
                            intersection_state,
                        );
                        if result == TERNARY_FALSE {
                            break;
                        }
                    }
                    return result;
                }
            }
            if self.c.type_flags(source) & TYPE_FLAGS_STRING_MAPPING != 0
                && self.c.same_optional_symbol_identity(
                    self.c.type_symbol_identity(source),
                    self.c.type_symbol_identity(target),
                )
            {
                let source_mapping = self.c.type_record(source).as_string_mapping_type().clone();
                let target_mapping = self.c.type_record(target).as_string_mapping_type().clone();
                return self.is_related_to_ex(
                    source_mapping.target.unwrap(),
                    target_mapping.target.unwrap(),
                    RECURSION_FLAGS_BOTH,
                    false, /*reportErrors*/
                    None,  /*headMessage*/
                    intersection_state,
                );
            }
            if self.c.type_flags(source) & TYPE_FLAGS_OBJECT == 0 {
                return TERNARY_FALSE;
            }
        } else if self.c.type_flags(source) & TYPE_FLAGS_UNION_OR_INTERSECTION != 0
            || self.c.type_flags(target) & TYPE_FLAGS_UNION_OR_INTERSECTION != 0
        {
            result = self.union_or_intersection_related_to(
                source,
                target,
                report_errors,
                intersection_state,
            );
            if result != TERNARY_FALSE {
                return result;
            }
            if !(self.c.type_flags(source) & TYPE_FLAGS_INSTANTIABLE != 0
                || self.c.type_flags(source) & TYPE_FLAGS_OBJECT != 0
                    && self.c.type_flags(target) & TYPE_FLAGS_UNION != 0
                || self.c.type_flags(source) & TYPE_FLAGS_INTERSECTION != 0
                    && self.c.type_flags(target)
                        & (TYPE_FLAGS_OBJECT | TYPE_FLAGS_UNION | TYPE_FLAGS_INSTANTIABLE)
                        != 0)
            {
                return TERNARY_FALSE;
            }
        }
        let source_alias = self.c.type_alias_record(source).cloned();
        let target_alias = self.c.type_alias_record(target).cloned();
        if self.c.type_flags(source) & (TYPE_FLAGS_OBJECT | TYPE_FLAGS_CONDITIONAL) != 0
            && source_alias
                .as_ref()
                .is_some_and(|alias| !alias.type_arguments.is_empty() && alias.symbol.is_some())
            && target_alias.is_some()
            && source_alias.as_ref().unwrap().symbol == target_alias.as_ref().unwrap().symbol
            && !(self.c.is_marker_type(source) || self.c.is_marker_type(target))
        {
            let source_alias = source_alias.unwrap();
            let target_alias = target_alias.unwrap();
            let alias_symbol = source_alias
                .symbol
                .expect("matching type alias must keep alias symbol");
            let variance_state = self.c.get_alias_variances_identity_state(alias_symbol);
            if variance_state.is_computing() {
                return TERNARY_UNKNOWN;
            }
            let variances = variance_state.into_variances_or_empty();
            let params = self
                .c
                .semantic_state
                .type_alias_type_parameters(alias_symbol);
            let min_params = self.c.get_min_type_argument_count(&params);
            let node_is_in_js_file = {
                let declaration = self
                    .c
                    .missing_name_symbol_identity_value_declaration(alias_symbol)
                    .or_else(|| self.c.first_symbol_identity_declaration(alias_symbol));
                declaration.is_some_and(|declaration| {
                    ast::is_in_js_file(self.c.store_for_node(declaration), declaration)
                })
            };
            let source_types = self.c.fill_missing_type_arguments(
                source_alias.type_arguments.clone(),
                &params,
                min_params,
                node_is_in_js_file,
            );
            let target_types = self.c.fill_missing_type_arguments(
                target_alias.type_arguments.clone(),
                &params,
                min_params,
                node_is_in_js_file,
            );
            let variance_result = self.type_arguments_related_to(
                &source_types,
                &target_types,
                &variances,
                report_errors,
                intersection_state,
            );
            if variance_result != TERNARY_FALSE {
                return variance_result;
            }
            if variances
                .iter()
                .any(|v| v & VARIANCE_FLAGS_ALLOWS_STRUCTURAL_FALLBACK != 0)
            {
                original_error_chain = None;
                self.restore_error_state(&save_error_state);
            } else {
                let allow_structural_fallback = self
                    .c
                    .has_covariant_void_argument(&target_types, &variances);
                variance_check_failed = !allow_structural_fallback;
                if !variances.is_empty() && !allow_structural_fallback {
                    let has_invariant = variances
                        .iter()
                        .any(|v| v & VARIANCE_FLAGS_VARIANCE_MASK == VARIANCE_FLAGS_INVARIANT);
                    if variance_check_failed && !(report_errors && has_invariant) {
                        return TERNARY_FALSE;
                    }
                    original_error_chain = self.error_chain;
                    self.restore_error_state(&save_error_state);
                }
            }
        }
        if self.c.is_single_element_generic_tuple_type(source)
            && !self.c.target_tuple_type_record(source).readonly
        {
            let source_type_argument = self.c.type_argument_at(source, 0);
            result = self.is_related_to(
                source_type_argument,
                target,
                RECURSION_FLAGS_SOURCE,
                false, /*reportErrors*/
            );
            if result != TERNARY_FALSE {
                return result;
            }
        }
        if self.c.is_single_element_generic_tuple_type(target)
            && (self.c.target_tuple_type_record(target).readonly || {
                let source_constraint = self.c.get_base_constraint_or_type(source);
                self.c.is_mutable_array_or_tuple(source_constraint)
            })
        {
            let target_type_argument = self.c.type_argument_at(target, 0);
            result = self.is_related_to(
                source,
                target_type_argument,
                RECURSION_FLAGS_TARGET,
                false, /*reportErrors*/
            );
            if result != TERNARY_FALSE {
                return result;
            }
        }
        if self.c.type_flags(target) & TYPE_FLAGS_TYPE_PARAMETER != 0 {
            // A source type { [P in Q]: X } is related to a target type T if keyof T is related to Q and X is related to T[Q].
            if self.c.object_flags(source) & OBJECT_FLAGS_MAPPED != 0
                && {
                    let declaration = self
                        .c
                        .type_record(source)
                        .as_mapped_type()
                        .declaration
                        .unwrap();
                    self.c
                        .store_for_node(declaration)
                        .name_type(declaration)
                        .is_none()
                }
                && {
                    let target_index_type = self.c.get_index_type(target);
                    let source_constraint_type =
                        self.c.get_constraint_type_from_mapped_type(source);
                    self.is_related_to(
                        target_index_type,
                        source_constraint_type,
                        RECURSION_FLAGS_BOTH,
                        false,
                    ) != TERNARY_FALSE
                }
            {
                if self.c.get_mapped_type_modifiers(source) & MAPPED_TYPE_MODIFIERS_INCLUDE_OPTIONAL
                    == 0
                {
                    let template_type = self.c.get_template_type_from_mapped_type(source);
                    let type_parameter = self.c.get_type_parameter_from_mapped_type(source);
                    let indexed_access_type =
                        self.c.get_indexed_access_type(target, type_parameter);
                    result = self.is_related_to(
                        template_type,
                        indexed_access_type,
                        RECURSION_FLAGS_BOTH,
                        report_errors,
                    );
                    if result != TERNARY_FALSE {
                        return result;
                    }
                }
            }
            if self.relation_is(self.c.semantic_state.comparable_relation)
                && self.c.type_flags(source) & TYPE_FLAGS_TYPE_PARAMETER != 0
            {
                // This is a carve-out in comparability to essentially forbid comparing a type parameter with another type parameter
                // unless one extends the other. (Remember: comparability is mostly bidirectional!)
                if let Some(constraint) = self.c.get_constraint_of_type_parameter(source) {
                    if some_type(self.c, constraint, |checker, c| {
                        checker.type_flags(c) & TYPE_FLAGS_TYPE_PARAMETER != 0
                    }) {
                        return self.is_related_to(
                            constraint,
                            target,
                            RECURSION_FLAGS_SOURCE,
                            false, /*reportErrors*/
                        );
                    }
                }
                return TERNARY_FALSE;
            }
        }
        if self.c.type_flags(target) & TYPE_FLAGS_INDEX != 0 {
            let target_index = self.c.type_record(target).as_index_type().clone();
            let target_type = target_index.target.unwrap();
            // A keyof S is related to a keyof T if T is related to S.
            if self.c.type_flags(source) & TYPE_FLAGS_INDEX != 0 {
                let source_index = self.c.type_record(source).as_index_type().clone();
                result = self.is_related_to(
                    target_type,
                    source_index.target.unwrap(),
                    RECURSION_FLAGS_BOTH,
                    false, /*reportErrors*/
                );
                if result != TERNARY_FALSE {
                    return result;
                }
            }
            if self.c.is_tuple_type(target_type) {
                // An index type can have a tuple type target when the tuple type contains variadic elements.
                // Check if the source is related to the known keys of the tuple type.
                let known_keys = self.c.get_known_keys_of_tuple_type(target_type);
                result =
                    self.is_related_to(source, known_keys, RECURSION_FLAGS_TARGET, report_errors);
                if result != TERNARY_FALSE {
                    return result;
                }
            } else if let Some(constraint) = self.c.get_simplified_type_or_constraint(target_type) {
                // A type S is assignable to keyof T if S is assignable to keyof C, where C is the
                // simplified form of T or, if T doesn't simplify, the constraint of T.
                let constraint_index_type = self.c.get_index_type_ex(
                    constraint,
                    target_index.index_flags | INDEX_FLAGS_NO_REDUCIBLE_CHECK,
                );
                result = self.is_related_to(
                    source,
                    constraint_index_type,
                    RECURSION_FLAGS_TARGET,
                    report_errors,
                );
                if result == TERNARY_TRUE {
                    return TERNARY_TRUE;
                }
            } else if self.c.is_generic_mapped_type(target_type) {
                // Generic mapped types that don't simplify or have a constraint still have a deferred key set:
                // either their remapped names or their constraint type.
                let name_type = self.c.get_name_type_from_mapped_type(target_type);
                let constraint_type = self.c.get_constraint_type_from_mapped_type(target_type);
                let target_keys = if name_type.is_some()
                    && self
                        .c
                        .is_mapped_type_with_keyof_constraint_declaration(target_type)
                {
                    let mapped_keys = self
                        .c
                        .get_apparent_mapped_type_keys(name_type.unwrap(), target_type);
                    self.c.get_union_type(vec![mapped_keys, name_type.unwrap()])
                } else {
                    name_type.unwrap_or(constraint_type)
                };
                result =
                    self.is_related_to(source, target_keys, RECURSION_FLAGS_TARGET, report_errors);
                if result == TERNARY_TRUE {
                    return TERNARY_TRUE;
                }
            }
        } else if self.c.type_flags(target) & TYPE_FLAGS_INDEXED_ACCESS != 0 {
            let target_indexed_access = self.c.type_record(target).as_indexed_access_type().clone();
            if self.c.type_flags(source) & TYPE_FLAGS_INDEXED_ACCESS != 0 {
                let source_indexed_access =
                    self.c.type_record(source).as_indexed_access_type().clone();
                // Relate components directly before falling back to constraint relationships.
                // A type S[K] is related to a type T[J] if S is related to T and K is related to J.
                result = self.is_related_to(
                    source_indexed_access.object_type.unwrap(),
                    target_indexed_access.object_type.unwrap(),
                    RECURSION_FLAGS_BOTH,
                    report_errors,
                );
                if result != TERNARY_FALSE {
                    result &= self.is_related_to(
                        source_indexed_access.index_type.unwrap(),
                        target_indexed_access.index_type.unwrap(),
                        RECURSION_FLAGS_BOTH,
                        report_errors,
                    );
                }
                if result != TERNARY_FALSE {
                    return result;
                }
                if report_errors {
                    original_error_chain = self.error_chain;
                }
            }
            // A type S is related to a type T[K] if S is related to C, where C is the base
            // constraint of T[K] for writing.
            if self.relation_is(self.c.semantic_state.assignable_relation)
                || self.relation_is(self.c.semantic_state.comparable_relation)
            {
                let object_type = target_indexed_access.object_type.unwrap();
                let index_type = target_indexed_access.index_type.unwrap();
                let base_object_type = self
                    .c
                    .get_base_constraint_of_type(object_type)
                    .unwrap_or(object_type);
                let base_index_type = self
                    .c
                    .get_base_constraint_of_type(index_type)
                    .unwrap_or(index_type);
                if !self.c.is_generic_object_type(base_object_type)
                    && !self.c.is_generic_index_type(base_index_type)
                {
                    let access_flags = ACCESS_FLAGS_WRITING
                        | if base_object_type != object_type {
                            ACCESS_FLAGS_NO_INDEX_SIGNATURES
                        } else {
                            ACCESS_FLAGS_NONE
                        };
                    if let Some(constraint) = self.c.get_indexed_access_type_or_undefined(
                        base_object_type,
                        base_index_type,
                        access_flags,
                        None,
                        None,
                    ) {
                        if report_errors && original_error_chain.is_some() {
                            self.restore_error_state(&save_error_state);
                        }
                        result = self.is_related_to_ex(
                            source,
                            constraint,
                            RECURSION_FLAGS_TARGET,
                            report_errors,
                            None, /*headMessage*/
                            intersection_state,
                        );
                        if result != TERNARY_FALSE {
                            return result;
                        }
                        if report_errors
                            && original_error_chain.is_some()
                            && self.error_chain.is_some()
                            && Self::count_message_chain_breadth(
                                &self.error_chains,
                                original_error_chain,
                            ) <= Self::count_message_chain_breadth(
                                &self.error_chains,
                                self.error_chain,
                            )
                        {
                            self.error_chain = original_error_chain;
                        }
                    }
                }
            }
            if report_errors {
                original_error_chain = None;
            }
        } else if self.c.type_flags(target) & TYPE_FLAGS_CONDITIONAL != 0 {
            if self.c.is_deeply_nested_type(target, &self.target_stack, 10) {
                return TERNARY_MAYBE;
            }
            let conditional = self.c.type_record(target).as_conditional_type().clone();
            let root = conditional.root.unwrap();
            let same_conditional_root = self.c.type_flags(source) & TYPE_FLAGS_CONDITIONAL != 0
                && self.c.type_record(source).as_conditional_type().root == Some(root);
            let root_record = self.c.semantic_state.conditional_root_record(root).clone();
            if root_record.infer_type_parameters.is_empty()
                && !self.c.is_distribution_dependent(&root_record)
                && !same_conditional_root
            {
                let permissive_check = self
                    .c
                    .get_permissive_instantiation(conditional.check_type.unwrap());
                let permissive_extends = self
                    .c
                    .get_permissive_instantiation(conditional.extends_type.unwrap());
                let skip_true = !self
                    .c
                    .is_type_assignable_to(permissive_check, permissive_extends);
                let restrictive_check = self
                    .c
                    .get_restrictive_instantiation(conditional.check_type.unwrap());
                let restrictive_extends = self
                    .c
                    .get_restrictive_instantiation(conditional.extends_type.unwrap());
                let skip_false = !skip_true
                    && self
                        .c
                        .is_type_assignable_to(restrictive_check, restrictive_extends);
                result = if skip_true {
                    TERNARY_TRUE
                } else {
                    let true_type = self.c.get_true_type_from_conditional_type(target);
                    self.is_related_to_ex(
                        source,
                        true_type,
                        RECURSION_FLAGS_TARGET,
                        false, /*reportErrors*/
                        None,  /*headMessage*/
                        intersection_state,
                    )
                };
                result &= if skip_false {
                    TERNARY_TRUE
                } else {
                    let false_type = self.c.get_false_type_from_conditional_type(target);
                    self.is_related_to_ex(
                        source,
                        false_type,
                        RECURSION_FLAGS_TARGET,
                        false, /*reportErrors*/
                        None,  /*headMessage*/
                        intersection_state,
                    )
                };
                if result != TERNARY_FALSE {
                    return result;
                }
            }
        } else if self.c.type_flags(target) & TYPE_FLAGS_TEMPLATE_LITERAL != 0 {
            let target_template = self
                .c
                .type_record(target)
                .as_template_literal_type()
                .clone();
            if self.c.type_flags(source) & TYPE_FLAGS_TEMPLATE_LITERAL != 0 {
                let source_template = self
                    .c
                    .type_record(source)
                    .as_template_literal_type()
                    .clone();
                if self.relation_is(self.c.semantic_state.comparable_relation) {
                    return if self.c.template_literal_types_definitely_unrelated(
                        &source_template,
                        &target_template,
                    ) {
                        TERNARY_FALSE
                    } else {
                        TERNARY_TRUE
                    };
                }
                self.c.instantiate_type_with_mapper_handle(
                    Some(source),
                    Some(
                        self.c
                            .semantic_state
                            .semantic_handles()
                            .report_unreliable_mapper,
                    ),
                );
            }
            if self.c.is_type_matched_by_template_literal_type(
                source,
                &target_template,
                self.c.semantic_state.compare_types_assignable,
            ) {
                return TERNARY_TRUE;
            }
        } else if self.c.type_flags(target) & TYPE_FLAGS_STRING_MAPPING != 0
            && self.c.type_flags(source) & TYPE_FLAGS_STRING_MAPPING == 0
            && self.c.is_member_of_string_mapping(source, target)
        {
            return TERNARY_TRUE;
        } else if self.c.is_generic_mapped_type(target)
            && !self.relation_is(self.c.semantic_state.identity_relation)
        {
            let target_mapped = self.c.type_record(target).as_mapped_type().clone();
            let keys_remapped = target_mapped.declaration.is_some_and(|declaration| {
                self.c
                    .store_for_node(declaration)
                    .name_type(declaration)
                    .is_some()
            });
            let template_type = self.c.get_template_type_from_mapped_type(target);
            let modifiers = self.c.get_mapped_type_modifiers(target);
            if modifiers & MAPPED_TYPE_MODIFIERS_EXCLUDE_OPTIONAL == 0 {
                let type_parameter = self.c.get_type_parameter_from_mapped_type(target);
                if !keys_remapped
                    && self.c.type_flags(template_type) & TYPE_FLAGS_INDEXED_ACCESS != 0
                {
                    let template_indexed = self
                        .c
                        .type_record(template_type)
                        .as_indexed_access_type()
                        .clone();
                    if template_indexed.object_type == Some(source)
                        && template_indexed.index_type == Some(type_parameter)
                    {
                        return TERNARY_TRUE;
                    }
                }
                let source_is_generic_mapped_type = self.c.is_generic_mapped_type(source);
                if !source_is_generic_mapped_type {
                    let target_keys = if keys_remapped {
                        self.c.get_name_type_from_mapped_type(target).unwrap()
                    } else {
                        self.c.get_constraint_type_from_mapped_type(target)
                    };
                    let source_keys = self
                        .c
                        .get_index_type_ex(source, INDEX_FLAGS_NO_INDEX_SIGNATURES);
                    let include_optional = modifiers & MAPPED_TYPE_MODIFIERS_INCLUDE_OPTIONAL != 0;
                    let filtered_by_applicability = if include_optional {
                        self.c.intersect_types(Some(target_keys), Some(source_keys))
                    } else {
                        None
                    };
                    let related_keys = if include_optional {
                        filtered_by_applicability.is_some_and(|filtered_by_applicability| {
                            self.c.type_flags(filtered_by_applicability) & TYPE_FLAGS_NEVER == 0
                        })
                    } else {
                        self.is_related_to(target_keys, source_keys, RECURSION_FLAGS_BOTH, false)
                            != TERNARY_FALSE
                    };
                    if related_keys {
                        let template_type = self.c.get_template_type_from_mapped_type(target);
                        let type_parameter = self.c.get_type_parameter_from_mapped_type(target);
                        let non_null_component = self
                            .c
                            .extract_types_of_kind(template_type, !TYPE_FLAGS_NULLABLE);
                        if !keys_remapped
                            && self.c.type_flags(non_null_component) & TYPE_FLAGS_INDEXED_ACCESS
                                != 0
                            && self
                                .c
                                .type_record(non_null_component)
                                .as_indexed_access_type()
                                .index_type
                                == Some(type_parameter)
                        {
                            let non_null_indexed = self
                                .c
                                .type_record(non_null_component)
                                .as_indexed_access_type()
                                .clone();
                            result = self.is_related_to(
                                source,
                                non_null_indexed.object_type.unwrap(),
                                RECURSION_FLAGS_TARGET,
                                report_errors,
                            );
                            if result != TERNARY_FALSE {
                                return result;
                            }
                        } else {
                            let indexing_type = if keys_remapped {
                                filtered_by_applicability.unwrap_or(target_keys)
                            } else if let Some(filtered_by_applicability) =
                                filtered_by_applicability
                            {
                                self.c.get_intersection_type(vec![
                                    filtered_by_applicability,
                                    type_parameter,
                                ])
                            } else {
                                type_parameter
                            };
                            let indexed_access_type =
                                self.c.get_indexed_access_type(source, indexing_type);
                            result = self.is_related_to(
                                indexed_access_type,
                                template_type,
                                RECURSION_FLAGS_BOTH,
                                report_errors,
                            );
                            if result != TERNARY_FALSE {
                                return result;
                            }
                        }
                    }
                    original_error_chain = self.error_chain;
                    self.restore_error_state(&save_error_state);
                }
            }
        }
        if self.c.type_flags(source) & TYPE_FLAGS_TYPE_VARIABLE != 0 {
            if self.c.type_flags(source) & TYPE_FLAGS_INDEXED_ACCESS == 0
                || self.c.type_flags(target) & TYPE_FLAGS_INDEXED_ACCESS == 0
            {
                let constraint = self
                    .c
                    .get_constraint_of_type(source)
                    .unwrap_or(self.c.semantic_state.semantic_handles().unknown_type);
                result = self.is_related_to_ex(
                    constraint,
                    target,
                    RECURSION_FLAGS_SOURCE,
                    false, /*reportErrors*/
                    None,  /*headMessage*/
                    intersection_state,
                );
                if result != TERNARY_FALSE {
                    return result;
                }
                let constraint_with_this = self.c.get_type_with_this_argument(
                    constraint,
                    Some(source),
                    false, /*needApparentType*/
                );
                result = self.is_related_to_ex(
                    constraint_with_this,
                    target,
                    RECURSION_FLAGS_SOURCE,
                    report_errors
                        && constraint != self.c.semantic_state.semantic_handles().unknown_type
                        && self.c.type_flags(target)
                            & self.c.type_flags(source)
                            & TYPE_FLAGS_TYPE_PARAMETER
                            == 0,
                    None, /*headMessage*/
                    intersection_state,
                );
                if result != TERNARY_FALSE {
                    return result;
                }
                if self.c.is_mapped_type_generic_indexed_access(source) {
                    // For an indexed access type { [P in K]: E}[X], above we have already explored an instantiation of E with X
                    // substituted for P. We also want to explore type { [P in K]: E }[C], where C is the constraint of X.
                    let indexed_access =
                        self.c.type_record(source).as_indexed_access_type().clone();
                    let index_constraint = self
                        .c
                        .get_constraint_of_type(indexed_access.index_type.unwrap());
                    if let Some(index_constraint) = index_constraint {
                        let indexed_access_type = self.c.get_indexed_access_type(
                            indexed_access.object_type.unwrap(),
                            index_constraint,
                        );
                        result = self.is_related_to(
                            indexed_access_type,
                            target,
                            RECURSION_FLAGS_SOURCE,
                            report_errors,
                        );
                        if result != TERNARY_FALSE {
                            return result;
                        }
                    }
                }
            }
        } else if self.c.type_flags(source) & TYPE_FLAGS_CONDITIONAL != 0 {
            if self.c.is_deeply_nested_type(source, &self.source_stack, 10) {
                return TERNARY_MAYBE;
            }
            if self.c.type_flags(target) & TYPE_FLAGS_CONDITIONAL != 0 {
                // Two conditional types 'T1 extends U1 ? X1 : Y1' and
                // 'T2 extends U2 ? X2 : Y2' are related if one of T1/T2 is
                // related to the other, U1 and U2 are identical, and both
                // branches are related.
                let source_conditional = self.c.type_record(source).as_conditional_type().clone();
                let target_conditional = self.c.type_record(target).as_conditional_type().clone();
                let source_root = source_conditional.root.unwrap();
                let source_params = self
                    .c
                    .semantic_state
                    .conditional_root_record(source_root)
                    .infer_type_parameters
                    .clone();
                let mut source_extends = source_conditional.extends_type.unwrap();
                let mut mapper = None;
                if !source_params.is_empty() {
                    let context = self.c.new_inference_context(
                        source_params,
                        None,
                        INFERENCE_FLAGS_NONE,
                        None,
                    );
                    {
                        let mut inferences = std::mem::take(
                            &mut self.c.inference_context_record_mut(context).inferences,
                        );
                        self.c.infer_types(
                            &mut inferences,
                            target_conditional.extends_type.unwrap(),
                            source_extends,
                            INFERENCE_PRIORITY_NO_CONSTRAINTS | INFERENCE_PRIORITY_ALWAYS_STRICT,
                            false,
                        );
                        self.c.inference_context_record_mut(context).inferences = inferences;
                    }
                    mapper = self.c.inference_context_record(context).mapper;
                    source_extends = self
                        .c
                        .instantiate_type_with_mapper_handle(Some(source_extends), mapper)
                        .unwrap();
                }
                if self
                    .c
                    .is_type_identical_to(source_extends, target_conditional.extends_type.unwrap())
                    && (self.is_related_to(
                        source_conditional.check_type.unwrap(),
                        target_conditional.check_type.unwrap(),
                        RECURSION_FLAGS_BOTH,
                        false, /*reportErrors*/
                    ) != TERNARY_FALSE
                        || self.is_related_to(
                            target_conditional.check_type.unwrap(),
                            source_conditional.check_type.unwrap(),
                            RECURSION_FLAGS_BOTH,
                            false, /*reportErrors*/
                        ) != TERNARY_FALSE)
                {
                    let source_true = self.c.get_true_type_from_conditional_type(source);
                    let source_true = self
                        .c
                        .instantiate_type_with_mapper_handle(Some(source_true), mapper)
                        .unwrap_or(source_true);
                    let target_true = self.c.get_true_type_from_conditional_type(target);
                    result = self.is_related_to_ex(
                        source_true,
                        target_true,
                        RECURSION_FLAGS_BOTH,
                        report_errors,
                        None, /*headMessage*/
                        intersection_state,
                    );
                    if result != TERNARY_FALSE {
                        let source_false = self.c.get_false_type_from_conditional_type(source);
                        let target_false = self.c.get_false_type_from_conditional_type(target);
                        result &= self.is_related_to_ex(
                            source_false,
                            target_false,
                            RECURSION_FLAGS_BOTH,
                            report_errors,
                            None, /*headMessage*/
                            intersection_state,
                        );
                    }
                    if result != TERNARY_FALSE {
                        return result;
                    }
                }
            }
            let default_constraint = self.c.get_default_constraint_of_conditional_type(source);
            result = self.is_related_to(
                default_constraint,
                target,
                RECURSION_FLAGS_SOURCE,
                report_errors,
            );
            if result != TERNARY_FALSE {
                return result;
            }
            if self.c.type_flags(target) & TYPE_FLAGS_CONDITIONAL == 0
                && self.c.has_non_circular_base_constraint(source)
            {
                if let Some(distributive_constraint) = self
                    .c
                    .get_constraint_of_distributive_conditional_type(source)
                {
                    self.restore_error_state(&save_error_state);
                    result = self.is_related_to(
                        distributive_constraint,
                        target,
                        RECURSION_FLAGS_SOURCE,
                        report_errors,
                    );
                    if result != TERNARY_FALSE {
                        return result;
                    }
                }
            }
        } else if self.c.type_flags(source) & TYPE_FLAGS_INDEX != 0 {
            let source_index = self.c.type_record(source).as_index_type().clone();
            let source_target = source_index.target.unwrap();
            let is_deferred_mapped_index = self
                .c
                .should_defer_index_type(source_target, source_index.index_flags)
                && self.c.object_flags(source_target) & OBJECT_FLAGS_MAPPED != 0;
            result = self.is_related_to(
                self.c
                    .semantic_state
                    .semantic_handles()
                    .string_number_symbol_type,
                target,
                RECURSION_FLAGS_SOURCE,
                report_errors && !is_deferred_mapped_index,
            );
            if result != TERNARY_FALSE {
                return result;
            }
            if is_deferred_mapped_index {
                let mapped_type = source_target;
                let name_type = self.c.get_name_type_from_mapped_type(mapped_type);
                let source_mapped_keys = if let Some(name_type) = name_type {
                    if self
                        .c
                        .is_mapped_type_with_keyof_constraint_declaration(mapped_type)
                    {
                        self.c.get_apparent_mapped_type_keys(name_type, mapped_type)
                    } else {
                        name_type
                    }
                } else {
                    self.c.get_constraint_type_from_mapped_type(mapped_type)
                };
                result = self.is_related_to(
                    source_mapped_keys,
                    target,
                    RECURSION_FLAGS_SOURCE,
                    report_errors,
                );
                if result != TERNARY_FALSE {
                    return result;
                }
            }
        } else if self.c.type_flags(source) & TYPE_FLAGS_TEMPLATE_LITERAL != 0
            && self.c.type_flags(target) & TYPE_FLAGS_OBJECT == 0
        {
            if self.c.type_flags(target) & TYPE_FLAGS_TEMPLATE_LITERAL == 0 {
                if let Some(constraint) = self.c.get_base_constraint_of_type(source) {
                    if constraint != source {
                        result = self.is_related_to_ex(
                            constraint,
                            target,
                            RECURSION_FLAGS_SOURCE,
                            report_errors,
                            None, /*headMessage*/
                            intersection_state,
                        );
                        if result != TERNARY_FALSE {
                            return result;
                        }
                    }
                }
            }
        } else if self.c.type_flags(source) & TYPE_FLAGS_STRING_MAPPING != 0 {
            if self.c.type_flags(target) & TYPE_FLAGS_STRING_MAPPING != 0 {
                if !self.c.same_optional_symbol_identity(
                    self.c.type_symbol_identity(source),
                    self.c.type_symbol_identity(target),
                ) {
                    return TERNARY_FALSE;
                }
                let source_mapping = self.c.type_record(source).as_string_mapping_type().clone();
                let target_mapping = self.c.type_record(target).as_string_mapping_type().clone();
                result = self.is_related_to_ex(
                    source_mapping.target.unwrap(),
                    target_mapping.target.unwrap(),
                    RECURSION_FLAGS_BOTH,
                    report_errors,
                    None, /*headMessage*/
                    intersection_state,
                );
                if result != TERNARY_FALSE {
                    return result;
                }
            } else {
                let constraint = self.c.get_base_constraint_of_type(source);
                if let Some(constraint) = constraint {
                    result = self.is_related_to(
                        constraint,
                        target,
                        RECURSION_FLAGS_SOURCE,
                        report_errors,
                    );
                    if result != TERNARY_FALSE {
                        return result;
                    }
                }
            }
        } else {
            if !self.relation_is(self.c.semantic_state.subtype_relation)
                && !self.relation_is(self.c.semantic_state.strict_subtype_relation)
                && self.c.is_partial_mapped_type(target)
                && self.c.is_empty_object_type(source)
            {
                return TERNARY_TRUE;
            }
            if self.c.is_generic_mapped_type(target) {
                if self.c.is_generic_mapped_type(source) {
                    result = self.mapped_type_related_to(source, target, report_errors);
                    if result != TERNARY_FALSE {
                        return result;
                    }
                }
                return TERNARY_FALSE;
            }
            let source_is_primitive = self.c.type_flags(source) & TYPE_FLAGS_PRIMITIVE != 0;
            if !self.relation_is(self.c.semantic_state.identity_relation) {
                source = self.c.get_apparent_type(source);
            } else if self.c.is_generic_mapped_type(source) {
                return TERNARY_FALSE;
            }
            if self.c.object_flags(source) & OBJECT_FLAGS_REFERENCE != 0
                && self.c.object_flags(target) & OBJECT_FLAGS_REFERENCE != 0
                && self.c.type_target(source) == self.c.type_target(target)
                && !self.c.is_tuple_type(source)
                && !self.c.is_marker_type(source)
                && !self.c.is_marker_type(target)
            {
                // When strictNullChecks is disabled, the element type of the empty array literal is undefinedWideningType,
                // and an empty array literal wouldn't be assignable to a `never[]` without this check.
                if self.c.is_empty_array_literal_type(source) {
                    return TERNARY_TRUE;
                }
                // We have type references to the same generic type, and the type references are not marker
                // type references (which are intended by be compared structurally). Obtain the variance
                // information for the type parameters and relate the type arguments accordingly.
                let variance_state = self.c.get_variances_state(self.c.type_target(source));
                // We return Ternary.Maybe for a recursive invocation of getVariances (signaled by emptyArray). This
                // effectively means we measure variance only from type parameter occurrences that aren't nested in
                // recursive instantiations of the generic type.
                if variance_state.is_computing() {
                    return TERNARY_UNKNOWN;
                }
                let variances = variance_state.into_variances_or_empty();
                result = self.type_reference_arguments_related_to(
                    source,
                    target,
                    &variances,
                    report_errors,
                    intersection_state,
                );
                if result != TERNARY_FALSE {
                    return result;
                }
                if variances
                    .iter()
                    .any(|v| v & VARIANCE_FLAGS_ALLOWS_STRUCTURAL_FALLBACK != 0)
                {
                    original_error_chain = None;
                    self.restore_error_state(&save_error_state);
                } else {
                    let allow_structural_fallback = self
                        .c
                        .has_covariant_void_argument_for_type_reference(target, &variances);
                    variance_check_failed = !allow_structural_fallback;
                    if !variances.is_empty() && !allow_structural_fallback {
                        let has_invariant = variances
                            .iter()
                            .any(|v| v & VARIANCE_FLAGS_VARIANCE_MASK == VARIANCE_FLAGS_INVARIANT);
                        if variance_check_failed && !(report_errors && has_invariant) {
                            return TERNARY_FALSE;
                        }
                        original_error_chain = self.error_chain;
                        self.restore_error_state(&save_error_state);
                    }
                }
            }
            let target_is_array = self.c.is_array_type(target);
            let target_is_readonly_array = target_is_array && self.c.is_readonly_array_type(target);
            let source_every_array_or_tuple = target_is_readonly_array
                && every_type(self.c, source, |checker, ty| {
                    checker.is_array_or_tuple_type(ty)
                });
            let source_every_mutable_tuple = target_is_array
                && every_type(self.c, source, |checker, ty| {
                    checker.is_tuple_type(ty) && !checker.target_tuple_type_record(ty).readonly
                });
            if target_is_array && (source_every_array_or_tuple || source_every_mutable_tuple) {
                if !self.relation_is(self.c.semantic_state.identity_relation) {
                    let number_type = self.c.semantic_state.semantic_handles().number_type;
                    let any_type = self.c.semantic_state.semantic_handles().any_type;
                    let source_index_type =
                        self.c
                            .get_index_type_of_type_ex(source, number_type, any_type);
                    let target_index_type =
                        self.c
                            .get_index_type_of_type_ex(target, number_type, any_type);
                    return self.is_related_to(
                        source_index_type,
                        target_index_type,
                        RECURSION_FLAGS_BOTH,
                        report_errors,
                    );
                }
                return TERNARY_FALSE;
            }
            if self.c.is_generic_tuple_type(source)
                && self.c.is_tuple_type(target)
                && !self.c.is_generic_tuple_type(target)
            {
                let constraint = self.c.get_base_constraint_or_type(source);
                if constraint != source {
                    return self.is_related_to(
                        constraint,
                        target,
                        RECURSION_FLAGS_SOURCE,
                        report_errors,
                    );
                }
            }
            if (self.relation_is(self.c.semantic_state.subtype_relation)
                || self.relation_is(self.c.semantic_state.strict_subtype_relation))
                && self.c.is_empty_object_type(target)
                && self.c.object_flags(target) & OBJECT_FLAGS_FRESH_LITERAL != 0
                && !self.c.is_empty_object_type(source)
            {
                return TERNARY_FALSE;
            }
            if self.c.type_flags(source) & (TYPE_FLAGS_OBJECT | TYPE_FLAGS_INTERSECTION) != 0
                && self.c.type_flags(target) & TYPE_FLAGS_OBJECT != 0
            {
                let report_structural_errors = report_errors
                    && match (self.error_chain, save_error_state.error_chain) {
                        (Some(left), Some(right)) => left == right,
                        (None, None) => true,
                        _ => false,
                    }
                    && !source_is_primitive;
                result = self.properties_related_to(
                    source,
                    target,
                    report_structural_errors,
                    collections::Set::new(), /*excludedProperties*/
                    false,                   /*optionalsOnly*/
                    intersection_state,
                );
                if result != TERNARY_FALSE {
                    result &= self.signatures_related_to(
                        source,
                        target,
                        SIGNATURE_KIND_CALL,
                        report_structural_errors,
                        intersection_state,
                    );
                    if result != TERNARY_FALSE {
                        result &= self.signatures_related_to(
                            source,
                            target,
                            SIGNATURE_KIND_CONSTRUCT,
                            report_structural_errors,
                            intersection_state,
                        );
                        if result != TERNARY_FALSE {
                            result &= self.index_signatures_related_to(
                                source,
                                target,
                                source_is_primitive,
                                report_structural_errors,
                                intersection_state,
                            );
                        }
                    }
                }
                if result != TERNARY_FALSE {
                    if !variance_check_failed {
                        return result;
                    }
                    if original_error_chain.is_some() {
                        self.error_chain = original_error_chain;
                    } else if self.error_chain.is_none() {
                        self.error_chain = save_error_state.error_chain;
                    }
                }
            }
            if self.c.type_flags(source) & (TYPE_FLAGS_OBJECT | TYPE_FLAGS_INTERSECTION) != 0
                && self.c.type_flags(target) & TYPE_FLAGS_UNION != 0
            {
                let object_only_target = self.c.extract_types_of_kind(
                    target,
                    TYPE_FLAGS_OBJECT | TYPE_FLAGS_INTERSECTION | TYPE_FLAGS_SUBSTITUTION,
                );
                if self.c.type_flags(object_only_target) & TYPE_FLAGS_UNION != 0 {
                    result = self.type_related_to_discriminated_type(source, object_only_target);
                    if result != TERNARY_FALSE {
                        return result;
                    }
                }
            }
        }
        TERNARY_FALSE
    }

    fn type_arguments_related_to(
        &mut self,
        sources: &[TypeHandle],
        targets: &[TypeHandle],
        variances: &[VarianceFlags],
        report_errors: bool,
        intersection_state: IntersectionState,
    ) -> Ternary {
        if sources.len() != targets.len()
            && self.relation_is(self.c.semantic_state.identity_relation)
        {
            return TERNARY_FALSE;
        }
        let length = sources.len().min(targets.len());
        let mut result = TERNARY_TRUE;
        for i in 0..length {
            let related = self.type_argument_related_to(
                sources[i],
                targets[i],
                variances,
                i,
                report_errors,
                intersection_state,
            );
            if related == TERNARY_FALSE {
                return TERNARY_FALSE;
            }
            result &= related;
        }
        result
    }

    fn type_argument_related_to(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        variances: &[VarianceFlags],
        index: usize,
        report_errors: bool,
        intersection_state: IntersectionState,
    ) -> Ternary {
        // When variance information isn't available we default to covariance. This happens
        // in the process of computing variance information for recursive types and when
        // comparing 'this' type arguments.
        let variance_flags = if index < variances.len() {
            variances[index]
        } else {
            VARIANCE_FLAGS_COVARIANT
        };
        let variance = variance_flags & VARIANCE_FLAGS_VARIANCE_MASK;
        // We ignore arguments for independent type parameters (because they're never witnessed).
        if variance == VARIANCE_FLAGS_INDEPENDENT {
            return TERNARY_TRUE;
        }
        if variance_flags & VARIANCE_FLAGS_UNMEASURABLE != 0 {
            // Even an `Unmeasurable` variance works out without a structural check if the source and target are _identical_.
            // We can't simply assume invariance, because `Unmeasurable` marks nonlinear relations, for example, a relation tainted by
            // the `-?` modifier in a mapped type (where, no matter how the inputs are related, the outputs still might not be)
            if self.relation_is(self.c.semantic_state.identity_relation) {
                return self.is_related_to(
                    source,
                    target,
                    RECURSION_FLAGS_BOTH,
                    false, /*reportErrors*/
                );
            }
            return self.c.compare_types_identical(source, target);
        }
        // Propagate unreliable variance flag
        if self.c.in_variance_computation() && variance_flags & VARIANCE_FLAGS_UNRELIABLE != 0 {
            let report_unreliable_mapper = self
                .c
                .semantic_state
                .semantic_handles()
                .report_unreliable_mapper;
            self.c
                .instantiate_type_with_mapper_handle(Some(source), Some(report_unreliable_mapper));
        }
        if variance == VARIANCE_FLAGS_COVARIANT {
            self.is_related_to_ex(
                source,
                target,
                RECURSION_FLAGS_BOTH,
                report_errors,
                None, /*headMessage*/
                intersection_state,
            )
        } else if variance == VARIANCE_FLAGS_CONTRAVARIANT {
            self.is_related_to_ex(
                target,
                source,
                RECURSION_FLAGS_BOTH,
                report_errors,
                None, /*headMessage*/
                intersection_state,
            )
        } else if variance == VARIANCE_FLAGS_BIVARIANT {
            // In the bivariant case we first compare contravariantly without reporting
            // errors. Then, if that doesn't succeed, we compare covariantly with error
            // reporting. Thus, error elaboration will be based on the covariant check,
            // which is generally easier to reason about.
            let mut related = self.is_related_to(
                target,
                source,
                RECURSION_FLAGS_BOTH,
                false, /*reportErrors*/
            );
            if related == TERNARY_FALSE {
                related = self.is_related_to_ex(
                    source,
                    target,
                    RECURSION_FLAGS_BOTH,
                    report_errors,
                    None, /*headMessage*/
                    intersection_state,
                );
            }
            related
        } else {
            // In the invariant case we first compare covariantly, and only when that
            // succeeds do we proceed to compare contravariantly. Thus, error elaboration
            // will typically be based on the covariant check.
            let mut related = self.is_related_to_ex(
                source,
                target,
                RECURSION_FLAGS_BOTH,
                report_errors,
                None, /*headMessage*/
                intersection_state,
            );
            if related != TERNARY_FALSE {
                related &= self.is_related_to_ex(
                    target,
                    source,
                    RECURSION_FLAGS_BOTH,
                    report_errors,
                    None, /*headMessage*/
                    intersection_state,
                );
            }
            related
        }
    }

    fn type_reference_arguments_related_to(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        variances: &[VarianceFlags],
        report_errors: bool,
        intersection_state: IntersectionState,
    ) -> Ternary {
        let source_type_arguments = self.c.ensure_type_arguments_available(source);
        let target_type_arguments = self.c.ensure_type_arguments_available(target);
        let source_len = source_type_arguments
            .as_ref()
            .map_or_else(|| self.c.cached_type_arguments_len(source), Vec::len);
        let target_len = target_type_arguments
            .as_ref()
            .map_or_else(|| self.c.cached_type_arguments_len(target), Vec::len);
        if source_len != target_len && self.relation_is(self.c.semantic_state.identity_relation) {
            return TERNARY_FALSE;
        }
        let length = source_len.min(target_len);
        let mut result = TERNARY_TRUE;
        for i in 0..length {
            let s = source_type_arguments
                .as_ref()
                .map_or_else(|| self.c.cached_type_argument_at(source, i), |args| args[i]);
            let t = target_type_arguments
                .as_ref()
                .map_or_else(|| self.c.cached_type_argument_at(target, i), |args| args[i]);
            let related = self.type_argument_related_to(
                s,
                t,
                variances,
                i,
                report_errors,
                intersection_state,
            );
            if related == TERNARY_FALSE {
                return TERNARY_FALSE;
            }
            result &= related;
        }
        result
    }

    // A type [P in S]: X is related to a type [Q in T]: Y if T is related to S and X' is
    // related to Y, where X' is an instantiation of X in which P is replaced with Q. Notice
    // that S and T are contra-variant whereas X and Y are co-variant.
    fn mapped_type_related_to(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        report_errors: bool,
    ) -> Ternary {
        let modifiers_related = self.relation_is(self.c.semantic_state.comparable_relation)
            || self.relation_is(self.c.semantic_state.identity_relation)
                && self.c.get_mapped_type_modifiers(source)
                    == self.c.get_mapped_type_modifiers(target)
            || !self.relation_is(self.c.semantic_state.identity_relation)
                && self.c.get_combined_mapped_type_optionality(source)
                    <= self.c.get_combined_mapped_type_optionality(target);
        if modifiers_related {
            let target_constraint = self.c.get_constraint_type_from_mapped_type(target);
            let source_constraint_type = self.c.get_constraint_type_from_mapped_type(source);
            let optionality = self.c.get_combined_mapped_type_optionality(source);
            let source_constraint_mapper = if optionality < 0 {
                self.c
                    .semantic_state
                    .semantic_handles()
                    .report_unmeasurable_mapper
            } else {
                self.c
                    .semantic_state
                    .semantic_handles()
                    .report_unreliable_mapper
            };
            let source_constraint = self.c.instantiate_type_with_mapper_handle(
                Some(source_constraint_type),
                Some(source_constraint_mapper),
            );
            let result = self.is_related_to(
                target_constraint,
                source_constraint.unwrap(),
                RECURSION_FLAGS_BOTH,
                report_errors,
            );
            if result != TERNARY_FALSE {
                let source_type_parameter = self.c.get_type_parameter_from_mapped_type(source);
                let target_type_parameter = self.c.get_type_parameter_from_mapped_type(target);
                let mapper = self
                    .c
                    .new_simple_type_mapper_handle(source_type_parameter, target_type_parameter);
                let source_name_type = self.c.get_name_type_from_mapped_type(source);
                let instantiated_source_name = self
                    .c
                    .instantiate_type_with_mapper_handle(source_name_type, Some(mapper));
                let target_name_type = self.c.get_name_type_from_mapped_type(target);
                let instantiated_target_name = self
                    .c
                    .instantiate_type_with_mapper_handle(target_name_type, Some(mapper));
                if instantiated_source_name == instantiated_target_name {
                    let source_template_type = self.c.get_template_type_from_mapped_type(source);
                    let instantiated_source_template = self
                        .c
                        .instantiate_type_with_mapper_handle(
                            Some(source_template_type),
                            Some(mapper),
                        )
                        .unwrap();
                    let target_template_type = self.c.get_template_type_from_mapped_type(target);
                    return result
                        & self.is_related_to(
                            instantiated_source_template,
                            target_template_type,
                            RECURSION_FLAGS_BOTH,
                            report_errors,
                        );
                }
            }
        }
        TERNARY_FALSE
    }

    fn type_related_to_discriminated_type(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
    ) -> Ternary {
        // 1. Generate the combinations of discriminant properties & types 'source' can satisfy.
        //    a. If the number of combinations is above a set limit, the comparison is too complex.
        // 2. Filter 'target' to the subset of types whose discriminants exist in the matrix.
        //    a. If 'target' does not satisfy all discriminants in the matrix, 'source' is not related.
        // 3. For each type in the filtered 'target', determine if all non-discriminant properties of
        //    'target' are related to a property in 'source'.
        //
        // NOTE: See ~/tests/cases/conformance/types/typeRelationships/assignmentCompatibility/assignmentCompatWithDiscriminatedUnion.ts
        //       for examples.
        let source_properties = self.c.relater_get_properties_of_type_identities(source);
        let source_properties_filtered = self
            .c
            .find_discriminant_properties(source_properties, target);
        if source_properties_filtered.is_empty() {
            return TERNARY_FALSE;
        }
        let mut num_combinations = 1usize;
        for source_property in source_properties_filtered.iter() {
            let source_property_type = self
                .c
                .get_non_missing_type_of_symbol_identity(*source_property);
            num_combinations *= count_types(self.c, source_property_type);
            if num_combinations > 25 {
                if let Some(tr) = self.c.tracer {
                    tr.instant(
                        tracing::PHASE_CHECK_TYPES,
                        "typeRelatedToDiscriminatedType_DepthLimit",
                        tracing::args([
                            ("sourceId", serde_json::json!(self.c.type_id(source))),
                            ("targetId", serde_json::json!(self.c.type_id(target))),
                            ("numCombinations", serde_json::json!(num_combinations)),
                        ]),
                    );
                }
                return TERNARY_FALSE;
            }
            if num_combinations == 0 {
                return TERNARY_FALSE;
            }
        }
        let mut source_discriminant_types = Vec::new();
        let mut excluded_properties = collections::Set::new();
        for source_property in source_properties_filtered.iter() {
            let discriminant_type = self
                .c
                .get_non_missing_type_of_symbol_identity(*source_property);
            source_discriminant_types.push(self.c.distributed_types(discriminant_type));
            excluded_properties.add(self.c.missing_name_symbol_identity_name(*source_property));
        }
        let mut matching_types = Vec::new();
        for combination_index in 0..num_combinations {
            let mut combination = vec![
                self.c.semantic_state.semantic_handles().never_type;
                source_discriminant_types.len()
            ];
            let mut n = combination_index;
            for j in (0..source_discriminant_types.len()).rev() {
                let source_types = &source_discriminant_types[j];
                combination[j] = source_types[n % source_types.len()];
                n /= source_types.len();
            }
            let mut has_match = false;
            let target_types_len = self.c.type_types_len(target);
            'outer: for target_index in 0..target_types_len {
                let t = self.c.type_type_at(target, target_index);
                for i in 0..source_properties_filtered.len() {
                    let source_property = source_properties_filtered[i];
                    let source_property_name =
                        self.c.missing_name_symbol_identity_name(source_property);
                    let target_property = self.c.get_property_of_type(t, &source_property_name);
                    if target_property.is_none() {
                        continue 'outer;
                    }
                    let target_property = target_property.unwrap();
                    let target_property_identity = target_property;
                    if source_property == target_property_identity {
                        continue;
                    }
                    let discriminant_type = combination[i];
                    let skip_optional = self.c.strict_null_checks()
                        || self.relation_is(self.c.semantic_state.comparable_relation);
                    let related = self.property_related_to(
                        source,
                        target,
                        source_property,
                        target_property_identity,
                        Some(discriminant_type),
                        false, /*reportErrors*/
                        INTERSECTION_STATE_NONE,
                        skip_optional,
                    );
                    if related == TERNARY_FALSE {
                        continue 'outer;
                    }
                }
                matching_types = core::append_if_unique(&matching_types, t);
                has_match = true;
            }
            if !has_match {
                return TERNARY_FALSE;
            }
        }
        let mut result = TERNARY_TRUE;
        for t in matching_types {
            result &= self.properties_related_to(
                source,
                t,
                false, /*reportErrors*/
                excluded_properties.clone(),
                false, /*optionalsOnly*/
                INTERSECTION_STATE_NONE,
            );
            if result != TERNARY_FALSE {
                result &= self.signatures_related_to(
                    source,
                    t,
                    SIGNATURE_KIND_CALL,
                    false, /*reportErrors*/
                    INTERSECTION_STATE_NONE,
                );
                if result != TERNARY_FALSE {
                    result &= self.signatures_related_to(
                        source,
                        t,
                        SIGNATURE_KIND_CONSTRUCT,
                        false, /*reportErrors*/
                        INTERSECTION_STATE_NONE,
                    );
                    if result != TERNARY_FALSE
                        && !(self.c.is_tuple_type(source) && self.c.is_tuple_type(t))
                    {
                        result &= self.index_signatures_related_to(
                            source,
                            t,
                            false, /*sourceIsPrimitive*/
                            false, /*reportErrors*/
                            INTERSECTION_STATE_NONE,
                        );
                    }
                }
            }
            if result == TERNARY_FALSE {
                return result;
            }
        }
        result
    }

    fn properties_related_to(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        report_errors: bool,
        excluded_properties: collections::Set<String>,
        optionals_only: bool,
        intersection_state: IntersectionState,
    ) -> Ternary {
        if self.relation_is(self.c.semantic_state.identity_relation) {
            return self.properties_identical_to(source, target, excluded_properties);
        }
        let mut result = TERNARY_TRUE;
        if self.c.is_tuple_type(target) {
            if self.c.is_array_or_tuple_type(source) {
                let target_tuple = self.c.target_tuple_type_record(target).clone();
                let source_tuple = if self.c.is_tuple_type(source) {
                    Some(self.c.target_tuple_type_record(source).clone())
                } else {
                    None
                };
                if !target_tuple.readonly
                    && (self.c.is_readonly_array_type(source)
                        || source_tuple.as_ref().is_some_and(|tuple| tuple.readonly))
                {
                    return TERNARY_FALSE;
                }
                let source_arity = self.c.get_type_reference_arity(source);
                let target_arity = self.c.get_type_reference_arity(target);
                let source_rest = if let Some(source_tuple) = source_tuple.as_ref() {
                    source_tuple.combined_flags & ELEMENT_FLAGS_REST != 0
                } else {
                    true
                };
                let target_has_rest_element =
                    target_tuple.combined_flags & ELEMENT_FLAGS_VARIABLE != 0;
                let source_min_length = if let Some(source_tuple) = source_tuple.as_ref() {
                    source_tuple.min_length
                } else {
                    0
                };
                let target_min_length = target_tuple.min_length;
                if !source_rest && source_arity < target_min_length {
                    if report_errors {
                        self.report_error(
                            &*diagnostics::SOURCE_HAS_0_ELEMENT_S_BUT_TARGET_REQUIRES_1,
                            vec![source_arity.into(), target_min_length.into()],
                        );
                    }
                    return TERNARY_FALSE;
                }
                if !target_has_rest_element && target_arity < source_min_length {
                    if report_errors {
                        self.report_error(
                            &*diagnostics::SOURCE_HAS_0_ELEMENT_S_BUT_TARGET_ALLOWS_ONLY_1,
                            vec![source_min_length.into(), target_arity.into()],
                        );
                    }
                    return TERNARY_FALSE;
                }
                if !target_has_rest_element && (source_rest || target_arity < source_arity) {
                    if report_errors {
                        if source_min_length < target_min_length {
                            self.report_error(
                                &diagnostics::TARGET_REQUIRES_0_ELEMENT_S_BUT_SOURCE_MAY_HAVE_FEWER,
                                vec![target_min_length.into()],
                            );
                        } else {
                            self.report_error(
                                &diagnostics::TARGET_ALLOWS_ONLY_0_ELEMENT_S_BUT_SOURCE_MAY_HAVE_MORE,
                                vec![target_arity.into()],
                            );
                        }
                    }
                    return TERNARY_FALSE;
                }
                let source_type_arguments = self.c.ensure_type_arguments_available(source);
                let target_type_arguments = self.c.ensure_type_arguments_available(target);
                let target_start_count =
                    relater_get_start_element_count(&target_tuple, ELEMENT_FLAGS_NON_REST);
                let target_end_count = get_end_element_count(&target_tuple, ELEMENT_FLAGS_NON_REST);
                let mut can_exclude_discriminants = !excluded_properties.is_empty();
                for source_position in 0..source_arity {
                    let source_flags = if let Some(source_tuple) = source_tuple.as_ref() {
                        source_tuple.element_infos[source_position].flags
                    } else {
                        ELEMENT_FLAGS_REST
                    };
                    let source_position_from_end = source_arity - 1 - source_position;
                    let target_position: isize =
                        if target_has_rest_element && source_position >= target_start_count {
                            target_arity as isize
                                - 1
                                - source_position_from_end.min(target_end_count) as isize
                        } else {
                            source_position as isize
                        };
                    let target_flags = if target_position >= 0 {
                        target_tuple.element_infos[target_position as usize].flags
                    } else {
                        ELEMENT_FLAGS_NONE
                    };
                    if target_flags & ELEMENT_FLAGS_VARIADIC != 0
                        && source_flags & ELEMENT_FLAGS_VARIADIC == 0
                    {
                        if report_errors {
                            self.report_error(
                                &diagnostics::SOURCE_PROVIDES_NO_MATCH_FOR_VARIADIC_ELEMENT_AT_POSITION_0_IN_TARGET,
                                vec![target_position.into()],
                            );
                        }
                        return TERNARY_FALSE;
                    }
                    if source_flags & ELEMENT_FLAGS_VARIADIC != 0
                        && target_flags & ELEMENT_FLAGS_VARIABLE == 0
                    {
                        if report_errors {
                            self.report_error(
                                &diagnostics::VARIADIC_ELEMENT_AT_POSITION_0_IN_SOURCE_DOES_NOT_MATCH_ELEMENT_AT_POSITION_1_IN_TARGET,
                                vec![source_position.into(), target_position.into()],
                            );
                        }
                        return TERNARY_FALSE;
                    }
                    if target_flags & ELEMENT_FLAGS_REQUIRED != 0
                        && source_flags & ELEMENT_FLAGS_REQUIRED == 0
                    {
                        if report_errors {
                            self.report_error(&diagnostics::SOURCE_PROVIDES_NO_MATCH_FOR_REQUIRED_ELEMENT_AT_POSITION_0_IN_TARGET, vec![target_position.into()]);
                        }
                        return TERNARY_FALSE;
                    }
                    if can_exclude_discriminants {
                        if source_flags & ELEMENT_FLAGS_VARIABLE != 0
                            || target_flags & ELEMENT_FLAGS_VARIABLE != 0
                        {
                            can_exclude_discriminants = false;
                        }
                        if can_exclude_discriminants
                            && excluded_properties.has(&source_position.to_string())
                        {
                            continue;
                        }
                    }
                    let source_type_argument = self.c.type_argument_at_from(
                        source,
                        source_type_arguments.as_deref(),
                        source_position,
                    );
                    let source_type = self.c.remove_missing_type(
                        source_type_argument,
                        source_flags & target_flags & ELEMENT_FLAGS_OPTIONAL != 0,
                    );
                    let target_type = self.c.type_argument_at_from(
                        target,
                        target_type_arguments.as_deref(),
                        target_position as usize,
                    );
                    let target_check_type = if source_flags & ELEMENT_FLAGS_VARIADIC != 0
                        && target_flags & ELEMENT_FLAGS_REST != 0
                    {
                        self.c.create_array_type(target_type)
                    } else {
                        self.c.remove_missing_type(
                            target_type,
                            target_flags & ELEMENT_FLAGS_OPTIONAL != 0,
                        )
                    };
                    let related = self.is_related_to_ex(
                        source_type,
                        target_check_type,
                        RECURSION_FLAGS_BOTH,
                        report_errors,
                        None, /*headMessage*/
                        intersection_state,
                    );
                    if related == TERNARY_FALSE {
                        if report_errors && (target_arity > 1 || source_arity > 1) {
                            if target_has_rest_element
                                && source_position >= target_start_count
                                && source_position_from_end >= target_end_count
                                && target_start_count != source_arity - target_end_count - 1
                            {
                                self.report_error(
                                    &diagnostics::TYPE_AT_POSITIONS_0_THROUGH_1_IN_SOURCE_IS_NOT_COMPATIBLE_WITH_TYPE_AT_POSITION_2_IN_TARGET,
                                    vec![
                                        target_start_count.into(),
                                        (source_arity - target_end_count - 1).into(),
                                        target_position.into(),
                                    ],
                                );
                            } else {
                                self.report_error(&diagnostics::TYPE_AT_POSITION_0_IN_SOURCE_IS_NOT_COMPATIBLE_WITH_TYPE_AT_POSITION_1_IN_TARGET, vec![source_position.into(), target_position.into()]);
                            }
                        }
                        return TERNARY_FALSE;
                    }
                    result &= related;
                }
                return result;
            }
            if self.c.target_tuple_type_record(target).combined_flags & ELEMENT_FLAGS_VARIABLE != 0
            {
                return TERNARY_FALSE;
            }
        }
        let require_optional_properties = (self
            .relation_is(self.c.semantic_state.subtype_relation)
            || self.relation_is(self.c.semantic_state.strict_subtype_relation))
            && self.c.object_flags(source) & OBJECT_FLAGS_OBJECT_LITERAL == 0
            && !self.c.is_empty_array_literal_type(source)
            && !self.c.is_tuple_type(source);
        let unmatched_property = self.c.get_unmatched_property(
            source,
            target,
            require_optional_properties,
            false, /*matchDiscriminantProperties*/
        );
        if let Some(unmatched_property) = unmatched_property {
            if report_errors
                && self
                    .c
                    .should_report_unmatched_property_error(source, target)
            {
                self.report_unmatched_property(
                    source,
                    target,
                    unmatched_property,
                    require_optional_properties,
                );
            }
            return TERNARY_FALSE;
        }
        if self.c.object_flags(target) & OBJECT_FLAGS_OBJECT_LITERAL != 0 {
            let source_properties = self.c.relater_get_properties_of_type_list(source);
            for source_prop_index in 0..source_properties.len(self.c) {
                let source_prop = source_properties.get(self.c, source_prop_index);
                if property_identity_is_excluded(self.c, source_prop, &excluded_properties) {
                    continue;
                }
                let source_prop_name = self.c.missing_name_symbol_identity_name(source_prop);
                if self
                    .c
                    .get_property_of_object_type(target, &source_prop_name)
                    .is_none()
                {
                    if report_errors {
                        let target_string = self.c.type_to_string_public(target);
                        self.report_error(
                            &*diagnostics::PROPERTY_0_DOES_NOT_EXIST_ON_TYPE_1,
                            vec![source_prop_name.into(), target_string.into()],
                        );
                    }
                    return TERNARY_FALSE;
                }
            }
        }
        // We only call this for union target types when we're attempting to do excess property checking - in those cases, we want to get _all possible props_
        // from the target union, across all members.
        let properties = self.c.relater_get_properties_of_type_list(target);
        let numeric_names_only = self.c.is_tuple_type(source) && self.c.is_tuple_type(target);
        for target_prop_index in 0..properties.len(self.c) {
            let target_prop = properties.get(self.c, target_prop_index);
            if property_identity_is_excluded(self.c, target_prop, &excluded_properties) {
                continue;
            }
            let name = self.c.missing_name_symbol_identity_name(target_prop);
            let target_prop_flags = self.c.symbol_identity_flags(target_prop);
            if target_prop_flags & ast::SYMBOL_FLAGS_PROTOTYPE == 0
                && (!numeric_names_only || is_numeric_literal_name(&name) || name == "length")
                && (!optionals_only || target_prop_flags & ast::SYMBOL_FLAGS_OPTIONAL != 0)
            {
                let source_prop = self.c.get_property_of_type(source, &name);
                if let Some(source_prop) = source_prop {
                    if source_prop != target_prop {
                        let skip_optional =
                            self.relation_is(self.c.semantic_state.comparable_relation);
                        let related = self.property_related_to(
                            source,
                            target,
                            source_prop,
                            target_prop,
                            None,
                            report_errors,
                            intersection_state,
                            skip_optional,
                        );
                        if related == TERNARY_FALSE {
                            return TERNARY_FALSE;
                        }
                        result &= related;
                    }
                }
            }
        }
        result
    }

    fn property_related_to(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        source_prop: SymbolIdentity,
        target_prop: SymbolIdentity,
        source_property_type: Option<TypeHandle>,
        report_errors: bool,
        intersection_state: IntersectionState,
        skip_optional: bool,
    ) -> Ternary {
        let source_prop_flags = self
            .c
            .relater_declaration_modifier_flags_from_symbol_identity(source_prop);
        let target_prop_flags = self
            .c
            .relater_declaration_modifier_flags_from_symbol_identity(target_prop);
        if source_prop_flags & ast::MODIFIER_FLAGS_PRIVATE != 0
            || target_prop_flags & ast::MODIFIER_FLAGS_PRIVATE != 0
        {
            if self
                .c
                .missing_name_symbol_identity_value_declaration(source_prop)
                != self
                    .c
                    .missing_name_symbol_identity_value_declaration(target_prop)
            {
                if report_errors {
                    if source_prop_flags & ast::MODIFIER_FLAGS_PRIVATE != 0
                        && target_prop_flags & ast::MODIFIER_FLAGS_PRIVATE != 0
                    {
                        let target_prop_string =
                            self.c.relater_symbol_identity_to_string(target_prop);
                        self.report_error(
                            &*diagnostics::TYPES_HAVE_SEPARATE_DECLARATIONS_OF_A_PRIVATE_PROPERTY_0,
                            vec![target_prop_string.into()],
                        );
                    } else {
                        let target_prop_string =
                            self.c.relater_symbol_identity_to_string(target_prop);
                        let source_type_string = self.c.type_to_string_public(
                            if source_prop_flags & ast::MODIFIER_FLAGS_PRIVATE != 0 {
                                source
                            } else {
                                target
                            },
                        );
                        let target_type_string = self.c.type_to_string_public(
                            if source_prop_flags & ast::MODIFIER_FLAGS_PRIVATE != 0 {
                                target
                            } else {
                                source
                            },
                        );
                        self.report_error(
                            &*diagnostics::PROPERTY_0_IS_PRIVATE_IN_TYPE_1_BUT_NOT_IN_TYPE_2,
                            vec![
                                target_prop_string.into(),
                                source_type_string.into(),
                                target_type_string.into(),
                            ],
                        );
                    }
                }
                return TERNARY_FALSE;
            }
        } else if target_prop_flags & ast::MODIFIER_FLAGS_PROTECTED != 0 {
            if !self
                .c
                .is_valid_override_of_identity(source_prop, target_prop)
            {
                if report_errors {
                    let target_prop_string = self.c.relater_symbol_identity_to_string(target_prop);
                    let source_type = self
                        .c
                        .get_declaring_class_identity(source_prop)
                        .unwrap_or(source);
                    let target_type = self
                        .c
                        .get_declaring_class_identity(target_prop)
                        .unwrap_or(target);
                    let source_type_string = self.c.type_to_string_public(source_type);
                    let target_type_string = self.c.type_to_string_public(target_type);
                    self.report_error(&*diagnostics::PROPERTY_0_IS_PROTECTED_BUT_TYPE_1_IS_NOT_A_CLASS_DERIVED_FROM_2, vec![target_prop_string.into(), source_type_string.into(), target_type_string.into()]);
                }
                return TERNARY_FALSE;
            }
        } else if source_prop_flags & ast::MODIFIER_FLAGS_PROTECTED != 0 {
            if report_errors {
                let target_prop_string = self.c.relater_symbol_identity_to_string(target_prop);
                let source_string = self.c.type_to_string_public(source);
                let target_string = self.c.type_to_string_public(target);
                self.report_error(
                    &*diagnostics::PROPERTY_0_IS_PROTECTED_IN_TYPE_1_BUT_PUBLIC_IN_TYPE_2,
                    vec![
                        target_prop_string.into(),
                        source_string.into(),
                        target_string.into(),
                    ],
                );
            }
            return TERNARY_FALSE;
        }
        if self.relation_is(self.c.semantic_state.strict_subtype_relation)
            && self.c.is_readonly_symbol_identity(source_prop)
            && !self.c.is_readonly_symbol_identity(target_prop)
        {
            return TERNARY_FALSE;
        }
        let related = self.is_property_symbol_type_related(
            source_prop,
            target_prop,
            source_property_type,
            report_errors,
            intersection_state,
        );
        if related == TERNARY_FALSE {
            if report_errors {
                let target_value_declaration = self
                    .c
                    .missing_name_symbol_identity_value_declaration(target_prop);
                let suppress_class_property_wrapper = self
                    .relation_is(self.c.semantic_state.comparable_relation)
                    && self.get_chain_message(0).is_some_and(|message| {
                        message_is(message, &diagnostics::TYPE_0_IS_NOT_COMPARABLE_TO_TYPE_1)
                    })
                    && target_value_declaration.is_some_and(|declaration| {
                        ast::is_property_declaration(
                            self.c.store_for_node(declaration),
                            declaration,
                        )
                    });
                if !suppress_class_property_wrapper {
                    let target_prop_string = self.c.relater_symbol_identity_to_string(target_prop);
                    self.report_error(
                        &*diagnostics::TYPES_OF_PROPERTY_0_ARE_INCOMPATIBLE,
                        vec![target_prop_string.into()],
                    );
                }
            }
            return TERNARY_FALSE;
        }
        let source_prop_flags = self.c.symbol_identity_flags(source_prop);
        let target_prop_flags = self.c.symbol_identity_flags(target_prop);
        if !skip_optional
            && source_prop_flags & ast::SYMBOL_FLAGS_OPTIONAL != 0
            && target_prop_flags & ast::SYMBOL_FLAGS_CLASS_MEMBER != 0
            && target_prop_flags & ast::SYMBOL_FLAGS_OPTIONAL == 0
        {
            if report_errors {
                let target_prop_string = self.c.relater_symbol_identity_to_string(target_prop);
                let source_string = self.c.type_to_string_public(source);
                let target_string = self.c.type_to_string_public(target);
                self.report_error(
                    &*diagnostics::PROPERTY_0_IS_OPTIONAL_IN_TYPE_1_BUT_REQUIRED_IN_TYPE_2,
                    vec![
                        target_prop_string.into(),
                        source_string.into(),
                        target_string.into(),
                    ],
                );
            }
            return TERNARY_FALSE;
        }
        related
    }

    fn is_property_symbol_type_related(
        &mut self,
        source_prop: SymbolIdentity,
        target_prop: SymbolIdentity,
        source_property_type: Option<TypeHandle>,
        report_errors: bool,
        intersection_state: IntersectionState,
    ) -> Ternary {
        let target_is_optional = self.c.strict_null_checks()
            && self.c.symbol_identity_check_flags(target_prop) & ast::CHECK_FLAGS_PARTIAL != 0;
        let target_type = self.c.get_non_missing_type_of_symbol_identity(target_prop);
        let effective_target =
            self.c
                .add_optionality_ex(target_type, false /*isProperty*/, target_is_optional);
        // source could resolve to `any` and that's not related to `unknown` target under strict subtype relation
        if self.c.type_flags(effective_target)
            & if self.relation_is(self.c.semantic_state.strict_subtype_relation) {
                TYPE_FLAGS_ANY
            } else {
                TYPE_FLAGS_ANY_OR_UNKNOWN
            }
            != 0
        {
            return TERNARY_TRUE;
        }
        let effective_source = source_property_type
            .unwrap_or_else(|| self.c.get_non_missing_type_of_symbol_identity(source_prop));
        self.is_related_to_ex(
            effective_source,
            effective_target,
            RECURSION_FLAGS_BOTH,
            report_errors,
            None, /*headMessage*/
            intersection_state,
        )
    }

    fn report_unmatched_property(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        unmatched_property: SymbolIdentity,
        require_optional_properties: bool,
    ) {
        // give specific error in case where private names have the same description
        let value_declaration = self
            .c
            .missing_name_symbol_identity_value_declaration(unmatched_property);
        let value_declaration_name = value_declaration.as_ref().and_then(|value_declaration| {
            self.c
                .store_for_node(*value_declaration)
                .name(*value_declaration)
        });
        if value_declaration.is_some()
            && value_declaration_name.is_some()
            && ast::is_private_identifier(
                self.c.store_for_node(value_declaration_name.unwrap()),
                value_declaration_name.unwrap(),
            )
            && self.c.type_symbol_identity(source).is_some_and(|symbol| {
                self.c.symbol_identity_flags(symbol) & ast::SYMBOL_FLAGS_CLASS != 0
            })
        {
            if let Some(source_symbol) = self.c.type_symbol_identity(source) {
                let value_declaration = value_declaration.as_ref().unwrap();
                let value_declaration_store = self.c.store_for_node(*value_declaration);
                let private_identifier_description =
                    value_declaration_store.text(value_declaration_name.unwrap());
                if let Some(symbol_table_key) = self
                    .c
                    .relater_get_symbol_name_for_private_identifier_from_identity(
                        source_symbol,
                        &private_identifier_description,
                    )
                {
                    if self
                        .c
                        .get_property_of_type(source, &symbol_table_key)
                        .is_some()
                    {
                        let source_string = self
                            .c
                            .type_symbol_identity(source)
                            .map(|symbol| self.c.symbol_identity_name(symbol).to_string())
                            .expect("source private identifier comparison must have a type symbol");
                        let target_string = self
                            .c
                            .type_symbol_identity(target)
                            .map(|symbol| self.c.symbol_identity_name(symbol).to_string())
                            .expect("target private identifier comparison must have a type symbol");
                        self.report_error(&*diagnostics::PROPERTY_0_IN_TYPE_1_REFERS_TO_A_DIFFERENT_MEMBER_THAT_CANNOT_BE_ACCESSED_FROM_WITHIN_TYPE_2, vec![private_identifier_description.into(), source_string.into(), target_string.into()]);
                        return;
                    }
                }
            }
        }
        let props = self.c.get_unmatched_properties(
            source,
            target,
            require_optional_properties,
            false, /*matchDiscriminantProperties*/
        );
        if props.len() == 1 {
            let (source_type, target_type) =
                self.c.get_type_names_for_error_display(source, target);
            let prop_name = self.c.relater_symbol_identity_to_string(unmatched_property);
            self.report_error(
                &*diagnostics::PROPERTY_0_IS_MISSING_IN_TYPE_1_BUT_REQUIRED_IN_TYPE_2,
                vec![
                    prop_name.clone().into(),
                    source_type.into(),
                    target_type.into(),
                ],
            );
            if let Some(unmatched_property_declaration) =
                self.c.first_symbol_identity_declaration(unmatched_property)
            {
                self.related_info.push(create_diagnostic_for_node_with_args(
                    self.c.store_for_node(unmatched_property_declaration),
                    unmatched_property_declaration,
                    &*diagnostics::X_0_IS_DECLARED_HERE,
                    prop_name,
                ));
            }
        } else if self.try_elaborate_array_like_errors(source, target, false /*reportErrors*/) {
            let (source_type, target_type) =
                self.c.get_type_names_for_error_display(source, target);
            if props.len() > 5 {
                let prop_names = props
                    .iter()
                    .take(4)
                    .map(|p| self.c.relater_symbol_identity_to_string(*p))
                    .collect::<Vec<_>>()
                    .join(", ");
                self.report_error(&*diagnostics::TYPE_0_IS_MISSING_THE_FOLLOWING_PROPERTIES_FROM_TYPE_1_COLON_2_AND_3_MORE, vec![source_type.into(), target_type.into(), prop_names.into(), (props.len() - 4).into()]);
            } else {
                let prop_names = props
                    .iter()
                    .map(|p| self.c.relater_symbol_identity_to_string(*p))
                    .collect::<Vec<_>>()
                    .join(", ");
                self.report_error(
                    &*diagnostics::TYPE_0_IS_MISSING_THE_FOLLOWING_PROPERTIES_FROM_TYPE_1_COLON_2,
                    vec![source_type.into(), target_type.into(), prop_names.into()],
                );
            }
        }
    }

    fn properties_identical_to(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        excluded_properties: collections::Set<String>,
    ) -> Ternary {
        if self.c.type_flags(source) & TYPE_FLAGS_OBJECT == 0
            || self.c.type_flags(target) & TYPE_FLAGS_OBJECT == 0
        {
            return TERNARY_FALSE;
        }
        let source_properties = self.c.relater_get_properties_of_object_type_list(source);
        let target_properties = self.c.relater_get_properties_of_object_type_list(target);
        if source_properties.excluded_len(self.c, &excluded_properties)
            != target_properties.excluded_len(self.c, &excluded_properties)
        {
            return TERNARY_FALSE;
        }
        let mut result = TERNARY_TRUE;
        for source_prop_index in 0..source_properties.len(self.c) {
            let source_prop = source_properties.get(self.c, source_prop_index);
            if property_identity_is_excluded(self.c, source_prop, &excluded_properties) {
                continue;
            }
            let source_prop_name = self.c.missing_name_symbol_identity_name(source_prop);
            let target_prop = self
                .c
                .relater_get_property_of_object_type_identity(target, &source_prop_name);
            if target_prop.is_none() {
                return TERNARY_FALSE;
            }
            let related =
                self.compare_properties_with_simple_related(source_prop, target_prop.unwrap());
            if related == TERNARY_FALSE {
                return TERNARY_FALSE;
            }
            result &= related;
        }
        result
    }

    fn signatures_related_to(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        kind: SignatureKind,
        report_errors: bool,
        intersection_state: IntersectionState,
    ) -> Ternary {
        if self.relation_is(self.c.semantic_state.identity_relation) {
            return self.signatures_identical_to(source, target, kind);
        }
        if source == self.c.semantic_state.semantic_handles().any_function_type {
            return TERNARY_TRUE;
        }
        if target == self.c.semantic_state.semantic_handles().any_function_type {
            return TERNARY_FALSE;
        }
        let source_signatures = self.c.relater_get_signatures_of_type_list(source, kind);
        let target_signatures = self.c.relater_get_signatures_of_type_list(target, kind);
        let source_signature_count = source_signatures.len(self.c);
        let target_signature_count = target_signatures.len(self.c);
        if kind == SIGNATURE_KIND_CONSTRUCT
            && !source_signatures.is_empty(self.c)
            && !target_signatures.is_empty(self.c)
        {
            let first_source_signature = source_signatures.get(self.c, 0);
            let first_target_signature = target_signatures.get(self.c, 0);
            let source_is_abstract = self.c.signature_record(first_source_signature).flags
                & SIGNATURE_FLAGS_ABSTRACT
                != 0;
            let target_is_abstract = self.c.signature_record(first_target_signature).flags
                & SIGNATURE_FLAGS_ABSTRACT
                != 0;
            if source_is_abstract && !target_is_abstract {
                // An abstract constructor type is not assignable to a non-abstract constructor type
                // as it would otherwise be possible to new an abstract class. Note that the assignability
                // check we perform for an extends clause excludes construct signatures from the target,
                // so this check never proceeds.
                if report_errors {
                    self.report_error(&diagnostics::CANNOT_ASSIGN_AN_ABSTRACT_CONSTRUCTOR_TYPE_TO_A_NON_ABSTRACT_CONSTRUCTOR_TYPE, vec![]);
                }
                return TERNARY_FALSE;
            }
            if !self.constructor_visibilities_are_compatible(
                first_source_signature,
                first_target_signature,
                report_errors,
            ) {
                return TERNARY_FALSE;
            }
        }
        let mut result = TERNARY_TRUE;
        let source_object_flags = self.c.object_flags(source);
        let target_object_flags = self.c.object_flags(target);
        if source_signature_count == target_signature_count
            && ((source_object_flags & OBJECT_FLAGS_INSTANTIATED != 0
                && target_object_flags & OBJECT_FLAGS_INSTANTIATED != 0
                && self.c.type_symbol_identity(source) == self.c.type_symbol_identity(target))
                || (source_object_flags & OBJECT_FLAGS_REFERENCE != 0
                    && target_object_flags & OBJECT_FLAGS_REFERENCE != 0
                    && self.c.type_target(source) == self.c.type_target(target)))
        {
            // We have instantiations of the same anonymous type (which typically will be the type of a
            // method). Simply do a pairwise comparison of the signatures in the two signature lists instead
            // of the much more expensive N * M comparison matrix we explore below. We erase type parameters
            // as they are known to always be the same.
            for signature_index in 0..target_signature_count {
                let related = self.signature_related_to(
                    source_signatures.get(self.c, signature_index),
                    target_signatures.get(self.c, signature_index),
                    true, /*erase*/
                    report_errors,
                    intersection_state,
                );
                if related == TERNARY_FALSE {
                    return TERNARY_FALSE;
                }
                result &= related;
            }
        } else if source_signature_count == 1 && target_signature_count == 1 {
            // For simple functions (functions with a single signature) we only erase type parameters for
            // the comparable relation. Otherwise, if the source signature is generic, we instantiate it
            // in the context of the target signature before checking the relationship. Ideally we'd do
            // this regardless of the number of signatures, but the potential costs are prohibitive due
            // to the quadratic nature of the logic below.
            let erase_generics = self.relation_is(self.c.semantic_state.comparable_relation);
            result = self.signature_related_to(
                source_signatures.get(self.c, 0),
                target_signatures.get(self.c, 0),
                erase_generics,
                report_errors,
                intersection_state,
            );
        } else {
            'outer: for target_signature_index in 0..target_signature_count {
                let t = target_signatures.get(self.c, target_signature_index);
                let save_error_state = self.get_error_state();
                // Only elaborate errors from the first failure.
                let mut should_elaborate_errors = report_errors;
                for source_signature_index in 0..source_signature_count {
                    let s = source_signatures.get(self.c, source_signature_index);
                    let related = self.signature_related_to(
                        s,
                        t,
                        true, /*erase*/
                        should_elaborate_errors,
                        intersection_state,
                    );
                    if related != TERNARY_FALSE {
                        result &= related;
                        self.restore_error_state(&save_error_state);
                        continue 'outer;
                    }
                    should_elaborate_errors = false;
                }
                if should_elaborate_errors {
                    let source_string = self.c.type_to_string_public(source);
                    let signature_string = self.c.signature_to_string_ex_public(
                        t,
                        None,
                        TYPE_FORMAT_FLAGS_NONE,
                        Some(kind),
                        None,
                    );
                    self.report_error(
                        &diagnostics::TYPE_0_PROVIDES_NO_MATCH_FOR_THE_SIGNATURE_1,
                        vec![source_string.into(), signature_string.into()],
                    );
                }
                return TERNARY_FALSE;
            }
        }
        result
    }

    fn signatures_identical_to(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        kind: SignatureKind,
    ) -> Ternary {
        let source_signatures = self.c.relater_get_signatures_of_type_list(source, kind);
        let target_signatures = self.c.relater_get_signatures_of_type_list(target, kind);
        let source_signature_count = source_signatures.len(self.c);
        let target_signature_count = target_signatures.len(self.c);
        if source_signature_count != target_signature_count {
            return TERNARY_FALSE;
        }
        let mut result = TERNARY_TRUE;
        for i in 0..source_signature_count {
            let related = self.compare_signatures_identical(
                source_signatures.get(self.c, i),
                target_signatures.get(self.c, i),
                false, /*partialMatch*/
                false, /*ignoreThisTypes*/
                false, /*ignoreReturnTypes*/
            );
            if related == 0 {
                return TERNARY_FALSE;
            }
            result &= related;
        }
        result
    }

    fn compare_signatures_identical(
        &mut self,
        mut source: SignatureHandle,
        target: SignatureHandle,
        partial_match: bool,
        ignore_this_types: bool,
        ignore_return_types: bool,
    ) -> Ternary {
        if source == target {
            return TERNARY_TRUE;
        }
        if !self.c.is_matching_signature(source, target, partial_match) {
            return TERNARY_FALSE;
        }
        if self.c.signature_record(source).type_parameters.len()
            != self.c.signature_record(target).type_parameters.len()
        {
            return TERNARY_FALSE;
        }
        if !self.c.signature_record(target).type_parameters.is_empty() {
            let source_type_parameters = self.c.signature_record(source).type_parameters.clone();
            let target_type_parameters = self.c.signature_record(target).type_parameters.clone();
            let mapper = self.c.new_type_mapper_handle(
                source_type_parameters.clone(),
                target_type_parameters.clone(),
            );
            for i in 0..target_type_parameters.len() {
                let s = source_type_parameters[i];
                let t = target_type_parameters[i];
                if s == t {
                    continue;
                }
                let source_constraint = self.c.get_constraint_or_unknown_from_type_parameter(s);
                let source_constraint = self
                    .c
                    .instantiate_type_with_mapper_handle(Some(source_constraint), Some(mapper))
                    .unwrap();
                let target_constraint = self.c.get_constraint_or_unknown_from_type_parameter(t);
                if self.is_related_to_simple(source_constraint, target_constraint) == TERNARY_FALSE
                {
                    return TERNARY_FALSE;
                }
                let source_default = self.c.get_default_or_unknown_from_type_parameter(s);
                let source_default = self
                    .c
                    .instantiate_type_with_mapper_handle(Some(source_default), Some(mapper))
                    .unwrap();
                let target_default = self.c.get_default_or_unknown_from_type_parameter(t);
                if self.is_related_to_simple(source_default, target_default) == TERNARY_FALSE {
                    return TERNARY_FALSE;
                }
            }
            source = self.c.instantiate_signature_ex_with_mapper_handle(
                source, mapper, true, /*eraseTypeParameters*/
            );
        }
        let mut result = TERNARY_TRUE;
        if !ignore_this_types {
            let source_this_type = self.c.get_this_type_of_signature(source);
            if let Some(source_this_type) = source_this_type {
                let target_this_type = self.c.get_this_type_of_signature(target);
                if let Some(target_this_type) = target_this_type {
                    let related = self.is_related_to_simple(source_this_type, target_this_type);
                    if related == TERNARY_FALSE {
                        return TERNARY_FALSE;
                    }
                    result &= related;
                }
            }
        }
        for i in 0..self.c.get_parameter_count(target) {
            let s = self.c.get_type_at_position(source, i);
            let t = self.c.get_type_at_position(target, i);
            let related = self.is_related_to_simple(t, s);
            if related == TERNARY_FALSE {
                return TERNARY_FALSE;
            }
            result &= related;
        }
        if !ignore_return_types {
            let source_type_predicate = self.c.get_type_predicate_of_signature(source);
            let target_type_predicate = self.c.get_type_predicate_of_signature(target);
            if source_type_predicate.is_some() || target_type_predicate.is_some() {
                result &= self.compare_type_predicates_identical(
                    source_type_predicate,
                    target_type_predicate,
                );
            } else {
                let source_return_type = self.c.get_return_type_of_signature(source);
                let target_return_type = self.c.get_return_type_of_signature(target);
                result &= self.is_related_to_simple(source_return_type, target_return_type);
            }
        }
        result
    }

    fn compare_type_predicates_identical(
        &mut self,
        source: Option<TypePredicateHandle>,
        target: Option<TypePredicateHandle>,
    ) -> Ternary {
        match (source, target) {
            (Some(source), Some(target)) if self.c.type_predicate_kinds_match(source, target) => {
                let source_t = self.c.type_predicate_record(source).t;
                let target_t = self.c.type_predicate_record(target).t;
                if source_t == target_t {
                    TERNARY_TRUE
                } else if source_t.is_some() && target_t.is_some() {
                    self.is_related_to_simple(source_t.unwrap(), target_t.unwrap())
                } else {
                    TERNARY_FALSE
                }
            }
            _ => TERNARY_FALSE,
        }
    }

    fn index_signatures_related_to(
        &mut self,
        source: TypeHandle,
        target: TypeHandle,
        source_is_primitive: bool,
        report_errors: bool,
        intersection_state: IntersectionState,
    ) -> Ternary {
        if self.relation_is(self.c.semantic_state.identity_relation) {
            return self.index_signatures_identical_to(source, target);
        }
        let index_infos = self.c.relater_get_index_infos_of_type_list(target);
        let target_has_string_index = (0..index_infos.len(self.c)).any(|index| {
            let info = index_infos.get(self.c, index);
            self.c.index_info_record(info).key_type.unwrap()
                == self.c.semantic_state.semantic_handles().string_type
        });
        let mut result = TERNARY_TRUE;
        for target_info_index in 0..index_infos.len(self.c) {
            let target_info = index_infos.get(self.c, target_info_index);
            let target_value_type = self.c.index_info_record(target_info).value_type.unwrap();
            let related = if !self.relation_is(self.c.semantic_state.strict_subtype_relation)
                && !source_is_primitive
                && target_has_string_index
                && self.c.type_flags(target_value_type) & TYPE_FLAGS_ANY != 0
            {
                TERNARY_TRUE
            } else if self.c.is_generic_mapped_type(source) && target_has_string_index {
                let template_type = self.c.get_template_type_from_mapped_type(source);
                self.is_related_to(
                    template_type,
                    target_value_type,
                    RECURSION_FLAGS_BOTH,
                    report_errors,
                )
            } else {
                self.type_related_to_index_info(
                    source,
                    target_info,
                    report_errors,
                    intersection_state,
                )
            };
            if related == TERNARY_FALSE {
                return TERNARY_FALSE;
            }
            result &= related;
        }
        result
    }

    fn type_related_to_index_info(
        &mut self,
        source: TypeHandle,
        target_info: IndexInfoHandle,
        report_errors: bool,
        intersection_state: IntersectionState,
    ) -> Ternary {
        let source_info = self.c.get_applicable_index_info(
            source,
            self.c.index_info_record(target_info).key_type.unwrap(),
        );
        if let Some(source_info) = source_info {
            return self.index_info_related_to(
                source_info,
                target_info,
                report_errors,
                intersection_state,
            );
        }
        // Intersection constituents are never considered to have an inferred index signature. Also, in the strict subtype relation,
        // only fresh object literals are considered to have inferred index signatures. This ensures { [x: string]: xxx } <: {} but
        // not vice-versa. Without this rule, those types would be mutual strict subtypes.
        if intersection_state & INTERSECTION_STATE_SOURCE == 0
            && (!self.relation_is(self.c.semantic_state.strict_subtype_relation)
                || self.c.object_flags(source) & OBJECT_FLAGS_FRESH_LITERAL != 0)
            && self.c.is_object_type_with_inferable_index(source)
        {
            return self.members_related_to_index_info(
                source,
                target_info,
                report_errors,
                intersection_state,
            );
        }
        if report_errors {
            let target_key_string = self
                .c
                .type_to_string_public(self.c.index_info_record(target_info).key_type.unwrap());
            let source_string = self.c.type_to_string_public(source);
            self.report_error(
                &diagnostics::INDEX_SIGNATURE_FOR_TYPE_0_IS_MISSING_IN_TYPE_1,
                vec![target_key_string.into(), source_string.into()],
            );
        }
        TERNARY_FALSE
    }

    fn members_related_to_index_info(
        &mut self,
        source: TypeHandle,
        target_info: IndexInfoHandle,
        report_errors: bool,
        intersection_state: IntersectionState,
    ) -> Ternary {
        let mut result = TERNARY_TRUE;
        let key_type = self.c.index_info_record(target_info).key_type.unwrap();
        let target_value_type = self.c.index_info_record(target_info).value_type.unwrap();
        let props = if self.c.type_flags(source) & TYPE_FLAGS_INTERSECTION != 0 {
            RelationPropertyList::Owned(self.c.get_properties_of_union_or_intersection_type(source))
        } else {
            self.c.relater_get_properties_of_object_type_list(source)
        };
        for prop_index in 0..props.len(self.c) {
            let prop_identity = props.get(self.c, prop_index);
            if is_ignored_jsx_property(self.c, source, prop_identity) {
                continue;
            }
            let prop_key_type = self.c.relater_get_literal_type_from_property(
                prop_identity,
                TYPE_FLAGS_STRING_OR_NUMBER_LITERAL_OR_UNIQUE,
                false,
            );
            if self.c.is_applicable_index_type(prop_key_type, key_type) {
                let prop_type = self.c.relater_get_non_missing_type_of_symbol(prop_identity);
                let prop_flags = self.c.missing_name_symbol_identity_flags(prop_identity);
                let t = if self.c.exact_optional_property_types()
                    || self.c.type_flags(prop_type) & TYPE_FLAGS_UNDEFINED != 0
                    || key_type == self.c.semantic_state.semantic_handles().number_type
                    || prop_flags & ast::SYMBOL_FLAGS_OPTIONAL == 0
                {
                    prop_type
                } else {
                    self.c
                        .get_type_with_facts(prop_type, TYPE_FACTS_NE_UNDEFINED)
                };
                let related = self.is_related_to_ex(
                    t,
                    target_value_type,
                    RECURSION_FLAGS_BOTH,
                    report_errors,
                    None, /*headMessage*/
                    intersection_state,
                );
                if related == TERNARY_FALSE {
                    if report_errors {
                        let prop_string = self.c.relater_symbol_identity_to_string(prop_identity);
                        self.report_error(
                            &diagnostics::PROPERTY_0_IS_INCOMPATIBLE_WITH_INDEX_SIGNATURE,
                            vec![prop_string.into()],
                        );
                    }
                    return TERNARY_FALSE;
                }
                result &= related;
            }
        }
        let source_infos = self.c.relater_get_index_infos_of_type_list(source);
        for info_index in 0..source_infos.len(self.c) {
            let info = source_infos.get(self.c, info_index);
            let source_key_type = self.c.index_info_record(info).key_type.unwrap();
            if self.c.is_applicable_index_type(source_key_type, key_type) {
                let related = self.index_info_related_to(
                    info,
                    target_info,
                    report_errors,
                    intersection_state,
                );
                if related == 0 {
                    return TERNARY_FALSE;
                }
                result &= related;
            }
        }
        result
    }

    fn index_info_related_to(
        &mut self,
        source_info: IndexInfoHandle,
        target_info: IndexInfoHandle,
        report_errors: bool,
        intersection_state: IntersectionState,
    ) -> Ternary {
        let source_info_record = self.c.index_info_record(source_info).clone();
        let target_info_record = self.c.index_info_record(target_info).clone();
        let related = self.is_related_to_ex(
            source_info_record.value_type.unwrap(),
            target_info_record.value_type.unwrap(),
            RECURSION_FLAGS_BOTH,
            report_errors,
            None, /*headMessage*/
            intersection_state,
        );
        if related == TERNARY_FALSE && report_errors {
            if source_info_record.key_type == target_info_record.key_type {
                let source_key_string = self
                    .c
                    .type_to_string_public(source_info_record.key_type.unwrap());
                self.report_error(
                    &diagnostics::X_0_INDEX_SIGNATURES_ARE_INCOMPATIBLE,
                    vec![source_key_string.into()],
                );
            } else {
                let source_key_string = self
                    .c
                    .type_to_string_public(source_info_record.key_type.unwrap());
                let target_key_string = self
                    .c
                    .type_to_string_public(target_info_record.key_type.unwrap());
                self.report_error(
                    &diagnostics::X_0_AND_1_INDEX_SIGNATURES_ARE_INCOMPATIBLE,
                    vec![source_key_string.into(), target_key_string.into()],
                );
            }
        }
        related
    }

    fn index_signatures_identical_to(&mut self, source: TypeHandle, target: TypeHandle) -> Ternary {
        let source_infos = self.c.relater_get_index_infos_of_type_list(source);
        let target_infos = self.c.relater_get_index_infos_of_type_list(target);
        if source_infos.len(self.c) != target_infos.len(self.c) {
            return TERNARY_FALSE;
        }
        for target_info_index in 0..target_infos.len(self.c) {
            let target_info = target_infos.get(self.c, target_info_index);
            let target_key_type = self.c.index_info_record(target_info).key_type.unwrap();
            let target_value_type = self.c.index_info_record(target_info).value_type.unwrap();
            let target_is_readonly = self.c.index_info_record(target_info).is_readonly;
            let source_info = source_infos.find_by_key_type(self.c, target_key_type);
            if !(source_info.is_some()
                && self.is_related_to(
                    self.c
                        .index_info_record(source_info.unwrap())
                        .value_type
                        .unwrap(),
                    target_value_type,
                    RECURSION_FLAGS_BOTH,
                    false,
                ) != TERNARY_FALSE
                && self.c.index_info_record(source_info.unwrap()).is_readonly == target_is_readonly)
            {
                return TERNARY_FALSE;
            }
        }
        TERNARY_TRUE
    }

    fn report_error_results(
        &mut self,
        original_source: TypeHandle,
        original_target: TypeHandle,
        mut source: TypeHandle,
        mut target: TypeHandle,
        head_message: Option<&'static diagnostics::Message>,
    ) {
        let source_has_base = self
            .c
            .get_single_base_for_non_augmenting_subtype(original_source)
            .is_some();
        let target_has_base = self
            .c
            .get_single_base_for_non_augmenting_subtype(original_target)
            .is_some();
        if self.c.type_alias_record(original_source).is_some() || source_has_base {
            source = original_source;
        }
        if self.c.type_alias_record(original_target).is_some() || target_has_base {
            target = original_target;
        }
        if self.c.type_flags(source) & TYPE_FLAGS_OBJECT != 0
            && self.c.type_flags(target) & TYPE_FLAGS_OBJECT != 0
        {
            self.try_elaborate_array_like_errors(source, target, true /*reportErrors*/);
        }
        if self.c.type_flags(source) & TYPE_FLAGS_OBJECT != 0
            && self.c.type_flags(target) & TYPE_FLAGS_PRIMITIVE != 0
        {
            self.try_elaborate_errors_for_primitives_and_objects(source, target);
        } else if self.c.type_symbol_identity(source).is_some()
            && self.c.type_flags(source) & TYPE_FLAGS_OBJECT != 0
            && self.c.semantic_state.semantic_handles().global_object_type == source
        {
            self.report_error(&diagnostics::THE_OBJECT_TYPE_IS_ASSIGNABLE_TO_VERY_FEW_OTHER_TYPES_DID_YOU_MEAN_TO_USE_THE_ANY_TYPE_INSTEAD, vec![]);
        } else if self.c.object_flags(source) & OBJECT_FLAGS_JSX_ATTRIBUTES != 0
            && self.c.type_flags(target) & TYPE_FLAGS_INTERSECTION != 0
        {
            let Some(error_node) = self.error_node else {
                self.report_relation_error(head_message, source, target);
                return;
            };
            let target_types = self.c.type_types_slice(target).to_vec();
            let intrinsic_attributes = self
                .c
                .get_jsx_type(JSX_NAMES_INTRINSIC_ATTRIBUTES, error_node);
            let intrinsic_class_attributes = self
                .c
                .get_jsx_type(JSX_NAMES_INTRINSIC_CLASS_ATTRIBUTES, error_node);
            if !self.c.is_error_type(intrinsic_attributes)
                && !self.c.is_error_type(intrinsic_class_attributes)
                && (target_types.contains(&intrinsic_attributes)
                    || target_types.contains(&intrinsic_class_attributes))
            {
                return;
            }
        } else if self.c.type_flags(original_target) & TYPE_FLAGS_INTERSECTION != 0
            && self.c.object_flags(original_target) & OBJECT_FLAGS_IS_NEVER_INTERSECTION != 0
        {
            let mut message: &'static diagnostics::Message = &*diagnostics::THE_INTERSECTION_0_WAS_REDUCED_TO_NEVER_BECAUSE_PROPERTY_1_HAS_CONFLICTING_TYPES_IN_SOME_CONSTITUENTS;
            let mut prop = self
                .c
                .get_properties_of_union_or_intersection_type(original_target)
                .into_iter()
                .find(|p| {
                    let flags = self.c.symbol_identity_flags(*p);
                    let check_flags = self.c.symbol_identity_check_flags(*p);
                    !flags.intersects(ast::SYMBOL_FLAGS_OPTIONAL)
                        && check_flags
                            & (ast::CHECK_FLAGS_NON_UNIFORM_AND_LITERAL
                                | ast::CHECK_FLAGS_HAS_NEVER_TYPE)
                            == ast::CHECK_FLAGS_NON_UNIFORM_AND_LITERAL
                        && {
                            let prop_type = self.c.relater_get_type_of_symbol(*p);
                            self.c.type_flags(prop_type) & TYPE_FLAGS_NEVER != 0
                        }
                });
            if prop.is_none() {
                message = &*diagnostics::THE_INTERSECTION_0_WAS_REDUCED_TO_NEVER_BECAUSE_PROPERTY_1_EXISTS_IN_MULTIPLE_CONSTITUENTS_AND_IS_PRIVATE_IN_SOME;
                prop = self
                    .c
                    .get_properties_of_union_or_intersection_type(original_target)
                    .into_iter()
                    .find(|p| {
                        self.c
                            .missing_name_symbol_identity_value_declaration(*p)
                            .is_none()
                            && self
                                .c
                                .symbol_identity_check_flags(*p)
                                .intersects(ast::CHECK_FLAGS_CONTAINS_PRIVATE)
                    });
            }
            if let Some(prop) = prop {
                let target_string = self.c.type_to_string_ex(
                    original_target,
                    None, /*enclosingDeclaration*/
                    TYPE_FORMAT_FLAGS_NO_TYPE_REDUCTION,
                    None,
                );
                let prop_string = self.c.missing_name_symbol_identity_name(prop);
                self.report_error(message, vec![target_string.into(), prop_string.into()]);
            }
        }
        self.report_relation_error(head_message, source, target);
        if self.c.type_flags(source) & TYPE_FLAGS_TYPE_PARAMETER != 0
            && self
                .c
                .type_symbol_identity(source)
                .is_some_and(|symbol| !self.c.symbol_identity_declarations_are_empty(symbol))
            && self.c.get_constraint_of_type(source).is_none()
        {
            let synthetic_param = self.c.clone_type_parameter(source);
            let mapper = self
                .c
                .new_simple_type_mapper_handle(source, synthetic_param);
            let constraint = self
                .c
                .instantiate_type_with_mapper_handle(Some(target), Some(mapper))
                .unwrap();
            self.c
                .semantic_state
                .type_record_mut(synthetic_param)
                .data
                .as_type_parameter_mut()
                .constraint = Some(constraint);
            if self.c.has_non_circular_base_constraint(synthetic_param) {
                let target_constraint_string = self.c.type_to_string_public(target);
                let source_declaration = self
                    .c
                    .first_symbol_identity_declaration(self.c.type_symbol_identity(source).unwrap())
                    .unwrap();
                self.related_info.push(new_diagnostic_for_node(
                    self.c.store_for_node(source_declaration),
                    Some(source_declaration),
                    &diagnostics::THIS_TYPE_PARAMETER_MIGHT_NEED_AN_EXTENDS_0_CONSTRAINT,
                    Vec::<DiagnosticArg>::from([target_constraint_string.into()]),
                ));
            }
        }
    }

    fn report_relation_error(
        &mut self,
        mut message: Option<&'static diagnostics::Message>,
        source: TypeHandle,
        target: TypeHandle,
    ) {
        let (source_type, target_type) = self.c.get_type_names_for_error_display(source, target);
        let mut generalized_source = source;
        let mut generalized_source_type = source_type.clone();
        // Don't generalize on 'never' - we really want the original type
        // to be displayed for use-cases like 'assertNever'.
        if self.c.type_flags(target) & TYPE_FLAGS_NEVER == 0
            && self.c.is_literal_type(source)
            && !self.c.type_could_have_top_level_singleton_types(target)
        {
            generalized_source = self.c.get_base_type_of_literal_type(source);
            generalized_source_type = self.c.get_type_name_for_error_display(generalized_source);
        }
        let target_flags = if self.c.type_flags(target) & TYPE_FLAGS_INDEXED_ACCESS != 0
            && self.c.type_flags(source) & TYPE_FLAGS_INDEXED_ACCESS == 0
        {
            let object_type = self
                .c
                .type_record(target)
                .as_indexed_access_type()
                .object_type
                .unwrap();
            self.c.type_flags(object_type)
        } else {
            self.c.type_flags(target)
        };
        if target_flags & TYPE_FLAGS_TYPE_PARAMETER != 0
            && target
                != self
                    .c
                    .semantic_state
                    .semantic_handles()
                    .marker_super_type_for_check
            && target
                != self
                    .c
                    .semantic_state
                    .semantic_handles()
                    .marker_sub_type_for_check
        {
            let constraint = self.c.get_base_constraint_of_type(target);
            if !constraint.is_some_and(|constraint| {
                if self.c.is_type_assignable_to(generalized_source, constraint) {
                    let constraint_string = self.c.type_to_string_public(constraint);
                    self.report_error(&diagnostics::X_0_IS_ASSIGNABLE_TO_THE_CONSTRAINT_OF_TYPE_1_BUT_1_COULD_BE_INSTANTIATED_WITH_A_DIFFERENT_SUBTYPE_OF_CONSTRAINT_2, vec![generalized_source_type.clone().into(), target_type.clone().into(), constraint_string.into()]);
                    true
                } else if self.c.is_type_assignable_to(source, constraint) {
                    let constraint_string = self.c.type_to_string_public(constraint);
                    self.report_error(&diagnostics::X_0_IS_ASSIGNABLE_TO_THE_CONSTRAINT_OF_TYPE_1_BUT_1_COULD_BE_INSTANTIATED_WITH_A_DIFFERENT_SUBTYPE_OF_CONSTRAINT_2, vec![source_type.clone().into(), target_type.clone().into(), constraint_string.into()]);
                    true
                } else {
                    false
                }
            }) {
                self.error_chain = None; // Only report this error once
                self.report_error(
                    &diagnostics::X_0_COULD_BE_INSTANTIATED_WITH_AN_ARBITRARY_TYPE_WHICH_COULD_BE_UNRELATED_TO_1,
                    vec![target_type.clone().into(), generalized_source_type.clone().into()],
                );
            }
        }
        if message.is_none() {
            if self.relation_is(self.c.semantic_state.comparable_relation) {
                message = Some(&diagnostics::TYPE_0_IS_NOT_COMPARABLE_TO_TYPE_1);
            } else if source_type == target_type {
                message = Some(&diagnostics::TYPE_0_IS_NOT_ASSIGNABLE_TO_TYPE_1_TWO_DIFFERENT_TYPES_WITH_THIS_NAME_EXIST_BUT_THEY_ARE_UNRELATED);
            } else if self.c.exact_optional_property_types()
                && !self
                    .c
                    .get_exact_optional_unassignable_properties(source, target)
                    .is_empty()
            {
                message = Some(&diagnostics::TYPE_0_IS_NOT_ASSIGNABLE_TO_TYPE_1_WITH_EXACT_OPTIONAL_PROPERTY_TYPES_COLON_TRUE_CONSIDER_ADDING_UNDEFINED_TO_THE_TYPES_OF_THE_TARGET_S_PROPERTIES);
            } else {
                if self.c.type_flags(source) & TYPE_FLAGS_STRING_LITERAL != 0
                    && self.c.type_flags(target) & TYPE_FLAGS_UNION != 0
                {
                    if let Some(suggested_type) = self
                        .c
                        .get_suggested_type_for_nonexistent_string_literal_type(source, target)
                    {
                        let suggested_type_string = self.c.type_to_string_public(suggested_type);
                        self.report_error(
                            &diagnostics::TYPE_0_IS_NOT_ASSIGNABLE_TO_TYPE_1_DID_YOU_MEAN_2,
                            vec![
                                generalized_source_type.clone().into(),
                                target_type.clone().into(),
                                suggested_type_string.into(),
                            ],
                        );
                        return;
                    }
                }
                message = Some(&diagnostics::TYPE_0_IS_NOT_ASSIGNABLE_TO_TYPE_1);
            }
        }
        match self.get_chain_message(0) {
            // Suppress if next message is an excess property error
            Some(m)
                if message_is(
                    m,
                    &diagnostics::OBJECT_LITERAL_MAY_ONLY_SPECIFY_KNOWN_PROPERTIES_AND_0_DOES_NOT_EXIST_IN_TYPE_1,
                ) || message_is(
                    m,
                    &diagnostics::OBJECT_LITERAL_MAY_ONLY_SPECIFY_KNOWN_PROPERTIES_BUT_0_DOES_NOT_EXIST_IN_TYPE_1_DID_YOU_MEAN_TO_WRITE_2,
                ) =>
            {
                return;
            }
            // Suppress if next message is an excessive complexity/stack depth message for source and target or a readonly
            // vs. mutable error for source and target
            Some(m) if (message_is(m, &diagnostics::EXCESSIVE_COMPLEXITY_COMPARING_TYPES_0_AND_1) || message_is(m, &diagnostics::EXCESSIVE_STACK_DEPTH_COMPARING_TYPES_0_AND_1) || message_is(m, &diagnostics::THE_TYPE_0_IS_READONLY_AND_CANNOT_BE_ASSIGNED_TO_THE_MUTABLE_TYPE_1)) && self.chain_args_match(vec![Some(generalized_source_type.clone().into()), Some(target_type.clone().into())]) => return,
            // Suppress if next message is a missing property message for source and target and we're not
            // reporting on conversion or interface implementation
            Some(m)
                if message_is(
                        m,
                        &diagnostics::PROPERTY_0_IS_MISSING_IN_TYPE_1_BUT_REQUIRED_IN_TYPE_2,
                    )
                    && !is_conversion_or_interface_implementation_message(message.unwrap())
                    && self.chain_args_match(vec![
                        None,
                        Some(generalized_source_type.clone().into()),
                        Some(target_type.clone().into()),
                    ]) =>
            {
                return;
            }
            // Suppress if next message is a missing property message for source and target and we're not
            // reporting on conversion or interface implementation
            Some(m)
                if (message_is(
                        m,
                        &diagnostics::TYPE_0_IS_MISSING_THE_FOLLOWING_PROPERTIES_FROM_TYPE_1_COLON_2_AND_3_MORE,
                    ) || message_is(
                        m,
                        &diagnostics::TYPE_0_IS_MISSING_THE_FOLLOWING_PROPERTIES_FROM_TYPE_1_COLON_2,
                    ))
                    && !is_conversion_or_interface_implementation_message(message.unwrap())
                    && self.chain_args_match(vec![
                        Some(generalized_source_type.clone().into()),
                        Some(target_type.clone().into()),
                    ]) =>
            {
                return;
            }
            _ => {}
        }
        self.report_error(
            message.unwrap(),
            vec![generalized_source_type.into(), target_type.into()],
        );
    }
}
