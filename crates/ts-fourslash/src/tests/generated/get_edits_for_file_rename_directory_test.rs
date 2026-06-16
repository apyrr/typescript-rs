#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_edits_for_file_rename_directory() {
    let mut t = TestingT;
    run_test_get_edits_for_file_rename_directory(&mut t);
}

fn run_test_get_edits_for_file_rename_directory(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /a.ts
/// <reference path="./src/old/file.ts" />
import old from "./src/old";
import old2 from "./src/old/file";
export default 0;
// @Filename: /src/b.ts
/// <reference path="./old/file.ts" />
import old from "./old";
import old2 from "./old/file";
export default 0;
// @Filename: /src/foo/c.ts
/// <reference path="../old/file.ts" />
import old from "../old";
import old2 from "../old/file";
export default 0;
// @Filename: /src/old/index.ts
import a from "../../a";
import a2 from "../b";
import a3 from "../foo/c";
import f from "./file";
export default 0;
// @Filename: /src/old/file.ts
export default 0;
// @Filename: /tsconfig.json
{ "files": ["a.ts", "src/b.ts", "src/foo/c.ts", "src/old/index.ts", "src/old/file.ts"] }"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_will_rename_files_edits(t, "/src/old", "/src/new", std::collections::HashMap::from([
    ("/a.ts".to_string(), r#"/// <reference path="./src/new/file.ts" />
import old from "./src/new";
import old2 from "./src/new/file";
export default 0;"#.to_string()),
    ("/src/b.ts".to_string(), r#"/// <reference path="./new/file.ts" />
import old from "./new";
import old2 from "./new/file";
export default 0;"#.to_string()),
    ("/src/foo/c.ts".to_string(), r#"/// <reference path="../new/file.ts" />
import old from "../new";
import old2 from "../new/file";
export default 0;"#.to_string()),
    ("/tsconfig.json".to_string(), r#"{ "files": ["a.ts", "src/b.ts", "src/foo/c.ts", "src/new/index.ts", "src/new/file.ts"] }"#.to_string()),
]));
    done();
}
