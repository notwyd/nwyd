#[derive(Clone, Copy)]
#[repr(usize)]

pub enum Shader {
    Sprite,
    CubeSprite,
    Blit,
    Terrain,
    TemporalTerrain,
    Mesh,
    TemporalMesh,
    Skin,
    TemporalSkin,
    Sea,
    TemporalSea,
    Billboard,
    TemporalBillboard,
}

pub const ALL: [Shader; 13] = [
    Shader::Sprite,
    Shader::CubeSprite,
    Shader::Blit,
    Shader::Terrain,
    Shader::TemporalTerrain,
    Shader::Mesh,
    Shader::TemporalMesh,
    Shader::Skin,
    Shader::TemporalSkin,
    Shader::Sea,
    Shader::TemporalSea,
    Shader::Billboard,
    Shader::TemporalBillboard,
];
