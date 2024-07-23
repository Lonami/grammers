// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;

#[derive(Clone, Debug, PartialEq)]
pub enum PhotoSize {
    Empty(SizeEmpty),
    Size(Size),
    Cached(CachedSize),
    Stripped(StrippedSize),
    Progressive(ProgressiveSize),
    Path(PathSize),
}

impl PhotoSize {
    pub(crate) fn make_from(size: &tl::enums::PhotoSize, photo: &tl::types::Photo) -> Self {
        match size {
            tl::enums::PhotoSize::Empty(size) => PhotoSize::Empty(SizeEmpty {
                photo_type: size.r#type.clone(),
            }),
            tl::enums::PhotoSize::Size(size) => PhotoSize::Size(Size {
                photo_type: size.r#type.clone(),
                width: size.w,
                height: size.h,
                size: size.size,
                id: photo.id,
                access_hash: photo.access_hash,
                file_reference: photo.file_reference.clone(),
                from_document: false,
            }),
            tl::enums::PhotoSize::PhotoCachedSize(size) => PhotoSize::Cached(CachedSize {
                photo_type: size.r#type.clone(),
                width: size.w,
                height: size.h,
                bytes: size.bytes.clone(),
            }),
            tl::enums::PhotoSize::PhotoStrippedSize(size) => PhotoSize::Stripped(StrippedSize {
                photo_type: size.r#type.clone(),
                bytes: size.bytes.clone(),
            }),
            tl::enums::PhotoSize::Progressive(size) => PhotoSize::Progressive(ProgressiveSize {
                photo_type: size.r#type.clone(),
                width: size.w,
                height: size.h,
                sizes: size.sizes.clone(),
                id: photo.id,
                access_hash: photo.access_hash,
                file_reference: photo.file_reference.clone(),
                from_document: false,
            }),
            tl::enums::PhotoSize::PhotoPathSize(size) => PhotoSize::Path(PathSize {
                photo_type: size.r#type.clone(),
                bytes: size.bytes.clone(),
            }),
        }
    }

    pub(crate) fn make_from_document(
        size: &tl::enums::PhotoSize,
        document: &tl::types::Document,
    ) -> Self {
        match size {
            tl::enums::PhotoSize::Empty(size) => PhotoSize::Empty(SizeEmpty {
                photo_type: size.r#type.clone(),
            }),
            tl::enums::PhotoSize::Size(size) => PhotoSize::Size(Size {
                photo_type: size.r#type.clone(),
                width: size.w,
                height: size.h,
                size: size.size,
                id: document.id,
                access_hash: document.access_hash,
                file_reference: document.file_reference.clone(),
                from_document: true,
            }),
            tl::enums::PhotoSize::PhotoCachedSize(size) => PhotoSize::Cached(CachedSize {
                photo_type: size.r#type.clone(),
                width: size.w,
                height: size.h,
                bytes: size.bytes.clone(),
            }),
            tl::enums::PhotoSize::PhotoStrippedSize(size) => PhotoSize::Stripped(StrippedSize {
                photo_type: size.r#type.clone(),
                bytes: size.bytes.clone(),
            }),
            tl::enums::PhotoSize::Progressive(size) => PhotoSize::Progressive(ProgressiveSize {
                photo_type: size.r#type.clone(),
                width: size.w,
                height: size.h,
                sizes: size.sizes.clone(),
                id: document.id,
                access_hash: document.access_hash,
                file_reference: document.file_reference.clone(),
                from_document: true,
            }),
            tl::enums::PhotoSize::PhotoPathSize(size) => PhotoSize::Path(PathSize {
                photo_type: size.r#type.clone(),
                bytes: size.bytes.clone(),
            }),
        }
    }

    /// Size of the photo thumb
    pub fn size(&self) -> usize {
        match self {
            PhotoSize::Empty(_) => 0,
            PhotoSize::Size(size) => size.size as usize,
            PhotoSize::Cached(size) => size.bytes.len(),
            PhotoSize::Stripped(size) => {
                let bytes = &size.bytes;
                if bytes.len() < 3 || bytes[0] != 0x01 {
                    return 0;
                }
                size.bytes.len() + 622
            }
            PhotoSize::Progressive(size) => {
                size.sizes.iter().max().unwrap_or(&0).to_owned() as usize
            }
            PhotoSize::Path(size) => size.bytes.len(),
        }
    }

    pub fn to_raw_input_location(&self) -> Option<tl::enums::InputFileLocation> {
        match self {
            PhotoSize::Size(size) => size.to_raw_input_location(),
            PhotoSize::Empty(_) => None,
            PhotoSize::Cached(_) => None,
            PhotoSize::Stripped(_) => None,
            PhotoSize::Progressive(size) => size.to_raw_input_location(),
            PhotoSize::Path(_) => None,
        }
    }

