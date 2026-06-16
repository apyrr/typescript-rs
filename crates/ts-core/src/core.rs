use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Mutex, OnceLock};

use ts_json as json;
use ts_stringutil as stringutil;
use ts_tspath as tspath;

use crate::{CompilerOptions, ScriptKind, TextPos};

pub fn apply_debug_stack_limit() {
    let v = std::env::var("TS_GO_DEBUG_STACK_LIMIT").unwrap_or_default();
    if v.is_empty() {
        return;
    }
    let Ok(n) = v.parse::<usize>() else {
        return;
    };
    if n == 0 {
        return;
    }
    let _ = n;
}

pub fn filter<T: Clone>(slice: &[T], f: impl Fn(&T) -> bool) -> Vec<T> {
    for (i, value) in slice.iter().enumerate() {
        if !f(value) {
            let mut result = slice[..i].to_vec();
            for value in &slice[i + 1..] {
                if f(value) {
                    result.push(value.clone());
                }
            }
            return result;
        }
    }
    slice.to_vec()
}

pub fn filter_seq<'a, T: Clone>(
    slice: &'a [T],
    f: impl Fn(&T) -> bool + 'a,
) -> impl Iterator<Item = T> + 'a {
    slice.iter().filter(move |value| f(value)).cloned()
}

pub fn filter_index<T: Clone>(slice: &[T], f: impl Fn(&T, usize, &[T]) -> bool) -> Vec<T> {
    for (i, value) in slice.iter().enumerate() {
        if !f(value, i, slice) {
            let mut result = slice[..i].to_vec();
            for i in i + 1..slice.len() {
                let value = &slice[i];
                if f(value, i, slice) {
                    result.push(value.clone());
                }
            }
            return result;
        }
    }
    slice.to_vec()
}

pub fn map<T, U>(slice: &[T], f: impl Fn(&T) -> U) -> Vec<U> {
    slice.iter().map(f).collect()
}

pub fn try_map<T, U, E>(slice: &[T], f: impl Fn(&T) -> Result<U, E>) -> Result<Vec<U>, E> {
    if slice.is_empty() {
        return Ok(Vec::new());
    }
    let mut result = Vec::with_capacity(slice.len());
    for value in slice {
        result.push(f(value)?);
    }
    Ok(result)
}

pub fn map_index<T, U>(slice: &[T], f: impl Fn(&T, usize) -> U) -> Vec<U> {
    slice
        .iter()
        .enumerate()
        .map(|(i, value)| f(value, i))
        .collect()
}

pub fn map_non_nil<T, U: Default + PartialEq>(slice: &[T], f: impl Fn(&T) -> U) -> Vec<U> {
    let mut result = Vec::new();
    for value in slice {
        let mapped = f(value);
        if mapped != U::default() {
            result.push(mapped);
        }
    }
    result
}

pub fn map_filtered<T, U>(slice: &[T], f: impl Fn(&T) -> Option<U>) -> Vec<U> {
    let mut result = Vec::new();
    for value in slice {
        if let Some(mapped) = f(value) {
            result.push(mapped);
        }
    }
    result
}

pub fn flat_map<T, U>(slice: &[T], f: impl Fn(&T) -> Vec<U>) -> Vec<U> {
    let mut result = Vec::new();
    for value in slice {
        let mapped = f(value);
        if !mapped.is_empty() {
            result.extend(mapped);
        }
    }
    result
}

pub fn same_map<T: Clone + PartialEq>(slice: &[T], f: impl Fn(&T) -> T) -> Vec<T> {
    for (i, value) in slice.iter().enumerate() {
        let mapped = f(value);
        if mapped != *value {
            let mut result = Vec::with_capacity(slice.len());
            result.extend_from_slice(&slice[..i]);
            result.push(mapped);
            for value in &slice[i + 1..] {
                result.push(f(value));
            }
            return result;
        }
    }
    slice.to_vec()
}

