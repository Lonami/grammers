use grammers_tl_types::{self as tl, enums::StarGift};

#[derive(Debug)]
pub enum Gift {
    SavedGift(tl::types::SavedStarGift),
    RegularGift(tl::enums::StarGift),
}

impl Gift {
    pub fn gift(&self) -> &StarGift {
        match self {
            Gift::SavedGift(gift) => &gift.gift,
            Gift::RegularGift(gift) => gift,
        }
    }

    pub fn id(&self) -> i64 {
        self.gift().id()
    }

    pub fn is_upgradable(&self) -> bool {
        match self {
            Gift::SavedGift(gift) => gift.can_upgrade,
            Gift::RegularGift(gift) => match gift {
                StarGift::Gift(star_gift) => star_gift.upgrade_stars.is_some(),
                StarGift::Unique(_) => true,
            },
        }
    }

    pub fn is_premium_only(&self) -> bool {
        let gift = self.gift();

        match &gift {
            grammers_tl_types::enums::StarGift::Gift(gift) => gift.require_premium,
            grammers_tl_types::enums::StarGift::Unique(gift) => gift.require_premium,
        }
    }
}

impl From<tl::enums::StarGift> for Gift {
    fn from(value: tl::enums::StarGift) -> Self {
        Self::RegularGift(value)
    }
}

impl From<tl::enums::payments::UniqueStarGift> for Gift {
    fn from(value: tl::enums::payments::UniqueStarGift) -> Self {
        let tl::enums::payments::UniqueStarGift::Gift(gift) = value;

        Self::RegularGift(gift.gift)
    }
}

impl From<tl::types::SavedStarGift> for Gift {
    fn from(value: tl::types::SavedStarGift) -> Self {
        Self::SavedGift(value)
    }
}
