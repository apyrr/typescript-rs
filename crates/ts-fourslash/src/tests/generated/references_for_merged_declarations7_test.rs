#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_merged_declarations7() {
    let mut t = TestingT;
    run_test_references_for_merged_declarations7(&mut t);
}

fn run_test_references_for_merged_declarations7(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForMergedDeclarations7") {
        return;
    }
    let content = r"interface Foo { }
namespace Foo {
    export interface /*1*/Bar { }
    export module /*2*/Bar { export interface Baz { } }
    export function /*3*/Bar() { }
}

// module, value and type
import a2 = Foo./*4*/Bar;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
        ],
    );
    done();
}
