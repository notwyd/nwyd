use super::{DEVICE_WORLD, Frame, ObjectScene};
use assets::GameDir;
use glam::Vec3;
use render::{
    Matrix4,
    mesh::{Cull, DrawParams, MeshDraw, MeshPass, ObjectUniforms, Stage1Blend},
    preset::ColorOp,
    world::Material,
};

#[derive(Debug, Clone, Copy)]
pub struct Recipe {
    pub mesh: u32,
    pub position: (i32, i32),
    pub angle: f32,
    pub scale: f32,
    pub color: u32,
    pub refine: i32,
    pub texture_bias: i32,
    pub style: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Refinement {
    pub texture: usize,
    pub blend: Stage1Blend,
    pub double: bool,
    pub tint: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct DrawState {
    pub pass: MeshPass,
    pub lighting: bool,
}

impl Recipe {
    pub fn refinement(self, color_key: bool) -> Option<Refinement> {
        if self.refine <= 0 && !(4..=8).contains(&self.style) {
            return None;
        }

        let tint = (116..=125).contains(&self.style);

        let base = match self.style {
            4 => 179,
            5 | 9 => 153,
            6 | 10 => 166,
            7 | 11 => 246,
            8 => 260,
            12 => 440,
            116..=125 => {
                [275, 288, 301, 314, 327, 340, 353, 366, 425, 392][usize::from(self.style - 116)]
                    - 1
            }
            _ => 233,
        };

        let texture = usize::try_from(self.refine.checked_add(base)?).ok()?;

        Some(Refinement {
            texture,
            tint,
            double: self.style == 4
                || (!tint && !(5..=8).contains(&self.style) && self.refine >= 7),
            blend: if self.style == 4 || tint {
                Stage1Blend::Modulate
            } else if color_key && !(5..=8).contains(&self.style) {
                Stage1Blend::ModulateAlphaAddColor
            } else {
                Stage1Blend::AddSmooth
            },
        })
    }

    pub fn world(self, viewport: (u32, u32), center: Vec3) -> Option<Matrix4> {
        let (width, height) = (viewport.0 as f32, viewport.1 as f32);

        if width == 0.0
            || height == 0.0
            || !self.scale.is_finite()
            || !self.angle.is_finite()
            || !center.is_finite()
        {
            return None;
        }

        let k = if width / height < 1.26 { 6.26 } else { 6.3 };

        let dx = self.position.0 as f32 / (width / k) - k * 0.5;

        let dy = self.position.1 as f32 / (height / 4.96) - 2.48;

        let scale = self.scale * 0.52;

        let r = Matrix4::rotation_yaw_pitch_roll(
            dx.atan2(50.0) + self.angle,
            dy.atan2(50.0) - std::f32::consts::FRAC_PI_2,
            0.0,
        );

        let m = Matrix4::multiply(&DEVICE_WORLD, &r);

        let m = Matrix4::multiply(&m, &Matrix4::scaling(scale, scale, scale));

        Some(Matrix4::multiply(
            &m,
            &Matrix4::translation(
                -dx - center.x * scale,
                -dy - center.z * scale,
                -center.y * scale,
            ),
        ))
    }

    pub fn material(self) -> Material {
        let rgb = [
            ((self.color >> 16) & 255) as f32 / 256.0,
            ((self.color >> 8) & 255) as f32 / 256.0,
            (self.color & 255) as f32 / 256.0,
        ];

        Material {
            diffuse: [rgb[0], rgb[1], rgb[2], 1.0],
            ambient: [0.0; 4],
            specular: [rgb[0], rgb[1], rgb[2], 1.0],
            emissive: [
                rgb[0] * 0.3 + 0.1,
                rgb[0] * 0.3 + 0.1,
                rgb[0] * 0.3 + 0.1,
                0.0,
            ],
            power: 0.0,
        }
    }
}

impl ObjectScene {
    pub fn load_ui_mesh(&mut self, game: &GameDir, recipe: Recipe) -> anyhow::Result<()> {
        self.load_by_id(game, &[recipe.mesh])?;

        self.load_texture_bias(game, recipe.mesh, recipe.texture_bias);

        if let Some(glow) = recipe.refinement(false) {
            self.load_effect_textures(game, &[glow.texture]);
        }

        Ok(())
    }

    pub fn append_ui_mesh(
        &self,
        out: &mut Frame,
        recipe: Recipe,
        viewport: (u32, u32),
        now: u32,
        state: DrawState,
    ) -> std::ops::Range<usize> {
        let start = out.draws.len();

        let Some(&index) = self.by_id_meshes.get(&recipe.mesh) else {
            return start..start;
        };

        let mesh = &self.meshes[index];

        let center = (Vec3::from(mesh.bounds.min) + Vec3::from(mesh.bounds.max)) * 0.5;

        let Some(world) = recipe.world(viewport, center) else {
            return start..start;
        };

        let color_key = mesh
            .subset_slots
            .first()
            .copied()
            .flatten()
            .and_then(|slot| self.registries.textures.get(slot))
            .is_some_and(|entry| {
                entry.format_flag == formats::texture::texture_list::FormatFlag::ColorKey
            });

        let params = DrawParams {
            material: recipe.material(),
            lighting: state.lighting,
            fog: false,
            color_op: ColorOp::Modulate,
            vertex_color: mesh.vertex_color,
            force_opaque: false,
            stage1: false,
        };

        let mut uniform = ObjectUniforms::new(world, &params, 0.0);

        if mesh.mesh.fvf == 0x112 {
            let [uv0, uv1] = super::super::house::uv_scroll(1, now);

            uniform.uv0 = uv0;

            uniform.uv1 = uv1;
        }

        let mut stage1 = None;

        if let Some(glow) = recipe.refinement(color_key) {
            stage1 = self
                .textures
                .iter()
                .position(|t| t.effect_table && t.slot == glow.texture);

            if stage1.is_some() {
                uniform.set_stage1(glow.blend, 0.0);

                uniform.refine[0] = if glow.double { 2.0 } else { 1.0 };

                if glow.tint {
                    uniform.set_ui_tint_refinement();
                }
            }
        }

        let object = out.objects.len() as u32;

        out.objects.push(uniform);

        let effect =
            formats::mesh::mesh_list::is_effect_dir(formats::mesh::mesh_list::split(&mesh.path).0);

        let alloc = &self.arena.allocs[mesh.alloc];

        for (i, subset) in alloc.subsets.iter().enumerate() {
            let texture = if recipe.texture_bias == 0 {
                mesh.subset_textures.get(i).copied().flatten()
            } else {
                mesh.subset_slots
                    .get(i)
                    .copied()
                    .flatten()
                    .and_then(|slot| slot.checked_add_signed(recipe.texture_bias as isize))
                    .and_then(|slot| {
                        self.textures
                            .iter()
                            .position(|t| t.effect_table == effect && t.slot == slot)
                    })
            };

            let (texture, object) = match texture {
                Some(texture) => (texture, object),
                None if effect => {
                    let mut null = uniform;

                    null.set_null_texture();

                    let n = out.objects.len() as u32;

                    out.objects.push(null);

                    (render::mesh::NULL_TEXTURE, n)
                }
                None => continue,
            };

            let cull = match state.pass {
                MeshPass::Opaque
                | MeshPass::Blended
                | MeshPass::Additive
                | MeshPass::Reflective => Cull::None,
                MeshPass::Silhouette => Cull::Cw,
                _ => Cull::Ccw,
            };

            out.draws.push(MeshDraw {
                pass: state.pass,
                cull,
                object,
                texture,
                stage1,
                base_vertex: alloc.base_vertex,
                subset: *subset,
            });
        }

        start..out.draws.len()
    }
}
