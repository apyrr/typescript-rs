#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_chained_callbacks() {
    let mut t = TestingT;
    run_test_formatting_on_chained_callbacks(&mut t);
}

fn run_test_formatting_on_chained_callbacks(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"Promise
    .resolve()
    .then(() => {/*1*/""/*2*/
}).then(() => {/*3*//*4*/
})/*semi1*//*semi2*/
function foo() {
    return Promise.resolve()
        .then(function () {
        ""/*a*/
    })/*b*/
}
Promise
    .then(
    /*n1*/
        )
    /*n2*/
    .then();
// @Filename: listSmart.ts
Promise
    .resolve().then(
    /*listSmart1*/
    3,
    /*listSmart2*/
    [
        3
        /*listSmart3*/
    ]
    /*listSmart4*/
    );
// @Filename: listZeroIndent.ts
Promise.resolve([
]).then(
    /*listZeroIndent1*/
    [
    /*listZeroIndent2*/
        3
    ]
    );
// @Filename: listTypeParameter1.ts
foo.then
    <
    /*listTypeParameter1*/
    void
    /*listTypeParameter2*/
    >(
    function (): void {
    },
    function (): void {
    }
    );
// @Filename: listComment.ts
Promise
    .then(
    // euphonium
    "k"
    // oboe
    );"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert_line(t, "");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "        \"\"");
    f.insert_line(t, "");
    f.verify_indentation(t, 8);
    f.go_to_marker(t, "4");
    f.insert_line(t, "");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "    }).then(() => {");
    f.go_to_marker(t, "semi1");
    f.insert(t, ";");
    f.verify_current_line_content(t, "    });");
    f.go_to_marker(t, "semi2");
    f.insert(t, ";");
    f.verify_current_line_content(t, "    });;");
    f.go_to_marker(t, "a");
    f.insert(t, ";");
    f.verify_current_line_content(t, "            \"\";");
    f.go_to_marker(t, "b");
    f.insert(t, ";");
    f.verify_current_line_content(t, "        });");
    f.go_to_marker(t, "n1");
    f.verify_indentation(t, 8);
    f.go_to_marker(t, "n2");
    f.verify_indentation(t, 4);
    f.go_to_file(t, "listSmart.ts");
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"Promise
    .resolve().then(

        3,

        [
            3

        ]

    );",
    );
    f.go_to_marker(t, "listSmart1");
    f.verify_indentation(t, 8);
    f.go_to_marker(t, "listSmart2");
    f.verify_indentation(t, 8);
    f.go_to_marker(t, "listSmart3");
    f.verify_indentation(t, 12);
    f.go_to_marker(t, "listSmart4");
    f.verify_indentation(t, 8);
    f.go_to_file(t, "listZeroIndent.ts");
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"Promise.resolve([
]).then(

    [

        3
    ]
);",
    );
    f.go_to_marker(t, "listZeroIndent1");
    f.verify_indentation(t, 4);
    f.go_to_marker(t, "listZeroIndent2");
    f.verify_indentation(t, 8);
    f.go_to_file(t, "listTypeParameter1.ts");
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"foo.then
    <

        void

    >(
        function(): void {
        },
        function(): void {
        }
    );",
    );
    f.go_to_marker(t, "listTypeParameter1");
    f.verify_indentation(t, 8);
    f.go_to_marker(t, "listTypeParameter2");
    f.verify_indentation(t, 8);
    f.go_to_file(t, "listComment.ts");
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r#"Promise
    .then(
        // euphonium
        "k"
        // oboe
    );"#,
    );
    done();
}
