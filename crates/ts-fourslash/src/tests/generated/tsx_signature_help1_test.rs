#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_signature_help1() {
    let mut t = TestingT;
    run_test_tsx_signature_help1(&mut t);
}

fn run_test_tsx_signature_help1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @jsx: preserve
//@Filename: file.tsx
import React = require('react');
export interface ClickableProps {
    children?: string;
    className?: string;
}
export interface ButtonProps extends ClickableProps {
    onClick(event?: React.MouseEvent<HTMLButtonElement>): void;
}
function _buildMainButton({ onClick, children, className }: ButtonProps): JSX.Element {
    return(<button className={className} onClick={onClick}>{ children || 'MAIN BUTTON'}</button>);
}
export function MainButton(props: ButtonProps): JSX.Element {
    return this._buildMainButton(props);
}
let e1 = <MainButton/*1*/ /*2*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("MainButton(props: ButtonProps): JSX.Element".to_string()),
            parameter_name: None,
            parameter_span: Some("props: ButtonProps".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("MainButton(props: ButtonProps): JSX.Element".to_string()),
            parameter_name: None,
            parameter_span: Some("props: ButtonProps".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
