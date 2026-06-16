#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_incremental_parsing_top_level_await2() {
    let mut t = TestingT;
    run_test_incremental_parsing_top_level_await2(&mut t);
}

fn run_test_incremental_parsing_top_level_await2(t: &mut TestingT) {
    if should_skip_if_failing("TestIncrementalParsingTopLevelAwait2") {
        return;
    }
    let content = r"// @target: esnext
// @module: esnext
// @Filename: ./foo.ts
export {};
/*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_number_of_errors_in_current_file(0);
    f.go_to_marker(t, "1");
    f.insert(t, "await(1);");
    f.verify_number_of_errors_in_current_file(0);
    f.replace_line(t, 1, "");
    f.verify_number_of_errors_in_current_file(0);
    done();
}
