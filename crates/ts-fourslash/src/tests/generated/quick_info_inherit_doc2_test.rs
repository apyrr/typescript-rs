#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_inherit_doc2() {
    let mut t = TestingT;
    run_test_quick_info_inherit_doc2(&mut t);
}

fn run_test_quick_info_inherit_doc2(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoInheritDoc2") {
        return;
    }
    let content = r"// @noEmit: true
// @allowJs: true
// @Filename: quickInfoInheritDoc2.ts
class Base<T> {
    /**
     * Base.prop
     */
    prop: T | undefined;
}

class SubClass<T> extends Base<T> {
    /**
     * @inheritdoc
     * SubClass.prop
     */
    /*1*/prop: T | undefined;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
