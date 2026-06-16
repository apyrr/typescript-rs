#![forbid(unsafe_code)]

#[expect(
    dead_code,
    reason = "ported parser entry points are ahead of current callers"
)]
mod parser;
mod references;
mod types;
#[expect(
    dead_code,
    reason = "ported parser utilities are ahead of current callers"
)]
mod utilities;

#[cfg(test)]
mod parser_test;

pub use parser::{
    ParsedIsolatedEntityName, parse_isolated_entity_name, parse_source_file,
    parse_source_file_as_parsed, parse_source_file_as_parsed_with_hash,
    parse_source_file_with_hash,
};
pub use types::ParseFlags;
