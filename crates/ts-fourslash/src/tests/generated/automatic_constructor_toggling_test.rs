#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_automatic_constructor_toggling() {
    let mut t = TestingT;
    run_test_automatic_constructor_toggling(&mut t);
}

fn run_test_automatic_constructor_toggling(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"class A<T> { }
class B<T> {/*B*/ }
class C<T> { /*C*/constructor(val: T) { } }
class D<T> { constructor(/*D*/val: T) { } }

new /*Asig*/A<string>();
new /*Bsig*/B("");
new /*Csig*/C("");
new /*Dsig*/D<string>();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "B");
    f.insert(t, "constructor(val: T) { }");
    f.verify_quick_info_at(t, "Asig", "constructor A<string>(): A<string>", "");
    f.verify_quick_info_at(
        t,
        "Bsig",
        "constructor B<string>(val: string): B<string>",
        "",
    );
    f.verify_quick_info_at(
        t,
        "Csig",
        "constructor C<string>(val: string): C<string>",
        "",
    );
    f.verify_quick_info_at(
        t,
        "Dsig",
        "constructor D<string>(val: string): D<string>",
        "",
    );
    f.go_to_marker(t, "C");
    f.delete_at_caret(t, 23);
    f.verify_quick_info_at(t, "Asig", "constructor A<string>(): A<string>", "");
    f.verify_quick_info_at(
        t,
        "Bsig",
        "constructor B<string>(val: string): B<string>",
        "",
    );
    f.verify_quick_info_at(t, "Csig", "constructor C<unknown>(): C<unknown>", "");
    f.verify_quick_info_at(
        t,
        "Dsig",
        "constructor D<string>(val: string): D<string>",
        "",
    );
    f.go_to_marker(t, "D");
    f.delete_at_caret(t, 6);
    f.verify_quick_info_at(t, "Asig", "constructor A<string>(): A<string>", "");
    f.verify_quick_info_at(
        t,
        "Bsig",
        "constructor B<string>(val: string): B<string>",
        "",
    );
    f.verify_quick_info_at(t, "Csig", "constructor C<unknown>(): C<unknown>", "");
    f.verify_quick_info_at(t, "Dsig", "constructor D<string>(): D<string>", "");
    done();
}
