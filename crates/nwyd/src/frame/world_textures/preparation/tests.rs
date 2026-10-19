use super::*;
use formats::texture::{MipLevel, PixelFormat};
use render::texture::MipPolicy;

fn source(value: u8) -> Arc<Texture> {
    Arc::new(Texture {
        width: 2,
        height: 2,
        format: PixelFormat::Rgba8Unorm,
        data: [value, 180, 60, 128].repeat(4),
        source_len: 16,
        mips: vec![MipLevel {
            width: 2,
            height: 2,
            offset: 0,
            len: 16,
        }],
    })
}

fn key(policy: MipPolicy) -> Key {
    (true, "synthetic/material.wyt".into(), false, policy)
}

fn controlled() -> (
    Preparation,
    mpsc::Receiver<Request>,
    mpsc::SyncSender<Reply>,
) {
    let (requests, received) = mpsc::sync_channel(1);
    let (reply, replies) = mpsc::sync_channel(1);
    let preparation = Preparation {
        worker: Some(Worker { requests, replies }),
        ..Default::default()
    };

    (preparation, received, reply)
}

fn complete(request: Request) -> Reply {
    let material = Arc::new(PreparedMaterial::from_shared(request.source, request.key.3));

    (request.key, request.generation, material)
}

#[test]
fn requests_keep_source_intact_and_wait_for_worker_before_publishing() {
    let (mut preparation, requests, replies) = controlled();
    let key = key(MipPolicy::Blend);
    let source = source(40);

    assert!(preparation.request(&key, &source));
    assert!(preparation.get(&key).is_none());

    let request = requests.try_recv().unwrap();

    assert_eq!(request.source, source);
    assert!(Arc::ptr_eq(&request.source, &source));
    assert_eq!(request.key, key);
    assert!(preparation.request(&key, &source));
    assert!(requests.try_recv().is_err());

    replies.send(complete(request)).unwrap();
    preparation.poll();

    let ready = preparation.get(&key).unwrap();

    assert_eq!(ready.policy(), MipPolicy::Blend);
    assert_eq!(ready.texture().mips.len(), 2);
    assert_eq!(source.mips.len(), 1);
    assert!(preparation.request(&key, &source));
    assert!(requests.try_recv().is_err());
    assert!(Arc::ptr_eq(&ready, &preparation.take(&key).unwrap()));
    assert!(preparation.get(&key).is_none());
}

#[test]
fn queued_requests_and_completed_materials_stay_bounded() {
    let (mut preparation, requests, replies) = controlled();
    let first = key(MipPolicy::Blend);
    let second = key(MipPolicy::Cutout(170));

    assert!(preparation.request(&first, &source(1)));
    assert!(!preparation.request(&second, &source(2)));

    let request = requests.try_recv().unwrap();

    assert!(requests.try_recv().is_err());
    assert!(!preparation.request(&second, &source(2)));
    replies.send(complete(request)).unwrap();
    preparation.poll();
    assert!(!preparation.request(&second, &source(2)));
    assert_eq!(preparation.ready.len(), 1);
    preparation.take(&first).unwrap();
    assert!(preparation.request(&second, &source(2)));

    let request = requests.try_recv().unwrap();

    replies.send(complete(request)).unwrap();
    preparation.poll();
    assert_eq!(
        preparation.get(&second).unwrap().policy(),
        MipPolicy::Cutout(170)
    );
    assert!(preparation.get(&first).is_none());
}

#[test]
fn invalidated_work_cannot_publish_over_replacement_source() {
    let (mut preparation, requests, replies) = controlled();
    let key = key(MipPolicy::Opaque);

    preparation.request(&key, &source(1));

    let stale = requests.try_recv().unwrap();

    preparation.invalidate(&key);
    assert!(!preparation.request(&key, &source(2)));
    replies.send(complete(stale)).unwrap();
    preparation.poll();
    assert!(preparation.get(&key).is_none());
    assert!(preparation.request(&key, &source(2)));

    let replacement = requests.try_recv().unwrap();

    assert_eq!(replacement.source, source(2));
    replies.send(complete(replacement)).unwrap();
    preparation.poll();
    assert_eq!(preparation.get(&key).unwrap().texture().data[0], 2);
}

