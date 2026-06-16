#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_infer_from_usage_empty_type_priority() {
    let mut t = TestingT;
    run_test_code_fix_infer_from_usage_empty_type_priority(&mut t);
}

fn run_test_code_fix_infer_from_usage_empty_type_priority(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @stableTypeOrdering: true
// @strict: true
function TokenType([|label, conf |]) {
  if ( conf === void 0 ) conf = {};

  var l = label;
  var keyword = conf.keyword;
  var beforeExpr = !!conf.beforeExpr;
};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(
        t,
        "label: any, conf: { beforeExpr?: any; keyword?: any; } | undefined",
        false,
        0,
        0,
    );
    done();
}
