use std::io;

/// Falhas exibidas na UI; as mensagens são em inglês (design.md, "UI").
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the managed block is missing, duplicated or malformed")]
    MalformedBlock,

    #[error("entry outside any group: {0}")]
    EntryOutsideGroup(String),

    #[error("invalid IP address: {0}")]
    InvalidIp(String),

    #[error("invalid hostname: {0}")]
    InvalidHostname(String),

    #[error("invalid group name: {0}")]
    InvalidGroupName(String),

    #[error("the file is not valid UTF-8")]
    NotUtf8,

    #[error("the content is larger than {0} bytes")]
    TooLarge(usize),

    #[error("the hosts file changed on disk since it was loaded")]
    HashMismatch,

    #[error("{0}")]
    Io(#[from] io::Error),

    #[error("{0}")]
    Time(#[from] time::error::Format),
}
