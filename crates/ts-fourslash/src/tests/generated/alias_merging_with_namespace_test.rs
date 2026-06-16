#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_alias_merging_with_namespace() {
    let mut t = TestingT;
    run_test_alias_merging_with_namespace(&mut t);
}

fn run_test_alias_merging_with_namespace(t: &mut TestingT) {
    if should_skip_if_failing("TestAliasMergingWithNamespace") {
        return;
    }
    let content = r"namespace bar { }
import bar = bar/**/;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "namespace bar\nimport bar = bar", "");
    done();
}
