use axum::{
    Router,
    routing::{delete, get, post},
};

use crate::core::db::AppState;

pub mod app_handlers;
pub mod collaborators;
pub mod deployments;
pub mod releases;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(app_handlers::list_apps).post(app_handlers::create_app))
        .route(
            "/{app_name}",
            delete(app_handlers::delete_app).patch(app_handlers::rename_app),
        )
        .route("/{app_name}/transfer/{email}", post(app_handlers::transfer_app))
        .route("/{app_name}/collaborators", get(collaborators::get_collaborators))
        .route(
            "/{app_name}/collaborators/{email}",
            post(collaborators::add_collaborator).delete(collaborators::remove_collaborator),
        )
        .route(
            "/{app_name}/deployments",
            get(deployments::list_deployments).post(deployments::create_deployment),
        )
        .route(
            "/{app_name}/deployments/{deployment_name}",
            get(deployments::get_deployment)
                .patch(deployments::update_deployment)
                .delete(deployments::delete_deployment),
        )
        .route(
            "/{app_name}/deployments/{deployment_name}/metrics",
            get(deployments::get_deployment_metrics),
        )
        .route(
            "/{app_name}/deployments/{deployment_name}/history",
            get(deployments::get_deployment_history).delete(deployments::delete_deployment_history),
        )
        .route(
            "/{app_name}/deployments/{deployment_name}/release",
            post(releases::release_package).patch(releases::modify_release_package),
        )
        .route(
            "/{app_name}/deployments/{deployment_name}/releases/{label}",
            get(releases::get_release_by_label)
                .patch(releases::modify_release_package_with_label)
                .delete(releases::delete_release_by_label),
        )
        .route(
            "/{app_name}/deployments/{source_deployment_name}/promote/{dest_deployment_name}",
            post(releases::promote_package),
        )
        .route(
            "/{app_name}/deployments/{deployment_name}/rollback",
            post(releases::rollback),
        )
        .route(
            "/{app_name}/deployments/{deployment_name}/rollback/{label}",
            post(releases::rollback_with_label),
        )
}
