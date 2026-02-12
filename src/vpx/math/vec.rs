//! Simple vector types for geometry calculations
//!
//! These are lightweight vector types used for mesh generation and other
//! geometry operations. For full 3D transformations with 4x4 matrices,
//! use `Vertex3D` and `Matrix3D` from the matrix module.

use std::f32::consts::PI;

/// A 2D vector helper used for geometry calculations
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl std::ops::Add for Vec2 {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Vec2 {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl std::ops::Sub for Vec2 {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Vec2 {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
}

impl std::ops::Mul<f32> for Vec2 {
    type Output = Self;
    fn mul(self, s: f32) -> Self {
        Vec2 {
            x: self.x * s,
            y: self.y * s,
        }
    }
}

impl Vec2 {
    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn normalize(self) -> Self {
        let len = self.length();
        if len == 0.0 {
            Self { x: 0.0, y: 0.0 }
        } else {
            Self {
                x: self.x / len,
                y: self.y / len,
            }
        }
    }
}

/// A 3D vector helper used for geometry calculations
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl std::ops::Add for Vec3 {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Vec3 {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Vec3 {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }
}

impl std::ops::Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, s: f32) -> Self {
        Vec3 {
            x: self.x * s,
            y: self.y * s,
            z: self.z * s,
        }
    }
}

impl Vec3 {
    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn normalize(self) -> Self {
        let len = self.length();
        if len == 0.0 {
            Self {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            }
        } else {
            Self {
                x: self.x / len,
                y: self.y / len,
                z: self.z / len,
            }
        }
    }

    pub fn cross(a: &Vec3, b: &Vec3) -> Vec3 {
        Vec3 {
            x: a.y * b.z - a.z * b.y,
            y: a.z * b.x - a.x * b.z,
            z: a.x * b.y - a.y * b.x,
        }
    }
}

/// A simple 3x3 matrix for rotations
/// Used for flipper and other mesh transformations
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat3 {
    pub m: [[f32; 3]; 3],
}

impl Mat3 {
    /// Create a rotation matrix around the Z axis
    pub fn rotate_z(angle: f32) -> Self {
        let (sin, cos) = angle.sin_cos();
        Mat3 {
            m: [[cos, -sin, 0.0], [sin, cos, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    /// Multiply a vector by this matrix (applies rotation)
    pub fn multiply_vector(&self, v: Vec3) -> Vec3 {
        Vec3 {
            x: self.m[0][0] * v.x + self.m[0][1] * v.y + self.m[0][2] * v.z,
            y: self.m[1][0] * v.x + self.m[1][1] * v.y + self.m[1][2] * v.z,
            z: self.m[2][0] * v.x + self.m[2][1] * v.y + self.m[2][2] * v.z,
        }
    }

    /// Multiply a vector by this matrix without translation (for normals)
    /// For a 3x3 rotation matrix, this is the same as multiply_vector
    pub fn multiply_vector_no_translate(&self, v: Vec3) -> Vec3 {
        self.multiply_vector(v)
    }
}

/// Rotate a vector around an axis using Rodrigues' rotation formula
pub fn get_rotated_axis(angle_degrees: f32, axis: &Vec3, temp: &Vec3) -> Vec3 {
    let u = axis.normalize();
    let angle_rad = angle_degrees * PI / 180.0;
    let sin_angle = angle_rad.sin();
    let cos_angle = angle_rad.cos();
    let one_minus_cos = 1.0 - cos_angle;

    let rot_row0 = Vec3 {
        x: u.x * u.x + cos_angle * (1.0 - u.x * u.x),
        y: u.x * u.y * one_minus_cos - sin_angle * u.z,
        z: u.x * u.z * one_minus_cos + sin_angle * u.y,
    };
    let rot_row1 = Vec3 {
        x: u.x * u.y * one_minus_cos + sin_angle * u.z,
        y: u.y * u.y + cos_angle * (1.0 - u.y * u.y),
        z: u.y * u.z * one_minus_cos - sin_angle * u.x,
    };
    let rot_row2 = Vec3 {
        x: u.x * u.z * one_minus_cos - sin_angle * u.y,
        y: u.y * u.z * one_minus_cos + sin_angle * u.x,
        z: u.z * u.z + cos_angle * (1.0 - u.z * u.z),
    };

    Vec3 {
        x: temp.x * rot_row0.x + temp.y * rot_row0.y + temp.z * rot_row0.z,
        y: temp.x * rot_row1.x + temp.y * rot_row1.y + temp.z * rot_row1.z,
        z: temp.x * rot_row2.x + temp.y * rot_row2.y + temp.z * rot_row2.z,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec2_operations() {
        let v1 = Vec2 { x: 1.0, y: 2.0 };
        let v2 = Vec2 { x: 3.0, y: 4.0 };
        assert_eq!(v1 + v2, Vec2 { x: 4.0, y: 6.0 });
        assert_eq!(v1 - v2, Vec2 { x: -2.0, y: -2.0 });
        assert_eq!(v1 * 2.0, Vec2 { x: 2.0, y: 4.0 });
        assert_eq!(v1.length(), (5.0f32).sqrt());
        assert_eq!(
            v1.normalize(),
            Vec2 {
                x: 1.0 / (5.0f32).sqrt(),
                y: 2.0 / (5.0f32).sqrt()
            }
        );
    }

    #[test]
    fn test_vec3_operations() {
        let v1 = Vec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        let v2 = Vec3 {
            x: 4.0,
            y: 5.0,
            z: 6.0,
        };
        assert_eq!(
            v1 + v2,
            Vec3 {
                x: 5.0,
                y: 7.0,
                z: 9.0
            }
        );
        assert_eq!(
            v1 - v2,
            Vec3 {
                x: -3.0,
                y: -3.0,
                z: -3.0
            }
        );
        assert_eq!(
            v1 * 2.0,
            Vec3 {
                x: 2.0,
                y: 4.0,
                z: 6.0
            }
        );
        assert_eq!(v1.length(), (14.0f32).sqrt());
        assert_eq!(
            v1.normalize(),
            Vec3 {
                x: 1.0 / (14.0f32).sqrt(),
                y: 2.0 / (14.0f32).sqrt(),
                z: 3.0 / (14.0f32).sqrt()
            }
        );
        assert_eq!(
            Vec3::cross(&v1, &v2),
            Vec3 {
                x: -3.0,
                y: 6.0,
                z: -3.0
            }
        );
    }

    #[test]
    fn test_mat3_rotation() {
        let angle_rad = 90.0f32.to_radians();
        let rot = Mat3::rotate_z(angle_rad);
        let v = Vec3 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        };
        let rotated_v = rot.multiply_vector(v);
        assert!((rotated_v.x - 0.0).abs() < 1e-6);
        assert!((rotated_v.y - 1.0).abs() < 1e-6);
        assert!((rotated_v.z - 0.0).abs() < 1e-6);
    }
}
