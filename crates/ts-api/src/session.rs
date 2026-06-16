use std::{
    collections::HashMap,
    ops::ControlFlow,
    sync::{
        Arc, Mutex, MutexGuard, RwLock,
        atomic::{AtomicI32, AtomicU64, Ordering},
    },
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde::{Serialize, de::DeserializeOwned};
use ts_ast as ast;
use ts_astnav as astnav;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as context;
use ts_json as json;
use ts_nodebuilder as nodebuilder;
use ts_printer as printer;
use ts_project as project;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use crate::encoder;
use crate::proto::*;
use crate::{Error, Handler, RawBinary};

static SESSION_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

// snapshotData holds the per-snapshot state including the snapshot itself
// and symbol/type registries scoped to this snapshot.
// Multiple clients may hold references to the same snapshot via ref counting;
// the registries are cleaned up when refCount reaches zero.
struct SnapshotData {
    snapshot: project::SnapshotHandle,
    ref_count: AtomicI32,

    symbol_registry: RwLock<HashMap<SymbolHandle, RegisteredSymbol>>,
    source_files_by_store: RwLock<HashMap<ast::StoreId, ast::SourceFile>>,
    type_registry: RwLock<HashMap<TypeHandle, RegisteredType>>,
    signature_registry: RwLock<HashMap<SignatureHandle, RegisteredSignature>>,
    signature_next_id: AtomicU64,
}

#[derive(Clone)]
struct RegisteredSymbol {
    state_identity: checker::CheckerStateIdentity,
    symbol: ast::SymbolHandle,
    response: SymbolResponse,
}

#[derive(Clone)]
struct RegisteredType {
    state_identity: checker::CheckerStateIdentity,
    id: u32,
    response: TypeResponse,
    symbol: SymbolHandle,
    target: TypeHandle,
    types: Vec<TypeHandle>,
    type_parameters: Vec<TypeHandle>,
    outer_type_parameters: Vec<TypeHandle>,
    local_type_parameters: Vec<TypeHandle>,
    object_type: TypeHandle,
    index_type: TypeHandle,
    check_type: TypeHandle,
    extends_type: TypeHandle,
    base_type: TypeHandle,
    subst_constraint: TypeHandle,
}

impl RegisteredType {
    fn from_type(
        ch: &checker::Checker<'_, '_>,
        t: checker::TypeHandle,
        resp: &TypeResponse,
    ) -> Self {
        let flags = ch.type_flags_public(t);
        let types = if flags
            & (checker::TYPE_FLAGS_UNION_OR_INTERSECTION | checker::TYPE_FLAGS_TEMPLATE_LITERAL)
            != 0
        {
            type_handles(ch, &ch.type_types_public(t))
        } else {
            Vec::new()
        };
        Self {
            state_identity: ch.state_identity(),
            id: ch.type_id(t),
            response: resp.clone(),
            symbol: resp.symbol.clone(),
            target: resp.target.clone(),
            types,
            type_parameters: resp.type_parameters.clone(),
            outer_type_parameters: resp.outer_type_parameters.clone(),
            local_type_parameters: resp.local_type_parameters.clone(),
            object_type: resp.object_type.clone(),
            index_type: resp.index_type.clone(),
            check_type: resp.check_type.clone(),
            extends_type: resp.extends_type.clone(),
            base_type: resp.base_type.clone(),
            subst_constraint: resp.subst_constraint.clone(),
        }
    }
}

#[derive(Clone)]
struct RegisteredSignature {
    state_identity: checker::CheckerStateIdentity,
    response: SignatureResponse,
    origin: SignatureOrigin,
}

#[derive(Clone)]
enum SignatureOrigin {
    Type {
        type_handle: TypeHandle,
        kind: checker::SignatureKind,
        index: usize,
    },
    Resolved {
        location: NodeHandle,
    },
    Target {
        signature: SignatureHandle,
    },
}

impl SnapshotData {
    // getProgram looks up a program from a project handle within this snapshot.
    fn get_program(
        &self,
        project_session: &project::Session,
        project_handle: ProjectHandle,
    ) -> Result<&compiler::Program, Error> {
        let project_name = parse_project_handle(&project_handle);
        let Some(program) =
            project_session.get_snapshot_project_program(&self.snapshot, project_name.clone())
        else {
            return Err(Error::new(format!(
                "{}: project {} not found",
                ERR_CLIENT_ERROR, project_name
            )));
        };

        Ok(program)
    }

    fn register_symbol(
        &self,
        ch: &mut checker::Checker<'_, '_>,
        symbol: Option<ast::SymbolHandle>,
    ) -> Option<SymbolResponse> {
        let symbol = symbol?;
        let resp = new_symbol_response_from_handle(ch, symbol, |node| self.node_handle_from(node))?;
        self.store_registered_symbol(ch, symbol, resp.clone());
        Some(resp)
    }

    fn store_registered_symbol(
        &self,
        ch: &checker::Checker<'_, '_>,
        symbol: ast::SymbolHandle,
        response: SymbolResponse,
    ) {
        self.symbol_registry
            .write()
            .unwrap_or_else(|err| err.into_inner())
            .insert(
                response.id.clone(),
                RegisteredSymbol {
                    state_identity: ch.state_identity(),
                    symbol,
                    response,
                },
            );
    }

    fn remember_program_source_files(&self, program: &dyn checker::Program) {
        let mut source_files = self
            .source_files_by_store
            .write()
            .unwrap_or_else(|err| err.into_inner());
        for source_file in program.source_files() {
            source_files
                .entry(source_file.store().store_id())
                .or_insert_with(|| source_file.share_readonly());
        }
    }

    fn node_handle_from(&self, node: ast::Node) -> NodeHandle {
        let source_files = self
            .source_files_by_store
            .read()
            .unwrap_or_else(|err| err.into_inner());
        let source_file = source_files
            .get(&node.store_id())
            .expect("node handle requires a registered source file store");
        crate::proto::node_handle_from(source_file.store(), node)
    }

    // registerType registers a type in this snapshot's registry and returns the response.
    fn register_type(
        &self,
        ch: &checker::Checker<'_, '_>,
        t: Option<checker::TypeHandle>,
    ) -> Option<TypeResponse> {
        let t = t?;
        let resp = new_type_data(ch, t);
        let registered = RegisteredType::from_type(ch, t, &resp);
        self.type_registry
            .write()
            .unwrap_or_else(|err| err.into_inner())
            .insert(resp.id.clone(), registered);

        Some(resp)
    }

    fn resolve_symbol_handle(&self, handle: SymbolHandle) -> Result<RegisteredSymbol, Error> {
        if handle.as_str().is_empty() {
            return Err(Error::new(format!(
                "{ERR_CLIENT_ERROR}: empty symbol handle"
            )));
        }

        let symbol = self
            .symbol_registry
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .get(&handle)
            .cloned();

        symbol.ok_or_else(|| {
            Error::new(format!(
                "{}: symbol handle {:?} not found in snapshot registry",
                ERR_CLIENT_ERROR, handle
            ))
        })
    }

    fn resolve_registered_type(&self, handle: TypeHandle) -> Result<RegisteredType, Error> {
        if handle.as_str().is_empty() {
            return Err(Error::new(format!("{ERR_CLIENT_ERROR}: empty type handle")));
        }

        self.type_registry
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .get(&handle)
            .cloned()
            .ok_or_else(|| {
                Error::new(format!(
                    "{}: type handle {:?} not found in snapshot registry",
                    ERR_CLIENT_ERROR, handle
                ))
            })
    }

    fn resolve_signature(&self, handle: SignatureHandle) -> Result<RegisteredSignature, Error> {
        if handle.as_str().is_empty() {
            return Err(Error::new(format!(
                "{ERR_CLIENT_ERROR}: empty signature handle"
            )));
        }

        let sig = self
            .signature_registry
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .get(&handle)
            .cloned();

        sig.ok_or_else(|| {
            Error::new(format!(
                "{}: signature handle {:?} not found in snapshot registry",
                ERR_CLIENT_ERROR, handle
            ))
        })
    }

    fn register_signature(
        &self,
        ch: &mut checker::Checker<'_, '_>,
        sig: Option<checker::SignatureHandle>,
        origin: SignatureOrigin,
    ) -> Option<SignatureResponse> {
        let sig = sig?;
        let id = self.signature_next_id.fetch_add(1, Ordering::SeqCst) + 1;
        let handle = signature_handle(ch, id);

        let mut resp = SignatureResponse {
            id: handle.clone(),
            flags: ch.signature_flags_public(sig),
            ..Default::default()
        };

        if let Some(declaration) = ch.signature_declaration_public(sig) {
            resp.declaration = self.node_handle_from(declaration);
        }

        let type_parameters = ch.signature_type_parameters_public(sig);
        if !type_parameters.is_empty() {
            resp.type_parameters = type_handles(ch, &type_parameters);
            let mut type_registry = self
                .type_registry
                .write()
                .unwrap_or_else(|err| err.into_inner());
            for &tp in &type_parameters {
                let resp = new_type_data(ch, tp);
                type_registry.insert(resp.id.clone(), RegisteredType::from_type(ch, tp, &resp));
            }
        }

        let parameters = ch.signature_parameter_symbol_identities_public(sig);
        if !parameters.is_empty() {
            resp.parameters = parameters
                .iter()
                .copied()
                .map(|symbol| symbol_handle(ch, symbol.symbol_handle()))
                .collect();
            let mut symbol_registry = self
                .symbol_registry
                .write()
                .unwrap_or_else(|err| err.into_inner());
            for param in parameters {
                if let Some(response) =
                    new_symbol_response_from_handle(ch, param.symbol_handle(), |node| {
                        self.node_handle_from(node)
                    })
                {
                    symbol_registry.insert(
                        response.id.clone(),
                        RegisteredSymbol {
                            state_identity: ch.state_identity(),
                            symbol: param.symbol_handle(),
                            response,
                        },
                    );
                }
            }
        }

        if let Some(this_parameter) = ch.signature_this_parameter_symbol_identity_public(sig) {
            let this_parameter = this_parameter.symbol_handle();
            if let Some(response) = new_symbol_response_from_handle(ch, this_parameter, |node| {
                self.node_handle_from(node)
            }) {
                resp.this_parameter = response.id.clone();
                self.store_registered_symbol(ch, this_parameter, response);
            }
        }

        if let Some(target) = ch.signature_target_public(sig) {
            if let Some(target_resp) = self.register_signature(
                ch,
                Some(target),
                SignatureOrigin::Target {
                    signature: handle.clone(),
                },
            ) {
                resp.target = target_resp.id;
            }
        }

        self.signature_registry
            .write()
            .unwrap_or_else(|err| err.into_inner())
            .insert(
                handle,
                RegisteredSignature {
                    state_identity: ch.state_identity(),
                    response: resp.clone(),
                    origin,
                },
            );

        Some(resp)
    }
}

// Session represents an API session that provides programmatic access
// to TypeScript language services through the LSP server.
// It implements the Handler interface to process incoming API requests.
// The session supports multiple active snapshots, each with their own
// symbol and type registries scoped to checker state identities.
pub struct Session {
    id: String,
    project_session: Arc<Mutex<project::Session>>,

    // This is set to true when using MessagePackProtocol.
    use_binary_responses: bool,

    // snapshots maps snapshot handles to their data.
    // Each snapshot has its own symbol/type registries.
    snapshots: RwLock<HashMap<SnapshotHandle, Arc<SnapshotData>>>,

    // latestSnapshot tracks the most recently created snapshot for computing diffs.
    latest_snapshot: RwLock<SnapshotHandle>,
}

// SessionOptions configures an API session.
#[derive(Debug, Clone, Default)]
pub struct SessionOptions {
    // UseBinaryResponses enables binary responses for msgpack protocol.
    pub use_binary_responses: bool,
}

// NewSession creates a new API session with the given project session.
pub fn new_session(project_session: project::Session, options: SessionOptions) -> Session {
    new_session_with_project_session(Arc::new(Mutex::new(project_session)), options)
}

pub fn new_session_with_project_session(
    project_session: Arc<Mutex<project::Session>>,
    options: SessionOptions,
) -> Session {
    let id = SESSION_ID_COUNTER.fetch_add(1, Ordering::SeqCst) + 1;
    Session {
        id: format_session_id(id),
        project_session,
        use_binary_responses: options.use_binary_responses,
        snapshots: RwLock::new(HashMap::new()),
        latest_snapshot: RwLock::new(SnapshotHandle::default()),
    }
}

impl Session {
    // ID returns the unique identifier for this session.
    pub fn id(&self) -> &str {
        &self.id
    }

    // ProjectSession returns the underlying project session.
    fn project_session(&self) -> MutexGuard<'_, project::Session> {
        self.project_session
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }

    // getSnapshotData looks up snapshot data by handle.
    fn get_snapshot_data(&self, handle: &SnapshotHandle) -> Result<Arc<SnapshotData>, Error> {
        self.snapshots
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .get(handle)
            .cloned()
            .ok_or_else(|| Error::new(format!("{ERR_CLIENT_ERROR}: snapshot {handle} not found")))
    }

    fn with_checker_for_file_name<R, F>(
        &self,
        ctx: &context::Context,
        snapshot: &SnapshotHandle,
        project_handle: ProjectHandle,
        file_name: &str,
        f: F,
    ) -> Result<R, Error>
    where
        F: for<'program, 'access, 'checker, 'state> FnOnce(
            ScopedCheckerSetup<'program, 'access, 'checker, 'state>,
            &'program ast::SourceFile,
        ) -> Result<R, Error>,
    {
        let sd = self.get_snapshot_data(snapshot)?;
        let setup_sd = sd.clone();
        let project_session = self.project_session();
        let program = sd.get_program(&project_session, project_handle)?;
        sd.remember_program_source_files(program);
        let checker_file = program.get_source_file_ref(file_name).ok_or_else(|| {
            Error::new(format!(
                "{ERR_CLIENT_ERROR}: source file not found: {file_name}"
            ))
        })?;
        program.with_type_checker_for_file_using(
            compiler::CheckerAccess::context(ctx),
            checker_file,
            |checker| {
                f(
                    ScopedCheckerSetup {
                        sd: setup_sd,
                        program,
                        checker,
                    },
                    checker_file,
                )
            },
        )
    }

    fn with_checker_for_node_handle<R, F>(
        &self,
        ctx: &context::Context,
        snapshot: &SnapshotHandle,
        project_handle: ProjectHandle,
        handle: NodeHandle,
        f: F,
    ) -> Result<R, Error>
    where
        F: for<'program, 'access, 'checker, 'state> FnOnce(
            ScopedCheckerSetup<'program, 'access, 'checker, 'state>,
            &'program ast::SourceFile,
            Option<ast::Node>,
        ) -> Result<R, Error>,
    {
        let sd = self.get_snapshot_data(snapshot)?;
        let setup_sd = sd.clone();
        let project_session = self.project_session();
        let program = sd.get_program(&project_session, project_handle)?;
        sd.remember_program_source_files(program);
        let checker_file = self.source_file_for_node_handle(program, &handle)?;
        program.with_type_checker_for_file_using(
            compiler::CheckerAccess::context(ctx),
            checker_file,
            |checker| {
                let node = self.resolve_node_handle(program, handle)?;
                debug_assert!(
                    node.is_none_or(|node| node.store_id() == checker_file.store().store_id())
                );
                f(
                    ScopedCheckerSetup {
                        sd: setup_sd,
                        program,
                        checker,
                    },
                    checker_file,
                    node,
                )
            },
        )
    }

    fn with_checker_for_symbol_handle<R, F>(
        &self,
        ctx: &context::Context,
        snapshot: &SnapshotHandle,
        project_handle: ProjectHandle,
        handle: SymbolHandle,
        f: F,
    ) -> Result<R, Error>
    where
        F: for<'program, 'access, 'checker, 'state> FnOnce(
            ScopedCheckerSetup<'program, 'access, 'checker, 'state>,
            ast::SymbolHandle,
        ) -> Result<R, Error>,
    {
        let sd = self.get_snapshot_data(snapshot)?;
        let setup_sd = sd.clone();
        let project_session = self.project_session();
        let program = sd.get_program(&project_session, project_handle)?;
        sd.remember_program_source_files(program);
        let symbol = sd.resolve_symbol_handle(handle.clone())?;
        let declaration = self.checker_declaration_for_symbol(program, handle, &symbol.response)?;
        let checker_file = self.source_file_for_program_node(program, declaration)?;
        program.with_type_checker_for_file_using(
            compiler::CheckerAccess::context(ctx),
            checker_file,
            |checker| {
                f(
                    ScopedCheckerSetup {
                        sd: setup_sd,
                        program,
                        checker,
                    },
                    symbol.symbol,
                )
            },
        )
    }

    fn resolve_signature_handle_in_setup<'program, 'access, 'checker, 'state>(
        &self,
        setup: &mut ScopedCheckerSetup<'program, 'access, 'checker, 'state>,
        handle: SignatureHandle,
    ) -> Result<checker::SignatureHandle, Error> {
        let registered = setup.sd.resolve_signature(handle.clone())?;
        if registered.state_identity != setup.checker.state_identity() {
            return Err(Error::new(format!(
                "{}: signature handle {:?} belongs to checker slot {} generation {}, active checker is slot {} generation {}",
                ERR_CLIENT_ERROR,
                handle,
                registered.state_identity.slot().get(),
                registered.state_identity.generation().get(),
                setup.checker.state_identity().slot().get(),
                setup.checker.state_identity().generation().get()
            )));
        }
        match registered.origin {
            SignatureOrigin::Type {
                type_handle,
                kind,
                index,
            } => {
                let t = setup.resolve_type_handle(type_handle)?;
                let signatures = setup.checker.get_signatures_of_type_public(t, kind);
                signatures.get(index).copied().ok_or_else(|| {
                    Error::new(format!(
                        "{}: signature handle {:?} not found in active checker",
                        ERR_CLIENT_ERROR, handle
                    ))
                })
            }
            SignatureOrigin::Resolved { location } => {
                let Some(node) = self.resolve_node_handle(setup.program, location)? else {
                    return Err(Error::new(format!(
                        "{}: signature handle {:?} has no resolved node",
                        ERR_CLIENT_ERROR, handle
                    )));
                };
                setup
                    .checker
                    .get_resolved_signature_public(node)
                    .ok_or_else(|| {
                        Error::new(format!(
                            "{}: signature handle {:?} not found in active checker",
                            ERR_CLIENT_ERROR, handle
                        ))
                    })
            }
            SignatureOrigin::Target { signature } => {
                let sig = self.resolve_signature_handle_in_setup(setup, signature)?;
                setup.checker.signature_target_public(sig).ok_or_else(|| {
                    Error::new(format!(
                        "{}: signature handle {:?} target not found",
                        ERR_CLIENT_ERROR, handle
                    ))
                })
            }
        }
    }

    fn with_checker_for_state_identity<R, F>(
        &self,
        ctx: &context::Context,
        snapshot: &SnapshotHandle,
        project_handle: ProjectHandle,
        identity: checker::CheckerStateIdentity,
        f: F,
    ) -> Result<R, Error>
    where
        F: for<'program, 'access, 'checker, 'state> FnOnce(
            ScopedCheckerSetup<'program, 'access, 'checker, 'state>,
        ) -> Result<R, Error>,
    {
        let sd = self.get_snapshot_data(snapshot)?;
        let setup_sd = sd.clone();
        let project_session = self.project_session();
        let program = sd.get_program(&project_session, project_handle)?;
        sd.remember_program_source_files(program);
        program.with_type_checker_for_state_identity_using(
            compiler::CheckerAccess::context(ctx),
            identity,
            |checker| {
                f(ScopedCheckerSetup {
                    sd: setup_sd,
                    program,
                    checker,
                })
            },
        )
    }

    fn with_checker_for_type_handle<R, F>(
        &self,
        ctx: &context::Context,
        snapshot: &SnapshotHandle,
        project_handle: ProjectHandle,
        handle: TypeHandle,
        f: F,
    ) -> Result<R, Error>
    where
        F: for<'program, 'access, 'checker, 'state> FnOnce(
            ScopedCheckerSetup<'program, 'access, 'checker, 'state>,
        ) -> Result<R, Error>,
    {
        let state_identity = self
            .get_snapshot_data(snapshot)?
            .resolve_registered_type(handle)?
            .state_identity;
        self.with_checker_for_state_identity(ctx, snapshot, project_handle, state_identity, f)
    }

    fn with_checker_for_signature_handle<R, F>(
        &self,
        ctx: &context::Context,
        snapshot: &SnapshotHandle,
        project_handle: ProjectHandle,
        handle: SignatureHandle,
        f: F,
    ) -> Result<R, Error>
    where
        F: for<'program, 'access, 'checker, 'state> FnOnce(
            ScopedCheckerSetup<'program, 'access, 'checker, 'state>,
        ) -> Result<R, Error>,
    {
        let state_identity = self
            .get_snapshot_data(snapshot)?
            .resolve_signature(handle)?
            .state_identity;
        self.with_checker_for_state_identity(ctx, snapshot, project_handle, state_identity, f)
    }

    // HandleRequest implements Handler.
    fn handle_request(
        &self,
        ctx: &context::Context,
        method: &str,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        // Handle simple methods that don't need param parsing
        match method {
            "echo" => {
                // Return raw binary for msgpack protocol compatibility
                if self.use_binary_responses {
                    let data =
                        json::marshal(&params, &[]).map_err(|err| Error::new(err.to_string()))?;
                    return marshal(&RawBinary::from(data));
                }
                return Ok(params);
            }
            "ping" => return marshal(&"pong"),
            _ => {}
        }

        let parsed = unmarshal_payload(method, params)
            .map_err(|err| Error::new(format!("{ERR_INVALID_REQUEST}: {err}")))?;

        match method {
            METHOD_RELEASE => self.handle_release(ctx, parsed),
            METHOD_INITIALIZE => self.handle_initialize(ctx),
            METHOD_UPDATE_SNAPSHOT => self.handle_update_snapshot(ctx, parsed),
            METHOD_PARSE_CONFIG_FILE => self.handle_parse_config_file(ctx, parsed),
            METHOD_GET_DEFAULT_PROJECT_FOR_FILE => {
                self.handle_get_default_project_for_file(ctx, parsed)
            }
            METHOD_GET_SOURCE_FILE => self.handle_get_source_file(ctx, parsed),
            METHOD_GET_SYMBOL_AT_POSITION => self.handle_get_symbol_at_position(ctx, parsed),
            METHOD_GET_SYMBOLS_AT_POSITIONS => self.handle_get_symbols_at_positions(ctx, parsed),
            METHOD_GET_SYMBOL_AT_LOCATION => self.handle_get_symbol_at_location(ctx, parsed),
            METHOD_GET_SYMBOLS_AT_LOCATIONS => self.handle_get_symbols_at_locations(ctx, parsed),
            METHOD_GET_TYPE_OF_SYMBOL => self.handle_get_type_of_symbol(ctx, parsed),
            METHOD_GET_TYPES_OF_SYMBOLS => self.handle_get_types_of_symbols(ctx, parsed),
            METHOD_GET_DECLARED_TYPE_OF_SYMBOL => {
                self.handle_get_declared_type_of_symbol(ctx, parsed)
            }
            METHOD_RESOLVE_NAME => self.handle_resolve_name(ctx, parsed),
            METHOD_GET_PARENT_OF_SYMBOL => self.handle_get_parent_of_symbol(ctx, parsed),
            METHOD_GET_MEMBERS_OF_SYMBOL => self.handle_get_members_of_symbol(ctx, parsed),
            METHOD_GET_EXPORTS_OF_SYMBOL => self.handle_get_exports_of_symbol(ctx, parsed),
            METHOD_GET_EXPORT_SYMBOL_OF_SYMBOL => {
                self.handle_get_export_symbol_of_symbol(ctx, parsed)
            }
            METHOD_GET_SYMBOL_OF_TYPE => self.handle_get_symbol_of_type(ctx, parsed),
            METHOD_GET_SIGNATURES_OF_TYPE => self.handle_get_signatures_of_type(ctx, parsed),
            METHOD_GET_RESOLVED_SIGNATURE => self.handle_get_resolved_signature(ctx, parsed),
            METHOD_GET_TYPE_AT_LOCATION => self.handle_get_type_at_location(ctx, parsed),
            METHOD_GET_TYPE_AT_LOCATIONS => self.handle_get_type_at_locations(ctx, parsed),
            METHOD_GET_TYPE_AT_POSITION => self.handle_get_type_at_position(ctx, parsed),
            METHOD_GET_TYPES_AT_POSITIONS => self.handle_get_types_at_positions(ctx, parsed),
            METHOD_GET_TARGET_OF_TYPE => self.handle_get_target_of_type(ctx, parsed),
            METHOD_GET_TYPES_OF_TYPE => self.handle_get_types_of_type(ctx, parsed),
            METHOD_GET_TYPE_PARAMETERS_OF_TYPE => {
                self.handle_get_type_parameters_of_type(ctx, parsed)
            }
            METHOD_GET_OUTER_TYPE_PARAMETERS_OF_TYPE => {
                self.handle_get_outer_type_parameters_of_type(ctx, parsed)
            }
            METHOD_GET_LOCAL_TYPE_PARAMETERS_OF_TYPE => {
                self.handle_get_local_type_parameters_of_type(ctx, parsed)
            }
            METHOD_GET_OBJECT_TYPE_OF_TYPE => self.handle_get_object_type_of_type(ctx, parsed),
            METHOD_GET_INDEX_TYPE_OF_TYPE => self.handle_get_index_type_of_type(ctx, parsed),
            METHOD_GET_CHECK_TYPE_OF_TYPE => self.handle_get_check_type_of_type(ctx, parsed),
            METHOD_GET_EXTENDS_TYPE_OF_TYPE => self.handle_get_extends_type_of_type(ctx, parsed),
            METHOD_GET_BASE_TYPE_OF_TYPE => self.handle_get_base_type_of_type(ctx, parsed),
            METHOD_GET_CONSTRAINT_OF_TYPE => self.handle_get_constraint_of_type(ctx, parsed),
            METHOD_GET_CONTEXTUAL_TYPE => self.handle_get_contextual_type(ctx, parsed),
            METHOD_GET_BASE_TYPE_OF_LITERAL_TYPE => {
                self.handle_get_base_type_of_literal_type(ctx, parsed)
            }
            METHOD_GET_NON_NULLABLE_TYPE => self.handle_get_non_nullable_type(ctx, parsed),
            METHOD_GET_TYPE_FROM_TYPE_NODE => self.handle_get_type_from_type_node(ctx, parsed),
            METHOD_GET_WIDENED_TYPE => self.handle_get_widened_type(ctx, parsed),
            METHOD_GET_PARAMETER_TYPE => self.handle_get_parameter_type(ctx, parsed),
            METHOD_IS_ARRAY_LIKE_TYPE => self.handle_is_array_like_type(ctx, parsed),
            METHOD_GET_SHORTHAND_ASSIGNMENT_VALUE_SYMBOL => {
                self.handle_get_shorthand_assignment_value_symbol(ctx, parsed)
            }
            METHOD_GET_TYPE_OF_SYMBOL_AT_LOCATION => {
                self.handle_get_type_of_symbol_at_location(ctx, parsed)
            }
            METHOD_TYPE_TO_TYPE_NODE => self.handle_type_to_type_node(ctx, parsed),
            METHOD_SIGNATURE_TO_SIGNATURE_DECLARATION => {
                self.handle_signature_to_signature_declaration(ctx, parsed)
            }
            METHOD_TYPE_TO_STRING => self.handle_type_to_string(ctx, parsed),
            METHOD_PRINT_NODE => self.handle_print_node(ctx, parsed),
            METHOD_IS_CONTEXT_SENSITIVE => self.handle_is_context_sensitive(ctx, parsed),
            METHOD_GET_RETURN_TYPE_OF_SIGNATURE => {
                self.handle_get_return_type_of_signature(ctx, parsed)
            }
            METHOD_GET_REST_TYPE_OF_SIGNATURE => {
                self.handle_get_rest_type_of_signature(ctx, parsed)
            }
            METHOD_GET_TYPE_PREDICATE_OF_SIGNATURE => {
                self.handle_get_type_predicate_of_signature(ctx, parsed)
            }
            METHOD_GET_BASE_TYPES => self.handle_get_base_types(ctx, parsed),
            METHOD_GET_PROPERTIES_OF_TYPE => self.handle_get_properties_of_type(ctx, parsed),
            METHOD_GET_INDEX_INFOS_OF_TYPE => self.handle_get_index_infos_of_type(ctx, parsed),
            METHOD_GET_CONSTRAINT_OF_TYPE_PARAMETER => {
                self.handle_get_constraint_of_type_parameter(ctx, parsed)
            }
            METHOD_GET_TYPE_ARGUMENTS => self.handle_get_type_arguments(ctx, parsed),
            METHOD_GET_ANY_TYPE
            | METHOD_GET_STRING_TYPE
            | METHOD_GET_NUMBER_TYPE
            | METHOD_GET_BOOLEAN_TYPE
            | METHOD_GET_VOID_TYPE
            | METHOD_GET_UNDEFINED_TYPE
            | METHOD_GET_NULL_TYPE
            | METHOD_GET_NEVER_TYPE
            | METHOD_GET_UNKNOWN_TYPE
            | METHOD_GET_BIG_INT_TYPE
            | METHOD_GET_ES_SYMBOL_TYPE => self.handle_get_intrinsic_type(ctx, method, parsed),
            METHOD_GET_SYNTACTIC_DIAGNOSTICS => self.handle_get_syntactic_diagnostics(ctx, parsed),
            METHOD_GET_SEMANTIC_DIAGNOSTICS => self.handle_get_semantic_diagnostics(ctx, parsed),
            METHOD_GET_SUGGESTION_DIAGNOSTICS => {
                self.handle_get_suggestion_diagnostics(ctx, parsed)
            }
            METHOD_GET_DECLARATION_DIAGNOSTICS => {
                self.handle_get_declaration_diagnostics(ctx, parsed)
            }
            METHOD_GET_CONFIG_FILE_PARSING_DIAGNOSTICS => {
                self.handle_get_config_file_parsing_diagnostics(ctx, parsed)
            }
            _ => Err(Error::new(format!("unknown method: {method}"))),
        }
    }

    // HandleNotification implements Handler.
    fn handle_notification(
        &self,
        _ctx: &context::Context,
        _method: &str,
        _params: json::Value,
    ) -> Result<(), Error> {
        // TODO: Implement notification handling
        Ok(())
    }

    fn handle_initialize(&self, _ctx: &context::Context) -> Result<json::Value, Error> {
        let project_session = self.project_session();
        marshal(&InitializeResponse {
            use_case_sensitive_file_names: project_session.fs().use_case_sensitive_file_names(),
            current_directory: project_session.get_current_directory().to_string(),
        })
    }
}

