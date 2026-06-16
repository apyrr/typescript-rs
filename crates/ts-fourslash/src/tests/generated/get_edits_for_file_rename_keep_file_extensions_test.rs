#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_edits_for_file_rename_keep_file_extensions() {
    let mut t = TestingT;
    run_test_get_edits_for_file_rename_keep_file_extensions(&mut t);
}

fn run_test_get_edits_for_file_rename_keep_file_extensions(t: &mut TestingT) {
    if should_skip_if_failing("TestGetEditsForFileRename_keepFileExtensions") {
        return;
    }
    let content = r#"// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "module": "Node16",
    "rootDirs": ["src"]
  }
}
// @Filename: /src/person.ts
export const name = 0;
// @Filename: /src/index.ts
import {name} from "./person.js";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_will_rename_files_edits(
        t,
        "/src/person.ts",
        "/src/vip.ts",
        std::collections::HashMap::from([(
            "/src/index.ts".to_string(),
            r#"import {name} from "./vip.js";"#.to_string(),
        )]),
    );
    done();
}
