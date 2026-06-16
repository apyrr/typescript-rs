#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_at_identifier_definition_locations_infers() {
    let mut t = TestingT;
    run_test_completion_list_at_identifier_definition_locations_infers(&mut t);
}

fn run_test_completion_list_at_identifier_definition_locations_infers(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListAtIdentifierDefinitionLocations_infers") {
        return;
    }
    let content = r"type UType = 1;
type Bar<T> = T extends { a: (x: infer /*1*/) => void; b: (x: infer U/*2*/) => void }
   ? U
   : never;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Markers(f.markers()), None);
    done();
}
