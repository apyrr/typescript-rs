use crate::{new_fourslash, TestingT};

pub fn test_navigation_bar_function_prototype_interlaced(t: &mut TestingT) {
    let content = r#"// @allowJs: true
// @Filename: foo.js
var b = 1;
function A() {}; 
A.prototype.a = function() { };
A.b = function() { };
b = 2
/* Comment */
A.prototype.c = function() { }
var b = 2
A.prototype.d = function() { }"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}

