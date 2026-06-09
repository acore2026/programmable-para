use std::net::{TcpStream, Shutdown};
use std::io::{Read, Write};
use serde::Serialize;
use serde::de::DeserializeOwned;
use anyhow::Result;

pub fn read_payload<T: DeserializeOwned>(stream: &mut TcpStream) -> Result<T> {
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf)?;
    let payload = serde_json::from_slice(&buf)?;
    Ok(payload)
}

pub fn write_payload<T: Serialize>(stream: &mut TcpStream, payload: &T) -> Result<()> {
    let bytes = serde_json::to_vec(payload)?;
    stream.write_all(&bytes)?;
    Ok(())
}

pub fn send_request_and_get_response<Req: Serialize, Resp: DeserializeOwned>(
    addr: &str,
    req: &Req,
) -> Result<Resp> {
    let mut stream = TcpStream::connect(addr)?;
    write_payload(&mut stream, req)?;
    stream.shutdown(Shutdown::Write)?;
    
    let resp = read_payload(&mut stream)?;
    Ok(resp)
}
