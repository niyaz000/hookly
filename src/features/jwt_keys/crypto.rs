use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::rngs::OsRng;
use serde_json::{json, Value};

use crate::error::AppError;

use super::models::JwtAlgorithm;

pub struct GeneratedKeyPair {
    pub public_key_pem: Option<String>,
    pub private_key_pem: Option<String>,
    /// For HMAC algorithms: the raw secret (not a PEM). Encrypted before storage.
    pub hmac_secret: Option<String>,
}

pub fn generate_key_pair(algorithm: &JwtAlgorithm) -> Result<GeneratedKeyPair, AppError> {
    match algorithm {
        JwtAlgorithm::ES256 => generate_p256(),
        JwtAlgorithm::ES384 => generate_p384(),
        JwtAlgorithm::ES512 => Err(AppError::BadRequest(
            "ES512 (P-521) is not supported; use ES256 or ES384".into(),
        )),
        JwtAlgorithm::RS256 | JwtAlgorithm::RS384 | JwtAlgorithm::RS512 => generate_rsa(),
        JwtAlgorithm::HS256 => Ok(generate_hmac(32)),
        JwtAlgorithm::HS512 => Ok(generate_hmac(64)),
    }
}

fn generate_p256() -> Result<GeneratedKeyPair, AppError> {
    use p256::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};

    let sk = p256::SecretKey::random(&mut OsRng);
    let pk = sk.public_key();

    let private_pem = sk
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|_| AppError::Internal("failed to encode P-256 private key".into()))?
        .to_string();

    let public_pem = pk
        .to_public_key_pem(LineEnding::LF)
        .map_err(|_| AppError::Internal("failed to encode P-256 public key".into()))?;

    Ok(GeneratedKeyPair {
        public_key_pem: Some(public_pem),
        private_key_pem: Some(private_pem),
        hmac_secret: None,
    })
}

fn generate_p384() -> Result<GeneratedKeyPair, AppError> {
    use p384::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};

    let sk = p384::SecretKey::random(&mut OsRng);
    let pk = sk.public_key();

    let private_pem = sk
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|_| AppError::Internal("failed to encode P-384 private key".into()))?
        .to_string();

    let public_pem = pk
        .to_public_key_pem(LineEnding::LF)
        .map_err(|_| AppError::Internal("failed to encode P-384 public key".into()))?;

    Ok(GeneratedKeyPair {
        public_key_pem: Some(public_pem),
        private_key_pem: Some(private_pem),
        hmac_secret: None,
    })
}

fn generate_rsa() -> Result<GeneratedKeyPair, AppError> {
    use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};

    let private_key = rsa::RsaPrivateKey::new(&mut OsRng, 2048)
        .map_err(|_| AppError::Internal("failed to generate RSA key".into()))?;
    let public_key = rsa::RsaPublicKey::from(&private_key);

    let private_pem = private_key
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|_| AppError::Internal("failed to encode RSA private key".into()))?
        .to_string();

    let public_pem = public_key
        .to_public_key_pem(LineEnding::LF)
        .map_err(|_| AppError::Internal("failed to encode RSA public key".into()))?;

    Ok(GeneratedKeyPair {
        public_key_pem: Some(public_pem),
        private_key_pem: Some(private_pem),
        hmac_secret: None,
    })
}

fn generate_hmac(bytes: usize) -> GeneratedKeyPair {
    use rand::RngCore;
    let mut key_bytes = vec![0u8; bytes];
    OsRng.fill_bytes(&mut key_bytes);
    let secret = URL_SAFE_NO_PAD.encode(&key_bytes);

    GeneratedKeyPair {
        public_key_pem: None,
        private_key_pem: None,
        hmac_secret: Some(secret),
    }
}

/// Converts a PEM public key to JWK format for the JWKS endpoint.
/// Returns None for HMAC algorithms (symmetric keys are never exposed in JWKS).
pub fn public_key_to_jwk(
    algorithm: &JwtAlgorithm,
    key_id: &str,
    public_key_pem: &str,
) -> Result<Option<Value>, AppError> {
    match algorithm {
        JwtAlgorithm::ES256 => Ok(Some(p256_pem_to_jwk(public_key_pem, key_id)?)),
        JwtAlgorithm::ES384 => Ok(Some(p384_pem_to_jwk(public_key_pem, key_id)?)),
        JwtAlgorithm::ES512 => Ok(None), // P-521 not supported
        JwtAlgorithm::RS256 => Ok(Some(rsa_pem_to_jwk(public_key_pem, key_id, "RS256")?)),
        JwtAlgorithm::RS384 => Ok(Some(rsa_pem_to_jwk(public_key_pem, key_id, "RS384")?)),
        JwtAlgorithm::RS512 => Ok(Some(rsa_pem_to_jwk(public_key_pem, key_id, "RS512")?)),
        JwtAlgorithm::HS256 | JwtAlgorithm::HS512 => Ok(None),
    }
}

fn p256_pem_to_jwk(pem: &str, kid: &str) -> Result<Value, AppError> {
    use p256::pkcs8::DecodePublicKey;
    use p256::elliptic_curve::sec1::ToEncodedPoint;

    let pk = p256::PublicKey::from_public_key_pem(pem)
        .map_err(|_| AppError::Internal("invalid P-256 public key PEM".into()))?;
    let point = pk.as_affine().to_encoded_point(false);
    let x = point.x().ok_or_else(|| AppError::Internal("missing x coordinate".into()))?.to_vec();
    let y = point.y().ok_or_else(|| AppError::Internal("missing y coordinate".into()))?.to_vec();

    Ok(json!({
        "kty": "EC",
        "use": "sig",
        "kid": kid,
        "alg": "ES256",
        "crv": "P-256",
        "x": URL_SAFE_NO_PAD.encode(&x),
        "y": URL_SAFE_NO_PAD.encode(&y),
    }))
}

fn p384_pem_to_jwk(pem: &str, kid: &str) -> Result<Value, AppError> {
    use p384::pkcs8::DecodePublicKey;
    use p384::elliptic_curve::sec1::ToEncodedPoint;

    let pk = p384::PublicKey::from_public_key_pem(pem)
        .map_err(|_| AppError::Internal("invalid P-384 public key PEM".into()))?;
    let point = pk.as_affine().to_encoded_point(false);
    let x = point.x().ok_or_else(|| AppError::Internal("missing x coordinate".into()))?.to_vec();
    let y = point.y().ok_or_else(|| AppError::Internal("missing y coordinate".into()))?.to_vec();

    Ok(json!({
        "kty": "EC",
        "use": "sig",
        "kid": kid,
        "alg": "ES384",
        "crv": "P-384",
        "x": URL_SAFE_NO_PAD.encode(&x),
        "y": URL_SAFE_NO_PAD.encode(&y),
    }))
}

fn rsa_pem_to_jwk(pem: &str, kid: &str, alg: &str) -> Result<Value, AppError> {
    use rsa::pkcs8::DecodePublicKey;
    use rsa::traits::PublicKeyParts;

    let pk = rsa::RsaPublicKey::from_public_key_pem(pem)
        .map_err(|_| AppError::Internal("invalid RSA public key PEM".into()))?;

    let n = URL_SAFE_NO_PAD.encode(pk.n().to_bytes_be());
    let e = URL_SAFE_NO_PAD.encode(pk.e().to_bytes_be());

    Ok(json!({
        "kty": "RSA",
        "use": "sig",
        "kid": kid,
        "alg": alg,
        "n": n,
        "e": e,
    }))
}
