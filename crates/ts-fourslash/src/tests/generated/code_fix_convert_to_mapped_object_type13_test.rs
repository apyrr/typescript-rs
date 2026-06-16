#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_convert_to_mapped_object_type13() {
    let mut t = TestingT;
    run_test_code_fix_convert_to_mapped_object_type13(&mut t);
}

fn run_test_code_fix_convert_to_mapped_object_type13(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixConvertToMappedObjectType13") {
        return;
    }
    let content = r#"let x: {
    [p: ""]: string;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &vec!["fixConvertToMappedObjectType".to_string()]);
    done();
}
