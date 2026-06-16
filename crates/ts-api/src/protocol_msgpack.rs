use std::io::{BufReader, BufWriter, Read, Write};
use std::sync::{Arc, Mutex};

use serde::ser::{SerializeMap, Serializer};
use ts_json as json;
use ts_jsonrpc as jsonrpc;

use crate::{Message, Protocol};

// MessageType represents the type of message in the msgpack protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MessageType(pub u8);

impl MessageType {
    pub const UNKNOWN: MessageType = MessageType(0);
    pub const REQUEST: MessageType = MessageType(1);
    pub const CALL_RESPONSE: MessageType = MessageType(2);
    pub const CALL_ERROR: MessageType = MessageType(3);
    pub const RESPONSE: MessageType = MessageType(4);
    pub const ERROR: MessageType = MessageType(5);
    pub const CALL: MessageType = MessageType(6);

    #[allow(non_upper_case_globals)]
    pub const Unknown: MessageType = Self::UNKNOWN;
    #[allow(non_upper_case_globals)]
    pub const Request: MessageType = Self::REQUEST;
    #[allow(non_upper_case_globals)]
    pub const CallResponse: MessageType = Self::CALL_RESPONSE;
    #[allow(non_upper_case_globals)]
    pub const CallError: MessageType = Self::CALL_ERROR;
    #[allow(non_upper_case_globals)]
    pub const Response: MessageType = Self::RESPONSE;
    #[allow(non_upper_case_globals)]
    pub const Error: MessageType = Self::ERROR;
    #[allow(non_upper_case_globals)]
    pub const Call: MessageType = Self::CALL;

    pub fn is_valid(self) -> bool {
        self >= Self::REQUEST && self <= Self::CALL
    }
}

impl From<u8> for MessageType {
    fn from(value: u8) -> Self {
        MessageType(value)
    }
}

// MessagePack format constants
const MSGPACK_FIXED_ARRAY3: u8 = 0x93;
const MSGPACK_BIN8: u8 = 0xC4;
const MSGPACK_BIN16: u8 = 0xC5;
const MSGPACK_BIN32: u8 = 0xC6;
const MSGPACK_U8: u8 = 0xCC;
const RAW_BINARY_MARKER: &str = "__ts_raw_binary";

// MessagePackProtocol implements the Protocol interface using a custom
// msgpack-based tuple format: [MessageType, method, payload].
pub struct MessagePackProtocol<RW: Read + Write> {
    r: Mutex<BufReader<SharedReadWriter<RW>>>,
    w: Mutex<BufWriter<SharedReadWriter<RW>>>,
}

struct SharedReadWriter<RW>(Arc<Mutex<RW>>);

impl<RW> Clone for SharedReadWriter<RW> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<RW> Read for SharedReadWriter<RW>
where
    RW: Read + Write,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("msgpack stream mutex poisoned"))?
            .read(buf)
    }
}

impl<RW> Write for SharedReadWriter<RW>
where
    RW: Read + Write,
{
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("msgpack stream mutex poisoned"))?
            .write(buf)
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("msgpack stream mutex poisoned"))?
            .flush()
    }
}

// NewMessagePackProtocol creates a new msgpack protocol handler.
pub fn new_message_pack_protocol<RW>(rw: RW) -> MessagePackProtocol<RW>
where
    RW: Read + Write,
{
    let rw = SharedReadWriter(Arc::new(Mutex::new(rw)));
    MessagePackProtocol {
        r: Mutex::new(BufReader::new(rw.clone())),
        w: Mutex::new(BufWriter::new(rw)),
    }
}

