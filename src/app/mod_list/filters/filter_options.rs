use druid::{
  lens, theme,
  widget::{Flex, SizedBox, ViewSwitcher},
  Data, Env, Lens, LensExt, Widget, WidgetExt as _,
};
use druid_widget_nursery::{material_icons::Icon, WidgetExt as _};

use crate::{
  app::{
    controllers::{HeightLinker, HeightLinkerShared},
    icon::Icon as CopyIcon,
    mod_list::{install::install_options::InstallOptions, Filters, ModList},
    util::{
      bold_text, WidgetExtEx as _, WithHoverState, ADD_BOX as FILLED_CHECKBOX,
      CHECK_BOX_OUTLINE_BLANK as EMPTY_CHECKBOX,
    },
    DESELECT, INDETERMINATE_CHECK_BOX,
  },
  widgets::card::Card,
};

use super::{filter_button::FilterButton, FilterState};

pub struct FilterOptions;

impl FilterOptions {
  pub fn view() -> impl Widget<FilterState> {
    Card::builder()
      .with_insets((0.0, 14.0))
      .with_corner_radius(4.0)
      .with_shadow_length(8.0)
      .build(
        FilterButton::inner()
          .fix_height(60.0)
          .padding((-8.0, 0.0, -8.0, -4.0)),
      )
      .or_empty(|data: &bool, _| *data)
      .fix_width(super::FILTER_WIDTH)
      .lens(FilterState::open)
  }

  pub fn wide_view() -> impl Widget<FilterState> {
    let mut width_linker = {
      let mut linker = HeightLinker::new();
      linker.axis = druid::widget::Axis::Horizontal;
      Some(linker.shared())
    };
    let width_linker = &mut width_linker;
    Card::builder()
      .with_insets((0.0, 14.0))
      .with_corner_radius(4.0)
      .with_shadow_length(8.0)
      .with_background(theme::BACKGROUND_DARK)
      .build(FilterButton::button_styling(
        Flex::row()
          .with_child(Self::status_options(width_linker))
          .with_child(Self::update_options(width_linker))
          .with_child(Self::update_from_server_options(width_linker))
          .main_axis_alignment(druid::widget::MainAxisAlignment::SpaceEvenly)
          .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
          .expand_width(),
      ))
      .or_empty(|data, _| *data)
      .on_command(InstallOptions::DISMISS, |ctx, payload, data| {
        let hitbox = ctx
          .size()
          .to_rect()
          .with_origin(ctx.to_window((0.0, 0.0).into()));
        *data = hitbox.contains(*payload);
      })
      .lens(FilterState::open)
  }

  fn option_text<T: Data>(text: &str) -> impl Widget<T> {
    bold_text(
      text,
      druid::theme::TEXT_SIZE_NORMAL,
      druid::FontWeight::SEMI_BOLD,
      druid::theme::TEXT_COLOR,
    )
    .padding((8.0, 0.0))
  }

  fn option<T: Data, U>(
    text: &str,
    width_linker: &mut Option<HeightLinkerShared>,
    switch: impl Fn(&T, &Env) -> U + 'static,
  ) -> impl Widget<T>
  where
    BoolIcon<T>: FromFn<T, U>,
  {
    Flex::row()
      .with_child(
        match BoolIcon::from_fn(switch) {
          BoolIcon::Bool(bool) => ViewSwitcher::new(bool, |filled, _, _| {
            Icon::new(*if *filled {
              FILLED_CHECKBOX
            } else {
              EMPTY_CHECKBOX
            })
            .boxed()
          })
          .boxed(),
          BoolIcon::Icon(icon) => ViewSwitcher::new(icon, |icon, _, _| {
            let mut icon_widget = Icon::new(**icon);
            if let Some(color) = icon.color() {
              icon_widget = icon_widget.with_color(*color)
            }
            icon_widget.boxed()
          })
          .boxed(),
        }
        .padding((5.0, 0.0, -5.0, 0.0)),
      )
      .with_child(Self::option_text(text))
      .main_axis_alignment(druid::widget::MainAxisAlignment::Start)
      .lens(lens!((T, bool), 0))
      .with_hover_state(false)
      .link_height_with(width_linker)
  }

