#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_inherited_link_tag() {
    let mut t = TestingT;
    run_test_quick_info_inherited_link_tag(&mut t);
}

fn run_test_quick_info_inherited_link_tag(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoInheritedLinkTag") {
        return;
    }
    let content = r"export class C {
     /**
      * @deprecated Use {@link PerspectiveCamera#setFocalLength .setFocalLength()} and {@link PerspectiveCamera#filmGauge .filmGauge} instead.
      */
    m() { }
}
export class D extends C {
    m() { } // crashes here
}
new C().m/**/ // and here (with a different thing trying to access undefined)";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_hover(t, &[]);
    done();
}
