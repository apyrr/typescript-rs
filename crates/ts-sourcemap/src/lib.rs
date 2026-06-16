#![forbid(unsafe_code)]
mod decoder;
mod generator;
mod lineinfo;
mod source;
mod source_mapper;
mod util;

#[cfg(test)]
mod generator_test;

pub use decoder::{
    MISSING_LINE_OR_COLUMN, MISSING_NAME, MISSING_SOURCE, MISSING_UTF16_COLUMN, Mapping,
    MappingsDecoder, NameIndex, SourceIndex, base64_format_decode, decode_mappings,
};
pub use generator::{
    Generator, NAME_INDEX_NOT_SET, NOT_SET, NOT_SET_UTF16, RawSourceMap, SOURCE_INDEX_NOT_SET,
    base64_format_encode, new_generator,
};
pub use lineinfo::{ECMALineInfo, create_ecma_line_info};
pub use source::Source;
pub use source_mapper::{
    DocumentPosition, DocumentPositionMapper, Host, MISSING_POSITION, MappedPosition,
    SourceMappedPosition, convert_document_to_source_mapper, create_document_position_mapper,
    get_document_position_mapper, try_get_source_mapping_url_from_host, try_parse_base64_url,
    try_parse_raw_source_map,
};
pub use util::try_get_source_mapping_url;
