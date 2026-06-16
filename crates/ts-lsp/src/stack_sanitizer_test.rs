use regex::Regex;

use crate::sanitize_stack_trace;

// This test uses non-trimmed paths to emulate debug builds.
// Most users won't actually see this.
#[test]
fn test_sanitized_debug_stack_trace_completions_request() {
    let input = r#"goroutine 1196 [running]:
runtime/debug.Stack()
        /usr/local/go/src/runtime/debug/stack.go:26 +0x8e
github.com/microsoft/typescript-go/internal/lsp.(*Server).recover(0xc0001dae08, {0x14bc418, 0xc00bc60960}, 0xc00baf16e0)
        /workspaces/typescript-go/internal/lsp/server.go:777 +0x65
panic({0x1077b40?, 0x1abcb70?})
        /usr/local/go/src/runtime/panic.go:783 +0x136
github.com/microsoft/typescript-go/internal/ls.(*LanguageService).getCompletionData.func15()
        /workspaces/typescript-go/internal/ls/completions.go:1303 +0xfa
github.com/microsoft/typescript-go/internal/ls.(*LanguageService).getCompletionData.func18()
        /workspaces/typescript-go/internal/ls/completions.go:1548 +0x2df
github.com/microsoft/typescript-go/internal/ls.(*LanguageService).getCompletionData(0xc004b08240, {0x14bc418, 0xc00bc60a20}, 0xc0069ef908, 0xc000272008, 0x1b, 0xc002b28e00)
        /workspaces/typescript-go/internal/ls/completions.go:1581 +0x2b92
github.com/microsoft/typescript-go/internal/ls.(*LanguageService).getCompletionsAtPosition(0xc004b08240, {0x14bc418, 0xc00bc60a20}, 0xc000272008, 0x1b, 0x0)
        /workspaces/typescript-go/internal/ls/completions.go:347 +0x690
github.com/microsoft/typescript-go/internal/ls.(*LanguageService).ProvideCompletion(0xc004b08240, {0x14bc418, 0xc00bc60a20}, {0xc0092e02a0, 0x28}, {0x2, 0x4}, 0xc004580c30)
        /workspaces/typescript-go/internal/ls/completions.go:47 +0x207
github.com/microsoft/typescript-go/internal/lsp.(*Server).handleCompletion(0xc0001dae08, {0x14bc418, 0xc00bc60960}, 0xc004b08240, 0xc00baf14d0)
        /workspaces/typescript-go/internal/lsp/server.go:1102 +0xe5
github.com/microsoft/typescript-go/internal/lsp.registerLanguageServiceWithAutoImportsRequestHandler[...].func1({0x14bc418, 0xc00bc60960}, 0xc00baf16e0)
        /workspaces/typescript-go/internal/lsp/server.go:682 +0x32a
github.com/microsoft/typescript-go/internal/lsp.(*Server).handleRequestOrNotification(0xc0001dae08, {0x14bc418, 0xc00bc60960}, 0xc00baf16e0)
        /workspaces/typescript-go/internal/lsp/server.go:531 +0x11e
github.com/microsoft/typescript-go/internal/lsp.(*Server).dispatchLoop.func1()
        /workspaces/typescript-go/internal/lsp/server.go:414 +0x65
created by github.com/microsoft/typescript-go/internal/lsp.(*Server).dispatchLoop in goroutine 19
        /workspaces/typescript-go/internal/lsp/server.go:438 +0x60"#;

    let output = sanitize_stack_trace(input);
    assert!(output.starts_with("(REDACTED FRAME)"));
    assert!(output.contains("typescript-go|>internal|>lsp|>server.go:777"));
    assert!(output.contains("typescript-go|>internal|>ls|>completions.go:1303"));
    assert!(output.contains("(REDACTED FRAME)"));
    assert!(!output.contains("/workspaces/typescript-go/"));
    assert!(!output.contains("/usr/local/go/src/"));
}

