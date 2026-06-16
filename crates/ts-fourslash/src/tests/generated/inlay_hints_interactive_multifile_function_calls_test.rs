#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_interactive_multifile_function_calls() {
    let mut t = TestingT;
    run_test_inlay_hints_interactive_multifile_function_calls(&mut t);
}

fn run_test_inlay_hints_interactive_multifile_function_calls(t: &mut TestingT) {
    if should_skip_if_failing("TestInlayHintsInteractiveMultifileFunctionCalls") {
        return;
    }
    let content = r#"// @Target: esnext
// @module: node18
// @Filename: aaa.mts
import { helperB } from "./bbb.mjs";
helperB("hello, world!");
// @Filename: bbb.mts
import { helperC } from "./ccc.mjs";
export function helperB(bParam: string) {
    helperC(bParam);
}
// @Filename: ccc.mts
export function helperC(cParam: string) {}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "./aaa.mts");
    f.verify_baseline_inlay_hints(t);
    done();
}
