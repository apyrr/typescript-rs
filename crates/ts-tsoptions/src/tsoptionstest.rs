use std::collections::BTreeMap;

use crate::parsedcommandline::ParsedCommandLine;
use crate::tsconfigparsing::{ParseConfigHost, parse_config};
use ts_tspath as tspath;
use ts_vfs::vfstest::{MapFs, from_map};

pub fn get_parsed_command_line(
    json_text: &str,
    files: BTreeMap<String, String>,
    current_directory: &str,
    use_case_sensitive_file_names: bool,
) -> ParsedCommandLine {
    let host = VfsParseConfigHost::new(files, current_directory, use_case_sensitive_file_names);
    let config_file_name = tspath::combine_paths(current_directory, &["tsconfig.json"]);
    parse_config(&config_file_name, json_text, &host)
}

pub fn parse_command_line(json: &str) -> ParsedCommandLine {
    get_parsed_command_line(json, BTreeMap::new(), "/", true)
}

pub fn fix_root(path: &str) -> String {
    let root_length = tspath::get_root_length(path);
    if root_length == 0 {
        return path.to_owned();
    }
    if path.len() == root_length {
        return ".".to_owned();
    }
    path[root_length..].to_owned()
}

#[derive(Clone)]
pub struct VfsParseConfigHost {
    pub vfs: MapFs,
    pub current_directory: String,
}

impl VfsParseConfigHost {
    pub fn new(
        files: BTreeMap<String, String>,
        current_directory: impl Into<String>,
        use_case_sensitive_file_names: bool,
    ) -> Self {
        Self {
            vfs: from_map(files, use_case_sensitive_file_names),
            current_directory: current_directory.into(),
        }
    }

    pub fn get_current_directory(&self) -> &str {
        &self.current_directory
    }
}

impl ParseConfigHost for VfsParseConfigHost {
    fn fs(&self) -> &dyn ts_vfs::Fs {
        &self.vfs
    }

    fn get_current_directory(&self) -> String {
        self.current_directory.clone()
    }
}
