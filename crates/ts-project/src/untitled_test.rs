use std::collections::HashMap;

use crate::projecttestutil;
use ts_core as core;
use ts_ls as lsconv;
use ts_lsproto::{self as lsproto, DocumentUriExt};

const LANGUAGE_KIND_TYPESCRIPT: &str = "typescript";

#[test]
fn test_untitled_references() {
    if !ts_bundled::EMBEDDED {
        return;
    }

    // First test the URI conversion functions to understand the issue.
    let untitled_uri = lsproto::DocumentUri::from("untitled:Untitled-2");
    let converted_file_name = untitled_uri.file_name();
    let back_to_uri = lsconv::file_name_to_document_uri(&converted_file_name);

    assert_eq!(back_to_uri, untitled_uri);

    // Create a test case that simulates how untitled files should work.
    let test_content = "let x = 42;\n\nx\n\nx++;";

    // Use the converted filename that DocumentURIToFileName would produce.
    let untitled_file_name = converted_file_name;
    assert_eq!(untitled_file_name, "^/untitled/ts-nul-authority/Untitled-2");

    // Set up the file system with an untitled file -
    // But use a regular file first to see the current behavior.
    let files = HashMap::from([("/Untitled-2.ts".to_string(), test_content.to_string())]);

    let (mut session, _) = projecttestutil::setup(files);

    let ctx = projecttestutil::with_request_id(core::Context::default());
    session.did_open_file(
        ctx.clone(),
        "file:///Untitled-2.ts".to_string(),
        1,
        test_content.to_string(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );

    // Get language service.
    let language_service = session
        .get_language_service(ctx.clone(), "file:///Untitled-2.ts".to_string())
        .expect("GetLanguageService should succeed");

    // Test the filename that the source file reports.
    let program = language_service.get_program();
    let source_file = program
        .get_source_file("/Untitled-2.ts")
        .expect("source file should exist");
    assert_eq!(source_file.file_name(), "/Untitled-2.ts");

    // Call ProvideReferences using the LSP method.
    let uri = lsproto::DocumentUri::from("file:///Untitled-2.ts");
    let lsp_position = lsproto::Position {
        line: 2,
        character: 0,
    };

    let ref_params = lsproto::ReferenceParams {
        text_document: lsproto::TextDocumentIdentifier {
            uri: uri.clone().parse().expect("valid document URI"),
        },
        position: lsp_position,
        work_done_token: None,
        partial_result_token: None,
        context: lsproto::ReferenceContext {
            include_declaration: true,
        },
    };

    let resp = language_service
        .provide_references(&ctx, &ref_params, None)
        .expect("ProvideReferences should succeed");

    let refs = resp
        .locations
        .expect("references response should contain locations");

    // We expect to find 3 references.
    assert_eq!(refs.len(), 3, "Expected 3 references, got {}", refs.len());

    // Also test definition using ProvideDefinition.
    let definition = language_service
        .provide_definition(&ctx, uri, lsp_position)
        .expect("ProvideDefinition should succeed");
    if let Some(locations) = definition.locations {
        assert!(
            !locations.is_empty(),
            "definition should include locations when present"
        );
    }
}

#[test]
fn test_untitled_file_in_inferred_project() {
    if !ts_bundled::EMBEDDED {
        return;
    }

    // Test that untitled files are properly handled in inferred projects.
    let test_content = "let x = 42;\n\nx\n\nx++;";

    let (mut session, _) = projecttestutil::setup(HashMap::<String, String>::new());

    let ctx = projecttestutil::with_request_id(core::Context::default());

    // Open untitled files - these should create an inferred project.
    session.did_open_file(
        ctx.clone(),
        "untitled:Untitled-1".to_string(),
        1,
        "x\n\n".to_string(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );
    session.did_open_file(
        ctx.clone(),
        "untitled:Untitled-2".to_string(),
        1,
        test_content.to_string(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );

    let snapshot = session.snapshot();

    // Should have an inferred project.
    assert!(snapshot.project_collection.inferred_project().is_some());

    // Get language service for the untitled file.
    let language_service = session
        .get_language_service(ctx.clone(), "untitled:Untitled-2".to_string())
        .expect("GetLanguageService should succeed");

    let program = language_service.get_program();
    let untitled_file_name = lsproto::DocumentUri::from("untitled:Untitled-2").file_name();
    let source_file = program.get_source_file(&untitled_file_name);
    assert!(source_file.is_some());
    assert_eq!(source_file.unwrap().text(), test_content);

    // Test references on 'x' at position 13 (line 3, after "let x = 42;\n\n").
    let uri = lsproto::DocumentUri::from("untitled:Untitled-2");
    let lsp_position = lsproto::Position {
        line: 2,
        character: 0,
    };

    let ref_params = lsproto::ReferenceParams {
        text_document: lsproto::TextDocumentIdentifier {
            uri: uri.clone().parse().expect("valid document URI"),
        },
        position: lsp_position,
        work_done_token: None,
        partial_result_token: None,
        context: lsproto::ReferenceContext {
            include_declaration: true,
        },
    };

    let resp = language_service
        .provide_references(&ctx, &ref_params, None)
        .expect("ProvideReferences should succeed");

    let refs = resp
        .locations
        .expect("references response should contain locations");
    for reference in &refs {
        // All URIs should be untitled: URIs, not file: URIs.
        assert!(
            reference.uri.starts_with("untitled:"),
            "Expected untitled: URI, got {}",
            reference.uri
        );
    }

    // We expect to find 4 references.
    assert_eq!(refs.len(), 4, "Expected 4 references, got {}", refs.len());
}

#[test]
fn test_imports_in_untitled() {
    if !ts_bundled::EMBEDDED {
        return;
    }

    let files = HashMap::from([(
        format!(
            "{}/node_modules/@types/somelib/index.d.ts",
            projecttestutil::TEST_TYPINGS_LOCATION
        ),
        "export const x: number;".to_string(),
    )]);
    let (mut session, _) = projecttestutil::setup(files);
    let content =
        "import \"https://deno.land/std@0.208.0/path/mod.ts\"\n\t\timport  \"./relative\"\n";
    let uri1 = lsproto::DocumentUri::from("untitled:Untitled-1");
    session.did_open_file(
        core::Context::default(),
        uri1.clone(),
        1,
        content.to_string(),
        LANGUAGE_KIND_TYPESCRIPT.to_string(),
    );

    // 2) Wait for ATA/background tasks to finish, then get a language service for the first file.
    session.wait_for_background_tasks();
    session
        .get_language_service(core::Context::default(), uri1)
        .expect("GetLanguageService should succeed");
}
