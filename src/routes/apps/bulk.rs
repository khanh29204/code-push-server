use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use axum::{
    Json,
    extract::{Path as AxumPath, State},
};
use serde::Serialize;
use serde_json::Value;

use crate::core::{app_error::AppError, db::AppState};
use crate::routes::middleware::AuthUser;

const MAX_LABELS: usize = 500;

#[derive(Serialize)]
struct DeleteResult {
    label: String,
    status: &'static str,
    #[serde(rename = "deletedCount")]
    deleted_count: usize,
    #[serde(rename = "skippedCount")]
    skipped_count: usize,
    #[serde(rename = "missingCount")]
    missing_count: usize,
    errors: Vec<String>,
}

#[derive(Serialize)]
pub struct DeleteResponse {
    results: Vec<DeleteResult>,
    #[serde(rename = "deletedCount")]
    deleted_count: usize,
    #[serde(rename = "freedBytes")]
    freed_bytes: u64,
}

#[derive(Clone)]
pub struct PackageFile {
    label: String,
    key: String,
    current: bool,
}

struct FileState {
    path: PathBuf,
    size: u64,
}

pub async fn patch_bulk(
    AuthUser(user): AuthUser,
    AxumPath((app_name, deployment_name)): AxumPath<(String, String)>,
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, AppError> {
    let labels = validate_labels(payload.get("labels"))?;
    let info = payload
        .get("packageInfo")
        .and_then(Value::as_object)
        .ok_or_else(|| AppError::new("packageInfo must be a non-empty object"))?;
    if info.is_empty()
        || info
            .keys()
            .any(|k| !matches!(k.as_str(), "isDisabled" | "isMandatory" | "rollout"))
    {
        return Err(AppError::new(
            "packageInfo contains unsupported or no fields",
        ));
    }
    let disabled = optional_bool(info.get("isDisabled"), "isDisabled")?;
    let mandatory = optional_bool(info.get("isMandatory"), "isMandatory")?;
    let rollout = optional_rollout(info.get("rollout"))?;

    let (app, deployment) = resolve_scope(&state, user.id, &app_name, &deployment_name).await?;
    let mut tx = state.db.pool.begin().await?;
    reserve_write(&mut tx, deployment.id).await?;
    recheck_scope(
        &mut tx,
        user.id,
        app.id,
        deployment.id,
        &app_name,
        &deployment_name,
    )
    .await?;

    for label in &labels {
        let packages = sqlx::query_as::<_, crate::models::packages::Packages>(
            "SELECT * FROM packages WHERE deployment_id = ? AND label = ?",
        )
        .bind(deployment.id)
        .bind(label)
        .fetch_all(&mut *tx)
        .await?;
        if packages.is_empty() {
            return Err(AppError::NotFound(format!(
                "Package label {label} not found in deployment"
            )));
        }
        if disabled == Some(false) {
            if state.config.storage_type != "local" {
                return Err(AppError::new("Enabling a release requires local storage"));
            }
            for package in &packages {
                for key in [&package.blob_url, &package.manifest_blob_url] {
                    if !file_is_available(&state.config.local_storage_dir, key).await? {
                        return Err(AppError::new(&format!(
                            "Cannot enable label {label}: required local file {key} is missing or unsafe"
                        )));
                    }
                }
                let diff_keys = sqlx::query_scalar::<_, String>(
                    "SELECT diff_blob_url FROM packages_diff WHERE package_id = ?",
                )
                .bind(package.id)
                .fetch_all(&mut *tx)
                .await?;
                for key in diff_keys.into_iter().filter(|key| !key.is_empty()) {
                    if !file_is_available(&state.config.local_storage_dir, &key).await? {
                        return Err(AppError::new(&format!(
                            "Cannot enable label {label}: required local file {key} is missing or unsafe"
                        )));
                    }
                }
            }
        }
    }

    for label in &labels {
        let mut sets = Vec::new();
        if disabled.is_some() {
            sets.push("is_disabled = ?");
        }
        if mandatory.is_some() {
            sets.push("is_mandatory = ?");
        }
        if rollout.is_some() {
            sets.push("rollout = ?");
        }
        let sql = format!(
            "UPDATE packages SET {}, updated_at = CURRENT_TIMESTAMP WHERE deployment_id = ? AND label = ?",
            sets.join(", ")
        );
        let mut query = sqlx::query(&sql);
        if let Some(value) = disabled {
            query = query.bind(i64::from(value));
        }
        if let Some(value) = mandatory {
            query = query.bind(i64::from(value));
        }
        if let Some(value) = rollout {
            query = query.bind(value);
        }
        query
            .bind(deployment.id)
            .bind(label)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    if state.config.update_check_cache {
        crate::services::client_manager::ClientManager::clear_update_check_cache(
            &state.db.redis,
            &deployment.deployment_key,
        )
        .await;
    }
    Ok(Json(serde_json::json!({"labels": labels})))
}

pub async fn delete_bulk(
    AuthUser(user): AuthUser,
    AxumPath((app_name, deployment_name)): AxumPath<(String, String)>,
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<Json<DeleteResponse>, AppError> {
    let labels = validate_labels(payload.get("labels"))?;
    let (app, deployment) = resolve_scope(&state, user.id, &app_name, &deployment_name).await?;
    if state.config.storage_type != "local" {
        return Err(AppError::new(
            "Bulk release cleanup requires local storage; no files were removed",
        ));
    }

    let mut tx = state.db.pool.begin().await?;
    reserve_write(&mut tx, deployment.id).await?;
    recheck_scope(
        &mut tx,
        user.id,
        app.id,
        deployment.id,
        &app_name,
        &deployment_name,
    )
    .await?;
    let mut label_exists = Vec::new();
    for label in &labels {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM packages WHERE deployment_id = ? AND label = ?)",
        )
        .bind(deployment.id)
        .bind(label)
        .fetch_one(&mut *tx)
        .await?;
        label_exists.push(exists);
    }
    for (label, exists) in labels.iter().zip(label_exists) {
        if !exists {
            return Err(AppError::NotFound(format!(
                "Package label {label} not found in deployment; cleanup aborted before removing files"
            )));
        }
    }
    let mut results: Vec<DeleteResult> = labels
        .iter()
        .map(|label| DeleteResult {
            label: label.clone(),
            status: "cleaned",
            deleted_count: 0,
            skipped_count: 0,
            missing_count: 0,
            errors: Vec::new(),
        })
        .collect();
    let mut selected_ids = HashSet::new();
    let mut files = Vec::new();
    let mut package_hashes = Vec::new();
    let mut labels_by_package = HashMap::new();

    for label in &labels {
        let packages = sqlx::query_as::<_, crate::models::packages::Packages>(
            "SELECT * FROM packages WHERE deployment_id = ? AND label = ?",
        )
        .bind(deployment.id)
        .bind(label)
        .fetch_all(&mut *tx)
        .await?;
        for package in packages {
            selected_ids.insert(package.id);
            package_hashes.push(package.package_hash.clone());
            labels_by_package.insert(package.id, label.clone());
            let current: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM deployments_versions WHERE current_package_id = ?",
            )
            .bind(package.id)
            .fetch_one(&mut *tx)
            .await?;
            let is_current = current > 0 && package.is_disabled == 0;
            for key in [&package.blob_url, &package.manifest_blob_url] {
                if !key.is_empty() {
                    files.push(PackageFile {
                        label: label.clone(),
                        key: key.clone(),
                        current: is_current,
                    });
                }
            }
        }
    }

    let ids = selected_ids.iter().copied().collect::<Vec<_>>();
    let mut diffs = Vec::new();
    if !ids.is_empty() {
        let placeholders = vec!["?"; ids.len()].join(",");
        let query = format!(
            "SELECT package_id, diff_blob_url FROM packages_diff WHERE package_id IN ({placeholders})"
        );
        let mut q = sqlx::query_as::<_, (i64, String)>(&query);
        for id in &ids {
            q = q.bind(id);
        }
        diffs = q.fetch_all(&mut *tx).await?;
    }
    for (package_id, key) in diffs {
        if !key.is_empty() {
            let label = labels_by_package
                .get(&package_id)
                .cloned()
                .unwrap_or_default();
            let current = sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM deployments_versions WHERE current_package_id = ?",
            )
            .bind(package_id)
            .fetch_one(&mut *tx)
            .await?;
            let disabled: i64 = sqlx::query_scalar("SELECT is_disabled FROM packages WHERE id = ?")
                .bind(package_id)
                .fetch_one(&mut *tx)
                .await?;
            files.push(PackageFile {
                label,
                key,
                current: current > 0 && disabled == 0,
            });
        }
    }

    let selected_placeholders = if ids.is_empty() {
        String::from("NULL")
    } else {
        vec!["?"; ids.len()].join(",")
    };
    let outside_packages = if !ids.is_empty() {
        let query = format!(
            "SELECT blob_url, manifest_blob_url FROM packages WHERE id NOT IN ({selected_placeholders})"
        );
        let mut q = sqlx::query_as::<_, (String, String)>(&query);
        for id in &ids {
            q = q.bind(id);
        }
        q.fetch_all(&mut *tx).await?
    } else {
        sqlx::query_as("SELECT blob_url, manifest_blob_url FROM packages")
            .fetch_all(&mut *tx)
            .await?
    };
    let mut protected_keys = HashSet::new();
    for (blob, manifest) in outside_packages {
        if !blob.is_empty() {
            protected_keys.insert(blob);
        }
        if !manifest.is_empty() {
            protected_keys.insert(manifest);
        }
    }
    let outside_diffs: Vec<String> = if ids.is_empty() {
        sqlx::query_scalar("SELECT diff_blob_url FROM packages_diff")
            .fetch_all(&mut *tx)
            .await?
    } else {
        let query = format!(
            "SELECT diff_blob_url FROM packages_diff WHERE package_id NOT IN ({selected_placeholders})"
        );
        let mut q = sqlx::query_scalar(&query);
        for id in &ids {
            q = q.bind(id);
        }
        q.fetch_all(&mut *tx).await?
    };
    protected_keys.extend(outside_diffs.into_iter().filter(|key| !key.is_empty()));
    if !package_hashes.is_empty() {
        let placeholders = vec!["?"; package_hashes.len()].join(",");
        let query = format!(
            "SELECT DISTINCT diff_against_package_hash FROM packages_diff WHERE package_id NOT IN ({selected_placeholders}) AND diff_against_package_hash IN ({placeholders})"
        );
        let mut q = sqlx::query_scalar::<_, String>(&query);
        for id in &ids {
            q = q.bind(id);
        }
        for hash in &package_hashes {
            q = q.bind(hash);
        }
        for hash in q.fetch_all(&mut *tx).await? {
            for (_, blob, manifest) in sqlx::query_as::<_, (i64, String, String)>(
                "SELECT id, blob_url, manifest_blob_url FROM packages WHERE package_hash = ?",
            )
            .bind(hash)
            .fetch_all(&mut *tx)
            .await?
            {
                if !blob.is_empty() {
                    protected_keys.insert(blob);
                }
                if !manifest.is_empty() {
                    protected_keys.insert(manifest);
                }
            }
        }
    }

    let mut unique = HashMap::<String, (Vec<usize>, bool)>::new();
    for file in &files {
        let index = labels
            .iter()
            .position(|label| label == &file.label)
            .unwrap_or(0);
        let entry = unique
            .entry(file.key.clone())
            .or_insert_with(|| (Vec::new(), false));
        if !entry.0.contains(&index) {
            entry.0.push(index);
        }
        entry.1 |= file.current;
    }

    let mut file_states = HashMap::new();
    let mut unsafe_keys = HashSet::new();
    for key in unique.keys() {
        match inspect_file(&state.config.local_storage_dir, key).await {
            Ok(Some(file)) => {
                file_states.insert(key.clone(), file);
            }
            Ok(None) => {}
            Err(error) => {
                unsafe_keys.insert(key.clone());
                for index in &unique[key].0 {
                    set_status(&mut results[*index], "error");
                    results[*index].errors.push(format!("{key}: {error}"));
                }
            }
        }
    }

    if !unsafe_keys.is_empty() {
        for (key, (owners, _)) in &unique {
            if unsafe_keys.contains(key) {
                continue;
            }
            for index in owners {
                set_status(&mut results[*index], "error");
                results[*index].errors.push(format!(
                    "{key}: cleanup batch aborted because another file reference is unsafe"
                ));
            }
        }
        tx.commit().await?;
        return Ok(Json(DeleteResponse {
            results,
            deleted_count: 0,
            freed_bytes: 0,
        }));
    }

    let mut deleted_count = 0;
    let mut freed_bytes = 0u64;
    for (key, (owners, current)) in unique {
        if protected_keys.contains(&key) || current {
            let why = if current {
                "file belongs to a current package; disable it before cleanup"
            } else {
                "file is referenced by a package or diff outside the selected releases"
            };
            for index in owners {
                set_status(&mut results[index], "skipped");
                results[index].skipped_count += 1;
                results[index].errors.push(format!("{key}: {why}"));
            }
            continue;
        }
        let Some(file) = file_states.get(&key) else {
            for index in owners {
                results[index].missing_count += 1;
                set_status(&mut results[index], "missing");
            }
            continue;
        };
        match tokio::fs::remove_file(&file.path).await {
            Ok(()) => {
                deleted_count += 1;
                freed_bytes += file.size;
                for index in owners {
                    results[index].deleted_count += 1;
                }
            }
            Err(error) => {
                for index in owners {
                    set_status(&mut results[index], "error");
                    results[index].errors.push(format!("{key}: {error}"));
                }
            }
        }
    }

    tx.commit().await?;
    Ok(Json(DeleteResponse {
        results,
        deleted_count,
        freed_bytes,
    }))
}

