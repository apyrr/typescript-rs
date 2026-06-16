use std::{
    io::{Read, Write},
    sync::{Arc, Mutex},
};

use serde::Serialize;
use ts_json as json;
use ts_jsonrpc as jsonrpc;

use crate::{Message, Protocol};

// JSONRPCProtocol implements the Protocol interface using JSON-RPC 2.0
// with the LSP base protocol framing (Content-Length headers).
pub struct JSONRPCProtocol<RW: Write> {
    reader: jsonrpc::Reader<RW>,
    writer: jsonrpc::Writer<RW>,
}

pub(crate) struct SharedReadWriter<RW>(Arc<Mutex<RW>>);

impl<RW> Clone for SharedReadWriter<RW> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<RW> Read for SharedReadWriter<RW>
where
    RW: Read + Write,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("jsonrpc stream mutex poisoned"))?
            .read(buf)
    }
}

impl<RW> Write for SharedReadWriter<RW>
where
    RW: Read + Write,
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("jsonrpc stream mutex poisoned"))?
            .write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("jsonrpc stream mutex poisoned"))?
            .flush()
    }
}

// NewJSONRPCProtocol creates a new JSON-RPC protocol handler.
pub fn new_jsonrpc_protocol<RW>(rw: RW) -> JSONRPCProtocol<SharedReadWriter<RW>>
where
    RW: Read + Write,
{
    let rw = SharedReadWriter(Arc::new(Mutex::new(rw)));
    JSONRPCProtocol {
        reader: jsonrpc::Reader::new(rw.clone()),
        writer: jsonrpc::Writer::new(rw),
    }
}

#[derive(Serialize)]
struct ResponseMessageWithResult {
    #[serde(rename = "jsonrpc")]
    jsonrpc: jsonrpc::JsonRpcVersion,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<jsonrpc::Id>,
    result: json::Value,
}

impl<RW> Protocol for JSONRPCProtocol<RW>
where
    RW: Read + Write + Clone,
{
    // ReadMessage implements Protocol.
    fn read_message(&self) -> Result<Message, std::io::Error> {
        let data = self.reader.read()?;

        let mut msg = Message::default();
        json::unmarshal(&data, &mut msg, &[])
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;

        Ok(msg)
    }

    // WriteRequest implements Protocol.
    fn write_request(
        &self,
        id: Option<&jsonrpc::Id>,
        method: &str,
        params: json::Value,
    ) -> Result<(), std::io::Error> {
        let msg = jsonrpc::RequestMessage {
            jsonrpc: jsonrpc::JsonRpcVersion,
            id: id.cloned(),
            method: method.to_string(),
            params,
        };
        let data = json::marshal(&msg, &[])
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        self.writer.write(&data)
    }

    // WriteNotification implements Protocol.
    fn write_notification(&self, method: &str, params: json::Value) -> Result<(), std::io::Error> {
        let msg = jsonrpc::RequestMessage {
            jsonrpc: jsonrpc::JsonRpcVersion,
            id: None,
            method: method.to_string(),
            params,
        };
        let data = json::marshal(&msg, &[])
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        self.writer.write(&data)
    }

    // WriteResponse implements Protocol.
    fn write_response(
        &self,
        id: Option<&jsonrpc::Id>,
        result: json::Value,
    ) -> Result<(), std::io::Error> {
        let msg = ResponseMessageWithResult {
            jsonrpc: jsonrpc::JsonRpcVersion,
            id: id.cloned(),
            result,
        };
        let data = json::marshal(&msg, &[])
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        self.writer.write(&data)
    }

    // WriteError implements Protocol.
    fn write_error(
        &self,
        id: Option<&jsonrpc::Id>,
        resp_err: &jsonrpc::ResponseError,
    ) -> Result<(), std::io::Error> {
        let msg = jsonrpc::ResponseMessage {
            jsonrpc: jsonrpc::JsonRpcVersion,
            id: id.cloned(),
            result: json::Value::default(),
            error: Some(resp_err.clone()),
        };
        let data = json::marshal(&msg, &[])
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        self.writer.write(&data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{
        io::{self, Cursor, Read, Write},
        sync::{Arc, Mutex},
    };

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
            Cursor::new(self.snapshot()).read(buf)
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

    fn written_body(output: &[u8]) -> serde_json::Value {
        let output = String::from_utf8(output.to_vec()).unwrap();
        let (_, body) = output.split_once("\r\n\r\n").unwrap();
        serde_json::from_str(body).unwrap()
    }

    #[test]
    fn write_response_includes_null_result() {
        let rw = SharedRw::default();
        let written = rw.clone();
        let protocol = new_jsonrpc_protocol(rw);

        protocol
            .write_response(Some(&jsonrpc::Id::new_int(1)), json::Value::Null)
            .unwrap();

        assert_eq!(
            written_body(&written.snapshot()),
            serde_json::json!({"jsonrpc":"2.0","id":1,"result":null})
        );
    }
}
