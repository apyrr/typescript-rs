#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_rename9() {
    let mut t = TestingT;
    run_test_tsx_rename9(&mut t);
}

fn run_test_tsx_rename9(t: &mut TestingT) {
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
    [|[|{| "contextRangeIndex": 0 |}onClick|](event?: React.MouseEvent<HTMLButtonElement>): void;|]
}
interface LinkProps extends ClickableProps {
    [|[|{| "contextRangeIndex": 2 |}goTo|]: string;|]
}
[|declare function [|{| "contextRangeIndex": 4 |}MainButton|](buttonProps: ButtonProps): JSX.Element;|]
[|declare function [|{| "contextRangeIndex": 6 |}MainButton|](linkProps: LinkProps): JSX.Element;|]
[|declare function [|{| "contextRangeIndex": 8 |}MainButton|](props: ButtonProps | LinkProps): JSX.Element;|]
let opt = [|<[|{| "contextRangeIndex": 10 |}MainButton|] />|];
let opt = [|<[|{| "contextRangeIndex": 12 |}MainButton|] children="chidlren" />|];
let opt = [|<[|{| "contextRangeIndex": 14 |}MainButton|] [|[|{| "contextRangeIndex": 16 |}onClick|]={()=>{}}|] />|];
let opt = [|<[|{| "contextRangeIndex": 18 |}MainButton|] [|[|{| "contextRangeIndex": 20 |}onClick|]={()=>{}}|] [|ignore-prop|] />|];
let opt = [|<[|{| "contextRangeIndex": 23 |}MainButton|] [|[|{| "contextRangeIndex": 25 |}goTo|]="goTo"|] />|];
let opt = [|<[|{| "contextRangeIndex": 27 |}MainButton|] [|wrong|] />|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "onClick");
    done();
}
