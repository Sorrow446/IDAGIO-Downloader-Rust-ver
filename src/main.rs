mod api;
mod structs;
mod utils;

use api::client::IDAGIOClient;
use api::structs::{AlbumMetaResult, Author, Track};
use clap::Parser;
use ctr::cipher::{KeyIvInit, StreamCipher};
use hex;
use indicatif::{ProgressBar, ProgressStyle};
use regex::{Regex, Error as RegexError};
use serde_json;
use sha2::{Sha256, Digest};
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write, Error as IoError, Cursor};
use structs::{Args, Config, ParsedAlbumMeta};
use std::path::PathBuf;
use metaflac::{Tag as FlacTag, Error as FlacError};
use reqwest::Error as ReqwestErr;
use reqwest::blocking::Response as ReqwestResp;
use metaflac::block::PictureType::CoverFront as FlacCoverFront;
use id3::{Error as Id3Error, Tag as Mp3Tag, TagLike, Version};
use id3::frame::{Picture as Mp3Image, PictureType as Mp3ImageType};
use mp4ameta::{Tag as Mp4Tag, Data as Mp4Data, Fourcc, Error as Mp4Error};
use crate::api::structs::AudioTrack;
use std::process::{Command, Output, Stdio};
use crate::utils::get_exe_path;

type Aes128Ctr128BE = ctr::Ctr128BE<aes::Aes128>;

const BUF_SIZE: usize = 1024 * 1024;

const REGEX_STRINGS: [&str; 2] = [
    r#"https://app.idagio.com/albums/([a-zA-Z\d-]+)"#,
    r#"https://app.idagio.com/live/event/([a-zA-Z\d-]+)"#,
 ];

const SAN_REGEX_STRING: &str = r#"[\/:*?"><|]"#;
const SECRET: &str = "prod-media-c-YaiJaoni7iebeed5";

#[derive(Clone)]
struct Quality {
    specs: &'static str,
    extension: &'static str,
    format: &'static u8,
}

static QUALITY_LIST: [(&str, Quality); 5] = [
    ("aes-128-ctr/aac-160-", Quality {specs: "160 Kbps AAC", extension: ".m4a", format: &2}),
    ("aes-128-ctr/aac-192-", Quality {specs: "192 Kbps AAC", extension: ".m4a", format: &2}),
    ("aes-128-ctr/aac-320-", Quality {specs: "320 Kbps AAC", extension: ".m4a", format: &2}),
    ("aes-128-ctr/flac-",    Quality {specs: "16-bit / 44.1 kHz FLAC", extension: ".flac", format: &3}),
    ("aes-128-ctr/mp3-320-", Quality {specs: "320 Kbps MP3", extension: ".mp3", format: &1}),
];

fn read_config(exe_path: &PathBuf) -> Result<Config, Box<dyn Error>> {
    let config_path = exe_path.join("config.json");
    let f = File::open(config_path)?;
    let config: Config = serde_json::from_reader(f)?;
    Ok(config)
}

fn resolve_format(fmt: u8) -> Option<u8> {
    match fmt {
        1 => Some(50),
        2 => Some(70),
        3 => Some(90),
        _ => None,
    }
}

fn parse_config() -> Result<Config, Box<dyn Error>> {
    let exe_path = get_exe_path()?;

    let mut config = read_config(&exe_path)?;
    let args = Args::parse();
    let proc_urls = utils::process_urls(&args.urls)?;

    if args.keep_covers {
        config.keep_covers = args.keep_covers;
    }

    if args.write_covers {
        config.write_covers = args.write_covers;
    }

    config.format = args.format.unwrap_or(config.format);
    config.out_path = args.out_path.unwrap_or(config.out_path);
    config.out_path.push("IDAGIO downloads");


    config.format = resolve_format(config.format)
        .ok_or("format must be between 1 and 3")?;

    if config.use_ffmpeg_env_var {
        config.ffmpeg_path = PathBuf::from("./ffmpeg");
    } else {
        let ffmpeg_path = exe_path.join("ffmpeg");
        config.ffmpeg_path = ffmpeg_path;
    }

    config.urls = proc_urls;
    Ok(config)
}

