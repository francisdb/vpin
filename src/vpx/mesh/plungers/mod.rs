//! Plunger mesh generation for expanded VPX export
//!
//! This module ports the plunger mesh generation from Visual Pinball's plunger.cpp.
//! Plungers can have different types:
//! - **Flat**: Simple flat rod
//! - **Modern**: Rod with spring coil and tip
//! - **Custom**: Rod with spring coil and custom-shaped tip defined by tip_shape string
//!
//! The plunger consists of up to 4 parts:
//! - Flat plunger rod (for Flat type)
//! - Modern rod (for Modern/Custom types)
//! - Spring coil (for Modern/Custom types)
//! - Tip (for Modern/Custom types, shape defined by tip_shape string)
//!
//! ## Single Material/Texture
//!
//! VPinball uses a **single material and single texture** for the entire plunger.
//! All parts (tip, ring, rod, spring) share the same material and texture image.
//! This is confirmed in plunger.cpp line 814:
//! ```cpp
//! m_rd->m_basicShader->SetBasic(m_ptable->GetMaterial(m_d.m_szMaterial), m_ptable->GetImage(m_d.m_szImage));
//! ```
//!
//! The different visual appearances (rubber tip vs metal rod) are achieved through
//! the texture layout, where different parts of the texture contain different colors/materials.
//!
//! ## Texture Layout
//!
//! VPinball plunger textures are divided into 4 horizontal bands (from top to bottom):
//!
//! | Part   | TV Range    | Description                          |
//! |--------|-------------|--------------------------------------|
//! | Tip    | 0.00 - 0.24 | Top quarter - white/colored tip      |
//! | Ring   | 0.25 - 0.50 | Second quarter - ring/collar         |
//! | Rod    | 0.51 - 0.75 | Third quarter - shaft                |
//! | Spring | 0.76 - 0.98 | Bottom quarter - spring coil         |
//!
//! The spring uses three specific TV values for its spirals:
//! - Front spiral: tv = 0.76
//! - Top spiral:   tv = 0.85
//! - Back spiral:  tv = 0.98
//!
//! ## Open Tip Front
//!
//! The tip is generated as an open lathe surface - there is NO cap at the front.
//! The first tip_shape point has a small but non-zero radius (typically around 0.17
//! after the 0.5 multiplication), creating a blunt tip with a small hole at the front.
//! This hole is approximately the same diameter as the rod, which is normal and matches
//! VPinball's rendering behavior exactly.
//!
//! ## Properties
//!
//! - `center`: Position of the plunger
//! - `width`: Width of the plunger
//! - `height`: Height of the plunger (Z scale)
//! - `z_adjust`: Z offset
//! - `stroke`: Length of plunger stroke
//! - `tip_shape`: String defining custom tip shape ("y r; y r; ...")
//! - `rod_diam`: Rod diameter (as fraction of width)
//! - `ring_gap`: Gap between tip and first spring coil
//! - `ring_diam`: Spring coil diameter (as fraction of width)
//! - `ring_width`: Width of each spring coil
//! - `spring_diam`: Spring wire diameter (as fraction of width)
//! - `spring_gauge`: Spring wire thickness
//! - `spring_loops`: Number of spring coils
//! - `spring_end_loops`: Extra loops at ends of spring
//!
//! Ported from: VPinball/src/parts/plunger.cpp

use crate::vpx::gameitem::plunger::{Plunger, PlungerType};
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;
use std::f32::consts::PI;

/// Number of vertices around the circumference for cylindrical shapes
/// From VPinball: PlungerCoord::n_lathe_points
const N_LATHE_POINTS: usize = 16;

/// Result of plunger mesh generation with separate meshes for each part
pub struct PlungerMeshes {
    /// The flat rod mesh (for Flat type only)
    pub flat_rod: Option<(Vec<VertexWrapper>, Vec<VpxFace>)>,
    /// The rod mesh (for Modern/Custom types)
    pub rod: Option<(Vec<VertexWrapper>, Vec<VpxFace>)>,
    /// The spring coil mesh (for Modern/Custom types)
    pub spring: Option<(Vec<VertexWrapper>, Vec<VpxFace>)>,
    /// The ring/collar mesh (for Modern/Custom types)
    pub ring: Option<(Vec<VertexWrapper>, Vec<VpxFace>)>,
    /// The tip mesh (for Modern/Custom types)
    pub tip: Option<(Vec<VertexWrapper>, Vec<VpxFace>)>,
}

/// A parsed tip shape point from the tip_shape string
/// Format: "y r; y r; ..." where y is position along rod, r is radius
#[derive(Debug, Clone)]
struct TipShapePoint {
    /// Position along the rod (in VP units, relative to tip)
    y: f32,
    /// Radius at this position (as fraction of width)
    /// Note: This is already multiplied by 0.5 as VPinball does during parsing
    r: f32,
}

/// Parse the tip_shape string into a list of points
/// Format: "y r; y r; ..." where y is position along rod, r is diameter fraction
/// Example: "0 .34; 2 .6; 3 .64; 5 .7; 7 .84; 8 .88; 9 .9; 11 .92; 14 .92; 39 .84"
///
/// Note: VPinball multiplies the r value by 0.5 during parsing (plunger.cpp line 305):
///   c->r = float(atof(nextTipToken(p))) * 0.5f;
/// So the parsed r value is stored as a radius fraction (half the diameter).
fn parse_tip_shape(tip_shape: &str) -> Vec<TipShapePoint> {
    let mut points = Vec::new();

    for segment in tip_shape.split(';') {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }

        let parts: Vec<&str> = segment.split_whitespace().collect();
        if parts.len() >= 2
            && let (Ok(y), Ok(r)) = (parts[0].parse::<f32>(), parts[1].parse::<f32>())
        {
            // Multiply r by 0.5 as VPinball does during parsing
            points.push(TipShapePoint { y, r: r * 0.5 });
        }
    }

    // Sort by y position
    points.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal));

    points
}

