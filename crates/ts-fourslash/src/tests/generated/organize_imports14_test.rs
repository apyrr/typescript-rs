#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports14() {
    let mut t = TestingT;
    run_test_organize_imports14(&mut t);
}

fn run_test_organize_imports14(t: &mut TestingT) {
    if should_skip_if_failing("TestOrganizeImports14") {
        return;
    }
    let content = r#"// @filename: /a.ts
export const foo = 1;
// @filename: /b.ts
/**
 * Module doc comment
 *
 * @module
 */

// comment 1

// comment 2

import { foo } from "./a";
import { foo } from "./a";
import { foo } from "./a";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/b.ts");
    f.verify_organize_imports(
        t,
        r"/**
 * Module doc comment
 *
 * @module
 */

// comment 1

// comment 2

",
        "source.organizeImports",
        None,
    );
    done();
}
