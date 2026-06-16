#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_union_properties_with_identical_js_doc_comments01() {
    let mut t = TestingT;
    run_test_quick_info_on_union_properties_with_identical_js_doc_comments01(&mut t);
}

fn run_test_quick_info_on_union_properties_with_identical_js_doc_comments01(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoOnUnionPropertiesWithIdenticalJSDocComments01") {
        return;
    }
    let content = r"export type DocumentFilter = {
    /** A language id, like `typescript`. */
    language: string;
    /** A Uri [scheme](#Uri.scheme), like `file` or `untitled`. */
    scheme?: string;
    /** A glob pattern, like `*.{ts,js}`. */
    pattern?: string;
} | {
    /** A language id, like `typescript`. */
    language?: string;
    /** A Uri [scheme](#Uri.scheme), like `file` or `untitled`. */
    scheme: string;
    /** A glob pattern, like `*.{ts,js}`. */
    pattern?: string;
} | {
    /** A language id, like `typescript`. */
    language?: string;
    /** A Uri [scheme](#Uri.scheme), like `file` or `untitled`. */
    scheme?: string;
    /** A glob pattern, like `*.{ts,js}`. */
    pattern: string;
};

declare let x: DocumentFilter;
x./**/language";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
