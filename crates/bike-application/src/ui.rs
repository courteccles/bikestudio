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
use crate::settings::{self, Settings};

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
    Tubes,
    Curves,
    Feel,
    Assistant,
}

impl Tray {
    const ALL: [Tray; 5] = [
        Tray::Diagnostics,
        Tray::Tubes,
        Tray::Curves,
        Tray::Feel,
        Tray::Assistant,
    ];

    fn label(self) -> &'static str {
        match self {
            Tray::Diagnostics => "Diagnostics",
            Tray::Tubes => "Tubes",
            Tray::Curves => "Curves",
            Tray::Feel => "Feel",
            Tray::Assistant => "Assistant",
        }
    }
}

/// Points the component theme at the mockup palette, so popup menus and
/// buttons share the segmented controls' colours. Call after `Theme::change`.
pub fn apply_theme(cx: &mut App) {
    use gpui_kit::component::Theme;
    Theme::update(cx, |t| {
        t.background = palette::background();
        t.foreground = palette::text();
        t.border = palette::border();
        t.input = palette::border();
        t.popover = palette::popover();
        t.popover_foreground = palette::text();
        t.accent = palette::hover();
        t.accent_foreground = palette::text();
        t.secondary = palette::strip();
        t.secondary_hover = palette::hover();
        t.secondary_active = palette::selected();
        t.secondary_foreground = palette::label();
        t.muted_foreground = palette::muted();
        t.list_active = palette::hover();
    });
}

/// A segmented control in the style of the Specification / Linkage /
/// Analysis bar: a rounded strip holding small buttons.
fn segmented() -> Div {
    h_flex().gap_1().p_0p5().rounded_md().bg(palette::strip())
}

/// One segment: filled when selected, ghost otherwise.
fn segment(id: impl Into<ElementId>, label: impl Into<SharedString>, selected: bool) -> Button {
    Button::new(id)
        .small()
        .ghost()
        .label(label)
        .selected(selected)
}

/// Pan and zoom on top of Fit (H7).
#[derive(Debug, Clone, Copy)]
struct ViewTransform {
    zoom: f32,
    pan: Point<Pixels>,
    /// The model extent Fit framed. Held until the next Fit, so editing the
    /// bike (a wheel size, say) doesn't rescale or shift the view.
    frame: Option<(bike_model::Point, bike_model::Point)>,
}

impl Default for ViewTransform {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: point(px(0.0), px(0.0)),
            frame: None,
        }
    }
}

/// The dark palette (H12): one colour per scene role.
mod palette {
    use gpui_kit::{Hsla, hsla, rgb};

    // Sampled from the Specification mockups (`designer experience images`).
    fn hex(c: u32) -> Hsla {
        rgb(c).into()
    }
    /// The shell behind everything: top bar, margins, status bar.
    pub fn background() -> Hsla {
        hex(0x0f1112)
    }
    /// The side view behind the grid.
    pub fn view() -> Hsla {
        hex(0x131517)
    }
    /// The Specification panel card.
    pub fn panel() -> Hsla {
        hex(0x151719)
    }
    /// The tray card.
    pub fn tray() -> Hsla {
        hex(0x141618)
    }
    /// Full-width dividers under the panel heading and the tray tabs.
    pub fn divider() -> Hsla {
        hex(0x262626)
    }
    /// Popup menus.
    pub fn popover() -> Hsla {
        hex(0x151719)
    }
    /// The strip behind a segmented control.
    pub fn strip() -> Hsla {
        hex(0x191b1d)
    }
    /// A selected segment.
    pub fn selected() -> Hsla {
        hex(0x3a3b40)
    }
    /// A hovered segment or menu item.
    pub fn hover() -> Hsla {
        hex(0x27292b)
    }
    pub fn input() -> Hsla {
        hex(0x121416)
    }
    pub fn border() -> Hsla {
        hex(0x232527)
    }
    pub fn text() -> Hsla {
        hex(0xeaeaec)
    }
    /// Field labels, inactive segments, the Bikes menu and the gear.
    pub fn label() -> Hsla {
        hex(0xb2b2b8)
    }
    /// Units, captions and diagnostic codes.
    pub fn muted() -> Hsla {
        hex(0x85858a)
    }
    /// Diagnostic messages.
    pub fn message() -> Hsla {
        hex(0xcfcfd4)
    }
    /// The status bar text.
    pub fn status() -> Hsla {
        hex(0xa3a3a3)
    }
    /// Grid lines below the ground, dimmer as in the mockups.
    pub fn grid_below() -> Hsla {
        hex(0x0e1012)
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
        hex(0x1b1e21)
    }
    /// Below the ground line.
    pub fn earth() -> Hsla {
        hex(0x0a0c0d)
    }
    pub fn hatch() -> Hsla {
        hex(0x2c2f33)
    }

