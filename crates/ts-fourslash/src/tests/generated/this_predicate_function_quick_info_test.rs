#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_this_predicate_function_quick_info() {
    let mut t = TestingT;
    run_test_this_predicate_function_quick_info(&mut t);
}

fn run_test_this_predicate_function_quick_info(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"class RoyalGuard {
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

if (((a.isLeader)())) {
    a./*9*/;
}
else if (((a).isFollower())) {
    a./*10*/;
}

if (((a["isLeader"])())) {
    a./*11*/;
}
else if (((a)["isFollower"]())) {
    a./*12*/;
}

let leader/*13*/Status = a.isLeader();
function isLeaderGuard(g: RoyalGuard) {
   return g.isLeader();
}
let checked/*14*/LeaderStatus = isLeader/*15*/Guard(a);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "1",
        "(method) RoyalGuard.isLeader(): this is LeadGuard",
        "",
    );
    f.verify_quick_info_at(
        t,
        "3",
        "(method) RoyalGuard.isFollower(): this is FollowerGuard",
        "",
    );
    f.verify_quick_info_at(
        t,
        "5",
        "(method) GuardInterface.isLeader(): this is LeadGuard",
        "",
    );
    f.verify_quick_info_at(
        t,
        "7",
        "(method) GuardInterface.isFollower(): this is FollowerGuard",
        "",
    );
    f.verify_quick_info_at(t, "13", "let leaderStatus: boolean", "");
    f.verify_quick_info_at(t, "14", "let checkedLeaderStatus: boolean", "");
    f.verify_quick_info_at(
        t,
        "15",
        "function isLeaderGuard(g: RoyalGuard): g is LeadGuard",
        "",
    );
    done();
}
