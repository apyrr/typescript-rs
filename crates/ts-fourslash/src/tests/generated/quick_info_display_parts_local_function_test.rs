#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_display_parts_local_function() {
    let mut t = TestingT;
    run_test_quick_info_display_parts_local_function(&mut t);
}

fn run_test_quick_info_display_parts_local_function(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoDisplayPartsLocalFunction") {
        return;
    }
    let content = r#"function /*1*/outerFoo() {
    function /*2*/foo(param: string, optionalParam?: string, paramWithInitializer = "hello", ...restParam: string[]) {
    }
    function /*3*/foowithoverload(a: string): string;
    function /*4*/foowithoverload(a: number): number;
    function /*5*/foowithoverload(a: any): any {
        return a;
    }
    function /*6*/foowith3overload(a: string): string;
    function /*7*/foowith3overload(a: number): number;
    function /*8*/foowith3overload(a: boolean): boolean;
    function /*9*/foowith3overload(a: any): any {
        return a;
    }
    /*10*/foo("hello");
    /*11*/foowithoverload("hello");
    /*12*/foowithoverload(10);
    /*13*/foowith3overload("hello");
    /*14*/foowith3overload(10);
    /*15*/foowith3overload(true);
}
/*16*/outerFoo();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
