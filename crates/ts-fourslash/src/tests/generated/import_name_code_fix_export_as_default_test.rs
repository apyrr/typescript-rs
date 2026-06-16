#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_export_as_default() {
    let mut t = TestingT;
    run_test_import_name_code_fix_export_as_default(&mut t);
}

fn run_test_import_name_code_fix_export_as_default(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFixExportAsDefault") {
        return;
    }
    let content = r"// @Filename: /foo.ts
const foo = 'foo'
export { foo as default }
// @Filename: /index.ts
 foo/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_apply_code_action_from_completion(
        t,
        Some(""),
        &ApplyCodeActionFromCompletionOptions {
            name: "foo".to_string(),
            source: "./foo".to_string(),
            auto_import_fix: None,
            description: "Add import from \"./foo\"".to_string(),
            new_file_content: Some(
                r#"import foo from "./foo";

foo"#
                    .to_string(),
            ),
            new_range_content: None,
            user_preferences: None,
        },
    );
    done();
}
