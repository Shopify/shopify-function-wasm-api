#[repr(usize)]
#[derive(Debug, strum::FromRepr, PartialEq, Eq)]
pub enum LogResult {
    /// The log operation was successful.
    Ok = 0,
}
