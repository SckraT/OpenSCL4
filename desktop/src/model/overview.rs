//! The overview's blocks in order, each shown or hidden, as android/app/src/main/java/com/oppzippy/openscq30/ui/sound/
//! OverviewLayout.kt, stored the same way: block ids in order, hidden ones prefixed with "!".

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OverviewBlock {
    SoundMode,
    Equalizer,
    QuickPresets,
    Controls,
    Comfort,
    Spatial,
    Power,
    Connections,
    Case,
}

impl OverviewBlock {
    pub const ALL: [Self; 9] = [
        Self::SoundMode,
        Self::Equalizer,
        Self::QuickPresets,
        Self::Controls,
        Self::Comfort,
        Self::Spatial,
        Self::Power,
        Self::Connections,
        Self::Case,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::SoundMode => "soundMode",
            Self::Equalizer => "equalizer",
            Self::QuickPresets => "quickPresets",
            Self::Controls => "controls",
            Self::Comfort => "comfort",
            // the id stays "spatial" for layouts saved before the block covered all sound effects
            Self::Spatial => "spatial",
            Self::Power => "power",
            Self::Connections => "connections",
            Self::Case => "case",
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|block| block.id() == id)
    }
}

/// Ready-made sets of blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverviewPreset {
    Minimal,
    Standard,
    Full,
}

impl OverviewPreset {
    pub const ALL: [Self; 3] = [Self::Minimal, Self::Standard, Self::Full];
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverviewLayout {
    pub blocks: Vec<(OverviewBlock, bool)>,
}

impl Default for OverviewLayout {
    fn default() -> Self {
        Self::of(OverviewPreset::Standard)
    }
}

impl OverviewLayout {
    pub fn of(preset: OverviewPreset) -> Self {
        let visible = |block| match preset {
            OverviewPreset::Minimal => {
                matches!(block, OverviewBlock::SoundMode | OverviewBlock::Equalizer)
            }
            OverviewPreset::Standard => {
                !matches!(block, OverviewBlock::QuickPresets | OverviewBlock::Power)
            }
            OverviewPreset::Full => true,
        };
        Self {
            blocks: OverviewBlock::ALL
                .into_iter()
                .map(|block| (block, visible(block)))
                .collect(),
        }
    }

    pub fn visible_blocks(&self) -> Vec<OverviewBlock> {
        self.blocks
            .iter()
            .filter(|(_, visible)| *visible)
            .map(|(block, _)| *block)
            .collect()
    }

    pub fn with_visibility(&self, block: OverviewBlock, visible: bool) -> Self {
        let blocks = self
            .blocks
            .iter()
            .map(|&(b, v)| if b == block { (b, visible) } else { (b, v) })
            .collect();
        Self { blocks }
    }

    pub fn moved(&self, from: usize, to: usize) -> Self {
        if from >= self.blocks.len() || to >= self.blocks.len() {
            return self.clone();
        }
        let mut blocks = self.blocks.clone();
        let block = blocks.remove(from);
        blocks.insert(to, block);
        Self { blocks }
    }

    /// The preset this layout is exactly, if any.
    pub fn preset(&self) -> Option<OverviewPreset> {
        OverviewPreset::ALL
            .into_iter()
            .find(|preset| Self::of(*preset) == *self)
    }

    pub fn serialize(&self) -> String {
        let ids: Vec<String> = self
            .blocks
            .iter()
            .map(|(block, visible)| {
                if *visible {
                    block.id().to_string()
                } else {
                    format!("!{}", block.id())
                }
            })
            .collect();
        ids.join(",")
    }

