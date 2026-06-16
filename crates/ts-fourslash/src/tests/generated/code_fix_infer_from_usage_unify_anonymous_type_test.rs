#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_infer_from_usage_unify_anonymous_type() {
    let mut t = TestingT;
    run_test_code_fix_infer_from_usage_unify_anonymous_type(&mut t);
}

fn run_test_code_fix_infer_from_usage_unify_anonymous_type(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @stableTypeOrdering: true
// @strict: true
function kw([|name, options |]) {
  if ( options === void 0 ) options = {};

  options.keyword = name;
  return keywords$1[name] = new TokenType(name, options)
}
kw("1")
kw("2", { startsExpr: true })
kw("3", { beforeExpr: false })
kw("4", { isLoop: false })
kw("5", { beforeExpr: true, startsExpr: true })
kw("6", { beforeExpr: true, prefix: true, startsExpr: true })"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "name: string, options: { beforeExpr?: boolean; isLoop?: boolean; keyword?: any; prefix?: boolean; startsExpr?: boolean; } | undefined", false, 0, 0);
    done();
}
