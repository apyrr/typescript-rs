use ts_core::TextPos;

pub type LspLineStarts = Vec<TextPos>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LspLineMap {
    pub line_starts: LspLineStarts,
    // TODO(jakebailey): collect ascii-only info per line
    pub ascii_only: bool,
}

pub fn compute_lsp_line_starts(text: &str) -> LspLineMap {
    // This is like core.ComputeLineStarts, but only considers "\n", "\r", and "\r\n" as line breaks,
    // and reports when the text is ASCII-only.
    let mut line_starts = Vec::with_capacity(text.bytes().filter(|b| *b == b'\n').count() + 1);
    let mut ascii_only = true;

    let bytes = text.as_bytes();
    let text_len = bytes.len();
    let mut pos = 0usize;
    let mut line_start = 0usize;
    while pos < text_len {
        let b = bytes[pos];
        if b < 0x80 {
            pos += 1;
            match b {
                b'\r' => {
                    if pos < text_len && bytes[pos] == b'\n' {
                        pos += 1;
                    }
                    line_starts.push(line_start as TextPos);
                    line_start = pos;
                }
                b'\n' => {
                    line_starts.push(line_start as TextPos);
                    line_start = pos;
                }
                _ => {}
            }
        } else {
            let size = text[pos..].chars().next().map(char::len_utf8).unwrap_or(1);
            pos += size;
            ascii_only = false;
        }
    }
    line_starts.push(line_start as TextPos);

    LspLineMap {
        line_starts,
        ascii_only,
    }
}

impl LspLineMap {
    pub fn compute_index_of_line_start(&self, target_pos: TextPos) -> usize {
        // port of computeLineOfPosition(lineStarts: readonly number[], position: number, lowerBound?: number): number {
        match self.line_starts.binary_search(&target_pos) {
            Ok(line_number) => line_number,
            Err(line_number) if line_number > 0 => {
                // If the actual position was not found, the binary search returns where the target line start would be inserted
                // if the target was in the slice.
                // e.g. if the line starts at [5, 10, 23, 80] and the position requested was 20
                // then the search will return (3, false).
                //
                // We want the index of the previous line start, so we subtract 1.
                line_number - 1
            }
            Err(line_number) => line_number,
        }
    }
}
