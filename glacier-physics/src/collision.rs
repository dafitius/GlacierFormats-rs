use binrw::{BinRead, BinResult, BinWrite, NullString};
use bitfield_struct::bitfield;
use physx34_sys::geometry::PrimitiveGeometry;
use physx34_sys::physX::PhysxFoundation;
use physx34_sys::physX::PxPhysics;
use std::io::{Error, ErrorKind};
use std::mem::ManuallyDrop;
use physx34_sys::math::Transform;

#[repr(u32)]
#[derive(BinRead, BinWrite, Debug, PartialEq, Copy, Clone)]
#[brw(repr = u32)]
pub enum CollidableLayer {
    CollideWithAll = 0,
    StaticCollidablesOnly = 1,
    DynamicCollidablesOnly = 2,
    Stairs = 3,
    ShotOnlyCollision = 4,
    DynamicClothOnly = 5,
    DynamicTrashCollidables = 6,
    KinematicCollidablesOnly = 7,
    CharacterClothingOnly = 8,
    StaticCollidablesOnlyTransparent = 9,
    DynamicCollidablesOnlyTransparent = 10,
    KinematicCollidablesOnlyTransparent = 11,
    StairsSteps = 12,
    StairsSlope = 13,
    Auxiliary = 14,
    HeroProxy = 15,
    ActorProxy = 16,
    Clip = 17,
    ActorRagdoll = 18,
    CrowdRagdoll = 19,
    LedgeAnchor = 20,
    ActorDynBody = 21,
    HeroDynBody = 22,
    Items = 23,
    Weapons = 24,
    CollisionVolumeHitmanOn = 25,
    CollisionVolumeHitmanOff = 26,
    DynamicCollidablesOnlyNoCharacter = 27,
    DynamicCollidablesOnlyNoCharacterTransparent = 28,
    CollideWithStaticOnly = 29,
    AiVisionBlocker = 30,
    AiVisionBlockerAmbientOnly = 31,
    UnusedLast = 32,
}

#[derive(Debug)]
pub struct ShapeDescription {
    pub geometry: physx34_sys::geometry::GeometryHolder,
    pub transform: Transform,
    pub collision_layer: CollidableLayer,
    pub is_opaque: bool,
}

#[bitfield(u32)]
#[derive(Eq, Hash, PartialEq, BinRead, BinWrite)]
//Found in alpha runtime.physics.physx as ECollisionPack
pub struct CollisionPack {
    convex_mesh: bool,   //1
    triangle_mesh: bool, //2
    primitive: bool,     //4
    __: bool,
    bone_primitive: bool, //16
    __: bool,
    kinematic_bone_primitive: bool, //64
    unknown: bool,
    #[bits(24)]
    __: usize,
}

impl CollisionPack {
    pub fn num_shapes(&self) -> usize {
        self.convex_mesh() as usize + self.triangle_mesh() as usize + self.primitive() as usize
    }
}

#[derive(Debug, PartialEq, BinRead)]
#[br(repr = u32)]
pub enum ObjectEntity {
    //Found in alpha runtime.physics.physx as EObjectEntity
    None = 0,
    Static = 0x1,
    RigidBody = 0x2,
    PressureSoftBody = 0x8,
    Shatter = 0x10,
    KinematicLinked = 0x20,
    BackwardCompatible = 2147483647,
}

#[derive(Debug, BinRead)]
#[br(assert(id == "ID", magic == "PhysX"))]
pub struct PhysXMagic {
    #[br(count = 2, try_map = |s: Vec<u8>| String::from_utf8(s))]
    id: String,
    version: u32,
    #[br(count = 5, try_map = |s: Vec<u8>| String::from_utf8(s))]
    magic: String,
}

impl Default for PhysXMagic {
    fn default() -> Self {
        Self{
            id: "ID".to_string(),
            version: 5,
            magic: "PhysX".to_string(),
        }
    }
}

#[derive(Debug, BinRead)]
#[br(assert(tag == "CVX"))]
#[br(import(physics: &PxPhysics))]
pub struct ConvexMeshDescription {
    #[br(count = 3, try_map = |s: Vec<u8>| String::from_utf8(s))]
    #[br(pad_after(1))]
    tag: String,
    num_subshapes: u32,
    #[br(
    parse_with = parse_convex_subshapes,
    args(physics, num_subshapes)
    )]
    pub subshapes: Vec<ShapeDescription>,
}

impl PhysicsShapeDescription for ConvexMeshDescription {
    fn subshapes(&self) -> &Vec<ShapeDescription> {
        self.subshapes.as_ref()
    }
}

#[derive(Debug, BinRead)]
#[br(assert(tag == "TRI"))]
#[br(import(physics: &PxPhysics))]
pub struct TriangleMeshDescription {
    #[br(count = 3, try_map = |s: Vec<u8>| String::from_utf8(s))]
    #[br(pad_after(1))]
    tag: String,
    num_subshapes: u32,
    #[br(
    parse_with = parse_triangle_subshapes,
    args(physics, num_subshapes)
    )]
    pub subshapes: Vec<ShapeDescription>,
}

