#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_infer_from_usage_jsx_element() {
    let mut t = TestingT;
    run_test_code_fix_infer_from_usage_jsx_element(&mut t);
}

fn run_test_code_fix_infer_from_usage_jsx_element(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @noImplicitAny: true
// @jsx: react
// @module: es2015
// @moduleResolution: bundler
declare namespace React {
    export class Component { render(): JSX.Element | null; }
}
declare global {
    namespace JSX {
        interface Element {}
    }
}

 export default function Component([|props |]) {
     if (props.isLoading) {
         return <div>...Loading < /div>;
     }
     else {
         return <AnotherComponent
             update={
             (rec) => {
                 return props.update(rec);
             }
         }
         />;
     }
 }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(
        t,
        "props: { isLoading: any; update: (arg0: any) => any; }",
        false,
        0,
        0,
    );
    done();
}
