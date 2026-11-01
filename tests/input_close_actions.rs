use agent_meter::input::{action_for, Action};
use agent_meter::popup::close_button;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;

fn left_down(column: u16, row: u16, modifiers: KeyModifiers) -> Event {
    Event::Mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column, row, modifiers })
}

#[test]
fn clicks_are_tested_against_the_popup_where_it_is_drawn() {
    let area = Rect::new(5, 3, 40, 12);
    let button = close_button(area);
    assert!(!button.is_empty());

    // Every cell of the button closes it, wherever the popup sits and whatever
    // modifier keys are held.
    for x in button.x..button.x + button.width {
        for modifiers in [KeyModifiers::NONE, KeyModifiers::SHIFT, KeyModifiers::CONTROL] {
            assert_eq!(
                action_for(&left_down(x, button.y, modifiers), area),
                Some(Action::Close),
                "x={x} {modifiers:?}"
            );
        }
    }

    // The same coordinates do nothing for a popup somewhere else.
    let elsewhere = Rect::new(0, 0, 40, 12);
    let moved = close_button(elsewhere);
    assert_ne!(moved, button);
    assert_eq!(action_for(&left_down(button.x, button.y, KeyModifiers::NONE), elsewhere), None);
}

#[test]
fn a_header_too_narrow_to_show_the_button_cannot_be_clicked_closed() {
    let narrow = Rect::new(0, 0, 8, 6);
    assert!(close_button(narrow).is_empty());
    for x in 0..8 {
        for y in 0..6 {
            assert_eq!(action_for(&left_down(x, y, KeyModifiers::NONE), narrow), None, "({x},{y})");
        }
    }
    // Esc still works without a visible button.
    let esc = Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(action_for(&esc, narrow), Some(Action::Close));
}
