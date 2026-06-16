#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_insert_space_after_type_assertion() {
    let mut t = TestingT;
    run_test_format_insert_space_after_type_assertion(&mut t);
}

fn run_test_format_insert_space_after_type_assertion(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatInsertSpaceAfterTypeAssertion") {
        return;
    }
    let content = r#"let a = <string> "";
let b = <number> 1;
let c = <any[]> [];
let d = <string[]> [];
let e = <string[]> ["e"];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings.insert_space_after_type_assertion = ts_core::TSTrue;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r#"let a=<string> "";
let b=<number> 1;
let c=<any[]> [];
let d=<string[]> [];
let e=<string[]> ["e"];"#,
    );
    done();
}
