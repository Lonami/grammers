use grammers_mtproto::transports::Transport;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use std::io;
use std::net::SocketAddr;

/// The maximum data that we're willing to send or receive at once.
///
/// By having a fixed-size buffer, we can avoid unnecessary allocations
/// and trivially prevent allocating more than this limit if we ever
/// received invalid data.
///
/// Telegram will close the connection with roughly a megabyte of data,
/// so to account for the transports' own overhead, we add a few extra
/// kilobytes to the maximum data size.
const MAXIMUM_DATA: usize = (1 * 1024 * 1024) + (8 * 1024);

pub struct TcpTransport<T: Transport> {
    stream: TcpStream,
    transport: T,
    send_buffer: Box<[u8]>,
    recv_buffer: Box<[u8]>,
}

impl<T: Transport> TcpTransport<T> {
    pub async fn connect(addr: SocketAddr) -> io::Result<Self> {
        Ok(Self {
            stream: TcpStream::connect(addr).await?,
            transport: T::default(),
            send_buffer: vec![0; MAXIMUM_DATA].into_boxed_slice(),
            recv_buffer: vec![0; MAXIMUM_DATA].into_boxed_slice(),
        })
    }

    pub async fn send(&mut self, data: &[u8]) -> io::Result<()> {
        let size = self
            .transport
            .write_into(&data, self.send_buffer.as_mut())
            .expect("tried to send more than MAXIMUM_DATA in a single frame");

        self.stream.write_all(&self.send_buffer[..size]).await
    }

    pub async fn recv(&mut self) -> io::Result<Vec<u8>> {
        let mut len = 0;
        loop {
            match self.transport.read(&self.recv_buffer[..len]) {
                Ok(data) => break Ok(data.into()),
                Err(required_len) => {
                    self.stream
                        .read_exact(&mut self.recv_buffer[len..required_len])
                        .await?;

                    len = required_len;
                }
            }
        }
    }
}
