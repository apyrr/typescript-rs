#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_const_assertions() {
    let mut t = TestingT;
    run_test_quick_info_for_const_assertions(&mut t);
}

fn run_test_quick_info_for_const_assertions(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoForConstAssertions") {
        return;
    }
    let content = r#"const a = { a: 1 } as /*1*/const;
const b = 1 as /*2*/const;
const c = "c" as /*3*/const;
const d = [1, 2] as /*4*/const;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
