#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_is_available_through_global_no_crash() {
    let mut t = TestingT;
    run_test_references_is_available_through_global_no_crash(&mut t);
}

fn run_test_references_is_available_through_global_no_crash(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesIsAvailableThroughGlobalNoCrash") {
        return;
    }
    let content = r#"// @Filename: /packages/playwright-core/bundles/utils/node_modules/@types/debug/index.d.ts
declare var debug: debug.Debug & { debug: debug.Debug; default: debug.Debug };
export = debug;
export as namespace debug;
declare namespace debug {
    interface Debug {
       coerce: (val: any) => any;
    }
}
// @Filename: /packages/playwright-core/bundles/utils/node_modules/@types/debug/package.json
{ "types": "index.d.ts" }
// @Filename: /packages/playwright-core/src/index.ts
export const debug: typeof import('../bundles/utils/node_modules//*1*/@types/debug') = require('./utilsBundleImpl').debug;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
