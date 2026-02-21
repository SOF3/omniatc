use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
use std::{env, fs, mem, thread};

use anyhow::{Context, Result, anyhow};
use bevy::app::App;
use bevy::asset::RenderAssetUsages;
use bevy::ecs::entity::Entity;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::ResMut;
use bevy::ecs::world::World;
use bevy::image::{CompressedImageFormats, Image, ImageSampler, ImageType};
use bevy::input::keyboard::{self, KeyboardInput};
use bevy::input::mouse::{MouseButton, MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::input::{ButtonInput, ButtonState};
use bevy::math::Vec2;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy::time::{self, Time, TimeUpdateStrategy};
use bevy::window::{
    PrimaryWindow, RawHandleWrapper, RawHandleWrapperHolder, Window, WindowWrapper,
};
use jiff::Timestamp;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window as WinitWindow, WindowId};

const DEFAULT_SCENARIO: &str = "omniatc.tutorial";
const DEFAULT_MAX_FRAMES: usize = 600;

pub struct ClientTest {
    app:             App,
    screenshots_dir: PathBuf,
    test_name:       String,
    max_frames:      usize,
}

pub fn start_test(test_name: impl Into<String>) -> Result<ClientTest> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let assets_dir = manifest_dir.join("..").join("..").join("assets");

    let options = omniatc_client::Options {
        assets_dir:       assets_dir.to_string_lossy().into_owned(),
        headless_test:    true,
        open_level_id:    None,
        default_scenario: DEFAULT_SCENARIO.to_string(),
    };

    let mut app = omniatc_client::main_app(options);
    app.init_resource::<ScreenshotCapture>();
    let test_name = test_name.into();
    let sanitized_test_name = sanitize_label(&test_name, "test")?;

    // Winit is still used to create a synthetic window and raw handles for the renderer.
    let raw_handle = {
        let (sender, receiver) = mpsc::channel::<Result<RawHandleWrapper>>();
        let thread = thread::spawn(move || {
            let mut builder = EventLoop::builder();
            #[cfg(target_os = "linux")]
            {
                use winit::platform::wayland::EventLoopBuilderExtWayland;
                use winit::platform::x11::EventLoopBuilderExtX11;
                EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
                EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
            }
            let event_loop = match builder.build() {
                Ok(event_loop) => event_loop,
                Err(err) => {
                    let _ = sender.send(Err(err.into()));
                    return;
                }
            };
            let mut handler = TestWindowHandler { sender: Some(sender) };
            if let Err(err) = event_loop.run_app(&mut handler)
                && let Some(sender) = handler.sender.take()
            {
                let _ = sender.send(Err(err.into()));
            }
        });
        let raw_handle = receiver.recv().context("Failed to receive window handle")??;
        thread.join().map_err(|_| anyhow!("Failed to join winit window thread"))?;
        raw_handle
    };
    let world = app.world_mut();
    let window_entity = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
        .context("Expected a primary window entity")?;
    if let Some(holder) = world.get::<RawHandleWrapperHolder>(window_entity) {
        *holder.0.lock().map_err(|_| anyhow!("RawHandleWrapperHolder lock"))? =
            Some(raw_handle.clone());
    }
    world.entity_mut(window_entity).insert(raw_handle);

    app.finish();
    app.cleanup();

    let screenshots_dir = if let Ok(env) = env::var("SCREENSHOTS_DIR") {
        PathBuf::from(env)
    } else {
        manifest_dir.join("screenshots")
    }
    .canonicalize()
    .context("canonicalize screenshots path")?;

    Ok(ClientTest {
        app,
        screenshots_dir,
        test_name: sanitized_test_name,
        max_frames: DEFAULT_MAX_FRAMES,
    })
}

impl ClientTest {
    pub fn with_screenshot(
        &mut self,
        name: &str,
        run: impl FnOnce(&mut Self) -> Result<()>,
    ) -> Result<()> {
        let start = Timestamp::now();
        bevy::log::info!("Starting step: {name}");
        run(self)?;
        self.screenshot_test(name)?;
        bevy::log::info!("Finished step: {name} ({:#})", start.duration_until(Timestamp::now()));
        Ok(())
    }

    pub fn with_time_scale<R>(&mut self, speed: f32, then: impl FnOnce(&mut Self) -> R) -> R {
        let mut time = self.app.world_mut().resource_mut::<Time<time::Virtual>>();
        let old_speed = time.relative_speed();
        time.set_relative_speed(speed);

        let old_strategy = mem::replace(
            &mut *self.app.world_mut().resource_mut::<TimeUpdateStrategy>(),
            TimeUpdateStrategy::ManualDuration(Duration::from_millis(50).mul_f32(speed)),
        );

        let result = then(self);

        self.app.world_mut().resource_mut::<Time<time::Virtual>>().set_relative_speed(old_speed);
        *self.app.world_mut().resource_mut::<TimeUpdateStrategy>() = old_strategy;

        result
    }