pub fn same_map_index<T: Clone + PartialEq>(slice: &[T], f: impl Fn(&T, usize) -> T) -> Vec<T> {
    for (i, value) in slice.iter().enumerate() {
        let mapped = f(value, i);
        if mapped != *value {
            let mut result = Vec::with_capacity(slice.len());
            result.extend_from_slice(&slice[..i]);
            result.push(mapped);
            for (j, item) in slice.iter().enumerate().skip(i + 1) {
                result.push(f(item, j));
            }
            return result;
        }
    }
    slice.to_vec()
}

pub fn same<T>(s1: &[T], s2: &[T]) -> bool {
    s1.len() == s2.len() && (s1.is_empty() || std::ptr::eq(s1.as_ptr(), s2.as_ptr()))
}

pub fn some<T>(slice: &[T], f: impl Fn(&T) -> bool) -> bool {
    slice.iter().any(f)
}

pub fn every<T>(slice: &[T], f: impl Fn(&T) -> bool) -> bool {
    slice.iter().all(f)
}

pub type Predicate<T> = Box<dyn Fn(&T) -> bool>;

pub fn or<T>(funcs: Vec<Predicate<T>>) -> impl Fn(&T) -> bool {
    move |input| funcs.iter().any(|f| f(input))
}

pub fn find<T: Clone + Default>(slice: &[T], f: impl Fn(&T) -> bool) -> T {
    slice
        .iter()
        .find(|value| f(value))
        .cloned()
        .unwrap_or_default()
}

pub fn find_last<T: Clone + Default>(slice: &[T], f: impl Fn(&T) -> bool) -> T {
    slice
        .iter()
        .rev()
        .find(|value| f(value))
        .cloned()
        .unwrap_or_default()
}

pub fn find_index<T>(slice: &[T], f: impl Fn(&T) -> bool) -> isize {
    slice.iter().position(f).map_or(-1, |i| i as isize)
}

pub fn find_last_index<T>(slice: &[T], f: impl Fn(&T) -> bool) -> isize {
    slice.iter().rposition(f).map_or(-1, |i| i as isize)
}

pub fn first_or_nil<T: Clone + Default>(slice: &[T]) -> T {
    slice.first().cloned().unwrap_or_default()
}

pub fn last_or_nil<T: Clone + Default>(slice: &[T]) -> T {
    slice.last().cloned().unwrap_or_default()
}

pub fn element_or_nil<T: Clone + Default>(slice: &[T], index: usize) -> T {
    slice.get(index).cloned().unwrap_or_default()
}

pub fn first_or_nil_seq<T: Default>(seq: impl IntoIterator<Item = T>) -> T {
    seq.into_iter().next().unwrap_or_default()
}

pub fn first_non_nil<T, U: Default + PartialEq>(slice: &[T], f: impl Fn(&T) -> U) -> U {
    for value in slice {
        let mapped = f(value);
        if mapped != U::default() {
            return mapped;
        }
    }
    U::default()
}

pub fn first_non_zero<T: Default + PartialEq + Clone>(values: &[T]) -> T {
    let zero = T::default();
    for value in values {
        if *value != zero {
            return value.clone();
        }
    }
    zero
}

pub fn concatenate<T: Clone>(s1: &[T], s2: &[T]) -> Vec<T> {
    if s2.is_empty() {
        return s1.to_vec();
    }
    if s1.is_empty() {
        return s2.to_vec();
    }
    let mut result = Vec::with_capacity(s1.len() + s2.len());
    result.extend_from_slice(s1);
    result.extend_from_slice(s2);
    result
}

pub fn splice<T: Clone>(
    s1: &[T],
    mut start: isize,
    mut delete_count: isize,
    items: &[T],
) -> Vec<T> {
    if start < 0 {
        start += s1.len() as isize;
    }
    if start < 0 {
        start = 0;
    }
    if start > s1.len() as isize {
        start = s1.len() as isize;
    }
    if delete_count < 0 {
        delete_count = 0;
    }
    let start = start as usize;
    let end = (start + delete_count.max(0) as usize).min(s1.len());
    if start == end && items.is_empty() {
        return s1.to_vec();
    }
    let mut result = Vec::new();
    result.extend_from_slice(&s1[..start]);
    result.extend_from_slice(items);
    result.extend_from_slice(&s1[end..]);
    result
}

