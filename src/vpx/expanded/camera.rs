//! Camera setup for GLTF export
//!
//! This module provides camera positioning based on VPinball's three view modes:
//! - **Desktop**: Default view for playing on a computer monitor
//! - **Fullscreen**: View for cabinet setups with separate backglass monitor
//! - **FSS (Full Single Screen)**: Single-monitor cabinet setups
//!
//! See [`crate::vpx::gamedata`] for documentation on VPU units and view settings.

use crate::vpx::VPX;
use crate::vpx::gamedata::ViewLayoutMode;
use serde_json::json;

/// Conversion factor from VP units to meters
/// This is based on the size of a typical pinball (1.0625 inches/27mm/50 VPU)
/// From VPinball def.h: 50 VPU = 1.0625 inches, 1 inch = 25.4mm
/// So 1 VPU = (25.4 * 1.0625) / 50 mm = 0.539750 mm = 0.000539750 meters
const VP_UNITS_TO_METERS: f32 = (25.4 * 1.0625) / (50.0 * 1000.0);

/// TODO: This scaling factor compensates for using simplified table bounds (8 corner vertices)
/// instead of actual object bounding vertices like VPinball does. VPinball collects bounds from
/// all table objects (only Ramp, Rubber, and Surface contribute to legacy_bounds), which results
/// in a smaller bounding volume. Our simplified approach using full table corners overestimates
/// the required viewing distance. This factor was empirically determined to give reasonable results.
/// A proper fix would be to collect actual object bounds from the table.
const FIT_CAMERA_DISTANCE_SCALE: f32 = 0.47;

/// Result from FitCameraToVertices - camera position to fit the table in view
#[derive(Debug, Clone, Copy)]
struct FittedCamera {
    /// X position (center of view)
    #[allow(dead_code)]
    x: f32,
    /// Y position (center of view)
    #[allow(dead_code)]
    y: f32,
    /// Z position (distance from table)
    z: f32,
}

/// Port of VPinball's FitCameraToVertices function
///
/// Computes the camera position needed to fit a set of vertices in view,
/// given the FOV, aspect ratio, rotation, inclination, and layback.
///
/// **Note:** VPinball passes actual object bounding vertices collected from all table elements.
/// We use a simplified approximation with 8 corner vertices of the table bounds.
/// This may result in different camera distances than VPinball for tables with tall objects.
///
/// # Arguments
/// * `bounds` - Table bounds (used to generate corner vertices)
/// * `aspect` - Aspect ratio (width/height)
/// * `rotation` - Viewport rotation in radians
/// * `inclination` - Camera inclination in radians
/// * `fov` - Field of view in degrees
/// * `xlatez` - Z offset (mViewZ from VPX)
/// * `layback` - Layback angle in degrees
/// * `table_height_z` - Height of table elements in VPU (typically glass_top_height)
fn fit_camera_to_vertices(
    bounds: &TableBounds,
    aspect: f32,
    rotation: f32,
    inclination: f32,
    fov: f32,
    xlatez: f32,
    layback: f32,
    table_height_z: f32,
) -> FittedCamera {
    let rrotsin = rotation.sin();
    let rrotcos = rotation.cos();
    let rincsin = inclination.sin();
    let rinccos = inclination.cos();

    // slope is half of FOV (FOV includes top and bottom)
    let slopey = (0.5 * fov.to_radians()).tan();

    // Field of view along x axis = atan(tan(yFOV)*width/height)
    // So the slope of x simply equals slopey * aspect
    let slopex = slopey * aspect;

    let mut maxyintercept = f32::NEG_INFINITY;
    let mut minyintercept = f32::INFINITY;
    let mut maxxintercept = f32::NEG_INFINITY;
    let mut minxintercept = f32::INFINITY;

    // Layback transformation matrix (only _32 element is non-identity)
    let layback_tan = -(0.5 * layback.to_radians()).tan();

    // Generate table corner vertices (simplified - VPinball uses actual object bounds)
    // We use the 8 corners of the table bounding box (4 corners at z=0, 4 at table_height_z)
    let corners = [
        // Bottom plane (z=0)
        (bounds.left, bounds.top, 0.0_f32),
        (bounds.right, bounds.top, 0.0),
        (bounds.left, bounds.bottom, 0.0),
        (bounds.right, bounds.bottom, 0.0),
        // Top plane (z=table_height_z)
        (bounds.left, bounds.top, table_height_z),
        (bounds.right, bounds.top, table_height_z),
        (bounds.left, bounds.bottom, table_height_z),
        (bounds.right, bounds.bottom, table_height_z),
    ];

    for (vx, vy, vz) in corners {
        // Apply layback transformation: v.y += layback_tan * v.z
        let vy = vy + layback_tan * vz;

        // Rotate vertex about x axis according to inclination
        let temp = vy;
        let vy = rinccos * temp - rincsin * vz;
        let vz = rincsin * temp + rinccos * vz;

        // Rotate vertex about z axis according to rotation
        let temp = vx;
        let vx = rrotcos * temp - rrotsin * vy;
        let vy = rrotsin * temp + rrotcos * vy;

        // Extend slope lines from point to find camera intersection
        maxyintercept = maxyintercept.max(vy + slopey * vz);
        minyintercept = minyintercept.min(vy - slopey * vz);
        maxxintercept = maxxintercept.max(vx + slopex * vz);
        minxintercept = minxintercept.min(vx - slopex * vz);
    }

    // Find camera center in xy plane and distance
    let ydist = (maxyintercept - minyintercept) / (slopey * 2.0);
    let xdist = (maxxintercept - minxintercept) / (slopex * 2.0);

    FittedCamera {
        x: (maxxintercept + minxintercept) * 0.5,
        y: (maxyintercept + minyintercept) * 0.5,
        z: ydist.max(xdist) + xlatez,
    }
}

