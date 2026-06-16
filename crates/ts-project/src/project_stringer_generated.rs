use std::fmt;

use crate::project::Kind;

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Inferred => "Inferred",
            Kind::Configured => "Configured",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let idx = *self as i32;
        if !(0..=1).contains(&idx) {
            return write!(f, "Kind({idx})");
        }
        f.write_str(self.as_str())
    }
}

// PORT STATUS
//   source:     internal/project/project_stringer_generated.go (25 lines)
//   confidence: medium
//   todos:      none tracked
//   notes:      generated String behavior attached to the real Kind enum in project.rs,
//               including the Go invalid-value fallback formatting
