use std::{fs::File, io::Write};

use base64::{decode, encode};
use interprocess::local_socket::LocalSocketListener;
use rand::random;
use url::Url;
use webview_shared::{
  connect_child, connect_parent, handle_error, WebviewMessage, CHILD_PARENT_SOCKET, PROJECT,
};
use wry::{
  WebContext, WebViewBuilder,
};

const FRACTAL_INDEX: &str = "https://fractalsoftworks.com/forum/index.php?topic=177.0";
const FRACTAL_MODS_FORUM: &str = "https://fractalsoftworks.com/forum/index.php?board=8.0";
const FRACTAL_MODDING_SUBFORUM: &str = "https://fractalsoftworks.com/forum/index.php?board=3.0";

#[derive(Debug)]
enum UserEvent {
  Navigation(String),
  NewWindow(String),
  AskDownload(String),
  Download(String),
  CancelDownload,
  BlobReceived(String),
  BlobChunk(Option<String>),
  Maximize,
  Minimize,
}

pub fn init_webview(url: Option<String>) -> wry::Result<()> {
  let event_loop: EventLoop<UserEvent> = EventLoop::with_user_event();
  let proxy = event_loop.create_proxy();

  let runtime = tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .expect("Build tokio runtime");

  runtime.spawn_blocking({
    let proxy = proxy.clone();
    move || {
      let listener = LocalSocketListener::bind(CHILD_PARENT_SOCKET).expect("Open socket");

      for conn in listener.incoming().filter_map(handle_error) {
        let message: WebviewMessage =
          bincode::deserialize_from(conn).expect("Read from connection");
        match message {
          WebviewMessage::Maximize => {
            let _ = proxy.send_event(UserEvent::Maximize);
          }
          WebviewMessage::Minimize => {
            let _ = proxy.send_event(UserEvent::Minimize);
          }
          WebviewMessage::Shutdown => {
            println!("shutting down");
            #[cfg(not(target_family = "windows"))]
            let _ = std::fs::remove_file(webview_shared::CHILD_PARENT_PATH);
            break;
          }
          _ => {}
        }
      }
    }
  });

  let mut menu_bar = MenuBar::new();
  let back = menu_bar.add_item(MenuItemAttributes::new("< Back"));
  let forward = menu_bar.add_item(MenuItemAttributes::new("Forward >"));

  menu_bar.add_item(MenuItemAttributes::new("|").with_enabled(false));

  let mut bookmarks = MenuBar::new();
  let mod_index = bookmarks.add_item(MenuItemAttributes::new("Mod Index"));
  let mods_forum = bookmarks.add_item(MenuItemAttributes::new("Mods Forum"));
  let modding_subforum = bookmarks.add_item(MenuItemAttributes::new("Modding Sub-Forum"));
  let cursed_discord = bookmarks.add_item(MenuItemAttributes::new("Starsector Discord"));
  menu_bar.add_submenu("Bookmarks", true, bookmarks);

  let window = WindowBuilder::new()
    .with_title("MOSS | Browser")
    .with_menu(menu_bar)
    .build(&event_loop)?;

  let mut webcontext = WebContext::default();
  webcontext.set_allows_automation(true);

  let init_script = include_str!("init.js");

  let webview = WebViewBuilder::new(window)?
    .with_url(url.as_deref().unwrap_or(FRACTAL_INDEX))?
    .with_initialization_script(init_script)
    .with_ipc_handler({
      let proxy = proxy.clone();
      move |_, string| match dbg!(string.as_str()) {
        _ if string.starts_with("data:") => {
          let _ = proxy.send_event(UserEvent::BlobChunk(Some(string)));
        }
        "#EOF" => {
          let _ = proxy.send_event(UserEvent::BlobChunk(None));
        }
        _ if string.starts_with("confirm_download") => {
          let mut parts = string.split(',');
          let confirm = parts
            .next()
            .expect("split ipc")
            .split(':')
            .nth(1)
            .expect("split ipc");
          if confirm == "true" {
            let base = parts
              .next()
              .expect("split ipc")
              .split(':')
              .nth(1)
              .expect("split ipc");
            let decoded = decode(base).expect("decode uri");
            let uri = String::from_utf8(decoded).expect("decode");
            let _ = proxy.send_event(UserEvent::Download(uri));
          } else {
            let _ = proxy.send_event(UserEvent::CancelDownload);
          }
        }
        _ => {}
      }
    })
    .with_navigation_handler({
      let proxy = proxy.clone();
      move |uri: String| {
        if &uri == "about:blank" {
          return false;
        }

        if let Ok(url) = Url::parse(&uri) {
          if url.host_str() == Some("drive.google.com")
            && url.query().map_or(false, |q| q.contains("export=download"))
          {
            let _ = proxy.send_event(UserEvent::AskDownload(uri + "&confirm=t"));
            return false;
          }
        }

        proxy.send_event(UserEvent::Navigation(uri)).is_ok()
      }
    })
    .with_new_window_req_handler({
      let proxy = proxy.clone();
      move |uri: String| {
        proxy
          .send_event(UserEvent::NewWindow(uri))
          .expect("Send event");

        false
      }
    })
    .with_download_started_handler({
      let proxy = proxy;
      move |uri, _| {
        if uri.starts_with("blob:https://mega.nz") {
          let _ = proxy.send_event(UserEvent::BlobReceived(uri));
          return false;
        }

        proxy
          .send_event(UserEvent::AskDownload(uri))
          .expect("Send event");

        false
      }
    })
    .with_devtools(true)
    .build()?;

  #[cfg(debug_assertions)]
  webview.open_devtools();

  let mut mega_file = None;
  let connect = || connect_parent().expect("Connect");
  event_loop.run(move |event, _, control_flow| {
      *control_flow = ControlFlow::Wait;

      match event {
        Event::NewEvents(StartCause::Init) => println!("Wry has started!"),
        Event::WindowEvent {
          event: WindowEvent::CloseRequested,
          ..
        } => {
          bincode::serialize_into(connect(), &WebviewMessage::Shutdown).expect("");
          let socket = connect_child().expect("Connect");
          bincode::serialize_into(socket, &WebviewMessage::Shutdown).expect("");
          *control_flow = ControlFlow::Exit
        },
        Event::MenuEvent {
          menu_id,
          origin: MenuType::MenuBar,
          ..
        } => {
          match menu_id {
            _ if menu_id == forward.clone().id() => webview.evaluate_script("window.history.forward()").expect("Go forward in webview history"),
            _ if menu_id == back.clone().id() => webview.evaluate_script("window.history.back()").expect("Go back in webview history"),
            _ if menu_id == mod_index.clone().id() => webview.evaluate_script(&format!("window.location.assign('{}')", FRACTAL_INDEX)).expect("Navigate webview"),
            _ if menu_id == mods_forum.clone().id() => webview.evaluate_script(&format!("window.location.assign('{}')", FRACTAL_MODS_FORUM)).expect("Navigate webview"),
            _ if menu_id == modding_subforum.clone().id() => webview.evaluate_script(&format!("window.location.assign('{}')", FRACTAL_MODDING_SUBFORUM)).expect("Navigate webview"),
            _ if menu_id == cursed_discord.clone().id() => webview.evaluate_script(&format!("window.location.assign('{}')", "https://discord.com/channels/187635036525166592/825068217361760306")).expect("Navigate webview"),
            _ => {}
          }
          println!("Clicked on {:?}", menu_id);
        },
        Event::UserEvent(user_event) => match user_event {
          UserEvent::Navigation(uri) => {
            println!("Navigation: {}", uri);
            if uri.starts_with("https://www.mediafire.com/file") {
              let _ = webview.evaluate_script(r#"window.alert("You appear to be on a Mediafire site.\nIn order to correctly trigger a Mediafire download, attempt to open the dowload link in a new window.\nThis can be done through the right click context menu, or using a platform shortcut.")"#);
            }
          },
          UserEvent::AskDownload(uri) => {
            #[cfg(not(target_os = "macos"))]
            let _ = webview.evaluate_script(&format!(r"
            let res = window.confirm('Detected an attempted download.\nDo you want to try and install a mod using this download?')
            window.ipc.postMessage(`confirm_download:${{res}},uri:{}`)
            ", encode(uri)));
            #[cfg(target_os = "macos")]
            let _ = webview.evaluate_script(&format!(r"
            let dialog = new Dialog();
            let res = dialog.confirm('Detected an attempted download.\nDo you want to try and install a mod using this download?', {{}})
              .then(res => window.ipc.postMessage(`confirm_download:${{res}},uri:{}`))
            ", encode(uri)));
          },
          UserEvent::Download(uri) => {
            let _ = webview.evaluate_script("location.reload();");
            bincode::serialize_into(connect(), &WebviewMessage::Download(uri)).expect("Send message to manager");
          },
          UserEvent::CancelDownload => {},
          UserEvent::NewWindow(uri) => {
            webview.evaluate_script(&format!("window.location.assign('{}')", uri)).expect("Navigate webview");
          },
          UserEvent::BlobReceived(uri) => {
            let path = PROJECT.cache_dir().join(format!("{}", random::<u16>()));
            mega_file = Some((File::create(&path).expect("Create file"), path));
            webview.evaluate_script(&format!(r#"
            (() => {{
              /**
              * @type Blob
              */
              let blob = URL.getObjectURLDict()['{}']
                || Object.values(URL.getObjectURLDict())[0]

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
            }})();
            "#, uri)).expect("Eval script");
          },
          UserEvent::BlobChunk(chunk) => {
            if let Some((file, path)) = mega_file.as_mut() {
              match chunk {
                Some(chunk) => {
                  let split = chunk.split(',').nth(1);
                  println!("{:?}", chunk.split(',').next());
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
          },
          UserEvent::Maximize => {
            webview.window().set_minimized(false)
          },
          UserEvent::Minimize => {
            webview.window().set_minimized(true)
          }
        }
        _ => ()
      }
    });
}
