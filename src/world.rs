extern crate glium;

use num::range_inclusive;

use std::ops::{Index, IndexMut};
use rand::Rng;
use std::iter::repeat;
use std::collections::*;
use std::borrow::Cow;

use vecmat::*;
use vecmat::num_ext::*;

use glium::{texture, index};
use glium::texture::*;
use glium::backend::Facade;
use glium::uniforms::*;
use glium::draw_parameters::*;
use glium::Surface;


use gui::color::*;
use gui::util::*;
use gui::widgets::*;
use gui::window::*;


#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum SolidType {Wall, Ice}

#[derive(Copy, Clone)]
pub struct Solid {
  typ: SolidType,
  name: &'static str,
  color: Color3,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum GranularType {Sand, Dirt, Snow, Nitro}

#[derive(Copy, Clone)]
pub struct Granular {
  typ: GranularType,
  name: &'static str,
  granularity_45: f64, //0.0-1.0
  granularity_90: f64, //0.0-1.0
  horizontal_spread: f64, //0.0-1.0
  spread_speed: f64,
  // TODO: support fall_speed < 1.0
  fall_speed: f64, //0.0-2.0
  color: Color3,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum FluidType {Water, Oil, Methane, Steam, Cement}

#[derive(Copy, Clone)]
pub struct Fluid {
  typ: FluidType,
  // TODO: not all of these properties are implemented
  name: &'static str,
  horizontal_spread: f64, //0.0-1.0
  fall_speed: f64, //0.0-2.0
  compressibility: f64,
  color: Color3,
  density: f64,
  // Used to determine whether the fluid falls or rises
  down_dir: Vec2<i32>,
  up_dir: Vec2<i32>,
}

// pub type TypeId = u16;

pub const virus_lifetime: i32 = 10;
pub const tail_lifetime: i32 = 2;

#[derive(Copy, Clone, PartialEq)]
pub enum CellType {
  Empty,
  // Wall,
  Solid(SolidType),
  Granular(GranularType, bool, bool),
  Fluid(FluidType, f64),
  WaterGenerator,
  SandGenerator,
  Destroyer,
  Plant,
  Fire,
  Torch,
  ExplodingNitro(Vec2<i32>),
  LifeOn,
  LifeTurningOn,
  Wire(WireType),
  ElectronHead(WireType),
  ElectronTail(WireType, i32),
  Eater,
  Fuse(bool),
  Virus(i32),
}

#[derive(Copy, Clone, PartialEq)]
pub enum WireType {
  Normal,
  Input,
  Output,
  Nor(bool),
  SignalEmitter,
}

impl WireType {
  pub fn can_signal_to(self, out: WireType) -> bool {
    match (self, out) {
      (WireType::Output, WireType::Input) => false,
      (WireType::Output, _) => true,
      (WireType::Input, WireType::Nor(_)) => true,
      (WireType::Nor(_), WireType::Nor(_)) => true,
      (WireType::Normal, WireType::Nor(false)) => true,
      (WireType::Nor(false), WireType::Output) => true,
      (WireType::Nor(true), WireType::Output) => false,
      (WireType::SignalEmitter, _) => true,
      (_, WireType::SignalEmitter) => false,


      (WireType::Nor(_), _) => false,
      (_, WireType::Nor(_)) => false,

      (_, _) => true
      /*(WireType::Normal, _) => true,
      (_, WireType::Normal) => true,
      (_, WireType::Output) => true,
      (WireType::Output, _) => true,
      (_, WireType::Input) => true,*/
    }
  }

