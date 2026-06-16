#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_type_infer() {
    let mut t = TestingT;
    run_test_formatting_type_infer(&mut t);
}

fn run_test_formatting_type_infer(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingTypeInfer") {
        return;
    }
    let content = r"
/*L1*/type C<T> = T extends Array<infer U> ? U : never;

/*L2*/  type   C  <  T  >   =   T   extends   Array   <   infer     U  >  ?   U   :   never  ; 

/*L3*/type C<T> = T extends Array<infer U> ? U : T;

/*L4*/  type   C  <  T  >   =   T   extends   Array   <   infer     U  >  ?   U   :   T  ;  

/*L5*/type Foo<T> = T extends { a: infer U, b: infer U } ? U : never;

/*L6*/  type   Foo  <  T  > = T   extends   {   a  :   infer   U  ,   b  :   infer   U   }   ?   U   :   never  ;  

/*L7*/type Bar<T> = T extends { a: (x: infer U) => void, b: (x: infer U) => void } ? U : never;

/*L8*/  type   Bar  <  T  >   =   T   extends   {   a  :   (x  :  infer  U  ) =>   void  ,   b  :   (x  :   infer   U  )   =>   void   }    ?   U   :   never  ;
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "L1");
    f.verify_current_line_content(t, "type C<T> = T extends Array<infer U> ? U : never;");
    f.go_to_marker(t, "L2");
    f.verify_current_line_content(t, "type C<T> = T extends Array<infer U> ? U : never;");
    f.go_to_marker(t, "L3");
    f.verify_current_line_content(t, "type C<T> = T extends Array<infer U> ? U : T;");
    f.go_to_marker(t, "L4");
    f.verify_current_line_content(t, "type C<T> = T extends Array<infer U> ? U : T;");
    f.go_to_marker(t, "L5");
    f.verify_current_line_content(
        t,
        "type Foo<T> = T extends { a: infer U, b: infer U } ? U : never;",
    );
    f.go_to_marker(t, "L6");
    f.verify_current_line_content(
        t,
        "type Foo<T> = T extends { a: infer U, b: infer U } ? U : never;",
    );
    f.go_to_marker(t, "L7");
    f.verify_current_line_content(
        t,
        "type Bar<T> = T extends { a: (x: infer U) => void, b: (x: infer U) => void } ? U : never;",
    );
    f.go_to_marker(t, "L8");
    f.verify_current_line_content(
        t,
        "type Bar<T> = T extends { a: (x: infer U) => void, b: (x: infer U) => void } ? U : never;",
    );
    done();
}
