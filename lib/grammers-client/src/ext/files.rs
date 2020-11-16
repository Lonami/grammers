// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;

pub trait InputFileExt {
    /// Get the file's name.
    fn name(&self) -> &str;
}

pub trait MessageMediaExt {
    fn to_input_file(&self) -> Option<tl::enums::InputFileLocation>;
}

impl InputFileExt for tl::enums::InputFile {
    fn name(&self) -> &str {
        match self {
            tl::enums::InputFile::File(f) => &f.name,
            tl::enums::InputFile::Big(f) => &f.name,
        }
    }
}

impl MessageMediaExt for tl::enums::MessageMedia {
    fn to_input_file(&self) -> Option<tl::enums::InputFileLocation> {
        use tl::enums::MessageMedia::*;

        match self {
            Photo(tl::types::MessageMediaPhoto {
                photo: Some(tl::enums::Photo::Photo(photo)),
                ..
            }) => Some(
                tl::types::InputPhotoFileLocation {
                    id: photo.id,
                    access_hash: photo.access_hash,
                    file_reference: photo.file_reference.clone(),
                    thumb_size: String::new(),
                }
                .into(),
            ),
            Document(tl::types::MessageMediaDocument {
                document: Some(tl::enums::Document::Document(doc)),
                ..
            }) => Some(
                tl::types::InputDocumentFileLocation {
                    id: doc.id,
                    access_hash: doc.access_hash,
                    file_reference: doc.file_reference.clone(),
                    thumb_size: String::new(),
                }
                .into(),
            ),
            _ => None,
        }
    }
}
