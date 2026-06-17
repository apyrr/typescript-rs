#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_shorthand_property_assignment2() {
    let mut t = TestingT;
    run_test_import_name_code_fix_shorthand_property_assignment2(&mut t);
}

fn run_test_import_name_code_fix_shorthand_property_assignment2(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_shorthandPropertyAssignment2") {
        return;
    }
    let content = r"// @Filename: /a.ts
const a = 1;
export default a;
// @Filename: /b.ts
const b = { /**/a };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/b.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import a from "./a";

const b = { a };"#
            .to_string()],
        None,
    );
    done();
}
