#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_link10() {
    let mut t = TestingT;
    run_test_quick_info_link10(&mut t);
}

fn run_test_quick_info_link10(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoLink10") {
        return;
    }
    let content = r"/**
 * start {@link https://vscode.dev/ | end}
 */
const /**/a = () => 1;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
