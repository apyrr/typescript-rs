#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_js_doc_tags3() {
    let mut t = TestingT;
    run_test_quick_info_js_doc_tags3(&mut t);
}

fn run_test_quick_info_js_doc_tags3(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJsDocTags3") {
        return;
    }
    let content = r#"// @Filename: quickInfoJsDocTags3.ts
interface Foo {
    /**
     * comment
     * @author Me <me@domain.tld>
     * @see x (the parameter)
     * @param {number} x - x comment
     * @param {number} y - y comment
     * @throws {Error} comment
     */
    method(x: number, y: number): void;
}

class Bar implements Foo {
    /**/method(): void {
        throw new Error("Method not implemented.");
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
