#[allow(dead_code)]
#[path = "../src/config/mod.rs"]
mod config;
#[allow(dead_code)]
#[path = "../src/core/mod.rs"]
mod core;
#[allow(dead_code)]
#[path = "../src/models/mod.rs"]
mod models;
#[allow(dead_code)]
#[path = "../src/routes/mod.rs"]
mod routes;
#[allow(dead_code)]
#[path = "../src/services/mod.rs"]
mod services;
#[allow(dead_code)]
#[path = "../src/utils/mod.rs"]
mod utils;

#[path = "support_bulk/mod.rs"]
mod support;

use support::{app, request, request_with_token};

async fn add_package(app: &support::TestApp, id: i64, label: &str, disabled: i64) {
    sqlx::query("INSERT INTO packages VALUES (?, 1, 1, '', ?, ?, 3, ?, 'Upload', ?, '', '', 1, 0, ?, 100, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .bind(id)
        .bind(format!("hash-{id}"))
        .bind(format!("blob-{id}"))
        .bind(format!("manifest-{id}"))
        .bind(label)
        .bind(disabled)
        .execute(&app.pool).await.unwrap();
}

async fn write_key(app: &support::TestApp, key: &str, bytes: &[u8]) {
    let prefix = &key[..2].to_ascii_lowercase();
    let dir = app.storage_dir.join(prefix);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(key), bytes).unwrap();
}

