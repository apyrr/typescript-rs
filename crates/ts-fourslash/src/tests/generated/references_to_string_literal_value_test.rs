#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_to_string_literal_value() {
    let mut t = TestingT;
    run_test_references_to_string_literal_value(&mut t);
}

fn run_test_references_to_string_literal_value(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @lib: es5
const s: string = "some /*1*/ string";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