  pub fn transition_to(self) -> Self {
    match self {
      WireType::Nor(_) => WireType::Nor(false),
      x => x
    }
  }
}

#[derive(Copy, Clone)]
pub struct Cell {
  pub typ: CellType,
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

fn background_color() -> Color3 {
  Color3::rgb(95.0/255.0, 188.0/255.0, 223.0/255.0)
}

impl CellType {
  pub fn name(self, grid: &Grid) -> &str {
    match self {
      CellType::Empty => "empty",
      CellType::Solid(typ) => grid.solid[&typ].name,
      CellType::Granular(typ, _, _) => grid.granular[&typ].name,
      CellType::Fluid(typ, _) => grid.fluid[&typ].name,
      CellType::WaterGenerator => "water generator",
      CellType::SandGenerator => "sand generator",
      CellType::Destroyer => "destroyer",
      CellType::Plant => "plant",
      CellType::Fire => "fire",
      CellType::Torch => "torch",
      CellType::ExplodingNitro(..) => "exploding nitro",
      CellType::LifeOn => "life cell",
      CellType::LifeTurningOn => "life cell",
      CellType::Wire(WireType::Normal) => "wire",
      CellType::Wire(WireType::Input) => "wire input",
      CellType::Wire(WireType::Output) => "wire output",
      CellType::Wire(WireType::Nor(_)) => "wire nor",
      CellType::Wire(WireType::SignalEmitter) => "signal emitter",
      CellType::ElectronHead(_) => "electron head",
      CellType::ElectronTail(_, _) => "electron tail",
      CellType::Eater => "eater",
      CellType::Fuse(_) => "fuse",
      CellType::Virus(_) => "virus",
    }
  }
}

impl Cell {
  pub fn color(self, grid: &Grid) -> Color3 {
    match self.typ {
      CellType::Empty => background_color(),
      // CellType::Wall => Color3::rgb(0.5, 0.5, 0.5),
      CellType::Solid(typ) => grid.solid[&typ].color,
      CellType::Granular(typ, _, _) => grid.granular[&typ].color,
      CellType::Fluid(typ, amount) => grid.fluid[&typ].color.blend(background_color(), (amount as f32/1.0).min(1.0).max(0.5)),
      CellType::WaterGenerator => Color3::rgb(0.0, 0.5, 1.0),
      CellType::SandGenerator => Color3::rgb(0.9, 0.5, 0.2),
      CellType::Destroyer => Color3::black(),
      CellType::Plant => Color3::green()*0.6,
      CellType::Fire => Color3::rgb(1.0, 0.325, 0.0),
      CellType::Torch => Color3::rgb(1.0, 0.1, 0.0),
      CellType::ExplodingNitro(..) => Color3::rgb(0.3, 0.5, 0.3),
      CellType::LifeOn => Color3::rgb(1.0, 1.0, 1.0),
      CellType::LifeTurningOn => Color3::rgb(0.8, 0.8, 0.8),
      CellType::Wire(WireType::Normal) => Color3::rgb(0.8, 0.4, 0.0),
      CellType::Wire(WireType::Input) => Color3::rgb(0.8, 0.4, 0.5),
      CellType::Wire(WireType::Output) => Color3::rgb(0.8, 0.9, 0.5),
      CellType::Wire(WireType::Nor(false)) => Color3::rgb(0.7, 0.4, 0.2),
      CellType::Wire(WireType::Nor(true)) => Color3::rgb(0.9, 0.3, 0.1),
      CellType::Wire(WireType::SignalEmitter) => Color3::rgb(1.0, 1.0, 0.7),
      CellType::ElectronHead(_) => Color3::rgb(1.0, 1.0, 0.5),
      CellType::ElectronTail(_, _) => Color3::rgb(0.5, 0.2, 1.0),
      CellType::Eater => Color3::black(),
      CellType::Fuse(false) => Color3::rgb(0.5, 0.1, 0.0),
      CellType::Fuse(true) => Color3::rgb(0.8, 0.15, 0.0),
      CellType::Virus(lifetime) => Color3::rgb(1.0, 0.25, 0.0).blend(background_color(), lifetime.max(0) as f32 / (virus_lifetime+2) as f32),
    }
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
      CellType::Fluid(FluidType::Cement, amount) if /*amount >= 1.0 &&*/ grid.in_range(pos+down) => {
        let allow = match grid[pos+down].typ {
          CellType::Fluid(..) => false,
          CellType::Empty => false,
          _ => true
        };
        if rng.gen::<f64>() < 0.05 && allow {
          grid[pos].typ = CellType::Solid(SolidType::Wall);
          return;
        }
      },
      CellType::Granular(GranularType::Nitro, _, _) if grid.in_range(pos+down+left) &&
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
      CellType::Fluid(FluidType::Steam, amount) => {
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
            CellType::Solid(SolidType::Ice) => {
              if rng.gen::<f64>() < 0.02 {
                grid[neighbor].typ = CellType::Fluid(FluidType::Water, 1.0);
                grid[pos].typ = CellType::Fluid(FluidType::Water, amount);
              }
            },
            _ => (),
          }
        }
      },
      _ => ()
    }