struct ScopedCheckerSetup<'program, 'access, 'checker, 'state> {
    sd: Arc<SnapshotData>,
    program: &'program compiler::Program,
    checker: &'access mut compiler::ActiveChecker<'program, 'checker, 'state>,
}

impl<'program, 'access, 'checker, 'state> ScopedCheckerSetup<'program, 'access, 'checker, 'state> {
    fn resolve_type_handle(&self, handle: TypeHandle) -> Result<checker::TypeHandle, Error> {
        let registered = self.sd.resolve_registered_type(handle.clone())?;
        if registered.state_identity != self.checker.state_identity() {
            return Err(Error::new(format!(
                "{}: type handle {:?} belongs to checker slot {} generation {}, active checker is slot {} generation {}",
                ERR_CLIENT_ERROR,
                handle,
                registered.state_identity.slot().get(),
                registered.state_identity.generation().get(),
                self.checker.state_identity().slot().get(),
                self.checker.state_identity().generation().get()
            )));
        }
        let type_id = registered.id;
        self.checker.find_type_by_id(type_id).ok_or_else(|| {
            Error::new(format!(
                "{}: type handle {:?} not found in active checker",
                ERR_CLIENT_ERROR, handle
            ))
        })
    }
}

fn decode_params<T>(params: json::Value) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    serde_json::from_value(params)
        .map_err(|err| Error::new(format!("{ERR_INVALID_REQUEST}: {err}")))
}

