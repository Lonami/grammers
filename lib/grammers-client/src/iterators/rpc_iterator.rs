use grammers_mtsender::InvocationError;

use crate::Client;

/// A trait implemented by iterators backed by RPC.
pub trait RPCIterator<T> {
    /// If the total amount of items corresponding to invoking this request
    /// is known, such value is returned. Note that this may not correspond
    /// with the amount of items that are yield.
    fn total(&self) -> Option<usize>;

    /// Pop the next item to yield from the buffer.
    fn pop_buffer(&mut self) -> Option<T>;

    /// Should the iterator fill its buffer before continuing?
    fn should_fill_buffer(&self) -> bool;

    /// Fills the iterator buffer by making another request.
    fn fill_buffer(&mut self, client: &mut Client) -> Result<(), InvocationError>;

    /// Advances the iterator and returns the next value.
    ///
    /// If the internal buffer is empty (e.g. when calling the method for the
    /// first time), a Remote Procedure Call will be made to fill it. If this
    /// fails, `Err` will be returned.
    ///
    /// If the internal buffer has items left to yield, `Ok(Some(T))` will be
    /// returned.
    ///
    /// Returns `Ok(None)` when iteration is finished.
    fn next(&mut self, client: &mut Client) -> Result<Option<T>, InvocationError> {
        if self.should_fill_buffer() {
            self.fill_buffer(client)?;
        }

        Ok(self.pop_buffer())
    }
}
