#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_union_type_property_discriminated() {
    let mut t = TestingT;
    run_test_go_to_definition_union_type_property_discriminated(&mut t);
}

fn run_test_go_to_definition_union_type_property_discriminated(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"type U = A | B;

interface A {
  /*aKind*/kind: "a";
  /*aProp*/prop: number;
};

interface B {
  /*bKind*/kind: "b";
  /*bProp*/prop: string;
}

const u: U = {
  [|/*kind*/kind|]: "a",
  [|/*prop*/prop|]: 0,
};
const u2: U = {
  [|/*kindBogus*/kind|]: "bogus",
  [|/*propBogus*/prop|]: 0,
};"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "kind".to_string(),
            "prop".to_string(),
            "kindBogus".to_string(),
            "propBogus".to_string(),
        ],
    );
    done();
}
