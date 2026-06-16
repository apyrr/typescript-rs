#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_edits_for_file_rename_rename_from_index() {
    let mut t = TestingT;
    run_test_get_edits_for_file_rename_rename_from_index(&mut t);
}

fn run_test_get_edits_for_file_rename_rename_from_index(t: &mut TestingT) {
    if should_skip_if_failing("TestGetEditsForFileRename_renameFromIndex") {
        return;
    }
    let content = r#"// @Filename: /a.ts
/// <reference path="./src/index.ts" />
import old from "./src";
import old2 from "./src/index";
// @Filename: /src/a.ts
/// <reference path="./index.ts" />
import old from ".";
import old2 from "./index";
// @Filename: /src/foo/a.ts
/// <reference path="../index.ts" />
import old from "..";
import old2 from "../index";
// @Filename: /src/index.ts

// @Filename: /tsconfig.json
{ "files": ["a.ts", "src/a.ts", "src/foo/a.ts", "src/index.ts"] }"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_will_rename_files_edits(
        t,
        "/src/index.ts",
        "/src/new.ts",
        std::collections::HashMap::from([
            (
                "/a.ts".to_string(),
                r#"/// <reference path="./src/new.ts" />
import old from "./src/new";
import old2 from "./src/new";"#
                    .to_string(),
            ),
            (
                "/src/a.ts".to_string(),
                r#"/// <reference path="./new.ts" />
import old from "./new";
import old2 from "./new";"#
                    .to_string(),
            ),
            (
                "/src/foo/a.ts".to_string(),
                r#"/// <reference path="../new.ts" />
import old from "../new";
import old2 from "../new";"#
                    .to_string(),
            ),
            (
                "/tsconfig.json".to_string(),
                r#"{ "files": ["a.ts", "src/a.ts", "src/foo/a.ts", "src/new.ts"] }"#.to_string(),
            ),
        ]),
    );
    done();
}
