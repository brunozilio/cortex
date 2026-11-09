use super::*;

#[test]
fn revealing_pans_just_enough_to_bring_a_rect_into_view() {
    let area = Rect::new(200.0, 0.0, 1000.0, 800.0);
    let mut camera = Camera::default();
    let keep = Rect::new(400.0, 100.0, 100.0, 100.0);
    camera.reveal(Rect::new(900.0, 100.0, 400.0, 300.0), keep, area, 20.0);
    camera.settle();
    let shown = camera.to_screen(Rect::new(900.0, 100.0, 400.0, 300.0));
    assert_eq!(shown.right(), area.right() - 20.0, "right edge just inside");
    assert_eq!(shown.y, 100.0, "no vertical move needed");
    let before = camera.pan;
    camera.reveal(Rect::new(900.0, 100.0, 100.0, 100.0), keep, area, 20.0);
    assert_eq!(camera.pan, before, "already in view");
    assert!(!camera.is_animating());
}

#[test]
fn revealing_zooms_out_when_the_rest_would_be_lost() {
    let area = Rect::new(200.0, 0.0, 1000.0, 800.0);
    let mut camera = Camera::default();
    let (card, keep) = (Rect::new(1100.0, 100.0, 600.0, 500.0), Rect::new(300.0, 100.0, 200.0, 200.0));
    camera.reveal(card, keep, area, 20.0);
    camera.settle();
    assert!(camera.zoom < 1.0, "zoomed out");
    let (card, keep) = (camera.to_screen(card), camera.to_screen(keep));
    assert!(card.right() <= area.right() - 20.0 + 0.5);
    assert!(keep.x >= area.x, "the rest of the canvas stays in view");
}

#[test]
fn nodes_pan_under_the_sidebar_until_the_right_edge_reaches_the_line() {
    let mut camera = Camera::default();
    camera.pan_by(-400.0, 0.0);
    camera.fence(900.0, 320.0);
    assert_eq!(camera.pan.x, -400.0, "the left nodes are under the sidebar");
    camera.pan_by(-500.0, 0.0);
    camera.fence(900.0, 320.0);
    assert_eq!(camera.pan.x, -580.0);
    assert_eq!(camera.to_screen(Rect::new(900.0, 0.0, 1.0, 1.0)).x, 320.0);
    camera.pan_by(500.0, 0.0);
    camera.fence(900.0, 320.0);
    assert_eq!(camera.pan.x, -80.0);
    camera.zoom_at(0.5, Point::new(1200.0, 0.0));
    camera.fence(900.0, 320.0);
    assert!(camera.to_screen(Rect::new(900.0, 0.0, 1.0, 1.0)).x >= 320.0 - 0.01);
}

#[test]
fn nodes_under_the_sidebar_stay_there_without_a_panel() {
    let mut camera = Camera::default();
    camera.pan_by(-400.0, 0.0);
    camera.make_room(None, 160.0);
    settle_room(&mut camera);
    assert_eq!(camera.offset().x, -400.0);
}

#[test]
fn room_made_for_a_panel_is_given_back_when_it_closes() {
    let mut camera = Camera::default();
    camera.fence(900.0, 320.0);
    camera.make_room(Some(60.0), 160.0);
    camera.step(1.0 / 60.0);
    assert!(camera.offset().x > 0.0 && camera.offset().x < 100.0);
    settle_room(&mut camera);
    assert_eq!((camera.pan.x, camera.offset().x), (0.0, 100.0));
    assert_eq!(camera.to_screen(Rect::new(60.0, 0.0, 1.0, 1.0)).x, 160.0);
    camera.make_room(None, 160.0);
    camera.step(1.0 / 60.0);
    assert!(camera.offset().x > 0.0 && camera.offset().x < 100.0);
    settle_room(&mut camera);
    assert_eq!(camera.offset().x, 0.0);
}

