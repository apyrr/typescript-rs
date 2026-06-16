// PositionMap provides bidirectional mapping between UTF-8 byte offsets (used by Go)
// and UTF-16 code unit offsets (used by JavaScript/TypeScript).
//
// For ASCII-only text, the two are identical. For text containing non-ASCII characters,
// the offsets diverge because multi-byte UTF-8 sequences map to different numbers of
// UTF-16 code units:
//   - U+0000..U+007F:   1 byte  in UTF-8, 1 code unit  in UTF-16
//   - U+0080..U+07FF:   2 bytes in UTF-8, 1 code unit  in UTF-16
//   - U+0800..U+FFFF:   3 bytes in UTF-8, 1 code unit  in UTF-16
//   - U+10000..U+10FFFF: 4 bytes in UTF-8, 2 code units in UTF-16 (surrogate pair)
#[derive(Clone, Default)]
pub struct PositionMap {
    // asciiOnly is true if the text contains only ASCII characters,
    // meaning UTF-8 byte offsets and UTF-16 code unit offsets are identical.
    pub ascii_only: bool,
    // For each multi-byte character, we store:
    //   - the UTF-8 byte offset of the character
    //   - the cumulative delta (utf8Offset - utf16Offset) at that character
    // This allows O(log n) conversion in either direction.
    //
    // entries[i].utf8Pos is the byte offset of the i-th multi-byte character.
    // entries[i].delta is the total (utf8 - utf16) difference accumulated
    // through and including the i-th multi-byte character.
    entries: Vec<PositionMapEntry>,
}

#[derive(Clone, Copy)]
struct PositionMapEntry {
    utf8_pos: i32, // UTF-8 byte offset AFTER this multi-byte character
    delta: i32,    // cumulative (utf8 - utf16) offset difference after this character
}

// ComputePositionMap builds a PositionMap for the given text.
pub fn compute_position_map(text: &str) -> PositionMap {
    let mut pm = PositionMap::default();
    let mut delta = 0;
    for (i, ch) in text.char_indices() {
        let size = ch.len_utf8() as i32;
        if size == 1 {
            continue;
        }
        let utf16_size = ch.len_utf16() as i32;
        delta += size - utf16_size;
        pm.entries.push(PositionMapEntry {
            utf8_pos: i as i32 + size,
            delta,
        });
    }
    pm.ascii_only = pm.entries.is_empty();
    pm
}

impl PositionMap {
    // IsAsciiOnly returns true if the text is ASCII-only,
    // meaning UTF-8 and UTF-16 offsets are identical.
    pub fn is_ascii_only(&self) -> bool {
        self.ascii_only
    }

    // UTF8ToUTF16 converts a UTF-8 byte offset to a UTF-16 code unit offset.
    pub fn utf8_to_utf16(&self, utf8_offset: i32) -> i32 {
        if self.ascii_only {
            return utf8_offset;
        }
        // Binary search: find the last entry where utf8Pos <= utf8Offset
        let mut lo = 0;
        let mut hi = self.entries.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.entries[mid].utf8_pos <= utf8_offset {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo == 0 {
            // Before any multi-byte character
            return utf8_offset;
        }
        utf8_offset - self.entries[lo - 1].delta
    }

    // UTF16ToUTF8 converts a UTF-16 code unit offset to a UTF-8 byte offset.
    pub fn utf16_to_utf8(&self, utf16_offset: i32) -> i32 {
        if self.ascii_only {
            return utf16_offset;
        }
        // We need the last entry where (utf8Pos - delta) <= utf16Offset.
        // (utf8Pos - delta) is the UTF-16 offset of that entry's character.
        let mut lo = 0;
        let mut hi = self.entries.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let utf16_pos = self.entries[mid].utf8_pos - self.entries[mid].delta;
            if utf16_pos <= utf16_offset {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo == 0 {
            return utf16_offset;
        }
        utf16_offset + self.entries[lo - 1].delta
    }
}
