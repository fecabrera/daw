pub fn find_label(shape: &egui::Shape, name: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(text) if text.galley.text() == name => {
            Some(text.pos + text.galley.rect.center().to_vec2())
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find_label(shape, name)),
        _ => None,
    }
}
