use ts_ast as ast;

// `PseudoType`s are skeletons of types: partially interpreted expressions and
// type nodes that represent how a real checker or node builder should construct
// an actual type later. They are intentionally not normalized.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i16)]
pub enum PseudoTypeKind {
    Direct = 0,
    Inferred = 1,
    NoResult = 2,
    MaybeConstLocation = 3,
    Union = 4,
    Undefined = 5,
    Null = 6,
    Any = 7,
    String = 8,
    Number = 9,
    BigInt = 10,
    Boolean = 11,
    False = 12,
    True = 13,
    SingleCallSignature = 14,
    Tuple = 15,
    ObjectLiteral = 16,
    StringLiteral = 17,
    NumericLiteral = 18,
    BigIntLiteral = 19,
}

#[derive(Clone)]
pub struct PseudoType {
    pub kind: PseudoTypeKind,
    pub data: PseudoTypeData,
}

#[derive(Clone)]
pub enum PseudoTypeData {
    Base,
    Direct(PseudoTypeDirect),
    Inferred(PseudoTypeInferred),
    NoResult(PseudoTypeNoResult),
    MaybeConstLocation(PseudoTypeMaybeConstLocation),
    Union(PseudoTypeUnion),
    SingleCallSignature(PseudoTypeSingleCallSignature),
    Tuple(PseudoTypeTuple),
    ObjectLiteral(PseudoTypeObjectLiteral),
    Literal(PseudoTypeLiteral),
}

fn new_pseudo_type(kind: PseudoTypeKind, data: PseudoTypeData) -> PseudoType {
    PseudoType { kind, data }
}

pub fn pseudo_type_undefined() -> PseudoType {
    new_pseudo_type(PseudoTypeKind::Undefined, PseudoTypeData::Base)
}

pub fn pseudo_type_null() -> PseudoType {
    new_pseudo_type(PseudoTypeKind::Null, PseudoTypeData::Base)
}

pub fn pseudo_type_any() -> PseudoType {
    new_pseudo_type(PseudoTypeKind::Any, PseudoTypeData::Base)
}

pub fn pseudo_type_string() -> PseudoType {
    new_pseudo_type(PseudoTypeKind::String, PseudoTypeData::Base)
}

pub fn pseudo_type_number() -> PseudoType {
    new_pseudo_type(PseudoTypeKind::Number, PseudoTypeData::Base)
}

pub fn pseudo_type_big_int() -> PseudoType {
    new_pseudo_type(PseudoTypeKind::BigInt, PseudoTypeData::Base)
}

pub fn pseudo_type_boolean() -> PseudoType {
    new_pseudo_type(PseudoTypeKind::Boolean, PseudoTypeData::Base)
}

pub fn pseudo_type_false() -> PseudoType {
    new_pseudo_type(PseudoTypeKind::False, PseudoTypeData::Base)
}

pub fn pseudo_type_true() -> PseudoType {
    new_pseudo_type(PseudoTypeKind::True, PseudoTypeData::Base)
}

// PseudoTypeDirect directly encodes the type referred to by a TypeNode.
#[derive(Clone)]
pub struct PseudoTypeDirect {
    pub type_node: ast::Node,
}

pub fn new_pseudo_type_direct(type_node: ast::Node) -> PseudoType {
    new_pseudo_type(
        PseudoTypeKind::Direct,
        PseudoTypeData::Direct(PseudoTypeDirect { type_node }),
    )
}

impl PseudoType {
    pub fn as_pseudo_type_direct(&self) -> &PseudoTypeDirect {
        match &self.data {
            PseudoTypeData::Direct(value) => value,
            _ => panic!("PseudoType is not Direct"),
        }
    }

    pub fn as_pseudo_type_inferred(&self) -> &PseudoTypeInferred {
        match &self.data {
            PseudoTypeData::Inferred(value) => value,
            _ => panic!("PseudoType is not Inferred"),
        }
    }

