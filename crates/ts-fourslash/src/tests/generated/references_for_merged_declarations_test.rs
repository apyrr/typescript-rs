#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_merged_declarations() {
    let mut t = TestingT;
    run_test_references_for_merged_declarations(&mut t);
}

fn run_test_references_for_merged_declarations(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForMergedDeclarations") {
        return;
    }
    let content = r"/*1*/interface /*2*/Foo {
}

/*3*/module /*4*/Foo {
    export interface Bar { }
}

/*5*/function /*6*/Foo(): void {
}

var f1: /*7*/Foo.Bar;
var f2: /*8*/Foo;
/*9*/Foo.bind(this);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
            "7".to_string(),
            "8".to_string(),
            "9".to_string(),
        ],
    );
    done();
}
