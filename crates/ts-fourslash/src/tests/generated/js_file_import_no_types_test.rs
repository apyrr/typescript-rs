#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_file_import_no_types() {
    let mut t = TestingT;
    run_test_js_file_import_no_types(&mut t);
}

fn run_test_js_file_import_no_types(t: &mut TestingT) {
    if should_skip_if_failing("TestJsFileImportNoTypes") {
        return;
    }
    let content = r"// @allowJs: true
// @filename: /declarations.ts
 export class TestClass {}
 export const testValue = {};
 export enum TestEnum {}
 export function testFunction() {}
 export interface testInterface {}
 export namespace TestNamespaceEmpty {}
 export namespace TestNamespaceWithType {
   export type testTypeInner = boolean;
 }
 export namespace TestNamespaceWithValue {
   export const testValueInner = true;
 }
 export type testType = {};

 export interface TestInterfaceMerged {}
 export interface TestInterfaceMerged {}

 export interface TestClassInterfaceMerged {}
 export class TestClassInterfaceMerged {}

 export declare const declaredVariable: number;
 export declare class DeclaredClass {}
 export declare interface DeclaredInterface {}
 export declare type DeclaredType = {};
// @filename: /a.js
import { /**/ } from './declarations.ts'";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
