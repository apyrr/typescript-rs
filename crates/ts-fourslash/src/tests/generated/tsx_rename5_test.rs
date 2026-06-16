#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_rename5() {
    let mut t = TestingT;
    run_test_tsx_rename5(&mut t);
}

fn run_test_tsx_rename5(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"//@Filename: file.tsx
declare namespace JSX {
    interface Element { }
    interface IntrinsicElements {
    }
    interface ElementAttributesProperty { props }
}
class MyClass {
  props: {
    name?: string;
    size?: number;
}

[|var [|{| "contextRangeIndex": 0 |}nn|]: string;|]
var x = <MyClass name={[|nn|]}></MyClass>;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "nn");
    done();
}
