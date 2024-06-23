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
use crate::tl::{Flag, Type};

/// A parameter type.
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum ParameterType {
    /// This parameter represents a flags field (`u32`).
    Flags,

    /// A "normal" type, which may depend on a flag.
    Normal {
        /// The actual type of the parameter.
        ty: Type,

        /// If this parameter is only present sometimes,
        /// the flag upon which its presence depends.
        flag: Option<Flag>,
    },
}

impl fmt::Display for ParameterType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Flags => write!(f, "#"),
            Self::Normal { ty, flag } => {
                if let Some(flag) = flag {
                    write!(f, "{flag}?")?;
                }
                write!(f, "{ty}")
            }
        }
    }
}

impl FromStr for ParameterType {
    type Err = ParamParseError;

    /// Parses the type of a parameter.
    ///
    /// # Examples
    ///
    /// ```
    /// use grammers_tl_parser::tl::ParameterType;
    ///
    /// assert!("vector<int>".parse::<ParameterType>().is_ok());
    /// ```
    fn from_str(ty: &str) -> Result<Self, Self::Err> {
        if ty.is_empty() {
            return Err(ParamParseError::Empty);
        }

        // Parse `#`
        if ty == "#" {
            return Ok(ParameterType::Flags);
        }

        // Parse `flag_name.flag_index?type`
        let (ty, flag) = if let Some(pos) = ty.find('?') {
            (&ty[pos + 1..], Some(ty[..pos].parse()?))
        } else {
            (ty, None)
        };

        // Parse `type<generic_arg>`
        let ty = ty.parse()?;

        Ok(ParameterType::Normal { ty, flag })
    }
}
