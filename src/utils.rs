//! Identifier and path helpers shared across parsing and rendering.
//!
//! Case conversions match serde's `rename_all` semantics (see `split_words`
//! below). `simple_name` strips Rust module paths down to the final
//! identifier — TypeScript has no concept of `crate::module::Type`, so every
//! place that renders a Rust type reference to TS must call it.
//!
//! Serde treats runs of uppercase letters as word boundaries the same way a
//! human would: `HTTPServer` is two words (`HTTP`, `Server`) not five
//! (`H`, `T`, `T`, `P`, `Server`). The earlier naive implementation here
//! emitted `h_t_t_p_server` for `rename_all = "snake_case"`, which diverges
//! from the runtime JSON. These helpers fix that by splitting on both
//! lower→upper and acronym→lower boundaries.

/// Split an identifier into its constituent lowercase words.
/// Handles:
///  * camelCase: `userId` → `["user", "id"]`
///  * PascalCase: `GetUser` → `["get", "user"]`
///  * acronyms: `HTTPServer` → `["http", "server"]`
///  * trailing acronyms: `ParseJSON` → `["parse", "json"]`
///  * digits as boundaries: `foo1Bar` → `["foo1", "bar"]`
///  * existing separators: `user_id`, `user-id` → `["user", "id"]`
///  * leading / repeated underscores collapse (`__private_field` → `["private", "field"]`)
fn split_words(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut words: Vec<String> = Vec::new();
    let mut current = String::new();

    let flush = |current: &mut String, words: &mut Vec<String>| {
        if !current.is_empty() {
            words.push(std::mem::take(current).to_lowercase());
        }
    };

    for i in 0..chars.len() {
        let c = chars[i];

        // Explicit separators: always a boundary.
        if c == '_' || c == '-' || c == ' ' {
            flush(&mut current, &mut words);
            continue;
        }

        if c.is_uppercase() && !current.is_empty() {
            let prev = chars[i - 1];
            let next = chars.get(i + 1).copied();

            // camelCase boundary: `userId` → `user` | `Id`
            // digit boundary:    `foo1Bar` → `foo1` | `Bar`
            // acronym→lower:     `HTTPServer` → `HTTP` | `Server`
            //                    (previous char is uppercase AND next is lowercase)
            let is_camel = prev.is_lowercase() || prev.is_ascii_digit();
            let is_acronym_end = prev.is_uppercase() && next.is_some_and(|n| n.is_lowercase());

            if is_camel || is_acronym_end {
                flush(&mut current, &mut words);
            }
        }

        current.push(c);
    }
    flush(&mut current, &mut words);

    words
}

/// snake_case → camelCase, PascalCase → camelCase, etc.
/// Matches serde's `rename_all = "camelCase"` behaviour: the first word
/// stays fully lowercase, every subsequent word is capitalised.
pub fn to_camel_case(s: &str) -> String {
    let mut words = split_words(s).into_iter();
    let mut result = words.next().unwrap_or_default();
    for word in words {
        result.push_str(&capitalise_first(&word));
    }
    result
}

/// snake_case → PascalCase. `HTTPServer` → `HttpServer`.
pub fn to_pascal_case(s: &str) -> String {
    let mut result = String::new();
    for word in split_words(s) {
        result.push_str(&capitalise_first(&word));
    }
    result
}

/// `HTTPServer` → `http_server`.
pub fn to_snake_case(s: &str) -> String {
    split_words(s).join("_")
}

/// `HTTPServer` → `HTTP_SERVER`.
pub fn to_screaming_snake_case(s: &str) -> String {
    to_snake_case(s).to_uppercase()
}

/// `HTTPServer` → `http-server`.
pub fn to_kebab_case(s: &str) -> String {
    split_words(s).join("-")
}

/// `HTTPServer` → `HTTP-SERVER`.
pub fn to_screaming_kebab_case(s: &str) -> String {
    to_kebab_case(s).to_uppercase()
}

/// Reduce a `::`-separated Rust path to its final segment. A pure-name
/// input passes through unchanged. Used everywhere we need to render or
/// look up a type by its simple identifier (TypeScript output, generator
/// context membership checks, Tauri-special-type filtering).
pub fn simple_name(path: &str) -> &str {
    path.rsplit("::").next().unwrap_or(path)
}

