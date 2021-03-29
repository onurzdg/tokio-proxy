use crate::config::MAX_HTTP_CONNECT_REQUEST_SIZE;
use crate::description::AsDescription;
use crate::errors::{
    HttpParseError, HttpTunnelRequestDecodeError, HttpTunnelRequestError, IoErrorKind,
};
use bytes::BytesMut;
use httparse::{Request, Status, EMPTY_HEADER};
use std::borrow::Cow;
use std::fmt;
use std::fmt::Write;
use std::io::ErrorKind;
use tokio_util::codec::{Decoder, Encoder};

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct HttpTunnelTarget {
    target: String,
}

impl HttpTunnelTarget {
    pub fn target(&self) -> &str {
        self.target.as_str()
    }
}

impl fmt::Display for HttpTunnelTarget {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "target: {}", self.target.as_str())
    }
}

#[derive(Clone)]
pub struct HttpCodec;

impl Decoder for HttpCodec {
    type Item = HttpTunnelTarget;
    type Error = HttpTunnelRequestDecodeError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let mut headers = [EMPTY_HEADER; 10];
        let mut req = Request::new(&mut headers[..]);
        let result = req.parse(src);
        /*
        println!("headers:");
        for header in req.headers {
            println!("{:?}", String::from_utf8(header.value.into()))
        }*/

        match result {
            Ok(Status::Partial) => Ok(None),
            Ok(Status::Complete(_)) => {
                check_method(req.method)?;
                check_size(src.len())?;
                check_version(req.version)?;
                Ok(HttpTunnelTarget {
                    target: req.path.expect("could not extract the hostname").into(),
                }
                .into())
            }
            Err(e) => Err(HttpTunnelRequestDecodeError::ParseError(
                HttpParseError::ParseError(e),
            )),
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum HttpTunnelRequestResult {
    Error(HttpTunnelRequestError),
    Success,
}

impl AsDescription for HttpTunnelRequestResult {
    fn as_description(&self) -> Cow<'static, str> {
        match self {
            Self::Error(err) => err.as_description(),
            Self::Success => "success".into(),
        }
    }
}

impl fmt::Display for HttpTunnelRequestResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_description().as_ref())
    }
}

impl Encoder<HttpTunnelRequestResult> for HttpCodec {
    type Error = std::io::Error;
    fn encode(
        &mut self,
        item: HttpTunnelRequestResult,
        dst: &mut BytesMut,
    ) -> Result<(), Self::Error> {
        use HttpTunnelRequestError::*;
        let (code, status_text) = match item {
            HttpTunnelRequestResult::Success => (200u16, "OK"),
            HttpTunnelRequestResult::Error(err) => match err {
                BadRequest => (400, "Bad Request"),
                Forbidden => (403, "Forbidden"),
                RequestTimeout => (408, "Request Timeout"),
                InternalError => (500, "Internal Error"),
                GatewayTimeout => (504, "Gateway Timeout"),
                BadGateway => (502, "Bad Gateway"),
                RequestDecodeError(decode_err) => {
                    use HttpTunnelRequestDecodeError::*;
                    match decode_err {
                        NotSupportedHTTPVersion(_) | ParseError(_) => (400, "Bad Request"),
                        NotSupportedMethod(_) => (405, "Method Not allowed"),
                        RequestSizeTooBig(_) => (413, "Payload Too Large"),
                        ServerError(err) => match err {
                            IoErrorKind::ErrorKind(ErrorKind::TimedOut) => (408, "Request Timeout"),
                            _ => (500, "Internal Server Error"),
                        },
                    }
                }
            },
        };
        dst.write_fmt(format_args!("HTTP/1.1 {} {}\r\n\r\n", code, status_text))
            .map_err(|_| std::io::Error::from(ErrorKind::Other))
    }
}

fn check_method(m: Option<&str>) -> Result<(), HttpTunnelRequestDecodeError> {
    match m {
        Some("CONNECT") => Ok(()),
        Some(other) => Err(HttpTunnelRequestDecodeError::NotSupportedMethod(
            other.into(),
        )),
        _ => Err(HttpTunnelRequestDecodeError::NotSupportedMethod(
            "Unknown".into(),
        )),
    }
}

fn check_size(s: usize) -> Result<(), HttpTunnelRequestDecodeError> {
    if s <= MAX_HTTP_CONNECT_REQUEST_SIZE {
        Ok(())
    } else {
        Err(HttpTunnelRequestDecodeError::RequestSizeTooBig(s))
    }
}

fn check_version(m: Option<u8>) -> Result<(), HttpTunnelRequestDecodeError> {
    match m {
        Some(1) => Ok(()),
        Some(other) => Err(HttpTunnelRequestDecodeError::NotSupportedHTTPVersion(
            format!("{}", other),
        )),
        _ => Err(HttpTunnelRequestDecodeError::NotSupportedHTTPVersion(
            "Unknown".into(),
        )),
    }
}
