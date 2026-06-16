#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_string_literal_types1() {
    let mut t = TestingT;
    run_test_rename_string_literal_types1(&mut t);
}

fn run_test_rename_string_literal_types1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface AnimationOptions {
    deltaX: number;
    deltaY: number;
    easing: "ease-in" | "ease-out" | "[|ease-in-out|]";
}

function animate(o: AnimationOptions) { }

animate({ deltaX: 100, deltaY: 100, easing: "[|ease-in-out|]" });"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "ease-in-out");
    done();
}
