use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

const SUBMODULE_FOLDER: &str = "submodule";
const SUBMODULE_ACCEPTED_FOLDER: &str = "submoduleAccepted";
const SUBMODULE_TRIAGED_FOLDER: &str = "submoduleTriaged";

#[derive(Clone, Default)]
pub struct Options {
    pub subfolder: String,
    pub is_submodule: bool,
    pub is_submodule_accepted: bool,
    pub is_submodule_triaged: bool,
    pub diff_fixup_old: Option<fn(String) -> String>,
    pub diff_fixup_new: Option<fn(String) -> String>,
    pub skip_diff_with_old: bool,
}

pub const NO_CONTENT: &str = "<no content>";

pub fn run(file_name: &str, actual: &str, opts: Options) -> Result<(), String> {
    let orig_subfolder = opts.subfolder.clone();
    let subfolder = if opts.is_submodule {
        path_join([SUBMODULE_FOLDER, &opts.subfolder])
    } else {
        opts.subfolder.clone()
    };

    record_baseline(&path_join([&subfolder, file_name]));
    if opts.is_submodule {
        let old_baseline_error = write_comparison(
            actual,
            local_root().join(&subfolder).join(file_name),
            reference_root().join(&subfolder).join(file_name),
            false,
        )
        .err();

        if opts.skip_diff_with_old {
            return old_baseline_error.map_or(Ok(()), Err);
        }

        let submodule_expected =
            read_reference_baseline(&submodule_reference_root().join(file_name))
                .content
                .clone();
        let diff_file_name = format!("{file_name}.diff");
        let diff = get_baseline_diff(
            actual,
            submodule_expected.as_ref(),
            file_name,
            opts.diff_fixup_old,
            opts.diff_fixup_new,
        );

        let diff_key = path_join([&orig_subfolder, &diff_file_name]);
        let is_submodule_accepted =
            opts.is_submodule_accepted || submodule_accepted_file_names().contains(&diff_key);
        let is_submodule_triaged =
            opts.is_submodule_triaged || submodule_triaged_file_names().contains(&diff_key);
        if is_submodule_accepted && is_submodule_triaged {
            return Err(format!(
                "diff file {}/{} is in both submoduleAccepted and submoduleTriaged; it should only be in one",
                orig_subfolder, diff_file_name
            ));
        }

        let out_root = if is_submodule_accepted {
            SUBMODULE_ACCEPTED_FOLDER
        } else if is_submodule_triaged {
            SUBMODULE_TRIAGED_FOLDER
        } else {
            SUBMODULE_FOLDER
        };

        let mut first_error = if is_submodule_accepted || is_submodule_triaged {
            None
        } else {
            old_baseline_error
        };
        for root in [
            SUBMODULE_FOLDER,
            SUBMODULE_ACCEPTED_FOLDER,
            SUBMODULE_TRIAGED_FOLDER,
        ] {
            let actual_diff = if root == out_root { &diff } else { NO_CONTENT };
            record_baseline(&path_join([root, &orig_subfolder, &diff_file_name]));
            write_comparison(
                actual_diff,
                local_root()
                    .join(root)
                    .join(&orig_subfolder)
                    .join(&diff_file_name),
                reference_root()
                    .join(root)
                    .join(&orig_subfolder)
                    .join(&diff_file_name),
                false,
            )
            .unwrap_or_else(|err| {
                if first_error.is_none() {
                    first_error = Some(err);
                }
            });
        }
        return first_error.map_or(Ok(()), Err);
    }

    write_comparison(
        actual,
        local_root().join(&subfolder).join(file_name),
        reference_root().join(&subfolder).join(file_name),
        false,
    )?;

    Ok(())
}

fn remove_local_baseline_if_exists(local: &Path) -> Result<(), String> {
    match fs::remove_file(local) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!(
            "failed to remove the local baseline file {}: {err}",
            local.display()
        )),
    }
}

pub fn read_file_or_no_content(file_name: impl AsRef<Path>) -> String {
    fs::read(file_name)
        .map(|content| String::from_utf8_lossy(&content).into_owned())
        .unwrap_or_else(|_| NO_CONTENT.to_string())
}

