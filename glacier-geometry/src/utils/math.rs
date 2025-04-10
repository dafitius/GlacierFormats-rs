use std::iter::Sum;
use std::ops;
use std::ops::{Add, Mul, Sub};
use bincode::Encode;
use binrw::{BinRead, BinWrite};
use itertools::Position;
use nalgebra::{Matrix4, UnitQuaternion};
use crate::utils::buffer::VertexWeights;

#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Encode)]
pub struct Quaternion {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Default for Quaternion {
    fn default() -> Self {
        Self {x: 0.0, y: 0.0, z: 0.0, w: 1.0}
    }
}

impl From<nalgebra::UnitQuaternion<f32>> for Quaternion {
    fn from(value: nalgebra::UnitQuaternion<f32>) -> Self {
        Self{
            x: value.as_ref().coords.x,
            y: value.as_ref().coords.y,
            z: value.as_ref().coords.z,
            w: value.as_ref().coords.w,
        }
    }
}

impl From<Quaternion> for nalgebra::UnitQuaternion<f32> {
    fn from(value: Quaternion) -> Self {
        Self::from_quaternion(nalgebra::Quaternion::new(value.w, value.x, value.y, value.z))
    }
}

impl Quaternion {
    fn add(&self, other: &Self) -> Self {
        let self_quat: nalgebra::UnitQuaternion<f32> = (*self).into();
        let other_quat: nalgebra::UnitQuaternion<f32> = (*other).into();
        nalgebra::UnitQuaternion::<f32>::from_quaternion(self_quat.add(other_quat.as_ref())).into()
    }

    fn multiply(&self, other: &Self) -> Self {
        let self_quat: nalgebra::UnitQuaternion<f32> = (*self).into();
        let other_quat: nalgebra::UnitQuaternion<f32> = (*other).into();
        let rotation_matrix = self_quat.to_rotation_matrix() * other_quat.to_rotation_matrix();
        nalgebra::UnitQuaternion::<f32>::from_matrix(rotation_matrix.matrix()).into()
    }

    pub fn normalize(&self) -> Self {
        let self_quat: nalgebra::UnitQuaternion<f32> = (*self).into();
        let quat : nalgebra::UnitQuaternion<f32> = UnitQuaternion::from_quaternion(self_quat.normalize());
        quat.into()
    }

    pub fn euler_angles_rad(&self) -> (f32, f32, f32) {
        let self_quat: nalgebra::UnitQuaternion<f32> = (*self).into();
        self_quat.euler_angles()
    }

    pub fn euler_angles_deg(&self) -> (f32, f32, f32) {
        let degs = self.euler_angles_rad();
        (degs.0.to_degrees(), degs.1.to_degrees(), degs.2.to_degrees())
    }

