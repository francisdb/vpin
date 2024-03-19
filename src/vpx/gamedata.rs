#![allow(dead_code)]

use super::{
    biff::{self, BiffReader, BiffWriter},
    model::StringWithEncoding,
    version::Version,
};
use crate::vpx::biff::{BiffRead, BiffWrite};
use crate::vpx::color::{Color, ColorJson};
use crate::vpx::material::{Material, SaveMaterial, SavePhysicsMaterial};
use crate::vpx::math::{dequantize_u8, quantize_u8};
use crate::vpx::renderprobe::RenderProbeWithGarbage;
use bytes::{Buf, BufMut, BytesMut};
use serde::{Deserialize, Serialize};

pub const VIEW_LAYOUT_MODE_LEGACY: u32 = 0; // All tables before 10.8 used a viewer position relative to a fitting of a set of bounding vertices (not all parts) with a standard perspective projection skewed by a layback angle
pub const VIEW_LAYOUT_MODE_CAMERA: u32 = 1; // Position viewer relative to the bottom center of the table, use a standard camera perspective projection, replace layback by a frustrum offset
pub const VIEW_LAYOUT_MODE_WINDOW: u32 = 2; // Position viewer relative to the bottom center of the table, use an oblique surface (re)projection (needs some postprocess to avoid distortion)

// TODO switch to a array of 3 view modes like in the original code
#[derive(Debug, PartialEq)]
pub struct ViewSetup {
    // ViewLayoutMode mMode = VLM_LEGACY;

    // // Overall scene scale
    // float mSceneScaleZ = 1.0f;

    // // View position (relative to table bounds for legacy mode, relative to the bottom center of the table for others)
    // float mViewX = 0.f;
    // float mViewY = CMTOVPU(20.f);
    // float mViewZ = CMTOVPU(70.f);
    // float mLookAt = 0.25f; // Look at expressed as a camera inclination for legacy, or a percent of the table height, starting from bottom (0.25 is around top of slingshots)

    // // Viewport adjustments
    // float mViewportRotation = 0.f;
    // float mSceneScaleX = 1.0f;
    // float mSceneScaleY = 1.0f;

    // // View properties
    // float mFOV = 45.0f; // Camera & Legacy: Field of view, in degrees
    // float mLayback = 0.0f; // Legacy: A skewing angle that deform the table to make it look 'good'
    // float mViewHOfs = 0.0f; // Camera & Window: horizontal frustrum offset
    // float mViewVOfs = 0.0f; // Camera & Window: vertical frustrum offset

    // // Magic Window mode properties
    // float mWindowTopXOfs = 0.0f; // Upper window border offset from left and right table bounds
    // float mWindowTopYOfs = 0.0f; // Upper window border Y coordinate, relative to table top
    // float mWindowTopZOfs = CMTOVPU(20.0f); // Upper window border Z coordinate, relative to table playfield Z
    // float mWindowBottomXOfs = 0.0f; // Lower window border offset from left and right table bounds
    // float mWindowBottomYOfs = 0.0f; // Lower window border Y coordinate, relative to table bottom
    // float mWindowBottomZOfs = CMTOVPU(7.5f); // Lower window border Z coordinate, relative to table playfield Z
    pub mode: u32,
}

impl ViewSetup {
    pub fn new() -> Self {
        ViewSetup {
            mode: VIEW_LAYOUT_MODE_LEGACY,
        }
    }

    fn default() -> Self {
        ViewSetup::new()
    }
}

