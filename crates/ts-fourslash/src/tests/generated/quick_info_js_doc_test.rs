#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_js_doc() {
    let mut t = TestingT;
    run_test_quick_info_js_doc(&mut t);
}

fn run_test_quick_info_js_doc(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJsDoc") {
        return;
    }
    let content = r#"// @target: esnext
/**
 * A constant
 * @deprecated
 */
var foo = "foo";

/**
 * A function
 * @deprecated
 */
function fn() { }

/**
 * A class
 * @deprecated
 */
class C {
    /**
     * A field
     * @deprecated
     */
    field = "field";

    /**
     * A getter
     * @deprecated
     */
    get getter() {
        return;
    }

    /**
     * A method
     * @deprecated
     */
    m() { }

    get a() {
        this.field/*0*/;
        this.getter/*1*/;
        this.m/*2*/;
        foo/*3*/;
        C/*4*//;
        fn()/*5*/;

        return 1;
    }

    set a(value: number) {
        this.field/*6*/;
        this.getter/*7*/;
        this.m/*8*/;
        foo/*9*/;
        C/*10*/;
        fn/*11*/();
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
