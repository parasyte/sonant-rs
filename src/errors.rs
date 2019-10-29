//! Utilities for working with `std::error::Error` on stable Rust. (Requires `std` feature.)

pub struct ErrorIter<'a> {
    inner: Option<&'a (dyn std::error::Error + 'static)>,
}

/// Create an iterator over the chained error sources.
///
/// `dyn Error iter_sources()` is a nightly-only feature. Approximate it so we can build on stable.
///
/// ```
/// use sonant::errors::iter_sources;
/// # use std::io::{Error, ErrorKind};
///
/// # let error = Error::new(ErrorKind::Other, "oh no!");
/// eprintln!("Error: {}", error);
/// for source in iter_sources(&error) {
///     eprintln!("Caused by: {}", source);
/// }
/// ```
pub fn iter_sources<E>(error: &E) -> ErrorIter
where
    E: std::error::Error + 'static,
{
    ErrorIter { inner: Some(error) }
}

impl<'a> Iterator for ErrorIter<'a> {
    type Item = &'a (dyn std::error::Error + 'static);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(error) = self.inner.take() {
            if let Some(source) = error.source() {
                self.inner = Some(source);
                return Some(source);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use thiserror::Error;

    #[derive(Debug, Error)]
    enum Error {
        #[error("Nested error: {0}")]
        Nested(#[source] Box<Error>),

        #[error("Leaf error")]
        Leaf,
    }

    #[test]
    fn iter_sources_ok() {
        let error = Error::Nested(Box::new(Error::Leaf));

        let mut counter = 0;

        for source in iter_sources(&error) {
            counter += 1;
            assert_eq!(format!("{}", source), "Leaf error");
        }

        assert_eq!(counter, 1);
    }
}
