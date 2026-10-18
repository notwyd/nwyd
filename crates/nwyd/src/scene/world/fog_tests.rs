use super::*;
use crate::scene::view_distance::ViewDistance;

#[test]
fn retail_restores_authored_fog_and_extended_modes_scale_after_camera_adjustment() {
    for distance in [1.2, CUTSCENE_DISTANCE, 15.0] {
        for pitch in [-1.0, 0.0, 0.75] {
            let mut environment = Environment::for_day(0, 16, 16).for_scene(SCENE_FIELD);

            environment.apply_pitch(SCENE_FIELD, pitch);

            let original = environment;
            let retail = environment.fog_at(distance);

            for choice in ViewDistance::ALL {
                let fog = environment.visual_fog(SCENE_FIELD, distance, false, choice);

                assert_eq!(fog.start, retail.start * choice.multiplier());
                assert_eq!(fog.end, retail.end * choice.multiplier());
                assert_eq!(fog.color, retail.color);
                assert_eq!(fog.enabled, retail.enabled);
                assert!(fog.start < fog.end);
                assert!(fog.end < environment.far_plane * choice.multiplier());
            }

            assert_eq!(environment, original);
        }
    }
}

#[test]
fn underwater_fog_and_scene_overrides_keep_their_ranges() {
    for (zone, scene, underwater) in [
        ((16, 16), SCENE_FIELD, true),
        ((16, 16), SCENE_BUDGET, false),
        ((16, 16), SCENE_SELECT_CHAR, false),
        ((16, 16), SCENE_LOGIN, false),
    ] {
        let environment = Environment::for_day(0, zone.0, zone.1).for_scene(scene);

        for choice in ViewDistance::ALL {
            assert_eq!(
                environment.visual_fog(scene, CUTSCENE_DISTANCE, underwater, choice),
                environment.fog_at(CUTSCENE_DISTANCE),
            );
            assert_eq!(
                environment.view_far_plane(scene, choice),
                environment.far_plane
                    * if scene == SCENE_FIELD {
                        choice.multiplier()
                    } else {
                        1.0
                    },
            );
        }
    }
}

#[test]
fn extended_visibility_follows_scaled_fog_with_an_unchanged_near_plane() {
    let environment = Environment::for_day(0, 16, 16).for_scene(SCENE_FIELD);
    let retail = environment.fog_at(CUTSCENE_DISTANCE);
    let retail_projection = Projection::new((800, 800), (0, 0), environment.far_plane);

    for choice in ViewDistance::ALL {
        let fog = environment.visual_fog(SCENE_FIELD, CUTSCENE_DISTANCE, false, choice);
        let projection = Projection::new(
            (800, 800),
            (0, 0),
            environment.view_far_plane(SCENE_FIELD, choice),
        );
        let camera = super::super::human_visibility::Camera {
            eye: glam::Vec3::ZERO,
            forward: glam::Vec3::Z,
            clip: projection.matrix(),
            viewport: (800, 800),
            fog_end: fog.end,
        };

        assert_eq!(projection.near, retail_projection.near);
        assert_eq!(projection.fov_y, retail_projection.fov_y);
        assert_eq!(projection.aspect, retail_projection.aspect);
        assert_eq!(projection.far, retail_projection.far * choice.multiplier());
        assert!(camera.admits(glam::Vec3::new(0.0, 0.0, fog.end - 2.0), 1.0, Some(1.0)));
        assert!(!camera.admits(glam::Vec3::new(0.0, 0.0, fog.end + 4.0), 1.0, Some(1.0)));
        assert_eq!(
            camera.admits(glam::Vec3::new(0.0, 0.0, retail.end * 1.25), 1.0, Some(1.0)),
            choice != ViewDistance::Retail,
        );
    }
}

#[test]
fn special_and_dungeon_fields_scale_their_own_authored_ranges_and_keep_their_style() {
    for (x, y) in [(28, 22), (18, 30), (13, 28), (28, 10)] {
        let environment = Environment::for_day(0, x, y).for_scene(SCENE_FIELD);
        let original = environment.fog_at(CUTSCENE_DISTANCE);

        for choice in ViewDistance::ALL {
            let fog = environment.visual_fog(SCENE_FIELD, CUTSCENE_DISTANCE, false, choice);

            assert_eq!(fog.start, original.start * choice.multiplier());
            assert_eq!(fog.end, original.end * choice.multiplier());
            assert_eq!(fog.color, original.color);
            assert_eq!(fog.enabled, original.enabled);
            assert_eq!(
                environment.view_far_plane(SCENE_FIELD, choice),
                environment.far_plane * choice.multiplier()
            );
        }
    }
}

