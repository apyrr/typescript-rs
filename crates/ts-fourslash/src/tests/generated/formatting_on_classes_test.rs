#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_classes() {
    let mut t = TestingT;
    run_test_formatting_on_classes(&mut t);
}

fn run_test_formatting_on_classes(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"/*1*/         class                    a                  {
/*2*/                                                        constructor       (       n   :                 number    )             ;
/*3*/                                                        constructor       (       s   :                 string    )             ;
/*4*/                                                        constructor       (       ns   :                 any    )                            {

/*5*/                                                        }

/*6*/                                                            public                 pgF       (           )                            {                  }

/*7*/                                                            public                 pv   ;
/*8*/                                                            public                 get              d       (           )                            {
/*9*/                                                                                                                return              30   ;
/*10*/                                                        }
/*11*/                                                            public                 set              d       (       number        )                            {
/*12*/                                                        }

/*13*/                                                            public                 static                    get              p2       (           )                            {
/*14*/                                                                                                                return                  {                  x   :                 30   ,                  y   :                 40              }   ;
/*15*/                                                        }

/*16*/                                                                         private                static                    d2       (           )                            {
/*17*/                                                        }
/*18*/                                                                         private                static                    get              p3       (           )                            {
/*19*/                                                                                                                return              "string"   ;
/*20*/                                                        }
/*21*/                                                                         private                pv3   ;

/*22*/                                                                         private                foo       (       n   :                 number    )             :                 string   ;
/*23*/                                                                         private                foo       (       s   :                 string    )             :                 string   ;
/*24*/                                                                         private                foo       (       ns   :                 any    )                            {
/*25*/                                                                                                                return              ns.toString       (           )             ;
/*26*/                                                        }
/*27*/}

/*28*/         class                    b              extends              a                  {
/*29*/}

/*30*/         class   m1b      {

/*31*/}

/*32*/                                                interface   m1ib                               {

/*33*/  }
/*34*/         class                    c              extends              m1b                  {
/*35*/}

/*36*/         class                    ib2              implements              m1ib                  {
/*37*/}

/*38*/    declare                            class                    aAmbient                  {
/*39*/                                                        constructor                     (       n   :                 number    )             ;
/*40*/                                                        constructor                     (       s   :                 string    )             ;
/*41*/                                                            public                 pgF       (           )             :                 void   ;
/*42*/                                                            public                 pv   ;
/*43*/                                                            public                 d                 :                 number   ;
/*44*/                                                        static                    p2                 :                     {                  x   :                 number   ;              y   :                 number   ;              }   ;
/*45*/                                                        static                    d2       (           )             ;
/*46*/                                                        static                    p3   ;
/*47*/                                                                         private                pv3   ;
/*48*/                                                                         private                foo       (       s    )             ;
/*49*/}

/*50*/         class                    d                  {
/*51*/                                                                         private                foo       (       n   :                 number    )             :                 string   ;
/*52*/                                                                         private                foo       (       s   :                 string    )             :                 string   ;
/*53*/                                                                         private                foo       (       ns   :                 any    )                            {
/*54*/                                                                                                                return              ns.toString       (           )             ;
/*55*/                                                        }
/*56*/}