#[test]
fn unrelated_invalidation_retains_completed_materials() {
    let (mut preparation, requests, replies) = controlled();
    let first = key(MipPolicy::Blend);
    let other = key(MipPolicy::Opaque);

    preparation.request(&first, &source(1));
    replies
        .send(complete(requests.try_recv().unwrap()))
        .unwrap();
    preparation.poll();
    preparation.invalidate(&other);
    assert!(preparation.get(&first).is_some());
    preparation.invalidate(&first);
    assert!(preparation.get(&first).is_none());
}

#[test]
fn unrelated_invalidation_does_not_restart_active_work() {
    let (mut preparation, requests, replies) = controlled();
    let active = key(MipPolicy::Blend);
    let other = key(MipPolicy::Cutout(170));

    preparation.request(&active, &source(1));

    let request = requests.try_recv().unwrap();

    preparation.invalidate(&other);
    assert!(preparation.request(&active, &source(1)));
    assert!(requests.try_recv().is_err());
    replies.send(complete(request)).unwrap();
    preparation.poll();
    assert!(preparation.get(&active).is_some());
}

#[test]
fn wrong_identity_and_policy_replies_are_rejected() {
    let (mut preparation, requests, replies) = controlled();
    let key = key(MipPolicy::Blend);

    preparation.request(&key, &source(1));

    let request = requests.try_recv().unwrap();
    let mut stale = complete(Request {
        key: request.key.clone(),
        generation: request.generation.wrapping_sub(1),
        source: request.source.clone(),
    });

    replies.send(stale.clone()).unwrap();
    preparation.poll();
    assert!(preparation.pending.is_some());
    assert!(preparation.get(&key).is_none());

    stale.0.1 = "synthetic/other.wyt".into();
    stale.1 = request.generation;
    replies.send(stale).unwrap();
    preparation.poll();
    assert!(preparation.pending.is_some());
    assert!(preparation.ready.is_empty());

    let wrong = Arc::new(PreparedMaterial::new(&request.source, MipPolicy::Opaque));

    replies
        .send((request.key, request.generation, wrong))
        .unwrap();
    preparation.poll();
    assert!(preparation.pending.is_none());
    assert!(preparation.ready.is_empty());
    assert!(preparation.request(&key, &source(2)));
}

#[test]
fn clear_disconnects_old_work_and_discards_completed_materials() {
    let (mut preparation, requests, replies) = controlled();
    let key = key(MipPolicy::Blend);

    preparation.request(&key, &source(1));

    let old = requests.try_recv().unwrap();

    preparation.clear();
    assert!(replies.send(complete(old)).is_err());
    assert!(matches!(
        requests.try_recv(),
        Err(mpsc::TryRecvError::Disconnected)
    ));
    assert!(preparation.pending.is_none());
    assert!(preparation.worker.is_none());
    assert!(preparation.ready.is_empty());

    preparation.ready.insert(
        key.clone(),
        Arc::new(PreparedMaterial::new(&source(2), key.3)),
    );
    preparation.clear();
    assert!(preparation.get(&key).is_none());
}

#[test]
fn disconnected_workers_wait_before_retrying() {
    let (mut preparation, requests, replies) = controlled();
    let key = key(MipPolicy::Blend);

    preparation.request(&key, &source(1));
    drop(requests);
    drop(replies);

    let before = Instant::now();

    preparation.poll();
    assert!(preparation.worker.is_none());
    assert!(preparation.pending.is_none());
    assert!(preparation.restart >= before + Duration::from_secs(1));
    assert!(!preparation.request(&key, &source(2)));
    assert!(preparation.worker.is_none());
}

#[test]
fn native_worker_prepares_the_requested_policy() {
    let worker = Worker::new().unwrap();
    let key = key(MipPolicy::Cutout(170));
    let original = source(75);

    worker
        .requests
        .send(Request {
            key: key.clone(),
            generation: 7,
            source: original.clone(),
        })
        .unwrap();

    let (completed_key, generation, material) =
        worker.replies.recv_timeout(Duration::from_secs(5)).unwrap();

    assert_eq!(completed_key, key);
    assert_eq!(generation, 7);
    assert_eq!(material.policy(), key.3);
    assert_eq!(
        material.texture(),
        &PreparedMaterial::new(&original, key.3).texture().clone()
    );
    assert_eq!(original.mips.len(), 1);
}
