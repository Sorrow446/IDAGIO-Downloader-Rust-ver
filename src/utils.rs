use std::error::Error;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Error as IoError};
use std::path::PathBuf;
use std::env;

pub fn get_exe_path() -> Result<PathBuf, Box<dyn Error>> {
    let exe_path = env::current_exe()?;
    let parent_dir = exe_path.parent()
        .ok_or("failed to get path of executable")?;
    let exe_path_buf = PathBuf::from(parent_dir);
    Ok(exe_path_buf)
}

fn contains(lines: &[String], value: &str) -> bool {
    lines.iter().any(|s| s.to_lowercase() == value.to_lowercase())
}

fn read_text_file_lines(filename: &str) -> Result<Vec<String>, IoError> {
    let f = File::open(filename)?;
    let br = BufReader::new(f);

    let mut lines: Vec<String> = Vec::new();
    for result in br.lines() {
        match result {
            Ok(line) => {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    lines.push(trimmed.to_string());
                }
            } 
            Err(e) => {
                return Err(e);
            }
        }
    }
    Ok(lines)
}

fn remove_query_params(url: &str) -> String {
    if let Some(pos) = url.find('?') {
        url[..pos].to_string()
    } else {
        url.to_string()
    }
}

pub fn clean_url(url: &str) -> String {
    let trimmed = url.trim();
    let stripped = trimmed.strip_suffix('/').unwrap_or(&trimmed);
    let cleaned = remove_query_params(stripped);
    cleaned
}

pub fn process_urls(urls: &[String]) -> Result<Vec<String>, Box<dyn Error>> {
    let mut processed: Vec<String> = Vec::new();
    let mut text_paths: Vec<String> = Vec::new();

    for url in urls {
        if url.ends_with(".txt") {
            if contains(&text_paths, &url) {
                continue;
            }
            let text_lines = read_text_file_lines(&url)?;
            for text_line in text_lines {
                let cleaned_line = clean_url(&text_line);
                if !contains(&processed, &cleaned_line) {
                    processed.push(cleaned_line);
                }
            }
            text_paths.push(url.clone());
        } else {
            let cleaned_line = clean_url(&url);
            if !contains(&processed, &cleaned_line) {
                processed.push(cleaned_line);
            }
        }
    }

    Ok(processed)
}

pub fn file_exists(file_path: &PathBuf) -> Result<bool, IoError> {
    match fs::metadata(file_path) {
        Ok(meta) => Ok(meta.is_file()),
        Err(err) => {
            if err.kind() == io::ErrorKind::NotFound {
                Ok(false)
            } else {
                Err(err)
            }
        }
    }
}

pub fn append_to_path(path: &PathBuf, to_append: &str) -> PathBuf {
    let path_str = path.to_string_lossy();
    let new_path_str = format!("{}{}", path_str, to_append);
    PathBuf::from(new_path_str)
}

pub fn set_path_ext(path: &PathBuf, ext: &str) -> PathBuf {
    let mut new_path = path.clone();
    new_path.set_extension(ext);
    new_path
}