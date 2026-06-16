#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_new_identifier_binding_element() {
    let mut t = TestingT;
    run_test_completion_list_new_identifier_binding_element(&mut t);
}

fn run_test_completion_list_new_identifier_binding_element(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListNewIdentifierBindingElement") {
        return;
    }
    let content = r"var { x:html/*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Name("1".to_string()), None);
    done();
}
