use base64::decode;
use druid::{ExtEventSink, WindowHandle};
use url::Url;
use webview_shared::{ExtEventSinkExt, WebviewEvent, FRACTAL_INDEX, WEBVIEW_EVENT};
use wry::{WebContext, WebView, WebViewBuilder};

pub fn init_webview_with_handle(
  url: Option<String>,
  parent: &WindowHandle,
  ext_ctx: ExtEventSink,
) -> wry::Result<WebView> {
  init_webview(url, WebViewBuilder::new_as_child(parent), ext_ctx)
}

pub fn init_webview(
  url: Option<String>,
  builder: WebViewBuilder,
  ext_ctx: ExtEventSink,
) -> wry::Result<WebView> {
  let mut webcontext = WebContext::default();
  webcontext.set_allows_automation(true);

  let init_script = include_str!("init.js");

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
              let decoded = decode(base).expect("decode uri");
              let uri = String::from_utf8(decoded).expect("decode");
              let _ = ext_ctx.submit_command_global(WEBVIEW_EVENT, WebviewEvent::Download(uri));
            } else {
              let _ = ext_ctx.submit_command_global(WEBVIEW_EVENT, WebviewEvent::CancelDownload);
            }
          }
          _ => {}
        }
      }
    })
    .with_navigation_handler({
      let ext_ctx = ext_ctx.clone();
      move |uri: String| {
        if &uri == "about:blank" {
          return false;
        }

        if let Ok(url) = Url::parse(&uri) {
          if url.host_str() == Some("drive.google.com")
            && url.query().map_or(false, |q| q.contains("export=download"))
          {
            let _ = ext_ctx
              .submit_command_global(WEBVIEW_EVENT, WebviewEvent::AskDownload(uri + "&confirm=t"));
            return false;
          }
        }

        ext_ctx
          .submit_command_global(WEBVIEW_EVENT, WebviewEvent::Navigation(uri))
          .is_ok()
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
    .with_on_page_load_handler({
      let ext_ctx = ext_ctx.clone();
      move |_, _| {
        ext_ctx
          .submit_command_global(WEBVIEW_EVENT, WebviewEvent::PageLoaded)
          .expect("Send event");
      }
    })
    .build()?;

  #[cfg(all(debug_assertions, not(target_os = "macos")))]
  webview.open_devtools();

  Ok(webview)
}
