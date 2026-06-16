use serde::de::DeserializeOwned;
use std::sync::Arc;
use ts_core as context;
use ts_json as json;

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("api: connection closed")]
    ConnClosed,
    #[error("api: request timeout")]
    RequestTimeout,
    #[error("{0}")]
    Message(String),
}

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

pub trait Handler {
    // handle_request handles an incoming request and returns a result or error.
    fn handle_request(
        &self,
        ctx: &context::Context,
        method: &str,
        params: json::Value,
    ) -> Result<json::Value, Error>;

    // handle_notification handles an incoming notification.
    fn handle_notification(
        &self,
        ctx: &context::Context,
        method: &str,
        params: json::Value,
    ) -> Result<(), Error>;
}

pub trait Conn {
    // run starts processing messages on the connection. It blocks until the
    // context is cancelled or an error occurs.
    fn run(&self, ctx: &context::Context) -> Result<(), Error>;

    // call sends a request to the client and waits for a response.
    fn call(
        &self,
        ctx: &context::Context,
        method: &str,
        params: json::Value,
    ) -> Result<json::Value, Error>;

    // notify sends a notification to the client with no response expected.
    fn notify(
        &self,
        ctx: &context::Context,
        method: &str,
        params: json::Value,
    ) -> Result<(), Error>;
}

impl<T> Conn for Arc<T>
where
    T: Conn + ?Sized,
{
    fn run(&self, ctx: &context::Context) -> Result<(), Error> {
        (**self).run(ctx)
    }

    fn call(
        &self,
        ctx: &context::Context,
        method: &str,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        (**self).call(ctx, method, params)
    }

    fn notify(
        &self,
        ctx: &context::Context,
        method: &str,
        params: json::Value,
    ) -> Result<(), Error> {
        (**self).notify(ctx, method, params)
    }
}

pub fn unmarshal_params<T>(params: json::Value) -> Result<Option<T>, Error>
where
    T: DeserializeOwned,
{
    // PORT NOTE: Go's json.Value is raw bytes and uses len(params) == 0 for
    // absent params. The current Rust json::Value alias cannot represent raw
    // empty input, so Null is the connection-layer absence sentinel.
    if params.is_null() {
        return Ok(None);
    }
    serde_json::from_value(params)
        .map(Some)
        .map_err(|err| Error::new(err.to_string()))
}
