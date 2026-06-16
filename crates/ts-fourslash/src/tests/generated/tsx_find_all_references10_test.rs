#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_find_all_references10() {
    let mut t = TestingT;
    run_test_tsx_find_all_references10(&mut t);
}

fn run_test_tsx_find_all_references10(t: &mut TestingT) {
    if should_skip_if_failing("TestTsxFindAllReferences10") {
        return;
    }
    let content = r#"//@Filename: file.tsx
// @jsx: preserve
// @noLib: true
declare namespace JSX {
    interface Element { }
    interface IntrinsicElements {
    }
    interface ElementAttributesProperty { props; }
}
interface ClickableProps {
    children?: string;
    className?: string;
}
interface ButtonProps extends ClickableProps {
    /*1*/onClick(event?: React.MouseEvent<HTMLButtonElement>): void;
}
interface LinkProps extends ClickableProps {
    goTo: string;
}
declare function MainButton(buttonProps: ButtonProps): JSX.Element;
declare function MainButton(linkProps: LinkProps): JSX.Element;
declare function MainButton(props: ButtonProps | LinkProps): JSX.Element;
let opt = <MainButton />;
let opt = <MainButton children="chidlren" />;
let opt = <MainButton onClick={()=>{}} />;
let opt = <MainButton onClick={()=>{}} ignore-prop />;
let opt = <MainButton goTo="goTo" />;
let opt = <MainButton wrong />;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
