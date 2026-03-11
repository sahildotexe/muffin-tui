use muffintui::theme::{THEMES, pane_block};
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

#[test]
fn exposes_expected_theme_count() {
    assert_eq!(THEMES.len(), 3);
    assert_eq!(THEMES[0].name, "Teal Night");
}

#[test]
fn pane_block_renders_title() {
    let area = Rect::new(0, 0, 20, 3);
    let mut buffer = Buffer::empty(area);
    pane_block("Files", true, THEMES[0]).render(area, &mut buffer);

    let rendered: String = (0..area.width).map(|x| buffer[(x, 0)].symbol()).collect();
    assert!(rendered.contains("Files"));
}
