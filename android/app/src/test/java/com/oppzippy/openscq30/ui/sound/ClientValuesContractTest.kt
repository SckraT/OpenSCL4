package com.oppzippy.openscq30.ui.sound

import com.oppzippy.openscq30.lib.bindings.clientEqualizerFrequency
import com.oppzippy.openscq30.lib.wrapper.Equalizer
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Test

class ClientValuesContractTest {
    @Test
    fun sharedValuesContract() {
        val text = requireNotNull(javaClass.getResource("/client_rules.json")).readText()
        val fixtures = Json.parseToJsonElement(text).jsonObject
        fixtures.getValue("equalizerInputs").jsonArray.forEach { item ->
            val case = item.jsonObject
            val equalizer = Equalizer(
                listOf(100u.toUShort()),
                case.getValue("fractionDigits").jsonPrimitive.int.toShort(),
                case.getValue("min").jsonPrimitive.int.toShort(),
                case.getValue("max").jsonPrimitive.int.toShort(),
            )
            val expected = case.getValue("value").takeUnless { it == JsonNull }?.jsonPrimitive?.int?.toShort()
            assertEquals(expected, parseEqualizerValue(case.getValue("text").jsonPrimitive.content, equalizer))
        }
        fixtures.getValue("equalizerFormats").jsonArray.forEach { item ->
            val case = item.jsonObject
            assertEquals(
                case.getValue("text").jsonPrimitive.content,
                formatEqualizerValue(
                    case.getValue("value").jsonPrimitive.int.toShort(),
                    case.getValue("fractionDigits").jsonPrimitive.int.toShort(),
                ),
            )
        }
        fixtures.getValue("equalizerFrequencies").jsonArray.forEach { item ->
            val case = item.jsonObject
            val frequency = clientEqualizerFrequency(case.getValue("hz").jsonPrimitive.int.toUShort())
            assertEquals(case.getValue("value").jsonPrimitive.content, frequency.value)
            assertEquals(case.getValue("khz").jsonPrimitive.boolean, frequency.khz)
        }
        fixtures.getValue("quickToggles").jsonArray.forEach { item ->
            val case = item.jsonObject
            val stored = case.getValue("stored").takeUnless { it == JsonNull }?.jsonArray
                ?.joinToString(",") { it.jsonPrimitive.content }
            val available = case.getValue("available").jsonArray.map { it.jsonPrimitive.content }
            val expected = case.getValue("expected").jsonArray.map { it.jsonPrimitive.content }
            assertEquals(expected, quickToggleIds(stored, available))
        }
    }
}
