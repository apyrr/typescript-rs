use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

pub type Path = String;

pub const DIRECTORY_SEPARATOR: char = '/';
const URL_SCHEME_SEPARATOR: &str = "://";

pub fn is_any_directory_separator(ch: u8) -> bool {
    ch == b'/' || ch == b'\\'
}

pub fn is_url(path: &str) -> bool {
    get_encoded_root_length(path) < 0
}

pub fn is_rooted_disk_path(path: &str) -> bool {
    get_encoded_root_length(path) > 0
}

pub fn is_disk_path_root(path: &str) -> bool {
    let root_length = get_encoded_root_length(path);
    root_length > 0 && root_length as usize == path.len()
}

pub fn is_dynamic_file_name(file_name: &str) -> bool {
    file_name.starts_with("^/")
}

pub fn path_is_absolute(path: &str) -> bool {
    get_encoded_root_length(path) != 0
}

pub fn has_trailing_directory_separator(path: &str) -> bool {
    path.as_bytes()
        .last()
        .is_some_and(|ch| is_any_directory_separator(*ch))
}

pub fn combine_paths(first_path: &str, paths: &[&str]) -> String {
    let mut result = normalize_slashes(first_path);
    for trailing_path in paths {
        if trailing_path.is_empty() {
            continue;
        }
        let trailing_path = normalize_slashes(trailing_path);
        if result.is_empty() || get_root_length(&trailing_path) != 0 {
            result = trailing_path;
        } else {
            if !has_trailing_directory_separator(&result) {
                result.push(DIRECTORY_SEPARATOR);
            }
            result.push_str(&trailing_path);
        }
    }
    result
}

pub fn get_path_components(path: &str, current_directory: &str) -> Vec<String> {
    let path = combine_paths(current_directory, &[path]);
    path_components(&path, get_root_length(&path))
}

pub fn path_components(path: &str, root_length: usize) -> Vec<String> {
    let mut result = vec![path[..root_length].to_owned()];
    let mut rest = path[root_length..].split('/').collect::<Vec<_>>();
    if rest.last().is_some_and(|component| component.is_empty()) {
        rest.pop();
    }
    result.extend(rest.into_iter().map(str::to_owned));
    result
}

pub fn is_volume_character(ch: u8) -> bool {
    ch.is_ascii_alphabetic()
}

fn get_file_url_volume_separator_end(url: &str, start: usize) -> Option<usize> {
    let bytes = url.as_bytes();
    if bytes.len() <= start {
        return None;
    }
    if bytes[start] == b':' {
        return Some(start + 1);
    }
    if bytes[start] == b'%' && bytes.len() > start + 2 && bytes[start + 1] == b'3' {
        let ch2 = bytes[start + 2];
        if ch2 == b'a' || ch2 == b'A' {
            return Some(start + 3);
        }
    }
    None
}

pub fn get_encoded_root_length(path: &str) -> isize {
    let bytes = path.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return 0;
    }
    let ch0 = bytes[0];
    if ch0 == b'/' || ch0 == b'\\' {
        if len == 1 || bytes[1] != ch0 {
            return 1;
        }
        let offset = 2;
        return path[offset..]
            .find(ch0 as char)
            .map(|p1| (p1 + offset + 1) as isize)
            .unwrap_or(len as isize);
    }
    if is_volume_character(ch0) && len > 1 && bytes[1] == b':' {
        if len == 2 {
            return 2;
        }
        if bytes[2] == b'/' || bytes[2] == b'\\' {
            return 3;
        }
    }
    if ch0 == b'^' && len > 1 && bytes[1] == b'/' {
        return 2;
    }
    if let Some(scheme_end) = path.find(URL_SCHEME_SEPARATOR) {
        let authority_start = scheme_end + URL_SCHEME_SEPARATOR.len();
        if let Some(authority_length) = path[authority_start..].find('/') {
            let authority_end = authority_start + authority_length;
            let scheme = &path[..scheme_end];
            let authority = &path[authority_start..authority_end];
            if scheme == "file"
                && (authority.is_empty() || authority == "localhost")
                && len > authority_end + 2
                && is_volume_character(bytes[authority_end + 1])
                && let Some(volume_separator_end) =
                    get_file_url_volume_separator_end(path, authority_end + 2)
            {
                if volume_separator_end == len {
                    return !(volume_separator_end as isize);
                }
                if bytes[volume_separator_end] == b'/' {
                    return !((volume_separator_end + 1) as isize);
                }
            }
            return !((authority_end + 1) as isize);
        }
        return !(len as isize);
    }
    0
}

