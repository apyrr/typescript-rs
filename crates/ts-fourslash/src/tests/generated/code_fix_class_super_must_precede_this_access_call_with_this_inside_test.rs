#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_super_must_precede_this_access_call_with_this_inside() {
    let mut t = TestingT;
    run_test_code_fix_class_super_must_precede_this_access_call_with_this_inside(&mut t);
}

fn run_test_code_fix_class_super_must_precede_this_access_call_with_this_inside(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class Base{
    constructor(id: number) { id; }
}
class C extends Base{
    constructor(private a:number) {
        super(this.a);
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &[]);
    done();
}
