/// Convert snake_case to camelCase
/// Handles edge cases like double underscores, leading underscores, and trailing underscores
pub fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    let mut seen_non_underscore = false;

    for c in s.chars() {
        if c == '_' {
            // Only capitalize next if we've seen a non-underscore character
            // This prevents leading underscores from capitalizing the first letter
            if seen_non_underscore {
                capitalize_next = true;
            }
            // Skip the underscore (don't add to result)
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
            seen_non_underscore = true;
        } else if !seen_non_underscore {
            // First non-underscore character: lowercase it
            result.push(c.to_ascii_lowercase());
            seen_non_underscore = true;
        } else {
            result.push(c);
        }
    }

    result
}

/// Convert PascalCase to snake_case
pub fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_ascii_lowercase());
    }
    result
}

/// Convert snake_case (or PascalCase) to PascalCase.
/// Safe to apply to already-PascalCase input: it round-trips via snake_case.
pub fn to_pascal_case(s: &str) -> String {
    let snake = to_snake_case(s);
    let mut result = String::new();
    let mut capitalize_next = true;
    for c in snake.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

/// Convert PascalCase to SCREAMING_SNAKE_CASE
pub fn to_screaming_snake_case(s: &str) -> String {
    to_snake_case(s).to_uppercase()
}

/// Convert PascalCase to kebab-case
pub fn to_kebab_case(s: &str) -> String {
    to_snake_case(s).replace('_', "-")
}

/// Convert PascalCase to SCREAMING-KEBAB-CASE
pub fn to_screaming_kebab_case(s: &str) -> String {
    to_kebab_case(s).to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_camel_case_basic() {
        assert_eq!(to_camel_case("get_user"), "getUser");
        assert_eq!(to_camel_case("get_user_by_id"), "getUserById");
        assert_eq!(to_camel_case("hello"), "hello");
        assert_eq!(to_camel_case("HELLO"), "hELLO");
    }

    #[test]
    fn test_to_camel_case_edge_cases() {
        // Double underscores - skipped
        assert_eq!(to_camel_case("get__user"), "getUser");
        // Leading underscore - should NOT capitalize first letter
        assert_eq!(to_camel_case("_private"), "private");
        // Multiple leading underscores
        assert_eq!(to_camel_case("__private_field"), "privateField");
        // Trailing underscore - just ignored
        assert_eq!(to_camel_case("trailing_"), "trailing");
        // Single letter
        assert_eq!(to_camel_case("a"), "a");
        // Numbers
        assert_eq!(to_camel_case("get_user_1"), "getUser1");
    }

    #[test]
    fn test_to_camel_case_already_camel() {
        assert_eq!(to_camel_case("getUser"), "getUser");
        assert_eq!(to_camel_case("getUserById"), "getUserById");
    }

    #[test]
    fn test_to_pascal_case_from_snake() {
        assert_eq!(to_pascal_case("user_id"), "UserId");
        assert_eq!(to_pascal_case("get_user_by_id"), "GetUserById");
        assert_eq!(to_pascal_case("hello"), "Hello");
    }

    #[test]
    fn test_to_pascal_case_round_trip_from_pascal() {
        assert_eq!(to_pascal_case("GetUser"), "GetUser");
        // All-caps acronyms survive the snake_case round-trip unchanged.
        assert_eq!(to_pascal_case("HTTP"), "HTTP");
    }

    #[test]
    fn test_to_pascal_case_edge_cases() {
        assert_eq!(to_pascal_case(""), "");
        assert_eq!(to_pascal_case("a"), "A");
        assert_eq!(to_pascal_case("get__user"), "GetUser");
    }
}

