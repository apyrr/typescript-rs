#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_infer_from_usage_array() {
    let mut t = TestingT;
    run_test_code_fix_infer_from_usage_array(&mut t);
}

fn run_test_code_fix_infer_from_usage_array(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @noImplicitAny: true
function foo([|p, a, b, c, d, e |]) {
    var x: string = a.pop()
    b.reverse()
    var rr: boolean[] = c.reverse()
    d.some(t => t > 1); // can't infer from callbacks right now
    var y = e.concat(12); // can't infer from overloaded functions right now
    return p.push(12)
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(
        t,
        "p: number[], a: string[], b: any[], c: boolean[], d: any[], e: any[]",
        false,
        0,
        0,
    );
    done();
}
