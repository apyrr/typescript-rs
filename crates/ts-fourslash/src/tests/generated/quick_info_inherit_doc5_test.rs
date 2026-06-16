#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_inherit_doc5() {
    let mut t = TestingT;
    run_test_quick_info_inherit_doc5(&mut t);
}

fn run_test_quick_info_inherit_doc5(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoInheritDoc5") {
        return;
    }
    let content = r"// @allowJs: true
// @checkJs: true
// @Filename: quickInfoInheritDoc5.js
function A() {}

class B extends A {
    /**
     * @inheritdoc
     */
    static /**/value() {
        return undefined;
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
