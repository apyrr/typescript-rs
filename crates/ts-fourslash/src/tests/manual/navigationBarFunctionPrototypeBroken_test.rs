use crate::{new_fourslash, TestingT};

pub fn test_navigation_bar_function_prototype_broken(t: &mut TestingT) {
    let content = r#"// @allowJs: true
// @Filename: foo.js
function A() {}
A. // Started typing something here
A.prototype.a = function() { };
G. // Started typing something here
A.prototype.a = function() { };"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}

