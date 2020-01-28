pub mod auth_key;
pub mod errors;
mod manual_tl;
pub mod transports;

use crate::errors::{DeserializeError, EnqueueError};

use std::collections::VecDeque;
use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use getrandom::getrandom;
use grammers_crypto::{decrypt_data_v2, encrypt_data_v2, AuthKey};
use grammers_tl_types::{self as tl, Deserializable, Identifiable, Serializable};

const DEFAULT_COMPRESSION_THRESHOLD: Option<usize> = Some(512);

/// A builder to configure `MTProto` instances.
pub struct MTProtoBuilder {
    compression_threshold: Option<usize>,
}

/// This structure holds everything needed by the Mobile Transport Protocol.
pub struct MTProto {
    /// The authorization key to use to encrypt payload.
    auth_key: AuthKey,

    /// The time offset from the server's time, in seconds.
    time_offset: i32,

    /// The current salt to be used when encrypting payload.
    salt: i64,

    /// The secure, random identifier for this instance.
    client_id: i64,

    /// The current message sequence number.
    sequence: i32,

    /// The ID of the last message.
    last_msg_id: i64,

    /// A queue of messages that are pending from being sent.
    message_queue: VecDeque<manual_tl::Message>,

    /// Identifiers that need to be acknowledged to the server.
    pending_ack: Vec<i64>,

    /// If present, the threshold in bytes at which a message will be
    /// considered large enough to attempt compressing it. Otherwise,
    /// outgoing messages will never be compressed.
    compression_threshold: Option<usize>,

    /// A queue of responses ready to be used.
    response_queue: VecDeque<(MsgId, Result<Vec<u8>, RequestError>)>,
}

/// This error occurs when a Remote Procedure call was unsuccessful.
/// The request should be retransmited when this happens, unless the
/// variant is `InvalidParameters`.
pub enum RequestError {
    /// The parameters used in the request were invalid and caused a
    /// Remote Procedure Call error.
    InvalidParameters { error: tl::types::RpcError },

    // TODO be more specific
    /// A different error occured.
    Other,
}

#[derive(Copy, Clone, Debug, Hash, PartialEq)]
pub struct MsgId(i64);

impl MTProtoBuilder {
    fn new() -> Self {
        Self {
            compression_threshold: DEFAULT_COMPRESSION_THRESHOLD,
        }
    }

    /// Configures the compression threshold for outgoing messages.
    pub fn compression_threshold(mut self, threshold: Option<usize>) -> Self {
        self.compression_threshold = threshold;
        self
    }

    /// Finishes the builder and returns the `MTProto` instance with all
    /// the configuration changes applied.
    pub fn finish(self) -> MTProto {
        let mut result = MTProto::new();
        result.compression_threshold = self.compression_threshold;
        result
    }
}

impl MTProto {
    /// Creates a new instance with default settings.
    pub fn new() -> Self {
        let client_id = {
            let mut buffer = [0u8; 8];
            getrandom(&mut buffer).expect("failed to generate a secure client_id");
            i64::from_le_bytes(buffer)
        };

        Self {
            auth_key: AuthKey::from_bytes([0; 256]),
            time_offset: 0,
            salt: 0,
            client_id,
            sequence: 0,
            last_msg_id: 0,
            message_queue: VecDeque::new(),
            pending_ack: vec![],
            compression_threshold: DEFAULT_COMPRESSION_THRESHOLD,
            response_queue: VecDeque::new(),
        }
    }

    /// Returns a builder to configure certain parameters.
    pub fn build() -> MTProtoBuilder {
        MTProtoBuilder::new()
    }

    /// Sets a generated authorization key as the current one, and also
    /// updates the time offset to be correct.
    pub fn set_auth_key(&mut self, auth_key: AuthKey, time_offset: i32) {
        self.auth_key = auth_key;
        self.time_offset = time_offset;
    }

    /// Correct our time offset based on a known valid message ID.
    fn correct_time_offset(&mut self, msg_id: i64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is before epoch")
            .as_secs() as i32;

        let correct = (msg_id >> 32) as i32;
        self.time_offset = correct - now;
    }

