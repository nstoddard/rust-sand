use std::ops::{Index, IndexMut};
use rand::Rng;
use std::iter::*;

use vecmat::*;
use vecmat::num::*;

use gui::color::*;
use gui::texture::*;
use gui::mesh::*;
use gui::opengl::*;
use gui::util::*;
use gui::gui::*;
use gui::new_gl_program::*;


#[derive(Copy, Clone)]
struct Granular {
  name: &'static str,
  granularity_45: f64, //0.0-1.0
  granularity_90: f64, //0.0-1.0
  horizontal_spread: f64, //0.0-1.0
  spread_speed: f64,
  // TODO: support fall_speed < 1.0
  fall_speed: f64, //0.0-2.0
  color: Color<f32>,
}

#[derive(Copy, Clone)]
struct Fluid {
  // TODO: not all of these properties are implemented
  name: &'static str,
  horizontal_spread: f64, //0.0-1.0
  fall_speed: f64, //0.0-2.0
  compressibility: f64,
  color: Color<f32>,
  density: f64,
  // Used to determine whether the fluid falls or rises
  // If down_dir=up and vice versa, it's considered ligher than air
  down_dir: Vec2<i32>,
  up_dir: Vec2<i32>,
}

pub type TypeId = u16;

#[derive(Copy, Clone, PartialEq)]
pub enum CellType {
  Empty,
  Wall,
  Granular(TypeId, bool, bool),
  Fluid(TypeId, f64),
  WaterGenerator,
  SandGenerator,
  Sink,
}

#[derive(Copy, Clone)]
pub struct Cell {
  pub typ: CellType,
  updated: bool,
}

const min_mass: f64 = 0.000001; //Ignore fluid cells that are almost dry - pretend the fluid evaporated

pub const cell_size: i32 = 5;

/// Gets the mass in the bottom cell of a fluid
fn stable_state(total_mass: f64, compressibility: f64) -> f64 {
  if total_mass <= 1.0 {
    total_mass
  } else if total_mass <= 2.0 + compressibility {
    (1.0 + total_mass*compressibility) / (1.0 + compressibility)
  } else {
    (total_mass + compressibility) * 0.5
  }
}

fn background_color() -> Color<f32> {
  Color::rgb(95.0/255.0, 188.0/255.0, 223.0/255.0)
}

impl CellType {
  pub fn name(self, grid: &Grid) -> &str {
    match self {
      CellType::Empty => "empty",
      CellType::Wall => "wall",
      CellType::Granular(id, _, _) => grid.granular[id as usize].name,
      CellType::Fluid(id, _) => grid.fluid[id as usize].name,
      CellType::WaterGenerator => "water generator",
      CellType::SandGenerator => "sand generator",
      CellType::Sink => "sink",
    }
  }
}

impl Cell {
  pub fn color(self, grid: &Grid) -> Color<f32> {
    match self.typ {
      CellType::Empty => background_color(),
      CellType::Wall => Color::rgb(0.5, 0.5, 0.5),
      CellType::Granular(id, _, _) => grid.granular[id as usize].color,
      CellType::Fluid(id, amount) => grid.fluid[id as usize].color.blend(background_color(), (amount as f32/1.0).min(1.0).max(0.3)),
      CellType::WaterGenerator => Color::rgb(0.0, 0.5, 1.0),
      CellType::SandGenerator => Color::rgb(0.9, 0.5, 0.2),
      CellType::Sink => Color::black(),
    }
  }

