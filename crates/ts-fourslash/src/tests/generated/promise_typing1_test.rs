#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_promise_typing1() {
    let mut t = TestingT;
    run_test_promise_typing1(&mut t);
}

fn run_test_promise_typing1(t: &mut TestingT) {
    if should_skip_if_failing("TestPromiseTyping1") {
        return;
    }
    let content = r"interface IPromise<T> {
    then<U>(success: (value: T) => IPromise<U>, error?: (error: any) => IPromise<U>, progress?: (progress: any) => void ): IPromise<U>;
    then<U>(success: (value: T) => IPromise<U>, error?: (error: any) => U, progress?: (progress: any) => void ): IPromise<U>;
    then<U>(success: (value: T) => U, error?: (error: any) => IPromise<U>, progress?: (progress: any) => void ): IPromise<U>;
    then<U>(success: (value: T) => U, error?: (error: any) => U, progress?: (progress: any) => void ): IPromise<U>;
    done? <U>(success: (value: T) => any, error?: (error: any) => any, progress?: (progress: any) => void ): void;
}
var p1: IPromise<string>;
var p/*1*/2 = p1.then(function (x/*2*/x) {
    return xx;
});
p2.then(function (x/*3*/x) {
} );";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var p2: IPromise<string>", "");
    f.verify_quick_info_at(t, "2", "(parameter) xx: string", "");
    f.verify_quick_info_at(t, "3", "(parameter) xx: string", "");
    done();
}
