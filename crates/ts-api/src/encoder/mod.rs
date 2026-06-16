#[cfg(test)]
mod encoder_test;
mod stringtable;
#[cfg(test)]
mod testmain_test;

use std::{cell::RefCell, collections::HashMap, ops::ControlFlow};

use ts_ast::{
    self as ast, Kind, Node, PositionMap, SourceFile, SourceNodeList, compute_position_map,
};
use ts_core as core;

pub const NODE_OFFSET_KIND: usize = 0;
pub const NODE_OFFSET_POS: usize = 4;
pub const NODE_OFFSET_END: usize = 8;
pub const NODE_OFFSET_NEXT: usize = 12;
pub const NODE_OFFSET_PARENT: usize = 16;
pub const NODE_OFFSET_DATA: usize = 20;
pub const NODE_OFFSET_FLAGS: usize = 24;
pub const NODE_SIZE: usize = 28;

pub const NODE_DATA_TYPE_CHILDREN: u32 = 0 << 30;
pub const NODE_DATA_TYPE_STRING: u32 = 1 << 30;
pub const NODE_DATA_TYPE_EXTENDED_DATA: u32 = 2 << 30;

pub const NODE_DATA_TYPE_MASK: u32 = 0xc0_00_00_00;
pub const NODE_DATA_CHILD_MASK: u32 = 0x00_00_00_ff;
pub const NODE_DATA_STRING_INDEX_MASK: u32 = 0x00_ff_ff_ff;

pub const SYNTAX_KIND_NODE_LIST: u32 = u32::MAX;

pub const HEADER_OFFSET_METADATA: usize = 0;
pub const HEADER_OFFSET_HASH_LO0: usize = 4;
pub const HEADER_OFFSET_HASH_LO1: usize = 8;
pub const HEADER_OFFSET_HASH_HI0: usize = 12;
pub const HEADER_OFFSET_HASH_HI1: usize = 16;
pub const HEADER_OFFSET_PARSE_OPTIONS: usize = 20;
pub const HEADER_OFFSET_STRING_OFFSETS: usize = 24;
pub const HEADER_OFFSET_STRING_DATA: usize = 28;
pub const HEADER_OFFSET_EXTENDED_DATA: usize = 32;
pub const HEADER_OFFSET_STRUCTURED_DATA: usize = 36;
pub const HEADER_OFFSET_NODES: usize = 40;
pub const HEADER_SIZE: usize = 44;

pub const PROTOCOL_VERSION: u8 = 5;

const NO_STRUCTURED_DATA: u32 = 0xffff_ffff;

pub fn source_file_hash(source_file: &SourceFile) -> String {
    let hash = source_file.hash();
    format!("{:016x}{:016x}", (hash >> 64) as u64, hash as u64)
}

fn encode_parse_options(opts: ast::ExternalModuleIndicatorOptions) -> u32 {
    let mut bits = 0;
    if opts.jsx {
        bits |= 1;
    }
    if opts.force {
        bits |= 2;
    }
    bits
}

pub fn encode_source_file(source_file: &SourceFile) -> Result<Vec<u8>, String> {
    let root = source_file.as_node();
    encode_tree(root, source_file.store(), Some(source_file))
}

pub fn encode_node(
    node: Node,
    store: &ast::AstStore,
    source_file: Option<&SourceFile>,
) -> Result<Vec<u8>, String> {
    encode_tree(node, store, source_file)
}

pub fn decode_nodes(data: Vec<u8>) -> Result<Node, String> {
    let decoder = AstDecoder::new(data)?;
    decoder.decode()
}

struct AstDecoder {
    raw: Vec<u8>,
    str_table: u32,
    str_data: u32,
    ext_data: u32,
    node_off: u32,
    node_count: usize,
}

impl AstDecoder {
    fn new(data: Vec<u8>) -> Result<Self, String> {
        if data.len() < HEADER_SIZE {
            return Err(format!("data too short for header: {} bytes", data.len()));
        }
        let version = data[HEADER_OFFSET_METADATA + 3];
        if version != PROTOCOL_VERSION {
            return Err(format!(
                "unsupported protocol version {version} (expected {PROTOCOL_VERSION})"
            ));
        }

        let str_table = read_le32(&data, HEADER_OFFSET_STRING_OFFSETS);
        let str_data = read_le32(&data, HEADER_OFFSET_STRING_DATA);
        let ext_data = read_le32(&data, HEADER_OFFSET_EXTENDED_DATA);
        let node_off = read_le32(&data, HEADER_OFFSET_NODES);
        let data_len = data.len() as u32;

        if str_table > data_len || str_data > data_len || ext_data > data_len || node_off > data_len
        {
            return Err(format!(
                "invalid AST header offsets: offsets exceed data length ({data_len})"
            ));
        }
        if !(str_table <= str_data && str_data <= ext_data && ext_data <= node_off) {
            return Err(format!(
                "invalid AST header offsets: expected strTable <= strData <= extData <= nodeOff (got {str_table}, {str_data}, {ext_data}, {node_off})"
            ));
        }

        Ok(Self {
            node_count: (data.len() - node_off as usize) / NODE_SIZE,
            raw: data,
            str_table,
            str_data,
            ext_data,
            node_off,
        })
    }

    fn decode(&self) -> Result<Node, String> {
        if self.node_count < 2 {
            return Err("no nodes to decode".to_string());
        }
        self.create_node(1)
    }

    fn node_field(&self, index: usize, field: usize) -> u32 {
        read_le32(
            &self.raw,
            self.node_off as usize + index * NODE_SIZE + field,
        )
    }

    fn get_string(&self, index: u32) -> Result<String, String> {
        let off_base = self.str_table as usize + index as usize * 4;
        let start = read_le32(&self.raw, off_base) as usize;
        let end = read_le32(&self.raw, off_base + 4) as usize;
        let base = self.str_data as usize;
        let start = base + start;
        let end = base + end;
        let bytes = self
            .raw
            .get(start..end)
            .ok_or_else(|| format!("string table entry {index} is out of bounds"))?;
        String::from_utf8(bytes.to_vec())
            .map_err(|err| format!("string table entry {index} is not UTF-8: {err}"))
    }

    fn create_node(&self, index: usize) -> Result<Node, String> {
        let kind = kind_from_u32(self.node_field(index, NODE_OFFSET_KIND));
        let data = self.node_field(index, NODE_OFFSET_DATA);
        let data_type = data & NODE_DATA_TYPE_MASK;
        match data_type {
            NODE_DATA_TYPE_STRING => self.create_string_node(kind, data),
            NODE_DATA_TYPE_EXTENDED_DATA => self.create_extended_node(kind, data),
            _ => self.create_token_node(kind),
        }
        .map_err(|err| format!("at node {index} (kind {kind:?}): {err}"))
    }

    fn create_string_node(&self, kind: Kind, data: u32) -> Result<Node, String> {
        let text = self.get_string(data & NODE_DATA_STRING_INDEX_MASK)?;
        let mut factory = ast::new_node_factory(ast::NodeFactoryHooks::default());
        match kind {
            Kind::Identifier => Ok(factory.new_identifier(text)),
            Kind::PrivateIdentifier => Ok(factory.new_private_identifier(text)),
            Kind::JsxText => Ok(factory.new_jsx_text(text, false)),
            _ => Err(format!("unknown string node kind {kind:?}")),
        }
    }