/// The three view modes supported by VPinball
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewMode {
    /// Desktop view - default for computer monitors
    Desktop,
    /// Fullscreen view - for cabinet setups with separate backglass
    Fullscreen,
    /// Full Single Screen - single-monitor cabinet setups
    Fss,
}

impl ViewMode {
    /// Get the camera name for this view mode
    pub fn camera_name(&self) -> &'static str {
        match self {
            ViewMode::Desktop => "DesktopCamera",
            ViewMode::Fullscreen => "FullscreenCamera",
            ViewMode::Fss => "FssCamera",
        }
    }
}

/// View settings extracted from VPX gamedata
///
/// These settings control the camera position for viewing the table.
/// Each view mode (Desktop, Fullscreen, FSS) has its own set of settings.
#[derive(Debug, Clone)]
pub(crate) struct ViewSettings {
    /// The view mode these settings are for
    pub mode: ViewMode,
    /// The layout mode (Legacy, Camera, Window) - affects how offsets are interpreted
    pub layout_mode: ViewLayoutMode,
    /// Field of view in degrees
    pub fov: f32,
    /// Inclination angle in degrees (legacy mode) or look-at percentage 0-1 (camera mode)
    pub inclination: f32,
    /// X offset from table center in VPU
    pub offset_x: f32,
    /// Y offset in VPU
    pub offset_y: f32,
    /// Z offset in VPU
    pub offset_z: f32,
    /// Scale X multiplier
    #[allow(dead_code)]
    pub scale_x: f32,
    /// Scale Y multiplier
    #[allow(dead_code)]
    pub scale_y: f32,
    /// Scale Z multiplier
    #[allow(dead_code)]
    pub scale_z: f32,
}

impl ViewSettings {
    /// Extract Desktop view settings from VPX gamedata
    ///
    /// ## Default values (legacy VPX format):
    /// - FOV: 45 degrees
    /// - Inclination: 0 degrees
    /// - Offset X: 0 VPU
    /// - Offset Y: 30 VPU (~1.6 cm)
    /// - Offset Z: -200 VPU (~-10.8 cm)
    /// - Scale: 1.0, 1.0, 1.0
    pub fn desktop_from_vpx(vpx: &VPX) -> Self {
        Self {
            mode: ViewMode::Desktop,
            layout_mode: vpx
                .gamedata
                .bg_view_mode_desktop
                .unwrap_or(ViewLayoutMode::Legacy),
            fov: vpx.gamedata.bg_fov_desktop.max(1.0),
            inclination: vpx.gamedata.bg_inclination_desktop,
            offset_x: vpx.gamedata.bg_offset_x_desktop,
            offset_y: vpx.gamedata.bg_offset_y_desktop,
            offset_z: vpx.gamedata.bg_offset_z_desktop,
            scale_x: vpx.gamedata.bg_scale_x_desktop,
            scale_y: vpx.gamedata.bg_scale_y_desktop,
            scale_z: vpx.gamedata.bg_scale_z_desktop,
        }
    }

    /// Extract Fullscreen view settings from VPX gamedata
    ///
    /// ## Default values (legacy VPX format):
    /// - FOV: 45 degrees
    /// - Inclination: 0 degrees
    /// - Offset X: 110 VPU (~5.9 cm)
    /// - Offset Y: -86 VPU (~-4.6 cm)
    /// - Offset Z: 400 VPU (~21.6 cm)
    /// - Scale: 1.3, 1.41, 1.0
    pub fn fullscreen_from_vpx(vpx: &VPX) -> Self {
        Self {
            mode: ViewMode::Fullscreen,
            layout_mode: vpx
                .gamedata
                .bg_view_mode_fullscreen
                .unwrap_or(ViewLayoutMode::Legacy),
            fov: vpx.gamedata.bg_fov_fullscreen.max(1.0),
            inclination: vpx.gamedata.bg_inclination_fullscreen,
            offset_x: vpx.gamedata.bg_offset_x_fullscreen,
            offset_y: vpx.gamedata.bg_offset_y_fullscreen,
            offset_z: vpx.gamedata.bg_offset_z_fullscreen,
            scale_x: vpx.gamedata.bg_scale_x_fullscreen,
            scale_y: vpx.gamedata.bg_scale_y_fullscreen,
            scale_z: vpx.gamedata.bg_scale_z_fullscreen,
        }
    }

