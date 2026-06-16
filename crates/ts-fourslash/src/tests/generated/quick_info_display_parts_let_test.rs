#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_display_parts_let() {
    let mut t = TestingT;
    run_test_quick_info_display_parts_let(&mut t);
}

fn run_test_quick_info_display_parts_let(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"let /*1*/a = 10;
function foo() {
    let /*2*/b = /*3*/a;
    if (b) {
        let /*4*/b1 = 10;
    }
}
namespace m {
    let /*5*/c = 10;
    export let /*6*/d = 10;
    if (c) {
        let /*7*/e = 10;
    }
}
let /*8*/f: () => number;
let /*9*/g = /*10*/f;
/*11*/f();
let /*12*/h: { (a: string): number; (a: number): string; };
let /*13*/i = /*14*/h;
/*15*/h(10);
/*16*/h("hello");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
