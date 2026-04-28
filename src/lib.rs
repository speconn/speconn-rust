pub mod error;
pub mod envelope;
pub mod transport;
pub mod client;

#[cfg(feature = "server")]
pub mod router;

pub use error::{Code, SpeconnError};
pub use envelope::{decode_envelope, encode_envelope, FLAG_COMPRESSED, FLAG_END_STREAM};
pub use transport::{HttpClient, HttpResponse};
pub use client::{SpeconnClient, RequestBuilder};

#[cfg(feature = "reqwest")]
pub use reqwest;

#[cfg(feature = "server")]
pub use router::{SpeconnRouter, SpeconnContext, RouterResponse, SpeconnRequest, Interceptor};