/*57*/         class                    e                  {
/*58*/                                                                         private                foo       (       s   :                 string    )             :                 string   ;
/*59*/                                                                         private                foo       (       n   :                 number    )             :                 string   ;
/*60*/                                                                         private                foo       (       ns   :                 any    )                            {
/*61*/                                                                                                                return              ns.toString       (           )             ;
/*62*/                                                        }
/*63*/                                                                         protected              bar        (            )  {                 }
/*64*/                                                                         protected     static   bar2       (            )  {                 }
/*65*/                                                                         private                pv4  :    number =
/*66*/                                                                         {};
/*END*/}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "class a {");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    constructor(n: number);");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "    constructor(s: string);");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "    constructor(ns: any) {");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "    }");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "    public pgF() { }");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "    public pv;");
    f.go_to_marker(t, "8");
    f.verify_current_line_content(t, "    public get d() {");
    f.go_to_marker(t, "9");
    f.verify_current_line_content(t, "        return 30;");
    f.go_to_marker(t, "10");
    f.verify_current_line_content(t, "    }");
    f.go_to_marker(t, "11");
    f.verify_current_line_content(t, "    public set d(number) {");
    f.go_to_marker(t, "12");
    f.verify_current_line_content(t, "    }");
    f.go_to_marker(t, "13");
    f.verify_current_line_content(t, "    public static get p2() {");
    f.go_to_marker(t, "14");
    f.verify_current_line_content(t, "        return { x: 30, y: 40 };");
    f.go_to_marker(t, "15");
    f.verify_current_line_content(t, "    }");
    f.go_to_marker(t, "16");
    f.verify_current_line_content(t, "    private static d2() {");
    f.go_to_marker(t, "17");
    f.verify_current_line_content(t, "    }");
    f.go_to_marker(t, "18");
    f.verify_current_line_content(t, "    private static get p3() {");
    f.go_to_marker(t, "19");
    f.verify_current_line_content(t, "        return \"string\";");
    f.go_to_marker(t, "20");
    f.verify_current_line_content(t, "    }");
    f.go_to_marker(t, "21");
    f.verify_current_line_content(t, "    private pv3;");
    f.go_to_marker(t, "22");
    f.verify_current_line_content(t, "    private foo(n: number): string;");
    f.go_to_marker(t, "23");
    f.verify_current_line_content(t, "    private foo(s: string): string;");
    f.go_to_marker(t, "24");
    f.verify_current_line_content(t, "    private foo(ns: any) {");
    f.go_to_marker(t, "25");
    f.verify_current_line_content(t, "        return ns.toString();");
    f.go_to_marker(t, "26");
    f.verify_current_line_content(t, "    }");
    f.go_to_marker(t, "27");
    f.verify_current_line_content(t, "}");
    f.go_to_marker(t, "28");
    f.verify_current_line_content(t, "class b extends a {");
    f.go_to_marker(t, "29");
    f.verify_current_line_content(t, "}");
    f.go_to_marker(t, "30");
    f.verify_current_line_content(t, "class m1b {");
    f.go_to_marker(t, "31");
    f.verify_current_line_content(t, "}");
    f.go_to_marker(t, "32");
    f.verify_current_line_content(t, "interface m1ib {");
    f.go_to_marker(t, "33");
    f.verify_current_line_content(t, "}");
    f.go_to_marker(t, "34");
    f.verify_current_line_content(t, "class c extends m1b {");
    f.go_to_marker(t, "35");
    f.verify_current_line_content(t, "}");
    f.go_to_marker(t, "36");
    f.verify_current_line_content(t, "class ib2 implements m1ib {");
    f.go_to_marker(t, "37");
    f.verify_current_line_content(t, "}");
    f.go_to_marker(t, "38");
    f.verify_current_line_content(t, "declare class aAmbient {");
    f.go_to_marker(t, "39");
    f.verify_current_line_content(t, "    constructor(n: number);");
    f.go_to_marker(t, "40");
    f.verify_current_line_content(t, "    constructor(s: string);");
    f.go_to_marker(t, "41");
    f.verify_current_line_content(t, "    public pgF(): void;");
    f.go_to_marker(t, "42");
    f.verify_current_line_content(t, "    public pv;");
    f.go_to_marker(t, "43");
    f.verify_current_line_content(t, "    public d: number;");
    f.go_to_marker(t, "44");
    f.verify_current_line_content(t, "    static p2: { x: number; y: number; };");
    f.go_to_marker(t, "45");
    f.verify_current_line_content(t, "    static d2();");
    f.go_to_marker(t, "46");
    f.verify_current_line_content(t, "    static p3;");
    f.go_to_marker(t, "47");
    f.verify_current_line_content(t, "    private pv3;");
    f.go_to_marker(t, "48");
    f.verify_current_line_content(t, "    private foo(s);");
    f.go_to_marker(t, "49");
    f.verify_current_line_content(t, "}");
    f.go_to_marker(t, "50");
    f.verify_current_line_content(t, "class d {");
    f.go_to_marker(t, "51");
    f.verify_current_line_content(t, "    private foo(n: number): string;");
    f.go_to_marker(t, "52");
    f.verify_current_line_content(t, "    private foo(s: string): string;");
    f.go_to_marker(t, "53");
    f.verify_current_line_content(t, "    private foo(ns: any) {");
    f.go_to_marker(t, "54");
    f.verify_current_line_content(t, "        return ns.toString();");
    f.go_to_marker(t, "55");
    f.verify_current_line_content(t, "    }");
    f.go_to_marker(t, "56");
    f.verify_current_line_content(t, "}");
    f.go_to_marker(t, "57");
    f.verify_current_line_content(t, "class e {");
    f.go_to_marker(t, "58");
    f.verify_current_line_content(t, "    private foo(s: string): string;");
    f.go_to_marker(t, "59");
    f.verify_current_line_content(t, "    private foo(n: number): string;");
    f.go_to_marker(t, "60");
    f.verify_current_line_content(t, "    private foo(ns: any) {");
    f.go_to_marker(t, "61");
    f.verify_current_line_content(t, "        return ns.toString();");
    f.go_to_marker(t, "62");
    f.verify_current_line_content(t, "    }");
    f.go_to_marker(t, "63");
    f.verify_current_line_content(t, "    protected bar() { }");
    f.go_to_marker(t, "64");
    f.verify_current_line_content(t, "    protected static bar2() { }");
    f.go_to_marker(t, "65");
    f.verify_current_line_content(t, "    private pv4: number =");
    f.go_to_marker(t, "66");
    f.verify_current_line_content(t, "        {};");
    f.go_to_marker(t, "END");
    f.verify_current_line_content(t, "}");
    done();
}
