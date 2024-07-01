use clap::Parser;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "IDAGIO Downloader")]
pub struct Args {
    #[clap(short, long, help="1 = AAC 160 / 192, 2 = MP3 320 / AAC 320, 3 = 16/44 FLAC.")]
    pub format: Option<u8>,

    #[clap(short, long, help="Output path.")]
    pub out_path: Option<PathBuf>,

    #[clap(short, long, help="Keep covers in album folder.")]
    pub keep_covers: bool,

    #[clap(short, long, help="Write covers to tracks.")]
    pub write_covers: bool,

    #[clap(short, long, num_args = 1.., required = true)]
    pub urls: Vec<String>,
}

#[derive(Deserialize)]
pub struct Config {
    pub email: String,
    #[serde(skip_deserializing)]
    pub ffmpeg_path: PathBuf,
    pub format: u8,
    pub keep_covers: bool,
    pub out_path: PathBuf,
    pub password: String,
    #[serde(skip_deserializing)]
    pub urls: Vec<String>,
    pub use_ffmpeg_env_var: bool,
    pub write_covers: bool,
}

pub struct ParsedAlbumMeta {
    pub album_title: String,
    pub album_artist: String,
    pub artist: String,
    pub copyright: String,
    pub cover_data: Vec<u8>,
    pub title: String,
    pub track_num: u16,
    pub track_total: u16,
    pub upc: String,
    pub year: u16,
}