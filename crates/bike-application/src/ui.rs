//! GPUI views (S-SHELL H3–H11): top bar, side view, Specification panel,
//! bottom tray and status bar. The logic they show lives in `document`,
//! `panel` and `scene`; this module only lays it out and routes input.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use bike_model::{LengthUnit, Material, Severity, WheelSize};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Disableable as _, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::document::{Command, Data, Document, End};
use crate::panel::{self, Field, FieldView, Section};
use crate::scene::{self, Role, Scene, Shape};
use crate::settings::{self, Settings, Theme};

actions!(
    bikestudio,
    [NewProject, OpenProject, SaveProject, SaveProjectAs]
);

/// Key bindings (H10): Cmd on macOS, Ctrl elsewhere.
pub fn bind_keys(cx: &mut App) {
    #[cfg(target_os = "macos")]
    let keys = ["cmd-n", "cmd-o", "cmd-s", "cmd-shift-s"];
    #[cfg(not(target_os = "macos"))]
    let keys = ["ctrl-n", "ctrl-o", "ctrl-s", "ctrl-shift-s"];
    cx.bind_keys([
        KeyBinding::new(keys[0], NewProject, None),
        KeyBinding::new(keys[1], OpenProject, None),
        KeyBinding::new(keys[2], SaveProject, None),
        KeyBinding::new(keys[3], SaveProjectAs, None),
    ]);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tray {
    Diagnostics,
    Curves,
    Feel,
    Assistant,
}

impl Tray {
    const ALL: [Tray; 4] = [Tray::Diagnostics, Tray::Curves, Tray::Feel, Tray::Assistant];

    fn label(self) -> &'static str {
        match self {
            Tray::Diagnostics => "Diagnostics",
            Tray::Curves => "Curves",
            Tray::Feel => "Feel",
            Tray::Assistant => "Assistant",
        }
    }
}

/// Pan and zoom on top of Fit (H7).
#[derive(Debug, Clone, Copy)]
struct ViewTransform {
    zoom: f32,
    pan: Point<Pixels>,
}

impl Default for ViewTransform {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: point(px(0.0), px(0.0)),
        }
    }
}

/// The dark palette (H12): one colour per scene role.
mod palette {
    use gpui_kit::{Hsla, hsla};

    pub fn background() -> Hsla {
        hsla(220. / 360., 0.08, 0.08, 1.0)
    }
    pub fn panel() -> Hsla {
        hsla(220. / 360., 0.07, 0.11, 1.0)
    }
    pub fn border() -> Hsla {
        hsla(220. / 360., 0.06, 0.2, 1.0)
    }
    pub fn text() -> Hsla {
        hsla(0., 0., 0.9, 1.0)
    }
    pub fn muted() -> Hsla {
        hsla(0., 0., 0.55, 1.0)
    }
    pub fn accent() -> Hsla {
        hsla(210. / 360., 0.8, 0.62, 1.0)
    }
    pub fn unsaved() -> Hsla {
        hsla(35. / 360., 0.9, 0.55, 1.0)
    }
    pub fn saved() -> Hsla {
        hsla(140. / 360., 0.6, 0.5, 1.0)
    }
    pub fn error() -> Hsla {
        hsla(0., 0.75, 0.6, 1.0)
    }
    pub fn grid() -> Hsla {
        hsla(220. / 360., 0.06, 0.15, 1.0)
    }

    /// Stroke colour and hairline width for each role.
    pub fn role(role: crate::scene::Role) -> (Hsla, f32) {
        use crate::scene::Role::*;
        match role {
            Ground => (hsla(0., 0., 0.35, 1.0), 1.0),
            Tyre => (hsla(0., 0., 0.42, 1.0), 2.0),
            Rim => (hsla(0., 0., 0.3, 1.0), 1.0),
            Frame => (hsla(0., 0., 0.78, 1.0), 1.0),
            Fork => (hsla(0., 0., 0.6, 1.0), 1.2),
            DriveUnit => (hsla(30. / 360., 0.5, 0.6, 1.0), 1.2),
            Ghost => (hsla(210. / 360., 0.3, 0.5, 0.35), 1.0),
            Centreline => (hsla(0., 0., 0.4, 1.0), 1.0),
            Dimension => (accent(), 1.0),
            Origin => (accent(), 1.5),
        }
    }
}

