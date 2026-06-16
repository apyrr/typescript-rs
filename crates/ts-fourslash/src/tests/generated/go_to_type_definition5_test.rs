#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_type_definition5() {
    let mut t = TestingT;
    run_test_go_to_type_definition5(&mut t);
}

fn run_test_go_to_type_definition5(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: foo.ts
let Foo: /*definition*/unresolved;
type Foo = { x: string };
/*reference*/Foo;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_type_definition(t, &["reference".to_string()]);
    done();
}
