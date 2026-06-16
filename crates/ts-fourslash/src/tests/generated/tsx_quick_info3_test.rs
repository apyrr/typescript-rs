#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_quick_info3() {
    let mut t = TestingT;
    run_test_tsx_quick_info3(&mut t);
}

fn run_test_tsx_quick_info3(t: &mut TestingT) {
    if should_skip_if_failing("TestTsxQuickInfo3") {
        return;
    }
    let content = r"//@Filename: file.tsx
// @jsx: preserve
// @noLib: true
interface OptionProp {
    propx: 2
}
class Opt extends React.Component<OptionProp, {}> {
    render() {
        return <div>Hello</div>;
    }
}
const obj1: OptionProp = {
    propx: 2
}
let y1 = <O/*1*/pt pro/*2*/px={2} />;
let y2 = <Opt {...ob/*3*/j1} />;
let y2 = <Opt {...obj1} pr/*4*/opx />;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "class Opt", "");
    f.verify_quick_info_at(t, "2", "(property) propx: number", "");
    f.verify_quick_info_at(t, "3", "const obj1: OptionProp", "");
    f.verify_quick_info_at(t, "4", "(property) propx: true", "");
    done();
}
