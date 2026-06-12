use std::collections::HashMap;

use chrono::Utc;
use validator::ValidationError;

use crate::error::AppError;

pub fn validate_not_blank(s: &str) -> Result<(), ValidationError> {
    if s.trim().is_empty() {
        return Err(ValidationError::new("required"));
    }
    Ok(())
}

pub fn validate_slug(s: &str) -> Result<(), ValidationError> {
    let chars_valid = s
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    let no_leading_trailing = !s.starts_with('-') && !s.ends_with('-');
    if !chars_valid || !no_leading_trailing {
        return Err(ValidationError::new("invalid_format"));
    }
    Ok(())
}

pub fn validate_future_date(dt: &chrono::DateTime<Utc>) -> Result<(), ValidationError> {
    if *dt <= Utc::now() {
        return Err(ValidationError::new("invalid_value"));
    }
    Ok(())
}

pub fn validate_tags(tags: &HashMap<String, String>) -> Result<(), AppError> {
    if tags.len() > 5 {
        return Err(AppError::BadRequest("tags: max 5 entries".into()));
    }
    for (k, v) in tags {
        if k.len() > 64 {
            return Err(AppError::BadRequest(
                "tags: key must be 64 characters or fewer".into(),
            ));
        }
        if v.len() > 255 {
            return Err(AppError::BadRequest(
                "tags: value must be 255 characters or fewer".into(),
            ));
        }
    }
    Ok(())
}
