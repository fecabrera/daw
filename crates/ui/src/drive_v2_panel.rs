use crate::{
    icons,
    knobs::{self, Knob, KnobSpec},
    midi_editor::UNIT_STRIP_WIDTH,
    panels, parameter_knobs, rows, theme,
};
use daw_core::{DriveV2Clip, DriveV2ColorShape, DriveV2Settings, ModulationParameter as P};
use egui::{Rect, Ui, Vec2};
pub(super) struct Bounds {
    pub power: Rect,
    #[cfg(test)]
    pub sections: [Rect; 3],
    #[cfg(test)]
    pub knobs: [[Rect; 3]; 3],
    #[cfg(test)]
    pub color_power: Rect,
    #[cfg(test)]
    pub clip: Rect,
}
fn diagram(value: f32) -> Vec<Vec2> {
    let ys = match value.round() as usize {
        0 => [0.5, 0.7, 1.0, 0.7, 0.5],
        1 => [0.5, 0.3, 0.0, 0.3, 0.5],
        _ => [1.0, 0.8, 0.0, 0.3, 0.5],
    };
    ys.into_iter()
        .enumerate()
        .map(|(i, y)| Vec2::new(i as f32 / 4.0 - 0.5, y - 0.5))
        .collect()
}
pub(super) fn show(
    ui: &mut Ui,
    s: &mut DriveV2Settings,
    compact: bool,
    height: f32,
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
) -> (Bounds, egui::Response) {
    ui.scope_builder(
        egui::UiBuilder::new()
            .ui_stack_info(egui::UiStackInfo::default().with_tag("unit-modulation-button")),
        |ui| show_inner(ui, s, compact, height, sources, live),
    )
    .inner
}
fn show_inner(
    ui: &mut Ui,
    s: &mut DriveV2Settings,
    compact: bool,
    height: f32,
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
) -> (Bounds, egui::Response) {
    let bases = [
        s.input_gain_db,
        s.gain,
        s.tone,
        s.color_strength,
        s.color_frequency_hz,
        s.level_db,
        s.dry_wet,
    ];
    let markers: [_; 7] = std::array::from_fn(|i| {
        crate::modulation_panel::marker(
            &s.modulations,
            sources,
            P::DRIVE_V2[i],
            bases[i],
            s.enabled && (!(3..=4).contains(&i) || s.color_enabled),
            None,
            [live.map(|v| v[i]), None],
        )
    });
    let open = s.modulation_open;
    let panel = panels::PanelStyle::default();
    let mut bounds = Bounds {
        power: Rect::NOTHING,
        #[cfg(test)]
        sections: [Rect::NOTHING; 3],
        #[cfg(test)]
        knobs: [[Rect::NOTHING; 3]; 3],
        #[cfg(test)]
        color_power: Rect::NOTHING,
        #[cfg(test)]
        clip: Rect::NOTHING,
    };
    let DriveV2Settings {
        enabled,
        collapsed,
        input_gain_db,
        gain,
        tone,
        color_enabled,
        color_shape,
        color_strength,
        color_frequency_hz,
        level_db,
        dry_wet,
        clip,
        modulation_open,
        modulations,
        ..
    } = s;
    let result = panels::PanelStyle { margin: 0, ..panel }.unit_in_lane(ui,("Drive v2",compact),height,enabled,collapsed,|ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(0.0,f32::from(theme::TOOLBAR_PADDING));
            let mut last = Rect::NOTHING;
            for (column,name) in ["Drive","Color","Output"].into_iter().enumerate() {
                let strip = ui.push_id(column,|ui| egui::Frame::new().inner_margin(panel.margin).show(ui,|ui| { ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_width(UNIT_STRIP_WIDTH-2.0*f32::from(panel.margin));
                    ui.set_min_height(crate::envelope_panel::body_height(compact));
                    if !compact {
                        for row in 0..3 {
                            let response = if column == 1 && row == 0 {
                                ui.add_enabled_ui(*color_enabled, |ui| knobs::row(ui,color_shape,knobs::CHOICE_SIZE,|ui,value| {
                                    let mut index = value.index() as f32;
                                    let response = ui.add(Knob::new(&mut index,KnobSpec::new("Color shape",0.0..=2.0,0.0,1.0).stepped(1.0).cyclic().diagrams(diagram).format(|v| DriveV2ColorShape::ALL[v.round().clamp(0.0,2.0) as usize].name().into())));
                                    if response.changed() { *value = DriveV2ColorShape::ALL[index as usize]; } response
                                })).inner
                            } else if column == 0 && row == 0 {
                                knobs::row(ui,clip,knobs::CHOICE_SIZE,|ui,value| {
                                    let mut mode = f32::from(*value == DriveV2Clip::Hard);
                                    let response = ui.add(Knob::new(&mut mode,KnobSpec::new("Clip mode",0.0..=1.0,0.0,1.0)
                                        .two_way_labels("Soft","Hard").format(|v| if v < 0.5 { "Soft".into() } else { "Hard".into() })));
                                    if response.changed() { *value = if mode < 0.5 { DriveV2Clip::Soft } else { DriveV2Clip::Hard }; }
                                    response
                                })
                            } else {
                                let (value,i): (&mut f32,usize) = match (column,row) { (2,0) => (&mut *input_gain_db,0), (0,1) => (&mut *gain,1), (0,2) => (&mut *tone,2), (1,1) => (&mut *color_strength,3), (1,2) => (&mut *color_frequency_hz,4), (2,1) => (&mut *level_db,5), (2,2) => (&mut *dry_wet,6), _ => unreachable!() };
                                ui.add_enabled_ui(column != 1 || *color_enabled,|ui| knobs::row(ui,value,knobs::CHOICE_SIZE,|ui,v| {
                                    let spec = parameter_knobs::destination(P::DRIVE_V2[i]).named(["Input gain","Drive gain","Drive tone","Color strength","Color frequency","Output level","Dry/Wet"][i]);
                                    ui.add(Knob::new(v,spec).modulated(markers[i].as_ref(),modulation_open))
                                })).inner
                            };
                            #[cfg(test)] { bounds.knobs[column][row] = response.rect; if column == 0 && row == 0 { bounds.clip = response.rect; } }
                            #[cfg(not(test))] let _ = response;
                        }
                    }
                    rows::centered(ui,if compact { crate::envelope_panel::body_height(true) } else { icons::COMPACT_BUTTON_SIZE },|ui| {
                        if column == 1 {
                            let r = rows::power_label(ui,"Color","Toggle Color",*color_enabled);
                            #[cfg(test)] { bounds.color_power = r.rect; }
                            if r.clicked() { *color_enabled = !*color_enabled; }
                        } else { rows::unit_label(ui,name); }
                    });
                }); })).inner.response.rect;
                #[cfg(test)] { bounds.sections[column] = strip; }
                if column > 0 { panels::unit_divider(ui,strip.left(),strip.y_range().into()); } last = strip;
            }
            if open { panels::unit_divider(ui,last.right(),last.y_range().into()); crate::modulation_panel::body(ui,modulations,sources,&P::DRIVE_V2,daw_unit_core::ModulationScope::Shared,compact); }
        });
    });
    crate::modulation_panel::toggle(ui, &result.header, *collapsed, modulation_open);
    bounds.power = result.power.rect;
    (bounds, result.header)
}
