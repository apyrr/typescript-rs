#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_fixes_quote_preference_double_import_helpers() {
    let mut t = TestingT;
    run_test_import_fixes_quote_preference_double_import_helpers(&mut t);
}

fn run_test_import_fixes_quote_preference_double_import_helpers(t: &mut TestingT) {
    if should_skip_if_failing("TestImportFixes_quotePreferenceDouble_importHelpers") {
        return;
    }
    let content = r#"// @importHelpers: true
// @filename: /a.ts
export default () => {};
// @filename: /b.ts
export default () => {};
// @filename: /test.ts
import a from "./a";
[|b|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/test.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import b from "./b";
b"#
        .to_string()],
        None,
    );
    done();
}
