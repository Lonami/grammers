// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::configuration::{Configuration, DcOption};
use crate::{
    AuthorizationError, Enqueuer, InvocationError, NoReconnect, ReadError, Sender, ServerAddr,
    connect,
};
use grammers_mtproto::{mtp, transport};
use grammers_session::UpdatesLike;
use grammers_tl_types as tl;
use std::panic;
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
        tx: oneshot::Sender<Result<InvokeResponse, Error>>,
    },
    Reconfigure(Configuration),
    Disconnect {
        dc_id: i32,
    },
    Quit,
}

struct ConnectionInfo {
    dc_id: i32,
    enqueuer: Enqueuer,
    abort_handle: AbortHandle,
}

pub enum Error {
    ConnectionClosed,
    InvalidDc,
    Invocation(InvocationError),
}

pub struct SenderPoolHandle(mpsc::UnboundedSender<Request>);

pub struct SenderPool {
    configuration: Configuration,
    request_rx: mpsc::UnboundedReceiver<Request>,
    updates_tx: mpsc::UnboundedSender<UpdatesLike>,
}

impl SenderPoolHandle {
    pub async fn invoke_in_dc(&self, dc_id: i32, body: Vec<u8>) -> Result<InvokeResponse, Error> {
        let (tx, rx) = oneshot::channel();
        self.0
            .send(Request::Invoke { dc_id, body, tx })
            .map_err(|_| Error::ConnectionClosed)?;
        rx.await.map_err(|_| Error::ConnectionClosed)?
    }

    pub fn reconfigure(&self, configuration: Configuration) -> bool {
        self.0.send(Request::Reconfigure(configuration)).is_ok()
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

    pub async fn run(self) {
        let Self {
            mut configuration,
            mut request_rx,
            updates_tx,
        } = self;
        let mut connections = Vec::<ConnectionInfo>::new();
        let mut connection_pool = JoinSet::<ReadError>::new();

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
                            let _ = tx.send(Err(Error::InvalidDc));
                            continue;
                        }
                    };

                    let connection = match connections
                        .iter()
                        .find(|connection| connection.dc_id == dc_id)
                    {
                        Some(connection) => connection,
                        None => {
                            let (sender, enqueuer) =
                                connect_sender(&configuration, dc_option).await.unwrap();
                            let abort_handle =
                                connection_pool.spawn(run_sender(sender, updates_tx.clone()));
                            connections.push(ConnectionInfo {
                                dc_id,
                                enqueuer,
                                abort_handle,
                            });
                            connections.last().unwrap()
                        }
                    };
                    let enqueued = connection.enqueuer.enqueue(body);

                    tokio::spawn(async move {
                        match enqueued.await {
                            Ok(result) => tx.send(result.map_err(Error::Invocation)),
                            Err(_) => tx.send(Err(Error::ConnectionClosed)),
                        }
                    });
                }
                Request::Reconfigure(new_configuration) => configuration = new_configuration,
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
    }
}

async fn connect_sender(
    configuration: &Configuration,
    dc_option: &DcOption,
) -> Result<(Sender<transport::Full, mtp::Encrypted>, Enqueuer), AuthorizationError> {
    let transport = transport::Full::new();

    let (mut sender, tx) = connect(
        transport,
        ServerAddr::Tcp {
            address: dc_option.address.clone(),
        },
        &NoReconnect,
    )
    .await?;

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

    Ok((sender, tx))
}

async fn run_sender(
    mut sender: Sender<Transport, grammers_mtproto::mtp::Encrypted>,
    updates: mpsc::UnboundedSender<UpdatesLike>,
) -> ReadError {
    loop {
        match sender.step().await {
            Ok(all_new_updates) => all_new_updates.into_iter().for_each(|new_updates| {
                let _ = updates.send(new_updates);
            }),
            Err(err) => break err,
        }
    }
}
