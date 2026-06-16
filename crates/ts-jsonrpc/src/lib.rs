#![forbid(unsafe_code)]
use std::{
    error::Error,
    fmt,
    io::{self, BufRead, BufReader, BufWriter, Read, Write},
    sync::Mutex,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

// Base protocol for JSON-RPC with Content-Length headers (as used by LSP).
// https://microsoft.github.io/language-server-protocol/specifications/base/0.9/specification/

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BaseProtocolError {
    InvalidHeader(Vec<u8>),
    InvalidContentLength(String),
    NoContentLength,
}

impl fmt::Display for BaseProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BaseProtocolError::InvalidHeader(line) => {
                write!(f, "jsonrpc: invalid header: {line:?}")
            }
            BaseProtocolError::InvalidContentLength(message) => {
                write!(f, "jsonrpc: invalid content length: {message}")
            }
            BaseProtocolError::NoContentLength => f.write_str("jsonrpc: no content length"),
        }
    }
}

impl Error for BaseProtocolError {}

impl From<BaseProtocolError> for io::Error {
    fn from(value: BaseProtocolError) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, value)
    }
}

// Reader reads JSON-RPC messages with Content-Length framing.
pub struct Reader<R> {
    r: Mutex<BufReader<R>>,
}

impl<R: Read> Reader<R> {
    // NewReader creates a new Reader.
    pub fn new(r: R) -> Self {
        Self {
            r: Mutex::new(BufReader::new(r)),
        }
    }

    // Read reads the next message payload.
    pub fn read(&self) -> io::Result<Vec<u8>> {
        let mut content_length: i64 = 0;
        let mut reader = self
            .r
            .lock()
            .map_err(|_| io::Error::other("jsonrpc reader mutex poisoned"))?;

        loop {
            let mut line = Vec::new();
            let read = reader.read_until(b'\n', &mut line).map_err(|err| {
                io::Error::new(err.kind(), format!("jsonrpc: read header: {err}"))
            })?;
            if read == 0 || !line.ends_with(b"\n") {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "EOF"));
            }

            if line == b"\r\n" {
                break;
            }

            let Some(colon) = line.iter().position(|b| *b == b':') else {
                return Err(BaseProtocolError::InvalidHeader(line).into());
            };
            let (key, value) = line.split_at(colon);
            let value = &value[1..];

            if key == b"Content-Length" {
                let value = String::from_utf8_lossy(trim_ascii_space(value));
                content_length = value.parse::<i64>().map_err(|err| {
                    BaseProtocolError::InvalidContentLength(format!("parse error: {err}"))
                })?;
                if content_length < 0 {
                    return Err(BaseProtocolError::InvalidContentLength(format!(
                        "negative value {content_length}"
                    ))
                    .into());
                }
            }
        }

        if content_length <= 0 {
            return Err(BaseProtocolError::NoContentLength.into());
        }

        let mut data = vec![0u8; content_length as usize];
        let mut offset = 0;
        while offset < data.len() {
            match reader.read(&mut data[offset..]) {
                Ok(0) => {
                    let message = if offset == 0 {
                        "jsonrpc: read content: EOF"
                    } else {
                        "jsonrpc: read content: unexpected EOF"
                    };
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof, message));
                }
                Ok(n) => offset += n,
                Err(err) => {
                    return Err(io::Error::new(
                        err.kind(),
                        format!("jsonrpc: read content: {err}"),
                    ));
                }
            }
        }

        Ok(data)
    }
}

// Writer writes JSON-RPC messages with Content-Length framing.
pub struct Writer<W: Write> {
    w: Mutex<BufWriter<W>>,
}

impl<W: Write> Writer<W> {
    // NewWriter creates a new Writer.
    pub fn new(w: W) -> Self {
        Self {
            w: Mutex::new(BufWriter::new(w)),
        }
    }

    // Write writes a message payload with Content-Length header.
    pub fn write(&self, data: &[u8]) -> io::Result<()> {
        let mut writer = self
            .w
            .lock()
            .map_err(|_| io::Error::other("jsonrpc writer mutex poisoned"))?;
        write!(writer, "Content-Length: {}\r\n\r\n", data.len())?;
        writer.write_all(data)?;
        writer.flush()
    }
}

fn trim_ascii_space(value: &[u8]) -> &[u8] {
    let start = value
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(value.len());
    let end = value
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .map(|i| i + 1)
        .unwrap_or(start);
    &value[start..end]
}

// Package jsonrpc provides generic JSON-RPC 2.0 types and utilities
// that can be shared between LSP and other JSON-RPC based protocols.

// JSONRPCVersion represents the JSON-RPC version field, always "2.0".
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct JsonRpcVersion;

const JSON_RPC_VERSION: &str = "2.0";

impl Serialize for JsonRpcVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(JSON_RPC_VERSION)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidJsonRpcVersion;

impl fmt::Display for InvalidJsonRpcVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid JSON-RPC version")
    }
}

impl Error for InvalidJsonRpcVersion {}

