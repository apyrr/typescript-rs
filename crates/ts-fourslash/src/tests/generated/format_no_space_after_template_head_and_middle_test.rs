#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_no_space_after_template_head_and_middle() {
    let mut t = TestingT;
    run_test_format_no_space_after_template_head_and_middle(&mut t);
}

fn run_test_format_no_space_after_template_head_and_middle(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"const a1 = ` + "`" + `${ 1 }${ 1 }` + "`" + `;
const a2 = ` + "`" + `
    ${ 1 }${ 1 }
` + "`" + `;
const a3 = ` + "`" + `


    ${ 1 }${ 1 }
` + "`" + `;
const a4 = ` + "`" + `

    ${ 1 }${ 1 }

` + "`" + `;
const a5 = ` + "`" + `text ${ 1 } text ${ 1 } text` + "`" + `;
const a6 = ` + "`" + `
    text ${ 1 }
    text ${ 1 }
    text
` + "`" + `;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings
            .insert_space_after_opening_and_before_closing_template_string_braces =
            ts_core::TSFalse;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"const a1 = `${1}${1}`;
const a2 = `
    ${1}${1}
`;
const a3 = `


    ${1}${1}
`;
const a4 = `

    ${1}${1}

`;
const a5 = `text ${1} text ${1} text`;
const a6 = `
    text ${1}
    text ${1}
    text
`;",
    );
    done();
}
