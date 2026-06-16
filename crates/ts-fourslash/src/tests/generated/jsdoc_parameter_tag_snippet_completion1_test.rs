#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_parameter_tag_snippet_completion1() {
    let mut t = TestingT;
    run_test_jsdoc_parameter_tag_snippet_completion1(&mut t);
}

fn run_test_jsdoc_parameter_tag_snippet_completion1(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocParameterTagSnippetCompletion1") {
        return;
    }
    let content = r#"// @allowJs: true
// @Filename: a.ts
 /**
  * @para/*0*/
  */
 function printValue(value, maximumFractionDigits) {}

 /**
  * @p/*a*/
  */
 function aa({ a = 1 }, b: string) {
     a;
 }

 /**
  * /*b*/
  */
 function bb(b: string) {}

 /**
  * 
  * @p/*c*/
  */
 function cc({ b: { a, c } = { a: 1, c: 3 } }) {

 }

 /**
  * 
  * @p/*d*/
  */
 function dd({ a: { b, c }, d: [e, f] }: { a: { b: number, c: number }, d: [string, string] }) {

 }
// @Filename: b.js
 /**
  * @p/*ja*/
  */
 function aa({ a = 1 }, b) {
     a;
 }

 /**
  * /*jb*/
  */
 function bb(b) {}

 /**
  * 
  * @p/*jc*/
  */
 function cc({ b: { a, c } = { a: 1, c: 3 } }) {

 }

 /**
  * 
  * @p/*jd*/
  */
 function dd({ a: { b, c }, d: [e, f] }) {

 }

 const someconst = "aa";
 /**
  * 
  * @p/*je*/
  */
 function ee({ [someconst]: b }) {

 }

 /**
  * 
  * @p/*jf*/
  */
 function ff({ "a": b }) {

 }

 /**
  * 
  * @p/*jg*/
  */
 function gg(a, { b }) {

 }

 /**
  * 
  * @param {boolean} a a's description
  * @p/*jh*/
  */
 function hh(a, { b }) {
    
 }
 /**
  * 
  * @p/*ji*/
  */
 function ii({ b, ...c }, ...a) {}

 /**
  * 
  * @p/*jj*/
  */
 function jj(...{ length }) {}

 /**
  * 
  * @p/*jk*/
  */
 function kk(...a) {}

 function reallylongfunctionnameabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijkl(a) {}
 /**
  *
  * @p/*jl*/
  */
 function ll(a = reallylongfunctionnameabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijkl("")) {}
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
