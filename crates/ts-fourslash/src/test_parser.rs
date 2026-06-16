use std::collections::{BTreeMap, HashMap};

use crate::{Converters, Marker, RangeMarker, TestData, TestFileInfo, TestingT};
use ts_core as core;
use ts_json as json;
use ts_lsproto as lsproto;
use ts_testrunner::{ParseTestFilesOptions, parse_test_files_and_symlinks_with_options};
use ts_tspath as tspath;

impl RangeMarker {
    pub fn ls_pos(&self) -> lsproto::Position {
        self.ls_range.start
    }

    pub fn get_name(&self) -> Option<String> {
        self.marker.as_ref().and_then(|marker| marker.name.clone())
    }

    pub fn ls_location(&self) -> lsproto::Location {
        lsproto::Location {
            uri: ts_ls::file_name_to_document_uri(&self.file_name),
            range: self.ls_range,
        }
    }
}

impl Marker {
    pub fn ls_pos(&self) -> lsproto::Position {
        self.ls_position
    }

    pub fn get_name(&self) -> Option<String> {
        self.name.clone()
    }

    pub fn maker_with_symlink(&self, file_name: String) -> Marker {
        Marker {
            file_name,
            position: self.position,
            ls_position: self.ls_position,
            name: self.name.clone(),
            data: self.data.clone(),
        }
    }
}

impl TestFileInfo {
    // FileName implements lsconv.Script.
    pub fn file_name(&self) -> String {
        self.file_name.clone()
    }

    // Text implements lsconv.Script.
    pub fn text(&self) -> String {
        self.content.clone()
    }
}

impl TestData {
    pub fn is_state_baselining_enabled(&self) -> bool {
        is_state_baselining_enabled(&self.global_options)
    }
}

pub struct TestFileWithMarkers {
    pub file: TestFileInfo,
    pub markers: Vec<Marker>,
    pub ranges: Vec<RangeMarker>,
}

pub fn is_state_baselining_enabled(global_options: &BTreeMap<String, String>) -> bool {
    global_options
        .get("statebaseline")
        .is_some_and(|value| value == "true")
}

pub fn parse_test_data(_t: &mut TestingT, contents: &str, file_name: &str) -> TestData {
    // List of all the subfiles we've parsed out
    let mut files = Vec::new();

    let mut marker_positions: BTreeMap<String, Marker> = BTreeMap::new();
    let mut markers = Vec::new();
    let mut ranges = Vec::new();

    let (files_with_marker, symlinks, _, global_options, parse_error) =
        parse_test_files_and_symlinks_with_options(
            contents,
            file_name,
            |file_name, content, file_options| {
                parse_file_content(&file_name, &content, hash_map_to_btree_map(file_options))
            },
            ParseTestFilesOptions {
                allow_implicit_first_file: true,
            },
        );
    if let Some(err) = parse_error {
        panic!("Error parsing fourslash data: {err}");
    }

    let mut has_tsconfig = false;
    for file_with_markers in files_with_marker {
        files.push(file_with_markers.file.clone());
        has_tsconfig = has_tsconfig || is_config_file(&file_with_markers.file.file_name);

        markers.extend(file_with_markers.markers.clone());
        ranges.extend(file_with_markers.ranges.clone());
        for marker in file_with_markers.markers {
            if marker.name.is_none() {
                if !marker.data.is_empty() {
                    // The marker is an anonymous object marker, which does not need a name. Markers are only set into markerPositions if they have a name
                    continue;
                }
                panic!("Marker at position {} is unnamed", marker.position);
            }
            let name = marker.name.clone().unwrap();
            if let Some(existing) = marker_positions.get(&name) {
                panic!(
                    "Duplicate marker name: \"{}\" at {} and {}",
                    name, marker.position, existing.position
                );
            }
            marker_positions.insert(name, marker);
        }
    }

    let symlinks = hash_map_to_btree_map(symlinks);
    let global_options = hash_map_to_btree_map(global_options);
    if has_tsconfig
        && has_unsupported_global_options_with_config(&global_options)
        && !is_state_baselining_enabled(&global_options)
    {
        panic!("It is not allowed to use global options along with config files.");
    }

    TestData {
        files,
        marker_positions,
        markers,
        symlinks,
        global_options,
        ranges,
    }
}

