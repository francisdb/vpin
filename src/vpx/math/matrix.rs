//! Matrix math utilities ported from VPinball
//!
//! This module provides matrix operations for 3D transformations,
//! ported from VPinball's math/matrix.h
//!
//! VPinball uses row-major matrices with pre-multiplication convention.
//! When matrices are multiplied as A * B, the transformation A is applied first, then B.
//!
//! For a vertex v, the transformation (A * B) * v means:
//! 1. First apply A to v
//! 2. Then apply B to the result

use std::ops::Mul;

/// 3D vector for positions and directions
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Vertex3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vertex3D {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn normalize(&mut self) {
        let len = (self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        if len > 0.0 {
            let inv_len = 1.0 / len;
            self.x *= inv_len;
            self.y *= inv_len;
            self.z *= inv_len;
        }
    }

    pub fn normalized(mut self) -> Self {
        self.normalize();
        self
    }

    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
}

/// 4x4 matrix for representing affine transformations of 3D vectors
///
/// Uses row-major storage matching VPinball's Matrix3D.
///
/// Layout:
/// ```text
/// [ _11 _12 _13 _14 ]   [ m[0][0] m[0][1] m[0][2] m[0][3] ]
/// [ _21 _22 _23 _24 ] = [ m[1][0] m[1][1] m[1][2] m[1][3] ]
/// [ _31 _32 _33 _34 ]   [ m[2][0] m[2][1] m[2][2] m[2][3] ]
/// [ _41 _42 _43 _44 ]   [ m[3][0] m[3][1] m[3][2] m[3][3] ]
/// ```
///
/// Translation is stored in row 4 (_41, _42, _43).
///
/// Ported from: VPinball/src/math/matrix.h
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix3D {
    /// Row-major 4x4 matrix data
    pub m: [[f32; 4]; 4],
}

impl Default for Matrix3D {
    fn default() -> Self {
        Self::identity()
    }
}

impl Matrix3D {
    /// Create a new matrix with explicit values (row-major order)
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        m11: f32,
        m12: f32,
        m13: f32,
        m14: f32,
        m21: f32,
        m22: f32,
        m23: f32,
        m24: f32,
        m31: f32,
        m32: f32,
        m33: f32,
        m34: f32,
        m41: f32,
        m42: f32,
        m43: f32,
        m44: f32,
    ) -> Self {
        Self {
            m: [
                [m11, m12, m13, m14],
                [m21, m22, m23, m24],
                [m31, m32, m33, m34],
                [m41, m42, m43, m44],
            ],
        }
    }

    /// Create an identity matrix
    pub const fn identity() -> Self {
        Self::new(
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        )
    }

    /// Create a rotation matrix around the X axis
    ///
    /// From VPinball Matrix3D::SetRotateX
    pub fn rotate_x(ang_rad: f32) -> Self {
        let (sin, cos) = ang_rad.sin_cos();
        Self::new(
            1.0, 0.0, 0.0, 0.0, 0.0, cos, sin, 0.0, 0.0, -sin, cos, 0.0, 0.0, 0.0, 0.0, 1.0,
        )
    }

    /// Create a rotation matrix around the Y axis
    ///
    /// From VPinball Matrix3D::SetRotateY
    pub fn rotate_y(ang_rad: f32) -> Self {
        let (sin, cos) = ang_rad.sin_cos();
        Self::new(
            cos, 0.0, -sin, 0.0, 0.0, 1.0, 0.0, 0.0, sin, 0.0, cos, 0.0, 0.0, 0.0, 0.0, 1.0,
        )
    }

    /// Create a rotation matrix around the Z axis
    ///
    /// From VPinball Matrix3D::SetRotateZ
    pub fn rotate_z(ang_rad: f32) -> Self {
        let (sin, cos) = ang_rad.sin_cos();
        Self::new(
            cos, sin, 0.0, 0.0, -sin, cos, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        )
    }

    /// Create a uniform scale matrix
    pub const fn scale_uniform(scale: f32) -> Self {
        Self::scale(scale, scale, scale)
    }

    /// Create a non-uniform scale matrix
    ///
    /// From VPinball Matrix3D::MatrixScale
    pub const fn scale(sx: f32, sy: f32, sz: f32) -> Self {
        Self::new(
            sx, 0.0, 0.0, 0.0, 0.0, sy, 0.0, 0.0, 0.0, 0.0, sz, 0.0, 0.0, 0.0, 0.0, 1.0,
        )
    }

    /// Create a translation matrix
    ///
    /// From VPinball Matrix3D::MatrixTranslate
    pub const fn translate(x: f32, y: f32, z: f32) -> Self {
        Self::new(
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, x, y, z, 1.0,
        )
    }

    /// Transform a vertex by this matrix (with perspective divide)
    ///
    /// From VPinball Matrix3D::operator* (Vertex3Ds)
    pub fn transform_vertex(&self, v: Vertex3D) -> Vertex3D {
        let xp = self.m[0][0] * v.x + self.m[1][0] * v.y + self.m[2][0] * v.z + self.m[3][0];
        let yp = self.m[0][1] * v.x + self.m[1][1] * v.y + self.m[2][1] * v.z + self.m[3][1];
        let zp = self.m[0][2] * v.x + self.m[1][2] * v.y + self.m[2][2] * v.z + self.m[3][2];
        let wp = self.m[0][3] * v.x + self.m[1][3] * v.y + self.m[2][3] * v.z + self.m[3][3];

        let inv_wp = 1.0 / wp;
        Vertex3D::new(xp * inv_wp, yp * inv_wp, zp * inv_wp)
    }

    /// Transform a vector by this matrix (no translation, for normals/directions)
    ///
    /// From VPinball Matrix3D::MultiplyVectorNoTranslate
    pub fn transform_vector(&self, v: Vertex3D) -> Vertex3D {
        let xp = self.m[0][0] * v.x + self.m[1][0] * v.y + self.m[2][0] * v.z;
        let yp = self.m[0][1] * v.x + self.m[1][1] * v.y + self.m[2][1] * v.z;
        let zp = self.m[0][2] * v.x + self.m[1][2] * v.y + self.m[2][2] * v.z;
        Vertex3D::new(xp, yp, zp)
    }

    /// Transform a normal vector from a vertex structure
    ///
    /// From VPinball Matrix3D::MultiplyVectorNoTranslateNormal
    pub fn transform_normal(&self, nx: f32, ny: f32, nz: f32) -> Vertex3D {
        let xp = self.m[0][0] * nx + self.m[1][0] * ny + self.m[2][0] * nz;
        let yp = self.m[0][1] * nx + self.m[1][1] * ny + self.m[2][1] * nz;
        let zp = self.m[0][2] * nx + self.m[1][2] * ny + self.m[2][2] * nz;
        Vertex3D::new(xp, yp, zp)
    }
}

