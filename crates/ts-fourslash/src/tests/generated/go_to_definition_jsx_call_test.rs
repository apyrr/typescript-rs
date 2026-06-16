#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_jsx_call() {
    let mut t = TestingT;
    run_test_go_to_definition_jsx_call(&mut t);
}

fn run_test_go_to_definition_jsx_call(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionJsxCall") {
        return;
    }
    let content = r"// @filename: ./test.tsx
interface FC<P = {}> {
    (props: P, context?: any): string;
}

const Thing: FC = (props) => <div></div>;
const HelloWorld = () => <[|/**/Thing|] />;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["".to_string()]);
    done();
}
