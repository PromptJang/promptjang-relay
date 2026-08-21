use axum::http::HeaderMap;
use axum::http::header::AUTHORIZATION;

use crate::domain::DomainError;

pub const MAX_ENDPOINTS: i64 = 10;
pub const MAX_KEYS: i64 = 5;
pub const MAX_PAYLOAD_BYTES: usize = 256 * 1024;
pub const PER_MINUTE_EVENTS: i64 = 1000;

pub fn validate_name(value: &str) -> Result<(), DomainError> {
    if value.trim().is_empty() || value.len() > 100 {
        Err(DomainError::bad_request(
            "name must contain 1 to 100 characters",
        ))
    } else {
        Ok(())
    }
}

pub fn extract_header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty() && value.len() <= 128)
        .map(String::from)
}

pub fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

pub fn idempotency_key(headers: &HeaderMap) -> Result<Option<String>, DomainError> {
    match headers.get("Idempotency-Key").and_then(|value| value.to_str().ok()) {
        None => Ok(None),
        Some(value) if value.is_empty() || value.len() > 255 => Err(DomainError::bad_request(
            "Idempotency-Key must contain 1 to 255 characters",
        )),
        Some(value) => Ok(Some(value.to_string())),
    }
}

pub fn ensure_payload_size(size: usize) -> Result<(), DomainError> {
    if size > MAX_PAYLOAD_BYTES {
        Err(DomainError::payload_too_large(
            "payload exceeds 256 KB",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ErrorKind;
    use axum::http::HeaderValue;

    #[test]
    fn extract_header_keeps_short_non_empty_values() {
        // Arrange
        let mut headers = HeaderMap::new();
        headers.insert("X-Event-Type", HeaderValue::from_static("order.created"));

        // Act
        let value = extract_header(&headers, "X-Event-Type");

        // Assert
        assert_eq!(value.as_deref(), Some("order.created"));
    }

    #[test]
    fn extract_header_drops_empty_and_overlong_values() {
        // Arrange
        let mut headers = HeaderMap::new();
        headers.insert("X-Empty", HeaderValue::from_static(""));
        headers.insert(
            "X-Long",
            HeaderValue::from_str(&"x".repeat(129)).expect("valid header value"),
        );

        // Act
        let empty = extract_header(&headers, "X-Empty");
        let long = extract_header(&headers, "X-Long");

        // Assert
        assert_eq!(empty, None);
        assert_eq!(long, None);
    }

    #[test]
    fn bearer_token_strips_the_scheme() {
        // Arrange
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer pj_oss_abc"),
        );

        // Act
        let token = bearer_token(&headers);

        // Assert
        assert_eq!(token, Some("pj_oss_abc"));
    }

    #[test]
    fn bearer_token_rejects_other_schemes() {
        // Arrange
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Basic abc"));

        // Act
        let token = bearer_token(&headers);

        // Assert
        assert_eq!(token, None);
    }

    #[test]
    fn idempotency_key_is_optional() {
        // Arrange
        let headers = HeaderMap::new();

        // Act
        let key = idempotency_key(&headers);

        // Assert
        assert_eq!(key, Ok(None));
    }

    #[test]
    fn idempotency_key_accepts_valid_range() {
        // Arrange
        let mut headers = HeaderMap::new();
        headers.insert("Idempotency-Key", HeaderValue::from_static("order-1042"));
        let mut edge = HeaderMap::new();
        edge.insert(
            "Idempotency-Key",
            HeaderValue::from_str(&"k".repeat(255)).expect("valid header value"),
        );

        // Act
        let typical = idempotency_key(&headers);
        let boundary = idempotency_key(&edge);

        // Assert
        assert_eq!(typical, Ok(Some("order-1042".into())));
        assert!(matches!(boundary, Ok(Some(value)) if value.len() == 255));
    }

    #[test]
    fn idempotency_key_rejects_empty_and_overlong() {
        // Arrange
        let mut empty = HeaderMap::new();
        empty.insert("Idempotency-Key", HeaderValue::from_static(""));
        let mut long = HeaderMap::new();
        long.insert(
            "Idempotency-Key",
            HeaderValue::from_str(&"k".repeat(256)).expect("valid header value"),
        );

        // Act
        let empty_result = idempotency_key(&empty);
        let long_result = idempotency_key(&long);

        // Assert
        assert_eq!(empty_result.unwrap_err().kind, ErrorKind::BadRequest);
        assert_eq!(long_result.unwrap_err().kind, ErrorKind::BadRequest);
    }

    #[test]
    fn payload_size_allows_up_to_256_kib() {
        // Arrange
        let sizes = [0, 256 * 1024];

        for size in sizes {
            // Act
            let result = ensure_payload_size(size);

            // Assert
            assert_eq!(result, Ok(()));
        }
    }

    #[test]
    fn payload_size_rejects_over_256_kib() {
        // Arrange
        let size = 256 * 1024 + 1;

        // Act
        let result = ensure_payload_size(size);

        // Assert
        assert_eq!(result.unwrap_err().kind, ErrorKind::PayloadTooLarge);
    }

    #[test]
    fn accepts_typical_name() {
        // Arrange
        let name = "Order service";

        // Act
        let result = validate_name(name);

        // Assert
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn rejects_whitespace_only_name() {
        // Arrange
        let name = "   ";

        // Act
        let result = validate_name(name);

        // Assert
        assert_eq!(result.unwrap_err().kind, ErrorKind::BadRequest);
    }

    #[test]
    fn rejects_overlong_name() {
        // Arrange
        let name = "a".repeat(101);

        // Act
        let result = validate_name(&name);

        // Assert
        assert_eq!(result.unwrap_err().kind, ErrorKind::BadRequest);
    }

    #[test]
    fn accepts_name_at_exactly_100_characters() {
        // Arrange
        let name = "a".repeat(100);

        // Act
        let result = validate_name(&name);

        // Assert
        assert_eq!(result, Ok(()));
    }
}
