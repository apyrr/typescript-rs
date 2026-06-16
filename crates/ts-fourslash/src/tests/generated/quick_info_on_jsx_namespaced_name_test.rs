#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_jsx_namespaced_name() {
    let mut t = TestingT;
    run_test_quick_info_on_jsx_namespaced_name(&mut t);
}

fn run_test_quick_info_on_jsx_namespaced_name(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoOnJsxNamespacedName") {
        return;
    }
    let content = r#"// @jsx: react
// @Filename: /types.d.ts
declare namespace JSX {
    interface IntrinsicElements { ['a:b']: { a: string }; }
}
// @filename: /a.tsx
</**/a:b a="accepted" b="rejected" />;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
