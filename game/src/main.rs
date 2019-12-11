use amethyst::{
    animation::{
        get_animation_set, AnimationBundle, AnimationCommand, AnimationControlSet, AnimationSet,
        AnimationSetPrefab, EndControl,
    },
    assets::{
        AssetStorage, Handle, Loader, PrefabData, PrefabLoader, PrefabLoaderSystemDesc,
        ProgressCounter, RonFormat,
    },
    core::{math::Vector3, Transform, TransformBundle},
    derive::PrefabData,
    ecs::{prelude::Entity, Entities, Join, Read, ReadStorage, System, WriteStorage},
    error::Error,
    input::{InputBundle, InputHandler, StringBindings},
    prelude::*,
    renderer::{
        camera::{ActiveCamera, Camera},
        formats::texture::ImageFormat,
        plugins::{RenderFlat2D, RenderToWindow},
        sprite::{prefab::SpriteScenePrefab, SpriteRender},
        types::DefaultBackend,
        RenderingBundle, Texture,
    },
    tiles::{FlatEncoder, RenderTiles2D},
    utils::application_root_dir,
    window::ScreenDimensions,
};
use serde::{Deserialize, Serialize};

use tiled_support::{TileGid, TileMapPrefab, TiledFormat};

#[derive(Eq, PartialOrd, PartialEq, Hash, Debug, Copy, Clone, Deserialize, Serialize)]
enum AnimationId {
    StillUp,
}

#[derive(Debug, Clone, Deserialize, PrefabData)]
struct HeroPrefabData {
    /// Information for rendering a scene with sprites
    sprite_scene: SpriteScenePrefab,
    /// Аll animations that can be run on the entity
    animation_set: AnimationSetPrefab<AnimationId, SpriteRender>,
}

#[derive(Default)]
pub struct CameraMovementSystem;
impl<'s> System<'s> for CameraMovementSystem {
    type SystemData = (
        Read<'s, ActiveCamera>,
        Entities<'s>,
        ReadStorage<'s, Camera>,
        WriteStorage<'s, Transform>,
        Read<'s, InputHandler<StringBindings>>,
    );

    fn run(&mut self, (active_camera, entities, cameras, mut transforms, input): Self::SystemData) {
        let x_move = input.axis_value("camera_x").unwrap();
        let y_move = input.axis_value("camera_y").unwrap();
        let z_move = input.axis_value("camera_z").unwrap();
        let z_move_scale = input.axis_value("camera_scale").unwrap();

        if x_move != 0.0 || y_move != 0.0 || z_move != 0.0 || z_move_scale != 0.0 {
            let mut camera_join = (&cameras, &mut transforms).join();
            if let Some((_, camera_transform)) = active_camera
                .entity
                .and_then(|a| camera_join.get(a, &entities))
                .or_else(|| camera_join.next())
            {
                camera_transform.prepend_translation_x(x_move * 5.0);
                camera_transform.prepend_translation_y(y_move * 5.0);
                camera_transform.prepend_translation_z(z_move);

                let z_scale = 0.01 * z_move_scale;
                let scale = camera_transform.scale();
                let scale = Vector3::new(scale.x + z_scale, scale.y + z_scale, scale.z + z_scale);
                camera_transform.set_scale(scale);
            }
        }
    }
}

pub fn load_texture<N>(name: N, world: &World) -> Handle<Texture>
where
    N: Into<String>,
{
    let loader = world.read_resource::<Loader>();
    loader.load(
        name,
        ImageFormat::default(),
        (),
        &world.read_resource::<AssetStorage<Texture>>(),
    )
}

#[derive(Default)]
struct Example {
    pub progress_counter: Option<ProgressCounter>,
}

