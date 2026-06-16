#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports45_decorators() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports45_decorators(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports45_decorators(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @isolatedDeclarations: true
// @declaration: true
// @Filename: /code.ts
function classDecorator<T extends Function> (value: T, context: ClassDecoratorContext) {}
function methodDecorator<This> (
  target: (...args: number[])=> number,
  context: ClassMethodDecoratorContext<This, (this: This, ...args: number[]) => number>) {}
function getterDecorator(value: Function, context: ClassGetterDecoratorContext) {}
function setterDecorator(value: Function, context: ClassSetterDecoratorContext) {}
function fieldDecorator(value: undefined, context: ClassFieldDecoratorContext) {}
function foo() { return 42;}

@classDecorator
export class A {
  @methodDecorator
  sum(...args: number[]) {
    return args.reduce((a, b) => a + b, 0);
  }
  getSelf() {
    return this;
  }
  @getterDecorator
  get a() {
    return foo();
  }
  @setterDecorator
  set a(value) {}

  @fieldDecorator classProp = foo();
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_all(t, VerifyCodeFixAllOptions {
    fix_id: "fixMissingTypeAnnotationOnExports".to_string(),
    new_file_content: r"function classDecorator<T extends Function> (value: T, context: ClassDecoratorContext) {}
function methodDecorator<This> (
  target: (...args: number[])=> number,
  context: ClassMethodDecoratorContext<This, (this: This, ...args: number[]) => number>) {}
function getterDecorator(value: Function, context: ClassGetterDecoratorContext) {}
function setterDecorator(value: Function, context: ClassSetterDecoratorContext) {}
function fieldDecorator(value: undefined, context: ClassFieldDecoratorContext) {}
function foo() { return 42;}

@classDecorator
export class A {
  @methodDecorator
  sum(...args: number[]): number {
    return args.reduce((a, b) => a + b, 0);
  }
  getSelf(): this {
    return this;
  }
  @getterDecorator
  get a(): number {
    return foo();
  }
  @setterDecorator
  set a(value) {}

  @fieldDecorator classProp: number = foo();
}".to_string(),
});
    done();
}
