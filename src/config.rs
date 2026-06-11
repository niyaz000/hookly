use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub crypto: CryptoConfig,
    pub otel: OtelConfig,
}

/// Optional OpenTelemetry export. Disabled when OTEL_EXPORTER_OTLP_ENDPOINT is unset.
#[derive(Deserialize, Clone, Default)]
pub struct OtelConfig {
    pub exporter_otlp_endpoint: Option<String>,
    pub service_name: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct CryptoConfig {
    /// Standard base64-encoded 32-byte master key. Generate with: openssl rand -base64 32
    pub master_key: String,
    /// Standard base64-encoded 32-byte key for api key envelope encryption. Generate with: openssl rand -base64 32
    pub api_key_encryption_key: String,
}

#[derive(Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Deserialize, Clone)]
pub struct RedisConfig {
    pub url: String,
}

impl Config {
    pub fn from_env() -> Result<Self, envy::Error> {
        Ok(Self {
            server: envy::prefixed("SERVER_").from_env()?,
            database: envy::prefixed("DATABASE_").from_env()?,
            redis: envy::prefixed("REDIS_").from_env()?,
            crypto: envy::prefixed("CRYPTO_").from_env()?,
            otel: envy::prefixed("OTEL_").from_env().unwrap_or_default(),
        })
    }
}

impl ServerConfig {
    pub fn addr(&self) -> std::net::SocketAddr {
        format!("{}:{}", self.host, self.port).parse().unwrap()
    }
}
