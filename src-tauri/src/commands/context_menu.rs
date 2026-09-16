// SPDX-License-Identifier: MIT OR Apache-2.0

//! The main window's right-click menu, drawn by the OS.
//!
//! It used to be HTML inside the webview, which capped it at the window's own
//! size: a short window cut the menu off, and no clamp can fix that from the
//! inside. A native popup menu is drawn by the OS above every window, so it
//! is never clipped and gets the OS's own dismissal and keyboard handling.
//!
//! The webview still owns what the menu says and does. It sends the rows as
//! [`MenuSpec`]s with ids of its own choosing; this side builds and pops the
//! menu, and `main`'s menu-event handler sends the picked id straight back as
//! [`CONTEXT_MENU_PICK_EVENT`]. Nothing here knows what a row means.

use serde::Deserialize;
use tauri::menu::{CheckMenuItem, ContextMenu, Menu, MenuItem, PredefinedMenuItem};
use tauri::{LogicalPosition, Manager, Position, Runtime};

/// Emitted to the main window with the picked row's id as its `String`
/// payload. Dismissing the menu without a pick emits nothing.
pub const CONTEXT_MENU_PICK_EVENT: &str = "context-menu-pick";

/// One row, as the webview describes it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum MenuSpec {
    /// A row with a check mark, one of a group the webview keeps exclusive.
    Radio {
        id: String,
        label: String,
        checked: bool,
    },
    Action {
        id: String,
        label: String,
    },
    Separator,
}

/// A menu with one item per spec, in order.
pub fn build<R: Runtime, M: Manager<R>>(app: &M, specs: &[MenuSpec]) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app)?;
    for spec in specs {
        match spec {
            MenuSpec::Radio { id, label, checked } => {
                // A check item, not a true radio: the webview keeps the group
                // exclusive by rebuilding the menu with one row checked, and
                // the OS draws a check mark either way.
                menu.append(&CheckMenuItem::with_id(
                    app,
                    id,
                    label,
                    true,
                    *checked,
                    None::<&str>,
                )?)?;
            }
            MenuSpec::Action { id, label } => {
                menu.append(&MenuItem::with_id(app, id, label, true, None::<&str>)?)?;
            }
            MenuSpec::Separator => menu.append(&PredefinedMenuItem::separator(app)?)?,
        }
    }
    Ok(menu)
}

/// Pop the menu at (X, Y), logical px from the calling window's top-left —
/// the `clientX`/`clientY` of the `contextmenu` event, as the webview sends
/// them.
#[tauri::command]
pub fn show_context_menu<R: Runtime>(
    items: Vec<MenuSpec>,
    x: f64,
    y: f64,
    window: tauri::Window<R>,
) -> Result<(), String> {
    let menu = build(&window, &items).map_err(|e| e.to_string())?;
    menu.popup_at(window, Position::Logical(LogicalPosition::new(x, y)))
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::menu::MenuItemKind;

    #[test]
    fn a_spec_is_the_webview_s_wire_shape() {
        let specs: Vec<MenuSpec> = serde_json::from_str(
            r#"[
                {"kind":"radio","id":"ctx-0","label":"Reading 1 of 2","checked":true},
                {"kind":"separator"},
                {"kind":"action","id":"ctx-2","label":"Quit"}
            ]"#,
        )
        .unwrap();
        assert_eq!(
            specs,
            vec![
                MenuSpec::Radio {
                    id: "ctx-0".into(),
                    label: "Reading 1 of 2".into(),
                    checked: true,
                },
                MenuSpec::Separator,
                MenuSpec::Action {
                    id: "ctx-2".into(),
                    label: "Quit".into(),
                },
            ]
        );
    }

    /// One item per spec, in order, with the check state carried through and
    /// the id the webview chose kept, since that id is what comes back.
    #[test]
    fn the_menu_mirrors_the_specs() {
        let app = tauri::test::mock_app();
        let specs = vec![
            MenuSpec::Radio {
                id: "ctx-0".into(),
                label: "Header".into(),
                checked: false,
            },
            MenuSpec::Radio {
                id: "ctx-1".into(),
                label: "Content".into(),
                checked: true,
            },
            MenuSpec::Separator,
            MenuSpec::Action {
                id: "ctx-3".into(),
                label: "Quit".into(),
            },
        ];

        let menu = build(app.handle(), &specs).unwrap();
        let items = menu.items().unwrap();
        assert_eq!(items.len(), 4);
        match &items[1] {
            MenuItemKind::Check(item) => {
                assert_eq!(item.id().0, "ctx-1");
                assert_eq!(item.text().unwrap(), "Content");
                assert!(item.is_checked().unwrap());
            }
            _ => panic!("expected a check item"),
        }
        match &items[0] {
            MenuItemKind::Check(item) => assert!(!item.is_checked().unwrap()),
            _ => panic!("expected a check item"),
        }
        assert!(matches!(items[2], MenuItemKind::Predefined(_)));
        match &items[3] {
            MenuItemKind::MenuItem(item) => assert_eq!(item.id().0, "ctx-3"),
            _ => panic!("expected a plain item"),
        }
    }
}
