use crate::core::app_error::AppError;
use crate::core::consts::*;
use crate::models::apps::App;
use crate::models::deployments::Deployment;
use crate::models::deployments_versions::DeploymentVersion;
use crate::models::packages::Packages;
use crate::models::packages_diff::PackagesDiff;
use crate::models::packages_metrics::PackagesMetrics;
use crate::services::datacenter_manager::{DataCenterManager, PackageInfo};
use crate::utils::common::{
    copy_dir_all, create_empty_folder, create_file_from_request, delete_folder, diff_collections,
    get_blob_download_url, unzip_file, validator_version,
};
use crate::utils::qetag::calc_qetag;
use crate::utils::security::{rand_token, upload_package_type};
use crate::utils::storage::StorageManager;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use zip::ZipWriter;
use zip::write::FileOptions;

pub struct PackageManager;

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct ReleaseParams {
    #[serde(rename = "appVersion")]
    pub app_version: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "isDisabled")]
    pub is_disabled: Option<bool>,
    pub rollout: Option<i64>,
    #[serde(rename = "isMandatory")]
    pub is_mandatory: Option<bool>,
    pub label: Option<String>,
}

pub struct CreatePackageParams<'a> {
    pub release_method: &'a str,
    pub release_uid: i64,
    pub is_mandatory: i64,
    pub is_disabled: i64,
    pub rollout: i64,
    pub size: i64,
    pub description: &'a str,
    pub original_label: &'a str,
    pub original_deployment: &'a str,
}

impl PackageManager {
    pub async fn get_metrics_by_package_id(
        pool: &SqlitePool,
        package_id: i64,
    ) -> Result<Option<PackagesMetrics>, AppError> {
        let metrics = sqlx::query_as::<_, PackagesMetrics>(
            "SELECT * FROM packages_metrics WHERE package_id = ?",
        )
        .bind(package_id)
        .fetch_optional(pool)
        .await?;
        Ok(metrics)
    }

    pub async fn find_package_info_by_deployment_id_and_label(
        pool: &SqlitePool,
        deployment_id: i64,
        label: &str,
    ) -> Result<Option<Packages>, AppError> {
        let package = sqlx::query_as::<_, Packages>(
            "SELECT * FROM packages WHERE deployment_id = ? AND label = ?",
        )
        .bind(deployment_id)
        .bind(label)
        .fetch_optional(pool)
        .await?;
        Ok(package)
    }

    pub async fn find_latest_package_info_by_deploy_version(
        pool: &SqlitePool,
        deployments_versions_id: i64,
    ) -> Result<Option<Packages>, AppError> {
        let dv = sqlx::query_as::<_, DeploymentVersion>(
            "SELECT * FROM deployments_versions WHERE id = ?",
        )
        .bind(deployments_versions_id)
        .fetch_optional(pool)
        .await?;

        if let Some(dv) = dv {
            if dv.current_package_id < 0 {
                return Err(AppError::General("not found last packages".into()));
            }
            let package = sqlx::query_as::<_, Packages>("SELECT * FROM packages WHERE id = ?")
                .bind(dv.current_package_id)
                .fetch_optional(pool)
                .await?;
            Ok(package)
        } else {
            Err(AppError::General("deployments_versions not found".into()))
        }
    }

    // `parseReqFile` is skipped; Axum handler will parse multipart forms.

    pub async fn create_deployments_version_if_not_exist(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        deployment_id: i64,
        app_version: &str,
        min_version: &str,
        max_version: &str,
    ) -> Result<DeploymentVersion, AppError> {
        let existing = sqlx::query_as::<_, DeploymentVersion>(
            "SELECT * FROM deployments_versions WHERE deployment_id = ? AND app_version = ? AND min_version = ? AND max_version = ?"
        )
        .bind(deployment_id)
        .bind(app_version)
        .bind(min_version)
        .bind(max_version)
        .fetch_optional(&mut **tx)
        .await?;

        if let Some(dv) = existing {
            return Ok(dv);
        }

        let inserted = sqlx::query_as::<_, DeploymentVersion>(
            "INSERT INTO deployments_versions (deployment_id, app_version, min_version, max_version, current_package_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, 0, datetime('now'), datetime('now')) RETURNING *"
        )
        .bind(deployment_id)
        .bind(app_version)
        .bind(min_version)
        .bind(max_version)
        .fetch_one(&mut **tx)
        .await?;

        Ok(inserted)
    }

