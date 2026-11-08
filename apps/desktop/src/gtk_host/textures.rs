//! CPU copies of live egui textures, retained while GTK recreates its GL surface.
use egui::{ColorImage, ImageData, TextureId, TextureOptions, TexturesDelta};
use std::{collections::HashMap, sync::Arc};

#[derive(Default)]
pub(super) struct Textures(HashMap<TextureId, (Arc<ColorImage>, TextureOptions)>);

impl Textures {
    pub fn apply(&mut self, changes: &TexturesDelta) {
        for (id, updates) in &changes.set {
            for update in updates {
                let ImageData::Color(image) = &update.image;
                if let Some([x, y]) = update.pos {
                    if let Some((stored, options)) = self.0.get_mut(id) {
                        let stored = Arc::make_mut(stored);
                        // egui guarantees patches fit their existing texture.
                        for row in 0..image.height() {
                            let start = (y + row) * stored.width() + x;
                            stored.pixels[start..start + image.width()].copy_from_slice(
                                &image.pixels[row * image.width()..(row + 1) * image.width()],
                            );
                        }
                        *options = update.options;
                    }
                } else {
                    self.0.insert(*id, (image.clone(), update.options));
                }
            }
        }
        for id in &changes.free {
            self.0.remove(id);
        }
    }

    pub fn restore(&self, painter: &mut egui_glow::Painter) {
        for (id, (image, options)) in &self.0 {
            painter.set_texture(
                *id,
                &egui::epaint::ImageDelta::full(ImageData::Color(image.clone()), *options),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Color32, epaint::ImageDelta};

    #[test]
    fn patches_survive_surface_recreation_and_freed_images_are_retired() {
        let id = TextureId::Managed(1);
        let mut textures = Textures::default();
        let mut changes = TexturesDelta::default();
        changes.push(
            id,
            ImageDelta::full(
                ColorImage::filled([3, 2], Color32::BLACK),
                TextureOptions::LINEAR,
            ),
        );
        textures.apply(&changes);
        changes.clear();
        changes.push(
            id,
            ImageDelta::partial(
                [1, 1],
                ColorImage::filled([2, 1], Color32::RED),
                TextureOptions::NEAREST,
            ),
        );
        textures.apply(&changes);
        changes.clear();
        let (image, options) = &textures.0[&id];
        assert_eq!(
            image.pixels,
            [
                Color32::BLACK,
                Color32::BLACK,
                Color32::BLACK,
                Color32::BLACK,
                Color32::RED,
                Color32::RED
            ]
        );
        assert_eq!(*options, TextureOptions::NEAREST);
        changes.free(id);
        textures.apply(&changes);
        assert!(textures.0.is_empty());
        changes.clear();
    }
}
