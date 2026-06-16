#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_modifiers_negatives1() {
    let mut t = TestingT;
    run_test_get_occurrences_modifiers_negatives1(&mut t);
}

fn run_test_get_occurrences_modifiers_negatives1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"class C {
    [|{| "count": 3 |}export|] foo;
    [|{| "count": 3 |}declare|] bar;
    [|{| "count": 3 |}export|] [|{| "count": 3 |}declare|] foobar;
    [|{| "count": 3 |}declare|] [|{| "count": 3 |}export|] barfoo;

    constructor([|{| "count": 9 |}export|] conFoo,
                [|{| "count": 9 |}declare|] conBar,
                [|{| "count": 9 |}export|] [|{| "count": 9 |}declare|] conFooBar,
                [|{| "count": 9 |}declare|] [|{| "count": 9 |}export|] conBarFoo,
                [|{| "count": 4 |}static|] sue,
                [|{| "count": 4 |}static|] [|{| "count": 9 |}export|] [|{| "count": 9 |}declare|] sueFooBar,
                [|{| "count": 4 |}static|] [|{| "count": 9 |}declare|] [|{| "count": 9 |}export|] sueBarFoo,
                [|{| "count": 9 |}declare|] [|{| "count": 4 |}static|] [|{| "count": 9 |}export|] barSueFoo) {
    }
}

namespace m {
    [|{| "count": 0 |}static|] a;
    [|{| "count": 0 |}public|] b;
    [|{| "count": 0 |}private|] c;
    [|{| "count": 0 |}protected|] d;
    [|{| "count": 0 |}static|] [|{| "count": 0 |}public|] [|{| "count": 0 |}private|] [|{| "count": 0 |}protected|] e;
    [|{| "count": 0 |}public|] [|{| "count": 0 |}static|] [|{| "count": 0 |}protected|] [|{| "count": 0 |}private|] f;
    [|{| "count": 0 |}protected|] [|{| "count": 0 |}static|] [|{| "count": 0 |}public|] g;
}
[|{| "count": 0 |}static|] a;
[|{| "count": 0 |}public|] b;
[|{| "count": 0 |}private|] c;
[|{| "count": 0 |}protected|] d;
[|{| "count": 0 |}static|] [|{| "count": 0 |}public|] [|{| "count": 0 |}private|] [|{| "count": 0 |}protected|] e;
[|{| "count": 0 |}public|] [|{| "count": 0 |}static|] [|{| "count": 0 |}protected|] [|{| "count": 0 |}private|] f;
[|{| "count": 0 |}protected|] [|{| "count": 0 |}static|] [|{| "count": 0 |}public|] g;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights(
        t,
        None,
        f.ranges()
            .into_iter()
            .map(MarkerOrRangeOrName::Range)
            .collect(),
    );
    done();
}
