use crate::tl::{Flag, Type};

/// A parameter type.
#[derive(Debug, PartialEq)]
pub enum ParameterType {
    /// This parameter represents a flags field (`u32`).
    Flags,

    /// A "normal" type, which may depend on a flag.
    Normal {
        /// The actual type of the parameter.
        ty: Type,

        /// If this parameter is conditional, which
        /// flag is used to determine its presence.
        flag: Option<Flag>,
    },
}
