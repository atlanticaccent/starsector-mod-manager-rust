use std::{collections::LinkedList, future::Future, sync::OnceLock, time::Duration};

use common::{EventExt, ExtEventSinkExt};
use druid::{
  widget::Controller, Env, Event, EventCtx, ExtEventSink, Selector, SingleUse, TimerToken, Widget,
};
use druid_widget_nursery::CommandCtx;
use strum_macros::EnumDiscriminants;
use tokio::{
  runtime::Handle,
  sync::oneshot,
};

use crate::{app::App, bang};

pub static GLOBAL_ASYNC_CONTROLLER: AsyncCoordinator = AsyncCoordinator::new();

#[derive(derive_more::Deref)]
pub struct AsyncCoordinator(OnceLock<AsyncCoordinatorImpl>);

impl AsyncCoordinator {
  pub const fn new() -> Self {
    Self(OnceLock::new())
  }

  pub fn get_unchecked(&self) -> &AsyncCoordinatorImpl {
    self.get().unwrap()
  }
}

#[derive(EnumDiscriminants)]
#[strum_discriminants(name(Status))]
enum TypedStatus<T> {
  Pending(oneshot::Receiver<T>),
  Blocked(T),
  Complete(T),
  Failed,
}

trait FutureHandle {
  fn progress(&mut self, ctx: &mut EventCtx, data: &App, env: &Env) -> Status;

  fn apply_result(self: Box<Self>, ctx: &mut EventCtx, app: &mut App, env: &Env);
}

pub struct AsyncCoordinatorImpl {
  runtime: Handle,
  ext_ctx: ExtEventSink,
}

#[allow(dead_code)]
impl AsyncCoordinatorImpl {
  const NEW_TASK: Selector<SingleUse<Box<dyn FutureHandle + Send + Sync>>> =
    Selector::new("async_controller.task.new");

  pub fn new(runtime: Handle, ext_ctx: ExtEventSink) -> Self {
    AsyncCoordinatorImpl { runtime, ext_ctx }
  }

  pub fn add_task<T: Send + Sync + 'static>(
    &self,
    task: impl Future<Output = T> + Send + 'static,
    handler: impl FnOnce(T, &mut EventCtx, &mut App, &Env) + Send + Sync + 'static,
  ) {
    self.add_task_opts(
      task,
      Option::<fn(&T, &mut EventCtx, &App, &Env) -> bool>::None,
      handler,
    );
  }

  pub fn add_task_with_check<T: Send + Sync + 'static>(
    &self,
    task: impl Future<Output = T> + Send + 'static,
    progress_handler: impl Fn(&T, &mut EventCtx, &App, &Env) -> bool + Send + Sync + 'static,
    result_handler: impl FnOnce(T, &mut EventCtx, &mut App, &Env) + Send + Sync + 'static,
  ) {
    self.add_task_opts(task, Some(progress_handler), result_handler);
  }

  pub fn add_task_opts<T: Send + Sync + 'static>(
    &self,
    task: impl Future<Output = T> + Send + 'static,
    progress_handler: Option<
      impl Fn(&T, &mut EventCtx, &App, &Env) -> bool + Send + Sync + 'static,
    >,
    result_handler: impl FnOnce(T, &mut EventCtx, &mut App, &Env) + Send + Sync + 'static,
  ) {
    let (tx, rx) = oneshot::channel();

    self.runtime.spawn(async move {
      let res = task.await;
      let _ = tx.send(res);
    });

    let handle: Box<dyn FutureHandle + Send + Sync> = Box::new(TypedFutureHandle {
      status: TypedStatus::Pending(rx),
      progress_handler,
      result_handler,
    });

    let _ = self
      .ext_ctx
      .submit_command_global(AsyncCoordinatorImpl::NEW_TASK, SingleUse::new(handle))
      .inspect_err(|err| bang!(err));
  }
}

pub struct AsyncController {
  handles: LinkedList<Box<dyn FutureHandle + Send + Sync>>,
  deadline: Option<TimerToken>,
  runtime: Handle,
}

