use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    str::FromStr,
    sync::LazyLock,
};

use serde::{Deserialize, Deserializer, Serialize, de};
use ts_ast as ast;
use ts_checker as checker;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_json as json;
use ts_locale as locale;
use ts_ls as lsconv;
use ts_lsproto as lsproto;
use ts_project as project;
use ts_tspath as tspath;

pub const ERR_INVALID_REQUEST: &str = "api: invalid request";
pub const ERR_CLIENT_ERROR: &str = "api: client error";

pub type Method = &'static str;

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct ApiHandle {
    value: String,
}

impl ApiHandle {
    fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    fn as_str(&self) -> &str {
        &self.value
    }

    fn is_empty(&self) -> bool {
        self.value.is_empty()
    }
}

impl Clone for ApiHandle {
    fn clone(&self) -> Self {
        Self::new(self.value.clone())
    }
}

impl PartialEq for ApiHandle {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl Eq for ApiHandle {}

impl Hash for ApiHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl std::fmt::Debug for ApiHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
    }
}

impl Default for ApiHandle {
    fn default() -> Self {
        Self::new(String::new())
    }
}

impl std::fmt::Display for ApiHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.value)
    }
}

macro_rules! api_handle {
    ($name:ident) => {
        #[derive(Clone, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(ApiHandle);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(ApiHandle::new(value))
            }

            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }

            pub fn is_empty(handle: &Self) -> bool {
                handle.0.is_empty()
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

api_handle!(ProjectHandle);
api_handle!(SnapshotHandle);
api_handle!(SymbolHandle);
api_handle!(NodeHandle);
api_handle!(TypeHandle);
api_handle!(SignatureHandle);

const HANDLE_PREFIX_PROJECT: char = 'p';
const HANDLE_PREFIX_SYMBOL: char = 's';
const HANDLE_PREFIX_TYPE: char = 't';
const HANDLE_PREFIX_SIGNATURE: char = 'g';

pub(crate) fn project_handle(id: &tspath::Path) -> ProjectHandle {
    ProjectHandle::new(format!("{}.{id}", HANDLE_PREFIX_PROJECT))
}

pub(crate) fn symbol_handle(
    ch: &checker::Checker<'_, '_>,
    symbol: ast::SymbolHandle,
) -> SymbolHandle {
    let domain = match symbol.domain() {
        ast::SymbolDomain::Program => 'p',
        ast::SymbolDomain::CheckerTransient => 't',
    };
    let identity = ast::SymbolIdentity::from_symbol_handle(symbol);
    let mut hasher = DefaultHasher::new();
    ch.symbol_id_public(identity).write_stable_hash(&mut hasher);
    SymbolHandle::new(format!(
        "{}{}{:016x}",
        HANDLE_PREFIX_SYMBOL,
        domain,
        hasher.finish(),
    ))
}

pub(crate) fn type_handle(ch: &checker::Checker<'_, '_>, t: checker::TypeHandle) -> TypeHandle {
    let identity = ch.state_identity();
    TypeHandle::new(format!(
        "{}{:08x}.{:08x}.{:016x}",
        HANDLE_PREFIX_TYPE,
        identity.slot().get(),
        identity.generation().get(),
        ch.type_id(t)
    ))
}

pub(crate) fn signature_handle(ch: &checker::Checker<'_, '_>, id: u64) -> SignatureHandle {
    let identity = ch.state_identity();
    SignatureHandle::new(format!(
        "{}{:08x}.{:08x}.{:016x}",
        HANDLE_PREFIX_SIGNATURE,
        identity.slot().get(),
        identity.generation().get(),
        id
    ))
}

// NodeHandleFrom creates a node handle from a node.
// Format: pos.end.kind.path
pub(crate) fn node_handle_from(store: &ast::AstStore, node: ast::Node) -> NodeHandle {
    let source_file = ast::get_source_file_of_node(store, Some(node))
        .expect("node handle requires node to belong to a source file");
    let loc = store.loc(node);
    NodeHandle::new(format!(
        "{}.{}.{}.{}",
        loc.pos(),
        loc.end(),
        store.kind(node),
        store.as_source_file(source_file).path()
    ))
}

// parseNodeHandle parses a node handle into its components.
// Format: pos.end.kind.path
pub(crate) fn parse_node_handle(
    handle: &NodeHandle,
) -> Result<(i32, i32, ast::Kind, tspath::Path), String> {
    let parts = handle.as_str().splitn(4, '.').collect::<Vec<_>>();
    if parts.len() != 4 {
        return Err(format!("invalid node handle {handle:?}"));
    }

    let pos_int =
        i32::from_str(parts[0]).map_err(|err| format!("invalid node handle {handle:?}: {err}"))?;
    let end_int =
        i32::from_str(parts[1]).map_err(|err| format!("invalid node handle {handle:?}: {err}"))?;
    let kind_int =
        i16::from_str(parts[2]).map_err(|err| format!("invalid node handle {handle:?}: {err}"))?;
    Ok((
        pos_int,
        end_int,
        kind_from_i16(kind_int),
        tspath::Path::from(parts[3]),
    ))
}

pub(crate) fn kind_from_i16(value: i16) -> ast::Kind {
    if value < 0 || value >= ast::Kind::Count as i16 {
        return ast::Kind::Unknown;
    }
    let mut kind = ast::Kind::Unknown;
    for _ in 0..value {
        kind = kind.next();
    }
    kind
}

pub(crate) fn parse_project_handle(handle: &ProjectHandle) -> tspath::Path {
    tspath::Path::from(&handle.as_str()[2..])
}

pub const METHOD_RELEASE: Method = "release";

pub const METHOD_INITIALIZE: Method = "initialize";
pub const METHOD_UPDATE_SNAPSHOT: Method = "updateSnapshot";
pub const METHOD_PARSE_CONFIG_FILE: Method = "parseConfigFile";
pub const METHOD_GET_DEFAULT_PROJECT_FOR_FILE: Method = "getDefaultProjectForFile";
pub const METHOD_GET_SYMBOL_AT_POSITION: Method = "getSymbolAtPosition";
pub const METHOD_GET_SYMBOLS_AT_POSITIONS: Method = "getSymbolsAtPositions";
pub const METHOD_GET_SYMBOL_AT_LOCATION: Method = "getSymbolAtLocation";
pub const METHOD_GET_SYMBOLS_AT_LOCATIONS: Method = "getSymbolsAtLocations";
pub const METHOD_GET_TYPE_OF_SYMBOL: Method = "getTypeOfSymbol";
pub const METHOD_GET_TYPES_OF_SYMBOLS: Method = "getTypesOfSymbols";
pub const METHOD_GET_DECLARED_TYPE_OF_SYMBOL: Method = "getDeclaredTypeOfSymbol";
pub const METHOD_GET_SOURCE_FILE: Method = "getSourceFile";
pub const METHOD_RESOLVE_NAME: Method = "resolveName";
pub const METHOD_GET_PARENT_OF_SYMBOL: Method = "getParentOfSymbol";
pub const METHOD_GET_MEMBERS_OF_SYMBOL: Method = "getMembersOfSymbol";
pub const METHOD_GET_EXPORTS_OF_SYMBOL: Method = "getExportsOfSymbol";
pub const METHOD_GET_EXPORT_SYMBOL_OF_SYMBOL: Method = "getExportSymbolOfSymbol";
pub const METHOD_GET_SYMBOL_OF_TYPE: Method = "getSymbolOfType";
pub const METHOD_GET_SIGNATURES_OF_TYPE: Method = "getSignaturesOfType";
pub const METHOD_GET_RESOLVED_SIGNATURE: Method = "getResolvedSignature";
pub const METHOD_GET_TYPE_AT_LOCATION: Method = "getTypeAtLocation";
pub const METHOD_GET_TYPE_AT_LOCATIONS: Method = "getTypeAtLocations";
pub const METHOD_GET_TYPE_AT_POSITION: Method = "getTypeAtPosition";
pub const METHOD_GET_TYPES_AT_POSITIONS: Method = "getTypesAtPositions";

// Type sub-property methods
pub const METHOD_GET_TARGET_OF_TYPE: Method = "getTargetOfType";
pub const METHOD_GET_TYPES_OF_TYPE: Method = "getTypesOfType";
pub const METHOD_GET_TYPE_PARAMETERS_OF_TYPE: Method = "getTypeParametersOfType";
pub const METHOD_GET_OUTER_TYPE_PARAMETERS_OF_TYPE: Method = "getOuterTypeParametersOfType";
pub const METHOD_GET_LOCAL_TYPE_PARAMETERS_OF_TYPE: Method = "getLocalTypeParametersOfType";
pub const METHOD_GET_OBJECT_TYPE_OF_TYPE: Method = "getObjectTypeOfType";
pub const METHOD_GET_INDEX_TYPE_OF_TYPE: Method = "getIndexTypeOfType";
pub const METHOD_GET_CHECK_TYPE_OF_TYPE: Method = "getCheckTypeOfType";
pub const METHOD_GET_EXTENDS_TYPE_OF_TYPE: Method = "getExtendsTypeOfType";
pub const METHOD_GET_BASE_TYPE_OF_TYPE: Method = "getBaseTypeOfType";
pub const METHOD_GET_CONSTRAINT_OF_TYPE: Method = "getConstraintOfType";

// Checker methods
pub const METHOD_GET_CONTEXTUAL_TYPE: Method = "getContextualType";
pub const METHOD_GET_BASE_TYPE_OF_LITERAL_TYPE: Method = "getBaseTypeOfLiteralType";
pub const METHOD_GET_NON_NULLABLE_TYPE: Method = "getNonNullableType";
pub const METHOD_GET_TYPE_FROM_TYPE_NODE: Method = "getTypeFromTypeNode";
pub const METHOD_GET_WIDENED_TYPE: Method = "getWidenedType";
pub const METHOD_GET_PARAMETER_TYPE: Method = "getParameterType";
pub const METHOD_IS_ARRAY_LIKE_TYPE: Method = "isArrayLikeType";
pub const METHOD_GET_SHORTHAND_ASSIGNMENT_VALUE_SYMBOL: Method =
    "getShorthandAssignmentValueSymbol";
pub const METHOD_GET_TYPE_OF_SYMBOL_AT_LOCATION: Method = "getTypeOfSymbolAtLocation";
pub const METHOD_TYPE_TO_TYPE_NODE: Method = "typeToTypeNode";
pub const METHOD_SIGNATURE_TO_SIGNATURE_DECLARATION: Method = "signatureToSignatureDeclaration";
pub const METHOD_TYPE_TO_STRING: Method = "typeToString";
pub const METHOD_IS_CONTEXT_SENSITIVE: Method = "isContextSensitive";
pub const METHOD_GET_RETURN_TYPE_OF_SIGNATURE: Method = "getReturnTypeOfSignature";
pub const METHOD_GET_REST_TYPE_OF_SIGNATURE: Method = "getRestTypeOfSignature";
pub const METHOD_GET_TYPE_PREDICATE_OF_SIGNATURE: Method = "getTypePredicateOfSignature";
pub const METHOD_GET_BASE_TYPES: Method = "getBaseTypes";
pub const METHOD_GET_PROPERTIES_OF_TYPE: Method = "getPropertiesOfType";
pub const METHOD_GET_INDEX_INFOS_OF_TYPE: Method = "getIndexInfosOfType";
pub const METHOD_GET_CONSTRAINT_OF_TYPE_PARAMETER: Method = "getConstraintOfTypeParameter";
pub const METHOD_GET_TYPE_ARGUMENTS: Method = "getTypeArguments";

// Diagnostic methods
pub const METHOD_GET_SYNTACTIC_DIAGNOSTICS: Method = "getSyntacticDiagnostics";
pub const METHOD_GET_SEMANTIC_DIAGNOSTICS: Method = "getSemanticDiagnostics";
pub const METHOD_GET_SUGGESTION_DIAGNOSTICS: Method = "getSuggestionDiagnostics";
pub const METHOD_GET_DECLARATION_DIAGNOSTICS: Method = "getDeclarationDiagnostics";
pub const METHOD_GET_CONFIG_FILE_PARSING_DIAGNOSTICS: Method = "getConfigFileParsingDiagnostics";

// Emitter methods
pub const METHOD_PRINT_NODE: Method = "printNode";

// Intrinsic type getters
pub const METHOD_GET_ANY_TYPE: Method = "getAnyType";
pub const METHOD_GET_STRING_TYPE: Method = "getStringType";
pub const METHOD_GET_NUMBER_TYPE: Method = "getNumberType";
pub const METHOD_GET_BOOLEAN_TYPE: Method = "getBooleanType";
pub const METHOD_GET_VOID_TYPE: Method = "getVoidType";
pub const METHOD_GET_UNDEFINED_TYPE: Method = "getUndefinedType";
pub const METHOD_GET_NULL_TYPE: Method = "getNullType";
pub const METHOD_GET_NEVER_TYPE: Method = "getNeverType";
pub const METHOD_GET_UNKNOWN_TYPE: Method = "getUnknownType";
pub const METHOD_GET_BIG_INT_TYPE: Method = "getBigIntType";
pub const METHOD_GET_ES_SYMBOL_TYPE: Method = "getESSymbolType";

// InitializeResponse is returned by the initialize method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResponse {
    // UseCaseSensitiveFileNames indicates whether the host file system is case-sensitive.
    #[serde(rename = "useCaseSensitiveFileNames")]
    pub use_case_sensitive_file_names: bool,
    // CurrentDirectory is the server's current working directory.
    #[serde(rename = "currentDirectory")]
    pub current_directory: String,
}