    /// Stroke colour, hairline width and fill for each role (H12).
    pub fn role(role: crate::scene::Role) -> (Hsla, f32, Option<Hsla>) {
        use crate::scene::Role::*;
        match role {
            Ground => (hex(0x272a2d), 1.0, None),
            Tyre => (
                hsla(0., 0., 0.34, 1.0),
                1.0,
                Some(hsla(220. / 360., 0.05, 0.17, 1.0)),
            ),
            Rim => (hsla(0., 0., 0.3, 1.0), 1.0, None),
            Spoke => (hsla(0., 0., 0.22, 1.0), 0.6, None),
            Hub => (
                hsla(0., 0., 0.45, 1.0),
                1.0,
                Some(hsla(220. / 360., 0.05, 0.14, 1.0)),
            ),
            Frame => (
                hsla(0., 0., 0.62, 1.0),
                1.0,
                Some(hsla(220. / 360., 0.06, 0.24, 0.9)),
            ),
            Fork => (
                hsla(0., 0., 0.55, 1.0),
                1.0,
                Some(hsla(220. / 360., 0.06, 0.2, 0.9)),
            ),
            DriveUnit => (
                hsla(30. / 360., 0.5, 0.6, 1.0),
                1.2,
                Some(hsla(30. / 360., 0.2, 0.2, 0.9)),
            ),
            Ghost => (hsla(210. / 360., 0.3, 0.5, 0.35), 1.0, None),
            Centreline => (hsla(0., 0., 0.42, 0.8), 0.8, None),
            Dimension => (accent(), 1.0, None),
            Extension => (accent().opacity(0.45), 0.8, None),
            Origin => (accent(), 1.5, None),
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
        self.redraw(cx);
    }

    /// Rebuilds the drawing only, for changes that don't touch the model,
    /// such as which field's dimension is highlighted.
    fn redraw(&mut self, cx: &mut Context<Self>) {
        let scene = self.scene();
        if scene.is_some() {
            self.last_scene = scene;
        }
        if self.transform.frame.is_none() {
            self.transform.frame = self.last_scene.as_ref().and_then(Scene::bounds);
        }
        cx.notify();
    }

    fn scene(&self) -> Option<Scene> {
        scene::build(
            &self.doc.solved,
            &self.doc.components,
            self.doc.project.front_travel.map_or(0.0, |t| t.0),
            self.focused,
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
                // Only the highlighted dimension changes: no solve needed.
                self.focused = Some(field);
                self.select_all_later(input, window, cx);
                self.redraw(cx);
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

    /// Selects the whole value so typing replaces it. Deferred so it lands
    /// after the click or key that focused the field has placed the cursor.
    fn select_all_later(
        &self,
        input: &Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = input.clone();
        cx.defer_in(window, move |_, window, cx| {
            input.update(cx, |state, cx| state.select_all(window, cx));
        });
    }

    /// Up and Down arrows: step the value, commit it and keep it selected.
    fn step_field(
        &mut self,
        field: Field,
        up: bool,
        big: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(view) = self.views.iter().find(|v| v.field == field).cloned() else {
            return;
        };
        let Some(input) = self.inputs.get(&field).cloned() else {
            return;
        };
        let text = input.read(cx).value().to_string();
        let Some(next) = panel::step(&view, &text, up, big, self.settings.display_unit) else {
            return;
        };
        self.refresh_input(field, &next, window, cx);
        self.commit(field, &next, window, cx);
        self.select_all_later(&input, window, cx);
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
            // Nothing changed: tidy the field's text and redraw, without a solve.
            Ok(None) => {
                self.refresh_input(field, &view.text, window, cx);
                self.redraw(cx);
            }
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
            input.update(cx, |state, cx| {
                if state.value() != text.as_str() {
                    state.set_value(text, window, cx);
                }
            });
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
        let bikes_menu = Button::new("bikes")
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
        let modes = segmented()
            .child(segment("mode-spec", "Specification", true))
            .child(segment("mode-link", "Linkage", false).disabled(true))
            .child(segment("mode-analysis", "Analysis", false).disabled(true));

        h_flex()
            .relative()
            .h(px(44.0))
            .pl(SHELL_MARGIN)
            .pr(px(16.0))
            .justify_between()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(segmented().child(bikes_menu))
                    .child(div().text_color(palette::muted()).child("/"))
                    .child(div().text_color(palette::text()).child(self.doc.name()))
                    .child(div().size(px(7.0)).rounded_full().bg(dot)),
            )
            // Centred on the window, whatever the widths on either side.
            .child(
                h_flex()
                    .absolute()
                    .inset_0()
                    .justify_center()
                    .items_center()
                    .child(modes),
            )
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    // No settings dialog yet (H11); the gear is in place for it.
                    .child(
                        Button::new("settings")
                            .ghost()
                            .small()
                            .icon(gpui_kit::component::IconName::Settings)
                            .tooltip("Settings"),
                    )
                    .child(div().text_color(palette::text()).child("bikestudio")),
            )
    }

    /// The grid and drawing fill the whole shell; the panel and tray float
    /// over it, and Fit frames the bike in the space they leave (H7).
    /// The framing Re-center would give: the bike as it is now, no pan or zoom.
    fn centred(&self) -> ViewTransform {
        ViewTransform {
            frame: self.last_scene.as_ref().and_then(Scene::bounds),
            ..ViewTransform::default()
        }
    }

    fn is_centred(&self) -> bool {
        let c = self.centred();
        let t = self.transform;
        t.zoom == c.zoom && t.pan == c.pan && t.frame == c.frame
    }

    /// Re-center: over the ground, just above the tray's right end. Enabled only when
    /// the view has been panned or zoomed, or the bike has changed size.
    fn render_recenter(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("recenter")
            .occlude()
            .absolute()
            // Right-aligned with the tray, beside the panel.
            .right(CARD_GAP + PANEL_WIDTH + CARD_GAP)
            .bottom(CARD_GAP + TRAY_HEIGHT + px(8.0))
            .child(
                segmented().child(
                    segment("recenter-button", "Re-center", false)
                        .disabled(self.is_centred())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.transform = this.centred();
                            cx.notify();
                        })),
                ),
            )
    }

    fn render_side_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let live = self.scene().is_some();
        let scene = self.last_scene.clone();
        let transform = self.transform;
        div()
            .id("side-view")
            .absolute()
            .top_0()
            .left(SHELL_MARGIN)
            .bottom_0()
            .right_0()
            // Open on the right: the grid runs to the edge of the shell.
            .border_t_1()
            .border_l_1()
            .border_b_1()
            .border_color(palette::divider())
            .rounded_tl_lg()
            .rounded_bl_lg()
            .overflow_hidden()
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
                        paint_view(scene.as_ref(), bounds, transform, !live, window, cx);
                    },
                )
                .size_full(),
            )
    }

    fn render_field(&self, view: &FieldView, cx: &mut Context<Self>) -> impl IntoElement {
        let field = view.field;
        let input = self.inputs.get(&field).cloned();
        let lock = view.locked.map(|locked| {
            Button::new(SharedString::from(format!("lock-{field:?}")))
                .xsmall()
                .outline()
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
        let read_only = view.read_only;
        let value = if read_only {
            div()
                .flex_1()
                .px_2()
                .text_sm()
                .text_color(palette::muted())
                .child(format!("{} {}", view.text, view.unit))
                .into_any_element()
        } else {
            div()
                .flex_1()
                .when(view.dimmed, |d| d.opacity(0.6))
                .capture_key_down(cx.listener(move |this, e: &KeyDownEvent, window, cx| {
                    let up = match e.keystroke.key.as_str() {
                        "up" => true,
                        "down" => false,
                        _ => return,
                    };
                    let m = &e.keystroke.modifiers;
                    if m.control || m.alt || m.platform {
                        return;
                    }
                    cx.stop_propagation();
                    this.step_field(field, up, m.shift, window, cx);
                }))
                .when_some(input, |d, input| {
                    d.child(
                        Input::new(&input)
                            .small()
                            .bg(palette::input())
                            .suffix(div().text_xs().text_color(palette::muted()).child(suffix)),
                    )
                })
                .into_any_element()
        };
        h_flex()
            .pl(ROW_INDENT)
            .gap_2()
            .items_center()
            .h(px(32.0))
            .child(
                div()
                    .w(px(16.0))
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(palette::accent())
                    .children(view.letter.map(|c| c.to_string())),
            )
            .child(
                div()
                    .w(px(134.0))
                    .text_sm()
                    .text_color(palette::label())
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(view.label.clone()),
            )
            .child(div().w(px(28.0)).children(lock))
            .child(value)
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
                .pl(ROW_INDENT)
                // Same columns as `render_field`: letter, label, lock, value.
                .child(div().w(px(16.0)))
                .child(
                    div()
                        .w(px(134.0))
                        .text_sm()
                        .text_color(palette::label())
                        .child(panel::end_label(end)),
                )
                .child(div().w(px(28.0)))
                .child(segmented().children(keys.iter().map(|key| {
                    let size = WheelSize(key.clone());
                    segment(
                        SharedString::from(format!("wheel-{end:?}-{key}")),
                        key.clone(),
                        *key == selected,
                    )
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
                    .text_color(palette::label())
                    .child("Material"),
            )
            .child(segmented().flex_wrap().children(choices.map(|m| {
                let label = match (m, shown) {
                    (Some(m), _) => m.label().to_string(),
                    (None, Some(s)) => format!("Default ({})", s.label()),
                    (None, None) => "Default".to_string(),
                };
                segment(
                    SharedString::from(format!("material-{m:?}")),
                    label,
                    current == m,
                )
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
        v_flex()
            .id("panel")
            .occlude()
            .absolute()
            .top(CARD_GAP)
            .right(CARD_GAP)
            .bottom(CARD_GAP)
            .w(PANEL_WIDTH)
            .overflow_y_scroll()
            .px_4()
            .pt_3()
            .pb_4()
            .gap_1()
            .rounded_lg()
            .bg(palette::panel())
            .border_1()
            .border_color(palette::border())
            .child(
                div()
                    // Cancel the panel's padding so the divider runs edge to edge.
                    .mx(px(-16.0))
                    .px_4()
                    .pb_3()
                    .mb_1()
                    .border_b_1()
                    .border_color(palette::divider())
                    .text_color(palette::text())
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Specification"),
            )
            .child(self.render_wheels(cx))
            .child(section_title(Section::Travel.label()))
            .child(section(Section::Travel, self, cx))
            .child(section_title(Section::Geometry.label()))
            .child(section(Section::Geometry, self, cx))
            .child(section_title(Section::Cranks.label()))
            .child(section(Section::Cranks, self, cx))
    }

    fn render_tray(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = segmented().children(Tray::ALL.map(|tab| {
            let label = if tab == Tray::Diagnostics {
                format!("{} {}", tab.label(), self.doc.solved.diagnostics.len())
            } else {
                tab.label().to_string()
            };
            segment(
                SharedString::from(format!("tray-{tab:?}")),
                label,
                self.tray == tab,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.tray = tab;
                cx.notify();
            }))
        }));
        let tabs = h_flex()
            // Cancel the tray's padding so the divider runs edge to edge.
            .mx(px(-8.0))
            .px_2()
            .pb_2()
            .border_b_1()
            .border_color(palette::divider())
            .child(tabs);
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
                                .text_color(palette::message())
                                .child(d.message.clone()),
                        )
                }))
                .into_any_element(),
            Tray::Tubes => {
                let fields = self
                    .views
                    .iter()
                    .filter(|v| v.section == Section::Tubes)
                    .map(|v| self.render_field(v, cx).into_any_element())
                    .collect::<Vec<_>>();
                v_flex()
                    .gap_1()
                    .child(self.render_material(cx))
                    .child(div().grid().grid_cols(2).gap_x_6().children(fields))
                    .into_any_element()
            }
            _ => div()
                .p_4()
                .text_sm()
                .text_color(palette::muted())
                .child("Coming soon")
                .into_any_element(),
        };
        v_flex()
            .id("tray")
            .occlude()
            .absolute()
            .left(SHELL_MARGIN + CARD_GAP)
            .right(CARD_GAP + PANEL_WIDTH + CARD_GAP)
            .bottom(CARD_GAP)
            .h(TRAY_HEIGHT)
            .p_2()
            .gap_2()
            .rounded_lg()
            .bg(palette::tray())
            .border_1()
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
            .h(px(40.0))
            .px(SHELL_MARGIN)
            .justify_between()
            .items_center()
            .text_xs()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().size(px(7.0)).rounded_full().bg(dot))
                    .child(div().text_color(palette::status()).child(text))
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
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(self.render_side_view(cx))
                    .child(self.render_tray(cx))
                    .child(self.render_recenter(cx))
                    .child(self.render_panel(cx)),
            )
            .child(self.render_status_bar(cx))
    }
}

