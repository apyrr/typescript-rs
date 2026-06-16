#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_class_members4() {
    let mut t = TestingT;
    run_test_completions_class_members4(&mut t);
}

fn run_test_completions_class_members4(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @filename: /foo.ts
export class Observable<T> {
    pipe(): Observable<T>;
    pipe<A>(): Observable<A>;
    pipe<A, B>(): Observable<B>;
    pipe<A, B, C>(): Observable<C>;
    pipe<A, B, C, D>(): Observable<D>;
    pipe<A, B, C, D, E>(): Observable<E>;
    pipe<A, B, C, D, E, F>(): Observable<F>;
    pipe<A, B, C, D, E, F, G>(): Observable<G>;
    pipe<A, B, C, D, E, F, G, H>(): Observable<H>;
    pipe<A, B, C, D, E, F, G, H, I>(): Observable<I>;
    pipe<A, B, C, D, E, F, G, H, I>(): Observable<unknown>;
}
export class Foo extends Observable<any> {
    /**/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
