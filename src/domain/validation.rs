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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ErrorKind;

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
