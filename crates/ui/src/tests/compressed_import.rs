use super::*;

#[test]
fn compressed_menu_import_and_file_drop_publish_prepared_clips() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../media/tests/fixtures");
    for extension in ["mp3", "aac", "m4a"] {
        let path = fixtures.join(format!("tone-2.{extension}"));
        let mut app = DawUi::default();
        let ctx = context();
        app.import(path.clone());
        finish_import(&mut app, &ctx);
        assert!(app.error.is_none(), "{:?}", app.error);
        assert_eq!(app.session.project.tracks.len(), 1);
        assert_eq!(
            app.session.project.assets[0]
                .source_metadata
                .bits_per_sample,
            0
        );
        let mut app = DawUi::default();
        let track = app.session.project.add_track().unwrap();
        frame(&mut app, &ctx, vec![]);
        let pointer = app.lane_bounds[&track].min + Vec2::new(80.0, 12.0);
        ready_file_hover(&mut app, &ctx, &path, pointer);
        assert!(
            app.file_hover
                .as_ref()
                .unwrap()
                .audio
                .as_ref()
                .unwrap()
                .is_ok()
        );
        file_frame(&mut app, &ctx, &path, pointer, true);
        finish_import(&mut app, &ctx);
        assert!(app.error.is_none(), "{:?}", app.error);
        assert_eq!(app.session.project.tracks.len(), 1);
        assert_eq!(app.session.project.tracks[0].clips.len(), 1);
    }
}
