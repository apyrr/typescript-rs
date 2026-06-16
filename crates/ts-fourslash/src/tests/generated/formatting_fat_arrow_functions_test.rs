#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_fat_arrow_functions() {
    let mut t = TestingT;
    run_test_formatting_fat_arrow_functions(&mut t);
}

fn run_test_formatting_fat_arrow_functions(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// valid
    (         )           =>    1  ;/*1*/
    (        arg )           =>    2  ;/*2*/
        arg       =>    2  ;/*3*/
        arg=>2  ;/*3a*/
      (        arg     = 1 )           =>    3  ;/*4*/
    (        arg    ?        )           =>    4  ;/*5*/
    (        arg    :    number )           =>    5  ;/*6*/
      (        arg    :    number     = 0 )           =>    6  ;/*7*/
    (        arg        ?                  :    number )           =>    7  ;/*8*/
    (                 ...     arg    :    number   [      ]    )           =>    8  ;/*9*/
      (        arg1   ,    arg2 )           =>    12  ;/*10*/
    (        arg1     = 1   ,    arg2     =3 )           =>    13  ;/*11*/
      (        arg1    ?          ,    arg2    ?        )           =>    14  ;/*12*/
    (        arg1    :    number   ,    arg2    :    number )           =>    15  ;/*13*/
    (        arg1    :    number     = 0   ,    arg2    :    number     = 1 )           =>    16  ;/*14*/
      (        arg1    ?           :    number   ,    arg2    ?           :    number )           =>    17  ;/*15*/
    (        arg1   ,             ...     arg2    :    number   [      ]    )           =>    18  ;/*16*/
      (        arg1   ,    arg2    ?           :    number )           =>    19  ;/*17*/

// in paren
    (            (         )           =>    21 )      ;/*18*/
    (            (        arg )           =>    22 )      ;/*19*/
    (            (        arg     = 1 )           =>    23 )      ;/*20*/
    (            (        arg    ?        )           =>    24 )      ;/*21*/
    (            (        arg    :    number )           =>    25 )      ;/*22*/
    (            (        arg    :    number     = 0 )           =>    26 )      ;/*23*/
    (            (        arg    ?           :    number )           =>    27 )      ;/*24*/
    (            (                 ...     arg    :    number   [      ]    )           =>    28 )      ;/*25*/

// in multiple paren
    (            (            (            (            (        arg )           =>    { return 32  ;    } )     )     )     )      ;/*26*/

// in ternary exression
      false        ?            (         )           =>    41     :    null  ;/*27*/
   false        ?            (        arg )           =>    42     :    null  ;/*28*/
    false        ?            (        arg     = 1 )           =>    43     :    null  ;/*29*/
      false        ?            (        arg    ?        )           =>    44     :    null  ;/*30*/
    false        ?            (        arg    :    number )           =>    45     :    null  ;/*31*/
   false        ?            (        arg    ?           :    number )           =>    46     :    null  ;/*32*/
      false        ?            (        arg    ?           :    number     = 0 )           =>    47     :    null  ;/*33*/
   false        ?            (                 ...     arg    :    number   [      ]    )           =>    48     :    null  ;/*34*/

// in ternary exression within paren
   false        ?            (            (         )           =>    51 )         :    null  ;/*35*/
    false        ?            (            (        arg )           =>    52 )         :    null  ;/*36*/
    false        ?            (            (        arg     = 1 )           =>    53 )         :    null  ;/*37*/
      false        ?            (            (        arg    ?        )           =>    54 )         :    null  ;/*38*/
    false        ?            (            (        arg    :    number )           =>    55 )         :    null  ;/*39*/
      false        ?            (            (        arg    ?           :    number )           =>    56 )         :    null  ;/*40*/
    false        ?            (            (        arg    ?           :    number     = 0 )           =>    57 )         :    null  ;/*41*/
   false        ?            (            (                 ...     arg    :    number   [      ]    )           =>    58 )         :    null  ;/*42*/

// ternary exression's else clause
   false        ?        null     :        (         )           =>    61  ;/*43*/
        false        ?        null     :        (        arg )           =>    62  ;/*44*/
   false        ?        null     :        (        arg     = 1 )           =>    63  ;/*45*/
      false        ?        null     :        (        arg    ?        )           =>    64  ;/*46*/
   false        ?        null     :        (        arg    :    number )           =>    65  ;/*47*/
    false        ?        null     :        (        arg    ?           :    number )           =>    66  ;/*48*/
        false        ?        null     :        (        arg    ?           :    number     = 0 )           =>    67  ;/*49*/
    false        ?        null     :        (                 ...     arg    :    number   [      ]    )           =>    68  ;/*50*/


// nested ternary expressions
    ((        a    ?        )           =>    { return a  ;    })     ?            (        b    ?         )           =>    { return b  ;    }     :        (        c    ?         )           =>    { return c  ;    }  ;/*51*/

//multiple levels
    ((        a    ?        )           =>    { return a  ;    })     ?            (        b )          =>       (        c )          =>   81     :        (        c )          =>       (        d )          =>   82  ;/*52*/


// In Expressions
    (            (        arg )           =>    90 )     instanceof Function  ;/*53*/
      (            (        arg     = 1 )           =>    91 )     instanceof Function  ;/*54*/
        (            (        arg    ?         )           =>    92 )     instanceof Function  ;/*55*/
      (            (        arg    :    number )           =>    93 )     instanceof Function  ;/*56*/
    (            (        arg    :    number     = 1 )           =>    94 )     instanceof Function  ;/*57*/
        (            (        arg    ?           :    number )           =>    95 )     instanceof Function  ;/*58*/
      (            (                 ...     arg    :    number   [      ]    )           =>    96 )     instanceof Function  ;/*59*/

''    +        ((        arg )           =>    100)  ;/*60*/
        (            (        arg )           =>    0 )        +    ''    +        ((        arg )           =>    101)  ;/*61*/
          (            (        arg     = 1 )           =>    0 )        +    ''    +        ((        arg     = 2 )           =>    102)  ;/*62*/
    (            (        arg    ?        )           =>    0 )        +    ''    +        ((        arg    ?        )           =>    103)  ;/*63*/
      (            (        arg    :   number )           =>    0 )        +    ''    +        ((        arg    :   number )           =>    104)  ;/*64*/
        (            (        arg    :   number     = 1 )           =>    0 )        +    ''    +        ((        arg    :   number     = 2 )           =>    105)  ;/*65*/
    (            (        arg    ?           :   number     )           =>    0 )        +    ''    +        ((        arg    ?           :   number     )           =>    106)  ;/*66*/
      (            (                 ...     arg    :   number   [      ]    )           =>    0 )        +    ''    +        ((                 ...     arg    :   number   [      ]    )           =>    107)  ;/*67*/
    (            (        arg1   ,    arg2    ?        )           =>    0 )        +    ''    +        ((        arg1   ,   arg2    ?        )           =>    108)  ;/*68*/
      (            (        arg1   ,             ...     arg2    :   number   [      ]    )           =>    0 )        +    ''    +        ((        arg1   ,             ...     arg2    :   number   [      ]    )           =>    108)  ;/*69*/


// Function Parameters
/*70*/function foo    (                 ...     arg    :    any   [      ]    )     { }

/*71*/foo    (
/*72*/        (        a )           =>    110   ,
/*73*/        (            (        a )           =>    111 )       ,
/*74*/        (        a )           =>    {
        return /*75*/112  ;
/*76*/    }   ,
/*77*/        (        a    ?         )           =>    113   ,
/*78*/        (        a   ,    b    ?         )           =>    114   ,
/*79*/        (        a    :    number )           =>    115   ,
/*80*/        (        a    :    number     = 0 )           =>    116   ,
/*81*/        (        a     = 0 )           =>    117   ,
/*82*/        (        a               :    number     = 0 )           =>    118   ,
/*83*/        (        a    ?    ,   b   ?          :    number      )           =>    118   ,
/*84*/        (                 ...     a    :    number   [      ]    )           =>    119   ,
/*85*/        (        a   ,    b                = 0   ,             ...     c    :    number   [      ]    )           =>    120   ,
/*86*/        (        a )           =>        (        b )           =>        (        c )           =>    121   ,
/*87*/        false       ?            (        a )           =>    0     :        (        b )           =>    122
 /*88*/)      ;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "() => 1;");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "(arg) => 2;");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "arg => 2;");
    f.go_to_marker(t, "3a");
    f.verify_current_line_content(t, "arg => 2;");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "(arg = 1) => 3;");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "(arg?) => 4;");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "(arg: number) => 5;");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "(arg: number = 0) => 6;");
    f.go_to_marker(t, "8");
    f.verify_current_line_content(t, "(arg?: number) => 7;");
    f.go_to_marker(t, "9");
    f.verify_current_line_content(t, "(...arg: number[]) => 8;");
    f.go_to_marker(t, "10");
    f.verify_current_line_content(t, "(arg1, arg2) => 12;");
    f.go_to_marker(t, "11");
    f.verify_current_line_content(t, "(arg1 = 1, arg2 = 3) => 13;");
    f.go_to_marker(t, "12");
    f.verify_current_line_content(t, "(arg1?, arg2?) => 14;");
    f.go_to_marker(t, "13");
    f.verify_current_line_content(t, "(arg1: number, arg2: number) => 15;");
    f.go_to_marker(t, "14");
    f.verify_current_line_content(t, "(arg1: number = 0, arg2: number = 1) => 16;");
    f.go_to_marker(t, "15");
    f.verify_current_line_content(t, "(arg1?: number, arg2?: number) => 17;");
    f.go_to_marker(t, "16");
    f.verify_current_line_content(t, "(arg1, ...arg2: number[]) => 18;");
    f.go_to_marker(t, "17");
    f.verify_current_line_content(t, "(arg1, arg2?: number) => 19;");
    f.go_to_marker(t, "18");
    f.verify_current_line_content(t, "(() => 21);");
    f.go_to_marker(t, "19");
    f.verify_current_line_content(t, "((arg) => 22);");
    f.go_to_marker(t, "20");
    f.verify_current_line_content(t, "((arg = 1) => 23);");
    f.go_to_marker(t, "21");
    f.verify_current_line_content(t, "((arg?) => 24);");
    f.go_to_marker(t, "22");
    f.verify_current_line_content(t, "((arg: number) => 25);");
    f.go_to_marker(t, "23");
    f.verify_current_line_content(t, "((arg: number = 0) => 26);");
    f.go_to_marker(t, "24");
    f.verify_current_line_content(t, "((arg?: number) => 27);");
    f.go_to_marker(t, "25");
    f.verify_current_line_content(t, "((...arg: number[]) => 28);");
    f.go_to_marker(t, "26");
    f.verify_current_line_content(t, "(((((arg) => { return 32; }))));");
    f.go_to_marker(t, "27");
    f.verify_current_line_content(t, "false ? () => 41 : null;");
    f.go_to_marker(t, "28");
    f.verify_current_line_content(t, "false ? (arg) => 42 : null;");
    f.go_to_marker(t, "29");
    f.verify_current_line_content(t, "false ? (arg = 1) => 43 : null;");
    f.go_to_marker(t, "30");
    f.verify_current_line_content(t, "false ? (arg?) => 44 : null;");
    f.go_to_marker(t, "31");
    f.verify_current_line_content(t, "false ? (arg: number) => 45 : null;");
    f.go_to_marker(t, "32");
    f.verify_current_line_content(t, "false ? (arg?: number) => 46 : null;");
    f.go_to_marker(t, "33");
    f.verify_current_line_content(t, "false ? (arg?: number = 0) => 47 : null;");
    f.go_to_marker(t, "34");
    f.verify_current_line_content(t, "false ? (...arg: number[]) => 48 : null;");
    f.go_to_marker(t, "35");
    f.verify_current_line_content(t, "false ? (() => 51) : null;");
    f.go_to_marker(t, "36");
    f.verify_current_line_content(t, "false ? ((arg) => 52) : null;");
    f.go_to_marker(t, "37");
    f.verify_current_line_content(t, "false ? ((arg = 1) => 53) : null;");
    f.go_to_marker(t, "38");
    f.verify_current_line_content(t, "false ? ((arg?) => 54) : null;");
    f.go_to_marker(t, "39");
    f.verify_current_line_content(t, "false ? ((arg: number) => 55) : null;");
    f.go_to_marker(t, "40");
    f.verify_current_line_content(t, "false ? ((arg?: number) => 56) : null;");
    f.go_to_marker(t, "41");
    f.verify_current_line_content(t, "false ? ((arg?: number = 0) => 57) : null;");
    f.go_to_marker(t, "42");
    f.verify_current_line_content(t, "false ? ((...arg: number[]) => 58) : null;");
    f.go_to_marker(t, "43");
    f.verify_current_line_content(t, "false ? null : () => 61;");
    f.go_to_marker(t, "44");
    f.verify_current_line_content(t, "false ? null : (arg) => 62;");
    f.go_to_marker(t, "45");
    f.verify_current_line_content(t, "false ? null : (arg = 1) => 63;");
    f.go_to_marker(t, "46");
    f.verify_current_line_content(t, "false ? null : (arg?) => 64;");
    f.go_to_marker(t, "47");
    f.verify_current_line_content(t, "false ? null : (arg: number) => 65;");
    f.go_to_marker(t, "48");
    f.verify_current_line_content(t, "false ? null : (arg?: number) => 66;");
    f.go_to_marker(t, "49");
    f.verify_current_line_content(t, "false ? null : (arg?: number = 0) => 67;");
    f.go_to_marker(t, "50");
    f.verify_current_line_content(t, "false ? null : (...arg: number[]) => 68;");
    f.go_to_marker(t, "51");
    f.verify_current_line_content(
        t,
        "((a?) => { return a; }) ? (b?) => { return b; } : (c?) => { return c; };",
    );
    f.go_to_marker(t, "52");
    f.verify_current_line_content(
        t,
        "((a?) => { return a; }) ? (b) => (c) => 81 : (c) => (d) => 82;",
    );
    f.go_to_marker(t, "53");
    f.verify_current_line_content(t, "((arg) => 90) instanceof Function;");
    f.go_to_marker(t, "54");
    f.verify_current_line_content(t, "((arg = 1) => 91) instanceof Function;");
    f.go_to_marker(t, "55");
    f.verify_current_line_content(t, "((arg?) => 92) instanceof Function;");
    f.go_to_marker(t, "56");
    f.verify_current_line_content(t, "((arg: number) => 93) instanceof Function;");
    f.go_to_marker(t, "57");
    f.verify_current_line_content(t, "((arg: number = 1) => 94) instanceof Function;");
    f.go_to_marker(t, "58");
    f.verify_current_line_content(t, "((arg?: number) => 95) instanceof Function;");
    f.go_to_marker(t, "59");
    f.verify_current_line_content(t, "((...arg: number[]) => 96) instanceof Function;");
    f.go_to_marker(t, "60");
    f.verify_current_line_content(t, "'' + ((arg) => 100);");
    f.go_to_marker(t, "61");
    f.verify_current_line_content(t, "((arg) => 0) + '' + ((arg) => 101);");
    f.go_to_marker(t, "62");
    f.verify_current_line_content(t, "((arg = 1) => 0) + '' + ((arg = 2) => 102);");
    f.go_to_marker(t, "63");
    f.verify_current_line_content(t, "((arg?) => 0) + '' + ((arg?) => 103);");
    f.go_to_marker(t, "64");
    f.verify_current_line_content(t, "((arg: number) => 0) + '' + ((arg: number) => 104);");
    f.go_to_marker(t, "65");
    f.verify_current_line_content(
        t,
        "((arg: number = 1) => 0) + '' + ((arg: number = 2) => 105);",
    );
    f.go_to_marker(t, "66");
    f.verify_current_line_content(t, "((arg?: number) => 0) + '' + ((arg?: number) => 106);");
    f.go_to_marker(t, "67");
    f.verify_current_line_content(
        t,
        "((...arg: number[]) => 0) + '' + ((...arg: number[]) => 107);",
    );
    f.go_to_marker(t, "68");
    f.verify_current_line_content(t, "((arg1, arg2?) => 0) + '' + ((arg1, arg2?) => 108);");
    f.go_to_marker(t, "69");
    f.verify_current_line_content(
        t,
        "((arg1, ...arg2: number[]) => 0) + '' + ((arg1, ...arg2: number[]) => 108);",
    );
    f.go_to_marker(t, "70");
    f.verify_current_line_content(t, "function foo(...arg: any[]) { }");
    f.go_to_marker(t, "71");
    f.verify_current_line_content(t, "foo(");
    f.go_to_marker(t, "72");
    f.verify_current_line_content(t, "    (a) => 110,");
    f.go_to_marker(t, "73");
    f.verify_current_line_content(t, "    ((a) => 111),");
    f.go_to_marker(t, "74");
    f.verify_current_line_content(t, "    (a) => {");
    f.go_to_marker(t, "75");
    f.verify_current_line_content(t, "        return 112;");
    f.go_to_marker(t, "76");
    f.verify_current_line_content(t, "    },");
    f.go_to_marker(t, "77");
    f.verify_current_line_content(t, "    (a?) => 113,");
    f.go_to_marker(t, "78");
    f.verify_current_line_content(t, "    (a, b?) => 114,");
    f.go_to_marker(t, "79");
    f.verify_current_line_content(t, "    (a: number) => 115,");
    f.go_to_marker(t, "80");
    f.verify_current_line_content(t, "    (a: number = 0) => 116,");
    f.go_to_marker(t, "81");
    f.verify_current_line_content(t, "    (a = 0) => 117,");
    f.go_to_marker(t, "82");
    f.verify_current_line_content(t, "    (a: number = 0) => 118,");
    f.go_to_marker(t, "83");
    f.verify_current_line_content(t, "    (a?, b?: number) => 118,");
    f.go_to_marker(t, "84");
    f.verify_current_line_content(t, "    (...a: number[]) => 119,");
    f.go_to_marker(t, "85");
    f.verify_current_line_content(t, "    (a, b = 0, ...c: number[]) => 120,");
    f.go_to_marker(t, "86");
    f.verify_current_line_content(t, "    (a) => (b) => (c) => 121,");
    f.go_to_marker(t, "87");
    f.verify_current_line_content(t, "    false ? (a) => 0 : (b) => 122");
    done();
}
