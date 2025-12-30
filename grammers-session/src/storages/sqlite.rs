// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::types::{
    ChannelKind, ChannelState, DcOption, PeerAuth, PeerId, PeerInfo, PeerKind, UpdateState,
    UpdatesState,
};
use crate::{DEFAULT_DC, KNOWN_DC_OPTIONS, Session};
use rusqlite::named_params;
use std::path::Path;
use std::sync::Mutex;

const VERSION: i64 = 1;

struct Database(rusqlite::Connection);

struct TransactionGuard<'c>(&'c rusqlite::Connection);

/// SQLite-based storage. This is the recommended option.
pub struct SqliteSession {
    database: Mutex<Database>,
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
    fn init(&self) -> rusqlite::Result<()> {
        let mut user_version: i64 = self
            .fetch_one("PRAGMA user_version", named_params![], |row| row.get(0))?
            .unwrap_or(0);
        if user_version == VERSION {
            return Ok(());
        }

        if user_version == 0 {
            self.migrate_v0_to_v1()?;
            user_version += 1;
        }
        if user_version == VERSION {
            // Can't bind PRAGMA parameters, but `VERSION` is not user-controlled input.
            self.0
                .execute(&format!("PRAGMA user_version = {VERSION}"), [])?;
        }
        Ok(())
    }

    fn migrate_v0_to_v1(&self) -> rusqlite::Result<()> {
        let _transaction = self.begin_transaction()?;
        self.0.execute(
            "CREATE TABLE dc_home (
                dc_id INTEGER NOT NULL,
                PRIMARY KEY(dc_id))",
            [],
        )?;
        self.0.execute(
            "CREATE TABLE dc_option (
                dc_id INTEGER NOT NULL,
                ipv4 TEXT NOT NULL,
                ipv6 TEXT NOT NULL,
                auth_key BLOB,
                PRIMARY KEY (dc_id))",
            [],
        )?;
        self.0.execute(
            "CREATE TABLE peer_info (
                peer_id INTEGER NOT NULL,
                hash INTEGER,
                subtype INTEGER,
                PRIMARY KEY (peer_id))",
            [],
        )?;
        self.0.execute(
            "CREATE TABLE update_state (
                pts INTEGER NOT NULL,
                qts INTEGER NOT NULL,
                date INTEGER NOT NULL,
                seq INTEGER NOT NULL)",
            [],
        )?;
        self.0.execute(
            "CREATE TABLE channel_state (
                peer_id INTEGER NOT NULL,
                pts INTEGER NOT NULL,
                PRIMARY KEY (peer_id))",
            [],
        )?;

        Ok(())
    }

    fn begin_transaction(&self) -> rusqlite::Result<TransactionGuard<'_>> {
        self.0.execute("BEGIN TRANSACTION", [])?;
        Ok(TransactionGuard(&self.0))
    }

    fn fetch_one<T, P: rusqlite::ToSql, F: FnOnce(&rusqlite::Row) -> rusqlite::Result<T>>(
        &self,
        statement: &str,
        params: &[(&str, P)],
        select: F,
    ) -> rusqlite::Result<Option<T>> {
        let mut statement = self.0.prepare(statement)?;
        let result = statement.query_row(params, select);
        match result {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn fetch_all<T, P: rusqlite::ToSql, F: FnMut(&rusqlite::Row) -> rusqlite::Result<T>>(
        &self,
        statement: &str,
        params: &[(&str, P)],
        select: F,
    ) -> rusqlite::Result<Vec<T>> {
        let mut statement = self.0.prepare(statement)?;
        let rows = statement.query_map(params, select)?;
        rows.collect()
    }
}

impl Drop for TransactionGuard<'_> {
    fn drop(&mut self) {
        self.0.execute("COMMIT", []).unwrap();
    }
}

impl SqliteSession {
    /// Open a connection to the SQLite database at `path`,
    /// creating one if it doesn't exist.
    pub fn open<P: AsRef<Path>>(path: P) -> rusqlite::Result<Self> {
        let database = Database(rusqlite::Connection::open(path)?);
        database.init()?;
        Ok(SqliteSession {
            database: Mutex::new(database),
        })
    }
}

impl Session for SqliteSession {
    fn home_dc_id(&self) -> i32 {
        let db = self.database.lock().unwrap();
        db.fetch_one("SELECT * FROM dc_home LIMIT 1", named_params![], |row| {
            Ok(row.get::<_, i32>("dc_id")?)
        })
        .unwrap()
        .unwrap_or(DEFAULT_DC)
    }

