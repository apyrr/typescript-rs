#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_multiple_definitions() {
    let mut t = TestingT;
    run_test_go_to_definition_multiple_definitions(&mut t);
}

fn run_test_go_to_definition_multiple_definitions(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: a.ts
interface /*interfaceDefinition1*/IFoo {
    instance1: number;
}
// @Filename: b.ts
interface /*interfaceDefinition2*/IFoo {
    instance2: number;
}

interface /*interfaceDefinition3*/IFoo {
    instance3: number;
}

var ifoo: [|IFo/*interfaceReference*/o|];
// @Filename: c.ts
module /*moduleDefinition1*/Module {
    export class c1 { }
}
// @Filename: d.ts
module /*moduleDefinition2*/Module {
    export class c2 { }
}
// @Filename: e.ts
[|Modul/*moduleReference*/e|];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "interfaceReference".to_string(),
            "moduleReference".to_string(),
        ],
    );
    done();
}