    /// Enqueues a request and returns its associated `msg_id`.
    ///
    /// Once a response arrives and it is decrypted, the caller
    /// is expected to compare the `msg_id` against previously
    /// enqueued requests to determine to which of these the
    /// response belongs.
    ///
    /// If a certain amount of time passes and the enqueued
    /// request has not been sent yet, the message ID will
    /// become stale and Telegram will reject it.
    pub fn enqueue_request(&mut self, mut body: Vec<u8>) -> Result<MsgId, EnqueueError> {
        if body.len() + manual_tl::Message::SIZE_OVERHEAD
            > manual_tl::MessageContainer::MAXIMUM_SIZE
        {
            return Err(EnqueueError::PayloadTooLarge);
        }
        if body.len() % 4 != 0 {
            return Err(EnqueueError::IncorrectPadding);
        }

        // Payload from the outside is always considered to be
        // content-related, which means we can apply compression.
        if let Some(threshold) = self.compression_threshold {
            if body.len() >= threshold {
                let compressed = manual_tl::GzipPacked::new(&body).to_bytes();
                if compressed.len() < body.len() {
                    body = compressed;
                }
            }
        }

        Ok(self.enqueue_body(body, true))
    }

    /// Enqueues a request to be executed by the server sequentially after
    /// another specific request.
    pub fn enqueue_sequential_request(
        &mut self,
        body: Vec<u8>,
        after: &MsgId,
    ) -> Result<MsgId, EnqueueError> {
        self.enqueue_request(
            tl::functions::InvokeAfterMsg {
                msg_id: after.0,
                query: body,
            }
            .to_bytes(),
        )
    }

    fn enqueue_body(&mut self, body: Vec<u8>, content_related: bool) -> MsgId {
        let msg_id = self.get_new_msg_id();
        let seq_no = self.get_seq_no(content_related);
        self.message_queue.push_back(manual_tl::Message {
            msg_id,
            seq_no,
            body,
        });

        MsgId(msg_id)
    }

    // TODO probably find a better name for this
    /// Pops as many enqueued requests as possible, and returns
    /// the serialized data. If there are no requests enqueued,
    /// this returns `None`.
    pub fn pop_queue(&mut self) -> Option<Vec<u8>> {
        // If we need to acknowledge messages, this notification goes
        // in with the rest of requests so that we can also include it.
        if !self.pending_ack.is_empty() {
            let msg_ids = std::mem::take(&mut self.pending_ack);
            self.enqueue_body(
                tl::enums::MsgsAck::MsgsAck(tl::types::MsgsAck { msg_ids }).to_bytes(),
                false,
            );
        }

        // If there is nothing in the queue, we don't have to do any work.
        if self.message_queue.is_empty() {
            return None;
        }

        // Try to pop as many messages as we possibly can fit in a container.
        // This will reduce overhead from encryption and outer network calls.
        let mut batch_size = 0;

        // Count how many messages we can send in a single batch
        // and determine the size needed to serialize all of them.
        //
        // We can batch `MAXIMUM_LENGTH` requests at most,
        // and their size cannot exceed `MAXIMUM_SIZE`.
        let batch_len = self
            .message_queue
            .iter()
            .take(manual_tl::MessageContainer::MAXIMUM_LENGTH)
            .take_while(|message| {
                if batch_size + message.size() < manual_tl::MessageContainer::MAXIMUM_SIZE {
                    batch_size += message.size();
                    true
                } else {
                    false
                }
            })
            .count();

        // If we're sending more than one, add room for the
        // `MessageContainer` header and its own message too.
        if batch_len > 1 {
            batch_size +=
                manual_tl::Message::SIZE_OVERHEAD + manual_tl::MessageContainer::SIZE_OVERHEAD;
        }

        // Allocate enough size and pop that many requests
        let mut buf = io::Cursor::new(Vec::with_capacity(batch_size));

        // If we're sending more than one, write the `MessageContainer` header.
        // This should be the moral equivalent of `MessageContainer.serialize(...)`.
        if batch_len > 1 {
            // This should be the moral equivalent of `enqueue_body`
            // and `Message::serialize`.
            let msg_id = self.get_new_msg_id();
            let seq_no = self.get_seq_no(false);

            // Safe to unwrap because we're serializing into a memory buffer.
            msg_id.serialize(&mut buf).unwrap();
            seq_no.serialize(&mut buf).unwrap();
            ((batch_size - manual_tl::Message::SIZE_OVERHEAD) as i32)
                .serialize(&mut buf)
                .unwrap();

            manual_tl::MessageContainer::CONSTRUCTOR_ID
                .serialize(&mut buf)
                .unwrap();
            (batch_len as i32).serialize(&mut buf).unwrap();
        }

        // Finally, pop that many requests and write them to the buffer.
        (0..batch_len).for_each(|_| {
            // Safe to unwrap because the length cannot exceed the queue's.
            let message = self.message_queue.pop_front().unwrap();
            // Safe to unwrap because we're serializing into a memory buffer.
            message.serialize(&mut buf).unwrap();
        });

        // The buffer is full, encrypt it and return the data ready to be
        // sent over the network!
        Some(buf.into_inner())
    }

