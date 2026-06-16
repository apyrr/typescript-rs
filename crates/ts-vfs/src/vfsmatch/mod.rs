#[cfg(test)]
mod bench_test;
mod stringer_generated;
#[cfg(test)]
mod vfsmatch_test;

use std::collections::HashSet;

use crate::vfs::Fs;

pub use stringer_generated::Usage;

pub const UNLIMITED_DEPTH: i32 = i32::MAX;

pub fn read_directory(
    host: &dyn Fs,
    current_dir: &str,
    path: &str,
    extensions: &[String],
    excludes: &[String],
    includes: &[String],
    depth: i32,
) -> Vec<String> {
    match_files(MatchFilesOptions {
        path,
        extensions,
        excludes,
        includes,
        use_case_sensitive_file_names: host.use_case_sensitive_file_names(),
        current_directory: current_dir,
        depth,
        host,
    })
}

pub fn is_implicit_glob(last_path_component: &str) -> bool {
    !last_path_component.contains(['.', '*', '?'])
}

fn get_include_base_path(absolute: &str) -> String {
    let wildcard_offset = absolute.find(['*', '?']);
    if let Some(offset) = wildcard_offset {
        return absolute[..offset]
            .rsplit_once('/')
            .map(|(base, _)| if base.is_empty() { "/" } else { base })
            .unwrap_or("")
            .to_owned();
    }
    if has_extension(absolute) {
        get_directory_path(absolute)
    } else {
        remove_trailing_directory_separator(absolute)
    }
}

