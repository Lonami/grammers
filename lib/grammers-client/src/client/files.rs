// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::types::{photo_sizes::PhotoSize, Downloadable, Media, Uploaded};
use crate::utils::generate_random_id;
use crate::Client;
use futures_util::stream::{FuturesUnordered, StreamExt as _};
use grammers_mtsender::InvocationError;
use grammers_tl_types as tl;
use std::{io::SeekFrom, path::Path, sync::Arc};
use tokio::sync::mpsc::unbounded_channel;
use tokio::{
    fs,
    io::{self, AsyncRead, AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
    sync::Mutex as AsyncMutex,
};

pub const MIN_CHUNK_SIZE: i32 = 4 * 1024;
pub const MAX_CHUNK_SIZE: i32 = 512 * 1024;
const FILE_MIGRATE_ERROR: i32 = 303;
const BIG_FILE_SIZE: usize = 10 * 1024 * 1024;
const WORKER_COUNT: usize = 4;

pub struct DownloadIter {
    client: Client,
    done: bool,
    request: tl::functions::upload::GetFile,
    photo_size_data: Option<Vec<u8>>,
}

impl DownloadIter {
    fn new(client: &Client, downloadable: &Downloadable) -> Self {
        match downloadable {
            Downloadable::PhotoSize(photo_size)
                if !matches!(photo_size, PhotoSize::Size(_) | PhotoSize::Progressive(_)) =>
            {
                Self::new_from_photo_size(client, photo_size.data())
            }
            _ => {
                Self::new_from_file_location(client, downloadable.to_raw_input_location().unwrap())
            }
        }
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
            photo_size_data: None,
        }
    }

    fn new_from_photo_size(client: &Client, data: Vec<u8>) -> Self {
        Self {
            client: client.clone(),
            done: false,
            // request is not needed, so fake one
            request: tl::functions::upload::GetFile {
                precise: false,
                cdn_supported: false,
                location: tl::enums::InputFileLocation::InputPhotoFileLocation(
                    tl::types::InputPhotoFileLocation {
                        id: 0,
                        access_hash: 0,
                        file_reference: vec![],
                        thumb_size: "".to_string(),
                    },
                ),
                offset: 0,
                limit: MAX_CHUNK_SIZE,
            },
            photo_size_data: Some(data),
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
        assert!((MIN_CHUNK_SIZE..=MAX_CHUNK_SIZE).contains(&size) && size % MIN_CHUNK_SIZE == 0);
        self.request.limit = size;
        self
    }

    /// Skips `n` chunks to start downloading a different offset from the file. If you want to
    /// skip less data, modify the `chunk_size` before calling this method, and then reset it to
    /// any value you want.
    pub fn skip_chunks(mut self, n: i32) -> Self {
        self.request.offset += (self.request.limit * n) as i64;
        self
    }

    /// Fetch and return the next chunk.
    pub async fn next(&mut self) -> Result<Option<Vec<u8>>, InvocationError> {
        if self.done {
            return Ok(None);
        }

        if let Some(data) = &self.photo_size_data {
            self.done = true;
            return Ok(Some(data.clone()));
        }

        use tl::enums::upload::File;

        // TODO handle maybe FILEREF_UPGRADE_NEEDED
        let mut dc: Option<u32> = None;
        loop {
            let result = match dc.take() {
                None => self.client.invoke(&self.request).await,
                Some(dc) => self.client.invoke_in_dc(&self.request, dc as i32).await,
            };

            break match result {
                Ok(File::File(f)) => {
                    if f.bytes.len() < self.request.limit as usize {
                        self.done = true;
                        if f.bytes.is_empty() {
                            return Ok(None);
                        }
                    }

                    self.request.offset += self.request.limit as i64;
                    Ok(Some(f.bytes))
                }
                Ok(File::CdnRedirect(_)) => {
                    panic!("API returned File::CdnRedirect even though cdn_supported = false");
                }
                Err(InvocationError::Rpc(err)) if err.code == FILE_MIGRATE_ERROR => {
                    dc = err.value;
                    continue;
                }
                Err(e) => Err(e),
            };
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
    /// # async fn f(downloadable: grammers_client::types::Downloadable, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let mut file_bytes = Vec::new();
    /// let mut download = client.iter_download(&downloadable);
    ///
    /// while let Some(chunk) = download.next().await? {
    ///     file_bytes.extend(chunk);
    /// }
    ///
    /// // The file is now downloaded in-memory, inside `file_bytes`!
    /// # Ok(())
    /// # }
    /// ```
    pub fn iter_download(&self, downloadable: &Downloadable) -> DownloadIter {
        DownloadIter::new(self, downloadable)
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
    /// # async fn f(downloadable: grammers_client::types::Downloadable, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// client.download_media(&downloadable, "/home/username/photos/holidays.jpg").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_media<P: AsRef<Path>>(
        &self,
        downloadable: &Downloadable,
        path: P,
    ) -> Result<(), io::Error> {
        // Concurrent downloader
        if let Downloadable::Media(media) = downloadable {
            if let Media::Document(document) = media {
                if document.size() as usize > BIG_FILE_SIZE {
                    return self
                        .download_media_concurrent(media, path, WORKER_COUNT)
                        .await;
                }
            }
        }

        if downloadable.to_raw_input_location().is_none() {
            let data = match downloadable {
                Downloadable::PhotoSize(photo_size)
                    if !matches!(photo_size, PhotoSize::Size(_) | PhotoSize::Progressive(_)) =>
                {
                    photo_size.data()
                }
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        "media not downloadable",
                    ));
                }
            };

            if !data.is_empty() {
                let mut file = fs::File::create(&path).await.unwrap();
                file.write_all(&data).await.unwrap();
            }

            return Ok(());
        }

        let mut download = self.iter_download(downloadable);
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

    /// Downloads a `Document` to specified path using multiple connections
    async fn download_media_concurrent<P: AsRef<Path>>(
        &self,
        media: &Media,
        path: P,
        workers: usize,
    ) -> Result<(), io::Error> {
        let document = match media {
            Media::Document(document) => document,
            _ => panic!("Only Document type is supported!"),
        };
        let size = document.size();
        let location = media.to_raw_input_location().unwrap();
        // Allocate
        let mut file = fs::File::create(path).await?;
        file.set_len(size as u64).await?;
        file.seek(SeekFrom::Start(0)).await?;

        // Start workers
        let (tx, mut rx) = unbounded_channel();
        let part_index = Arc::new(tokio::sync::Mutex::new(0));
        let mut tasks = vec![];
        for _ in 0..workers {
            let location = location.clone();
            let tx = tx.clone();
            let part_index = part_index.clone();
            let client = self.clone();
            let task = tokio::task::spawn(async move {
                let mut retry_offset = None;
                let mut dc = None;
                loop {
                    // Calculate file offset
                    let offset: i64 = {
                        if let Some(offset) = retry_offset {
                            retry_offset = None;
                            offset
                        } else {
                            let mut i = part_index.lock().await;
                            *i += 1;
                            (MAX_CHUNK_SIZE * (*i - 1)) as i64
                        }
                    };
                    if offset > size {
                        break;
                    }
                    // Fetch from telegram
                    let request = &tl::functions::upload::GetFile {
                        precise: true,
                        cdn_supported: false,
                        location: location.clone(),
                        offset,
                        limit: MAX_CHUNK_SIZE,
                    };
                    let res = match dc {
                        None => client.invoke(request).await,
                        Some(dc) => client.invoke_in_dc(request, dc as i32).await,
                    };
                    match res {
                        Ok(tl::enums::upload::File::File(file)) => {
                            tx.send((offset as u64, file.bytes)).unwrap();
                        }
                        Ok(tl::enums::upload::File::CdnRedirect(_)) => {
                            panic!(
                                "API returned File::CdnRedirect even though cdn_supported = false"
                            );
                        }
                        Err(InvocationError::Rpc(err)) => {
                            if err.code == FILE_MIGRATE_ERROR {
                                dc = err.value;
                                retry_offset = Some(offset);
                                continue;
                            }
                            return Err(InvocationError::Rpc(err));
                        }
                        Err(e) => return Err(e),
                    }
                }
                Ok::<(), InvocationError>(())
            });
            tasks.push(task);
        }
        drop(tx);

        // File write loop
        let mut pos = 0;
        while let Some((offset, data)) = rx.recv().await {
            if offset != pos {
                file.seek(SeekFrom::Start(offset)).await?;
            }
            file.write_all(&data).await?;
            pos = offset + data.len() as u64;
        }

        // Check if all tasks finished succesfully
        for task in tasks {
            task.await?
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
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
            let mut tasks = FuturesUnordered::new();
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

            while let Some(res) = tasks.next().await {
                res?;
            }

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
    /// # async fn f(chat: grammers_client::types::Chat, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
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
            inner: AsyncMutex::new(PartStreamInner {
                stream,
                current_part: 0,
            }),
            total_parts,
        }
    }

    fn total_parts(&self) -> i32 {
        self.total_parts
    }

    async fn next_part(&self) -> Result<Option<(i32, Vec<u8>)>, io::Error> {
        let mut lock = self.inner.lock().await;
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
