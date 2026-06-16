#![forbid(unsafe_code)]
use std::{cmp::Ordering, error::Error, fmt};

#[cfg(test)]
mod version_range_test;
#[cfg(test)]
mod version_test;

// https://semver.org/#spec-item-2
// > A normal version number MUST take the form X.Y.Z where X, Y, and Z are non-negative
// > integers, and MUST NOT contain leading zeroes. X is the major version, Y is the minor
// > version, and Z is the patch version. Each element MUST increase numerically.
//
// NOTE: We differ here in that we allow X and X.Y, with missing parts having the default
// value of `0`.
//
// https://semver.org/#spec-item-9
// > A pre-release version MAY be denoted by appending a hyphen and a series of dot separated
// > identifiers immediately following the patch version. Identifiers MUST comprise only ASCII
// > alphanumerics and hyphen [0-9A-Za-z-]. Identifiers MUST NOT be empty. Numeric identifiers
// > MUST NOT include leading zeroes.
//
// https://semver.org/#spec-item-10
// > Build metadata MAY be denoted by appending a plus sign and a series of dot separated
// > identifiers immediately following the patch or pre-release version. Identifiers MUST
// > comprise only ASCII alphanumerics and hyphen [0-9A-Za-z-]. Identifiers MUST NOT be empty.
//
// https://semver.org/#spec-item-9
// > Numeric identifiers MUST NOT include leading zeroes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Version {
    major: u32,
    minor: u32,
    patch: u32,
    prerelease: Vec<String>,
    build: Vec<String>,
}

pub fn version_zero() -> Version {
    Version {
        prerelease: vec!["0".to_owned()],
        ..Version::default()
    }
}

impl Version {
    pub fn increment_major(&self) -> Version {
        Version {
            major: self.major + 1,
            ..Version::default()
        }
    }

    pub fn increment_minor(&self) -> Version {
        Version {
            major: self.major,
            minor: self.minor + 1,
            ..Version::default()
        }
    }

    pub fn increment_patch(&self) -> Version {
        Version {
            major: self.major,
            minor: self.minor,
            patch: self.patch + 1,
            ..Version::default()
        }
    }

    pub fn compare(&self, b: Option<&Version>) -> i32 {
        // https://semver.org/#spec-item-11
        // > Precedence is determined by the first difference when comparing each of these
        // > identifiers from left to right as follows: Major, minor, and patch versions are
        // > always compared numerically.
        //
        // https://semver.org/#spec-item-11
        // > Precedence for two pre-release versions with the same major, minor, and patch version
        // > MUST be determined by comparing each dot separated identifier from left to right until
        // > a difference is found [...]
        //
        // https://semver.org/#spec-item-11
        // > Build metadata does not figure into precedence
        let Some(b) = b else {
            return COMPARISON_GREATER_THAN;
        };

        ordering_to_comparison(self.major.cmp(&b.major))
            .then_with(|| ordering_to_comparison(self.minor.cmp(&b.minor)))
            .then_with(|| ordering_to_comparison(self.patch.cmp(&b.patch)))
            .then_with(|| compare_pre_release_identifiers(&self.prerelease, &b.prerelease))
    }
}

trait ComparisonExt {
    fn then_with(self, f: impl FnOnce() -> i32) -> i32;
}

impl ComparisonExt for i32 {
    fn then_with(self, f: impl FnOnce() -> i32) -> i32 {
        if self != COMPARISON_EQUAL_TO {
            self
        } else {
            f()
        }
    }
}

const COMPARISON_LESS_THAN: i32 = -1;
const COMPARISON_EQUAL_TO: i32 = 0;
const COMPARISON_GREATER_THAN: i32 = 1;

fn compare_pre_release_identifiers(left: &[String], right: &[String]) -> i32 {
    // https://semver.org/#spec-item-11
    // > When major, minor, and patch are equal, a pre-release version has lower precedence
    // > than a normal version.
    if left.is_empty() {
        if right.is_empty() {
            return COMPARISON_EQUAL_TO;
        }
        return COMPARISON_GREATER_THAN;
    } else if right.is_empty() {
        return COMPARISON_LESS_THAN;
    }

    // https://semver.org/#spec-item-11
    // > Precedence for two pre-release versions with the same major, minor, and patch version
    // > MUST be determined by comparing each dot separated identifier from left to right until
    // > a difference is found [...]
    for (l, r) in left.iter().zip(right.iter()) {
        let result = compare_pre_release_identifier(l, r);
        if result != COMPARISON_EQUAL_TO {
            return result;
        }
    }
    ordering_to_comparison(left.len().cmp(&right.len()))
}

