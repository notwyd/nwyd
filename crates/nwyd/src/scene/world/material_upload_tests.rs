use super::fog_tests::field_world;
use crate::frame::SceneRenderer;
use crate::scene::objects::BoundTexture;
use formats::texture::{MipLevel, PixelFormat, Texture, texture_list::FormatFlag};
use render::device::Gpu;
use std::sync::Arc;

#[test]
#[ignore = "requires a compatible graphics adapter"]
fn repeated_world_uploads_share_one_frame_budget_and_resume_without_residency_changes() {
    let gpu = pollster::block_on(Gpu::open(Gpu::instance(), None)).unwrap();
    let validation = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut renderer = SceneRenderer::new(&gpu, render::target::COLOR_FORMAT);
    let mut world = field_world();
    let source = Arc::new(Texture {
        width: 1024,
        height: 1024,
        format: PixelFormat::Rgba8Unorm,
        data: vec![255; 1024 * 1024 * 4],
        source_len: 16,
        mips: vec![MipLevel {
            width: 1024,
            height: 1024,
            offset: 0,
            len: 1024 * 1024 * 4,
        }],
    });
    let prepared = Arc::new(render::texture::PreparedMaterial::from_shared(
        Arc::clone(&source),
        render::texture::MipPolicy::Opaque,
    ));
    assert!(prepared.upload_bytes() > 2 * 1024 * 1024);
    world.objects.textures = (0..3)
        .map(|slot| BoundTexture {
            texture: Arc::clone(&source),
            prepared: Some(Arc::clone(&prepared)),
            pending: false,
            effect_table: false,
            slot,
            path: format!("synthetic/frame-budget-{slot}.wys"),
            flag: FormatFlag::None_,
            mip_policy_override: None,
        })
        .collect();

    for frame in 1..=3 {
        renderer.begin_upload_frame();
        renderer.upload_world(&gpu, &world);
        assert_eq!(renderer.material_upload_stats().uploads, frame);
        renderer.begin();
        renderer.upload_world(&gpu, &world);
        renderer.upload_world(&gpu, &world);
        let stats = renderer.material_upload_stats();
        assert_eq!(stats.uploads, frame);
        assert_eq!(stats.pending, 3 - frame);
        assert_eq!(stats.bytes, (frame * prepared.upload_bytes()) as u64);
        renderer.finish_upload_frame();
    }
    gpu.queue.submit([]);
    assert!(pollster::block_on(validation.pop()).is_none());
}
