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
use std::io::SeekFrom;
use std::path::Path;
use tokio::{
    fs,
    io::{self, AsyncRead, AsyncReadExt as _, AsyncSeek, AsyncSeekExt as _, AsyncWriteExt as _},
};

pub const MIN_CHUNK_SIZE: i32 = 4 * 1024;
pub const MAX_CHUNK_SIZE: i32 = 512 * 1024;
const BIG_FILE_SIZE: usize = 10 * 1024 * 1024;

/// Generate a random file ID suitable for `upload_file`.
fn generate_random_file_id() -> i64 {
    let mut buffer = [0; 8];
    getrandom::getrandom(&mut buffer).expect("failed to generate random file id");
    i64::from_le_bytes(buffer)
}

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

/// Method implementations related to uploading or downloading files.
impl ClientHandle {
    /// Returns a new iterator over the contents of a media document that will be downloaded.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(file: grammers_tl_types::enums::InputFileLocation, mut client: grammers_client::ClientHandle) -> Result<(), Box<dyn std::error::Error>> {
    /// let mut file_bytes = Vec::new();
    /// let mut download = client.iter_download(file);
    ///
    /// while let Some(chunk) = download.next().await? {
    ///     file_bytes.extend(chunk);
    /// }
    ///
    /// // The file is now downloaded in-memory, inside `file_bytes`!
    /// # Ok(())
    /// # }
    /// ```
    pub fn iter_download(&self, file: tl::enums::InputFileLocation) -> DownloadIter {
        DownloadIter::new(self, file)
    }

    /// Downloads a media file into the specified path.
    ///
    /// If the file already exists, it will be overwritten.
    ///
    /// This is a small wrapper around [`ClientHandle::iter_download`] for the common case of
    /// wanting to save the file locally.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(file: grammers_tl_types::enums::InputFileLocation, mut client: grammers_client::ClientHandle) -> Result<(), Box<dyn std::error::Error>> {
    /// client.download_media(file, "/home/username/photos/holidays.jpg").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_media<P: AsRef<Path>>(
        &mut self,
        media: tl::enums::InputFileLocation,
        path: P,
    ) -> Result<(), io::Error> {
        let mut file = fs::File::create(path).await?;
        let mut download = self.iter_download(media);

        while let Some(chunk) = download
            .next()
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
        {
            file.write_all(&chunk).await?;
        }

        Ok(())
    }

    /// Uploads an async buffer to Telegram servers.
    ///
    /// The file is not sent to any chat, but can be used as media when sending messages for a
    /// certain period of time (less than a day). You can use this uploaded file multiple times.
    ///
    /// Refer to [`InputMessage`] to learn more uses for `uploaded_file`.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(chat: grammers_tl_types::enums::InputPeer, mut client: grammers_client::ClientHandle, stream: &mut std::io::Cursor<Vec<u8>>) -> Result<(), Box<dyn std::error::Error>> {
    /// use grammers_client::InputMessage;
    ///
    /// let uploaded_file = client.upload_stream(stream, Some("sleep.jpg".to_string())).await?;
    ///
    /// client.send_message(&chat, InputMessage::text("Check this out!").photo(uploaded_file)).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn upload_stream<S: AsyncRead + AsyncSeek + Unpin>(
        &mut self,
        stream: &mut S,
        name: Option<String>,
    ) -> Result<tl::enums::InputFile, io::Error> {
        let file_id = generate_random_file_id();
        let name: String = name.unwrap_or("a".into());

        let sz = stream.seek(SeekFrom::End(0)).await? as usize;
        let big_file = sz > BIG_FILE_SIZE;
        let mut buffer = vec![0; MAX_CHUNK_SIZE as usize];
        let total_parts = ((sz + buffer.len() - 1) / buffer.len()) as i32;
        let mut md5 = md5::Context::new();

        stream.seek(SeekFrom::Start(0)).await?;
        for part in 0..total_parts {
            let mut read = 0;
            while read != buffer.len() {
                let n = stream.read(&mut buffer[read..]).await?;
                if n == 0 {
                    if part == total_parts - 1 {
                        break;
                    } else {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "reached EOF before reaching the last file part",
                        ));
                    }
                }
                read += n;
            }
            let bytes = buffer[..read].to_vec();

            let ok = if big_file {
                self.invoke(&tl::functions::upload::SaveBigFilePart {
                    file_id,
                    file_part: part,
                    file_total_parts: total_parts,
                    bytes,
                })
                .await
            } else {
                md5.consume(&bytes);
                self.invoke(&tl::functions::upload::SaveFilePart {
                    file_id,
                    file_part: part,
                    bytes,
                })
                .await
            }
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            if !ok {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "server failed to store uploaded data",
                ));
            }
        }

        Ok(if big_file {
            tl::enums::InputFile::Big(tl::types::InputFileBig {
                id: file_id,
                parts: total_parts,
                name,
            })
        } else {
            tl::enums::InputFile::File(tl::types::InputFile {
                id: file_id,
                parts: total_parts,
                name,
                md5_checksum: format!("{:x}", md5.compute()),
            })
        })
    }

    /// Uploads a local file to Telegram servers.
    ///
    /// The file is not sent to any chat, but can be used as media when sending messages for a
    /// certain period of time (less than a day). You can use this uploaded file multiple times.
    ///
    /// Refer to [`InputMessage`] to learn more uses for `uploaded_file`.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(chat: grammers_tl_types::enums::InputPeer, mut client: grammers_client::ClientHandle) -> Result<(), Box<dyn std::error::Error>> {
    /// use grammers_client::InputMessage;
    ///
    /// let uploaded_file = client.upload_file("/home/username/photos/holidays.jpg").await?;
    ///
    /// client.send_message(&chat, InputMessage::text("Check this out!").photo(uploaded_file)).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`InputMessage`]: crate::InputMessage
    pub async fn upload_file<P: AsRef<Path>>(
        &mut self,
        path: P,
    ) -> Result<tl::enums::InputFile, io::Error> {
        let path = path.as_ref();
        let name = path.file_name().map(|n| n.to_string_lossy().to_string());
        let mut file = fs::File::open(path).await?;
        self.upload_stream(&mut file, name).await
    }
}
