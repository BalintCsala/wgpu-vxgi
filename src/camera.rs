use cgmath::{Vector3, Euler, Deg, Vector2, Zero, Matrix4, num_traits::{ToPrimitive, clamp}, SquareMatrix, Vector4, Point3};
use winit::{window::Window, event::{WindowEvent, MouseButton, ElementState, KeyboardInput, VirtualKeyCode}};


#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);

pub struct ShadowCamera {
    pub position: Point3<f32>,
    pub direction: Vector3<f32>,
    near: f32,
    far: f32,
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
}

impl ShadowCamera {
    
    pub fn new(
        position: Point3<f32>,
        direction: Vector3<f32>,
        near: f32,
        far: f32,
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
    ) -> Self {
        Self {
            position,
            direction,
            near,
            far,
            left,
            right,
            bottom,
            top,
        }
    }

    pub fn proj_mat(&self) -> Matrix4<f32> {
        cgmath::ortho(self.left, self.right, self.bottom, self.top, self.near, self.far)
    }

    pub fn view_mat(&self) -> Matrix4<f32> {
        Matrix4::look_to_rh(self.position, self.direction, Vector3 { x: 0.0, y: 1.0, z: 0.0 })
    }

    pub fn get_uniform_data(&self) -> [[f32; 4]; 4] {
        return (OPENGL_TO_WGPU_MATRIX * self.proj_mat() * self.view_mat()).into();
    }

}

pub struct PerspectiveCamera {
    pub position: Vector3<f32>,
    pub rotation: Euler<Deg<f32>>,
    near: f32,
    far: f32,
    fov: Deg<f32>,
    aspect_ratio: f32,
    movement: Vector3<f32>,
    dragging: bool,
    last_cursor: Vector2<f32>,
    speed: f32,
}

impl PerspectiveCamera {
    pub fn new(
        window: &Window,
        position: Vector3<f32>,
        rotation: Euler<Deg<f32>>,
        near: f32,
        far: f32,
        fov: Deg<f32>,
    ) -> Self {
        let size = window.inner_size();
        Self {
            position,
            rotation,
            near,
            far,
            fov,
            aspect_ratio: (size.width as f32) / (size.height as f32),
            movement: Vector3::zero(),
            dragging: false,
            last_cursor: Vector2::zero(),
            speed: 1.0,
        }
    }

    pub fn proj_mat(&self) -> Matrix4<f32> {
        cgmath::perspective(self.fov, self.aspect_ratio, self.near, self.far)
    }

    pub fn view_mat(&self) -> Matrix4<f32> {
        Matrix4::from(self.rotation) * Matrix4::from_translation(-self.position)
    }

    pub fn get_uniform_data(&self) -> [[f32; 4]; 4] {
        return (OPENGL_TO_WGPU_MATRIX * self.proj_mat() * self.view_mat()).into();
    }

    pub fn process_event(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::MouseWheel { delta, .. } => {
                let delta_value = match delta {
                    winit::event::MouseScrollDelta::LineDelta(delta, _) => delta / 50.0,
                    winit::event::MouseScrollDelta::PixelDelta(delta) => {
                        delta.y.to_f32().unwrap() / 200.0
                    }
                };
                self.speed = clamp(self.speed + delta_value, 0.0, 5.0);
                false
            }
            WindowEvent::MouseInput { button, state, .. } => {
                if button != &MouseButton::Left {
                    return false;
                }
                match state {
                    ElementState::Pressed => self.dragging = true,
                    ElementState::Released => self.dragging = false,
                }
                false
            }
            WindowEvent::CursorMoved { position, .. } => {
                let pos =
                    Vector2::<f32>::new(position.x.to_f32().unwrap(), position.y.to_f32().unwrap());
                let diff = pos - self.last_cursor;
                if self.dragging {
                    self.rotation.x += Deg(diff.y / 3.0);
                    self.rotation.y += Deg(diff.x / 3.0);
                    self.rotation.x = Deg(clamp(self.rotation.x.0, -90.0, 90.0));
                }
                self.last_cursor = pos;
                false
            }
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state,
                        virtual_keycode: Some(keycode),
                        ..
                    },
                ..
            } => {
                let speed = if *state == ElementState::Pressed {
                    self.speed
                } else {
                    0.0
                };
                match keycode {
                    VirtualKeyCode::A => {
                        self.movement.x = -speed;
                        true
                    }
                    VirtualKeyCode::D => {
                        self.movement.x = speed;
                        true
                    }
                    VirtualKeyCode::W => {
                        self.movement.z = -speed;
                        true
                    }
                    VirtualKeyCode::S => {
                        self.movement.z = speed;
                        true
                    }
                    VirtualKeyCode::LShift => {
                        self.movement.y = -speed;
                        true
                    }
                    VirtualKeyCode::Space => {
                        self.movement.y = speed;
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    pub fn update(&mut self) {
        self.position += (self.view_mat().invert().unwrap()
            * Vector4::new(self.movement.x, self.movement.y, self.movement.z, 0.0))
        .xyz()
            * 0.016;
    }
}