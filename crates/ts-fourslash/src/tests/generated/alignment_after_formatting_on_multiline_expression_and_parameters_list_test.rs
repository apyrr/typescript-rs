#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_alignment_after_formatting_on_multiline_expression_and_parameters_list() {
    let mut t = TestingT;
    run_test_alignment_after_formatting_on_multiline_expression_and_parameters_list(&mut t);
}

fn run_test_alignment_after_formatting_on_multiline_expression_and_parameters_list(
    t: &mut TestingT,
) {
    skip_if_failing(t);
    let content = r"class TestClass {
    private testMethod1(param1: boolean,
                        param2/*1*/: boolean) {
    }

    public testMethod2(a: number, b: number, c: number) {
        if (a === b) {
        }
        else if (a != c &&
                 a/*2*/ > b &&
                 b/*3*/ < c) {
        }

    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_indentation(t, 8);
    f.go_to_marker(t, "2");
    f.verify_indentation(t, 12);
    f.go_to_marker(t, "3");
    f.verify_indentation(t, 12);
    done();
}
