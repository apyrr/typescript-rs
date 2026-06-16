#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_at_identifier_definition_locations_generics() {
    let mut t = TestingT;
    run_test_completion_list_at_identifier_definition_locations_generics(&mut t);
}

fn run_test_completion_list_at_identifier_definition_locations_generics(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListAtIdentifierDefinitionLocations_Generics") {
        return;
    }
    let content = r"interface A</*genericName1*/
class A</*genericName2*/
class B<T, /*genericName3*/
class A{
     f</*genericName4*/
function A</*genericName5*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Markers(f.markers()), None);
    done();
}
