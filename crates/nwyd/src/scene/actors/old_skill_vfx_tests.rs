use super::test_support::{add_model, empty_scene};
use super::*;
use crate::scene::{actor_effects, effects::BillboardScene, lod::EffectOptions};

fn actor_look() -> Look {
    Look {
        class: 0,
        helmet_item: 0,
        model: Some(0),
        variant: 0,
        parts: [PartRef::EMPTY; appearance::PARTS],
        part_color: [0; appearance::PARTS],
        part_tier: [0; appearance::PARTS],
        part_tint: [0; appearance::PARTS],
        costume: None,
        costume_body: 0,
        kingdom: 0,
        cape: None,
        mount: None,
        speed: 0.0,
        action: 0,
    }
}

fn actor_scene() -> ActorScene {
    let mut actors = empty_scene();

    let model = add_model(&mut actors, 0, &[texture_list::FormatFlag::None_]);

    let look = actor_look();

    actors.by_spec.insert(
        ModelSpec {
            model_id: 0,
            clip: ClipChoice::Action(IDLE_ACTION),
            parts: look.parts,
            variant: 0,
            costume_body: 0,
        },
        Some(model),
    );

    let doll = Doll {
        look,
        pet_item: 0,
        position: Vec3::ZERO,
        yaw: 0.0,
        scale: 1.0,
        hands: (0, 0),
        record: 0,
        entity: 17,
        name: "synthetic".into(),
        status: Default::default(),
        name_level: 0,
        level: 0,
        guild: 0,
        guild_name: String::new(),
        face: 0,
        affects: affect::NO_AFFECTS,
        pick_action: None,
        shop_title: String::new(),
        green_name: false,
    };

    let game = Arc::clone(&actors.effect_game);

    assert_eq!(actors.place_one(&game, &doll).unwrap(), Some(0));

    actors.actors[0].flags.set(0xB3B, 1);

    actors.actors[0].flags.set(0xB36, 1);

    actors.actors[0].skill_anchor = Some(Vec3::new(1.0, 2.0, 3.0));

    actors
}

#[test]
fn old_skill_effects_respect_secondary_quality_and_remain_independent_of_quality_toggles() {
    let mut actors = empty_scene();

    assert!(actors.secondary_effects_enabled());

    let mut options = EffectOptions::default();

    options.old_skill_vfx = false;

    actors.set_effect_options(options);

    assert!(!actors.secondary_effects_enabled());

    for _ in 0..4 {
        actors.toggle_effects(false);

        assert!(!actors.secondary_effects_enabled());
        assert!(!actors.effect_options.unwrap().old_skill_vfx);
    }

    options.old_skill_vfx = true;

    options.secondary = false;

    actors.set_effect_options(options);

    assert!(!actors.secondary_effects_enabled());

    options.secondary = true;

    actors.set_effect_options(options);

    assert!(actors.secondary_effects_enabled());
}

#[test]
fn actor_updates_propagate_old_skill_effect_preferences_in_both_directions() {
    let mut actors = empty_scene();

    let mut effects = BillboardScene::empty();

    let mut options = EffectOptions::default();

    for (now, enabled) in [(1000, false), (1100, true), (1200, false)] {
        options.old_skill_vfx = enabled;

        actors.set_effect_options(options);

        actor_effects::update(&mut actors, &mut effects, now);

        assert_eq!(effects.old_skill_vfx, enabled);
    }
}

