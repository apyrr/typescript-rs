#![forbid(unsafe_code)]
mod cache;
mod expected;
#[cfg(test)]
mod expected_test;
mod exportsorimports;
#[cfg(test)]
mod exportsorimports_test;
mod jsonvalue;
#[cfg(test)]
mod jsonvalue_test;
mod packagejson;
#[cfg(test)]
mod packagejson_test;
mod validated;

pub use cache::{InfoCache, InfoCacheEntry, PackageJson, VersionPaths};
pub use expected::{AnyJson, Expected, ExpectedJsonValue, expected_of};
pub use exportsorimports::ExportsOrImports;
pub use jsonvalue::{JsonValue, JsonValueType};
pub use packagejson::{DependencyFields, Fields, HeaderFields, PathFields, parse};
pub use validated::TypeValidatedField;

pub fn new_info_cache(current_directory: String, use_case_sensitive_file_names: bool) -> InfoCache {
    InfoCache::new(current_directory, use_case_sensitive_file_names)
}