// DocumentIdentifier identifies a document by either a file name (plain string) or a URI object.
// On the wire it is string | { uri: string }.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DocumentIdentifier {
    #[serde(rename = "fileName", skip_serializing_if = "String::is_empty")]
    pub file_name: String,
    #[serde(rename = "uri", skip_serializing_if = "lsproto::DocumentUri::is_empty")]
    pub uri: lsproto::DocumentUri,
}

impl<'de> Deserialize<'de> for DocumentIdentifier {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = json::Value::deserialize(deserializer)?;
        match value {
            json::Value::String(file_name) => Ok(Self {
                file_name,
                uri: Default::default(),
            }),
            json::Value::Object(mut object) => {
                let uri = object
                    .remove("uri")
                    .map(serde_json::from_value)
                    .transpose()
                    .map_err(de::Error::custom)?
                    .unwrap_or_default();
                Ok(Self {
                    file_name: String::new(),
                    uri,
                })
            }
            other => Err(de::Error::custom(format!(
                "DocumentIdentifier: expected string or object, got {}",
                json_value_kind(&other)
            ))),
        }
    }
}

fn json_value_kind(value: &json::Value) -> &'static str {
    match value {
        json::Value::Null => "null",
        json::Value::Bool(_) => "bool",
        json::Value::Number(_) => "number",
        json::Value::String(_) => "string",
        json::Value::Array(_) => "array",
        json::Value::Object(_) => "object",
    }
}

