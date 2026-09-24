//! Sticky-note widget: a macOS-Stickies-like post-it floating IN FRONT of
//! the network. Pure annotation — dragging/resizing/typing only ever touches
//! `Document.notes`, never the model. Inline editing replaces the body with
//! a transparent `ui::TextArea` (keeps the TYPING hotkey guard intact).

use bn_session::NoteId;
use dioxus::html::input_data::keyboard_types::{Key, Modifiers};
use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use icons::{SquareArrowDown, SquareArrowDownLeft, X};

use crate::canvas::controller::{self, Gesture, DRAG_HAPPENED, GESTURE};
use crate::canvas::geometry::Rect;
use crate::canvas::note_style::{note_bar_color, note_fill, note_html, note_plain_text};
use crate::state::{
    exec, ContextMenuState, ContextMenuTarget, CONTEXT_MENU, EDITING_NOTE, SELECTION, SESSION,
};
use crate::ui::TextArea;

#[component]
pub fn StickyNote(id: NoteId, rect: Rect) -> Element {
    // One scoped read; bail if the note vanished mid-render (undo/delete).
    let Some((text, color, font_size, collapsed)) = ({
        let s = SESSION.read();
        s.doc().notes.get(id).map(|n| (n.text.clone(), n.color, n.font_size, n.collapsed))
    }) else {
        return rsx! {};
    };

    let selected = SELECTION.read().notes.contains(&id);
    let editing = *EDITING_NOTE.read() == Some(id) && !collapsed;

    let fill = note_fill(color);
    let bar = note_bar_color(color);
    let border =
        if selected { "2px solid rgb(100,140,220)" } else { "1px solid rgba(0,0,0,0.15)" };

    // Collapsed: the whole note IS the bar, showing the first text line
    // (markdown markers stripped: a collapsed `# Title` reads "Title").
    let first_line = note_plain_text(&text).lines().next().unwrap_or("").to_string();
    let bar_class = if collapsed {
        "flex min-h-0 h-full items-center rounded-sm px-0.5"
    } else {
        "flex h-[14px] shrink-0 items-center rounded-t-sm px-0.5"
    };
    let toggle_title = if collapsed { "Expand" } else { "Collapse" };
    let cursor = if editing { "default" } else { "grab" };
    // pointer-events gating: while invisible (not hovered) the buttons must
    // not steal mousedowns from the drag bar.
    const BAR_BTN: &str = "pointer-events-none flex size-3 shrink-0 cursor-default items-center \
                           justify-center rounded-sm text-black/60 opacity-0 \
                           group-hover:pointer-events-auto group-hover:opacity-100 \
                           hover:bg-black/15";
    const BAR_ICON: &str = "size-2.5";

    // Select this note (exclusively, unless it already is) and arm a drag.
    // Shared by the wrapper (when not editing) and the bar (always — macOS
    // Stickies stay draggable by the title bar while focused).
    let arm_drag = move |ev: &MouseEvent| {
        *DRAG_HAPPENED.write() = false;
        if !SELECTION.read().notes.contains(&id) {
            let mut sel = SELECTION.write();
            sel.nodes.clear();
            sel.edges.clear();
            sel.notes.clear();
            sel.notes.insert(id);
        }
        let c = ev.data().client_coordinates();
        let (starts, note_starts) = controller::drag_starts(None, Some(id));
        *GESTURE.write() =
            Gesture::PendingNodeDrag { start_client: (c.x, c.y), starts, note_starts };
    };

    // True when the mousedown that armed the current gesture hit an
    // already-selected note — a still click then opens the editor on mouseup
    // (forgiving alternative to a strictly timed double-click).
    let mut was_selected = use_signal(|| false);

    rsx! {
        div {
            // z-index 20: the node link handles use z-10 and would otherwise
            // poke through (nodes create no stacking context of their own).
            class: "group absolute flex flex-col rounded-sm shadow-md",
            style: "left: {rect.x:.1}px; top: {rect.y:.1}px; width: {rect.w:.0}px; \
                    height: {rect.h:.0}px; background: {fill}; border: {border}; \
                    z-index: 20; cursor: {cursor};",

            onmousedown: move |ev| {
                if ev.data().trigger_button() != Some(MouseButton::Primary) {
                    return; // right/middle: pan or context menu, handled by the canvas
                }
                ev.stop_propagation();
                was_selected.set(false);
                if editing || controller::is_connecting() {
                    return;
                }
                let shift = ev.data().modifiers().contains(Modifiers::SHIFT);
                if shift {
                    *DRAG_HAPPENED.write() = false;
                    let mut sel = SELECTION.write();
                    if !sel.notes.remove(&id) {
                        sel.notes.insert(id);
                    }
                    drop(sel);
                    let c = ev.data().client_coordinates();
                    let (starts, note_starts) = controller::drag_starts(None, Some(id));
                    *GESTURE.write() = Gesture::PendingNodeDrag {
                        start_client: (c.x, c.y),
                        starts,
                        note_starts,
                    };
                } else {
                    was_selected.set(SELECTION.read().notes.contains(&id));
                    arm_drag(&ev);
                }
            },

            ondoubleclick: move |ev| {
                ev.stop_propagation();
                if collapsed {
                    exec(|s| bn_session::ops::edit::set_note_collapsed(s, id, false));
                } else {
                    *EDITING_NOTE.write() = Some(id);
                }
            },

            oncontextmenu: move |ev| {
                ev.prevent_default();
                ev.stop_propagation();
                let c = ev.data().client_coordinates();
                *CONTEXT_MENU.write() = Some(ContextMenuState {
                    target: ContextMenuTarget::Note(id),
                    client: (c.x, c.y),
                });
            },

            // ── top drag bar with close / collapse controls (macOS
            //    Stickies: hover-revealed, double-click toggles collapse) ──
            div {
                class: bar_class,
                style: "background: {bar}; cursor: grab;",
                // The bar drags the note even while editing (the wrapper
                // ignores mousedown then); the textarea blur commits first.
                onmousedown: move |ev| {
                    if !editing || ev.data().trigger_button() != Some(MouseButton::Primary) {
                        return; // not editing → the wrapper handles it
                    }
                    ev.stop_propagation();
                    if !controller::is_connecting() {
                        arm_drag(&ev);
                    }
                },
                ondoubleclick: move |ev| {
                    ev.stop_propagation();
                    exec(|s| bn_session::ops::edit::set_note_collapsed(s, id, !collapsed));
                },
                button {
                    class: BAR_BTN,
                    title: "Delete note",
                    onmousedown: move |ev| ev.stop_propagation(),
                    ondoubleclick: move |ev| ev.stop_propagation(),
                    onclick: move |ev| {
                        ev.stop_propagation();
                        exec(|s| bn_session::ops::edit::delete_items(s, &[], &[], &[id]));
                    },
                    X { class: Some(BAR_ICON.to_string()) }
                }
                if collapsed {
                    div {
                        class: "min-w-0 flex-1 truncate px-1 text-[10px] leading-none",
                        "{first_line}"
                    }
                } else {
                    div { class: "flex-1" }
                }
                button {
                    class: BAR_BTN,
                    title: toggle_title,
                    onmousedown: move |ev| ev.stop_propagation(),
                    ondoubleclick: move |ev| ev.stop_propagation(),
                    onclick: move |ev| {
                        ev.stop_propagation();
                        exec(|s| bn_session::ops::edit::set_note_collapsed(s, id, !collapsed));
                    },
                    if collapsed {
                        SquareArrowDown { class: Some(BAR_ICON.to_string()) }
                    } else {
                        SquareArrowDownLeft { class: Some(BAR_ICON.to_string()) }
                    }
                }
            }

            if !collapsed {
                // ── body: static text or the inline editor ──────────────
                if editing {
                    NoteEditor { id, initial: text, font_size }
                } else {
                    div {
                        // `.note-md` (tailwind.css) styles the rendered
                        // markdown; everything there is em-based so the
                        // per-note font size scales the whole layout.
                        class: "note-md min-h-0 flex-1 overflow-hidden px-2 py-1",
                        style: "font-size: {font_size}px;",
                        // A still click on an already-selected note starts
                        // editing right away — no double-click timing needed.
                        onmouseup: move |ev| {
                            if ev.data().trigger_button() != Some(MouseButton::Primary) {
                                return;
                            }
                            let pending =
                                matches!(*GESTURE.read(), Gesture::PendingNodeDrag { .. });
                            if pending && !*DRAG_HAPPENED.read() && *was_selected.read() {
                                *EDITING_NOTE.write() = Some(id);
                            }
                            // No stop_propagation: the canvas still resets
                            // the gesture on this mouseup.
                        },
                        dangerous_inner_html: note_html(&text),
                    }
                }

                // ── bottom-right resize handle ───────────────────────────
                div {
                    class: "absolute right-0 bottom-0 size-3 opacity-0 group-hover:opacity-100",
                    style: "cursor: nwse-resize; border-bottom-right-radius: 2px; \
                            background: linear-gradient(135deg, transparent 50%, rgba(0,0,0,0.25) 50%);",
                    onmousedown: move |ev| {
                        if ev.data().trigger_button() != Some(MouseButton::Primary) {
                            return;
                        }
                        ev.stop_propagation();
                        let c = ev.data().client_coordinates();
                        *GESTURE.write() = Gesture::ResizeNote {
                            id,
                            start_client: (c.x, c.y),
                            start_size: (rect.w, rect.h),
                        };
                    },
                }
            }
        }
    }
}

