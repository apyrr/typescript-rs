#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_symlink() {
    let mut t = TestingT;
    run_test_import_name_code_fix_symlink(&mut t);
}

fn run_test_import_name_code_fix_symlink(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @moduleResolution: bundler
// @noLib: true
// @Filename: /node_modules/real/index.d.ts
// @Symlink: /node_modules/link/index.d.ts
export const foo: number;
// @Filename: /a.ts
import { foo } from "link";
// @Filename: /b.ts
[|foo;|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/b.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { foo } from "link";

foo;"#
                .to_string(),
            r#"import { foo } from "real";

foo;"#
                .to_string(),
        ],
        None,
    );
    done();
}