pub fn count_where<T>(slice: &[T], f: impl Fn(&T) -> bool) -> usize {
    slice.iter().filter(|value| f(value)).count()
}

pub fn replace_element<T: Clone>(slice: &[T], i: usize, t: T) -> Vec<T> {
    let mut result = slice.to_vec();
    result[i] = t;
    result
}

pub fn insert_sorted<T: Clone>(
    slice: &[T],
    element: T,
    cmp: impl Fn(&T, &T) -> std::cmp::Ordering,
) -> Vec<T> {
    let i = slice
        .binary_search_by(|probe| cmp(probe, &element))
        .unwrap_or_else(|i| i);
    let mut result = slice.to_vec();
    result.insert(i, element);
    result
}

// MinAllFunc returns all minimum elements from xs according to the comparison function cmp.
pub fn min_all_func<T: Clone>(xs: &[T], cmp: impl Fn(&T, &T) -> i32) -> Vec<T> {
    if xs.is_empty() {
        return Vec::new();
    }

    let mut m = xs[0].clone();
    let mut mins = vec![m.clone()];

    for x in &xs[1..] {
        let c = cmp(x, &m);
        match c.cmp(&0) {
            std::cmp::Ordering::Less => {
                m = x.clone();
                mins.clear();
                mins.push(x.clone());
            }
            std::cmp::Ordering::Equal => mins.push(x.clone()),
            std::cmp::Ordering::Greater => {}
        }
    }

    mins
}

pub fn append_if_unique<T: Clone + PartialEq>(slice: &[T], element: T) -> Vec<T> {
    if slice.contains(&element) {
        return slice.to_vec();
    }
    let mut result = slice.to_vec();
    result.push(element);
    result
}

pub fn memoize<T: Clone + Default>(
    mut create: Option<Box<dyn FnOnce() -> T>>,
) -> impl FnMut() -> T {
    let mut value = T::default();
    move || {
        if let Some(create) = create.take() {
            value = create();
        }
        value.clone()
    }
}

// Returns whenTrue if b is true; otherwise, returns whenFalse. IfElse should only be used when branches are either
// constant or precomputed as both branches will be evaluated regardless as to the value of b.
pub fn if_else<T>(b: bool, when_true: T, when_false: T) -> T {
    if b { when_true } else { when_false }
}

// Returns value if value is not the zero value of T; Otherwise, returns defaultValue. OrElse should only be used when
// defaultValue is constant or precomputed as its argument will be evaluated regardless as to the content of value.
pub fn or_else<T: Default + PartialEq>(value: T, default_value: T) -> T {
    if value != T::default() {
        value
    } else {
        default_value
    }
}

// Returns `a` if `a` is not `nil`; Otherwise, returns `b`. Coalesce is roughly analogous to `??` in JS, except that it
// non-shortcutting, so it is advised to only use a constant or precomputed value for `b`
pub fn coalesce<T>(a: Option<T>, b: Option<T>) -> Option<T> {
    if a.is_none() { b } else { a }
}

pub type ECMALineStarts = Vec<TextPos>;

pub fn compute_ecmaline_starts(text: &str) -> ECMALineStarts {
    compute_ecmaline_starts_seq(text).collect()
}

pub fn compute_ecma_line_starts(text: &str) -> ECMALineStarts {
    compute_ecmaline_starts(text)
}

pub struct ECMALineStartsSeq<'a> {
    text: &'a str,
    pos: TextPos,
    line_start: TextPos,
    done: bool,
}

