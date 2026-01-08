//! Integration tests for the full pipeline

use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;
use tauri_ts_generator::config::{Config, InputConfig, NamingConfig, OutputConfig};
use tauri_ts_generator::pipeline::Pipeline;

/// Create a test config with temp directories
fn create_test_config(source_dir: PathBuf, output_dir: PathBuf) -> Config {
    Config {
        input: InputConfig {
            source_dir,
            exclude: vec!["tests".to_string(), "target".to_string()],
            use_cargo_expand: false,
            cargo_manifest: None,
        },
        output: OutputConfig {
            types_file: output_dir.join("types.ts"),
            commands_file: output_dir.join("commands.ts"),
        },
        naming: NamingConfig::default(),
    }
}

#[test]
fn test_full_pipeline_simple() {
    let temp = tempdir().unwrap();
    let src_dir = temp.path().join("src");
    let output_dir = temp.path().join("generated");

    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    // Create a simple Rust file with commands and types
    let code = r#"
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct User {
    pub id: i32,
    pub name: String,
}

#[tauri::command]
pub fn get_user(id: i32) -> Result<User, String> {
    unimplemented!()
}

#[tauri::command]
pub fn list_users() -> Vec<User> {
    vec![]
}
"#;

    fs::write(src_dir.join("commands.rs"), code).unwrap();

    let config = create_test_config(src_dir, output_dir.clone());
    let pipeline = Pipeline::new(false);

    let result = pipeline.run(&config);
    assert!(result.is_ok(), "Pipeline should succeed: {:?}", result.err());

    // Verify output files exist
    assert!(output_dir.join("types.ts").exists());
    assert!(output_dir.join("commands.ts").exists());

    // Verify types.ts content
    let types_content = fs::read_to_string(output_dir.join("types.ts")).unwrap();
    assert!(types_content.contains("export interface User"));
    assert!(types_content.contains("id: number"));
    assert!(types_content.contains("name: string"));

    // Verify commands.ts content
    let commands_content = fs::read_to_string(output_dir.join("commands.ts")).unwrap();
    assert!(commands_content.contains("export async function getUser"));
    assert!(commands_content.contains("export async function listUsers"));
    assert!(commands_content.contains("import { invoke }"));
}

#[test]
fn test_pipeline_with_multiple_files() {
    let temp = tempdir().unwrap();
    let src_dir = temp.path().join("src");
    let output_dir = temp.path().join("generated");

    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    // types.rs
    let types_code = r#"
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct User {
    pub id: i32,
    pub name: String,
}

#[derive(Serialize, Deserialize)]
pub struct Item {
    pub id: i32,
    pub title: String,
}
"#;
    fs::write(src_dir.join("types.rs"), types_code).unwrap();

    // commands.rs
    let commands_code = r#"
use crate::types::{User, Item};

#[tauri::command]
pub fn get_user(id: i32) -> User {
    unimplemented!()
}

#[tauri::command]
pub fn get_item(id: i32) -> Item {
    unimplemented!()
}
"#;
    fs::write(src_dir.join("commands.rs"), commands_code).unwrap();

    let config = create_test_config(src_dir, output_dir.clone());
    let pipeline = Pipeline::new(false);

    let result = pipeline.run(&config);
    assert!(result.is_ok(), "Pipeline should succeed: {:?}", result.err());

    // Verify both types are in output
    let types_content = fs::read_to_string(output_dir.join("types.ts")).unwrap();
    assert!(types_content.contains("export interface User"));
    assert!(types_content.contains("export interface Item"));

    // Verify both commands are in output
    let commands_content = fs::read_to_string(output_dir.join("commands.ts")).unwrap();
    assert!(commands_content.contains("export async function getUser"));
    assert!(commands_content.contains("export async function getItem"));
}

