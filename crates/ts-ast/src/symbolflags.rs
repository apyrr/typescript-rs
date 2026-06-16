// SymbolFlags

pub type SymbolFlags = u32;

pub const SYMBOL_FLAGS_NONE: SymbolFlags = 0;
pub const SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE: SymbolFlags = 1 << 0; // Variable (var) or parameter
pub const SYMBOL_FLAGS_BLOCK_SCOPED_VARIABLE: SymbolFlags = 1 << 1; // A block-scoped variable (let or const)
pub const SYMBOL_FLAGS_PROPERTY: SymbolFlags = 1 << 2; // Property or enum member
pub const SYMBOL_FLAGS_ENUM_MEMBER: SymbolFlags = 1 << 3; // Enum member
pub const SYMBOL_FLAGS_FUNCTION: SymbolFlags = 1 << 4; // Function
pub const SYMBOL_FLAGS_CLASS: SymbolFlags = 1 << 5; // Class
pub const SYMBOL_FLAGS_INTERFACE: SymbolFlags = 1 << 6; // Interface
pub const SYMBOL_FLAGS_CONST_ENUM: SymbolFlags = 1 << 7; // Const enum
pub const SYMBOL_FLAGS_REGULAR_ENUM: SymbolFlags = 1 << 8; // Enum
pub const SYMBOL_FLAGS_VALUE_MODULE: SymbolFlags = 1 << 9; // Instantiated module
pub const SYMBOL_FLAGS_NAMESPACE_MODULE: SymbolFlags = 1 << 10; // Uninstantiated module
pub const SYMBOL_FLAGS_TYPE_LITERAL: SymbolFlags = 1 << 11; // Type Literal or mapped type
pub const SYMBOL_FLAGS_OBJECT_LITERAL: SymbolFlags = 1 << 12; // Object Literal
pub const SYMBOL_FLAGS_METHOD: SymbolFlags = 1 << 13; // Method
pub const SYMBOL_FLAGS_CONSTRUCTOR: SymbolFlags = 1 << 14; // Constructor
pub const SYMBOL_FLAGS_GET_ACCESSOR: SymbolFlags = 1 << 15; // Get accessor
pub const SYMBOL_FLAGS_SET_ACCESSOR: SymbolFlags = 1 << 16; // Set accessor
pub const SYMBOL_FLAGS_SIGNATURE: SymbolFlags = 1 << 17; // Call, construct, or index signature
pub const SYMBOL_FLAGS_TYPE_PARAMETER: SymbolFlags = 1 << 18; // Type parameter
pub const SYMBOL_FLAGS_TYPE_ALIAS: SymbolFlags = 1 << 19; // Type alias
pub const SYMBOL_FLAGS_EXPORT_VALUE: SymbolFlags = 1 << 20; // Exported value marker (see comment in declareModuleMember in binder)
pub const SYMBOL_FLAGS_ALIAS: SymbolFlags = 1 << 21; // An alias for another symbol (see comment in isAliasSymbolDeclaration in checker)
pub const SYMBOL_FLAGS_PROTOTYPE: SymbolFlags = 1 << 22; // Prototype property (no source representation)
pub const SYMBOL_FLAGS_EXPORT_STAR: SymbolFlags = 1 << 23; // Export * declaration
pub const SYMBOL_FLAGS_OPTIONAL: SymbolFlags = 1 << 24; // Optional property
pub const SYMBOL_FLAGS_TRANSIENT: SymbolFlags = 1 << 25; // Transient symbol (created during type check)
pub const SYMBOL_FLAGS_ASSIGNMENT: SymbolFlags = 1 << 26; // Assignment to property on function acting as declaration (eg `func.prop = 1`)
pub const SYMBOL_FLAGS_MODULE_EXPORTS: SymbolFlags = 1 << 27; // Symbol for CommonJS `module` of `module.exports`
pub const SYMBOL_FLAGS_CONST_ENUM_ONLY_MODULE: SymbolFlags = 1 << 28; // Module contains only const enums or other modules with only const enums
pub const SYMBOL_FLAGS_REPLACEABLE_BY_METHOD: SymbolFlags = 1 << 29;
pub const SYMBOL_FLAGS_GLOBAL_LOOKUP: SymbolFlags = 1 << 30; // Flag to signal this is a global lookup
pub const SYMBOL_FLAGS_ALL: SymbolFlags = (1 << 30) - 1; // All flags except SymbolFlagsGlobalLookup

pub const SYMBOL_FLAGS_ENUM: SymbolFlags = SYMBOL_FLAGS_REGULAR_ENUM | SYMBOL_FLAGS_CONST_ENUM;
pub const SYMBOL_FLAGS_VARIABLE: SymbolFlags =
    SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE | SYMBOL_FLAGS_BLOCK_SCOPED_VARIABLE;