    /// Generates a new unique message ID based on the current
    /// time (in ms) since epoch, applying a known time offset.
    fn get_new_msg_id(&mut self) -> i64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is before epoch");

        let seconds = (now.as_secs() as i32 + self.time_offset) as u64;
        let nanoseconds = now.subsec_nanos() as u64;
        let mut new_msg_id = ((seconds << 32) | (nanoseconds << 2)) as i64;

        if self.last_msg_id >= new_msg_id {
            new_msg_id = self.last_msg_id + 4;
        }

        self.last_msg_id = new_msg_id;
        new_msg_id
    }

    /// Generates the next sequence number depending on whether
    /// it should be for a content-related query or not.
    fn get_seq_no(&mut self, content_related: bool) -> i32 {
        if content_related {
            let result = self.sequence * 2 + 1;
            self.sequence += 1;
            result
        } else {
            self.sequence * 2
        }
    }

    /// A plain message's structure is different from the structure of
    /// messages that are meant to be encrypted, and are made up of:
    ///
    /// ```text
    /// [auth_key_id = 0] [   message_id  ] [ msg len ] [ message data ... ]
    /// [    64 bits    ] [    64 bits    ] [ 32 bits ] [       ...        ]
    /// ```
    ///
    /// They are also known as [unencrypted messages].
    ///
    /// [unencrypted messages]: https://core.telegram.org/mtproto/description#unencrypted-message
    pub fn serialize_plain_message(&mut self, body: &[u8]) -> Vec<u8> {
        let mut buf = io::Cursor::new(Vec::with_capacity(body.len() + 8 + 8 + 4));
        // Safe to unwrap because we're serializing into a memory buffer.
        0i64.serialize(&mut buf).unwrap();
        self.get_new_msg_id().serialize(&mut buf).unwrap();
        (body.len() as u32).serialize(&mut buf).unwrap();
        buf.write_all(&body).unwrap();
        buf.into_inner()
    }

    /// The opposite of `serialize_plain_message`. It validates that the
    /// returned data is valid.
    pub fn deserialize_plain_message<'a>(&self, message: &'a [u8]) -> io::Result<&'a [u8]> {
        if message.len() == 4 {
            // Probably a negative HTTP error code
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                DeserializeError::HTTPErrorCode {
                    // Safe to unwrap because we just checked the length
                    code: i32::from_bytes(message).unwrap(),
                },
            ));
        } else if message.len() < 20 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                DeserializeError::MessageBufferTooSmall,
            ));
        }

        let mut buf = io::Cursor::new(message);
        let auth_key_id = i64::deserialize(&mut buf)?;
        if auth_key_id != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                DeserializeError::BadAuthKey {
                    got: auth_key_id,
                    expected: 0,
                },
            ));
        }

        let msg_id = i64::deserialize(&mut buf)?;
        if msg_id == 0 {
            // TODO make sure it's close to our system time
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                DeserializeError::BadMessageId { got: msg_id },
            ));
        }

        let len = i32::deserialize(&mut buf)?;
        if len <= 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                DeserializeError::NegativeMessageLength { got: len },
            ));
        }
        if (20 + len) as usize > message.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                DeserializeError::TooLongMessageLength {
                    got: len as usize,
                    max_length: message.len() - 20,
                },
            ));
        }

        Ok(&message[20..20 + len as usize])
    }

    /// Encrypts the data returned by `pop_queue` to be able to send it
    /// over the network, using the current authorization key as indicated
    /// by the [MTProto 2.0 guidelines].
    ///
    /// [MTProto 2.0 guidelines]: https://core.telegram.org/mtproto/description.
    pub fn encrypt_message_data(&self, plaintext: Vec<u8>) -> Vec<u8> {
        // TODO maybe this should be part of `pop_queue` which has the entire buffer.
        //      IIRC plaintext mtproto also needs this but with salt 0
        let mut buffer = io::Cursor::new(Vec::with_capacity(8 + 8 + plaintext.len()));

        // Prepend salt (8 bytes) and client_id (8 bytes) to the plaintext
        // Safe to unwrap because we have an in-memory buffer
        self.salt.serialize(&mut buffer).unwrap();
        self.client_id.serialize(&mut buffer).unwrap();
        buffer.write_all(&plaintext).unwrap();
        let plaintext = buffer.into_inner();

        encrypt_data_v2(&plaintext, &self.auth_key)
    }

    /// Decrypts a response packet and returns the inner message.
    fn decrypt_response(&self, ciphertext: &[u8]) -> io::Result<manual_tl::Message> {
        let plaintext = decrypt_data_v2(ciphertext, &self.auth_key)?;
        let mut buffer = io::Cursor::new(plaintext);

        let _salt = i64::deserialize(&mut buffer)?;
        let client_id = i64::deserialize(&mut buffer)?;
        if client_id != self.client_id {
            panic!("wrong session id");
        }

        manual_tl::Message::deserialize(&mut buffer)
    }

    /// Processes an encrypted response from the server. If the
    /// response belonged to a previous request, it is returned.
    pub fn process_response(&mut self, ciphertext: &[u8]) -> io::Result<()> {
        self.process_message(self.decrypt_response(ciphertext)?)
    }

    fn process_message(&mut self, message: manual_tl::Message) -> io::Result<()> {
        self.pending_ack.push(message.msg_id);

        // Determine what to do based on the inner body's constructor
        match message.constructor_id()? {
            manual_tl::RpcResult::CONSTRUCTOR_ID => self.handle_rpc_result(&message),
            manual_tl::MessageContainer::CONSTRUCTOR_ID => self.handle_container(&message),
            manual_tl::GzipPacked::CONSTRUCTOR_ID => self.handle_gzip_packed(&message),
            tl::types::Pong::CONSTRUCTOR_ID => self.handle_pong(&message),
            tl::types::BadServerSalt::CONSTRUCTOR_ID => self.handle_bad_notification(&message),
            tl::types::BadMsgNotification::CONSTRUCTOR_ID => self.handle_bad_notification(&message),
            tl::types::MsgDetailedInfo::CONSTRUCTOR_ID => self.handle_detailed_info(&message),
            tl::types::MsgNewDetailedInfo::CONSTRUCTOR_ID => self.handle_detailed_info(&message),
            tl::types::NewSessionCreated::CONSTRUCTOR_ID => {
                self.handle_new_session_created(&message)
            }
            tl::types::MsgsAck::CONSTRUCTOR_ID => self.handle_ack(&message),
            tl::types::FutureSalts::CONSTRUCTOR_ID => self.handle_future_salts(&message),
            tl::types::MsgsStateReq::CONSTRUCTOR_ID => self.handle_state_forgotten(&message),
            tl::types::MsgResendReq::CONSTRUCTOR_ID => self.handle_state_forgotten(&message),
            tl::types::MsgsAllInfo::CONSTRUCTOR_ID => self.handle_msg_all(&message),
            _ => self.handle_update(&message),
        }
    }

    // TODO process_response and pop_response? maybe one should be named "answer" or "reply"
    /// Pop a response to a remote procedure call.
    pub fn pop_response(&mut self) -> Option<(MsgId, Result<Vec<u8>, RequestError>)> {
        self.response_queue.pop_front()
    }

    /// Handles the result for Remote Procedure Calls:
    ///
    ///     rpc_result#f35c6d01 req_msg_id:long result:bytes = RpcResult;
    fn handle_rpc_result(&mut self, message: &manual_tl::Message) -> io::Result<()> {
        let rpc_result = manual_tl::RpcResult::from_bytes(&message.body)?;
        let manual_tl::RpcResult { req_msg_id, result } = rpc_result;
        // TODO handle the body being RpcError (return some enum variant)
        // TODO acknowledge the message id if it's an error
        // TODO handle the body being GzipPacked (decompress it before return)
        self.response_queue
            .push_back((MsgId(req_msg_id), Ok(result)));
        Ok(())
    }

    /// Processes the inner messages of a container with many of them:
    ///
    ///     msg_container#73f1f8dc messages:vector<%Message> = MessageContainer;
    fn handle_container(&mut self, message: &manual_tl::Message) -> io::Result<()> {
        let container = manual_tl::MessageContainer::from_bytes(&message.body)?;
        for inner_message in container.messages {
            self.process_message(inner_message)?;
        }

        Ok(())
    }

    /// Unpacks the data from a gzipped object and processes it:
    ///
    ///     gzip_packed#3072cfa1 packed_data:bytes = Object;
    fn handle_gzip_packed(&mut self, message: &manual_tl::Message) -> io::Result<()> {
        // TODO custom error, don't use a string
        let container = manual_tl::GzipPacked::from_bytes(&message.body)?;
        self.process_message(manual_tl::Message {
            body: container
                .decompress()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "decompression failed"))?,
            ..*message
        })
        .map(|_| ())
    }

    /// Handles pong results, which don't come inside a ``rpc_result``
    /// but are still sent through a request:
    ///
    ///     pong#347773c5 msg_id:long ping_id:long = Pong;
    fn handle_pong(&mut self, message: &manual_tl::Message) -> io::Result<()> {
        let pong = tl::enums::Pong::from_bytes(&message.body)?;
        let pong = match pong {
            tl::enums::Pong::Pong(x) => x,
        };

        self.response_queue
            .push_back((MsgId(pong.msg_id), Ok(message.body.clone())));
        Ok(())
    }

    /// Adjusts the current state to be correct based on the
    /// received bad message notification whenever possible:
    ///
    ///     bad_msg_notification#a7eff811 bad_msg_id:long bad_msg_seqno:int
    ///     error_code:int = BadMsgNotification;
    ///
    /// Corrects the currently used server salt to use the right value
    /// before enqueuing the rejected message to be re-sent:
    ///
    ///     bad_server_salt#edab447b bad_msg_id:long bad_msg_seqno:int
    ///     error_code:int new_server_salt:long = BadMsgNotification;
    fn handle_bad_notification(&mut self, message: &manual_tl::Message) -> io::Result<()> {
        let bad_msg = tl::enums::BadMsgNotification::from_bytes(&message.body)?;
        let bad_msg = match bad_msg {
            tl::enums::BadMsgNotification::BadMsgNotification(x) => x,
            tl::enums::BadMsgNotification::BadServerSalt(x) => {
                self.response_queue
                    .push_back((MsgId(x.bad_msg_id), Err(RequestError::Other)));
                self.salt = x.new_server_salt;
                return Ok(());
            }
        };

        self.response_queue
            .push_back((MsgId(bad_msg.bad_msg_id), Err(RequestError::Other)));
        match bad_msg.error_code {
            16 => {
                // Sent `msg_id` was too low (our `time_offset` is wrong).
                self.correct_time_offset(message.msg_id);
            }
            17 => {
                // Sent `msg_id` was too high (our `time_offset` is wrong).
                self.correct_time_offset(message.msg_id);
            }
            32 => {
                // Sent `seq_no` was too low. Bump it by some large-ish value.
                // TODO start with a fresh session rather than guessing
                self.sequence += 64;
            }
            33 => {
                // Sent `seq_no` was too high (this error doesn't seem to occur).
                // TODO start with a fresh session rather than guessing
                self.sequence -= 16;
            }
            _ => {
                // Just notify about it.
            }
        }

        Ok(())
    }

    /// Updates the current status with the received detailed information:
    ///
    ///     msg_detailed_info#276d3ec6 msg_id:long answer_msg_id:long
    ///     bytes:int status:int = MsgDetailedInfo;
    ///
    ///     msg_new_detailed_info#809db6df answer_msg_id:long
    ///     bytes:int status:int = MsgDetailedInfo;
    fn handle_detailed_info(&mut self, message: &manual_tl::Message) -> io::Result<()> {
        // TODO https://github.com/telegramdesktop/tdesktop/blob/8f82880b938e06b7a2a27685ef9301edb12b4648/Telegram/SourceFiles/mtproto/connection.cpp#L1790-L1820
        // TODO https://github.com/telegramdesktop/tdesktop/blob/8f82880b938e06b7a2a27685ef9301edb12b4648/Telegram/SourceFiles/mtproto/connection.cpp#L1822-L1845
        let msg_detailed = tl::enums::MsgDetailedInfo::from_bytes(&message.body)?;
        match msg_detailed {
            tl::enums::MsgDetailedInfo::MsgDetailedInfo(x) => {
                self.pending_ack.push(x.answer_msg_id);
            }
            tl::enums::MsgDetailedInfo::MsgNewDetailedInfo(x) => {
                self.pending_ack.push(x.answer_msg_id);
            }
        }
        Ok(())
    }

    /// Updates the current status with the received session information:
    ///
    ///     new_session_created#9ec20908 first_msg_id:long unique_id:long
    ///     server_salt:long = NewSession;
    fn handle_new_session_created(&mut self, message: &manual_tl::Message) -> io::Result<()> {
        let new_session = tl::enums::NewSession::from_bytes(&message.body)?;
        match new_session {
            tl::enums::NewSession::NewSessionCreated(x) => {
                self.salt = x.server_salt;
            }
        }
        Ok(())
    }

    /// Handles a server acknowledge about our messages.
    ///
    ///     tl::enums::MsgsAck::MsgsAck
    ///
    /// Normally these can be ignored except in the case of ``auth.logOut``:
    ///
    ///     auth.logOut#5717da40 = Bool;
    ///
    /// Telegram doesn't seem to send its result so we need to confirm
    /// it manually. No other request is known to have this behaviour.
    ///
    /// Since the ID of sent messages consisting of a container is
    /// never returned (unless on a bad notification), this method
    /// also removes containers messages when any of their inner
    /// messages are acknowledged.
    fn handle_ack(&self, message: &manual_tl::Message) -> io::Result<()> {
        // TODO notify about this somehow
        let _ack = tl::enums::MsgsAck::from_bytes(&message.body)?;
        Ok(())
    }

    /// Handles future salt results, which don't come inside a
    /// ``rpc_result`` but are still sent through a request:
    ///
    ///     future_salts#ae500895 req_msg_id:long now:int
    ///     salts:vector<future_salt> = FutureSalts;
    fn handle_future_salts(&mut self, message: &manual_tl::Message) -> io::Result<()> {
        let salts = tl::enums::FutureSalts::from_bytes(&message.body)?;
        let salts = match salts {
            tl::enums::FutureSalts::FutureSalts(x) => x,
        };

        self.response_queue
            .push_back((MsgId(salts.req_msg_id), Ok(message.body.clone())));
        Ok(())
    }

    /// Handles both :tl:`MsgsStateReq` and :tl:`MsgResendReq` by
    /// enqueuing a :tl:`MsgsStateInfo` to be sent at a later point.
    fn handle_state_forgotten(&self, _message: &manual_tl::Message) -> io::Result<()> {
        // TODO implement
        Ok(())
    }

    /// Handles :tl:`MsgsAllInfo` by doing nothing (yet).
    fn handle_msg_all(&self, _message: &manual_tl::Message) -> io::Result<()> {
        // TODO implement
        Ok(())
    }

    fn handle_update(&self, _message: &manual_tl::Message) -> io::Result<()> {
        // TODO dispatch this somehow
        Ok(())
    }
}