fn marshal<T>(value: &T) -> Result<json::Value, Error>
where
    T: Serialize,
{
    serde_json::to_value(value).map_err(|err| Error::new(err.to_string()))
}

fn marshal_binary_or_source(
    data: Vec<u8>,
    use_binary_responses: bool,
) -> Result<json::Value, Error> {
    if use_binary_responses {
        marshal(&RawBinary::from(data))
    } else {
        marshal(&SourceFileResponse {
            data: BASE64_STANDARD.encode(data),
        })
    }
}

impl Handler for Session {
    fn handle_request(
        &self,
        ctx: &context::Context,
        method: &str,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        self.handle_request(ctx, method, params)
    }

    fn handle_notification(
        &self,
        ctx: &context::Context,
        method: &str,
        params: json::Value,
    ) -> Result<(), Error> {
        self.handle_notification(ctx, method, params)
    }
}

impl Session {
    // handleUpdateSnapshot creates a new snapshot, optionally opening a project.
    // With no args, it adopts the latest LSP state.
    // With OpenProject set, it opens the specified project in the new snapshot.
    fn handle_update_snapshot(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: UpdateSnapshotParams = decode_params(params)?;
        let file_changes = self.to_file_change_summary(params.file_changes.as_ref());

        let snapshot = if !params.open_project.is_empty() {
            let config_file_name = self.to_absolute_file_name(&params.open_project);
            let mut project_session = self.project_session();
            let (_, new_snapshot, err) =
                project_session.api_open_project(ctx, config_file_name, file_changes);
            if let Some(err) = err {
                // APIOpenProject returns a ref'd snapshot even on error; release it.
                project_session.release_snapshot_handle(&new_snapshot);
                return Err(Error::new(format!(
                    "{ERR_CLIENT_ERROR}: failed to load project: {err}"
                )));
            }
            new_snapshot
        } else {
            // Even when fileChanges is empty, APIUpdateWithFileChanges ensures all projects
            // opened by the API are up to date. For an API connected to an LSP server, this
            // brings the API state up to date with the LSP state and ensures projects the
            // API cares about are ready to be queried.
            self.project_session()
                .api_update_with_file_changes(ctx, file_changes)
        };

        // Create or ref-count snapshot data.
        // If the same snapshot ID is returned (no changes), we increment the
        // ref count so each client-side Snapshot can be disposed independently.
        let handle = snapshot_handle(&snapshot);
        let sd = {
            let mut snapshots = self
                .snapshots
                .write()
                .unwrap_or_else(|err| err.into_inner());
            if let Some(sd) = snapshots.get(&handle) {
                // Same snapshot already stored - release the caller's ref since
                // the stored snapshot already has one, and bump the API refcount.
                self.project_session().release_snapshot_handle(&snapshot);
                sd.ref_count.fetch_add(1, Ordering::SeqCst);
                sd.clone()
            } else {
                let sd = Arc::new(SnapshotData {
                    snapshot,
                    ref_count: AtomicI32::new(1),
                    symbol_registry: RwLock::new(HashMap::new()),
                    source_files_by_store: RwLock::new(HashMap::new()),
                    type_registry: RwLock::new(HashMap::new()),
                    signature_registry: RwLock::new(HashMap::new()),
                    signature_next_id: AtomicU64::new(0),
                });
                snapshots.insert(handle.clone(), sd.clone());
                sd
            }
        };

        // Build projects list
        let projects = self.project_session().snapshot_project_infos(&sd.snapshot);
        let project_responses = projects
            .iter()
            .map(|project| new_project_response(project))
            .collect::<Vec<_>>();

        // Compute changes from the previous latest snapshot
        let changes = {
            let latest_snapshot = self
                .latest_snapshot
                .read()
                .unwrap_or_else(|err| err.into_inner())
                .clone();
            let snapshots = self.snapshots.read().unwrap_or_else(|err| err.into_inner());
            snapshots.get(&latest_snapshot).map(|prev_sd| {
                compute_snapshot_changes(&self.project_session(), &prev_sd.snapshot, &sd.snapshot)
            })
        };

        // Update the latest snapshot
        *self
            .latest_snapshot
            .write()
            .unwrap_or_else(|err| err.into_inner()) = handle.clone();

        marshal(&UpdateSnapshotResponse {
            snapshot: handle,
            projects: project_responses,
            changes,
        })
    }