impl DocumentIdentifier {
    fn uri_file_name(uri: &lsproto::DocumentUri) -> String {
        // PORT NOTE: current lsproto::DocumentUri is a String alias; the Go API
        // exposes FileName(), and the Rust lsconv shim currently stores file
        // names directly in the URI value.
        uri.clone()
    }

    pub fn to_file_name(&self) -> String {
        if !self.uri.is_empty() {
            return Self::uri_file_name(&self.uri);
        }
        self.file_name.clone()
    }

    pub fn to_uri(&self) -> lsproto::DocumentUri {
        if !self.uri.is_empty() {
            return self.uri.clone();
        }
        lsconv::file_name_to_document_uri(&self.file_name)
    }

    pub fn to_absolute_file_name(&self, cwd: &str) -> String {
        if !self.uri.is_empty() {
            return Self::uri_file_name(&self.uri);
        }
        tspath::get_normalized_absolute_path(&self.file_name, cwd)
    }
}

impl std::fmt::Display for DocumentIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.uri.is_empty() {
            write!(f, "{}", self.uri)
        } else {
            f.write_str(&self.file_name)
        }
    }
}

// APIFileChangeSummary lists documents that have been changed, created, or deleted.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct APIFileChangeSummary {
    #[serde(rename = "changed", default, skip_serializing_if = "Vec::is_empty")]
    pub changed: Vec<DocumentIdentifier>,
    #[serde(rename = "created", default, skip_serializing_if = "Vec::is_empty")]
    pub created: Vec<DocumentIdentifier>,
    #[serde(rename = "deleted", default, skip_serializing_if = "Vec::is_empty")]
    pub deleted: Vec<DocumentIdentifier>,
}

// APIFileChanges describes file changes to apply when updating a snapshot.
// Either InvalidateAll is true (discard all caches) or Changed/Created/Deleted
// list individual documents.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct APIFileChanges {
    #[serde(rename = "invalidateAll", default, skip_serializing_if = "is_false")]
    pub invalidate_all: bool,
    #[serde(rename = "changed", default, skip_serializing_if = "Vec::is_empty")]
    pub changed: Vec<DocumentIdentifier>,
    #[serde(rename = "created", default, skip_serializing_if = "Vec::is_empty")]
    pub created: Vec<DocumentIdentifier>,
    #[serde(rename = "deleted", default, skip_serializing_if = "Vec::is_empty")]
    pub deleted: Vec<DocumentIdentifier>,
}

// UpdateSnapshotParams are the parameters for creating a new snapshot.
// All fields are optional. With no fields set, the server adopts the latest LSP state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateSnapshotParams {
    // OpenProject is the path to a tsconfig.json file to open/load in the new snapshot.
    #[serde(
        rename = "openProject",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub open_project: String,
    // FileChanges describes file system changes since the last snapshot.
    #[serde(
        rename = "fileChanges",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub file_changes: Option<APIFileChanges>,
}

// ProjectFileChanges describes what source files changed within a single project.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectFileChanges {
    // ChangedFiles lists source file paths whose content differs.
    #[serde(
        rename = "changedFiles",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub changed_files: Vec<tspath::Path>,
    // DeletedFiles lists source file paths removed from the project's program.
    #[serde(
        rename = "deletedFiles",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub deleted_files: Vec<tspath::Path>,
}

// SnapshotChanges describes what changed between the previous latest snapshot
// and the newly created snapshot. Changes are reported per-project so clients
// can track cache refs at the (snapshot, project) level.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SnapshotChanges {
    // ChangedProjects maps project handles to the file changes within that project.
    // Projects not listed here (and not in RemovedProjects) are unchanged.
    #[serde(
        rename = "changedProjects",
        default,
        skip_serializing_if = "HashMap::is_empty"
    )]
    pub changed_projects: HashMap<ProjectHandle, ProjectFileChanges>,
    // RemovedProjects lists project handles that were present in the previous
    // snapshot but absent from the new one.
    #[serde(
        rename = "removedProjects",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub removed_projects: Vec<ProjectHandle>,
}

// UpdateSnapshotResponse is returned by updateSnapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSnapshotResponse {
    // Snapshot is the handle for the newly created snapshot.
    #[serde(rename = "snapshot")]
    pub snapshot: SnapshotHandle,
    // Projects is the list of projects in the snapshot.
    #[serde(rename = "projects")]
    pub projects: Vec<ProjectResponse>,
    // Changes describes source file differences from the previous snapshot.
    // Nil for the first snapshot in a session.
    #[serde(rename = "changes", default, skip_serializing_if = "Option::is_none")]
    pub changes: Option<SnapshotChanges>,
}

