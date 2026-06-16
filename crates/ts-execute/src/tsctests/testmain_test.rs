use std::sync::Once;

use ts_core as core;
use ts_testutil::baseline;

static TEST_INIT: Once = Once::new();

pub(crate) fn test_main() {
    TEST_INIT.call_once(|| {
        core::apply_debug_stack_limit();
        baseline::track_process();
    });
}
