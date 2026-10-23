use super::*;
use formats::texture::{MipLevel, PixelFormat, Texture};
use std::time::Duration;

fn texture(color: [u8; 4]) -> Arc<Texture> {
    Arc::new(Texture {
        width: 1,
        height: 1,
        format: PixelFormat::Rgba8Unorm,
        mips: vec![MipLevel {
            width: 1,
            height: 1,
            offset: 0,
            len: 4,
        }],
        data: color.to_vec(),
        source_len: 4,
    })
}

#[test]
#[ignore = "requires a graphics adapter"]
fn shared_scene_textures_upload_only_when_replaced() {
    let gpu = pollster::block_on(Gpu::open(Gpu::instance(), None)).expect("graphics adapter");
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut renderer = SceneRenderer::new(&gpu, render::target::COLOR_FORMAT);
    let key = TextureKey::Scene(309);
    let original = texture([255, 0, 0, 255]);
    let initial = renderer.upload_count();

    for _ in 0..128 {
        renderer.upload_shared(&gpu, key, &original, AlphaPolicy::Stored, "shared UI");
    }

    assert_eq!(renderer.upload_count(), initial + 1);
    assert_eq!(Arc::strong_count(&original), 1);

    let clone = Arc::clone(&original);
    renderer.upload_shared(&gpu, key, &clone, AlphaPolicy::Stored, "shared UI clone");
    assert_eq!(renderer.upload_count(), initial + 1);

    renderer.upload_shared(&gpu, key, &clone, AlphaPolicy::Opaque, "opaque UI");
    assert_eq!(renderer.upload_count(), initial + 2);
    assert_eq!(
        renderer.textures.get(&key).unwrap().alpha,
        AlphaPolicy::Opaque
    );

    let mut replacement = texture([0, 255, 0, 255]);
    renderer.upload_shared(
        &gpu,
        key,
        &replacement,
        AlphaPolicy::Stored,
        "replacement UI",
    );
    assert_eq!(renderer.upload_count(), initial + 3);

    Arc::make_mut(&mut replacement).data = vec![0, 0, 255, 255];
    renderer.upload_shared(&gpu, key, &replacement, AlphaPolicy::Stored, "updated UI");
    assert_eq!(renderer.upload_count(), initial + 4);

    renderer.upload(&gpu, key, &original, AlphaPolicy::Stored, "direct UI");
    renderer.upload_shared(&gpu, key, &replacement, AlphaPolicy::Stored, "restored UI");
    assert_eq!(renderer.upload_count(), initial + 6);

    let retired = Arc::downgrade(&replacement);
    drop(replacement);
    assert!(retired.upgrade().is_none());

    let replacement = texture([255, 255, 0, 255]);
    renderer.upload_shared(&gpu, key, &replacement, AlphaPolicy::Stored, "new scene UI");
    assert_eq!(renderer.upload_count(), initial + 7);

    assert!(renderer.textures.remove(&key));
    renderer.upload_shared(&gpu, key, &replacement, AlphaPolicy::Stored, "removed UI");
    assert_eq!(renderer.upload_count(), initial + 8);

    let mut recreated = SceneRenderer::new(&gpu, render::target::COLOR_FORMAT);
    let initial = recreated.upload_count();
    recreated.upload_shared(&gpu, key, &replacement, AlphaPolicy::Stored, "recreated UI");
    assert_eq!(recreated.upload_count(), initial + 1);

    let submission = gpu.queue.submit([]);
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(30)),
        })
        .unwrap();
    assert!(pollster::block_on(scope.pop()).is_none());
}
