#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_quote_preference2() {
    let mut t = TestingT;
    run_test_inlay_hints_quote_preference2(&mut t);
}

fn run_test_inlay_hints_quote_preference2(t: &mut TestingT) {
    if should_skip_if_failing("TestInlayHintsQuotePreference2") {
        return;
    }
    let content = r#"const a1: "'" = "'";
const b1: "\\" = "\\";
export function fn(a = a1, b = b1) {}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints(t);
    done();
}