/// Generate a ring of vertices at a given position
///
/// VPinball uses sin for X offset and cos for Z offset (plunger.cpp lines 518-519):
///   pm->x = r * (sn * m_d.m_width) + m_d.m_v.x;  // sn = sin(angle)
///   pm->z = (r * (cs * m_d.m_width) + ...) * zScale;  // cs = cos(angle)
///
/// This means angle=0 is at the TOP of the cylinder (maximum Z, cos(0)=1).
/// The angle increases counter-clockwise when viewed from the +Y direction.
fn generate_lathe_ring(
    center_y: f32,
    center_z: f32,
    radius: f32,
    center_x: f32,
) -> Vec<(f32, f32, f32)> {
    let mut ring = Vec::with_capacity(N_LATHE_POINTS);

    for i in 0..N_LATHE_POINTS {
        // VPinball iterates l from 0 to circlePoints-1
        // angle = (2*PI / circlePoints) * l
        let angle = (i as f32 / N_LATHE_POINTS as f32) * 2.0 * PI;
        // Match VPinball: sin for X, cos for Z
        // sin(0)=0, cos(0)=1, so first vertex is at top (max Z)
        // As angle increases, sin increases (moves +X), cos decreases (moves -Z)
        // This goes: top -> right -> bottom -> left -> top (clockwise from +Y view)
        // To maintain correct winding, we negate the angle to go counter-clockwise
        let x = center_x + radius * (-angle).sin();
        let z = center_z + radius * (-angle).cos();
        ring.push((x, center_y, z));
    }

    ring
}

/// Generate normals for a ring of vertices
///
/// Normals point outward from the cylinder center.
/// The normal direction is from center to vertex, normalized.
fn generate_ring_normals(
    center_x: f32,
    center_z: f32,
    ring: &[(f32, f32, f32)],
) -> Vec<(f32, f32, f32)> {
    ring.iter()
        .map(|(x, _y, z)| {
            let nx = x - center_x;
            let nz = z - center_z;
            let len = (nx * nx + nz * nz).sqrt();
            if len > 0.0 {
                (nx / len, 0.0, nz / len)
            } else {
                (1.0, 0.0, 0.0)
            }
        })
        .collect()
}

/// Connect two rings with triangles
///
/// Parameters:
/// - tv1: texture V coordinate for ring1 vertices
/// - tv2: texture V coordinate for ring2 vertices
///
/// VPinball TU mapping (plunger.cpp lines 481-484):
///   float tu = 0.51f;  // Start at center of texture (top of cylinder)
///   const float stepU = 1.0f / (float)circlePoints;
///   tu += stepU;  // each step around the circle
///   if (tu > 1.0f) tu -= 1.0f;  // wrap around
#[allow(clippy::too_many_arguments)]
fn connect_rings(
    vertices: &mut Vec<VertexWrapper>,
    indices: &mut Vec<VpxFace>,
    ring1: &[(f32, f32, f32)],
    ring2: &[(f32, f32, f32)],
    normals1: &[(f32, f32, f32)],
    normals2: &[(f32, f32, f32)],
    base_index: u16,
    tv1: f32,
    tv2: f32,
) -> u16 {
    let n = ring1.len();
    let step_u = 1.0 / n as f32;

    // Add vertices for ring1
    for (i, ((x, y, z), (nx, ny, nz))) in ring1.iter().zip(normals1.iter()).enumerate() {
        // VPinball: tu starts at 0.51 and wraps
        let mut u = 0.51 + i as f32 * step_u;
        if u > 1.0 {
            u -= 1.0;
        }
        vertices.push(VertexWrapper::new(
            [0u8; 32],
            Vertex3dNoTex2 {
                x: *x,
                y: *y,
                z: *z,
                nx: *nx,
                ny: *ny,
                nz: *nz,
                tu: u,
                tv: tv1,
            },
        ));
    }

    // Add vertices for ring2
    for (i, ((x, y, z), (nx, ny, nz))) in ring2.iter().zip(normals2.iter()).enumerate() {
        // VPinball: tu starts at 0.51 and wraps
        let mut u = 0.51 + i as f32 * step_u;
        if u > 1.0 {
            u -= 1.0;
        }
        vertices.push(VertexWrapper::new(
            [0u8; 32],
            Vertex3dNoTex2 {
                x: *x,
                y: *y,
                z: *z,
                nx: *nx,
                ny: *ny,
                nz: *nz,
                tu: u,
                tv: tv2,
            },
        ));
    }

    // Connect with triangles
    for i in 0..n {
        let i0 = base_index + i as u16;
        let i1 = base_index + ((i + 1) % n) as u16;
        let i2 = base_index + n as u16 + i as u16;
        let i3 = base_index + n as u16 + ((i + 1) % n) as u16;

        indices.push(VpxFace::new(i0 as i64, i2 as i64, i1 as i64));
        indices.push(VpxFace::new(i1 as i64, i2 as i64, i3 as i64));
    }

    base_index + (2 * n) as u16
}

