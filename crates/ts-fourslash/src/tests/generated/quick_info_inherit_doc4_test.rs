#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_inherit_doc4() {
    let mut t = TestingT;
    run_test_quick_info_inherit_doc4(&mut t);
}

fn run_test_quick_info_inherit_doc4(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: quickInfoInheritDoc4.ts
var A: any;

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
