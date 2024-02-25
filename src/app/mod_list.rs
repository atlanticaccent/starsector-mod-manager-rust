use std::{
  collections::{HashMap, HashSet},
  ops::Deref,
  path::{Path, PathBuf},
  rc::Rc,
  sync::Arc,
};

use druid::{
  im::Vector,
  kurbo::Line,
  lens, theme,
  widget::{Checkbox, Either, Flex, Label, List, ListIter, Painter, Scroll, SizedBox},
  Color, Data, Env, EventCtx, ExtEventSink, KeyOrValue, Lens, LensExt, LifeCycleCtx, Rect,
  RenderContext, Selector, Target, Widget, WidgetExt,
};
use druid_widget_nursery::{
  Stack, StackChildParams, StackChildPosition, WidgetExt as WidgetExtNursery,
};
use internment::Intern;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter};
use sublime_fuzzy::best_match;

use super::{
  app_delegate::AppCommands,
  controllers::{ExtensibleController, HeightLinkerShared, SharedHoverState},
  installer::HybridPath,
  mod_entry::{GameVersion, ModEntry, ModMetadata, UpdateStatus},
  util::{self, xxHashMap, LoadBalancer, SaveError, WidgetExtEx, WithHoverState as _},
  App,
};
use crate::{
  app::util::StarsectorVersionDiff,
  nav_bar::{Nav, NavLabel},
  patch::table::{ComplexTableColumnWidth, FlexTable, TableColumnWidth, TableRow},
  widgets::card::Card,
};

pub mod filters;
pub mod headings;
pub mod install;
pub mod search;
use self::{
  filters::{filter_button::FilterButton, filter_options::FilterOptions, FILTER_POSITION},
  headings::{Header, Heading},
  install::{install_button::InstallButton, install_options::InstallOptions, InstallState},
  search::Search,
};

static UPDATE_BALANCER: LoadBalancer<Arc<ModEntry>, Vec<Arc<ModEntry>>, Vec<Arc<ModEntry>>> =
  LoadBalancer::new(ModList::SUBMIT_ENTRY);

#[derive(Clone, Data, Lens)]
pub struct ModList {
  pub mods: xxHashMap<String, Arc<ModEntry>>,
  pub header: Header,
  search_text: String,
  #[data(eq)]
  active_filters: HashSet<Filters>,
  starsector_version: Option<GameVersion>,
  install_state: InstallState,
  #[data(ignore)]
  filter_state: ((bool, bool), StackChildPosition),
  pub install_dir_available: bool,
}

impl ModList {
  pub const SUBMIT_ENTRY: Selector<Vec<Arc<ModEntry>>> = Selector::new("mod_list.submit_entry");
  pub const OVERWRITE: Selector<(PathBuf, HybridPath, Arc<ModEntry>)> =
    Selector::new("mod_list.install.overwrite");
  pub const AUTO_UPDATE: Selector<Arc<ModEntry>> = Selector::new("mod_list.install.auto_update");
  pub const SEARCH_UPDATE: Selector<bool> = Selector::new("mod_list.filter.search.update");
  pub const FILTER_UPDATE: Selector<(Filters, bool)> = Selector::new("mod_list.filter.update");
  pub const FILTER_RESET: Selector = Selector::new("mod_list.filter.reset");
  pub const DUPLICATE: Selector<(Arc<ModEntry>, Arc<ModEntry>)> =
    Selector::new("mod_list.submit_entry.duplicate");

  pub const UPDATE_COLUMN_WIDTH: Selector<(usize, f64)> =
    Selector::new("mod_list.column.update_width");
  const UPDATE_TABLE_SORT: Selector = Selector::new("mod_list.table.update_sorting");

  pub fn new(headings: Vector<Heading>) -> Self {
    Self {
      mods: xxHashMap::new(),
      header: Header::new(headings),
      search_text: String::new(),
      active_filters: HashSet::new(),
      starsector_version: None,
      install_state: InstallState::default(),
      filter_state: Default::default(),
      install_dir_available: false,
    }
  }

