use std::{cell::RefCell, io::Write, ops::Deref, rc::Rc};

use base64::{decode, encode};
use druid::{
  widget::{Flex, Maybe, Painter, SizedBox},
  Data, ExtEventSink, ImageBuf, Lens, Selector, SingleUse, Widget, WidgetExt,
};
use druid_widget_nursery::{material_icons::Icon, WidgetExt as _};
use gtk::glib::translate::{FromGlibPtrFull, ToGlibPtr};
use rand::random;
use webview_shared::{
  ExtEventSinkExt, InstallType, WebviewEvent, PROJECT, WEBVIEW_EVENT, WEBVIEW_INSTALL,
};
use webview_subsystem::init_webview;
use wry::{WebView, WebViewBuilder, WebViewBuilderExtUnix};

use crate::{
  app::{
    browser::button::{button, button_text},
    controllers::ExtensibleController,
    ARROW_LEFT, ARROW_RIGHT, BOOKMARK, BOOKMARK_BORDER,
  },
  nav_bar::{Nav, NavLabel},
  widgets::card::Card,
};

use super::util::WidgetExtEx;

mod button;

#[derive(Data, Clone, Lens, Default)]
pub struct Browser {
  pub inner: Option<BrowserInner>,
  url: Option<String>,
}

#[derive(Data, Clone, Lens)]
pub struct BrowserInner {
  pub webview: Rc<WebView>,
  visible: Rc<RefCell<bool>>,
  #[data(ignore)]
  image: Option<Rc<ImageBuf>>,
  #[data(ignore)]
  mega_file: Option<Rc<RefCell<Vec<u8>>>>,
  #[data(ignore)]
  ext_ctx: ExtEventSink,
  #[data(ignore)]
  screenshot_in_progress: Rc<RefCell<bool>>,
}

impl Browser {
  pub const WEBVIEW_NAVIGATE: Selector<String> = Selector::new("browser.webview.navigate");
  pub const WEBVIEW_HIDE: Selector = Selector::new("browser.webview.hide");
  pub const WEBVIEW_SHOW: Selector = Selector::new("browser.webview.show");
  const WEBVIEW_SCREENSHOT_DATA: Selector<SingleUse<Vec<u8>>> =
    Selector::new("browser.webview.screenshot_receive");

  pub fn init(&self) -> bool {
    self.inner.is_some()
  }