impl<RW> MessagePackProtocol<RW>
where
    RW: Read + Write,
{
    fn read_tuple<R: Read>(
        reader: &mut BufReader<R>,
    ) -> Result<(MessageType, String, Vec<u8>), std::io::Error> {
        // Read fixed array marker (0x93 = 3-element array)
        let t = read_byte(reader)?;
        if t != MSGPACK_FIXED_ARRAY3 {
            return Err(invalid_request(format!(
                "expected fixed 3-element array (0x93), received: 0x{t:02x}"
            )));
        }

        // Read message type - can be positive fixint (0x00-0x7F) or uint8 (0xCC + value)
        let t = read_byte(reader)?;
        let raw_type = if t <= 0x7F {
            // Positive fixint - the byte IS the value
            t
        } else if t == MSGPACK_U8 {
            // uint8 marker - next byte is the value
            read_byte(reader)?
        } else {
            return Err(invalid_request(format!(
                "expected positive fixint or uint8 marker, received: 0x{t:02x}"
            )));
        };
        let msg_type = MessageType::from(raw_type);
        if !msg_type.is_valid() {
            return Err(invalid_request(format!(
                "unknown message type: {}",
                raw_type
            )));
        }

        // Read method (binary)
        let method_bytes = Self::read_bin(reader)?;
        let method = String::from_utf8(method_bytes)
            .map_err(|err| invalid_request(format!("invalid method utf8: {err}")))?;

        // Read payload (binary)
        let payload = Self::read_bin(reader)?;

        Ok((msg_type, method, payload))
    }

    fn read_bin<R: Read>(reader: &mut BufReader<R>) -> Result<Vec<u8>, std::io::Error> {
        let t = read_byte(reader)?;

        let size = match t {
            MSGPACK_BIN8 => read_byte(reader)? as usize,
            MSGPACK_BIN16 => {
                let mut buf = [0u8; 2];
                reader.read_exact(&mut buf)?;
                u16::from_be_bytes(buf) as usize
            }
            MSGPACK_BIN32 => {
                let mut buf = [0u8; 4];
                reader.read_exact(&mut buf)?;
                u32::from_be_bytes(buf) as usize
            }
            _ => {
                return Err(invalid_request(format!(
                    "expected binary data (0xc4-0xc6), received: 0x{t:02x}"
                )));
            }
        };

        let mut payload = vec![0u8; size];
        reader.read_exact(&mut payload)?;
        Ok(payload)
    }

    fn write_tuple<W: Write>(
        writer: &mut BufWriter<W>,
        msg_type: MessageType,
        method: &str,
        payload: &[u8],
    ) -> Result<(), std::io::Error> {
        // Write fixed array marker
        writer.write_all(&[MSGPACK_FIXED_ARRAY3])?;
        // Write message type as positive fixint (values 0-127 are written directly)
        writer.write_all(&[msg_type.0])?;
        // Write method
        Self::write_bin(writer, method.as_bytes())?;
        // Write payload
        Self::write_bin(writer, payload)?;
        writer.flush()
    }

    fn write_bin<W: Write>(writer: &mut BufWriter<W>, data: &[u8]) -> Result<(), std::io::Error> {
        let length = data.len();
        if length < 256 {
            writer.write_all(&[MSGPACK_BIN8, length as u8])?;
        } else if length < 1 << 16 {
            writer.write_all(&[MSGPACK_BIN16])?;
            writer.write_all(&(length as u16).to_be_bytes())?;
        } else {
            writer.write_all(&[MSGPACK_BIN32])?;
            writer.write_all(&(length as u32).to_be_bytes())?;
        }
        writer.write_all(data)
    }
}

