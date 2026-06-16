use crate::{new_fourslash, skip_if_failing, TestingT};

pub fn test_quick_info_merged_alias(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @filename: /a.ts
/**
 * A function
 */
export function foo/*1*/() {}
// @filename: /b.ts
import { foo/*2*/ } from './a';
export { foo/*3*/ };

/**
 * A type
 */
type foo/*4*/ = number;

foo/*5*/()
let x1: foo/*6*/;
// @filename: /c.ts
import { foo/*7*/ } from './b';

/**
 * A namespace
 */
namespace foo/*8*/ {
    export type bar = string[];
}

foo/*9*/()
let x1: foo/*10*/;
let x2: foo/*11*/.bar;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}