  pub fn simulate<R: Rng>(self, grid: &mut Grid, pos: Vec2<i32>, rng: &mut R) {
    // Skip cells that have already been updated
    if grid[pos].updated {
      return;
    }

    // TODO: refactor this shit
    let up = up_;
    let down = down_;
    let left = left_;
    let right = right_;

    let d_left = left + down;
    let d_right = right + down;

    let can_move_down = grid.in_range(pos+down) &&
      grid[pos+down].typ == CellType::Empty;
    let can_move_d_left = grid.in_range(pos+d_left) &&
      grid[pos+d_left].typ == CellType::Empty;
    let can_move_d_right = grid.in_range(pos+d_right) &&
      grid[pos+d_right].typ == CellType::Empty;
    let can_move_left = grid.in_range(pos+left) &&
      grid[pos+left].typ == CellType::Empty;
    let can_move_right = grid.in_range(pos+right) &&
      grid[pos+right].typ == CellType::Empty;
    let can_move_up = grid.in_range(pos+up) &&
      grid[pos+up].typ == CellType::Empty;

    match self.typ {
      CellType::Granular(id, settled_45, settled_90) => {
        let typ = grid.granular[id as usize];
        if can_move_down || ((can_move_d_left || can_move_d_right) && !settled_90&& rng.gen::<f64>() < typ.spread_speed) || ((can_move_left || can_move_right) && !settled_45 && (rng.gen::<f64>() < 0.2)) {
          let new_pos = if can_move_down && (rng.gen::<f64>() < 1.0-typ.horizontal_spread ||
            (!can_move_d_left && !can_move_d_right)) {pos+down}
          else if can_move_d_left && !can_move_d_right {pos+d_left}
          else if can_move_d_right && !can_move_d_left {pos+d_right}
          else if can_move_d_left && can_move_d_right {
            if rng.gen::<f64>() < 0.5 {pos+d_left} else {pos+d_right}
          } else if can_move_left && !can_move_right {pos+left}
          else if can_move_right && !can_move_left {pos+right}
          else if rng.gen::<f64>() < 0.5 {pos+left} else {pos+right};
          assert!(grid[new_pos].typ == CellType::Empty);
          grid[pos].typ = CellType::Empty;
          grid[new_pos].typ = CellType::Granular(id, rng.gen::<f64>() < typ.granularity_45, rng.gen::<f64>() < typ.granularity_90);
          if typ.fall_speed <= 1.0 || rng.gen::<f64>() < 2.0-typ.fall_speed {
            grid[new_pos].updated = true;
          }
        }
      },
      CellType::WaterGenerator => {
        if can_move_down {
          grid[pos+down].typ = CellType::Fluid(0, 1.0);
        }
      },
      CellType::SandGenerator => {
        if can_move_down {
          grid[pos+down].typ = CellType::Granular(0, false, false);
        }
      },
      CellType::Sink => {
        if grid.in_range(pos+up) {
          grid[pos+up].typ = CellType::Empty;
        }
      },

      CellType::Fluid(id, mut amount) => {
        let typ = grid.fluid[id as usize];
        let up = typ.up_dir;
        let down = typ.down_dir;

        let d_left = left + down;
        let d_right = right + down;


        let mydown = if rng.gen::<f64>() < 1.0-typ.horizontal_spread {down}
          else if rng.gen::<f64>() < 0.5 {d_left} else {d_right};
        if grid.in_range(pos+mydown) {
          match grid[pos+mydown].typ {
            CellType::Fluid(id2, amount2) if id2 == id => {
              let total_amount = amount + amount2;
              let amount_in_bottom = stable_state(total_amount, typ.compressibility);
              let amount_in_bottom = amount_in_bottom.min(amount2+1.0);
              if amount_in_bottom > total_amount {
                println!("Down: {}/{}", amount_in_bottom, total_amount);
              }
              grid[pos+mydown].typ = CellType::Fluid(id, amount_in_bottom);
              grid[pos].typ = CellType::Fluid(id, total_amount-amount_in_bottom);
              amount = total_amount-amount_in_bottom;
            }
            CellType::Fluid(id2, amount2) if id2 != id => {
              let other_typ = grid.fluid[id2 as usize];
              if other_typ.density < typ.density && rng.gen::<f64>() < (typ.density/other_typ.density).min(2.0) - 1.0 {
                grid[pos].typ = CellType::Fluid(id2, amount2);
                grid[pos+mydown].typ = CellType::Fluid(id, amount);
                amount = 0.0;
              }
            }
            CellType::Empty => {
              // TODO: sometimes the mass should be split in this case
              grid[pos].typ = CellType::Empty;
              grid[pos+mydown].typ = CellType::Fluid(id, amount);
              if typ.fall_speed <= 1.0 || rng.gen::<f64>() < 2.0-typ.fall_speed {
                grid[pos+mydown].updated = true;
              }
              amount = 0.0;
            }
            _ => ()
          }
        }

        if amount > 0.0 && (can_move_left || can_move_right) && rng.gen::<f64>() < 1.0 {
          // TODO: add a config setting for this, spread_speed or something
          if can_move_left && !can_move_right {
            grid[pos].typ = CellType::Fluid(id, amount*0.25);
            grid[pos+left].typ = CellType::Fluid(id, amount*0.75);
            if typ.fall_speed <= 1.0 || rng.gen::<f64>() < 2.0-typ.fall_speed {
              grid[pos+left].updated = true;
            }
          } else if can_move_right && !can_move_left {
            grid[pos].typ = CellType::Fluid(id, amount*0.25);
            grid[pos+right].typ = CellType::Fluid(id, amount*0.75);
            if typ.fall_speed <= 1.0 || rng.gen::<f64>() < 2.0-typ.fall_speed {
              grid[pos+right].updated = true;
            }
          } else if rng.gen::<f64>() < 0.5 {
            grid[pos].typ = CellType::Fluid(id, amount*0.25);
            grid[pos+left].typ = CellType::Fluid(id, amount*0.75);
            if typ.fall_speed <= 1.0 || rng.gen::<f64>() < 2.0-typ.fall_speed {
              grid[pos+left].updated = true;
            }
          } else {
            grid[pos].typ = CellType::Fluid(id, amount*0.25);
            grid[pos+right].typ = CellType::Fluid(id, amount*0.75);
            if typ.fall_speed <= 1.0 || rng.gen::<f64>() < 2.0-typ.fall_speed {
              grid[pos+right].updated = true;
            }
          }
          amount = amount * 0.25;
        }

        let (dir1, dir2) = if rng.gen::<f64>() < 0.5 {(left, right)}
          else {(right, left)};

        if amount > 0.0 && grid.in_range(pos+dir1) {
          match grid[pos+dir1].typ {
            CellType::Fluid(id2, amount2) if id2 == id => {
              let mut flow = (amount - amount2)*0.5;
              flow = flow.min(amount).max(0.0);
              grid[pos].typ = CellType::Fluid(id, amount-flow);
              grid[pos+dir1].typ = CellType::Fluid(id, amount2+flow);
              amount -= flow;
            },
            _ => ()
          }
        }

        if amount > 0.0 && grid.in_range(pos+dir2) {
          match grid[pos+dir2].typ {
            CellType::Fluid(id2, amount2) if id2 == id => {
              let mut flow = (amount - amount2)*0.5;
              flow = flow.min(amount).max(0.0);
              grid[pos].typ = CellType::Fluid(id, amount-flow);
              grid[pos+dir2].typ = CellType::Fluid(id, amount2+flow);
              amount -= flow;
            },
            _ => ()
          }
        }

        if amount > 0.0 && grid.in_range(pos+up) {
          match grid[pos+up].typ {
            CellType::Fluid(id2, amount2) if id2 == id => {
              let total_amount = amount + amount2;
              let amount_in_bottom = stable_state(total_amount, typ.compressibility);
              if amount_in_bottom > total_amount {
                println!("Up: {}/{}", amount_in_bottom, total_amount);
              }
              grid[pos].typ = CellType::Fluid(id, amount_in_bottom);
              grid[pos+up].typ = CellType::Fluid(id, total_amount-amount_in_bottom);
              amount = amount_in_bottom;
            }
            CellType::Empty => {
              let amount_in_bottom = stable_state(amount, typ.compressibility);
              grid[pos].typ = CellType::Fluid(id, amount_in_bottom);
              grid[pos+up].typ = CellType::Fluid(id, amount - amount_in_bottom);
              if typ.fall_speed <= 1.0 || rng.gen::<f64>() < 2.0-typ.fall_speed {
                grid[pos+up].updated = true;
              }
            }
            _ => ()
          }
        }
        assert!(amount >= 0.0);
      },
      _ => ()
    }
  }
}