    fn create_extended_node(&self, kind: Kind, data: u32) -> Result<Node, String> {
        let ext_off = self.ext_data as usize + (data & NODE_DATA_STRING_INDEX_MASK) as usize;
        let text_index = read_le32(&self.raw, ext_off);
        let flags = ast::TokenFlags(read_le32(&self.raw, ext_off + 4) as i32);
        let mut factory = ast::new_node_factory(ast::NodeFactoryHooks::default());
        match kind {
            Kind::SourceFile => self.decode_source_file(data),
            Kind::StringLiteral => {
                Ok(factory.new_string_literal(self.get_string(text_index)?, flags))
            }
            Kind::NumericLiteral => {
                Ok(factory.new_numeric_literal(self.get_string(text_index)?, flags))
            }
            Kind::BigIntLiteral => {
                Ok(factory.new_big_int_literal(self.get_string(text_index)?, flags))
            }
            Kind::RegularExpressionLiteral => {
                Ok(factory.new_regular_expression_literal(self.get_string(text_index)?, flags))
            }
            Kind::NoSubstitutionTemplateLiteral => {
                Ok(factory
                    .new_no_substitution_template_literal(self.get_string(text_index)?, flags))
            }
            Kind::TemplateHead => Ok(factory.new_template_head(
                self.get_string(text_index)?,
                self.get_string(read_le32(&self.raw, ext_off + 4))?,
                ast::TokenFlags(read_le32(&self.raw, ext_off + 8) as i32),
            )),
            Kind::TemplateMiddle => Ok(factory.new_template_middle(
                self.get_string(text_index)?,
                self.get_string(read_le32(&self.raw, ext_off + 4))?,
                ast::TokenFlags(read_le32(&self.raw, ext_off + 8) as i32),
            )),
            Kind::TemplateTail => Ok(factory.new_template_tail(
                self.get_string(text_index)?,
                self.get_string(read_le32(&self.raw, ext_off + 4))?,
                ast::TokenFlags(read_le32(&self.raw, ext_off + 8) as i32),
            )),
            _ => Err(format!("unknown extended data node kind {kind:?}")),
        }
    }

    fn decode_source_file(&self, data: u32) -> Result<Node, String> {
        let ext_off = self.ext_data as usize + (data & NODE_DATA_STRING_INDEX_MASK) as usize;
        let text = self.get_string(read_le32(&self.raw, ext_off))?;
        let file_name = self.get_string(read_le32(&self.raw, ext_off + 4))?;
        let path = self.get_string(read_le32(&self.raw, ext_off + 8))?;
        let parse_opts = read_le32(&self.raw, HEADER_OFFSET_PARSE_OPTIONS);
        let mut factory = ast::new_node_factory(ast::NodeFactoryHooks::default());
        let statements = factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            Vec::new(),
        );
        let eof = factory.new_token(Kind::EndOfFile);
        Ok(factory.new_source_file(
            ast::SourceFileParseOptions {
                file_name,
                path,
                external_module_indicator_options: ast::ExternalModuleIndicatorOptions {
                    jsx: parse_opts & 1 != 0,
                    force: parse_opts & 2 != 0,
                },
                ..Default::default()
            },
            text,
            statements,
            Some(eof),
        ))
    }

    fn create_token_node(&self, kind: Kind) -> Result<Node, String> {
        let mut factory = ast::new_node_factory(ast::NodeFactoryHooks::default());
        Ok(factory.new_token(kind))
    }
}

fn read_le32(data: &[u8], offset: usize) -> u32 {
    let Some(bytes) = data.get(offset..offset + 4) else {
        return 0;
    };
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn kind_from_u32(value: u32) -> Kind {
    if value > Kind::Count as u32 {
        return Kind::Unknown;
    }
    let mut kind = Kind::Unknown;
    for _ in 0..value {
        kind = kind.next();
    }
    kind
}

fn encode_tree(
    root_node: Node,
    store: &ast::AstStore,
    source_file: Option<&SourceFile>,
) -> Result<Vec<u8>, String> {
    let mut encoder = TreeEncoder::new(root_node, store, source_file);
    encoder.encode_root(root_node, source_file);
    Ok(encoder.finish(root_node, source_file))
}

struct TreeEncoder<'a> {
    parent_index: u32,
    node_count: u32,
    prev_index: u32,
    extended_data: Vec<u8>,
    structured_data: Vec<u8>,
    strs: EncoderStringTable,
    position_map: PositionMap,
    nodes: Vec<u8>,
    node_index_map: HashMap<Node, u32>,
    sf_extended_data_offset: usize,
    store: &'a ast::AstStore,
    source_file: Option<&'a SourceFile>,
}

impl<'a> TreeEncoder<'a> {
    fn new(root_node: Node, store: &'a ast::AstStore, source_file: Option<&'a SourceFile>) -> Self {
        let (strs, position_map) = if store.kind(root_node) == Kind::SourceFile {
            let source_file = source_file.expect("SourceFile root requires source file data");
            (
                EncoderStringTable::new(
                    source_file.text().to_string(),
                    source_file.text_count() as usize,
                ),
                source_file.get_position_map(),
            )
        } else if let Some(source_file) = source_file {
            (
                EncoderStringTable::new(String::new(), 0),
                source_file.get_position_map(),
            )
        } else {
            (
                EncoderStringTable::new(String::new(), 0),
                compute_position_map(""),
            )
        };

        let initial_node_count =
            source_file.map_or(0, |source_file| source_file.node_count() as usize);
        let mut node_index_map = HashMap::new();
        if store.kind(root_node) == Kind::SourceFile {
            let sf = store.as_source_file(root_node);
            for imp in sf.imports() {
                node_index_map.insert(*imp, 0);
            }
            for aug in sf.module_augmentations() {
                node_index_map.insert(*aug, 0);
            }
            if let Some(external_module_indicator) = sf.external_module_indicator() {
                if external_module_indicator != root_node {
                    node_index_map.insert(external_module_indicator, 0);
                }
            }
        }

        Self {
            parent_index: 0,
            node_count: 0,
            prev_index: 0,
            extended_data: Vec::new(),
            structured_data: Vec::new(),
            strs,
            position_map,
            nodes: Vec::with_capacity((initial_node_count + 1) * NODE_SIZE),
            node_index_map,
            sf_extended_data_offset: 0,
            store,
            source_file,
        }
    }

    fn utf16(&self, pos: i32) -> u32 {
        self.position_map.utf8_to_utf16(pos) as u32
    }

    fn encode_root(&mut self, root_node: Node, _source_file: Option<&SourceFile>) {
        append_u32s(&mut self.nodes, &[0, 0, 0, 0, 0, 0, 0]);

        self.node_count += 1;
        self.parent_index += 1;

        self.sf_extended_data_offset = self.extended_data.len();
        let node_data = self.get_node_data(&root_node);
        // PORT NOTE: reshaped for borrowck; Go computes these values while appending.
        let loc = self.store.loc(root_node);
        let pos = self.utf16(loc.pos());
        let end = self.utf16(loc.end());
        append_u32s(
            &mut self.nodes,
            &[
                self.store.kind(root_node) as u32,
                pos,
                end,
                0,
                0,
                node_data,
                self.store.flags(root_node).bits(),
            ],
        );

        self.visit_each_child(&root_node);
    }

