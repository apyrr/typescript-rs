use ts_core::TextPos;

pub struct ECMALineInfo {
    text: String,
    line_starts: Vec<TextPos>,
}

pub fn create_ecma_line_info(text: String, line_starts: Vec<TextPos>) -> ECMALineInfo {
    ECMALineInfo { text, line_starts }
}

impl ECMALineInfo {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn line_starts(&self) -> &[TextPos] {
        &self.line_starts
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    pub fn line_text(&self, line: usize) -> &str {
        let pos = self.line_starts[line] as usize;
        let end = if line + 1 < self.line_starts.len() {
            self.line_starts[line + 1] as usize
        } else {
            self.text.len()
        };
        &self.text[pos..end]
    }
}
