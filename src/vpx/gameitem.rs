pub mod bumper;
pub mod decal;
pub mod dragpoint;
pub mod flasher;
pub mod flipper;
pub mod font;
pub mod gate;
pub mod generic;
pub mod hittarget;
pub mod kicker;
pub mod light;
pub mod lightsequencer;
pub mod plunger;
pub mod primitive;
pub mod ramp;
pub mod ramp_image_alignment;
pub mod reel;
pub mod rubber;
pub mod spinner;
pub mod textbox;
pub mod timer;
pub mod trigger;
pub mod vertex2d;
pub mod vertex3d;
pub mod vertex4d;
pub mod wall;

use crate::vpx::biff::BiffRead;
use serde::{Deserialize, Serialize};

use super::biff::{BiffReader, BiffWrite, BiffWriter};

// TODO we might come up with a macro that generates the biff reading from the struct annotations
//   like VPE

trait GameItem: BiffRead {
    fn name(&self) -> &str;
}

#[allow(clippy::large_enum_variant)]
#[derive(PartialEq, Debug, Serialize, Deserialize)]
// #[serde(tag = "type")]
pub enum GameItemEnum {
    Wall(wall::Wall),
    Flipper(flipper::Flipper),
    Timer(timer::Timer),
    Plunger(plunger::Plunger),
    TextBox(textbox::TextBox),
    Bumper(bumper::Bumper),
    Trigger(trigger::Trigger),
    Light(light::Light),
    Kicker(kicker::Kicker),
    Decal(decal::Decal),
    Gate(gate::Gate),
    Spinner(spinner::Spinner),
    Ramp(ramp::Ramp),
    Reel(reel::Reel),
    LightSequencer(lightsequencer::LightSequencer),
    Primitive(primitive::Primitive),
    Flasher(flasher::Flasher),
    Rubber(rubber::Rubber),
    HitTarget(hittarget::HitTarget),
    Generic(u32, generic::Generic),
}

impl GameItemEnum {
    // TODO clean up this mess

    pub(crate) fn editor_layer_visibility(&self) -> Option<bool> {
        match self {
            GameItemEnum::Wall(wall) => wall.editor_layer_visibility,
            GameItemEnum::Flipper(flipper) => flipper.editor_layer_visibility,
            GameItemEnum::Timer(timer) => timer.editor_layer_visibility,
            GameItemEnum::Plunger(plunger) => plunger.editor_layer_visibility,
            GameItemEnum::TextBox(textbox) => textbox.editor_layer_visibility,
            GameItemEnum::Bumper(bumper) => bumper.editor_layer_visibility,
            GameItemEnum::Trigger(trigger) => trigger.editor_layer_visibility,
            GameItemEnum::Light(light) => light.editor_layer_visibility,
            GameItemEnum::Kicker(kicker) => kicker.editor_layer_visibility,
            GameItemEnum::Decal(decal) => decal.editor_layer_visibility,
            GameItemEnum::Gate(gate) => gate.editor_layer_visibility,
            GameItemEnum::Spinner(spinner) => spinner.editor_layer_visibility,
            GameItemEnum::Ramp(ramp) => ramp.editor_layer_visibility,
            GameItemEnum::Reel(reel) => reel.editor_layer_visibility,
            GameItemEnum::LightSequencer(lightsequencer) => lightsequencer.editor_layer_visibility,
            GameItemEnum::Primitive(primitive) => primitive.editor_layer_visibility,
            GameItemEnum::Flasher(flasher) => flasher.editor_layer_visibility,
            GameItemEnum::Rubber(rubber) => rubber.editor_layer_visibility,
            GameItemEnum::HitTarget(hittarget) => hittarget.editor_layer_visibility,
            GameItemEnum::Generic(_item_type, _generic) => None,
        }
    }

