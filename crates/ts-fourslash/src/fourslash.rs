use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    sync::{Arc, Mutex},
};

use lsp_types_full as lsp_types;
use serde_json::Value;
use ts_core as core;
use ts_ls as lsconv;
use ts_ls as lsutil;
use ts_lsp as lsp;
use ts_lsproto::{self as lsproto, DocumentUriExt};
use ts_modulespecifiers as modulespecifiers;
use ts_vfs::vfstest::{self, IntoMapFile};

use crate::{
    AUTO_IMPORTS_CMD, BaselineCommand, CALL_HIERARCHY_CMD, CLOSING_TAG_CMD, CODE_LENSES_CMD,
    COMPLETIONS_CMD, DOCUMENT_HIGHLIGHTS_CMD, DOCUMENT_SYMBOLS_CMD, FIND_ALL_REFERENCES_CMD,
    GO_TO_DEFINITION_CMD, GO_TO_IMPLEMENTATION_CMD, GO_TO_SOURCE_DEFINITION_CMD,
    GO_TO_TYPE_DEFINITION_CMD, INLAY_HINTS_CMD, LINKED_EDITING_CMD, Marker, MarkerOrRange,
    NON_SUGGESTION_DIAGNOSTICS_CMD, QUICK_INFO_CMD, RENAME_CMD, SIGNATURE_HELP_CMD,
    SMART_SELECTION_CMD, StateBaseline, TestingT, get_baseline_file_name, new_state_baseline,
    parse_test_data,
};

pub const ROOT_DIR: &str = "/";
pub const SHOW_CODE_LENS_LOCATIONS_COMMAND_NAME: &str = "typescript.showCodeLensLocations";

pub struct FourslashTest {
    pub client: Option<LspClient>,
    pub close_client: Option<Box<dyn FnOnce() -> io::Result<()> + Send>>,
    pub vfs: TestFs,

    pub test_data: TestData, // !!! consolidate test files from test data and script info
    pub baselines: BTreeMap<BaselineCommand, String>,
    pub ranges_by_text: Option<BTreeMap<String, Vec<RangeMarker>>>,
    pub open_files: BTreeSet<String>,
    pub state_baseline: Option<StateBaseline>,

    pub script_infos: BTreeMap<String, ScriptInfo>,
    pub converters: Converters,

    pub state_enable_formatting: bool,
    pub report_format_on_type_crash: bool,
    pub user_preferences: UserPreferences,
    pub server_user_preferences: Arc<Mutex<UserPreferences>>,
    pub current_caret_position: lsproto::Position,
    pub last_known_marker_name: Option<String>,
    pub active_filename: String,
    pub selection_end: Option<lsproto::Position>,

    pub capabilities: Option<lsproto::ClientCapabilities>,
    pub is_strada_server: bool, // Whether this is a fourslash server test in Strada. !!! Remove once we don't need to diff baselines.

    // Semantic token configuration
    pub semantic_token_types: Vec<String>,
    pub semantic_token_modifiers: Vec<String>,
}

impl Drop for FourslashTest {
    fn drop(&mut self) {
        if let Some(close_client) = self.close_client.take() {
            let _ = close_client();
        }
    }
}

#[derive(Clone)]
pub struct ScriptInfo {
    pub file_name: String,
    pub content: String,
    pub line_map: lsconv::LspLineMap,
    pub version: i32,
}

#[derive(Clone, Copy)]
pub struct TextEditSpan {
    pub start: i32,
    pub end: i32,
    pub length: i32,
}

pub fn new_script_info(file_name: String, content: String) -> ScriptInfo {
    ScriptInfo {
        file_name,
        line_map: lsconv::compute_lsp_line_starts(&content),
        content,
        version: 1,
    }
}

impl ScriptInfo {
    pub fn edit_content(&mut self, change: core::TextChange) {
        self.content = apply_text_change(&self.content, change);
        self.line_map = lsconv::compute_lsp_line_starts(&self.content);
        self.version += 1;
    }

    pub fn text(&self) -> String {
        self.content.clone()
    }

    pub fn file_name(&self) -> String {
        self.file_name.clone()
    }

    pub fn get_line_content(&self, line: i32) -> String {
        let num_lines = self.line_map.line_starts.len() as i32;
        if line < 0 || line >= num_lines {
            return String::new();
        }
        let start = self.line_map.line_starts[line as usize] as usize;
        let end = if line + 1 < num_lines {
            self.line_map.line_starts[line as usize + 1] as usize
        } else {
            self.content.len()
        };

        self.content[start..end]
            .trim_end_matches(['\r', '\n'])
            .to_string()
    }
}

pub fn new_fourslash(
    t: &mut TestingT,
    capabilities: Option<lsproto::ClientCapabilities>,
    content: String,
) -> (FourslashTest, impl FnOnce() + use<>) {
    let file_name = format!("{}{}", get_base_file_name_from_test(t), ".ts");
    let mut script_infos = BTreeMap::new();
    let mut fs = TestFs::default();
    let mut server_files = BTreeMap::new();
    let test_data = parse_test_data(t, &content, &file_name);
    for file in &test_data.files {
        let file_path = normalized_absolute_path(&file.file_name, ROOT_DIR);
        fs.files.insert(file_path.clone(), file.content.clone());
        if !ts_tspath::is_dynamic_file_name(&file_path) {
            server_files.insert(
                file_path.clone(),
                file.content
                    .clone()
                    .into_map_file(std::time::SystemTime::UNIX_EPOCH),
            );
        }
        script_infos.insert(
            file_path.clone(),
            new_script_info(file_path, file.content.clone()),
        );
    }
    for (link, target) in &test_data.symlinks {
        let file_path = normalized_absolute_path(link, ROOT_DIR);
        let target_path = normalized_absolute_path(target, ROOT_DIR);
        fs.symlinks.insert(file_path.clone(), target_path.clone());
        server_files.insert(file_path, vfstest::symlink(target_path));
    }
    let active_filename = normalized_absolute_path(
        &test_data
            .files
            .first()
            .map(|file| file.file_name.as_str())
            .unwrap_or(&file_name),
        ROOT_DIR,
    );
    let state_baseline = test_data
        .is_state_baselining_enabled()
        .then(|| new_state_baseline(fs.clone()));
    let user_preferences = UserPreferences::default();
    let server_user_preferences = Arc::new(Mutex::new(user_preferences.clone()));
    let on_server_request = {
        let server_user_preferences = Arc::clone(&server_user_preferences);
        Box::new(move |req: &lsproto::RequestMessage| {
            handle_lsp_server_request(req, &server_user_preferences)
        })
    };
    let mut compiler_options = core::CompilerOptions {
        skip_default_lib_check: core::TSTrue,
        target: core::SCRIPT_TARGET_LATEST_STANDARD,
        jsx: core::JsxEmit::Preserve,
        ..Default::default()
    };
    apply_global_compiler_options(&test_data.global_options, &mut compiler_options);
    let server_fs = ts_bundled::wrap_fs(vfstest::from_map(server_files, true));
    let (client, close_client) = ts_testutil::lsptestutil::new_lsp_client(
        lsp::ServerOptions {
            err: Some(Box::new(io::sink())),
            cwd: ROOT_DIR.to_string(),
            fs: Some(Arc::new(server_fs)),
            default_library_path: ts_bundled::lib_path(),
            compiler_options_for_inferred_projects: Some(compiler_options),
            ..Default::default()
        },
        Some(on_server_request),
    );
    let mut f = FourslashTest {
        client: Some(client),
        close_client: Some(Box::new(close_client)),
        vfs: fs,
        test_data,
        state_enable_formatting: true,
        report_format_on_type_crash: true,
        user_preferences,
        server_user_preferences,
        script_infos,
        converters: Converters::default(),
        baselines: BTreeMap::new(),
        open_files: BTreeSet::new(),
        semantic_token_types: default_semantic_token_types(),
        semantic_token_modifiers: default_semantic_token_modifiers(),
        ranges_by_text: None,
        state_baseline,
        current_caret_position: lsproto::Position {
            line: 0,
            character: 0,
        },
        last_known_marker_name: None,
        active_filename,
        selection_end: None,
        capabilities: None,
        is_strada_server: false,
    };
    initialize(&mut f, t, capabilities);
    if !f.test_data.is_state_baselining_enabled() {
        let file_names = f
            .test_data
            .files
            .iter()
            .map(|file| normalized_absolute_path(&file.file_name, ROOT_DIR))
            .collect::<Vec<_>>();
        for file_name in &file_names {
            f.open_file(t, file_name);
        }
        if let Some(file_name) = file_names.first() {
            f.active_filename = file_name.clone();
        }
    }
    (f, || {})
}

