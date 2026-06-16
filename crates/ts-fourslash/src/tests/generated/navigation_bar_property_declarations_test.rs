#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_property_declarations() {
    let mut t = TestingT;
    run_test_navigation_bar_property_declarations(&mut t);
}

fn run_test_navigation_bar_property_declarations(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"class A {
    public A1 = class {
        public x = 1;
        private y() {}
        protected z() {}
    }

    public A2 = {
        x: 1,
        y() {},
        z() {}
    }

    public A3 = function () {}
    public A4 = () => {}
    public A5 = 1;
    public A6 = "A6";

    public ["A7"] = class {
        public x = 1;
        private y() {}
        protected z() {}
    }

    public [1] = {
        x: 1,
        y() {},
        z() {}
    }

    public [1 + 1] = 1;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
