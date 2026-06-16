#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_type_definition_array_type() {
    let mut t = TestingT;
    run_test_go_to_type_definition_array_type(&mut t);
}

fn run_test_go_to_type_definition_array_type(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToTypeDefinition_arrayType") {
        return;
    }
    let content = r"// @lib: es5
type User = { name: string };
declare const users: User[]
/*reference*/users

type UsersArr = Array<User>
declare const users2: UsersArr
/*reference2*/users2

class CustomArray<T> extends Array<T> { immutableReverse() { return [...this].reverse() } }
declare const users3: CustomArray<User>
/*reference3*/users3";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_type_definition(
        t,
        &[
            "reference".to_string(),
            "reference2".to_string(),
            "reference3".to_string(),
        ],
    );
    done();
}
