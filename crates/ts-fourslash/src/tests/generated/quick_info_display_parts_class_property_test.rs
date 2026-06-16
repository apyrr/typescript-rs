#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_display_parts_class_property() {
    let mut t = TestingT;
    run_test_quick_info_display_parts_class_property(&mut t);
}

fn run_test_quick_info_display_parts_class_property(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoDisplayPartsClassProperty") {
        return;
    }
    let content = r"class c {
    public /*1*/publicProperty: string;
    private /*2*/privateProperty: string;
    protected /*21*/protectedProperty: string;
    static /*3*/staticProperty: string;
    private static /*4*/privateStaticProperty: string;
    protected static /*41*/protectedStaticProperty: string;
    method() {
        this./*5*/publicProperty;
        this./*6*/privateProperty;
        this./*61*/protectedProperty;
        c./*7*/staticProperty;
        c./*8*/privateStaticProperty;
        c./*81*/protectedStaticProperty;
    }
}
var cInstance = new c();
/*9*/cInstance./*10*/publicProperty;
/*11*/c./*12*/staticProperty;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
