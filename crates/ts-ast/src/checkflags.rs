// CheckFlags

pub type CheckFlags = u32;

pub const CHECK_FLAGS_NONE: CheckFlags = 0;
pub const CHECK_FLAGS_INSTANTIATED: CheckFlags = 1 << 0; // Instantiated symbol
pub const CHECK_FLAGS_SYNTHETIC_PROPERTY: CheckFlags = 1 << 1; // Property in union or intersection type
pub const CHECK_FLAGS_SYNTHETIC_METHOD: CheckFlags = 1 << 2; // Method in union or intersection type
pub const CHECK_FLAGS_READONLY: CheckFlags = 1 << 3; // Readonly transient symbol
pub const CHECK_FLAGS_READ_PARTIAL: CheckFlags = 1 << 4; // Synthetic property present in some but not all constituents
pub const CHECK_FLAGS_WRITE_PARTIAL: CheckFlags = 1 << 5; // Synthetic property present in some but only satisfied by an index signature in others
pub const CHECK_FLAGS_HAS_NON_UNIFORM_TYPE: CheckFlags = 1 << 6; // Synthetic property with non-uniform type in constituents
pub const CHECK_FLAGS_HAS_LITERAL_TYPE: CheckFlags = 1 << 7; // Synthetic property with at least one literal type in constituents
pub const CHECK_FLAGS_CONTAINS_PUBLIC: CheckFlags = 1 << 8; // Synthetic property with public constituent(s)
pub const CHECK_FLAGS_CONTAINS_PROTECTED: CheckFlags = 1 << 9; // Synthetic property with protected constituent(s)
pub const CHECK_FLAGS_CONTAINS_PRIVATE: CheckFlags = 1 << 10; // Synthetic property with private constituent(s)
pub const CHECK_FLAGS_CONTAINS_STATIC: CheckFlags = 1 << 11; // Synthetic property with static constituent(s)
pub const CHECK_FLAGS_LATE: CheckFlags = 1 << 12; // Late-bound symbol for a computed property with a dynamic name
pub const CHECK_FLAGS_REVERSE_MAPPED: CheckFlags = 1 << 13; // Property of reverse-inferred homomorphic mapped type
pub const CHECK_FLAGS_OPTIONAL_PARAMETER: CheckFlags = 1 << 14; // Optional parameter
pub const CHECK_FLAGS_REST_PARAMETER: CheckFlags = 1 << 15; // Rest parameter
pub const CHECK_FLAGS_DEFERRED_TYPE: CheckFlags = 1 << 16; // Calculation of the type of this symbol is deferred due to processing costs, should be fetched with `getTypeOfSymbolWithDeferredType`
pub const CHECK_FLAGS_HAS_NEVER_TYPE: CheckFlags = 1 << 17; // Synthetic property with at least one never type in constituents
pub const CHECK_FLAGS_MAPPED: CheckFlags = 1 << 18; // Property of mapped type
pub const CHECK_FLAGS_STRIP_OPTIONAL: CheckFlags = 1 << 19; // Strip optionality in mapped property
pub const CHECK_FLAGS_UNRESOLVED: CheckFlags = 1 << 20; // Unresolved type alias symbol
pub const CHECK_FLAGS_IS_DISCRIMINANT_COMPUTED: CheckFlags = 1 << 21; // IsDiscriminant flags has been computed
pub const CHECK_FLAGS_IS_DISCRIMINANT: CheckFlags = 1 << 22; // Discriminant property
pub const CHECK_FLAGS_INDEX_SYMBOL: CheckFlags = 1 << 23; // Synthetic property created from index signature
pub const CHECK_FLAGS_SYNTHETIC: CheckFlags =
    CHECK_FLAGS_SYNTHETIC_PROPERTY | CHECK_FLAGS_SYNTHETIC_METHOD;
pub const CHECK_FLAGS_NON_UNIFORM_AND_LITERAL: CheckFlags =
    CHECK_FLAGS_HAS_NON_UNIFORM_TYPE | CHECK_FLAGS_HAS_LITERAL_TYPE;
pub const CHECK_FLAGS_PARTIAL: CheckFlags = CHECK_FLAGS_READ_PARTIAL | CHECK_FLAGS_WRITE_PARTIAL;
