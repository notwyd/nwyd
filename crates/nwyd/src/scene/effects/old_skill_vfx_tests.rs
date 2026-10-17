use super::*;
use crate::scene::{
    actors::Reach,
    effect_context::{ActorKey, Snapshot},
    effect_tree::{MeshSubmission, Removal, RemovalContext},
    magic_shield,
};
use formats::texture::{MipLevel, PixelFormat, Texture};

fn body() -> Snapshot {
    Snapshot {
        key: ActorKey::synthetic(17),
        position: Vec3::new(1.0, 2.0, 3.0),
        yaw: 0.0,
        class: 0,
        model_id: 0,
        scale: 1.0,
        part0_mesh: 0,
        movement_origin: [1.0, 3.0],
        destination: (1, 3),
        appearance: None,
        dimensions: Some((1.0, 2.0)),
        skill_anchor: None,
        critical: false,
        skill_child: None,
        primary_tier: 0,
        affect_29: false,
        effects_hidden: false,
        appearance_epoch: 0,
        hand_glow_points: [None; 2],
    }
}

fn shields(body: &Snapshot) -> BillboardScene {
    let mut scene = BillboardScene::empty();

    scene.scene_id = 30000;

    for mode in 0..5 {
        magic_shield::refresh(&mut scene, body, mode, 1000);
    }

    scene.update(1250, 0.0, 0.0);

    scene
}