    fn visit_node_list(&mut self, node_list: SourceNodeList<'_>) {
        self.node_count += 1;
        self.patch_prev_next();

        // PORT NOTE: reshaped for borrowck; Go computes these inline while appending.
        let pos = self.utf16(node_list.loc().pos());
        let end = self.utf16(node_list.loc().end());
        let parent_index = self.parent_index;
        let node_list_len = node_list.len() as u32;

        append_u32s(
            &mut self.nodes,
            &[
                SYNTAX_KIND_NODE_LIST,
                pos,
                end,
                0,
                parent_index,
                node_list_len,
                0,
            ],
        );

        let save_parent_index = self.parent_index;
        let current_index = self.node_count;
        self.prev_index = 0;
        self.parent_index = current_index;
        for node in node_list {
            self.visit_node(&node);
        }
        self.prev_index = current_index;
        self.parent_index = save_parent_index;
    }

    fn visit_node(&mut self, node: &Node) {
        self.node_count += 1;
        self.patch_prev_next();

        let node_data = self.get_node_data(node);
        // PORT NOTE: reshaped for borrowck; Go computes these values while appending.
        let loc = self.store.loc(*node);
        let pos = self.utf16(loc.pos());
        let end = self.utf16(loc.end());
        append_u32s(
            &mut self.nodes,
            &[
                self.store.kind(*node) as u32,
                pos,
                end,
                0,
                self.parent_index,
                node_data,
                self.store.flags(*node).bits(),
            ],
        );

        if let Some(index) = self.node_index_map.get_mut(node) {
            *index = self.node_count;
        }

        let save_parent_index = self.parent_index;
        let current_index = self.node_count;
        self.prev_index = 0;
        self.parent_index = current_index;
        self.visit_each_child(node);
        self.prev_index = current_index;
        self.parent_index = save_parent_index;
    }

    fn visit_each_child(&mut self, node: &Node) {
        let store = self.store;
        let encoder = RefCell::new(self);
        let _ = store.visit_each_child_with_lists(
            *node,
            |node| {
                if let Some(node) = node {
                    encoder.borrow_mut().visit_node(&node);
                }
                ControlFlow::Continue(())
            },
            |node_list| {
                encoder.borrow_mut().visit_node_list(node_list);
                ControlFlow::Continue(())
            },
            |modifiers| {
                let node_list = modifiers.nodes();
                if !node_list.is_empty() {
                    encoder.borrow_mut().visit_node_list(node_list);
                }
                ControlFlow::Continue(())
            },
        );
    }

    fn patch_prev_next(&mut self) {
        if self.prev_index != 0 {
            let offset = self.prev_index as usize * NODE_SIZE + NODE_OFFSET_NEXT;
            self.nodes[offset..offset + 4].copy_from_slice(&self.node_count.to_le_bytes());
        }
    }

    fn get_node_data(&mut self, node: &Node) -> u32 {
        let t = get_node_data_type(self.store, node);
        match t {
            NODE_DATA_TYPE_CHILDREN => {
                t | get_node_common_data(self.store, node)
                    | get_children_property_mask(self.store, node) as u32
            }
            NODE_DATA_TYPE_STRING => {
                t | get_node_common_data(self.store, node) | self.record_node_strings(node)
            }
            NODE_DATA_TYPE_EXTENDED_DATA => {
                t | get_node_common_data(self.store, node) | self.record_extended_data(node)
            }
            _ => panic!("unreachable"),
        }
    }

    fn record_node_strings(&mut self, node: &Node) -> u32 {
        let store = self.store;
        let kind = store.kind(*node);
        let loc = store.loc(*node);
        match kind {
            Kind::Identifier | Kind::PrivateIdentifier | Kind::JsxText => {
                self.strs
                    .add(&store.text(*node), kind, loc.pos(), loc.end())
            }
            _ => panic!("Unexpected node kind {:?}", kind),
        }
    }

    fn record_extended_data(&mut self, node: &Node) -> u32 {
        let store = self.store;
        let offset = self.extended_data.len() as u32;
        let kind = store.kind(*node);
        let loc = store.loc(*node);
        match kind {
            Kind::StringLiteral => {
                let text_index = self
                    .strs
                    .add(&store.text(*node), kind, loc.pos(), loc.end());
                append_u32s(
                    &mut self.extended_data,
                    &[text_index, store.token_flags(*node).unwrap().bits()],
                );
            }
            Kind::NumericLiteral => {
                let text_index = self
                    .strs
                    .add(&store.text(*node), kind, loc.pos(), loc.end());
                append_u32s(
                    &mut self.extended_data,
                    &[text_index, store.token_flags(*node).unwrap().bits()],
                );
            }
            Kind::BigIntLiteral => {
                let text_index = self
                    .strs
                    .add(&store.text(*node), kind, loc.pos(), loc.end());
                append_u32s(
                    &mut self.extended_data,
                    &[text_index, store.token_flags(*node).unwrap().bits()],
                );
            }
            Kind::RegularExpressionLiteral => {
                let text_index = self
                    .strs
                    .add(&store.text(*node), kind, loc.pos(), loc.end());
                append_u32s(
                    &mut self.extended_data,
                    &[text_index, store.token_flags(*node).unwrap().bits()],
                );
            }
            Kind::NoSubstitutionTemplateLiteral => {
                let text_index = self
                    .strs
                    .add(&store.text(*node), kind, loc.pos(), loc.end());
                append_u32s(
                    &mut self.extended_data,
                    &[text_index, store.template_flags(*node).unwrap().bits()],
                );
            }
            Kind::TemplateHead => self.record_template_literal_like(
                node,
                &store.text(*node),
                &store.raw_text(*node).unwrap_or_default(),
                store.template_flags(*node).unwrap().bits(),
            ),
            Kind::TemplateMiddle => self.record_template_literal_like(
                node,
                &store.text(*node),
                &store.raw_text(*node).unwrap_or_default(),
                store.template_flags(*node).unwrap().bits(),
            ),
            Kind::TemplateTail => self.record_template_literal_like(
                node,
                &store.text(*node),
                &store.raw_text(*node).unwrap_or_default(),
                store.template_flags(*node).unwrap().bits(),
            ),
            Kind::SourceFile => self.record_extended_data_source_file(),
            _ => panic!("unknown extended data node kind {:?}", kind),
        }
        offset
    }

    fn record_template_literal_like(
        &mut self,
        node: &Node,
        text: &str,
        raw_text: &str,
        template_flags: u32,
    ) {
        let kind = self.store.kind(*node);
        let loc = self.store.loc(*node);
        let text_index = self.strs.add(text, kind, loc.pos(), loc.end());
        let raw_text_index = self.strs.add(raw_text, kind, loc.pos(), loc.end());
        append_u32s(
            &mut self.extended_data,
            &[text_index, raw_text_index, template_flags],
        );
    }