type UnmarshalFn = fn(json::Value) -> Result<json::Value, String>;

static UNMARSHALERS: LazyLock<HashMap<Method, UnmarshalFn>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    map.insert(
        METHOD_RELEASE,
        unmarshaller_for::<ReleaseParams> as UnmarshalFn,
    );
    map.insert(METHOD_INITIALIZE, no_params as UnmarshalFn);
    map.insert(
        METHOD_UPDATE_SNAPSHOT,
        unmarshaller_for::<UpdateSnapshotParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_PARSE_CONFIG_FILE,
        unmarshaller_for::<ParseConfigFileParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_DEFAULT_PROJECT_FOR_FILE,
        unmarshaller_for::<GetDefaultProjectForFileParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_SOURCE_FILE,
        unmarshaller_for::<GetSourceFileParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_SYMBOL_AT_POSITION,
        unmarshaller_for::<GetSymbolAtPositionParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_SYMBOLS_AT_POSITIONS,
        unmarshaller_for::<GetSymbolsAtPositionsParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_SYMBOL_AT_LOCATION,
        unmarshaller_for::<GetSymbolAtLocationParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_SYMBOLS_AT_LOCATIONS,
        unmarshaller_for::<GetSymbolsAtLocationsParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_TYPE_OF_SYMBOL,
        unmarshaller_for::<GetTypeOfSymbolParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_TYPES_OF_SYMBOLS,
        unmarshaller_for::<GetTypesOfSymbolsParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_DECLARED_TYPE_OF_SYMBOL,
        unmarshaller_for::<GetTypeOfSymbolParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_RESOLVE_NAME,
        unmarshaller_for::<ResolveNameParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_PARENT_OF_SYMBOL,
        unmarshaller_for::<GetParentOfSymbolParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_MEMBERS_OF_SYMBOL,
        unmarshaller_for::<GetMembersOfSymbolParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_EXPORTS_OF_SYMBOL,
        unmarshaller_for::<GetExportsOfSymbolParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_EXPORT_SYMBOL_OF_SYMBOL,
        unmarshaller_for::<GetExportSymbolOfSymbolParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_SYMBOL_OF_TYPE,
        unmarshaller_for::<GetSymbolOfTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_SIGNATURES_OF_TYPE,
        unmarshaller_for::<GetSignaturesOfTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_RESOLVED_SIGNATURE,
        unmarshaller_for::<GetResolvedSignatureParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_TYPE_AT_LOCATION,
        unmarshaller_for::<GetTypeAtLocationParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_TYPE_AT_LOCATIONS,
        unmarshaller_for::<GetTypeAtLocationsParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_TYPE_AT_POSITION,
        unmarshaller_for::<GetTypeAtPositionParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_TYPES_AT_POSITIONS,
        unmarshaller_for::<GetTypesAtPositionsParams> as UnmarshalFn,
    );

    map.insert(
        METHOD_GET_TARGET_OF_TYPE,
        unmarshaller_for::<GetTypePropertyParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_TYPES_OF_TYPE,
        unmarshaller_for::<GetTypePropertyParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_TYPE_PARAMETERS_OF_TYPE,
        unmarshaller_for::<GetTypePropertyParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_OUTER_TYPE_PARAMETERS_OF_TYPE,
        unmarshaller_for::<GetTypePropertyParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_LOCAL_TYPE_PARAMETERS_OF_TYPE,
        unmarshaller_for::<GetTypePropertyParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_OBJECT_TYPE_OF_TYPE,
        unmarshaller_for::<GetTypePropertyParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_INDEX_TYPE_OF_TYPE,
        unmarshaller_for::<GetTypePropertyParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_CHECK_TYPE_OF_TYPE,
        unmarshaller_for::<GetTypePropertyParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_EXTENDS_TYPE_OF_TYPE,
        unmarshaller_for::<GetTypePropertyParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_BASE_TYPE_OF_TYPE,
        unmarshaller_for::<GetTypePropertyParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_CONSTRAINT_OF_TYPE,
        unmarshaller_for::<GetTypePropertyParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_CONTEXTUAL_TYPE,
        unmarshaller_for::<GetContextualTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_BASE_TYPE_OF_LITERAL_TYPE,
        unmarshaller_for::<GetBaseTypeOfLiteralTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_NON_NULLABLE_TYPE,
        unmarshaller_for::<GetNonNullableTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_TYPE_FROM_TYPE_NODE,
        unmarshaller_for::<GetTypeFromTypeNodeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_WIDENED_TYPE,
        unmarshaller_for::<GetWidenedTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_PARAMETER_TYPE,
        unmarshaller_for::<GetParameterTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_IS_ARRAY_LIKE_TYPE,
        unmarshaller_for::<IsArrayLikeTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_SHORTHAND_ASSIGNMENT_VALUE_SYMBOL,
        unmarshaller_for::<GetTypeAtLocationParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_TYPE_OF_SYMBOL_AT_LOCATION,
        unmarshaller_for::<GetTypeOfSymbolAtLocationParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_TYPE_TO_TYPE_NODE,
        unmarshaller_for::<TypeToTypeNodeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_SIGNATURE_TO_SIGNATURE_DECLARATION,
        unmarshaller_for::<SignatureToSignatureDeclarationParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_TYPE_TO_STRING,
        unmarshaller_for::<TypeToTypeNodeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_IS_CONTEXT_SENSITIVE,
        unmarshaller_for::<GetContextualTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_RETURN_TYPE_OF_SIGNATURE,
        unmarshaller_for::<CheckerSignatureParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_REST_TYPE_OF_SIGNATURE,
        unmarshaller_for::<CheckerSignatureParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_TYPE_PREDICATE_OF_SIGNATURE,
        unmarshaller_for::<CheckerSignatureParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_BASE_TYPES,
        unmarshaller_for::<CheckerTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_PROPERTIES_OF_TYPE,
        unmarshaller_for::<CheckerTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_INDEX_INFOS_OF_TYPE,
        unmarshaller_for::<CheckerTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_CONSTRAINT_OF_TYPE_PARAMETER,
        unmarshaller_for::<CheckerTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_TYPE_ARGUMENTS,
        unmarshaller_for::<CheckerTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_PRINT_NODE,
        unmarshaller_for::<PrintNodeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_ANY_TYPE,
        unmarshaller_for::<GetIntrinsicTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_STRING_TYPE,
        unmarshaller_for::<GetIntrinsicTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_NUMBER_TYPE,
        unmarshaller_for::<GetIntrinsicTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_BOOLEAN_TYPE,
        unmarshaller_for::<GetIntrinsicTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_VOID_TYPE,
        unmarshaller_for::<GetIntrinsicTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_UNDEFINED_TYPE,
        unmarshaller_for::<GetIntrinsicTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_NULL_TYPE,
        unmarshaller_for::<GetIntrinsicTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_NEVER_TYPE,
        unmarshaller_for::<GetIntrinsicTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_UNKNOWN_TYPE,
        unmarshaller_for::<GetIntrinsicTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_BIG_INT_TYPE,
        unmarshaller_for::<GetIntrinsicTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_ES_SYMBOL_TYPE,
        unmarshaller_for::<GetIntrinsicTypeParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_SYNTACTIC_DIAGNOSTICS,
        unmarshaller_for::<GetDiagnosticsParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_SEMANTIC_DIAGNOSTICS,
        unmarshaller_for::<GetDiagnosticsParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_SUGGESTION_DIAGNOSTICS,
        unmarshaller_for::<GetDiagnosticsParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_DECLARATION_DIAGNOSTICS,
        unmarshaller_for::<GetDiagnosticsParams> as UnmarshalFn,
    );
    map.insert(
        METHOD_GET_CONFIG_FILE_PARSING_DIAGNOSTICS,
        unmarshaller_for::<GetProjectDiagnosticsParams> as UnmarshalFn,
    );
    map
});

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseConfigFileParams {
    #[serde(rename = "file")]
    pub file: DocumentIdentifier,
}