fn meditation_scene(key: ActorKey) -> BillboardScene {
    let mut scene = BillboardScene::empty();

    scene.scene_id = 30000;

    for slot in 109..117 {
        scene.push_texture(BoundTexture {
            pending: false,
            effect_table: true,
            slot,
            path: format!("synthetic-effect-{slot}"),
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
    }

    for slot in [Some(0x820), Some(0x824), None] {
        let mut sprite = Billboard::new(109, 0, Vec3::ONE, 0.0, 8, 80);

        sprite.birth_ms = Some(1000);

        sprite.body_owned = slot.map(|slot| (key, slot));

        scene.billboards.push(sprite);
    }

    scene.update(1250, 0.0, 0.0);

    scene
}

#[test]
fn default_skill_effects_preserve_every_shield_mesh_and_texture() {
    let scene = shields(&body());

    assert!(scene.old_skill_vfx);

    let meshes = scene.mesh_frame(Vec3::ZERO, 1250);

    assert_eq!(meshes.len(), 42);

    for (mode, texture) in [(0, 57), (1, 45), (2, 99), (3, 99)] {
        for (index, submission) in meshes[mode * 10..mode * 10 + 10].iter().enumerate() {
            let MeshSubmission::Textured(mesh) = submission else {
                panic!("shield must submit textured meshes");
            };

            assert_eq!(mesh.mesh_id, 704 + index as u32 % 2);
            assert_eq!(mesh.texture, texture);
            assert_eq!(mesh.blend, 1);
        }
    }

    for (submission, id) in meshes[40..].iter().zip([501, 502]) {
        let MeshSubmission::Textured(mesh) = submission else {
            panic!("shield must submit textured meshes");
        };

        assert_eq!(mesh.mesh_id, id);
        assert_eq!(mesh.texture, 202);
    }
}

#[test]
fn skill_effect_option_hides_selected_shields_only_in_the_field() {
    let mut scene = shields(&body());

    let original = scene.mesh_frame(Vec3::ZERO, 1250);

    scene.old_skill_vfx = false;

    let expected: Vec<_> = original[10..20]
        .iter()
        .chain(&original[40..])
        .copied()
        .collect();

    assert_eq!(scene.mesh_frame(Vec3::ZERO, 1250), expected);
    assert_eq!(scene.tree.len(), 5);

    for scene_id in [0, 10000, 20000] {
        scene.scene_id = scene_id;

        assert_eq!(scene.mesh_frame(Vec3::ZERO, 1250), original);
    }

    scene.scene_id = 30000;

    scene.old_skill_vfx = true;

    assert_eq!(scene.mesh_frame(Vec3::ZERO, 1250), original);
}

#[test]
fn hidden_shields_keep_the_current_phase_and_expire_without_replaying() {
    let body = body();

    let mut hidden = shields(&body);

    let mut reference = shields(&body);

    hidden.old_skill_vfx = false;

    hidden.update(1600, 0.0, 0.0);

    reference.update(1600, 0.0, 0.0);

    hidden.old_skill_vfx = true;

    assert_eq!(
        hidden.mesh_frame(Vec3::ZERO, 1600),
        reference.mesh_frame(Vec3::ZERO, 1600)
    );

    hidden.old_skill_vfx = false;

    hidden.update(2001, 0.0, 0.0);

    hidden.old_skill_vfx = true;

    assert!(hidden.tree.is_empty());
    assert!(hidden.mesh_frame(Vec3::ZERO, 2001).is_empty());
}

#[test]
fn removing_a_hidden_owner_cannot_resurrect_its_shield() {
    let body = body();

    let mut scene = shields(&body);

    scene.old_skill_vfx = false;

    scene.tree.remove_where(
        Removal::Forced,
        &RemovalContext { now_ms: 1300, detail: 0, ground: None },
        |node| matches!(node.owned_group(), Some(OwnedGroup::HumanAffect(key, _)) if key == body.key),
    );

    scene.old_skill_vfx = true;

    assert!(scene.tree.is_empty());
    assert!(scene.mesh_frame(Vec3::ZERO, 1300).is_empty());

    let replacement = Snapshot {
        key: ActorKey::synthetic(body.key.entity),
        ..body
    };

    assert_ne!(replacement.key, body.key);
    assert_eq!(magic_shield::refresh(&mut scene, &replacement, 0, 1300), 1);

    scene.update(1550, 0.0, 0.0);

    assert_eq!(scene.mesh_frame(Vec3::ZERO, 1550).len(), 10);
}

#[test]
fn meditation_toggle_preserves_unrelated_effects_using_the_same_texture() {
    let mut scene = meditation_scene(ActorKey::synthetic(17));

    let reach = Reach::at(Vec3::ZERO);

    let original = scene.frame(reach, 1250);

    assert_eq!(original.billboards, 3);
    assert_eq!(original.unbound, 0);

    scene.old_skill_vfx = false;

    let hidden = scene.frame(reach, 1250);

    assert_eq!(hidden.billboards, 1);
    assert_eq!(hidden.culled, 2);
    assert_eq!(hidden.objects, original.objects[2..]);
    assert_eq!(scene.billboards.len(), 3);
    assert!(scene.billboards.iter().all(|sprite| !sprite.held_disabled));

    scene.old_skill_vfx = true;

    assert_eq!(scene.frame(reach, 1250), original);

    scene.old_skill_vfx = false;

    scene.scene_id = 20000;

    assert_eq!(scene.frame(reach, 1250), original);
}

#[test]
fn hidden_meditation_advances_animation_and_survives_repeated_toggles() {
    let key = ActorKey::synthetic(17);

    let mut hidden = meditation_scene(key);

    let mut reference = meditation_scene(key);

    let reach = Reach::at(Vec3::ZERO);

    for now in [1400, 1600, 1800] {
        hidden.old_skill_vfx = false;

        for scene in [&mut hidden, &mut reference] {
            for sprite in &mut scene.billboards {
                sprite.position.x += 1.0;
            }

            scene.update(now, 0.25, 0.5);
        }

        assert_eq!(hidden.frame(reach, now).billboards, 1);

        hidden.old_skill_vfx = true;

        assert_eq!(hidden.billboards, reference.billboards);
        assert_eq!(hidden.frame(reach, now), reference.frame(reach, now));
    }

    hidden.old_skill_vfx = false;

    hidden
        .billboards
        .retain(|sprite| sprite.body_owned.is_none());

    hidden.old_skill_vfx = true;

    assert_eq!(hidden.frame(reach, 1800).billboards, 1);
}
