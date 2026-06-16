#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_find_all_references5() {
    let mut t = TestingT;
    run_test_tsx_find_all_references5(&mut t);
}

fn run_test_tsx_find_all_references5(t: &mut TestingT) {
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
/*1*/declare function /*2*/Opt(attributes: OptionPropBag): JSX.Element;
let opt = /*3*/</*4*/Opt />;
let opt1 = /*5*/</*6*/Opt propx={100} propString />;
let opt2 = /*7*/</*8*/Opt propx={100} optional/>;
let opt3 = /*9*/</*10*/Opt wrong />;
let opt4 = /*11*/</*12*/Opt propx={100} propString="hi" />;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
            "7".to_string(),
            "8".to_string(),
            "9".to_string(),
            "10".to_string(),
            "11".to_string(),
            "12".to_string(),
        ],
    );
    done();
}