    pub(crate) fn editor_layer_name(&self) -> &Option<String> {
        match self {
            GameItemEnum::Wall(wall) => &wall.editor_layer_name,
            GameItemEnum::Flipper(flipper) => &flipper.editor_layer_name,
            GameItemEnum::Timer(timer) => &timer.editor_layer_name,
            GameItemEnum::Plunger(plunger) => &plunger.editor_layer_name,
            GameItemEnum::TextBox(textbox) => &textbox.editor_layer_name,
            GameItemEnum::Bumper(bumper) => &bumper.editor_layer_name,
            GameItemEnum::Trigger(trigger) => &trigger.editor_layer_name,
            GameItemEnum::Light(light) => &light.editor_layer_name,
            GameItemEnum::Kicker(kicker) => &kicker.editor_layer_name,
            GameItemEnum::Decal(decal) => &decal.editor_layer_name,
            GameItemEnum::Gate(gate) => &gate.editor_layer_name,
            GameItemEnum::Spinner(spinner) => &spinner.editor_layer_name,
            GameItemEnum::Ramp(ramp) => &ramp.editor_layer_name,
            GameItemEnum::Reel(reel) => &reel.editor_layer_name,
            GameItemEnum::LightSequencer(lightsequencer) => &lightsequencer.editor_layer_name,
            GameItemEnum::Primitive(primitive) => &primitive.editor_layer_name,
            GameItemEnum::Flasher(flasher) => &flasher.editor_layer_name,
            GameItemEnum::Rubber(rubber) => &rubber.editor_layer_name,
            GameItemEnum::HitTarget(hittarget) => &hittarget.editor_layer_name,
            GameItemEnum::Generic(_item_type, _generic) => &None,
        }
    }

    pub(crate) fn editor_layer(&self) -> Option<u32> {
        match self {
            GameItemEnum::Wall(wall) => Some(wall.editor_layer),
            GameItemEnum::Flipper(flipper) => Some(flipper.editor_layer),
            GameItemEnum::Timer(timer) => Some(timer.editor_layer),
            GameItemEnum::Plunger(plunger) => Some(plunger.editor_layer),
            GameItemEnum::TextBox(textbox) => Some(textbox.editor_layer),
            GameItemEnum::Bumper(bumper) => Some(bumper.editor_layer),
            GameItemEnum::Trigger(trigger) => Some(trigger.editor_layer),
            GameItemEnum::Light(light) => Some(light.editor_layer),
            GameItemEnum::Kicker(kicker) => Some(kicker.editor_layer),
            GameItemEnum::Decal(decal) => Some(decal.editor_layer),
            GameItemEnum::Gate(gate) => Some(gate.editor_layer),
            GameItemEnum::Spinner(spinner) => Some(spinner.editor_layer),
            GameItemEnum::Ramp(ramp) => Some(ramp.editor_layer),
            GameItemEnum::Reel(reel) => Some(reel.editor_layer),
            GameItemEnum::LightSequencer(lightsequencer) => lightsequencer.editor_layer,
            GameItemEnum::Primitive(primitive) => Some(primitive.editor_layer),
            GameItemEnum::Flasher(flasher) => Some(flasher.editor_layer),
            GameItemEnum::Rubber(rubber) => Some(rubber.editor_layer),
            GameItemEnum::HitTarget(hittarget) => Some(hittarget.editor_layer),
            GameItemEnum::Generic(_item_type, _generic) => None,
        }
    }

    pub(crate) fn is_locked(&self) -> Option<bool> {
        match self {
            GameItemEnum::Wall(wall) => Some(wall.is_locked),
            GameItemEnum::Flipper(flipper) => Some(flipper.is_locked),
            GameItemEnum::Timer(timer) => Some(timer.is_locked),
            GameItemEnum::Plunger(plunger) => Some(plunger.is_locked),
            GameItemEnum::TextBox(textbox) => Some(textbox.is_locked),
            GameItemEnum::Bumper(bumper) => Some(bumper.is_locked),
            GameItemEnum::Trigger(trigger) => Some(trigger.is_locked),
            GameItemEnum::Light(light) => Some(light.is_locked),
            GameItemEnum::Kicker(kicker) => Some(kicker.is_locked),
            GameItemEnum::Decal(decal) => Some(decal.is_locked),
            GameItemEnum::Gate(gate) => Some(gate.is_locked),
            GameItemEnum::Spinner(spinner) => Some(spinner.is_locked),
            GameItemEnum::Ramp(ramp) => Some(ramp.is_locked),
            GameItemEnum::Reel(reel) => Some(reel.is_locked),
            GameItemEnum::LightSequencer(lightsequencer) => lightsequencer.is_locked,
            GameItemEnum::Primitive(primitive) => Some(primitive.is_locked),
            GameItemEnum::Flasher(flasher) => Some(flasher.is_locked),
            GameItemEnum::Rubber(rubber) => Some(rubber.is_locked),
            GameItemEnum::HitTarget(hittarget) => Some(hittarget.is_locked),
            GameItemEnum::Generic(_item_type, _generic) => None,
        }
    }

