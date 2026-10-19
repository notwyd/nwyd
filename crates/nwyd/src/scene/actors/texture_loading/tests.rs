use super::*;
use formats::texture::texture_list::FormatFlag;

fn game() -> GameDir {
    GameDir::from_files("synthetic-texture-loader", []).unwrap()
}

fn texture(value: u8) -> Texture {
    Texture {
        width: 2,
        height: 2,
        format: PixelFormat::Rgba8Unorm,
        mips: vec![MipLevel {
            width: 2,
            height: 2,
            offset: 0,
            len: 16,
        }],
        data: vec![value; 16],
        source_len: 38,
    }
}

fn bound(path: &str, slot: usize, pending: bool) -> BoundTexture {
    BoundTexture {
        pending,
        effect_table: false,
        slot,
        path: path.into(),
        flag: FormatFlag::Alpha,
        texture: if pending { placeholder() } else { texture(100) }.into(),
        mip_policy_override: None,
        prepared: None,
    }
}

fn controlled() -> (Loader, mpsc::Receiver<Request>, mpsc::SyncSender<Reply>) {
    let mut loader = Loader::new(&game(), true);
    let (requests, received) = mpsc::sync_channel(1);
    let (reply, replies) = mpsc::sync_channel(1);
    loader.worker = Some(Worker { requests, replies });
    (loader, received, reply)
}

fn take(requests: &mpsc::Receiver<Request>, index: usize, path: &str) -> Request {
    let request = requests.try_recv().unwrap();
    assert_eq!((request.index, request.path.as_str()), (index, path));
    request
}

fn loaded(request: Request, texture: Texture) -> Reply {
    let texture = Arc::new(texture);
    let prepared = Arc::new(PreparedMaterial::from_shared(
        Arc::clone(&texture),
        request.policy,
    ));
    (request, Ok(Loaded { texture, prepared }))
}

#[test]
fn placeholder_is_a_valid_transparent_single_pixel() {
    let placeholder = placeholder();
    assert_eq!((placeholder.width, placeholder.height), (1, 1));
    assert_eq!(placeholder.format, PixelFormat::Rgba8Unorm);
    assert_eq!(placeholder.data, [0; 4]);
    assert_eq!(placeholder.mips.len(), 1);
    assert_eq!(placeholder.sample_rgba8(0, 0), Some([0; 4]));
}

#[test]
fn loader_publishes_each_success_once_and_keeps_only_one_request_in_flight() {
    let (mut loader, requests, replies) = controlled();
    let mut textures = vec![bound("one.wyt", 1, true), bound("two.wyt", 2, true)];
    let mut updates = Vec::new();
    assert!(!loader.poll(&mut textures, &mut updates));
    let request = take(&requests, 0, "one.wyt");
    assert!(!loader.poll(&mut textures, &mut updates));
    assert!(requests.try_recv().is_err());
    assert!(updates.is_empty());
    let expected = texture(73);
    replies.send(loaded(request, expected.clone())).unwrap();
    assert!(loader.poll(&mut textures, &mut updates));
    assert_eq!(*textures[0].texture, expected);
    assert!(!textures[0].pending);
    assert_eq!(updates, [0]);
    take(&requests, 1, "two.wyt");
    assert!(!loader.poll(&mut textures, &mut updates));
    assert_eq!(updates, [0]);
}

#[test]
fn failure_keeps_the_placeholder_and_retries_after_its_cooldown() {
    let (mut loader, requests, replies) = controlled();
    let mut textures = vec![bound("missing.wyt", 1, true)];
    let mut updates = Vec::new();
    loader.poll(&mut textures, &mut updates);
    let request = take(&requests, 0, "missing.wyt");
    replies
        .send((
            request.clone(),
            Err(anyhow::anyhow!("synthetic missing texture")),
        ))
        .unwrap();
    let before = Instant::now();
    assert!(!loader.poll(&mut textures, &mut updates));
    assert!(textures[0].pending);
    assert_eq!(*textures[0].texture, placeholder());
    assert!(updates.is_empty());
    assert!(loader.retry[&0].1 >= before + Duration::from_millis(15_001));
    assert!(requests.try_recv().is_err());
    loader.retry.insert(0, (request, Instant::now()));
    assert!(!loader.poll(&mut textures, &mut updates));
    take(&requests, 0, "missing.wyt");
}

