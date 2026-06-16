#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navbar_export_default() {
    let mut t = TestingT;
    run_test_navbar_export_default(&mut t);
}

fn run_test_navbar_export_default(t: &mut TestingT) {
    if should_skip_if_failing("TestNavbar_exportDefault") {
        return;
    }
    let content = r"// @Filename: a.ts
export default class { }
// @Filename: b.ts
export default class C { }
// @Filename: c.ts
export default function { }
// @Filename: d.ts
export default function Func { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "a.ts");
    f.verify_baseline_document_symbol(t);
    f.go_to_file(t, "b.ts");
    f.verify_baseline_document_symbol(t);
    f.go_to_file(t, "c.ts");
    f.verify_baseline_document_symbol(t);
    f.go_to_file(t, "d.ts");
    f.verify_baseline_document_symbol(t);
    done();
}