    pub(crate) fn set_locked(&mut self, locked: Option<bool>) {
        match self {
            GameItemEnum::Wall(wall) => {
                if let Some(locked) = locked {
                    wall.is_locked = locked;
                }
            }
            GameItemEnum::Flipper(flipper) => {
                if let Some(locked) = locked {
                    flipper.is_locked = locked;
                }
            }
            GameItemEnum::Timer(timer) => {
                if let Some(locked) = locked {
                    timer.is_locked = locked;
                }
            }
            GameItemEnum::Plunger(plunger) => {
                if let Some(locked) = locked {
                    plunger.is_locked = locked;
                }
            }
            GameItemEnum::TextBox(textbox) => {
                if let Some(locked) = locked {
                    textbox.is_locked = locked;
                }
            }
            GameItemEnum::Bumper(bumper) => {
                if let Some(locked) = locked {
                    bumper.is_locked = locked;
                }
            }
            GameItemEnum::Trigger(trigger) => {
                if let Some(locked) = locked {
                    trigger.is_locked = locked;
                }
            }
            GameItemEnum::Light(light) => {
                if let Some(locked) = locked {
                    light.is_locked = locked;
                }
            }
            GameItemEnum::Kicker(kicker) => {
                if let Some(locked) = locked {
                    kicker.is_locked = locked;
                }
            }
            GameItemEnum::Decal(decal) => {
                if let Some(locked) = locked {
                    decal.is_locked = locked;
                }
            }
            GameItemEnum::Gate(gate) => {
                if let Some(locked) = locked {
                    gate.is_locked = locked;
                }
            }
            GameItemEnum::Spinner(spinner) => {
                if let Some(locked) = locked {
                    spinner.is_locked = locked;
                }
            }
            GameItemEnum::Ramp(ramp) => {
                if let Some(locked) = locked {
                    ramp.is_locked = locked;
                }
            }
            GameItemEnum::Reel(reel) => {
                if let Some(locked) = locked {
                    reel.is_locked = locked;
                }
            }
            GameItemEnum::LightSequencer(lightsequencer) => {
                lightsequencer.is_locked = locked;
            }
            GameItemEnum::Primitive(primitive) => {
                if let Some(locked) = locked {
                    primitive.is_locked = locked;
                }
            }
            GameItemEnum::Flasher(flasher) => {
                if let Some(locked) = locked {
                    flasher.is_locked = locked;
                }
            }
            GameItemEnum::Rubber(rubber) => {
                if let Some(locked) = locked {
                    rubber.is_locked = locked;
                }
            }
            GameItemEnum::HitTarget(hittarget) => {
                if let Some(locked) = locked {
                    hittarget.is_locked = locked;
                }
            }
            GameItemEnum::Generic(_item_type, _generic) => {}
        }
    }

