//! Error type for the plotting crate.

use std::fmt;

/// Errors that can occur while building or saving a figure.
#[non_exhaustive]
#[derive(Debug)]
pub enum Error {
    /// An I/O error while writing an image file.
    Io(std::io::Error),
    /// An image encoder failed.
    Encode(String),
    /// The output path had an extension other than `.png`, `.svg`, or `.pdf`.
    UnknownFormat(String),
    /// A figure dimension was zero or absurdly large.
    BadSize(String),
    /// A custom font could not be parsed.
    Font(String),
    /// Inputs that must have the same length do not (e.g. `plot`'s `ys` and
    /// `xs`); nothing was drawn.
    LengthMismatch {
        /// What has the wrong length, e.g. `plot ys`.
        what: &'static str,
        /// The length it must have (that of the input it is paired with).
        expected: usize,
        /// Its actual length.
        found: usize,
    },
    /// Histograms that must share a binning (the layers of a stack) do not;
    /// nothing was drawn.
    BinningMismatch(String),
    /// The output needs a crate feature this build does not enable.
    MissingFeature {
        /// The output that was asked for (e.g. `"PNG"`).
        output: &'static str,
        /// The `oxiplot` feature that provides it.
        feature: &'static str,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "io error: {e}"),
            Error::Encode(m) => write!(f, "image encode error: {m}"),
            Error::UnknownFormat(ext) => {
                write!(
                    f,
                    "unknown image format `{ext}` (use a .png, .svg, or .pdf path)"
                )
            }
            Error::BadSize(m) => write!(f, "invalid figure size: {m}"),
            Error::Font(m) => write!(f, "font error: {m}"),
            Error::LengthMismatch {
                what,
                expected,
                found,
            } => write!(
                f,
                "length mismatch: {what} has length {found}, expected {expected}"
            ),
            Error::BinningMismatch(m) => write!(f, "binning mismatch: {m}"),
            Error::MissingFeature { output, feature } => write!(
                f,
                "{output} output needs the `{feature}` feature of oxiplot"
            ),
        }
    }
}

impl std::error::Error for Error {
    /// The I/O error behind [`Error::Io`]; the other variants carry their whole
    /// cause in their message.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

/// Crate result alias.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_io_error_is_the_source() {
        use std::error::Error as _;
        let e: Error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "no").into();
        let source = e.source().expect("a source");
        let io = source
            .downcast_ref::<std::io::Error>()
            .expect("an io::Error");
        assert_eq!(io.kind(), std::io::ErrorKind::PermissionDenied);
        assert!(Error::BadSize("0x0".into()).source().is_none());
    }
}