fn set_status(result: &mut DeleteResult, status: &'static str) {
    let priority = |status| match status {
        "error" => 3,
        "skipped" => 2,
        "missing" => 1,
        _ => 0,
    };
    if priority(status) > priority(result.status) {
        result.status = status;
    }
}

async fn resolve_scope(
    state: &AppState,
    uid: i64,
    app_name: &str,
    deployment_name: &str,
) -> Result<
    (
        crate::models::apps::App,
        crate::models::deployments::Deployment,
    ),
    AppError,
> {
    let app = sqlx::query_as::<_, crate::models::apps::App>(
        "SELECT a.* FROM apps a JOIN collaborators c ON c.appid = a.id WHERE a.name = ? AND a.uid = ? AND c.uid = ? AND c.roles = 'Owner'",
    ).bind(app_name).bind(uid).bind(uid).fetch_optional(&state.db.pool).await?
        .ok_or_else(|| AppError::NotFound(format!("App {app_name} not found")))?;
    let deployment = sqlx::query_as::<_, crate::models::deployments::Deployment>(
        "SELECT * FROM deployments WHERE appid = ? AND name = ?",
    )
    .bind(app.id)
    .bind(deployment_name)
    .fetch_optional(&state.db.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Deployment not found".into()))?;
    Ok((app, deployment))
}

async fn recheck_scope(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    uid: i64,
    app_id: i64,
    deployment_id: i64,
    app_name: &str,
    deployment_name: &str,
) -> Result<(), AppError> {
    let authorized: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM apps a JOIN collaborators c ON c.appid = a.id JOIN deployments d ON d.appid = a.id WHERE a.id = ? AND a.name = ? AND a.uid = ? AND c.uid = ? AND c.roles = 'Owner' AND d.id = ? AND d.name = ?)")
        .bind(app_id).bind(app_name).bind(uid).bind(uid).bind(deployment_id).bind(deployment_name)
        .fetch_one(&mut **tx).await?;
    if !authorized {
        return Err(AppError::NotFound(format!(
            "App {app_name} or deployment {deployment_name} not found"
        )));
    }
    Ok(())
}