    /// Unknown ids are skipped; blocks missing from the stored value (added later) keep their default place.
    pub fn parse(stored: Option<&str>) -> Self {
        let Some(stored) = stored.filter(|s| !s.trim().is_empty()) else {
            return Self::default();
        };
        let mut blocks: Vec<(OverviewBlock, bool)> = Vec::new();
        for token in stored.split(',') {
            let token = token.trim();
            let hidden = token.starts_with('!');
            if let Some(block) = OverviewBlock::from_id(token.trim_start_matches('!'))
                && !blocks.iter().any(|(b, _)| *b == block)
            {
                blocks.push((block, !hidden));
            }
        }
        let missing: Vec<_> = Self::default()
            .blocks
            .into_iter()
            .filter(|(block, _)| !blocks.iter().any(|(b, _)| b == block))
            .collect();
        blocks.extend(missing);
        Self { blocks }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visible(layout: &OverviewLayout, block: OverviewBlock) -> Option<bool> {
        layout
            .blocks
            .iter()
            .find(|(b, _)| *b == block)
            .map(|(_, v)| *v)
    }

    #[test]
    fn nothing_stored_is_the_standard_set() {
        assert_eq!(
            OverviewLayout::of(OverviewPreset::Standard),
            OverviewLayout::parse(None)
        );
        assert_eq!(
            Some(OverviewPreset::Standard),
            OverviewLayout::parse(Some("")).preset()
        );
        assert_eq!(
            Some(false),
            visible(&OverviewLayout::default(), OverviewBlock::QuickPresets)
        );
    }

    #[test]
    fn a_layout_survives_saving() {
        let layout = OverviewLayout::default()
            .moved(0, 3)
            .with_visibility(OverviewBlock::Case, false);
        assert_eq!(layout, OverviewLayout::parse(Some(&layout.serialize())));
    }

    #[test]
    fn unknown_blocks_are_skipped_and_new_ones_keep_their_default() {
        let layout = OverviewLayout::parse(Some("equalizer,!soundMode,removedBlock"));
        assert_eq!(
            vec![OverviewBlock::Equalizer, OverviewBlock::SoundMode],
            layout
                .blocks
                .iter()
                .take(2)
                .map(|(b, _)| *b)
                .collect::<Vec<_>>()
        );
        assert_eq!(Some(false), visible(&layout, OverviewBlock::SoundMode));
        assert_eq!(OverviewBlock::ALL.len(), layout.blocks.len());
        assert_eq!(Some(true), visible(&layout, OverviewBlock::Controls));
    }

    #[test]
    fn presets_show_their_blocks() {
        assert_eq!(
            vec![OverviewBlock::SoundMode, OverviewBlock::Equalizer],
            OverviewLayout::of(OverviewPreset::Minimal).visible_blocks()
        );
        assert_eq!(
            OverviewBlock::ALL.to_vec(),
            OverviewLayout::of(OverviewPreset::Full).visible_blocks()
        );
        assert_eq!(
            Some(OverviewPreset::Full),
            OverviewLayout::of(OverviewPreset::Full).preset()
        );
    }

    #[test]
    fn power_and_prompts_are_only_in_the_full_set() {
        assert_eq!(
            Some(false),
            visible(
                &OverviewLayout::of(OverviewPreset::Standard),
                OverviewBlock::Power
            )
        );
        assert_eq!(
            Some(true),
            visible(
                &OverviewLayout::of(OverviewPreset::Full),
                OverviewBlock::Power
            )
        );
        // a layout saved before the block existed gets it hidden, and is still the standard set
        let saved = OverviewLayout {
            blocks: OverviewLayout::default()
                .blocks
                .into_iter()
                .filter(|(b, _)| *b != OverviewBlock::Power)
                .collect(),
        };
        assert_eq!(
            Some(false),
            visible(
                &OverviewLayout::parse(Some(&saved.serialize())),
                OverviewBlock::Power
            )
        );
    }

    #[test]
    fn a_changed_layout_is_no_longer_a_preset() {
        assert_eq!(
            None,
            OverviewLayout::default()
                .with_visibility(OverviewBlock::Spatial, false)
                .preset()
        );
        assert_eq!(None, OverviewLayout::default().moved(0, 1).preset());
    }

    #[test]
    fn moving_outside_the_list_changes_nothing() {
        assert_eq!(
            OverviewLayout::default(),
            OverviewLayout::default().moved(0, 99)
        );
    }
}
