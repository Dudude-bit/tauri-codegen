use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i32,
    pub name: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub name: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserStatus {
    Active,
    Inactive,
    Pending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserRole {
    Admin { permissions: Vec<String> },
    User,
    Guest,
}

#[tauri::command]
pub async fn get_user(id: i32) -> Result<User, String> {
    Ok(User {
        id,
        name: "John Doe".to_string(),
        email: Some("john@example.com".to_string()),
    })
}

#[tauri::command]
pub async fn get_all_users() -> Result<Vec<User>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn create_user(request: CreateUserRequest) -> Result<User, String> {
    Ok(User {
        id: 1,
        name: request.name,
        email: request.email,
    })
}

#[tauri::command]
pub fn delete_user(user_id: i32) -> Result<bool, String> {
    Ok(true)
}

#[tauri::command]
pub async fn get_user_status(user_id: i32) -> Result<UserStatus, String> {
    Ok(UserStatus::Active)
}

#[tauri::command]
pub fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}

#[tauri::command]
pub async fn get_settings() -> Result<std::collections::HashMap<String, String>, String> {
    Ok(std::collections::HashMap::new())
}

// Generic paginated list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceList<T> {
    pub items: Vec<T>,
    pub total: u32,
    pub page: u32,
    pub per_page: u32,
    pub continue_token: Option<String>,
}

// Example with DateTime and other external types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: i32,
    pub title: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub date: chrono::NaiveDate,
    pub uuid: uuid::Uuid,
    pub path: std::path::PathBuf,
}

#[tauri::command]
pub async fn get_event(id: i32) -> Result<Event, String> {
    unimplemented!()
}

#[tauri::command]
pub async fn get_events_by_date(date: chrono::NaiveDate) -> Result<Vec<Event>, String> {
    unimplemented!()
}

