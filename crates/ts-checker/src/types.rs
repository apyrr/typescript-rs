use std::fmt;

use ts_ast as ast;
use ts_core as core;

//go:generate go tool golang.org/x/tools/cmd/stringer -type=SignatureKind -output=stringer_generated.go
//go:generate npx dprint fmt stringer_generated.go

// ParseFlags

pub type ParseFlags = u32;

pub const PARSE_FLAGS_NONE: ParseFlags = 0;
pub const PARSE_FLAGS_YIELD: ParseFlags = 1 << 0;
pub const PARSE_FLAGS_AWAIT: ParseFlags = 1 << 1;
pub const PARSE_FLAGS_TYPE: ParseFlags = 1 << 2;
pub const PARSE_FLAGS_IGNORE_MISSING_OPEN_BRACE: ParseFlags = 1 << 4;

pub type SignatureKind = i32;

pub const SIGNATURE_KIND_CALL: SignatureKind = 0;
pub const SIGNATURE_KIND_CONSTRUCT: SignatureKind = 1;

pub type ContextFlags = u32;

pub const CONTEXT_FLAGS_NONE: ContextFlags = 0;
pub const CONTEXT_FLAGS_SIGNATURE: ContextFlags = 1 << 0; // Obtaining contextual signature
pub const CONTEXT_FLAGS_NO_CONSTRAINTS: ContextFlags = 1 << 1; // Don't obtain type variable constraints
pub const CONTEXT_FLAGS_IGNORE_NODE_INFERENCES: ContextFlags = 1 << 2; // Ignore inference to current node and parent nodes out to the containing call for, for example, completions
pub const CONTEXT_FLAGS_SKIP_BINDING_PATTERNS: ContextFlags = 1 << 3; // Ignore contextual types applied by binding patterns

pub type TypeFormatFlags = u32;

pub const TYPE_FORMAT_FLAGS_NONE: TypeFormatFlags = 0;
pub const TYPE_FORMAT_FLAGS_NO_TRUNCATION: TypeFormatFlags = 1 << 0; // Don't truncate typeToString result
pub const TYPE_FORMAT_FLAGS_WRITE_ARRAY_AS_GENERIC_TYPE: TypeFormatFlags = 1 << 1; // Write Array<T> instead T[]
pub const TYPE_FORMAT_FLAGS_GENERATE_NAMES_FOR_SHADOWED_TYPE_PARAMS: TypeFormatFlags = 1 << 2; // When a type parameter T is shadowing another T, generate a name for it so it can still be referenced
pub const TYPE_FORMAT_FLAGS_USE_STRUCTURAL_FALLBACK: TypeFormatFlags = 1 << 3; // When an alias cannot be named by its symbol, rather than report an error, fallback to a structural printout if possible
// hole because there's a hole in node builder flags
pub const TYPE_FORMAT_FLAGS_WRITE_TYPE_ARGUMENTS_OF_SIGNATURE: TypeFormatFlags = 1 << 5; // Write the type arguments instead of type parameters of the signature
pub const TYPE_FORMAT_FLAGS_USE_FULLY_QUALIFIED_TYPE: TypeFormatFlags = 1 << 6; // Write out the fully qualified type name (eg. Module.Type, instead of Type)
// hole because `UseOnlyExternalAliasing` is here in node builder flags, but functions which take old flags use `SymbolFormatFlags` instead
pub const TYPE_FORMAT_FLAGS_SUPPRESS_ANY_RETURN_TYPE: TypeFormatFlags = 1 << 8; // If the return type is any-like, don't offer a return type.
// hole because `WriteTypeParametersInQualifiedName` is here in node builder flags, but functions which take old flags use `SymbolFormatFlags` for this instead
pub const TYPE_FORMAT_FLAGS_MULTILINE_OBJECT_LITERALS: TypeFormatFlags = 1 << 10; // Always print object literals across multiple lines (only used to map into node builder flags)
pub const TYPE_FORMAT_FLAGS_WRITE_CLASS_EXPRESSION_AS_TYPE_LITERAL: TypeFormatFlags = 1 << 11; // Write a type literal instead of (Anonymous class)
pub const TYPE_FORMAT_FLAGS_USE_TYPE_OF_FUNCTION: TypeFormatFlags = 1 << 12; // Write typeof instead of function type literal
pub const TYPE_FORMAT_FLAGS_OMIT_PARAMETER_MODIFIERS: TypeFormatFlags = 1 << 13; // Omit modifiers on parameters
pub const TYPE_FORMAT_FLAGS_USE_ALIAS_DEFINED_OUTSIDE_CURRENT_SCOPE: TypeFormatFlags = 1 << 14; // For a `type T = ... ` defined in a different file, write `T` instead of its value, even though `T` can't be accessed in the current scope.
pub const TYPE_FORMAT_FLAGS_USE_SINGLE_QUOTES_FOR_STRING_LITERAL_TYPE: TypeFormatFlags = 1 << 28; // Use single quotes for string literal type
pub const TYPE_FORMAT_FLAGS_NO_TYPE_REDUCTION: TypeFormatFlags = 1 << 29; // Don't call getReducedType
pub const TYPE_FORMAT_FLAGS_USE_INSTANTIATION_EXPRESSIONS: TypeFormatFlags = 1 << 30; // Use instantiation expressions for qualified instantiated names like Foo<string>.Bar
pub const TYPE_FORMAT_FLAGS_OMIT_THIS_PARAMETER: TypeFormatFlags = 1 << 25;
pub const TYPE_FORMAT_FLAGS_WRITE_CALL_STYLE_SIGNATURE: TypeFormatFlags = 1 << 27; // Write construct signatures as call style signatures
// Error Handling
pub const TYPE_FORMAT_FLAGS_ALLOW_UNIQUE_ES_SYMBOL_TYPE: TypeFormatFlags = 1 << 20; // This is bit 20 to align with the same bit in `NodeBuilderFlags`
// TypeFormatFlags exclusive
pub const TYPE_FORMAT_FLAGS_ADD_UNDEFINED: TypeFormatFlags = 1 << 17; // Add undefined to types of initialized, non-optional parameters
pub const TYPE_FORMAT_FLAGS_WRITE_ARROW_STYLE_SIGNATURE: TypeFormatFlags = 1 << 18; // Write arrow style signature
// State
pub const TYPE_FORMAT_FLAGS_IN_ARRAY_TYPE: TypeFormatFlags = 1 << 19; // Writing an array element type
pub const TYPE_FORMAT_FLAGS_IN_ELEMENT_TYPE: TypeFormatFlags = 1 << 21; // Writing an array or union element type
pub const TYPE_FORMAT_FLAGS_IN_FIRST_TYPE_ARGUMENT: TypeFormatFlags = 1 << 22; // Writing first type argument of the instantiated type
pub const TYPE_FORMAT_FLAGS_IN_TYPE_ALIAS: TypeFormatFlags = 1 << 23; // Writing type in type alias declaration

