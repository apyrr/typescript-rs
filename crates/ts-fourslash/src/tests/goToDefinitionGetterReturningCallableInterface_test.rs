use crate::{new_fourslash, TestingT};

pub fn test_go_to_definition_getter_returning_callable_interface(t: &mut TestingT) {
    let content = r#"// @Filename: /home/src/workspaces/project/type.d.ts
export interface DidChangeContentEvent {
    (): void;
}

export declare class TextDocuments {
    get onDidChangeContent(): DidChangeContentEvent;
}

// @Filename: /home/src/workspaces/project/index.ts
import { TextDocuments } from "./type";

declare const documents: TextDocuments | undefined;

documents!./*usage*/onDidChangeContent()"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["usage".to_string()]);
    done();
}

