use std::collections::BTreeMap;

use ts_tspath as tspath;
use ts_vfs::vfsmatch::{self, Usage};

pub type WildcardDirectories = BTreeMap<String, bool>;

#[derive(Clone, Debug, Eq, PartialEq)]
struct WildcardDirectoryMatch {
    key: String,
    path: String,
    recursive: bool,
}

pub fn get_wildcard_directories(
    include: &[String],
    exclude: &[String],
    compare_paths_options: &tspath::ComparePathsOptions,
) -> WildcardDirectories {
    if include.is_empty() {
        return BTreeMap::new();
    }

    let exclude_matcher = vfsmatch::new_spec_matcher(
        exclude,
        &compare_paths_options.current_directory,
        Usage::Exclude,
        compare_paths_options.use_case_sensitive_file_names,
    );
    let mut wildcard_directories = BTreeMap::new();
    let mut wildcard_key_to_path = BTreeMap::new();
    let mut recursive_keys = Vec::new();

    for file in include {
        let spec = tspath::normalize_slashes(&tspath::combine_paths(
            &compare_paths_options.current_directory,
            &[file.as_str()],
        ));
        if exclude_matcher
            .as_ref()
            .is_some_and(|matcher| matcher.match_string(&spec))
        {
            continue;
        }

        if let Some(matched) = get_wildcard_directory_from_spec(
            &spec,
            compare_paths_options.use_case_sensitive_file_names,
        ) {
            let existing_path = wildcard_key_to_path.get(&matched.key).cloned();
            let existing_recursive = existing_path
                .as_ref()
                .and_then(|path| wildcard_directories.get(path))
                .copied()
                .unwrap_or(false);

            if existing_path.is_none() || (!existing_recursive && matched.recursive) {
                let path_to_use = existing_path.unwrap_or_else(|| matched.path.clone());
                wildcard_directories.insert(path_to_use.clone(), matched.recursive);
                wildcard_key_to_path
                    .entry(matched.key.clone())
                    .or_insert(matched.path);
                if matched.recursive {
                    recursive_keys.push(matched.key);
                }
            }
        }

        let paths = wildcard_directories.keys().cloned().collect::<Vec<_>>();
        for path in paths {
            let key = to_canonical_key(&path, compare_paths_options.use_case_sensitive_file_names);
            if recursive_keys.iter().any(|recursive_key| {
                key != *recursive_key
                    && tspath::contains_path(recursive_key, &key, compare_paths_options)
            }) {
                wildcard_directories.remove(&path);
            }
        }
    }

    wildcard_directories
}

fn to_canonical_key(path: &str, use_case_sensitive_file_names: bool) -> String {
    if use_case_sensitive_file_names {
        path.to_owned()
    } else {
        path.to_lowercase()
    }
}

fn get_wildcard_directory_from_spec(
    spec: &str,
    use_case_sensitive_file_names: bool,
) -> Option<WildcardDirectoryMatch> {
    let first_wildcard = spec.find(['*', '?']);
    if let Some(first_wildcard) = first_wildcard
        && let Some(last_sep_before_wildcard) = spec[..first_wildcard].rfind('/')
    {
        let path = &spec[..last_sep_before_wildcard];
        let last_directory_separator_index = spec.rfind('/').unwrap_or(0);
        let recursive = first_wildcard < last_directory_separator_index;
        return Some(WildcardDirectoryMatch {
            key: to_canonical_key(path, use_case_sensitive_file_names),
            path: path.to_owned(),
            recursive,
        });
    }

    if let Some(last_sep_index) = spec.rfind('/') {
        let last_segment = &spec[last_sep_index + 1..];
        if vfsmatch::is_implicit_glob(last_segment) {
            let path = tspath::remove_trailing_directory_separator(spec).to_owned();
            return Some(WildcardDirectoryMatch {
                key: to_canonical_key(&path, use_case_sensitive_file_names),
                path,
                recursive: true,
            });
        }
    }

    None
}

pub fn contains_wildcard(spec: &str) -> bool {
    spec.contains('*') || spec.contains('?')
}
