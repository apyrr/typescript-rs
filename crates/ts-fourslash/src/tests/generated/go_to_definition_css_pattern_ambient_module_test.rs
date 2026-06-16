#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_css_pattern_ambient_module() {
    let mut t = TestingT;
    run_test_go_to_definition_css_pattern_ambient_module(&mut t);
}

fn run_test_go_to_definition_css_pattern_ambient_module(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @esModuleInterop: true
// @Filename: index.css
/*2a*/html { font-size: 16px; }
// @Filename: types.ts
declare module /*2b*/"*.css" {
  const styles: any;
  export = styles;
}
// @Filename: index.ts
import styles from [|/*1*/"./index.css"|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["1".to_string()]);
    done();
}
