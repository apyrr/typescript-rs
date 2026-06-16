#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_goto_definition_link_tag1() {
    let mut t = TestingT;
    run_test_goto_definition_link_tag1(&mut t);
}

fn run_test_goto_definition_link_tag1(t: &mut TestingT) {
    if should_skip_if_failing("TestGotoDefinitionLinkTag1") {
        return;
    }
    let content = r#"// @Filename: foo.ts
interface [|/*def1*/Foo|] {
    foo: string
}
namespace NS {
    export interface [|/*def2*/Bar|] {
        baz: Foo
    }
}
/** {@link /*use1*/[|Foo|]} foooo*/
const a = ""
/** {@link NS./*use2*/[|Bar|]} ns.bar*/
const b = ""
/** {@link /*use3*/[|Foo|] f1}*/
const c = ""
/** {@link NS./*use4*/[|Bar|] ns.bar}*/
const [|/*def3*/d|] = ""
/** {@link /*use5*/[|d|] }dd*/
const e = ""
/** @param x {@link /*use6*/[|Foo|]} */
function foo(x) { }
// @Filename: bar.ts
/** {@link /*use7*/[|Foo|] }dd*/
const f = """#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "use1".to_string(),
            "use2".to_string(),
            "use3".to_string(),
            "use4".to_string(),
            "use5".to_string(),
            "use6".to_string(),
            "use7".to_string(),
        ],
    );
    done();
}
