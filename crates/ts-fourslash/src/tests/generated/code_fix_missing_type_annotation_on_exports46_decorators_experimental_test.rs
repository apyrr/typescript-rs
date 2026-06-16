#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports46_decorators_experimental() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports46_decorators_experimental(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports46_decorators_experimental(
    t: &mut TestingT,
) {
    skip_if_failing(t);
    let content = r"// @isolatedDeclarations: true
// @declaration: true
// @experimentalDecorators: true
// @Filename: /code.ts
function classDecorator<T extends Function>() { return (target: T) => target; }
function methodDecorator() { return (target: any, key: string, descriptor: PropertyDescriptor) => descriptor;}
function parameterDecorator() { return (target: any, key: string, idx: number) => {};}
function getterDecorator() { return (target: any, key: string) => {}; }
function setterDecorator() { return (target: any, key: string) => {}; }
function fieldDecorator()  { return (target: any, key: string) => {}; }
function foo() { return 42; }

@classDecorator()
export class A {
  @methodDecorator()
  sum(...args: number[]) {
    return args.reduce((a, b) => a + b, 0);
  }
  getSelf() {
    return this;
  }
  passParameter(@parameterDecorator() param = foo()) {}
  @getterDecorator()
  get a() {
    return foo();
  }
  @setterDecorator()
  set a(value) {}
  @fieldDecorator() classProp = foo();
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_all(t, VerifyCodeFixAllOptions {
    fix_id: "fixMissingTypeAnnotationOnExports".to_string(),
    new_file_content: r"function classDecorator<T extends Function>() { return (target: T) => target; }
function methodDecorator() { return (target: any, key: string, descriptor: PropertyDescriptor) => descriptor;}
function parameterDecorator() { return (target: any, key: string, idx: number) => {};}
function getterDecorator() { return (target: any, key: string) => {}; }
function setterDecorator() { return (target: any, key: string) => {}; }
function fieldDecorator()  { return (target: any, key: string) => {}; }
function foo() { return 42; }

@classDecorator()
export class A {
  @methodDecorator()
  sum(...args: number[]): number {
    return args.reduce((a, b) => a + b, 0);
  }
  getSelf(): this {
    return this;
  }
  passParameter(@parameterDecorator() param: number = foo()): void {}
  @getterDecorator()
  get a(): number {
    return foo();
  }
  @setterDecorator()
  set a(value) {}
  @fieldDecorator() classProp: number = foo();
}".to_string(),
});
    done();
}
