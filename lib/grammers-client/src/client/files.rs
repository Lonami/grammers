// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::types::{Media, Uploaded};
use crate::utils::{generate_random_id, AsyncMutex};
use crate::Client;
use futures_util::future::try_join_all;
use grammers_mtsender::InvocationError;
use grammers_tl_types as tl;
use std::{io::SeekFrom, path::Path, sync::Arc};
use tokio::{
    fs,
    io::{self, AsyncRead, AsyncReadExt as _, AsyncSeekExt as _, AsyncWriteExt as _},
};

pub const MIN_CHUNK_SIZE: i32 = 4 * 1024;
pub const MAX_CHUNK_SIZE: i32 = 512 * 1024;
const BIG_FILE_SIZE: usize = 10 * 1024 * 1024;
const WORKER_COUNT: usize = 4;

pub struct DownloadIter {
    client: Client,
    done: bool,
    request: tl::functions::upload::GetFile,
}

impl DownloadIter {
    fn new(client: &Client, media: &Media) -> Self {
        DownloadIter::new_from_file_location(client, media.to_input_location().unwrap())
    }

    fn new_from_location(client: &Client, location: tl::enums::InputFileLocation) -> Self {
        DownloadIter::new_from_file_location(client, location)
    }

    fn new_from_file_location(client: &Client, location: tl::enums::InputFileLocation) -> Self {
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

                self.request.offset += self.request.limit;
                Ok(Some(f.bytes))
            }
            File::CdnRedirect(_) => {
                panic!("API returned File::CdnRedirect even though cdn_supported = false");
            }
        }
    }
}

/// Method implementations related to uploading or downloading files.
impl Client {
    /// Returns a new iterator over the contents of a media document that will be downloaded.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(media: grammers_client::types::Media, mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let mut file_bytes = Vec::new();
    /// let mut download = client.iter_download(&media);
    ///
    /// while let Some(chunk) = download.next().await? {
    ///     file_bytes.extend(chunk);
    /// }
    ///
    /// // The file is now downloaded in-memory, inside `file_bytes`!
    /// # Ok(())
    /// # }
    /// ```
    pub fn iter_download(&self, media: &Media) -> DownloadIter {
        DownloadIter::new(self, media)
    }

