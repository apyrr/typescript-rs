#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_this_predicate_function_quick_info01() {
    let mut t = TestingT;
    run_test_this_predicate_function_quick_info01(&mut t);
}

fn run_test_this_predicate_function_quick_info01(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"class FileSystemObject {
    /*1*/isFile(): this is Item {
        return this instanceof Item;
    }
    /*2*/isDirectory(): this is Directory {
        return this instanceof Directory;
    }
    /*3*/isNetworked(): this is (Networked & this) {
       return !!(this as Networked).host;
    }
    constructor(public path: string) {}
}

class Item extends FileSystemObject {
    constructor(path: string, public content: string) { super(path); }
}
class Directory extends FileSystemObject {
    children: FileSystemObject[];
}
interface Networked {
    host: string;
}

const obj: FileSystemObject = new Item("/foo", "");
if (obj.isFile/*4*/()) {
    obj.;
    if (obj.isNetworked/*5*/()) {
        obj.;
    }
}
if (obj.isDirectory/*6*/()) {
    obj.;
    if (obj.isNetworked/*7*/()) {
        obj.;
    }
}
if (obj.isNetworked/*8*/()) {
    obj.;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "1",
        "(method) FileSystemObject.isFile(): this is Item",
        "",
    );
    f.verify_quick_info_at(
        t,
        "2",
        "(method) FileSystemObject.isDirectory(): this is Directory",
        "",
    );
    f.verify_quick_info_at(
        t,
        "3",
        "(method) FileSystemObject.isNetworked(): this is (Networked & this)",
        "",
    );
    f.verify_quick_info_at(
        t,
        "4",
        "(method) FileSystemObject.isFile(): this is Item",
        "",
    );
    f.verify_quick_info_at(
        t,
        "5",
        "(method) FileSystemObject.isNetworked(): this is (Networked & Item)",
        "",
    );
    f.verify_quick_info_at(
        t,
        "6",
        "(method) FileSystemObject.isDirectory(): this is Directory",
        "",
    );
    f.verify_quick_info_at(
        t,
        "7",
        "(method) FileSystemObject.isNetworked(): this is (Networked & Directory)",
        "",
    );
    f.verify_quick_info_at(
        t,
        "8",
        "(method) FileSystemObject.isNetworked(): this is (Networked & FileSystemObject)",
        "",
    );
    done();
}