async fn add_other_app(app: &support::TestApp) {
    sqlx::query("INSERT INTO apps VALUES (2, 'other-app', 2, 1, 1, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&app.pool).await.unwrap();
    sqlx::query("INSERT INTO deployments VALUES (2, 2, 'Production', '', 'other-key', 0, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&app.pool).await.unwrap();
}

#[tokio::test]
async fn patch_bulk_updates_all_scoped_releases_and_preserves_omitted_fields() {
    let app = app().await;
    add_package(&app, 1, "v1", 0).await;
    add_package(&app, 2, "v2", 1).await;
    let (status, response) = request(
        &app,
        "PATCH",
        "/app/deployments/Production/releases/bulk",
        r#"{"labels":["v1","v2"],"packageInfo":{"isMandatory":true,"rollout":55}}"#,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(response, serde_json::json!({"labels":["v1","v2"]}));
    let rows: Vec<(i64, i64, i64)> =
        sqlx::query_as("SELECT is_disabled, is_mandatory, rollout FROM packages ORDER BY id")
            .fetch_all(&app.pool)
            .await
            .unwrap();
    assert_eq!(rows, [(0, 1, 55), (1, 1, 55)]);
}

#[tokio::test]
async fn patch_bulk_rejects_missing_label_without_partial_updates() {
    let app = app().await;
    add_package(&app, 1, "v1", 0).await;
    let (status, _) = request(
        &app,
        "PATCH",
        "/app/deployments/Production/releases/bulk",
        r#"{"labels":["v1","missing"],"packageInfo":{"isDisabled":true}}"#,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
    let disabled: i64 = sqlx::query_scalar("SELECT is_disabled FROM packages WHERE label = 'v1'")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(disabled, 0);
}

#[tokio::test]
async fn patch_bulk_requires_owner_and_resolves_app_deployment_scope() {
    let app = app().await;
    add_package(&app, 1, "v1", 0).await;
    let (status, _) = request(
        &app,
        "PATCH",
        "/app/deployments/Production/releases/bulk",
        r#"{"labels":["v1"],"packageInfo":{"isMandatory":true}}"#,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let (status, _) = request(
        &app,
        "PATCH",
        "/missing/deployments/Production/releases/bulk",
        r#"{"labels":["v1"],"packageInfo":{"isMandatory":true}}"#,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_bulk_removes_only_unshared_local_files_and_reports_exact_bytes() {
    let app = app().await;
    add_package(&app, 1, "v1", 1).await;
    write_key(&app, "blob-1", b"bundle").await;
    write_key(&app, "manifest-1", b"manifest").await;
    let (status, response) = request(
        &app,
        "DELETE",
        "/app/deployments/Production/releases/bulk",
        r#"{"labels":["v1"]}"#,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(response["deletedCount"], 2);
    assert_eq!(response["freedBytes"], 14);
    assert_eq!(response["results"][0]["status"], "cleaned");
    assert!(!app.storage_dir.join("bl/blob-1").exists());
    assert!(!app.storage_dir.join("ma/manifest-1").exists());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM packages WHERE id = 1")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn patch_bulk_cannot_enable_release_with_missing_local_files() {
    let app = app().await;
    add_package(&app, 1, "v1", 1).await;
    let (status, response) = request(
        &app,
        "PATCH",
        "/app/deployments/Production/releases/bulk",
        r#"{"labels":["v1"],"packageInfo":{"isDisabled":false}}"#,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(response["status"], "ERROR");
    let disabled: i64 = sqlx::query_scalar("SELECT is_disabled FROM packages WHERE id = 1")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(disabled, 1);
}

#[tokio::test]
async fn delete_bulk_preserves_files_shared_by_another_apps_package() {
    let app = app().await;
    add_package(&app, 1, "v1", 1).await;
    add_other_app(&app).await;
    sqlx::query("INSERT INTO packages VALUES (2, 2, 2, '', 'other-hash', 'blob-1', 8, 'other-manifest', 'Upload', 'v1', '', '', 1, 0, 0, 100, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&app.pool).await.unwrap();
    write_key(&app, "blob-1", b"bundle").await;
    write_key(&app, "manifest-1", b"manifest").await;
    let (_, response) = request(
        &app,
        "DELETE",
        "/app/deployments/Production/releases/bulk",
        r#"{"labels":["v1"]}"#,
    )
    .await;
    assert_eq!(response["deletedCount"], 1);
    assert_eq!(response["freedBytes"], 8);
    assert_eq!(response["results"][0]["status"], "skipped");
    assert!(app.storage_dir.join("bl/blob-1").exists());
    assert!(!app.storage_dir.join("ma/manifest-1").exists());
}

#[tokio::test]
async fn delete_bulk_preserves_file_referenced_by_an_unselected_diff() {
    let app = app().await;
    add_package(&app, 1, "selected", 1).await;
    add_package(&app, 2, "other", 1).await;
    sqlx::query("INSERT INTO packages_diff VALUES (1, 2, 'hash-1', 'diff-blob', 4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&app.pool).await.unwrap();
    write_key(&app, "blob-1", b"bundle").await;
    write_key(&app, "manifest-1", b"manifest").await;
    write_key(&app, "diff-blob", b"diff").await;
    let (_, response) = request(
        &app,
        "DELETE",
        "/app/deployments/Production/releases/bulk",
        r#"{"labels":["selected"]}"#,
    )
    .await;
    assert_eq!(response["results"][0]["status"], "skipped");
    assert!(app.storage_dir.join("di/diff-blob").exists());
}

#[tokio::test]
async fn delete_bulk_fails_closed_for_traversal_keys_before_removing_other_files() {
    let app = app().await;
    sqlx::query("INSERT INTO packages VALUES (1, 1, 1, '', 'hash-1', '../escape', 3, 'manifest-1', 'Upload', 'v1', '', '', 1, 0, 1, 100, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&app.pool).await.unwrap();
    write_key(&app, "manifest-1", b"manifest").await;
    let outside = app.test_dir.join("escape");
    std::fs::write(&outside, b"outside").unwrap();
    let (_, response) = request(
        &app,
        "DELETE",
        "/app/deployments/Production/releases/bulk",
        r#"{"labels":["v1"]}"#,
    )
    .await;
    assert_eq!(response["deletedCount"], 0);
    assert_eq!(response["results"][0]["status"], "error");
    assert!(app.storage_dir.join("ma/manifest-1").exists());
    assert_eq!(std::fs::read(outside).unwrap(), b"outside");
}

#[tokio::test]
async fn delete_bulk_skips_files_for_live_current_packages() {
    let app = app().await;
    add_package(&app, 1, "v1", 0).await;
    sqlx::query("INSERT INTO deployments_versions VALUES (1, 1, '1.0', 1, 0, 100, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&app.pool).await.unwrap();
    write_key(&app, "blob-1", b"bundle").await;
    write_key(&app, "manifest-1", b"manifest").await;
    let (_, response) = request(
        &app,
        "DELETE",
        "/app/deployments/Production/releases/bulk",
        r#"{"labels":["v1"]}"#,
    )
    .await;
    assert_eq!(response["deletedCount"], 0);
    assert_eq!(response["results"][0]["status"], "skipped");
    assert_eq!(response["results"][0]["skippedCount"], 2);
    assert!(app.storage_dir.join("bl/blob-1").exists());
}

#[tokio::test]
async fn delete_bulk_protects_shared_file_used_by_current_package() {
    let app = app().await;
    add_package(&app, 1, "inactive", 1).await;
    sqlx::query("INSERT INTO packages VALUES (2, 1, 1, '', 'hash-current', 'blob-1', 3, 'manifest-2', 'Upload', 'active', '', '', 1, 0, 0, 100, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&app.pool).await.unwrap();
    sqlx::query("INSERT INTO deployments_versions VALUES (1, 1, '1.0', 2, 0, 100, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&app.pool).await.unwrap();
    write_key(&app, "blob-1", b"bundle").await;
    write_key(&app, "manifest-1", b"manifest").await;
    write_key(&app, "manifest-2", b"current manifest").await;
    let (_, response) = request(
        &app,
        "DELETE",
        "/app/deployments/Production/releases/bulk",
        r#"{"labels":["inactive"]}"#,
    )
    .await;
    assert_eq!(response["deletedCount"], 1);
    assert_eq!(response["results"][0]["status"], "skipped");
    assert!(app.storage_dir.join("bl/blob-1").exists());
    assert!(!app.storage_dir.join("ma/manifest-1").exists());
}

#[tokio::test]
async fn delete_bulk_can_clean_a_disabled_current_package() {
    let app = app().await;
    add_package(&app, 1, "disabled", 1).await;
    sqlx::query("INSERT INTO deployments_versions VALUES (1, 1, '1.0', 1, 0, 100, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&app.pool).await.unwrap();
    write_key(&app, "blob-1", b"bundle").await;
    write_key(&app, "manifest-1", b"manifest").await;
    let (_, response) = request(
        &app,
        "DELETE",
        "/app/deployments/Production/releases/bulk",
        r#"{"labels":["disabled"]}"#,
    )
    .await;
    assert_eq!(response["deletedCount"], 2);
    assert_eq!(response["results"][0]["status"], "cleaned");
    assert!(!app.storage_dir.join("bl/blob-1").exists());
    assert!(!app.storage_dir.join("ma/manifest-1").exists());
}

#[tokio::test]
async fn delete_bulk_fails_closed_on_symlinked_storage_prefix() {
    let app = app().await;
    add_package(&app, 1, "v1", 1).await;
    write_key(&app, "manifest-1", b"manifest").await;
    let outside = app.test_dir.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("blob-1"), b"external").unwrap();
    std::os::unix::fs::symlink(&outside, app.storage_dir.join("bl")).unwrap();
    let (_, response) = request(
        &app,
        "DELETE",
        "/app/deployments/Production/releases/bulk",
        r#"{"labels":["v1"]}"#,
    )
    .await;
    assert_eq!(response["deletedCount"], 0);
    assert_eq!(response["results"][0]["status"], "error");
    assert_eq!(std::fs::read(outside.join("blob-1")).unwrap(), b"external");
    assert!(app.storage_dir.join("ma/manifest-1").exists());
}

#[tokio::test]
async fn owner_collaborator_can_bulk_update_only_the_named_apps_release() {
    let app = app().await;
    add_package(&app, 1, "same-label", 0).await;
    add_other_app(&app).await;
    sqlx::query("INSERT INTO users VALUES (2, 'owner2', '', 'owner2@example.test', 'identical2', 'ack', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&app.pool).await.unwrap();
    sqlx::query(
        "INSERT INTO collaborators VALUES (2, 2, 2, 'Owner', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
    )
    .execute(&app.pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO packages VALUES (2, 2, 2, '', 'hash-other', 'blob-other', 3, 'manifest-other', 'Upload', 'same-label', '', '', 1, 0, 0, 100, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&app.pool).await.unwrap();
    let (status, _) = request_with_token(
        &app,
        "PATCH",
        "/other-app/deployments/Production/releases/bulk",
        r#"{"labels":["same-label"],"packageInfo":{"isMandatory":true}}"#,
        Some(&support::token_for_uid(2)),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let rows: Vec<(i64, i64)> =
        sqlx::query_as("SELECT deployment_id, is_mandatory FROM packages ORDER BY id")
            .fetch_all(&app.pool)
            .await
            .unwrap();
    assert_eq!(rows, [(1, 0), (2, 1)]);
}

#[tokio::test]
async fn invalid_bulk_patch_fields_are_rejected_without_writing() {
    let app = app().await;
    add_package(&app, 1, "v1", 0).await;
    for body in [
        r#"{"labels":["v1"],"packageInfo":{"unexpected":true}}"#,
        r#"{"labels":["v1"],"packageInfo":{}}"#,
        r#"{"labels":["v1","v1"],"packageInfo":{"rollout":50}}"#,
        r#"{"labels":["v1"],"packageInfo":{"rollout":101}}"#,
        r#"{"labels":["v1"],"packageInfo":{"rollout":null}}"#,
    ] {
        let (status, response) = request(
            &app,
            "PATCH",
            "/app/deployments/Production/releases/bulk",
            body,
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(response["status"], "ERROR");
        assert!(response["message"].as_str().is_some());
        if body.contains("unexpected") {
            assert!(
                response["message"]
                    .as_str()
                    .unwrap()
                    .contains("unsupported")
            );
        }
    }
    let row: (i64, i64) = sqlx::query_as("SELECT is_mandatory, rollout FROM packages WHERE id = 1")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(row, (0, 100));
}

#[tokio::test]
async fn delete_bulk_rejects_missing_label_before_removing_any_files() {
    let app = app().await;
    add_package(&app, 1, "v1", 1).await;
    write_key(&app, "blob-1", b"bundle").await;
    let (status, _) = request(
        &app,
        "DELETE",
        "/app/deployments/Production/releases/bulk",
        r#"{"labels":["v1","missing"]}"#,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
    assert!(app.storage_dir.join("bl/blob-1").exists());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM packages WHERE label = 'v1'")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn delete_bulk_keeps_skipped_status_when_another_file_is_missing() {
    let app = app().await;
    add_package(&app, 1, "v1", 1).await;
    add_other_app(&app).await;
    sqlx::query("INSERT INTO packages VALUES (2, 2, 2, '', 'other-hash', 'blob-1', 8, 'other-manifest', 'Upload', 'other', '', '', 1, 0, 0, 100, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&app.pool).await.unwrap();
    write_key(&app, "blob-1", b"bundle").await;

    let (_, response) = request(
        &app,
        "DELETE",
        "/app/deployments/Production/releases/bulk",
        r#"{"labels":["v1"]}"#,
    )
    .await;

    assert_eq!(response["results"][0]["status"], "skipped");
    assert_eq!(response["results"][0]["skippedCount"], 1);
    assert_eq!(response["results"][0]["missingCount"], 1);
    assert!(app.storage_dir.join("bl/blob-1").exists());
}

#[tokio::test]
async fn delete_bulk_counts_a_shared_file_once_per_release_and_globally() {
    let app = app().await;
    add_package(&app, 1, "v1", 1).await;
    add_package(&app, 2, "v2", 1).await;
    sqlx::query("UPDATE packages SET blob_url = 'shared-blob', manifest_blob_url = 'shared-manifest' WHERE id IN (1, 2)")
        .execute(&app.pool).await.unwrap();
    write_key(&app, "shared-blob", b"bundle-bytes").await;
    write_key(&app, "shared-manifest", b"manifest").await;
    let (status, response) = request(
        &app,
        "DELETE",
        "/app/deployments/Production/releases/bulk",
        r#"{"labels":["v1","v2"]}"#,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(response["deletedCount"], 2);
    assert_eq!(
        response["freedBytes"],
        (b"bundle-bytes".len() + b"manifest".len()) as u64
    );
    assert_eq!(response["results"][0]["deletedCount"], 2);
    assert_eq!(response["results"][1]["deletedCount"], 2);
    assert!(!app.storage_dir.join("sh/shared-blob").exists());
    assert!(!app.storage_dir.join("sh/shared-manifest").exists());
}

#[tokio::test]
async fn patch_bulk_enables_release_when_required_local_files_exist() {
    let app = app().await;
    add_package(&app, 1, "v1", 1).await;
    write_key(&app, "blob-1", b"bundle").await;
    write_key(&app, "manifest-1", b"manifest").await;
    let (status, response) = request(
        &app,
        "PATCH",
        "/app/deployments/Production/releases/bulk",
        r#"{"labels":["v1"],"packageInfo":{"isDisabled":false}}"#,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(response, serde_json::json!({"labels":["v1"]}));
    let disabled: i64 = sqlx::query_scalar("SELECT is_disabled FROM packages WHERE id = 1")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(disabled, 0);
}

#[tokio::test]
async fn patch_bulk_cannot_enable_when_required_diff_file_is_missing() {
    let app = app().await;
    add_package(&app, 1, "v1", 1).await;
    write_key(&app, "blob-1", b"bundle").await;
    write_key(&app, "manifest-1", b"manifest").await;
    sqlx::query("INSERT INTO packages_diff VALUES (1, 1, 'base-hash', 'missing-diff', 4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&app.pool).await.unwrap();
    let (status, response) = request(
        &app,
        "PATCH",
        "/app/deployments/Production/releases/bulk",
        r#"{"labels":["v1"],"packageInfo":{"isDisabled":false}}"#,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(response["status"], "ERROR");
    assert!(
        response["message"]
            .as_str()
            .unwrap()
            .contains("missing-diff")
    );
    let disabled: i64 = sqlx::query_scalar("SELECT is_disabled FROM packages WHERE id = 1")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(disabled, 1);
}

#[tokio::test]
async fn bulk_writes_require_authenticated_current_app_owner_and_preserve_rows() {
    let app = app().await;
    add_package(&app, 1, "v1", 1).await;
    add_other_app(&app).await;
    sqlx::query("INSERT INTO users VALUES (2, 'collab', '', 'collab@example.test', 'identical2', 'ack', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&app.pool).await.unwrap();
    sqlx::query("INSERT INTO collaborators VALUES (2, 1, 2, 'Collaborator', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&app.pool).await.unwrap();
    sqlx::query("INSERT INTO packages_diff VALUES (1, 1, 'base', 'diff-1', 4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&app.pool).await.unwrap();
    sqlx::query("INSERT INTO packages_metrics VALUES (1, 1, 1, 2, 3, 4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&app.pool).await.unwrap();
    sqlx::query("INSERT INTO deployments_history VALUES (1, 1, 1, CURRENT_TIMESTAMP)")
        .execute(&app.pool)
        .await
        .unwrap();
    write_key(&app, "blob-1", b"bundle").await;
    let before: (i64, i64, i64, i64, i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM packages), (SELECT count(*) FROM packages_diff), (SELECT count(*) FROM packages_metrics), (SELECT count(*) FROM deployments_history), (SELECT is_disabled FROM packages WHERE id=1), (SELECT is_mandatory FROM packages WHERE id=1)")
        .fetch_one(&app.pool).await.unwrap();
    for (method, body, token) in [
        (
            "PATCH",
            r#"{"labels":["v1"],"packageInfo":{"isMandatory":true}}"#,
            Some(support::token_for_uid(2)),
        ),
        (
            "DELETE",
            r#"{"labels":["v1"]}"#,
            Some(support::token_for_uid(2)),
        ),
        (
            "PATCH",
            r#"{"labels":["v1"],"packageInfo":{"isMandatory":true}}"#,
            None,
        ),
        ("DELETE", r#"{"labels":["v1"]}"#, None),
    ] {
        let (status, _response) = request_with_token(
            &app,
            method,
            "/app/deployments/Production/releases/bulk",
            body,
            token.as_deref(),
        )
        .await;
        if token.is_none() {
            assert_eq!(status, axum::http::StatusCode::UNAUTHORIZED);
        } else {
            assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
        }
    }
    let after: (i64, i64, i64, i64, i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM packages), (SELECT count(*) FROM packages_diff), (SELECT count(*) FROM packages_metrics), (SELECT count(*) FROM deployments_history), (SELECT is_disabled FROM packages WHERE id=1), (SELECT is_mandatory FROM packages WHERE id=1)")
        .fetch_one(&app.pool).await.unwrap();
    assert_eq!(after, before);
    assert!(app.storage_dir.join("bl/blob-1").exists());
}

#[tokio::test]
async fn bulk_writes_recheck_deployment_id_separately_from_app_id() {
    let app = app().await;
    sqlx::query("INSERT INTO packages VALUES (5, 1, 5, '', 'hash-staging', 'blob-staging', 3, 'manifest-staging', 'Upload', 'staging-release', '', '', 1, 0, 1, 100, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&app.pool).await.unwrap();
    write_key(&app, "blob-staging", b"bundle").await;
    write_key(&app, "manifest-staging", b"manifest").await;
    let (patch_status, patched) = request(
        &app,
        "PATCH",
        "/app/deployments/Staging/releases/bulk",
        r#"{"labels":["staging-release"],"packageInfo":{"isMandatory":true}}"#,
    )
    .await;
    assert_eq!(patch_status, axum::http::StatusCode::OK);
    assert_eq!(patched, serde_json::json!({"labels":["staging-release"]}));
    let (delete_status, deleted) = request(
        &app,
        "DELETE",
        "/app/deployments/Staging/releases/bulk",
        r#"{"labels":["staging-release"]}"#,
    )
    .await;
    assert_eq!(delete_status, axum::http::StatusCode::OK);
    assert_eq!(deleted["deletedCount"], 2);
    assert!(!app.storage_dir.join("bl/blob-staging").exists());
    assert!(!app.storage_dir.join("ma/manifest-staging").exists());
}

#[tokio::test]
async fn delete_bulk_removes_selected_diff_file_but_preserves_database_rows() {
    let app = app().await;
    add_package(&app, 1, "v1", 1).await;
    sqlx::query("INSERT INTO packages_diff VALUES (1, 1, 'base-hash', 'diff-v1', 4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&app.pool).await.unwrap();
    sqlx::query("INSERT INTO deployments_history VALUES (1, 1, 1, CURRENT_TIMESTAMP)")
        .execute(&app.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO packages_metrics VALUES (1, 1, 2, 3, 4, 5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&app.pool).await.unwrap();
    for (key, contents) in [
        ("blob-1", &b"bundle"[..]),
        ("manifest-1", &b"manifest"[..]),
        ("diff-v1", &b"diff"[..]),
    ] {
        write_key(&app, key, contents).await;
    }
    let (status, response) = request(
        &app,
        "DELETE",
        "/app/deployments/Production/releases/bulk",
        r#"{"labels":["v1"]}"#,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(response["deletedCount"], 3);
    assert_eq!(
        response["freedBytes"],
        (b"bundle".len() + b"manifest".len() + b"diff".len()) as u64
    );
    for key in ["blob-1", "manifest-1", "diff-v1"] {
        assert!(!app.storage_dir.join(&key[..2]).join(key).exists());
    }
    let rows: (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM packages), (SELECT count(*) FROM packages_diff), (SELECT count(*) FROM packages_metrics), (SELECT count(*) FROM deployments_history)",
    )
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(rows, (1, 1, 1, 1));
}
