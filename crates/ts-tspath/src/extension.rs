use ts_stringutil::get_string_equality_comparer;

pub type Path = String;

pub const EXTENSION_TS: &str = ".ts";
pub const EXTENSION_TSX: &str = ".tsx";
pub const EXTENSION_DTS: &str = ".d.ts";
pub const EXTENSION_JS: &str = ".js";
pub const EXTENSION_JSX: &str = ".jsx";
pub const EXTENSION_JSON: &str = ".json";
pub const EXTENSION_TS_BUILD_INFO: &str = ".tsbuildinfo";
pub const EXTENSION_MJS: &str = ".mjs";
pub const EXTENSION_MTS: &str = ".mts";
pub const EXTENSION_DMTS: &str = ".d.mts";
pub const EXTENSION_CJS: &str = ".cjs";
pub const EXTENSION_CTS: &str = ".cts";
pub const EXTENSION_DCTS: &str = ".d.cts";

pub const SUPPORTED_DECLARATION_EXTENSIONS: &[&str] =
    &[EXTENSION_DTS, EXTENSION_DCTS, EXTENSION_DMTS];
pub const SUPPORTED_TS_IMPLEMENTATION_EXTENSIONS: &[&str] =
    &[EXTENSION_TS, EXTENSION_TSX, EXTENSION_MTS, EXTENSION_CTS];
const SUPPORTED_TS_EXTENSIONS_FOR_EXTRACT_EXTENSION: &[&str] = &[
    EXTENSION_DTS,
    EXTENSION_DCTS,
    EXTENSION_DMTS,
    EXTENSION_TS,
    EXTENSION_TSX,
    EXTENSION_MTS,
    EXTENSION_CTS,
];
pub const ALL_SUPPORTED_EXTENSIONS: &[&[&str]] = &[
    &[
        EXTENSION_TS,
        EXTENSION_TSX,
        EXTENSION_DTS,
        EXTENSION_JS,
        EXTENSION_JSX,
    ],
    &[EXTENSION_CTS, EXTENSION_DCTS, EXTENSION_CJS],
    &[EXTENSION_MTS, EXTENSION_DMTS, EXTENSION_MJS],
];
pub const SUPPORTED_TS_EXTENSIONS: &[&[&str]] = &[
    &[EXTENSION_TS, EXTENSION_TSX, EXTENSION_DTS],
    &[EXTENSION_CTS, EXTENSION_DCTS],
    &[EXTENSION_MTS, EXTENSION_DMTS],
];
pub const SUPPORTED_TS_EXTENSIONS_FLAT: &[&str] = &[
    EXTENSION_TS,
    EXTENSION_TSX,
    EXTENSION_DTS,
    EXTENSION_CTS,
    EXTENSION_DCTS,
    EXTENSION_MTS,
    EXTENSION_DMTS,
];
pub const SUPPORTED_JS_EXTENSIONS: &[&[&str]] = &[
    &[EXTENSION_JS, EXTENSION_JSX],
    &[EXTENSION_MJS],
    &[EXTENSION_CJS],
];
pub const SUPPORTED_JS_EXTENSIONS_FLAT: &[&str] =
    &[EXTENSION_JS, EXTENSION_JSX, EXTENSION_MJS, EXTENSION_CJS];
pub const EXTENSIONS_NOT_SUPPORTING_EXTENSIONLESS_RESOLUTION: &[&str] = &[
    EXTENSION_MTS,
    EXTENSION_DMTS,
    EXTENSION_MJS,
    EXTENSION_CTS,
    EXTENSION_DCTS,
    EXTENSION_CJS,
];

pub fn all_supported_extensions_with_json() -> Vec<Vec<&'static str>> {
    let mut result = ALL_SUPPORTED_EXTENSIONS
        .iter()
        .map(|extensions| extensions.to_vec())
        .collect::<Vec<_>>();
    result.push(vec![EXTENSION_JSON]);
    result
}