#[test]
fn test_sanitized_release_stack_trace_completions_request() {
    let input = r#"runtime error: invalid memory address or nil pointer dereference
goroutine 2331 [running]:
runtime/debug.Stack()
	runtime/debug/stack.go:26 +0x5e
github.com/microsoft/typescript-go/internal/lsp.(*Server).recover(0xc0001c6e08, {0x441ae5?, 0xc000e976c0?}, 0xc00ab6c7b0)
	github.com/microsoft/typescript-go/internal/lsp/server.go:777 +0x58
panic({0xc323a0?, 0x1780b90?})
	runtime/panic.go:783 +0x132
github.com/microsoft/typescript-go/internal/ls.(*LanguageService).getCompletionData.func15()
	github.com/microsoft/typescript-go/internal/ls/completions.go:1303 +0xba
github.com/microsoft/typescript-go/internal/ls.(*LanguageService).getCompletionData.func18(...)
	github.com/microsoft/typescript-go/internal/ls/completions.go:1548
github.com/microsoft/typescript-go/internal/ls.(*LanguageService).getCompletionData(0xc008329200, {0x10f6688, 0xc00c2871d0}, 0xc00190b308, 0xc0001fe008, 0x1b, 0xc0008a2f00)
	github.com/microsoft/typescript-go/internal/ls/completions.go:1581 +0x1ed4
github.com/microsoft/typescript-go/internal/ls.(*LanguageService).getCompletionsAtPosition(0xc008329200, {0x10f6688, 0xc00c2871d0}, 0xc0001fe008, 0x1b, 0x0)
	github.com/microsoft/typescript-go/internal/ls/completions.go:347 +0x35f
github.com/microsoft/typescript-go/internal/ls.(*LanguageService).ProvideCompletion(0xc008329200, {0x10f6688, 0xc00c287110}, {0xc00b472030?, 0xc00c287110?}, {0xb472030?, 0xc0?}, 0xc00c3ea000)
	github.com/microsoft/typescript-go/internal/ls/completions.go:47 +0x11c
github.com/microsoft/typescript-go/internal/lsp.(*Server).handleCompletion(0x418834?, {0x10f6688?, 0xc00c287110?}, 0xc00b472030?, 0x10f6688?)
	github.com/microsoft/typescript-go/internal/lsp/server.go:1105 +0x39
github.com/microsoft/typescript-go/internal/lsp.init.func1.registerLanguageServiceWithAutoImportsRequestHandler[...].28({0x10f6688, 0xc00c287110}, 0xc00ab6c7b0)
	github.com/microsoft/typescript-go/internal/lsp/server.go:682 +0x16c
github.com/microsoft/typescript-go/internal/lsp.(*Server).handleRequestOrNotification(0xc0001c6e08, {0x10f66c0?, 0xc006589180?}, 0xc00ab6c7b0)
	github.com/microsoft/typescript-go/internal/lsp/server.go:531 +0x1c6
github.com/microsoft/typescript-go/internal/lsp.(*Server).dispatchLoop.func1()
	github.com/microsoft/typescript-go/internal/lsp/server.go:414 +0x3a
created by github.com/microsoft/typescript-go/internal/lsp.(*Server).dispatchLoop in goroutine 35
	github.com/microsoft/typescript-go/internal/lsp/server.go:438 +0x9f1"#;

    let output = sanitize_stack_trace(input);
    assert!(output.starts_with("(REDACTED FRAME)"));
    assert!(output.contains("typescript-go|>internal|>lsp|>server.go:777"));
    assert!(output.contains("typescript-go|>internal|>ls|>completions.go:1303"));
    assert!(output.contains("(REDACTED FRAME)"));
    assert!(!output.contains("runtime error: invalid memory address"));
}

fn sanitized_stack_trace_baseline_contents(test_name: &str, input: &str, output: &str) -> String {
    let mut builder = String::new();
    builder.push_str("Test name: `");
    builder.push_str(test_name);
    builder.push_str("`\n\n# Unsanitized input:\n\n````\n");
    builder.push_str(input);
    builder.push_str("\n````\n\n# Sanitized output:\n\n````\n");
    builder.push_str(output);
    builder.push_str("\n````\n");
    builder
}

// Mirror of the "Generic Secret" pattern from VS Code's
// removePropertiesWithPossibleUserInfo. If this matches the sanitized output,
// VS Code's telemetry pipeline will replace the entire string with
// `<REDACTED: Generic Secret>`, destroying the stack trace.
fn vscode_generic_secret_regex() -> Regex {
    Regex::new(
        r"(?i)(key|token|sig|secret|signature|password|passwd|pwd|android:value)[^a-zA-Z0-9]",
    )
    .expect("valid regex")
}

#[test]
fn test_sanitized_stack_trace_defeats_vscode_generic_secret_regex() {
    // Frame names contain identifiers that contain trigger keywords:
    // `getSignatureHelp` (signature), `LookupKey` (key), `validateToken` (token),
    // `signRequest` (sig), `setPwd` (pwd), and a file `signature.go`.
    let input = r#"goroutine 7 [running]:
runtime/debug.Stack()
	runtime/debug/stack.go:26 +0x5e
github.com/microsoft/typescript-go/internal/ls.(*LanguageService).getSignatureHelp(0x1)
	github.com/microsoft/typescript-go/internal/ls/signature.go:42 +0x10
github.com/microsoft/typescript-go/internal/ls.LookupKey(0x2)
	github.com/microsoft/typescript-go/internal/ls/keys.go:7 +0x10
github.com/microsoft/typescript-go/internal/ls.validateToken(0x3)
	github.com/microsoft/typescript-go/internal/ls/token.go:9 +0x10
github.com/microsoft/typescript-go/internal/ls.signRequest(0x4)
	github.com/microsoft/typescript-go/internal/ls/sig.go:11 +0x10
github.com/microsoft/typescript-go/internal/ls.setPwd(0x5)
	github.com/microsoft/typescript-go/internal/ls/pwd.go:13 +0x10"#;

    let output = sanitize_stack_trace(input);
    let regex = vscode_generic_secret_regex();
    assert!(
        regex.find(&output).is_none(),
        "sanitized stack trace would be redacted by VS Code's Generic Secret regex: {output}"
    );
    assert!(output.contains("getSignatureHelp()"));
    assert!(output.contains("LookupKeyX_X()"));
    assert!(output.contains("validateTokenX_X()"));
    assert!(output.contains("signRequest()"));
    assert!(output.contains("setPwdX_X()"));
    assert!(output.contains("signatureX_X.go:42"));
    assert!(output.contains("tokenX_X.go:9"));
    assert!(output.contains("sigX_X.go:11"));
    assert!(output.contains("pwdX_X.go:13"));
}

#[test]
fn test_sanitized_stack_trace_baseline_contents() {
    let input = "before";
    let output = "after";
    let contents = sanitized_stack_trace_baseline_contents("example", input, output);
    assert!(contents.contains("Test name: `example`"));
    assert!(contents.contains("# Unsanitized input:"));
    assert!(contents.contains("before"));
    assert!(contents.contains("# Sanitized output:"));
    assert!(contents.contains("after"));
}