/// The studio window's root view.
pub struct Studio {
    data: Rc<Data>,
    doc: Document,
    settings: Settings,
    settings_path: PathBuf,
    views: Vec<FieldView>,
    inputs: BTreeMap<Field, Entity<InputState>>,
    focused: Option<Field>,
    /// Shown when the solve fails (H8).
    last_scene: Option<Scene>,
    transform: ViewTransform,
    drag_from: Option<Point<Pixels>>,
    tray: Tray,
    show_tubes: bool,
    show_cranks: bool,
    settings_open: bool,
    /// A typed value that didn't parse, or a file error.
    message: Option<String>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl Studio {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_settings(settings::default_path(), window, cx)
    }

    /// A studio reading and writing settings at `settings_path` (tests use a temp file).
    pub fn with_settings(
        settings_path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut studio = Self {
            data: Rc::new(Data::builtin()),
            doc: Document::new(),
            settings: Settings::load(&settings_path),
            settings_path,
            views: Vec::new(),
            inputs: BTreeMap::new(),
            focused: None,
            last_scene: None,
            transform: ViewTransform::default(),
            drag_from: None,
            tray: Tray::Diagnostics,
            show_tubes: false,
            show_cranks: false,
            settings_open: false,
            message: None,
            focus: cx.focus_handle(),
            _subscriptions: Vec::new(),
        };
        studio.refresh(window, cx);

        let this = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            this.update(cx, |this, cx| this.should_close(window, cx))
                .unwrap_or(true)
        });
        studio
    }

    // ----- Commands and refresh -----

    fn apply(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        self.doc.apply(command);
        self.refresh(window, cx);
    }

    /// Solves, rebuilds the panel's fields and updates every input not being edited.
    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.doc.solve(&self.data);
        self.views = panel::fields(&self.doc, &self.data, self.settings.display_unit);
        for view in self.views.clone() {
            let field = view.field;
            let input = match self.inputs.get(&field) {
                Some(input) => input.clone(),
                None => {
                    let input = cx.new(|cx| InputState::new(window, cx));
                    let sub =
                        cx.subscribe_in(&input, window, move |this, input, event, window, cx| {
                            this.on_input(field, input, event, window, cx)
                        });
                    self._subscriptions.push(sub);
                    self.inputs.insert(field, input.clone());
                    input
                }
            };
            let editing = input.read(cx).focus_handle(cx).is_focused(window);
            if !editing || self.focused != Some(field) {
                input.update(cx, |state, cx| {
                    if state.value() != view.text.as_str() {
                        state.set_value(view.text.clone(), window, cx);
                    }
                });
            }
        }
        let scene = self.scene();
        if scene.is_some() {
            self.last_scene = scene;
        }
        cx.notify();
    }

    fn scene(&self) -> Option<Scene> {
        let focused = match self.focused {
            Some(Field::Geometry(v)) => Some(v),
            _ => None,
        };
        scene::build(
            &self.doc.solved,
            &self.doc.components,
            self.doc.project.front_travel.map_or(0.0, |t| t.0),
            focused,
            self.settings.display_unit,
        )
    }

    fn on_input(
        &mut self,
        field: Field,
        input: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Focus => {
                self.focused = Some(field);
                self.refresh(window, cx);
            }
            InputEvent::Blur | InputEvent::PressEnter { .. } => {
                if matches!(event, InputEvent::Blur) && self.focused == Some(field) {
                    self.focused = None;
                }
                let text = input.read(cx).value().to_string();
                self.commit(field, &text, window, cx);
            }
            _ => {}
        }
    }

    /// Commits `text` to a panel field, as Enter or leaving the field does.
    pub fn commit(
        &mut self,
        field: Field,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(view) = self.views.iter().find(|v| v.field == field).cloned() else {
            return;
        };
        match panel::commit(&self.doc, &view, text, self.settings.display_unit) {
            Ok(Some(command)) => {
                self.message = None;
                self.apply(command, window, cx);
            }
            Ok(None) => self.refresh(window, cx),
            Err(e) => {
                self.message = Some(format!("{}: {e}", view.label));
                // Put the displayed value back.
                self.refresh_input(field, &view.text, window, cx);
                cx.notify();
            }
        }
    }

    fn refresh_input(
        &mut self,
        field: Field,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(input) = self.inputs.get(&field).cloned() {
            let text = text.to_string();
            input.update(cx, |state, cx| state.set_value(text, window, cx));
        }
    }

    fn save_settings(&mut self) {
        if let Err(e) = self.settings.save(&self.settings_path) {
            self.message = Some(format!("Couldn't save settings: {e}"));
        }
    }

    // ----- Files (H10) -----

    /// Runs `then` once unsaved changes are saved or discarded; Cancel stops.
    fn confirm_discard(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) {
        if !self.doc.dirty {
            then(self, window, cx);
            return;
        }
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("Save changes to {}?", self.doc.name()),
            Some("Your changes will be lost if you don't save them."),
            &["Save", "Don't save", "Cancel"],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            let Ok(choice) = answer.await else { return };
            let _ = this.update_in(cx, |this, window, cx| match choice {
                0 => this.save_then(window, cx, then),
                1 => then(this, window, cx),
                _ => {}
            });
        })
        .detach();
    }

    /// Saves (asking for a path if there's none), then runs `then`.
    fn save_then(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) {
        match self.doc.save() {
            Ok(true) => {
                self.saved_path();
                then(self, window, cx);
            }
            Ok(false) => self.save_as_then(window, cx, then),
            Err(e) => self.message = Some(e.to_string()),
        }
        cx.notify();
    }

    fn save_as_then(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) {
        let folder = self.settings.projects_folder.clone();
        let _ = std::fs::create_dir_all(&folder);
        let name = format!("{}.{}", self.doc.name(), crate::document::EXTENSION);
        let rx = cx.prompt_for_new_path(&folder, Some(&name));
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(path))) = rx.await else { return };
            let _ = this.update_in(cx, |this, window, cx| {
                match this.doc.save_as(&path) {
                    Ok(()) => {
                        this.saved_path();
                        then(this, window, cx);
                    }
                    Err(e) => this.message = Some(e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn saved_path(&mut self) {
        if let Some(path) = self.doc.path.clone() {
            self.settings.opened(&path);
            self.save_settings();
        }
    }

    fn new_project(&mut self, _: &NewProject, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_discard(window, cx, |this, window, cx| {
            this.load(Document::new(), window, cx);
        });
    }

    fn open_project(&mut self, _: &OpenProject, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_discard(window, cx, |_, window, cx| {
            let rx = cx.prompt_for_paths(PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: Some("Open".into()),
            });
            cx.spawn_in(window, async move |this, cx| {
                let Ok(Ok(Some(paths))) = rx.await else {
                    return;
                };
                let Some(path) = paths.into_iter().next() else {
                    return;
                };
                let _ = this.update_in(cx, |this, window, cx| this.open_path(&path, window, cx));
            })
            .detach();
        });
    }

    fn open_recent(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_discard(window, cx, move |this, window, cx| {
            this.open_path(&path, window, cx)
        });
    }

    pub fn open_path(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        match Document::open(path) {
            Ok(doc) => {
                self.settings.opened(path);
                self.save_settings();
                self.load(doc, window, cx);
            }
            Err(e) => {
                self.message = Some(format!("Couldn't open {}: {e}", path.display()));
                cx.notify();
            }
        }
    }

    fn load(&mut self, doc: Document, window: &mut Window, cx: &mut Context<Self>) {
        self.doc = doc;
        self.focused = None;
        self.message = None;
        self.last_scene = None;
        self.transform = ViewTransform::default();
        self.refresh(window, cx);
    }

    fn save_project(&mut self, _: &SaveProject, window: &mut Window, cx: &mut Context<Self>) {
        self.save_then(window, cx, |_, _, _| {});
    }

    fn save_project_as(&mut self, _: &SaveProjectAs, window: &mut Window, cx: &mut Context<Self>) {
        self.save_as_then(window, cx, |_, _, _| {});
    }

    fn change_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update_in(cx, |this, _, cx| {
                this.settings.projects_folder = path;
                this.save_settings();
                cx.notify();
            });
        })
        .detach();
    }

    /// The window's close button (H10): ask first when there are unsaved changes.
    fn should_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.doc.dirty {
            return true;
        }
        self.confirm_discard(window, cx, |this, window, _| {
            this.doc.dirty = false;
            window.remove_window();
        });
        false
    }

    /// The open document, for tests.
    pub fn document(&self) -> &Document {
        &self.doc
    }

    /// The message in the status bar, for tests.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Saves to `path` without a dialog, for tests.
    pub fn save_to(&mut self, path: &Path, cx: &mut Context<Self>) {
        if let Err(e) = self.doc.save_as(path) {
            self.message = Some(e.to_string());
        }
        cx.notify();
    }

    // ----- Rendering -----

    fn render_top_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        let recent = self.settings.recent.clone();
        let folder = self.settings.projects_folder.display().to_string();
        let bikes = Button::new("bikes")
            .ghost()
            .small()
            .label("Bikes")
            .dropdown_caret(true)
            .dropdown_menu(move |menu, _, _| {
                let item = |label: &str, f: fn(&mut Studio, &mut Window, &mut Context<Studio>)| {
                    let this = this.clone();
                    PopupMenuItem::new(label.to_string()).on_click(move |_, window, cx| {
                        let _ = this.update(cx, |s, cx| f(s, window, cx));
                    })
                };
                let mut menu = menu
                    .item(item("New project", |s, w, cx| {
                        s.new_project(&NewProject, w, cx)
                    }))
                    .item(item("Open…", |s, w, cx| {
                        s.open_project(&OpenProject, w, cx)
                    }))
                    .item(item(&format!("Change folder…  {folder}"), |s, w, cx| {
                        s.change_folder(w, cx)
                    }))
                    .separator();
                if !recent.is_empty() {
                    menu = menu.label("Recent");
                    for path in &recent {
                        let this = this.clone();
                        let path = path.clone();
                        let name = path
                            .file_stem()
                            .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
                        menu =
                            menu.item(PopupMenuItem::new(name).on_click(move |_, window, cx| {
                                let path = path.clone();
                                let _ = this.update(cx, |s, cx| s.open_recent(path, window, cx));
                            }));
                    }
                    menu = menu.separator();
                }
                menu.item(item("Save", |s, w, cx| s.save_project(&SaveProject, w, cx)))
                    .item(item("Save as…", |s, w, cx| {
                        s.save_project_as(&SaveProjectAs, w, cx)
                    }))
            });

        let dot = if self.doc.dirty {
            palette::unsaved()
        } else {
            palette::saved()
        };
        let modes = h_flex()
            .gap_1()
            .p_0p5()
            .rounded_md()
            .bg(palette::panel())
            .child(
                Button::new("mode-spec")
                    .small()
                    .label("Specification")
                    .selected(true),
            )
            .child(
                Button::new("mode-link")
                    .small()
                    .ghost()
                    .label("Linkage")
                    .disabled(true),
            )
            .child(
                Button::new("mode-analysis")
                    .small()
                    .ghost()
                    .label("Analysis")
                    .disabled(true),
            );

        h_flex()
            .h(px(44.0))
            .px_3()
            .justify_between()
            .border_b_1()
            .border_color(palette::border())
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(bikes)
                    .child(div().text_color(palette::muted()).child("/"))
                    .child(div().text_color(palette::text()).child(self.doc.name()))
                    .child(div().size(px(7.0)).rounded_full().bg(dot)),
            )
            .child(modes)
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        Button::new("settings")
                            .ghost()
                            .small()
                            .label("Settings")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.settings_open = true;
                                cx.notify();
                            })),
                    )
                    .child(div().text_color(palette::text()).child("bikestudio")),
            )
    }

    fn render_side_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let live = self.scene().is_some();
        let scene = self.last_scene.clone();
        let transform = self.transform;
        let caption = if !live && scene.is_some() {
            "Side view · can't solve, showing the last good drawing"
        } else if self.doc.is_preview() {
            "Side view · preview from defaults"
        } else {
            "Side view"
        };
        div()
            .id("side-view")
            .relative()
            .flex_1()
            .size_full()
            .bg(palette::background())
            .on_scroll(cx.listener(|this, e: &ScrollEvent, _, cx| {
                let dy = e.delta.pixel_delta(px(20.0)).y;
                let factor = (1.0 + f32::from(dy) / 400.0).clamp(0.5, 2.0);
                this.transform.zoom = (this.transform.zoom * factor).clamp(0.2, 20.0);
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(|this, e: &MouseDownEvent, _, _| this.drag_from = Some(e.position)),
            )
            .on_mouse_up(
                MouseButton::Middle,
                cx.listener(|this, _: &MouseUpEvent, _, _| this.drag_from = None),
            )
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                if let Some(from) = this.drag_from {
                    if e.pressed_button == Some(MouseButton::Middle) {
                        this.transform.pan += e.position - from;
                        this.drag_from = Some(e.position);
                        cx.notify();
                    } else {
                        this.drag_from = None;
                    }
                }
            }))
            .child(
                canvas(
                    |bounds, _, _| bounds,
                    move |bounds, _, window, cx| {
                        paint_grid(bounds, window);
                        if let Some(scene) = &scene {
                            paint_scene(scene, bounds, transform, !live, window, cx);
                        }
                    },
                )
                .size_full(),
            )
            .child(
                div()
                    .absolute()
                    .top_3()
                    .left_4()
                    .text_sm()
                    .text_color(palette::muted())
                    .child(caption),
            )
            .child(
                div().absolute().top_2().right_3().child(
                    Button::new("fit")
                        .small()
                        .ghost()
                        .label("Fit")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.transform = ViewTransform::default();
                            cx.notify();
                        })),
                ),
            )
    }

    fn render_field(&self, view: &FieldView, cx: &mut Context<Self>) -> impl IntoElement {
        let field = view.field;
        let input = self.inputs.get(&field).cloned();
        let lock = view.locked.map(|locked| {
            Button::new(SharedString::from(format!("lock-{field:?}")))
                .small()
                .ghost()
                .label(if locked { "●" } else { "○" })
                .selected(locked)
                .tooltip(if locked {
                    "Pinned: click to free"
                } else {
                    "Free: click to pin"
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    if let Some(command) = panel::toggle_lock(&this.doc, field) {
                        this.apply(command, window, cx);
                    }
                }))
        });
        let suffix = if view.is_default {
            format!("default · {}", view.unit)
        } else {
            view.unit.to_string()
        };
        h_flex()
            .gap_2()
            .items_center()
            .h(px(32.0))
            .child(
                div()
                    .w(px(150.0))
                    .text_sm()
                    .text_color(palette::text())
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(view.label.clone()),
            )
            .child(div().w(px(28.0)).children(lock))
            .child(
                div()
                    .flex_1()
                    .when(view.dimmed, |d| d.opacity(0.6))
                    .when_some(input, |d, input| {
                        d.child(
                            Input::new(&input)
                                .small()
                                .suffix(div().text_xs().text_color(palette::muted()).child(suffix)),
                        )
                    }),
            )
    }

    fn render_wheels(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut keys: Vec<String> = self
            .data
            .defaults
            .wheel_sizes
            .sizes
            .keys()
            .cloned()
            .collect();
        keys.sort_by(|a, b| {
            let n = |s: &str| s.parse::<f64>().unwrap_or(f64::MAX);
            n(a).total_cmp(&n(b))
        });
        let default = self.data.defaults.geometry.default_wheel.clone();
        let row = |end: End, current: Option<&WheelSize>, cx: &mut Context<Self>| {
            let selected = current
                .map_or(default.as_str(), |w| w.0.as_str())
                .to_string();
            h_flex()
                .gap_2()
                .items_center()
                .h(px(32.0))
                .child(
                    div()
                        .w(px(178.0))
                        .text_sm()
                        .text_color(palette::text())
                        .child(panel::end_label(end)),
                )
                .child(h_flex().gap_0p5().children(keys.iter().map(|key| {
                    let size = WheelSize(key.clone());
                    Button::new(SharedString::from(format!("wheel-{end:?}-{key}")))
                        .small()
                        .ghost()
                        .label(key.clone())
                        .selected(*key == selected)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.apply(Command::SetWheel(end, Some(size.clone())), window, cx)
                        }))
                })))
        };
        v_flex()
            .child(section_title("Wheels"))
            .child(row(End::Front, self.doc.project.front_wheel.as_ref(), cx))
            .child(row(End::Rear, self.doc.project.rear_wheel.as_ref(), cx))
    }

    fn render_material(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.doc.project.tubes.material;
        let shown = self.doc.solved.tubes.as_ref().map(|t| t.material);
        let choices = std::iter::once(None).chain(Material::ALL.into_iter().map(Some));
        v_flex()
            .gap_1()
            .py_1()
            .child(
                div()
                    .text_sm()
                    .text_color(palette::text())
                    .child("Material"),
            )
            .child(h_flex().gap_0p5().flex_wrap().children(choices.map(|m| {
                let label = match (m, shown) {
                    (Some(m), _) => m.label().to_string(),
                    (None, Some(s)) => format!("Default ({})", s.label()),
                    (None, None) => "Default".to_string(),
                };
                Button::new(SharedString::from(format!("material-{m:?}")))
                    .small()
                    .ghost()
                    .label(label)
                    .selected(current == m)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let command = panel::set_material(&this.doc, m);
                        this.apply(command, window, cx)
                    }))
            })))
    }

    fn render_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let section = |s: Section, this: &Self, cx: &mut Context<Self>| {
            v_flex().children(
                this.views
                    .iter()
                    .filter(|v| v.section == s)
                    .map(|v| this.render_field(v, cx).into_any_element())
                    .collect::<Vec<_>>(),
            )
        };
        let toggle = |id: &'static str,
                      label: &'static str,
                      open: bool,
                      cx: &mut Context<Self>,
                      f: fn(&mut Studio)| {
            Button::new(id)
                .small()
                .ghost()
                .label(format!("{} {label}", if open { "▾" } else { "▸" }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    f(this);
                    cx.notify();
                }))
        };
        v_flex()
            .id("panel")
            .w(px(420.0))
            .h_full()
            .overflow_y_scroll()
            .p_3()
            .gap_1()
            .bg(palette::panel())
            .border_l_1()
            .border_color(palette::border())
            .child(
                div()
                    .text_color(palette::text())
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Specification"),
            )
            .child(self.render_wheels(cx))
            .child(section_title(Section::Travel.label()))
            .child(section(Section::Travel, self, cx))
            .child(section_title(Section::Geometry.label()))
            .child(section(Section::Geometry, self, cx))
            .child(section_title(Section::Frame.label()))
            .child(self.render_material(cx))
            .child(section(Section::Frame, self, cx))
            .child(toggle("toggle-tubes", "Tubes", self.show_tubes, cx, |s| {
                s.show_tubes = !s.show_tubes
            }))
            .when(self.show_tubes, |d| {
                d.child(section(Section::Tubes, self, cx))
            })
            .child(toggle(
                "toggle-cranks",
                "Cranks",
                self.show_cranks,
                cx,
                |s| s.show_cranks = !s.show_cranks,
            ))
            .when(self.show_cranks, |d| {
                d.child(section(Section::Cranks, self, cx))
            })
    }

    fn render_tray(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = h_flex().gap_1().children(Tray::ALL.map(|tab| {
            let label = if tab == Tray::Diagnostics {
                format!("{} {}", tab.label(), self.doc.solved.diagnostics.len())
            } else {
                tab.label().to_string()
            };
            Button::new(SharedString::from(format!("tray-{tab:?}")))
                .small()
                .ghost()
                .label(label)
                .selected(self.tray == tab)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.tray = tab;
                    cx.notify();
                }))
        }));
        let body = match self.tray {
            Tray::Diagnostics => v_flex()
                .children(self.doc.solved.diagnostics.iter().map(|d| {
                    let (icon, colour) = match d.severity {
                        Severity::Info => ("ⓘ", palette::accent()),
                        Severity::Warning => ("⚠", palette::unsaved()),
                        Severity::Error => ("✕", palette::error()),
                    };
                    h_flex()
                        .gap_3()
                        .py_1()
                        .border_b_1()
                        .border_color(palette::border())
                        .text_sm()
                        .child(div().w(px(16.0)).text_color(colour).child(icon))
                        .child(
                            div()
                                .w(px(200.0))
                                .font_family("monospace")
                                .text_color(palette::muted())
                                .child(d.code.clone()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_color(palette::text())
                                .child(d.message.clone()),
                        )
                }))
                .into_any_element(),
            _ => div()
                .p_4()
                .text_sm()
                .text_color(palette::muted())
                .child("Coming soon")
                .into_any_element(),
        };
        v_flex()
            .id("tray")
            .h(px(230.0))
            .p_2()
            .gap_2()
            .bg(palette::panel())
            .border_t_1()
            .border_color(palette::border())
            .child(tabs)
            .child(
                div()
                    .id("tray-body")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(body),
            )
    }

    fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (dot, text) = if self.doc.dirty {
            (palette::unsaved(), "Unsaved changes")
        } else {
            (palette::saved(), "Saved")
        };
        let unit = self.settings.display_unit;
        let unit_button = |u: LengthUnit, cx: &mut Context<Self>| {
            Button::new(SharedString::from(format!("unit-{}", u.label())))
                .small()
                .ghost()
                .label(u.label())
                .selected(unit == u)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.settings.display_unit = u;
                    this.save_settings();
                    this.refresh(window, cx);
                }))
        };
        h_flex()
            .h(px(28.0))
            .px_3()
            .justify_between()
            .items_center()
            .text_xs()
            .border_t_1()
            .border_color(palette::border())
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().size(px(7.0)).rounded_full().bg(dot))
                    .child(div().text_color(palette::muted()).child(text))
                    .when_some(self.message.clone(), |d, m| {
                        d.child(div().text_color(palette::error()).child(m))
                    }),
            )
            .child(
                h_flex()
                    .gap_0p5()
                    .child(unit_button(LengthUnit::Millimeters, cx))
                    .child(unit_button(LengthUnit::Inches, cx)),
            )
    }

    fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.settings.theme;
        div()
            .id("settings-backdrop")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(hsla(0., 0., 0., 0.5))
            .child(
                v_flex()
                    .id("settings")
                    .w(px(560.0))
                    .p_4()
                    .gap_3()
                    .rounded_lg()
                    .bg(palette::panel())
                    .border_1()
                    .border_color(palette::border())
                    .child(
                        div()
                            .text_color(palette::text())
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Settings"),
                    )
                    .child(section_title("Appearance"))
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(div().w(px(120.0)).text_sm().child("Theme"))
                            .child(
                                Button::new("theme-dark")
                                    .small()
                                    .label("Dark")
                                    .selected(theme == Theme::Dark),
                            )
                            .child(
                                Button::new("theme-drafting")
                                    .small()
                                    .ghost()
                                    .label("Drafting (coming soon)")
                                    .disabled(true),
                            ),
                    )
                    .child(section_title("Projects"))
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(div().w(px(120.0)).text_sm().child("Folder"))
                            .child(
                                Button::new("folder")
                                    .small()
                                    .label(self.settings.projects_folder.display().to_string())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.change_folder(window, cx)
                                    })),
                            ),
                    )
                    .child(
                        h_flex().justify_end().child(
                            Button::new("settings-close")
                                .small()
                                .primary()
                                .label("Done")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.settings_open = false;
                                    cx.notify();
                                })),
                        ),
                    ),
            )
    }
}

