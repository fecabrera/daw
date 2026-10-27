//! Saturator body shared by the DAW shell and plugin editors.
use crate::{
    knobs::{self, Knob, ModulationMarker},
    layout::*,
    meters, parameter_knobs, rows, theme,
};
use daw_unit_core::{ModulationParameter as P, SaturatorSettings};
use egui::{Rect, Response, Ui, Vec2};
pub const SIZE: Vec2 = Vec2::new(
    2.0 * UNIT_STRIP_WIDTH,
    crate::layout::unit_control_height() + 2.0 * PANEL_MARGIN as f32,
);
pub struct Bounds {
    pub meter: Rect,
    pub sections: [Rect; 2],
    /// Spatial order: three left controls, then the two output controls.
    pub knobs: [Rect; 5],
    /// Host parameter order: Level, Drive, then the two unit choices and Dry/Wet.
    pub controls: [Option<Response>; 5],
    pub rect: Rect,
}
pub fn show(
    ui: &mut Ui,
    settings: &mut SaturatorSettings,
    compact: bool,
    compact_height: f32,
    markers: &[Option<ModulationMarker>; 3],
    level: meters::OutputLevel<'_>,
) -> Bounds {
    let mut bounds = Bounds {
        meter: Rect::NOTHING,
        sections: [Rect::NOTHING; 2],
        knobs: [Rect::NOTHING; 5],
        controls: std::array::from_fn(|_| None),
        rect: Rect::NOTHING,
    };
    let result=ui.horizontal_top(|ui| {
 ui.spacing_mut().item_spacing=Vec2::new(0.0,f32::from(theme::TOOLBAR_PADDING));
 for (column,name) in ["Saturation","Out"].into_iter().enumerate() {
  let strip=ui.push_id(column,|ui| egui::Frame::new().inner_margin(PANEL_MARGIN).show(ui,|ui| {
    ui.with_layout(egui::Layout::top_down(egui::Align::Min),|ui| {
     ui.set_width(UNIT_STRIP_WIDTH-2.0*f32::from(PANEL_MARGIN));
     ui.set_min_height(if compact {compact_height} else {crate::layout::unit_control_height()});
     if !compact { for row in 0..3 { let index=column*3+row;
      if index==5 { knobs::row(ui,settings,knobs::CHOICE_SIZE,|ui,_| ui.allocate_response(Vec2::splat(knobs::CHOICE_SIZE),egui::Sense::hover())); continue; }

      let control=|ui:&mut Ui,s:&mut SaturatorSettings| {
       match index {
        0=>rows::selector_row(ui,"Shape",|ui| {
          let before=s.shape;
          let mut response=crate::menus::Dropdown::from_id_salt("shape").small_items().selected_text(egui::RichText::new(s.shape.label()).size(HEADER_FONT_SIZE)).width(65.0).truncate().show_ui(ui,|ui| {for shape in daw_unit_core::SaturatorShape::ALL {ui.selectable_value(&mut s.shape,shape,shape.label());}}).response;
          if before!=s.shape {response.mark_changed();} response
        }),
        1=>rows::selector_row(ui,"Clip Mode",|ui| {
          let before=s.clip_mode;
          let mut response=crate::menus::Dropdown::from_id_salt("clip-mode").small_items().selected_text(egui::RichText::new(s.clip_mode.label()).size(HEADER_FONT_SIZE)).width(65.0).truncate().show_ui(ui,|ui| {for clip in daw_unit_core::SaturatorClipMode::ALL {ui.selectable_value(&mut s.clip_mode,clip,clip.label());}}).response;
          if before!=s.clip_mode {response.mark_changed();} response
        }),
        _=> {let target=match index {2=>0,3=>1,_=>2}; let value=match index {2=>&mut s.drive_db,3=>&mut s.level_db,_=>&mut s.dry_wet}; ui.add(Knob::new(value,parameter_knobs::destination(P::SATURATOR[target]).named(P::SATURATOR[target].label())).modulated(markers[target].as_ref(),&mut s.modulation_open))}
       }
      };
      let response=if index<2 {control(ui,settings)} else {knobs::row(ui,settings,knobs::CHOICE_SIZE,control)};
      let slot=[2,3,1,0,4][index];

      bounds.knobs[index]=response.rect;
      bounds.controls[slot]=Some(response);
     }}
     rows::centered(ui,if compact {compact_height} else {COMPACT_BUTTON_SIZE},|ui| {rows::unit_label(ui,name);});
    });
  })).inner.response.rect;
  bounds.sections[column]=strip;
  if column==1 && !compact {bounds.meter=meters::effect_output_level(ui,strip,PANEL_MARGIN,level,settings.enabled);}
  if column>0 {crate::drive::divider(ui,strip.left(),strip.y_range());}
 }
 });
    bounds.rect = result.response.rect;
    bounds
}
