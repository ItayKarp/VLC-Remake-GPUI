//! A window showing every kit component, for design checks and screenshots
//! (S0.4 Definition of Done). Open it with `--component-gallery` (`app::cli::COMPONENT_GALLERY`).
//!
//! The theme and accent switches at the top change the real settings, so the
//! gallery shows exactly what the app will look like; point `RVLC_CONFIG_DIR` at
//! a scratch folder to keep your own settings untouched.

use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, MouseButton, Pixels, Point, SharedString,
    Subscription, Window, div, prelude::*, px,
};

use super::button::{Button, ButtonStyle, IconButton};
use super::collapsible::Collapsible;
use super::dropdown::{self, Dropdown};
use super::list::{self, Columns, VirtualList};
use super::menu::{ContextMenu, MenuItem};
use super::modal::{Dialog, DialogButton, ModalLayer};
use super::popover::{self, BoundsTracker, Side};
use super::segmented::{Choice, Segmented, StackedChoice};
use super::slider::{Slider, SliderModel};
use super::tabs::{Tab, Tabs};
use super::text_input::{TextInput, Validation};
use super::toast::{self, Level, Toast, Toasts};
use super::toggle::{Checkbox, Toggle};
use super::{focus, metrics};
use crate::assets::{icons, two_tone};
use crate::settings;
use crate::theme::{self, Accent, ThemeMode};

pub struct Gallery {
    focus_handle: FocusHandle,
    toggle_on: bool,
    toggle_off: bool,
    checkbox: bool,
    segment2: usize,
    segment3: usize,
    stacked: usize,
    tab: usize,
    section_open: bool,
    banner_visible: bool,
    dropdown: Entity<Dropdown>,
    long_dropdown: Entity<Dropdown>,
    saturation: Entity<Slider>,
    panel_saturation: Entity<Slider>,
    speed: Entity<Slider>,
    sepia: Entity<Slider>,
    delay: Entity<Slider>,
    eq: Vec<Entity<Slider>>,
    name: Entity<TextInput>,
    volume_field: Entity<TextInput>,
    grid: Entity<VirtualList>,
    menu: Option<(Point<Pixels>, Entity<ContextMenu>, Subscription)>,
    popover_open: bool,
    popover_trigger: BoundsTracker,
    modals: Entity<ModalLayer>,
    toasts: Entity<Toasts>,
    status: SharedString,
    _subscriptions: Vec<Subscription>,
}

impl Gallery {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        theme::observe(window, cx);
        let presets: Vec<SharedString> = [
            "Default",
            "Classical",
            "Club",
            "Dance",
            "Full Bass",
            "Full Bass and Treble",
            "Full Treble",
            "Headphones",
            "Large Hall",
            "Live",
            "Party",
            "Pop",
            "Reggae",
            "Rock",
            "Ska",
            "Soft",
            "Soft Rock",
            "Techno",
        ]
        .into_iter()
        .map(SharedString::from)
        .collect();
        let dropdown = cx.new(|cx| {
            Dropdown::new(
                "Stereo mode",
                ["Mono", "Stereo", "Reverse stereo", "Left", "Right", "Dolby Surround"]
                    .into_iter()
                    .map(SharedString::from)
                    .collect(),
                0,
                window,
                cx,
            )
        });
        let long_dropdown = cx.new(|cx| Dropdown::new("Equalizer preset", presets, 0, window, cx));
        let saturation = cx.new(|cx| {
            Slider::new("Saturation", SliderModel::new(0., 3., 0.01, 1.), window, cx)
                .format(|v| format!("{v:.2}"))
        });
        let panel_saturation = cx.new(|cx| {
            Slider::new("Saturation", SliderModel::new(0., 3., 0.01, 1.), window, cx)
                .format(|v| format!("{v:.2}"))
        });
        let speed = cx.new(|cx| {
            Slider::new("Playback speed", SliderModel::new(0.25, 4., 0.05, 1.), window, cx)
                .accent()
                .format(|v| format!("{v:.2}×"))
        });
        let sepia = cx.new(|cx| {
            Slider::new("Sepia intensity", SliderModel::new(0., 255., 1., 120.), window, cx)
                .accent()
                .format(|v| format!("{v:.0}"))
                .with_field(window, cx)
        });
        let delay = cx.new(|cx| {
            Slider::new("Audio track delay", SliderModel::new(-10., 10., 0.05, 0.), window, cx)
                .accent()
                .centered()
                .format(|v| format!("{v:.3} s"))
                .with_field(window, cx)
        });
        let eq = (0..10)
            .map(|band| {
                cx.new(|cx| {
                    let mut model = SliderModel::new(-20., 20., 0.5, 0.);
                    model.set([3., 5., 2., 0., -2., -3., 0., 2., 4., 6.][band]);
                    Slider::new(format!("Band {}", band + 1), model, window, cx)
                        .vertical()
                        .centered()
                        .accent()
                        .format(|v| format!("{v:+.1} dB"))
                })
            })
            .collect();
        let name = cx.new(|cx| {
            TextInput::new(window, cx).with_placeholder("Playlist name").with_label("Playlist name")
        });
        let volume_field =
            cx.new(|cx| TextInput::new(window, cx).with_text("200%").with_label("Starting volume"));
        let grid = cx.new(|cx| {
            VirtualList::new(
                "Media",
                10_000,
                Columns::Fit { width: 120., gap: 8. },
                96.,
                media_card,
                window,
                cx,
            )
        });
        let modals = cx.new(|_| ModalLayer::new());
        let toasts = cx.new(|_| Toasts::new());