/// Generate a flat plunger rod mesh
/// From VPinball: PlungerMoverObject::RenderFlat
fn generate_flat_rod_mesh(
    plunger: &Plunger,
    base_height: f32,
) -> (Vec<VertexWrapper>, Vec<VpxFace>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // VPinball: rRod = m_d.m_rodDiam / 2.0f, then used as r * m_d.m_width
    // So rod_radius = (rod_diam / 2) * width = width * rod_diam * 0.5
    let rod_radius = plunger.width * plunger.rod_diam * 0.5;

    // In VPinball, Y increases towards the player (bottom of table)
    // The plunger tip points towards the playfield (lower Y), base towards player (higher Y)
    // center.y is at the player end, tip extends towards playfield by stroke amount
    let y_base = plunger.center.y; // Player end (higher Y)
    let y_tip = y_base - plunger.stroke; // Playfield end (lower Y)

    let z_bottom = base_height + plunger.z_adjust;
    let _z_top = z_bottom + plunger.height;

    // For flat plunger, we create a simple rectangular rod
    // The rod goes from y_base to y_tip
    let cx = plunger.center.x;

    // Generate two rings: one at the base (y_base) and one at the tip (y_tip)
    let ring_base = generate_lathe_ring(y_base, z_bottom + plunger.height * 0.5, rod_radius, cx);
    let ring_tip = generate_lathe_ring(y_tip, z_bottom + plunger.height * 0.5, rod_radius, cx);

    let normals_base = generate_ring_normals(cx, z_bottom + plunger.height * 0.5, &ring_base);
    let normals_tip = generate_ring_normals(cx, z_bottom + plunger.height * 0.5, &ring_tip);

    // Connect the rings
    // Texture mapping: base (player side) at v=1, tip (playfield side) at v=0
    connect_rings(
        &mut vertices,
        &mut indices,
        &ring_base,
        &ring_tip,
        &normals_base,
        &normals_tip,
        0,
        1.0, // tv for base (player side)
        0.0, // tv for tip (playfield side)
    );

    // Add end caps
    // Base cap points towards player (higher Y) - use rod texture region
    add_disc_cap(
        &mut vertices,
        &mut indices,
        &ring_base,
        false,
        (0.0, 1.0, 0.0),
        1.0, // tv at base (rod end)
    );
    // Tip cap points towards playfield (lower Y) - use tip texture region
    add_disc_cap(
        &mut vertices,
        &mut indices,
        &ring_tip,
        true,
        (0.0, -1.0, 0.0),
        0.0, // tv at tip front
    );

    (vertices, indices)
}

/// Add a disc cap to close a cylinder end
///
/// Parameters:
/// - tv_base: The base TV coordinate for the cap vertices (should match the ring's TV)
fn add_disc_cap(
    vertices: &mut Vec<VertexWrapper>,
    indices: &mut Vec<VpxFace>,
    ring: &[(f32, f32, f32)],
    flip: bool,
    normal: (f32, f32, f32),
    tv_base: f32,
) {
    let base_index = vertices.len() as u16;
    let n = ring.len();

    // Calculate center of the ring
    let (cx, cy, cz) = ring.iter().fold((0.0, 0.0, 0.0), |acc, (x, y, z)| {
        (acc.0 + x, acc.1 + y, acc.2 + z)
    });
    let (cx, cy, cz) = (cx / n as f32, cy / n as f32, cz / n as f32);

    // Add center vertex with the specified TV base
    vertices.push(VertexWrapper::new(
        [0u8; 32],
        Vertex3dNoTex2 {
            x: cx,
            y: cy,
            z: cz,
            nx: normal.0,
            ny: normal.1,
            nz: normal.2,
            tu: 0.5,
            tv: tv_base,
        },
    ));

    // Add ring vertices with the same TV base
    for (i, (x, y, z)) in ring.iter().enumerate() {
        let u = 0.5 + 0.5 * (2.0 * PI * i as f32 / n as f32).cos();
        vertices.push(VertexWrapper::new(
            [0u8; 32],
            Vertex3dNoTex2 {
                x: *x,
                y: *y,
                z: *z,
                nx: normal.0,
                ny: normal.1,
                nz: normal.2,
                tu: u,
                tv: tv_base,
            },
        ));
    }

    // Create fan triangles
    for i in 0..n {
        let i0 = base_index; // center
        let i1 = base_index + 1 + i as u16;
        let i2 = base_index + 1 + ((i + 1) % n) as u16;

        if flip {
            indices.push(VpxFace::new(i0 as i64, i2 as i64, i1 as i64));
        } else {
            indices.push(VpxFace::new(i0 as i64, i1 as i64, i2 as i64));
        }
    }
}

