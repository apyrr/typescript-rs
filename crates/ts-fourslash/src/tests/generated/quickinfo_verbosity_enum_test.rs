#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity_enum() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity_enum(&mut t);
}

fn run_test_quickinfo_verbosity_enum(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @filename: a.ts
export {};
enum Color/*c*/ {
    Red,
    Green,
    Blue,
}
const x/*x*/: Color = Color.Red;
const enum Direction/*d*/ {
    Up,
    Down,
}
const y/*y*/: Direction = Direction.Up;
enum Flags/*f*/ {
    None = 0,
    IsDirectory = 1 << 0,
    IsFile = 1 << 1,
    IsSymlink = 1 << 2,
}
// @filename: b.ts
export enum Color {
    Red = "red"
}
// @filename: c.ts
import { Color } from "./b";
const c: Color/*a*/ = Color.Red;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([
            ("c".to_string(), vec![0, 1]),
            ("x".to_string(), vec![0, 1]),
            ("d".to_string(), vec![0, 1]),
            ("y".to_string(), vec![0, 1]),
            ("f".to_string(), vec![0, 1]),
            ("a".to_string(), vec![0, 1]),
        ]),
    );
    done();
}
