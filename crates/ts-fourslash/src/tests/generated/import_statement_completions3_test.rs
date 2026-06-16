#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_statement_completions3() {
    let mut t = TestingT;
    run_test_import_statement_completions3(&mut t);
}

fn run_test_import_statement_completions3(t: &mut TestingT) {
    if should_skip_if_failing("TestImportStatementCompletions3") {
        return;
    }
    let content = r"// @Filename: ./$foo.ts
export function foo() {}
// @Filename: ./bar.ts
import f/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
