#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_paths_without_base_url2() {
    let mut t = TestingT;
    run_test_import_name_code_fix_paths_without_base_url2(&mut t);
}

fn run_test_import_name_code_fix_paths_without_base_url2(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_pathsWithoutBaseUrl2") {
        return;
    }
    let content = r#"// @Filename: /packages/test-package-1/tsconfig.json
{
  "compilerOptions": {
    "module": "commonjs",
    "paths": {
      "test-package-2/*": ["../test-package-2/src/*"]
    }
  }
}
// @Filename: /packages/test-package-1/src/common/logging.ts
export class Logger {};
// @Filename: /packages/test-package-1/src/something/index.ts
Logger/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { Logger } from "../common/logging";

Logger"#
                .to_string(),
        ],
        None,
    );
    done();
}
