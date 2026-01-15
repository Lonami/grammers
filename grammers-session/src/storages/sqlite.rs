// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use futures_core::future::BoxFuture;
use libsql::{named_params, params};
use tokio::sync::Mutex as AsyncMutex;

use crate::types::{
    ChannelKind, ChannelState, DcOption, PeerAuth, PeerId, PeerInfo, PeerKind, UpdateState,
    UpdatesState,
};
use crate::{DEFAULT_DC, KNOWN_DC_OPTIONS, Session};

const VERSION: i64 = 1;

struct Database(libsql::Connection);

struct Cache {
    pub home_dc: i32,
    pub dc_options: HashMap<i32, DcOption>,
}

/// SQLite-based storage. This is the recommended option.
pub struct SqliteSession {
    database: AsyncMutex<Database>,
    cache: Mutex<Cache>,
}

#[repr(u8)]
enum PeerSubtype {
    UserSelf = 1,
    UserBot = 2,
    UserSelfBot = 3,
    Megagroup = 4,
    Broadcast = 8,
    Gigagroup = 12,
}

impl Database {
    async fn init(&self) -> libsql::Result<()> {
        let mut user_version: i64 = self
            .fetch_one("PRAGMA user_version", params![], |row| row.get(0))
            .await?
            .unwrap_or(0);
        if user_version == VERSION {
            return Ok(());
        }

        if user_version == 0 {
            self.migrate_v0_to_v1().await?;
            user_version += 1;
        }
        if user_version == VERSION {
            // Can't bind PRAGMA parameters, but `VERSION` is not user-controlled input.
            self.0
                .execute(&format!("PRAGMA user_version = {VERSION}"), params![])
                .await?;
        }
        Ok(())
    }

