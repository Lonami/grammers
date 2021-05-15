// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! This library contains the Rust definitions for Telegram's [`types`] and
//! [`functions`] in the form of `struct` and `enum`. All of them implement
//! [`Serializable`], and by default only types implement [`Deserializable`].
//!
//! If you're here because you want to invoke the "raw API" methods Telegram
//! has to offer, use the search or read through the available [`functions`]
//! to find out what "parameters" such remote call needs. Then, create an
//! instance of it and pass it to some higher level library that can talk
//! with Telegram's servers.
//!
//! To preserve compatibility with older applications, the API has a concept
//! of "layers", where new layers can change, remove or add new  definitions.
//! The [`LAYER`] constant indicates which of the many layers was used to
//! generate the definitions present in this version of this crate.
//!
//! # Usage
//!
//! The primary purpose is using these definitions to create requests
//! (known as [`functions`]) which can be serialized and sent to Telegram.
//! Note that this crate has no way to "invoke" any of these requests,
//! this is to be done by a higher-level crate.
//!
//! All of the requests implement [`RemoteCall`]. This trait's associated
//! type indicates what type the response from Telegram will be when invoked.
//!
//! After opening one of the many [`types`], you can inspect their fields
//! to figure out what data Telegram will return.
//!
//! # Features
//!
//! The default feature set is intended to make the use of the library
//! comfortable, and not intended to be minimal in code size. If you need
//! a smaller library or are concerned about build-times, consider disabling
//! some of the default features.
//!
//! The default feature set includes:
//!
//! * `impl-debug`.
//! * `impl-from-enum`.
//! * `impl-from-type`.
//! * `tl-api`.
//!
//! The available features are:
//!
//! * `deserializable-functions`: implements [`Deserializable`] for
//!   [`functions`]. This might be of interest for server implementations,
//!   which need to deserialize the client's requests, but is otherwise not
//!   required.
//!
//! * `impl-debug`: implements `Debug` for the generated code.
//!
//! * `impl-from-enum`: implements `TryFrom<Enum> for Type`.
//!
//! * `impl-from-type`: implements `From<Type> for Enum`.
//!
//! * `tl-api`: generates code for the `api.tl`.
//!   This is what high-level libraries often need.
//!
//! * `tl-mtproto`: generates code for the `mtproto.tl`.
//!   Only useful for low-level libraries.
//!
//! [`types`]: types/index.html
//! [`functions`]: functions/index.html
//! [`RemoteCall`]: trait.RemoteCall.html
//! [`Serializable`]: trait.Serializable.html
//! [`Deserializable`]: trait.Deserializable.html
//! [`LAYER`]: constant.LAYER.html
pub mod deserialize;
mod generated;
pub mod serialize;

pub use deserialize::{Cursor, Deserializable};
pub use generated::{enums, functions, name_for_id, types, LAYER};
pub use serialize::Serializable;

/// This struct represents the concrete type of a vector, that is,
/// `vector` as opposed to the type `Vector`. This bare type is less
/// common, so instead of creating a enum for `Vector` wrapping `vector`
/// as Rust's `Vec` (as we would do with auto-generated code),
/// a new-type for `vector` is used instead.
#[derive(Clone, Debug, PartialEq)]
pub struct RawVec<T>(pub Vec<T>);

/// This struct represents an unparsed blob, which should not be deserialized
/// as a bytes string. Used by functions returning generic objects which pass
/// the underlying result without any modification or interpretation.
#[derive(Clone, Debug, PartialEq)]
pub struct Blob(pub Vec<u8>);

impl From<Vec<u8>> for Blob {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

/// Anything implementing this trait is identifiable by both ends (client-server)
/// when performing Remote Procedure Calls (RPC) and transmission of objects.
pub trait Identifiable {
    /// The unique identifier for the type.
    const CONSTRUCTOR_ID: u32;
}

/// Structures implementing this trait indicate that they are suitable for
/// use to perform Remote Procedure Calls (RPC), and know what the type of
/// the response will be.
pub trait RemoteCall: Serializable {
    /// The type of the "return" value coming from the other end of the
    /// connection.
    type Return: Deserializable;
}
