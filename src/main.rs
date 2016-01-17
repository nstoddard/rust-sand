/* TODO
  Bugs:
    When placing cells near the bottom of the screen, the cells are a few units lower than they should be.
      This happens because the window is larger than expected - the large number of interface buttons expands the window beyond the world widget's minimum size, so the widget scales but mouse clicks don't.
    When holding the mouse, moving over to the GUI elements on the right, and releasing the mouse, cells are still placed
*/

#![allow(dead_code, unused_imports, non_upper_case_globals, unused_unsafe, unused_variables, unused_mut)]


extern crate glfw;
extern crate freetype;
extern crate image;
extern crate rand;
extern crate num;

extern crate vecmat;
extern crate timer;
#[macro_use] extern crate glium;
extern crate gui;

use num::*;

// use std::iter::*;
use rand::Rng;
use std::cmp;
use std::path::Path;

use timer::*;
use vecmat::*;
// use vecmat::num_ext::*;

use gui::color::*;
use gui::util::*;
use gui::widgets::*;
use gui::window::*;
use gui::event::*;
use gui::text::*;

mod world;

use world::*;

// #[cfg(windows)] #[link_args = "-Wl,--subsystem,windows"] extern {}

const fps: i32 = 60;
const dt: f64 = 1.0 / fps as f64;


