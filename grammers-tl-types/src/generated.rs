// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub use common::*;

mod common {
    include!(concat!(env!("OUT_DIR"), "/generated_common.rs"));
}

pub mod types {
    #![allow(
        clippy::cognitive_complexity,
        clippy::identity_op,
        clippy::unreadable_literal
    )]

    //! All of the bare types, each represented by a `struct`.
    //!
    //! All of them implement [`crate::Identifiable`], [`crate::Serializable`] and [`crate::Deserializable`].

    include!(concat!(env!("OUT_DIR"), "/generated_types.rs"));
}

pub mod enums {
    #![allow(clippy::large_enum_variant)]

    //! All of the boxed types, each represented by a `enum`.
    //!
    //! All of them implement [`crate::Serializable`] and [`crate::Deserializable`].

    include!(concat!(env!("OUT_DIR"), "/generated_enums.rs"));
}

pub mod functions {
    #![allow(
        clippy::cognitive_complexity,
        clippy::identity_op,
        clippy::unreadable_literal
    )]

    //! All of the functions, each represented by a `struct`.
    //!
    //! All of them implement [`crate::Identifiable`] and [`crate::Serializable`]
    //! (and, when the feature is enabled, [`crate::Deserializable`]).
    //!
    //! To find out the type that Telegram will return upon
    //! invoking one of these requests, check out the associated
    //! type in the corresponding [`crate::RemoteCall`] trait impl.

    include!(concat!(env!("OUT_DIR"), "/generated_functions.rs"));
}
