use amethyst::{
    animation::{
        get_animation_set, AnimationBundle, AnimationCommand, AnimationControlSet, AnimationSet,
        AnimationSetPrefab, EndControl,
    },
    assets::{
        AssetStorage, Handle, Loader, PrefabData, PrefabLoader, PrefabLoaderSystemDesc,
        ProgressCounter, RonFormat,
    },
    core::{
        geometry::Plane,
        math::{Point2, Point3, Vector2, Vector3},
        Transform, TransformBundle,
    },
    derive::PrefabData,
    ecs::{
        prelude::Entity, BitSet, Entities, Join, Read, ReadExpect, ReadStorage, System, SystemData,
        World, Write, WriteStorage,
    },
    error::Error,
    input::{is_close_requested, is_key_down, InputBundle, InputHandler, StringBindings},
    prelude::*,
    renderer::{
        camera::{ActiveCamera, Camera},
        formats::texture::ImageFormat,
        plugins::{RenderFlat2D, RenderToWindow},
        sprite::{prefab::SpriteScenePrefab, SpriteRender},
        types::DefaultBackend,
        RenderingBundle, Texture,
    },
    tiles::{
        iters::Region, CoordinateEncoder, DrawTiles2DBounds, FlatEncoder, Map, RenderTiles2D, Tile,
        TileMap,
    },
    utils::application_root_dir,
    window::ScreenDimensions,
    winit,
};
use serde::{Deserialize, Serialize};

use tiled_support::{TileGid, TileMapPrefab, TiledFormat};

#[derive(Default)]
pub struct CurrentTileZ(pub u32, pub (f32, f32));

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
        Write<'s, CurrentTileZ>,
    );

    fn run(
        &mut self,
        (active_camera, entities, cameras, mut transforms, input, mut current_tile_z): Self::SystemData,
    ) {
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

#[derive(Default, Debug)]
pub struct DrawRegionTileBounds;
impl DrawTiles2DBounds for DrawRegionTileBounds {
    fn bounds<T: Tile, E: CoordinateEncoder>(map: &TileMap<T, E>, world: &World) -> Region {
        let camera_fetch =
            amethyst::renderer::submodules::gather::CameraGatherer::gather_camera_entity(world);
        assert!(camera_fetch.is_some());

        let (entities, active_camera, screen_dimensions, transforms, cameras, current_tile_z) =
            <(
                Entities<'_>,
                Read<'_, ActiveCamera>,
                ReadExpect<'_, ScreenDimensions>,
                ReadStorage<'_, Transform>,
                ReadStorage<'_, Camera>,
                Read<'_, CurrentTileZ>,
            )>::fetch(world);

        //let camera_tile_id = entity_tile_ids.get(camera_entity).u wrap();
        let mut camera_join = (&cameras, &transforms).join();
        if let Some((camera, camera_transform)) = active_camera
            .entity
            .and_then(|a| camera_join.get(a, &entities))
            .or_else(|| camera_join.next())
        {
            let current_z = 5.0; // current_tile_z.0 as f32 * map.tile_dimensions().z as f32;

            // Shoot a ray at each corner of the camera, and determine what tile it hits at the target
            // Z-level
            let proj = camera.projection();
            let plane = Plane::with_z(current_z);

            let ray = proj.screen_ray(
                Point2::new(0.0, 0.0),
                Vector2::new(screen_dimensions.width(), screen_dimensions.height()),
                camera_transform,
            );
            let top_left = ray.at_distance(ray.intersect_plane(&plane).unwrap());

            let ray = proj.screen_ray(
                Point2::new(screen_dimensions.width(), screen_dimensions.height()),
                Vector2::new(screen_dimensions.width(), screen_dimensions.height()),
                camera_transform,
            );
            let bottom_right = ray.at_distance(ray.intersect_plane(&plane).unwrap()).coords
                + Vector3::new(
                    map.tile_dimensions().x as f32 * 5.0,
                    -(map.tile_dimensions().y as f32 * 5.0),
                    0.0,
                );

            let half_dimensions = Vector3::new(
                (map.tile_dimensions().x * map.dimensions().x) as f32 / 2.0,
                (map.tile_dimensions().x * map.dimensions().y) as f32 / 2.0,
                (map.tile_dimensions().x * map.dimensions().z) as f32 / 2.0,
            );
            let bottom_right = Point3::new(
                bottom_right
                    .x
                    .min(half_dimensions.x - map.tile_dimensions().x as f32)
                    .max(-half_dimensions.x),
                bottom_right
                    .y
                    .min(half_dimensions.y - map.tile_dimensions().y as f32)
                    .max(-half_dimensions.y + map.tile_dimensions().y as f32),
                bottom_right
                    .z
                    .min(half_dimensions.z - map.tile_dimensions().z as f32)
                    .max(-half_dimensions.z),
            );

            let min = map
                .to_tile(&top_left.coords, None)
                .unwrap_or_else(|| Point3::new(0, 0, current_tile_z.0));

            let max = map.to_tile(&bottom_right.coords, None).unwrap_or_else(|| {
                Point3::new(
                    map.dimensions().x - 1,
                    map.dimensions().y - 1,
                    current_tile_z.0,
                )
            });
            Region::new(min, max)
        } else {
            Region::empty()
        }
    }
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

    fn handle_event(
        &mut self,
        data: StateData<'_, GameData<'_, '_>>,
        event: StateEvent,
    ) -> SimpleTrans {
        let StateData { .. } = data;
        if let StateEvent::Window(event) = &event {
            if is_close_requested(&event) || is_key_down(&event, winit::VirtualKeyCode::Escape) {
                Trans::Quit
            } else {
                Trans::None
            }
        } else {
            Trans::None
        }
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
                    RenderToWindow::from_config_path(display_config_path)?.with_clear([1.0; 4]),
                )
                .with_plugin(RenderFlat2D::default())
                .with_plugin(RenderTiles2D::<TileGid, FlatEncoder>::default()),
        )?;

    let mut game = Application::build(assets_directory, Example::default())?.build(game_data)?;
    game.run();
    Ok(())
}