    // handleRelease decrements the ref count for a snapshot.
    // The snapshot and its registries are only cleaned up when the ref count reaches zero.
    fn handle_release(
        &self,
        _ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: ReleaseParams = decode_params(params)?;
        if params.handle.is_empty() {
            return Err(Error::new(format!("{ERR_CLIENT_ERROR}: empty handle")));
        }

        let Some(prefix) = params.handle.chars().next() else {
            return Err(Error::new(format!("{ERR_CLIENT_ERROR}: empty handle")));
        };
        if prefix != HANDLE_PREFIX_SNAPSHOT_FOR_SESSION {
            return Err(Error::new(format!(
                "{ERR_CLIENT_ERROR}: can only release snapshot handles, got prefix {prefix:?}"
            )));
        }

        let snapshot_handle = SnapshotHandle::new(params.handle);
        let mut snapshots = self
            .snapshots
            .write()
            .unwrap_or_else(|err| err.into_inner());
        let Some(sd) = snapshots.get(&snapshot_handle).cloned() else {
            return Err(Error::new(format!(
                "{ERR_CLIENT_ERROR}: snapshot {snapshot_handle} not found"
            )));
        };

        if sd.ref_count.fetch_sub(1, Ordering::SeqCst) <= 1 {
            snapshots.remove(&snapshot_handle);
            // Release the API session's ref on the project snapshot.
            self.project_session().release_snapshot_handle(&sd.snapshot);
        }

        marshal(&true)
    }

    // handleGetDefaultProjectForFile returns the default project for a given file.
    fn handle_get_default_project_for_file(
        &self,
        _ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetDefaultProjectForFileParams = decode_params(params)?;
        let sd = self.get_snapshot_data(&params.snapshot)?;

        let uri = params.file.to_uri();
        let Some(proj) = self
            .project_session()
            .get_snapshot_default_project_info(&sd.snapshot, uri)
        else {
            return Err(Error::new(format!(
                "{ERR_CLIENT_ERROR}: no project found for file {:?}",
                params.file
            )));
        };

        marshal(&new_project_response(&proj))
    }

    // handleParseConfigFile parses a tsconfig.json file and returns its contents.
    fn handle_parse_config_file(
        &self,
        _ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: ParseConfigFileParams = decode_params(params)?;
        let project_session = self.project_session();
        let config_file_name = params
            .file
            .to_absolute_file_name(&project_session.get_current_directory());
        let (config_file_content, ok) = project_session.fs().read_file(&config_file_name);
        if !ok {
            return Err(Error::new(format!(
                "{ERR_CLIENT_ERROR}: could not read file {config_file_name:?}"
            )));
        }

        let config_dir = tspath::get_directory_path(&config_file_name);
        let ts_config_source_file = tsoptions::new_tsconfig_source_file_from_file_path(
            &config_file_name,
            self.to_path(&config_file_name),
            &config_file_content,
        );
        let parsed_command_line = tsoptions::parse_json_source_file_config_file_content(
            tsoptions::ParseJsonSourceFileConfigFileContentInput {
                source_file: ts_config_source_file,
                host: &*project_session,
                base_path: &config_dir,
                existing_options: None,
                existing_options_raw: None,
                config_file_name: &config_file_name,
                resolution_stack: &[],
                extra_file_extensions: &[],
                extended_config_cache: None,
            },
        );

        marshal(&ConfigFileResponse {
            file_names: parsed_command_line.file_names().to_vec(),
            options: Some(parsed_command_line.compiler_options()),
        })
    }

    // handleGetSourceFile returns a source file from a project within a snapshot.
    fn handle_get_source_file(
        &self,
        _ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetSourceFileParams = decode_params(params)?;
        let sd = self.get_snapshot_data(&params.snapshot)?;
        let project_session = self.project_session();
        let program = sd.get_program(&project_session, params.project)?;

        let Some(source_file) = program.get_source_file_ref(&params.file.to_file_name()) else {
            if self.use_binary_responses {
                return marshal(&RawBinary::from(Vec::<u8>::new()));
            }
            return Ok(json::Value::Null);
        };

        // Encode the full source file
        let data = encoder::encode_source_file(source_file)
            .map_err(|err| Error::new(format!("failed to encode source file: {err}")))?;

        // Return raw binary for msgpack protocol, or base64 for JSON
        marshal_binary_or_source(data, self.use_binary_responses)
    }

    // handleGetSymbolAtPosition returns the symbol at a position in a file.
    fn handle_get_symbol_at_position(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetSymbolAtPositionParams = decode_params(params)?;
        let file_name = params.file.to_file_name();
        self.with_checker_for_file_name(
            ctx,
            &params.snapshot,
            params.project,
            &file_name,
            |mut setup, source_file| {
                let position_map = source_file.get_position_map();
                let Some(node) = astnav::get_touching_property_name(
                    source_file,
                    position_map.utf16_to_utf8(params.position as i32),
                ) else {
                    return Ok(json::Value::Null);
                };

                let Some(symbol) = setup.checker.get_symbol_identity_at_location_public(node)
                else {
                    return Ok(json::Value::Null);
                };

                marshal(
                    &setup
                        .sd
                        .register_symbol(&mut setup.checker, Some(symbol.symbol_handle())),
                )
            },
        )
    }

    // handleGetSymbolsAtPositions returns symbols at multiple positions in a file.
    fn handle_get_symbols_at_positions(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetSymbolsAtPositionsParams = decode_params(params)?;
        let file_name = params.file.to_file_name();
        self.with_checker_for_file_name(
            ctx,
            &params.snapshot,
            params.project,
            &file_name,
            |mut setup, source_file| {
                let position_map = source_file.get_position_map();
                let mut results = vec![None; params.positions.len()];
                for (i, pos) in params.positions.iter().enumerate() {
                    let Some(node) = astnav::get_touching_property_name(
                        source_file,
                        position_map.utf16_to_utf8(*pos as i32),
                    ) else {
                        continue;
                    };
                    if let Some(symbol) = setup.checker.get_symbol_identity_at_location_public(node)
                    {
                        results[i] = setup
                            .sd
                            .register_symbol(&mut setup.checker, Some(symbol.symbol_handle()));
                    }
                }

                marshal(&results)
            },
        )
    }

