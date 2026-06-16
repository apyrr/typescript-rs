#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_this_binding_in_lambda() {
    let mut t = TestingT;
    run_test_this_binding_in_lambda(&mut t);
}

fn run_test_this_binding_in_lambda(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class Greeter {
    constructor() { 
		[].forEach((anything)=>{
			console.log(th/**/is);
		});
	}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "this: this", "");
    done();
}
