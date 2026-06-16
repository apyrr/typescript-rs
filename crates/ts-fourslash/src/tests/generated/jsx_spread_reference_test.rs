#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsx_spread_reference() {
    let mut t = TestingT;
    run_test_jsx_spread_reference(&mut t);
}

fn run_test_jsx_spread_reference(t: &mut TestingT) {
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
}

[|var [|/*dst*/{| "contextRangeIndex": 0 |}nn|]: {name?: string; size?: number};|]
var x = <MyClass {...[|n/*src*/n|]}></MyClass>;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "nn");
    f.verify_baseline_go_to_definition(t, &["src".to_string()]);
    done();
}
