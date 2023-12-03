use thiserror::Error;

/// Errors created by this library
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// Formatting error
    #[error(transparent)]
    Fmt(#[from] std::fmt::Error),

    /// A multibase conversion error
    #[error(transparent)]
    Multibase(#[from] multibase::Error),

    /// A multicodec decoding error
    #[error(transparent)]
    Multicodec(#[from] multicodec::Error),

    /// A multiutil error
    #[error(transparent)]
    Multiutil(#[from] multiutil::Error),

    /// Missing sigil 0x39
    #[error("Missing Multisig sigil")]
    MissingSigil,

    /// Missing signature data
    #[error("Missing signature data")]
    MissingSignature,

    /// Unsupported signature algorithm
    #[error("Unsupported signature algorithm: {0}")]
    UnsupportedAlgorithm(String),

    /// Failed Varsig conversion
    #[error("Failed Varsig conversion: {0}")]
    FailedConversion(String),
}