pub fn get_base_paths(
    path: &str,
    includes: &[String],
    use_case_sensitive_file_names: bool,
) -> Vec<String> {
    let mut base_paths = vec![path.to_owned()];
    if !includes.is_empty() {
        let mut include_base_paths = includes
            .iter()
            .map(|include| {
                let absolute = if is_rooted_disk_path(include) {
                    include.to_owned()
                } else {
                    normalize_path(&combine_paths(path, include))
                };
                get_include_base_path(&absolute)
            })
            .collect::<Vec<_>>();
        include_base_paths.sort_by(|a, b| compare_paths(a, b, use_case_sensitive_file_names));
        for include_base_path in include_base_paths {
            if base_paths
                .iter()
                .all(|base| !contains_path(base, &include_base_path, use_case_sensitive_file_names))
            {
                base_paths.push(include_base_path);
            }
        }
    }
    base_paths
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GlobPattern {
    components: Vec<Component>,
    is_exclude: bool,
    case_sensitive: bool,
    exclude_min_js: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Component {
    kind: ComponentKind,
    literal: String,
    segments: Vec<Segment>,
    skip_package_folders: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ComponentKind {
    Literal,
    Wildcard,
    DoubleAsterisk,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Segment {
    kind: SegmentKind,
    literal: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SegmentKind {
    Literal,
    Star,
    Question,
}

fn compile_glob_pattern(
    spec: &str,
    base_path: &str,
    usage: Usage,
    case_sensitive: bool,
) -> Option<GlobPattern> {
    let mut parts = get_normalized_path_components(spec, base_path);
    if usage != Usage::Exclude && parts.last().is_some_and(|part| part == "**") {
        return None;
    }
    if let Some(root) = parts.first_mut() {
        *root = remove_trailing_directory_separator(root);
    }
    if parts.last().is_some_and(|part| is_implicit_glob(part)) {
        parts.push("**".to_owned());
        parts.push("*".to_owned());
    }
    Some(GlobPattern {
        is_exclude: usage == Usage::Exclude,
        case_sensitive,
        exclude_min_js: usage == Usage::Files,
        components: parts
            .into_iter()
            .map(|part| parse_component(&part, usage != Usage::Exclude))
            .collect(),
    })
}

fn parse_component(value: &str, is_include: bool) -> Component {
    if value == "**" {
        return Component {
            kind: ComponentKind::DoubleAsterisk,
            literal: String::new(),
            segments: Vec::new(),
            skip_package_folders: false,
        };
    }
    if !value.contains(['*', '?']) {
        return Component {
            kind: ComponentKind::Literal,
            literal: value.to_owned(),
            segments: Vec::new(),
            skip_package_folders: false,
        };
    }
    Component {
        kind: ComponentKind::Wildcard,
        literal: String::new(),
        segments: parse_segments(value),
        skip_package_folders: is_include,
    }
}

fn parse_segments(value: &str) -> Vec<Segment> {
    let mut result = Vec::new();
    let mut start = 0;
    for (index, ch) in value.char_indices() {
        if ch == '*' || ch == '?' {
            if index > start {
                result.push(Segment {
                    kind: SegmentKind::Literal,
                    literal: value[start..index].to_owned(),
                });
            }
            result.push(Segment {
                kind: if ch == '*' {
                    SegmentKind::Star
                } else {
                    SegmentKind::Question
                },
                literal: String::new(),
            });
            start = index + ch.len_utf8();
        }
    }
    if start < value.len() {
        result.push(Segment {
            kind: SegmentKind::Literal,
            literal: value[start..].to_owned(),
        });
    }
    result
}

impl GlobPattern {
    fn matches(&self, path: &str) -> bool {
        self.match_path_parts(path, "", 0, 0, false)
    }

    fn matches_parts(&self, prefix: &str, suffix: &str) -> bool {
        self.match_path_parts(prefix, suffix, 0, 0, false)
    }

    fn matches_prefix_parts(&self, prefix: &str, suffix: &str) -> bool {
        self.match_path_parts(prefix, suffix, 0, 0, true)
    }

    fn match_path_parts(
        &self,
        prefix: &str,
        suffix: &str,
        mut path_offset: usize,
        mut component_index: usize,
        prefix_only: bool,
    ) -> bool {
        loop {
            let Some((path_part, next_offset)) = next_path_part_parts(prefix, suffix, path_offset)
            else {
                return if prefix_only {
                    true
                } else {
                    self.pattern_satisfied(component_index)
                };
            };
            if component_index >= self.components.len() {
                return self.is_exclude && !prefix_only;
            }
            let component = &self.components[component_index];
            match component.kind {
                ComponentKind::DoubleAsterisk => {
                    if self.match_path_parts(
                        prefix,
                        suffix,
                        path_offset,
                        component_index + 1,
                        prefix_only,
                    ) {
                        return true;
                    }
                    if !self.is_exclude
                        && (is_hidden_path(&path_part) || is_package_folder(&path_part))
                    {
                        return false;
                    }
                    path_offset = next_offset;
                    continue;
                }
                ComponentKind::Literal => {
                    if !self.strings_equal(&component.literal, &path_part) {
                        return false;
                    }
                }
                ComponentKind::Wildcard => {
                    if component.skip_package_folders && is_package_folder(&path_part) {
                        return false;
                    }
                    if !self.match_wildcard(&component.segments, &path_part) {
                        return false;
                    }
                }
            }
            path_offset = next_offset;
            component_index += 1;
        }
    }

    fn pattern_satisfied(&self, component_index: usize) -> bool {
        self.components[component_index..]
            .iter()
            .all(|component| component.kind == ComponentKind::DoubleAsterisk)
    }

    fn match_wildcard(&self, segments: &[Segment], value: &str) -> bool {
        if !self.is_exclude
            && !segments.is_empty()
            && is_hidden_path(value)
            && matches!(segments[0].kind, SegmentKind::Star | SegmentKind::Question)
        {
            return false;
        }
        if segments.len() == 2
            && segments[0].kind == SegmentKind::Star
            && segments[1].kind == SegmentKind::Literal
        {
            let suffix = &segments[1].literal;
            return value.len() >= suffix.len()
                && self.strings_equal(suffix, &value[value.len() - suffix.len()..])
                && self.should_include_min_js(value, segments);
        }
        self.match_segments(segments, value) && self.should_include_min_js(value, segments)
    }

    fn match_segments(&self, segments: &[Segment], value: &str) -> bool {
        let (mut segment_index, mut value_index) = (0, 0);
        let (mut star_segment_index, mut star_value_index) = (None, 0);
        while value_index < value.len() {
            if segment_index < segments.len() {
                let segment = &segments[segment_index];
                match segment.kind {
                    SegmentKind::Literal => {
                        let end = value_index + segment.literal.len();
                        if end <= value.len()
                            && self.strings_equal(&segment.literal, &value[value_index..end])
                        {
                            value_index = end;
                            segment_index += 1;
                            continue;
                        }
                    }
                    SegmentKind::Question => {
                        if value.as_bytes()[value_index] != b'/' {
                            let ch = value[value_index..].chars().next().unwrap();
                            value_index += ch.len_utf8();
                            segment_index += 1;
                            continue;
                        }
                    }
                    SegmentKind::Star => {
                        star_segment_index = Some(segment_index);
                        star_value_index = value_index;
                        segment_index += 1;
                        continue;
                    }
                }
            }
            if let Some(star) = star_segment_index
                && star_value_index < value.len()
                && value.as_bytes()[star_value_index] != b'/'
            {
                let ch = value[star_value_index..].chars().next().unwrap();
                star_value_index += ch.len_utf8();
                value_index = star_value_index;
                segment_index = star + 1;
                continue;
            }
            return false;
        }
        while segment_index < segments.len() && segments[segment_index].kind == SegmentKind::Star {
            segment_index += 1;
        }
        segment_index >= segments.len()
    }

    fn should_include_min_js(&self, filename: &str, segments: &[Segment]) -> bool {
        if !self.exclude_min_js || !self.has_min_js_suffix(filename) {
            return true;
        }
        self.pattern_mentions_min_suffix(segments)
    }

    fn has_min_js_suffix(&self, filename: &str) -> bool {
        if self.case_sensitive {
            filename.ends_with(".min.js")
        } else {
            filename
                .get(filename.len().saturating_sub(".min.js".len())..)
                .is_some_and(|suffix| suffix.eq_ignore_ascii_case(".min.js"))
        }
    }

    fn pattern_mentions_min_suffix(&self, segments: &[Segment]) -> bool {
        segments.iter().any(|segment| {
            if segment.kind != SegmentKind::Literal {
                return false;
            }
            let literal = if self.case_sensitive {
                segment.literal.clone()
            } else {
                segment.literal.to_ascii_lowercase()
            };
            literal.contains(".min.js") || literal.contains(".min.")
        })
    }

    fn strings_equal(&self, a: &str, b: &str) -> bool {
        if self.case_sensitive {
            a == b
        } else {
            a.eq_ignore_ascii_case(b)
        }
    }
}

fn next_path_part_parts(prefix: &str, suffix: &str, offset: usize) -> Option<(String, usize)> {
    let combined = if suffix.is_empty() {
        prefix.to_owned()
    } else {
        format!("{prefix}{suffix}")
    };
    next_path_part_single(&combined, offset)
}

fn next_path_part_single(value: &str, mut offset: usize) -> Option<(String, usize)> {
    if offset >= value.len() {
        return None;
    }
    if offset == 0 && value.as_bytes().first() == Some(&b'/') {
        return Some((String::new(), 1));
    }
    while offset < value.len() && value.as_bytes()[offset] == b'/' {
        offset += 1;
    }
    if offset >= value.len() {
        return None;
    }
    let rest = &value[offset..];
    if let Some(index) = rest.find('/') {
        Some((rest[..index].to_owned(), offset + index))
    } else {
        Some((rest.to_owned(), value.len()))
    }
}

pub fn is_hidden_path(name: &str) -> bool {
    !name.is_empty() && name.starts_with('.')
}

pub fn is_package_folder(name: &str) -> bool {
    name.eq_ignore_ascii_case("node_modules")
        || name.eq_ignore_ascii_case("bower_components")
        || name.eq_ignore_ascii_case("jspm_packages")
}

pub fn ensure_trailing_slash(value: &str) -> String {
    if !value.is_empty() && !value.ends_with('/') {
        format!("{value}/")
    } else {
        value.to_owned()
    }
}

#[derive(Clone, Debug)]
struct GlobMatcher {
    includes: Vec<GlobPattern>,
    excludes: Vec<GlobPattern>,
    had_includes: bool,
}

fn new_glob_matcher(
    include_specs: &[String],
    exclude_specs: &[String],
    base_path: &str,
    case_sensitive: bool,
    usage: Usage,
) -> GlobMatcher {
    GlobMatcher {
        had_includes: !include_specs.is_empty(),
        includes: include_specs
            .iter()
            .filter_map(|spec| compile_glob_pattern(spec, base_path, usage, case_sensitive))
            .collect(),
        excludes: exclude_specs
            .iter()
            .filter_map(|spec| {
                compile_glob_pattern(spec, base_path, Usage::Exclude, case_sensitive)
            })
            .collect(),
    }
}

impl GlobMatcher {
    fn matches_file_parts(&self, prefix: &str, suffix: &str) -> Option<usize> {
        if self
            .excludes
            .iter()
            .any(|pattern| pattern.matches_parts(prefix, suffix))
        {
            return None;
        }
        if self.includes.is_empty() {
            return (!self.had_includes).then_some(0);
        }
        self.includes
            .iter()
            .position(|pattern| pattern.matches_parts(prefix, suffix))
    }

    fn matches_directory_parts(&self, prefix: &str, suffix: &str) -> bool {
        if self
            .excludes
            .iter()
            .any(|pattern| pattern.matches_parts(prefix, suffix))
        {
            return false;
        }
        if self.includes.is_empty() {
            return !self.had_includes;
        }
        self.includes
            .iter()
            .any(|pattern| pattern.matches_prefix_parts(prefix, suffix))
    }
}

struct GlobVisitor<'a> {
    host: &'a dyn Fs,
    file_matcher: GlobMatcher,
    directory_matcher: GlobMatcher,
    extensions: &'a [String],
    use_case_sensitive_file_names: bool,
    visited: HashSet<String>,
    results: Vec<Vec<String>>,
}

impl GlobVisitor<'_> {
    fn visit(&mut self, path: &str, absolute_path: &str, depth: i32, resolved_realpath: &str) {
        let realpath = if resolved_realpath.is_empty() {
            self.host.realpath(absolute_path)
        } else {
            resolved_realpath.to_owned()
        };
        let canonical = canonical_file_name(&realpath, self.use_case_sensitive_file_names);
        if !self.visited.insert(canonical) {
            return;
        }
        let entries = self.host.get_accessible_entries(absolute_path);
        let path_prefix = ensure_trailing_slash(path);
        let absolute_prefix = ensure_trailing_slash(absolute_path);
        for file in entries.files {
            if !self.extensions.is_empty()
                && !self
                    .extensions
                    .iter()
                    .any(|extension| file.ends_with(extension))
            {
                continue;
            }
            if let Some(index) = self
                .file_matcher
                .matches_file_parts(&absolute_prefix, &file)
            {
                self.results[index].push(format!("{path_prefix}{file}"));
            }
        }
        let next_depth = if depth == UNLIMITED_DEPTH {
            depth
        } else {
            depth - 1
        };
        if depth != UNLIMITED_DEPTH && next_depth == 0 {
            return;
        }
        for dir in entries.directories {
            if !self
                .directory_matcher
                .matches_directory_parts(&absolute_prefix, &dir)
            {
                continue;
            }
            let absolute_dir = format!("{absolute_prefix}{dir}");
            let child_realpath = if entries
                .symlinks
                .as_ref()
                .is_some_and(|symlinks| !symlinks.contains(&dir))
            {
                combine_paths(&realpath, &dir)
            } else {
                String::new()
            };
            self.visit(
                &format!("{path_prefix}{dir}"),
                &absolute_dir,
                next_depth,
                &child_realpath,
            );
        }
    }
}

pub struct MatchFilesOptions<'a> {
    pub path: &'a str,
    pub extensions: &'a [String],
    pub excludes: &'a [String],
    pub includes: &'a [String],
    pub use_case_sensitive_file_names: bool,
    pub current_directory: &'a str,
    pub depth: i32,
    pub host: &'a dyn Fs,
}

pub fn match_files(options: MatchFilesOptions<'_>) -> Vec<String> {
    let MatchFilesOptions {
        path,
        extensions,
        excludes,
        includes,
        use_case_sensitive_file_names,
        current_directory,
        depth,
        host,
    } = options;

    let path = normalize_path(path);
    let current_directory = normalize_path(current_directory);
    let absolute_path = combine_paths(&current_directory, &path);
    let file_matcher = new_glob_matcher(
        includes,
        excludes,
        &absolute_path,
        use_case_sensitive_file_names,
        Usage::Files,
    );
    let directory_matcher = new_glob_matcher(
        includes,
        excludes,
        &absolute_path,
        use_case_sensitive_file_names,
        Usage::Directories,
    );
    let result_count = file_matcher.includes.len().max(1);
    let mut visitor = GlobVisitor {
        host,
        file_matcher,
        directory_matcher,
        extensions,
        use_case_sensitive_file_names,
        visited: HashSet::new(),
        results: vec![Vec::new(); result_count],
    };
    for base_path in get_base_paths(&path, includes, use_case_sensitive_file_names) {
        let absolute = combine_paths(&current_directory, &base_path);
        visitor.visit(&base_path, &absolute, depth, "");
    }
    if visitor.results.len() == 1 {
        visitor.results.pop().unwrap_or_default()
    } else {
        visitor.results.into_iter().flatten().collect()
    }
}

pub struct SpecMatcher {
    patterns: Vec<GlobPattern>,
}

impl SpecMatcher {
    pub fn match_string(&self, path: &str) -> bool {
        self.patterns.iter().any(|pattern| pattern.matches(path))
    }

    pub fn match_index(&self, path: &str) -> Option<usize> {
        self.patterns
            .iter()
            .position(|pattern| pattern.matches(path))
    }
}

pub fn new_spec_matcher(
    specs: &[String],
    base_path: &str,
    usage: Usage,
    use_case_sensitive_file_names: bool,
) -> Option<SpecMatcher> {
    if specs.is_empty() {
        return None;
    }
    let patterns = specs
        .iter()
        .filter_map(|spec| {
            compile_glob_pattern(spec, base_path, usage, use_case_sensitive_file_names)
        })
        .collect::<Vec<_>>();
    (!patterns.is_empty()).then_some(SpecMatcher { patterns })
}

fn has_extension(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|name| name.contains('.'))
}

fn get_directory_path(path: &str) -> String {
    path.rsplit_once('/')
        .map(|(dir, _)| if dir.is_empty() { "/" } else { dir })
        .unwrap_or("")
        .to_owned()
}

fn remove_trailing_directory_separator(path: &str) -> String {
    if path.len() > 1 {
        path.trim_end_matches('/').to_owned()
    } else {
        path.to_owned()
    }
}

fn is_rooted_disk_path(path: &str) -> bool {
    path.starts_with('/')
        || (path.len() >= 3 && path.as_bytes()[1] == b':' && path.as_bytes()[2] == b'/')
}

fn normalize_path(path: &str) -> String {
    let mut result = Vec::new();
    let rooted = path.starts_with('/');
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                result.pop();
            }
            _ => result.push(part),
        }
    }
    let joined = result.join("/");
    if rooted {
        format!("/{joined}")
    } else if joined.is_empty() {
        ".".to_owned()
    } else {
        joined
    }
}