    /// Extract FSS (Full Single Screen) view settings from VPX gamedata
    ///
    /// ## Default values (legacy VPX format, used when not present):
    /// - FOV: 45 degrees
    /// - Inclination: 52 degrees
    /// - Offset X: 0 VPU
    /// - Offset Y: 30 VPU (~1.6 cm)
    /// - Offset Z: -50 VPU (~-2.7 cm)
    /// - Scale: 1.2, 1.1, 1.0
    pub fn fss_from_vpx(vpx: &VPX) -> Self {
        Self {
            mode: ViewMode::Fss,
            layout_mode: vpx
                .gamedata
                .bg_view_mode_full_single_screen
                .unwrap_or(ViewLayoutMode::Legacy),
            fov: vpx
                .gamedata
                .bg_fov_full_single_screen
                .unwrap_or(45.0)
                .max(1.0),
            inclination: vpx
                .gamedata
                .bg_inclination_full_single_screen
                .unwrap_or(52.0),
            offset_x: vpx.gamedata.bg_offset_x_full_single_screen.unwrap_or(0.0),
            offset_y: vpx.gamedata.bg_offset_y_full_single_screen.unwrap_or(30.0),
            offset_z: vpx.gamedata.bg_offset_z_full_single_screen.unwrap_or(-50.0),
            scale_x: vpx.gamedata.bg_scale_x_full_single_screen.unwrap_or(1.2),
            scale_y: vpx.gamedata.bg_scale_y_full_single_screen.unwrap_or(1.1),
            scale_z: vpx.gamedata.bg_scale_z_full_single_screen.unwrap_or(1.0),
        }
    }

    /// Extract all three view settings from VPX gamedata
    pub fn all_from_vpx(vpx: &VPX) -> [Self; 3] {
        [
            Self::desktop_from_vpx(vpx),
            Self::fullscreen_from_vpx(vpx),
            Self::fss_from_vpx(vpx),
        ]
    }
}

// Keep the old type alias for backward compatibility
pub(crate) type FssViewSettings = ViewSettings;

/// Table bounds in VPX coordinates
#[derive(Debug, Clone, Copy)]
pub(crate) struct TableBounds {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    /// Height of the glass above the playfield in VPU
    pub glass_height: f32,
}

impl TableBounds {
    pub fn from_vpx(vpx: &VPX) -> Self {
        Self {
            left: vpx.gamedata.left,
            top: vpx.gamedata.top,
            right: vpx.gamedata.right,
            bottom: vpx.gamedata.bottom,
            glass_height: vpx.gamedata.glass_top_height,
        }
    }

    #[allow(dead_code)]
    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    #[allow(dead_code)]
    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    pub fn center_x(&self) -> f32 {
        (self.left + self.right) / 2.0
    }

    #[allow(dead_code)]
    pub fn center_y(&self) -> f32 {
        (self.top + self.bottom) / 2.0
    }
}

/// Camera position and orientation in glTF coordinates
#[derive(Debug, Clone)]
pub(crate) struct GltfCamera {
    /// The view mode this camera represents
    pub mode: ViewMode,
    /// Position in glTF coordinates (meters)
    pub position: [f32; 3],
    /// Rotation as quaternion [x, y, z, w]
    pub rotation: [f32; 4],
    /// Vertical field of view in radians
    pub yfov: f32,
    /// Near clipping plane
    pub znear: f32,
    /// Far clipping plane
    pub zfar: f32,
}

