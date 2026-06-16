#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_string_literal_property_names3() {
    let mut t = TestingT;
    run_test_references_for_string_literal_property_names3(&mut t);
}

fn run_test_references_for_string_literal_property_names3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"class Foo2 {
    /*1*/get "/*2*/42"() { return 0; }
    /*3*/set /*4*/42(n) { }
}

var y: Foo2;
y[/*5*/42];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
        ],
    );
    done();
}
