//! This module contains additional, manual structures for some TL types.
use grammers_tl_types::{Deserializable, Identifiable, Serializable};
use miniz_oxide::deflate::compress_to_vec;
use std::io::{self, Read, Write};

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
    pub body: Vec<u8>,
}

impl Message {
    // msg_id (8 bytes), seq_no (4 bytes), bytes (4 len)
    pub const SIZE_OVERHEAD: usize = 16;

    /// Peek the constructor ID from the body.
    pub fn _constructor_id(&self) -> io::Result<u32> {
        let mut buffer = io::Cursor::new(&self.body);
        u32::deserialize(&mut buffer)
    }

    /// Determine the size this serialized message will occupy.
    pub fn size(&self) -> usize {
        // msg_id (8 bytes), seq_no (4 bytes), bytes (4 len), data
        Self::SIZE_OVERHEAD + self.body.len()
    }
}

impl Serializable for Message {
    fn serialize<B: Write>(&self, buf: &mut B) -> io::Result<()> {
        self.msg_id.serialize(buf)?;
        self.seq_no.serialize(buf)?;
        (self.body.len() as i32).serialize(buf)?;
        buf.write_all(&self.body)?;
        Ok(())
    }
}

impl Deserializable for Message {
    fn deserialize<B: Read>(buf: &mut B) -> io::Result<Self> {
        let msg_id = i64::deserialize(buf)?;
        let seq_no = i32::deserialize(buf)?;
        let len = i32::deserialize(buf)?;
        // TODO check that this len is not ridiculously long
        let mut body = vec![0; len as usize];
        buf.read_exact(&mut body)?;

        Ok(Message {
            msg_id,
            seq_no,
            body,
        })
    }
}

/// This struct represents the following TL definition:
///
/// ```tl
/// rpc_result#f35c6d01 req_msg_id:long result:Object = RpcResult;
/// ```
pub(crate) struct _RpcResult {
    pub req_msg_id: u64,
    pub result: Vec<u8>,
}

impl Identifiable for _RpcResult {
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
    // constructor id (4 bytes), inner vec len (4 bytes)
    pub const SIZE_OVERHEAD: usize = 8;

    /// Maximum size in bytes for the inner payload of the container.
    /// Telegram will close the connection if the payload is bigger.
    /// The overhead of the container itself is subtracted.
    pub const MAXIMUM_SIZE: usize = 1044456 - Self::SIZE_OVERHEAD;

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

impl Identifiable for MessageContainer {
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
    pub fn new(unpacked_data: &[u8]) -> Self {
        Self {
            packed_data: compress_to_vec(unpacked_data, 9),
        }
    }
}

impl Identifiable for GzipPacked {
    const CONSTRUCTOR_ID: u32 = 0x3072cfa1_u32;
}

impl Serializable for GzipPacked {
    fn serialize<B: Write>(&self, buf: &mut B) -> io::Result<()> {
        Self::CONSTRUCTOR_ID.serialize(buf)?;
        self.packed_data.serialize(buf)?;
        Ok(())
    }
}
