use ts_collections::{FastHashMap as HashMap, FastHashMapExt};
use ts_core as core;
use ts_diagnostics as diagnostics;

use crate::Scanner;
use crate::scanner::{
    EscapeSequenceScanningFlags, is_identifier_part, is_identifier_start, is_word_character,
};
use crate::unicodeproperties::{
    binary_unicode_properties, binary_unicode_properties_of_strings, non_binary_unicode_properties,
    values_of_non_binary_unicode_properties,
};
use crate::utilities::{
    SURR_SELF, SURR1, SURR2, code_point_is_high_surrogate, code_point_is_low_surrogate,
    decode_class_atom_rune, encode_surrogate,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(i32)]
pub enum RegularExpressionFlags {
    None = 0,
    HasIndices = 1 << 0,
    Global = 1 << 1,
    IgnoreCase = 1 << 2,
    Multiline = 1 << 3,
    DotAll = 1 << 4,
    Unicode = 1 << 5,
    UnicodeSets = 1 << 6,
    Sticky = 1 << 7,
}

pub const REGULAR_EXPRESSION_FLAGS_ANY_UNICODE_MODE: i32 =
    RegularExpressionFlags::Unicode as i32 | RegularExpressionFlags::UnicodeSets as i32;
pub const REGULAR_EXPRESSION_FLAGS_MODIFIERS: i32 = RegularExpressionFlags::IgnoreCase as i32
    | RegularExpressionFlags::Multiline as i32
    | RegularExpressionFlags::DotAll as i32;

pub fn char_code_to_reg_exp_flag(ch: char) -> Option<RegularExpressionFlags> {
    match ch {
        'd' => Some(RegularExpressionFlags::HasIndices),
        'g' => Some(RegularExpressionFlags::Global),
        'i' => Some(RegularExpressionFlags::IgnoreCase),
        'm' => Some(RegularExpressionFlags::Multiline),
        's' => Some(RegularExpressionFlags::DotAll),
        'u' => Some(RegularExpressionFlags::Unicode),
        'v' => Some(RegularExpressionFlags::UnicodeSets),
        'y' => Some(RegularExpressionFlags::Sticky),
        _ => None,
    }
}

pub fn reg_exp_flag_first_available_language_version(
    flag: RegularExpressionFlags,
) -> Option<core::ScriptTarget> {
    match flag {
        RegularExpressionFlags::HasIndices => Some(core::ScriptTarget::ES2022),
        RegularExpressionFlags::DotAll => Some(core::ScriptTarget::ES2018),
        RegularExpressionFlags::UnicodeSets => Some(core::ScriptTarget::ES2024),
        _ => None,
    }
}

