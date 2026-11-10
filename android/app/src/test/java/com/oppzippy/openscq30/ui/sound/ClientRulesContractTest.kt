package com.oppzippy.openscq30.ui.sound

import com.oppzippy.openscq30.lib.bindings.AvailabilityReason
import com.oppzippy.openscq30.lib.bindings.ClientSettingAvailability
import com.oppzippy.openscq30.lib.bindings.ClientSettingCategory
import com.oppzippy.openscq30.lib.bindings.clientAdaptiveNoiseCancelingDisplay
import com.oppzippy.openscq30.lib.bindings.clientGroupSettings
import com.oppzippy.openscq30.lib.bindings.clientSettingsPresentation
import com.oppzippy.openscq30.lib.bindings.clientVisibleSoundModeSettings
import com.oppzippy.openscq30.lib.wrapper.Equalizer
import com.oppzippy.openscq30.lib.wrapper.QuickPreset
import com.oppzippy.openscq30.lib.wrapper.QuickPresetField
import com.oppzippy.openscq30.lib.wrapper.Select
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.lib.wrapper.toValue
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.double
import kotlinx.serialization.json.float
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

private fun reason(value: JsonElement?): AvailabilityReason = when (value?.jsonPrimitive?.content) {
    "RequiresBothEarbuds" -> AvailabilityReason.REQUIRES_BOTH_EARBUDS
    "NoReadBack" -> AvailabilityReason.NO_READ_BACK
    else -> error("unknown availability reason $value")
}

