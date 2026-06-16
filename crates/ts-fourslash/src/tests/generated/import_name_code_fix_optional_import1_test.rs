#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_optional_import1() {
    let mut t = TestingT;
    run_test_import_name_code_fix_optional_import1(&mut t);
}

fn run_test_import_name_code_fix_optional_import1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: a/f1.ts
[|foo/*0*/();|]
// @Filename: a/node_modules/bar/index.ts
export function foo() {};
// @Filename: a/foo.ts
export { foo } from "bar";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { foo } from "bar";

foo();"#
                .to_string(),
            r#"import { foo } from "./foo";

foo();"#
                .to_string(),
        ],
        None,
    );
    done();
}