    fn set_home_dc_id(&self, dc_id: i32) {
        let db = self.database.lock().unwrap();
        let _transaction = db.begin_transaction().unwrap();
        db.0.execute("DELETE FROM dc_home", []).unwrap();
        let mut stmt = db.0.prepare("INSERT INTO dc_home VALUES (:dc_id)").unwrap();
        stmt.execute(named_params! {":dc_id": dc_id}).unwrap();
    }

    fn dc_option(&self, dc_id: i32) -> Option<DcOption> {
        let db = self.database.lock().unwrap();
        db.fetch_one(
            "SELECT * FROM dc_option WHERE dc_id = :dc_id LIMIT 1",
            &[(":dc_id", dc_id)],
            |row| {
                Ok(DcOption {
                    id: row.get::<_, i32>("dc_id")?,
                    ipv4: row.get::<_, String>("ipv4")?.parse().unwrap(),
                    ipv6: row.get::<_, String>("ipv6")?.parse().unwrap(),
                    auth_key: row
                        .get::<_, Option<Vec<u8>>>("auth_key")?
                        .map(|auth_key| auth_key.try_into().unwrap()),
                })
            },
        )
        .unwrap()
        .or_else(|| {
            KNOWN_DC_OPTIONS
                .iter()
                .find(|dc_option| dc_option.id == dc_id)
                .cloned()
        })
    }

    fn set_dc_option(&self, dc_option: &DcOption) {
        let db = self.database.lock().unwrap();
        db.0.execute(
            "INSERT OR REPLACE INTO dc_option VALUES (:dc_id, :ipv4, :ipv6, :auth_key)",
            named_params! {
                ":dc_id": dc_option.id,
                ":ipv4": dc_option.ipv4.to_string(),
                ":ipv6": dc_option.ipv6.to_string(),
                ":auth_key": dc_option.auth_key.map(|k| k.to_vec()),
            },
        )
        .unwrap();
    }

