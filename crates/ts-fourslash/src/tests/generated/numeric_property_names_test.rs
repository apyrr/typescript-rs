#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_numeric_property_names() {
    let mut t = TestingT;
    run_test_numeric_property_names(&mut t);
}

fn run_test_numeric_property_names(t: &mut TestingT) {
    if should_skip_if_failing("TestNumericPropertyNames") {
        return;
    }
    let content = r#"var /**/t2 = { 0: 1, 1: "" };"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "var t2: {\n    0: number;\n    1: string;\n}", "");
    done();
}
