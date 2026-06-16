use ts_tspath as tspath;

pub const LIB_FOLDER: &str = "built/local/";
pub const BUILT_FOLDER: &str = "/.ts";

const TEST_PATH_PREFIX_REPLACEMENTS: [(&str, &str); 7] = [
    ("/.ts/", ""),
    ("/.lib/", ""),
    ("/.src/", ""),
    ("bundled:///libs/", ""),
    ("file:///./ts/", "file:///"),
    ("file:///./lib/", "file:///"),
    ("file:///./src/", "file:///"),
];

const TEST_PATH_TRAILING_REPLACEMENTS: [(&str, &str); 7] = [
    ("/.ts/", "/"),
    ("/.lib/", "/"),
    ("/.src/", "/"),
    ("bundled:///libs/", "/"),
    ("file:///./ts/", "file:///"),
    ("file:///./lib/", "file:///"),
    ("file:///./src/", "file:///"),
];

pub fn remove_test_path_prefixes(text: &str, retain_trailing_directory_separator: bool) -> String {
    let replacements = if retain_trailing_directory_separator {
        TEST_PATH_TRAILING_REPLACEMENTS
    } else {
        TEST_PATH_PREFIX_REPLACEMENTS
    };

    replace_all_string_replacer(text, &replacements)
}

pub fn is_default_library_file(file_path: &str) -> bool {
    let file_name = tspath::get_base_file_name(file_path);
    file_name.starts_with("lib.") && file_name.ends_with(tspath::EXTENSION_DTS)
}

pub fn is_built_file(file_path: &str) -> bool {
    file_path.starts_with(LIB_FOLDER)
        || file_path.starts_with(&tspath::ensure_trailing_directory_separator(BUILT_FOLDER))
}

pub fn is_ts_config_file(path: &str) -> bool {
    path.contains("tsconfig") && path.contains("json")
}

pub fn sanitize_test_file_path(name: &str) -> String {
    let mut path = name
        .chars()
        .map(|ch| if "^<>:\"|?*%".contains(ch) { '_' } else { ch })
        .collect::<String>()
        .replace('\\', "/");
    path = tspath::normalize_slashes(&path);
    path = path.replace("../", "__dotdot/");
    let path = tspath::to_path(&path, "", false);
    path.strip_prefix('/').unwrap_or(&path).to_string()
}

pub fn split_lines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, _) in text.match_indices('\n') {
        let end = if index > start && text.as_bytes()[index - 1] == b'\r' {
            index - 1
        } else {
            index
        };
        lines.push(&text[start..end]);
        start = index + 1;
    }
    lines.push(&text[start..]);
    lines
}

pub fn replace_ts_extension(path: &str, replacement: &str) -> String {
    for ext in [".tsx", ".ts"] {
        if let Some(prefix) = path.strip_suffix(ext) {
            return format!("{prefix}{replacement}");
        }
    }
    path.to_string()
}

pub fn replace_non_whitespace_with_space(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if matches!(ch, '\t' | '\n' | '\u{000C}' | '\r' | ' ') {
                ch
            } else {
                ' '
            }
        })
        .collect()
}

fn replace_all_string_replacer(text: &str, replacements: &[(&str, &str)]) -> String {
    let mut result = String::with_capacity(text.len());
    let mut index = 0;
    while index < text.len() {
        let rest = &text[index..];
        if let Some((from, to)) = replacements.iter().find(|(from, _)| rest.starts_with(from)) {
            result.push_str(to);
            index += from.len();
        } else {
            let ch = rest.chars().next().unwrap();
            result.push(ch);
            index += ch.len_utf8();
        }
    }
    result
}