    fn record_extended_data_source_file(&mut self) {
        let sf = self
            .source_file
            .expect("SourceFile extended data requires source file data");
        let sf_node = sf.as_node();
        let sf_loc = sf.store().loc(sf_node);
        let text_index = self.strs.add(
            sf.text(),
            sf.store().kind(sf_node),
            sf_loc.pos(),
            sf_loc.end(),
        );
        let file_name_index = self.strs.add(&sf.file_name(), Kind::Unknown, 0, 0);
        let path_index = self.strs.add(&sf.path().to_string(), Kind::Unknown, 0, 0);
        let referenced_files_offset = encode_file_references(
            sf.referenced_files(),
            &self.position_map,
            &mut self.structured_data,
        );
        let type_ref_directives_offset = encode_file_references(
            sf.type_reference_directives(),
            &self.position_map,
            &mut self.structured_data,
        );
        let lib_ref_directives_offset = encode_file_references(
            sf.lib_reference_directives(),
            &self.position_map,
            &mut self.structured_data,
        );
        append_u32s(
            &mut self.extended_data,
            &[
                text_index,
                file_name_index,
                path_index,
                sf.language_variant().0 as u32,
                sf.script_kind().0 as u32,
                referenced_files_offset,
                type_ref_directives_offset,
                lib_ref_directives_offset,
                NO_STRUCTURED_DATA,
                NO_STRUCTURED_DATA,
                NO_STRUCTURED_DATA,
                0,
            ],
        );
    }

    fn finish(mut self, root_node: Node, source_file: Option<&SourceFile>) -> Vec<u8> {
        let store = self.store;
        let mut hash = 0u128;
        let mut parse_opts = 0;
        if store.kind(root_node) == Kind::SourceFile {
            let sf_for_metadata = source_file.expect("SourceFile root requires source file data");
            let sf = store.as_source_file(root_node);
            hash = sf_for_metadata.hash();
            parse_opts = encode_parse_options(
                sf_for_metadata
                    .parse_options()
                    .external_module_indicator_options,
            );

            let imports_offset = encode_node_index_array(
                sf.imports(),
                &self.node_index_map,
                &mut self.structured_data,
            );
            let module_augmentations_offset = encode_module_augmentations(
                sf.module_augmentations(),
                &self.node_index_map,
                &mut self.structured_data,
            );
            let ambient_module_names_offset =
                encode_string_array(sf.ambient_module_names(), &mut self.structured_data);
            self.extended_data
                [self.sf_extended_data_offset + 32..self.sf_extended_data_offset + 36]
                .copy_from_slice(&imports_offset.to_le_bytes());
            self.extended_data
                [self.sf_extended_data_offset + 36..self.sf_extended_data_offset + 40]
                .copy_from_slice(&module_augmentations_offset.to_le_bytes());
            self.extended_data
                [self.sf_extended_data_offset + 40..self.sf_extended_data_offset + 44]
                .copy_from_slice(&ambient_module_names_offset.to_le_bytes());

            let external_module_indicator_index =
                sf.external_module_indicator().map_or(0, |indicator| {
                    if indicator == root_node {
                        1
                    } else {
                        *self.node_index_map.get(&indicator).unwrap_or(&0)
                    }
                });
            self.extended_data
                [self.sf_extended_data_offset + 44..self.sf_extended_data_offset + 48]
                .copy_from_slice(&external_module_indicator_index.to_le_bytes());
        }

        let metadata = (PROTOCOL_VERSION as u32) << 24;
        let offset_string_table_offsets = HEADER_SIZE;
        let offset_string_table_data = HEADER_SIZE + self.strs.offsets.len() * 4;
        let offset_extended_data = offset_string_table_data + self.strs.string_length();
        let offset_structured_data = offset_extended_data + self.extended_data.len();
        let offset_nodes = offset_structured_data + self.structured_data.len();

        let header = [
            metadata,
            hash as u32,
            (hash >> 32) as u32,
            (hash >> 64) as u32,
            (hash >> 96) as u32,
            parse_opts,
            offset_string_table_offsets as u32,
            offset_string_table_data as u32,
            offset_extended_data as u32,
            offset_structured_data as u32,
            offset_nodes as u32,
        ];

        let mut result = Vec::with_capacity(offset_nodes + self.nodes.len());
        append_u32s(&mut result, &header);
        result.extend(self.strs.encode());
        result.extend(self.extended_data);
        result.extend(self.structured_data);
        result.extend(self.nodes);
        result
    }
}

fn get_node_data_type(store: &ast::AstStore, node: &Node) -> u32 {
    match store.kind(*node) {
        Kind::Identifier | Kind::PrivateIdentifier | Kind::JsxText => NODE_DATA_TYPE_STRING,
        Kind::StringLiteral
        | Kind::NumericLiteral
        | Kind::BigIntLiteral
        | Kind::RegularExpressionLiteral
        | Kind::NoSubstitutionTemplateLiteral
        | Kind::TemplateHead
        | Kind::TemplateMiddle
        | Kind::TemplateTail
        | Kind::SourceFile => NODE_DATA_TYPE_EXTENDED_DATA,
        _ => NODE_DATA_TYPE_CHILDREN,
    }
}

