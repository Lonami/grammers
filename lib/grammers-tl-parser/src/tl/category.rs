/// The category to which a definition belongs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Category {
    /// The default category, a definition represents a type.
    Types,

    /// A definition represents a callable function.
    Functions,
}
