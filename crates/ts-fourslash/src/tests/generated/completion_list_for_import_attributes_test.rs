#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_for_import_attributes() {
    let mut t = TestingT;
    run_test_completion_list_for_import_attributes(&mut t);
}

fn run_test_completion_list_for_import_attributes(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListForImportAttributes") {
        return;
    }
    let content = r#"// @module: esnext
// @target: esnext
// @filename: ./a.ts
export default {};
// @filename: ./b.ts
declare global {
    interface ImportAttributes {
        type: "json",
        "resolution-mode": "import"
    }
}
const str = "hello";

import * as t1 from "./a" with { /*1*/ };
import * as t2 from "./a" with { type: "/*2*/" };
import * as t3 from "./a" with { type: "json", /*3*/ };
import * as t4 from "./a" with { type: /*4*/ };"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
