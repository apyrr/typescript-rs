#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_merging() {
    let mut t = TestingT;
    run_test_navigation_bar_merging(&mut t);
}

fn run_test_navigation_bar_merging(t: &mut TestingT) {
    if should_skip_if_failing("TestNavigationBarMerging") {
        return;
    }
    let content = r"// @Filename: file1.ts
namespace a {
    function foo() {}
}
namespace b {
    function foo() {}
}
namespace a {
    function bar() {}
}
// @Filename: file2.ts
namespace a {}
function a() {}
// @Filename: file3.ts
namespace a {
    interface A {
        foo: number;
    }
}
namespace a {
    interface A {
        bar: number;
    }
}
// @Filename: file4.ts
namespace A { export var x; }
namespace A.B { export var y; }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    f.go_to_file(t, "file2.ts");
    f.verify_baseline_document_symbol(t);
    f.go_to_file(t, "file3.ts");
    f.verify_baseline_document_symbol(t);
    f.go_to_file(t, "file4.ts");
    f.verify_baseline_document_symbol(t);
    done();
}
