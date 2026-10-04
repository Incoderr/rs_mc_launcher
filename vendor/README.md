# GPUI inspector compatibility patch

`gpui-pre/` is the unchanged crates.io distribution of `gpui-pre` 0.3.7 except
for the inspector fixes described below. Its original licenses and source
metadata are retained. The workspace uses it through `[patch.crates-io]`.

Changes:

- `src/window.rs`: keep pointer events in the inspector panel available while
  picking application elements. Share the panel width calculation with layout.
- `src/elements/div.rs`: omit inspector-only hitboxes for transparent,
  noninteractive containers covering the application viewport. This prevents
  empty GPUI Component notification/tooltip hosts from masking real elements.

Regression coverage lives in `app/tests/inspector.rs` and exercises the public
GPUI mouse event and inspector renderer APIs.

These changes do not alter normal application hitboxes or input handling when
the inspector is closed. Remove the patch when the upstream GPUI version used
by GPUI Kit contains equivalent fixes.
