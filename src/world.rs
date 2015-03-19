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
struct Solid {
  name: &'static str,
  color: Color<f32>,
}

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
  // Wall,
  Solid(TypeId),
  Granular(TypeId, bool, bool),
  Fluid(TypeId, f64),
  WaterGenerator,
  SandGenerator,
  Sink,
  Plant,
  Fire,
  Torch,
  ExplodingNitro(Vec2<i32>),
}

#[derive(Copy, Clone)]
pub struct Cell {
  pub typ: CellType,
  // pub temp: f64,
  pub heat: f64,
}

const min_fluid: f64 = 0.001; //Ignore fluid cells that are almost dry - pretend the fluid evaporated

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
      // CellType::Wall => "wall",
      CellType::Solid(id) => grid.solid[id as usize].name,
      CellType::Granular(id, _, _) => grid.granular[id as usize].name,
      CellType::Fluid(id, _) => grid.fluid[id as usize].name,
      CellType::WaterGenerator => "water generator",
      CellType::SandGenerator => "sand generator",
      CellType::Sink => "sink",
      CellType::Plant => "plant",
      CellType::Fire => "fire",
      CellType::Torch => "torch",
      CellType::ExplodingNitro(..) => "exploding nitro",
    }
  }

  pub fn conductivity(self, grid: &Grid) -> f64 {
    // TODO: using self.typ here results in ICE
    match self {
      CellType::Empty => 0.5,
      CellType::Solid(id) => 0.25,
      CellType::Granular(id, _, _) => 0.1,
      CellType::Fluid(id, amount) => 0.1,
      _ => 0.1,
    }
  }

  pub fn heat_cap(self, grid: &Grid) -> f64 {
    match self {
      CellType::Empty => 0.05,
      CellType::Solid(id) => 1.0,
      CellType::Granular(id, _, _) => 1.0,
      CellType::Fluid(id, amount) => 1.0,
      _ => 1.0,
    }
  }
}

impl Cell {
  pub fn color(self, grid: &Grid) -> Color<f32> {
    match self.typ {
      CellType::Empty => background_color(),
      // CellType::Wall => Color::rgb(0.5, 0.5, 0.5),
      CellType::Solid(id) => grid.solid[id as usize].color,
      CellType::Granular(id, _, _) => grid.granular[id as usize].color,
      CellType::Fluid(id, amount) => grid.fluid[id as usize].color.blend(background_color(), (amount as f32/1.0).min(1.0).max(0.5)),
      CellType::WaterGenerator => Color::rgb(0.0, 0.5, 1.0),
      CellType::SandGenerator => Color::rgb(0.9, 0.5, 0.2),
      CellType::Sink => Color::black(),
      CellType::Plant => Color::green()*0.6,
      CellType::Fire => Color::rgb(1.0, 0.325, 0.0),
      CellType::Torch => Color::rgb(1.0, 0.1, 0.0),
      CellType::ExplodingNitro(..) => Color::rgb(0.3, 0.5, 0.3),
    }
  }

  pub fn temp(self, grid: &Grid) -> f64 {
    let amount = match self.typ {
      CellType::Fluid(_, amount) => amount,
      _ => 1.0
    };
    // TODO: the amount should never be zero
    if amount == 0.0 {0.0} else {self.typ.heat_cap(grid) * self.heat / amount}
  }

  fn temp_color(self, grid: &Grid) -> Color<f32> {
    Color::red().blend(Color::blue(), self.temp(grid) as f32 / 200.0)
  }

  // TODO: fix this hack
  pub fn handle_temp<R: Rng>(self, grid: &mut Grid, pos: Vec2<i32>, rng: &mut R) {
    /*let temp = self.temp(grid);
    // println!("{}", temp);
    match self.typ {
      CellType::Fluid(0, amount) if temp > 100.0 => {grid[pos].typ = CellType::Fluid(3, amount);}
      CellType::Fluid(3, amount) if temp < 100.0 => {grid[pos].typ = CellType::Fluid(0, amount);}
      _ => {}
    }*/
  }

