#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_js_doc_getter_setter() {
    let mut t = TestingT;
    run_test_quick_info_js_doc_getter_setter(&mut t);
}

fn run_test_quick_info_js_doc_getter_setter(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJsDocGetterSetter") {
        return;
    }
    let content = r#"class A {
    /**
     * getter A
     * @returns return A
     */
    get /*1*/x(): string {
        return "";
    }
    /**
     * setter A
     * @param value foo A
     * @todo empty jsdoc
     */
    set /*2*/x(value) { }
}
// override both getter and setter
class B extends A {
    /**
     * getter B
     * @returns return B
     */
    get /*3*/x(): string {
        return "";
    }
    /**
     * setter B
     * @param value foo B
     */
    set /*4*/x(vale) { }
}
// not override
class C extends A { }
// only override setter
class D extends A {
    /**
     * setter D
     * @param value foo D
     */
    set /*5*/x(val: string) { }
}
new A()./*6*/x = "1";
new B()./*7*/x = "1";
new C()./*8*/x = "1";
new D()./*9*/x = "1";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
