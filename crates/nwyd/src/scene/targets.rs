use super::actors::Actor;
use super::attack::MIDPOINT_CLASS;
use super::field::FieldScene;
use super::skill_fx::Pending;
use super::spawn::Statics;
use formats::data::skill_data::Skill;
use formats::net::attack::{self as wire, Attack};
use glam::Vec3;

pub const ADMITTED: [i16; 4] = [0x29, 0x1D, 0x21, 0x02];

pub const SPAWNING_SKILL: i16 = 0x21;

pub const FORCED_ACTION: u8 = 0xFE;

pub const FORCED_CAP: usize = 4;

pub const CEILING: usize = wire::HITS;

pub const IMPACT_ACTION: u8 = 0xFC;

pub const BOLT_MODE_TWO_ACTION: u8 = 0xFD;

pub const SPARK_LIFT: f32 = 5.6;

pub const SPARK_MS: i32 = 1000;

pub const SPARK_ARGB: (u32, u32) = (0xFF55_55FF, 0xFF22_2299);

pub const SPARK_SCALE: f32 = 1.0;

pub const SPARK_KIND: i32 = 5;

pub const ARM_FUSE_MS: u32 = 500;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Body {
    pub entity: u16,
    pub position: Vec3,
    pub class: i32,
    pub part0_mesh: u16,
    pub scale: f32,
}

impl Body {
    pub fn of(actor: &Actor) -> Body {
        Body {
            entity: actor.entity,
            position: actor.position,
            class: actor.class,
            part0_mesh: actor.parts[0].mesh as u16,
            scale: actor.scale,
        }
    }

