#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_namespace_import_name01() {
    let mut t = TestingT;
    run_test_completion_list_in_namespace_import_name01(&mut t);
}

fn run_test_completion_list_in_namespace_import_name01(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListInNamespaceImportName01") {
        return;
    }
    let content = r#"// @Filename: m1.ts
export var foo: number = 1;
// @Filename: m2.ts
import * as /**/ from "m1""#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Name("".to_string()), None);
    done();
}
