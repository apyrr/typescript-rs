use crate::baseline;
use crate::harnessutil::{
    CompilerOptions, Diagnostic, EmitResult, ProgramHandle, get_source_map_record_from_parts,
};
use ts_collections::OrderedMap;

pub struct SourceMapRecordBaselineInput<'a> {
    pub baseline_path: &'a str,
    pub options: &'a CompilerOptions,
    pub diagnostics: &'a [Diagnostic],
    pub emit_result: Option<&'a EmitResult>,
    pub program: Option<&'a ProgramHandle>,
    pub js: &'a OrderedMap<String, String>,
    pub dts: &'a OrderedMap<String, String>,
    pub opts: baseline::Options,
}

pub fn do_sourcemap_record_baseline(input: SourceMapRecordBaselineInput<'_>) -> Result<(), String> {
    let SourceMapRecordBaselineInput {
        baseline_path,
        options,
        diagnostics,
        emit_result,
        program,
        js,
        dts,
        opts,
    } = input;
    let mut actual = baseline::NO_CONTENT.to_string();
    if options.source_map
        || options.inline_source_map
        || options.declaration_map && (options.declaration || options.composite)
    {
        let record = remove_test_path_prefixes(&get_source_map_record_from_parts(
            emit_result,
            program,
            js,
            dts,
        ));
        if !(options.no_emit_on_error && !diagnostics.is_empty()) && !record.is_empty() {
            actual = record;
        }
    }

    let baseline_path = if baseline_path.ends_with(".ts") || baseline_path.ends_with(".tsx") {
        format!(
            "{}.sourcemap.txt",
            baseline_path
                .rsplit_once('.')
                .map(|(base, _)| base)
                .unwrap_or(baseline_path)
        )
    } else {
        baseline_path.to_string()
    };
    baseline::run(&baseline_path, &actual, opts)
}

fn remove_test_path_prefixes(path: &str) -> String {
    path.replace("/.src/", "").replace("/.lib/", "")
}
