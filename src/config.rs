use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
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
        envy::from_env()
    }
}

impl ServerConfig {
    pub fn addr(&self) -> std::net::SocketAddr {
        format!("{}:{}", self.host, self.port).parse().unwrap()
    }
}
