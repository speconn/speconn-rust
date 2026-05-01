use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeconnError {
    pub code: Code,
    pub message: String,
}

impl std::error::Error for SpeconnError {}

impl fmt::Display for SpeconnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl SpeconnError {
    pub fn new(code: Code, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    pub fn http_status(&self) -> u16 {
        self.code.http_status()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    Canceled,
    Unknown,
    InvalidArgument,
    DeadlineExceeded,
    NotFound,
    AlreadyExists,
    PermissionDenied,
    ResourceExhausted,
    FailedPrecondition,
    Aborted,
    OutOfRange,
    Unimplemented,
    Internal,
    Unavailable,
    DataLoss,
    Unauthenticated,
}

impl Code {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Canceled => "canceled",
            Self::Unknown => "unknown",
            Self::InvalidArgument => "invalid_argument",
            Self::DeadlineExceeded => "deadline_exceeded",
            Self::NotFound => "not_found",
            Self::AlreadyExists => "already_exists",
            Self::PermissionDenied => "permission_denied",
            Self::ResourceExhausted => "resource_exhausted",
            Self::FailedPrecondition => "failed_precondition",
            Self::Aborted => "aborted",
            Self::OutOfRange => "out_of_range",
            Self::Unimplemented => "unimplemented",
            Self::Internal => "internal",
            Self::Unavailable => "unavailable",
            Self::DataLoss => "data_loss",
            Self::Unauthenticated => "unauthenticated",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "canceled" => Self::Canceled,
            "unknown" => Self::Unknown,
            "invalid_argument" => Self::InvalidArgument,
            "deadline_exceeded" => Self::DeadlineExceeded,
            "not_found" => Self::NotFound,
            "already_exists" => Self::AlreadyExists,
            "permission_denied" => Self::PermissionDenied,
            "resource_exhausted" => Self::ResourceExhausted,
            "failed_precondition" => Self::FailedPrecondition,
            "aborted" => Self::Aborted,
            "out_of_range" => Self::OutOfRange,
            "unimplemented" => Self::Unimplemented,
            "internal" => Self::Internal,
            "unavailable" => Self::Unavailable,
            "data_loss" => Self::DataLoss,
            "unauthenticated" => Self::Unauthenticated,
            _ => Self::Unknown,
        }
    }

    pub fn http_status(&self) -> u16 {
        match self {
            Self::InvalidArgument | Self::FailedPrecondition | Self::OutOfRange => 400,
            Self::Unauthenticated => 401,
            Self::PermissionDenied => 403,
            Self::NotFound => 404,
            Self::AlreadyExists | Self::Aborted => 409,
            Self::ResourceExhausted => 429,
            Self::Canceled => 499,
            Self::Unimplemented => 501,
            Self::Unavailable => 503,
            Self::DeadlineExceeded => 504,
            _ => 500,
        }
    }

    pub fn from_http_status(status: u16) -> Self {
        match status {
            400 => Self::Internal,
            401 => Self::Unauthenticated,
            403 => Self::PermissionDenied,
            404 => Self::Unimplemented,
            429 => Self::Unavailable,
            502 | 503 | 504 => Self::Unavailable,
            _ => Self::Unknown,
        }
    }
}

use specodec::{respond, dispatch, SpecCodec, JsonWriter, MsgPackWriter, JsonReader, MsgPackReader};

impl SpeconnError {
    /// Encode to the given format (json / msgpack / gron).
    pub fn encode(&self, format: &str) -> Vec<u8> {
        let codec: SpecCodec<SpeconnError> = SpecCodec {
            encode: |obj, w| {
                w.begin_object(2);
                w.write_field("code"); w.write_string(obj.code.as_str());
                w.write_field("message"); w.write_string(&obj.message);
                w.end_object();
            },
            decode: |_| Err(specodec::SCodecError::new("not used")),
        };
        respond(&codec, self, format).body
    }

    /// Decode a non-empty payload into a SpeconnError.
    pub fn decode(payload: &[u8], format: &str) -> Self {
        let codec: SpecCodec<SpeconnError> = SpecCodec {
            encode: |_, _| {},
            decode: |r| {
                let mut code = String::new();
                let mut message = String::new();
                r.begin_object()?;
                while r.has_next_field()? {
                    match r.read_field_name()?.as_str() {
                        "code"    => { code = r.read_string()?; }
                        "message" => { message = r.read_string()?; }
                        _         => { r.skip()?; }
                    }
                }
                r.end_object()?;
                Ok(SpeconnError { code: Code::from_str(&code), message })
            },
        };
        dispatch(&codec, payload, format)
            .unwrap_or_else(|_| SpeconnError::new(Code::Unknown, "decode error"))
    }
}
