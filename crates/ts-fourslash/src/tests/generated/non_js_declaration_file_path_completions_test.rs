#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_non_js_declaration_file_path_completions() {
    let mut t = TestingT;
    run_test_non_js_declaration_file_path_completions(&mut t);
}

fn run_test_non_js_declaration_file_path_completions(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @lib: es5
// @allowArbitraryExtensions: true
// @Filename: /home/src/workspaces/project/mod.d.html.ts
export declare class HtmlModuleThing {}
// @Filename: /home/src/workspaces/project/node_modules/package/mod.d.html.ts
export declare class PackageHtmlModuleThing {}
// @Filename: /home/src/workspaces/project/usage.ts
import { HtmlModuleThing } from ".//*1*/";
import { PackageHtmlModuleThing } from "package//*2*/";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_completions(t, &[]);
    done();
}
