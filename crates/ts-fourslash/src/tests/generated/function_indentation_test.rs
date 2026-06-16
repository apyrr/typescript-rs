#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_function_indentation() {
    let mut t = TestingT;
    run_test_function_indentation(&mut t);
}

fn run_test_function_indentation(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"namespace M {
export =
C;
class C {
constructor(b
) {
}
foo(a
: string) {
return a
|| true;
}
get bar(
) {
return 1;
}
}
function foo(a,
b?) {
new M.C(
"hello");
}
{
{
}
}
foo(
function() {
"hello";
});
foo(
() => {
"hello";
});
var t,
u = 1,
v;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r#"namespace M {
    export =
        C;
    class C {
        constructor(b
        ) {
        }
        foo(a
            : string) {
            return a
                || true;
        }
        get bar(
        ) {
            return 1;
        }
    }
    function foo(a,
        b?) {
        new M.C(
            "hello");
    }
    {
        {
        }
    }
    foo(
        function() {
            "hello";
        });
    foo(
        () => {
            "hello";
        });
    var t,
        u = 1,
        v;
}"#,
    );
    done();
}
