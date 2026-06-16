#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_remove_semicolons2() {
    let mut t = TestingT;
    run_test_format_remove_semicolons2(&mut t);
}

fn run_test_format_remove_semicolons2(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatRemoveSemicolons2") {
        return;
    }
    let content = r"namespace ts {
    let x = 0;
    //
    interface I {
        a: string;
        /** @internal */
        b: string;
    }
    let y = 0; //
}
let z = 0; //";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings.semicolons = lsutil::SemicolonPreference::Remove;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"namespace ts {
    let x = 0
    //
    interface I {
        a: string
        /** @internal */
        b: string
    }
    let y = 0 //
}
let z = 0 //",
    );
    done();
}
