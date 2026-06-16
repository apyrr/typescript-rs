use std::collections::HashMap;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use ts_scanner as scanner;
use ts_tspath as tspath;

use crate::{
    ECMALineInfo, MISSING_SOURCE, NameIndex, RawSourceMap, SourceIndex, decode_mappings,
    try_get_source_mapping_url,
};

pub trait Host {
    fn use_case_sensitive_file_names(&self) -> bool;
    fn get_ecma_line_info(&self, file_name: &str) -> Option<ECMALineInfo>;
    fn read_file(&self, file_name: &str) -> Option<String>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MappedPosition {
    pub generated_position: i32,
    pub source_position: i32,
    pub source_index: SourceIndex,
    pub name_index: NameIndex,
}

pub const MISSING_POSITION: i32 = -1;

impl MappedPosition {
    pub fn is_source_mapped_position(&self) -> bool {
        self.source_index != MISSING_SOURCE && self.source_position != MISSING_POSITION
    }
}

pub type SourceMappedPosition = MappedPosition;

#[derive(Clone)]
pub struct DocumentPositionMapper {
    pub use_case_sensitive_file_names: bool,
    pub source_file_absolute_paths: Vec<String>,
    pub source_to_source_index_map: HashMap<String, SourceIndex>,
    pub generated_absolute_file_path: String,
    pub generated_mappings: Vec<MappedPosition>,
    pub source_mappings: HashMap<SourceIndex, Vec<SourceMappedPosition>>,
}

pub fn create_document_position_mapper(
    host: &impl Host,
    source_map: &RawSourceMap,
    map_path: &str,
) -> DocumentPositionMapper {
    let map_directory = tspath::get_directory_path(map_path);
    let source_root = if source_map.source_root.is_empty() {
        map_directory.clone()
    } else {
        tspath::get_normalized_absolute_path(&source_map.source_root, &map_directory)
    };
    let generated_absolute_file_path =
        tspath::get_normalized_absolute_path(&source_map.file, &map_directory);
    let source_file_absolute_paths = source_map
        .sources
        .iter()
        .map(|source| tspath::get_normalized_absolute_path(source, &source_root))
        .collect::<Vec<_>>();

    let use_case_sensitive_file_names = host.use_case_sensitive_file_names();
    let mut source_to_source_index_map = HashMap::with_capacity(source_file_absolute_paths.len());
    for (i, source) in source_file_absolute_paths.iter().enumerate() {
        source_to_source_index_map.insert(
            tspath::get_canonical_file_name(source, use_case_sensitive_file_names),
            i as SourceIndex,
        );
    }

    let mut decoded_mappings = Vec::new();
    let mut decoder = decode_mappings(source_map.mappings.clone());
    for mapping in decoder.by_ref() {
        let generated_position = host
            .get_ecma_line_info(&generated_absolute_file_path)
            .map(|line_info| {
                scanner::compute_position_of_line_and_utf16_character(
                    line_info.line_starts(),
                    mapping.generated_line as usize,
                    mapping.generated_character,
                    line_info.text(),
                    true,
                ) as i32
            })
            .unwrap_or(MISSING_POSITION);

        let source_position = if mapping.is_source_mapping() {
            source_file_absolute_paths
                .get(mapping.source_index as usize)
                .and_then(|file_name| host.get_ecma_line_info(file_name))
                .map(|line_info| {
                    scanner::compute_position_of_line_and_utf16_character(
                        line_info.line_starts(),
                        mapping.source_line as usize,
                        mapping.source_character,
                        line_info.text(),
                        true,
                    ) as i32
                })
                .unwrap_or(MISSING_POSITION)
        } else {
            MISSING_POSITION
        };

        decoded_mappings.push(MappedPosition {
            generated_position,
            source_position,
            source_index: mapping.source_index,
            name_index: mapping.name_index,
        });
    }
    if decoder.error().is_some() {
        decoded_mappings.clear();
    }

    let mut source_mappings: HashMap<SourceIndex, Vec<SourceMappedPosition>> = HashMap::new();
    for mapping in &decoded_mappings {
        if !mapping.is_source_mapped_position() {
            continue;
        }
        source_mappings
            .entry(mapping.source_index)
            .or_default()
            .push(SourceMappedPosition {
                generated_position: mapping.generated_position,
                source_position: mapping.source_position,
                source_index: mapping.source_index,
                name_index: mapping.name_index,
            });
    }
    for list in source_mappings.values_mut() {
        list.sort_by_key(|mapping| mapping.source_position);
        list.dedup_by(|a, b| {
            a.generated_position == b.generated_position
                && a.source_index == b.source_index
                && a.source_position == b.source_position
        });
    }

    let mut generated_mappings = decoded_mappings;
    generated_mappings.sort_by_key(|mapping| mapping.generated_position);
    generated_mappings.dedup_by(|a, b| {
        a.generated_position == b.generated_position
            && a.source_index == b.source_index
            && a.source_position == b.source_position
    });

    DocumentPositionMapper {
        use_case_sensitive_file_names,
        source_file_absolute_paths,
        source_to_source_index_map,
        generated_absolute_file_path,
        generated_mappings,
        source_mappings,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentPosition {
    pub file_name: String,
    pub pos: i32,
}

impl DocumentPositionMapper {
    pub fn get_source_position(&self, loc: &DocumentPosition) -> Option<DocumentPosition> {
        if self.generated_mappings.is_empty() {
            return None;
        }

        let target_index = self
            .generated_mappings
            .partition_point(|mapping| mapping.generated_position < loc.pos);
        let mapping = self.generated_mappings.get(target_index)?;
        if !mapping.is_source_mapped_position() {
            return None;
        }

        Some(DocumentPosition {
            file_name: self
                .source_file_absolute_paths
                .get(mapping.source_index as usize)?
                .clone(),
            pos: mapping.source_position,
        })
    }

    pub fn get_generated_position(&self, loc: &DocumentPosition) -> Option<DocumentPosition> {
        let source_index =
            *self
                .source_to_source_index_map
                .get(&tspath::get_canonical_file_name(
                    &loc.file_name,
                    self.use_case_sensitive_file_names,
                ))?;
        let source_mappings = self.source_mappings.get(&source_index)?;
        if source_index < 0 || source_index as usize >= self.source_mappings.len() {
            return None;
        }
        let target_index =
            source_mappings.partition_point(|mapping| mapping.source_position < loc.pos);
        let mapping = source_mappings.get(target_index)?;
        if mapping.source_index != source_index {
            return None;
        }

        Some(DocumentPosition {
            file_name: self.generated_absolute_file_path.clone(),
            pos: mapping.generated_position,
        })
    }
}

pub fn get_document_position_mapper(
    host: &impl Host,
    generated_file_name: &str,
) -> Option<DocumentPositionMapper> {
    let mut map_file_name =
        try_get_source_mapping_url(host.get_ecma_line_info(generated_file_name).as_ref());
    if !map_file_name.is_empty() {
        let (base64_object, matched) = try_parse_base64_url(&map_file_name);
        if matched {
            if !base64_object.is_empty()
                && let Ok(decoded) = BASE64_STANDARD.decode(&base64_object)
                && let Ok(contents) = String::from_utf8(decoded)
            {
                return convert_document_to_source_mapper(host, &contents, generated_file_name);
            }
            map_file_name.clear();
        }
    }

    let mut possible_map_locations = Vec::new();
    if !map_file_name.is_empty() {
        possible_map_locations.push(map_file_name);
    }
    possible_map_locations.push(format!("{generated_file_name}.map"));
    for location in possible_map_locations {
        let map_file_name = tspath::get_normalized_absolute_path(
            &location,
            &tspath::get_directory_path(generated_file_name),
        );
        if let Some(map_file_contents) = host.read_file(&map_file_name) {
            return convert_document_to_source_mapper(host, &map_file_contents, &map_file_name);
        }
    }
    None
}

pub fn convert_document_to_source_mapper(
    host: &impl Host,
    contents: &str,
    map_file_name: &str,
) -> Option<DocumentPositionMapper> {
    let source_map = try_parse_raw_source_map(contents)?;
    if source_map.sources.is_empty() || source_map.file.is_empty() || source_map.mappings.is_empty()
    {
        return None;
    }
    if source_map
        .sources_content
        .iter()
        .any(|content| content.is_some())
    {
        return None;
    }
    Some(create_document_position_mapper(
        host,
        &source_map,
        map_file_name,
    ))
}

pub fn try_parse_raw_source_map(contents: &str) -> Option<RawSourceMap> {
    let source_map: RawSourceMap = serde_json::from_str(contents).ok()?;
    if source_map.version != 3 {
        return None;
    }
    Some(source_map)
}

pub fn try_get_source_mapping_url_from_host(host: &impl Host, file_name: &str) -> String {
    try_get_source_mapping_url(host.get_ecma_line_info(file_name).as_ref())
}

pub fn try_parse_base64_url(url: &str) -> (String, bool) {
    let Some(mut url) = url.strip_prefix("data:") else {
        return (String::new(), false);
    };
    let Some(rest) = url.strip_prefix("application/json;") else {
        return (String::new(), true);
    };
    url = rest;
    if let Some(rest) = url.strip_prefix("charset=") {
        if rest.len() < "utf-8;".len() || !rest[.."utf-8;".len()].eq_ignore_ascii_case("utf-8;") {
            return (String::new(), true);
        }
        url = &rest["utf-8;".len()..];
    }
    let Some(rest) = url.strip_prefix("base64,") else {
        return (String::new(), true);
    };
    if rest
        .chars()
        .any(|ch| !ch.is_ascii_alphanumeric() && ch != '+' && ch != '/' && ch != '=')
    {
        return (String::new(), true);
    }
    (rest.to_string(), true)
}
