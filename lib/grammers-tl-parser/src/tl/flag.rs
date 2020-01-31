// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use std::fmt;
use std::str::FromStr;

use crate::errors::ParamParseError;

/// Data attached to parameters conditional on flags.
#[derive(Debug, PartialEq)]
pub struct Flag {
    /// The name of the parameter containing the flags in its bits.
    pub name: String,

    /// The bit index used by this flag inside the flags parameter.
    pub index: usize,
}

impl fmt::Display for Flag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.name, self.index)
    }
}

impl FromStr for Flag {
    type Err = ParamParseError;

    /// Parses a flag.
    ///
    /// # Examples
    ///
    /// ```
    /// use grammers_tl_parser::tl::Flag;
    ///
    /// assert!("flags.1".parse::<Flag>().is_ok());
    /// ```
    fn from_str(ty: &str) -> Result<Self, Self::Err> {
        if let Some(dot_pos) = ty.find('.') {
            Ok(Flag {
                name: ty[..dot_pos].into(),
                index: ty[dot_pos + 1..]
                    .parse()
                    .map_err(|_| ParamParseError::InvalidFlag)?,
            })
        } else {
            Err(ParamParseError::InvalidFlag)
        }
    }
}