#[derive(Debug, PartialEq)]
pub struct GameData {
    pub left: f32,                                                 // LEFT 1
    pub top: f32,                                                  // TOPX 2
    pub right: f32,                                                // RGHT 3
    pub bottom: f32,                                               // BOTM 4
    pub clmo: Option<u32>,                                         // CLMO added in 10.8.0?
    pub bg_view_mode_desktop: Option<u32>,                         // VSM0 added in 10.8.x
    pub bg_rotation_desktop: f32,                                  // ROTA 5
    pub bg_inclination_desktop: f32,                               // INCL 6
    pub bg_layback_desktop: f32,                                   // LAYB 7
    pub bg_fov_desktop: f32,                                       // FOVX 8
    pub bg_offset_x_desktop: f32,                                  // XLTX 9
    pub bg_offset_y_desktop: f32,                                  // XLTY 10
    pub bg_offset_z_desktop: f32,                                  // XLTZ 11
    pub bg_scale_x_desktop: f32,                                   // SCLX 12
    pub bg_scale_y_desktop: f32,                                   // SCLY 13
    pub bg_scale_z_desktop: f32,                                   // SCLZ 14
    pub bg_enable_fss: Option<bool>,                               // EFSS 15 (added in 10.?)
    pub bg_view_horizontal_offset_desktop: Option<f32>,            // HOF0 added in 10.8.x
    pub bg_view_vertical_offset_desktop: Option<f32>,              // VOF0 added in 10.8.x
    pub bg_window_top_x_offset_desktop: Option<f32>,               // WTX0 added in 10.8.x
    pub bg_window_top_y_offset_desktop: Option<f32>,               // WTY0 added in 10.8.x
    pub bg_window_top_z_offset_desktop: Option<f32>,               // WTZ0 added in 10.8.x
    pub bg_window_bottom_x_offset_desktop: Option<f32>,            // WBX0 added in 10.8.x
    pub bg_window_bottom_y_offset_desktop: Option<f32>,            // WBY0 added in 10.8.x
    pub bg_window_bottom_z_offset_desktop: Option<f32>,            // WBZ0 added in 10.8.x
    pub bg_view_mode_fullscreen: Option<u32>,                      // VSM1 added in 10.8.x
    pub bg_rotation_fullscreen: f32,                               // ROTF 16
    pub bg_inclination_fullscreen: f32,                            // INCF 17
    pub bg_layback_fullscreen: f32,                                // LAYF 18
    pub bg_fov_fullscreen: f32,                                    // FOVF 19
    pub bg_offset_x_fullscreen: f32,                               // XLFX 20
    pub bg_offset_y_fullscreen: f32,                               // XLFY 21
    pub bg_offset_z_fullscreen: f32,                               // XLFZ 22
    pub bg_scale_x_fullscreen: f32,                                // SCFX 23
    pub bg_scale_y_fullscreen: f32,                                // SCFY 24
    pub bg_scale_z_fullscreen: f32,                                // SCFZ 25
    pub bg_view_horizontal_offset_fullscreen: Option<f32>,         // HOF1 added in 10.8.x
    pub bg_view_vertical_offset_fullscreen: Option<f32>,           // VOF1 added in 10.8.x
    pub bg_window_top_x_offset_fullscreen: Option<f32>,            // WTX1 added in 10.8.x
    pub bg_window_top_y_offset_fullscreen: Option<f32>,            // WTY1 added in 10.8.x
    pub bg_window_top_z_offset_fullscreen: Option<f32>,            // WTZ1 added in 10.8.x
    pub bg_window_bottom_x_offset_fullscreen: Option<f32>,         // WBX1 added in 10.8.x
    pub bg_window_bottom_y_offset_fullscreen: Option<f32>,         // WBY1 added in 10.8.x
    pub bg_window_bottom_z_offset_fullscreen: Option<f32>,         // WBZ1 added in 10.8.x
    pub bg_view_mode_full_single_screen: Option<u32>,              // VSM2 added in 10.8.x
    pub bg_rotation_full_single_screen: Option<f32>,               // ROFS 26 (added in 10.?)
    pub bg_inclination_full_single_screen: Option<f32>,            // INFS 27 (added in 10.?)
    pub bg_layback_full_single_screen: Option<f32>,                // LAFS 28 (added in 10.?)
    pub bg_fov_full_single_screen: Option<f32>,                    // FOFS 29 (added in 10.?)
    pub bg_offset_x_full_single_screen: Option<f32>,               // XLXS 30 (added in 10.?)
    pub bg_offset_y_full_single_screen: Option<f32>,               // XLYS 31 (added in 10.?)
    pub bg_offset_z_full_single_screen: Option<f32>,               // XLZS 32 (added in 10.?)
    pub bg_scale_x_full_single_screen: Option<f32>,                // SCXS 33 (added in 10.?)
    pub bg_scale_y_full_single_screen: Option<f32>,                // SCYS 34 (added in 10.?)
    pub bg_scale_z_full_single_screen: Option<f32>,                // SCZS 35 (added in 10.?)
    pub bg_view_horizontal_offset_full_single_screen: Option<f32>, // HOF2 added in 10.8.x
    pub bg_view_vertical_offset_full_single_screen: Option<f32>,   // VOF2 added in 10.8.x
    pub bg_window_top_x_offset_full_single_screen: Option<f32>,    // WTX2 added in 10.8.x
    pub bg_window_top_y_offset_full_single_screen: Option<f32>,    // WTY2 added in 10.8.x
    pub bg_window_top_z_offset_full_single_screen: Option<f32>,    // WTZ2 added in 10.8.x
    pub bg_window_bottom_x_offset_full_single_screen: Option<f32>, // WBX2 added in 10.8.x
    pub bg_window_bottom_y_offset_full_single_screen: Option<f32>, // WBY2 added in 10.8.x
    pub bg_window_bottom_z_offset_full_single_screen: Option<f32>, // WBZ2 added in 10.8.x
    pub override_physics: u32,                                     // ORRP 36
    pub override_physics_flipper: Option<bool>,                    // ORPF 37 added in ?
    pub gravity: f32,                                              // GAVT 38
    pub friction: f32,                                             // FRCT 39
    pub elasticity: f32,                                           // ELAS 40
    pub elastic_falloff: f32,                                      // ELFA 41
    pub scatter: f32,                                              // PFSC 42
    pub default_scatter: f32,                                      // SCAT 43
    pub nudge_time: f32,                                           // NDGT 44
    pub plunger_normalize: u32,                                    // MPGC 45
    pub plunger_filter: bool,                                      // MPDF 46
    pub physics_max_loops: u32,                                    // PHML 47
    pub render_em_reels: bool,                                     // REEL 48
    pub render_decals: bool,                                       // DECL 49
    pub offset_x: f32,                                             // OFFX 50
    pub offset_y: f32,                                             // OFFY 51
    pub zoom: f32,                                                 // ZOOM 52
    pub angle_tilt_max: f32,                                       // SLPX 53
    pub angle_tilt_min: f32,                                       // SLOP 54
    pub stereo_max_separation: f32,                                // MAXS 55
    pub stereo_zero_parallax_displacement: f32,                    // ZPD 56
    pub stereo_offset: Option<f32>,                                // STO 57 (was missing in  10.01)
    pub overwrite_global_stereo3d: bool,                           // OGST 58
    pub image: String,                                             // IMAG 59
    pub backglass_image_full_desktop: String,                      // BIMG 60
    pub backglass_image_full_fullscreen: String,                   // BIMF 61
    pub backglass_image_full_single_screen: Option<String>,        // BIMS 62 (added in 10.?)
    pub image_backdrop_night_day: bool,                            // BIMN 63
    pub image_color_grade: String,                                 // IMCG 64
    pub ball_image: String,                                        // BLIM 65
    pub ball_spherical_mapping: Option<bool>,                      // BLSM (added in 10.8)
    pub ball_image_front: String,                                  // BLIF 66
    pub env_image: Option<String>,                                 // EIMG 67 (was missing in 10.01)
    pub notes: Option<String>,                                     // NOTX 67.5 (added in 10.7)
    pub screen_shot: String,                                       // SSHT 68
    pub display_backdrop: bool,                                    // FBCK 69
    pub glass_top_height: f32,                                     // GLAS 70
    pub glass_bottom_height: Option<f32>,                          // GLAB 70.5 (added in 10.8)
    pub table_height: Option<f32>,                                 // TBLH 71 (optional in 10.8)
    pub playfield_material: String,                                // PLMA 72
    pub backdrop_color: u32,                                       // BCLR 73 (color bgr)
    pub global_difficulty: f32,                                    // TDFT 74
    pub light_ambient: u32,                                        // LZAM 75 (color)
    pub light0_emission: u32,                                      // LZDI 76 (color)
    pub light_height: f32,                                         // LZHI 77
    pub light_range: f32,                                          // LZRA 78
    pub light_emission_scale: f32,                                 // LIES 79
    pub env_emission_scale: f32,                                   // ENES 80
    pub global_emission_scale: f32,                                // GLES 81
    pub ao_scale: f32,                                             // AOSC 82
    pub ssr_scale: Option<f32>,                                    // SSSC 83 (added in 10.?)
    pub table_sound_volume: f32,                                   // SVOL 84
    pub table_music_volume: f32,                                   // MVOL 85
    pub table_adaptive_vsync: Option<i32>, // AVSY 86 (became optional in 10.8)
    pub use_reflection_for_balls: Option<i32>, // BREF 87 (became optional in 10.8)
    pub brst: Option<i32>,                 // BRST (in use in 10.01)
    pub playfield_reflection_strength: f32, // PLST 88
    pub use_trail_for_balls: Option<i32>,  // BTRA 89 (became optional in 10.8)
    pub ball_decal_mode: bool,             // BDMO 90
    pub ball_playfield_reflection_strength: Option<f32>, // BPRS 91 (was missing in 10.01)
    pub default_bulb_intensity_scale_on_ball: Option<f32>, // DBIS 92 (added in 10.?)
    /// this has a special quantization,
    /// See [`Self::get_ball_trail_strength`] and [`Self::set_ball_trail_strength`]
    pub ball_trail_strength: Option<u32>, // BTST 93 (became optional in 10.8)
    pub user_detail_level: Option<u32>,    // ARAC 94 (became optional in 10.8)
    pub overwrite_global_detail_level: Option<bool>, // OGAC 95 (became optional in 10.8)
    pub overwrite_global_day_night: Option<bool>, // OGDN 96 (became optional in 10.8)
    pub show_grid: bool,                   // GDAC 97
    pub reflect_elements_on_playfield: Option<bool>, // REOP 98 (became optional in 10.8)
    pub use_aal: Option<i32>,              // UAAL 99 (became optional in 10.8)
    pub use_fxaa: Option<i32>,             // UFXA 100 (became optional in 10.8)
    pub use_ao: Option<i32>,               // UAOC 101 (became optional in 10.8)
    pub use_ssr: Option<i32>,              // USSR 102 (added in 10.?)
    pub tone_mapper: Option<i32>,          // TMAP 102.5 (added in 10.8)
    pub bloom_strength: f32,               // BLST 103
    pub materials_size: u32,               // MASI 104
    /// Legacy material saving for backward compatibility
    pub materials_old: Vec<SaveMaterial>, // MATE 105 (only for <10.8)
    /// Legacy material saving for backward compatibility
    pub materials_physics_old: Option<Vec<SavePhysicsMaterial>>, // PHMA 106 (only for <10.8, added in 10.?)
    /// 10.8+ material saving (this format supports new properties, and can be extended in future versions, and does not perform quantizations)
    pub materials: Option<Vec<Material>>, // MATR (added in 10.8)
    pub render_probes: Option<Vec<RenderProbeWithGarbage>>, // RPRB (added in 10.8)
    pub gameitems_size: u32,                                // SEDT 107
    pub sounds_size: u32,                                   // SSND 108
    pub images_size: u32,                                   // SIMG 109
    pub fonts_size: u32,                                    // SFNT 110
    pub collections_size: u32,                              // SCOL 111
    pub name: String,                                       // NAME 112
    pub custom_colors: [Color; 16],                         //[Color; 16], // CCUS 113
    pub protection_data: Option<Vec<u8>>,                   // SECB (removed in ?)
    pub code: StringWithEncoding,                           // CODE 114
    pub is_locked: Option<bool>, // TLCK (added in 10.8 for tournament mode?)
    // This is a bit of a hack because we want reproducible builds.
    // 10.8.0 beta 1-4 had EFSS at the old location, but it was moved to the new location in beta 5
    // Some tables were released with these old betas, so we need to support both locations to be 100% reproducing the orignal table
    // and it's MAC hash.
    pub is_10_8_0_beta1_to_beta4: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct GameDataJson {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub clmo: Option<u32>,
    pub bg_view_mode_desktop: Option<u32>,
    pub bg_rotation_desktop: f32,
    pub bg_inclination_desktop: f32,
    pub bg_layback_desktop: f32,
    pub bg_fov_desktop: f32,
    pub bg_offset_x_desktop: f32,
    pub bg_offset_y_desktop: f32,
    pub bg_offset_z_desktop: f32,
    pub bg_scale_x_desktop: f32,
    pub bg_scale_y_desktop: f32,
    pub bg_scale_z_desktop: f32,
    pub bg_enable_fss: Option<bool>,
    pub bg_view_horizontal_offset_desktop: Option<f32>,
    pub bg_view_vertical_offset_desktop: Option<f32>,
    pub bg_window_top_x_offset_desktop: Option<f32>,
    pub bg_window_top_y_offset_desktop: Option<f32>,
    pub bg_window_top_z_offset_desktop: Option<f32>,
    pub bg_window_bottom_x_offset_desktop: Option<f32>,
    pub bg_window_bottom_y_offset_desktop: Option<f32>,
    pub bg_window_bottom_z_offset_desktop: Option<f32>,
    pub bg_view_mode_fullscreen: Option<u32>,
    pub bg_rotation_fullscreen: f32,
    pub bg_inclination_fullscreen: f32,
    pub bg_layback_fullscreen: f32,
    pub bg_fov_fullscreen: f32,
    pub bg_offset_x_fullscreen: f32,
    pub bg_offset_y_fullscreen: f32,
    pub bg_offset_z_fullscreen: f32,
    pub bg_scale_x_fullscreen: f32,
    pub bg_scale_y_fullscreen: f32,
    pub bg_scale_z_fullscreen: f32,
    pub bg_view_horizontal_offset_fullscreen: Option<f32>,
    pub bg_view_vertical_offset_fullscreen: Option<f32>,
    pub bg_window_top_x_offset_fullscreen: Option<f32>,
    pub bg_window_top_y_offset_fullscreen: Option<f32>,
    pub bg_window_top_z_offset_fullscreen: Option<f32>,
    pub bg_window_bottom_x_offset_fullscreen: Option<f32>,
    pub bg_window_bottom_y_offset_fullscreen: Option<f32>,
    pub bg_window_bottom_z_offset_fullscreen: Option<f32>,
    pub bg_view_mode_full_single_screen: Option<u32>,
    pub bg_rotation_full_single_screen: Option<f32>,
    pub bg_inclination_full_single_screen: Option<f32>,
    pub bg_layback_full_single_screen: Option<f32>,
    pub bg_fov_full_single_screen: Option<f32>,
    pub bg_offset_x_full_single_screen: Option<f32>,
    pub bg_offset_y_full_single_screen: Option<f32>,
    pub bg_offset_z_full_single_screen: Option<f32>,
    pub bg_scale_x_full_single_screen: Option<f32>,
    pub bg_scale_y_full_single_screen: Option<f32>,
    pub bg_scale_z_full_single_screen: Option<f32>,
    pub bg_view_horizontal_offset_full_single_screen: Option<f32>,
    pub bg_view_vertical_offset_full_single_screen: Option<f32>,
    pub bg_window_top_x_offset_full_single_screen: Option<f32>,
    pub bg_window_top_y_offset_full_single_screen: Option<f32>,
    pub bg_window_top_z_offset_full_single_screen: Option<f32>,
    pub bg_window_bottom_x_offset_full_single_screen: Option<f32>,
    pub bg_window_bottom_y_offset_full_single_screen: Option<f32>,
    pub bg_window_bottom_z_offset_full_single_screen: Option<f32>,
    pub override_physics: u32,
    pub override_physics_flipper: Option<bool>,
    pub gravity: f32,
    pub friction: f32,
    pub elasticity: f32,
    pub elastic_falloff: f32,
    pub scatter: f32,
    pub default_scatter: f32,
    pub nudge_time: f32,
    pub plunger_normalize: u32,
    pub plunger_filter: bool,
    pub physics_max_loops: u32,
    pub render_em_reels: bool,
    pub render_decals: bool,
    pub offset_x: f32,
    pub offset_y: f32,
    pub zoom: f32,
    pub angle_tilt_max: f32,
    pub angle_tilt_min: f32,
    pub stereo_max_separation: f32,
    pub stereo_zero_parallax_displacement: f32,
    pub stereo_offset: Option<f32>,
    pub overwrite_global_stereo3d: bool,
    pub image: String,
    pub backglass_image_full_desktop: String,
    pub backglass_image_full_fullscreen: String,
    pub backglass_image_full_single_screen: Option<String>,
    pub image_backdrop_night_day: bool,
    pub image_color_grade: String,
    pub ball_image: String,
    pub ball_spherical_mapping: Option<bool>,
    pub ball_image_front: String,
    pub env_image: Option<String>,
    pub notes: Option<String>,
    pub screen_shot: String,
    pub display_backdrop: bool,
    pub glass_top_height: f32,
    pub glass_bottom_height: Option<f32>,
    pub table_height: Option<f32>,
    pub playfield_material: String,
    pub backdrop_color: u32,
    pub global_difficulty: f32,
    pub light_ambient: u32,
    pub light0_emission: u32,
    pub light_height: f32,
    pub light_range: f32,
    pub light_emission_scale: f32,
    pub env_emission_scale: f32,
    pub global_emission_scale: f32,
    pub ao_scale: f32,
    pub ssr_scale: Option<f32>,
    pub table_sound_volume: f32,
    pub table_music_volume: f32,
    pub table_adaptive_vsync: Option<i32>,
    pub use_reflection_for_balls: Option<i32>,
    pub brst: Option<i32>,
    pub playfield_reflection_strength: f32,
    pub use_trail_for_balls: Option<i32>,
    pub ball_decal_mode: bool,
    pub ball_playfield_reflection_strength: Option<f32>,
    pub default_bulb_intensity_scale_on_ball: Option<f32>,
    pub ball_trail_strength: Option<u32>,
    pub user_detail_level: Option<u32>,
    pub overwrite_global_detail_level: Option<bool>,
    pub overwrite_global_day_night: Option<bool>,
    pub show_grid: bool,
    pub reflect_elements_on_playfield: Option<bool>,
    pub use_aal: Option<i32>,
    pub use_fxaa: Option<i32>,
    pub use_ao: Option<i32>,
    pub use_ssr: Option<i32>,
    pub tone_mapper: Option<i32>,
    pub bloom_strength: f32,
    pub name: String,
    pub custom_colors: [ColorJson; 16],
    pub protection_data: Option<Vec<u8>>,
    //pub code: StringWithEncoding,
    pub is_locked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_10_8_0_beta1_to_beta4: Option<bool>,
}

impl GameDataJson {
    pub fn to_game_data(&self) -> GameData {
        let custom_colors: [Color; 16] = self
            .custom_colors
            .iter()
            .map(|c| c.to_color())
            .collect::<Vec<Color>>()
            .try_into()
            .unwrap();

        GameData {
            left: self.left,
            top: self.top,
            right: self.right,
            bottom: self.bottom,
            clmo: self.clmo,
            bg_view_mode_desktop: self.bg_view_mode_desktop,
            bg_rotation_desktop: self.bg_rotation_desktop,
            bg_inclination_desktop: self.bg_inclination_desktop,
            bg_layback_desktop: self.bg_layback_desktop,
            bg_fov_desktop: self.bg_fov_desktop,
            bg_offset_x_desktop: self.bg_offset_x_desktop,
            bg_offset_y_desktop: self.bg_offset_y_desktop,
            bg_offset_z_desktop: self.bg_offset_z_desktop,
            bg_scale_x_desktop: self.bg_scale_x_desktop,
            bg_scale_y_desktop: self.bg_scale_y_desktop,
            bg_scale_z_desktop: self.bg_scale_z_desktop,
            bg_enable_fss: self.bg_enable_fss,
            bg_view_horizontal_offset_desktop: self.bg_view_horizontal_offset_desktop,
            bg_view_vertical_offset_desktop: self.bg_view_vertical_offset_desktop,
            bg_window_top_x_offset_desktop: self.bg_window_top_x_offset_desktop,
            bg_window_top_y_offset_desktop: self.bg_window_top_y_offset_desktop,
            bg_window_top_z_offset_desktop: self.bg_window_top_z_offset_desktop,
            bg_window_bottom_x_offset_desktop: self.bg_window_bottom_x_offset_desktop,
            bg_window_bottom_y_offset_desktop: self.bg_window_bottom_y_offset_desktop,
            bg_window_bottom_z_offset_desktop: self.bg_window_bottom_z_offset_desktop,
            bg_view_mode_fullscreen: self.bg_view_mode_fullscreen,
            bg_rotation_fullscreen: self.bg_rotation_fullscreen,
            bg_inclination_fullscreen: self.bg_inclination_fullscreen,
            bg_layback_fullscreen: self.bg_layback_fullscreen,
            bg_fov_fullscreen: self.bg_fov_fullscreen,
            bg_offset_x_fullscreen: self.bg_offset_x_fullscreen,
            bg_offset_y_fullscreen: self.bg_offset_y_fullscreen,
            bg_offset_z_fullscreen: self.bg_offset_z_fullscreen,
            bg_scale_x_fullscreen: self.bg_scale_x_fullscreen,
            bg_scale_y_fullscreen: self.bg_scale_y_fullscreen,
            bg_scale_z_fullscreen: self.bg_scale_z_fullscreen,
            bg_view_horizontal_offset_fullscreen: self.bg_view_horizontal_offset_fullscreen,
            bg_view_vertical_offset_fullscreen: self.bg_view_vertical_offset_fullscreen,
            bg_window_top_x_offset_fullscreen: self.bg_window_top_x_offset_fullscreen,
            bg_window_top_y_offset_fullscreen: self.bg_window_top_y_offset_fullscreen,
            bg_window_top_z_offset_fullscreen: self.bg_window_top_z_offset_fullscreen,
            bg_window_bottom_x_offset_fullscreen: self.bg_window_bottom_x_offset_fullscreen,
            bg_window_bottom_y_offset_fullscreen: self.bg_window_bottom_y_offset_fullscreen,
            bg_window_bottom_z_offset_fullscreen: self.bg_window_bottom_z_offset_fullscreen,
            bg_view_mode_full_single_screen: self.bg_view_mode_full_single_screen,
            bg_rotation_full_single_screen: self.bg_rotation_full_single_screen,
            bg_inclination_full_single_screen: self.bg_inclination_full_single_screen,
            bg_layback_full_single_screen: self.bg_layback_full_single_screen,
            bg_fov_full_single_screen: self.bg_fov_full_single_screen,
            bg_offset_x_full_single_screen: self.bg_offset_x_full_single_screen,
            bg_offset_y_full_single_screen: self.bg_offset_y_full_single_screen,
            bg_offset_z_full_single_screen: self.bg_offset_z_full_single_screen,
            bg_scale_x_full_single_screen: self.bg_scale_x_full_single_screen,
            bg_scale_y_full_single_screen: self.bg_scale_y_full_single_screen,
            bg_scale_z_full_single_screen: self.bg_scale_z_full_single_screen,
            bg_view_horizontal_offset_full_single_screen: self
                .bg_view_horizontal_offset_full_single_screen,
            bg_view_vertical_offset_full_single_screen: self
                .bg_view_vertical_offset_full_single_screen,
            bg_window_top_x_offset_full_single_screen: self
                .bg_window_top_x_offset_full_single_screen,
            bg_window_top_y_offset_full_single_screen: self
                .bg_window_top_y_offset_full_single_screen,
            bg_window_top_z_offset_full_single_screen: self
                .bg_window_top_z_offset_full_single_screen,
            bg_window_bottom_x_offset_full_single_screen: self
                .bg_window_bottom_x_offset_full_single_screen,
            bg_window_bottom_y_offset_full_single_screen: self
                .bg_window_bottom_y_offset_full_single_screen,
            bg_window_bottom_z_offset_full_single_screen: self
                .bg_window_bottom_z_offset_full_single_screen,
            override_physics: self.override_physics,
            override_physics_flipper: self.override_physics_flipper,
            gravity: self.gravity,
            friction: self.friction,
            elasticity: self.elasticity,
            elastic_falloff: self.elastic_falloff,
            scatter: self.scatter,
            default_scatter: self.default_scatter,
            nudge_time: self.nudge_time,
            plunger_normalize: self.plunger_normalize,
            plunger_filter: self.plunger_filter,
            physics_max_loops: self.physics_max_loops,
            render_em_reels: self.render_em_reels,
            render_decals: self.render_decals,
            offset_x: self.offset_x,
            offset_y: self.offset_y,
            zoom: self.zoom,
            angle_tilt_max: self.angle_tilt_max,
            angle_tilt_min: self.angle_tilt_min,
            stereo_max_separation: self.stereo_max_separation,
            stereo_zero_parallax_displacement: self.stereo_zero_parallax_displacement,
            stereo_offset: self.stereo_offset,
            overwrite_global_stereo3d: self.overwrite_global_stereo3d,
            image: self.image.clone(),
            backglass_image_full_desktop: self.backglass_image_full_desktop.clone(),
            backglass_image_full_fullscreen: self.backglass_image_full_fullscreen.clone(),
            backglass_image_full_single_screen: self.backglass_image_full_single_screen.clone(),
            image_backdrop_night_day: self.image_backdrop_night_day,
            image_color_grade: self.image_color_grade.clone(),
            ball_image: self.ball_image.clone(),
            ball_spherical_mapping: self.ball_spherical_mapping,
            ball_image_front: self.ball_image_front.clone(),
            env_image: self.env_image.clone(),
            notes: self.notes.clone(),
            screen_shot: self.screen_shot.clone(),
            display_backdrop: self.display_backdrop,
            glass_top_height: self.glass_top_height,
            glass_bottom_height: self.glass_bottom_height,
            table_height: self.table_height,
            playfield_material: self.playfield_material.clone(),
            backdrop_color: self.backdrop_color,
            global_difficulty: self.global_difficulty,
            light_ambient: self.light_ambient,
            light0_emission: self.light0_emission,
            light_height: self.light_height,
            light_range: self.light_range,
            light_emission_scale: self.light_emission_scale,
            env_emission_scale: self.env_emission_scale,
            global_emission_scale: self.global_emission_scale,
            ao_scale: self.ao_scale,
            ssr_scale: self.ssr_scale,
            table_sound_volume: self.table_sound_volume,
            table_music_volume: self.table_music_volume,
            table_adaptive_vsync: self.table_adaptive_vsync,
            use_reflection_for_balls: self.use_reflection_for_balls,
            brst: self.brst,
            playfield_reflection_strength: self.playfield_reflection_strength,
            use_trail_for_balls: self.use_trail_for_balls,
            ball_decal_mode: self.ball_decal_mode,
            ball_playfield_reflection_strength: self.ball_playfield_reflection_strength,
            default_bulb_intensity_scale_on_ball: self.default_bulb_intensity_scale_on_ball,
            ball_trail_strength: self.ball_trail_strength,
            user_detail_level: self.user_detail_level,
            overwrite_global_detail_level: self.overwrite_global_detail_level,
            overwrite_global_day_night: self.overwrite_global_day_night,
            show_grid: self.show_grid,
            reflect_elements_on_playfield: self.reflect_elements_on_playfield,
            use_aal: self.use_aal,
            use_fxaa: self.use_fxaa,
            use_ao: self.use_ao,
            use_ssr: self.use_ssr,
            tone_mapper: self.tone_mapper,
            bloom_strength: self.bloom_strength,
            // this data is loaded from a separate file
            materials_size: 0,
            // this data is loaded from a separate file
            materials_old: vec![],
            // this data is loaded from a separate file
            materials_physics_old: None,
            // this data is loaded from a separate file
            materials: None,
            // this data is loaded from a separate file
            render_probes: None,
            // this data is loaded from a separate file
            gameitems_size: 0,
            // this data is loaded from a separate file
            sounds_size: 0,
            // this data is loaded from a separate file
            images_size: 0,
            // this data is loaded from a separate file
            fonts_size: 0,
            // this data is loaded from a separate file
            collections_size: 0,
            name: self.name.clone(),
            custom_colors,
            protection_data: self.protection_data.clone(),
            code: StringWithEncoding::empty(),
            is_locked: self.is_locked,
            is_10_8_0_beta1_to_beta4: self.is_10_8_0_beta1_to_beta4.unwrap_or(false),
        }
    }