pub const TYPE_FORMAT_FLAGS_NODE_BUILDER_FLAGS_MASK: TypeFormatFlags =
    TYPE_FORMAT_FLAGS_NO_TRUNCATION
        | TYPE_FORMAT_FLAGS_WRITE_ARRAY_AS_GENERIC_TYPE
        | TYPE_FORMAT_FLAGS_GENERATE_NAMES_FOR_SHADOWED_TYPE_PARAMS
        | TYPE_FORMAT_FLAGS_USE_STRUCTURAL_FALLBACK
        | TYPE_FORMAT_FLAGS_WRITE_TYPE_ARGUMENTS_OF_SIGNATURE
        | TYPE_FORMAT_FLAGS_USE_FULLY_QUALIFIED_TYPE
        | TYPE_FORMAT_FLAGS_SUPPRESS_ANY_RETURN_TYPE
        | TYPE_FORMAT_FLAGS_MULTILINE_OBJECT_LITERALS
        | TYPE_FORMAT_FLAGS_WRITE_CLASS_EXPRESSION_AS_TYPE_LITERAL
        | TYPE_FORMAT_FLAGS_USE_TYPE_OF_FUNCTION
        | TYPE_FORMAT_FLAGS_OMIT_PARAMETER_MODIFIERS
        | TYPE_FORMAT_FLAGS_USE_ALIAS_DEFINED_OUTSIDE_CURRENT_SCOPE
        | TYPE_FORMAT_FLAGS_ALLOW_UNIQUE_ES_SYMBOL_TYPE
        | TYPE_FORMAT_FLAGS_IN_TYPE_ALIAS
        | TYPE_FORMAT_FLAGS_USE_INSTANTIATION_EXPRESSIONS
        | TYPE_FORMAT_FLAGS_USE_SINGLE_QUOTES_FOR_STRING_LITERAL_TYPE
        | TYPE_FORMAT_FLAGS_NO_TYPE_REDUCTION
        | TYPE_FORMAT_FLAGS_OMIT_THIS_PARAMETER;

pub type SymbolFormatFlags = u32;

pub const SYMBOL_FORMAT_FLAGS_NONE: SymbolFormatFlags = 0;
// Write symbols's type argument if it is instantiated symbol
// eg. class C<T> { p: T }   <-- Show p as C<T>.p here
//     var a: C<number>;
//     var p = a.p; <--- Here p is property of C<number> so show it as C<number>.p instead of just C.p
pub const SYMBOL_FORMAT_FLAGS_WRITE_TYPE_PARAMETERS_OR_ARGUMENTS: SymbolFormatFlags = 1 << 0;
// Use only external alias information to get the symbol name in the given context
// eg.  module m { export class c { } } import x = m.c;
// When this flag is specified m.c will be used to refer to the class instead of alias symbol x
pub const SYMBOL_FORMAT_FLAGS_USE_ONLY_EXTERNAL_ALIASING: SymbolFormatFlags = 1 << 1;
// Build symbol name using any nodes needed, instead of just components of an entity name
pub const SYMBOL_FORMAT_FLAGS_ALLOW_ANY_NODE_KIND: SymbolFormatFlags = 1 << 2;
// Prefer aliases which are not directly visible
pub const SYMBOL_FORMAT_FLAGS_USE_ALIAS_DEFINED_OUTSIDE_CURRENT_SCOPE: SymbolFormatFlags = 1 << 3;
// { [E.A]: 1 }
/** @internal */
pub const SYMBOL_FORMAT_FLAGS_WRITE_COMPUTED_PROPS: SymbolFormatFlags = 1 << 4;
// Skip building an accessible symbol chain
/** @internal */
pub const SYMBOL_FORMAT_FLAGS_DO_NOT_INCLUDE_SYMBOL_CHAIN: SymbolFormatFlags = 1 << 5;

pub type TypeFlags = u32;

// Note that for types of different kinds, the numeric values of TypeFlags determine the order
// computed by the CompareTypes function and therefore the order of constituent types in union types.
// Since union type processing often bails out early when a result is known, it is important to order
// TypeFlags in increasing order of potential type complexity. In particular, indexed access and
// conditional types should sort last as those types are potentially recursive and possibly infinite.