pub fn get_root_length(path: &str) -> usize {
    let root_length = get_encoded_root_length(path);
    if root_length < 0 {
        (!root_length) as usize
    } else {
        root_length as usize
    }
}

pub fn get_directory_path(path: &str) -> String {
    let path = normalize_slashes(path);
    let root_length = get_root_length(&path);
    if root_length == path.len() {
        return path;
    }
    let path = remove_trailing_directory_separator(&path).to_owned();
    let last_separator = path.rfind('/').unwrap_or(0);
    path[..root_length.max(last_separator)].to_owned()
}

pub fn get_path_from_path_components(path_components: &[String]) -> String {
    if path_components.is_empty() {
        return String::new();
    }
    let mut root = path_components[0].clone();
    if !root.is_empty() {
        root = ensure_trailing_directory_separator(&root);
    }
    format!("{}{}", root, path_components[1..].join("/"))
}

pub fn normalize_slashes(path: &str) -> String {
    path.replace('\\', "/")
}

pub fn reduce_path_components(components: &[String]) -> Vec<String> {
    if components.is_empty() {
        return Vec::new();
    }
    let mut reduced = vec![components[0].clone()];
    for component in &components[1..] {
        if component.is_empty() || component == "." {
            continue;
        }
        if component == ".." {
            if reduced.len() > 1 && reduced.last().is_some_and(|last| last != "..") {
                reduced.pop();
                continue;
            }
            if reduced.len() == 1 && !reduced[0].is_empty() {
                continue;
            }
        }
        reduced.push(component.clone());
    }
    reduced
}

pub fn resolve_path(path: &str, paths: &[&str]) -> String {
    let combined = if paths.is_empty() {
        normalize_slashes(path)
    } else {
        combine_paths(path, paths)
    };
    normalize_path(&combined)
}

pub fn resolve_tripleslash_reference(module_name: &str, containing_file: &str) -> String {
    let base_path = get_directory_path(containing_file);
    if is_rooted_disk_path(module_name) {
        normalize_path(module_name)
    } else {
        normalize_path(&combine_paths(&base_path, &[module_name]))
    }
}

pub fn get_normalized_path_components(path: &str, current_directory: &str) -> Vec<String> {
    let combined = combine_paths(current_directory, &[path]);
    get_normalized_path_components_from_combined(&combined)
}

fn get_normalized_path_components_from_combined(path: &str) -> Vec<String> {
    let root_length = get_root_length(path);
    let mut components = vec![path[..root_length].to_owned()];
    let mut index = root_length;
    while index < path.len() {
        while index < path.len() && path.as_bytes()[index] == b'/' {
            index += 1;
        }
        if index >= path.len() {
            break;
        }

        let start = index;
        while index < path.len() && path.as_bytes()[index] != b'/' {
            index += 1;
        }
        let component = &path[start..index];
        if component.is_empty() || component == "." {
            continue;
        }
        if component == ".." {
            if components.len() > 1 {
                if components.last().is_some_and(|last| last != "..") {
                    components.pop();
                    continue;
                }
            } else if !components[0].is_empty() {
                continue;
            }
        }
        components.push(component.to_owned());
    }
    components
}

pub fn get_normalized_absolute_path_without_root(
    file_name: &str,
    current_directory: &str,
) -> String {
    let absolute_path = get_normalized_absolute_path(file_name, current_directory);
    let root_length = get_root_length(&absolute_path);
    absolute_path[root_length..].to_owned()
}

pub fn get_normalized_absolute_path(file_name: &str, current_directory: &str) -> String {
    let file_name = if get_root_length(file_name) == 0 && !current_directory.is_empty() {
        combine_paths(current_directory, &[file_name])
    } else {
        normalize_slashes(file_name)
    };
    let components = get_normalized_path_components_from_combined(&file_name);
    let result = get_path_from_path_components(&components);
    let root_length = get_root_length(&file_name);
    if result.len() > root_length {
        remove_trailing_directory_separator(&result).to_owned()
    } else if result.len() == root_length && root_length != 0 {
        ensure_trailing_directory_separator(&result)
    } else {
        result
    }
}

