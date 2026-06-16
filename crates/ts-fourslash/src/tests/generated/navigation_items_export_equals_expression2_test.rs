#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_items_export_equals_expression2() {
    let mut t = TestingT;
    run_test_navigation_items_export_equals_expression2(&mut t);
}

fn run_test_navigation_items_export_equals_expression2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"export const foo = {
  foo: {},
};

export = {
  foo: {},
};

export = {
  foo: {},
};

type Type = typeof foo;

export = {
  foo: {},
} as Type;

export = {
  foo: {},
} satisfies Type;

export = (class {
  prop = 42;
});

export = (class Cls {
  prop = 42;
});";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