fn submodule_accepted_file_names() -> &'static HashSet<String> {
    static SET: OnceLock<HashSet<String>> = OnceLock::new();
    SET.get_or_init(|| {
        read_file_name_set(ts_repo::test_data_path().join("submoduleAccepted.txt"))
    })
}

fn submodule_triaged_file_names() -> &'static HashSet<String> {
    static SET: OnceLock<HashSet<String>> = OnceLock::new();
    SET.get_or_init(|| read_file_name_set(ts_repo::test_data_path().join("submoduleTriaged.txt")))
}

fn read_file_name_set(path: impl AsRef<Path>) -> HashSet<String> {
    let path = path.as_ref();
    let content = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read file {}: {err}", path.display()));
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToOwned::to_owned)
        .collect()
}

pub fn diff_text(old_name: &str, new_name: &str, expected: &str, actual: &str) -> String {
    let mut out = format!("--- {old_name}\n+++ {new_name}\n");
    let expected_lines = split_lines(expected);
    let actual_lines = split_lines(actual);
    if expected_lines == actual_lines {
        return out;
    }
    out.push_str(&unified_diff_hunks(&expected_lines, &actual_lines, 3));
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiffTag {
    Equal,
    Delete,
    Insert,
}

#[derive(Clone, Debug)]
struct DiffOp<'a> {
    tag: DiffTag,
    line: &'a str,
}

fn unified_diff_hunks<'a>(
    expected: &'a [&'a str],
    actual: &'a [&'a str],
    context: usize,
) -> String {
    let ops = diff_lines(expected, actual);
    let mut out = String::new();
    for hunk in make_hunks(&ops, context, context) {
        append_hunk(&mut out, &hunk);
    }
    if out.ends_with('\n') {
        out.pop();
    }

    out
}