/// Inline editor with a local draft; committed on blur or Escape. Mounted
/// fresh per edit session (the conditional render drops it on close).
#[component]
fn NoteEditor(id: NoteId, initial: String, font_size: f32) -> Element {
    let mut draft = use_signal(|| initial.clone());

    let commit = move || {
        let text = draft.peek().clone();
        // An undo while typing can remove the note — committing then would
        // only log a spurious "note no longer exists".
        if SESSION.read().doc().notes.contains_key(id) {
            exec(|s| bn_session::ops::edit::set_note_text(s, id, text));
        }
        if *EDITING_NOTE.peek() == Some(id) {
            *EDITING_NOTE.write() = None;
        }
    };

    rsx! {
        TextArea {
            value: "{draft}",
            oninput: move |v: String| draft.set(v),
            unstyled: true,
            autofocus: true,
            class: "min-h-0 w-full flex-1 resize-none bg-transparent px-2 py-1",
            style: "font-size: {font_size}px; line-height: 1.35;",
            onblur: move |_| commit(),
            // The global Esc hotkey runs before the TYPING guard, so end the
            // edit right here (Escape commits, like macOS Stickies).
            onkeydown: move |ev: KeyboardEvent| {
                if ev.key() == Key::Escape {
                    ev.stop_propagation();
                    commit();
                }
            },
        }
    }
}
