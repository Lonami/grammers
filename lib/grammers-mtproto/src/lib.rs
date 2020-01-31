//! This library is an implementation of the [Mobile Transport Protocol].
//!
//! It is capable of efficiently packing enqueued requests into message
//! containers to later be encrypted and transmitted, and processing the
//! server responses to maintain a correct state.
//!
//! [Mobile Transport Protocol]: https://core.telegram.org/mtproto
pub mod errors;
mod manual_tl;
pub mod transports;
mod utils;

use crate::errors::{DeserializeError, EnqueueError, RequestError};

use std::collections::VecDeque;
use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use getrandom::getrandom;
use grammers_crypto::{decrypt_data_v2, encrypt_data_v2, AuthKey};
use grammers_tl_types::{self as tl, Deserializable, Identifiable, Serializable};

/// The default compression threshold to be used.
pub const DEFAULT_COMPRESSION_THRESHOLD: Option<usize> = Some(512);

/// A builder to configure [`MTProto`] instances.
///
/// Use the [`MTProto::build`] method to create builder instances.
///
/// [`MTProto`]: struct.mtproto.html
/// [`MTProto::build`]: fn.mtproto.build.html
pub struct MTProtoBuilder {
    compression_threshold: Option<usize>,
    auth_key: Option<AuthKey>,
}

/// An implementation of the [Mobile Transport Protocol].
///
/// When working with unencrypted data (for example, when generating a new
/// authorization key), the [`serialize_plain_message`] may be used to wrap
/// a serialized request inside a message.
///
/// When working with encrypted data (every other time besides generating
/// authorization keys), use [`enqueue_request`] with the serialized request
/// that you want to send. You may enqueue as many requests as you want.
///
/// Once your transport is ready to send more data, use
/// [`serialize_encrypted_messages`] to pop requests from the queue,
/// serialize them, and encrypt them. This data can be sent to the server.
///
/// Once your transport receives some data, use [`process_encrypted_response`]
/// to decrypt, deserialize and process the server's response. This response
/// may contain replies to your previously-sent requests.
///
/// Finally, [`poll_response`] can be used to poll for server's responses to
/// your previously-sent requests.
///
/// When server responses
///
/// [Mobile Transport Protocol]: https://core.telegram.org/mtproto
/// [`serialize_plain_message`]: #method.serialize_plain_message
/// [`enqueue_request`]: #method.enqueue_request
/// [`serialize_encrypted_messages`]: #method.serialize_encrypted_messages
/// [`process_encrypted_response`]: #method.process_encrypted_response
/// [`poll_response`]: #method.poll_response
pub struct MTProto {
    /// The authorization key to use to encrypt payload.
    auth_key: Option<AuthKey>,

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

/// A Message Identifier.
///
/// When requests are enqueued, a new associated message identifier is
/// returned. As server responses get processed, some of them will be a
/// response to a previous request. You can now  `pop_response` to get
/// all the server responses, and if one matches your original identifier,
/// you will know the response corresponds to it.
#[derive(Copy, Clone, Debug, Hash, PartialEq)]
pub struct MsgId(i64);

impl MTProtoBuilder {
    fn new() -> Self {
        Self {
            compression_threshold: DEFAULT_COMPRESSION_THRESHOLD,
            auth_key: None,
        }
    }

    /// Configures the compression threshold for outgoing messages.
    pub fn compression_threshold(mut self, threshold: Option<usize>) -> Self {
        self.compression_threshold = threshold;
        self
    }

    /// Sets the authorization key to be used. Otherwise, no authorization
    /// key will be present, and a new one will have to be generated before
    /// being able to create encrypted messages.
    pub fn auth_key(mut self, auth_key: AuthKey) -> Self {
        self.auth_key = Some(auth_key);
        self
    }

    /// Finishes the builder and returns the `MTProto` instance with all
    /// the configuration changes applied.
    pub fn finish(self) -> MTProto {
        let mut result = MTProto::new();
        result.compression_threshold = self.compression_threshold;
        result.auth_key = self.auth_key;
        result
    }
}

impl MTProto {
    // Constructors
    // ========================================

