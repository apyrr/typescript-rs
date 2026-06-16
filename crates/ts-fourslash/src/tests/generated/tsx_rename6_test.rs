#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_rename6() {
    let mut t = TestingT;
    run_test_tsx_rename6(&mut t);
}

fn run_test_tsx_rename6(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"//@Filename: file.tsx
// @jsx: preserve
// @noLib: true
declare namespace JSX {
    interface Element { }
    interface IntrinsicElements {
    }
    interface ElementAttributesProperty { props; }
}
interface OptionPropBag {
    propx: number
    propString: string
    optional?: boolean
}
[|declare function [|{| "contextRangeIndex": 0 |}Opt|](attributes: OptionPropBag): JSX.Element;|]
let opt = [|<[|{| "contextRangeIndex": 2 |}Opt|] />|];
let opt1 = [|<[|{| "contextRangeIndex": 4 |}Opt|] propx={100} propString />|];
let opt2 = [|<[|{| "contextRangeIndex": 6 |}Opt|] propx={100} optional/>|];
let opt3 = [|<[|{| "contextRangeIndex": 8 |}Opt|] wrong />|];
let opt4 = [|<[|{| "contextRangeIndex": 10 |}Opt|] propx={100} propString="hi" />|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "Opt");
    done();
}
