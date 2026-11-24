//! Just enough HTTP/1.1 for the web remote: one request per connection, its head and a small
//! body read with limits, and a response that closes the connection.
//!
//! The remote serves a phone on the same network, a page and a few JSON calls, so there is no
//! keep-alive, chunked encoding or compression: whatever doesn't fit is refused.

use std::io::{self, BufRead, BufReader, Read, Write};

/// The most a request's head (request line and headers) may take.
const MAX_HEAD_BYTES: usize = 8 * 1024;
/// The most a request's body may take: a password or a command.
const MAX_BODY_BYTES: usize = 4 * 1024;

/// A request's method, as far as the remote cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// `GET` (and `HEAD`, answered the same way).
    Get,
    /// `POST`.
    Post,
    /// Anything else, which is refused.
    Other,
}

/// One request.
#[derive(Debug, PartialEq, Eq)]
pub struct Request {
    /// Its method.
    pub method: Method,
    /// Its path, without the query.
    pub path: String,
    /// Its headers, names in lower case, in order.
    headers: Vec<(String, String)>,
    /// Its body.
    pub body: Vec<u8>,
}

impl Request {
    /// The first header called `name` (lower case), if any.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(key, _)| key == name).map(|(_, value)| value.as_str())
    }

    /// The value of the cookie called `name`, if the request has one.
    pub fn cookie(&self, name: &str) -> Option<&str> {
        let cookies = self.header("cookie")?;
        cookies.split(';').find_map(|pair| {
            let (key, value) = pair.trim().split_once('=')?;
            (key == name).then_some(value)
        })
    }
}

/// Why a request couldn't be read.
#[derive(Debug)]
pub enum ReadError {
    /// The connection failed or timed out.
    Io,
    /// The request was malformed or too large.
    Malformed,
}

impl From<io::Error> for ReadError {
    fn from(_: io::Error) -> Self {
        Self::Io
    }
}

/// Reads one request from `stream`.
pub fn read_request(stream: impl Read) -> Result<Request, ReadError> {
    let mut reader = BufReader::new(stream.take((MAX_HEAD_BYTES + MAX_BODY_BYTES) as u64));
    let head = read_head(&mut reader)?;
    let mut lines = head.split("\r\n");
    let (method, path) = parse_request_line(lines.next().unwrap_or_default())?;
    let headers =
        lines.filter(|line| !line.is_empty()).map(parse_header).collect::<Result<_, _>>()?;
    let mut request = Request { method, path, headers, body: Vec::new() };
    let length = match request.header("content-length") {
        Some(text) => text.trim().parse::<usize>().map_err(|_| ReadError::Malformed)?,
        None => 0,
    };
    if length > MAX_BODY_BYTES || request.header("transfer-encoding").is_some() {
        return Err(ReadError::Malformed);
    }
    request.body.resize(length, 0);
    reader.read_exact(&mut request.body)?;
    Ok(request)
}

/// The head up to the blank line, without it.
fn read_head(reader: &mut impl BufRead) -> Result<String, ReadError> {
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        let read = reader.read_until(b'\n', &mut head)?;
        if read == 0 || head.len() > MAX_HEAD_BYTES {
            return Err(ReadError::Malformed);
        }
    }
    head.truncate(head.len() - 4);
    String::from_utf8(head).map_err(|_| ReadError::Malformed)
}

fn parse_request_line(line: &str) -> Result<(Method, String), ReadError> {
    let mut parts = line.split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(ReadError::Malformed);
    };
    if !version.starts_with("HTTP/1.") || !target.starts_with('/') {
        return Err(ReadError::Malformed);
    }
    let method = match method {
        "GET" | "HEAD" => Method::Get,
        "POST" => Method::Post,
        _ => Method::Other,
    };
    let path = target.split_once('?').map_or(target, |(path, _)| path);
    Ok((method, path.to_owned()))
}

fn parse_header(line: &str) -> Result<(String, String), ReadError> {
    let (name, value) = line.split_once(':').ok_or(ReadError::Malformed)?;
    Ok((name.trim().to_ascii_lowercase(), value.trim().to_owned()))
}

/// A response's status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// 200.
    Ok,
    /// 204: done, nothing to say.
    NoContent,
    /// 400: the request makes no sense.
    BadRequest,
    /// 401: sign in first, or the password was wrong.
    Unauthorized,
    /// 403: refused whoever asks (another network, another site).
    Forbidden,
    /// 404.
    NotFound,
    /// 405.
    MethodNotAllowed,
    /// 429: too many wrong passwords.
    TooManyRequests,
    /// 503: no password is set, so nobody can sign in.
    Unavailable,
}