#[test]
fn a_reply_does_not_replace_a_texture_already_installed_by_another_path() {
    let (mut loader, requests, replies) = controlled();
    let mut textures = vec![bound("one.wyt", 1, true)];
    let mut updates = Vec::new();
    loader.poll(&mut textures, &mut updates);
    let request = take(&requests, 0, "one.wyt");
    textures[0].texture = texture(33).into();
    textures[0].pending = false;
    replies.send(loaded(request, texture(200))).unwrap();
    assert!(!loader.poll(&mut textures, &mut updates));
    assert_eq!(*textures[0].texture, texture(33));
    assert!(updates.is_empty());
}

#[test]
fn eviction_preserves_referenced_effect_and_reserved_textures() {
    let (mut loader, _, _) = controlled();
    let mut textures = vec![
        bound("idle.wyt", 1, false),
        bound("used.wyt", 2, false),
        bound("effect.wyt", 3, false),
        bound("reserved.wyt", 3048, false),
        bound("pending.wyt", 4, true),
    ];
    textures[2].effect_table = true;
    for texture in &mut textures {
        if !texture.pending {
            texture.prepared = Some(Arc::new(PreparedMaterial::new(
                &texture.texture,
                texture.mip_policy(true),
            )));
        }
    }
    let base = loader.sweep;
    loader.touched.insert(0, base);
    loader.touched.insert(1, base);
    let mut updates = Vec::new();
    loader.maintain(
        &mut textures,
        &mut updates,
        [1].into_iter(),
        base + Duration::from_secs(120),
    );
    assert!(updates.is_empty());
    loader.maintain(
        &mut textures,
        &mut updates,
        [1].into_iter(),
        base + Duration::from_secs(125),
    );
    assert_eq!(updates, [0]);
    assert_eq!(*textures[0].texture, placeholder());
    assert!(textures[0].pending);
    assert!(textures[0].prepared.is_none());
    assert_eq!(loader.evicted, BTreeSet::from([0]));
    for index in [1, 2, 3] {
        assert!(!textures[index].pending);
        assert_eq!(*textures[index].texture, texture(100));
        assert!(textures[index].prepared.is_some());
    }
}

#[test]
fn evicted_textures_reload_only_after_becoming_used_again() {
    let (mut loader, requests, _replies) = controlled();
    let mut textures = vec![bound("idle.wyt", 1, false)];
    let base = loader.sweep;
    loader.touched.insert(0, base);
    let mut updates = Vec::new();
    loader.maintain(
        &mut textures,
        &mut updates,
        [].into_iter(),
        base + Duration::from_secs(121),
    );
    assert!(loader.poll(&mut textures, &mut updates));
    assert!(requests.try_recv().is_err());
    loader.maintain(
        &mut textures,
        &mut updates,
        [0].into_iter(),
        base + Duration::from_secs(122),
    );
    assert!(!loader.poll(&mut textures, &mut updates));
    take(&requests, 0, "idle.wyt");
}

