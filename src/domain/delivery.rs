use crate::domain::DomainError;
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub const RETRY_DELAYS: [i64; 5] = [60, 120, 240, 480, 960];
const MAX_RESPONSE_BYTES: usize = 10_240;
const FALLBACK_DELAY: i64 = 960;

pub fn retry_delay(retry_count: i32) -> Option<i64> {
    RETRY_DELAYS.get(retry_count.max(0) as usize).copied()
}

pub fn fallback_delay() -> i64 {
    FALLBACK_DELAY
}

pub fn truncate_body(body: String) -> String {
    if body.len() <= MAX_RESPONSE_BYTES {
        return body;
    }
    let mut boundary = MAX_RESPONSE_BYTES;
    while !body.is_char_boundary(boundary) {
        boundary -= 1;
    }
    format!("{}[truncated]", &body[..boundary])
}

pub fn signature(secret: &str, timestamp: i64, payload: &[u8]) -> Result<String, DomainError> {
    let mut signed = timestamp.to_string().into_bytes();
    signed.push(b'.');
    signed.extend_from_slice(payload);
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes())
        .map_err(|error| DomainError::internal(format!("signing key rejected: {error}")))?;
    mac.update(&signed);
    Ok(hex::encode(mac.finalize().into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_retry_schedule_matches_cloud() {
        // Arrange
        let expected = vec![60, 120, 240, 480, 960];

        // Act
        let delays = (0..5).filter_map(retry_delay).collect::<Vec<_>>();

        // Assert
        assert_eq!(delays, expected);
    }

    #[test]
    fn retry_delay_is_none_past_the_last_attempt() {
        // Arrange
        let retry_count = 5;

        // Act
        let delay = retry_delay(retry_count);

        // Assert
        assert_eq!(delay, None);
    }

    #[test]
    fn negative_retry_counts_read_the_first_slot() {
        // Arrange
        let retry_count = -1;

        // Act
        let delay = retry_delay(retry_count);

        // Assert
        assert_eq!(delay, Some(60));
    }

    #[test]
    fn short_bodies_pass_through_unchanged() {
        // Arrange
        let body = "{\"ok\":true}".to_string();

        // Act
        let truncated = truncate_body(body.clone());

        // Assert
        assert_eq!(truncated, body);
    }

    #[test]
    fn long_bodies_are_marked_as_truncated() {
        // Arrange
        let body = "x".repeat(MAX_RESPONSE_BYTES + 100);

        // Act
        let truncated = truncate_body(body);

        // Assert
        assert!(truncated.ends_with("[truncated]"));
        assert!(truncated.len() <= MAX_RESPONSE_BYTES + "[truncated]".len());
    }

    #[test]
    fn truncation_respects_multibyte_boundaries() {
        // Arrange
        let body = "あ".repeat(MAX_RESPONSE_BYTES);

        // Act
        let truncated = truncate_body(body);

        // Assert
        assert!(truncated.ends_with("[truncated]"));
        assert!(truncated.chars().all(|character| character == 'あ' || character == '[' || character == 't' || character == 'r' || character == 'u' || character == 'n' || character == 'c' || character == 'a' || character == 'e' || character == 'd' || character == ']'));
    }

    #[test]
    fn signing_fixture_is_stable() {
        // Arrange
        let secret = "whsec_fixture";
        let timestamp = 1_700_000_000;
        let payload = br#"{"ok":true}"#;

        // Act
        let signed = signature(secret, timestamp, payload);

        // Assert
        assert_eq!(
            signed,
            Ok("31a99e5c88be4311395a895ea0d686baf164714d49a52bae17fad334b78db984".to_string())
        );
    }

    #[test]
    fn signature_changes_with_payload_or_secret() {
        // Arrange
        let base = signature("secret", 1_000, b"{}").expect("signing succeeds");
        let other_payload = signature("secret", 1_000, b"{ }").expect("signing succeeds");
        let other_secret = signature("Secret", 1_000, b"{}").expect("signing succeeds");

        // Act
        let all_distinct = base != other_payload && base != other_secret;

        // Assert
        assert!(all_distinct);
    }
}
