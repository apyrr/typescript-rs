use std::collections::HashMap;

use base64::{Engine as _, engine::general_purpose};
use ts_core::UTF16Offset;
use ts_tspath as tspath;

use crate::{NameIndex, SourceIndex};

pub const SOURCE_INDEX_NOT_SET: SourceIndex = -1;
pub const NAME_INDEX_NOT_SET: NameIndex = -1;
pub const NOT_SET: i32 = -1;
pub const NOT_SET_UTF16: UTF16Offset = -1;

pub struct Generator {
    pub path_options: tspath::ComparePathsOptions,
    pub file: String,
    pub source_root: String,
    pub sources_directory_path: String,
    pub sources: Vec<String>,
    pub source_map_sources: Vec<String>,
    pub source_to_source_index_map: HashMap<String, SourceIndex>,
    pub sources_content: Vec<Option<String>>,
    pub names: Vec<String>,
    pub name_to_name_index_map: HashMap<String, NameIndex>,
    pub mappings: String,
    last_generated_line: i32,
    last_generated_character: UTF16Offset,
    last_source_index: SourceIndex,
    last_source_line: i32,
    last_source_character: UTF16Offset,
    last_name_index: NameIndex,
    has_last: bool,
    pending_generated_line: i32,
    pending_generated_character: UTF16Offset,
    pending_source_index: SourceIndex,
    pending_source_line: i32,
    pending_source_character: UTF16Offset,
    pending_name_index: NameIndex,
    has_pending: bool,
    has_pending_source: bool,
    has_pending_name: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct RawSourceMap {
    #[serde(default)]
    pub version: i32,
    #[serde(default)]
    pub file: String,
    #[serde(default, rename = "sourceRoot")]
    pub source_root: String,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub names: Vec<String>,
    #[serde(default)]
    pub mappings: String,
    #[serde(
        default,
        rename = "sourcesContent",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub sources_content: Vec<Option<String>>,
}

pub fn new_generator(
    file: String,
    source_root: String,
    sources_directory_path: String,
    options: tspath::ComparePathsOptions,
) -> Generator {
    Generator {
        path_options: options,
        file,
        source_root,
        sources_directory_path,
        sources: Vec::new(),
        source_map_sources: Vec::new(),
        source_to_source_index_map: HashMap::new(),
        sources_content: Vec::new(),
        names: Vec::new(),
        name_to_name_index_map: HashMap::new(),
        mappings: String::new(),
        last_generated_line: 0,
        last_generated_character: 0,
        last_source_index: 0,
        last_source_line: 0,
        last_source_character: 0,
        last_name_index: 0,
        has_last: false,
        pending_generated_line: 0,
        pending_generated_character: 0,
        pending_source_index: 0,
        pending_source_line: 0,
        pending_source_character: 0,
        pending_name_index: 0,
        has_pending: false,
        has_pending_source: false,
        has_pending_name: false,
    }
}

impl Generator {
    pub fn sources(&self) -> &[String] {
        &self.sources
    }

    pub fn add_source(&mut self, file_name: String) -> SourceIndex {
        let source = tspath::get_relative_path_to_directory_or_url(
            &self.sources_directory_path,
            &file_name,
            true, /*isAbsolutePathAnUrl*/
            &self.path_options,
        );
        if let Some(source_index) = self.source_to_source_index_map.get(&source) {
            return *source_index;
        }
        let source_index = self.source_map_sources.len() as SourceIndex;
        self.source_map_sources.push(source.clone());
        self.sources.push(file_name);
        self.source_to_source_index_map.insert(source, source_index);
        source_index
    }

    pub fn set_source_content(
        &mut self,
        source_index: SourceIndex,
        content: String,
    ) -> Result<(), String> {
        if source_index < 0 || source_index as usize >= self.source_map_sources.len() {
            return Err("sourceIndex is out of range".to_string());
        }
        while self.sources_content.len() <= source_index as usize {
            self.sources_content.push(None);
        }
        self.sources_content[source_index as usize] = Some(content);
        Ok(())
    }