  pub fn view() -> impl Widget<Self> {
    Stack::new()
      .with_child(
        Flex::column()
          .with_child(
            Flex::row()
              .with_child(
                InstallButton::view()
                  .lens(Self::install_state)
                  .padding((0.0, 5.0))
                  .disabled_if(|data, _| !data.install_dir_available),
              )
              .with_flex_spacer(1.0)
              .with_child(
                FilterButton::view()
                  .lens(Self::filter_state.then(lens!(((bool, bool), StackChildPosition), 0))),
              )
              .with_child(
                Search::view()
                  .on_change(|ctx, old, data, _| {
                    if !old.same(&data) {
                      ctx.submit_command(Self::SEARCH_UPDATE.with(!data.is_empty()));
                      ctx.submit_command(Self::UPDATE_TABLE_SORT)
                    }
                  })
                  .scope(|curr| Search::new(curr), Search::buffer)
                  .lens(Self::search_text),
              )
              .expand_width(),
          )
          .with_flex_child(
            Card::builder()
              .with_insets((0.0, 14.0))
              .with_corner_radius(4.0)
              .with_shadow_length(6.0)
              .build(
                Flex::column()
                  .with_child(headings::Header::view().lens(ModList::header))
                  .with_flex_child(
                    FlexTable::default()
                      .row_background(Painter::new(move |ctx, _, env| {
                        let rect = ctx.size().to_rect();

                        if env.try_get(FlexTable::<u64>::ROW_NUM).unwrap_or(0) % 2 == 0 {
                          ctx.fill(rect, &env.get(theme::BACKGROUND_DARK))
                        } else {
                          ctx.fill(rect, &env.get(theme::BACKGROUND_LIGHT))
                        }
                      }))
                      .with_column_width(TableColumnWidth::Fixed(Header::ENABLED_WIDTH))
                      .column_border(theme::BORDER_DARK, 1.0)
                      .controller(
                        ExtensibleController::new()
                          .on_command(Self::UPDATE_COLUMN_WIDTH, Self::column_resized)
                          .on_command(Self::UPDATE_TABLE_SORT, Self::on_mod_list_change)
                          .on_command(Self::SUBMIT_ENTRY, Self::entry_submitted)
                          .on_command(Self::SEARCH_UPDATE, |_, _, searching, data| {
                            if *searching {
                              data.header.sort_by = (Heading::Score, true)
                            } else {
                              data.header.sort_by = (Heading::Name, true)
                            };
                            false
                          })
                          .on_command(ModMetadata::SUBMIT_MOD_METADATA, Self::metadata_submitted)
                          .on_command(Self::FILTER_UPDATE, Self::on_filter_change)
                          .on_command(Self::FILTER_RESET, Self::on_filter_reset)
                          .on_added(Self::init_table),
                      )
                      .scroll()
                      .vertical()
                      .expand_width(),
                    1.0,
                  )
                  .on_change(|ctx, old, data, _| {
                    if !old.header.same(&data.header) || !old.mods.same(&data.mods) {
                      ctx.submit_command(Self::UPDATE_TABLE_SORT)
                    }
                  }),
              ),
            1.0,
          ),
      )
      .with_positioned_child(
        InstallOptions::view()
          .lens(Self::install_state)
          .padding((0.0, 5.0)),
        StackChildPosition::default().top(Some(0.0)).left(Some(0.0)),
      )
      .with_positioned_child(
        FilterOptions::view()
          .lens(Self::filter_state.then(lens!(((bool, bool), StackChildPosition), 0))),
        StackChildParams::dynamic(|data: &ModList, _| &data.filter_state.1).duration(0.0),
      )
      .with_positioned_child(
        FilterOptions::wide_view()
          .lens(Self::filter_state.then(lens!(((bool, bool), StackChildPosition), 0))),
        StackChildPosition::default()
          .top(Some(54.0))
          .left(Some(0.0))
          .right(Some(0.0)),
      )
      .fit(true)
      .on_command(FILTER_POSITION, |ctx, point, data| {
        let rect = Rect::from_points(ctx.window_origin(), *point);
        data.filter_state.1.top = Some(rect.height());
        data.filter_state.1.left = Some(rect.width());
      })
  }

