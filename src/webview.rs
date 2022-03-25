use wry::{
  application::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
  },
  webview::{WebViewBuilder, WebContext},
};

enum UserEvent {
  NewWindow(String),
}

pub fn init_webview() -> wry::Result<()> {
  let html = r#"
    <body>
      <div>
        <p> WRYYYYYYYYYYYYYYYYYYYYYY! </p>
        <a href="https://www.wikipedia.org" target="_blank">Visit Wikipedia</a>
        <a href="https://www.github.com" target="_blank">(Try to) visit GitHub</a>
      </div>
    </body>
  "#;

  let event_loop: EventLoop<UserEvent> = EventLoop::with_user_event();
  let proxy = event_loop.create_proxy();
  let mut webcontext = WebContext::default();
  webcontext.set_allows_automation(true);
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .build(&event_loop)?;
  let webview = WebViewBuilder::new(window)?
    .with_html(html)?
    .with_new_window_req_handler(move |uri: String| {
      let submitted = proxy.send_event(UserEvent::NewWindow(uri.clone())).is_ok();

      submitted && uri.contains("wikipedia")
    })
    .build()?;

  #[cfg(debug_assertions)]
  webview.devtool();

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::NewEvents(StartCause::Init) => println!("Wry has started!"),
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      Event::UserEvent(UserEvent::NewWindow(uri)) => {
        println!("New Window: {}", uri);
      }
      _ => (),
    }
  });
}