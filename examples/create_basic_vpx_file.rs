use std::path::Path;
use vpin::vpx;
use vpin::vpx::color::Color;
use vpin::vpx::gameitem::bumper::Bumper;
use vpin::vpx::gameitem::flipper::Flipper;
use vpin::vpx::gameitem::plunger::Plunger;
use vpin::vpx::gameitem::vertex2d::Vertex2D;
use vpin::vpx::gameitem::GameItemEnum;
use vpin::vpx::material::Material;
use vpin::vpx::VPX;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut vpx = VPX::default();

    // playfield material
    let material = Material {
        name: "Playfield".to_string(),
        // material defaults to purple
        base_color: Color::from_rgb(0x966F33), // Wood
        ..Default::default()
    };
    vpx.gamedata.materials = Some(vec![material]);

    // black background (default is bluish gray)
    vpx.gamedata.backdrop_color = Color::from_rgb(0x060606); // Dark Gray
    vpx.gamedata.playfield_material = "Playfield".to_string();

    // add a plunger
    let plunger = Plunger {
        name: "Plunger".to_string(),
        center: Vertex2D {
            x: 898.027,
            y: 2105.312,
        },
        ..Default::default()
    };

    vpx.add_game_item(GameItemEnum::Plunger(plunger));

    // add a bumper in the center of the playfield
    let bumper = Bumper {
        name: "Bumper1".to_string(),
        center: Vertex2D {
            x: (vpx.gamedata.left + vpx.gamedata.right) / 2.,
            y: (vpx.gamedata.top + vpx.gamedata.bottom) / 2.,
        },
        ..Default::default()
    };

    vpx.add_game_item(GameItemEnum::Bumper(bumper));

    // add 2 flippers
    let flipper_left = Flipper {
        name: "LeftFlipper".to_string(),
        center: Vertex2D {
            x: 278.2138,
            y: 1803.271,
        },
        start_angle: 120.5,
        end_angle: 70.,
        ..Default::default()
    };

    vpx.add_game_item(GameItemEnum::Flipper(flipper_left));

    let flipper_right = Flipper {
        name: "RightFlipper".to_string(),
        center: Vertex2D {
            x: 595.869,
            y: 1803.271,
        },
        start_angle: -120.5,
        end_angle: -70.,
        ..Default::default()
    };

    vpx.add_game_item(GameItemEnum::Flipper(flipper_right));

    // add a script
    let script = std::fs::read_to_string(Path::new("examples").join("basic.vbs"))?;
    vpx.set_script(script);

    vpx::write("basic.vpx", &vpx)?;

    println!("Wrote basic.vpx.");
    println!(r#"Try running it with "VPinballX_GL -play basic.vpx""#);
    Ok(())
}
