#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_in_type_argument() {
    let mut t = TestingT;
    run_test_go_to_definition_in_type_argument(&mut t);
}

fn run_test_go_to_definition_in_type_argument(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class /*fooDefinition*/Foo<T> { }

class /*barDefinition*/Bar { }

var x = new Fo/*fooReference*/o<Ba/*barReference*/r>();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &["barReference".to_string(), "fooReference".to_string()],
    );
    done();
}
