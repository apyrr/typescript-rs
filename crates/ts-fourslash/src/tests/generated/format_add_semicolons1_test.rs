#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_add_semicolons1() {
    let mut t = TestingT;
    run_test_format_add_semicolons1(&mut t);
}

fn run_test_format_add_semicolons1(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatAddSemicolons1") {
        return;
    }
    let content = r#"console.log(1)
console.log(2)
const x = function() { }
for (let i = 0; i < 1; i++) {
    1
    2
}
do { } while (false) console.log(3)
function f() { }
class C {
    ["one"] = {}
    ["two"]
    three: string
    m() { }
    ;["three"] = {}
    ;["four"]
}
enum E {
    C
}
type M<T> = { [K in keyof T]: any }
declare module 'foo' { }
declare module 'bar'
type T = { x: string, y: number }"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings.semicolons = lsutil::SemicolonPreference::Insert;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r#"console.log(1);
console.log(2);
const x = function() { };
for (let i = 0; i < 1; i++) {
    1;
    2;
}
do { } while (false); console.log(3);
function f() { }
class C {
    ["one"] = {}
    ["two"];
    three: string;
    m() { }
    ;["three"] = {}
        ;["four"];
}
enum E {
    C
}
type M<T> = { [K in keyof T]: any };
declare module 'foo' { }
declare module 'bar';
type T = { x: string, y: number; };"#,
    );
    done();
}
