use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct ModuleKind(pub i32);

impl ModuleKind {
    #[allow(non_upper_case_globals)]
    pub const None: ModuleKind = ModuleKind(0);
    #[allow(non_upper_case_globals)]
    pub const CommonJS: ModuleKind = ModuleKind(1);
    // Deprecated: Do not use outside of options parsing and validation.
    #[allow(non_upper_case_globals)]
    pub const AMD: ModuleKind = ModuleKind(2);
    // Deprecated: Do not use outside of options parsing and validation.
    #[allow(non_upper_case_globals)]
    pub const UMD: ModuleKind = ModuleKind(3);
    // Deprecated: Do not use outside of options parsing and validation.
    #[allow(non_upper_case_globals)]
    pub const System: ModuleKind = ModuleKind(4);
    // NOTE: ES module kinds should be contiguous to more easily check whether a module kind is *any* ES module kind.
    //       Non-ES module kinds should not come between ES2015 (the earliest ES module kind) and ESNext (the last ES
    //       module kind).
    #[allow(non_upper_case_globals)]
    pub const ES2015: ModuleKind = ModuleKind(5);
    #[allow(non_upper_case_globals)]
    pub const ES2020: ModuleKind = ModuleKind(6);
    #[allow(non_upper_case_globals)]
    pub const ES2022: ModuleKind = ModuleKind(7);
    #[allow(non_upper_case_globals)]
    pub const ESNext: ModuleKind = ModuleKind(99);
    // Node16+ is an amalgam of commonjs (albeit updated) and es2022+, and represents a distinct module system from es2020/esnext
    #[allow(non_upper_case_globals)]
    pub const Node16: ModuleKind = ModuleKind(100);
    #[allow(non_upper_case_globals)]
    pub const Node18: ModuleKind = ModuleKind(101);
    #[allow(non_upper_case_globals)]
    pub const Node20: ModuleKind = ModuleKind(102);
    #[allow(non_upper_case_globals)]
    pub const NodeNext: ModuleKind = ModuleKind(199);
    // Emit as written
    #[allow(non_upper_case_globals)]
    pub const Preserve: ModuleKind = ModuleKind(200);

    #[allow(non_upper_case_globals)]
    pub const CommonJs: ModuleKind = Self::CommonJS;
    #[allow(non_upper_case_globals)]
    pub const Amd: ModuleKind = Self::AMD;
    #[allow(non_upper_case_globals)]
    pub const Umd: ModuleKind = Self::UMD;
    #[allow(non_upper_case_globals)]
    pub const Es2015: ModuleKind = Self::ES2015;
    #[allow(non_upper_case_globals)]
    pub const Es2020: ModuleKind = Self::ES2020;
    #[allow(non_upper_case_globals)]
    pub const Es2022: ModuleKind = Self::ES2022;
    #[allow(non_upper_case_globals)]
    pub const EsNext: ModuleKind = Self::ESNext;

    pub fn is_non_node_esm(self) -> bool {
        self >= ModuleKind::ES2015 && self <= ModuleKind::ESNext
    }

    pub fn is_non_node_es_m(self) -> bool {
        self.is_non_node_esm()
    }

    pub fn supports_import_attributes(self) -> bool {
        ModuleKind::Node18 <= self && self <= ModuleKind::NodeNext
            || self == ModuleKind::Preserve
            || self == ModuleKind::ESNext
    }
}

pub type ResolutionMode = ModuleKind; // ModuleKindNone | ModuleKindCommonJS | ModuleKindESNext

pub const RESOLUTION_MODE_NONE: ResolutionMode = ModuleKind::None;
pub const RESOLUTION_MODE_COMMON_JS: ResolutionMode = ModuleKind::CommonJS;
pub const RESOLUTION_MODE_ESM: ResolutionMode = ModuleKind::ESNext;
