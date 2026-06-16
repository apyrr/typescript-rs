#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_label() {
    let mut t = TestingT;
    run_test_references_for_label(&mut t);
}

fn run_test_references_for_label(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForLabel") {
        return;
    }
    let content = r#"/*1*/label: while (true) {
    if (false) /*2*/break /*3*/label;
    if (true) /*4*/continue /*5*/label;
}

/*6*/label: while (false) { }
var label = "label";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
        ],
    );
    done();
}