fn compare_pre_release_identifier(left: &str, right: &str) -> i32 {
    // https://semver.org/#spec-item-11
    // > Precedence for two pre-release versions with the same major, minor, and patch version
    // > MUST be determined by comparing each dot separated identifier from left to right until
    // > a difference is found [...]
    let compare_result = ordering_to_comparison(left.cmp(right));
    if compare_result == COMPARISON_EQUAL_TO {
        return compare_result;
    }

    let left_is_numeric = is_numeric_identifier(left);
    let right_is_numeric = is_numeric_identifier(right);

    if left_is_numeric || right_is_numeric {
        // https://semver.org/#spec-item-11
        // > Numeric identifiers always have lower precedence than non-numeric identifiers.
        if !right_is_numeric {
            return COMPARISON_LESS_THAN;
        }
        if !left_is_numeric {
            return COMPARISON_GREATER_THAN;
        }

        // https://semver.org/#spec-item-11
        // > identifiers consisting of only digits are compared numerically
        let (Ok(left_as_number), Ok(right_as_number)) =
            (get_uint_component(left), get_uint_component(right))
        else {
            // This should only happen in the event of an overflow.
            // If so, use the lengths or fall back to string comparison.
            let len_compare = ordering_to_comparison(left.len().cmp(&right.len()));
            if len_compare == COMPARISON_EQUAL_TO {
                return compare_result;
            } else {
                return len_compare;
            }
        };
        return ordering_to_comparison(left_as_number.cmp(&right_as_number));
    }

    // https://semver.org/#spec-item-11
    // > identifiers with letters or hyphens are compared lexically in ASCII sort order.
    compare_result
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if !self.prerelease.is_empty() {
            write!(f, "-{}", self.prerelease.join("."))?;
        }
        if !self.build.is_empty() {
            write!(f, "+{}", self.build.join("."))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SemverParseError {
    orig_input: String,
}

impl fmt::Display for SemverParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Could not parse version string from {:?}",
            self.orig_input
        )
    }
}

impl Error for SemverParseError {}

// https://github.com/npm/node-semver#range-grammar
//
// range-set    ::= range ( logical-or range ) *
// range        ::= hyphen | simple ( ' ' simple ) * | ”
// logical-or   ::= ( ' ' ) * '||' ( ' ' ) *
//
// https://github.com/npm/node-semver#range-grammar
//
// partial      ::= xr ( '.' xr ( '.' xr qualifier ? )? )?
// xr           ::= 'x' | 'X' | '*' | nr
// nr           ::= '0' | ['1'-'9'] ( ['0'-'9'] ) *
// qualifier    ::= ( '-' pre )? ( '+' build )?
// pre          ::= parts
// build        ::= parts
// parts        ::= part ( '.' part ) *
// part         ::= nr | [-0-9A-Za-z]+
//
// https://github.com/npm/node-semver#range-grammar
//
// hyphen       ::= partial ' - ' partial
//
// https://github.com/npm/node-semver#range-grammar
//
// simple       ::= primitive | partial | tilde | caret
// primitive    ::= ( '<' | '>' | '>=' | '<=' | '=' ) partial
// tilde        ::= '~' partial
// caret        ::= '^' partial
pub struct VersionRange {
    alternatives: Vec<Vec<VersionComparator>>,
}

struct VersionComparator {
    operator: ComparatorOperator,
    operand: Version,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ComparatorOperator {
    LessThan,
    LessThanEqual,
    Equal,
    GreaterThanEqual,
    GreaterThan,
    Tilde,
    Caret,
}

impl ComparatorOperator {
    fn as_str(self) -> &'static str {
        match self {
            ComparatorOperator::LessThan => "<",
            ComparatorOperator::LessThanEqual => "<=",
            ComparatorOperator::Equal => "=",
            ComparatorOperator::GreaterThanEqual => ">=",
            ComparatorOperator::GreaterThan => ">",
            ComparatorOperator::Tilde => "~",
            ComparatorOperator::Caret => "^",
        }
    }
}

impl fmt::Display for VersionRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = String::new();
        format_disjunction(&mut s, &self.alternatives);
        f.write_str(&s)
    }
}