    pub fn siege(&self) -> bool {
        self.class == MIDPOINT_CLASS && self.part0_mesh == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Spark {
    pub at: Vec3,
    pub owner: u16,
    pub owner_at: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Bolt {
    pub at: Vec3,
    pub mode: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Armed {
    pub body: u16,
    pub fire_at_ms: u32,
    pub skill: i16,
    pub variant: i16,
    pub target: u16,
    pub to: Vec3,
    pub to_body: u16,
}

impl Armed {
    pub fn misplaced(&self) -> bool {
        self.to_body != self.body
    }

    pub fn pending(&self) -> Pending {
        Pending {
            fire_at_ms: self.fire_at_ms,
            to: self.to,
            target: Some(self.target),
            skill: self.skill,
            variant: self.variant,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub enum Entry {
    Unvisited,
    Unresolved(u16),
    Caster,
    Declined,
    Spark(Spark),
    Bolt(Bolt),
    Armed(Armed),
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Plan {
    pub entries: [Entry; CEILING],
    pub visited: usize,
}

impl Plan {
    pub fn visited(&self) -> &[Entry] {
        &self.entries[..self.visited.min(CEILING)]
    }

    pub fn acted(&self) -> usize {
        let skipped = |e: &&Entry| {
            matches!(
                e,
                Entry::Unvisited | Entry::Unresolved(_) | Entry::Caster | Entry::Declined
            )
        };

        self.visited().iter().filter(|e| !skipped(e)).count()
    }

    pub fn describe(&self) -> Option<String> {
        if self.visited == 0 {
            return None;
        }

        let names: Vec<&str> = self
            .visited()
            .iter()
            .map(|entry| match entry {
                Entry::Unvisited => "unvisited",
                Entry::Unresolved(_) => "unresolved",
                Entry::Caster => "caster",
                Entry::Declined => "declined",
                Entry::Spark(_) => "EffectSpark",
                Entry::Bolt(_) => "SkillThunderBolt",
                Entry::Armed(_) => "armed",
            })
            .collect();

        Some(format!("{} entry(ies): {}", self.visited, names.join(", ")))
    }
}

pub fn admits(skill_id: i16) -> bool {
    ADMITTED.contains(&skill_id)
}

pub fn max_targets(statics: &Statics, skill_id: i16) -> i32 {
    let Ok(id) = usize::try_from(skill_id) else {
        return 0;
    };

    statics
        .skills
        .as_ref()
        .and_then(|s| s.get(id))
        .map_or(0, Skill::max_targets)
}

pub fn cap(action: u8, targets: i32) -> usize {
    match action == FORCED_ACTION {
        true => FORCED_CAP,
        false => targets.clamp(0, CEILING as i32) as usize,
    }
}

pub fn bolt_mode(action: u8) -> i32 {
    match action {
        FORCED_ACTION => 1,
        BOLT_MODE_TWO_ACTION => 2,
        _ => 0,
    }
}

pub fn mine(echo: bool, caster: u16, local_player: Option<u16>) -> bool {
    match echo {
        false => true,
        true => local_player == Some(caster),
    }
}

pub fn plan(
    attack: &Attack,
    targets: i32,
    caster: &Body,
    local_player: Option<u16>,
    now_ms: u32,
    resolve: impl Fn(u16) -> Option<Body>,
) -> Plan {
    let mut plan = Plan {
        entries: [Entry::Unvisited; CEILING],
        visited: 0,
    };

    if !admits(attack.skill_id) {
        return plan;
    }

    for i in 0..CEILING {
        match attack.action == FORCED_ACTION {
            true => {
                if i >= FORCED_CAP {
                    break;
                }
            }
            false => {
                if targets <= i as i32 {
                    break;
                }
            }
        }

        plan.visited = i + 1;

        plan.entries[i] = one(attack, caster, local_player, now_ms, &resolve, i);
    }

    plan
}

fn one(
    attack: &Attack,
    caster: &Body,
    local_player: Option<u16>,
    now_ms: u32,
    resolve: &impl Fn(u16) -> Option<Body>,
    entry: usize,
) -> Entry {
    let id = attack.hits[entry].entity;

    let Some(body) = resolve(id) else {
        return Entry::Unresolved(id);
    };

    if body.entity == caster.entity {
        return Entry::Caster;
    }

    if attack.skill_id != SPAWNING_SKILL {
        return Entry::Armed(arm(attack, caster, &body, now_ms));
    }

    if !mine(attack.echo, caster.entity, local_player) {
        return Entry::Declined;
    }

    match attack.action == IMPACT_ACTION {
        true => Entry::Spark(Spark {
            at: Vec3::new(
                caster.position.x,
                caster.scale * SPARK_LIFT + caster.position.y,
                caster.position.z,
            ),
            owner: body.entity,
            owner_at: body.position,
        }),
        false => Entry::Bolt(Bolt {
            at: body.position,
            mode: bolt_mode(attack.action),
        }),
    }
}

fn arm(attack: &Attack, caster: &Body, body: &Body, now_ms: u32) -> Armed {
    let (to, to_body) = match body.siege() {
        true => (
            Vec3::new(
                caster.position.x + (body.position.x - caster.position.x) * 0.5,
                body.position.y,
                (body.position.z - caster.position.z) * 0.5 + caster.position.z,
            ),
            caster.entity,
        ),
        false => (body.position, body.entity),
    };

    Armed {
        body: body.entity,
        fire_at_ms: now_ms.wrapping_add(ARM_FUSE_MS),
        skill: attack.skill_id,
        variant: i16::from(attack.variant),
        target: body.entity,
        to,
        to_body,
    }
}

#[derive(Debug, Clone, PartialEq)]

pub struct Raised {
    pub plan: Plan,
    pub bolts: usize,
    pub sparks: usize,
    pub objects: usize,
    pub sounds: Vec<i64>,
}

impl Raised {
    pub fn describe(&self) -> Option<String> {
        if self.bolts == 0 && self.sparks == 0 {
            return None;
        }

        Some(format!(
            "NWYD: target action — {} ThunderBolt(s), {} EffectSpark(s), {} object(s), {} sound \
             request(s)",
            self.bolts,
            self.sparks,
            self.objects,
            self.sounds.len(),
        ))
    }
}

pub fn raise(scene: &mut FieldScene, attack: &Attack, caster: usize, now_ms: u32) -> Raised {
    let context = super::effect_context::Construction::capture(
        scene,
        caster,
        attack.hits.first().map(|hit| hit.entity),
        &attack.hits,
    );

    raise_with(scene, attack, &context, now_ms)
}

pub fn raise_with(
    scene: &mut FieldScene,
    attack: &Attack,
    context: &super::effect_context::Construction,
    now_ms: u32,
) -> Raised {
    let local = context
        .viewer
        .is_some_and(|viewer| viewer.key == context.caster.key);

    if !super::attack::effect_admitted(attack, local) {
        return Raised {
            plan: Plan {
                entries: [Entry::Unvisited; CEILING],
                visited: 0,
            },
            bolts: 0,
            sparks: 0,
            objects: 0,
            sounds: Vec::new(),
        };
    }

    let targets = max_targets(&scene.statics, attack.skill_id);

    let body = |s: super::effect_context::Snapshot| Body {
        entity: s.key.entity,
        position: s.position,
        class: s.class,
        part0_mesh: s.part0_mesh as u16,
        scale: s.scale,
    };

    let caster_body = body(context.caster);

    let local_player = scene
        .player
        .and_then(|index| scene.world.actors.actors.get(index))
        .map(|actor| actor.entity);

    let plan = plan(
        attack,
        targets,
        &caster_body,
        local_player,
        now_ms,
        |entity| {
            context
                .victims
                .iter()
                .flatten()
                .find(|s| s.key.entity == entity)
                .copied()
                .map(body)
        },
    );

    let mut raised = Raised {
        plan,
        bolts: 0,
        sparks: 0,
        objects: 0,
        sounds: Vec::new(),
    };

    for entry in plan.visited().iter().copied() {
        match entry {
            Entry::Bolt(bolt) => register_bolt(scene, bolt, now_ms, &mut raised),
            Entry::Spark(spark) => register_spark(scene, spark, now_ms, &mut raised),
            Entry::Armed(armed) => arm_recipient(scene, armed),
            _ => {}
        }
    }

    raised
}

fn arm_recipient(scene: &mut FieldScene, armed: Armed) {
    let Some(index) = scene.world.actors.entity(armed.body) else {
        return;
    };

    let mut pending = armed.pending();

    if armed.misplaced() {
        let actor = &scene.world.actors.actors[index];

        pending.to = actor.pending.map_or(actor.position, |old| old.to);

        if let Some(caster) = scene.world.actors.entity(armed.to_body)
            && let Some(cast) = &mut scene.world.actors.actors[caster].pending
        {
            cast.to = armed.to;
        }
    }

    scene.world.actors.actors[index].pending = Some(pending);
}

fn register_spark(scene: &mut FieldScene, plan: Spark, now_ms: u32, raised: &mut Raised) {
    let tracked = super::spark::TrackedBody {
        entity: plan.owner,
        position: plan.owner_at,
    };

    let mut spark = super::spark::Spark::new(
        plan.at,
        Some(tracked),
        Vec3::ZERO,
        SPARK_ARGB.0,
        SPARK_ARGB.1,
        SPARK_MS as u32,
        SPARK_SCALE,
        0.0,
        now_ms,
        now_ms,
    );

    spark.target_key = scene
        .world
        .actors
        .entity(plan.owner)
        .map(|index| super::effect_context::ActorKey::of(&scene.world.actors.actors[index]));

    {
        let heights = &scene.world.actors.heights;

        let zones = scene
            .world
            .streamed
            .iter()
            .map(|streamed| &streamed.terrain)
            .collect();

        let ground = super::shade::Ground::new(&scene.world.terrain, heights, zones);

        spark.place_constructor_shades(&ground);
    }

    raised.sparks += 1;

    raised.objects += 1 + super::spark::SEGMENTS * 2;

    scene.world.effects.spawn(Box::new(spark));

    scene.world.play_sound(super::spark::SOUND);

    raised.sounds.push(super::spark::SOUND);
}

fn register_bolt(scene: &mut FieldScene, bolt: Bolt, now_ms: u32, raised: &mut Raised) {
    use super::skill::{poison, thunderbolt};

    let detail = match scene.world.actors.effects_detail.effects {
        true => 0,
        false => 1,
    };

    let mut built = thunderbolt::build_at(bolt.at, bolt.mode, now_ms);

    let poison_recipe = poison::Recipe {
        at: built.poison.at,
        argb: built.poison.argb.unwrap_or(0xFFFF_FFFF),
        count: built.poison.count,
        play_sound: built.poison.play_sound,
        style: built.poison.style,
    };

    let poison = poison::build(
        &poison_recipe,
        now_ms,
        &mut scene.world.actors.effect_rng,
        detail,
    );

    let poison_sound = poison.sound;

    let mut poison_nodes = Vec::new();

    poison::push(poison, &mut poison_nodes);

    {
        let heights = &scene.world.actors.heights;

        let mut zones: Vec<&formats::world::trn::Terrain> = vec![&scene.world.terrain];

        zones.extend(
            scene
                .world
                .streamed
                .iter()
                .map(|streamed| &streamed.terrain),
        );

        let ground = super::shade::Ground::new(&scene.world.terrain, heights, zones);

        built.place(&ground);
    }

    let nodes = built.into_nodes(poison_nodes);

    raised.bolts += 1;

    raised.objects += nodes.len();

    for node in nodes {
        scene.world.effects.spawn(node);
    }

    if let Some(sound) = poison_sound {
        scene.world.play_sound(sound);

        raised.sounds.push(sound);
    }

    let sound = i64::from(thunderbolt::SOUND);

    scene.world.play_sound(sound);

    raised.sounds.push(sound);
}