pub const TYPE_FLAGS_NONE: TypeFlags = 0;
pub const TYPE_FLAGS_ANY: TypeFlags = 1 << 0;
pub const TYPE_FLAGS_UNKNOWN: TypeFlags = 1 << 1;
pub const TYPE_FLAGS_UNDEFINED: TypeFlags = 1 << 2;
pub const TYPE_FLAGS_NULL: TypeFlags = 1 << 3;
pub const TYPE_FLAGS_VOID: TypeFlags = 1 << 4;
pub const TYPE_FLAGS_STRING: TypeFlags = 1 << 5;
pub const TYPE_FLAGS_NUMBER: TypeFlags = 1 << 6;
pub const TYPE_FLAGS_BIG_INT: TypeFlags = 1 << 7;
pub const TYPE_FLAGS_BOOLEAN: TypeFlags = 1 << 8;
pub const TYPE_FLAGS_ES_SYMBOL: TypeFlags = 1 << 9; // Type of symbol primitive introduced in ES6
pub const TYPE_FLAGS_STRING_LITERAL: TypeFlags = 1 << 10;
pub const TYPE_FLAGS_NUMBER_LITERAL: TypeFlags = 1 << 11;
pub const TYPE_FLAGS_BIG_INT_LITERAL: TypeFlags = 1 << 12;
pub const TYPE_FLAGS_BOOLEAN_LITERAL: TypeFlags = 1 << 13;
pub const TYPE_FLAGS_UNIQUE_ES_SYMBOL: TypeFlags = 1 << 14; // unique symbol
pub const TYPE_FLAGS_ENUM_LITERAL: TypeFlags = 1 << 15; // Always combined with StringLiteral, NumberLiteral, or Union
pub const TYPE_FLAGS_ENUM: TypeFlags = 1 << 16; // Numeric computed enum member value (must be right after EnumLiteral, see getSortOrderFlags)
pub const TYPE_FLAGS_NON_PRIMITIVE: TypeFlags = 1 << 17; // intrinsic object type
pub const TYPE_FLAGS_NEVER: TypeFlags = 1 << 18; // Never type
pub const TYPE_FLAGS_TYPE_PARAMETER: TypeFlags = 1 << 19; // Type parameter
pub const TYPE_FLAGS_OBJECT: TypeFlags = 1 << 20; // Object type
pub const TYPE_FLAGS_INDEX: TypeFlags = 1 << 21; // keyof T
pub const TYPE_FLAGS_TEMPLATE_LITERAL: TypeFlags = 1 << 22; // Template literal type
pub const TYPE_FLAGS_STRING_MAPPING: TypeFlags = 1 << 23; // Uppercase/Lowercase type
pub const TYPE_FLAGS_SUBSTITUTION: TypeFlags = 1 << 24; // Type parameter substitution
pub const TYPE_FLAGS_INDEXED_ACCESS: TypeFlags = 1 << 25; // T[K]
pub const TYPE_FLAGS_CONDITIONAL: TypeFlags = 1 << 26; // T extends U ? X
pub const TYPE_FLAGS_UNION: TypeFlags = 1 << 27; // Union (T | U)
pub const TYPE_FLAGS_INTERSECTION: TypeFlags = 1 << 28; // Intersection (T & U)
pub const TYPE_FLAGS_RESERVED1: TypeFlags = 1 << 29; // Used by union/intersection type construction
pub const TYPE_FLAGS_RESERVED2: TypeFlags = 1 << 30; // Used by union/intersection type construction
pub const TYPE_FLAGS_RESERVED3: TypeFlags = 1 << 31;

pub const TYPE_FLAGS_ANY_OR_UNKNOWN: TypeFlags = TYPE_FLAGS_ANY | TYPE_FLAGS_UNKNOWN;
pub const TYPE_FLAGS_NULLABLE: TypeFlags = TYPE_FLAGS_UNDEFINED | TYPE_FLAGS_NULL;
pub const TYPE_FLAGS_LITERAL: TypeFlags = TYPE_FLAGS_STRING_LITERAL
    | TYPE_FLAGS_NUMBER_LITERAL
    | TYPE_FLAGS_BIG_INT_LITERAL
    | TYPE_FLAGS_BOOLEAN_LITERAL;
pub const TYPE_FLAGS_UNIT: TypeFlags =
    TYPE_FLAGS_ENUM | TYPE_FLAGS_LITERAL | TYPE_FLAGS_UNIQUE_ES_SYMBOL | TYPE_FLAGS_NULLABLE;
pub const TYPE_FLAGS_FRESHABLE: TypeFlags = TYPE_FLAGS_ENUM | TYPE_FLAGS_LITERAL;
pub const TYPE_FLAGS_STRING_OR_NUMBER_LITERAL: TypeFlags =
    TYPE_FLAGS_STRING_LITERAL | TYPE_FLAGS_NUMBER_LITERAL;
pub const TYPE_FLAGS_STRING_OR_NUMBER_LITERAL_OR_UNIQUE: TypeFlags =
    TYPE_FLAGS_STRING_LITERAL | TYPE_FLAGS_NUMBER_LITERAL | TYPE_FLAGS_UNIQUE_ES_SYMBOL;
pub const TYPE_FLAGS_DEFINITELY_FALSY: TypeFlags = TYPE_FLAGS_STRING_LITERAL
    | TYPE_FLAGS_NUMBER_LITERAL
    | TYPE_FLAGS_BIG_INT_LITERAL
    | TYPE_FLAGS_BOOLEAN_LITERAL
    | TYPE_FLAGS_VOID
    | TYPE_FLAGS_UNDEFINED
    | TYPE_FLAGS_NULL;
pub const TYPE_FLAGS_POSSIBLY_FALSY: TypeFlags = TYPE_FLAGS_DEFINITELY_FALSY
    | TYPE_FLAGS_STRING
    | TYPE_FLAGS_NUMBER
    | TYPE_FLAGS_BIG_INT
    | TYPE_FLAGS_BOOLEAN;
pub const TYPE_FLAGS_INTRINSIC: TypeFlags = TYPE_FLAGS_ANY
    | TYPE_FLAGS_UNKNOWN
    | TYPE_FLAGS_STRING
    | TYPE_FLAGS_NUMBER
    | TYPE_FLAGS_BIG_INT
    | TYPE_FLAGS_ES_SYMBOL
    | TYPE_FLAGS_VOID
    | TYPE_FLAGS_UNDEFINED
    | TYPE_FLAGS_NULL
    | TYPE_FLAGS_NEVER
    | TYPE_FLAGS_NON_PRIMITIVE;