fn format_disjunction(s: &mut String, alternatives: &[Vec<VersionComparator>]) {
    let orig_len = s.len();

    for (i, alternative) in alternatives.iter().enumerate() {
        if i > 0 {
            s.push_str(" || ");
        }
        format_alternative(s, alternative);
    }

    if s.len() == orig_len {
        s.push('*');
    }
}

fn format_alternative(s: &mut String, comparators: &[VersionComparator]) {
    for (i, comparator) in comparators.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        format_comparator(s, comparator);
    }
}

fn format_comparator(s: &mut String, comparator: &VersionComparator) {
    s.push_str(comparator.operator.as_str());
    s.push_str(&comparator.operand.to_string());
}

impl VersionRange {
    pub fn test(&self, version: &Version) -> bool {
        test_disjunction(&self.alternatives, version)
    }
}

fn test_disjunction(alternatives: &[Vec<VersionComparator>], version: &Version) -> bool {
    // an empty disjunction is treated as "*" (all versions)
    if alternatives.is_empty() {
        return true;
    }

    for alternative in alternatives {
        if test_alternative(alternative, version) {
            return true;
        }
    }

    false
}

fn test_alternative(alternative: &[VersionComparator], version: &Version) -> bool {
    for comparator in alternative {
        if !test_comparator(comparator, version) {
            return false;
        }
    }
    true
}

fn test_comparator(comparator: &VersionComparator, version: &Version) -> bool {
    let cmp = version.compare(Some(&comparator.operand));
    match comparator.operator {
        ComparatorOperator::LessThan => cmp < 0,
        ComparatorOperator::LessThanEqual => cmp <= 0,
        ComparatorOperator::Equal => cmp == 0,
        ComparatorOperator::GreaterThanEqual => cmp >= 0,
        ComparatorOperator::GreaterThan => cmp > 0,
        _ => panic!("Unexpected operator: {}", comparator.operator.as_str()),
    }
}

pub fn try_parse_version_range(text: &str) -> (VersionRange, bool) {
    let (alternatives, ok) = parse_alternatives(text);
    (VersionRange { alternatives }, ok)
}

fn parse_alternatives(text: &str) -> (Vec<Vec<VersionComparator>>, bool) {
    let mut alternatives = Vec::new();

    let text = text.trim();
    let ranges = text.split("||");
    for mut r in ranges {
        r = r.trim();
        if r.is_empty() {
            continue;
        }

        let mut comparators = Vec::new();

        if let Some((left, right)) = parse_hyphen_range(r) {
            let (parsed_comparators, ok) = parse_hyphen(left, right);
            if ok {
                comparators.extend(parsed_comparators);
            } else {
                return (Vec::new(), false);
            }
        } else {
            for simple in r.split_whitespace() {
                let Some((op, text)) = parse_range(simple.trim()) else {
                    return (Vec::new(), false);
                };

                let (parsed_comparators, ok) = parse_comparator(op, text);
                if ok {
                    comparators.extend(parsed_comparators);
                } else {
                    return (Vec::new(), false);
                }
            }
        }

        alternatives.push(comparators);
    }

    (alternatives, true)
}

fn parse_hyphen_range(text: &str) -> Option<(&str, &str)> {
    for (i, ch) in text.char_indices() {
        if ch != '-' {
            continue;
        }
        let before = &text[..i];
        let after = &text[i + 1..];
        if before.chars().last().is_some_and(char::is_whitespace)
            && after.chars().next().is_some_and(char::is_whitespace)
        {
            let left = before.trim();
            let right = after.trim();
            if !left.is_empty() && !right.is_empty() && is_range_atom(left) && is_range_atom(right)
            {
                return Some((left, right));
            }
        }
    }
    None
}

fn parse_range(text: &str) -> Option<(Option<ComparatorOperator>, &str)> {
    let (op, rest) = if let Some(rest) = text.strip_prefix("<=") {
        (Some(ComparatorOperator::LessThanEqual), rest)
    } else if let Some(rest) = text.strip_prefix(">=") {
        (Some(ComparatorOperator::GreaterThanEqual), rest)
    } else if let Some(rest) = text.strip_prefix('<') {
        (Some(ComparatorOperator::LessThan), rest)
    } else if let Some(rest) = text.strip_prefix('>') {
        (Some(ComparatorOperator::GreaterThan), rest)
    } else if let Some(rest) = text.strip_prefix('=') {
        (Some(ComparatorOperator::Equal), rest)
    } else if let Some(rest) = text.strip_prefix('~') {
        (Some(ComparatorOperator::Tilde), rest)
    } else if let Some(rest) = text.strip_prefix('^') {
        (Some(ComparatorOperator::Caret), rest)
    } else {
        (None, text)
    };
    let rest = rest.trim_start();
    if rest.is_empty() || !is_range_atom(rest) {
        return None;
    }
    Some((op, rest))
}

