package com.oppzippy.openscq30.features.soundcoredevice.service

import android.app.Notification
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.graphics.drawable.Icon
import com.oppzippy.openscq30.MainActivity
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.lib.bindings.translateDeviceModel
import com.oppzippy.openscq30.lib.bindings.translateSettingId
import com.oppzippy.openscq30.lib.bindings.translateValue
import dagger.hilt.android.scopes.ServiceScoped
import javax.inject.Inject

internal fun ConnectionStatus.offersRetryAction(): Boolean = this is ConnectionStatus.Lost

@ServiceScoped
class NotificationBuilder @Inject constructor(private val context: Service) {
    operator fun invoke(
        status: ConnectionStatus,
        quickPresetNames: List<String?>,
        featuredSettingIds: List<String?>,
    ): Notification {
        val openAppIntent = Intent(context, MainActivity::class.java)
        openAppIntent.addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)

        val device = if (status is ConnectionStatus.Connected) {
            status.deviceManager.device
        } else {
            null
        }

        val builder = Notification.Builder(context, DeviceService.NOTIFICATION_CHANNEL_ID).setOngoing(true)
            .setOnlyAlertOnce(true).setSmallIcon(R.drawable.openscq30_logo_foreground).setContentTitle(
                when (status) {
                    is ConnectionStatus.AwaitingConnection -> context.getString(R.string.awaiting_connection)

                    is ConnectionStatus.Connected -> context.getString(
                        R.string.connected_to,
                        translateDeviceModel(status.deviceManager.device.model()),
                    )

                    is ConnectionStatus.Connecting -> context.getString(R.string.connecting_to, status.macAddress)

                    is ConnectionStatus.Recovering -> context.getString(
                        R.string.reconnecting_to,
                        translateDeviceModel(status.deviceManager.device.model()),
                    )

                    is ConnectionStatus.Lost -> context.getString(R.string.sound_overview_disconnected)

                    ConnectionStatus.Disconnected -> context.getString(R.string.disconnected)
                },
            ).setContentText(
                when (status) {
                    is ConnectionStatus.Recovering -> context.getString(status.reason.textResource())

                    is ConnectionStatus.Lost -> context.getString(status.reason.textResource())

                    else -> featuredSettingIds.takeIf { device != null }?.let { featuredSettingIds ->
                        featuredSettingIds.filterNotNull()
                            .mapNotNull { settingId ->
                                device?.setting(settingId)?.let { setting -> Pair(settingId, setting) }
                            }
                            .joinToString(separator = "\n") { (settingId, setting) ->
                                context.getString(
                                    R.string.setting_id_with_value,
                                    translateSettingId(settingId),
                                    translateValue(setting, setting.toValue()),
                                )
                            }
                    }
                },
            ).setContentIntent(
                PendingIntent.getActivity(
                    context,
                    1,
                    openAppIntent,
                    PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
                ),
            )

        if (status.offersRetryAction()) {
            builder.addAction(
                Notification.Action.Builder(
                    Icon.createWithResource(context, R.drawable.update_24px),
                    context.getString(R.string.sound_overview_retry),
                    PendingIntent.getBroadcast(
                        context,
                        3,
                        Intent().apply {
                            action = DeviceService.ACTION_RETRY
                            `package` = context.packageName
                        },
                        PendingIntent.FLAG_IMMUTABLE,
                    ),
                ).build(),
            )
        }

        builder.addAction(
            Notification.Action.Builder(
                Icon.createWithResource(context, R.drawable.baseline_headset_off_24),
                context.getString(R.string.disconnect),
                PendingIntent.getBroadcast(
                    context,
                    1,
                    Intent().apply {
                        action = DeviceService.ACTION_DISCONNECT
                        `package` = context.packageName
                    },
                    PendingIntent.FLAG_IMMUTABLE,
                ),
            ).build(),
        )

        quickPresetNotificationSlots(quickPresetNames).forEach { (slotIndex, name) ->
            builder.addAction(
                buildQuickPresetNotificationAction(
                    slotIndex,
                    name,
                    icon = Icon.createWithResource(
                        context,
                        if (slotIndex == 0) R.drawable.counter_1_24px else R.drawable.counter_2_24px,
                    ),
                ),
            )
        }

        return builder.build()
    }

    private fun buildQuickPresetNotificationAction(presetId: Int, name: String, icon: Icon): Notification.Action =
        Notification.Action.Builder(
            icon,
            name,
            PendingIntent.getBroadcast(
                context,
                presetId + 2,
                Intent().apply {
                    action = DeviceService.ACTION_QUICK_PRESET
                    `package` = context.packageName
                    putExtra(DeviceService.INTENT_EXTRA_PRESET_SLOT_INDEX, presetId)
                },
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            ),
        ).build()
}
