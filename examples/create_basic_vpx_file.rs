use std::path::Path;
use vpin::vpx;
use vpin::vpx::color::ColorNoAlpha;
use vpin::vpx::gameitem::bumper::Bumper;
use vpin::vpx::gameitem::flipper::Flipper;
use vpin::vpx::gameitem::GameItemEnum;
use vpin::vpx::material::Material;
use vpin::vpx::VPX;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut vpx = VPX::default();

    // playfield material
    let mut material = Material::default();
    material.name = "Playfield".to_string();
    // material defaults to purple
    material.base_color = ColorNoAlpha::from_rgb(0x966F33); // Wood
    vpx.gamedata.materials = Some(vec![material]);

    // black background (default is bluish gray)
    vpx.gamedata.backdrop_color = ColorNoAlpha::from_rgb(0x060606); // Dark Gray
    vpx.gamedata.playfield_material = "Playfield".to_string();

    // add a plunger
    let mut plunger = vpx::gameitem::plunger::Plunger::default();
    plunger.name = "Plunger".to_string();
    plunger.center.x = 898.027;
    plunger.center.y = 2105.312;
    vpx.add_game_item(GameItemEnum::Plunger(plunger));

    // add a bumper in the center of the playfield
    let mut bumper = Bumper::default();
    bumper.name = "Bumper1".to_string();
    bumper.center.x = (vpx.gamedata.left + vpx.gamedata.right) / 2.;
    bumper.center.y = (vpx.gamedata.top + vpx.gamedata.bottom) / 2.;
    vpx.add_game_item(GameItemEnum::Bumper(bumper));

    // add 2 flippers
    let mut flipper_left = Flipper::default();
    flipper_left.name = "LeftFlipper".to_string();
    flipper_left.center.x = 278.2138;
    flipper_left.center.y = 1803.271;
    flipper_left.start_angle = 120.5;
    flipper_left.end_angle = 70.;
    vpx.add_game_item(GameItemEnum::Flipper(flipper_left));

    let mut flipper_right = Flipper::default();
    flipper_right.name = "RightFlipper".to_string();
    flipper_right.center.x = 595.869;
    flipper_right.center.y = 1803.271;
    flipper_right.start_angle = -120.5;
    flipper_right.end_angle = -70.;
    vpx.add_game_item(GameItemEnum::Flipper(flipper_right));

    // add a script
    let script = std::fs::read_to_string(Path::new("examples").join("basic.vbs"))?;
    vpx.set_script(script);

    vpx::write("basic.vpx", &vpx)?;

    println!("Wrote basic.vpx.");
    println!(r#"Try running it with "VPinballX_GL -play basic.vpx""#);
    Ok(())
}
