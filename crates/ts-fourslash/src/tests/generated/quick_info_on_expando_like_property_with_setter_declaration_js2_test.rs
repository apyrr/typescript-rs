#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_expando_like_property_with_setter_declaration_js2() {
    let mut t = TestingT;
    run_test_quick_info_on_expando_like_property_with_setter_declaration_js2(&mut t);
}

fn run_test_quick_info_on_expando_like_property_with_setter_declaration_js2(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoOnExpandoLikePropertyWithSetterDeclarationJs2") {
        return;
    }
    let content = r#"// @strict: true
// @checkJs: true
// @filename: index.js
const obj = {};
let val = 10;
Object.defineProperty(obj, "a", {
  configurable: true,
  enumerable: true,
  set(v) {
    val = v;
  },
});

obj.a/**/ = 100;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "(property) obj.a: any", "");
    done();
}
