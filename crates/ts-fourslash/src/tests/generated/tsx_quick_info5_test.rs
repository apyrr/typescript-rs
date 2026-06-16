#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_quick_info5() {
    let mut t = TestingT;
    run_test_tsx_quick_info5(&mut t);
}

fn run_test_tsx_quick_info5(t: &mut TestingT) {
    if should_skip_if_failing("TestTsxQuickInfo5") {
        return;
    }
    let content = r#"//@Filename: file.tsx
// @jsx: preserve
// @noLib: true
declare function ComponentWithTwoAttributes<K,V>(l: {key1: K, value: V}): JSX.Element;
function Baz<T,U>(key1: T, value: U) {
    let a0 = <ComponentWi/*1*/thTwoAttributes k/*2*/ey1={key1} val/*3*/ue={value} />
    let a1 = <ComponentWithTwoAttributes {...{key1, value: value}} key="Component" />
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "function ComponentWithTwoAttributes<T, U>(l: {\n    key1: T;\n    value: U;\n}): JSX.Element", "");
    f.verify_quick_info_at(t, "2", "(property) key1: T", "");
    f.verify_quick_info_at(t, "3", "(property) value: U", "");
    done();
}