  fn append_table(
    table: &mut FlexTable<ModList>,
    mods: &xxHashMap<String, Arc<ModEntry>>,
    headings: &Vector<Heading>,
  ) {
    for id in mods.keys() {
      let intern = Intern::new(id.clone());

      let len = headings.len();
      let painter = |idx: usize| {
        Painter::new(move |ctx, data: &(Arc<ModEntry>, SharedHoverState), env| {
          if data.1.get() {
            let rect = ctx.size().to_rect().inset(-0.5);
            ctx.stroke(
              Line::new((rect.x0, rect.y0), (rect.x1, rect.y0)),
              &env.get(theme::BORDER_DARK),
              1.0,
            );
            ctx.stroke(
              Line::new((rect.x0, rect.y1), (rect.x1, rect.y1)),
              &env.get(theme::BORDER_DARK),
              1.0,
            );
            if idx == 0 {
              ctx.stroke(
                Line::new((rect.x0, rect.y0), (rect.x0, rect.y1)),
                &env.get(theme::BORDER_DARK),
                1.0,
              );
            }
            if idx == len {
              ctx.stroke(
                Line::new((rect.x1, rect.y0), (rect.x1, rect.y1)),
                &env.get(theme::BORDER_DARK),
                1.0,
              );
            }
          }
        })
      };

      let mut shared_linker: Option<HeightLinkerShared> = None;
      let hover_state = SharedHoverState::default();
      let _intern = intern.as_ref();
      let mut row = TableRow::new(id.clone()).with_child(
        Checkbox::new("")
          .center()
          .padding(5.)
          .lens(lens!((Arc<ModEntry>, SharedHoverState), 0).then(ModEntry::enabled.in_arc()))
          .padding(2.0)
          .background(painter(0))
          .with_hover_state(hover_state.clone())
          .lens(ModList::mods.deref().index(intern.as_ref()))
          .link_height_with(&mut shared_linker),
      );

      for (idx, heading) in headings.iter().enumerate() {
        if let Some(cell) = ModEntry::view_cell(*heading) {
          row.add_child(
            cell
              .lens(lens!((Arc<ModEntry>, SharedHoverState), 0))
              .padding(2.0)
              .background(painter(idx + 1))
              .with_hover_state(hover_state.clone())
              .on_click(|ctx, data, _| {
                ctx.submit_command(Nav::NAV_SELECTOR.with(NavLabel::ModDetails));
                ctx.submit_command(
                  App::SELECTOR.with(AppCommands::UpdateModDescription(data.id.clone())),
                );
              })
              .lens(ModList::mods.deref().index(intern.as_ref()))
              .link_height_with(&mut shared_linker),
          )
        }
      }
      table.add_row(row)
    }
  }

  fn init_table(
    table: &mut FlexTable<ModList>,
    ctx: &mut LifeCycleCtx,
    data: &ModList,
    _env: &Env,
  ) {
    Self::append_table(table, &data.mods, &data.header.headings);
    ctx.children_changed();
  }

  fn entry_submitted(
    table: &mut FlexTable<ModList>,
    ctx: &mut EventCtx,
    payload: &Vec<Arc<ModEntry>>,
    data: &mut ModList,
  ) -> bool {
    let old = data.mods.clone();
    for entry in payload {
      *data.mods = data.mods.alter(
        |existing| {
          if let Some(inner) = &existing {
            ctx.submit_command(ModList::DUPLICATE.with((inner.clone(), entry.clone())));
            existing
          } else {
            Some(entry.clone())
          }
        },
        entry.id.clone(),
      );
    }
    if !old.same(&data.mods) {
      let diff = data
        .mods
        .clone()
        .deref()
        .clone()
        .relative_complement(old.into())
        .into();
      Self::append_table(table, &diff, &data.header.headings);
    }
    ctx.children_changed();
    false
  }

