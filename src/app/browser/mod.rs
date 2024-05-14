use std::{cell::RefCell, io::Write, rc::Rc};

use base64::{decode, encode};
use druid::{
  widget::{Flex, Label, Maybe},
  Data, Lens, Selector, Widget, WidgetExt,
};
use druid_widget_nursery::WidgetExt as _;
use rand::random;
use webview_shared::{
  ExtEventSinkExt, InstallType, UserEvent, PROJECT, WEBVIEW_EVENT, WEBVIEW_INSTALL,
};
use webview_subsystem::init_webview;
use wry::WebView;

use crate::{
  app::controllers::ExtensibleController,
  nav_bar::{Nav, NavLabel},
  widgets::card::Card,
};

use super::{
  overlays::Popup,
  util::{PrintAndPanic, WidgetExtEx},
};

#[derive(Data, Clone, Lens, Default)]
pub struct Browser {
  pub inner: Option<BrowserInner>,
  url: Option<String>,
}

#[derive(Data, Clone, Lens)]
pub struct BrowserInner {
  pub webview: Rc<WebView>,
  #[data(ignore)]
  mega_file: Option<Rc<RefCell<Vec<u8>>>>,
}

impl Browser {
  pub const WEBVIEW_NAVIGATE: Selector<String> = Selector::new("browser.webview.navigate");
  pub const WEBVIEW_HIDE: Selector = Selector::new("browser.webview.hide");

  pub fn init(&self) -> bool {
    self.inner.is_some()
  }

  pub fn view() -> impl Widget<Browser> {
    const INIT_WEBVIEW: Selector = Selector::new("browser.webview.init");

    Maybe::or_empty(|| {
      Card::builder()
        .with_insets((0.0, 14.0))
        .with_corner_radius(4.0)
        .with_shadow_length(8.0)
        .build(
          Label::new("boo").expand().controller(
            ExtensibleController::new()
              .on_lifecycle(|_, ctx, event, browser: &BrowserInner, _| {
                let (origin, size) = if let druid::LifeCycle::ViewContextChanged(context) = event {
                  let _ = browser
                    .webview
                    .set_visible(true)
                    .inspect_err(|e| eprintln!("{e}"));
                  (context.window_origin, ctx.size())
                } else if let druid::LifeCycle::Size(size) = event {
                  (ctx.window_origin(), size.clone())
                } else {
                  return;
                };

                let _ = browser
                  .webview
                  .set_bounds(wry::Rect {
                    position: wry::dpi::Position::Logical((origin.x, origin.y - 7.).into()),
                    size: wry::dpi::Size::Logical((size.width, size.height + 14.).into()),
                  })
                  .inspect_err(|e| eprintln!("{e}"));
              })
              .on_command(Nav::NAV_SELECTOR, |_, _, payload, browser| {
                let _ = browser
                  .webview
                  .set_visible(*payload == NavLabel::WebBrowser)
                  .inspect_err(|e| eprintln!("{e}"));
                true
              })
              .on_command(super::App::OPEN_WEBVIEW, |_, _, payload, browser| {
                if let Some(url) = payload {
                  let _ = browser
                    .webview
                    .load_url(url)
                    .inspect_err(|e| eprintln!("{e}"));
                }
                let _ = browser
                  .webview
                  .set_visible(true)
                  .inspect_err(|e| eprintln!("{e}"));
                false
              })
              .on_command(Browser::WEBVIEW_HIDE, |_, _, _, data| {
                data.webview.set_visible(false);
                true
              })
              .on_command(WEBVIEW_EVENT, Browser::handle_webview_events),
          ),
        )
    })
    .lens(Browser::inner)
    .on_command(Nav::NAV_SELECTOR, |ctx, payload, data| {
      if *payload == NavLabel::WebBrowser && data.inner.is_none() {
        ctx.submit_command(INIT_WEBVIEW)
      }
    })
    .on_command2(INIT_WEBVIEW, |_, ctx, _, data| {
      if let Ok(webview) = init_webview(data.url.clone(), ctx.window(), ctx.get_external_handle()) {
        let _ = webview.set_visible(false).inspect_err(|e| eprintln!("{e}"));
        data.inner = Some(BrowserInner {
          webview: Rc::new(webview),
          mega_file: None,
        });
      }
      ctx.request_update();
      false
    })
    .on_command2(super::App::OPEN_WEBVIEW, |_, ctx, payload, data| {
      ctx.submit_command(Nav::NAV_SELECTOR.with(NavLabel::WebBrowser));
      data.url = payload.clone();
      true
    })
    .expand()
  }

