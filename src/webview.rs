use std::process::{Command, Child};

use druid::{ExtEventSink, Selector, Target};
use interprocess::local_socket::{LocalSocketStream, LocalSocketListener};
use serde::{Serialize, Deserialize};
use tokio::runtime::Handle;
use wry::{
  application::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder, menu::{MenuBar, MenuItemAttributes, MenuType},
  },
  webview::{WebViewBuilder, WebContext},
};

use crate::app::App;

pub const WEBVIEW_SHUTDOWN: Selector = Selector::new("webview.shutdown");

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum WebviewMessage {
  Navigation(String),
  Download(String),
  Allow,
  Deny,
  Shutdown,
}

#[derive(Debug)]
enum UserEvent {
  Navigation(String),
  NewWindow(String),
  Download(String),
}

pub fn init_webview() -> wry::Result<()> {
  let event_loop: EventLoop<UserEvent> = EventLoop::with_user_event();
  let proxy = event_loop.create_proxy();

  let mut menu_bar = MenuBar::new();
  let back = menu_bar.add_item(MenuItemAttributes::new("<"));
  let forward = menu_bar.add_item(MenuItemAttributes::new(">"));

  let window = WindowBuilder::new()
    .with_title("MOSS | Browser")
    .with_menu(menu_bar)
    .build(&event_loop)?;

  let mut webcontext = WebContext::default();
  webcontext.set_allows_automation(true);

  let webview = WebViewBuilder::new(window)?
    .with_url("https://fractalsoftworks.com/forum/index.php?topic=177.0")?
    .with_navigation_handler({
      let proxy = proxy.clone();
      move |uri: String| {
        let submitted = proxy.send_event(UserEvent::Navigation(uri.clone())).is_ok();

        submitted
      }
    })
    .with_new_window_req_handler({
      let proxy = proxy.clone();
      move |uri: String| {
        proxy.send_event(UserEvent::NewWindow(uri.clone())).expect("Send event");

        false
      }
    })
    .with_download_handler({
      let proxy = proxy.clone();
      move |uri: String| {
        proxy.send_event(UserEvent::Download(uri.clone())).expect("Send event");

        false
      }
    })
    .build()?;

  #[cfg(debug_assertions)]
  webview.devtool();

  let connect = || {
    LocalSocketStream::connect("@/tmp/moss.sock").expect("Connect socket")
  };
  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::NewEvents(StartCause::Init) => println!("Wry has started!"),
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => {
        bincode::serialize_into(connect(), &WebviewMessage::Shutdown).expect("");
        *control_flow = ControlFlow::Exit
      },
      Event::MenuEvent {
        menu_id,
        origin: MenuType::MenuBar,
        ..
      } => {
        if menu_id == forward.clone().id() {
          webview.evaluate_script("window.history.forward()").expect("Go forward in webview history");
        } else if menu_id == back.clone().id() {
          webview.evaluate_script("window.history.back()").expect("Go back in webview history");
        }
        println!("Clicked on {:?}", menu_id);
      }
      Event::UserEvent(UserEvent::Navigation(uri)) => {
        println!("Navigation: {}", uri);
      },
      Event::UserEvent(UserEvent::Download(uri)) => {
        println!("Download: {}", uri);
        bincode::serialize_into(connect(), &WebviewMessage::Download(uri)).expect("");
      },
      Event::UserEvent(UserEvent::NewWindow(uri)) => {
        println!("New Window: {}", uri);
        webview.evaluate_script(&format!("window.location.assign('{}')", uri)).expect("Navigate webview");
      },
      _ => {
        let _ = webview.resize();
      }
    }
  });
}

pub fn fork_into_webview(handle: &Handle, ext_sink: ExtEventSink) -> Child {
  let exe = std::env::current_exe().expect("Get current executable path");
  fn handle_error(conn: std::io::Result<LocalSocketStream>) -> Option<LocalSocketStream> {
    match conn {
      Ok(val) => Some(val),
      Err(error) => {
        eprintln!("Incoming connection failed: {}", error);
        None
      }
    }
  }

  let listener = LocalSocketListener::bind("@/tmp/moss.sock").expect("Open socket");

  handle.spawn_blocking(move || {
    let allow = None;

    for conn in listener.incoming().filter_map(handle_error) {
      if let Some(allow) = allow {
        bincode::serialize_into(
          conn,
          if allow {
            &WebviewMessage::Allow
          } else {
            &WebviewMessage::Deny
          }
        ).expect("Write out");
      } else {
        let message: WebviewMessage = bincode::deserialize_from(conn).expect("Read from");
        match &message {
          WebviewMessage::Navigation(uri) => {},
          WebviewMessage::Download(uri) => {},
          WebviewMessage::Shutdown => {
            ext_sink.submit_command(WEBVIEW_SHUTDOWN, (), Target::Auto).expect("Remove child ref from parent");
            ext_sink.submit_command(App::ENABLE, (), Target::Auto).expect("Re-enable");
            break;
          },
          _ => {}
        }
        println!("Client answered: {:?}", message);
      }
    }
  });

  Command::new(exe)
    .arg("--webview")
    .spawn()
    .expect("Failed to start child process")
}

pub fn kill_server_thread() {
  let socket = LocalSocketStream::connect("@/tmp/moss.sock").expect("Connect socket");
  bincode::serialize_into(socket, &WebviewMessage::Shutdown).expect("");
}