impl PhysicsShapeDescription for TriangleMeshDescription {
    fn subshapes(&self) -> &Vec<ShapeDescription> {
        self.subshapes.as_ref()
    }
}

#[derive(Debug, BinRead)]
#[br(assert(tag == "ICP"))]
#[br(import(physics: &PxPhysics))]
pub struct PrimitiveMeshDescription {
    #[br(count = 3, try_map = |s: Vec<u8>| String::from_utf8(s))]
    #[br(pad_after(1))]
    tag: String,
    num_subshapes: u32,
    #[br(
    parse_with = parse_primitive_subshapes,
    args(physics, num_subshapes)
    )]
    pub subshapes: Vec<ShapeDescription>,
}

impl Default for PrimitiveMeshDescription {
    fn default() -> Self {
        Self{
            tag: "ICP".to_string(),
            num_subshapes: 0,
            subshapes: vec![],
        }
    }
}

impl PhysicsShapeDescription for PrimitiveMeshDescription {
    fn subshapes(&self) -> &Vec<ShapeDescription> {
        self.subshapes.as_ref()
    }
}

pub trait PhysicsShapeDescription {
    fn subshapes(&self) -> &Vec<ShapeDescription>;
}

#[binrw::parser(reader)]
fn parse_triangle_subshapes(
    physics: &PxPhysics,
    num_subshapes: u32,
) -> BinResult<Vec<ShapeDescription>> {
    Ok((0..num_subshapes)
        .flat_map(|_| -> BinResult<ShapeDescription> {
            let collision_layer = CollidableLayer::read_le(reader)?;
            let is_opaque = u32::read_le(reader)? > 0;
            Ok(ShapeDescription {
                geometry: physics.read_triangle_mesh(reader).unwrap(),
                transform: Transform::default(),
                collision_layer,
                is_opaque,
            })
        })
        .collect())
}

#[binrw::parser(reader)]
fn parse_convex_subshapes(
    physics: &PxPhysics,
    num_subshapes: u32,
) -> BinResult<Vec<ShapeDescription>> {
    Ok((0..num_subshapes)
        .flat_map(|_| -> BinResult<ShapeDescription> {
            let collision_layer = CollidableLayer::read_le(reader)?;
            let is_opaque = u32::read_le(reader)? > 0;
            let transform = Transform::read(reader)?;
            Ok(ShapeDescription {
                geometry: physics.read_convex_mesh(reader).unwrap(),
                transform,
                collision_layer,
                is_opaque,
            })
        })
        .collect())
}

#[binrw::parser(reader)]
fn parse_primitive_subshapes(
    physics: &PxPhysics,
    num_subshapes: u32,
) -> BinResult<Vec<ShapeDescription>> {
    Ok((0..num_subshapes)
        .flat_map(|_| -> BinResult<ShapeDescription> {
            let tag = String::from_utf8(
                [
                    u8::read_le(reader)?,
                    u8::read_le(reader)?,
                    u8::read_le(reader)?,
                ]
                .to_vec(),
            )
            .unwrap();
            let _ = u8::read_le(reader)?;
            let primitive_type = match tag.as_str() {
                "BOX" => Ok(PrimitiveGeometry::Box),
                "CAP" => Ok(PrimitiveGeometry::Capsule),
                "SPH" => Ok(PrimitiveGeometry::Sphere),
                _ => Err(Error::new(
                    ErrorKind::Other,
                    format!("Unknown primitive shape found: {}", tag),
                )),
            }?;

            let geometry = physics.read_primitive(reader, primitive_type).unwrap();
            let collision_layer = CollidableLayer::read_le(reader)?;
            let is_opaque = u32::read_le(reader)? > 0;
            let transform = Transform::read(reader)?;
            Ok(ShapeDescription {
                geometry,
                transform,
                collision_layer,
                is_opaque,
            })
        })
        .collect())
}

