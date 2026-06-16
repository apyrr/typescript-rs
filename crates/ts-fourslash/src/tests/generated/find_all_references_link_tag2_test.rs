#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_references_link_tag2() {
    let mut t = TestingT;
    run_test_find_all_references_link_tag2(&mut t);
}

fn run_test_find_all_references_link_tag2(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllReferencesLinkTag2") {
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
     * @see {Consider.prototype.m}
     * {@link Consider#m}
     * @see {Consider#This#show}
     * {@link Consider.This.show}
     * @see {NPR.Consider#This#show}
     * {@link NPR.Consider.This#show}
     * @see {NPR.Consider#This.show} # doesn't parse trailing .
     * @see {NPR.Consider.This.show}
     */
    export function ref() { }
}
/**
 * {@link NPR.Consider#This#show hello hello}
 * {@link NPR.Consider.This#show}
 * @see {NPR.Consider#This.show} # doesn't parse trailing .
 * @see {NPR.Consider.This.show}
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