// ReleaseParams are the parameters for the release method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseParams {
    #[serde(rename = "handle")]
    pub handle: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFileResponse {
    #[serde(rename = "fileNames")]
    pub file_names: Vec<String>,
    #[serde(rename = "options")]
    pub options: Option<core::CompilerOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetDefaultProjectForFileParams {
    #[serde(rename = "snapshot")]
    pub snapshot: SnapshotHandle,
    #[serde(rename = "file")]
    pub file: DocumentIdentifier,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectResponse {
    #[serde(rename = "id")]
    pub id: ProjectHandle,
    #[serde(rename = "configFileName")]
    pub config_file_name: String,
    #[serde(rename = "rootFiles")]
    pub root_files: Vec<String>,
    #[serde(rename = "compilerOptions")]
    pub compiler_options: Option<core::CompilerOptions>,
}

pub(crate) fn new_project_response(p: &project::ProjectInfo) -> ProjectResponse {
    ProjectResponse {
        id: project_handle(&p.id),
        config_file_name: p.name.clone(),
        root_files: p.root_files.clone(),
        compiler_options: p.compiler_options.clone(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSymbolAtPositionParams {
    #[serde(rename = "snapshot")]
    pub snapshot: SnapshotHandle,
    #[serde(rename = "project")]
    pub project: ProjectHandle,
    #[serde(rename = "file")]
    pub file: DocumentIdentifier,
    #[serde(rename = "position")]
    pub position: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSymbolsAtPositionsParams {
    #[serde(rename = "snapshot")]
    pub snapshot: SnapshotHandle,
    #[serde(rename = "project")]
    pub project: ProjectHandle,
    #[serde(rename = "file")]
    pub file: DocumentIdentifier,
    #[serde(rename = "positions")]
    pub positions: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSymbolAtLocationParams {
    #[serde(rename = "snapshot")]
    pub snapshot: SnapshotHandle,
    #[serde(rename = "project")]
    pub project: ProjectHandle,
    #[serde(rename = "location")]
    pub location: NodeHandle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSymbolsAtLocationsParams {
    #[serde(rename = "snapshot")]
    pub snapshot: SnapshotHandle,
    #[serde(rename = "project")]
    pub project: ProjectHandle,
    #[serde(rename = "locations")]
    pub locations: Vec<NodeHandle>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SymbolResponse {
    #[serde(rename = "id")]
    pub id: SymbolHandle,
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "flags")]
    pub flags: u32,
    #[serde(rename = "checkFlags")]
    pub check_flags: u32,
    #[serde(
        rename = "declarations",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub declarations: Vec<NodeHandle>,
    #[serde(
        rename = "valueDeclaration",
        default,
        skip_serializing_if = "NodeHandle::is_empty"
    )]
    pub value_declaration: NodeHandle,
}

pub(crate) fn new_symbol_response_from_handle(
    ch: &mut checker::Checker<'_, '_>,
    symbol: ast::SymbolHandle,
    mut node_handle_from: impl FnMut(ast::Node) -> NodeHandle,
) -> Option<SymbolResponse> {
    let identity = ast::SymbolIdentity::from_symbol_handle(symbol);
    let mut resp = SymbolResponse {
        id: symbol_handle(ch, symbol),
        name: ch.symbol_name_public(identity)?,
        flags: ch.symbol_flags_public(identity)? as u32,
        check_flags: ch.symbol_check_flags_public(identity)?,
        ..Default::default()
    };

    let declarations = ch.collect_symbol_declarations_public(identity);
    if !declarations.is_empty() {
        resp.declarations = declarations
            .iter()
            .map(|decl| node_handle_from(*decl))
            .collect();
    }

    if let Some(value_declaration) = ch.symbol_value_declaration_public(identity) {
        resp.value_declaration = node_handle_from(value_declaration);
    }

    Some(resp)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTypeOfSymbolParams {
    #[serde(rename = "snapshot")]
    pub snapshot: SnapshotHandle,
    #[serde(rename = "project")]
    pub project: ProjectHandle,
    #[serde(rename = "symbol")]
    pub symbol: SymbolHandle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTypesOfSymbolsParams {
    #[serde(rename = "snapshot")]
    pub snapshot: SnapshotHandle,
    #[serde(rename = "project")]
    pub project: ProjectHandle,
    #[serde(rename = "symbols")]
    pub symbols: Vec<SymbolHandle>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TypeResponse {
    #[serde(rename = "id")]
    pub id: TypeHandle,
    #[serde(rename = "flags")]
    pub flags: u32,
    #[serde(rename = "objectFlags", default, skip_serializing_if = "is_zero_u32")]
    pub object_flags: u32,

    // LiteralType data
    #[serde(rename = "value", default, skip_serializing_if = "Option::is_none")]
    pub value: Option<json::Value>,

    // ObjectType / TypeReference / StringMappingType / IndexType target
    #[serde(
        rename = "target",
        default,
        skip_serializing_if = "TypeHandle::is_empty"
    )]
    pub target: TypeHandle,

    // InterfaceType type parameters
    #[serde(
        rename = "typeParameters",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub type_parameters: Vec<TypeHandle>,
    #[serde(
        rename = "outerTypeParameters",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub outer_type_parameters: Vec<TypeHandle>,
    #[serde(
        rename = "localTypeParameters",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub local_type_parameters: Vec<TypeHandle>,

    // TupleType data
    #[serde(
        rename = "elementFlags",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub element_flags: Vec<u32>,
    #[serde(
        rename = "fixedLength",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub fixed_length: Option<i32>,
    #[serde(rename = "readonly", default, skip_serializing_if = "Option::is_none")]
    pub tuple_readonly: Option<bool>,

    // IndexedAccessType data
    #[serde(
        rename = "objectType",
        default,
        skip_serializing_if = "TypeHandle::is_empty"
    )]
    pub object_type: TypeHandle,
    #[serde(
        rename = "indexType",
        default,
        skip_serializing_if = "TypeHandle::is_empty"
    )]
    pub index_type: TypeHandle,

    // ConditionalType data
    #[serde(
        rename = "checkType",
        default,
        skip_serializing_if = "TypeHandle::is_empty"
    )]
    pub check_type: TypeHandle,
    #[serde(
        rename = "extendsType",
        default,
        skip_serializing_if = "TypeHandle::is_empty"
    )]
    pub extends_type: TypeHandle,

    // SubstitutionType data
    #[serde(
        rename = "baseType",
        default,
        skip_serializing_if = "TypeHandle::is_empty"
    )]
    pub base_type: TypeHandle,
    #[serde(
        rename = "substConstraint",
        default,
        skip_serializing_if = "TypeHandle::is_empty"
    )]
    pub subst_constraint: TypeHandle,

    // TemplateLiteralType text segments
    #[serde(rename = "texts", default, skip_serializing_if = "Vec::is_empty")]
    pub texts: Vec<String>,

    // Symbol associated with structured types
    #[serde(
        rename = "symbol",
        default,
        skip_serializing_if = "SymbolHandle::is_empty"
    )]
    pub symbol: SymbolHandle,
}

