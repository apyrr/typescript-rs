#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_items_modules1() {
    let mut t = TestingT;
    run_test_navigation_bar_items_modules1(&mut t);
}

fn run_test_navigation_bar_items_modules1(t: &mut TestingT) {
    if should_skip_if_failing("TestNavigationBarItemsModules1") {
        return;
    }
    let content = r#"declare module "X.Y.Z" {}

declare module 'X2.Y2.Z2' {}

declare module "foo";

namespace A.B.C {
    export var x;
}

namespace A.B {
    export var y;
}

namespace A {
    export var z;
}

namespace A {
    namespace B {
        namespace C {
            declare var x;
        }
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
