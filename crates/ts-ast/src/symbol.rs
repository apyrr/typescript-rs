use std::{
    cmp, fmt,
    hash::Hasher,
    num::{NonZeroU32, NonZeroU64},
    ops::Deref,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

use smallvec::SmallVec;
use smol_str::SmolStr;
use ts_collections::{Arena, GxBuildHasher, Idx, RawIdx};

use crate::*;

static NEXT_SYMBOL_OWNER_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_SYMBOL_ID: AtomicU64 = AtomicU64::new(0);

#[allow(non_snake_case, non_upper_case_globals)]
pub mod InternalSymbolName {
    pub const Prefix: &str = "\u{fe}";
    pub const Call: &str = "\u{fe}call";
    pub const Constructor: &str = "\u{fe}constructor";
    pub const New: &str = "\u{fe}new";
    pub const Index: &str = "\u{fe}index";
    pub const ExportStar: &str = "\u{fe}export";
    pub const Global: &str = "\u{fe}global";
    pub const Missing: &str = "\u{fe}missing";
    pub const Type: &str = "\u{fe}type";
    pub const Object: &str = "\u{fe}object";
    pub const JSXAttributes: &str = "\u{fe}jsxAttributes";
    pub const Class: &str = "\u{fe}class";
    pub const Function: &str = "\u{fe}function";
    pub const Computed: &str = "\u{fe}computed";
    pub const AssignmentDeclaration: &str = "\u{fe}assignment";
    pub const InstantiationExpression: &str = "\u{fe}instantiationExpression";
    pub const ImportAttributes: &str = "\u{fe}importAttributes";
    pub const ExportEquals: &str = "export=";
    pub const Default: &str = "default";
    pub const This: &str = "this";
    pub const ModuleExports: &str = "module.exports";
}

pub trait NonNullOptionExt<T> {
    fn as_deref(&self) -> Option<&T>;
    fn as_deref_mut(&mut self) -> Option<&mut T>;
}

impl NonNullOptionExt<Node> for Option<Node> {
    fn as_deref(&self) -> Option<&Node> {
        self.as_ref()
    }

    fn as_deref_mut(&mut self) -> Option<&mut Node> {
        self.as_mut()
    }
}

// Symbol

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SymbolDomain {
    Program,
    CheckerTransient,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct SymbolOwnerId(NonZeroU64);

impl SymbolOwnerId {
    fn fresh() -> Self {
        let owner = NEXT_SYMBOL_OWNER_ID.fetch_add(1, Ordering::Relaxed);
        Self(NonZeroU64::new(owner).expect("symbol owner id must be non-zero"))
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct ProgramSymbolOwnerKey(SymbolOwnerId);

impl ProgramSymbolOwnerKey {
    #[inline]
    pub fn as_u64(self) -> u64 {
        self.0.0.get()
    }
}

impl fmt::Debug for ProgramSymbolOwnerKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ProgramSymbolOwnerKey")
            .field(&"<opaque>")
            .finish()
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct SymbolOwnerKey {
    domain: SymbolDomain,
    owner: SymbolOwnerId,
}

impl SymbolOwnerKey {
    #[inline]
    pub fn as_u64(self) -> u64 {
        self.owner.0.get()
    }
}

impl fmt::Debug for SymbolOwnerKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SymbolOwnerKey")
            .field("domain", &self.domain)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct RawSymbolId(NonZeroU32);

impl RawSymbolId {
    fn from_idx(idx: Idx<StoredSymbol>) -> Self {
        let raw = idx.into_raw().into_u32();
        let packed = raw
            .checked_add(1)
            .expect("symbol store index exceeds u32 payload space");
        Self(NonZeroU32::new(packed).expect("packed symbol id must be non-zero"))
    }

    #[inline]
    fn to_idx(self) -> Idx<StoredSymbol> {
        Idx::from_raw(RawIdx::from_u32(self.0.get() - 1))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SymbolId(NonZeroU64);

impl SymbolId {
    fn fresh() -> Self {
        let id = NEXT_SYMBOL_ID.fetch_add(1, Ordering::Relaxed) + 1;
        Self(NonZeroU64::new(id).expect("symbol id must be non-zero"))
    }

    const fn get(self) -> u64 {
        self.0.get()
    }

    pub fn cmp_for_ordering(self, other: Self) -> cmp::Ordering {
        self.get().cmp(&other.get())
    }

    pub fn write_stable_hash(self, state: &mut impl Hasher) {
        state.write(&self.get().to_le_bytes());
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct SymbolHandle {
    domain: SymbolDomain,
    owner: SymbolOwnerId,
    raw: RawSymbolId,
}

impl SymbolHandle {
    fn from_idx(domain: SymbolDomain, owner: SymbolOwnerId, idx: Idx<StoredSymbol>) -> Self {
        Self {
            domain,
            owner,
            raw: RawSymbolId::from_idx(idx),
        }
    }

    pub const fn domain(self) -> SymbolDomain {
        self.domain
    }

    pub fn checker_transient_index(self) -> Option<usize> {
        (self.domain == SymbolDomain::CheckerTransient)
            .then(|| self.raw.to_idx().into_raw().into_usize())
    }

    pub fn symbol_index(self) -> usize {
        self.raw.to_idx().into_raw().into_usize()
    }

    pub fn owner_key(self) -> SymbolOwnerKey {
        SymbolOwnerKey {
            domain: self.domain,
            owner: self.owner,
        }
    }

    pub fn program_owner_key(self) -> Option<ProgramSymbolOwnerKey> {
        (self.domain == SymbolDomain::Program).then_some(ProgramSymbolOwnerKey(self.owner))
    }
}

impl fmt::Debug for SymbolHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SymbolHandle")
            .field("domain", &self.domain)
            .finish_non_exhaustive()
    }
}

pub type SymbolHandleTable = indexmap::IndexMap<SymbolName, SymbolHandle, GxBuildHasher>;
type OwnedSymbolDeclarations = SmallVec<[Node; 1]>;

#[derive(Clone, Debug)]
enum SymbolDeclarationStorage {
    Owned(OwnedSymbolDeclarations),
    Shared(Arc<[Node]>),
}

#[derive(Clone, Debug)]
pub struct SymbolDeclarations {
    storage: SymbolDeclarationStorage,
}

#[derive(Clone, Copy, Debug)]
pub struct SymbolInstantiationHeader {
    pub flags: SymbolFlags,
    pub check_flags: CheckFlags,
}

#[derive(Clone, Debug)]
pub struct SymbolInstantiationSnapshot {
    pub flags: SymbolFlags,
    pub check_flags: CheckFlags,
    pub name: SymbolName,
    pub declarations: SymbolDeclarations,
    pub value_declaration: Option<Node>,
    pub parent: Option<SymbolHandle>,
}

#[derive(Clone, Copy, Debug)]
pub struct SymbolValueDeclarationSnapshot {
    pub flags: SymbolFlags,
    pub value_declaration: Option<Node>,
}

impl SymbolDeclarations {
    pub fn new() -> Self {
        Self {
            storage: SymbolDeclarationStorage::Owned(OwnedSymbolDeclarations::new()),
        }
    }

    fn shared(declarations: Arc<[Node]>) -> Self {
        Self {
            storage: SymbolDeclarationStorage::Shared(declarations),
        }
    }

    fn as_slice(&self) -> &[Node] {
        match &self.storage {
            SymbolDeclarationStorage::Owned(declarations) => declarations.as_slice(),
            SymbolDeclarationStorage::Shared(declarations) => declarations,
        }
    }

    fn make_owned_mut(&mut self) -> &mut OwnedSymbolDeclarations {
        if let SymbolDeclarationStorage::Shared(declarations) = &self.storage {
            let mut owned = OwnedSymbolDeclarations::new();
            owned.extend(declarations.iter().copied());
            self.storage = SymbolDeclarationStorage::Owned(owned);
        }
        let SymbolDeclarationStorage::Owned(declarations) = &mut self.storage else {
            unreachable!("shared declarations were converted to owned declarations");
        };
        declarations
    }

    fn push(&mut self, declaration: Node) {
        self.make_owned_mut().push(declaration);
    }
}

impl Default for SymbolDeclarations {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for SymbolDeclarations {
    type Target = [Node];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl From<Vec<Node>> for SymbolDeclarations {
    fn from(declarations: Vec<Node>) -> Self {
        Self {
            storage: SymbolDeclarationStorage::Owned(declarations.into_iter().collect()),
        }
    }
}

impl FromIterator<Node> for SymbolDeclarations {
    fn from_iter<T: IntoIterator<Item = Node>>(iter: T) -> Self {
        Self {
            storage: SymbolDeclarationStorage::Owned(iter.into_iter().collect()),
        }
    }
}

#[derive(Debug)]
struct StoredSymbol {
    id: AtomicU64,
    flags: SymbolFlags,
    check_flags: CheckFlags,
    name: SymbolName,
    declarations: SymbolDeclarations,
    shared_declarations: OnceLock<Arc<[Node]>>,
    value_declaration: Option<Node>,
    members: Option<SymbolHandleTable>,
    exports: Option<SymbolHandleTable>,
    parent: Option<SymbolHandle>,
    export_symbol: Option<SymbolHandle>,
}

impl StoredSymbol {
    fn empty() -> Self {
        Self {
            id: AtomicU64::new(0),
            flags: SYMBOL_FLAGS_NONE,
            check_flags: CHECK_FLAGS_NONE,
            name: SymbolName::new_inline(""),
            declarations: SymbolDeclarations::new(),
            shared_declarations: OnceLock::new(),
            value_declaration: None,
            members: None,
            exports: None,
            parent: None,
            export_symbol: None,
        }
    }

    fn new(flags: SymbolFlags, name: impl Into<SymbolName>) -> Self {
        Self {
            id: AtomicU64::new(0),
            flags,
            check_flags: CHECK_FLAGS_NONE,
            name: name.into(),
            declarations: SymbolDeclarations::new(),
            shared_declarations: OnceLock::new(),
            value_declaration: None,
            members: None,
            exports: None,
            parent: None,
            export_symbol: None,
        }
    }

    fn new_with_check_flags(
        flags: SymbolFlags,
        name: impl Into<SymbolName>,
        check_flags: CheckFlags,
    ) -> Self {
        Self {
            id: AtomicU64::new(0),
            flags,
            check_flags,
            name: name.into(),
            declarations: SymbolDeclarations::new(),
            shared_declarations: OnceLock::new(),
            value_declaration: None,
            members: None,
            exports: None,
            parent: None,
            export_symbol: None,
        }
    }

    fn new_from_instantiation(
        flags: SymbolFlags,
        name: impl Into<SymbolName>,
        check_flags: CheckFlags,
        declarations: SymbolDeclarations,
        value_declaration: Option<Node>,
        parent: Option<SymbolHandle>,
    ) -> Self {
        Self {
            id: AtomicU64::new(0),
            flags,
            check_flags,
            name: name.into(),
            declarations,
            shared_declarations: OnceLock::new(),
            value_declaration,
            members: None,
            exports: None,
            parent,
            export_symbol: None,
        }
    }

    fn flags(&self) -> SymbolFlags {
        self.flags
    }

    fn add_flags(&mut self, flags: SymbolFlags) {
        self.flags |= flags;
    }

    fn remove_flags(&mut self, flags: SymbolFlags) {
        self.flags &= !flags;
    }

    fn check_flags(&self) -> CheckFlags {
        self.check_flags
    }

    fn set_check_flags(&mut self, check_flags: CheckFlags) {
        self.check_flags = check_flags;
    }

    fn add_check_flags(&mut self, check_flags: CheckFlags) {
        self.check_flags |= check_flags;
    }

    fn name(&self) -> &SymbolName {
        &self.name
    }

    fn declarations(&self) -> &[Node] {
        self.declarations.as_slice()
    }

    fn share_declarations(&self) -> SymbolDeclarations {
        match &self.declarations.storage {
            SymbolDeclarationStorage::Owned(declarations) if declarations.is_empty() => {
                SymbolDeclarations::new()
            }
            SymbolDeclarationStorage::Owned(declarations) => {
                let shared = self
                    .shared_declarations
                    .get_or_init(|| Arc::from(declarations.as_slice()));
                SymbolDeclarations::shared(Arc::clone(shared))
            }
            SymbolDeclarationStorage::Shared(declarations) => {
                SymbolDeclarations::shared(Arc::clone(declarations))
            }
        }
    }

    fn set_declarations(&mut self, declarations: impl Into<SymbolDeclarations>) {
        let _ = self.shared_declarations.take();
        self.declarations = declarations.into();
    }

    fn add_declaration(&mut self, declaration: Node) {
        let _ = self.shared_declarations.take();
        self.declarations.push(declaration);
    }

    fn add_declaration_if_unique(&mut self, declaration: Node) {
        if self.declarations.is_empty() {
            let _ = self.shared_declarations.take();
            self.declarations.push(declaration);
        } else if !self.declarations.contains(&declaration) {
            let _ = self.shared_declarations.take();
            self.declarations.push(declaration);
        }
    }

    fn value_declaration(&self) -> Option<Node> {
        self.value_declaration
    }

    fn set_value_declaration(&mut self, value_declaration: Option<Node>) {
        self.value_declaration = value_declaration;
    }

    fn members(&self) -> Option<&SymbolHandleTable> {
        self.members.as_ref()
    }

    fn ensure_members(&mut self) -> &mut SymbolHandleTable {
        self.members.get_or_insert_with(SymbolHandleTable::default)
    }

    fn set_members(&mut self, members: Option<SymbolHandleTable>) {
        self.members = members;
    }

    fn exports(&self) -> Option<&SymbolHandleTable> {
        self.exports.as_ref()
    }

    fn ensure_exports(&mut self) -> &mut SymbolHandleTable {
        self.exports.get_or_insert_with(SymbolHandleTable::default)
    }

    fn set_exports(&mut self, exports: Option<SymbolHandleTable>) {
        self.exports = exports;
    }

    fn insert_export(
        &mut self,
        name: impl Into<SymbolName>,
        symbol: SymbolHandle,
    ) -> Option<SymbolHandle> {
        self.ensure_exports().insert(name.into(), symbol)
    }

    fn insert_member(
        &mut self,
        name: impl Into<SymbolName>,
        symbol: SymbolHandle,
    ) -> Option<SymbolHandle> {
        self.ensure_members().insert(name.into(), symbol)
    }

    fn parent(&self) -> Option<SymbolHandle> {
        self.parent
    }

    fn set_parent(&mut self, parent: Option<SymbolHandle>) {
        self.parent = parent;
    }

    fn export_symbol(&self) -> Option<SymbolHandle> {
        self.export_symbol
    }

    fn set_export_symbol(&mut self, export_symbol: Option<SymbolHandle>) {
        self.export_symbol = export_symbol;
    }

    fn instantiation_header(&self) -> SymbolInstantiationHeader {
        SymbolInstantiationHeader {
            flags: self.flags,
            check_flags: self.check_flags,
        }
    }

    fn instantiation_snapshot(&self) -> SymbolInstantiationSnapshot {
        SymbolInstantiationSnapshot {
            flags: self.flags,
            check_flags: self.check_flags,
            name: self.name.clone(),
            declarations: self.share_declarations(),
            value_declaration: self.value_declaration,
            parent: self.parent,
        }
    }

    fn value_declaration_snapshot(&self) -> SymbolValueDeclarationSnapshot {
        SymbolValueDeclarationSnapshot {
            flags: self.flags,
            value_declaration: self.value_declaration,
        }
    }
}

#[derive(Debug)]
pub(crate) struct SymbolStore {
    domain: SymbolDomain,
    owner: SymbolOwnerId,
    symbols: Arena<StoredSymbol>,
}

#[derive(Debug)]
pub struct ProgramSymbolStore {
    store: SymbolStore,
}

#[derive(Debug)]
pub struct TransientSymbolStore {
    store: SymbolStore,
}

impl SymbolStore {
    fn new(domain: SymbolDomain) -> Self {
        Self::with_owner(domain, SymbolOwnerId::fresh())
    }

    fn program() -> Self {
        Self::new(SymbolDomain::Program)
    }

    fn checker_transient() -> Self {
        Self::new(SymbolDomain::CheckerTransient)
    }

    fn with_owner(domain: SymbolDomain, owner: SymbolOwnerId) -> Self {
        Self {
            domain,
            owner,
            symbols: Arena::new(),
        }
    }

    pub const fn domain(&self) -> SymbolDomain {
        self.domain
    }

    fn allocate(&mut self, symbol: StoredSymbol) -> SymbolHandle {
        let idx = self.symbols.alloc(symbol);
        SymbolHandle::from_idx(self.domain, self.owner, idx)
    }

    fn create_symbol(&mut self, flags: SymbolFlags, name: impl Into<SymbolName>) -> SymbolHandle {
        self.allocate(StoredSymbol::new(flags, name))
    }

    fn create_symbol_with_check_flags(
        &mut self,
        flags: SymbolFlags,
        name: impl Into<SymbolName>,
        check_flags: CheckFlags,
    ) -> SymbolHandle {
        self.allocate(StoredSymbol::new_with_check_flags(flags, name, check_flags))
    }

    fn create_symbol_from_instantiation(
        &mut self,
        flags: SymbolFlags,
        name: impl Into<SymbolName>,
        check_flags: CheckFlags,
        declarations: SymbolDeclarations,
        value_declaration: Option<Node>,
        parent: Option<SymbolHandle>,
    ) -> SymbolHandle {
        self.allocate(StoredSymbol::new_from_instantiation(
            flags,
            name,
            check_flags,
            declarations,
            value_declaration,
            parent,
        ))
    }

    fn get(&self, handle: SymbolHandle) -> Option<&StoredSymbol> {
        self.idx_for(handle).and_then(|idx| self.symbols.get(idx))
    }

    fn get_mut(&mut self, handle: SymbolHandle) -> Option<&mut StoredSymbol> {
        self.idx_for(handle)
            .and_then(|idx| self.symbols.get_mut(idx))
    }

    fn require(&self, handle: SymbolHandle) -> &StoredSymbol {
        self.get(handle)
            .expect("symbol handle does not belong to this symbol store")
    }

    fn require_mut(&mut self, handle: SymbolHandle) -> &mut StoredSymbol {
        self.get_mut(handle)
            .expect("symbol handle does not belong to this symbol store")
    }

    #[inline]
    fn require_owned(&self, handle: SymbolHandle) -> &StoredSymbol {
        self.symbols
            .get(handle.raw.to_idx())
            .expect("owned symbol handle must resolve in this symbol store")
    }

    pub fn owns(&self, handle: SymbolHandle) -> bool {
        handle.domain == self.domain && handle.owner == self.owner
    }

    pub fn symbol_id(&self, handle: SymbolHandle) -> SymbolId {
        let id = &self.require(handle).id;
        let current = id.load(Ordering::Acquire);
        if let Some(current) = NonZeroU64::new(current) {
            return SymbolId(current);
        }

        let fresh = SymbolId::fresh();
        match id.compare_exchange(0, fresh.get(), Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => fresh,
            Err(existing) => {
                SymbolId(NonZeroU64::new(existing).expect("racing symbol id must be non-zero"))
            }
        }
    }

    pub fn private_identifier_symbol_name(
        &self,
        handle: SymbolHandle,
        description: &str,
    ) -> String {
        format!(
            "{}#{}@{}",
            INTERNAL_SYMBOL_NAME_PREFIX,
            self.symbol_id(handle).get(),
            description
        )
    }

    pub fn unique_es_symbol_type_name(&self, handle: SymbolHandle, symbol_name: &str) -> String {
        format!(
            "{}@{}@{}",
            INTERNAL_SYMBOL_NAME_PREFIX,
            symbol_name,
            self.symbol_id(handle).get()
        )
    }

    pub fn flags(&self, handle: SymbolHandle) -> SymbolFlags {
        self.require(handle).flags()
    }

    #[inline]
    fn flags_for_owned_handle(&self, handle: SymbolHandle) -> SymbolFlags {
        self.require_owned(handle).flags()
    }

    fn add_flags(&mut self, handle: SymbolHandle, flags: SymbolFlags) {
        self.require_mut(handle).add_flags(flags);
    }

    fn remove_flags(&mut self, handle: SymbolHandle, flags: SymbolFlags) {
        self.require_mut(handle).remove_flags(flags);
    }

    pub fn check_flags(&self, handle: SymbolHandle) -> CheckFlags {
        self.require(handle).check_flags()
    }

    #[inline]
    fn check_flags_for_owned_handle(&self, handle: SymbolHandle) -> CheckFlags {
        self.require_owned(handle).check_flags()
    }

    fn set_check_flags(&mut self, handle: SymbolHandle, check_flags: CheckFlags) {
        self.require_mut(handle).set_check_flags(check_flags);
    }

    fn add_check_flags(&mut self, handle: SymbolHandle, check_flags: CheckFlags) {
        self.require_mut(handle).add_check_flags(check_flags);
    }

    pub fn name(&self, handle: SymbolHandle) -> &SymbolName {
        self.require(handle).name()
    }

    #[inline]
    fn name_for_owned_handle(&self, handle: SymbolHandle) -> &SymbolName {
        self.require_owned(handle).name()
    }

    fn declarations(&self, handle: SymbolHandle) -> &[Node] {
        self.require(handle).declarations()
    }

    fn share_declarations(&self, handle: SymbolHandle) -> SymbolDeclarations {
        self.require(handle).share_declarations()
    }

    #[inline]
    fn declarations_for_owned_handle(&self, handle: SymbolHandle) -> &[Node] {
        self.require_owned(handle).declarations()
    }

    #[inline]
    fn share_declarations_for_owned_handle(&self, handle: SymbolHandle) -> SymbolDeclarations {
        self.require_owned(handle).share_declarations()
    }

    #[inline]
    fn first_declaration_for_owned_handle(&self, handle: SymbolHandle) -> Option<Node> {
        self.require_owned(handle).declarations().first().copied()
    }

    fn set_declarations(
        &mut self,
        handle: SymbolHandle,
        declarations: impl Into<SymbolDeclarations>,
    ) {
        self.require_mut(handle).set_declarations(declarations);
    }

    fn add_declaration(&mut self, handle: SymbolHandle, declaration: Node) {
        self.require_mut(handle).add_declaration(declaration);
    }

    fn add_declaration_if_unique(&mut self, handle: SymbolHandle, declaration: Node) {
        self.require_mut(handle)
            .add_declaration_if_unique(declaration);
    }

    pub fn value_declaration(&self, handle: SymbolHandle) -> Option<Node> {
        self.require(handle).value_declaration()
    }

    #[inline]
    fn value_declaration_for_owned_handle(&self, handle: SymbolHandle) -> Option<Node> {
        self.require_owned(handle).value_declaration()
    }

    #[inline]
    fn value_declaration_snapshot_for_owned_handle(
        &self,
        handle: SymbolHandle,
    ) -> SymbolValueDeclarationSnapshot {
        self.require_owned(handle).value_declaration_snapshot()
    }

    fn set_value_declaration(&mut self, handle: SymbolHandle, value_declaration: Option<Node>) {
        self.require_mut(handle)
            .set_value_declaration(value_declaration);
    }

    fn members(&self, handle: SymbolHandle) -> Option<&SymbolHandleTable> {
        self.require(handle).members()
    }

    #[inline]
    fn members_for_owned_handle(&self, handle: SymbolHandle) -> Option<&SymbolHandleTable> {
        self.require_owned(handle).members()
    }

    fn set_members(&mut self, handle: SymbolHandle, members: Option<SymbolHandleTable>) {
        self.require_mut(handle).set_members(members);
    }

    fn lookup_member(&self, handle: SymbolHandle, name: &str) -> Option<SymbolHandle> {
        self.members(handle)
            .and_then(|members| members.get(name).copied())
    }

    fn insert_member(
        &mut self,
        handle: SymbolHandle,
        name: impl Into<SymbolName>,
        symbol: SymbolHandle,
    ) -> Option<SymbolHandle> {
        self.require_mut(handle).insert_member(name, symbol)
    }

    fn exports(&self, handle: SymbolHandle) -> Option<&SymbolHandleTable> {
        self.require(handle).exports()
    }

    #[inline]
    fn exports_for_owned_handle(&self, handle: SymbolHandle) -> Option<&SymbolHandleTable> {
        self.require_owned(handle).exports()
    }

    fn set_exports(&mut self, handle: SymbolHandle, exports: Option<SymbolHandleTable>) {
        self.require_mut(handle).set_exports(exports);
    }

    fn lookup_export(&self, handle: SymbolHandle, name: &str) -> Option<SymbolHandle> {
        self.exports(handle)
            .and_then(|exports| exports.get(name).copied())
    }

    fn insert_export(
        &mut self,
        handle: SymbolHandle,
        name: impl Into<SymbolName>,
        symbol: SymbolHandle,
    ) -> Option<SymbolHandle> {
        self.require_mut(handle).insert_export(name, symbol)
    }

    pub fn parent(&self, handle: SymbolHandle) -> Option<SymbolHandle> {
        self.require(handle).parent()
    }

    #[inline]
    fn parent_for_owned_handle(&self, handle: SymbolHandle) -> Option<SymbolHandle> {
        self.require_owned(handle).parent()
    }

    #[inline]
    fn instantiation_header_for_owned_handle(
        &self,
        handle: SymbolHandle,
    ) -> SymbolInstantiationHeader {
        self.require_owned(handle).instantiation_header()
    }

    #[inline]
    fn instantiation_snapshot_for_owned_handle(
        &self,
        handle: SymbolHandle,
    ) -> SymbolInstantiationSnapshot {
        self.require_owned(handle).instantiation_snapshot()
    }

    fn set_parent(&mut self, handle: SymbolHandle, parent: Option<SymbolHandle>) {
        self.require_mut(handle).set_parent(parent);
    }

    pub fn export_symbol(&self, handle: SymbolHandle) -> Option<SymbolHandle> {
        self.require(handle).export_symbol()
    }

    #[inline]
    fn export_symbol_for_owned_handle(&self, handle: SymbolHandle) -> Option<SymbolHandle> {
        self.require_owned(handle).export_symbol()
    }

    fn set_export_symbol(&mut self, handle: SymbolHandle, export_symbol: Option<SymbolHandle>) {
        self.require_mut(handle).set_export_symbol(export_symbol);
    }

    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    fn idx_for(&self, handle: SymbolHandle) -> Option<Idx<StoredSymbol>> {
        self.owns(handle).then(|| handle.raw.to_idx())
    }

    #[inline]
    fn index_for(&self, handle: SymbolHandle) -> Option<usize> {
        self.idx_for(handle).map(|idx| idx.into_raw().into_usize())
    }

    #[inline]
    fn index_for_owned_handle(&self, handle: SymbolHandle) -> usize {
        handle.raw.to_idx().into_raw().into_usize()
    }
}

impl ProgramSymbolStore {
    pub fn new() -> Self {
        Self {
            store: SymbolStore::program(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }

    pub fn owner_key(&self) -> ProgramSymbolOwnerKey {
        ProgramSymbolOwnerKey(self.store.owner)
    }

    pub fn owns(&self, handle: SymbolHandle) -> bool {
        self.store.owns(handle)
    }

    pub fn symbol_id(&self, handle: SymbolHandle) -> SymbolId {
        self.store.symbol_id(handle)
    }

    pub fn private_identifier_symbol_name(
        &self,
        handle: SymbolHandle,
        description: &str,
    ) -> String {
        self.store
            .private_identifier_symbol_name(handle, description)
    }

    pub fn unique_es_symbol_type_name(&self, handle: SymbolHandle, symbol_name: &str) -> String {
        self.store.unique_es_symbol_type_name(handle, symbol_name)
    }

    pub fn flags(&self, handle: SymbolHandle) -> SymbolFlags {
        self.store.flags(handle)
    }

    #[inline]
    pub fn flags_for_owned_handle(&self, handle: SymbolHandle) -> SymbolFlags {
        self.store.flags_for_owned_handle(handle)
    }

    pub fn check_flags(&self, handle: SymbolHandle) -> CheckFlags {
        self.store.check_flags(handle)
    }

    #[inline]
    pub fn check_flags_for_owned_handle(&self, handle: SymbolHandle) -> CheckFlags {
        self.store.check_flags_for_owned_handle(handle)
    }

    pub fn name(&self, handle: SymbolHandle) -> &SymbolName {
        self.store.name(handle)
    }

    #[inline]
    pub fn name_for_owned_handle(&self, handle: SymbolHandle) -> &SymbolName {
        self.store.name_for_owned_handle(handle)
    }

    fn declarations(&self, handle: SymbolHandle) -> &[Node] {
        self.store.declarations(handle)
    }

    pub fn with_declarations<R>(&self, handle: SymbolHandle, f: impl FnOnce(&[Node]) -> R) -> R {
        f(self.declarations(handle))
    }

    pub fn share_declarations(&self, handle: SymbolHandle) -> SymbolDeclarations {
        self.store.share_declarations(handle)
    }

    #[inline]
    pub fn with_declarations_for_owned_handle<R>(
        &self,
        handle: SymbolHandle,
        f: impl FnOnce(&[Node]) -> R,
    ) -> R {
        f(self.store.declarations_for_owned_handle(handle))
    }

    #[inline]
    pub fn share_declarations_for_owned_handle(&self, handle: SymbolHandle) -> SymbolDeclarations {
        self.store.share_declarations_for_owned_handle(handle)
    }

    #[inline]
    pub fn first_declaration_for_owned_handle(&self, handle: SymbolHandle) -> Option<Node> {
        self.store.first_declaration_for_owned_handle(handle)
    }

    pub fn value_declaration(&self, handle: SymbolHandle) -> Option<Node> {
        self.store.value_declaration(handle)
    }

    #[inline]
    pub fn value_declaration_for_owned_handle(&self, handle: SymbolHandle) -> Option<Node> {
        self.store.value_declaration_for_owned_handle(handle)
    }

    #[inline]
    pub fn value_declaration_snapshot_for_owned_handle(
        &self,
        handle: SymbolHandle,
    ) -> SymbolValueDeclarationSnapshot {
        self.store
            .value_declaration_snapshot_for_owned_handle(handle)
    }

    fn members(&self, handle: SymbolHandle) -> Option<&SymbolHandleTable> {
        self.store.members(handle)
    }

    fn exports(&self, handle: SymbolHandle) -> Option<&SymbolHandleTable> {
        self.store.exports(handle)
    }

    pub fn with_members<R>(
        &self,
        handle: SymbolHandle,
        f: impl FnOnce(Option<&SymbolHandleTable>) -> R,
    ) -> R {
        f(self.members(handle))
    }

    #[inline]
    pub fn with_members_for_owned_handle<R>(
        &self,
        handle: SymbolHandle,
        f: impl FnOnce(Option<&SymbolHandleTable>) -> R,
    ) -> R {
        f(self.store.members_for_owned_handle(handle))
    }

    pub fn with_exports<R>(
        &self,
        handle: SymbolHandle,
        f: impl FnOnce(Option<&SymbolHandleTable>) -> R,
    ) -> R {
        f(self.exports(handle))
    }

    #[inline]
    pub fn with_exports_for_owned_handle<R>(
        &self,
        handle: SymbolHandle,
        f: impl FnOnce(Option<&SymbolHandleTable>) -> R,
    ) -> R {
        f(self.store.exports_for_owned_handle(handle))
    }

    pub fn lookup_member(&self, handle: SymbolHandle, name: &str) -> Option<SymbolHandle> {
        self.store.lookup_member(handle, name)
    }

    pub fn lookup_export(&self, handle: SymbolHandle, name: &str) -> Option<SymbolHandle> {
        self.store.lookup_export(handle, name)
    }

    pub fn parent(&self, handle: SymbolHandle) -> Option<SymbolHandle> {
        self.store.parent(handle)
    }

    #[inline]
    pub fn parent_for_owned_handle(&self, handle: SymbolHandle) -> Option<SymbolHandle> {
        self.store.parent_for_owned_handle(handle)
    }

    #[inline]
    pub fn instantiation_header_for_owned_handle(
        &self,
        handle: SymbolHandle,
    ) -> SymbolInstantiationHeader {
        self.store.instantiation_header_for_owned_handle(handle)
    }

    #[inline]
    pub fn instantiation_snapshot_for_owned_handle(
        &self,
        handle: SymbolHandle,
    ) -> SymbolInstantiationSnapshot {
        self.store.instantiation_snapshot_for_owned_handle(handle)
    }

    pub fn export_symbol(&self, handle: SymbolHandle) -> Option<SymbolHandle> {
        self.store.export_symbol(handle)
    }

    #[inline]
    pub fn export_symbol_for_owned_handle(&self, handle: SymbolHandle) -> Option<SymbolHandle> {
        self.store.export_symbol_for_owned_handle(handle)
    }

    pub fn create_binding_symbol(
        &mut self,
        flags: SymbolFlags,
        name: impl Into<SymbolName>,
    ) -> SymbolHandle {
        self.store.create_symbol(flags, name)
    }

    pub fn add_binding_flags(&mut self, handle: SymbolHandle, flags: SymbolFlags) {
        self.store.add_flags(handle, flags);
    }

    pub fn remove_binding_flags(&mut self, handle: SymbolHandle, flags: SymbolFlags) {
        self.store.remove_flags(handle, flags);
    }

    pub fn set_binding_declarations(
        &mut self,
        handle: SymbolHandle,
        declarations: impl Into<SymbolDeclarations>,
    ) {
        self.store.set_declarations(handle, declarations);
    }

    pub fn add_binding_declaration(&mut self, handle: SymbolHandle, declaration: Node) {
        self.store.add_declaration(handle, declaration);
    }

    pub fn add_binding_declaration_if_unique(&mut self, handle: SymbolHandle, declaration: Node) {
        self.store.add_declaration_if_unique(handle, declaration);
    }

    pub fn set_binding_value_declaration(
        &mut self,
        handle: SymbolHandle,
        value_declaration: Option<Node>,
    ) {
        self.store.set_value_declaration(handle, value_declaration);
    }

    pub fn set_binding_parent(&mut self, handle: SymbolHandle, parent: Option<SymbolHandle>) {
        self.store.set_parent(handle, parent);
    }

    pub fn set_binding_export_symbol(
        &mut self,
        handle: SymbolHandle,
        export_symbol: Option<SymbolHandle>,
    ) {
        self.store.set_export_symbol(handle, export_symbol);
    }

    pub fn ensure_binding_members(&mut self, handle: SymbolHandle) {
        self.store.require_mut(handle).ensure_members();
    }

    pub fn ensure_binding_exports(&mut self, handle: SymbolHandle) {
        self.store.require_mut(handle).ensure_exports();
    }

    pub fn insert_member(
        &mut self,
        handle: SymbolHandle,
        name: impl Into<SymbolName>,
        member: SymbolHandle,
    ) -> Option<SymbolHandle> {
        self.store.insert_member(handle, name, member)
    }

    pub fn insert_export(
        &mut self,
        handle: SymbolHandle,
        name: impl Into<SymbolName>,
        export: SymbolHandle,
    ) -> Option<SymbolHandle> {
        self.store.insert_export(handle, name, export)
    }
}

impl Default for ProgramSymbolStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TransientSymbolStore {
    pub fn new() -> Self {
        Self {
            store: SymbolStore::checker_transient(),
        }
    }

    pub fn owns(&self, handle: SymbolHandle) -> bool {
        self.store.owns(handle)
    }

    #[inline]
    pub fn index_for(&self, handle: SymbolHandle) -> Option<usize> {
        self.store.index_for(handle)
    }

    #[inline]
    pub fn index_for_owned_handle(&self, handle: SymbolHandle) -> usize {
        self.store.index_for_owned_handle(handle)
    }

    pub fn symbol_id(&self, handle: SymbolHandle) -> SymbolId {
        self.store.symbol_id(handle)
    }

    pub fn private_identifier_symbol_name(
        &self,
        handle: SymbolHandle,
        description: &str,
    ) -> String {
        self.store
            .private_identifier_symbol_name(handle, description)
    }

    pub fn unique_es_symbol_type_name(&self, handle: SymbolHandle, symbol_name: &str) -> String {
        self.store.unique_es_symbol_type_name(handle, symbol_name)
    }

    pub fn flags(&self, handle: SymbolHandle) -> SymbolFlags {
        self.store.flags(handle)
    }

    #[inline]
    pub fn flags_for_owned_handle(&self, handle: SymbolHandle) -> SymbolFlags {
        self.store.flags_for_owned_handle(handle)
    }

    pub fn check_flags(&self, handle: SymbolHandle) -> CheckFlags {
        self.store.check_flags(handle)
    }

    #[inline]
    pub fn check_flags_for_owned_handle(&self, handle: SymbolHandle) -> CheckFlags {
        self.store.check_flags_for_owned_handle(handle)
    }

    pub fn name(&self, handle: SymbolHandle) -> &SymbolName {
        self.store.name(handle)
    }

    #[inline]
    pub fn name_for_owned_handle(&self, handle: SymbolHandle) -> &SymbolName {
        self.store.name_for_owned_handle(handle)
    }

    fn declarations(&self, handle: SymbolHandle) -> &[Node] {
        self.store.declarations(handle)
    }

    pub fn with_declarations<R>(&self, handle: SymbolHandle, f: impl FnOnce(&[Node]) -> R) -> R {
        f(self.declarations(handle))
    }

    pub fn share_declarations(&self, handle: SymbolHandle) -> SymbolDeclarations {
        self.store.share_declarations(handle)
    }

    #[inline]
    pub fn with_declarations_for_owned_handle<R>(
        &self,
        handle: SymbolHandle,
        f: impl FnOnce(&[Node]) -> R,
    ) -> R {
        f(self.store.declarations_for_owned_handle(handle))
    }

    #[inline]
    pub fn share_declarations_for_owned_handle(&self, handle: SymbolHandle) -> SymbolDeclarations {
        self.store.share_declarations_for_owned_handle(handle)
    }

    #[inline]
    pub fn first_declaration_for_owned_handle(&self, handle: SymbolHandle) -> Option<Node> {
        self.store.first_declaration_for_owned_handle(handle)
    }

    pub fn value_declaration(&self, handle: SymbolHandle) -> Option<Node> {
        self.store.value_declaration(handle)
    }

    #[inline]
    pub fn value_declaration_for_owned_handle(&self, handle: SymbolHandle) -> Option<Node> {
        self.store.value_declaration_for_owned_handle(handle)
    }

    #[inline]
    pub fn value_declaration_snapshot_for_owned_handle(
        &self,
        handle: SymbolHandle,
    ) -> SymbolValueDeclarationSnapshot {
        self.store
            .value_declaration_snapshot_for_owned_handle(handle)
    }

    fn members(&self, handle: SymbolHandle) -> Option<&SymbolHandleTable> {
        self.store.members(handle)
    }

    fn exports(&self, handle: SymbolHandle) -> Option<&SymbolHandleTable> {
        self.store.exports(handle)
    }

    pub fn with_members<R>(
        &self,
        handle: SymbolHandle,
        f: impl FnOnce(Option<&SymbolHandleTable>) -> R,
    ) -> R {
        f(self.members(handle))
    }

    #[inline]
    pub fn with_members_for_owned_handle<R>(
        &self,
        handle: SymbolHandle,
        f: impl FnOnce(Option<&SymbolHandleTable>) -> R,
    ) -> R {
        f(self.store.members_for_owned_handle(handle))
    }

    pub fn with_exports<R>(
        &self,
        handle: SymbolHandle,
        f: impl FnOnce(Option<&SymbolHandleTable>) -> R,
    ) -> R {
        f(self.exports(handle))
    }

    #[inline]
    pub fn with_exports_for_owned_handle<R>(
        &self,
        handle: SymbolHandle,
        f: impl FnOnce(Option<&SymbolHandleTable>) -> R,
    ) -> R {
        f(self.store.exports_for_owned_handle(handle))
    }

    pub fn lookup_member(&self, handle: SymbolHandle, name: &str) -> Option<SymbolHandle> {
        self.store.lookup_member(handle, name)
    }

    pub fn lookup_export(&self, handle: SymbolHandle, name: &str) -> Option<SymbolHandle> {
        self.store.lookup_export(handle, name)
    }

    pub fn parent(&self, handle: SymbolHandle) -> Option<SymbolHandle> {
        self.store.parent(handle)
    }

    #[inline]
    pub fn parent_for_owned_handle(&self, handle: SymbolHandle) -> Option<SymbolHandle> {
        self.store.parent_for_owned_handle(handle)
    }

    #[inline]
    pub fn instantiation_header_for_owned_handle(
        &self,
        handle: SymbolHandle,
    ) -> SymbolInstantiationHeader {
        self.store.instantiation_header_for_owned_handle(handle)
    }

    #[inline]
    pub fn instantiation_snapshot_for_owned_handle(
        &self,
        handle: SymbolHandle,
    ) -> SymbolInstantiationSnapshot {
        self.store.instantiation_snapshot_for_owned_handle(handle)
    }

    pub fn export_symbol(&self, handle: SymbolHandle) -> Option<SymbolHandle> {
        self.store.export_symbol(handle)
    }

    #[inline]
    pub fn export_symbol_for_owned_handle(&self, handle: SymbolHandle) -> Option<SymbolHandle> {
        self.store.export_symbol_for_owned_handle(handle)
    }

    pub fn insert_member(
        &mut self,
        handle: SymbolHandle,
        name: impl Into<SymbolName>,
        symbol: SymbolHandle,
    ) -> Option<SymbolHandle> {
        self.store.insert_member(handle, name, symbol)
    }

    pub fn insert_export(
        &mut self,
        handle: SymbolHandle,
        name: impl Into<SymbolName>,
        symbol: SymbolHandle,
    ) -> Option<SymbolHandle> {
        self.store.insert_export(handle, name, symbol)
    }

    pub fn create_transient_symbol(
        &mut self,
        flags: SymbolFlags,
        name: impl Into<SymbolName>,
    ) -> SymbolHandle {
        self.store.create_symbol(flags, name)
    }

    pub fn create_transient_symbol_with_check_flags(
        &mut self,
        flags: SymbolFlags,
        name: impl Into<SymbolName>,
        check_flags: CheckFlags,
    ) -> SymbolHandle {
        self.store
            .create_symbol_with_check_flags(flags, name, check_flags)
    }

    pub fn create_transient_symbol_from_instantiation(
        &mut self,
        flags: SymbolFlags,
        name: impl Into<SymbolName>,
        check_flags: CheckFlags,
        declarations: SymbolDeclarations,
        value_declaration: Option<Node>,
        parent: Option<SymbolHandle>,
    ) -> SymbolHandle {
        self.store.create_symbol_from_instantiation(
            flags,
            name,
            check_flags,
            declarations,
            value_declaration,
            parent,
        )
    }

    pub fn add_transient_flags(&mut self, handle: SymbolHandle, flags: SymbolFlags) {
        self.store.add_flags(handle, flags);
    }

    pub fn remove_transient_flags(&mut self, handle: SymbolHandle, flags: SymbolFlags) {
        self.store.remove_flags(handle, flags);
    }

    pub fn set_transient_check_flags(&mut self, handle: SymbolHandle, check_flags: CheckFlags) {
        self.store.set_check_flags(handle, check_flags);
    }

    pub fn add_transient_check_flags(&mut self, handle: SymbolHandle, check_flags: CheckFlags) {
        self.store.add_check_flags(handle, check_flags);
    }

    pub fn set_transient_declarations(
        &mut self,
        handle: SymbolHandle,
        declarations: impl Into<SymbolDeclarations>,
    ) {
        self.store.set_declarations(handle, declarations);
    }

    pub fn add_transient_declaration(&mut self, handle: SymbolHandle, declaration: Node) {
        self.store.add_declaration(handle, declaration);
    }

    pub fn set_transient_value_declaration(
        &mut self,
        handle: SymbolHandle,
        value_declaration: Option<Node>,
    ) {
        self.store.set_value_declaration(handle, value_declaration);
    }

    pub fn set_transient_parent(&mut self, handle: SymbolHandle, parent: Option<SymbolHandle>) {
        self.store.set_parent(handle, parent);
    }

    pub fn set_transient_members(
        &mut self,
        handle: SymbolHandle,
        members: Option<SymbolHandleTable>,
    ) {
        self.store.set_members(handle, members);
    }

    pub fn set_transient_exports(
        &mut self,
        handle: SymbolHandle,
        exports: Option<SymbolHandleTable>,
    ) {
        self.store.set_exports(handle, exports);
    }

    pub fn set_transient_export_symbol(
        &mut self,
        handle: SymbolHandle,
        export_symbol: Option<SymbolHandle>,
    ) {
        self.store.set_export_symbol(handle, export_symbol);
    }
}

impl Default for TransientSymbolStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_store_get_rejects_handle_from_different_owner_with_same_domain() {
        let mut first = ProgramSymbolStore::new();
        let mut second = ProgramSymbolStore::new();

        let first_handle = first.create_binding_symbol(SYMBOL_FLAGS_NONE, "first");
        let second_handle = second.create_binding_symbol(SYMBOL_FLAGS_NONE, "second");

        assert_ne!(first.store.owner, second.store.owner);
        assert_eq!(
            (
                first.store.get(first_handle).is_some(),
                second.store.get(second_handle).is_some(),
                first.store.get(second_handle).is_none(),
                second.store.get(first_handle).is_none(),
            ),
            (true, true, true, true),
        );
    }

    #[test]
    fn symbol_store_get_rejects_handle_from_different_domain_with_same_owner() {
        let owner = SymbolOwnerId::fresh();
        let mut program = SymbolStore::with_owner(SymbolDomain::Program, owner);
        let mut transient = SymbolStore::with_owner(SymbolDomain::CheckerTransient, owner);

        let program_handle = program.create_symbol(SYMBOL_FLAGS_NONE, "program");
        let transient_handle = transient.create_symbol(SYMBOL_FLAGS_NONE, "transient");

        assert_eq!(program.owner, transient.owner);
        assert_ne!(program_handle.domain(), transient_handle.domain());
        assert!(program.get(program_handle).is_some());
        assert!(transient.get(transient_handle).is_some());
        assert!(program.get(transient_handle).is_none());
        assert!(transient.get(program_handle).is_none());
    }

    #[test]
    fn symbol_store_owner_key_routes_only_own_handles() {
        let mut program = ProgramSymbolStore::new();
        let mut other_program = ProgramSymbolStore::new();

        let program_handle = program.create_binding_symbol(SYMBOL_FLAGS_NONE, "program");
        let other_program_handle = other_program.create_binding_symbol(SYMBOL_FLAGS_NONE, "other");

        assert_eq!(
            program_handle.program_owner_key(),
            Some(program.owner_key())
        );
        assert_eq!(
            other_program_handle.program_owner_key(),
            Some(other_program.owner_key())
        );
        assert_ne!(program.owner_key(), other_program.owner_key());
        assert!(program.owns(program_handle));
        assert!(!program.owns(other_program_handle));
    }

    #[test]
    fn symbol_store_helpers_mutate_binder_fields() {
        let mut store = ProgramSymbolStore::new();
        let symbol = store.create_binding_symbol(SYMBOL_FLAGS_FUNCTION, "symbol");
        let parent = store.create_binding_symbol(SYMBOL_FLAGS_MODULE, "parent");
        let exported = store.create_binding_symbol(SYMBOL_FLAGS_EXPORT_VALUE, "exported");

        assert_eq!(store.flags(symbol), SYMBOL_FLAGS_FUNCTION);
        store.add_binding_flags(symbol, SYMBOL_FLAGS_CLASS);
        assert_eq!(
            store.flags(symbol),
            SYMBOL_FLAGS_FUNCTION | SYMBOL_FLAGS_CLASS,
        );
        store.remove_binding_flags(symbol, SYMBOL_FLAGS_FUNCTION);
        assert_eq!(store.flags(symbol), SYMBOL_FLAGS_CLASS);

        store.set_binding_parent(symbol, Some(parent));
        assert_eq!(store.parent(symbol), Some(parent));
        store.set_binding_export_symbol(symbol, Some(exported));
        assert_eq!(store.export_symbol(symbol), Some(exported));
    }

    #[test]
    fn symbol_store_preserves_lazy_members_and_exports_semantics() {
        let mut store = ProgramSymbolStore::new();
        let symbol = store.create_binding_symbol(SYMBOL_FLAGS_MODULE, "module");
        let member = store.create_binding_symbol(SYMBOL_FLAGS_PROPERTY, "member");
        let exported = store.create_binding_symbol(SYMBOL_FLAGS_ALIAS, "exported");

        assert!(store.members(symbol).is_none());
        assert!(store.exports(symbol).is_none());

        assert_eq!(store.insert_member(symbol, "member", member), None);
        assert_eq!(store.insert_export(symbol, "exported", exported), None);
        assert_eq!(store.lookup_member(symbol, "member"), Some(member));
        assert_eq!(store.lookup_export(symbol, "exported"), Some(exported));
    }

    #[test]
    fn shared_symbol_declarations_copy_on_write_when_instantiation_is_mutated() {
        let mut factory = NodeFactory::default();
        let first_declaration = factory.new_identifier("first");
        let second_declaration = factory.new_identifier("second");

        let mut program_symbols = ProgramSymbolStore::new();
        let source = program_symbols.create_binding_symbol(SYMBOL_FLAGS_PROPERTY, "source");
        program_symbols.set_binding_declarations(source, vec![first_declaration]);

        let declarations = program_symbols.share_declarations_for_owned_handle(source);
        let mut transient_symbols = TransientSymbolStore::new();
        let instantiated = transient_symbols.create_transient_symbol_from_instantiation(
            SYMBOL_FLAGS_PROPERTY,
            "source",
            CHECK_FLAGS_INSTANTIATED,
            declarations,
            Some(first_declaration),
            None,
        );

        transient_symbols.add_transient_declaration(instantiated, second_declaration);

        program_symbols.with_declarations_for_owned_handle(source, |declarations| {
            assert_eq!(declarations, &[first_declaration]);
        });
        transient_symbols.with_declarations_for_owned_handle(instantiated, |declarations| {
            assert_eq!(declarations, &[first_declaration, second_declaration]);
        });
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SymbolIdentity(SymbolHandle);

impl SymbolIdentity {
    pub const fn from_symbol_handle(handle: SymbolHandle) -> Self {
        Self(handle)
    }

    pub const fn symbol_handle(self) -> SymbolHandle {
        self.0
    }
}

impl ts_core::IntoLinkKey<SymbolIdentity> for SymbolHandle {
    fn into_link_key(self) -> SymbolIdentity {
        SymbolIdentity::from_symbol_handle(self)
    }
}

impl ts_core::IntoLinkKey<SymbolIdentity> for &SymbolHandle {
    fn into_link_key(self) -> SymbolIdentity {
        SymbolIdentity::from_symbol_handle(*self)
    }
}

pub type SymbolName = SmolStr;

// Go uses "\xFE", an invalid UTF-8 byte that cannot be represented as a Rust str.
pub const INTERNAL_SYMBOL_NAME_PREFIX: &str = "\u{00fe}";

pub const INTERNAL_SYMBOL_NAME_CALL: &str = "\u{00fe}call"; // Call signatures
pub const INTERNAL_SYMBOL_NAME_CONSTRUCTOR: &str = "\u{00fe}constructor"; // Constructor implementations
pub const INTERNAL_SYMBOL_NAME_NEW: &str = "\u{00fe}new"; // Constructor signatures
pub const INTERNAL_SYMBOL_NAME_INDEX: &str = "\u{00fe}index"; // Index signatures
pub const INTERNAL_SYMBOL_NAME_EXPORT_STAR: &str = "\u{00fe}export"; // Module export * declarations
pub const INTERNAL_SYMBOL_NAME_GLOBAL: &str = "\u{00fe}global"; // Global self-reference
pub const INTERNAL_SYMBOL_NAME_MISSING: &str = "\u{00fe}missing"; // Indicates missing symbol
pub const INTERNAL_SYMBOL_NAME_TYPE: &str = "\u{00fe}type"; // Anonymous type literal symbol
pub const INTERNAL_SYMBOL_NAME_OBJECT: &str = "\u{00fe}object"; // Anonymous object literal declaration
pub const INTERNAL_SYMBOL_NAME_JSX_ATTRIBUTES: &str = "\u{00fe}jsxAttributes"; // Anonymous JSX attributes object literal declaration
pub const INTERNAL_SYMBOL_NAME_CLASS: &str = "\u{00fe}class"; // Unnamed class expression
pub const INTERNAL_SYMBOL_NAME_FUNCTION: &str = "\u{00fe}function"; // Unnamed function expression
pub const INTERNAL_SYMBOL_NAME_COMPUTED: &str = "\u{00fe}computed"; // Computed property name declaration with dynamic name
pub const INTERNAL_SYMBOL_NAME_ASSIGNMENT_DECLARATION: &str = "\u{00fe}assignment"; // Assignment declarations
pub const INTERNAL_SYMBOL_NAME_INSTANTIATION_EXPRESSION: &str = "\u{00fe}instantiationExpression"; // Instantiation expressions
pub const INTERNAL_SYMBOL_NAME_IMPORT_ATTRIBUTES: &str = "\u{00fe}importAttributes";
pub const INTERNAL_SYMBOL_NAME_EXPORT_EQUALS: &str = "export="; // Export assignment symbol
pub const INTERNAL_SYMBOL_NAME_DEFAULT: &str = "default"; // Default export symbol (technically not wholly internal, but included here for usability)
pub const INTERNAL_SYMBOL_NAME_THIS: &str = "this";
pub const INTERNAL_SYMBOL_NAME_MODULE_EXPORTS: &str = "module.exports";

// EscapeAllInternalSymbolNames replaces internal symbol name markers ("\xFE") with "__".
pub fn escape_all_internal_symbol_names(name: &str) -> String {
    name.replace(INTERNAL_SYMBOL_NAME_PREFIX, "__")
}
