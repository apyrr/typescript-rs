use std::collections::{HashMap, HashSet};

use crate::autoimport::word_indices;

// Named is a constraint for types that can provide their name.
pub trait Named {
    fn name(&self) -> &str;
}

// Index stores entries with an index mapping uppercase letters to entries whose name
// starts with that letter, and lowercase letters to entries whose name contains a
// word starting with that letter.
#[derive(Clone, Debug)]
pub struct Index<T: Named + Clone> {
    pub entries: Vec<T>,
    pub index: HashMap<char, Vec<usize>>,
}

impl<T: Named + Clone> Default for Index<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
        }
    }
}

impl<T: Named + Clone> Index<T> {
    pub fn find(&self, name: &str, case_sensitive: bool) -> Vec<T> {
        if self.entries.is_empty() || name.is_empty() {
            return Vec::new();
        }
        let Some(first_rune) = name.chars().next() else {
            return Vec::new();
        };
        let first_rune_upper = first_rune.to_uppercase().next().unwrap_or(first_rune);
        let Some(candidates) = self.index.get(&first_rune_upper) else {
            return Vec::new();
        };

        let mut results = Vec::new();
        for entry_index in candidates {
            let entry = &self.entries[*entry_index];
            let entry_name = entry.name();
            if (case_sensitive && entry_name == name)
                || (!case_sensitive && entry_name.to_lowercase() == name.to_lowercase())
            {
                results.push(entry.clone());
            }
        }

        results
    }

    // SearchWordPrefix returns each entry whose name contains a word beginning with
    // the first character of 'prefix', and whose name contains all characters
    // of 'prefix' in order (case-insensitive). If 'filter' is provided, only entries
    // for which filter(entry) returns true are included.
    pub fn search_word_prefix(&self, prefix: &str) -> Vec<T> {
        if self.entries.is_empty() {
            return Vec::new();
        }

        if prefix.is_empty() {
            return self.entries.clone();
        }

        let prefix = prefix.to_lowercase();
        let Some(first_rune) = prefix.chars().next() else {
            return Vec::new();
        };

        let first_rune_upper = first_rune.to_uppercase().next().unwrap_or(first_rune);
        let first_rune_lower = first_rune.to_lowercase().next().unwrap_or(first_rune);

        // Look up entries that have words starting with this letter
        let name_starts = self
            .index
            .get(&first_rune_upper)
            .cloned()
            .unwrap_or_default();
        let word_starts = if first_rune_upper != first_rune_lower {
            self.index
                .get(&first_rune_lower)
                .cloned()
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let count = name_starts.len() + word_starts.len();
        if count == 0 {
            return Vec::new();
        }

        // Filter entries by checking if they contain all characters in order
        let mut results = Vec::with_capacity(count);
        for starts in [name_starts, word_starts] {
            for i in starts {
                let entry = &self.entries[i];
                if contains_chars_in_order(entry.name(), &prefix) {
                    results.push(entry.clone());
                }
            }
        }
        results
    }

    // insertAsWords adds a value to the index keyed by the first letter of each word in its name.
    pub fn insert_as_words(&mut self, value: T) {
        let name = value.name().to_string();
        if name.is_empty() {
            panic!("Cannot index entry with empty name");
        }
        let entry_index = self.entries.len();
        self.entries.push(value);

        let indices = word_indices(&name);
        let mut seen_runes = HashSet::new();

        for (i, start) in indices.iter().enumerate() {
            let substr = &name[*start..];
            let Some(mut first_rune) = substr.chars().next() else {
                continue;
            };
            if i == 0 {
                // Name start keyed by uppercase
                first_rune = first_rune.to_uppercase().next().unwrap_or(first_rune);
                self.index.entry(first_rune).or_default().push(entry_index);
                seen_runes.insert(first_rune); // (Still set seenRunes in case first character is non-alphabetic)
            } else {
                // Subsequent word starts keyed by lowercase
                first_rune = first_rune.to_lowercase().next().unwrap_or(first_rune);
                if !seen_runes.contains(&first_rune) {
                    self.index.entry(first_rune).or_default().push(entry_index);
                    seen_runes.insert(first_rune);
                }
            }
        }
    }

    // Clone creates a new Index containing only entries for which filter returns true.
    pub fn clone_filtered(&self, filter: impl Fn(&T) -> bool) -> Option<Index<T>> {
        let mut new_idx = Index {
            entries: Vec::with_capacity(self.entries.len()),
            index: HashMap::with_capacity(self.index.len()),
        };

        // Build mapping from old index to new index for filtered entries
        let mut old_to_new = HashMap::with_capacity(self.entries.len());
        for (old_index, entry) in self.entries.iter().enumerate() {
            if filter(entry) {
                let new_index = new_idx.entries.len();
                new_idx.entries.push(entry.clone());
                old_to_new.insert(old_index, new_index);
            }
        }

        // Rebuild the index with remapped indices
        for (r, old_indices) in &self.index {
            let mut new_indices = Vec::with_capacity(old_indices.len());
            for old_index in old_indices {
                if let Some(new_index) = old_to_new.get(old_index) {
                    new_indices.push(*new_index);
                }
            }
            if !new_indices.is_empty() {
                new_idx.index.insert(*r, new_indices);
            }
        }

        Some(new_idx)
    }
}

// containsCharsInOrder checks if str contains all characters from pattern in order (case-insensitive).
pub fn contains_chars_in_order(str_: &str, pattern: &str) -> bool {
    let str_ = str_.to_lowercase();
    let pattern = pattern.to_lowercase();

    let pattern_chars: Vec<char> = pattern.chars().collect();
    let mut pattern_idx = 0usize;
    for ch in str_.chars() {
        if pattern_idx < pattern_chars.len() && ch == pattern_chars[pattern_idx] {
            pattern_idx += 1;
        }
    }
    pattern_idx == pattern_chars.len()
}
