#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_decorator() {
    let mut t = TestingT;
    run_test_go_to_definition_decorator(&mut t);
}

fn run_test_go_to_definition_decorator(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionDecorator") {
        return;
    }
    let content = r#"// @Filename: b.ts
@[|/*decoratorUse*/decorator|]
class C {
    @[|decora/*decoratorFactoryUse*/torFactory|](a, "22", true)
    method() {}
}
// @Filename: a.ts
function /*decoratorDefinition*/decorator(target) {
    return target;
}
function /*decoratorFactoryDefinition*/decoratorFactory(...args) {
    return target => target;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "decoratorUse".to_string(),
            "decoratorFactoryUse".to_string(),
        ],
    );
    done();
}
