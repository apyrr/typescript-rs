#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_edits_for_file_rename_amd() {
    let mut t = TestingT;
    run_test_get_edits_for_file_rename_amd(&mut t);
}

fn run_test_get_edits_for_file_rename_amd(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @moduleResolution: classic
// @Filename: /src/user.ts
import { x } from "old";
// @Filename: /src/old.ts
export const x = 0;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_will_rename_files_edits(
        t,
        "/src/old.ts",
        "/src/new.ts",
        std::collections::HashMap::from([(
            "/src/user.ts".to_string(),
            r#"import { x } from "./new";"#.to_string(),
        )]),
    );
    done();
}