  fn metadata_submitted(
    _table: &mut FlexTable<ModList>,
    _ctx: &mut EventCtx,
    (id, metadata): &(String, ModMetadata),
    data: &mut ModList,
  ) -> bool {
    ModList::mods
      .deref()
      .index(id)
      .then(ModEntry::manager_metadata.in_arc())
      .put(data, metadata.clone());

    false
  }

  fn column_resized(
    table: &mut FlexTable<ModList>,
    ctx: &mut EventCtx,
    payload: &(usize, f64),
    _data: &mut ModList,
  ) -> bool {
    if !table.rows().is_empty() {
      let column_count = table.column_count();
      let widths = table.get_column_widths();
      if widths.len() < column_count {
        widths.resize_with(column_count, || {
          ComplexTableColumnWidth::Simple(TableColumnWidth::Flex(1.0))
        })
      }
      widths[payload.0] = ComplexTableColumnWidth::Simple(TableColumnWidth::Fixed(payload.1 - 1.0));

      ctx.request_layout();
    }

    false
  }

  fn on_mod_list_change(
    table: &mut FlexTable<ModList>,
    ctx: &mut EventCtx,
    _payload: &(),
    data: &mut ModList,
  ) -> bool {
    let sorted_vec = data.sorted_vals();
    let sorted_map: HashMap<&str, usize> = sorted_vec
      .iter()
      .enumerate()
      .map(|(idx, entry)| (entry.id.as_str(), idx))
      .collect();
    table
      .rows()
      .sort_unstable_by_key(|row| sorted_map.get(row.id()));
    table
      .rows()
      .iter_mut()
      .for_each(|row| *row.visible() = sorted_map.contains_key(row.id()));
    ctx.request_layout();
    ctx.request_paint();
    false
  }

  pub fn on_app_data_change(
    _ctx: &mut EventCtx,
    old: &super::App,
    data: &mut super::App,
    _env: &druid::Env,
  ) {
    if let Some(install_dir) = &data.settings.install_dir {
      let diff = old
        .mod_list
        .mods
        .deref()
        .clone()
        .difference_with(data.mod_list.mods.clone().into(), |left, right| {
          (left.enabled != right.enabled).then_some(right)
        });

      if !diff.is_empty() {
        let enabled: Vec<String> = data
          .mod_list
          .mods
          .values()
          .filter_map(|v| v.enabled.then_some(v.id.clone()))
          .collect();
        if let Err(err) = EnabledMods::from(enabled).save(install_dir) {
          eprintln!("{:?}", err)
        };
      }
    }
  }

  fn on_filter_change(
    table: &mut FlexTable<ModList>,
    ctx: &mut EventCtx,
    payload: &(Filters, bool),
    data: &mut ModList,
  ) -> bool {
    if payload.1 {
      data.active_filters.insert(payload.0)
    } else {
      data.active_filters.remove(&payload.0)
    };
    data.filter_state.0 .1 = !data.active_filters.is_empty();
    Self::on_mod_list_change(table, ctx, &(), data);

    false
  }

  fn on_filter_reset(
    _: &mut FlexTable<ModList>,
    ctx: &mut EventCtx,
    _: &(),
    data: &mut ModList,
  ) -> bool {
    data.active_filters.clear();
    data.filter_state.0 .1 = false;
    ctx.children_changed();
    ctx.request_update();
    ctx.request_layout();
    ctx.request_paint();

    true
  }