fn check_url(url: &str, regexes: &[Regex]) -> Result<(String, usize), RegexError> {
    for (idx, re) in regexes.iter().enumerate() {
        if let Some(capture) = re.captures(url) {
            if let Some(matched) = capture.get(1) {
                return Ok((matched.as_str().to_string(), idx));
            }
        }
    }

    Ok((String::new(), 0))
}

fn derive_key(mut key: Vec<u8>) -> Vec<u8> {
    key.extend_from_slice(SECRET.as_bytes());

    let hashed_key_base = Sha256::digest(key);
    let hex_key_base = hex::encode(&hashed_key_base[..8]);
    hex_key_base.into_bytes()
}

fn decrypt(incomp_path: &PathBuf, key: &[u8], iv: &[u8]) -> Result<(), IoError> {
    let dec_path = utils::set_path_ext(incomp_path, ".decrypted");

    {
        let in_file = File::open(incomp_path)?;
        let out_file = File::create(&dec_path)?;
        let mut br = BufReader::new(in_file);
        let mut bw = BufWriter::new(out_file);

        let mut buf = vec![0u8; BUF_SIZE];
        let mut cipher = Aes128Ctr128BE::new(key.into(), iv.into());

        loop {
            let bytes_read = br.read(&mut buf)?;
            if bytes_read == 0 {
                break;
            }

            cipher.apply_keystream(&mut buf[..bytes_read]);
            bw.write_all(&buf[..bytes_read])?;
        }
    }

    fs::remove_file(incomp_path)?;
    fs::rename(dec_path, incomp_path)?;

    Ok(())
}

fn parse_key_and_iv(key_and_iv: &str) -> Result<(Vec<u8>, Vec<u8>), Box<dyn Error>> {
    let split = key_and_iv.splitn(2, ' ');
    let split_strings: Vec<&str> = split.collect();
    if split_strings.len() != 2 {
        return Err("failed to parse key and iv".into());
    }

    let key = split_strings[0].as_bytes().to_vec();
    let iv = split_strings[1].as_bytes().to_vec();
    Ok((key, iv))
}

fn sanitise(filename: &str) -> Result<String, RegexError> {
    let re = Regex::new(SAN_REGEX_STRING)?;
    Ok(re.replace_all(filename, "_").to_string())    
}

fn download(resp: &mut ReqwestResp, out_path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let total_size = resp
        .content_length()
        .ok_or("no content length header")?;

    let f = File::create(out_path)?;
    let mut writer = BufWriter::new(f);
    let mut buf = vec![0u8; BUF_SIZE];

    let mut downloaded: usize = 0;
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::with_template("[{elapsed_precise}] [{bar:40.cyan/blue}] {percent}% at {binary_bytes_per_sec}, {bytes}/{total_bytes} (ETA: {eta})")?
        .progress_chars("#>-"));

    loop {
        let n = &resp.read(&mut buf)?;
        if n.to_owned() == 0 {
            break;
        }
        writer.write_all(&buf[..n.to_owned()])?;
        downloaded += n;
        pb.set_position(downloaded as u64);
    }

    pb.finish();
    Ok(())
}
fn download_track(c: &mut IDAGIOClient, url: &str, incomp_path: &PathBuf, out_path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let mut resp = c.get_file_resp(url)?;
    let key_and_iv_str = resp.headers().get("x-x")
        .map_or_else(
            || Ok("".to_string()),
            |value| value.to_str().map(|s| s.to_string()).map_err(|_| "failed to convert header value to string")
        )?;

    download(&mut resp, incomp_path)?;

    if !key_and_iv_str.is_empty() {
        let (key, iv) = parse_key_and_iv(&key_and_iv_str)?;
        println!("Decrypting...");
        let derived_key = derive_key(key);
        decrypt(incomp_path, &derived_key, &iv)?;
    }

    fs::rename(incomp_path, out_path)?;

    Ok(())
}

fn query_quality(stream_url: &str) -> Option<Quality> {
    for (key, quality) in QUALITY_LIST.iter() {
        if stream_url.contains(key) {
            return Some(quality.clone());
        }
    }
    None
}

