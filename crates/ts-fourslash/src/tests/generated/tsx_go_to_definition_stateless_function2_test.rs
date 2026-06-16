#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_go_to_definition_stateless_function2() {
    let mut t = TestingT;
    run_test_tsx_go_to_definition_stateless_function2(&mut t);
}

fn run_test_tsx_go_to_definition_stateless_function2(t: &mut TestingT) {
    skip_if_failing(t);
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
    onClick(event?: React.MouseEvent<HTMLButtonElement>): void;
}
interface LinkProps extends ClickableProps {
    goTo: string;
}
declare function /*firstSource*/MainButton(buttonProps: ButtonProps): JSX.Element;
declare function /*secondSource*/MainButton(linkProps: LinkProps): JSX.Element;
declare function /*thirdSource*/MainButton(props: ButtonProps | LinkProps): JSX.Element;
let opt = <[|Main/*firstTarget*/Button|] />;
let opt = <[|Main/*secondTarget*/Button|] children="chidlren" />;
let opt = <[|Main/*thirdTarget*/Button|] onClick={()=>{}} />;
let opt = <[|Main/*fourthTarget*/Button|] onClick={()=>{}} ignore-prop />;
let opt = <[|Main/*fifthTarget*/Button|] goTo="goTo" />;
let opt = <[|Main/*sixthTarget*/Button|] wrong />;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "firstTarget".to_string(),
            "secondTarget".to_string(),
            "thirdTarget".to_string(),
            "fourthTarget".to_string(),
            "fifthTarget".to_string(),
            "sixthTarget".to_string(),
        ],
    );
    done();
}