impl<RW> Protocol for MessagePackProtocol<RW>
where
    RW: Read + Write,
{
    // ReadMessage implements Protocol.
    fn read_message(&self) -> Result<Message, std::io::Error> {
        let mut reader = self
            .r
            .lock()
            .map_err(|_| std::io::Error::other("msgpack reader mutex poisoned"))?;
        let (msg_type, method, payload) = Self::read_tuple(&mut reader)?;

        // Convert msgpack message type to JSON-RPC message
        let mut msg = Message::default();

        match msg_type {
            MessageType::Request => {
                // Client request - needs an ID for response
                // We use the method as a pseudo-ID since this protocol doesn't have explicit IDs
                let id = jsonrpc::Id::new_string(method.clone());
                msg.id = Some(id);
                msg.method = method;
                msg.params = json_value_from_payload(&payload)?;
            }
            MessageType::CallResponse => {
                // Response to our Call - use method as ID
                // Note: Method must be empty for IsResponse() to return true
                let id = jsonrpc::Id::new_string(method);
                msg.id = Some(id);
                msg.result = json_value_from_payload(&payload)?;
            }
            MessageType::CallError => {
                // Error response to our Call
                // Note: Method must be empty for IsResponse() to return true
                let id = jsonrpc::Id::new_string(method);
                msg.id = Some(id);
                msg.error = Some(jsonrpc::ResponseError {
                    code: jsonrpc::CODE_INTERNAL_ERROR,
                    message: String::from_utf8_lossy(&payload).into_owned(),
                    data: None,
                });
            }
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unexpected message type: {}", msg_type.0),
                ));
            }
        }

        Ok(msg)
    }

    // WriteRequest implements Protocol.
    fn write_request(
        &self,
        _id: Option<&jsonrpc::Id>,
        method: &str,
        params: json::Value,
    ) -> Result<(), std::io::Error> {
        // For msgpack protocol, requests from server are "Call" type
        let payload = json::marshal(&params, &[])
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        let mut writer = self
            .w
            .lock()
            .map_err(|_| std::io::Error::other("msgpack writer mutex poisoned"))?;
        Self::write_tuple(&mut writer, MessageType::Call, method, &payload)
    }

    // WriteNotification implements Protocol.
    fn write_notification(&self, method: &str, params: json::Value) -> Result<(), std::io::Error> {
        // Msgpack protocol doesn't distinguish notifications from calls
        self.write_request(None, method, params)
    }

    // WriteResponse implements Protocol.
    fn write_response(
        &self,
        id: Option<&jsonrpc::Id>,
        result: json::Value,
    ) -> Result<(), std::io::Error> {
        let method = id.map(|id| id.to_string()).unwrap_or_default();

        let payload = if let Some(payload) = raw_binary_payload(&result) {
            payload
        } else {
            json::marshal(&result, &[])
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?
        };

        let mut writer = self
            .w
            .lock()
            .map_err(|_| std::io::Error::other("msgpack writer mutex poisoned"))?;
        Self::write_tuple(&mut writer, MessageType::Response, &method, &payload)
    }

    // WriteError implements Protocol.
    fn write_error(
        &self,
        id: Option<&jsonrpc::Id>,
        resp_err: &jsonrpc::ResponseError,
    ) -> Result<(), std::io::Error> {
        let method = id.map(|id| id.to_string()).unwrap_or_default();
        let mut writer = self
            .w
            .lock()
            .map_err(|_| std::io::Error::other("msgpack writer mutex poisoned"))?;
        Self::write_tuple(
            &mut writer,
            MessageType::Error,
            &method,
            resp_err.message.as_bytes(),
        )
    }
}

fn read_byte<R: Read>(reader: &mut R) -> Result<u8, std::io::Error> {
    let mut buf = [0u8; 1];
    reader.read_exact(&mut buf)?;
    Ok(buf[0])
}

fn invalid_request(message: String) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!("api: invalid request: {message}"),
    )
}

fn json_value_from_payload(payload: &[u8]) -> Result<json::Value, std::io::Error> {
    serde_json::from_slice(payload)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
}

// RawBinary is a marker type for binary data that should be written
// directly by MessagePackProtocol instead of being JSON-encoded.
pub struct RawBinary(Vec<u8>);

impl From<Vec<u8>> for RawBinary {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

impl serde::Serialize for RawBinary {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(RAW_BINARY_MARKER, &self.0)?;
        map.end()
    }
}

fn raw_binary_payload(value: &json::Value) -> Option<Vec<u8>> {
    let object = value.as_object()?;
    if object.len() != 1 {
        return None;
    }
    let bytes = object.get(RAW_BINARY_MARKER)?.as_array()?;
    let mut payload = Vec::with_capacity(bytes.len());
    for byte in bytes {
        let byte = byte.as_u64()?;
        if byte > u8::MAX as u64 {
            return None;
        }
        payload.push(byte as u8);
    }
    Some(payload)
}
