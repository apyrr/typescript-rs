#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsx_generic_quick_info() {
    let mut t = TestingT;
    run_test_jsx_generic_quick_info(&mut t);
}

fn run_test_jsx_generic_quick_info(t: &mut TestingT) {
    if should_skip_if_failing("TestJsxGenericQuickInfo") {
        return;
    }
    let content = r#"//@Filename: file.tsx
declare namespace JSX {
    interface Element { }
    interface IntrinsicElements {
    }
    interface ElementAttributesProperty { props }
}
interface PropsA<T> {
    /** comments for A */
    name: 'A',
    items: T[];
    renderItem: (item: T) => string;
}
interface PropsB<T> {
    /** comments for B */
    name: 'B',
    items: T[];
    renderItem: (item: T) => string;
}
class Component<T> {
    constructor(props: PropsA<T> | PropsB<T>) {}
    props: PropsA<T> | PropsB<T>;
}   
var b = new Component({items: [0, 1, 2], render/*0*/Item: it/*1*/em => item.toFixed(), name/*2*/: 'A',});
var c = <Component items={[0, 1, 2]} render/*3*/Item={it/*4*/em => item.toFixed()} name/*5*/="A" />"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "0",
        "(property) PropsA<number>.renderItem: (item: number) => string",
        "",
    );
    f.verify_quick_info_at(t, "1", "(parameter) item: number", "");
    f.verify_quick_info_at(t, "2", "(property) PropsA<T>.name: \"A\"", "comments for A");
    f.verify_quick_info_at(
        t,
        "3",
        "(property) PropsA<number>.renderItem: (item: number) => string",
        "",
    );
    f.verify_quick_info_at(t, "4", "(parameter) item: number", "");
    f.verify_quick_info_at(t, "5", "(property) PropsA<T>.name: \"A\"", "comments for A");
    done();
}
