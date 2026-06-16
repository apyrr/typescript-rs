use crate::{new_fourslash, TestingT};

pub fn test_hover_qualified_generic_names(t: &mut TestingT) {
    let content = r#"
function f<T>(x: T) {
    class C {
        value = x
    }
    return new C()
}

class A<T> {
    foo() {}
}
class B extends A<string> {}

let t1/*1*/ = f("hello")
const t2/*2*/ = new B()
t2./*3*/foo()
"#;

    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    f.verify_quick_info_at(t, "1", "let t1: f<string>.C", "");
    f.verify_quick_info_at(t, "2", "const t2: B", "");
    f.verify_quick_info_at(t, "3", "(method) A<string>.foo(): void", "");
    done();
}