impl GltfCamera {
    /// Create a camera from view settings and table bounds
    ///
    /// The camera is positioned to give a good overview of the entire table,
    /// looking down at the playfield from above and slightly behind (towards the player position).
    ///
    /// ## Legacy mode
    /// In legacy mode, VPinball first computes a "fitted" camera position using `FitCameraToVertices()`
    /// that automatically frames the table. The offset_x/y/z values are then small adjustments
    /// from this fitted position. The inclination angle (in degrees) determines the camera tilt.
    ///
    /// ## Camera/Window mode
    /// In camera mode, the offset values are absolute positions relative to the table bottom center.
    /// The inclination is a look-at percentage (0-1) of table height.
    pub fn from_view_settings(settings: &ViewSettings, bounds: &TableBounds) -> Self {
        let fov_rad = settings.fov.to_radians();

        let (camera_position, rotation) = match settings.layout_mode {
            ViewLayoutMode::Legacy => {
                // In legacy mode, VPinball uses FitCameraToVertices to compute base camera distance.
                //
                // "Look At" / Inclination is a PERCENTAGE (0-100) that controls the viewing angle:
                // - 0% = looking straight down at table (90° from horizontal)
                // - 100% = looking horizontally (0° from horizontal)
                // - 56% = 90° * (1 - 0.56) = 39.6° from horizontal
                //
                // Offsets are in SCREEN/CAMERA-LOCAL coordinates:
                // - offset_x: screen X (left/right on screen)
                // - offset_y: screen Y (up/down on screen)
                // - offset_z: closer/farther along the view axis (positive = farther from table)

                // Convert percentage to pitch angle (from horizontal)
                let look_at_fraction = settings.inclination / 100.0;
                let pitch_degrees = 90.0 * (1.0 - look_at_fraction);
                let pitch_rad = pitch_degrees.to_radians();

                let aspect = 16.0 / 9.0;
                let rotation_rad = 0.0_f32;
                let layback = 0.0;

                let fit = fit_camera_to_vertices(
                    bounds,
                    aspect,
                    rotation_rad,
                    pitch_rad,
                    settings.fov,
                    0.0, // Don't include offset_z in fit, we add it separately
                    layback,
                    bounds.glass_height,
                );

                // Use fit.z as the base distance, scaled to compensate for simplified bounds
                // Also apply scene scale: scale > 1 means you see more of the table (it appears smaller),
                // so the camera should be farther away to match that view
                let scene_scale = (settings.scale_x + settings.scale_y) / 2.0;
                let base_distance = fit.z * FIT_CAMERA_DISTANCE_SCALE * scene_scale;

                // Base camera position: looking at table center from distance at pitch angle
                let look_at_x = bounds.center_x();
                let look_at_y = bounds.center_y();
                let look_at_z = 0.0;

                // Camera base position (before screen-space offsets)
                // At pitch angle from horizontal:
                // - Y offset (toward player) = distance * cos(pitch)
                // - Z offset (height) = distance * sin(pitch)
                let cam_base_x = look_at_x;
                let cam_base_y = look_at_y + base_distance * pitch_rad.cos();
                let cam_base_z = look_at_z + base_distance * pitch_rad.sin();

                // Apply screen-space offsets:
                // Screen X = World X (left/right)
                // Screen Y (up on screen) at pitch angle:
                //   When pitch=0 (horizontal), screen up = World +Z
                //   When pitch=90 (looking down), screen up = World +Y
                //   General: screen up = (0, sin(pitch), cos(pitch))
                // offset_z = along view axis (positive = farther from table)
                //   View direction toward table = (0, -cos(pitch), -sin(pitch))
                //   So offset_z (away from table) adds (0, cos(pitch), sin(pitch)) * offset_z

                // offset_x: screen left/right = world X
                let world_offset_x = settings.offset_x;

                // offset_y: screen up/down
                let screen_up_y = pitch_rad.sin();
                let screen_up_z = pitch_rad.cos();
                let world_offset_y_from_screen_y = settings.offset_y * screen_up_y;
                let world_offset_z_from_screen_y = settings.offset_y * screen_up_z;

                // offset_z: along view axis (away from table)
                let view_dir_y = pitch_rad.cos();
                let view_dir_z = pitch_rad.sin();
                let world_offset_y_from_view = settings.offset_z * view_dir_y;
                let world_offset_z_from_view = settings.offset_z * view_dir_z;

                let vpx_x = cam_base_x + world_offset_x;
                let vpx_y = cam_base_y + world_offset_y_from_screen_y + world_offset_y_from_view;
                let vpx_z = cam_base_z + world_offset_z_from_screen_y + world_offset_z_from_view;

                #[cfg(test)]
                {
                    println!("Legacy mode calculation:");
                    println!(
                        "  inclination: {}% -> pitch: {:.1}°",
                        settings.inclination, pitch_degrees
                    );
                    println!("  fit.z: {}, base_distance: {}", fit.z, base_distance);
                    println!(
                        "  cam_base: ({}, {}, {})",
                        cam_base_x, cam_base_y, cam_base_z
                    );
                    println!(
                        "  offsets (screen): x={}, y={}, z={}",
                        settings.offset_x, settings.offset_y, settings.offset_z
                    );
                    println!("  final vpx: ({}, {}, {})", vpx_x, vpx_y, vpx_z);
                    println!(
                        "  final meters: x={:.3}, y(height)={:.3}, z(depth)={:.3}",
                        vpx_x * VP_UNITS_TO_METERS,
                        vpx_z * VP_UNITS_TO_METERS,
                        vpx_y * VP_UNITS_TO_METERS
                    );
                }

                // Transform VPX -> glTF: (x, y, z) -> (x, z, y)
                let camera_x = vpx_x * VP_UNITS_TO_METERS;
                let camera_y = vpx_z * VP_UNITS_TO_METERS; // VPX Z (height) -> glTF Y
                let camera_z = vpx_y * VP_UNITS_TO_METERS; // VPX Y (depth) -> glTF Z

                let position = [camera_x, camera_y, camera_z];

                // Camera rotation: pitch down from horizontal
                // In glTF, cameras look down -Z axis by default. To look at the table (which is below),
                // we need to pitch DOWN, which is a NEGATIVE rotation around X axis.
                let rotation = Self::euler_to_quaternion_yxz(0.0, -pitch_rad);

                (position, rotation)
            }
            ViewLayoutMode::Camera | ViewLayoutMode::Window => {
                // In camera/window mode (VPinball 10.8+):
                // The camera orbits around the table center at a fixed distance.
                // Offsets are in CAMERA-LOCAL coordinates:
                // - offset_x: left/right offset (perpendicular to view, in screen X)
                // - offset_y: offset along the VIEW AXIS (toward/away from table)
                // - offset_z: up/down offset (perpendicular to view, in screen Y)
                //
                // "Look At" percentage (0-100) controls the camera angle:
                // - 0% = looking straight down at table (camera above table)
                // - 100% = looking horizontally (camera at table level, looking at front)
                //
                // The angle maps as: pitch = 90° * (1 - lookAt/100)
                // So 56% -> 90° * 0.44 = 39.6° pitch from horizontal

                // Convert Look At percentage to pitch angle (from horizontal)
                let look_at_fraction = settings.inclination / 100.0;
                let pitch_degrees = 90.0 * (1.0 - look_at_fraction);
                let pitch_rad = pitch_degrees.to_radians();

                // Base camera distance - use fit_camera_to_vertices to get a reasonable distance
                let aspect = 16.0 / 9.0;
                let fit = fit_camera_to_vertices(
                    bounds,
                    aspect,
                    0.0,       // rotation
                    pitch_rad, // inclination
                    settings.fov,
                    0.0, // no z offset in fit calculation
                    0.0, // layback
                    bounds.glass_height,
                );

                // Use fit.z as the base distance, scaled to compensate for simplified bounds
                let base_distance = fit.z * FIT_CAMERA_DISTANCE_SCALE;

                // Camera looks at table center
                let look_at_x = bounds.center_x();
                let look_at_y = bounds.center_y();
                let look_at_z = 0.0; // playfield level

                // Camera position before offsets:
                // At pitch angle from horizontal, distance base_distance from look_at point
                // - cos(pitch) component along world Y (toward player)
                // - sin(pitch) component along world Z (height)
                let cam_base_x = look_at_x;
                let cam_base_y = look_at_y + base_distance * pitch_rad.cos();
                let cam_base_z = look_at_z + base_distance * pitch_rad.sin();

                // Now apply offsets in camera-local coordinates:
                // Camera's local axes at this pitch angle:
                // - Local X = World X (left/right)
                // - Local Y (view direction) = (0, -cos(pitch), -sin(pitch)) pointing toward table
                // - Local Z (up in camera) = (0, sin(pitch), -cos(pitch))... wait, need to think about this
                //
                // Actually, the offset_y moves along the view axis (away from table = positive)
                // offset_z moves perpendicular to view in the vertical plane
                //
                // View direction: from camera toward look_at = (0, -cos(pitch), -sin(pitch))
                // So offset_y (along view, positive = away from table):
                //   adds (0, cos(pitch), sin(pitch)) * offset_y
                // offset_z (up perpendicular to view):
                //   the "up" perpendicular to view direction in the YZ plane is (0, -sin(pitch), cos(pitch))
                //   wait, let me think again...
                //
                // If camera is above and behind table looking down:
                // - View direction: (0, -cos(pitch), -sin(pitch)) [toward table]
                // - Camera "up" direction: (0, sin(pitch), cos(pitch))... no wait
                //
                // Let's use standard camera orientation:
                // - Camera looks toward -Z in its local space
                // - Camera up is +Y in local space
                // After rotating by pitch around X axis:
                // - Local -Z (forward) becomes (0, sin(pitch), -cos(pitch)) in world...
                //
                // I think I'm overcomplicating. Let me just use the simple geometric interpretation:
                // offset_y moves camera along the line from camera to look_at point (positive = further away)
                // offset_z moves camera vertically in world space

                // Simple interpretation:
                // offset_y: along view axis (positive = further from table)
                let offset_along_view_y = settings.offset_y * pitch_rad.cos();
                let offset_along_view_z = settings.offset_y * pitch_rad.sin();

                // offset_z: perpendicular to view in vertical plane (positive = up in camera view)
                // This is perpendicular to the view direction in the YZ plane
                // If view is (0, -cos(pitch), -sin(pitch)), perpendicular up is (0, sin(pitch), -cos(pitch))
                // Wait, that would point "up" relative to the tilted view
                // But maybe offset_z is just world Z? Let's try that first.

                let vpx_x = cam_base_x + settings.offset_x;
                let vpx_y = cam_base_y + offset_along_view_y;
                let vpx_z = cam_base_z + offset_along_view_z + settings.offset_z;

                #[cfg(test)]
                {
                    println!("Camera mode calculation:");
                    println!(
                        "  look_at_fraction: {}, pitch_degrees: {}",
                        look_at_fraction, pitch_degrees
                    );
                    println!("  fit.z: {}, base_distance: {}", fit.z, base_distance);
                    println!("  look_at: ({}, {}, {})", look_at_x, look_at_y, look_at_z);
                    println!(
                        "  cam_base: ({}, {}, {})",
                        cam_base_x, cam_base_y, cam_base_z
                    );
                    println!(
                        "  offsets: x={}, y={}, z={}",
                        settings.offset_x, settings.offset_y, settings.offset_z
                    );
                    println!(
                        "  offset_along_view: y={}, z={}",
                        offset_along_view_y, offset_along_view_z
                    );
                    println!("  final vpx: ({}, {}, {})", vpx_x, vpx_y, vpx_z);
                }

                // Transform VPX -> glTF: (x, y, z) -> (x, z, y)
                let camera_x = vpx_x * VP_UNITS_TO_METERS;
                let camera_y = vpx_z * VP_UNITS_TO_METERS; // VPX Z (height) -> glTF Y
                let camera_z = vpx_y * VP_UNITS_TO_METERS; // VPX Y (depth) -> glTF Z

                let position = [camera_x, camera_y, camera_z];

                // Camera rotation: pitch down from horizontal
                // In glTF, cameras look down -Z axis by default. To look at the table (which is below),
                // we need to pitch DOWN, which is a NEGATIVE rotation around X axis.
                let rotation = Self::euler_to_quaternion_yxz(0.0, -pitch_rad);

                (position, rotation)
            }
        };

        Self {
            mode: settings.mode,
            position: camera_position,
            rotation,
            yfov: fov_rad,
            znear: 0.01,
            zfar: 100.0,
        }
    }