  pub fn simulate<R: Rng>(self, grid: &mut Grid, pos: Vec2<i32>, rng: &mut R) {
    // Skip cells that have already been updated
    if grid.updated(pos) {
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
      CellType::Fluid(4, amount) if amount >= 1.0 && grid.in_range(pos+down) => {
        let allow = match grid[pos+down].typ {
          CellType::Fluid(..) => false,
          CellType::Empty => false,
          _ => true
        };
        if rng.gen::<f64>() < 0.05 && allow {
          grid[pos].typ = CellType::Solid(0);
          return;
        }
      },
      CellType::Granular(3, _, _) if grid.in_range(pos+down+left) &&
        grid.in_range(pos+up+right) => {
        if rng.gen::<f64>() < 0.1 {
          for x in range_inclusive(-1, 1) {
            for y in range_inclusive(-1, 1) {
              if x != 0 || y != 0 {
                grid[pos+Vec2(x,y)].typ = CellType::ExplodingNitro(Vec2(x,y));
              }
            }
          }
          grid[pos].typ = CellType::Empty;
          return;
        }
      },
      _ => ()
    }

    match self.typ {
      CellType::Granular(id, settled_45, settled_90) => {
        let typ = grid.granular[id as usize];
        if can_move_down || ((can_move_d_left || can_move_d_right) && !settled_90 && rng.gen::<f64>() < typ.spread_speed) || ((can_move_left || can_move_right) && !settled_45 && (rng.gen::<f64>() < 0.2)) {
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
          grid.swap_temps(pos, new_pos);
          grid[pos].typ = CellType::Empty;
          grid[new_pos].typ = CellType::Granular(id, rng.gen::<f64>() < typ.granularity_45, rng.gen::<f64>() < typ.granularity_90);
          if typ.fall_speed <= 1.0 || rng.gen::<f64>() < 2.0-typ.fall_speed {
            grid.update(new_pos);
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
      CellType::Torch => {
        if can_move_up {
          grid[pos+up].typ = CellType::Fire;
        }
      },
      CellType::ExplodingNitro(dir) => {
        if grid.in_range(pos+dir) && rng.gen::<f64>() < 0.6 {
          grid[pos+dir].typ = CellType::ExplodingNitro(dir);
          if rng.gen::<f64>() < 0.1 {
            grid[pos].typ = CellType::Empty;
          }
        } else {
          grid[pos].typ = CellType::Empty;
        }
      }
      CellType::Sink => {
        if grid.in_range(pos+up) {
          grid[pos+up].typ = CellType::Empty;
        }
      },
      CellType::Plant => {
        let mut neighbor = pos;
        let rand = rng.gen::<f64>();
        if rand < 0.25 {
          neighbor = neighbor + Vec2(1,0)
        } else if rand < 0.5 {
          neighbor = neighbor + Vec2(-1,0)
        } else if rand < 0.75 {
          neighbor = neighbor + Vec2(0,1)
        } else {
          neighbor = neighbor + Vec2(0,-1)
        }
        if grid.in_range(neighbor) {
          match grid[neighbor].typ {
            CellType::Fluid(0, amount) if amount >= 1.0 => grid[neighbor].typ = CellType::Plant,
            _ => (),
          }
        }
      },
      CellType::Fire => {
        if rng.gen::<f64>() < 0.18 && can_move_up {
          grid[pos+up].typ = CellType::Fire;
        }
        if rng.gen::<f64>() < 0.1 {
          grid[pos].typ = CellType::Empty;
        }
        let mut neighbor = pos;
        let rand = rng.gen::<f64>();
        if rand < 0.25 {
          neighbor = neighbor + Vec2(1,0)
        } else if rand < 0.5 {
          neighbor = neighbor + Vec2(-1,0)
        } else if rand < 0.75 {
          neighbor = neighbor + Vec2(0,1)
        } else {
          neighbor = neighbor + Vec2(0,-1)
        }
        if grid.in_range(neighbor) {
          match grid[neighbor].typ {
            CellType::Plant => grid[neighbor].typ = CellType::Fire,
            CellType::Fluid(1, amount) => grid[neighbor].typ = CellType::Fire,
            _ => (),
          }
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
              // TODO: temperature mixing
              grid[pos+mydown].typ = CellType::Fluid(id, amount_in_bottom);
              grid[pos].typ = CellType::Fluid(id, total_amount-amount_in_bottom);
              amount = total_amount-amount_in_bottom;
            }
            CellType::Fluid(id2, amount2) if id2 != id => {
              let other_typ = grid.fluid[id2 as usize];
              if other_typ.density < typ.density && rng.gen::<f64>() < (typ.density/other_typ.density).min(2.0) - 1.0 {
                grid.swap_temps(pos, pos+mydown);
                grid[pos].typ = CellType::Fluid(id2, amount2);
                grid[pos+mydown].typ = CellType::Fluid(id, amount);
                amount = 0.0;
              }
            }
            CellType::Empty => {
              // TODO: sometimes the mass should be split in this case
              grid.swap_temps(pos, pos+mydown);
              grid[pos].typ = CellType::Empty;
              grid[pos+mydown].typ = CellType::Fluid(id, amount);
              if typ.fall_speed <= 1.0 || rng.gen::<f64>() < 2.0-typ.fall_speed {
                grid.update(pos+mydown);
              }
              amount = 0.0;
            }
            _ => ()
          }
        }

        if amount > 0.0 && (can_move_left || can_move_right) && rng.gen::<f64>() < 1.0 {
          // TODO: add a config setting for this, spread_speed or something
          if can_move_left && !can_move_right {
            // TODO: temperature mixing
            grid[pos].typ = CellType::Fluid(id, amount*0.25);
            grid[pos+left].typ = CellType::Fluid(id, amount*0.75);
            if typ.fall_speed <= 1.0 || rng.gen::<f64>() < 2.0-typ.fall_speed {
              grid.update(pos+left);
            }
          } else if can_move_right && !can_move_left {
            // TODO: temperature mixing
            grid[pos].typ = CellType::Fluid(id, amount*0.25);
            grid[pos+right].typ = CellType::Fluid(id, amount*0.75);
            if typ.fall_speed <= 1.0 || rng.gen::<f64>() < 2.0-typ.fall_speed {
              grid.update(pos+right);
            }
          } else if rng.gen::<f64>() < 0.5 {
            // TODO: temperature mixing
            grid[pos].typ = CellType::Fluid(id, amount*0.25);
            grid[pos+left].typ = CellType::Fluid(id, amount*0.75);
            if typ.fall_speed <= 1.0 || rng.gen::<f64>() < 2.0-typ.fall_speed {
              grid.update(pos+left);
            }
          } else {
            // TODO: temperature mixing
            grid[pos].typ = CellType::Fluid(id, amount*0.25);
            grid[pos+right].typ = CellType::Fluid(id, amount*0.75);
            if typ.fall_speed <= 1.0 || rng.gen::<f64>() < 2.0-typ.fall_speed {
              grid.update(pos+right);
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
              // TODO: temperature mixing
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
              // TODO: temperature mixing
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
              // TODO: temperature mixing
              grid[pos].typ = CellType::Fluid(id, amount_in_bottom);
              grid[pos+up].typ = CellType::Fluid(id, total_amount-amount_in_bottom);
              amount = amount_in_bottom;
            }
            CellType::Empty => {
              let amount_in_bottom = stable_state(amount, typ.compressibility);
              if amount - amount_in_bottom > min_fluid {
                // TODO: temperature mixing
                grid[pos].typ = CellType::Fluid(id, amount_in_bottom);
                grid[pos+up].typ = CellType::Fluid(id, amount - amount_in_bottom);
                if typ.fall_speed <= 1.0 || rng.gen::<f64>() < 2.0-typ.fall_speed {
                  grid.update(pos+up);
                }
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
    let mut updated = Vec::new();
    for y in range(0, size.y) {
      let row = repeat(Cell{typ: CellType::Empty, heat: 10.0}).take(size.x as usize).collect();
      let updated_row = repeat(false).take(size.x as usize).collect();
      cells.push(row);
      updated.push(updated_row);
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
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(2, 3, 0);

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

    let solid = vec![
      Solid{
        name: "wall",
        color: Color::rgb(0.5, 0.5, 0.5),
      },
      Solid{
        name: "ice",
        color: Color::white().blend(background_color(), 0.65),
      },
    ];
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
      Granular{
        name: "nitro",
        granularity_45: 0.2,
        granularity_90: 0.0,
        horizontal_spread: 0.05,
        spread_speed: 0.8,
        fall_speed: 1.0,
        color: Color::rgb(0.1, 0.4, 0.05),
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
      Fluid{
        name: "steam",
        horizontal_spread: 0.05,
        fall_speed: 1.0,
        compressibility: 0.05,
        color: Color::rgb(0.0, 0.2, 1.0).blend(Color::white(), 0.6).blend(background_color(), 0.7),
        density: 0.3,
        down_dir: up_,
        up_dir: down_,
      },
      Fluid{
        name: "cement",
        horizontal_spread: 0.01,
        fall_speed: 0.5, // TODO: this doesn't seem to do anything
        compressibility: 0.01,
        color: Color::rgb(0.3, 0.3, 0.3),
        density: 2.0,
        down_dir: down_,
        up_dir: up_,
      },
    ];

    let grid = Grid{cells: cells, updated: updated, size: size, solid: solid, granular: granular, fluid: fluid};
    World{grid: grid, mesh: mesh, texture: texture, coords: coords, pixels: pixels, id: next_id()}
  }

  pub fn simulate<R: Rng>(&mut self, rng: &mut R) {
    for y in range(0, self.grid.size.y) {
      for x in range(0, self.grid.size.x) {
        match self.grid.cells[y as usize][x as usize].typ {
          CellType::Fluid(id, amount) if amount <= min_fluid => {
            assert!(amount >= 0.0);
            self.grid.cells[y as usize][x as usize].typ = CellType::Empty;
          },
          _ => ()
        }

        // self.grid.cells[y as usize][x as usize].updated = false;
        self.grid.updated[y as usize][x as usize] = false;
      }
    }

    for &coord in self.coords.iter() {
      self.grid.cells[coord.y as usize][coord.x as usize].handle_temp(&mut self.grid, coord, rng);
      self.grid.cells[coord.y as usize][coord.x as usize].simulate(&mut self.grid, coord, rng);
    }

    // TODO: fix this
    for y in range(1, self.grid.size.y-1) {
      for x in range(1, self.grid.size.x-1) {
        let cur = self.grid[Vec2(x,y)];
        for &dir in [left_, right_, up_, down_].iter() {
          let neighbor = self.grid[Vec2(x,y)+dir];
          let conductivity = cur.typ.conductivity(&self.grid) *
            neighbor.typ.conductivity(&self.grid);
          let tempDiff = neighbor.temp(&self.grid) - cur.temp(&self.grid);
          if tempDiff < 0.0 {
            let transfer_temp = -tempDiff * 0.25 * conductivity;
            // println!("{} {} {}", tempDiff, conductivity, transfer_temp);
            self.grid[Vec2(x,y)].heat -= transfer_temp;
            self.grid[Vec2(x,y)+dir].heat += transfer_temp;
          }
        }
      }
    }
  }

  pub fn update_mesh(&mut self, draw_temp: bool) {
    self.pixels.clear();
    for y in range(0, self.grid.size.y) {
      for _ in range(0, cell_size) {
        for x in range(0, self.grid.size.x) {
          for _ in range(0, cell_size) {
            let color = if draw_temp {
              self.grid.cells[y as usize][x as usize].temp_color(&self.grid)
            } else {
              self.grid.cells[y as usize][x as usize].color(&self.grid)
            };
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
  updated: Vec<Vec<bool>>,
  solid: Vec<Solid>,
  granular: Vec<Granular>,
  fluid: Vec<Fluid>,
}

impl Grid {
  pub fn updated(&self, pos: Vec2<i32>) -> bool {
    self.updated[pos.y as usize][pos.x as usize]
  }
  pub fn update(&mut self, pos: Vec2<i32>) {
    self.updated[pos.y as usize][pos.x as usize] = true;
  }

  pub fn swap_temps(&mut self, old_pos: Vec2<i32>, new_pos: Vec2<i32>) {
    let old_heat = self[old_pos].heat;
    let new_heat = self[new_pos].heat;
    self[old_pos].heat = new_heat;
    self[new_pos].heat = old_heat;
  }

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
  fn index_mut(&mut self, index: &Vec2<i32>) -> &mut Cell {
    &mut self.cells[index.y as usize][index.x as usize]
  }
}
