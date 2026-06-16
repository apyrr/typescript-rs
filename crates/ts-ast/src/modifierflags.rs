use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, Not};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModifierFlags(pub u32);

impl ModifierFlags {
    pub const NONE: ModifierFlags = ModifierFlags(0);
    #[allow(non_upper_case_globals)]
    pub const None: ModifierFlags = Self::NONE;
    #[allow(non_upper_case_globals)]
    pub const Public: ModifierFlags = Self::PUBLIC;
    #[allow(non_upper_case_globals)]
    pub const Private: ModifierFlags = Self::PRIVATE;
    #[allow(non_upper_case_globals)]
    pub const Protected: ModifierFlags = Self::PROTECTED;
    #[allow(non_upper_case_globals)]
    pub const Readonly: ModifierFlags = Self::READONLY;
    #[allow(non_upper_case_globals)]
    pub const Override: ModifierFlags = Self::OVERRIDE;
    #[allow(non_upper_case_globals)]
    pub const Export: ModifierFlags = Self::EXPORT;
    #[allow(non_upper_case_globals)]
    pub const Abstract: ModifierFlags = Self::ABSTRACT;
    #[allow(non_upper_case_globals)]
    pub const Ambient: ModifierFlags = Self::AMBIENT;
    #[allow(non_upper_case_globals)]
    pub const Static: ModifierFlags = Self::STATIC;
    #[allow(non_upper_case_globals)]
    pub const Accessor: ModifierFlags = Self::ACCESSOR;
    #[allow(non_upper_case_globals)]
    pub const Async: ModifierFlags = Self::ASYNC;
    #[allow(non_upper_case_globals)]
    pub const Default: ModifierFlags = Self::DEFAULT;
    #[allow(non_upper_case_globals)]
    pub const Const: ModifierFlags = Self::CONST;
    #[allow(non_upper_case_globals)]
    pub const In: ModifierFlags = Self::IN;
    #[allow(non_upper_case_globals)]
    pub const Out: ModifierFlags = Self::OUT;
    #[allow(non_upper_case_globals)]
    pub const Decorator: ModifierFlags = Self::DECORATOR;
    #[allow(non_upper_case_globals)]
    pub const ExportDefault: ModifierFlags = Self::EXPORT_DEFAULT;
    #[allow(non_upper_case_globals)]
    pub const Modifier: ModifierFlags = Self::MODIFIER;
    #[allow(non_upper_case_globals)]
    pub const AccessibilityModifier: ModifierFlags = Self::ACCESSIBILITY_MODIFIER;
    #[allow(non_upper_case_globals)]
    pub const ParameterPropertyModifier: ModifierFlags = Self::PARAMETER_PROPERTY_MODIFIER;

    // Syntactic modifiers
    pub const PUBLIC: ModifierFlags = ModifierFlags(1 << 0); // Property/Method
    pub const PRIVATE: ModifierFlags = ModifierFlags(1 << 1); // Property/Method
    pub const PROTECTED: ModifierFlags = ModifierFlags(1 << 2); // Property/Method
    pub const READONLY: ModifierFlags = ModifierFlags(1 << 3); // Property/Method
    pub const OVERRIDE: ModifierFlags = ModifierFlags(1 << 4); // Override method
    // Syntactic-only modifiers
    pub const EXPORT: ModifierFlags = ModifierFlags(1 << 5); // Declarations
    pub const ABSTRACT: ModifierFlags = ModifierFlags(1 << 6); // Class/Method/ConstructSignature
    pub const AMBIENT: ModifierFlags = ModifierFlags(1 << 7); // Declarations (declare keyword)
    pub const STATIC: ModifierFlags = ModifierFlags(1 << 8); // Property/Method
    pub const ACCESSOR: ModifierFlags = ModifierFlags(1 << 9); // Property
    pub const ASYNC: ModifierFlags = ModifierFlags(1 << 10); // Property/Method/Function
    pub const DEFAULT: ModifierFlags = ModifierFlags(1 << 11); // Function/Class (export default declaration)
    pub const CONST: ModifierFlags = ModifierFlags(1 << 12); // Const enum
    pub const IN: ModifierFlags = ModifierFlags(1 << 13); // Contravariance modifier
    pub const OUT: ModifierFlags = ModifierFlags(1 << 14); // Covariance modifier
    pub const DECORATOR: ModifierFlags = ModifierFlags(1 << 15); // Contains a decorator
    pub const HAS_COMPUTED_FLAGS: ModifierFlags = ModifierFlags(1 << 29); // Modifier flags have been computed

