//! `dyn Error iter_sources()` is a nightly-only feature. Approximate it so we can build on stable.

pub struct ErrorIter<'a> {
    inner: Option<&'a (dyn std::error::Error + 'static)>,
}

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
