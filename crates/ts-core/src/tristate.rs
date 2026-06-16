use std::fmt;

use serde::de::{self, IgnoredAny, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

//go:generate go tool golang.org/x/tools/cmd/stringer -type=Tristate -output=tristate_stringer_generated.go
//go:generate npx dprint fmt tristate_stringer_generated.go
// PORT NOTE: Rust stringer equivalent lives in tristate_stringer_generated.rs.

// Tristate

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tristate(pub u8);

impl Tristate {
    #[allow(non_upper_case_globals)]
    pub const TSUnknown: Tristate = Tristate(0);
    #[allow(non_upper_case_globals)]
    pub const TSFalse: Tristate = Tristate(1);
    #[allow(non_upper_case_globals)]
    pub const TSTrue: Tristate = Tristate(2);

    #[allow(non_upper_case_globals)]
    pub const Unknown: Tristate = Self::TSUnknown;
    #[allow(non_upper_case_globals)]
    pub const False: Tristate = Self::TSFalse;
    #[allow(non_upper_case_globals)]
    pub const True: Tristate = Self::TSTrue;
}

#[allow(non_upper_case_globals)]
pub const TSUnknown: Tristate = Tristate::TSUnknown;
#[allow(non_upper_case_globals)]
pub const TSFalse: Tristate = Tristate::TSFalse;
#[allow(non_upper_case_globals)]
pub const TSTrue: Tristate = Tristate::TSTrue;

pub const TS_UNKNOWN: Tristate = TSUnknown;
pub const TS_FALSE: Tristate = TSFalse;
pub const TS_TRUE: Tristate = TSTrue;

impl Default for Tristate {
    fn default() -> Self {
        Tristate::Unknown
    }
}

impl Tristate {
    pub fn is_true(self) -> bool {
        self == Tristate::True
    }

    pub fn is_true_or_unknown(self) -> bool {
        self == Tristate::True || self == Tristate::Unknown
    }

    pub fn is_false(self) -> bool {
        self == Tristate::False
    }

    pub fn is_false_or_unknown(self) -> bool {
        self == Tristate::False || self == Tristate::Unknown
    }

    pub fn is_unknown(&self) -> bool {
        *self == Tristate::Unknown
    }

    pub fn default_if_unknown(self, value: Tristate) -> Tristate {
        if self == Tristate::Unknown {
            return value;
        }
        self
    }
}

impl<'de> Deserialize<'de> for Tristate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TristateVisitor;

        impl<'de> Visitor<'de> for TristateVisitor {
            type Value = Tristate;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a JSON value for Tristate")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(bool_to_tristate(value))
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Tristate::Unknown)
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Tristate::Unknown)
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                deserializer.deserialize_any(self)
            }

            fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Tristate::Unknown)
            }

            fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Tristate::Unknown)
            }

            fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Tristate::Unknown)
            }

            fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Tristate::Unknown)
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                while seq.next_element::<IgnoredAny>()?.is_some() {}
                Ok(Tristate::Unknown)
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
                Ok(Tristate::Unknown)
            }
        }

        deserializer.deserialize_any(TristateVisitor)
    }
}

impl Serialize for Tristate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *self {
            Tristate::True => serializer.serialize_bool(true),
            Tristate::False => serializer.serialize_bool(false),
            _ => serializer.serialize_none(),
        }
    }
}

pub fn bool_to_tristate(b: bool) -> Tristate {
    if b {
        return Tristate::True;
    }
    Tristate::False
}
