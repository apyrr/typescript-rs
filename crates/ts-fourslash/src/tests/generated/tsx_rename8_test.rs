#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_rename8() {
    let mut t = TestingT;
    run_test_tsx_rename8(&mut t);
}

fn run_test_tsx_rename8(t: &mut TestingT) {
    if should_skip_if_failing("TestTsxRename8") {
        return;
    }
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
declare function Opt(attributes: OptionPropBag): JSX.Element;
let opt = <Opt />;
let opt1 = <Opt propx={100} propString />;
let opt2 = <Opt propx={100} optional/>;
let opt3 = <Opt [|wrong|] />;
let opt4 = <Opt propx={100} propString="hi" />;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename(t, &[]);
    done();
}
