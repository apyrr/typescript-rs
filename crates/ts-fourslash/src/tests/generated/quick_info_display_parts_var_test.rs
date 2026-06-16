#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_display_parts_var() {
    let mut t = TestingT;
    run_test_quick_info_display_parts_var(&mut t);
}

fn run_test_quick_info_display_parts_var(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"var /*1*/a = 10;
function foo() {
    var /*2*/b = /*3*/a;
}
namespace m {
    var /*4*/c = 10;
    export var /*5*/d = 10;
}
var /*6*/f: () => number;
var /*7*/g = /*8*/f;
/*9*/f();
var /*10*/h: { (a: string): number; (a: number): string; };
var /*11*/i = /*12*/h;
/*13*/h(10);
/*14*/h("hello");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