impl Status {
    fn line(self) -> &'static str {
        match self {
            Self::Ok => "200 OK",
            Self::NoContent => "204 No Content",
            Self::BadRequest => "400 Bad Request",
            Self::Unauthorized => "401 Unauthorized",
            Self::Forbidden => "403 Forbidden",
            Self::NotFound => "404 Not Found",
            Self::MethodNotAllowed => "405 Method Not Allowed",
            Self::TooManyRequests => "429 Too Many Requests",
            Self::Unavailable => "503 Service Unavailable",
        }
    }
}

/// What the page may do: its own inline script and style, and calls to this server only.
const CONTENT_SECURITY_POLICY: &str = "default-src 'none'; script-src 'unsafe-inline'; \
    style-src 'unsafe-inline'; connect-src 'self'; form-action 'self'; frame-ancestors 'none'; \
    base-uri 'none'";

/// A response.
#[derive(Debug, PartialEq, Eq)]
pub struct Response {
    /// Its status.
    pub status: Status,
    content_type: &'static str,
    /// A `Set-Cookie` value, if any.
    cookie: Option<String>,
    /// Its body.
    pub body: Vec<u8>,
}

impl Response {
    /// A response with `status` and no body.
    pub fn empty(status: Status) -> Self {
        Self { status, content_type: "text/plain; charset=utf-8", cookie: None, body: Vec::new() }
    }

    /// A JSON response.
    pub fn json(status: Status, body: String) -> Self {
        Self { content_type: "application/json", body: body.into_bytes(), ..Self::empty(status) }
    }

    /// A JSON `{"error": message}` response.
    pub fn error(status: Status, message: &str) -> Self {
        Self::json(status, serde_json::json!({ "error": message }).to_string())
    }

    /// An HTML page.
    pub fn html(body: &'static str) -> Self {
        Self {
            content_type: "text/html; charset=utf-8",
            body: body.as_bytes().to_vec(),
            ..Self::empty(Status::Ok)
        }
    }

    /// The same response, setting a cookie.
    pub fn with_cookie(mut self, cookie: String) -> Self {
        self.cookie = Some(cookie);
        self
    }

    /// Writes the response to `stream`, telling the client the connection closes.
    pub fn write_to(&self, mut stream: impl Write) -> io::Result<()> {
        let cookie = (self.cookie.as_ref())
            .map_or_else(String::new, |cookie| format!("Set-Cookie: {cookie}\r\n"));
        let head = format!(
            "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\
             Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n\
             X-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\n\
             Content-Security-Policy: {CONTENT_SECURITY_POLICY}\r\n{cookie}\r\n",
            self.status.line(),
            self.content_type,
            self.body.len(),
        );
        stream.write_all(head.as_bytes())?;
        stream.write_all(&self.body)?;
        stream.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Result<Request, ReadError> {
        read_request(text.as_bytes())
    }

    #[test]
    fn reads_a_post_with_its_body_and_cookies() {
        let request = read(
            "POST /api/command?x=1 HTTP/1.1\r\nHost: 10.0.0.2:8080\r\nCookie: a=1; vlc=abc\r\n\
             Content-Length: 4\r\n\r\nnext",
        )
        .unwrap();
        assert_eq!(request.method, Method::Post);
        assert_eq!(request.path, "/api/command");
        assert_eq!(request.header("host"), Some("10.0.0.2:8080"));
        assert_eq!(request.cookie("vlc"), Some("abc"));
        assert_eq!(request.body, b"next");
    }

    #[test]
    fn refuses_what_it_cannot_read() {
        for text in [
            "GET / HTTP/1.1\r\n",
            "GET example.com HTTP/1.1\r\n\r\n",
            "GET / SPDY\r\n\r\n",
            "POST / HTTP/1.1\r\nContent-Length: 99999\r\n\r\n",
            "POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n",
            "GET / HTTP/1.1\r\nNoColon\r\n\r\n",
        ] {
            assert!(matches!(read(text), Err(ReadError::Malformed)), "{text:?}");
        }
    }

    #[test]
    fn refuses_a_head_too_long() {
        let text = format!("GET / HTTP/1.1\r\nX: {}\r\n\r\n", "a".repeat(MAX_HEAD_BYTES));
        assert!(read(&text).is_err());
    }

    #[test]
    fn a_response_says_its_length_and_closes() {
        let mut out = Vec::new();
        Response::json(Status::Ok, "{}".into())
            .with_cookie("a=b".into())
            .write_to(&mut out)
            .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(text.contains("Content-Length: 2\r\n"));
        assert!(text.contains("Connection: close\r\n"));
        assert!(text.contains("Set-Cookie: a=b\r\n"));
        assert!(text.ends_with("\r\n\r\n{}"));
    }
}
