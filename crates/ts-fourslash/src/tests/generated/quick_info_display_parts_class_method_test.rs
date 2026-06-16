#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_display_parts_class_method() {
    let mut t = TestingT;
    run_test_quick_info_display_parts_class_method(&mut t);
}

fn run_test_quick_info_display_parts_class_method(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoDisplayPartsClassMethod") {
        return;
    }
    let content = r"class c {
    public /*1*/publicMethod() { }
    private /*2*/privateMethod() { }
    protected /*21*/protectedMethod() { }
    static /*3*/staticMethod() { }
    private static /*4*/privateStaticMethod() { }
    protected static /*41*/protectedStaticMethod() { }
    method() {
        this./*5*/publicMethod();
        this./*6*/privateMethod();
        this./*61*/protectedMethod();
        c./*7*/staticMethod();
        c./*8*/privateStaticMethod();
        c./*81*/protectedStaticMethod();
    }
}
var cInstance = new c();
/*9*/cInstance./*10*/publicMethod();
/*11*/c./*12*/staticMethod();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