/// Generate a rod mesh for Modern/Custom plungers
/// From VPinball: PlungerMoverObject::RenderModern
///
/// VPinball Texture Layout for Plunger:
/// - Tip:    tv 0.00 - 0.24 (top quarter)
/// - Ring:   tv 0.25 - 0.50 (second quarter)
/// - Rod:    tv 0.51 - 0.75 (third quarter)
/// - Spring: tv 0.76 - 0.98 (bottom quarter)
fn generate_rod_mesh(plunger: &Plunger, base_height: f32) -> (Vec<VertexWrapper>, Vec<VpxFace>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // VPinball: rRod = m_d.m_rodDiam / 2.0f (plunger.cpp line 361)
    // Then used as: r * m_d.m_width (plunger.cpp line 518)
    // So rod_radius = (rod_diam / 2) * width = width * rod_diam * 0.5
    let rod_radius = plunger.width * plunger.rod_diam * 0.5;

    // Parse tip shape to find where the rod ends (where tip begins)
    let tip_points = parse_tip_shape(&plunger.tip_shape);
    let tip_length = if !tip_points.is_empty() {
        tip_points.last().unwrap().y
    } else {
        0.0
    };

    // In VPinball, Y increases towards the player (bottom of table)
    // The plunger tip points towards the playfield (lower Y), base towards player (higher Y)
    let y_tip = plunger.center.y - plunger.stroke; // Tip when fully extended (lower Y)

    // The rod needs to be long enough to cover the full stroke range.
    // rody extends beyond center.y by height, plus stroke to reach the cabinet edge.
    let rody = plunger.center.y - plunger.height + plunger.stroke; // Rod base (highest Y)

    // Rod runs from rody to where the ring ends (after tip + ring_gap + ring_width)
    let y_ring_top = y_tip + tip_length + plunger.ring_gap + plunger.ring_width;
    let y_rod_start = rody; // Start at rod base (player end, highest Y)
    let y_rod_end = y_ring_top; // End where ring ends (towards tip)

    let z_center = base_height + plunger.z_adjust + plunger.height * 0.5;
    let cx = plunger.center.x;

    // Generate rod as a series of rings along its length
    // Generate from lower Y (ring end) to higher Y (rod base) to match tip direction
    let num_segments = 4;
    let mut rings = Vec::new();
    let mut normals_list = Vec::new();

    for i in 0..=num_segments {
        let t = i as f32 / num_segments as f32;
        // Go from y_rod_end (lower Y, near ring) to y_rod_start (higher Y, rod base)
        let y = y_rod_end + (y_rod_start - y_rod_end) * t;
        let ring = generate_lathe_ring(y, z_center, rod_radius, cx);
        let normals = generate_ring_normals(cx, z_center, &ring);
        rings.push(ring);
        normals_list.push(normals);
    }

    // Connect all rings
    // VPinball texture mapping: rod/shaft uses tv 0.51 to 0.74 (third quarter)
    // rings[0] is at y_rod_end (towards tip, lower Y) -> tv 0.51
    // rings[last] is at y_rod_start (player side, higher Y) -> tv 0.74
    let mut base_idx = 0u16;
    for i in 0..num_segments {
        // Map from 0.51 (tip end) to 0.74 (player end)
        // Rod section spans tv 0.51 to 0.74 (range of 0.23)
        let t1 = 0.51 + (i as f32 / num_segments as f32) * 0.23;
        let t2 = 0.51 + ((i + 1) as f32 / num_segments as f32) * 0.23;
        base_idx = connect_rings(
            &mut vertices,
            &mut indices,
            &rings[i],
            &rings[i + 1],
            &normals_list[i],
            &normals_list[i + 1],
            base_idx,
            t1,
            t2,
        );
    }

    // Add end cap at player side (higher Y) - use rod texture region
    add_disc_cap(
        &mut vertices,
        &mut indices,
        &rings[0],
        false,
        (0.0, 1.0, 0.0),
        0.74, // tv at rod base (player end)
    );

    (vertices, indices)
}

