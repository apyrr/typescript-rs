#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_special_property_assignment() {
    let mut t = TestingT;
    run_test_quick_info_special_property_assignment(&mut t);
}

fn run_test_quick_info_special_property_assignment(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @allowJs: true
// @Filename: /a.js
class C {
    constructor() {
      /** Doc */
      this./*write*/x = 0;
      this./*read*/x;
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "write", "(property) C.x: any", "Doc");
    f.verify_quick_info_at(t, "read", "(property) C.x: number", "Doc");
    done();
}
