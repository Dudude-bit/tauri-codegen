//! Compile-time tests for the TS derive macro
//!
//! These tests verify that the `#[derive(TS)]` macro allows the `#[ts(...)]`
//! attribute to compile without errors.

#![allow(dead_code)]

use tauri_ts_generator_derive::TS;

/// Test: Basic derive on empty struct compiles
#[derive(TS)]
struct EmptyStruct;

/// Test: Derive with #[ts(optional)] on Option field compiles
#[derive(TS)]
struct ConfigWithOptional {
    #[ts(optional)]
    volume: Option<f32>,
    name: String,
}

/// Test: Derive with multiple #[ts(optional)] attributes compiles
#[derive(TS)]
struct MultipleOptionals {
    #[ts(optional)]
    field_a: Option<String>,
    #[ts(optional)]
    field_b: Option<i32>,
    normal_field: String,
}

/// Test: Derive on struct with generics compiles
#[derive(TS)]
struct GenericStruct<T> {
    #[ts(optional)]
    data: Option<T>,
}

/// Test: Derive on enum compiles
#[derive(TS)]
enum Status {
    Active,
    Inactive,
}

/// Test: Derive on enum with struct variants and #[ts(optional)] compiles
#[derive(TS)]
enum Event {
    Click {
        x: i32,
        y: i32,
    },
    Config {
        #[ts(optional)]
        volume: Option<f32>,
    },
}

/// Test: Derive with other derive macros (like Debug) compiles
#[derive(Debug, Clone, TS)]
struct WithOtherDerives {
    #[ts(optional)]
    value: Option<String>,
}

/// Test: Derive on tuple struct compiles
#[derive(TS)]
struct TupleStruct(String, i32);

/// Test: Complex nested types with #[ts(optional)] compile
#[derive(TS)]
struct ComplexTypes {
    #[ts(optional)]
    nested_option: Option<Option<String>>,
    #[ts(optional)]
    vec_option: Option<Vec<i32>>,
}

#[test]
fn test_derive_compiles() {
    // This test just needs to compile - if it compiles, the derive macro works
    let _ = EmptyStruct;
    let _ = ConfigWithOptional {
        volume: Some(0.5),
        name: "test".to_string(),
    };
    let _ = MultipleOptionals {
        field_a: None,
        field_b: Some(42),
        normal_field: "test".to_string(),
    };
    let _ = GenericStruct::<String> {
        data: Some("hello".to_string()),
    };
    let _ = Status::Active;
    let _ = Event::Click { x: 10, y: 20 };
    let _ = Event::Config { volume: Some(1.0) };
    let _ = WithOtherDerives { value: None };
    let _ = TupleStruct("test".to_string(), 42);
    let _ = ComplexTypes {
        nested_option: None,
        vec_option: Some(vec![1, 2, 3]),
    };
}

#[test]
fn test_derive_with_serde_like_usage() {
    // Simulating how users would typically use the derive
    #[derive(Debug, TS)]
    #[allow(dead_code)]
    struct PlaySoundRequest {
        #[ts(optional)]
        volume_normalized: Option<f32>,
        volume_absolute: Option<f32>,
    }

    let request = PlaySoundRequest {
        volume_normalized: Some(0.8),
        volume_absolute: None,
    };

    // If this compiles and runs, the derive macro is working correctly
    assert!(request.volume_normalized.is_some());
    assert!(request.volume_absolute.is_none());
}