    pub fn from_game_data(game_data: &GameData) -> GameDataJson {
        let custom_colors: [ColorJson; 16] = game_data
            .custom_colors
            .map(|c| ColorJson::from_color(&c))
            .try_into()
            .unwrap();
        GameDataJson {
            left: game_data.left,
            top: game_data.top,
            right: game_data.right,
            bottom: game_data.bottom,
            clmo: game_data.clmo,
            bg_view_mode_desktop: game_data.bg_view_mode_desktop,
            bg_rotation_desktop: game_data.bg_rotation_desktop,
            bg_inclination_desktop: game_data.bg_inclination_desktop,
            bg_layback_desktop: game_data.bg_layback_desktop,
            bg_fov_desktop: game_data.bg_fov_desktop,
            bg_offset_x_desktop: game_data.bg_offset_x_desktop,
            bg_offset_y_desktop: game_data.bg_offset_y_desktop,
            bg_offset_z_desktop: game_data.bg_offset_z_desktop,
            bg_scale_x_desktop: game_data.bg_scale_x_desktop,
            bg_scale_y_desktop: game_data.bg_scale_y_desktop,
            bg_scale_z_desktop: game_data.bg_scale_z_desktop,
            bg_enable_fss: game_data.bg_enable_fss,
            bg_view_horizontal_offset_desktop: game_data.bg_view_horizontal_offset_desktop,
            bg_view_vertical_offset_desktop: game_data.bg_view_vertical_offset_desktop,
            bg_window_top_x_offset_desktop: game_data.bg_window_top_x_offset_desktop,
            bg_window_top_y_offset_desktop: game_data.bg_window_top_y_offset_desktop,
            bg_window_top_z_offset_desktop: game_data.bg_window_top_z_offset_desktop,
            bg_window_bottom_x_offset_desktop: game_data.bg_window_bottom_x_offset_desktop,
            bg_window_bottom_y_offset_desktop: game_data.bg_window_bottom_y_offset_desktop,
            bg_window_bottom_z_offset_desktop: game_data.bg_window_bottom_z_offset_desktop,
            bg_view_mode_fullscreen: game_data.bg_view_mode_fullscreen,
            bg_rotation_fullscreen: game_data.bg_rotation_fullscreen,
            bg_inclination_fullscreen: game_data.bg_inclination_fullscreen,
            bg_layback_fullscreen: game_data.bg_layback_fullscreen,
            bg_fov_fullscreen: game_data.bg_fov_fullscreen,
            bg_offset_x_fullscreen: game_data.bg_offset_x_fullscreen,
            bg_offset_y_fullscreen: game_data.bg_offset_y_fullscreen,
            bg_offset_z_fullscreen: game_data.bg_offset_z_fullscreen,
            bg_scale_x_fullscreen: game_data.bg_scale_x_fullscreen,
            bg_scale_y_fullscreen: game_data.bg_scale_y_fullscreen,
            bg_scale_z_fullscreen: game_data.bg_scale_z_fullscreen,
            bg_view_horizontal_offset_fullscreen: game_data.bg_view_horizontal_offset_fullscreen,
            bg_view_vertical_offset_fullscreen: game_data.bg_view_vertical_offset_fullscreen,
            bg_window_top_x_offset_fullscreen: game_data.bg_window_top_x_offset_fullscreen,
            bg_window_top_y_offset_fullscreen: game_data.bg_window_top_y_offset_fullscreen,
            bg_window_top_z_offset_fullscreen: game_data.bg_window_top_z_offset_fullscreen,
            bg_window_bottom_x_offset_fullscreen: game_data.bg_window_bottom_x_offset_fullscreen,
            bg_window_bottom_y_offset_fullscreen: game_data.bg_window_bottom_y_offset_fullscreen,
            bg_window_bottom_z_offset_fullscreen: game_data.bg_window_bottom_z_offset_fullscreen,
            bg_view_mode_full_single_screen: game_data.bg_view_mode_full_single_screen,
            bg_rotation_full_single_screen: game_data.bg_rotation_full_single_screen,
            bg_inclination_full_single_screen: game_data.bg_inclination_full_single_screen,
            bg_layback_full_single_screen: game_data.bg_layback_full_single_screen,
            bg_fov_full_single_screen: game_data.bg_fov_full_single_screen,
            bg_offset_x_full_single_screen: game_data.bg_offset_x_full_single_screen,
            bg_offset_y_full_single_screen: game_data.bg_offset_y_full_single_screen,
            bg_offset_z_full_single_screen: game_data.bg_offset_z_full_single_screen,
            bg_scale_x_full_single_screen: game_data.bg_scale_x_full_single_screen,
            bg_scale_y_full_single_screen: game_data.bg_scale_y_full_single_screen,
            bg_scale_z_full_single_screen: game_data.bg_scale_z_full_single_screen,
            bg_view_horizontal_offset_full_single_screen: game_data
                .bg_view_horizontal_offset_full_single_screen,
            bg_view_vertical_offset_full_single_screen: game_data
                .bg_view_vertical_offset_full_single_screen,
            bg_window_top_x_offset_full_single_screen: game_data
                .bg_window_top_x_offset_full_single_screen,
            bg_window_top_y_offset_full_single_screen: game_data
                .bg_window_top_y_offset_full_single_screen,
            bg_window_top_z_offset_full_single_screen: game_data
                .bg_window_top_z_offset_full_single_screen,
            bg_window_bottom_x_offset_full_single_screen: game_data
                .bg_window_bottom_x_offset_full_single_screen,
            bg_window_bottom_y_offset_full_single_screen: game_data
                .bg_window_bottom_y_offset_full_single_screen,
            bg_window_bottom_z_offset_full_single_screen: game_data
                .bg_window_bottom_z_offset_full_single_screen,
            override_physics: game_data.override_physics,
            override_physics_flipper: game_data.override_physics_flipper,
            gravity: game_data.gravity,
            friction: game_data.friction,
            elasticity: game_data.elasticity,
            elastic_falloff: game_data.elastic_falloff,
            scatter: game_data.scatter,
            default_scatter: game_data.default_scatter,
            nudge_time: game_data.nudge_time,
            plunger_normalize: game_data.plunger_normalize,
            plunger_filter: game_data.plunger_filter,
            physics_max_loops: game_data.physics_max_loops,
            render_em_reels: game_data.render_em_reels,
            render_decals: game_data.render_decals,
            offset_x: game_data.offset_x,
            offset_y: game_data.offset_y,
            zoom: game_data.zoom,
            angle_tilt_max: game_data.angle_tilt_max,
            angle_tilt_min: game_data.angle_tilt_min,
            stereo_max_separation: game_data.stereo_max_separation,
            stereo_zero_parallax_displacement: game_data.stereo_zero_parallax_displacement,
            stereo_offset: game_data.stereo_offset,
            overwrite_global_stereo3d: game_data.overwrite_global_stereo3d,
            image: game_data.image.clone(),
            backglass_image_full_desktop: game_data.backglass_image_full_desktop.clone(),
            backglass_image_full_fullscreen: game_data.backglass_image_full_fullscreen.clone(),
            backglass_image_full_single_screen: game_data
                .backglass_image_full_single_screen
                .clone(),
            image_backdrop_night_day: game_data.image_backdrop_night_day,
            image_color_grade: game_data.image_color_grade.clone(),
            ball_image: game_data.ball_image.clone(),
            ball_spherical_mapping: game_data.ball_spherical_mapping,
            ball_image_front: game_data.ball_image_front.clone(),
            env_image: game_data.env_image.clone(),
            notes: game_data.notes.clone(),
            screen_shot: game_data.screen_shot.clone(),
            display_backdrop: game_data.display_backdrop,
            glass_top_height: game_data.glass_top_height,
            glass_bottom_height: game_data.glass_bottom_height,
            table_height: game_data.table_height,
            playfield_material: game_data.playfield_material.clone(),
            backdrop_color: game_data.backdrop_color,
            global_difficulty: game_data.global_difficulty,
            light_ambient: game_data.light_ambient,
            light0_emission: game_data.light0_emission,
            light_height: game_data.light_height,
            light_range: game_data.light_range,
            light_emission_scale: game_data.light_emission_scale,
            env_emission_scale: game_data.env_emission_scale,
            global_emission_scale: game_data.global_emission_scale,
            ao_scale: game_data.ao_scale,
            ssr_scale: game_data.ssr_scale,
            table_sound_volume: game_data.table_sound_volume,
            table_music_volume: game_data.table_music_volume,
            table_adaptive_vsync: game_data.table_adaptive_vsync,
            use_reflection_for_balls: game_data.use_reflection_for_balls,
            brst: game_data.brst,
            playfield_reflection_strength: game_data.playfield_reflection_strength,
            use_trail_for_balls: game_data.use_trail_for_balls,
            ball_decal_mode: game_data.ball_decal_mode,
            ball_playfield_reflection_strength: game_data.ball_playfield_reflection_strength,
            default_bulb_intensity_scale_on_ball: game_data.default_bulb_intensity_scale_on_ball,
            ball_trail_strength: game_data.ball_trail_strength,
            user_detail_level: game_data.user_detail_level,
            overwrite_global_detail_level: game_data.overwrite_global_detail_level,
            overwrite_global_day_night: game_data.overwrite_global_day_night,
            show_grid: game_data.show_grid,
            reflect_elements_on_playfield: game_data.reflect_elements_on_playfield,
            use_aal: game_data.use_aal,
            use_fxaa: game_data.use_fxaa,
            use_ao: game_data.use_ao,
            use_ssr: game_data.use_ssr,
            tone_mapper: game_data.tone_mapper,
            bloom_strength: game_data.bloom_strength,
            name: game_data.name.clone(),
            custom_colors,
            protection_data: game_data.protection_data.clone(),
            // code: game_data.code.clone(),
            is_locked: game_data.is_locked,
            is_10_8_0_beta1_to_beta4: Some(game_data.is_10_8_0_beta1_to_beta4)
                .filter(|x| x == &true),
        }
    }
}

impl GameData {
    pub fn set_code(&mut self, script: String) {
        self.code = StringWithEncoding::new(script);
    }

