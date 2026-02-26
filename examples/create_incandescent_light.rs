/// Creates a minimal VPX table with a single incandescent Halo light
/// in the center of the playfield.
///
/// The light uses:
/// - Halo/Bulb mode with a visible 3D bulb mesh
/// - Incandescent fading for realistic filament warm-up/cool-down
/// - Slow fade speed so the effect is clearly visible
///
/// Run with: `cargo run --example create_incandescent_light`
/// Then open in VPinball: `VPinballX_GL -play incandescent_light.vpx`
use std::f32::consts::PI;
use std::path::Path;

use vpin::vpx;
use vpin::vpx::VPX;
use vpin::vpx::color::Color;
use vpin::vpx::gamedata::ViewLayoutMode;
use vpin::vpx::gameitem::GameItemEnum;
use vpin::vpx::gameitem::dragpoint::DragPoint;
use vpin::vpx::gameitem::light::{Fader, Light, ShadowMode};
use vpin::vpx::gameitem::vertex2d::Vertex2D;
use vpin::vpx::gameitem::wall::Wall;
use vpin::vpx::material::Material;
use vpin::vpx::units::mm_to_vpu;

/// Create a wall segment between two points
fn wall_segment(name: &str, x1: f32, y1: f32, x2: f32, y2: f32, material: &str) -> Wall {
    Wall {
        name: name.to_string(),
        height_top: 50.0,
        top_material: material.to_string(),
        side_material: material.to_string(),
        drag_points: vec![
            DragPoint {
                x: x1,
                y: y1,
                ..Default::default()
            },
            DragPoint {
                x: x2,
                y: y2,
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

/// Generate circular drag points for a light insert
fn circle_drag_points(
    center_x: f32,
    center_y: f32,
    radius: f32,
    num_points: usize,
) -> Vec<DragPoint> {
    (0..num_points)
        .map(|i| {
            let angle = (i as f32 / num_points as f32) * 2.0 * PI;
            DragPoint {
                x: center_x + radius * angle.cos(),
                y: center_y + radius * angle.sin(),
                z: 0.0,
                smooth: true,
                has_auto_texture: true,
                ..Default::default()
            }
        })
        .collect()
}

/// Add boundary walls around the playfield edges.
fn add_playfield_walls(vpx: &mut VPX) {
    let left = vpx.gamedata.left;
    let right = vpx.gamedata.right;
    let top = vpx.gamedata.top;
    let bottom = vpx.gamedata.bottom;

    vpx.add_game_item(GameItemEnum::Wall(wall_segment(
        "WallLeft", left, top, left, bottom, "Wood",
    )));
    vpx.add_game_item(GameItemEnum::Wall(wall_segment(
        "WallRight",
        right,
        top,
        right,
        bottom,
        "Wood",
    )));
    vpx.add_game_item(GameItemEnum::Wall(wall_segment(
        "WallTop", left, top, right, top, "Wood",
    )));
    vpx.add_game_item(GameItemEnum::Wall(wall_segment(
        "WallBottom",
        left,
        bottom,
        right,
        bottom,
        "Wood",
    )));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    // Start from a default table
    let mut vpx = VPX::default();

    let mut playfield_mat = Material::default();
    playfield_mat.name = "Playfield".to_string();
    playfield_mat.base_color = Color::from_rgb(0x202020);

    // Wood-colored wall material
    let mut wall_mat = Material::default();
    wall_mat.name = "Wood".to_string();
    wall_mat.base_color = Color::from_rgb(0x966F33);

    vpx.gamedata.materials = Some(vec![playfield_mat, wall_mat]);
    vpx.gamedata.playfield_material = "Playfield".to_string();

    vpx.gamedata.backdrop_color = Color::from_rgb(0x040404); // Dark Gray
    vpx.gamedata.light_ambient = Color::from_rgb(0x000000);
    vpx.gamedata.light0_emission = Color::from_rgb(0xFFFFF0);

    // Camera — 45° inclination gives a typical desktop pinball viewing angle
    vpx.gamedata.bg_view_mode_desktop = Some(ViewLayoutMode::Legacy);
    vpx.gamedata.bg_inclination_desktop = 45.0;
    vpx.gamedata.bg_fov_desktop = 45.0;
    vpx.gamedata.bg_offset_y_desktop = 30.0;
    vpx.gamedata.bg_offset_z_desktop = -200.0;

    //vpx.gamedata.bloom_strength = 10.0;
    vpx.gamedata.use_ao = Some(1);
    vpx.gamedata.use_ssr = Some(1);
    vpx.gamedata.ssr_scale = Some(0.5);

    // VPinball's legacy camera mode uses FitCameraToVertices to frame the view
    // around all game items. Without any items the camera has nothing to fit and
    // the table renders as a blank screen. Four boundary walls solve this and
    // also give the table a proper frame.
    add_playfield_walls(&mut vpx);

    // Center of the playfield
    let cx = (vpx.gamedata.left + vpx.gamedata.right) / 2.0;
    let cy = (vpx.gamedata.top + vpx.gamedata.bottom) / 2.0;

    let light_radius = mm_to_vpu(200.0);

    // Add a Halo light with incandescent fading
    let light = Light {
        name: "CenterLight".to_string(),
        center: Vertex2D { x: cx, y: cy },

        // Warm white color at the center (2700K incandescent)
        color: Color::rgb(255, 209, 137),
        // Deeper amber at the edges for a warm falloff gradient
        color2: Color::rgb(255, 120, 40),
        intensity: 15.0,
        falloff_radius: light_radius,
        falloff_power: 3.0,

        // Halo mode — renders as a radial glow, ignores the image field
        is_bulb_light: true,
        visible: Some(true),

        // Incandescent fading — physically-based filament simulation
        fader: Some(Fader::Incandescent),
        fade_speed_up: 0.1,    // slow warm-up
        fade_speed_down: 0.05, // even slower cool-down (visible red shift)

        // Show the 3D bulb and socket mesh
        show_bulb_mesh: true,
        has_static_bulb_mesh: Some(true),
        mesh_radius: 200.0,
        bulb_halo_height: 10.0,
        bulb_modulate_vs_add: 0.5,

        // Enable ball shadow casting from this light
        shadows: Some(ShadowMode::RaytracedBallShadows),
        show_reflection_on_ball: true,
        transmission_scale: 0.5,

        state: Some(2.0),     // blinking (float version for 10.8+)
        blink_interval: 1500, // 1.0 seconds per step

        // The drag point polygon defines the rendered glow shape. For a
        // realistic bulb light, the polygon must be at least as large as the
        // falloff radius so the glow fades naturally to zero. If the polygon
        // is smaller, the glow is clipped at the polygon edge creating an
        // unnatural hard border (which is fine for inserts but not for bulbs).
        drag_points: circle_drag_points(cx, cy, light_radius, 6),
        ..Default::default()
    };
    vpx.add_game_item(GameItemEnum::Light(light));

    // Test: replace the original script with a minimal one
    vpx.set_script(
        r#"Option Explicit
Randomize

Sub Table1_Init
    debug.print "Incandescent light demo"
End Sub
"#
        .to_string(),
    );

    vpx::write(Path::new("incandescent_light.vpx"), &vpx)?;

    println!("Wrote incandescent_light.vpx");
    println!(r#"Try running it with "VPinballX_GL -play incandescent_light.vpx""#);
    Ok(())
}