    fn peer(&self, peer: PeerId) -> Option<PeerInfo> {
        let db = self.database.lock().unwrap();
        let map_row = |row: &rusqlite::Row| {
            let subtype = row.get::<_, Option<i64>>("subtype")?.map(|s| s as u8);
            Ok(match peer.kind() {
                PeerKind::User | PeerKind::UserSelf => PeerInfo::User {
                    id: PeerId::user(row.get::<_, i64>("peer_id")?).bare_id(),
                    auth: row.get::<_, Option<i64>>("hash")?.map(PeerAuth::from_hash),
                    bot: subtype.map(|s| s & PeerSubtype::UserBot as u8 != 0),
                    is_self: subtype.map(|s| s & PeerSubtype::UserSelf as u8 != 0),
                },
                PeerKind::Chat => PeerInfo::Chat { id: peer.bare_id() },
                PeerKind::Channel => PeerInfo::Channel {
                    id: peer.bare_id(),
                    auth: row.get::<_, Option<i64>>("hash")?.map(PeerAuth::from_hash),
                    kind: subtype.and_then(|s| {
                        if (s & PeerSubtype::Gigagroup as u8) == PeerSubtype::Gigagroup as _ {
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
                &[(":type", PeerSubtype::UserSelf as i64)],
                map_row,
            )
            .unwrap()
        } else {
            db.fetch_one(
                "SELECT * FROM peer_info WHERE peer_id = :peer_id LIMIT 1",
                &[(":peer_id", peer.bot_api_dialog_id())],
                map_row,
            )
            .unwrap()
        }
    }

    fn cache_peer(&self, peer: &PeerInfo) {
        let db = self.database.lock().unwrap();
        let mut stmt =
            db.0.prepare("INSERT OR REPLACE INTO peer_info VALUES (:peer_id, :hash, :subtype)")
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
        params.extend_from_slice(named_params! {":peer_id": peer_id});
        let hash = peer.auth().hash();
        if peer.auth() != PeerAuth::default() {
            params.extend_from_slice(named_params! {":hash": hash});
        }
        let subtype = subtype.map(|s| s as i64);
        if subtype.is_some() {
            params.extend_from_slice(named_params! {":subtype": subtype});
        }
        stmt.execute(params.as_slice()).unwrap();
    }

    fn updates_state(&self) -> UpdatesState {
        let db = self.database.lock().unwrap();
        let mut state = db
            .fetch_one(
                "SELECT * FROM update_state LIMIT 1",
                named_params![],
                |row| {
                    Ok(UpdatesState {
                        pts: row.get("pts")?,
                        qts: row.get("qts")?,
                        date: row.get("date")?,
                        seq: row.get("seq")?,
                        channels: Vec::new(),
                    })
                },
            )
            .unwrap()
            .unwrap_or_default();
        state.channels = db
            .fetch_all("SELECT * FROM channel_state", named_params![], |row| {
                Ok(ChannelState {
                    id: row.get("peer_id")?,
                    pts: row.get("pts")?,
                })
            })
            .unwrap();
        state
    }

    fn set_update_state(&self, update: UpdateState) {
        let db = self.database.lock().unwrap();
        let _transaction = db.begin_transaction().unwrap();

        match update {
            UpdateState::All(updates_state) => {
                db.0.execute("DELETE FROM update_state", []).unwrap();
                db.0.execute(
                    "INSERT INTO update_state VALUES (:pts, :qts, :date, :seq)",
                    named_params! {
                        ":pts": updates_state.pts,
                        ":qts": updates_state.qts,
                        ":date": updates_state.date,
                        ":seq": updates_state.seq,
                    },
                )
                .unwrap();

                db.0.execute("DELETE FROM channel_state", []).unwrap();
                for channel in updates_state.channels {
                    db.0.execute(
                        "INSERT INTO channel_state VALUES (:peer_id, :pts)",
                        named_params! {
                            ":peer_id": channel.id,
                            ":pts": channel.pts,
                        },
                    )
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
                    .unwrap();

                if previous.is_some() {
                    db.0.execute(
                        "UPDATE update_state SET pts = :pts, date = :date, seq = :seq",
                        named_params! {
                            ":pts": pts,
                            ":date": date,
                            ":seq": seq,
                        },
                    )
                    .unwrap();
                } else {
                    db.0.execute(
                        "INSERT INTO update_state VALUES (:pts, 0, :date, :seq)",
                        named_params! {
                            ":pts": pts,
                            ":date": date,
                            ":seq": seq,
                        },
                    )
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
                    .unwrap();

                if previous.is_some() {
                    db.0.execute(
                        "UPDATE update_state SET qts = :qts",
                        named_params! {":qts": qts},
                    )
                    .unwrap();
                } else {
                    db.0.execute(
                        "INSERT INTO update_state VALUES (0, :qts, 0, 0)",
                        named_params! {":qts": qts},
                    )
                    .unwrap();
                }
            }
            UpdateState::Channel { id, pts } => {
                db.0.execute(
                    "INSERT OR REPLACE INTO channel_state VALUES (:peer_id, :pts)",
                    named_params! {
                        ":peer_id": id,
                        ":pts": pts,
                    },
                )
                .unwrap();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6};

    use {DcOption, KNOWN_DC_OPTIONS, PeerInfo, Session, UpdateState};

    use super::*;

    #[test]
    fn exercise_sqlite_session() {
        let session = SqliteSession::open(":memory:").unwrap();

        assert_eq!(session.home_dc_id(), DEFAULT_DC);
        session.set_home_dc_id(DEFAULT_DC + 1);
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
        session.set_dc_option(&new_dc_option);
        assert_eq!(session.dc_option(new_dc_option.id), Some(new_dc_option));

        assert_eq!(session.peer(PeerId::self_user()), None);
        assert_eq!(session.peer(PeerId::user(1)), None);
        let peer = PeerInfo::User {
            id: 1,
            auth: None,
            bot: Some(true),
            is_self: Some(true),
        };
        session.cache_peer(&peer);
        assert_eq!(session.peer(PeerId::self_user()), Some(peer.clone()));
        assert_eq!(session.peer(PeerId::user(1)), Some(peer));

        assert_eq!(session.peer(PeerId::channel(1)), None);
        let peer = PeerInfo::Channel {
            id: 1,
            auth: Some(PeerAuth::from_hash(-1)),
            kind: Some(ChannelKind::Broadcast),
        };
        session.cache_peer(&peer);
        assert_eq!(session.peer(PeerId::channel(1)), Some(peer));

        assert_eq!(session.updates_state(), UpdatesState::default());
        session.set_update_state(UpdateState::All(UpdatesState {
            pts: 1,
            qts: 2,
            date: 3,
            seq: 4,
            channels: vec![
                ChannelState { id: 5, pts: 6 },
                ChannelState { id: 7, pts: 8 },
            ],
        }));
        session.set_update_state(UpdateState::Primary {
            pts: 2,
            date: 4,
            seq: 5,
        });
        session.set_update_state(UpdateState::Secondary { qts: 3 });
        session.set_update_state(UpdateState::Channel { id: 7, pts: 9 });
        assert_eq!(
            session.updates_state(),
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
