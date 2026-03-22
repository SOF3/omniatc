use std::mem;

use bevy::app::{self, App, Plugin};
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Camera, Camera2d};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{self, IntoScheduleConfigs, Schedulable, ScheduleConfigs};
use bevy::ecs::system::{Commands, Local, ParamSet, ResMut, SystemParam};
use bevy_egui::egui::WidgetText;
use bevy_egui::{
    EguiContexts, EguiGlobalSettings, EguiPrimaryContextPass, PrimaryEguiContext, egui,
};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_dock::{DockArea, DockState, NodeIndex, SurfaceIndex, TabIndex};

use crate::EguiSystemSets;
use crate::render::{config_editor, level_info, messages, object_info, twodim};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<State>();
        app.allow_ambiguous_resource::<State>();
        app.add_systems(app::Startup, setup_system);
        app.add_systems(
            EguiPrimaryContextPass,
            Tab::schedule_configs(render_dock_system.in_set(EguiSystemSets::Dock)),
        );
    }
}

pub trait TabType {
    fn schedule_configs<T>(configs: ScheduleConfigs<T>) -> ScheduleConfigs<T>
    where
        T: Schedulable<Metadata = schedule::GraphInfo, GroupMetadata = schedule::Chain>,
    {
        configs
    }

    type TitleSystemParam<'w, 's>: SystemParam;
    fn title(&self, param: Self::TitleSystemParam<'_, '_>) -> String;

    type UiSystemParam<'w, 's>: SystemParam;
    fn ui(&mut self, param: Self::UiSystemParam<'_, '_>, ui: &mut egui::Ui, order: usize);

    type OnCloseSystemParam<'w, 's>: SystemParam;
    fn on_close(&mut self, _params: Self::OnCloseSystemParam<'_, '_>) -> OnCloseResponse {
        OnCloseResponse::Close
    }

    type PrepareRenderSystemParam<'w, 's>: SystemParam;
    fn prepare_render(
        &mut self,
        _contexts: &mut EguiContexts,
        _param: Self::PrepareRenderSystemParam<'_, '_>,
    ) {
    }
}

macro_rules! define_tabs {
(
    $(
        #[$meta:meta]
        $ps_path:tt
        $variant:ident ($tab_type:ty)
    )*
) => {
    pub enum Tab {
        $(
            #[$meta]
            $variant($tab_type),
        )*
    }

    impl Tab {
        fn schedule_configs<T>(mut configs: ScheduleConfigs<T>) -> ScheduleConfigs<T>
        where
            T: Schedulable<Metadata = schedule::GraphInfo, GroupMetadata = schedule::Chain>,
        {
            $(configs = <$tab_type>::schedule_configs(configs);)*
            configs
        }

        fn prepare_render(&mut self, contexts: &mut EguiContexts, viewer: &mut TabViewer) {
            match self {
                $(
                    Tab::$variant(t) => do_ps_path!(viewer.ps, $ps_path; |p| t.prepare_render(contexts, p.p0())),
                )*
            }
        }
    }

    #[derive(SystemParam)]
    struct TabViewer<'w, 's>{
        /// Render order of the current tab.
        next_order: Local<'s, usize>,
        ps: recurse_param_set!(
            'w, 's,
            $(
                (
                    <$tab_type as TabType>::PrepareRenderSystemParam<'w, 's>,
                    <$tab_type as TabType>::TitleSystemParam<'w, 's>,
                    <$tab_type as TabType>::UiSystemParam<'w, 's>,
                    <$tab_type as TabType>::OnCloseSystemParam<'w, 's>,
                ),
            )*
        ),
    }

    impl<'w, 's> egui_dock::TabViewer for TabViewer<'w, 's> {
        type Tab = Tab;

        fn title(&mut self, tab: &mut Tab) -> WidgetText {
            match tab {
                $(
                    Tab::$variant(t) => do_ps_path!(self.ps, $ps_path; |p| t.title(p.p1())) ,
                )*
            }.into()
        }

        fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
            let order_mut = &mut *self.next_order;
            let order = mem::replace(order_mut, *order_mut + 1);
            match tab {
                $(
                    Tab::$variant(t) => do_ps_path!(self.ps, $ps_path; |p| t.ui(p.p2(), ui, order)),
                )*
            }
        }

        fn on_close(&mut self, tab: &mut Tab) -> OnCloseResponse {
            match tab {
                $(
                    Tab::$variant(t) => do_ps_path!(self.ps, $ps_path; |p| t.on_close(p.p3())),
                )*
            }
        }
    }
}
}