        let mut subscriptions = vec![
            cx.subscribe(&dropdown, |this, _, event: &dropdown::Changed, cx| {
                this.status = format!("Dropdown → item {}", event.0).into();
                cx.notify();
            }),
            cx.subscribe(&delay, |this, _, event: &super::slider::Changed, cx| {
                this.status = format!("Delay → {:.3} s", event.0).into();
                cx.notify();
            }),
            cx.subscribe(&grid, |this, _, event: &list::Event, cx| {
                this.status = format!("Grid → {event:?}").into();
                cx.notify();
            }),
        ];
        subscriptions.push(cx.subscribe(&name, |this, input, event, cx| {
            if let super::text_input::Event::Changed = event {
                let text = input.read(cx).text().to_owned();
                let validation = if text.chars().any(|c| matches!(c, '/' | '\\' | ':')) {
                    Validation::Error("Names can't contain / \\ or :".into())
                } else {
                    Validation::None
                };
                input.update(cx, |input, cx| input.set_validation(validation, cx));
                this.status = format!("Name → {text:?}").into();
                cx.notify();
            }
        }));
        popover::close_on_deactivate(window, cx, |this: &mut Self, _, cx| {
            this.popover_open = false;
            cx.notify();
        });

        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        Self {
            focus_handle,
            toggle_on: true,
            toggle_off: false,
            checkbox: true,
            segment2: 0,
            segment3: 1,
            stacked: 1,
            tab: 2,
            section_open: true,
            banner_visible: true,
            dropdown,
            long_dropdown,
            saturation,
            panel_saturation,
            speed,
            sepia,
            delay,
            eq,
            name,
            volume_field,
            grid,
            menu: None,
            popover_open: false,
            popover_trigger: BoundsTracker::new(),
            modals,
            toasts,
            status: "Tab through the controls; arrow keys move inside groups.".into(),
            _subscriptions: subscriptions,
        }
    }

    fn open_menu(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let entity = cx.entity().downgrade();
        let say = move |text: &'static str| {
            let entity = entity.clone();
            move |_: &mut Window, cx: &mut App| {
                entity
                    .update(cx, |this, cx| {
                        this.status = text.into();
                        cx.notify();
                    })
                    .ok();
            }
        };
        let items = vec![
            MenuItem::action("Play", say("Menu → Play")).icon(icons::PLAY).shortcut("Space"),
            MenuItem::action("Add to Queue", say("Menu → Add to Queue")).icon(icons::LIST),
            MenuItem::action("Show in File Explorer", say("Menu → Show")).icon(icons::FOLDER),
            MenuItem::Separator,
            MenuItem::check("Mark as Watched", true, say("Menu → Watched")),
            MenuItem::submenu(
                "Sort By",
                vec![
                    MenuItem::radio("Name", true, say("Sort → Name")),
                    MenuItem::radio("Date Added", false, say("Sort → Date")),
                    MenuItem::radio("Duration", false, say("Sort → Duration")),
                    MenuItem::Separator,
                    MenuItem::submenu(
                        "Direction",
                        vec![
                            MenuItem::radio("Ascending", true, say("Sort → Asc")),
                            MenuItem::radio("Descending", false, say("Sort → Desc")),
                        ],
                    ),
                ],
            ),
            MenuItem::action("Convert", say("Menu → Convert")).disabled(true),
            MenuItem::action(
                "Properties & Metadata of a file with a very long name that truncates",
                say("Menu → Properties"),
            )
            .icon(icons::INFO),
        ];
        let menu = ContextMenu::build(items, window, cx);
        let subscription = cx.subscribe(&menu, |this, _, _: &gpui::DismissEvent, cx| {
            this.menu = None;
            cx.notify();
        });
        self.menu = Some((position, menu, subscription));
        cx.notify();
    }

    fn open_dialog(&mut self, nested: bool, window: &mut Window, cx: &mut Context<Self>) {
        let gallery = cx.entity().downgrade();
        let modals = self.modals.clone();
        let dialog = cx.new(|cx| {
            Dialog::new(
                if nested { "Are you sure?" } else { "New Playlist" },
                move |_, _| {
                    if nested {
                        div()
                            .child("This dialog is on top of another. Escape closes only this one.")
                            .into_any_element()
                    } else {
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(12.))
                            .child("Tab stays inside the dialog. Enter presses Create.")
                            .into_any_element()
                    }
                },
                if nested {
                    vec![DialogButton::new("Cancel"), DialogButton::new("Yes").primary()]
                } else {
                    vec![
                        DialogButton::new("Cancel"),
                        DialogButton::new("Ask again…").on_click({
                            let gallery = gallery.clone();
                            move |window, cx| {
                                gallery
                                    .update(cx, |this, cx| this.open_dialog(true, window, cx))
                                    .ok();
                            }
                        }),
                        DialogButton::new("Create").primary().on_click({
                            let gallery = gallery.clone();
                            move |_, cx| {
                                gallery
                                    .update(cx, |this, cx| {
                                        this.toasts.update(cx, |t, cx| {
                                            t.push(
                                                Toast::new(Level::Success, "Playlist created"),
                                                cx,
                                            );
                                        });
                                    })
                                    .ok();
                            }
                        }),
                    ]
                },
                cx,
            )
        });
        modals.update(cx, |layer, cx| layer.push(dialog, window, cx));
    }
}