    pub const SYNTACTIC_VISIBILITY_MODIFIERS: ModifierFlags = ModifierFlags(
        Self::PUBLIC.0 | Self::PRIVATE.0 | Self::PROTECTED.0 | Self::READONLY.0 | Self::OVERRIDE.0,
    );
    pub const SYNTACTIC_ONLY_MODIFIERS: ModifierFlags = ModifierFlags(
        Self::EXPORT.0
            | Self::AMBIENT.0
            | Self::ABSTRACT.0
            | Self::STATIC.0
            | Self::ACCESSOR.0
            | Self::ASYNC.0
            | Self::DEFAULT.0
            | Self::CONST.0
            | Self::IN.0
            | Self::OUT.0
            | Self::DECORATOR.0,
    );
    pub const SYNTACTIC_MODIFIERS: ModifierFlags =
        ModifierFlags(Self::SYNTACTIC_VISIBILITY_MODIFIERS.0 | Self::SYNTACTIC_ONLY_MODIFIERS.0);
    pub const NON_CACHE_ONLY_MODIFIERS: ModifierFlags =
        ModifierFlags(Self::SYNTACTIC_VISIBILITY_MODIFIERS.0 | Self::SYNTACTIC_ONLY_MODIFIERS.0);

    pub const ACCESSIBILITY_MODIFIER: ModifierFlags =
        ModifierFlags(Self::PUBLIC.0 | Self::PRIVATE.0 | Self::PROTECTED.0);
    // Accessibility modifiers and 'readonly' can be attached to a parameter in a constructor to make it a property.
    pub const PARAMETER_PROPERTY_MODIFIER: ModifierFlags =
        ModifierFlags(Self::ACCESSIBILITY_MODIFIER.0 | Self::READONLY.0 | Self::OVERRIDE.0);
    pub const NON_PUBLIC_ACCESSIBILITY_MODIFIER: ModifierFlags =
        ModifierFlags(Self::PRIVATE.0 | Self::PROTECTED.0);

    pub const TYPE_SCRIPT_MODIFIER: ModifierFlags = ModifierFlags(
        Self::AMBIENT.0
            | Self::PUBLIC.0
            | Self::PRIVATE.0
            | Self::PROTECTED.0
            | Self::READONLY.0
            | Self::ABSTRACT.0
            | Self::CONST.0
            | Self::OVERRIDE.0
            | Self::IN.0
            | Self::OUT.0,
    );
    pub const EXPORT_DEFAULT: ModifierFlags = ModifierFlags(Self::EXPORT.0 | Self::DEFAULT.0);
    pub const ALL: ModifierFlags = ModifierFlags(
        Self::EXPORT.0
            | Self::AMBIENT.0
            | Self::PUBLIC.0
            | Self::PRIVATE.0
            | Self::PROTECTED.0
            | Self::STATIC.0
            | Self::READONLY.0
            | Self::ABSTRACT.0
            | Self::ACCESSOR.0
            | Self::ASYNC.0
            | Self::DEFAULT.0
            | Self::CONST.0
            | Self::OVERRIDE.0
            | Self::IN.0
            | Self::OUT.0
            | Self::DECORATOR.0,
    );
    pub const MODIFIER: ModifierFlags = ModifierFlags(Self::ALL.0 & !Self::DECORATOR.0);
    pub const JAVA_SCRIPT: ModifierFlags = ModifierFlags(
        Self::EXPORT.0 | Self::STATIC.0 | Self::ACCESSOR.0 | Self::ASYNC.0 | Self::DEFAULT.0,
    );
    pub const JAVASCRIPT: ModifierFlags = Self::JAVA_SCRIPT;

    pub fn contains(self, other: ModifierFlags) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn intersects(self, other: ModifierFlags) -> bool {
        self.0 & other.0 != 0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl BitOr for ModifierFlags {
    type Output = ModifierFlags;

    fn bitor(self, rhs: ModifierFlags) -> ModifierFlags {
        ModifierFlags(self.0 | rhs.0)
    }
}

impl BitOrAssign for ModifierFlags {
    fn bitor_assign(&mut self, rhs: ModifierFlags) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for ModifierFlags {
    type Output = ModifierFlags;

    fn bitand(self, rhs: ModifierFlags) -> ModifierFlags {
        ModifierFlags(self.0 & rhs.0)
    }
}

impl BitAndAssign for ModifierFlags {
    fn bitand_assign(&mut self, rhs: ModifierFlags) {
        self.0 &= rhs.0;
    }
}

impl BitXor for ModifierFlags {
    type Output = ModifierFlags;

    fn bitxor(self, rhs: ModifierFlags) -> ModifierFlags {
        ModifierFlags(self.0 ^ rhs.0)
    }
}

impl Not for ModifierFlags {
    type Output = ModifierFlags;

    fn not(self) -> ModifierFlags {
        ModifierFlags(!self.0)
    }
}

impl PartialEq<i32> for ModifierFlags {
    fn eq(&self, other: &i32) -> bool {
        self.0 == *other as u32
    }
}

impl PartialEq<ModifierFlags> for i32 {
    fn eq(&self, other: &ModifierFlags) -> bool {
        *self as u32 == other.0
    }
}

impl PartialEq<u32> for ModifierFlags {
    fn eq(&self, other: &u32) -> bool {
        self.0 == *other
    }
}

impl PartialEq<ModifierFlags> for u32 {
    fn eq(&self, other: &ModifierFlags) -> bool {
        *self == other.0
    }
}
