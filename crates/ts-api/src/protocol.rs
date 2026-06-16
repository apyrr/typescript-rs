use ts_json as json;
use ts_jsonrpc as jsonrpc;

// Message is an alias for jsonrpc.Message for convenience.
pub type Message = jsonrpc::Message;

// Protocol defines the interface for reading and writing API messages.
pub trait Protocol {
    // ReadMessage reads the next message from the connection.
    fn read_message(&self) -> Result<Message, std::io::Error>;
    // WriteRequest writes a request message.
    fn write_request(
        &self,
        id: Option<&jsonrpc::Id>,
        method: &str,
        params: json::Value,
    ) -> Result<(), std::io::Error>;
    // WriteNotification writes a notification message (no ID).
    fn write_notification(&self, method: &str, params: json::Value) -> Result<(), std::io::Error>;
    // WriteResponse writes a successful response.
    fn write_response(
        &self,
        id: Option<&jsonrpc::Id>,
        result: json::Value,
    ) -> Result<(), std::io::Error>;
    // WriteError writes an error response.
    fn write_error(
        &self,
        id: Option<&jsonrpc::Id>,
        err: &jsonrpc::ResponseError,
    ) -> Result<(), std::io::Error>;
}
