use std::path::PathBuf;
use std::hash::{Hash, Hasher};
use iced_futures::futures::{self, future, StreamExt};
use tokio::sync::mpsc;

// Just a little utility function
pub fn install<I: 'static + Hash + Copy + Send>(
  id: I,
  paths: Vec<PathBuf>,
  mods_dir: PathBuf
) -> iced::Subscription<Progress> {
  iced::Subscription::from_recipe(Installation {
    id,
    paths,
    mods_dir
  })
}

pub struct Installation<I> {
  id: I,
  paths: Vec<PathBuf>,
  mods_dir: PathBuf
}

// Make sure iced can use our download stream
impl<H, I, T> iced_native::subscription::Recipe<H, I> for Installation<T>
where
  T: 'static + Hash + Copy + Send,
  H: Hasher,
{
  type Output = Progress;

  fn hash(&self, state: &mut H) {
    struct Marker;
    std::any::TypeId::of::<Marker>().hash(state);

    self.id.hash(state);
  }

  fn stream(
    self: Box<Self>,
    _input: futures::stream::BoxStream<'static, I>,
  ) -> futures::stream::BoxStream<'static, Self::Output> {
    let id = self.id;

    Box::pin(futures::stream::unfold(
      State::Ready(self.paths, self.mods_dir),
      move |state| async move {
        match state {
          State::Ready(paths, mods_dir) => {
            let (tx, rx) = mpsc::unbounded_channel();

            async {
              for path in paths {
                let task_tx = tx.clone();
                let mods_dir = mods_dir.clone();

                tokio::spawn(async move {
                  handle_path(task_tx, path, mods_dir).await;
                });
              }
            }.await;

            Some((
              (id, None),
              State::Installing {
                receiver: rx,
                complete: vec![],
                errored: vec![]
              }
            ))
          }
          State::Installing {
            mut receiver,
            mut complete,
            mut errored
          } => match receiver.recv().await {
            Some(Message::Success(mod_id)) => {
              complete.push(mod_id);

              Some((
                (id, None),
                State::Installing {
                  receiver,
                  complete,
                  errored
                }
              ))
            },
            Some(Message::Duplicate(mod_id, path)) => {
              Some((
                (id, Some(Progress::Query(mod_id, path))),
                State::Installing {
                  receiver,
                  complete,
                  errored
                }
              ))
            },
            Some(Message::Error(mod_id)) => {
              errored.push(mod_id.clone());

              Some((
                (id, Some(Progress::Errored(mod_id))),
                State::Installing {
                  receiver,
                  complete,
                  errored
                }
              ))
            },
            None => {
              Some((
                (id, Some(Progress::Finished(complete, errored))),
                State::Finished
              ))
            }
          },
          State::Finished => {
            None
          }
        }
      },
    ).filter_map(|(_, prog)| future::ready(prog)))
  }
}

async fn handle_path(tx: mpsc::UnboundedSender<Message>, path: PathBuf, mods_dir: PathBuf) {
  if path.clone().to_string_lossy() == "good" {
    tx.send(Message::Success(String::from("good"))).expect("Sending message from task");
  } else if path.clone().to_string_lossy() == "dupe" {
    tx.send(Message::Duplicate(String::from("dupe"), path)).expect("Sending message from task");
  } else {
    tx.send(Message::Error(String::from("error"))).expect("Sending message from task");
  }
}

#[derive(Debug, Clone)]
pub enum Progress {
  Finished(Vec<String>, Vec<String>),
  Query(String, PathBuf),
  Errored(String),
}

pub enum State {
  Ready(Vec<PathBuf>, PathBuf),
  Installing {
    receiver: mpsc::UnboundedReceiver<Message>,
    complete: Vec<String>,
    errored: Vec<String>
  },
  Finished
}

#[derive(Debug, Clone)]
pub enum Message {
  Success(String),
  Duplicate(String, PathBuf),
  Error(String)
}
