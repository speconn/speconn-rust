pub mod error;
pub mod envelope;
pub mod transport;
pub mod client;


#[cfg(feature = "server")]
pub mod context;

#[cfg(feature = "server")]
pub mod context_key;

#[cfg(feature = "server")]
pub mod router;

pub use error::{Code, SpeconnError};
pub use envelope::{decode_envelope, encode_envelope, FLAG_COMPRESSED, FLAG_END_STREAM};
pub use transport::{SpeconnTransport, HttpResponse};
pub use client::SpeconnClient;

pub use specodec::{
    SpecCodec, SpecReader, SCodecError,
    JsonReader, JsonWriter, MsgPackReader, MsgPackWriter,
    dispatch, respond,
};

#[cfg(feature = "reqwest")]
pub use transport::ReqwestTransport;

#[cfg(feature = "server")]
pub use router::{SpeconnRouter, RouterResponse, SpeconnRequest, Interceptor};

#[cfg(feature = "server")]
pub use context::SpeconnContext;

#[cfg(feature = "server")]
pub use context_key::{
    ContextKey, set_value, get_value, delete_value,
    user_key, request_id_key, user_id_key,
    get_user, set_user, get_request_id, set_request_id,
};

#[cfg(all(test, feature = "server"))]
mod context_test;