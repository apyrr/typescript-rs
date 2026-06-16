#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_class_expression_static_this() {
    let mut t = TestingT;
    run_test_get_occurrences_class_expression_static_this(&mut t);
}

fn run_test_get_occurrences_class_expression_static_this(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"var x = class C {
    public x;
    public y;
    public z;
    public staticX;
    constructor() {
        this;
        this.x;
        this.y;
        this.z;
    }
    foo() {
        this;
        () => this;
        () => {
            if (this) {
                this;
            }
        }
        function inside() {
            this;
            (function (_) {
                this;
            })(this);
        }
        return this.x;
    }

    static bar() {
        [|this|];
        [|this|].staticX;
        () => [|this|];
        () => {
            if ([|this|]) {
                [|this|];
            }
        }
        function inside() {
            this;
            (function (_) {
                this;
            })(this);
        }
    }
}";
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
