#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_this_predicate_function_completions03() {
    let mut t = TestingT;
    run_test_this_predicate_function_completions03(&mut t);
}

fn run_test_this_predicate_function_completions03(t: &mut TestingT) {
    if should_skip_if_failing("TestThisPredicateFunctionCompletions03") {
        return;
    }
    let content = r"class RoyalGuard {
    isLeader(): this is LeadGuard {
        return this instanceof LeadGuard;
    }
    isFollower(): this is FollowerGuard {
        return this instanceof FollowerGuard;
    }
}

class LeadGuard extends RoyalGuard {
    lead(): void {};
}

class FollowerGuard extends RoyalGuard {
    follow(): void {};
}

let a: RoyalGuard = new FollowerGuard();
if (a.is/*1*/Leader()) {
    a./*2*/;
}
else if (a.is/*3*/Follower()) {
    a./*4*/;
}

interface GuardInterface {
   isLeader(): this is LeadGuard;
   isFollower(): this is FollowerGuard;
}

let b: GuardInterface;
if (b.is/*5*/Leader()) {
    b./*6*/;
}
else if (b.is/*7*/Follower()) {
    b./*8*/;
}

let leader/*13*/Status = a.isLeader();
function isLeaderGuard(g: RoyalGuard) {
   return g.isLeader();
}
let checked/*14*/LeaderStatus = isLeader/*15*/Guard(a);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["2".to_string(), "6".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: vec![
                    CompletionsExpectedItem::Label("lead".to_string()),
                    CompletionsExpectedItem::Label("isLeader".to_string()),
                    CompletionsExpectedItem::Label("isFollower".to_string()),
                ],
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["4".to_string(), "8".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: vec![
                    CompletionsExpectedItem::Label("follow".to_string()),
                    CompletionsExpectedItem::Label("isLeader".to_string()),
                    CompletionsExpectedItem::Label("isFollower".to_string()),
                ],
            }),
            user_preferences: None,
        }),
    );
    done();
}
