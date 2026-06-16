#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_go_to_definition_classes() {
    let mut t = TestingT;
    run_test_tsx_go_to_definition_classes(&mut t);
}

fn run_test_tsx_go_to_definition_classes(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"//@Filename: file.tsx
declare namespace JSX {
    interface Element { }
    interface IntrinsicElements { }
    interface ElementAttributesProperty { props; }
}
class /*ct*/MyClass {
    props: {
        /*pt*/foo: string;
    }
}
var x = <[|My/*c*/Class|] />;
var y = <MyClass [|f/*p*/oo|]= 'hello' />;
var z = <[|MyCl/*w*/ass|] wrong= 'hello' />;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["c".to_string(), "p".to_string(), "w".to_string()]);
    done();
}
