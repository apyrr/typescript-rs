#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_default_export() {
    let mut t = TestingT;
    run_test_import_name_code_fix_default_export(&mut t);
}

fn run_test_import_name_code_fix_default_export(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @module: esnext
// @allowJs: true
// @checkJs: true
// @Filename: /a.js
class C {}
export default C;
// @Filename: /b.js
[|C;|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/b.js");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import C from "./a";

C;"#
            .to_string(),
        ],
        None,
    );
    done();
}
