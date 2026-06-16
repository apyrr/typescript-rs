#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_display_parts_class_accessors() {
    let mut t = TestingT;
    run_test_quick_info_display_parts_class_accessors(&mut t);
}

fn run_test_quick_info_display_parts_class_accessors(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"class c {
    public get /*1*/publicProperty() { return ""; }
    public set /*1s*/publicProperty(x: string) { }
    private get /*2*/privateProperty() { return ""; }
    private set /*2s*/privateProperty(x: string) { }
    protected get /*21*/protectedProperty() { return ""; }
    protected set /*21s*/protectedProperty(x: string) { }
    static get /*3*/staticProperty() { return ""; }
    static set /*3s*/staticProperty(x: string) { }
    private static get  /*4*/privateStaticProperty() { return ""; }
    private static set /*4s*/privateStaticProperty(x: string) { }
    protected static get /*41*/protectedStaticProperty() { return ""; }
    protected static set /*41s*/protectedStaticProperty(x: string) { }
    method() {
        var x : string;
        x = this./*5*/publicProperty;
        x = this./*6*/privateProperty;
        x = this./*61*/protectedProperty;
        x = c./*7*/staticProperty;
        x = c./*8*/privateStaticProperty;
        x = c./*81*/protectedStaticProperty;
        this./*5s*/publicProperty = "";
        this./*6s*/privateProperty = "";
        this./*61s*/protectedProperty = "";
        c./*7s*/staticProperty = "";
        c./*8s*/privateStaticProperty = "";
        c./*81s*/protectedStaticProperty = "";
    }
}
var cInstance = new c();
var y: string;
y = /*9*/cInstance./*10*/publicProperty;
y = /*11*/c./*12*/staticProperty;
/*9s*/cInstance./*10s*/publicProperty = y;
/*11s*/c./*12s*/staticProperty = y;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
