// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use super::attributes::Attribute;
use crate::types::{Media, Uploaded};
use grammers_tl_types as tl;

/// Construct and send albums.
#[derive(Default)]
pub struct InputMedia {
    pub(crate) entities: Vec<tl::enums::MessageEntity>,
    pub(crate) reply_to: Option<i32>,
    pub(crate) caption: String,
    pub(crate) media: Option<tl::enums::InputMedia>,
    media_ttl: Option<i32>,
    mime_type: Option<String>,
}

impl InputMedia {
    /// Creates a new empty media message for input.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the plaintext in the message.
    ///
    /// The caller must ensure that formatting entities remain valid for the given text.
    /// If you need to update formatting entities, call method [`InputMedia::fmt_entities`].
    ///
    /// <div class="warning">
    /// Note that this method does not modify formatting entities, which may break
    /// formatting or cause out-of-bounds errors if entities do not match the given text.
    /// </div>
    pub fn caption<T>(mut self, s: T) -> Self
    where
        T: Into<String>,
    {
        self.caption = s.into();
        self
    }

    /// The formatting entities within the caption (such as bold, italics, etc.).
    pub fn fmt_entities(mut self, entities: Vec<tl::enums::MessageEntity>) -> Self {
        self.entities = entities;
        self
    }

    /// Builds a new media from the given markdown-formatted string as the
    /// caption contents and entities.
    ///
    /// Note that Telegram only supports a very limited subset of entities:
    /// bold, italic, underline, strikethrough, code blocks, pre blocks and inline links (inline
    /// links with this format `tg://user?id=12345678` will be replaced with inline mentions when
    /// possible).
    #[cfg(feature = "markdown")]
    pub fn markdown<T>(mut self, s: T) -> Self
    where
        T: AsRef<str>,
    {
        let (caption, entities) = crate::parsers::parse_markdown_message(s.as_ref());
        self.caption = caption;
        self.entities = entities;
        self
    }

    /// Builds a new media from the given HTML-formatted string as the
    /// caption contents and entities.
    ///
    /// Note that Telegram only supports a very limited subset of entities:
    /// bold, italic, underline, strikethrough, code blocks, pre blocks and inline links (inline
    /// links with this format `tg://user?id=12345678` will be replaced with inline mentions when
    /// possible).
    #[cfg(feature = "html")]
    pub fn html<T>(mut self, s: T) -> Self
    where
        T: AsRef<str>,
    {
        let (caption, entities) = crate::parsers::parse_html_message(s.as_ref());
        self.caption = caption;
        self.entities = entities;
        self
    }

    /// The album identifier to which this album should reply to, if any.
    ///
    /// Otherwise, this album will not be a reply to any other.
    ///
    /// Only the reply_to from the first media is used.
    pub fn reply_to(mut self, reply_to: Option<i32>) -> Self {
        self.reply_to = reply_to;
        self
    }

    /// Include the uploaded file as a photo in the album.
    ///
    /// The Telegram server will compress the image and convert it to JPEG format if necessary.
    ///
    /// The text will be the caption of the photo, which may be empty for no caption.
    pub fn photo(mut self, file: Uploaded) -> Self {
        self.media = Some(
            (tl::types::InputMediaUploadedPhoto {
                spoiler: false,
                file: file.raw,
                stickers: None,
                ttl_seconds: self.media_ttl,
            })
            .into(),
        );
        self
    }

    /// Include an external photo in the album.
    ///
    /// The Telegram server will download and compress the image and convert it to JPEG format if
    /// necessary.
    ///
    /// The text will be the caption of the photo, which may be empty for no caption.
    pub fn photo_url(mut self, url: impl Into<String>) -> Self {
        self.media = Some(
            (tl::types::InputMediaPhotoExternal {
                spoiler: false,
                url: url.into(),
                ttl_seconds: self.media_ttl,
            })
            .into(),
        );
        self
    }

    /// Include the uploaded file as a document in the album.
    ///
    /// You can use this to send videos, stickers, audios, or uncompressed photos.
    ///
    /// The text will be the caption of the document, which may be empty for no caption.
    pub fn document(mut self, file: Uploaded) -> Self {
        let mime_type = self.get_file_mime(&file);
        let file_name = file.name().to_string();
        self.media = Some(
            (tl::types::InputMediaUploadedDocument {
                nosound_video: false,
                force_file: false,
                spoiler: false,
                file: file.raw,
                thumb: None,
                mime_type,
                attributes: vec![(tl::types::DocumentAttributeFilename { file_name }).into()],
                stickers: None,
                ttl_seconds: self.media_ttl,
                video_cover: None,
                video_timestamp: None,
            })
            .into(),
        );
        self
    }