    pub(crate) fn set_editor_layer(&mut self, editor_layer: Option<u32>) {
        match self {
            GameItemEnum::Wall(wall) => {
                if let Some(editor_layer) = editor_layer {
                    wall.editor_layer = editor_layer;
                }
            }
            GameItemEnum::Flipper(flipper) => {
                if let Some(editor_layer) = editor_layer {
                    flipper.editor_layer = editor_layer;
                }
            }
            GameItemEnum::Timer(timer) => {
                if let Some(editor_layer) = editor_layer {
                    timer.editor_layer = editor_layer;
                }
            }
            GameItemEnum::Plunger(plunger) => {
                if let Some(editor_layer) = editor_layer {
                    plunger.editor_layer = editor_layer;
                }
            }
            GameItemEnum::TextBox(textbox) => {
                if let Some(editor_layer) = editor_layer {
                    textbox.editor_layer = editor_layer;
                }
            }
            GameItemEnum::Bumper(bumper) => {
                if let Some(editor_layer) = editor_layer {
                    bumper.editor_layer = editor_layer;
                }
            }
            GameItemEnum::Trigger(trigger) => {
                if let Some(editor_layer) = editor_layer {
                    trigger.editor_layer = editor_layer;
                }
            }
            GameItemEnum::Light(light) => {
                if let Some(editor_layer) = editor_layer {
                    light.editor_layer = editor_layer;
                }
            }
            GameItemEnum::Kicker(kicker) => {
                if let Some(editor_layer) = editor_layer {
                    kicker.editor_layer = editor_layer;
                }
            }
            GameItemEnum::Decal(decal) => {
                if let Some(editor_layer) = editor_layer {
                    decal.editor_layer = editor_layer;
                }
            }
            GameItemEnum::Gate(gate) => {
                if let Some(editor_layer) = editor_layer {
                    gate.editor_layer = editor_layer;
                }
            }
            GameItemEnum::Spinner(spinner) => {
                if let Some(editor_layer) = editor_layer {
                    spinner.editor_layer = editor_layer;
                }
            }
            GameItemEnum::Ramp(ramp) => {
                if let Some(editor_layer) = editor_layer {
                    ramp.editor_layer = editor_layer;
                }
            }
            GameItemEnum::Reel(reel) => {
                if let Some(editor_layer) = editor_layer {
                    reel.editor_layer = editor_layer;
                }
            }
            GameItemEnum::LightSequencer(lightsequencer) => {
                lightsequencer.editor_layer = editor_layer;
            }
            GameItemEnum::Primitive(primitive) => {
                if let Some(editor_layer) = editor_layer {
                    primitive.editor_layer = editor_layer;
                }
            }
            GameItemEnum::Flasher(flasher) => {
                if let Some(editor_layer) = editor_layer {
                    flasher.editor_layer = editor_layer;
                }
            }
            GameItemEnum::Rubber(rubber) => {
                if let Some(editor_layer) = editor_layer {
                    rubber.editor_layer = editor_layer;
                }
            }
            GameItemEnum::HitTarget(hittarget) => {
                if let Some(editor_layer) = editor_layer {
                    hittarget.editor_layer = editor_layer;
                }
            }
            GameItemEnum::Generic(_item_type, _generic) => {}
        }
    }

    pub(crate) fn set_editor_layer_name(&mut self, editor_layer_name: Option<String>) {
        match self {
            GameItemEnum::Wall(wall) => wall.editor_layer_name = editor_layer_name,
            GameItemEnum::Flipper(flipper) => flipper.editor_layer_name = editor_layer_name,
            GameItemEnum::Timer(timer) => timer.editor_layer_name = editor_layer_name,
            GameItemEnum::Plunger(plunger) => plunger.editor_layer_name = editor_layer_name,
            GameItemEnum::TextBox(textbox) => textbox.editor_layer_name = editor_layer_name,
            GameItemEnum::Bumper(bumper) => bumper.editor_layer_name = editor_layer_name,
            GameItemEnum::Trigger(trigger) => trigger.editor_layer_name = editor_layer_name,
            GameItemEnum::Light(light) => light.editor_layer_name = editor_layer_name,
            GameItemEnum::Kicker(kicker) => kicker.editor_layer_name = editor_layer_name,
            GameItemEnum::Decal(decal) => decal.editor_layer_name = editor_layer_name,
            GameItemEnum::Gate(gate) => gate.editor_layer_name = editor_layer_name,
            GameItemEnum::Spinner(spinner) => spinner.editor_layer_name = editor_layer_name,
            GameItemEnum::Ramp(ramp) => ramp.editor_layer_name = editor_layer_name,
            GameItemEnum::Reel(reel) => reel.editor_layer_name = editor_layer_name,
            GameItemEnum::LightSequencer(lightsequencer) => {
                lightsequencer.editor_layer_name = editor_layer_name;
            }
            GameItemEnum::Primitive(primitive) => primitive.editor_layer_name = editor_layer_name,
            GameItemEnum::Flasher(flasher) => flasher.editor_layer_name = editor_layer_name,
            GameItemEnum::Rubber(rubber) => rubber.editor_layer_name = editor_layer_name,
            GameItemEnum::HitTarget(hittarget) => hittarget.editor_layer_name = editor_layer_name,
            GameItemEnum::Generic(_item_type, _generic) => {}
        }
    }