  pub fn view() -> impl Widget<Browser> {
    const INIT_WEBVIEW: Selector = Selector::new("browser.webview.init");

    Maybe::or_empty(|| {
      Flex::column()
        .with_child(
          Flex::row()
            .with_child(
              button(|_| Icon::new(*ARROW_LEFT).padding(-5.).boxed())
                .on_click(|_, browser: &mut BrowserInner, _| {
                  let _ = browser.webview.evaluate_script("window.history.back()");
                })
                .padding((0.0, 5.0)),
            )
            .with_child(
              button(|_| Icon::new(*ARROW_RIGHT).padding(-5.).boxed())
                .on_click(|_, browser: &mut BrowserInner, _| {
                  let _ = browser.webview.evaluate_script("window.history.forward()");
                })
                .padding((0.0, 5.0)),
            )
            .with_child(
              button(|hovered| {
                Flex::row()
                  .cross_axis_alignment(druid::widget::CrossAxisAlignment::Center)
                  .main_axis_alignment(druid::widget::MainAxisAlignment::Center)
                  .with_child(button_text("Bookmarks"))
                  .with_child(Icon::new(*if hovered { BOOKMARK } else { BOOKMARK_BORDER }))
                  .valign_centre()
              })
              .fix_width(175.)
              .padding((0.0, 5.0)),
            )
            .expand_width(),
        )
        .with_flex_child(
          Card::builder()
            .with_insets((0.0, 14.0))
            .with_corner_radius(4.0)
            .with_shadow_length(8.0)
            .build(SizedBox::empty().expand().foreground(Painter::new(
              move |ctx, browser: &BrowserInner, _| {
                if !ctx.size().is_empty() && browser.is_visible() {
                  browser.set_bounds(ctx.window_origin(), ctx.size());
                }
              },
            )))
            .foreground(Painter::new(move |ctx, browser: &BrowserInner, _| {
              if let Some(image) = &browser.image {
                let image = image.deref().clone();
                let region = ctx.size().to_rect().with_origin((7., 7.));
                let region = region.with_size((region.width() - 14., region.height() - 14.));
                ctx.with_save(|ctx| {
                  use druid::RenderContext;

                  let image = image.to_image(ctx.render_ctx);
                  ctx.clip(region);
                  ctx.draw_image(
                    &image,
                    region,
                    druid::piet::InterpolationMode::NearestNeighbor,
                  );
                });
              }
            }))
            .mask_default()
            .controller(
              ExtensibleController::new()
                .on_lifecycle(|_, ctx, event, browser: &BrowserInner, _| {
                  if matches!(
                    event,
                    druid::LifeCycle::ViewContextChanged(_) | druid::LifeCycle::Size(_)
                  ) && !ctx.size().is_empty()
                  {
                    browser.set_visible(true);
                    ctx.request_layout();
                    ctx.request_paint();
                    browser.screenshot(ctx.get_external_handle());
                  }
                })
                .on_command(Nav::NAV_SELECTOR, |_, _, payload, browser| {
                  browser.set_visible(*payload == NavLabel::WebBrowser);
                  true
                })
                .on_command(super::App::OPEN_WEBVIEW, |_, _, payload, browser| {
                  if let Some(url) = payload {
                    let _ = browser
                      .webview
                      .load_url(url)
                      .inspect_err(|e| eprintln!("{e}"));
                  }
                  browser.set_visible(true);
                  false
                })
                .on_command(Browser::WEBVIEW_HIDE, |_, _, _, data| {
                  data.set_visible(false);
                  true
                })
                .on_command(Browser::WEBVIEW_SHOW, |_, _, _, data| {
                  data.set_visible(true);
                  true
                })
                .on_command(Browser::WEBVIEW_SCREENSHOT_DATA, |_, ctx, payload, data| {
                  let image = payload.take().unwrap();
                  *data.screenshow_wip() = false;
                  if let Ok(image) = ImageBuf::from_data(&image) {
                    if image.size() + druid::Size::from((14., 14.)) == ctx.size() {
                      data.image = Some(Rc::new(image));
                    } else if data.is_visible() {
                      data.screenshot(ctx.get_external_handle())
                    }
                  }

                  ctx.request_update();
                  ctx.request_paint();
                  true
                })
                .on_command(WEBVIEW_EVENT, Browser::handle_webview_events),
            ),
          1.,
        )
    })
    .lens(Browser::inner)
    .on_command(Nav::NAV_SELECTOR, |ctx, payload, data| {
      if *payload == NavLabel::WebBrowser && data.inner.is_none() {
        ctx.submit_command(INIT_WEBVIEW)
      }
    })
    .on_command2(INIT_WEBVIEW, |_, ctx, _, data| {
      #[cfg(target_os = "linux")]
      let res = {
        use gtk::prelude::*;

        let window = ctx.window().get_gtk_application_window();
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

        init_webview(data.url.clone(), builder, ctx.get_external_handle())
      };
      #[cfg(not(target_os = "linux"))]
      let res = webview_subsystem::init_webview_with_handle(
        data.url.clone(),
        ctx.window(),
        ctx.get_external_handle(),
      );

      match res {
        Ok(webview) => {
          let inner = BrowserInner {
            webview: Rc::new(webview),
            visible: Default::default(),
            image: None,
            mega_file: None,
            ext_ctx: ctx.get_external_handle(),
            screenshot_in_progress: Default::default(),
          };
          inner.set_visible(true);
          data.inner = Some(inner);
          ctx.submit_command(Browser::WEBVIEW_SHOW)
        }
        Err(err) => eprintln!("webview build error: {err:?}"),
      }
      ctx.request_update();
      ctx.request_layout();
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
    user_event: &WebviewEvent,
    inner: &mut BrowserInner,
  ) -> bool {
    let BrowserInner {
      webview, mega_file, ..
    } = inner;
    match user_event {
      WebviewEvent::Navigation(uri) => {
        println!("Navigation: {}", uri);
        if uri.starts_with("https://www.mediafire.com/file") {
          let _ = webview.evaluate_script(r#"window.alert("You appear to be on a Mediafire site.\nIn order to correctly trigger a Mediafire download, attempt to open the dowload link in a new window.\nThis can be done through the right click context menu, or using a platform shortcut.")"#);
        }
      }
      WebviewEvent::AskDownload(uri) => {
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
      WebviewEvent::Download(uri) => {
        let _ = webview.evaluate_script("location.reload();");
        ctx.submit_command(WEBVIEW_INSTALL.with(InstallType::Uri(uri.clone())))
      }
      WebviewEvent::CancelDownload => {}
      WebviewEvent::NewWindow(uri) => {
        let _ = webview.load_url(uri).inspect_err(|e| eprintln!("{e}"));
      }
      WebviewEvent::BlobReceived(uri) => {
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
      WebviewEvent::BlobChunk(chunk) => {
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
      WebviewEvent::PageLoaded => {
        inner.screenshot(ctx.get_external_handle());
      }
    }

    false
  }
}

impl BrowserInner {
  fn set_visible(&self, visible: bool) {
    *self.visible.borrow_mut() = visible;
    let _ = self.webview.set_visible(visible);
  }

  fn is_visible(&self) -> bool {
    *self.visible.borrow()
  }

  fn set_bounds(&self, origin: druid::Point, size: druid::widget::prelude::Size) {
    let _ = self
      .webview
      .set_bounds(wry::Rect {
        position: wry::dpi::Position::Logical((origin.x, origin.y - 7.).into()),
        size: wry::dpi::Size::Logical((size.width, size.height + 14.).into()),
      })
      .inspect_err(|e| eprintln!("{e}"));
  }

  fn screenshot(&self, ext_ctx: ExtEventSink) {
    if !*self.screenshow_wip() {
      if self
        .webview
        .screenshot(move |res| {
          let func = || -> anyhow::Result<()> {
            let image = res?;
            let _ = ext_ctx
              .submit_command_global(Browser::WEBVIEW_SCREENSHOT_DATA, SingleUse::new(image));

            Ok(())
          };
          if let Err(e) = func() {
            eprintln!("{e:?}")
          }
        })
        .inspect_err(|e| eprintln!("{e:?}"))
        .is_ok()
      {
        *self.screenshow_wip() = true;
      }
    }
  }

  fn screenshow_wip(&self) -> std::cell::RefMut<bool> {
    self.screenshot_in_progress.borrow_mut()
  }
}