pub(super) fn field_world() -> World {
    use formats::data::item_list;
    use formats::mesh::registry;
    use formats::texture::texture_list;
    use formats::world::object_bin;
    use formats::world::trn::TrnCell;

    let mut masks = vec![0; object_bin::PAYLOAD_LEN];

    masks.extend_from_slice(&object_bin::checksum(&masks).to_le_bytes());

    let game = GameDir::from_files(
        "view-distance",
        [
            ("object.bin".into(), masks),
            ("Env/view-distance.dat".into(), Vec::new()),
        ],
    )
    .unwrap();
    let registries = Arc::new(Registries {
        resource_names: formats::scene::rc::parse(&[]).unwrap(),
        entries: Vec::new(),
        valid: registry::parse_valid_index(&vec![0; registry::VALID_INDEX_LEN]).unwrap(),
        sounds: formats::audio::ani_sound::AniSound::default(),
        textures: texture_list::parse(&[]).unwrap(),
        effect_textures: texture_list::parse(&[]).unwrap(),
        items: item_list::ItemList::from_plain(
            vec![0; item_list::BODY_LEN],
            formats::text::Codepage::Windows1252,
        ),
        list: Default::default(),
        arena_visual: Vec::new(),
    });
    let terrain = Terrain {
        name: "view-distance".into(),
        zone_x: 16,
        zone_y: 16,
        cells: vec![
            TrnCell {
                height: 0,
                uv_mode: 0,
                uv_mode2: 0,
                pad: 0,
                tile0: 0,
                tile1: 0,
                color: [255; 4],
            };
            formats::world::TRN_DIM * formats::world::TRN_DIM
        ],
    };
    let environment = Environment::for_day(0, 16, 16).for_scene(SCENE_FIELD);
    let actors = actors::build(
        &game,
        None,
        &terrain,
        &[],
        environment.light0,
        environment.light1,
        false,
        0,
        Arc::clone(&registries),
    )
    .unwrap();
    let mut rng = Lcg::new(1);

    World {
        identity: next_world_identity(),
        terrain_revision: 0,
        terrain_pages: Vec::new(),
        background_loader: None,
        visual_zone: None,
        visual_collision_dirty: false,
        background_retry: None,
        variant: FIELD_VARIANT,
        scene: SCENE_FIELD,
        name: terrain.name.clone(),
        lamps: lights::Lamps::new(SCENE_FIELD),
        current: (16, 16),
        force_load: None,
        force_load_restore_yaw: None,
        mesh: world::build_terrain(&terrain),
        terrain,
        streamed: Vec::new(),
        tiles: Vec::new(),
        tile_list: env_texture_list::parse(&vec![0; env_texture_list::FILE_LEN]).unwrap(),
        missing: Vec::new(),
        path: CameraPath {
            action: CamAction {
                keyframes: Vec::new(),
            },
            start_ms: 0,
        },
        camera_distance: CUTSCENE_DISTANCE,
        environment,
        view_distance: ViewDistance::default(),
        env_type: environment.env_type,
        env_global: environment.env_type,
        sky: sky::Sky::at(0),
        sky_ornaments: sky::Ornaments::new(&mut rng),
        sun: None,
        projected: None,
        weather: weather::Weather::new(0),
        rain: rain::Rain::new(&mut rng),
        snow: snow::Snow::new(1.0, &mut rng),
        snow_fast: snow::Snow::new(2.0, &mut rng),
        weather_rng: rng,
        weather_target: None,
        weather_anchor: weather::Anchor::adrift(2064.0, 2064.0),
        pose: Pose {
            eye: glam::Vec3::new(2064.0, 10.0, 2064.0),
            yaw: 0.0,
            pitch: 0.0,
        },
        objects: objects::build(&game, "view-distance", (16, 16), 0, registries).unwrap(),
        actors,
        lights: lights::Lights::default(),
        effects: effects::BillboardScene::empty(),
        effect_scratch: effects::Frame::default(),
        actor_scratch: actors::Frame::default(),
        effect_owners: super::super::effect_context::OwnerIndex::default(),
        sea: sea::SeaScene::empty(),
        ambient: Vec::new(),
        terrain_ambient: super::super::terrain_ambient::State::default(),
        drips: Vec::new(),
        drip_rng: Lcg::new(1),
        script: None,
        sounds: Vec::new(),
        damage_sounds: Vec::new(),
    }
}

