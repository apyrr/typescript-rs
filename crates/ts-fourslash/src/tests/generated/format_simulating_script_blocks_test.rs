#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_simulating_script_blocks() {
    let mut t = TestingT;
    run_test_format_simulating_script_blocks(&mut t);
}

fn run_test_format_simulating_script_blocks(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"/* BEGIN EXTERNAL SOURCE */
/*begin5*/
                        var a = 1;
                        alert("/*end5*//********//*begin4*/");
                    /*end4*/
/* END EXTERNAL SOURCE */

/* BEGIN EXTERNAL SOURCE */
/*begin3*/
                            var b = 1;

                        var c = "/*end3*//********//*begin2*/";
       var d = 1;

            var e = "/*end2*//********//*begin1*/";
            var f = 1;
        /*end1*/
/* END EXTERNAL SOURCE */"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings.base_indent_size = 12;
        f.configure(t, opts);
    }
    f.format_selection(t, "begin1", "end1");
    f.format_selection(t, "begin2", "end2");
    f.format_selection(t, "begin3", "end3");
    {
        let mut opts = f.get_options();
        opts.format_code_settings.base_indent_size = 24;
        f.configure(t, opts);
    }
    f.format_selection(t, "begin4", "end4");
    f.format_selection(t, "begin5", "end5");
    f.verify_current_file_content(
        t,
        r#"/* BEGIN EXTERNAL SOURCE */

                        var a = 1;
                        alert("/********/");

/* END EXTERNAL SOURCE */

/* BEGIN EXTERNAL SOURCE */

            var b = 1;

            var c = "/********/";
            var d = 1;

            var e = "/********/";
            var f = 1;

/* END EXTERNAL SOURCE */"#,
    );
    done();
}
