use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, Deserialize, Hash, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Default, Clone, Copy, Deserialize, Hash, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormattingOptions {
    pub tab_size: u32,
    pub insert_spaces: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trim_trailing_whitespace: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert_final_newline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trim_final_newlines: Option<bool>,
}

// Implements a cmp.Compare like function for two Position
// ComparePositions(pos, other) == cmp.Compare(pos, other)
pub fn compare_positions(pos: Position, other: Position) -> Ordering {
    match pos.line.cmp(&other.line) {
        Ordering::Equal => pos.character.cmp(&other.character),
        order => order,
    }
}

impl Position {
    pub fn compare(&self, other: &Self) -> i32 {
        ordering_to_i32(compare_positions(*self, *other))
    }
}

// Implements a cmp.Compare like function for two Range
// CompareRanges(lsRange, other) == cmp.Compare(lsRange, other)
//
// Range.Start is compared before Range.End
pub fn compare_ranges(ls_range: Range, other: Range) -> Ordering {
    match compare_positions(ls_range.start, other.start) {
        Ordering::Equal => compare_positions(ls_range.end, other.end),
        order => order,
    }
}

impl Range {
    pub fn compare(&self, other: &Self) -> i32 {
        ordering_to_i32(compare_ranges(*self, *other))
    }
}

fn ordering_to_i32(order: Ordering) -> i32 {
    match order {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}