#[test]
fn live_distance_switching_changes_render_ranges_without_moving_or_rebuilding_the_world() {
    let mut world = field_world();
    let pose = world.pose;
    let initial = world.frame_at(pose, 0, (800, 800));
    let original = world.environment;
    let fog = world.fog();
    let mesh = world.mesh.clone();
    let terrain = world.terrain.clone();
    let collision = world.actors.window.clone();
    let identity = world.identity;
    let revision = world.terrain_revision;
    let reach = world.pose_reach();
    let distant_position = pose.eye + glam::Vec3::X * 90.0;
    let distant_bounds = render::cull::Aabb {
        min: (distant_position - glam::Vec3::ONE).to_array(),
        max: (distant_position + glam::Vec3::ONE).to_array(),
    };

    assert_eq!(world.view_distance, ViewDistance::Retail);
    assert_eq!(fog, original.fog_at(world.camera_distance));
    assert_eq!(initial.projection.far, original.far_plane);
    assert_eq!(world.skin_frame(&initial).scroll[1], 0.0);
    assert!(!initial.object_frustum().admits(&distant_bounds));

    for choice in [
        ViewDistance::Distant,
        ViewDistance::Far,
        ViewDistance::Maximum,
        ViewDistance::FiveTimes,
        ViewDistance::SixTimes,
        ViewDistance::SevenTimes,
        ViewDistance::EightTimes,
        ViewDistance::Extended,
        ViewDistance::Retail,
    ] {
        world.set_view_distance(choice);

        assert_eq!(world.projected, None);

        let frame = world.frame_at(pose, 0, (800, 800));

        assert_eq!(
            frame.projection.far,
            initial.projection.far * choice.multiplier()
        );
        assert_eq!(frame.projection.near, initial.projection.near);
        assert_eq!(
            frame.object_frustum().admits(&distant_bounds),
            choice != ViewDistance::Retail,
        );
        assert_eq!(world.fog().start, fog.start * choice.multiplier());
        assert_eq!(world.fog().end, fog.end * choice.multiplier());
        assert_eq!(
            world.pose_reach().far,
            reach.far.map(|far| far * choice.multiplier())
        );
        assert_eq!(
            world.skin_frame(&frame).scroll[1],
            if choice == ViewDistance::Retail {
                0.0
            } else {
                world.fog().end
            }
        );
        assert_eq!(world.pose, pose);
        assert_eq!(world.effects.view_distance, choice);
        assert_eq!(world.actors.grass.field.view_distance, choice);
        assert_eq!(world.environment, original);
        assert_eq!(world.mesh, mesh);
        assert_eq!(world.terrain, terrain);
        assert_eq!(world.actors.window, collision);
        assert_eq!(world.actors.zone, (16, 16));
        assert_eq!(world.current, (16, 16));
        assert_eq!(world.force_load, None);
        assert_eq!(world.identity, identity);
        assert_eq!(world.terrain_revision, revision);

        let cached = world.projected;

        world.set_view_distance(choice);

        assert_eq!(world.projected, cached);
    }
}

#[test]
fn underwater_live_switch_preserves_water_fog_and_only_expands_projection() {
    let mut world = field_world();
    let sea = sea::Sea::new(
        &formats::world::objects::ObjRecord {
            kind: 2,
            x: 16.0,
            z: 16.0,
            y: 20.0,
            yaw: 0.0,
            p0: 16,
            p1: 16,
            extra: None,
        },
        (16, 16),
        SCENE_FIELD,
    )
    .unwrap();

    world.sea.seas.push(sea);

    assert!(world.camera_under_water());

    let fog = world.fog();
    let clear = world.clear_color();

    for choice in ViewDistance::ALL {
        world.set_view_distance(choice);

        let frame = world.frame_at(world.pose, 0, (800, 800));

        assert_eq!(world.fog(), fog);
        assert_eq!(world.clear_color(), clear);
        assert_eq!(
            frame.projection.far,
            world.environment.far_plane * choice.multiplier()
        );
        assert_eq!(world.skin_frame(&frame).scroll[1], 0.0);
    }
}

