#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_literal_type_in_union_or_intersection_type() {
    let mut t = TestingT;
    run_test_format_literal_type_in_union_or_intersection_type(&mut t);
}

fn run_test_format_literal_type_in_union_or_intersection_type(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatLiteralTypeInUnionOrIntersectionType") {
        return;
    }
    let content = r"type NumberAndString = {
    a: number
} & {
    b: string
};

type NumberOrString = {
    a: number
} | {
    b: string
};

type Complexed =
    Foo &
    Bar |
    Baz;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"type NumberAndString = {
    a: number
} & {
    b: string
};

type NumberOrString = {
    a: number
} | {
    b: string
};

type Complexed =
    Foo &
    Bar |
    Baz;",
    );
    done();
}