    pub fn get_ball_trail_strength(&self) -> Option<f32> {
        self.ball_trail_strength.map(|v| dequantize_u8(8, v as u8))
    }

    pub fn set_ball_trail_strength(&mut self, value: f32) {
        self.ball_trail_strength = Some(quantize_u8(8, value) as u32);
    }
}

impl Default for GameData {
    fn default() -> Self {
        GameData {
            left: 0.0,
            top: 0.0,
            right: 952.0,
            bottom: 2162.0,
            clmo: None,
            bg_view_mode_desktop: None,
            bg_rotation_desktop: 0.0,
            bg_inclination_desktop: 0.0,
            bg_layback_desktop: 0.0,
            bg_fov_desktop: 45.0,
            bg_offset_x_desktop: 0.0,
            bg_offset_y_desktop: 30.0,
            bg_offset_z_desktop: -200.0,
            bg_scale_x_desktop: 1.0,
            bg_scale_y_desktop: 1.0,
            bg_scale_z_desktop: 1.0,
            bg_enable_fss: None, //false,
            is_10_8_0_beta1_to_beta4: false,
            bg_rotation_fullscreen: 0.0,
            bg_inclination_fullscreen: 0.0,
            bg_layback_fullscreen: 0.0,
            bg_fov_fullscreen: 45.0,
            bg_offset_x_fullscreen: 110.0,
            bg_offset_y_fullscreen: -86.0,
            bg_offset_z_fullscreen: 400.0,
            bg_scale_x_fullscreen: 1.3,
            bg_scale_y_fullscreen: 1.41,
            bg_scale_z_fullscreen: 1.0,
            bg_rotation_full_single_screen: None,    //0.0,
            bg_inclination_full_single_screen: None, //52.0,
            bg_layback_full_single_screen: None,     //0.0,
            bg_fov_full_single_screen: None,         //45.0,
            bg_offset_x_full_single_screen: None,    //0.0,
            bg_offset_y_full_single_screen: None,    //30.0,
            bg_offset_z_full_single_screen: None,    //-50.0,
            bg_scale_x_full_single_screen: None,     //1.2,
            bg_scale_y_full_single_screen: None,     //1.1,
            bg_scale_z_full_single_screen: None,     //1.0,
            override_physics: 0,
            override_physics_flipper: None, //false,
            gravity: 1.762985,
            friction: 0.075,
            elasticity: 0.25,
            elastic_falloff: 0.0,
            scatter: 0.0,
            default_scatter: 0.0,
            nudge_time: 5.0,
            plunger_normalize: 100,
            plunger_filter: false,
            physics_max_loops: 0,
            render_em_reels: false,
            render_decals: false,
            offset_x: 476.0,
            offset_y: 1081.0,
            zoom: 0.5,
            angle_tilt_max: 6.0,
            angle_tilt_min: 6.0,
            stereo_max_separation: 0.015,
            stereo_zero_parallax_displacement: 0.1,
            stereo_offset: None,
            overwrite_global_stereo3d: false,
            image: String::new(),
            backglass_image_full_desktop: String::new(),
            backglass_image_full_fullscreen: String::new(),
            backglass_image_full_single_screen: None,
            image_backdrop_night_day: false,
            image_color_grade: String::new(),
            ball_image: String::new(),
            ball_spherical_mapping: None,
            ball_image_front: String::new(),
            env_image: None,
            notes: None,
            screen_shot: String::new(),
            display_backdrop: false,
            glass_top_height: 400.0,   // new default 210 for both
            glass_bottom_height: None, // new default 210 for both
            table_height: None,        //0.0,
            playfield_material: "".to_string(),
            backdrop_color: 0x232323ff, // bgra
            global_difficulty: 0.2,
            light_ambient: 0x000000ff, // TODO what is the format for all these?
            light0_emission: 0xfffff0ff, // TODO is this correct?
            light_height: 5000.0,
            light_range: 4000000.0,
            light_emission_scale: 4000000.0,
            env_emission_scale: 2.0,
            global_emission_scale: 0.52,
            ao_scale: 1.75,
            ssr_scale: None, //1.0,
            table_sound_volume: 1.0,
            table_music_volume: 1.0,
            table_adaptive_vsync: None,     //-1,
            use_reflection_for_balls: None, //-1,
            brst: None,
            playfield_reflection_strength: 0.2941177,
            use_trail_for_balls: None, //-1,
            ball_decal_mode: false,
            ball_playfield_reflection_strength: None,
            default_bulb_intensity_scale_on_ball: None, //1.0,
            ball_trail_strength: None,                  //quantize_unsigned(8, 0.4901961),
            user_detail_level: None,                    //5,
            overwrite_global_detail_level: None,        //false,
            overwrite_global_day_night: None,           //false,
            show_grid: true,
            reflect_elements_on_playfield: None, //true,
            use_aal: None,                       //-1,
            use_fxaa: None,                      //-1,
            use_ao: None,                        //-1,
            use_ssr: None,                       //-1,
            tone_mapper: None,                   // 0 = TM_REINHARD,
            bloom_strength: 1.8,
            materials_size: 0,
            materials_old: Vec::new(),
            materials_physics_old: None,
            materials: None,
            render_probes: None,
            gameitems_size: 0,
            sounds_size: 0,
            images_size: 0,
            fonts_size: 0,
            collections_size: 0,
            name: "Table1".to_string(), // seems to be the default name
            custom_colors: [Color::BLACK; 16],
            protection_data: None,
            code: StringWithEncoding::empty(),
            bg_view_horizontal_offset_desktop: None,
            bg_view_vertical_offset_desktop: None,
            bg_window_top_x_offset_desktop: None,
            bg_window_top_y_offset_desktop: None,
            bg_window_top_z_offset_desktop: None,
            bg_window_bottom_x_offset_desktop: None,
            bg_window_bottom_y_offset_desktop: None,
            bg_window_bottom_z_offset_desktop: None,
            bg_view_mode_fullscreen: None,
            bg_view_horizontal_offset_fullscreen: None,
            bg_view_vertical_offset_fullscreen: None,
            bg_window_top_x_offset_fullscreen: None,
            bg_window_top_y_offset_fullscreen: None,
            bg_window_top_z_offset_fullscreen: None,
            bg_window_bottom_x_offset_fullscreen: None,
            bg_window_bottom_y_offset_fullscreen: None,
            bg_window_bottom_z_offset_fullscreen: None,
            bg_view_mode_full_single_screen: None,
            bg_view_horizontal_offset_full_single_screen: None,
            bg_view_vertical_offset_full_single_screen: None,
            bg_window_top_x_offset_full_single_screen: None,
            bg_window_top_y_offset_full_single_screen: None,
            bg_window_top_z_offset_full_single_screen: None,
            bg_window_bottom_x_offset_full_single_screen: None,
            bg_window_bottom_y_offset_full_single_screen: None,
            bg_window_bottom_z_offset_full_single_screen: None,
            is_locked: None,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Record {
    name: String,
    data: Vec<u8>,
}

pub fn write_all_gamedata_records(gamedata: &GameData, version: &Version) -> Vec<u8> {
    let mut writer = BiffWriter::new();
    // order is important
    writer.write_tagged_f32("LEFT", gamedata.left);
    writer.write_tagged_f32("TOPX", gamedata.top);
    writer.write_tagged_f32("RGHT", gamedata.right);
    writer.write_tagged_f32("BOTM", gamedata.bottom);
    if let Some(clmo) = gamedata.clmo {
        writer.write_tagged_u32("CLMO", clmo);
    }

    if version.u32() >= 1080 && !gamedata.is_10_8_0_beta1_to_beta4 {
        if let Some(efss) = gamedata.bg_enable_fss {
            writer.write_tagged_bool("EFSS", efss);
        }
    }
    if let Some(vsm0) = gamedata.bg_view_mode_desktop {
        writer.write_tagged_u32("VSM0", vsm0);
    }
    writer.write_tagged_f32("ROTA", gamedata.bg_rotation_desktop);
    writer.write_tagged_f32("INCL", gamedata.bg_inclination_desktop);
    writer.write_tagged_f32("LAYB", gamedata.bg_layback_desktop);
    writer.write_tagged_f32("FOVX", gamedata.bg_fov_desktop);
    writer.write_tagged_f32("XLTX", gamedata.bg_offset_x_desktop);
    writer.write_tagged_f32("XLTY", gamedata.bg_offset_y_desktop);
    writer.write_tagged_f32("XLTZ", gamedata.bg_offset_z_desktop);
    writer.write_tagged_f32("SCLX", gamedata.bg_scale_x_desktop);
    writer.write_tagged_f32("SCLY", gamedata.bg_scale_y_desktop);
    writer.write_tagged_f32("SCLZ", gamedata.bg_scale_z_desktop);

    if let Some(hof0) = gamedata.bg_view_horizontal_offset_desktop {
        writer.write_tagged_f32("HOF0", hof0);
    }
    if let Some(vof0) = gamedata.bg_view_vertical_offset_desktop {
        writer.write_tagged_f32("VOF0", vof0);
    }
    if let Some(wtx0) = gamedata.bg_window_top_x_offset_desktop {
        writer.write_tagged_f32("WTX0", wtx0);
    }
    if let Some(wty0) = gamedata.bg_window_top_y_offset_desktop {
        writer.write_tagged_f32("WTY0", wty0);
    }
    if let Some(wtz0) = gamedata.bg_window_top_z_offset_desktop {
        writer.write_tagged_f32("WTZ0", wtz0);
    }
    if let Some(wbx0) = gamedata.bg_window_bottom_x_offset_desktop {
        writer.write_tagged_f32("WBX0", wbx0);
    }
    if let Some(wby0) = gamedata.bg_window_bottom_y_offset_desktop {
        writer.write_tagged_f32("WBY0", wby0);
    }
    if let Some(wbz0) = gamedata.bg_window_bottom_z_offset_desktop {
        writer.write_tagged_f32("WBZ0", wbz0);
    }

    if let Some(vsm1) = gamedata.bg_view_mode_fullscreen {
        writer.write_tagged_u32("VSM1", vsm1);
    }

    if version.u32() < 1080 || gamedata.is_10_8_0_beta1_to_beta4 {
        if let Some(efss) = gamedata.bg_enable_fss {
            writer.write_tagged_bool("EFSS", efss);
        }
    }
    writer.write_tagged_f32("ROTF", gamedata.bg_rotation_fullscreen);
    writer.write_tagged_f32("INCF", gamedata.bg_inclination_fullscreen);
    writer.write_tagged_f32("LAYF", gamedata.bg_layback_fullscreen);
    writer.write_tagged_f32("FOVF", gamedata.bg_fov_fullscreen);
    writer.write_tagged_f32("XLFX", gamedata.bg_offset_x_fullscreen);
    writer.write_tagged_f32("XLFY", gamedata.bg_offset_y_fullscreen);
    writer.write_tagged_f32("XLFZ", gamedata.bg_offset_z_fullscreen);
    writer.write_tagged_f32("SCFX", gamedata.bg_scale_x_fullscreen);
    writer.write_tagged_f32("SCFY", gamedata.bg_scale_y_fullscreen);
    writer.write_tagged_f32("SCFZ", gamedata.bg_scale_z_fullscreen);
    if let Some(hof1) = gamedata.bg_view_horizontal_offset_fullscreen {
        writer.write_tagged_f32("HOF1", hof1);
    }
    if let Some(vof1) = gamedata.bg_view_vertical_offset_fullscreen {
        writer.write_tagged_f32("VOF1", vof1);
    }
    if let Some(wtx1) = gamedata.bg_window_top_x_offset_fullscreen {
        writer.write_tagged_f32("WTX1", wtx1);
    }
    if let Some(wty1) = gamedata.bg_window_top_y_offset_fullscreen {
        writer.write_tagged_f32("WTY1", wty1);
    }
    if let Some(wtz1) = gamedata.bg_window_top_z_offset_fullscreen {
        writer.write_tagged_f32("WTZ1", wtz1);
    }
    if let Some(wbx1) = gamedata.bg_window_bottom_x_offset_fullscreen {
        writer.write_tagged_f32("WBX1", wbx1);
    }
    if let Some(wby1) = gamedata.bg_window_bottom_y_offset_fullscreen {
        writer.write_tagged_f32("WBY1", wby1);
    }
    if let Some(wbz1) = gamedata.bg_window_bottom_z_offset_fullscreen {
        writer.write_tagged_f32("WBZ1", wbz1);
    }

    if let Some(vsm2) = gamedata.bg_view_mode_full_single_screen {
        writer.write_tagged_u32("VSM2", vsm2);
    }
    if let Some(rofs) = gamedata.bg_rotation_full_single_screen {
        writer.write_tagged_f32("ROFS", rofs);
    }
    if let Some(infs) = gamedata.bg_inclination_full_single_screen {
        writer.write_tagged_f32("INFS", infs);
    }
    if let Some(lafs) = gamedata.bg_layback_full_single_screen {
        writer.write_tagged_f32("LAFS", lafs);
    }
    if let Some(fofs) = gamedata.bg_fov_full_single_screen {
        writer.write_tagged_f32("FOFS", fofs);
    }
    if let Some(xlxs) = gamedata.bg_offset_x_full_single_screen {
        writer.write_tagged_f32("XLXS", xlxs);
    }
    if let Some(xlys) = gamedata.bg_offset_y_full_single_screen {
        writer.write_tagged_f32("XLYS", xlys);
    }
    if let Some(xlzs) = gamedata.bg_offset_z_full_single_screen {
        writer.write_tagged_f32("XLZS", xlzs);
    }
    if let Some(scxs) = gamedata.bg_scale_x_full_single_screen {
        writer.write_tagged_f32("SCXS", scxs);
    }
    if let Some(scys) = gamedata.bg_scale_y_full_single_screen {
        writer.write_tagged_f32("SCYS", scys);
    }
    if let Some(sczs) = gamedata.bg_scale_z_full_single_screen {
        writer.write_tagged_f32("SCZS", sczs);
    }
    if let Some(hof2) = gamedata.bg_view_horizontal_offset_full_single_screen {
        writer.write_tagged_f32("HOF2", hof2);
    }
    if let Some(vof2) = gamedata.bg_view_vertical_offset_full_single_screen {
        writer.write_tagged_f32("VOF2", vof2);
    }
    if let Some(wtx2) = gamedata.bg_window_top_x_offset_full_single_screen {
        writer.write_tagged_f32("WTX2", wtx2);
    }
    if let Some(wty2) = gamedata.bg_window_top_y_offset_full_single_screen {
        writer.write_tagged_f32("WTY2", wty2);
    }
    if let Some(wtz2) = gamedata.bg_window_top_z_offset_full_single_screen {
        writer.write_tagged_f32("WTZ2", wtz2);
    }
    if let Some(wbx2) = gamedata.bg_window_bottom_x_offset_full_single_screen {
        writer.write_tagged_f32("WBX2", wbx2);
    }
    if let Some(wby2) = gamedata.bg_window_bottom_y_offset_full_single_screen {
        writer.write_tagged_f32("WBY2", wby2);
    }
    if let Some(wbz2) = gamedata.bg_window_bottom_z_offset_full_single_screen {
        writer.write_tagged_f32("WBZ2", wbz2);
    }

    writer.write_tagged_u32("ORRP", gamedata.override_physics);
    if let Some(orpf) = gamedata.override_physics_flipper {
        writer.write_tagged_bool("ORPF", orpf);
    }
    writer.write_tagged_f32("GAVT", gamedata.gravity);
    writer.write_tagged_f32("FRCT", gamedata.friction);
    writer.write_tagged_f32("ELAS", gamedata.elasticity);
    writer.write_tagged_f32("ELFA", gamedata.elastic_falloff);
    writer.write_tagged_f32("PFSC", gamedata.scatter);
    writer.write_tagged_f32("SCAT", gamedata.default_scatter);
    writer.write_tagged_f32("NDGT", gamedata.nudge_time);
    writer.write_tagged_u32("MPGC", gamedata.plunger_normalize);
    writer.write_tagged_bool("MPDF", gamedata.plunger_filter);
    writer.write_tagged_u32("PHML", gamedata.physics_max_loops);
    writer.write_tagged_bool("REEL", gamedata.render_em_reels);
    writer.write_tagged_bool("DECL", gamedata.render_decals);
    writer.write_tagged_f32("OFFX", gamedata.offset_x);
    writer.write_tagged_f32("OFFY", gamedata.offset_y);
    writer.write_tagged_f32("ZOOM", gamedata.zoom);
    writer.write_tagged_f32("SLPX", gamedata.angle_tilt_max);
    writer.write_tagged_f32("SLOP", gamedata.angle_tilt_min);
    writer.write_tagged_f32("MAXS", gamedata.stereo_max_separation);
    writer.write_tagged_f32("ZPD", gamedata.stereo_zero_parallax_displacement);
    if let Some(sto) = gamedata.stereo_offset {
        writer.write_tagged_f32("STO", sto);
    }
    writer.write_tagged_bool("OGST", gamedata.overwrite_global_stereo3d);
    writer.write_tagged_string("IMAG", &gamedata.image);
    writer.write_tagged_string("BIMG", &gamedata.backglass_image_full_desktop);
    writer.write_tagged_string("BIMF", &gamedata.backglass_image_full_fullscreen);

    if let Some(bims) = &gamedata.backglass_image_full_single_screen {
        writer.write_tagged_string("BIMS", bims);
    }
    writer.write_tagged_bool("BIMN", gamedata.image_backdrop_night_day);
    writer.write_tagged_string("IMCG", &gamedata.image_color_grade);
    writer.write_tagged_string("BLIM", &gamedata.ball_image);
    if let Some(ball_spherical_mapping) = gamedata.ball_spherical_mapping {
        writer.write_tagged_bool("BLSM", ball_spherical_mapping);
    }
    writer.write_tagged_string("BLIF", &gamedata.ball_image_front);
    if let Some(env_image) = &gamedata.env_image {
        writer.write_tagged_string("EIMG", env_image);
    }
    if let Some(notes) = &gamedata.notes {
        writer.write_tagged_string("NOTX", notes);
    }
    writer.write_tagged_string("SSHT", &gamedata.screen_shot);
    writer.write_tagged_bool("FBCK", gamedata.display_backdrop);
    writer.write_tagged_f32("GLAS", gamedata.glass_top_height);
    if let Some(glass_bottom_height) = gamedata.glass_bottom_height {
        writer.write_tagged_f32("GLAB", glass_bottom_height);
    }
    if let Some(table_height) = gamedata.table_height {
        writer.write_tagged_f32("TBLH", table_height);
    }
    writer.write_tagged_string("PLMA", &gamedata.playfield_material);
    writer.write_tagged_u32("BCLR", gamedata.backdrop_color);
    writer.write_tagged_f32("TDFT", gamedata.global_difficulty);
    writer.write_tagged_u32("LZAM", gamedata.light_ambient);
    writer.write_tagged_u32("LZDI", gamedata.light0_emission);
    writer.write_tagged_f32("LZHI", gamedata.light_height);
    writer.write_tagged_f32("LZRA", gamedata.light_range);
    writer.write_tagged_f32("LIES", gamedata.light_emission_scale);
    writer.write_tagged_f32("ENES", gamedata.env_emission_scale);
    writer.write_tagged_f32("GLES", gamedata.global_emission_scale);
    writer.write_tagged_f32("AOSC", gamedata.ao_scale);
    if let Some(sssc) = gamedata.ssr_scale {
        writer.write_tagged_f32("SSSC", sssc);
    }
    writer.write_tagged_f32("SVOL", gamedata.table_sound_volume);
    writer.write_tagged_f32("MVOL", gamedata.table_music_volume);
    if let Some(avsy) = gamedata.table_adaptive_vsync {
        writer.write_tagged_i32("AVSY", avsy);
    }
    if let Some(bref) = gamedata.use_reflection_for_balls {
        writer.write_tagged_i32("BREF", bref);
    }
    if let Some(brst) = gamedata.brst {
        writer.write_tagged_i32("BRST", brst);
    }
    writer.write_tagged_f32("PLST", gamedata.playfield_reflection_strength);
    if let Some(btst) = gamedata.use_trail_for_balls {
        writer.write_tagged_i32("BTRA", btst);
    }
    writer.write_tagged_bool("BDMO", gamedata.ball_decal_mode);
    if let Some(bprs) = gamedata.ball_playfield_reflection_strength {
        writer.write_tagged_f32("BPRS", bprs);
    }
    if let Some(dbis) = gamedata.default_bulb_intensity_scale_on_ball {
        writer.write_tagged_f32("DBIS", dbis);
    }
    if let Some(btst) = gamedata.ball_trail_strength {
        writer.write_tagged_u32("BTST", btst);
    }
    if let Some(arac) = gamedata.user_detail_level {
        writer.write_tagged_u32("ARAC", arac);
    }
    if let Some(ogac) = gamedata.overwrite_global_detail_level {
        writer.write_tagged_bool("OGAC", ogac);
    }
    if let Some(ogdn) = gamedata.overwrite_global_day_night {
        writer.write_tagged_bool("OGDN", ogdn);
    }
    writer.write_tagged_bool("GDAC", gamedata.show_grid);
    if let Some(reop) = gamedata.reflect_elements_on_playfield {
        writer.write_tagged_bool("REOP", reop);
    }
    if let Some(uaal) = gamedata.use_aal {
        writer.write_tagged_i32("UAAL", uaal);
    }
    if let Some(ufxa) = gamedata.use_fxaa {
        writer.write_tagged_i32("UFXA", ufxa);
    }
    if let Some(uaoc) = gamedata.use_ao {
        writer.write_tagged_i32("UAOC", uaoc);
    }
    if let Some(ussr) = gamedata.use_ssr {
        writer.write_tagged_i32("USSR", ussr);
    }
    if let Some(tmap) = gamedata.tone_mapper {
        writer.write_tagged_i32("TMAP", tmap);
    }
    writer.write_tagged_f32("BLST", gamedata.bloom_strength);
    writer.write_tagged_u32("MASI", gamedata.materials_size);
    let mut bytes = BytesMut::new();
    for mat in &gamedata.materials_old {
        mat.write(&mut bytes);
    }
    writer.write_tagged_data("MATE", &bytes);
    if let Some(phma) = &gamedata.materials_physics_old {
        let mut bytes = BytesMut::new();
        for mat in phma {
            mat.write(&mut bytes);
        }
        writer.write_tagged_data("PHMA", &bytes);
    }
    if let Some(materials_new) = &gamedata.materials {
        for mat in materials_new {
            let mut mat_writer = BiffWriter::new();
            mat.biff_write(&mut mat_writer);
            writer.write_tagged_data("MATR", mat_writer.get_data());
        }
    }
    // multiple RPRB // added in 10.8.x
    if let Some(render_probes) = &gamedata.render_probes {
        for render_probe in render_probes {
            let mut probe_writer = BiffWriter::new();
            render_probe.biff_write(&mut probe_writer);
            writer.write_tagged_data("RPRB", probe_writer.get_data());
        }
    }
    writer.write_tagged_u32("SEDT", gamedata.gameitems_size);
    writer.write_tagged_u32("SSND", gamedata.sounds_size);
    writer.write_tagged_u32("SIMG", gamedata.images_size);
    writer.write_tagged_u32("SFNT", gamedata.fonts_size);
    writer.write_tagged_u32("SCOL", gamedata.collections_size);
    writer.write_tagged_wide_string("NAME", &gamedata.name);

    let custom_color_bytes = write_colors(&gamedata.custom_colors);

    writer.write_tagged_data("CCUS", custom_color_bytes.as_slice());
    if let Some(protection_data) = &gamedata.protection_data {
        writer.write_tagged_data("SECB", protection_data);
    }
    writer.write_tagged_string_with_encoding_no_size("CODE", &gamedata.code);
    if let Some(is_locked) = gamedata.is_locked {
        writer.write_tagged_bool("TLCK", is_locked);
    }

    writer.close(true);
    // TODO how do we get rid of this extra copy?
    writer.get_data().to_vec()
}

pub fn read_all_gamedata_records(input: &[u8], version: &Version) -> GameData {
    let mut reader = BiffReader::new(input);
    let mut gamedata = GameData::default();
    let mut previous_tag = String::new();
    loop {
        reader.next(biff::WARN);
        if reader.is_eof() {
            break;
        }
        let tag = reader.tag();

        let reader: &mut BiffReader<'_> = &mut reader;

        match tag.as_str() {
            "LEFT" => gamedata.left = reader.get_f32(),
            "TOPX" => gamedata.top = reader.get_f32(),
            "RGHT" => gamedata.right = reader.get_f32(),
            "BOTM" => gamedata.bottom = reader.get_f32(),
            "CLMO" => gamedata.clmo = Some(reader.get_u32()),
            "VSM0" => gamedata.bg_view_mode_desktop = Some(reader.get_u32()),
            "ROTA" => gamedata.bg_rotation_desktop = reader.get_f32(),
            "INCL" => gamedata.bg_inclination_desktop = reader.get_f32(),
            "LAYB" => gamedata.bg_layback_desktop = reader.get_f32(),
            "FOVX" => gamedata.bg_fov_desktop = reader.get_f32(),
            "XLTX" => gamedata.bg_offset_x_desktop = reader.get_f32(),
            "XLTY" => gamedata.bg_offset_y_desktop = reader.get_f32(),
            "XLTZ" => gamedata.bg_offset_z_desktop = reader.get_f32(),
            "SCLX" => gamedata.bg_scale_x_desktop = reader.get_f32(),
            "SCLY" => gamedata.bg_scale_y_desktop = reader.get_f32(),
            "SCLZ" => gamedata.bg_scale_z_desktop = reader.get_f32(),
            "EFSS" => {
                if version.u32() == 1080 && previous_tag != "BOTM" {
                    gamedata.is_10_8_0_beta1_to_beta4 = true;
                }
                gamedata.bg_enable_fss = Some(reader.get_bool())
            }
            "HOF0" => gamedata.bg_view_horizontal_offset_desktop = Some(reader.get_f32()),
            "VOF0" => gamedata.bg_view_vertical_offset_desktop = Some(reader.get_f32()),
            "WTX0" => gamedata.bg_window_top_x_offset_desktop = Some(reader.get_f32()),
            "WTY0" => gamedata.bg_window_top_y_offset_desktop = Some(reader.get_f32()),
            "WTZ0" => gamedata.bg_window_top_z_offset_desktop = Some(reader.get_f32()),
            "WBX0" => gamedata.bg_window_bottom_x_offset_desktop = Some(reader.get_f32()),
            "WBY0" => gamedata.bg_window_bottom_y_offset_desktop = Some(reader.get_f32()),
            "WBZ0" => gamedata.bg_window_bottom_z_offset_desktop = Some(reader.get_f32()),
            "VSM1" => gamedata.bg_view_mode_fullscreen = Some(reader.get_u32()),
            "ROTF" => gamedata.bg_rotation_fullscreen = reader.get_f32(),
            "INCF" => gamedata.bg_inclination_fullscreen = reader.get_f32(),
            "LAYF" => gamedata.bg_layback_fullscreen = reader.get_f32(),
            "FOVF" => gamedata.bg_fov_fullscreen = reader.get_f32(),
            "XLFX" => gamedata.bg_offset_x_fullscreen = reader.get_f32(),
            "XLFY" => gamedata.bg_offset_y_fullscreen = reader.get_f32(),
            "XLFZ" => gamedata.bg_offset_z_fullscreen = reader.get_f32(),
            "SCFX" => gamedata.bg_scale_x_fullscreen = reader.get_f32(),
            "SCFY" => gamedata.bg_scale_y_fullscreen = reader.get_f32(),
            "SCFZ" => gamedata.bg_scale_z_fullscreen = reader.get_f32(),
            "HOF1" => gamedata.bg_view_horizontal_offset_fullscreen = Some(reader.get_f32()),
            "VOF1" => gamedata.bg_view_vertical_offset_fullscreen = Some(reader.get_f32()),
            "WTX1" => gamedata.bg_window_top_x_offset_fullscreen = Some(reader.get_f32()),
            "WTY1" => gamedata.bg_window_top_y_offset_fullscreen = Some(reader.get_f32()),
            "WTZ1" => gamedata.bg_window_top_z_offset_fullscreen = Some(reader.get_f32()),
            "WBX1" => gamedata.bg_window_bottom_x_offset_fullscreen = Some(reader.get_f32()),
            "WBY1" => gamedata.bg_window_bottom_y_offset_fullscreen = Some(reader.get_f32()),
            "WBZ1" => gamedata.bg_window_bottom_z_offset_fullscreen = Some(reader.get_f32()),
            "VSM2" => gamedata.bg_view_mode_full_single_screen = Some(reader.get_u32()),
            "ROFS" => gamedata.bg_rotation_full_single_screen = Some(reader.get_f32()),
            "INFS" => gamedata.bg_inclination_full_single_screen = Some(reader.get_f32()),
            "LAFS" => gamedata.bg_layback_full_single_screen = Some(reader.get_f32()),
            "FOFS" => gamedata.bg_fov_full_single_screen = Some(reader.get_f32()),
            "XLXS" => gamedata.bg_offset_x_full_single_screen = Some(reader.get_f32()),
            "XLYS" => gamedata.bg_offset_y_full_single_screen = Some(reader.get_f32()),
            "XLZS" => gamedata.bg_offset_z_full_single_screen = Some(reader.get_f32()),
            "SCXS" => gamedata.bg_scale_x_full_single_screen = Some(reader.get_f32()),
            "SCYS" => gamedata.bg_scale_y_full_single_screen = Some(reader.get_f32()),
            "SCZS" => gamedata.bg_scale_z_full_single_screen = Some(reader.get_f32()),
            "HOF2" => {
                gamedata.bg_view_horizontal_offset_full_single_screen = Some(reader.get_f32())
            }
            "VOF2" => gamedata.bg_view_vertical_offset_full_single_screen = Some(reader.get_f32()),
            "WTX2" => gamedata.bg_window_top_x_offset_full_single_screen = Some(reader.get_f32()),
            "WTY2" => gamedata.bg_window_top_y_offset_full_single_screen = Some(reader.get_f32()),
            "WTZ2" => gamedata.bg_window_top_z_offset_full_single_screen = Some(reader.get_f32()),
            "WBX2" => {
                gamedata.bg_window_bottom_x_offset_full_single_screen = Some(reader.get_f32())
            }
            "WBY2" => {
                gamedata.bg_window_bottom_y_offset_full_single_screen = Some(reader.get_f32())
            }
            "WBZ2" => {
                gamedata.bg_window_bottom_z_offset_full_single_screen = Some(reader.get_f32())
            }
            "ORRP" => gamedata.override_physics = reader.get_u32(),
            "ORPF" => gamedata.override_physics_flipper = Some(reader.get_bool()),
            "GAVT" => gamedata.gravity = reader.get_f32(),
            "FRCT" => gamedata.friction = reader.get_f32(),
            "ELAS" => gamedata.elasticity = reader.get_f32(),
            "ELFA" => gamedata.elastic_falloff = reader.get_f32(),
            "PFSC" => gamedata.scatter = reader.get_f32(),
            "SCAT" => gamedata.default_scatter = reader.get_f32(),
            "NDGT" => gamedata.nudge_time = reader.get_f32(),
            "MPGC" => gamedata.plunger_normalize = reader.get_u32(),
            "MPDF" => gamedata.plunger_filter = reader.get_bool(),
            "PHML" => gamedata.physics_max_loops = reader.get_u32(),
            "REEL" => gamedata.render_em_reels = reader.get_bool(),
            "DECL" => gamedata.render_decals = reader.get_bool(),
            "OFFX" => gamedata.offset_x = reader.get_f32(),
            "OFFY" => gamedata.offset_y = reader.get_f32(),
            "ZOOM" => gamedata.zoom = reader.get_f32(),
            "SLPX" => gamedata.angle_tilt_max = reader.get_f32(),
            "SLOP" => gamedata.angle_tilt_min = reader.get_f32(),
            "MAXS" => gamedata.stereo_max_separation = reader.get_f32(),
            "ZPD" => gamedata.stereo_zero_parallax_displacement = reader.get_f32(),
            "STO" => gamedata.stereo_offset = Some(reader.get_f32()),
            "OGST" => gamedata.overwrite_global_stereo3d = reader.get_bool(),
            "IMAG" => gamedata.image = reader.get_string(),
            "BIMG" => gamedata.backglass_image_full_desktop = reader.get_string(),
            "BIMF" => gamedata.backglass_image_full_fullscreen = reader.get_string(),
            "BIMS" => gamedata.backglass_image_full_single_screen = Some(reader.get_string()),
            "BIMN" => gamedata.image_backdrop_night_day = reader.get_bool(),
            "IMCG" => gamedata.image_color_grade = reader.get_string(),
            "BLIM" => gamedata.ball_image = reader.get_string(),
            "BLSM" => gamedata.ball_spherical_mapping = Some(reader.get_bool()),
            "BLIF" => gamedata.ball_image_front = reader.get_string(),
            "EIMG" => gamedata.env_image = Some(reader.get_string()),
            "NOTX" => gamedata.notes = Some(reader.get_string()),
            "SSHT" => gamedata.screen_shot = reader.get_string(),
            "FBCK" => gamedata.display_backdrop = reader.get_bool(),
            "GLAS" => gamedata.glass_top_height = reader.get_f32(),
            "GLAB" => gamedata.glass_bottom_height = Some(reader.get_f32()),
            "TBLH" => gamedata.table_height = Some(reader.get_f32()),
            "PLMA" => gamedata.playfield_material = reader.get_string(),
            "BCLR" => gamedata.backdrop_color = reader.get_u32(),
            "TDFT" => gamedata.global_difficulty = reader.get_f32(),
            "LZAM" => gamedata.light_ambient = reader.get_u32(),
            "LZDI" => gamedata.light0_emission = reader.get_u32(),
            "LZHI" => gamedata.light_height = reader.get_f32(),
            "LZRA" => gamedata.light_range = reader.get_f32(),
            "LIES" => gamedata.light_emission_scale = reader.get_f32(),
            "ENES" => gamedata.env_emission_scale = reader.get_f32(),
            "GLES" => gamedata.global_emission_scale = reader.get_f32(),
            "AOSC" => gamedata.ao_scale = reader.get_f32(),
            "SSSC" => gamedata.ssr_scale = Some(reader.get_f32()),
            "SVOL" => gamedata.table_sound_volume = reader.get_f32(),
            "MVOL" => gamedata.table_music_volume = reader.get_f32(),
            "AVSY" => gamedata.table_adaptive_vsync = Some(reader.get_i32()),
            "BREF" => gamedata.use_reflection_for_balls = Some(reader.get_i32()),
            "BRST" => gamedata.brst = Some(reader.get_i32()),
            "PLST" => gamedata.playfield_reflection_strength = reader.get_f32(),
            "BTRA" => gamedata.use_trail_for_balls = Some(reader.get_i32()),
            "BDMO" => gamedata.ball_decal_mode = reader.get_bool(),
            "BPRS" => gamedata.ball_playfield_reflection_strength = Some(reader.get_f32()),
            "DBIS" => gamedata.default_bulb_intensity_scale_on_ball = Some(reader.get_f32()),
            "BTST" => {
                // TODO do we need this QuantizedUnsignedBits for some of the float fields?
                gamedata.ball_trail_strength = Some(reader.get_u32());
            }
            "ARAC" => gamedata.user_detail_level = Some(reader.get_u32()),
            "OGAC" => gamedata.overwrite_global_detail_level = Some(reader.get_bool()),
            "OGDN" => gamedata.overwrite_global_day_night = Some(reader.get_bool()),
            "GDAC" => gamedata.show_grid = reader.get_bool(),
            "REOP" => gamedata.reflect_elements_on_playfield = Some(reader.get_bool()),
            "UAAL" => gamedata.use_aal = Some(reader.get_i32()),
            "UFXA" => gamedata.use_fxaa = Some(reader.get_i32()),
            "UAOC" => gamedata.use_ao = Some(reader.get_i32()),
            "USSR" => gamedata.use_ssr = Some(reader.get_i32()),
            "TMAP" => gamedata.tone_mapper = Some(reader.get_i32()),
            "BLST" => gamedata.bloom_strength = reader.get_f32(),
            "MASI" => gamedata.materials_size = reader.get_u32(),
            "MATE" => {
                let data = reader.get_record_data(false).to_vec();
                let mut materials: Vec<SaveMaterial> = Vec::new();
                let mut buff = BytesMut::from(data.as_slice());
                for _ in 0..gamedata.materials_size {
                    let material = SaveMaterial::read(&mut buff);
                    materials.push(material);
                }
                gamedata.materials_old = materials;
            }
            "PHMA" => {
                let data = reader.get_record_data(false).to_vec();
                let mut materials: Vec<SavePhysicsMaterial> = Vec::new();
                let mut buff = BytesMut::from(data.as_slice());
                for _ in 0..gamedata.materials_size {
                    let material = SavePhysicsMaterial::read(&mut buff);
                    materials.push(material);
                }
                gamedata.materials_physics_old = Some(materials);
            }
            // see https://github.com/vpinball/vpinball/blob/1a994086a6092733272fda36a2f449753a1ca21a/pintable.cpp#L4429
            "MATR" => {
                let data = reader.get_record_data(false).to_vec();
                let mut reader = BiffReader::new(&data);
                let material = Material::biff_read(&mut reader);
                gamedata
                    .materials
                    .get_or_insert_with(Vec::new)
                    .push(material);
            }
            "RPRB" => {
                let data = reader.get_record_data(false).to_vec();
                let mut reader = BiffReader::new(&data);
                let render_probe = RenderProbeWithGarbage::biff_read(&mut reader);
                gamedata
                    .render_probes
                    .get_or_insert_with(Vec::new)
                    .push(render_probe);
            }
            "SEDT" => gamedata.gameitems_size = reader.get_u32(),
            "SSND" => gamedata.sounds_size = reader.get_u32(),
            "SIMG" => gamedata.images_size = reader.get_u32(),
            "SFNT" => gamedata.fonts_size = reader.get_u32(),
            "SCOL" => gamedata.collections_size = reader.get_u32(),
            "NAME" => gamedata.name = reader.get_wide_string(),
            "CCUS" => {
                let data = reader.get_record_data(false);
                let custom_colors = read_colors(data);
                gamedata.custom_colors = custom_colors;
            }
            "SECB" => gamedata.protection_data = Some(reader.get_record_data(false).to_vec()),
            "CODE" => {
                let len = reader.get_u32_no_remaining_update();
                // at least a the time of 1060, some code was still encoded in latin1
                gamedata.code = reader.get_str_with_encoding_no_remaining_update(len as usize);
            }
            "TLCK" => gamedata.is_locked = Some(reader.get_bool()),
            other => {
                let data = reader.get_record_data(false);
                println!("unhandled tag {} {} bytes", other, data.len());
            }
        };
        previous_tag = tag;
    }
    gamedata
}

fn read_colors(data: Vec<u8>) -> [Color; 16] {
    // COLORREF: 0x00BBGGRR
    // sizeof(COLORREF) * 16
    let mut colors = Vec::new();
    let mut buff = BytesMut::from(data.as_slice());
    for _ in 0..16 {
        let color = Color::new_bgr(buff.get_u32_le());
        colors.push(color);
    }
    <[Color; 16]>::try_from(colors).unwrap()
}

fn write_colors(colors: &[Color; 16]) -> Vec<u8> {
    let mut bytes = BytesMut::new();
    for color in colors {
        bytes.put_u32_le(color.bgr());
    }
    bytes.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fake::{Fake, Faker};
    use pretty_assertions::assert_eq;

    #[test]
    fn read_write_empty() {
        let game_data = GameData::default();
        let version: Version = Version::new(1074);
        let bytes = write_all_gamedata_records(&game_data, &version);
        let read_game_data = read_all_gamedata_records(&bytes, &version);

        assert_eq!(game_data, read_game_data);
    }

    #[test]
    fn read_write() {
        let gamedata = GameData {
            left: 1.0,
            right: 2.0,
            top: 3.0,
            bottom: 4.0,
            clmo: None,
            bg_view_mode_desktop: Some(1),
            bg_rotation_desktop: 1.0,
            bg_inclination_desktop: 2.0,
            bg_layback_desktop: 3.0,
            bg_fov_desktop: 4.0,
            bg_offset_x_desktop: 1.0,
            bg_offset_y_desktop: 2.0,
            bg_offset_z_desktop: 3.0,
            bg_scale_x_desktop: 3.3,
            bg_scale_y_desktop: 2.2,
            bg_scale_z_desktop: 1.1,
            bg_enable_fss: Some(true),
            bg_rotation_fullscreen: 1.0,
            bg_inclination_fullscreen: 2.0,
            bg_layback_fullscreen: 3.0,
            bg_fov_fullscreen: 4.0,
            bg_offset_x_fullscreen: 1.0,
            bg_offset_y_fullscreen: 2.0,
            bg_offset_z_fullscreen: 3.0,
            bg_scale_x_fullscreen: 3.3,
            bg_scale_y_fullscreen: 2.2,
            bg_scale_z_fullscreen: 1.1,
            bg_rotation_full_single_screen: Some(1.0),
            bg_inclination_full_single_screen: Some(2.0),
            bg_layback_full_single_screen: Some(3.0),
            bg_fov_full_single_screen: Some(4.0),
            bg_offset_x_full_single_screen: Some(1.0),
            bg_offset_y_full_single_screen: Some(2.0),
            bg_offset_z_full_single_screen: Some(3.0),
            bg_scale_x_full_single_screen: Some(3.3),
            bg_scale_y_full_single_screen: Some(2.2),
            bg_scale_z_full_single_screen: Some(1.1),
            override_physics: 1,
            override_physics_flipper: Some(true),
            gravity: 1.0,
            friction: 0.1,
            elasticity: 0.2,
            elastic_falloff: 0.3,
            scatter: 0.2,
            default_scatter: 0.1,
            nudge_time: 3.0,
            plunger_normalize: 105,
            plunger_filter: true,
            physics_max_loops: 30,
            render_em_reels: true,
            render_decals: true,
            offset_x: 50.0,
            offset_y: 60.0,
            zoom: 0.2,
            angle_tilt_max: 4.0,
            angle_tilt_min: 3.0,
            stereo_max_separation: 0.03,
            stereo_zero_parallax_displacement: 0.2,
            stereo_offset: Some(0.5),
            overwrite_global_stereo3d: true,
            image: String::from("test image"),
            backglass_image_full_desktop: String::from("test desktop"),
            backglass_image_full_fullscreen: String::from("test fullscreen"),
            backglass_image_full_single_screen: Some(String::from("test single screen")),
            image_backdrop_night_day: true,
            image_color_grade: String::from("test color grade"),
            ball_image: String::from("test ball image"),
            ball_spherical_mapping: Some(true),
            ball_image_front: String::from("test ball image"),
            env_image: Some(String::from("test env image")),
            notes: Some(String::from("test notes")),
            screen_shot: String::from("test screenshot"),
            display_backdrop: true,
            glass_top_height: 234.0,
            glass_bottom_height: Some(123.0),
            table_height: Some(12.0),
            playfield_material: "material_pf".to_string(),
            backdrop_color: 0x333333ff,
            global_difficulty: 0.3,
            light_ambient: 0x11223344,
            light0_emission: 0xaabbccdd,
            light_height: 4000.0,
            light_range: 50000.0,
            light_emission_scale: 1.2,
            env_emission_scale: 1.23,
            global_emission_scale: 0.111,
            ao_scale: 0.9,
            ssr_scale: Some(0.5),
            table_sound_volume: 0.6,
            table_music_volume: 0.5,
            table_adaptive_vsync: Some(1),
            use_reflection_for_balls: Some(1),
            brst: Some(123),
            playfield_reflection_strength: 0.02,
            use_trail_for_balls: Some(-3),
            ball_decal_mode: true,
            ball_playfield_reflection_strength: Some(2.0),
            default_bulb_intensity_scale_on_ball: Some(2.0),
            ball_trail_strength: Some(quantize_u8(8, 0.55) as u32),
            user_detail_level: Some(9),
            overwrite_global_detail_level: Some(true),
            overwrite_global_day_night: Some(false),
            show_grid: false,
            reflect_elements_on_playfield: Some(false),
            use_aal: Some(-10),
            use_fxaa: Some(-2),
            use_ao: Some(-3),
            use_ssr: Some(-4),
            tone_mapper: Some(1), // TM_TONY_MC_MAPFACE
            bloom_strength: 0.3,
            materials_size: 0,
            gameitems_size: 0,
            sounds_size: 0,
            images_size: 0,
            fonts_size: 0,
            collections_size: 0,
            materials_old: vec![],
            materials_physics_old: Some(vec![]),
            materials: None,
            render_probes: Some(vec![Faker.fake(), Faker.fake()]),
            name: String::from("test name"),
            custom_colors: [Color::RED; 16],
            protection_data: None,
            code: StringWithEncoding::from("test code wit some unicode: Ǣ"),
            bg_view_horizontal_offset_desktop: None,
            bg_view_vertical_offset_desktop: None,
            bg_window_top_x_offset_desktop: None,
            bg_window_top_y_offset_desktop: None,
            bg_window_top_z_offset_desktop: None,
            bg_window_bottom_x_offset_desktop: None,
            bg_window_bottom_y_offset_desktop: None,
            bg_window_bottom_z_offset_desktop: None,
            bg_view_mode_fullscreen: None,
            bg_view_horizontal_offset_fullscreen: None,
            bg_view_vertical_offset_fullscreen: None,
            bg_window_top_x_offset_fullscreen: None,
            bg_window_top_y_offset_fullscreen: None,
            bg_window_top_z_offset_fullscreen: None,
            bg_window_bottom_x_offset_fullscreen: None,
            bg_window_bottom_y_offset_fullscreen: None,
            bg_window_bottom_z_offset_fullscreen: None,
            bg_view_mode_full_single_screen: None,
            bg_view_horizontal_offset_full_single_screen: None,
            bg_view_vertical_offset_full_single_screen: None,
            bg_window_top_x_offset_full_single_screen: None,
            bg_window_top_y_offset_full_single_screen: None,
            bg_window_top_z_offset_full_single_screen: None,
            bg_window_bottom_x_offset_full_single_screen: None,
            bg_window_bottom_y_offset_full_single_screen: None,
            bg_window_bottom_z_offset_full_single_screen: None,
            is_locked: Some(true),
            is_10_8_0_beta1_to_beta4: false,
        };
        let version = Version::new(1074);
        let bytes = write_all_gamedata_records(&gamedata, &version);
        let read_game_data = read_all_gamedata_records(&bytes, &version);

        assert_eq!(gamedata, read_game_data);
    }
}