    pub(crate) fn data(&self) -> Vec<u8> {
        match self {
            PhotoSize::Empty(_) => vec![],
            PhotoSize::Size(_) => vec![],
            PhotoSize::Cached(size) => size.data(),
            PhotoSize::Stripped(size) => size.data(),
            PhotoSize::Progressive(_) => vec![],
            PhotoSize::Path(size) => size.data(),
        }
    }

    pub fn photo_type(&self) -> String {
        match self {
            PhotoSize::Empty(size) => size.photo_type.clone(),
            PhotoSize::Size(size) => size.photo_type.clone(),
            PhotoSize::Cached(size) => size.photo_type.clone(),
            PhotoSize::Stripped(size) => size.photo_type.clone(),
            PhotoSize::Progressive(size) => size.photo_type.clone(),
            PhotoSize::Path(size) => size.photo_type.clone(),
        }
    }
}

/// Empty thumbnail. Image with this thumbnail is unavailable.
#[derive(Clone, Debug, PartialEq)]
pub struct SizeEmpty {
    photo_type: String,
}

/// Image description. An additional request to Telegram should be perfomed to download the image
#[derive(Clone, Debug, PartialEq)]
pub struct Size {
    photo_type: String,
    pub width: i32,
    pub height: i32,
    pub size: i32,

    id: i64,
    access_hash: i64,
    file_reference: Vec<u8>,

    from_document: bool,
}

impl Size {
    pub fn to_raw_input_location(&self) -> Option<tl::enums::InputFileLocation> {
        let input_location = if !self.from_document {
            tl::enums::InputFileLocation::from(tl::types::InputPhotoFileLocation {
                id: self.id,
                access_hash: self.access_hash,
                file_reference: self.file_reference.clone(),
                thumb_size: self.photo_type.clone(),
            })
        } else {
            tl::enums::InputFileLocation::from(tl::types::InputDocumentFileLocation {
                id: self.id,
                access_hash: self.access_hash,
                file_reference: self.file_reference.clone(),
                thumb_size: self.photo_type.clone(),
            })
        };

        Some(input_location)
    }
}

/// Description of an image and its content.
#[derive(Clone, Debug, PartialEq)]
pub struct CachedSize {
    photo_type: String,

    pub width: i32,
    pub height: i32,
    pub bytes: Vec<u8>,
}

impl CachedSize {
    fn data(&self) -> Vec<u8> {
        self.bytes.clone()
    }
}

/// A low-resolution compressed JPG payload
#[derive(Clone, Debug, PartialEq)]
pub struct StrippedSize {
    photo_type: String,

    pub bytes: Vec<u8>,
}