    // handleGetSymbolAtLocation returns the symbol at a node location.
    fn handle_get_symbol_at_location(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetSymbolAtLocationParams = decode_params(params)?;
        self.with_checker_for_node_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.location,
            |mut setup, _source_file, node| {
                let Some(node) = node else {
                    return Ok(json::Value::Null);
                };

                let Some(symbol) = setup.checker.get_symbol_identity_at_location_public(node)
                else {
                    return Ok(json::Value::Null);
                };

                marshal(
                    &setup
                        .sd
                        .register_symbol(&mut setup.checker, Some(symbol.symbol_handle())),
                )
            },
        )
    }

    // handleGetSymbolsAtLocations returns symbols at multiple node locations.
    fn handle_get_symbols_at_locations(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetSymbolsAtLocationsParams = decode_params(params)?;
        let mut results = vec![None; params.locations.len()];
        for (i, loc) in params.locations.into_iter().enumerate() {
            results[i] = self.with_checker_for_node_handle(
                ctx,
                &params.snapshot,
                params.project.clone(),
                loc,
                |mut setup, _source_file, node| {
                    let Some(node) = node else {
                        return Ok(None);
                    };
                    let symbol = setup.checker.get_symbol_identity_at_location_public(node);
                    Ok(setup.sd.register_symbol(
                        &mut setup.checker,
                        symbol.map(ast::SymbolIdentity::symbol_handle),
                    ))
                },
            )?;
        }
        marshal(&results)
    }

    // handleGetTypeOfSymbol returns the type of a symbol.
    fn handle_get_type_of_symbol(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetTypeOfSymbolParams = decode_params(params)?;
        self.with_checker_for_symbol_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.symbol,
            |setup, symbol| {
                let symbol = ast::SymbolIdentity::from_symbol_handle(symbol);
                let Some(t) = setup.checker.get_type_of_symbol_identity_public(symbol) else {
                    return Ok(json::Value::Null);
                };

                marshal(&setup.sd.register_type(&setup.checker, Some(t)))
            },
        )
    }

    // handleGetTypesOfSymbols returns the types of multiple symbols.
    fn handle_get_types_of_symbols(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetTypesOfSymbolsParams = decode_params(params)?;
        let mut results = vec![None; params.symbols.len()];
        for (i, sym_handle) in params.symbols.into_iter().enumerate() {
            results[i] = self.with_checker_for_symbol_handle(
                ctx,
                &params.snapshot,
                params.project.clone(),
                sym_handle,
                |setup, symbol| {
                    let symbol = ast::SymbolIdentity::from_symbol_handle(symbol);
                    let Some(t) = setup.checker.get_type_of_symbol_identity_public(symbol) else {
                        return Ok(None);
                    };
                    Ok(setup.sd.register_type(&setup.checker, Some(t)))
                },
            )?;
        }
        marshal(&results)
    }

    // handleGetDeclaredTypeOfSymbol returns the declared type of a symbol (e.g. the type alias body for type alias symbols).
    fn handle_get_declared_type_of_symbol(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetTypeOfSymbolParams = decode_params(params)?;
        self.with_checker_for_symbol_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.symbol,
            |setup, symbol| {
                let symbol = ast::SymbolIdentity::from_symbol_handle(symbol);
                let Some(t) = setup
                    .checker
                    .get_declared_type_of_symbol_identity_public(symbol)
                else {
                    return Ok(json::Value::Null);
                };

                marshal(&setup.sd.register_type(&setup.checker, Some(t)))
            },
        )
    }

    // handleResolveName resolves a name to a symbol at a given location.
    fn handle_resolve_name(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: ResolveNameParams = decode_params(params)?;
        if !params.location.as_str().is_empty() {
            return self.with_checker_for_node_handle(
                ctx,
                &params.snapshot,
                params.project,
                params.location,
                |mut setup, _source_file, location| {
                    let Some(location) = location else {
                        return Ok(json::Value::Null);
                    };
                    let Some(symbol) = setup.checker.resolve_name_symbol_identity_public(
                        &params.name,
                        location,
                        ast::SymbolFlags::from(params.meaning),
                        params.exclude_globals,
                    ) else {
                        return Ok(json::Value::Null);
                    };

                    marshal(
                        &setup
                            .sd
                            .register_symbol(&mut setup.checker, Some(symbol.symbol_handle())),
                    )
                },
            );
        }

        let (Some(file), Some(position)) = (&params.file, params.position) else {
            return Ok(json::Value::Null);
        };
        let file_name = file.to_file_name();
        self.with_checker_for_file_name(
            ctx,
            &params.snapshot,
            params.project,
            &file_name,
            |mut setup, source_file| {
                let location = astnav::get_touching_property_name(
                    source_file,
                    source_file
                        .get_position_map()
                        .utf16_to_utf8(position as i32),
                );

                let Some(location) = location else {
                    return Ok(json::Value::Null);
                };
                let Some(symbol) = setup.checker.resolve_name_symbol_identity_public(
                    &params.name,
                    location,
                    ast::SymbolFlags::from(params.meaning),
                    params.exclude_globals,
                ) else {
                    return Ok(json::Value::Null);
                };

                marshal(
                    &setup
                        .sd
                        .register_symbol(&mut setup.checker, Some(symbol.symbol_handle())),
                )
            },
        )
    }

    // handleGetParentOfSymbol returns the parent of a symbol.
    fn handle_get_parent_of_symbol(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetParentOfSymbolParams = decode_params(params)?;
        self.with_checker_for_symbol_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.symbol,
            |mut setup, symbol| {
                let symbol = ast::SymbolIdentity::from_symbol_handle(symbol);
                let Some(parent) = setup.checker.symbol_parent_public(symbol) else {
                    return Ok(json::Value::Null);
                };
                marshal(
                    &setup
                        .sd
                        .register_symbol(&mut setup.checker, Some(parent.symbol_handle())),
                )
            },
        )
    }

    // handleGetMembersOfSymbol returns the members of a symbol.
    fn handle_get_members_of_symbol(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetMembersOfSymbolParams = decode_params(params)?;
        self.with_checker_for_symbol_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.symbol,
            |mut setup, symbol| {
                let symbol = ast::SymbolIdentity::from_symbol_handle(symbol);
                let members = setup.checker.symbol_members_snapshot_public(symbol);
                if members.is_empty() {
                    return Ok(json::Value::Null);
                }

                let results = members
                    .into_iter()
                    .map(|member| {
                        setup
                            .sd
                            .register_symbol(&mut setup.checker, Some(member.symbol_handle()))
                    })
                    .collect::<Vec<_>>();
                marshal(&results)
            },
        )
    }

    // handleGetExportsOfSymbol returns the exports of a symbol.
    fn handle_get_exports_of_symbol(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetExportsOfSymbolParams = decode_params(params)?;
        self.with_checker_for_symbol_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.symbol,
            |mut setup, symbol| {
                let symbol = ast::SymbolIdentity::from_symbol_handle(symbol);
                let exports = setup.checker.symbol_export_values_snapshot_public(symbol);
                if exports.is_empty() {
                    return Ok(json::Value::Null);
                }

                let results = exports
                    .into_iter()
                    .map(|exp| {
                        setup
                            .sd
                            .register_symbol(&mut setup.checker, Some(exp.symbol_handle()))
                    })
                    .collect::<Vec<_>>();
                marshal(&results)
            },
        )
    }

    // handleGetExportSymbolOfSymbol returns the export symbol of a symbol.
    fn handle_get_export_symbol_of_symbol(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetExportSymbolOfSymbolParams = decode_params(params)?;
        self.with_checker_for_symbol_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.symbol,
            |mut setup, symbol| {
                let symbol = ast::SymbolIdentity::from_symbol_handle(symbol);
                let export_symbol = setup
                    .checker
                    .symbol_export_symbol_public(symbol)
                    .unwrap_or(symbol);
                marshal(
                    &setup
                        .sd
                        .register_symbol(&mut setup.checker, Some(export_symbol.symbol_handle())),
                )
            },
        )
    }

    // handleGetSymbolOfType returns the symbol associated with a type.
    fn handle_get_symbol_of_type(
        &self,
        _ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetSymbolOfTypeParams = decode_params(params)?;
        let sd = self.get_snapshot_data(&params.snapshot)?;
        let registered = sd.resolve_registered_type(params.r#type)?;
        if registered.symbol.as_str().is_empty() {
            return Ok(json::Value::Null);
        }
        let symbol = sd.resolve_symbol_handle(registered.symbol)?;
        marshal(&Some(symbol.response))
    }

    // handleGetSignaturesOfType returns the call or construct signatures of a type.
    fn handle_get_signatures_of_type(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetSignaturesOfTypeParams = decode_params(params)?;
        self.with_checker_for_type_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.r#type.clone(),
            |mut setup| {
                let t = setup.resolve_type_handle(params.r#type.clone())?;

                let kind = checker::SignatureKind::from(params.kind);
                let sigs = setup.checker.get_signatures_of_type_public(t, kind);
                let mut results = Vec::with_capacity(sigs.len());
                for (index, sig) in sigs.into_iter().enumerate() {
                    results.push(setup.sd.register_signature(
                        &mut setup.checker,
                        Some(sig),
                        SignatureOrigin::Type {
                            type_handle: params.r#type.clone(),
                            kind,
                            index,
                        },
                    ));
                }
                marshal(&results)
            },
        )
    }

    // handleGetResolvedSignature returns the resolved signature of a call-like expression.
    fn handle_get_resolved_signature(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetResolvedSignatureParams = decode_params(params)?;
        let location = params.location.clone();
        self.with_checker_for_node_handle(
            ctx,
            &params.snapshot,
            params.project,
            location,
            |mut setup, _source_file, node| {
                let Some(node) = node else {
                    return Ok(json::Value::Null);
                };
                let sig = setup.checker.get_resolved_signature_public(node);
                marshal(&setup.sd.register_signature(
                    &mut setup.checker,
                    sig,
                    SignatureOrigin::Resolved {
                        location: params.location,
                    },
                ))
            },
        )
    }

    // handleGetTypeAtLocation returns the type at a node location.
    fn handle_get_type_at_location(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetTypeAtLocationParams = decode_params(params)?;
        self.with_checker_for_node_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.location,
            |setup, _source_file, node| {
                let Some(node) = node else {
                    return Ok(json::Value::Null);
                };
                let t = setup.checker.get_type_at_location(node);
                marshal(&setup.sd.register_type(&setup.checker, Some(t)))
            },
        )
    }

    // handleGetTypeAtLocations returns types at multiple node locations.
    fn handle_get_type_at_locations(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetTypeAtLocationsParams = decode_params(params)?;
        let mut results = vec![None; params.locations.len()];
        for (i, loc) in params.locations.into_iter().enumerate() {
            results[i] = self.with_checker_for_node_handle(
                ctx,
                &params.snapshot,
                params.project.clone(),
                loc,
                |setup, _source_file, node| {
                    let Some(node) = node else {
                        return Ok(None);
                    };
                    let t = setup.checker.get_type_at_location(node);
                    Ok(setup.sd.register_type(&setup.checker, Some(t)))
                },
            )?;
        }
        marshal(&results)
    }

    // handleGetTypeAtPosition returns the type at a position in a file.
    fn handle_get_type_at_position(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetTypeAtPositionParams = decode_params(params)?;
        let file_name = params.file.to_file_name();
        self.with_checker_for_file_name(
            ctx,
            &params.snapshot,
            params.project,
            &file_name,
            |setup, source_file| {
                let position_map = source_file.get_position_map();
                let Some(node) = astnav::get_touching_property_name(
                    source_file,
                    position_map.utf16_to_utf8(params.position as i32),
                ) else {
                    return Ok(json::Value::Null);
                };

                let t = setup.checker.get_type_at_location(node);
                marshal(&setup.sd.register_type(&setup.checker, Some(t)))
            },
        )
    }

    // handleGetTypesAtPositions returns types at multiple positions in a file.
    fn handle_get_types_at_positions(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetTypesAtPositionsParams = decode_params(params)?;
        let file_name = params.file.to_file_name();
        self.with_checker_for_file_name(
            ctx,
            &params.snapshot,
            params.project,
            &file_name,
            |setup, source_file| {
                let position_map = source_file.get_position_map();
                let mut results = vec![None; params.positions.len()];
                for (i, pos) in params.positions.iter().enumerate() {
                    let Some(node) = astnav::get_touching_property_name(
                        source_file,
                        position_map.utf16_to_utf8(*pos as i32),
                    ) else {
                        continue;
                    };
                    let t = setup.checker.get_type_at_location(node);
                    results[i] = setup.sd.register_type(&setup.checker, Some(t));
                }
                marshal(&results)
            },
        )
    }

    fn resolve_registered_type_property(
        &self,
        params: GetTypePropertyParams,
        getter: impl FnOnce(&RegisteredType) -> TypeHandle,
    ) -> Result<json::Value, Error> {
        let sd = self.get_snapshot_data(&params.snapshot)?;
        let registered = sd.resolve_registered_type(params.r#type)?;
        let handle = getter(&registered);
        if handle.as_str().is_empty() {
            return Ok(json::Value::Null);
        };
        let result = sd.resolve_registered_type(handle)?;
        marshal(&result.response)
    }

    fn resolve_registered_type_array_property(
        &self,
        params: GetTypePropertyParams,
        getter: impl FnOnce(&RegisteredType) -> &[TypeHandle],
    ) -> Result<json::Value, Error> {
        let sd = self.get_snapshot_data(&params.snapshot)?;
        let registered = sd.resolve_registered_type(params.r#type)?;
        let handles = getter(&registered);
        if handles.is_empty() {
            return Ok(json::Value::Null);
        }

        let results = handles
            .iter()
            .map(|handle| {
                sd.resolve_registered_type(handle.clone())
                    .map(|result| result.response)
            })
            .collect::<Result<Vec<_>, _>>()?;
        marshal(&results)
    }

    fn handle_get_target_of_type(
        &self,
        _ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetTypePropertyParams = decode_params(params)?;
        self.resolve_registered_type_property(params, |t| t.target.clone())
    }

    fn handle_get_types_of_type(
        &self,
        _ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetTypePropertyParams = decode_params(params)?;
        self.resolve_registered_type_array_property(params, |t| &t.types)
    }

    fn handle_get_type_parameters_of_type(
        &self,
        _ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetTypePropertyParams = decode_params(params)?;
        self.resolve_registered_type_array_property(params, |t| &t.type_parameters)
    }

    fn handle_get_outer_type_parameters_of_type(
        &self,
        _ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetTypePropertyParams = decode_params(params)?;
        self.resolve_registered_type_array_property(params, |t| &t.outer_type_parameters)
    }

    fn handle_get_local_type_parameters_of_type(
        &self,
        _ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetTypePropertyParams = decode_params(params)?;
        self.resolve_registered_type_array_property(params, |t| &t.local_type_parameters)
    }

    fn handle_get_object_type_of_type(
        &self,
        _ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetTypePropertyParams = decode_params(params)?;
        self.resolve_registered_type_property(params, |t| t.object_type.clone())
    }

    fn handle_get_index_type_of_type(
        &self,
        _ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetTypePropertyParams = decode_params(params)?;
        self.resolve_registered_type_property(params, |t| t.index_type.clone())
    }

    fn handle_get_check_type_of_type(
        &self,
        _ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetTypePropertyParams = decode_params(params)?;
        self.resolve_registered_type_property(params, |t| t.check_type.clone())
    }

    fn handle_get_extends_type_of_type(
        &self,
        _ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetTypePropertyParams = decode_params(params)?;
        self.resolve_registered_type_property(params, |t| t.extends_type.clone())
    }

    fn handle_get_base_type_of_type(
        &self,
        _ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetTypePropertyParams = decode_params(params)?;
        self.resolve_registered_type_property(params, |t| t.base_type.clone())
    }

    fn handle_get_constraint_of_type(
        &self,
        _ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetTypePropertyParams = decode_params(params)?;
        self.resolve_registered_type_property(params, |t| t.subst_constraint.clone())
    }

    // handleGetContextualType returns the contextual type for a node.
    fn handle_get_contextual_type(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetContextualTypeParams = decode_params(params)?;
        self.with_checker_for_node_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.location,
            |setup, _source_file, node| {
                let Some(node) = node else {
                    return Ok(json::Value::Null);
                };

                let Some(t) = setup
                    .checker
                    .get_contextual_type_public(node, checker::CONTEXT_FLAGS_NONE)
                else {
                    return Ok(json::Value::Null);
                };
                marshal(&setup.sd.register_type(&setup.checker, Some(t)))
            },
        )
    }

    // handleGetBaseTypeOfLiteralType returns the base type of a literal type (e.g. number for 42).
    fn handle_get_base_type_of_literal_type(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetBaseTypeOfLiteralTypeParams = decode_params(params)?;
        self.with_checker_for_type_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.r#type.clone(),
            |setup| {
                let t = setup.resolve_type_handle(params.r#type)?;
                let result = setup.checker.get_base_type_of_literal_type_public(t);
                marshal(&setup.sd.register_type(&setup.checker, Some(result)))
            },
        )
    }

    // handleGetNonNullableType returns the type with null and undefined removed.
    fn handle_get_non_nullable_type(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetNonNullableTypeParams = decode_params(params)?;
        self.with_checker_for_type_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.r#type.clone(),
            |setup| {
                let t = setup.resolve_type_handle(params.r#type)?;
                let result = setup.checker.get_non_nullable_type_public(t);
                marshal(&setup.sd.register_type(&setup.checker, Some(result)))
            },
        )
    }

    // handleGetTypeFromTypeNode returns the type for a type node.
    fn handle_get_type_from_type_node(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetTypeFromTypeNodeParams = decode_params(params)?;
        self.with_checker_for_node_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.location,
            |setup, _source_file, node| {
                let Some(node) = node else {
                    return Ok(json::Value::Null);
                };

                let t = setup.checker.get_type_from_type_node_public(node);
                marshal(&setup.sd.register_type(&setup.checker, Some(t)))
            },
        )
    }

    // handleGetWidenedType returns the widened type.
    fn handle_get_widened_type(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetWidenedTypeParams = decode_params(params)?;
        self.with_checker_for_type_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.r#type.clone(),
            |setup| {
                let t = setup.resolve_type_handle(params.r#type)?;
                let result = setup.checker.get_widened_type_public(t);
                marshal(&setup.sd.register_type(&setup.checker, Some(result)))
            },
        )
    }

    // handleGetParameterType returns the type of a parameter at a given index in a signature.
    fn handle_get_parameter_type(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetParameterTypeParams = decode_params(params)?;
        self.with_checker_for_signature_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.signature.clone(),
            |mut setup| {
                let sig = self.resolve_signature_handle_in_setup(&mut setup, params.signature)?;

                if params.index < 0 {
                    return Err(Error::new(format!(
                        "{ERR_CLIENT_ERROR}: invalid parameter index"
                    )));
                }

                let t = setup
                    .checker
                    .get_type_at_position(sig, params.index as usize);
                marshal(&setup.sd.register_type(&setup.checker, Some(t)))
            },
        )
    }

    // handleIsArrayLikeType returns whether a type is array-like.
    fn handle_is_array_like_type(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: IsArrayLikeTypeParams = decode_params(params)?;
        self.with_checker_for_type_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.r#type.clone(),
            |setup| {
                let t = setup.resolve_type_handle(params.r#type)?;
                marshal(&setup.checker.is_array_like_type_public(t))
            },
        )
    }

    // handleGetShorthandAssignmentValueSymbol returns the value symbol of a shorthand property assignment.
    fn handle_get_shorthand_assignment_value_symbol(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetTypeAtLocationParams = decode_params(params)?;
        self.with_checker_for_node_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.location,
            |mut setup, _source_file, node| {
                let Some(node) = node else {
                    return Ok(json::Value::Null);
                };

                let Some(symbol) = setup
                    .checker
                    .get_shorthand_assignment_value_symbol_identity_public(Some(node))
                else {
                    return Ok(json::Value::Null);
                };
                marshal(
                    &setup
                        .sd
                        .register_symbol(&mut setup.checker, Some(symbol.symbol_handle())),
                )
            },
        )
    }

    // handleGetTypeOfSymbolAtLocation returns the narrowed type of a symbol at a specific location.
    fn handle_get_type_of_symbol_at_location(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetTypeOfSymbolAtLocationParams = decode_params(params)?;
        self.with_checker_for_node_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.location,
            |setup, _source_file, node| {
                let symbol = setup.sd.resolve_symbol_handle(params.symbol)?.symbol;
                let symbol = ast::SymbolIdentity::from_symbol_handle(symbol);
                let Some(node) = node else {
                    return Ok(json::Value::Null);
                };

                let Some(t) = setup
                    .checker
                    .get_type_of_symbol_identity_at_location_public(symbol, Some(node))
                else {
                    return Ok(json::Value::Null);
                };
                marshal(&setup.sd.register_type(&setup.checker, Some(t)))
            },
        )
    }
}

