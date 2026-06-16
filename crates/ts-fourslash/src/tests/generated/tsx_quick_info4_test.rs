#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_quick_info4() {
    let mut t = TestingT;
    run_test_tsx_quick_info4(&mut t);
}

fn run_test_tsx_quick_info4(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"//@Filename: file.tsx
// @jsx: preserve
// @noLib: true
export interface ClickableProps {
    children?: string;
    className?: string;
}
export interface ButtonProps extends ClickableProps {
    onClick(event?: React.MouseEvent<HTMLButtonElement>): void;
}
export interface LinkProps extends ClickableProps {
    to: string;
}
export function MainButton(buttonProps: ButtonProps): JSX.Element;
export function MainButton(linkProps: LinkProps): JSX.Element;
export function MainButton(props: ButtonProps | LinkProps): JSX.Element {
    const linkProps = props as LinkProps;
    if(linkProps.to) {
        return this._buildMainLink(props);
    }
    return this._buildMainButton(props);
}
function _buildMainButton({ onClick, children, className }: ButtonProps): JSX.Element {
    return(<button className={className} onClick={onClick}>{ children || 'MAIN BUTTON'}</button>);
}
declare function buildMainLink({ to, children, className }: LinkProps): JSX.Element;
function buildSomeElement1(): JSX.Element {
    return (
        <MainB/*1*/utton t/*2*/o='/some/path'>GO</MainButton>
    );
}
function buildSomeElement2(): JSX.Element {
    return (
        <MainB/*3*/utton onC/*4*/lick={()=>{}}>GO</MainButton>;
    );
}
let componenet = <MainButton onClick={()=>{}} ext/*5*/ra-prop>GO</MainButton>;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "1",
        "function MainButton(linkProps: LinkProps): JSX.Element (+1 overload)",
        "",
    );
    f.verify_quick_info_at(t, "2", "(property) LinkProps.to: string", "");
    f.verify_quick_info_at(
        t,
        "3",
        "function MainButton(buttonProps: ButtonProps): JSX.Element (+1 overload)",
        "",
    );
    f.verify_quick_info_at(
        t,
        "4",
        "(method) ButtonProps.onClick(event?: React.MouseEvent<HTMLButtonElement>): void",
        "",
    );
    f.verify_quick_info_at(t, "5", "(property) extra-prop: true", "");
    done();
}
