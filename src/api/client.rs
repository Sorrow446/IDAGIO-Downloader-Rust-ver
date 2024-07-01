use crate::api::structs::*;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, USER_AGENT, AUTHORIZATION, RANGE, REFERER};
use reqwest::Error as ReqwestErr;
use reqwest::blocking::{Client, Response as ReqwestResp};
use std::collections::HashMap;
use serde_json::{self, Error as SerdeErr};
use std::error::Error;
use reqwest::Url;
use scraper::{Html, Selector};

const BASE_URL: &str = "https://api.idagio.com/";
const CLIENT_ID: &str = "com.idagio.app.android";
const IDAGIO_USER_AGENT: &str = "Android 3.8.8 (Build 3080800) [release]";
const CLIENT_SECRET: &str = "adbisIGrocsUckWyodUj2knedpyepubGurlyeawosShyufJishleseanreBlogIbCefHodCigNafweegyeebraftEdnooshDeavolirdoppEcIassyet9CirIrnofmaj";

pub struct IDAGIOClient {
    c: Client,
    pub user_info: UserInfo,
}

impl IDAGIOClient {
	pub fn new() -> Result<IDAGIOClient, ReqwestErr> {
		let mut headers = HeaderMap::new();
		headers.insert(USER_AGENT, HeaderValue::from_static(IDAGIO_USER_AGENT));

		let c = Client::builder()
			.default_headers(headers)
			.build()?;

		let user_info = UserInfo {
			access_token: String::new(),
			allow_concert_playback: false,
			plan_display_name: String::new(),
			premium: false,
		};

		let idagio_client = IDAGIOClient {
			c: c,
			user_info: user_info,
		};

		Ok(idagio_client)
	}

	pub fn auth(&mut self, email: &str, pwd: &str) -> Result<(), ReqwestErr> {
		let mut data: HashMap<&str, &str> = HashMap::new();
		data.insert("client_id", CLIENT_ID);
		data.insert("client_secret", CLIENT_SECRET);
		data.insert("username", email);
		data.insert("password", pwd);
		data.insert("grant_type", "password");

		let resp = self.c.post(BASE_URL.to_owned() + "v2.1/oauth")
			.header(CONTENT_TYPE, "application/x-www-form-urlencoded")
			.form(&data)
			.send()?;
		resp.error_for_status_ref()?;

		let auth: AuthResp = resp.json()?;
		let user_info = UserInfo {
			access_token: auth.access_token,
			allow_concert_playback: auth.user.features.gch.allow_concert_playback,
			plan_display_name: auth.user.plan_display_name.unwrap_or("<no subscription>".to_string()),
			premium: auth.user.premium,
		};
		self.user_info = user_info;
		Ok(())
	}

	pub fn get_album_meta(&mut self, album_slug: &str) -> Result<AlbumMetaResult, ReqwestErr> {
		let url = format!("{}v2.0/albums/{}", BASE_URL, album_slug);
		let resp = self.c.get(url)
			.header(AUTHORIZATION, format!("Bearer  {}", self.user_info.access_token))
			.header(CONTENT_TYPE, "application/json; charset=UTF-8")
			.send()?;
		resp.error_for_status_ref()?;
		let album_meta: AlbumMeta = resp.json()?;
		Ok(album_meta.result)
	}

	fn serialise_track_ids(&mut self, ids: Vec<String>) -> Result<String, SerdeErr> {
		let ids_struct = IDs { ids };
		let serialised = serde_json::to_string(&ids_struct)?;
		Ok(serialised)
	}

	pub fn get_stream_meta(&mut self, ids: Vec<String>, fmt: u8) -> Result<Vec<StreamMetaResult>, Box<dyn Error>> {
		let mut params: HashMap<&str, &str> = HashMap::new();
		let fmt_str = fmt.to_string();

		params.insert("client_type", "android-3");
		params.insert("client_version", "3.8.8");
		params.insert("device_id", "757a7c4dca4121ec");
		params.insert("quality", fmt_str.as_str());

		let url_no_params = BASE_URL.to_owned() + "v2.0/streams/bulk";
		let url = Url::parse_with_params(&url_no_params, &params)?;
		let serialised_track_ids = self.serialise_track_ids(ids)?;

		let resp = self.c.post(url)
			.header(AUTHORIZATION, format!("Bearer  {}", self.user_info.access_token))
			.header(CONTENT_TYPE, "application/json; charset=UTF-8")
			.body(serialised_track_ids)
			.send()?;
		resp.error_for_status_ref()?;

		let stream_meta: StreamMeta = resp.json()?;
		Ok(stream_meta.results)
	}

	pub fn get_file_resp(&mut self, url: &str) -> Result<ReqwestResp, ReqwestErr> {
		let resp = self.c.get(url)
			.header(RANGE, "bytes=0-")
			.send()?;
		resp.error_for_status_ref()?;
		Ok(resp)
	}

	pub fn get_cover_resp(&mut self, url: &str) -> Result<ReqwestResp, ReqwestErr> {
		let resp = self.c.get(url)
			.send()?;
		resp.error_for_status_ref()?;
		Ok(resp)
	}

	// https://api.idagio.com/livestream-event.v2/claudio-abbado-and-the-lucerne-festival-orchestra-wagner-and-mahler
	pub fn get_video_meta(&mut self, slug: &str) -> Result<VideoMetaResult, ReqwestErr> {
		let url = format!("{}livestream-event.v2/{}", BASE_URL, slug);
		let resp = self.c.get(url)
			.header(AUTHORIZATION, format!("Bearer  {}", self.user_info.access_token))
			.header(CONTENT_TYPE, "application/json; charset=UTF-8")
			.send()?;
		resp.error_for_status_ref()?;
		let meta: VideoMeta = resp.json()?;
		Ok(meta.result)
	}

	fn get_vimeo_player_html(&mut self, url: &str) -> Result<String, ReqwestErr> {
		let resp = self.c.get(url)
			.header(REFERER, "https://app.idagio.com/")
			.send()?;
		resp.error_for_status_ref()?;
		let html = resp.text()?;
		Ok(html)
	}

	pub fn get_vimeo_meta(&mut self, video_id: &str) -> Result<VimeoMeta, Box<dyn Error>> {
		let url = format!("https://player.vimeo.com/video/{}", video_id);
		let html = self.get_vimeo_player_html(&url)?;

		let s = Selector::parse("script")?;
		let document = Html::parse_document(&html);

		for e in document.select(&s) {
			let mut text = e.inner_html();
			if !text.starts_with("window.playerConfig") {
				continue;
			}
			text.drain(0..22);

			let meta: VimeoMeta = serde_json::from_str(&text)?;
			return Ok(meta);
		}

		Err("couldn't find vimeo meta json in vimeo html".into())
	}

	pub fn get_video_master(&mut self, url: &str) -> Result<VideoMaster, ReqwestErr> {
		let resp = self.c.get(url)
			.header(CONTENT_TYPE, "application/json; charset=UTF-8")
			.send()?;
		resp.error_for_status_ref()?;
		let meta: VideoMaster = resp.json()?;
		Ok(meta)
	}
}