impl Iterator for ECMALineStartsSeq<'_> {
    type Item = TextPos;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let text_len = self.text.len() as TextPos;
        while self.pos < text_len {
            let b = self.text.as_bytes()[self.pos as usize];
            if b < 0x80 {
                self.pos += 1;
                match b {
                    b'\r' => {
                        if self.pos < text_len && self.text.as_bytes()[self.pos as usize] == b'\n' {
                            self.pos += 1;
                        }
                        let line_start = self.line_start;
                        self.line_start = self.pos;
                        return Some(line_start);
                    }
                    b'\n' => {
                        let line_start = self.line_start;
                        self.line_start = self.pos;
                        return Some(line_start);
                    }
                    _ => {}
                }
            } else {
                let ch = self.text[self.pos as usize..].chars().next().unwrap();
                self.pos += ch.len_utf8() as TextPos;
                if stringutil::is_line_break(ch) {
                    let line_start = self.line_start;
                    self.line_start = self.pos;
                    return Some(line_start);
                }
            }
        }

        self.done = true;
        Some(self.line_start)
    }
}

pub fn compute_ecmaline_starts_seq(text: &str) -> ECMALineStartsSeq<'_> {
    ECMALineStartsSeq {
        text,
        pos: 0,
        line_start: 0,
        done: false,
    }
}

pub fn compute_ecma_line_starts_seq(text: &str) -> ECMALineStartsSeq<'_> {
    compute_ecmaline_starts_seq(text)
}

// PositionToLineAndByteOffset returns the 0-based line and byte offset from the
// start of that line for the given byte position, using the provided line starts.
// The byte offset is a raw UTF-8 byte offset from the line start, not a UTF-16 code unit count.
pub fn position_to_line_and_byte_offset(
    position: usize,
    line_starts: &[TextPos],
) -> (usize, usize) {
    let line = line_starts
        .partition_point(|line_start| *line_start as usize <= position)
        .saturating_sub(1);
    (line, position - line_starts[line] as usize)
}

// UTF16Offset represents a character offset measured in UTF-16 code units.
pub type UTF16Offset = i32;

// UTF16Len returns the number of UTF-16 code units needed to
// represent the given UTF-8 encoded string.
pub fn utf16len(s: &str) -> UTF16Offset {
    // Fast path: scan for non-ASCII bytes. For ASCII-only strings,
    // each byte is one UTF-16 code unit, so we can return len(s) directly.
    for i in 0..s.len() {
        if s.as_bytes()[i] >= 0x80 {
            // Found non-ASCII; count the ASCII prefix, then decode the rest.
            let mut n = i as UTF16Offset;
            for r in s[i..].chars() {
                n += r.len_utf16() as UTF16Offset;
            }
            return n;
        }
    }
    s.len() as UTF16Offset
}

pub fn utf16_len(s: &str) -> UTF16Offset {
    utf16len(s)
}

pub fn flatten<T: Clone>(array: &[Vec<T>]) -> Vec<T> {
    let mut result = Vec::new();
    for sub_array in array {
        result.extend_from_slice(sub_array);
    }
    result
}

pub fn must<T, E: std::fmt::Debug>(v: Result<T, E>) -> T {
    match v {
        Ok(v) => v,
        Err(err) => panic!("{err:?}"),
    }
}

// Extracts the first value of a multi-value return.
pub fn first_result<T1>(t1: T1) -> T1 {
    t1
}

#[macro_export]
macro_rules! first_result {
    ($first:expr $(, $rest:expr)* $(,)?) => {
        $first
    };
}

pub fn stringify_json<T: serde::Serialize>(
    input: &T,
    prefix: &str,
    indent: &str,
) -> Result<String, serde_json::Error> {
    Ok(
        String::from_utf8(json::marshal_indent(input, prefix, indent)?)
            .expect("JSON serialization should produce UTF-8"),
    )
}

