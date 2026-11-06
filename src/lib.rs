//! Braid — TUN-first proxy client (P3b: privileged helper + DataPlane).

mod connection;
pub mod helper_proto;
mod probe;
mod profile;
mod runtime;
mod singbox;

use connection::{Phase, Snapshot};
use nsh_base::theme;
use nsh_core::prelude::*;
use runtime::{use_mock_plane, DataPlane, MockPlane, SingBoxPlane};
use std::cell::RefCell;
use std::rc::Rc;

/// What a future TSX entry may import from `braid/native`.
#[native_module("braid/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>();
}

fn make_plane() -> Rc<RefCell<Box<dyn DataPlane>>> {
    if use_mock_plane() {
        Rc::new(RefCell::new(Box::new(MockPlane::default())))
    } else {
        Rc::new(RefCell::new(Box::new(SingBoxPlane::new())))
    }
}

/// Compact connection panel (menu-bar popover shape; real status-item comes later).
#[component]
pub fn App() -> impl IntoElement {
    let plane = make_plane();
    let snap = signal({
        let mut s = Snapshot::default();
        let mock = use_mock_plane();
        s.set_mock_plane(mock);
        if !mock {
            s.set_helper_online(SingBoxPlane::helper_reachable());
        }
        s
    });

    view()
        .flex_1()
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        .child(
            view()
                .flex_1()
                .p_5()
                .gap_3()
                .child(header(&snap))
                .child(summary(&snap))
                .child(config_status(&snap))
                .child(policy_row())
                .child(primary_button(&snap, &plane))
                .child(load_saved_button(&snap))
                .child(load_demo_button(&snap))
                .child(secondary_row(&snap, &plane))
                .child(footer(&snap)),
        )
}

fn header(snap: &Signal<Snapshot>) -> impl IntoElement {
    let snap = snap.clone();
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .child(status_dot(&snap))
        .child(
            view()
                .flex_col()
                .gap_1()
                .child(
                    text(move || snap.get().status_title().to_owned())
                        .text_size(px(20.0))
                        .font_weight(FontWeight::SEMIBOLD),
                )
                .child(
                    text("Braid")
                        .text_size(px(13.0))
                        .text_color(move || theme().muted_foreground),
                ),
        )
}

fn status_dot(snap: &Signal<Snapshot>) -> impl IntoElement {
    let snap = snap.clone();
    view()
        .w(px(12.0))
        .h(px(12.0))
        .rounded_full()
        .bg(move || match snap.get().phase {
            Phase::Connected => theme().primary,
            Phase::Connecting | Phase::Disconnecting => theme().muted_foreground,
            Phase::Failed => theme().destructive,
            Phase::Idle => theme().border,
        })
}

fn summary(snap: &Signal<Snapshot>) -> impl IntoElement {
    let snap = snap.clone();
    view()
        .w_full()
        .px_3()
        .py_2()
        .rounded(px(8.0))
        .bg(move || theme().muted)
        .child(
            text(move || snap.get().summary_line())
                .text_size(px(13.0))
                .text_color(move || theme().muted_foreground),
        )
}

fn config_status(snap: &Signal<Snapshot>) -> impl IntoElement {
    let snap = snap.clone();
    text(move || snap.get().config_line())
        .text_size(px(12.0))
        .text_color(move || theme().muted_foreground)
}

fn policy_row() -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .justify_between()
        .child(text("Link policy").text_size(px(13.0)))
        .child(
            text("Auto")
                .text_size(px(13.0))
                .text_color(move || theme().muted_foreground),
        )
}

fn primary_button(
    snap: &Signal<Snapshot>,
    plane: &Rc<RefCell<Box<dyn DataPlane>>>,
) -> impl IntoElement {
    let snap = snap.clone();
    let snap_press = snap.clone();
    let plane = Rc::clone(plane);
    view()
        .w_full()
        .items_center()
        .justify_center()
        .px_5()
        .py_3()
        .rounded(px(10.0))
        .bg(move || {
            if snap.get().primary_enabled() {
                theme().primary
            } else {
                theme().muted
            }
        })
        .active(|s| s.opacity(0.7))
        .on_press(move || {
            if snap_press.get().primary_enabled() {
                let mut plane = plane.borrow_mut();
                snap_press.update(|s| s.toggle_primary(plane.as_mut()));
            }
        })
        .child(
            text(move || snap.get().primary_label().to_owned())
                .text_size(px(16.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || theme().primary_foreground),
        )
}

fn load_saved_button(snap: &Signal<Snapshot>) -> impl IntoElement {
    let snap = snap.clone();
    view()
        .w_full()
        .items_center()
        .justify_center()
        .px_4()
        .py_2()
        .rounded(px(8.0))
        .border_1()
        .border_color(move || theme().border)
        .active(|s| s.opacity(0.7))
        .on_press(move || snap.update(|s| s.load_saved_profile()))
        .child(
            text("Load saved profile")
                .text_size(px(13.0))
                .text_color(move || theme().foreground),
        )
}

fn load_demo_button(snap: &Signal<Snapshot>) -> impl IntoElement {
    let snap = snap.clone();
    view()
        .w_full()
        .items_center()
        .justify_center()
        .px_4()
        .py_2()
        .rounded(px(8.0))
        .border_1()
        .border_color(move || theme().border)
        .active(|s| s.opacity(0.7))
        .on_press(move || snap.update(|s| s.load_demo_profile()))
        .child(
            text("Load demo profile")
                .text_size(px(13.0))
                .text_color(move || theme().foreground),
        )
}

fn secondary_row(
    snap: &Signal<Snapshot>,
    plane: &Rc<RefCell<Box<dyn DataPlane>>>,
) -> impl IntoElement {
    let snap_refresh = snap.clone();
    let snap_btn = snap.clone();
    let snap_banner = snap.clone();
    let plane_refresh = Rc::clone(plane);
    let plane = Rc::clone(plane);
    view()
        .flex_col()
        .gap_2()
        .child(
            view()
                .w_full()
                .items_center()
                .justify_center()
                .px_4()
                .py_2()
                .rounded(px(8.0))
                .border_1()
                .border_color(move || theme().border)
                .active(|s| s.opacity(0.7))
                .on_press(move || {
                    let mut plane = plane_refresh.borrow_mut();
                    snap_refresh.update(|s| s.tick(plane.as_mut()));
                })
                .child(
                    text("Refresh status")
                        .text_size(px(13.0))
                        .text_color(move || theme().foreground),
                ),
        )
        .child(
            view()
                .w_full()
                .items_center()
                .justify_center()
                .px_4()
                .py_2()
                .rounded(px(8.0))
                .border_1()
                .border_color(move || theme().border)
                .active(|s| s.opacity(0.7))
                .on_press(move || {
                    let mut plane = plane.borrow_mut();
                    let _ = plane.stop();
                    snap_btn.update(|s| s.simulate_path_failure());
                })
                .child(
                    text("Simulate path failure")
                        .text_size(px(13.0))
                        .text_color(move || theme().foreground),
                ),
        )
        .child(fail_banner(&snap_banner))
}

fn fail_banner(snap: &Signal<Snapshot>) -> impl IntoElement {
    let snap = snap.clone();
    view().child(
        text(move || snap.get().fail_copy().unwrap_or_default())
            .text_size(px(12.0))
            .text_color(move || theme().destructive),
    )
}

fn footer(snap: &Signal<Snapshot>) -> impl IntoElement {
    let snap = snap.clone();
    text(move || snap.get().footer_line())
        .text_size(px(11.0))
        .text_color(move || theme().muted_foreground)
}
