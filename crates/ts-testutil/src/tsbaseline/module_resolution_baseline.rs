use crate::baseline;

use super::replace_ts_extension;

pub fn do_module_resolution_baseline(
    baseline_path: &str,
    trace: &str,
    opts: baseline::Options,
) -> Result<(), String> {
    let baseline_path = replace_ts_extension(baseline_path, ".trace.json");
    let error_baseline = if trace.is_empty() {
        baseline::NO_CONTENT
    } else {
        trace
    };
    baseline::run(&baseline_path, error_baseline, opts)
}
