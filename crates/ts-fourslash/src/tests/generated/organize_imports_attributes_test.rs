#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports_attributes() {
    let mut t = TestingT;
    run_test_organize_imports_attributes(&mut t);
}

fn run_test_organize_imports_attributes(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"import { A } from "./file";
import { type B } from "./file";
import { C } from "./file" with { type: "a" };
import { A as D } from "./file" with { type: "b" };
import { E } from "./file" with { type: "a" };
import { A as F } from "./file" with { type: "b" };

type G = A | B | C | D | E | F;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r#"import { A, type B } from "./file";
import { C, E } from "./file" with { type: "a" };
import { A as D, A as F } from "./file" with { type: "b" };

type G = A | B | C | D | E | F;"#,
        "source.organizeImports",
        None,
    );
    done();
}
