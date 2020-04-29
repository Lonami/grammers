// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::errors::{DeserializeError, RequestError};
use crate::{manual_tl, MsgId};
use getrandom::getrandom;
use grammers_crypto::{decrypt_data_v2, encrypt_data_v2, AuthKey};
use grammers_tl_types::{self as tl, Cursor, Deserializable, Identifiable, Serializable};
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

/// A builder to configure [`Mtp`] instances.
///
/// Use the [`Mtp::build`] method to create builder instances.
///
/// [`Mtp`]: struct.mtp.html
/// [`Mtp::build`]: fn.mtp.build.html
pub struct MtpBuilder {
    time_offset: i32,
    compression_threshold: Option<usize>,
}

/// An implementation of the [Mobile Transport Protocol] for ciphertext
/// (encrypted) messages.
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
pub struct Mtp {
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
    ///
    /// A [Content-related Message] is "a message requiring an explicit
    /// acknowledgment. These include all the user and many service messages,
    /// virtually all with the exception of containers and acknowledgments."
    ///
    /// [Content-related Message]: https://core.telegram.org/mtproto/description#content-related-message
    pending_ack: Vec<i64>,

    /// If present, the threshold in bytes at which a message will be
    /// considered large enough to attempt compressing it. Otherwise,
    /// outgoing messages will never be compressed.
    compression_threshold: Option<usize>,

    /// A queue of responses ready to be used.
    response_queue: VecDeque<(MsgId, Result<Vec<u8>, RequestError>)>,

    /// A queue of updates ready to be used.
    update_queue: VecDeque<Vec<u8>>,
}

impl MtpBuilder {
    fn new() -> Self {
        MtpBuilder {
            time_offset: 0,
            compression_threshold: crate::DEFAULT_COMPRESSION_THRESHOLD,
        }
    }

    /// Configures the time offset to Telegram servers.
    pub fn time_offset(mut self, offset: i32) -> Self {
        self.time_offset = offset;
        self
    }

    /// Configures the compression threshold for outgoing messages.
    pub fn compression_threshold(mut self, threshold: Option<usize>) -> Self {
        self.compression_threshold = threshold;
        self
    }

    /// Finishes the builder and returns the `MTProto` instance with all
    /// the configuration changes applied.
    pub fn finish(self, auth_key: AuthKey) -> Mtp {
        let mut result = Mtp::new(auth_key);
        result.compression_threshold = self.compression_threshold;
        result
    }
}

impl Mtp {
    // Constructors
    // ========================================

    /// Creates a new instance with default settings.
    pub fn new(auth_key: AuthKey) -> Self {
        let client_id = {
            let mut buffer = [0u8; 8];
            getrandom(&mut buffer).expect("failed to generate a secure client_id");
            i64::from_le_bytes(buffer)
        };

        Mtp {
            auth_key,
            time_offset: 0,
            salt: 0,
            client_id,
            sequence: 0,
            last_msg_id: 0,
            message_queue: VecDeque::new(),
            pending_ack: vec![],
            compression_threshold: crate::DEFAULT_COMPRESSION_THRESHOLD,
            response_queue: VecDeque::new(),
            update_queue: VecDeque::new(),
        }
    }

    /// Returns a builder to configure certain parameters.
    pub fn build() -> MtpBuilder {
        MtpBuilder::new()
    }

    // State management
    // ========================================

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
    ///
    /// # Panics
    ///
    /// The method panics if the body length is not padded to 4 bytes. The
    /// serialization of requests will always be correctly padded, so adding
    /// an error case for this rare case (impossible with the expected inputs)
    /// would simply be unnecessary.
    ///
    /// The method also panics if the body length is too large for similar
    /// reasons. It is not reasonable to construct huge requests (although
    /// possible) because they would likely fail with a RPC error anyway,
    /// so we avoid another error case by simply panicking.
    ///
    /// The definition of "too large" is roughly 1MB, so as long as the
    /// payload is below that mark, it's safe to call.
    pub fn enqueue_request(&mut self, mut body: Vec<u8>) -> MsgId {
        // Requests that are too large would cause Telegram to close the
        // connection but are so uncommon it's not worth returning `Err`.
        assert!(
            body.len() + manual_tl::Message::SIZE_OVERHEAD
                <= manual_tl::MessageContainer::MAXIMUM_SIZE
        );

        // Serialized requests will always be well-formed
        assert!(body.len() % 4 == 0);

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

        self.enqueue_body(body, true)
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
                tl::enums::MsgsAck::Ack(tl::types::MsgsAck { msg_ids }).to_bytes(),
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
        let mut buf = Vec::with_capacity(8 + 8 + batch_size);

        self.salt.serialize(&mut buf);
        self.client_id.serialize(&mut buf);

        // If we're sending more than one, write the `MessageContainer` header.
        // This should be the moral equivalent of `MessageContainer.serialize(...)`.
        if batch_len > 1 {
            // This should be the moral equivalent of `enqueue_body`
            // and `Message::serialize`.
            let msg_id = self.get_new_msg_id();
            let seq_no = self.get_seq_no(false);

            msg_id.serialize(&mut buf);
            seq_no.serialize(&mut buf);
            ((batch_size - manual_tl::Message::SIZE_OVERHEAD) as i32).serialize(&mut buf);

            manual_tl::MessageContainer::CONSTRUCTOR_ID.serialize(&mut buf);
            (batch_len as i32).serialize(&mut buf);
        }

        // Pop `batch_len` requests and append them to the final message.
        (0..batch_len).for_each(|_| {
            // Safe to unwrap because the length cannot exceed the queue's.
            let message = self.message_queue.pop_front().unwrap();
            message.serialize(&mut buf);
        });

        // Our message is ready.
        Some(buf)
    }

