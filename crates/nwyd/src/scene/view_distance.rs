use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewDistance {
    #[default]
    Retail,
    Extended,
    Distant,
    Far,
    Maximum,
    FiveTimes,
    SixTimes,
    SevenTimes,
    EightTimes,
}

impl ViewDistance {
    pub const ALL: [Self; 9] = [
        Self::Retail,
        Self::Extended,
        Self::Distant,
        Self::Far,
        Self::Maximum,
        Self::FiveTimes,
        Self::SixTimes,
        Self::SevenTimes,
        Self::EightTimes,
    ];

    pub const fn multiplier(self) -> f32 {
        match self {
            Self::Retail => 1.0,
            Self::Extended => 1.5,
            Self::Distant => 2.0,
            Self::Far => 3.0,
            Self::Maximum => 4.0,
            Self::FiveTimes => 5.0,
            Self::SixTimes => 6.0,
            Self::SevenTimes => 7.0,
            Self::EightTimes => 8.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn higher_choices_restore_three_through_eight_times_view_distance() {
        for (name, multiplier) in [
            ("far", 3.0),
            ("maximum", 4.0),
            ("five_times", 5.0),
            ("six_times", 6.0),
            ("seven_times", 7.0),
            ("eight_times", 8.0),
        ] {
            let distance: ViewDistance = serde_json::from_str(&format!("\"{name}\"")).unwrap();
            assert_eq!(distance.multiplier(), multiplier);
            assert_eq!(
                serde_json::to_string(&distance).unwrap(),
                format!("\"{name}\"")
            );
            assert!(ViewDistance::ALL.contains(&distance));
        }
        assert_eq!(ViewDistance::ALL.len(), 9);
    }

    #[test]
    fn default_and_saved_choices_preserve_original_distance_tiers() {
        assert_eq!(ViewDistance::default(), ViewDistance::Retail);

        for (choice, name, multiplier) in [
            (ViewDistance::Retail, "retail", 1.0),
            (ViewDistance::Extended, "extended", 1.5),
            (ViewDistance::Distant, "distant", 2.0),
        ] {
            let encoded = serde_json::to_string(&choice).unwrap();

            assert_eq!(encoded, format!("\"{name}\""));
            assert_eq!(
                serde_json::from_str::<ViewDistance>(&encoded).unwrap(),
                choice
            );
            assert_eq!(choice.multiplier(), multiplier);
        }

        assert_eq!(
            ViewDistance::ALL.map(ViewDistance::multiplier),
            [1.0, 1.5, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]
        );
    }
}