// TODO: get rid of this hack
const up_: Vec2<i32> = Vec2{x: 0, y: -1};
const down_: Vec2<i32> = Vec2{x: 0, y: 1};
const left_: Vec2<i32> = Vec2{x: -1, y: 0};
const right_: Vec2<i32> = Vec2{x: 1, y: 0};




pub struct World {
  id: Id,
  mesh: NewMesh<UnlitProgram>,
  texture: Texture,
  coords: Vec<Vec2<i32>>,
  pub grid: Grid,
  pixels: Vec<u8>,
}

impl World {
  pub fn new<R: Rng>(size: Vec2<i32>, window: &GUIWindow, rng: &mut R) -> World {
    let mut cells = Vec::new();
    for y in range(0, size.y) {
      let row = repeat(Cell{typ: CellType::Empty, updated: false}).take(size.x as usize).collect();
      cells.push(row);
    }
    let mut mesh = NewMesh::new(window.unlit_program.clone(), Primitive::Triangles, MeshUsage::DynamicDraw);
    mesh.add_vertex(UnlitVertex{
      pos: Vec2(0.0, 0.0),
      texcoord: Vec2(0.0, 0.0),
    });
    mesh.add_vertex(UnlitVertex{
      pos: Vec2(size.x as f32, 0.0),
      texcoord: Vec2(1.0, 0.0),
    });
    mesh.add_vertex(UnlitVertex{
      pos: Vec2(size.x as f32, size.y as f32),
      texcoord: Vec2(1.0, 1.0),
    });
    mesh.add_vertex(UnlitVertex{
      pos: Vec2(0.0, size.y as f32),
      texcoord: Vec2(0.0, 1.0),
    });
    mesh.triangle(0, 1, 2);
    mesh.triangle(2, 3, 0);

    let texture = Texture::texture2d_empty(size.x*cell_size, size.y*cell_size, gl::RGB8,
      MinNearest, MagNearest);
    let pixels = Vec::with_capacity((size.x*size.y*3) as usize);

    // We use a pre-shuffled list of coordinates to get rid of some poblems
    // in the simulation. Without it, some materials would prefer to move
    // to the left and others would prefer to move to the right.
    // Ideally we'd shuffle it each frame, but that's way too expensive.
    // This is close enough.
    let mut coords = Vec::new();
    for y in range(0, size.y) {
      for x in range(0, size.x) {
        coords.push(Vec2(x,y));
      }
    }
    rng.shuffle(coords.as_mut_slice());

    // TODO: move these to a config file
    let granular = vec![
      Granular{
        name: "sand",
        granularity_45: 0.1,
        granularity_90: 0.0,
        horizontal_spread: 0.05,
        spread_speed: 0.8,
        fall_speed: 1.0,
        color: Color::yellow(),
      },
      Granular{
        name: "dirt",
        granularity_45: 0.4,
        granularity_90: 0.0,
        horizontal_spread: 0.05,
        spread_speed: 0.8,
        fall_speed: 1.0,
        color: Color::rgb(0.3, 0.13, 0.0),
      },
      Granular{
        name: "snow",
        granularity_45: 1.0,
        granularity_90: 0.3,
        horizontal_spread: 0.05,
        spread_speed: 0.8,
        fall_speed: 1.0,
        color: Color::rgb(1.0, 1.0, 1.0),
      },
    ];
    let fluid = vec![
      Fluid{
        name: "water",
        horizontal_spread: 0.05,
        fall_speed: 1.0,
        compressibility: 0.05,
        color: Color::rgb(0.0, 0.2, 1.0),
        density: 1.0,
        down_dir: down_,
        up_dir: up_,
      },
      Fluid{
        name: "oil",
        horizontal_spread: 0.05,
        fall_speed: 1.0,
        compressibility: 0.05,
        color: Color::rgb(0.5, 0.3, 0.0),
        density: 0.9,
        down_dir: down_,
        up_dir: up_,
      },
      Fluid{
        name: "methane",
        horizontal_spread: 0.05,
        fall_speed: 1.0,
        compressibility: 0.05,
        color: Color::rgb(0.15, 0.1, 0.1).blend(background_color(), 0.8),
        density: 0.5,
        down_dir: up_,
        up_dir: down_,
      },
    ];

    let grid = Grid{cells: cells, size: size, granular: granular, fluid: fluid};
    World{grid: grid, mesh: mesh, texture: texture, coords: coords, pixels: pixels, id: next_id()}
  }