    pub fn as_pseudo_type_no_result(&self) -> &PseudoTypeNoResult {
        match &self.data {
            PseudoTypeData::NoResult(value) => value,
            _ => panic!("PseudoType is not NoResult"),
        }
    }

    pub fn as_pseudo_type_maybe_const_location(&self) -> &PseudoTypeMaybeConstLocation {
        match &self.data {
            PseudoTypeData::MaybeConstLocation(value) => value,
            _ => panic!("PseudoType is not MaybeConstLocation"),
        }
    }

    pub fn as_pseudo_type_union(&self) -> &PseudoTypeUnion {
        match &self.data {
            PseudoTypeData::Union(value) => value,
            _ => panic!("PseudoType is not Union"),
        }
    }

    pub fn as_pseudo_type_single_call_signature(&self) -> &PseudoTypeSingleCallSignature {
        match &self.data {
            PseudoTypeData::SingleCallSignature(value) => value,
            _ => panic!("PseudoType is not SingleCallSignature"),
        }
    }

    pub fn as_pseudo_type_tuple(&self) -> &PseudoTypeTuple {
        match &self.data {
            PseudoTypeData::Tuple(value) => value,
            _ => panic!("PseudoType is not Tuple"),
        }
    }

    pub fn as_pseudo_type_object_literal(&self) -> &PseudoTypeObjectLiteral {
        match &self.data {
            PseudoTypeData::ObjectLiteral(value) => value,
            _ => panic!("PseudoType is not ObjectLiteral"),
        }
    }

    pub fn as_pseudo_type_literal(&self) -> &PseudoTypeLiteral {
        match &self.data {
            PseudoTypeData::Literal(value) => value,
            _ => panic!("PseudoType is not Literal"),
        }
    }
}

#[derive(Clone)]
pub struct PseudoTypeInferred {
    pub expression: ast::Node,
    pub error_nodes: Vec<ast::Node>,
}

pub fn new_pseudo_type_inferred(expr: ast::Node) -> PseudoType {
    new_pseudo_type(
        PseudoTypeKind::Inferred,
        PseudoTypeData::Inferred(PseudoTypeInferred {
            expression: expr,
            error_nodes: Vec::new(),
        }),
    )
}

pub fn new_pseudo_type_inferred_with_errors(
    expr: ast::Node,
    error_nodes: Vec<ast::Node>,
) -> PseudoType {
    new_pseudo_type(
        PseudoTypeKind::Inferred,
        PseudoTypeData::Inferred(PseudoTypeInferred {
            expression: expr,
            error_nodes,
        }),
    )
}

#[derive(Clone)]
pub struct PseudoTypeNoResult {
    pub declaration: ast::Node,
}

pub fn new_pseudo_type_no_result(decl: ast::Node) -> PseudoType {
    new_pseudo_type(
        PseudoTypeKind::NoResult,
        PseudoTypeData::NoResult(PseudoTypeNoResult { declaration: decl }),
    )
}

#[derive(Clone)]
pub struct PseudoTypeMaybeConstLocation {
    pub node: ast::Node,
    pub const_type: Box<PseudoType>,
    pub regular_type: Box<PseudoType>,
}

pub fn new_pseudo_type_maybe_const_location(
    loc: ast::Node,
    const_type: PseudoType,
    regular_type: PseudoType,
) -> PseudoType {
    new_pseudo_type(
        PseudoTypeKind::MaybeConstLocation,
        PseudoTypeData::MaybeConstLocation(PseudoTypeMaybeConstLocation {
            node: loc,
            const_type: Box::new(const_type),
            regular_type: Box::new(regular_type),
        }),
    )
}

#[derive(Clone)]
pub struct PseudoTypeUnion {
    pub types: Vec<PseudoType>,
}

pub fn new_pseudo_type_union(types: Vec<PseudoType>) -> PseudoType {
    new_pseudo_type(
        PseudoTypeKind::Union,
        PseudoTypeData::Union(PseudoTypeUnion { types }),
    )
}

