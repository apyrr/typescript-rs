#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_type_formatting() {
    let mut t = TestingT;
    run_test_import_type_formatting(&mut t);
}

fn run_test_import_type_formatting(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"var y: import("./c2").mytype;
var z: import ("./c2").mytype;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r#"var y: import("./c2").mytype;
var z: import("./c2").mytype;"#,
    );
    done();
}
