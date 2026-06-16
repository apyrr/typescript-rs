use std::sync::Arc;

use ts_ast as ast;
use ts_core as core;
use ts_parser as parser;

use crate::{FileHandleRef, RefCountCache, RefCountCacheOptions, new_ref_count_cache};

#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct ParseCacheKey {
    pub source_file_parse_options: ast::SourceFileParseOptions,
    pub script_kind: core::ScriptKind,
    pub content_hash: u128,
}

pub fn new_parse_cache_key(
    options: ast::SourceFileParseOptions,
    content_hash: u128,
    script_kind: core::ScriptKind,
) -> ParseCacheKey {
    ParseCacheKey {
        source_file_parse_options: options,
        script_kind,
        content_hash,
    }
}

pub type ParseCache = Arc<RefCountCache<ParseCacheKey, FileHandleRef, ast::ParsedSourceFile>>;

pub fn new_parse_cache(options: RefCountCacheOptions) -> ParseCache {
    Arc::new(new_ref_count_cache(
        options,
        |key: &ParseCacheKey, fh: FileHandleRef| {
            parser::parse_source_file_as_parsed_with_hash(
                key.source_file_parse_options.clone(),
                fh.content(),
                key.script_kind,
                key.content_hash,
            )
        },
    ))
}