pub const TYPE_FLAGS_STRING_LIKE: TypeFlags = TYPE_FLAGS_STRING
    | TYPE_FLAGS_STRING_LITERAL
    | TYPE_FLAGS_TEMPLATE_LITERAL
    | TYPE_FLAGS_STRING_MAPPING;
pub const TYPE_FLAGS_NUMBER_LIKE: TypeFlags =
    TYPE_FLAGS_NUMBER | TYPE_FLAGS_NUMBER_LITERAL | TYPE_FLAGS_ENUM;
pub const TYPE_FLAGS_BIG_INT_LIKE: TypeFlags = TYPE_FLAGS_BIG_INT | TYPE_FLAGS_BIG_INT_LITERAL;
pub const TYPE_FLAGS_BOOLEAN_LIKE: TypeFlags = TYPE_FLAGS_BOOLEAN | TYPE_FLAGS_BOOLEAN_LITERAL;
pub const TYPE_FLAGS_ENUM_LIKE: TypeFlags = TYPE_FLAGS_ENUM | TYPE_FLAGS_ENUM_LITERAL;
pub const TYPE_FLAGS_ES_SYMBOL_LIKE: TypeFlags = TYPE_FLAGS_ES_SYMBOL | TYPE_FLAGS_UNIQUE_ES_SYMBOL;
pub const TYPE_FLAGS_VOID_LIKE: TypeFlags = TYPE_FLAGS_VOID | TYPE_FLAGS_UNDEFINED;
pub const TYPE_FLAGS_PRIMITIVE: TypeFlags = TYPE_FLAGS_STRING_LIKE
    | TYPE_FLAGS_NUMBER_LIKE
    | TYPE_FLAGS_BIG_INT_LIKE
    | TYPE_FLAGS_BOOLEAN_LIKE
    | TYPE_FLAGS_ENUM_LIKE
    | TYPE_FLAGS_ES_SYMBOL_LIKE
    | TYPE_FLAGS_VOID_LIKE
    | TYPE_FLAGS_NULL;
pub const TYPE_FLAGS_DEFINITELY_NON_NULLABLE: TypeFlags = TYPE_FLAGS_STRING_LIKE
    | TYPE_FLAGS_NUMBER_LIKE
    | TYPE_FLAGS_BIG_INT_LIKE
    | TYPE_FLAGS_BOOLEAN_LIKE
    | TYPE_FLAGS_ENUM_LIKE
    | TYPE_FLAGS_ES_SYMBOL_LIKE
    | TYPE_FLAGS_OBJECT
    | TYPE_FLAGS_NON_PRIMITIVE;
pub const TYPE_FLAGS_DISJOINT_DOMAINS: TypeFlags = TYPE_FLAGS_NON_PRIMITIVE
    | TYPE_FLAGS_STRING_LIKE
    | TYPE_FLAGS_NUMBER_LIKE
    | TYPE_FLAGS_BIG_INT_LIKE
    | TYPE_FLAGS_BOOLEAN_LIKE
    | TYPE_FLAGS_ES_SYMBOL_LIKE
    | TYPE_FLAGS_VOID_LIKE
    | TYPE_FLAGS_NULL;
pub const TYPE_FLAGS_UNION_OR_INTERSECTION: TypeFlags = TYPE_FLAGS_UNION | TYPE_FLAGS_INTERSECTION;
pub const TYPE_FLAGS_STRUCTURED_TYPE: TypeFlags =
    TYPE_FLAGS_OBJECT | TYPE_FLAGS_UNION | TYPE_FLAGS_INTERSECTION;
pub const TYPE_FLAGS_TYPE_VARIABLE: TypeFlags =
    TYPE_FLAGS_TYPE_PARAMETER | TYPE_FLAGS_INDEXED_ACCESS;
pub const TYPE_FLAGS_INSTANTIABLE_NON_PRIMITIVE: TypeFlags =
    TYPE_FLAGS_TYPE_VARIABLE | TYPE_FLAGS_CONDITIONAL | TYPE_FLAGS_SUBSTITUTION;
pub const TYPE_FLAGS_INSTANTIABLE_PRIMITIVE: TypeFlags =
    TYPE_FLAGS_INDEX | TYPE_FLAGS_TEMPLATE_LITERAL | TYPE_FLAGS_STRING_MAPPING;
pub const TYPE_FLAGS_INSTANTIABLE: TypeFlags =
    TYPE_FLAGS_INSTANTIABLE_NON_PRIMITIVE | TYPE_FLAGS_INSTANTIABLE_PRIMITIVE;
pub const TYPE_FLAGS_STRUCTURED_OR_INSTANTIABLE: TypeFlags =
    TYPE_FLAGS_STRUCTURED_TYPE | TYPE_FLAGS_INSTANTIABLE;
pub const TYPE_FLAGS_OBJECT_FLAGS_TYPE: TypeFlags = TYPE_FLAGS_ANY
    | TYPE_FLAGS_NULLABLE
    | TYPE_FLAGS_NEVER
    | TYPE_FLAGS_OBJECT
    | TYPE_FLAGS_UNION
    | TYPE_FLAGS_INTERSECTION;
pub const TYPE_FLAGS_SIMPLIFIABLE: TypeFlags =
    TYPE_FLAGS_INDEXED_ACCESS | TYPE_FLAGS_CONDITIONAL | TYPE_FLAGS_INDEX;
pub const TYPE_FLAGS_SINGLETON: TypeFlags = TYPE_FLAGS_ANY
    | TYPE_FLAGS_UNKNOWN
    | TYPE_FLAGS_STRING
    | TYPE_FLAGS_NUMBER
    | TYPE_FLAGS_BOOLEAN
    | TYPE_FLAGS_BIG_INT
    | TYPE_FLAGS_ES_SYMBOL
    | TYPE_FLAGS_VOID
    | TYPE_FLAGS_UNDEFINED
    | TYPE_FLAGS_NULL
    | TYPE_FLAGS_NEVER
    | TYPE_FLAGS_NON_PRIMITIVE;
