use std::collections::{HashMap, HashSet};

use crate::{Expected, ExportsOrImports, JsonValue};

#[derive(Clone, Default)]
pub struct HeaderFields {
    pub name: Expected<String>,
    pub version: Expected<String>,
    pub type_: Expected<String>,
}

#[derive(Clone, Default)]
pub struct PathFields {
    pub tsconfig: Expected<String>,
    pub main: Expected<String>,
    pub types: Expected<String>,
    pub typings: Expected<String>,
    pub types_versions: JsonValue,
    pub imports: ExportsOrImports,
    pub exports: ExportsOrImports,
}

#[derive(Clone, Default)]
pub struct DependencyFields {
    pub dependencies: Expected<HashMap<String, String>>,
    pub dev_dependencies: Expected<HashMap<String, String>>,
    pub peer_dependencies: Expected<HashMap<String, String>>,
    pub optional_dependencies: Expected<HashMap<String, String>>,
}

impl DependencyFields {
    // HasDependency returns true if the package.json has a dependency with the given name
    // under any of the dependency fields (dependencies, devDependencies, peerDependencies,
    // optionalDependencies).
    pub fn has_dependency(&self, name: &str) -> bool {
        for deps in [
            &self.dependencies,
            &self.dev_dependencies,
            &self.peer_dependencies,
            &self.optional_dependencies,
        ] {
            if deps.get_value().1 && deps.value.contains_key(name) {
                return true;
            }
        }
        false
    }

    pub fn range_dependencies(&self, mut f: impl FnMut(&str, &str, &str) -> bool) {
        for (deps, field) in [
            (&self.dependencies, "dependencies"),
            (&self.dev_dependencies, "devDependencies"),
            (&self.peer_dependencies, "peerDependencies"),
            (&self.optional_dependencies, "optionalDependencies"),
        ] {
            if !deps.get_value().1 {
                continue;
            }
            for (name, version) in &deps.value {
                if !f(name, version, field) {
                    return;
                }
            }
        }
    }

    pub fn get_runtime_dependency_names(&self) -> HashSet<String> {
        let mut names = HashSet::new();
        for deps in [
            &self.dependencies,
            &self.peer_dependencies,
            &self.optional_dependencies,
        ] {
            if deps.get_value().1 {
                names.extend(deps.value.keys().cloned());
            }
        }
        names
    }
}

#[derive(Clone, Default)]
pub struct Fields {
    pub header_fields: HeaderFields,
    pub path_fields: PathFields,
    pub dependency_fields: DependencyFields,
}

pub fn parse(data: &[u8]) -> serde_json::Result<Fields> {
    let value: serde_json::Value = serde_json::from_slice(data)?;
    let mut fields = Fields::default();
    let Some(obj) = value.as_object() else {
        return Ok(fields);
    };

    if let Some(value) = obj.get("name") {
        fields
            .header_fields
            .name
            .unmarshal_json(value.to_string().as_bytes())
            .ok();
    }
    if let Some(value) = obj.get("version") {
        fields
            .header_fields
            .version
            .unmarshal_json(value.to_string().as_bytes())
            .ok();
    }
    if let Some(value) = obj.get("type") {
        fields
            .header_fields
            .type_
            .unmarshal_json(value.to_string().as_bytes())
            .ok();
    }
    if let Some(value) = obj.get("tsconfig") {
        fields
            .path_fields
            .tsconfig
            .unmarshal_json(value.to_string().as_bytes())
            .ok();
    }
    if let Some(value) = obj.get("main") {
        fields
            .path_fields
            .main
            .unmarshal_json(value.to_string().as_bytes())
            .ok();
    }
    if let Some(value) = obj.get("types") {
        fields
            .path_fields
            .types
            .unmarshal_json(value.to_string().as_bytes())
            .ok();
    }
    if let Some(value) = obj.get("typings") {
        fields
            .path_fields
            .typings
            .unmarshal_json(value.to_string().as_bytes())
            .ok();
    }
    if let Some(value) = obj.get("typesVersions") {
        fields
            .path_fields
            .types_versions
            .unmarshal_json(value.to_string().as_bytes())
            .ok();
    }
    if let Some(value) = obj.get("imports") {
        fields
            .path_fields
            .imports
            .unmarshal_json(value.to_string().as_bytes())
            .ok();
    }
    if let Some(value) = obj.get("exports") {
        fields
            .path_fields
            .exports
            .unmarshal_json(value.to_string().as_bytes())
            .ok();
    }

    for (json_name, field) in [
        ("dependencies", &mut fields.dependency_fields.dependencies),
        (
            "devDependencies",
            &mut fields.dependency_fields.dev_dependencies,
        ),
        (
            "peerDependencies",
            &mut fields.dependency_fields.peer_dependencies,
        ),
        (
            "optionalDependencies",
            &mut fields.dependency_fields.optional_dependencies,
        ),
    ] {
        if let Some(value) = obj.get(json_name) {
            field.unmarshal_json(value.to_string().as_bytes()).ok();
        }
    }
    Ok(fields)
}