  pub fn simulate<R: Rng>(&mut self, rng: &mut R) {
    for y in range(0, self.grid.size.y) {
      for x in range(0, self.grid.size.x) {
        self.grid.cells[y as usize][x as usize].updated = false;
      }
    }

    for &coord in self.coords.iter() {
      self.grid.cells[coord.y as usize][coord.x as usize].simulate(&mut self.grid, coord, rng);
    }

    for y in range(0, self.grid.size.y) {
      for x in range(0, self.grid.size.x) {
        match self.grid.cells[y as usize][x as usize].typ {
          CellType::Fluid(id, amount) if amount <= min_mass => {
            assert!(amount >= 0.0);
            self.grid.cells[y as usize][x as usize].typ = CellType::Empty;
          },
          _ => ()
        }
      }
    }
  }

  pub fn update_mesh(&mut self) {
    self.pixels.clear();
    for y in range(0, self.grid.size.y) {
      for _ in range(0, cell_size) {
        for x in range(0, self.grid.size.x) {
          for _ in range(0, cell_size) {
            let color = self.grid.cells[y as usize][x as usize].color(&self.grid);
            self.pixels.push((color.r*255.0) as u8);
            self.pixels.push((color.g*255.0) as u8);
            self.pixels.push((color.b*255.0) as u8);
          }
        }
      }
    }
    self.texture.update_texture2d_from_pixels(&self.pixels);
  }
}

