// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::configuration::{Configuration, DcOption};
use crate::{
    AuthorizationError, InvocationError, NoReconnect, ReadError, Sender, ServerAddr, connect,
    connect_with_auth,
};
use futures_util::future::{Either, select};
use grammers_mtproto::{mtp, transport};
use grammers_session::UpdatesLike;
use grammers_tl_types as tl;
use std::panic;
use std::pin::pin;
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
    QueryDcOptions(oneshot::Sender<Vec<DcOption>>),
    ReconfigureDcOptions(Vec<DcOption>),
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

pub struct SenderPoolHandle(mpsc::UnboundedSender<Request>);

pub struct SenderPool {
    configuration: Configuration,
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

    pub async fn query_dc_options(&self) -> Vec<DcOption> {
        let (tx, rx) = oneshot::channel();
        let _ = self.0.send(Request::QueryDcOptions(tx));
        rx.await.unwrap_or_default()
    }

    pub fn reconfigure_dc_options(&self, dc_options: Vec<DcOption>) -> bool {
        self.0
            .send(Request::ReconfigureDcOptions(dc_options))
            .is_ok()
    }

    pub fn disconnect_from_dc(&self, dc_id: i32) -> bool {
        self.0.send(Request::Disconnect { dc_id }).is_ok()
    }

    pub fn quit(&self) -> bool {
        self.0.send(Request::Quit).is_ok()
    }
}

impl SenderPool {
    pub fn new(
        configuration: Configuration,
    ) -> (Self, SenderPoolHandle, mpsc::UnboundedReceiver<UpdatesLike>) {
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let (updates_tx, updates_rx) = mpsc::unbounded_channel();

        (
            Self {
                configuration,
                request_rx,
                updates_tx,
            },
            SenderPoolHandle(request_tx),
            updates_rx,
        )
    }

    /// Run the sender pool until [`crate::SenderPoolHandle::quit`] is called.
    ///
    /// Connections will be initiated on-demand whenever the first request to a DC is made.
    ///
    /// The most-recent configuration is returned so that any changes to the persistent
    /// authentication keys can be persisted. However, It is recommended to persist them
    /// earlier via a call to [`crate::SenderPoolHandle::query_dc_options`], so that the
    /// client can sign out if it has just signed in but failed to persist them.
    pub async fn run(self) -> Configuration {
        let Self {
            mut configuration,
            mut request_rx,
            updates_tx,
        } = self;
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
                    let dc_option = match configuration
                        .dc_options
                        .iter()
                        .find(|dc_option| dc_option.id == dc_id)
                    {
                        Some(dc_option) => dc_option,
                        None => {
                            let _ = tx.send(Err(InvocationError::InvalidDc));
                            continue;
                        }
                    };

                    let connection = match connections
                        .iter()
                        .find(|connection| connection.dc_id == dc_id)
                    {
                        Some(connection) => connection,
                        None => {
                            let sender = connect_sender(&configuration, dc_option).await.unwrap();
                            for dc_option in configuration.dc_options.iter_mut() {
                                if dc_option.id == dc_id {
                                    dc_option.auth_key = Some(sender.auth_key());
                                    break;
                                }
                            }

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
                Request::QueryDcOptions(tx) => {
                    let _ = tx.send(configuration.dc_options.clone());
                }
                Request::ReconfigureDcOptions(dc_options) => configuration.dc_options = dc_options,
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

        connections
            .into_iter()
            .for_each(|connection| connection.abort_handle.abort());

        connection_pool.join_all().await;
        configuration
    }
}

async fn connect_sender(
    configuration: &Configuration,
    dc_option: &DcOption,
) -> Result<Sender<transport::Full, mtp::Encrypted>, AuthorizationError> {
    let transport = transport::Full::new();
    let addr = ServerAddr::Tcp {
        address: dc_option.address.clone(),
    };

    let mut sender = if let Some(auth_key) = dc_option.auth_key {
        connect_with_auth(transport, addr, auth_key, &NoReconnect).await?
    } else {
        connect(transport, addr, &NoReconnect).await?
    };

    let _remote_config = sender
        .invoke(&tl::functions::InvokeWithLayer {
            layer: tl::LAYER,
            query: tl::functions::InitConnection {
                api_id: configuration.api_id,
                device_model: configuration.device_model.clone(),
                system_version: configuration.system_version.clone(),
                app_version: configuration.app_version.clone(),
                system_lang_code: configuration.system_lang_code.clone(),
                lang_pack: "".into(),
                lang_code: configuration.lang_code.clone(),
                proxy: None,
                params: None,
                query: tl::functions::help::GetConfig {},
            },
        })
        .await?;

    Ok(sender)
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