// 'TypeFlagsNarrowable' types are types where narrowing actually narrows.
// This *should* be every type other than null, undefined, void, and never
pub const TYPE_FLAGS_NARROWABLE: TypeFlags = TYPE_FLAGS_ANY
    | TYPE_FLAGS_UNKNOWN
    | TYPE_FLAGS_STRUCTURED_OR_INSTANTIABLE
    | TYPE_FLAGS_STRING_LIKE
    | TYPE_FLAGS_NUMBER_LIKE
    | TYPE_FLAGS_BIG_INT_LIKE
    | TYPE_FLAGS_BOOLEAN_LIKE
    | TYPE_FLAGS_ES_SYMBOL
    | TYPE_FLAGS_UNIQUE_ES_SYMBOL
    | TYPE_FLAGS_NON_PRIMITIVE;
// The following flags are aggregated during union and intersection type construction
pub const TYPE_FLAGS_INCLUDES_MASK: TypeFlags = TYPE_FLAGS_ANY
    | TYPE_FLAGS_UNKNOWN
    | TYPE_FLAGS_PRIMITIVE
    | TYPE_FLAGS_NEVER
    | TYPE_FLAGS_OBJECT
    | TYPE_FLAGS_UNION
    | TYPE_FLAGS_INTERSECTION
    | TYPE_FLAGS_NON_PRIMITIVE
    | TYPE_FLAGS_TEMPLATE_LITERAL
    | TYPE_FLAGS_STRING_MAPPING;
// The following flags are used for different purposes during union and intersection type construction
pub const TYPE_FLAGS_INCLUDES_MISSING_TYPE: TypeFlags = TYPE_FLAGS_TYPE_PARAMETER;
pub const TYPE_FLAGS_INCLUDES_NON_WIDENING_TYPE: TypeFlags = TYPE_FLAGS_INDEX;
pub const TYPE_FLAGS_INCLUDES_WILDCARD: TypeFlags = TYPE_FLAGS_INDEXED_ACCESS;
pub const TYPE_FLAGS_INCLUDES_EMPTY_OBJECT: TypeFlags = TYPE_FLAGS_CONDITIONAL;
pub const TYPE_FLAGS_INCLUDES_INSTANTIABLE: TypeFlags = TYPE_FLAGS_SUBSTITUTION;
pub const TYPE_FLAGS_INCLUDES_CONSTRAINED_TYPE_VARIABLE: TypeFlags = TYPE_FLAGS_RESERVED1;
pub const TYPE_FLAGS_INCLUDES_ERROR: TypeFlags = TYPE_FLAGS_RESERVED2;
pub const TYPE_FLAGS_NOT_PRIMITIVE_UNION: TypeFlags = TYPE_FLAGS_ANY
    | TYPE_FLAGS_UNKNOWN
    | TYPE_FLAGS_VOID
    | TYPE_FLAGS_NEVER
    | TYPE_FLAGS_OBJECT
    | TYPE_FLAGS_INTERSECTION
    | TYPE_FLAGS_INCLUDES_INSTANTIABLE;

struct TypeFlagName {
    flag: TypeFlags,
    name: &'static str,
}

static TYPE_FLAG_NAMES: &[TypeFlagName] = &[
    TypeFlagName {
        flag: TYPE_FLAGS_ANY,
        name: "Any",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_UNKNOWN,
        name: "Unknown",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_UNDEFINED,
        name: "Undefined",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_NULL,
        name: "Null",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_VOID,
        name: "Void",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_STRING,
        name: "String",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_NUMBER,
        name: "Number",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_BIG_INT,
        name: "BigInt",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_BOOLEAN,
        name: "Boolean",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_ES_SYMBOL,
        name: "ESSymbol",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_STRING_LITERAL,
        name: "StringLiteral",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_NUMBER_LITERAL,
        name: "NumberLiteral",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_BIG_INT_LITERAL,
        name: "BigIntLiteral",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_BOOLEAN_LITERAL,
        name: "BooleanLiteral",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_UNIQUE_ES_SYMBOL,
        name: "UniqueESSymbol",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_ENUM_LITERAL,
        name: "EnumLiteral",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_ENUM,
        name: "Enum",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_NON_PRIMITIVE,
        name: "NonPrimitive",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_NEVER,
        name: "Never",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_TYPE_PARAMETER,
        name: "TypeParameter",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_OBJECT,
        name: "Object",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_INDEX,
        name: "Index",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_TEMPLATE_LITERAL,
        name: "TemplateLiteral",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_STRING_MAPPING,
        name: "StringMapping",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_SUBSTITUTION,
        name: "Substitution",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_INDEXED_ACCESS,
        name: "IndexedAccess",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_CONDITIONAL,
        name: "Conditional",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_UNION,
        name: "Union",
    },
    TypeFlagName {
        flag: TYPE_FLAGS_INTERSECTION,
        name: "Intersection",
    },
];

// FormatTypeFlags returns the individual flag names as a slice of strings.
pub fn format_type_flags(flags: TypeFlags) -> Vec<String> {
    let mut result = Vec::with_capacity(flags.count_ones() as usize);
    for fn_ in TYPE_FLAG_NAMES {
        if flags & fn_.flag != 0 {
            result.push(fn_.name.to_string());
        }
    }
    if result.is_empty() {
        result.push("None".to_string());
    }
    result
}

pub fn type_flags_string(f: TypeFlags) -> String {
    format_type_flags(f).join("|")
}

pub type ObjectFlags = u32;

