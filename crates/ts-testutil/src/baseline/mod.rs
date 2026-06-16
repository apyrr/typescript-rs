mod implementation;
mod testmain;

pub use implementation::{
    NO_CONTENT, Options, diff_text, get_baseline_diff, read_file_or_no_content, record_baseline,
    run, run_against_submodule, write_comparison,
};
pub use testmain::{do_write_recorded_baselines, track, track_process, write_recorded_baselines};