#[test]
fn test_pipeline_excludes_directories() {
    let temp = tempdir().unwrap();
    let src_dir = temp.path().join("src");
    let output_dir = temp.path().join("generated");
    let tests_dir = src_dir.join("tests");

    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&tests_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    // Main file
    let main_code = r#"
use serde::Serialize;

#[derive(Serialize)]
pub struct User {
    pub id: i32,
}

#[tauri::command]
pub fn get_user(id: i32) -> User {
    unimplemented!()
}
"#;
    fs::write(src_dir.join("main.rs"), main_code).unwrap();

    // Test file (should be excluded)
    let test_code = r#"
use serde::Serialize;

#[derive(Serialize)]
pub struct TestType {
    pub data: String,
}

#[tauri::command]
pub fn test_command() -> TestType {
    unimplemented!()
}
"#;
    fs::write(tests_dir.join("test.rs"), test_code).unwrap();

    let config = create_test_config(src_dir, output_dir.clone());
    let pipeline = Pipeline::new(false);

    let result = pipeline.run(&config);
    assert!(result.is_ok());

    // Verify User is included but TestType is not
    let types_content = fs::read_to_string(output_dir.join("types.ts")).unwrap();
    assert!(types_content.contains("export interface User"));
    assert!(!types_content.contains("TestType"));

    // Verify get_user is included but test_command is not
    let commands_content = fs::read_to_string(output_dir.join("commands.ts")).unwrap();
    assert!(commands_content.contains("export async function getUser"));
    assert!(!commands_content.contains("testCommand"));
}

#[test]
fn test_pipeline_with_naming_config() {
    let temp = tempdir().unwrap();
    let src_dir = temp.path().join("src");
    let output_dir = temp.path().join("generated");

    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    let code = r#"
use serde::Serialize;

#[derive(Serialize)]
pub struct User {
    pub id: i32,
}

#[tauri::command]
pub fn get_user(id: i32) -> User {
    unimplemented!()
}
"#;
    fs::write(src_dir.join("lib.rs"), code).unwrap();

    let config = Config {
        input: InputConfig {
            source_dir: src_dir,
            exclude: vec![],
            use_cargo_expand: false,
            cargo_manifest: None,
        },
        output: OutputConfig {
            types_file: output_dir.join("types.ts"),
            commands_file: output_dir.join("commands.ts"),
        },
        naming: NamingConfig {
            type_prefix: "I".to_string(),
            type_suffix: "".to_string(),
            function_prefix: "".to_string(),
            function_suffix: "Cmd".to_string(),
            preserve_field_names: false,
        },
    };

    let pipeline = Pipeline::new(false);
    let result = pipeline.run(&config);
    assert!(result.is_ok());

    let types_content = fs::read_to_string(output_dir.join("types.ts")).unwrap();
    assert!(types_content.contains("export interface IUser"));

    let commands_content = fs::read_to_string(output_dir.join("commands.ts")).unwrap();
    assert!(commands_content.contains("export async function getUserCmd"));
}

#[test]
fn test_pipeline_empty_source() {
    let temp = tempdir().unwrap();
    let src_dir = temp.path().join("src");
    let output_dir = temp.path().join("generated");

    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    // No Rust files

    let config = create_test_config(src_dir, output_dir.clone());
    let pipeline = Pipeline::new(false);

    let result = pipeline.run(&config);
    assert!(result.is_ok());

    // Files should still be generated (though empty)
    assert!(output_dir.join("types.ts").exists());
    assert!(output_dir.join("commands.ts").exists());
}

#[test]
fn test_pipeline_creates_output_directories() {
    let temp = tempdir().unwrap();
    let src_dir = temp.path().join("src");
    let output_dir = temp.path().join("deeply").join("nested").join("output");

    fs::create_dir_all(&src_dir).unwrap();
    // Don't create output_dir - pipeline should create it

    let code = r#"
use serde::Serialize;

#[derive(Serialize)]
pub struct User {
    pub id: i32,
}

#[tauri::command]
pub fn greet() -> User {
    unimplemented!()
}
"#;
    fs::write(src_dir.join("lib.rs"), code).unwrap();

    let config = create_test_config(src_dir, output_dir.clone());
    let pipeline = Pipeline::new(false);

    let result = pipeline.run(&config);
    assert!(result.is_ok());

    assert!(output_dir.join("types.ts").exists());
    assert!(output_dir.join("commands.ts").exists());
}