fn settle_room(camera: &mut Camera) {
    for _ in 0..600 {
        camera.step(1.0 / 60.0);
    }
}

#[test]
fn screen_rects_map_through_zoom_and_pan() {
    let mut camera = Camera::default();
    camera.zoom_at(2.0, Point::new(0.0, 0.0));
    camera.pan_by(20.0, 0.0);
    assert_eq!(camera.rect_to_canvas(Rect::new(0.0, 0.0, 160.0, 300.0)), Rect::new(-10.0, 0.0, 80.0, 150.0));
}

fn close(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3
}

#[test]
fn identity_maps_screen_to_canvas() {
    let camera = Camera::default();
    assert_eq!(camera.to_canvas(Point::new(10.0, 20.0)), Point::new(10.0, 20.0));
    assert_eq!(camera, Camera::default());
}

#[test]
fn panning_follows_the_fingers_at_any_zoom() {
    let mut camera = Camera { zoom: 2.0, raw: 2.0, ..Camera::default() };
    let before = camera.to_canvas(Point::new(100.0, 100.0));
    camera.pan_by(40.0, -20.0);
    let after = camera.to_canvas(Point::new(140.0, 80.0));
    assert!(close(before, after), "the content moved with the fingers");
}

#[test]
fn zoom_keeps_the_point_under_the_pointer() {
    let mut camera = Camera::default();
    let pointer = Point::new(300.0, 200.0);
    let anchor = camera.to_canvas(pointer);
    camera.zoom_at(1.5, pointer);
    assert!(close(camera.to_canvas(pointer), anchor));
    assert_eq!(camera.zoom, 1.5);
}

#[test]
fn zoom_is_clamped_and_resets() {
    let mut camera = Camera::default();
    camera.zoom_at(100.0, Point::default());
    assert_eq!(camera.zoom, MAX_ZOOM);
    camera.zoom_at(0.0001, Point::default());
    assert_eq!(camera.zoom, MIN_ZOOM);
    camera.zoom_at(1.0123, Point::default());
    assert_eq!(camera.zoom, MIN_ZOOM, "a tiny pinch snaps to the same step");
    for _ in 0..10 {
        camera.zoom_at(1.005, Point::default());
    }
    assert!(camera.zoom > MIN_ZOOM, "but many tiny pinches add up");
    camera.reset();
    assert_eq!(camera, Camera::default());
}

#[test]
fn centring_puts_the_composer_in_the_middle_of_the_canvas_at_the_same_zoom() {
    let area = Rect::new(240.0, 0.0, 1400.0, 900.0);
    let composer = Rect::new(900.0, 500.0, 440.0, 190.0);
    let mut camera = Camera { zoom: 1.5, raw: 1.5, ..Camera::default() };
    camera.pan_by(-3000.0, -2500.0);
    camera.centre_on(composer, area);
    for _ in 0..600 {
        camera.step(1.0 / 60.0);
    }
    let centre = camera.to_screen(composer).center();
    assert!(close(centre, area.center()), "composer centre {centre:?} vs area {:?}", area.center());
    assert_eq!(camera.zoom, 1.5, "the zoom stays");
}

#[test]
fn centring_brings_a_composer_panned_out_of_sight_back_into_the_canvas() {
    let area = Rect::new(240.0, 0.0, 1400.0, 900.0);
    let composer = Rect::new(900.0, 500.0, 440.0, 190.0);
    let mut camera = Camera { zoom: 1.0, raw: 1.0, ..Camera::default() };
    camera.pan_by(-3000.0, -2500.0);
    assert!(crate::view::sidebar::off_screen(camera.to_screen(composer), area), "precondition: out of sight");
    camera.reveal(composer, composer, area, 24.0);
    for _ in 0..600 {
        camera.step(1.0 / 60.0);
    }
    assert!(!crate::view::sidebar::off_screen(camera.to_screen(composer), area), "back in view");
}
