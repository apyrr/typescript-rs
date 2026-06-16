#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_link_code_plain() {
    let mut t = TestingT;
    run_test_quick_info_link_code_plain(&mut t);
}

fn run_test_quick_info_link_code_plain(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoLinkCodePlain") {
        return;
    }
    let content = r"export class C {
     /**
      * @deprecated Use {@linkplain PerspectiveCamera#setFocalLength .setFocalLength()} and {@linkcode PerspectiveCamera#filmGauge .filmGauge} instead.
      */
    m() { }
}
new C().m/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_hover(t, &[]);
    done();
}