pub fn get_script_kind_from_file_name(file_name: &str) -> ScriptKind {
    if let Some(dot_pos) = file_name.rfind('.') {
        match file_name[dot_pos..].to_lowercase().as_str() {
            tspath::EXTENSION_JS | tspath::EXTENSION_CJS | tspath::EXTENSION_MJS => {
                return ScriptKind::JS;
            }
            tspath::EXTENSION_JSX => return ScriptKind::JSX,
            tspath::EXTENSION_TS | tspath::EXTENSION_CTS | tspath::EXTENSION_MTS => {
                return ScriptKind::TS;
            }
            tspath::EXTENSION_TSX => return ScriptKind::TSX,
            tspath::EXTENSION_JSON => return ScriptKind::JSON,
            _ => {}
        }
    }
    ScriptKind::Unknown
}

// Given a name and a list of names that are *not* equal to the name, return a spelling suggestion if there is one that is close enough.
// Names less than length 3 only check for case-insensitive equality.
//
// find the candidate with the smallest Levenshtein distance,
//
//	except for candidates:
//	  * With no name
//	  * Whose length differs from the target name by more than 0.34 of the length of the name.
//	  * Whose levenshtein distance is more than 0.4 of the length of the name
//	    (0.4 allows 1 substitution/transposition for every 5 characters,
//	     and 1 insertion/deletion at 3 characters)
//
// @internal
pub fn get_spelling_suggestion<T: Clone + Default>(
    name: &str,
    candidates: impl IntoIterator<Item = T>,
    get_name: impl Fn(&T) -> String,
    compare: impl Fn(&T, &T) -> std::cmp::Ordering,
) -> T {
    let maximum_length_difference = 2.max((name.len() as f64 * 0.34) as usize);
    let mut best_distance = (name.len() as f64 * 0.4).floor() + 0.9; // If the best result is worse than this, don't bother.
    let rune_name: Vec<char> = name.chars().collect();
    let pool = levenshtein_buffers_pool();
    let mut buffers = pool.lock().unwrap().pop().unwrap_or_default();
    let mut best_candidate = T::default();
    let mut has_best = false;
    for candidate in candidates {
        let candidate_name = get_name(&candidate);
        let max_len = candidate_name.len().max(name.len());
        let min_len = candidate_name.len().min(name.len());
        if !candidate_name.is_empty() && max_len - min_len <= maximum_length_difference {
            if candidate_name == name {
                continue;
            }
            // Only consider candidates less than 3 characters long when they differ by case.
            // Otherwise, don't bother, since a user would usually notice differences of a 2-character name.
            if candidate_name.len() < 3 && !equal_fold(&candidate_name, name) {
                continue;
            }
            let rune_candidate: Vec<char> = candidate_name.chars().collect();
            let distance =
                levenshtein_with_max(&mut buffers, &rune_name, &rune_candidate, best_distance);
            if distance < 0.0 {
                continue;
            }
            if distance < best_distance {
                best_distance = distance;
                best_candidate = candidate;
                has_best = true;
            } else if !has_best || compare(&candidate, &best_candidate).is_lt() {
                best_candidate = candidate;
                has_best = true;
            }
        }
    }
    pool.lock().unwrap().push(buffers);
    best_candidate
}

pub fn get_spelling_suggestion_for_strings(
    name: &str,
    candidates: impl IntoIterator<Item = String>,
) -> String {
    get_spelling_suggestion(
        name,
        candidates,
        |candidate| candidate.clone(),
        |a, b| a.cmp(b),
    )
}

#[derive(Default)]
struct LevenshteinBuffers {
    previous: Vec<f64>,
    current: Vec<f64>,
}

fn levenshtein_buffers_pool() -> &'static Mutex<Vec<LevenshteinBuffers>> {
    static POOL: OnceLock<Mutex<Vec<LevenshteinBuffers>>> = OnceLock::new();
    POOL.get_or_init(|| Mutex::new(Vec::new()))
}