    pub fn add_name(&mut self, name: String) -> NameIndex {
        if let Some(name_index) = self.name_to_name_index_map.get(&name) {
            return *name_index;
        }
        let name_index = self.names.len() as NameIndex;
        self.names.push(name.clone());
        self.name_to_name_index_map.insert(name, name_index);
        name_index
    }

    fn is_new_generated_position(
        &self,
        generated_line: i32,
        generated_character: UTF16Offset,
    ) -> bool {
        !self.has_pending
            || self.pending_generated_line != generated_line
            || self.pending_generated_character != generated_character
    }

    fn is_backtracking_source_position(
        &self,
        source_index: SourceIndex,
        source_line: i32,
        source_character: UTF16Offset,
    ) -> bool {
        source_index != SOURCE_INDEX_NOT_SET
            && source_line != NOT_SET
            && source_character != NOT_SET_UTF16
            && self.pending_source_index == source_index
            && (self.pending_source_line > source_line
                || self.pending_source_line == source_line
                    && self.pending_source_character > source_character)
    }

    fn should_commit_mapping(&self) -> bool {
        self.has_pending
            && (!self.has_last
                || self.last_generated_line != self.pending_generated_line
                || self.last_generated_character != self.pending_generated_character
                || self.last_source_index != self.pending_source_index
                || self.last_source_line != self.pending_source_line
                || self.last_source_character != self.pending_source_character
                || self.last_name_index != self.pending_name_index)
    }

    fn append_mapping_char_code(&mut self, char_code: char) {
        self.mappings.push(char_code);
    }

    fn append_base64_vlq(&mut self, mut in_value: i32) {
        if in_value < 0 {
            in_value = ((-in_value) << 1) + 1;
        } else {
            in_value <<= 1;
        }
        loop {
            let mut current_digit = in_value & 31;
            in_value >>= 5;
            if in_value > 0 {
                current_digit |= 32;
            }
            self.append_mapping_char_code(base64_format_encode(current_digit));
            if in_value <= 0 {
                break;
            }
        }
    }

    fn commit_pending_mapping(&mut self) {
        if !self.should_commit_mapping() {
            return;
        }
        if self.last_generated_line < self.pending_generated_line {
            while self.last_generated_line < self.pending_generated_line {
                self.append_mapping_char_code(';');
                self.last_generated_line += 1;
            }
            self.last_generated_character = 0;
        } else {
            if self.last_generated_line != self.pending_generated_line {
                panic!("generatedLine cannot backtrack");
            }
            if self.has_last {
                self.append_mapping_char_code(',');
            }
        }

        self.append_base64_vlq(self.pending_generated_character - self.last_generated_character);
        self.last_generated_character = self.pending_generated_character;

        if self.has_pending_source {
            self.append_base64_vlq(self.pending_source_index - self.last_source_index);
            self.last_source_index = self.pending_source_index;
            self.append_base64_vlq(self.pending_source_line - self.last_source_line);
            self.last_source_line = self.pending_source_line;
            self.append_base64_vlq(self.pending_source_character - self.last_source_character);
            self.last_source_character = self.pending_source_character;
            if self.has_pending_name {
                self.append_base64_vlq(self.pending_name_index - self.last_name_index);
                self.last_name_index = self.pending_name_index;
            }
        }

        self.has_last = true;
    }

    fn add_mapping(
        &mut self,
        generated_line: i32,
        generated_character: UTF16Offset,
        source_index: SourceIndex,
        source_line: i32,
        source_character: UTF16Offset,
        name_index: NameIndex,
    ) {
        if self.is_new_generated_position(generated_line, generated_character)
            || self.is_backtracking_source_position(source_index, source_line, source_character)
        {
            self.commit_pending_mapping();
            self.pending_generated_line = generated_line;
            self.pending_generated_character = generated_character;
            self.has_pending_source = false;
            self.has_pending_name = false;
            self.has_pending = true;
        }

        if source_index != SOURCE_INDEX_NOT_SET
            && source_line != NOT_SET
            && source_character != NOT_SET_UTF16
        {
            self.pending_source_index = source_index;
            self.pending_source_line = source_line;
            self.pending_source_character = source_character;
            self.has_pending_source = true;
            if name_index != NAME_INDEX_NOT_SET {
                self.pending_name_index = name_index;
                self.has_pending_name = true;
            }
        }
    }