async fn reserve_write(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    deployment_id: i64,
) -> Result<(), AppError> {
    sqlx::query("UPDATE deployments SET id = id WHERE id = ?")
        .bind(deployment_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

fn validate_labels(value: Option<&Value>) -> Result<Vec<String>, AppError> {
    let labels = value
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::new("labels must be an array"))?;
    if labels.is_empty() || labels.len() > MAX_LABELS {
        return Err(AppError::new(
            "labels must contain 1 to 500 distinct values",
        ));
    }
    let mut seen = HashSet::new();
    let mut result = Vec::with_capacity(labels.len());
    for value in labels {
        let label = value
            .as_str()
            .ok_or_else(|| AppError::new("labels must contain strings only"))?;
        if label.is_empty() || label.trim() != label || label.chars().any(char::is_control) {
            return Err(AppError::new(
                "labels must be non-empty and contain no surrounding whitespace or control characters",
            ));
        }
        if !seen.insert(label) {
            return Err(AppError::new("labels must be distinct"));
        }
        result.push(label.to_owned());
    }
    Ok(result)
}

fn optional_bool(value: Option<&Value>, field: &str) -> Result<Option<bool>, AppError> {
    match value {
        None => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(AppError::new(&format!("{field} must be a boolean"))),
    }
}

fn optional_rollout(value: Option<&Value>) -> Result<Option<i64>, AppError> {
    match value {
        None => Ok(None),
        Some(value) => {
            let rollout = value
                .as_i64()
                .ok_or_else(|| AppError::new("rollout must be an integer from 1 to 100"))?;
            if !(1..=100).contains(&rollout) {
                return Err(AppError::new("rollout must be an integer from 1 to 100"));
            }
            Ok(Some(rollout))
        }
    }
}

fn validate_key(key: &str) -> Result<(), AppError> {
    if key.is_empty()
        || key == "."
        || key.starts_with("..")
        || !key.is_ascii()
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(AppError::new(&format!("unsafe storage key: {key}")));
    }
    Ok(())
}

