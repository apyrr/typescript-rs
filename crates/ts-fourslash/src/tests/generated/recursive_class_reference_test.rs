#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_recursive_class_reference() {
    let mut t = TestingT;
    run_test_recursive_class_reference(&mut t);
}

fn run_test_recursive_class_reference(t: &mut TestingT) {
    if should_skip_if_failing("TestRecursiveClassReference") {
        return;
    }
    let content = r"declare namespace Thing { }

namespace Thing {
   var /**/x: Mode;
}

namespace Thing {
  export class Mode { }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_quick_info_exists(t);
    done();
}