#[test]
fn live_affects_toggle_without_poisoning_visibility_or_recreating_retained_sprites() {
    let mut actors = actor_scene();

    let mut effects = BillboardScene::empty();

    effects.scene_id = 30000;

    let mut options = EffectOptions::default();

    actor_effects::update(&mut actors, &mut effects, 1001);

    effects.update(1251, 0.0, 0.0);

    let initial_births: Vec<_> = effects.billboards.iter().map(|b| b.birth_ms).collect();

    assert_eq!(initial_births, vec![Some(1001); 2]);
    assert_eq!(effects.mesh_frame(Vec3::ZERO, 1251).len(), 10);

    for (now, enabled) in [(1300, false), (1400, true), (1500, false), (1600, true)] {
        options.old_skill_vfx = enabled;

        actors.set_effect_options(options);

        actors.actors[0].skill_anchor = Some(Vec3::new(2.0, 3.0, 4.0));

        actor_effects::update(&mut actors, &mut effects, now);

        effects.update(now, 0.0, 0.0);

        assert_eq!(effects.billboards.len(), 2);
        assert!(
            effects
                .billboards
                .iter()
                .all(|b| { b.ready && !b.held_disabled && b.position == Vec3::new(2.0, 4.2, 4.0) })
        );
        assert_eq!(
            effects
                .billboards
                .iter()
                .map(|b| b.birth_ms)
                .collect::<Vec<_>>(),
            initial_births
        );
        assert_eq!(
            effects.frame(Reach::at(Vec3::ZERO), now).unbound,
            if enabled { 2 } else { 0 }
        );
        assert_eq!(
            effects.mesh_frame(Vec3::ZERO, now).len(),
            if enabled { 10 } else { 0 }
        );
    }

    options.old_skill_vfx = false;

    actors.set_effect_options(options);

    actors.actors[0].flags.set(0xB3B, 0);

    actors.actors[0].flags.set(0xB36, 0);

    actor_effects::update(&mut actors, &mut effects, 2200);

    effects.update(2200, 0.0, 0.0);

    options.old_skill_vfx = true;

    actors.set_effect_options(options);

    actor_effects::update(&mut actors, &mut effects, 2300);

    effects.update(2300, 0.0, 0.0);

    assert!(effects.billboards.is_empty());
    assert!(effects.tree.is_empty());
}

#[test]
fn weapon_emission_restores_without_reapplying_affects_and_preserves_other_arms() {
    use crate::scene::aura::{Arm, Emitters, Frame};

    let mut actors = actor_scene();

    actors.actors[0].flags.set(0xB3B, 0);

    actors.actors[0].flags.set(0xB36, 0);

    let mut effects = BillboardScene::empty();

    effects.scene_id = 30000;

    actors.actors[0].swings.push(swing::Swing::new(
        0,
        appearance::Hand::Main,
        1,
        0,
        None,
        &actor_look(),
    ));

    let mut options = EffectOptions::default();

    let mut rng = Lcg::new(7);

    for arm in Arm::ALL {
        let recipe = Emitters {
            aura_mode: match arm {
                Arm::AuraLow => 1,
                Arm::AuraHigh => aura::AURA_MODE_SWING,
                _ => 0,
            },
            glow: arm == Arm::Glow,
            spark: arm == Arm::Spark,
            sanc: arm == Arm::Sanc,
            weapon_effect: i32::from(arm == Arm::WeaponEffect),
            skill: arm == Arm::Skill,
            panel: arm == Arm::Panel,
        };

        let hand = &mut actors.actors[0].swings[0];

        hand.emitters = recipe;

        hand.clock = Default::default();

        hand.swing_started_ms = 1000;

        for (now, enabled) in [(1200, true), (1500, false), (1800, true)] {
            options.old_skill_vfx = enabled;

            actors.set_effect_options(options);

            actor_effects::update(&mut actors, &mut effects, now);

            let hand = &mut actors.actors[0].swings[0];

            assert_eq!(hand.emitters, recipe, "actor update changed {arm:?}");

            let mut frame = Frame::default();

            hand.emit(
                aura::Detail::default(),
                enabled,
                now,
                &mut rng,
                &mut frame.sink(),
            );

            assert_eq!(hand.emitters, recipe, "emission changed {arm:?}");

            if enabled || matches!(arm, Arm::Skill | Arm::Glow | Arm::Sanc) {
                assert_eq!(frame.fired, vec![arm]);
                assert_eq!(
                    frame.spawned.len(),
                    match arm {
                        Arm::Glow | Arm::Spark => 2,
                        Arm::Sanc => 3,
                        _ => 1,
                    }
                );
            } else {
                assert!(frame.fired.is_empty());
                assert!(frame.spawned.is_empty());
            }
        }
    }
}