pub fn supported_ts_extensions_with_json() -> Vec<Vec<&'static str>> {
    let mut result = SUPPORTED_TS_EXTENSIONS
        .iter()
        .map(|extensions| extensions.to_vec())
        .collect::<Vec<_>>();
    result.push(vec![EXTENSION_JSON]);
    result
}

pub fn supported_ts_extensions_with_json_flat() -> Vec<&'static str> {
    let mut result = SUPPORTED_TS_EXTENSIONS_FLAT.to_vec();
    result.push(EXTENSION_JSON);
    result
}

pub fn extension_is_ts(ext: &str) -> bool {
    ext == EXTENSION_TS
        || ext == EXTENSION_TSX
        || ext == EXTENSION_DTS
        || ext == EXTENSION_MTS
        || ext == EXTENSION_DMTS
        || ext == EXTENSION_CTS
        || ext == EXTENSION_DCTS
        || ext.len() >= 7 && ext.starts_with(".d.") && ext.ends_with(".ts")
}

const EXTENSIONS_TO_REMOVE: &[&str] = &[
    EXTENSION_DTS,
    EXTENSION_DMTS,
    EXTENSION_DCTS,
    EXTENSION_MJS,
    EXTENSION_MTS,
    EXTENSION_CJS,
    EXTENSION_CTS,
    EXTENSION_TS,
    EXTENSION_JS,
    EXTENSION_TSX,
    EXTENSION_JSX,
    EXTENSION_JSON,
];

pub fn remove_file_extension(path: &str) -> String {
    // Remove any known extension even if it has more than one dot
    for ext in EXTENSIONS_TO_REMOVE {
        if let Some(path) = path.strip_suffix(ext) {
            return path.to_owned();
        }
    }
    // Otherwise just remove single dot extension, if any
    let ext = get_any_extension_from_path(path, None, false);
    path[..path.len() - ext.len()].to_owned()
}

pub fn try_get_extension_from_path(p: &str) -> &'static str {
    for ext in EXTENSIONS_TO_REMOVE {
        if file_extension_is(p, ext) {
            return ext;
        }
    }
    ""
}

pub fn remove_extension(path: &str, extension: &str) -> String {
    path[..path.len() - extension.len()].to_owned()
}

pub fn file_extension_is_one_of(path: &str, extensions: &[&str]) -> bool {
    for ext in extensions {
        if file_extension_is(path, ext) {
            return true;
        }
    }
    false
}

pub fn try_extract_ts_extension(file_name: &str) -> &'static str {
    for ext in SUPPORTED_TS_EXTENSIONS_FOR_EXTRACT_EXTENSION {
        if file_extension_is(file_name, ext) {
            return ext;
        }
    }
    ""
}

pub fn has_ts_file_extension(path: &str) -> bool {
    file_extension_is_one_of(path, SUPPORTED_TS_EXTENSIONS_FLAT)
}

pub fn has_implementation_ts_file_extension(path: &str) -> bool {
    file_extension_is_one_of(path, SUPPORTED_TS_IMPLEMENTATION_EXTENSIONS)
        && !is_declaration_file_name(path)
}

pub fn has_js_file_extension(path: &str) -> bool {
    file_extension_is_one_of(path, SUPPORTED_JS_EXTENSIONS_FLAT)
}

pub fn has_json_file_extension(path: &str) -> bool {
    file_extension_is(path, EXTENSION_JSON)
}

pub fn is_declaration_file_name(file_name: &str) -> bool {
    !get_declaration_file_extension(file_name).is_empty()
}

pub fn extension_is_one_of(ext: &str, extensions: &[&str]) -> bool {
    extensions.contains(&ext)
}

pub fn get_declaration_file_extension(file_name: &str) -> String {
    let base = get_base_file_name(file_name);
    for ext in SUPPORTED_DECLARATION_EXTENSIONS {
        if base.ends_with(ext) {
            return (*ext).to_owned();
        }
    }
    if base.ends_with(EXTENSION_TS)
        && let Some(index) = base.find(".d.")
    {
        return base[index..].to_owned();
    }
    String::new()
}

