//! S-SHELL H14: the window opens on an empty project, edits redraw, and a
//! project saves and opens again.

use bike_application::panel::Field;
use bike_application::ui::Studio;
use bike_model::GeometryVariable as V;
use gpui_kit::TestAppContext;

#[gpui_kit::test]
fn open_edit_save_and_reopen(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let dir = std::env::temp_dir().join(format!("bikestudio-app-{}", std::process::id()));
    let settings = dir.join("settings.json");
    let s1 = settings.clone();
    let window = cx.add_window(move |window, cx| Studio::with_settings(s1, window, cx));
    let studio = window.root(cx).unwrap();

    studio.read_with(cx, |s, _| {
        let doc = s.document();
        assert!(doc.is_preview() && !doc.dirty);
        assert!(doc.solved.geometry.is_some(), "the default preview draws");
    });

    window
        .update(cx, |s, window, cx| {
            s.commit(Field::Geometry(V::HeadTubeAngle), "64", window, cx);
            s.commit(Field::Geometry(V::Reach), "nonsense", window, cx);
        })
        .unwrap();
    studio.read_with(cx, |s, _| {
        let doc = s.document();
        assert!(doc.dirty && !doc.is_preview());
        let g = doc.solved.geometry.as_ref().unwrap();
        assert!((g.static_state.value(V::HeadTubeAngle) - 64.0).abs() < 1e-9);
        assert!(s.message().is_some_and(|m| m.starts_with("Reach")));
        assert_eq!(doc.project.geometry.pinned(V::Reach), None);
    });

    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("enduro.bikestud");
    studio.update(cx, |s, cx| s.save_to(&path, cx));
    studio.read_with(cx, |s, _| assert!(!s.document().dirty));

    let s2 = settings.clone();
    let window2 = cx.add_window(move |window, cx| Studio::with_settings(s2, window, cx));
    window2
        .update(cx, |s, window, cx| s.open_path(&path, window, cx))
        .unwrap();
    let studio2 = window2.root(cx).unwrap();
    studio2.read_with(cx, |s, _| {
        assert_eq!(
            s.document().project.geometry.pinned(V::HeadTubeAngle),
            Some(64.0)
        );
        assert_eq!(
            s.document().name(),
            path.file_stem().unwrap().to_string_lossy()
        );
    });
    // Opening it put it in the recent list.
    let saved = bike_application::settings::Settings::load(&settings);
    assert_eq!(saved.recent.first(), Some(&path));
    std::fs::remove_dir_all(&dir).unwrap();
}
