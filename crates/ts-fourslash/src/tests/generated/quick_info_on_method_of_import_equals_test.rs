#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_method_of_import_equals() {
    let mut t = TestingT;
    run_test_quick_info_on_method_of_import_equals(&mut t);
}

fn run_test_quick_info_on_method_of_import_equals(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /a.d.ts
declare class C<T> {
    m(): void;
}
export = C;
// @Filename: /b.ts
import C = require("./a");
declare var x: C<number>;
x./**/m;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "(method) C<number>.m(): void", "");
    done();
}