fn media_card(
    index: usize,
    state: list::ItemState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let p = theme::palette(window, cx);
    div()
        .size_full()
        .flex()
        .flex_col()
        .gap(px(4.))
        .p(px(4.))
        .rounded(px(6.))
        .border_2()
        .border_color(if state.selected && state.focused {
            p.focus_ring
        } else if state.selected {
            p.selected
        } else {
            gpui::transparent_black()
        })
        .when(state.selected, |c| c.bg(p.selected))
        .hover(|s| s.bg(p.hover))
        .child(div().flex_1().rounded(px(4.)).bg(p.well))
        .child(
            div()
                .text_size(px(12.))
                .text_color(p.text)
                .truncate()
                .child(SharedString::from(format!("Video {:05}.mkv", index + 1))),
        )
        .into_any_element()
}

fn section(title: &'static str, p: &theme::Palette) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .p(px(16.))
        .rounded(px(10.))
        .bg(p.panel)
        .border_1()
        .border_color(p.divider)
        .child(div().text_size(px(12.)).text_color(p.text_muted).child(title.to_uppercase()))
}

fn row() -> gpui::Div {
    div().flex().flex_row().flex_wrap().items_center().gap(px(12.))
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let prefs = settings::get(cx).prefs.clone();
        let theme_index = ThemeMode::ALL.iter().position(|m| *m == prefs.theme).unwrap_or(0);
        let accent_index = Accent::ALL.iter().position(|a| *a == prefs.accent).unwrap_or(0);

        let appearance = section("Theme", &p).child(
            row()
                .child(
                    div().w(px(260.)).child(
                        Segmented::new(
                            "theme",
                            "Theme",
                            ThemeMode::ALL.map(ThemeMode::label),
                            theme_index,
                        )
                        .on_select(|i, _, cx| {
                            settings::update(cx, |s| s.prefs.theme = ThemeMode::ALL[*i]);
                        }),
                    ),
                )
                .child(
                    div().flex_1().min_w(px(420.)).child(
                        Segmented::new(
                            "accent",
                            "Accent",
                            Accent::ALL.map(Accent::key),
                            accent_index,
                        )
                        .on_select(|i, _, cx| {
                            settings::update(cx, |s| s.prefs.accent = Accent::ALL[*i]);
                        }),
                    ),
                ),
        );

        let buttons = section("Buttons", &p)
            .child(
                row()
                    .child(
                        Button::new("b-primary", "Create")
                            .primary()
                            .width(120.)
                            .tooltip("Create playlist", Some("Enter")),
                    )
                    .child(Button::new("b-subtle", "Show Advanced Settings").icon(icons::SETTINGS))
                    .child(
                        Button::new("b-filled", "Show Advanced Settings")
                            .icon(icons::SETTINGS)
                            .style(ButtonStyle::Filled),
                    )
                    .child(Button::new("b-ghost", "Ghost").style(ButtonStyle::Ghost))
                    .child(Button::new("b-disabled", "Disabled").disabled(true)),
            )
            .child(
                row()
                    .child(IconButton::new("i-play", icons::PLAY, "Play").shortcut("Space"))
                    .child(
                        IconButton::new("i-sub", icons::SUBTITLES, "Subtitles")
                            .shortcut("V")
                            .selected(true),
                    )
                    .child(IconButton::new("i-vol", icons::VOLUME, "Mute").shortcut("M"))
                    .child(IconButton::new("i-off", icons::SETTINGS, "Settings").disabled(true))
                    .child(super::icon::two_tone(two_tone::CHECK_BOX, 24., p.icon, p.accent_fg))
                    .child(super::icon::two_tone(two_tone::INFO, 24., p.icon, p.accent_fg)),
            );

        let toggles = section("Toggles, checkbox and values", &p)
            .child(
                row()
                    .child(Toggle::new("t-on", self.toggle_on, "Negate colors").on_change(
                        cx.listener(|this, on: &bool, _, cx| {
                            this.toggle_on = *on;
                            cx.notify();
                        }),
                    ))
                    .child(Toggle::new("t-off", self.toggle_off, "Posterize").on_change(
                        cx.listener(|this, on: &bool, _, cx| {
                            this.toggle_off = *on;
                            cx.notify();
                        }),
                    ))
                    .child(Toggle::new("t-dis", true, "Disabled").disabled(true))
                    .child(Checkbox::new("c", self.checkbox, "Remember last position").on_change(
                        cx.listener(|this, on: &bool, _, cx| {
                            this.checkbox = *on;
                            cx.notify();
                        }),
                    ))
                    .child(Checkbox::new("c-dis", false, "Unavailable").disabled(true)),
            )
            .child(
                row()
                    .child(
                        div().w(px(213.)).child(
                            Segmented::new("v2", "Unit", ["Relative", "Pixels"], self.segment2)
                                .on_select(cx.listener(|this, i: &usize, _, cx| {
                                    this.segment2 = *i;
                                    cx.notify();
                                })),
                        ),
                    )
                    .child(div().w(px(213.)).child(
                        Segmented::new("v3", "Value", ["1", "2", "3"], self.segment3).on_select(
                            cx.listener(|this, i: &usize, _, cx| {
                                this.segment3 = *i;
                                cx.notify();
                            }),
                        ),
                    ))
                    .child(
                        div().w(px(320.)).child(
                            StackedChoice::new(
                                "stacked",
                                "Starting volume",
                                vec![
                                    Choice::new("Remember last volume"),
                                    Choice::new("Always starts at:")
                                        .field(self.volume_field.clone()),
                                ],
                                self.stacked,
                            )
                            .on_select(cx.listener(
                                |this, i: &usize, _, cx| {
                                    this.stacked = *i;
                                    cx.notify();
                                },
                            )),
                        ),
                    ),
            );

        let pickers = section("Dropdowns, text input, tabs", &p)
            .child(
                row()
                    .child(div().w(px(300.)).child(self.dropdown.clone()))
                    .child(div().w(px(300.)).child(self.long_dropdown.clone())),
            )
            .child(row().child(div().w(px(300.)).child(self.name.clone())))
            .child(
                Tabs::new(
                    "tabs",
                    vec![
                        Tab::new(Some(icons::PLAY), "Playback"),
                        Tab::new(Some(icons::FILE), "Video"),
                        Tab::new(Some(icons::AUDIO), "Audio"),
                        Tab::new(Some(icons::SUBTITLES), "Subtitles"),
                    ],
                    self.tab,
                )
                .on_select(cx.listener(|this, i: &usize, _, cx| {
                    this.tab = *i;
                    cx.notify();
                })),
            );

        let sliders = section("Sliders", &p)
            .child(div().text_color(p.text_muted).child("Saturation"))
            .child(self.saturation.clone())
            .child(div().text_color(p.text_muted).child("Sepia Intensity"))
            .child(self.sepia.clone())
            .child(div().text_color(p.text_muted).child("Audio Track Delay"))
            .child(self.delay.clone())
            .child(
                div()
                    .h(px(180.))
                    .flex()
                    .flex_row()
                    .justify_between()
                    .children(self.eq.iter().cloned()),
            );

        let panel = div()
            .w(px(360.))
            .rounded(px(10.))
            .bg(p.popover)
            .border_1()
            .border_color(p.divider)
            .child(
                Collapsible::new("section", "Collapse Settings", self.section_open)
                    .on_toggle(cx.listener(|this, open: &bool, _, cx| {
                        this.section_open = *open;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child("Negate Colors")
                            .child(Toggle::new("t-panel", self.toggle_off, "Negate colors")),
                    )
                    .child(div().child("Saturation"))
                    .child(div().h(px(28.)).child(self.panel_saturation.clone())),
            )
            .child(Collapsible::new("section-2", "Sharpen", false));

        let overlays = section("Menus, popovers, dialogs, toasts", &p)
            .child(
                row()
                    .child(
                        div()
                            .id("menu-target")
                            .px(px(16.))
                            .py(px(10.))
                            .rounded(px(6.))
                            .border_1()
                            .border_dashed()
                            .border_color(p.key_border)
                            .child("Right-click for a context menu")
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                                    this.open_menu(event.position, window, cx);
                                }),
                            ),
                    )
                    .child(
                        div()
                            .relative()
                            .child(self.popover_trigger.element())
                            .child(
                                Button::new("popover", "Popover")
                                    .style(ButtonStyle::Filled)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.popover_open = !this.popover_open;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        Button::new("dialog", "Open dialog…")
                            .style(ButtonStyle::Filled)
                            .on_click(cx.listener(|this, _, window, cx| this.open_dialog(false, window, cx))),
                    )
                    .child(Button::new("toast-info", "Toast").style(ButtonStyle::Filled).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.toasts.update(cx, |t, cx| {
                                t.push(Toast::new(Level::Info, "Snapshot saved to Pictures"), cx);
                            });
                        }),
                    ))
                    .child(
                        Button::new("toast-error", "Error toast")
                            .style(ButtonStyle::Filled)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toasts.update(cx, |t, cx| {
                                    t.push(
                                        Toast::new(Level::Error, "Couldn't download subtitles: the server didn't respond.")
                                            .action("Retry", |_, _| {}),
                                        cx,
                                    );
                                });
                            })),
                    ),
            )
            .when(self.banner_visible, |s| {
                s.child(
                    toast::banner(
                        "banner",
                        Level::Error,
                        "C:\\Users\\someone\\AppData\\Roaming\\VLC\\settings.toml couldn't be saved: access denied.",
                    )
                    .action("Retry", |_, _| {})
                    .on_dismiss({
                        let this = cx.entity().downgrade();
                        move |_, cx| {
                            this.update(cx, |this, cx| {
                                this.banner_visible = false;
                                cx.notify();
                            })
                            .ok();
                        }
                    }),
                )
            });

        let grid = section("Virtualized grid (10,000 items)", &p)
            .child(div().h(px(260.)).child(self.grid.clone()));

        let popover =
            self.popover_open.then(|| self.popover_trigger.bounds()).flatten().map(|bounds| {
                popover::attached(
                    bounds,
                    // Like the control bar's popovers: above, flipping below near the top.
                    Side::Above,
                    6.,
                    1,
                    popover::surface(&p, window)
                        .id("popover-surface")
                        .w(px(280.))
                        .p(px(12.))
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .on_mouse_down_out(cx.listener(
                            |this, event: &gpui::MouseDownEvent, _, cx| {
                                if !this.popover_trigger.contains(&event.position) {
                                    this.popover_open = false;
                                    cx.notify();
                                }
                            },
                        ))
                        .child(div().text_color(p.text_strong).child("Playback speed"))
                        .child(self.speed.clone())
                        .child(
                            div().text_size(px(12.)).text_color(p.text_muted).child(
                                "Opens above its button, and below it when there's no room.",
                            ),
                        ),
                )
            });

        let menu = self
            .menu
            .as_ref()
            .map(|(position, menu, _)| popover::at_position(*position, 1, menu.clone()));

        let root = div()
            .id("gallery")
            .track_focus(&self.focus_handle)
            .relative()
            .size_full()
            .bg(p.background)
            .text_color(p.text)
            .text_size(px(metrics::TEXT - 2.))
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" && this.popover_open {
                    this.popover_open = false;
                    cx.notify();
                }
            }))
            .child(
                div()
                    .id("gallery-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .p(px(20.))
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(22.))
                                    .text_color(p.text_strong)
                                    .child("Component kit"),
                            )
                            .child(div().text_color(p.text_muted).child(self.status.clone())),
                    )
                    .child(appearance)
                    .child(buttons)
                    .child(toggles)
                    .child(pickers)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(16.))
                            .items_start()
                            .child(div().flex_1().child(sliders))
                            .child(panel),
                    )
                    .child(overlays)
                    .child(grid),
            )
            .children(popover)
            .children(menu)
            .child(self.toasts.clone())
            .child(self.modals.clone());
        focus::tab_navigation(root)
    }
}
