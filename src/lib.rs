//! Braid — TUN-first proxy client (main panel: connect-first UX).

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

/// Compact connection panel — brand + Connect first; tools under More.
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
                .px_5()
                .pt_6()
                .pb_4()
                .gap_4()
                .child(brand_header(&snap))
                .child(summary(&snap))
                .child(primary_button(&snap, &plane))
                .child(policy_row())
                .child(fail_banner(&snap))
                .child(footer(&snap))
                .child(more_section(&snap, &plane)),
        )
}

fn brand_header(snap: &Signal<Snapshot>) -> impl IntoElement {
    let snap = snap.clone();
    view()
        .flex_col()
        .gap_2()
        .child(
            text("Braid")
                .text_size(px(28.0))
                .font_weight(FontWeight::SEMIBOLD),
        )
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(status_dot(&snap))
                .child(
                    text(move || snap.get().status_title().to_owned())
                        .text_size(px(15.0))
                        .text_color(move || theme().muted_foreground),
                ),
        )
}

fn status_dot(snap: &Signal<Snapshot>) -> impl IntoElement {
    let snap = snap.clone();
    view()
        .w(move || {
            if snap.get().phase == Phase::Connected {
                px(14.0)
            } else {
                px(10.0)
            }
        })
        .h(move || {
            if snap.get().phase == Phase::Connected {
                px(14.0)
            } else {
                px(10.0)
            }
        })
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

fn policy_row() -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            text("Link policy")
                .text_size(px(13.0))
                .text_color(move || theme().muted_foreground),
        )
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
        .py_5()
        .rounded(px(14.0))
        .bg(move || {
            if snap.get().primary_enabled() {
                theme().primary
            } else {
                theme().muted
            }
        })
        .opacity(move || {
            if snap.get().phase == Phase::Connecting || snap.get().phase == Phase::Disconnecting {
                0.85_f32
            } else {
                1.0_f32
            }
        })
        .active(|s| s.opacity(0.72))
        .on_press(move || {
            if snap_press.get().primary_enabled() {
                let mut plane = plane.borrow_mut();
                snap_press.update(|s| s.toggle_primary(plane.as_mut()));
            }
        })
        .child(
            text(move || snap.get().primary_label().to_owned())
                .text_size(px(18.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(move || {
                    if snap.get().primary_enabled() {
                        theme().primary_foreground
                    } else {
                        theme().muted_foreground
                    }
                }),
        )
}

fn more_section(
    snap: &Signal<Snapshot>,
    plane: &Rc<RefCell<Box<dyn DataPlane>>>,
) -> impl IntoElement {
    let snap = snap.clone();
    let plane = Rc::clone(plane);
    let mock = use_mock_plane();
    view()
        .flex_col()
        .gap_2()
        .w_full()
        .pt_2()
        .child(
            text("More")
                .text_size(px(11.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || theme().muted_foreground),
        )
        .child(config_status(&snap))
        .child(quiet_button("Saved profile", {
            let snap = snap.clone();
            move || snap.update(|s| s.load_saved_profile())
        }))
        .child(quiet_button("Demo profile", {
            let snap = snap.clone();
            move || snap.update(|s| s.load_demo_profile())
        }))
        .child(refresh_button(&snap, &plane))
        .child(diag_controls(&snap))
        .when(mock, |row| {
            row.child(quiet_button("Simulate path failure", {
                let snap = snap.clone();
                let plane = Rc::clone(&plane);
                move || {
                    let mut plane = plane.borrow_mut();
                    let _ = plane.stop();
                    snap.update(|s| s.simulate_path_failure());
                }
            }))
        })
}

fn config_status(snap: &Signal<Snapshot>) -> impl IntoElement {
    let snap = snap.clone();
    text(move || snap.get().config_line())
        .text_size(px(11.0))
        .text_color(move || theme().muted_foreground)
}

fn quiet_button(label: &'static str, on_press: impl Fn() + 'static) -> impl IntoElement {
    view()
        .w_full()
        .items_center()
        .justify_center()
        .px_3()
        .py_2()
        .rounded(px(8.0))
        .border_1()
        .border_color(move || theme().border)
        .active(|s| s.opacity(0.7))
        .on_press(on_press)
        .child(
            text(label)
                .text_size(px(12.0))
                .text_color(move || theme().foreground),
        )
}

fn refresh_button(
    snap: &Signal<Snapshot>,
    plane: &Rc<RefCell<Box<dyn DataPlane>>>,
) -> impl IntoElement {
    let snap = snap.clone();
    let plane = Rc::clone(plane);
    quiet_button("Refresh", move || {
        let mut plane = plane.borrow_mut();
        snap.update(|s| s.tick(plane.as_mut()));
    })
}

fn diag_controls(snap: &Signal<Snapshot>) -> impl IntoElement {
    let snap_btn = snap.clone();
    let snap_line = snap.clone();
    view()
        .flex_col()
        .gap_1()
        .child(quiet_button("Diagnose", move || {
            snap_btn.update(|s| s.run_diagnostics())
        }))
        .child(
            text(move || snap_line.get().diag_copy())
                .text_size(px(11.0))
                .text_color(move || theme().muted_foreground),
        )
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
