#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_type_definition_promise_type() {
    let mut t = TestingT;
    run_test_go_to_type_definition_promise_type(&mut t);
}

fn run_test_go_to_type_definition_promise_type(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @lib: es5,es2015.promise
type User = { name: string };
async function /*reference*/getUser() { return { name: "Bob" } satisfies User as User }

const /*reference2*/promisedBob = getUser() 

export {}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_type_definition(
        t,
        &["reference".to_string(), "reference2".to_string()],
    );
    done();
}
