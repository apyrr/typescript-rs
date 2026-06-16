#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_top_level_for_await_module_target_es2017_compiler_options_in_ts_config() {
    let mut t = TestingT;
    run_test_code_fix_top_level_for_await_module_target_es2017_compiler_options_in_ts_config(
        &mut t,
    );
}

fn run_test_code_fix_top_level_for_await_module_target_es2017_compiler_options_in_ts_config(
    t: &mut TestingT,
) {
    skip_if_failing(t);
    let content = r#"// @filename: /dir/a.ts
declare const p: number[];
for await (const _ of p);
export {};
// @filename: /dir/tsconfig.json
{
    "compilerOptions": {
        "target": "es2017"
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &vec!["fixTargetOption".to_string()]);
    f.verify_code_fix_available(t, None);
    done();
}
