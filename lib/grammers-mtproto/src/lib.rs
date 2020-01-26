mod manual_tl;
use getrandom::getrandom;
use grammers_tl_types::{self as tl, Identifiable};

struct RequestState {
    data: Vec<u8>,
}

struct AuthKey {
    data: Vec<u8>,
}

/// This structure holds everything needed by the Mobile Transport Protocol.
pub struct MTProto {
    /// The authorization key to use to encrypt payload.
    auth_key: AuthKey,

    /// The time offset from the server's time, in seconds.
    time_offset: i32,

    /// The current salt to be used when encrypting payload.
    salt: u64,

    /// The secure, random identifier for this instance.
    client_id: u64,

    /// The current message sequence number.
    sequence: u64,

    /// The ID of the last message.
    last_msg_id: u64,

    /// A queue of requests that are pending from being packed in a message.
    requests_queue: Vec<RequestState>,

    /// Identifiers that need to be acknowledged to the server.
    pending_ack: Vec<i64>,
}

#[derive(Copy, Clone, Debug, Hash, PartialEq)]
pub struct MsgId(u32);

pub struct Response {
    pub msg_id: MsgId,
    pub data: Vec<u8>,
}

impl MTProto {
    pub fn new() -> Self {
        let client_id = {
            let mut buffer = [0u8; 8];
            getrandom(&mut buffer).expect("failed to generate a secure client_id");
            u64::from_le_bytes(buffer)
        };

        Self {
            auth_key: AuthKey { data: vec![] },
            time_offset: 0,
            salt: 0,
            client_id,
            sequence: 0,
            last_msg_id: 0,
            requests_queue: vec![],
            pending_ack: vec![],
        }
    }

    /// Enqueues a request and returns its associated `msg_id`.
    ///
    /// Once a response arrives and it is decrypted, the caller
    /// is expected to compare the `msg_id` against previously
    /// enqueued requests to determine to which of these the
    /// response belongs.
    pub fn enqueue_request() -> MsgId {
        unimplemented!("send into pending queue not implemented");
    }

    /// Enqueues a request to be executed by the server sequentially after
    /// another specific request.
    pub fn enqueue_sequential_request(after: MsgId) {
        unimplemented!();
    }

    /// Pops as many enqueued requests as possible, and returns
    /// encrypted data. If there are no requests enqueued, this
    /// returns `None`.
    pub fn pop_queue() -> Option<Vec<u8>> {
        unimplemented!("send queue packer not implemented");
    }

    /// Decrypts a response packet and handles its contents. If
    /// the response belonged to a previous request, it is returned.
    pub fn decrypt_response(&mut self) -> Option<Response> {
        unimplemented!("recv handler not implemented");
    }

    fn process_message(&mut self, message: manual_tl::Message) {
        self.pending_ack.push(message.msg_id);
        let constructor_id = match message.constructor_id() {
            Ok(x) => x,
            Err(e) => {
                // TODO propagate
                eprintln!("failed to peek constructor ID from message: {:?}", e);
                return;
            }
        };

        match constructor_id {
            manual_tl::RpcResult::CONSTRUCTOR_ID => self.handle_rpc_result(),
            manual_tl::MessageContainer::CONSTRUCTOR_ID => self.handle_container(),
            tl::types::Pong::CONSTRUCTOR_ID => self.handle_pong(),
            tl::types::BadServerSalt::CONSTRUCTOR_ID => self.handle_bad_server_salt(),
            tl::types::BadMsgNotification::CONSTRUCTOR_ID => self.handle_bad_notification(),
            tl::types::MsgDetailedInfo::CONSTRUCTOR_ID => self.handle_detailed_info(),
            tl::types::MsgNewDetailedInfo::CONSTRUCTOR_ID => self.handle_new_detailed_info(),
            tl::types::NewSessionCreated::CONSTRUCTOR_ID => self.handle_new_session_created(),
            tl::types::MsgsAck::CONSTRUCTOR_ID => self.handle_ack(),
            tl::types::FutureSalts::CONSTRUCTOR_ID => self.handle_future_salts(),
            tl::types::MsgsStateReq::CONSTRUCTOR_ID => self.handle_state_forgotten(),
            tl::types::MsgResendReq::CONSTRUCTOR_ID => self.handle_state_forgotten(),
            tl::types::MsgsAllInfo::CONSTRUCTOR_ID => self.handle_msg_all(),
            _ => self.handle_update(),
        }
        unimplemented!();
    }

    fn handle_rpc_result(&self) {
        unimplemented!();
    }
    fn handle_container(&self) {
        unimplemented!();
    }
    fn handle_gzip_packed(&self) {
        unimplemented!();
    }
    fn handle_pong(&self) {
        unimplemented!();
    }
    fn handle_bad_server_salt(&self) {
        unimplemented!();
    }
    fn handle_bad_notification(&self) {
        unimplemented!();
    }
    fn handle_detailed_info(&self) {
        unimplemented!();
    }
    fn handle_new_detailed_info(&self) {
        unimplemented!();
    }
    fn handle_new_session_created(&self) {
        unimplemented!();
    }
    fn handle_ack(&self) {
        unimplemented!();
    }
    fn handle_future_salts(&self) {
        unimplemented!();
    }
    fn handle_state_forgotten(&self) {
        unimplemented!();
    }
    fn handle_msg_all(&self) {
        unimplemented!();
    }
    fn handle_update(&self) {
        unimplemented!();
    }
}
