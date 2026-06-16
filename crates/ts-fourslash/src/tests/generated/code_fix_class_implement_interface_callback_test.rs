#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_callback() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_callback(&mut t);
}

fn run_test_code_fix_class_implement_interface_callback(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixClassImplementInterfaceCallback") {
        return;
    }
    let content = r"interface IFoo1 {
    parse(reviver: () => any): void;
}

class Foo1 implements IFoo1 {
}

interface IFoo2 {
    parse(reviver: { (): any }): void;
}

class Foo2 implements IFoo2 {
}

interface IFoo3 {
    parse(reviver: new () => any): void;
}

class Foo3 implements IFoo3 {
}

interface IFoo4 {
    parse(reviver: { new (): any }): void;
}

class Foo4 implements IFoo4 {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_all(
        t,
        VerifyCodeFixAllOptions {
            fix_id: "fixClassIncorrectlyImplementsInterface".to_string(),
            new_file_content: r#"interface IFoo1 {
    parse(reviver: () => any): void;
}

class Foo1 implements IFoo1 {
    parse(reviver: () => any): void {
        throw new Error("Method not implemented.");
    }
}

interface IFoo2 {
    parse(reviver: { (): any }): void;
}

class Foo2 implements IFoo2 {
    parse(reviver: { (): any; }): void {
        throw new Error("Method not implemented.");
    }
}

interface IFoo3 {
    parse(reviver: new () => any): void;
}

class Foo3 implements IFoo3 {
    parse(reviver: new () => any): void {
        throw new Error("Method not implemented.");
    }
}

interface IFoo4 {
    parse(reviver: { new (): any }): void;
}

class Foo4 implements IFoo4 {
    parse(reviver: { new(): any; }): void {
        throw new Error("Method not implemented.");
    }
}"#
            .to_string(),
        },
    );
    done();
}
