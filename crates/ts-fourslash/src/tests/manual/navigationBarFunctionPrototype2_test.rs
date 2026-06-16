use crate::{new_fourslash, TestingT};

pub fn test_navigation_bar_function_prototype2(t: &mut TestingT) {
    let content = r#"// @allowJs: true
// @Filename: foo.js
A.prototype.a = function() { };
A.prototype.b = function() { };
function A() {}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}