    /// Create a camera from FSS view settings (backward compatibility)
    #[allow(dead_code)]
    pub fn from_fss_settings(settings: &FssViewSettings, bounds: &TableBounds) -> Self {
        Self::from_view_settings(settings, bounds)
    }

    /// Create all three cameras from VPX data
    pub fn all_from_vpx(vpx: &VPX) -> [Self; 3] {
        let bounds = TableBounds::from_vpx(vpx);
        let settings = ViewSettings::all_from_vpx(vpx);
        [
            Self::from_view_settings(&settings[0], &bounds),
            Self::from_view_settings(&settings[1], &bounds),
            Self::from_view_settings(&settings[2], &bounds),
        ]
    }

    /// Convert Euler angles (YXZ order) to quaternion
    ///
    /// YXZ order: first rotate around Y (yaw), then around X (pitch)
    /// This is a common order for camera rotations.
    fn euler_to_quaternion_yxz(yaw: f32, pitch: f32) -> [f32; 4] {
        let half_yaw = yaw / 2.0;
        let half_pitch = pitch / 2.0;

        let cy = half_yaw.cos();
        let sy = half_yaw.sin();
        let cp = half_pitch.cos();
        let sp = half_pitch.sin();

        // Quaternion for YXZ rotation order
        // q = qY * qX (yaw first, then pitch)
        [
            cy * sp,  // x
            sy * cp,  // y
            -sy * sp, // z
            cy * cp,  // w
        ]
    }