    pub(crate) fn set_editor_layer_visibility(&mut self, editor_layer_visibility: Option<bool>) {
        match self {
            GameItemEnum::Wall(wall) => wall.editor_layer_visibility = editor_layer_visibility,
            GameItemEnum::Flipper(flipper) => {
                flipper.editor_layer_visibility = editor_layer_visibility
            }
            GameItemEnum::Timer(timer) => timer.editor_layer_visibility = editor_layer_visibility,
            GameItemEnum::Plunger(plunger) => {
                plunger.editor_layer_visibility = editor_layer_visibility
            }
            GameItemEnum::TextBox(textbox) => {
                textbox.editor_layer_visibility = editor_layer_visibility
            }
            GameItemEnum::Bumper(bumper) => {
                bumper.editor_layer_visibility = editor_layer_visibility
            }
            GameItemEnum::Trigger(trigger) => {
                trigger.editor_layer_visibility = editor_layer_visibility
            }
            GameItemEnum::Light(light) => light.editor_layer_visibility = editor_layer_visibility,
            GameItemEnum::Kicker(kicker) => {
                kicker.editor_layer_visibility = editor_layer_visibility
            }
            GameItemEnum::Decal(decal) => decal.editor_layer_visibility = editor_layer_visibility,
            GameItemEnum::Gate(gate) => gate.editor_layer_visibility = editor_layer_visibility,
            GameItemEnum::Spinner(spinner) => {
                spinner.editor_layer_visibility = editor_layer_visibility
            }
            GameItemEnum::Ramp(ramp) => ramp.editor_layer_visibility = editor_layer_visibility,
            GameItemEnum::Reel(reel) => reel.editor_layer_visibility = editor_layer_visibility,
            GameItemEnum::LightSequencer(lightsequencer) => {
                lightsequencer.editor_layer_visibility = editor_layer_visibility;
            }
            GameItemEnum::Primitive(primitive) => {
                primitive.editor_layer_visibility = editor_layer_visibility
            }
            GameItemEnum::Flasher(flasher) => {
                flasher.editor_layer_visibility = editor_layer_visibility
            }
            GameItemEnum::Rubber(rubber) => {
                rubber.editor_layer_visibility = editor_layer_visibility
            }
            GameItemEnum::HitTarget(hittarget) => {
                hittarget.editor_layer_visibility = editor_layer_visibility
            }
            GameItemEnum::Generic(_item_type, _generic) => {}
        }
    }
}

impl GameItemEnum {
    pub fn name(&self) -> &str {
        match self {
            GameItemEnum::Wall(wall) => &wall.name,
            GameItemEnum::Flipper(flipper) => flipper.name(),
            GameItemEnum::Timer(timer) => &timer.name,
            GameItemEnum::Plunger(plunger) => &plunger.name,
            GameItemEnum::TextBox(textbox) => &textbox.name,
            GameItemEnum::Bumper(bumper) => bumper.name(),
            GameItemEnum::Trigger(trigger) => &trigger.name,
            GameItemEnum::Light(light) => &light.name,
            GameItemEnum::Kicker(kicker) => &kicker.name,
            GameItemEnum::Decal(decal) => &decal.name,
            GameItemEnum::Gate(gate) => &gate.name,
            GameItemEnum::Spinner(spinner) => &spinner.name,
            GameItemEnum::Ramp(ramp) => &ramp.name,
            GameItemEnum::Reel(reel) => &reel.name,
            GameItemEnum::LightSequencer(lightsequencer) => &lightsequencer.name,
            GameItemEnum::Primitive(primitive) => &primitive.name,
            GameItemEnum::Flasher(flasher) => &flasher.name,
            GameItemEnum::Rubber(rubber) => &rubber.name,
            GameItemEnum::HitTarget(hittarget) => &hittarget.name,
            GameItemEnum::Generic(_item_type, generic) => generic.name(),
        }
    }