    pub fn rotate(&mut self, x: f32, y: f32, z: f32) {
        let mut self_quat: nalgebra::UnitQuaternion<f32> = (*self).into();
        let rotation_euler = UnitQuaternion::from_euler_angles(x, y, z);
        let global_matrix = self_quat * rotation_euler;
        *self = global_matrix.into();
    }
}


#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Default, Encode)]
pub struct Transform { //in glacier this is known as SQV
    pub rotation: Quaternion,
    pub position: Vector4,
}

impl Transform {
    pub fn mul(&self, other: &Transform) -> Self {
        let mut mat = Matrix43::from(*self).mul(&Matrix43::from(*other));
        mat.into()
    }
}

#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Encode)]
pub struct Matrix43 {
    x_axis: Vector3,
    y_axis: Vector3,
    z_axis: Vector3,
    trans: Vector3,
}

impl From<Matrix43> for nalgebra::Matrix4x3<f32> {
    fn from(value: Matrix43) -> Self {
        // Matrix4x3::new(
        //     value.x_axis.x, value.x_axis.y, value.x_axis.z,
        //     value.y_axis.x, value.y_axis.y, value.y_axis.z,
        //     value.z_axis.x, value.z_axis.y, value.z_axis.z,
        //     value.trans.x,  value.trans.y,  value.trans.z,
        // )
        nalgebra::Matrix4x3::new(
            value.x_axis.x, value.y_axis.x, value.z_axis.x, value.trans.x,
            value.x_axis.y, value.y_axis.y, value.z_axis.y, value.trans.y,
            value.x_axis.z, value.y_axis.z, value.z_axis.z, value.trans.z,
        )
    }
}

impl From<Matrix43> for nalgebra::Matrix4<f32> {
    fn from(value: Matrix43) -> Self {
        // nalgebra::Matrix4::new(
        //     value.x_axis.x, value.x_axis.y, value.x_axis.z,0.0f32,
        //     value.y_axis.x, value.y_axis.y, value.y_axis.z, 0.0f32,
        //     value.z_axis.x, value.z_axis.y, value.z_axis.z, 0.0f32,
        //     value.trans.x,  value.trans.y,  value.trans.z, 1.0f32,
        // )
        nalgebra::Matrix4::new(
            value.x_axis.x, value.y_axis.x, value.z_axis.x, value.trans.x,
            value.x_axis.y, value.y_axis.y, value.z_axis.y, value.trans.y,
            value.x_axis.z, value.y_axis.z, value.z_axis.z, value.trans.z,
            0.0, 0.0, 0.0, 1.0,
        )
    }
}

impl From<nalgebra::Matrix4<f32>> for Matrix43 {
    fn from(value: nalgebra::Matrix4<f32>) -> Self {
        Self{
            x_axis: Vector3{
                x: value.m11,
                y: value.m21,
                z: value.m31,
            },
            y_axis: Vector3{
                x: value.m12,
                y: value.m22,
                z: value.m32,
            },
            z_axis: Vector3{
                x: value.m13,
                y: value.m23,
                z: value.m33,
            },
            trans: Vector3{
                x: value.m14,
                y: value.m24,
                z: value.m34,
            },
        }
    }
}


impl From<Transform> for Matrix43 {
    fn from(transform: Transform) -> Self {
            let translation = nalgebra::Translation3::new(transform.position.x, transform.position.y, transform.position.z);
            let quat: nalgebra::UnitQuaternion<f32> = transform.rotation.into();
            let rotation = quat.to_rotation_matrix();

            Self{
                x_axis: Vector3{x: rotation.matrix().m11, y: rotation.matrix().m21, z: rotation.matrix().m31},
                y_axis: Vector3{x: rotation.matrix().m12, y: rotation.matrix().m22, z: rotation.matrix().m32},
                z_axis: Vector3{x: rotation.matrix().m13, y: rotation.matrix().m23, z: rotation.matrix().m33},
                trans:  Vector3{x: translation.x, y: translation.y, z: translation.z},
            }
    }
}

impl From<Matrix43> for Transform {
    fn from(value: Matrix43) -> Self {

        let quaternion =
            nalgebra::UnitQuaternion::from_matrix(&nalgebra::Matrix3::new(
                value.x_axis.x, value.y_axis.x, value.z_axis.x,
                value.x_axis.y, value.y_axis.y, value.z_axis.y,
                value.x_axis.z, value.y_axis.z, value.z_axis.z));

        Self{
            rotation: quaternion.into(),
            position: Vector4::from_vector3(value.trans, 1.0),
        }
    }
}

impl Default for Matrix43 {
    fn default() -> Self {
        Matrix43{
            x_axis: Vector3{x: 1.0, y: 0.0, z: 0.0},
            y_axis: Vector3{x: 0.0, y: 1.0, z: 0.0},
            z_axis: Vector3{x: 0.0, y: 0.0, z: 1.0},
            trans:  Vector3{x: 0.0, y: 0.0, z: 0.0},
        }
    }
}

impl Matrix43 {

    pub fn identity() -> Matrix43 {
        Matrix43::default()
    }

    pub fn new(x_axis: Vector3, y_axis: Vector3, z_axis: Vector3, trans: Vector3) -> Self {
        Matrix43 {
            x_axis,
            y_axis,
            z_axis,
            trans,
        }
    }

    pub fn x_axis(&self) -> &Vector3 {
        &self.x_axis
    }

    fn x_axis_mut(&mut self) -> &mut Vector3 {
        &mut self.x_axis
    }

    pub fn y_axis(&self) -> &Vector3 {
        &self.y_axis
    }

    fn y_axis_mut(&mut self) -> &mut Vector3 {
        &mut self.y_axis
    }

    pub fn z_axis(&self) -> &Vector3 {
        &self.z_axis
    }

    fn z_axis_mut(&mut self) -> &mut Vector3 {
        &mut self.z_axis
    }

    pub fn trans(&self) -> &Vector3 {
        &self.trans
    }

    fn trans_mut(&mut self) -> &mut Vector3 {
        &mut self.trans
    }