    match self.typ {
      CellType::Granular(id, settled_45, settled_90) => {
        let typ = grid.granular[&id];
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
          grid[pos].typ = CellType::Empty;
          grid[new_pos].typ = CellType::Granular(id, rng.gen::<f64>() < typ.granularity_45, rng.gen::<f64>() < typ.granularity_90);
          if typ.fall_speed <= 1.0 || rng.gen::<f64>() < 2.0-typ.fall_speed {
            grid.update(new_pos);
          }
        }
      },
      CellType::WaterGenerator => {
        if can_move_down {
          grid[pos+down].typ = CellType::Fluid(FluidType::Water, 1.0);
        }
      },
      CellType::SandGenerator => {
        if can_move_down {
          grid[pos+down].typ = CellType::Granular(GranularType::Sand, false, false);
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
      CellType::Destroyer => {
        if grid.in_range(pos+up) {
          grid[pos+up].typ = CellType::Empty;
        }
      },
      CellType::Empty => {
        let mut neighbors = 0;
        let mut some_life = false;
        let mut some_nonlife = false;
        for neighbor in grid.moore(pos) {
          if neighbor.typ != CellType::LifeTurningOn && neighbor.typ != CellType::Empty {
            if neighbor.typ == CellType::LifeOn {
              some_life = true;
            } else {
              some_nonlife = true;
            }
            neighbors += 1;
          }
        }
        if (some_life && (neighbors == 3 || neighbors == 5 || neighbors == 6)) || (some_life && some_nonlife && neighbors >= 4 && neighbors <= 4) {
          grid[pos].typ = CellType::LifeTurningOn;
        }
      }
      CellType::LifeOn => {
        let mut neighbors = 0;
        let mut some_life = false;
        let mut some_nonlife = false;
        for neighbor in grid.moore(pos) {
          if neighbor.typ != CellType::LifeTurningOn && neighbor.typ != CellType::Empty {
            if neighbor.typ == CellType::LifeOn {
              some_life = true;
            } else {
              some_nonlife = true;
            }
            neighbors += 1;
          }
        }
        if (!some_life || (neighbors != 2 && neighbors != 3)) && (!some_life || !some_nonlife || neighbors < 4 || neighbors > 4) {
          grid[pos].typ = CellType::Empty;
        }
      }
      CellType::Wire(wtype) => {
        let mut neighbors = 0;
        let mut set_nor = false;
        for neighbor in grid.moore(pos) {
          if let CellType::ElectronHead(nbr_wtype) = neighbor.typ {
            if nbr_wtype.can_signal_to(wtype) {
              neighbors += 1;
            }
          }
          if let CellType::Wire(WireType::Nor(false)) = self.typ {
            if let CellType::Wire(WireType::Nor(true)) = neighbor.typ {
              neighbors += 1;
              set_nor = true;
            }
            if let CellType::ElectronHead(WireType::Normal) = neighbor.typ {
              set_nor = true;
            }
          }
        }
        if neighbors > 0 {
          if set_nor {
            grid[pos].typ = CellType::Wire(WireType::Nor(true));
          } else {
            grid[pos].typ = CellType::ElectronHead(wtype);
          }
        }
        if wtype == WireType::SignalEmitter {
          grid[pos].typ = CellType::ElectronHead(WireType::SignalEmitter);
        }
      }
      CellType::ElectronHead(base) => {
        grid[pos].typ = CellType::ElectronTail(base, tail_lifetime);
      }
      CellType::ElectronTail(base, lifetime) => {
        let lifetime = lifetime-1;
        if lifetime == 0 {
          grid[pos].typ = CellType::Wire(base.transition_to());
        } else {
          grid[pos].typ = CellType::ElectronTail(base, lifetime);
        }
      }
      CellType::LifeTurningOn => {
        grid[pos].typ = CellType::LifeOn;
      }
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
            CellType::Fluid(FluidType::Water, amount) /*if amount >= 1.0*/ => grid[neighbor].typ = CellType::Plant,
            _ => (),
          }
        }
      },
      CellType::Eater => {
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
            CellType::Empty => if rng.gen::<f64>() < 0.25 {
              grid[pos].typ = CellType::Empty;
            },
            _ => grid[neighbor].typ = CellType::Eater,
          }
        }
      },
      CellType::Fuse(true) => {
        grid[pos].typ = CellType::Fire;
      },
      CellType::Fuse(false) => {
        for neighbor in grid.moore(pos) {
          if neighbor.typ == CellType::Fuse(true) || neighbor.typ == CellType::Fire {
            grid[pos].typ = CellType::Fuse(true);
            break;
          }
        }
      },
      CellType::Virus(lifetime) => {
        if rng.gen::<f64>() < 0.1 && can_move_up && lifetime >= 2 {
          grid[pos+up].typ = CellType::Virus(virus_lifetime);
          return;
        }
        if rng.gen::<f64>() < 0.1 && can_move_left && lifetime >= 2 {
          grid[pos+left].typ = CellType::Virus(virus_lifetime);
          return;
        }
        if rng.gen::<f64>() < 0.1 && can_move_right && lifetime >= 2 {
          grid[pos+right].typ = CellType::Virus(virus_lifetime);
          return;
        }
        if rng.gen::<f64>() < 0.1 && can_move_down && lifetime >= 2 {
          grid[pos+down].typ = CellType::Virus(virus_lifetime);
          return;
        }
        if lifetime <= 0 {
          grid[pos].typ = CellType::Empty;
        } else {
          grid[pos].typ = CellType::Virus(lifetime-1);
        }
      },
      CellType::Fire => {
        if rng.gen::<f64>() < 0.18 && can_move_up {
          grid[pos+up].typ = CellType::Fire;
          grid[pos].typ = CellType::Empty;
          return;
        }
        if rng.gen::<f64>() < 0.05 && can_move_left {
          grid[pos+left].typ = CellType::Fire;
          grid[pos].typ = CellType::Empty;
          return;
        }
        if rng.gen::<f64>() < 0.05 && can_move_right {
          grid[pos+right].typ = CellType::Fire;
          grid[pos].typ = CellType::Empty;
          return;
        }
        if rng.gen::<f64>() < 0.03 && can_move_down {
          grid[pos+down].typ = CellType::Fire;
          grid[pos].typ = CellType::Empty;
          return;
        }
        if rng.gen::<f64>() < 0.03 {
          grid[pos].typ = CellType::Empty;
        }
        for _ in 0..3 {
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
              CellType::Fluid(FluidType::Oil, amount) => grid[neighbor].typ = CellType::Fire,
              CellType::Fluid(FluidType::Methane, amount) => grid[neighbor].typ = CellType::Fire,
              CellType::Fluid(FluidType::Water, amount) => grid[neighbor].typ = CellType::Fluid(FluidType::Steam, amount),
              CellType::Solid(SolidType::Ice) => grid[neighbor].typ = CellType::Fluid(FluidType::Steam, 1.0),
              _ => (),
            }
          }
        }
      },
      CellType::Fluid(id, mut amount) => {
        let typ = grid.fluid[&id];
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
              let other_typ = grid.fluid[&id2];
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
            grid[pos].typ = CellType::Fluid(id, amount*0.25);
            grid[pos+left].typ = CellType::Fluid(id, amount*0.75);
            if typ.fall_speed <= 1.0 || rng.gen::<f64>() < 2.0-typ.fall_speed {
              grid.update(pos+left);
            }
          } else if can_move_right && !can_move_left {
            grid[pos].typ = CellType::Fluid(id, amount*0.25);
            grid[pos+right].typ = CellType::Fluid(id, amount*0.75);
            if typ.fall_speed <= 1.0 || rng.gen::<f64>() < 2.0-typ.fall_speed {
              grid.update(pos+right);
            }
          } else if rng.gen::<f64>() < 0.5 {
            grid[pos].typ = CellType::Fluid(id, amount*0.25);
            grid[pos+left].typ = CellType::Fluid(id, amount*0.75);
            if typ.fall_speed <= 1.0 || rng.gen::<f64>() < 2.0-typ.fall_speed {
              grid.update(pos+left);
            }
          } else {
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
              if amount - amount_in_bottom > min_fluid {
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
  mesh: glium::VertexBuffer<UnlitVertex>,//NewMesh<UnlitProgram>,
  texture: texture::Texture2d,//Texture,
  coords: Vec<Vec2<i32>>,
  pub grid: Grid,
  // pixels: Vec<Vec<(u8,u8,u8)>>, //Vec<u8>,
  pixels: Vec<(u8,u8,u8)>,
  unlit_program: glium::Program,
}

#[derive(Copy, Clone)]
pub struct UnlitVertex {
  pub pos: Vec2<f32>,
  pub texcoord: Vec2<f32>,
}
implement_vertex!(UnlitVertex, pos, texcoord);

impl World {
  pub fn new<R: Rng>(size: Vec2<i32>, window: &Window, rng: &mut R) -> World {
    let mut cells = Vec::new();
    let mut updated = Vec::new();
    for y in 0..size.y {
      let row = repeat(Cell{typ: CellType::Empty}).take(size.x as usize).collect();
      let updated_row = repeat(false).take(size.x as usize).collect();
      cells.push(row);
      updated.push(updated_row);
    }
    let mesh = glium::VertexBuffer::new(window, &vec![
      UnlitVertex{pos: Vec2(0.0, 0.0),
        texcoord: Vec2(0.0, 0.0)},
      UnlitVertex{pos: Vec2(size.x as f32, 0.0),
        texcoord: Vec2(1.0, 0.0)},
      UnlitVertex{pos: Vec2(size.x as f32, size.y as f32),
        texcoord: Vec2(1.0, 1.0)},
      UnlitVertex{pos: Vec2(0.0, size.y as f32),
        texcoord: Vec2(0.0, 1.0)},
    ]).unwrap();

    let unlit_program = glium::Program::from_source(window,
      include_str!("../unlit_vert_shader.glsl"),
      include_str!("../unlit_frag_shader.glsl"),
      None
    ).unwrap();

    /*let mut mesh = NewMesh::new(window.unlit_program.clone(), Primitive::Triangles, MeshUsage::DynamicDraw);
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
    mesh.add_triangle(2, 3, 0);*/

    /*let texture = Texture::texture2d_empty(size.x*cell_size, size.y*cell_size, gl::RGB8,
      MinNearest, MagNearest);*/
    let texture = texture::Texture2d::empty_with_format(window, texture::UncompressedFloatFormat::U8U8U8, MipmapsOption::NoMipmap, (size.x*cell_size) as u32, (size.y*cell_size) as u32).unwrap();

    let pixels = Vec::new();//Vec::with_capacity((size.x*size.y*3) as usize);

    // We use a pre-shuffled list of coordinates to get rid of some poblems
    // in the simulation. Without it, some materials would prefer to move
    // to the left and others would prefer to move to the right.
    // Ideally we'd shuffle it each frame, but that's way too expensive.
    // This is close enough.
    let mut coords = Vec::new();
    for y in 0..size.y {
      for x in 0..size.x {
        coords.push(Vec2(x,y));
      }
    }
    rng.shuffle(&mut coords);

    let solid = vec![
      Solid{
        typ: SolidType::Wall,
        name: "wall",
        color: Color3::rgb(0.5, 0.5, 0.5),
      },
      Solid{
        typ: SolidType::Ice,
        name: "ice",
        color: Color3::white().blend(background_color(), 0.65),
      },
    ];
    // TODO: move these to a config file
    let granular = vec![
      Granular{
        typ: GranularType::Sand,
        name: "sand",
        granularity_45: 0.1,
        granularity_90: 0.0,
        horizontal_spread: 0.05,
        spread_speed: 0.8,
        fall_speed: 1.0,
        color: Color3::yellow()*0.9,
      },
      Granular{
        typ: GranularType::Dirt,
        name: "dirt",
        granularity_45: 0.4,
        granularity_90: 0.0,
        horizontal_spread: 0.05,
        spread_speed: 0.8,
        fall_speed: 1.0,
        color: Color3::rgb(0.3, 0.13, 0.0),
      },
      Granular{
        typ: GranularType::Snow,
        name: "snow",
        granularity_45: 1.0,
        granularity_90: 0.3,
        horizontal_spread: 0.05,
        spread_speed: 0.8,
        fall_speed: 1.0,
        color: Color3::rgb(1.0, 1.0, 1.0),
      },
      Granular{
        typ: GranularType::Nitro,
        name: "nitro",
        granularity_45: 0.2,
        granularity_90: 0.0,
        horizontal_spread: 0.05,
        spread_speed: 0.8,
        fall_speed: 1.0,
        color: Color3::rgb(0.1, 0.4, 0.05),
      },
    ];
    let fluid = vec![
      Fluid{
        typ: FluidType::Water,
        name: "water",
        horizontal_spread: 0.05,
        fall_speed: 1.0,
        compressibility: 0.05,
        color: Color3::rgb(0.0, 0.2, 1.0),
        density: 1.0,
        down_dir: down_,
        up_dir: up_,
      },
      Fluid{
        typ: FluidType::Oil,
        name: "oil",
        horizontal_spread: 0.05,
        fall_speed: 1.0,
        compressibility: 0.05,
        color: Color3::rgb(0.5, 0.3, 0.0),
        density: 0.9,
        down_dir: down_,
        up_dir: up_,
      },
      Fluid{
        typ: FluidType::Methane,
        name: "methane",
        horizontal_spread: 0.05,
        fall_speed: 1.0,
        compressibility: 0.05,
        color: Color3::rgb(0.15, 0.1, 0.1).blend(background_color(), 0.8),
        density: 0.5,
        down_dir: up_,
        up_dir: down_,
      },
      Fluid{
        typ: FluidType::Steam,
        name: "steam",
        horizontal_spread: 0.05,
        fall_speed: 1.0,
        compressibility: 0.05,
        color: Color3::rgb(0.0, 0.2, 1.0).blend(Color3::white(), 0.6).blend(background_color(), 0.7),
        density: 0.3,
        down_dir: up_,
        up_dir: down_,
      },
      Fluid{
        typ: FluidType::Cement,
        name: "cement",
        horizontal_spread: 0.01,
        fall_speed: 0.5, // TODO: this doesn't seem to do anything
        compressibility: 0.01,
        color: Color3::rgb(0.3, 0.3, 0.3),
        density: 1.5,
        down_dir: down_,
        up_dir: up_,
      },
    ];
    let solid: HashMap<SolidType, Solid> = solid.into_iter().map(|x| (x.typ, x)).collect();
    let granular: HashMap<GranularType, Granular> = granular.into_iter().map(|x| (x.typ, x)).collect();
    let fluid: HashMap<FluidType, Fluid> = fluid.into_iter().map(|x| (x.typ, x)).collect();

    let grid = Grid{cells: cells, updated: updated, size: size, solid: solid, granular: granular, fluid: fluid};
    World{grid: grid, mesh: mesh, texture: texture, coords: coords, pixels: pixels, id: Id::new(), unlit_program: unlit_program}
  }

  pub fn simulate<R: Rng>(&mut self, rng: &mut R) {
    for y in 0..self.grid.size.y {
      for x in 0..self.grid.size.x {
        match self.grid.cells[y as usize][x as usize].typ {
          CellType::Fluid(id, amount) if amount <= min_fluid => {
            assert!(amount >= 0.0);
            self.grid.cells[y as usize][x as usize].typ = CellType::Empty;
          },
          _ => ()
        }

        self.grid.updated[y as usize][x as usize] = false;
      }
    }

    for &coord in self.coords.iter() {
      self.grid.cells[coord.y as usize][coord.x as usize].simulate(&mut self.grid, coord, rng);
    }
  }

  pub fn update_mesh(&mut self, window: &Window) {
    // TODO: with this here, why not just alloc pixels here?
    self.pixels.clear();

    let mut pixels = Vec::new();
    for y in 0..self.grid.size.y as usize {
      let mut row = Vec::new();
      for x in 0..self.grid.size.x as usize {
        let color = self.grid.cells[y][x].color(&self.grid);
        row.push((color.r*255.0) as u8);
        row.push((color.g*255.0) as u8);
        row.push((color.b*255.0) as u8);
      }
      pixels.push(row);
    }
    for y in 0..self.grid.size.y as usize {
      let ref row = pixels[y];
      for _ in 0..cell_size {
        // let mut out_row = Vec::new();
        for x in 0..self.grid.size.x as usize {
          let r = row[x*3+0];
          let g = row[x*3+1];
          let b = row[x*3+2];
          for _ in 0..cell_size {
            /*self.pixels.push(r);
            self.pixels.push(g);
            self.pixels.push(b);*/
            // out_row.push((r,g,b));
            self.pixels.push((r,g,b));
          }
        }
        // self.pixels.push(out_row);
      }
    }

    // TODO: can I avoid the clone?
    let raw = RawImage2d{
      data: Cow::Borrowed(&self.pixels),
      width: (self.grid.size.x*cell_size) as u32,
      height: (self.grid.size.y*cell_size) as u32,
      format: ClientFormat::U8U8U8
    };
    self.texture.write(glium::Rect{left: 0, width: (self.grid.size.x*cell_size) as u32,
      bottom: 0, height: (self.grid.size.y*cell_size) as u32}, raw);
    // self.texture = texture::Texture2d::with_mipmaps(window, raw, false);

    // self.texture.update_texture2d_from_pixels(&self.pixels);
  }
}

impl Widget for World {
  fn id(&self) -> &Id {&self.id}
  fn draw(&mut self, pos: Vec2<i32>, size: Vec2<i32>, facade: &DrawContext, frame: &mut DrawFrame) {//window: &mut Window) {

    let indices = index::NoIndices(index::PrimitiveType::TriangleFan);
    let uniforms = uniform!{
      modelViewMatrix: Mat4::generic_ortho(
      Vec2::zero(), Vec2(self.grid.size.x as f32, self.grid.size.y as f32),
      Vec2::<f32>::gen_from(pos), Vec2::<f32>::gen_from(pos+size)),
      projMatrix: Mat4::ortho_flip(frame.width() as f32, frame.height() as f32), // TODO //window.window_size.x as f32, window.window_size.y as f32),
      tex: Sampler::new(&self.texture), // TODO: filters
    };
    // println!("Drawing world; {} {}", frame.width(), frame.height());
    frame.draw(&self.mesh, &indices, &self.unlit_program, &uniforms, &default_draw_params/*, None*/);

    /*self.mesh.draw(UnlitUniforms{
      model_view_matrix: Mat4::generic_ortho(
      Vec2::zero(), Vec2(self.grid.size.x as f32, self.grid.size.y as f32),
      Vec2::<f32>::gen_from(pos), Vec2::<f32>::gen_from(pos+size)),
      proj_matrix: Mat4::ortho_flip(window.window_size.x as f32, window.window_size.y as f32),
      tex: &self.texture,
    });*/
  }

  fn min_size(&self, facade: &DrawContext) -> Vec2<i32> {
    self.grid.size * cell_size
  }
}



pub struct Grid {
  pub size: Vec2<i32>,
  cells: Vec<Vec<Cell>>,
  updated: Vec<Vec<bool>>,
  solid: HashMap<SolidType, Solid>,
  granular: HashMap<GranularType, Granular>,
  fluid: HashMap<FluidType, Fluid>,
}

impl Grid {
  pub fn updated(&self, pos: Vec2<i32>) -> bool {
    self.updated[pos.y as usize][pos.x as usize]
  }
  pub fn update(&mut self, pos: Vec2<i32>) {
    self.updated[pos.y as usize][pos.x as usize] = true;
  }

  pub fn in_range(&self, pos: Vec2<i32>) -> bool {
    pos.x >= 0 && pos.y >= 0 && pos.x < self.size.x && pos.y < self.size.y
  }

  pub fn moore(&self, pos: Vec2<i32>) -> Vec<Cell> {
    let mut res = Vec::new();
    for x in (pos.x-1)..(pos.x+2) {
      for y in (pos.y-1)..(pos.y+2) {
        if (x != pos.x || y != pos.y) && self.in_range(Vec2(x, y)) {
          res.push(self[Vec2(x, y)]);
        }
      }
    }
    res
  }
}

impl Index<Vec2<i32>> for Grid {
  type Output = Cell;
  fn index(&self, index: Vec2<i32>) -> &Cell {
    &self.cells[index.y as usize][index.x as usize]
  }
}

impl IndexMut<Vec2<i32>> for Grid {
  fn index_mut(&mut self, index: Vec2<i32>) -> &mut Cell {
    &mut self.cells[index.y as usize][index.x as usize]
  }
}
