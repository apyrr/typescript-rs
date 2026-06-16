use crate::ECMALineInfo;

// Tries to find the sourceMappingURL comment at the end of a file.
pub fn try_get_source_mapping_url(line_info: Option<&ECMALineInfo>) -> String {
    if let Some(line_info) = line_info {
        for index in (0..line_info.line_count()).rev() {
            let mut line = line_info.line_text(index).trim_start().to_string();
            line = line.trim_end_matches(['\r', '\n']).to_string();
            if line.is_empty() {
                continue;
            }
            let bytes = line.as_bytes();
            if bytes.len() < 4
                || !line.starts_with("//")
                || (bytes[2] != b'#' && bytes[2] != b'@')
                || bytes[3] != b' '
            {
                break;
            }
            if let Some(url) = line[4..].strip_prefix("sourceMappingURL=") {
                return url.trim_end().to_string();
            }
        }
    }
    String::new()
}