pub(crate) fn new_type_data(ch: &checker::Checker<'_, '_>, t: checker::TypeHandle) -> TypeResponse {
    let flags = ch.type_flags_public(t);
    let mut resp = TypeResponse {
        id: type_handle(ch, t),
        flags: u32::from(flags),
        ..Default::default()
    };

    if let Some(symbol) = ch.type_symbol_identity_public(t) {
        resp.symbol = symbol_handle(ch, symbol.symbol_handle());
    }

    if flags & checker::TYPE_FLAGS_LITERAL != 0 {
        resp.value = literal_value_to_json(ch.literal_value_public(t));
    } else if flags & checker::TYPE_FLAGS_OBJECT != 0 {
        resp.object_flags = u32::from(ch.object_flags_public(t));
        let object_flags = ch.object_flags_public(t);
        if object_flags & checker::OBJECT_FLAGS_REFERENCE != 0 {
            if object_flags & checker::OBJECT_FLAGS_TUPLE != 0 {
                resp.element_flags = ch.tuple_element_flags_public(t);
                resp.fixed_length = Some(ch.tuple_fixed_length_public(t) as i32);
                resp.tuple_readonly = Some(ch.tuple_readonly_public(t));
                resp.target = ch
                    .object_target_public(t)
                    .map(|target| type_handle(ch, target))
                    .unwrap_or_default();
            } else if let Some(target) = ch.object_target_public(t) {
                resp.target = type_handle(ch, target);
            }
        }
        if object_flags & checker::OBJECT_FLAGS_CLASS_OR_INTERFACE != 0 {
            resp.type_parameters = type_handles(ch, &ch.interface_type_parameters_public(t));
            resp.outer_type_parameters =
                type_handles(ch, &ch.interface_outer_type_parameters_public(t));
            resp.local_type_parameters =
                type_handles(ch, &ch.interface_local_type_parameters_public(t));
        }
    } else if flags & checker::TYPE_FLAGS_UNION_OR_INTERSECTION != 0 {
        // types omitted; fetched via separate request
    } else if flags & checker::TYPE_FLAGS_INDEX != 0 {
        if let Some(target) = ch.index_type_target_public(t) {
            resp.target = type_handle(ch, target);
        }
    } else if flags & checker::TYPE_FLAGS_INDEXED_ACCESS != 0 {
        if let Some(object_type) = ch.indexed_access_object_type_public(t) {
            resp.object_type = type_handle(ch, object_type);
        }
        if let Some(index_type) = ch.indexed_access_index_type_public(t) {
            resp.index_type = type_handle(ch, index_type);
        }
    } else if flags & checker::TYPE_FLAGS_CONDITIONAL != 0 {
        if let Some(check_type) = ch.conditional_check_type_public(t) {
            resp.check_type = type_handle(ch, check_type);
        }
        if let Some(extends_type) = ch.conditional_extends_type_public(t) {
            resp.extends_type = type_handle(ch, extends_type);
        }
    } else if flags & checker::TYPE_FLAGS_SUBSTITUTION != 0 {
        if let Some(base_type) = ch.substitution_base_type_public(t) {
            resp.base_type = type_handle(ch, base_type);
        }
        if let Some(subst_constraint) = ch.substitution_constraint_public(t) {
            resp.subst_constraint = type_handle(ch, subst_constraint);
        }
    } else if flags & checker::TYPE_FLAGS_TEMPLATE_LITERAL != 0 {
        resp.texts = ch.template_literal_texts_public(t);
        // types omitted; fetched via separate request
    } else if flags & checker::TYPE_FLAGS_STRING_MAPPING != 0 {
        if let Some(target) = ch.string_mapping_target_public(t) {
            resp.target = type_handle(ch, target);
        }
    }

    resp
}

pub(crate) fn type_handles(
    ch: &checker::Checker<'_, '_>,
    types: &[checker::TypeHandle],
) -> Vec<TypeHandle> {
    if types.is_empty() {
        return Vec::new();
    }
    types.iter().map(|&t| type_handle(ch, t)).collect()
}

