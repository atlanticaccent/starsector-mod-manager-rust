use std::sync::{Arc, Mutex};

use derive_more::derive::{Deref, From};
use tokio::sync::oneshot;

#[derive(Debug, Clone, From, Deref)]
pub struct CloneTx(Arc<Mutex<Option<oneshot::Sender<bool>>>>);

impl CloneTx {
  pub fn new(tx: oneshot::Sender<bool>) -> Self {
    Self(Arc::new(Mutex::new(Some(tx))))
  }

  pub fn send(&self, val: bool) {
    let Ok(mut guard) = self.lock() else {
      return;
    };

    if let Some(sender) = guard.take() {
      let _ = sender.send(val);
    }
  }
}

impl PartialEq for CloneTx {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(self, other)
  }
}