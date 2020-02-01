use crate::types;

use grammers_tl_types as tl;

pub struct Dialog {
    pub dialog: tl::types::Dialog,
    pub entity: types::Entity,
    pub last_message: Option<tl::enums::Message>,
}
