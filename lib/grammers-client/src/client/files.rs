// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::ClientHandle;
use grammers_mtsender::InvocationError;
use grammers_tl_types as tl;

pub const MIN_CHUNK_SIZE: i32 = 4 * 1024;
pub const MAX_CHUNK_SIZE: i32 = 512 * 1024;

pub struct DownloadIter {
    client: ClientHandle,
    done: bool,
    request: tl::functions::upload::GetFile,
}

impl DownloadIter {
    fn new(client: &ClientHandle, location: tl::enums::InputFileLocation) -> Self {
        // TODO let users tweak all the options from the request
        // TODO cdn support
        Self {
            client: client.clone(),
            done: false,
            request: tl::functions::upload::GetFile {
                precise: false,
                cdn_supported: false,
                location,
                offset: 0,
                limit: MAX_CHUNK_SIZE,
            },
        }
    }

    /// Changes the chunk size, in bytes, used to make requests. Useful if you only need to get a
    /// small part of a file. By default, `MAX_CHUNK_SIZE` is used.
    ///
    /// # Panics
    ///
    /// Panics if `size` is not divisible by `MIN_CHUNK_SIZE`, or if `size` is not in contained in
    /// the range `MIN_CHUNK_SIZE..=MAX_CHUNK_SIZE`.
    pub fn chunk_size(mut self, size: i32) -> Self {
        assert!(MIN_CHUNK_SIZE <= size && size <= MAX_CHUNK_SIZE && size % MIN_CHUNK_SIZE == 0);
        self.request.limit = size as i32;
        self
    }

    /// Skips `n` chunks to start downloading a different offset from the file. If you want to
    /// skip less data, modify the `chunk_size` before calling this method, and then reset it to
    /// any value you want.
    pub fn skip_chunks(mut self, n: i32) -> Self {
        self.request.offset += self.request.limit * n;
        self
    }

    /// Fetch and return the next chunk.
    pub async fn next(&mut self) -> Result<Option<Vec<u8>>, InvocationError> {
        if self.done {
            return Ok(None);
        }

        use tl::enums::upload::File;

        // TODO handle FILE_MIGRATE and maybe FILEREF_UPGRADE_NEEDED
        match self.client.invoke(&self.request).await? {
            File::File(f) => {
                if f.bytes.len() < self.request.limit as usize {
                    self.done = true;
                    if f.bytes.is_empty() {
                        return Ok(None);
                    }
                }

                Ok(Some(f.bytes))
            }
            File::CdnRedirect(_) => {
                panic!("API returned File::CdnRedirect even though cdn_supported = false");
            }
        }
    }
}

impl ClientHandle {
    /// Returns a new iterator over the contents of a media document that will be downloaded.
    pub fn iter_download(&self, file: tl::enums::InputFileLocation) -> DownloadIter {
        DownloadIter::new(self, file)
    }
}