#[derive(Debug, BinRead)]
#[br(import(physics: &PxPhysics))]
pub enum CollisionShape {
    Convex(#[br(args(physics))] ConvexMeshDescription),
    Triangle(#[br(args(physics))] TriangleMeshDescription),
    Primitive(#[br(args(physics))] PrimitiveMeshDescription),
}

impl CollisionShape {
    pub fn base_shape(&self) -> String {
        match self {
            CollisionShape::Convex(_) => "Convex",
            CollisionShape::Triangle(_) => "Triangle",
            CollisionShape::Primitive(_) => "Primitive",
        }
        .to_string()
    }

    pub fn subshapes(&self) -> Option<&Vec<ShapeDescription>> {
        match self {
            CollisionShape::Convex(cvx) => Some(cvx.subshapes()),
            CollisionShape::Triangle(tri) => Some(tri.subshapes()),
            CollisionShape::Primitive(icp) => Some(icp.subshapes()),
        }
    }
}

impl AsRef<CollisionShape> for CollisionShape {
    fn as_ref(&self) -> &Self {
        self
    }
}

#[derive(Debug, BinRead)]
#[br(import(physics: &PxPhysics))]
#[br(assert(primitive_collider.as_ref().map_or(true, |val| val.base_shape() == "Primitive")))]
#[br(assert(convex_collider.as_ref().map_or(true, |val| val.base_shape() == "Convex")))]
pub struct ShatterBoneShard
{
    pub id: u32,
    pub parent_id: u32,
    pub name: NullString,
    pub(crate) unki3: u32,
    #[br(map = |x: u32| x != 0)]
    pub(crate)unkb1: bool, // result of ZPhysicsDataPacker::IsCollisionTypePresent?
    pub(crate)unkf1: f32, // mass?
    pub material_resource_id: NullString,
    num_connections: u32,
    #[br(count = num_connections)]
    pub connections: Vec<u32>, //could also be called children. I think?
    primitive_collider_size: u32,
    convex_collider_size: u32,

    #[br(args(physics,))]
    #[br(if(primitive_collider_size > 0))]
    pub primitive_collider : Option<CollisionShape>,

    #[br(args(physics,))]
    #[br(if(convex_collider_size > 0))]
    pub convex_collider : Option<CollisionShape>,
}

#[derive(Debug, BinRead)]
#[br(assert(tag == "BCP"))]
#[br(import(physics: &PxPhysics))]
pub struct ShatterData{
    #[br(count = 3, try_map = |s: Vec<u8>| String::from_utf8(s))]
    #[br(pad_after(1))]
    tag: String,
    size: u32,

    #[br(args{inner: (physics,)})]
    #[br(count = size)]
    pub bone_shards: Vec<ShatterBoneShard>,
}


#[derive(Debug, BinRead)]
#[br(import(physics: &PxPhysics))]
#[br(assert(primitive_collider.as_ref().map_or(true, |val| val.base_shape() == "Primitive")))]
pub struct KinematicBoneData
{
    pub global_id: u32,
    pub parent_id: u32,
    pub debug_name: NullString,
    primitive_collider_size: u32,

    #[br(args(physics,))]
    #[br(if(primitive_collider_size > 0))]
    pub primitive_collider : Option<CollisionShape>,
}


#[derive(Debug, BinRead)]
#[br(assert(tag == "KBP"))]
#[br(import(physics: &PxPhysics))]
pub struct KinematicLinkedData {
    #[br(count = 3, try_map = |s: Vec<u8>| String::from_utf8(s))]
    #[br(pad_after(1))]
    tag: String,
    size: u32,

    #[br(args{inner: (physics,)})]
    #[br(count = size)]
    pub kinematic_bones: Vec<KinematicBoneData>,
}

#[derive(Debug, BinRead)]
#[br(assert(magic.version == 5))]
pub struct PhysicsResourceData {
    #[br(try_calc = create_physics_system())]
    physics_system: ManuallyDrop<PhysicsSystem>,

    pub collision_flags: CollisionPack,
    pub object_type: ObjectEntity,
    magic: PhysXMagic,
    #[br(if(object_type == ObjectEntity::Static || object_type == ObjectEntity::RigidBody || object_type == ObjectEntity::BackwardCompatible))]
    #[br(args{inner: (&physics_system.physics,)})]
    #[br(count = collision_flags.num_shapes())]
    pub collision_shapes: Vec<CollisionShape>,

    #[br(if(object_type == ObjectEntity::Shatter))]
    #[br(args(&physics_system.physics))]
    pub shatter_data: Option<ShatterData>,

    #[br(if(object_type == ObjectEntity::KinematicLinked))]
    #[br(args(&physics_system.physics))]
    pub kinematic_linked_data: Option<KinematicLinkedData>,
}

impl Drop for PhysicsResourceData {
    fn drop(&mut self) {
        let shapes = std::mem::take(&mut self.collision_shapes);
        drop(shapes);

        if let Some(shatter_data) = &mut self.shatter_data {
            let bone_shards = std::mem::take(&mut shatter_data.bone_shards);
            drop(bone_shards);
        }

        unsafe {
            ManuallyDrop::drop(&mut self.physics_system);
        }
    }
}

fn create_physics_system() -> BinResult<ManuallyDrop<PhysicsSystem>> {
    Ok(ManuallyDrop::new(PhysicsSystem::new().unwrap()))
}

#[derive(Debug, PartialEq)]
struct PhysicsSystem {
    pub physics: PxPhysics,
    pub foundation: PhysxFoundation,
}

impl PhysicsSystem {
    pub fn new() -> Option<Self> {
        let foundation = PhysxFoundation::new().ok()?;
        let physics = PxPhysics::new(&foundation).ok()?;
        Some(Self {
            foundation,
            physics,
        })
    }
}
