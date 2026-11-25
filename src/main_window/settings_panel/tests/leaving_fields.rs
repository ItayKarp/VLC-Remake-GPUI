//! The Video tab's typed fields keep what was typed when the keyboard leaves them without Enter,
//! as clicking or tabbing elsewhere does: the panel redraws before the field hears it lost the
//! focus, and that redraw mustn't put the value in effect back over the typed one.

use gpui::{Entity, Focusable as _, TestAppContext, VisualTestContext};

use crate::app::media::Media;
use crate::effects::{self, VIDEO_COLORS, VIDEO_CROP};
use crate::engine::backend::Backend;
use crate::engine::mock::{MockBackend, MockMedia};
use crate::engine::transform::Rotation;
use crate::main_window::settings_panel::PanelTab;
use crate::main_window::tests::{main_view, open};
use crate::settings;
use crate::settings::registry::Value;
use crate::ui::text_input::TextInput;
use crate::video::Output;

/// The window playing the mock's clip, the Video tab open with Crop and Colors expanded and
/// Aspect Ratio on Custom, so every field shows.
fn open_video_tab(cx: &mut TestAppContext) -> &mut VisualTestContext {
    let cx = open(cx);
    cx.update(|_, cx| {
        let ids = [VIDEO_CROP.id, VIDEO_COLORS.id].map(str::to_owned).to_vec();
        settings::update(cx, |s| s.local.expanded_effects = ids);
        effects::set(cx, "video.aspect_ratio", Value::Choice("custom"));
    });
    main_view(cx).update_in(cx, |main, window, cx| {
        let make = |events| {
            let (backend, _) = MockBackend::new(events, MockMedia::default());
            Box::new(backend) as Box<dyn Backend>
        };
        main.attach_player(Output::new(), make, window, cx);
        main.queue.replace(vec![Media::Url("https://e.com/clip".into())]);
        main.play(0, window, cx);
        main.open_settings_panel(PanelTab::Video, window, cx);
    });
    draw(cx);
    cx
}

/// Paints a frame, as the window would, after letting every task settle.
fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

/// Types `text` over what `field` holds, in the active window, then moves the keyboard to the
/// player without pressing Enter.
fn type_and_leave(cx: &mut VisualTestContext, field: &Entity<TextInput>, text: &str) {
    cx.update(|window, cx| {
        window.activate_window();
        field.read(cx).focus_handle(cx).focus(window, cx);
        field.update(cx, TextInput::select_everything);
    });
    draw(cx);
    cx.simulate_input(text);
    draw(cx);
    let main = main_view(cx);
    cx.update(|window, cx| main.read(cx).focus_handle.clone().focus(window, cx));
    draw(cx);
}

fn text(cx: &mut VisualTestContext, field: &Entity<TextInput>) -> String {
    field.read_with(cx, |field, _| field.text().to_owned())
}

#[gpui::test]
fn leaving_rotate_takes_the_typed_angle(cx: &mut TestAppContext) {
    let cx = open_video_tab(cx);
    let field = main_view(cx)
        .read_with(cx, |main, _| main.settings_panel.video_defaults.rotate.field().clone());
    type_and_leave(cx, &field, "90");
    let rotation = cx.update(|_, cx| effects::effective(cx).prefs.video.rotation);
    assert_eq!(rotation, Rotation::Quarter);
    assert_eq!(text(cx, &field), "90 deg");
}

#[gpui::test]
fn leaving_custom_ratio_takes_the_typed_ratio(cx: &mut TestAppContext) {
    let cx = open_video_tab(cx);
    let field = main_view(cx)
        .read_with(cx, |main, _| main.settings_panel.video_defaults.custom_ratio.field());
    type_and_leave(cx, &field, "21:9");
    let ratio = cx.update(|_, cx| effects::effective(cx).prefs.video.custom_aspect_ratio);
    assert_eq!(ratio, "21:9");
    assert_eq!(text(cx, &field), "21:9");
}

#[gpui::test]
fn leaving_a_crop_edge_takes_the_typed_pixels(cx: &mut TestAppContext) {
    let cx = open_video_tab(cx);
    let field =
        main_view(cx).read_with(cx, |main, _| main.settings_panel.crop.fields[0].field().clone());
    type_and_leave(cx, &field, "12 px");
    let top = cx.update(|_, cx| effects::effective(cx).prefs.video.crop_edges.top);
    assert_eq!(top, 12);
    assert_eq!(text(cx, &field), "12");
}

#[gpui::test]
fn leaving_a_color_field_takes_the_typed_color(cx: &mut TestAppContext) {
    let cx = open_video_tab(cx);
    let field =
        main_view(cx).read_with(cx, |main, _| main.settings_panel.colors.colors[0].field().clone());
    type_and_leave(cx, &field, "#123456");
    let color = cx.update(|_, cx| effects::value(cx, "video.colors.extract_color"));
    assert_eq!(color, Some(Value::Text("#123456".into())));
    assert_eq!(text(cx, &field), "#123456");
}
