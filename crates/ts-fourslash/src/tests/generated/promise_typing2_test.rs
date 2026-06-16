#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_promise_typing2() {
    let mut t = TestingT;
    run_test_promise_typing2(&mut t);
}

fn run_test_promise_typing2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface IPromise<T> {
    then<U>(success?: (value: T) => IPromise<U>, error?: (error: any) => IPromise<U>, progress?: (progress: any) => void ): IPromise<U>;
    then<U>(success?: (value: T) => IPromise<U>, error?: (error: any) => U, progress?: (progress: any) => void ): IPromise<U>;
    then<U>(success?: (value: T) => U, error?: (error: any) => IPromise<U>, progress?: (progress: any) => void ): IPromise<U>;
    then<U>(success?: (value: T) => U, error?: (error: any) => U, progress?: (progress: any) => void ): IPromise<U>;
    done? <U>(success?: (value: T) => any, error?: (error: any) => any, progress?: (progress: any) => void ): void;
}
var p1: IPromise<number> = null;
p/*1*/1.then(function (x/*2*/x) { }); 
var p/*3*/2 = p1.then(function (x/*4*/x) { return "hello"; })
var p/*5*/3 = p2.then(function (x/*6*/x) {
    return x/*7*/x;
});"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var p1: IPromise<number>", "");
    f.verify_quick_info_at(t, "2", "(parameter) xx: number", "");
    f.verify_quick_info_at(t, "3", "var p2: IPromise<string>", "");
    f.verify_quick_info_at(t, "4", "(parameter) xx: number", "");
    f.verify_quick_info_at(t, "5", "var p3: IPromise<string>", "");
    f.verify_quick_info_at(t, "6", "(parameter) xx: string", "");
    f.verify_quick_info_at(t, "7", "(parameter) xx: string", "");
    done();
}
