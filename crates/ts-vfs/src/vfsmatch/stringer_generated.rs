use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i8)]
#[derive(Default)]
pub enum Usage {
    #[default]
    Files = 0,
    Directories = 1,
    Exclude = 2,
}

impl Usage {
    pub fn as_str(self) -> &'static str {
        match self {
            Usage::Files => "Files",
            Usage::Directories => "Directories",
            Usage::Exclude => "Exclude",
        }
    }
}

impl fmt::Display for Usage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// PORT STATUS
//   source:     internal/vfs/vfsmatch/stringer_generated.go (26 lines)
//   confidence: high
//   todos:      none tracked
//   notes:      generated String behavior ported for Rust enum variants.
