use bcrypt::{hash, verify, DEFAULT_COST, BcryptError};
use md5;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncReadExt;
use walkdir::WalkDir;

pub const ANDROID: i64 = 1;
pub const IOS: i64 = 2;

pub fn md5_hash(str: &str) -> String {
    let digest = md5::compute(str);
    format!("{:x}", digest)
}

pub fn password_hash_sync(password: &str) -> Result<String, BcryptError> {
    hash(password, DEFAULT_COST)
}

pub fn password_verify_sync(password: &str, hash_str: &str) -> Result<bool, BcryptError> {
    verify(password, hash_str)
}

pub fn rand_token(num: usize) -> String {
    let rng = thread_rng();
    rng.sample_iter(&Alphanumeric)
        .take(num)
        .map(char::from)
        .collect()
}

pub fn parse_token(token: &str) -> (String, String) {
    let len = token.len();
    if len < 28 {
        return (token.to_string(), token.to_string());
    }
    let identical = token[len.saturating_sub(9)..].to_string();
    let token_part = token[..28].to_string();
    (identical, token_part)
}

pub async fn file_sha256<P: AsRef<Path>>(file: P) -> Result<String, std::io::Error> {
    let mut f = fs::File::open(file).await?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];
    loop {
        let n = f.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn string_sha256_sync(contents: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(contents);
    hex::encode(hasher.finalize())
}

fn is_hash_ignored(relative_path: &str) -> bool {
    if relative_path.is_empty() {
        return true;
    }
    let ignore_macosx = "__MACOSX/";
    let ignore_ds_store = ".DS_Store";

    relative_path.starts_with(ignore_macosx)
        || relative_path == ignore_ds_store
        || relative_path.ends_with(ignore_ds_store)
}

fn is_package_hash_ignored(relative_path: &str) -> bool {
    if relative_path.is_empty() {
        return true;
    }
    let ignore_codepush_metadata = ".codepushrelease";
    relative_path == ignore_codepush_metadata
        || relative_path.ends_with(ignore_codepush_metadata)
        || is_hash_ignored(relative_path)
}

pub fn package_hash_sync(json_data: &BTreeMap<String, String>) -> String {
    let mut manifest_data: Vec<String> = json_data
        .iter()
        .filter(|(k, _)| !is_package_hash_ignored(k))
        .map(|(k, v)| format!("{}:{}", k, v))
        .collect();
        
    manifest_data.sort();
    
    let mut manifest_string = serde_json::to_string(&manifest_data).unwrap_or_default();
    manifest_string = manifest_string.replace("\\/", "/");
    
    string_sha256_sync(&manifest_string)
}

pub fn upload_package_type<P: AsRef<Path>>(directory_path: P) -> Result<i64, String> {
    let mut package_type = 0;
    
    for entry in WalkDir::new(directory_path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let path_str = entry.path().to_string_lossy();
            if path_str.contains("android.bundle") {
                package_type = ANDROID;
                break;
            }
            if path_str.contains("main.jsbundle") {
                package_type = IOS;
                break;
            }
        }
    }
    
    if package_type == 0 {
        return Err("empty files or unknown package type".to_string());
    }
    Ok(package_type)
}

pub async fn calc_all_file_sha256<P: AsRef<Path>>(directory_path: P) -> Result<BTreeMap<String, String>, std::io::Error> {
    let dir = directory_path.as_ref();
    let mut results = BTreeMap::new();
    
    let mut files_to_hash = Vec::new();
    
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let relative = entry.path().strip_prefix(dir).unwrap_or(entry.path());
            let relative_str = relative.to_string_lossy().replace("\\", "/");
            
            if !is_hash_ignored(&relative_str) {
                files_to_hash.push((entry.path().to_path_buf(), relative_str));
            }
        }
    }
    
    if files_to_hash.is_empty() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "empty files"));
    }
    
    for (file_path, relative_str) in files_to_hash {
        let hash = file_sha256(file_path).await?;
        results.insert(relative_str, hash);
    }
    
    Ok(results)
}
