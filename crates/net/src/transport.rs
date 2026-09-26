//! Nonblocking stream I/O with bounded receive work and outbound memory.
use crate::codec;
use serde::{Serialize, de::DeserializeOwned};
use std::{
    collections::VecDeque,
    io::{self, Read, Write},
    net::TcpStream,
};
const QUEUE_LIMIT: usize = 12 * 1024 * 1024;
pub struct Connection {
    stream: TcpStream,
    input: Vec<u8>,
    closed: bool,
    output: VecDeque<Vec<u8>>,
    offset: usize,
    queued: usize,
    pub sent: u64,
    pub received: u64,
}
impl Connection {
    pub fn new(stream: TcpStream) -> io::Result<Self> {
        // Bound kernel buffering too: otherwise a slow reader can accumulate
        // seconds of obsolete snapshots even with an empty application queue.
        socket2::SockRef::from(&stream).set_send_buffer_size(32 * 1024)?;
        stream.set_nonblocking(true)?;
        stream.set_nodelay(true)?;
        Ok(Self {
            stream,
            input: Vec::new(),
            closed: false,
            output: VecDeque::new(),
            offset: 0,
            queued: 0,
            sent: 0,
            received: 0,
        })
    }
    pub fn pending(&self) -> usize {
        self.queued
    }
    pub fn send<T: Serialize>(&mut self, value: &T) -> io::Result<()> {
        self.queue(codec::encode(value)?)
    }
    fn queue(&mut self, bytes: Vec<u8>) -> io::Result<()> {
        if self.queued + bytes.len() > QUEUE_LIMIT {
            return Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "Peer outbound limit",
            ));
        }
        self.queued += bytes.len();
        self.output.push_back(bytes);
        Ok(())
    }
    pub fn flush(&mut self) -> io::Result<()> {
        let mut budget = 512 * 1024;
        while let Some(front) = self.output.front() {
            if budget == 0 {
                break;
            }
            let end = (self.offset + budget).min(front.len());
            match self.stream.write(&front[self.offset..end]) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(n) => {
                    self.offset += n;
                    self.queued -= n;
                    self.sent += n as u64;
                    budget -= n;
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
            if self.offset == front.len() {
                self.output.pop_front();
                self.offset = 0;
            }
        }
        Ok(())
    }
    pub fn receive<T: DeserializeOwned>(&mut self) -> io::Result<Vec<T>> {
        if self.closed && self.input.is_empty() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        let mut scratch = [0u8; 16384];
        for _ in 0..32 {
            match self.stream.read(&mut scratch) {
                Ok(0) => {
                    self.closed = true;
                    break;
                }
                Ok(n) => {
                    self.input.extend_from_slice(&scratch[..n]);
                    self.received += n as u64;
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
            if self.input.len() > codec::MAX_MESSAGE + 512 * 1024 + 9 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Peer receive limit",
                ));
            }
        }
        let mut messages = Vec::new();
        let mut consumed = 0;
        while messages.len() < 64 {
            let Some((message, n)) = codec::decode(&self.input[consumed..])? else {
                break;
            };
            messages.push(message);
            consumed += n;
        }
        if consumed > 0 {
            self.input.drain(..consumed);
        }
        if self.closed && messages.is_empty() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        Ok(messages)
    }
}
