#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_outlining_spans_for_template_literal() {
    let mut t = TestingT;
    run_test_get_outlining_spans_for_template_literal(&mut t);
}

fn run_test_get_outlining_spans_for_template_literal(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOutliningSpansForTemplateLiteral") {
        return;
    }
    let content = r"declare function tag(...args: any[]): void
const a = [|`signal line`|]
const b = [|`multi
line`|]
const c = tag[|`signal line`|]
const d = tag[|`multi
line`|]
const e = [|`signal ${1} line`|]
const f = [|`multi
${1}
line`|]
const g = tag[|`signal ${1} line`|]
const h = tag[|`multi
${1}
line`|]
const i = ``";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_outlining_spans_from_ranges(t);
    done();
}