// Types included in TypeFlags.ObjectFlagsType have an objectFlags property. Some ObjectFlags
// are specific to certain types and reuse the same bit position. Those ObjectFlags require a check
// for a certain TypeFlags value to determine their meaning.
pub const OBJECT_FLAGS_NONE: ObjectFlags = 0;
pub const OBJECT_FLAGS_CLASS: ObjectFlags = 1 << 0; // Class
pub const OBJECT_FLAGS_INTERFACE: ObjectFlags = 1 << 1; // Interface
pub const OBJECT_FLAGS_REFERENCE: ObjectFlags = 1 << 2; // Generic type reference
pub const OBJECT_FLAGS_TUPLE: ObjectFlags = 1 << 3; // Synthesized generic tuple type
pub const OBJECT_FLAGS_ANONYMOUS: ObjectFlags = 1 << 4; // Anonymous
pub const OBJECT_FLAGS_MAPPED: ObjectFlags = 1 << 5; // Mapped
pub const OBJECT_FLAGS_INSTANTIATED: ObjectFlags = 1 << 6; // Instantiated anonymous or mapped type
pub const OBJECT_FLAGS_OBJECT_LITERAL: ObjectFlags = 1 << 7; // Originates in an object literal
pub const OBJECT_FLAGS_EVOLVING_ARRAY: ObjectFlags = 1 << 8; // Evolving array type
pub const OBJECT_FLAGS_OBJECT_LITERAL_PATTERN_WITH_COMPUTED_PROPERTIES: ObjectFlags = 1 << 9; // Object literal pattern with computed properties
pub const OBJECT_FLAGS_REVERSE_MAPPED: ObjectFlags = 1 << 10; // Object contains a property from a reverse-mapped type
pub const OBJECT_FLAGS_JSX_ATTRIBUTES: ObjectFlags = 1 << 11; // Jsx attributes type
pub const OBJECT_FLAGS_JS_LITERAL: ObjectFlags = 1 << 12; // Object type declared in JS - disables errors on read/write of nonexisting members
pub const OBJECT_FLAGS_FRESH_LITERAL: ObjectFlags = 1 << 13; // Fresh object literal
pub const OBJECT_FLAGS_ARRAY_LITERAL: ObjectFlags = 1 << 14; // Originates in an array literal
pub const OBJECT_FLAGS_PRIMITIVE_UNION: ObjectFlags = 1 << 15; // Union of only primitive types
pub const OBJECT_FLAGS_CONTAINS_WIDENING_TYPE: ObjectFlags = 1 << 16; // Type is or contains undefined or null widening type
pub const OBJECT_FLAGS_CONTAINS_OBJECT_OR_ARRAY_LITERAL: ObjectFlags = 1 << 17; // Type is or contains object literal type
pub const OBJECT_FLAGS_NON_INFERRABLE_TYPE: ObjectFlags = 1 << 18; // Type is or contains anyFunctionType or silentNeverType
pub const OBJECT_FLAGS_COULD_CONTAIN_TYPE_VARIABLES_COMPUTED: ObjectFlags = 1 << 19; // CouldContainTypeVariables flag has been computed
pub const OBJECT_FLAGS_COULD_CONTAIN_TYPE_VARIABLES: ObjectFlags = 1 << 20; // Type could contain a type variable
pub const OBJECT_FLAGS_MEMBERS_RESOLVED: ObjectFlags = 1 << 21; // Members have been resolved

pub const OBJECT_FLAGS_CLASS_OR_INTERFACE: ObjectFlags =
    OBJECT_FLAGS_CLASS | OBJECT_FLAGS_INTERFACE;
pub const OBJECT_FLAGS_REQUIRES_WIDENING: ObjectFlags =
    OBJECT_FLAGS_CONTAINS_WIDENING_TYPE | OBJECT_FLAGS_CONTAINS_OBJECT_OR_ARRAY_LITERAL;
pub const OBJECT_FLAGS_PROPAGATING_FLAGS: ObjectFlags = OBJECT_FLAGS_CONTAINS_WIDENING_TYPE
    | OBJECT_FLAGS_CONTAINS_OBJECT_OR_ARRAY_LITERAL
    | OBJECT_FLAGS_NON_INFERRABLE_TYPE;
pub const OBJECT_FLAGS_INSTANTIATED_MAPPED: ObjectFlags =
    OBJECT_FLAGS_MAPPED | OBJECT_FLAGS_INSTANTIATED;
// Object flags that uniquely identify the kind of ObjectType
pub const OBJECT_FLAGS_OBJECT_TYPE_KIND_MASK: ObjectFlags = OBJECT_FLAGS_CLASS_OR_INTERFACE
    | OBJECT_FLAGS_REFERENCE
    | OBJECT_FLAGS_TUPLE
    | OBJECT_FLAGS_ANONYMOUS
    | OBJECT_FLAGS_MAPPED
    | OBJECT_FLAGS_REVERSE_MAPPED
    | OBJECT_FLAGS_EVOLVING_ARRAY
    | OBJECT_FLAGS_INSTANTIATION_EXPRESSION_TYPE
    | OBJECT_FLAGS_SINGLE_SIGNATURE_TYPE;
