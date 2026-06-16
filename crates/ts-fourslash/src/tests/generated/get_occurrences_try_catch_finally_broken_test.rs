#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_try_catch_finally_broken() {
    let mut t = TestingT;
    run_test_get_occurrences_try_catch_finally_broken(&mut t);
}

fn run_test_get_occurrences_try_catch_finally_broken(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"t /*1*/ry {
    t/*2*/ry {
    }
    ctch (x) {
    }

    tr {
    }
    fin/*3*/ally {
    }
}
c/*4*/atch (e) {
}
f/*5*/inally {
}

// Missing catch variable
t/*6*/ry {
}
catc/*7*/h {
}
/*8*/finally {
}

// Missing try entirely
cat/*9*/ch (x) {
}
final/*10*/ly {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights(
        t,
        None,
        f.markers()
            .into_iter()
            .map(MarkerOrRangeOrName::Marker)
            .collect(),
    );
    done();
}