    pub fn with_max_frames<R>(
        &mut self,
        max_frames: usize,
        then: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let old_max_frames = mem::replace(&mut self.max_frames, max_frames);
        let result = then(self);
        self.max_frames = old_max_frames;
        result
    }

    pub fn drive_until<F>(&mut self, condition: F) -> Result<()>
    where
        F: FnMut(&mut World) -> bool,
    {
        let mut frames = 0usize;
        let mut condition = condition;
        while !condition(self.app.world_mut()) {
            if frames >= self.max_frames {
                anyhow::bail!("Timed out waiting for test condition after {frames} frames");
            }
            self.app.update();
            frames += 1;
        }
        bevy::log::info!("Condition satisfied after {frames} frames");
        Ok(())
    }

    pub fn drive_frames(&mut self, frames: usize) {
        for _ in 0..frames {
            self.app.update();
        }
    }

    pub fn world(&mut self) -> &mut World { self.app.world_mut() }

    /// Capture a screenshot for a named step, creating or comparing against the baseline.
    pub fn screenshot_test(&mut self, step: impl AsRef<str>) -> Result<()> {
        bevy::log::info!("Capturing screenshot for step \"{}\"", step.as_ref());

        let step = step.as_ref();
        let sanitized_step = sanitize_label(step, "step")?;
        let baseline_path = self.screenshot_path(&sanitized_step, "baseline");
        let test_path = self.screenshot_path(&sanitized_step, "actual");
        let diff_path = self.screenshot_path(&sanitized_step, "diff");
        let is_new_baseline = !baseline_path.exists();

        {
            let mut capture = self.app.world_mut().resource_mut::<ScreenshotCapture>();
            capture.image = None;
        }

        let mut entity = self.app.world_mut().spawn(Screenshot::primary_window());
        entity.observe(store_screenshot);
        entity.observe(save_to_disk(test_path.clone()));
        if is_new_baseline {
            entity.observe(save_to_disk(baseline_path.clone()));
        }

        self.drive_until(|world| world.resource::<ScreenshotCapture>().image.is_some())?;

        let captured = self
            .app
            .world_mut()
            .resource_mut::<ScreenshotCapture>()
            .image
            .take()
            .context("Screenshot capture did not produce an image")?;

        if is_new_baseline {
            bevy::log::info!("Generated new screenshot baseline at {}.", baseline_path.display());
        } else {
            let expected = load_image(&baseline_path)?;
            compare_images(expected, captured, &diff_path).with_context(|| {
                format!("{} != {}", baseline_path.display(), test_path.display())
            })?;
        }

        Ok(())
    }

    fn screenshot_path(&self, step: &str, suffix: &str) -> PathBuf {
        self.screenshots_dir.join(format!("{}.{step}.{suffix}.png", self.test_name))
    }

