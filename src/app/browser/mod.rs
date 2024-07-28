use std::{cell::RefCell, io::Write, ops::Deref, rc::Rc};

use base64::decode;
use druid::{
  widget::{Flex, Maybe, Painter, SizedBox},
  Data, ExtEventSink, ImageBuf, Lens, Selector, SingleUse, Widget, WidgetExt,
};
use druid_widget_nursery::{material_icons::Icon, AnyCtx, LaidOutCtx, WidgetExt as _};
use rand::random;
use webview_shared::{
  ExtEventSinkExt, InstallType, WebviewEvent, PROJECT, WEBVIEW_EVENT, WEBVIEW_INSTALL,
};
use wry::WebView;

use super::{
  overlays::Popup,
  util::{DataTimer, WidgetExtEx},
};
use crate::{
  app::{
    browser::button::{button, button_text, button_unconstrained},
    controllers::ExtensibleController,
    util::ShadeColor,
    ARROW_LEFT, ARROW_RIGHT, BOOKMARK, BOOKMARK_BORDER, REFRESH,
  },
  match_command,
  nav_bar::{Nav, NavLabel},
  widgets::{card::Card, root_stack::RootStack},
};

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
  screenshot_in_progress: Rc<RefCell<bool>>,
  tab_open: bool,
  force_hidden: bool,
  load_in_progress: bool,
}

impl Browser {
  pub const WEBVIEW_NAVIGATE: Selector<String> = Selector::new("browser.webview.navigate");
  pub const WEBVIEW_HIDE: Selector = Selector::new("browser.webview.hide");
  pub const WEBVIEW_SHOW: Selector = Selector::new("browser.webview.show");
  const WEBVIEW_SCREENSHOT_DATA: Selector<SingleUse<Vec<u8>>> =
    Selector::new("browser.webview.screenshot_receive");

  pub fn is_init(&self) -> bool {
    self.inner.is_some()
  }

