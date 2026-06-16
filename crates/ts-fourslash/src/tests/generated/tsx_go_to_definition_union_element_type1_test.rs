#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_go_to_definition_union_element_type1() {
    let mut t = TestingT;
    run_test_tsx_go_to_definition_union_element_type1(&mut t);
}

fn run_test_tsx_go_to_definition_union_element_type1(t: &mut TestingT) {
    if should_skip_if_failing("TestTsxGoToDefinitionUnionElementType1") {
        return;
    }
    let content = r"//@Filename: file.tsx
// @jsx: preserve
// @noLib: true
declare namespace JSX {
    interface Element { }
    interface IntrinsicElements {
    }
    interface ElementAttributesProperty { props; }
}
function /*pt1*/SFC1(prop: { x: number }) {
    return <div>hello </div>;
};
function SFC2(prop: { x: boolean }) {
    return <h1>World </h1>;
}
var /*def*/SFCComp = SFC1 || SFC2;
<[|SFC/*one*/Comp|] x />";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["one".to_string()]);
    done();
}
