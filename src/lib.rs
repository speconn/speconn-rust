pub mod error;
pub mod envelope;
pub mod transport;
pub mod client;

#[cfg(feature = "server")]
pub mod router;

pub use error::{Code, SpeconnError};
pub use envelope::{decode_envelope, encode_envelope, FLAG_COMPRESSED, FLAG_END_STREAM};
pub use transport::Transport;
pub use client::SpeconnClient;

#[cfg(feature = "reqwest")]
pub use transport::ReqwestTransport;

#[cfg(feature = "isahc")]
pub use transport::IsahcTransport;

#[cfg(feature = "server")]
pub use router::SpeconnRouter;