fn hash_map_to_btree_map(map: HashMap<String, String>) -> BTreeMap<String, String> {
    map.into_iter().collect()
}

pub fn has_unsupported_global_options_with_config(
    global_options: &BTreeMap<String, String>,
) -> bool {
    for option in global_options.keys() {
        match option.to_lowercase().as_str() {
            "symlink" | "link" | "usecasesensitivefilenames" => {}
            _ => return true,
        }
    }
    false
}

pub fn is_config_file(file_name: &str) -> bool {
    let file_name = file_name.to_lowercase();
    file_name.ends_with("tsconfig.json") || file_name.ends_with("jsconfig.json")
}

#[derive(Clone)]
pub struct LocationInformation {
    pub position: usize,
    pub source_position: usize,
    pub source_line: usize,
    pub source_column: usize,
}

#[derive(Clone)]
pub struct RangeLocationInformation {
    pub location_information: LocationInformation,
    pub marker: Option<Marker>,
}

pub const EMIT_THIS_FILE_OPTION: &str = "emitthisfile";

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum ParserState {
    None,
    InSlashStarMarker,
    InObjectMarker,
}

pub fn parse_file_content(
    file_name: &str,
    content: &str,
    file_options: BTreeMap<String, String>,
) -> Result<TestFileWithMarkers, String> {
    let file_name = tspath::get_normalized_absolute_path(file_name, "/");
    let content = chomp_leading_space(content);

    // The file content (minus metacharacters) so far
    let mut output = String::new();
    let mut markers = Vec::new();

    // A stack of the open range markers that are still unclosed
    let mut open_ranges: Vec<RangeLocationInformation> = Vec::new();
    // A list of closed ranges we've collected so far
    let mut range_markers: Vec<RangeMarker> = Vec::new();

    // The total number of metacharacters removed from the file (so far)
    let mut difference = 0_usize;

    // One-based current position data
    let mut line = 1_usize;
    let mut column = 1_usize;

    // The current marker (or maybe multi-line comment?) we're parsing, possibly
    let mut open_marker: Option<LocationInformation> = None;

    // The latest position of the start of an unflushed plain text area
    let mut last_normal_char_position = 0_usize;

    fn flush(
        content: &str,
        last_normal_char_position: usize,
        output: &mut String,
        last_safe_char_index: Option<usize>,
    ) {
        if let Some(last_safe_char_index) = last_safe_char_index {
            output.push_str(&content[last_normal_char_position..last_safe_char_index]);
        } else {
            output.push_str(&content[last_normal_char_position..]);
        }
    }

    let mut state = ParserState::None;
    let chars = content.char_indices().collect::<Vec<_>>();
    if chars.is_empty() {
        return Ok(TestFileWithMarkers {
            file: TestFileInfo {
                file_name,
                content: String::new(),
                emit: false,
            },
            markers,
            ranges: range_markers,
        });
    }
    let mut previous_character = chars[0].1;
    let mut index = 1;
    while index < chars.len() {
        let (i, current_character) = chars[index];
        match state {
            ParserState::None => {
                if previous_character == '[' && current_character == '|' {
                    // found a range start
                    open_ranges.push(RangeLocationInformation {
                        location_information: LocationInformation {
                            position: (i - 1) - difference,
                            source_position: i - 1,
                            source_line: line,
                            source_column: column,
                        },
                        marker: None,
                    });
                    // copy all text up to marker position
                    flush(
                        &content,
                        last_normal_char_position,
                        &mut output,
                        Some(i - 1),
                    );
                    last_normal_char_position = i + 1;
                    difference += 2;
                } else if previous_character == '|' && current_character == ']' {
                    // found a range end
                    if open_ranges.is_empty() {
                        return Err(report_error(
                            &file_name,
                            line,
                            column,
                            "Found range end with no matching start.",
                        ));
                    }
                    let range_start = open_ranges.pop().unwrap();

                    let closed_range = RangeMarker {
                        file_name: file_name.clone(),
                        range: core::new_text_range(
                            range_start.location_information.position as i32,
                            ((i - 1) - difference) as i32,
                        ),
                        ls_range: lsproto::Range::default(),
                        marker: range_start.marker,
                    };

                    range_markers.push(closed_range);

                    // copy all text up to range marker position
                    flush(
                        &content,
                        last_normal_char_position,
                        &mut output,
                        Some(i - 1),
                    );
                    last_normal_char_position = i + 1;
                    difference += 2;
                } else if previous_character == '/' && current_character == '*' {
                    // found a possible marker start
                    state = ParserState::InSlashStarMarker;
                    open_marker = Some(LocationInformation {
                        position: (i - 1) - difference,
                        source_position: i - 1,
                        source_line: line,
                        source_column: column.saturating_sub(1),
                    });
                } else if previous_character == '{' && current_character == '|' {
                    // found an object marker start
                    state = ParserState::InObjectMarker;
                    open_marker = Some(LocationInformation {
                        position: (i - 1) - difference,
                        source_position: i - 1,
                        source_line: line,
                        source_column: column,
                    });
                    flush(
                        &content,
                        last_normal_char_position,
                        &mut output,
                        Some(i - 1),
                    );
                }
            }
            ParserState::InObjectMarker => {
                // Object markers are only ever terminated by |} and have no content restrictions
                if previous_character == '|' && current_character == '}' {
                    let open = open_marker.clone().unwrap();
                    let object_marker_data = content[open.source_position + 2..i - 1].trim();
                    let marker = get_object_marker(&file_name, &open, object_marker_data)?;

                    if let Some(open_range) = open_ranges.last_mut() {
                        open_range.marker = Some(marker.clone());
                    }
                    markers.push(marker);

                    // Set the current start to point to the end of the current marker to ignore its text
                    last_normal_char_position = i + 1;
                    difference += i + 1 - open.source_position;

                    // Reset the state
                    open_marker = None;
                    state = ParserState::None;
                }
            }
            ParserState::InSlashStarMarker => {
                if previous_character == '*' && current_character == '/' {
                    // Record the marker
                    // start + 2 to ignore the */, -1 on the end to ignore the * (/ is next)
                    let open = open_marker.clone().unwrap();
                    let marker_name_text =
                        content[open.source_position + 2..i - 1].trim().to_string();
                    let marker = Marker {
                        file_name: file_name.clone(),
                        position: open.position,
                        ls_position: lsproto::Position {
                            line: 0,
                            character: 0,
                        },
                        name: Some(marker_name_text),
                        data: BTreeMap::new(),
                    };
                    if let Some(open_range) = open_ranges.last_mut() {
                        open_range.marker = Some(marker.clone());
                    }
                    markers.push(marker);

                    // Set the current start to point to the end of the current marker to ignore its text
                    flush(
                        &content,
                        last_normal_char_position,
                        &mut output,
                        Some(open.source_position),
                    );
                    last_normal_char_position = i + 1;
                    difference += i + 1 - open.source_position;

                    // Reset the state
                    open_marker = None;
                    state = ParserState::None;
                } else if !(current_character.is_ascii_digit()
                    || current_character.is_ascii_alphabetic()
                    || current_character == '$'
                    || current_character == '_')
                {
                    // Invalid marker character
                    if current_character == '*'
                        && i < content.len() - 1
                        && content.as_bytes()[i + 1] == b'/'
                    {
                        // The marker is about to be closed, ignore the 'invalid' char
                    } else {
                        // We've hit a non-valid marker character, so we were actually in a block comment
                        // Bail out the text we've gathered so far back into the output
                        flush(&content, last_normal_char_position, &mut output, Some(i));
                        last_normal_char_position = i;
                        open_marker = None;
                        state = ParserState::None;
                    }
                }
            }
        }
        if current_character == '\n' && previous_character == '\r' {
            // Ignore trailing \n after \r
            index += 1;
            continue;
        } else if current_character == '\n' || current_character == '\r' {
            line += 1;
            column = 1;
            index += 1;
            continue;
        }
        column += 1;
        if i >= last_normal_char_position {
            previous_character = current_character;
        } else {
            previous_character = char::REPLACEMENT_CHARACTER; // reset to avoid accidentally reusing marker delimiters as part of other markers
        }
        index += 1;
    }

    // Add the remaining text
    flush(&content, last_normal_char_position, &mut output, None);

    if let Some(open_range) = open_ranges.first() {
        return Err(report_error(
            &file_name,
            open_range.location_information.source_line,
            open_range.location_information.source_column,
            "Unterminated range.",
        ));
    }

    if let Some(open_marker) = open_marker {
        return Err(report_error(
            &file_name,
            open_marker.source_line,
            open_marker.source_column,
            "Unterminated marker.",
        ));
    }

    let output_string = output;
    // Set LS positions for markers
    let script = crate::new_script_info(file_name.clone(), output_string.clone());
    let converters = Converters::default();
    let emit = file_options
        .get(EMIT_THIS_FILE_OPTION)
        .is_some_and(|value| value == "true");

    let test_file_info = TestFileInfo {
        file_name: file_name.clone(),
        content: output_string,
        emit,
    };

    range_markers.sort_by(|a, b| {
        a.range
            .pos()
            .cmp(&b.range.pos())
            .then_with(|| b.range.end().cmp(&a.range.end()))
    });

    for marker in &mut markers {
        marker.ls_position =
            converters.position_to_line_and_character(&script, marker.position as i32);
    }
    for range_marker in &mut range_markers {
        range_marker.ls_range = lsproto::Range {
            start: converters.position_to_line_and_character(&script, range_marker.range.pos()),
            end: converters.position_to_line_and_character(&script, range_marker.range.end()),
        };
    }

    Ok(TestFileWithMarkers {
        file: test_file_info,
        markers,
        ranges: range_markers,
    })
}

