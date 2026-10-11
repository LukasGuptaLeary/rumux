//! Authenticated transport for a profile's headless terminal owner.
use anyhow::{Context, Result, ensure};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

pub const PROTOCOL: u32 = 1;
pub const MAX_REQUEST: usize = 1024 * 1024;
pub const MAX_COMPRESSED: usize = 16 * 1024 * 1024;
pub const MAX_RESPONSE: usize = 192 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
pub struct Endpoint {
    pub protocol: u32,
    pub address: SocketAddr,
    pub pid: u32,
    pub token: String,
}

pub fn endpoint_path() -> PathBuf {
    crate::runtime::rumux_config_dir().join("terminal-service.json")
}

#[derive(Clone)]
pub struct Client {
    endpoint: Endpoint,
}

impl Client {
    pub fn discover() -> Result<Self> {
        let bytes = std::fs::read(endpoint_path()).context("Terminal service is not running")?;
        ensure!(bytes.len() <= 4096, "Invalid terminal service descriptor");
        Self::from_endpoint(serde_json::from_slice(&bytes)?)
    }

    pub fn from_endpoint(endpoint: Endpoint) -> Result<Self> {
        ensure!(
            endpoint.protocol == PROTOCOL,
            "Terminal service protocol differs from this app; existing terminals were preserved"
        );
        ensure!(
            endpoint.address.ip().is_loopback(),
            "Terminal service must use loopback"
        );
        ensure!(
            endpoint.token.len() == 64
                && endpoint.token.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "Invalid terminal service authentication"
        );
        Ok(Self { endpoint })
    }

    pub fn call(&self, method: &str, params: Value) -> Result<Value> {
        self.call_typed(method, params)
    }

    pub fn call_typed<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: Value,
    ) -> Result<T> {
        ensure!(
            params.is_object(),
            "Terminal service params must be an object"
        );
        let mut stream = TcpStream::connect_timeout(&self.endpoint.address, Duration::from_secs(2))
            .context("Unable to connect to terminal service")?;
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(Duration::from_secs(4)))?;
        stream.set_write_timeout(Some(Duration::from_secs(4)))?;
        let request = json!({"protocol":PROTOCOL,"token":self.endpoint.token,"method":method,"params":params});
        write_request(&mut stream, &request)?;
        let response: Response<T> = read_response(&mut stream)?;
        ensure!(
            response.ok,
            "{}",
            response
                .error
                .unwrap_or_else(|| "Terminal service request failed".into())
        );
        response
            .result
            .context("Terminal service response has no result")
    }
}

#[derive(Serialize, Deserialize)]
pub struct Request {
    pub protocol: u32,
    pub token: String,
    pub method: String,
    pub params: Value,
}

#[derive(Serialize, Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub struct Response<T> {
    pub ok: bool,
    #[serde(default, deserialize_with = "deserialize_present")]
    pub result: Option<T>,
    pub error: Option<String>,
}

fn deserialize_present<'de, T: Deserialize<'de>, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

