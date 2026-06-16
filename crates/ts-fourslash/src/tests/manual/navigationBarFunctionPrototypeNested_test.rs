use crate::{new_fourslash, TestingT};

pub fn test_navigation_bar_function_prototype_nested(t: &mut TestingT) {
    let content = r#"// @allowJs: true
// @Filename: foo.js
function A() {}
A.B = function () {  } 
A.B.prototype.d = function () {  }  
Object.defineProperty(A.B.prototype, "x", {
    get() {}
})
A.prototype.D = function () {  } 
A.prototype.D.prototype.d = function () {  } "#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}