    pub fn drag_mouse(
        &mut self,
        button: MouseButton,
        start: Vec2,
        end: Vec2,
        frames: usize,
    ) -> Result<()> {
        #[expect(
            clippy::cast_precision_loss,
            reason = "screen coordinate precision loss is acceptable with the clamp() call"
        )]
        let delta = (end - start) / frames as f32;
        let mut position = start;
        self.set_cursor_position(position)?;
        self.set_button_state(button, true);
        self.world().write_message(MouseMotion { delta: Vec2::ZERO });
        self.drive_frames(1);

        for _ in 0..frames {
            position += delta;
            position = position.clamp(start.min(end), start.max(end));
            self.set_cursor_position(position)?;
            self.world().write_message(MouseMotion { delta });
            self.drive_frames(1);
        }

        self.set_cursor_position(end)?;
        self.set_button_state(button, false);
        self.world().write_message(MouseMotion { delta: Vec2::ZERO });
        self.drive_frames(1);
        Ok(())
    }

    pub fn set_cursor_position(&mut self, position: Vec2) -> Result<()> {
        let mut query = self.world().query_filtered::<&mut Window, With<PrimaryWindow>>();
        let mut window = query.single_mut(self.world()).context("Expected primary window")?;
        window.set_cursor_position(Some(position));
        Ok(())
    }

    pub fn set_button_state(&mut self, button: MouseButton, down: bool) {
        let mut buttons = self.world().resource_mut::<ButtonInput<MouseButton>>();
        if down {
            buttons.press(button);
        } else {
            buttons.release(button);
        }
    }

    pub fn click_at(&mut self, button: MouseButton, cursor: Vec2) -> Result<()> {
        self.set_cursor_position(cursor)?;
        self.set_button_state(button, true);

        self.drive_frames(1);

        self.set_cursor_position(cursor)?;
        self.set_button_state(button, false);
        self.drive_frames(1);

        Ok(())
    }

    pub fn primary_window_entity(&mut self) -> Result<Entity> {
        let mut query = self.world().query_filtered::<Entity, With<PrimaryWindow>>();
        query.single(self.world()).context("Expected primary window entity")
    }

    pub fn window_center(&mut self) -> Result<Vec2> {
        let mut query = self.world().query_filtered::<&Window, With<PrimaryWindow>>();
        let window = query.single(self.world()).context("Expected primary window")?;
        #[expect(
            clippy::cast_precision_loss,
            reason = "screen coordinate precision loss is acceptable"
        )]
        Ok(Vec2::new(window.physical_width() as f32 * 0.5, window.physical_height() as f32 * 0.5))
    }

    pub fn scroll_mouse(&mut self, cursor: Vec2, delta: Vec2, frames: usize) -> Result<()> {
        for _ in 0..frames {
            self.set_cursor_position(cursor)?;
            let window_entity = self.primary_window_entity()?;
            self.world().write_message(MouseWheel {
                unit:   MouseScrollUnit::Line,
                x:      delta.x,
                y:      delta.y,
                window: window_entity,
            });

            self.drive_frames(1);
        }
        Ok(())
    }

    pub fn press_key(
        &mut self,
        key_code: keyboard::KeyCode,
        logical_key: keyboard::Key,
    ) -> Result<()> {
        let window = self.primary_window_entity()?;
        self.world().write_message(KeyboardInput {
            key_code,
            logical_key: logical_key.clone(),
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window,
        });
        self.drive_frames(2);

        self.world().write_message(KeyboardInput {
            key_code,
            logical_key,
            state: ButtonState::Released,
            text: None,
            repeat: false,
            window,
        });
        self.drive_frames(2);

        Ok(())
    }
}

#[derive(Resource, Default)]
struct ScreenshotCapture {
    image: Option<Image>,
}

fn store_screenshot(event: On<ScreenshotCaptured>, mut capture: ResMut<ScreenshotCapture>) {
    capture.image = Some(event.image.clone());
}

fn load_image(path: &Path) -> Result<Image> {
    let bytes = fs::read(path)
        .with_context(|| format!("Failed to read screenshot baseline from {}", path.display()))?;
    Image::from_buffer(
        &bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::default(),
        RenderAssetUsages::default(),
    )
    .with_context(|| format!("Failed to load screenshot baseline from {}", path.display()))
}

fn compare_images(baseline: Image, actual: Image, diff_path: &Path) -> Result<()> {
    bevy::log::info!("Comparing screenshot with baseline");

    omniatc_diff_image::compare_images(
        baseline
            .try_into_dynamic()
            .context("Failed to convert baseline screenshot to dynamic image")?
            .to_rgb8(),
        actual
            .try_into_dynamic()
            .context("Failed to convert captured screenshot to dynamic image")?
            .to_rgb8(),
        diff_path,
    )
}

fn sanitize_label(label: &str, kind: &str) -> Result<String> {
    let mut sanitized = String::new();
    let mut last_dash = false;
    for ch in label.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            sanitized.push(ch);
            last_dash = false;
        } else if !last_dash {
            sanitized.push('-');
            last_dash = true;
        }
    }
    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        anyhow::bail!("Invalid screenshot {kind} name '{label}'");
    }
    Ok(trimmed.to_string())
}

struct TestWindowHandler {
    sender: Option<mpsc::Sender<Result<RawHandleWrapper>>>,
}

impl ApplicationHandler for TestWindowHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = match event_loop.create_window(WinitWindow::default_attributes()) {
            Ok(window) => window,
            Err(err) => {
                if let Some(sender) = self.sender.take() {
                    let _ = sender.send(Err(err.into()));
                }
                event_loop.exit();
                return;
            }
        };
        let window_wrapper = WindowWrapper::new(window);
        let raw_handle =
            RawHandleWrapper::new(&window_wrapper).context("Failed to create window handle");
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(raw_handle);
        }
        event_loop.exit();
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        _event: WindowEvent,
    ) {
    }
}
