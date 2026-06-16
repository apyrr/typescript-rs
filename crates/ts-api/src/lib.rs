#![forbid(unsafe_code)]

mod callbackfs;
mod conn;
#[expect(
    dead_code,
    reason = "async connection owns transport for future request handling"
)]
mod conn_async;
#[expect(
    dead_code,
    reason = "sync connection owns transport for future request handling"
)]
mod conn_sync;
#[expect(
    dead_code,
    reason = "ported API encoder helpers are ahead of current callers"
)]
mod encoder;
mod proto;
mod protocol;
#[expect(
    private_interfaces,
    reason = "JSON-RPC protocol constructor intentionally hides shared transport internals"
)]
mod protocol_jsonrpc;
mod protocol_msgpack;
mod server;
#[expect(
    dead_code,
    reason = "ported API session helpers are ahead of current callers"
)]
mod session;
mod stringer_generated;
mod transport;
#[cfg(not(windows))]
mod transport_unix;
#[cfg(windows)]
mod transport_windows;

use callbackfs::{CallbackClient, CallbackFs, new_callback_fs};

pub use conn::{Conn, Error, Handler, unmarshal_params};
pub use conn_async::{AsyncConn, new_async_conn, new_async_conn_with_protocol};
pub use conn_sync::{SyncConn, new_sync_conn};
pub use proto::{
    APIFileChangeSummary, APIFileChanges, CheckerSignatureParams, CheckerTypeParams,
    ConfigFileResponse, DiagnosticResponse, DocumentIdentifier, ERR_CLIENT_ERROR,
    ERR_INVALID_REQUEST, GetBaseTypeOfLiteralTypeParams, GetContextualTypeParams,
    GetDefaultProjectForFileParams, GetDiagnosticsParams, GetExportSymbolOfSymbolParams,
    GetExportsOfSymbolParams, GetIntrinsicTypeParams, GetMembersOfSymbolParams,
    GetNonNullableTypeParams, GetParameterTypeParams, GetParentOfSymbolParams,
    GetProjectDiagnosticsParams, GetResolvedSignatureParams, GetSignaturesOfTypeParams,
    GetSourceFileParams, GetSymbolAtLocationParams, GetSymbolAtPositionParams,
    GetSymbolOfTypeParams, GetSymbolsAtLocationsParams, GetSymbolsAtPositionsParams,
    GetTypeAtLocationParams, GetTypeAtLocationsParams, GetTypeAtPositionParams,
    GetTypeFromTypeNodeParams, GetTypeOfSymbolAtLocationParams, GetTypeOfSymbolParams,
    GetTypePropertyParams, GetTypesAtPositionsParams, GetTypesOfSymbolsParams,
    GetWidenedTypeParams, IndexInfoResponse, InitializeResponse, IsArrayLikeTypeParams,
    METHOD_GET_ANY_TYPE, METHOD_GET_BASE_TYPE_OF_LITERAL_TYPE, METHOD_GET_BASE_TYPE_OF_TYPE,
    METHOD_GET_BASE_TYPES, METHOD_GET_BIG_INT_TYPE, METHOD_GET_BOOLEAN_TYPE,
    METHOD_GET_CHECK_TYPE_OF_TYPE, METHOD_GET_CONFIG_FILE_PARSING_DIAGNOSTICS,
    METHOD_GET_CONSTRAINT_OF_TYPE, METHOD_GET_CONSTRAINT_OF_TYPE_PARAMETER,
    METHOD_GET_CONTEXTUAL_TYPE, METHOD_GET_DECLARATION_DIAGNOSTICS,
    METHOD_GET_DECLARED_TYPE_OF_SYMBOL, METHOD_GET_DEFAULT_PROJECT_FOR_FILE,
    METHOD_GET_ES_SYMBOL_TYPE, METHOD_GET_EXPORT_SYMBOL_OF_SYMBOL, METHOD_GET_EXPORTS_OF_SYMBOL,
    METHOD_GET_EXTENDS_TYPE_OF_TYPE, METHOD_GET_INDEX_INFOS_OF_TYPE, METHOD_GET_INDEX_TYPE_OF_TYPE,
    METHOD_GET_LOCAL_TYPE_PARAMETERS_OF_TYPE, METHOD_GET_MEMBERS_OF_SYMBOL, METHOD_GET_NEVER_TYPE,
    METHOD_GET_NON_NULLABLE_TYPE, METHOD_GET_NULL_TYPE, METHOD_GET_NUMBER_TYPE,
    METHOD_GET_OBJECT_TYPE_OF_TYPE, METHOD_GET_OUTER_TYPE_PARAMETERS_OF_TYPE,
    METHOD_GET_PARAMETER_TYPE, METHOD_GET_PARENT_OF_SYMBOL, METHOD_GET_PROPERTIES_OF_TYPE,
    METHOD_GET_RESOLVED_SIGNATURE, METHOD_GET_REST_TYPE_OF_SIGNATURE,
    METHOD_GET_RETURN_TYPE_OF_SIGNATURE, METHOD_GET_SEMANTIC_DIAGNOSTICS,
    METHOD_GET_SHORTHAND_ASSIGNMENT_VALUE_SYMBOL, METHOD_GET_SIGNATURES_OF_TYPE,
    METHOD_GET_SOURCE_FILE, METHOD_GET_STRING_TYPE, METHOD_GET_SUGGESTION_DIAGNOSTICS,
    METHOD_GET_SYMBOL_AT_LOCATION, METHOD_GET_SYMBOL_AT_POSITION, METHOD_GET_SYMBOL_OF_TYPE,
    METHOD_GET_SYMBOLS_AT_LOCATIONS, METHOD_GET_SYMBOLS_AT_POSITIONS,
    METHOD_GET_SYNTACTIC_DIAGNOSTICS, METHOD_GET_TARGET_OF_TYPE, METHOD_GET_TYPE_ARGUMENTS,
    METHOD_GET_TYPE_AT_LOCATION, METHOD_GET_TYPE_AT_LOCATIONS, METHOD_GET_TYPE_AT_POSITION,
    METHOD_GET_TYPE_FROM_TYPE_NODE, METHOD_GET_TYPE_OF_SYMBOL,
    METHOD_GET_TYPE_OF_SYMBOL_AT_LOCATION, METHOD_GET_TYPE_PARAMETERS_OF_TYPE,
    METHOD_GET_TYPE_PREDICATE_OF_SIGNATURE, METHOD_GET_TYPES_AT_POSITIONS,
    METHOD_GET_TYPES_OF_SYMBOLS, METHOD_GET_TYPES_OF_TYPE, METHOD_GET_UNDEFINED_TYPE,
    METHOD_GET_UNKNOWN_TYPE, METHOD_GET_VOID_TYPE, METHOD_GET_WIDENED_TYPE, METHOD_INITIALIZE,
    METHOD_IS_ARRAY_LIKE_TYPE, METHOD_IS_CONTEXT_SENSITIVE, METHOD_PARSE_CONFIG_FILE,
    METHOD_PRINT_NODE, METHOD_RELEASE, METHOD_RESOLVE_NAME,
    METHOD_SIGNATURE_TO_SIGNATURE_DECLARATION, METHOD_TYPE_TO_STRING, METHOD_TYPE_TO_TYPE_NODE,
    METHOD_UPDATE_SNAPSHOT, Method, NodeHandle, ParseConfigFileParams, PrintNodeParams,
    ProjectFileChanges, ProjectHandle, ProjectResponse, ReleaseParams, ResolveNameParams,
    SignatureHandle, SignatureResponse, SignatureToSignatureDeclarationParams, SnapshotChanges,
    SnapshotHandle, SourceFileResponse, SymbolHandle, SymbolResponse, TypeHandle,
    TypePredicateResponse, TypeResponse, TypeToTypeNodeParams, UpdateSnapshotParams,
    UpdateSnapshotResponse,
};
pub use protocol::{Message, Protocol};
pub use protocol_jsonrpc::{JSONRPCProtocol, new_jsonrpc_protocol};
pub use protocol_msgpack::{
    MessagePackProtocol, MessageType, RawBinary, new_message_pack_protocol,
};
pub use server::{StdioServer, StdioServerOptions, new_stdio_server, spawn_pipe_session};
pub use session::{Session, SessionOptions, new_session, new_session_with_project_session};
pub use transport::{
    Listener, PipeTransport, ReadClose, ReadWriteClose, StdioTransport, Transport, WriteClose,
    new_pipe_transport, new_stdio_transport,
};
#[cfg(not(windows))]
pub use transport_unix::generate_pipe_path;
#[cfg(windows)]
pub use transport_windows::generate_pipe_path;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Read, Write};
    use std::sync::{Arc, Mutex};

    #[test]
    #[should_panic(expected = "StdioServerOptions.Cwd is required")]
    fn new_stdio_server_rejects_empty_cwd() {
        let _ = new_stdio_server(StdioServerOptions::default());
    }

    #[derive(Clone, Default)]
    struct SharedRw {
        data: Arc<Mutex<Vec<u8>>>,
    }

    impl SharedRw {
        fn snapshot(&self) -> Vec<u8> {
            self.data
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .clone()
        }
    }

    impl Read for SharedRw {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let mut data = self.data.lock().unwrap_or_else(|err| err.into_inner());
            let len = buf.len().min(data.len());
            buf[..len].copy_from_slice(&data[..len]);
            data.drain(..len);
            Ok(len)
        }
    }

    impl Write for SharedRw {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.data
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn jsonrpc_protocol_writes_framed_request() {
        let rw = SharedRw::default();
        let written = rw.clone();
        let protocol = new_jsonrpc_protocol(rw);

        protocol
            .write_request(None, "typescript/test", serde_json::json!({"ok": true}))
            .unwrap();

        let output = String::from_utf8(written.snapshot()).unwrap();
        assert!(output.starts_with("Content-Length: "));
        assert!(output.contains("\"method\":\"typescript/test\""));
    }
}