  pub fn _view() -> impl Widget<Self> {
    Flex::column()
      .with_child(headings::Header::view().lens(ModList::header))
      .with_flex_child(
        Either::new(
          |data: &ModList, _| !data.mods.is_empty(),
          Scroll::new(
            List::new(|| {
              SizedBox::empty()
                .expand_width()
                .lens(lens::Map::new(
                  |val: &EntryAlias| (val.0.clone(), val.2.clone(), val.3.clone()),
                  |_, _| {},
                ))
                .background(Painter::new(
                  |ctx, (entry, i, ratios, headings, game_version): &EntryAlias, env| {
                    let rect = ctx.size().to_rect();
                    // manually paint cells here to indicate version info
                    // set ratios in ModList through a command listener on this widget
                    // implement update status parser
                    // calculate cell widths using ratios and paint appropriately
                    fn calc_pos(idx: usize, ratios: &Vector<f64>, width: f64) -> f64 {
                      if idx == 0 {
                        0.
                      } else if idx == 1 {
                        (ratios[idx - 1] * width) + 3.
                      } else {
                        let prev = calc_pos(idx - 1, ratios, width);
                        prev + ((width - prev) * ratios[idx - 1]) + 3.
                      }
                    }

                    if i % 2 == 0 {
                      ctx.fill(rect, &env.get(theme::BACKGROUND_DARK))
                    } else {
                      ctx.fill(rect, &env.get(theme::BACKGROUND_LIGHT))
                    }
                    if let Some(idx) = headings.index_of(&Heading::Version) {
                      if let Some(local) = &entry.version_checker {
                        let update_status = UpdateStatus::from((local, &entry.remote_version));

                        let enabled_shift = (headings::Header::ENABLED_WIDTH) * rect.width();
                        let mut row_origin = rect.origin();
                        row_origin.x += enabled_shift + 3.;
                        let row_rect = rect.with_origin(row_origin).intersect(rect);

                        let cell_left =
                          row_rect.origin().x + calc_pos(idx, ratios, row_rect.width());
                        let cell_right = if idx < ratios.len() {
                          row_rect.origin().x + calc_pos(idx + 1, ratios, row_rect.width())
                        } else {
                          row_rect.max_x()
                        };
                        let cell_0_rect = Rect::from_points(
                          (cell_left, row_rect.origin().y),
                          (cell_right, row_rect.height()),
                        );

                        let color = <KeyOrValue<Color>>::from(&update_status).resolve(env);
                        ctx.fill(cell_0_rect, &color)
                      }
                    }
                    if let Some(idx) = headings.index_of(&Heading::GameVersion) {
                      if let Some(game_version) = game_version.as_ref() {
                        let diff = StarsectorVersionDiff::from((&entry.game_version, game_version));
                        let enabled_shift = (headings::Header::ENABLED_WIDTH) * rect.width();
                        let mut row_origin = rect.origin();
                        row_origin.x += enabled_shift + 3.;
                        let row_rect = rect.with_origin(row_origin).intersect(rect);

                        let cell_left =
                          row_rect.origin().x + calc_pos(idx, ratios, row_rect.width());
                        let cell_right = if idx < ratios.len() {
                          row_rect.origin().x + calc_pos(idx + 1, ratios, row_rect.width())
                        } else {
                          row_rect.max_x()
                        };
                        let cell_0_rect = Rect::from_points(
                          (cell_left, row_rect.origin().y),
                          (cell_right, row_rect.height()),
                        );

                        let color = <KeyOrValue<Color>>::from(diff).resolve(env);
                        ctx.fill(cell_0_rect, &color)
                      }
                    }
                  },
                ))
            })
            .background(theme::BACKGROUND_LIGHT)
            .on_command(ModEntry::REPLACE, |ctx, payload, data: &mut ModList| {
              data.mods.insert(payload.id.clone(), payload.clone());
              ctx.children_changed();
            })
            .on_command(ModList::SEARCH_UPDATE, |ctx, _, data| {
              data.header.sort_by = (Heading::Score, true);
              ctx.children_changed()
            }),
          )
          .vertical(),
          Label::new("No mods")
            .expand()
            .background(theme::BACKGROUND_LIGHT),
        ),
        1.,
      )
      .on_command(ModList::SUBMIT_ENTRY, |ctx, payload, data| {
        for entry in payload {
          *data.mods = data.mods.alter(
            |existing| {
              if let Some(inner) = &existing {
                ctx.submit_command(ModList::DUPLICATE.with((inner.clone(), entry.clone())));
                existing
              } else {
                Some(entry.clone())
              }
            },
            entry.id.clone(),
          );
        }
      })
      .on_command(util::MASTER_VERSION_RECEIVED, |_ctx, payload, data| {
        if let Some(mut entry) = data.mods.get(&payload.0).cloned() {
          let remote = payload.1.as_ref().ok().cloned();
          ModEntry::remote_version
            .in_arc()
            .put(&mut entry, remote.clone());
          if let Some(version_checker) = &entry.version_checker {
            let status = UpdateStatus::from((version_checker, &remote));
            ModEntry::update_status
              .in_arc()
              .put(&mut entry, Some(status));
          }
          data.mods.insert(entry.id.clone(), entry);
        };
      })
      .on_command(
        ModMetadata::SUBMIT_MOD_METADATA,
        |_ctx, (id, metadata), data| {
          if let Some(mut entry) = data.mods.remove(id) {
            ModEntry::manager_metadata
              .in_arc()
              .put(&mut entry, metadata.clone());

            data.mods.insert(id.clone(), entry);
          }
        },
      )
  }