// fn set_vorbis<T: ToString>(tag: &mut metaflac::Tag, key: &str, value: T) {
//     let val_str = value.to_string();
//
//     if ["TRACKNUMBER", "TRACKTOTAL"].contains(&key) {
//         let num = val_str.parse::<i16>();
//         if let Ok(parsed_num) = num {
//             if parsed_num < 1 {
//                 return;
//             }
//         } else {
//             return;
//         }
//     } else if val_str.is_empty() {
//         return;
//     }
//
//     tag.set_vorbis(key, vec!(val_str));
// }

fn set_vorbis(tag: &mut metaflac::Tag, key: &str, value: &str) {
    if !value.is_empty() {
        tag.set_vorbis(key, vec!(value));
    }
}

fn set_vorbis_num(tag: &mut metaflac::Tag, key: &str, n: u16) {
    if n > 0 {
        tag.set_vorbis(key, vec!(n.to_string()));
    }
}

fn write_mp3_tags(track_path: &PathBuf, meta: &ParsedAlbumMeta) -> Result<(), Id3Error> {
    let mut tag = Mp3Tag::new();

    tag.set_album(&meta.album_title);
    tag.set_album_artist(&meta.album_artist);
    tag.set_artist(&meta.artist);
    tag.set_title(&meta.title);
    tag.set_track(meta.track_num as u32);
    tag.set_total_tracks(meta.track_total as u32);
    tag.set_year(meta.year as i32);

    if !meta.cover_data.is_empty() {
        let pic = Mp3Image {
            mime_type: "image/jpeg".to_string(),
            picture_type: Mp3ImageType::CoverFront,
            description: String::new(),
            data: meta.cover_data.clone(),
        };
        tag.add_frame(pic);
    }

    tag.write_to_path(track_path, Version::Id3v24)?;
    Ok(())
}

fn write_mp4_tags(track_path: &PathBuf, meta: &ParsedAlbumMeta) -> Result<(), Mp4Error> {
    let mut tag = Mp4Tag::read_from_path(&track_path)?;

    tag.set_album(&meta.album_title);
    tag.set_album_artist(&meta.album_artist);
    tag.set_artist(&meta.artist);
    tag.set_title(&meta.title);
    tag.set_track(meta.track_num, meta.track_total);
    tag.set_year(meta.year.to_string());

    let covr = Fourcc(*b"covr");
    if !meta.cover_data.is_empty() {
        tag.add_data(covr, Mp4Data::Jpeg(meta.cover_data.clone()));
    }

    tag.write_to_path(&track_path)?;
    Ok(())
}

fn write_flac_tags(track_path: &PathBuf, meta: &ParsedAlbumMeta) -> Result<(), FlacError> {
    let mut tag = FlacTag::read_from_path(&track_path)?;

    set_vorbis(&mut tag, "ALBUM", &meta.album_title);
    set_vorbis(&mut tag, "ALBUMARTIST", &meta.album_artist);
    set_vorbis(&mut tag, "ARTIST", &meta.artist);
    set_vorbis(&mut tag, "COPYRIGHT", &meta.copyright);
    set_vorbis(&mut tag, "TRACK", &meta.title);
    set_vorbis(&mut tag, "UPC", &meta.upc);

    set_vorbis_num(&mut tag, "TRACKNUMBER", meta.track_num);
    set_vorbis_num(&mut tag, "TRACKTOTAL", meta.track_total);
    set_vorbis_num(&mut tag, "YEAR", meta.year);

    if !meta.cover_data.is_empty() {
        tag.add_picture("image/jpeg", FlacCoverFront, meta.cover_data.clone());
    }

    tag.save()?;
    Ok(())
}

fn write_tags(track_path: &PathBuf, fmt: &u8, meta: &ParsedAlbumMeta) -> Result<(), Box<dyn Error>> {
    match fmt {
        1 => write_mp3_tags(track_path, meta)?,
        2 => write_mp4_tags(track_path, meta)?,
        3 => write_flac_tags(track_path, meta)?,
        _ => {},
    }
    Ok(())
}