    pub fn type_name(&self) -> String {
        match self {
            GameItemEnum::Wall(_) => "Wall".to_string(),
            GameItemEnum::Flipper(_) => "Flipper".to_string(),
            GameItemEnum::Timer(_) => "Timer".to_string(),
            GameItemEnum::Plunger(_) => "Plunger".to_string(),
            GameItemEnum::TextBox(_) => "TextBox".to_string(),
            GameItemEnum::Bumper(_) => "Bumper".to_string(),
            GameItemEnum::Trigger(_) => "Trigger".to_string(),
            GameItemEnum::Light(_) => "Light".to_string(),
            GameItemEnum::Kicker(_) => "Kicker".to_string(),
            GameItemEnum::Decal(_) => "Decal".to_string(),
            GameItemEnum::Gate(_) => "Gate".to_string(),
            GameItemEnum::Spinner(_) => "Spinner".to_string(),
            GameItemEnum::Ramp(_) => "Ramp".to_string(),
            GameItemEnum::Reel(_) => "Reel".to_string(),
            GameItemEnum::LightSequencer(_) => "LightSequencer".to_string(),
            GameItemEnum::Primitive(_) => "Primitive".to_string(),
            GameItemEnum::Flasher(_) => "Flasher".to_string(),
            GameItemEnum::Rubber(_) => "Rubber".to_string(),
            GameItemEnum::HitTarget(_) => "HitTarget".to_string(),
            GameItemEnum::Generic(item_type, _) => format!("Generic_{}", item_type),
        }
    }

    // from type name to type id
    pub fn type_id(type_name: &str) -> u32 {
        match type_name {
            "Wall" => ITEM_TYPE_WALL,
            "Flipper" => ITEM_TYPE_FLIPPER,
            "Timer" => ITEM_TYPE_TIMER,
            "Plunger" => ITEM_TYPE_PLUNGER,
            "TextBox" => ITEM_TYPE_TEXT_BOX,
            "Bumper" => ITEM_TYPE_BUMPER,
            "Trigger" => ITEM_TYPE_TRIGGER,
            "Light" => ITEM_TYPE_LIGHT,
            "Kicker" => ITEM_TYPE_KICKER,
            "Decal" => ITEM_TYPE_DECAL,
            "Gate" => ITEM_TYPE_GATE,
            "Spinner" => ITEM_TYPE_SPINNER,
            "Ramp" => ITEM_TYPE_RAMP,
            "Table" => ITEM_TYPE_TABLE,
            "LightCenter" => ITEM_TYPE_LIGHT_CENTER,
            "DragPoint" => ITEM_TYPE_DRAG_POINT,
            "Collection" => ITEM_TYPE_COLLECTION,
            "Reel" => ITEM_TYPE_REEL,
            "LightSequencer" => ITEM_TYPE_LIGHT_SEQUENCER,
            "Primitive" => ITEM_TYPE_PRIMITIVE,
            "Flasher" => ITEM_TYPE_FLASHER,
            "Rubber" => ITEM_TYPE_RUBBER,
            "HitTarget" => ITEM_TYPE_HIT_TARGET,
            _ => unimplemented!("type_id for {}", type_name),
        }
    }
}

// Item types:
// 0: Wall
// 1: Flipper
// 2: Timer
// 3: Plunger
// 4: Text box
// 5: Bumper
// 6: Trigger
// 7: Light
// 8: Kicker
// 9: Decal
// 10: Gate
// 11: Spinner
// 12: Ramp
// 13: Table
// 14: Light Center
// 15: Drag Point (does this make sense on it's own?)
// 16: Collection
// 17: Reel
// 18: Light sequencer
// 19: Primitive
// 20: Flasher
// 21: Rubber
// 22: Hit Target