  pub async fn parse_mod_folder(
    event_sink: Option<ExtEventSink>,
    root_dir: Option<PathBuf>,
  ) -> Option<Vec<Arc<ModEntry>>> {
    let handle = tokio::runtime::Handle::current();

    if let Some(root_dir) = root_dir {
      let mod_dir = root_dir.join("mods");
      let enabled_mods_filename = mod_dir.join("enabled_mods.json");

      let enabled_mods = if !enabled_mods_filename.exists() {
        vec![]
      } else if let Ok(enabled_mods_text) = std::fs::read_to_string(enabled_mods_filename)
        && let Ok(EnabledMods { enabled_mods }) =
          serde_json::from_str::<EnabledMods>(&enabled_mods_text)
      {
        enabled_mods
      } else {
        return None;
      };

      if let Ok(dir_iter) = std::fs::read_dir(mod_dir) {
        let enabled_mods_iter = enabled_mods.par_iter();

        let mods = dir_iter
          .par_bridge()
          .filter_map(|entry| entry.ok())
          .filter(|entry| {
            if let Ok(file_type) = entry.file_type() {
              file_type.is_dir()
            } else {
              false
            }
          })
          .filter_map(
            |entry| match ModEntry::from_file(&entry.path(), ModMetadata::default()) {
              Ok(mut mod_info) => {
                mod_info.set_enabled(
                  enabled_mods_iter
                    .clone()
                    .find_any(|id| mod_info.id.clone().eq(*id))
                    .is_some(),
                );
                Some(Arc::new(mod_info))
              }
              Err(err) => {
                eprintln!("Failed to get mod info for mod at: {:?}", entry.path());
                eprintln!("With err: {:?}", err);
                None
              }
            },
          );

        if let Some(event_sink) = event_sink.as_ref() {
          mods.for_each(|entry| {
            let tx = {
              let _guard = handle.enter();

              UPDATE_BALANCER.sender(event_sink.clone())
            };

            if let Err(err) = tx.send(entry.clone()) {
              eprintln!("Failed to submit found mod {}", err);
            };
            if let Some(version) = entry.version_checker.clone() {
              handle.spawn(util::get_master_version(Some(event_sink.clone()), version));
            }
            if ModMetadata::path(&entry.path).exists() {
              handle.spawn(ModMetadata::parse_and_send(
                entry.id.clone(),
                entry.path.clone(),
                Some(event_sink.clone()),
              ));
            }
          });

          if let Err(err) = event_sink.submit_command(super::App::ENABLE, (), Target::Auto) {
            eprintln!("{:?}", err)
          }
        } else {
          let mods = mods.map(|mut entry| {
            let mut mut_entry = Arc::make_mut(&mut entry);
            if let Some(version) = mut_entry.version_checker.clone() {
              let master_version = handle.block_on(util::get_master_version(None, version.clone()));
              mut_entry.remote_version = master_version.clone();
              mut_entry.update_status = Some(UpdateStatus::from((&version, &master_version)));
            }
            if ModMetadata::path(&mut_entry.path).exists() {
              if let Some(mod_metadata) = handle.block_on(ModMetadata::parse_and_send(
                mut_entry.id.clone(),
                mut_entry.path.clone(),
                None,
              )) {
                mut_entry.manager_metadata = mod_metadata;
              }
            }

            entry
          });
          return Some(mods.collect::<Vec<_>>());
        }
      };
    }

    None
  }