    /// If there is one or more requests enqueued, this method will pack as
    /// many as possible into a single message, serialize it, and return it
    /// encrypted using the current authorization key as indicated by the
    /// [MTProto 2.0 guidelines].
    ///
    /// If there are no enqueued requests, the method returns `None`.
    ///
    /// [MTProto 2.0 guidelines]: https://core.telegram.org/mtproto/description.
    pub fn serialize_encrypted_messages(&mut self) -> Option<Vec<u8>> {
        self.pop_queued_messages()
            .map(|payload| encrypt_data_v2(&payload, &self.auth_key))
    }

    /// Processes an encrypted response from the server. If the response
    /// contains replies to previously-sent requests, they will be enqueued,
    /// and can be polled for later with [`poll_response`].
    ///
    /// [`poll_response`]: #method.poll_response
    pub fn process_encrypted_response(
        &mut self,
        ciphertext: &[u8],
    ) -> Result<(), DeserializeError> {
        crate::utils::check_message_buffer(ciphertext)?;

        let plaintext = decrypt_data_v2(ciphertext, &self.auth_key)?;
        let mut buffer = Cursor::from_slice(&plaintext[..]);

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

    /// Poll for updates that arrived.
    ///
    /// If there are no new updates, the method returns `None`.
    pub fn poll_update(&mut self) -> Option<Vec<u8>> {
        self.update_queue.pop_front()
    }

    // Response handlers
    // ========================================

    fn process_message(&mut self, message: manual_tl::Message) -> Result<(), DeserializeError> {
        if message.requires_ack() {
            self.pending_ack.push(message.msg_id);
        }

        // Handle all the possible Service Messages:
        // * https://core.telegram.org/mtproto/service_messages
        // * https://core.telegram.org/mtproto/service_messages_about_messages
        //
        // The order of the `match` here is the same as the order in which the
        // items appear in the documentation (to make it easier to review).
        match message.constructor_id()? {
            // Response to an RPC query
            manual_tl::RpcResult::CONSTRUCTOR_ID => self.handle_rpc_result(message),

            // Service Messages about Messages
            // Acknowledgment of Receipt
            tl::types::MsgsAck::CONSTRUCTOR_ID => self.handle_ack(message),
            // Notice of Ignored Error Message
            tl::types::BadMsgNotification::CONSTRUCTOR_ID
            | tl::types::BadServerSalt::CONSTRUCTOR_ID => self.handle_bad_notification(message),
            // Request for Message Status Information
            tl::types::MsgsStateReq::CONSTRUCTOR_ID => self.handle_state_req(message),
            // Informational Message regarding Status of Messages
            tl::types::MsgsStateInfo::CONSTRUCTOR_ID => self.handle_state_info(message),
            // Voluntary Communication of Status of Messages
            tl::types::MsgsAllInfo::CONSTRUCTOR_ID => self.handle_msg_all(message),
            // Extended Voluntary Communication of Status of One Message
            tl::types::MsgDetailedInfo::CONSTRUCTOR_ID
            | tl::types::MsgNewDetailedInfo::CONSTRUCTOR_ID => self.handle_detailed_info(message),
            // Explicit Request to Re-Send Messages & Explicit Request to Re-Send Answers
            tl::types::MsgResendReq::CONSTRUCTOR_ID
            | tl::types::MsgResendAnsReq::CONSTRUCTOR_ID => self.handle_msg_resend(message),

            // Request for several future salts
            tl::types::FutureSalt::CONSTRUCTOR_ID => self.handle_future_salt(message),
            tl::types::FutureSalts::CONSTRUCTOR_ID => self.handle_future_salts(message),
            // Ping Messages (PING/PONG)
            tl::types::Pong::CONSTRUCTOR_ID => self.handle_pong(message),
            // Request to Destroy Session
            tl::types::DestroySessionOk::CONSTRUCTOR_ID
            | tl::types::DestroySessionNone::CONSTRUCTOR_ID => self.handle_destroy_session(message),
            // New Session Creation Notification
            tl::types::NewSessionCreated::CONSTRUCTOR_ID => {
                self.handle_new_session_created(message)
            }
            // Containers (Simple Container)
            manual_tl::MessageContainer::CONSTRUCTOR_ID => self.handle_container(message),
            // Message Copies
            manual_tl::MessageCopy::CONSTRUCTOR_ID => self.handle_msg_copy(message),
            // Packed Object
            manual_tl::GzipPacked::CONSTRUCTOR_ID => self.handle_gzip_packed(message),
            // HTTP Wait/Long Poll
            tl::types::HttpWait::CONSTRUCTOR_ID => self.handle_http_wait(message),
            _ => self.handle_update(message),
        }
    }

    /// **[Response to an RPC query]**
    ///
    /// A response to an RPC query is normally wrapped as follows:
    ///
    /// ```tl
    /// rpc_result#f35c6d01 req_msg_id:long result:Object = RpcResult;
    /// ```
    ///
    /// Here `req_msg_id` is the identifier of the message sent by the other
    /// party and containing an RPC query. This way, the recipient knows that
    /// the result is a response to the specific RPC query in question.
    ///
    /// At the same time, this response serves as acknowledgment of the other
    /// party's receipt of the `req_msg_id` message.
    ///
    /// Note that the response to an RPC query must also be acknowledged. Most
    /// frequently, this coincides with the transmission of the next message
    /// (which may have a container attached to it carrying a service message
    /// with the acknowledgment).
    ///
    /// **[RPC Error]**
    ///
    /// The result field returned in response to any RPC query may also
    /// contain an error message in the following format:
    ///
    /// ```tl
    /// rpc_result#f35c6d01 req_msg_id:long result:Object = RpcResult;
    /// ```
    ///
    /// **[Cancellation of an RPC Query]**
    ///
    /// In certain situations, the client does not want to receive a response
    /// to an already transmitted RPC query, for example because the response
    /// turns out to be long and the client has decided to do without it
    /// because of insufficient link capacity. Simply interrupting the TCP
    /// connection will not have any effect because the server would re-send
    /// the missing response at the first opportunity. Therefore, the client
    /// needs a way to cancel receipt of the RPC response message, actually
    /// acknowledging its receipt prior to it being in fact received, which
    /// will settle the server down and prevent it from re-sending the response.
    /// However, the client does not know the RPC response's msg_id prior to
    /// receiving the response; the only thing it knows is the req_msg_id.
    /// i.e. the msg_id of the relevant RPC query. Therefore, a special query
    /// is used:
    ///
    /// ```tl
    /// rpc_drop_answer#58e4a740 req_msg_id:long = RpcDropAnswer;
    /// ```
    ///
    /// The response to this query returns as one of the following messages
    /// wrapped in `rpc_result` and requiring an acknowledgment:
    ///
    /// ```tl
    /// rpc_answer_unknown#5e2ad36e = RpcDropAnswer;
    /// rpc_answer_dropped_running#cd78e586 = RpcDropAnswer;
    /// rpc_answer_dropped#a43ad8b7 msg_id:long seq_no:int bytes:int = RpcDropAnswer;
    /// ```
    ///
    /// The first version of the response is used if the server remembers
    /// nothing of the incoming req_msg_id (if it has already been responded
    /// to, for example). The second version is used if the response was
    /// canceled while the RPC query was being processed (where the RPC query
    /// itself was still fully processed); in this case, the same
    /// `rpc_answer_dropped_running` is also returned in response to the
    /// original query, and both of these responses require an acknowledgment
    /// from the client. The final version means that the RPC response was
    /// removed from the server's outgoing queue, and its `msg_id`, `seq_no`,
    /// and length in `bytes` are transmitted to the client.
    ///
    /// Note that `rpc_answer_dropped_running` and `rpc_answer_dropped` serve
    /// as acknowledgments of the server's receipt of the original query (the
    /// same one, the response to which we wish to forget). In addition, same
    /// as for any RPC queries, any response to `rpc_drop_answer` is an
    /// acknowledgment for `rpc_drop_answer` itself.
    ///
    /// As an alternative to using `rpc_drop_answer`, a new session may be
    /// created after the connection is reset and the old session is removed
    /// through `destroy_session`.
    ///
    /// [Response to an RPC query]: https://core.telegram.org/mtproto/service_messages#response-to-an-rpc-query
    /// [RPC Error]: https://core.telegram.org/mtproto/service_messages#rpc-error
    /// [Cancellation of an RPC Query]: https://core.telegram.org/mtproto/service_messages#cancellation-of-an-rpc-query
    fn handle_rpc_result(&mut self, message: manual_tl::Message) -> Result<(), DeserializeError> {
        let rpc_result = manual_tl::RpcResult::from_bytes(&message.body)?;
        let inner_constructor = rpc_result.inner_constructor();
        let manual_tl::RpcResult { req_msg_id, result } = rpc_result;
        let msg_id = MsgId(req_msg_id);

        // Can't use `?` because we need to make sure to push a response
        let inner_constructor = match inner_constructor {
            Ok(x) => x,
            Err(e) => {
                self.response_queue.push_back((msg_id, Err(e.into())));
                return Err(e.into());
            }
        };

        match inner_constructor {
            // RPC Error
            tl::types::RpcError::CONSTRUCTOR_ID => match tl::enums::RpcError::from_bytes(&result) {
                Ok(tl::enums::RpcError::Error(error)) => self
                    .response_queue
                    .push_back((msg_id, Err(RequestError::RPCError(error.into())))),
                Err(error) => {
                    self.response_queue.push_back((msg_id, Err(error.into())));
                    return Err(error.into());
                }
            },

            // Cancellation of an RPC Query
            tl::types::RpcAnswerUnknown::CONSTRUCTOR_ID => {
                // The `msg_id` corresponds to the `rpc_drop_answer` request.
            }
            tl::types::RpcAnswerDroppedRunning::CONSTRUCTOR_ID => {
                // We will receive two `rpc_result`, one with the `msg_id` of
                // `rpc_drop_answer` request and other for the original RPC.
                // TODO ack to the server
            }
            tl::types::RpcAnswerDropped::CONSTRUCTOR_ID => {
                // "the RPC response was removed from the server's outgoing
                // queue, and its msg_id, seq_no, and length in bytes are
                // transmitted to the client."
            }

            // Response to an RPC query
            manual_tl::GzipPacked::CONSTRUCTOR_ID => {
                // Telegram shouldn't send compressed errors (the overhead
                // would probably outweight the benefits) so we don't check
                // that the decompressed payload is an error or answer drop.
                let gzip = match manual_tl::GzipPacked::from_bytes(&result) {
                    Ok(x) => x,
                    Err(e) => {
                        self.response_queue.push_back((msg_id, Err(e.into())));
                        return Err(e.into());
                    }
                };
                let data = match gzip.decompress() {
                    Ok(x) => self.response_queue.push_back((msg_id, Ok(x))),
                    Err(e) => {
                        self.response_queue.push_back((msg_id, Err(e.into())));
                        return Err(e);
                    }
                };
            }
            _ => {
                self.response_queue.push_back((msg_id, Ok(result)));
            }
        }

        Ok(())
    }

    /// **[Acknowledgment of Receipt]**
    ///
    /// Receipt of virtually all messages (with the exception of some purely
    /// service ones as well as the plain-text messages used in the protocol
    /// for creating an authorization key) must be acknowledged.
    ///
    /// This requires the use of the following service message (not requiring
    /// an acknowledgment):
    ///
    /// ```tl
    /// msgs_ack#62d6b459 msg_ids:Vector long = MsgsAck;
    /// ```
    ///
    /// A server usually acknowledges the receipt of a message from a client
    /// (normally, an RPC query) using an RPC response. If a response is a
    /// long time coming, a server may first send a receipt acknowledgment,
    /// and somewhat later, the RPC response itself.
    ///
    /// A client normally acknowledges the receipt of a message from a server
    /// (usually, an RPC response) by adding an acknowledgment to the next RPC
    /// query if it is not transmitted too late (if it is generated, say,
    /// 60-120 seconds following the receipt of a message from the server).
    /// However, if for a long period of time there is no reason to send
    /// messages to the server or if there is a large number of
    /// unacknowledged messages from the server (say, over 16), the client
    /// transmits a stand-alone acknowledgment.
    ///
    /// [Acknowledgment of Receipt]: https://core.telegram.org/mtproto/service_messages_about_messages#acknowledgment-of-receipt
    fn handle_ack(&self, message: manual_tl::Message) -> Result<(), DeserializeError> {
        // TODO notify about this somehow
        let _ack = tl::enums::MsgsAck::from_bytes(&message.body)?;
        Ok(())
    }

    /// **[Notice of Ignored Error Message]**
    ///
    /// In certain cases, a server may notify a client that its incoming
    /// message was ignored for whatever reason. Note that such a notification
    /// cannot be generated unless a message is correctly decoded by the
    /// server.
    ///
    /// ```tl
    /// bad_msg_notification#a7eff811 bad_msg_id:long bad_msg_seqno:int error_code:int = BadMsgNotification;
    /// bad_server_salt#edab447b bad_msg_id:long bad_msg_seqno:int error_code:int new_server_salt:long = BadMsgNotification;
    /// ```
    ///
    /// Here, `error_code` can also take on the following values:
    ///
    /// * 16: `msg_id` too low (most likely, client time is wrong; it would
    ///   be worthwhile to synchronize it using msg_id notifications and re-
    ///   send the original message with the “correct” msg_id or wrap it in a
    ///   container with a new msg_id if the original message had waited too
    ///   long on the client to be transmitted)
    /// * 17: `msg_id` too high (similar to the previous case, the client time
    ///   has to be synchronized, and the message re-sent with the correct
    ///   `msg_id`)
    /// * 18: incorrect two lower order `msg_id` bits (the server expects
    ///   client message `msg_id` to be divisible by 4)
    /// * 19: container `msg_id` is the same as `msg_id` of a previously
    ///   received message (this must never happen)
    /// * 20: message too old, and it cannot be verified whether the server
    ///   has received a message with this `msg_id` or not
    /// * 32: `msg_seqno` too low (the server has already received a message
    ///   with a lower `msg_id` but with either a higher or an equal and odd
    ///   `seqno`)
    /// * 33: `msg_seqno` too high (similarly, there is a message with a
    ///   higher `msg_id` but with either a lower or an equal and odd `seqno`)
    /// * 34: an even `msg_seqno` expected (irrelevant message), but odd
    ///   received
    /// * 35: odd `msg_seqno` expected (relevant message), but even received
    /// * 48: incorrect server salt (in this case, the `bad_server_salt`
    ///   response is received with the correct salt, and the message is to be
    ///   re-sent with it)
    /// * 64: invalid container.
    ///
    /// The intention is that `error_code` values are grouped
    /// (`error_code >> 4`): for example, the codes `0x40 - 0x4f` correspond
    /// to errors in container decomposition.
    ///
    /// Notifications of an ignored message do not require acknowledgment
    /// (i.e., are irrelevant).
    ///
    /// **Important**: if `server_salt` has changed on the server or if client
    /// time is incorrect, any query will result in a notification in the
    /// above format. The client must check that it has, in fact, recently
    /// sent a message with the specified msg_id, and if that is the case,
    /// update its time correction value (the difference between the client's
    /// and the server's clocks) and the server salt based on msg_id and the
    /// `server_salt` notification, so as to use these to (re)send future
    /// messages. In the meantime, the original message (the one that caused
    /// the error message to be returned) must also be re-sent with a better
    /// `msg_id` and/or `server_salt`.
    ///
    /// In addition, the client can update the `server_salt` value used to
    /// send messages to the server, based on the values of RPC responses or
    /// containers carrying an RPC response, provided that this RPC response
    /// is actually a match for the query sent recently. (If there is doubt,
    /// it is best not to update since there is risk of a replay attack).
    ///
    /// [Notice of Ignored Error Message]: https://core.telegram.org/mtproto/service_messages_about_messages#notice-of-ignored-error-message
    fn handle_bad_notification(
        &mut self,
        message: manual_tl::Message,
    ) -> Result<(), DeserializeError> {
        let bad_msg = tl::enums::BadMsgNotification::from_bytes(&message.body)?;
        let bad_msg = match bad_msg {
            tl::enums::BadMsgNotification::Notification(x) => x,
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

    /// **[Request for Message Status Information]**
    ///
    /// If either party has not received information on the status of its
    /// outgoing messages for a while, it may explicitly request it from the
    /// other party:
    ///
    /// ```tl
    /// msgs_state_req#da69fb52 msg_ids:Vector long = MsgsStateReq;
    /// ```
    ///
    /// [Request for Message Status Information]: https://core.telegram.org/mtproto/service_messages_about_messages#request-for-message-status-information
    fn handle_state_req(&self, _message: manual_tl::Message) -> Result<(), DeserializeError> {
        // TODO implement
        Ok(())
    }

    /// **[Informational Message regarding Status of Messages]**
    ///
    /// ```tl
    /// msgs_state_info#04deb57d req_msg_id:long info:string = MsgsStateInfo;
    /// ```
    ///
    /// Here, `info` is a string that contains exactly one byte of message
    /// status for each message from the incoming `msg_ids` list:
    ///
    /// * 1 = nothing is known about the message (`msg_id` too low, the other
    ///   party may have forgotten it)
    /// * 2 = message not received (`msg_id` falls within the range of stored
    ///   identifiers; however, the other party has certainly not received a
    ///   message like that)
    /// * 3 = message not received (`msg_id` too high; however, the other
    ///   party has certainly not received it yet)
    /// * 4 = message received (note that this response is also at the same
    ///   time a receipt acknowledgment)
    /// * +8 = message already acknowledged
    /// * +16 = message not requiring acknowledgment
    /// * +32 = RPC query contained in message being processed or processing
    ///   already complete
    /// * +64 = content-related response to message already generated
    /// * +128 = other party knows for a fact that message is already received
    ///
    /// This response does not require an acknowledgment. It is an
    /// acknowledgment of the relevant `msgs_state_req`, in and of itself.
    ///
    /// Note that if it turns out suddenly that the other party does not have
    /// a message that looks like it has been sent to it, the message can
    /// simply be re-sent. Even if the other party should receive two copies
    /// of the message at the same time, the duplicate will be ignored.
    /// (If too much time has passed, and the original `msg_id` is not longer
    /// valid, the message is to be wrapped in `msg_copy`).
    ///
    /// [Informational Message regarding Status of Messages]: https://core.telegram.org/mtproto/service_messages_about_messages#informational-message-regarding-status-of-messages
    fn handle_state_info(&mut self, message: manual_tl::Message) -> Result<(), DeserializeError> {
        // TODO implement
        Ok(())
    }

    /// **[Voluntary Communication of Status of Messages]**
    ///
    /// Either party may voluntarily inform the other party of the status of
    /// the messages transmitted by the other party.
    ///
    /// ```tl
    /// msgs_all_info#8cc0d131 msg_ids:Vector long info:string = MsgsAllInfo;
    /// ```
    ///
    /// All message codes known to this party are enumerated, with the
    /// exception of those for which the +128 and the +16 flags are set.
    /// However, if the +32 flag is set but not +64, then the message status
    /// will still be communicated.
    ///
    /// This message does not require an acknowledgment.
    ///
    /// [Voluntary Communication of Status of Messages]: https://core.telegram.org/mtproto/service_messages_about_messages#voluntary-communication-of-status-of-messages
    fn handle_msg_all(&mut self, message: manual_tl::Message) -> Result<(), DeserializeError> {
        // TODO implement
        Ok(())
    }

    /// **[Extended Voluntary Communication of Status of One Message]**
    ///
    /// Normally used by the server to respond to the receipt of a duplicate
    /// `msg_id`, especially if a response to the message has already been
    /// generated and the response is large. If the response is small, the
    /// server may re-send the answer itself instead. This message can also
    /// be used as a notification instead of resending a large message.
    ///
    /// ```tl
    /// msg_detailed_info#276d3ec6 msg_id:long answer_msg_id:long bytes:int status:int = MsgDetailedInfo;
    /// msg_new_detailed_info#809db6df answer_msg_id:long bytes:int status:int = MsgDetailedInfo;
    /// ```
    ///
    /// The second version is used to notify of messages that were created on
    /// the server not in response to an RPC query (such as notifications of
    /// new messages) and were transmitted to the client some time ago, but
    /// not acknowledged.
    ///
    /// Currently, `status` is always zero. This may change in future.
    ///
    /// This message does not require an acknowledgment.
    ///
    /// [Extended Voluntary Communication of Status of One Message]: https://core.telegram.org/mtproto/service_messages_about_messages#extended-voluntary-communication-of-status-of-one-message
    fn handle_detailed_info(
        &mut self,
        message: manual_tl::Message,
    ) -> Result<(), DeserializeError> {
        // TODO https://github.com/telegramdesktop/tdesktop/blob/8f82880b938e06b7a2a27685ef9301edb12b4648/Telegram/SourceFiles/mtproto/connection.cpp#L1790-L1820
        // TODO https://github.com/telegramdesktop/tdesktop/blob/8f82880b938e06b7a2a27685ef9301edb12b4648/Telegram/SourceFiles/mtproto/connection.cpp#L1822-L1845
        let msg_detailed = tl::enums::MsgDetailedInfo::from_bytes(&message.body)?;
        match msg_detailed {
            tl::enums::MsgDetailedInfo::Info(x) => {
                self.pending_ack.push(x.answer_msg_id);
            }
            tl::enums::MsgDetailedInfo::MsgNewDetailedInfo(x) => {
                self.pending_ack.push(x.answer_msg_id);
            }
        }
        Ok(())
    }

    /// **[Explicit Request to Re-Send Messages]**
    ///
    /// ```tl
    /// msg_resend_req#7d861a08 msg_ids:Vector long = MsgResendReq;
    /// ```
    ///
    /// The remote party immediately responds by re-sending the requested
    /// messages, normally using the same connection that was used to transmit
    /// the query. If at least one message with requested msg_id does not
    /// exist or has already been forgotten, or has been sent by the
    /// requesting party (known from parity), `MsgsStateInfo` is returned for
    /// all messages requested as if the `MsgResendReq` query had been a
    /// `MsgsStateReq` query as well.
    ///
    /// **[Explicit Request to Re-Send Answers]**
    ///
    /// ```tl
    /// msg_resend_ans_req#8610baeb msg_ids:Vector long = MsgResendReq;
    /// ```
    ///
    /// The remote party immediately responds by re-sending answers to the
    /// requested messages, normally using the same connection that was used
    /// to transmit the query. `MsgsStateInfo` is returned for all messages
    /// requested as if the `MsgResendReq` query had been a MsgsStateReq query
    /// as well.
    ///
    /// [Explicit Request to Re-Send Answers]: https://core.telegram.org/mtproto/service_messages_about_messages#explicit-request-to-re-send-answers
    /// [Explicit Request to Re-Send Messages]: https://core.telegram.org/mtproto/service_messages_about_messages#explicit-request-to-re-send-messages
    fn handle_msg_resend(&self, _message: manual_tl::Message) -> Result<(), DeserializeError> {
        // TODO implement
        // `msg_resend_ans_req` seems to never occur (it was even missing from `mtproto.tl`)
        Ok(())
    }

    /// **[Request for several future salts]**
    ///
    /// The client may at any time request from the server several (between
    /// 1 and 64) future server salts together with their validity periods.
    /// Having stored them in persistent memory, the client may use them to
    /// send messages in the future even if he changes sessions (a server
    /// salt is attached to the authorization key rather than being
    /// session-specific).
    ///
    /// ```tl
    /// get_future_salts#b921bd04 num:int = FutureSalts;
    /// future_salt#0949d9dc valid_since:int valid_until:int salt:long = FutureSalt;
    /// future_salts#ae500895 req_msg_id:long now:int salts:vector future_salt = FutureSalts;
    /// ```
    ///
    /// The client must check to see that the response's `req_msg_id` in fact
    /// coincides with `msg_id` of the query for `get_future_salts`. The
    /// server returns a maximum of `num` future server salts (may return
    /// fewer). The response serves as the acknowledgment of the query and
    /// does not require an acknowledgment itself.
    ///
    /// [Request for several future salts]: https://core.telegram.org/mtproto/service_messages#request-for-several-future-salts
    fn handle_future_salts(&mut self, message: manual_tl::Message) -> Result<(), DeserializeError> {
        let tl::enums::FutureSalts::Salts(salts) =
            tl::enums::FutureSalts::from_bytes(&message.body)?;

        self.response_queue
            .push_back((MsgId(salts.req_msg_id), Ok(message.body.clone())));
        Ok(())
    }

    fn handle_future_salt(&mut self, _message: manual_tl::Message) -> Result<(), DeserializeError> {
        panic!("no request should trigger a `future_salt` result")
    }

    /// **[Ping Messages (PING/PONG)]**
    ///
    /// ```tl
    /// ping#7abe77ec ping_id:long = Pong;
    /// ```
    ///
    /// A response is usually returned to the same connection:
    ///
    /// ```tl
    /// pong#347773c5 msg_id:long ping_id:long = Pong;
    /// ```
    ///
    /// These messages do not require acknowledgments. A `pong` is
    /// transmitted only in response to a `ping` while a `ping` can be
    /// initiated by either side.
    ///
    /// **[Deferred Connection Closure + PING]**
    ///
    /// ```tl
    /// ping_delay_disconnect#f3427b8c ping_id:long disconnect_delay:int = Pong;
    /// ```
    ///
    /// Works like `ping`. In addition, after this is received, the server
    /// starts a timer which will close the current connection
    /// `disconnect_delay` seconds later unless it receives a new message of
    /// the same type which automatically resets all previous timers. If the
    /// client sends these pings once every 60 seconds, for example, it may
    /// set `disconnect_delay` equal to 75 seconds.
    ///
    /// [Ping Messages (PING/PONG)]: https://core.telegram.org/mtproto/service_messages#ping-messages-ping-pong
    /// [Deferred Connection Closure + PING]: https://core.telegram.org/mtproto/service_messages#deferred-connection-closure-ping
    fn handle_pong(&mut self, message: manual_tl::Message) -> Result<(), DeserializeError> {
        let tl::enums::Pong::Pong(pong) = tl::enums::Pong::from_bytes(&message.body)?;

        self.response_queue
            .push_back((MsgId(pong.msg_id), Ok(message.body.clone())));
        Ok(())
    }

    /// **[Request to Destroy Session]**
    ///
    /// Used by the client to notify the server that it may forget the data
    /// from a different session belonging to the same user (i.e. with the
    /// same `auth_key_id`). The result of this being applied to the current
    /// session is undefined.
    ///
    /// ```tl
    /// destroy_session#e7512126 session_id:long = DestroySessionRes;
    /// destroy_session_ok#e22045fc session_id:long = DestroySessionRes;
    /// destroy_session_none#62d350c9 session_id:long = DestroySessionRes;
    /// ```
    ///
    /// [Request to Destroy Session]: https://core.telegram.org/mtproto/service_messages#request-to-destroy-session
    fn handle_destroy_session(&self, _message: manual_tl::Message) -> Result<(), DeserializeError> {
        // TODO implement
        Ok(())
    }

    /// **[New Session Creation Notification]**
    ///
    /// The server notifies the client that a new session (from the server's
    /// standpoint) had to be created to handle a client message. If, after
    /// this, the server receives a message with an even smaller `msg_id`
    /// within the same session, a similar notification will be generated for
    /// this `msg_id` as well. No such notifications are generated for high
    /// `msg_id` values.
    ///
    /// ```tl
    /// new_session_created#9ec20908 first_msg_id:long unique_id:long server_salt:long = NewSession
    /// ```
    ///
    /// The `unique_id` parameter is generated by the server every time a
    /// session is (re-)created.
    ///
    /// This notification must be acknowledged by the client. It is necessary,
    /// for instance, for the client to understand that there is, in fact, a
    /// "gap" in the stream of long poll notifications received from the
    /// server (the user may have failed to receive notifications during some
    /// period of time).
    ///
    /// Notice that the server may unilaterally destroy (close) any existing
    /// client sessions with all pending messages and notifications, without
    /// sending any notifications. This happens, for example, if the session
    /// is inactive for a long time, and the server runs out of memory. If the
    /// client at some point decides to send new messages to the server using
    /// the old session, already forgotten by the server, such a "new session
    /// created" notification will be generated. The client is expected to
    /// handle such situations gracefully.
    ///
    /// [New Session Creation Notification]: https://core.telegram.org/mtproto/service_messages#new-session-creation-notification
    fn handle_new_session_created(
        &mut self,
        message: manual_tl::Message,
    ) -> Result<(), DeserializeError> {
        let new_session = tl::enums::NewSession::from_bytes(&message.body)?;
        match new_session {
            tl::enums::NewSession::Created(x) => {
                self.salt = x.server_salt;
            }
        }
        Ok(())
    }

    /// **[Containers]**
    ///
    /// *Containers* are messages containing several other messages. Used
    /// for the ability to transmit several RPC queries and/or service
    /// messages at the same time, using HTTP or even TCP or UDP protocol.
    /// A container may only be accepted or rejected by the other party as
    /// a whole.
    ///
    /// **[Simple Container]**
    ///
    /// A simple container carries several messages as follows:
    ///
    /// ```tl
    /// msg_container#73f1f8dc messages:vector message = MessageContainer;
    /// ```
    ///
    /// Here message refers to any message together with its length and
    /// `msg_id`:
    ///
    /// ```tl
    /// message msg_id:long seqno:int bytes:int body:Object = Message;
    /// ```
    ///
    /// `bytes` is the number of bytes in the body serialization.
    ///
    /// All messages in a container must have `msg_id` lower than that of the
    /// container itself. A container does not require an acknowledgment and
    /// may not carry other simple containers. When messages are re-sent,
    /// they may be combined into a container in a different manner or sent
    /// individually.
    ///
    /// Empty containers are also allowed. They are used by the server,
    /// for example, to respond to an HTTP request when the timeout specified
    /// in `http_wait` expires, and there are no messages to transmit.
    ///
    /// [Containers]: https://core.telegram.org/mtproto/service_messages#containers
    /// [Simple Container]: https://core.telegram.org/mtproto/service_messages#simple-container
    fn handle_container(&mut self, message: manual_tl::Message) -> Result<(), DeserializeError> {
        let container = manual_tl::MessageContainer::from_bytes(&message.body)?;
        for inner_message in container.messages {
            self.process_message(inner_message)?;
        }

        Ok(())
    }

    /// **[Message Copies]**
    ///
    /// In some situations, an old message with a `msg_id` that is no longer
    /// valid needs to be re-sent. Then, it is wrapped in a copy container:
    ///
    /// ```tl
    /// msg_copy#e06046b2 orig_message:Message = MessageCopy;
    /// ```
    ///
    /// Once received, the message is processed as if the wrapper were not
    /// there. However, if it is known for certain that the message
    /// `orig_message.msg_id` was received, then the new message is not
    /// processed (while at the same time, it and `orig_message.msg_id`
    /// are acknowledged). The value of `orig_message.msg_id` must be lower
    /// than the container's `msg_id`.
    ///
    /// This is not used at this time, because an old message can be wrapped
    /// in a simple container with the same result.
    ///
    /// [Message Copies]: https://core.telegram.org/mtproto/service_messages#message-copies
    fn handle_msg_copy(&self, _message: manual_tl::Message) -> Result<(), DeserializeError> {
        panic!("msg_copy should not be used")
    }

    /// **[Packed Object]**
    ///
    /// Used to replace any other object (or rather, a serialization thereof)
    /// with its archived (gzipped) representation:
    ///
    /// ```tl
    /// gzip_packed#3072cfa1 packed_data:string = Object;
    /// ```
    ///
    /// At the present time, it is supported in the body of an RPC response
    /// (i.e., as result in rpc_result) and generated by the server for a
    /// limited number of high-level queries. In addition, in the future it
    /// may be used to transmit non-service messages (i.e. RPC queries) from
    /// client to server.
    ///
    /// [Packed Object]: https://core.telegram.org/mtproto/service_messages#packed-object
    fn handle_gzip_packed(&mut self, message: manual_tl::Message) -> Result<(), DeserializeError> {
        let container = manual_tl::GzipPacked::from_bytes(&message.body)?;
        self.process_message(manual_tl::Message {
            body: container.decompress()?,
            ..message
        })
        .map(|_| ())
    }

    /// **[HTTP Wait/Long Poll]**
    ///
    /// The following special service query not requiring an acknowledgement
    /// (which must be transmitted only through an HTTP connection) is used to
    /// enable the server to send messages in the future to the client using
    /// HTTP protocol:
    ///
    /// ```tl
    /// http_wait#9299359f max_delay:int wait_after:int max_wait:int = HttpWait;
    /// ```
    ///
    /// When such a message (or a container carrying such a message) is
    /// received, the server either waits `max_delay` milliseconds, whereupon
    /// it forwards all the messages that it is holding on to the client if
    /// there is at least one message queued in session (if needed, by placing
    /// them into a container to which acknowledgments may also be added); or
    /// else waits no more than `max_wait` milliseconds until such a message
    /// is available. If a message never appears, an empty container is
    /// transmitted.
    ///
    /// The `max_delay` parameter denotes the maximum number of milliseconds
    /// that has elapsed between the first message for this session and the
    /// transmission of an HTTP response. The wait_after parameter works as
    /// follows: after the receipt of the latest message for a particular
    /// session, the server waits another wait_after milliseconds in case
    /// there are more messages. If there are no additional messages, the
    /// result is transmitted (a container with all the messages). If more
    /// messages appear, the `wait_after` timer is reset.
    ///
    /// At the same time, the max_delay parameter has higher priority than
    /// `wait_after`, and `max_wait` has higher priority than `max_delay`.
    ///
    /// This message does not require a response or an acknowledgement. If
    /// the container transmitted over HTTP carries several such messages,
    /// the behavior is undefined (in fact, the latest parameter will be
    /// used).
    ///
    /// If no `http_wait` is present in container, default values
    /// `max_delay=0` (milliseconds), `wait_after=0` (milliseconds), and
    /// `max_wait=25000` (milliseconds) are used.
    ///
    /// If the client's ping of the server takes a long time, it may make
    /// sense to set `max_delay` to a value that is comparable in magnitude
    /// to ping time.
    ///
    /// [HTTP Wait/Long Poll]: https://core.telegram.org/mtproto/service_messages#http-wait-long-poll
    fn handle_http_wait(&mut self, message: manual_tl::Message) -> Result<(), DeserializeError> {
        // TODO implement
        Ok(())
    }

    /// Anything else that's not Service Message will be `Updates`.
    ///
    /// Since we handle all the possible service messages, we can
    /// safely treat whatever message body we received as `Updates`.
    fn handle_update(&mut self, message: manual_tl::Message) -> Result<(), DeserializeError> {
        self.update_queue.push_back(message.body);
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

    fn auth_key() -> AuthKey {
        AuthKey::from_bytes([0; 256])
    }

    #[test]
    fn ensure_buffer_used_exact_capacity() {
        {
            // Single body (no container)
            let mut mtproto = Mtp::build().compression_threshold(None).finish(auth_key());

            mtproto.enqueue_request(vec![b'H', b'e', b'y', b'!']);
            let buffer = mtproto.pop_queued_messages().unwrap();
            assert_eq!(buffer.capacity(), buffer.len());
        }
        {
            // Multiple bodies (using a container)
            let mut mtproto = Mtp::build().compression_threshold(None).finish(auth_key());

            mtproto.enqueue_request(vec![b'H', b'e', b'y', b'!']);
            mtproto.enqueue_request(vec![b'B', b'y', b'e', b'!']);
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
        let mut mtproto = Mtp::build().compression_threshold(None).finish(auth_key());

        mtproto.enqueue_request(vec![b'H', b'e', b'y', b'!']);
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
        let mut mtproto = Mtp::build().compression_threshold(None).finish(auth_key());

        mtproto.enqueue_request(vec![b'H', b'e', b'y', b'!']);

        let buffer = &mtproto.pop_queued_messages().unwrap()[MESSAGE_PREFIX_LEN..];
        ensure_buffer_is_message(&buffer, b"Hey!", 1);
    }

    #[test]
    fn ensure_correct_multi_serialization() {
        let mut mtproto = Mtp::build().compression_threshold(None).finish(auth_key());

        mtproto.enqueue_request(vec![b'H', b'e', b'y', b'!']);
        mtproto.enqueue_request(vec![b'B', b'y', b'e', b'!']);
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
        let mut mtproto = Mtp::build().compression_threshold(None).finish(auth_key());
        let data = vec![0x7f; 768 * 1024];

        mtproto.enqueue_request(data.clone());
        let buffer = &mtproto.pop_queued_messages().unwrap()[MESSAGE_PREFIX_LEN..];
        assert_eq!(buffer.len(), 16 + data.len());
    }

    #[test]
    fn ensure_correct_multi_large_serialization() {
        let mut mtproto = Mtp::build().compression_threshold(None).finish(auth_key());
        let data = vec![0x7f; 768 * 1024];

        mtproto.enqueue_request(data.clone());

        mtproto.enqueue_request(data.clone());

        // The messages are large enough that they should not be able to go in
        // a container together (so there are two things to pop).
        let buffer = &mtproto.pop_queued_messages().unwrap()[MESSAGE_PREFIX_LEN..];
        assert_eq!(buffer.len(), 16 + data.len());

        let buffer = &mtproto.pop_queued_messages().unwrap()[MESSAGE_PREFIX_LEN..];
        assert_eq!(buffer.len(), 16 + data.len());
    }

    #[test]
    fn ensure_queue_is_clear() {
        let mut mtproto = Mtp::build().compression_threshold(None).finish(auth_key());

        assert!(mtproto.pop_queued_messages().is_none());
        mtproto.enqueue_request(vec![b'H', b'e', b'y', b'!']);

        assert!(mtproto.pop_queued_messages().is_some());
        assert!(mtproto.pop_queued_messages().is_none());
    }

    #[test]
    #[should_panic]
    fn ensure_large_payload_panics() {
        let mut mtproto = Mtp::build().compression_threshold(None).finish(auth_key());

        drop(mtproto.enqueue_request(vec![0; 2 * 1024 * 1024]));
    }

    #[test]
    #[should_panic]
    fn ensure_non_padded_payload_panics() {
        let mut mtproto = Mtp::build().compression_threshold(None).finish(auth_key());

        drop(mtproto.enqueue_request(vec![1, 2, 3]));
    }

    #[test]
    fn ensure_no_compression_is_honored() {
        // A large vector of null bytes should compress
        let mut mtproto = Mtp::build().compression_threshold(None).finish(auth_key());
        mtproto.enqueue_request(vec![0; 512 * 1024]);
        let buffer = mtproto.pop_queued_messages().unwrap();
        assert!(!buffer.windows(4).any(|w| w == GZIP_PACKED_HEADER));
    }

    #[test]
    fn ensure_some_compression() {
        // A large vector of null bytes should compress
        {
            // High threshold not reached, should not compress
            let mut mtproto = Mtp::build()
                .compression_threshold(Some(768 * 1024))
                .finish(auth_key());
            mtproto.enqueue_request(vec![0; 512 * 1024]);
            let buffer = mtproto.pop_queued_messages().unwrap();
            assert!(!buffer.windows(4).any(|w| w == GZIP_PACKED_HEADER));
        }
        {
            // Low threshold is exceeded, should compress
            let mut mtproto = Mtp::build()
                .compression_threshold(Some(256 * 1024))
                .finish(auth_key());
            mtproto.enqueue_request(vec![0; 512 * 1024]);
            let buffer = mtproto.pop_queued_messages().unwrap();
            assert!(buffer.windows(4).any(|w| w == GZIP_PACKED_HEADER));
        }
        {
            // The default should compress
            let mut mtproto = Mtp::new(auth_key());
            mtproto.enqueue_request(vec![0; 512 * 1024]);
            let buffer = mtproto.pop_queued_messages().unwrap();
            assert!(buffer.windows(4).any(|w| w == GZIP_PACKED_HEADER));
        }
    }
}
