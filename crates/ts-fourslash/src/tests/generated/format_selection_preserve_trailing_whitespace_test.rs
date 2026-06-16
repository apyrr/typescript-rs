#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_selection_preserve_trailing_whitespace() {
    let mut t = TestingT;
    run_test_format_selection_preserve_trailing_whitespace(&mut t);
}

fn run_test_format_selection_preserve_trailing_whitespace(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"
/*begin*/;    
    
/*end*/    
    
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings.trim_trailing_whitespace = ts_core::TSFalse;
        f.configure(t, opts);
    }
    f.format_selection(t, "begin", "end");
    f.verify_current_file_content(
        t,
        r"
;    
    
    
    
",
    );
    done();
}
