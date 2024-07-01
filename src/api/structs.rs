use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Gch {
    pub allow_concert_playback: bool,
}

#[derive(Deserialize)]
pub struct Features {
    pub gch: Gch,
}

#[derive(Deserialize)]
pub struct User {
    pub features: Features,
    pub premium: bool,
    pub plan_display_name: Option<String>,
}

#[derive(Deserialize)]
pub struct AuthResp {
    pub access_token: String,
    pub user: User,
}

#[derive(Clone, Deserialize)]
pub struct Person {
    pub name: String,
}
#[derive(Clone, Deserialize)]
pub struct Author {
    pub persons: Vec<Person>,
}

#[derive(Deserialize)]
pub struct Work {
    pub title: String,
    pub authors: Vec<Author>,
}

#[derive(Deserialize)]
pub struct Workpart {
    pub work: Work,

}

#[derive(Deserialize)]
pub struct Piece {
    pub title: String,
    pub workpart: Workpart,
}

#[derive(Deserialize)]
pub struct Track {
    pub id: i64,
    pub piece: Piece,
    pub position: i64,
}

#[derive(Deserialize)]
pub struct Participant {
    pub name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumMetaResult {
    pub copyright: String,
    pub copyright_year: u16,
    pub image_url: String,
    pub participants: Vec<Participant>,
    pub title: String,
    pub track_ids: Vec<String>,
    pub tracks: Vec<Track>,
    pub upc: String,
}

#[derive(Deserialize)]
pub struct AlbumMeta {
    pub result: AlbumMetaResult,
}

pub struct UserInfo {
    pub access_token: String,
    // pub allow_concert_playback: bool,
    pub plan_display_name: String,
   //  pub premium: bool,
    pub premium: bool,
    pub allow_concert_playback: bool,
}

#[derive(Serialize)]
pub struct IDs {
    pub ids: Vec<String>,
}

#[derive(Deserialize)]
pub struct StreamMeta {
    pub results: Vec<StreamMetaResult>,
}

#[derive(Deserialize)]
pub struct StreamMetaResult {
    pub id: i64,
    pub url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Video {
    pub name: String,
    pub source: String,
    pub video_id: String,
}
#[derive(Deserialize)]
pub struct VideoMetaResult {
    pub video: Video,
}

#[derive(Deserialize)]
pub struct VideoMeta {
    pub result: VideoMetaResult,
}

#[derive(Deserialize)]
pub struct Request {
    pub files: Files,
}

#[derive(Deserialize)]
pub struct Files {
    pub dash: Dash,
}

#[derive(Deserialize)]
pub struct Dash {
    pub cdns: Cdns
}

#[derive(Deserialize)]
pub struct Cdns {
    pub akfire_interconnect_quic: AkfireInterconnectQuic,
}

#[derive(Deserialize)]
pub struct AkfireInterconnectQuic {
    pub avc_url: String,
    // pub url: String,
}

#[derive(Deserialize)]
pub struct VimeoMeta {
    pub request: Request,
}

#[derive(Deserialize)]
pub struct VideoTrack {
    pub avg_bitrate: u32,
    pub base_url: String,
    pub framerate: f32,
    pub id: String,
    pub height: u16,
    pub width: u16,
}

#[derive(Deserialize)]
pub struct AudioTrack {
    pub avg_bitrate: u32,
    pub base_url: String,
    pub codecs: String,
    pub id: String,
    // pub sample_rate: u32
}

#[derive(Deserialize)]
pub struct VideoMaster {
    pub audio: Vec<AudioTrack>,
    pub video: Vec<VideoTrack>,
}