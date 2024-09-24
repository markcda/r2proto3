use std::error::Error;
use std::fmt::{Display, Formatter};
use std::ops::Deref;

#[derive(Debug)]
// NOTE: ToProtobuf
pub(crate) struct R2Proto3Error {
  cause: Option<Box<dyn Error>>,
  description: String,
}

impl PartialEq for R2Proto3Error {
  fn eq(&self, other: &Self) -> bool {
    self.description.eq(&other.description)
  }
  
  fn ne(&self, other: &Self) -> bool {
    self.description.ne(&other.description)
  }
}

impl R2Proto3Error {
  pub(crate) fn new(
    cause: Option<Box<dyn Error>>,
    description: impl Into<String>,
  ) -> Self {
    Self {
      cause: match cause {
        Some(cause) => Some(cause.into()),
        None => None,
      },
      description: description.into(),
    }
  }
}

impl Display for R2Proto3Error {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match &self.cause {
      None => write!(f, "{}", self.description),
      Some(cause) => write!(f, "{} :: cause of = {}", self.description, cause),
    }
  }
}

impl Error for R2Proto3Error {
  fn source(&self) -> Option<&(dyn Error + 'static)> { None }
  fn description(&self) -> &str { &self.description }
  fn cause(&self) -> Option<&dyn Error> {
    self.cause.as_ref().map(|e| e.deref())
  }
}

pub(crate) type MResult<T> = std::result::Result<T, R2Proto3Error>;

#[allow(unused)]
// NOTE: ToProtobuf
pub struct TestStruct(i32);
