// SPDX-License-Identifier: MIT
//! Loopback TCP transport for simulator automation hosts.
//!
//! This transport accepts a single non-blocking client on `127.0.0.1` and
//! exposes it through the byte-oriented [`PlayitTransport`] trait.

use crate::transport::PlayitTransport;
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};

/// Single-client loopback TCP transport for playit automation.
pub struct TcpServerTransport {
    listener: TcpListener,
    stream: Option<TcpStream>,
    read_buf: VecDeque<u8>,
    write_buf: VecDeque<u8>,
}

impl TcpServerTransport {
    /// Bind a loopback TCP listener on the requested port.
    ///
    /// Pass `0` to allow the OS to assign an ephemeral port.
    pub fn bind_loopback(port: u16) -> io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
            stream: None,
            read_buf: VecDeque::new(),
            write_buf: VecDeque::new(),
        })
    }

    /// Return the bound local address.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    fn poll_accept(&mut self) {
        if self.stream.is_some() {
            return;
        }

        match self.listener.accept() {
            Ok((stream, _addr)) => {
                if stream.set_nonblocking(true).is_ok() {
                    self.stream = Some(stream);
                }
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(_err) => {}
        }
    }

    fn drop_stream(&mut self) {
        self.stream = None;
        self.read_buf.clear();
        self.write_buf.clear();
    }

    fn flush_write_buf(&mut self) {
        self.poll_accept();
        let Some(stream) = self.stream.as_mut() else {
            return;
        };

        while !self.write_buf.is_empty() {
            let chunk = self.write_buf.make_contiguous();
            match stream.write(chunk) {
                Ok(0) => {
                    self.drop_stream();
                    return;
                }
                Ok(written) => {
                    self.write_buf.drain(..written);
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => return,
                Err(_err) => {
                    self.drop_stream();
                    return;
                }
            }
        }
    }

    fn fill_read_buf(&mut self) {
        self.poll_accept();
        let Some(stream) = self.stream.as_mut() else {
            return;
        };

        let mut chunk = [0u8; 256];
        loop {
            match stream.read(&mut chunk) {
                Ok(0) => {
                    self.drop_stream();
                    return;
                }
                Ok(count) => {
                    self.read_buf.extend(chunk[..count].iter().copied());
                    if count < chunk.len() {
                        return;
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => return,
                Err(_err) => {
                    self.drop_stream();
                    return;
                }
            }
        }
    }
}

impl PlayitTransport for TcpServerTransport {
    fn read_byte(&mut self) -> Option<u8> {
        if self.read_buf.is_empty() {
            self.fill_read_buf();
        }
        self.read_buf.pop_front()
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        self.write_buf.extend(bytes.iter().copied());
        self.flush_write_buf();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpStream;
    use std::thread;
    use std::time::{Duration, Instant};

    fn wait_for_connect(addr: SocketAddr) -> TcpStream {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match TcpStream::connect(addr) {
                Ok(stream) => return stream,
                Err(err) if Instant::now() < deadline => {
                    assert!(
                        matches!(
                            err.kind(),
                            io::ErrorKind::ConnectionRefused
                                | io::ErrorKind::ConnectionAborted
                                | io::ErrorKind::NotConnected
                        ),
                        "unexpected connect error: {err}"
                    );
                    thread::sleep(Duration::from_millis(10));
                }
                Err(err) => panic!("failed to connect to tcp transport: {err}"),
            }
        }
    }

    #[test]
    fn transport_accepts_client_and_reads_bytes() {
        let mut transport = TcpServerTransport::bind_loopback(0).unwrap();
        let addr = transport.local_addr().unwrap();
        let mut client = wait_for_connect(addr);
        client.write_all(b"ABC").unwrap();

        let deadline = Instant::now() + Duration::from_secs(2);
        let mut received = Vec::new();
        while received.len() < 3 && Instant::now() < deadline {
            if let Some(byte) = transport.read_byte() {
                received.push(byte);
            } else {
                thread::sleep(Duration::from_millis(10));
            }
        }

        assert_eq!(received, b"ABC");
    }

    #[test]
    fn transport_writes_back_to_connected_client() {
        let mut transport = TcpServerTransport::bind_loopback(0).unwrap();
        let addr = transport.local_addr().unwrap();
        let client = wait_for_connect(addr);
        let mut reader = BufReader::new(client.try_clone().unwrap());

        let deadline = Instant::now() + Duration::from_secs(2);
        let mut accepted = false;
        while Instant::now() < deadline {
            transport.poll_accept();
            if transport.stream.is_some() {
                accepted = true;
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(accepted, "transport never accepted the client");

        transport.write_bytes(b"OK\r\n");

        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert_eq!(line, "OK\r\n");
    }
}
