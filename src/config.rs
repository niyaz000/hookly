use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub crypto: CryptoConfig,
}

#[derive(Deserialize, Clone)]
pub struct CryptoConfig {
    /// Standard base64-encoded 32-byte master key. Generate with: openssl rand -base64 32
    pub master_key: String,
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
        })
    }
}

impl ServerConfig {
    pub fn addr(&self) -> std::net::SocketAddr {
        format!("{}:{}", self.host, self.port).parse().unwrap()
    }
}
