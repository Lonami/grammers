use grammers_tl_types::{enums::ChannelParticipantsFilter, types};

pub struct Filters;

impl Filters {
    pub fn channel_participants_search(q: String) -> ChannelParticipantsFilter {
        ChannelParticipantsFilter::ChannelParticipantsSearch(types::ChannelParticipantsSearch { q })
    }
}
