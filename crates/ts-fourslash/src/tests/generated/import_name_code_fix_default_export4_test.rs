#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_default_export4() {
    let mut t = TestingT;
    run_test_import_name_code_fix_default_export4(&mut t);
}

fn run_test_import_name_code_fix_default_export4(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFixDefaultExport4") {
        return;
    }
    let content = r"// @Filename: /foo.ts
const a = () => {};
export default a;
// @Filename: /test.ts
[|foo|];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/test.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import foo from "./foo";

foo"#
                .to_string(),
        ],
        None,
    );
    done();
}