class ClientRulesContractTest {
    @Test
    fun sharedClientContract() {
        val text = requireNotNull(javaClass.getResource("/client_rules.json")).readText()
        val fixtures = Json.parseToJsonElement(text).jsonObject
        fixtures.getValue("battery").jsonArray.forEach { item ->
            val case = item.jsonObject
            val actual = batteryProgress(case.getValue("raw").jsonPrimitive.content)
            if (case.getValue("progress") == JsonNull) {
                assertNull(actual)
            } else {
                assertEquals(case.getValue("progress").jsonPrimitive.float, requireNotNull(actual), 0.0001f)
            }
        }
        fixtures.getValue("groups").jsonArray.forEach { item ->
            val case = item.jsonObject
            val category = case.getValue("category").takeUnless { it == JsonNull }?.jsonPrimitive?.content
            assertEquals(
                case.getValue("group").jsonPrimitive.content,
                clientGroupSettings(
                    listOf(
                        ClientSettingCategory(category.orEmpty(), listOf(case.getValue("id").jsonPrimitive.content)),
                    ),
                ).single().group,
            )
        }
        fixtures.getValue("availability").jsonArray.forEach { item ->
            val scenario = item.jsonObject
            val values = buildMap {
                scenario.getValue("twsStatus").takeUnless { it == JsonNull }?.jsonPrimitive?.content?.let {
                    put("twsStatus", Setting.InformationSetting(it, it))
                }
                if (scenario["dualConnections"]?.jsonPrimitive?.boolean == true) {
                    put("dualConnections", Setting.ToggleSetting(true))
                }
            }
            val cases = scenario.getValue("cases").jsonArray.map { it.jsonObject }
            val availability = clientSettingsPresentation(
                values,
                cases.map { it.getValue("id").jsonPrimitive.content },
            ).availability
            cases.forEach { case ->
                val id = case.getValue("id").jsonPrimitive.content
                val reason = case.getValue("reason").takeUnless { it == JsonNull }
                val expected = when (case.getValue("status").jsonPrimitive.content) {
                    "Available" -> {
                        assertNull(reason)
                        ClientSettingAvailability.Available
                    }

                    "ReadOnly" -> ClientSettingAvailability.ReadOnly(reason(reason))

                    "Unavailable" -> ClientSettingAvailability.Unavailable(reason(reason))

                    else -> error("unknown availability $case")
                }
                assertEquals(id, expected, availability.getValue(id))
            }
        }
        fixtures.getValue("settingControls").jsonArray.forEach { item ->
            val scenario = item.jsonObject
            val values = scenario.getValue("settings").jsonObject.mapValues { (_, value) ->
                val primitive = value.jsonPrimitive
                if (primitive.isString) {
                    val mode = primitive.content
                    Setting.SelectSetting(Select(listOf(mode), listOf(mode)), mode)
                } else {
                    Setting.ToggleSetting(primitive.boolean)
                }
            }
            val cases = scenario.getValue("cases").jsonObject
            val controls = clientSettingsPresentation(values, cases.keys.toList()).controls
            cases.forEach { (id, expected) ->
                assertEquals(
                    "$id with settings $values",
                    expected.jsonPrimitive.content.lowercase(),
                    controls[id]?.name?.lowercase() ?: "enabled",
                )
            }
        }
        fixtures.getValue("isTws").jsonArray.forEach { item ->
            val case = item.jsonObject
            val values = case["twsStatus"]?.takeUnless { it == JsonNull }?.jsonPrimitive?.content
                ?.let { mapOf("twsStatus" to Setting.InformationSetting(it, it)) }
                .orEmpty()
            assertEquals(case.getValue("tws").jsonPrimitive.boolean, isTws(values))
        }
        fixtures.getValue("scale").jsonArray.forEach { item ->
            val case = item.jsonObject
            val values = soundModeScaleSettings(case.getValue("position").jsonPrimitive.int).toMap()
            assertEquals(case.getValue("mode").jsonPrimitive.content.toValue(), values.getValue("ambientSoundMode"))
            if (case.getValue("strengthId") == JsonNull) {
                assertEquals(1, values.size)
            } else {
                assertEquals(
                    case.getValue("strength").jsonPrimitive.int.toValue(),
                    values.getValue(case.getValue("strengthId").jsonPrimitive.content),
                )
            }
        }
        val eq = Equalizer(listOf(100u.toUShort()), fractionDigits = 1, min = -120, max = 134)
        fixtures.getValue("equalizer").jsonArray.forEach { item ->
            val case = item.jsonObject
            val profile = TransferProfile("p", listOf(case.getValue("value").jsonPrimitive.double))
            assertEquals(
                case.getValue("status").jsonPrimitive.content,
                previewImport(listOf(profile), emptyList(), eq).single().status.name,
            )
        }
        fixtures.getValue("presets").jsonArray.forEach { item ->
            val case = item.jsonObject
            val fields = case.getValue("enabled").jsonArray.zip(case.getValue("values").jsonArray) { enabled, value ->
                QuickPresetField("spatialAudio", value.jsonPrimitive.boolean.toValue(), enabled.jsonPrimitive.boolean)
            }
            val values = mapOf("spatialAudio" to Setting.ToggleSetting(case.getValue("current").jsonPrimitive.boolean))
            val lastApplied = case.getValue("lastApplied").takeUnless { it == JsonNull }?.jsonPrimitive?.content
            assertEquals(
                case.getValue("status").jsonPrimitive.content,
                soundPresetStatus(QuickPreset("p", fields), values, lastApplied).name,
            )
        }
        fun modeValues(case: Map<String, JsonElement>): Map<String, Setting> = buildMap {
            listOf("ambientSoundMode", "noiseCancelingMode", "transparencyMode").forEach { id ->
                val mode = case[id]?.takeUnless { it == JsonNull }?.jsonPrimitive?.content
                if (mode != null) put(id, Setting.SelectSetting(Select(listOf(mode), listOf(mode)), mode))
            }
        }
        fixtures.getValue("adaptiveLevel").jsonArray.forEach { item ->
            val case = item.jsonObject
            val values = modeValues(case) + ("adaptiveNoiseCanceling" to Setting.InformationSetting("1/5", "1/5"))
            assertEquals(
                case.getValue("shown").jsonPrimitive.boolean,
                clientAdaptiveNoiseCancelingDisplay(values) != null,
            )
        }
        val modeIds = listOf(
            "ambientSoundMode",
            "noiseCancelingMode",
            "customNoiseCanceling",
            "manualNoiseCanceling",
            "transparencyMode",
            "manualTransparency",
            "airplaneMode",
        )
        fixtures.getValue("soundModeRows").jsonArray.forEach { item ->
            val case = item.jsonObject
            val scale = case["scale"]?.jsonPrimitive?.boolean ?: false
            assertEquals(
                case.getValue("shown").jsonArray.map { it.jsonPrimitive.content },
                clientVisibleSoundModeSettings(modeValues(case), modeIds, scale),
            )
        }
    }
}
