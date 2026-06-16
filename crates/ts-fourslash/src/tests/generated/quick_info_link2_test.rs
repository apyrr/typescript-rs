#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_link2() {
    let mut t = TestingT;
    run_test_quick_info_link2(&mut t);
}

fn run_test_quick_info_link2(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoLink2") {
        return;
    }
    let content = r"// @checkJs: true
// @Filename: quickInfoLink2.js
/**
 * @typedef AdditionalWallabyConfig/**/ Additional valid Wallaby config properties
 * that aren't defined in {@link IWallabyConfig}.
 * @property {boolean} autoDetect
 */";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_hover(t, &[]);
    done();
}
