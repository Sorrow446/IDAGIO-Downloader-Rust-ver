use crate::api::structs::*;

use std::collections::HashMap;
use std::error::Error;

use reqwest::blocking::{Client, Response as ReqwestResp};
use reqwest::Error as ReqwestErr;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, USER_AGENT, AUTHORIZATION, RANGE, REFERER};
use reqwest::Url;
use scraper::{Html, Selector};
use serde_json::{self, Error as SerdeErr};

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
			c,
			user_info,
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
		let meta: AlbumMeta = resp.json()?;
		Ok(meta.result)
	}

	pub fn get_playlist_meta(&mut self, plist_slug: &str) -> Result<PlaylistMetaResult, ReqwestErr> {
		let url = format!("{}v2.0/playlists/{}", BASE_URL, plist_slug);
		let resp = self.c.get(url)
			.header(AUTHORIZATION, format!("Bearer  {}", self.user_info.access_token))
			.header(CONTENT_TYPE, "application/json; charset=UTF-8")
			.send()?;
		resp.error_for_status_ref()?;
		let meta: PlaylistMeta = resp.json()?;
		Ok(meta.result)
	}

	fn get_artist_meta(&mut self, artist_slug: &str) -> Result<ArtistMetaResult, ReqwestErr> {
		let url = format!("{}artists.v3/{}", BASE_URL, artist_slug);
		let resp = self.c.get(url)
			.header(AUTHORIZATION, format!("Bearer  {}", self.user_info.access_token))
			.header(CONTENT_TYPE, "application/json; charset=UTF-8")
			.send()?;
		resp.error_for_status_ref()?;
		let meta: ArtistMeta = resp.json()?;
		Ok(meta.result)
	}

	fn resolve_artist_id(&mut self, artist_slug: &str) -> Result<u64, ReqwestErr> {
		let artist_meta = self.get_artist_meta(artist_slug)?;
		Ok(artist_meta.id)
	}

	fn filter_artist_params(&mut self, base_url: &str, params: &mut HashMap<String, String>) -> Result<(), Box<dyn Error>> {
		let url = Url::parse(base_url)?;

		let mut allowed_keys = HashMap::new();
		allowed_keys.insert("composers", true);
		allowed_keys.insert("conductors", true);
		allowed_keys.insert("ensembles", true);
		allowed_keys.insert("instruments", true);
		allowed_keys.insert("soloists", true);

		for (k, v) in url.query_pairs() {
			if allowed_keys.contains_key(&k.as_ref()) {
				let mut key = k.to_string();
				key.pop();
				params.insert(key, v.into_owned());
			} else {
				println!("Dropped param: {}.", k);
			}
		}

		Ok(())
	}

	pub fn get_artist_albums_meta(&mut self, artist_slug: &str, params_opt: Option<String>) -> Result<Vec<ArtistAlbumsMetaResult>, Box<dyn Error>> {
		let artist_id = self.resolve_artist_id(artist_slug)?;
		let artist_id_string = artist_id.to_string();
		let mut all_meta: Vec<ArtistAlbumsMetaResult> = Vec::new();

		// Lifetime crap.
		let mut cursor_opt: Option<String> = None;

		let mut params: HashMap<String, String> = HashMap::new();
		let url_no_params = format!("{}v2.0/metadata/albums/filter", BASE_URL);

		if let Some(p) = params_opt {
			let url_with_params = format!("{}?{}", url_no_params, p.to_lowercase());
			self.filter_artist_params(&url_with_params, &mut params)?;
		}

		params.insert("artist".to_string(), artist_id_string.to_string());
		params.insert("sort".to_string(), "relevance".to_string());

		loop {

			if let Some(cursor) = cursor_opt.as_ref() {
				params.insert("cursor".to_string(), cursor.to_string());
			}

			let url = Url::parse_with_params(&url_no_params, &params)?;

			let resp = self.c.get(url)
				.header(AUTHORIZATION, format!("Bearer {}", self.user_info.access_token))
				.header(CONTENT_TYPE, "application/json; charset=UTF-8")
				.send()?;
			resp.error_for_status_ref()?;

			let meta: ArtistAlbumsMeta = resp.json()?;
			all_meta.extend(meta.results);

			if let Some(c) = meta.meta.cursor.next.clone() {
				if meta.meta.cursor.prev.is_none() {
					println!("Artist has more than 100 albums. Fetching the remaining metadata...")
				}
				cursor_opt = Some(c);
			} else {
				break;
			}

		}

		Ok(all_meta)
	}

	// pub fn get_personal_plists_meta(&mut self, id: &str) -> Result<PersonalPlaylistMetaResult, Box<dyn Error>> {
	// 	let url = format!("{}v1.0/personal-playlists/{}", BASE_URL, id);
	// 	let resp = self.c.get(url)
	// 		.header(AUTHORIZATION, format!("Bearer  {}", self.user_info.access_token))
	// 		.header(CONTENT_TYPE, "application/json; charset=UTF-8")
	// 		.send()?;
	// 	resp.error_for_status_ref()?;
	// 	let meta: PersonalPlaylistsMeta = resp.json()?;
	// 	Ok(meta.result)
	// }

	pub fn get_personal_plists_meta(&mut self, id: &str) -> Result<PersonalPlaylistMetaResult, Box<dyn Error>> {
		let url = format!("{}v1.0/personal-playlists/{}", BASE_URL, id);
		let resp = self.c.get(url)
			.header(AUTHORIZATION, format!("Bearer  {}", self.user_info.access_token))
			.header(CONTENT_TYPE, "application/json; charset=UTF-8")
			.send()?;
		resp.error_for_status_ref()?;
		let meta: PersonalPlaylistsMeta = resp.json()?;
		Ok(meta.result)
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

	pub fn get_file_resp(&mut self, url: &str, with_range: bool) -> Result<ReqwestResp, ReqwestErr> {
		let mut req = self.c.get(url);
		if with_range {
			req = req.header(RANGE, "bytes=0-")
		}
		let resp = req.send()?;
		resp.error_for_status_ref()?;
		Ok(resp)
	}

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