fn levenshtein_with_max(
    buffers: &mut LevenshteinBuffers,
    s1: &[char],
    s2: &[char],
    max_value: f64,
) -> f64 {
    let buffer_size = s2.len() + 1;
    buffers.previous.resize(buffer_size, 0.0);
    buffers.current.resize(buffer_size, 0.0);

    let big = max_value + 0.01;
    for i in 0..buffers.previous.len() {
        buffers.previous[i] = i as f64;
    }
    for i in 1..=s1.len() {
        let c1 = s1[i - 1];
        let min_j = ((i as f64 - max_value).ceil() as usize).max(1);
        let max_j = ((max_value + i as f64).floor() as usize).min(s2.len());
        let mut col_min = i as f64;
        buffers.current[0] = col_min;
        for j in 1..min_j {
            buffers.current[j] = big;
        }
        for j in min_j..=max_j {
            let substitution_distance = if to_lower_rune(s1[i - 1]) == to_lower_rune(s2[j - 1]) {
                buffers.previous[j - 1] + 0.1
            } else {
                buffers.previous[j - 1] + 2.0
            };
            let dist = if c1 == s2[j - 1] {
                buffers.previous[j - 1]
            } else {
                (buffers.previous[j] + 1.0)
                    .min((buffers.current[j - 1] + 1.0).min(substitution_distance))
            };
            buffers.current[j] = dist;
            col_min = col_min.min(dist);
        }
        for j in max_j + 1..=s2.len() {
            buffers.current[j] = big;
        }
        if col_min > max_value {
            // Give up -- everything in this column is > max and it can't get better in future columns.
            return -1.0;
        }
        std::mem::swap(&mut buffers.previous, &mut buffers.current);
    }
    let res = buffers.previous[s2.len()];
    if res > max_value {
        return -1.0;
    }
    res
}

fn to_lower_rune(ch: char) -> char {
    ch.to_lowercase().next().unwrap_or(ch)
}

fn equal_fold(a: &str, b: &str) -> bool {
    a == b
        || (a.chars().count() == b.chars().count()
            && a.chars()
                .zip(b.chars())
                .all(|(a, b)| to_lower_rune(a) == to_lower_rune(b)))
}

pub fn identity<T>(t: T) -> T {
    t
}

pub fn check_each_defined<T>(s: Vec<Option<T>>, msg: &str) -> Vec<T> {
    let mut result = Vec::with_capacity(s.len());
    for value in s {
        if value.is_none() {
            panic!("{msg}");
        }
        result.push(value.unwrap());
    }
    result
}

pub fn index_after(s: &str, pattern: &str, start_index: usize) -> isize {
    match s[start_index..].find(pattern) {
        None => -1,
        Some(matched) => (matched + start_index) as isize,
    }
}

pub fn should_rewrite_module_specifier(
    specifier: &str,
    compiler_options: &CompilerOptions,
) -> bool {
    compiler_options
        .rewrite_relative_import_extensions
        .is_true()
        && tspath::path_is_relative(specifier)
        && !tspath::is_declaration_file_name(specifier)
        && tspath::has_ts_file_extension(specifier)
}

pub fn single_element_slice<T>(element: Option<T>) -> Vec<T> {
    element.into_iter().collect()
}

pub fn concatenate_seq<T>(seqs: Vec<Box<dyn Iterator<Item = T>>>) -> impl Iterator<Item = T> {
    seqs.into_iter().flatten()
}

// Enumerate returns a sequence of (index, value) pairs from the input sequence.
pub fn enumerate<T>(seq: impl IntoIterator<Item = T>) -> impl Iterator<Item = (usize, T)> {
    seq.into_iter().enumerate()
}

fn comparable_values_equal<T: Eq>(a: &T, b: &T) -> bool {
    a == b
}

// DiffMaps compares two maps m1 and m2 and calls the provided callbacks for added, removed, and changed entries.
// onAdded is called for each key-value pair that is in m2 but not in m1.
// onRemoved is called for each key-value pair that is in m1 but not in m2.
// onChanged is called for each key where the value in m1 differs from the value in m2.
pub fn diff_maps<K, V>(
    m1: &HashMap<K, V>,
    m2: &HashMap<K, V>,
    on_added: Option<impl FnMut(&K, &V)>,
    on_removed: Option<impl FnMut(&K, &V)>,
    on_changed: Option<impl FnMut(&K, &V, &V)>,
) where
    K: Eq + Hash,
    V: Eq,
{
    diff_maps_func(
        m1,
        m2,
        comparable_values_equal,
        on_added,
        on_removed,
        on_changed,
    )
}

