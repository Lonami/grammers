// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::configuration::ConnectionParams;
use crate::{
    AuthorizationError, InvocationError, NoReconnect, ReadError, Sender, ServerAddr, connect,
    connect_with_auth,
};
use futures_util::future::{Either, select};
use grammers_mtproto::{mtp, transport};
use grammers_session::{DcOption, Session, UpdatesLike};
use grammers_tl_types::{self as tl, enums};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6};
use std::panic;
use std::pin::pin;
use std::sync::Arc;
use tokio::task::AbortHandle;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinSet,
};

pub(crate) type Transport = transport::Full;

type InvokeResponse = Vec<u8>;

enum Request {
    Invoke {
        dc_id: i32,
        body: Vec<u8>,
        tx: oneshot::Sender<Result<InvokeResponse, InvocationError>>,
    },
    Disconnect {
        dc_id: i32,
    },
    Quit,
}

struct Rpc {
    body: Vec<u8>,
    tx: oneshot::Sender<Result<InvokeResponse, InvocationError>>,
}

struct ConnectionInfo {
    dc_id: i32,
    rpc_tx: mpsc::UnboundedSender<Rpc>,
    abort_handle: AbortHandle,
}

#[derive(Clone)]
pub struct SenderPoolHandle(mpsc::UnboundedSender<Request>);

pub struct SenderPool {
    pub runner: SenderPoolRunner,
    pub handle: SenderPoolHandle,
    pub updates: mpsc::UnboundedReceiver<UpdatesLike>,
}

pub struct SenderPoolRunner {
    pub session: Arc<dyn Session>,
    pub api_id: i32,
    pub connection_params: ConnectionParams,
    request_rx: mpsc::UnboundedReceiver<Request>,
    updates_tx: mpsc::UnboundedSender<UpdatesLike>,
}

impl SenderPoolHandle {
    pub async fn invoke_in_dc(
        &self,
        dc_id: i32,
        body: Vec<u8>,
    ) -> Result<InvokeResponse, InvocationError> {
        let (tx, rx) = oneshot::channel();
        self.0
            .send(Request::Invoke { dc_id, body, tx })
            .map_err(|_| InvocationError::Dropped)?;
        rx.await.map_err(|_| InvocationError::Dropped)?
    }

    pub fn disconnect_from_dc(&self, dc_id: i32) -> bool {
        self.0.send(Request::Disconnect { dc_id }).is_ok()
    }

    pub fn quit(&self) -> bool {
        self.0.send(Request::Quit).is_ok()
    }
}

impl SenderPool {
    pub fn new<S: Session + 'static>(session: Arc<S>, api_id: i32) -> Self {
        Self::with_configuration(session, api_id, Default::default())
    }

    pub fn with_configuration<S: Session + 'static>(
        session: Arc<S>,
        api_id: i32,
        connection_params: ConnectionParams,
    ) -> Self {
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let (updates_tx, updates_rx) = mpsc::unbounded_channel();

        Self {
            runner: SenderPoolRunner {
                session: session as Arc<dyn Session>,
                api_id,
                connection_params,
                request_rx,
                updates_tx,
            },
            handle: SenderPoolHandle(request_tx),
            updates: updates_rx,
        }
    }
}

impl SenderPoolRunner {
    /// Run the sender pool until [`crate::SenderPoolHandle::quit`] is called.
    ///
    /// Connections will be initiated on-demand whenever the first request to a DC is made.
    pub async fn run(self) {
        let Self {
            session,
            api_id,
            connection_params,
            mut request_rx,
            updates_tx,
        } = self;
        let init_connection = tl::functions::InvokeWithLayer {
            layer: tl::LAYER,
            query: tl::functions::InitConnection {
                api_id,
                device_model: connection_params.device_model.clone(),
                system_version: connection_params.system_version.clone(),
                app_version: connection_params.app_version.clone(),
                system_lang_code: connection_params.system_lang_code.clone(),
                lang_pack: "".into(),
                lang_code: connection_params.lang_code.clone(),
                proxy: None,
                params: None,
                query: tl::functions::help::GetConfig {},
            },
        };

        let mut connections = Vec::<ConnectionInfo>::new();
        let mut connection_pool = JoinSet::<Result<(), ReadError>>::new();

        while let Some(request) = request_rx.recv().await {
            while let Some(completion) = connection_pool.try_join_next() {
                if let Err(err) = completion {
                    if let Ok(reason) = err.try_into_panic() {
                        panic::resume_unwind(reason);
                    }
                }
            }

            match request {
                Request::Invoke { dc_id, body, tx } => {
                    let Some(mut dc_option) = session.dc_option(dc_id) else {
                        let _ = tx.send(Err(InvocationError::InvalidDc));
                        continue;
                    };

                    let connection = match connections
                        .iter()
                        .find(|connection| connection.dc_id == dc_id)
                    {
                        Some(connection) => connection,
                        None => {
                            let (sender, config) =
                                connect_sender(&init_connection, &dc_option).await.unwrap();

                            update_config(session.as_ref(), config);

                            dc_option.auth_key = Some(sender.auth_key());
                            session.set_dc_option(&dc_option);

                            let (rpc_tx, rpc_rx) = mpsc::unbounded_channel();
                            let abort_handle = connection_pool.spawn(run_sender(
                                sender,
                                rpc_rx,
                                updates_tx.clone(),
                            ));
                            connections.push(ConnectionInfo {
                                dc_id,
                                rpc_tx,
                                abort_handle,
                            });
                            connections.last().unwrap()
                        }
                    };
                    let _ = connection.rpc_tx.send(Rpc { body, tx });
                }
                Request::Disconnect { dc_id } => {
                    connections.retain(|connection| {
                        if connection.dc_id == dc_id {
                            connection.abort_handle.abort();
                            false
                        } else {
                            true
                        }
                    });
                }
                Request::Quit => break,
            }
        }

        connections.clear(); // drop all channels to cause the `run_sender` loop to stop
        connection_pool.join_all().await;
    }
}

