# IDAGIO-Downloader-Rust-ver
IDAGIO downloader written in Rust.
![](https://i.imgur.com/mHBAub0.png)
[Pre-compiled binaries](https://github.com/Sorrow446/IDAGIO-Downloader-Rust-ver/releases/)

# Setup
Input credentials into config file.
Configure any other options if needed.
|Option|Info|
| --- | --- |
|email|Email address.
|password|Password.
|format|Track download quality. 1 = AAC 160 / 192, 2 = MP3 320 / AAC 320, 3 = 16/44 FLAC.
|out_path|Where to download to. Path will be made if it doesn't already exist.
|keep_covers|Keep covers in album folder.
|write_covers|Write covers to tracks.
|use_ffmpeg_env_var|true = call FFmpeg from environment variable, false = call from script dir.

**FFmpeg is needed for TS -> MP4 losslessly for concerts, see below.**

# FFmpeg Setup
[Windows (gpl)](https://github.com/BtbN/FFmpeg-Builds/releases)    
Linux: `sudo apt install ffmpeg`    
Termux `pkg install ffmpeg`    
Place in IDAGIO DL's script/binary directory if using FFmpeg binary.

If you don't have root in Linux, you can have Nugs DL look for the binary in the same dir by setting the `use_ffmpeg_env_var` option to false.

## Supported Media
|Type|URL example|
| --- | --- |
|Album|`https://app.idagio.com/albums/1628a93d-cfdc-4850-bda1-3b14209f729b`
|Concert|`https://app.idagio.com/live/event/francesco-cavalli-ercole-amante` Best format is automatically chosen for now.

# Usage
Args take priority over the config file.

Download two albums:   
`idagio_dl.exe -u https://app.idagio.com/albums/1628a93d-cfdc-4850-bda1-3b14209f729b https://app.idagio.com/albums/3e801bcb-30cf-48de-9bc5-c8d2e7f53513`

Download a single concert and from a text file containing links:   
`idagio_dl.exe -u https://app.idagio.com/live/event/francesco-cavalli-ercole-amante G:\1.txt`

```
Usage: idagio_dl.exe [OPTIONS] --urls <URLS>...

Options:
  -f, --format <FORMAT>      1 = AAC 160 / 192, 2 = MP3 320 / AAC 320, 3 = 16/44 FLAC.
  -o, --out-path <OUT_PATH>  Output path.
  -k, --keep-covers          Keep covers in album folder.
  -w, --write-covers         Write covers to tracks.
  -u, --urls <URLS>...
  -h, --help                 Print help
```

# Disclaimer
- I will not be responsible for how you use IDAGIO Downloader.    
- IDAGIO brand and name is the registered trademark of its respective owner.    
- IDAGIO Downloader has no partnership, sponsorship or endorsement with IDAGIO.
