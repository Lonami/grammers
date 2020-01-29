/// Data attached to parameters conditional on flags.
#[derive(Debug, PartialEq)]
pub struct Flag {
    /// The name of the field containing the flags.
    pub name: String,

    /// The bit index for the flag inside the flags variable.
    pub index: usize,
}
