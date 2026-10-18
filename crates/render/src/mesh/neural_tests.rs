use super::*;

#[test]
fn material_masks_preserve_cutouts_and_effects_after_motion_history_settles() {
    let mut history = std::collections::HashMap::new();
    let frame = FrameUniforms::zeroed();
    for (pass, expected) in [
        (MeshPass::Opaque, 0.0),
        (MeshPass::Cutout, 0.25),
        (MeshPass::Sky, 1.0),
        (MeshPass::SolidEffect, 1.0),
        (MeshPass::Additive, 1.0),
    ] {
        let object = ObjectUniforms::new(
            Matrix4::IDENTITY,
            &DrawParams::static_object(false, false),
            0.0,
        );
        let draw = MeshDraw {
            pass,
            cull: Cull::None,
            object: 0,
            texture: 0,
            stage1: None,
            base_vertex: 0,
            subset: SubsetRange {
                start_index: 0,
                index_count: 3,
            },
        };
        history.clear();
        assert_eq!(
            motion_objects(&mut history, &frame, &[object], &[draw], &[])[0].history[0],
            1.0
        );
        assert_eq!(
            motion_objects(&mut history, &frame, &[object], &[draw], &[])[0].history[0],
            expected,
            "{pass:?}"
        );
    }
}
