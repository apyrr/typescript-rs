use ts_tsoptions as tsoptions;
use ts_tspath as tspath;
use xxhash_rust::xxh3::Xxh3;

use crate::{OwnerCache, SourceFs, new_owner_cache};

pub struct ExtendedConfigParseArgs {
    pub file_name: String,
    pub content: String,
    pub fs: SourceFs,
    pub resolution_stack: Vec<String>,
}

#[derive(Clone, Default)]
pub struct ExtendedConfigCacheEntry {
    pub extended_config_cache_entry: Option<tsoptions::ExtendedConfigCacheEntry>,
    pub hash: u128,
}

pub type ExtendedConfigCache =
    OwnerCache<tspath::Path, ExtendedConfigCacheEntry, ExtendedConfigParseArgs>;

pub fn new_extended_config_cache() -> ExtendedConfigCache {
    new_owner_cache(Some(
        |_path: &tspath::Path, entry: &ExtendedConfigCacheEntry, args: &ExtendedConfigParseArgs| {
            entry.hash == u128::default()
                || entry.hash != hash(entry.extended_config_cache_entry.as_ref().unwrap(), args)
        },
    ))
}

pub(crate) fn parse_extended_config_cache_entry(
    path: tspath::Path,
    args: &ExtendedConfigParseArgs,
    host: &dyn tsoptions::ParseConfigHost,
    cache: &dyn tsoptions::ExtendedConfigCache,
) -> ExtendedConfigCacheEntry {
    let parsed = tsoptions::parse_extended_config(
        &args.file_name,
        path,
        &args.resolution_stack,
        host,
        Some(cache),
    );
    let hash = hash(&parsed, args);
    ExtendedConfigCacheEntry {
        extended_config_cache_entry: Some(parsed),
        hash,
    }
}

fn hash(entry: &tsoptions::ExtendedConfigCacheEntry, args: &ExtendedConfigParseArgs) -> u128 {
    let mut hasher = Xxh3::new();
    hasher.update(args.content.as_bytes());
    for file_name in entry.extended_file_names() {
        let fh = args.fs.get_file(file_name);
        let Some(fh) = fh else {
            return u128::default();
        };
        hasher.update(fh.content().as_bytes());
    }
    hasher.digest128()
}