#[test]
fn hidden_weapon_swing_expires_without_resurrecting_on_enable() {
    let mut hand = swing::Swing::new(0, appearance::Hand::Main, 1, 0, None, &actor_look());

    hand.emitters.aura_mode = aura::AURA_MODE_SWING;

    hand.swing_started_ms = 1000;

    let mut rng = Lcg::new(7);

    for (now, enabled) in [(2600, false), (2700, true)] {
        let mut frame = aura::Frame::default();

        hand.emit(
            aura::Detail::default(),
            enabled,
            now,
            &mut rng,
            &mut frame.sink(),
        );

        assert!(frame.spawned.is_empty());
        assert!(frame.fired.is_empty());
        assert_eq!(hand.emitters.aura_mode, 0);
        assert_eq!(hand.swing_started_ms, 0);
    }
}

#[test]
fn retained_hand_sprites_hide_and_resume_without_recreation() {
    use crate::scene::objects::BoundTexture;
    use formats::texture::{MipLevel, PixelFormat, Texture};

    let mut actors = actor_scene();

    actors.actors[0].flags.set(0xB3B, 0);

    actors.actors[0].flags.set(0xB36, 0);

    let mut hand = swing::Swing::new(0, appearance::Hand::Main, 1, 0, None, &actor_look());

    hand.emitters.weapon_effect = 1;

    actors.actors[0].swings.push(hand);

    let mut effects = BillboardScene::empty();

    effects.scene_id = 30000;

    effects.push_texture(BoundTexture {
        mip_policy_override: None,
        pending: false,
        effect_table: true,
        slot: 56,
        path: "synthetic-hand-effect".into(),
        flag: texture_list::FormatFlag::None_,
        texture: Texture {
            width: 1,
            height: 1,
            format: PixelFormat::Rgba8Unorm,
            mips: vec![MipLevel {
                width: 1,
                height: 1,
                offset: 0,
                len: 4,
            }],
            data: vec![255; 4],
            source_len: 4,
        },
    });

    actor_effects::hand_sprites(&actors, &mut effects, 1000, true);

    effects.update(1100, 0.0, 0.0);

    let initial = effects.frame(Reach::at(Vec3::ZERO), 1100);

    assert_eq!(initial.billboards, 1);
    assert_eq!(effects.tree.len(), 1);

    let mut options = EffectOptions::default();

    for (now, enabled) in [(1200, false), (1300, true), (1400, false), (1500, true)] {
        options.old_skill_vfx = enabled;

        actors.set_effect_options(options);

        actor_effects::update(&mut actors, &mut effects, now);

        actor_effects::hand_sprites(&actors, &mut effects, now, false);

        effects.update(now, 0.0, 0.0);

        assert_eq!(actors.actors[0].swings[0].emitters.weapon_effect, 1);
        assert_eq!(effects.tree.len(), 1);

        let frame = effects.frame(Reach::at(Vec3::ZERO), now);

        if enabled {
            assert_eq!(frame, initial);
        } else {
            assert_eq!(frame.billboards, 0);
            assert!(frame.objects.is_empty());
        }
    }

    effects.old_skill_vfx = false;

    effects.scene_id = 0;

    assert_eq!(effects.frame(Reach::at(Vec3::ZERO), 1600), initial);

    effects.scene_id = 30000;

    actors.actors[0].swings[0].emitters.weapon_effect = 0;

    actor_effects::hand_sprites(&actors, &mut effects, 1700, false);

    effects.old_skill_vfx = true;

    assert!(effects.tree.is_empty());
    assert_eq!(effects.frame(Reach::at(Vec3::ZERO), 1800).billboards, 0);
}