const ITEM_TYPE_WALL: u32 = 0;
const ITEM_TYPE_FLIPPER: u32 = 1;
const ITEM_TYPE_TIMER: u32 = 2;
const ITEM_TYPE_PLUNGER: u32 = 3;
const ITEM_TYPE_TEXT_BOX: u32 = 4;
const ITEM_TYPE_BUMPER: u32 = 5;
const ITEM_TYPE_TRIGGER: u32 = 6;
const ITEM_TYPE_LIGHT: u32 = 7;
const ITEM_TYPE_KICKER: u32 = 8;
const ITEM_TYPE_DECAL: u32 = 9;
const ITEM_TYPE_GATE: u32 = 10;
const ITEM_TYPE_SPINNER: u32 = 11;
const ITEM_TYPE_RAMP: u32 = 12;
const ITEM_TYPE_TABLE: u32 = 13;
const ITEM_TYPE_LIGHT_CENTER: u32 = 14;
const ITEM_TYPE_DRAG_POINT: u32 = 15;
const ITEM_TYPE_COLLECTION: u32 = 16;
const ITEM_TYPE_REEL: u32 = 17;
const ITEM_TYPE_LIGHT_SEQUENCER: u32 = 18;
const ITEM_TYPE_PRIMITIVE: u32 = 19;
const ITEM_TYPE_FLASHER: u32 = 20;
const ITEM_TYPE_RUBBER: u32 = 21;
const ITEM_TYPE_HIT_TARGET: u32 = 22;

// const TYPE_NAMES: [&str; 23] = [
//     "Wall",
//     "Flipper",
//     "Timer",
//     "Plunger",
//     "Text",
//     "Bumper",
//     "Trigger",
//     "Light",
//     "Kicker",
//     "Decal",
//     "Gate",
//     "Spinner",
//     "Ramp",
//     "Table",
//     "LightCenter",
//     "DragPoint",
//     "Collection",
//     "DispReel",
//     "LightSeq",
//     "Prim",
//     "Flasher",
//     "Rubber",
//     "Target",
// ];

pub fn read(input: &[u8]) -> GameItemEnum {
    let mut reader = BiffReader::new(input);
    let item_type = reader.get_u32_no_remaining_update();
    match item_type {
        ITEM_TYPE_WALL => GameItemEnum::Wall(wall::Wall::biff_read(&mut reader)),
        ITEM_TYPE_FLIPPER => GameItemEnum::Flipper(flipper::Flipper::biff_read(&mut reader)),
        ITEM_TYPE_TIMER => GameItemEnum::Timer(timer::Timer::biff_read(&mut reader)),
        ITEM_TYPE_PLUNGER => GameItemEnum::Plunger(plunger::Plunger::biff_read(&mut reader)),
        ITEM_TYPE_TEXT_BOX => GameItemEnum::TextBox(textbox::TextBox::biff_read(&mut reader)),
        ITEM_TYPE_BUMPER => GameItemEnum::Bumper(bumper::Bumper::biff_read(&mut reader)),
        ITEM_TYPE_TRIGGER => GameItemEnum::Trigger(trigger::Trigger::biff_read(&mut reader)),
        ITEM_TYPE_LIGHT => GameItemEnum::Light(light::Light::biff_read(&mut reader)),
        ITEM_TYPE_KICKER => GameItemEnum::Kicker(kicker::Kicker::biff_read(&mut reader)),
        ITEM_TYPE_DECAL => GameItemEnum::Decal(decal::Decal::biff_read(&mut reader)),
        ITEM_TYPE_GATE => GameItemEnum::Gate(gate::Gate::biff_read(&mut reader)),
        ITEM_TYPE_SPINNER => GameItemEnum::Spinner(spinner::Spinner::biff_read(&mut reader)),
        ITEM_TYPE_RAMP => GameItemEnum::Ramp(ramp::Ramp::biff_read(&mut reader)),
        ITEM_TYPE_TABLE => panic!("Table should not be read on it's own"),
        ITEM_TYPE_LIGHT_CENTER => panic!("LightCenter should not be read on it's own"),
        ITEM_TYPE_DRAG_POINT => panic!("DragPoint should not be read on it's own"),
        ITEM_TYPE_COLLECTION => panic!("Collection should not be read on it's own"),
        ITEM_TYPE_REEL => GameItemEnum::Reel(reel::Reel::biff_read(&mut reader)),
        ITEM_TYPE_LIGHT_SEQUENCER => {
            GameItemEnum::LightSequencer(lightsequencer::LightSequencer::biff_read(&mut reader))
        }
        ITEM_TYPE_PRIMITIVE => {
            GameItemEnum::Primitive(primitive::Primitive::biff_read(&mut reader))
        }
        ITEM_TYPE_FLASHER => GameItemEnum::Flasher(flasher::Flasher::biff_read(&mut reader)),
        ITEM_TYPE_RUBBER => GameItemEnum::Rubber(rubber::Rubber::biff_read(&mut reader)),
        ITEM_TYPE_HIT_TARGET => {
            GameItemEnum::HitTarget(hittarget::HitTarget::biff_read(&mut reader))
        }
        other_item_type => {
            GameItemEnum::Generic(other_item_type, generic::Generic::biff_read(&mut reader))
        }
    }
}