#[test]
fn world_distance_survives_lifecycle_transitions_through_scenes_without_a_world() {
    use crate::runtime::lifecycle::{SceneHost, SceneState};
    use crate::scene::{Scene, login_panels, server_select};

    struct Host(crate::runtime::state::RuntimeState);

    impl SceneHost for Host {
        fn state(&mut self) -> SceneState<'_> {
            SceneState {
                scene: &mut self.0.scene,
                retained: &mut self.0.retained,
                session: &mut self.0.net,
                ime: &mut self.0.ime,
                pointer: &mut self.0.input.pointer_state,
                audio: None,
                launcher_login: false,
                launcher_handoff: false,
            }
        }

        fn disable_host_ime(&mut self) {}

        fn describe_scene(&mut self, _: &Scene) {}

        fn upload_scene(&mut self) -> bool {
            false
        }

        fn game_time(&self) -> crate::clock::GameTime {
            crate::clock::GameTime::from_millis(0)
        }

        fn redraw(&mut self) {}
    }

    let server = |distance| {
        let mut world = field_world();
        let panels = login_panels::language_tests::login();

        world.scene = SCENE_LOGIN;

        world.set_view_distance(distance);

        Scene::ServerSelect(Box::new(server_select::ServerSelect {
            layout: panels.layout,
            time: panels.time,
            world,
            panels: Box::new(panels),
        }))
    };
    let mut host = Host(crate::runtime::state::RuntimeState::new(
        server(ViewDistance::Distant),
        crate::clock::Clock::frozen(0),
        Lcg::new(1),
    ));

    assert_eq!(host.0.retained.view_distance, ViewDistance::Distant);

    for distance in [
        ViewDistance::Distant,
        ViewDistance::Far,
        ViewDistance::Maximum,
        ViewDistance::FiveTimes,
        ViewDistance::SixTimes,
        ViewDistance::SevenTimes,
        ViewDistance::EightTimes,
        ViewDistance::Extended,
        ViewDistance::Retail,
    ] {
        host.0.scene.set_view_distance(distance);

        crate::runtime::lifecycle::replace(
            &mut host,
            Scene::LoginPanels(Box::new(login_panels::language_tests::login())),
        );

        assert_eq!(host.0.scene.view_distance(), None);
        assert_eq!(host.0.retained.view_distance, distance);

        crate::runtime::lifecycle::replace(&mut host, server(ViewDistance::Retail));

        assert_eq!(host.0.scene.view_distance(), Some(distance));
        assert_eq!(host.0.retained.view_distance, distance);

        let Scene::ServerSelect(scene) = &mut host.0.scene else {
            panic!("expected a server-selection world");
        };

        assert_eq!(
            scene.world.view_far_plane(),
            scene.world.environment.far_plane
        );
        assert_eq!(
            scene.world.fog(),
            scene.world.camera_environment().fog_at(CUTSCENE_DISTANCE)
        );

        scene.world.frame_at(scene.world.pose, 0, (800, 800));

        assert_eq!(
            scene.world.actors.grass.field.view_distance,
            ViewDistance::Retail
        );
    }
}