#[derive(Clone)]
pub struct PseudoParameter {
    pub rest: bool,
    pub name: ast::Node,
    pub optional: bool,
    pub type_: PseudoType,
}

pub fn new_pseudo_parameter(
    is_rest: bool,
    name: ast::Node,
    is_optional: bool,
    type_: PseudoType,
) -> PseudoParameter {
    PseudoParameter {
        rest: is_rest,
        name,
        optional: is_optional,
        type_,
    }
}

#[derive(Clone)]
pub struct PseudoTypeSingleCallSignature {
    pub signature: ast::Node,
    pub parameters: Vec<PseudoParameter>,
    pub type_parameters: Vec<ast::Node>,
    pub return_type: Box<PseudoType>,
}

pub fn new_pseudo_type_single_call_signature(
    signature: ast::Node,
    parameters: Vec<PseudoParameter>,
    type_parameters: Vec<ast::Node>,
    return_type: PseudoType,
) -> PseudoType {
    new_pseudo_type(
        PseudoTypeKind::SingleCallSignature,
        PseudoTypeData::SingleCallSignature(PseudoTypeSingleCallSignature {
            signature,
            parameters,
            type_parameters,
            return_type: Box::new(return_type),
        }),
    )
}

#[derive(Clone)]
pub struct PseudoTypeTuple {
    pub elements: Vec<PseudoType>,
}

pub fn new_pseudo_type_tuple(elements: Vec<PseudoType>) -> PseudoType {
    new_pseudo_type(
        PseudoTypeKind::Tuple,
        PseudoTypeData::Tuple(PseudoTypeTuple { elements }),
    )
}

#[derive(Clone)]
pub struct PseudoObjectElement {
    pub name: ast::Node,
    pub optional: bool,
    pub kind: PseudoObjectElementKind,
    pub data: PseudoObjectElementData,
}

impl PseudoObjectElement {
    pub fn signature(&self) -> Option<ast::Node> {
        match &self.data {
            PseudoObjectElementData::Method(value) => Some(value.signature),
            PseudoObjectElementData::SetAccessor(value) => Some(value.signature),
            PseudoObjectElementData::GetAccessor(value) => Some(value.signature),
            PseudoObjectElementData::PropertyAssignment(_) => None,
        }
    }

    pub fn as_pseudo_object_method(&self) -> &PseudoObjectMethod {
        match &self.data {
            PseudoObjectElementData::Method(value) => value,
            _ => panic!("PseudoObjectElement is not Method"),
        }
    }

    pub fn as_pseudo_property_assignment(&self) -> &PseudoPropertyAssignment {
        match &self.data {
            PseudoObjectElementData::PropertyAssignment(value) => value,
            _ => panic!("PseudoObjectElement is not PropertyAssignment"),
        }
    }

    pub fn as_pseudo_set_accessor(&self) -> &PseudoSetAccessor {
        match &self.data {
            PseudoObjectElementData::SetAccessor(value) => value,
            _ => panic!("PseudoObjectElement is not SetAccessor"),
        }
    }

