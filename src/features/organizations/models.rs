use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;

use crate::error::{AppError, FieldError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "organization_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum OrganizationStatus {
    Active,
    Suspended,
    Inactive,
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct Organization {
    pub id: Uuid,
    pub public_id: String,
    pub name: String,
    pub slug: String,
    pub status: OrganizationStatus,
    pub billing_email: Option<String>,
    pub plan: String,
    pub stripe_customer_id: Option<String>,
    pub external_id: Option<String>,
    pub tags: Json<HashMap<String, String>>,
    pub metadata: Json<HashMap<String, String>>,
    pub settings: Json<HashMap<String, String>>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateOrganizationRequest {
    pub name: String,
    pub slug: String,
    pub billing_email: Option<String>,
    pub stripe_customer_id: Option<String>,
    pub external_id: Option<String>,
    pub tags: Option<HashMap<String, String>>,
    pub metadata: Option<HashMap<String, String>>,
    pub settings: Option<HashMap<String, String>>,
}

impl CreateOrganizationRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        let mut errors = Vec::new();

        if self.name.trim().is_empty() {
            errors.push(FieldError::new("name", "required", "name is required"));
        } else if self.name.len() > 255 {
            errors.push(FieldError::new(
                "name",
                "max_length",
                "name must be 255 characters or fewer",
            ));
        }

        collect_slug_errors(&self.slug, &mut errors);

        if let Some(ref e) = self.billing_email {
            if e.len() > 64 {
                errors.push(
                    FieldError::new(
                        "billing_email",
                        "max_length",
                        "billing_email must be 64 characters or fewer",
                    )
                    .with_value(e.clone()),
                );
            }
        }
        if let Some(ref s) = self.stripe_customer_id {
            if s.len() > 32 {
                errors.push(
                    FieldError::new(
                        "stripe_customer_id",
                        "max_length",
                        "stripe_customer_id must be 32 characters or fewer",
                    )
                    .with_value(s.clone()),
                );
            }
        }
        if let Some(ref e) = self.external_id {
            if e.len() > 64 {
                errors.push(
                    FieldError::new(
                        "external_id",
                        "max_length",
                        "external_id must be 64 characters or fewer",
                    )
                    .with_value(e.clone()),
                );
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(AppError::Validation(errors))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateOrganizationRequest {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub billing_email: Option<String>,
    pub stripe_customer_id: Option<String>,
    pub external_id: Option<String>,
    pub tags: Option<HashMap<String, String>>,
    pub metadata: Option<HashMap<String, String>>,
    pub settings: Option<HashMap<String, String>>,
}

impl UpdateOrganizationRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        let mut errors = Vec::new();

        if let Some(ref n) = self.name {
            if n.trim().is_empty() {
                errors.push(FieldError::new("name", "required", "name cannot be empty"));
            } else if n.len() > 255 {
                errors.push(FieldError::new(
                    "name",
                    "max_length",
                    "name must be 255 characters or fewer",
                ));
            }
        }
        if let Some(ref s) = self.slug {
            collect_slug_errors(s, &mut errors);
        }
        if let Some(ref e) = self.billing_email {
            if e.len() > 64 {
                errors.push(
                    FieldError::new(
                        "billing_email",
                        "max_length",
                        "billing_email must be 64 characters or fewer",
                    )
                    .with_value(e.clone()),
                );
            }
        }
        if let Some(ref s) = self.stripe_customer_id {
            if s.len() > 32 {
                errors.push(
                    FieldError::new(
                        "stripe_customer_id",
                        "max_length",
                        "stripe_customer_id must be 32 characters or fewer",
                    )
                    .with_value(s.clone()),
                );
            }
        }
        if let Some(ref e) = self.external_id {
            if e.len() > 64 {
                errors.push(
                    FieldError::new(
                        "external_id",
                        "max_length",
                        "external_id must be 64 characters or fewer",
                    )
                    .with_value(e.clone()),
                );
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(AppError::Validation(errors))
        }
    }
}

fn collect_slug_errors(slug: &str, errors: &mut Vec<FieldError>) {
    if slug.trim().is_empty() {
        errors.push(FieldError::new("slug", "required", "slug is required"));
    } else if slug.len() > 64 {
        errors.push(
            FieldError::new("slug", "max_length", "slug must be 64 characters or fewer")
                .with_value(slug.to_owned()),
        );
    } else {
        let chars_valid = slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        let no_leading_trailing_hyphen = !slug.starts_with('-') && !slug.ends_with('-');
        if !chars_valid || !no_leading_trailing_hyphen {
            errors.push(
                FieldError::new(
                    "slug",
                    "invalid_format",
                    "slug must be lowercase alphanumeric and hyphens, not starting or ending with a hyphen",
                )
                .with_value(slug.to_owned()),
            );
        }
    }
}

#[derive(Debug, Serialize)]
pub struct OrganizationResponse {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub status: OrganizationStatus,
    pub billing_email: Option<String>,
    pub plan: String,
    pub stripe_customer_id: Option<String>,
    pub external_id: Option<String>,
    pub tags: HashMap<String, String>,
    pub metadata: HashMap<String, String>,
    pub settings: HashMap<String, String>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Organization> for OrganizationResponse {
    fn from(org: Organization) -> Self {
        Self {
            id: org.public_id,
            name: org.name,
            slug: org.slug,
            status: org.status,
            billing_email: org.billing_email,
            plan: org.plan,
            stripe_customer_id: org.stripe_customer_id,
            external_id: org.external_id,
            tags: org.tags.0,
            metadata: org.metadata.0,
            settings: org.settings.0,
            created_by: org.created_by,
            updated_by: org.updated_by,
            created_at: org.created_at,
            updated_at: org.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListOrganizationsQuery {
    pub limit: Option<i64>,
    pub cursor: Option<String>,
    pub status: Option<OrganizationStatus>,
}

#[derive(Debug, Serialize)]
pub struct ListOrganizationsResponse {
    pub items: Vec<OrganizationResponse>,
    pub next_cursor: Option<String>,
    pub limit: i64,
}
