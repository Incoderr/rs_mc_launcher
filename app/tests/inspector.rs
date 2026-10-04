#![cfg(feature = "devtools")]

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use gpui_kit::{
    App, AppContext as _, Context, Empty, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, MouseUpEvent, ParentElement as _, Pixels, PlatformInput, Point, Render,
    StatefulInteractiveElement as _, Styled as _, TestAppContext, Window, div, point, px, rgb,
};

struct OverlayRoot;

impl Render for OverlayRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(div().id("picker-target").size(px(100.)).bg(rgb(0x333333)))
            .child(div().absolute().inset_0())
    }
}

#[gpui_kit::test]
fn picker_selects_content_under_empty_window_overlay(cx: &mut TestAppContext) {
    let selected = Rc::new(RefCell::new(None::<String>));
    let selection = selected.clone();
    cx.update(|cx| {
        cx.set_inspector_renderer(Box::new(move |inspector, _, _| {
            *selection.borrow_mut() = inspector
                .active_element_id()
                .map(|id| id.path.global_id.to_string());
            Empty.into_any_element()
        }));
    });

    let window = cx.add_window(|_, _| OverlayRoot);
    cx.update_window(window.into(), |_, window, cx| {
        window.toggle_inspector(cx);
        window.draw(cx).clear(cx);
        window.draw(cx).clear(cx);
        let position = point(px(20.), px(20.));
        window.simulate_mouse_move(position, cx);
        click(window, position, cx);
        window.draw(cx).clear(cx);

        assert!(!window.is_inspector_picking(cx));
        assert!(
            selected
                .borrow()
                .as_ref()
                .is_some_and(|id| id.contains("picker-target"))
        );
    })
    .expect("inspect content");
}

#[gpui_kit::test]
fn close_button_receives_first_click_while_picker_is_active(cx: &mut TestAppContext) {
    let closed = Rc::new(Cell::new(false));
    let close_state = closed.clone();
    cx.update(|cx| {
        cx.set_inspector_renderer(Box::new(move |_, _, cx| {
            let close_state = close_state.clone();
            div()
                .size_full()
                .child(
                    div()
                        .id("close-inspector")
                        .size(px(80.))
                        .on_click(cx.listener(move |_, _, window, cx| {
                            close_state.set(true);
                            window.toggle_inspector(cx);
                        })),
                )
                .into_any_element()
        }));
    });

    let window = cx.add_window(|_, _| OverlayRoot);
    cx.update_window(window.into(), |_, window, cx| {
        window.toggle_inspector(cx);
        window.draw(cx).clear(cx);
        window.draw(cx).clear(cx);
        assert!(window.is_inspector_picking(cx));

        let panel_width = gpui_kit::rems(30.)
            .to_pixels(window.rem_size())
            .min(window.viewport_size().width);
        let position = point(
            window.viewport_size().width - panel_width + px(20.),
            px(20.),
        );
        window.simulate_mouse_move(position, cx);
        click(window, position, cx);

        assert!(closed.get(), "the close handler runs on the first click");
        assert!(!window.is_inspector_picking(cx));
    })
    .expect("close inspector from panel");
}

fn click(window: &mut Window, position: Point<Pixels>, cx: &mut App) {
    window.dispatch_event(
        PlatformInput::MouseDown(MouseDownEvent {
            position,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Default::default(),
            first_mouse: false,
        }),
        cx,
    );
    window.dispatch_event(
        PlatformInput::MouseUp(MouseUpEvent {
            position,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Default::default(),
        }),
        cx,
    );
}
