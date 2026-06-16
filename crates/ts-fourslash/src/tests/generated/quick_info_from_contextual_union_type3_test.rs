#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_from_contextual_union_type3() {
    let mut t = TestingT;
    run_test_quick_info_from_contextual_union_type3(&mut t);
}

fn run_test_quick_info_from_contextual_union_type3(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoFromContextualUnionType3") {
        return;
    }
    let content = r#"// @strict: true
declare const foo1: <D extends Foo1<D>>(definition: D) => D;

type Foo1<D, Bar = Prop<D, "bar">> = {
  bar: {
    [K in keyof Bar]: Bar[K] extends boolean
      ? Bar[K]
      : "Error: bar should be boolean";
  };
};

declare const foo2: <D extends Foo2<D>>(definition: D) => D;

type Foo2<D, Bar = Prop<D, "bar">> = {
  bar?: {
    [K in keyof Bar]: Bar[K] extends boolean
      ? Bar[K]
      : "Error: bar should be boolean";
  };
};

type Prop<T, K> = K extends keyof T ? T[K] : never;

foo1({ bar: { /*1*/X: "test" } });

foo2({ bar: { /*2*/X: "test" } });"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(property) X: \"Error: bar should be boolean\"", "");
    f.verify_quick_info_at(t, "2", "(property) X: \"Error: bar should be boolean\"", "");
    done();
}
