use std::fmt;

/// Data attached to parameters conditional on flags.
#[derive(Debug, PartialEq)]
pub struct Flag {
    /// The name of the field containing the flags.
    pub name: String,

    /// The bit index for the flag inside the flags variable.
    pub index: usize,
}

impl fmt::Display for Flag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.name, self.index)
    }
}
