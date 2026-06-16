#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_go_to_definition_intrinsics() {
    let mut t = TestingT;
    run_test_tsx_go_to_definition_intrinsics(&mut t);
}

fn run_test_tsx_go_to_definition_intrinsics(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"//@Filename: file.tsx
declare namespace JSX {
    interface Element { }
    interface IntrinsicElements {
        /*dt*/div: {
            /*pt*/name?: string;
            isOpen?: boolean;
        };
        /*st*/span: { n: string; };
    }
}
var x = <[|di/*ds*/v|] />;
var y = <[|s/*ss*/pan|] />;
var z = <div [|na/*ps*/me|]='hello' />;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["ds".to_string(), "ss".to_string(), "ps".to_string()]);
    done();
}
