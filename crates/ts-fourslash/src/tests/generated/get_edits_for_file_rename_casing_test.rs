#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_edits_for_file_rename_casing() {
    let mut t = TestingT;
    run_test_get_edits_for_file_rename_casing(&mut t);
}

fn run_test_get_edits_for_file_rename_casing(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /a.ts
import { foo } from "./dir/fOo";
// @Filename: /dir/fOo.ts
export const foo = 0;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_will_rename_files_edits(
        t,
        "/dir",
        "/newDir",
        std::collections::HashMap::from([(
            "/a.ts".to_string(),
            r#"import { foo } from "./newDir/fOo";"#.to_string(),
        )]),
    );
    done();
}
