#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_display_parts_literal_like_names01() {
    let mut t = TestingT;
    run_test_quick_info_display_parts_literal_like_names01(&mut t);
}

fn run_test_quick_info_display_parts_literal_like_names01(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoDisplayPartsLiteralLikeNames01") {
        return;
    }
    let content = r#"class C {
    public /*1*/1() { }
    private /*2*/Infinity() { }
    protected /*3*/NaN() { }
    static /*4*/"stringLiteralName"() { }
    method() {
        this[/*5*/1]();
        this[/*6*/"1"]();
        this./*7*/Infinity();
        this[/*8*/"Infinity"]();
        this./*9*/NaN();
        C./*10*/stringLiteralName();
    }"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
