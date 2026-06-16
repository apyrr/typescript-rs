use std::error::Error;
use std::fmt;

use ts_core::UTF16Offset;

pub type SourceIndex = i32;
pub type NameIndex = i32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mapping {
    pub generated_line: i32,
    pub generated_character: UTF16Offset,
    pub source_index: SourceIndex,
    pub source_line: i32,
    pub source_character: UTF16Offset,
    pub name_index: NameIndex,
}

impl Mapping {
    pub fn equals(&self, other: &Mapping) -> bool {
        self == other
    }

    pub fn is_source_mapping(&self) -> bool {
        self.source_index != MISSING_SOURCE
            && self.source_line != MISSING_LINE_OR_COLUMN
            && self.source_character != MISSING_UTF16_COLUMN
    }
}

pub const MISSING_SOURCE: SourceIndex = -1;
pub const MISSING_NAME: NameIndex = -1;
pub const MISSING_LINE_OR_COLUMN: i32 = -1;
pub const MISSING_UTF16_COLUMN: UTF16Offset = -1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodeMappingsError(String);

impl fmt::Display for DecodeMappingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for DecodeMappingsError {}

pub struct MappingsDecoder {
    mappings: String,
    done: bool,
    pos: usize,
    generated_line: i32,
    generated_character: UTF16Offset,
    source_index: SourceIndex,
    source_line: i32,
    source_character: UTF16Offset,
    name_index: NameIndex,
    error: Option<DecodeMappingsError>,
}

pub fn decode_mappings(mappings: String) -> MappingsDecoder {
    MappingsDecoder {
        mappings,
        done: false,
        pos: 0,
        generated_line: 0,
        generated_character: 0,
        source_index: 0,
        source_line: 0,
        source_character: 0,
        name_index: 0,
        error: None,
    }
}

impl MappingsDecoder {
    pub fn mappings_string(&self) -> &str {
        &self.mappings
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn error(&self) -> Option<&DecodeMappingsError> {
        self.error.as_ref()
    }

    pub fn state(&self) -> Mapping {
        self.capture_mapping(true, true)
    }

    fn next_mapping(&mut self) -> Option<Mapping> {
        while !self.done && self.pos < self.mappings.len() {
            let ch = self.mappings.as_bytes()[self.pos];
            if ch == b';' {
                self.generated_line += 1;
                self.generated_character = 0;
                self.pos += 1;
                continue;
            }
            if ch == b',' {
                self.pos += 1;
                continue;
            }

            let mut has_source = false;
            let mut has_name = false;
            self.generated_character += self.base64_vlq_format_decode() as UTF16Offset;
            if self.has_reported_error() {
                return self.stop_iterating();
            }
            if self.generated_character < 0 {
                return self.set_error_and_stop_iterating("Invalid generatedCharacter found");
            }

            if !self.is_source_mapping_segment_end() {
                has_source = true;
                self.source_index += self.base64_vlq_format_decode() as SourceIndex;
                if self.has_reported_error() {
                    return self.stop_iterating();
                }
                if self.source_index < 0 {
                    return self.set_error_and_stop_iterating("Invalid sourceIndex found");
                }
                if self.is_source_mapping_segment_end() {
                    return self.set_error_and_stop_iterating(
                        "Unsupported Format: No entries after sourceIndex",
                    );
                }

                self.source_line += self.base64_vlq_format_decode();
                if self.has_reported_error() {
                    return self.stop_iterating();
                }
                if self.source_line < 0 {
                    return self.set_error_and_stop_iterating("Invalid sourceLine found");
                }
                if self.is_source_mapping_segment_end() {
                    return self.set_error_and_stop_iterating(
                        "Unsupported Format: No entries after sourceLine",
                    );
                }

                self.source_character += self.base64_vlq_format_decode() as UTF16Offset;
                if self.has_reported_error() {
                    return self.stop_iterating();
                }
                if self.source_character < 0 {
                    return self.set_error_and_stop_iterating("Invalid sourceCharacter found");
                }

                if !self.is_source_mapping_segment_end() {
                    has_name = true;
                    self.name_index += self.base64_vlq_format_decode() as NameIndex;
                    if self.has_reported_error() {
                        return self.stop_iterating();
                    }
                    if self.name_index < 0 {
                        return self.set_error_and_stop_iterating("Invalid nameIndex found");
                    }
                    if !self.is_source_mapping_segment_end() {
                        return self.set_error_and_stop_iterating(
                            "Unsupported Error Format: Entries after nameIndex",
                        );
                    }
                }
            }

            return Some(self.capture_mapping(has_source, has_name));
        }
        self.stop_iterating()
    }

    fn capture_mapping(&self, has_source: bool, has_name: bool) -> Mapping {
        Mapping {
            generated_line: self.generated_line,
            generated_character: self.generated_character,
            source_index: if has_source {
                self.source_index
            } else {
                MISSING_SOURCE
            },
            source_line: if has_source {
                self.source_line
            } else {
                MISSING_LINE_OR_COLUMN
            },
            source_character: if has_source {
                self.source_character
            } else {
                MISSING_UTF16_COLUMN
            },
            name_index: if has_name {
                self.name_index
            } else {
                MISSING_NAME
            },
        }
    }

    fn stop_iterating(&mut self) -> Option<Mapping> {
        self.done = true;
        None
    }

    fn set_error(&mut self, err: &str) {
        self.error = Some(DecodeMappingsError(err.to_string()));
    }

    fn set_error_and_stop_iterating(&mut self, err: &str) -> Option<Mapping> {
        self.set_error(err);
        self.stop_iterating()
    }

    fn has_reported_error(&self) -> bool {
        self.error.is_some()
    }

    fn is_source_mapping_segment_end(&self) -> bool {
        self.pos == self.mappings.len()
            || self.mappings.as_bytes()[self.pos] == b','
            || self.mappings.as_bytes()[self.pos] == b';'
    }

    fn base64_vlq_format_decode(&mut self) -> i32 {
        let mut more_digits = true;
        let mut shift_count = 0;
        let mut value = 0;
        while more_digits {
            if self.pos >= self.mappings.len() {
                self.set_error("Error in decoding base64VLQFormatDecode, past the mapping string");
                return -1;
            }
            let current_byte = base64_format_decode(self.mappings.as_bytes()[self.pos]);
            self.pos += 1;
            if current_byte == -1 {
                self.set_error("Invalid character in VLQ");
                return -1;
            }
            more_digits = (current_byte & 32) != 0;
            value |= (current_byte & 31) << shift_count;
            shift_count += 5;
        }

        if (value & 1) == 0 {
            value >> 1
        } else {
            -(value >> 1)
        }
    }
}

impl Iterator for MappingsDecoder {
    type Item = Mapping;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_mapping()
    }
}

pub fn base64_format_decode(ch: u8) -> i32 {
    match ch {
        b'A'..=b'Z' => (ch - b'A') as i32,
        b'a'..=b'z' => (ch - b'a' + 26) as i32,
        b'0'..=b'9' => (ch - b'0' + 52) as i32,
        b'+' => 62,
        b'/' => 63,
        _ => -1,
    }
}