impl Widget for World {
  fn id(&self) -> Id {self.id}
  fn draw(&mut self, pos: Vec2<i32>, size: Vec2<i32>, window: &mut GUIWindow) {
    self.mesh.draw(UnlitUniforms{
      model_view_matrix: Mat4::generic_ortho(
      Vec2::zero(), Vec2(self.grid.size.x as f32, self.grid.size.y as f32),
      pos.cvt::<Vec2<f32>>(), (pos+size).cvt::<Vec2<f32>>()),
      proj_matrix: Mat4::ortho_flip(window.window_size.x as f32, window.window_size.y as f32),
      tex: &self.texture,
    });
  }

  fn min_size(&self, window: &mut GUIWindow) -> Vec2<i32> {
    self.grid.size * cell_size
  }
}



pub struct Grid {
  pub size: Vec2<i32>,
  cells: Vec<Vec<Cell>>,
  granular: Vec<Granular>,
  fluid: Vec<Fluid>,
}

impl Grid {
  pub fn in_range(&self, pos: Vec2<i32>) -> bool {
    pos.x >= 0 && pos.y >= 0 && pos.x < self.size.x && pos.y < self.size.y
  }
}

impl Index<Vec2<i32>> for Grid {
  type Output = Cell;
  fn index(&self, index: &Vec2<i32>) -> &Cell {
    &self.cells[index.y as usize][index.x as usize]
  }
}

impl IndexMut<Vec2<i32>> for Grid {
  type Output = Cell;
  fn index_mut(&mut self, index: &Vec2<i32>) -> &mut Cell {
    &mut self.cells[index.y as usize][index.x as usize]
  }
}