fn diff_lines<'a>(expected: &'a [&'a str], actual: &'a [&'a str]) -> Vec<DiffOp<'a>> {
    match (expected.is_empty(), actual.is_empty()) {
        (true, true) => return Vec::new(),
        (true, false) => return to_diff_ops(actual, DiffTag::Insert),
        (false, true) => return to_diff_ops(expected, DiffTag::Delete),
        (false, false) => {}
    }

    let mut head = 0;
    while head < expected.len() && head < actual.len() && expected[head] == actual[head] {
        head += 1;
    }
    if head > 0 {
        let mut ops = to_diff_ops(&expected[..head], DiffTag::Equal);
        ops.extend(diff_lines(&expected[head..], &actual[head..]));
        return ops;
    }

    let mut tail = 0;
    while tail < expected.len()
        && tail < actual.len()
        && expected[expected.len() - 1 - tail] == actual[actual.len() - 1 - tail]
    {
        tail += 1;
    }
    if tail > 0 {
        let mut ops = diff_lines(
            &expected[..expected.len() - tail],
            &actual[..actual.len() - tail],
        );
        ops.extend(to_diff_ops(
            &expected[expected.len() - tail..],
            DiffTag::Equal,
        ));
        return ops;
    }

    let (unique_expected, expected_indices) = unique_elements(expected);
    let (unique_actual, actual_indices) = unique_elements(actual);
    let lcs = lcs_pairs(&unique_expected, &unique_actual);
    if lcs.is_empty() {
        let mut ops = to_diff_ops(expected, DiffTag::Delete);
        ops.extend(to_diff_ops(actual, DiffTag::Insert));
        return ops;
    }

    let mut ops = Vec::new();
    let mut old_cursor = 0;
    let mut new_cursor = 0;
    for (unique_old_index, unique_new_index) in lcs {
        let old_index = expected_indices[unique_old_index];
        let new_index = actual_indices[unique_new_index];
        ops.extend(diff_lines(
            &expected[old_cursor..old_index],
            &actual[new_cursor..new_index],
        ));
        ops.push(DiffOp {
            tag: DiffTag::Equal,
            line: expected[old_index],
        });
        old_cursor = old_index + 1;
        new_cursor = new_index + 1;
    }
    ops.extend(diff_lines(&expected[old_cursor..], &actual[new_cursor..]));
    ops
}

fn to_diff_ops<'a>(lines: &'a [&'a str], tag: DiffTag) -> Vec<DiffOp<'a>> {
    lines
        .iter()
        .map(|line| DiffOp { tag, line: *line })
        .collect()
}

fn unique_elements<'a>(lines: &'a [&'a str]) -> (Vec<&'a str>, Vec<usize>) {
    let mut counts = HashMap::new();
    for line in lines {
        *counts.entry(*line).or_insert(0usize) += 1;
    }

    let mut elements = Vec::new();
    let mut indices = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if counts[*line] == 1 {
            elements.push(*line);
            indices.push(index);
        }
    }
    (elements, indices)
}

fn lcs_pairs(a: &[&str], b: &[&str]) -> Vec<(usize, usize)> {
    let mut lcs = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            if a[i - 1] == b[j - 1] {
                lcs[i][j] = lcs[i - 1][j - 1] + 1;
            } else {
                lcs[i][j] = lcs[i - 1][j].max(lcs[i][j - 1]);
            }
        }
    }

    let mut i = a.len();
    let mut j = b.len();
    let mut result = Vec::with_capacity(lcs[i][j]);
    while i > 0 && j > 0 {
        if a[i - 1] == b[j - 1] {
            result.push((i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if lcs[i - 1][j] > lcs[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    result.reverse();
    result
}

#[derive(Default)]
struct Hunk<'a> {
    ops: Vec<DiffOp<'a>>,
    src_start: usize,
    src_lines: usize,
    dst_start: usize,
    dst_lines: usize,
}

fn make_hunks<'a>(ops: &[DiffOp<'a>], precontext: usize, postcontext: usize) -> Vec<Hunk<'a>> {
    if ops.is_empty() {
        return Vec::new();
    }

    let mut hunks: Vec<Hunk<'a>> = Vec::new();
    let mut modified_lines = 0usize;
    let mut src_line_num = 0usize;
    let mut dst_line_num = 0usize;
    let mut block = Hunk::default();

    for op in ops {
        if block.ops.is_empty()
            || block.ops[0].tag == op.tag
            || (block.ops[0].tag != op.tag
                && block.ops[0].tag != DiffTag::Equal
                && op.tag != DiffTag::Equal)
        {
            block.ops.push(op.clone());
        } else {
            update_hunks(&mut hunks, block, precontext, postcontext, false);
            block = Hunk {
                ops: vec![op.clone()],
                ..Hunk::default()
            };
        }

        match op.tag {
            DiffTag::Delete => {
                src_line_num += 1;
                block.src_lines += 1;
                modified_lines += 1;
            }
            DiffTag::Insert => {
                dst_line_num += 1;
                block.dst_lines += 1;
                modified_lines += 1;
            }
            DiffTag::Equal => {
                src_line_num += 1;
                dst_line_num += 1;
                block.src_lines += 1;
                block.dst_lines += 1;
            }
        }

        if block.src_start == 0 && (op.tag == DiffTag::Equal || op.tag == DiffTag::Delete) {
            block.src_start = src_line_num;
        }
        if block.dst_start == 0 && (op.tag == DiffTag::Equal || op.tag == DiffTag::Insert) {
            block.dst_start = dst_line_num;
        }
    }
    update_hunks(&mut hunks, block, precontext, postcontext, true);

    if modified_lines == 0 {
        Vec::new()
    } else {
        hunks
    }
}

fn update_hunks<'a>(
    hunks: &mut Vec<Hunk<'a>>,
    block: Hunk<'a>,
    precontext: usize,
    postcontext: usize,
    last_block: bool,
) {
    if block.ops[0].tag == DiffTag::Equal {
        if hunks.is_empty() {
            let ctx_len = precontext.min(block.ops.len());
            hunks.push(Hunk {
                ops: block.ops[block.ops.len() - ctx_len..].to_vec(),
                src_start: block.ops.len() - ctx_len + block.src_start,
                src_lines: ctx_len,
                dst_start: block.ops.len() - ctx_len + block.dst_start,
                dst_lines: ctx_len,
            });
            return;
        }

        let current = hunks.len() - 1;
        let max_non_context = if last_block {
            postcontext
        } else {
            precontext + postcontext
        };
        if block.ops.len() <= max_non_context {
            hunks[current].ops.extend(block.ops);
            hunks[current].src_lines += block.src_lines;
            hunks[current].dst_lines += block.dst_lines;
        } else {
            hunks[current]
                .ops
                .extend(block.ops[..postcontext].iter().cloned());
            hunks[current].src_lines += postcontext;
            hunks[current].dst_lines += postcontext;
            if !last_block {
                hunks.push(Hunk {
                    ops: block.ops[block.ops.len() - precontext..].to_vec(),
                    src_start: block.ops.len() - precontext + block.src_start,
                    src_lines: precontext,
                    dst_start: block.ops.len() - precontext + block.dst_start,
                    dst_lines: precontext,
                });
            }
        }
        if hunks[current].src_start == 0 {
            hunks[current].src_start = block.src_start;
        }
        if hunks[current].dst_start == 0 {
            hunks[current].dst_start = block.dst_start;
        }
    } else if let Some(current) = hunks.last_mut() {
        current.ops.extend(block.ops);
        current.src_lines += block.src_lines;
        current.dst_lines += block.dst_lines;
    } else {
        hunks.push(block);
    }
}