fn combine_paths(left: &str, right: &str) -> String {
    if right.is_empty() {
        return normalize_path(left);
    }
    if is_rooted_disk_path(right) {
        return normalize_path(right);
    }
    normalize_path(&format!("{}/{}", left.trim_end_matches('/'), right))
}

fn get_normalized_path_components(spec: &str, base_path: &str) -> Vec<String> {
    let path = combine_paths(base_path, spec);
    if path == "/" {
        return vec![String::new()];
    }
    let mut parts = Vec::new();
    if path.starts_with('/') {
        parts.push(String::new());
    }
    parts.extend(
        path.split('/')
            .filter(|part| !part.is_empty())
            .map(str::to_owned),
    );
    parts
}

fn contains_path(base: &str, child: &str, case_sensitive: bool) -> bool {
    let base = ensure_trailing_slash(&canonical_file_name(base, case_sensitive));
    let child = canonical_file_name(child, case_sensitive);
    child == base.trim_end_matches('/') || child.starts_with(&base)
}

fn compare_paths(a: &str, b: &str, case_sensitive: bool) -> std::cmp::Ordering {
    canonical_file_name(a, case_sensitive).cmp(&canonical_file_name(b, case_sensitive))
}

fn canonical_file_name(path: &str, case_sensitive: bool) -> String {
    if case_sensitive {
        path.to_owned()
    } else {
        path.to_ascii_lowercase()
    }
}
