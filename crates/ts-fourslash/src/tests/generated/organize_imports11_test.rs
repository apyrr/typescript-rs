#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports11() {
    let mut t = TestingT;
    run_test_organize_imports11(&mut t);
}

fn run_test_organize_imports11(t: &mut TestingT) {
    if should_skip_if_failing("TestOrganizeImports11") {
        return;
    }
    let content = r"// @Filename: /test.ts
import { TypeA, TypeB, TypeC, UnreferencedType } from './my-types';

/**
 * MyClass {@link TypeA}
 */
export class MyClass {

  /**
   * Some Property {@link TypeB}
   */
  public something;

  /**
   * Some function {@link TypeC}
   */
  public myMethod() {

    /**
     * Some lambda function {@link TypeC}
     */
    const someFunction = () => {
      return '';
    }
    someFunction();
  }
}
// @Filename: /my-types.ts
 export type TypeA = string;
 export class TypeB { }
 export type TypeC = () => string;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r"import { TypeA, TypeB, TypeC } from './my-types';

/**
 * MyClass {@link TypeA}
 */
export class MyClass {

  /**
   * Some Property {@link TypeB}
   */
  public something;

  /**
   * Some function {@link TypeC}
   */
  public myMethod() {

    /**
     * Some lambda function {@link TypeC}
     */
    const someFunction = () => {
      return '';
    }
    someFunction();
  }
}",
        "source.organizeImports",
        None,
    );
    done();
}
