#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_indented_identifier() {
    let mut t = TestingT;
    run_test_import_name_code_fix_indented_identifier(&mut t);
}

fn run_test_import_name_code_fix_indented_identifier(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFixIndentedIdentifier") {
        return;
    }
    let content = r#"// @Filename: /a.ts
[|import * as b from "./b";
{
    x/**/
}|]
// @Filename: /b.ts
export const x = 0;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import * as b from "./b";
{
    b.x
}"#
            .to_string(),
            r#"import * as b from "./b";
import { x } from "./b";
{
    x
}"#
            .to_string(),
        ],
        None,
    );
    done();
}