pub const SYMBOL_FLAGS_VALUE: SymbolFlags = SYMBOL_FLAGS_VARIABLE
    | SYMBOL_FLAGS_PROPERTY
    | SYMBOL_FLAGS_ENUM_MEMBER
    | SYMBOL_FLAGS_OBJECT_LITERAL
    | SYMBOL_FLAGS_FUNCTION
    | SYMBOL_FLAGS_CLASS
    | SYMBOL_FLAGS_ENUM
    | SYMBOL_FLAGS_VALUE_MODULE
    | SYMBOL_FLAGS_METHOD
    | SYMBOL_FLAGS_GET_ACCESSOR
    | SYMBOL_FLAGS_SET_ACCESSOR;
pub const SYMBOL_FLAGS_TYPE: SymbolFlags = SYMBOL_FLAGS_CLASS
    | SYMBOL_FLAGS_INTERFACE
    | SYMBOL_FLAGS_ENUM
    | SYMBOL_FLAGS_ENUM_MEMBER
    | SYMBOL_FLAGS_TYPE_LITERAL
    | SYMBOL_FLAGS_TYPE_PARAMETER
    | SYMBOL_FLAGS_TYPE_ALIAS;
pub const SYMBOL_FLAGS_NAMESPACE: SymbolFlags =
    SYMBOL_FLAGS_VALUE_MODULE | SYMBOL_FLAGS_NAMESPACE_MODULE | SYMBOL_FLAGS_ENUM;
pub const SYMBOL_FLAGS_MODULE: SymbolFlags =
    SYMBOL_FLAGS_VALUE_MODULE | SYMBOL_FLAGS_NAMESPACE_MODULE;
pub const SYMBOL_FLAGS_ACCESSOR: SymbolFlags =
    SYMBOL_FLAGS_GET_ACCESSOR | SYMBOL_FLAGS_SET_ACCESSOR;

// Variables can be redeclared, but can not redeclare a block-scoped declaration with the
// same name, or any other value that is not a variable, e.g. ValueModule or Class
pub const SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE_EXCLUDES: SymbolFlags =
    SYMBOL_FLAGS_VALUE & !SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE;

// Block-scoped declarations are not allowed to be re-declared
// they can not merge with anything in the value space
pub const SYMBOL_FLAGS_BLOCK_SCOPED_VARIABLE_EXCLUDES: SymbolFlags = SYMBOL_FLAGS_VALUE;

pub const SYMBOL_FLAGS_PARAMETER_EXCLUDES: SymbolFlags = SYMBOL_FLAGS_VALUE;
pub const SYMBOL_FLAGS_PROPERTY_EXCLUDES: SymbolFlags =
    SYMBOL_FLAGS_VALUE & !(SYMBOL_FLAGS_PROPERTY | SYMBOL_FLAGS_ACCESSOR);
pub const SYMBOL_FLAGS_ENUM_MEMBER_EXCLUDES: SymbolFlags = SYMBOL_FLAGS_VALUE | SYMBOL_FLAGS_TYPE;
pub const SYMBOL_FLAGS_FUNCTION_EXCLUDES: SymbolFlags =
    SYMBOL_FLAGS_VALUE & !(SYMBOL_FLAGS_FUNCTION | SYMBOL_FLAGS_VALUE_MODULE | SYMBOL_FLAGS_CLASS);
pub const SYMBOL_FLAGS_CLASS_EXCLUDES: SymbolFlags = (SYMBOL_FLAGS_VALUE | SYMBOL_FLAGS_TYPE)
    & !(SYMBOL_FLAGS_VALUE_MODULE | SYMBOL_FLAGS_INTERFACE | SYMBOL_FLAGS_FUNCTION); // class-interface mergability done in checker.ts
pub const SYMBOL_FLAGS_INTERFACE_EXCLUDES: SymbolFlags =
    SYMBOL_FLAGS_TYPE & !(SYMBOL_FLAGS_INTERFACE | SYMBOL_FLAGS_CLASS);
pub const SYMBOL_FLAGS_REGULAR_ENUM_EXCLUDES: SymbolFlags = (SYMBOL_FLAGS_VALUE
    | SYMBOL_FLAGS_TYPE)
    & !(SYMBOL_FLAGS_REGULAR_ENUM | SYMBOL_FLAGS_VALUE_MODULE); // regular enums merge only with regular enums and modules
pub const SYMBOL_FLAGS_CONST_ENUM_EXCLUDES: SymbolFlags =
    (SYMBOL_FLAGS_VALUE | SYMBOL_FLAGS_TYPE) & !SYMBOL_FLAGS_CONST_ENUM; // const enums merge only with const enums
pub const SYMBOL_FLAGS_VALUE_MODULE_EXCLUDES: SymbolFlags = SYMBOL_FLAGS_VALUE
    & !(SYMBOL_FLAGS_FUNCTION
        | SYMBOL_FLAGS_CLASS
        | SYMBOL_FLAGS_REGULAR_ENUM
        | SYMBOL_FLAGS_VALUE_MODULE);
