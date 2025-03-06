use thiserror::Error;

#[derive(Error, Debug)]
pub enum TuariError {
    #[error("unable to fetch from data store")]
    QueryError(#[from] rusqlite::Error),
}

impl serde::Serialize for TuariError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
