use grammers_mtproto::transports::Transport;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use std::io;
use std::net::SocketAddr;

// TODO look into reusing send and recv buffers to avoid allocations
pub struct TcpTransport<T: Transport> {
    stream: TcpStream,
    transport: T,
}

impl<T: Transport> TcpTransport<T> {
    pub async fn connect(addr: SocketAddr) -> io::Result<Self> {
        Ok(Self {
            stream: TcpStream::connect(addr).await?,
            transport: T::default(),
        })
    }

    pub async fn send(&mut self, data: &[u8]) -> io::Result<()> {
        let mut buffer = vec![0; T::MAX_OVERHEAD + data.len()];
        let size = self
            .transport
            .write_into(&data, &mut buffer)
            .expect("bad max overhead");
        self.stream.write_all(&buffer[..size]).await
    }

    pub async fn recv(&mut self) -> io::Result<Vec<u8>> {
        let mut buffer = Vec::new();

        loop {
            match self.transport.read(&buffer) {
                Ok(data) => break Ok(data.into()),
                Err(len) => {
                    let offset = buffer.len();
                    buffer.resize(len, 0);
                    self.stream.read_exact(&mut buffer[offset..]).await?;
                }
            }
        }
    }
}