fn section_title(title: &'static str) -> impl IntoElement {
    div()
        .pt_3()
        .pb_1()
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(palette::text())
        .child(title)
}

impl Render for Studio {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("studio")
            .key_context("Studio")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::new_project))
            .on_action(cx.listener(Self::open_project))
            .on_action(cx.listener(Self::save_project))
            .on_action(cx.listener(Self::save_project_as))
            .relative()
            .size_full()
            .bg(palette::background())
            .text_color(palette::text())
            .child(self.render_top_bar(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(self.render_side_view(cx))
                            .child(self.render_tray(cx)),
                    )
                    .child(self.render_panel(cx)),
            )
            .child(self.render_status_bar(cx))
            .when(self.settings_open, |d| d.child(self.render_settings(cx)))
    }
}

// ----- Side view painting -----

fn paint_grid(bounds: Bounds<Pixels>, window: &mut Window) {
    window.paint_quad(fill(bounds, palette::background()));
    let step = 40.0;
    let mut builder = PathBuilder::stroke(px(1.0));
    let (x0, y0) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
    let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let mut x = x0;
    while x < x0 + w {
        builder.move_to(point(px(x), px(y0)));
        builder.line_to(point(px(x), px(y0 + h)));
        x += step;
    }
    let mut y = y0;
    while y < y0 + h {
        builder.move_to(point(px(x0), px(y)));
        builder.line_to(point(px(x0 + w), px(y)));
        y += step;
    }
    if let Ok(path) = builder.build() {
        window.paint_path(path, palette::grid());
    }
}