fn capitalise_first(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_basic() {
        assert_eq!(to_snake_case("user_id"), "user_id");
        assert_eq!(to_snake_case("userId"), "user_id");
        assert_eq!(to_snake_case("GetUser"), "get_user");
        assert_eq!(to_snake_case("hello"), "hello");
    }

    #[test]
    fn snake_case_acronyms() {
        // This used to produce "h_t_t_p_server" — the cardinal bug behind
        // broken `rename_all = "snake_case"` on enum variants.
        assert_eq!(to_snake_case("HTTPServer"), "http_server");
        assert_eq!(to_snake_case("URLParser"), "url_parser");
        assert_eq!(to_snake_case("XMLHttp"), "xml_http");
        assert_eq!(to_snake_case("HTTP"), "http");
        assert_eq!(to_snake_case("ABC"), "abc");
        assert_eq!(to_snake_case("ParseJSON"), "parse_json");
    }

    #[test]
    fn snake_case_digits_and_separators() {
        assert_eq!(to_snake_case("foo1Bar"), "foo1_bar");
        assert_eq!(to_snake_case("get_user_1"), "get_user_1");
        assert_eq!(to_snake_case("user-id"), "user_id");
    }

    #[test]
    fn camel_case_basic() {
        assert_eq!(to_camel_case("get_user"), "getUser");
        assert_eq!(to_camel_case("get_user_by_id"), "getUserById");
        assert_eq!(to_camel_case("hello"), "hello");
        // All-caps one-word input is a single word → fully lowercase.
        // The previous implementation returned "hELLO", which no language's
        // camelCase convention allows.
        assert_eq!(to_camel_case("HELLO"), "hello");
    }

    #[test]
    fn camel_case_edge_cases() {
        assert_eq!(to_camel_case("get__user"), "getUser");
        assert_eq!(to_camel_case("_private"), "private");
        assert_eq!(to_camel_case("__private_field"), "privateField");
        assert_eq!(to_camel_case("trailing_"), "trailing");
        assert_eq!(to_camel_case("a"), "a");
        assert_eq!(to_camel_case("get_user_1"), "getUser1");
    }

    #[test]
    fn camel_case_already_camel() {
        assert_eq!(to_camel_case("getUser"), "getUser");
        assert_eq!(to_camel_case("getUserById"), "getUserById");
    }

    #[test]
    fn camel_case_acronyms() {
        assert_eq!(to_camel_case("HTTPServer"), "httpServer");
        assert_eq!(to_camel_case("URLParser"), "urlParser");
        assert_eq!(to_camel_case("ParseJSON"), "parseJson");
    }

    #[test]
    fn pascal_case_from_snake() {
        assert_eq!(to_pascal_case("user_id"), "UserId");
        assert_eq!(to_pascal_case("get_user_by_id"), "GetUserById");
        assert_eq!(to_pascal_case("hello"), "Hello");
    }

    #[test]
    fn pascal_case_round_trip_from_pascal() {
        assert_eq!(to_pascal_case("GetUser"), "GetUser");
        // `HTTP` is now treated as one lowercase word and title-cased:
        // `Http`. The previous behaviour kept the all-caps form, but that
        // only happened by accident of the buggy snake-case split.
        assert_eq!(to_pascal_case("HTTP"), "Http");
        assert_eq!(to_pascal_case("HTTPServer"), "HttpServer");
    }

    #[test]
    fn pascal_case_edge_cases() {
        assert_eq!(to_pascal_case(""), "");
        assert_eq!(to_pascal_case("a"), "A");
        assert_eq!(to_pascal_case("get__user"), "GetUser");
    }

    #[test]
    fn simple_name_strips_path_segments() {
        assert_eq!(simple_name("crate::types::User"), "User");
        assert_eq!(simple_name("types::User"), "User");
        assert_eq!(simple_name("User"), "User");
        assert_eq!(simple_name(""), "");
        assert_eq!(simple_name("super::super::Foo"), "Foo");
    }

    #[test]
    fn kebab_and_screaming_forms() {
        assert_eq!(to_kebab_case("HTTPServer"), "http-server");
        assert_eq!(to_screaming_snake_case("HTTPServer"), "HTTP_SERVER");
        assert_eq!(to_screaming_kebab_case("HTTPServer"), "HTTP-SERVER");
    }
}
