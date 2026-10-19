package com.oppzippy.openscq30.ui.sound

import com.oppzippy.openscq30.lib.wrapper.Equalizer
import com.oppzippy.openscq30.lib.wrapper.QuickPreset
import com.oppzippy.openscq30.lib.wrapper.QuickPresetField
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.lib.wrapper.toValue
import kotlinx.serialization.json.Json
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
                settingGroup(case.getValue("id").jsonPrimitive.content, category).name,
            )
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
    }
}
