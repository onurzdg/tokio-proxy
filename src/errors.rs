use crate::config::MAX_HTTP_CONNECT_REQUEST_SIZE;
use crate::description::AsDescription;
use serde::{Serialize, Serializer};
use std::borrow::Cow;
use std::fmt;
use tokio::io::ErrorKind;

#[derive(Eq, PartialEq, Debug, Clone, Serialize)]
pub enum HttpTunnelRequestError {
    RequestDecodeError(HttpTunnelRequestDecodeError),
    BadRequest,
    RequestTimeout,
    GatewayTimeout,
    BadGateway,
    Forbidden,
    InternalError,
}

impl AsDescription for HttpTunnelRequestError {
    fn as_description(&self) -> Cow<'static, str> {
        match self {
            Self::BadRequest => "bad client request".into(),
            Self::RequestTimeout => "timeout occurred while decoding client request".into(),
            Self::GatewayTimeout => "timeout occurred while establishing connection to target".into(),
            Self::BadGateway => "unable to connect to target".into(),
            Self::Forbidden => "access to site is not allowed".into(),
            Self::InternalError => "internal error occurred".into(),
            Self::RequestDecodeError(err) => err.as_description(),
        }
    }
}

impl fmt::Display for HttpTunnelRequestError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_description().as_ref())
    }
}

impl std::error::Error for HttpTunnelRequestError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpParseError {
    ParseError(httparse::Error),
}

impl fmt::Display for HttpParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let HttpParseError::ParseError(err) = self;
        let err_str = format!("HttpParseError({:?})", err);
        f.write_str(&err_str)
    }
}

impl Serialize for HttpParseError {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        let HttpParseError::ParseError(err) = self;
        let err_str = format!("HttpParseError({:?})", err);
        serializer.serialize_str(&err_str)
    }
}

impl std::error::Error for HttpParseError {}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IoErrorKind {
    ErrorKind(ErrorKind),
}

impl Serialize for IoErrorKind {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        let IoErrorKind::ErrorKind(err) = self;
        let err_str = format!("IoErrorKind({:?})", err);
        serializer.serialize_str(&err_str)
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize)]
pub enum HttpTunnelRequestDecodeError {
    RequestSizeTooBig(usize),
    NotSupportedMethod(String),
    NotSupportedHTTPVersion(String),
    ParseError(HttpParseError),
    ServerError(IoErrorKind),
}

impl AsDescription for HttpTunnelRequestDecodeError {
    fn as_description(&self) -> Cow<'static, str> {
        match self {
            Self::ParseError(HttpParseError::ParseError(v @ httparse::Error::Version)) => {
                format!("{}: required HTTP version is 1.1", v).into()
            },
            Self::ParseError(HttpParseError::ParseError(t @ httparse::Error::Token)) => {
                format!("bad request: {}", t).into()
            },
            Self::ParseError(HttpParseError::ParseError(err)) => {
                format!("parse error: {}", err).into()
            },
            Self::RequestSizeTooBig(size) => format!(
                "request size too big; max allowed{} bytes. size: {}",
                MAX_HTTP_CONNECT_REQUEST_SIZE, size
            ).into(),
            Self::NotSupportedMethod(method) => {
                format!("only CONNECT is supported, provided {}", method).into()
            },
            Self::NotSupportedHTTPVersion(version) => {
                format!("required HTTP version is 1.1, found {}", version).into()
            },
            Self::ServerError(err) => format!("server error: {:?}", err).into(),
        }
    }
}

impl fmt::Display for HttpTunnelRequestDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_description().as_ref())
    }
}

impl std::error::Error for HttpTunnelRequestDecodeError {}

impl From<std::io::Error> for HttpTunnelRequestDecodeError {
    fn from(e: std::io::Error) -> Self {
        HttpTunnelRequestDecodeError::ServerError(IoErrorKind::ErrorKind(e.kind()))
    }
}
