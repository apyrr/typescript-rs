use serde_json::{Map, Value};

use crate::{JsonValue, JsonValueType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ObjectKind {
    #[default]
    Unknown,
    Subpaths,
    Conditions,
    Imports,
    Invalid,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExportsOrImports {
    pub json_value: JsonValue,
    object_kind: ObjectKind,
}

impl ExportsOrImports {
    pub fn from_json_value(value: Value) -> Self {
        Self {
            json_value: JsonValue::from_json_value(value),
            object_kind: ObjectKind::Unknown,
        }
    }

    pub fn is_present(&self) -> bool {
        self.json_value.is_present()
    }

    pub fn unmarshal_json(&mut self, data: &[u8]) -> serde_json::Result<()> {
        self.json_value.unmarshal_json(data)
    }

    pub fn as_object(&self) -> &Map<String, Value> {
        self.json_value.as_object()
    }

    pub fn as_array(&self) -> &[Value] {
        self.json_value.as_array()
    }

    pub fn is_subpaths(&mut self) -> bool {
        self.init_object_kind();
        self.object_kind == ObjectKind::Subpaths
    }

    pub fn is_imports(&mut self) -> bool {
        self.init_object_kind();
        self.object_kind == ObjectKind::Imports
    }

    pub fn is_conditions(&mut self) -> bool {
        self.init_object_kind();
        self.object_kind == ObjectKind::Conditions
    }

    fn init_object_kind(&mut self) {
        if self.object_kind != ObjectKind::Unknown || self.json_value.type_ != JsonValueType::Object
        {
            return;
        }
        if !self.as_object().is_empty() {
            let mut seen_dot = false;
            let mut seen_hash = false;
            let mut seen_other = false;
            for key in self.as_object().keys() {
                if let Some(first) = key.as_bytes().first().copied() {
                    seen_dot |= first == b'.';
                    seen_hash |= first == b'#';
                    seen_other |= first != b'.' && first != b'#';
                    if seen_other && (seen_dot || seen_hash) {
                        self.object_kind = ObjectKind::Invalid;
                        return;
                    }
                }
            }
            if seen_dot {
                self.object_kind = ObjectKind::Subpaths;
                return;
            }
            if seen_hash {
                self.object_kind = ObjectKind::Imports;
                return;
            }
        }
        self.object_kind = ObjectKind::Conditions;
    }
}