fn key_path(root: &str, key: &str) -> Result<PathBuf, AppError> {
    validate_key(key)?;
    let prefix = if key.len() >= 2 {
        key[..2].to_ascii_lowercase()
    } else {
        "00".into()
    };
    Ok(Path::new(root).join(prefix).join(key))
}

async fn inspect_file(root: &str, key: &str) -> Result<Option<FileState>, String> {
    let path = key_path(root, key).map_err(|e| e.to_string())?;
    let root = tokio::fs::canonicalize(root)
        .await
        .map_err(|e| format!("storage root unavailable: {e}"))?;
    let parent_meta =
        match tokio::fs::symlink_metadata(path.parent().unwrap_or(Path::new("."))).await {
            Ok(meta) => meta,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
    if parent_meta.file_type().is_symlink() || !parent_meta.is_dir() {
        return Err("storage prefix is not a real directory".into());
    }
    let parent = tokio::fs::canonicalize(path.parent().unwrap())
        .await
        .map_err(|e| e.to_string())?;
    if !parent.starts_with(&root) {
        return Err("storage prefix escapes the storage root".into());
    }
    let meta = match tokio::fs::symlink_metadata(&path).await {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err("target is not a regular non-symlink file".into());
    }
    let target = tokio::fs::canonicalize(&path)
        .await
        .map_err(|e| e.to_string())?;
    if target.parent() != Some(parent.as_path()) || !target.starts_with(&root) {
        return Err("target escapes the storage root".into());
    }
    Ok(Some(FileState {
        path,
        size: meta.len(),
    }))
}

async fn file_is_available(root: &str, key: &str) -> Result<bool, AppError> {
    match inspect_file(root, key).await {
        Ok(Some(_)) => Ok(true),
        Ok(None) => Ok(false),
        Err(error) => Err(AppError::new(&error)),
    }
}