fn append_hunk(out: &mut String, hunk: &Hunk<'_>) {
    out.push_str(&format!(
        "@@ -{},{} +{},{} @@\n",
        hunk.src_start, hunk.src_lines, hunk.dst_start, hunk.dst_lines
    ));
    for op in &hunk.ops {
        match op.tag {
            DiffTag::Equal if op.line.is_empty() => {}
            DiffTag::Equal => out.push(' '),
            DiffTag::Delete => out.push('-'),
            DiffTag::Insert => out.push('+'),
        }
        out.push_str(op.line);
        out.push('\n');
    }
}

fn split_lines(text: &str) -> Vec<&str> {
    let mut lines = Vec::with_capacity(text.bytes().filter(|&byte| byte == b'\n').count() + 1);
    let mut start = 0;
    let mut pos = 0;
    let bytes = text.as_bytes();
    while pos < bytes.len() {
        match bytes[pos] {
            b'\r' => {
                lines.push(&text[start..pos]);
                pos += if pos + 1 < bytes.len() && bytes[pos + 1] == b'\n' {
                    2
                } else {
                    1
                };
                start = pos;
            }
            b'\n' => {
                lines.push(&text[start..pos]);
                pos += 1;
                start = pos;
            }
            _ => pos += 1,
        }
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

pub fn get_baseline_diff(
    actual: &str,
    expected: &str,
    file_name: &str,
    fixup_old: Option<fn(String) -> String>,
    fixup_new: Option<fn(String) -> String>,
) -> String {
    let expected = fixup_old
        .map(|fixup| Cow::Owned(fixup(expected.to_string())))
        .unwrap_or(Cow::Borrowed(expected));
    let actual = fixup_new
        .map(|fixup| Cow::Owned(fixup(actual.to_string())))
        .unwrap_or(Cow::Borrowed(actual));
    if actual == expected {
        return NO_CONTENT.to_string();
    }
    let diff = diff_text(
        &format!("old.{file_name}"),
        &format!("new.{file_name}"),
        &expected,
        &actual,
    );
    if diff.contains("@@") {
        normalize_unified_diff_headers(&diff)
    } else {
        NO_CONTENT.to_string()
    }
}

fn normalize_unified_diff_headers(diff: &str) -> String {
    let mut out = String::with_capacity(diff.len());
    let mut old_current_line = 1isize;
    let mut new_current_line = 1isize;

    for line in diff.split_inclusive('\n') {
        let trimmed = line.strip_suffix('\n').unwrap_or(line);
        if let Some((prefix, old_line, new_line, suffix)) = parse_unified_diff_header(trimmed) {
            out.push_str(prefix);
            out.push_str(&format!(
                "@@= skipped -{}, +{} lines =@@",
                old_line - old_current_line,
                new_line - new_current_line
            ));
            out.push_str(suffix);
            if line.ends_with('\n') {
                out.push('\n');
            }
            old_current_line = old_line;
            new_current_line = new_line;
        } else {
            out.push_str(line);
        }
    }

    out
}

fn parse_unified_diff_header(line: &str) -> Option<(&str, isize, isize, &str)> {
    let start = line.find("@@ -")?;
    let end = line[start..].find(" @@")? + start + " @@".len();
    let header = &line[start..end];
    let rest = header.strip_prefix("@@ -")?;
    let (old_start, rest) = rest.split_once(',')?;
    let (_, rest) = rest.split_once(" +")?;
    let (new_start, rest) = rest.split_once(',')?;
    if !rest.ends_with(" @@") {
        return None;
    }
    Some((
        &line[..start],
        old_start.parse().ok()?,
        new_start.parse().ok()?,
        &line[end..],
    ))
}

pub fn run_against_submodule(file_name: &str, actual: &str, opts: Options) -> Result<(), String> {
    record_baseline(&path_join([&opts.subfolder, file_name]));
    write_comparison(
        actual,
        local_root().join(&opts.subfolder).join(file_name),
        submodule_reference_root()
            .join(&opts.subfolder)
            .join(file_name),
        true,
    )
}

#[derive(Clone)]
struct ReferenceBaseline {
    content: Arc<str>,
    found: bool,
}

fn read_reference_baseline(reference: &Path) -> ReferenceBaseline {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, ReferenceBaseline>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    {
        let cache = cache
            .lock()
            .expect("reference baseline cache mutex poisoned");
        if let Some(baseline) = cache.get(reference) {
            return baseline.clone();
        }
    }

    let baseline = match fs::read(reference) {
        Ok(content) => ReferenceBaseline {
            content: Arc::from(String::from_utf8_lossy(&content).as_ref()),
            found: true,
        },
        Err(_) => ReferenceBaseline {
            content: Arc::from(NO_CONTENT),
            found: false,
        },
    };
    cache
        .lock()
        .expect("reference baseline cache mutex poisoned")
        .entry(reference.to_path_buf())
        .or_insert_with(|| baseline.clone())
        .clone()
}

fn write_file_if_changed(path: &Path, content: impl AsRef<[u8]>) -> Result<(), std::io::Error> {
    let content = content.as_ref();
    if fs::read(path).is_ok_and(|existing| existing == content) {
        return Ok(());
    }
    fs::write(path, content)
}

pub fn write_comparison(
    actual_content: &str,
    local: impl AsRef<Path>,
    reference: impl AsRef<Path>,
    comparing_against_submodule: bool,
) -> Result<(), String> {
    if actual_content.is_empty() {
        panic!(
            "the generated content was \"\". Return 'baseline.NoContent' if no baselining is required."
        );
    }

    let local = local.as_ref();
    let reference = reference.as_ref();

    let expected = read_reference_baseline(reference);
    if expected.content.as_ref() == actual_content
        && !(actual_content == NO_CONTENT && expected.found)
    {
        remove_local_baseline_if_exists(local)?;
        return Ok(());
    }

    if let Some(parent) = local.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create directories for the local baseline file {}: {err}",
                local.display()
            )
        })?;
    }

    if actual_content == NO_CONTENT {
        remove_local_baseline_if_exists(local)?;
        write_file_if_changed(&local.with_extension("delete"), []).map_err(|err| {
            format!(
                "failed to write the local baseline file {}.delete: {err}",
                local.display()
            )
        })?;
    } else {
        write_file_if_changed(local, actual_content).map_err(|err| {
            format!(
                "failed to write the local baseline file {}: {err}",
                local.display()
            )
        })?;
    }

    if !expected.found {
        if comparing_against_submodule {
            return Err(format!(
                "the baseline file {} does not exist in the TypeScript submodule",
                reference.display()
            ));
        }
        return Err(format!("new baseline created at {}.", local.display()));
    }
    if comparing_against_submodule {
        return Err(format!(
            "the baseline file {} does not match the reference in the TypeScript submodule",
            reference.display()
        ));
    }
    Err(format!(
        "the baseline file {} has changed. (Run `hereby baseline-accept` if the new baseline is correct.)",
        reference.display()
    ))
}

pub fn record_baseline(path: &str) {
    super::testmain::record_baseline_tracking(path);
}

fn local_root() -> PathBuf {
    ts_repo::baseline_output_path().join("local")
}

pub(crate) fn reference_root() -> PathBuf {
    ts_repo::test_data_path()
        .join("baselines")
        .join("reference")
}

fn submodule_reference_root() -> PathBuf {
    type_script_submodule_path()
        .join("tests")
        .join("baselines")
        .join("reference")
}

fn type_script_submodule_path() -> PathBuf {
    ts_repo::type_script_submodule_path().to_path_buf()
}

fn path_join<const N: usize>(parts: [&str; N]) -> String {
    parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}