pub(crate) fn write(gameitem: &GameItemEnum) -> Vec<u8> {
    match gameitem {
        GameItemEnum::Wall(wall) => write_with_type(ITEM_TYPE_WALL, wall),
        GameItemEnum::Flipper(flipper) => write_with_type(ITEM_TYPE_FLIPPER, flipper),
        GameItemEnum::Timer(timer) => write_with_type(ITEM_TYPE_TIMER, timer),
        GameItemEnum::Plunger(plunger) => write_with_type(ITEM_TYPE_PLUNGER, plunger),
        GameItemEnum::TextBox(textbox) => write_with_type(ITEM_TYPE_TEXT_BOX, textbox),
        GameItemEnum::Bumper(bumper) => write_with_type(ITEM_TYPE_BUMPER, bumper),
        GameItemEnum::Trigger(trigger) => write_with_type(ITEM_TYPE_TRIGGER, trigger),
        GameItemEnum::Light(light) => write_with_type(ITEM_TYPE_LIGHT, light),
        GameItemEnum::Kicker(kicker) => write_with_type(ITEM_TYPE_KICKER, kicker),
        GameItemEnum::Decal(decal) => write_with_type(ITEM_TYPE_DECAL, decal),
        GameItemEnum::Gate(gate) => write_with_type(ITEM_TYPE_GATE, gate),
        GameItemEnum::Spinner(spinner) => write_with_type(ITEM_TYPE_SPINNER, spinner),
        GameItemEnum::Ramp(ramp) => write_with_type(ITEM_TYPE_RAMP, ramp),
        GameItemEnum::Reel(reel) => write_with_type(ITEM_TYPE_REEL, reel),
        GameItemEnum::LightSequencer(lightsequencer) => {
            write_with_type(ITEM_TYPE_LIGHT_SEQUENCER, lightsequencer)
        }
        GameItemEnum::Primitive(primitive) => write_with_type(ITEM_TYPE_PRIMITIVE, primitive),
        GameItemEnum::Flasher(flasher) => write_with_type(ITEM_TYPE_FLASHER, flasher),
        GameItemEnum::Rubber(rubber) => write_with_type(ITEM_TYPE_RUBBER, rubber),
        GameItemEnum::HitTarget(hittarget) => write_with_type(ITEM_TYPE_HIT_TARGET, hittarget),
        // GameItemEnum::Generic(item_type, generic) => write_with_type(*item_type, generic),
        _ => {
            unimplemented!("write gameitem {:?}", gameitem);
            //vec![]
        }
    }
}

fn write_with_type<T: BiffWrite>(item_type: u32, item: &T) -> Vec<u8> {
    let mut writer = BiffWriter::new();
    writer.write_u32(item_type);
    item.biff_write(&mut writer);
    writer.get_data().to_vec()
}