  fn sorted_vals(&self) -> Vec<Arc<ModEntry>> {
    let mut values: Vec<Arc<ModEntry>> = self
      .mods
      .iter()
      .filter_map(|(_, entry)| {
        let search = if let Heading::Score = self.header.sort_by.0 {
          if !self.search_text.is_empty() {
            let id_score = best_match(&self.search_text, &entry.id).map(|m| m.score());
            let name_score = best_match(&self.search_text, &entry.name).map(|m| m.score());
            let author_score = best_match(&self.search_text, &entry.author).map(|m| m.score());

            id_score.is_some() || name_score.is_some() || author_score.is_some()
          } else {
            true
          }
        } else {
          true
        };
        let filters = self.active_filters.par_iter().all(|f| f.as_fn()(entry));

        (search && filters).then(|| entry.clone())
      })
      .collect();

    values.par_sort_unstable_by(|a, b| {
      let ord = match self.header.sort_by.0 {
        Heading::ID => a.id.cmp(&b.id),
        Heading::Name => a.name.cmp(&b.name),
        Heading::Author => a.author.cmp(&b.author),
        Heading::GameVersion => a.game_version.cmp(&b.game_version),
        Heading::Enabled => a.enabled.cmp(&b.enabled),
        Heading::Version => match (a.update_status.as_ref(), b.update_status.as_ref()) {
          (None, None) => a.name.cmp(&b.name),
          (_, _) if a.update_status.cmp(&b.update_status) == std::cmp::Ordering::Equal => {
            a.name.cmp(&b.name)
          }
          (_, _) => a.update_status.cmp(&b.update_status),
        },
        Heading::Score => {
          let scoring = |entry: &Arc<ModEntry>| -> Option<isize> {
            let id_score = best_match(&self.search_text, &entry.id).map(|m| m.score());
            let name_score = best_match(&self.search_text, &entry.name).map(|m| m.score());
            let author_score = best_match(&self.search_text, &entry.author).map(|m| m.score());

            std::cmp::max(std::cmp::max(id_score, name_score), author_score)
          };

          scoring(a).cmp(&scoring(b))
        }
        Heading::AutoUpdateSupport => a
          .remote_version
          .as_ref()
          .and_then(|r| r.direct_download_url.as_ref())
          .is_some()
          .cmp(
            &b.remote_version
              .as_ref()
              .and_then(|r| r.direct_download_url.as_ref())
              .is_some(),
          ),
        Heading::InstallDate => a
          .manager_metadata
          .install_date
          .cmp(&b.manager_metadata.install_date),
      };

      if self.header.sort_by.1 {
        ord.reverse()
      } else {
        ord
      }
    });
    values
  }
}

type EntryAlias = (
  Arc<ModEntry>,
  usize,
  Vector<f64>,
  Vector<Heading>,
  Rc<Option<GameVersion>>,
);

impl ListIter<EntryAlias> for ModList {
  fn for_each(&self, mut cb: impl FnMut(&EntryAlias, usize)) {
    let ratios = self.header.ratios.clone();
    let headers = self.header.headings.clone();
    let game_version = Rc::new(self.starsector_version.clone());

    for (i, item) in self.sorted_vals().into_iter().enumerate() {
      cb(
        &(
          item,
          i,
          ratios.clone(),
          headers.clone(),
          game_version.clone(),
        ),
        i,
      );
    }
  }

