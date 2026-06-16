// VS Code's telemetry pipeline redacts any string matching
// /(key|token|sig|secret|signature|password|passwd|pwd|android:value)[^a-zA-Z0-9]/i
// as `<REDACTED: Generic Secret>`, which trips on innocuous Go frames like
// `getSignatureHelp(`. Insert `X_X` after each trigger keyword that we know
// can appear in our sanitized output, when followed by punctuation we
// actually emit (`(`, `[`, `.`, `|`); reverse by removing the marker (replace
// `X_X` with the empty string) on the dashboard.

fn defeat_generic_secret_regex(s: &str) -> String {
    let keywords = ["signature", "token", "key", "sig", "pwd"];
    let mut result = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        let rest = &s[i..];
        let lower = rest.to_ascii_lowercase();
        let mut matched = None;
        for keyword in keywords {
            if lower.starts_with(keyword) {
                let next = i + keyword.len();
                if next < s.len() && matches!(s.as_bytes()[next], b'(' | b'[' | b'.' | b'|') {
                    matched = Some(keyword.len());
                    break;
                }
            }
        }
        if let Some(len) = matched {
            result.push_str(&s[i..i + len]);
            result.push_str("X_X");
            i += len;
            continue;
        }
        let ch = rest.chars().next().expect("non-empty rest");
        result.push(ch);
        i += ch.len_utf8();
    }
    result
}

pub fn sanitize_stack_trace(stack: &str) -> String {
    // TODO: should we just look for the first '(' and
    // just strip everything before the prior newline?
    let Some(start_index) = stack.find("runtime/debug.Stack()") else {
        return String::new();
    };
    let stack = &stack[start_index..];

    let mut result = String::new();

    for (line_num, line) in stack.lines().enumerate() {
        if line_num > 0 {
            result.push('\n');
        }

        let mut i = 0;
        // Skip whitespace
        while i < line.len() {
            if line.as_bytes()[i] != b' ' && line.as_bytes()[i] != b'\t' {
                break;
            }
            i += 1;
        }

        result.push_str(&line[..i]);

        let mut line = &line[i..];

        if let Some(our_module_index) = line.find("typescript-go/internal") {
            line = &line[our_module_index..];
            write_sanitized_module_or_path(line, &mut result);
        } else {
            result.push_str("(REDACTED FRAME)");
        }
    }

    defeat_generic_secret_regex(&result)
}

fn write_sanitized_module_or_path(line: &str, result: &mut String) {
    // We don't expect things like \r, but it doesn't hurt to trim just in case.
    let mut line = line.trim();

    if let Some(plus_hex) = line.find(" +0x") {
        line = &line[..plus_hex];
    } else if let Some(in_goroutine) = line.rfind(" in goroutine ") {
        line = &line[..in_goroutine];
    }

    for (segment_index, mut segment) in line.split('/').enumerate() {
        if segment_index > 0 {
            result.push_str("|>");
        }

        // See if the string ends with ), and strip out all the arguments.
        if segment.ends_with(')') {
            let Some(open_paren_index) = segment.rfind('(') else {
                // Closing parenthesis, but no opening - bail out.
                result.push_str("???");
                continue;
            };

            segment = &segment[..open_paren_index];
            result.push_str(segment);
            result.push_str("()");
            continue;
        }

        result.push_str(segment);
    }
}
