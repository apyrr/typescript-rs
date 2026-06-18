pub type NodeId = u64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceId(u32);

impl SourceId {
    pub const fn from_u32(value: u32) -> Self {
        Self(value)
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for SourceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalAstId(u32);

impl LocalAstId {
    pub const fn from_u32(value: u32) -> Self {
        Self(value)
    }

    pub fn from_usize(value: usize) -> Self {
        Self(u32::try_from(value).expect("local AST id exceeds u32 payload space"))
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for LocalAstId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StableNodeId {
    source_id: SourceId,
    local_id: LocalAstId,
}

impl StableNodeId {
    pub const fn new(source_id: SourceId, local_id: LocalAstId) -> Self {
        Self {
            source_id,
            local_id,
        }
    }

    pub const fn source_id(self) -> SourceId {
        self.source_id
    }

    pub const fn local_id(self) -> LocalAstId {
        self.local_id
    }
}

impl std::fmt::Display for StableNodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.source_id, self.local_id)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceSnapshotId {
    source_id: SourceId,
    source_hash: u128,
}

impl SourceSnapshotId {
    pub const fn new(source_id: SourceId, source_hash: u128) -> Self {
        Self {
            source_id,
            source_hash,
        }
    }

    pub const fn source_id(self) -> SourceId {
        self.source_id
    }

    pub const fn source_hash(self) -> u128 {
        self.source_hash
    }
}

impl std::fmt::Display for SourceSnapshotId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{:032x}", self.source_id, self.source_hash)
    }
}