fn main() {
  let cell_types = vec![CellType::Empty,
    CellType::Solid(SolidType::Wall),
    CellType::Solid(SolidType::Ice),
    CellType::Granular(GranularType::Sand, false, false),
    CellType::Granular(GranularType::Dirt, false, false),
    CellType::Granular(GranularType::Snow, false, false),
    CellType::Granular(GranularType::Nitro, false, false),
    CellType::Fluid(FluidType::Water, 1.0),
    CellType::Fluid(FluidType::Oil, 1.0),
    CellType::Fluid(FluidType::Methane, 1.0),
    CellType::Fluid(FluidType::Steam, 1.0),
    CellType::Fluid(FluidType::Cement, 1.0),
    CellType::WaterGenerator,
    CellType::SandGenerator,
    CellType::Destroyer,
    CellType::Plant,
    CellType::Fire,
    CellType::Torch,
    CellType::LifeOn,
    CellType::Wire(WireType::Normal),
    CellType::Wire(WireType::Input),
    CellType::Wire(WireType::Output),
    CellType::Wire(WireType::Nor(false)),
    CellType::Wire(WireType::SignalEmitter),
    CellType::ElectronHead(WireType::Normal),
    CellType::Eater,
    CellType::Fuse(false),
    CellType::Virus(virus_lifetime),
  ];

  let world_size = Vec2(1200/cell_size, 750/cell_size);
  // println!("{}", world_size);

  let mut rng = rand::thread_rng();

  // let mut glfw = init_glfw();
  let window_mode = WindowMode::Windowed{title: "Falling sand game".to_string(), min_size: world_size * cell_size};
  let resource_path = Path::new("resources");
  let mut window = Window::new(window_mode/*, &resource_path*/);
  let font = Font::new(&(resource_path.join("DejaVuSans.ttf")), 14, &window);

  // let mut fps_logger = FPSLogger::new(1.0);

  let mut quit_button = Button::new(font.clone(), "Quit");
  let mut pause_button = Button::new(font.clone(), "Pause");
  let mut step_button = Button::new(font.clone(), "Step");

  let mut shape_buttons = vec![
    (Button::new(font.clone(), "Circle"), Brush::Circle),
    (Button::new(font.clone(), "Square"), Brush::Square),
    (Button::new(font.clone(), "Diamond"), Brush::Diamond),
    (Button::new(font.clone(), "Random"), Brush::Random),
  ];

  let mut size_buttons = vec![
    (Button::new(font.clone(), "Size 1"), 1),
    (Button::new(font.clone(), "Size 2"), 2),
    (Button::new(font.clone(), "Size 5"), 5),
    (Button::new(font.clone(), "Size 10"), 10),
    (Button::new(font.clone(), "Size 20"), 20),
    (Button::new(font.clone(), "Size 50"), 50),
  ];

  // TODO: get rid of this hack
  let mut gap0 = EmptyWidget::new(Vec2::zero());
  let mut gap1 = EmptyWidget::new(Vec2::zero());
  let mut gap2 = EmptyWidget::new(Vec2::zero());
  let mut gap3 = EmptyWidget::new(Vec2::zero());
  let mut gap4 = EmptyWidget::new(Vec2::zero());

  let mut timer = Timer::new();

  let mut world = World::new(world_size, &window, &mut rng);

  let mut paused = false;

  let mut cell_type_widgets = Vec::new();
  for typ in cell_types.iter() {
    cell_type_widgets.push(Button::new(font.clone(), typ.name(&world.grid)));
  }

  let mut cur_cell_type_index = 3;
  let mut cur_cell_type = cell_types[cur_cell_type_index];

  let mut brush = Brush::Circle;
  let mut brush_size = 10;

  let mut old_mouse_pos = None;

  while !window.should_close() {
    // fps_logger.update();
    // check_gl_error("game loop");


    if !paused {
      world.simulate(&mut rng);
    }
    world.update_mesh(&window);

    {
      let mut controls = vec![
        (LWidget(&mut quit_button), 0.0),
        (LWidget(&mut gap4), 1.0),
        (LWidget(&mut pause_button), 0.0),
        (LWidget(&mut step_button), 0.0),
        (LWidget(&mut gap0), 1.0),
      ];
      for widget in cell_type_widgets.iter_mut() {
        controls.push((LWidget(widget), 0.0));
      }
      controls.push((LWidget(&mut gap1), 1.0));
      for &mut (ref mut button, _) in &mut shape_buttons {
        controls.push((LWidget(button), 0.0));
      }
      controls.push((LWidget(&mut gap2), 1.0));
      for &mut (ref mut button, _) in &mut size_buttons {
        controls.push((LWidget(button), 0.0));
      }
      controls.push((LWidget(&mut gap3), 1.0));

      window.draw_gui(
        Row(Leading, 0, vec![
          (LWidget(&mut world), 0.0),
          (Col(Leading, 0, controls), 1.0),
        ]),
        /*&mut glfw, */Color3::white());
    }

    for (i, widget) in cell_type_widgets.iter_mut().enumerate() {
      if widget.was_pressed() {
        cur_cell_type_index = i;
        cur_cell_type = cell_types[cur_cell_type_index];
      }
      if cur_cell_type_index == i {
        widget.set_text_color(Color4::red());
      } else {
        widget.set_text_color(Color4::black());
      }
    }

    if quit_button.was_pressed() {
      // TODO!
      window.glfw_window().set_should_close(true);
    }
    if pause_button.was_pressed() {
      paused = !paused;
      if paused {
        pause_button.set_text("Unpause");
      } else {
        pause_button.set_text("Pause");
      }
    }
    if step_button.was_pressed() {
      paused = true;
      pause_button.set_text("Unpause");
      world.simulate(&mut rng);
    }

    for &mut (ref mut button, shape) in &mut shape_buttons {
      if button.was_pressed() {
        brush = shape;
      }
      if brush == shape {
        button.set_text_color(Color4::red());
      } else {
        button.set_text_color(Color4::black());
      }
    }
    for &mut (ref mut button, size) in &mut size_buttons {
      if button.was_pressed() {
        brush_size = size;
      }
      if brush_size == size {
        button.set_text_color(Color4::red());
      } else {
        button.set_text_color(Color4::black());
      }
    }

    for event in window.get_events().into_iter() {
      match event {
        Event::Key(key, _, Action::Press, _) => {
          match key {
            //TODO!!
            glfw::Key::Escape => window.glfw_window().set_should_close(true),
            glfw::Key::P => {
              paused = !paused;
              if paused {
                pause_button.set_text("Unpause");
              } else {
                pause_button.set_text("Pause");
              }
            },
            glfw::Key::Space => {
              paused = true;
              pause_button.set_text("Unpause");
              world.simulate(&mut rng);
            },
            glfw::Key::Num2 => {
              cur_cell_type_index = (cur_cell_type_index+1) % cell_types.len();
              cur_cell_type = cell_types[cur_cell_type_index];
            },
            glfw::Key::Num1 => {
              if cur_cell_type_index == 0 {
                cur_cell_type_index = cell_types.len()-1;
              } else {
                cur_cell_type_index -= 1;
              }
              cur_cell_type = cell_types[cur_cell_type_index];
            },
            glfw::Key::W => {
              let mut total_water = 0.0;
              for y in 0..world.grid.size.y {
                for x in 0..world.grid.size.x {
                  match world.grid[Vec2(x,y)].typ {
                    CellType::Fluid(FluidType::Water, amount) => total_water += amount,
                    _ => ()
                  }
                }
              }
              println!("Total water: {}", total_water);
            },
            _ => ()
          }
        },
        Event::CursorLeave => {
          old_mouse_pos = None;
        },
        _ => ()
      }
    }
    for event in window.get_widget_events(&world).into_iter() {
      match event {
        // TODO: make this work when holding the mouse button down
        Event::MouseButton(glfw::MouseButton::Button1, Action::Press, _, pos) => {
          let pos = Vec2(pos.x as i32, pos.y as i32);
          let old_mouse_pos2 = match old_mouse_pos {
            None => pos/cell_size,
            Some(pos) => pos
          };
          brush.draw(brush_size, pos/cell_size, old_mouse_pos2, cur_cell_type, &mut world, &mut rng);
          old_mouse_pos = Some(pos/cell_size);
        },
        Event::MouseButton(glfw::MouseButton::Button1, Action::Release, _, pos) => {
          old_mouse_pos = None;
        },
        Event::MouseMove(pos, ref buttons) if buttons.contains(&glfw::MouseButton::Button1) => {
          let pos = Vec2(pos.x as i32, pos.y as i32);
          let old_mouse_pos2 = match old_mouse_pos {
            None => pos/cell_size,
            Some(pos) => pos
          };
          brush.draw(brush_size, pos/cell_size, old_mouse_pos2, cur_cell_type, &mut world, &mut rng);
          old_mouse_pos = Some(pos/cell_size);
        },
        _ => ()
      }
    }

    if old_mouse_pos.is_some() {
      brush.draw(brush_size, old_mouse_pos.unwrap(), old_mouse_pos.unwrap(), cur_cell_type, &mut world, &mut rng);
    }

    // We have to do this instead of glfwSwapInterval b/c that function does busy waiting on some platforms, using 100% of a cpu core for no good reason
    timer.sleep_until(dt);
    timer.add_time(-dt);
  }
}


