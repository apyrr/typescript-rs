use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct ScriptTarget(pub i32);

impl ScriptTarget {
    #[allow(non_upper_case_globals)]
    pub const None: ScriptTarget = ScriptTarget(0);
    // Deprecated: Do not use outside of options parsing and validation.
    #[allow(non_upper_case_globals)]
    pub const ES5: ScriptTarget = ScriptTarget(1);
    #[allow(non_upper_case_globals)]
    pub const ES2015: ScriptTarget = ScriptTarget(2);
    #[allow(non_upper_case_globals)]
    pub const ES2016: ScriptTarget = ScriptTarget(3);
    #[allow(non_upper_case_globals)]
    pub const ES2017: ScriptTarget = ScriptTarget(4);
    #[allow(non_upper_case_globals)]
    pub const ES2018: ScriptTarget = ScriptTarget(5);
    #[allow(non_upper_case_globals)]
    pub const ES2019: ScriptTarget = ScriptTarget(6);
    #[allow(non_upper_case_globals)]
    pub const ES2020: ScriptTarget = ScriptTarget(7);
    #[allow(non_upper_case_globals)]
    pub const ES2021: ScriptTarget = ScriptTarget(8);
    #[allow(non_upper_case_globals)]
    pub const ES2022: ScriptTarget = ScriptTarget(9);
    #[allow(non_upper_case_globals)]
    pub const ES2023: ScriptTarget = ScriptTarget(10);
    #[allow(non_upper_case_globals)]
    pub const ES2024: ScriptTarget = ScriptTarget(11);
    #[allow(non_upper_case_globals)]
    pub const ES2025: ScriptTarget = ScriptTarget(12);
    #[allow(non_upper_case_globals)]
    pub const ESNext: ScriptTarget = ScriptTarget(99);
    #[allow(non_upper_case_globals)]
    pub const JSON: ScriptTarget = ScriptTarget(100);
    #[allow(non_upper_case_globals)]
    pub const Latest: ScriptTarget = Self::ESNext;
    #[allow(non_upper_case_globals)]
    pub const LatestStandard: ScriptTarget = Self::ES2025;

    #[allow(non_upper_case_globals)]
    pub const Es5: ScriptTarget = Self::ES5;
    #[allow(non_upper_case_globals)]
    pub const Es2015: ScriptTarget = Self::ES2015;
    #[allow(non_upper_case_globals)]
    pub const Es2016: ScriptTarget = Self::ES2016;
    #[allow(non_upper_case_globals)]
    pub const Es2017: ScriptTarget = Self::ES2017;
    #[allow(non_upper_case_globals)]
    pub const Es2018: ScriptTarget = Self::ES2018;
    #[allow(non_upper_case_globals)]
    pub const Es2019: ScriptTarget = Self::ES2019;
    #[allow(non_upper_case_globals)]
    pub const Es2020: ScriptTarget = Self::ES2020;
    #[allow(non_upper_case_globals)]
    pub const Es2021: ScriptTarget = Self::ES2021;
    #[allow(non_upper_case_globals)]
    pub const Es2022: ScriptTarget = Self::ES2022;
    #[allow(non_upper_case_globals)]
    pub const Es2023: ScriptTarget = Self::ES2023;
    #[allow(non_upper_case_globals)]
    pub const Es2024: ScriptTarget = Self::ES2024;
    #[allow(non_upper_case_globals)]
    pub const Es2025: ScriptTarget = Self::ES2025;
    #[allow(non_upper_case_globals)]
    pub const EsNext: ScriptTarget = Self::ESNext;
    #[allow(non_upper_case_globals)]
    pub const Json: ScriptTarget = Self::JSON;
}

pub const SCRIPT_TARGET_LATEST: ScriptTarget = ScriptTarget::Latest;
pub const SCRIPT_TARGET_LATEST_STANDARD: ScriptTarget = ScriptTarget::LatestStandard;
