use crate::{new_fourslash, TestingT};

pub fn test_implementations_across_projects(t: &mut TestingT) {
    let content = r#"
// @stateBaseline: true
// @Filename: /projects/temp/temp.ts
/*temp*/let x = 10
// @Filename: /projects/temp/tsconfig.json
{}
// @Filename: /projects/container/lib/tsconfig.json
{
	"compilerOptions": {
		"composite": true,
	},
	references: [],
	files: [
		"index.ts",
		"bar.ts"
	],
}
// @Filename: /projects/container/lib/index.ts
export interface /*impl*/Foo {
    func();
}
export const val = 42;
// @Filename: /projects/container/lib/bar.ts
import {Foo} from './index'
class A implements Foo {
    func() {}
}
class B implements Foo {
    func() {}
}
// @Filename: /projects/container/exec/tsconfig.json
{
	"files": ["./index.ts"],
	"references": [
		{ "path": "../lib" },
	],
}
// @Filename: /projects/container/exec/index.ts
import { Foo } from "../lib";
class A1 implements Foo {
    func() {}
}
class B1 implements Foo {
    func() {}
}
// @Filename: /projects/container/compositeExec/tsconfig.json
{
	"compilerOptions": {
		"composite": true,
	},
	"files": ["./index.ts"],
	"references": [
		{ "path": "../lib" },
	],
}
// @Filename: /projects/container/compositeExec/index.ts
import { Foo } from "../lib";
class A2 implements Foo {
    func() {}
}
class B2 implements Foo {
    func() {}
}
// @Filename: /projects/container/tsconfig.json
{
	"files": [],
	"include": [],
	"references": [
		{ "path": "./exec" },
		{ "path": "./compositeExec" },
	],
}
// @Filename: /projects/container/tsconfig.json
{
	"files": [],
	"include": [],
	"references": [
		{ "path": "./exec" },
		{ "path": "./compositeExec" },
	],
}
// @Filename: /projects/container/tsconfig.json
{
	"files": [],
	"include": [],
	"references": [
		{ "path": "./exec" },
		{ "path": "./compositeExec" },
	],
}
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "impl");
    // Open temp file and verify all projects alive
    f.go_to_marker(t, "temp");

    // Ref projects are loaded after as part of this command
    f.verify_baseline_go_to_implementation(t, &["impl".to_string()]);

    // Open temp file and verify all projects alive
    f.close_file_of_marker(t, "temp");
    f.go_to_marker(t, "temp");

    // Close all files and open temp file, only inferred project should be alive
    f.close_file_of_marker(t, "impl");
    f.close_file_of_marker(t, "temp");
    f.go_to_marker(t, "temp");
    done();
}

