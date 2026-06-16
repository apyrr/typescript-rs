#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_circular_instantiation_expression() {
    let mut t = TestingT;
    run_test_quick_info_circular_instantiation_expression(&mut t);
}

fn run_test_quick_info_circular_instantiation_expression(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoCircularInstantiationExpression") {
        return;
    }
    let content = r#"declare function foo<T>(t: T): typeof foo<T>;
/**/foo("");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