    async fn migrate_v0_to_v1(&self) -> libsql::Result<()> {
        let transaction = self.begin_transaction().await?;
        transaction
            .execute(
                "CREATE TABLE dc_home (
                dc_id INTEGER NOT NULL,
                PRIMARY KEY(dc_id))",
                params![],
            )
            .await?;
        transaction
            .execute(
                "CREATE TABLE dc_option (
                dc_id INTEGER NOT NULL,
                ipv4 TEXT NOT NULL,
                ipv6 TEXT NOT NULL,
                auth_key BLOB,
                PRIMARY KEY (dc_id))",
                params![],
            )
            .await?;
        transaction
            .execute(
                "CREATE TABLE peer_info (
                peer_id INTEGER NOT NULL,
                hash INTEGER,
                subtype INTEGER,
                PRIMARY KEY (peer_id))",
                params![],
            )
            .await?;
        transaction
            .execute(
                "CREATE TABLE update_state (
                pts INTEGER NOT NULL,
                qts INTEGER NOT NULL,
                date INTEGER NOT NULL,
                seq INTEGER NOT NULL)",
                params![],
            )
            .await?;
        transaction
            .execute(
                "CREATE TABLE channel_state (
                peer_id INTEGER NOT NULL,
                pts INTEGER NOT NULL,
                PRIMARY KEY (peer_id))",
                params![],
            )
            .await?;

        transaction.commit().await?;
        Ok(())
    }

    async fn begin_transaction(&self) -> libsql::Result<libsql::Transaction> {
        self.0.transaction().await
    }

    async fn fetch_one<
        T,
        P: libsql::params::IntoParams,
        F: FnOnce(libsql::Row) -> libsql::Result<T>,
    >(
        &self,
        statement: &str,
        params: P,
        select: F,
    ) -> libsql::Result<Option<T>> {
        let mut statement = self.0.prepare(statement).await?;
        let result = statement.query_row(params).await;
        match result {
            Ok(value) => Ok(Some(select(value)?)),
            Err(libsql::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn fetch_all<
        T,
        P: libsql::params::IntoParams,
        F: FnMut(libsql::Row) -> libsql::Result<T>,
    >(
        &self,
        statement: &str,
        params: P,
        mut select: F,
    ) -> libsql::Result<Vec<T>> {
        let statement = self.0.prepare(statement).await?;
        let mut rows = statement.query(params).await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            result.push(select(row)?);
        }
        Ok(result)
    }
}

impl SqliteSession {
    /// Open a connection to the SQLite database at `path`,
    /// creating one if it doesn't exist.
    pub async fn open<P: AsRef<Path>>(path: P) -> libsql::Result<Self> {
        let conn = libsql::Builder::new_local(path).build().await?.connect()?;
        let db = Database(conn);
        db.init().await?;

        let home_dc = db
            .fetch_one("SELECT * FROM dc_home LIMIT 1", named_params![], |row| {
                Ok(row.get::<i32>(0)?)
            })
            .await?
            .unwrap_or(DEFAULT_DC);

        let dc_options = db
            .fetch_all("SELECT * FROM dc_option", named_params![], |row| {
                Ok(DcOption {
                    id: row.get::<i32>(0)?,
                    ipv4: row.get::<String>(1)?.parse().unwrap(),
                    ipv6: row.get::<String>(2)?.parse().unwrap(),
                    auth_key: row
                        .get::<Option<Vec<u8>>>(3)?
                        .map(|auth_key| auth_key.try_into().unwrap()),
                })
            })
            .await?
            .into_iter()
            .map(|dc_option| (dc_option.id, dc_option))
            .collect();

        Ok(SqliteSession {
            database: AsyncMutex::new(db),
            cache: Mutex::new(Cache {
                home_dc,
                dc_options,
            }),
        })
    }
}

impl Session for SqliteSession {
    fn home_dc_id(&self) -> i32 {
        self.cache.lock().unwrap().home_dc
    }

    fn set_home_dc_id(&self, dc_id: i32) -> BoxFuture<'_, ()> {
        self.cache.lock().unwrap().home_dc = dc_id;
        Box::pin(async move {
            let transaction = self
                .database
                .lock()
                .await
                .begin_transaction()
                .await
                .unwrap();
            transaction
                .execute("DELETE FROM dc_home", params![])
                .await
                .unwrap();
            let stmt = transaction
                .prepare("INSERT INTO dc_home VALUES (:dc_id)")
                .await
                .unwrap();
            stmt.execute(named_params! {":dc_id": dc_id}).await.unwrap();
            transaction.commit().await.unwrap();
        })
    }

    fn dc_option(&self, dc_id: i32) -> Option<DcOption> {
        self.cache
            .lock()
            .unwrap()
            .dc_options
            .get(&dc_id)
            .cloned()
            .or_else(|| {
                KNOWN_DC_OPTIONS
                    .iter()
                    .find(|dc_option| dc_option.id == dc_id)
                    .cloned()
            })
    }

    fn set_dc_option(&self, dc_option: &DcOption) -> BoxFuture<'_, ()> {
        self.cache
            .lock()
            .unwrap()
            .dc_options
            .insert(dc_option.id, dc_option.clone());

        let dc_option = dc_option.clone();
        Box::pin(async move {
            let db = self.database.lock().await;
            db.0.execute(
                "INSERT OR REPLACE INTO dc_option VALUES (:dc_id, :ipv4, :ipv6, :auth_key)",
                named_params! {
                    ":dc_id": dc_option.id,
                    ":ipv4": dc_option.ipv4.to_string(),
                    ":ipv6": dc_option.ipv6.to_string(),
                    ":auth_key": dc_option.auth_key.map(|k| k.to_vec()),
                },
            )
            .await
            .unwrap();
        })
    }

    fn peer(&self, peer: PeerId) -> BoxFuture<'_, Option<PeerInfo>> {
        Box::pin(async move {
            let db = self.database.lock().await;
            let map_row = |row: libsql::Row| {
                let subtype = row.get::<Option<i64>>(2)?.map(|s| s as u8);
                Ok(match peer.kind() {
                    PeerKind::User | PeerKind::UserSelf => PeerInfo::User {
                        id: PeerId::user(row.get::<i64>(0)?).bare_id(),
                        auth: row.get::<Option<i64>>(1)?.map(PeerAuth::from_hash),
                        bot: subtype.map(|s| s & PeerSubtype::UserBot as u8 != 0),
                        is_self: subtype.map(|s| s & PeerSubtype::UserSelf as u8 != 0),
                    },
                    PeerKind::Chat => PeerInfo::Chat { id: peer.bare_id() },
                    PeerKind::Channel => PeerInfo::Channel {
                        id: peer.bare_id(),
                        auth: row.get::<Option<i64>>(1)?.map(PeerAuth::from_hash),
                        kind: subtype.and_then(|s| {
                            if (s & PeerSubtype::Gigagroup as u8) == PeerSubtype::Gigagroup as u8 {
                                Some(ChannelKind::Gigagroup)
                            } else if s & PeerSubtype::Broadcast as u8 != 0 {
                                Some(ChannelKind::Broadcast)
                            } else if s & PeerSubtype::Megagroup as u8 != 0 {
                                Some(ChannelKind::Megagroup)
                            } else {
                                None
                            }
                        }),
                    },
                })
            };

            if peer.kind() == PeerKind::UserSelf {
                db.fetch_one(
                    "SELECT * FROM peer_info WHERE subtype & :type LIMIT 1",
                    named_params! {":type": PeerSubtype::UserSelf as i64},
                    map_row,
                )
                .await
                .unwrap()
            } else {
                db.fetch_one(
                    "SELECT * FROM peer_info WHERE peer_id = :peer_id LIMIT 1",
                    named_params! {":peer_id": peer.bot_api_dialog_id()},
                    map_row,
                )
                .await
                .unwrap()
            }
        })
    }

    fn cache_peer(&self, peer: &PeerInfo) -> BoxFuture<'_, ()> {
        let peer = peer.clone();
        Box::pin(async move {
            let db = self.database.lock().await;
            let stmt =
                db.0.prepare("INSERT OR REPLACE INTO peer_info VALUES (:peer_id, :hash, :subtype)")
                    .await
                    .unwrap();
            let subtype = match peer {
                PeerInfo::User { bot, is_self, .. } => {
                    match (bot.unwrap_or_default(), is_self.unwrap_or_default()) {
                        (true, true) => Some(PeerSubtype::UserSelfBot),
                        (true, false) => Some(PeerSubtype::UserBot),
                        (false, true) => Some(PeerSubtype::UserSelf),
                        (false, false) => None,
                    }
                }
                PeerInfo::Chat { .. } => None,
                PeerInfo::Channel { kind, .. } => kind.map(|kind| match kind {
                    ChannelKind::Megagroup => PeerSubtype::Megagroup,
                    ChannelKind::Broadcast => PeerSubtype::Broadcast,
                    ChannelKind::Gigagroup => PeerSubtype::Gigagroup,
                }),
            };
            let mut params = vec![];
            let peer_id = peer.id().bot_api_dialog_id();
            params.push((":peer_id".to_owned(), peer_id));
            let hash = peer.auth().unwrap_or_default().hash();
            if peer.auth().is_some() {
                params.push((":hash".to_owned(), hash));
            }
            let subtype = subtype.map(|s| s as i64);
            if subtype.is_some() {
                params.push((":subtype".to_owned(), subtype.unwrap()));
            }
            stmt.execute(params).await.unwrap();
        })
    }

    fn updates_state(&self) -> BoxFuture<'_, UpdatesState> {
        Box::pin(async move {
            let db = self.database.lock().await;
            let mut state = db
                .fetch_one(
                    "SELECT * FROM update_state LIMIT 1",
                    named_params![],
                    |row| {
                        Ok(UpdatesState {
                            pts: row.get(0)?,
                            qts: row.get(1)?,
                            date: row.get(2)?,
                            seq: row.get(3)?,
                            channels: Vec::new(),
                        })
                    },
                )
                .await
                .unwrap()
                .unwrap_or_default();
            state.channels = db
                .fetch_all("SELECT * FROM channel_state", named_params![], |row| {
                    Ok(ChannelState {
                        id: row.get(0)?,
                        pts: row.get(1)?,
                    })
                })
                .await
                .unwrap();
            state
        })
    }

    fn set_update_state(&self, update: UpdateState) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            let db = self.database.lock().await;
            let transaction = db.begin_transaction().await.unwrap();

            match update {
                UpdateState::All(updates_state) => {
                    transaction
                        .execute("DELETE FROM update_state", params![])
                        .await
                        .unwrap();
                    transaction
                        .execute(
                            "INSERT INTO update_state VALUES (:pts, :qts, :date, :seq)",
                            named_params! {
                                ":pts": updates_state.pts,
                                ":qts": updates_state.qts,
                                ":date": updates_state.date,
                                ":seq": updates_state.seq,
                            },
                        )
                        .await
                        .unwrap();

                    transaction
                        .execute("DELETE FROM channel_state", params![])
                        .await
                        .unwrap();
                    for channel in updates_state.channels {
                        transaction
                            .execute(
                                "INSERT INTO channel_state VALUES (:peer_id, :pts)",
                                named_params! {
                                    ":peer_id": channel.id,
                                    ":pts": channel.pts,
                                },
                            )
                            .await
                            .unwrap();
                    }
                }
                UpdateState::Primary { pts, date, seq } => {
                    let previous = db
                        .fetch_one(
                            "SELECT * FROM update_state LIMIT 1",
                            named_params![],
                            |_| Ok(()),
                        )
                        .await
                        .unwrap();

                    if previous.is_some() {
                        transaction
                            .execute(
                                "UPDATE update_state SET pts = :pts, date = :date, seq = :seq",
                                named_params! {
                                    ":pts": pts,
                                    ":date": date,
                                    ":seq": seq,
                                },
                            )
                            .await
                            .unwrap();
                    } else {
                        transaction
                            .execute(
                                "INSERT INTO update_state VALUES (:pts, 0, :date, :seq)",
                                named_params! {
                                    ":pts": pts,
                                    ":date": date,
                                    ":seq": seq,
                                },
                            )
                            .await
                            .unwrap();
                    }
                }
                UpdateState::Secondary { qts } => {
                    let previous = db
                        .fetch_one(
                            "SELECT * FROM update_state LIMIT 1",
                            named_params![],
                            |_| Ok(()),
                        )
                        .await
                        .unwrap();

                    if previous.is_some() {
                        transaction
                            .execute(
                                "UPDATE update_state SET qts = :qts",
                                named_params! {":qts": qts},
                            )
                            .await
                            .unwrap();
                    } else {
                        transaction
                            .execute(
                                "INSERT INTO update_state VALUES (0, :qts, 0, 0)",
                                named_params! {":qts": qts},
                            )
                            .await
                            .unwrap();
                    }
                }
                UpdateState::Channel { id, pts } => {
                    transaction
                        .execute(
                            "INSERT OR REPLACE INTO channel_state VALUES (:peer_id, :pts)",
                            named_params! {
                                ":peer_id": id,
                                ":pts": pts,
                            },
                        )
                        .await
                        .unwrap();
                }
            }

            transaction.commit().await.unwrap();
        })
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6};

    use {DcOption, KNOWN_DC_OPTIONS, PeerInfo, Session, UpdateState};

    use super::*;

    #[test]
    fn exercise_sqlite_session() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(do_exercise_sqlite_session());
    }

    async fn do_exercise_sqlite_session() {
        let session = SqliteSession::open(":memory:").await.unwrap();

        assert_eq!(session.home_dc_id(), DEFAULT_DC);
        session.set_home_dc_id(DEFAULT_DC + 1).await;
        assert_eq!(session.home_dc_id(), DEFAULT_DC + 1);

        assert_eq!(
            session.dc_option(KNOWN_DC_OPTIONS[0].id),
            Some(KNOWN_DC_OPTIONS[0].clone())
        );
        let new_dc_option = DcOption {
            id: KNOWN_DC_OPTIONS
                .iter()
                .map(|dc_option| dc_option.id)
                .max()
                .unwrap()
                + 1,
            ipv4: SocketAddrV4::new(Ipv4Addr::from_bits(0), 1),
            ipv6: SocketAddrV6::new(Ipv6Addr::from_bits(0), 1, 0, 0),
            auth_key: Some([1; 256]),
        };
        assert_eq!(session.dc_option(new_dc_option.id), None);
        session.set_dc_option(&new_dc_option).await;
        assert_eq!(session.dc_option(new_dc_option.id), Some(new_dc_option));

        assert_eq!(session.peer(PeerId::self_user()).await, None);
        assert_eq!(session.peer(PeerId::user(1)).await, None);
        let peer = PeerInfo::User {
            id: 1,
            auth: None,
            bot: Some(true),
            is_self: Some(true),
        };
        session.cache_peer(&peer).await;
        assert_eq!(session.peer(PeerId::self_user()).await, Some(peer.clone()));
        assert_eq!(session.peer(PeerId::user(1)).await, Some(peer));

        assert_eq!(session.peer(PeerId::channel(1)).await, None);
        let peer = PeerInfo::Channel {
            id: 1,
            auth: Some(PeerAuth::from_hash(-1)),
            kind: Some(ChannelKind::Broadcast),
        };
        session.cache_peer(&peer).await;
        assert_eq!(session.peer(PeerId::channel(1)).await, Some(peer));

        assert_eq!(session.updates_state().await, UpdatesState::default());
        session
            .set_update_state(UpdateState::All(UpdatesState {
                pts: 1,
                qts: 2,
                date: 3,
                seq: 4,
                channels: vec![
                    ChannelState { id: 5, pts: 6 },
                    ChannelState { id: 7, pts: 8 },
                ],
            }))
            .await;
        session
            .set_update_state(UpdateState::Primary {
                pts: 2,
                date: 4,
                seq: 5,
            })
            .await;
        session
            .set_update_state(UpdateState::Secondary { qts: 3 })
            .await;
        session
            .set_update_state(UpdateState::Channel { id: 7, pts: 9 })
            .await;
        assert_eq!(
            session.updates_state().await,
            UpdatesState {
                pts: 2,
                qts: 3,
                date: 4,
                seq: 5,
                channels: vec![
                    ChannelState { id: 5, pts: 6 },
                    ChannelState { id: 7, pts: 9 },
                ],
            }
        );
    }
}
