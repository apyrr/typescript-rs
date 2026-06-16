#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_at_identifier_definition_locations_catch() {
    let mut t = TestingT;
    run_test_completion_list_at_identifier_definition_locations_catch(&mut t);
}

fn run_test_completion_list_at_identifier_definition_locations_catch(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"var aa = 1;
 try {} catch(/*catchVariable1*/
 try {} catch(a/*catchVariable2*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Markers(f.markers()), None);
    done();
}
