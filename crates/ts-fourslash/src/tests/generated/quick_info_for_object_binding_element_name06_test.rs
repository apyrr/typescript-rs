#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_object_binding_element_name06() {
    let mut t = TestingT;
    run_test_quick_info_for_object_binding_element_name06(&mut t);
}

fn run_test_quick_info_for_object_binding_element_name06(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoForObjectBindingElementName06") {
        return;
    }
    let content = r"type Foo = {
    /**
     * Thing is a bar
     */
    isBar: boolean

    /**
     * Thing is a baz
     */
    isBaz: boolean
}

function f(): Foo {
    return undefined as any
}

const { isBaz: isBar } = f();
isBar/**/;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