    pub async fn is_match_package_hash(
        pool: &SqlitePool,
        package_id: i64,
        package_hash: &str,
    ) -> Result<bool, AppError> {
        if package_id < 0 {
            return Ok(false);
        }
        let package = sqlx::query_as::<_, Packages>("SELECT * FROM packages WHERE id = ?")
            .bind(package_id)
            .fetch_optional(pool)
            .await?;

        if let Some(p) = package
            && p.package_hash == package_hash
        {
            return Ok(true);
        }
        Ok(false)
    }

    pub async fn generate_deployments_label_id(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        deployment_id: i64,
    ) -> Result<i64, AppError> {
        let label_id = sqlx::query_scalar::<_, i64>(
            "UPDATE deployments SET label_id = label_id + 1 WHERE id = ? RETURNING label_id",
        )
        .bind(deployment_id)
        .fetch_one(&mut **tx)
        .await?;
        Ok(label_id)
    }

    pub async fn create_package(
        pool: &SqlitePool,
        deployment_id: i64,
        app_version: &str,
        package_hash: &str,
        manifest_hash: &str,
        blob_hash: &str,
        params: CreatePackageParams<'_>,
    ) -> Result<Packages, AppError> {
        let release_method = params.release_method;
        let release_uid = params.release_uid;
        let is_mandatory = params.is_mandatory;
        let rollout = params.rollout;
        let size = params.size;
        let description = params.description;
        let is_disabled = params.is_disabled;
        let original_label = params.original_label;
        let original_deployment = params.original_deployment;

        // Parse version
        let (is_valid, min_version, max_version) = validator_version(app_version);
        if !is_valid {
            return Err(AppError::General(format!(
                "targetBinaryVersion {} not support.",
                app_version
            )));
        }

        let mut tx = pool.begin().await?;

        let label_id = Self::generate_deployments_label_id(&mut tx, deployment_id).await?;

        let dv = Self::create_deployments_version_if_not_exist(
            &mut tx,
            deployment_id,
            app_version,
            &min_version,
            &max_version,
        )
        .await?;

        let label = format!("v{}", label_id);

        let package = sqlx::query_as::<_, Packages>(
            r#"INSERT INTO packages (
                deployment_version_id, deployment_id, description, package_hash, blob_url, size,
                manifest_blob_url, release_method, label, released_by, is_mandatory, is_disabled,
                rollout, original_label, original_deployment, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now')) RETURNING *"#
        )
        .bind(dv.id)
        .bind(deployment_id)
        .bind(description)
        .bind(package_hash)
        .bind(blob_hash)
        .bind(size)
        .bind(manifest_hash)
        .bind(release_method)
        .bind(label)
        .bind(release_uid)
        .bind(is_mandatory)
        .bind(is_disabled)
        .bind(rollout)
        .bind(original_label)
        .bind(original_deployment)
        .fetch_one(&mut *tx)
        .await?;

        // Update deployment version
        sqlx::query("UPDATE deployments_versions SET current_package_id = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(package.id)
            .bind(dv.id)
            .execute(&mut *tx)
            .await?;

        // Update deployment
        sqlx::query("UPDATE deployments SET last_deployment_version_id = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(dv.id)
            .bind(deployment_id)
            .execute(&mut *tx)
            .await?;

        // Create metrics
        sqlx::query("INSERT INTO packages_metrics (package_id, active, downloaded, failed, installed, created_at, updated_at) VALUES (?, 0, 0, 0, 0, datetime('now'), datetime('now'))")
            .bind(package.id)
            .execute(&mut *tx)
            .await?;

        // Create history
        sqlx::query("INSERT INTO deployments_history (deployment_id, package_id, created_at) VALUES (?, ?, datetime('now'))")
            .bind(deployment_id)
            .bind(package.id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok(package)
    }

    pub fn zip_diff_package(
        file_name: &Path,
        files: &[String],
        base_directory_path: &Path,
        hot_code_push_file: &Path,
    ) -> Result<PathBuf, AppError> {
        let file = std::fs::File::create(file_name)?;
        let mut zip = ZipWriter::new(file);
        let options =
            FileOptions::<'_, ()>::default().compression_method(zip::CompressionMethod::Deflated);

        for f in files {
            let p = base_directory_path.join(f);
            zip.start_file(f.replace("\\", "/"), options)?;
            let mut f_in = std::fs::File::open(p)?;
            std::io::copy(&mut f_in, &mut zip)?;
        }

        zip.start_file(DIFF_MANIFEST_FILE_NAME, options)?;
        let mut hotcodepush_in = std::fs::File::open(hot_code_push_file)?;
        std::io::copy(&mut hotcodepush_in, &mut zip)?;
        zip.finish()?;

        Ok(file_name.to_path_buf())
    }

    pub async fn release_package(
        config: &crate::config::AppConfig,
        pool: &SqlitePool,
        storage: &StorageManager,
        app_id: i64,
        deployment_id: i64,
        package_info: ReleaseParams,
        file_path: &Path,
        release_uid: i64,
    ) -> Result<Packages, AppError> {
        let app_version = package_info.app_version.clone().unwrap_or_default();
        let (is_valid, _, _) = validator_version(&app_version);
        if !is_valid {
            return Err(AppError::General(format!(
                "targetBinaryVersion {} not support.",
                app_version
            )));
        }

        let tmp_dir = std::path::Path::new(&config.data_dir);
        let directory_path_parent = tmp_dir.join(format!("codepush_{}", rand_token(32)));
        let directory_path = directory_path_parent.join("current");

        let blob_hash = calc_qetag(file_path).await?;
        create_empty_folder(&directory_path).await?;
        unzip_file(file_path, &directory_path).await?;

        let package_type = upload_package_type(&directory_path).unwrap_or(0);
        let app_info = sqlx::query_as::<_, App>("SELECT * FROM apps WHERE id = ?")
            .bind(app_id)
            .fetch_one(pool)
            .await?;

        if package_type > 0 && app_info.os > 0 && app_info.os != package_type {
            let _ = delete_folder(&directory_path_parent).await;
            return Err(AppError::General(
                "it must be publish it by ios type".into(),
            ));
        }

        let data_center =
            DataCenterManager::store_package(config, &directory_path.to_string_lossy(), false)
                .await?;
        let package_hash = data_center.package_hash.clone();
        let manifest_file_path = std::path::PathBuf::from(data_center.manifest_file_path);

        let dv = sqlx::query_as::<_, DeploymentVersion>(
            "SELECT * FROM deployments_versions WHERE deployment_id = ? AND app_version = ?",
        )
        .bind(deployment_id)
        .bind(&app_version)
        .fetch_optional(pool)
        .await?;

        if let Some(dv) = dv {
            let is_exist =
                Self::is_match_package_hash(pool, dv.current_package_id, &package_hash).await?;
            if is_exist {
                let _ = delete_folder(&directory_path_parent).await;
                return Err(AppError::General("The uploaded package is identical to the contents of the specified deployment's current release.".into()));
            }
        }

        let manifest_hash = calc_qetag(&manifest_file_path).await?;

        // Upload to storage
        storage
            .upload_file(&manifest_hash, manifest_file_path.to_str().unwrap())
            .await?;
        storage
            .upload_file(&blob_hash, file_path.to_str().unwrap())
            .await?;

        let file_size = tokio::fs::metadata(file_path).await?.len() as i64;

        let create_params = CreatePackageParams {
            release_method: RELEASE_METHOD_UPLOAD,
            release_uid,
            is_mandatory: package_info.is_mandatory.unwrap_or(false) as i64,
            is_disabled: package_info.is_disabled.unwrap_or(false) as i64,
            rollout: package_info.rollout.unwrap_or(100),
            size: file_size,
            description: package_info.description.as_deref().unwrap_or(""),
            original_label: "",
            original_deployment: "",
        };

        let package = Self::create_package(
            pool,
            deployment_id,
            &app_version,
            &package_hash,
            &manifest_hash,
            &blob_hash,
            create_params,
        )
        .await?;

        let _ = delete_folder(&directory_path_parent).await;

        Ok(package)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn generate_one_diff_package(
        config: &crate::config::AppConfig,
        pool: &SqlitePool,
        storage: &StorageManager,
        work_directory_path: &Path,
        package_id: i64,
        origin_data_center: &crate::services::datacenter_manager::PackageInfo,
        diff_package_hash: &str,
        diff_manifest_blob_hash: &str,
    ) -> Result<Option<PackagesDiff>, AppError> {
        let diff_package = sqlx::query_as::<_, PackagesDiff>(
            "SELECT * FROM packages_diff WHERE package_id = ? AND diff_against_package_hash = ?",
        )
        .bind(package_id)
        .bind(diff_package_hash)
        .fetch_optional(pool)
        .await?;

        if diff_package.is_some() {
            return Ok(None);
        }

        let download_url = get_blob_download_url(&config.download_url, diff_manifest_blob_hash);
        let diff_manifest_path = work_directory_path.join(diff_manifest_blob_hash);
        create_file_from_request(&download_url, &diff_manifest_path).await?;

        let data_center_content_path = work_directory_path.join("dataCenter");
        copy_dir_all(&origin_data_center.content_path, &data_center_content_path).await?;

        let origin_manifest_str =
            fs::read_to_string(&origin_data_center.manifest_file_path).await?;
        let origin_manifest_json: BTreeMap<String, String> =
            serde_json::from_str(&origin_manifest_str)?;

        let diff_manifest_str = fs::read_to_string(&diff_manifest_path).await?;
        let diff_manifest_json: BTreeMap<String, String> =
            serde_json::from_str(&diff_manifest_str)?;

        let diff_result = diff_collections(&origin_manifest_json, &diff_manifest_json);

        let mut files = diff_result.diff.clone();
        files.extend(diff_result.collection1_only.clone());

        // Code push expects a JSON with deletedFiles and patchedFiles
        let hotcodepush = serde_json::json!({
            "deletedFiles": diff_result.collection2_only,
            "patchedFiles": []
        });

        let hot_code_push_file =
            work_directory_path.join(format!("{}_hotcodepush", diff_manifest_blob_hash));
        fs::write(&hot_code_push_file, serde_json::to_string(&hotcodepush)?).await?;

        let file_name = work_directory_path.join(format!("{}.zip", diff_manifest_blob_hash));
        Self::zip_diff_package(
            &file_name,
            &files,
            &data_center_content_path,
            &hot_code_push_file,
        )?;

        let diff_hash = calc_qetag(&file_name).await?;
        storage
            .upload_file(&diff_hash, file_name.to_str().unwrap())
            .await?;

        let stats = tokio::fs::metadata(&file_name).await?;

        let diff = sqlx::query_as::<_, PackagesDiff>(
            "INSERT INTO packages_diff (package_id, diff_against_package_hash, diff_blob_url, diff_size, created_at, updated_at) VALUES (?, ?, ?, ?, datetime('now'), datetime('now')) RETURNING *"
        )
        .bind(package_id)
        .bind(diff_package_hash)
        .bind(&diff_hash)
        .bind(stats.len() as i64)
        .fetch_one(pool)
        .await?;

        Ok(Some(diff))
    }

    pub async fn create_diff_packages_by_last_nums(
        config: &crate::config::AppConfig,
        pool: &SqlitePool,
        storage: &StorageManager,
        app_id: i64,
        original_package: &Packages,
        num: i64,
    ) -> Result<(), AppError> {
        let package_id = original_package.id;
        let deployment_version_id = original_package.deployment_version_id;

        let mut last_nums_packages = sqlx::query_as::<_, Packages>(
            "SELECT * FROM packages WHERE deployment_version_id = ? AND id < ? ORDER BY id DESC LIMIT ?"
        )
        .bind(deployment_version_id)
        .bind(package_id)
        .bind(num)
        .fetch_all(pool)
        .await?;

        let base_packages = sqlx::query_as::<_, Packages>(
            "SELECT * FROM packages WHERE deployment_version_id = ? AND id < ? ORDER BY id ASC LIMIT 2"
        )
        .bind(deployment_version_id)
        .bind(package_id)
        .fetch_all(pool)
        .await?;

        last_nums_packages.extend(base_packages);

        // Remove duplicates by package_hash
        let mut seen = std::collections::HashSet::new();
        let mut dest_packages = Vec::new();
        for p in last_nums_packages {
            if seen.insert(p.package_hash.clone()) {
                dest_packages.push(p);
            }
        }

        Self::create_diff_packages(config, pool, storage, original_package, dest_packages).await?;
        Ok(())
    }

    pub async fn download_package_and_extract(
        config: &crate::config::AppConfig,
        work_directory_path: &Path,
        package_hash: &str,
        blob_hash: &str,
    ) -> Result<PackageInfo, AppError> {
        if DataCenterManager::validate_store(config, package_hash).await {
            return DataCenterManager::get_package_info(config, package_hash);
        }
        let download_url = get_blob_download_url(&config.download_url, blob_hash);
        let blob_path = work_directory_path.join(blob_hash);
        create_file_from_request(&download_url, &blob_path).await?;

        let extract_path = work_directory_path.join("current");
        unzip_file(&blob_path, &extract_path).await?;

        DataCenterManager::store_package(config, &extract_path.to_string_lossy(), true).await
    }

    pub async fn create_diff_packages(
        config: &crate::config::AppConfig,
        pool: &SqlitePool,
        storage: &StorageManager,
        original_package: &Packages,
        dest_packages: Vec<Packages>,
    ) -> Result<(), AppError> {
        if dest_packages.is_empty() {
            return Ok(());
        }

        let package_hash = original_package.package_hash.as_str();
        let blob_url = original_package.blob_url.as_str();

        let tmp_dir = std::path::Path::new(&config.data_dir);
        let work_directory_path = tmp_dir.join(format!("codepush_{}", rand_token(32)));
        create_empty_folder(&work_directory_path).await?;

        let origin_data_center = match Self::download_package_and_extract(
            config,
            &work_directory_path,
            package_hash,
            blob_url,
        )
        .await
        {
            Ok(data) => data,
            Err(e) => {
                tracing::error!("Failed to download and extract original package: {:?}", e);
                let _ = delete_folder(&work_directory_path).await;
                return Err(e);
            }
        };

        for v in dest_packages {
            let diff_package_hash = v.package_hash.as_str();
            let diff_manifest_blob_url = v.manifest_blob_url.as_str();

            let diff_work_directory_path = work_directory_path.join(diff_package_hash);
            if let Err(e) = create_empty_folder(&diff_work_directory_path).await {
                tracing::error!(
                    "Failed to create diff folder for {}: {:?}",
                    diff_package_hash,
                    e
                );
                continue;
            }

            if let Err(e) = Self::generate_one_diff_package(
                config,
                pool,
                storage,
                &diff_work_directory_path,
                original_package.id,
                &origin_data_center,
                diff_package_hash,
                diff_manifest_blob_url,
            )
            .await
            {
                tracing::error!("Failed to generate diff for {}: {:?}", diff_package_hash, e);
            }
        }

        let _ = delete_folder(&work_directory_path).await;
        Ok(())
    }

    pub async fn modify_release_package(
        pool: &SqlitePool,
        package_id: i64,
        params: ReleaseParams,
    ) -> Result<Packages, AppError> {
        let package_info = sqlx::query_as::<_, Packages>("SELECT * FROM packages WHERE id = ?")
            .bind(package_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::new("packageInfo not found"))?;

        if let Some(app_version) = &params.app_version {
            let (is_valid, min_version, max_version) = validator_version(app_version);
            if !is_valid {
                return Err(AppError::new(&format!(
                    "--targetBinaryVersion {} not support.",
                    app_version
                )));
            }

            let v1 = sqlx::query_as::<_, DeploymentVersion>(
                "SELECT * FROM deployments_versions WHERE deployment_id = ? AND app_version = ?",
            )
            .bind(package_info.deployment_id)
            .bind(app_version)
            .fetch_optional(pool)
            .await?;

            let v2 = sqlx::query_as::<_, DeploymentVersion>(
                "SELECT * FROM deployments_versions WHERE id = ?",
            )
            .bind(package_info.deployment_version_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::new("packages not found."))?;

            if let Some(v1_info) = v1
                && v1_info.id != v2.id
            {
                return Err(AppError::new(&format!("{} already exist.", app_version)));
            }

            sqlx::query(
                "UPDATE deployments_versions SET app_version = ?, min_version = ?, max_version = ? WHERE id = ?"
            )
            .bind(app_version)
            .bind(min_version)
            .bind(max_version)
            .bind(v2.id)
            .execute(pool)
            .await?;
        }

        let mut query = String::from("UPDATE packages SET ");
        let mut bindings_str = Vec::new();
        let mut bindings_i64 = Vec::new();

        if let Some(desc) = &params.description {
            query.push_str("description = ?, ");
            bindings_str.push(desc.clone());
        }
        if let Some(rollout) = params.rollout {
            query.push_str("rollout = ?, ");
            bindings_i64.push(rollout);
        }
        if let Some(is_mandatory) = params.is_mandatory {
            query.push_str("is_mandatory = ?, ");
            bindings_i64.push(if is_mandatory {
                IS_MANDATORY_YES
            } else {
                IS_MANDATORY_NO
            });
        }
        if let Some(is_disabled) = params.is_disabled {
            query.push_str("is_disabled = ?, ");
            bindings_i64.push(if is_disabled {
                IS_DISABLED_YES
            } else {
                IS_DISABLED_NO
            });
        }

        if query.ends_with(", ") {
            query.truncate(query.len() - 2);
            query.push_str(" WHERE id = ?");

            let mut q = sqlx::query(&query);
            for b in bindings_str {
                q = q.bind(b);
            }
            for b in bindings_i64 {
                q = q.bind(b);
            }
            q = q.bind(package_id);
            q.execute(pool).await?;
        }

        let updated_package = sqlx::query_as::<_, Packages>("SELECT * FROM packages WHERE id = ?")
            .bind(package_id)
            .fetch_one(pool)
            .await?;

        Ok(updated_package)
    }

    pub async fn promote_package(
        pool: &SqlitePool,
        source_deployment_info: &Deployment,
        dest_deployment_info: &Deployment,
        params: ReleaseParams,
        promote_uid: i64,
    ) -> Result<Packages, AppError> {
        let (source_pack, deployments_version) = if let Some(label) = &params.label {
            let source_pack = sqlx::query_as::<_, Packages>(
                "SELECT * FROM packages WHERE deployment_id = ? AND label = ?",
            )
            .bind(source_deployment_info.id)
            .bind(label)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::new("label does not exist."))?;

            let deployments_version = sqlx::query_as::<_, DeploymentVersion>(
                "SELECT * FROM deployments_versions WHERE id = ?",
            )
            .bind(source_pack.deployment_version_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::new("deploymentsVersions does not exist."))?;

            (source_pack, deployments_version)
        } else {
            let last_deployment_version_id = source_deployment_info.last_deployment_version_id;
            if last_deployment_version_id <= 0 {
                return Err(AppError::new("does not exist last_deployment_version_id."));
            }

            let deployments_version = sqlx::query_as::<_, DeploymentVersion>(
                "SELECT * FROM deployments_versions WHERE id = ?",
            )
            .bind(last_deployment_version_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::new("deploymentsVersions does not exist."))?;

            let source_pack = sqlx::query_as::<_, Packages>("SELECT * FROM packages WHERE id = ?")
                .bind(deployments_version.current_package_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::new("packageInfo not found."))?;

            (source_pack, deployments_version)
        };

        let app_final_version = params
            .app_version
            .clone()
            .unwrap_or(deployments_version.app_version.clone());

        let dest_deployments_version = sqlx::query_as::<_, DeploymentVersion>(
            "SELECT * FROM deployments_versions WHERE deployment_id = ? AND app_version = ?",
        )
        .bind(dest_deployment_info.id)
        .bind(&app_final_version)
        .fetch_optional(pool)
        .await?;

        if let Some(dest_dv) = dest_deployments_version {
            let is_match = Self::is_match_package_hash(
                pool,
                dest_dv.current_package_id,
                source_pack.package_hash.as_str(),
            )
            .await?;
            if is_match {
                return Err(AppError::new(
                    "The uploaded package is identical to the contents of the specified deployment's current release.",
                ));
            }
        }

        let (is_valid, min_version, max_version) = validator_version(&app_final_version);
        if !is_valid {
            return Err(AppError::new(&format!(
                "targetBinaryVersion {} not support.",
                app_final_version
            )));
        }

        let create_params = CreatePackageParams {
            release_method: RELEASE_METHOD_PROMOTE,
            release_uid: promote_uid,
            is_mandatory: params
                .is_mandatory
                .unwrap_or(source_pack.is_mandatory == IS_MANDATORY_YES)
                as i64,
            is_disabled: params
                .is_disabled
                .unwrap_or(source_pack.is_disabled == IS_DISABLED_YES)
                as i64,
            rollout: params.rollout.unwrap_or(100),
            size: source_pack.size,
            description: params
                .description
                .as_deref()
                .unwrap_or(source_pack.description.as_str()),
            original_label: source_pack.label.as_str(),
            original_deployment: &source_deployment_info.name,
        };

        Self::create_package(
            pool,
            dest_deployment_info.id,
            &app_final_version,
            source_pack.package_hash.as_str(),
            source_pack.manifest_blob_url.as_str(),
            source_pack.blob_url.as_str(),
            create_params,
        )
        .await
    }

    pub async fn rollback_package(
        pool: &SqlitePool,
        deployment_version_id: i64,
        target_label: Option<&str>,
        rollback_uid: i64,
    ) -> Result<Packages, AppError> {
        let deployments_version = sqlx::query_as::<_, DeploymentVersion>(
            "SELECT * FROM deployments_versions WHERE id = ?",
        )
        .bind(deployment_version_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::new("does not find the deploymentsVersions"))?;

        let current_package_info =
            sqlx::query_as::<_, Packages>("SELECT * FROM packages WHERE id = ?")
                .bind(deployments_version.current_package_id)
                .fetch_optional(pool)
                .await?;

        let rollback_package_infos = if let Some(label) = target_label {
            sqlx::query_as::<_, Packages>(
                "SELECT * FROM packages WHERE deployment_version_id = ? AND label = ? LIMIT 1",
            )
            .bind(deployment_version_id)
            .bind(label)
            .fetch_all(pool)
            .await?
        } else {
            Self::get_can_rollback_packages(pool, deployment_version_id).await?
        };

        let current_package_info =
            current_package_info.ok_or_else(|| AppError::new("no current package info"))?;

        let mut target_rollback = None;
        if !rollback_package_infos.is_empty() {
            for rp in rollback_package_infos.into_iter().rev() {
                if rp.package_hash != current_package_info.package_hash {
                    target_rollback = Some(rp);
                    break;
                }
            }
        }

        let rollback_package =
            target_rollback.ok_or_else(|| AppError::new("no package can be rolled back."))?;

        let create_params = CreatePackageParams {
            release_method: "Rollback",
            release_uid: rollback_uid,
            is_mandatory: rollback_package.is_mandatory,
            is_disabled: rollback_package.is_disabled,
            rollout: rollback_package.rollout,
            size: rollback_package.size,
            description: rollback_package.description.as_str(),
            original_label: rollback_package.label.as_str(),
            original_deployment: "",
        };

        Self::create_package(
            pool,
            deployments_version.deployment_id,
            &deployments_version.app_version,
            rollback_package.package_hash.as_str(),
            rollback_package.manifest_blob_url.as_str(),
            rollback_package.blob_url.as_str(),
            create_params,
        )
        .await
    }

    pub async fn get_can_rollback_packages(
        pool: &SqlitePool,
        deployment_version_id: i64,
    ) -> Result<Vec<Packages>, AppError> {
        let packages = sqlx::query_as::<_, Packages>(
            "SELECT * FROM packages WHERE deployment_version_id = ? AND release_method IN (?, ?) ORDER BY id DESC LIMIT 2"
        )
        .bind(deployment_version_id)
        .bind(RELEASE_METHOD_UPLOAD)
        .bind(RELEASE_METHOD_PROMOTE)
        .fetch_all(pool)
        .await?;

        Ok(packages)
    }

    pub async fn delete_package_by_label(
        pool: &SqlitePool,
        deployment_id: i64,
        label: &str,
    ) -> Result<(), AppError> {
        let package_info = sqlx::query_as::<_, Packages>(
            "SELECT * FROM packages WHERE deployment_id = ? AND label = ?",
        )
        .bind(deployment_id)
        .bind(label)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::new("Package not found."))?;

        // Here you would typically interact with your storage solution
        // to delete the actual package files.

        Ok(())
    }
}