impl StrippedSize {
    fn data(&self) -> Vec<u8> {
        // Based on https://core.tlgr.org/api/files#stripped-thumbnails
        let bytes = &self.bytes;
        if bytes.len() < 3 || bytes[0] != 0x01 {
            return vec![];
        }

        let header = vec![
            0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, 0x4a, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00,
            0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0xff, 0xdb, 0x00, 0x43, 0x00, 0x28, 0x1c, 0x1e,
            0x23, 0x1e, 0x19, 0x28, 0x23, 0x21, 0x23, 0x2d, 0x2b, 0x28, 0x30, 0x3c, 0x64, 0x41,
            0x3c, 0x37, 0x37, 0x3c, 0x7b, 0x58, 0x5d, 0x49, 0x64, 0x91, 0x80, 0x99, 0x96, 0x8f,
            0x80, 0x8c, 0x8a, 0xa0, 0xb4, 0xe6, 0xc3, 0xa0, 0xaa, 0xda, 0xad, 0x8a, 0x8c, 0xc8,
            0xff, 0xcb, 0xda, 0xee, 0xf5, 0xff, 0xff, 0xff, 0x9b, 0xc1, 0xff, 0xff, 0xff, 0xfa,
            0xff, 0xe6, 0xfd, 0xff, 0xf8, 0xff, 0xdb, 0x00, 0x43, 0x01, 0x2b, 0x2d, 0x2d, 0x3c,
            0x35, 0x3c, 0x76, 0x41, 0x41, 0x76, 0xf8, 0xa5, 0x8c, 0xa5, 0xf8, 0xf8, 0xf8, 0xf8,
            0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8,
            0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8,
            0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8, 0xf8,
            0xf8, 0xf8, 0xf8, 0xf8, 0xff, 0xc0, 0x00, 0x11, 0x08, 0x00, 0x00, 0x00, 0x00, 0x03,
            0x01, 0x22, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11, 0x01, 0xff, 0xc4, 0x00, 0x1f, 0x00,
            0x00, 0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b,
            0xff, 0xc4, 0x00, 0xb5, 0x10, 0x00, 0x02, 0x01, 0x03, 0x03, 0x02, 0x04, 0x03, 0x05,
            0x05, 0x04, 0x04, 0x00, 0x00, 0x01, 0x7d, 0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05,
            0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32, 0x81,
            0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0, 0x24, 0x33, 0x62,
            0x72, 0x82, 0x09, 0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28, 0x29,
            0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
            0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66,
            0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84,
            0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99,
            0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5,
            0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca,
            0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2, 0xe3, 0xe4, 0xe5,
            0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9,
            0xfa, 0xff, 0xc4, 0x00, 0x1f, 0x01, 0x00, 0x03, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
            0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05,
            0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0xff, 0xc4, 0x00, 0xb5, 0x11, 0x00, 0x02, 0x01,
            0x02, 0x04, 0x04, 0x03, 0x04, 0x07, 0x05, 0x04, 0x04, 0x00, 0x01, 0x02, 0x77, 0x00,
            0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61,
            0x71, 0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23,
            0x33, 0x52, 0xf0, 0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1,
            0x17, 0x18, 0x19, 0x1a, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39,
            0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57,
            0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75,
            0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a,
            0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6,
            0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2,
            0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7,
            0xd8, 0xd9, 0xda, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3,
            0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa, 0xff, 0xda, 0x00, 0x0c, 0x03, 0x01, 0x00,
            0x02, 0x11, 0x03, 0x11, 0x00, 0x3f, 0x00,
        ];
        let mut footer = vec![0xff, 0xd9];
        let mut real = header;
        real[164] = bytes[1];
        real[166] = bytes[2];

        let mut bytes_clone = bytes.clone()[3..].to_vec();
        real.append(&mut bytes_clone);
        real.append(&mut footer);

        real
    }
}

/// Progressively encoded photosize. An additional request to Telegram should be perfomed to download the image
#[derive(Clone, Debug, PartialEq)]
pub struct ProgressiveSize {
    photo_type: String,

    pub width: i32,
    pub height: i32,
    pub sizes: Vec<i32>,

    id: i64,
    access_hash: i64,
    file_reference: Vec<u8>,

    from_document: bool,
}

impl ProgressiveSize {
    pub fn to_raw_input_location(&self) -> Option<tl::enums::InputFileLocation> {
        let input_location = if !self.from_document {
            tl::enums::InputFileLocation::from(tl::types::InputPhotoFileLocation {
                id: self.id,
                access_hash: self.access_hash,
                file_reference: self.file_reference.clone(),
                thumb_size: self.photo_type.clone(),
            })
        } else {
            tl::enums::InputFileLocation::from(tl::types::InputDocumentFileLocation {
                id: self.id,
                access_hash: self.access_hash,
                file_reference: self.file_reference.clone(),
                thumb_size: self.photo_type.clone(),
            })
        };

        Some(input_location)
    }
}

/// Messages with animated stickers can have a compressed svg (< 300 bytes) to show the outline
/// of the sticker before fetching the actual lottie animation.
#[derive(Clone, Debug, PartialEq)]
pub struct PathSize {
    photo_type: String,

    pub bytes: Vec<u8>,
}

impl PathSize {
    fn data(&self) -> Vec<u8> {
        // Based on https://core.tlgr.org/api/files#vector-thumbnails
        let lookup = "AACAAAAHAAALMAAAQASTAVAAAZaacaaaahaaalmaaaqastava.az0123456789-,";
        let mut path = String::from("M");
        for num in &self.bytes {
            let num = *num;
            if num >= 128 + 64 {
                path.push(lookup.chars().nth((num - 128 - 64) as usize).unwrap());
            } else {
                if num >= 128 {
                    path.push(',');
                } else if num >= 64 {
                    path.push('-');
                }
                path.push((num & 63) as char);
            }
        }
        path.push('z');
        let res = format!(
            r###"<?xml version="1.0" encoding="utf-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"
viewBox="0 0 512 512" xml:space="preserve">
<path d="{path}"/>
</svg>"###
        );

        res.as_bytes().to_vec()
    }
}

pub trait VecExt {
    /// Helper method to get the largest photo thumb
    fn largest(&self) -> Option<&PhotoSize>;
}

impl VecExt for Vec<PhotoSize> {
    fn largest(&self) -> Option<&PhotoSize> {
        self.iter().max_by_key(|x| x.size())
    }
}