  fn status_options<T: Data>(mut width_linker: &mut Option<HeightLinkerShared>) -> impl Widget<T> {
    Card::builder()
      .with_insets((0.0, 10.0))
      .build(
        Flex::column()
          .with_child(
            Self::option("Status", &mut width_linker, |_: &Option<bool>, _: &Env| {
              DESELECT.with_color(druid::Color::GRAY)
            })
            .disabled_if(|_, _| true),
          )
          .with_child(
            SizedBox::empty()
              .link_height_with(&mut width_linker)
              .border(druid::Color::BLACK, 0.5)
              .padding((0.0, 2.0)),
          )
          .with_child(
            Self::option(
              "Enabled",
              &mut width_linker,
              |data: &Option<bool>, _: &Env| data.is_some_and(|data| data),
            )
            .on_click(|_, data, _| {
              if *data == Some(true) {
                *data = None
              } else {
                *data = Some(true)
              }
            }),
          )
          .with_child(
            Self::option(
              "Disabled",
              &mut width_linker,
              |data: &Option<bool>, _: &Env| data.is_some_and(|data| !data),
            )
            .on_click(|_, data, _| {
              if *data == Some(false) {
                *data = None
              } else {
                *data = Some(false)
              }
            }),
          )
          .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start),
      )
      .on_change(|ctx, _, data, _| {
        ctx.submit_command(
          ModList::FILTER_UPDATE.with((Filters::Enabled, data.is_some_and(|d| !d))),
        );
        ctx.submit_command(
          ModList::FILTER_UPDATE.with((Filters::Disabled, data.is_some_and(|d| d))),
        );
      })
      .on_command(ModList::FILTER_RESET, |_, _, data| *data = None)
      .scope_independent(|| Option::<bool>::None)
  }

  #[allow(non_local_definitions)]
  fn update_options<T: Data>(mut width_linker: &mut Option<HeightLinkerShared>) -> impl Widget<T> {
    #[derive(Default, Clone, Data, Lens)]
    struct VersionCheckerFilter {
      none: bool,
      major: bool,
      minor: bool,
      patch: bool,
      unimplemented: bool,
      error: bool,
      local_exceeds: bool,
    }

    impl VersionCheckerFilter {
      fn apply(&self, cmp: impl for<'a> Fn(&'a bool, &'a bool) -> bool) -> bool {
        cmp(
          &cmp(
            &cmp(
              &cmp(
                &cmp(&cmp(&self.none, &self.major), &self.minor),
                &self.patch,
              ),
              &self.unimplemented,
            ),
            &self.error,
          ),
          &self.local_exceeds,
        )
      }

      fn all(&self) -> bool {
        self.apply(|a, b| *a && *b)
      }

      fn any(&self) -> bool {
        self.apply(|a, b| *a || *b)
      }
    }

    fn version_filter_option(
      text: &str,
      width_linker: &mut Option<HeightLinkerShared>,
      lens: impl Lens<VersionCheckerFilter, bool> + Copy + 'static,
      filter: Filters,
    ) -> impl Widget<VersionCheckerFilter> {
      FilterOptions::option(
        text,
        width_linker,
        move |data: &VersionCheckerFilter, _: &Env| lens.get(data),
      )
      .on_click(move |_, data, _| lens.put(data, !lens.get(data)))
      .on_change(move |ctx, _, data, _| {
        ctx.submit_command(ModList::FILTER_UPDATE.with((filter, lens.get(data))))
      })
    }

    Card::builder()
      .with_insets((0.0, 10.0))
      .build(
        Flex::column()
          .with_child(
            Self::option(
              "Version Checker",
              &mut width_linker,
              |d: &VersionCheckerFilter, _: &Env| match (d.all(), d.any()) {
                (true, _) => FILLED_CHECKBOX,
                (false, true) => INDETERMINATE_CHECK_BOX,
                (false, false) => EMPTY_CHECKBOX,
              },
            )
            .on_click(|ctx, data, _| {
              let filters = vec![
                Filters::UpToDate,
                Filters::Major,
                Filters::Minor,
                Filters::Patch,
                Filters::Unimplemented,
                Filters::Error,
                Filters::Discrepancy,
              ];
              let mut enable = false;
              *data = match (data.all(), data.any()) {
                (true, _) | (false, true) => VersionCheckerFilter::default(),
                (false, false) => {
                  enable = true;
                  VersionCheckerFilter {
                    none: true,
                    major: true,
                    minor: true,
                    patch: true,
                    unimplemented: true,
                    error: true,
                    local_exceeds: true,
                  }
                }
              };
              for filter in filters.into_iter() {
                ctx.submit_command(ModList::FILTER_UPDATE.with((filter, enable)))
              }
            }),
          )
          .with_child(
            SizedBox::empty()
              .link_height_with(&mut width_linker)
              .border(druid::Color::BLACK, 0.5)
              .padding((0.0, 2.0)),
          )
          .with_child(version_filter_option(
            "No Update Available",
            &mut width_linker,
            VersionCheckerFilter::none,
            Filters::UpToDate,
          ))
          .with_child(version_filter_option(
            "Major Update Available",
            &mut width_linker,
            VersionCheckerFilter::major,
            Filters::Major,
          ))
          .with_child(version_filter_option(
            "Minor Update Available",
            &mut width_linker,
            VersionCheckerFilter::minor,
            Filters::Minor,
          ))
          .with_child(version_filter_option(
            "Patch Update Available",
            &mut width_linker,
            VersionCheckerFilter::patch,
            Filters::Patch,
          ))
          .with_child(version_filter_option(
            "Unimplemented",
            &mut width_linker,
            VersionCheckerFilter::unimplemented,
            Filters::Unimplemented,
          ))
          .with_child(version_filter_option(
            "Error",
            &mut width_linker,
            VersionCheckerFilter::error,
            Filters::Error,
          ))
          .with_child(version_filter_option(
            "Local Exceeds Remote",
            &mut width_linker,
            VersionCheckerFilter::local_exceeds,
            Filters::Discrepancy,
          ))
          .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start),
      )
      .on_command(ModList::FILTER_RESET, |_, _, data| {
        *data = VersionCheckerFilter::default()
      })
      .scope_independent(|| VersionCheckerFilter::default())
  }

  fn update_from_server_options<T: Data>(
    mut width_linker: &mut Option<HeightLinkerShared>,
  ) -> impl Widget<T> {
    Card::builder()
      .with_insets((0.0, 10.0))
      .build(
        Flex::column()
          .with_child(
            Self::option(
              "Update From Server",
              &mut width_linker,
              |_: &Option<bool>, _: &Env| DESELECT.with_color(druid::Color::GRAY),
            )
            .disabled_if(|_, _| true),
          )
          .with_child(
            SizedBox::empty()
              .link_height_with(&mut width_linker)
              .border(druid::Color::BLACK, 0.5)
              .padding((0.0, 2.0)),
          )
          .with_child(
            Self::option(
              "Supported",
              &mut width_linker,
              |data: &Option<bool>, _: &Env| data.is_some_and(|data| data),
            )
            .on_click(|_, data, _| {
              if *data == Some(true) {
                *data = None
              } else {
                *data = Some(true)
              }
            }),
          )
          .with_child(
            Self::option(
              "Unsupported",
              &mut width_linker,
              |data: &Option<bool>, _: &Env| data.is_some_and(|data| !data),
            )
            .on_click(|_, data, _| {
              if *data == Some(false) {
                *data = None
              } else {
                *data = Some(false)
              }
            }),
          )
          .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start),
      )
      .on_change(|ctx, _, data, _| {
        ctx.submit_command(
          ModList::FILTER_UPDATE.with((Filters::AutoUpdateAvailable, data.is_some_and(|d| d))),
        );
        ctx.submit_command(
          ModList::FILTER_UPDATE.with((Filters::AutoUpdateUnsupported, data.is_some_and(|d| !d))),
        );
      })
      .on_command(ModList::FILTER_RESET, |_, _, data| *data = None)
      .scope_independent(|| Option::<bool>::None)
  }
}

enum BoolIcon<T> {
  Bool(Box<dyn Fn(&T, &Env) -> bool>),
  Icon(Box<dyn Fn(&T, &Env) -> CopyIcon>),
}

trait FromFn<T, U> {
  fn from_fn<F>(v: F) -> BoolIcon<T>
  where
    F: Fn(&T, &Env) -> U + 'static;
}

impl<T> FromFn<T, bool> for BoolIcon<T> {
  fn from_fn<F>(v: F) -> BoolIcon<T>
  where
    F: Fn(&T, &Env) -> bool + 'static,
  {
    BoolIcon::Bool(Box::new(v))
  }
}

impl<T> FromFn<T, CopyIcon> for BoolIcon<T> {
  fn from_fn<F>(v: F) -> BoolIcon<T>
  where
    F: Fn(&T, &Env) -> CopyIcon + 'static,
  {
    BoolIcon::Icon(Box::new(v))
  }
}
