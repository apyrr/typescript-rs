#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_items_export_default_expression2() {
    let mut t = TestingT;
    run_test_navigation_items_export_default_expression2(&mut t);
}

fn run_test_navigation_items_export_default_expression2(t: &mut TestingT) {
    if should_skip_if_failing("TestNavigationItemsExportDefaultExpression2") {
        return;
    }
    let content = r"export const foo = {
  foo: {},
};

export default {
  foo: {},
};

export default {
  foo: {},
};

type Type = typeof foo;

export default {
  foo: {},
} as Type;

export default {
  foo: {},
} satisfies Type;

export default (class {
  prop = 42;
});

export default (class Cls {
  prop = 42;
});";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
