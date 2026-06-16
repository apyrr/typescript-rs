#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_highlights_for_export_from_unfound_module() {
    let mut t = TestingT;
    run_test_highlights_for_export_from_unfound_module(&mut t);
}

fn run_test_highlights_for_export_from_unfound_module(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @allowJs: true
// @Filename: a.js
import foo from 'unfound';
export {
  foo,
};
// @Filename: b.js
export {
   /**/foo
} from './a';";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_baseline_rename(t, &["".to_string()]);
    done();
}