impl SimpleState for Example {
    fn on_start(&mut self, data: StateData<'_, GameData<'_, '_>>) {
        let world = data.world;

        // Crates new progress counter
        self.progress_counter = Some(Default::default());

        let (width, height) = {
            let dim = world.read_resource::<ScreenDimensions>();
            (dim.width(), dim.height())
        };

        // Starts asset loading
        let hero_prefab = world.exec(|loader: PrefabLoader<'_, HeroPrefabData>| {
            loader.load(
                "prefab/hero.ron",
                RonFormat,
                self.progress_counter.as_mut().unwrap(),
            )
        });

        world.create_entity().with(hero_prefab).build();

        // Init camera
        world
            .create_entity()
            .with(Transform::from(Vector3::new(0.0, 0.0, 5.0)))
            .with(Camera::standard_2d(width, height))
            .build();

        // Use a prefab loader to get the tiled .tmx file loaded
        let prefab_handle = world.exec(|loader: PrefabLoader<'_, TileMapPrefab>| {
            loader.load("maps/map.tmx", TiledFormat, ())
        });

        let _map_entity = world
            .create_entity()
            .with(prefab_handle)
            .with(Transform::default())
            .build();
    }

    fn update(&mut self, data: &mut StateData<'_, GameData<'_, '_>>) -> SimpleTrans {
        // Checks if we are still loading data

        if let Some(ref progress_counter) = self.progress_counter {
            // Checks progress
            if progress_counter.is_complete() {
                let StateData { world, .. } = data;
                // Execute a pass similar to a system
                world.exec(
                    |(entities, animation_sets, mut control_sets): (
                        Entities,
                        ReadStorage<AnimationSet<AnimationId, SpriteRender>>,
                        WriteStorage<AnimationControlSet<AnimationId, SpriteRender>>,
                    )| {
                        // For each entity that has AnimationSet
                        for (entity, animation_set) in (&entities, &animation_sets).join() {
                            // Creates a new AnimationControlSet for the entity
                            let control_set = get_animation_set(&mut control_sets, entity).unwrap();
                            // Adds the `Fly` animation to AnimationControlSet and loops infinitely
                            control_set.add_animation(
                                AnimationId::StillUp,
                                &animation_set.get(&AnimationId::StillUp).unwrap(),
                                EndControl::Loop(None),
                                1.0,
                                AnimationCommand::Start,
                            );
                        }
                    },
                );
                // All data loaded
                self.progress_counter = None;
            }
        }
        Trans::None
    }
}

fn main() -> amethyst::Result<()> {
    amethyst::Logger::from_config(Default::default())
        .level_for("amethyst_tiles", amethyst::LogLevelFilter::Warn)
        .start();

    let app_root = application_root_dir()?;
    println!("{:?}", app_root);
    let assets_directory = app_root.join("assets");
    let display_config_path = app_root.join("config/display.ron");

    let game_data = GameDataBuilder::default()
        .with_system_desc(PrefabLoaderSystemDesc::<TileMapPrefab>::default(), "", &[])
        .with_system_desc(
            PrefabLoaderSystemDesc::<HeroPrefabData>::default(),
            "scene_loader",
            &[],
        )
        .with_bundle(AnimationBundle::<AnimationId, SpriteRender>::new(
            "sprite_animation_control",
            "sprite_sampler_interpolation",
        ))?
        .with_bundle(
            TransformBundle::new()
                .with_dep(&["sprite_animation_control", "sprite_sampler_interpolation"]),
        )?
        .with_bundle(
            InputBundle::<StringBindings>::new()
                .with_bindings_from_file("game/config/input.ron")?,
        )?
        .with(CameraMovementSystem::default(), "movement", &[])
        .with_bundle(
            RenderingBundle::<DefaultBackend>::new()
                .with_plugin(
                    RenderToWindow::from_config_path(display_config_path).with_clear([1.0; 4]),
                )
                .with_plugin(RenderFlat2D::default())
                .with_plugin(RenderTiles2D::<TileGid, FlatEncoder>::default()),
        )?;

    let mut game = Application::build(assets_directory, Example::default())?.build(game_data)?;
    game.run();
    Ok(())
}
