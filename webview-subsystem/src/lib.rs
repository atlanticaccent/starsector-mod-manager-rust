use base64::decode;
use druid::{ExtEventSink, WindowHandle};
use url::Url;
use webview_shared::{ExtEventSinkExt, UserEvent, WEBVIEW_EVENT, WEBVIEW_OFFSET, FRACTAL_INDEX};
use wry::{WebContext, WebView, WebViewBuilder};

pub fn init_webview(
  url: Option<String>,
  window: &WindowHandle,
  ext_ctx: ExtEventSink,
) -> wry::Result<WebView> {
  let mut webcontext = WebContext::default();
  webcontext.set_allows_automation(true);

  let init_script = include_str!("init.js");

  let webview = WebViewBuilder::new_as_child(window)
    .with_bounds(wry::Rect {
      x: 0,
      y: WEBVIEW_OFFSET.into(),
      width: window.get_size().width as u32,
      height: (window.get_size().height as u32).saturating_sub(WEBVIEW_OFFSET as u32),
    })
    .with_url(url.as_deref().unwrap_or(FRACTAL_INDEX))?
    .with_initialization_script(init_script)
    .with_ipc_handler({
      let ext_ctx = ext_ctx.clone();
      move |string| match dbg!(string.as_str()) {
        _ if string.starts_with("data:") => {
          let _ = ext_ctx.submit_command_global(WEBVIEW_EVENT, UserEvent::BlobChunk(Some(string)));
        }
        "#EOF" => {
          let _ = ext_ctx.submit_command_global(WEBVIEW_EVENT, UserEvent::BlobChunk(None));
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
            let _ = ext_ctx.submit_command_global(WEBVIEW_EVENT, UserEvent::Download(uri));
          } else {
            let _ = ext_ctx.submit_command_global(WEBVIEW_EVENT, UserEvent::CancelDownload);
          }
        }
        _ => {}
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
              .submit_command_global(WEBVIEW_EVENT, UserEvent::AskDownload(uri + "&confirm=t"));
            return false;
          }
        }

        ext_ctx
          .submit_command_global(WEBVIEW_EVENT, UserEvent::Navigation(uri))
          .is_ok()
      }
    })
    .with_new_window_req_handler({
      let ext_ctx = ext_ctx.clone();
      move |uri: String| {
        ext_ctx
          .submit_command_global(WEBVIEW_EVENT, UserEvent::NewWindow(uri))
          .expect("Send event");

        false
      }
    })
    .with_download_started_handler({
      let ext_ctx = ext_ctx;
      move |uri, _| {
        if uri.starts_with("blob:https://mega.nz") {
          let _ = ext_ctx.submit_command_global(WEBVIEW_EVENT, UserEvent::BlobReceived(uri));
          return false;
        }

        ext_ctx
          .submit_command_global(WEBVIEW_EVENT, UserEvent::AskDownload(uri))
          .expect("Send event");

        false
      }
    })
    .build()?;

  #[cfg(debug_assertions)]
  webview.open_devtools();

  Ok(webview)
}
