use std::error::Error;
use std::fmt;

use ts_ast as ast;
use ts_astnav as astnav;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as core;
use ts_core::Context;

use crate::LanguageService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    NoSourceFile { file_name: String },
    NoTokenAtPosition { file_name: String, position: i32 },
    NodeOutsideSourceFile { file_name: String },
    CheckerUnavailable { message: String },
    StaleCheckerState { file_name: String },
    UnknownSymbol { file_name: String },
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSourceFile { file_name } => {
                write!(f, "source file not found: {file_name}")
            }
            Self::NoTokenAtPosition {
                file_name,
                position,
            } => {
                write!(f, "no token found at position: {file_name}:{position}")
            }
            Self::NodeOutsideSourceFile { file_name } => {
                write!(f, "node does not belong to source file: {file_name}")
            }
            Self::CheckerUnavailable { message } => write!(f, "{message}"),
            Self::StaleCheckerState { file_name } => {
                write!(
                    f,
                    "checker state is no longer valid for source file: {file_name}"
                )
            }
            Self::UnknownSymbol { file_name } => {
                write!(
                    f,
                    "symbol is not available in checker state for source file: {file_name}"
                )
            }
        }
    }
}

impl Error for ApiError {}

pub const ERR_NO_SOURCE_FILE: &str = "source file not found";
pub const ERR_NO_TOKEN_AT_POSITION: &str = "no token found at position";

#[derive(Clone, Copy)]
pub(crate) struct LanguageServiceNodeLocation<'a> {
    source_file: &'a ast::SourceFile,
    node: ast::Node,
}

impl<'a> LanguageServiceNodeLocation<'a> {
    pub(crate) fn new(source_file: &'a ast::SourceFile, node: ast::Node) -> Result<Self, ApiError> {
        if node.store_id() != source_file.store().store_id() {
            return Err(ApiError::NodeOutsideSourceFile {
                file_name: source_file.file_name().to_string(),
            });
        }

        let source_root = ast::get_source_file_of_node(source_file.store(), Some(node));
        if source_root != Some(source_file.as_node()) {
            return Err(ApiError::NodeOutsideSourceFile {
                file_name: source_file.file_name().to_string(),
            });
        }

        Ok(Self { source_file, node })
    }

    pub(crate) fn source_file(self) -> &'a ast::SourceFile {
        self.source_file
    }

    pub(crate) fn node(self) -> ast::Node {
        self.node
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct LanguageServiceSymbolHandle {
    state_identity: checker::CheckerStateIdentity,
    symbol: ast::SymbolHandle,
    file_name: String,
}

impl LanguageServiceSymbolHandle {
    pub(crate) fn new(
        state_identity: checker::CheckerStateIdentity,
        symbol: ast::SymbolHandle,
        file_name: String,
    ) -> Self {
        Self {
            state_identity,
            symbol,
            file_name,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct LanguageServiceTypeHandle {
    state_identity: checker::CheckerStateIdentity,
    id: checker::TypeId,
    file_name: String,
}

impl LanguageServiceTypeHandle {
    pub(crate) fn new(
        state_identity: checker::CheckerStateIdentity,
        id: checker::TypeId,
        file_name: String,
    ) -> Self {
        Self {
            state_identity,
            id,
            file_name,
        }
    }
}

impl LanguageService<'_> {
    pub fn get_symbol_at_position(
        &self,
        ctx: &Context,
        file_name: &str,
        position: i32,
    ) -> Result<Option<LanguageServiceSymbolHandle>, ApiError> {
        let (program, file) = self.try_get_program_and_file(file_name);
        let file = file.ok_or_else(|| ApiError::NoSourceFile {
            file_name: file_name.to_string(),
        })?;
        let node = astnav::get_token_at_position(file, position);
        let node = node.ok_or_else(|| ApiError::NoTokenAtPosition {
            file_name: file_name.to_string(),
            position,
        })?;
        program
            .with_type_checker_for_file_using(
                compiler::CheckerAccess::context(ctx),
                file,
                |checker| {
                    let state_identity = checker.state_identity();
                    Ok::<_, core::Error>(checker.get_symbol_identity_at_location_public(node).map(
                        |symbol| {
                            LanguageServiceSymbolHandle::new(
                                state_identity,
                                symbol.symbol_handle(),
                                file.file_name().to_string(),
                            )
                        },
                    ))
                },
            )
            .map_err(|err| ApiError::CheckerUnavailable {
                message: err.to_string(),
            })
    }

    pub(crate) fn get_symbol_at_location(
        &self,
        ctx: &Context,
        location: LanguageServiceNodeLocation<'_>,
    ) -> Result<Option<LanguageServiceSymbolHandle>, ApiError> {
        let program = self.get_program();
        let source_file = location.source_file();
        let node = location.node();
        program
            .with_type_checker_for_file_using(
                compiler::CheckerAccess::context(ctx),
                source_file,
                |checker| {
                    let state_identity = checker.state_identity();
                    Ok::<_, core::Error>(checker.get_symbol_identity_at_location_public(node).map(
                        |symbol| {
                            LanguageServiceSymbolHandle::new(
                                state_identity,
                                symbol.symbol_handle(),
                                source_file.file_name().to_string(),
                            )
                        },
                    ))
                },
            )
            .map_err(|err| ApiError::CheckerUnavailable {
                message: err.to_string(),
            })
    }

    pub fn get_type_of_symbol(
        &self,
        ctx: &Context,
        symbol: &LanguageServiceSymbolHandle,
    ) -> Result<LanguageServiceTypeHandle, ApiError> {
        let program = self.get_program();
        let (_, source_file) = self.try_get_program_and_file(&symbol.file_name);
        let source_file = source_file.ok_or_else(|| ApiError::NoSourceFile {
            file_name: symbol.file_name.clone(),
        })?;
        program.with_type_checker_for_file_using(
            compiler::CheckerAccess::context(ctx),
            source_file,
            |checker| {
                if checker.state_identity() != symbol.state_identity {
                    return Err(ApiError::StaleCheckerState {
                        file_name: symbol.file_name.clone(),
                    });
                }

                let symbol_identity = ast::SymbolIdentity::from_symbol_handle(symbol.symbol);
                let id = checker
                    .get_type_id_of_symbol_identity_public(symbol_identity, None)
                    .ok_or_else(|| ApiError::UnknownSymbol {
                        file_name: symbol.file_name.clone(),
                    })?;
                Ok(LanguageServiceTypeHandle::new(
                    symbol.state_identity,
                    id,
                    symbol.file_name.clone(),
                ))
            },
        )
    }
}
