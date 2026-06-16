#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_export3() {
    let mut t = TestingT;
    run_test_get_occurrences_export3(&mut t);
}

fn run_test_get_occurrences_export3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"
declare var x;
[|export|] declare var y, z;

namespace m {
    export class C1 {
        public pub1;
        public pub2;
        private priv1;
        private priv2;
        protected prot1;
        protected prot2;

        public public;
        private private;
        protected protected;

        public constructor(public a, private b, protected c, public d, private e, protected f) {
            this.public = 10;
            this.private = 10;
            this.protected = 10;
        }

        public get x() { return 10; }
        public set x(value) { }

        public static statPub;
        private static statPriv;
        protected static statProt;
    }

    export interface I1 {
    }

    export declare namespace ma.m1.m2.m3 {
        interface I2 {
        }
    }

    export namespace mb.m1.m2.m3 {
        declare var foo;

        export class C2 {
            public pub1;
            private priv1;
            protected prot1;

            protected constructor(public public, protected protected, private private) {
            }
        }
    }

    declare var ambientThing: number;
    export var exportedThing = 10;
    declare function foo(): string;
}

declare [|export|] var v1, v2;
declare namespace dm { }
[|export|] class EC { }";
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