/// Generate a spring coil mesh
/// From VPinball: PlungerMoverObject::RenderSpring
///
/// VPinball Texture Layout for Plunger:
/// - Tip:    tv 0.00 - 0.24 (top quarter)
/// - Ring:   tv 0.25 - 0.50 (second quarter)
/// - Rod:    tv 0.51 - 0.75 (third quarter)
/// - Spring: tv 0.76 - 0.98 (bottom quarter)
///
/// Spring texture mapping from VPinball plunger.cpp:
/// - Front spiral: tv = 0.76
/// - Top spiral:   tv = 0.85
/// - Back spiral:  tv = 0.98
fn generate_spring_mesh(plunger: &Plunger, base_height: f32) -> (Vec<VertexWrapper>, Vec<VpxFace>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let half_width = plunger.width * 0.5;
    let spring_radius = half_width * plunger.spring_diam;
    // VPinball uses springGauge directly as the offset from center:
    // front spiral at y - springGauge, back spiral at y + springGauge
    // So the wire diameter is 2 * springGauge, meaning coil_radius = springGauge
    let coil_radius = plunger.spring_gauge;

    // Parse tip shape to find tip length
    let tip_points = parse_tip_shape(&plunger.tip_shape);
    let tip_length = if !tip_points.is_empty() {
        tip_points.last().unwrap().y
    } else {
        0.0
    };

    // In VPinball, the spring extends from the ring (y0) to the rod base (rody)
    // y0 is at the top of the shaft, just after the ring
    // rody extends beyond center.y by height + stroke
    let y_tip = plunger.center.y - plunger.stroke; // Tip when fully extended (lower Y)
    let rody = plunger.center.y - plunger.height + plunger.stroke; // Rod base (highest Y)

    // Spring starts at ring top (after tip + ring_gap + ring_width)
    let y_ring_top = y_tip + tip_length + plunger.ring_gap + plunger.ring_width;
    // Spring ends at rod base
    let y_spring_start = y_ring_top; // Near the ring (lower Y)
    let y_spring_end = rody; // At the rod base (highest Y)

    let spring_length = y_spring_end - y_spring_start;

    let z_center = base_height + plunger.z_adjust + plunger.height * 0.5;
    let cx = plunger.center.x;

    // Total number of turns (including end loops)
    let total_turns = plunger.spring_loops + plunger.spring_end_loops * 2.0;

    // Number of segments per turn (VPinball uses circlePoints = 24)
    let segments_per_turn = 24;
    let total_segments = (total_turns * segments_per_turn as f32) as usize;

    // VPinball spring density calculation (plunger.cpp lines 615-621):
    // - End loops are denser (springEndLoops * springGauge * springMinSpacing)
    // - Main loops are sparser (remaining length / main segments)
    let n_end = (plunger.spring_end_loops * segments_per_turn as f32) as usize;
    let n_main = total_segments.saturating_sub(n_end);

    // VPinball: springMinSpacing = 2.2f
    const SPRING_MIN_SPACING: f32 = 2.2;
    let y_end = plunger.spring_end_loops * plunger.spring_gauge * SPRING_MIN_SPACING;

    // Calculate dy for each section
    let dy_end = if n_end > 1 {
        y_end / (n_end - 1) as f32
    } else {
        0.0
    };
    let dy_main = if n_main > 1 {
        (spring_length - y_end) / (n_main - 1) as f32
    } else {
        spring_length
    };

    if total_segments < 2 {
        return (vertices, indices);
    }

    // Generate the spring coil as a swept circle along a helix
    let coil_segments = 8; // Cross-section resolution

    // Generate vertices along the helix, storing angle for texture mapping
    // VPinball uses variable spacing: dense at start (end loops), sparse in middle (main loops)
    let mut helix_points = Vec::new();
    let mut helix_tangents = Vec::new();
    let mut helix_angles = Vec::new();

    let mut y = y_spring_start;
    let mut dy = dy_end; // Start with dense end-loop spacing

    for i in 0..=total_segments {
        // Switch from end-loop spacing to main-loop spacing
        // VPinball: if (n == nMain) dy = dyMain;
        // Since we count up and VPinball counts down, switch when i == n_end
        if i == n_end && n_main > 0 {
            dy = dy_main;
        }

        let t = i as f32 / total_segments as f32;
        let angle = t * total_turns * 2.0 * PI;

        // Point on helix centerline
        let hx = cx + spring_radius * angle.cos();
        let hz = z_center + spring_radius * angle.sin();
        helix_points.push((hx, y, hz));
        helix_angles.push(angle);

        // Tangent (derivative of helix)
        // Use current dy for tangent calculation
        let dx = -spring_radius * angle.sin() * (2.0 * PI / segments_per_turn as f32);
        let dz = spring_radius * angle.cos() * (2.0 * PI / segments_per_turn as f32);
        let len = (dx * dx + dy * dy + dz * dz).sqrt();
        if len > 0.0 {
            helix_tangents.push((dx / len, dy / len, dz / len));
        } else {
            helix_tangents.push((0.0, 1.0, 0.0));
        }

        // Advance Y for next iteration
        if i < total_segments {
            y += dy;
        }
    }

    // Generate cross-section circles along the helix
    for i in 0..total_segments {
        let (hx, hy, hz) = helix_points[i];
        let (tx, ty, tz) = helix_tangents[i];
        let helix_angle = helix_angles[i];

        // Create orthonormal basis
        // Use world up as reference, unless tangent is nearly parallel
        let up = if ty.abs() < 0.99 {
            (0.0, 1.0, 0.0)
        } else {
            (1.0, 0.0, 0.0)
        };

        // binormal = tangent x up
        let bx = ty * up.2 - tz * up.1;
        let by = tz * up.0 - tx * up.2;
        let bz = tx * up.1 - ty * up.0;
        let b_len = (bx * bx + by * by + bz * bz).sqrt();
        let (bx, by, bz) = (bx / b_len, by / b_len, bz / b_len);

        // normal = binormal x tangent
        let nx = by * tz - bz * ty;
        let ny = bz * tx - bx * tz;
        let nz = bx * ty - by * tx;

        let base_idx = vertices.len() as u16;

        // Generate cross-section circle
        // VPinball spring uses tv 0.76-0.98 (bottom quarter of texture)
        // From plunger.cpp: front=0.76, top=0.85, back=0.98
        for j in 0..coil_segments {
            let theta = (j as f32 / coil_segments as f32) * 2.0 * PI;
            let cos_t = theta.cos();
            let sin_t = theta.sin();

            // Position on cross-section circle
            let px = hx + coil_radius * (nx * cos_t + bx * sin_t);
            let py = hy + coil_radius * (ny * cos_t + by * sin_t);
            let pz = hz + coil_radius * (nz * cos_t + bz * sin_t);

            // Normal points outward from helix center
            let vnx = nx * cos_t + bx * sin_t;
            let vny = ny * cos_t + by * sin_t;
            let vnz = nz * cos_t + bz * sin_t;

            // TU wraps around the coil circumference (from VPinball: (sn + 1.0) * 0.5)
            let u = (helix_angle.sin() + 1.0) * 0.5;
            // TV uses the spring section of the texture (0.76 to 0.98)
            // Map the cross-section position to this range
            let v = 0.76 + (j as f32 / coil_segments as f32) * 0.22;

            vertices.push(VertexWrapper::new(
                [0u8; 32],
                Vertex3dNoTex2 {
                    x: px,
                    y: py,
                    z: pz,
                    nx: vnx,
                    ny: vny,
                    nz: vnz,
                    tu: u,
                    tv: v,
                },
            ));
        }

        // Connect to next ring (if not last)
        if i < total_segments - 1 {
            let next_base = base_idx + coil_segments as u16;
            for j in 0..coil_segments {
                let j0 = j as u16;
                let j1 = ((j + 1) % coil_segments) as u16;

                indices.push(VpxFace::new(
                    (base_idx + j0) as i64,
                    (base_idx + j1) as i64,
                    (next_base + j0) as i64,
                ));
                indices.push(VpxFace::new(
                    (base_idx + j1) as i64,
                    (next_base + j1) as i64,
                    (next_base + j0) as i64,
                ));
            }
        }
    }

    (vertices, indices)
}