#[test]
fn a_disconnected_worker_releases_pending_work_and_delays_restart() {
    let (mut loader, requests, replies) = controlled();
    let mut textures = vec![bound("one.wyt", 1, true)];
    let mut updates = Vec::new();
    loader.poll(&mut textures, &mut updates);
    requests.try_recv().unwrap();
    drop(replies);
    let before = Instant::now();
    assert!(!loader.poll(&mut textures, &mut updates));
    assert!(loader.worker.is_none());
    assert!(loader.pending.is_none());
    assert!(loader.restart >= before + Duration::from_secs(1));
    assert!(textures[0].pending);
    assert!(loader.wanted.contains(&0));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn worker_decodes_a_synthetic_game_texture_and_reports_missing_files() {
    let mut bytes = b"WT10".to_vec();
    bytes.extend_from_slice(&[0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 2, 0, 32, 0x28]);
    bytes.extend_from_slice(&[11, 22, 33, 255].repeat(4));
    let expected = formats::texture::decode(&bytes).unwrap();
    let game =
        GameDir::from_files("synthetic-decoded-texture", [("one.wyt".into(), bytes)]).unwrap();
    let worker = Worker::new(game).unwrap();
    worker
        .requests
        .send(Request {
            index: 0,
            path: "one.wyt".into(),
            policy: MipPolicy::Blend,
            generation: 1,
        })
        .unwrap();
    let (index, result) = worker
        .replies
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(index.index, 0);
    let result = result.unwrap();
    assert_eq!(*result.texture, expected);
    assert_eq!(result.prepared.policy(), MipPolicy::Blend);
    assert_eq!(result.prepared.texture().mips.len(), 2);
    worker
        .requests
        .send(Request {
            index: 1,
            path: "missing.wyt".into(),
            policy: MipPolicy::Blend,
            generation: 2,
        })
        .unwrap();
    let (index, result) = worker
        .replies
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(index.index, 1);
    assert!(result.is_err());
}

#[test]
fn replies_for_replaced_paths_or_policies_are_discarded_and_requeued() {
    for changed_path in [false, true] {
        let (mut loader, requests, replies) = controlled();
        let mut textures = vec![bound("old.wyt", 1, true)];
        let mut updates = Vec::new();
        loader.poll(&mut textures, &mut updates);
        let old = take(&requests, 0, "old.wyt");
        assert_eq!(old.policy, MipPolicy::Blend);
        if changed_path {
            textures[0].path = "new.wyt".into();
        } else {
            textures[0].mip_policy_override = Some(MipPolicy::Cutout(170));
        }
        replies.send(loaded(old.clone(), texture(55))).unwrap();
        assert!(!loader.poll(&mut textures, &mut updates));
        assert_eq!(*textures[0].texture, placeholder());
        assert!(textures[0].prepared.is_none());
        assert!(updates.is_empty());
        let next = take(
            &requests,
            0,
            if changed_path { "new.wyt" } else { "old.wyt" },
        );
        assert_ne!(next.generation, old.generation);
        assert_eq!(next.policy, textures[0].mip_policy(true));
        let expected = texture(200);
        replies.send(loaded(next, expected.clone())).unwrap();
        assert!(loader.poll(&mut textures, &mut updates));
        assert_eq!(*textures[0].texture, expected);
        assert_eq!(
            textures[0].prepared.as_ref().unwrap().policy(),
            textures[0].mip_policy(true)
        );
        assert_eq!(updates, [0]);
    }
}

#[test]
fn removed_indices_and_reused_slots_cannot_receive_stale_work() {
    let (mut loader, requests, replies) = controlled();
    let mut textures = vec![bound("old.wyt", 1, true)];
    let mut updates = Vec::new();
    loader.poll(&mut textures, &mut updates);
    let old = take(&requests, 0, "old.wyt");
    textures.clear();
    replies.send(loaded(old.clone(), texture(55))).unwrap();
    assert!(!loader.poll(&mut textures, &mut updates));
    assert!(loader.pending.is_none());
    assert!(updates.is_empty());

    textures.push(bound("new.wyt", 2, true));
    loader.poll(&mut textures, &mut updates);
    let current = take(&requests, 0, "new.wyt");
    replies.send(loaded(old, texture(100))).unwrap();
    assert!(!loader.poll(&mut textures, &mut updates));
    assert_eq!(loader.pending.as_ref(), Some(&current));
    assert_eq!(*textures[0].texture, placeholder());
    replies.send(loaded(current, texture(200))).unwrap();
    assert!(loader.poll(&mut textures, &mut updates));
    assert_eq!(*textures[0].texture, texture(200));
    assert_eq!(updates, [0]);
}

#[test]
fn failure_cooldowns_do_not_delay_replacement_paths_or_policies() {
    let (mut loader, requests, replies) = controlled();
    let mut textures = vec![bound("old.wyt", 1, true)];
    let mut updates = Vec::new();
    loader.poll(&mut textures, &mut updates);
    let request = take(&requests, 0, "old.wyt");
    replies
        .send((request, Err(anyhow::anyhow!("synthetic failure"))))
        .unwrap();
    loader.poll(&mut textures, &mut updates);
    assert!(loader.retry.contains_key(&0));
    textures[0].path = "new.wyt".into();
    assert!(!loader.poll(&mut textures, &mut updates));
    take(&requests, 0, "new.wyt");
    assert!(!loader.retry.contains_key(&0));
}

#[test]
fn object_and_actor_loaders_request_their_actual_material_policy() {
    for skin in [false, true] {
        let (mut loader, requests, replies) = controlled();
        loader.skin = skin;
        let mut textures = vec![bound("shared.wyt", 1, true)];
        let mut updates = Vec::new();
        loader.poll(&mut textures, &mut updates);
        let request = take(&requests, 0, "shared.wyt");
        assert_eq!(request.policy, textures[0].mip_policy(skin));
        assert_eq!(
            request.policy,
            if skin {
                MipPolicy::Blend
            } else {
                MipPolicy::Cutout(170)
            }
        );
        let source = texture(180);
        replies.send(loaded(request, source.clone())).unwrap();
        assert!(loader.poll(&mut textures, &mut updates));
        assert_eq!(*textures[0].texture, source);
        let prepared = textures[0].prepared.as_ref().unwrap();
        assert_eq!(prepared.policy(), textures[0].mip_policy(skin));
        assert_eq!(prepared.texture().mips.len(), 2);
        assert_eq!(prepared.upload_bytes(), 20);
        assert_eq!(textures[0].texture.mips.len(), 1);
    }
}
