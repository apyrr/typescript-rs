#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_decorators() {
    let mut t = TestingT;
    run_test_quick_info_for_decorators(&mut t);
}

fn run_test_quick_info_for_decorators(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoForDecorators") {
        return;
    }
    let content = r"@/*1*/decorator
class C {
}
/** decorator documentation*/
var decorator = t=> t;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "1",
        "var decorator: (t: any) => any",
        "decorator documentation",
    );
    done();
}
