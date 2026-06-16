#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_deprecated_suggestion9() {
    let mut t = TestingT;
    run_test_jsdoc_deprecated_suggestion9(&mut t);
}

fn run_test_jsdoc_deprecated_suggestion9(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocDeprecated_suggestion9") {
        return;
    }
    let content = r"// @Filename: first.ts
export class logger { }
// @Filename: second.ts
import { logger } from './first';
new logger()";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "second.ts");
    f.verify_no_errors();
    f.verify_suggestion_diagnostics(&[]);
    done();
}
