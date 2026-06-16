#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_declaration_expressions() {
    let mut t = TestingT;
    run_test_declaration_expressions(&mut t);
}

fn run_test_declaration_expressions(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @noLib: true
[|{| "name": "A", "kind": "class" |}class A {}|]
const [|{| "name": "B", "kind": "const" |}B = [|{| "name": "Cz", "kind": "class" |}class Cz {
    public x;
}|]|];
[|{| "name": "D", "kind": "function" |}function D() {}|]
const [|{| "name": "E", "kind": "const" |}E = [|{| "name": "F", "kind": "function" |}function F() {}|]|]
console.log(function() {}, class {}); // Expression with no name should have no effect.
console.log([|{| "name": "inner", "kind": "function" |}function inner() {}|]);
String([|{| "name": "nn", "kind": "function" |}function nn() {
	[|{| "name": "cls", "kind": "class", "containerName": "nn", "containerKind": "function" |}class cls {
		[|{| "name": "prop", "kind": "property", "kindModifiers": "public", "containerName": "cls", "containerKind": "class" |}public prop;|]
	}|]
}|]));"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    for range in f.ranges() {
        f.verify_workspace_symbol(&[workspace_symbol_case_from_range_with_pattern(
            &range,
            range_marker_data(&range)
                .data
                .get("name")
                .unwrap()
                .to_string(),
        )]);
    }
    done();
}
