#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_get_canonical_file_name() {
    let mut t = TestingT;
    run_test_import_name_code_fix_get_canonical_file_name(&mut t);
}

fn run_test_import_name_code_fix_get_canonical_file_name(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: /howNow/node_modules/brownCow/index.d.ts
export const foo: number;
// @Filename: /howNow/a.ts
foo;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/howNow/a.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { foo } from "brownCow";

foo;"#
                .to_string(),
        ],
        None,
    );
    done();
}
