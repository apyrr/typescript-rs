#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generics_formatting() {
    let mut t = TestingT;
    run_test_generics_formatting(&mut t);
}

fn run_test_generics_formatting(t: &mut TestingT) {
    if should_skip_if_failing("TestGenericsFormatting") {
        return;
    }
    let content = r"/*inClassDeclaration*/class Foo   <    T1   ,  T2    >  {
/*inMethodDeclaration*/    public method    <   T3,    T4   >   ( a: T1,   b: Array    < T4 > ):   Map < T1  ,   T2, Array < T3    >    > {
    }
}
/*typeArguments*/var foo = new Foo   <  number, Array <   number  >   >  (  );
/*typeArgumentsWithTypeLiterals*/foo = new Foo  <  {   bar  :  number }, Array   < {   baz :  string   }  >  >  (  );

interface IFoo {
/*inNewSignature*/new < T  > ( a: T);
/*inOptionalMethodSignature*/op?< T , M > (a: T, b : M );
}

foo()<number, string, T >();
(a + b)<number, string, T >();

/*inFunctionDeclaration*/function bar <T> () {
/*inClassExpression*/    return class  <  T2 > {
    }
}
/*expressionWithTypeArguments*/class A < T > extends bar <  T >( )  <  T > {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "inClassDeclaration");
    f.verify_current_line_content(t, "class Foo<T1, T2> {");
    f.go_to_marker(t, "inMethodDeclaration");
    f.verify_current_line_content(
        t,
        "    public method<T3, T4>(a: T1, b: Array<T4>): Map<T1, T2, Array<T3>> {",
    );
    f.go_to_marker(t, "typeArguments");
    f.verify_current_line_content(t, "var foo = new Foo<number, Array<number>>();");
    f.go_to_marker(t, "typeArgumentsWithTypeLiterals");
    f.verify_current_line_content(
        t,
        "foo = new Foo<{ bar: number }, Array<{ baz: string }>>();",
    );
    f.go_to_marker(t, "inNewSignature");
    f.verify_current_line_content(t, "    new <T>(a: T);");
    f.go_to_marker(t, "inOptionalMethodSignature");
    f.verify_current_line_content(t, "    op?<T, M>(a: T, b: M);");
    f.go_to_marker(t, "inFunctionDeclaration");
    f.verify_current_line_content(t, "function bar<T>() {");
    f.go_to_marker(t, "inClassExpression");
    f.verify_current_line_content(t, "    return class <T2> {");
    f.go_to_marker(t, "expressionWithTypeArguments");
    f.verify_current_line_content(t, "class A<T> extends bar<T>()<T> {");
    done();
}