impl Session {
    // handleTypeToTypeNode converts a Type to a TypeNode AST and returns it as binary-encoded data.
    fn handle_type_to_type_node(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: TypeToTypeNodeParams = decode_params(params)?;
        self.with_checker_for_type_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.r#type.clone(),
            |setup| {
                let t = setup.resolve_type_handle(params.r#type)?;

                let enclosing_declaration = if !params.location.as_str().is_empty() {
                    self.resolve_node_handle(setup.program, params.location)?
                } else {
                    None
                };

                let (mut emit_context, done) = printer::get_emit_context();
                let (type_node, _) = setup.checker.type_to_type_node_for_ls_public(
                    &mut emit_context,
                    t,
                    enclosing_declaration,
                    params.flags as u32,
                    nodebuilder::INTERNAL_FLAGS_NONE,
                );
                done(emit_context);
                let Some(type_node) = type_node else {
                    return Ok(json::Value::Null);
                };

                let data = encoder::encode_node(
                    type_node,
                    setup.checker.store_for_output_node(type_node),
                    None,
                )
                .map_err(|err| Error::new(format!("failed to encode type node: {err}")))?;
                marshal_binary_or_source(data, self.use_binary_responses)
            },
        )
    }

    fn handle_signature_to_signature_declaration(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: SignatureToSignatureDeclarationParams = decode_params(params)?;
        self.with_checker_for_signature_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.signature.clone(),
            |mut setup| {
                let sig = self.resolve_signature_handle_in_setup(&mut setup, params.signature)?;

                let enclosing_declaration = if !params.location.as_str().is_empty() {
                    self.resolve_node_handle(setup.program, params.location)?
                } else {
                    None
                };

                let Some(node) = setup.checker.signature_to_signature_declaration(
                    sig,
                    crate::proto::kind_from_i16(params.kind as i16),
                    enclosing_declaration,
                    params.flags as u32,
                ) else {
                    return Ok(json::Value::Null);
                };

                let data =
                    encoder::encode_node(node, setup.checker.store_for_output_node(node), None)
                        .map_err(|err| {
                            Error::new(format!("failed to encode signature declaration: {err}"))
                        })?;
                marshal_binary_or_source(data, self.use_binary_responses)
            },
        )
    }

    // handleTypeToString converts a Type to its string representation.
    fn handle_type_to_string(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: TypeToTypeNodeParams = decode_params(params)?;
        self.with_checker_for_type_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.r#type.clone(),
            |setup| {
                let t = setup.resolve_type_handle(params.r#type)?;

                let enclosing_declaration = if !params.location.as_str().is_empty() {
                    self.resolve_node_handle(setup.program, params.location)?
                } else {
                    None
                };

                let text = if params.flags != 0 {
                    setup.checker.type_to_string_ex(
                        t,
                        enclosing_declaration,
                        params.flags as u32,
                        None,
                    )
                } else {
                    setup.checker.type_to_string_ex(
                        t,
                        enclosing_declaration,
                        checker::TYPE_FORMAT_FLAGS_ALLOW_UNIQUE_ES_SYMBOL_TYPE
                            | checker::TYPE_FORMAT_FLAGS_USE_ALIAS_DEFINED_OUTSIDE_CURRENT_SCOPE,
                        None,
                    )
                };
                marshal(&text)
            },
        )
    }

    // handlePrintNode decodes a binary-encoded AST node and prints it to text.
    fn handle_print_node(
        &self,
        _ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: PrintNodeParams = decode_params(params)?;
        let data = BASE64_STANDARD
            .decode(params.data)
            .map_err(|err| Error::new(format!("{ERR_CLIENT_ERROR}: invalid base64 data: {err}")))?;

        let node = encoder::decode_nodes(data).map_err(|err| {
            Error::new(format!("{ERR_CLIENT_ERROR}: failed to decode AST: {err}"))
        })?;

        let emit_context = printer::new_emit_context();
        let mut p = printer::new_printer(
            printer::PrinterOptions {
                preserve_source_newlines: params.preserve_source_newlines,
                never_ascii_escape: params.never_ascii_escape,
                terminate_unterminated_literals: params.terminate_unterminated_literals,
                ..Default::default()
            },
            printer::PrintHandlers::default(),
            Some(emit_context),
        );
        marshal(&p.emit(&node, None))
    }

    // handleGetIntrinsicType returns an intrinsic type (any, string, number, etc.).
    fn handle_get_intrinsic_type(
        &self,
        ctx: &context::Context,
        method: &str,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetIntrinsicTypeParams = decode_params(params)?;
        let file_name = params.file.to_file_name();
        self.with_checker_for_file_name(
            ctx,
            &params.snapshot,
            params.project,
            &file_name,
            |setup, _source_file| {
                let t = match method {
                    METHOD_GET_ANY_TYPE => setup.checker.get_any_type(),
                    METHOD_GET_STRING_TYPE => setup.checker.get_string_type(),
                    METHOD_GET_NUMBER_TYPE => setup.checker.get_number_type(),
                    METHOD_GET_BOOLEAN_TYPE => setup.checker.get_boolean_type(),
                    METHOD_GET_VOID_TYPE => setup.checker.get_void_type(),
                    METHOD_GET_UNDEFINED_TYPE => setup.checker.get_undefined_type(),
                    METHOD_GET_NULL_TYPE => setup.checker.get_null_type(),
                    METHOD_GET_NEVER_TYPE => setup.checker.get_never_type(),
                    METHOD_GET_UNKNOWN_TYPE => setup.checker.get_unknown_type(),
                    METHOD_GET_BIG_INT_TYPE => setup.checker.get_big_int_type(),
                    METHOD_GET_ES_SYMBOL_TYPE => setup.checker.get_es_symbol_type(),
                    _ => return Ok(json::Value::Null),
                };
                marshal(&setup.sd.register_type(&setup.checker, Some(t)))
            },
        )
    }

    // handleIsContextSensitive returns whether a node is context-sensitive.
    fn handle_is_context_sensitive(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetContextualTypeParams = decode_params(params)?;
        self.with_checker_for_node_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.location,
            |setup, _source_file, node| {
                let Some(node) = node else {
                    return marshal(&false);
                };
                marshal(&setup.checker.is_context_sensitive_public(node))
            },
        )
    }

    // handleGetReturnTypeOfSignature returns the return type of a signature.
    fn handle_get_return_type_of_signature(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: CheckerSignatureParams = decode_params(params)?;
        self.with_checker_for_signature_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.signature.clone(),
            |mut setup| {
                let sig = self.resolve_signature_handle_in_setup(&mut setup, params.signature)?;
                let t = setup.checker.get_return_type_of_signature_public(sig);
                marshal(&setup.sd.register_type(&setup.checker, Some(t)))
            },
        )
    }

    // handleGetRestTypeOfSignature returns the rest type of a signature.
    fn handle_get_rest_type_of_signature(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: CheckerSignatureParams = decode_params(params)?;
        self.with_checker_for_signature_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.signature.clone(),
            |mut setup| {
                let sig = self.resolve_signature_handle_in_setup(&mut setup, params.signature)?;
                let t = setup.checker.get_rest_type_of_signature_public(sig);
                marshal(&setup.sd.register_type(&setup.checker, Some(t)))
            },
        )
    }

    // handleGetTypePredicateOfSignature returns the type predicate of a signature.
    fn handle_get_type_predicate_of_signature(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: CheckerSignatureParams = decode_params(params)?;
        self.with_checker_for_signature_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.signature.clone(),
            |mut setup| {
                let sig = self.resolve_signature_handle_in_setup(&mut setup, params.signature)?;
                let Some(pred) = setup.checker.get_type_predicate_of_signature_public(sig) else {
                    return Ok(json::Value::Null);
                };

                let mut resp = TypePredicateResponse {
                    kind: setup.checker.type_predicate_kind_public(pred),
                    parameter_index: setup.checker.type_predicate_parameter_index_public(pred),
                    parameter_name: setup.checker.type_predicate_parameter_name_public(pred),
                    ..Default::default()
                };
                if let Some(t) = setup.checker.type_predicate_type_public(pred) {
                    resp.r#type = setup.sd.register_type(&setup.checker, Some(t));
                }
                marshal(&resp)
            },
        )
    }

    // handleGetBaseTypes returns the base types of an interface/class type.
    fn handle_get_base_types(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: CheckerTypeParams = decode_params(params)?;
        self.with_checker_for_type_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.r#type.clone(),
            |setup| {
                let t = setup.resolve_type_handle(params.r#type)?;
                let base_types = setup.checker.get_base_types_public(t);
                if base_types.is_empty() {
                    return Ok(json::Value::Null);
                }
                let results = base_types
                    .into_iter()
                    .map(|bt| setup.sd.register_type(&setup.checker, Some(bt)))
                    .collect::<Vec<_>>();
                marshal(&results)
            },
        )
    }

    // handleGetPropertiesOfType returns the properties of a type.
    fn handle_get_properties_of_type(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: CheckerTypeParams = decode_params(params)?;
        self.with_checker_for_type_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.r#type.clone(),
            |mut setup| {
                let t = setup.resolve_type_handle(params.r#type)?;
                let props = setup
                    .checker
                    .get_property_symbol_identities_of_type_public(t);
                if props.is_empty() {
                    return Ok(json::Value::Null);
                }
                let results = props
                    .into_iter()
                    .map(|prop| {
                        setup
                            .sd
                            .register_symbol(&mut setup.checker, Some(prop.symbol_handle()))
                    })
                    .collect::<Vec<_>>();
                marshal(&results)
            },
        )
    }

    // handleGetIndexInfosOfType returns the index infos of a type.
    fn handle_get_index_infos_of_type(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: CheckerTypeParams = decode_params(params)?;
        self.with_checker_for_type_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.r#type.clone(),
            |setup| {
                let t = setup.resolve_type_handle(params.r#type)?;
                let infos = setup.checker.get_index_infos_of_type_public(t);
                if infos.is_empty() {
                    return Ok(json::Value::Null);
                }
                let results = infos
                    .into_iter()
                    .map(|info| IndexInfoResponse {
                        key_type: setup
                            .sd
                            .register_type(
                                &setup.checker,
                                setup.checker.index_info_key_type_public(info),
                            )
                            .unwrap_or_default(),
                        value_type: setup
                            .sd
                            .register_type(
                                &setup.checker,
                                setup.checker.index_info_value_type_public(info),
                            )
                            .unwrap_or_default(),
                        is_readonly: setup.checker.index_info_is_readonly_public(info),
                    })
                    .collect::<Vec<_>>();
                marshal(&results)
            },
        )
    }

    // handleGetConstraintOfTypeParameter returns the constraint of a type parameter.
    fn handle_get_constraint_of_type_parameter(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: CheckerTypeParams = decode_params(params)?;
        self.with_checker_for_type_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.r#type.clone(),
            |setup| {
                let t = setup.resolve_type_handle(params.r#type)?;
                let Some(constraint) = setup.checker.get_constraint_of_type_parameter_public(t)
                else {
                    return Ok(json::Value::Null);
                };
                marshal(&setup.sd.register_type(&setup.checker, Some(constraint)))
            },
        )
    }

    // handleGetTypeArguments returns the type arguments of a type reference.
    fn handle_get_type_arguments(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: CheckerTypeParams = decode_params(params)?;
        self.with_checker_for_type_handle(
            ctx,
            &params.snapshot,
            params.project,
            params.r#type.clone(),
            |setup| {
                let t = setup.resolve_type_handle(params.r#type)?;
                let type_args = setup.checker.get_type_arguments_public(t);
                if type_args.is_empty() {
                    return Ok(json::Value::Null);
                }
                let results = type_args
                    .into_iter()
                    .map(|ta| setup.sd.register_type(&setup.checker, Some(ta)))
                    .collect::<Vec<_>>();
                marshal(&results)
            },
        )
    }

    fn resolve_node_handle(
        &self,
        program: &compiler::Program,
        handle: NodeHandle,
    ) -> Result<Option<ast::Node>, Error> {
        let (pos, end, kind, path) = parse_node_handle(&handle)
            .map_err(|err| Error::new(format!("{ERR_CLIENT_ERROR}: {err}")))?;

        // Find the source file by path
        let Some(source_file) = program.get_source_file_by_path(path.clone()) else {
            return Err(Error::new(format!(
                "{ERR_CLIENT_ERROR}: source file not found: {path}"
            )));
        };

        // If the handle refers to the source file itself, return it directly
        if kind == ast::Kind::SourceFile {
            return Ok(Some(source_file.as_node()));
        }

        // Find the node at the position with the expected kind and end
        let store = source_file.store();
        let node = ast::get_node_at_position(store, source_file.as_node(), pos);

        // Verify the kind and end match
        let loc = store.loc(node);
        if store.kind(node) != kind || loc.end() != end {
            // Try to find the exact node by walking children
            let mut found = None;
            let _ = store.for_each_present_child(node, |child| {
                let child_loc = store.loc(child);
                if child_loc.pos() == pos && child_loc.end() == end && store.kind(child) == kind {
                    found = Some(child);
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            });
            if found.is_some() {
                return Ok(found);
            }
            // Return the node we found even if it doesn't match exactly
        }

        Ok(Some(node))
    }

    fn source_file_for_node_handle<'program>(
        &self,
        program: &'program compiler::Program,
        handle: &NodeHandle,
    ) -> Result<&'program ast::SourceFile, Error> {
        let (_, _, _, path) = parse_node_handle(handle)
            .map_err(|err| Error::new(format!("{ERR_CLIENT_ERROR}: {err}")))?;
        program
            .get_source_file_by_path_ref(&path)
            .ok_or_else(|| Error::new(format!("{ERR_CLIENT_ERROR}: source file not found: {path}")))
    }

    fn source_file_for_program_node<'program>(
        &self,
        program: &'program compiler::Program,
        node: ast::Node,
    ) -> Result<&'program ast::SourceFile, Error> {
        program
            .get_parsed_source_files_refs()
            .into_iter()
            .find(|source_file| source_file.store().store_id() == node.store_id())
            .ok_or_else(|| {
                Error::new(format!(
                    "{ERR_CLIENT_ERROR}: node source file not found in program"
                ))
            })
    }

    fn checker_declaration_for_symbol(
        &self,
        program: &compiler::Program,
        handle: SymbolHandle,
        symbol: &SymbolResponse,
    ) -> Result<ast::Node, Error> {
        if !symbol.value_declaration.as_str().is_empty() {
            if let Some(node) =
                self.resolve_node_handle(program, symbol.value_declaration.clone())?
            {
                return Ok(node);
            }
        }
        for declaration in &symbol.declarations {
            if let Some(node) = self.resolve_node_handle(program, declaration.clone())? {
                return Ok(node);
            }
        }
        Err(Error::new(format!(
            "{}: symbol handle {:?} has no declaration for checker acquisition",
            ERR_CLIENT_ERROR, handle
        )))
    }

    pub fn close(&self) {
        let mut snapshots = self
            .snapshots
            .write()
            .unwrap_or_else(|err| err.into_inner());
        snapshots.clear();
    }

    // toAbsoluteFileName converts a file name to an absolute path.
    fn to_absolute_file_name(&self, file_name: &str) -> String {
        tspath::get_normalized_absolute_path(
            file_name,
            &self.project_session().get_current_directory(),
        )
    }

    // toPath converts a file name to a normalized path.
    fn to_path(&self, file_name: &str) -> tspath::Path {
        let project_session = self.project_session();
        tspath::to_path(
            file_name,
            &project_session.get_current_directory(),
            project_session.fs().use_case_sensitive_file_names(),
        )
    }

    // toFileChangeSummary converts API file changes to a project.FileChangeSummary.
    fn to_file_change_summary(
        &self,
        changes: Option<&APIFileChanges>,
    ) -> project::FileChangeSummary {
        let Some(changes) = changes else {
            return project::FileChangeSummary::default();
        };
        let mut summary = project::FileChangeSummary::default();
        if changes.invalidate_all {
            summary.invalidate_all = true;
            summary.includes_watch_change_outside_node_modules = true;
            return summary;
        }
        for doc in &changes.changed {
            let uri = doc.to_uri();
            summary.changed.insert(uri);
        }
        for doc in &changes.created {
            let uri = doc.to_uri();
            summary.created.insert(uri);
        }
        for doc in &changes.deleted {
            let uri = doc.to_uri();
            summary.deleted.insert(uri);
        }
        if summary.changed.len() + summary.created.len() + summary.deleted.len() > 0 {
            summary.includes_watch_change_outside_node_modules = true;
        }
        summary
    }

    // handleGetSyntacticDiagnostics returns syntactic diagnostics for a file or all files.
    fn handle_get_syntactic_diagnostics(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetDiagnosticsParams = decode_params(params)?;
        let sd = self.get_snapshot_data(&params.snapshot)?;
        let project_session = self.project_session();
        let program = sd.get_program(&project_session, params.project)?;
        let source_file = self.resolve_optional_source_file(&program, params.file)?;
        let diags = program.get_syntactic_diagnostics(ctx.clone(), source_file);
        marshal(&new_diagnostic_responses(&diags))
    }

    // handleGetSemanticDiagnostics returns semantic diagnostics for a file or all files.
    fn handle_get_semantic_diagnostics(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetDiagnosticsParams = decode_params(params)?;
        let sd = self.get_snapshot_data(&params.snapshot)?;
        let project_session = self.project_session();
        let program = sd.get_program(&project_session, params.project)?;
        let source_file = self.resolve_optional_source_file(&program, params.file)?;
        let diags = program.get_semantic_diagnostics(ctx.clone(), source_file);
        marshal(&new_diagnostic_responses(&diags))
    }

    // handleGetSuggestionDiagnostics returns suggestion diagnostics for a file or all files.
    fn handle_get_suggestion_diagnostics(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetDiagnosticsParams = decode_params(params)?;
        let sd = self.get_snapshot_data(&params.snapshot)?;
        let project_session = self.project_session();
        let program = sd.get_program(&project_session, params.project)?;
        let source_file = self.resolve_optional_source_file(&program, params.file)?;
        let diags = program.get_suggestion_diagnostics(ctx.clone(), source_file);
        marshal(&new_diagnostic_responses(&diags))
    }

    // handleGetDeclarationDiagnostics returns declaration diagnostics for a file or all files.
    fn handle_get_declaration_diagnostics(
        &self,
        ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetDiagnosticsParams = decode_params(params)?;
        let sd = self.get_snapshot_data(&params.snapshot)?;
        let project_session = self.project_session();
        let program = sd.get_program(&project_session, params.project)?;
        let source_file = self.resolve_optional_source_file(&program, params.file)?;
        let diags = program.get_declaration_diagnostics(ctx.clone(), source_file);
        marshal(&new_diagnostic_responses(&diags))
    }

    // handleGetConfigFileParsingDiagnostics returns config file parsing diagnostics.
    fn handle_get_config_file_parsing_diagnostics(
        &self,
        _ctx: &context::Context,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        let params: GetProjectDiagnosticsParams = decode_params(params)?;
        let sd = self.get_snapshot_data(&params.snapshot)?;
        let project_session = self.project_session();
        let program = sd.get_program(&project_session, params.project)?;
        let diags = program.get_config_file_parsing_diagnostics();
        marshal(&new_diagnostic_responses(&diags))
    }

    // resolveOptionalSourceFile resolves an optional DocumentIdentifier to a source file.
    // Returns nil if the identifier is nil (meaning all files).
    fn resolve_optional_source_file<'a>(
        &self,
        program: &'a compiler::Program,
        file: Option<DocumentIdentifier>,
    ) -> Result<Option<&'a ast::SourceFile>, Error> {
        let Some(file) = file else {
            return Ok(None);
        };
        let Some(source_file) = program.get_source_file_ref(&file.to_file_name()) else {
            return Err(Error::new(format!(
                "{ERR_CLIENT_ERROR}: source file not found: {file:?}"
            )));
        };
        Ok(Some(source_file))
    }
}