fn process_track(c: &mut IDAGIOClient, album_path: &PathBuf, meta: &ParsedAlbumMeta, url: &str) -> Result<(), Box<dyn Error>> {
    let quality = match query_quality(url) {
        Some(q) => q,
        None => {
            let err_str = format!("the api returned an unknown format: {}", url);
            return Err(err_str.into());
        }
    };

    println!("Track {} of {}: {} - {}", meta.track_num, meta.track_total, meta.title, quality.specs);

    let san_track_fname = format!("{:02}. {}", meta.track_num, sanitise(&meta.title)?);
    let track_path_no_ext = album_path.join(san_track_fname);
    let track_path = utils::append_to_path(&track_path_no_ext, quality.extension)?;
    let track_path_incomp = utils::append_to_path(&track_path_no_ext, ".incomplete")?;

    if utils::file_exists(&track_path)? {
        println!("Track already exists locally.");
        return Ok(());
    }

    download_track(c, url, &track_path_incomp, &track_path)?;
    write_tags(&track_path, quality.format, meta)?;

    Ok(())
}

fn parse_album_meta(meta: &AlbumMetaResult, track_total: u16) -> ParsedAlbumMeta {
    ParsedAlbumMeta {
        album_title: meta.title.clone(),
        album_artist: meta.participants[0].name.clone(),
        artist: String::new(),
        copyright: meta.copyright.clone(),
        cover_data: Vec::new(),
        title: String::new(),
        track_num: 0,
        track_total,
        upc: meta.upc.clone(),
        year: meta.copyright_year,
    }
}

fn parse_track_artists(authors: Vec<Author>) -> String {
    let mut artists = Vec::new();
    for author in authors {
        for person in author.persons {
            artists.push(person.name);
        }
    }
    artists.join(", ")
}
fn parse_track_meta(meta: &mut ParsedAlbumMeta, track_meta: &Track, track_num: u16) {
    let piece_title = &track_meta.piece.title;

    let mut title = track_meta.piece.workpart.work.title.clone();
    if title != *piece_title {
        title += &format!(" - {}", piece_title)
    }

    meta.artist =  parse_track_artists(track_meta.piece.workpart.work.authors.clone());
    meta.title = title;
    meta.track_num = track_num;
}

fn get_cover_data(c: &mut IDAGIOClient, url: &str) -> Result<Vec<u8>, Box<ReqwestErr>> {
    let resp = c.get_cover_resp(url)?;
    let body_bytes = resp.bytes()?;
    let body_vec: Vec<u8> = body_bytes.into_iter().collect();
    Ok(body_vec)
}

fn write_cover(cover_data: &[u8], album_path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let cover_path = album_path.join("folder.jpg");
    let mut f = File::create(cover_path)?;
    let mut cursor = Cursor::new(cover_data);
    io::copy(&mut cursor, &mut f)?;
    Ok(())
}
fn process_album(c: &mut IDAGIOClient, slug: &str, config: &Config) -> Result<(), Box<dyn Error>> {
    let mut meta = c.get_album_meta(slug)?;
    let track_total = meta.tracks.len() as u16;
    let mut parsed_meta = parse_album_meta(&meta, track_total);
    meta.tracks.sort_by_key(|t| t.position);

    let album_folder = format!("{} - {}", parsed_meta.album_artist, parsed_meta.album_title);
    println!("{}", album_folder);

    let san_album_folder = sanitise(&album_folder)?;
    let album_path = config.out_path.join(san_album_folder);
    fs::create_dir_all(&album_path)?;

    let stream_meta = c.get_stream_meta(meta.track_ids, config.format)?;

    let cover_data = get_cover_data(c, &meta.image_url)?;

    if config.keep_covers {
        write_cover(&cover_data, &album_path)?;
    }

    if config.write_covers {
        parsed_meta.cover_data = cover_data.clone();
    }

    for (mut idx, track) in meta.tracks.iter().enumerate() {
        idx += 1;
        if let Some(res) = stream_meta.iter().find(|res| res.id == track.id) {
            parse_track_meta(&mut parsed_meta, track, idx as u16);
            process_track(c, &album_path, &parsed_meta, &res.url)?;
        } else {
            println!("The API didn't return any stream metadata for this track.")
        }
    }

    Ok(())
}

fn make_base_url(url: &str) -> Result<String, Box<dyn Error>>{
    let idx = url.find("/sep/").ok_or("url separator not present")?;
    let base = format!("{}/parcel/", &url[..idx]);
    Ok(base)
}