#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Brush {
  Circle,
  Square,
  Diamond,
  Random,
}


pub fn line(start: Vec2<i32>, end: Vec2<i32>) -> Vec<Vec2<i32>> {
  let mut x0 = start.x;
  let mut y0 = start.y;
  let x1 = end.x;
  let y1 = end.y;
  let dx = (x1-x0).abs();
  let dy = (y1-y0).abs();
  let sx = if x0 < x1 {1} else {-1};
  let sy = if y0 < y1 {1} else {-1};
  let mut err = dx - dy;

  let mut points = Vec::new();
  loop {
    points.push(Vec2(x0,y0));
    if x0 == x1 && y0 == y1 {return points;}
    let e2 = 2*err;
    if e2 > -dy {
      err -= dy;
      x0 += sx;
    }
    if x0 == x1 && y0 == y1 {
      points.push(Vec2(x0,y0));
      return points;
    }
    if e2 < dx {
      err += dx;
      y0 += sy;
    }
  }
}


impl Brush {
  pub fn draw<R: Rng>(self, brush_size: i32, pos_1: Vec2<i32>, pos_2: Vec2<i32>, cell_type: CellType, world: &mut World, rng: &mut R) {
    for pos in line(pos_1, pos_2) {
      for point in self.get_points(brush_size, pos, rng).into_iter() {
        if world.grid.in_range(point) {
          world.grid[point].typ = cell_type;
        }
      }
    }
  }

  // TODO: performance
  pub fn get_points<R: Rng>(self, diameter: i32, center: Vec2<i32>, rng: &mut R) -> Vec<Vec2<i32>> {
    match self {
      // TODO: verify that the radius calculations are right (also below)
      Brush::Circle => {
        let radius = diameter as f64*0.5;
        let radius2 = radius as i32 + 1;
        let mut points = Vec::new();
        for y in range_inclusive(-radius2, radius2) {
          for x in range_inclusive(-radius2, radius2) {
            let x2 = x as f64;
            let y2 = y as f64;
            if x2*x2+y2*y2 <= radius*radius {
              points.push(Vec2(x,y) + center);
            }
          }
        }
        points
      },
      Brush::Square => {
        let radius = diameter as f64*0.5;
        let radius2 = radius as i32 + 1;
        let mut points = Vec::new();
        for y in range_inclusive(-radius2, radius2) {
          for x in range_inclusive(-radius2, radius2) {
            let x2 = x as f64;
            let y2 = y as f64;
            if x2 <= radius && x2 >= -radius && y2 <= radius && y2 >= -radius {
              points.push(Vec2(x,y) + center);
            }
          }
        }
        points
      },
      Brush::Diamond => {
        let radius = diameter as f64*0.5;
        let radius2 = radius as i32 + 1;
        let mut points = Vec::new();
        for y in range_inclusive(-radius2, radius2) {
          for x in range_inclusive(-radius2, radius2) {
            let x2 = x as f64;
            let y2 = y as f64;
            if x2.abs() + y2.abs() <= radius {
              points.push(Vec2(x,y) + center);
            }
          }
        }
        points
      },
      Brush::Random => {
        let radius = diameter as f64*0.5;
        let radius2 = radius as i32 + 1;
        let mut points = Vec::new();
        for y in range_inclusive(-radius2, radius2) {
          for x in range_inclusive(-radius2, radius2) {
            let x2 = x as f64;
            let y2 = y as f64;
            if x2*x2+y2*y2 <= radius*radius && rng.gen::<f64>() < 0.01 {
              points.push(Vec2(x,y) + center);
            }
          }
        }
        points
      },
    }
  }
}
