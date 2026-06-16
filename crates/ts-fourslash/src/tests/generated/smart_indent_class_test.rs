#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_indent_class() {
    let mut t = TestingT;
    run_test_smart_indent_class(&mut t);
}

fn run_test_smart_indent_class(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"class Bar {
    {| "indentation": 4|}
    private foo: string = "";
    {| "indentation": 4|}
    private f() {
        var a: any[] = [[1, 2], [3, 4], 5];
        {| "indentation": 8|}
        return ((1 + 1));
    }
    {| "indentation": 4|}
    private f2() {
        if (true) { } { };
    }
}
{| "indentation": 0|}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_indentation_at_markers_from_data(t);
    done();
}