fn get_aac_audio(audio: &[AudioTrack]) -> Option<&AudioTrack> {
    audio.iter().find(|&d|d.codecs == "mp4a.40.2")
}

fn mux_mp4(ffmpeg_path: &PathBuf, video_path: &PathBuf, audio_path: &PathBuf, out_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let output: Output = Command::new(ffmpeg_path)
        .arg("-i")
        .arg(video_path)
        .arg("-i")
        .arg(audio_path)
        .arg("-c")
        .arg("copy")
        .arg(out_path)
        .stderr(Stdio::piped())
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let err_msg = format!("bad exit code, output: {}", stderr);
        Err(err_msg)?
    }
    Ok(())
}

fn process_video(c: &mut IDAGIOClient, slug: &str, config: &Config) -> Result<(), Box<dyn Error>> {
    if !c.user_info.allow_concert_playback {
        return Err("plan doesn't allow concerts".into());
    }
    let meta = c.get_video_meta(slug)?;
    let name = meta.video.name;
    println!("{}", name);

    if meta.video.source != "vimeo" {
        return Err("unsupported video source, was expecting vimeo".into());
    }

    let vimeo_meta = c.get_vimeo_meta(&meta.video.video_id)?;
    let master_url = vimeo_meta.request.files.dash.cdns.akfire_interconnect_quic.avc_url;
    let base_url = make_base_url(&master_url)?;
    let mut master = c.get_video_master(&master_url)?;

    master.audio.sort_by_key(|a| -(a.avg_bitrate as i32));

    // master.video.sort_by_key(|v| -(v.height as i16));
    master.video.sort_by_key(|v| v.height);

    let video = &master.video[0];

    let san_video_name = sanitise(&name)?;
    let fname_string = format!("{} ({}p).mp4", san_video_name, video.height);
    let out_path = config.out_path.join(fname_string);
    if utils::file_exists(&out_path)? {
        println!("Concert already exists locally.");
        return Ok(());
    }

    let audio = get_aac_audio(&master.audio).ok_or("aac audio track not present")?;

    let video_url = format!("{}{}{}.mp4", base_url, video.base_url, video.id);
    let audio_url = format!("{}{}{}.mp4", base_url, audio.base_url, audio.id);

    let video_path = config.out_path.join("v.mp4");
    let audio_path = config.out_path.join("a.mp4");

    println!("Video: ~{} Kbps | {} FPS | {}p ({}x{2})", video.avg_bitrate/1000, video.framerate, video.height, video.width);
    let mut video_resp = c.get_file_resp(&video_url)?;
    download(&mut video_resp, &video_path)?;

    println!("Audio: AAC ~{} Kbps", audio.avg_bitrate/1000);
    let mut audio_resp = c.get_file_resp(&audio_url)?;
    download(&mut audio_resp, &audio_path)?;

    println!("Muxing...");
    mux_mp4(&config.ffmpeg_path, &video_path, &audio_path, &out_path)?;
    fs::remove_file(video_path)?;
    fs::remove_file(audio_path)?;

    Ok(())
}

fn compile_regexes() -> Result<Vec<Regex>, regex::Error> {
    REGEX_STRINGS.iter()
        .map(|&s| Regex::new(s))
        .collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let config = parse_config()
        .expect("failed to parse args/config");
    fs::create_dir_all(&config.out_path)?;
    
    let mut c = IDAGIOClient::new()?;
    c.auth(&config.email, &config.password)
        .expect("failed to auth");
    
    println!("Signed in successfully - {}\n", c.user_info.plan_display_name);

    if !c.user_info.premium {
        println!("No active subscription; audio quality limited.");
    }

    let regexes = compile_regexes()?;

    for url in &config.urls {
        let (slug, media_type) = check_url(&url, &regexes)?;
        if slug.is_empty() {
            println!("Invalid URL: {}", url);
            continue;
        }
        match media_type {
            0 => if let Err (e) = process_album(&mut c, &slug, &config) {
                println!("Album failed.\n{}", e);
            },
            1 => if let Err (e) = process_video(&mut c, &slug, &config) {
                println!("Video failed.\n{}", e);
            },
            _ => {},
        }
    }

    Ok(())
}
