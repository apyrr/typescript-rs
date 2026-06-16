#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_default_export2() {
    let mut t = TestingT;
    run_test_import_name_code_fix_default_export2(&mut t);
}

fn run_test_import_name_code_fix_default_export2(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFixDefaultExport2") {
        return;
    }
    let content = r"// @Filename: /lib.ts
class Base { }
export default Base;
// @Filename: /test.ts
[|class Derived extends Base { }|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/test.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import Base from "./lib";

class Derived extends Base { }"#
                .to_string(),
        ],
        None,
    );
    done();
}