// ----- Side view painting -----

/// Rows sit this far in from their section title.
const ROW_INDENT: Pixels = px(12.0);
const CARD_GAP: Pixels = px(16.0);
/// Spacing from the mockups: the grid window is inset from the window's left
/// and right edges and runs from the top bar to the status bar.
/// Lines the side view up with the Bikes dropdown in the top bar.
const SHELL_MARGIN: Pixels = px(32.0);
const PANEL_WIDTH: Pixels = px(420.0);
const TRAY_HEIGHT: Pixels = px(250.0);

/// Grid cells are 1 cm in the model, with a heavier line every 10 cm.
const GRID_MM: f64 = 10.0;
const GRID_MAJOR: i64 = 10;
/// Below this spacing the 1 cm lines are left out, so the grid doesn't turn solid.
const MIN_GRID_PX: f32 = 5.0;
const HATCH_DEPTH_PX: f32 = 20.0;
const HATCH_STEP_PX: f32 = 10.0;
/// Finer than the 1 px ground line.
const HATCH_WIDTH_PX: f32 = 0.5;

/// Model millimetres to screen pixels: Fit, then the user's zoom and pan.
struct Mapper {
    centre: Point<Pixels>,
    mid: bike_model::Point,
    scale: f32,
}

impl Mapper {
    fn new(scene: Option<&Scene>, bounds: Bounds<Pixels>, t: ViewTransform) -> Self {
        let (lo, hi) = t
            .frame
            .or_else(|| scene.and_then(Scene::bounds))
            .unwrap_or((
                bike_model::Point::new(-700.0, -350.0),
                bike_model::Point::new(1300.0, 900.0),
            ));
        // Fit inside the space the panel and tray leave uncovered.
        let free = Bounds::new(
            bounds.origin,
            size(
                (bounds.size.width - PANEL_WIDTH - CARD_GAP * 2.0).max(px(100.0)),
                (bounds.size.height - TRAY_HEIGHT - CARD_GAP * 2.0).max(px(100.0)),
            ),
        );
        let margin = 0.9;
        let w = (hi.x - lo.x).max(1.0) as f32;
        let h = (hi.y - lo.y).max(1.0) as f32;
        let fit = (f32::from(free.size.width) / w).min(f32::from(free.size.height) / h) * margin;
        Self {
            centre: free.center() + t.pan,
            mid: bike_model::Point::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0),
            scale: fit * t.zoom,
        }
    }

    fn map(&self, p: bike_model::Point) -> Point<Pixels> {
        point(
            self.centre.x + px((p.x - self.mid.x) as f32 * self.scale),
            self.centre.y - px((p.y - self.mid.y) as f32 * self.scale),
        )
    }

    /// The model point under a screen point.
    fn unmap(&self, p: Point<Pixels>) -> bike_model::Point {
        bike_model::Point::new(
            self.mid.x + f64::from(f32::from(p.x - self.centre.x) / self.scale),
            self.mid.y - f64::from(f32::from(p.y - self.centre.y) / self.scale),
        )
    }
}