    pub fn add_generated_mapping(
        &mut self,
        generated_line: i32,
        generated_character: UTF16Offset,
    ) -> Result<(), String> {
        if generated_line < self.pending_generated_line {
            return Err("generatedLine cannot backtrack".to_string());
        }
        if generated_character < 0 {
            return Err("generatedCharacter cannot be negative".to_string());
        }
        self.add_mapping(
            generated_line,
            generated_character,
            SOURCE_INDEX_NOT_SET,
            NOT_SET,
            NOT_SET_UTF16,
            NAME_INDEX_NOT_SET,
        );
        Ok(())
    }

    pub fn add_source_mapping(
        &mut self,
        generated_line: i32,
        generated_character: UTF16Offset,
        source_index: SourceIndex,
        source_line: i32,
        source_character: UTF16Offset,
    ) -> Result<(), String> {
        self.validate_source_mapping_args(
            generated_line,
            generated_character,
            source_index,
            source_line,
            source_character,
        )?;
        self.add_mapping(
            generated_line,
            generated_character,
            source_index,
            source_line,
            source_character,
            NAME_INDEX_NOT_SET,
        );
        Ok(())
    }

    pub fn add_named_source_mapping(
        &mut self,
        generated_line: i32,
        generated_character: UTF16Offset,
        source_index: SourceIndex,
        source_line: i32,
        source_character: UTF16Offset,
        name_index: NameIndex,
    ) -> Result<(), String> {
        self.validate_source_mapping_args(
            generated_line,
            generated_character,
            source_index,
            source_line,
            source_character,
        )?;
        if name_index < 0 || name_index as usize >= self.names.len() {
            return Err("nameIndex is out of range".to_string());
        }
        self.add_mapping(
            generated_line,
            generated_character,
            source_index,
            source_line,
            source_character,
            name_index,
        );
        Ok(())
    }

    fn validate_source_mapping_args(
        &self,
        generated_line: i32,
        generated_character: UTF16Offset,
        source_index: SourceIndex,
        source_line: i32,
        source_character: UTF16Offset,
    ) -> Result<(), String> {
        if generated_line < self.pending_generated_line {
            return Err("generatedLine cannot backtrack".to_string());
        }
        if generated_character < 0 {
            return Err("generatedCharacter cannot be negative".to_string());
        }
        if source_index < 0 || source_index as usize >= self.source_map_sources.len() {
            return Err("sourceIndex is out of range".to_string());
        }
        if source_line < 0 {
            return Err("sourceLine cannot be negative".to_string());
        }
        if source_character < 0 {
            return Err("sourceCharacter cannot be negative".to_string());
        }
        Ok(())
    }

    pub fn raw_source_map(&mut self) -> RawSourceMap {
        self.commit_pending_mapping();
        RawSourceMap {
            version: 3,
            file: self.file.clone(),
            source_root: self.source_root.clone(),
            sources: self.source_map_sources.clone(),
            names: self.names.clone(),
            mappings: self.mappings.clone(),
            sources_content: self.sources_content.clone(),
        }
    }

    pub fn bytes(&mut self) -> Vec<u8> {
        serde_json::to_vec(&self.raw_source_map()).unwrap_or_else(|err| panic!("{err}"))
    }

    pub fn string(&mut self) -> String {
        String::from_utf8(self.bytes()).unwrap_or_else(|err| panic!("{err}"))
    }

    pub fn base64_data_url(&mut self) -> String {
        const PREFIX: &str = "data:application/json;base64,";
        let data = self.bytes();
        let mut result = String::with_capacity(PREFIX.len() + data.len().div_ceil(3) * 4);
        result.push_str(PREFIX);
        general_purpose::STANDARD.encode_string(data, &mut result);
        result
    }
}

pub fn base64_format_encode(value: i32) -> char {
    match value {
        0..=25 => (b'A' + value as u8) as char,
        26..=51 => (b'a' + (value - 26) as u8) as char,
        52..=61 => (b'0' + (value - 52) as u8) as char,
        62 => '+',
        63 => '/',
        _ => panic!("not a base64 value"),
    }
}
