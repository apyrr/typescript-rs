// ParseFlags
use std::ops::{BitAnd, BitOr, BitOrAssign};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseFlags(pub u32);

impl ParseFlags {
    pub const NONE: ParseFlags = ParseFlags(0);
    pub const YIELD: ParseFlags = ParseFlags(1 << 0);
    pub const AWAIT: ParseFlags = ParseFlags(1 << 1);
    pub const TYPE: ParseFlags = ParseFlags(1 << 2);
    pub const IGNORE_MISSING_OPEN_BRACE: ParseFlags = ParseFlags(1 << 4);
    pub const ARROW_FUNCTION: ParseFlags = ParseFlags(0);
}

impl BitOr for ParseFlags {
    type Output = ParseFlags;

    fn bitor(self, rhs: ParseFlags) -> ParseFlags {
        ParseFlags(self.0 | rhs.0)
    }
}

impl BitOrAssign for ParseFlags {
    fn bitor_assign(&mut self, rhs: ParseFlags) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for ParseFlags {
    type Output = ParseFlags;

    fn bitand(self, rhs: ParseFlags) -> ParseFlags {
        ParseFlags(self.0 & rhs.0)
    }
}
