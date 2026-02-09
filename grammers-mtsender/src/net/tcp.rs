// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use log::info;
use tokio::net::TcpStream;
pub use tokio::net::tcp::{ReadHalf, WriteHalf};

use super::ServerAddr;

pub enum NetStream {
    Tcp(TcpStream),
    #[cfg(feature = "proxy")]
    ProxySocks5(tokio_socks::tcp::Socks5Stream<TcpStream>),
}

impl NetStream {
    pub(crate) fn split(&mut self) -> (ReadHalf<'_>, WriteHalf<'_>) {
        match self {
            Self::Tcp(stream) => stream.split(),
            #[cfg(feature = "proxy")]
            Self::ProxySocks5(stream) => stream.split(),
        }
    }

    pub(crate) async fn connect(addr: &ServerAddr) -> Result<Self, std::io::Error> {
        info!("connecting...");
        match addr {
            ServerAddr::Tcp { address } => Ok(NetStream::Tcp(TcpStream::connect(address).await?)),
            #[cfg(feature = "proxy")]
            ServerAddr::Proxied { address, proxy } => {
                Self::connect_proxy_stream(address, proxy).await
            }
        }
    }

    #[cfg(feature = "proxy")]
    async fn connect_proxy_stream(
        addr: &std::net::SocketAddr,
        proxy_url: &str,
    ) -> Result<NetStream, std::io::Error> {
        use std::{
            io::{self, ErrorKind},
            net::{IpAddr, SocketAddr},
        };

        use hickory_resolver::Resolver;
        use url::Host;

        let proxy = url::Url::parse(proxy_url)
            .map_err(|err| io::Error::new(ErrorKind::InvalidData, err))?;
        let scheme = proxy.scheme();
        let host = proxy.host().ok_or(io::Error::new(
            ErrorKind::NotFound,
            format!("proxy host is missing from url: {}", proxy_url),
        ))?;
        let port = proxy.port().ok_or(io::Error::new(
            ErrorKind::NotFound,
            format!("proxy port is missing from url: {}", proxy_url),
        ))?;
        let username = proxy.username();
        let password = proxy.password().unwrap_or("");
        let socks_addr = match host {
            Host::Domain(domain) => {
                let resolver = Resolver::builder_tokio().unwrap().build();
                let response = resolver.lookup_ip(domain).await?;
                let socks_ip_addr = response.into_iter().next().ok_or(io::Error::new(
                    ErrorKind::NotFound,
                    format!("proxy host did not return any ip address: {}", domain),
                ))?;
                SocketAddr::new(socks_ip_addr, port)
            }
            Host::Ipv4(v4) => SocketAddr::new(IpAddr::from(v4), port),
            Host::Ipv6(v6) => SocketAddr::new(IpAddr::from(v6), port),
        };

        match scheme {
            "socks5" => {
                if username.is_empty() {
                    Ok(NetStream::ProxySocks5(
                        tokio_socks::tcp::Socks5Stream::connect(socks_addr, addr)
                            .await
                            .map_err(|err| io::Error::new(ErrorKind::ConnectionAborted, err))?,
                    ))
                } else {
                    Ok(NetStream::ProxySocks5(
                        tokio_socks::tcp::Socks5Stream::connect_with_password(
                            socks_addr, addr, username, password,
                        )
                        .await
                        .map_err(|err| io::Error::new(ErrorKind::ConnectionAborted, err))?,
                    ))
                }
            }
            "socks4" => {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};

                let mut stream = TcpStream::connect(socks_addr).await?;

                // SOCKS4 CONNECT: VER(04) CMD(01) DSTPORT(2B) DSTIP(4B) USERID NULL
                let ip = match addr.ip() {
                    std::net::IpAddr::V4(v4) => v4,
                    _ => {
                        return Err(io::Error::new(
                            ErrorKind::InvalidInput,
                            "SOCKS4 does not support IPv6",
                        ))
                    }
                };
                let mut req = vec![0x04, 0x01];
                req.extend_from_slice(&addr.port().to_be_bytes());
                req.extend_from_slice(&ip.octets());
                // userid（可选，用 username）
                req.extend_from_slice(username.as_bytes());
                req.push(0x00); // null terminator

                stream.write_all(&req).await?;

                // 响应 8 字节: VN(00) CD DSTPORT(2B) DSTIP(4B)
                let mut resp = [0u8; 8];
                stream.read_exact(&mut resp).await?;

                if resp[1] != 0x5A {
                    return Err(io::Error::new(
                        ErrorKind::ConnectionAborted,
                        format!("SOCKS4 CONNECT rejected (code: 0x{:02X})", resp[1]),
                    ));
                }

                Ok(NetStream::Tcp(stream))
            }
            "http" | "https" => {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                use base64::Engine;

                let mut stream = TcpStream::connect(socks_addr).await?;

                let target = format!("{}:{}", addr.ip(), addr.port());
                let mut request = format!("CONNECT {} HTTP/1.1\r\nHost: {}\r\n", target, target);

                if !username.is_empty() {
                    let creds = format!("{}:{}", username, password);
                    let encoded = base64::engine::general_purpose::STANDARD
                        .encode(creds.as_bytes());
                    request.push_str(&format!("Proxy-Authorization: Basic {}\r\n", encoded));
                }
                request.push_str("\r\n");
                stream.write_all(request.as_bytes()).await?;

                // 逐字节读响应头直到 \r\n\r\n
                let mut buf = Vec::with_capacity(512);
                let mut byte = [0u8; 1];
                loop {
                    stream.read_exact(&mut byte).await?;
                    buf.push(byte[0]);
                    if buf.ends_with(b"\r\n\r\n") {
                        break;
                    }
                    if buf.len() > 4096 {
                        return Err(io::Error::new(
                            ErrorKind::InvalidData,
                            "HTTP CONNECT response too large",
                        ));
                    }
                }

                let response = String::from_utf8_lossy(&buf);
                let status_line = response.lines().next().unwrap_or("");
                let parts: Vec<&str> = status_line.splitn(3, ' ').collect();
                if parts.len() < 2 || parts[1] != "200" {
                    return Err(io::Error::new(
                        ErrorKind::ConnectionAborted,
                        format!("HTTP CONNECT failed: {}", status_line),
                    ));
                }

                // 隧道建立后底层 TCP 等同直连
                Ok(NetStream::Tcp(stream))
            }
            scheme => Err(io::Error::new(
                ErrorKind::ConnectionAborted,
                format!("proxy scheme not supported: {}", scheme),
            )),
        }
    }
}
