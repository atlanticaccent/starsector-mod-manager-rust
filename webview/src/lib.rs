use std::{cell::OnceCell, path::PathBuf, rc::Rc, str::FromStr};

use base64::{engine::general_purpose::STANDARD, Engine};
use common::ExtEventSinkExt;
use druid::{ExtEventSink, WindowHandle};
use mime::Mime;
use serde::{Deserialize, Serialize};
use url::Url;
use wry::{WebContext, WebView, WebViewBuilder};

mod consts;
mod links;

pub use consts::*;

pub fn init_webview_with_handle(
  url: Option<String>,
  parent: &WindowHandle,
  ext_ctx: ExtEventSink,
) -> wry::Result<Rc<WebView>> {
  #[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
  ))]
  {
    use gtk::{
      glib::translate::{FromGlibPtrFull, ToGlibPtr},
      prelude::*,
    };
    use wry::{WebViewBuilder, WebViewBuilderExtUnix};

    let window = parent.get_gtk_application_window();
    let bin: &gtk::Bin = window.upcast_ref();
    let child = bin.child().unwrap();
    let vbox: &gtk::Box = child.downcast_ref().unwrap();

    eprintln!("{}", vbox.type_());
    eprintln!("{:?}", vbox.children());

    let child_ref: *const _ = child.to_glib_full();
    let container: &gtk::Container = window.upcast_ref();

    container.remove(&child);

    let overlay = gtk::Overlay::new();
    overlay.add(&child);
    unsafe {
      drop(gtk::glib::object::ObjectRef::from_glib_full(
        child_ref as *mut _,
      ))
    };

    let fixed = gtk::Fixed::new();
    overlay.add_overlay(&fixed);
    overlay.set_overlay_pass_through(&fixed, true);
    fixed.show_all();

    overlay.show_all();

    container.add(&overlay);

    let builder = WebViewBuilder::new_gtk(&fixed);

    return init_webview(url, builder, ext_ctx);
  };
  #[cfg(not(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
  )))]
  init_webview(url, WebViewBuilder::new_as_child(parent), ext_ctx)
}

pub fn init_webview(
  url: Option<String>,
  builder: WebViewBuilder,
  ext_ctx: ExtEventSink,
) -> wry::Result<Rc<WebView>> {
  let mut webcontext = WebContext::default();
  webcontext.set_allows_automation(true);

  let init_script = include_str!("init.js");

  let webview_ref: Rc<OnceCell<Rc<WebView>>> = Rc::new(OnceCell::new());

  let webview = builder
    .with_url(url.as_deref().unwrap_or(FRACTAL_INDEX))
    .with_bounds(wry::Rect {
      position: wry::dpi::Position::Logical((0., 0.).into()),
      size: wry::dpi::Size::Logical((0, 0).into()),
    })
    .with_initialization_script(init_script)
    .with_ipc_handler({
      let ext_ctx = ext_ctx.clone();
      move |req| {
        let string = req.into_body();
        match string.as_str() {
          _ if string.starts_with("data:") => {
            let _ =
              ext_ctx.submit_command_global(WEBVIEW_EVENT, WebviewEvent::BlobChunk(Some(string)));
          }
          "#EOF" => {
            let _ = ext_ctx.submit_command_global(WEBVIEW_EVENT, WebviewEvent::BlobChunk(None));
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
              let decoded = STANDARD.decode(base).expect("decode uri");
              let uri = String::from_utf8(decoded).expect("decode");
              let _ = ext_ctx.submit_command_global(WEBVIEW_EVENT, WebviewEvent::Download(uri));
            } else {
              let _ = ext_ctx.submit_command_global(WEBVIEW_EVENT, WebviewEvent::CancelDownload);
            }
          }
          _ if string.starts_with("pageLoaded") => {
            let _ = ext_ctx.submit_command_global(WEBVIEW_EVENT, WebviewEvent::PageLoaded);
          }
          _ if string.starts_with("pageUnload") => {
            let _ = ext_ctx.submit_command_global(WEBVIEW_EVENT, WebviewEvent::PageUnloading);
          }
          _ => {}
        }
      }
    })
    .with_navigation_handler({
      let client = reqwest::blocking::Client::new();
      let ext_ctx = ext_ctx.clone();
      let _webview_ref = webview_ref.clone();
      move |uri: String| {
        if &uri == "about:blank" {
          return false;
        }

        let uri = if let Ok(res) = client.head(&uri).send() {
          let final_url = res.url().to_string();

          if let Some(mime) = res
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .and_then(|c| Mime::from_str(c).ok())
          {
            if mime_type_is_archive(&mime) {
              let _ =
                ext_ctx.submit_command_global(WEBVIEW_EVENT, WebviewEvent::AskDownload(final_url));
              return false;
            }
          }

          if let Ok(parsed_url) = Url::parse(&final_url) {
            let mime_guess = mime_guess::from_path(parsed_url.path());
            if let Some(mime) = mime_guess.first() {
              if mime_type_is_archive(&mime) {
                let _ = ext_ctx
                  .submit_command_global(WEBVIEW_EVENT, WebviewEvent::AskDownload(final_url));
                return false;
              }
            }
          }

          final_url
        } else {
          uri
        };

        if let Some(url) = links::as_direct_download_link(&uri) {
          let _ = ext_ctx.submit_command_global(WEBVIEW_EVENT, WebviewEvent::AskDownload(url));
          false
        } else {
          ext_ctx
            .submit_command_global(WEBVIEW_EVENT, WebviewEvent::Navigation(uri))
            .is_ok()
        }
      }
    })
    .with_new_window_req_handler({
      let ext_ctx = ext_ctx.clone();
      move |uri: String| {
        ext_ctx
          .submit_command_global(WEBVIEW_EVENT, WebviewEvent::NewWindow(uri))
          .expect("Send event");

        false
      }
    })
    .with_download_started_handler({
      let ext_ctx = ext_ctx.clone();
      move |uri, _| {
        if uri.starts_with("blob:https://mega.nz") {
          let _ = ext_ctx.submit_command_global(WEBVIEW_EVENT, WebviewEvent::BlobReceived(uri));
          return false;
        }

        ext_ctx
          .submit_command_global(WEBVIEW_EVENT, WebviewEvent::AskDownload(uri))
          .expect("Send event");

        false
      }
    })
    .build()?;

  #[cfg(debug_assertions)]
  webview.open_devtools();

  let webview = Rc::new(webview);

  let _ = webview_ref.set(webview.clone());

  Ok(webview)
}

fn mime_type_is_archive(mime: &Mime) -> bool {
  match mime.subtype().as_str() {
    // 7-zip
    "x-7z-compressed"
    // Tarball and friends
    | "x-tar"
    | "x-gtar"
    | "x-bzip"
    | "x-bzip2"
    // Zip
    | "x-zip-compressed"
    | "zip"
    // Rar
    | "vnd.rar"
    | "x-rar-compressed" => {
      true
    }
    _ => false
  }
}

#[derive(Clone)]
pub enum InstallType {
  Uri(String),
  Path(PathBuf),
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub enum WebviewMessage {
  Navigation(String),
  Download(String),
  Shutdown,
  BlobFile(PathBuf),
  Maximize,
  Minimize,
}

#[derive(Debug)]
pub enum WebviewEvent {
  Navigation(String),
  NewWindow(String),
  AskDownload(String),
  Download(String),
  CancelDownload,
  BlobReceived(String),
  BlobChunk(Option<String>),
  PageLoaded,
  PageUnloading,
  ShowConfirmPopup(String),
}