fn is_range_atom(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '+' | '.' | '*'))
}

fn parse_hyphen(left: &str, right: &str) -> (Vec<VersionComparator>, bool) {
    let (left_result, left_ok) = parse_partial(left);
    if !left_ok {
        return (Vec::new(), false);
    }

    let (right_result, right_ok) = parse_partial(right);
    if !right_ok {
        return (Vec::new(), false);
    }

    let mut comparators = Vec::new();
    if !is_wildcard(&left_result.major_str) {
        // `MAJOR.*.*-...` gives us `>=MAJOR.0.0 ...`
        comparators.push(VersionComparator {
            operator: ComparatorOperator::GreaterThanEqual,
            operand: left_result.version,
        });
    }

    if !is_wildcard(&right_result.major_str) {
        let operator;
        let mut operand = right_result.version;

        if is_wildcard(&right_result.minor_str) {
            // `...-MAJOR.*.*` gives us `... <(MAJOR+1).0.0`
            operand = operand.increment_major();
            operator = ComparatorOperator::LessThan;
        } else if is_wildcard(&right_result.patch_str) {
            // `...-MAJOR.MINOR.*` gives us `... <MAJOR.(MINOR+1).0`
            operand = operand.increment_minor();
            operator = ComparatorOperator::LessThan;
        } else {
            // `...-MAJOR.MINOR.PATCH` gives us `... <=MAJOR.MINOR.PATCH`
            operator = ComparatorOperator::LessThanEqual;
        }

        comparators.push(VersionComparator { operator, operand });
    }

    (comparators, true)
}

struct PartialVersion {
    version: Version,
    major_str: String,
    minor_str: String,
    patch_str: String,
}

// Produces a "partial" version
fn parse_partial(text: &str) -> (PartialVersion, bool) {
    let Some((version_part, build_str)) = split_optional(text, '+') else {
        return (partial_version_default(), false);
    };
    let Some((core_part, prerelease_str)) = split_optional_first(version_part, '-') else {
        return (partial_version_default(), false);
    };
    let parts: Vec<&str> = core_part.split('.').collect();
    if parts.is_empty() || parts.len() > 3 || parts.iter().any(|part| !is_xr(part)) {
        return (partial_version_default(), false);
    }
    if parts.len() < 3 && (prerelease_str.is_some() || build_str.is_some()) {
        return (partial_version_default(), false);
    }

    let major_str = parts[0].to_owned();
    let minor_str = parts.get(1).copied().unwrap_or("*").to_owned();
    let patch_str = parts.get(2).copied().unwrap_or("*").to_owned();

    let (major_numeric, minor_numeric, patch_numeric);
    if is_wildcard(&major_str) {
        major_numeric = 0;
        minor_numeric = 0;
        patch_numeric = 0;
    } else {
        let Ok(major) = get_uint_component(&major_str) else {
            return (partial_version_default(), false);
        };
        major_numeric = major;

        if is_wildcard(&minor_str) {
            minor_numeric = 0;
            patch_numeric = 0;
        } else {
            let Ok(minor) = get_uint_component(&minor_str) else {
                return (partial_version_default(), false);
            };
            minor_numeric = minor;

            if is_wildcard(&patch_str) {
                patch_numeric = 0;
            } else {
                let Ok(patch) = get_uint_component(&patch_str) else {
                    return (partial_version_default(), false);
                };
                patch_numeric = patch;
            }
        }
    }

    let prerelease = prerelease_str
        .map(|s| s.split('.').map(str::to_owned).collect())
        .unwrap_or_default();

    let build = build_str
        .map(|s| s.split('.').map(str::to_owned).collect())
        .unwrap_or_default();

    let result = PartialVersion {
        version: Version {
            major: major_numeric,
            minor: minor_numeric,
            patch: patch_numeric,
            prerelease,
            build,
        },
        major_str,
        minor_str,
        patch_str,
    };

    (result, true)
}

fn partial_version_default() -> PartialVersion {
    PartialVersion {
        version: Version::default(),
        major_str: String::new(),
        minor_str: String::new(),
        patch_str: String::new(),
    }
}

