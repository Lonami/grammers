//! This module contains additional, manual structures for some TL types.
use grammers_tl_types::{self as tl, Deserializable};
use std::io;

/// This struct represents the following TL definition:
///
/// ```tl
/// message msg_id:long seqno:int bytes:int body:Object = Message;
/// ```
///
/// Messages are what's ultimately sent to Telegram.
///
/// Each message has its own unique identifier, and the body is simply
/// the serialized request that should be executed on the server, or
/// the response object from Telegram.
pub(crate) struct Message {
    pub msg_id: i64,
    pub seq_no: i32,
    pub bytes: i32,
    pub body: Vec<u8>,
}

impl Message {
    pub const SIZE_OVERHEAD: usize = 12;

    /// Peek the constructor ID from the body.
    pub fn constructor_id(&self) -> io::Result<u32> {
        let mut buffer = io::Cursor::new(&self.body);
        u32::deserialize(&mut buffer)
    }
}

/// This struct represents the following TL definition:
///
/// ```tl
/// rpc_result#f35c6d01 req_msg_id:long result:Object = RpcResult;
/// ```
pub(crate) struct RpcResult {
    pub req_msg_id: u64,
    pub result: Vec<u8>,
}

impl tl::Identifiable for RpcResult {
    const CONSTRUCTOR_ID: u32 = 0xf35c6d01_u32;
}

/// This struct represents the following TL definition:
///
/// ```tl
/// msg_container#73f1f8dc messages:vector<message> = MessageContainer;
/// ```
pub(crate) struct MessageContainer {
    pub messages: Vec<Message>,
}

impl MessageContainer {
    /// Maximum size in bytes for the inner payload of the container.
    /// Telegram will close the connection if the payload is bigger.
    /// The overhead of the container itself is subtracted.
    pub const MAXIMUM_SIZE: usize = 1044456 - 8;

    /// Maximum amount of messages that can't be sent inside a single
    /// container, inclusive. Beyond this limit Telegram will respond
    /// with `BAD_MESSAGE` `64` (invalid container).
    ///
    /// This limit is not 100% accurate and may in some cases be higher.
    /// However, sending up to 100 requests at once in a single container
    /// is a reasonable conservative value, since it could also depend on
    /// other factors like size per request, but we cannot know this.
    pub const MAXIMUM_LENGTH: usize = 100;
}

impl tl::Identifiable for MessageContainer {
    const CONSTRUCTOR_ID: u32 = 0x73f1f8dc_u32;
}

/// This struct represents the following TL definition:
///
/// ```tl
/// gzip_packed#3072cfa1 packed_data:string = Object;
/// ```
pub(crate) struct GzipPacked {
    pub packed_data: Vec<u8>,
}

impl GzipPacked {
    /// If the given data is larger than a certain threshold,
    /// and compressing it with gzip is benefitial, the compressed
    /// data is returned. Otherwise, the original data is returned.
    ///
    /// This should only be done for content-related requests.
    pub fn gzip_if_smaller() {
        /*
        if content_related and len(data) > 512:
            gzipped = bytes(GzipPacked(data))
            return gzipped if len(gzipped) < len(data) else data
        else:
            return data
        */
        unimplemented!();
    }
}

impl tl::Identifiable for GzipPacked {
    const CONSTRUCTOR_ID: u32 = 0x3072cfa1_u32;
}
