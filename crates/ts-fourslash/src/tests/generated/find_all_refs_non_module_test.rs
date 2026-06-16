#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_non_module() {
    let mut t = TestingT;
    run_test_find_all_refs_non_module(&mut t);
}

fn run_test_find_all_refs_non_module(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsNonModule") {
        return;
    }
    let content = r#"// @checkJs: true
// @Filename: /script.ts
console.log("I'm a script!");
// @Filename: /import.ts
import "./script/*1*/";
// @Filename: /require.js
require("./script/*2*/");
console.log("./script/*3*/");
// @Filename: /tripleSlash.ts
/// <reference path="script.ts" />
// @Filename: /stringLiteral.ts
console.log("./script");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
