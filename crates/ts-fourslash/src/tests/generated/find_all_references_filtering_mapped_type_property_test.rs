#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_references_filtering_mapped_type_property() {
    let mut t = TestingT;
    run_test_find_all_references_filtering_mapped_type_property(&mut t);
}

fn run_test_find_all_references_filtering_mapped_type_property(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllReferencesFilteringMappedTypeProperty") {
        return;
    }
    let content = r"const obj = { /*1*/a: 1, b: 2 };
const filtered: { [P in keyof typeof obj as P extends 'b' ? never : P]: 0; } = { /*2*/a: 0 };
filtered./*3*/a;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
