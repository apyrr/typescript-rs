#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_jsx_intrinsic_declared_using_catch_call_index_signature() {
    let mut t = TestingT;
    run_test_quick_info_on_jsx_intrinsic_declared_using_catch_call_index_signature(&mut t);
}

fn run_test_quick_info_on_jsx_intrinsic_declared_using_catch_call_index_signature(
    t: &mut TestingT,
) {
    skip_if_failing(t);
    let content = r#"// @jsx: react
// @filename: /a.tsx
declare namespace JSX {
  interface IntrinsicElements { [elemName: string]: any; }
}
</**/div class="democlass" />;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
