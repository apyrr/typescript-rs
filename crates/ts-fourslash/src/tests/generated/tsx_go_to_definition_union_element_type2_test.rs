#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_go_to_definition_union_element_type2() {
    let mut t = TestingT;
    run_test_tsx_go_to_definition_union_element_type2(&mut t);
}

fn run_test_tsx_go_to_definition_union_element_type2(t: &mut TestingT) {
    if should_skip_if_failing("TestTsxGoToDefinitionUnionElementType2") {
        return;
    }
    let content = r"//@Filename: file.tsx
// @jsx: preserve
// @noLib: true
class RC1 extends React.Component<{}, {}> {
    render() {
        return null;
    }
}
class RC2 extends React.Component<{}, {}> {
    render() {
        return null;
    }
    private method() { }
}
var /*pt1*/RCComp = RC1 || RC2;
<[|RC/*one*/Comp|] />";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["one".to_string()]);
    done();
}