  fn handle_webview_events(
    _w: &mut impl Widget<BrowserInner>,
    ctx: &mut druid::EventCtx,
    user_event: &UserEvent,
    BrowserInner { webview, mega_file }: &mut BrowserInner,
  ) -> bool {
    match user_event {
      UserEvent::Navigation(uri) => {
        println!("Navigation: {}", uri);
        if uri.starts_with("https://www.mediafire.com/file") {
          let _ = webview.evaluate_script(r#"window.alert("You appear to be on a Mediafire site.\nIn order to correctly trigger a Mediafire download, attempt to open the dowload link in a new window.\nThis can be done through the right click context menu, or using a platform shortcut.")"#);
        }
      }
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
      }
      UserEvent::Download(uri) => {
        let _ = webview.evaluate_script("location.reload();");
        ctx.submit_command(WEBVIEW_INSTALL.with(InstallType::Uri(uri.clone())))
      }
      UserEvent::CancelDownload => {}
      UserEvent::NewWindow(uri) => {
        let _ = webview.load_url(uri).inspect_err(|e| eprintln!("{e}"));
      }
      UserEvent::BlobReceived(uri) => {
        *mega_file = Some(Default::default());
        webview
          .evaluate_script(&format!(
            r#"
              (() => {{
                /**
                * @type Blob
                */
                let blob = URL.getObjectURLDict()['{}']
                  || Object.values(URL.getObjectURLDict())[0]

                var increment = 2 ** 20;
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
            "#,
            uri
          ))
          .expect("Eval script");
      }
      UserEvent::BlobChunk(chunk) => {
        if let Some(data) = mega_file.as_mut() {
          match chunk {
            Some(chunk) => {
              let split = chunk.split(',').nth(1);
              if let Some(split) = split {
                if let Ok(decoded) = decode(split) {
                  data.borrow_mut().extend(decoded)
                }
              }
            }
            None => {
              let data = data.take();
              *mega_file = None;
              let ext_ctx = ctx.get_external_handle();
              tokio::task::spawn_blocking(move || {
                let path = PROJECT.cache_dir().join(format!("{}", random::<u16>()));
                let mut file =
                  std::fs::File::create(&path).expect("Create temp file for Mega download");
                file.write_all(&data).expect("Write data");
                ext_ctx
                  .submit_command_global(WEBVIEW_INSTALL, InstallType::Path(path))
                  .expect("Submit install command");
              });
            }
          }
        }
      }
      UserEvent::PageLoaded => {
        let ext_ctx = ctx.get_external_handle();
        webview
          .screenshot(move |res| {
            let func = || -> anyhow::Result<()> {
              let image = res?;
              let mut file = std::fs::File::create("baaaaar.png")?;
              file.write(image.as_slice())?;

              Ok(())
            };
            if let Err(e) = func() {
              ext_ctx
                .submit_command_global(Browser::WEBVIEW_HIDE, ())
                .inspanic("hide webview");
              ext_ctx
                .submit_command_global(
                  Popup::OPEN_POPUP,
                  Popup::custom(move || {
                    Flex::column()
                      .with_child(Label::new(format!("Failed to save screenshot {e:?}")))
                      .on_command(super::App::DUMB_UNIVERSAL_ESCAPE, |ctx, _, _| {
                        ctx.submit_command(Popup::DISMISS)
                      })
                      .in_card()
                      .boxed()
                  }),
                )
                .inspanic("send popup");
            }
          })
          .inspanic("take webview screenshot");
      }
    }

    false
  }
}
