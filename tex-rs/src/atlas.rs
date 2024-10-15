use binrw::binrw;
use serde::{Deserialize, Serialize};

/// Represents a vertex in a tile polygon, used in atlas data.
#[binrw]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TilePolygonVertex {
    pub pos_lerp_x: f32,
    pub pos_lerp_y: f32,
    pub text_uv_x: f32,
    pub text_uv_y: f32,
}

#[binrw]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AtlasData {
    #[br(temp)]
    #[bw(calc(self.polygon_vertex_count() as u32))]
    pub polygon_vertex_count: u32,
    /// The amount of columns in the atlas.
    pub width: u32,
    /// The amount of rows in the atlas.
    pub height: u32,

    #[br(count = (width * height * polygon_vertex_count) as usize)]
    pub polygon_vertices: Vec<TilePolygonVertex>,
}

impl AtlasData {
    pub fn polygon_vertex_count(&self) -> usize {
        self.polygon_vertices.len() / (self.width * self.height) as usize
    }

    pub(crate) fn size(&self) -> usize {
        (3 * size_of::<u32>()) + (self.polygon_vertices.len() * size_of::<TilePolygonVertex>())
    }

    /// Retrieves the polygon vertices for a specific tile given its column and row.
    pub fn get_tile_vertices(&self, column: u32, row: u32) -> Option<&[TilePolygonVertex]> {
        if column >= self.width || row >= self.height {
            return None;
        }
        let tile_index = (row * self.width + column) as usize;
        let start = tile_index * self.polygon_vertex_count();
        let end = start + self.polygon_vertex_count();
        self.polygon_vertices.get(start..end)
    }

    /// Iterates over all tiles, yielding (column, row, &[TilePolygonVertex]).
    pub fn iter_tiles(&self) -> impl Iterator<Item=(u32, u32, &[TilePolygonVertex])> + '_ {
        self.polygon_vertices
            .chunks(self.polygon_vertex_count())
            .enumerate()
            .map(move |(i, vertices)| {
                let row = (i as u32) / self.width;
                let column = (i as u32) % self.width;
                (column, row, vertices)
            })
    }



    pub fn new_grid(width: u32, height: u32) -> Self {
        let polygon_vertex_count = 4;

        let total_tiles = width * height;
        let total_vertices = total_tiles * polygon_vertex_count;

        let mut polygon_vertices = Vec::with_capacity(total_vertices as usize);

        for row in 0..height {
            for col in 0..width {

                let vertices = [
                    // top-left vertex
                    TilePolygonVertex {
                        pos_lerp_x: 0.0,
                        pos_lerp_y: 0.0,
                        text_uv_x: col as f32 / width as f32,
                        text_uv_y: row as f32 / height as f32,
                    },
                    // top-right vertex
                    TilePolygonVertex {
                        pos_lerp_x: 1.0,
                        pos_lerp_y: 0.0,
                        text_uv_x: (col + 1) as f32 / width as f32,
                        text_uv_y: row as f32 / height as f32,
                    },
                    // bottom-right vertex
                    TilePolygonVertex {
                        pos_lerp_x: 1.0,
                        pos_lerp_y: 1.0,
                        text_uv_x: (col + 1) as f32 / width as f32,
                        text_uv_y: (row + 1) as f32 / height as f32,
                    },
                    // bottom-left vertex
                    TilePolygonVertex {
                        pos_lerp_x: 0.0,
                        pos_lerp_y: 1.0,
                        text_uv_x: col as f32 / width as f32,
                        text_uv_y: (row + 1) as f32 / height as f32,
                    },
                ];

                polygon_vertices.extend_from_slice(&vertices);
            }
        }

        Self {
            width,
            height,
            polygon_vertices,
        }
    }
}