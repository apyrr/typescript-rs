#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_display_parts_const() {
    let mut t = TestingT;
    run_test_quick_info_display_parts_const(&mut t);
}

fn run_test_quick_info_display_parts_const(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoDisplayPartsConst") {
        return;
    }
    let content = r#"const /*1*/a = 10;
function foo() {
    const /*2*/b = /*3*/a;
    if (b) {
        const /*4*/b1 = 10;
    }
}
namespace m {
    const /*5*/c = 10;
    export const /*6*/d = 10;
    if (c) {
        const /*7*/e = 10;
    }
}
const /*8*/f: () => number = () => 10;
const /*9*/g = /*10*/f;
/*11*/f();
const /*12*/h: { (a: string): number; (a: number): string; } = a => a;
const /*13*/i = /*14*/h;
/*15*/h(10);
/*16*/h("hello");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