fn get_children_property_mask(store: &ast::AstStore, node: &Node) -> u8 {
    match store.kind(*node) {
        Kind::QualifiedName => {
            b(store.left(*node).is_some()) << 0 | b(store.right(*node).is_some()) << 1
        }
        Kind::ComputedPropertyName => b(store.expression(*node).is_some()) << 0,
        Kind::Decorator => b(store.expression(*node).is_some()) << 0,
        Kind::IfStatement => {
            b(store.expression(*node).is_some()) << 0
                | b(store.then_statement(*node).is_some()) << 1
                | b(store.else_statement(*node).is_some()) << 2
        }
        Kind::DoStatement => {
            b(store.statement(*node).is_some()) << 0 | b(store.expression(*node).is_some()) << 1
        }
        Kind::WhileStatement => {
            b(store.expression(*node).is_some()) << 0 | b(store.statement(*node).is_some()) << 1
        }
        Kind::ForStatement => {
            b(store.initializer(*node).is_some()) << 0
                | b(store.condition(*node).is_some()) << 1
                | b(store.incrementor(*node).is_some()) << 2
                | b(store.statement(*node).is_some()) << 3
        }
        Kind::ForInStatement | Kind::ForOfStatement => {
            b(store.await_modifier(*node).is_some()) << 0
                | b(store.initializer(*node).is_some()) << 1
                | b(store.expression(*node).is_some()) << 2
                | b(store.statement(*node).is_some()) << 3
        }
        Kind::BreakStatement => b(store.label(*node).is_some()) << 0,
        Kind::ContinueStatement => b(store.label(*node).is_some()) << 0,
        Kind::ReturnStatement => b(store.expression(*node).is_some()) << 0,
        Kind::WithStatement => {
            b(store.expression(*node).is_some()) << 0 | b(store.statement(*node).is_some()) << 1
        }
        Kind::SwitchStatement => {
            b(store.expression(*node).is_some()) << 0 | b(store.case_block(*node).is_some()) << 1
        }
        Kind::CaseBlock => b(store.clauses(*node).is_some()) << 0,
        Kind::CaseClause | Kind::DefaultClause => {
            b(store.expression(*node).is_some()) << 0 | b(store.statements(*node).is_some()) << 1
        }
        Kind::ThrowStatement => b(store.expression(*node).is_some()) << 0,
        Kind::TryStatement => {
            b(store.try_block(*node).is_some()) << 0
                | b(store.catch_clause(*node).is_some()) << 1
                | b(store.finally_block(*node).is_some()) << 2
        }
        Kind::CatchClause => {
            b(store.variable_declaration(*node).is_some()) << 0
                | b(store.block(*node).is_some()) << 1
        }
        Kind::LabeledStatement => {
            b(store.label(*node).is_some()) << 0 | b(store.statement(*node).is_some()) << 1
        }
        Kind::ExpressionStatement => b(store.expression(*node).is_some()) << 0,
        Kind::Block => b(store.statements(*node).is_some()) << 0,
        Kind::VariableStatement => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.declaration_list(*node).is_some()) << 1
        }
        Kind::VariableDeclaration => {
            b(store.name(*node).is_some()) << 0
                | b(store.exclamation_token(*node).is_some()) << 1
                | b(store.r#type(*node).is_some()) << 2
                | b(store.initializer(*node).is_some()) << 3
        }
        Kind::VariableDeclarationList => b(store.declarations(*node).is_some()) << 0,
        Kind::ObjectBindingPattern | Kind::ArrayBindingPattern => {
            b(store.elements(*node).is_some()) << 0
        }
        Kind::Parameter => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.dot_dot_dot_token(*node).is_some()) << 1
                | b(store.name(*node).is_some()) << 2
                | b(store.question_token(*node).is_some()) << 3
                | b(store.r#type(*node).is_some()) << 4
                | b(store.initializer(*node).is_some()) << 5
        }
        Kind::BindingElement => {
            b(store.dot_dot_dot_token(*node).is_some()) << 0
                | b(store.property_name(*node).is_some()) << 1
                | b(store.name(*node).is_some()) << 2
                | b(store.initializer(*node).is_some()) << 3
        }
        Kind::MissingDeclaration => b(has_modifiers(store.source_modifiers(*node))) << 0,
        Kind::FunctionDeclaration => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.asterisk_token(*node).is_some()) << 1
                | b(store.name(*node).is_some()) << 2
                | b(store.type_parameters(*node).is_some()) << 3
                | b(store.parameters(*node).is_some()) << 4
                | b(store.r#type(*node).is_some()) << 5
                | b(store.body(*node).is_some()) << 6
        }
        Kind::ClassDeclaration => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.name(*node).is_some()) << 1
                | b(store.type_parameters(*node).is_some()) << 2
                | b(store.heritage_clauses(*node).is_some()) << 3
                | b(store.members(*node).is_some()) << 4
        }
        Kind::ClassExpression => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.name(*node).is_some()) << 1
                | b(store.type_parameters(*node).is_some()) << 2
                | b(store.heritage_clauses(*node).is_some()) << 3
                | b(store.members(*node).is_some()) << 4
        }
        Kind::HeritageClause => b(store.types(*node).is_some()) << 0,
        Kind::InterfaceDeclaration => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.name(*node).is_some()) << 1
                | b(store.type_parameters(*node).is_some()) << 2
                | b(store.heritage_clauses(*node).is_some()) << 3
                | b(store.members(*node).is_some()) << 4
        }
        Kind::TypeAliasDeclaration | Kind::JSTypeAliasDeclaration => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.name(*node).is_some()) << 1
                | b(store.type_parameters(*node).is_some()) << 2
                | b(store.r#type(*node).is_some()) << 3
        }
        Kind::EnumMember => {
            b(store.name(*node).is_some()) << 0 | b(store.initializer(*node).is_some()) << 1
        }
        Kind::EnumDeclaration => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.name(*node).is_some()) << 1
                | b(store.members(*node).is_some()) << 2
        }
        Kind::ModuleBlock => b(store.statements(*node).is_some()) << 0,
        Kind::ImportDeclaration | Kind::JSImportDeclaration => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.import_clause(*node).is_some()) << 1
                | b(store.module_specifier(*node).is_some()) << 2
                | b(store.attributes(*node).is_some()) << 3
        }
        Kind::ExternalModuleReference => b(store.expression(*node).is_some()) << 0,
        Kind::NamespaceImport => b(store.name(*node).is_some()) << 0,
        Kind::NamedImports => b(store.elements(*node).is_some()) << 0,
        Kind::ExportAssignment => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.r#type(*node).is_some()) << 1
                | b(store.expression(*node).is_some()) << 2
        }
        Kind::NamespaceExportDeclaration => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.name(*node).is_some()) << 1
        }
        Kind::NamespaceExport => b(store.name(*node).is_some()) << 0,
        Kind::NamedExports => b(store.elements(*node).is_some()) << 0,
        Kind::ExportSpecifier => {
            b(store.property_name(*node).is_some()) << 0 | b(store.name(*node).is_some()) << 1
        }
        Kind::CallSignature => {
            b(store.type_parameters(*node).is_some()) << 0
                | b(store.parameters(*node).is_some()) << 1
                | b(store.r#type(*node).is_some()) << 2
        }
        Kind::ConstructSignature => {
            b(store.type_parameters(*node).is_some()) << 0
                | b(store.parameters(*node).is_some()) << 1
                | b(store.r#type(*node).is_some()) << 2
        }
        Kind::Constructor => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.type_parameters(*node).is_some()) << 1
                | b(store.parameters(*node).is_some()) << 2
                | b(store.r#type(*node).is_some()) << 3
                | b(store.body(*node).is_some()) << 4
        }
        Kind::GetAccessor => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.name(*node).is_some()) << 1
                | b(store.type_parameters(*node).is_some()) << 2
                | b(store.parameters(*node).is_some()) << 3
                | b(store.r#type(*node).is_some()) << 4
                | b(store.body(*node).is_some()) << 5
        }
        Kind::SetAccessor => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.name(*node).is_some()) << 1
                | b(store.type_parameters(*node).is_some()) << 2
                | b(store.parameters(*node).is_some()) << 3
                | b(store.r#type(*node).is_some()) << 4
                | b(store.body(*node).is_some()) << 5
        }
        Kind::IndexSignature => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.parameters(*node).is_some()) << 1
                | b(store.r#type(*node).is_some()) << 2
        }
        Kind::MethodSignature => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.name(*node).is_some()) << 1
                | b(store.postfix_token(*node).is_some()) << 2
                | b(store.type_parameters(*node).is_some()) << 3
                | b(store.parameters(*node).is_some()) << 4
                | b(store.r#type(*node).is_some()) << 5
        }
        Kind::MethodDeclaration => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.asterisk_token(*node).is_some()) << 1
                | b(store.name(*node).is_some()) << 2
                | b(store.postfix_token(*node).is_some()) << 3
                | b(store.type_parameters(*node).is_some()) << 4
                | b(store.parameters(*node).is_some()) << 5
                | b(store.r#type(*node).is_some()) << 6
                | b(store.body(*node).is_some()) << 7
        }
        Kind::PropertySignature => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.name(*node).is_some()) << 1
                | b(store.postfix_token(*node).is_some()) << 2
                | b(store.r#type(*node).is_some()) << 3
                | b(store.initializer(*node).is_some()) << 4
        }
        Kind::PropertyDeclaration => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.name(*node).is_some()) << 1
                | b(store.postfix_token(*node).is_some()) << 2
                | b(store.r#type(*node).is_some()) << 3
                | b(store.initializer(*node).is_some()) << 4
        }
        Kind::ClassStaticBlockDeclaration => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.body(*node).is_some()) << 1
        }
        Kind::BinaryExpression => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.left(*node).is_some()) << 1
                | b(store.r#type(*node).is_some()) << 2
                | b(store.operator_token(*node).is_some()) << 3
                | b(store.right(*node).is_some()) << 4
        }
        Kind::PrefixUnaryExpression => b(store.operand(*node).is_some()) << 0,
        Kind::PostfixUnaryExpression => b(store.operand(*node).is_some()) << 0,
        Kind::YieldExpression => {
            b(store.asterisk_token(*node).is_some()) << 0
                | b(store.expression(*node).is_some()) << 1
        }
        Kind::ArrowFunction => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.type_parameters(*node).is_some()) << 1
                | b(store.parameters(*node).is_some()) << 2
                | b(store.r#type(*node).is_some()) << 3
                | b(store.equals_greater_than_token(*node).is_some()) << 4
                | b(store.body(*node).is_some()) << 5
        }
        Kind::FunctionExpression => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.asterisk_token(*node).is_some()) << 1
                | b(store.name(*node).is_some()) << 2
                | b(store.type_parameters(*node).is_some()) << 3
                | b(store.parameters(*node).is_some()) << 4
                | b(store.r#type(*node).is_some()) << 5
                | b(store.body(*node).is_some()) << 6
        }
        Kind::AsExpression => {
            b(store.expression(*node).is_some()) << 0 | b(store.r#type(*node).is_some()) << 1
        }
        Kind::SatisfiesExpression => {
            b(store.expression(*node).is_some()) << 0 | b(store.r#type(*node).is_some()) << 1
        }
        Kind::ConditionalExpression => {
            b(store.condition(*node).is_some()) << 0
                | b(store.question_token(*node).is_some()) << 1
                | b(store.when_true(*node).is_some()) << 2
                | b(store.colon_token(*node).is_some()) << 3
                | b(store.when_false(*node).is_some()) << 4
        }
        Kind::PropertyAccessExpression => {
            b(store.expression(*node).is_some()) << 0
                | b(store.question_dot_token(*node).is_some()) << 1
                | b(store.name(*node).is_some()) << 2
        }
        Kind::ElementAccessExpression => {
            b(store.expression(*node).is_some()) << 0
                | b(store.question_dot_token(*node).is_some()) << 1
                | b(store.argument_expression(*node).is_some()) << 2
        }
        Kind::CallExpression => {
            b(store.expression(*node).is_some()) << 0
                | b(store.question_dot_token(*node).is_some()) << 1
                | b(store.type_arguments(*node).is_some()) << 2
                | b(store.arguments(*node).is_some()) << 3
        }
        Kind::NewExpression => {
            b(store.expression(*node).is_some()) << 0
                | b(store.type_arguments(*node).is_some()) << 1
                | b(store.arguments(*node).is_some()) << 2
        }
        Kind::MetaProperty => b(store.name(*node).is_some()) << 0,
        Kind::NonNullExpression => b(store.expression(*node).is_some()) << 0,
        Kind::SpreadElement => b(store.expression(*node).is_some()) << 0,
        Kind::TemplateExpression => {
            b(store.head(*node).is_some()) << 0 | b(store.template_spans(*node).is_some()) << 1
        }
        Kind::TemplateSpan => {
            b(store.expression(*node).is_some()) << 0 | b(store.literal(*node).is_some()) << 1
        }
        Kind::TaggedTemplateExpression => {
            b(store.tag(*node).is_some()) << 0
                | b(store.question_dot_token(*node).is_some()) << 1
                | b(store.type_arguments(*node).is_some()) << 2
                | b(store.template(*node).is_some()) << 3
        }
        Kind::ParenthesizedExpression => b(store.expression(*node).is_some()) << 0,
        Kind::ArrayLiteralExpression => b(store.elements(*node).is_some()) << 0,
        Kind::ObjectLiteralExpression => b(store.properties(*node).is_some()) << 0,
        Kind::SpreadAssignment => b(store.expression(*node).is_some()) << 0,
        Kind::PropertyAssignment => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.name(*node).is_some()) << 1
                | b(store.postfix_token(*node).is_some()) << 2
                | b(store.r#type(*node).is_some()) << 3
                | b(store.initializer(*node).is_some()) << 4
        }
        Kind::ShorthandPropertyAssignment => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.name(*node).is_some()) << 1
                | b(store.postfix_token(*node).is_some()) << 2
                | b(store.r#type(*node).is_some()) << 3
                | b(store.equals_token(*node).is_some()) << 4
                | b(store.object_assignment_initializer(*node).is_some()) << 5
        }
        Kind::DeleteExpression => b(store.expression(*node).is_some()) << 0,
        Kind::TypeOfExpression => b(store.expression(*node).is_some()) << 0,
        Kind::VoidExpression => b(store.expression(*node).is_some()) << 0,
        Kind::AwaitExpression => b(store.expression(*node).is_some()) << 0,
        Kind::TypeAssertionExpression => {
            b(store.r#type(*node).is_some()) << 0 | b(store.expression(*node).is_some()) << 1
        }
        Kind::UnionType => b(store.types(*node).is_some()) << 0,
        Kind::IntersectionType => b(store.types(*node).is_some()) << 0,
        Kind::ConditionalType => {
            b(store.check_type(*node).is_some()) << 0
                | b(store.extends_type(*node).is_some()) << 1
                | b(store.true_type(*node).is_some()) << 2
                | b(store.false_type(*node).is_some()) << 3
        }
        Kind::TypeOperator => b(store.r#type(*node).is_some()) << 0,
        Kind::InferType => b(store.type_parameter(*node).is_some()) << 0,
        Kind::ArrayType => b(store.element_type(*node).is_some()) << 0,
        Kind::IndexedAccessType => {
            b(store.object_type(*node).is_some()) << 0 | b(store.index_type(*node).is_some()) << 1
        }
        Kind::TypeReference => {
            b(store.type_name(*node).is_some()) << 0 | b(store.type_arguments(*node).is_some()) << 1
        }
        Kind::ExpressionWithTypeArguments => {
            b(store.expression(*node).is_some()) << 0
                | b(store.type_arguments(*node).is_some()) << 1
        }
        Kind::LiteralType => b(store.literal(*node).is_some()) << 0,
        Kind::TypePredicate => {
            b(store.asserts_modifier(*node).is_some()) << 0
                | b(store.parameter_name(*node).is_some()) << 1
                | b(store.r#type(*node).is_some()) << 2
        }
        Kind::ImportAttribute => {
            b(store.name(*node).is_some()) << 0 | b(store.value(*node).is_some()) << 1
        }
        Kind::ImportAttributes => b(store.attributes(*node).is_some()) << 0,
        Kind::TypeQuery => {
            b(store.expr_name(*node).is_some()) << 0 | b(store.type_arguments(*node).is_some()) << 1
        }
        Kind::MappedType => {
            b(store.readonly_token(*node).is_some()) << 0
                | b(store.type_parameter(*node).is_some()) << 1
                | b(store.name_type(*node).is_some()) << 2
                | b(store.question_token(*node).is_some()) << 3
                | b(store.r#type(*node).is_some()) << 4
                | b(store.members(*node).is_some()) << 5
        }
        Kind::TypeLiteral => b(store.members(*node).is_some()) << 0,
        Kind::TupleType => b(store.elements(*node).is_some()) << 0,
        Kind::NamedTupleMember => {
            b(store.dot_dot_dot_token(*node).is_some()) << 0
                | b(store.name(*node).is_some()) << 1
                | b(store.question_token(*node).is_some()) << 2
                | b(store.r#type(*node).is_some()) << 3
        }
        Kind::OptionalType => b(store.r#type(*node).is_some()) << 0,
        Kind::RestType => b(store.r#type(*node).is_some()) << 0,
        Kind::ParenthesizedType => b(store.r#type(*node).is_some()) << 0,
        Kind::FunctionType => {
            b(store.type_parameters(*node).is_some()) << 0
                | b(store.parameters(*node).is_some()) << 1
                | b(store.r#type(*node).is_some()) << 2
        }
        Kind::ConstructorType => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.type_parameters(*node).is_some()) << 1
                | b(store.parameters(*node).is_some()) << 2
                | b(store.r#type(*node).is_some()) << 3
        }
        Kind::TemplateLiteralType => {
            b(store.head(*node).is_some()) << 0 | b(store.template_spans(*node).is_some()) << 1
        }
        Kind::TemplateLiteralTypeSpan => {
            b(store.r#type(*node).is_some()) << 0 | b(store.literal(*node).is_some()) << 1
        }
        Kind::SyntheticExpression => b(store.tuple_name_source(*node).is_some()) << 0,
        Kind::PartiallyEmittedExpression => b(store.expression(*node).is_some()) << 0,
        Kind::JsxElement => {
            b(store.opening_element(*node).is_some()) << 0
                | b(store.children(*node).is_some()) << 1
                | b(store.closing_element(*node).is_some()) << 2
        }
        Kind::JsxAttributes => b(store.properties(*node).is_some()) << 0,
        Kind::JsxNamespacedName => {
            b(store.namespace(*node).is_some()) << 0 | b(store.name(*node).is_some()) << 1
        }
        Kind::JsxOpeningElement => {
            b(store.tag_name(*node).is_some()) << 0
                | b(store.type_arguments(*node).is_some()) << 1
                | b(store.attributes(*node).is_some()) << 2
        }
        Kind::JsxSelfClosingElement => {
            b(store.tag_name(*node).is_some()) << 0
                | b(store.type_arguments(*node).is_some()) << 1
                | b(store.attributes(*node).is_some()) << 2
        }
        Kind::JsxFragment => {
            b(store.opening_fragment(*node).is_some()) << 0
                | b(store.children(*node).is_some()) << 1
                | b(store.closing_fragment(*node).is_some()) << 2
        }
        Kind::JsxAttribute => {
            b(store.name(*node).is_some()) << 0 | b(store.initializer(*node).is_some()) << 1
        }
        Kind::JsxSpreadAttribute => b(store.expression(*node).is_some()) << 0,
        Kind::JsxClosingElement => b(store.tag_name(*node).is_some()) << 0,
        Kind::JsxExpression => {
            b(store.dot_dot_dot_token(*node).is_some()) << 0
                | b(store.expression(*node).is_some()) << 1
        }
        Kind::SyntaxList => {
            b(store
                .children(*node)
                .is_some_and(|children| !children.is_empty()))
                << 0
        }
        Kind::ModuleDeclaration => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.name(*node).is_some()) << 1
                | b(store.body(*node).is_some()) << 2
        }
        Kind::ImportEqualsDeclaration => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.name(*node).is_some()) << 1
                | b(store.module_reference(*node).is_some()) << 2
        }
        Kind::ExportDeclaration => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.export_clause(*node).is_some()) << 1
                | b(store.module_specifier(*node).is_some()) << 2
                | b(store.attributes(*node).is_some()) << 3
        }
        Kind::ImportType => {
            b(store.argument(*node).is_some()) << 0
                | b(store.attributes(*node).is_some()) << 1
                | b(store.qualifier(*node).is_some()) << 2
                | b(store.type_arguments(*node).is_some()) << 3
        }
        Kind::ImportClause => {
            b(store.name(*node).is_some()) << 0 | b(store.named_bindings(*node).is_some()) << 1
        }
        Kind::ImportSpecifier => {
            b(store.property_name(*node).is_some()) << 0 | b(store.name(*node).is_some()) << 1
        }
        Kind::TypeParameter => {
            b(has_modifiers(store.source_modifiers(*node))) << 0
                | b(store.name(*node).is_some()) << 1
                | b(store.constraint(*node).is_some()) << 2
                | b(store.expression(*node).is_some()) << 3
                | b(store.default_type(*node).is_some()) << 4
        }
        Kind::SyntheticReferenceExpression => {
            b(store.expression(*node).is_some()) << 0 | b(store.this_arg(*node).is_some()) << 1
        }
        _ => 0,
    }
}

fn b(value: bool) -> u8 {
    bool_to_byte(value)
}

fn has_modifiers(modifiers: Option<ast::SourceModifierList<'_>>) -> bool {
    modifiers.is_some_and(|modifiers| !modifiers.is_empty())
}

fn get_node_common_data(store: &ast::AstStore, node: &Node) -> u32 {
    match store.kind(*node) {
        Kind::Block => (bool_to_byte(store.multi_line(*node).unwrap_or(false)) as u32) << 24,
        Kind::ExportAssignment => {
            (bool_to_byte(store.is_export_equals(*node).unwrap_or(false)) as u32) << 24
        }
        Kind::ExportSpecifier => {
            (bool_to_byte(store.is_type_only(*node).unwrap_or(false)) as u32) << 24
        }
        Kind::ArrayLiteralExpression => {
            (bool_to_byte(store.multi_line(*node).unwrap_or(false)) as u32) << 24
        }
        Kind::ObjectLiteralExpression => {
            (bool_to_byte(store.multi_line(*node).unwrap_or(false)) as u32) << 24
        }
        Kind::JsxText => {
            (bool_to_byte(
                store
                    .contains_only_trivia_white_spaces(*node)
                    .unwrap_or(false),
            ) as u32)
                << 24
        }
        Kind::ImportEqualsDeclaration => {
            (bool_to_byte(store.is_type_only(*node).unwrap_or(false)) as u32) << 24
        }
        Kind::ExportDeclaration => {
            (bool_to_byte(store.is_type_only(*node).unwrap_or(false)) as u32) << 24
        }
        Kind::ImportType => (bool_to_byte(store.is_type_of(*node).unwrap_or(false)) as u32) << 24,
        Kind::ImportSpecifier => {
            (bool_to_byte(store.is_type_only(*node).unwrap_or(false)) as u32) << 24
        }
        Kind::PrefixUnaryExpression => {
            operator_index(
                store.operator(*node).unwrap(),
                &[
                    Kind::PlusToken,
                    Kind::MinusToken,
                    Kind::TildeToken,
                    Kind::ExclamationToken,
                    Kind::PlusPlusToken,
                    Kind::MinusMinusToken,
                ],
            ) << 24
        }
        Kind::PostfixUnaryExpression => {
            operator_index(
                store.operator(*node).unwrap(),
                &[Kind::PlusPlusToken, Kind::MinusMinusToken],
            ) << 24
        }
        Kind::HeritageClause => {
            operator_index(
                store.token(*node).unwrap(),
                &[Kind::ExtendsKeyword, Kind::ImplementsKeyword],
            ) << 24
        }
        Kind::MetaProperty => {
            operator_index(
                store.keyword_token(*node).unwrap(),
                &[Kind::ImportKeyword, Kind::NewKeyword],
            ) << 24
        }
        Kind::TypeOperator => {
            operator_index(
                store.operator(*node).unwrap(),
                &[
                    Kind::KeyOfKeyword,
                    Kind::ReadonlyKeyword,
                    Kind::UniqueKeyword,
                ],
            ) << 24
        }
        Kind::ImportAttributes => {
            ((bool_to_byte(store.multi_line(*node).unwrap_or(false)) as u32) << 24)
                | (operator_index(
                    store.token(*node).unwrap(),
                    &[Kind::WithKeyword, Kind::AssertKeyword],
                ) << 25)
        }
        Kind::ModuleDeclaration => {
            operator_index(
                store.keyword(*node).unwrap(),
                &[Kind::ModuleKeyword, Kind::NamespaceKeyword],
            ) << 24
        }
        Kind::ImportClause => {
            store
                .phase_modifier(*node)
                .map_or(0, |phase_modifier| match phase_modifier {
                    Kind::TypeKeyword => 1 << 24,
                    Kind::DeferKeyword => 2 << 24,
                    _ => 0,
                })
        }
        Kind::SyntheticExpression => panic!("SyntheticExpression should never be encoded"),
        _ => 0,
    }
}

fn operator_index(value: Kind, values: &[Kind]) -> u32 {
    values
        .iter()
        .position(|candidate| *candidate == value)
        .unwrap_or(0) as u32
}

fn bool_to_byte(value: bool) -> u8 {
    if value { 1 } else { 0 }
}

fn encode_file_references(
    refs: &[ast::FileReference],
    position_map: &PositionMap,
    buf: &mut Vec<u8>,
) -> u32 {
    if refs.is_empty() {
        return NO_STRUCTURED_DATA;
    }
    let offset = buf.len() as u32;
    msgpack_write_array_header(buf, refs.len());
    for ref_ in refs {
        msgpack_write_array_header(buf, 5);
        msgpack_write_uint(
            buf,
            position_map.utf8_to_utf16(ref_.text_range.pos()) as u32,
        );
        msgpack_write_uint(
            buf,
            position_map.utf8_to_utf16(ref_.text_range.end()) as u32,
        );
        msgpack_write_string(buf, &ref_.file_name);
        msgpack_write_uint(buf, ref_.resolution_mode.0 as u32);
        msgpack_write_bool(buf, ref_.preserve);
    }
    offset
}

fn encode_node_index_array(
    nodes: &[ast::Node],
    index_map: &HashMap<Node, u32>,
    buf: &mut Vec<u8>,
) -> u32 {
    if nodes.is_empty() {
        return NO_STRUCTURED_DATA;
    }
    let offset = buf.len() as u32;
    msgpack_write_array_header(buf, nodes.len());
    for node in nodes {
        msgpack_write_uint(buf, *index_map.get(node).unwrap_or(&0));
    }
    offset
}

fn encode_module_augmentations(
    nodes: &[ast::Node],
    index_map: &HashMap<Node, u32>,
    buf: &mut Vec<u8>,
) -> u32 {
    if nodes.is_empty() {
        return NO_STRUCTURED_DATA;
    }
    let offset = buf.len() as u32;
    msgpack_write_array_header(buf, nodes.len());
    for node in nodes {
        msgpack_write_uint(buf, *index_map.get(node).unwrap_or(&0));
    }
    offset
}

fn encode_string_array(strs: &[String], buf: &mut Vec<u8>) -> u32 {
    if strs.is_empty() {
        return NO_STRUCTURED_DATA;
    }
    let offset = buf.len() as u32;
    msgpack_write_array_header(buf, strs.len());
    for s in strs {
        msgpack_write_string(buf, s);
    }
    offset
}

fn msgpack_write_array_header(buf: &mut Vec<u8>, length: usize) {
    if length <= 0x0f {
        buf.push(0x90 | length as u8);
    } else if length <= 0xffff {
        buf.extend_from_slice(&[0xdc, (length >> 8) as u8, length as u8]);
    } else {
        buf.extend_from_slice(&[
            0xdd,
            (length >> 24) as u8,
            (length >> 16) as u8,
            (length >> 8) as u8,
            length as u8,
        ]);
    }
}

fn msgpack_write_uint(buf: &mut Vec<u8>, value: u32) {
    if value <= 0x7f {
        buf.push(value as u8);
    } else if value <= 0xff {
        buf.extend_from_slice(&[0xcc, value as u8]);
    } else if value <= 0xffff {
        buf.extend_from_slice(&[0xcd, (value >> 8) as u8, value as u8]);
    } else {
        buf.extend_from_slice(&[
            0xce,
            (value >> 24) as u8,
            (value >> 16) as u8,
            (value >> 8) as u8,
            value as u8,
        ]);
    }
}

fn msgpack_write_string(buf: &mut Vec<u8>, s: &str) {
    let n = s.len();
    if n <= 0x1f {
        buf.push(0xa0 | n as u8);
    } else if n <= 0xff {
        buf.extend_from_slice(&[0xd9, n as u8]);
    } else if n <= 0xffff {
        buf.extend_from_slice(&[0xda, (n >> 8) as u8, n as u8]);
    } else {
        buf.extend_from_slice(&[
            0xdb,
            (n >> 24) as u8,
            (n >> 16) as u8,
            (n >> 8) as u8,
            n as u8,
        ]);
    }
    buf.extend_from_slice(s.as_bytes());
}

fn msgpack_write_bool(buf: &mut Vec<u8>, value: bool) {
    buf.push(if value { 0xc3 } else { 0xc2 });
}

fn append_u32s(buf: &mut Vec<u8>, values: &[u32]) {
    for value in values {
        buf.extend_from_slice(&value.to_le_bytes());
    }
}

struct EncoderStringTable {
    file_text: String,
    other_strings: String,
    offsets: Vec<u32>,
}

impl EncoderStringTable {
    fn new(file_text: String, string_count: usize) -> Self {
        Self {
            file_text,
            other_strings: String::new(),
            offsets: Vec::with_capacity(string_count * 2),
        }
    }

    fn add(&mut self, text: &str, kind: Kind, pos: i32, end: i32) -> u32 {
        let index = self.offsets.len() as u32;
        if kind == Kind::SourceFile {
            self.offsets.push(pos as u32);
            self.offsets.push(end as u32);
            return index;
        }

        let length = text.len();
        if end - pos > 0 && end as usize <= self.file_text.len() {
            let mut end_offset = 0;
            if kind == Kind::StringLiteral
                || kind == Kind::TemplateTail
                || kind == Kind::NoSubstitutionTemplateLiteral
            {
                end_offset = 1;
            }
            let end = (end - end_offset) as usize;
            let start = end - length;
            if self.file_text.as_bytes()[start..end] == *text.as_bytes() {
                self.offsets.push(start as u32);
                self.offsets.push(end as u32);
                return index;
            }
        }

        let offset = self.file_text.len() + self.other_strings.len();
        self.other_strings.push_str(text);
        self.offsets.push(offset as u32);
        self.offsets.push((offset + length) as u32);
        index
    }

    fn encode(&self) -> Vec<u8> {
        let mut result = Vec::with_capacity(self.encoded_length());
        append_u32s(&mut result, &self.offsets);
        result.extend_from_slice(self.file_text.as_bytes());
        result.extend_from_slice(self.other_strings.as_bytes());
        result
    }

    fn string_length(&self) -> usize {
        self.file_text.len() + self.other_strings.len()
    }

    fn encoded_length(&self) -> usize {
        self.offsets.len() * 4 + self.file_text.len() + self.other_strings.len()
    }
}