fn stroke(window: &mut Window, width: f32, colour: Hsla, f: impl FnOnce(&mut PathBuilder)) {
    let mut builder = PathBuilder::stroke(px(width));
    f(&mut builder);
    if let Ok(path) = builder.build() {
        window.paint_path(path, colour);
    }
}

/// Short dashes along a screen-space line, the same size at any zoom.
fn paint_dotted(a: Point<Pixels>, b: Point<Pixels>, width: f32, colour: Hsla, window: &mut Window) {
    const ON: f32 = 3.0;
    const OFF: f32 = 3.0;
    let (dx, dy) = (f32::from(b.x - a.x), f32::from(b.y - a.y));
    let length = dx.hypot(dy);
    if length < 1.0 {
        return;
    }
    let (ux, uy) = (dx / length, dy / length);
    let at = |t: f32| point(a.x + px(ux * t), a.y + px(uy * t));
    stroke(window, width, colour, |p| {
        let mut t = 0.0;
        while t < length {
            p.move_to(at(t));
            p.line_to(at((t + ON).min(length)));
            t += ON + OFF;
        }
    });
}

fn fill_polygon(window: &mut Window, points: &[Point<Pixels>], colour: Hsla) {
    let mut builder = PathBuilder::fill();
    builder.add_polygon(points, true);
    if let Ok(path) = builder.build() {
        window.paint_path(path, colour);
    }
}

