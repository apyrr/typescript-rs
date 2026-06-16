#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_js_doc_alias() {
    let mut t = TestingT;
    run_test_quick_info_js_doc_alias(&mut t);
}

fn run_test_quick_info_js_doc_alias(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJsDocAlias") {
        return;
    }
    let content = r#"// @filename: /a.d.ts
/** docs - type T */
export type T = () => void;
/**
 * docs - const A: T
 */
export declare const A: T;
// @filename: /b.ts
import { A } from "./a";
A/**/()"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