// DiffMapsFunc compares two maps m1 and m2 and calls the provided callbacks for added, removed, and changed entries.
// onAdded is called for each key-value pair that is in m2 but not in m1.
// onRemoved is called for each key-value pair that is in m1 but not in m2.
// onChanged is called for each key where the value in m1 differs from the value in m2.
pub fn diff_maps_func<K, V1, V2>(
    m1: &HashMap<K, V1>,
    m2: &HashMap<K, V2>,
    equal_values: impl Fn(&V1, &V2) -> bool,
    mut on_added: Option<impl FnMut(&K, &V2)>,
    mut on_removed: Option<impl FnMut(&K, &V1)>,
    mut on_changed: Option<impl FnMut(&K, &V1, &V2)>,
) where
    K: Eq + Hash,
{
    if let Some(on_added) = on_added.as_mut() {
        for (k, v2) in m2 {
            if !m1.contains_key(k) {
                on_added(k, v2);
            }
        }
    }
    if on_changed.is_none() && on_removed.is_none() {
        return;
    }
    for (k, v1) in m1 {
        if let Some(v2) = m2.get(k) {
            if !equal_values(v1, v2)
                && let Some(on_changed) = on_changed.as_mut()
            {
                on_changed(k, v1, v2);
            }
        } else {
            let Some(on_removed) = on_removed.as_mut() else {
                panic!("nil onRemoved");
            };
            on_removed(k, v1);
        }
    }
}

// CopyMapInto is maps.Copy, unless dst is nil, in which case it clones and returns src.
// Use CopyMapInto anywhere you would use maps.Copy preceded by a nil check and map initialization.
pub fn copy_map_into<K: Eq + Hash + Clone, V: Clone>(
    dst: Option<HashMap<K, V>>,
    src: &HashMap<K, V>,
) -> HashMap<K, V> {
    if let Some(mut dst) = dst {
        dst.extend(src.iter().map(|(k, v)| (k.clone(), v.clone())));
        return dst;
    }
    src.clone()
}

// UnorderedEqual returns true if s1 and s2 contain the same elements, regardless of order.
pub fn unordered_equal<T: Eq + Hash>(s1: &[T], s2: &[T]) -> bool {
    if s1.len() != s2.len() {
        return false;
    }
    let mut counts: HashMap<&T, isize> = HashMap::new();
    for v in s1 {
        *counts.entry(v).or_insert(0) += 1;
    }
    for v in s2 {
        let entry = counts.entry(v).or_insert(0);
        *entry -= 1;
        if *entry < 0 {
            return false;
        }
    }
    true
}

pub fn deduplicate<T: Clone + PartialEq>(slice: &[T]) -> Vec<T> {
    if slice.len() > 1 {
        for (i, value) in slice.iter().enumerate() {
            if slice[..i].contains(value) {
                let mut result = slice[..i].to_vec();
                for value in &slice[i + 1..] {
                    if !result.contains(value) {
                        result.push(value.clone());
                    }
                }
                return result;
            }
        }
    }
    slice.to_vec()
}

pub fn deduplicate_sorted<T: Clone>(slice: &[T], is_equal: impl Fn(&T, &T) -> bool) -> Vec<T> {
    if slice.is_empty() {
        return slice.to_vec();
    }
    let mut last = slice[0].clone();
    let mut deduplicated = vec![last.clone()];
    for next in &slice[1..] {
        if is_equal(&last, next) {
            continue;
        }

        deduplicated.push(next.clone());
        last = next.clone();
    }

    deduplicated
}

// CompareBooleans treats true as greater than false.
pub fn compare_booleans(a: bool, b: bool) -> i32 {
    if a && !b {
        1
    } else if !a && b {
        -1
    } else {
        0
    }
}
