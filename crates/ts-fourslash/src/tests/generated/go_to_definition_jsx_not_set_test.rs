#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_jsx_not_set() {
    let mut t = TestingT;
    run_test_go_to_definition_jsx_not_set(&mut t);
}

fn run_test_go_to_definition_jsx_not_set(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionJsxNotSet") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: /foo.jsx
const /*def*/Foo = () => (
    <div>foo</div>
);
export default Foo;
// @Filename: /bar.jsx
import Foo from './foo';
const a = <[|/*use*/Foo|] />";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["use".to_string()]);
    done();
}