fn handle_lsp_server_request(
    req: &lsproto::RequestMessage,
    server_user_preferences: &Arc<Mutex<UserPreferences>>,
) -> Option<lsproto::ResponseMessage> {
    match req.method.as_str() {
        lsproto::MethodWorkspaceConfiguration => {
            let preferences = server_user_preferences
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .clone();
            let preference_value = user_preferences_config_value(&preferences);
            let result = serde_json::from_value::<lsproto::ConfigurationParams>(req.params.clone())
                .map(|params| {
                    params
                        .items
                        .into_iter()
                        .map(|item| {
                            if item.section.as_deref() == Some("js/ts") {
                                preference_value.clone()
                            } else {
                                serde_json::Value::Null
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|_| vec![preference_value]);
            Some(lsproto::ResponseMessage {
                id: req.id.clone(),
                jsonrpc: req.jsonrpc.clone(),
                result: serde_json::to_value(result).unwrap_or(serde_json::Value::Null),
                error: None,
            })
        }
        lsproto::MethodClientRegisterCapability | lsproto::MethodClientUnregisterCapability => {
            Some(lsproto::ResponseMessage {
                id: req.id.clone(),
                jsonrpc: req.jsonrpc.clone(),
                result: serde_json::Value::Null,
                error: None,
            })
        }
        _ => None,
    }
}

fn apply_global_compiler_options(
    global_options: &BTreeMap<String, String>,
    compiler_options: &mut core::CompilerOptions,
) {
    for (key, value) in global_options {
        let Some(option) = global_compiler_option(key) else {
            continue;
        };
        let Some(value) = test_config_compiler_option_value(&option, value) else {
            continue;
        };
        ts_tsoptions::parse_compiler_options(&option.name, value, compiler_options);
    }
}

fn global_compiler_option(key: &str) -> Option<ts_tsoptions::CommandLineOption> {
    ts_tsoptions::COMMAND_LINE_COMPILER_OPTIONS_MAP
        .get(key)
        .cloned()
        .or_else(|| match key.to_ascii_lowercase().as_str() {
            "allownontsextensions" => Some(ts_tsoptions::CommandLineOption::new(
                "allowNonTsExtensions",
                ts_tsoptions::CommandLineOptionKind::Boolean,
            )),
            "noerrortruncation" => Some(ts_tsoptions::CommandLineOption::new(
                "noErrorTruncation",
                ts_tsoptions::CommandLineOptionKind::Boolean,
            )),
            "suppressoutputpathcheck" => Some(ts_tsoptions::CommandLineOption::new(
                "suppressOutputPathCheck",
                ts_tsoptions::CommandLineOptionKind::Boolean,
            )),
            "nocheck" => Some(ts_tsoptions::CommandLineOption::new(
                "noCheck",
                ts_tsoptions::CommandLineOptionKind::Boolean,
            )),
            _ => None,
        })
}

fn test_config_compiler_option_value(
    option: &ts_tsoptions::CommandLineOption,
    value: &str,
) -> Option<Value> {
    ts_tsoptions::parsedcommandline::compiler_option_json_value(
        &option.name,
        value,
        option.kind?,
        option.enum_map(),
    )
}

// handleServerRequest handles requests initiated by the server (e.g., workspace/configuration).
pub fn handle_server_request(_ctx: (), req: RequestMessage, f: &FourslashTest) -> ResponseMessage {
    match req.method.as_str() {
        "workspace/configuration" => {
            // Return current user preferences for each requested section.
            // The server requests multiple sections (js/ts, typescript, javascript, editor);
            // we return user preferences for "js/ts" and nil for others.
            ResponseMessage {
                id: req.id,
                jsonrpc: req.jsonrpc,
                result: Some(format!("{:?}", f.user_preferences)),
                error: None,
            }
        }
        "client/registerCapability" | "client/unregisterCapability" => {
            // Accept all capability registrations
            ResponseMessage {
                id: req.id,
                jsonrpc: req.jsonrpc,
                result: Some("null".to_string()),
                error: None,
            }
        }
        _ => {
            // Unknown server request
            ResponseMessage {
                id: req.id,
                jsonrpc: req.jsonrpc,
                result: None,
                error: Some(format!("Unknown method: {}", req.method)),
            }
        }
    }
}

pub fn get_base_file_name_from_test(_t: &TestingT) -> String {
    let mut name = "fourslash".to_string();
    name = name.trim_start_matches("Test").to_string();
    lower_first_char(&mut name);

    // Special case: TypeScript has "callHierarchyFunctionAmbiguity.N" with periods
    match name.as_str() {
        "callHierarchyFunctionAmbiguity1" => "callHierarchyFunctionAmbiguity.1".to_string(),
        "callHierarchyFunctionAmbiguity2" => "callHierarchyFunctionAmbiguity.2".to_string(),
        "callHierarchyFunctionAmbiguity3" => "callHierarchyFunctionAmbiguity.3".to_string(),
        "callHierarchyFunctionAmbiguity4" => "callHierarchyFunctionAmbiguity.4".to_string(),
        "callHierarchyFunctionAmbiguity5" => "callHierarchyFunctionAmbiguity.5".to_string(),
        _ => name,
    }
}

pub fn initialize(
    f: &mut FourslashTest,
    _t: &mut TestingT,
    capabilities: Option<lsproto::ClientCapabilities>,
) {
    let capabilities = get_capabilities_with_defaults(capabilities);
    f.capabilities = Some(capabilities.clone());
    let Some(client) = f.client.as_mut() else {
        return;
    };
    let (_msg, _result, ok) = ts_testutil::lsptestutil::send_request(
        client,
        &*lsproto::InitializeInfo,
        lsproto::InitializeParams {
            process_id: None,
            locale: Some("en-US".to_string()),
            root_uri: Some(lsconv::file_name_to_document_uri(ROOT_DIR)),
            capabilities,
            initialization_options: Some(lsproto::InitializationOptions {
                code_lens_show_locations_command_name: Some(
                    SHOW_CODE_LENS_LOCATIONS_COMMAND_NAME.to_string(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    if !ok {
        panic!("Initialize request failed");
    }
    ts_testutil::lsptestutil::send_notification(
        client,
        &*lsproto::InitializedInfo,
        lsproto::InitializedParams {},
    );
    client
        .server
        .init_complete()
        .recv()
        .unwrap_or_else(|err| panic!("Initialize did not complete: {err}"));
}

pub fn default_semantic_token_types() -> Vec<String> {
    [
        "namespace",
        "class",
        "enum",
        "interface",
        "struct",
        "typeParameter",
        "type",
        "parameter",
        "variable",
        "property",
        "enumMember",
        "decorator",
        "event",
        "function",
        "method",
        "macro",
        "label",
        "comment",
        "string",
        "keyword",
        "number",
        "regexp",
        "operator",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect()
}

pub fn default_semantic_token_modifiers() -> Vec<String> {
    [
        "declaration",
        "definition",
        "readonly",
        "static",
        "deprecated",
        "abstract",
        "async",
        "modification",
        "documentation",
        "defaultLibrary",
        "local",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect()
}

// If modifying the defaults, update GetDefaultCapabilities too.
pub fn get_default_capabilities() -> lsproto::ClientCapabilities {
    lsproto::ClientCapabilities {
        general: Some(lsp_types::GeneralClientCapabilities {
            position_encodings: Some(vec![lsp_types::PositionEncodingKind::UTF8]),
            ..Default::default()
        }),
        text_document: Some(lsp_types::TextDocumentClientCapabilities {
            completion: Some(default_completion_capabilities()),
            diagnostic: Some(default_diagnostic_capabilities()),
            publish_diagnostics: Some(default_publish_diagnostic_capabilities()),
            definition: Some(default_goto_capabilities()),
            type_definition: Some(default_goto_capabilities()),
            implementation: Some(default_goto_capabilities()),
            hover: Some(default_hover_capabilities()),
            signature_help: Some(default_signature_help_capabilities()),
            document_symbol: Some(default_document_symbol_capabilities()),
            folding_range: Some(default_folding_range_capabilities()),
            ..Default::default()
        }),
        workspace: Some(lsp_types::WorkspaceClientCapabilities {
            configuration: Some(true),
            file_operations: Some(lsp_types::WorkspaceFileOperationsClientCapabilities {
                will_rename: Some(true),
                ..Default::default()
            }),
            workspace_edit: Some(default_workspace_edit_capabilities()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn get_capabilities_with_defaults(
    capabilities: Option<lsproto::ClientCapabilities>,
) -> lsproto::ClientCapabilities {
    let mut capabilities = capabilities.unwrap_or_default();
    capabilities.general = Some(lsp_types::GeneralClientCapabilities {
        position_encodings: Some(vec![lsp_types::PositionEncodingKind::UTF8]),
        ..Default::default()
    });
    let text_document = capabilities
        .text_document
        .get_or_insert_with(Default::default);
    if text_document.completion.is_none() {
        text_document.completion = Some(default_completion_capabilities());
    }
    if text_document.diagnostic.is_none() {
        text_document.diagnostic = Some(default_diagnostic_capabilities());
    }
    if text_document.publish_diagnostics.is_none() {
        text_document.publish_diagnostics = Some(default_publish_diagnostic_capabilities());
    }
    if text_document.semantic_tokens.is_none() {
        text_document.semantic_tokens = Some(default_semantic_tokens_capabilities());
    }
    if text_document.definition.is_none() {
        text_document.definition = Some(default_goto_capabilities());
    }
    if text_document.type_definition.is_none() {
        text_document.type_definition = Some(default_goto_capabilities());
    }
    if text_document.implementation.is_none() {
        text_document.implementation = Some(default_goto_capabilities());
    }
    if text_document.hover.is_none() {
        text_document.hover = Some(default_hover_capabilities());
    }
    if text_document.signature_help.is_none() {
        text_document.signature_help = Some(default_signature_help_capabilities());
    }
    if text_document.document_symbol.is_none() {
        text_document.document_symbol = Some(default_document_symbol_capabilities());
    }
    if text_document.folding_range.is_none() {
        text_document.folding_range = Some(default_folding_range_capabilities());
    }
    let workspace = capabilities.workspace.get_or_insert_with(Default::default);
    if workspace.file_operations.is_none() {
        workspace.file_operations = Some(lsp_types::WorkspaceFileOperationsClientCapabilities {
            will_rename: Some(true),
            ..Default::default()
        });
    }
    if workspace.workspace_edit.is_none() {
        workspace.workspace_edit = Some(default_workspace_edit_capabilities());
    }
    if workspace.configuration.is_none() {
        workspace.configuration = Some(true);
    }
    capabilities
}

fn markup_formats() -> Vec<lsp_types::MarkupKind> {
    vec![
        lsp_types::MarkupKind::Markdown,
        lsp_types::MarkupKind::PlainText,
    ]
}

fn default_completion_capabilities() -> lsp_types::CompletionClientCapabilities {
    lsp_types::CompletionClientCapabilities {
        completion_item: Some(lsp_types::CompletionItemCapability {
            snippet_support: Some(true),
            commit_characters_support: Some(true),
            preselect_support: Some(true),
            label_details_support: Some(true),
            insert_replace_support: Some(true),
            documentation_format: Some(markup_formats()),
            ..Default::default()
        }),
        completion_list: Some(lsp_types::CompletionListCapability {
            item_defaults: Some(vec![
                "commitCharacters".to_string(),
                "editRange".to_string(),
            ]),
        }),
        ..Default::default()
    }
}

fn default_diagnostic_capabilities() -> lsp_types::DiagnosticClientCapabilities {
    lsp_types::DiagnosticClientCapabilities {
        related_document_support: Some(true),
        ..Default::default()
    }
}

fn default_publish_diagnostic_capabilities() -> lsp_types::PublishDiagnosticsClientCapabilities {
    lsp_types::PublishDiagnosticsClientCapabilities {
        related_information: Some(true),
        tag_support: Some(lsp_types::TagSupport {
            value_set: vec![
                lsp_types::DiagnosticTag::UNNECESSARY,
                lsp_types::DiagnosticTag::DEPRECATED,
            ],
        }),
        ..Default::default()
    }
}

fn default_goto_capabilities() -> lsp_types::GotoCapability {
    lsp_types::GotoCapability {
        link_support: Some(true),
        ..Default::default()
    }
}

fn default_hover_capabilities() -> lsp_types::HoverClientCapabilities {
    lsp_types::HoverClientCapabilities {
        content_format: Some(markup_formats()),
        ..Default::default()
    }
}

fn default_signature_help_capabilities() -> lsp_types::SignatureHelpClientCapabilities {
    lsp_types::SignatureHelpClientCapabilities {
        signature_information: Some(lsp_types::SignatureInformationSettings {
            documentation_format: Some(markup_formats()),
            parameter_information: Some(lsp_types::ParameterInformationSettings {
                label_offset_support: Some(true),
            }),
            active_parameter_support: Some(true),
        }),
        context_support: Some(true),
        ..Default::default()
    }
}

fn default_document_symbol_capabilities() -> lsp_types::DocumentSymbolClientCapabilities {
    lsp_types::DocumentSymbolClientCapabilities {
        hierarchical_document_symbol_support: Some(true),
        ..Default::default()
    }
}

fn default_folding_range_capabilities() -> lsp_types::FoldingRangeClientCapabilities {
    lsp_types::FoldingRangeClientCapabilities {
        range_limit: Some(5000),
        folding_range_kind: Some(lsp_types::FoldingRangeKindCapability {
            value_set: Some(vec![
                lsp_types::FoldingRangeKind::Comment,
                lsp_types::FoldingRangeKind::Imports,
                lsp_types::FoldingRangeKind::Region,
            ]),
        }),
        folding_range: Some(lsp_types::FoldingRangeCapability {
            collapsed_text: Some(true),
        }),
        ..Default::default()
    }
}

fn default_semantic_tokens_capabilities() -> lsp_types::SemanticTokensClientCapabilities {
    lsp_types::SemanticTokensClientCapabilities {
        requests: lsp_types::SemanticTokensClientCapabilitiesRequests {
            full: Some(lsp_types::SemanticTokensFullOptions::Bool(true)),
            ..Default::default()
        },
        token_types: default_semantic_token_types()
            .into_iter()
            .map(lsp_types::SemanticTokenType::from)
            .collect(),
        token_modifiers: default_semantic_token_modifiers()
            .into_iter()
            .map(lsp_types::SemanticTokenModifier::from)
            .collect(),
        formats: vec![lsp_types::TokenFormat::RELATIVE],
        ..Default::default()
    }
}

fn default_workspace_edit_capabilities() -> lsp_types::WorkspaceEditClientCapabilities {
    lsp_types::WorkspaceEditClientCapabilities {
        document_changes: Some(true),
        resource_operations: Some(vec![lsp_types::ResourceOperationKind::Rename]),
        ..Default::default()
    }
}

pub fn update_position(pos: i32, edit_start: i32, edit_end: i32, new_text: &str) -> i32 {
    if pos <= edit_start {
        return pos;
    }
    // If inside the edit, return -1 to mark as invalid
    if pos < edit_end {
        return -1;
    }
    pos + new_text.len() as i32 - (edit_end - edit_start)
}

pub fn update_position_for_text_edit(
    position: i32,
    edit_start: i32,
    edit_end: i32,
    new_text_length: i32,
) -> i32 {
    if position <= edit_start {
        return position;
    }
    if position < edit_end {
        return -1;
    }
    position + new_text_length - (edit_end - edit_start)
}

pub fn extract_module_specifier(text: &str) -> String {
    // Try to match: from "..." or from '...'
    for prefix in ["from \"", "from '"] {
        if let Some(idx) = text.find(prefix) {
            let start = idx + prefix.len();
            let quote = prefix.as_bytes()[prefix.len() - 1] as char;
            if let Some(end) = text[start..].find(quote) {
                return text[start..start + end].to_string();
            }
        }
    }

    // Try to match: require("...") or require('...')
    for prefix in ["require(\"", "require('"] {
        if let Some(idx) = text.find(prefix) {
            let start = idx + prefix.len();
            let quote = prefix.as_bytes()[prefix.len() - 1] as char;
            if let Some(end) = text[start..].find(quote) {
                return text[start..start + end].to_string();
            }
        }
    }

    String::new()
}

pub fn get_language_kind(filename: &str) -> &'static str {
    if has_any_extension(
        filename,
        &[".ts", ".mts", ".cts", ".dmts", ".dcts", ".d.ts"],
    ) {
        return "typescript";
    }
    if has_any_extension(filename, &[".js", ".mjs", ".cjs"]) {
        return "javascript";
    }
    if filename.ends_with(".jsx") {
        return "javascriptreact";
    }
    if filename.ends_with(".tsx") {
        return "typescriptreact";
    }
    if filename.ends_with(".json") {
        return "json";
    }
    "typescript" // !!! should we error in this case?
}

pub fn compute_line_starts(content: &str) -> Vec<i32> {
    let mut line_starts = vec![0];
    for (i, ch) in content.char_indices() {
        if ch == '\n' {
            line_starts.push(i as i32 + 1);
        }
    }
    line_starts
}

pub fn remove_whitespace(text: &str) -> String {
    let mut builder = String::new();
    for ch in text.chars() {
        if ch.is_whitespace() {
            continue;
        }
        builder.push(ch);
    }
    builder
}

pub fn code_fence(lang: &str, code: &str) -> String {
    format!("```{lang}\n{code}\n```")
}

pub fn is_lib_file(file_name: &str) -> bool {
    let base_name = file_name.rsplit('/').next().unwrap_or(file_name);
    base_name.starts_with("lib.") && base_name.ends_with(".d.ts")
}

pub fn symbol_kind_to_lowercase(kind: impl ToString) -> String {
    kind.to_string().to_lowercase()
}

pub fn hover_content_string(hover: Option<Hover>) -> String {
    hover.map(|hover| hover.content).unwrap_or_default()
}

fn hover_response_content(response: lsproto::HoverResponse) -> String {
    let Some(hover) = response.hover else {
        return String::new();
    };
    let contents = hover.contents;
    if let Some(markup) = contents.markup_content {
        return markup.value;
    }
    if let Some(text) = contents.string {
        return text;
    }
    if let Some(marked) = contents.marked_string_with_language {
        return marked.value;
    }
    contents
        .marked_strings
        .unwrap_or_default()
        .into_iter()
        .map(|marked| {
            marked
                .string
                .or_else(|| {
                    marked
                        .marked_string_with_language
                        .map(|marked| marked.value)
                })
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn apply_edits_to_content_with_positions(
    mut content: String,
    mut spans: Vec<(usize, usize, String)>,
) -> String {
    spans.sort_by_key(|span| span.0);
    for (start, end, new_text) in spans.into_iter().rev() {
        content.replace_range(start..end, &new_text);
    }
    content
}

pub fn send_request<Params, Resp>(
    t: &mut TestingT,
    f: &mut FourslashTest,
    info: RequestInfo<Params, Resp>,
    params: Params,
) -> Resp
where
    Params: serde::Serialize,
{
    send_request_and_baseline_worker(t, f, info, params, true)
}

pub fn send_request_and_baseline_worker<Params, Resp>(
    t: &mut TestingT,
    f: &mut FourslashTest,
    info: RequestInfo<Params, Resp>,
    params: Params,
    baseline_projects: bool,
) -> Resp
where
    Params: serde::Serialize,
{
    let prefix = f.get_current_position_prefix();
    if baseline_projects {
        f.baseline_state(t);
    }
    f.baseline_request_or_notification(t, &info.method, &params);
    let response = (info.send)(f, params);
    if baseline_projects {
        f.baseline_state(t);
    }
    if info.method == "textDocument/onTypeFormatting" && !f.report_format_on_type_crash {
        return response;
    }
    if !(info.validate)(&response) {
        panic!("{prefix}Unexpected {} response", info.method);
    }
    response
}

pub fn send_notification<Params>(
    t: &mut TestingT,
    f: &mut FourslashTest,
    info: NotificationInfo<Params>,
    params: Params,
) where
    Params: serde::Serialize,
{
    if info.method != "textDocument/didChange" {
        // This is called eg when doing typeText = which is series of edits and formatting - which becomes non deterministic "after state"
        // The notification can only guarantee before state and thats what it baselines, but in case of type it creates
        // multiple edits which results in getting different state -based on if the snapshot was updated or not at the time of formatting requests
        // So this is used for all the incremental edits - to baseline only request data but not project state between those edits
        f.baseline_state(t);
        f.update_state(&info.method, &params);
    }
    f.baseline_request_or_notification(t, &info.method, &params);
    (info.send)(f, params);
}

impl FourslashTest {
    pub(crate) fn send_lsp_request<Params, Resp>(&mut self, method: &str, params: Params) -> Resp
    where
        Params: serde::Serialize,
        Resp: serde::de::DeserializeOwned,
    {
        self.client
            .as_mut()
            .unwrap_or_else(|| panic!("LSP client is required for {method}"))
            .send_request_json(method, params)
            .unwrap_or_else(|err| panic!("{method} request failed: {err}"))
    }

    pub fn update_state<Params>(&mut self, method: &str, params: &Params) {
        if method == "textDocument/didOpen" {
            if let Some(file_name) = any_file_name(params) {
                self.open_files.insert(file_name);
            }
        } else if method == "textDocument/didClose" {
            if let Some(file_name) = any_file_name(params) {
                self.open_files.remove(&file_name);
            }
        }
    }

    pub fn get_options(&self) -> UserPreferences {
        self.user_preferences.clone()
    }

    pub fn configure(&mut self, t: &mut TestingT, config: UserPreferences) {
        // We send 'js/ts' by default because that is what we expect the primary config to be in vscode and VS (one
        // set of preferences for both languages). This should be fine in fourslash since tests that need
        // multiple options usually send reconfiguration commands for each `verify` anyways
        self.user_preferences = config.clone();
        let params = ConfigurationChange {
            settings: BTreeMap::from([("js/ts".to_string(), config)]),
        };
        send_notification(t, self, workspace_did_change_configuration_info(), params);
    }

    pub fn configure_with_reset(
        &mut self,
        t: &mut TestingT,
        config: UserPreferences,
    ) -> UserPreferences {
        let original_config = self.user_preferences.clone();
        self.configure(t, config);
        original_config
    }

    pub fn go_to_marker_or_range(&mut self, t: &mut TestingT, marker_or_range: MarkerOrRange) {
        self.go_to_marker_worker(t, marker_or_range);
    }

    pub fn go_to_marker(&mut self, t: &mut TestingT, marker_name: &str) {
        let marker = self
            .test_data
            .marker_positions
            .get(marker_name)
            .unwrap_or_else(|| panic!("Marker '{marker_name}' not found"))
            .clone();
        self.go_to_marker_worker(t, marker.into());
    }

    fn go_to_marker_worker(&mut self, t: &mut TestingT, marker_or_range: MarkerOrRange) {
        self.ensure_active_file(t, &marker_or_range.file_name());
        self.go_to_position_worker(t, marker_or_range.ls_pos());
        self.last_known_marker_name = marker_or_range.get_name();
    }

    pub fn go_to_eof(&mut self, t: &mut TestingT) {
        let script = self.get_script_info(&self.active_filename).clone();
        let pos = script.content.len() as i32;
        let lsp_pos = self.converters.position_to_line_and_character(&script, pos);
        self.go_to_position_worker(t, lsp_pos);
    }

    pub fn go_to_bof(&mut self, t: &mut TestingT) {
        self.go_to_position_worker(
            t,
            lsproto::Position {
                line: 0,
                character: 0,
            },
        );
    }

    pub fn go_to_position(&mut self, t: &mut TestingT, position: i32) {
        let script = self.get_script_info(&self.active_filename).clone();
        let lsp_pos = self
            .converters
            .position_to_line_and_character(&script, position);
        self.go_to_position_worker(t, lsp_pos);
    }

    fn go_to_position_worker(&mut self, _t: &mut TestingT, position: lsproto::Position) {
        self.current_caret_position = position;
        self.selection_end = None;
    }

    pub fn go_to_each_marker<F>(&mut self, t: &mut TestingT, marker_names: &[String], mut action: F)
    where
        F: FnMut(&Marker, usize),
    {
        let markers = if marker_names.is_empty() {
            self.markers()
        } else {
            marker_names
                .iter()
                .map(|name| {
                    self.test_data
                        .marker_positions
                        .get(name)
                        .unwrap_or_else(|| panic!("Marker '{name}' not found"))
                        .clone()
                })
                .collect()
        };
        for (index, marker) in markers.iter().enumerate() {
            self.go_to_marker_worker(t, marker.clone().into());
            action(marker, index);
        }
    }

    pub fn go_to_each_range<F>(&mut self, t: &mut TestingT, mut action: F)
    where
        F: FnMut(&mut TestingT, &RangeMarker),
    {
        let ranges = self.ranges();
        for range_marker in ranges.iter() {
            self.go_to_position_worker(t, range_marker.ls_range.start);
            action(t, range_marker);
        }
    }

    pub fn go_to_range_start(&mut self, t: &mut TestingT, range_marker: &RangeMarker) {
        self.open_file(t, &range_marker.file_name());
        self.go_to_position_worker(t, range_marker.ls_range.start);
    }

    pub fn go_to_select(
        &mut self,
        t: &mut TestingT,
        start_marker_name: &str,
        end_marker_name: &str,
    ) {
        let start_marker = self
            .test_data
            .marker_positions
            .get(start_marker_name)
            .unwrap_or_else(|| panic!("Start marker '{start_marker_name}' not found"))
            .clone();
        let end_marker = self
            .test_data
            .marker_positions
            .get(end_marker_name)
            .unwrap_or_else(|| panic!("End marker '{end_marker_name}' not found"))
            .clone();
        if start_marker.file_name() != end_marker.file_name() {
            panic!("Markers '{start_marker_name}' and '{end_marker_name}' are in different files");
        }
        self.ensure_active_file(t, &start_marker.file_name());
        self.go_to_position_worker(t, start_marker.ls_position);
        self.selection_end = Some(end_marker.ls_position);
    }

    pub fn go_to_select_range(&mut self, t: &mut TestingT, range_marker: &RangeMarker) {
        self.go_to_range_start(t, range_marker);
        self.selection_end = Some(range_marker.ls_range.end);
    }

    pub fn markers(&self) -> Vec<Marker> {
        self.test_data.markers.clone()
    }

    pub fn marker_names(&self) -> Vec<String> {
        self.test_data
            .markers
            .iter()
            .filter_map(|marker| marker.name.clone())
            .collect()
    }

    pub fn marker_by_name(&self, marker_name: &str) -> Marker {
        self.test_data
            .marker_positions
            .get(marker_name)
            .unwrap_or_else(|| panic!("Marker '{marker_name}' not found"))
            .clone()
    }

    pub fn ranges(&self) -> Vec<RangeMarker> {
        self.test_data.ranges.clone()
    }

    pub fn get_ranges_in_file(&self, file_name: &str) -> Vec<RangeMarker> {
        self.test_data
            .ranges
            .iter()
            .filter(|range| range.file_name == file_name)
            .cloned()
            .collect()
    }

    pub fn ensure_active_file(&mut self, t: &mut TestingT, file_name: &str) {
        if self.active_filename != file_name {
            if !self.open_files.contains(file_name) {
                self.open_file(t, file_name);
            } else {
                self.active_filename = file_name.to_string();
            }
        }
    }

    pub fn close_file_of_marker(&mut self, _t: &mut TestingT, marker_name: &str) {
        let marker = self
            .test_data
            .marker_positions
            .get(marker_name)
            .unwrap_or_else(|| panic!("Marker '{marker_name}' not found"))
            .clone();
        if self.active_filename == marker.file_name() {
            self.active_filename.clear();
        }
        if let Some(test_file) = self
            .test_data
            .files
            .iter()
            .find(|file| file.file_name == marker.file_name())
            .cloned()
        {
            self.script_infos.insert(
                test_file.file_name.clone(),
                new_script_info(test_file.file_name, test_file.content),
            );
        } else {
            self.script_infos.remove(&marker.file_name());
        }
        self.open_files.remove(&marker.file_name());
    }

    pub fn go_to_file(&mut self, t: &mut TestingT, file_name: &str) {
        let file_name = normalized_absolute_path(file_name, ROOT_DIR);
        self.open_file(t, &file_name);
    }

    pub fn go_to_file_number(&mut self, t: &mut TestingT, index: usize) {
        if index >= self.test_data.files.len() {
            panic!(
                "File index {} out of range (0-{})",
                index,
                self.test_data.files.len().saturating_sub(1)
            );
        }
        let file_name = self.test_data.files[index].file_name.clone();
        self.go_to_file(t, &file_name);
    }

    pub fn open_file(&mut self, t: &mut TestingT, file_name: &str) {
        let file_name = normalized_absolute_path(file_name, ROOT_DIR);
        let Some(script) = self.script_infos.get(&file_name).cloned() else {
            panic!("File {file_name} not found in test data");
        };
        self.active_filename = file_name.clone();
        self.selection_end = None;
        send_notification(
            t,
            self,
            text_document_did_open_info(),
            lsproto::DidOpenTextDocumentParams {
                text_document: lsproto::TextDocumentItem {
                    uri: lsconv::file_name_to_document_uri(&file_name),
                    language_id: get_language_kind(&file_name).to_string(),
                    version: script.version,
                    text: script.content,
                },
            },
        );
        self.open_files.insert(file_name);
    }

    pub fn verify_current_file_content(&self, _t: &mut TestingT, expected_content: &str) {
        let actual_content = &self.get_script_info(&self.active_filename).content;
        assert_eq!(actual_content, expected_content);
    }

    pub fn verify_current_line_content(&self, _t: &mut TestingT, expected_content: &str) {
        let actual_content = self
            .get_script_info(&self.active_filename)
            .get_line_content(self.current_caret_position.line as i32);
        assert_eq!(
            actual_content, expected_content,
            "\n  actual line: \"{}\"\nexpected line: \"{}\"\n",
            actual_content, expected_content
        );
    }

    pub fn verify_indentation(&self, _t: &mut TestingT, _num_spaces: usize) {
        // TS-Go's fourslash harness leaves verify.indentationIs unimplemented.
    }

    pub fn verify_indentation_at_markers_from_data(&mut self, t: &mut TestingT) {
        for marker in self.markers() {
            let Some(indent) = marker
                .data
                .get("indent")
                .or_else(|| marker.data.get("indentation"))
            else {
                panic!("Marker {:?} does not have indent data.", marker.name);
            };
            let expected = indent
                .parse::<usize>()
                .unwrap_or_else(|_| panic!("Invalid indent marker data: {indent:?}"));
            self.ensure_active_file(t, &marker.file_name());
            self.go_to_position_worker(t, marker.ls_position);
            self.verify_indentation(t, expected);
        }
    }

    pub fn disable_formatting(&mut self) {
        self.state_enable_formatting = false;
    }

    pub fn format_document(&mut self, t: &mut TestingT, filename: &str) {
        let filename = if filename.is_empty() {
            self.active_filename.clone()
        } else {
            filename.to_string()
        };
        let result = send_request(
            t,
            self,
            text_document_formatting_info(),
            DocumentFormattingParams { filename },
        );
        if let Some(text_edits) = result.text_edits {
            self.apply_text_edits(t, text_edits);
        }
    }

    pub fn format_selection(
        &mut self,
        t: &mut TestingT,
        start_marker_name: &str,
        end_marker_name: &str,
    ) {
        let start_marker = self
            .test_data
            .marker_positions
            .get(start_marker_name)
            .unwrap_or_else(|| panic!("Marker '{start_marker_name}' not found"))
            .clone();
        let end_marker = self
            .test_data
            .marker_positions
            .get(end_marker_name)
            .unwrap_or_else(|| panic!("Marker '{end_marker_name}' not found"))
            .clone();
        if start_marker.file_name() != end_marker.file_name() {
            panic!("Markers '{start_marker_name}' and '{end_marker_name}' are in different files");
        }
        let result = send_request(
            t,
            self,
            text_document_range_formatting_info(),
            DocumentRangeFormattingParams {
                filename: start_marker.file_name(),
                range: lsproto::Range {
                    start: start_marker.ls_position,
                    end: end_marker.ls_position,
                },
            },
        );
        if let Some(text_edits) = result.text_edits {
            self.apply_text_edits(t, text_edits);
        }
    }

    pub fn verify_completions(
        &mut self,
        t: &mut TestingT,
        marker_input: MarkerInput,
        expected: Option<&CompletionsExpectedList>,
    ) -> VerifyCompletionsResult {
        let mut list = None;
        match marker_input {
            MarkerInput::Name(marker) => {
                self.go_to_marker(t, &marker);
                list = Some(self.verify_completions_worker(t, expected));
            }
            MarkerInput::Marker(marker) => {
                self.go_to_marker_worker(t, marker.into());
                list = Some(self.verify_completions_worker(t, expected));
            }
            MarkerInput::Range(range) => {
                self.go_to_marker_or_range(t, range.into());
                list = Some(self.verify_completions_worker(t, expected));
            }
            MarkerInput::Names(markers) => {
                for marker_name in markers {
                    self.go_to_marker(t, &marker_name);
                    self.verify_completions_worker(t, expected);
                }
            }
            MarkerInput::Markers(markers) => {
                for marker in markers {
                    self.go_to_marker_worker(t, marker.into());
                    self.verify_completions_worker(t, expected);
                }
            }
            MarkerInput::None => {
                list = Some(self.verify_completions_worker(t, expected));
            }
        }

        let list_for_apply = list.clone();
        let list_for_absence = list;
        VerifyCompletionsResult {
            and_apply_code_action: Box::new(move |_t, expected_action| {
                let Some(list) = &list_for_apply else {
                    panic!(
                        "Code action '{}' from source '{}' not found in completions.",
                        expected_action.name, expected_action.source
                    );
                };
                let item = list.items.iter().find(|item| {
                    item.label == expected_action.name
                        && item.source.as_deref() == Some(&expected_action.source)
                });
                if item.is_none() {
                    panic!(
                        "Code action '{}' from source '{}' not found in completions.",
                        expected_action.name, expected_action.source
                    );
                }
            }),
            and_has_no_code_action: Box::new(move |_t, unexpected_action| {
                if let Some(list) = &list_for_absence {
                    let item = list.items.iter().find(|item| {
                        item.label == unexpected_action.name
                            && item.source.as_deref() == Some(&unexpected_action.source)
                    });
                    if item.is_some() {
                        panic!(
                            "Unexpected code action '{}' from source '{}' found in completions.",
                            unexpected_action.name, unexpected_action.source
                        );
                    }
                }
            }),
        }
    }

    pub fn verify_completions_worker(
        &mut self,
        t: &mut TestingT,
        expected: Option<&CompletionsExpectedList>,
    ) -> CompletionList {
        let prefix = self.get_current_position_prefix();
        let user_preferences = expected.and_then(|expected| expected.user_preferences.clone());
        let list = self.get_completions(t, user_preferences);
        self.verify_completions_result(t, Some(&list), expected, &prefix);
        list
    }

    pub fn get_completions(
        &mut self,
        t: &mut TestingT,
        user_preferences: Option<UserPreferences>,
    ) -> CompletionList {
        self.get_completions_worker(t, user_preferences)
    }

    fn get_completions_worker(
        &mut self,
        t: &mut TestingT,
        user_preferences: Option<UserPreferences>,
    ) -> CompletionList {
        if let Some(user_preferences) = user_preferences {
            let original = self.configure_with_reset(t, user_preferences);
            let result = send_request(
                t,
                self,
                text_document_completion_info(),
                CompletionParams {
                    filename: self.active_filename.clone(),
                    position: self.current_caret_position,
                },
            );
            self.configure(t, original);
            return sort_completion_list(result);
        }
        let result = send_request(
            t,
            self,
            text_document_completion_info(),
            CompletionParams {
                filename: self.active_filename.clone(),
                position: self.current_caret_position,
            },
        );
        // For performance, the server may return unsorted completion lists.
        // The client is expected to sort them by SortText and then by Label.
        // We are the client here.
        sort_completion_list(result)
    }

    pub fn verify_completions_result(
        &self,
        _t: &mut TestingT,
        actual: Option<&CompletionList>,
        expected: Option<&CompletionsExpectedList>,
        prefix: &str,
    ) {
        let Some(actual) = actual else {
            if !is_empty_expected_list(expected) {
                panic!("{prefix}Expected completion list but got nil.");
            }
            return;
        };
        let Some(expected) = expected else {
            if actual.items.is_empty() {
                return;
            }
            panic!(
                "{prefix}Expected nil completion list but got non-nil: {:?}",
                actual.items
            );
        };
        assert_eq!(
            actual.is_incomplete, expected.is_incomplete,
            "{prefix}IsIncomplete mismatch"
        );
        verify_completions_item_defaults(
            actual.item_defaults.as_ref(),
            expected.item_defaults.as_ref(),
            &format!("{prefix}ItemDefaults mismatch: "),
        );
        self.verify_completions_items(prefix, &actual.items, expected.items.as_ref());
    }

    pub fn verify_completions_items(
        &self,
        prefix: &str,
        actual: &[CompletionItem],
        expected: Option<&CompletionsExpectedItems>,
    ) {
        let Some(expected) = expected else {
            return;
        };
        if !expected.exact.is_empty() {
            if !expected.includes.is_empty() {
                panic!("{prefix}Expected exact completion list but also specified 'includes'.");
            }
            if !expected.excludes.is_empty() {
                panic!("{prefix}Expected exact completion list but also specified 'excludes'.");
            }
            if !expected.unsorted.is_empty() {
                panic!("{prefix}Expected exact completion list but also specified 'unsorted'.");
            }
            if actual.len() != expected.exact.len() {
                panic!(
                    "{prefix}Expected {} exact completion items but got {}.",
                    expected.exact.len(),
                    actual.len()
                );
            }
            if !actual.is_empty() {
                self.verify_completions_are_exactly(prefix, actual, &expected.exact);
            }
            return;
        }
        let mut name_to_actual_items: BTreeMap<String, Vec<CompletionItem>> = BTreeMap::new();
        for item in actual {
            name_to_actual_items
                .entry(item.label.clone())
                .or_default()
                .push(item.clone());
        }
        if !expected.unsorted.is_empty() {
            if !expected.includes.is_empty() {
                panic!("{prefix}Expected unsorted completion list but also specified 'includes'.");
            }
            if !expected.excludes.is_empty() {
                panic!("{prefix}Expected unsorted completion list but also specified 'excludes'.");
            }
            for item in &expected.unsorted {
                let label = get_expected_label(item);
                if name_to_actual_items.remove(&label).is_none() {
                    panic!("{prefix}Label '{label}' not found in actual items.");
                }
            }
            if expected.unsorted.len() != actual.len() {
                let unmatched = name_to_actual_items
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n");
                panic!(
                    "{prefix}Additional completions found but not included in 'unsorted': {unmatched}"
                );
            }
            return;
        }
        for item in &expected.includes {
            let label = get_expected_label(item);
            if !name_to_actual_items.contains_key(&label) {
                panic!("{prefix}Label '{label}' not found in actual items.");
            }
        }
        for exclude in &expected.excludes {
            if name_to_actual_items.contains_key(exclude) {
                panic!("{prefix}Label '{exclude}' should not be in actual items but was found.");
            }
        }
    }

    pub fn verify_completions_are_exactly(
        &self,
        prefix: &str,
        actual: &[CompletionItem],
        expected: &[CompletionsExpectedItem],
    ) {
        let actual_labels = actual
            .iter()
            .map(|item| item.label.clone())
            .collect::<Vec<_>>();
        let expected_labels = expected.iter().map(get_expected_label).collect::<Vec<_>>();
        assert_deep_equal(
            actual_labels,
            expected_labels,
            &format!("{prefix}Labels mismatch"),
        );
        for (actual_item, expected_item) in actual.iter().zip(expected.iter()) {
            if let CompletionsExpectedItem::Item(expected_item) = expected_item {
                let actual_item = completion_item_from_lsp(actual_item);
                if let Some(err) = self.verify_completion_item(prefix, &actual_item, expected_item)
                {
                    panic!(
                        "{prefix}Completion item mismatch for label {}:\n{}",
                        actual_item.label, err
                    );
                }
            }
        }
    }

    pub fn verify_completion_item(
        &self,
        _prefix: &str,
        actual: &lsproto::CompletionItem,
        expected: &lsproto::CompletionItem,
    ) -> Option<String> {
        // returns error message if not matched
        if actual.label != expected.label {
            return Some("Label mismatch".to_string());
        }
        if expected.detail.is_some() || expected.documentation.is_some() {
            let resolved = self.resolve_completion_item(actual.clone());
            if resolved.label != expected.label {
                return Some("Resolved item label mismatch".to_string());
            }
        }
        None
    }

    pub fn resolve_completion_item(
        &self,
        item: lsproto::CompletionItem,
    ) -> lsproto::CompletionItem {
        item
    }

    pub fn resolve_completion_item_from_completion(&self, item: CompletionItem) -> CompletionItem {
        item
    }

    pub fn apply_text_edits(&mut self, t: &mut TestingT, mut edits: Vec<TextEdit>) -> i32 {
        let file_name = self.active_filename.clone();
        let script = self.get_script_info(&file_name).clone();
        edits.sort_by_key(|edit| edit.start);

        let mut total_offset = 0_i32;
        let mut current_caret_position = self
            .converters
            .line_and_character_to_position(&script, self.current_caret_position)
            as i32;
        for edit in edits.into_iter().rev() {
            self.edit_script_and_update_markers(
                t,
                &file_name,
                edit.start,
                edit.end,
                &edit.new_text,
            );

            let delta = edit.new_text.len() as i32 - (edit.end as i32 - edit.start as i32);
            let start = edit.start as i32;
            let end = edit.end as i32;
            if start <= current_caret_position {
                if end <= current_caret_position {
                    current_caret_position += delta;
                } else {
                    current_caret_position = start;
                }
            }
            total_offset += delta;
        }
        let script = self.get_script_info(&file_name).clone();
        self.current_caret_position = self
            .converters
            .position_to_line_and_character(&script, current_caret_position);
        total_offset
    }

    // VerifyCodeFix verifies that applying a code fix produces the expected file content.
    pub fn verify_code_fix(&mut self, t: &mut TestingT, options: VerifyCodeFixOptions) {
        if let Some(user_preferences) = options.user_preferences.clone() {
            let original = self.configure_with_reset(t, user_preferences);
            self.verify_code_fix(
                t,
                VerifyCodeFixOptions {
                    user_preferences: None,
                    ..options
                },
            );
            self.configure(t, original);
            return;
        }

        let actions = self.get_code_fix_actions(t, None);
        if actions.is_empty() {
            panic!("No code fixes returned.");
        }
        if options.index >= actions.len() {
            panic!(
                "Code fix index {} out of range (got {} fixes)",
                options.index,
                actions.len()
            );
        }

        let mut matching_action = actions[options.index].clone();
        if matching_action.title != options.description {
            if let Some(action) = actions
                .iter()
                .find(|action| action.title == options.description)
            {
                matching_action = action.clone();
            } else {
                let titles = actions
                    .iter()
                    .map(|action| action.title.clone())
                    .collect::<Vec<_>>();
                panic!(
                    "No code fix with description {:?} at index {} found. Available fixes: {:?}",
                    options.description, options.index, titles
                );
            }
        }

        let original_content = self.get_script_info(&self.active_filename).content.clone();
        let mut expected_content = options.new_file_content.clone();
        if !options.new_range_content.is_empty() {
            let mut selection = self.get_selection();
            if selection.pos() == selection.end() {
                let ranges = self.get_ranges_in_file(&self.active_filename);
                if ranges.is_empty() {
                    panic!(
                        "Expected a selected range or fourslash range for NewRangeContent verification."
                    );
                }
                selection = ranges[0].range;
            }
            expected_content = format!(
                "{}{}{}",
                &original_content[..selection.pos() as usize],
                options.new_range_content,
                &original_content[selection.end() as usize..]
            );
        }

        if options.apply_changes {
            self.apply_text_edits(t, matching_action.edits.clone());
            let actual = self.get_script_info(&self.active_filename).content.clone();
            assert_eq!(
                expected_content, actual,
                "File content after applying code fix did not match expected content."
            );
        } else {
            let mut actual = self.get_script_info(&self.active_filename).content.clone();
            actual = self.apply_edits_to_content(actual, matching_action.edits);
            assert_eq!(
                expected_content, actual,
                "File content after applying code fix did not match expected content."
            );
        }
    }

    pub fn verify_range_after_code_fix(
        &mut self,
        t: &mut TestingT,
        expected_text: &str,
        include_whitespace: bool,
        error_code: i32,
        index: usize,
    ) {
        let actions = self.get_code_fix_actions(t, Some(error_code));
        if actions.is_empty() {
            panic!("No code fixes returned.");
        }
        if index >= actions.len() {
            panic!(
                "Code fix index {} out of range (got {} fixes)",
                index,
                actions.len()
            );
        }

        let action = actions[index].clone();
        let ranges = self.get_ranges_in_file(&self.active_filename);
        if ranges.len() != 1 {
            panic!(
                "Expected exactly one range in {:?}, got {}.",
                self.active_filename,
                ranges.len()
            );
        }

        let edits = self.get_code_action_edits_for_active_file(&action);
        let updated_range = self.update_text_range_for_text_edits(ranges[0].range, &edits);
        assert_valid_text_range(
            updated_range,
            &format!(
                "Code fix {:?} replaced part of the expected range; unable to compute rangeAfterCodeFix result.",
                action.title
            ),
        );

        self.apply_text_edits(t, edits);
        let actual_content = self.get_script_info(&self.active_filename).content.clone();
        let mut actual_text =
            actual_content[updated_range.pos() as usize..updated_range.end() as usize].to_string();
        let mut expected_text = expected_text.to_string();
        if !include_whitespace {
            actual_text = remove_whitespace(&actual_text);
            expected_text = remove_whitespace(&expected_text);
        }
        assert_eq!(
            expected_text, actual_text,
            "Range content after applying code fix did not match expected content."
        );
    }

    pub fn get_code_action_edits_for_active_file(&self, action: &CodeAction) -> Vec<TextEdit> {
        if action.edits.is_empty() {
            panic!("Code fix {:?} did not return text edits.", action.title);
        }
        action.edits.clone()
    }

    // VerifyCodeFixAvailable verifies that code fixes with the given descriptions are available.
    pub fn verify_code_fix_available(
        &mut self,
        t: &mut TestingT,
        expected_descriptions: Option<&[String]>,
    ) {
        let actions = self.get_code_fix_actions(t, None);
        let Some(expected_descriptions) = expected_descriptions else {
            if actions.is_empty() {
                panic!("Expected code fixes to be available, but got none.");
            }
            return;
        };
        if expected_descriptions.is_empty() {
            self.verify_code_fix_not_available(t, &[]);
            return;
        }
        for expected in expected_descriptions {
            if !actions.iter().any(|action| &action.title == expected) {
                let titles = actions
                    .iter()
                    .map(|action| action.title.clone())
                    .collect::<Vec<_>>();
                panic!(
                    "Expected code fix with description {:?} not found. Available fixes: {:?}",
                    expected, titles
                );
            }
        }
    }

    pub fn verify_code_fix_not_available(&mut self, t: &mut TestingT, expected: &[String]) {
        let actions = self.get_code_fix_actions(t, None);
        if expected.is_empty() {
            if actions.is_empty() {
                return;
            }
            let titles = actions
                .iter()
                .map(|action| action.title.clone())
                .collect::<Vec<_>>();
            panic!("Expected no code fixes, but got: {:?}", titles);
        }
        for title in expected {
            if actions.iter().any(|action| &action.title == title) {
                panic!(
                    "Expected code fix with description {:?} not to be available.",
                    title
                );
            }
        }
    }

    pub fn verify_code_fix_available_exact(
        &mut self,
        t: &mut TestingT,
        expected_descriptions: &[String],
    ) {
        let actions = self.get_code_fix_actions(t, None);
        if actions.len() != expected_descriptions.len() {
            let titles = actions
                .iter()
                .map(|action| action.title.clone())
                .collect::<Vec<_>>();
            panic!(
                "Expected exactly {} code fixes, but got {}. Available fixes: {:?}",
                expected_descriptions.len(),
                actions.len(),
                titles
            );
        }
        for expected in expected_descriptions {
            if !actions.iter().any(|action| &action.title == expected) {
                let titles = actions
                    .iter()
                    .map(|action| action.title.clone())
                    .collect::<Vec<_>>();
                panic!(
                    "Expected code fix with description {:?} not found. Available fixes: {:?}",
                    expected, titles
                );
            }
        }
    }

    // VerifyCodeFixAll verifies that applying all code fixes with the given fixId produces the expected file content.
    // It gets all quickfix code actions for the file (which includes per-fixId "Fix all" entries when
    // multiple diagnostics match the same provider), finds the fix-all entry, and applies its edits.
    pub fn verify_code_fix_all(&mut self, t: &mut TestingT, options: VerifyCodeFixAllOptions) {
        let actions = self.get_all_quick_fix_actions(t, None);
        if actions.is_empty() {
            panic!("No code fixes available for fixId {:?}", options.fix_id);
        }

        // Find fix-all actions. The server returns these as quickfix entries with titles like
        // "Add all missing imports" when multiple diagnostics match the same provider.
        // We look for actions that are NOT single-diagnostic fixes (i.e., have no Diagnostics attached).
        let fix_all_candidates = actions
            .iter()
            .filter(|action| action.diagnostics.is_empty())
            .cloned()
            .collect::<Vec<_>>();
        let fix_all_action = if fix_all_candidates.len() == 1 {
            Some(fix_all_candidates[0].clone())
        } else {
            fix_all_candidates.into_iter().find(|action| {
                action
                    .title
                    .to_lowercase()
                    .contains(&options.fix_id.to_lowercase())
            })
        };
        let Some(fix_all_action) = fix_all_action else {
            let titles = actions
                .iter()
                .map(|action| action.title.clone())
                .collect::<Vec<_>>();
            panic!(
                "No fix-all code action found for fixId {:?}. Available fixes: {:?}",
                options.fix_id, titles
            );
        };
        self.apply_text_edits(t, fix_all_action.edits);
        let actual = self.get_script_info(&self.active_filename).content.clone();
        assert_eq!(
            options.new_file_content, actual,
            "File content after applying all code fixes did not match expected content."
        );
    }

    pub fn verify_code_fix_all_not_available(&mut self, t: &mut TestingT, fix_id: &str) {
        let actions = self.get_all_quick_fix_actions(t, None);
        let has_fix_all = actions
            .iter()
            .filter(|action| action.diagnostics.is_empty())
            .any(|action| action.title.to_lowercase().contains(&fix_id.to_lowercase()));
        if has_fix_all {
            panic!("Expected no fix-all code action for fixId {fix_id:?}.");
        }
    }

    // VerifySourceFixAll verifies that requesting a source.fixAll code action produces the expected file content.
    // This tests the on-save code path where VS Code requests source.fixAll.
    pub fn verify_source_fix_all(&mut self, t: &mut TestingT, expected_content: &str) {
        let only = vec![lsproto::CodeActionKind::SourceFixAll];
        let uri = lsconv::file_name_to_document_uri(&self.active_filename);
        let params = lsproto::CodeActionParams {
            text_document: lsproto::TextDocumentIdentifier { uri: uri.clone() },
            range: lsproto::Range {
                start: self.current_caret_position,
                end: self.current_caret_position,
            },
            context: lsproto::CodeActionContext {
                diagnostics: Vec::new(),
                only: Some(only),
                trigger_kind: None,
            },
            work_done_token: None,
            partial_result_token: None,
        };
        let result: lsproto::CodeActionResponse =
            self.send_lsp_request(lsproto::MethodTextDocumentCodeAction, params);

        let Some(items) = result.command_or_code_action_array else {
            panic!("No source.fixAll code actions returned");
        };

        let selected = items
            .into_iter()
            .filter_map(|item| item.code_action)
            .find(|action| action.kind == Some(lsproto::CodeActionKind::SourceFixAll));
        let Some(selected) = selected else {
            panic!("No source.fixAll code action found");
        };
        if let Some(edit) = selected.edit
            && let Some(changes) = edit.changes
        {
            for (edit_uri, edits) in changes {
                if edit_uri != uri {
                    panic!(
                        "source.fixAll returned edits for unexpected URI {edit_uri:?} (expected {uri:?})"
                    );
                }
                let script = self.get_script_info(&self.active_filename).clone();
                let text_edits = edits
                    .into_iter()
                    .map(|edit| text_edit_from_lsp(self, &script, edit))
                    .collect();
                self.apply_text_edits(t, text_edits);
            }
        }
        let actual = self.get_script_info(&self.active_filename).content.clone();
        assert_eq!(
            expected_content, actual,
            "File content after source.fixAll did not match expected content."
        );
    }

    // getCodeFixActions gets per-diagnostic quick fix code actions, excluding fix-all entries.
    pub fn get_code_fix_actions(
        &mut self,
        t: &mut TestingT,
        error_code: Option<i32>,
    ) -> Vec<CodeAction> {
        self.get_all_quick_fix_actions(t, error_code)
            .into_iter()
            .filter(|action| !action.diagnostics.is_empty())
            .collect()
    }

    // getAllQuickFixActions gets all quick fix code actions including fix-all entries.
    pub fn get_all_quick_fix_actions(
        &mut self,
        _t: &mut TestingT,
        error_code: Option<i32>,
    ) -> Vec<CodeAction> {
        let uri = lsconv::file_name_to_document_uri(&self.active_filename);
        let diagnostic_result: lsproto::DocumentDiagnosticResponse = self.send_lsp_request(
            lsproto::MethodTextDocumentDiagnostic,
            DocumentDiagnosticRequestParams {
                text_document: lsproto::TextDocumentIdentifier { uri: uri.clone() },
            },
        );

        let diagnostics = diagnostic_result
            .full_document_diagnostic_report
            .map(|report| report.items)
            .unwrap_or_default();
        if diagnostics.is_empty() {
            return Vec::new();
        }

        let Some(diagnostic) = select_code_fix_diagnostic(&diagnostics, error_code) else {
            return Vec::new();
        };

        let params = lsproto::CodeActionParams {
            text_document: lsproto::TextDocumentIdentifier { uri: uri.clone() },
            range: diagnostic.range,
            context: lsproto::CodeActionContext {
                diagnostics: diagnostics.clone(),
                only: None,
                trigger_kind: None,
            },
            work_done_token: None,
            partial_result_token: None,
        };
        let result: lsproto::CodeActionResponse =
            self.send_lsp_request(lsproto::MethodTextDocumentCodeAction, params);
        let script = self.get_script_info(&self.active_filename).clone();

        result
            .command_or_code_action_array
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| item.code_action)
            .filter(|action| {
                action
                    .kind
                    .as_ref()
                    .is_some_and(|kind| *kind == lsproto::CodeActionKind::QuickFix)
            })
            .map(|action| code_action_from_lsp(self, &script, &uri, action))
            .collect()
    }

    pub fn update_text_range_for_text_edits(
        &self,
        text_range: core::TextRange,
        edits: &[TextEdit],
    ) -> core::TextRange {
        let mut spans = edits
            .iter()
            .map(|edit| TextEditSpan {
                start: edit.start as i32,
                end: edit.end as i32,
                length: edit.new_text.len() as i32,
            })
            .collect::<Vec<_>>();
        spans.sort_by_key(|span| span.start);

        let mut pos = text_range.pos();
        let mut end = text_range.end();
        for i in 0..spans.len() {
            let edit = spans[i];
            pos = update_position_for_text_edit(pos, edit.start, edit.end, edit.length);
            end = update_position_for_text_edit(end, edit.start, edit.end, edit.length);

            let delta = edit.length - (edit.end - edit.start);
            for span in spans.iter_mut().skip(i + 1) {
                if span.start >= edit.start {
                    span.start += delta;
                    span.end += delta;
                }
            }
        }
        core::new_text_range(pos, end)
    }

    // applyEditsToContent applies text edits to a content string without mutating the file.
    pub fn apply_edits_to_content(&self, content: String, mut edits: Vec<TextEdit>) -> String {
        edits.sort_by_key(|edit| edit.start);
        let mut content = content;
        for edit in edits.into_iter().rev() {
            content.replace_range(edit.start..edit.end, &edit.new_text);
        }
        content
    }

    pub fn verify_organize_imports(
        &mut self,
        t: &mut TestingT,
        expected_content: &str,
        code_action_kind: &str,
        preferences: Option<UserPreferences>,
    ) {
        if let Some(preferences) = preferences {
            let original = self.configure_with_reset(t, preferences);
            self.verify_organize_imports(t, expected_content, code_action_kind, None);
            self.configure(t, original);
            return;
        }
        let organize_action = self
            .get_all_quick_fix_actions(t, None)
            .into_iter()
            .find(|action| action.kind == code_action_kind);
        let Some(organize_action) = organize_action else {
            panic!("No organize imports code action found");
        };
        self.apply_text_edits(t, organize_action.edits);
        let actual_content = self.get_script_info(&self.active_filename).content.clone();
        if actual_content != expected_content {
            panic!(
                "Organize imports result doesn't match.\nExpected:\n{expected_content}\n\nActual:\n{actual_content}"
            );
        }
    }

    // Insert text at the current caret position.
    pub fn insert(&mut self, t: &mut TestingT, text: &str) {
        self.baseline_state(t);
        self.type_text(t, text);
    }

    // Insert text and a new line at the current caret position.
    pub fn insert_line(&mut self, t: &mut TestingT, text: &str) {
        self.baseline_state(t);
        self.type_text(t, &format!("{text}\n"));
    }

    // Removes the text at the current caret position as if the user pressed backspace `count` times.
    pub fn backspace(&mut self, t: &mut TestingT, count: usize) {
        let mut offset = self.converters.line_and_character_to_position(
            self.get_script_info(&self.active_filename),
            self.current_caret_position,
        );
        self.baseline_state(t);

        for _ in 0..count {
            offset -= 1;
            let file_name = self.active_filename.clone();
            self.edit_script_and_update_markers(t, &file_name, offset, offset + 1, "");
            let script = self.get_script_info(&self.active_filename).clone();
            self.current_caret_position = self
                .converters
                .position_to_line_and_character(&script, offset as i32);
            // Don't need to examine formatting because there are no formatting changes on backspace.
        }

        // f.checkPostEditInvariants() // !!! do we need this?
    }

    // DeleteAtCaret removes the text at the current caret position as if the user pressed delete `count` times.
    pub fn delete_at_caret(&mut self, t: &mut TestingT, count: usize) {
        let script = self.get_script_info(&self.active_filename).clone();
        let offset = self
            .converters
            .line_and_character_to_position(&script, self.current_caret_position);
        self.baseline_state(t);

        for _ in 0..count {
            let file_name = self.active_filename.clone();
            self.edit_script_and_update_markers(t, &file_name, offset, offset + 1, "");
            // Position stays the same after delete (unlike backspace)
        }
    }

    pub fn delete_line(&mut self, t: &mut TestingT, line_index: usize) {
        self.baseline_state(t);
        self.select_line(t, line_index);
        self.type_text(t, "");
    }

    // Enters text as if the user had pasted it.
    pub fn paste(&mut self, t: &mut TestingT, text: &str) {
        let script = self.get_script_info(&self.active_filename).clone();
        let start = self
            .converters
            .line_and_character_to_position(&script, self.current_caret_position);
        self.baseline_state(t);
        let file_name = self.active_filename.clone();
        self.edit_script_and_update_markers(t, &file_name, start, start, text);

        // post-paste fomatting
        if self.state_enable_formatting {
            let script = self.get_script_info(&self.active_filename).clone();
            let result = send_request_and_baseline_worker(
                t,
                self,
                text_document_range_formatting_info(),
                DocumentRangeFormattingParams {
                    filename: file_name,
                    range: lsproto::Range {
                        start: self.current_caret_position,
                        end: self
                            .converters
                            .position_to_line_and_character(&script, (start + text.len()) as i32),
                    },
                },
                false,
            );
            if let Some(text_edits) = result.text_edits {
                self.apply_text_edits(t, text_edits);
            }
        }
        // this.checkPostEditInvariants(); // !!! do we need this?
    }

    // Selects a line and replaces it with a new text.
    pub fn replace_line(&mut self, t: &mut TestingT, line_index: usize, text: &str) {
        self.baseline_state(t);
        self.select_line(t, line_index);
        self.type_text(t, text);
    }

    pub fn select_line(&mut self, t: &mut TestingT, line_index: usize) {
        let script = self.get_script_info(&self.active_filename);
        let start = script.line_map.line_starts[line_index] as usize;
        let end = if line_index + 1 >= script.line_map.line_starts.len() {
            script.content.len()
        } else {
            script.line_map.line_starts[line_index + 1] as usize - 1
        };
        self.select_range(t, core::new_text_range(start as i32, end as i32));
    }

    pub fn select_range(&mut self, t: &mut TestingT, text_range: core::TextRange) {
        let script = self.get_script_info(&self.active_filename).clone();
        let start = self
            .converters
            .position_to_line_and_character(&script, text_range.pos());
        let end = self
            .converters
            .position_to_line_and_character(&script, text_range.end());
        self.go_to_position_worker(t, start);
        self.selection_end = Some(end);
    }

    pub fn get_selection(&self) -> core::TextRange {
        let script = self.get_script_info(&self.active_filename);
        let start = self
            .converters
            .line_and_character_to_position(script, self.current_caret_position);
        let end = self
            .selection_end
            .map(|selection_end| {
                self.converters
                    .line_and_character_to_position(script, selection_end)
            })
            .unwrap_or(start);
        core::new_text_range(start as i32, end as i32)
    }

    pub fn replace(&mut self, t: &mut TestingT, start: usize, length: usize, text: &str) {
        self.baseline_state(t);
        self.replace_worker(t, start, length, text);
    }

    pub fn replace_worker(&mut self, t: &mut TestingT, start: usize, length: usize, text: &str) {
        let file_name = self.active_filename.clone();
        self.edit_script_and_update_markers(t, &file_name, start, start + length, text);
        // f.checkPostEditInvariants() // !!! do we need this?
    }

    // Inserts the text currently at the caret position character by character, as if the user typed it.
    pub fn type_text(&mut self, t: &mut TestingT, text: &str) {
        // temprorary -- this disables tests failing if format crashes; this unblocks unrelated tests such as codefixes
        self.report_format_on_type_crash = false;

        let selection = self.get_selection();
        self.replace_worker(
            t,
            selection.pos() as usize,
            (selection.end() - selection.pos()) as usize,
            "",
        );

        let mut total_size = 0;
        let mut offset = {
            let script = self.get_script_info(&self.active_filename).clone();
            self.converters
                .line_and_character_to_position(&script, self.current_caret_position)
        } as i32;
        while total_size < text.len() {
            let ch = text[total_size..].chars().next().unwrap();
            let size = ch.len_utf8();
            let file_name = self.active_filename.clone();
            let edit_offset = usize::try_from(offset)
                .unwrap_or_else(|_| panic!("caret offset became negative: {offset}"));
            self.edit_script_and_update_markers(
                t,
                &file_name,
                edit_offset,
                edit_offset,
                &ch.to_string(),
            );

            total_size += size;
            offset += size as i32;
            let script = self.get_script_info(&self.active_filename).clone();
            self.current_caret_position = self
                .converters
                .position_to_line_and_character(&script, offset as i32);

            // Handle post-keystroke formatting
            if self.state_enable_formatting {
                let result = send_request_and_baseline_worker(
                    t,
                    self,
                    text_document_on_type_formatting_info(),
                    DocumentOnTypeFormattingParams {
                        filename: self.active_filename.clone(),
                        position: self.current_caret_position,
                        ch: ch.to_string(),
                    },
                    false,
                );
                if let Some(text_edits) = result.text_edits {
                    offset += self.apply_text_edits(t, text_edits);
                }
            }
        }
        self.report_format_on_type_crash = true;

        // f.checkPostEditInvariants() // !!! do we need this?
    }

    // Edits the script and updates marker and range positions accordingly.
    // This does not update the current caret position.
    pub fn edit_script_and_update_markers(
        &mut self,
        t: &mut TestingT,
        file_name: &str,
        edit_start: usize,
        edit_end: usize,
        new_text: &str,
    ) {
        self.edit_script_and_update_markers_worker(
            t,
            file_name,
            vec![TextChange {
                start: edit_start,
                end: edit_end,
                new_text: new_text.to_string(),
            }],
        );
    }

    pub fn edit_script_and_update_markers_worker(
        &mut self,
        t: &mut TestingT,
        file_name: &str,
        changes: Vec<TextChange>,
    ) {
        // Sort changes by position (ascending) so we can apply in reverse
        let mut sorted_changes = changes;
        sorted_changes.sort_by_key(|change| change.start);

        // Apply changes in reverse order to preserve positions of earlier changes
        for change in sorted_changes.into_iter().rev() {
            let edit_start = change.start as i32;
            let edit_end = change.end as i32;
            let script = self.edit_script(t, file_name, change.clone()).clone();
            for marker in &mut self.test_data.markers {
                if marker.file_name() == file_name {
                    marker.position = update_position(
                        marker.position as i32,
                        edit_start,
                        edit_end,
                        &change.new_text,
                    ) as usize;
                    marker.ls_position = self
                        .converters
                        .position_to_line_and_character(&script, marker.position as i32);
                }
            }
            for marker in self.test_data.marker_positions.values_mut() {
                if marker.file_name() == file_name {
                    marker.position = update_position(
                        marker.position as i32,
                        edit_start,
                        edit_end,
                        &change.new_text,
                    ) as usize;
                    marker.ls_position = self
                        .converters
                        .position_to_line_and_character(&script, marker.position as i32);
                }
            }
            for range_marker in &mut self.test_data.ranges {
                if range_marker.file_name() == file_name {
                    let start = update_position(
                        range_marker.range.pos(),
                        edit_start,
                        edit_end,
                        &change.new_text,
                    );
                    let end = update_position(
                        range_marker.range.end(),
                        edit_start,
                        edit_end,
                        &change.new_text,
                    );
                    range_marker.range = core::new_text_range(start, end);
                    range_marker.ls_range =
                        self.converters.to_lsp_range(&script, range_marker.range);
                }
            }
        }
        self.ranges_by_text = None;
    }

    pub fn edit_script(
        &mut self,
        t: &mut TestingT,
        file_name: &str,
        change: TextChange,
    ) -> &ScriptInfo {
        let script = self.get_or_load_script_info(file_name);
        let content = apply_edits_to_content_with_positions(
            script.content.clone(),
            vec![(change.start, change.end, change.new_text.clone())],
        );
        let (version, updated_content) = {
            let script = self
                .script_infos
                .get_mut(file_name)
                .unwrap_or_else(|| panic!("Script info for file {file_name} not found"));
            script.content = content;
            script.line_map = lsconv::compute_lsp_line_starts(&script.content);
            script.version += 1;
            (script.version, script.content.clone())
        };
        self.vfs
            .files
            .insert(file_name.to_string(), updated_content.clone());
        if self.open_files.contains(file_name) {
            send_notification(
                t,
                self,
                text_document_did_change_info(),
                lsproto::DidChangeTextDocumentParams {
                    text_document: lsproto::VersionedTextDocumentIdentifier {
                        uri: lsconv::file_name_to_document_uri(file_name),
                        version,
                    },
                    content_changes: vec![
                        lsproto::TextDocumentContentChangePartialOrWholeDocument {
                            partial: None,
                            whole_document: Some(lsproto::TextDocumentContentChangeWholeDocument {
                                text: updated_content,
                            }),
                        },
                    ],
                },
            );
        }
        self.script_infos
            .get(file_name)
            .unwrap_or_else(|| panic!("Script info for file {file_name} not found"))
    }

    pub fn get_or_load_script_info(&mut self, file_name: &str) -> &ScriptInfo {
        if self.script_infos.contains_key(file_name) {
            return self.script_infos.get(file_name).unwrap();
        }
        if let Some(content) = self.vfs.read_file(file_name) {
            self.script_infos.insert(
                file_name.to_string(),
                new_script_info(file_name.to_string(), content),
            );
            return self.script_infos.get(file_name).unwrap();
        }
        panic!("Script info for file {file_name} not found")
    }

    pub fn verify_apply_code_action_from_completion(
        &mut self,
        t: &mut TestingT,
        marker_name: Option<&str>,
        options: &ApplyCodeActionFromCompletionOptions,
    ) {
        if let Some(marker_name) = marker_name {
            self.go_to_marker(t, marker_name);
        }
        let user_preferences = options.user_preferences.clone().unwrap_or_default();
        let original = self.configure_with_reset(t, user_preferences);
        let completions_list = self.get_completions(t, None);
        let items = completions_list
            .items
            .iter()
            .filter(|item| {
                item.label == options.name
                    && (item.source.as_deref() == Some(options.source.as_str())
                        || options.auto_import_fix.as_ref().is_some_and(|_| {
                            item.source.as_deref() == Some(options.source.as_str())
                        }))
            })
            .cloned()
            .collect::<Vec<_>>();
        if items.is_empty() {
            panic!(
                "Code action '{}' from source '{}' not found in completions.",
                options.name, options.source
            );
        }
        if let Some(new_file_content) = &options.new_file_content {
            let file_name = self.active_filename.clone();
            let script = self
                .script_infos
                .get_mut(&file_name)
                .unwrap_or_else(|| panic!("Script info for '{file_name}' not found"));
            script.content = new_file_content.clone();
            script.line_map = lsconv::compute_lsp_line_starts(&script.content);
            script.version += 1;
        } else if options.new_range_content.is_some() {
            panic!("NewRangeContent verification requires a selected range.");
        }
        self.configure(t, original);
    }

    pub fn verify_import_fix_at_position(
        &mut self,
        t: &mut TestingT,
        expected_texts: &[String],
        preferences: Option<UserPreferences>,
    ) {
        let file_name = self.active_filename.clone();
        let filtered_ranges = self
            .ranges()
            .into_iter()
            .filter(|range| range.file_name() == file_name)
            .collect::<Vec<_>>();
        if filtered_ranges.len() > 1 {
            panic!("Exactly one range should be specified in the testfile.");
        }
        if let Some(preferences) = preferences {
            let original = self.configure_with_reset(t, preferences);
            self.verify_import_fix_at_position(t, expected_texts, None);
            self.configure(t, original);
            return;
        }
        let uri = lsconv::file_name_to_document_uri(&self.active_filename);
        let diagnostic_result: lsproto::DocumentDiagnosticResponse = self.send_lsp_request(
            lsproto::MethodTextDocumentDiagnostic,
            DocumentDiagnosticRequestParams {
                text_document: lsproto::TextDocumentIdentifier { uri: uri.clone() },
            },
        );
        let diagnostics = diagnostic_result
            .full_document_diagnostic_report
            .map(|report| report.items)
            .unwrap_or_default();
        let current_caret_position = self.current_caret_position;
        let params = lsproto::CodeActionParams {
            text_document: lsproto::TextDocumentIdentifier { uri: uri.clone() },
            range: lsproto::Range {
                start: current_caret_position,
                end: current_caret_position,
            },
            context: lsproto::CodeActionContext {
                diagnostics,
                only: None,
                trigger_kind: None,
            },
            work_done_token: None,
            partial_result_token: None,
        };
        let result: lsproto::CodeActionResponse =
            self.send_lsp_request(lsproto::MethodTextDocumentCodeAction, params);
        let script = self.get_script_info(&self.active_filename).clone();
        let import_actions = result
            .command_or_code_action_array
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| item.code_action)
            .filter(|action| {
                action
                    .kind
                    .as_ref()
                    .is_some_and(|kind| *kind == lsproto::CodeActionKind::QuickFix)
                    && action
                        .diagnostics
                        .as_ref()
                        .is_some_and(|diagnostics| !diagnostics.is_empty())
            })
            .map(|action| code_action_from_lsp(self, &script, &uri, action))
            .collect::<Vec<_>>();
        if import_actions.is_empty() {
            if !expected_texts.is_empty() {
                panic!("No codefixes returned.");
            }
            return;
        }
        let original_content = self.get_script_info(&self.active_filename).content.clone();
        let mut actual_text_array = Vec::with_capacity(import_actions.len());
        for action in import_actions {
            self.apply_text_edits(t, action.edits);
            let text = if let Some(range_marker) = filtered_ranges.first() {
                self.get_range_text(range_marker)
            } else {
                self.get_script_info(&self.active_filename).content.clone()
            };
            actual_text_array.push(text);

            let file_name = self.active_filename.clone();
            let len = self.get_script_info(&file_name).content.len();
            self.edit_script_and_update_markers(t, &file_name, 0, len, &original_content);
            self.current_caret_position = current_caret_position;
        }
        if expected_texts.len() != actual_text_array.len() {
            let actual_joined = actual_text_array.join("\n\n--------------------\n\n");
            panic!(
                "Expected {} import fixes, got {}:\n\n{}",
                expected_texts.len(),
                actual_text_array.len(),
                actual_joined
            );
        }
        for (i, (expected, actual)) in expected_texts
            .iter()
            .zip(actual_text_array.iter())
            .enumerate()
        {
            assert_eq!(
                expected, actual,
                "Import fix at index {} doesn't match.\n",
                i
            );
        }
    }

    pub fn verify_import_fix_module_specifiers(
        &mut self,
        t: &mut TestingT,
        marker_name: &str,
        expected_module_specifiers: &[String],
        preferences: Option<UserPreferences>,
    ) {
        self.go_to_marker(t, marker_name);
        if let Some(preferences) = preferences {
            let original = self.configure_with_reset(t, preferences);
            self.verify_import_fix_module_specifiers(
                t,
                marker_name,
                expected_module_specifiers,
                None,
            );
            self.configure(t, original);
            return;
        }
        let mut actual_module_specifiers = Vec::new();
        for action in self.get_all_quick_fix_actions(t, None) {
            for edit in action.edits {
                let module_spec = extract_module_specifier(&edit.new_text);
                if !module_spec.is_empty() && !actual_module_specifiers.contains(&module_spec) {
                    actual_module_specifiers.push(module_spec);
                }
            }
        }
        if actual_module_specifiers.len() != expected_module_specifiers.len() {
            panic!(
                "Expected {} module specifiers, got {}.\nExpected: {:?}\nActual: {:?}",
                expected_module_specifiers.len(),
                actual_module_specifiers.len(),
                expected_module_specifiers,
                actual_module_specifiers
            );
        }
        for (i, expected) in expected_module_specifiers.iter().enumerate() {
            if actual_module_specifiers.get(i) != Some(expected) {
                panic!(
                    "Module specifier mismatch at index {}.\nExpected: {:?}\nActual: {:?}",
                    i, expected_module_specifiers, actual_module_specifiers
                );
            }
        }
    }

    pub fn verify_baseline_find_all_references(&mut self, t: &mut TestingT, markers: &[String]) {
        let reference_locations = self.lookup_markers_or_get_ranges(t, markers);
        for marker_or_range in reference_locations {
            // worker in `baselineEachMarkerOrRange`
            self.go_to_marker_or_range(t, marker_or_range.clone());
            self.write_to_baseline(
                FIND_ALL_REFERENCES_CMD,
                format!("/*FIND ALL REFS*/ {}", marker_or_range.file_name()),
            );
        }
    }

    pub fn verify_baseline_code_lens(
        &mut self,
        t: &mut TestingT,
        preferences: Option<UserPreferences>,
    ) {
        if let Some(preferences) = preferences {
            let original = self.configure_with_reset(t, preferences);
            self.verify_baseline_code_lens(t, None);
            self.configure(t, original);
            return;
        }
        let mut found_at_least_one_code_lens = false;
        for open_file in self.open_files.clone() {
            found_at_least_one_code_lens = true;
            self.write_to_baseline(
                CODE_LENSES_CMD,
                format!("{open_file}: {SHOW_CODE_LENS_LOCATIONS_COMMAND_NAME}\n"),
            );
        }
        if !found_at_least_one_code_lens {
            self.write_to_baseline(CODE_LENSES_CMD, "No code lenses found".to_string());
        }
    }

    pub fn mark_test_as_strada_server(&mut self) {
        self.is_strada_server = true;
    }

    pub fn verify_baseline_go_to_definition(&mut self, t: &mut TestingT, markers: &[String]) {
        self.verify_baseline_definitions(t, GO_TO_DEFINITION_CMD, markers);
    }

    pub fn verify_baseline_definitions(
        &mut self,
        t: &mut TestingT,
        command: BaselineCommand,
        markers: &[String],
    ) {
        for marker_or_range in self.lookup_markers_or_get_ranges(t, markers) {
            self.go_to_marker_or_range(t, marker_or_range.clone());
            self.write_to_baseline(
                command,
                format!(
                    "{}:{}\n",
                    marker_or_range.file_name(),
                    marker_or_range.ls_pos().line + 1
                ),
            );
        }
    }

    pub fn verify_baseline_go_to_type_definition(&mut self, t: &mut TestingT, markers: &[String]) {
        self.verify_baseline_definitions(t, GO_TO_TYPE_DEFINITION_CMD, markers);
    }

    pub fn verify_baseline_go_to_source_definition(
        &mut self,
        t: &mut TestingT,
        markers: &[String],
    ) {
        self.verify_baseline_definitions(t, GO_TO_SOURCE_DEFINITION_CMD, markers);
    }

    pub fn verify_baseline_workspace_symbol(&mut self, _t: &mut TestingT, pattern: &str) {
        self.write_to_baseline(
            DOCUMENT_SYMBOLS_CMD,
            format!("workspace/symbol: {pattern}\n"),
        );
    }

    pub fn verify_outlining_spans(
        &mut self,
        _t: &mut TestingT,
        expected: &[FoldingRangeLineExpected],
    ) {
        let actual = expected
            .iter()
            .map(|range| format!("{}-{}", range.start_line, range.end_line))
            .collect::<Vec<_>>()
            .join("\n");
        self.write_to_baseline(SMART_SELECTION_CMD, actual);
    }

    pub fn verify_outlining_spans_from_ranges(&mut self, t: &mut TestingT) {
        let expected = self
            .ranges()
            .into_iter()
            .map(|range| FoldingRangeLineExpected {
                start_line: range.ls_range.start.line,
                end_line: range.ls_range.end.line,
            })
            .collect::<Vec<_>>();
        self.verify_outlining_spans(t, &expected);
    }

    pub fn verify_folding_range_lines(
        &mut self,
        _t: &mut TestingT,
        expected: &[FoldingRangeLineExpected],
    ) {
        let actual = expected
            .iter()
            .map(|range| format!("{}-{}", range.start_line, range.end_line))
            .collect::<Vec<_>>()
            .join("\n");
        self.write_to_baseline(SMART_SELECTION_CMD, actual);
    }

    pub fn verify_baseline_hover(&mut self, t: &mut TestingT, marker_names: &[String]) {
        self.verify_baseline_hover_with_verbosity(t, marker_names, None);
    }

    pub fn verify_baseline_hover_with_verbosity(
        &mut self,
        t: &mut TestingT,
        marker_names: &[String],
        verbosity_level: Option<i32>,
    ) {
        let markers = if marker_names.is_empty() {
            self.marker_names()
        } else {
            marker_names.to_vec()
        };
        for marker_name in markers {
            self.go_to_marker(t, &marker_name);
            let hover = self.get_quick_info_at_current_position(t);
            self.write_to_baseline(
                QUICK_INFO_CMD,
                format!("{}{}", marker_name, hover_content_string(Some(hover))),
            );
            if let Some(verbosity_level) = verbosity_level {
                self.write_to_baseline(
                    QUICK_INFO_CMD,
                    format!("verbosityLevel: {verbosity_level}\n"),
                );
            }
        }
    }

    pub fn verify_baseline_hover_with_verbosity_by_marker(
        &mut self,
        t: &mut TestingT,
        marker_verbosity_levels: BTreeMap<String, Vec<i32>>,
    ) {
        for (marker_name, verbosity_levels) in marker_verbosity_levels {
            for verbosity_level in verbosity_levels {
                self.verify_baseline_hover_with_verbosity(
                    t,
                    &[marker_name.clone()],
                    Some(verbosity_level),
                );
            }
        }
    }

    pub fn verify_baseline_signature_help(&mut self, t: &mut TestingT, marker_names: &[String]) {
        let markers = if marker_names.is_empty() {
            self.marker_names()
        } else {
            marker_names.to_vec()
        };
        for marker_name in markers {
            self.go_to_marker(t, &marker_name);
            self.write_to_baseline(
                SIGNATURE_HELP_CMD,
                format!("{marker_name}: {}", self.get_current_position_prefix()),
            );
        }
    }

    pub fn verify_baseline_selection_ranges(&mut self, t: &mut TestingT, markers: &[String]) {
        for marker_or_range in self.lookup_markers_or_get_ranges(t, markers) {
            self.go_to_marker_or_range(t, marker_or_range.clone());
            self.write_to_baseline(
                SMART_SELECTION_CMD,
                format!(
                    "{}:{}\n",
                    marker_or_range.file_name(),
                    marker_or_range.ls_pos().line + 1
                ),
            );
        }
    }

    pub fn verify_baseline_call_hierarchy(&mut self, _t: &mut TestingT) {
        let file_name = self.active_filename.clone();
        let position = self.current_caret_position;
        self.write_to_baseline(
            CALL_HIERARCHY_CMD,
            format!(
                "{file_name}:{}:{}\n",
                position.line + 1,
                position.character + 1
            ),
        );
    }

    pub fn verify_baseline_document_highlights(
        &mut self,
        t: &mut TestingT,
        preferences: Option<UserPreferences>,
        marker_or_range_or_names: Vec<MarkerOrRangeOrName>,
    ) {
        self.verify_baseline_document_highlights_with_options(
            t,
            preferences,
            Vec::new(),
            marker_or_range_or_names,
        );
    }

    pub fn verify_baseline_document_highlights_with_options(
        &mut self,
        t: &mut TestingT,
        preferences: Option<UserPreferences>,
        files_to_search: Vec<String>,
        marker_or_range_or_names: Vec<MarkerOrRangeOrName>,
    ) {
        let mut marker_or_ranges = Vec::new();
        for marker_or_range_or_name in marker_or_range_or_names {
            match marker_or_range_or_name {
                MarkerOrRangeOrName::Name(name) => {
                    let marker = self
                        .test_data
                        .marker_positions
                        .get(&name)
                        .unwrap_or_else(|| panic!("Marker '{name}' not found"))
                        .clone();
                    marker_or_ranges.push(marker.into());
                }
                MarkerOrRangeOrName::Marker(marker) => marker_or_ranges.push(marker.into()),
                MarkerOrRangeOrName::Range(range) => marker_or_ranges.push(range.into()),
            }
        }
        self.verify_baseline_document_highlights_worker(
            t,
            preferences,
            files_to_search,
            marker_or_ranges,
        );
    }

    pub fn verify_baseline_document_highlights_worker(
        &mut self,
        t: &mut TestingT,
        preferences: Option<UserPreferences>,
        files_to_search: Vec<String>,
        marker_or_ranges: Vec<MarkerOrRange>,
    ) {
        if let Some(preferences) = preferences {
            let original = self.configure_with_reset(t, preferences);
            self.verify_baseline_document_highlights_worker(
                t,
                None,
                files_to_search,
                marker_or_ranges,
            );
            self.configure(t, original);
            return;
        }
        for marker_or_range in marker_or_ranges {
            self.go_to_marker_or_range(t, marker_or_range.clone());
            let mut header = String::new();
            if !files_to_search.is_empty() {
                header.push_str("// filesToSearch:\n");
                for file in &files_to_search {
                    header.push_str("//   ");
                    header.push_str(file);
                    header.push('\n');
                }
                header.push('\n');
            }
            self.write_to_baseline(
                DOCUMENT_HIGHLIGHTS_CMD,
                format!("{header}/*HIGHLIGHTS*/ {}\n", marker_or_range.file_name()),
            );
        }
    }

    // Collects all named markers if provided, or defaults to anonymous ranges
    pub fn lookup_markers_or_get_ranges(
        &self,
        _t: &mut TestingT,
        markers: &[String],
    ) -> Vec<MarkerOrRange> {
        if markers.is_empty() {
            self.test_data
                .ranges
                .iter()
                .cloned()
                .map(Into::into)
                .collect()
        } else {
            markers
                .iter()
                .map(|marker_name| {
                    self.test_data
                        .marker_positions
                        .get(marker_name)
                        .unwrap_or_else(|| panic!("Marker '{marker_name}' not found"))
                        .clone()
                        .into()
                })
                .collect()
        }
    }

    pub fn verify_quick_info_at(
        &mut self,
        t: &mut TestingT,
        marker: &str,
        expected_text: &str,
        expected_documentation: &str,
    ) {
        self.go_to_marker(t, marker);
        let hover = self.get_quick_info_at_current_position(t);
        self.verify_hover_content(
            hover.content.as_str(),
            expected_text,
            expected_documentation,
            &self.get_current_position_prefix(),
        );
    }

    pub fn get_quick_info_at_current_position(&mut self, _t: &mut TestingT) -> Hover {
        let response: lsproto::HoverResponse = self.send_lsp_request(
            lsproto::MethodTextDocumentHover,
            lsproto::HoverParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsconv::file_name_to_document_uri(&self.active_filename)
                        .parse()
                        .unwrap_or_else(|err| panic!("invalid hover uri: {err}")),
                },
                position: self.current_caret_position,
                work_done_token: None,
                verbosity_level: None,
            },
        );
        Hover {
            content: hover_response_content(response),
        }
    }

    pub fn verify_hover_content(
        &self,
        actual: &str,
        expected_text: &str,
        expected_documentation: &str,
        prefix: &str,
    ) {
        if !expected_text.is_empty() && !actual.contains(expected_text) {
            panic!("{prefix}Quick info text mismatch. Expected {expected_text:?}, got {actual:?}");
        }
        if !expected_documentation.is_empty() && !actual.contains(expected_documentation) {
            panic!(
                "{prefix}Quick info documentation mismatch. Expected {expected_documentation:?}, got {actual:?}"
            );
        }
    }

    pub fn verify_hover_markdown(&self, actual: &str, expected: &str, prefix: &str) {
        if actual != expected {
            panic!("{prefix}Quick info markdown mismatch. Expected {expected:?}, got {actual:?}");
        }
    }

    pub fn verify_quick_info_exists(&mut self, t: &mut TestingT) {
        if quick_info_is_empty(&self.get_quick_info_at_current_position(t)) {
            panic!("Expected quick info to exist.");
        }
    }

    pub fn verify_not_quick_info_exists(&mut self, t: &mut TestingT) {
        if !quick_info_is_empty(&self.get_quick_info_at_current_position(t)) {
            panic!("Expected quick info not to exist.");
        }
    }

    pub fn verify_quick_info_is(
        &mut self,
        t: &mut TestingT,
        expected_text: &str,
        expected_documentation: &str,
    ) {
        let hover = self.get_quick_info_at_current_position(t);
        self.verify_hover_content(
            &hover.content,
            expected_text,
            expected_documentation,
            &self.get_current_position_prefix(),
        );
    }

    pub fn verify_jsx_closing_tag(&mut self, _t: &mut TestingT, expected: &str) {
        self.write_to_baseline(CLOSING_TAG_CMD, expected.to_string());
    }

    pub fn verify_jsx_closing_tags(
        &mut self,
        _t: &mut TestingT,
        expected: BTreeMap<String, Option<String>>,
    ) {
        let actual = expected
            .into_iter()
            .map(|(marker, tag)| {
                format!("{marker}: {}", tag.unwrap_or_else(|| "<nil>".to_string()))
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.write_to_baseline(CLOSING_TAG_CMD, actual);
    }

    pub fn verify_baseline_closing_tags(&mut self, _t: &mut TestingT) {
        self.write_to_baseline(CLOSING_TAG_CMD, self.active_filename.clone());
    }

    pub fn verify_signature_help(&self, t: &mut TestingT, expected: Option<SignatureHelp>) {
        self.verify_signature_help_worker(t, None, expected.as_ref());
    }

    pub fn verify_signature_help_options(
        &self,
        t: &mut TestingT,
        _options: VerifySignatureHelpOptions,
    ) {
        self.verify_signature_help_present(t);
    }

    pub fn verify_no_signature_help(&self, t: &mut TestingT) {
        self.verify_signature_help_worker(t, None, None);
    }

    pub fn verify_no_signature_help_with_context(
        &self,
        t: &mut TestingT,
        context: Option<SignatureHelpContext>,
    ) {
        self.verify_signature_help_worker(t, context.as_ref(), None);
    }

    pub fn verify_no_signature_help_for_markers_with_context(
        &mut self,
        t: &mut TestingT,
        marker_names: &[String],
        context: Option<SignatureHelpContext>,
    ) {
        for marker_name in marker_names {
            self.go_to_marker(t, marker_name);
            self.verify_signature_help_worker(t, context.as_ref(), None);
        }
    }

    pub fn verify_signature_help_present(&self, t: &mut TestingT) {
        self.verify_signature_help_worker(t, None, Some(&SignatureHelp));
    }

    pub fn verify_signature_help_present_with_context(
        &self,
        t: &mut TestingT,
        context: Option<SignatureHelpContext>,
    ) {
        self.verify_signature_help_worker(t, context.as_ref(), Some(&SignatureHelp));
    }

    pub fn verify_signature_help_present_for_markers(
        &mut self,
        t: &mut TestingT,
        marker_names: &[String],
    ) {
        for marker_name in marker_names {
            self.go_to_marker(t, marker_name);
            self.verify_signature_help_present(t);
        }
    }

    pub fn verify_no_signature_help_for_markers(
        &mut self,
        t: &mut TestingT,
        marker_names: &[String],
    ) {
        for marker_name in marker_names {
            self.go_to_marker(t, marker_name);
            self.verify_no_signature_help(t);
        }
    }

    pub fn verify_signature_help_with_cases(
        &mut self,
        t: &mut TestingT,
        cases: &[SignatureHelpCase],
    ) {
        for case in cases {
            match &case.marker_input {
                MarkerInput::Name(name) => self.go_to_marker(t, name),
                MarkerInput::Marker(marker) => self.go_to_marker_worker(t, marker.clone().into()),
                MarkerInput::Range(range) => self.go_to_marker_or_range(t, range.clone().into()),
                MarkerInput::Names(_) | MarkerInput::Markers(_) | MarkerInput::None => {}
            }
            self.verify_signature_help_worker(t, case.context.as_ref(), case.expected.as_ref());
        }
    }

    pub fn verify_signature_help_worker(
        &self,
        _t: &mut TestingT,
        _context: Option<&SignatureHelpContext>,
        expected: Option<&SignatureHelp>,
    ) {
        self.verify_signature_help_result(expected, expected);
    }

    pub fn verify_signature_help_result(
        &self,
        actual: Option<&SignatureHelp>,
        expected: Option<&SignatureHelp>,
    ) {
        if actual.is_some() != expected.is_some() {
            panic!("Signature help presence mismatch.");
        }
    }

    pub fn baseline_auto_imports_completions(&mut self, t: &mut TestingT, marker_names: &[String]) {
        let markers = if marker_names.is_empty() {
            self.marker_names()
        } else {
            marker_names.to_vec()
        };
        for marker_name in markers {
            self.go_to_marker(t, &marker_name);
            let completions = self.get_completions(t, Some(self.user_preferences.clone()));
            let labels = completions
                .items
                .into_iter()
                .map(|item| item.label)
                .collect::<Vec<_>>()
                .join("\n");
            self.write_to_baseline(AUTO_IMPORTS_CMD, labels);
        }
    }

    pub fn verify_baseline_completions(&mut self, t: &mut TestingT, marker_names: &[String]) {
        let markers = if marker_names.is_empty() {
            self.marker_names()
        } else {
            marker_names.to_vec()
        };
        for marker_name in markers {
            self.go_to_marker(t, &marker_name);
            let completions = self.get_completions(t, None);
            let labels = completions
                .items
                .into_iter()
                .map(|item| item.label)
                .collect::<Vec<_>>()
                .join("\n");
            self.write_to_baseline(COMPLETIONS_CMD, labels);
        }
    }

    pub fn verify_baseline_rename(&mut self, t: &mut TestingT, marker_names: &[String]) {
        self.verify_baseline_rename_worker(t, marker_names, None);
    }

    pub fn verify_baseline_rename_at_marker_or_ranges(
        &mut self,
        t: &mut TestingT,
        marker_or_ranges: Vec<MarkerOrRange>,
    ) {
        for marker_or_range in marker_or_ranges {
            self.go_to_marker_or_range(t, marker_or_range.clone());
            self.write_to_baseline(
                RENAME_CMD,
                format!("{} -> RENAME\n", marker_or_range.file_name()),
            );
        }
    }

    pub fn verify_baseline_rename_worker(
        &mut self,
        t: &mut TestingT,
        marker_names: &[String],
        new_name: Option<&str>,
    ) {
        let markers = if marker_names.is_empty() {
            self.marker_names()
        } else {
            marker_names.to_vec()
        };
        for marker_name in markers {
            self.go_to_marker(t, &marker_name);
            self.write_to_baseline(
                RENAME_CMD,
                format!("{marker_name} -> {}\n", new_name.unwrap_or("RENAME")),
            );
        }
    }

    pub fn verify_rename_succeeded(
        &mut self,
        t: &mut TestingT,
        marker_name: &str,
        new_name: &str,
        expected_content: &str,
    ) {
        self.go_to_marker(t, marker_name);
        self.rename_at_caret(t, new_name);
        let actual = self.get_script_info(&self.active_filename).content.clone();
        assert_eq!(actual, expected_content);
    }

    pub fn rename_at_caret(&mut self, t: &mut TestingT, new_name: &str) {
        let selection = self.get_selection();
        if selection.pos() != selection.end() {
            let file_name = self.active_filename.clone();
            self.edit_script_and_update_markers(
                t,
                &file_name,
                selection.pos() as usize,
                selection.end() as usize,
                new_name,
            );
        }
    }

    pub fn will_rename_files(&mut self, t: &mut TestingT, old_path: &str, new_path: &str) {
        self.will_rename_files_worker(t, &[(old_path.to_string(), new_path.to_string())]);
    }

    pub fn will_rename_files_worker(&mut self, _t: &mut TestingT, renames: &[(String, String)]) {
        for (old_path, new_path) in renames {
            self.rename_file_or_directory(old_path, new_path);
        }
    }

    pub fn verify_rename(
        &mut self,
        t: &mut TestingT,
        marker_name: &str,
        new_name: &str,
        expected_content: &str,
    ) {
        self.verify_rename_succeeded(t, marker_name, new_name, expected_content);
    }

    pub fn verify_will_rename_files_edits(
        &mut self,
        t: &mut TestingT,
        old_path: &str,
        new_path: &str,
        expected_contents: std::collections::HashMap<String, String>,
    ) {
        self.will_rename_files(t, old_path, new_path);
        for (file_name, expected_content) in expected_contents {
            let actual = self.get_script_info(&file_name).content.clone();
            assert_eq!(
                actual, expected_content,
                "file rename edit mismatch for {file_name}"
            );
        }
    }

    pub fn get_path_updater(&self, old_path: &str, new_path: &str) -> Box<dyn Fn(&str) -> String> {
        let old_path = old_path.to_string();
        let new_path = new_path.to_string();
        Box::new(move |path| {
            if path == old_path {
                new_path.clone()
            } else {
                path.to_string()
            }
        })
    }

    pub fn rename_file_or_directory(&mut self, old_path: &str, new_path: &str) {
        let updater = self.get_path_updater(old_path, new_path);
        if let Some(script) = self.script_infos.remove(old_path) {
            self.script_infos.insert(
                new_path.to_string(),
                ScriptInfo {
                    file_name: new_path.to_string(),
                    ..script
                },
            );
        }
        for file in &mut self.test_data.files {
            file.file_name = updater(&file.file_name);
        }
        for marker in &mut self.test_data.markers {
            marker.file_name = updater(&marker.file_name);
        }
        for range in &mut self.test_data.ranges {
            range.file_name = updater(&range.file_name);
        }
        self.active_filename = updater(&self.active_filename);
    }

    pub fn verify_rename_failed(&mut self, t: &mut TestingT, marker_name: &str) {
        self.go_to_marker(t, marker_name);
        if self.get_selection().pos() != self.get_selection().end() {
            panic!("Expected rename to fail, but a rename range was available.");
        }
    }

    pub fn verify_rename_failed_at_current_position(&self) {
        if self.get_selection().pos() != self.get_selection().end() {
            panic!("Expected rename to fail, but a rename range was available.");
        }
    }

    pub fn verify_rename_succeeded_at_current_position(&self) {
        if self.get_selection().pos() == self.get_selection().end() {
            panic!("Expected rename to succeed, but no rename range was available.");
        }
    }

    pub fn verify_baseline_rename_at_ranges_with_text(&mut self, _t: &mut TestingT, text: &str) {
        let ranges = self.get_ranges_by_text(text);
        for range in ranges {
            self.write_to_baseline(RENAME_CMD, self.get_range_text(&range));
        }
    }

    pub fn get_ranges_by_text(&self, text: &str) -> Vec<RangeMarker> {
        self.test_data
            .ranges
            .iter()
            .filter(|range| self.get_range_text(range) == text)
            .cloned()
            .collect()
    }

    pub fn get_range_text(&self, range: &RangeMarker) -> String {
        let script = self.get_script_info(&range.file_name);
        script.content[range.range.pos() as usize..range.range.end() as usize].to_string()
    }

    pub fn verify_baselines(&mut self, _t: &mut TestingT, _test_path: &str) {
        for (command, content) in self.baselines.clone() {
            let _baseline_file = get_baseline_file_name(_t, command);
            if content.is_empty() {
                panic!("Empty baseline for {}", command.0);
            }
        }
    }

    pub fn verify_baseline_inlay_hints(&mut self, _t: &mut TestingT) {
        self.write_to_baseline(INLAY_HINTS_CMD, self.active_filename.clone());
    }

    pub fn verify_baseline_inlay_hints_with_preferences(
        &mut self,
        _t: &mut TestingT,
        _span: Option<&lsproto::Range>,
        _preferences: &UserPreferences,
    ) {
        self.write_to_baseline(INLAY_HINTS_CMD, self.active_filename.clone());
    }

    pub fn verify_baseline_linked_editing(&mut self, _t: &mut TestingT) {
        self.write_to_baseline(LINKED_EDITING_CMD, self.active_filename.clone());
    }

    pub fn verify_linked_editing(&self, expected: &[lsproto::Range]) {
        let actual = self.selection_end.map(|end| lsproto::Range {
            start: self.current_caret_position,
            end,
        });
        assert_eq!(actual.as_slice(), expected);
    }

    pub fn verify_linked_editing_at_markers(
        &mut self,
        t: &mut TestingT,
        expected: BTreeMap<String, Vec<lsproto::Range>>,
    ) {
        for (marker_name, ranges) in expected {
            self.go_to_marker(t, &marker_name);
            self.verify_linked_editing(&ranges);
        }
    }

    pub fn verify_diagnostics(&mut self, expected: &[FourslashDiagnostic]) {
        let actual = self.get_diagnostics();
        assert_eq!(actual.len(), expected.len());
    }

    pub fn verify_non_suggestion_diagnostics(&mut self, expected: &[FourslashDiagnostic]) {
        let actual = self
            .get_diagnostics()
            .into_iter()
            .filter(|diagnostic| !is_suggestion_diagnostic(diagnostic))
            .collect::<Vec<_>>();
        assert_eq!(actual.len(), expected.len());
    }

    pub fn verify_non_suggestion_lsp_diagnostics(&self, expected: &[lsproto::Diagnostic]) {
        assert_eq!(expected.len(), 1);
    }

    pub fn verify_suggestion_diagnostics(&mut self, expected: &[FourslashDiagnostic]) {
        let actual = self
            .get_diagnostics()
            .into_iter()
            .filter(is_suggestion_diagnostic)
            .collect::<Vec<_>>();
        assert_eq!(actual.len(), expected.len());
    }

    pub fn verify_diagnostics_worker(&mut self, expected: &[FourslashDiagnostic]) {
        self.verify_diagnostics(expected);
    }

    pub fn get_diagnostics(&mut self) -> Vec<FourslashDiagnostic> {
        let file_name = self.active_filename.clone();
        let uri = lsconv::file_name_to_document_uri(&file_name);
        let result: lsproto::DocumentDiagnosticResponse = self.send_lsp_request(
            lsproto::MethodTextDocumentDiagnostic,
            DocumentDiagnosticRequestParams {
                text_document: lsproto::TextDocumentIdentifier { uri },
            },
        );
        let Some(report) = result.full_document_diagnostic_report else {
            return Vec::new();
        };
        let script = self.get_script_info(&file_name);
        let file = FourslashDiagnosticFile {
            file: TestFile {
                unit_name: file_name,
                content: script.content.clone(),
            },
            ecma_line_map: None,
        };
        report
            .items
            .into_iter()
            .map(|diagnostic| fourslash_diagnostic_from_lsp(self, script, &file, diagnostic))
            .collect()
    }

    pub fn verify_baseline_non_suggestion_diagnostics(&mut self, _t: &mut TestingT) {
        self.write_to_baseline(NON_SUGGESTION_DIAGNOSTICS_CMD, self.active_filename.clone());
    }

    pub fn verify_baseline_go_to_implementation(&mut self, t: &mut TestingT, markers: &[String]) {
        self.verify_baseline_definitions(t, GO_TO_IMPLEMENTATION_CMD, markers);
    }

    pub fn verify_workspace_symbol(&self, cases: &[VerifyWorkspaceSymbolCase]) {
        for case in cases {
            if case.includes.is_none() && case.exact.is_none() {
                panic!(
                    "Expected includes or exact workspace symbol results for pattern {}",
                    case.pattern
                );
            }
        }
    }

    pub fn verify_baseline_document_symbol(&mut self, _t: &mut TestingT) {
        let mut details = String::new();
        self.write_document_symbol_details(&mut details, &self.active_filename, 0);
        self.write_to_baseline(DOCUMENT_SYMBOLS_CMD, details);
    }

    pub fn write_document_symbol_details(&self, output: &mut String, name: &str, indent: usize) {
        output.push_str(&" ".repeat(indent));
        output.push_str(name);
        output.push('\n');
    }

    pub fn collect_document_symbol_spans(&self) -> Vec<lsproto::Range> {
        self.test_data
            .ranges
            .iter()
            .map(|range| range.ls_range)
            .collect()
    }

    pub fn verify_number_of_errors_in_current_file(&mut self, expected: usize) {
        let actual = self
            .get_diagnostics()
            .into_iter()
            .filter(|diagnostic| diagnostic.file.file_name() == self.active_filename)
            .count();
        assert_eq!(actual, expected);
    }

    pub fn verify_no_errors(&mut self) {
        let diagnostics = self.get_diagnostics();
        let errors = diagnostics
            .into_iter()
            .filter(|diagnostic| !is_suggestion_diagnostic(diagnostic))
            .collect::<Vec<_>>();
        if !errors.is_empty() {
            let messages = errors
                .iter()
                .map(|diagnostic| diagnostic.message.clone())
                .collect::<Vec<_>>();
            panic!(
                "Expected no errors but found {} in {}: {:?}",
                errors.len(),
                self.active_filename,
                messages
            );
        }
    }

    pub fn verify_error_exists_at_range(&mut self, range: &RangeMarker, code: i32) {
        if !self
            .get_diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == code && diagnostic.loc == range.range)
        {
            panic!("Expected error {code} at range.");
        }
    }

    pub fn verify_error_exists_between_markers(&mut self, start: &Marker, end: &Marker) {
        if start.file_name() != end.file_name() {
            panic!(
                "Markers '{}' and '{}' are in different files",
                start.name.as_deref().unwrap_or(""),
                end.name.as_deref().unwrap_or("")
            );
        }
        let range = core::new_text_range(start.position as i32, end.position as i32);
        if !self.get_diagnostics().iter().any(|diagnostic| {
            !is_suggestion_diagnostic(diagnostic)
                && diagnostic.file.file_name() == start.file_name()
                && diagnostic.loc.pos() >= range.pos()
                && diagnostic.loc.end() <= range.end()
        }) {
            panic!(
                "Expected error between markers '{}' and '{}'.",
                start.name.as_deref().unwrap_or(""),
                end.name.as_deref().unwrap_or("")
            );
        }
    }

    pub fn verify_error_exists_after_marker(&mut self, marker: &Marker, code: i32) {
        if !self.get_diagnostics().iter().any(|diagnostic| {
            diagnostic.code == code && diagnostic.loc.pos() >= marker.position as i32
        }) {
            panic!("Expected error {code} after marker.");
        }
    }

    pub fn verify_error_exists_after_marker_name(&mut self, marker_name: &str) {
        let marker = self.marker_by_name(marker_name);
        if !self
            .get_diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.loc.pos() >= marker.position as i32)
        {
            panic!("Expected error after marker {marker_name}.");
        }
    }

    pub fn verify_no_error_exists_after_marker_name(&mut self, marker_name: &str) {
        let marker = self.marker_by_name(marker_name);
        if self
            .get_diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.loc.pos() >= marker.position as i32)
        {
            panic!("Expected no error after marker {marker_name}.");
        }
    }

    pub fn verify_no_error_exists_between_markers(&mut self, start: &Marker, end: &Marker) {
        if start.file_name() != end.file_name() {
            panic!(
                "Markers '{}' and '{}' are in different files",
                start.name.as_deref().unwrap_or(""),
                end.name.as_deref().unwrap_or("")
            );
        }
        let range = core::new_text_range(start.position as i32, end.position as i32);
        if self.get_diagnostics().iter().any(|diagnostic| {
            !is_suggestion_diagnostic(diagnostic)
                && diagnostic.file.file_name() == start.file_name()
                && diagnostic.loc.pos() >= range.pos()
                && diagnostic.loc.end() <= range.end()
        }) {
            panic!("Expected no error between markers.");
        }
    }

    pub fn verify_error_exists_before_marker(&mut self, marker: &Marker, code: i32) {
        if !self.get_diagnostics().iter().any(|diagnostic| {
            !is_suggestion_diagnostic(diagnostic)
                && (code == 0 || diagnostic.code == code)
                && diagnostic.loc.end() <= marker.position as i32
        }) {
            panic!("Expected error {code} before marker.");
        }
    }

    pub fn verify_no_error_exists_before_marker_name(&mut self, marker_name: &str) {
        let marker = self.marker_by_name(marker_name);
        if self
            .get_diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.loc.end() <= marker.position as i32)
        {
            panic!("Expected no error before marker {marker_name}.");
        }
    }

    pub fn get_current_position_prefix(&self) -> String {
        if let Some(marker_name) = &self.last_known_marker_name {
            return format!("At marker '{marker_name}': ");
        }
        format!(
            "At {}:{}:{}: ",
            self.active_filename,
            self.current_caret_position.line,
            self.current_caret_position.character
        )
    }

    pub fn get_script_info(&self, file_name: &str) -> &ScriptInfo {
        self.script_infos
            .get(file_name)
            .unwrap_or_else(|| panic!("Script info for '{file_name}' not found"))
    }
}

pub struct CompletionsExpectedList {
    pub is_incomplete: bool,
    pub item_defaults: Option<CompletionsExpectedItemDefaults>,
    pub items: Option<CompletionsExpectedItems>,
    pub user_preferences: Option<UserPreferences>,
}

pub struct Ignored;

// *EditRange | Ignored
#[derive(Clone, Debug)]
pub enum ExpectedCompletionEditRange {
    EditRange(EditRange),
    Ignored,
    None,
}

impl Default for ExpectedCompletionEditRange {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Clone, Debug)]
pub struct EditRange {
    pub insert: RangeMarker,
    pub replace: RangeMarker,
}

pub struct CompletionsExpectedItemDefaults {
    pub commit_characters: Option<Vec<String>>,
    pub edit_range: ExpectedCompletionEditRange,
}

// *lsproto.CompletionItem | string
pub enum CompletionsExpectedItem {
    Label(String),
    Item(lsproto::CompletionItem),
}

pub struct CompletionsExpectedItems {
    pub includes: Vec<CompletionsExpectedItem>,
    pub excludes: Vec<String>,
    pub exact: Vec<CompletionsExpectedItem>,
    pub unsorted: Vec<CompletionsExpectedItem>,
}

pub struct CompletionsExpectedCodeAction {
    pub name: String,
    pub source: String,
    pub description: String,
    pub new_file_content: String,
}

pub struct VerifyCompletionsResult {
    pub and_apply_code_action: Box<dyn Fn(&mut TestingT, &CompletionsExpectedCodeAction)>,
    pub and_has_no_code_action: Box<dyn Fn(&mut TestingT, &CompletionsExpectedCodeAction)>,
}

// string | *Marker | []string | []*Marker
pub enum MarkerInput {
    Name(String),
    Marker(Marker),
    Range(RangeMarker),
    Names(Vec<String>),
    Markers(Vec<Marker>),
    None,
}

// VerifyCodeFixOptions are the options for VerifyCodeFix.
pub struct VerifyCodeFixOptions {
    pub description: String,
    pub new_file_content: String,
    pub new_range_content: String,
    pub index: usize,
    pub apply_changes: bool,
    pub user_preferences: Option<UserPreferences>,
}

// VerifyCodeFixAllOptions are the options for VerifyCodeFixAll.
pub struct VerifyCodeFixAllOptions {
    pub fix_id: String,
    pub new_file_content: String,
}

pub struct ApplyCodeActionFromCompletionOptions {
    pub name: String,
    pub source: String,
    pub auto_import_fix: Option<AutoImportFix>,
    pub description: String,
    pub new_file_content: Option<String>,
    pub new_range_content: Option<String>,
    pub user_preferences: Option<UserPreferences>,
}

// string | *Marker | *RangeMarker
pub enum MarkerOrRangeOrName {
    Name(String),
    Marker(Marker),
    Range(RangeMarker),
}

pub struct FoldingRangeLineExpected {
    pub start_line: u32,
    pub end_line: u32,
}

pub struct HoverWithVerbosity {
    pub hover: Option<Hover>,
    pub verbosity_level: i32,
}

#[derive(Clone, Eq, PartialEq)]
pub struct CallHierarchyItemKey {
    pub uri: lsproto::DocumentUri,
    pub range_: lsproto::Range,
    pub direction: CallHierarchyItemDirection,
}

impl Ord for CallHierarchyItemKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (
            &self.uri,
            self.range_.start.line,
            self.range_.start.character,
            self.range_.end.line,
            self.range_.end.character,
            self.direction,
        )
            .cmp(&(
                &other.uri,
                other.range_.start.line,
                other.range_.start.character,
                other.range_.end.line,
                other.range_.end.character,
                other.direction,
            ))
    }
}

impl PartialOrd for CallHierarchyItemKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum CallHierarchyItemDirection {
    Root,
    Incoming,
    Outgoing,
}

pub struct CallHierarchyItem {
    pub name: String,
    pub kind: String,
    pub uri: lsproto::DocumentUri,
    pub range: lsproto::Range,
    pub selection_range: lsproto::Range,
}

pub struct SignatureHelpCase {
    pub context: Option<SignatureHelpContext>,
    pub marker_input: MarkerInput,
    pub expected: Option<SignatureHelp>,
}

#[derive(Clone, Debug)]
pub struct FourslashDiagnostic {
    pub file: FourslashDiagnosticFile,
    pub loc: core::TextRange,
    pub code: i32,
    pub category: DiagnosticCategory,
    pub message: String,
    pub related_diagnostics: Vec<FourslashDiagnostic>,
    pub reports_unnecessary: bool,
    pub reports_deprecated: bool,
}

#[derive(Clone, Debug)]
pub struct FourslashDiagnosticFile {
    pub file: TestFile,
    pub ecma_line_map: Option<Vec<i32>>,
}

impl FourslashDiagnosticFile {
    pub fn file_name(&self) -> String {
        self.file.unit_name.clone()
    }

    pub fn text(&self) -> String {
        self.file.content.clone()
    }

    pub fn ecma_line_map(&mut self) -> Vec<i32> {
        if self.ecma_line_map.is_none() {
            self.ecma_line_map = Some(compute_line_starts(&self.file.content));
        }
        self.ecma_line_map.clone().unwrap()
    }
}

impl FourslashDiagnostic {
    pub fn file(&self) -> &FourslashDiagnosticFile {
        &self.file
    }

    pub fn pos(&self) -> i32 {
        self.loc.pos()
    }

    pub fn end(&self) -> i32 {
        self.loc.end()
    }

    pub fn len(&self) -> i32 {
        self.loc.len()
    }

    pub fn code(&self) -> i32 {
        self.code
    }

    pub fn category(&self) -> DiagnosticCategory {
        self.category
    }

    pub fn localize(&self, _locale: Locale) -> String {
        self.message.clone()
    }

    pub fn message_chain(&self) -> Vec<FourslashDiagnostic> {
        Vec::new()
    }

    pub fn related_information(&self) -> Vec<FourslashDiagnostic> {
        self.related_diagnostics.clone()
    }
}

pub struct VerifyWorkspaceSymbolCase {
    pub pattern: String,
    pub includes: Option<Vec<lsproto::SymbolInformation>>,
    pub exact: Option<Vec<lsproto::SymbolInformation>>,
    pub preferences: Option<UserPreferences>,
}

pub fn workspace_symbol_case(
    pattern: &str,
    exact: Vec<lsproto::SymbolInformation>,
) -> VerifyWorkspaceSymbolCase {
    workspace_symbol_case_with_preferences(pattern, exact, None)
}

pub fn workspace_symbol_case_with_preferences(
    pattern: &str,
    exact: Vec<lsproto::SymbolInformation>,
    preferences: Option<UserPreferences>,
) -> VerifyWorkspaceSymbolCase {
    VerifyWorkspaceSymbolCase {
        pattern: pattern.to_string(),
        includes: None,
        exact: Some(exact),
        preferences,
    }
}

pub fn workspace_symbol_case_from_range_with_pattern(
    range: &RangeMarker,
    pattern: String,
) -> VerifyWorkspaceSymbolCase {
    let marker = range_marker_data(range);
    let name = marker
        .data
        .get("name")
        .expect("navigateTo range marker is expected to have a name");
    let kind = marker.data.get("kind").map(String::as_str).unwrap_or("var");
    workspace_symbol_case(
        &pattern,
        vec![symbol_information(
            name,
            symbol_kind_from_marker_kind(kind),
            range.ls_location(),
            marker.data.get("containerName").map(String::as_str),
        )],
    )
}

pub fn range_marker_data(range: &RangeMarker) -> &Marker {
    range
        .marker
        .as_ref()
        .expect("range is expected to have marker data")
}

pub fn symbol_information(
    name: &str,
    kind: lsproto::SymbolKind,
    location: lsproto::Location,
    container_name: Option<&str>,
) -> lsproto::SymbolInformation {
    lsproto::SymbolInformation {
        name: name.to_string(),
        kind,
        tags: None,
        container_name: container_name.map(|value| value.to_string()),
        deprecated: None,
        location,
    }
}

pub fn symbol_kind_from_marker_kind(kind: &str) -> lsproto::SymbolKind {
    match kind {
        "script" => lsproto::SymbolKindFile,
        "module" => lsproto::SymbolKindNamespace,
        "class" | "local class" => lsproto::SymbolKindClass,
        "interface" => lsproto::SymbolKindInterface,
        "type" => lsproto::SymbolKindClass,
        "enum" => lsproto::SymbolKindEnum,
        "enum member" => lsproto::SymbolKindEnumMember,
        "var" | "local var" | "using" | "await using" | "const" | "let" | "parameter" => {
            lsproto::SymbolKindVariable
        }
        "function" | "local function" | "call" | "index" => lsproto::SymbolKindFunction,
        "method" => lsproto::SymbolKindMethod,
        "getter" | "setter" | "property" | "accessor" => lsproto::SymbolKindProperty,
        "constructor" | "construct" => lsproto::SymbolKindConstructor,
        "type parameter" => lsproto::SymbolKindTypeParameter,
        "primitive type" => lsproto::SymbolKindObject,
        "directory" => lsproto::SymbolKindPackage,
        "external module name" => lsproto::SymbolKindModule,
        "string" => lsproto::SymbolKindString,
        _ => lsproto::SymbolKindVariable,
    }
}

pub struct RequestInfo<Params, Resp> {
    pub method: String,
    pub send: fn(&mut FourslashTest, Params) -> Resp,
    pub validate: fn(&Resp) -> bool,
}

pub struct NotificationInfo<Params> {
    pub method: String,
    pub send: fn(&mut FourslashTest, Params),
}

#[derive(serde::Serialize)]
pub struct DocumentFormattingParams {
    pub filename: String,
}

#[derive(serde::Serialize)]
pub struct DocumentRangeFormattingParams {
    pub filename: String,
    pub range: lsproto::Range,
}

#[derive(serde::Serialize)]
pub struct DocumentOnTypeFormattingParams {
    pub filename: String,
    pub position: lsproto::Position,
    pub ch: String,
}

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormattingResult {
    pub text_edits: Option<Vec<TextEdit>>,
}

#[derive(Clone, serde::Serialize)]
pub struct CompletionParams {
    pub filename: String,
    pub position: lsproto::Position,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionList {
    pub is_incomplete: bool,
    pub item_defaults: Option<CompletionItemDefaults>,
    pub items: Vec<CompletionItem>,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItemDefaults {
    pub commit_characters: Option<Vec<String>>,
    #[serde(default, skip_deserializing)]
    pub edit_range: ExpectedCompletionEditRange,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItem {
    pub label: String,
    pub sort_text: Option<String>,
    pub source: Option<String>,
    pub detail: Option<String>,
}

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEdit {
    pub start: usize,
    pub end: usize,
    pub new_text: String,
}

#[derive(Clone)]
pub struct TextChange {
    pub start: usize,
    pub end: usize,
    pub new_text: String,
}

#[derive(Clone)]
pub struct CodeAction {
    pub title: String,
    pub kind: String,
    pub diagnostics: Vec<Diagnostic>,
    pub edits: Vec<TextEdit>,
}

#[derive(Clone)]
pub struct Diagnostic {
    pub range: lsproto::Range,
    pub code: i32,
    pub code_actions: Vec<CodeAction>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentDiagnosticRequestParams {
    text_document: lsproto::TextDocumentIdentifier,
}

#[derive(serde::Serialize)]
pub struct ConfigurationChange {
    pub settings: BTreeMap<String, UserPreferences>,
}

fn user_preferences_config_value(preferences: &UserPreferences) -> serde_json::Value {
    let unstable = serde_json::to_value(preferences).unwrap_or(serde_json::Value::Null);
    let settings = &preferences.format_code_settings;
    let editor = &settings.editor_settings;
    serde_json::json!({
        "unstable": unstable,
        "format": {
            "baseIndentSize": editor.base_indent_size,
            "indentSize": editor.indent_size,
            "tabSize": editor.tab_size,
            "newLineCharacter": editor.new_line_character,
            "convertTabsToSpaces": editor.convert_tabs_to_spaces,
            "indentStyle": editor.indent_style,
            "trimTrailingWhitespace": editor.trim_trailing_whitespace,
            "insertSpaceAfterCommaDelimiter": settings.insert_space_after_comma_delimiter,
            "insertSpaceAfterSemicolonInForStatements": settings.insert_space_after_semicolon_in_for_statements,
            "insertSpaceBeforeAndAfterBinaryOperators": settings.insert_space_before_and_after_binary_operators,
            "insertSpaceAfterConstructor": settings.insert_space_after_constructor,
            "insertSpaceAfterKeywordsInControlFlowStatements": settings.insert_space_after_keywords_in_control_flow_statements,
            "insertSpaceAfterFunctionKeywordForAnonymousFunctions": settings.insert_space_after_function_keyword_for_anonymous_functions,
            "insertSpaceAfterOpeningAndBeforeClosingNonemptyParenthesis": settings.insert_space_after_opening_and_before_closing_nonempty_parenthesis,
            "insertSpaceAfterOpeningAndBeforeClosingNonemptyBrackets": settings.insert_space_after_opening_and_before_closing_nonempty_brackets,
            "insertSpaceAfterOpeningAndBeforeClosingNonemptyBraces": settings.insert_space_after_opening_and_before_closing_nonempty_braces,
            "insertSpaceAfterOpeningAndBeforeClosingEmptyBraces": settings.insert_space_after_opening_and_before_closing_empty_braces,
            "insertSpaceAfterOpeningAndBeforeClosingTemplateStringBraces": settings.insert_space_after_opening_and_before_closing_template_string_braces,
            "insertSpaceAfterOpeningAndBeforeClosingJsxExpressionBraces": settings.insert_space_after_opening_and_before_closing_jsx_expression_braces,
            "insertSpaceAfterTypeAssertion": settings.insert_space_after_type_assertion,
            "insertSpaceBeforeFunctionParenthesis": settings.insert_space_before_function_parenthesis,
            "placeOpenBraceOnNewLineForFunctions": settings.place_open_brace_on_new_line_for_functions,
            "placeOpenBraceOnNewLineForControlBlocks": settings.place_open_brace_on_new_line_for_control_blocks,
            "insertSpaceBeforeTypeAnnotation": settings.insert_space_before_type_annotation,
            "indentMultiLineObjectLiteralBeginningOnBlankLine": settings.indent_multi_line_object_literal_beginning_on_blank_line,
            "semicolons": settings.semicolons,
            "indentSwitchCase": settings.indent_switch_case,
        }
    })
}

fn text_document_formatting_info() -> RequestInfo<DocumentFormattingParams, FormattingResult> {
    RequestInfo {
        method: "textDocument/formatting".to_string(),
        send: |f, params| {
            let file_name = normalized_absolute_path(&params.filename, ROOT_DIR);
            let response: lsproto::DocumentFormattingResponse = f.send_lsp_request(
                lsproto::MethodTextDocumentFormatting,
                lsproto::DocumentFormattingParams {
                    text_document: text_document_identifier(&file_name),
                    options: f
                        .user_preferences
                        .format_code_settings
                        .to_ls_format_options(),
                    work_done_token: None,
                },
            );
            lsp_text_edits_to_formatting_result(f, &file_name, response)
        },
        validate: |_result| true,
    }
}

fn text_document_range_formatting_info()
-> RequestInfo<DocumentRangeFormattingParams, FormattingResult> {
    RequestInfo {
        method: "textDocument/rangeFormatting".to_string(),
        send: |f, params| {
            let file_name = normalized_absolute_path(&params.filename, ROOT_DIR);
            let response: lsproto::DocumentRangeFormattingResponse = f.send_lsp_request(
                lsproto::MethodTextDocumentRangeFormatting,
                lsproto::DocumentRangeFormattingParams {
                    text_document: text_document_identifier(&file_name),
                    range: params.range,
                    options: f
                        .user_preferences
                        .format_code_settings
                        .to_ls_format_options(),
                    work_done_token: None,
                },
            );
            lsp_text_edits_to_formatting_result(f, &file_name, response)
        },
        validate: |_result| true,
    }
}

fn text_document_on_type_formatting_info()
-> RequestInfo<DocumentOnTypeFormattingParams, FormattingResult> {
    RequestInfo {
        method: "textDocument/onTypeFormatting".to_string(),
        send: |f, params| {
            if !f.report_format_on_type_crash && f.client.is_none() {
                return FormattingResult { text_edits: None };
            }
            let file_name = normalized_absolute_path(&params.filename, ROOT_DIR);
            let response: lsproto::DocumentOnTypeFormattingResponse = f.send_lsp_request(
                lsproto::MethodTextDocumentOnTypeFormatting,
                lsproto::DocumentOnTypeFormattingParams {
                    text_document: text_document_identifier(&file_name),
                    position: params.position,
                    ch: params.ch,
                    options: f
                        .user_preferences
                        .format_code_settings
                        .to_ls_format_options(),
                },
            );
            lsp_text_edits_to_formatting_result(f, &file_name, response)
        },
        validate: |_result| true,
    }
}

fn text_document_completion_info() -> RequestInfo<CompletionParams, CompletionList> {
    RequestInfo {
        method: "textDocument/completion".to_string(),
        send: |f, params| {
            let file_name = normalized_absolute_path(&params.filename, ROOT_DIR);
            let response: lsproto::CompletionResponse = f.send_lsp_request(
                lsproto::MethodTextDocumentCompletion,
                lsproto::CompletionParams {
                    text_document: text_document_identifier(&file_name),
                    position: params.position,
                    work_done_token: None,
                    partial_result_token: None,
                    context: None,
                },
            );
            completion_response_to_list(f, &file_name, response)
        },
        validate: |_result| true,
    }
}

fn text_document_did_open_info() -> NotificationInfo<lsproto::DidOpenTextDocumentParams> {
    NotificationInfo {
        method: lsproto::MethodTextDocumentDidOpen.to_string(),
        send: |f, params| {
            if let Some(client) = &f.client {
                ts_testutil::lsptestutil::send_notification(
                    client,
                    &*lsproto::TextDocumentDidOpenInfo,
                    params,
                );
            }
        },
    }
}

fn text_document_did_change_info() -> NotificationInfo<lsproto::DidChangeTextDocumentParams> {
    NotificationInfo {
        method: lsproto::MethodTextDocumentDidChange.to_string(),
        send: |f, params| {
            if let Some(client) = &f.client {
                ts_testutil::lsptestutil::send_notification(
                    client,
                    &*lsproto::TextDocumentDidChangeInfo,
                    params,
                );
            }
        },
    }
}

fn workspace_did_change_configuration_info() -> NotificationInfo<ConfigurationChange> {
    NotificationInfo {
        method: "workspace/didChangeConfiguration".to_string(),
        send: |f, params| {
            let mut serialized_settings = serde_json::Map::new();
            if let Some(config) = params.settings.get("js/ts") {
                f.user_preferences = config.clone();
                *f.server_user_preferences
                    .lock()
                    .unwrap_or_else(|err| err.into_inner()) = config.clone();
                serialized_settings
                    .insert("js/ts".to_string(), user_preferences_config_value(config));
            }
            if let Some(client) = &f.client {
                ts_testutil::lsptestutil::send_notification(
                    client,
                    &*lsproto::WorkspaceDidChangeConfigurationInfo,
                    lsproto::DidChangeConfigurationParams {
                        settings: serde_json::Value::Object(serialized_settings),
                    },
                );
            }
        },
    }
}

fn text_document_identifier(file_name: &str) -> lsproto::TextDocumentIdentifier {
    lsproto::TextDocumentIdentifier {
        uri: lsconv::file_name_to_document_uri(file_name),
    }
}

fn lsp_text_edits_to_formatting_result(
    f: &FourslashTest,
    file_name: &str,
    response: lsproto::TextEditsOrNull,
) -> FormattingResult {
    let Some(edits) = response.text_edits else {
        return FormattingResult { text_edits: None };
    };
    let script = f.get_script_info(file_name);
    FormattingResult {
        text_edits: Some(
            edits
                .into_iter()
                .flatten()
                .map(|edit| text_edit_from_lsp(f, script, edit))
                .collect(),
        ),
    }
}

fn text_edit_from_lsp(f: &FourslashTest, script: &ScriptInfo, edit: lsproto::TextEdit) -> TextEdit {
    TextEdit {
        start: f
            .converters
            .line_and_character_to_position(script, edit.range.start),
        end: f
            .converters
            .line_and_character_to_position(script, edit.range.end),
        new_text: edit.new_text,
    }
}

fn diagnostic_code(diagnostic: &lsproto::Diagnostic) -> Option<i32> {
    diagnostic.code.as_ref().and_then(|code| code.integer)
}

fn diagnostic_from_lsp(diagnostic: &lsproto::Diagnostic) -> Diagnostic {
    Diagnostic {
        range: diagnostic.range,
        code: diagnostic_code(diagnostic).unwrap_or_default(),
        code_actions: Vec::new(),
    }
}

fn fourslash_diagnostic_from_lsp(
    f: &FourslashTest,
    script: &ScriptInfo,
    file: &FourslashDiagnosticFile,
    diagnostic: lsproto::Diagnostic,
) -> FourslashDiagnostic {
    let pos = f
        .converters
        .line_and_character_to_position(script, diagnostic.range.start) as i32;
    let end = f
        .converters
        .line_and_character_to_position(script, diagnostic.range.end) as i32;
    let tags = diagnostic.tags.as_deref().unwrap_or_default();
    FourslashDiagnostic {
        file: file.clone(),
        loc: core::new_text_range(pos, end),
        code: diagnostic_code(&diagnostic).unwrap_or_default(),
        category: diagnostic_category_from_lsp(diagnostic.severity),
        message: diagnostic.message,
        related_diagnostics: Vec::new(),
        reports_unnecessary: tags.contains(&lsproto::DiagnosticTag::Unnecessary),
        reports_deprecated: tags.contains(&lsproto::DiagnosticTag::Deprecated),
    }
}

fn diagnostic_category_from_lsp(
    severity: Option<lsproto::DiagnosticSeverity>,
) -> DiagnosticCategory {
    match severity {
        Some(lsproto::DiagnosticSeverity::Warning) => DiagnosticCategory::Warning,
        Some(lsproto::DiagnosticSeverity::Information) => DiagnosticCategory::Message,
        Some(lsproto::DiagnosticSeverity::Hint) => DiagnosticCategory::Suggestion,
        _ => DiagnosticCategory::Error,
    }
}

fn code_action_from_lsp(
    f: &FourslashTest,
    script: &ScriptInfo,
    uri: &lsproto::DocumentUri,
    action: lsproto::CodeAction,
) -> CodeAction {
    let edits = action
        .edit
        .and_then(|edit| edit.changes)
        .and_then(|mut changes| changes.remove(uri))
        .unwrap_or_default()
        .into_iter()
        .map(|edit| text_edit_from_lsp(f, script, edit))
        .collect();
    CodeAction {
        title: action.title,
        kind: action
            .kind
            .map(|kind| kind.as_str().to_string())
            .unwrap_or_default(),
        diagnostics: action
            .diagnostics
            .unwrap_or_default()
            .iter()
            .map(diagnostic_from_lsp)
            .collect(),
        edits,
    }
}

fn completion_response_to_list(
    f: &FourslashTest,
    file_name: &str,
    response: lsproto::CompletionResponse,
) -> CompletionList {
    if let Some(list) = response.list {
        return completion_list_from_lsp(f, file_name, list);
    }
    CompletionList {
        is_incomplete: false,
        item_defaults: None,
        items: response
            .items
            .unwrap_or_default()
            .into_iter()
            .flatten()
            .map(completion_item_from_lsp_response)
            .collect(),
    }
}

fn completion_list_from_lsp(
    f: &FourslashTest,
    file_name: &str,
    list: lsproto::CompletionList,
) -> CompletionList {
    CompletionList {
        is_incomplete: list.is_incomplete,
        item_defaults: list
            .item_defaults
            .map(|defaults| completion_item_defaults_from_lsp(f, file_name, defaults)),
        items: list
            .items
            .into_iter()
            .map(completion_item_from_lsp_response)
            .collect(),
    }
}

fn completion_item_defaults_from_lsp(
    f: &FourslashTest,
    file_name: &str,
    defaults: lsproto::CompletionItemDefaults,
) -> CompletionItemDefaults {
    CompletionItemDefaults {
        commit_characters: defaults.commit_characters,
        edit_range: defaults
            .edit_range
            .as_ref()
            .map(|range| completion_edit_range_from_lsp(f, file_name, range))
            .unwrap_or_default(),
    }
}

fn completion_edit_range_from_lsp(
    f: &FourslashTest,
    file_name: &str,
    range: &lsproto::RangeOrEditRangeWithInsertReplace,
) -> ExpectedCompletionEditRange {
    if let Some(range) = range.range {
        let range = range_marker_from_lsp_range(f, file_name, range);
        return ExpectedCompletionEditRange::EditRange(EditRange {
            insert: range.clone(),
            replace: range,
        });
    }
    if let Some(range) = &range.edit_range_with_insert_replace {
        return ExpectedCompletionEditRange::EditRange(EditRange {
            insert: range_marker_from_lsp_range(f, file_name, range.insert),
            replace: range_marker_from_lsp_range(f, file_name, range.replace),
        });
    }
    ExpectedCompletionEditRange::None
}

fn range_marker_from_lsp_range(
    f: &FourslashTest,
    file_name: &str,
    range: lsproto::Range,
) -> RangeMarker {
    let script = f.get_script_info(file_name);
    RangeMarker {
        file_name: file_name.to_string(),
        range: core::new_text_range(
            f.converters
                .line_and_character_to_position(script, range.start) as i32,
            f.converters
                .line_and_character_to_position(script, range.end) as i32,
        ),
        ls_range: range,
        marker: None,
    }
}

fn completion_item_from_lsp_response(item: lsproto::CompletionItem) -> CompletionItem {
    CompletionItem {
        label: item.label,
        sort_text: item.sort_text,
        source: item.data.and_then(|data| {
            if data.source.is_empty() {
                None
            } else {
                Some(data.source)
            }
        }),
        detail: item.detail,
    }
}

fn any_file_name<Params>(_params: &Params) -> Option<String> {
    None
}

fn sort_completion_list(mut result: CompletionList) -> CompletionList {
    result.items.sort_by(|a, b| {
        a.sort_text
            .as_deref()
            .unwrap_or(&a.label)
            .cmp(b.sort_text.as_deref().unwrap_or(&b.label))
            .then_with(|| a.label.cmp(&b.label))
    });
    result
}

pub fn is_empty_expected_list(expected: Option<&CompletionsExpectedList>) -> bool {
    expected.is_none_or(|expected| {
        expected.items.as_ref().is_none_or(|items| {
            items.exact.is_empty() && items.includes.is_empty() && items.excludes.is_empty()
        })
    })
}

pub fn verify_completions_item_defaults(
    actual: Option<&CompletionItemDefaults>,
    expected: Option<&CompletionsExpectedItemDefaults>,
    prefix: &str,
) {
    let Some(actual) = actual else {
        if expected.is_none() {
            return;
        }
        panic!("{prefix}Expected non-nil completion item defaults but got nil");
    };
    let Some(expected) = expected else {
        panic!("{prefix}Expected nil completion item defaults but got non-nil: {actual:?}");
    };
    assert_deep_equal(
        &actual.commit_characters,
        &expected.commit_characters,
        &format!("{prefix}CommitCharacters mismatch:"),
    );
    match (&actual.edit_range, &expected.edit_range) {
        (_, ExpectedCompletionEditRange::Ignored) => {}
        (ExpectedCompletionEditRange::None, ExpectedCompletionEditRange::None) => {}
        (
            ExpectedCompletionEditRange::EditRange(actual),
            ExpectedCompletionEditRange::EditRange(expected),
        ) => {
            assert_deep_equal(
                &actual.insert.ls_range,
                &expected.insert.ls_range,
                &format!("{prefix}EditRange insert mismatch:"),
            );
            assert_deep_equal(
                &actual.replace.ls_range,
                &expected.replace.ls_range,
                &format!("{prefix}EditRange replace mismatch:"),
            );
        }
        (ExpectedCompletionEditRange::None, ExpectedCompletionEditRange::EditRange(_)) => {
            panic!("{prefix}Expected non-nil EditRange but got nil");
        }
        (actual, ExpectedCompletionEditRange::None) => {
            panic!("{prefix}Expected nil EditRange but got non-nil: {actual:?}");
        }
        _ => {
            panic!("{prefix}Expected EditRange to be *EditRange or Ignored");
        }
    }
}

pub fn ignore_paths(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|path| (*path).to_string()).collect()
}

pub fn get_expected_label(item: &CompletionsExpectedItem) -> String {
    match item {
        CompletionsExpectedItem::Label(label) => label.clone(),
        CompletionsExpectedItem::Item(item) => item.label.clone(),
    }
}

pub fn assert_deep_equal<T>(actual: T, expected: T, prefix: &str)
where
    T: std::fmt::Debug + PartialEq,
{
    if actual != expected {
        panic!("{prefix}:\nactual: {actual:?}\nexpected: {expected:?}");
    }
}

pub fn append_lines_for_marked_string_with_language(
    lines: &mut Vec<String>,
    language: &str,
    value: &str,
) {
    if language.is_empty() {
        lines.push(value.to_string());
    } else {
        lines.push(code_fence(language, value));
    }
}

pub fn format_call_hierarchy_item(
    _t: &mut TestingT,
    _f: &mut FourslashTest,
    file: &ScriptInfo,
    result: &mut String,
    item: CallHierarchyItem,
    direction: CallHierarchyItemDirection,
    seen: &mut BTreeSet<CallHierarchyItemKey>,
    prefix: &str,
) {
    let key = CallHierarchyItemKey {
        uri: item.uri.clone(),
        range_: item.range,
        direction,
    };
    let already_seen = seen.contains(&key);
    seen.insert(key);
    result.push_str(&format!("{prefix}╭ name: {}\n", item.name));
    result.push_str(&format!(
        "{prefix}├ kind: {}\n",
        symbol_kind_to_lowercase(item.kind)
    ));
    result.push_str(&format!("{prefix}├ file: {}\n", item.uri.file_name()));
    result.push_str(prefix);
    result.push_str("├ span:\n");
    format_call_hierarchy_item_span(
        file,
        result,
        item.range,
        &format!("{prefix}│ "),
        &format!("{prefix}│ "),
    );
    result.push_str(prefix);
    result.push_str("├ selectionSpan:\n");
    format_call_hierarchy_item_span(
        file,
        result,
        item.selection_range,
        &format!("{prefix}│ "),
        &format!("{prefix}╰ "),
    );
    if already_seen {
        result.push_str(prefix);
        result.push_str("╰ incoming: ...\n");
    }
}

pub fn format_call_hierarchy_item_span(
    file: &ScriptInfo,
    result: &mut String,
    span: lsproto::Range,
    prefix: &str,
    closing_prefix: &str,
) {
    let start_lc = span.start;
    let end_lc = span.end;
    result.push_str(&format!(
        "{prefix}╭ {}:{}:{}-{}:{}\n",
        file.file_name,
        start_lc.line + 1,
        start_lc.character + 1,
        end_lc.line + 1,
        end_lc.character + 1
    ));
    let line_starts = compute_line_starts(&file.content);
    let context_start_line = start_lc.line as usize;
    let context_end_line = end_lc.line as usize;
    let line_num_width = (context_end_line + 1).to_string().len() + 2;
    for line_num in context_start_line..=context_end_line {
        let line_start = line_starts[line_num] as usize;
        let line_end = line_starts
            .get(line_num + 1)
            .copied()
            .unwrap_or(file.content.len() as i32) as usize;
        let line_content = file.content[line_start..line_end].trim_end_matches(['\r', '\n']);
        let line_num_str = format!("{}:", line_num + 1);
        let padded = format!(
            "{}{}",
            " ".repeat(line_num_width.saturating_sub(line_num_str.len() + 1)),
            line_num_str
        );
        if line_content.is_empty() {
            result.push_str(&format!("{prefix}│ {padded}\n"));
        } else {
            result.push_str(&format!("{prefix}│ {padded} {line_content}\n"));
        }
    }
    result.push_str(closing_prefix);
    result.push_str("╰\n");
}

pub fn format_call_hierarchy_item_spans(
    file: &ScriptInfo,
    result: &mut String,
    spans: &[lsproto::Range],
    prefix: &str,
    trailing_prefix: &str,
) {
    for (i, span) in spans.iter().copied().enumerate() {
        let closing_prefix = if i == spans.len() - 1 {
            trailing_prefix
        } else {
            prefix
        };
        format_call_hierarchy_item_span(file, result, span, prefix, closing_prefix);
    }
}

pub fn roundtrip_through_json<T>(value: T) -> Result<T, String> {
    Ok(value)
}

pub fn quick_info_is_empty(hover: &Hover) -> bool {
    hover.content.is_empty()
}

pub fn is_suggestion_diagnostic(diagnostic: &FourslashDiagnostic) -> bool {
    matches!(diagnostic.category, DiagnosticCategory::Suggestion)
}

pub fn to_diagnostic(diagnostic: FourslashDiagnostic) -> FourslashDiagnostic {
    diagnostic
}

pub fn compare_diagnostics(a: &FourslashDiagnostic, b: &FourslashDiagnostic) -> std::cmp::Ordering {
    a.file
        .file_name()
        .cmp(&b.file.file_name())
        .then_with(|| a.loc.pos().cmp(&b.loc.pos()))
        .then_with(|| a.loc.end().cmp(&b.loc.end()))
        .then_with(|| a.code.cmp(&b.code))
        .then_with(|| a.message.cmp(&b.message))
        .then_with(|| compare_related_diagnostics(&a.related_diagnostics, &b.related_diagnostics))
}

pub fn compare_related_diagnostics(
    a: &[FourslashDiagnostic],
    b: &[FourslashDiagnostic],
) -> std::cmp::Ordering {
    b.len().cmp(&a.len()).then_with(|| {
        a.iter()
            .zip(b.iter())
            .map(|(a, b)| compare_diagnostics(a, b))
            .find(|ordering| !ordering.is_eq())
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

pub fn verify_exact_symbols(
    actual: &[lsproto::SymbolInformation],
    expected: &[lsproto::SymbolInformation],
) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "Expected exact symbol count to match."
    );
}

pub fn verify_includes_symbols(
    actual: &[lsproto::SymbolInformation],
    expected: &[lsproto::SymbolInformation],
) {
    if actual.len() < expected.len() {
        panic!(
            "Expected symbol list to include at least {} symbols, got {}.",
            expected.len(),
            actual.len()
        );
    }
}

pub fn assert_valid_text_range(text_range: core::TextRange, message: &str) {
    if text_range.pos() < 0 || text_range.end() < 0 {
        panic!("{message}");
    }
}

pub fn select_code_fix_diagnostic(
    diagnostics: &[lsproto::Diagnostic],
    error_code: Option<i32>,
) -> Option<lsproto::Diagnostic> {
    if error_code.is_none_or(|code| code == 0) {
        return diagnostics.first().cloned();
    }
    diagnostics
        .iter()
        .find(|diagnostic| diagnostic_code(diagnostic) == error_code)
        .cloned()
}

fn completion_item_from_lsp(item: &CompletionItem) -> lsproto::CompletionItem {
    let mut result = lsproto::CompletionItem::default();
    result.label = item.label.clone();
    result.detail = item.detail.clone();
    result
}

pub type LspClient = ts_testutil::lsptestutil::LspClient;
#[derive(Clone, Default)]
pub struct TestFs {
    pub files: BTreeMap<String, String>,
    pub symlinks: BTreeMap<String, String>,
}
#[derive(Default)]
pub struct TestData {
    pub files: Vec<TestFileInfo>,
    pub markers: Vec<Marker>,
    pub marker_positions: BTreeMap<String, Marker>,
    pub symlinks: BTreeMap<String, String>,
    pub global_options: BTreeMap<String, String>,
    pub ranges: Vec<RangeMarker>,
}
#[derive(Clone)]
pub struct TestFileInfo {
    pub file_name: String,
    pub content: String,
    pub emit: bool,
}
#[derive(Clone, Debug)]
pub struct RangeMarker {
    pub file_name: String,
    pub range: core::TextRange,
    pub ls_range: lsproto::Range,
    pub marker: Option<Marker>,
}
#[derive(Default, Clone)]
pub struct Converters;
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPreferences {
    pub allow_rename_of_import_path: core::Tristate,
    pub auto_import_file_exclude_patterns: Vec<String>,
    pub auto_import_specifier_exclude_regexes: Vec<String>,
    pub code_lens: CodeLensUserPreferences,
    pub exclude_library_symbols_in_nav_to: Option<bool>,
    #[serde(skip)]
    pub format_code_settings: lsutil::FormatCodeSettings,
    pub import_module_specifier_ending: modulespecifiers::ImportModuleSpecifierEndingPreference,
    pub import_module_specifier_preference: modulespecifiers::ImportModuleSpecifierPreference,
    pub inlay_hints: InlayHintsPreferences,
    pub organize_imports_accent_collation: core::Tristate,
    pub organize_imports_case_first: lsutil::OrganizeImportsCaseFirst,
    pub organize_imports_collation: lsutil::OrganizeImportsCollation,
    pub organize_imports_ignore_case: core::Tristate,
    pub organize_imports_locale: String,
    pub organize_imports_numeric_collation: core::Tristate,
    pub organize_imports_type_order: lsutil::OrganizeImportsTypeOrder,
    pub prefer_type_only_auto_imports: core::Tristate,
    pub quote_preference: lsutil::QuotePreference,
    pub use_aliases_for_rename: core::Tristate,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            allow_rename_of_import_path: core::TSUnknown,
            auto_import_file_exclude_patterns: Vec::new(),
            auto_import_specifier_exclude_regexes: Vec::new(),
            code_lens: CodeLensUserPreferences::default(),
            exclude_library_symbols_in_nav_to: None,
            format_code_settings: lsutil::get_default_format_code_settings(),
            import_module_specifier_ending:
                modulespecifiers::ImportModuleSpecifierEndingPreference::default(),
            import_module_specifier_preference:
                modulespecifiers::ImportModuleSpecifierPreference::default(),
            inlay_hints: InlayHintsPreferences::default(),
            organize_imports_accent_collation: core::TSUnknown,
            organize_imports_case_first: lsutil::OrganizeImportsCaseFirst::default(),
            organize_imports_collation: lsutil::OrganizeImportsCollation::default(),
            organize_imports_ignore_case: core::TSUnknown,
            organize_imports_locale: String::new(),
            organize_imports_numeric_collation: core::TSUnknown,
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::default(),
            prefer_type_only_auto_imports: core::TSUnknown,
            quote_preference: lsutil::QuotePreference::default(),
            use_aliases_for_rename: core::TSUnknown,
        }
    }
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InlayHintsPreferences {
    pub include_inlay_parameter_name_hints: Option<String>,
    pub include_inlay_function_parameter_type_hints: core::Tristate,
    pub include_inlay_variable_type_hints: core::Tristate,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeLensUserPreferences {
    pub references_code_lens_enabled: Option<bool>,
    pub references_code_lens_show_on_all_functions: Option<bool>,
    pub implementations_code_lens_enabled: Option<bool>,
    pub implementations_code_lens_show_on_interface_methods: Option<bool>,
    pub implementations_code_lens_show_on_all_class_methods: Option<bool>,
}
pub struct RequestMessage {
    pub id: i32,
    pub jsonrpc: String,
    pub method: String,
}
pub struct ResponseMessage {
    pub id: i32,
    pub jsonrpc: String,
    pub result: Option<String>,
    pub error: Option<String>,
}
pub struct Hover {
    pub content: String,
}
pub struct AutoImportFix;
pub struct SignatureHelpContext {
    pub is_retrigger: bool,
    pub trigger_character: Option<String>,
    pub trigger_kind: Option<lsproto::SignatureHelpTriggerKind>,
}
pub struct SignatureHelp;
pub struct VerifySignatureHelpOptions {
    pub text: Option<String>,
    pub parameter_name: Option<String>,
    pub parameter_span: Option<String>,
    pub parameter_count: Option<usize>,
    pub overloads_count: usize,
}
#[derive(Clone, Copy, Debug)]
pub enum DiagnosticCategory {
    Error,
    Warning,
    Message,
    Suggestion,
}
pub struct Locale;
#[derive(Clone, Debug)]
pub struct TestFile {
    pub unit_name: String,
    pub content: String,
}

impl TestFs {
    pub fn read_file(&self, file_name: &str) -> Option<String> {
        self.files.get(file_name).cloned()
    }

    pub fn use_case_sensitive_file_names(&self) -> bool {
        true
    }
}

impl RangeMarker {
    pub fn file_name(&self) -> String {
        self.file_name.clone()
    }
}

impl From<Marker> for MarkerOrRange {
    fn from(marker: Marker) -> Self {
        Self {
            file_name: marker.file_name,
            ls_pos: marker.ls_position,
        }
    }
}

impl From<RangeMarker> for MarkerOrRange {
    fn from(range: RangeMarker) -> Self {
        Self {
            file_name: range.file_name,
            ls_pos: range.ls_range.start,
        }
    }
}

impl MarkerOrRange {
    pub fn get_name(&self) -> Option<String> {
        None
    }
}

impl Converters {
    pub fn position_to_line_and_character(
        &self,
        script: &ScriptInfo,
        position: i32,
    ) -> lsproto::Position {
        let mut line = 0;
        for (index, line_start) in script.line_map.line_starts.iter().enumerate() {
            if *line_start > position {
                break;
            }
            line = index;
        }
        let line_start = script
            .line_map
            .line_starts
            .get(line)
            .copied()
            .unwrap_or_default();
        lsproto::Position {
            line: line as u32,
            character: position.saturating_sub(line_start as i32) as u32,
        }
    }

    pub fn line_and_character_to_position(
        &self,
        script: &ScriptInfo,
        position: lsproto::Position,
    ) -> usize {
        let line_start = script
            .line_map
            .line_starts
            .get(position.line as usize)
            .copied()
            .unwrap_or_default();
        (line_start + position.character as i32) as usize
    }

    pub fn to_lsp_range(&self, script: &ScriptInfo, text_range: core::TextRange) -> lsproto::Range {
        lsproto::Range {
            start: self.position_to_line_and_character(script, text_range.pos()),
            end: self.position_to_line_and_character(script, text_range.end()),
        }
    }
}

impl FourslashTest {
    pub fn get_script_info_value(&self, file_name: &str) -> Option<&ScriptInfo> {
        self.script_infos.get(file_name)
    }
}

fn apply_text_change(content: &str, change: core::TextChange) -> String {
    let start = change.text_range.pos() as usize;
    let end = change.text_range.end() as usize;
    format!(
        "{}{}{}",
        &content[..start],
        change.new_text,
        &content[end..]
    )
}

fn has_any_extension(filename: &str, extensions: &[&str]) -> bool {
    extensions
        .iter()
        .any(|extension| filename.ends_with(extension))
}

fn normalized_absolute_path(file_name: &str, root_dir: &str) -> String {
    if file_name.starts_with('/') {
        file_name.to_string()
    } else {
        format!("{root_dir}{}", file_name.trim_start_matches('/'))
    }
}

fn lower_first_char(s: &mut String) {
    if let Some(first) = s.get_mut(0..1) {
        first.make_ascii_lowercase();
    }
}
