#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_invalid_codes() {
    let mut t = TestingT;
    run_test_formatting_on_invalid_codes(&mut t);
}

fn run_test_formatting_on_invalid_codes(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/*1*/var a;var c          , b;var  $d
/*2*/var $e
/*3*/var f
/*4*/a++;b++;

/*5*/function        f     (     )        {
/*6*/    for (i = 0; i < 10; i++) {
/*7*/        k = abc + 123 ^ d;
/*8*/        a = XYZ[m  (a[b[c][d]])];
/*9*/        break;

/*10*/        switch ( variable){
/*11*/       case  1: abc += 425;
/*12*/break;
/*13*/case 404 : a [x--/2]%=3 ;
/*14*/                    break ;
/*15*/                case vari : v[--x ] *=++y*( m + n / k[z]);
/*16*/                for (a in b){
/*17*/             for (a = 0; a < 10; ++a) {
/*18*/              a++;--a;
/*19*/                   if (a == b) {
/*20*/                          a++;b--;
/*21*/                     }
/*22*/else
/*23*/if (a == c){
/*24*/++a;
/*25*/(--c)+=d;
/*26*/$c = $a + --$b;
/*27*/}
/*28*/if (a == b)
/*29*/if (a != b) {
/*30*/ if (a !== b)
/*31*/ if (a === b)
/*32*/ --a;
/*33*/ else
/*34*/  --a;
/*35*/  else {
/*36*/  a--;++b;
/*37*/a++
/*38*/                    }
/*39*/                    }
/*40*/                    }
/*41*/                    for (x in y) {
/*42*/m-=m;
/*43*/k=1+2+3+4;
/*44*/}
/*45*/}
/*46*/    break;

/*47*/    }
/*48*/    }
/*49*/    var a  ={b:function(){}};
/*50*/    return {a:1,b:2}
/*51*/}

/*52*/var z = 1;
/*53*/            for (i = 0; i < 10; i++)
/*54*/     for (j = 0; j < 10; j++)
/*55*/for (k = 0; k < 10; ++k) {
/*56*/z++;
/*57*/}

/*58*/for (k = 0; k < 10; k += 2) {
/*59*/z++;
/*60*/}

/*61*/    $(document).ready ();


/*62*/ function  pageLoad() {
/*63*/ $('#TextBox1' ) .     unbind   (  ) ;
/*64*/$('#TextBox1' ) . datepicker ( ) ;
/*65*/}

/*66*/        function pageLoad    (     )    {
/*67*/    var webclass=[
/*68*/                { 'student'     :/*69*/
/*70*/                { 'id': '1', 'name': 'Linda Jones', 'legacySkill': 'Access, VB 5.0' }
/*71*/        }   ,
/*72*/{    'student':/*73*/
/*74*/{'id':'2','name':'Adam Davidson','legacySkill':'Cobol,MainFrame'}
/*75*/}      ,
/*76*/    { 'student':/*77*/
/*78*/{   'id':'3','name':'Charles Boyer' ,'legacySkill':'HTML, XML'}
/*79*/}
/*80*/    ];

/*81*/$create(Sys.UI.DataView,{data:webclass},null,null,$get('SList'));

/*82*/}

/*83*/$( document ).ready(function(){
/*84*/alert('hello');
/*85*/    } ) ;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "var a; var c, b; var $d");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "var $e");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "var f");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "a++; b++;");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "function f() {");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "    for (i = 0; i < 10; i++) {");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "        k = abc + 123 ^ d;");
    f.go_to_marker(t, "8");
    f.verify_current_line_content(t, "        a = XYZ[m(a[b[c][d]])];");
    f.go_to_marker(t, "9");
    f.verify_current_line_content(t, "        break;");
    f.go_to_marker(t, "10");
    f.verify_current_line_content(t, "        switch (variable) {");
    f.go_to_marker(t, "11");
    f.verify_current_line_content(t, "            case 1: abc += 425;");
    f.go_to_marker(t, "12");
    f.verify_current_line_content(t, "                break;");
    f.go_to_marker(t, "13");
    f.verify_current_line_content(t, "            case 404: a[x-- / 2] %= 3;");
    f.go_to_marker(t, "14");
    f.verify_current_line_content(t, "                break;");
    f.go_to_marker(t, "15");
    f.verify_current_line_content(t, "            case vari: v[--x] *= ++y * (m + n / k[z]);");
    f.go_to_marker(t, "16");
    f.verify_current_line_content(t, "                for (a in b) {");
    f.go_to_marker(t, "17");
    f.verify_current_line_content(t, "                    for (a = 0; a < 10; ++a) {");
    f.go_to_marker(t, "18");
    f.verify_current_line_content(t, "                        a++; --a;");
    f.go_to_marker(t, "19");
    f.verify_current_line_content(t, "                        if (a == b) {");
    f.go_to_marker(t, "20");
    f.verify_current_line_content(t, "                            a++; b--;");
    f.go_to_marker(t, "21");
    f.verify_current_line_content(t, "                        }");
    f.go_to_marker(t, "22");
    f.verify_current_line_content(t, "                        else");
    f.go_to_marker(t, "23");
    f.verify_current_line_content(t, "                            if (a == c) {");
    f.go_to_marker(t, "24");
    f.verify_current_line_content(t, "                                ++a;");
    f.go_to_marker(t, "25");
    f.verify_current_line_content(t, "                                (--c) += d;");
    f.go_to_marker(t, "26");
    f.verify_current_line_content(t, "                                $c = $a + --$b;");
    f.go_to_marker(t, "27");
    f.verify_current_line_content(t, "                            }");
    f.go_to_marker(t, "28");
    f.verify_current_line_content(t, "                        if (a == b)");
    f.go_to_marker(t, "29");
    f.verify_current_line_content(t, "                            if (a != b) {");
    f.go_to_marker(t, "30");
    f.verify_current_line_content(t, "                                if (a !== b)");
    f.go_to_marker(t, "31");
    f.verify_current_line_content(t, "                                    if (a === b)");
    f.go_to_marker(t, "32");
    f.verify_current_line_content(t, "                                        --a;");
    f.go_to_marker(t, "33");
    f.verify_current_line_content(t, "                                    else");
    f.go_to_marker(t, "34");
    f.verify_current_line_content(t, "                                        --a;");
    f.go_to_marker(t, "35");
    f.verify_current_line_content(t, "                                else {");
    f.go_to_marker(t, "36");
    f.verify_current_line_content(t, "                                    a--; ++b;");
    f.go_to_marker(t, "37");
    f.verify_current_line_content(t, "                                    a++");
    f.go_to_marker(t, "38");
    f.verify_current_line_content(t, "                                }");
    f.go_to_marker(t, "39");
    f.verify_current_line_content(t, "                            }");
    f.go_to_marker(t, "40");
    f.verify_current_line_content(t, "                    }");
    f.go_to_marker(t, "41");
    f.verify_current_line_content(t, "                    for (x in y) {");
    f.go_to_marker(t, "42");
    f.verify_current_line_content(t, "                        m -= m;");
    f.go_to_marker(t, "43");
    f.verify_current_line_content(t, "                        k = 1 + 2 + 3 + 4;");
    f.go_to_marker(t, "44");
    f.verify_current_line_content(t, "                    }");
    f.go_to_marker(t, "45");
    f.verify_current_line_content(t, "                }");
    f.go_to_marker(t, "46");
    f.verify_current_line_content(t, "                break;");
    f.go_to_marker(t, "47");
    f.verify_current_line_content(t, "        }");
    f.go_to_marker(t, "48");
    f.verify_current_line_content(t, "    }");
    f.go_to_marker(t, "49");
    f.verify_current_line_content(t, "    var a = { b: function() { } };");
    f.go_to_marker(t, "50");
    f.verify_current_line_content(t, "    return { a: 1, b: 2 }");
    f.go_to_marker(t, "51");
    f.verify_current_line_content(t, "}");
    f.go_to_marker(t, "52");
    f.verify_current_line_content(t, "var z = 1;");
    f.go_to_marker(t, "53");
    f.verify_current_line_content(t, "for (i = 0; i < 10; i++)");
    f.go_to_marker(t, "54");
    f.verify_current_line_content(t, "    for (j = 0; j < 10; j++)");
    f.go_to_marker(t, "55");
    f.verify_current_line_content(t, "        for (k = 0; k < 10; ++k) {");
    f.go_to_marker(t, "56");
    f.verify_current_line_content(t, "            z++;");
    f.go_to_marker(t, "57");
    f.verify_current_line_content(t, "        }");
    f.go_to_marker(t, "58");
    f.verify_current_line_content(t, "for (k = 0; k < 10; k += 2) {");
    f.go_to_marker(t, "59");
    f.verify_current_line_content(t, "    z++;");
    f.go_to_marker(t, "60");
    f.verify_current_line_content(t, "}");
    f.go_to_marker(t, "61");
    f.verify_current_line_content(t, "$(document).ready();");
    f.go_to_marker(t, "62");
    f.verify_current_line_content(t, "function pageLoad() {");
    f.go_to_marker(t, "63");
    f.verify_current_line_content(t, "    $('#TextBox1').unbind();");
    f.go_to_marker(t, "64");
    f.verify_current_line_content(t, "    $('#TextBox1').datepicker();");
    f.go_to_marker(t, "65");
    f.verify_current_line_content(t, "}");
    f.go_to_marker(t, "66");
    f.verify_current_line_content(t, "function pageLoad() {");
    f.go_to_marker(t, "67");
    f.verify_current_line_content(t, "    var webclass = [");
    f.go_to_marker(t, "68");
    f.verify_current_line_content(t, "        {");
    f.go_to_marker(t, "69");
    f.verify_current_line_content(t, "            'student':");
    f.go_to_marker(t, "70");
    f.verify_current_line_content(
        t,
        "                { 'id': '1', 'name': 'Linda Jones', 'legacySkill': 'Access, VB 5.0' }",
    );
    f.go_to_marker(t, "71");
    f.verify_current_line_content(t, "        },");
    f.go_to_marker(t, "72");
    f.verify_current_line_content(t, "        {");
    f.go_to_marker(t, "73");
    f.verify_current_line_content(t, "            'student':");
    f.go_to_marker(t, "74");
    f.verify_current_line_content(
        t,
        "                { 'id': '2', 'name': 'Adam Davidson', 'legacySkill': 'Cobol,MainFrame' }",
    );
    f.go_to_marker(t, "75");
    f.verify_current_line_content(t, "        },");
    f.go_to_marker(t, "76");
    f.verify_current_line_content(t, "        {");
    f.go_to_marker(t, "77");
    f.verify_current_line_content(t, "            'student':");
    f.go_to_marker(t, "78");
    f.verify_current_line_content(
        t,
        "                { 'id': '3', 'name': 'Charles Boyer', 'legacySkill': 'HTML, XML' }",
    );
    f.go_to_marker(t, "79");
    f.verify_current_line_content(t, "        }");
    f.go_to_marker(t, "80");
    f.verify_current_line_content(t, "    ];");
    f.go_to_marker(t, "81");
    f.verify_current_line_content(
        t,
        "    $create(Sys.UI.DataView, { data: webclass }, null, null, $get('SList'));",
    );
    f.go_to_marker(t, "82");
    f.verify_current_line_content(t, "}");
    f.go_to_marker(t, "83");
    f.verify_current_line_content(t, "$(document).ready(function() {");
    f.go_to_marker(t, "84");
    f.verify_current_line_content(t, "    alert('hello');");
    f.go_to_marker(t, "85");
    f.verify_current_line_content(t, "});");
    done();
}
