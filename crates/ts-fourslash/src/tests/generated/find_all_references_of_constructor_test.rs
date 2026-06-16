#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_references_of_constructor() {
    let mut t = TestingT;
    run_test_find_all_references_of_constructor(&mut t);
}

fn run_test_find_all_references_of_constructor(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllReferencesOfConstructor") {
        return;
    }
    let content = r#"// @Filename: a.ts
export class C {
    /*0*/constructor(n: number);
    /*1*/constructor();
    /*2*/constructor(n?: number){}
    static f() {
        this.f();
        new this();
    }
}
new C();
const D = C;
new D();
// @Filename: b.ts
import { C } from "./a";
new C();
// @Filename: c.ts
import { C } from "./a";
class D extends C {
    constructor() {
        super();
        super.method();
    }
    method() { super(); }
}
class E implements C {
    constructor() { super(); }
}
// @Filename: d.ts
import * as a from "./a";
new a.C();
class d extends a.C { constructor() { super(); }"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["0".to_string(), "1".to_string(), "2".to_string()]);
    done();
}
