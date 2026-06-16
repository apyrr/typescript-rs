#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_object_define_property_rename_locations() {
    let mut t = TestingT;
    run_test_js_object_define_property_rename_locations(&mut t);
}

fn run_test_js_object_define_property_rename_locations(t: &mut TestingT) {
    if should_skip_if_failing("TestJsObjectDefinePropertyRenameLocations") {
        return;
    }
    let content = r#"// @allowJs: true
// @checkJs: true
// @noEmit: true
// @Filename: index.js
var CircularList = (function () {
    var CircularList = function() {};
    Object.defineProperty(CircularList.prototype, "[|maxLength|]", { value: 0, writable: true });
    CircularList.prototype.push = function (value) {
        // ...
        this.[|maxLength|] + this.[|maxLength|]
    }
    return CircularList;
})()"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "");
    done();
}
