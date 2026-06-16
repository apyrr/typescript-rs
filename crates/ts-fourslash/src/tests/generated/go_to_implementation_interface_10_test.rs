#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_interface_10() {
    let mut t = TestingT;
    run_test_go_to_implementation_interface_10(&mut t);
}

fn run_test_go_to_implementation_interface_10(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: /a.ts
interface /*def*/A {
	foo: boolean;
}
interface [|B|] extends A {
	bar: boolean;
}
export class [|C|] implements B {
	foo = true;
	bar = true;
}
export class [|D|] extends C { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["def".to_string()]);
    done();
}