// Flags that require TypeFlags.Object
pub const OBJECT_FLAGS_CONTAINS_SPREAD: ObjectFlags = 1 << 22; // Object literal contains spread operation
pub const OBJECT_FLAGS_OBJECT_REST_TYPE: ObjectFlags = 1 << 23; // Originates in object rest declaration
pub const OBJECT_FLAGS_INSTANTIATION_EXPRESSION_TYPE: ObjectFlags = 1 << 24; // Originates in instantiation expression
pub const OBJECT_FLAGS_SINGLE_SIGNATURE_TYPE: ObjectFlags = 1 << 25; // A single signature type extracted from a potentially broader type
pub const OBJECT_FLAGS_IS_CLASS_INSTANCE_CLONE: ObjectFlags = 1 << 26; // Type is a clone of a class instance type
// Flags that require TypeFlags.Object and ObjectFlags.Reference
pub const OBJECT_FLAGS_IDENTICAL_BASE_TYPE_CALCULATED: ObjectFlags = 1 << 27; // has had `getSingleBaseForNonAugmentingSubtype` invoked on it already
pub const OBJECT_FLAGS_IDENTICAL_BASE_TYPE_EXISTS: ObjectFlags = 1 << 28; // has a defined cachedEquivalentBaseType member
pub const OBJECT_FLAGS_UNRESOLVED_MEMBERS: ObjectFlags = 1 << 29; // Member resolution in process
pub const OBJECT_FLAGS_FROM_TYPE_NODE: ObjectFlags = 1 << 30; // Originates in resolution of AST type node
// Flags that require TypeFlags.UnionOrIntersection or TypeFlags.Substitution
pub const OBJECT_FLAGS_IS_GENERIC_TYPE_COMPUTED: ObjectFlags = 1 << 22; // IsGenericObjectType flag has been computed
pub const OBJECT_FLAGS_IS_GENERIC_OBJECT_TYPE: ObjectFlags = 1 << 23; // Union or intersection contains generic object type
pub const OBJECT_FLAGS_IS_GENERIC_INDEX_TYPE: ObjectFlags = 1 << 24; // Union or intersection contains generic index type
pub const OBJECT_FLAGS_IS_GENERIC_TYPE: ObjectFlags =
    OBJECT_FLAGS_IS_GENERIC_OBJECT_TYPE | OBJECT_FLAGS_IS_GENERIC_INDEX_TYPE;
// Flags that require TypeFlags.Union
pub const OBJECT_FLAGS_CONTAINS_INTERSECTIONS: ObjectFlags = 1 << 25; // Union contains intersections
pub const OBJECT_FLAGS_IS_UNKNOWN_LIKE_UNION_COMPUTED: ObjectFlags = 1 << 26; // IsUnknownLikeUnion flag has been computed
pub const OBJECT_FLAGS_IS_UNKNOWN_LIKE_UNION: ObjectFlags = 1 << 27; // Union of null, undefined, and empty object type
// Flags that require TypeFlags.Intersection
pub const OBJECT_FLAGS_IS_NEVER_INTERSECTION_COMPUTED: ObjectFlags = 1 << 25; // IsNeverLike flag has been computed
pub const OBJECT_FLAGS_IS_NEVER_INTERSECTION: ObjectFlags = 1 << 26; // Intersection reduces to never
pub const OBJECT_FLAGS_IS_CONSTRAINED_TYPE_VARIABLE: ObjectFlags = 1 << 27; // T & C, where T's constraint and C are primitives, object, or {}

// TupleType

pub type ElementFlags = u32;

pub const ELEMENT_FLAGS_NONE: ElementFlags = 0;
pub const ELEMENT_FLAGS_REQUIRED: ElementFlags = 1 << 0; // T
pub const ELEMENT_FLAGS_OPTIONAL: ElementFlags = 1 << 1; // T?
pub const ELEMENT_FLAGS_REST: ElementFlags = 1 << 2; // ., ..T[]
pub const ELEMENT_FLAGS_VARIADIC: ElementFlags = 1 << 3; // ., ..T
pub const ELEMENT_FLAGS_FIXED: ElementFlags = ELEMENT_FLAGS_REQUIRED | ELEMENT_FLAGS_OPTIONAL;
pub const ELEMENT_FLAGS_VARIABLE: ElementFlags = ELEMENT_FLAGS_REST | ELEMENT_FLAGS_VARIADIC;
pub const ELEMENT_FLAGS_NON_REQUIRED: ElementFlags =
    ELEMENT_FLAGS_OPTIONAL | ELEMENT_FLAGS_REST | ELEMENT_FLAGS_VARIADIC;
pub const ELEMENT_FLAGS_NON_REST: ElementFlags =
    ELEMENT_FLAGS_REQUIRED | ELEMENT_FLAGS_OPTIONAL | ELEMENT_FLAGS_VARIADIC;

#[derive(Clone, Copy, Default)]
pub struct TupleElementInfo {
    pub flags: ElementFlags,
    pub labeled_declaration: Option<ast::Node>, // NamedTupleMember | ParameterDeclaration | nil
}

impl TupleElementInfo {
    pub fn tuple_element_flags(&self) -> ElementFlags {
        self.flags
    }
    pub fn labeled_declaration(&self) -> Option<ast::Node> {
        self.labeled_declaration
    }
}

// IndexFlags

pub type IndexFlags = u32;

pub const INDEX_FLAGS_NONE: IndexFlags = 0;
pub const INDEX_FLAGS_STRINGS_ONLY: IndexFlags = 1 << 0;
pub const INDEX_FLAGS_NO_INDEX_SIGNATURES: IndexFlags = 1 << 1;
pub const INDEX_FLAGS_NO_REDUCIBLE_CHECK: IndexFlags = 1 << 2;

// SignatureFlags

pub type SignatureFlags = u32;

