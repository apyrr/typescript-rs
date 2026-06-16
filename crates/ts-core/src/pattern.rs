#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pattern {
    pub text: String,
    pub star_index: isize, // -1 for exact match
}

pub fn try_parse_pattern(pattern: &str) -> Pattern {
    let star_index = pattern.find('*').map(|i| i as isize).unwrap_or(-1);
    if star_index == -1 || !pattern[star_index as usize + 1..].contains('*') {
        return Pattern {
            text: pattern.to_string(),
            star_index,
        };
    }
    Pattern::default()
}

impl Pattern {
    pub fn is_valid(&self) -> bool {
        self.star_index == -1 || self.star_index < self.text.len() as isize
    }

    pub fn matches(&self, candidate: &str) -> bool {
        if self.star_index == -1 {
            return self.text == candidate;
        }
        let star_index = self.star_index as usize;
        candidate.len() >= star_index
            && candidate.starts_with(&self.text[..star_index])
            && candidate.ends_with(&self.text[star_index + 1..])
    }

    pub fn matched_text<'a>(&self, candidate: &'a str) -> &'a str {
        if !self.matches(candidate) {
            panic!("candidate does not match pattern");
        }
        if self.star_index == -1 {
            return "";
        }
        let star_index = self.star_index as usize;
        &candidate[star_index..candidate.len() - self.text.len() + star_index + 1]
    }
}

pub fn find_best_pattern_match<T: Default + Clone>(
    values: &[T],
    get_pattern: impl Fn(&T) -> Pattern,
    candidate: &str,
) -> T {
    let mut best_pattern = T::default();
    let mut longest_match_prefix_length = -1;
    for value in values {
        let pattern = get_pattern(value);
        if (pattern.star_index == -1 || pattern.star_index > longest_match_prefix_length)
            && pattern.matches(candidate)
        {
            best_pattern = value.clone();
            longest_match_prefix_length = pattern.star_index;
        }
    }
    best_pattern
}