#[test]
fn test_pipeline_with_enums() {
    let temp = tempdir().unwrap();
    let src_dir = temp.path().join("src");
    let output_dir = temp.path().join("generated");

    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    let code = r#"
use serde::Serialize;

#[derive(Serialize)]
pub enum Status {
    Active,
    Inactive,
    Pending,
}

#[tauri::command]
pub fn get_status() -> Status {
    Status::Active
}
"#;
    fs::write(src_dir.join("lib.rs"), code).unwrap();

    let config = create_test_config(src_dir, output_dir.clone());
    let pipeline = Pipeline::new(false);

    let result = pipeline.run(&config);
    assert!(result.is_ok());

    let types_content = fs::read_to_string(output_dir.join("types.ts")).unwrap();
    assert!(types_content.contains("export type Status"));
    assert!(types_content.contains("\"Active\""));
    assert!(types_content.contains("\"Inactive\""));
    assert!(types_content.contains("\"Pending\""));
}

#[test]
fn test_pipeline_with_complex_return_types() {
    let temp = tempdir().unwrap();
    let src_dir = temp.path().join("src");
    let output_dir = temp.path().join("generated");

    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    let code = r#"
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
pub struct User {
    pub id: i32,
}

#[tauri::command]
pub fn get_users() -> Vec<User> {
    vec![]
}

#[tauri::command]
pub fn get_optional_user(id: i32) -> Option<User> {
    None
}

#[tauri::command]
pub fn get_user_result(id: i32) -> Result<User, String> {
    unimplemented!()
}

#[tauri::command]
pub fn get_user_map() -> HashMap<String, User> {
    HashMap::new()
}
"#;
    fs::write(src_dir.join("lib.rs"), code).unwrap();

    let config = create_test_config(src_dir, output_dir.clone());
    let pipeline = Pipeline::new(false);

    let result = pipeline.run(&config);
    assert!(result.is_ok());

    let commands_content = fs::read_to_string(output_dir.join("commands.ts")).unwrap();
    assert!(commands_content.contains("Promise<User[]>"));
    assert!(commands_content.contains("Promise<User | null>"));
    assert!(commands_content.contains("Promise<User>"));
    assert!(commands_content.contains("Promise<Record<string, User>>"));
}

#[test]
fn test_pipeline_verbose_mode() {
    let temp = tempdir().unwrap();
    let src_dir = temp.path().join("src");
    let output_dir = temp.path().join("generated");

    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    let code = r#"
use serde::Serialize;

#[derive(Serialize)]
pub struct User {
    pub id: i32,
}

#[tauri::command]
pub fn greet() -> User {
    unimplemented!()
}
"#;
    fs::write(src_dir.join("lib.rs"), code).unwrap();

    let config = create_test_config(src_dir, output_dir.clone());

    // Run with verbose mode - should not panic
    let pipeline = Pipeline::new(true);
    let result = pipeline.run(&config);
    assert!(result.is_ok());
}

#[test]
fn test_pipeline_filters_only_used_types() {
    let temp = tempdir().unwrap();
    let src_dir = temp.path().join("src");
    let output_dir = temp.path().join("generated");

    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    let code = r#"
use serde::Serialize;

#[derive(Serialize)]
pub struct UsedType {
    pub id: i32,
}

#[derive(Serialize)]
pub struct UnusedType {
    pub name: String,
}

#[tauri::command]
pub fn get_used() -> UsedType {
    unimplemented!()
}
"#;
    fs::write(src_dir.join("lib.rs"), code).unwrap();

    let config = create_test_config(src_dir, output_dir.clone());
    let pipeline = Pipeline::new(false);

    let result = pipeline.run(&config);
    assert!(result.is_ok());

    let types_content = fs::read_to_string(output_dir.join("types.ts")).unwrap();

    // UsedType should be included
    assert!(types_content.contains("export interface UsedType"));

    // UnusedType should NOT be included (not referenced by any command)
    assert!(!types_content.contains("UnusedType"));
}