pub(crate) fn literal_value_to_json(value: checker::LiteralValue) -> Option<json::Value> {
    match value {
        checker::LiteralValue::String(v) => Some(json::Value::from(v)),
        checker::LiteralValue::Number(v) => Some(json::Value::from(v.0)),
        checker::LiteralValue::Bool(v) => Some(json::Value::from(v)),
        checker::LiteralValue::PseudoBigInt(v) => Some(json::Value::from(v.to_string())),
        _ => None,
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SignatureResponse {
    #[serde(rename = "id")]
    pub id: SignatureHandle,
    #[serde(rename = "flags")]
    pub flags: u32,
    #[serde(
        rename = "declaration",
        default,
        skip_serializing_if = "NodeHandle::is_empty"
    )]
    pub declaration: NodeHandle,
    #[serde(
        rename = "typeParameters",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub type_parameters: Vec<TypeHandle>,
    #[serde(rename = "parameters", default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<SymbolHandle>,
    #[serde(
        rename = "thisParameter",
        default,
        skip_serializing_if = "SymbolHandle::is_empty"
    )]
    pub this_parameter: SymbolHandle,
    #[serde(
        rename = "target",
        default,
        skip_serializing_if = "SignatureHandle::is_empty"
    )]
    pub target: SignatureHandle,
}

macro_rules! handle_field_struct {
    ($name:ident { $($field:ident : $ty:ty => $json:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub struct $name {
            $(#[serde(rename = $json)] pub $field: $ty,)+
        }
    };
}

handle_field_struct!(GetSourceFileParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
    file: DocumentIdentifier => "file",
});

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResolveNameParams {
    #[serde(rename = "snapshot")]
    pub snapshot: SnapshotHandle,
    #[serde(rename = "project")]
    pub project: ProjectHandle,
    #[serde(rename = "name")]
    pub name: String,
    #[serde(
        rename = "location",
        default,
        skip_serializing_if = "NodeHandle::is_empty"
    )]
    pub location: NodeHandle, // Optional: node handle for location context
    #[serde(rename = "file", default, skip_serializing_if = "Option::is_none")]
    pub file: Option<DocumentIdentifier>, // Optional: file for location context (alternative to Location)
    #[serde(rename = "position", default, skip_serializing_if = "Option::is_none")]
    pub position: Option<u32>, // Optional: position in file for location context (with File)
    #[serde(rename = "meaning")]
    pub meaning: u32, // SymbolFlags for what kind of symbol to find
    #[serde(rename = "excludeGlobals", default, skip_serializing_if = "is_false")]
    pub exclude_globals: bool, // Whether to exclude global symbols
}

handle_field_struct!(GetParentOfSymbolParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
    symbol: SymbolHandle => "symbol",
});

handle_field_struct!(GetMembersOfSymbolParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
    symbol: SymbolHandle => "symbol",
});

handle_field_struct!(GetExportsOfSymbolParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
    symbol: SymbolHandle => "symbol",
});

handle_field_struct!(GetExportSymbolOfSymbolParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
    symbol: SymbolHandle => "symbol",
});

handle_field_struct!(GetSymbolOfTypeParams {
    snapshot: SnapshotHandle => "snapshot",
    r#type: TypeHandle => "type",
});

// GetTypePropertyParams is used for all type sub-property endpoints.
handle_field_struct!(GetTypePropertyParams {
    snapshot: SnapshotHandle => "snapshot",
    r#type: TypeHandle => "type",
});

// GetContextualTypeParams returns the contextual type for a node.
handle_field_struct!(GetContextualTypeParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
    location: NodeHandle => "location",
});

// GetTypeOfSymbolAtLocationParams returns the narrowed type of a symbol at a specific location.
handle_field_struct!(GetTypeOfSymbolAtLocationParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
    symbol: SymbolHandle => "symbol",
    location: NodeHandle => "location",
});

// GetIntrinsicTypeParams is used for intrinsic type getters (anyType, stringType, etc.).
handle_field_struct!(GetIntrinsicTypeParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
    file: DocumentIdentifier => "file",
});

// GetBaseTypeOfLiteralTypeParams returns the base type of a literal type.
handle_field_struct!(GetBaseTypeOfLiteralTypeParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
    r#type: TypeHandle => "type",
});

// GetNonNullableTypeParams are the parameters for the getNonNullableType method.
handle_field_struct!(GetNonNullableTypeParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
    r#type: TypeHandle => "type",
});

// GetTypeFromTypeNodeParams are the parameters for the getTypeFromTypeNode method.
handle_field_struct!(GetTypeFromTypeNodeParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
    location: NodeHandle => "location",
});

// GetWidenedTypeParams are the parameters for the getWidenedType method.
handle_field_struct!(GetWidenedTypeParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
    r#type: TypeHandle => "type",
});

// GetParameterTypeParams are the parameters for the getParameterType method.
handle_field_struct!(GetParameterTypeParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
    signature: SignatureHandle => "signature",
    index: i32 => "index",
});

// IsArrayLikeTypeParams checks whether a type is array-like.
handle_field_struct!(IsArrayLikeTypeParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
    r#type: TypeHandle => "type",
});

handle_field_struct!(GetSignaturesOfTypeParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
    r#type: TypeHandle => "type",
    kind: i32 => "kind",
});

handle_field_struct!(GetResolvedSignatureParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
    location: NodeHandle => "location",
});

handle_field_struct!(GetTypeAtLocationParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
    location: NodeHandle => "location",
});

handle_field_struct!(GetTypeAtLocationsParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
    locations: Vec<NodeHandle> => "locations",
});

handle_field_struct!(GetTypeAtPositionParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
    file: DocumentIdentifier => "file",
    position: u32 => "position",
});

handle_field_struct!(GetTypesAtPositionsParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
    file: DocumentIdentifier => "file",
    positions: Vec<u32> => "positions",
});

// TypeToTypeNodeParams are the parameters for the typeToTypeNode method.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TypeToTypeNodeParams {
    #[serde(rename = "snapshot")]
    pub snapshot: SnapshotHandle,
    #[serde(rename = "project")]
    pub project: ProjectHandle,
    #[serde(rename = "type")]
    pub r#type: TypeHandle,
    #[serde(
        rename = "location",
        default,
        skip_serializing_if = "NodeHandle::is_empty"
    )]
    pub location: NodeHandle,
    #[serde(rename = "flags", default, skip_serializing_if = "is_zero_i32")]
    pub flags: i32,
}

// SignatureToSignatureDeclarationParams are the parameters for the signatureToSignatureDeclaration method.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SignatureToSignatureDeclarationParams {
    #[serde(rename = "snapshot")]
    pub snapshot: SnapshotHandle,
    #[serde(rename = "project")]
    pub project: ProjectHandle,
    #[serde(rename = "signature")]
    pub signature: SignatureHandle,
    #[serde(rename = "kind")]
    pub kind: i32,
    #[serde(
        rename = "location",
        default,
        skip_serializing_if = "NodeHandle::is_empty"
    )]
    pub location: NodeHandle,
    #[serde(rename = "flags", default, skip_serializing_if = "is_zero_i32")]
    pub flags: i32,
}