macro_rules! do_ps_path {
($ps:expr, (); |$var:ident| $closure:expr) => {{
    let mut $var = $ps.p1();
    $closure
}};
($ps:expr, ($p0:ident $($rest:ident)*); |$var:ident| $closure:expr) => {{
    let mut $var = $ps.$p0();
    do_ps_path!($var, ($($rest)*); |$var| $closure)
}};
}

macro_rules! recurse_param_set {
($w:lifetime, $s: lifetime,) => { () };
($w:lifetime, $s: lifetime, $args:tt, $($rest:tt)*) => {
    ParamSet<'w, 's, (recurse_param_set!($w, $s, $($rest)*), ParamSet<'w, 's, $args>)>
}
}

define_tabs! {
    // Singleton tabs.

    /// Show scalar level info.
    () LevelInfo(level_info::ScalarTabType)
    /// List of all objects.
    (p0) ObjectList(level_info::objects::TabType)
    /// Text transmission history.
    (p0 p0) Messages(messages::TabType)
    /// Quest browser.
    (p0 p0 p0) Quests(level_info::quests::TabType)
    /// Configuration editor.
    (p0 p0 p0 p0) ConfigEditor(config_editor::TabType)

    // Repeatable tabs.

    /// Show information about an object.
    (p0 p0 p0 p0 p0) ObjectInfo(object_info::TabType)
    /// Render 2D world camera.
    (p0 p0 p0 p0 p0 p0) TwoDimCamera(twodim::camera::TabType)
}

#[derive(Resource, Default)]
pub struct State {
    pub state: Option<DockState<Tab>>,
}

fn setup_system(
    mut egui_global_settings: ResMut<EguiGlobalSettings>,
    mut dock_state: ResMut<State>,
    mut params: DefaultStateParams,
) {
    egui_global_settings.auto_create_primary_context = false;
    params.commands.spawn((
        Camera2d,
        Camera { order: 1, ..Default::default() },
        PrimaryEguiContext,
        RenderLayers::none(),
    )); // egui camera
    dock_state.state = Some(create_initial_state(&mut params));
}

#[derive(SystemParam)]
struct DefaultStateParams<'w, 's> {
    commands: Commands<'w, 's>,
    images:   twodim::camera::SpawnParams<'w>,
}

fn create_initial_state(params: &mut DefaultStateParams) -> DockState<Tab> {
    let camera_dock = twodim::camera::new_tab(&mut params.images, &mut params.commands);
    let mut dock_state = DockState::new([camera_dock].into());
    level_info::create_splits(&mut dock_state);
    object_info::create_splits(&mut dock_state);
    messages::create_splits(&mut dock_state);
    dock_state
}

fn render_dock_system(
    mut contexts: EguiContexts,
    mut dock_state: ResMut<State>,
    mut viewer: TabViewer,
) {
    let Some(state) = dock_state.state.as_mut() else { return };

    for (_, tab) in state.iter_all_tabs_mut() {
        tab.prepare_render(&mut contexts, &mut viewer);
    }

    let Ok(ctx) = contexts.ctx_mut() else { return };
    *viewer.next_order = 0;

    DockArea::new(state)
        .style(egui_dock::Style { ..egui_dock::Style::from_egui(ctx.style().as_ref()) })
        .show(ctx, &mut viewer);
}

pub fn focus_or_create_tab(
    state: &mut DockState<Tab>,
    creator: impl FnOnce() -> Tab,
    placement: impl AlwaysTabPlacement,
) {
    let path = placement.always_place(state, creator);
    state.set_active_tab(path);
}

pub type NodePath = (SurfaceIndex, NodeIndex, TabIndex);

pub trait TabPlacement: Sized {
    fn place<F: FnOnce() -> Tab>(self, state: &mut DockState<Tab>, tab: F) -> Result<NodePath, F>;