    /// Include the video file with thumb in the album.
    ///
    /// The text will be the caption of the document, which may be empty for no caption.
    ///
    /// # Examples
    ///
    /// ```
    /// async fn f(client: &mut grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    ///     use grammers_client::{InputMedia};
    ///
    ///     let video = client.upload_file("video.mp4").await?;
    ///     let thumb = client.upload_file("thumb.png").await?;
    ///     let media = InputMedia::new().caption("").document(video).thumbnail(thumb);
    ///     Ok(())
    /// }
    /// ```
    pub fn thumbnail(mut self, thumb: Uploaded) -> Self {
        if let Some(tl::enums::InputMedia::UploadedDocument(document)) = &mut self.media {
            document.thumb = Some(thumb.raw);
        }
        self
    }

    /// Include an external file as a document in the album.
    ///
    /// You can use this to send videos, stickers, audios, or uncompressed photos.
    ///
    /// The Telegram server will be the one that downloads and includes the document as media.
    ///
    /// The text will be the caption of the document, which may be empty for no caption.
    pub fn document_url(mut self, url: impl Into<String>) -> Self {
        self.media = Some(
            (tl::types::InputMediaDocumentExternal {
                spoiler: false,
                url: url.into(),
                ttl_seconds: self.media_ttl,
                video_cover: None,
                video_timestamp: None,
            })
            .into(),
        );
        self
    }

    /// Add additional attributes to the media.
    ///
    /// This must be called *after* setting a file.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(client: &mut grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// # let audio = client.upload_file("audio.flac").await?;
    /// #
    /// use std::time::Duration;
    /// use grammers_client::{types::Attribute, InputMedia};
    ///
    /// let media = InputMedia::new().caption("").document(audio).attribute(
    ///    Attribute::Audio {
    ///        duration: Duration::new(123, 0),
    ///        title: Some("Hello".to_string()),
    ///        performer: Some("World".to_string()),
    ///    }
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub fn attribute(mut self, attr: Attribute) -> Self {
        if let Some(tl::enums::InputMedia::UploadedDocument(document)) = &mut self.media {
            document.attributes.push(attr.into());
        }
        self
    }

    /// Copy media from an existing message.
    ///
    /// You can use this to send media from another message without re-uploading it.
    pub fn copy_media(mut self, media: &Media) -> Self {
        self.media = media.to_raw_input_media();
        self
    }

    /// Include the uploaded file as a document file in the album.
    ///
    /// You can use this to send any type of media as a simple document file.
    ///
    /// The text will be the caption of the file, which may be empty for no caption.
    pub fn file(mut self, file: Uploaded) -> Self {
        let mime_type = self.get_file_mime(&file);
        let file_name = file.name().to_string();
        self.media = Some(
            (tl::types::InputMediaUploadedDocument {
                nosound_video: false,
                force_file: true,
                spoiler: false,
                file: file.raw,
                thumb: None,
                mime_type,
                attributes: vec![(tl::types::DocumentAttributeFilename { file_name }).into()],
                stickers: None,
                ttl_seconds: self.media_ttl,
                video_cover: None,
                video_timestamp: None,
            })
            .into(),
        );
        self
    }

    /// Change the media's Time To Live (TTL).
    ///
    /// For example, this enables you to send a `photo` that can only be viewed for a certain
    /// amount of seconds before it expires.
    ///
    /// Not all media supports this feature.
    ///
    /// This method should be called before setting any media, else it won't have any effect.
    pub fn media_ttl(mut self, seconds: i32) -> Self {
        self.media_ttl = if seconds < 0 { None } else { Some(seconds) };
        self
    }

    /// Change the media's mime type.
    ///
    /// This method will override the mime type that would otherwise be automatically inferred
    /// from the extension of the used file
    ///
    /// If no mime type is set and it cannot be inferred, the mime type will be
    /// "application/octet-stream".
    ///
    /// This method should be called before setting any media, else it won't have any effect.
    pub fn mime_type(mut self, mime_type: &str) -> Self {
        self.mime_type = Some(mime_type.to_string());
        self
    }

    /// Return the mime type string for the given file.
    fn get_file_mime(&self, file: &Uploaded) -> String {
        if let Some(mime) = self.mime_type.as_ref() {
            mime.clone()
        } else if let Some(mime) = mime_guess::from_path(file.name()).first() {
            mime.essence_str().to_string()
        } else {
            "application/octet-stream".to_string()
        }
    }
}
