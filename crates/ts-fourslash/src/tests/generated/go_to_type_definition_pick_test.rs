#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_type_definition_pick() {
    let mut t = TestingT;
    run_test_go_to_type_definition_pick(&mut t);
}

fn run_test_go_to_type_definition_pick(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToTypeDefinition_Pick") {
        return;
    }
    let content = r#"// @lib: es5
type User = { id: number; name: string; };
declare const user: Pick<User, "name">
/*reference*/user

type PickedUser = Pick<User, "name">
declare const user2: PickedUser
/*reference2*/user2"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_type_definition(
        t,
        &["reference".to_string(), "reference2".to_string()],
    );
    done();
}
