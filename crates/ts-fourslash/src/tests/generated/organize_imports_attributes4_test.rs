#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports_attributes4() {
    let mut t = TestingT;
    run_test_organize_imports_attributes4(&mut t);
}

fn run_test_organize_imports_attributes4(t: &mut TestingT) {
    if should_skip_if_failing("TestOrganizeImportsAttributes4") {
        return;
    }
    let content = r#"import { A } from "./a" with { foo: "foo", bar: "bar" };
import { B } from "./a" with { bar: "bar", foo: "foo" };
import { D } from "./a" with { bar: "foo", foo: "bar" };
import { E } from "./a" with { foo: 'bar', bar: "foo" };
import { C } from "./a" with { foo: "bar", bar: "foo" };
import { F } from "./a" with { foo: "42" };
import { Y } from "./a" with { foo: 42 };
import { Z } from "./a" with { foo: "42" };

export type G = A | B | C | D | E | F | Y | Z;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r#"import { A, B } from "./a" with { foo: "foo", bar: "bar" };
import { C, D, E } from "./a" with { bar: "foo", foo: "bar" };
import { F, Z } from "./a" with { foo: "42" };
import { Y } from "./a" with { foo: 42 };

export type G = A | B | C | D | E | F | Y | Z;"#,
        "source.organizeImports",
        None,
    );
    done();
}
