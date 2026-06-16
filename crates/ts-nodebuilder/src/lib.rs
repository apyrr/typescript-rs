#![forbid(unsafe_code)]
// Exports interfaces and types defining the node builder - concrete implementations are on top of the checker, but these types and interfaces are used by the emit resolver in the printer

use ts_ast as ast;

pub type SymbolIdentity = ast::SymbolIdentity;
pub type Node = ast::Node;
pub type SourceFile = ast::SourceFile;
pub type SymbolFlags = ast::SymbolFlags;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolAccessibility {
    CannotBeNamed,
    NotAccessible,
}

// TODO: previously all symboltracker methods were optional, but now they're required.
pub trait SymbolTracker {
    fn track_symbol(
        &mut self,
        symbol: SymbolIdentity,
        symbol_flags: SymbolFlags,
        enclosing_declaration: Option<Node>,
        meaning: SymbolFlags,
    ) -> bool;
    fn report_inaccessible_this_error(&mut self);
    fn report_private_in_base_of_class_expression(&mut self, property_name: &str);
    fn report_inaccessible_unique_symbol_error(&mut self);
    fn report_cyclic_structure_error(&mut self);
    fn report_likely_unsafe_import_required_error(&mut self, specifier: &str, symbol_name: &str);
    fn report_truncation_error(&mut self);
    fn report_nonlocal_augmentation(
        &mut self,
        containing_file: &SourceFile,
        parent_symbol: SymbolIdentity,
        augmenting_symbol: SymbolIdentity,
    );
    fn report_non_serializable_property(&mut self, property_name: &str);
    fn mark_aliases_visible(&mut self, _aliases: &[Node]) {}
    fn report_symbol_accessibility_error(
        &mut self,
        _accessibility: SymbolAccessibility,
        _error_symbol_name: &str,
        _error_module_name: &str,
        _error_node: Option<Node>,
    ) -> bool {
        false
    }

    fn report_inference_fallback(&mut self, node: Node);
    fn push_error_fallback_node(&mut self, node: Node);
    fn pop_error_fallback_node(&mut self);
}

// NOTE: If modifying this enum, must modify `TypeFormatFlags` too!
pub type Flags = u32;

pub const FLAGS_NONE: Flags = 0;
// Options
pub const FLAGS_NO_TRUNCATION: Flags = 1 << 0;
pub const FLAGS_WRITE_ARRAY_AS_GENERIC_TYPE: Flags = 1 << 1;
pub const FLAGS_GENERATE_NAMES_FOR_SHADOWED_TYPE_PARAMS: Flags = 1 << 2;
pub const FLAGS_USE_STRUCTURAL_FALLBACK: Flags = 1 << 3;
pub const FLAGS_FORBID_INDEXED_ACCESS_SYMBOL_REFERENCES: Flags = 1 << 4;
pub const FLAGS_WRITE_TYPE_ARGUMENTS_OF_SIGNATURE: Flags = 1 << 5;
pub const FLAGS_USE_FULLY_QUALIFIED_TYPE: Flags = 1 << 6;
pub const FLAGS_USE_ONLY_EXTERNAL_ALIASING: Flags = 1 << 7;
pub const FLAGS_SUPPRESS_ANY_RETURN_TYPE: Flags = 1 << 8;
pub const FLAGS_WRITE_TYPE_PARAMETERS_IN_QUALIFIED_NAME: Flags = 1 << 9;
pub const FLAGS_MULTILINE_OBJECT_LITERALS: Flags = 1 << 10;
pub const FLAGS_WRITE_CLASS_EXPRESSION_AS_TYPE_LITERAL: Flags = 1 << 11;
pub const FLAGS_USE_TYPE_OF_FUNCTION: Flags = 1 << 12;
pub const FLAGS_OMIT_PARAMETER_MODIFIERS: Flags = 1 << 13;
pub const FLAGS_USE_ALIAS_DEFINED_OUTSIDE_CURRENT_SCOPE: Flags = 1 << 14;
pub const FLAGS_USE_SINGLE_QUOTES_FOR_STRING_LITERAL_TYPE: Flags = 1 << 28;
pub const FLAGS_NO_TYPE_REDUCTION: Flags = 1 << 29;
pub const FLAGS_USE_INSTANTIATION_EXPRESSIONS: Flags = 1 << 30;
pub const FLAGS_OMIT_THIS_PARAMETER: Flags = 1 << 25;
pub const FLAGS_WRITE_CALL_STYLE_SIGNATURE: Flags = 1 << 27;
// Error handling
pub const FLAGS_ALLOW_THIS_IN_OBJECT_LITERAL: Flags = 1 << 15;
pub const FLAGS_ALLOW_QUALIFIED_NAME_IN_PLACE_OF_IDENTIFIER: Flags = 1 << 16;
pub const FLAGS_ALLOW_ANONYMOUS_IDENTIFIER: Flags = 1 << 17;
pub const FLAGS_ALLOW_EMPTY_UNION_OR_INTERSECTION: Flags = 1 << 18;
pub const FLAGS_ALLOW_EMPTY_TUPLE: Flags = 1 << 19;
pub const FLAGS_ALLOW_UNIQUE_ES_SYMBOL_TYPE: Flags = 1 << 20;
pub const FLAGS_ALLOW_EMPTY_INDEX_INFO_TYPE: Flags = 1 << 21;
// Errors (cont.)
pub const FLAGS_ALLOW_NODE_MODULES_RELATIVE_PATHS: Flags = 1 << 26;
pub const FLAGS_IGNORE_ERRORS: Flags = FLAGS_ALLOW_THIS_IN_OBJECT_LITERAL
    | FLAGS_ALLOW_QUALIFIED_NAME_IN_PLACE_OF_IDENTIFIER
    | FLAGS_ALLOW_ANONYMOUS_IDENTIFIER
    | FLAGS_ALLOW_EMPTY_UNION_OR_INTERSECTION
    | FLAGS_ALLOW_EMPTY_TUPLE
    | FLAGS_ALLOW_EMPTY_INDEX_INFO_TYPE
    | FLAGS_ALLOW_NODE_MODULES_RELATIVE_PATHS;
// State
pub const FLAGS_IN_OBJECT_TYPE_LITERAL: Flags = 1 << 22;
pub const FLAGS_IN_TYPE_ALIAS: Flags = 1 << 23;
pub const FLAGS_IN_INITIAL_ENTITY_NAME: Flags = 1 << 24;

/* @internal */

pub type InternalFlags = i32;

pub const INTERNAL_FLAGS_NONE: InternalFlags = 0;
pub const INTERNAL_FLAGS_WRITE_COMPUTED_PROPS: InternalFlags = 1 << 0;
pub const INTERNAL_FLAGS_NO_SYNTACTIC_PRINTER: InternalFlags = 1 << 1;
pub const INTERNAL_FLAGS_DO_NOT_INCLUDE_SYMBOL_CHAIN: InternalFlags = 1 << 2;
pub const INTERNAL_FLAGS_ALLOW_UNRESOLVED_NAMES: InternalFlags = 1 << 3;
pub const INTERNAL_FLAGS_SIGNATURE_TO_STRING: InternalFlags = 1 << 4;
