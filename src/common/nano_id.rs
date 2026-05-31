use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

const LEN: usize = 16;

const ALPHANUMERIC: [char; 62] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I',
    'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b',
    'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u',
    'v', 'w', 'x', 'y', 'z',
];

/// Compact, URL-safe identifier (16 alphanumeric characters).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NanoId(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NanoIdError {
    InvalidLength,
    InvalidCharacter,
}

impl fmt::Display for NanoIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength => write!(f, "nanoid must be {LEN} characters"),
            Self::InvalidCharacter => write!(f, "nanoid must contain only alphanumeric characters"),
        }
    }
}

impl std::error::Error for NanoIdError {}

impl Default for NanoId {
    fn default() -> Self {
        Self::new()
    }
}

impl NanoId {
    pub const LENGTH: usize = LEN;

    pub fn new() -> Self {
        Self(nanoid::nanoid!(LEN, &ALPHANUMERIC))
    }

    pub fn generate(len: usize) -> String {
        nanoid::nanoid!(len, &ALPHANUMERIC)
    }

    pub fn into_inner(self) -> String {
        self.0
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(value: &str) -> Result<(), NanoIdError> {
        if value.len() != LEN {
            return Err(NanoIdError::InvalidLength);
        }

        if !value.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(NanoIdError::InvalidCharacter);
        }

        Ok(())
    }
}

impl From<NanoId> for String {
    fn from(value: NanoId) -> Self {
        value.0
    }
}

impl FromStr for NanoId {
    type Err = NanoIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::validate(s)?;
        Ok(Self(s.to_owned()))
    }
}

impl TryFrom<String> for NanoId {
    type Error = NanoIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::validate(&value)?;
        Ok(Self(value))
    }
}

impl fmt::Display for NanoId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for NanoId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl sqlx::Type<sqlx::Postgres> for NanoId {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for NanoId {
    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::Postgres as sqlx::Database>::ArgumentBuffer<'q>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <String as sqlx::Encode<'q, sqlx::Postgres>>::encode_by_ref(&self.0, buf)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for NanoId {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <String as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        NanoId::try_from(s).map_err(Into::into)
    }
}
