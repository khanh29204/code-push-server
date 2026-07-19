use regex::Regex;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use tokio::fs;
use zip::ZipArchive;

pub fn parse_version(version_no: &str) -> String {
    let re_3 = Regex::new(r"^([0-9]{1,3})\.([0-9]{1,5})\.([0-9]{1,10})$").unwrap();
    let re_2 = Regex::new(r"^([0-9]{1,3})\.([0-9]{1,5})$").unwrap();

    if let Some(caps) = re_3.captures(version_no) {
        format!("{}{:0>5}{:0>10}", &caps[1], &caps[2], &caps[3])
    } else if let Some(caps) = re_2.captures(version_no) {
        format!("{}{:0>5}{:0>10}", &caps[1], &caps[2], "0")
    } else {
        "0".to_string()
    }
}

pub fn validator_version(version_no: &str) -> (bool, String, String) {
    let mut flag = false;
    let mut min = "0".to_string();
    let mut max = "9999999999999999999".to_string();

    if version_no == "*" {
        flag = true;
    } else {
        let re_3 = Regex::new(r"^([0-9]{1,3})\.([0-9]{1,5})\.([0-9]{1,10})$").unwrap();
        let re_2 = Regex::new(r"^([0-9]{1,3})\.([0-9]{1,5})(\.\*)?$").unwrap();
        let re_tilde = Regex::new(r"^~([0-9]{1,3})\.([0-9]{1,5})\.([0-9]{1,10})$").unwrap();
        let re_caret = Regex::new(r"^\^([0-9]{1,3})\.([0-9]{1,5})\.([0-9]{1,10})$").unwrap();
        let re_range = Regex::new(r"^([0-9]{1,3})\.([0-9]{1,5})\.([0-9]{1,10})\s?-\s?([0-9]{1,3})\.([0-9]{1,5})\.([0-9]{1,10})$").unwrap();
        let re_ge_lt = Regex::new(r"^>=([0-9]{1,3})\.([0-9]{1,5})\.([0-9]{1,10})\s?<([0-9]{1,3})\.([0-9]{1,5})\.([0-9]{1,10})$").unwrap();

        if let Some(caps) = re_3.captures(version_no) {
            flag = true;
            min = format!("{}{:0>5}{:0>10}", &caps[1], &caps[2], &caps[3]);
            max = format!("{}{:0>5}{:0>10}", &caps[1], &caps[2], caps[3].parse::<u64>().unwrap() + 1);
        } else if let Some(caps) = re_2.captures(version_no) {
            flag = true;
            min = format!("{}{:0>5}{:0>10}", &caps[1], &caps[2], "0");
            max = format!("{}{:0>5}{:0>10}", &caps[1], caps[2].parse::<u64>().unwrap() + 1, "0");
        } else if let Some(caps) = re_tilde.captures(version_no) {
            flag = true;
            min = format!("{}{:0>5}{:0>10}", &caps[1], &caps[2], &caps[3]);
            max = format!("{}{:0>5}{:0>10}", &caps[1], caps[2].parse::<u64>().unwrap() + 1, "0");
        } else if let Some(caps) = re_caret.captures(version_no) {
            flag = true;
            min = format!("{}{:0>5}{:0>10}", &caps[1], &caps[2], &caps[3]);
            max = format!("{}{:0>5}{:0>10}", caps[1].parse::<u64>().unwrap() + 1, "0", "0");
        } else if let Some(caps) = re_range.captures(version_no) {
            flag = true;
            min = format!("{}{:0>5}{:0>10}", &caps[1], &caps[2], &caps[3]);
            max = format!("{}{:0>5}{:0>10}", &caps[4], &caps[5], caps[6].parse::<u64>().unwrap() + 1);
        } else if let Some(caps) = re_ge_lt.captures(version_no) {
            flag = true;
            min = format!("{}{:0>5}{:0>10}", &caps[1], &caps[2], &caps[3]);
            max = format!("{}{:0>5}{:0>10}", &caps[4], &caps[5], &caps[6]);
        }
    }

    (flag, min, max)
}

pub async fn create_file_from_request<P: AsRef<Path>>(url: &str, file_path: P) -> Result<(), Box<dyn std::error::Error>> {
    let path = file_path.as_ref();
    if path.exists() {
        return Ok(());
    }

    let response = reqwest::get(url).await?;
    if !response.status().is_success() {
        return Err(format!("unexpected response {}", response.status()).into());
    }

    let bytes = response.bytes().await?;
    fs::write(path, bytes).await?;
    Ok(())
}

pub async fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    fs::create_dir_all(&dst).await?;
    let mut entries = fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let ty = entry.file_type().await?;
        if ty.is_dir() {
            Box::pin(copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))).await?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name())).await?;
        }
    }
    Ok(())
}

pub async fn delete_folder<P: AsRef<Path>>(folder_path: P) -> std::io::Result<()> {
    if folder_path.as_ref().exists() {
        fs::remove_dir_all(folder_path).await?;
    }
    Ok(())
}

pub async fn create_empty_folder<P: AsRef<Path>>(folder_path: P) -> std::io::Result<()> {
    delete_folder(&folder_path).await?;
    fs::create_dir_all(&folder_path).await?;
    Ok(())
}

pub async fn unzip_file<P: AsRef<Path>, Q: AsRef<Path>>(zip_file: P, output_path: Q) -> Result<String, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(zip_file.as_ref())?;
    let mut archive = ZipArchive::new(file)?;
    
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => output_path.as_ref().join(path),
            None => continue,
        };

        if (*file.name()).ends_with('/') {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    std::fs::create_dir_all(p)?;
                }
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }

    Ok(output_path.as_ref().to_string_lossy().into_owned())
}

#[derive(Debug)]
pub struct DiffResult {
    pub diff: Vec<String>,
    pub collection1_only: Vec<String>,
    pub collection2_only: Vec<String>,
}

pub fn get_blob_download_url(blob_url: &str) -> String {
    let sub_dir = if blob_url.len() >= 2 {
        blob_url[0..2].to_lowercase()
    } else {
        "00".to_string()
    };
    format!("/download/{}/{}", sub_dir, blob_url)
}

pub fn diff_collections(collection1: &BTreeMap<String, String>, collection2: &BTreeMap<String, String>) -> DiffResult {
    let mut diff = Vec::new();
    let mut collection1_only = Vec::new();
    let mut collection2_keys: HashSet<&String> = collection2.keys().collect();

    for (key, val1) in collection1.iter() {
        if !collection2_keys.contains(key) {
            collection1_only.push(key.clone());
        } else {
            collection2_keys.remove(key);
            let val2 = collection2.get(key).unwrap();
            if val1 != val2 {
                diff.push(key.clone());
            }
        }
    }

    DiffResult {
        diff,
        collection1_only,
        collection2_only: collection2_keys.into_iter().cloned().collect(),
    }
}
