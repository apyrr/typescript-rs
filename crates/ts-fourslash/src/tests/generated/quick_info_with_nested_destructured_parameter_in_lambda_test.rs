#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_with_nested_destructured_parameter_in_lambda() {
    let mut t = TestingT;
    run_test_quick_info_with_nested_destructured_parameter_in_lambda(&mut t);
}

fn run_test_quick_info_with_nested_destructured_parameter_in_lambda(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @filename: a.tsx
import * as React from 'react';
interface SomeInterface {
    someBoolean: boolean,
    someString: string;
}
interface SomeProps {
    someProp: SomeInterface;
}
export const /*1*/SomeStatelessComponent = ({someProp: { someBoolean, someString}}: SomeProps) => (<div>{` + "`" + `${someBoolean}${someString}` + "`" + `});"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_quick_info_exists(t);
    done();
}