pub fn normalize_path(path: &str) -> String {
    let path = normalize_slashes(path);
    if let Some(normalized) = simple_normalize_path(&path) {
        return normalized;
    }
    let mut result = get_normalized_absolute_path(&path, "");
    if has_trailing_directory_separator(&path) && !has_trailing_directory_separator(&result) {
        result.push('/');
    }
    result
}

fn simple_normalize_path(path: &str) -> Option<String> {
    if !has_relative_path_segment(path) {
        return Some(path.to_owned());
    }
    let simplified = path.replace("/./", "/");
    let trimmed = simplified.strip_prefix("./").unwrap_or(&simplified);
    if trimmed != path
        && !has_relative_path_segment(trimmed)
        && !(trimmed != simplified && trimmed.starts_with('/'))
    {
        return Some(trimmed.to_owned());
    }
    None
}

fn has_relative_path_segment(path: &str) -> bool {
    let bytes = path.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return false;
    }
    if path == "." || path == ".." {
        return true;
    }
    if bytes[0] == b'.' {
        if len >= 2 && bytes[1] == b'/' {
            return true;
        }
        if len >= 3 && bytes[1] == b'.' && bytes[2] == b'/' {
            return true;
        }
    }
    if bytes[len - 1] == b'.' {
        if len >= 2 && bytes[len - 2] == b'/' {
            return true;
        }
        if len >= 3 && bytes[len - 2] == b'.' && bytes[len - 3] == b'/' {
            return true;
        }
    }

    let mut previous_slash = false;
    let mut segment_len = 0usize;
    let mut dot_count: isize = 0;
    for &byte in bytes {
        if byte == b'/' {
            if previous_slash {
                return true;
            }
            if (segment_len == 1 && dot_count == 1) || (segment_len == 2 && dot_count == 2) {
                return true;
            }
            previous_slash = true;
            segment_len = 0;
            dot_count = 0;
            continue;
        }
        if byte == b'.' {
            if dot_count >= 0 {
                dot_count += 1;
            }
        } else {
            dot_count = -1;
        }
        segment_len += 1;
        previous_slash = false;
    }
    (segment_len == 1 && dot_count == 1) || (segment_len == 2 && dot_count == 2)
}

pub fn get_canonical_file_name(file_name: &str, use_case_sensitive_file_names: bool) -> String {
    if use_case_sensitive_file_names {
        file_name.to_owned()
    } else {
        to_file_name_lower_case(file_name)
    }
}

pub fn to_file_name_lower_case(file_name: &str) -> String {
    const I_WITH_DOT: char = '\u{0130}';
    if file_name.is_ascii() {
        return file_name.to_ascii_lowercase();
    }
    file_name
        .chars()
        .flat_map(|ch| {
            if ch == I_WITH_DOT {
                ch.to_string()
            } else {
                ch.to_lowercase().collect::<String>()
            }
            .chars()
            .collect::<Vec<_>>()
        })
        .collect()
}

pub fn to_path(file_name: &str, base_path: &str, use_case_sensitive_file_names: bool) -> Path {
    let path = if is_rooted_disk_path(file_name) {
        normalize_path(file_name)
    } else {
        get_normalized_absolute_path(file_name, base_path)
    };
    get_canonical_file_name(&path, use_case_sensitive_file_names)
}

pub fn remove_trailing_directory_separator(path: &str) -> &str {
    if has_trailing_directory_separator(path) {
        &path[..path.len() - 1]
    } else {
        path
    }
}

pub fn remove_trailing_directory_separators(mut path: &str) -> &str {
    while has_trailing_directory_separator(path) {
        path = remove_trailing_directory_separator(path);
    }
    path
}

