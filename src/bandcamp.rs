use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct LoginResponse {
    // pub ok: bool,
    pub access_token: String,
    // pub token_type: String,
    // pub expires_in: u32,
    // pub refresh_token: String,
}

#[derive(Debug, Deserialize)]
pub struct CollectionResponse {
    pub items: Vec<Item>,
}

#[derive(Debug, Deserialize)]
pub struct Item {
    pub tralbum_type: String,
    pub tralbum_id: u32,
    pub sale_item_type: String,
    pub title: String,
    pub tracks: Vec<Track>,
    pub band_info: BandInfo,
}

#[derive(Debug, Deserialize)]
pub struct Track {
    pub track_id: u32,
    pub title: String,
    pub hq_audio_url: String,
    pub track_number: u8,
}

#[derive(Debug, Deserialize)]
pub struct BandInfo {
    pub band_id: u32,
    pub name: String,
    pub bio: String,
    pub page_url: String,
}
