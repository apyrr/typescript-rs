use crate::{new_fourslash, TestingT};

pub fn test_navigation_bar_function_prototype3(t: &mut TestingT) {
    let content = r#"// @allowJs: true
// @Filename: foo.js
var A; 
A.prototype.a = function() { };
A.b = function() { };"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}

