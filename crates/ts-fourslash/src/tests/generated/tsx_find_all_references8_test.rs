#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_find_all_references8() {
    let mut t = TestingT;
    run_test_tsx_find_all_references8(&mut t);
}

fn run_test_tsx_find_all_references8(t: &mut TestingT) {
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
/*1*/declare function /*2*/MainButton(buttonProps: ButtonProps): JSX.Element;
/*3*/declare function /*4*/MainButton(linkProps: LinkProps): JSX.Element;
/*5*/declare function /*6*/MainButton(props: ButtonProps | LinkProps): JSX.Element;
let opt = /*7*/</*8*/MainButton />;
let opt = /*9*/</*10*/MainButton children="chidlren" />;
let opt = /*11*/</*12*/MainButton onClick={()=>{}} />;
let opt = /*13*/</*14*/MainButton onClick={()=>{}} ignore-prop />;
let opt = /*15*/</*16*/MainButton goTo="goTo" />;
let opt = /*17*/</*18*/MainButton wrong />;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
            "7".to_string(),
            "8".to_string(),
            "9".to_string(),
            "10".to_string(),
            "11".to_string(),
            "12".to_string(),
            "13".to_string(),
            "14".to_string(),
            "15".to_string(),
            "16".to_string(),
            "17".to_string(),
            "18".to_string(),
        ],
    );
    done();
}
