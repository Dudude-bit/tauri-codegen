//! Types for wildcard re-export test

use serde::{Deserialize, Serialize};

/// Pod information for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodInfo {
    pub name: String,
    pub namespace: String,
    pub status: String,
    pub containers: Vec<ContainerInfo>,
}

/// Container information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub name: String,
    pub image: String,
    pub ready: bool,
}

/// Deployment information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentInfo {
    pub name: String,
    pub namespace: String,
    pub replicas: ReplicaInfo,
}

/// Replica information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaInfo {
    pub desired: i32,
    pub ready: i32,
    pub available: i32,
}

/// Service information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    pub namespace: String,
    pub cluster_ip: Option<String>,
}

/// Status enum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceStatus {
    Running,
    Pending,
    Failed,
    Unknown,
}

