#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_remote_get_references() {
    let mut t = TestingT;
    run_test_remote_get_references(&mut t);
}

fn run_test_remote_get_references(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: remoteGetReferences_1.ts
// Comment Refence Test: globalVar
var globalVar: number = 2;

class fooCls {
    static clsSVar = 1;
    //Declare
    clsVar = 1;

    constructor (public clsParam: number) {
        //Increments
        globalVar++;
        this.clsVar++;
        fooCls.clsSVar++;
        this.clsParam++;
        modTest.modVar++;
    }
}

function foo(x: number) {
    //Declare
    var fnVar = 1;

    //Increments
    fooCls.clsSVar++;
    globalVar++;
    modTest.modVar++;
    fnVar++;

    //Return
    return x++;
}

namespace modTest {
    //Declare
    export var modVar:number;

    //Increments
    globalVar++;
    fooCls.clsSVar++;
    modVar++;

    class testCls {
        static boo = foo;
    }

    function testFn(){
        static boo = foo;

        //Increments
        globalVar++;
        fooCls.clsSVar++;
        modVar++;
    }

    namespace testMod {
        var boo = foo;
    }
}

//Type test
var clsTest: fooCls;

//Arguments
clsTest = new fooCls(globalVar);
foo(globalVar);

//Increments
fooCls.clsSVar++;
modTest.modVar++;
globalVar = globalVar + globalVar;

//ETC - Other cases
globalVar = 3;
foo = foo + 1;
err = err++;

//Shadowed fn Parameter
function shdw(globalVar: number) {
    //Increments
    globalVar++;
    return globalVar;
}

//Remotes
//Type test
var remoteclsTest: /*1*/remotefooCls;

//Arguments
remoteclsTest = new /*2*/remotefooCls(/*3*/remoteglobalVar);
remotefoo(/*4*/remoteglobalVar);

//Increments
/*5*/remotefooCls./*6*/remoteclsSVar++;
remotemodTest.remotemodVar++;
/*7*/remoteglobalVar = /*8*/remoteglobalVar + /*9*/remoteglobalVar;

//ETC - Other cases
/*10*/remoteglobalVar = 3;

//Find References misses method param
var



 array = ["f", "o", "o"];

array.forEach(


function(str) {



   return str + " ";

});
// @Filename: remoteGetReferences_2.ts
/*11*/var /*12*/remoteglobalVar: number = 2;

/*13*/class /*14*/remotefooCls {
	//Declare
	/*15*/remoteclsVar = 1;
	/*16*/static /*17*/remoteclsSVar = 1;

	constructor(public remoteclsParam: number) {
		//Increments
		/*18*/remoteglobalVar++;
		this./*19*/remoteclsVar++;
		/*20*/remotefooCls./*21*/remoteclsSVar++;
		this.remoteclsParam++;
		remotemodTest.remotemodVar++;
	}
}

function remotefoo(remotex: number) {
	//Declare
	var remotefnVar = 1;

	//Increments
	/*22*/remotefooCls./*23*/remoteclsSVar++;
	/*24*/remoteglobalVar++;
	remotemodTest.remotemodVar++;
	remotefnVar++;

	//Return
	return remotex++;
}

namespace remotemodTest {
	//Declare
	export var remotemodVar: number;

	//Increments
	/*25*/remoteglobalVar++;
	/*26*/remotefooCls./*27*/remoteclsSVar++;
	remotemodVar++;

	class remotetestCls {
		static remoteboo = remotefoo;
	}

	function remotetestFn(){
        static remoteboo = remotefoo;

		//Increments
		/*28*/remoteglobalVar++;
		/*29*/remotefooCls./*30*/remoteclsSVar++;
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
        ],
    );
    done();
}
