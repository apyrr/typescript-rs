#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_super_must_precede_this_access() {
    let mut t = TestingT;
    run_test_code_fix_class_super_must_precede_this_access(&mut t);
}

fn run_test_code_fix_class_super_must_precede_this_access(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixClassSuperMustPrecedeThisAccess") {
        return;
    }
    let content = r"class Base{
}
class C extends Base{
    private a:number;
    constructor() {[|
        this.a = 12;
        super();
    |]}
    m() { this.a; } // avoid unused 'a'
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(
        t,
        "\n        super();\n        this.a = 12;\n    ",
        true,
        0,
        0,
    );
    done();
}
