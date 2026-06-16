#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_template_tag() {
    let mut t = TestingT;
    run_test_quick_info_template_tag(&mut t);
}

fn run_test_quick_info_template_tag(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoTemplateTag") {
        return;
    }
    let content = r"// @allowJs: true
// @checkJs: true
// @Filename: /foo.js
/**
 * Doc
 * @template {new (...args: any[]) => any} T
 * @param {T} cls
 */
function /**/myMixin(cls) {
    return class extends cls {}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "function myMixin<T extends new (...args: any[]) => any>(cls: T): {\n    new (...args: any[]): (Anonymous class);\n    prototype: myMixin<any>.(Anonymous class);\n} & T", "Doc");
    done();
}
