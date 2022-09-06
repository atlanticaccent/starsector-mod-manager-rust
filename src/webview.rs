use std::process::{Child, Command};

use druid::{ExtEventSink, Selector, Target};
use interprocess::local_socket::LocalSocketListener;
use tap::Pipe;
use tokio::runtime::Handle;
use webview_shared::{
  connect_child, connect_parent, handle_error, InstallType, WebviewMessage, PARENT_CHILD_SOCKET,
};

pub const WEBVIEW_SHUTDOWN: Selector = Selector::new("webview.shutdown");
pub const WEBVIEW_INSTALL: Selector<InstallType> = Selector::new("webview.install");
pub const ENABLE: Selector<()> = Selector::new("app.enable");

pub fn fork_into_webview(handle: &Handle, ext_sink: ExtEventSink, url: Option<String>) -> Child {
  let exe = std::env::current_exe().expect("Get current executable path");

  let listener = LocalSocketListener::bind(PARENT_CHILD_SOCKET).expect("Open socket");

  handle.spawn_blocking(move || {
    for conn in listener.incoming().filter_map(handle_error) {
      let message: WebviewMessage = bincode::deserialize_from(conn).expect("Read from");
      match &message {
        WebviewMessage::Navigation(_uri) => {}
        WebviewMessage::Download(uri) => {
          ext_sink
            .submit_command(WEBVIEW_INSTALL, InstallType::Uri(uri.clone()), Target::Auto)
            .expect("Send install from webview");
        }
        WebviewMessage::Shutdown => {
          ext_sink
            .submit_command(WEBVIEW_SHUTDOWN, (), Target::Auto)
            .expect("Remove child ref from parent");
          ext_sink
            .submit_command(ENABLE, (), Target::Global)
            .expect("Re-enable");
          #[cfg(not(target_family = "windows"))]
          let _ = std::fs::remove_file(webview_shared::PARENT_CHILD_PATH);
          break;
        }
        WebviewMessage::BlobFile(file) => {
          ext_sink
            .submit_command(
              WEBVIEW_INSTALL,
              InstallType::Path(file.clone()),
              Target::Auto,
            )
            .expect("Send install from webview");
        }
        _ => {}
      }
      println!("Client answered: {:?}", message);
    }
  });

  Command::new(exe)
    .arg("--webview")
    .pipe(|cmd| {
      if let Some(url) = url {
        cmd.arg(&url)
      } else {
        cmd
      }
    })
    .spawn()
    .expect("Failed to start child process")
}

pub fn kill_server_thread() {
  let socket = connect_parent().unwrap();
  bincode::serialize_into(socket, &WebviewMessage::Shutdown).unwrap();
  let socket = connect_child().unwrap();
  bincode::serialize_into(socket, &WebviewMessage::Shutdown).unwrap();
}

pub fn minimize_webview() {
  if let Ok(socket) = connect_child() {
    let _ = bincode::serialize_into(socket, &WebviewMessage::Minimize);
  }
}

pub fn maximize_webview() {
  if let Ok(socket) = connect_child() {
    let _ = bincode::serialize_into(socket, &WebviewMessage::Maximize);
  }
}
