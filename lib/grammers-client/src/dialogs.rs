use std::collections::HashMap;
use std::convert::TryInto;
use std::io;

use grammers_tl_types as tl;

use crate::types;
use crate::Client;

const MAX_DIALOGS_PER_REQUEST: i32 = 100;

pub struct Dialogs {
    batch_stack: Vec<types::Dialog>,
    total: Option<usize>,
    done: bool,
    entities: HashMap<i32, types::Entity>,
    messages: HashMap<(i32, i32), tl::enums::Message>,
    request: tl::functions::messages::GetDialogs,
}

// TODO more reusable methods to get ids from things
fn peer_id(peer: &tl::enums::Peer) -> i32 {
    match peer {
        tl::enums::Peer::PeerUser(user) => user.user_id,
        tl::enums::Peer::PeerChat(chat) => chat.chat_id,
        tl::enums::Peer::PeerChannel(channel) => channel.channel_id,
    }
}

fn message_id(message: &tl::enums::Message) -> Option<(i32, i32)> {
    match message {
        tl::enums::Message::Message(message) => {
            // TODO this will probably fail in pm
            Some((peer_id(&message.to_id), message.id))
        }
        tl::enums::Message::MessageService(message) => Some((peer_id(&message.to_id), message.id)),
        tl::enums::Message::MessageEmpty(_) => None,
    }
}

impl Dialogs {
    pub fn iter() -> Self {
        Self {
            batch_stack: Vec::with_capacity(MAX_DIALOGS_PER_REQUEST as usize),
            total: None,
            done: false,
            entities: HashMap::new(),
            messages: HashMap::new(),
            request: tl::functions::messages::GetDialogs {
                exclude_pinned: false,
                folder_id: None,
                offset_date: 0,
                offset_id: 0,
                offset_peer: tl::types::InputPeerEmpty {}.into(),
                limit: MAX_DIALOGS_PER_REQUEST,
                hash: 0,
            },
        }
    }

    /// If the batch index is beyond the buffer length, it fills the buffer.
    fn should_fill_buffer(&mut self) -> bool {
        self.batch_stack.is_empty() && !self.done
    }

    fn update_user_entities(&mut self, users: Vec<tl::enums::User>) {
        users
            .into_iter()
            .filter_map(|user| {
                if let Ok(user) = user.try_into() {
                    Some(user)
                } else {
                    None
                }
            })
            .for_each(|user: tl::types::User| {
                self.entities.insert(user.id, types::Entity::User(user));
            });
    }

    fn update_chat_entities(&mut self, chats: Vec<tl::enums::Chat>) {
        chats.into_iter().for_each(|chat| match chat {
            tl::enums::Chat::Chat(chat) => {
                self.entities.insert(chat.id, types::Entity::Chat(chat));
            }
            tl::enums::Chat::Channel(channel) => {
                self.entities
                    .insert(channel.id, types::Entity::Channel(channel));
            }
            _ => {}
        });
    }

    fn update_messages(&mut self, messages: Vec<tl::enums::Message>) {
        messages.into_iter().for_each(|message| {
            if let Some(id) = message_id(&message) {
                self.messages.insert(id, message);
            }
        });
    }

    fn update_dialogs(&mut self, dialogs: Vec<tl::enums::Dialog>) {
        dialogs
            .into_iter()
            .rev()
            .for_each(move |dialog| match dialog {
                tl::enums::Dialog::Dialog(dialog) => {
                    let peer_id = peer_id(&dialog.peer);
                    if let Some(entity) = self.entities.remove(&peer_id) {
                        let last_message = self.messages.remove(&(peer_id, dialog.top_message));
                        self.batch_stack.push(types::Dialog {
                            dialog,
                            entity,
                            last_message,
                        });
                    }
                }
                tl::enums::Dialog::DialogFolder(_) => {}
            });
    }

    fn update_request_offsets(&mut self) {
        if let Some(dialog) = self.batch_stack.get(0) {
            self.request.offset_peer = dialog.entity.to_input_peer();
        }

        // Find last dialog with a message
        for dialog in self.batch_stack.iter() {
            if let Some(message) = &dialog.last_message {
                match message {
                    tl::enums::Message::Message(message) => {
                        self.request.offset_id = message.id;
                        self.request.offset_date = message.date;
                    }
                    tl::enums::Message::MessageService(message) => {
                        self.request.offset_id = message.id;
                        self.request.offset_date = message.date;
                    }
                    tl::enums::Message::MessageEmpty(message) => {
                        self.request.offset_id = message.id;
                    }
                }
                break;
            }
        }
    }

    fn fill_buffer(&mut self, client: &mut Client) -> io::Result<()> {
        match client.invoke(&self.request)?? {
            tl::enums::messages::Dialogs::Dialogs(tl::types::messages::Dialogs {
                dialogs,
                messages,
                chats,
                users,
            }) => {
                self.total = Some(dialogs.len());
                self.done = true;
                self.update_user_entities(users);
                self.update_chat_entities(chats);
                self.update_messages(messages);
                self.update_dialogs(dialogs);
            }
            tl::enums::messages::Dialogs::DialogsSlice(tl::types::messages::DialogsSlice {
                count,
                dialogs,
                messages,
                chats,
                users,
            }) => {
                self.total = Some(count as usize);
                self.done = dialogs.len() < self.request.limit as usize;
                self.update_user_entities(users);
                self.update_chat_entities(chats);
                self.update_messages(messages);
                self.update_dialogs(dialogs);
                self.update_request_offsets();
            }
            tl::enums::messages::Dialogs::DialogsNotModified(dialogs) => {
                self.total = Some(dialogs.count as usize);
                self.done = true;
            }
        }
        Ok(())
    }

    pub fn next(&mut self, client: &mut Client) -> Result<Option<types::Dialog>, io::Error> {
        if self.should_fill_buffer() {
            self.fill_buffer(client)?;
        }

        Ok(self.batch_stack.pop())
    }
}