#[test]
fn test_wildcard_reexport_from_submodule() {
    let temp = tempdir().unwrap();
    let src_dir = temp.path().join("src");
    let resources_dir = src_dir.join("resources");
    let output_dir = temp.path().join("generated");

    fs::create_dir_all(&resources_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    // resources/types.rs - actual type definitions
    let types_code = r#"
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct PodInfo {
    pub name: String,
    pub namespace: String,
    pub status: String,
}

#[derive(Serialize, Deserialize)]
pub struct ContainerInfo {
    pub name: String,
    pub image: String,
}
"#;
    fs::write(resources_dir.join("types.rs"), types_code).unwrap();

    // resources/mod.rs - wildcard re-export
    let mod_code = r#"
mod types;

pub use types::*;
"#;
    fs::write(resources_dir.join("mod.rs"), mod_code).unwrap();

    // commands.rs - uses types via re-export
    let commands_code = r#"
use crate::resources::PodInfo;

#[tauri::command]
pub fn list_pods(namespace: Option<String>) -> Vec<PodInfo> {
    vec![]
}

#[tauri::command]
pub fn get_pod(name: String) -> PodInfo {
    unimplemented!()
}
"#;
    fs::write(src_dir.join("commands.rs"), commands_code).unwrap();

    let config = create_test_config(src_dir, output_dir.clone());
    let pipeline = Pipeline::new(false);

    let result = pipeline.run(&config);
    assert!(result.is_ok(), "Pipeline should succeed: {:?}", result.err());

    // Verify PodInfo is generated
    let types_content = fs::read_to_string(output_dir.join("types.ts")).unwrap();
    assert!(types_content.contains("export interface PodInfo"), 
            "PodInfo should be in types.ts. Content:\n{}", types_content);
    assert!(types_content.contains("name: string"));
    assert!(types_content.contains("namespace: string"));
    assert!(types_content.contains("status: string"));

    // ContainerInfo should NOT be included (not used by any command)
    assert!(!types_content.contains("ContainerInfo"), 
            "ContainerInfo should not be in types.ts as it's unused");

    // Verify commands
    let commands_content = fs::read_to_string(output_dir.join("commands.ts")).unwrap();
    assert!(commands_content.contains("export async function listPods"));
    assert!(commands_content.contains("export async function getPod"));
    assert!(commands_content.contains("Promise<PodInfo[]>"));
    assert!(commands_content.contains("import type { PodInfo }"));
}

#[test]
fn test_relative_wildcard_path() {
    let temp = tempdir().unwrap();
    let src_dir = temp.path().join("src");
    let inner_dir = src_dir.join("inner");
    let output_dir = temp.path().join("generated");

    fs::create_dir_all(&inner_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    // inner/types.rs
    let types_code = r#"
use serde::Serialize;

#[derive(Serialize)]
pub struct InnerType {
    pub value: i32,
}
"#;
    fs::write(inner_dir.join("types.rs"), types_code).unwrap();

    // inner/mod.rs with relative wildcard import
    let mod_code = r#"
mod types;
pub use types::*;
"#;
    fs::write(inner_dir.join("mod.rs"), mod_code).unwrap();

    // lib.rs
    let lib_code = r#"
use crate::inner::InnerType;

#[tauri::command]
pub fn get_inner() -> InnerType {
    unimplemented!()
}
"#;
    fs::write(src_dir.join("lib.rs"), lib_code).unwrap();

    let config = create_test_config(src_dir, output_dir.clone());
    let pipeline = Pipeline::new(false);

    let result = pipeline.run(&config);
    assert!(result.is_ok(), "Pipeline should succeed: {:?}", result.err());

    let types_content = fs::read_to_string(output_dir.join("types.ts")).unwrap();
    assert!(types_content.contains("export interface InnerType"),
            "InnerType should be generated. Content:\n{}", types_content);
}

#[test]
fn test_nested_wildcard_reexport() {
    let temp = tempdir().unwrap();
    let src_dir = temp.path().join("src");
    let a_dir = src_dir.join("a");
    let b_dir = a_dir.join("b");
    let output_dir = temp.path().join("generated");

    fs::create_dir_all(&b_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    // a/b/types.rs - deepest level
    let deep_types_code = r#"
use serde::Serialize;

#[derive(Serialize)]
pub struct DeepType {
    pub depth: i32,
}
"#;
    fs::write(b_dir.join("types.rs"), deep_types_code).unwrap();

    // a/b/mod.rs
    let b_mod_code = r#"
mod types;
pub use types::*;
"#;
    fs::write(b_dir.join("mod.rs"), b_mod_code).unwrap();

    // a/mod.rs - re-exports from b
    let a_mod_code = r#"
pub mod b;
pub use b::*;
"#;
    fs::write(a_dir.join("mod.rs"), a_mod_code).unwrap();

    // lib.rs - uses type from nested module
    let lib_code = r#"
use crate::a::b::DeepType;

#[tauri::command]
pub fn get_deep() -> DeepType {
    unimplemented!()
}
"#;
    fs::write(src_dir.join("lib.rs"), lib_code).unwrap();

    let config = create_test_config(src_dir, output_dir.clone());
    let pipeline = Pipeline::new(false);

    let result = pipeline.run(&config);
    assert!(result.is_ok(), "Pipeline should succeed: {:?}", result.err());

    let types_content = fs::read_to_string(output_dir.join("types.ts")).unwrap();
    assert!(types_content.contains("export interface DeepType"),
            "DeepType should be generated. Content:\n{}", types_content);
}

#[test]
fn test_mixed_explicit_and_wildcard_imports() {
    let temp = tempdir().unwrap();
    let src_dir = temp.path().join("src");
    let types_dir = src_dir.join("types");
    let output_dir = temp.path().join("generated");

    fs::create_dir_all(&types_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    // types/user.rs
    let user_code = r#"
use serde::Serialize;

#[derive(Serialize)]
pub struct User {
    pub id: i32,
    pub name: String,
}
"#;
    fs::write(types_dir.join("user.rs"), user_code).unwrap();

    // types/item.rs
    let item_code = r#"
use serde::Serialize;

#[derive(Serialize)]
pub struct Item {
    pub id: i32,
    pub title: String,
}
"#;
    fs::write(types_dir.join("item.rs"), item_code).unwrap();

    // types/mod.rs - mixed explicit and wildcard
    let mod_code = r#"
mod user;
mod item;

pub use user::User;
pub use item::*;
"#;
    fs::write(types_dir.join("mod.rs"), mod_code).unwrap();

    // commands.rs
    let commands_code = r#"
use crate::types::{User, Item};

#[tauri::command]
pub fn get_user(id: i32) -> User {
    unimplemented!()
}

#[tauri::command]
pub fn get_item(id: i32) -> Item {
    unimplemented!()
}
"#;
    fs::write(src_dir.join("commands.rs"), commands_code).unwrap();

    let config = create_test_config(src_dir, output_dir.clone());
    let pipeline = Pipeline::new(false);

    let result = pipeline.run(&config);
    assert!(result.is_ok(), "Pipeline should succeed: {:?}", result.err());

    let types_content = fs::read_to_string(output_dir.join("types.ts")).unwrap();
    assert!(types_content.contains("export interface User"),
            "User should be generated. Content:\n{}", types_content);
    assert!(types_content.contains("export interface Item"),
            "Item should be generated. Content:\n{}", types_content);

    let commands_content = fs::read_to_string(output_dir.join("commands.ts")).unwrap();
    assert!(commands_content.contains("import type { User, Item }") || 
            commands_content.contains("import type { Item, User }"),
            "Both User and Item should be imported. Content:\n{}", commands_content);
}

#[test]
fn test_wildcard_with_enum() {
    let temp = tempdir().unwrap();
    let src_dir = temp.path().join("src");
    let models_dir = src_dir.join("models");
    let output_dir = temp.path().join("generated");

    fs::create_dir_all(&models_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    // models/types.rs
    let types_code = r#"
use serde::Serialize;

#[derive(Serialize)]
pub struct Resource {
    pub name: String,
    pub status: ResourceStatus,
}

#[derive(Serialize)]
pub enum ResourceStatus {
    Running,
    Pending,
    Failed,
}
"#;
    fs::write(models_dir.join("types.rs"), types_code).unwrap();

    // models/mod.rs
    let mod_code = r#"
mod types;
pub use types::*;
"#;
    fs::write(models_dir.join("mod.rs"), mod_code).unwrap();

    // lib.rs
    let lib_code = r#"
use crate::models::Resource;

#[tauri::command]
pub fn get_resource(name: String) -> Resource {
    unimplemented!()
}
"#;
    fs::write(src_dir.join("lib.rs"), lib_code).unwrap();

    let config = create_test_config(src_dir, output_dir.clone());
    let pipeline = Pipeline::new(false);

    let result = pipeline.run(&config);
    assert!(result.is_ok(), "Pipeline should succeed: {:?}", result.err());

    let types_content = fs::read_to_string(output_dir.join("types.ts")).unwrap();
    assert!(types_content.contains("export interface Resource"),
            "Resource should be generated. Content:\n{}", types_content);
    assert!(types_content.contains("export type ResourceStatus"),
            "ResourceStatus enum should be generated. Content:\n{}", types_content);
    assert!(types_content.contains("\"Running\""));
    assert!(types_content.contains("\"Pending\""));
    assert!(types_content.contains("\"Failed\""));
}


#[test]
fn test_parse_expanded_real_progenitor_output() {
    use tauri_ts_generator::parser::parse_types_expanded;
    use std::path::PathBuf;

    // This is actual cargo expand output from progenitor
    let code = r#"
        pub mod auth {
            pub mod generated_client {
                pub mod types {
                    pub struct AuthResponse {
                        #[serde(rename = "accessToken")]
                        pub access_token: ::std::string::String,
                        #[serde(rename = "expiresIn")]
                        pub expires_in: i64,
                        #[serde(rename = "refreshToken")]
                        pub refresh_token: ::std::string::String,
                        #[serde(rename = "tokenType")]
                        pub token_type: ::std::string::String,
                    }
                    pub struct UserProfile {
                        pub email: String,
                        #[serde(rename = "firstName")]
                        pub first_name: String,
                    }
                }
            }
        }
    "#;

    let (structs, _) = parse_types_expanded(code, &PathBuf::from("<test>")).unwrap();

    // Should find both AuthResponse and UserProfile through nested modules
    let names: Vec<&str> = structs.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"AuthResponse"), "Should find AuthResponse, got {:?}", names);
    assert!(names.contains(&"UserProfile"), "Should find UserProfile, got {:?}", names);
}

/// Test that cross-file type usage doesn't cause false ambiguous warnings.
///
/// This test reproduces the bug where a type defined in types.rs and used as a field
/// in workloads.rs would incorrectly trigger "Ambiguous type" warnings because
/// cargo-expand also registered the type. The fix ensures types are only registered
/// from their actual source files.
#[test]
fn test_cross_file_type_no_ambiguous_warning() {
    let temp = tempdir().unwrap();
    let src_dir = temp.path().join("src");
    let resources_dir = src_dir.join("resources");
    let output_dir = temp.path().join("generated");

    fs::create_dir_all(&resources_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    // resources/types.rs - defines the shared type
    let types_code = r#"
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct DeploymentContainerInfo {
    pub name: String,
    pub image: String,
    pub ready: bool,
}

#[derive(Serialize, Deserialize)]
pub struct PodCondition {
    pub condition_type: String,
    pub status: String,
}
"#;
    fs::write(resources_dir.join("types.rs"), types_code).unwrap();

    // resources/workloads.rs - uses types from types.rs as fields
    let workloads_code = r#"
use serde::{Serialize, Deserialize};
use super::types::{DeploymentContainerInfo, PodCondition};

#[derive(Serialize, Deserialize)]
pub struct StatefulSetDetailInfo {
    pub name: String,
    pub namespace: String,
    pub containers: Vec<DeploymentContainerInfo>,
    pub conditions: Vec<PodCondition>,
}

#[derive(Serialize, Deserialize)]
pub struct DeploymentInfo {
    pub name: String,
    pub replicas: i32,
    pub containers: Vec<DeploymentContainerInfo>,
}
"#;
    fs::write(resources_dir.join("workloads.rs"), workloads_code).unwrap();

    // resources/mod.rs - re-exports
    let mod_code = r#"
pub mod types;
pub mod workloads;

pub use types::*;
pub use workloads::*;
"#;
    fs::write(resources_dir.join("mod.rs"), mod_code).unwrap();

    // commands.rs - uses the composite types
    let commands_code = r#"
use crate::resources::{StatefulSetDetailInfo, DeploymentInfo};

#[tauri::command]
pub fn get_statefulset(name: String, namespace: String) -> StatefulSetDetailInfo {
    unimplemented!()
}

#[tauri::command]
pub fn list_deployments(namespace: String) -> Vec<DeploymentInfo> {
    vec![]
}
"#;
    fs::write(src_dir.join("commands.rs"), commands_code).unwrap();

    let config = create_test_config(src_dir, output_dir.clone());
    let pipeline = Pipeline::new(false);

    // The pipeline should succeed without any ambiguity issues
    let result = pipeline.run(&config);
    assert!(result.is_ok(), "Pipeline should succeed: {:?}", result.err());

    // Verify all types are generated correctly
    let types_content = fs::read_to_string(output_dir.join("types.ts")).unwrap();

    // Main types should be present
    assert!(types_content.contains("export interface StatefulSetDetailInfo"),
            "StatefulSetDetailInfo should be generated. Content:\n{}", types_content);
    assert!(types_content.contains("export interface DeploymentInfo"),
            "DeploymentInfo should be generated. Content:\n{}", types_content);

    // Nested types should also be present (they're used as fields)
    assert!(types_content.contains("export interface DeploymentContainerInfo"),
            "DeploymentContainerInfo should be generated. Content:\n{}", types_content);
    assert!(types_content.contains("export interface PodCondition"),
            "PodCondition should be generated. Content:\n{}", types_content);

    // Verify field references are correct
    assert!(types_content.contains("containers: DeploymentContainerInfo[]"),
            "containers field should reference DeploymentContainerInfo. Content:\n{}", types_content);
    assert!(types_content.contains("conditions: PodCondition[]"),
            "conditions field should reference PodCondition. Content:\n{}", types_content);

    // Verify commands are generated
    let commands_content = fs::read_to_string(output_dir.join("commands.ts")).unwrap();
    assert!(commands_content.contains("export async function getStatefulset"),
            "getStatefulset command should be generated");
    assert!(commands_content.contains("export async function listDeployments"),
            "listDeployments command should be generated");
}

/// Test that types genuinely defined in multiple files still trigger proper handling.
/// This ensures the fix didn't break legitimate ambiguity detection.
#[test]
fn test_same_type_name_in_different_modules() {
    let temp = tempdir().unwrap();
    let src_dir = temp.path().join("src");
    let v1_dir = src_dir.join("v1");
    let v2_dir = src_dir.join("v2");
    let output_dir = temp.path().join("generated");

    fs::create_dir_all(&v1_dir).unwrap();
    fs::create_dir_all(&v2_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    // v1/types.rs - version 1 of User
    let v1_code = r#"
use serde::Serialize;

#[derive(Serialize)]
pub struct User {
    pub id: i32,
    pub name: String,
}
"#;
    fs::write(v1_dir.join("types.rs"), v1_code).unwrap();

    // v2/types.rs - version 2 of User (different structure)
    let v2_code = r#"
use serde::Serialize;

#[derive(Serialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: String,
}
"#;
    fs::write(v2_dir.join("types.rs"), v2_code).unwrap();

    // commands.rs - explicitly imports from v1
    let commands_code = r#"
use crate::v1::types::User;

#[tauri::command]
pub fn get_user_v1(id: i32) -> User {
    unimplemented!()
}
"#;
    fs::write(src_dir.join("commands.rs"), commands_code).unwrap();

    let config = create_test_config(src_dir, output_dir.clone());
    let pipeline = Pipeline::new(false);

    // Pipeline should succeed - the explicit import should resolve correctly
    let result = pipeline.run(&config);
    assert!(result.is_ok(), "Pipeline should succeed with explicit imports: {:?}", result.err());

    // Verify v1 User is generated (has id: number)
    let types_content = fs::read_to_string(output_dir.join("types.ts")).unwrap();
    assert!(types_content.contains("export interface User"),
            "User should be generated. Content:\n{}", types_content);
    assert!(types_content.contains("id: number"),
            "User should have id: number (v1 version). Content:\n{}", types_content);
}

/// Test that simulates the cargo-expand scenario that caused the original bug.
///
/// This test directly uses parse_types_expanded to parse "cargo expand" output
/// alongside regular source file parsing, verifying that:
/// 1. Types from cargo-expand are parsed correctly
/// 2. Types from source files are also parsed
/// 3. The resolver only registers types from source files (not cargo-expand)
/// 4. No false ambiguity warnings occur
#[test]
fn test_cargo_expand_does_not_cause_ambiguity() {
    use tauri_ts_generator::parser::parse_types_expanded;
    use tauri_ts_generator::resolver::ModuleResolver;
    use std::path::PathBuf;

    // Simulate cargo-expand output containing DeploymentContainerInfo
    // This is what cargo expand would produce for a struct with #[derive(Serialize)]
    let cargo_expand_output = r#"
        pub mod resources {
            pub mod types {
                pub struct DeploymentContainerInfo {
                    pub name: String,
                    pub image: String,
                }
                #[automatically_derived]
                impl ::serde::Serialize for DeploymentContainerInfo {
                    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                    where
                        S: ::serde::Serializer,
                    {
                        unimplemented!()
                    }
                }
            }
            pub mod workloads {
                use super::types::DeploymentContainerInfo;

                pub struct StatefulSetDetailInfo {
                    pub name: String,
                    pub containers: Vec<DeploymentContainerInfo>,
                }
                #[automatically_derived]
                impl ::serde::Serialize for StatefulSetDetailInfo {
                    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                    where
                        S: ::serde::Serializer,
                    {
                        unimplemented!()
                    }
                }
            }
        }
    "#;

    // Parse cargo-expand output
    let expanded_path = PathBuf::from("<cargo-expand>");
    let (expanded_structs, _) = parse_types_expanded(cargo_expand_output, &expanded_path).unwrap();

    // Verify we found the types in cargo-expand output
    let expanded_names: Vec<&str> = expanded_structs.iter().map(|s| s.name.as_str()).collect();
    assert!(expanded_names.contains(&"DeploymentContainerInfo"),
            "Should find DeploymentContainerInfo in expanded output");
    assert!(expanded_names.contains(&"StatefulSetDetailInfo"),
            "Should find StatefulSetDetailInfo in expanded output");

    // Now create a resolver and parse the "actual" source files
    let mut resolver = ModuleResolver::new();
    let base_path = PathBuf::from("src");

    // Simulate parsing src/resources/types.rs
    let types_source = r#"
        use serde::Serialize;

        #[derive(Serialize)]
        pub struct DeploymentContainerInfo {
            pub name: String,
            pub image: String,
        }
    "#;
    let types_path = PathBuf::from("src/resources/types.rs");
    resolver.parse_file(&types_path, types_source, &base_path).unwrap();

    // Simulate parsing src/resources/workloads.rs
    let workloads_source = r#"
        use serde::Serialize;
        use super::types::DeploymentContainerInfo;

        #[derive(Serialize)]
        pub struct StatefulSetDetailInfo {
            pub name: String,
            pub containers: Vec<DeploymentContainerInfo>,
        }
    "#;
    let workloads_path = PathBuf::from("src/resources/workloads.rs");
    resolver.parse_file(&workloads_path, workloads_source, &base_path).unwrap();

    // KEY ASSERTION: DeploymentContainerInfo should only be registered once
    // (from types.rs), NOT twice (types.rs + <cargo-expand>)
    //
    // BEFORE THE FIX: pipeline.rs would call register_expanded_type() for each
    // type found in cargo-expand, causing duplicates.
    //
    // AFTER THE FIX: register_expanded_type() was removed, so only source files
    // register types.
    let locations = resolver.type_definitions.get("DeploymentContainerInfo").unwrap();
    assert_eq!(
        locations.len(), 1,
        "DeploymentContainerInfo should be registered only once (from source file), not from cargo-expand. Found: {:?}",
        locations
    );
    assert_eq!(locations[0], types_path);

    // Resolving the type from workloads.rs should work without ambiguity
    use tauri_ts_generator::resolver::ResolutionResult;
    match resolver.resolve_type("DeploymentContainerInfo", &workloads_path) {
        ResolutionResult::Found(p) => {
            assert_eq!(p, types_path, "Should resolve to types.rs");
        }
        ResolutionResult::Ambiguous(paths) => {
            panic!(
                "BUG REPRODUCED: Type is ambiguous when it shouldn't be! \
                This was the original cargo-expand bug. Found in: {:?}",
                paths
            );
        }
        res => panic!("Expected Found, got {:?}", res),
    }
}
