#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_from_contextual_union_type1() {
    let mut t = TestingT;
    run_test_quick_info_from_contextual_union_type1(&mut t);
}

fn run_test_quick_info_from_contextual_union_type1(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoFromContextualUnionType1") {
        return;
    }
    let content = r#"// @strict: true
// based on https://github.com/microsoft/TypeScript/issues/55495
type X =
  | {
      name: string;
      [key: string]: any;
    }
  | {
      name: "john";
      someProp: boolean;
    };

const obj = { name: "john", /*1*/someProp: "foo" } satisfies X;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(property) someProp: string", "");
    done();
}