fn snapshot_handle(snapshot: &project::SnapshotHandle) -> SnapshotHandle {
    SnapshotHandle::new(format!(
        "{}{:016x}",
        HANDLE_PREFIX_SNAPSHOT_FOR_SESSION,
        snapshot.id()
    ))
}

const HANDLE_PREFIX_SNAPSHOT_FOR_SESSION: char = 'n';

fn format_session_id(id: u64) -> String {
    format!("api-session-{id}")
}

// computeSnapshotChanges computes the per-project source file differences between
// two snapshots. It uses DiffOrderedMaps on projects to find changed/removed projects,
// then DiffMaps on FilesByPath for each changed project to collect file-level changes.
fn compute_snapshot_changes(
    project_session: &project::Session,
    prev: &project::SnapshotHandle,
    next: &project::SnapshotHandle,
) -> SnapshotChanges {
    let prev_projects = project_session.snapshot_project_infos_by_path(prev);
    let next_projects = project_session.snapshot_project_infos_by_path(next);

    let mut changes = SnapshotChanges::default();

    for (path, old_proj) in prev_projects.entries() {
        if !next_projects.has(path) {
            changes.removed_projects.push(project_handle(&old_proj.id));
        }
    }
    for (path, new_proj) in next_projects.entries() {
        let changed = match prev_projects.get(path) {
            Some(old_proj) => old_proj.program_id != new_proj.program_id,
            None => true,
        };
        if changed {
            changes
                .changed_projects
                .insert(project_handle(&new_proj.id), ProjectFileChanges::default());
        }
    }

    changes
}
