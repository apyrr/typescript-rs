#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_completion_in_function_expression_of_children_callback() {
    let mut t = TestingT;
    run_test_tsx_completion_in_function_expression_of_children_callback(&mut t);
}

fn run_test_tsx_completion_in_function_expression_of_children_callback(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"//@module: commonjs
//@jsx: preserve
// @Filename: 1.tsx
declare namespace JSX {
    interface Element { }
    interface IntrinsicElements {
    }
    interface ElementAttributesProperty { props; }
}
interface IUser {
    Name: string;
}
interface IFetchUserProps {
    children: (user: IUser) => any;
}
function FetchUser(props: IFetchUserProps) { return undefined; }
function UserName() {
    return (
        <FetchUser>
            { user => (
                <h1>{ user./**/ }</h1>
            )}
        </FetchUser>
    );
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Name("".to_string()), None);
    done();
}