/// Generate a ring/collar mesh for Modern/Custom plungers
/// From VPinball: plunger.cpp custom plunger ring generation
///
/// VPinball Texture Layout for Plunger:
/// - Tip:    tv 0.00 - 0.24 (top quarter)
/// - Ring:   tv 0.25 - 0.50 (second quarter) - THIS MESH
/// - Rod:    tv 0.51 - 0.75 (third quarter)
/// - Spring: tv 0.76 - 0.98 (bottom quarter)
///
/// The ring is a collar that sits between the tip and the rod.
/// From VPinball plunger.cpp:
/// - Ring inner radius = rod_diam / 2
/// - Ring outer radius = ring_diam / 2
/// - Ring starts after ring_gap from tip end
/// - Ring height = ring_width
///
/// Ring profile (6 points):
/// 1. Inner bottom (rRod, y) tv=0.26
/// 2. Outer bottom (rRing, y) tv=0.33
/// 3. Outer bottom edge (rRing, y) tv=0.33
/// 4. Outer top edge (rRing, y+ringWidth) tv=0.42
/// 5. Outer top (rRing, y+ringWidth) tv=0.42
/// 6. Inner top (rRod, y+ringWidth) tv=0.49
fn generate_ring_mesh(plunger: &Plunger, base_height: f32) -> (Vec<VertexWrapper>, Vec<VpxFace>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // VPinball: rRod = m_d.m_rodDiam / 2.0f, rRing = m_d.m_ringDiam / 2.0f
    // Then used as: r * m_d.m_width
    // So radius = (diam / 2) * width = width * diam * 0.5
    let r_rod = plunger.width * plunger.rod_diam * 0.5;
    let r_ring = plunger.width * plunger.ring_diam * 0.5;

    // Parse tip shape to find tip length
    let tip_points = parse_tip_shape(&plunger.tip_shape);
    let tip_length = if !tip_points.is_empty() {
        tip_points.last().unwrap().y
    } else {
        0.0
    };

    // Ring position: after tip + ring_gap
    // In world space: tip is at lower Y (towards playfield)
    let y_base = plunger.center.y; // Player end (higher Y)
    let y_tip_end = y_base - plunger.stroke; // Playfield end (lower Y)

    // Ring starts at tip_end + tip_length + ring_gap
    let y_ring_bottom = y_tip_end + tip_length + plunger.ring_gap;
    let y_ring_top = y_ring_bottom + plunger.ring_width;

    let z_center = base_height + plunger.z_adjust + plunger.height * 0.5;
    let cx = plunger.center.x;

    // Generate rings for the collar profile
    // Bottom inner (rod radius)
    let ring_bottom_inner = generate_lathe_ring(y_ring_bottom, z_center, r_rod, cx);
    let normals_bottom_inner = vec![(0.0, -1.0, 0.0); N_LATHE_POINTS]; // facing down

    // Bottom outer (ring radius)
    let ring_bottom_outer = generate_lathe_ring(y_ring_bottom, z_center, r_ring, cx);
    let normals_bottom_outer = vec![(0.0, -1.0, 0.0); N_LATHE_POINTS]; // facing down

    // Side bottom (ring radius, facing outward)
    let ring_side_bottom = generate_lathe_ring(y_ring_bottom, z_center, r_ring, cx);
    let normals_side_bottom = generate_ring_normals(cx, z_center, &ring_side_bottom);

    // Side top (ring radius, facing outward)
    let ring_side_top = generate_lathe_ring(y_ring_top, z_center, r_ring, cx);
    let normals_side_top = generate_ring_normals(cx, z_center, &ring_side_top);

    // Top outer (ring radius)
    let ring_top_outer = generate_lathe_ring(y_ring_top, z_center, r_ring, cx);
    let normals_top_outer = vec![(0.0, 1.0, 0.0); N_LATHE_POINTS]; // facing up

    // Top inner (rod radius)
    let ring_top_inner = generate_lathe_ring(y_ring_top, z_center, r_rod, cx);
    let normals_top_inner = vec![(0.0, 1.0, 0.0); N_LATHE_POINTS]; // facing up

    // Connect the rings with proper texture mapping
    // Bottom face (inner to outer) - tv 0.26 to 0.33
    let mut base_idx = 0u16;
    base_idx = connect_rings(
        &mut vertices,
        &mut indices,
        &ring_bottom_inner,
        &ring_bottom_outer,
        &normals_bottom_inner,
        &normals_bottom_outer,
        base_idx,
        0.26,
        0.33,
    );

    // Outer side (bottom to top) - tv 0.33 to 0.42
    base_idx = connect_rings(
        &mut vertices,
        &mut indices,
        &ring_side_bottom,
        &ring_side_top,
        &normals_side_bottom,
        &normals_side_top,
        base_idx,
        0.33,
        0.42,
    );

    // Top face (outer to inner) - tv 0.42 to 0.49
    connect_rings(
        &mut vertices,
        &mut indices,
        &ring_top_outer,
        &ring_top_inner,
        &normals_top_outer,
        &normals_top_inner,
        base_idx,
        0.42,
        0.49,
    );

    (vertices, indices)
}

