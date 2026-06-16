#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_local_get_references() {
    let mut t = TestingT;
    run_test_local_get_references(&mut t);
}

fn run_test_local_get_references(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: localGetReferences_1.ts
// Comment Refence Test: g/*43*/lobalVar
// References to a variable declared in global.
/*1*/var /*2*/globalVar: number = 2;

class fooCls {
    // References to static variable declared in a class.
    /*3*/static /*4*/clsSVar = 1;
    // References to a variable declared in a class.
    /*5*/clsVar = 1;

    constructor (/*6*/public /*7*/clsParam: number) {
        //Increments
        /*8*/globalVar++;
        this./*9*/clsVar++;
        fooCls./*10*/clsSVar++;
        // References to a class parameter.
        this./*11*/clsParam++;
        modTest.modVar++;
    }
}

// References to a function parameter.
/*12*/function /*13*/foo(/*14*/x: number) {
    // References to a variable declared in a function.
    /*15*/var /*16*/fnVar = 1;

    //Increments
    fooCls./*17*/clsSVar++;
    /*18*/globalVar++;
    modTest.modVar++;
    /*19*/fnVar++;

    //Return
    return /*20*/x++;
}

namespace modTest {
    //Declare
    export var modVar:number;

    //Increments
    /*21*/globalVar++;
    fooCls./*22*/clsSVar++;
    modVar++;

    class testCls {
        static boo = /*23*/foo;
    }

    function testFn(){
        static boo = /*24*/foo;

        //Increments
        /*25*/globalVar++;
        fooCls./*26*/clsSVar++;
        modVar++;
    }

    namespace testMod {
        var boo = /*27*/foo;
    }
}

//Type test
var clsTest: fooCls;

//Arguments
// References to a class argument.
clsTest = new fooCls(/*28*/globalVar);
// References to a function argument.
/*29*/foo(/*30*/globalVar);

//Increments
fooCls./*31*/clsSVar++;
modTest.modVar++;
/*32*/globalVar = /*33*/globalVar + /*34*/globalVar;

//ETC - Other cases
/*35*/globalVar = 3;
// References to illegal assignment.
/*36*/foo = /*37*/foo + 1;
/*44*/err = err++;
/*45*/
//Shadowed fn Parameter
function shdw(/*38*/globalVar: number) {
    //Increments
    /*39*/globalVar++;
    return /*40*/globalVar;
}

//Remotes
//Type test
var remoteclsTest: remotefooCls;

//Arguments
remoteclsTest = new remotefooCls(remoteglobalVar);
remotefoo(remoteglobalVar);

//Increments
remotefooCls.remoteclsSVar++;
remotemodTest.remotemodVar++;
remoteglobalVar = remoteglobalVar + remoteglobalVar;

//ETC - Other cases
remoteglobalVar = 3;

//Find References misses method param
var



 array = ["f", "o", "o"];

array.forEach(


function(/*41*/str) {



   // Reference misses function parameter.
   return /*42*/str + " ";

});
// @Filename: localGetReferences_2.ts
var remoteglobalVar: number = 2;

class remotefooCls {
	//Declare
	remoteclsVar = 1;
	static remoteclsSVar = 1;

	constructor(public remoteclsParam: number) {
		//Increments
		remoteglobalVar++;
		this.remoteclsVar++;
		remotefooCls.remoteclsSVar++;
		this.remoteclsParam++;
		remotemodTest.remotemodVar++;
	}
}

function remotefoo(remotex: number) {
	//Declare
	var remotefnVar = 1;

	//Increments
	remotefooCls.remoteclsSVar++;
	remoteglobalVar++;
	remotemodTest.remotemodVar++;
	remotefnVar++;

	//Return
	return remotex++;
}

namespace remotemodTest {
	//Declare
	export var remotemodVar: number;

	//Increments
	remoteglobalVar++;
	remotefooCls.remoteclsSVar++;
	remotemodVar++;

	class remotetestCls {
		static remoteboo = remotefoo;
	}
` + "`" + `
	function remotetestFn(){
        static remoteboo = remotefoo;

		//Increments
		remoteglobalVar++;
		remotefooCls.remoteclsSVar++;
		remotemodVar++;
    }

	namespace remotetestMod {
		var remoteboo = remotefoo;
	}
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
            "7".to_string(),
            "8".to_string(),
            "9".to_string(),
            "10".to_string(),
            "11".to_string(),
            "12".to_string(),
            "13".to_string(),
            "14".to_string(),
            "15".to_string(),
            "16".to_string(),
            "17".to_string(),
            "18".to_string(),
            "19".to_string(),
            "20".to_string(),
            "21".to_string(),
            "22".to_string(),
            "23".to_string(),
            "24".to_string(),
            "25".to_string(),
            "26".to_string(),
            "27".to_string(),
            "28".to_string(),
            "29".to_string(),
            "30".to_string(),
            "31".to_string(),
            "32".to_string(),
            "33".to_string(),
            "34".to_string(),
            "35".to_string(),
            "36".to_string(),
            "37".to_string(),
            "38".to_string(),
            "39".to_string(),
            "40".to_string(),
            "41".to_string(),
            "42".to_string(),
            "43".to_string(),
            "44".to_string(),
            "45".to_string(),
        ],
    );
    done();
}
