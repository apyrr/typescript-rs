use crate::autoimport;
use crate::lsconv;
pub use crate::lsutil::UserPreferences;
use ts_sourcemap as sourcemap;

pub trait Host {
    fn use_case_sensitive_file_names(&self) -> bool;
    fn read_file(&self, path: &str) -> (String, bool);
    fn converters(&self) -> lsconv::Converters;
    fn get_preferences(&self, active_file: &str) -> UserPreferences;
    fn get_ecma_line_info(&self, file_name: &str) -> Option<sourcemap::ECMALineInfo>;
    fn auto_import_registry(&self) -> Option<autoimport::Registry>;

    // Used for module specifier completions.
    // ! Do not use for anything else, as this violates the principle that
    // the host is a snapshot-in-time.
    fn read_directory(
        &self,
        current_dir: &str,
        path: &str,
        extensions: &[String],
        excludes: &[String],
        includes: &[String],
        depth: i32,
    ) -> Vec<String>;
    fn get_directories(&self, path: &str) -> Vec<String>;
    fn directory_exists(&self, path: &str) -> bool;
    fn file_exists(&self, path: &str) -> bool;
}
