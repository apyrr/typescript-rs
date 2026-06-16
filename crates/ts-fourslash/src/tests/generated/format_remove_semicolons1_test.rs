#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_remove_semicolons1() {
    let mut t = TestingT;
    run_test_format_remove_semicolons1(&mut t);
}

fn run_test_format_remove_semicolons1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"; (function f() { })();
const a = 3;
+ 4;
const b = 3
+ 4;
const c = 3 +
4;
class C {
    prop;
    ["p"];
    zero: void;
    ["one"] = {};
    ["two"];
    ;
}
a;
` + "`" + `b` + "`" + `;
b;
(3);
4;
    / regex /;
;
[];
/** blah */[0];
interface I {
    new;
    ();
    foo;
    ();
}
type T = {
    new;
    ();
    foo;
    ();
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings.semicolons = lsutil::SemicolonPreference::Remove;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r#"; (function f() { })()
const a = 3;
+ 4
const b = 3
    + 4
const c = 3 +
    4
class C {
    prop
    ["p"]
    zero: void
    ["one"] = {};
    ["two"]
    ;
}
a;
`b`
b;
(3)
4;
/ regex /
;
[];
/** blah */[0]
interface I {
    new;
    ()
    foo;
    ()
}
type T = {
    new;
    ()
    foo;
    ()
}"#,
    );
    done();
}