/// Model millimetres to screen pixels: Fit, then the user's zoom and pan.
struct Mapper {
    centre: Point<Pixels>,
    mid: bike_model::Point,
    scale: f32,
}

impl Mapper {
    fn new(scene: &Scene, bounds: Bounds<Pixels>, t: ViewTransform) -> Option<Self> {
        let (lo, hi) = scene.bounds()?;
        let margin = 0.9;
        let w = (hi.x - lo.x).max(1.0) as f32;
        let h = (hi.y - lo.y).max(1.0) as f32;
        let fit =
            (f32::from(bounds.size.width) / w).min(f32::from(bounds.size.height) / h) * margin;
        Some(Self {
            centre: bounds.center() + t.pan,
            mid: bike_model::Point::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0),
            scale: fit * t.zoom,
        })
    }

    fn map(&self, p: bike_model::Point) -> Point<Pixels> {
        point(
            self.centre.x + px((p.x - self.mid.x) as f32 * self.scale),
            self.centre.y - px((p.y - self.mid.y) as f32 * self.scale),
        )
    }
}

fn paint_scene(
    scene: &Scene,
    bounds: Bounds<Pixels>,
    transform: ViewTransform,
    stale: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(m) = Mapper::new(scene, bounds, transform) else {
        return;
    };
    window.with_content_mask(Some(ContentMask { bounds }), |window| {
        for item in &scene.items {
            let (mut colour, hairline) = palette::role(item.role);
            if stale {
                colour = colour.opacity(0.35);
            }
            match &item.shape {
                Shape::Line { a, b, width } => {
                    let w = width.map_or(hairline, |w| (w as f32 * m.scale).max(hairline));
                    let mut builder = PathBuilder::stroke(px(w));
                    builder.move_to(m.map(*a));
                    builder.line_to(m.map(*b));
                    if let Ok(path) = builder.build() {
                        let colour = if width.is_some() && item.role == Role::Frame {
                            colour.opacity(if stale { 0.2 } else { 0.55 })
                        } else {
                            colour
                        };
                        window.paint_path(path, colour);
                    }
                    if width.is_some() {
                        // The centreline over the tube's body.
                        let mut builder = PathBuilder::stroke(px(1.0));
                        builder.move_to(m.map(*a));
                        builder.line_to(m.map(*b));
                        if let Ok(path) = builder.build() {
                            window.paint_path(path, palette::role(Role::Frame).0);
                        }
                    }
                }
                Shape::Polygon(points) => {
                    let mut builder = PathBuilder::stroke(px(hairline));
                    let points: Vec<_> = points.iter().map(|p| m.map(*p)).collect();
                    builder.add_polygon(&points, true);
                    if let Ok(path) = builder.build() {
                        window.paint_path(path, colour);
                    }
                }
                Shape::Circle { centre, radius } => {
                    let sides = 96;
                    let points: Vec<_> = (0..sides)
                        .map(|i| {
                            let a = std::f64::consts::TAU * f64::from(i) / f64::from(sides);
                            m.map(bike_model::Point::new(
                                centre.x + radius * a.cos(),
                                centre.y + radius * a.sin(),
                            ))
                        })
                        .collect();
                    let mut builder = PathBuilder::stroke(px(hairline));
                    builder.add_polygon(&points, true);
                    if let Ok(path) = builder.build() {
                        window.paint_path(path, colour);
                    }
                }
                Shape::Label { at, text } => {
                    let font_size = px(12.0);
                    let run = TextRun {
                        len: text.len(),
                        font: font("monospace"),
                        color: colour,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let line = window.text_system().shape_line(
                        text.clone().into(),
                        font_size,
                        &[run],
                        None,
                    );
                    let origin = m.map(*at) - point(line.width / 2.0, px(8.0));
                    let _ = line.paint(origin, px(16.0), TextAlign::Left, None, window, cx);
                }
            }
        }
    });
}
