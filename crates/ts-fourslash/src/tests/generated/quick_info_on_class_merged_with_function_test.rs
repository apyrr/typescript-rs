#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_class_merged_with_function() {
    let mut t = TestingT;
    run_test_quick_info_on_class_merged_with_function(&mut t);
}

fn run_test_quick_info_on_class_merged_with_function(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoOnClassMergedWithFunction") {
        return;
    }
    let content = r#"namespace Test {
    class Mocked {
        myProp: string;
    }
    class Tester {
        willThrowError() {
            Mocked = Mocked || function () { // => Error: Invalid left-hand side of assignment expression.
                return { /**/myProp: "test" };
            };
        }
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "(property) myProp: string", "");
    done();
}