pub fn get_declaration_emit_extension_for_path(path: &str) -> String {
    if file_extension_is_one_of(path, &[EXTENSION_MJS, EXTENSION_MTS]) {
        return EXTENSION_DMTS.to_owned();
    }
    if file_extension_is_one_of(path, &[EXTENSION_CJS, EXTENSION_CTS]) {
        return EXTENSION_DCTS.to_owned();
    }
    if file_extension_is_one_of(
        path,
        &[EXTENSION_TS, EXTENSION_TSX, EXTENSION_JS, EXTENSION_JSX],
    ) {
        return EXTENSION_DTS.to_owned();
    }
    let ext = get_any_extension_from_path(path, None, false);
    if !ext.is_empty() {
        return format!(".d{ext}.ts");
    }
    EXTENSION_DTS.to_owned()
}

// ChangeAnyExtension changes the extension of a path to the provided extension if it has one of the provided extensions.
//
// ChangeAnyExtension("/path/to/file.ext", ".js", ".ext") === "/path/to/file.js"
// ChangeAnyExtension("/path/to/file.ext", ".js", ".ts") === "/path/to/file.ext"
// ChangeAnyExtension("/path/to/file.ext", ".js", [".ext", ".ts"]) === "/path/to/file.js"
pub fn change_any_extension(
    path: &str,
    ext: &str,
    extensions: Option<&[&str]>,
    ignore_case: bool,
) -> String {
    let path_ext = get_any_extension_from_path(path, extensions, ignore_case);
    if !path_ext.is_empty() {
        let result = &path[..path.len() - path_ext.len()];
        if ext.is_empty() {
            return result.to_owned();
        }
        if ext.starts_with('.') {
            return format!("{result}{ext}");
        }
        return format!("{result}.{ext}");
    }
    path.to_owned()
}

pub fn change_extension(path: &str, new_extension: &str) -> String {
    change_any_extension(path, new_extension, Some(EXTENSIONS_TO_REMOVE), false)
}

// Like `changeAnyExtension`, but declaration file extensions are recognized
// and replaced starting from the `.d`.
//
//	changeAnyExtension("file.d.ts", ".js") === "file.d.js"
//	changeFullExtension("file.d.ts", ".js") === "file.js"
pub fn change_full_extension(path: &str, new_extension: &str) -> String {
    let declaration_extension = get_declaration_file_extension(path);
    if !declaration_extension.is_empty() {
        let ext = if new_extension.starts_with('.') {
            new_extension.to_owned()
        } else {
            format!(".{new_extension}")
        };
        return format!(
            "{}{}",
            &path[..path.len() - declaration_extension.len()],
            ext
        );
    }
    change_extension(path, new_extension)
}

pub fn get_possible_original_input_extension_for_extension(path: &str) -> Vec<String> {
    if file_extension_is_one_of(path, &[EXTENSION_DMTS, EXTENSION_MJS, EXTENSION_MTS]) {
        return vec![EXTENSION_MTS.to_owned(), EXTENSION_MJS.to_owned()];
    }
    if file_extension_is_one_of(path, &[EXTENSION_DCTS, EXTENSION_CJS, EXTENSION_CTS]) {
        return vec![EXTENSION_CTS.to_owned(), EXTENSION_CJS.to_owned()];
    }
    // Handle any custom .d.x.ts extension (e.g., .d.json.ts -> .json, .d.css.ts -> .css)
    let ext = get_declaration_file_extension(path);
    if !ext.is_empty() && ext != EXTENSION_DTS {
        let inner = &ext[".d.".len()..ext.len() - ".ts".len()];
        return vec![format!(".{inner}")];
    }
    vec![
        EXTENSION_TSX.to_owned(),
        EXTENSION_TS.to_owned(),
        EXTENSION_JSX.to_owned(),
        EXTENSION_JS.to_owned(),
    ]
}

pub fn normalize_slashes(path: &str) -> String {
    path.replace('\\', "/")
}

fn is_any_directory_separator(ch: u8) -> bool {
    ch == b'/' || ch == b'\\'
}

