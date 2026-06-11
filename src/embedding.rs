//! `Displace\Infer\Embedding` — the result of `Model::embed()`.
//!
//! Wraps the float vector llama.cpp returns for a single input text, plus
//! convenience math: dimensionality, L2 norm, unit-vector normalization,
//! cosine similarity against another embedding.
//!
//! Vectors are stored as `Vec<f32>` (llama.cpp's native representation) and
//! returned to PHP as arrays of doubles — PHP's `float` is double-precision,
//! so f32 → f64 is lossless and the conversion happens in ext-php-rs's
//! `IntoZval` impl for `Vec<f32>`.

use ext_php_rs::binary::Binary;
use ext_php_rs::prelude::*;

use crate::error::InferError;

/// A single embedding vector.
///
/// Read-only. Instances are produced by `Model::embed()`; direct construction
/// is refused — a vector built by PHP would lie about which model produced
/// it and what pooling strategy was applied.
#[php_class]
#[php(name = "Displace\\Infer\\Embedding")]
#[derive(Default, Clone)]
pub struct Embedding {
    vector: Vec<f32>,
}

#[php_impl]
impl Embedding {
    /// Refuse direct construction.
    pub fn __construct() -> PhpResult<Self> {
        Err(InferError::InvalidConstruction(
            "Displace\\Infer\\Embedding is produced by Model::embed(); do not instantiate directly"
                .into(),
        )
        .into())
    }

    /// The embedding as a flat array of floats. The length is `dimensions()`
    /// and matches the loaded model's `n_embd`.
    pub fn vector(&self) -> Vec<f32> {
        self.vector.clone()
    }

    /// Vector length — equivalent to the embedding model's hidden size.
    pub fn dimensions(&self) -> usize {
        self.vector.len()
    }

    /// The embedding as a packed little-endian float32 binary string —
    /// byte-identical to `pack('g*', ...$embedding->vector())` and the
    /// format every Displace vector API speaks (`Displace\Vector`
    /// indexes, `Displace\AI\Contracts\Embedder`, ...).
    ///
    /// This is the zero-inflation handoff: the bytes are produced
    /// directly from the f32 vector held on the Rust side, so the
    /// coordinates are never materialized as PHP zvals. Prefer this over
    /// `vector()` whenever the destination wants packed bytes.
    pub fn packed(&self) -> Binary<u8> {
        let mut bytes = Vec::with_capacity(self.vector.len() * 4);
        for v in &self.vector {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        Binary::new(bytes)
    }

    /// L2 norm of the vector. Useful for verifying that an upstream model
    /// already returned unit-length vectors, or for computing cosine
    /// similarity manually.
    pub fn norm(&self) -> f64 {
        l2_norm(&self.vector) as f64
    }

    /// Return a new `Embedding` with the vector scaled to unit length.
    /// A zero-length vector is returned unchanged (dividing by zero would
    /// produce NaN; surfacing that to PHP would be worse than the identity).
    pub fn normalize(&self) -> Self {
        let n = l2_norm(&self.vector);
        if n == 0.0 {
            return self.clone();
        }
        Self {
            vector: self.vector.iter().map(|x| x / n).collect(),
        }
    }

    /// Cosine similarity against another embedding. Throws if the two
    /// vectors have different dimensions — comparing across model families
    /// is almost always a bug.
    pub fn cosine_similarity(&self, other: &Embedding) -> PhpResult<f64> {
        if self.vector.len() != other.vector.len() {
            return Err(InferError::Inference(format!(
                "cannot compare embeddings of different dimensions: {} vs {}",
                self.vector.len(),
                other.vector.len()
            ))
            .into());
        }
        let dot: f32 = self
            .vector
            .iter()
            .zip(other.vector.iter())
            .map(|(a, b)| a * b)
            .sum();
        let na = l2_norm(&self.vector);
        let nb = l2_norm(&other.vector);
        if na == 0.0 || nb == 0.0 {
            // No meaningful angle between a vector and the origin; return 0.
            return Ok(0.0);
        }
        Ok((dot / (na * nb)) as f64)
    }
}

impl Embedding {
    /// Rust-side constructor used by `Model::embed()`.
    pub(crate) fn from_vec(vector: Vec<f32>) -> Self {
        Self { vector }
    }
}

fn l2_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}