// PrintNodeParams are the parameters for the printNode method.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrintNodeParams {
    #[serde(rename = "data")]
    pub data: String, // base64-encoded binary AST data
    #[serde(
        rename = "preserveSourceNewlines",
        default,
        skip_serializing_if = "is_false"
    )]
    pub preserve_source_newlines: bool,
    #[serde(rename = "neverAsciiEscape", default, skip_serializing_if = "is_false")]
    pub never_ascii_escape: bool,
    #[serde(
        rename = "terminateUnterminatedLiterals",
        default,
        skip_serializing_if = "is_false"
    )]
    pub terminate_unterminated_literals: bool,
}

// CheckerTypeParams are parameters for checker methods that operate on a type.
handle_field_struct!(CheckerTypeParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
    r#type: TypeHandle => "type",
});

// CheckerSignatureParams are parameters for checker methods that operate on a signature.
handle_field_struct!(CheckerSignatureParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
    signature: SignatureHandle => "signature",
});

// TypePredicateResponse is the response for getTypePredicateOfSignature.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TypePredicateResponse {
    #[serde(rename = "kind")]
    pub kind: i32,
    #[serde(rename = "parameterIndex")]
    pub parameter_index: i32,
    #[serde(
        rename = "parameterName",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub parameter_name: String,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<TypeResponse>,
}

// IndexInfoResponse represents a single index signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfoResponse {
    #[serde(rename = "keyType")]
    pub key_type: TypeResponse,
    #[serde(rename = "valueType")]
    pub value_type: TypeResponse,
    #[serde(rename = "isReadonly", default, skip_serializing_if = "is_false")]
    pub is_readonly: bool,
}

// SourceFileResponse contains the binary-encoded AST data for a source file.
// The Data field is base64-encoded binary data in the encoder's format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceFileResponse {
    #[serde(rename = "data")]
    pub data: String,
}

// GetDiagnosticsParams are parameters for per-file diagnostic methods.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GetDiagnosticsParams {
    #[serde(rename = "snapshot")]
    pub snapshot: SnapshotHandle,
    #[serde(rename = "project")]
    pub project: ProjectHandle,
    #[serde(rename = "file", default, skip_serializing_if = "Option::is_none")]
    pub file: Option<DocumentIdentifier>,
}

// GetProjectDiagnosticsParams are parameters for project-wide diagnostic methods.
handle_field_struct!(GetProjectDiagnosticsParams {
    snapshot: SnapshotHandle => "snapshot",
    project: ProjectHandle => "project",
});

// DiagnosticResponse is the API response for a single diagnostic.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiagnosticResponse {
    // FileName is the path of the file this diagnostic belongs to, if any.
    #[serde(rename = "fileName", default, skip_serializing_if = "String::is_empty")]
    pub file_name: String,
    // Pos is the start position of the diagnostic in the source file.
    #[serde(rename = "pos")]
    pub pos: i32,
    // End is the end position of the diagnostic in the source file.
    #[serde(rename = "end")]
    pub end: i32,
    // Code is the diagnostic error code.
    #[serde(rename = "code")]
    pub code: i32,
    // Category is the diagnostic category (error, warning, suggestion, message).
    #[serde(rename = "category")]
    pub category: diagnostics::Category,
    // Text is the localized diagnostic message text.
    #[serde(rename = "text")]
    pub text: String,
    // ReportsUnnecessary indicates this diagnostic highlights unnecessary code.
    #[serde(
        rename = "reportsUnnecessary",
        default,
        skip_serializing_if = "is_false"
    )]
    pub reports_unnecessary: bool,
    // ReportsDeprecated indicates this diagnostic highlights deprecated code.
    #[serde(
        rename = "reportsDeprecated",
        default,
        skip_serializing_if = "is_false"
    )]
    pub reports_deprecated: bool,
    // MessageChain contains chained diagnostic messages, if any.
    #[serde(
        rename = "messageChain",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub message_chain: Vec<DiagnosticResponse>,
    // RelatedInformation contains related diagnostic information, if any.
    #[serde(
        rename = "relatedInformation",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub related_information: Vec<DiagnosticResponse>,
}

// NewDiagnosticResponse converts an ast.Diagnostic to a DiagnosticResponse.
pub fn new_diagnostic_response(d: &ast::Diagnostic) -> DiagnosticResponse {
    let mut resp = DiagnosticResponse {
        pos: d.pos(),
        end: d.end(),
        code: d.code(),
        category: d.category(),
        text: d.localize(locale::DEFAULT),
        reports_unnecessary: d.reports_unnecessary(),
        reports_deprecated: d.reports_deprecated(),
        ..Default::default()
    };

    if let Some(file) = d.file() {
        resp.file_name = file.file_name().to_string();
    }

    let chain = d.message_chain();
    if !chain.is_empty() {
        resp.message_chain = chain.iter().map(new_diagnostic_response).collect();
    }

    let related = d.related_information();
    if !related.is_empty() {
        resp.related_information = related.iter().map(new_diagnostic_response).collect();
    }

    resp
}

// NewDiagnosticResponses converts a slice of ast.Diagnostics to DiagnosticResponses.
pub fn new_diagnostic_responses(diags: &[ast::Diagnostic]) -> Vec<DiagnosticResponse> {
    if diags.is_empty() {
        return Vec::new();
    }
    diags.iter().map(new_diagnostic_response).collect()
}

pub fn unmarshal_payload(method: &str, payload: json::Value) -> Result<json::Value, String> {
    let Some(unmarshaler) = UNMARSHALERS.get(method) else {
        return Err(format!("unknown API method {method:?}"));
    };
    unmarshaler(payload)
}

fn unmarshaller_for<T>(data: json::Value) -> Result<json::Value, String>
where
    T: for<'de> Deserialize<'de> + Serialize,
{
    let v: T = serde_json::from_value(data)
        .map_err(|err| format!("failed to unmarshal {}: {err}", std::any::type_name::<T>()))?;
    serde_json::to_value(v).map_err(|err| err.to_string())
}

fn no_params(_data: json::Value) -> Result<json::Value, String> {
    Ok(json::Value::Null)
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

fn is_zero_i32(value: &i32) -> bool {
    *value == 0
}