/// The grid, lined up on the crank axis (the model origin) in 1 cm cells.
/// The earth below `ground` is filled first, so the grid runs on over it.
fn paint_grid(m: &Mapper, bounds: Bounds<Pixels>, ground: Option<Pixels>, window: &mut Window) {
    window.paint_quad(fill(bounds, palette::view()));
    if let Some(y) = ground.filter(|y| *y < bounds.bottom()) {
        let top = y.max(bounds.origin.y);
        window.paint_quad(fill(
            Bounds::from_corners(point(bounds.origin.x, top), bounds.bottom_right()),
            palette::earth(),
        ));
    }
    let lo = m.unmap(point(bounds.origin.x, bounds.bottom()));
    let hi = m.unmap(point(bounds.right(), bounds.origin.y));
    let cell_px = GRID_MM as f32 * m.scale;
    let minor = cell_px >= MIN_GRID_PX;
    let lines = |from: f64, to: f64| {
        let first = (from / GRID_MM).floor() as i64;
        let last = (to / GRID_MM).ceil() as i64;
        first..=last
    };
    // Above the ground at full strength, below it dimmer (mockup 04).
    let split = ground.map_or(bounds.bottom(), |y| {
        y.clamp(bounds.origin.y, bounds.bottom())
    });
    let above = Bounds::from_corners(bounds.origin, point(bounds.right(), split));
    let below = Bounds::from_corners(point(bounds.origin.x, split), bounds.bottom_right());
    for (area, base) in [(above, palette::grid()), (below, palette::grid_below())] {
        if area.size.height <= px(0.0) {
            continue;
        }
        window.with_content_mask(Some(ContentMask { bounds: area }), |window| {
            for major in [false, true] {
                if !major && !minor {
                    continue;
                }
                let colour = if major { base } else { base.opacity(0.45) };
                stroke(window, 1.0, colour, |b| {
                    for i in lines(lo.x, hi.x) {
                        if (i % GRID_MAJOR == 0) == major {
                            let x = m.map(bike_model::Point::new(i as f64 * GRID_MM, 0.0)).x;
                            b.move_to(point(x, area.origin.y));
                            b.line_to(point(x, area.bottom()));
                        }
                    }
                    for i in lines(lo.y, hi.y) {
                        if (i % GRID_MAJOR == 0) == major {
                            let y = m.map(bike_model::Point::new(0.0, i as f64 * GRID_MM)).y;
                            b.move_to(point(area.origin.x, y));
                            b.line_to(point(area.right(), y));
                        }
                    }
                });
            }
        });
    }
}

