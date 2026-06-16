use serde::{Deserialize, Serialize, ser::SerializeStruct};
use ts_json as json;
use ts_jsonrpc as jsonrpc;

use super::{
    ErrorCodeInvalidParams, ErrorCodeInvalidRequest, IntegerOrString, Method, unmarshal_params,
};

// NewID creates an ID from an IntegerOrString value.
// This wrapper exists because lsproto has its own IntegerOrString type.
pub fn new_id(raw_value: IntegerOrString) -> jsonrpc::Id {
    if let Some(string) = raw_value.string {
        return jsonrpc::Id::new_string(string);
    }
    jsonrpc::Id::new_int(raw_value.integer.unwrap())
}

#[derive(Clone)]
pub struct Message {
    pub kind: jsonrpc::MessageKind,
    msg: MessageData,
}

#[derive(Clone)]
enum MessageData {
    Request(RequestMessage),
    Response(ResponseMessage),
}

impl Message {
    pub fn as_request(&self) -> &RequestMessage {
        match &self.msg {
            MessageData::Request(msg) => msg,
            MessageData::Response(_) => panic!("Message is not a RequestMessage"),
        }
    }

    pub fn as_response(&self) -> &ResponseMessage {
        match &self.msg {
            MessageData::Response(msg) => msg,
            MessageData::Request(_) => panic!("Message is not a ResponseMessage"),
        }
    }
}

impl<'de> Deserialize<'de> for Message {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = serde_json::to_vec(&serde_json::Value::deserialize(deserializer)?)
            .map_err(serde::de::Error::custom)?;
        Message::unmarshal_json(&data).map_err(serde::de::Error::custom)
    }
}

impl Serialize for Message {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match &self.msg {
            MessageData::Request(msg) => msg.serialize(serializer),
            MessageData::Response(msg) => msg.serialize(serializer),
        }
    }
}

impl Message {
    pub fn unmarshal_json(data: &[u8]) -> Result<Self, String> {
        #[derive(Default, Deserialize)]
        struct RawMessage {
            #[serde(default, rename = "jsonrpc")]
            jsonrpc: jsonrpc::JsonRpcVersion,
            #[serde(default)]
            method: Method,
            #[serde(default)]
            id: Option<jsonrpc::Id>,
            params: Option<json::Value>,
            result: Option<json::Value>,
            #[serde(default)]
            error: Option<jsonrpc::ResponseError>,
        }

        let mut raw = RawMessage::default();
        json::unmarshal(data, &mut raw, &[])
            .map_err(|err| format!("{ErrorCodeInvalidRequest}: {err}"))?;
        let _ = raw.jsonrpc;

        if raw.id.is_some() && raw.method.is_empty() {
            return Ok(Self {
                kind: jsonrpc::MessageKind::Response,
                msg: MessageData::Response(ResponseMessage {
                    jsonrpc: jsonrpc::JsonRpcVersion,
                    id: raw.id,
                    result: raw.result.unwrap_or_default(),
                    error: raw.error,
                }),
            });
        }

        let mut params = serde_json::Value::Null;
        let mut decode_error = None;
        if let Some(raw_params) = raw.params {
            match unmarshal_params(&raw.method, raw_params) {
                Ok(value) => params = value,
                Err(err) => decode_error = Some(err),
            }
        }

        let kind = if raw.id.is_none() {
            jsonrpc::MessageKind::Notification
        } else {
            jsonrpc::MessageKind::Request
        };

        let message = Self {
            kind,
            msg: MessageData::Request(RequestMessage {
                jsonrpc: jsonrpc::JsonRpcVersion,
                id: raw.id,
                method: raw.method,
                params,
            }),
        };

        if let Some(err) = decode_error {
            return Err(format!("{ErrorCodeInvalidParams}: {err}"));
        }

        Ok(message)
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct RequestMessage {
    #[serde(default, rename = "jsonrpc")]
    pub jsonrpc: jsonrpc::JsonRpcVersion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<jsonrpc::Id>,
    pub method: Method,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub params: serde_json::Value,
}

impl RequestMessage {
    pub fn message(&self) -> Message {
        let kind = if self.id.is_none() {
            jsonrpc::MessageKind::Notification
        } else {
            jsonrpc::MessageKind::Request
        };
        Message {
            kind,
            msg: MessageData::Request(self.clone()),
        }
    }

    pub fn unmarshal_json(data: &[u8]) -> Result<Self, String> {
        #[derive(Default, Deserialize)]
        struct RawRequestMessage {
            #[serde(default, rename = "jsonrpc")]
            jsonrpc: jsonrpc::JsonRpcVersion,
            #[serde(default)]
            id: Option<jsonrpc::Id>,
            #[serde(default)]
            method: Method,
            params: Option<json::Value>,
        }

        let mut raw = RawRequestMessage::default();
        json::unmarshal(data, &mut raw, &[])
            .map_err(|err| format!("{ErrorCodeInvalidRequest}: {err}"))?;
        let params = unmarshal_params(&raw.method, raw.params.unwrap_or_default())
            .map_err(|err| format!("{ErrorCodeInvalidRequest}: {err}"))?;

        Ok(Self {
            jsonrpc: raw.jsonrpc,
            id: raw.id,
            method: raw.method,
            params,
        })
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ResponseMessage {
    #[serde(default, rename = "jsonrpc")]
    pub jsonrpc: jsonrpc::JsonRpcVersion,
    #[serde(default)]
    pub id: Option<jsonrpc::Id>,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub result: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<jsonrpc::ResponseError>,
}

impl Serialize for ResponseMessage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut len = 2;
        if !self.result.is_null() || self.error.is_none() {
            len += 1;
        }
        if self.error.is_some() {
            len += 1;
        }

        let mut state = serializer.serialize_struct("ResponseMessage", len)?;
        state.serialize_field("jsonrpc", &self.jsonrpc)?;
        state.serialize_field("id", &self.id)?;
        if !self.result.is_null() || self.error.is_none() {
            state.serialize_field("result", &self.result)?;
        }
        if let Some(error) = &self.error {
            state.serialize_field("error", error)?;
        }
        state.end()
    }
}

impl ResponseMessage {
    pub fn message(&self) -> Message {
        Message {
            kind: jsonrpc::MessageKind::Response,
            msg: MessageData::Response(self.clone()),
        }
    }
}
