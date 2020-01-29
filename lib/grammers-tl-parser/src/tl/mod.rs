//! This module contains all the different structures representing the
//! various terms of the [Type Language].
///
/// [Type Language]: https://core.telegram.org/mtproto/TL
mod category;
mod definition;
mod flag;
mod parameter;
mod parameter_type;
mod ty;

pub use category::Category;
pub use definition::Definition;
pub use flag::Flag;
pub use parameter::Parameter;
pub use parameter_type::ParameterType;
pub use ty::Type;
