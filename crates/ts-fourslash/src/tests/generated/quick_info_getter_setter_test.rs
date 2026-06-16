#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_getter_setter() {
    let mut t = TestingT;
    run_test_quick_info_getter_setter(&mut t);
}

fn run_test_quick_info_getter_setter(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @target: es2015
class C {
    #x = Promise.resolve("")
    set /*setterDef*/myValue(x: Promise<string> | string) {
        this.#x = Promise.resolve(x);
    }
    get /*getterDef*/myValue(): Promise<string> {
        return this.#x;
    }
}
let instance = new C();
instance./*setterUse*/myValue = instance./*getterUse*/myValue;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "getterUse", "(property) C.myValue: Promise<string>", "");
    f.verify_quick_info_at(t, "getterDef", "(getter) C.myValue: Promise<string>", "");
    f.verify_quick_info_at(
        t,
        "setterUse",
        "(property) C.myValue: string | Promise<string>",
        "",
    );
    f.verify_quick_info_at(
        t,
        "setterDef",
        "(setter) C.myValue: string | Promise<string>",
        "",
    );
    done();
}
