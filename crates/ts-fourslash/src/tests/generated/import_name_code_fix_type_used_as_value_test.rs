#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_type_used_as_value() {
    let mut t = TestingT;
    run_test_import_name_code_fix_type_used_as_value(&mut t);
}

fn run_test_import_name_code_fix_type_used_as_value(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: /a.ts
export class ReadonlyArray<T> {}
// @Filename: /b.ts
[|new ReadonlyArray<string>();|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/b.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { ReadonlyArray } from "./a";

new ReadonlyArray<string>();"#
                .to_string(),
        ],
        None,
    );
    done();
}