fn split_optional(text: &str, delimiter: char) -> Option<(&str, Option<&str>)> {
    let mut parts = text.split(delimiter);
    let before = parts.next().unwrap_or_default();
    let after = parts.next();
    if parts.next().is_some() || after == Some("") {
        return None;
    }
    if let Some(after) = after
        && !is_qualifier_part_list(after)
    {
        return None;
    }
    Some((before, after))
}

fn split_optional_first(text: &str, delimiter: char) -> Option<(&str, Option<&str>)> {
    let Some((before, after)) = text.split_once(delimiter) else {
        return Some((text, None));
    };
    if after.is_empty() || !is_qualifier_part_list(after) {
        return None;
    }
    Some((before, Some(after)))
}

fn is_qualifier_part_list(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '.')
}

fn is_xr(text: &str) -> bool {
    is_wildcard(text) || is_uint_component(text)
}

fn parse_comparator(op: Option<ComparatorOperator>, text: &str) -> (Vec<VersionComparator>, bool) {
    let mut operator = op.unwrap_or(ComparatorOperator::Equal);

    let (result, ok) = parse_partial(text);
    if !ok {
        return (Vec::new(), false);
    }

    let mut comparators_result = Vec::new();

    if !is_wildcard(&result.major_str) {
        match operator {
            ComparatorOperator::Tilde => {
                let first = VersionComparator {
                    operator: ComparatorOperator::GreaterThanEqual,
                    operand: result.version.clone(),
                };

                let second_version = if is_wildcard(&result.minor_str) {
                    result.version.increment_major()
                } else {
                    result.version.increment_minor()
                };

                let second = VersionComparator {
                    operator: ComparatorOperator::LessThan,
                    operand: second_version,
                };
                comparators_result = vec![first, second];
            }

            ComparatorOperator::Caret => {
                let first = VersionComparator {
                    operator: ComparatorOperator::GreaterThanEqual,
                    operand: result.version.clone(),
                };

                let second_version = if result.version.major > 0 || is_wildcard(&result.minor_str) {
                    result.version.increment_major()
                } else if result.version.minor > 0 || is_wildcard(&result.patch_str) {
                    result.version.increment_minor()
                } else {
                    result.version.increment_patch()
                };
                let second = VersionComparator {
                    operator: ComparatorOperator::LessThan,
                    operand: second_version,
                };
                comparators_result = vec![first, second];
            }

            ComparatorOperator::LessThan | ComparatorOperator::GreaterThanEqual => {
                let mut version = result.version;
                if is_wildcard(&result.minor_str) || is_wildcard(&result.patch_str) {
                    version.prerelease = vec!["0".to_owned()];
                }
                comparators_result = vec![VersionComparator {
                    operator,
                    operand: version,
                }];
            }

            ComparatorOperator::LessThanEqual | ComparatorOperator::GreaterThan => {
                let mut version = result.version;
                if is_wildcard(&result.minor_str) {
                    if operator == ComparatorOperator::LessThanEqual {
                        operator = ComparatorOperator::LessThan;
                    } else {
                        operator = ComparatorOperator::GreaterThanEqual;
                    }

                    version = version.increment_major();
                    version.prerelease = vec!["0".to_owned()];
                } else if is_wildcard(&result.patch_str) {
                    if operator == ComparatorOperator::LessThanEqual {
                        operator = ComparatorOperator::LessThan;
                    } else {
                        operator = ComparatorOperator::GreaterThanEqual;
                    }

                    version = version.increment_minor();
                    version.prerelease = vec!["0".to_owned()];
                }

                comparators_result = vec![VersionComparator {
                    operator,
                    operand: version,
                }];
            }
            ComparatorOperator::Equal => {
                // normalize empty string to `=`
                operator = ComparatorOperator::Equal;

                if is_wildcard(&result.minor_str) || is_wildcard(&result.patch_str) {
                    let original_version = result.version;

                    let mut first_version = original_version.clone();
                    first_version.prerelease = vec!["0".to_owned()];

                    let mut second_version = if is_wildcard(&result.minor_str) {
                        original_version.increment_major()
                    } else {
                        original_version.increment_minor()
                    };
                    second_version.prerelease = vec!["0".to_owned()];

                    comparators_result = vec![
                        VersionComparator {
                            operator: ComparatorOperator::GreaterThanEqual,
                            operand: first_version,
                        },
                        VersionComparator {
                            operator: ComparatorOperator::LessThan,
                            operand: second_version,
                        },
                    ];
                } else {
                    comparators_result = vec![VersionComparator {
                        operator,
                        operand: result.version,
                    }];
                }
            }
        }
    } else if operator == ComparatorOperator::LessThan
        || operator == ComparatorOperator::GreaterThan
    {
        comparators_result = vec![
            // < 0.0.0-0
            VersionComparator {
                operator: ComparatorOperator::LessThan,
                operand: version_zero(),
            },
        ];
    }

    (comparators_result, true)
}

