use std::{process::{Command, Child}, io::Write, path::PathBuf, fs::File};

use base64::{decode, encode};
use druid::{ExtEventSink, Selector, Target};
use interprocess::local_socket::{LocalSocketStream, LocalSocketListener};
use rand::random;
use reqwest::Url;
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

use crate::app::{App, PROJECT};

pub const WEBVIEW_SHUTDOWN: Selector = Selector::new("webview.shutdown");
pub const WEBVIEW_INSTALL: Selector<InstallType> = Selector::new("webview.install");

#[derive(Clone)]
pub enum InstallType {
  Uri(String),
  Path(PathBuf)
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum WebviewMessage {
  Navigation(String),
  Download(String),
  Allow,
  Deny,
  Shutdown,
  BlobFile(PathBuf),
}

#[derive(Debug)]
enum UserEvent {
  Navigation(String),
  NewWindow(String),
  AskDownload(String),
  Download(String),
  BlobReceived(String),
  BlobChunk(Option<String>)
}

pub fn init_webview() -> wry::Result<()> {
  let event_loop: EventLoop<UserEvent> = EventLoop::with_user_event();
  let proxy = event_loop.create_proxy();

  let mut menu_bar = MenuBar::new();
  let back = menu_bar.add_item(MenuItemAttributes::new("< Back"));
  let forward = menu_bar.add_item(MenuItemAttributes::new("Forward >"));

  let window = WindowBuilder::new()
    .with_title("MOSS | Browser")
    .with_menu(menu_bar)
    .build(&event_loop)?;

  let mut webcontext = WebContext::default();
  webcontext.set_allows_automation(true);

  let webview = WebViewBuilder::new(window)?
    .with_url("https://fractalsoftworks.com/forum/index.php?topic=177.0")?
    .with_initialization_script(r"
      // Adds an URL.getFromObjectURL( <blob:// URI> ) method
      // returns the original object (<Blob> or <MediaSource>) the URI points to or null
      (() => {
        // overrides URL methods to be able to retrieve the original blobs later on
        const old_create = URL.createObjectURL;
        const old_revoke = URL.revokeObjectURL;
        Object.defineProperty(URL, 'createObjectURL', {
          get: () => storeAndCreate
        });
        Object.defineProperty(URL, 'revokeObjectURL', {
          get: () => forgetAndRevoke
        });
        Object.defineProperty(URL, 'getFromObjectURL', {
          get: () => getBlob
        });
        Object.defineProperty(URL, 'getObjectURLDict', {
          get: () => getDict
        });
        Object.defineProperty(URL, 'clearURLDict', {
          get: () => clearDict
        });
        const dict = {};
      
        function storeAndCreate(blob) {
          const url = old_create(blob); // let it throw if it has to
          dict[url] = blob;
          console.log(blob)
          return url
        }
      
        function forgetAndRevoke(url) {
          console.log(`revoke ${url}`)
          old_revoke(url);
        }
      
        function getBlob(url) {
          return dict[url] || null;
        }

        function getDict() {
          return dict;
        }

        function clearDict() {
          dict = {};
        }
      })();
    ")
    .with_ipc_handler({
      let proxy = proxy.clone();
      move |_, string| {
        match string.as_str() {
          _ if string.starts_with("data:") => {
            let _ = proxy.send_event(UserEvent::BlobChunk(Some(string)));
          },
          "#EOF" => {
            let _ = proxy.send_event(UserEvent::BlobChunk(None));
          },
          _ if string.starts_with("confirm_download") => {
            let mut parts = string.split(',');
            let confirm = parts.next().expect("split ipc").split(":").nth(1).expect("split ipc");
            if confirm == "true" {
              let base = parts.next().expect("split ipc").split(":").nth(1).expect("split ipc");
              let decoded = decode(base).expect("decode uri");
              let uri = String::from_utf8(decoded).expect("decode");
              let _ = proxy.send_event(UserEvent::Download(uri));
            }
          },
          _ => {}
        }
      }
    })
    .with_navigation_handler({
      let proxy = proxy.clone();
      move |uri: String| {
        if &uri == "about:blank" {
          return false
        }

        if let Ok(url) = Url::parse(&uri) {
          if url.host_str() == Some("drive.google.com") && url.query().map_or(false, |q| q.contains("export=download")) {
            let _ = proxy.send_event(UserEvent::AskDownload(uri.clone()));
            return false
          }
        }

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
      move |uri: String, _download_to: &mut String| {
        if uri.starts_with("blob:https://mega.nz") {
          let _ = proxy.send_event(UserEvent::BlobReceived(uri));
          return false
        }

        proxy.send_event(UserEvent::AskDownload(uri.clone())).expect("Send event");

        false
      }}, {
      move || Box::new(move |_path, _success| {})
    })
    .build()?;

  #[cfg(debug_assertions)]
  webview.devtool();

  let mut mega_file = None;
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
        if uri.starts_with("https://www.mediafire.com/file") {
          let _ = webview.evaluate_script(r#"window.alert("You appear to be on a Mediafire site.\nIn order to correctly trigger a Mediafire download, attempt to open the dowload link in a new window.\nThis can be done through the right click context menu, or using a platform shortcut.")"#);
        }
      },
      Event::UserEvent(UserEvent::AskDownload(uri)) => {
        let _ = webview.evaluate_script(&format!(r"
          let res = window.confirm('Detected an attempted download.\nDo you want to try and install a mod using this download?')
          window.ipc.postMessage(`confirm_download:${{res}},uri:{}`)
        ", encode(uri)));
      },
      Event::UserEvent(UserEvent::Download(uri)) => {
        webview.window().set_minimized(true);
        bincode::serialize_into(connect(), &WebviewMessage::Download(uri)).expect("");
      },
      Event::UserEvent(UserEvent::NewWindow(uri)) => {
        webview.evaluate_script(&format!("window.location.assign('{}')", uri)).expect("Navigate webview");
      },
      Event::UserEvent(UserEvent::BlobReceived(uri)) => {
        let path = PROJECT.cache_dir().join(format!("{}", random::<u16>()));
        mega_file = Some((File::create(&path).expect("Create file"), path));
        webview.evaluate_script(&format!(r#"
          /**
           * @type Blob
           */
          let blob = URL.getObjectURLDict()['{}']

          var increment = 1024;
          var index = 0;
          var reader = new FileReader();
          let func = function() {{
            let res = reader.result;
            window.ipc.postMessage(`${{res}}`);
            index += increment;
            if (index < blob.size) {{
              let slice = blob.slice(index, index + increment);
              reader = new FileReader();
              reader.onloadend = func;
              reader.readAsDataURL(slice);
            }} else {{
              window.ipc.postMessage('#EOF');
            }}
          }};
          reader.onloadend = func;
          reader.readAsDataURL(blob.slice(index, increment))
        "#, uri)).expect("Eval script");
      },
      Event::UserEvent(UserEvent::BlobChunk(chunk)) => {
        if let Some((file, path)) = mega_file.as_mut() {
          match chunk {
            Some(chunk) => {
              let split = chunk.split(',').nth(1);
              println!("{:?}", chunk.split(',').nth(0));
              if let Some(split) = split {
                if let Ok(decoded) = decode(split) {
                  if file.write(&decoded).is_err() {
                    eprintln!("Failed to write bytes to temp file")
                  }
                }
              }
            },
            None => {
              let _ = bincode::serialize_into(connect(), &WebviewMessage::BlobFile(path.clone()));
              mega_file = None;
            }
          }
        }
      }
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
          WebviewMessage::Navigation(_uri) => {},
          WebviewMessage::Download(uri) => {
            ext_sink.submit_command(WEBVIEW_INSTALL, InstallType::Uri(uri.clone()), Target::Auto).expect("Send install from webview");
          },
          WebviewMessage::Shutdown => {
            ext_sink.submit_command(WEBVIEW_SHUTDOWN, (), Target::Auto).expect("Remove child ref from parent");
            ext_sink.submit_command(App::ENABLE, (), Target::Auto).expect("Re-enable");
            break;
          },
          WebviewMessage::BlobFile(file) => {
            ext_sink.submit_command(WEBVIEW_INSTALL, InstallType::Path(file.clone()), Target::Auto).expect("Send install from webview");
          }
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
