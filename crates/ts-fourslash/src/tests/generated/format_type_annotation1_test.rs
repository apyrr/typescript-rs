#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_type_annotation1() {
    let mut t = TestingT;
    run_test_format_type_annotation1(&mut t);
}

fn run_test_format_type_annotation1(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatTypeAnnotation1") {
        return;
    }
    let content = r"function foo(x: number, y?: string): number {}
interface Foo {
    x: number;
    y?: number;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings
            .insert_space_before_type_annotation = ts_core::TSTrue;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"function foo(x : number, y ?: string) : number { }
interface Foo {
    x : number;
    y ?: number;
}",
    );
    done();
}