    pub fn m11(&self) -> f32 { self.x_axis.x }
    pub fn m12(&self) -> f32 { self.x_axis.y }
    pub fn m13(&self) -> f32 { self.x_axis.z }
    pub fn m21(&self) -> f32 { self.y_axis.x }
    pub fn m22(&self) -> f32 { self.y_axis.y }
    pub fn m23(&self) -> f32 { self.y_axis.z }
    pub fn m31(&self) -> f32 { self.z_axis.x }
    pub fn m32(&self) -> f32 { self.z_axis.y }
    pub fn m33(&self) -> f32 { self.z_axis.z }
    pub fn m41(&self) -> f32 { self.trans.x }
    pub fn m42(&self) -> f32 { self.trans.y }
    pub fn m43(&self) -> f32 { self.trans.z }

    pub fn members(&self) -> [f32; 12] {
        [
            self.m11(),self.m12(),self.m13(),
            self.m21(),self.m22(),self.m23(),
            self.m31(),self.m32(),self.m33(),
            self.m41(),self.m42(),self.m43()
        ]
    }

    pub fn inverse(&self) -> Option<Self> {
        let mat: nalgebra::Matrix4<f32> = (*self).into();
        let inv = mat.try_inverse()?;
        Some(inv.into())
    }

    pub fn mul(&self, other: &Self) -> Self {
        let cur: nalgebra::Matrix4<f32> = (*self).into();
        let fact: nalgebra::Matrix4<f32> = (*other).into();
        // println!("adding {:?} to {:?} resulting in {:?}", cur, fact, cur * fact);
        (cur * fact).into()
    }

    pub fn round(&mut self, epsilon: f32) {
        if self.x_axis.x.abs() < epsilon { self.x_axis.x = 0.0; }
        if self.x_axis.y.abs() < epsilon { self.x_axis.y = 0.0; }
        if self.x_axis.z.abs() < epsilon { self.x_axis.z = 0.0; }
        if self.y_axis.x.abs() < epsilon { self.y_axis.x = 0.0; }
        if self.y_axis.y.abs() < epsilon { self.y_axis.y = 0.0; }
        if self.y_axis.z.abs() < epsilon { self.y_axis.z = 0.0; }
        if self.z_axis.x.abs() < epsilon { self.z_axis.x = 0.0; }
        if self.z_axis.y.abs() < epsilon { self.z_axis.y = 0.0; }
        if self.z_axis.z.abs() < epsilon { self.z_axis.z = 0.0; }
        if self.trans.x.abs() < epsilon { self.trans.x = 0.0; }
        if self.trans.y.abs() < epsilon { self.trans.y = 0.0; }
        if self.trans.z.abs() < epsilon { self.trans.z = 0.0; }

        if 1.0 - self.x_axis.x.abs() < epsilon { self.x_axis.x = 1.0; }
        if 1.0 - self.x_axis.y.abs() < epsilon { self.x_axis.y = 1.0; }
        if 1.0 - self.x_axis.z.abs() < epsilon { self.x_axis.z = 1.0; }
        if 1.0 - self.y_axis.x.abs() < epsilon { self.y_axis.x = 1.0; }
        if 1.0 - self.y_axis.y.abs() < epsilon { self.y_axis.y = 1.0; }
        if 1.0 - self.y_axis.z.abs() < epsilon { self.y_axis.z = 1.0; }
        if 1.0 - self.z_axis.x.abs() < epsilon { self.z_axis.x = 1.0; }
        if 1.0 - self.z_axis.y.abs() < epsilon { self.z_axis.y = 1.0; }
        if 1.0 - self.z_axis.z.abs() < epsilon { self.z_axis.z = 1.0; }
        if 1.0 - self.trans.x.abs() < epsilon { self.trans.x = 1.0; }
        if 1.0 - self.trans.y.abs() < epsilon { self.trans.y = 1.0; }
        if 1.0 - self.trans.z.abs() < epsilon { self.trans.z = 1.0; }
    }

