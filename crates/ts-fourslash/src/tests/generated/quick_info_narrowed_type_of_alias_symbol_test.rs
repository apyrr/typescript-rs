#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_narrowed_type_of_alias_symbol() {
    let mut t = TestingT;
    run_test_quick_info_narrowed_type_of_alias_symbol(&mut t);
}

fn run_test_quick_info_narrowed_type_of_alias_symbol(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoNarrowedTypeOfAliasSymbol") {
        return;
    }
    let content = r#"// @strict: true
// @Filename: modules.ts
export declare const someEnv: string | undefined;
// @Filename: app.ts
import { someEnv } from "./modules";
declare function isString(v: any): v is string;

if (isString(someEnv)) {
  someEnv/*1*/.charAt(0);
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "app.ts");
    f.go_to_marker(t, "1");
    f.verify_quick_info_is(t, "(alias) const someEnv: string\nimport someEnv", "");
    done();
}
