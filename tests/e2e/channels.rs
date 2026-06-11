//! E2e tests for tauri::ipc::Channel support.

use crate::helpers::{run_generate_ok, Project};

#[test]
fn channel_arg_generates_channel_import_and_type() {
    let project = Project::with_source(
        r#"
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize)]
        pub enum DownloadEvent {
            Progress { percent: u32 },
            Done,
        }

        #[tauri::command]
        fn download(on_event: tauri::ipc::Channel<DownloadEvent>) {}
        "#,
    );

    run_generate_ok(&project);
    let commands = std::fs::read_to_string(&project.commands_out).unwrap();

    assert!(
        commands.contains("import { invoke, Channel }"),
        "Channel must be imported from @tauri-apps/api/core:\n{commands}"
    );
    assert!(
        commands.contains("onEvent: Channel<DownloadEvent>"),
        "parameter must be typed as Channel<DownloadEvent>:\n{commands}"
    );
    assert!(
        commands.contains("{ onEvent }"),
        "channel arg must be forwarded to invoke:\n{commands}"
    );
    assert!(
        commands.contains("import type { DownloadEvent }"),
        "DownloadEvent must be imported from types file:\n{commands}"
    );
}

#[test]
fn channel_generates_channel_type_alias_in_types_file() {
    let project = Project::with_source(
        r#"
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize)]
        pub enum DownloadEvent { Progress { percent: u32 }, Done }

        #[tauri::command]
        fn download(on_event: tauri::ipc::Channel<DownloadEvent>) {}
        "#,
    );

    run_generate_ok(&project);
    let types = std::fs::read_to_string(&project.types_out).unwrap();

    assert!(
        types.contains("export type DownloadOnEventChannelType = DownloadEvent;"),
        "channel type alias must appear in types.ts:\n{types}"
    );
}

#[test]
fn channel_without_custom_inner_type() {
    // Channel<String> — inner type is primitive, no type import needed
    let project = Project::with_source(
        r#"
        #[tauri::command]
        fn stream(on_data: tauri::ipc::Channel<String>) {}
        "#,
    );

    run_generate_ok(&project);
    let commands = std::fs::read_to_string(&project.commands_out).unwrap();

    assert!(
        commands.contains("import { invoke, Channel }"),
        "Channel import must be present:\n{commands}"
    );
    assert!(
        commands.contains("onData: Channel<string>"),
        "primitive inner type maps to string:\n{commands}"
    );
    assert!(
        !commands.contains("import type {"),
        "no type import expected for primitive channel:\n{commands}"
    );

    let types = std::fs::read_to_string(&project.types_out).unwrap();
    assert!(
        types.contains("export type StreamOnDataChannelType = string;"),
        "primitive channel type alias must be string:\n{types}"
    );
}

#[test]
fn command_without_channel_omits_channel_import() {
    let project = Project::with_source(
        r#"
        #[tauri::command]
        fn greet(name: String) -> String { name }
        "#,
    );

    run_generate_ok(&project);
    let commands = std::fs::read_to_string(&project.commands_out).unwrap();

    assert!(
        !commands.contains("Channel"),
        "Channel must not appear in commands without a channel arg:\n{commands}"
    );
    assert!(
        commands.contains("import { invoke }"),
        "plain invoke import must be present:\n{commands}"
    );

    let types = std::fs::read_to_string(&project.types_out).unwrap();
    assert!(
        !types.contains("ChannelType"),
        "no channel type alias in types.ts without channel commands:\n{types}"
    );
}

#[test]
fn channel_and_regular_args_together() {
    let project = Project::with_source(
        r#"
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize)]
        pub struct Progress { pub bytes: u64 }

        #[tauri::command]
        fn upload(url: String, on_progress: tauri::ipc::Channel<Progress>) {}
        "#,
    );

    run_generate_ok(&project);
    let commands = std::fs::read_to_string(&project.commands_out).unwrap();

    assert!(commands.contains("url: string"), "{commands}");
    assert!(
        commands.contains("onProgress: Channel<Progress>"),
        "{commands}"
    );
    assert!(
        commands.contains("{ url, onProgress }"),
        "both args forwarded to invoke:\n{commands}"
    );

    let types = std::fs::read_to_string(&project.types_out).unwrap();
    assert!(
        types.contains("export type UploadOnProgressChannelType = Progress;"),
        "{types}"
    );
}

#[test]
fn multiple_channel_args_disambiguated_by_arg_name() {
    let project = Project::with_source(
        r#"
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize)]
        pub struct Progress { pub bytes: u64 }

        #[derive(Serialize, Deserialize)]
        pub struct Done { pub path: String }

        #[tauri::command]
        fn upload(
            on_progress: tauri::ipc::Channel<Progress>,
            on_done: tauri::ipc::Channel<Done>,
        ) {}
        "#,
    );

    run_generate_ok(&project);
    let types = std::fs::read_to_string(&project.types_out).unwrap();

    assert!(
        types.contains("export type UploadOnProgressChannelType = Progress;"),
        "must disambiguate with arg name:\n{types}"
    );
    assert!(
        types.contains("export type UploadOnDoneChannelType = Done;"),
        "must disambiguate with arg name:\n{types}"
    );
}

#[test]
fn bare_channel_type_without_path_prefix() {
    // Channel<T> used without the tauri::ipc:: prefix (via use tauri::ipc::Channel)
    let project = Project::with_source(
        r#"
        use tauri::ipc::Channel;
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize)]
        pub struct Msg { pub text: String }

        #[tauri::command]
        fn listen(on_msg: Channel<Msg>) {}
        "#,
    );

    run_generate_ok(&project);
    let commands = std::fs::read_to_string(&project.commands_out).unwrap();

    assert!(
        commands.contains("import { invoke, Channel }"),
        "{commands}"
    );
    assert!(commands.contains("onMsg: Channel<Msg>"), "{commands}");

    let types = std::fs::read_to_string(&project.types_out).unwrap();
    assert!(
        types.contains("export type ListenOnMsgChannelType = Msg;"),
        "{types}"
    );
}