/// The ground: a line with hatching below it, the full width of the view (mockup 04).
fn paint_ground(y: Pixels, bounds: Bounds<Pixels>, colour: Hsla, window: &mut Window) {
    let (left, right) = (bounds.origin.x, bounds.right());
    stroke(window, 1.0, colour, |b| {
        b.move_to(point(left, y));
        b.line_to(point(right, y));
    });
    stroke(window, HATCH_WIDTH_PX, palette::hatch(), |b| {
        let mut x = f32::from(left) - HATCH_DEPTH_PX;
        while x < f32::from(right) {
            b.move_to(point(px(x + HATCH_DEPTH_PX), y));
            b.line_to(point(px(x), y + px(HATCH_DEPTH_PX)));
            x += HATCH_STEP_PX;
        }
    });
}

fn circle_points(m: &Mapper, centre: bike_model::Point, radius: f64) -> Vec<Point<Pixels>> {
    let sides = 120;
    (0..sides)
        .map(|i| {
            let a = std::f64::consts::TAU * f64::from(i) / f64::from(sides);
            m.map(bike_model::Point::new(
                centre.x + radius * a.cos(),
                centre.y + radius * a.sin(),
            ))
        })
        .collect()
}

fn paint_view(
    scene: Option<&Scene>,
    bounds: Bounds<Pixels>,
    transform: ViewTransform,
    stale: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let m = Mapper::new(scene, bounds, transform);
    window.with_content_mask(Some(ContentMask { bounds }), |window| {
        let ground = scene.and_then(|s| {
            s.items
                .iter()
                .find_map(|item| match (&item.role, &item.shape) {
                    (Role::Ground, Shape::Line { a, .. }) => Some(m.map(*a).y),
                    _ => None,
                })
        });
        paint_grid(&m, bounds, ground, window);
        let Some(scene) = scene else { return };
        let dim = |c: Hsla| if stale { c.opacity(c.a * 0.35) } else { c };
        for item in &scene.items {
            let (colour, hairline, shade) = palette::role(item.role);
            let (colour, shade) = (dim(colour), shade.map(dim));
            match &item.shape {
                Shape::Line { a, b, width } => {
                    if item.role == Role::Ground {
                        paint_ground(m.map(*a).y, bounds, colour, window);
                        continue;
                    }
                    if item.role == Role::Extension {
                        paint_dotted(m.map(*a), m.map(*b), hairline, colour, window);
                        continue;
                    }
                    let w = width.map_or(hairline, |w| (w as f32 * m.scale).max(hairline));
                    stroke(window, w, colour, |p| {
                        p.move_to(m.map(*a));
                        p.line_to(m.map(*b));
                    });
                }
                Shape::Polygon(points) => {
                    let points: Vec<_> = points.iter().map(|p| m.map(*p)).collect();
                    if let Some(shade) = shade {
                        fill_polygon(window, &points, shade);
                    }
                    stroke(window, hairline, colour, |p| p.add_polygon(&points, true));
                }
                Shape::Circle { centre, radius } => {
                    let points = circle_points(&m, *centre, *radius);
                    if let Some(shade) = shade {
                        fill_polygon(window, &points, shade);
                    }
                    stroke(window, hairline, colour, |p| p.add_polygon(&points, true));
                }
                Shape::Ring {
                    centre,
                    outer,
                    inner,
                } => {
                    let mid = (outer + inner) / 2.0;
                    let band = ((outer - inner) as f32 * m.scale).max(1.0);
                    if let Some(shade) = shade {
                        let points = circle_points(&m, *centre, mid);
                        stroke(window, band, shade, |p| p.add_polygon(&points, true));
                    }
                    for r in [*outer, *inner] {
                        let points = circle_points(&m, *centre, r);
                        stroke(window, hairline, colour, |p| p.add_polygon(&points, true));
                    }
                }
                Shape::Label { at, text } => {
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
                        px(12.0),
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
