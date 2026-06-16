#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_this6() {
    let mut t = TestingT;
    run_test_get_occurrences_this6(&mut t);
}

fn run_test_get_occurrences_this6(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesThis6") {
        return;
    }
    let content = r"this/*1*/;
this;

function f() {
    this/*2*/;
    this;
    () => this;
    () => {
        if (this) {
            this;
        }
        else {
            this.this;
        }
    }
    function inside() {
        this;
        (function (_) {
            this;
        })(this);
    }
}

namespace m {
    var x = th/*6*/is;
    function f() {
        this;
        this;
        () => this;
        () => {
            if (this) {
                this;
            }
            else {
                this.this;
            }
        }
        function inside() {
            this;
            (function (_) {
                this;
            })(this);
        }
    }
}

class A {
    public b = this.method1;

    public method1() {
        this;
        this;
        () => this;
        () => {
            if (this) {
                this;
            }
            else {
                this.this;
            }
        }
        function inside() {
            this;
            (function (_) {
                this/*3*/;
            })(this);
        }
    }

    private method2() {
        this;
        this;
        () => this;
        () => {
            if (this) {
                this;
            }
            else {
                this.this;
            }
        }
        function inside() {
            this;
            (function (_) {
                this;
            })(this);
        }
    }

    public static staticB = this.staticMethod1;

    public static staticMethod1() {
        this;
        this;
        () => this;
        () => {
            if (this) {
                this;
            }
            else {
                this.this;
            }
        }
        function inside() {
            this;
            (function (_) {
                this;
            })(this);
        }
    }

    private static staticMethod2() {
        this;
        this;
        () => this;
        () => {
            if (this) {
                this;
            }
            else {
                this.this;
            }
        }
        function inside() {
            this;
            (function (_) {
                this;
            })(this);
        }
    }
}

var x = {
    a: /*4*/this,

    f() {
        this/*5*/;
        function foo() {
            this;
        }
        const bar = () => {
            this;
        }
    },

    g() {
        this;
    },

    get h() {
        /*7*/this;
        function foo() {
            this;
        }
        const bar = () => {
            this;
        }
        return;
    },

    set h(foo: any) {
        this;
    },

    l: () => {
        /*8*/this;
        function foo() {
            this;
        }
        const bar = () => {
            this;
        }
    },
};
";
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