pub fn ensure_trailing_directory_separator(path: &str) -> String {
    if has_trailing_directory_separator(path) {
        path.to_owned()
    } else {
        format!("{path}/")
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ComparePathsOptions {
    pub use_case_sensitive_file_names: bool,
    pub current_directory: String,
}

impl ComparePathsOptions {
    pub fn compare_strings(&self, a: &str, b: &str) -> Ordering {
        if self.use_case_sensitive_file_names {
            a.cmp(b)
        } else {
            to_file_name_lower_case(a).cmp(&to_file_name_lower_case(b))
        }
    }

    pub fn equal_strings(&self, a: &str, b: &str) -> bool {
        self.compare_strings(a, b) == Ordering::Equal
    }
}

pub fn get_path_components_relative_to(
    from: &str,
    to: &str,
    options: &ComparePathsOptions,
) -> Vec<String> {
    let from_components =
        reduce_path_components(&get_path_components(from, &options.current_directory));
    let to_components =
        reduce_path_components(&get_path_components(to, &options.current_directory));
    let mut start = 0;
    let max_common = from_components.len().min(to_components.len());
    while start < max_common {
        let equal = if start == 0 {
            from_components[start].eq_ignore_ascii_case(&to_components[start])
        } else {
            options.equal_strings(&from_components[start], &to_components[start])
        };
        if !equal {
            break;
        }
        start += 1;
    }
    if start == 0 {
        return to_components;
    }
    let mut result = vec![String::new()];
    result.extend((start..from_components.len()).map(|_| "..".to_owned()));
    result.extend(to_components[start..].iter().cloned());
    result
}

pub fn get_relative_path_from_directory(
    from_directory: &str,
    to: &str,
    options: &ComparePathsOptions,
) -> String {
    assert_eq!(
        get_root_length(from_directory) > 0,
        get_root_length(to) > 0,
        "paths must either both be absolute or both be relative"
    );
    let components = get_path_components_relative_to(from_directory, to, options);
    get_path_from_path_components(&components)
}

pub fn get_relative_path_from_file(from: &str, to: &str, options: &ComparePathsOptions) -> String {
    ensure_path_is_non_module_name(&get_relative_path_from_directory(
        &get_directory_path(from),
        to,
        options,
    ))
}

pub fn convert_to_relative_path(path: &str, options: &ComparePathsOptions) -> String {
    if !is_rooted_disk_path(path) {
        return path.to_owned();
    }
    get_relative_path_to_directory_or_url(&options.current_directory, path, false, options)
}

pub fn get_relative_path_to_directory_or_url(
    directory_path_or_url: &str,
    relative_or_absolute_path: &str,
    is_absolute_path_an_url: bool,
    options: &ComparePathsOptions,
) -> String {
    let mut components =
        get_path_components_relative_to(directory_path_or_url, relative_or_absolute_path, options);
    if is_absolute_path_an_url
        && components
            .first()
            .is_some_and(|component| is_rooted_disk_path(component))
    {
        let prefix = if components[0].starts_with('/') {
            "file://"
        } else {
            "file:///"
        };
        components[0] = format!("{prefix}{}", components[0]);
    }
    get_path_from_path_components(&components)
}

pub fn get_base_file_name(path: &str) -> String {
    let path = normalize_slashes(path);
    let root_length = get_root_length(&path);
    if root_length == path.len() {
        return String::new();
    }
    let path = remove_trailing_directory_separator(&path).to_owned();
    let last_separator = path.rfind('/').map(|index| index + 1).unwrap_or(0);
    path[root_length.max(last_separator)..].to_owned()
}

pub fn get_any_extension_from_path(path: &str, extensions: &[&str], ignore_case: bool) -> String {
    if !extensions.is_empty() {
        let path = remove_trailing_directory_separator(path);
        for extension in extensions {
            let extension = if extension.starts_with('.') {
                (*extension).to_owned()
            } else {
                format!(".{extension}")
            };
            if path.len() >= extension.len() {
                let path_extension = &path[path.len() - extension.len()..];
                let equal = if ignore_case {
                    path_extension.eq_ignore_ascii_case(&extension)
                } else {
                    path_extension == extension
                };
                if equal {
                    return path_extension.to_owned();
                }
            }
        }
        return String::new();
    }
    let base = get_base_file_name(path);
    base.rfind('.')
        .map(|index| base[index..].to_owned())
        .unwrap_or_default()
}

pub fn path_is_relative(path: &str) -> bool {
    path == "."
        || path == ".."
        || path.starts_with("./")
        || path.starts_with(".\\")
        || path.starts_with("../")
        || path.starts_with("..\\")
}

pub fn ensure_path_is_non_module_name(path: &str) -> String {
    if !path_is_absolute(path) && !path_is_relative(path) {
        format!("./{path}")
    } else {
        path.to_owned()
    }
}

pub fn is_external_module_name_relative(module_name: &str) -> bool {
    path_is_relative(module_name) || is_rooted_disk_path(module_name)
}

pub fn compare_paths(a: &str, b: &str, options: &ComparePathsOptions) -> Ordering {
    let a = combine_paths(&options.current_directory, &[a]);
    let b = combine_paths(&options.current_directory, &[b]);
    if a == b {
        return Ordering::Equal;
    }
    if a.is_empty() {
        return Ordering::Less;
    }
    if b.is_empty() {
        return Ordering::Greater;
    }

    let a_root_length = get_root_length(&a);
    let b_root_length = get_root_length(&b);
    let a_root = &a[..a_root_length];
    let b_root = &b[..b_root_length];
    let root_order = to_file_name_lower_case(a_root).cmp(&to_file_name_lower_case(b_root));
    if root_order != Ordering::Equal {
        return root_order;
    }

    let a_rest = &a[a_root_length..];
    let b_rest = &b[b_root_length..];
    if !has_relative_path_segment(a_rest) && !has_relative_path_segment(b_rest) {
        return options.compare_strings(a_rest, b_rest);
    }

    let a_components = reduce_path_components(&get_path_components(&a, ""));
    let b_components = reduce_path_components(&get_path_components(&b, ""));
    for index in 1..a_components.len().min(b_components.len()) {
        let order = options.compare_strings(&a_components[index], &b_components[index]);
        if order != Ordering::Equal {
            return order;
        }
    }
    a_components.len().cmp(&b_components.len())
}

pub fn compare_paths_case_sensitive(a: &str, b: &str, current_directory: &str) -> Ordering {
    compare_paths(
        a,
        b,
        &ComparePathsOptions {
            use_case_sensitive_file_names: true,
            current_directory: current_directory.to_owned(),
        },
    )
}

pub fn compare_paths_case_insensitive(a: &str, b: &str, current_directory: &str) -> Ordering {
    compare_paths(
        a,
        b,
        &ComparePathsOptions {
            use_case_sensitive_file_names: false,
            current_directory: current_directory.to_owned(),
        },
    )
}

pub fn contains_path(parent: &str, child: &str, options: &ComparePathsOptions) -> bool {
    let parent = combine_paths(&options.current_directory, &[parent]);
    let child = combine_paths(&options.current_directory, &[child]);
    if parent.is_empty() || child.is_empty() {
        return false;
    }
    if parent == child {
        return true;
    }

    let parent = reduce_path_components(&get_path_components(&parent, ""));
    let child = reduce_path_components(&get_path_components(&child, ""));
    child.len() >= parent.len()
        && parent.iter().enumerate().all(|(index, component)| {
            if index == 0 {
                to_file_name_lower_case(component) == to_file_name_lower_case(&child[index])
            } else {
                options.equal_strings(component, &child[index])
            }
        })
}

pub fn path_contains_path(parent: &Path, child: &Path) -> bool {
    !parent.is_empty()
        && (parent == child
            || child.len() > parent.len()
                && child.starts_with(parent)
                && (parent.ends_with('/') || child.as_bytes()[parent.len()] == b'/'))
}

pub fn file_extension_is(path: &str, extension: &str) -> bool {
    path.len() > extension.len() && path.ends_with(extension)
}

pub fn for_each_ancestor_directory<T>(
    mut directory: String,
    mut callback: impl FnMut(&str) -> Option<T>,
) -> Option<T> {
    loop {
        if let Some(result) = callback(&directory) {
            return Some(result);
        }
        let parent = get_directory_path(&directory);
        if parent == directory {
            return None;
        }
        directory = parent;
    }
}

pub fn for_each_ancestor_directory_stopping_at_global_cache<T>(
    global_cache_location: &str,
    directory: String,
    mut callback: impl FnMut(&str) -> (T, bool),
) -> Option<T> {
    for_each_ancestor_directory(directory, |ancestor| {
        let (result, stop) = callback(ancestor);
        if stop || ancestor == global_cache_location {
            Some(result)
        } else {
            None
        }
    })
}

pub fn for_each_ancestor_directory_path<T>(
    directory: Path,
    mut callback: impl FnMut(&Path) -> Option<T>,
) -> Option<T> {
    for_each_ancestor_directory(directory, |ancestor| callback(&ancestor.to_owned()))
}

pub fn has_extension(file_name: &str) -> bool {
    get_base_file_name(file_name).contains('.')
}

pub fn split_volume_path(path: &str) -> Option<(String, String)> {
    let bytes = path.as_bytes();
    if bytes.len() >= 2 && is_volume_character(bytes[0]) && bytes[1] == b':' {
        Some((path[..2].to_ascii_lowercase(), path[2..].to_owned()))
    } else {
        None
    }
}

pub fn get_common_parents(
    paths: &[String],
    min_components: usize,
    options: &ComparePathsOptions,
) -> (Vec<String>, BTreeSet<String>) {
    assert!(min_components >= 1, "minComponents must be at least 1");
    if paths.is_empty() {
        return (Vec::new(), BTreeSet::new());
    }
    if paths.len() == 1 {
        let components =
            reduce_path_components(&get_path_components(&paths[0], &options.current_directory));
        if components.len() < min_components {
            return (Vec::new(), BTreeSet::from([paths[0].clone()]));
        }
        return (paths.to_vec(), BTreeSet::new());
    }

    let mut ignored = BTreeSet::new();
    let mut path_components = Vec::with_capacity(paths.len());
    for path in paths {
        let components =
            reduce_path_components(&get_path_components(path, &options.current_directory));
        if components.len() < min_components {
            ignored.insert(path.clone());
        } else {
            path_components.push(components);
        }
    }
    let parents = get_common_parents_worker(&path_components, min_components, options)
        .into_iter()
        .map(|components| get_path_from_path_components(&components))
        .collect();
    (parents, ignored)
}

fn get_common_parents_worker(
    component_groups: &[Vec<String>],
    min_components: usize,
    options: &ComparePathsOptions,
) -> Vec<Vec<String>> {
    if component_groups.is_empty() {
        return Vec::new();
    }

    let max_depth = component_groups
        .iter()
        .map(Vec::len)
        .min()
        .unwrap_or_default();
    for last_common_index in 0..max_depth {
        let candidate = &component_groups[0][last_common_index];
        for components in component_groups.iter().skip(1) {
            if options.equal_strings(candidate, &components[last_common_index]) {
                continue;
            }
            if last_common_index < min_components {
                let mut grouped: BTreeMap<Path, (Vec<String>, Vec<Vec<String>>)> = BTreeMap::new();
                for group in component_groups {
                    let key = to_path(
                        &group[last_common_index],
                        &options.current_directory,
                        options.use_case_sensitive_file_names,
                    );
                    let entry = grouped
                        .entry(key)
                        .or_insert_with(|| (group[..=last_common_index].to_vec(), Vec::new()));
                    entry.1.push(group[last_common_index + 1..].to_vec());
                }

                let mut result = Vec::new();
                for (_, (head, tails)) in grouped {
                    let sub_results = get_common_parents_worker(
                        &tails,
                        min_components - (last_common_index + 1),
                        options,
                    );
                    for mut sub_result in sub_results {
                        let mut parent = head.clone();
                        parent.append(&mut sub_result);
                        result.push(parent);
                    }
                }
                return result;
            }
            return vec![component_groups[0][..last_common_index].to_vec()];
        }
    }

    vec![component_groups[0][..max_depth].to_vec()]
}

pub fn starts_with_directory(
    file_name: &str,
    directory_name: &str,
    use_case_sensitive_file_names: bool,
) -> bool {
    if directory_name.is_empty() {
        return false;
    }
    let file_name = get_canonical_file_name(file_name, use_case_sensitive_file_names);
    let directory_name = get_canonical_file_name(directory_name, use_case_sensitive_file_names);
    let directory_name = directory_name.trim_end_matches(['/', '\\']);
    file_name.starts_with(&format!("{directory_name}/"))
        || file_name.starts_with(&format!("{directory_name}\\"))
}

pub fn compare_number_of_directory_separators(path1: &str, path2: &str) -> Ordering {
    path1.matches('/').count().cmp(&path2.matches('/').count())
}
