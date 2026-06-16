#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_assertion_node_not_reused_when_type_not_equivalent1() {
    let mut t = TestingT;
    run_test_quick_info_assertion_node_not_reused_when_type_not_equivalent1(&mut t);
}

fn run_test_quick_info_assertion_node_not_reused_when_type_not_equivalent1(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoAssertionNodeNotReusedWhenTypeNotEquivalent1") {
        return;
    }
    let content = r#"// @strict: true
type Wrapper<T> = {
  _type: T;
};

function stringWrapper(): Wrapper<string> {
  return { _type: "" };
}

function objWrapper<T extends Record<string, Wrapper<any>>>(
  obj: T,
): Wrapper<T> {
  return { _type: obj };
}

const value = objWrapper({
  prop1: stringWrapper() as Wrapper<"hello">,
});

type Unwrap<T extends Wrapper<any>> = T["_type"] extends Record<
  string,
  Wrapper<any>
>
  ? { [Key in keyof T["_type"]]: Unwrap<T["_type"][Key]> }
  : T["_type"];

type Test/*1*/ = Unwrap<typeof value>;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "type Test = {\n    prop1: \"hello\";\n}", "");
    done();
}
