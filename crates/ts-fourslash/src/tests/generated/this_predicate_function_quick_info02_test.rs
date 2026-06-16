#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_this_predicate_function_quick_info02() {
    let mut t = TestingT;
    run_test_this_predicate_function_quick_info02(&mut t);
}

fn run_test_this_predicate_function_quick_info02(t: &mut TestingT) {
    if should_skip_if_failing("TestThisPredicateFunctionQuickInfo02") {
        return;
    }
    let content = r"interface Sundries {
    broken: boolean;
}

interface Supplies {
    spoiled: boolean;
}

interface Crate<T> {
    contents: T;
    /*1*/isSundries(): this is Crate<Sundries>;
    /*2*/isSupplies(): this is Crate<Supplies>;
    /*3*/isPackedTight(): this is (this & {extraContents: T});
}
const crate: Crate<any>;
if (crate.isPackedTight/*4*/()) {
    crate.;
}
if (crate.isSundries/*5*/()) {
    crate.contents.;
    if (crate.isPackedTight/*6*/()) {
       crate.;
    }
}
if (crate.isSupplies/*7*/()) {
    crate.contents.;
    if (crate.isPackedTight/*8*/()) {
       crate.;
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "1",
        "(method) Crate<T>.isSundries(): this is Crate<Sundries>",
        "",
    );
    f.verify_quick_info_at(
        t,
        "2",
        "(method) Crate<T>.isSupplies(): this is Crate<Supplies>",
        "",
    );
    f.verify_quick_info_at(
        t,
        "3",
        "(method) Crate<T>.isPackedTight(): this is (this & {\n    extraContents: T;\n})",
        "",
    );
    f.verify_quick_info_at(
        t,
        "4",
        "(method) Crate<any>.isPackedTight(): this is (Crate<any> & {\n    extraContents: any;\n})",
        "",
    );
    f.verify_quick_info_at(
        t,
        "5",
        "(method) Crate<any>.isSundries(): this is Crate<Sundries>",
        "",
    );
    f.verify_quick_info_at(t, "6", "(method) Crate<Sundries>.isPackedTight(): this is (Crate<Sundries> & {\n    extraContents: Sundries;\n})", "");
    f.verify_quick_info_at(
        t,
        "7",
        "(method) Crate<any>.isSupplies(): this is Crate<Supplies>",
        "",
    );
    f.verify_quick_info_at(t, "8", "(method) Crate<Supplies>.isPackedTight(): this is (Crate<Supplies> & {\n    extraContents: Supplies;\n})", "");
    done();
}
