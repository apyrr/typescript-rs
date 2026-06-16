#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_paths_without_base_url1() {
    let mut t = TestingT;
    run_test_import_name_code_fix_paths_without_base_url1(&mut t);
}

fn run_test_import_name_code_fix_paths_without_base_url1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: tsconfig.json
{
  "compilerOptions": {
    "module": "commonjs",
    "paths": {
      "@app/*": ["./lib/*"]
    }
  }
}
// @Filename: index.ts
utils/**/
// @Filename: lib/utils.ts
export const utils = {};"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { utils } from "@app/utils";

utils"#
                .to_string(),
        ],
        None,
    );
    done();
}
