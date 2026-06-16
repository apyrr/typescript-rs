#![forbid(unsafe_code)]

#[expect(
    dead_code,
    reason = "ported AST storage API is ahead of current callers"
)]
mod arena;
pub mod ast;
mod ast_generated;
#[expect(
    dead_code,
    reason = "ported AST mutation adapters are ahead of current callers"
)]
mod ast_mutation;
pub mod checkflags;
#[expect(
    dead_code,
    reason = "ported diagnostic API is ahead of current callers"
)]
pub mod diagnostic;
pub mod flags_aliases_generated;
pub mod flow;
pub mod functionflags;
pub mod ids;
pub mod kind_aliases_generated;
pub mod kind_generated;
pub mod kind_stringer_generated;
pub mod modifierflags;
pub mod nodeflags;
pub mod parseoptions;
pub mod positionmap;
#[cfg(test)]
mod positionmap_test;
pub mod precedence;
#[expect(
    dead_code,
    reason = "ported subtree fact helpers are ahead of current callers"
)]
pub mod subtreefacts;
#[expect(
    dead_code,
    reason = "ported symbol storage API is ahead of current callers"
)]
pub mod symbol;
pub mod symbolflags;
pub mod tokenflags;

pub use arena::*;
pub use ast::*;
pub use ast_generated::*;
pub use checkflags::*;
pub use diagnostic::*;
pub use flags_aliases_generated::*;
pub use flow::*;
pub use functionflags::*;
pub use ids::*;
pub use kind_aliases_generated::*;
pub use kind_generated::*;
pub use modifierflags::*;
pub use nodeflags::*;
pub use parseoptions::*;
pub use positionmap::*;
pub use precedence::*;
pub use subtreefacts::*;
pub use symbol::*;
pub use symbolflags::*;
pub use tokenflags::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticMeaning(pub i32);

impl SemanticMeaning {
    pub const NONE: Self = Self(0);
    pub const VALUE: Self = Self(1 << 0);
    pub const TYPE: Self = Self(1 << 1);
    pub const NAMESPACE: Self = Self(1 << 2);
    pub const ALL: Self = Self(Self::VALUE.0 | Self::TYPE.0 | Self::NAMESPACE.0);
}

impl std::ops::BitOr for SemanticMeaning {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OuterExpressionKinds(pub i16);

impl OuterExpressionKinds {
    pub const PARENTHESES: Self = Self(1 << 0);
    pub const TYPE_ASSERTIONS: Self = Self(1 << 1);
    pub const NON_NULL_ASSERTIONS: Self = Self(1 << 2);
    pub const PARTIALLY_EMITTED_EXPRESSIONS: Self = Self(1 << 3);
    pub const EXPRESSIONS_WITH_TYPE_ARGUMENTS: Self = Self(1 << 4);
    pub const SATISFIES: Self = Self(1 << 5);
    pub const ASSERTIONS: Self =
        Self(Self::TYPE_ASSERTIONS.0 | Self::NON_NULL_ASSERTIONS.0 | Self::SATISFIES.0);
    pub const ALL: Self = Self(
        Self::PARENTHESES.0
            | Self::TYPE_ASSERTIONS.0
            | Self::NON_NULL_ASSERTIONS.0
            | Self::PARTIALLY_EMITTED_EXPRESSIONS.0
            | Self::EXPRESSIONS_WITH_TYPE_ARGUMENTS.0
            | Self::SATISFIES.0,
    );

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

impl std::ops::BitOr for OuterExpressionKinds {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

pub const TOKEN_FLAGS_NONE: TokenFlags = TokenFlags::NONE;
pub const TOKEN_FLAGS_SINGLE_QUOTE: TokenFlags = TokenFlags::SINGLE_QUOTE;
pub const MODIFIER_FLAGS_EXPORT: ModifierFlags = ModifierFlags::EXPORT;
pub const MODIFIER_FLAGS_DEFAULT: ModifierFlags = ModifierFlags::DEFAULT;
pub const MODIFIER_FLAGS_EXPORT_DEFAULT: ModifierFlags = ModifierFlags::EXPORT_DEFAULT;
pub const SUBTREE_CONTAINS_DECORATORS: SubtreeFacts = SubtreeFacts::CONTAINS_DECORATORS;
pub const SUBTREE_CONTAINS_JSX: SubtreeFacts = SubtreeFacts::CONTAINS_JSX;
pub const SEMANTIC_MEANING_ALL: SemanticMeaning = SemanticMeaning::ALL;
pub const OEK_PARENTHESES: OuterExpressionKinds = OuterExpressionKinds::PARENTHESES;
pub const OEK_SATISFIES: OuterExpressionKinds = OuterExpressionKinds::SATISFIES;
pub const OEK_ASSERTIONS: OuterExpressionKinds = OuterExpressionKinds::ASSERTIONS;
pub const OEK_ALL: OuterExpressionKinds = OuterExpressionKinds::ALL;

pub type EntityName = Node;

pub fn is_keyword_kind(kind: Kind) -> bool {
    kind >= Kind::FirstKeyword && kind <= Kind::LastKeyword
}

pub fn is_punctuation_kind(kind: Kind) -> bool {
    kind >= Kind::FirstPunctuation && kind <= Kind::LastPunctuation
}