impl AsyncController {
  pub fn new(runtime: Handle) -> Self {
    Self {
      handles: LinkedList::new(),
      deadline: None,
      runtime,
    }
  }

  fn update_deadline(&mut self, ctx: &mut impl CommandCtx) {
    self.deadline = Some(ctx.request_timer(std::time::Duration::from_millis(50)))
  }
}

impl<W: Widget<App>> Controller<App, W> for AsyncController {
  fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, app: &mut App, env: &Env) {
    if let Event::Timer(token) = event
      && let Some(requested) = self.deadline.as_ref()
      && token == requested
    {
      let mut handles = self.handles.cursor_front_mut();
      let work = async {
        while let Some(handle) = handles.current() {
          match handle.progress(ctx, &app, env) {
            Status::Complete => {
              let completed = handles
                .remove_current()
                .expect("Must be `Some` if entered loop");
              completed.apply_result(ctx, app, env);
            }
            Status::Failed => {
              handles.remove_current();
              bang!("Task channel closed")
            }
            Status::Pending | Status::Blocked => {
              handles.move_next();
            }
          }

          tokio::task::yield_now().await
        }
      };

      if let Err(_) = self
        .runtime
        .block_on(tokio::time::timeout(Duration::from_millis(25), work))
      {
        bang!("Didn't process all tasks in time")
      }

      if !self.handles.is_empty() {
        self.update_deadline(ctx);
      } else {
        self.deadline = None;
      }
    }
    if let Some(task) = event.get_cmd(AsyncCoordinatorImpl::NEW_TASK) {
      let task = task.take().unwrap();
      self.handles.push_back(task);
      if self.deadline.is_none() {
        self.update_deadline(ctx);
      }
    }

    child.event(ctx, event, app, env);
  }

  fn update(
    &mut self,
    child: &mut W,
    ctx: &mut druid::UpdateCtx,
    old_data: &App,
    data: &App,
    env: &Env,
  ) {
    if self.deadline.is_none() {
      self.update_deadline(ctx);
    }

    child.update(ctx, old_data, data, env);
  }
}

struct TypedFutureHandle<T, FP, FR> {
  status: TypedStatus<T>,
  progress_handler: FP,
  result_handler: FR,
}

#[allow(dead_code)]
impl<T, FR> TypedFutureHandle<T, Option<fn(&T, &mut EventCtx, &App, &Env) -> bool>, FR> {
  pub fn new(rx: oneshot::Receiver<T>, result_handler: FR) -> Self {
    Self {
      status: TypedStatus::Pending(rx),
      progress_handler: None,
      result_handler,
    }
  }
}

impl<
    T,
    FP: Fn(&T, &mut EventCtx, &App, &Env) -> bool,
    FR: FnOnce(T, &mut EventCtx, &mut App, &Env),
  > FutureHandle for TypedFutureHandle<T, Option<FP>, FR>
{
  fn progress(&mut self, ctx: &mut EventCtx, data: &App, env: &Env) -> Status {
    self.status = match std::mem::replace(&mut self.status, TypedStatus::Failed) {
      TypedStatus::Pending(mut rx) => match rx.try_recv() {
        Ok(res) => {
          if let Some(false) = self
            .progress_handler
            .as_ref()
            .map(|f| (f)(&res, ctx, data, env))
          {
            TypedStatus::Blocked(res)
          } else {
            TypedStatus::Complete(res)
          }
        }
        Err(oneshot::error::TryRecvError::Closed) => TypedStatus::Failed,
        Err(oneshot::error::TryRecvError::Empty) => TypedStatus::Pending(rx),
      },
      TypedStatus::Blocked(res) => {
        if let None | Some(true) = self
          .progress_handler
          .as_ref()
          .map(|f| (f)(&res, ctx, data, env))
        {
          TypedStatus::Complete(res)
        } else {
          TypedStatus::Blocked(res)
        }
      }
      rem => rem,
    };

    (&self.status).into()
  }

  fn apply_result(self: Box<Self>, ctx: &mut EventCtx, app: &mut App, env: &Env) {
    if let TypedStatus::Complete(res) = self.status {
      (self.result_handler)(res, ctx, app, env)
    }
  }
}