pub const SIGNATURE_FLAGS_NONE: SignatureFlags = 0;
// Propagating flags
pub const SIGNATURE_FLAGS_HAS_REST_PARAMETER: SignatureFlags = 1 << 0; // Indicates last parameter is rest parameter
pub const SIGNATURE_FLAGS_HAS_LITERAL_TYPES: SignatureFlags = 1 << 1; // Indicates signature is specialized
pub const SIGNATURE_FLAGS_CONSTRUCT: SignatureFlags = 1 << 2; // Indicates signature is a construct signature
pub const SIGNATURE_FLAGS_ABSTRACT: SignatureFlags = 1 << 3; // Indicates signature comes from an abstract class, abstract construct signature, or abstract constructor type
// Non-propagating flags
pub const SIGNATURE_FLAGS_IS_INNER_CALL_CHAIN: SignatureFlags = 1 << 4; // Indicates signature comes from a CallChain nested in an outer OptionalChain
pub const SIGNATURE_FLAGS_IS_OUTER_CALL_CHAIN: SignatureFlags = 1 << 5; // Indicates signature comes from a CallChain that is the outermost chain of an optional expression
pub const SIGNATURE_FLAGS_IS_UNTYPED_SIGNATURE_IN_JS_FILE: SignatureFlags = 1 << 6; // Indicates signature is from a js file and has no types
pub const SIGNATURE_FLAGS_IS_NON_INFERRABLE: SignatureFlags = 1 << 7; // Indicates signature comes from a non-inferrable type
pub const SIGNATURE_FLAGS_IS_SIGNATURE_CANDIDATE_FOR_OVERLOAD_FAILURE: SignatureFlags = 1 << 8;
// We do not propagate `IsInnerCallChain` or `IsOuterCallChain` to instantiated signatures, as that would result in us
// attempting to add `| undefined` on each recursive call to `getReturnTypeOfSignature` when
// instantiating the return type.
pub const SIGNATURE_FLAGS_PROPAGATING_FLAGS: SignatureFlags = SIGNATURE_FLAGS_HAS_REST_PARAMETER
    | SIGNATURE_FLAGS_HAS_LITERAL_TYPES
    | SIGNATURE_FLAGS_CONSTRUCT
    | SIGNATURE_FLAGS_ABSTRACT
    | SIGNATURE_FLAGS_IS_UNTYPED_SIGNATURE_IN_JS_FILE
    | SIGNATURE_FLAGS_IS_SIGNATURE_CANDIDATE_FOR_OVERLOAD_FAILURE;
pub const SIGNATURE_FLAGS_CALL_CHAIN_FLAGS: SignatureFlags =
    SIGNATURE_FLAGS_IS_INNER_CALL_CHAIN | SIGNATURE_FLAGS_IS_OUTER_CALL_CHAIN;

pub type TypePredicateKind = i32;

pub const TYPE_PREDICATE_KIND_THIS: TypePredicateKind = 0;
pub const TYPE_PREDICATE_KIND_IDENTIFIER: TypePredicateKind = 1;
pub const TYPE_PREDICATE_KIND_ASSERTS_THIS: TypePredicateKind = 2;
pub const TYPE_PREDICATE_KIND_ASSERTS_IDENTIFIER: TypePredicateKind = 3;

/**
 * Ternary values are defined such that
 * x & y picks the lesser in the order False < Unknown < Maybe < True, and
 * x | y picks the greater in the order False < Unknown < Maybe < True.
 * Generally, Ternary.Maybe is used as the result of a relation that depends on itself, and
 * Ternary.Unknown is used as the result of a variance check that depends on itself. We make
 * a distinction because we don't want to cache circular variance check results.
 */
pub type Ternary = i8;

pub const TERNARY_FALSE: Ternary = 0;
pub const TERNARY_UNKNOWN: Ternary = 1;
pub const TERNARY_MAYBE: Ternary = 3;
pub const TERNARY_TRUE: Ternary = -1;

pub struct LanguageFeatureMinimumTargetMap {
    pub exponentiation: core::ScriptTarget,
    pub async_functions: core::ScriptTarget,
    pub for_await_of: core::ScriptTarget,
    pub async_generators: core::ScriptTarget,
    pub async_iteration: core::ScriptTarget,
    pub object_spread_rest: core::ScriptTarget,
    pub regular_expression_flags_dot_all: core::ScriptTarget,
    pub bindingless_catch: core::ScriptTarget,
    pub big_int: core::ScriptTarget,
    pub nullish_coalesce: core::ScriptTarget,
    pub optional_chaining: core::ScriptTarget,
    pub logical_assignment: core::ScriptTarget,
    pub top_level_await: core::ScriptTarget,
    pub class_fields: core::ScriptTarget,
    pub private_names_and_class_static_blocks: core::ScriptTarget,
    pub regular_expression_flags_has_indices: core::ScriptTarget,
    pub shebang_comments: core::ScriptTarget,
    pub using_and_await_using: core::ScriptTarget,
    pub class_and_class_element_decorators: core::ScriptTarget,
    pub regular_expression_flags_unicode_sets: core::ScriptTarget,
}

pub const LANGUAGE_FEATURE_MINIMUM_TARGET: LanguageFeatureMinimumTargetMap =
    LanguageFeatureMinimumTargetMap {
        exponentiation: core::ScriptTarget::ES2016,
        async_functions: core::ScriptTarget::ES2017,
        for_await_of: core::ScriptTarget::ES2018,
        async_generators: core::ScriptTarget::ES2018,
        async_iteration: core::ScriptTarget::ES2018,
        object_spread_rest: core::ScriptTarget::ES2018,
        regular_expression_flags_dot_all: core::ScriptTarget::ES2018,
        bindingless_catch: core::ScriptTarget::ES2019,
        big_int: core::ScriptTarget::ES2020,
        nullish_coalesce: core::ScriptTarget::ES2020,
        optional_chaining: core::ScriptTarget::ES2020,
        logical_assignment: core::ScriptTarget::ES2021,
        top_level_await: core::ScriptTarget::ES2022,
        class_fields: core::ScriptTarget::ES2022,
        private_names_and_class_static_blocks: core::ScriptTarget::ES2022,
        regular_expression_flags_has_indices: core::ScriptTarget::ES2022,
        shebang_comments: core::ScriptTarget::ESNext,
        using_and_await_using: core::ScriptTarget::ESNext,
        class_and_class_element_decorators: core::ScriptTarget::ESNext,
        regular_expression_flags_unicode_sets: core::ScriptTarget::ESNext,
    };

impl fmt::Display for TypeFlagsDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&type_flags_string(self.0))
    }
}

pub struct TypeFlagsDisplay(pub TypeFlags);