pub fn token_matches(expected: &str, provided: &str) -> bool {
    if expected.len() != provided.len() {
        return false;
    }
    expected
        .bytes()
        .zip(provided.bytes())
        .fold(0u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

pub fn read_request(stream: &mut impl Read) -> Result<Request> {
    let length = read_length(stream)?;
    ensure!(
        length <= MAX_REQUEST,
        "Terminal service request exceeds 1 MiB"
    );
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn write_request(stream: &mut impl Write, value: &Value) -> Result<()> {
    let bytes = serde_json::to_vec(value)?;
    ensure!(
        bytes.len() <= MAX_REQUEST,
        "Terminal service request exceeds 1 MiB"
    );
    stream.write_all(&(bytes.len() as u32).to_be_bytes())?;
    stream.write_all(&bytes)?;
    stream.flush()?;
    Ok(())
}

pub fn write_response<T: Serialize>(stream: &mut impl Write, response: &Response<T>) -> Result<()> {
    // Serialize through a bounded encoder; do not first allocate a raw JSON snapshot.
    let mut encoder = GzEncoder::new(LimitedWriter::new(MAX_COMPRESSED), Compression::fast());
    {
        let mut bounded = LimitedSink {
            inner: &mut encoder,
            remaining: MAX_RESPONSE,
        };
        let mut buffered = std::io::BufWriter::with_capacity(64 * 1024, &mut bounded);
        serde_json::to_writer(&mut buffered, response)?;
        buffered.flush()?;
    }
    let bytes = encoder.finish()?.bytes;
    stream.write_all(&(bytes.len() as u32).to_be_bytes())?;
    stream.write_all(&bytes)?;
    stream.flush()?;
    Ok(())
}

pub fn read_response<T: serde::de::DeserializeOwned>(
    stream: &mut impl Read,
) -> Result<Response<T>> {
    let length = read_length(stream)?;
    ensure!(
        length <= MAX_COMPRESSED,
        "Terminal snapshot exceeds transport limit"
    );
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes)?;
    let decoder = std::io::BufReader::with_capacity(64 * 1024, GzDecoder::new(bytes.as_slice()));
    let mut bounded = decoder.take((MAX_RESPONSE + 1) as u64);
    let response = serde_json::from_reader(&mut bounded)?;
    let mut tail = [0; 1];
    ensure!(
        bounded.read(&mut tail)? == 0 && bounded.limit() > 0,
        "Terminal snapshot exceeds decoded limit"
    );
    Ok(response)
}

fn read_length(stream: &mut impl Read) -> Result<usize> {
    let mut length = [0; 4];
    stream.read_exact(&mut length)?;
    Ok(u32::from_be_bytes(length) as usize)
}

struct LimitedWriter {
    bytes: Vec<u8>,
    maximum: usize,
}
impl LimitedWriter {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
        }
    }
}
impl Write for LimitedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.maximum.saturating_sub(self.bytes.len()) {
            return Err(std::io::Error::other("Compressed snapshot limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
struct LimitedSink<'a, W> {
    inner: &'a mut W,
    remaining: usize,
}
impl<W: Write> Write for LimitedSink<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.remaining {
            return Err(std::io::Error::other("Decoded snapshot limit"));
        }
        let written = self.inner.write(bytes)?;
        self.remaining -= written;
        Ok(written)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn framed_snapshot_round_trip_and_bounds() {
        let mut bytes = Vec::new();
        write_response(
            &mut bytes,
            &Response {
                ok: true,
                result: Some(json!({"text":"界🙂"})),
                error: None,
            },
        )
        .unwrap();
        let response: Response<Value> = read_response(&mut bytes.as_slice()).unwrap();
        assert_eq!(response.result.unwrap()["text"], "界🙂");
        let mut bytes = Vec::new();
        write_response(
            &mut bytes,
            &Response::<Option<Value>> {
                ok: true,
                result: Some(None),
                error: None,
            },
        )
        .unwrap();
        let response: Response<Option<Value>> = read_response(&mut bytes.as_slice()).unwrap();
        assert_eq!(response.result, Some(None));
        assert!(read_request(&mut ((MAX_REQUEST + 1) as u32).to_be_bytes().as_slice()).is_err());
        assert!(
            read_response::<Value>(&mut ((MAX_COMPRESSED + 1) as u32).to_be_bytes().as_slice())
                .is_err()
        );
        assert!(token_matches("abcd", "abcd"));
        assert!(!token_matches("abcd", "abce"));
    }
    #[test]
    fn incompatible_or_remote_endpoint_cannot_connect() {
        let endpoint = Endpoint {
            protocol: PROTOCOL + 1,
            address: "127.0.0.1:1".parse().unwrap(),
            pid: 1,
            token: "a".repeat(64),
        };
        assert!(Client::from_endpoint(endpoint).is_err());
        let endpoint = Endpoint {
            protocol: PROTOCOL,
            address: "192.0.2.1:1".parse().unwrap(),
            pid: 1,
            token: "a".repeat(64),
        };
        assert!(Client::from_endpoint(endpoint).is_err());
    }
}
