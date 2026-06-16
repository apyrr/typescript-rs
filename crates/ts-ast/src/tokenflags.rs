use std::ops::{BitAnd, BitOr, BitOrAssign};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TokenFlags(pub i32);

impl TokenFlags {
    pub const NONE: TokenFlags = TokenFlags(0);
    #[allow(non_upper_case_globals)]
    pub const None: TokenFlags = Self::NONE;
    pub const PRECEDING_LINE_BREAK: TokenFlags = TokenFlags(1 << 0);
    pub const UNTERMINATED: TokenFlags = TokenFlags(1 << 2);
    pub const EXTENDED_UNICODE_ESCAPE: TokenFlags = TokenFlags(1 << 3); // e.g. `\u{10ffff}`
    pub const SCIENTIFIC: TokenFlags = TokenFlags(1 << 4); // e.g. `10e2`
    pub const OCTAL: TokenFlags = TokenFlags(1 << 5); // e.g. `0777`
    pub const HEX_SPECIFIER: TokenFlags = TokenFlags(1 << 6); // e.g. `0x00000000`
    pub const BINARY_SPECIFIER: TokenFlags = TokenFlags(1 << 7); // e.g. `0b0110010000000000`
    pub const OCTAL_SPECIFIER: TokenFlags = TokenFlags(1 << 8); // e.g. `0o777`
    pub const CONTAINS_SEPARATOR: TokenFlags = TokenFlags(1 << 9); // e.g. `0b1100_0101`
    pub const UNICODE_ESCAPE: TokenFlags = TokenFlags(1 << 10); // e.g. `\u00a0`
    pub const CONTAINS_INVALID_ESCAPE: TokenFlags = TokenFlags(1 << 11); // e.g. `\uhello`
    pub const HEX_ESCAPE: TokenFlags = TokenFlags(1 << 12); // e.g. `\xa0`
    pub const CONTAINS_LEADING_ZERO: TokenFlags = TokenFlags(1 << 13); // e.g. `0888`
    pub const CONTAINS_INVALID_SEPARATOR: TokenFlags = TokenFlags(1 << 14); // e.g. `0_1`
    pub const SINGLE_QUOTE: TokenFlags = TokenFlags(1 << 16); // e.g. `'abc'`
    #[allow(non_upper_case_globals)]
    pub const SingleQuote: TokenFlags = Self::SINGLE_QUOTE;
    pub const BINARY_OR_OCTAL_SPECIFIER: TokenFlags =
        TokenFlags(Self::BINARY_SPECIFIER.0 | Self::OCTAL_SPECIFIER.0);
    pub const WITH_SPECIFIER: TokenFlags =
        TokenFlags(Self::HEX_SPECIFIER.0 | Self::BINARY_OR_OCTAL_SPECIFIER.0);
    pub const STRING_LITERAL_FLAGS: TokenFlags = TokenFlags(
        Self::UNTERMINATED.0
            | Self::HEX_ESCAPE.0
            | Self::UNICODE_ESCAPE.0
            | Self::EXTENDED_UNICODE_ESCAPE.0
            | Self::CONTAINS_INVALID_ESCAPE.0
            | Self::SINGLE_QUOTE.0,
    );
    pub const NUMERIC_LITERAL_FLAGS: TokenFlags = TokenFlags(
        Self::SCIENTIFIC.0
            | Self::OCTAL.0
            | Self::CONTAINS_LEADING_ZERO.0
            | Self::WITH_SPECIFIER.0
            | Self::CONTAINS_SEPARATOR.0
            | Self::CONTAINS_INVALID_SEPARATOR.0,
    );
    pub const TEMPLATE_LITERAL_LIKE_FLAGS: TokenFlags = TokenFlags(
        Self::UNTERMINATED.0
            | Self::HEX_ESCAPE.0
            | Self::UNICODE_ESCAPE.0
            | Self::EXTENDED_UNICODE_ESCAPE.0
            | Self::CONTAINS_INVALID_ESCAPE.0,
    );
    pub const REGULAR_EXPRESSION_LITERAL_FLAGS: TokenFlags = Self::UNTERMINATED;
    pub const IS_INVALID: TokenFlags = TokenFlags(
        Self::OCTAL.0
            | Self::CONTAINS_LEADING_ZERO.0
            | Self::CONTAINS_INVALID_SEPARATOR.0
            | Self::CONTAINS_INVALID_ESCAPE.0,
    );

    pub fn contains(self, other: TokenFlags) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn intersects(self, other: TokenFlags) -> bool {
        self.0 & other.0 != 0
    }

    pub fn bits(self) -> u32 {
        self.0 as u32
    }
}

impl BitOr for TokenFlags {
    type Output = TokenFlags;

    fn bitor(self, rhs: TokenFlags) -> TokenFlags {
        TokenFlags(self.0 | rhs.0)
    }
}

impl BitOrAssign for TokenFlags {
    fn bitor_assign(&mut self, rhs: TokenFlags) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for TokenFlags {
    type Output = TokenFlags;

    fn bitand(self, rhs: TokenFlags) -> TokenFlags {
        TokenFlags(self.0 & rhs.0)
    }
}