    pub fn as_pseudo_get_accessor(&self) -> &PseudoGetAccessor {
        match &self.data {
            PseudoObjectElementData::GetAccessor(value) => value,
            _ => panic!("PseudoObjectElement is not GetAccessor"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i8)]
pub enum PseudoObjectElementKind {
    Method = 0,
    PropertyAssignment = 1,
    SetAccessor = 2,
    GetAccessor = 3,
}

#[derive(Clone)]
pub enum PseudoObjectElementData {
    Method(PseudoObjectMethod),
    PropertyAssignment(PseudoPropertyAssignment),
    SetAccessor(PseudoSetAccessor),
    GetAccessor(PseudoGetAccessor),
}

fn new_pseudo_object_element(
    kind: PseudoObjectElementKind,
    name: ast::Node,
    optional: bool,
    data: PseudoObjectElementData,
) -> PseudoObjectElement {
    PseudoObjectElement {
        name,
        optional,
        kind,
        data,
    }
}

#[derive(Clone)]
pub struct PseudoObjectMethod {
    pub signature: ast::Node,
    pub type_parameters: Vec<ast::Node>,
    pub parameters: Vec<PseudoParameter>,
    pub return_type: PseudoType,
}

pub fn new_pseudo_object_method(
    signature: ast::Node,
    name: ast::Node,
    optional: bool,
    type_parameters: Vec<ast::Node>,
    parameters: Vec<PseudoParameter>,
    return_type: PseudoType,
) -> PseudoObjectElement {
    new_pseudo_object_element(
        PseudoObjectElementKind::Method,
        name,
        optional,
        PseudoObjectElementData::Method(PseudoObjectMethod {
            signature,
            type_parameters,
            parameters,
            return_type,
        }),
    )
}

#[derive(Clone)]
pub struct PseudoPropertyAssignment {
    pub readonly: bool,
    pub type_: PseudoType,
}

pub fn new_pseudo_property_assignment(
    readonly: bool,
    name: ast::Node,
    optional: bool,
    type_: PseudoType,
) -> PseudoObjectElement {
    new_pseudo_object_element(
        PseudoObjectElementKind::PropertyAssignment,
        name,
        optional,
        PseudoObjectElementData::PropertyAssignment(PseudoPropertyAssignment { readonly, type_ }),
    )
}

#[derive(Clone)]
pub struct PseudoSetAccessor {
    pub signature: ast::Node,
    pub parameter: PseudoParameter,
}

pub fn new_pseudo_set_accessor(
    signature: ast::Node,
    name: ast::Node,
    optional: bool,
    parameter: PseudoParameter,
) -> PseudoObjectElement {
    new_pseudo_object_element(
        PseudoObjectElementKind::SetAccessor,
        name,
        optional,
        PseudoObjectElementData::SetAccessor(PseudoSetAccessor {
            signature,
            parameter,
        }),
    )
}

#[derive(Clone)]
pub struct PseudoGetAccessor {
    pub signature: ast::Node,
    pub type_: PseudoType,
}

pub fn new_pseudo_get_accessor(
    signature: ast::Node,
    name: ast::Node,
    optional: bool,
    type_: PseudoType,
) -> PseudoObjectElement {
    new_pseudo_object_element(
        PseudoObjectElementKind::GetAccessor,
        name,
        optional,
        PseudoObjectElementData::GetAccessor(PseudoGetAccessor { signature, type_ }),
    )
}

#[derive(Clone)]
pub struct PseudoTypeObjectLiteral {
    pub expression: ast::Node,
    pub elements: Vec<PseudoObjectElement>,
}

pub fn new_pseudo_type_object_literal(
    expression: ast::Node,
    elements: Vec<PseudoObjectElement>,
) -> PseudoType {
    new_pseudo_type(
        PseudoTypeKind::ObjectLiteral,
        PseudoTypeData::ObjectLiteral(PseudoTypeObjectLiteral {
            expression,
            elements,
        }),
    )
}

#[derive(Clone)]
pub struct PseudoTypeLiteral {
    pub node: ast::Node,
}

pub fn new_pseudo_type_string_literal(node: ast::Node) -> PseudoType {
    new_pseudo_type(
        PseudoTypeKind::StringLiteral,
        PseudoTypeData::Literal(PseudoTypeLiteral { node }),
    )
}

pub fn new_pseudo_type_numeric_literal(node: ast::Node) -> PseudoType {
    new_pseudo_type(
        PseudoTypeKind::NumericLiteral,
        PseudoTypeData::Literal(PseudoTypeLiteral { node }),
    )
}

pub fn new_pseudo_type_big_int_literal(node: ast::Node) -> PseudoType {
    new_pseudo_type(
        PseudoTypeKind::BigIntLiteral,
        PseudoTypeData::Literal(PseudoTypeLiteral { node }),
    )
}
