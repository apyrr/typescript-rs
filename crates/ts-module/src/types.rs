use crate::{PackageId, ResolvedModule, ResolvedTypeReferenceDirective};

pub type NodeResolutionFeatures = u32;
pub const NODE_RESOLUTION_FEATURES_IMPORTS: NodeResolutionFeatures = 1 << 0;
pub const NODE_RESOLUTION_FEATURES_SELF_NAME: NodeResolutionFeatures = 1 << 1;
pub const NODE_RESOLUTION_FEATURES_EXPORTS: NodeResolutionFeatures = 1 << 2;
pub const NODE_RESOLUTION_FEATURES_EXPORTS_PATTERN_TRAILERS: NodeResolutionFeatures = 1 << 3;
// allowing `#/` root imports in package.json imports field
// not supported until mass adoption - https://github.com/nodejs/node/pull/60864
pub const NODE_RESOLUTION_FEATURES_IMPORTS_PATTERN_ROOT: NodeResolutionFeatures = 1 << 4;
pub const NODE_RESOLUTION_FEATURES_NONE: NodeResolutionFeatures = 0;
pub const NODE_RESOLUTION_FEATURES_ALL: NodeResolutionFeatures = NODE_RESOLUTION_FEATURES_IMPORTS
    | NODE_RESOLUTION_FEATURES_SELF_NAME
    | NODE_RESOLUTION_FEATURES_EXPORTS
    | NODE_RESOLUTION_FEATURES_EXPORTS_PATTERN_TRAILERS
    | NODE_RESOLUTION_FEATURES_IMPORTS_PATTERN_ROOT;
pub const NODE_RESOLUTION_FEATURES_NODE16_DEFAULT: NodeResolutionFeatures =
    NODE_RESOLUTION_FEATURES_IMPORTS
        | NODE_RESOLUTION_FEATURES_SELF_NAME
        | NODE_RESOLUTION_FEATURES_EXPORTS
        | NODE_RESOLUTION_FEATURES_EXPORTS_PATTERN_TRAILERS;
pub const NODE_RESOLUTION_FEATURES_NODE_NEXT_DEFAULT: NodeResolutionFeatures =
    NODE_RESOLUTION_FEATURES_ALL;
pub const NODE_RESOLUTION_FEATURES_BUNDLER_DEFAULT: NodeResolutionFeatures =
    NODE_RESOLUTION_FEATURES_IMPORTS
        | NODE_RESOLUTION_FEATURES_SELF_NAME
        | NODE_RESOLUTION_FEATURES_EXPORTS
        | NODE_RESOLUTION_FEATURES_EXPORTS_PATTERN_TRAILERS
        | NODE_RESOLUTION_FEATURES_IMPORTS_PATTERN_ROOT;

pub type Extensions = u32;
pub const EXTENSIONS_TYPESCRIPT: Extensions = 1 << 0;
pub const EXTENSIONS_JAVASCRIPT: Extensions = 1 << 1;
pub const EXTENSIONS_DECLARATION: Extensions = 1 << 2;
pub const EXTENSIONS_JSON: Extensions = 1 << 3;
pub const EXTENSIONS_IMPLEMENTATION_FILES: Extensions =
    EXTENSIONS_TYPESCRIPT | EXTENSIONS_JAVASCRIPT;

impl PackageId {
    pub fn package_name(&self) -> String {
        if self.sub_module_name.is_empty() {
            self.name.clone()
        } else {
            format!("{}/{}", self.name, self.sub_module_name)
        }
    }
}

impl std::fmt::Display for PackageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}@{}{}",
            self.package_name(),
            self.version,
            self.peer_dependencies
        )
    }
}

impl ResolvedModule {
    pub fn is_resolved(&self) -> bool {
        !self.resolved_file_name.is_empty()
    }
}

impl ResolvedTypeReferenceDirective {
    pub fn is_resolved(&self) -> bool {
        !self.resolved_file_name.is_empty()
    }
}

pub fn extensions_to_string(extensions: Extensions) -> String {
    let mut result = Vec::new();
    if extensions & EXTENSIONS_TYPESCRIPT != 0 {
        result.push("TypeScript");
    }
    if extensions & EXTENSIONS_JAVASCRIPT != 0 {
        result.push("JavaScript");
    }
    if extensions & EXTENSIONS_DECLARATION != 0 {
        result.push("Declaration");
    }
    if extensions & EXTENSIONS_JSON != 0 {
        result.push("JSON");
    }
    result.join(", ")
}

pub fn extensions_array(extensions: Extensions) -> Vec<&'static str> {
    let mut result = Vec::new();
    if extensions & EXTENSIONS_TYPESCRIPT != 0 {
        result.extend([".ts", ".tsx", ".mts", ".cts"]);
    }
    if extensions & EXTENSIONS_JAVASCRIPT != 0 {
        result.extend([".js", ".jsx", ".mjs", ".cjs"]);
    }
    if extensions & EXTENSIONS_DECLARATION != 0 {
        result.extend([".d.ts", ".d.mts", ".d.cts"]);
    }
    if extensions & EXTENSIONS_JSON != 0 {
        result.push(".json");
    }
    result
}
