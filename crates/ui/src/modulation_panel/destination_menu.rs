//! Presentation of destination choices, independent of connection defaults.
use super::*;

struct Group<'a> {
    metadata: &'a TargetGroup,
    targets: Vec<&'a Target>,
}
fn groups(targets: &[Target]) -> Vec<Group<'_>> {
    let mut groups: Vec<Group<'_>> = Vec::new();
    for target in targets {
        let Some(metadata) = &target.group else {
            continue;
        };
        if let Some(group) = groups.iter_mut().find(|g| g.metadata.id == metadata.id) {
            group.targets.push(target);
        } else {
            groups.push(Group {
                metadata,
                targets: vec![target],
            });
        }
    }
    groups.sort_by_key(|g| g.metadata.order);
    groups
}
fn assign<'a>(
    route: &mut Modulation,
    target: &Target,
    scope: ModulationScope,
    sources_for_target: &impl Fn(Option<daw_core::Id>) -> &'a [Source],
) {
    let target_changed = route.target != target.id;
    route.parameter = target.parameter;
    route.target = target.id;
    if target_changed
        && scope == ModulationScope::Shared
        && route.source.is_some_and(|source| {
            !sources_for_target(target.id)
                .iter()
                .any(|s| s.id == source && scope.accepts(s.scope))
        })
    {
        route.source = None;
    }
    let limit = target
        .amount_limit
        .unwrap_or_else(|| target.parameter.limit());
    route.amount = route.amount.clamp(-limit, limit);
}
fn leaf<'a>(
    ui: &mut Ui,
    route: &mut Modulation,
    target: &Target,
    scope: ModulationScope,
    sources_for_target: &impl Fn(Option<daw_core::Id>) -> &'a [Source],
) {
    ui.push_id(
        (target.id, std::mem::discriminant(&target.parameter)),
        |ui| {
            if ui
                .selectable_label(
                    route.parameter == target.parameter && route.target == target.id,
                    source_menu_label(&target.leaf),
                )
                .clicked()
            {
                assign(route, target, scope, sources_for_target);
                ui.close();
            }
        },
    );
}
pub(super) fn destination_picker<'a>(
    ui: &mut Ui,
    route: &mut Modulation,
    targets: &[Target],
    scope: ModulationScope,
    sources_for_target: impl Fn(Option<daw_core::Id>) -> &'a [Source],
) -> egui::Response {
    crate::menus::Dropdown::from_id_salt("parameter")
        .small_items()
        .selected_text(
            egui::RichText::new(
                targets
                    .iter()
                    .find(|t| t.parameter == route.parameter && t.id == route.target)
                    .map_or("Unavailable", |t| t.label.as_str()),
            )
            .size(panels::HEADER_FONT_SIZE),
        )
        .width(ui.available_width())
        .truncate()
        .show_ui(ui, |ui| {
            let groups = groups(targets);
            let roots: Vec<_> = targets.iter().filter(|t| t.group.is_none()).collect();
            let width = source_menu_width(
                ui,
                roots
                    .iter()
                    .map(|t| t.leaf.as_str())
                    .chain(groups.iter().map(|g| g.metadata.name.as_str())),
                !groups.is_empty(),
            );
            crate::menus::layout(ui, width, |ui| {
                for target in &roots {
                    leaf(ui, route, target, scope, &sources_for_target);
                }
                if !roots.is_empty() && !groups.is_empty() {
                    crate::menus::separator(ui);
                }
                for group in groups {
                    ui.push_id(group.metadata.id, |ui| {
                        egui::containers::menu::SubMenuButton::from_button(
                            egui::Button::new(source_menu_label(&group.metadata.name))
                                .right_text(
                                    crate::icons::Lucide::ChevronRight
                                        .size(crate::icons::TOOLBAR_ICON_SIZE)
                                        .image(),
                                )
                                .image_tint_follows_text_color(true),
                        )
                        .config(
                            egui::containers::menu::MenuConfig::new()
                                .style(crate::menus::small_popup_style),
                        )
                        .ui(ui, |ui| {
                            let width = source_menu_width(
                                ui,
                                group.targets.iter().map(|t| t.leaf.as_str()),
                                false,
                            );
                            crate::menus::layout(ui, width, |ui| {
                                for target in group.targets {
                                    leaf(ui, route, target, scope, &sources_for_target);
                                }
                            });
                        });
                    });
                }
            });
        })
        .response
}

#[cfg(test)]
#[path = "destination_tests.rs"]
mod tests;