pub fn is_rooted_disk_path(path: &str) -> bool {
    get_encoded_root_length(path) > 0
}

pub fn has_trailing_directory_separator(path: &str) -> bool {
    path.as_bytes()
        .last()
        .is_some_and(|ch| *ch == b'/' || *ch == b'\\')
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

fn get_base_file_name(path: &str) -> String {
    let path = normalize_slashes(path);

    // if the path provided is itself the root, then it has no file name.
    let root_length = get_root_length(&path);
    if root_length == path.len() {
        return String::new();
    }

    // return the trailing portion of the path starting after the last (non-terminal) directory
    // separator but not including any trailing directory separator.
    let path = remove_trailing_directory_separator(&path);
    let last_separator = path.rfind('/').map(|i| i + 1).unwrap_or(0);
    path[root_length.max(last_separator)..].to_owned()
}

fn get_any_extension_from_path(
    path: &str,
    extensions: Option<&[&str]>,
    ignore_case: bool,
) -> String {
    // Retrieves any string from the final "." onwards from a base file name.
    // Unlike extensionFromPath, which throws an exception on unrecognized extensions.
    if let Some(extensions) = extensions
        && !extensions.is_empty()
    {
        return get_any_extension_from_path_worker(
            remove_trailing_directory_separator(path),
            extensions,
            get_string_equality_comparer(ignore_case),
        );
    }

    let base_file_name = get_base_file_name(path);
    if let Some(extension_index) = base_file_name.rfind('.') {
        return base_file_name[extension_index..].to_owned();
    }
    String::new()
}

fn get_any_extension_from_path_worker(
    path: &str,
    extensions: &[&str],
    string_equality_comparer: fn(&str, &str) -> bool,
) -> String {
    for extension in extensions {
        let result =
            try_get_specific_extension_from_path(path, extension, string_equality_comparer);
        if !result.is_empty() {
            return result;
        }
    }
    String::new()
}

fn try_get_specific_extension_from_path(
    path: &str,
    extension: &str,
    string_equality_comparer: fn(&str, &str) -> bool,
) -> String {
    let owned_extension;
    let extension = if extension.starts_with('.') {
        extension
    } else {
        owned_extension = format!(".{extension}");
        &owned_extension
    };
    if path.len() >= extension.len() && path.as_bytes()[path.len() - extension.len()] == b'.' {
        let path_extension = &path[path.len() - extension.len()..];
        if string_equality_comparer(path_extension, extension) {
            return path_extension.to_owned();
        }
    }
    String::new()
}

fn file_extension_is(path: &str, extension: &str) -> bool {
    path.len() > extension.len() && path.ends_with(extension)
}

fn is_volume_character(ch: u8) -> bool {
    ch.is_ascii_alphabetic()
}

fn get_file_url_volume_separator_end(url: &str, start: usize) -> Option<usize> {
    let bytes = url.as_bytes();
    if bytes.len() <= start {
        return None;
    }
    let ch0 = bytes[start];
    if ch0 == b':' {
        return Some(start + 1);
    }
    if ch0 == b'%' && bytes.len() > start + 2 && bytes[start + 1] == b'3' {
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

    // POSIX or UNC
    if ch0 == b'/' || ch0 == b'\\' {
        if len == 1 || bytes[1] != ch0 {
            return 1; // POSIX: "/" (or non-normalized "\")
        }

        let offset = 2;
        if let Some(p1) = path[offset..].find(ch0 as char) {
            return (p1 + offset + 1) as isize; // UNC: "//server/" or "\\server\"
        }
        return len as isize; // UNC: "//server" or "\\server"
    }

    // DOS
    if is_volume_character(ch0) && len > 1 && bytes[1] == b':' {
        if len == 2 {
            return 2; // DOS: "c:" (but not "c:d")
        }
        let ch2 = bytes[2];
        if ch2 == b'/' || ch2 == b'\\' {
            return 3; // DOS: "c:/" or "c:\"
        }
    }

    // Untitled paths (e.g., "^/untitled/ts-nul-authority/Untitled-1")
    if ch0 == b'^' && len > 1 && bytes[1] == b'/' {
        return 2; // Untitled: "^/"
    }

    // URL
    if let Some(scheme_end) = path.find("://") {
        let authority_start = scheme_end + "://".len();
        if let Some(authority_length) = path[authority_start..].find('/') {
            // URL: "file:///", "file://server/", "file://server/path"
            let authority_end = authority_start + authority_length;

            // For local "file" URLs, include the leading DOS volume (if present).
            // Per https://www.ietf.org/rfc/rfc1738.txt, a host of "" or "localhost" is a
            // special case interpreted as "the machine from which the URL is being interpreted".
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
                    // URL: "file:///c:", "file://localhost/c:", "file:///c$3a", "file://localhost/c%3a"
                    // but not "file:///c:d" or "file:///c%3ad"
                    return !(volume_separator_end as isize);
                }
                if bytes[volume_separator_end] == b'/' {
                    // URL: "file:///c:/", "file://localhost/c:/", "file:///c%3a/", "file://localhost/c%3a/"
                    return !((volume_separator_end + 1) as isize);
                }
            }
            return !((authority_end + 1) as isize); // URL: "file://server/", "http://server/"
        }
        return !(len as isize); // URL: "file://server", "http://server"
    }

    // relative
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

pub fn combine_paths(first_path: &str, paths: &[&str]) -> String {
    let first_path = normalize_slashes(first_path);
    let size = first_path.len() + paths.iter().map(|path| path.len() + 1).sum::<usize>();
    let mut result = String::with_capacity(size);
    result.push_str(&first_path);

    for trailing_path in paths {
        if trailing_path.is_empty() {
            continue;
        }
        let trailing_path = normalize_slashes(trailing_path);
        if result.is_empty() || get_root_length(&trailing_path) != 0 {
            result = trailing_path;
        } else {
            if !has_trailing_directory_separator(&result) {
                result.push('/');
            }
            result.push_str(&trailing_path);
        }
    }

    result
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
    let mut file_name = if get_root_length(file_name) == 0 && !current_directory.is_empty() {
        combine_paths(current_directory, &[file_name])
    } else {
        normalize_slashes(file_name)
    };

    let root_length = get_root_length(&file_name);
    if let Some(simple_normalized) = simple_normalize_path(&file_name) {
        let length = simple_normalized.len();
        if length > root_length {
            return remove_trailing_directory_separator(&simple_normalized).to_owned();
        }
        if length == root_length && root_length != 0 {
            return ensure_trailing_directory_separator(&simple_normalized);
        }
        return simple_normalized;
    }

    let length = file_name.len();
    let root = file_name[..root_length].to_owned();
    let mut changed = false;
    let mut normalized = String::new();
    let mut index = root_length;
    let mut normalized_up_to = index;
    let mut seen_non_dot_dot_segment = root_length != 0;
    while index < length {
        let segment_start = index;
        while index < length && file_name.as_bytes()[index] == b'/' {
            index += 1;
        }
        let mut segment_start = segment_start;
        if index > segment_start {
            if !changed {
                let end = root_length.max(segment_start.saturating_sub(1));
                normalized = file_name[..end].to_owned();
                changed = true;
            }
            if index == length {
                break;
            }
            segment_start = index;
        }

        let segment_end = file_name[index + 1..]
            .find('/')
            .map_or(length, |offset| index + 1 + offset);
        let segment = &file_name[segment_start..segment_end];
        if segment == "." {
            if !changed {
                normalized = file_name[..normalized_up_to].to_owned();
                changed = true;
            }
        } else if segment == ".." {
            if !seen_non_dot_dot_segment {
                if changed {
                    if normalized.len() == root_length {
                        normalized.push_str("..");
                    } else {
                        normalized.push_str("/..");
                    }
                } else {
                    normalized_up_to = index + 2;
                }
            } else if !changed {
                let end = if normalized_up_to > 0 {
                    file_name[..normalized_up_to - 1]
                        .rfind('/')
                        .map_or(root_length, |slash| root_length.max(slash))
                } else {
                    normalized_up_to
                };
                normalized = file_name[..end].to_owned();
                changed = true;
                seen_non_dot_dot_segment = (normalized.len() != root_length || root_length != 0)
                    && normalized != ".."
                    && !normalized.ends_with("/..");
            } else {
                if let Some(last_slash) = normalized.rfind('/') {
                    let end = root_length.max(last_slash);
                    normalized.truncate(end);
                } else {
                    normalized = root.clone();
                }
                seen_non_dot_dot_segment = (normalized.len() != root_length || root_length != 0)
                    && normalized != ".."
                    && !normalized.ends_with("/..");
            }
        } else if changed {
            if normalized.len() != root_length {
                normalized.push('/');
            }
            seen_non_dot_dot_segment = true;
            normalized.push_str(segment);
        } else {
            seen_non_dot_dot_segment = true;
            normalized_up_to = segment_end;
        }
        index = segment_end + 1;
    }
    if changed {
        return normalized;
    }
    if length > root_length {
        return remove_trailing_directory_separators(&file_name).to_owned();
    }
    if length == root_length {
        return ensure_trailing_directory_separator(&file_name);
    }
    file_name.clear();
    file_name
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

    let mut prev_slash = false;
    let mut segment_len = 0usize;
    let mut dot_count: isize = 0;
    for ch in bytes {
        if *ch == b'/' {
            if prev_slash {
                return true;
            }
            if (segment_len == 1 && dot_count == 1) || (segment_len == 2 && dot_count == 2) {
                return true;
            }
            prev_slash = true;
            segment_len = 0;
            dot_count = 0;
            continue;
        }

        if *ch == b'.' {
            if dot_count >= 0 {
                dot_count += 1;
            }
        } else {
            dot_count = -1;
        }
        segment_len += 1;
        prev_slash = false;
    }

    (segment_len == 1 && dot_count == 1) || (segment_len == 2 && dot_count == 2)
}

pub fn normalize_path(path: &str) -> String {
    let path = normalize_slashes(path);
    if let Some(normalized) = simple_normalize_path(&path) {
        return normalized;
    }
    let mut normalized = get_normalized_absolute_path(&path, "");
    if !normalized.is_empty() && has_trailing_directory_separator(&path) {
        normalized = ensure_trailing_directory_separator(&normalized);
    }
    normalized
}

pub fn to_file_name_lower_case(file_name: &str) -> String {
    const I_WITH_DOT: char = '\u{0130}';
    if file_name.is_ascii() {
        if !file_name.bytes().any(|ch| ch.is_ascii_uppercase()) {
            return file_name.to_owned();
        }
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

pub fn get_canonical_file_name(file_name: &str, use_case_sensitive_file_names: bool) -> String {
    if use_case_sensitive_file_names {
        file_name.to_owned()
    } else {
        to_file_name_lower_case(file_name)
    }
}

pub fn to_path(file_name: &str, base_path: &str, use_case_sensitive_file_names: bool) -> Path {
    let non_canonicalized_path = if is_rooted_disk_path(file_name) {
        normalize_path(file_name)
    } else {
        get_normalized_absolute_path(file_name, base_path)
    };
    get_canonical_file_name(&non_canonicalized_path, use_case_sensitive_file_names)
}

pub fn starts_with_directory(
    file_name: &str,
    directory_name: &str,
    use_case_sensitive_file_names: bool,
) -> bool {
    if directory_name.is_empty() {
        return false;
    }

    let canonical_file_name = get_canonical_file_name(file_name, use_case_sensitive_file_names);
    let mut canonical_directory_name =
        get_canonical_file_name(directory_name, use_case_sensitive_file_names);
    while canonical_directory_name
        .as_bytes()
        .last()
        .is_some_and(|ch| is_any_directory_separator(*ch))
    {
        canonical_directory_name.pop();
    }

    canonical_file_name.starts_with(&format!("{canonical_directory_name}/"))
        || canonical_file_name.starts_with(&format!("{canonical_directory_name}\\"))
}