    /// Downloads a media file into the specified path.
    ///
    /// If the file already exists, it will be overwritten.
    ///
    /// This is a small wrapper around [`Client::iter_download`] for the common case of
    /// wanting to save the file locally.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(media: grammers_client::types::Media, mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// client.download_media(&media, "/home/username/photos/holidays.jpg").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_media<P: AsRef<Path>>(
        &self,
        media: &Media,
        path: P,
    ) -> Result<(), io::Error> {
        let mut download = self.iter_download(media);

        Client::load(path, &mut download).await
    }

    pub(crate) async fn download_media_at_location<P: AsRef<Path>>(
        &self,
        location: tl::enums::InputFileLocation,
        path: P,
    ) -> Result<(), io::Error> {
        let mut download = DownloadIter::new_from_location(self, location);

        Client::load(path, &mut download).await
    }

    async fn load<P: AsRef<Path>>(path: P, download: &mut DownloadIter) -> Result<(), io::Error> {
        let mut file = fs::File::create(path).await?;
        while let Some(chunk) = download
            .next()
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
        {
            file.write_all(&chunk).await?;
        }

        Ok(())
    }

    /// Uploads an async stream to Telegram servers.
    ///
    /// The file is not sent to any chat, but can be used as media when sending messages for a
    /// certain period of time (less than a day). You can use this uploaded file multiple times.
    ///
    /// Refer to [`InputMessage`] to learn more uses for `uploaded_file`.
    ///
    /// The stream size must be known beforehand. If this is not possible, you might need to
    /// process the entire async stream to determine its size, and then use the size and the
    /// downloaded buffer.
    ///
    /// The stream size may be less or equal to the actual length of the stream, but not more.
    /// If it's less, you may continue to read from the stream after the method returns.
    /// If it's more, the method will fail because it does not have enough data to read.
    ///
    /// Note that Telegram uses the file name in certain methods, for example, to make sure the
    /// file is an image when trying to use send the file as photo media, so it is important that
    /// the file name at least uses the right extension, even if the name is a dummy value.
    /// If the input file name is empty, the non-empty dummy value "a" will be used instead.
    /// Because it has no extension, you may not be able to use the file in certain methods.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(chat: grammers_client::types::Chat, client: grammers_client::Client, some_vec: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    /// use grammers_client::InputMessage;
    ///
    /// // In-memory `Vec<u8>` buffers can be used as async streams
    /// let size = some_vec.len();
    /// let mut stream = std::io::Cursor::new(some_vec);
    /// let uploaded_file = client.upload_stream(&mut stream, size, "sleep.jpg".to_string()).await?;
    ///
    /// client.send_message(&chat, InputMessage::text("Zzz...").photo(uploaded_file)).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`InputMessage`]: crate::types::InputMessage
    pub async fn upload_stream<S: AsyncRead + Unpin>(
        &self,
        stream: &mut S,
        size: usize,
        name: String,
    ) -> Result<Uploaded, io::Error> {
        let file_id = generate_random_id();
        let name = if name.is_empty() {
            "a".to_string()
        } else {
            name
        };

        let big_file = size > BIG_FILE_SIZE;
        let parts = PartStream::new(stream, size);
        let total_parts = parts.total_parts();

        if big_file {
            let parts = Arc::new(parts);
            let mut tasks = Vec::with_capacity(WORKER_COUNT);
            for _ in 0..WORKER_COUNT {
                let handle = self.clone();
                let parts = Arc::clone(&parts);
                let task = async move {
                    while let Some((part, bytes)) = parts.next_part().await? {
                        let ok = handle
                            .invoke(&tl::functions::upload::SaveBigFilePart {
                                file_id,
                                file_part: part,
                                file_total_parts: total_parts,
                                bytes,
                            })
                            .await
                            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

                        if !ok {
                            return Err(io::Error::new(
                                io::ErrorKind::Other,
                                "server failed to store uploaded data",
                            ));
                        }
                    }
                    Ok(())
                };
                tasks.push(task);
            }

            try_join_all(tasks).await?;

            Ok(Uploaded::from_raw(
                tl::types::InputFileBig {
                    id: file_id,
                    parts: total_parts,
                    name,
                }
                .into(),
            ))
        } else {
            let mut md5 = md5::Context::new();
            while let Some((part, bytes)) = parts.next_part().await? {
                md5.consume(&bytes);
                let ok = self
                    .invoke(&tl::functions::upload::SaveFilePart {
                        file_id,
                        file_part: part,
                        bytes,
                    })
                    .await
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

                if !ok {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        "server failed to store uploaded data",
                    ));
                }
            }
            Ok(Uploaded::from_raw(
                tl::types::InputFile {
                    id: file_id,
                    parts: total_parts,
                    name,
                    md5_checksum: format!("{:x}", md5.compute()),
                }
                .into(),
            ))
        }
    }

    /// Uploads a local file to Telegram servers.
    ///
    /// The file is not sent to any chat, but can be used as media when sending messages for a
    /// certain period of time (less than a day). You can use this uploaded file multiple times.
    ///
    /// Refer to [`InputMessage`] to learn more uses for `uploaded_file`.
    ///
    /// If you need more control over the uploaded data, such as performing only a partial upload
    /// or with a different name, use [`Client::upload_stream`] instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(chat: grammers_client::types::Chat, mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
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
    pub async fn upload_file<P: AsRef<Path>>(&self, path: P) -> Result<Uploaded, io::Error> {
        let path = path.as_ref();

        let mut file = fs::File::open(path).await?;
        let size = file.seek(SeekFrom::End(0)).await? as usize;
        file.seek(SeekFrom::Start(0)).await?;

        // File name will only be `None` for `..` path, and directories cannot be uploaded as
        // files, so it's fine to unwrap.
        let name = path.file_name().unwrap().to_string_lossy().to_string();

        self.upload_stream(&mut file, size, name).await
    }
}

struct PartStreamInner<'a, S: AsyncRead + Unpin> {
    stream: &'a mut S,
    current_part: i32,
}

struct PartStream<'a, S: AsyncRead + Unpin> {
    inner: AsyncMutex<PartStreamInner<'a, S>>,
    total_parts: i32,
}

impl<'a, S: AsyncRead + Unpin> PartStream<'a, S> {
    fn new(stream: &'a mut S, size: usize) -> Self {
        let total_parts = ((size + MAX_CHUNK_SIZE as usize - 1) / MAX_CHUNK_SIZE as usize) as i32;
        Self {
            inner: AsyncMutex::new(
                "upload_stream",
                PartStreamInner {
                    stream,
                    current_part: 0,
                },
            ),
            total_parts,
        }
    }

    fn total_parts(&self) -> i32 {
        self.total_parts
    }

    async fn next_part(&self) -> Result<Option<(i32, Vec<u8>)>, io::Error> {
        let mut lock = self.inner.lock("read part").await;
        if lock.current_part >= self.total_parts {
            return Ok(None);
        }
        let mut read = 0;
        let mut buffer = vec![0; MAX_CHUNK_SIZE as usize];

        while read != buffer.len() {
            let n = lock.stream.read(&mut buffer[read..]).await?;
            if n == 0 {
                if lock.current_part == self.total_parts - 1 {
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

        let bytes = if read == buffer.len() {
            buffer
        } else {
            buffer[..read].to_vec()
        };

        let res = Ok(Some((lock.current_part, bytes)));
        lock.current_part += 1;
        res
    }
}
