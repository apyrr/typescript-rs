#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_references_link_tag3() {
    let mut t = TestingT;
    run_test_find_all_references_link_tag3(&mut t);
}

fn run_test_find_all_references_link_tag3(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllReferencesLinkTag3") {
        return;
    }
    let content = r"namespace NPR/*5*/ {
    export class Consider/*4*/ {
        This/*3*/ = class {
            show/*2*/() { }
        }
        m/*1*/() { }
    }
    /**
     * {@linkcode Consider.prototype.m}
     * {@linkplain Consider#m}
     * {@linkcode Consider#This#show}
     * {@linkplain Consider.This.show}
     * {@linkcode NPR.Consider#This#show}
     * {@linkplain NPR.Consider.This#show}
     * {@linkcode NPR.Consider#This.show} # doesn't parse trailing .
     * {@linkcode NPR.Consider.This.show}
     */
    export function ref() { }
}
/**
 * {@linkplain NPR.Consider#This#show hello hello}
 * {@linkplain NPR.Consider.This#show}
 * {@linkcode NPR.Consider#This.show} # doesn't parse trailing .
 * {@linkcode NPR.Consider.This.show}
 */
export function outerref() { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
        ],
    );
    done();
}
