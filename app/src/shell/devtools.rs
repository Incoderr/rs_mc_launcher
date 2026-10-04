use gpui_kit::*;

pub fn initialize(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("f12", gpui_devtools::ToggleInspector, None),
        KeyBinding::new("ctrl-alt-i", gpui_devtools::ToggleInspector, None),
        KeyBinding::new("cmd-alt-i", gpui_devtools::ToggleInspector, None),
    ]);
}