#[test]
#[ignore = "requires a compatible GPU adapter"]
fn every_distance_tier_reveals_farther_geometry_in_rendered_world_frames() {
    use render::world::{TerrainBatch, TerrainMaterial, TerrainRenderer, WorldVertex};

    const WIDTH: u32 = 256;
    const HEIGHT: u32 = 64;
    const DEPTHS: [f32; 9] = [32.0, 48.0, 65.0, 95.0, 130.0, 150.0, 180.0, 215.0, 250.0];

    let gpu = pollster::block_on(Gpu::open(Gpu::instance(), None)).unwrap();
    let validation = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut world = field_world();
    let pose = Pose {
        eye: glam::Vec3::ZERO,
        yaw: std::f32::consts::FRAC_PI_2,
        pitch: 0.0,
    };
    let half_y = (camera::FOV_FRACTION * camera::PI * 0.5).tan();
    let half_x = half_y * WIDTH as f32 / HEIGHT as f32;
    let mut vertices = Vec::new();
    let mut batches = Vec::new();

    for (column, depth) in DEPTHS.into_iter().enumerate() {
        let centre = -1.0 + (column as f32 + 0.5) * 2.0 / DEPTHS.len() as f32;
        let half_width = 0.5 / DEPTHS.len() as f32;
        let min = [
            (centre - half_width) * depth * half_x,
            -depth * half_y * 0.5,
            depth,
        ];
        let max = [
            (centre + half_width) * depth * half_x,
            depth * half_y * 0.5,
            depth,
        ];

        for pos in [min, [min[0], max[1], depth], [max[0], min[1], depth], max] {
            vertices.push(WorldVertex {
                pos,
                normal: [0.0; 3],
                diffuse: u32::MAX,
                uv0: [0.0; 2],
                uv1: [0.0; 2],
            });
        }

        batches.push(TerrainBatch {
            base_texture_id: 0,
            detail_texture_id: 0,
            material: TerrainMaterial::Base,
            first_quad: column as u32,
            quad_count: 1,
            bounds: render::cull::Aabb { min, max },
        });
    }

    world.mesh = TerrainMesh {
        vertices,
        batches,
        world: Matrix4::IDENTITY,
        normal_matrix: Matrix4::IDENTITY,
        zone: (16, 16),
    };

    world.environment.sky_color = 0;

    let mut renderer = TerrainRenderer::new(
        &gpu,
        render::COLOR_FORMAT,
        wgpu::TextureFormat::Depth32Float,
    );

    renderer.upload_mesh(&gpu, &world.mesh);

    let tiles = renderer.bind_null_tiles(&gpu);
    let target = render::Offscreen::new(&gpu, WIDTH, HEIGHT);
    let depth =
        render::DepthBuffer::with_format(&gpu, WIDTH, HEIGHT, wgpu::TextureFormat::Depth32Float);
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("view distance regression pixels"),
        size: u64::from(WIDTH * HEIGHT * 4),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut previous: Option<Vec<u8>> = None;

    for (tier, choice) in ViewDistance::ALL.into_iter().enumerate() {
        world.set_view_distance(choice);

        let mut frame = world.frame_at(pose, 0, (WIDTH, HEIGHT));

        frame.uniforms.light0_color = [0.0; 4];
        frame.uniforms.light1_color = [0.0; 4];
        frame.uniforms.global_ambient = [0.0; 4];
        frame.uniforms.material_emissive = [1.0; 4];

        renderer.prepare(&gpu, &world.mesh.batches, &tiles, &frame.uniforms, false);

        let mut encoder = gpu.device.create_command_encoder(&Default::default());

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("view distance regression"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(depth.attachment()),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            renderer.draw(&mut pass, &tiles);
        }

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(WIDTH * 4),
                    rows_per_image: Some(HEIGHT),
                },
            },
            wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
        );

        let submission = gpu.queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);

        slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap()
        });

        gpu.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(std::time::Duration::from_secs(30)),
            })
            .unwrap();

        receiver
            .recv_timeout(std::time::Duration::from_secs(30))
            .unwrap()
            .unwrap();

        let pixels = slice.get_mapped_range().unwrap().to_vec();

        readback.unmap();

        let sample = |column: usize| {
            let x = ((column as f32 + 0.5) * WIDTH as f32 / DEPTHS.len() as f32) as u32;

            ((HEIGHT / 2 * WIDTH + x) * 4) as usize
        };

        assert!(
            pixels[sample(tier)] > 50,
            "{choice:?} did not reveal depth {}: {}",
            DEPTHS[tier],
            pixels[sample(tier)]
        );

        if let Some(previous) = &previous {
            assert_eq!(
                previous[sample(tier)],
                0,
                "depth {} must be hidden in previous tier",
                DEPTHS[tier]
            );

            for column in 0..tier {
                assert!(pixels[sample(column)] >= previous[sample(column)]);
            }
        }

        for column in tier + 1..DEPTHS.len() {
            assert_eq!(
                pixels[sample(column)],
                0,
                "{choice:?} unexpectedly revealed depth {}",
                DEPTHS[column]
            );
        }

        previous = Some(pixels);
    }

    assert!(pollster::block_on(validation.pop()).is_none());
}