impl Scanner {
    pub fn check_regular_expression_flag_availability(
        &mut self,
        flag: RegularExpressionFlags,
        pos: usize,
        size: usize,
    ) {
        if let Some(available_from) = reg_exp_flag_first_available_language_version(flag)
            && self.language_version() < available_from
        {
            self.error_at(
                    &diagnostics::This_regular_expression_flag_is_only_available_when_targeting_0_or_later,
                    pos,
                    size,
                    vec![available_from.to_string().to_lowercase()],
                );
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum ClassSetExpressionType {
    Unknown = 0,
    ClassUnion = 1,
    ClassIntersection = 2,
    ClassSubtraction = 3,
}

pub struct GroupNameReference {
    pub pos: usize,
    pub end: usize,
    pub name: String,
}

pub struct DecimalEscapeValue {
    pub pos: usize,
    pub end: usize,
    pub value: usize,
}

pub struct RegExpParser<'a> {
    pub scanner: &'a mut Scanner,
    pub end: usize,
    pub reg_exp_flags: i32,
    pub any_unicode_mode: bool,
    pub unicode_sets_mode: bool,
    pub annex_b: bool,
    pub any_unicode_mode_or_non_annex_b: bool,
    pub named_capture_groups: bool,
    pub may_contain_strings: bool,
    pub number_of_capturing_groups: usize,
    pub group_specifiers: HashMap<String, bool>,
    pub group_name_references: Vec<GroupNameReference>,
    pub decimal_escapes: Vec<DecimalEscapeValue>,
    pub named_capturing_groups: Vec<HashMap<String, bool>>,
    pub pending_low_surrogate: Option<u32>,
}

impl<'a> RegExpParser<'a> {
    pub fn pos(&self) -> usize {
        self.scanner.pos
    }

    pub fn set_pos(&mut self, value: usize) {
        self.scanner.pos = value;
    }

    pub fn inc_pos(&mut self, n: isize) {
        self.scanner.pos = self.scanner.pos.saturating_add_signed(n);
    }

    pub fn char(&self) -> char {
        self.scanner.char()
    }

    pub fn char_at(&self, pos: usize) -> char {
        self.scanner.char_at(pos - self.pos())
    }

    pub fn error(&mut self, msg: &diagnostics::Message, pos: usize, length: usize) {
        self.error_with_args(msg, pos, length, Vec::new());
    }

    pub fn error_with_args(
        &mut self,
        msg: &diagnostics::Message,
        pos: usize,
        length: usize,
        args: Vec<String>,
    ) {
        self.scanner.error_at(msg, pos, length, args);
    }

    pub fn text(&self) -> &str {
        &self.scanner.text
    }

    // Disjunction ::= Alternative ('|' Alternative)*
    pub fn scan_disjunction(&mut self, is_in_group: bool) {
        loop {
            self.named_capturing_groups.push(HashMap::new());
            self.scan_alternative(is_in_group);
            self.named_capturing_groups.pop();
            if self.char() != '|' {
                return;
            }
            self.inc_pos(1);
        }
    }

    pub fn scan_alternative(&mut self, is_in_group: bool) {
        let mut is_previous_term_quantifiable = false;
        while self.pos() < self.end {
            let start = self.pos();
            let ch = self.char();
            match ch {
                '^' | '$' => {
                    self.inc_pos(1);
                    is_previous_term_quantifiable = false;
                }
                '\\' => {
                    self.inc_pos(1);
                    match self.char() {
                        'b' | 'B' => {
                            self.inc_pos(1);
                            is_previous_term_quantifiable = false;
                        }
                        _ => {
                            self.scan_atom_escape();
                            is_previous_term_quantifiable = true;
                        }
                    }
                }
                '(' => {
                    self.inc_pos(1);
                    if self.char() == '?' {
                        self.inc_pos(1);
                        match self.char() {
                            '=' | '!' => {
                                self.inc_pos(1);
                                is_previous_term_quantifiable =
                                    !self.any_unicode_mode_or_non_annex_b;
                            }
                            '<' => {
                                let group_name_start = self.pos();
                                self.inc_pos(1);
                                match self.char() {
                                    '=' | '!' => {
                                        self.inc_pos(1);
                                        is_previous_term_quantifiable = false;
                                    }
                                    _ => {
                                        self.scan_group_name(false);
                                        self.scan_expected_char('>');
                                        if self.scanner.language_version()
                                            < core::ScriptTarget::ES2018
                                        {
                                            self.error(
                                                &diagnostics::Named_capturing_groups_are_only_available_when_targeting_ES2018_or_later,
                                                group_name_start,
                                                self.pos() - group_name_start,
                                            );
                                        }
                                        self.number_of_capturing_groups += 1;
                                        is_previous_term_quantifiable = true;
                                    }
                                }
                            }
                            _ => {
                                let flags_start = self.pos();
                                let set_flags = self
                                    .scan_pattern_modifiers(RegularExpressionFlags::None as i32);
                                if self.char() == '-' {
                                    self.inc_pos(1);
                                    self.scan_pattern_modifiers(set_flags);
                                    if self.pos() == flags_start + 1 {
                                        self.error(
                                            &diagnostics::Subpattern_flags_must_be_present_when_there_is_a_minus_sign,
                                            flags_start,
                                            self.pos() - flags_start,
                                        );
                                    }
                                }
                                self.scan_expected_char(':');
                                is_previous_term_quantifiable = true;
                            }
                        }
                    } else {
                        self.number_of_capturing_groups += 1;
                        is_previous_term_quantifiable = true;
                    }
                    self.scan_disjunction(true);
                    self.scan_expected_char(')');
                }
                '{' => {
                    self.inc_pos(1);
                    let digits_start = self.pos();
                    self.scan_digits();
                    let min_str = self.scanner.token_value().to_string();
                    if !self.any_unicode_mode_or_non_annex_b && min_str.is_empty() {
                        is_previous_term_quantifiable = true;
                        continue;
                    }
                    if self.char() == ',' {
                        self.inc_pos(1);
                        self.scan_digits();
                        let max_str = self.scanner.token_value().to_string();
                        if min_str.is_empty() {
                            if !max_str.is_empty() || self.char() == '}' {
                                self.error(
                                    &diagnostics::Incomplete_quantifier_Digit_expected,
                                    digits_start,
                                    0,
                                );
                            } else {
                                self.error_with_args(
                                    &diagnostics::Unexpected_0_Did_you_mean_to_escape_it_with_backslash,
                                    start,
                                    1,
                                    vec![ch.to_string()],
                                );
                                is_previous_term_quantifiable = true;
                                continue;
                            }
                        } else if !max_str.is_empty()
                            && compare_decimal_strings(&min_str, &max_str) > 0
                            && (self.any_unicode_mode_or_non_annex_b || self.char() == '}')
                        {
                            self.error(
                                &diagnostics::Numbers_out_of_order_in_quantifier,
                                digits_start,
                                self.pos() - digits_start,
                            );
                        }
                    } else if min_str.is_empty() {
                        if self.any_unicode_mode_or_non_annex_b {
                            self.error_with_args(
                                &diagnostics::Unexpected_0_Did_you_mean_to_escape_it_with_backslash,
                                start,
                                1,
                                vec![ch.to_string()],
                            );
                        }
                        is_previous_term_quantifiable = true;
                        continue;
                    }
                    if self.char() != '}' {
                        if self.any_unicode_mode_or_non_annex_b {
                            self.error_with_args(
                                &diagnostics::X_0_expected,
                                self.pos(),
                                0,
                                vec!["}".to_string()],
                            );
                            self.inc_pos(-1);
                        } else {
                            is_previous_term_quantifiable = true;
                            continue;
                        }
                    }
                    self.scan_quantifier_suffix(start, &mut is_previous_term_quantifiable);
                }
                '*' | '+' | '?' => {
                    self.scan_quantifier_suffix(start, &mut is_previous_term_quantifiable)
                }
                '.' => {
                    self.inc_pos(1);
                    is_previous_term_quantifiable = true;
                }
                '[' => {
                    self.inc_pos(1);
                    if self.unicode_sets_mode {
                        self.scan_class_set_expression();
                    } else {
                        self.scan_class_ranges();
                        self.pending_low_surrogate = None;
                    }
                    self.scan_expected_char(']');
                    is_previous_term_quantifiable = true;
                }
                ')' => {
                    if is_in_group {
                        return;
                    }
                    self.error_with_args(
                        &diagnostics::Unexpected_0_Did_you_mean_to_escape_it_with_backslash,
                        self.pos(),
                        1,
                        vec![ch.to_string()],
                    );
                    self.inc_pos(1);
                    is_previous_term_quantifiable = true;
                }
                ']' | '}' => {
                    if self.any_unicode_mode_or_non_annex_b {
                        self.error_with_args(
                            &diagnostics::Unexpected_0_Did_you_mean_to_escape_it_with_backslash,
                            self.pos(),
                            1,
                            vec![ch.to_string()],
                        );
                    }
                    self.inc_pos(1);
                    is_previous_term_quantifiable = true;
                }
                '/' | '|' => return,
                _ => {
                    self.scan_source_character();
                    is_previous_term_quantifiable = true;
                }
            }
        }
    }

    fn scan_quantifier_suffix(&mut self, start: usize, is_previous_term_quantifiable: &mut bool) {
        self.inc_pos(1);
        if self.char() == '?' {
            self.inc_pos(1);
        }
        if !*is_previous_term_quantifiable {
            self.error(
                &diagnostics::There_is_nothing_available_for_repetition,
                start,
                self.pos() - start,
            );
        }
        *is_previous_term_quantifiable = false;
    }

    pub fn scan_pattern_modifiers(&mut self, mut curr_flags: i32) -> i32 {
        while self.pos() < self.end {
            let Some(ch) = self.text()[self.pos()..].chars().next() else {
                break;
            };
            let size = ch.len_utf8();
            if !is_identifier_part(ch) {
                break;
            }
            if let Some(flag) = char_code_to_reg_exp_flag(ch) {
                let flag_value = flag as i32;
                if curr_flags & flag_value != 0 {
                    self.error(
                        &diagnostics::Duplicate_regular_expression_flag,
                        self.pos(),
                        size,
                    );
                } else if flag_value & REGULAR_EXPRESSION_FLAGS_MODIFIERS == 0 {
                    self.error(
                        &diagnostics::This_regular_expression_flag_cannot_be_toggled_within_a_subpattern,
                        self.pos(),
                        size,
                    );
                } else {
                    curr_flags |= flag_value;
                    self.scanner
                        .check_regular_expression_flag_availability(flag, self.pos(), size);
                }
            } else {
                self.error(
                    &diagnostics::Unknown_regular_expression_flag,
                    self.pos(),
                    size,
                );
            }
            self.inc_pos(size as isize);
        }
        curr_flags
    }

    pub fn scan_atom_escape(&mut self) {
        match self.char() {
            'k' => {
                self.inc_pos(1);
                if self.char() == '<' {
                    self.inc_pos(1);
                    self.scan_group_name(true);
                    self.scan_expected_char('>');
                } else if self.any_unicode_mode_or_non_annex_b || self.named_capture_groups {
                    self.error(
                        &diagnostics::X_k_must_be_followed_by_a_capturing_group_name_enclosed_in_angle_brackets,
                        self.pos().saturating_sub(2),
                        2,
                    );
                }
            }
            'q' if self.unicode_sets_mode => {
                self.inc_pos(1);
                self.error(
                    &diagnostics::X_q_is_only_available_inside_character_class,
                    self.pos().saturating_sub(2),
                    2,
                );
            }
            _ => {
                if !self.scan_character_class_escape() && !self.scan_decimal_escape() {
                    self.scan_character_escape(true);
                }
            }
        }
    }

    pub fn scan_decimal_escape(&mut self) -> bool {
        let ch = self.char();
        if ('1'..='9').contains(&ch) {
            let start = self.pos();
            self.scan_digits();
            let value = self
                .scanner
                .token_value()
                .parse::<usize>()
                .unwrap_or(usize::MAX);
            self.decimal_escapes.push(DecimalEscapeValue {
                pos: start,
                end: self.pos(),
                value,
            });
            return true;
        }
        false
    }

    pub fn scan_character_escape(&mut self, atom_escape: bool) -> String {
        let ch = self.char();
        match ch {
            '\0' => {
                self.error(
                    &diagnostics::Undetermined_character_escape,
                    self.pos().saturating_sub(1),
                    1,
                );
                "\\".to_string()
            }
            'c' => {
                self.inc_pos(1);
                let ch = self.char();
                if ch.is_ascii_alphabetic() {
                    self.inc_pos(1);
                    return char::from_u32((ch as u32) & 0x1f)
                        .unwrap_or('\0')
                        .to_string();
                }
                if self.any_unicode_mode_or_non_annex_b {
                    self.error(
                        &diagnostics::X_c_must_be_followed_by_an_ASCII_letter,
                        self.pos().saturating_sub(2),
                        2,
                    );
                } else if atom_escape {
                    self.inc_pos(-1);
                    return "\\".to_string();
                }
                ch.to_string()
            }
            '^' | '$' | '/' | '\\' | '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}'
            | '|' => {
                self.inc_pos(1);
                ch.to_string()
            }
            _ => {
                self.inc_pos(-1);
                let mut flags = EscapeSequenceScanningFlags::RegularExpression as i32;
                if self.annex_b {
                    flags |= EscapeSequenceScanningFlags::AnnexB as i32;
                }
                if self.any_unicode_mode {
                    flags |= EscapeSequenceScanningFlags::AnyUnicodeMode as i32;
                }
                if atom_escape {
                    flags |= EscapeSequenceScanningFlags::AtomEscape as i32;
                }
                self.scanner.scan_escape_sequence(flags)
            }
        }
    }

    pub fn scan_group_name(&mut self, is_reference: bool) {
        self.scanner.token_start = self.pos();
        let start = self.pos();
        let Some(first) = self.text()[start..].chars().next() else {
            self.error(&diagnostics::Expected_a_capturing_group_name, start, 0);
            return;
        };
        if !is_identifier_start(first) {
            self.error(&diagnostics::Expected_a_capturing_group_name, start, 0);
            return;
        }

        self.inc_pos(first.len_utf8() as isize);
        while self.pos() < self.end {
            let ch = self.char();
            if !is_identifier_part(ch) {
                break;
            }
            self.inc_pos(ch.len_utf8() as isize);
        }

        let name = self.text()[start..self.pos()].to_string();
        self.scanner.set_token_value(name.clone());
        if is_reference {
            self.group_name_references.push(GroupNameReference {
                pos: start,
                end: self.pos(),
                name,
            });
        } else if self.named_capturing_groups_contains(&name) {
            self.error(
                &diagnostics::Named_capturing_groups_with_the_same_name_must_be_mutually_exclusive_to_each_other,
                start,
                self.pos() - start,
            );
        } else {
            if let Some(scope) = self.named_capturing_groups.last_mut() {
                scope.insert(name.clone(), true);
            }
            self.group_specifiers.insert(name, true);
        }
    }

    pub fn named_capturing_groups_contains(&self, name: &str) -> bool {
        self.named_capturing_groups
            .iter()
            .any(|group| group.get(name).copied().unwrap_or(false))
    }

    pub fn is_class_content_exit(&self, ch: char) -> bool {
        ch == ']' || self.pos() >= self.end
    }

    pub fn scan_class_ranges(&mut self) {
        self.pending_low_surrogate = None;
        if self.char() == '^' {
            self.inc_pos(1);
        }
        while self.pos() < self.end {
            let mut ch = self.char();
            if self.is_class_content_exit(ch) {
                return;
            }
            let min_start = self.pos();
            let min_character = self.scan_class_atom();
            if self.char() == '-' {
                self.inc_pos(1);
                ch = self.char();
                if self.is_class_content_exit(ch) {
                    return;
                }
                if min_character.is_empty() && self.any_unicode_mode_or_non_annex_b {
                    self.error(
                        &diagnostics::A_character_class_range_must_not_be_bounded_by_another_character_class,
                        min_start,
                        self.pos().saturating_sub(1 + min_start),
                    );
                }
                let max_start = self.pos();
                let max_character = self.scan_class_atom();
                if max_character.is_empty() && self.any_unicode_mode_or_non_annex_b {
                    self.error(
                        &diagnostics::A_character_class_range_must_not_be_bounded_by_another_character_class,
                        max_start,
                        self.pos() - max_start,
                    );
                    continue;
                }
                if min_character.is_empty() {
                    continue;
                }
                let (min_character_value, min_size) = decode_class_atom_rune(&min_character);
                let (max_character_value, max_size) = decode_class_atom_rune(&max_character);
                if min_character.len() == min_size
                    && max_character.len() == max_size
                    && min_character_value > max_character_value
                {
                    self.error(
                        &diagnostics::Range_out_of_order_in_character_class,
                        min_start,
                        self.pos() - min_start,
                    );
                }
            }
        }
    }

    pub fn scan_class_set_expression(&mut self) {
        let mut is_character_complement = false;
        if self.char() == '^' {
            self.inc_pos(1);
            is_character_complement = true;
        }
        let mut expression_may_contain_strings = false;
        let mut ch = self.char();
        if self.is_class_content_exit(ch) {
            return;
        }
        let mut start = self.pos();
        let mut operand = Vec::new();
        let mut two_chars = self.two_chars_at_pos();
        match two_chars {
            Some([b'-', b'-'] | [b'&', b'&']) => {
                self.error(&diagnostics::Expected_a_class_set_operand, self.pos(), 0);
                self.may_contain_strings = false;
            }
            _ => operand = self.scan_class_set_operand(),
        }
        match self.char() {
            '-' if self.pos() + 1 < self.end && self.char_at(self.pos() + 1) == '-' => {
                if is_character_complement && self.may_contain_strings {
                    self.error(&diagnostics::Anything_that_would_possibly_match_more_than_a_single_character_is_invalid_inside_a_negated_character_class, start, self.pos() - start);
                }
                expression_may_contain_strings = self.may_contain_strings;
                self.scan_class_set_sub_expression(ClassSetExpressionType::ClassSubtraction);
                self.may_contain_strings =
                    !is_character_complement && expression_may_contain_strings;
                return;
            }
            '&' if self.pos() + 1 < self.end && self.char_at(self.pos() + 1) == '&' => {
                self.scan_class_set_sub_expression(ClassSetExpressionType::ClassIntersection);
                if is_character_complement && self.may_contain_strings {
                    self.error(&diagnostics::Anything_that_would_possibly_match_more_than_a_single_character_is_invalid_inside_a_negated_character_class, start, self.pos() - start);
                }
                expression_may_contain_strings = self.may_contain_strings;
                self.may_contain_strings =
                    !is_character_complement && expression_may_contain_strings;
                return;
            }
            '&' => self.error_with_args(
                &diagnostics::Unexpected_0_Did_you_mean_to_escape_it_with_backslash,
                self.pos(),
                1,
                vec![ch.to_string()],
            ),
            _ => {
                if is_character_complement && self.may_contain_strings {
                    self.error(&diagnostics::Anything_that_would_possibly_match_more_than_a_single_character_is_invalid_inside_a_negated_character_class, start, self.pos() - start);
                }
                expression_may_contain_strings = self.may_contain_strings;
            }
        }
        while self.pos() < self.end {
            ch = self.char();
            match ch {
                '-' => {
                    self.inc_pos(1);
                    ch = self.char();
                    if self.is_class_content_exit(ch) {
                        self.may_contain_strings =
                            !is_character_complement && expression_may_contain_strings;
                        return;
                    }
                    if ch == '-' {
                        self.inc_pos(1);
                        self.error(&diagnostics::Operators_must_not_be_mixed_within_a_character_class_Wrap_it_in_a_nested_class_instead, self.pos().saturating_sub(2), 2);
                        start = self.pos().saturating_sub(2);
                        operand = self.text().as_bytes()[start..self.pos()].to_vec();
                        continue;
                    }
                    if operand.is_empty() {
                        self.error(&diagnostics::A_character_class_range_must_not_be_bounded_by_another_character_class, start, self.pos().saturating_sub(1 + start));
                    }
                    let second_start = self.pos();
                    let second_operand = self.scan_class_set_operand();
                    if is_character_complement && self.may_contain_strings {
                        self.error(&diagnostics::Anything_that_would_possibly_match_more_than_a_single_character_is_invalid_inside_a_negated_character_class, second_start, self.pos() - second_start);
                    }
                    expression_may_contain_strings =
                        expression_may_contain_strings || self.may_contain_strings;
                    if second_operand.is_empty() {
                        self.error(&diagnostics::A_character_class_range_must_not_be_bounded_by_another_character_class, second_start, self.pos() - second_start);
                    } else if !operand.is_empty() {
                        let (min_character_value, min_size) = decode_class_atom_rune(&operand);
                        let (max_character_value, max_size) =
                            decode_class_atom_rune(&second_operand);
                        if operand.len() == min_size
                            && second_operand.len() == max_size
                            && min_character_value > max_character_value
                        {
                            self.error(
                                &diagnostics::Range_out_of_order_in_character_class,
                                start,
                                self.pos() - start,
                            );
                        }
                    }
                }
                '&' => {
                    start = self.pos();
                    self.inc_pos(1);
                    if self.char() == '&' {
                        self.inc_pos(1);
                        self.error(&diagnostics::Operators_must_not_be_mixed_within_a_character_class_Wrap_it_in_a_nested_class_instead, self.pos().saturating_sub(2), 2);
                        if self.char() == '&' {
                            self.error_with_args(
                                &diagnostics::Unexpected_0_Did_you_mean_to_escape_it_with_backslash,
                                self.pos(),
                                1,
                                vec![ch.to_string()],
                            );
                            self.inc_pos(1);
                        }
                    } else {
                        self.error_with_args(
                            &diagnostics::Unexpected_0_Did_you_mean_to_escape_it_with_backslash,
                            self.pos().saturating_sub(1),
                            1,
                            vec![ch.to_string()],
                        );
                    }
                    operand = self.text().as_bytes()[start..self.pos()].to_vec();
                    continue;
                }
                _ => {}
            }
            if self.is_class_content_exit(self.char()) {
                break;
            }
            start = self.pos();
            two_chars = self.two_chars_at_pos();
            match two_chars {
                Some([b'-', b'-'] | [b'&', b'&']) => {
                    self.error(&diagnostics::Operators_must_not_be_mixed_within_a_character_class_Wrap_it_in_a_nested_class_instead, self.pos(), 2);
                    self.inc_pos(2);
                    operand = self.text().as_bytes()[start..self.pos()].to_vec();
                }
                _ => operand = self.scan_class_set_operand(),
            }
        }
        self.may_contain_strings = !is_character_complement && expression_may_contain_strings;
    }

    pub fn scan_class_set_sub_expression(&mut self, expression_type: ClassSetExpressionType) {
        let mut expression_may_contain_strings = self.may_contain_strings;
        while self.pos() < self.end {
            let mut ch = self.char();
            if self.is_class_content_exit(ch) {
                break;
            }
            match ch {
                '-' => {
                    self.inc_pos(1);
                    if self.char() == '-' {
                        self.inc_pos(1);
                        if expression_type != ClassSetExpressionType::ClassSubtraction {
                            self.error(&diagnostics::Operators_must_not_be_mixed_within_a_character_class_Wrap_it_in_a_nested_class_instead, self.pos().saturating_sub(2), 2);
                        }
                    } else {
                        self.error(&diagnostics::Operators_must_not_be_mixed_within_a_character_class_Wrap_it_in_a_nested_class_instead, self.pos().saturating_sub(1), 1);
                    }
                }
                '&' => {
                    self.inc_pos(1);
                    if self.char() == '&' {
                        self.inc_pos(1);
                        if expression_type != ClassSetExpressionType::ClassIntersection {
                            self.error(&diagnostics::Operators_must_not_be_mixed_within_a_character_class_Wrap_it_in_a_nested_class_instead, self.pos().saturating_sub(2), 2);
                        }
                        if self.char() == '&' {
                            self.error_with_args(
                                &diagnostics::Unexpected_0_Did_you_mean_to_escape_it_with_backslash,
                                self.pos(),
                                1,
                                vec![ch.to_string()],
                            );
                            self.inc_pos(1);
                        }
                    } else {
                        self.error_with_args(
                            &diagnostics::Unexpected_0_Did_you_mean_to_escape_it_with_backslash,
                            self.pos().saturating_sub(1),
                            1,
                            vec![ch.to_string()],
                        );
                    }
                }
                _ => match expression_type {
                    ClassSetExpressionType::ClassSubtraction => self.error_with_args(
                        &diagnostics::X_0_expected,
                        self.pos(),
                        0,
                        vec!["--".to_string()],
                    ),
                    ClassSetExpressionType::ClassIntersection => self.error_with_args(
                        &diagnostics::X_0_expected,
                        self.pos(),
                        0,
                        vec!["&&".to_string()],
                    ),
                    _ => {}
                },
            }
            ch = self.char();
            if self.is_class_content_exit(ch) {
                self.error(&diagnostics::Expected_a_class_set_operand, self.pos(), 0);
                break;
            }
            self.scan_class_set_operand();
            if expression_type == ClassSetExpressionType::ClassIntersection {
                expression_may_contain_strings =
                    expression_may_contain_strings && self.may_contain_strings;
            }
        }
        self.may_contain_strings = expression_may_contain_strings;
    }

    pub fn scan_class_set_operand(&mut self) -> Vec<u8> {
        self.may_contain_strings = false;
        match self.char() {
            '[' => {
                self.inc_pos(1);
                self.scan_class_set_expression();
                self.scan_expected_char(']');
                Vec::new()
            }
            '\\' => {
                self.inc_pos(1);
                if self.scan_character_class_escape() {
                    return Vec::new();
                }
                if self.char() == 'q' {
                    self.inc_pos(1);
                    if self.char() == '{' {
                        self.inc_pos(1);
                        self.scan_class_string_disjunction_contents();
                        self.scan_expected_char('}');
                        return Vec::new();
                    }
                    self.error(&diagnostics::X_q_must_be_followed_by_string_alternatives_enclosed_in_braces, self.pos().saturating_sub(2), 2);
                    return b"q".to_vec();
                }
                self.inc_pos(-1);
                self.scan_class_set_character()
            }
            _ => self.scan_class_set_character(),
        }
    }

    pub fn scan_class_string_disjunction_contents(&mut self) {
        let mut character_count = 0;
        while self.pos() < self.end {
            match self.char() {
                '}' => {
                    if character_count != 1 {
                        self.may_contain_strings = true;
                    }
                    return;
                }
                '|' => {
                    if character_count != 1 {
                        self.may_contain_strings = true;
                    }
                    self.inc_pos(1);
                    character_count = 0;
                }
                _ => {
                    self.scan_class_set_character();
                    character_count += 1;
                }
            }
        }
    }

    pub fn scan_class_set_character(&mut self) -> Vec<u8> {
        let ch = self.char();
        if ch == '\\' {
            self.inc_pos(1);
            let inner_ch = self.char();
            match inner_ch {
                'b' => {
                    self.inc_pos(1);
                    b"\x08".to_vec()
                }
                '&' | '-' | '!' | '#' | '%' | ',' | ':' | ';' | '<' | '=' | '>' | '@' | '`'
                | '~' => {
                    self.inc_pos(1);
                    char_bytes(inner_ch)
                }
                _ => self.scan_character_escape_bytes(false),
            }
        } else if self.pos() + 1 < self.end && ch == self.char_at(self.pos() + 1) {
            match ch {
                '&' | '!' | '#' | '%' | '*' | '+' | ',' | '.' | ':' | ';' | '<' | '=' | '>'
                | '?' | '@' | '`' | '~' => {
                    self.error(&diagnostics::A_character_class_must_not_contain_a_reserved_double_punctuator_Did_you_mean_to_escape_it_with_backslash, self.pos(), 2);
                    self.inc_pos(2);
                    self.text().as_bytes()[self.pos() - 2..self.pos()].to_vec()
                }
                _ => self.scan_class_set_source_character(ch),
            }
        } else {
            self.scan_class_set_source_character(ch)
        }
    }

    fn scan_class_set_source_character(&mut self, ch: char) -> Vec<u8> {
        match ch {
            '/' | '(' | ')' | '[' | ']' | '{' | '}' | '-' | '|' => {
                self.error_with_args(
                    &diagnostics::Unexpected_0_Did_you_mean_to_escape_it_with_backslash,
                    self.pos(),
                    1,
                    vec![ch.to_string()],
                );
                self.inc_pos(1);
                char_bytes(ch)
            }
            _ => self.scan_source_character(),
        }
    }

    pub fn scan_class_atom(&mut self) -> Vec<u8> {
        if self.char() == '\\' {
            self.inc_pos(1);
            match self.char() {
                'b' => {
                    self.inc_pos(1);
                    b"\x08".to_vec()
                }
                '-' => {
                    let ch = self.char();
                    self.inc_pos(1);
                    char_bytes(ch)
                }
                _ => {
                    if self.scan_character_class_escape() {
                        return Vec::new();
                    }
                    self.scan_character_escape_bytes(false)
                }
            }
        } else {
            self.scan_source_character()
        }
    }

    pub fn scan_character_class_escape(&mut self) -> bool {
        let mut is_character_complement = false;
        let start = self.pos().saturating_sub(1);
        let ch = self.char();
        match ch {
            'd' | 'D' | 's' | 'S' | 'w' | 'W' => {
                self.inc_pos(1);
                true
            }
            'P' | 'p' => {
                if ch == 'P' {
                    is_character_complement = true;
                }
                self.inc_pos(1);
                if self.char() == '{' {
                    self.inc_pos(1);
                    let property_name_or_value_start = self.pos();
                    let property_name_or_value = self.scan_word_characters();
                    if self.char() == '=' {
                        let non_binary = non_binary_unicode_properties();
                        let property_name = non_binary
                            .get(property_name_or_value.as_str())
                            .copied()
                            .unwrap_or("");
                        if self.pos() == property_name_or_value_start {
                            self.error(
                                &diagnostics::Expected_a_Unicode_property_name,
                                self.pos(),
                                0,
                            );
                        } else if property_name.is_empty() {
                            self.error(
                                &diagnostics::Unknown_Unicode_property_name,
                                property_name_or_value_start,
                                self.pos() - property_name_or_value_start,
                            );
                            let suggestion = self
                                .get_spelling_suggestion_for_unicode_property_name(
                                    &property_name_or_value,
                                );
                            if !suggestion.is_empty() {
                                self.error_with_args(
                                    &diagnostics::Did_you_mean_0,
                                    property_name_or_value_start,
                                    self.pos() - property_name_or_value_start,
                                    vec![suggestion],
                                );
                            }
                        }
                        self.inc_pos(1);
                        let property_value_start = self.pos();
                        let property_value = self.scan_word_characters();
                        if self.pos() == property_value_start {
                            self.error(
                                &diagnostics::Expected_a_Unicode_property_value,
                                self.pos(),
                                0,
                            );
                        } else if !property_name.is_empty() {
                            let values = values_of_non_binary_unicode_properties();
                            if let Some(property_values) = values.get(property_name)
                                && !property_values.contains(property_value.as_str())
                            {
                                self.error(
                                    &diagnostics::Unknown_Unicode_property_value,
                                    property_value_start,
                                    self.pos() - property_value_start,
                                );
                                let suggestion = self
                                    .get_spelling_suggestion_for_unicode_property_value(
                                        property_name,
                                        &property_value,
                                    );
                                if !suggestion.is_empty() {
                                    self.error_with_args(
                                        &diagnostics::Did_you_mean_0,
                                        property_value_start,
                                        self.pos() - property_value_start,
                                        vec![suggestion],
                                    );
                                }
                            }
                        }
                    } else {
                        if self.pos() == property_name_or_value_start {
                            self.error(
                                &diagnostics::Expected_a_Unicode_property_name_or_value,
                                self.pos(),
                                0,
                            );
                        } else if binary_unicode_properties_of_strings()
                            .contains(property_name_or_value.as_str())
                        {
                            if !self.unicode_sets_mode {
                                self.error(&diagnostics::Any_Unicode_property_that_would_possibly_match_more_than_a_single_character_is_only_available_when_the_Unicode_Sets_v_flag_is_set, property_name_or_value_start, self.pos() - property_name_or_value_start);
                            } else if is_character_complement {
                                self.error(&diagnostics::Anything_that_would_possibly_match_more_than_a_single_character_is_invalid_inside_a_negated_character_class, property_name_or_value_start, self.pos() - property_name_or_value_start);
                            } else {
                                self.may_contain_strings = true;
                            }
                        } else {
                            let values = values_of_non_binary_unicode_properties();
                            let general_category = values.get("General_Category");
                            let is_general_category = general_category
                                .map(|v| v.contains(property_name_or_value.as_str()))
                                .unwrap_or(false);
                            if !is_general_category
                                && !binary_unicode_properties()
                                    .contains(property_name_or_value.as_str())
                            {
                                self.error(
                                    &diagnostics::Unknown_Unicode_property_name_or_value,
                                    property_name_or_value_start,
                                    self.pos() - property_name_or_value_start,
                                );
                                let suggestion = self
                                    .get_spelling_suggestion_for_unicode_property_name_or_value(
                                        &property_name_or_value,
                                    );
                                if !suggestion.is_empty() {
                                    self.error_with_args(
                                        &diagnostics::Did_you_mean_0,
                                        property_name_or_value_start,
                                        self.pos() - property_name_or_value_start,
                                        vec![suggestion],
                                    );
                                }
                            }
                        }
                    }
                    self.scan_expected_char('}');
                    if !self.any_unicode_mode {
                        self.error(
                            &diagnostics::Unicode_property_value_expressions_are_only_available_when_the_Unicode_u_flag_or_the_Unicode_Sets_v_flag_is_set,
                            start,
                            self.pos() - start,
                        );
                    }
                } else if self.any_unicode_mode_or_non_annex_b {
                    self.error_with_args(
                        &diagnostics::X_0_must_be_followed_by_a_Unicode_property_value_expression_enclosed_in_braces,
                        self.pos().saturating_sub(2),
                        2,
                        vec![ch.to_string()],
                    );
                } else {
                    self.inc_pos(-1);
                    return false;
                }
                true
            }
            _ => false,
        }
    }

    pub fn get_spelling_suggestion_for_unicode_property_name(&self, name: &str) -> String {
        core::get_spelling_suggestion_for_strings(
            name,
            non_binary_unicode_properties()
                .keys()
                .map(|key| (*key).to_string()),
        )
    }

    pub fn get_spelling_suggestion_for_unicode_property_value(
        &self,
        property_name: &str,
        value: &str,
    ) -> String {
        let values = values_of_non_binary_unicode_properties();
        let Some(values) = values.get(property_name) else {
            return String::new();
        };
        core::get_spelling_suggestion_for_strings(
            value,
            values.iter().map(|value| (*value).to_string()),
        )
    }

    pub fn get_spelling_suggestion_for_unicode_property_name_or_value(&self, name: &str) -> String {
        let values = values_of_non_binary_unicode_properties();
        let mut candidates: Vec<String> = values
            .get("General_Category")
            .into_iter()
            .flat_map(|values| values.iter().map(|value| (*value).to_string()))
            .collect();
        candidates.extend(
            binary_unicode_properties()
                .iter()
                .map(|value| (*value).to_string()),
        );
        candidates.extend(
            binary_unicode_properties_of_strings()
                .iter()
                .map(|value| (*value).to_string()),
        );
        core::get_spelling_suggestion_for_strings(name, candidates)
    }

    pub fn scan_word_characters(&mut self) -> String {
        let start = self.pos();
        while self.pos() < self.end {
            let ch = self.char();
            if !is_word_character(ch) {
                break;
            }
            self.inc_pos(ch.len_utf8() as isize);
        }
        self.text()[start..self.pos()].to_string()
    }

    pub fn scan_source_character(&mut self) -> Vec<u8> {
        if self.pos() >= self.end {
            return Vec::new();
        }

        if !self.any_unicode_mode {
            if let Some(low) = self.pending_low_surrogate.take() {
                let size = self.decode_rune_at_pos().map_or(1, |(_, size)| size);
                self.inc_pos(size as isize);
                return encode_surrogate(low);
            }

            let Some((ch, size)) = self.decode_rune_at_pos() else {
                return Vec::new();
            };
            let code = ch as u32;
            if code >= SURR_SELF {
                let high = SURR1 + ((code - SURR_SELF) >> 10);
                let low = SURR2 + ((code - SURR_SELF) & 0x3ff);
                self.pending_low_surrogate = Some(low);
                return encode_surrogate(high);
            }

            self.inc_pos(size as isize);
            return char_bytes(ch);
        }

        if let Some((ch, size)) = self.decode_rune_at_pos() {
            self.inc_pos(size as isize);
            char_bytes(ch)
        } else {
            Vec::new()
        }
    }

    fn scan_character_escape_bytes(&mut self, atom_escape: bool) -> Vec<u8> {
        if self.char() == 'u' && !self.any_unicode_mode {
            let start = self.pos().saturating_sub(1);
            if self.pos() + 1 < self.end && self.char_at(self.pos() + 1) == '{' {
                return self.scan_character_escape(atom_escape).into_bytes();
            }

            self.set_pos(start);
            let code_point = self.scanner.scan_unicode_escape(true);
            if let Some(code_point) = code_point {
                if code_point_is_high_surrogate(code_point)
                    || code_point_is_low_surrogate(code_point)
                {
                    return encode_surrogate(code_point);
                }
                if let Some(ch) = char::from_u32(code_point) {
                    return char_bytes(ch);
                }
            }
            return self.text().as_bytes()[start..self.pos()].to_vec();
        }

        self.scan_character_escape(atom_escape).into_bytes()
    }

    pub fn scan_expected_char(&mut self, ch: char) {
        if self.char() == ch {
            self.inc_pos(ch.len_utf8() as isize);
        } else {
            self.error_with_args(
                &diagnostics::X_0_expected,
                self.pos(),
                0,
                vec![ch.to_string()],
            );
        }
    }

    pub fn scan_digits(&mut self) {
        let start = self.pos();
        while self.pos() < self.end && self.char().is_ascii_digit() {
            self.inc_pos(1);
        }
        self.scanner.set_token_value_from_range(start, self.pos());
    }

    pub fn run(&mut self) {
        self.any_unicode_mode_or_non_annex_b = self.any_unicode_mode || !self.annex_b;

        self.scan_disjunction(false);

        for reference in std::mem::take(&mut self.group_name_references) {
            if !self
                .group_specifiers
                .get(&reference.name)
                .copied()
                .unwrap_or(false)
            {
                self.error_with_args(
                    &diagnostics::There_is_no_capturing_group_named_0_in_this_regular_expression,
                    reference.pos,
                    reference.end - reference.pos,
                    vec![reference.name.clone()],
                );
                if !self.group_specifiers.is_empty() {
                    let suggestion = core::get_spelling_suggestion_for_strings(
                        &reference.name,
                        self.group_specifiers.keys().cloned(),
                    );
                    if !suggestion.is_empty() {
                        self.error_with_args(
                            &diagnostics::Did_you_mean_0,
                            reference.pos,
                            reference.end - reference.pos,
                            vec![suggestion],
                        );
                    }
                }
            }
        }
        for escape in std::mem::take(&mut self.decimal_escapes) {
            if escape.value > self.number_of_capturing_groups {
                if self.number_of_capturing_groups > 0 {
                    self.error_with_args(
                        &diagnostics::This_backreference_refers_to_a_group_that_does_not_exist_There_are_only_0_capturing_groups_in_this_regular_expression,
                        escape.pos,
                        escape.end - escape.pos,
                        vec![self.number_of_capturing_groups.to_string()],
                    );
                } else {
                    self.error(
                        &diagnostics::This_backreference_refers_to_a_group_that_does_not_exist_There_are_no_capturing_groups_in_this_regular_expression,
                        escape.pos,
                        escape.end - escape.pos,
                    );
                }
            }
        }
    }

    fn two_chars_at_pos(&self) -> Option<[u8; 2]> {
        if self.pos() + 1 < self.end {
            let bytes = self.text().as_bytes();
            Some([bytes[self.pos()], bytes[self.pos() + 1]])
        } else {
            None
        }
    }

    fn decode_rune_at_pos(&self) -> Option<(char, usize)> {
        if self.pos() >= self.end {
            return None;
        }
        match std::str::from_utf8(&self.text().as_bytes()[self.pos()..]) {
            Ok(text) => text.chars().next().map(|ch| (ch, ch.len_utf8())),
            Err(err) if err.valid_up_to() == 0 => Some((char::REPLACEMENT_CHARACTER, 1)),
            Err(err) => {
                let text = std::str::from_utf8(
                    &self.text().as_bytes()[self.pos()..self.pos() + err.valid_up_to()],
                )
                .ok()?;
                text.chars().next().map(|ch| (ch, ch.len_utf8()))
            }
        }
    }
}

pub fn compare_decimal_strings(a: &str, b: &str) -> i32 {
    let mut a = a.trim_start_matches('0');
    let mut b = b.trim_start_matches('0');
    if a.is_empty() {
        a = "0";
    }
    if b.is_empty() {
        b = "0";
    }
    if a.len() != b.len() {
        return if a.len() < b.len() { -1 } else { 1 };
    }
    match a.cmp(b) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

fn char_bytes(ch: char) -> Vec<u8> {
    let mut bytes = [0; 4];
    ch.encode_utf8(&mut bytes).as_bytes().to_vec()
}
