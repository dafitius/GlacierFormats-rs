use std::iter::Sum;
use std::ops;
use bincode::Encode;
use binrw::{BinRead, BinWrite};

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

#[derive(BinRead, BinWrite, Debug, Clone, Copy, Default, PartialEq, Encode)]
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

#[derive(Debug, PartialEq)]
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