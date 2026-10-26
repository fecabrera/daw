use std::sync::Arc;
use egui::{Pos2,Vec2,Rect,Event,PointerButton};
use daw_drive_plugin::{PluginState,Parameter as P};
fn frame(ctx:&egui::Context,editor:&mut Editor,events:Vec<Event>,focused:bool)->Vec<egui::epaint::ClippedShape> {
 let mut output=ctx.run_ui(egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(WIDTH as f32,HEIGHT as f32))),events,focused,..Default::default()},|ui|editor.show(ui));output.textures_delta.clear();output.shapes
}
fn pointer(pos:Pos2,pressed:bool)->Event{Event::PointerButton{pos,button:PointerButton::Primary,pressed,modifiers:Default::default()}}
fn setup()->(egui::Context,Arc<Model>,Editor){let ctx=egui::Context::default();daw_unit_ui::fonts::configure(&ctx);daw_unit_ui::theme::configure(&ctx);let model=Arc::new(Model::default());let editor=Editor::new(model.clone(),||{});(ctx,model,editor)}
fn events(model:&Model)->Vec<Edit>{std::iter::from_fn(||model.pop()).collect()}
fn drag(ctx:&egui::Context,e:&mut Editor,point:Pos2,dy:f32,release:bool){frame(ctx,e,vec![Event::PointerMoved(point),pointer(point,true)],true);let end=point+Vec2::new(0.0,dy);frame(ctx,e,vec![Event::PointerMoved(end)],true);if release{frame(ctx,e,vec![pointer(end,false)],true);}}
#[test]
fn knobs_and_selectors_emit_balanced_typed_gestures() {
 for slot in 0..5 {
 let (ctx,model,mut editor)=setup();frame(&ctx,&mut editor,vec![],true);
 let p=P::ALL[slot+1];
 if IS_SATURATOR && (slot==2 || slot==3) {
  let pos=Pos2::new(45.0,if slot==2{36.0}else{96.0});
  for pressed in [true,false]{frame(&ctx,&mut editor,vec![Event::PointerMoved(pos),pointer(pos,pressed)],true);}
  let shapes=frame(&ctx,&mut editor,vec![],true);let text=if slot==2{"Bass Shaper"}else{"Hard Clip"};
  let pos=shapes.iter().find_map(|s| match &s.shape {egui::Shape::Text(t) if t.galley.text()==text=>Some(t.pos+t.galley.size()*0.5),_=>None}).expect("selector choice");
  for pressed in [true,false]{frame(&ctx,&mut editor,vec![Event::PointerMoved(pos),pointer(pos,pressed)],true);}
 } else {
  let point=match slot {0=>Pos2::new(135.0,36.0),1=>Pos2::new(45.0,if IS_SATURATOR{156.0}else{36.0}),2=>Pos2::new(45.0,96.0),3=>Pos2::new(45.0,156.0),_=>Pos2::new(135.0,96.0)};
  drag(&ctx,&mut editor,point,if slot==2 || slot==4{18.0}else{-18.0},true);
 }
 let edits=events(&model);assert_eq!(edits.first(),Some(&Edit::Begin(p)),"{p:?}: {edits:?}");assert!(edits.iter().any(|e| matches!(e,Edit::Value(q,_)if *q==p)));assert_eq!(edits.last(),Some(&Edit::End(p)));
 }
}
#[test]
fn focus_loss_close_and_queue_pressure_preserve_gesture_end() {
 let (ctx,model,mut editor)=setup();frame(&ctx,&mut editor,vec![],true);drag(&ctx,&mut editor,Pos2::new(135.0,36.0),-18.0,false);assert_eq!(events(&model).first(),Some(&Edit::Begin(P::Level)));frame(&ctx,&mut editor,vec![],false);assert_eq!(events(&model),[Edit::End(P::Level)]);
 let (ctx,model,mut editor)=setup();frame(&ctx,&mut editor,vec![],true);drag(&ctx,&mut editor,Pos2::new(135.0,36.0),-18.0,false);events(&model);let remaining=model.remaining();assert!(model.push(&vec![Edit::Begin(P::Bypass);remaining-1]));assert!(!model.push(&[Edit::Begin(P::Level),Edit::End(P::Level)]));drop(editor);assert_eq!(events(&model).last(),Some(&Edit::End(P::Level)));
}
#[test]
fn output_peak_reset_sends_no_parameter_edits() {
 use daw_unit_core::MeterStorage;
 let (ctx,model,mut editor)=setup();model.output_meter.store([1.5,0.0,0.0]);model.output_meter.initialize([0.25,0.0,0.0]);let state=model.current();frame(&ctx,&mut editor,vec![],true);
 let pos=Pos2::new(135.0,4.0);for pressed in [true,false]{frame(&ctx,&mut editor,vec![Event::PointerMoved(pos),pointer(pos,pressed)],true);}
 assert_eq!(model.output_meter.boost_peaks()[0].peak(),0.0);assert_eq!(model.output_meter.readings()[0],0.25);assert!(events(&model).is_empty());assert_eq!(model.current(),state);assert_eq!(state,Some(PluginState::default()));
}

#[test]
fn host_bypass_masks_queued_output_peaks_without_parameter_edits() {
    use daw_unit_core::MeterStorage;
    let (ctx, model, mut editor) = setup();
    let mut state = PluginState::default();
    state.set(P::Bypass, 1.0).unwrap();
    model.publish(state);
    model.output_meter.store([1.5, 0.0, 0.0]);
    frame(&ctx, &mut editor, vec![], true);
    assert_eq!(model.output_meter.boost_peaks()[0].peak(), 0.0);
    assert!(events(&model).is_empty());
    assert_eq!(model.current(), Some(state));
}
