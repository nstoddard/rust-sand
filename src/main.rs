#![feature(optin_builtin_traits, core, path)]

#![allow(dead_code, unused_imports, non_upper_case_globals, unused_unsafe, unused_variables, unused_mut)]


extern crate glfw;
extern crate freetype;
extern crate image;
extern crate rand;

extern crate vecmat;
extern crate timer;
extern crate gui;

use std::iter::*;
use rand::Rng;
use std::cmp;

use timer::*;
use vecmat::*;
use vecmat::num::*;

use gui::color::*;
use gui::gui::*;
use gui::renderer::*;
use gui::mesh::*;
use gui::gl_program::*;
use gui::texture::*;
use gui::util::*;
use gui::opengl::*;
use gui::widgets::*;

mod world;

use world::*;

const fps: i32 = 60;
const dt: f64 = 1.0 / fps as f64;


fn main() {
  let cell_types = vec![CellType::Empty, CellType::Wall, CellType::Granular(0, false, false), CellType::Granular(1, false, false), CellType::Granular(2, false, false), CellType::Fluid(0, 1.0), CellType::Fluid(1, 1.0), CellType::Fluid(2, 1.0), CellType::WaterGenerator, CellType::SandGenerator, CellType::Sink];

  let world_size = Vec2(1200/cell_size, 750/cell_size);
  println!("{}", world_size);

  let mut rng = rand::thread_rng();

  let mut glfw = init_glfw();
  let window_mode = GUIWindowMode::Windowed{title: "Falling sand game", min_size: world_size * cell_size};
  let resource_path = Path::new("resources");
  let mut window = GUIWindow::new(&mut glfw, window_mode, &resource_path);
  let font = window.load_font(&(resource_path.join("DejaVuSans.ttf")), 16);

  let mut fps_logger = FPSLogger::new(1.0);

  let mut quit_button = ButtonWidget::new(font.clone(), "Quit", Color::black());
  let mut pause_button = ButtonWidget::new(font.clone(), "Pause", Color::black());
  let mut step_button = ButtonWidget::new(font.clone(), "Step", Color::black());

  let mut circle_button = ButtonWidget::new(font.clone(), "Circle", Color::black());
  let mut square_button = ButtonWidget::new(font.clone(), "Square", Color::black());
  let mut size_1_button = ButtonWidget::new(font.clone(), "Size 1", Color::black());
  let mut size_2_button = ButtonWidget::new(font.clone(), "Size 2", Color::black());
  let mut size_5_button = ButtonWidget::new(font.clone(), "Size 5", Color::black());
  let mut size_10_button = ButtonWidget::new(font.clone(), "Size 10", Color::black());
  let mut size_20_button = ButtonWidget::new(font.clone(), "Size 20", Color::black());

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
    cell_type_widgets.push(ButtonWidget::new(font.clone(), typ.name(&world.grid), Color::black()));
  }

  let mut cur_cell_type_index = 2;
  let mut cur_cell_type = cell_types[cur_cell_type_index];

  let mut brush = Brush::Circle;
  let mut brush_size = 10;

  while !window.glfw_window.should_close() {
    fps_logger.update();
    check_gl_error("game loop");



    if !paused {
      world.simulate(&mut rng);
    }
    world.update_mesh();

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
      controls.extend(vec![
        (LWidget(&mut gap1), 1.0),
        (LWidget(&mut circle_button), 0.0),
        (LWidget(&mut square_button), 0.0),
        (LWidget(&mut gap2), 1.0),
        (LWidget(&mut size_1_button), 0.0),
        (LWidget(&mut size_2_button), 0.0),
        (LWidget(&mut size_5_button), 0.0),
        (LWidget(&mut size_10_button), 0.0),
        (LWidget(&mut size_20_button), 0.0),
        (LWidget(&mut gap3), 1.0),
      ].into_iter());

      window.draw_gui(
        HPanel(Leading, vec![
          (LWidget(&mut world), 0.0),
          (VPanel(Leading, controls), 1.0),
        ]),
        &mut glfw, Color::white());
    }

    for (i, widget) in cell_type_widgets.iter_mut().enumerate() {
      if widget.was_pressed() {
        cur_cell_type_index = i;
        cur_cell_type = cell_types[cur_cell_type_index];
      }
    }

    if quit_button.was_pressed() {
      window.glfw_window.set_should_close(true);
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

    if circle_button.was_pressed() {
      brush = Brush::Circle;
    }
    if square_button.was_pressed() {
      brush = Brush::Square;
    }
    if size_1_button.was_pressed() {
      brush_size = 1;
    }
    if size_2_button.was_pressed() {
      brush_size = 2;
    }
    if size_5_button.was_pressed() {
      brush_size = 5;
    }
    if size_10_button.was_pressed() {
      brush_size = 10;
    }
    if size_20_button.was_pressed() {
      brush_size = 20;
    }

    for event in window.get_events().into_iter() {
      match event {
        Event::Key(key, _, Action::Press, _) => {
          match key {
            glfw::Key::Escape => window.glfw_window.set_should_close(true),
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
              for y in range(0, world.grid.size.y) {
                for x in range(0, world.grid.size.x) {
                  match world.grid[Vec2(x,y)].typ {
                    CellType::Fluid(0, amount) => total_water += amount,
                    _ => ()
                  }
                }
              }
              println!("Total water: {}", total_water);
            },
            _ => ()
          }
        },
        _ => ()
      }
    }
    for event in window.get_widget_events(&world).into_iter() {
      match event {
        // TODO: make this work when holding the mouse button down
        Event::MouseButton(glfw::MouseButton::Button1, Action::Press, _, pos) => {
          let pos = pos/cell_size;
          for point in brush.get_points(brush_size, pos).into_iter() {
            if world.grid.in_range(point) {
              world.grid[point].typ = cur_cell_type;
            }
          }
        },
        Event::MouseMove(pos, ref buttons) if buttons.contains(&glfw::MouseButton::Button1) => {
          let pos = pos/cell_size;
          for point in brush.get_points(brush_size, pos).into_iter() {
            if world.grid.in_range(point) {
              world.grid[point].typ = cur_cell_type;
            }
          }
        },
        _ => ()
      }
    }

    // We have to do this instead of glfwSwapInterval b/c that function does busy waiting on some platforms, using 100% of a cpu core for no good reason
    timer.sleep_until(dt);
    timer.add_time(-dt);
  }
}


#[derive(Copy, Clone)]
pub enum Brush {
  Circle,
  Square,
}

impl Brush {
  // TODO: performance
  pub fn get_points(self, diameter: i32, center: Vec2<i32>) -> Vec<Vec2<i32>> {
    match self {
      // TODO: verify that the radius calculations are right
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
      }
    }
  }
}