    /// Creates a new instance with default settings.
    pub fn new() -> Self {
        let client_id = {
            let mut buffer = [0u8; 8];
            getrandom(&mut buffer).expect("failed to generate a secure client_id");
            i64::from_le_bytes(buffer)
        };

        Self {
            auth_key: None,
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

    // State management
    // ========================================

    /// Sets a generated authorization key as the current one, and also
    /// updates the time offset to be correct.
    ///
    /// An authorization key must be set before calling the
    /// [`serialize_encrypted_messages`] and [`process_encrypted_response`]
    /// methods, or they will return a missing key error.
    ///
    /// [`serialize_encrypted_messages`]: #method.serialize_encrypted_messages
    /// [`process_encrypted_response`]: #method.process_encrypted_response
    pub fn set_auth_key(&mut self, auth_key: AuthKey, time_offset: i32) {
        self.auth_key = Some(auth_key);
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

    // Plain requests
    // ========================================

    /// Wraps a request's data into a plain message (also known as
    /// [unencrypted messages]), and returns its serialized contents.
    ///
    /// Plain messages may be used for requests that don't require an
    /// authorization key to be present, such as those needed to generate
    /// the authorization key itself.
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

    /// Validates that the returned data is a correct plain message, and
    /// if it is, the method returns the inner contents of the message.
    ///
    /// [`serialize_plain_message`]: #method.serialize_plain_message
    pub fn deserialize_plain_message<'a>(&self, message: &'a [u8]) -> io::Result<&'a [u8]> {
        utils::check_message_buffer(message)?;

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

    // Encrypted requests
    // ========================================

    /// Enqueues a request and returns its associated `msg_id`.
    ///
    /// Once a response arrives and it is decrypted, the caller is expected
    /// to compare the `msg_id` against the `msg_id` of previously enqueued
    /// requests to determine to which of these the response belongs.
    ///
    /// If a certain amount of time passes and the enqueued request has not
    /// been sent yet, the message ID will become stale and Telegram will
    /// reject it. The caller is then expected to re-enqueue their request
    /// and send a new encrypted message.
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

    fn pop_queued_messages(&mut self) -> Option<Vec<u8>> {
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

        // Allocate enough size for the final message:
        //     8 bytes `salt` + 8 bytes `client_id` + `batch_size` bytes body
        let mut buf = io::Cursor::new(Vec::with_capacity(8 + 8 + batch_size));

        // All of the `serialize(...).unwrap()` are safe because we have an
        // in-memory buffer.
        self.salt.serialize(&mut buf).unwrap();
        self.client_id.serialize(&mut buf).unwrap();

        // If we're sending more than one, write the `MessageContainer` header.
        // This should be the moral equivalent of `MessageContainer.serialize(...)`.
        if batch_len > 1 {
            // This should be the moral equivalent of `enqueue_body`
            // and `Message::serialize`.
            let msg_id = self.get_new_msg_id();
            let seq_no = self.get_seq_no(false);

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

        // Pop `batch_len` requests and append them to the final message.
        (0..batch_len).for_each(|_| {
            // Safe to unwrap because the length cannot exceed the queue's.
            let message = self.message_queue.pop_front().unwrap();
            message.serialize(&mut buf).unwrap();
        });

        // Our message is ready.
        Some(buf.into_inner())
    }

    /// If there is one or more requests enqueued, this method will pack as
    /// many as possible into a single message, serialize it, and return it
    /// encrypted using the current authorization key as indicated by the
    /// [MTProto 2.0 guidelines].
    ///
    /// The function will fail if there is no valid authorization key present.
    ///
    /// If there are no enqueued requests, the method succeeds but returns
    /// `None`.
    ///
    /// [MTProto 2.0 guidelines]: https://core.telegram.org/mtproto/description.
    pub fn serialize_encrypted_messages(&mut self) -> Result<Option<Vec<u8>>, io::Error> {
        // Attempting to encrypt something with an authorization key full of
        // zeros will cause Telegram to reply with -404, presumably for no
        // authorization key.
        if self.auth_key.is_none() {
            // TODO proper error
            return Err(io::Error::new(io::ErrorKind::NotFound, "no auth_key found"));
        }

        Ok(self
            .pop_queued_messages()
            .map(|payload| encrypt_data_v2(&payload, self.auth_key.as_ref().unwrap())))
    }

    /// Processes an encrypted response from the server. If the response
    /// contains replies to previously-sent requests, they will be enqueued,
    /// and can be polled for later with [`poll_response`].
    ///
    /// [`poll_response`]: #method.poll_response
    pub fn process_encrypted_response(&mut self, ciphertext: &[u8]) -> io::Result<()> {
        let auth_key = match &self.auth_key {
            Some(x) => x,
            // TODO proper error
            None => return Err(io::Error::new(io::ErrorKind::NotFound, "no auth_key found")),
        };

        utils::check_message_buffer(ciphertext)?;

        let plaintext = decrypt_data_v2(ciphertext, auth_key)?;
        let mut buffer = io::Cursor::new(plaintext);

        let _salt = i64::deserialize(&mut buffer)?;
        let client_id = i64::deserialize(&mut buffer)?;
        if client_id != self.client_id {
            panic!("wrong session id");
        }

        self.process_message(manual_tl::Message::deserialize(&mut buffer)?)
    }

    /// Poll for responses to previously-sent Remote Procedure Calls.
    ///
    /// If there are no new server responses, the method returns `None`.
    pub fn poll_response(&mut self) -> Option<(MsgId, Result<Vec<u8>, RequestError>)> {
        self.response_queue.pop_front()
    }

    // Response handlers
    // ========================================

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

    /// Handles the result for Remote Procedure Calls:
    ///
    /// ```tl
    /// rpc_result#f35c6d01 req_msg_id:long result:bytes = RpcResult;
    /// ```
    fn handle_rpc_result(&mut self, message: &manual_tl::Message) -> io::Result<()> {
        // TODO make sure we notify about errors if any step fails in any handler
        let rpc_result = manual_tl::RpcResult::from_bytes(&message.body)?;
        let inner_constructor = rpc_result.inner_constructor()?;
        let manual_tl::RpcResult { req_msg_id, result } = rpc_result;
        let msg_id = MsgId(req_msg_id);

        match inner_constructor {
            tl::types::RpcError::CONSTRUCTOR_ID => {
                match tl::enums::RpcError::from_bytes(&result)? {
                    tl::enums::RpcError::RpcError(error) => self
                        .response_queue
                        .push_back((msg_id, Err(RequestError::RPCError(error.into())))),
                }
            }
            manual_tl::GzipPacked::CONSTRUCTOR_ID => {
                // Telegram shouldn't send compressed errors (the overhead
                // would probably outweight the benefits) so we don't check
                // that the decompressed payload is an error.
                self.response_queue.push_back((
                    msg_id,
                    Ok(manual_tl::GzipPacked::from_bytes(&result)?.decompress()?),
                ))
            }
            _ => {
                self.response_queue.push_back((msg_id, Ok(result)));
            }
        }

        Ok(())
    }

    /// Processes the inner messages of a container with many of them:
    ///
    /// ```tl
    /// msg_container#73f1f8dc messages:vector<%Message> = MessageContainer;
    /// ```
    fn handle_container(&mut self, message: &manual_tl::Message) -> io::Result<()> {
        let container = manual_tl::MessageContainer::from_bytes(&message.body)?;
        for inner_message in container.messages {
            self.process_message(inner_message)?;
        }

        Ok(())
    }

    /// Unpacks the data from a gzipped object and processes it:
    ///
    /// ```tl
    /// gzip_packed#3072cfa1 packed_data:bytes = Object;
    /// ```
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
    /// ```tl
    /// pong#347773c5 msg_id:long ping_id:long = Pong;
    /// ```
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
    /// ```tl
    /// bad_msg_notification#a7eff811 bad_msg_id:long bad_msg_seqno:int
    /// error_code:int = BadMsgNotification;
    /// ```
    ///
    /// Corrects the currently used server salt to use the right value
    /// before enqueuing the rejected message to be re-sent:
    ///
    /// ```tl
    /// bad_server_salt#edab447b bad_msg_id:long bad_msg_seqno:int
    /// error_code:int new_server_salt:long = BadMsgNotification;
    /// ```
    fn handle_bad_notification(&mut self, message: &manual_tl::Message) -> io::Result<()> {
        let bad_msg = tl::enums::BadMsgNotification::from_bytes(&message.body)?;
        let bad_msg = match bad_msg {
            tl::enums::BadMsgNotification::BadMsgNotification(x) => x,
            tl::enums::BadMsgNotification::BadServerSalt(x) => {
                self.response_queue.push_back((
                    MsgId(x.bad_msg_id),
                    Err(RequestError::BadMessage { code: x.error_code }),
                ));
                self.salt = x.new_server_salt;
                return Ok(());
            }
        };

        self.response_queue.push_back((
            MsgId(bad_msg.bad_msg_id),
            Err(RequestError::BadMessage {
                code: bad_msg.error_code,
            }),
        ));
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
    /// ```tl
    /// msg_detailed_info#276d3ec6 msg_id:long answer_msg_id:long
    /// bytes:int status:int = MsgDetailedInfo;
    /// ```
    ///
    /// ```tl
    /// msg_new_detailed_info#809db6df answer_msg_id:long
    /// bytes:int status:int = MsgDetailedInfo;
    /// ```
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
    /// ```tl
    /// new_session_created#9ec20908 first_msg_id:long unique_id:long
    /// server_salt:long = NewSession;
    /// ```
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
    /// ```tl
    /// tl::enums::MsgsAck::MsgsAck
    /// ```
    ///
    /// Normally these can be ignored except in the case of ``auth.logOut``:
    ///
    /// ```tl
    /// auth.logOut#5717da40 = Bool;
    /// ```
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
    /// ```tl
    /// future_salts#ae500895 req_msg_id:long now:int
    /// salts:vector<future_salt> = FutureSalts;
    /// ```
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

#[cfg(test)]
mod tests {
    use super::*;

    // salt + client_id
    const MESSAGE_PREFIX_LEN: usize = 8 + 8;

    // gzip_packed#3072cfa1 packed_data:string = Object;
    const GZIP_PACKED_HEADER: [u8; 4] = [0xa1, 0xcf, 0x72, 0x30];

    // msg_container#73f1f8dc messages:vector<message> = MessageContainer;
    const MSG_CONTAINER_HEADER: [u8; 4] = [0xdc, 0xf8, 0xf1, 0x73];

    #[test]
    fn ensure_buffer_used_exact_capacity() {
        {
            // Single body (no container)
            let mut mtproto = MTProto::build().compression_threshold(None).finish();

            mtproto
                .enqueue_request(vec![b'H', b'e', b'y', b'!'])
                .unwrap();
            let buffer = mtproto.pop_queued_messages().unwrap();
            assert_eq!(buffer.capacity(), buffer.len());
        }
        {
            // Multiple bodies (using a container)
            let mut mtproto = MTProto::build().compression_threshold(None).finish();

            mtproto
                .enqueue_request(vec![b'H', b'e', b'y', b'!'])
                .unwrap();
            mtproto
                .enqueue_request(vec![b'B', b'y', b'e', b'!'])
                .unwrap();
            let buffer = mtproto.pop_queued_messages().unwrap();
            assert_eq!(buffer.capacity(), buffer.len());
        }
    }

    fn ensure_buffer_is_message(buffer: &[u8], body: &[u8], seq_no: u8) {
        // buffer[0..8] is the msg_id, based on `SystemTime::now()`
        assert_ne!(&buffer[0..8], [0, 0, 0, 0, 0, 0, 0, 0]);
        // buffer[8..12] is the seq_no, ever-increasing odd number (little endian)
        assert_eq!(&buffer[8..12], [seq_no, 0, 0, 0]);
        // buffer[12..16] is the bytes, the len of the body (little endian)
        assert_eq!(&buffer[12..16], [body.len() as u8, 0, 0, 0]);
        // buffer[16..] is the body, which is padded to 4 bytes
        assert_eq!(&buffer[16..], body);
    }

    #[test]
    fn ensure_serialization_has_salt_client_id() {
        let mut mtproto = MTProto::build().compression_threshold(None).finish();

        mtproto
            .enqueue_request(vec![b'H', b'e', b'y', b'!'])
            .unwrap();
        let buffer = mtproto.pop_queued_messages().unwrap();

        // salt comes first, it's zero by default.
        assert_eq!(&buffer[0..8], [0, 0, 0, 0, 0, 0, 0, 0]);

        // client_id should be random.
        assert_ne!(&buffer[8..16], [0, 0, 0, 0, 0, 0, 0, 0]);

        // Only one message should remain.
        ensure_buffer_is_message(&buffer[MESSAGE_PREFIX_LEN..], b"Hey!", 1);
    }

    #[test]
    fn ensure_correct_single_serialization() {
        let mut mtproto = MTProto::build().compression_threshold(None).finish();

        mtproto
            .enqueue_request(vec![b'H', b'e', b'y', b'!'])
            .unwrap();

        let buffer = &mtproto.pop_queued_messages().unwrap()[MESSAGE_PREFIX_LEN..];
        ensure_buffer_is_message(&buffer, b"Hey!", 1);
    }

    #[test]
    fn ensure_correct_multi_serialization() {
        let mut mtproto = MTProto::build().compression_threshold(None).finish();

        mtproto
            .enqueue_request(vec![b'H', b'e', b'y', b'!'])
            .unwrap();
        mtproto
            .enqueue_request(vec![b'B', b'y', b'e', b'!'])
            .unwrap();
        let buffer = &mtproto.pop_queued_messages().unwrap()[MESSAGE_PREFIX_LEN..];

        // buffer[0..8] is the msg_id for the container
        assert_ne!(&buffer[0..8], [0, 0, 0, 0, 0, 0, 0, 0]);
        // buffer[8..12] is the seq_no, maybe-increasing even number.
        // after two messages (1, 3) the next non-content related is 4.
        assert_eq!(&buffer[8..12], [4, 0, 0, 0]);
        // buffer[12..16] is the bytes, the len of the body
        assert_eq!(&buffer[12..16], [48, 0, 0, 0]);

        // buffer[16..20] is the constructor id of the container
        assert_eq!(&buffer[16..20], MSG_CONTAINER_HEADER);
        // buffer[20..24] is how many messages are included
        assert_eq!(&buffer[20..24], [2, 0, 0, 0]);

        // buffer[24..44] is an inner message
        ensure_buffer_is_message(&buffer[24..44], b"Hey!", 1);

        // buffer[44..] is the other inner message
        ensure_buffer_is_message(&buffer[44..], b"Bye!", 3);
    }

    #[test]
    fn ensure_correct_single_large_serialization() {
        let mut mtproto = MTProto::build().compression_threshold(None).finish();
        let data = vec![0x7f; 768 * 1024];

        mtproto.enqueue_request(data.clone()).unwrap();
        let buffer = &mtproto.pop_queued_messages().unwrap()[MESSAGE_PREFIX_LEN..];
        assert_eq!(buffer.len(), 16 + data.len());
    }

    #[test]
    fn ensure_correct_multi_large_serialization() {
        let mut mtproto = MTProto::build().compression_threshold(None).finish();
        let data = vec![0x7f; 768 * 1024];

        mtproto.enqueue_request(data.clone()).unwrap();

        mtproto.enqueue_request(data.clone()).unwrap();

        // The messages are large enough that they should not be able to go in
        // a container together (so there are two things to pop).
        let buffer = &mtproto.pop_queued_messages().unwrap()[MESSAGE_PREFIX_LEN..];
        assert_eq!(buffer.len(), 16 + data.len());

        let buffer = &mtproto.pop_queued_messages().unwrap()[MESSAGE_PREFIX_LEN..];
        assert_eq!(buffer.len(), 16 + data.len());
    }

    #[test]
    fn ensure_queue_is_clear() {
        let mut mtproto = MTProto::build().compression_threshold(None).finish();

        assert!(mtproto.pop_queued_messages().is_none());
        mtproto
            .enqueue_request(vec![b'H', b'e', b'y', b'!'])
            .unwrap();

        assert!(mtproto.pop_queued_messages().is_some());
        assert!(mtproto.pop_queued_messages().is_none());
    }

    #[test]
    fn ensure_large_payload_errors() {
        let mut mtproto = MTProto::build().compression_threshold(None).finish();

        assert!(match mtproto.enqueue_request(vec![0; 2 * 1024 * 1024]) {
            Err(EnqueueError::PayloadTooLarge) => true,
            _ => false,
        });

        assert!(mtproto.pop_queued_messages().is_none());

        // Make sure the queue is not in a broken state
        mtproto
            .enqueue_request(vec![b'H', b'e', b'y', b'!'])
            .unwrap();

        assert_eq!(
            mtproto.pop_queued_messages().unwrap().len(),
            20 + MESSAGE_PREFIX_LEN
        );
    }

    #[test]
    fn ensure_non_padded_payload_errors() {
        let mut mtproto = MTProto::build().compression_threshold(None).finish();

        assert!(match mtproto.enqueue_request(vec![1, 2, 3]) {
            Err(EnqueueError::IncorrectPadding) => true,
            _ => false,
        });

        assert!(mtproto.pop_queued_messages().is_none());

        // Make sure the queue is not in a broken state
        mtproto
            .enqueue_request(vec![b'H', b'e', b'y', b'!'])
            .unwrap();

        assert_eq!(
            mtproto.pop_queued_messages().unwrap().len(),
            20 + MESSAGE_PREFIX_LEN
        );
    }

    #[test]
    fn ensure_no_compression_is_honored() {
        // A large vector of null bytes should compress
        let mut mtproto = MTProto::build().compression_threshold(None).finish();
        mtproto.enqueue_request(vec![0; 512 * 1024]).unwrap();
        let buffer = mtproto.pop_queued_messages().unwrap();
        assert!(!buffer.windows(4).any(|w| w == GZIP_PACKED_HEADER));
    }

    #[test]
    fn ensure_some_compression() {
        // A large vector of null bytes should compress
        {
            // High threshold not reached, should not compress
            let mut mtproto = MTProto::build()
                .compression_threshold(Some(768 * 1024))
                .finish();
            mtproto.enqueue_request(vec![0; 512 * 1024]).unwrap();
            let buffer = mtproto.pop_queued_messages().unwrap();
            assert!(!buffer.windows(4).any(|w| w == GZIP_PACKED_HEADER));
        }
        {
            // Low threshold is exceeded, should compress
            let mut mtproto = MTProto::build()
                .compression_threshold(Some(256 * 1024))
                .finish();
            mtproto.enqueue_request(vec![0; 512 * 1024]).unwrap();
            let buffer = mtproto.pop_queued_messages().unwrap();
            assert!(buffer.windows(4).any(|w| w == GZIP_PACKED_HEADER));
        }
        {
            // The default should compress
            let mut mtproto = MTProto::new();
            mtproto.enqueue_request(vec![0; 512 * 1024]).unwrap();
            let buffer = mtproto.pop_queued_messages().unwrap();
            assert!(buffer.windows(4).any(|w| w == GZIP_PACKED_HEADER));
        }
    }
}