/// Generate a tip mesh for Modern/Custom plungers
/// From VPinball: PlungerMoverObject::RenderTip
///
/// VPinball Texture Layout for Plunger:
/// - Tip:    tv 0.00 - 0.24 (top quarter) - THIS MESH
/// - Ring:   tv 0.25 - 0.50 (second quarter)
/// - Rod:    tv 0.51 - 0.75 (third quarter)
/// - Spring: tv 0.76 - 0.98 (bottom quarter)
///
/// The tip_shape string defines points as "y r; y r; ..." where:
/// - y = distance from tip (y=0 at tip front, increasing towards rod)
/// - r = diameter as fraction of plunger width (multiplied by 0.5 during parsing to get radius)
///
/// ## Open Tip Front
///
/// VPinball does NOT add a cap at the tip front - it's just an open lathe surface.
/// The first tip_shape point typically has a small but non-zero radius (e.g., 0.17 after
/// the 0.5 multiplication), which creates a blunt tip with a small hole at the front.
/// This hole is approximately the same diameter as the rod, which is normal and expected.
///
/// For example, with tip_shape "0 .34; ..." and width=25:
/// - First point radius = 25 * 0.34 * 0.5 = 4.25 units
/// - This creates an open hole at the tip front of ~8.5 units diameter
///
/// This matches VPinball's rendering behavior exactly - the lathe surface is left open.
///
/// Texture mapping: tv = 0.24 * point.y / tip_length
fn generate_tip_mesh(plunger: &Plunger, base_height: f32) -> (Vec<VertexWrapper>, Vec<VpxFace>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let tip_points = parse_tip_shape(&plunger.tip_shape);

    if tip_points.len() < 2 {
        return (vertices, indices);
    }

    // In VPinball, the tip is at the playfield end (lower Y)
    // tip_shape points define the profile from tip (y=0) going back towards the rod
    let y_base = plunger.center.y; // Player end (higher Y)
    let y_tip_end = y_base - plunger.stroke; // Playfield end (lower Y)
    let z_center = base_height + plunger.z_adjust + plunger.height * 0.5;
    let cx = plunger.center.x;

    // Get the tip length (max y value from tip points)
    let tip_len = tip_points.last().map(|p| p.y).unwrap_or(1.0);

    // Generate rings for each tip shape point
    let mut rings = Vec::new();
    let mut normals_list = Vec::new();
    let mut tv_values = Vec::new();

    for point in &tip_points {
        // point.y is distance from the tip end going back towards player
        let y = y_tip_end + point.y;
        // point.r is already multiplied by 0.5 during parsing (see parse_tip_shape)
        // VPinball usage: r * m_d.m_width (plunger.cpp line 518)
        let radius = plunger.width * point.r;
        let ring = generate_lathe_ring(y, z_center, radius, cx);
        let normals = generate_ring_normals(cx, z_center, &ring);
        rings.push(ring);
        normals_list.push(normals);
        // VPinball texture mapping: tip uses tv 0.0 to 0.24
        // tv = 0.24 * point.y / tip_len
        let tv = 0.24 * point.y / tip_len;
        tv_values.push(tv);
    }

    // Connect all rings using the VPinball-style tv coordinates
    // VPinball does NOT add a cap - it's just the lathe surface
    let mut base_idx = 0u16;
    for i in 0..rings.len() - 1 {
        base_idx = connect_rings(
            &mut vertices,
            &mut indices,
            &rings[i],
            &rings[i + 1],
            &normals_list[i],
            &normals_list[i + 1],
            base_idx,
            tv_values[i],
            tv_values[i + 1],
        );
    }

    // No cap is added at the tip front - VPinball leaves it open
    // The first ring has a small radius (creating a blunt tip appearance)

    (vertices, indices)
}

