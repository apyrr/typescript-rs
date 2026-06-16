#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_tagged_template_overloads() {
    let mut t = TestingT;
    run_test_go_to_definition_tagged_template_overloads(&mut t);
}

fn run_test_go_to_definition_tagged_template_overloads(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"function /*defFNumber*/f(strs: TemplateStringsArray, x: number): void;
function /*defFBool*/f(strs: TemplateStringsArray, x: boolean): void;
function f(strs: TemplateStringsArray, x: number | boolean) {}

[|/*useFNumber*/f|]` + "`" + `${0}` + "`" + `;
[|/*useFBool*/f|]` + "`" + `${false}` + "`" + `;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["useFNumber".to_string(), "useFBool".to_string()]);
    done();
}
