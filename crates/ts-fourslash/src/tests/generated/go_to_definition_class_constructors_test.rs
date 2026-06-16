#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_class_constructors() {
    let mut t = TestingT;
    run_test_go_to_definition_class_constructors(&mut t);
}

fn run_test_go_to_definition_class_constructors(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @filename: definitions.ts
export class Base {
    constructor(protected readonly cArg: string) {}
}

export class Derived extends Base {
    readonly email = this.cArg.getByLabel('Email')
    readonly password =  this.cArg.getByLabel('Password')
}
// @filename: main.ts
import { Derived } from './definitions'
const derived = new [|/*Derived*/Derived|](cArg)
// @filename: defInSameFile.ts
import { Base } from './definitions'
class SameFile extends Base {
    readonly name: string = 'SameFile'
}
const SameFile = new [|/*SameFile*/SameFile|](cArg)
const wrapper = new [|/*Base*/Base|](cArg)
// @filename: hasConstructor.ts
import { Base } from './definitions'
class HasConstructor extends Base {
    constructor() {}
    readonly name: string = '';
}
const hasConstructor = new [|/*HasConstructor*/HasConstructor|](cArg)";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "Derived".to_string(),
            "SameFile".to_string(),
            "HasConstructor".to_string(),
            "Base".to_string(),
        ],
    );
    done();
}
