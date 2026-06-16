use ts_ast::Kind;

struct StringTable {
    file_text: String,
    other_strings: String,
    // offsets are pos/end pairs
    offsets: Vec<u32>,
}

fn new_string_table(file_text: String, string_count: usize) -> StringTable {
    StringTable {
        file_text,
        other_strings: String::new(),
        offsets: Vec::with_capacity(string_count * 2),
    }
}

impl StringTable {
    fn add(&mut self, text: &str, kind: Kind, pos: i32, end: i32) -> u32 {
        let index = self.offsets.len() as u32;
        if kind == Kind::SourceFile {
            self.offsets.push(pos as u32);
            self.offsets.push(end as u32);
            return index;
        }

        let length = text.len();
        if end - pos > 0 && end as usize <= self.file_text.len() {
            // pos includes leading trivia, but we can usually infer the actual start of the
            // string from the kind and end
            let mut end_offset = 0;
            if kind == Kind::StringLiteral
                || kind == Kind::TemplateTail
                || kind == Kind::NoSubstitutionTemplateLiteral
            {
                end_offset = 1;
            }
            let end = (end - end_offset) as usize;
            let start = end - length;
            let file_slice = &self.file_text.as_bytes()[start..end];
            if file_slice == text.as_bytes() {
                self.offsets.push(start as u32);
                self.offsets.push(end as u32);
                return index;
            }
        }

        // no exact match, so we need to add it to the string table
        let offset = self.file_text.len() + self.other_strings.len();
        self.other_strings.push_str(text);
        self.offsets.push(offset as u32);
        self.offsets.push((offset + length) as u32);
        index
    }

    fn encode(&self) -> Vec<u8> {
        let mut result = Vec::with_capacity(self.encoded_length());
        append_u32s(&mut result, &self.offsets);
        result.extend_from_slice(self.file_text.as_bytes());
        result.extend_from_slice(self.other_strings.as_bytes());
        result
    }

    fn string_length(&self) -> usize {
        self.file_text.len() + self.other_strings.len()
    }

    fn encoded_length(&self) -> usize {
        self.offsets.len() * 4 + self.file_text.len() + self.other_strings.len()
    }
}

fn append_u32s(result: &mut Vec<u8>, values: &[u32]) {
    for value in values {
        result.extend_from_slice(&value.to_le_bytes());
    }
}
