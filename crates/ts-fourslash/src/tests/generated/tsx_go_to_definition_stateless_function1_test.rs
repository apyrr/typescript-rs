#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_go_to_definition_stateless_function1() {
    let mut t = TestingT;
    run_test_tsx_go_to_definition_stateless_function1(&mut t);
}

fn run_test_tsx_go_to_definition_stateless_function1(t: &mut TestingT) {
    if should_skip_if_failing("TestTsxGoToDefinitionStatelessFunction1") {
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
    /*pt1*/propx: number
    propString: "hell"
    /*pt2*/optional?: boolean
}
declare function /*opt*/Opt(attributes: OptionPropBag): JSX.Element;
let opt = <[|O/*one*/pt|] />;
let opt1 = <[|Op/*two*/t|] [|pr/*p1*/opx|]={100} />;
let opt2 = <[|Op/*three*/t|] propx={100} [|opt/*p2*/ional|] />;
let opt3 = <[|Op/*four*/t|] wr/*p3*/ong />;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "one".to_string(),
            "two".to_string(),
            "three".to_string(),
            "four".to_string(),
            "p1".to_string(),
            "p2".to_string(),
        ],
    );
    done();
}
