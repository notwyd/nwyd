use super::*;

fn prepared(zone: (u8, u8), serial: u64) -> Prepared {
    Prepared {
        serial,
        terrain: Terrain {
            name: "synthetic".into(),
            zone_x: zone.0,
            zone_y: zone.1,
            cells: Vec::new(),
        },
        mesh: TerrainMesh {
            vertices: Vec::new(),
            batches: Vec::new(),
            world: Matrix4::IDENTITY,
            normal_matrix: Matrix4::IDENTITY,
            zone,
        },
        records: Vec::new(),
        tiles: Vec::new(),
        resources: None,
    }
}

fn loader() -> (Loader, mpsc::Receiver<Request>, mpsc::SyncSender<Reply>) {
    let (requests, work) = mpsc::sync_channel(1);
    let (done, replies) = mpsc::sync_channel(1);
    (
        Loader {
            requests,
            replies,
            pending: None,
            ready: None,
            retry: HashMap::new(),
            disconnected: false,
        },
        work,
        done,
    )
}

#[test]
fn staging_retries_the_same_prepared_zone_without_reissuing_worker_reads() {
    let (mut loader, work, done) = loader();
    let zone = (17, 16);
    assert!(
        loader
            .take_or_stage(zone, &[], &[], || None, &mut |_| panic!("not ready"))
            .unwrap()
            .is_none()
    );
    assert_eq!(work.try_recv().unwrap().zone, zone);
    done.send((zone, Ok(Some(prepared(zone, 41))))).unwrap();
    for _ in 0..3 {
        assert!(
            loader
                .take_or_stage(zone, &[], &[], || None, &mut |prepared| {
                    assert_eq!(prepared.serial, 41);
                    false
                })
                .unwrap()
                .is_none()
        );
        assert!(work.try_recv().is_err());
    }
    let ready = loader
        .take_or_stage(zone, &[], &[], || None, &mut |_| true)
        .unwrap()
        .unwrap();
    assert_eq!(ready.serial, 41);
    assert!(loader.is_idle());
}

#[test]
fn a_new_zone_never_stages_the_old_workers_texture_payload() {
    let (mut loader, work, done) = loader();
    let old = (17, 16);
    let current = (17, 17);
    loader
        .take_or_stage(old, &[], &[], || None, &mut |_| true)
        .unwrap();
    assert_eq!(work.try_recv().unwrap().zone, old);
    done.send((old, Ok(Some(prepared(old, 41))))).unwrap();
    assert!(
        loader
            .take_or_stage(current, &[], &[], || None, &mut |_| panic!("stale zone"))
            .unwrap()
            .is_none()
    );
    assert_eq!(work.try_recv().unwrap().zone, current);
    done.send((current, Ok(Some(prepared(current, 42)))))
        .unwrap();
    let ready = loader
        .take_or_stage(current, &[], &[], || None, &mut |prepared| {
            assert_eq!(prepared.serial, 42);
            true
        })
        .unwrap()
        .unwrap();
    assert_eq!(ready.terrain.zone_y, current.1);
    assert!(loader.is_idle());
}

#[test]
fn discarding_a_staged_zone_releases_it_before_a_new_request() {
    let (mut loader, work, done) = loader();
    let zone = (17, 16);
    loader
        .take_or_stage(zone, &[], &[], || None, &mut |_| true)
        .unwrap();
    work.try_recv().unwrap();
    done.send((zone, Ok(Some(prepared(zone, 41))))).unwrap();
    loader
        .take_or_stage(zone, &[], &[], || None, &mut |_| false)
        .unwrap();
    assert!(loader.discard(zone).unwrap());
    assert!(loader.ready.is_none());
    assert!(work.try_recv().is_err());
    loader
        .take_or_stage((18, 16), &[], &[], || None, &mut |_| panic!("not ready"))
        .unwrap();
    assert_eq!(work.try_recv().unwrap().zone, (18, 16));
}