    pub fn flipped_z(&self) -> Self{
        let new_mat: nalgebra::Matrix4<f32> = (*self).into();
        let flip_z = nalgebra::Matrix4::new_nonuniform_scaling(&nalgebra::Vector3::new(1.0, 1.0, -1.0));
        (flip_z * new_mat).into()
    }
}

pub trait Vector: Copy {
    fn add(&self, other: &Self) -> Self;
    fn sub(&self, other: &Self) -> Self;
    fn scale(&self, factor: f32) -> Self;
    fn min(&self, other: &Self) -> Self;
    fn max(&self, other: &Self) -> Self;
}

#[derive(BinRead, BinWrite, Debug, Clone, Copy, Default, PartialEq, Encode)]
pub struct Vector4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vector4 {
    pub fn from_float(val: f32) -> Self {
        Self {
            x: val,
            y: val,
            z: val,
            w: val,
        }
    }

    pub fn from_vector3(vec3: Vector3, w: f32) -> Self {
        Self {
            x: vec3.x,
            y: vec3.y,
            z: vec3.z,
            w,
        }
    }

    pub fn as_slice(&self) -> [f32; 4] {
        [self.x, self.y, self.z, self.w]
    }
}

impl Vector for Vector4 {
    fn add(&self, other: &Self) -> Self {
        Vector4 {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
            w: self.w + other.w,
        }
    }

    fn sub(&self, other: &Self) -> Self {
        Vector4 {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
            w: self.w - other.w,
        }
    }

    fn scale(&self, factor: f32) -> Self {
        Vector4 {
            x: self.x * factor,
            y: self.y * factor,
            z: self.z * factor,
            w: self.w * factor,
        }
    }

    fn min(&self, other: &Self) -> Self {
        Vector4 {
            x: self.x.min(other.x),
            y: self.y.min(other.y),
            z: self.z.min(other.z),
            w: self.w.min(other.w),
        }
    }

    fn max(&self, other: &Self) -> Self {
        Vector4 {
            x: self.x.max(other.x),
            y: self.y.max(other.y),
            z: self.z.max(other.z),
            w: self.w.max(other.w),
        }
    }
}

#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Default, Encode)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector for Vector3 {
    fn add(&self, other: &Self) -> Self {
        Vector3 {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }

    fn sub(&self, other: &Self) -> Self {
        Vector3 {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }

    fn scale(&self, factor: f32) -> Self {
        Vector3 {
            x: self.x * factor,
            y: self.y * factor,
            z: self.z * factor,
        }
    }

    fn min(&self, other: &Self) -> Self {
        Vector3 {
            x: self.x.min(other.x),
            y: self.y.min(other.y),
            z: self.z.min(other.z),
        }
    }

    fn max(&self, other: &Self) -> Self {
        Vector3 {
            x: self.x.max(other.x),
            y: self.y.max(other.y),
            z: self.z.max(other.z),
        }
    }
}

#[derive(BinRead, BinWrite, Debug, Clone, Copy, Default, PartialEq, Encode)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
}

impl Vector for Vector2 {
    fn add(&self, other: &Self) -> Self {
        Vector2 {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }

    fn sub(&self, other: &Self) -> Self {
        Vector2 {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }

    fn scale(&self, factor: f32) -> Self {
        Vector2 {
            x: self.x * factor,
            y: self.y * factor,
        }
    }

    fn min(&self, other: &Self) -> Self {
        Vector2 {
            x: self.x.min(other.x),
            y: self.y.min(other.y),
        }
    }

    fn max(&self, other: &Self) -> Self {
        Vector2 {
            x: self.x.max(other.x),
            y: self.y.max(other.y),
        }
    }
}

#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Encode)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Default for Color {
    fn default() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 1,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct BoundingBox<V> {
    pub min: V,
    pub max: V,
}

impl<V> BoundingBox<V>
    where
        V: Vector,
{
    pub fn center(&self) -> V {
        self.min.add(&self.max).scale(0.5)
    }

    pub fn dimensions(&self) -> V {
        self.max.sub(&self.min)
    }
}

impl<V> Sum for BoundingBox<V> where V: Vector + Default{
    fn sum<I: Iterator<Item=Self>>(iter: I) -> Self {
        let mut iter = iter.into_iter();
        if let Some(first) = iter.next() {
            iter.fold(first, |acc, x| acc + x)
        } else {
            // Return a default BoundingBox if the iterator is empty
            BoundingBox {
                min: V::default(),
                max: V::default(),
            }
        }
    }
}

impl<V> ops::Add<BoundingBox<V>> for BoundingBox<V>
    where V: Vector,{
    type Output = BoundingBox<V>;

    fn add(self, rhs: BoundingBox<V>) -> Self::Output {
        BoundingBox {
            min: self.min.min(&rhs.min),
            max: self.max.max(&rhs.max),
        }
    }
}