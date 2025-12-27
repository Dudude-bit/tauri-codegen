//! Commands that use wildcard re-exported types

use crate::wildcard_reexport::{PodInfo, DeploymentInfo, ServiceInfo, ResourceStatus};
use tauri::State;

/// List pods
#[tauri::command]
pub async fn list_pods(namespace: Option<String>) -> Result<Vec<PodInfo>, String> {
    todo!()
}

/// Get pod by name
#[tauri::command]
pub async fn get_pod(name: String, namespace: Option<String>) -> Result<PodInfo, String> {
    todo!()
}

/// List deployments
#[tauri::command]
pub async fn list_deployments(namespace: Option<String>) -> Result<Vec<DeploymentInfo>, String> {
    todo!()
}

/// Get service
#[tauri::command]
pub async fn get_service(name: String, namespace: Option<String>) -> Result<ServiceInfo, String> {
    todo!()
}

/// Get resource status
#[tauri::command]
pub async fn get_resource_status(name: String) -> Result<ResourceStatus, String> {
    todo!()
}

