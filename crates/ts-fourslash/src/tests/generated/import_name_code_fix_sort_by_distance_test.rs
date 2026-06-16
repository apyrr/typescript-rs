#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_sort_by_distance() {
    let mut t = TestingT;
    run_test_import_name_code_fix_sort_by_distance(&mut t);
}

fn run_test_import_name_code_fix_sort_by_distance(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_sortByDistance") {
        return;
    }
    let content = r#"// @module: commonjs
// @Filename: /src/admin/utils/db/db.ts
export const db = {};
// @Filename: /src/admin/utils/db/index.ts
export * from "./db";
// @Filename: /src/client/helpers/db.ts
export const db = {};
// @Filename: /src/client/db.ts
export const db = {};
// @Filename: /src/client/foo.ts
db/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { db } from "./db";

db"#
            .to_string(),
            r#"import { db } from "./helpers/db";

db"#
            .to_string(),
            r#"import { db } from "../admin/utils/db";

db"#
            .to_string(),
            r#"import { db } from "../admin/utils/db/db";

db"#
            .to_string(),
        ],
        None,
    );
    done();
}
