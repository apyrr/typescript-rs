// Package diagnostics contains generated localizable diagnostic messages.

//go:generate go run generate.go -diagnostics ./diagnostics_generated.go -loc ./loc_generated.go -locdir ./loc
//go:generate go tool golang.org/x/tools/cmd/stringer -type=Category -output=stringer_generated.go
//go:generate npx dprint fmt diagnostics_generated.go loc_generated.go stringer_generated.go

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Category(pub i32);

impl Category {
    #[allow(non_upper_case_globals)]
    pub const Warning: Category = Category(0);
    #[allow(non_upper_case_globals)]
    pub const Error: Category = Category(1);
    #[allow(non_upper_case_globals)]
    pub const Suggestion: Category = Category(2);
    #[allow(non_upper_case_globals)]
    pub const Message: Category = Category(3);
}

impl Category {
    pub fn name(self) -> &'static str {
        match self {
            Category::Warning => "warning",
            Category::Error => "error",
            Category::Suggestion => "suggestion",
            Category::Message => "message",
            _ => panic!("Unhandled diagnostic category"),
        }
    }
}

impl fmt::Debug for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Serialize for Category {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i32(self.0)
    }
}

impl<'de> Deserialize<'de> for Category {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Category(i32::deserialize(deserializer)?))
    }
}

pub type Key = String;

#[derive(Clone)]
pub struct Message {
    pub(crate) code: i32,
    pub(crate) category: Category,
    pub(crate) key: Key,
    pub(crate) text: String,
    pub(crate) reports_unnecessary: bool,
    pub(crate) elided_in_compatibility_pyramid: bool,
    pub(crate) reports_deprecated: bool,
}

impl Message {
    pub fn new(code: i32, category: Category, key: Key, text: String) -> Self {
        Self {
            code,
            category,
            key,
            text,
            reports_unnecessary: false,
            elided_in_compatibility_pyramid: false,
            reports_deprecated: false,
        }
    }

    pub fn code(&self) -> i32 {
        self.code
    }

    pub fn category(&self) -> Category {
        self.category
    }

    pub fn key(&self) -> &Key {
        &self.key
    }

    pub fn reports_unnecessary(&self) -> bool {
        self.reports_unnecessary
    }

    pub fn elided_in_compatibility_pyramid(&self) -> bool {
        self.elided_in_compatibility_pyramid
    }

    pub fn reports_deprecated(&self) -> bool {
        self.reports_deprecated
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}