fn is_wildcard(text: &str) -> bool {
    text == "*" || text == "x" || text == "X"
}

pub fn try_parse_version(text: &str) -> Result<Version, SemverParseError> {
    let mut result = Version::default();
    let (version_part, build_str) = split_once(text, '+', text)?;
    let (core_part, prerelease_str) = split_once_first(version_part, '-', text)?;
    let core_parts: Vec<&str> = core_part.split('.').collect();
    if core_parts.is_empty() || core_parts.len() > 3 {
        return Err(parse_error(text));
    }
    if core_parts.iter().any(|part| !is_uint_component(part)) {
        return Err(parse_error(text));
    }
    if core_parts.len() < 3 && (prerelease_str.is_some() || build_str.is_some()) {
        return Err(parse_error(text));
    }

    result.major = get_uint_component(core_parts[0]).map_err(|_| parse_error(text))?;
    if core_parts.len() > 1 {
        result.minor = get_uint_component(core_parts[1]).map_err(|_| parse_error(text))?;
    }
    if core_parts.len() > 2 {
        result.patch = get_uint_component(core_parts[2]).map_err(|_| parse_error(text))?;
    }

    if let Some(prerelease_str) = prerelease_str {
        if !is_prerelease(prerelease_str) {
            return Err(parse_error(text));
        }

        result.prerelease = prerelease_str.split('.').map(str::to_owned).collect();
    }
    if let Some(build_str) = build_str {
        if !is_build(build_str) {
            return Err(parse_error(text));
        }

        result.build = build_str.split('.').map(str::to_owned).collect();
    }

    Ok(result)
}

pub fn must_parse(text: &str) -> Version {
    match try_parse_version(text) {
        Ok(v) => v,
        Err(err) => panic!("{err}"),
    }
}

fn split_once<'a>(
    text: &'a str,
    delimiter: char,
    orig_input: &str,
) -> Result<(&'a str, Option<&'a str>), SemverParseError> {
    let mut parts = text.split(delimiter);
    let before = parts.next().unwrap_or_default();
    let after = parts.next();
    if parts.next().is_some() || after == Some("") {
        return Err(parse_error(orig_input));
    }
    Ok((before, after))
}

fn split_once_first<'a>(
    text: &'a str,
    delimiter: char,
    orig_input: &str,
) -> Result<(&'a str, Option<&'a str>), SemverParseError> {
    let Some((before, after)) = text.split_once(delimiter) else {
        return Ok((text, None));
    };
    if after.is_empty() {
        return Err(parse_error(orig_input));
    }
    Ok((before, Some(after)))
}

fn get_uint_component(text: &str) -> Result<u32, std::num::ParseIntError> {
    text.parse::<u32>()
}

fn is_uint_component(text: &str) -> bool {
    is_numeric_identifier(text)
}

fn is_numeric_identifier(text: &str) -> bool {
    if text == "0" {
        return true;
    }
    let mut chars = text.chars();
    matches!(chars.next(), Some('1'..='9')) && chars.all(|ch| ch.is_ascii_digit())
}

fn is_prerelease(text: &str) -> bool {
    !text.is_empty() && text.split('.').all(is_prerelease_part)
}

fn is_prerelease_part(text: &str) -> bool {
    if is_numeric_identifier(text) {
        return true;
    }
    let mut chars = text.chars();
    matches!(chars.next(), Some('A'..='Z' | 'a'..='z' | '-'))
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
}

fn is_build(text: &str) -> bool {
    !text.is_empty() && text.split('.').all(is_build_part)
}

fn is_build_part(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
}

fn ordering_to_comparison(ordering: Ordering) -> i32 {
    match ordering {
        Ordering::Less => COMPARISON_LESS_THAN,
        Ordering::Equal => COMPARISON_EQUAL_TO,
        Ordering::Greater => COMPARISON_GREATER_THAN,
    }
}

fn parse_error(text: &str) -> SemverParseError {
    SemverParseError {
        orig_input: text.to_owned(),
    }
}