impl<'de> Deserialize<'de> for JsonRpcVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value != JSON_RPC_VERSION {
            return Err(serde::de::Error::custom(InvalidJsonRpcVersion));
        }
        Ok(JsonRpcVersion)
    }
}

// ID represents a JSON-RPC message ID, which can be either a string or integer.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Id {
    str: String,
    int: i32,
}

pub type ID = Id;

// IntegerOrString is a helper type for creating IDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegerOrString {
    pub integer: Option<i32>,
    pub string: Option<String>,
}

impl Id {
    // NewID creates an ID from an IntegerOrString value.
    pub fn new(raw_value: IntegerOrString) -> Self {
        if let Some(string) = raw_value.string {
            return Self {
                str: string,
                int: 0,
            };
        }
        Self {
            str: String::new(),
            int: raw_value.integer.unwrap(),
        }
    }

    // NewIDString creates a string ID.
    pub fn new_string(str: String) -> Self {
        Self { str, int: 0 }
    }

    // NewIDInt creates an integer ID.
    pub fn new_int(i: i32) -> Self {
        Self {
            str: String::new(),
            int: i,
        }
    }

    pub fn try_int(&self) -> Option<i32> {
        if !self.str.is_empty() {
            return None;
        }
        Some(self.int)
    }

    pub fn must_int(&self) -> i32 {
        if !self.str.is_empty() {
            panic!("ID is not an integer");
        }
        self.int
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.str.is_empty() {
            return f.write_str(&self.str);
        }
        write!(f, "{}", self.int)
    }
}

impl Serialize for Id {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if !self.str.is_empty() {
            return serializer.serialize_str(&self.str);
        }
        serializer.serialize_i32(self.int)
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct IdVisitor;

        impl<'de> serde::de::Visitor<'de> for IdVisitor {
            type Value = Id;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a JSON-RPC string or integer id")
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                i32::try_from(v)
                    .map(Id::new_int)
                    .map_err(|_| E::custom("integer id out of int32 range"))
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                i32::try_from(v)
                    .map(Id::new_int)
                    .map_err(|_| E::custom("integer id out of int32 range"))
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Id::new_string(v.to_string()))
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Id::new_string(v))
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Id::new_int(0))
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Id::new_int(0))
            }
        }

        deserializer.deserialize_any(IdVisitor)
    }
}

// ResponseError represents a JSON-RPC error response.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ResponseError {
    pub code: i32,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl fmt::Display for ResponseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(data) = &self.data
            && serde_json::to_string(data).is_err()
        {
            return write!(f, "[{}]: {}\n{}", self.code, self.message, data);
        }
        write!(f, "[{}]: {}", self.code, self.message)
    }
}

impl Error for ResponseError {}

// Standard JSON-RPC error codes.
pub const CODE_PARSE_ERROR: i32 = -32700;
pub const CODE_INVALID_REQUEST: i32 = -32600;
pub const CODE_METHOD_NOT_FOUND: i32 = -32601;
pub const CODE_INVALID_PARAMS: i32 = -32602;
pub const CODE_INTERNAL_ERROR: i32 = -32603;

// MessageKind indicates what type of message this is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageKind {
    Notification,
    Request,
    Response,
}

// Message represents a raw JSON-RPC message that can be a request, notification, or response.
// Unlike lsproto.Message, this keeps params/result as raw JSON for generic handling.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Message {
    #[serde(default, rename = "jsonrpc")]
    pub jsonrpc: JsonRpcVersion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub method: String,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub params: serde_json::Value,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub result: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ResponseError>,
}

impl Message {
    // Kind returns the kind of message this is.
    pub fn kind(&self) -> MessageKind {
        if self.id.is_some() && self.method.is_empty() {
            return MessageKind::Response;
        }
        if self.id.is_none() {
            return MessageKind::Notification;
        }
        MessageKind::Request
    }

    // IsRequest returns true if this message is a request (has ID and method).
    pub fn is_request(&self) -> bool {
        self.id.is_some() && !self.method.is_empty()
    }

    // IsNotification returns true if this message is a notification (has method but no ID).
    pub fn is_notification(&self) -> bool {
        self.id.is_none() && !self.method.is_empty()
    }

    // IsResponse returns true if this message is a response (has ID but no method).
    pub fn is_response(&self) -> bool {
        self.id.is_some() && self.method.is_empty()
    }
}

// RequestMessage is a convenience type for creating request/notification messages.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(bound(
    serialize = "T: Default + PartialEq + Serialize",
    deserialize = "T: Default + Deserialize<'de>"
))]
pub struct RequestMessage<T = serde_json::Value> {
    #[serde(default, rename = "jsonrpc")]
    pub jsonrpc: JsonRpcVersion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
    pub method: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub params: T,
}

// ResponseMessage is a convenience type for creating response messages.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(bound(
    serialize = "T: Default + PartialEq + Serialize",
    deserialize = "T: Default + Deserialize<'de>"
))]
pub struct ResponseMessage<T = serde_json::Value> {
    #[serde(default, rename = "jsonrpc")]
    pub jsonrpc: JsonRpcVersion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub result: T,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ResponseError>,
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    value == &T::default()
}