pub fn get_object_marker(
    file_name: &str,
    location: &LocationInformation,
    text: &str,
) -> Result<Marker, String> {
    // Attempt to parse the marker value as JSON
    let marker_json = format!("{{ {text} }}");
    let mut value = json::Value::Null;
    if json::unmarshal(marker_json.as_bytes(), &mut value, &[]).is_err() {
        return Err(report_error(
            file_name,
            location.source_line,
            location.source_column,
            &format!("Unable to parse marker text {text}"),
        ));
    }

    let Some(marker_object) = value.as_object() else {
        return Err(report_error(
            file_name,
            location.source_line,
            location.source_column,
            "Object markers can not be empty",
        ));
    };

    let marker_value = marker_object
        .iter()
        .map(|(key, value)| (key.clone(), marker_value_to_string(value)))
        .collect::<BTreeMap<_, _>>();

    if marker_value.is_empty() {
        return Err(report_error(
            file_name,
            location.source_line,
            location.source_column,
            "Object markers can not be empty",
        ));
    }

    let mut marker = Marker {
        file_name: file_name.to_string(),
        position: location.position,
        ls_position: lsproto::Position {
            line: 0,
            character: 0,
        },
        name: None,
        data: marker_value.clone(),
    };

    // Object markers can be anonymous
    if let Some(name) = marker_object.get("name") {
        if let Some(name) = name.as_str() {
            if !name.is_empty() {
                marker.name = Some(name.to_string());
            }
        }
    }

    Ok(marker)
}

fn marker_value_to_string(value: &json::Value) -> String {
    match value {
        json::Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

pub fn report_error(file_name: &str, line: usize, col: usize, message: &str) -> String {
    FourslashError {
        err: format!("{file_name} ({line},{col}): {message}"),
    }
    .error()
}

pub fn chomp_leading_space(content: &str) -> String {
    let lines = content.split('\n').collect::<Vec<_>>();
    for line in &lines {
        if !line.is_empty() && !line.starts_with(' ') {
            return content.to_string();
        }
    }

    let result = lines
        .into_iter()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                line[1..].to_string()
            }
        })
        .collect::<Vec<_>>();
    result.join("\n")
}

pub struct FourslashError {
    pub err: String,
}

impl FourslashError {
    pub fn error(&self) -> String {
        self.err.clone()
    }
}