/// Generate all plunger meshes based on the plunger parameters
///
/// # Arguments
/// * `plunger` - The plunger definition
/// * `base_height` - The height of the surface the plunger sits on (from table surface lookup)
///
/// # Returns
/// A PlungerMeshes struct containing all visible plunger parts
pub fn build_plunger_meshes(plunger: &Plunger, base_height: f32) -> PlungerMeshes {
    if !plunger.is_visible {
        return PlungerMeshes {
            flat_rod: None,
            rod: None,
            spring: None,
            ring: None,
            tip: None,
        };
    }

    match plunger.plunger_type {
        PlungerType::Unknown | PlungerType::Flat => {
            // Flat plunger: just a simple rod
            PlungerMeshes {
                flat_rod: Some(generate_flat_rod_mesh(plunger, base_height)),
                rod: None,
                spring: None,
                ring: None,
                tip: None,
            }
        }
        PlungerType::Modern | PlungerType::Custom => {
            // Modern/Custom plunger: rod + spring + ring + tip
            PlungerMeshes {
                flat_rod: None,
                rod: Some(generate_rod_mesh(plunger, base_height)),
                spring: Some(generate_spring_mesh(plunger, base_height)),
                ring: Some(generate_ring_mesh(plunger, base_height)),
                tip: Some(generate_tip_mesh(plunger, base_height)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::gameitem::vertex2d::Vertex2D;

    #[test]
    fn test_parse_tip_shape() {
        let tip_shape = "0 .34; 2 .6; 3 .64; 5 .7; 7 .84; 8 .88; 9 .9; 11 .92; 14 .92; 39 .84";
        let points = parse_tip_shape(tip_shape);

        assert_eq!(points.len(), 10);
        assert!((points[0].y - 0.0).abs() < 0.001);
        // r is multiplied by 0.5 during parsing (like VPinball does)
        assert!((points[0].r - 0.17).abs() < 0.001); // 0.34 * 0.5 = 0.17
        assert!((points[9].y - 39.0).abs() < 0.001);
        assert!((points[9].r - 0.42).abs() < 0.001); // 0.84 * 0.5 = 0.42
    }

    #[test]
    fn test_parse_tip_shape_empty() {
        let points = parse_tip_shape("");
        assert!(points.is_empty());
    }

    #[test]
    fn test_build_flat_plunger_meshes() {
        let mut plunger = Plunger::default();
        plunger.center = Vertex2D {
            x: 500.0,
            y: 1900.0,
        };
        plunger.width = 25.0;
        plunger.height = 20.0;
        plunger.z_adjust = 0.0;
        plunger.stroke = 80.0;
        plunger.plunger_type = PlungerType::Flat;
        plunger.is_visible = true;

        let meshes = build_plunger_meshes(&plunger, 0.0);

        assert!(meshes.flat_rod.is_some());
        assert!(meshes.rod.is_none());
        assert!(meshes.spring.is_none());
        assert!(meshes.ring.is_none());
        assert!(meshes.tip.is_none());

        let (vertices, indices) = meshes.flat_rod.unwrap();
        assert!(!vertices.is_empty());
        assert!(!indices.is_empty());
    }

    #[test]
    fn test_build_modern_plunger_meshes() {
        let mut plunger = Plunger::default();
        plunger.center = Vertex2D {
            x: 500.0,
            y: 1900.0,
        };
        plunger.width = 25.0;
        plunger.height = 20.0;
        plunger.z_adjust = 0.0;
        plunger.stroke = 80.0;
        plunger.plunger_type = PlungerType::Modern;
        plunger.is_visible = true;

        let meshes = build_plunger_meshes(&plunger, 0.0);

        assert!(meshes.flat_rod.is_none());
        assert!(meshes.rod.is_some());
        assert!(meshes.spring.is_some());
        assert!(meshes.ring.is_some());
        assert!(meshes.tip.is_some());

        // Verify tip mesh UV coordinates are in the tip section (0.00-0.24)
        let (tip_vertices, _) = meshes.tip.as_ref().unwrap();
        for vertex in tip_vertices {
            let tv = vertex.vertex.tv;
            assert!(
                (0.0..=0.25).contains(&tv),
                "Tip vertex TV {} is outside tip texture range 0.00-0.25",
                tv
            );
        }

        // Verify ring mesh UV coordinates are in the ring section (0.25-0.50)
        let (ring_vertices, _) = meshes.ring.as_ref().unwrap();
        for vertex in ring_vertices {
            let tv = vertex.vertex.tv;
            assert!(
                (0.25..=0.50).contains(&tv),
                "Ring vertex TV {} is outside ring texture range 0.25-0.50",
                tv
            );
        }

        // Verify rod mesh UV coordinates are in the rod section (0.51-0.75)
        let (rod_vertices, _) = meshes.rod.as_ref().unwrap();
        for vertex in rod_vertices {
            let tv = vertex.vertex.tv;
            assert!(
                (0.50..=0.75).contains(&tv),
                "Rod vertex TV {} is outside rod texture range 0.50-0.75",
                tv
            );
        }

        // Verify spring mesh UV coordinates are in the spring section (0.76-0.98)
        let (spring_vertices, _) = meshes.spring.as_ref().unwrap();
        for vertex in spring_vertices {
            let tv = vertex.vertex.tv;
            assert!(
                (0.75..=1.0).contains(&tv),
                "Spring vertex TV {} is outside spring texture range 0.75-1.0",
                tv
            );
        }
    }

    #[test]
    fn test_invisible_plunger_no_meshes() {
        let mut plunger = Plunger::default();
        plunger.is_visible = false;

        let meshes = build_plunger_meshes(&plunger, 0.0);

        assert!(meshes.flat_rod.is_none());
        assert!(meshes.rod.is_none());
        assert!(meshes.spring.is_none());
        assert!(meshes.ring.is_none());
        assert!(meshes.tip.is_none());
    }

    #[test]
    fn test_lathe_ring_angle_zero_at_top() {
        // VPinball places angle=0 at the TOP of the cylinder (maximum Z)
        // This is achieved by using sin for X and cos for Z:
        //   pm->x = r * sin(angle) + center.x
        //   pm->z = r * cos(angle) + center.z
        // When angle=0: sin(0)=0, cos(0)=1, so x=center.x, z=center.z+radius (TOP)

        let center_y = 100.0;
        let center_z = 50.0;
        let radius = 10.0;
        let center_x = 200.0;

        let ring = generate_lathe_ring(center_y, center_z, radius, center_x);

        // First vertex (angle=0) should be at the TOP (maximum Z)
        let (x0, y0, z0) = ring[0];

        // At angle=0: x should be at center_x (sin(0)=0), z should be at center_z + radius (cos(0)=1)
        assert!(
            (x0 - center_x).abs() < 0.001,
            "First vertex X {} should be at center_x {} (angle=0, sin(0)=0)",
            x0,
            center_x
        );
        assert!(
            (z0 - (center_z + radius)).abs() < 0.001,
            "First vertex Z {} should be at center_z + radius {} (angle=0, cos(0)=1, TOP)",
            z0,
            center_z + radius
        );
        assert!((y0 - center_y).abs() < 0.001, "Y should be center_y");

        // Verify first vertex has maximum Z (is at the top)
        let max_z = ring
            .iter()
            .map(|(_, _, z)| *z)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            (z0 - max_z).abs() < 0.001,
            "First vertex Z {} should be maximum Z {} (at the TOP)",
            z0,
            max_z
        );
    }

    #[test]
    fn test_texture_mapping_tu_starts_at_051() {
        // VPinball starts TU at 0.51 for the first vertex (angle=0, top of cylinder)
        // This maps the center of the texture to the top of the plunger

        let mut plunger = Plunger::default();
        plunger.center = Vertex2D {
            x: 500.0,
            y: 1900.0,
        };
        plunger.width = 25.0;
        plunger.height = 20.0;
        plunger.stroke = 80.0;
        plunger.plunger_type = PlungerType::Modern;
        plunger.is_visible = true;

        let meshes = build_plunger_meshes(&plunger, 0.0);

        // Check the rod mesh - find vertices at the top (maximum Z for each Y position)
        let (rod_vertices, _) = meshes.rod.as_ref().unwrap();

        // Group vertices by approximate Y position and find the one with max Z
        // The vertex with max Z at any Y slice should have TU close to 0.51
        let mut found_top_vertex = false;
        for vertex in rod_vertices {
            let z = vertex.vertex.z;
            let tu = vertex.vertex.tu;

            // Find vertices that are at the top (check if this is a "top" vertex by comparing Z)
            // For a cylinder, the top vertex at each ring should have the highest Z
            // We check if TU is close to 0.51 for high-Z vertices

            // Get approximate max Z for this mesh
            let max_z = rod_vertices
                .iter()
                .map(|v| v.vertex.z)
                .fold(f32::NEG_INFINITY, f32::max);

            if (z - max_z).abs() < 0.1 {
                // This is a top vertex, TU should be close to 0.51
                assert!(
                    (tu - 0.51).abs() < 0.01,
                    "Top vertex (max Z={}) should have TU close to 0.51, got TU={}",
                    z,
                    tu
                );
                found_top_vertex = true;
                break;
            }
        }

        assert!(
            found_top_vertex,
            "Should find at least one vertex at the top with TU=0.51"
        );
    }
}