  pub fn view() -> impl Widget<Browser> {
    const INIT_WEBVIEW: Selector = Selector::new("browser.webview.init");

    Maybe::or_empty(|| {
      Flex::column().with_child(toolbar()).with_flex_child(
        Card::builder()
          .with_insets((0.0, 14.0))
          .with_corner_radius(4.0)
          .with_shadow_length(8.0)
          .build(SizedBox::empty().expand().foreground(Painter::new(
            move |ctx, browser: &BrowserInner, _| {
              if !ctx.size().is_empty() && browser.is_visible() {
                browser.set_bounds(ctx);
              }
            },
          )))
          .foreground(Painter::new({
            let mut saved_image = None;
            move |ctx, browser: &BrowserInner, _| {
              if let Some(incoming_image) = &browser.image {
                let image = incoming_image.deref().to_image(ctx.render_ctx);
                saved_image = Some((image, incoming_image.size()))
              }

              if let Some((image, size)) = saved_image.as_ref() {
                let region = ctx.size().to_rect().with_origin((7., 7.));
                let region = region.with_size((region.width() - 14., region.height() - 14.));
                ctx.with_save(|ctx| {
                  use druid::RenderContext;

                  let target_rect = druid::Rect::from_center_size(region.center(), *size);

                  ctx.clip(region);
                  ctx.draw_image(
                    image,
                    target_rect,
                    druid::piet::InterpolationMode::NearestNeighbor,
                  );
                });
              }
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
                  if !ctx.is_disabled() {
                    browser.set_visible(true);
                  }
                  ctx.request_layout();
                  ctx.request_paint();
                  browser.screenshot(ctx.get_external_handle());
                }
              })
              .on_command(Nav::NAV_SELECTOR, |_, _, payload, browser| {
                browser.tab_open = *payload == NavLabel::WebBrowser;
                browser.force_hidden = !browser.tab_open;
                browser.set_visible(browser.tab_open);
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
              .on_command(Browser::WEBVIEW_NAVIGATE, |_, _, url, data| {
                let _ = data.webview.load_url(url);
                true
              })
              .on_event(|_, _, event, data| {
                if let druid::Event::Command(cmd) = event {
                  match_command!(cmd, () => {
                    Popup::OPEN_POPUP,
                    Popup::QUEUE_POPUP,
                    Popup::OPEN_NEXT,
                    Popup::DELAYED_POPUP, => {
                      data.force_hidden = true;
                      data.set_visible(false)
                    }
                  })
                }
                true
              })
              .on_command(Popup::IS_EMPTY, |_, ctx, _, data| {
                if data.tab_open {
                  data.force_hidden = false;
                  data.set_visible(true);
                  ctx.request_update();
                  ctx.request_paint();
                  data.screenshot(ctx.get_external_handle())
                }
                true
              })
              .on_command(
                Browser::WEBVIEW_SCREENSHOT_DATA,
                |_, ctx, payload, browser| {
                  let image = payload.take().unwrap();
                  *browser.screenshow_wip() = false;
                  if let Ok(image) = ImageBuf::from_data(&image) {
                    if (image.size() + (14., 14.).into()).aspect_ratio()
                      == ctx.size().aspect_ratio()
                    {
                      browser.image = Some(Rc::new(image));
                    } else if browser.is_visible() {
                      browser.screenshot(ctx.get_external_handle())
                    }
                  }
                  ctx.request_update();
                  ctx.request_paint();
                  true
                },
              )
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
      let res = init_webview(ctx, data);

      match res {
        Ok(webview) => {
          let inner = BrowserInner {
            webview,
            visible: Default::default(),
            image: None,
            mega_file: None,
            screenshot_in_progress: Default::default(),
            tab_open: true,
            force_hidden: false,
            load_in_progress: true,
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
      WebviewEvent::Navigation(_uri) => {
        // println!("Navigation: {}", uri);
        // if uri.starts_with("https://www.mediafire.com/file") {
        //   let _ = webview.evaluate_script(r#"window.alert("You appear to be
        // on a Mediafire site.\nIn order to correctly trigger a Mediafire
        // download, attempt to open the dowload link in a new window.\nThis can
        // be done through the right click context menu, or using a platform
        // shortcut.")"#); }
      }
      WebviewEvent::AskDownload(uri) => {
        inner.screenshot(ctx.get_external_handle());
        inner.force_hidden = true;
        inner.load_in_progress = false;
        ctx.submit_command(Browser::WEBVIEW_HIDE);
        ctx.submit_command(Popup::OPEN_POPUP.with(Popup::browser_install(uri.clone())))
      }
      WebviewEvent::Download(uri) => {
        let _ = inner.reload();
        inner.load_in_progress = false;
        ctx.submit_command(WEBVIEW_INSTALL.with(InstallType::Uri(uri.clone())))
      }
      WebviewEvent::CancelDownload => {
        inner.load_in_progress = false;
      }
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
        inner.load_in_progress = false;
      }
      WebviewEvent::PageUnloading => {
        inner.load_in_progress = true;
      }
      WebviewEvent::ShowConfirmPopup(_url) => {}
    }

    false
  }
}

fn init_webview(ctx: &mut druid::EventCtx, data: &mut Browser) -> Result<Rc<WebView>, wry::Error> {
  #[cfg(target_os = "linux")]
  let res = {
    use gtk::prelude::*;
    use wry::{WebViewBuilder, WebViewBuilderExtUnix};

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

    webview_subsystem::init_webview(data.url.clone(), builder, ctx.get_external_handle())
  };
  #[cfg(not(target_os = "linux"))]
  let res = webview_subsystem::init_webview_with_handle(
    data.url.clone(),
    ctx.window(),
    ctx.get_external_handle(),
  );
  res
}

const BOOKMARK_WIDTH: f64 = 190.0;

fn toolbar() -> SizedBox<BrowserInner> {
  Flex::row()
    .with_child(
      button(|_| Icon::new(*ARROW_LEFT).padding(-5.).boxed())
        .on_click(|_, browser: &mut BrowserInner, _| {
          let _ = browser.webview.evaluate_script("history.back()");
        })
        .padding((0.0, 5.0)),
    )
    .with_child(
      button(|_| Icon::new(*ARROW_RIGHT).padding(-5.).boxed())
        .on_click(|_, browser: &mut BrowserInner, _| {
          let _ = browser.webview.evaluate_script("history.forward()");
        })
        .padding((0.0, 5.0)),
    )
    .with_child(
      button(|_| Icon::new(*REFRESH).padding(-2.5).boxed())
        .on_click(|_, browser: &mut BrowserInner, _| {
          browser.reload();
        })
        .padding((0.0, 5.0)),
    )
    .with_child(bookmarks())
    .disabled_if(|data, _| data.load_in_progress)
    .env_scope(|env, data| {
      if data.load_in_progress {
        const TEXT: druid::Key<druid::Color> = druid::theme::TEXT_COLOR;
        env.set(TEXT, env.get(TEXT).lighter_by(8))
      }
    })
    .expand_width()
}

fn bookmarks() -> druid::widget::Padding<BrowserInner, impl Widget<BrowserInner>> {
  button(|hovered| bookmarks_heading_button::<BrowserInner>(hovered))
    .fix_width(BOOKMARK_WIDTH)
    .scope_with(|_| (true, DataTimer::INVALID), |widget| {
      const RE_ENABLE: Selector = Selector::new("browser.bookmarks.toggle");

      widget
        .invisible_if(|data, _| !data.inner.0)
        .on_command(RE_ENABLE, |ctx, _, data| {
          data.inner.0 = !data.inner.0;
          data.outer.force_hidden = false;
          data.inner.1 = ctx
            .request_timer(std::time::Duration::from_millis(50))
            .into();
        })
        .on_event(|_, ctx, event, data| {
          if let druid::Event::Timer(token) = event
            && *token == *data.inner.1
          {
            data.inner.1 = DataTimer::INVALID;
            data.outer.set_visible(true);
            ctx.request_paint();
            data.outer.screenshot(ctx.get_external_handle());
            true
          } else {
            false
          }
        })
        .on_click(|ctx, data, env| {
          let background = env.get(druid::theme::BACKGROUND_LIGHT);
          data.inner.0 = false;
          data.outer.screenshot(ctx.get_external_handle());
          data.outer.set_visible(false);
          data.outer.force_hidden = true;
          RootStack::show(
            ctx,
            ctx.window_origin(),
            move || {
              button_unconstrained(move |hovered| {
                Flex::column()
                  .with_child(bookmarks_heading_button(hovered))
                  .with_default_spacer()
                  .with_child(bookmark_button(
                    "Forum Mod Index",
                    "https://fractalsoftworks.com/forum/index.php?topic=177.0",
                    background,
                  ))
                  .with_default_spacer()
                  .with_child(bookmark_button(
                    "Mods Sub-forum",
                    "https://fractalsoftworks.com/forum/index.php?board=8.0",
                    background,
                  ))
                  .with_default_spacer()
                  .with_child(bookmark_button(
                    "Modding Sub-forum",
                    "https://fractalsoftworks.com/forum/index.php?board=3.0",
                    background,
                  ))
              })
              .fix_width(BOOKMARK_WIDTH)
              .on_click(|ctx, _, _| ctx.submit_command(RootStack::DISMISS))
              .boxed()
            },
            Some(|ctx: &mut druid::EventCtx| ctx.submit_command(RE_ENABLE)),
          )
        })
    })
    .padding((0.0, 5.0))
}

fn bookmark_button(text: &str, url: &str, background: druid::Color) -> impl Widget<super::App> {
  let text = text.to_owned();
  let url = url.to_owned();
  Card::builder()
    .with_insets((0.0, 14.0))
    .with_shadow_length(0.0)
    .with_shadow_increase(0.0)
    .hoverable(move |hovered| {
      button_text(&text)
        .valign_centre()
        .align_left()
        .padding((4., 0., 0., 0.))
        .background(if hovered {
          background.interpolate_with(druid::Color::GRAY, 6)
        } else {
          background
        })
        .rounded(4.0)
        .padding((-7., -14.))
    })
    .fix_width(BOOKMARK_WIDTH - 2.5)
    .on_click(move |ctx, _, _| ctx.submit_command(Browser::WEBVIEW_NAVIGATE.with(url.clone())))
}

fn bookmarks_heading_button<T: Data>(hovered: bool) -> druid::widget::Align<T> {
  Flex::row()
    .cross_axis_alignment(druid::widget::CrossAxisAlignment::Center)
    .main_axis_alignment(druid::widget::MainAxisAlignment::Center)
    .with_child(button_text("Bookmarks"))
    .with_child(Icon::new(*if hovered { BOOKMARK } else { BOOKMARK_BORDER }))
    .valign_centre()
}

impl BrowserInner {
  fn set_visible(&self, visible: bool) {
    if self.force_hidden && visible {
      return;
    }

    *self.visible.borrow_mut() = visible;
    let _ = self.webview.set_visible(visible);
  }

  fn is_visible(&self) -> bool {
    *self.visible.borrow()
  }

  fn set_bounds(&self, ctx: &mut (impl LaidOutCtx + AnyCtx)) {
    let window_origin = ctx.window_origin();
    let size = ctx.size();
    #[cfg(not(target_os = "macos"))]
    let position = (window_origin.x, window_origin.y - 7.);
    #[cfg(target_os = "macos")]
    let position = {
      let actual_y = ctx.window().get_size().height - (ctx.window_origin().y + ctx.size().height);
      (window_origin.x, actual_y - 35.0)
    };
    let _ = self
      .webview
      .set_bounds(wry::Rect {
        position: wry::dpi::Position::Logical(position.into()),
        size: wry::dpi::Size::Logical((size.width, size.height + 14.).into()),
      })
      .inspect_err(|e| eprintln!("{e}"));
  }

  fn screenshot(&self, ext_ctx: ExtEventSink) {
    #[cfg(not(target_os = "macos"))]
    if self.is_visible()
      && !*self.screenshow_wip()
      && self
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
    #[cfg(target_os = "macos")]
    drop(ext_ctx)
  }

  fn screenshow_wip(&self) -> std::cell::RefMut<bool> {
    self.screenshot_in_progress.borrow_mut()
  }

  fn reload(&mut self) {
    self.load_in_progress = true;
    let _ = self.webview.load_url(&self.webview.url().unwrap());
  }
}