pub const SYMBOL_FLAGS_NAMESPACE_MODULE_EXCLUDES: SymbolFlags = SYMBOL_FLAGS_NONE;
pub const SYMBOL_FLAGS_METHOD_EXCLUDES: SymbolFlags = SYMBOL_FLAGS_VALUE & !SYMBOL_FLAGS_METHOD;
pub const SYMBOL_FLAGS_GET_ACCESSOR_EXCLUDES: SymbolFlags =
    SYMBOL_FLAGS_VALUE & !(SYMBOL_FLAGS_SET_ACCESSOR | SYMBOL_FLAGS_PROPERTY);
pub const SYMBOL_FLAGS_SET_ACCESSOR_EXCLUDES: SymbolFlags =
    SYMBOL_FLAGS_VALUE & !(SYMBOL_FLAGS_GET_ACCESSOR | SYMBOL_FLAGS_PROPERTY);
pub const SYMBOL_FLAGS_ACCESSOR_EXCLUDES: SymbolFlags = SYMBOL_FLAGS_VALUE & !SYMBOL_FLAGS_PROPERTY;
pub const SYMBOL_FLAGS_TYPE_PARAMETER_EXCLUDES: SymbolFlags =
    SYMBOL_FLAGS_TYPE & !SYMBOL_FLAGS_TYPE_PARAMETER;
pub const SYMBOL_FLAGS_TYPE_ALIAS_EXCLUDES: SymbolFlags = SYMBOL_FLAGS_TYPE;
pub const SYMBOL_FLAGS_ALIAS_EXCLUDES: SymbolFlags = SYMBOL_FLAGS_ALIAS;
pub const SYMBOL_FLAGS_MODULE_MEMBER: SymbolFlags = SYMBOL_FLAGS_VARIABLE
    | SYMBOL_FLAGS_FUNCTION
    | SYMBOL_FLAGS_CLASS
    | SYMBOL_FLAGS_INTERFACE
    | SYMBOL_FLAGS_ENUM
    | SYMBOL_FLAGS_MODULE
    | SYMBOL_FLAGS_TYPE_ALIAS
    | SYMBOL_FLAGS_ALIAS;
pub const SYMBOL_FLAGS_EXPORT_HAS_LOCAL: SymbolFlags =
    SYMBOL_FLAGS_FUNCTION | SYMBOL_FLAGS_CLASS | SYMBOL_FLAGS_ENUM | SYMBOL_FLAGS_VALUE_MODULE;
pub const SYMBOL_FLAGS_BLOCK_SCOPED: SymbolFlags =
    SYMBOL_FLAGS_BLOCK_SCOPED_VARIABLE | SYMBOL_FLAGS_CLASS | SYMBOL_FLAGS_ENUM;
pub const SYMBOL_FLAGS_PROPERTY_OR_ACCESSOR: SymbolFlags =
    SYMBOL_FLAGS_PROPERTY | SYMBOL_FLAGS_ACCESSOR;
pub const SYMBOL_FLAGS_CLASS_MEMBER: SymbolFlags =
    SYMBOL_FLAGS_METHOD | SYMBOL_FLAGS_ACCESSOR | SYMBOL_FLAGS_PROPERTY;
pub const SYMBOL_FLAGS_EXPORT_SUPPORTS_DEFAULT_MODIFIER: SymbolFlags =
    SYMBOL_FLAGS_CLASS | SYMBOL_FLAGS_FUNCTION | SYMBOL_FLAGS_INTERFACE;
pub const SYMBOL_FLAGS_EXPORT_DOES_NOT_SUPPORT_DEFAULT_MODIFIER: SymbolFlags =
    !SYMBOL_FLAGS_EXPORT_SUPPORTS_DEFAULT_MODIFIER;
// The set of things we consider semantically classifiable.  Used to speed up the LS during
// classification.
pub const SYMBOL_FLAGS_CLASSIFIABLE: SymbolFlags = SYMBOL_FLAGS_CLASS
    | SYMBOL_FLAGS_ENUM
    | SYMBOL_FLAGS_TYPE_ALIAS
    | SYMBOL_FLAGS_INTERFACE
    | SYMBOL_FLAGS_TYPE_PARAMETER
    | SYMBOL_FLAGS_MODULE
    | SYMBOL_FLAGS_ALIAS;
pub const SYMBOL_FLAGS_LATE_BINDING_CONTAINER: SymbolFlags = SYMBOL_FLAGS_CLASS
    | SYMBOL_FLAGS_INTERFACE
    | SYMBOL_FLAGS_TYPE_LITERAL
    | SYMBOL_FLAGS_OBJECT_LITERAL
    | SYMBOL_FLAGS_FUNCTION;

pub trait SymbolFlagsExt {
    fn contains(self, other: SymbolFlags) -> bool;
    fn intersects(self, other: SymbolFlags) -> bool;
    fn is_empty(self) -> bool;
}

impl SymbolFlagsExt for SymbolFlags {
    fn contains(self, other: SymbolFlags) -> bool {
        self & other == other
    }

    fn intersects(self, other: SymbolFlags) -> bool {
        self & other != 0
    }

    fn is_empty(self) -> bool {
        self == SYMBOL_FLAGS_NONE
    }
}