    fn or<P: TabPlacement>(self, other: P) -> impl TabPlacement {
        struct Or<A, B>(A, B);

        impl<A: TabPlacement, B: TabPlacement> TabPlacement for Or<A, B> {
            fn place<F: FnOnce() -> Tab>(
                self,
                state: &mut DockState<Tab>,
                tab: F,
            ) -> Result<NodePath, F> {
                self.0.place(state, tab).or_else(|tab| self.1.place(state, tab))
            }
        }

        Or(self, other)
    }

    fn or_always<P: AlwaysTabPlacement>(self, other: P) -> impl AlwaysTabPlacement {
        struct Or<A, B>(A, B);

        impl<A: TabPlacement, B: AlwaysTabPlacement> AlwaysTabPlacement for Or<A, B> {
            fn always_place<F: FnOnce() -> Tab>(
                self,
                state: &mut DockState<Tab>,
                tab: F,
            ) -> NodePath {
                self.0.place(state, tab).unwrap_or_else(|tab| self.1.always_place(state, tab))
            }
        }

        Or(self, other)
    }
}

pub trait AlwaysTabPlacement: TabPlacement {
    fn always_place<F: FnOnce() -> Tab>(self, state: &mut DockState<Tab>, tab: F) -> NodePath;
}

impl<T: AlwaysTabPlacement> TabPlacement for T {
    fn place<F: FnOnce() -> Tab>(self, state: &mut DockState<Tab>, tab: F) -> Result<NodePath, F> {
        Ok(self.always_place(state, tab))
    }
}

pub struct ReplaceTab<R: Fn(&Tab) -> bool>(pub R);

impl<R> TabPlacement for ReplaceTab<R>
where
    R: Fn(&Tab) -> bool,
{
    fn place<F: FnOnce() -> Tab>(
        self,
        state: &mut DockState<Tab>,
        make_tab: F,
    ) -> Result<NodePath, F> {
        for (si, surface) in state.iter_surfaces_mut().enumerate() {
            let si = SurfaceIndex(si);
            for (ni, node) in surface.iter_nodes_mut().enumerate() {
                let ni = NodeIndex(ni);
                if let Some(leaf) = node.get_leaf_mut() {
                    for (ti, tab) in leaf.tabs.iter_mut().enumerate() {
                        let ti = TabIndex(ti);

                        if (self.0)(tab) {
                            *tab = make_tab();
                            return Ok((si, ni, ti));
                        }
                    }
                }
            }
        }

        Err(make_tab)
    }
}

pub struct AfterTab<R: Fn(&Tab) -> bool>(pub R);

impl<R> TabPlacement for AfterTab<R>
where
    R: Fn(&Tab) -> bool,
{
    fn place<F: FnOnce() -> Tab>(
        self,
        state: &mut DockState<Tab>,
        tab_fn: F,
    ) -> Result<NodePath, F> {
        for (si, surface) in state.iter_surfaces_mut().enumerate() {
            let si = SurfaceIndex(si);
            for (ni, node) in surface.iter_nodes_mut().enumerate() {
                let ni = NodeIndex(ni);
                if let Some(leaf) = node.get_leaf_mut() {
                    for (ti, tab) in leaf.tabs.iter().enumerate() {
                        if (self.0)(tab) {
                            leaf.tabs.insert(ti + 1, tab_fn());
                            return Ok((si, ni, TabIndex(ti + 1)));
                        }
                    }
                }
            }
        }

        Err(tab_fn)
    }
}

pub struct SplitRoot {
    pub split: egui_dock::Split,
    pub ratio: f32,
}

impl AlwaysTabPlacement for SplitRoot {
    fn always_place<F: FnOnce() -> Tab>(self, state: &mut DockState<Tab>, tab: F) -> NodePath {
        let [_, new_node] = state.split(
            (SurfaceIndex::main(), NodeIndex::root()),
            self.split,
            self.ratio,
            egui_dock::Node::leaf(tab()),
        );
        (SurfaceIndex::main(), new_node, TabIndex(0))
    }
}

pub struct NewSurface;

impl AlwaysTabPlacement for NewSurface {
    fn always_place<F: FnOnce() -> Tab>(self, state: &mut DockState<Tab>, tab: F) -> NodePath {
        let window = state.add_window([tab()].into());
        (window, NodeIndex::root(), TabIndex(0))
    }
}
