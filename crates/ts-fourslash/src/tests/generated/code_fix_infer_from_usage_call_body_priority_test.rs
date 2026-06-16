#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_infer_from_usage_call_body_priority() {
    let mut t = TestingT;
    run_test_code_fix_infer_from_usage_call_body_priority(&mut t);
}

fn run_test_code_fix_infer_from_usage_call_body_priority(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function isIdentifierStart([|code, astral |]) {
  if (code < 65) { return code === 36 }
  if (code < 91) { return true }
  if (code < 97) { return code === 95 }
  if (code < 123) { return true }
  if (code <= 0xffff) { return code >= 0xaa }
  if (astral === false) { return false }
}

function isLet(nextCh: any) {
    return isIdentifierStart(nextCh, true)
};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "code: number, astral: boolean", false, 0, 0);
    done();
}