async fn connect_sender(
    init_connection: &tl::functions::InvokeWithLayer<
        tl::functions::InitConnection<tl::functions::help::GetConfig>,
    >,
    dc_option: &DcOption,
) -> Result<(Sender<transport::Full, mtp::Encrypted>, tl::types::Config), AuthorizationError> {
    let transport = transport::Full::new;
    let addr = || ServerAddr::Tcp {
        address: dc_option.ipv4.into(),
    };

    let mut sender = if let Some(auth_key) = dc_option.auth_key {
        connect_with_auth(transport(), addr(), auth_key, &NoReconnect).await?
    } else {
        connect(transport(), addr(), &NoReconnect).await?
    };

    let enums::Config::Config(remote_config) = match sender.invoke(init_connection).await {
        Ok(config) => config,
        Err(InvocationError::Read(ReadError::Transport(transport::Error::BadStatus {
            status: 404,
        }))) => {
            sender = connect(transport(), addr(), &NoReconnect).await?;
            sender.invoke(init_connection).await?
        }
        Err(e) => return Err(dbg!(e).into()),
    };

    Ok((sender, remote_config))
}

fn update_config(session: &dyn Session, config: tl::types::Config) {
    config
        .dc_options
        .iter()
        .map(|tl::enums::DcOption::Option(option)| option)
        .filter(|option| !option.media_only && !option.tcpo_only && option.r#static)
        .for_each(|option| {
            let mut dc_option = session.dc_option(option.id).unwrap_or_else(|| DcOption {
                id: option.id,
                ipv4: SocketAddrV4::new(Ipv4Addr::from_bits(0), 0),
                ipv6: SocketAddrV6::new(Ipv6Addr::from_bits(0), 0, 0, 0),
                auth_key: None,
            });
            if option.ipv6 {
                dc_option.ipv6 = SocketAddrV6::new(
                    option
                        .ip_address
                        .parse()
                        .expect("Telegram to return a valid IPv6 address"),
                    option.port as _,
                    0,
                    0,
                );
            } else {
                dc_option.ipv4 = SocketAddrV4::new(
                    option
                        .ip_address
                        .parse()
                        .expect("Telegram to return a valid IPv4 address"),
                    option.port as _,
                );
                if dc_option.ipv6.ip().to_bits() == 0 {
                    dc_option.ipv6 = SocketAddrV6::new(
                        dc_option.ipv4.ip().to_ipv6_mapped(),
                        dc_option.ipv4.port(),
                        0,
                        0,
                    )
                }
            }
        });
}

async fn run_sender(
    mut sender: Sender<Transport, grammers_mtproto::mtp::Encrypted>,
    mut rpc_rx: mpsc::UnboundedReceiver<Rpc>,
    updates: mpsc::UnboundedSender<UpdatesLike>,
) -> Result<(), ReadError> {
    loop {
        let rpc = {
            let step = pin!(sender.step());
            let rpc = pin!(rpc_rx.recv());

            match select(step, rpc).await {
                Either::Left((step, _)) => match step {
                    Ok(all_new_updates) => {
                        all_new_updates.into_iter().for_each(|new_updates| {
                            let _ = updates.send(new_updates);
                        });
                        continue;
                    }
                    Err(err) => break Err(err),
                },
                Either::Right((Some(rpc), _)) => rpc,
                Either::Right((None, _)) => break Ok(()),
            }
        };

        sender.enqueue_body(rpc.body, rpc.tx);
    }
}