/// Matrix multiplication (A * B means B is applied first, then A)
///
/// From VPinball Matrix3D::operator*
impl Mul for Matrix3D {
    type Output = Matrix3D;

    fn mul(self, mult: Matrix3D) -> Matrix3D {
        let mut result = [[0.0f32; 4]; 4];
        for (i, row) in result.iter_mut().enumerate() {
            for (l, cell) in row.iter_mut().enumerate() {
                *cell = mult.m[0][l] * self.m[i][0]
                    + mult.m[1][l] * self.m[i][1]
                    + mult.m[2][l] * self.m[i][2]
                    + mult.m[3][l] * self.m[i][3];
            }
        }
        Matrix3D { m: result }
    }
}

impl Mul for &Matrix3D {
    type Output = Matrix3D;

    fn mul(self, mult: &Matrix3D) -> Matrix3D {
        let mut result = [[0.0f32; 4]; 4];
        for (i, row) in result.iter_mut().enumerate() {
            for (l, cell) in row.iter_mut().enumerate() {
                *cell = mult.m[0][l] * self.m[i][0]
                    + mult.m[1][l] * self.m[i][1]
                    + mult.m[2][l] * self.m[i][2]
                    + mult.m[3][l] * self.m[i][3];
            }
        }
        Matrix3D { m: result }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-5;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn test_identity() {
        let m = Matrix3D::identity();
        let v = Vertex3D::new(1.0, 2.0, 3.0);
        let result = m.transform_vertex(v);
        assert!(approx_eq(result.x, 1.0));
        assert!(approx_eq(result.y, 2.0));
        assert!(approx_eq(result.z, 3.0));
    }

    #[test]
    fn test_translation() {
        let m = Matrix3D::translate(10.0, 20.0, 30.0);
        let v = Vertex3D::new(1.0, 2.0, 3.0);
        let result = m.transform_vertex(v);
        assert!(approx_eq(result.x, 11.0));
        assert!(approx_eq(result.y, 22.0));
        assert!(approx_eq(result.z, 33.0));
    }

    #[test]
    fn test_scale() {
        let m = Matrix3D::scale(2.0, 3.0, 4.0);
        let v = Vertex3D::new(1.0, 2.0, 3.0);
        let result = m.transform_vertex(v);
        assert!(approx_eq(result.x, 2.0));
        assert!(approx_eq(result.y, 6.0));
        assert!(approx_eq(result.z, 12.0));
    }

    #[test]
    fn test_rotate_z_90() {
        let m = Matrix3D::rotate_z(std::f32::consts::FRAC_PI_2);
        let v = Vertex3D::new(1.0, 0.0, 0.0);
        let result = m.transform_vertex(v);
        // Rotating (1,0,0) by 90 degrees around Z should give (0,1,0)
        assert!(approx_eq(result.x, 0.0), "x: expected 0, got {}", result.x);
        assert!(approx_eq(result.y, 1.0), "y: expected 1, got {}", result.y);
        assert!(approx_eq(result.z, 0.0), "z: expected 0, got {}", result.z);
    }

    #[test]
    fn test_rotate_x_90() {
        let m = Matrix3D::rotate_x(std::f32::consts::FRAC_PI_2);
        let v = Vertex3D::new(0.0, 1.0, 0.0);
        let result = m.transform_vertex(v);
        // Rotating (0,1,0) by 90 degrees around X should give (0,0,1)
        assert!(approx_eq(result.x, 0.0), "x: expected 0, got {}", result.x);
        assert!(approx_eq(result.y, 0.0), "y: expected 0, got {}", result.y);
        assert!(approx_eq(result.z, 1.0), "z: expected 1, got {}", result.z);
    }

    #[test]
    fn test_rotate_y_90() {
        let m = Matrix3D::rotate_y(std::f32::consts::FRAC_PI_2);
        let v = Vertex3D::new(1.0, 0.0, 0.0);
        let result = m.transform_vertex(v);
        // Rotating (1,0,0) by 90 degrees around Y should give (0,0,-1)
        assert!(approx_eq(result.x, 0.0), "x: expected 0, got {}", result.x);
        assert!(approx_eq(result.y, 0.0), "y: expected 0, got {}", result.y);
        assert!(
            approx_eq(result.z, -1.0),
            "z: expected -1, got {}",
            result.z
        );
    }

    #[test]
    fn test_matrix_multiplication_order() {
        // Scale then translate: vertex at (1,0,0) scaled by 2 = (2,0,0), then translated by (10,0,0) = (12,0,0)
        let scale = Matrix3D::scale(2.0, 1.0, 1.0);
        let translate = Matrix3D::translate(10.0, 0.0, 0.0);

        // In VPinball convention, A * B means A is applied first, then B
        // So scale * translate means: first scale, then translate
        let combined = scale * translate;

        let v = Vertex3D::new(1.0, 0.0, 0.0);
        let result = combined.transform_vertex(v);

        assert!(approx_eq(result.x, 12.0), "Expected 12.0, got {}", result.x);
    }

    #[test]
    fn test_transform_vector_no_translation() {
        let m = Matrix3D::translate(10.0, 20.0, 30.0);
        let v = Vertex3D::new(1.0, 0.0, 0.0);
        let result = m.transform_vector(v);
        // Translation should not affect vectors (normals/directions)
        assert!(approx_eq(result.x, 1.0));
        assert!(approx_eq(result.y, 0.0));
        assert!(approx_eq(result.z, 0.0));
    }

    #[test]
    fn test_combined_rotation() {
        // Test combining multiple rotations
        let rot_x = Matrix3D::rotate_x(90.0_f32.to_radians());
        let rot_z = Matrix3D::rotate_z(90.0_f32.to_radians());

        // Apply RotX first, then RotZ: rot_x * rot_z
        let combined = rot_x * rot_z;

        let v = Vertex3D::new(0.0, 1.0, 0.0);
        let result = combined.transform_vertex(v);

        // (0,1,0) rotated 90° around X gives (0,0,1)
        // (0,0,1) rotated 90° around Z gives (0,0,1) (Z rotation doesn't affect Z-aligned vector)
        assert!(approx_eq(result.x, 0.0), "x: expected 0, got {}", result.x);
        assert!(approx_eq(result.y, 0.0), "y: expected 0, got {}", result.y);
        assert!(approx_eq(result.z, 1.0), "z: expected 1, got {}", result.z);
    }
}
