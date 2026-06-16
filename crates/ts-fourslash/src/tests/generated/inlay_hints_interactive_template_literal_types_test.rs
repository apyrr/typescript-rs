#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_interactive_template_literal_types() {
    let mut t = TestingT;
    run_test_inlay_hints_interactive_template_literal_types(&mut t);
}

fn run_test_inlay_hints_interactive_template_literal_types(t: &mut TestingT) {
    if should_skip_if_failing("TestInlayHintsInteractiveTemplateLiteralTypes") {
        return;
    }
    let content = r"declare function getTemplateLiteral1(): `${string},${string}`;
const lit1 = getTemplateLiteral1();
declare function getTemplateLiteral2(): `\${${string},${string}`;
const lit2 = getTemplateLiteral2();
declare function getTemplateLiteral3(): `start${string}\${,$${string}end`;
const lit3 = getTemplateLiteral3();
declare function getTemplateLiteral4(): `${string}\`,${string}`;
const lit4 = getTemplateLiteral4();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints(t);
    done();
}