  fn for_each_mut(&mut self, mut cb: impl FnMut(&mut EntryAlias, usize)) {
    let ratios = self.header.ratios.clone();
    let headers = self.header.headings.clone();
    let game_version = Rc::new(self.starsector_version.clone());

    for (i, item) in self.sorted_vals().iter_mut().enumerate() {
      cb(
        &mut (
          item.clone(),
          i,
          ratios.clone(),
          headers.clone(),
          game_version.clone(),
        ),
        i,
      );
    }
  }

  fn data_len(&self) -> usize {
    self.mods.len()
  }
}

#[derive(Serialize, Deserialize)]
pub struct EnabledMods {
  #[serde(rename = "enabledMods")]
  enabled_mods: Vec<String>,
}

impl EnabledMods {
  pub fn empty() -> Self {
    Self {
      enabled_mods: Vec::new(),
    }
  }

  pub fn save(self, path: &Path) -> Result<(), SaveError> {
    use std::{fs, io::Write};

    let json = serde_json::to_string_pretty(&self).map_err(|_| SaveError::Format)?;

    let mut file =
      fs::File::create(path.join("mods").join("enabled_mods.json")).map_err(|_| SaveError::File)?;

    file
      .write_all(json.as_bytes())
      .map_err(|_| SaveError::Write)
  }
}

impl From<Vec<Arc<ModEntry>>> for EnabledMods {
  fn from(from: Vec<Arc<ModEntry>>) -> Self {
    Self {
      enabled_mods: from.iter().map(|v| v.id.clone()).collect(),
    }
  }
}

impl From<Vec<String>> for EnabledMods {
  fn from(enabled_mods: Vec<String>) -> Self {
    Self { enabled_mods }
  }
}

#[derive(Clone, Copy, Eq, PartialEq, Hash, Data, EnumIter, Display, Debug)]
pub enum Filters {
  Enabled,
  Disabled,
  Unimplemented,
  Error,
  Discrepancy,
  #[strum(to_string = "Up To Date")]
  UpToDate,
  Patch,
  Minor,
  Major,
  #[strum(to_string = "Auto Update Available")]
  AutoUpdateAvailable,
  #[strum(to_string = "Auto Update Unsupported")]
  AutoUpdateUnsupported,
}

impl Filters {
  fn as_fn(&self) -> impl FnMut(&Arc<ModEntry>) -> bool {
    match self {
      Filters::Enabled => |entry: &Arc<ModEntry>| !entry.enabled,
      Filters::Disabled => |entry: &Arc<ModEntry>| entry.enabled,
      Filters::Unimplemented => |entry: &Arc<ModEntry>| entry.version_checker.is_some(),
      Filters::Error => |entry: &Arc<ModEntry>| entry.update_status != Some(UpdateStatus::Error),
      Filters::UpToDate => {
        |entry: &Arc<ModEntry>| entry.update_status == Some(UpdateStatus::UpToDate)
      }
      Filters::Discrepancy => {
        |entry: &Arc<ModEntry>| matches!(entry.update_status, Some(UpdateStatus::Discrepancy(_)))
      }
      Filters::Patch => {
        |entry: &Arc<ModEntry>| matches!(entry.update_status, Some(UpdateStatus::Patch(_)))
      }
      Filters::Minor => {
        |entry: &Arc<ModEntry>| matches!(entry.update_status, Some(UpdateStatus::Minor(_)))
      }
      Filters::Major => {
        |entry: &Arc<ModEntry>| matches!(entry.update_status, Some(UpdateStatus::Major(_)))
      }
      Filters::AutoUpdateAvailable => |entry: &Arc<ModEntry>| {
        entry
          .remote_version
          .as_ref()
          .and_then(|r| r.direct_download_url.as_ref())
          .is_some()
      },
      Filters::AutoUpdateUnsupported => |entry: &Arc<ModEntry>| {
        entry
          .remote_version
          .as_ref()
          .and_then(|r| r.direct_download_url.as_ref())
          .is_none()
      },
    }
  }
}
