use std::fmt::Debug;

use crate::Entry;

#[derive(Debug, thiserror::Error)]
pub enum InstallError<ENT: Entry> {
  #[error("Failed to find mod")]
  ModSearchError(#[source] std::io::Error),
  #[error("Unknown failure to find mod")]
  ModSearchErrorUnknown,
  #[error("Entry parsing error")]
  EntryParsingError(#[source] <ENT as Entry>::ParseError),
  #[error("Entry enrichment error")]
  EntryEnrichmentError(#[source] <ENT as Entry>::EnrichmentError),
  #[error("I/O error: {0:?}")]
  Io(#[from] std::io::Error),
  #[error("Failed to determine file type")]
  Mime,
  #[error("Libarchive error: {0:?}")]
  CompressTools(#[from] compress_tools::Error),
  #[error("Error in Unrar rar decompression lib: {0:?}")]
  Unrar(String),
  #[error("Generic network error: {0:?}")]
  Network(#[from] reqwest::Error),
  #[error("Task timed out")]
  Timeout(#[from] tokio::time::error::Elapsed),
  #[error("Failed to join task/thread: {0:?}")]
  Join(#[from] tokio::task::JoinError),
  #[error("Multiple errors")]
  MultipleErrors(Vec<InstallError<ENT>>),
  #[error(transparent)]
  Generic(#[from] anyhow::Error),
}
