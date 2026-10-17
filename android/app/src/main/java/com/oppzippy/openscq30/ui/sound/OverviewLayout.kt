package com.oppzippy.openscq30.ui.sound

import androidx.annotation.DrawableRes
import androidx.annotation.StringRes
import com.oppzippy.openscq30.R

/** A block of the device overview that can be shown, hidden and moved. "All settings" always stays at the bottom. */
enum class OverviewBlock(val id: String, @StringRes val title: Int, @DrawableRes val icon: Int) {
    SoundMode("soundMode", R.string.sound_overview_sound_mode, R.drawable.sound_noise),
    Equalizer("equalizer", R.string.equalizer, R.drawable.equalizer_24px),
    QuickPresets("quickPresets", R.string.quick_presets, R.drawable.bolt_24px),
    Controls("controls", R.string.sound_overview_controls, R.drawable.sound_hand),
    Comfort("comfort", R.string.sound_overview_comfort, R.drawable.sound_heart),
    // the id stays "spatial" for layouts saved before the block covered all sound effects
    Spatial("spatial", R.string.sound_overview_effects, R.drawable.speaker_24px),
    Power("power", R.string.sound_overview_power, R.drawable.power_settings_new_24px),
    Connections("connections", R.string.sound_overview_connections, R.drawable.devices_other_24px),
    Case("case", R.string.sound_overview_case, R.drawable.earbud_case_24px),
}

/** Ready-made sets of blocks. */
enum class OverviewPreset(@StringRes val title: Int) {
    Minimal(R.string.sound_overview_preset_minimal),
    Standard(R.string.sound_overview_preset_standard),
    Full(R.string.sound_overview_preset_full),
}

/** The overview's blocks in order, each shown or hidden. */
data class OverviewLayout(val blocks: List<Pair<OverviewBlock, Boolean>>) {
    val visibleBlocks: List<OverviewBlock> get() = blocks.filter { it.second }.map { it.first }

    fun withVisibility(block: OverviewBlock, visible: Boolean) =
        OverviewLayout(blocks.map { if (it.first == block) block to visible else it })

    fun moved(from: Int, to: Int): OverviewLayout {
        if (from !in blocks.indices || to !in blocks.indices) return this
        val list = blocks.toMutableList()
        list.add(to, list.removeAt(from))
        return OverviewLayout(list)
    }

    /** The preset this layout is exactly, if any. */
    val preset: OverviewPreset? get() = OverviewPreset.entries.firstOrNull { of(it) == this }

    /** Stored as block ids in order, hidden ones prefixed with "!". */
    fun serialize(): String = blocks.joinToString(",") { (block, visible) -> if (visible) block.id else "!${block.id}" }

    companion object {
        val Default = of(OverviewPreset.Standard)

        fun of(preset: OverviewPreset): OverviewLayout = OverviewLayout(
            OverviewBlock.entries.map { block ->
                block to when (preset) {
                    OverviewPreset.Minimal -> block == OverviewBlock.SoundMode || block == OverviewBlock.Equalizer
                    OverviewPreset.Standard -> block != OverviewBlock.QuickPresets && block != OverviewBlock.Power
                    OverviewPreset.Full -> true
                }
            },
        )

        /** Unknown ids are skipped; blocks missing from the stored value (added later) keep their default place. */
        fun parse(stored: String?): OverviewLayout {
            if (stored.isNullOrBlank()) return Default
            val parsed = stored.split(',').mapNotNull { token ->
                val hidden = token.startsWith("!")
                val id = token.removePrefix("!")
                OverviewBlock.entries.firstOrNull { it.id == id }?.let { it to !hidden }
            }.distinctBy { it.first }
            val missing = Default.blocks.filter { (block, _) -> parsed.none { it.first == block } }
            return OverviewLayout(parsed + missing)
        }
    }
}
