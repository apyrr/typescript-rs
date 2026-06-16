#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generics_formatting_multiline() {
    let mut t = TestingT;
    run_test_generics_formatting_multiline(&mut t);
}

fn run_test_generics_formatting_multiline(t: &mut TestingT) {
    if should_skip_if_failing("TestGenericsFormattingMultiline") {
        return;
    }
    let content = r#"
class Foo   <   
 T1   extends unknown,
  T2   
    > {
    public method    <  
 T3,
    >   (a: T1,   b: Array    < 
     string 
     > ):   Map <
          T1 ,
      Array < T3    >  
          > { throw new Error(); } 
}

interface IFoo<
       T, 
  > {
    new < T
      > ( a: T);
    op?< 
   T,
      M
    > (a: T, b : M );
    <
     T,
      >(x: T): T;
}

type foo<
  T
   > = Foo   <
  number, Array <   number  >  > ;

function bar <
T, U extends T
 >  () {
    return class  < 
       T2,
  > {
    }
}

bar<
string, 
     "s"
     > ();

declare const func: <
T   extends number[], 
                       > (x: T) => new <
       U
                          > () => U;

class A < T > extends bar <  
        T,number
 >( )  <  T
     > {
}

function s<T, U>(x: TemplateStringsArray, ...args: any[]) { return x.join(); }

const t = s<
      number , 
  string[] & ArrayLike<any>
      >`abc${1}def` ;
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r#"
class Foo<
    T1 extends unknown,
    T2
> {
    public method<
        T3,
    >(a: T1, b: Array<
        string
    >): Map<
        T1,
        Array<T3>
    > { throw new Error(); }
}

interface IFoo<
    T,
> {
    new <T
    >(a: T);
    op?<
        T,
        M
    >(a: T, b: M);
    <
        T,
    >(x: T): T;
}

type foo<
    T
> = Foo<
    number, Array<number>>;

function bar<
    T, U extends T
>() {
    return class <
        T2,
    > {
    }
}

bar<
    string,
    "s"
>();

declare const func: <
    T extends number[],
> (x: T) => new <
    U
> () => U;

class A<T> extends bar<
    T, number
>()<T
> {
}

function s<T, U>(x: TemplateStringsArray, ...args: any[]) { return x.join(); }

const t = s<
    number,
    string[] & ArrayLike<any>
>`abc${1}def`;
"#,
    );
    done();
}