    /// Generate the glTF camera definition JSON
    pub fn to_gltf_camera_json(&self) -> serde_json::Value {
        json!({
            "name": self.mode.camera_name(),
            "type": "perspective",
            "perspective": {
                "yfov": self.yfov,
                "znear": self.znear,
                "zfar": self.zfar
            }
        })
    }

    /// Generate the glTF camera node JSON
    pub fn to_gltf_node_json(&self, camera_index: usize) -> serde_json::Value {
        json!({
            "name": self.mode.camera_name(),
            "camera": camera_index,
            "translation": self.position,
            "rotation": self.rotation
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_default_bounds() -> TableBounds {
        // Default table: left=0, top=0, right=952, bottom=2162
        TableBounds {
            left: 0.0,
            top: 0.0,
            right: 952.0,
            bottom: 2162.0,
            glass_height: 300.0, // Typical glass height in VPU
        }
    }

    fn create_fss_settings() -> ViewSettings {
        ViewSettings {
            mode: ViewMode::Fss,
            layout_mode: ViewLayoutMode::Legacy,
            fov: 45.0,
            inclination: 52.0,
            offset_x: 0.0,
            offset_y: 30.0,
            offset_z: -50.0,
            scale_x: 1.2,
            scale_y: 1.1,
            scale_z: 1.0,
        }
    }

    fn create_desktop_settings() -> ViewSettings {
        ViewSettings {
            mode: ViewMode::Desktop,
            layout_mode: ViewLayoutMode::Legacy,
            fov: 45.0,
            inclination: 0.0,
            offset_x: 0.0,
            offset_y: 30.0,
            offset_z: -200.0,
            scale_x: 1.0,
            scale_y: 1.0,
            scale_z: 1.0,
        }
    }

    fn create_fullscreen_settings() -> ViewSettings {
        ViewSettings {
            mode: ViewMode::Fullscreen,
            layout_mode: ViewLayoutMode::Legacy,
            fov: 45.0,
            inclination: 0.0,
            offset_x: 110.0,
            offset_y: -86.0,
            offset_z: 400.0,
            scale_x: 1.3,
            scale_y: 1.41,
            scale_z: 1.0,
        }
    }

    #[test]
    fn test_camera_creation_does_not_panic() {
        let bounds = create_default_bounds();

        // Just verify that camera creation doesn't panic for all view modes
        let _desktop = GltfCamera::from_view_settings(&create_desktop_settings(), &bounds);
        let _fullscreen = GltfCamera::from_view_settings(&create_fullscreen_settings(), &bounds);
        let _fss = GltfCamera::from_view_settings(&create_fss_settings(), &bounds);
    }

    #[test]
    fn test_camera_quaternion_is_normalized() {
        let bounds = create_default_bounds();
        let settings = create_fss_settings();
        let camera = GltfCamera::from_view_settings(&settings, &bounds);

        let qx = camera.rotation[0];
        let qy = camera.rotation[1];
        let qz = camera.rotation[2];
        let qw = camera.rotation[3];

        let qlen = (qx * qx + qy * qy + qz * qz + qw * qw).sqrt();
        assert!(
            (qlen - 1.0).abs() < 0.001,
            "Quaternion should be normalized. Length = {}",
            qlen
        );
    }

    #[test]
    fn test_three_cameras_have_different_modes() {
        let bounds = create_default_bounds();

        let desktop = GltfCamera::from_view_settings(&create_desktop_settings(), &bounds);
        let fullscreen = GltfCamera::from_view_settings(&create_fullscreen_settings(), &bounds);
        let fss = GltfCamera::from_view_settings(&create_fss_settings(), &bounds);

        assert_eq!(desktop.mode, ViewMode::Desktop);
        assert_eq!(fullscreen.mode, ViewMode::Fullscreen);
        assert_eq!(fss.mode, ViewMode::Fss);
    }

    #[test]
    fn test_camera_names() {
        assert_eq!(ViewMode::Desktop.camera_name(), "DesktopCamera");
        assert_eq!(ViewMode::Fullscreen.camera_name(), "FullscreenCamera");
        assert_eq!(ViewMode::Fss.camera_name(), "FssCamera");
    }

    #[test]
    fn test_fit_camera_to_vertices_no_inclination() {
        // Test with 0° inclination (looking straight at the table)
        let bounds = create_default_bounds();
        let aspect = 16.0 / 9.0;
        let rotation = 0.0;
        let inclination = 0.0; // radians
        let fov = 45.0; // degrees
        let xlatez = 0.0;
        let layback = 0.0;
        let table_height_z = bounds.glass_height;

        let fit = fit_camera_to_vertices(
            &bounds,
            aspect,
            rotation,
            inclination,
            fov,
            xlatez,
            layback,
            table_height_z,
        );

        // With 0° inclination, the camera should be positioned to see the full table length
        // fit.z should be positive (camera distance from table)
        assert!(
            fit.z > 0.0,
            "Camera distance should be positive. Got z={}",
            fit.z
        );

        // fit.x should be near table center x (476 = 952/2)
        assert!(
            (fit.x - 476.0).abs() < 1.0,
            "Camera X should be near table center (476). Got x={}",
            fit.x
        );

        // fit.y should be near table center y (1081 = 2162/2)
        assert!(
            (fit.y - 1081.0).abs() < 1.0,
            "Camera Y should be near table center (1081). Got y={}",
            fit.y
        );
    }

    #[test]
    fn test_fit_camera_to_vertices_with_inclination() {
        // Test with 56° inclination (typical desktop view angle)
        let bounds = create_default_bounds();
        let aspect = 16.0 / 9.0;
        let rotation = 0.0;
        let inclination = 56.0_f32.to_radians();
        let fov = 39.0; // degrees
        let xlatez = 0.0;
        let layback = 0.0;
        let table_height_z = bounds.glass_height;

        let fit = fit_camera_to_vertices(
            &bounds,
            aspect,
            rotation,
            inclination,
            fov,
            xlatez,
            layback,
            table_height_z,
        );

        // With inclination, the camera distance should still be positive
        assert!(
            fit.z > 0.0,
            "Camera distance should be positive. Got z={}",
            fit.z
        );

        // The distance should be larger than the table dimensions to fit everything
        let table_height = bounds.bottom - bounds.top; // 2162
        assert!(
            fit.z > table_height * 0.5,
            "Camera distance should be significant. Got z={}, table_height={}",
            fit.z,
            table_height
        );
    }

    #[test]
    fn test_fit_camera_to_vertices_xlatez_offset() {
        // Test that xlatez (offset_z) is added to the result
        let bounds = create_default_bounds();
        let aspect = 16.0 / 9.0;
        let rotation = 0.0;
        let inclination = 0.0;
        let fov = 45.0;
        let layback = 0.0;
        let table_height_z = bounds.glass_height;

        let fit_no_offset = fit_camera_to_vertices(
            &bounds,
            aspect,
            rotation,
            inclination,
            fov,
            0.0,
            layback,
            table_height_z,
        );
        let fit_with_offset = fit_camera_to_vertices(
            &bounds,
            aspect,
            rotation,
            inclination,
            fov,
            100.0,
            layback,
            table_height_z,
        );

        // The z value should differ by exactly the xlatez amount
        let z_diff = fit_with_offset.z - fit_no_offset.z;
        assert!(
            (z_diff - 100.0).abs() < 0.001,
            "xlatez should be added to z. Expected diff=100, got diff={}",
            z_diff
        );
    }

    #[test]
    fn test_fit_camera_to_vertices_different_fov() {
        // Test that wider FOV results in closer camera distance
        let bounds = create_default_bounds();
        let aspect = 16.0 / 9.0;
        let rotation = 0.0;
        let inclination = 0.0;
        let xlatez = 0.0;
        let layback = 0.0;
        let table_height_z = bounds.glass_height;

        let fit_narrow = fit_camera_to_vertices(
            &bounds,
            aspect,
            rotation,
            inclination,
            30.0,
            xlatez,
            layback,
            table_height_z,
        );
        let fit_wide = fit_camera_to_vertices(
            &bounds,
            aspect,
            rotation,
            inclination,
            60.0,
            xlatez,
            layback,
            table_height_z,
        );

        // Wider FOV should result in closer camera (smaller z)
        assert!(
            fit_wide.z < fit_narrow.z,
            "Wider FOV should result in closer camera. narrow_z={}, wide_z={}",
            fit_narrow.z,
            fit_wide.z
        );
    }

    #[test]
    fn test_desktop_camera_position_matches_vpinball() {
        // Test case based on actual VPinball desktop view observation
        // Table: 952x2162 VPU
        //
        // VPinball POV screen shows (Legacy mode):
        // - Look At: 56% (0%=looking down, 100%=looking horizontal)
        //   56% -> pitch = 90° * (1 - 0.56) = 39.6° from horizontal
        // - X: 0 (screen left/right)
        // - Y: 99 VPU (screen up/down)
        // - Z: 0 (closer/farther along view axis)
        // - FOV: 39°
        //
        // Expected position in Blender (manually adjusted to match VPinball view):
        // - Blender X = 0.257m (centered on table)
        // - Blender Y = -1.50m (toward player)
        // - Blender Z = 0.564m (height above table)

        let bounds = TableBounds {
            left: 0.0,
            top: 0.0,
            right: 952.0,
            bottom: 2162.0,
            glass_height: 300.0,
        };

        let settings = ViewSettings {
            mode: ViewMode::Desktop,
            layout_mode: ViewLayoutMode::Legacy,
            fov: 39.0,
            inclination: 56.0, // 56% look-at -> 39.6° pitch
            offset_x: 0.0,
            offset_y: 99.0, // Screen Y offset
            offset_z: 0.0,  // Closer/farther
            scale_x: 1.24,
            scale_y: 1.24,
            scale_z: 1.0,
        };

        let camera = GltfCamera::from_view_settings(&settings, &bounds);

        println!(
            "Camera position: x={}, y={}, z={}",
            camera.position[0], camera.position[1], camera.position[2]
        );
        println!(
            "Camera rotation: x={}, y={}, z={}, w={}",
            camera.rotation[0], camera.rotation[1], camera.rotation[2], camera.rotation[3]
        );

        // Expected values from manually adjusting camera in Blender to match VPinball
        let expected_x = 0.257;
        let expected_y = 0.72; // Height (glTF Y = Blender Z)
        let expected_z = 1.52; // Toward player (glTF Z = -Blender Y)

        assert!(
            (camera.position[0] - expected_x).abs() < 0.05,
            "Camera X should be ~{}m. Got {}m",
            expected_x,
            camera.position[0]
        );

        assert!(
            (camera.position[1] - expected_y).abs() < 0.2,
            "Camera Y (height) should be ~{}m. Got {}m",
            expected_y,
            camera.position[1]
        );

        assert!(
            (camera.position[2] - expected_z).abs() < 0.35,
            "Camera Z (depth) should be ~{}m. Got {}m",
            expected_z,
            camera.position[2]
        );
    }
}
